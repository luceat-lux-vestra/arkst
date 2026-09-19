use arkst_engine::{
    ast_to_ir::ast_to_ir_with_diagnostics_for_mode, DocumentMetadataDefaults, EnvironmentInputs,
};
use arkst_ir::{IrDocument, IrInline, IrNode};
use arkst_markdown::Mode;
use arkst_source::SourceId;
use std::collections::BTreeMap;

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

fn environment(values: &[(&str, &str)]) -> EnvironmentInputs {
    EnvironmentInputs::new(
        values
            .iter()
            .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
            .collect::<BTreeMap<_, _>>(),
    )
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
fn injected_environment_returns_exact_string_and_absent_none() {
    let source_id = SourceId(1900);
    let source = ".var {present} {.env {ARKST_PRESENT}}\nvalue:.present\n\nabsent:.string {.isnone {.env {ARKST_ABSENT}}}";
    let inputs = environment(&[("ARKST_PRESENT", "qd190-present-value")]);

    let (result, diagnostics) = arkst_engine::evaluator::Evaluator::new()
        .evaluate_with_environment(&document(source, source_id), &inputs);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(
        paragraph_texts(&result),
        vec!["value:qd190-present-value", "absent:true"]
    );
}

#[test]
fn missing_environment_capability_fails_closed_without_ambient_fallback() {
    let source_id = SourceId(1901);
    let source = "before\n\n.env {PATH}\n\nafter";

    let (result, diagnostics) =
        arkst_engine::evaluator::Evaluator::new().evaluate(&document(source, source_id));

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E3004");
    let call_start = source.find(".env {PATH}").expect("env call");
    assert_eq!(
        diagnostics[0].primary,
        Some(arkst_source::SourceSpan::new(
            source_id,
            call_start,
            call_start + ".env {PATH}".len(),
        ))
    );
    assert!(diagnostics[0]
        .message
        .contains("Process environment capability"));
    assert!(diagnostics[0]
        .hints
        .iter()
        .any(|hint| hint.contains("EnvironmentInputs")));
    assert_eq!(paragraph_texts(&result), vec!["before", "after"]);
}

#[test]
fn malformed_env_call_fails_binding_before_capability_check() {
    let source_id = SourceId(1902);
    let (_, diagnostics) =
        arkst_engine::evaluator::Evaluator::new().evaluate(&document(".env", source_id));

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E3001");
    assert!(!diagnostics[0]
        .message
        .contains("Process environment capability"));
}

#[test]
fn env_argument_conversion_fails_before_missing_authority() {
    let source_id = SourceId(1906);
    let (_, diagnostics) = arkst_engine::evaluator::Evaluator::new()
        .evaluate(&document(".env {.pair {a} {b}}", source_id));

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E3001");
    assert!(diagnostics[0].message.contains(".env"));
    assert!(!diagnostics[0]
        .message
        .contains("Process environment capability"));
}

#[test]
fn named_env_argument_uses_the_same_binding_and_duplicate_fails_before_lookup() {
    let source_id = SourceId(1908);
    let inputs = environment(&[("ARKST_NAMED", "named-value")]);

    let (named_result, named_diagnostics) = arkst_engine::evaluator::Evaluator::new()
        .evaluate_with_environment(&document(".env name:{ARKST_NAMED}", source_id), &inputs);
    assert!(named_diagnostics.is_empty(), "{named_diagnostics:?}");
    assert_eq!(paragraph_texts(&named_result), vec!["named-value"]);

    let (duplicate_result, duplicate_diagnostics) = arkst_engine::evaluator::Evaluator::new()
        .evaluate_with_environment(
            &document(".env {ARKST_NAMED} name:{ARKST_NAMED}", source_id),
            &inputs,
        );
    assert_eq!(duplicate_diagnostics.len(), 1, "{duplicate_diagnostics:?}");
    assert_eq!(duplicate_diagnostics[0].code, "E3001");
    assert!(
        duplicate_result.nodes.is_empty(),
        "{:?}",
        duplicate_result.nodes
    );
}

#[test]
fn env_is_visible_to_library_inspection_as_a_real_native_owner() {
    let source_id = SourceId(1907);
    let inputs = EnvironmentInputs::default();
    let (result, diagnostics) = arkst_engine::evaluator::Evaluator::new()
        .evaluate_with_environment(&document(".functionexists {env}", source_id), &inputs);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(paragraph_texts(&result), vec!["true"]);
}

#[test]
fn environment_reference_propagates_through_source_defined_callable_scope() {
    let source_id = SourceId(1903);
    let source = ".function {readenv}\n    .env {ARKST_NESTED}\n\n.readenv";
    let inputs = environment(&[("ARKST_NESTED", "nested-value")]);

    let (result, diagnostics) = arkst_engine::evaluator::Evaluator::new()
        .evaluate_with_environment(&document(source, source_id), &inputs);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(paragraph_texts(&result), vec!["nested-value"]);
}

#[test]
fn repeated_evaluations_observe_only_the_supplied_snapshot() {
    let source_id = SourceId(1904);
    let document = document(".env {ARKST_REPEAT}", source_id);
    let evaluator = arkst_engine::evaluator::Evaluator::new();

    let first = environment(&[("ARKST_REPEAT", "first")]);
    let second = environment(&[("ARKST_REPEAT", "second")]);

    let (first_result, first_diagnostics) = evaluator.evaluate_with_environment(&document, &first);
    let (second_result, second_diagnostics) =
        evaluator.evaluate_with_environment(&document, &second);

    assert!(first_diagnostics.is_empty(), "{first_diagnostics:?}");
    assert!(second_diagnostics.is_empty(), "{second_diagnostics:?}");
    assert_eq!(paragraph_texts(&first_result), vec!["first"]);
    assert_eq!(paragraph_texts(&second_result), vec!["second"]);
}

#[test]
fn source_defined_env_name_keeps_existing_dispatch_precedence() {
    let source_id = SourceId(1905);
    let source = ".function {env}\n    shadow-env\n\n.env {ignored}";
    let inputs = environment(&[("ignored", "injected")]);

    let (result, diagnostics) = arkst_engine::evaluator::Evaluator::new()
        .evaluate_with_environment(&document(source, source_id), &inputs);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(paragraph_texts(&result), vec!["shadow-env"]);
}
