use arkst_engine::{
    ast_to_ir::ast_to_ir_with_diagnostics_for_mode, DocumentMetadataDefaults, IncludedSource,
    LoadableLibraryProvider, LoadableLibrarySource, ResourceAccessError, ResourceProvider,
    ResourceText, RuntimeMessageEvent, RuntimeMessageLevel, RuntimeMessageSink,
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
struct RecordingSink {
    events: RefCell<Vec<RuntimeMessageEvent>>,
}

impl RuntimeMessageSink for RecordingSink {
    fn emit(&self, event: RuntimeMessageEvent) {
        self.events.borrow_mut().push(event);
    }
}

#[test]
fn log_and_debug_require_explicit_sink_and_fail_closed_without_host_io() {
    let source_id = SourceId(1);
    let source = ".log {hello}\n.debug {hidden}\nafter";
    let (result, diagnostics) =
        arkst_engine::evaluator::Evaluator::new().evaluate(&document(source, source_id));

    assert_eq!(diagnostics.len(), 2, "{diagnostics:?}");
    assert!(diagnostics.iter().all(|diagnostic| diagnostic.code == "E3006"));
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.primary.map(|span| span.source_id) == Some(source_id)));
    assert_eq!(paragraph_texts(&result), vec!["after"]);
}

#[test]
fn explicit_sink_observes_log_debug_and_error_in_evaluation_order() {
    let source_id = SourceId(2);
    let sink = RecordingSink::default();
    let source = ".log {one}\n.debug {two}\n.error {three}\nafter";
    let (result, diagnostics) = arkst_engine::evaluator::Evaluator::new()
        .evaluate_with_runtime_message_sink(&sink, &document(source, source_id));

    let events = sink.events.borrow();
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].level, RuntimeMessageLevel::Log);
    assert_eq!(events[0].message, "one");
    assert_eq!(events[1].level, RuntimeMessageLevel::Debug);
    assert_eq!(events[1].message, "two");
    assert_eq!(events[2].level, RuntimeMessageLevel::Error);
    assert_eq!(events[2].message, "three");
    assert!(events
        .iter()
        .all(|event| event.span.source_id == source_id));
    drop(events);

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E3006");
    assert_eq!(diagnostics[0].message, "`.error`: three");
    assert_eq!(paragraph_texts(&result), vec!["after"]);
}

#[test]
fn logger_messages_use_shared_scalar_string_conversion() {
    let source_id = SourceId(3);
    let sink = RecordingSink::default();
    let source = ".log {42}\n.debug {true}";
    let (_, diagnostics) = arkst_engine::evaluator::Evaluator::new()
        .evaluate_with_runtime_message_sink(&sink, &document(source, source_id));
    assert!(diagnostics.is_empty(), "{diagnostics:?}");

    let events = sink.events.borrow();
    assert_eq!(
        events
            .iter()
            .map(|event| (event.level, event.message.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (RuntimeMessageLevel::Log, "42"),
            (RuntimeMessageLevel::Debug, "true"),
        ]
    );
}

#[test]
fn malformed_logger_arguments_emit_no_host_event() {
    let source_id = SourceId(4);
    let sink = RecordingSink::default();
    let source = ".log\n.debug {a} {b}";
    let (_, diagnostics) = arkst_engine::evaluator::Evaluator::new()
        .evaluate_with_runtime_message_sink(&sink, &document(source, source_id));
    assert_eq!(diagnostics.len(), 2, "{diagnostics:?}");
    assert!(diagnostics.iter().all(|diagnostic| diagnostic.code == "E3001"));
    assert!(sink.events.borrow().is_empty());
}

#[test]
fn source_defined_logger_name_keeps_existing_callable_precedence() {
    let source_id = SourceId(5);
    let sink = RecordingSink::default();
    let source = ".function {log}\n    shadowed\n.log {ignored}";
    let (result, diagnostics) = arkst_engine::evaluator::Evaluator::new()
        .evaluate_with_runtime_message_sink(&sink, &document(source, source_id));
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(paragraph_texts(&result), vec!["shadowed"]);
    assert!(sink.events.borrow().is_empty());
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
fn runtime_sink_propagates_through_user_functions_and_loaded_libraries() {
    let main = SourceId(10);
    let library_source = SourceId(11);
    let mut env = FakeEnvironment::default();
    env.libraries.insert(
        "probe".into(),
        LoadableLibrarySource {
            name: "probe".into(),
            source_id: library_source,
            text: ".log {from-library}\n.function {inside}\n    .debug {from-function}".into(),
        },
    );
    let sink = RecordingSink::default();
    let source = ".function {local}\n    .log {from-local}\n.local\n.include {probe}\n.inside";
    let evaluator = arkst_engine::evaluator::Evaluator::new();
    let (result, diagnostics) =
        evaluator.evaluate_with_resources_and_libraries_and_runtime_message_sink_for_mode(
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
            (RuntimeMessageLevel::Log, "from-local", main),
            (RuntimeMessageLevel::Log, "from-library", library_source),
            (RuntimeMessageLevel::Debug, "from-function", library_source),
        ]
    );
}

#[test]
fn repeated_evaluation_is_deterministic_and_does_not_retain_sink_state_in_evaluator() {
    let source_id = SourceId(20);
    let evaluator = arkst_engine::evaluator::Evaluator::new();
    let doc = document(".log {once}", source_id);

    let first = RecordingSink::default();
    let (_, first_diagnostics) = evaluator.evaluate_with_runtime_message_sink(&first, &doc);
    assert!(first_diagnostics.is_empty());
    assert_eq!(first.events.borrow().len(), 1);

    let second = RecordingSink::default();
    let (_, second_diagnostics) = evaluator.evaluate_with_runtime_message_sink(&second, &doc);
    assert!(second_diagnostics.is_empty());
    assert_eq!(second.events.borrow().len(), 1);
}
