use scribium_core::evaluator::Evaluator;
use scribium_core::ir::{IrInline, IrNode, IrSize, IrSizeUnit, IrValue};
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

fn whitespace(result: &scribium_core::CompileResult) -> &IrInline {
    let [IrNode::Paragraph { content, .. }] = result.ir.nodes.as_slice() else {
        panic!("expected one paragraph, got {:?}", result.ir.nodes);
    };
    let [inline] = content.as_slice() else {
        panic!("expected one inline, got {content:?}");
    };
    inline
}

fn paragraph_text(node: &IrNode) -> String {
    let IrNode::Paragraph { content, .. } = node else {
        panic!("expected paragraph, got {node:?}");
    };
    content
        .iter()
        .map(|inline| match inline {
            IrInline::Text { content, .. } => content.as_str(),
            other => panic!("expected text, got {other:?}"),
        })
        .collect()
}

#[test]
fn whitespace_supports_the_bounded_argument_surface() {
    let cases = [
        (".whitespace\n", None, None),
        (
            ".whitespace {10pt}\n",
            Some((10.0, IrSizeUnit::Pt)),
            Some((0.0, IrSizeUnit::Px)),
        ),
        (
            ".whitespace width:{10pt}\n",
            Some((10.0, IrSizeUnit::Pt)),
            Some((0.0, IrSizeUnit::Px)),
        ),
        (
            ".whitespace height:{20pt}\n",
            Some((0.0, IrSizeUnit::Px)),
            Some((20.0, IrSizeUnit::Pt)),
        ),
        (
            ".whitespace {10pt} {20pt}\n",
            Some((10.0, IrSizeUnit::Pt)),
            Some((20.0, IrSizeUnit::Pt)),
        ),
        (
            ".whitespace {10pt} height:{20pt}\n",
            Some((10.0, IrSizeUnit::Pt)),
            Some((20.0, IrSizeUnit::Pt)),
        ),
        (
            ".whitespace width:{10pt} height:{20pt}\n",
            Some((10.0, IrSizeUnit::Pt)),
            Some((20.0, IrSizeUnit::Pt)),
        ),
    ];

    for (source, expected_width, expected_height) in cases {
        let result = compile_source(source);
        assert!(result.diagnostics.is_empty(), "{source:?}: {result:?}");
        let IrInline::Whitespace { width, height, .. } = whitespace(&result) else {
            panic!("expected semantic whitespace for {source:?}");
        };
        assert_eq!(
            width.as_ref().map(|value| (value.value, value.unit)),
            expected_width,
            "width for {source:?}"
        );
        assert_eq!(
            height.as_ref().map(|value| (value.value, value.unit)),
            expected_height,
            "height for {source:?}"
        );
    }
}

#[test]
fn whitespace_preserves_inline_order_and_call_provenance() {
    let source = "A .whitespace width:{1em} B\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let [IrNode::Paragraph { content, span }] = result.ir.nodes.as_slice() else {
        panic!("expected one paragraph, got {:?}", result.ir.nodes);
    };
    assert_eq!(content.len(), 3);
    assert!(matches!(
        &content[0],
        IrInline::Text { content, .. } if content == "A "
    ));
    let IrInline::Whitespace {
        width: Some(width),
        height: Some(height),
        span: whitespace_span,
    } = &content[1]
    else {
        panic!(
            "expected dimensioned inline whitespace, got {:?}",
            content[1]
        );
    };
    assert_eq!(width.value, 1.0);
    assert_eq!(width.unit, IrSizeUnit::Em);
    assert_eq!(height.value, 0.0);
    assert_eq!(height.unit, IrSizeUnit::Px);
    let call_start = source.find(".whitespace").expect("call span");
    assert_eq!(whitespace_span.start, call_start);
    assert_eq!(
        whitespace_span.end,
        source.trim_end_matches('\n').len() - " B".len()
    );
    assert!(matches!(
        &content[2],
        IrInline::Text { content, .. } if content == " B"
    ));
    assert!(span.start < whitespace_span.start);
}

#[test]
fn whitespace_standalone_output_uses_the_existing_inline_materialization_boundary() {
    let result = compile_source(".whitespace width:{2cm}\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let [IrNode::Paragraph { content, .. }] = result.ir.nodes.as_slice() else {
        panic!("expected paragraph output, got {:?}", result.ir.nodes);
    };
    assert!(matches!(content.as_slice(), [IrInline::Whitespace { .. }]));
}

