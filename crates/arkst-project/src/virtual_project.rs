//! VirtualProject - I/O-free compilation project representation.
//!
//! `VirtualProject` represents a complete compilation unit without any
//! filesystem access. This enables:
//! - WASM compilation (no filesystem access)
//! - Deterministic builds
//! - Easy testing with in-memory fixtures
//! - CLI and WASM builds from same core

use crate::{
    AssetStore, AssetStoreError, ResourceAccessError, ResourceReference, SourceStore,
    SourceStoreError, VirtualPathBuf, VirtualPathError,
};
use arkst_source::SourceId;

/// A compilation project with all sources and assets in memory.
///
/// `VirtualProject` contains all sources and assets needed for compilation
/// without any filesystem access. This enables:
/// - WASM compilation (no filesystem access)
/// - Deterministic builds
/// - Easy testing with in-memory fixtures
/// - CLI and WASM builds from same core
#[derive(Debug, Clone)]
pub struct VirtualProject {
    entry: VirtualPathBuf,
    sources: SourceStore,
    assets: AssetStore,
    metadata: ProjectMetadata,
}

impl VirtualProject {
    /// Creates a new VirtualProject from a builder.
    ///
    /// This is the only way to construct a VirtualProject, ensuring all
    /// invariants are validated.
    pub(crate) fn from_builder(
        entry: VirtualPathBuf,
        sources: SourceStore,
        assets: AssetStore,
        metadata: ProjectMetadata,
    ) -> Self {
        Self {
            entry,
            sources,
            assets,
            metadata,
        }
    }

    /// Gets the entry point file.
    pub fn entry(&self) -> &VirtualPathBuf {
        &self.entry
    }

    /// Gets the source store.
    pub fn sources(&self) -> &SourceStore {
        &self.sources
    }

    /// Gets the asset store.
    pub fn assets(&self) -> &AssetStore {
        &self.assets
    }

    /// Gets the project metadata.
    pub fn metadata(&self) -> &ProjectMetadata {
        &self.metadata
    }

    /// Resolves a local logical resource relative to the source document that
    /// issued the request. This never touches the host filesystem.
    pub fn resolve_resource_path(
        &self,
        source_id: SourceId,
        reference: &str,
    ) -> Result<VirtualPathBuf, ResourceAccessError> {
        let ResourceReference::LocalPath(reference) = ResourceReference::classify(reference) else {
            return Err(ResourceAccessError::UnsupportedReference {
                reference: reference.to_string(),
            });
        };
        let source_path = self
            .sources
            .path_by_id(source_id)
            .ok_or(ResourceAccessError::UnknownSource(source_id))?;
        let base = source_path.parent().unwrap_or_else(VirtualPathBuf::root);
        base.join(reference).map_err(ResourceAccessError::Boundary)
    }

    /// Reads a project resource as bytes after source-relative resolution.
    pub fn read_resource_bytes(
        &self,
        source_id: SourceId,
        reference: &str,
    ) -> Result<(VirtualPathBuf, Vec<u8>), ResourceAccessError> {
        let path = self.resolve_resource_path(source_id, reference)?;
        if let Some(source) = self.sources.get(&path) {
            return Ok((path, source.as_bytes().to_vec()));
        }
        if let Some(asset) = self.assets.get(&path) {
            return Ok((path, asset.to_vec()));
        }
        Err(ResourceAccessError::NotFound(path))
    }

    /// Reads a project resource as UTF-8 text after source-relative
    /// resolution. The bytes are not normalized or lossily converted.
    pub fn read_resource_text(
        &self,
        source_id: SourceId,
        reference: &str,
    ) -> Result<(VirtualPathBuf, String), ResourceAccessError> {
        let (path, bytes) = self.read_resource_bytes(source_id, reference)?;
        let text = String::from_utf8(bytes).map_err(|error| ResourceAccessError::InvalidUtf8 {
            path: path.clone(),
            message: error.utf8_error().to_string(),
        })?;
        Ok((path, text))
    }
}

