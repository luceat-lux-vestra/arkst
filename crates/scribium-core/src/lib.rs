/// `scribium-core` — Scribium's foundational library.
///
/// Responsibilities:
/// - Source abstraction and span management
/// - Markdown + Quarkdown-compatible parsing
/// - Semantic analysis and scope resolution
/// - Evaluator and built-in functions
/// - Document IR (Intermediate Representation)
/// - Source map construction and querying
/// - Structured diagnostics with stable codes
/// - Compatibility profile selection and divergence tracking
pub mod ast_to_ir;
pub mod builtins;
pub mod compatibility;
pub mod diagnostics;
pub mod evaluator;
pub mod ir;
pub mod source;
pub mod source_map;
pub mod syntax;

pub use diagnostics::*;
pub use source::*;

/// The Scribium core result type.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Top-level error type for the core crate.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A diagnostic-laden error from compilation.
    #[error("{0}")]
    Diagnostic(Diagnostic),

    /// An internal invariant violation (bug).
    #[error("internal error: {0}")]
    Internal(String),
}

/// Compile a Scribium source string through the full pipeline.
///
/// Returns a `CompileResult` with the generated Typst code and diagnostics.
pub fn compile(source: &str, _options: &CompileOptions) -> CompileResult {
    let source_id = source::fresh_source_id();
    let doc = syntax::markdown::parse(source);
    let ir = ast_to_ir::ast_to_ir(&doc, source_id);
    CompileResult {
        ir,
        diagnostics: vec![],
    }
}

/// Options for the compilation pipeline.
#[derive(Debug, Clone)]
pub struct CompileOptions {
    pub compatibility_profile: Option<String>,
}

/// Result of compilation through the frontend.
#[derive(Debug, Clone)]
pub struct CompileResult {
    pub ir: ir::IrDocument,
    pub diagnostics: Vec<Diagnostic>,
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles_empty_document() {
        let result = super::compile(
            "",
            &super::CompileOptions {
                compatibility_profile: None,
            },
        );
        assert!(result.ir.nodes.is_empty());
    }
}
