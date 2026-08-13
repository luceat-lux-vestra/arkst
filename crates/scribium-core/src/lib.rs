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

    let parsed = if entry.as_str().ends_with(".md") {
        scribium_markdown::parse_with_mode(source, scribium_markdown::Mode::Markdown)
    } else {
        scribium_markdown::parse_with_diagnostics(source)
    };
    let (ir, lowering_diagnostics) =
        ast_to_ir::ast_to_ir_with_diagnostics(&parsed.document, source_id, project.metadata());
    let (ir, evaluation_diagnostics) = evaluator::Evaluator::new().evaluate(&ir);
    let mut diagnostics: Vec<Diagnostic> = parsed
        .diagnostics
        .into_iter()
        .map(|d| Diagnostic {
            code: d.code.to_string(),
            severity: Severity::Error,
            message: d.message,
            primary: Some(SourceSpan {
                source_id,
                start: d.span.start,
                end: d.span.end,
            }),
            secondary: Vec::new(),
            hints: Vec::new(),
        })
        .collect();
    diagnostics.extend(evaluation_diagnostics);
    diagnostics.extend(lowering_diagnostics);
    CompileResult { ir, diagnostics }
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
    use crate::ir::{IrInline, IrNode};
    use crate::{CompileOptions, Severity, VirtualPathBuf, VirtualProjectBuilder};
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
    fn known_metadata_keys_are_not_duplicated_in_raw() {
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

        // Typed fields from front matter should be in typed fields only, not in raw
        assert_eq!(result.ir.metadata.title, Some("FM Title".into()));
        assert_eq!(result.ir.metadata.author, Some("FM Author".into()));
        assert_eq!(result.ir.metadata.date, Some("2025-12-31".into()));

        // raw should be empty (no duplicate of title/author/date)
        assert_eq!(result.ir.metadata.raw.len(), 0);
    }

    #[test]
    fn custom_metadata_order_does_not_affect_compiled_ir() {
        // Build two projects with same custom metadata but different insertion order
        let project1 = VirtualProjectBuilder::new()
            .entry("main.qd")
            .expect("valid path")
            .add_source("main.qd", "hello")
            .expect("valid path")
            .field("zeta", "last")
            .field("alpha", "first")
            .field("epsilon", "middle")
            .build()
            .unwrap();

        let project2 = VirtualProjectBuilder::new()
            .entry("main.qd")
            .expect("valid path")
            .add_source("main.qd", "hello")
            .expect("valid path")
            .field("epsilon", "middle")
            .field("zeta", "last")
            .field("alpha", "first")
            .build()
            .unwrap();

        let result1 = super::compile(&project1, &CompileOptions::default());
        let result2 = super::compile(&project2, &CompileOptions::default());

        // IR metadata should be identical regardless of field insertion order
        assert_eq!(result1.ir.metadata.raw, result2.ir.metadata.raw);

        // Verify sorting: should be alphabetical by key
        assert_eq!(
            result1.ir.metadata.raw,
            vec![
                ("alpha".into(), "first".into()),
                ("epsilon".into(), "middle".into()),
                ("zeta".into(), "last".into()),
            ]
        );
    }

    #[test]
    fn source_ids_are_independent_of_builder_insertion_order() {
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

    fn compile_source(source: &str) -> (crate::CompileResult, crate::SourceId) {
        let project = VirtualProjectBuilder::new()
            .entry("main.qd")
            .expect("valid path")
            .add_source("main.qd", source)
            .expect("valid path")
            .build()
            .unwrap();
        let source_id = project.sources().get_id(project.entry()).unwrap();
        (
            super::compile(&project, &CompileOptions::default()),
            source_id,
        )
    }

    #[test]
    fn compile_propagates_parser_diagnostics() {
        for (input, expected_code) in [
            (".foo {", "E2003"),
            (".foo width:{x} {y}", "E2001"),
            (".foo key:", "E2002"),
        ] {
            let (result, source_id) = compile_source(input);
            assert_eq!(result.diagnostics.len(), 1, "input {input:?}");
            let diag = &result.diagnostics[0];
            assert_eq!(diag.code, expected_code, "input {input:?}");
            assert!(matches!(diag.severity, Severity::Error), "input {input:?}");
            assert!(!diag.message.is_empty(), "input {input:?}");
            assert_eq!(
                diag.primary.as_ref().map(|s| s.source_id),
                Some(source_id),
                "input {input:?}"
            );
            // Malformed calls are not coerced into ordinary text or another
            // semantic node merely to produce IR.
            assert_eq!(result.ir.nodes.len(), 0, "input {input:?}");
        }
    }

    #[test]
    fn compile_reports_no_diagnostics_for_valid_input() {
        let (result, _) = compile_source(".foo {bar}\n");
        assert!(result.diagnostics.is_empty());
        assert_eq!(result.ir.nodes.len(), 1);
    }

    #[test]
    fn compile_evaluates_if_true() {
        let (result, _) = compile_source(".if {true}\n    hello\n");
        assert!(result.diagnostics.is_empty());
        assert_eq!(result.ir.nodes.len(), 1);
        let IrNode::Paragraph { content, .. } = &result.ir.nodes[0] else {
            panic!()
        };
        assert_eq!(content.len(), 1);
        let IrInline::Text { content: text, .. } = &content[0] else {
            panic!()
        };
        assert_eq!(text, "hello");
    }

    #[test]
    fn compile_evaluates_if_false() {
        let (result, _) = compile_source(".if {false}\n    dropped\n");
        assert!(result.diagnostics.is_empty());
        assert!(result.ir.nodes.is_empty());
    }

    #[test]
    fn compile_evaluates_ifnot() {
        let (result, _) = compile_source(".ifnot {no}\n    kept\n");
        assert!(result.diagnostics.is_empty());
        assert_eq!(result.ir.nodes.len(), 1);
    }

    #[test]
    fn compile_evaluates_nested_if() {
        let (result, _) =
            compile_source(".if {yes}\n    .if {no}\n        inner-dropped\n    inner-kept\n");
        assert!(result.diagnostics.is_empty());
        assert_eq!(result.ir.nodes.len(), 1);
        let IrNode::Paragraph { content, .. } = &result.ir.nodes[0] else {
            panic!()
        };
        let IrInline::Text { content: text, .. } = &content[0] else {
            panic!()
        };
        assert_eq!(text, "inner-kept");
    }

    #[test]
    fn compile_reports_e3001_for_unresolvable_condition() {
        let (result, source_id) = compile_source(".if {maybe}\n    body\n");
        assert_eq!(result.diagnostics.len(), 1);
        let diag = &result.diagnostics[0];
        assert_eq!(diag.code, "E3001");
        assert!(matches!(diag.severity, Severity::Error));
        assert_eq!(diag.primary.as_ref().map(|s| s.source_id), Some(source_id));
        // If condition unknown -> false -> body dropped
        assert!(result.ir.nodes.is_empty());
    }

    #[test]
    fn compile_evaluates_named_condition_true() {
        let (result, _) = compile_source(".if condition:{true}\n    kept\n");
        assert!(result.diagnostics.is_empty());
        assert_eq!(result.ir.nodes.len(), 1);
    }

    #[test]
    fn compile_evaluates_named_condition_false() {
        let (result, _) = compile_source(".if condition:{false}\n    dropped\n");
        assert!(result.diagnostics.is_empty());
        assert!(result.ir.nodes.is_empty());
    }

    #[test]
    fn compile_evaluates_named_condition_yes_no() {
        let (result, _) = compile_source(".if condition:{yes}\n    kept\n");
        assert!(result.diagnostics.is_empty());
        assert_eq!(result.ir.nodes.len(), 1);

        let (result, _) = compile_source(".ifnot condition:{no}\n    kept\n");
        assert!(result.diagnostics.is_empty());
        assert_eq!(result.ir.nodes.len(), 1);
    }

    #[test]
    fn compile_evaluates_named_body() {
        let (result, _) = compile_source(".if {true} body:{shown}\n");
        assert!(result.diagnostics.is_empty());
        assert_eq!(result.ir.nodes.len(), 1);
        let IrNode::Paragraph { content, .. } = &result.ir.nodes[0] else {
            panic!()
        };
        let IrInline::Text { content: text, .. } = &content[0] else {
            panic!()
        };
        assert_eq!(text, "shown");
    }

    #[test]
    fn compile_evaluates_named_condition_and_body() {
        let (result, _) = compile_source(".if condition:{true} body:{shown}\n");
        assert!(result.diagnostics.is_empty());
        assert_eq!(result.ir.nodes.len(), 1);
        let IrNode::Paragraph { content, .. } = &result.ir.nodes[0] else {
            panic!()
        };
        let IrInline::Text { content: text, .. } = &content[0] else {
            panic!()
        };
        assert_eq!(text, "shown");
    }

    #[test]
    fn compile_inline_named_condition() {
        let (result, _) = compile_source("before .if condition:{true} body:{inline} after\n");
        assert!(result.diagnostics.is_empty());
        let IrNode::Paragraph { content, .. } = &result.ir.nodes[0] else {
            panic!()
        };
        let rendered: String = content
            .iter()
            .map(|i| match i {
                IrInline::Text { content, .. } => content.clone(),
                _ => String::new(),
            })
            .collect();
        assert!(rendered.contains("inline"));
    }

    #[test]
    fn compile_variable_declaration_and_reference() {
        let (result, _) = compile_source(".var {name} {Scribium}\nHello .name\n");
        assert!(result.diagnostics.is_empty());
        assert_eq!(result.ir.nodes.len(), 1);
        let IrNode::Paragraph { content, .. } = &result.ir.nodes[0] else {
            panic!()
        };
        let IrInline::Text { content: text, .. } = &content[1] else {
            panic!()
        };
        assert_eq!(text, "Scribium");
    }

    #[test]
    fn compile_variable_boolean_in_conditional() {
        let (result, _) = compile_source(".var {enabled} {yes}\n.if {.enabled}\n    visible\n");
        assert!(result.diagnostics.is_empty());
        assert_eq!(result.ir.nodes.len(), 1);
        let IrNode::Paragraph { content, .. } = &result.ir.nodes[0] else {
            panic!()
        };
        let IrInline::Text { content: text, .. } = &content[0] else {
            panic!()
        };
        assert_eq!(text, "visible");
    }

    #[test]
    fn compile_variable_false_conditional() {
        let (result, _) = compile_source(".var {enabled} {no}\n.if {.enabled}\n    hidden\n");
        assert!(result.diagnostics.is_empty());
        assert!(result.ir.nodes.is_empty());
    }

    #[test]
    fn compile_variable_ifnot() {
        let (result, _) = compile_source(".var {enabled} {no}\n.ifnot {.enabled}\n    visible\n");
        assert!(result.diagnostics.is_empty());
        assert_eq!(result.ir.nodes.len(), 1);
    }

    #[test]
    fn compile_variable_explicit_reassignment() {
        let (result, _) = compile_source(".var {name} {A}\n.var {name} {B}\n.name\n");
        assert!(result.diagnostics.is_empty());
        assert_eq!(result.ir.nodes.len(), 1);
        let IrNode::Paragraph { content, .. } = &result.ir.nodes[0] else {
            panic!()
        };
        let IrInline::Text { content: text, .. } = &content[0] else {
            panic!()
        };
        assert_eq!(text, "B");
    }

    #[test]
    fn compile_variable_name_reassignment() {
        let (result, _) = compile_source(".var {name} {A}\n.name\n.name {B}\n.name\n");
        assert!(result.diagnostics.is_empty());
        assert_eq!(result.ir.nodes.len(), 2);
        let IrNode::Paragraph { content, .. } = &result.ir.nodes[0] else {
            panic!()
        };
        let IrInline::Text { content: text, .. } = &content[0] else {
            panic!()
        };
        assert_eq!(text, "A");
        let IrNode::Paragraph { content, .. } = &result.ir.nodes[1] else {
            panic!()
        };
        let IrInline::Text { content: text, .. } = &content[0] else {
            panic!()
        };
        assert_eq!(text, "B");
    }

    #[test]
    fn compile_variable_inline_use() {
        let (result, _) = compile_source(".var {name} {world}\nHello **.name**\n");
        assert!(result.diagnostics.is_empty());
        assert_eq!(result.ir.nodes.len(), 1);
        let IrNode::Paragraph { content, .. } = &result.ir.nodes[0] else {
            panic!()
        };
        let IrInline::Strong { content, .. } = &content[1] else {
            panic!()
        };
        let IrInline::Text { content: text, .. } = &content[0] else {
            panic!()
        };
        assert_eq!(text, "world");
    }

    #[test]
    fn compile_variable_block_variable() {
        let (result, _) = compile_source(".var {section}\n    # Title\n    body\n.section\n");
        assert!(result.diagnostics.is_empty());
        assert_eq!(result.ir.nodes.len(), 2);
        let IrNode::Heading { content, .. } = &result.ir.nodes[0] else {
            panic!()
        };
        let IrInline::Text { content: text, .. } = &content[0] else {
            panic!()
        };
        assert_eq!(text, "Title");
        let IrNode::Paragraph { content, .. } = &result.ir.nodes[1] else {
            panic!()
        };
        let IrInline::Text { content: text, .. } = &content[0] else {
            panic!()
        };
        assert_eq!(text, "body");
    }

    #[test]
    fn compile_variable_conditional_declaration() {
        let (result, _) = compile_source(".if {false}\n    .var {x} {hidden}\n.x\n");
        assert!(result.diagnostics.is_empty());
        // x not declared, preserved as function call
        assert_eq!(result.ir.nodes.len(), 1);
        let IrNode::FunctionCall { name, .. } = &result.ir.nodes[0] else {
            panic!()
        };
        assert_eq!(name, "x");
    }

    #[test]
    fn compile_variable_unknown_preserved() {
        let (result, _) = compile_source(".unknown\n");
        assert!(result.diagnostics.is_empty());
        assert_eq!(result.ir.nodes.len(), 1);
        let IrNode::FunctionCall { name, .. } = &result.ir.nodes[0] else {
            panic!()
        };
        assert_eq!(name, "unknown");
    }

    #[test]
    fn compile_variable_malformed_reports_e3002() {
        let (result, source_id) = compile_source(".var\n");
        assert_eq!(result.diagnostics.len(), 1);
        let diag = &result.diagnostics[0];
        assert_eq!(diag.code, "E3002");
        assert!(matches!(diag.severity, Severity::Error));
        assert_eq!(diag.primary.as_ref().map(|s| s.source_id), Some(source_id));
    }

    #[test]
    fn compile_variable_nested_in_block() {
        let (result, _) =
            compile_source(".var {section}\n    .if {true}\n        nested\n.section\n");
        assert!(result.diagnostics.is_empty());
        assert_eq!(result.ir.nodes.len(), 1);
        let IrNode::Paragraph { content, .. } = &result.ir.nodes[0] else {
            panic!()
        };
        let IrInline::Text { content: text, .. } = &content[0] else {
            panic!()
        };
        assert_eq!(text, "nested");
    }

    #[test]
    fn compile_variable_immutable_and_deterministic() {
        let source = ".var {name} {A}\n.name\n";
        let project1 = VirtualProjectBuilder::new()
            .entry("main.qd")
            .expect("valid path")
            .add_source("main.qd", source)
            .expect("valid path")
            .build()
            .unwrap();
        let project2 = VirtualProjectBuilder::new()
            .entry("main.qd")
            .expect("valid path")
            .add_source("main.qd", source)
            .expect("valid path")
            .build()
            .unwrap();
        let result1 = super::compile(&project1, &CompileOptions::default());
        let result2 = super::compile(&project2, &CompileOptions::default());
        assert_eq!(result1.ir, result2.ir);
    }

    #[test]
    fn compile_variable_rich_content_block_reference() {
        // Rushdown exposes no original-source inline-fragment parser for this
        // content span. Preserve the source and report the unsupported gap;
        // do not synthesize a Markdown document or claim Strong semantics.
        let (result, _) = compile_source(".var {x} {**hello**}\n.x\n");
        assert!(result.diagnostics.iter().any(|diag| diag.code == "E3010"));
        assert_eq!(result.ir.nodes.len(), 1);
        let IrNode::Paragraph { content, .. } = &result.ir.nodes[0] else {
            panic!("expected paragraph, got {:?}", result.ir.nodes[0])
        };
        assert!(content
            .iter()
            .all(|inline| !matches!(inline, IrInline::Strong { .. })));
    }

    #[test]
    fn compile_variable_rich_content_inline_reference() {
        // The same original-source-only limitation applies to inline variable
        // expansion. The unsupported diagnostic prevents silent data loss.
        let (result, _) = compile_source(".var {x} {**world**}\nHello .x\n");
        assert!(result.diagnostics.iter().any(|diag| diag.code == "E3010"));
        assert_eq!(result.ir.nodes.len(), 1);
        let IrNode::Paragraph { content, .. } = &result.ir.nodes[0] else {
            panic!()
        };
        assert!(content
            .iter()
            .all(|inline| !matches!(inline, IrInline::Strong { .. })));
    }

    #[test]
    fn compile_variable_invalid_name_reports_e3002() {
        // .var {"bad name"} {hello} should report E3002
        let (result, source_id) = compile_source(r#".var {"bad name"} {hello}"#);
        assert_eq!(result.diagnostics.len(), 1);
        let diag = &result.diagnostics[0];
        assert_eq!(diag.code, "E3002");
        assert!(matches!(diag.severity, Severity::Error));
        assert!(diag.message.contains("Invalid variable name"));
        assert_eq!(diag.primary.as_ref().map(|s| s.source_id), Some(source_id));
    }

    #[test]
    fn compile_variable_reference_with_body_preserved_as_call() {
        // .var {foo} {value} / .foo { body } should preserve the call with body
        let (result, _) = compile_source(".var {foo} {value}\n.foo\n    body\n");
        assert!(
            result.diagnostics.is_empty(),
            "diagnostics: {:?}",
            result.diagnostics
        );
        // Should be preserved as function call, not variable reference
        assert_eq!(result.ir.nodes.len(), 1);
        let IrNode::FunctionCall {
            name,
            body: call_body,
            ..
        } = &result.ir.nodes[0]
        else {
            panic!("expected function call, got {:?}", result.ir.nodes[0])
        };
        assert_eq!(name, "foo");
        assert!(call_body.is_some());
        let body_nodes = call_body.as_ref().unwrap();
        assert_eq!(body_nodes.len(), 1);
        let IrNode::Paragraph { content, .. } = &body_nodes[0] else {
            panic!()
        };
        let IrInline::Text { content: text, .. } = &content[0] else {
            panic!()
        };
        assert_eq!(text, "body");
    }
}
