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

fn paragraph_text(node: &IrNode) -> String {
    let IrNode::Paragraph { content, .. } = node else {
        panic!("expected paragraph, got {node:?}");
    };
    content
        .iter()
        .filter_map(|inline| match inline {
            IrInline::Text { content, .. } => Some(content.as_str()),
            _ => None,
        })
        .collect::<String>()
        .trim()
        .to_string()
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
fn logger_dynamic_string_boundary_matches_clean_room_structured_subset() {
    let source_id = SourceId(1989);
    let sink = CollectingSink::default();
    let source = ".log {.pair {left} {right}}\n.var {stored} {.pair {.pair {left} {right}} {.none}}\n.debug {.stored}\n.log {.range {1} {3}}\n.log {.range to:{3}}\n.log {.range from:{2}}\n.log {.range}\n.log {.none}\n.var {table}\n    .dictionary\n        - first: one\n        - second: 2\n.log {.table}\n.error {.stored}";
    let (result, diagnostics) = evaluate_with_sink(source, source_id, &sink);
    let pair =
        "[DynamicValue(unwrappedValue=left, evaluationContext=null), DynamicValue(unwrappedValue=right, evaluationContext=null)]";
    let nested_pair = format!(
        "[DynamicValue(unwrappedValue={pair}, evaluationContext=null), DynamicValue(unwrappedValue=None, evaluationContext=null)]"
    );
    let dictionary =
        "{first=DynamicValue(unwrappedValue=one, evaluationContext=null), second=DynamicValue(unwrappedValue=2, evaluationContext=null)}";

    let [IrNode::Component {
        component: arkst_ir::IrComponent::ExplicitError(error),
    }] = result.nodes.as_slice()
    else {
        panic!("expected explicit error component, got {:?}", result.nodes);
    };
    assert_eq!(error.message, nested_pair);
    assert_eq!(error.span.source_id, source_id);
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E3011");
    assert_eq!(
        diagnostics[0].primary.map(|span| span.source_id),
        Some(source_id)
    );

    assert!(
        diagnostics[0].message.contains(&nested_pair),
        "{diagnostics:?}"
    );

    let events = sink.events.borrow();
    assert_eq!(
        events
            .iter()
            .map(|event| (event.level, event.message.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (LogLevel::Log, pair),
            (LogLevel::Debug, nested_pair.as_str()),
            (LogLevel::Log, "1..3"),
            (LogLevel::Log, "..3"),
            (LogLevel::Log, "2.."),
            (LogLevel::Log, ".."),
            (LogLevel::Log, "None"),
            (LogLevel::Log, dictionary),
        ]
    );
}

#[test]
fn direct_markdown_list_logger_conversion_is_bounded_to_evidenced_source_origin() {
    let source_id = SourceId(1998);
    let sink = CollectingSink::default();
    let source = ".var {values}\n    - alpha\n    - beta\n.log {.values}\n.debug {.values}\n.error {.values}";
    let (result, diagnostics) = evaluate_with_sink(source, source_id, &sink);
    let expected = "- alpha\n- beta";

    let [IrNode::Component {
        component: arkst_ir::IrComponent::ExplicitError(error),
    }] = result.nodes.as_slice()
    else {
        panic!("expected explicit error component, got {:?}", result.nodes);
    };
    assert_eq!(error.message, expected);
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E3011");
    assert!(diagnostics[0].message.contains(expected), "{diagnostics:?}");

    let events = sink.events.borrow();
    assert_eq!(
        events
            .iter()
            .map(|event| (event.level, event.message.as_str()))
            .collect::<Vec<_>>(),
        vec![(LogLevel::Log, expected), (LogLevel::Debug, expected)]
    );
}

#[test]
fn direct_markdown_list_logger_conversion_preserves_evidenced_source_spelling() {
    let source_id = SourceId(2001);
    let cases = [
        (
            ".var {values}\n    * alpha\n    * beta\n.log {.values}",
            "* alpha\n* beta",
        ),
        (
            ".var {values}\n    + alpha\n    + beta\n.log {.values}",
            "+ alpha\n+ beta",
        ),
        (
            ".var {values}\n    -   alpha\n    -    beta\n.log {.values}",
            "-   alpha\n-    beta",
        ),
        (
            ".var {values}\n    1. alpha\n    2. beta\n.log {.values}",
            "1. alpha\n2. beta",
        ),
        (
            ".var {values}\n    - **alpha**\n    - beta *em*\n.log {.values}",
            "- **alpha**\n- beta *em*",
        ),
        (
            ".var {values}\n    - alpha\n        - nested-a\n        - nested-b\n    - beta\n.log {.values}",
            "- alpha\n    - nested-a\n    - nested-b\n- beta",
        ),
        (
            ".var {values}\n    - [x] alpha\n    - [ ] beta\n.log {.values}",
            "- [x] alpha\n- [ ] beta",
        ),
        (
            ".var {values}\n    - alpha\n      continuation\n    - beta\n.log {.values}",
            "- alpha\n  continuation\n- beta",
        ),
    ];

    for (source, expected) in cases {
        let sink = CollectingSink::default();
        let (result, diagnostics) = evaluate_with_sink(source, source_id, &sink);
        assert!(diagnostics.is_empty(), "{source}: {diagnostics:?}");
        assert!(result.nodes.is_empty(), "{source}: {:?}", result.nodes);
        let events = sink.events.borrow();
        assert_eq!(events.len(), 1, "{source}: {events:?}");
        assert_eq!(events[0].level, LogLevel::Log, "{source}");
        assert_eq!(events[0].message, expected, "{source}");
    }
}

#[test]
fn operation_generated_collections_remain_fail_closed_at_logger_string_boundary() {
    let source_id = SourceId(1999);
    let cases = [
        ".var {values}\n    - alpha\n    - beta\n.debug {.values::reversed}",
        ".var {values}\n    - alpha\n    - beta\n    - alpha\n.debug {.values::distinct}",
        ".var {values}\n    - beta\n    - alpha\n.debug {.values::sorted}",
        ".debug {.range {1} {3}::reversed}",
        ".debug {.pair {left} {right}::reversed}",
        ".var {generated}\n    .foreach {1..2}\n        .1\n.debug {.generated}",
        ".var {values}\n    - alpha\n    - beta\n    - alpha\n.debug {.values::groupvalues}",
    ];

    for source in cases {
        let (result, diagnostics) = evaluate_plain(source, source_id);
        assert!(result.nodes.is_empty(), "{source}: {:?}", result.nodes);
        assert_eq!(diagnostics.len(), 1, "{source}: {diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E3001", "{source}: {diagnostics:?}");
        assert!(
            diagnostics[0].message.contains(".debug"),
            "{source}: {diagnostics:?}"
        );
    }
}

#[test]
fn direct_markdown_list_provenance_does_not_escape_evidenced_variable_boundary() {
    let source_id = SourceId(2000);
    let cases = [
        ".var {values}\n    - alpha\n    - beta\n.var {alias} {.values}\n.debug {.alias}",
        ".function {make}\n    - alpha\n    - beta\n.var {generated} {.make}\n.debug {.generated}",
        ".var {values}\n    - alpha\n    - beta\n.var {values} {.range {1} {3}::reversed}\n.debug {.values}",
        ".var {values}\n    - alpha\n    - beta\n.function {show}\n    .debug {.values}\n.show",
        ".var {suffix} {beta}\n.var {values}\n    - alpha\n    - .suffix\n.debug {.values}",
        ".var {values}\n    - alpha  \n      hard-break-tail\n.debug {.values}",
        ".var {values}\n    1. **alpha**\n    2. beta\n.debug {.values}",
        ".var {values}\n    - alpha\n        - **nested-rich**\n    - beta\n.debug {.values}",
    ];

    for source in cases {
        let (result, diagnostics) = evaluate_plain(source, source_id);
        assert!(result.nodes.is_empty(), "{source}: {:?}", result.nodes);
        assert_eq!(diagnostics.len(), 1, "{source}: {diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E3001", "{source}: {diagnostics:?}");
        assert!(
            diagnostics[0].message.contains(".debug"),
            "{source}: {diagnostics:?}"
        );
    }
}

#[test]
fn debug_without_sink_is_silent_and_validates_bounded_logger_conversion() {
    let source_id = SourceId(1971);
    let (result, diagnostics) = evaluate_plain(".debug {hidden}", source_id);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(result.nodes.is_empty(), "{:?}", result.nodes);

    let source = ".debug {.pair {a} {b}}\n.debug {.none}\n.var {u} {.debug {unit}}\n.debug {.u}";
    let (result, diagnostics) = evaluate_plain(source, source_id);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(result.nodes.is_empty(), "{:?}", result.nodes);

    let (result, diagnostics) = evaluate_plain(
        ".var {table}\n    .dictionary\n        - a: 1\n.debug {.table}",
        source_id,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(result.nodes.is_empty(), "{:?}", result.nodes);

    let (result, diagnostics) = evaluate_plain(
        ".var {values}\n    - alpha\n    - beta\n.debug {.values}",
        source_id,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(result.nodes.is_empty(), "{:?}", result.nodes);
}

#[test]
fn log_without_sink_fails_after_supported_message_conversion() {
    let source_id = SourceId(1972);
    for source in [
        ".log {hello}",
        ".log {.pair {left} {right}}",
        ".log {.range {1} {3}}",
        ".log {.range to:{3}}",
        ".log {.range from:{2}}",
        ".log {.range}",
        ".log {.pair {.pair {left} {right}} {.none}}",
        ".var {table}\n    .dictionary\n        - a: 1\n.log {.table}",
        ".log {.none}",
    ] {
        let (_, diagnostics) = evaluate_plain(source, source_id);
        assert_eq!(diagnostics.len(), 1, "{source}: {diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E3010", "{source}: {diagnostics:?}");
        assert_eq!(
            diagnostics[0].primary.map(|span| span.source_id),
            Some(source_id)
        );
    }

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
    assert_eq!(
        diagnostics[0].message,
        "Cannot call function error(String message) with arguments (boom): boom"
    );
    assert!(sink.events.borrow().is_empty());
    assert_eq!(paragraph_texts(&result), vec!["before", "after"]);
    assert!(matches!(
        result.nodes.as_slice(),
        [
            IrNode::Paragraph { .. },
            IrNode::Component {
                component: arkst_ir::IrComponent::ExplicitError(_)
            },
            IrNode::Paragraph { .. }
        ]
    ));
}

#[test]
fn explicit_error_inside_container_family_preserves_evidenced_sibling_content() {
    let cases = [
        (
            ".center\n    center-before\n    .error {center-error}\n    center-after",
            "center-before",
            "center-error",
            "center-after",
        ),
        (
            ".align {center}\n    align-before\n    .error {align-error}\n    align-after",
            "align-before",
            "align-error",
            "align-after",
        ),
        (
            ".container\n    container-before\n    .error {container-error}\n    container-after",
            "container-before",
            "container-error",
            "container-after",
        ),
    ];

    for (source, before_text, error_text, after_text) in cases {
        let source_id = SourceId(1994);
        let (result, diagnostics) = evaluate_plain(source, source_id);

        assert_eq!(diagnostics.len(), 1, "{source}: {diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E3011", "{source}: {diagnostics:?}");

        let [IrNode::Component {
            component: arkst_ir::IrComponent::Container(container),
        }] = result.nodes.as_slice()
        else {
            panic!(
                "expected one container for {source}, got {:?}",
                result.nodes
            );
        };
        let [before, error, after] = container.children.as_slice() else {
            panic!(
                "expected paragraph/error/paragraph for {source}, got {:?}",
                container.children
            );
        };
        assert_eq!(paragraph_text(before), before_text, "{source}");
        let IrNode::Component {
            component: arkst_ir::IrComponent::ExplicitError(error),
        } = error
        else {
            panic!("expected explicit error component for {source}, got {error:?}");
        };
        assert_eq!(error.message, error_text, "{source}");
        assert_eq!(paragraph_text(after), after_text, "{source}");
    }
}

#[test]
fn explicit_error_inside_blockquote_preserves_evidenced_sibling_content() {
    let source_id = SourceId(1991);
    let source = "> quote-before .error {quote-error} quote-after";
    let (result, diagnostics) = evaluate_plain(source, source_id);

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E3011");

    let [IrNode::Blockquote { content, .. }] = result.nodes.as_slice() else {
        panic!("expected one blockquote, got {:?}", result.nodes);
    };
    let [before, error, after] = content.as_slice() else {
        panic!("expected paragraph/error/paragraph, got {content:?}");
    };
    assert_eq!(paragraph_text(before), "quote-before");
    let IrNode::Component {
        component: arkst_ir::IrComponent::ExplicitError(error),
    } = error
    else {
        panic!("expected explicit error component, got {error:?}");
    };
    assert_eq!(error.message, "quote-error");
    assert_eq!(paragraph_text(after), "quote-after");
}

#[test]
fn explicit_error_inside_unordered_list_preserves_evidenced_sibling_content() {
    let source_id = SourceId(1992);
    let source = "- list-before .error {list-error} list-after";
    let (result, diagnostics) = evaluate_plain(source, source_id);

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E3011");

    let [IrNode::UnorderedList { items, .. }] = result.nodes.as_slice() else {
        panic!("expected one unordered list, got {:?}", result.nodes);
    };
    assert_eq!(items.len(), 1, "{items:?}");
    let [before, error, after] = items[0].nodes.as_slice() else {
        panic!(
            "expected paragraph/error/paragraph, got {:?}",
            items[0].nodes
        );
    };
    assert_eq!(paragraph_text(before), "list-before");
    let IrNode::Component {
        component: arkst_ir::IrComponent::ExplicitError(error),
    } = error
    else {
        panic!("expected explicit error component, got {error:?}");
    };
    assert_eq!(error.message, "list-error");
    assert_eq!(paragraph_text(after), "list-after");
}

#[test]
fn top_level_inline_explicit_error_preserves_evidenced_sibling_content() {
    let source_id = SourceId(1993);
    let source = "before .error {top-inline-error} after";
    let (result, diagnostics) = evaluate_plain(source, source_id);

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E3011");
    let [before, error, after] = result.nodes.as_slice() else {
        panic!("expected paragraph/error/paragraph, got {:?}", result.nodes);
    };
    assert_eq!(paragraph_text(before), "before");
    let IrNode::Component {
        component: arkst_ir::IrComponent::ExplicitError(error),
    } = error
    else {
        panic!("expected explicit error component, got {error:?}");
    };
    assert_eq!(error.message, "top-inline-error");
    assert_eq!(paragraph_text(after), "after");
}

#[test]
fn inline_explicit_error_stays_fail_closed_in_selected_conditional_body() {
    let source_id = SourceId(1999);
    let source = "outer-before\n.if {true}\n    inner-before .error {conditional-inline-error} inner-after\nouter-after";
    let (result, diagnostics) = evaluate_plain(source, source_id);

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E3011"),
        "{diagnostics:?}"
    );
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "E3001"
                && diagnostic.message == "Semantic component is block-only"
        }),
        "unevidenced inline error must keep ordinary inline materialization rejection: {diagnostics:?}"
    );
    assert!(
        result.nodes.iter().all(|node| !matches!(
            node,
            IrNode::Component {
                component: arkst_ir::IrComponent::ExplicitError(_)
            }
        )),
        "unevidenced conditional-inline error must not escape as recoverable output: {:?}",
        result.nodes
    );
}

