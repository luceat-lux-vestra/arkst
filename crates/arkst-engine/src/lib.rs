//! Arkst's platform-neutral semantic compilation engine.
//!
//! The engine owns AST-to-IR conversion, semantic evaluation, builtin
//! dispatch, value conversion, and normalization. It accepts only immutable
//! semantic inputs and a narrow resource provider; project composition and
//! host I/O remain outside this crate.

pub mod ast_to_ir;
pub mod builtins;
pub mod csv_resource;
pub(crate) mod doclang_v260;
pub mod evaluator;
pub(crate) mod invocation_binder;
pub(crate) mod library_inspection;
pub(crate) mod locale;
pub(crate) mod unicode_case;
pub(crate) mod value_conversion;
pub(crate) mod word_break;

/// Deterministic semantic resource limits for one evaluator compilation.
///
/// `max_materialized_elements` is a per-operation bound: every finite range
/// or iterable/materialization operation may produce at most this many
/// elements. `max_evaluation_depth` bounds active evaluator call and callback
/// frames for one compilation. These limits are semantic, platform-neutral,
/// and independent of host process or allocator behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvaluationLimits {
    /// Maximum number of elements produced by one finite materialization
    /// operation, including closed-range materialization.
    pub max_materialized_elements: usize,
    /// Maximum number of active evaluator call/callback frames.
    pub max_evaluation_depth: usize,
}

impl Default for EvaluationLimits {
    fn default() -> Self {
        Self {
            // Existing fixtures and ordinary documents are far below this
            // bound, while a document cannot silently request an unbounded
            // range allocation.
            max_materialized_elements: 1_000_000,
            // This leaves ample room for ordinary nested components and
            // functions without relying on the native thread stack size.
            max_evaluation_depth: 256,
        }
    }
}

/// Explicit deterministic environment input for one evaluation.
///
/// Supplying this value is both the authorization boundary for Quarkdown
/// `.env` and the complete set of environment values visible to the
/// evaluator. The engine never falls back to `std::env`, process-global
/// state, a working directory, or another ambient host source. An empty map is
/// therefore an explicitly authorized environment in which every lookup is
/// absent, while omitting `EnvironmentInputs` denies process-environment
/// access entirely.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnvironmentInputs {
    values: std::collections::BTreeMap<String, String>,
}

impl EnvironmentInputs {
    /// Creates an explicit environment snapshot from deterministic key/value
    /// pairs. Later duplicate keys follow `BTreeMap`'s last-write behavior.
    pub fn new(values: std::collections::BTreeMap<String, String>) -> Self {
        Self { values }
    }

    /// Returns the exact injected value for `name`, if present.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(String::as_str)
    }

    /// Returns whether this authorized environment snapshot contains no
    /// variables.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

impl FromIterator<(String, String)> for EnvironmentInputs {
    fn from_iter<T: IntoIterator<Item = (String, String)>>(iter: T) -> Self {
        Self {
            values: iter.into_iter().collect(),
        }
    }
}

/// The closed evaluator capability set used by the compatibility pipeline.
///
/// This is deliberately narrow: granting native content authorizes creation
/// of the opaque target-specific semantic payload, not execution, parsing, or
/// access to any host capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Capability {
    NativeContent,
}

/// Explicit evaluator capabilities for one compilation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Capabilities {
    native_content: bool,
}

impl Capabilities {
    /// The normal Quarkdown-compatible default, matching v2.5.1.
    pub const fn compatibility_default() -> Self {
        Self {
            native_content: true,
        }
    }

    /// No optional evaluator capabilities are granted.
    pub const fn none() -> Self {
        Self {
            native_content: false,
        }
    }

    /// Returns a copy with NativeContent explicitly granted or denied.
    pub const fn with_native_content(self, granted: bool) -> Self {
        Self {
            native_content: granted,
        }
    }

    pub const fn allows(self, capability: Capability) -> bool {
        match capability {
            Capability::NativeContent => self.native_content,
        }
    }
}

impl Default for Capabilities {
    fn default() -> Self {
        Self::compatibility_default()
    }
}

/// Immutable metadata defaults supplied by the project/composition layer.
///
/// Document front matter is applied by AST-to-IR conversion and overrides
/// these values. The engine does not own project metadata or project
/// lifecycle; this plain-data boundary is all the conversion stage consumes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DocumentMetadataDefaults {
    pub title: Option<String>,
    pub author: Option<String>,
    pub date: Option<String>,
    pub fields: Vec<(String, String)>,
}

/// A successfully read logical text resource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceText {
    pub path: String,
    pub text: String,
}

/// Metadata for an existing logical project resource.
///
/// This carries canonical logical identity only. It deliberately exposes no
/// bytes, native path, timestamps, permissions, or other host filesystem state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceMetadata {
    pub path: String,
}

/// Kind of one logical directory entry returned by a resource provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceEntryKind {
    File,
    Directory,
}

/// One platform-neutral logical directory entry.
///
/// The entry exposes only its bare logical name and kind. Native paths,
/// timestamps, permissions, and other host filesystem metadata are not part
/// of this semantic boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceDirectoryEntry {
    pub name: String,
    pub kind: ResourceEntryKind,
}

/// A successfully resolved and read logical project source.
///
/// The source identity is part of the resource contract. `.include` uses it
/// for nested source-relative access, cycle detection, and source-backed
/// diagnostics; subdocument registration consumers can reuse the same
/// canonical identity without introducing a second resolver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncludedSource {
    pub path: String,
    pub source_id: arkst_source::SourceId,
    pub text: String,
}

