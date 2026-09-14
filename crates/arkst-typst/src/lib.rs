/// `arkst-typst` — platform-neutral Typst lowering for Arkst.
///
/// Responsibilities:
/// - Typst lowering (IR → Typst source code)
/// - platform-neutral Typst backend input/output contract
/// - Source map updates during lowering
pub mod backend;
#[path = "focus_lowering.rs"]
pub mod lowering;
#[path = "lowering.rs"]
mod lowering_base;

pub use backend::{TypstBackend, TypstInput, TypstOutput};
pub use lowering::{lower_to_typst, lower_to_typst_code};
/// The Arkst-Typst result type.
pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Typst compile failed: {0}")]
    Compile(String),
    #[error("Typst backend not available: {0}")]
    BackendUnavailable(String),
}
