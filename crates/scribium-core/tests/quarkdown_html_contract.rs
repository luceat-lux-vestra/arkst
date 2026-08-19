use scribium_core::ir::{IrInline, IrNode, IrValue};
use scribium_core::{compile, CompileOptions, VirtualProjectBuilder};

fn compile_source(source: &str) -> scribium_core::CompileResult {
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("valid path")
        .add_source("main.qd", source)
        .expect("valid path")
        .build()
        .expect("valid project");
    compile(&project, &CompileOptions::default())
}

#[test]
fn current_evaluator_preserves_unknown_html_block_call_without_native_semantics() {
    let result = compile_source(".html {<em>world</em>}\n");
    assert!(result.diagnostics.iter().any(|diag| diag.code == "E3010"));
    let IrNode::FunctionCall {
        name,
        positional_args,
        body: None,
        ..
    } = &result.ir.nodes[0]
    else {
        panic!(
            "expected preserved unresolved block call, got {:?}",
            result.ir
        );
    };
    assert_eq!(name, "html");
    let [IrValue::Content(content)] = positional_args.as_slice() else {
        panic!("expected one content argument, got {positional_args:?}");
    };
    assert!(matches!(
        content.as_slice(),
        [IrNode::Paragraph { content, .. }]
            if matches!(content.as_slice(), [IrInline::Text { content, .. }] if content == "<em>world</em>")
    ));
}

#[test]
fn current_evaluator_preserves_unknown_html_inline_call_order() {
    let result = compile_source("**Hello** .html {<em>world</em>}!\n");
    assert!(result.diagnostics.iter().any(|diag| diag.code == "E3010"));
    let IrNode::Paragraph { content, .. } = &result.ir.nodes[0] else {
        panic!("expected paragraph, got {:?}", result.ir);
    };
    assert!(matches!(
        content.as_slice(),
        [
            IrInline::Strong { .. },
            IrInline::Text { content: space, .. },
            IrInline::DirectiveCall { name, .. },
            IrInline::Text { content: exclamation, .. },
        ] if space == " " && name == "html" && exclamation == "!"
    ));
}

#[test]
fn current_evaluator_rejects_raw_html_in_html_body_at_lowering_boundary() {
    let result = compile_source(".html\n    <div>\n        Hello\n    </div>\n");
    assert!(result.diagnostics.iter().any(|diag| diag.code == "E8001"));
    let IrNode::FunctionCall {
        name,
        positional_args,
        body: Some(body),
        ..
    } = &result.ir.nodes[0]
    else {
        panic!("expected preserved block call, got {:?}", result.ir);
    };
    assert_eq!(name, "html");
    assert!(positional_args.is_empty());
    assert!(body.is_empty());
}
