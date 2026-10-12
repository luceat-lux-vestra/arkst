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

fn single_paragraph(source: &str) -> Vec<IrInline> {
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{source:?}: {result:?}");
    let [IrNode::Paragraph { content, .. }] = result.ir.nodes.as_slice() else {
        panic!("expected one paragraph: {result:?}");
    };
    let wire = serde_json::to_string(&result.ir).expect("serialize");
    let parsed: arkst_core::ir::IrDocument = serde_json::from_str(&wire).expect("deserialize");
    assert_eq!(result.ir, parsed);
    content.clone()
}

#[test]
fn none_decoration_and_explicit_none_are_semantically_unstyled() {
    for source in [
        ".text text:{Hello} decoration:{none}\n",
        ".text text:{Hello} decoration:{.none}\n",
    ] {
        let content = single_paragraph(source);
        assert!(
            matches!(content.as_slice(), [IrInline::Text { content, .. }] if content == "Hello")
        );
    }
}

#[test]
fn strikethrough_reuses_typed_inline_ir_with_evaluated_rich_children() {
    let content =
        single_paragraph(".text decoration:{strikethrough}\n    Hello **bold** and `code`\n");
    let [IrInline::Strikethrough {
        content: inner,
        span,
    }] = content.as_slice()
    else {
        panic!("expected typed strike: {content:?}");
    };
    assert!(span.end > span.start);
    assert!(inner
        .iter()
        .any(|node| matches!(node, IrInline::Strong { .. })));
    assert!(inner
        .iter()
        .any(|node| matches!(node, IrInline::Code { .. })));
}

#[test]
fn strikethrough_and_italic_bold_can_wrap_legacy_positional_url() {
    let content = single_paragraph(
        ".text {Hello} {https://example.org/docs} weight:{bold} style:{italic} decoration:{strikethrough}\n",
    );
    let [IrInline::Link {
        destination,
        content: linked,
        ..
    }] = content.as_slice()
    else {
        panic!("expected link: {content:?}");
    };
    assert_eq!(destination, "https://example.org/docs");
    let [IrInline::Strikethrough {
        content: strike, ..
    }] = linked.as_slice()
    else {
        panic!("expected strike: {linked:?}");
    };
    let [IrInline::Emphasis {
        content: emphasis, ..
    }] = strike.as_slice()
    else {
        panic!("expected emphasis: {strike:?}");
    };
    assert!(matches!(emphasis.as_slice(), [IrInline::Strong { .. }]));
}

#[test]
fn blank_url_uses_unstyled_rich_text_projection() {
    let content =
        single_paragraph(".text url:{   } decoration:{strikethrough}\n    Hello *world*\n");
    assert!(matches!(
        content.as_slice(),
        [IrInline::Link { destination, content: linked, .. }]
            if destination == "Hello world"
                && matches!(linked.as_slice(), [IrInline::Strikethrough { .. }])
    ));
}

#[test]
fn unsupported_decoration_variants_and_binding_conflicts_fail_atomically() {
    for source in [
        ".text text:{hello} decoration:{underline}\n",
        ".text text:{hello} decoration:{overline}\n",
        ".text text:{hello} decoration:{underoverline}\n",
        ".text text:{hello} decoration:{all}\n",
        ".text text:{hello} decoration:{123}\n",
        ".text text:{hello} decoration:{strikethrough} decoration:{none}\n",
        ".text {hello} {https://example.org} {strikethrough}\n",
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