/// Project-level metadata defaults provided by the host (CLI, WASM embedder).
/// These are *not* extracted from document front matter; they serve as
/// defaults that document front matter can override on a per-document basis.
///
/// Precedence (highest to lowest):
/// - Document front matter (per-document, overrides everything)
/// - Project-level defaults (this struct)
/// - None
///
/// Typed fields (`title`, `author`, `date`) are stored in their dedicated
/// accessors. Custom keys are stored in `raw`.
/// Known keys (`title`, `author`, `date`) are never duplicated in `raw`.
/// Duplicate custom keys use last-wins semantics.
/// The order of entries in `raw` is not semantically meaningful; it is
/// normalized to lexicographic key order during IR construction.
#[derive(Debug, Clone, Default)]
pub struct ProjectMetadata {
    title: Option<String>,
    author: Option<String>,
    date: Option<String>,
    raw: Vec<(String, String)>,
}

impl ProjectMetadata {
    /// Gets the title.
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    /// Gets the author.
    pub fn author(&self) -> Option<&str> {
        self.author.as_deref()
    }

    /// Gets the date.
    pub fn date(&self) -> Option<&str> {
        self.date.as_deref()
    }

    /// Gets the custom fields.
    pub fn fields(&self) -> &[(String, String)] {
        &self.raw
    }

    /// Sets the title.
    pub fn set_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Sets the author.
    pub fn set_author(mut self, author: impl Into<String>) -> Self {
        self.author = Some(author.into());
        self
    }

    /// Sets the date.
    pub fn set_date(mut self, date: impl Into<String>) -> Self {
        self.date = Some(date.into());
        self
    }

    /// Adds a custom metadata field (last-wins for duplicates).
    /// Known keys (title, author, date) are stored only in their typed fields,
    /// not in raw.
    pub fn set_field(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let key = key.into();
        let value = value.into();

        match key.as_str() {
            "title" => self.title = Some(value),
            "author" => self.author = Some(value),
            "date" => self.date = Some(value),
            _ => {
                // Remove any existing entry with the same key
                self.raw.retain(|(k, _)| k != &key);
                self.raw.push((key, value));
            }
        }

        self
    }
}

/// Builder for constructing a VirtualProject.
///
/// Hosts load files and assets themselves before adding them to this builder;
/// the builder itself only accepts in-memory data.
#[derive(Default)]
pub struct VirtualProjectBuilder {
    entry: Option<VirtualPathBuf>,
    sources: Vec<(VirtualPathBuf, String)>,
    assets: Vec<(VirtualPathBuf, Vec<u8>)>,
    metadata: ProjectMetadata,
}

impl VirtualProjectBuilder {
    /// Creates a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the entry point file.
    pub fn entry(mut self, path: impl AsRef<str>) -> Result<Self, VirtualPathError> {
        let path = VirtualPathBuf::parse(path)?;
        if path.is_root() {
            return Err(VirtualPathError::RootPathNotAllowed);
        }
        self.entry = Some(path);
        Ok(self)
    }

    /// Adds a source file.
    pub fn add_source(
        mut self,
        path: impl AsRef<str>,
        content: impl Into<String>,
    ) -> Result<Self, VirtualPathError> {
        let path = VirtualPathBuf::parse(path)?;
        if path.is_root() {
            return Err(VirtualPathError::RootPathNotAllowed);
        }
        self.sources.push((path, content.into()));
        Ok(self)
    }

    /// Adds an asset (font, image, etc.).
    pub fn add_asset(
        mut self,
        path: impl AsRef<str>,
        data: Vec<u8>,
    ) -> Result<Self, VirtualPathError> {
        let path = VirtualPathBuf::parse(path)?;
        if path.is_root() {
            return Err(VirtualPathError::RootPathNotAllowed);
        }
        self.assets.push((path, data));
        Ok(self)
    }

