use arkst_engine::{
    ast_to_ir::ast_to_ir_with_diagnostics_for_mode, DocumentMetadataDefaults, IncludedSource,
    LoadableLibraryProvider, LoadableLibrarySource, LogEvent, LogLevel, LogSink,
    ResourceAccessError, ResourceProvider, ResourceText,
};
use arkst_ir::{IrDocument, IrInline, IrNode};
use arkst_markdown::Mode;
use arkst_source::SourceId;
use std::cell::RefCell;
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

fn evaluate_plain(
    source: &str,
    source_id: SourceId,
) -> (IrDocument, Vec<arkst_diagnostics::Diagnostic>) {
    arkst_engine::evaluator::Evaluator::new().evaluate(&document(source, source_id))
}

fn evaluate_with_sink(
    source: &str,
    source_id: SourceId,
    sink: &CollectingSink,
) -> (IrDocument, Vec<arkst_diagnostics::Diagnostic>) {
    arkst_engine::evaluator::Evaluator::new()
        .evaluate_with_log_sink(&document(source, source_id), sink)
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

#[derive(Default)]
struct CollectingSink {
    events: RefCell<Vec<LogEvent>>,
}

impl LogSink for CollectingSink {
    fn emit(&self, event: &LogEvent) {
        self.events.borrow_mut().push(event.clone());
    }
}

#[test]
fn explicit_sink_receives_log_and_debug_events_in_evaluation_order() {
    let source_id = SourceId(1970);
    let sink = CollectingSink::default();
    let (result, diagnostics) =
        evaluate_with_sink(".log {first}\n.debug {second}\n.log {42}", source_id, &sink);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(result.nodes.is_empty(), "{:?}", result.nodes);

    let events = sink.events.borrow();
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].level, LogLevel::Log);
    assert_eq!(events[0].message, "first");
    assert_eq!(events[1].level, LogLevel::Debug);
    assert_eq!(events[1].message, "second");
    assert_eq!(events[2].level, LogLevel::Log);
    assert_eq!(events[2].message, "42");
    assert!(events.iter().all(|event| event.span.source_id == source_id));
    assert!(events[0].span.start < events[1].span.start);
    assert!(events[1].span.start < events[2].span.start);
}

#[test]
fn debug_without_sink_is_a_silent_noop_but_still_validates_message_conversion() {
    let source_id = SourceId(1971);
    let (result, diagnostics) = evaluate_plain(".debug {hidden}", source_id);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(result.nodes.is_empty(), "{:?}", result.nodes);

    let (_, diagnostics) = evaluate_plain(".debug {.pair {a} {b}}", source_id);
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(
        diagnostics[0].primary.map(|span| span.source_id),
        Some(source_id)
    );
    assert!(diagnostics[0].message.contains(".debug"));
}

#[test]
fn log_without_sink_fails_deterministically_after_normal_argument_binding() {
    let source_id = SourceId(1972);
    let (_, diagnostics) = evaluate_plain(".log {hello}", source_id);
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E3010");
    assert_eq!(
        diagnostics[0].primary.map(|span| span.source_id),
        Some(source_id)
    );

    let (_, malformed) = evaluate_plain(".log", source_id);
    assert_eq!(malformed.len(), 1, "{malformed:?}");
    assert_eq!(malformed[0].code, "E3001");
    assert_ne!(malformed[0].code, "E3010");
}

#[test]
fn explicit_error_is_source_backed_and_document_evaluation_continues() {
    let source_id = SourceId(1973);
    let sink = CollectingSink::default();
    let source = "before\n\n.error {boom}\n\nafter";
    let (result, diagnostics) = evaluate_with_sink(source, source_id, &sink);

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E3011");
    assert_eq!(
        diagnostics[0].primary.map(|span| span.source_id),
        Some(source_id)
    );
    assert!(diagnostics[0].message.contains("boom"));
    assert!(sink.events.borrow().is_empty());
    assert_eq!(paragraph_texts(&result), vec!["before", "after"]);
}

#[test]
fn error_does_not_require_a_log_sink_and_continues_evaluation() {
    let source_id = SourceId(1982);
    let source = "before\n\n.error {boom-without-sink}\n\nafter";
    let (result, diagnostics) = evaluate_plain(source, source_id);

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E3011");
    assert_eq!(
        diagnostics[0].primary.map(|span| span.source_id),
        Some(source_id)
    );
    assert!(diagnostics[0].message.contains("boom-without-sink"));
    assert_eq!(paragraph_texts(&result), vec!["before", "after"]);
}

