use arkst_core::evaluator::Evaluator;
use arkst_core::ir::{
    IrComponent, IrContainerAlignment, IrInline, IrNode, IrParameter, IrStackedLayout,
};
use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};

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

fn align(result: &arkst_core::CompileResult) -> &arkst_core::ir::IrContainerComponent {
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
fn align_accepts_positional_named_and_generated_rich_markdown_values() {
    for (source, expected_alignment, expected_text) in [
        (
            ".align {start} {Positional **Markdown** body.}\n",
            IrContainerAlignment::Start,
            "Positional Markdown body.",
        ),
        (
            ".align alignment:{end} body:{Named **Markdown** body.}\n",
            IrContainerAlignment::End,
            "Named Markdown body.",
        ),
        (
            ".align {center} body:{.string {\"Generated **Markdown** body.\"}}\n",
            IrContainerAlignment::Center,
            "Generated Markdown body.",
        ),
    ] {
        let result = compile_source(source);
        assert!(result.diagnostics.is_empty(), "{result:?}");
        let component = align(&result);
        assert_eq!(component.alignment, Some(expected_alignment));
        let inlines = paragraph_inlines(component);
        assert_eq!(inline_text(inlines), expected_text);
        assert!(
            inlines
                .iter()
                .any(|inline| matches!(inline, IrInline::Strong { .. })),
            "{result:?}"
        );
    }
}

#[test]
fn align_materializes_non_null_scalar_markdown_content_values() {
    for (source, expected) in [
        (".align {start} body:{.sum {8} {5}}\n", "13"),
        (".align {center} body:{.iseven {4}}\n", "true"),
        (".align {end} body:{.none}\n", "None"),
    ] {
        let result = compile_source(source);
        assert!(result.diagnostics.is_empty(), "{result:?}");
        assert_eq!(paragraph_text(&align(&result).children[0]), expected);
    }
}

#[test]
fn align_preserves_variable_and_callable_rich_content_values() {
    for source in [
        ".var {alignbody} {Variable **Markdown** body.}\n\n.align {start} body:{.alignbody}\n",
        ".function {makealignbody}\n    Callable **Markdown** body.\n\n.align {end} body:{.makealignbody}\n",
    ] {
        let result = compile_source(source);
        assert!(result.diagnostics.is_empty(), "{result:?}");
        let inlines = paragraph_inlines(align(&result));
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
fn align_rejects_unverified_structured_conversion_with_body_provenance() {
    let source = ".align {center} body:{.range {1} {3}}\n";
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
fn align_body_binding_validation_precedes_lazy_body_evaluation() {
    let result = compile_source(".align {center} body:{foo}\n    .grid columns:{0}\n        A\n");
    assert_one_failure(&result);
    assert!(!result.diagnostics[0].message.contains("Column count"));
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
    let span = arkst_core::SourceSpan::new(arkst_core::SourceId(1), 0, 7);
    let document = arkst_core::ir::IrDocument {
        nodes: vec![IrNode::FunctionCall {
            name: "align".to_string(),
            positional_args: vec![arkst_core::ir::IrValue::Identifier("center".to_string())],
            named_args: Vec::new(),
            ordered_args: None,
            lambda_parameters: Some(vec![IrParameter {
                name: "x".to_string(),
                name_span: span,
                span,
                optional: false,
            }]),
            body: Some(vec![IrNode::Paragraph {
                content: vec![arkst_core::ir::IrInline::Text {
                    content: "A".to_string(),
                    span,
                }],
                span,
            }]),
            raw_body: None,
            span,
        }],
        metadata: arkst_core::ir::IrMetadata::default(),
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
            arkst_core::ir::IrInline::Text { content, .. } => Some(content.as_str()),
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
        .all(|inline| matches!(inline, arkst_core::ir::IrInline::Text { .. })));
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
