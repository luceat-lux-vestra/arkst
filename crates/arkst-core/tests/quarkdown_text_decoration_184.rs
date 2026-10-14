use arkst_core::ir::{IrInline, IrNode};
use arkst_core::{compile, CompileOptions, SourceId, VirtualProjectBuilder};

fn compile_source(source: &str) -> arkst_core::CompileResult {
    compile_source_with_id(source).0
}

fn compile_source_with_id(source: &str) -> (arkst_core::CompileResult, SourceId) {
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("entry")
        .add_source("main.qd", source)
        .expect("source")
        .build()
        .expect("project");
    let source_id = project
        .sources()
        .get_id(project.entry())
        .expect("source id");
    (compile(&project, &CompileOptions::default()), source_id)
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
fn underline_and_overline_preserve_rich_children_and_call_provenance() {
    let source = ".text decoration:{underline}\n    Hello **bold** and `code`\n";
    let (result, source_id) = compile_source_with_id(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let [IrNode::Paragraph { content, .. }] = result.ir.nodes.as_slice() else {
        panic!("expected one paragraph: {result:?}");
    };
    let [IrInline::Underline {
        content: inner,
        span,
    }] = content.as_slice()
    else {
        panic!("expected underline: {content:?}");
    };
    assert_eq!(span.source_id, source_id);
    assert_eq!(&source[span.start..span.end], source.trim_end());
    assert!(inner
        .iter()
        .any(|node| matches!(node, IrInline::Strong { .. })));
    assert!(inner
        .iter()
        .any(|node| matches!(node, IrInline::Code { .. })));

    let overline = single_paragraph(".text text:{Hello} decoration:{overline}\n");
    assert!(matches!(
        overline.as_slice(),
        [IrInline::Overline { content, .. }]
            if matches!(content.as_slice(), [IrInline::Text { content, .. }] if content == "Hello")
    ));
}

#[test]
fn combined_decoration_values_retain_each_authored_line_semantic() {
    let under_over = single_paragraph(".text text:{Hello} decoration:{underoverline}\n");
    assert!(matches!(
        under_over.as_slice(),
        [IrInline::Underline { content: underline, .. }]
            if matches!(underline.as_slice(), [IrInline::Overline { content, .. }]
                if matches!(content.as_slice(), [IrInline::Text { content, .. }] if content == "Hello"))
    ));

    let all = single_paragraph(".text text:{Hello} decoration:{all}\n");
    assert!(matches!(
        all.as_slice(),
        [IrInline::Underline { content: underline, .. }]
            if matches!(underline.as_slice(), [IrInline::Overline { content: overline, .. }]
                if matches!(overline.as_slice(), [IrInline::Strikethrough { content, .. }]
                    if matches!(content.as_slice(), [IrInline::Text { content, .. }] if content == "Hello")))
    ));

    let styled = single_paragraph(
        ".text text:{Hello} weight:{bold} style:{italic} decoration:{underline}\n",
    );
    assert!(matches!(
        styled.as_slice(),
        [IrInline::Underline { content: underline, .. }]
            if matches!(underline.as_slice(), [IrInline::Emphasis { content, .. }]
                if matches!(content.as_slice(), [IrInline::Strong { .. }]))
    ));
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
fn blank_url_projects_plain_text_through_the_complete_decoration_domain() {
    let content = single_paragraph(".text url:{   } decoration:{all}\n    Hello *world*\n");
    assert!(matches!(
        content.as_slice(),
        [IrInline::Link { destination, content: linked, .. }]
            if destination == "Hello world"
                && matches!(linked.as_slice(), [IrInline::Underline { .. }])
    ));
}

#[test]
fn invalid_decoration_values_and_binding_conflicts_fail_atomically() {
    for source in [
        ".text text:{hello} decoration:{123}\n",
        ".text text:{hello} decoration:{blink}\n",
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