    /// Sets the title metadata.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.metadata = self.metadata.set_title(title);
        self
    }

    /// Sets the author metadata.
    pub fn author(mut self, author: impl Into<String>) -> Self {
        self.metadata = self.metadata.set_author(author);
        self
    }

    /// Sets the date metadata.
    pub fn date(mut self, date: impl Into<String>) -> Self {
        self.metadata = self.metadata.set_date(date);
        self
    }

    /// Adds a custom metadata field (last-wins for duplicates).
    pub fn field(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata = self.metadata.set_field(key, value);
        self
    }

    /// Builds the VirtualProject.
    pub fn build(self) -> Result<VirtualProject, BuildError> {
        let entry = self.entry.ok_or(BuildError::MissingEntry)?;

        // Sort sources by canonical path to ensure deterministic SourceId assignment
        let mut sources = self.sources;
        sources.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));

        let mut source_store = SourceStore::new();
        for (path, content) in sources {
            source_store.insert(path, content).map_err(|e| match e {
                SourceStoreError::DuplicateSource(p) => BuildError::DuplicateSource(p),
                SourceStoreError::SourceIdExhausted => BuildError::SourceIdExhausted,
            })?;
        }

        // Validate entry exists in sources
        if !source_store.contains(&entry) {
            return Err(BuildError::EntryNotFound(entry));
        }

        // Sort assets by canonical path for deterministic iteration
        let mut assets = self.assets;
        assets.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));

        let mut asset_store = AssetStore::new();
        for (path, data) in assets {
            asset_store
                .insert(path, std::sync::Arc::from(data.into_boxed_slice()))
                .map_err(|e| match e {
                    AssetStoreError::DuplicateAsset(p) => BuildError::DuplicateAsset(p),
                })?;
        }

        Ok(VirtualProject::from_builder(
            entry,
            source_store,
            asset_store,
            self.metadata,
        ))
    }
}