#[test]
fn inline_explicit_error_stays_fail_closed_in_user_function_body() {
    let source_id = SourceId(2000);
    let source = ".function {boom}\n    fn-before .error {function-inline-error} fn-after\nouter-before\n.boom\nouter-after";
    let (result, diagnostics) = evaluate_plain(source, source_id);

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E3011"),
        "{diagnostics:?}"
    );
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "E3001"
                && diagnostic.message == "Semantic component is block-only"
        }),
        "unevidenced inline error must keep ordinary inline materialization rejection: {diagnostics:?}"
    );
    assert!(
        result.nodes.iter().all(|node| !matches!(
            node,
            IrNode::Component {
                component: arkst_ir::IrComponent::ExplicitError(_)
            }
        )),
        "unevidenced function-inline error must not escape as recoverable output: {:?}",
        result.nodes
    );
}

#[test]
fn explicit_error_inside_ordered_list_preserves_evidenced_sibling_content() {
    let source_id = SourceId(1995);
    let source = "1. ordered-before .error {ordered-error} ordered-after";
    let (result, diagnostics) = evaluate_plain(source, source_id);

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E3011");
    let [IrNode::OrderedList { items, .. }] = result.nodes.as_slice() else {
        panic!("expected one ordered list, got {:?}", result.nodes);
    };
    let [before, error, after] = items[0].nodes.as_slice() else {
        panic!(
            "expected paragraph/error/paragraph, got {:?}",
            items[0].nodes
        );
    };
    assert_eq!(paragraph_text(before), "ordered-before");
    let IrNode::Component {
        component: arkst_ir::IrComponent::ExplicitError(error),
    } = error
    else {
        panic!("expected explicit error component, got {error:?}");
    };
    assert_eq!(error.message, "ordered-error");
    assert_eq!(paragraph_text(after), "ordered-after");
}

