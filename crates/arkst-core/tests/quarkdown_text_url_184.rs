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

fn single_link(source: &str) -> (String, Vec<IrInline>) {
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{source}: {result:?}");
    let [IrNode::Paragraph { content, .. }] = result.ir.nodes.as_slice() else {
        panic!("expected one inline paragraph: {result:?}");
    };
    let [IrInline::Link {
        content: children,
        destination,
        title: None,
        ..
    }] = content.as_slice()
    else {
        panic!("expected a typed link: {content:?}");
    };
    let wire = serde_json::to_string(&result.ir).expect("serialize");
    let decoded: arkst_core::ir::IrDocument = serde_json::from_str(&wire).expect("deserialize");
    assert_eq!(decoded, result.ir);
    (destination.clone(), children.clone())
}

#[test]
fn text_nonblank_url_uses_typed_link_and_retains_nested_emphasis() {
    let (url, content) =
        single_link(".text url:{https://example.org/notes}\n    **Bold** and *italic*\n");
    assert_eq!(url, "https://example.org/notes");
    assert!(
        content
            .iter()
            .any(|part| matches!(part, IrInline::Strong { .. })),
        "{content:?}"
    );
    assert!(
        content
            .iter()
            .any(|part| matches!(part, IrInline::Emphasis { .. })),
        "{content:?}"
    );
}

#[test]
fn text_explicit_named_text_and_url_share_normal_binding() {
    let (url, content) = single_link(".text text:{hello} url:{https://example.org/hello}\n");
    assert_eq!(url, "https://example.org/hello");
    assert!(matches!(&content[..], [IrInline::Text { content, .. }] if content == "hello"));
}

#[test]
fn text_positional_content_and_url_are_valid_and_typed() {
    let (url, content) = single_link(".text {hello} {https://example.org/positional}\n");
    assert_eq!(url, "https://example.org/positional");
    assert!(matches!(&content[..], [IrInline::Text { content, .. }] if content == "hello"));
}

#[test]
fn text_blank_url_derives_destination_from_rich_inline_content() {
    let (url, content) = single_link(".text url:{   }\n    **Bold**\n");
    assert_eq!(url, "Bold");
    assert!(matches!(content.as_slice(), [IrInline::Strong { .. }]));
}

#[test]
fn text_blank_url_derives_code_span_without_reparsing() {
    let (url, content) = single_link(".text url:{   }\n    `https://example.org/derived`\n");
    assert_eq!(url, "https://example.org/derived");
    assert!(matches!(content.as_slice(), [IrInline::Code { .. }]));
}

#[test]
fn text_none_url_still_preserves_unstyled_inline_content() {
    let result = compile_source(".text {**Bold**} url:{.none}\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let [IrNode::Paragraph { content, .. }] = result.ir.nodes.as_slice() else {
        panic!("expected one paragraph: {result:?}");
    };
    assert!(matches!(content.as_slice(), [IrInline::Strong { .. }]));
}

#[test]
fn text_url_invalid_and_body_collision_fail_atomically() {
    for source in [
        ".text url:{   }\n",
        ".text url:{.none} text:{hello} {world}\n",
        ".text text:{hello} {world}\n",
        ".text weight:{heavy} {hello}\n",
        ".text url:{https://example.org} {hello} {world}\n",
    ] {
        let result = compile_source(source);
        assert!(!result.diagnostics.is_empty(), "{source:?}: {result:?}");
        assert!(result.ir.nodes.is_empty(), "{source:?}: {result:?}");
    }
}

#[test]
fn user_text_function_still_shadows_builtin_url_link() {
    let result = compile_source(
        ".function {text}\n    custom\n\n.text url:{https://example.org} {original}\n",
    );
    // Shadowing retains the user-defined function's own binding contract;
    // the added builtin URL parameter must not leak into the override.
    assert!(!result.diagnostics.is_empty(), "{result:?}");
}
