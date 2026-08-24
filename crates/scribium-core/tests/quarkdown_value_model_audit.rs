use scribium_core::ir::{IrInline, IrNode};
use scribium_core::{compile, CompileOptions, SourceId, VirtualProjectBuilder};

fn compile_source(source: &str) -> (scribium_core::CompileResult, SourceId) {
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
    (compile(&project, &CompileOptions::default()), source_id)
}

fn output_text(result: &scribium_core::CompileResult) -> String {
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
fn audit_dynamic_text_converts_but_static_text_stays_text() {
    let dynamic = ".var {number-text} {.string {-3.5}}\n.abs {.number-text}\n";
    let (result, _) = compile_source(dynamic);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(output_text(&result), "3.5");

    let static_text = ".abs {.string {-3.5}}\n";
    let (result, source_id) = compile_source(static_text);
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert_eq!(result.diagnostics[0].code, "E3001");
    assert_eq!(
        result.diagnostics[0].primary.map(|span| span.source_id),
        Some(source_id)
    );
    assert!(result.ir.nodes.is_empty(), "{result:?}");
}

#[test]
fn audit_ordered_dictionary_replaces_duplicate_values_in_the_first_slot() {
    let source = ".dictionary\n    - first: one\n    - second: two\n    - first: replaced\n";
    let (result, source_id) = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");

    let [IrNode::Table { rows, span, .. }] = result.ir.nodes.as_slice() else {
        panic!("expected dictionary table, got {:?}", result.ir.nodes);
    };
    assert_eq!(span.source_id, source_id);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].cells[0].content.len(), 1);
    assert_eq!(rows[0].cells[1].content.len(), 1);
    assert!(matches!(
        rows[0].cells[0].content[0],
        IrInline::Text { ref content, .. } if content == "first"
    ));
    assert!(matches!(
        rows[0].cells[1].content[0],
        IrInline::Text { ref content, .. } if content == "replaced"
    ));
    assert!(matches!(
        rows[1].cells[0].content[0],
        IrInline::Text { ref content, .. } if content == "second"
    ));
}

#[test]
fn audit_named_and_optional_binding_preserves_missing_none_and_named_slots() {
    let source = ".function {greet}\n    to from?:\n    Hello, .to from .from!\n\n.greet {world}\n.greet {world} from:{Jane}\n";
    let (result, _) = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(
        output_text(&result),
        "Hello, world from None!\nHello, world from Jane!"
    );
}

#[test]
fn audit_nested_candidate_conversion_is_before_state_commit_and_rolls_back() {
    let source = ".doclang {en}\n.doclang {.pair {.doclang {it}} {invalid}}\n.doclang\n";
    let (result, source_id) = compile_source(source);
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert_eq!(result.diagnostics[0].code, "E3001");
    assert_eq!(
        result.diagnostics[0].primary.map(|span| span.source_id),
        Some(source_id)
    );
    assert_eq!(output_text(&result), "English");
    assert_eq!(
        result
            .ir
            .metadata
            .document_state
            .locale
            .as_ref()
            .map(|locale| locale.tag.as_str()),
        Some("en")
    );
}
