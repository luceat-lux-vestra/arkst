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
pub mod virtual_project;

pub use diagnostics::*;
pub use source::*;
pub use virtual_project::*;

/// The Scribium core result type.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Top-level error type for the core crate.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Diagnostic(#[from] Diagnostic),
    #[error("internal error: {0}")]
    Internal(String),
}

/// Compile a Scribium project through the full pipeline.
///
/// Returns a `CompileResult` with the generated IR and diagnostics.
/// The entry point source and its `SourceId` come from the project's
/// `SourceStore`; no global ID generator is involved.
pub fn compile(project: &VirtualProject, _options: &CompileOptions) -> CompileResult {
    let entry = project.entry();

    // Use get_with_id to get both source and SourceId atomically.
    let Some((source, source_id)) = project.sources().get_with_id(entry) else {
        return CompileResult {
            ir: ir::IrDocument {
                nodes: vec![],
                metadata: ir::IrMetadata::default(),
            },
            diagnostics: vec![Diagnostic {
                code: "E9001".to_string(),
                severity: Severity::Error,
                message: "internal error: entry source or SourceId missing in project".to_string(),
                primary: None,
                secondary: vec![],
                hints: vec![
                    "this indicates an internal VirtualProject invariant violation".to_string(),
                ],
            }],
        };
    };

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
    use crate::{CompileOptions, VirtualProjectBuilder};

    #[test]
    fn it_compiles_empty_document() {
        let project = VirtualProjectBuilder::new()
            .entry("main.qd")
            .expect("valid path")
            .add_source("main.qd", "")
            .expect("valid path")
            .build()
            .unwrap();
        let result = super::compile(
            &project,
            &CompileOptions {
                compatibility_profile: None,
            },
        );
        assert!(result.ir.nodes.is_empty());
    }
}
