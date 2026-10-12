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
        panic!("expected paragraph: {result:?}");
    };
    let json = serde_json::to_string(&result.ir).expect("serialize");
    let decoded: arkst_core::ir::IrDocument = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(decoded, result.ir);
    content.clone()
}

#[test]
fn normal_weight_and_normal_style_preserve_original_inline_children() {
    let plain = one_paragraph(".text text:{Hello} weight:{normal} style:{normal}\n");
    assert!(matches!(plain.as_slice(), [IrInline::Text { content, .. }] if content == "Hello"));
    let explicit_none = one_paragraph(".text text:{Hello} weight:{.none} style:{.none}\n");
    assert!(
        matches!(explicit_none.as_slice(), [IrInline::Text { content, .. }] if content == "Hello")
    );
}

#[test]
fn italic_style_wraps_rich_inline_children_with_source_provenance() {
    let content = one_paragraph(".text style:{italic}\n    Hello **Bold** and `code`\n");
    let [IrInline::Emphasis {
        content: inner,
        span,
    }] = content.as_slice()
    else {
        panic!("expected one italic wrapper: {content:?}");
    };
    assert!(span.end > span.start, "{span:?}");
    assert!(inner
        .iter()
        .any(|node| matches!(node, IrInline::Strong { .. })));
    assert!(inner
        .iter()
        .any(|node| matches!(node, IrInline::Code { .. })));
}

#[test]
fn italic_bold_and_link_preserve_rich_structure_and_legacy_url() {
    let content =
        one_paragraph(".text {Hello} {https://example.org/docs} weight:{bold} style:{italic}\n");
    let [IrInline::Link {
        destination,
        content: linked,
        ..
    }] = content.as_slice()
    else {
        panic!("expected outer link: {content:?}");
    };
    assert_eq!(destination, "https://example.org/docs");
    let [IrInline::Emphasis {
        content: italics, ..
    }] = linked.as_slice()
    else {
        panic!("expected italic children: {linked:?}");
    };
    assert!(matches!(italics.as_slice(), [IrInline::Strong { .. }]));
}

#[test]
fn blank_url_ignores_styling_when_deriving_destination() {
    let content = one_paragraph(".text url:{   } weight:{bold} style:{italic}\n    Hello\n");
    assert!(matches!(
        content.as_slice(),
        [IrInline::Link { destination, content: children, .. }]
            if destination == "Hello" && matches!(children.as_slice(), [IrInline::Emphasis { .. }])
    ));
}

#[test]
fn invalid_style_domains_and_named_only_boundary_are_fail_closed() {
    for source in [
        ".text text:{Hello} weight:{heavy}\n",
        ".text text:{Hello} style:{oblique}\n",
        ".text text:{Hello} style:{123}\n",
        ".text text:{Hello} style:{italic} style:{normal}\n",
        ".text {Hello} {https://example.org} {bold}\n",
        ".text {Hello} {https://example.org} {italic}\n",
        ".text text:{Hello} style:{italic} decoration:{underline}\n",
    ] {
        let result = compile_source(source);
        assert!(!result.diagnostics.is_empty(), "{source:?}: {result:?}");
        assert!(
            result.diagnostics[0].primary.is_some(),
            "{source:?}: {result:?}"
        );
        assert!(result.ir.nodes.is_empty(), "{source:?}: {result:?}");
    }
}
