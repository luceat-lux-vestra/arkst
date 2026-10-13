use arkst_core::ir::{IrComponent, IrInline, IrNode, IrTodoComponent};
use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};

fn compile_source(source: &str) -> arkst_core::CompileResult {
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("entry")
        .add_source("main.qd", source)
        .expect("source")
        .build()
        .expect("project");
    compile(&project, &CompileOptions::default())
}

fn todo(result: &arkst_core::CompileResult) -> &IrTodoComponent {
    let [IrNode::Component {
        component: IrComponent::Todo(component),
    }] = result.ir.nodes.as_slice()
    else {
        panic!("expected one Todo component, got {:?}", result.ir.nodes);
    };
    component
}

#[test]
fn todo_explicit_markdown_content_builds_typed_warning_box() {
    let result = compile_source(".todo {Hello, **world**!}\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let component = todo(&result);
    assert_eq!(component.title, "TO DO");
    let [IrNode::Paragraph { content, .. }] = component.children.as_slice() else {
        panic!("expected one paragraph child, got {:?}", component.children);
    };
    assert!(matches!(
        content.as_slice(),
        [
            IrInline::Text { content: prefix, .. },
            IrInline::Strong { .. },
            IrInline::Text { content: suffix, .. }
        ] if prefix == "Hello, " && suffix == "!"
    ));
    assert_eq!(component.span.start, 0);
}

#[test]
fn todo_indented_body_preserves_structured_markdown_children() {
    let result = compile_source(".todo\n    ## Later\n\n    Finish *this*.\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let component = todo(&result);
    assert_eq!(component.title, "TO DO");
    assert!(matches!(
        component.children.as_slice(),
        [IrNode::Heading { level: 2, .. }, IrNode::Paragraph { .. }]
    ));
    let IrNode::Paragraph { content, .. } = &component.children[1] else {
        unreachable!();
    };
    assert!(content
        .iter()
        .any(|inline| matches!(inline, IrInline::Emphasis { .. })));
}

#[test]
fn todo_default_and_english_fallback_are_bounded_but_non_english_fails_closed() {
    let default = compile_source(".todo {A}\n");
    assert!(default.diagnostics.is_empty(), "{default:?}");
    assert_eq!(todo(&default).title, "TO DO");

    let english = compile_source(".doclang {english}\n.todo {A}\n");
    assert!(english.diagnostics.is_empty(), "{english:?}");
    assert_eq!(todo(&english).title, "TO DO");

    let italian = compile_source(".doclang {italian}\n.todo {A}\n");
    assert_eq!(italian.diagnostics.len(), 1, "{italian:?}");
    assert!(italian.ir.nodes.is_empty(), "{italian:?}");
    assert!(italian.diagnostics[0]
        .message
        .contains("localized titles are not yet implemented"));
}

#[test]
fn todo_rejects_missing_or_inline_materialization_without_fabricated_output() {
    let missing = compile_source(".todo\n");
    assert_eq!(missing.diagnostics.len(), 1, "{missing:?}");
    assert!(missing.ir.nodes.is_empty(), "{missing:?}");

    let inline = compile_source("before .todo {A} after\n");
    assert_eq!(inline.diagnostics.len(), 1, "{inline:?}");
    let [IrNode::Paragraph { content, .. }] = inline.ir.nodes.as_slice() else {
        panic!("expected surrounding paragraph, got {:?}", inline.ir.nodes);
    };
    let text: String = content
        .iter()
        .filter_map(|inline| match inline {
            IrInline::Text { content, .. } => Some(content.as_str()),
            _ => None,
        })
        .collect();
    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert!(!text.contains("TO DO"));
}

#[test]
fn todo_component_roundtrips_through_document_wire_serde() {
    let result = compile_source(".todo {Ship it}\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let json = serde_json::to_string(&result.ir).expect("serialize Todo IR");
    let roundtrip: arkst_core::ir::IrDocument =
        serde_json::from_str(&json).expect("deserialize Todo IR");
    assert_eq!(roundtrip, result.ir);
}