/// Errors during VirtualProject building.
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("missing entry point")]
    MissingEntry,
    #[error("entry source not found: {0}")]
    EntryNotFound(VirtualPathBuf),
    #[error("duplicate source: {0}")]
    DuplicateSource(VirtualPathBuf),
    #[error("duplicate asset: {0}")]
    DuplicateAsset(VirtualPathBuf),
    #[error("source id space exhausted")]
    SourceIdExhausted,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::VirtualPathBuf;

    #[test]
    fn build_requires_entry() {
        let result = VirtualProjectBuilder::new().build();
        assert!(matches!(result, Err(BuildError::MissingEntry)));
    }

    #[test]
    fn build_requires_existing_entry_source() {
        let result = VirtualProjectBuilder::new()
            .entry("main.qd")
            .expect("valid path")
            .add_source("lib.qd", "content")
            .expect("valid path")
            .build();
        assert!(matches!(result, Err(BuildError::EntryNotFound(_))));
    }

    #[test]
    fn build_preserves_metadata() {
        let project = VirtualProjectBuilder::new()
            .entry("main.qd")
            .expect("valid path")
            .add_source("main.qd", "hello")
            .expect("valid path")
            .title("Example")
            .author("Tester")
            .date("2026-08-05")
            .build()
            .unwrap();

        assert_eq!(project.metadata().title, Some("Example".to_string()));
        assert_eq!(project.metadata().author, Some("Tester".to_string()));
        assert_eq!(project.metadata().date, Some("2026-08-05".to_string()));
    }

    #[test]
    fn typed_metadata_matches_raw_metadata() {
        let project = VirtualProjectBuilder::new()
            .entry("main.qd")
            .expect("valid path")
            .add_source("main.qd", "hello")
            .expect("valid path")
            .title("Example")
            .field("custom", "value")
            .build()
            .unwrap();

        assert_eq!(project.metadata().title, Some("Example".to_string()));
        assert_eq!(project.metadata().raw.len(), 1);
        assert_eq!(
            project.metadata().raw[0],
            ("custom".to_string(), "value".to_string())
        );
    }

    #[test]
    fn duplicate_metadata_uses_last_value() {
        let project = VirtualProjectBuilder::new()
            .entry("main.qd")
            .expect("valid path")
            .add_source("main.qd", "hello")
            .expect("valid path")
            .field("title", "Old")
            .field("title", "New")
            .build()
            .unwrap();

        assert_eq!(project.metadata().title, Some("New".to_string()));
        // Known keys (title, author, date) are not stored in raw
        assert_eq!(project.metadata().raw.len(), 0);
    }

    #[test]
    fn build_preserves_sources() {
        let project = VirtualProjectBuilder::new()
            .entry("main.qd")
            .expect("valid path")
            .add_source("main.qd", "main content")
            .expect("valid path")
            .add_source("lib.qd", "lib content")
            .expect("valid path")
            .build()
            .unwrap();

        assert_eq!(project.sources().len(), 2);
        assert_eq!(
            project
                .sources()
                .get(&VirtualPathBuf::parse("main.qd").unwrap()),
            Some("main content")
        );
        assert_eq!(
            project
                .sources()
                .get(&VirtualPathBuf::parse("lib.qd").unwrap()),
            Some("lib content")
        );
    }

    #[test]
    fn build_preserves_assets() {
        let project = VirtualProjectBuilder::new()
            .entry("main.qd")
            .expect("valid path")
            .add_source("main.qd", "hello")
            .expect("valid path")
            .add_asset("font.ttf", b"font data".to_vec())
            .expect("valid path")
            .build()
            .unwrap();

        assert_eq!(project.assets().len(), 1);
        assert_eq!(
            project
                .assets()
                .get(&VirtualPathBuf::parse("font.ttf").unwrap()),
            Some(b"font data".as_ref())
        );
    }

    #[test]
    fn resources_resolve_relative_to_the_source_identity() {
        let project = VirtualProjectBuilder::new()
            .entry("docs/main.qd")
            .unwrap()
            .add_source("docs/main.qd", ".read {data/main.txt}")
            .unwrap()
            .add_source("docs/partials/a.qd", ".read {data/nested.txt}")
            .unwrap()
            .add_asset("docs/data/main.txt", b"main".to_vec())
            .unwrap()
            .add_asset("docs/partials/data/nested.txt", b"nested".to_vec())
            .unwrap()
            .build()
            .unwrap();
        let main_id = project.sources().get_id(project.entry()).unwrap();
        let nested_id = project
            .sources()
            .get_id(&VirtualPathBuf::parse("docs/partials/a.qd").unwrap())
            .unwrap();

        assert_eq!(
            project
                .read_resource_text(main_id, "data/main.txt")
                .unwrap(),
            (
                VirtualPathBuf::parse("docs/data/main.txt").unwrap(),
                "main".into()
            )
        );
        assert_eq!(
            project
                .read_resource_text(nested_id, "data/nested.txt")
                .unwrap(),
            (
                VirtualPathBuf::parse("docs/partials/data/nested.txt").unwrap(),
                "nested".into()
            )
        );
    }

    #[test]
    fn resource_access_rejects_nonlocal_and_out_of_root_references() {
        let project = VirtualProjectBuilder::new()
            .entry("docs/main.qd")
            .unwrap()
            .add_source("docs/main.qd", "main")
            .unwrap()
            .build()
            .unwrap();
        let source_id = project.sources().get_id(project.entry()).unwrap();

        assert!(matches!(
            project.resolve_resource_path(source_id, "https://example.com/file"),
            Err(crate::ResourceAccessError::UnsupportedReference { .. })
        ));
        assert!(matches!(
            project.resolve_resource_path(source_id, "../../secret"),
            Err(crate::ResourceAccessError::Boundary(_))
        ));
    }

    #[test]
    fn invalid_utf8_is_not_lossily_decoded() {
        let project = VirtualProjectBuilder::new()
            .entry("main.qd")
            .unwrap()
            .add_source("main.qd", ".read {bad.bin}")
            .unwrap()
            .add_asset("bad.bin", vec![0xff, 0xfe])
            .unwrap()
            .build()
            .unwrap();
        let source_id = project.sources().get_id(project.entry()).unwrap();

        assert!(matches!(
            project.read_resource_text(source_id, "bad.bin"),
            Err(crate::ResourceAccessError::InvalidUtf8 { .. })
        ));
    }

    #[test]
    fn entry_path_is_canonicalized() {
        let project = VirtualProjectBuilder::new()
            .entry("src/./main.qd")
            .expect("valid path")
            .add_source("src/main.qd", "content")
            .expect("valid path")
            .build()
            .unwrap();

        assert_eq!(project.entry().as_str(), "src/main.qd");
    }

    #[test]
    fn reject_root_as_entry() {
        let result = VirtualProjectBuilder::new().entry("");
        assert!(result.is_err());
    }

    #[test]
    fn reject_root_as_source() {
        let result = VirtualProjectBuilder::new()
            .entry("main.qd")
            .expect("valid path")
            .add_source("", "content");
        assert!(result.is_err());
    }

    #[test]
    fn reject_root_as_asset() {
        let result = VirtualProjectBuilder::new()
            .entry("main.qd")
            .expect("valid path")
            .add_asset("", b"data".to_vec());
        assert!(result.is_err());
    }
}
