use scribium_core::evaluator::Evaluator;
use scribium_core::ir::{IrComponent, IrContainerAlignment, IrNode, IrParameter, IrStackedLayout};
use scribium_core::{compile, CompileOptions, VirtualProjectBuilder};

fn compile_source(source: &str) -> scribium_core::CompileResult {
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("valid entry")
        .add_source("main.qd", source)
        .expect("valid source")
        .build()
        .expect("valid project");
    compile(&project, &CompileOptions::default())
}

fn align(result: &scribium_core::CompileResult) -> &scribium_core::ir::IrContainerComponent {
    let [IrNode::Component {
        component: IrComponent::Container(component),
    }] = result.ir.nodes.as_slice()
    else {
        panic!(
            "expected one container component, got {:?}",
            result.ir.nodes
        );
    };
    component
}

fn paragraph_text(node: &IrNode) -> &str {
    let IrNode::Paragraph { content, .. } = node else {
        panic!("expected paragraph, got {node:?}");
    };
    let [scribium_core::ir::IrInline::Text { content, .. }] = content.as_slice() else {
        panic!("expected one text fragment, got {content:?}");
    };
    content
}

#[test]
fn align_start_center_and_end_construct_typed_containers() {
    for (name, expected) in [
        ("start", IrContainerAlignment::Start),
        ("center", IrContainerAlignment::Center),
        ("end", IrContainerAlignment::End),
    ] {
        let result = compile_source(&format!(".align {{{name}}}\n    A\n"));
        assert!(result.diagnostics.is_empty(), "{name}: {result:?}");
        let component = align(&result);
        assert!(component.full_width);
        assert_eq!(component.alignment, Some(expected));
        assert_eq!(paragraph_text(&component.children[0]), "A");
    }
}

