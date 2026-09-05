//! Shared logical resource-resolution primitives for project-backed language features.
//!
//! This module keeps source-relative and project-relative lookup in the
//! in-memory [`VirtualProject`] path space. It does not perform host I/O and it
//! deliberately does not guess whether a bare string is a loadable-library
//! name: library registration is a separate semantic capability from path
//! resolution.

use crate::{ResourceAccessError, ResourceReference, VirtualPathBuf, VirtualProject};
use arkst_source::SourceId;

/// The logical directory used as the base for resolving one resource reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceResolutionBase {
    /// Resolve from the logical project root.
    ProjectRoot,
    /// Resolve from the parent directory of the identified source.
    Source(SourceId),
}

/// A logical root used when projecting a relative path from the current source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceRoot {
    /// The root of the complete [`VirtualProject`].
    Project,
    /// The parent directory of a source that defines a nested document root.
    Source(SourceId),
}

impl VirtualProject {
    /// Resolves a local logical reference from an explicit project or source base.
    ///
    /// Source-relative resolution delegates to the existing resource contract,
    /// so current `.read`, `.json`, and `.include` behavior keeps exactly one
    /// source-relative authority. Project-relative resolution uses the same
    /// reference classification and [`VirtualPathBuf`] normalization rules.
    pub fn resolve_logical_resource(
        &self,
        base: ResourceResolutionBase,
        reference: &str,
    ) -> Result<VirtualPathBuf, ResourceAccessError> {
        match base {
            ResourceResolutionBase::Source(source_id) => {
                self.resolve_resource_path(source_id, reference)
            }
            ResourceResolutionBase::ProjectRoot => {
                resolve_from_directory(VirtualPathBuf::root(), reference)
            }
        }
    }

    /// Returns the relative logical path from `current_source`'s directory to
    /// an explicit project or nested-source root.
    ///
    /// The result is `.` when both directories are identical. Otherwise it is
    /// a forward-slash path composed only from `..` and canonical project path
    /// components. No host path or platform separator can enter the result.
    pub fn relative_path_to_resource_root(
        &self,
        current_source: SourceId,
        root: ResourceRoot,
    ) -> Result<String, ResourceAccessError> {
        let current_directory = self.source_directory(current_source)?;
        let root_directory = match root {
            ResourceRoot::Project => VirtualPathBuf::root(),
            ResourceRoot::Source(source_id) => self.source_directory(source_id)?,
        };

        Ok(relative_logical_path(&current_directory, &root_directory))
    }

    fn source_directory(&self, source_id: SourceId) -> Result<VirtualPathBuf, ResourceAccessError> {
        let source_path = self
            .sources()
            .path_by_id(source_id)
            .ok_or(ResourceAccessError::UnknownSource(source_id))?;
        Ok(source_path.parent().unwrap_or_else(VirtualPathBuf::root))
    }
}

fn resolve_from_directory(
    directory: VirtualPathBuf,
    reference: &str,
) -> Result<VirtualPathBuf, ResourceAccessError> {
    let ResourceReference::LocalPath(reference) = ResourceReference::classify(reference) else {
        return Err(ResourceAccessError::UnsupportedReference {
            reference: reference.to_string(),
        });
    };

    directory
        .join(reference)
        .map_err(ResourceAccessError::Boundary)
}

fn relative_logical_path(from: &VirtualPathBuf, to: &VirtualPathBuf) -> String {
    let from_components = logical_components(from);
    let to_components = logical_components(to);
    let common = from_components
        .iter()
        .zip(&to_components)
        .take_while(|(left, right)| left == right)
        .count();

    let mut result = Vec::with_capacity(
        from_components.len().saturating_sub(common) + to_components.len().saturating_sub(common),
    );
    result.extend(std::iter::repeat_n("..", from_components.len() - common));
    result.extend(to_components[common..].iter().copied());

    if result.is_empty() {
        ".".to_string()
    } else {
        result.join("/")
    }
}