#[test]
fn explicit_error_inside_stacked_and_landscape_preserves_evidenced_siblings() {
    let stacked_cases = [
        (
            ".row\n    row-before\n    .error {row-error}\n    row-after",
            "row-before",
            "row-error",
            "row-after",
        ),
        (
            ".column\n    column-before\n    .error {column-error}\n    column-after",
            "column-before",
            "column-error",
            "column-after",
        ),
        (
            ".grid columns:{2}\n    grid-before\n    .error {grid-error}\n    grid-after",
            "grid-before",
            "grid-error",
            "grid-after",
        ),
    ];

    for (source, before_text, error_text, after_text) in stacked_cases {
        let (result, diagnostics) = evaluate_plain(source, SourceId(1996));
        assert_eq!(diagnostics.len(), 1, "{source}: {diagnostics:?}");
        let [IrNode::Component {
            component: arkst_ir::IrComponent::Stacked(stacked),
        }] = result.nodes.as_slice()
        else {
            panic!(
                "expected one stacked component for {source}, got {:?}",
                result.nodes
            );
        };
        let [before, error, after] = stacked.children.as_slice() else {
            panic!(
                "expected paragraph/error/paragraph for {source}, got {:?}",
                stacked.children
            );
        };
        assert_eq!(paragraph_text(before), before_text);
        let IrNode::Component {
            component: arkst_ir::IrComponent::ExplicitError(error),
        } = error
        else {
            panic!("expected explicit error component, got {error:?}");
        };
        assert_eq!(error.message, error_text);
        assert_eq!(paragraph_text(after), after_text);
    }

    let source =
        ".landscape\n    landscape-before\n    .error {landscape-error}\n    landscape-after";
    let (result, diagnostics) = evaluate_plain(source, SourceId(1997));
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    let [IrNode::Component {
        component: arkst_ir::IrComponent::Landscape(landscape),
    }] = result.nodes.as_slice()
    else {
        panic!("expected one landscape component, got {:?}", result.nodes);
    };
    let [before, error, after] = landscape.children.as_slice() else {
        panic!(
            "expected paragraph/error/paragraph, got {:?}",
            landscape.children
        );
    };
    assert_eq!(paragraph_text(before), "landscape-before");
    let IrNode::Component {
        component: arkst_ir::IrComponent::ExplicitError(error),
    } = error
    else {
        panic!("expected explicit error component, got {error:?}");
    };
    assert_eq!(error.message, "landscape-error");
    assert_eq!(paragraph_text(after), "landscape-after");
}

