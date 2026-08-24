use scribium_markdown::{parse_with_diagnostics, parse_with_mode, Block, Inline, Mode, Value};
use scribium_source::ByteSpan;

fn source_slice(source: &str, span: ByteSpan) -> &str {
    assert!(span.is_valid_for(source));
    &source[span.start..span.end]
}

fn first_inline_call(document: &scribium_markdown::Document) -> &Inline {
    let Block::Paragraph { content, .. } = &document.nodes[0] else {
        panic!("expected paragraph, got {:?}", document.nodes)
    };
    content
        .iter()
        .find(|inline| matches!(inline, Inline::DirectiveCall { .. }))
        .expect("expected inline call")
}

#[test]
fn audit_preserves_multiline_nested_and_named_argument_spans() {
    let source = ".outer {\n  .inner {값}\n} named:{\n  .sum {2} {1}\n}\n";
    let output = parse_with_diagnostics(source);
    assert!(output.diagnostics.is_empty(), "{output:?}");

    let Block::DirectiveCall {
        name,
        name_span,
        positional_args,
        named_args,
        span,
        ..
    } = &output.document.nodes[0]
    else {
        panic!("expected block call, got {:?}", output.document.nodes)
    };
    assert_eq!(name, "outer");
    assert_eq!(source_slice(source, *name_span), ".outer");
    assert_eq!(source_slice(source, *span), source.trim_end());
    assert!(matches!(positional_args.first(), Some(Value::Content(_))));
    assert_eq!(named_args.len(), 1);
    assert_eq!(source_slice(source, named_args[0].name_span), "named");
    assert_eq!(
        source_slice(source, named_args[0].span),
        "named:{\n  .sum {2} {1}\n}"
    );
}

#[test]
fn audit_preserves_crlf_continuation_and_inline_boundary() {
    let source = "앞 .call {a} \\\r\n\tsecond:{b} 뒤\r\n";
    let output = parse_with_diagnostics(source);
    assert!(output.diagnostics.is_empty(), "{output:?}");
    let call = first_inline_call(&output.document);
    let Inline::DirectiveCall {
        name,
        named_args,
        span,
        ..
    } = call
    else {
        unreachable!()
    };
    assert_eq!(name, "call");
    assert_eq!(named_args.len(), 1);
    assert_eq!(source_slice(source, *span), ".call {a} \\\r\n\tsecond:{b}");
    assert!(source_slice(source, *span).contains("\r\n"));
}

#[test]
fn audit_preserves_chain_segments_and_tight_wrapper_span() {
    let chain_source = "prefix .a {x}::b {y} suffix\n";
    let chain_output = parse_with_diagnostics(chain_source);
    assert!(chain_output.diagnostics.is_empty(), "{chain_output:?}");
    let chain = first_inline_call(&chain_output.document);
    let Inline::DirectiveCall {
        head_span,
        chain: segments,
        span,
        ..
    } = chain
    else {
        unreachable!()
    };
    assert_eq!(source_slice(chain_source, *head_span), ".a {x}");
    assert_eq!(source_slice(chain_source, segments[0].span), "b {y}");
    assert_eq!(source_slice(chain_source, *span), ".a {x}::b {y}");

    let tight_source = "H{.text {2}}O\n";
    let tight_output = parse_with_diagnostics(tight_source);
    assert!(tight_output.diagnostics.is_empty(), "{tight_output:?}");
    let tight = first_inline_call(&tight_output.document);
    let Inline::DirectiveCall { span, .. } = tight else {
        unreachable!()
    };
    assert_eq!(source_slice(tight_source, *span), "{.text {2}}");
}

#[test]
fn audit_keeps_implicit_reference_structural_and_modes_isolated() {
    let source = ".function {identity}\n    .1\n";
    let output = parse_with_diagnostics(source);
    assert!(output.diagnostics.is_empty(), "{output:?}");
    let Block::DirectiveCall { body, .. } = &output.document.nodes[0] else {
        panic!("expected function block")
    };
    let body = body.as_ref().expect("function body");
    assert!(matches!(
        body.first(),
        Some(Block::DirectiveCall { name, span, .. })
            if name == "1" && source_slice(source, *span) == ".1"
    ));

    let markdown = parse_with_mode(".note {x}\n", Mode::Markdown);
    assert!(!markdown.document.nodes.iter().any(|node| {
        matches!(node, Block::DirectiveCall { .. })
            || matches!(
                node,
                Block::Paragraph { content, .. }
                    if content.iter().any(|inline| matches!(inline, Inline::DirectiveCall { .. }))
            )
    }));
}

#[test]
fn audit_malformed_braces_are_structured_and_source_backed() {
    let source = ".foo {unterminated";
    let output = parse_with_diagnostics(source);
    let diagnostic = output
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "E2003")
        .expect("expected unclosed-argument diagnostic");
    assert_eq!(source_slice(source, diagnostic.span), "{unterminated");
}
