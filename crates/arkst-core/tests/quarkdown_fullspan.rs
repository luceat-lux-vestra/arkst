use arkst_core::evaluator::Evaluator;
use arkst_core::ir::{IrComponent, IrInline, IrNode, IrParameter};
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

fn full_span(result: &arkst_core::CompileResult) -> &arkst_core::ir::IrContainerComponent {
    let [IrNode::Component {
        component: IrComponent::Container(component),
    }] = result.ir.nodes.as_slice()
    else {
        panic!(
            "expected one full-span container component, got {:?}",
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

fn paragraph_inlines(component: &arkst_core::ir::IrContainerComponent) -> &[IrInline] {
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
fn fullspan_constructs_existing_container_with_required_structured_body() {
    let source = ".fullspan\n    Hello\n\n    ## World\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");

    let component = full_span(&result);
    assert_eq!(component.width, None);
    assert_eq!(component.height, None);
    assert!(!component.full_width);
    assert!(component.full_column_span);
    assert_eq!(component.alignment, None);
    assert_eq!(component.children.len(), 2);
    assert_eq!(paragraph_text(&component.children[0]), "Hello");
    assert!(matches!(component.children[1], IrNode::Heading { .. }));
}

#[test]
fn fullspan_accepts_positional_named_and_generated_rich_markdown_values() {
    for (source, expected) in [
        (
            ".fullspan {Positional **Markdown** body.}\n",
            "Positional Markdown body.",
        ),
        (
            ".fullspan body:{Named **Markdown** body.}\n",
            "Named Markdown body.",
        ),
        (
            ".fullspan body:{.string {\"Generated **Markdown** body.\"}}\n",
            "Generated Markdown body.",
        ),
    ] {
        let result = compile_source(source);
        assert!(result.diagnostics.is_empty(), "{result:?}");
        let inlines = paragraph_inlines(full_span(&result));
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
fn fullspan_materializes_non_null_scalar_markdown_content_values() {
    for (source, expected) in [
        (".fullspan body:{.sum {8} {5}}\n", "13"),
        (".fullspan body:{.iseven {4}}\n", "true"),
        (".fullspan body:{.none}\n", "None"),
    ] {
        let result = compile_source(source);
        assert!(result.diagnostics.is_empty(), "{result:?}");
        assert_eq!(paragraph_text(&full_span(&result).children[0]), expected);
    }
}

#[test]
fn fullspan_preserves_variable_and_callable_rich_content_values() {
    for source in [
        ".var {widebody} {Variable **Markdown** body.}\n\n.fullspan body:{.widebody}\n",
        ".function {makewidebody}\n    Callable **Markdown** body.\n\n.fullspan body:{.makewidebody}\n",
    ] {
        let result = compile_source(source);
        assert!(result.diagnostics.is_empty(), "{result:?}");
        let inlines = paragraph_inlines(full_span(&result));
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
fn fullspan_rejects_unverified_structured_conversion_with_body_provenance() {
    let source = ".fullspan body:{.range {1} {3}}\n";
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
fn fullspan_requires_body_and_rejects_collisions_and_unknown_arguments() {
    for (source, expected) in [
        (".fullspan\n", "missing required argument `body`"),
        (".fullspan {foo}\n    A\n", "argument `body` more than once"),
        (".fullspan foo:{bar}\n    A\n", "named argument `foo`"),
    ] {
        let result = compile_source(source);
        assert_one_failure(&result);
        assert!(
            result.diagnostics[0]
                .message
                .to_ascii_lowercase()
                .contains(expected),
            "source={source:?} result={result:?}"
        );
    }
}

#[test]
fn fullspan_argument_validation_precedes_lazy_body_evaluation() {
    let result = compile_source(".fullspan {foo}\n    .grid columns:{0}\n        A\n");
    assert_one_failure(&result);
    assert!(result.diagnostics[0].message.contains("fullspan"));
    assert!(!result.diagnostics[0].message.contains("Column count"));
}

#[test]
fn fullspan_rejects_lambda_body_without_evaluating_it() {
    let span = SourceSpan::new(SourceId(1), 0, 9);
    let document = arkst_core::ir::IrDocument {
        nodes: vec![IrNode::FunctionCall {
            name: "fullspan".to_string(),
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
                named_args: vec![arkst_core::ir::IrNamedArg {
                    name: "columns".to_string(),
                    name_span: span,
                    value: arkst_core::ir::IrValue::Number(0.0),
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
fn source_defined_fullspan_shadows_native_in_block_and_inline_contexts() {
    let block = compile_source(".function {fullspan}\n    custom\n\n.fullspan\n");
    assert!(block.diagnostics.is_empty(), "{block:?}");
    assert_eq!(paragraph_text(&block.ir.nodes[0]), "custom");

    let inline = compile_source(".function {fullspan}\n    custom\n\nprefix .fullspan suffix\n");
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
fn native_inline_fullspan_fails_closed_without_fabricated_output() {
    let result = compile_source("prefix .fullspan suffix\n");
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
    assert!(!text.contains("fullspan"));
}

#[test]
fn fullspan_callable_result_preserves_component_value() {
    let result = compile_source(".function {wide}\n    .fullspan\n        A\n\n.wide\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(paragraph_text(&full_span(&result).children[0]), "A");
}

#[test]
fn fullspan_composes_with_existing_container_and_stacked_components() {
    let nested = compile_source(".container\n    .fullspan\n        A\n");
    assert!(nested.diagnostics.is_empty(), "{nested:?}");
    let [IrNode::Component {
        component: IrComponent::Container(outer),
    }] = nested.ir.nodes.as_slice()
    else {
        panic!("expected outer container, got {:?}", nested.ir.nodes);
    };
    let [IrNode::Component {
        component: IrComponent::Container(inner),
    }] = outer.children.as_slice()
    else {
        panic!(
            "expected nested fullspan container, got {:?}",
            outer.children
        );
    };
    assert!(inner.full_column_span);

    let stacked = compile_source(".fullspan\n    .row\n        A\n\n        B\n");
    assert!(stacked.diagnostics.is_empty(), "{stacked:?}");
    assert!(matches!(
        full_span(&stacked).children.as_slice(),
        [IrNode::Component {
            component: IrComponent::Stacked(_)
        }]
    ));
}

#[test]
fn fullspan_preserves_call_and_child_provenance() {
    let source = ".fullspan\n    Hello\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let component = full_span(&result);
    assert_eq!(component.span.start, 0);
    assert_eq!(component.span.end, source.len() - 1);
    let IrNode::Paragraph { span, .. } = &component.children[0] else {
        panic!("expected paragraph child");
    };
    let child_start = source.find("Hello").expect("child text");
    assert_eq!(span.start, child_start);
    assert_eq!(span.end, child_start + "Hello".len());
}