#[test]
fn explicit_error_inside_evidenced_center_row_composition_preserves_siblings() {
    let source = ".center\n    center-before\n    .row\n        deep-before\n        .error {deep-error}\n        deep-after\n    center-after";
    let (result, diagnostics) = evaluate_plain(source, SourceId(1998));

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    let [IrNode::Component {
        component: arkst_ir::IrComponent::Container(container),
    }] = result.nodes.as_slice()
    else {
        panic!("expected one container, got {:?}", result.nodes);
    };
    let [center_before, stacked, center_after] = container.children.as_slice() else {
        panic!("unexpected center children: {:?}", container.children);
    };
    assert_eq!(paragraph_text(center_before), "center-before");
    let IrNode::Component {
        component: arkst_ir::IrComponent::Stacked(stacked),
    } = stacked
    else {
        panic!("expected nested stacked component, got {stacked:?}");
    };
    let [deep_before, error, deep_after] = stacked.children.as_slice() else {
        panic!("unexpected stacked children: {:?}", stacked.children);
    };
    assert_eq!(paragraph_text(deep_before), "deep-before");
    let IrNode::Component {
        component: arkst_ir::IrComponent::ExplicitError(error),
    } = error
    else {
        panic!("expected explicit error component, got {error:?}");
    };
    assert_eq!(error.message, "deep-error");
    assert_eq!(paragraph_text(deep_after), "deep-after");
    assert_eq!(paragraph_text(center_after), "center-after");
}

