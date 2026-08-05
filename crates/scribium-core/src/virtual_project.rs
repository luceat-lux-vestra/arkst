//! VirtualProject - I/O-free compilation project representation.
//!
//! `VirtualProject` represents a complete compilation unit without any
//! filesystem access. This enables:
//! - WASM compilation (no filesystem access)
//! - Deterministic builds
//! - Easy testing with in-memory fixtures
//! - CLI and WASM builds from same core

use crate::source::{AssetStore, SourceStore, VirtualPathBuf, VirtualPathError};
use crate::source::{AssetStoreError, SourceStoreError};

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
}
/// Metadata extracted from document front matter.
///
/// Typed fields (`title`, `author`, `date`) are managed by their respective setters.
/// The `field` method adds arbitrary key-value pairs to `raw` and also syncs
/// typed fields when the key matches a known typed field (last-wins).
/// Unknown/custom keys are stored only in `raw`.
#[derive(Debug, Clone, Default)]
pub struct ProjectMetadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub date: Option<String>,
    pub raw: Vec<(String, String)>,
}

impl ProjectMetadata {
    /// Sets the title.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Sets the author.
    pub fn author(mut self, author: impl Into<String>) -> Self {
        self.author = Some(author.into());
        self
    }

    /// Sets the date.
    pub fn date(mut self, date: impl Into<String>) -> Self {
        self.date = Some(date.into());
        self
    }

    /// Adds a custom metadata field (last-wins for duplicates).
    pub fn field(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let key = key.into();
        let value = value.into();

        // Remove any existing entry with the same key
        self.raw.retain(|(k, _)| k != &key);
        self.raw.push((key.clone(), value.clone()));

        // Sync typed fields for special keys
        match key.as_str() {
            "title" => self.title = Some(value),
            "author" => self.author = Some(value),
            "date" => self.date = Some(value),
            _ => {}
        }

        self
    }
}

/// Builder for constructing a VirtualProject.
///
/// Supports building from disk (CLI) or from memory (WASM/testing).
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
        self.metadata = self.metadata.title(title);
        self
    }

    /// Sets the author metadata.
    pub fn author(mut self, author: impl Into<String>) -> Self {
        self.metadata = self.metadata.author(author);
        self
    }

    /// Sets the date metadata.
    pub fn date(mut self, date: impl Into<String>) -> Self {
        self.metadata = self.metadata.date(date);
        self
    }

    /// Adds a custom metadata field (last-wins for duplicates).
    pub fn field(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata = self.metadata.field(key, value);
        self
    }

    /// Builds the VirtualProject.
    pub fn build(self) -> Result<VirtualProject, BuildError> {
        let entry = self.entry.ok_or(BuildError::MissingEntry)?;

        let mut sources = SourceStore::new();
        for (path, content) in self.sources {
            sources.insert(path, content).map_err(|e| match e {
                SourceStoreError::DuplicateSource(p) => BuildError::DuplicateSource(p),
                SourceStoreError::SourceIdExhausted => BuildError::SourceIdExhausted,
            })?;
        }

        // Validate entry exists in sources
        if !sources.contains(&entry) {
            return Err(BuildError::EntryNotFound(entry));
        }

        let mut assets = AssetStore::new();
        for (path, data) in self.assets {
            assets
                .insert(path, std::sync::Arc::from(data.into_boxed_slice()))
                .map_err(|e| match e {
                    AssetStoreError::DuplicateAsset(p) => BuildError::DuplicateAsset(p),
                })?;
        }

        Ok(VirtualProject::from_builder(
            entry,
            sources,
            assets,
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
    use crate::source::virtual_path::VirtualPathBuf;

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
        assert_eq!(project.metadata().raw.len(), 1);
        assert_eq!(project.metadata().raw[0].1, "New");
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
