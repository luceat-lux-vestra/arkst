/// `scribium-core` — Scribium's foundational library.
///
/// Responsibilities:
/// - Source abstraction and span management
/// - Composition of the Markdown/Quarkdown frontend
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

    fn output_text(result: &crate::CompileResult) -> String {
        result
            .ir
            .nodes
            .iter()
            .filter_map(|node| match node {
                IrNode::Paragraph { content, .. } => Some(
                    content
                        .iter()
                        .filter_map(|inline| match inline {
                            IrInline::Text { content, .. } => Some(content.as_str()),
                            _ => None,
                        })
                        .collect::<String>(),
                ),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn inline_text(content: &[IrInline]) -> String {
        content
            .iter()
            .map(|inline| match inline {
                IrInline::Text { content, .. } => content.clone(),
                IrInline::Strong { content, .. }
                | IrInline::Emphasis { content, .. }
                | IrInline::Strikethrough { content, .. } => inline_text(content),
                other => panic!("unexpected inline {other:?}"),
            })
            .collect()
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
    fn compile_evaluates_block_and_inline_chain_value_flow() {
        let source = ".sum {10} {5}::multiply {2}\n\nprefix .uppercase {hello}::lowercase suffix\n\n.uppercase {hello}::uppercase::lowercase\n";
        let (result, source_id) = compile_source(source);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);

        let IrNode::Paragraph { content, .. } = &result.ir.nodes[0] else {
            panic!("expected block-chain paragraph")
        };
        assert!(matches!(
            content.as_slice(),
            [IrInline::Text { content, .. }] if content == "30"
        ));
        let first_span = match &result.ir.nodes[0] {
            IrNode::Paragraph { span, .. } => *span,
            _ => panic!("expected paragraph span"),
        };
        assert_eq!(
            first_span,
            crate::source::SourceSpan::new(source_id, 0, source.find('\n').unwrap())
        );

        let IrNode::Paragraph { content, .. } = &result.ir.nodes[1] else {
            panic!("expected inline-chain paragraph")
        };
        assert!(content.iter().any(|inline| matches!(
            inline,
            IrInline::Text { content, .. } if content == "hello"
        )));

        let IrNode::Paragraph { content, .. } = &result.ir.nodes[2] else {
            panic!("expected three-chain paragraph")
        };
        assert!(matches!(
            content.as_slice(),
            [IrInline::Text { content, .. }] if content == "hello"
        ));
    }

    #[test]
    fn compile_evaluates_chain_inside_a_content_argument() {
        let source = ".var {value} {.uppercase {hello}::lowercase}\n.value\n";
        let (result, _) = compile_source(source);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        let IrNode::Paragraph { content, .. } = &result.ir.nodes[0] else {
            panic!("expected content-chain result")
        };
        assert!(matches!(
            content.as_slice(),
            [IrInline::Text { content, .. }] if content == "hello"
        ));
    }

    #[test]
    fn compile_chain_and_nested_call_are_semantically_equivalent() {
        for (chain_source, nested_source, expected) in [
            (
                ".sum {10} {5}::multiply {2}\n",
                ".multiply {.sum {10} {5}} {2}\n",
                "30",
            ),
            (
                ".uppercase {hello}::lowercase\n",
                ".lowercase {.uppercase {hello}}\n",
                "hello",
            ),
        ] {
            let (chain, _) = compile_source(chain_source);
            let (nested, _) = compile_source(nested_source);
            assert!(chain.diagnostics.is_empty(), "{chain:?}");
            assert!(nested.diagnostics.is_empty(), "{nested:?}");
            assert_eq!(output_text(&chain), expected);
            assert_eq!(output_text(&nested), expected);
        }
    }

    #[test]
    fn compile_user_functions_support_zero_and_required_parameters() {
        let source = ".function {hello}\n    Hello\n\n.hello\n\n.function {greet}\n    to from:\n    .to from .from\n\n.greet {world} {John}\n.greet {world} from:{John}\n";
        let (result, _) = compile_source(source);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(
            output_text(&result),
            "Hello\nworld from John\nworld from John"
        );
    }

    #[test]
    fn compile_let_supports_explicit_and_implicit_block_lambdas() {
        let source = ".let {Quarkdown}\n    name:\n    .uppercase {.name}\n\n.let {Quarkdown}\n    .uppercase {.1}\n\n.let {true}\n    condition:\n    .if {.condition}\n        yes\n";
        let (result, _) = compile_source(source);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(output_text(&result), "QUARKDOWN\nQUARKDOWN\nyes");
    }

    #[test]
    fn compile_let_preserves_content_results_and_parent_lookup() {
        let source = ".var {name} {outer}\n.function {decorate}\n    value:\n    .uppercase {.value}\n\n.let {inner}\n    name:\n    .name\n\n.name\n\n.let {hello}\n    value:\n    .decorate {.value}\n\n.let {Quarkdown}\n    name:\n    **Hello .name**\n";
        let (result, _) = compile_source(source);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(output_text(&result), "inner\nouter\nHELLO\n");
        let Some(IrNode::Paragraph { content, .. }) = result.ir.nodes.last() else {
            panic!("expected structured let result")
        };
        assert_eq!(inline_text(content), "Hello Quarkdown");
        assert!(matches!(content.as_slice(), [IrInline::Strong { .. }]));
    }

    #[test]
    fn compile_let_nested_scopes_use_nearest_implicit_argument() {
        let source = ".let {outer}\n    .let {.1}\n        .1\n\n.let {outer}\n    .let {.1}\n        value:\n        .value\n";
        let (result, _) = compile_source(source);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(output_text(&result), "outer\nouter");
    }

    #[test]
    fn compile_let_isolates_local_variables_and_functions() {
        let source = ".var {x} {outer}\n.let {inner}\n    value:\n    .var {x} {.value}\n    .x\n\n.x\n\n.let {hello}\n    value:\n    .function {local}\n        body:\n        .body\n\n.local\n";
        let (result, _) = compile_source(source);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(output_text(&result), "inner\nouter");
        let Some(IrNode::FunctionCall { name, .. }) = result.ir.nodes.last() else {
            panic!("expected local function reference to remain outside the let scope")
        };
        assert_eq!(name, "local");
    }

    #[test]
    fn compile_foreach_closed_range_is_inclusive_and_preserves_numbers() {
        for source in [
            ".foreach {2..4}\n    number:\n    .number\n",
            ".foreach {2..4}\n    .1\n",
        ] {
            let (result, _) = compile_source(source);
            assert!(result.diagnostics.is_empty(), "{result:?}");
            assert_eq!(output_text(&result), "2\n3\n4");
        }
    }

    #[test]
    fn compile_direct_range_output_reports_one_source_backed_failure() {
        let source = ".var {r} {2..4}\n.r\n";
        let (result, source_id) = compile_source(source);
        let range_start = source.find("2..4").expect("range literal");
        assert_eq!(result.diagnostics.len(), 1, "{result:?}");
        assert_eq!(result.diagnostics[0].code, "E3001");
        assert_eq!(
            result.diagnostics[0].primary,
            Some(crate::source::SourceSpan::new(
                source_id,
                range_start,
                range_start + "2..4".len()
            ))
        );
        assert!(result.ir.nodes.is_empty(), "{result:?}");
    }

    #[test]
    fn compile_range_composition_fails_without_fabricating_empty_content() {
        let source = ".let {ignored}\n    .var {r} {2..4}\n    .r\n    tail\n";
        let (result, source_id) = compile_source(source);
        let range_start = source.find("2..4").expect("range literal");
        assert_eq!(result.diagnostics.len(), 1, "{result:?}");
        assert_eq!(result.diagnostics[0].code, "E3001");
        assert_eq!(
            result.diagnostics[0].primary,
            Some(crate::source::SourceSpan::new(
                source_id,
                range_start,
                range_start + "2..4".len()
            ))
        );
        assert!(result.ir.nodes.is_empty(), "{result:?}");
    }

    #[test]
    fn compile_collection_composition_materializes_in_order_without_stringifying() {
        let source = ".let {ignored}\n    .var {c}\n        .foreach {1..2}\n            .1\n    .c\n    tail\n";
        let (result, _) = compile_source(source);
        assert!(result.diagnostics.is_empty(), "{result:?}");
        assert_eq!(output_text(&result), "1\n2\ntail");
    }

    #[test]
    fn compile_unresolved_range_argument_fails_before_typst_lowering() {
        let source = ".foo {2..4}\n";
        let (result, source_id) = compile_source(source);
        let range_start = source.find("2..4").expect("range literal");
        assert_eq!(result.diagnostics.len(), 1, "{result:?}");
        assert_eq!(result.diagnostics[0].code, "E3001");
        assert_eq!(
            result.diagnostics[0].primary,
            Some(crate::source::SourceSpan::new(
                source_id,
                range_start,
                range_start + "2..4".len()
            ))
        );
        assert!(result.ir.nodes.is_empty(), "{result:?}");
    }

    #[test]
    fn compile_foreach_returns_a_typed_collection_that_can_be_stored_and_consumed() {
        let source = ".var {mapped}\n    .foreach {1..3}\n        n:\n        .multiply {.n} by:{2}\n\n.mapped\n";
        let (result, _) = compile_source(source);
        assert!(result.diagnostics.is_empty(), "{result:?}");
        assert_eq!(output_text(&result), "2\n4\n6");
        assert!(matches!(
            result.ir.nodes.as_slice(),
            [
                IrNode::Paragraph { .. },
                IrNode::Paragraph { .. },
                IrNode::Paragraph { .. }
            ]
        ));
    }

    #[test]
    fn compile_foreach_reads_parent_values_and_functions_with_isolated_children() {
        let source = ".var {prefix} {item}\n.function {square}\n    n:\n    .multiply {.n} by:{.n}\n\n.foreach {1..3}\n    n:\n    .prefix .square {.n}\n\n.foreach {1..2}\n    n:\n    .var {local} {.n}\n    .local\n\n.local\n";
        let (result, _) = compile_source(source);
        assert!(result.diagnostics.is_empty(), "{result:?}");
        assert_eq!(output_text(&result), "item 1\nitem 4\nitem 9\n1\n2");
        assert!(result
            .ir
            .nodes
            .iter()
            .any(|node| { matches!(node, IrNode::FunctionCall { name, .. } if name == "local") }));
    }

    #[test]
    fn compile_foreach_adapts_only_list_values_and_preserves_nested_collections() {
        let source = ".var {letters}\n    1. A\n    2. B\n    3. C\n\n.foreach {.letters}\n    .1::lowercase\n\n.var {matrix}\n    - - A\n      - B\n    - - C\n      - D\n\n.foreach {.matrix}\n    .1\n\n- ordinary\n";
        let (result, _) = compile_source(source);
        assert!(result.diagnostics.is_empty(), "{result:?}");
        assert_eq!(output_text(&result), "a\nb\nc\nA\nB\nC\nD");
        assert!(result
            .ir
            .nodes
            .iter()
            .any(|node| { matches!(node, IrNode::UnorderedList { .. }) }));
    }

    #[test]
    fn compile_foreach_scopes_implicit_parameters_at_the_nearest_boundary() {
        let implicit = ".let {outer}\n    .foreach {1..2}\n        .1\n";
        let (result, _) = compile_source(implicit);
        assert!(result.diagnostics.is_empty(), "{result:?}");
        assert_eq!(output_text(&result), "1\n2");

        let explicit = ".let {outer}\n    .foreach {1..2}\n        n:\n        .1\n";
        let (result, _) = compile_source(explicit);
        assert_eq!(result.diagnostics.len(), 1, "{result:?}");
        assert_eq!(result.diagnostics[0].code, "E3003");
    }

    #[test]
    fn compile_repeat_is_one_based_and_uses_the_shared_collection_result() {
        for source in [".repeat {3}\n    n:\n    .n\n", ".repeat {3}\n    .1\n"] {
            let (result, _) = compile_source(source);
            assert!(result.diagnostics.is_empty(), "{result:?}");
            assert_eq!(output_text(&result), "1\n2\n3");
        }
    }

    #[test]
    fn compile_repeat_zero_and_descending_ranges_are_empty_per_upstream_evidence() {
        for source in [".repeat {0}\n    .1\n", ".foreach {4..2}\n    .1\n"] {
            let (result, _) = compile_source(source);
            assert!(result.diagnostics.is_empty(), "{result:?}");
            assert!(result.ir.nodes.is_empty());
        }
    }

    #[test]
    fn compile_iteration_rejects_open_ranges_invalid_counts_and_destructuring() {
        for source in [
            ".foreach {2..}\n    .1\n",
            ".foreach {..4}\n    .1\n",
            ".foreach {..}\n    .1\n",
            ".repeat {1.5}\n    .1\n",
            ".repeat {-1}\n    .1\n",
            ".foreach {1..2}\n    first second:\n    .first\n",
        ] {
            let (result, _) = compile_source(source);
            assert_eq!(result.diagnostics.len(), 1, "{source:?}: {result:?}");
            assert_eq!(result.diagnostics[0].code, "E3001", "{source:?}");
        }
    }

    #[test]
    fn compile_iteration_body_no_value_and_failure_are_single_diagnostics() {
        for source in [
            ".foreach {1..3}\n    n:\n    .var {local} {.n}\n",
            ".foreach {1..3}\n    n:\n    .multiply {.n} by:{true}\n",
        ] {
            let (result, _) = compile_source(source);
            assert_eq!(result.diagnostics.len(), 1, "{source:?}: {result:?}");
            assert_eq!(result.diagnostics[0].code, "E3001", "{source:?}");
            assert!(result.ir.nodes.is_empty(), "{source:?}: {result:?}");
        }
    }

    #[test]
    fn compile_iteration_fixture_qd_exercises_the_document_boundary() {
        let source = include_str!("../../../fixtures/markdown/quarkdown_iteration.qd");
        let (result, _) = compile_source(source);
        assert!(result.diagnostics.is_empty(), "{result:?}");
        assert_eq!(output_text(&result), "2\n3\n4\n1\n2\n3\na\nb\nc");
    }

    #[test]
    fn compile_let_reports_arity_and_implicit_parameter_spans() {
        let missing_value = ".let\n    value:\n    .value\n";
        let (result, source_id) = compile_source(missing_value);
        assert_eq!(result.diagnostics.len(), 1, "{result:?}");
        assert_eq!(result.diagnostics[0].code, "E3003");
        assert_eq!(
            result.diagnostics[0].primary,
            Some(crate::source::SourceSpan::new(
                source_id,
                0,
                missing_value.trim_end().len()
            ))
        );

        let missing_implicit = ".let {1}\n    .2\n";
        let (result, source_id) = compile_source(missing_implicit);
        assert_eq!(result.diagnostics.len(), 1, "{result:?}");
        let reference_start = missing_implicit.find(".2").expect("implicit reference");
        assert_eq!(
            result.diagnostics[0].primary,
            Some(crate::source::SourceSpan::new(
                source_id,
                reference_start,
                reference_start + 2
            ))
        );

        let multiple_parameters = ".let {1}\n    first second:\n    .first\n";
        let (result, source_id) = compile_source(multiple_parameters);
        assert_eq!(result.diagnostics.len(), 1, "{result:?}");
        let first_start = multiple_parameters.find("first").expect("first parameter");
        assert_eq!(
            result.diagnostics[0].primary,
            Some(crate::source::SourceSpan::new(
                source_id,
                first_start,
                first_start + "first".len()
            ))
        );
    }

    #[test]
    fn compile_implicit_lambda_parameters_use_the_shared_callable_path() {
        let source = ".function {identity}\n    .1\n\n.identity {first}\n.identity {second}\n\n.function {pair}\n    .1\n    .2\n\n.pair {one} {two}\n\n.identity {2}::multiply {3}\n";
        let (result, _) = compile_source(source);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(output_text(&result), "first\nsecond\none\ntwo\n6");
    }

    #[test]
    fn compile_implicit_parameters_preserve_typed_values() {
        let numeric = ".function {triple}\n    .multiply {.1} {3}\n\n.triple {2}\n";
        let (result, _) = compile_source(numeric);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(output_text(&result), "6");

        let boolean = ".function {truth}\n    .if {.1}\n        yes\n\n.truth {true}\n";
        let (result, _) = compile_source(boolean);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(output_text(&result), "yes");

        let none = ".function {optional}\n    value?:\n    .value\n\n.function {identity}\n    .1\n\n.function {is-none}\n    .isnone {.1}\n\n.is-none {.identity {.optional}}\n.is-none {\"None\"}\n";
        let (result, _) = compile_source(none);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(output_text(&result), "true\nfalse");
    }

    #[test]
    fn compile_implicit_parameter_content_keeps_markdown_structure() {
        let source = ".function {identity}\n    .1\n\n.identity\n    **rich**\n";
        let (result, _) = compile_source(source);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        let IrNode::Paragraph { content, .. } = &result.ir.nodes[0] else {
            panic!("expected rich implicit parameter result")
        };
        assert!(matches!(content.as_slice(), [IrInline::Strong { .. }]));
        assert_eq!(inline_text(content), "rich");
    }

    #[test]
    fn compile_implicit_lambda_scopes_are_nested_and_reusable() {
        let source = ".function {inner}\n    .1\n\n.function {outer}\n    .inner {inner}\n    .1\n\n.outer {outer}\n.outer {again}\n";
        let (result, _) = compile_source(source);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(output_text(&result), "inner\nouter\ninner\nagain");
    }

    #[test]
    fn compile_implicit_parameter_missing_and_zero_argument_are_diagnostics() {
        for source in [
            ".function {missing}\n    .2\n\n.missing {one}\n",
            ".function {zero}\n    .1\n\n.zero\n",
        ] {
            let (result, source_id) = compile_source(source);
            assert_eq!(result.diagnostics.len(), 1, "{source:?}: {result:?}");
            let diagnostic = &result.diagnostics[0];
            assert_eq!(diagnostic.code, "E3003");
            assert_eq!(
                diagnostic.primary.map(|span| span.source_id),
                Some(source_id)
            );
            assert!(diagnostic.message.contains("Implicit lambda parameter"));
            assert!(result.ir.nodes.is_empty());
        }
    }

    #[test]
    fn compile_implicit_parameter_diagnostic_preserves_utf8_and_crlf_span() {
        let source = ".function {missing}\r\n    .2\r\n\r\n.missing {세계}\r\n";
        let (result, source_id) = compile_source(source);
        assert_eq!(result.diagnostics.len(), 1, "{result:?}");
        let start = source.find(".2").expect("implicit parameter span");
        assert_eq!(
            result.diagnostics[0].primary,
            Some(crate::source::SourceSpan::new(source_id, start, start + 2))
        );
    }

    #[test]
    fn compile_implicit_parameters_keep_container_and_md_boundaries() {
        let source = ".function {identity}\n    .1\n\n- .identity {list}\n\n> .identity {quote}\n";
        let (result, _) = compile_source(source);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert!(matches!(
            result.ir.nodes.first(),
            Some(IrNode::UnorderedList { items, .. }) if items.len() == 1
        ));
        assert!(matches!(
            result.ir.nodes.get(1),
            Some(IrNode::Blockquote { content, .. }) if !content.is_empty()
        ));

        let md_source = ".function {identity}\n    .1\n\n.identity {value}\n";
        let project = VirtualProjectBuilder::new()
            .entry("main.md")
            .expect("valid path")
            .add_source("main.md", md_source)
            .expect("valid source")
            .build()
            .expect("valid project");
        let result = super::compile(&project, &CompileOptions::default());
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert!(result
            .ir
            .nodes
            .iter()
            .all(|node| !matches!(node, IrNode::FunctionDeclaration { .. })));
        assert!(output_text(&result).contains(".function"));
        assert!(output_text(&result).contains(".identity"));
    }

    #[test]
    fn compile_user_functions_keep_scalar_values_for_nested_and_chain_calls() {
        let source = ".function {area}\n    width height:\n    .multiply {.width} by:{.height}\n\n.sum {.area {4} {2}} {1}\n\n.area {4} {2}::sum {1}\n";
        let (result, _) = compile_source(source);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(output_text(&result), "9\n9");
        assert!(result
            .ir
            .nodes
            .iter()
            .all(|node| { !matches!(node, IrNode::FunctionDeclaration { .. }) }));
    }

    #[test]
    fn compile_user_function_multi_statement_body_preserves_last_semantic_value() {
        let source = ".function {f}\n    .var {x} {2}\n    .sum {.x} {1}\n\n.sum {.f} {1}\n";
        let (result, _) = compile_source(source);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(output_text(&result), "4");

        let source = ".function {f}\n    .function {local}\n        body\n    .sum {2} {1}\n\n.sum {.f} {1}\n";
        let (result, _) = compile_source(source);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(output_text(&result), "4");
    }

    #[test]
    fn compile_user_function_multi_statement_body_stops_after_first_failure() {
        let source = ".function {bad}\n    .multiply {true} {true}\n    .var {after} {ran}\n\n.sum {.bad} {1}\n.after\n";
        let (result, _) = compile_source(source);
        assert_eq!(result.diagnostics.len(), 1, "{result:?}");
        assert!(result.diagnostics[0]
            .message
            .contains("requires numeric arguments"));
        assert!(!output_text(&result).contains("ran"));
    }

    #[test]
    fn compile_user_function_multi_statement_rich_content_keeps_source_spans() {
        let source = ".function {rich}\n    First **one**\n\n    Second *two*\n\n.rich\n";
        let (result, source_id) = compile_source(source);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        // Rushdown's original inline spans are retained verbatim; in
        // particular, the closing delimiter is not part of these paragraph
        // ranges, so assert against the exact source-backed range.
        let expected = ["First **one", "Second *two"];
        assert_eq!(result.ir.nodes.len(), expected.len());
        for (node, expected) in result.ir.nodes.iter().zip(expected) {
            let IrNode::Paragraph { span, .. } = node else {
                panic!("expected paragraph, got {node:?}")
            };
            assert_eq!(span.source_id, source_id);
            assert_eq!(&source[span.start..span.end], expected);
        }
    }

    #[test]
    fn compile_user_function_rich_and_block_results_keep_markdown_structure() {
        let rich_source = ".function {greet}\n    name:\n    **Hello, .name!**\n\n.greet {world}\n";
        let (rich, _) = compile_source(rich_source);
        assert!(rich.diagnostics.is_empty(), "{:?}", rich.diagnostics);
        let IrNode::Paragraph { content, .. } = &rich.ir.nodes[0] else {
            panic!("expected rich function result")
        };
        assert!(matches!(content.as_slice(), [IrInline::Strong { .. }]));
        assert_eq!(inline_text(content), "Hello, world!");

        let block_source = ".function {wrapper}\n    title content:\n    .content\n\n.wrapper {Title}\n    **Body**\n";
        let (block, _) = compile_source(block_source);
        assert!(block.diagnostics.is_empty(), "{:?}", block.diagnostics);
        let IrNode::Paragraph { content, .. } = &block.ir.nodes[0] else {
            panic!("expected block function result")
        };
        assert!(matches!(content.as_slice(), [IrInline::Strong { .. }]));
        assert_eq!(inline_text(content), "Body");

        let inline_source = ".function {inline_greet}\n    name:\n    **Hello, .name!**\n\nprefix .inline_greet {world} suffix\n";
        let (inline, _) = compile_source(inline_source);
        assert!(inline.diagnostics.is_empty(), "{:?}", inline.diagnostics);
        let IrNode::Paragraph { content, .. } = &inline.ir.nodes[0] else {
            panic!("expected inline function result")
        };
        assert!(content
            .iter()
            .any(|inline| { matches!(inline, IrInline::Strong { .. }) }));

        let unsupported_inline = ".function {heading}\n    # Heading\n\nprefix .heading suffix\n";
        let (unsupported, _) = compile_source(unsupported_inline);
        assert_eq!(unsupported.diagnostics.len(), 1);
        assert_eq!(unsupported.diagnostics[0].code, "E3003");
        assert!(unsupported.diagnostics[0]
            .message
            .contains("Rich block content"));

        let multiple_paragraphs =
            ".function {two}\n    First\n\n    Second\n\nprefix .two suffix\n";
        let (multiple, _) = compile_source(multiple_paragraphs);
        assert_eq!(multiple.diagnostics.len(), 1, "{multiple:?}");
        assert!(!output_text(&multiple).contains("First"));
        assert!(!output_text(&multiple).contains("Second"));
    }

    #[test]
    fn compile_user_functions_use_source_order_and_override_builtins() {
        let redeclaration = ".function {answer}\n    first\n\n.answer\n\n.function {answer}\n    second\n\n.answer\n";
        let (result, _) = compile_source(redeclaration);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(output_text(&result), "first\nsecond");

        let override_source = ".uppercase {Quarkdown}\n\n.function {uppercase}\n    text:\n    .text::lowercase\n\n.uppercase {Quarkdown}\n";
        let (result, _) = compile_source(override_source);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(output_text(&result), "QUARKDOWN\nquarkdown");
    }

    #[test]
    fn compile_user_functions_bind_block_last_and_isolate_child_scope() {
        let source = ".var {outside} {A}\n.var {value} {parent}\n.function {inner}\n    inherited\n\n.function {demo}\n    value:\n    .function {local}\n        local\n    .outside\n    .value\n    .inner\n    .var {local_value} {.value}\n    .local\n\n.demo {B}\n\n.outside\n.value\n.local\n";
        let (result, _) = compile_source(source);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        let output = output_text(&result);
        assert!(output.contains("A"), "{output:?}");
        assert!(output.contains("B"), "{output:?}");
        assert!(output.contains("inherited"), "{output:?}");
        assert!(
            output.ends_with("parent"),
            "shadowed parent changed: {output:?}"
        );
        assert!(result
            .ir
            .nodes
            .iter()
            .any(|node| { matches!(node, IrNode::FunctionCall { name, .. } if name == "local") }));
    }

    #[test]
    fn compile_user_function_no_value_and_failed_nested_calls_keep_original_diagnostic() {
        let no_value = ".function {noop}\n    .var {temporary} {value}\n\n.sum {.noop} {1}\n";
        let (result, _) = compile_source(no_value);
        assert_eq!(result.diagnostics.len(), 1, "{result:?}");
        assert_eq!(result.diagnostics[0].code, "E3001");
        assert!(result.diagnostics[0].message.contains("no value"));

        let declaration_no_value =
            ".function {noop}\n    .function {local}\n        body\n\n.sum {.noop} {1}\n";
        let (result, _) = compile_source(declaration_no_value);
        assert_eq!(result.diagnostics.len(), 1, "{result:?}");
        assert_eq!(result.diagnostics[0].code, "E3001");
        assert!(result.diagnostics[0].message.contains("no value"));

        let failed = ".function {bad}\n    .multiply {true} {true}\n\n.sum {.bad} {1}\n";
        let (result, _) = compile_source(failed);
        assert_eq!(result.diagnostics.len(), 1, "{result:?}");
        assert_eq!(result.diagnostics[0].code, "E3001");
        assert!(result.diagnostics[0]
            .message
            .contains("requires numeric arguments"));
    }

    #[test]
    fn compile_user_function_argument_failures_are_single_and_body_is_not_run() {
        for (source, expected_message) in [
            (
                ".function {needs}\n    first:\n    .multiply {true} {true}\n\n.needs\n",
                "Missing required argument",
            ),
            (
                ".function {needs}\n    first:\n    .multiply {true} {true}\n\n.needs {one} {two}\n",
                "too many positional arguments",
            ),
            (
                ".function {needs}\n    first:\n    .multiply {true} {true}\n\n.needs unknown:{one}\n",
                "Unknown named parameter",
            ),
            (
                ".function {needs}\n    first:\n    .multiply {true} {true}\n\n.needs {one} first:{two}\n",
                "bound more than once",
            ),
        ] {
            let (result, _) = compile_source(source);
            assert_eq!(result.diagnostics.len(), 1, "{source:?}: {result:?}");
            assert_eq!(result.diagnostics[0].code, "E3003");
            assert!(result.diagnostics[0].message.contains(expected_message));
            assert!(!result.diagnostics[0].message.contains("requires numeric arguments"));
        }
    }

    #[test]
    fn compile_user_function_declaration_errors_are_explicit_and_source_backed() {
        for source in [
            ".function {1invalid}\n    body\n",
            ".function {duplicate}\n    first first:\n    body\n",
            ".function {missing-body}\n",
            ".function {named} extra:{value}\n    body\n",
            ".function {named}::sum {1}\n    body\n",
        ] {
            let (result, source_id) = compile_source(source);
            assert_eq!(result.diagnostics.len(), 1, "{source:?}: {result:?}");
            assert_eq!(result.diagnostics[0].code, "E3003");
            assert_eq!(
                result.diagnostics[0]
                    .primary
                    .as_ref()
                    .map(|span| span.source_id),
                Some(source_id)
            );
        }

        let source = ".function {named}\n    value:\n    body\n\n.named unknown:{value}\n";
        let (result, source_id) = compile_source(source);
        assert_eq!(result.diagnostics.len(), 1, "{result:?}");
        let diagnostic = &result.diagnostics[0];
        assert_eq!(diagnostic.code, "E3003");
        let start = source.find("unknown").expect("named argument name");
        assert_eq!(
            diagnostic.primary,
            Some(crate::source::SourceSpan::new(
                source_id,
                start,
                start + "unknown".len()
            ))
        );
    }

    #[test]
    fn compile_optional_user_parameters_bind_missing_positional_and_named_values() {
        let source = ".function {greet}\n    to from?:\n    Hello, .to from .from!\n\n.greet {world}\n.greet {world} {John}\n.greet {world} from:{Jane}\n\n.function {ordered}\n    first? second:\n    .first::otherwise {missing} .second\n\n.ordered second:{provided}\n";
        let (result, _) = compile_source(source);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(
            output_text(&result),
            "Hello, world from None!\nHello, world from John!\nHello, world from Jane!\nmissing provided"
        );
    }

    #[test]
    fn compile_optional_parameters_support_otherwise_and_preserve_value_types() {
        let source = ".function {greet}\n    to from?:\n    Hello, .to from .from::otherwise {unnamed}!\n\n.greet {world}\n.greet {world} {John}\n\n.function {f}\n    x?:\n    .x::otherwise {42}\n\n.sum {.f} {1}\n\n.function {g}\n    value?:\n    .value\n\n.uppercase {.g::otherwise {fallback}}\n";
        let (result, _) = compile_source(source);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(
            output_text(&result),
            "Hello, world from unnamed!\nHello, world from John!\n43\nFALLBACK"
        );
    }

    #[test]
    fn compile_optional_none_is_distinct_from_no_value() {
        let none_source = ".function {f}\n    x?:\n    .x\n\n.sum {.f} {1}\n";
        let (none_result, _) = compile_source(none_source);
        assert_eq!(none_result.diagnostics.len(), 1, "{none_result:?}");
        assert_eq!(none_result.diagnostics[0].code, "E3001");
        assert!(none_result.diagnostics[0]
            .message
            .contains("requires numeric arguments"));
        assert!(!none_result.diagnostics[0].message.contains("no value"));

        let no_value_source = ".function {f}\n    .var {local} {1}\n\n.sum {.f} {1}\n";
        let (no_value_result, _) = compile_source(no_value_source);
        assert_eq!(no_value_result.diagnostics.len(), 1, "{no_value_result:?}");
        assert_eq!(no_value_result.diagnostics[0].code, "E3001");
        assert!(no_value_result.diagnostics[0].message.contains("no value"));
    }

    #[test]
    fn compile_required_parameter_stays_required_after_optional_support() {
        let source = ".function {f}\n    required optional?:\n    .required\n\n.f\n";
        let (result, source_id) = compile_source(source);
        assert_eq!(result.diagnostics.len(), 1, "{result:?}");
        let diagnostic = &result.diagnostics[0];
        assert_eq!(diagnostic.code, "E3003");
        assert!(diagnostic
            .message
            .contains("Missing required argument `required`"));
        let parameter_start = source.find("required").expect("required parameter");
        assert_eq!(
            diagnostic.primary,
            Some(crate::source::SourceSpan::new(
                source_id,
                parameter_start,
                parameter_start + "required".len()
            ))
        );
    }

    #[test]
    fn compile_optional_final_parameter_accepts_missing_or_block_content_and_keeps_collision() {
        let source = ".function {wrap}\n    title content?:\n    .content::otherwise {empty}\n\n.wrap {Title}\n.wrap {Title}\n    Body\n";
        let (result, _) = compile_source(source);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(output_text(&result), "empty\nBody");

        let collision =
            ".function {wrap}\n    content?:\n    .content\n\n.wrap {explicit}\n    body\n";
        let (result, _) = compile_source(collision);
        assert_eq!(result.diagnostics.len(), 1, "{result:?}");
        assert_eq!(result.diagnostics[0].code, "E3003");
        assert!(result.diagnostics[0].message.contains("collides"));
    }

    #[test]
    fn compile_optional_none_can_be_stored_locally_without_parent_scope_leak() {
        let source = ".function {f}\n    value?:\n    .var {local} {.value}\n    .local::otherwise {fallback}\n\n.f\n.local\n";
        let (result, _) = compile_source(source);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(output_text(&result), "fallback");
        assert!(result
            .ir
            .nodes
            .iter()
            .any(|node| { matches!(node, IrNode::FunctionCall { name, .. } if name == "local") }));
    }

    #[test]
    fn compile_optional_none_direct_output_materializes_as_text() {
        let source = ".function {f}\n    value?:\n    .value\n\n.f\n";
        let (result, _) = compile_source(source);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(output_text(&result), "None");
    }

    #[test]
    fn compile_isnone_returns_a_semantic_boolean_for_optional_values() {
        let source = ".function {f}\n    value?:\n    .value::isnone\n\n.f\n.f {hello}\n";
        let (result, _) = compile_source(source);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(output_text(&result), "true\nfalse");
    }

    #[test]
    fn optional_parameter_spans_survive_utf8_and_crlf_frontend_to_ir_conversion() {
        let source = ".function {greet}\r\n    from? name:\r\n    안녕, .from .name!\r\n\r\n.greet {세계} {친구}\r\n";
        let project = VirtualProjectBuilder::new()
            .entry("main.qd")
            .expect("valid path")
            .add_source("main.qd", source)
            .expect("valid path")
            .build()
            .expect("valid project");
        let source_id = project
            .sources()
            .get_id(project.entry())
            .expect("source id");
        let parsed = scribium_markdown::parse_with_diagnostics(source);
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        let (ir, diagnostics) = crate::ast_to_ir::ast_to_ir_with_diagnostics(
            &parsed.document,
            source_id,
            project.metadata(),
        );
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let IrNode::FunctionDeclaration { parameters, .. } = &ir.nodes[0] else {
            panic!("expected function declaration")
        };
        assert!(parameters[0].optional);
        assert_eq!(
            &source[parameters[0].span.start..parameters[0].span.end],
            "from?"
        );
        assert_eq!(
            &source[parameters[1].span.start..parameters[1].span.end],
            "name"
        );

        let result = super::compile(&project, &CompileOptions::default());
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(output_text(&result), "안녕, 세계 친구!");
    }

    #[test]
    fn compile_markdown_mode_does_not_enable_quarkdown_functions() {
        let project = VirtualProjectBuilder::new()
            .entry("main.md")
            .expect("valid path")
            .add_source(
                "main.md",
                ".function {hello}\n    value?:\n    Hello .value\n\n.hello\n",
            )
            .expect("valid path")
            .build()
            .unwrap();
        let result = super::compile(&project, &CompileOptions::default());
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert!(result
            .ir
            .nodes
            .iter()
            .all(|node| { !matches!(node, IrNode::FunctionDeclaration { .. }) }));
    }

    #[test]
    fn compile_variable_values_keep_types_across_chain_and_nested_forms() {
        for (chain_source, nested_source, expected) in [
            (
                ".var {myvar} {hello!}\n.myvar::uppercase\n",
                ".var {myvar} {hello!}\n.uppercase {.myvar}\n",
                "HELLO!",
            ),
            (
                ".var {myvar} {true}\n.myvar::uppercase\n",
                ".var {myvar} {true}\n.uppercase {.myvar}\n",
                "TRUE",
            ),
        ] {
            let (chain, _) = compile_source(chain_source);
            let (nested, _) = compile_source(nested_source);
            assert!(chain.diagnostics.is_empty(), "{chain:?}");
            assert!(nested.diagnostics.is_empty(), "{nested:?}");
            assert_eq!(output_text(&chain), expected);
            assert_eq!(output_text(&nested), expected);
        }
    }

    #[test]
    fn compile_numeric_variable_reassignment_preserves_numeric_value_context() {
        let source = ".var {mynumber} {5}\n.mynumber {.mynumber::sum {1}}\n.mynumber::sum {1}\n";
        let (result, _) = compile_source(source);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(output_text(&result), "7");
    }

    #[test]
    fn compile_final_chain_no_value_is_legal_but_non_final_is_not() {
        let (final_result, _) = compile_source(".var {x} {0}\n.sum {1} {2}::x\n.x\n");
        assert!(final_result.diagnostics.is_empty(), "{final_result:?}");
        assert_eq!(output_text(&final_result), "3");

        let (non_final_result, _) = compile_source(".var {x} {0}\n.sum {1} {2}::x::sum {1}\n.x\n");
        assert_eq!(non_final_result.diagnostics.len(), 1);
        assert_eq!(non_final_result.diagnostics[0].code, "E3001");
        assert_eq!(output_text(&non_final_result), "3");
    }

    #[test]
    fn compile_nested_no_value_matches_chain_failure_classification() {
        let (nested_result, _) = compile_source(".var {x} {0}\n.multiply {.x {3}} {2}\n.x\n");
        assert_eq!(nested_result.diagnostics.len(), 1, "{nested_result:?}");
        assert_eq!(nested_result.diagnostics[0].code, "E3001");
        assert_eq!(output_text(&nested_result), "3");

        let (failed_child, _) = compile_source(".multiply {.sum {true}} {2}\n");
        assert_eq!(failed_child.diagnostics.len(), 1, "{failed_child:?}");
        assert_eq!(failed_child.diagnostics[0].code, "E3001");
        assert!(failed_child.diagnostics[0]
            .message
            .contains("requires numeric arguments"));
    }

    #[test]
    fn compile_chain_and_ordinary_conditional_are_equally_lazy() {
        let chain_source =
            ".var {flag} {false}\n.var {x} {before}\n.flag::if\n    .x {after}\n.x\n";
        let ordinary_source =
            ".var {flag} {false}\n.var {x} {before}\n.if {.flag}\n    .x {after}\n.x\n";
        let (chain, _) = compile_source(chain_source);
        let (ordinary, _) = compile_source(ordinary_source);
        assert!(chain.diagnostics.is_empty(), "{chain:?}");
        assert!(ordinary.diagnostics.is_empty(), "{ordinary:?}");
        assert_eq!(output_text(&chain), "before");
        assert_eq!(output_text(&ordinary), "before");
    }

    #[test]
    fn chain_gate_removal_does_not_remove_other_e8001_diagnostics() {
        let (result, _) = compile_source("![image](image.png)\n");
        assert!(result.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "E8001" && diagnostic.message.contains("image")
        }));
    }

    #[test]
    fn compile_reports_unimplemented_chain_callees_with_specific_spans() {
        for source in [".a::b\n", ".a::b::c\n", ".a {x}::b {y}\n"] {
            let parsed = scribium_markdown::parse_qd(source);
            let scribium_markdown::ast::Block::DirectiveCall { chain, .. } = &parsed.nodes[0]
            else {
                panic!("expected parsed block chain for {source:?}");
            };
            assert!(!chain.is_empty(), "{source:?}");

            let (result, source_id) = compile_source(source);
            assert_eq!(result.diagnostics.len(), 1, "{source:?}");
            let diagnostic = &result.diagnostics[0];
            assert_eq!(diagnostic.code, "E3001");
            assert!(matches!(diagnostic.severity, Severity::Error));
            assert!(diagnostic.message.contains("no semantic implementation"));
            assert_eq!(
                diagnostic.primary,
                Some(crate::source::SourceSpan::new(source_id, 0, 2))
            );
            assert!(result.ir.nodes.is_empty());
        }
    }

    #[test]
    fn compile_reports_chain_failures_in_inline_and_content_paths() {
        let inline_source = "prefix .a {x}::b {y} suffix\n";
        let parsed = scribium_markdown::parse_qd(inline_source);
        let scribium_markdown::ast::Block::Paragraph { content, .. } = &parsed.nodes[0] else {
            panic!("expected inline paragraph");
        };
        assert!(content.iter().any(|inline| matches!(
            inline,
            scribium_markdown::ast::Inline::DirectiveCall { chain, .. }
                if !chain.is_empty()
        )));
        let (result, source_id) = compile_source(inline_source);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].code, "E3001");
        assert!(matches!(result.diagnostics[0].severity, Severity::Error));
        let IrNode::Paragraph { content, .. } = &result.ir.nodes[0] else {
            panic!("expected inline paragraph IR");
        };
        assert!(content
            .iter()
            .all(|inline| !matches!(inline, IrInline::ChainedDirectiveCall { .. })));
        assert_eq!(
            result.diagnostics[0].primary.as_ref().unwrap().source_id,
            source_id
        );

        let content_source = ".outer {.a::b}\n";
        let parsed = scribium_markdown::parse_qd(content_source);
        let scribium_markdown::ast::Block::DirectiveCall {
            positional_args, ..
        } = &parsed.nodes[0]
        else {
            panic!("expected outer call");
        };
        let scribium_markdown::ast::Value::Content(content) = &positional_args[0] else {
            panic!("expected content argument");
        };
        assert!(content.iter().any(|inline| matches!(
            inline,
            scribium_markdown::ast::Inline::DirectiveCall { chain, .. }
                if !chain.is_empty()
        )));

        let (result, source_id) = compile_source(content_source);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].code, "E3001");
        assert_eq!(
            result.diagnostics[0].primary.as_ref().unwrap().source_id,
            source_id
        );
        assert!(result.ir.nodes.is_empty());
    }

    #[test]
    fn compile_qd_uses_the_production_frontend_pipeline() {
        let project = VirtualProjectBuilder::new()
            .entry("main.qd")
            .expect("valid path")
            .add_source("main.qd", "# Hello\n.note {hello}\n")
            .expect("valid path")
            .build()
            .unwrap();

        let result = super::compile(&project, &CompileOptions::default());
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(result.ir.nodes.len(), 2);
        assert!(
            matches!(result.ir.nodes[1], IrNode::FunctionCall { ref name, .. } if name == "note")
        );
    }

    #[test]
    fn compile_md_uses_markdown_mode_through_the_production_frontend() {
        let project = VirtualProjectBuilder::new()
            .entry("main.md")
            .expect("valid path")
            .add_source("main.md", "# Hello\n\n**world**\n")
            .expect("valid path")
            .build()
            .unwrap();

        let result = super::compile(&project, &CompileOptions::default());
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(result.ir.nodes.len(), 2);
        assert!(matches!(result.ir.nodes[0], IrNode::Heading { .. }));
        assert!(matches!(result.ir.nodes[1], IrNode::Paragraph { .. }));
    }

    #[test]
    fn compile_md_preserves_utf8_crlf_break_semantics_and_spans() {
        let source = "한글\r\n다음  \r\n끝";
        let project = VirtualProjectBuilder::new()
            .entry("main.md")
            .expect("valid path")
            .add_source("main.md", source)
            .expect("valid path")
            .build()
            .unwrap();
        let source_id = project.sources().get_id(project.entry()).unwrap();
        let result = super::compile(&project, &CompileOptions::default());
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        let IrNode::Paragraph { content, .. } = &result.ir.nodes[0] else {
            panic!("expected paragraph")
        };
        match content.as_slice() {
            [IrInline::Text {
                content: first,
                span: first_span,
            }, IrInline::SoftBreak { span: soft_span }, IrInline::Text {
                content: second,
                span: second_span,
            }, IrInline::HardBreak { span: hard_span }, IrInline::Text {
                content: third,
                span: third_span,
            }] => {
                assert_eq!(first, "한글");
                assert_eq!(second, "다음");
                assert_eq!(third, "끝");
                assert_eq!(*first_span, crate::source::SourceSpan::new(source_id, 0, 6));
                assert_eq!(*soft_span, crate::source::SourceSpan::new(source_id, 6, 8));
                assert_eq!(
                    *second_span,
                    crate::source::SourceSpan::new(source_id, 8, 14)
                );
                assert_eq!(
                    *hard_span,
                    crate::source::SourceSpan::new(source_id, 14, 18)
                );
                assert_eq!(
                    *third_span,
                    crate::source::SourceSpan::new(source_id, 18, 21)
                );
            }
            other => panic!("unexpected inline structure: {other:?}"),
        }
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
    fn compile_variable_multiple_paragraphs_inline_reference_is_not_flattened() {
        let source = ".var {x}\n    First\n\n    Second\n\nprefix .x suffix\n";
        let (result, _) = compile_source(source);
        assert_eq!(result.diagnostics.len(), 1, "{result:?}");
        assert_eq!(result.diagnostics[0].code, "E3003");
        assert!(!output_text(&result).contains("First"));
        assert!(!output_text(&result).contains("Second"));
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
