//! Compatibility facade for the physically extracted document IR.

pub use scribium_ir::*;

use scribium_source::SourceSpan;

/// An entry in the source map linking a range of generated output
/// back to its originating source span.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SourceMapEntry {
    /// Range in the generated Typst source (byte offsets).
    pub generated_start: usize,
    pub generated_end: usize,
    /// The original source span this generated range belongs to.
    pub original: SourceSpan,
}
