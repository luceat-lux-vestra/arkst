//! Adapters from the in-memory project model to engine-neutral inputs.

use arkst_engine::{
    DocumentMetadataDefaults, IncludedSource, ResourceAccessError, ResourceProvider,
    ResourceRoot as EngineResourceRoot, ResourceText,
};
use arkst_project::{
    ProjectMetadata, ResourceAccessError as ProjectResourceAccessError,
    ResourceRoot as ProjectResourceRoot, VirtualProject,
};
use arkst_source::SourceId;

pub(crate) fn document_metadata_defaults(metadata: &ProjectMetadata) -> DocumentMetadataDefaults {
    DocumentMetadataDefaults {
        title: metadata.title().map(ToString::to_string),
        author: metadata.author().map(ToString::to_string),
        date: metadata.date().map(ToString::to_string),
        fields: metadata.fields().to_vec(),
    }
}

/// Core-owned adapter that keeps project path and store semantics outside the
/// engine while exposing only the evaluator's semantic resource operations.
#[derive(Debug, Clone, Copy)]
pub(crate) struct VirtualProjectResourceProvider<'a> {
    project: &'a VirtualProject,
}

impl<'a> VirtualProjectResourceProvider<'a> {
    pub(crate) fn new(project: &'a VirtualProject) -> Self {
        Self { project }
    }
}

impl ResourceProvider for VirtualProjectResourceProvider<'_> {
    fn source_path(&self, source_id: SourceId) -> Option<String> {
        self.project
            .sources()
            .path_by_id(source_id)
            .map(ToString::to_string)
    }

    fn relative_path_to_root(
        &self,
        source_id: SourceId,
        root: EngineResourceRoot,
    ) -> Result<String, ResourceAccessError> {
        let root = match root {
            EngineResourceRoot::Project => ProjectResourceRoot::Project,
            EngineResourceRoot::Source(source_id) => ProjectResourceRoot::Source(source_id),
        };
        self.project
            .relative_path_to_resource_root(source_id, root)
            .map_err(map_resource_error)
    }

    fn read_text(
        &self,
        source_id: SourceId,
        reference: &str,
    ) -> Result<ResourceText, ResourceAccessError> {
        let (path, text) = self
            .project
            .read_resource_text(source_id, reference)
            .map_err(map_resource_error)?;
        Ok(ResourceText {
            path: path.to_string(),
            text,
        })
    }

    fn read_source(
        &self,
        source_id: SourceId,
        reference: &str,
    ) -> Result<IncludedSource, ResourceAccessError> {
        let path = self
            .project
            .resolve_resource_path(source_id, reference)
            .map_err(map_resource_error)?;
        let Some((text, target_id)) = self.project.sources().get_with_id(&path) else {
            return Err(ResourceAccessError::NotFound {
                path: path.to_string(),
            });
        };
        Ok(IncludedSource {
            path: path.to_string(),
            source_id: target_id,
            text: text.to_string(),
        })
    }
}

