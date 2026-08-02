/// `scribium-typst` — Typst backend for Scribium.
///
/// Responsibilities:
/// - Typst lowering (IR → Typst source code)
/// - TypstBackend trait definition
/// - Subprocess backend adapter
/// - Backend diagnostics conversion
/// - Source map updates during lowering
pub mod backend;
pub mod lowering;

/// The Scribium-Typst result type.
pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Typst compile failed: {0}")]
    Compile(String),
    #[error("Typst backend not available: {0}")]
    BackendUnavailable(String),
}
