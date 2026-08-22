//! Platform-neutral contract for Typst compiler adapters.

use std::time::Duration;

/// Abstract interface for a Typst compiler backend.
///
/// Concrete execution errors remain owned by the adapter that produces them;
/// this contract therefore exposes no filesystem, process, or host-path type.
pub trait TypstBackend {
    type Error: std::error::Error;

    /// Compile a Typst source document.
    fn compile(&self, input: &TypstInput) -> Result<TypstOutput, Self::Error>;

    /// Return the Typst compiler version.
    fn version(&self) -> Result<String, Self::Error>;
}

/// Input to a Typst compiler adapter.
pub struct TypstInput {
    pub source: String,
    /// Project-root-relative logical path of the Scribium source entry.
    ///
    /// A native adapter validates this value before interpreting it in a host
    /// project context. Lowering itself treats it as opaque contract data.
    pub entry_path: String,
}

/// Output from a Typst compiler adapter.
#[derive(Debug)]
pub struct TypstOutput {
    pub pdf: Option<Vec<u8>>,
    pub html: Option<String>,
    pub svg: Option<Vec<u8>>,
    pub png: Option<Vec<u8>>,
    pub diagnostics: Vec<String>,
    pub duration: Duration,
}