#[test]
fn explicit_error_inside_selected_conditional_replaces_body_but_caller_continues() {
    let source_id = SourceId(1988);
    let source = "outer-before\n.if {true}\n    if-before\n    .error {conditional-error}\n    if-after\nouter-after";
    let (result, diagnostics) = evaluate_plain(source, source_id);

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E3011");
    assert_eq!(
        paragraph_texts(&result),
        vec!["outer-before", "outer-after"]
    );
    assert!(matches!(
        result.nodes.as_slice(),
        [
            IrNode::Paragraph { .. },
            IrNode::Component {
                component: arkst_ir::IrComponent::ExplicitError(_)
            },
            IrNode::Paragraph { .. }
        ]
    ));
}

#[test]
fn explicit_error_in_unselected_conditional_stays_lazy_and_silent() {
    let source_id = SourceId(1987);
    let source = "before\n.if {false}\n    .error {must-not-run}\nafter";
    let (result, diagnostics) = evaluate_plain(source, source_id);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(paragraph_texts(&result), vec!["before", "after"]);
    assert!(result.nodes.iter().all(|node| !matches!(
        node,
        IrNode::Component {
            component: arkst_ir::IrComponent::ExplicitError(_)
        }
    )));
}

#[test]
fn error_component_stops_a_following_chain_without_secondary_diagnostics() {
    let source_id = SourceId(1990);
    let source = "before\n\n.error {chain-error}::isnone\n\nafter";
    let (result, diagnostics) = evaluate_plain(source, source_id);

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E3011");
    assert_eq!(paragraph_texts(&result), vec!["before", "after"]);
    assert!(matches!(
        result.nodes.as_slice(),
        [
            IrNode::Paragraph { .. },
            IrNode::Component {
                component: arkst_ir::IrComponent::ExplicitError(_)
            },
            IrNode::Paragraph { .. }
        ]
    ));
}

