use scribium_core::evaluator::Evaluator;
use scribium_core::ir::{
    IrComponent, IrContainerAlignment, IrDocument, IrInline, IrNode, IrParameter, IrSize,
    IrSizeUnit, IrStackedLayout, IrValue,
};
use scribium_core::{compile, CompileOptions, SourceId, SourceSpan, VirtualProjectBuilder};

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

fn container(result: &scribium_core::CompileResult) -> &scribium_core::ir::IrContainerComponent {
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

fn paragraph_text(node: &IrNode) -> String {
    let IrNode::Paragraph { content, .. } = node else {
        panic!("expected paragraph, got {node:?}");
    };
    content
        .iter()
        .map(|inline| match inline {
            IrInline::Text { content, .. } => content.as_str(),
            other => panic!("expected text fragment, got {other:?}"),
        })
        .collect()
}

fn assert_one_failure(result: &scribium_core::CompileResult, message: &str) {
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert!(result.ir.nodes.is_empty(), "{result:?}");
    assert!(
        result.diagnostics[0].message.contains(message),
        "expected {message:?}, got {:?}",
        result.diagnostics[0]
    );
}

#[test]
fn container_empty_constructs_unaligned_container() {
    let result = compile_source(".container\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let component = container(&result);
    assert_eq!(component.width, None);
    assert_eq!(component.height, None);
    assert!(!component.full_width);
    assert_eq!(component.alignment, None);
    assert!(component.children.is_empty());
}

#[test]
fn container_body_only_preserves_structured_children() {
    let result = compile_source(".container\n    ## Left\n    Text left\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let component = container(&result);
    assert_eq!(component.children.len(), 2);
    assert!(matches!(
        component.children[0],
        IrNode::Heading { level: 2, .. }
    ));
    assert_eq!(paragraph_text(&component.children[1]), "Text left");
}

#[test]
fn container_accepts_bounded_sizing_and_boolean_values() {
    let width = compile_source(".container width:{4cm}\n    A\n");
    assert!(width.diagnostics.is_empty(), "{width:?}");
    assert_eq!(
        container(&width).width,
        Some(IrSize {
            value: 4.0,
            unit: IrSizeUnit::Cm,
        })
    );

    let height = compile_source(".container height:{2cm}\n    A\n");
    assert!(height.diagnostics.is_empty(), "{height:?}");
    assert_eq!(
        container(&height).height,
        Some(IrSize {
            value: 2.0,
            unit: IrSizeUnit::Cm,
        })
    );

    let both = compile_source(".container width:{4cm} height:{2cm}\n    A\n");
    assert!(both.diagnostics.is_empty(), "{both:?}");
    assert_eq!(
        container(&both).width.as_ref().map(|size| size.value),
        Some(4.0)
    );
    assert_eq!(
        container(&both).height.as_ref().map(|size| size.value),
        Some(2.0)
    );

    let yes = compile_source(".container fullwidth:{yes}\n    A\n");
    assert!(yes.diagnostics.is_empty(), "{yes:?}");
    assert!(container(&yes).full_width);

    let no = compile_source(".container fullwidth:{no}\n    A\n");
    assert!(no.diagnostics.is_empty(), "{no:?}");
    assert!(!container(&no).full_width);
}

#[test]
fn container_accepts_positional_prefix_and_preserves_typed_size_identity() {
    let positional = compile_source(".container {4cm} {2cm} {yes}\n    A\n");
    assert!(positional.diagnostics.is_empty(), "{positional:?}");
    let component = container(&positional);
    assert_eq!(component.width.as_ref().map(|size| size.value), Some(4.0));
    assert_eq!(component.height.as_ref().map(|size| size.value), Some(2.0));
    assert!(component.full_width);

    let span = SourceSpan::new(SourceId(1), 0, 1);
    let document = IrDocument {
        nodes: vec![IrNode::FunctionCall {
            name: "container".to_string(),
            positional_args: vec![IrValue::Size(IrSize {
                value: 12.0,
                unit: IrSizeUnit::Pt,
            })],
            named_args: Vec::new(),
            ordered_args: None,
            lambda_parameters: None,
            body: Some(vec![IrNode::Paragraph {
                content: vec![IrInline::Text {
                    content: "A".to_string(),
                    span,
                }],
                span,
            }]),
            raw_body: None,
            span,
        }],
        metadata: scribium_core::ir::IrMetadata::default(),
    };
    let (evaluated, diagnostics) = Evaluator::new().evaluate(&document);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let [IrNode::Component {
        component: IrComponent::Container(component),
    }] = evaluated.nodes.as_slice()
    else {
        panic!("expected typed container, got {:?}", evaluated.nodes);
    };
    assert_eq!(component.width.as_ref().map(|size| size.value), Some(12.0));
    assert_eq!(
        component.width.as_ref().map(|size| size.unit),
        Some(IrSizeUnit::Pt)
    );
}

#[test]
fn container_origin_aware_conversion_rejects_static_text() {
    let size = compile_source(".container width:{.string {4cm}}\n    A\n");
    assert_one_failure(&size, "parameter `width`");

    let boolean = compile_source(".container fullwidth:{.string {yes}}\n    A\n");
    assert_one_failure(&boolean, "parameter `fullwidth`");
}

#[test]
fn container_rejects_lambda_body() {
    let span = SourceSpan::new(SourceId(1), 0, 7);
    let document = IrDocument {
        nodes: vec![IrNode::FunctionCall {
            name: "container".to_string(),
            positional_args: Vec::new(),
            named_args: Vec::new(),
            ordered_args: None,
            lambda_parameters: Some(vec![IrParameter {
                name: "value".to_string(),
                name_span: span,
                span,
                optional: false,
            }]),
            body: Some(vec![IrNode::Paragraph {
                content: vec![IrInline::Text {
                    content: "A".to_string(),
                    span,
                }],
                span,
            }]),
            raw_body: None,
            span,
        }],
        metadata: scribium_core::ir::IrMetadata::default(),
    };
    let (evaluated, diagnostics) = Evaluator::new().evaluate(&document);
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert!(diagnostics[0]
        .message
        .contains("body is a Markdown block, not a lambda"));
    assert!(evaluated.nodes.is_empty());
}

#[test]
fn container_rejects_duplicate_unknown_and_out_of_signature_arguments() {
    assert_one_failure(
        &compile_source(".container width:{4cm} width:{2cm}\n    A\n"),
        "bound more than once",
    );
    assert_one_failure(
        &compile_source(".container wat:{4cm}\n    A\n"),
        "Unknown named argument `wat`",
    );
    assert_one_failure(
        &compile_source(".container {1cm} {2cm} {yes} {4cm}\n    A\n"),
        "at most three positional arguments",
    );
    assert_one_failure(
        &compile_source(".container {4cm} width:{2cm}\n    A\n"),
        "bound more than once",
    );
}

#[test]
fn container_rejects_deferred_known_parameters_explicitly() {
    for parameter in [
        "float",
        "fullspan",
        "classname",
        "padding",
        "foreground",
        "textalignment",
    ] {
        let source = format!(".container {parameter}:{{value}}\n    A\n");
        assert_one_failure(
            &compile_source(&source),
            &format!(
                "parameter `{parameter}` is not supported by the bounded container sizing slice"
            ),
        );
    }
}

#[test]
fn container_argument_failure_does_not_evaluate_body() {
    let width = compile_source(".container width:{INVALID}\n    .grid columns:{0}\n        A\n");
    assert_one_failure(&width, "parameter `width`");
    assert!(!width.diagnostics[0].message.contains("columns"));

    let full_width =
        compile_source(".container fullwidth:{maybe}\n    .grid columns:{0}\n        A\n");
    assert_one_failure(&full_width, "parameter `fullwidth`");
    assert!(!full_width.diagnostics[0].message.contains("columns"));
}

#[test]
fn custom_container_shadows_native_container_in_block_and_inline_contexts() {
    let block = compile_source(".function {container}\n    custom\n\n.container\n");
    assert!(block.diagnostics.is_empty(), "{block:?}");
    assert_eq!(paragraph_text(&block.ir.nodes[0]), "custom");

    let inline = compile_source(".function {container}\n    custom\n\nprefix .container suffix\n");
    assert!(inline.diagnostics.is_empty(), "{inline:?}");
    let [IrNode::Paragraph { content, .. }] = inline.ir.nodes.as_slice() else {
        panic!("expected paragraph, got {:?}", inline.ir.nodes);
    };
    let text: String = content
        .iter()
        .filter_map(|inline| match inline {
            IrInline::Text { content, .. } => Some(content.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(text, "prefix custom suffix");
}

#[test]
fn native_inline_container_fails_closed_without_fabricated_text() {
    let result = compile_source("prefix .container suffix\n");
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert!(result.diagnostics[0]
        .message
        .contains("`.container` is block-only"));
    let [IrNode::Paragraph { content, .. }] = result.ir.nodes.as_slice() else {
        panic!("expected surrounding paragraph, got {:?}", result.ir.nodes);
    };
    assert!(content
        .iter()
        .all(|inline| matches!(inline, IrInline::Text { .. })));
    assert!(!format!("{content:?}").contains("container"));
}

#[test]
fn nested_and_composed_containers_remain_structured() {
    let nested = compile_source(".container\n    .container\n        A\n");
    assert!(nested.diagnostics.is_empty(), "{nested:?}");
    let outer = container(&nested);
    assert!(matches!(
        outer.children.as_slice(),
        [IrNode::Component {
            component: IrComponent::Container(_)
        }]
    ));

    let row = compile_source(".row\n    .container\n        A\n\n    .container\n        B\n");
    assert!(row.diagnostics.is_empty(), "{row:?}");
    let [IrNode::Component {
        component: IrComponent::Stacked(row),
    }] = row.ir.nodes.as_slice()
    else {
        panic!("expected row, got {:?}", row.ir.nodes);
    };
    assert_eq!(row.layout, IrStackedLayout::Row);
    assert_eq!(row.children.len(), 2);
    assert!(row.children.iter().all(|child| matches!(
        child,
        IrNode::Component {
            component: IrComponent::Container(_)
        }
    )));

    let composed = compile_source(".container\n    .align {end}\n        A\n");
    assert!(composed.diagnostics.is_empty(), "{composed:?}");
    let outer = container(&composed);
    let [IrNode::Component {
        component: IrComponent::Container(inner),
    }] = outer.children.as_slice()
    else {
        panic!("expected aligned container, got {:?}", outer.children);
    };
    assert_eq!(inner.alignment, Some(IrContainerAlignment::End));

    let container_row = compile_source(".container\n    .row\n        A\n\n        B\n");
    assert!(container_row.diagnostics.is_empty(), "{container_row:?}");
    assert!(matches!(
        container(&container_row).children.as_slice(),
        [IrNode::Component {
            component: IrComponent::Stacked(_)
        }]
    ));

    let center_container = compile_source(".center\n    .container\n        A\n");
    assert!(
        center_container.diagnostics.is_empty(),
        "{center_container:?}"
    );
    let centered = {
        let [IrNode::Component {
            component: IrComponent::Container(component),
        }] = center_container.ir.nodes.as_slice()
        else {
            panic!(
                "expected centered container, got {:?}",
                center_container.ir.nodes
            );
        };
        component
    };
    assert_eq!(centered.alignment, Some(IrContainerAlignment::Center));
    assert!(matches!(
        centered.children.as_slice(),
        [IrNode::Component {
            component: IrComponent::Container(_)
        }]
    ));

    let align_container = compile_source(".align {center}\n    .container\n        A\n");
    assert!(
        align_container.diagnostics.is_empty(),
        "{align_container:?}"
    );
    let IrNode::Component {
        component: IrComponent::Container(align),
    } = &align_container.ir.nodes[0]
    else {
        panic!(
            "expected aligned container, got {:?}",
            align_container.ir.nodes
        );
    };
    assert_eq!(align.alignment, Some(IrContainerAlignment::Center));
    assert!(matches!(
        align.children.as_slice(),
        [IrNode::Component {
            component: IrComponent::Container(_)
        }]
    ));
}

#[test]
fn container_groups_row_children_without_flattening() {
    let result = compile_source(
        ".row\n    .container\n        ## Left\n        Text left\n\n    .container\n        ## Right\n        Text right\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let [IrNode::Component {
        component: IrComponent::Stacked(row),
    }] = result.ir.nodes.as_slice()
    else {
        panic!("expected row, got {:?}", result.ir.nodes);
    };
    assert_eq!(row.children.len(), 2);
    for (child, heading, paragraph) in [
        (&row.children[0], "Left", "Text left"),
        (&row.children[1], "Right", "Text right"),
    ] {
        let IrNode::Component {
            component: IrComponent::Container(container),
        } = child
        else {
            panic!("expected container child, got {child:?}");
        };
        assert_eq!(container.children.len(), 2);
        assert!(matches!(container.children[0], IrNode::Heading { .. }));
        assert_eq!(paragraph_text(&container.children[1]), paragraph);
        assert!(format!("{:?}", container.children[0]).contains(heading));
    }
}

#[test]
fn container_callable_result_preserves_component_value_and_provenance() {
    let source = ".function {wrapper}\n    .container width:{4cm}\n        A\n\n.wrapper\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let component = container(&result);
    assert_eq!(component.width.as_ref().map(|size| size.value), Some(4.0));
    assert_eq!(paragraph_text(&component.children[0]), "A");
    assert_eq!(
        component.span.start,
        source.find(".container").expect("container call")
    );
    let IrNode::Paragraph { span, .. } = &component.children[0] else {
        panic!("expected paragraph child");
    };
    assert_eq!(span.start, source.find("A").expect("child text"));
    assert!(span.end <= component.span.end);
}