#[test]
fn log_before_nested_error_is_observable_even_when_the_enclosing_call_fails() {
    let source_id = SourceId(1974);
    let sink = CollectingSink::default();
    let source = ".function {boom}\n    .log {inside-log}\n    .error {inside-error}\n\nbefore\n\n.boom\n\nafter";
    let (result, diagnostics) = evaluate_with_sink(source, source_id, &sink);

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E3011");
    assert!(diagnostics[0].message.contains("inside-error"));
    assert_eq!(paragraph_texts(&result), vec!["before", "after"]);

    let events = sink.events.borrow();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].level, LogLevel::Log);
    assert_eq!(events[0].message, "inside-log");
}

#[test]
fn source_defined_logger_names_keep_existing_dispatch_precedence() {
    let source_id = SourceId(1975);
    let sink = CollectingSink::default();
    let source = ".function {log}\n    shadow-log\n.function {debug}\n    shadow-debug\n.function {error}\n    shadow-error\n.log {ignored}\n.debug {ignored}\n.error {ignored}";
    let (result, diagnostics) = evaluate_with_sink(source, source_id, &sink);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(sink.events.borrow().is_empty());
    assert_eq!(
        paragraph_texts(&result),
        vec!["shadow-log", "shadow-debug", "shadow-error"]
    );
}

#[test]
fn logger_names_are_visible_to_library_inspection_as_real_dispatch_owners() {
    let source_id = SourceId(1978);
    let (result, diagnostics) = evaluate_plain(
        ".functionexists {log}\n.functionexists {debug}\n.functionexists {error}",
        source_id,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(paragraph_texts(&result), vec!["true", "true", "true"]);
}

#[test]
fn malformed_or_non_stringable_log_arguments_never_emit_events() {
    let source_id = SourceId(1976);
    let sink = CollectingSink::default();

    let (_, duplicate) = evaluate_with_sink(".log {first} message:{second}", source_id, &sink);
    assert_eq!(duplicate.len(), 1, "{duplicate:?}");
    assert!(sink.events.borrow().is_empty());

    let (_, structured) = evaluate_with_sink(".log {.pair {a} {b}}", source_id, &sink);
    assert_eq!(structured.len(), 1, "{structured:?}");
    assert_eq!(
        structured[0].primary.map(|span| span.source_id),
        Some(source_id)
    );
    assert!(structured[0].message.contains(".log"));
    assert!(sink.events.borrow().is_empty());
}

#[test]
fn named_message_arguments_use_the_same_logger_binding_contract() {
    let source_id = SourceId(1979);
    let sink = CollectingSink::default();
    let source =
        ".log message:{named-log}\n.debug message:{named-debug}\n.error message:{named-error}";
    let (_, diagnostics) = evaluate_with_sink(source, source_id, &sink);

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E3011");
    assert!(diagnostics[0].message.contains("named-error"));

    let events = sink.events.borrow();
    assert_eq!(
        events
            .iter()
            .map(|event| (event.level, event.message.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (LogLevel::Log, "named-log"),
            (LogLevel::Debug, "named-debug"),
        ]
    );
}

#[test]
fn repeated_evaluation_has_no_hidden_global_logger_state() {
    let source_id = SourceId(1977);
    let sink = CollectingSink::default();
    let document = document(".log {repeat}", source_id);
    let evaluator = arkst_engine::evaluator::Evaluator::new();

    let (_, first_diagnostics) = evaluator.evaluate_with_log_sink(&document, &sink);
    let (_, second_diagnostics) = evaluator.evaluate_with_log_sink(&document, &sink);

    assert!(first_diagnostics.is_empty(), "{first_diagnostics:?}");
    assert!(second_diagnostics.is_empty(), "{second_diagnostics:?}");
    let events = sink.events.borrow();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0], events[1]);
}

#[derive(Default)]
struct FakeEnvironment {
    libraries: BTreeMap<String, LoadableLibrarySource>,
}

