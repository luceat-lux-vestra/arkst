use arkst_engine::{ast_to_ir::ast_to_ir_with_diagnostics_for_mode, DocumentMetadataDefaults};
use arkst_ir::{IrDocument, IrInline, IrNode};
use arkst_markdown::Mode;
use arkst_source::SourceId;

fn document(source: &str, source_id: SourceId) -> IrDocument {
    let parsed = arkst_markdown::parse_with_mode(source, Mode::Quarkdown);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let (document, diagnostics) = ast_to_ir_with_diagnostics_for_mode(
        &parsed.document,
        source_id,
        &DocumentMetadataDefaults::default(),
        Mode::Quarkdown,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    document
}

fn evaluate(source: &str, source_id: SourceId) -> (IrDocument, Vec<arkst_diagnostics::Diagnostic>) {
    arkst_engine::evaluator::Evaluator::new().evaluate(&document(source, source_id))
}

fn paragraph_texts(document: &IrDocument) -> Vec<String> {
    document
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
        .collect()
}

#[test]
fn special_language_owned_functions_are_reported_when_arkst_supports_them() {
    let source_id = SourceId(70);
    let (result, diagnostics) = evaluate(
        ".functionexists {function}\n.functionexists {pageformat}\n.functionexists {code}\n.functionexists {extend}",
        source_id,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(
        paragraph_texts(&result),
        vec!["true", "true", "true", "true"]
    );
}

#[test]
fn source_defined_function_keeps_precedence_over_new_inspection_native_name() {
    let source_id = SourceId(71);
    let source = ".function {libexists}\n    shadowed\n.libexists {anything}";
    let (result, diagnostics) = evaluate(source, source_id);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(paragraph_texts(&result), vec!["shadowed"]);
}

#[test]
fn malformed_extra_and_unknown_named_arguments_fail_without_registry_mutation() {
    let source_id = SourceId(72);
    let source = ".libraries {extra}\n.libexists nope:{stdlib}\n.libraries";
    let (result, diagnostics) = evaluate(source, source_id);
    assert_eq!(diagnostics.len(), 2, "{diagnostics:?}");
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.primary.map(|span| span.source_id) == Some(source_id)));
    assert_eq!(paragraph_texts(&result), vec!["stdlib"]);
}

#[test]
fn structured_name_conversion_fails_closed_with_source_provenance() {
    let source_id = SourceId(73);
    let (result, diagnostics) = evaluate(".libexists {.pair {a} {b}}\n.libraries", source_id);
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(
        diagnostics[0].primary.map(|span| span.source_id),
        Some(source_id)
    );
    assert_eq!(paragraph_texts(&result), vec!["stdlib"]);
}