#[test]
fn repeated_top_level_explicit_errors_preserve_order_and_continuation() {
    let source_id = SourceId(1994);
    let source = "before\n.error {first-error}\nmiddle\n.error {second-error}\nafter";
    let (result, diagnostics) = evaluate_plain(source, source_id);

    assert_eq!(diagnostics.len(), 2, "{diagnostics:?}");
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code == "E3011"));
    assert_eq!(paragraph_texts(&result), vec!["before", "middle", "after"]);

    let messages = result
        .nodes
        .iter()
        .filter_map(|node| match node {
            IrNode::Component {
                component: arkst_ir::IrComponent::ExplicitError(error),
            } => Some(error.message.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(messages, vec!["first-error", "second-error"]);
}

#[test]
fn explicit_error_in_value_context_fails_without_fabricating_a_component_value() {
    let source_id = SourceId(1991);
    let (result, diagnostics) = evaluate_plain(".uppercase {.error {nested-error}}", source_id);

    assert!(result.nodes.is_empty(), "{:?}", result.nodes);
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E3011");
    assert!(diagnostics[0].message.contains("nested-error"));
}

#[test]
fn explicit_error_in_logger_arguments_fails_without_emitting_events() {
    let source_id = SourceId(1995);
    let sink = CollectingSink::default();
    let source = ".log {.error {positional-error}}\n.log message:{.error {named-error}}";
    let (result, diagnostics) = evaluate_with_sink(source, source_id, &sink);

    assert!(result.nodes.is_empty(), "{:?}", result.nodes);
    assert_eq!(diagnostics.len(), 2, "{diagnostics:?}");
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code == "E3011"));
    assert!(diagnostics[0].message.contains("positional-error"));
    assert!(diagnostics[1].message.contains("named-error"));
    assert!(sink.events.borrow().is_empty());
}

