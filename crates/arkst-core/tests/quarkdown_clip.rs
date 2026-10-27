use arkst_core::evaluator::Evaluator;
use arkst_core::ir::{
    IrClipComponent, IrClipShape, IrComponent, IrEnumValue, IrInline, IrNode, IrParameter, IrValue,
};
use arkst_core::{compile, CompileOptions, SourceId, SourceSpan, VirtualProjectBuilder};

fn compile_source(source: &str) -> arkst_core::CompileResult {
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("valid entry")
        .add_source("main.qd", source)
        .expect("valid source")
        .build()
        .expect("valid project");
    compile(&project, &CompileOptions::default())
}

fn clip(result: &arkst_core::CompileResult) -> &IrClipComponent {
    let [IrNode::Component {
        component: IrComponent::Clip(component),
    }] = result.ir.nodes.as_slice()
    else {
        panic!("expected one clip component, got {:?}", result.ir.nodes);
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

fn paragraph_inlines(component: &IrClipComponent) -> &[IrInline] {
    let [IrNode::Paragraph { content, .. }] = component.children.as_slice() else {
        panic!("expected one paragraph, got {:?}", component.children);
    };
    content
}

fn inline_text(inlines: &[IrInline]) -> String {
    let mut text = String::new();
    for inline in inlines {
        match inline {
            IrInline::Text { content, .. } => text.push_str(content),
            IrInline::Strong { content, .. } => text.push_str(&inline_text(content)),
            other => panic!("unexpected inline {other:?}"),
        }
    }
    text
}

fn assert_one_failure(result: &arkst_core::CompileResult) {
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert!(result.ir.nodes.is_empty(), "{result:?}");
}

#[test]
fn clip_circle_constructs_typed_component_with_structured_body() {
    let result = compile_source(".clip {circle}\n    ## Heading\n\n    Body\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let component = clip(&result);
    assert_eq!(component.shape, IrClipShape::Circle);
    assert!(matches!(
        component.children[0],
        IrNode::Heading { level: 2, .. }
    ));
    assert_eq!(paragraph_text(&component.children[1]), "Body");
}

#[test]
fn clip_accepts_named_and_case_insensitive_circle() {
    for source in [
        ".clip clip:{circle}\n    A\n",
        ".clip {CIRCLE}\n    A\n",
        ".clip clip:{CiRcLe}\n    A\n",
    ] {
        let result = compile_source(source);
        assert!(result.diagnostics.is_empty(), "{source:?}: {result:?}");
        assert_eq!(clip(&result).shape, IrClipShape::Circle);
    }
}

#[test]
fn clip_accepts_positional_named_and_generated_rich_markdown_values() {
    for (source, expected) in [
        (
            ".clip {circle} {Positional **Markdown** body.}\n",
            "Positional Markdown body.",
        ),
        (
            ".clip clip:{circle} body:{Named **Markdown** body.}\n",
            "Named Markdown body.",
        ),
        (
            ".clip {circle} body:{.string {\"Generated **Markdown** body.\"}}\n",
            "Generated Markdown body.",
        ),
    ] {
        let result = compile_source(source);
        assert!(result.diagnostics.is_empty(), "{result:?}");
        let inlines = paragraph_inlines(clip(&result));
        assert_eq!(inline_text(inlines), expected);
        assert!(
            inlines
                .iter()
                .any(|inline| matches!(inline, IrInline::Strong { .. })),
            "{result:?}"
        );
    }
}

#[test]
fn clip_materializes_non_null_scalar_markdown_content_values() {
    for (source, expected) in [
        (".clip {circle} body:{.sum {8} {5}}\n", "13"),
        (".clip {circle} body:{.iseven {4}}\n", "true"),
        (".clip {circle} body:{.none}\n", "None"),
    ] {
        let result = compile_source(source);
        assert!(result.diagnostics.is_empty(), "{result:?}");
        assert_eq!(paragraph_text(&clip(&result).children[0]), expected);
    }
}

#[test]
fn clip_preserves_variable_and_callable_rich_content_values() {
    for source in [
        ".var {clipbody} {Variable **Markdown** body.}\n\n.clip {circle} body:{.clipbody}\n",
        ".function {makeclipbody}\n    Callable **Markdown** body.\n\n.clip {circle} body:{.makeclipbody}\n",
    ] {
        let result = compile_source(source);
        assert!(result.diagnostics.is_empty(), "{result:?}");
        let inlines = paragraph_inlines(clip(&result));
        assert!(inline_text(inlines).ends_with("Markdown body."));
        assert!(
            inlines
                .iter()
                .any(|inline| matches!(inline, IrInline::Strong { .. })),
            "{result:?}"
        );
    }
}

#[test]
fn clip_rejects_unverified_structured_body_conversion_with_provenance() {
    let source = ".clip {circle} body:{.range {1} {3}}\n";
    let result = compile_source(source);
    assert_one_failure(&result);
    let diagnostic = &result.diagnostics[0];
    assert!(
        diagnostic.message.contains("parameter `body`"),
        "{result:?}"
    );
    let primary = diagnostic.primary.expect("body conversion primary span");
    assert_eq!(&source[primary.start..primary.end], "body:{.range {1} {3}}");
}

#[test]
fn clip_rejects_missing_duplicate_unknown_and_extra_bindings() {
    for source in [
        ".clip\n    A\n",
        ".clip {circle} clip:{circle}\n    A\n",
        ".clip shape:{circle}\n    A\n",
        ".clip {circle} {Explicit body} extra:{value}\n",
        ".clip {circle} {Explicit body}\n    Indented body\n",
        ".clip {circle}\n",
    ] {
        let result = compile_source(source);
        assert_one_failure(&result);
    }
}

#[test]
fn clip_conversion_is_origin_aware_and_precedes_body_evaluation() {
    let static_text = compile_source(".clip {.string {circle}}\n    A\n");
    assert_one_failure(&static_text);

    let invalid = compile_source(".clip {square}\n    .grid columns:{0}\n        A\n");
    assert_one_failure(&invalid);
    assert!(invalid.diagnostics[0].message.contains("circle"));
    assert!(!invalid.diagnostics[0].message.contains("Column count"));
}

#[test]
fn clip_rejects_lambda_body_without_evaluating_it() {
    let span = SourceSpan::new(SourceId(1), 0, 5);
    let document = arkst_core::ir::IrDocument {
        nodes: vec![IrNode::FunctionCall {
            name: "clip".to_string(),
            positional_args: vec![IrValue::Enum(IrEnumValue::ClipShape(IrClipShape::Circle))],
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
                named_args: vec![arkst_core::ir::IrNamedArg {
                    name: "columns".to_string(),
                    name_span: span,
                    value: IrValue::Number(0.0),
                    span,
                }],
                ordered_args: None,
                lambda_parameters: None,
                body: None,
                raw_body: None,
                span,
            }]),
            raw_body: None,
            span,
        }],
        metadata: arkst_core::ir::IrMetadata::default(),
    };

    let (evaluated, diagnostics) = Evaluator::new().evaluate(&document);
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert!(diagnostics[0].message.contains("not a lambda"));
    assert!(!diagnostics[0].message.contains("Column count"));
    assert!(evaluated.nodes.is_empty());
}

