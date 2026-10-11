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

fn paragraph_text(source: &str) -> String {
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let [IrNode::Paragraph { content, .. }] = result.ir.nodes.as_slice() else {
        panic!("expected one paragraph: {:?}", result.ir.nodes);
    };
    let [IrInline::Text { content, .. }] = content.as_slice() else {
        panic!("expected one scalar text inline: {content:?}");
    };
    content.clone()
}

#[test]
fn loremipsum_is_a_nonempty_deterministic_zero_argument_scalar() {
    let first = paragraph_text(".loremipsum\n");
    let second = paragraph_text(".loremipsum\n");
    assert_eq!(first, second);
    assert!(first.starts_with("Lorem ipsum dolor sit amet."));
    assert!(first.len() > 100);
    assert!(!first.contains('\n'));
}

#[test]
fn loremipsum_interpolates_between_surrounding_inline_text_without_new_ir_type() {
    let result = compile_source("Before .loremipsum after.\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let [IrNode::Paragraph { content, .. }] = result.ir.nodes.as_slice() else {
        panic!("expected paragraph: {:?}", result.ir.nodes);
    };
    assert!(matches!(&content[0], IrInline::Text { content, .. } if content == "Before "));
    assert!(content.iter().any(|inline| {
        matches!(inline, IrInline::Text { content, .. } if content.starts_with("Lorem ipsum dolor sit amet."))
    }));
    assert!(matches!(content.last(), Some(IrInline::Text { content, .. }) if content == " after."));
    let encoded = serde_json::to_string(&result.ir).expect("serialize IR");
    let decoded: arkst_core::ir::IrDocument = serde_json::from_str(&encoded).expect("deserialize IR");
    assert_eq!(decoded, result.ir);
}

#[test]
fn loremipsum_rejects_all_arguments_and_bodies_atomically() {
    for source in [
        ".loremipsum {unexpected}\n",
        ".loremipsum option:{unexpected}\n",
        ".loremipsum {.grid columns:{0}}\n",
        ".loremipsum\n    body\n",
    ] {
        let result = compile_source(source);
        assert!(!result.diagnostics.is_empty(), "{source}: {result:?}");
        assert!(result.diagnostics[0].primary.is_some(), "{result:?}");
        assert!(result.ir.nodes.is_empty(), "{source}: {result:?}");
    }
}

#[test]
fn source_defined_loremipsum_retains_precedence_over_builtin() {
    let result = compile_source(".function {loremipsum}\n    custom\n\n.loremipsum\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let debug = format!("{:?}", result.ir);
    assert!(debug.contains("custom"), "{debug}");
    assert!(!debug.contains("Lorem ipsum"), "{debug}");
}