fn logical_components(path: &VirtualPathBuf) -> Vec<&str> {
    if path.is_root() {
        Vec::new()
    } else {
        path.as_str().split('/').collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::VirtualProjectBuilder;

    fn project() -> VirtualProject {
        VirtualProjectBuilder::new()
            .entry("docs/main.qd")
            .unwrap()
            .add_source("docs/main.qd", "main")
            .unwrap()
            .add_source("docs/partials/a.qd", "partial")
            .unwrap()
            .add_source("subdocuments/root.qd", "subdocument")
            .unwrap()
            .add_source("subdocuments/includes/example.qd", "nested")
            .unwrap()
            .add_source("utils/example.qd", "utility")
            .unwrap()
            .build()
            .unwrap()
    }

    fn source_id(project: &VirtualProject, path: &str) -> SourceId {
        project
            .sources()
            .get_id(&VirtualPathBuf::parse(path).unwrap())
            .unwrap()
    }

    #[test]
    fn resolves_source_and_project_bases_without_host_paths() {
        let project = project();
        let main = source_id(&project, "docs/main.qd");

        assert_eq!(
            project
                .resolve_logical_resource(ResourceResolutionBase::Source(main), "partials/a.qd",)
                .unwrap(),
            VirtualPathBuf::parse("docs/partials/a.qd").unwrap()
        );
        assert_eq!(
            project
                .resolve_logical_resource(
                    ResourceResolutionBase::ProjectRoot,
                    "subdocuments/root.qd",
                )
                .unwrap(),
            VirtualPathBuf::parse("subdocuments/root.qd").unwrap()
        );
    }

    #[test]
    fn equivalent_references_preserve_one_canonical_source_identity() {
        let project = project();
        let main = source_id(&project, "docs/main.qd");
        let expected_path = VirtualPathBuf::parse("docs/partials/a.qd").unwrap();
        let expected_id = project.sources().get_id(&expected_path).unwrap();

        for reference in [
            "partials/a.qd",
            "./partials//a.qd",
            "scratch/../partials/a.qd",
        ] {
            let resolved = project
                .resolve_logical_resource(ResourceResolutionBase::Source(main), reference)
                .unwrap();
            assert_eq!(resolved, expected_path, "{reference}");
            assert_eq!(project.sources().get_id(&resolved), Some(expected_id));
        }
    }

    #[test]
    fn resolution_fails_closed_for_nonlocal_and_escaping_references() {
        let project = project();
        let main = source_id(&project, "docs/main.qd");

        for reference in ["/etc/passwd", r"C:\\secret.txt", "https://example.test/a"] {
            assert!(matches!(
                project.resolve_logical_resource(ResourceResolutionBase::Source(main), reference,),
                Err(ResourceAccessError::UnsupportedReference { .. })
            ));
        }

        assert!(matches!(
            project.resolve_logical_resource(
                ResourceResolutionBase::Source(main),
                "../../outside.txt",
            ),
            Err(ResourceAccessError::Boundary(_))
        ));
        assert!(matches!(
            project
                .resolve_logical_resource(ResourceResolutionBase::ProjectRoot, "../outside.txt",),
            Err(ResourceAccessError::Boundary(_))
        ));
    }

    #[test]
    fn unknown_source_never_falls_back_to_entry_or_project_root() {
        let project = project();
        let unknown = SourceId(u32::MAX - 1);

        assert_eq!(
            project
                .resolve_logical_resource(ResourceResolutionBase::Source(unknown), "docs/main.qd",),
            Err(ResourceAccessError::UnknownSource(unknown))
        );
        assert_eq!(
            project.relative_path_to_resource_root(unknown, ResourceRoot::Project),
            Err(ResourceAccessError::UnknownSource(unknown))
        );
    }

    #[test]
    fn project_root_projection_is_canonical_and_platform_independent() {
        let project = project();
        let main = source_id(&project, "docs/main.qd");
        let utility = source_id(&project, "utils/example.qd");
        let nested = source_id(&project, "subdocuments/includes/example.qd");

        assert_eq!(
            project
                .relative_path_to_resource_root(main, ResourceRoot::Project)
                .unwrap(),
            ".."
        );
        assert_eq!(
            project
                .relative_path_to_resource_root(utility, ResourceRoot::Project)
                .unwrap(),
            ".."
        );
        assert_eq!(
            project
                .relative_path_to_resource_root(nested, ResourceRoot::Project)
                .unwrap(),
            "../.."
        );
    }

    #[test]
    fn nested_source_root_projection_matches_source_parent_semantics() {
        let project = project();
        let subdocument = source_id(&project, "subdocuments/root.qd");
        let utility = source_id(&project, "utils/example.qd");
        let nested = source_id(&project, "subdocuments/includes/example.qd");

        assert_eq!(
            project
                .relative_path_to_resource_root(utility, ResourceRoot::Source(subdocument))
                .unwrap(),
            "../subdocuments"
        );
        assert_eq!(
            project
                .relative_path_to_resource_root(nested, ResourceRoot::Source(subdocument))
                .unwrap(),
            ".."
        );
        assert_eq!(
            project
                .relative_path_to_resource_root(subdocument, ResourceRoot::Source(subdocument))
                .unwrap(),
            "."
        );
    }
}