#[test]
fn source_defined_clip_shadows_native_in_block_and_inline_contexts() {
    let block = compile_source(".function {clip}\n    custom\n\n.clip\n");
    assert!(block.diagnostics.is_empty(), "{block:?}");
    assert_eq!(paragraph_text(&block.ir.nodes[0]), "custom");

    let inline = compile_source(".function {clip}\n    custom\n\nprefix .clip suffix\n");
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
fn native_inline_clip_fails_closed_without_fabricated_output() {
    let result = compile_source("prefix .clip {circle} suffix\n");
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    let [IrNode::Paragraph { content, .. }] = result.ir.nodes.as_slice() else {
        panic!("expected surrounding paragraph, got {:?}", result.ir.nodes);
    };
    assert!(content
        .iter()
        .all(|inline| matches!(inline, IrInline::Text { .. })));
    assert!(!format!("{content:?}").contains("clip"));
}

#[test]
fn clip_callable_and_nested_composition_preserve_component_values() {
    let callable = compile_source(".function {round}\n    .clip {circle}\n        A\n\n.round\n");
    assert!(callable.diagnostics.is_empty(), "{callable:?}");
    assert_eq!(paragraph_text(&clip(&callable).children[0]), "A");

    let nested = compile_source(".clip {circle}\n    .container\n        A\n");
    assert!(nested.diagnostics.is_empty(), "{nested:?}");
    assert!(matches!(
        clip(&nested).children.as_slice(),
        [IrNode::Component {
            component: IrComponent::Container(_)
        }]
    ));

    let outer = compile_source(".container\n    .clip {circle}\n        A\n");
    assert!(outer.diagnostics.is_empty(), "{outer:?}");
    let [IrNode::Component {
        component: IrComponent::Container(container),
    }] = outer.ir.nodes.as_slice()
    else {
        panic!("expected container, got {:?}", outer.ir.nodes);
    };
    assert!(matches!(
        container.children.as_slice(),
        [IrNode::Component {
            component: IrComponent::Clip(_)
        }]
    ));
}

#[test]
fn clip_preserves_call_child_provenance_and_wire_roundtrip() {
    let source = ".clip {circle}\n    Hello\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let component = clip(&result);
    assert_eq!(component.span.start, 0);
    assert_eq!(component.span.end, source.len() - 1);
    let IrNode::Paragraph { span, .. } = &component.children[0] else {
        panic!("expected paragraph child");
    };
    let child_start = source.find("Hello").expect("child text");
    assert_eq!(span.start, child_start);
    assert_eq!(span.end, child_start + "Hello".len());

    let encoded = serde_json::to_string(&result.ir).expect("clip IR serializes");
    let decoded: arkst_core::ir::IrDocument =
        serde_json::from_str(&encoded).expect("clip IR deserializes");
    assert_eq!(decoded, result.ir);
    assert_eq!(
        serde_json::to_string(&decoded).expect("decoded clip IR serializes"),
        encoded
    );
}
