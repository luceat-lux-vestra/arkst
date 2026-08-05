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
    let ir = ast_to_ir::ast_to_ir(&doc, source_id, project.metadata());
    CompileResult {
        ir,
        diagnostics: vec![],
    }
}

/// Options for the compilation pipeline.
#[derive(Debug, Clone, Default)]
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
    use crate::{CompileOptions, VirtualPathBuf, VirtualProjectBuilder};
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

    #[test]
    fn compile_uses_project_metadata_without_front_matter() {
        let project = VirtualProjectBuilder::new()
            .entry("main.qd")
            .expect("valid path")
            .add_source("main.qd", "hello")
            .expect("valid path")
            .title("Project Title")
            .author("Project Author")
            .date("2026-01-01")
            .field("custom", "project_value")
            .build()
            .unwrap();

        let result = super::compile(&project, &CompileOptions::default());
        assert_eq!(result.ir.metadata.title, Some("Project Title".into()));
        assert_eq!(result.ir.metadata.author, Some("Project Author".into()));
        assert_eq!(result.ir.metadata.date, Some("2026-01-01".into()));
        assert_eq!(result.ir.metadata.raw.len(), 1);
        assert_eq!(
            result.ir.metadata.raw[0],
            ("custom".into(), "project_value".into())
        );
    }

    #[test]
    fn compile_front_matter_overrides_typed_project_metadata() {
        let project = VirtualProjectBuilder::new()
            .entry("main.qd")
            .expect("valid path")
            .add_source(
                "main.qd",
                "---\ntitle: FM Title\nauthor: FM Author\ndate: 2025-12-31\n---\n\nhello",
            )
            .expect("valid path")
            .title("Project Title")
            .author("Project Author")
            .date("2026-01-01")
            .build()
            .unwrap();

        let result = super::compile(&project, &CompileOptions::default());
        // Front matter overrides project metadata
        assert_eq!(result.ir.metadata.title, Some("FM Title".into()));
        assert_eq!(result.ir.metadata.author, Some("FM Author".into()));
        assert_eq!(result.ir.metadata.date, Some("2025-12-31".into()));
    }

    #[test]
    fn compile_front_matter_overrides_custom_project_metadata() {
        let project = VirtualProjectBuilder::new()
            .entry("main.qd")
            .expect("valid path")
            .add_source("main.qd", "---\ncustom: fm_value\n---\n\nhello")
            .expect("valid path")
            .field("custom", "project_value")
            .build()
            .unwrap();

        let result = super::compile(&project, &CompileOptions::default());
        assert_eq!(result.ir.metadata.raw.len(), 1);
        assert_eq!(
            result.ir.metadata.raw[0],
            ("custom".into(), "fm_value".into())
        );
    }

    #[test]
    fn compile_preserves_non_overridden_project_metadata() {
        let project = VirtualProjectBuilder::new()
            .entry("main.qd")
            .expect("valid path")
            .add_source("main.qd", "---\ntitle: FM Title\n---\n\nhello")
            .expect("valid path")
            .title("Project Title")
            .author("Project Author")
            .field("custom", "project_value")
            .build()
            .unwrap();

        let result = super::compile(&project, &CompileOptions::default());
        // title overridden by front matter
        assert_eq!(result.ir.metadata.title, Some("FM Title".into()));
        // author preserved from project
        assert_eq!(result.ir.metadata.author, Some("Project Author".into()));
        // custom preserved from project (not in front matter)
        assert_eq!(result.ir.metadata.raw.len(), 1);
        assert_eq!(
            result.ir.metadata.raw[0],
            ("custom".into(), "project_value".into())
        );
    }

    #[test]
    fn source_ids_are_independent_of_builder_insertion_order() {
        // Build two projects with same sources but different insertion order
        let project1 = VirtualProjectBuilder::new()
            .entry("main.qd")
            .expect("valid path")
            .add_source("a.qd", "content a")
            .expect("valid path")
            .add_source("b.qd", "content b")
            .expect("valid path")
            .add_source("main.qd", "main")
            .expect("valid path")
            .build()
            .unwrap();

        let project2 = VirtualProjectBuilder::new()
            .entry("main.qd")
            .expect("valid path")
            .add_source("b.qd", "content b")
            .expect("valid path")
            .add_source("a.qd", "content a")
            .expect("valid path")
            .add_source("main.qd", "main")
            .expect("valid path")
            .build()
            .unwrap();

        // Each path should have the same SourceId regardless of insertion order
        let path_a = VirtualPathBuf::parse("a.qd").unwrap();
        let path_b = VirtualPathBuf::parse("b.qd").unwrap();
        let path_main = VirtualPathBuf::parse("main.qd").unwrap();

        let id_a_1 = project1.sources().get_id(&path_a).unwrap();
        let id_a_2 = project2.sources().get_id(&path_a).unwrap();
        assert_eq!(id_a_1, id_a_2);

        let id_b_1 = project1.sources().get_id(&path_b).unwrap();
        let id_b_2 = project2.sources().get_id(&path_b).unwrap();
        assert_eq!(id_b_1, id_b_2);

        let id_main_1 = project1.sources().get_id(&path_main).unwrap();
        let id_main_2 = project2.sources().get_id(&path_main).unwrap();
        assert_eq!(id_main_1, id_main_2);

        // Entry SourceId should also be the same
        assert_eq!(
            project1.sources().get_id(project1.entry()).unwrap(),
            project2.sources().get_id(project2.entry()).unwrap()
        );
    }

    #[test]
    fn compile_result_is_independent_of_source_insertion_order() {
        let project1 = VirtualProjectBuilder::new()
            .entry("main.qd")
            .expect("valid path")
            .add_source("a.qd", "content a")
            .expect("valid path")
            .add_source("b.qd", "content b")
            .expect("valid path")
            .add_source("main.qd", "# Main\n\n{{ a.qd }} {{ b.qd }}")
            .expect("valid path")
            .build()
            .unwrap();

        let project2 = VirtualProjectBuilder::new()
            .entry("main.qd")
            .expect("valid path")
            .add_source("b.qd", "content b")
            .expect("valid path")
            .add_source("a.qd", "content a")
            .expect("valid path")
            .add_source("main.qd", "# Main\n\n{{ a.qd }} {{ b.qd }}")
            .expect("valid path")
            .build()
            .unwrap();

        let result1 = super::compile(&project1, &CompileOptions::default());
        let result2 = super::compile(&project2, &CompileOptions::default());

        // Serialize and compare
        let json1 = serde_json::to_string(&result1.ir).unwrap();
        let json2 = serde_json::to_string(&result2.ir).unwrap();
        assert_eq!(json1, json2);

        // Also verify all span SourceIds match
        for (span1, span2) in result1.ir.nodes.iter().zip(&result2.ir.nodes) {
            // Nodes should have same SourceIds in their spans
            assert_eq!(span1, span2);
        }
    }
}
