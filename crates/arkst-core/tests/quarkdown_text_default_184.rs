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

#[test]
fn text_default_preserves_inline_structure_and_surrounding_content() {
    let result = compile_source("before .text {hello} after\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let [IrNode::Paragraph { content, .. }] = result.ir.nodes.as_slice() else {
        panic!("expected one paragraph: {:?}", result.ir.nodes);
    };
    assert!(matches!(&content[0], IrInline::Text { content, .. } if content == "before "));
    assert!(content.iter().any(|part| matches!(part, IrInline::Text { content, .. } if content == "hello")));
    assert!(matches!(content.last(), Some(IrInline::Text { content, .. }) if content == " after"));
    let wire = serde_json::to_string(&result.ir).expect("serialize");
    let decoded: arkst_core::ir::IrDocument = serde_json::from_str(&wire).expect("deserialize");
    assert_eq!(decoded, result.ir);
}

#[test]
fn text_accepts_named_and_evaluated_body_content() {
    let named = compile_source(".text text:{hello}\n");
    assert!(named.diagnostics.is_empty(), "{named:?}");
    let body = compile_source(".text\n    **bold** and *italic*\n");
    assert!(body.diagnostics.is_empty(), "{body:?}");
    let [IrNode::Paragraph { content, .. }] = body.ir.nodes.as_slice() else {
        panic!("expected inline paragraph: {:?}", body.ir.nodes);
    };
    assert!(content.iter().any(|part| matches!(part, IrInline::Strong { .. })), "{content:?}");
    assert!(content.iter().any(|part| matches!(part, IrInline::Emphasis { .. })), "{content:?}");
}

#[test]
fn text_rejects_unimplemented_style_and_malformed_invocations_atomically() {
    for source in [
        ".text\n",
        ".text {one} text:{two}\n",
        ".text weight:{bold} {one}\n",
        ".text url:{https://example.com} {one}\n",
        ".text {one} {two}\n",
    ] {
        let result = compile_source(source);
        assert!(!result.diagnostics.is_empty(), "{source:?}: {result:?}");
        assert!(result.diagnostics[0].primary.is_some(), "{source:?}: {result:?}");
        assert!(result.ir.nodes.is_empty(), "{source:?}: {result:?}");
    }
}

#[test]
fn text_source_defined_function_keeps_precedence() {
    let result = compile_source(".function {text}\n    custom\n\n.text {original}\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert!(format!("{:?}", result.ir).contains("custom"), "{result:?}");
}
