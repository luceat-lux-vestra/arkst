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