#[test]
fn whitespace_reuses_size_origin_rules_and_typed_identity() {
    let dynamic = compile_source(".var {space} {2cm}\n.whitespace width:{.space}\n");
    assert!(dynamic.diagnostics.is_empty(), "{dynamic:?}");
    assert!(matches!(
        whitespace(&dynamic),
        IrInline::Whitespace {
            width: Some(IrSize { value, unit: IrSizeUnit::Cm }),
            ..
        } if *value == 2.0
    ));

    let static_text = compile_source(".whitespace width:{.string {2cm}}\n");
    assert_eq!(static_text.diagnostics.len(), 1, "{static_text:?}");
    assert!(static_text.ir.nodes.is_empty(), "{static_text:?}");

    let span = SourceSpan::new(SourceId(1), 0, 20);
    let document = scribium_core::ir::IrDocument {
        nodes: vec![IrNode::FunctionCall {
            name: "whitespace".to_string(),
            positional_args: vec![IrValue::Size(IrSize {
                value: 12.0,
                unit: IrSizeUnit::Pt,
            })],
            named_args: Vec::new(),
            ordered_args: None,
            lambda_parameters: None,
            body: None,
            span,
        }],
        metadata: scribium_core::ir::IrMetadata::default(),
    };
    let (evaluated, diagnostics) = Evaluator::new().evaluate(&document);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let [IrNode::Paragraph { content, .. }] = evaluated.nodes.as_slice() else {
        panic!("expected paragraph output, got {:?}", evaluated.nodes);
    };
    assert!(matches!(
        content.as_slice(),
        [IrInline::Whitespace {
            width: Some(IrSize {
                value: 12.0,
                unit: IrSizeUnit::Pt
            }),
            height: Some(IrSize {
                value: 0.0,
                unit: IrSizeUnit::Px
            }),
            ..
        }]
    ));
}

#[test]
fn whitespace_argument_failures_are_atomic_and_source_backed() {
    for source in [
        ".whitespace {1pt} {2pt} {3pt}\n",
        ".whitespace depth:{1pt}\n",
        ".whitespace {1pt} width:{2pt}\n",
        ".whitespace width:{1pt} width:{2pt}\n",
        ".whitespace width:{not-a-size}\n",
        ".whitespace\n    body\n",
    ] {
        let result = compile_source(source);
        assert_eq!(result.diagnostics.len(), 1, "{source:?}: {result:?}");
        assert!(result.ir.nodes.is_empty(), "{source:?}: {result:?}");
        assert!(
            result.diagnostics[0].primary.is_some(),
            "diagnostic lost provenance for {source:?}: {:?}",
            result.diagnostics
        );
    }
}

#[test]
fn whitespace_rejects_inline_and_lambda_bodies() {
    let span = SourceSpan::new(SourceId(1), 0, 10);
    let inline_document = scribium_core::ir::IrDocument {
        nodes: vec![IrNode::Paragraph {
            content: vec![IrInline::DirectiveCall {
                name: "whitespace".to_string(),
                positional_args: Vec::new(),
                named_args: Vec::new(),
                ordered_args: None,
                body: Some(vec![IrInline::Text {
                    content: "body".to_string(),
                    span,
                }]),
                span,
            }],
            span,
        }],
        metadata: scribium_core::ir::IrMetadata::default(),
    };
    let (inline, inline_diagnostics) = Evaluator::new().evaluate(&inline_document);
    assert_eq!(inline_diagnostics.len(), 1, "{inline_diagnostics:?}");
    assert!(inline.nodes.iter().all(|node| {
        !matches!(
            node,
            IrNode::Paragraph { content, .. }
                if content.iter().any(|inline| matches!(inline, IrInline::Whitespace { .. }))
        )
    }));

    let lambda_document = scribium_core::ir::IrDocument {
        nodes: vec![IrNode::FunctionCall {
            name: "whitespace".to_string(),
            positional_args: Vec::new(),
            named_args: Vec::new(),
            ordered_args: None,
            lambda_parameters: Some(vec![scribium_core::ir::IrParameter {
                name: "value".to_string(),
                name_span: span,
                span,
                optional: false,
            }]),
            body: Some(vec![IrNode::Paragraph {
                content: vec![IrInline::Text {
                    content: "body".to_string(),
                    span,
                }],
                span,
            }]),
            span,
        }],
        metadata: scribium_core::ir::IrMetadata::default(),
    };
    let (lambda, lambda_diagnostics) = Evaluator::new().evaluate(&lambda_document);
    assert_eq!(lambda_diagnostics.len(), 1, "{lambda_diagnostics:?}");
    assert!(lambda.nodes.is_empty(), "{lambda:?}");
}

#[test]
fn whitespace_respects_source_defined_shadowing_in_block_and_inline_contexts() {
    let result = compile_source(
        ".function {whitespace}\n    custom\n\n.whitespace\n\nprefix .whitespace suffix\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(result.ir.nodes.len(), 2);
    assert_eq!(paragraph_text(&result.ir.nodes[0]), "custom");
    assert_eq!(paragraph_text(&result.ir.nodes[1]), "prefix custom suffix");
}

#[test]
fn whitespace_ir_roundtrips_through_serde() {
    let value = IrInline::Whitespace {
        width: Some(IrSize {
            value: 2.0,
            unit: IrSizeUnit::Cm,
        }),
        height: Some(IrSize {
            value: 0.0,
            unit: IrSizeUnit::Px,
        }),
        span: SourceSpan::new(SourceId(3), 4, 30),
    };
    let encoded = serde_json::to_value(&value).expect("whitespace serializes");
    let decoded: IrInline = serde_json::from_value(encoded).expect("whitespace deserializes");
    assert_eq!(decoded, value);
}
