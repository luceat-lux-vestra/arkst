use arkst_core::ir::{IrInline, IrNode};
use arkst_core::{compile, CompileOptions, SourceId, SourceSpan, VirtualProjectBuilder};

fn compile_source(source: &str) -> (arkst_core::CompileResult, SourceId) {
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("valid entry")
        .add_source("main.qd", source)
        .expect("valid source")
        .build()
        .expect("valid project");
    let source_id = project
        .sources()
        .get_id(project.entry())
        .expect("entry source id");
    (compile(&project, &CompileOptions::default()), source_id)
}

fn paragraph(result: &arkst_core::CompileResult) -> &[IrInline] {
    let [IrNode::Paragraph { content, .. }] = result.ir.nodes.as_slice() else {
        panic!("expected one paragraph, got {:?}", result.ir.nodes);
    };
    content
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
fn lastheading_materializes_signed_depth_without_documented_range_validation() {
    let source = ".doctype {paged}\n.lastheading {0} .lastheading {-2} .lastheading {7}\n";
    let (result, source_id) = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");

    let content = paragraph(&result);
    let observed = content
        .iter()
        .filter_map(|inline| match inline {
            IrInline::LastHeading { depth, span } => Some((*depth, *span)),
            IrInline::Text { .. } => None,
            other => panic!("unexpected inline: {other:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        observed.iter().map(|(depth, _)| *depth).collect::<Vec<_>>(),
        vec![0, -2, 7]
    );
    for ((depth, span), spelling) in
        observed
            .iter()
            .zip([".lastheading {0}", ".lastheading {-2}", ".lastheading {7}"])
    {
        let start = source.find(spelling).expect("call source");
        assert_eq!(span.source_id, source_id, "depth={depth}");
        assert_eq!(span.start, start, "depth={depth}");
        assert_eq!(span.end, start + spelling.len(), "depth={depth}");
    }
}

#[test]
fn lastheading_is_rejected_for_plain_documents() {
    let (result, _) = compile_source(".lastheading {1}\n");
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert!(result.ir.nodes.is_empty(), "{result:?}");
    assert!(
        result.diagnostics[0].message.contains("plain documents"),
        "{result:?}"
    );
}

#[test]
fn lastheading_is_available_for_paged_slides_and_docs_document_types() {
    for document_type in ["paged", "slides", "docs"] {
        let source = format!(".doctype {{{document_type}}}\n.lastheading {{2}}\n");
        let (result, _) = compile_source(&source);
        assert!(result.diagnostics.is_empty(), "{document_type}: {result:?}");
        assert!(matches!(
            paragraph(&result),
            [IrInline::LastHeading { depth: 2, .. }]
        ));
    }
}

#[test]
fn source_defined_lastheading_keeps_precedence_over_native_binding_and_plain_gate() {
    let source = ".function {lastheading}\n    custom\n\n.lastheading\n";
    let (result, _) = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(result.ir.nodes.len(), 1, "{result:?}");
    assert_eq!(paragraph_text(&result.ir.nodes[0]), "custom");
}

#[test]
fn lastheading_binding_accepts_named_depth_and_rejects_invalid_shapes() {
    let (named, _) = compile_source(".doctype {paged}\n.lastheading depth:{4}\n");
    assert!(named.diagnostics.is_empty(), "{named:?}");
    assert!(matches!(
        paragraph(&named),
        [IrInline::LastHeading { depth: 4, .. }]
    ));

    for source in [
        ".doctype {paged}\n.lastheading\n",
        ".doctype {paged}\n.lastheading {1} {2}\n",
        ".doctype {paged}\n.lastheading {1.5}\n",
        ".doctype {paged}\n.lastheading {1}\n    body\n",
    ] {
        let (result, _) = compile_source(source);
        assert!(!result.diagnostics.is_empty(), "{source:?}: {result:?}");
        assert!(
            !result.ir.nodes.iter().any(|node| matches!(
                node,
                IrNode::Paragraph { content, .. }
                    if content.iter().any(|inline| matches!(inline, IrInline::LastHeading { .. }))
            )),
            "{source:?}: {result:?}"
        );
    }
}

#[test]
fn lastheading_ir_roundtrips_with_depth_and_span() {
    let value = IrInline::LastHeading {
        depth: -9,
        span: SourceSpan::new(SourceId(3), 4, 20),
    };
    let encoded = serde_json::to_value(&value).expect("lastheading serializes");
    let decoded: IrInline = serde_json::from_value(encoded).expect("lastheading deserializes");
    assert_eq!(decoded, value);
}