fn map_resource_error(error: ProjectResourceAccessError) -> ResourceAccessError {
    match error {
        ProjectResourceAccessError::UnsupportedReference { reference } => {
            ResourceAccessError::UnsupportedReference { reference }
        }
        ProjectResourceAccessError::UnknownSource(source_id) => {
            ResourceAccessError::UnknownSource { source_id }
        }
        ProjectResourceAccessError::Boundary(error) => ResourceAccessError::Boundary {
            message: error.to_string(),
        },
        ProjectResourceAccessError::NotFound(path) => ResourceAccessError::NotFound {
            path: path.to_string(),
        },
        ProjectResourceAccessError::InvalidUtf8 { path, message } => {
            ResourceAccessError::InvalidUtf8 {
                path: path.to_string(),
                message,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arkst_project::{VirtualPathBuf, VirtualProjectBuilder};

    fn project() -> VirtualProject {
        VirtualProjectBuilder::new()
            .entry("docs/main.qd")
            .expect("valid entry")
            .add_source("docs/main.qd", "root")
            .expect("valid source")
            .add_source("docs/guide/chapter.qd", "chapter")
            .expect("valid source")
            .add_source("docs/shared/target.md", "target")
            .expect("valid source")
            .add_source("docs/shared/nested/child.qd", "child")
            .expect("valid source")
            .add_source("docs/guide/theme", "local theme")
            .expect("valid source")
            .build()
            .expect("valid project")
    }

    fn source_id(project: &VirtualProject, path: &str) -> SourceId {
        project
            .sources()
            .get_id(&VirtualPathBuf::parse(path).expect("valid logical path"))
            .expect("source exists")
    }

    #[test]
    fn source_reads_preserve_subdocument_identity_across_normalized_aliases() {
        let project = project();
        let chapter = source_id(&project, "docs/guide/chapter.qd");
        let target = source_id(&project, "docs/shared/target.md");
        let provider = VirtualProjectResourceProvider::new(&project);

        for reference in [
            "../shared/target.md",
            "../shared/./target.md",
            "../shared//target.md",
            "../shared/tmp/../target.md",
        ] {
            let source = provider
                .read_source(chapter, reference)
                .expect("target resolves");
            assert_eq!(source.path, "docs/shared/target.md", "{reference}");
            assert_eq!(source.source_id, target, "{reference}");
            assert_eq!(source.text, "target", "{reference}");
        }
    }

    #[test]
    fn nested_subdocument_targets_use_the_defining_source_identity_as_their_base() {
        let project = project();
        let target = source_id(&project, "docs/shared/target.md");
        let child = source_id(&project, "docs/shared/nested/child.qd");
        let provider = VirtualProjectResourceProvider::new(&project);

        let source = provider
            .read_source(target, "nested/child.qd")
            .expect("nested target resolves from its defining source");
        assert_eq!(source.path, "docs/shared/nested/child.qd");
        assert_eq!(source.source_id, child);
        assert_eq!(source.text, "child");
    }

    #[test]
    fn subdocument_source_reads_fail_closed_for_invalid_or_missing_targets() {
        let project = project();
        let chapter = source_id(&project, "docs/guide/chapter.qd");
        let provider = VirtualProjectResourceProvider::new(&project);

        assert!(matches!(
            provider.read_source(chapter, "missing.md"),
            Err(ResourceAccessError::NotFound { path }) if path == "docs/guide/missing.md"
        ));
        for reference in [
            "https://example.test/target.qd",
            "/target.qd",
            r"C:\target.qd",
        ] {
            assert!(matches!(
                provider.read_source(chapter, reference),
                Err(ResourceAccessError::UnsupportedReference { .. })
            ));
        }
        assert!(matches!(
            provider.read_source(chapter, "../../../outside.qd"),
            Err(ResourceAccessError::Boundary { .. })
        ));
        assert!(matches!(
            provider.read_source(SourceId(u32::MAX - 1), "target.qd"),
            Err(ResourceAccessError::UnknownSource { .. })
        ));
    }

    #[test]
    fn bare_names_remain_local_until_an_explicit_library_registry_intercepts() {
        let project = project();
        let chapter = source_id(&project, "docs/guide/chapter.qd");
        let local_theme = source_id(&project, "docs/guide/theme");
        let provider = VirtualProjectResourceProvider::new(&project);

        // Quarkdown checks its explicit, case-sensitive loadable-library
        // registry before file lookup. The logical resource provider owns only
        // the second step and therefore must not infer a library from syntax.
        let source = provider
            .read_source(chapter, "theme")
            .expect("bare local path resolves normally");
        assert_eq!(source.path, "docs/guide/theme");
        assert_eq!(source.source_id, local_theme);
        assert_eq!(source.text, "local theme");

        assert!(matches!(
            provider.read_source(chapter, "Theme"),
            Err(ResourceAccessError::NotFound { path }) if path == "docs/guide/Theme"
        ));
    }
}
