use arkst_core::ir::{IrDocument, IrInline, IrNode};
use arkst_core::{
    compile, CompileOptions, CompileResult, Diagnostic, SourceId, VirtualProjectBuilder,
};
use arkst_engine::DocumentMetadataDefaults;
use arkst_markdown::Mode;

struct LegacyResult {
    ir: IrDocument,
    diagnostics: Vec<Diagnostic>,
}

fn build_project(source: &str) -> (arkst_core::VirtualProject, SourceId) {
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("valid entry")
        .add_source("main.qd", source)
        .expect("valid source")
        .build()
        .expect("valid project");
    let source_id = project
        .sources()
        .get_id(project.entry())
        .expect("entry source id");
    (project, source_id)
}

fn legacy_compile(
    project: &arkst_core::VirtualProject,
    source: &str,
    source_id: SourceId,
) -> LegacyResult {
    let parsed = arkst_markdown::parse_with_mode(source, Mode::Quarkdown);
    assert!(
        parsed.diagnostics.is_empty(),
        "G0 differential fixtures must start from parser-clean source: {:?}",
        parsed.diagnostics
    );

    let (lowered, lowering_diagnostics) =
        arkst_engine::ast_to_ir::ast_to_ir_with_diagnostics_for_mode(
            &parsed.document,
            source_id,
            &DocumentMetadataDefaults::default(),
            Mode::Quarkdown,
        );
    assert!(
        lowering_diagnostics.is_empty(),
        "G0 differential fixture must be representable by the legacy AST-to-IR bridge: {lowering_diagnostics:?}"
    );

    let (ir, diagnostics) =
        arkst_core::evaluator::Evaluator::new().evaluate_project(project, source_id, &lowered);
    LegacyResult { ir, diagnostics }
}

fn diagnostic_signature(diagnostics: &[Diagnostic]) -> Vec<String> {
    diagnostics
        .iter()
        .map(|diagnostic| {
            format!(
                "{}|{:?}|{}|{:?}|{:?}|{:?}",
                diagnostic.code,
                diagnostic.severity,
                diagnostic.message,
                diagnostic.primary,
                diagnostic.secondary,
                diagnostic.hints
            )
        })
        .collect()
}

fn compile_and_assert_legacy_parity(source: &str) -> CompileResult {
    let (project, source_id) = build_project(source);
    let frontend = compile(&project, &CompileOptions::default());
    let legacy = legacy_compile(&project, source, source_id);

    assert_eq!(
        frontend.ir, legacy.ir,
        "normal direct-frontend result diverged from the legacy pre-lowered IR evaluator"
    );
    assert_eq!(
        diagnostic_signature(&frontend.diagnostics),
        diagnostic_signature(&legacy.diagnostics),
        "normal direct-frontend diagnostics diverged from the legacy pre-lowered IR evaluator"
    );
    frontend
}

fn paragraph_text(result: &CompileResult) -> String {
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

#[test]
fn g0_s04_source_defined_direct_calls_match_legacy_ir_entrypoint() {
    let source = ".function {greet}\n    to from?:\n    Hello, .to from .from!\n\n.greet {world}\n.greet {world} from:{Jane}\n";
    let result = compile_and_assert_legacy_parity(source);

    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(
        paragraph_text(&result),
        "Hello, world from None!\nHello, world from Jane!"
    );
}

#[test]
fn g0_s12_failed_extension_document_state_rollback_matches_legacy_ir_entrypoint() {
    let source = ".docname {before}\n.function {greet}\n    name:\n    .docname {changed}\n    .sum {true} {2}\n\n.extend {greet}\n    name:\n    .super\n\n.greet {world}\n.docname\n";
    let result = compile_and_assert_legacy_parity(source);

    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert!(result.diagnostics[0].message.contains("numeric arguments"));
    assert_eq!(paragraph_text(&result), "before");
}

#[test]
fn g0_s16_dynamic_and_static_string_conversion_match_legacy_ir_entrypoint() {
    let dynamic = ".var {numbertext} {.string {-3.5}}\n.abs {.numbertext}\n";
    let dynamic_result = compile_and_assert_legacy_parity(dynamic);
    assert!(dynamic_result.diagnostics.is_empty(), "{dynamic_result:?}");
    assert_eq!(paragraph_text(&dynamic_result), "3.5");

    let static_text = ".abs {.string {-3.5}}\n";
    let static_result = compile_and_assert_legacy_parity(static_text);
    assert_eq!(static_result.diagnostics.len(), 1, "{static_result:?}");
    assert_eq!(static_result.diagnostics[0].code, "E3001");
    assert!(static_result.ir.nodes.is_empty(), "{static_result:?}");
}
