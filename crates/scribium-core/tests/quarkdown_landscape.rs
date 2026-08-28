use scribium_core::evaluator::Evaluator;
use scribium_core::ir::{
    IrComponent, IrContainerAlignment, IrInline, IrLandscapeComponent, IrNode, IrParameter,
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

fn landscape(result: &scribium_core::CompileResult) -> &IrLandscapeComponent {
    let [IrNode::Component {
        component: IrComponent::Landscape(component),
    }] = result.ir.nodes.as_slice()
    else {
        panic!(
            "expected one landscape component, got {:?}",
            result.ir.nodes
        );
    };
    component
}

fn paragraph_text(node: &IrNode) -> &str {
    let IrNode::Paragraph { content, .. } = node else {
        panic!("expected paragraph, got {node:?}");
    };
    let [IrInline::Text { content, .. }] = content.as_slice() else {
        panic!("expected one text fragment, got {content:?}");
    };
    content
}

fn assert_one_failure(result: &scribium_core::CompileResult) {
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert!(result.ir.nodes.is_empty(), "{result:?}");
}

#[test]
fn landscape_constructs_typed_component_and_preserves_structured_children() {
    let source =
        ".landscape\n    ## Heading\n\n    Paragraph\n\n    .row\n        A\n\n        B\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let component = landscape(&result);
    assert_eq!(component.children.len(), 3);
    assert!(matches!(
        component.children[0],
        IrNode::Heading { level: 2, .. }
    ));
    assert_eq!(paragraph_text(&component.children[1]), "Paragraph");
    assert!(matches!(
        component.children[2],
        IrNode::Component {
            component: IrComponent::Stacked(_)
        }
    ));
}

#[test]
fn landscape_requires_body_and_rejects_all_non_body_arguments() {
    for source in [
        ".landscape\n",
        ".landscape {foo}\n    A\n",
        ".landscape foo:{bar}\n    A\n",
        ".landscape {foo} bar:{baz}\n    A\n",
    ] {
        let result = compile_source(source);
        assert_one_failure(&result);
        assert!(result.diagnostics[0].message.contains("landscape"));
    }
}

#[test]
fn landscape_argument_validation_is_lazy_and_atomic() {
    let result = compile_source(".landscape {foo}\n    .grid columns:{0}\n        A\n");
    assert_one_failure(&result);
    assert!(!result.diagnostics[0].message.contains("Column count"));
}

#[test]
fn landscape_rejects_lambda_body_without_evaluating_it() {
    let span = SourceSpan::new(SourceId(1), 0, 10);
    let document = scribium_core::ir::IrDocument {
        nodes: vec![IrNode::FunctionCall {
            name: "landscape".to_string(),
            positional_args: Vec::new(),
            named_args: Vec::new(),
            ordered_args: None,
            lambda_parameters: Some(vec![IrParameter {
                name: "value".to_string(),
                name_span: span,
                span,
                optional: false,
            }]),
            body: Some(vec![IrNode::FunctionCall {
                name: "grid".to_string(),
                positional_args: Vec::new(),
                named_args: vec![scribium_core::ir::IrNamedArg {
                    name: "columns".to_string(),
                    name_span: span,
                    value: scribium_core::ir::IrValue::Number(0.0),
                    span,
                }],
                ordered_args: None,
                lambda_parameters: None,
                body: None,
                span,
            }]),
            span,
        }],
        metadata: scribium_core::ir::IrMetadata::default(),
    };
    let (evaluated, diagnostics) = Evaluator::new().evaluate(&document);
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert!(diagnostics[0].message.contains("not a lambda"));
    assert!(evaluated.nodes.is_empty());
}

#[test]
fn custom_landscape_shadows_native_landscape_in_block_and_inline_contexts() {
    let block = compile_source(".function {landscape}\n    custom\n\n.landscape\n");
    assert!(block.diagnostics.is_empty(), "{block:?}");
    assert_eq!(paragraph_text(&block.ir.nodes[0]), "custom");

    let inline = compile_source(".function {landscape}\n    custom\n\nprefix .landscape suffix\n");
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
fn native_inline_landscape_fails_closed_and_keeps_surrounding_text() {
    let result = compile_source("prefix .landscape suffix\n");
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    let [IrNode::Paragraph { content, .. }] = result.ir.nodes.as_slice() else {
        panic!("expected surrounding paragraph, got {:?}", result.ir.nodes);
    };
    assert!(content
        .iter()
        .all(|inline| matches!(inline, IrInline::Text { .. })));
    let text: String = content
        .iter()
        .filter_map(|inline| match inline {
            IrInline::Text { content, .. } => Some(content.as_str()),
            _ => None,
        })
        .collect();
    assert!(text.contains("prefix"));
    assert!(text.contains("suffix"));
    assert!(!text.contains("landscape"));
}

#[test]
fn landscape_callable_result_preserves_component_value() {
    let result = compile_source(".function {wide}\n    .landscape\n        A\n\n.wide\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(paragraph_text(&landscape(&result).children[0]), "A");
}

#[test]
fn landscape_composes_with_container_stacked_align_and_nested_landscape() {
    let cases = [
        (
            ".landscape\n    .container width:{4cm}\n        A\n",
            "container",
        ),
        (".container\n    .landscape\n        A\n", "landscape"),
        (".landscape\n    .row\n        A\n\n        B\n", "stacked"),
        (".row\n    .landscape\n        A\n\n    B\n", "stacked"),
        (".align {center}\n    .landscape\n        A\n", "align"),
        (".landscape\n    .align {end}\n        A\n", "align"),
    ];
    for (source, expected) in cases {
        let result = compile_source(source);
        assert!(result.diagnostics.is_empty(), "{expected}: {result:?}");
        assert!(matches!(result.ir.nodes[0], IrNode::Component { .. }));
    }

    let result = compile_source(".landscape\n    .landscape\n        A\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let outer = landscape(&result);
    assert!(matches!(
        outer.children.as_slice(),
        [IrNode::Component {
            component: IrComponent::Landscape(_)
        }]
    ));
}

#[test]
fn landscape_preserves_call_and_child_provenance() {
    let source = ".landscape\n    Hello\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let component = landscape(&result);
    assert_eq!(component.span.start, 0);
    let IrNode::Paragraph { span, .. } = &component.children[0] else {
        panic!("expected paragraph child");
    };
    let child_start = source.find("Hello").expect("child text");
    assert_eq!(span.start, child_start);
    assert_eq!(span.end, child_start + "Hello".len());
}

#[test]
fn landscape_align_composition_keeps_typed_container() {
    let result = compile_source(".landscape\n    .align {end}\n        A\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let component = landscape(&result);
    let [IrNode::Component {
        component: IrComponent::Container(container),
    }] = component.children.as_slice()
    else {
        panic!("expected typed container child");
    };
    assert_eq!(container.alignment, Some(IrContainerAlignment::End));
}