/// Immutable source returned by an explicit loadable-library registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadableLibrarySource {
    pub name: String,
    pub source_id: arkst_source::SourceId,
    pub text: String,
}

/// Semantic lookup for host-supplied loadable libraries.
///
/// This is intentionally distinct from `ResourceProvider`: a bare `.include`
/// argument is first matched exactly against this registry and only falls back
/// to logical project-path resolution when no library exists.
pub trait LoadableLibraryProvider {
    fn loadable_library(&self, name: &str) -> Option<LoadableLibrarySource>;
}

/// Semantic severity/category of one Quarkdown logger event.
///
/// These values are evaluator semantics only. They do not imply stdout,
/// stderr, a process logger, or any other host destination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Log,
    Debug,
}

/// One source-backed logger event emitted by evaluator semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEvent {
    pub level: LogLevel,
    pub message: String,
    pub span: arkst_source::SourceSpan,
}

/// Explicit host sink for Quarkdown logger events.
///
/// The evaluator never discovers or writes ambient process streams. A host
/// that wants `.log` / `.debug` observability may inject this sink for the
/// evaluation. `.log` requires an explicit sink, while `.debug` remains a
/// silent no-op when no sink is present, matching the observed v2.6 CLI
/// default. `.error` remains independent of this sink: the evaluator records a
/// structured compiler diagnostic plus a backend-neutral explicit-error component,
/// so its failure/rollback semantics never depend on host logging or ambient I/O.
pub trait LogSink {
    fn emit(&self, event: &LogEvent);
}

/// Logical root requested by a resource-aware evaluator operation.
///
/// The engine sees source identities only. Project composition remains
/// responsible for mapping those identities to canonical logical paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceRoot {
    /// The root of the complete logical project.
    Project,
    /// The parent directory of a source that defines a nested document root.
    Source(arkst_source::SourceId),
}

/// Minimal semantic resource failures translated by the composition adapter.
///
/// These variants intentionally contain only stable semantic information and
/// no project path, store, filesystem, or host error types.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ResourceAccessError {
    #[error("resource provider does not support operation: {operation}")]
    UnsupportedOperation { operation: &'static str },
    #[error("resource reference is not a local project path: {reference}")]
    UnsupportedReference { reference: String },
    #[error("source identity is not present in the project: {source_id:?}")]
    UnknownSource { source_id: arkst_source::SourceId },
    #[error("resource path leaves the virtual project boundary: {message}")]
    Boundary { message: String },
    #[error("resource not found: {path}")]
    NotFound { path: String },
    #[error("resource path is not a directory: {path}")]
    NotDirectory { path: String },
    #[error("logical resource tree is inconsistent at: {path}")]
    InconsistentDirectoryTree { path: String },
    #[error("resource is not valid UTF-8: {path}: {message}")]
    InvalidUtf8 { path: String, message: String },
}

/// Semantic resource operations required by the current evaluator.
///
/// Implementations belong to the composition/host layer. The engine never
/// resolves native paths, accesses stores directly, performs I/O, or mutates
/// project state.
pub trait ResourceProvider {
    /// Returns the stable logical path used for diagnostics and include-cycle
    /// messages for a source identity.
    fn source_path(&self, source_id: arkst_source::SourceId) -> Option<String>;

    /// Returns the exact immutable source text for one known source identity.
    ///
    /// This is a provenance operation, not a resource-discovery capability.
    /// Providers that cannot expose source text may return `None`; callers
    /// must then fail closed rather than reconstruct source spelling.
    fn source_text(&self, _source_id: arkst_source::SourceId) -> Option<&str> {
        None
    }

    /// Returns the platform-neutral relative path from `source_id`'s parent
    /// directory to the requested logical root.
    fn relative_path_to_root(
        &self,
        source_id: arkst_source::SourceId,
        root: ResourceRoot,
    ) -> Result<String, ResourceAccessError> {
        let _ = root;
        Err(ResourceAccessError::UnknownSource { source_id })
    }

    /// Resolves an existing logical project resource without reading its bytes.
    ///
    /// Providers that do not support metadata lookup fail closed by default;
    /// they must opt in explicitly before `.filename` can observe resource identity.
    fn resource_metadata(
        &self,
        _source_id: arkst_source::SourceId,
        reference: &str,
    ) -> Result<ResourceMetadata, ResourceAccessError> {
        let _ = reference;
        Err(ResourceAccessError::UnsupportedOperation {
            operation: "resource_metadata",
        })
    }

    /// Enumerates an existing logical project directory.
    ///
    /// Providers opt in explicitly. The engine owns filtering and presentation;
    /// this operation only returns bare logical names/kinds for direct or
    /// recursive descendants and never exposes host filesystem metadata.
    fn list_directory(
        &self,
        _source_id: arkst_source::SourceId,
        reference: &str,
        _recursive: bool,
    ) -> Result<Vec<ResourceDirectoryEntry>, ResourceAccessError> {
        let _ = reference;
        Err(ResourceAccessError::UnsupportedOperation {
            operation: "list_directory",
        })
    }

    /// Reads any project resource as validated UTF-8 text.
    fn read_text(
        &self,
        source_id: arkst_source::SourceId,
        reference: &str,
    ) -> Result<ResourceText, ResourceAccessError>;

    /// Resolves and reads a project source while retaining canonical logical
    /// path and source identity.
    ///
    /// Non-path semantic dispatch, such as an explicit loadable-library
    /// registry, must happen before this method is called. Bare strings are
    /// deliberately not guessed to be library names by the resource layer.
    fn read_source(
        &self,
        source_id: arkst_source::SourceId,
        reference: &str,
    ) -> Result<IncludedSource, ResourceAccessError>;
}