#[test]
fn explicit_error_in_variable_body_does_not_become_a_stored_value() {
    let source_id = SourceId(1996);
    let source = ".var {stored} {before}\n.var {stored}\n    .error {var-error}\n.stored";
    let (result, diagnostics) = evaluate_plain(source, source_id);

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E3011");
    assert_eq!(paragraph_texts(&result), vec!["before"]);
    assert!(result.nodes.iter().all(|node| !matches!(
        node,
        IrNode::Component {
            component: arkst_ir::IrComponent::ExplicitError(_)
        }
    )));
}

#[test]
fn explicit_error_in_user_function_body_argument_fails_closed() {
    let source_id = SourceId(1997);
    let source = ".function {wrap}\n    title content?:\n    .content\n.wrap {Title}\n    .error {body-error}";
    let (result, diagnostics) = evaluate_plain(source, source_id);

    assert!(result.nodes.is_empty(), "{:?}", result.nodes);
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E3011");
    assert!(diagnostics[0].message.contains("body-error"));
}

#[test]
fn explicit_error_terminates_callable_body_but_caller_continues() {
    let source_id = SourceId(1993);
    let source = ".function {boom}\n    fn-before\n    .error {fn-error}\n    fn-after\nouter-before\n.boom\nouter-after";
    let (result, diagnostics) = evaluate_plain(source, source_id);

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E3011");
    assert_eq!(
        paragraph_texts(&result),
        vec!["outer-before", "outer-after"]
    );
    assert!(matches!(
        result.nodes.as_slice(),
        [
            IrNode::Paragraph { .. },
            IrNode::Component {
                component: arkst_ir::IrComponent::ExplicitError(_)
            },
            IrNode::Paragraph { .. }
        ]
    ));
}

