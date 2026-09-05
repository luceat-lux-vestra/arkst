//! Arkst's platform-neutral semantic compilation engine.
//!
//! The engine owns AST-to-IR conversion, semantic evaluation, builtin
//! dispatch, value conversion, and normalization. It accepts only immutable
//! semantic inputs and a narrow resource provider; project composition and
//! host I/O remain outside this crate.

pub mod ast_to_ir;
pub mod builtins;
pub mod evaluator;
pub(crate) mod invocation_binder;
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
    #[error("resource reference is not a local project path: {reference}")]
    UnsupportedReference { reference: String },
    #[error("source identity is not present in the project: {source_id:?}")]
    UnknownSource { source_id: arkst_source::SourceId },
    #[error("resource path leaves the virtual project boundary: {message}")]
    Boundary { message: String },
    #[error("resource not found: {path}")]
    NotFound { path: String },
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
