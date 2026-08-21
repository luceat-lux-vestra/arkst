use scribium_core::evaluator::Evaluator;
use scribium_core::ir::{IrComponent, IrContainerAlignment, IrNode, IrStackedLayout};
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

fn center(result: &scribium_core::CompileResult) -> &scribium_core::ir::IrContainerComponent {
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
fn center_constructs_typed_container() {
    let source = ".center\n    Hello\n\n    ## World\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let component = center(&result);
    assert_eq!(component.width, None);
    assert_eq!(component.height, None);
    assert!(component.full_width);
    assert_eq!(component.alignment, Some(IrContainerAlignment::Center));
    assert_eq!(component.children.len(), 2);
    assert_eq!(paragraph_text(&component.children[0]), "Hello");
    assert!(matches!(component.children[1], IrNode::Heading { .. }));
}

#[test]
fn center_requires_block_body() {
    let result = compile_source(".center\n");
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert!(result.ir.nodes.is_empty());
}

#[test]
fn center_rejects_positional_arguments() {
    let result = compile_source(".center {foo}\n    A\n");
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert!(result.ir.nodes.is_empty());
}

#[test]
fn center_rejects_named_arguments() {
    let result = compile_source(".center foo:{bar}\n    A\n");
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert!(result.ir.nodes.is_empty());
}

#[test]
fn center_rejects_lambda_parameters() {
    let span = scribium_core::SourceSpan::new(scribium_core::SourceId(1), 0, 7);
    let document = scribium_core::ir::IrDocument {
        nodes: vec![IrNode::FunctionCall {
            name: "center".to_string(),
            positional_args: Vec::new(),
            named_args: Vec::new(),
            lambda_parameters: Some(vec![scribium_core::ir::IrParameter {
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
fn custom_center_shadows_native_center() {
    let result = compile_source(".function {center}\n    custom\n\n.center\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(result.ir.nodes.len(), 1);
    assert_eq!(paragraph_text(&result.ir.nodes[0]), "custom");
}

#[test]
fn center_body_failure_is_atomic() {
    let result = compile_source(".center\n    A\n\n    .grid columns:{0}\n        B\n\n    C\n");
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert!(result.ir.nodes.is_empty(), "{result:?}");
}

#[test]
fn center_preserves_child_provenance() {
    let source = ".center\n    Hello\n\n    ## World\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let component = center(&result);
    assert_eq!(component.span.start, 0);
    assert_eq!(component.span.end, source.len() - 1);
    let IrNode::Paragraph {
        span: hello_span, ..
    } = &component.children[0]
    else {
        panic!("expected paragraph child");
    };
    assert_eq!(hello_span.start, source.find("Hello").unwrap());
    assert_eq!(hello_span.end, source.find("Hello").unwrap() + 5);
    let IrNode::Heading {
        span: heading_span, ..
    } = &component.children[1]
    else {
        panic!("expected heading child");
    };
    assert_eq!(heading_span.start, source.find("##").unwrap());
}

#[test]
fn center_nested_component_is_structured() {
    let result = compile_source(".center\n    .center\n        A\n\n    B\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let outer = center(&result);
    assert_eq!(outer.children.len(), 2);
    let IrNode::Component {
        component: IrComponent::Container(inner),
    } = &outer.children[0]
    else {
        panic!("expected nested container, got {:?}", outer.children[0]);
    };
    assert_eq!(inner.alignment, Some(IrContainerAlignment::Center));
    assert_eq!(paragraph_text(&inner.children[0]), "A");
    assert_eq!(paragraph_text(&outer.children[1]), "B");
}

#[test]
fn center_inside_stacked_is_structured() {
    let result = compile_source(".row\n    .center\n        A\n\n    B\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let [IrNode::Component {
        component: IrComponent::Stacked(row),
    }] = result.ir.nodes.as_slice()
    else {
        panic!("expected row component, got {:?}", result.ir.nodes);
    };
    assert_eq!(row.layout, IrStackedLayout::Row);
    assert!(matches!(
        row.children[0],
        IrNode::Component {
            component: IrComponent::Container(_)
        }
    ));
    assert_eq!(paragraph_text(&row.children[1]), "B");
}

#[test]
fn stacked_inside_center_is_structured() {
    let result = compile_source(".center\n    .row\n        A\n\n        B\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let outer = center(&result);
    let [IrNode::Component {
        component: IrComponent::Stacked(row),
    }] = outer.children.as_slice()
    else {
        panic!("expected nested row component, got {:?}", outer.children);
    };
    assert_eq!(row.layout, IrStackedLayout::Row);
    assert_eq!(paragraph_text(&row.children[0]), "A");
    assert_eq!(paragraph_text(&row.children[1]), "B");
}

#[test]
fn center_callable_result_preserves_component_value() {
    let result = compile_source(".function {wrapper}\n    .center\n        A\n\n.wrapper\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let component = center(&result);
    assert_eq!(paragraph_text(&component.children[0]), "A");
}

#[test]
fn center_component_composes_with_following_output() {
    let result =
        compile_source(".function {wrapper}\n    .center\n        A\n\n    B\n\n.wrapper\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(result.ir.nodes.len(), 2);
    assert!(matches!(
        result.ir.nodes[0],
        IrNode::Component {
            component: IrComponent::Container(_)
        }
    ));
    assert_eq!(paragraph_text(&result.ir.nodes[1]), "B");
}

#[test]
fn inline_center_fails_closed_without_fabricated_output() {
    let result = compile_source("prefix .center suffix\n");
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    let [IrNode::Paragraph { content, .. }] = result.ir.nodes.as_slice() else {
        panic!("expected surrounding paragraph, got {:?}", result.ir.nodes);
    };
    assert!(content
        .iter()
        .all(|inline| matches!(inline, scribium_core::ir::IrInline::Text { .. })));
    assert!(!format!("{content:?}").contains("center"));
}

#[test]
fn inline_custom_center_shadows_native_center() {
    let result = compile_source(".function {center}\n    custom\n\nprefix .center suffix\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let [IrNode::Paragraph { content, .. }] = result.ir.nodes.as_slice() else {
        panic!("expected surrounding paragraph, got {:?}", result.ir.nodes);
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
fn inline_callable_container_fails_closed_without_flattening() {
    let result =
        compile_source(".function {wrapper}\n    .center\n        A\n\nprefix .wrapper suffix\n");
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert!(result.diagnostics[0]
        .message
        .contains("Semantic component is block-only"));
    assert!(!result.diagnostics[0].message.contains("Stacked layout"));
    let [IrNode::Paragraph { content, .. }] = result.ir.nodes.as_slice() else {
        panic!("expected surrounding paragraph, got {:?}", result.ir.nodes);
    };
    let text: String = content
        .iter()
        .filter_map(|inline| match inline {
            scribium_core::ir::IrInline::Text { content, .. } => Some(content.as_str()),
            _ => None,
        })
        .collect();
    assert!(text.contains("prefix"));
    assert!(text.contains("suffix"));
    assert!(!text.contains('A'));
    assert!(!text.contains("wrapper"));
}
