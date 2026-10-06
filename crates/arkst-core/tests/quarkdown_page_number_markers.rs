use arkst_core::ir::{IrInline, IrNode};
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

fn single_inline(node: &IrNode) -> &IrInline {
    let IrNode::Paragraph { content, .. } = node else {
        panic!("expected paragraph, got {node:?}");
    };
    assert_eq!(content.len(), 1, "{content:?}");
    &content[0]
}

#[test]
fn page_number_markers_materialize_typed_values_in_source_order() {
    let result = compile_source(
        ".formatpagenumber {i}\n.resetpagenumber start:{7}\n.resetpagenumber {0}\n.resetpagenumber {-2}\n",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.ir.nodes.len(), 4, "{:?}", result.ir.nodes);

    assert!(matches!(
        single_inline(&result.ir.nodes[0]),
        IrInline::PageNumberFormat { format, .. } if format == "i"
    ));
    assert!(matches!(
        single_inline(&result.ir.nodes[1]),
        IrInline::PageNumberReset { start: 7, .. }
    ));
    assert!(matches!(
        single_inline(&result.ir.nodes[2]),
        IrInline::PageNumberReset { start: 0, .. }
    ));
    assert!(matches!(
        single_inline(&result.ir.nodes[3]),
        IrInline::PageNumberReset { start: -2, .. }
    ));
}

#[test]
fn reset_page_number_defaults_to_one_without_eager_positive_filtering() {
    let result = compile_source(".resetpagenumber\n");
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert!(matches!(
        single_inline(&result.ir.nodes[0]),
        IrInline::PageNumberReset { start: 1, .. }
    ));
}

#[test]
fn page_number_markers_preserve_source_defined_precedence() {
    let source = ".function {formatpagenumber}\n    custom-format\n\n.function {resetpagenumber}\n    custom-reset\n\n.formatpagenumber {i}\n.resetpagenumber\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let text = result
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
        .join("|");
    assert!(text.contains("custom-format"), "{text}");
    assert!(text.contains("custom-reset"), "{text}");
    assert!(!result.ir.nodes.iter().any(|node| matches!(
        node,
        IrNode::Paragraph { content, .. }
            if content.iter().any(|inline| matches!(
                inline,
                IrInline::PageNumberFormat { .. } | IrInline::PageNumberReset { .. }
            ))
    )));
}

#[test]
fn page_number_marker_binding_rejects_invalid_shapes_and_integer_conversion() {
    for source in [
        ".formatpagenumber\n",
        ".formatpagenumber {i} {I}\n",
        ".resetpagenumber {1.5}\n",
        ".formatpagenumber {i}\n    body\n",
        ".resetpagenumber\n    body\n",
    ] {
        let result = compile_source(source);
        assert!(!result.diagnostics.is_empty(), "{source:?}: {result:?}");
        assert!(
            !result.ir.nodes.iter().any(|node| matches!(
                node,
                IrNode::Paragraph { content, .. }
                    if content.iter().any(|inline| matches!(
                        inline,
                        IrInline::PageNumberFormat { .. } | IrInline::PageNumberReset { .. }
                    ))
            )),
            "{source:?}: {result:?}"
        );
    }
}

#[test]
fn page_number_marker_ir_roundtrips_with_values_and_spans() {
    for value in [
        IrInline::PageNumberFormat {
            format: "I".to_string(),
            span: SourceSpan::new(SourceId(3), 4, 20),
        },
        IrInline::PageNumberReset {
            start: -9,
            span: SourceSpan::new(SourceId(3), 21, 39),
        },
    ] {
        let encoded = serde_json::to_value(&value).expect("marker serializes");
        let decoded: IrInline = serde_json::from_value(encoded).expect("marker deserializes");
        assert_eq!(decoded, value);
    }
}