#[test]
fn align_accepts_named_alignment() {
    let result = compile_source(".align alignment:{end}\n    A\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(align(&result).alignment, Some(IrContainerAlignment::End));
}

#[test]
fn align_rejects_invalid_bindings_and_missing_body() {
    for source in [
        ".align\n    A\n",
        ".align {center} alignment:{end}\n    A\n",
        ".align {center} {end}\n    A\n",
        ".align foo:{center}\n    A\n",
        ".align {center}\n",
    ] {
        let result = compile_source(source);
        assert_eq!(result.diagnostics.len(), 1, "{source:?}: {result:?}");
        assert!(result.ir.nodes.is_empty(), "{source:?}: {result:?}");
    }
}

#[test]
fn align_rejects_lambda_body() {
    let span = scribium_core::SourceSpan::new(scribium_core::SourceId(1), 0, 7);
    let document = scribium_core::ir::IrDocument {
        nodes: vec![IrNode::FunctionCall {
            name: "align".to_string(),
            positional_args: vec![scribium_core::ir::IrValue::Identifier("center".to_string())],
            named_args: Vec::new(),
            ordered_args: None,
            lambda_parameters: Some(vec![IrParameter {
                name: "x".to_string(),
                name_span: span,
                span,
                optional: false,
            }]),
            body: Some(vec![IrNode::Paragraph {
                content: vec![scribium_core::ir::IrInline::Text {
                    content: "A".to_string(),
                    span,
                }],
                span,
            }]),
            span,
        }],
        metadata: scribium_core::ir::IrMetadata::default(),
    };
    let (evaluated, diagnostics) = Evaluator::new().evaluate(&document);
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert!(evaluated.nodes.is_empty());
}

#[test]
fn align_conversion_is_origin_aware_and_body_failure_is_lazy() {
    let static_text = compile_source(".align {.string {center}}\n    A\n");
    assert_eq!(static_text.diagnostics.len(), 1, "{static_text:?}");
    assert!(static_text.ir.nodes.is_empty());

    let invalid = compile_source(".align {INVALID}\n    .grid columns:{0}\n        A\n");
    assert_eq!(invalid.diagnostics.len(), 1, "{invalid:?}");
    assert!(invalid.ir.nodes.is_empty());
    assert!(!invalid.diagnostics[0].message.contains("Column count"));
}

#[test]
fn custom_align_shadows_native_align_in_block_and_inline_contexts() {
    let block = compile_source(".function {align}\n    custom\n\n.align\n");
    assert!(block.diagnostics.is_empty(), "{block:?}");
    assert_eq!(paragraph_text(&block.ir.nodes[0]), "custom");

    let inline = compile_source(".function {align}\n    custom\n\nprefix .align suffix\n");
    assert!(inline.diagnostics.is_empty(), "{inline:?}");
    let [IrNode::Paragraph { content, .. }] = inline.ir.nodes.as_slice() else {
        panic!("expected paragraph, got {:?}", inline.ir.nodes);
    };
    let text: String = content
        .iter()
        .filter_map(|inline| match inline {
            scribium_core::ir::IrInline::Text { content, .. } => Some(content.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(text, "prefix custom suffix");
}

#[test]
fn native_inline_align_fails_closed_without_fabricated_text() {
    let result = compile_source("prefix .align {center} suffix\n");
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    let [IrNode::Paragraph { content, .. }] = result.ir.nodes.as_slice() else {
        panic!("expected paragraph, got {:?}", result.ir.nodes);
    };
    assert!(content
        .iter()
        .all(|inline| matches!(inline, scribium_core::ir::IrInline::Text { .. })));
    assert!(!format!("{content:?}").contains("align"));
}

#[test]
fn align_composes_with_nested_and_stacked_components() {
    let nested = compile_source(".align {end}\n    .align {start}\n        A\n");
    assert!(nested.diagnostics.is_empty(), "{nested:?}");
    let outer = align(&nested);
    assert_eq!(outer.alignment, Some(IrContainerAlignment::End));
    let [IrNode::Component {
        component: IrComponent::Container(inner),
    }] = outer.children.as_slice()
    else {
        panic!("expected nested align, got {:?}", outer.children);
    };
    assert_eq!(inner.alignment, Some(IrContainerAlignment::Start));

    let align_row = compile_source(".align {center}\n    .row\n        A\n\n        B\n");
    assert!(align_row.diagnostics.is_empty(), "{align_row:?}");
    let outer = align(&align_row);
    let [IrNode::Component {
        component: IrComponent::Stacked(row),
    }] = outer.children.as_slice()
    else {
        panic!("expected row inside align, got {:?}", outer.children);
    };
    assert_eq!(row.layout, IrStackedLayout::Row);

    let row_align = compile_source(".row\n    .align {start}\n        A\n\n    B\n");
    assert!(row_align.diagnostics.is_empty(), "{row_align:?}");
    let [IrNode::Component {
        component: IrComponent::Stacked(row),
    }] = row_align.ir.nodes.as_slice()
    else {
        panic!("expected row, got {:?}", row_align.ir.nodes);
    };
    assert!(matches!(
        row.children[0],
        IrNode::Component {
            component: IrComponent::Container(_)
        }
    ));
}

#[test]
fn center_and_align_share_container_semantics() {
    let result = compile_source(".align {center}\n    .center\n        A\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let outer = align(&result);
    let [IrNode::Component {
        component: IrComponent::Container(inner),
    }] = outer.children.as_slice()
    else {
        panic!("expected nested center, got {:?}", outer.children);
    };
    assert_eq!(outer.alignment, Some(IrContainerAlignment::Center));
    assert_eq!(inner.alignment, Some(IrContainerAlignment::Center));
}

#[test]
fn align_callable_result_preserves_component_value() {
    let result = compile_source(".function {wrapper}\n    .align {end}\n        A\n\n.wrapper\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let component = align(&result);
    assert_eq!(component.alignment, Some(IrContainerAlignment::End));
    assert_eq!(paragraph_text(&component.children[0]), "A");
}

#[test]
fn align_preserves_call_and_child_provenance() {
    let source = ".align {start}\n    Hello\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let component = align(&result);
    assert_eq!(component.span.start, 0);
    assert_eq!(component.span.end, source.len() - 1);
    let IrNode::Paragraph { span, .. } = &component.children[0] else {
        panic!("expected paragraph child");
    };
    let start = source.find("Hello").expect("child text");
    assert_eq!(span.start, start);
    assert_eq!(span.end, start + "Hello".len());
}
