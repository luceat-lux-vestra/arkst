use arkst_core::ir::{IrInline, IrNode};
use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};

fn compile_source(source: &str) -> arkst_core::CompileResult {
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("entry")
        .add_source("main.qd", source)
        .expect("source")
        .build()
        .expect("project");
    compile(&project, &CompileOptions::default())
}

fn one_paragraph(source: &str) -> Vec<IrInline> {
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{source:?}: {result:?}");
    let [IrNode::Paragraph { content, .. }] = result.ir.nodes.as_slice() else {
        panic!("expected one paragraph: {result:?}");
    };
    let json = serde_json::to_string(&result.ir).expect("serialize");
    let restored: arkst_core::ir::IrDocument = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored, result.ir);
    content.clone()
}

#[test]
fn named_bold_preserves_rich_children_without_flattening() {
    let content = one_paragraph(".text weight:{bold}\n    hello *italic* and `code`\n");
    let [IrInline::Strong {
        content: inner,
        span,
    }] = content.as_slice()
    else {
        panic!("expected typed bold wrapper: {content:?}");
    };
    assert!(span.end > span.start);
    assert!(inner
        .iter()
        .any(|node| matches!(node, IrInline::Emphasis { .. })));
    assert!(inner
        .iter()
        .any(|node| matches!(node, IrInline::Code { .. })));
}

#[test]
fn bold_transform_precedes_url_link_and_preserves_legacy_positional_url() {
    let content = one_paragraph(".text {hello} url:{https://example.org/docs} weight:{bold}\n");
    let [IrInline::Link {
        content: children,
        destination,
        ..
    }] = content.as_slice()
    else {
        panic!("expected outer link: {content:?}");
    };
    assert_eq!(destination, "https://example.org/docs");
    assert!(matches!(children.as_slice(), [IrInline::Strong { .. }]));

    let old = one_paragraph(".text {hello} {https://example.org/legacy}\n");
    assert!(
        matches!(old.as_slice(), [IrInline::Link { destination, .. }] if destination == "https://example.org/legacy")
    );
}

#[test]
fn named_bold_keeps_blank_url_derivation_from_unstyled_text() {
    let content = one_paragraph(".text weight:{bold} url:{   }\n    Hello\n");
    assert!(matches!(
        content.as_slice(),
        [IrInline::Link { destination, content, .. }]
            if destination == "Hello" && matches!(content.as_slice(), [IrInline::Strong { .. }])
    ));
}

#[test]
fn unsupported_weight_and_style_values_and_conflicting_bindings_fail_atomically() {
    for source in [
        ".text weight:{heavy} {hello}\n",
        ".text weight:{123} {hello}\n",
        ".text style:{oblique} text:{hello}\n",
        ".text text:{hello} style:{underline}\n",
        ".text weight:{bold} weight:{bold} {hello}\n",
        ".text weight:{bold} text:{hello} {world}\n",
        ".text {hello} {https://example.org/docs} {bold}\n",
    ] {
        let result = compile_source(source);
        assert!(!result.diagnostics.is_empty(), "{source:?}: {result:?}");
        assert!(result.ir.nodes.is_empty(), "{source:?}: {result:?}");
    }
}