#[test]
fn explicit_error_preserves_failure_style_state_rollback_inside_user_function() {
    let source_id = SourceId(1992);
    let source = ".var {state} {before}\n.function {boom}\n    .state {after}\n    .error {rollback}\n.boom\n.state";
    let (result, diagnostics) = evaluate_plain(source, source_id);

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E3011");
    assert_eq!(paragraph_texts(&result), vec!["before"]);
    assert!(matches!(
        result.nodes.first(),
        Some(IrNode::Component {
            component: arkst_ir::IrComponent::ExplicitError(_)
        })
    ));
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
fn malformed_or_unsupported_log_arguments_never_emit_events() {
    let source_id = SourceId(1976);
    let sink = CollectingSink::default();

    let (_, duplicate) = evaluate_with_sink(".log {first} message:{second}", source_id, &sink);
    assert_eq!(duplicate.len(), 1, "{duplicate:?}");
    assert!(sink.events.borrow().is_empty());

    let source = ".var {values}\n    - alpha\n    - beta\n.log {.values::reversed}";
    let (_, unsupported) = evaluate_with_sink(source, source_id, &sink);
    assert_eq!(unsupported.len(), 1, "{unsupported:?}");
    assert_eq!(
        unsupported[0].primary.map(|span| span.source_id),
        Some(source_id)
    );
    assert!(unsupported[0].message.contains(".log"));
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
    let source =
        ".function {silent}\n    .debug {inside}\n.silent\n.var {x} {.silent}\n.x\n.silent::string";
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

#[test]
fn unit_logger_conversion_is_target_specific_and_does_not_widen_generic_string_consumers() {
    let source_id = SourceId(1987);
    let sink = CollectingSink::default();
    let source = ".var {u} {.debug {unit}}\n.uppercase {.u}\n.log {.u}";
    let (result, diagnostics) = evaluate_with_sink(source, source_id, &sink);

    assert!(paragraph_texts(&result).is_empty());
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(
        diagnostics[0].primary.map(|span| span.source_id),
        Some(source_id)
    );
    assert!(diagnostics[0].message.contains("adapt"), "{diagnostics:?}");

    let events = sink.events.borrow();
    assert_eq!(events.len(), 2, "{events:?}");
    assert_eq!(events[0].level, LogLevel::Debug);
    assert_eq!(events[0].message, "unit");
    assert_eq!(events[1].level, LogLevel::Log);
    assert_eq!(events[1].message, "kotlin.Unit");
}

#[test]
fn recursive_pair_logger_projection_matches_clean_room_evidence() {
    let source_id = SourceId(1992);
    let sink = CollectingSink::default();
    let source = concat!(
        ".log {.pair {.pair {.pair {.pair {a} {b}} {c}} {d}} {e}}\n",
        ".log {.pair {a} {.pair {b} {.pair {c} {.pair {d} {e}}}}}\n",
        ".debug {.pair {.pair {a} {b}} {.pair {c} {d}}}\n",
        ".error {.pair {.pair {.range {1} {3}} {.none}} {.pair {tail} {.range to:{2}}}}",
    );
    let (result, diagnostics) = evaluate_with_sink(source, source_id, &sink);

    let pair = |left: &str, right: &str| {
        format!(
            "[DynamicValue(unwrappedValue={left}, evaluationContext=null), DynamicValue(unwrappedValue={right}, evaluationContext=null)]"
        )
    };
    let ab = pair("a", "b");
    let abc = pair(&ab, "c");
    let abcd = pair(&abc, "d");
    let abcde = pair(&abcd, "e");

    let de = pair("d", "e");
    let cde = pair("c", &de);
    let bcde = pair("b", &cde);
    let right_deep = pair("a", &bcde);

    let balanced = pair(&ab, &pair("c", "d"));
    let mixed = pair(&pair("1..3", "None"), &pair("tail", "..2"));

    let events = sink.events.borrow();
    assert_eq!(
        events
            .iter()
            .map(|event| (event.level, event.message.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (LogLevel::Log, abcde.as_str()),
            (LogLevel::Log, right_deep.as_str()),
            (LogLevel::Debug, balanced.as_str()),
        ]
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E3011");
    assert!(diagnostics[0].message.contains(&mixed), "{diagnostics:?}");

    let [IrNode::Component {
        component: arkst_ir::IrComponent::ExplicitError(error),
    }] = result.nodes.as_slice()
    else {
        panic!("expected explicit error component, got {:?}", result.nodes);
    };
    assert_eq!(error.message, mixed);
}

#[test]
fn structured_logger_conversion_does_not_widen_generic_string_consumers() {
    let source_id = SourceId(1990);
    let sink = CollectingSink::default();
    let source = ".var {table}\n    .dictionary\n        - first: one\n        - second: 2\n.uppercase {.table}\n.log {.table}";
    let (result, diagnostics) = evaluate_with_sink(source, source_id, &sink);

    assert!(paragraph_texts(&result).is_empty());
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(
        diagnostics[0].primary.map(|span| span.source_id),
        Some(source_id)
    );
    assert!(diagnostics[0].message.contains("adapt"), "{diagnostics:?}");

    let events = sink.events.borrow();
    assert_eq!(events.len(), 1, "{events:?}");
    assert_eq!(events[0].level, LogLevel::Log);
    assert_eq!(
        events[0].message,
        "{first=DynamicValue(unwrappedValue=one, evaluationContext=null), second=DynamicValue(unwrappedValue=2, evaluationContext=null)}"
    );
}

#[test]
fn dictionary_logger_projection_rejects_unevidenced_rich_or_nested_entries() {
    let source_id = SourceId(1991);
    for source in [
        ".var {table}\n    .dictionary\n        - first: **one**\n.log {.table}",
        ".var {table}\n    .dictionary\n        - first: .whitespace\n.log {.table}",
        ".var {table}\n    .dictionary\n        - nested:\n            - child: value\n.log {.table}",
    ] {
        let sink = CollectingSink::default();
        let (_, diagnostics) = evaluate_with_sink(source, source_id, &sink);

        assert_eq!(diagnostics.len(), 1, "{source}: {diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E3001", "{source}: {diagnostics:?}");
        assert!(sink.events.borrow().is_empty(), "{source}");
    }
}

#[test]
fn repeated_unit_statements_collapse_to_empty_content_value() {
    let source_id = SourceId(1988);
    let source = ".function {multi}\n    .debug {a}\n    .debug {b}\n.multi\n.var {x} {.multi}\n.var {u} {.debug {single}}\n.isnone {.x}\n.equals {.x} to:{.none}\n.equals {.x} to:{.u}\n.equals {.x} to:{.x}\n.string {.x}";
    let (result, diagnostics) = evaluate_plain(source, source_id);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(
        paragraph_texts(&result),
        vec!["false", "false", "false", "true", ""]
    );
}