impl ResourceProvider for FakeEnvironment {
    fn source_path(&self, source_id: SourceId) -> Option<String> {
        self.libraries
            .values()
            .find(|library| library.source_id == source_id)
            .map(|library| format!("@library/{}", library.name))
    }

    fn read_text(
        &self,
        _source_id: SourceId,
        reference: &str,
    ) -> Result<ResourceText, ResourceAccessError> {
        Err(ResourceAccessError::NotFound {
            path: reference.to_string(),
        })
    }

    fn read_source(
        &self,
        _source_id: SourceId,
        reference: &str,
    ) -> Result<IncludedSource, ResourceAccessError> {
        Err(ResourceAccessError::NotFound {
            path: reference.to_string(),
        })
    }
}

impl LoadableLibraryProvider for FakeEnvironment {
    fn loadable_library(&self, name: &str) -> Option<LoadableLibrarySource> {
        self.libraries.get(name).cloned()
    }
}

#[test]
fn explicit_sink_propagates_through_loaded_libraries_and_function_calls() {
    let main = SourceId(1980);
    let library_source = SourceId(1981);
    let mut env = FakeEnvironment::default();
    env.libraries.insert(
        "probe".into(),
        LoadableLibrarySource {
            name: "probe".into(),
            source_id: library_source,
            text: ".log {from-library}\n.function {inside}\n    .debug {from-function}".into(),
        },
    );
    let sink = CollectingSink::default();
    let source = ".function {local}\n    .log {from-local}\n.local\n.include {probe}\n.inside";
    let evaluator = arkst_engine::evaluator::Evaluator::new();
    let (result, diagnostics) = evaluator
        .evaluate_with_resources_and_libraries_and_log_sink_for_mode(
            &env,
            &env,
            &sink,
            main,
            Mode::Quarkdown,
            &document(source, main),
            &DocumentMetadataDefaults::default(),
        );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(paragraph_texts(&result).is_empty());

    let events = sink.events.borrow();
    assert_eq!(
        events
            .iter()
            .map(|event| (event.level, event.message.as_str(), event.span.source_id))
            .collect::<Vec<_>>(),
        vec![
            (LogLevel::Log, "from-local", main),
            (LogLevel::Log, "from-library", library_source),
            (LogLevel::Debug, "from-function", library_source),
        ]
    );
}

#[test]
fn unit_result_is_output_suppressed_for_direct_logger_calls_but_observable_after_capture() {
    let source_id = SourceId(1983);
    let sink = CollectingSink::default();
    let source = ".log {direct}\n.var {x} {.log {captured}}\n.x";
    let (result, diagnostics) = evaluate_with_sink(source, source_id, &sink);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(paragraph_texts(&result), vec!["kotlin.Unit"]);

    let events = sink.events.borrow();
    assert_eq!(
        events
            .iter()
            .map(|event| (event.level, event.message.as_str()))
            .collect::<Vec<_>>(),
        vec![(LogLevel::Log, "direct"), (LogLevel::Log, "captured")]
    );
}

#[test]
fn unit_result_propagates_through_functions_without_becoming_direct_output() {
    let source_id = SourceId(1984);
    let source = ".function {silent}\n    .debug {inside}\n.silent\n.var {x} {.silent}\n.x\n.silent::string";
    let (result, diagnostics) = evaluate_plain(source, source_id);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(paragraph_texts(&result), vec!["kotlin.Unit", "kotlin.Unit"]);
}

#[test]
fn suppressed_unit_does_not_pollute_later_callable_content() {
    let source_id = SourceId(1985);
    let source = ".function {mixed}\n    .debug {side-effect}\n    visible\n.mixed";
    let (result, diagnostics) = evaluate_plain(source, source_id);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(paragraph_texts(&result), vec!["visible"]);
}

#[test]
fn unit_optionality_equality_and_string_conversion_match_clean_room_contract() {
    let source_id = SourceId(1986);
    let source = ".var {u} {.debug {unit}}\n.isnone {.u}\n.equals {.u} to:{.none}\n.equals {.none} to:{.u}\n.equals {.u} to:{\"kotlin.Unit\"}\n.string {.u}";
    let (result, diagnostics) = evaluate_plain(source, source_id);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(
        paragraph_texts(&result),
        vec!["false", "true", "true", "false", "kotlin.Unit"]
    );
}
