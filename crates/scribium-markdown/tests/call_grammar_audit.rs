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

#[test]
fn audit_records_current_escaped_delimiter_gap() {
    let escaped_introducer = r"\.foo {x}";
    let output = parse_with_diagnostics(escaped_introducer);
    assert!(output.diagnostics.is_empty(), "{output:?}");
    assert!(!output.document.nodes.iter().any(|node| match node {
        Block::DirectiveCall { .. } => true,
        Block::Paragraph { content, .. } => content
            .iter()
            .any(|inline| matches!(inline, Inline::DirectiveCall { .. })),
        _ => false,
    }));
    assert_eq!(
        output.document.nodes,
        vec![Block::Paragraph {
            content: vec![
                Inline::Text {
                    content: ".foo ".to_string(),
                    span: ByteSpan::new(0, 6),
                },
                Inline::Text {
                    content: "{x}".to_string(),
                    span: ByteSpan::new(6, 9),
                },
            ],
            span: ByteSpan::new(0, 9),
        }]
    );

    let escaped_closing = r".foo {a \} b}";
    let output = parse_with_diagnostics(escaped_closing);
    assert!(output.diagnostics.is_empty(), "{output:?}");
    let call = first_inline_call(&output.document);
    let Inline::DirectiveCall {
        positional_args,
        span,
        ..
    } = call
    else {
        unreachable!()
    };
    assert_eq!(source_slice(escaped_closing, *span), r".foo {a \}");
    assert!(matches!(
        positional_args.as_slice(),
        [Value::Content(content)]
            if content.iter().any(|inline| matches!(
                inline,
                Inline::Text { span, .. } if source_slice(escaped_closing, *span) == r"a \"
            ))
    ));

    let escaped_opening = r".foo {a \{ b}";
    let output = parse_with_diagnostics(escaped_opening);
    assert_eq!(output.diagnostics.len(), 1, "{output:?}");
    assert_eq!(output.diagnostics[0].code, "E2003");
    assert_eq!(
        source_slice(escaped_opening, output.diagnostics[0].span),
        r"{a \{ b}"
    );

    let nested_escaped = r".foo {a \{ nested \} b}";
    let output = parse_with_diagnostics(nested_escaped);
    assert!(output.diagnostics.is_empty(), "{output:?}");
    let Block::DirectiveCall { span, .. } = &output.document.nodes[0] else {
        panic!("expected current parser to close the escaped nested-brace probe")
    };
    assert_eq!(source_slice(nested_escaped, *span), nested_escaped);

    let utf8 = r".foo {한글 \} text}";
    let output = parse_with_diagnostics(utf8);
    assert!(output.diagnostics.is_empty(), "{output:?}");
    let call = first_inline_call(&output.document);
    let Inline::DirectiveCall { span, .. } = call else {
        unreachable!()
    };
    assert_eq!(source_slice(utf8, *span), r".foo {한글 \}");

    let crlf = ".foo {a \\}\r\nb}";
    let output = parse_with_diagnostics(crlf);
    assert!(output.diagnostics.is_empty(), "{output:?}");
    let Block::DirectiveCall { span, .. } = &output.document.nodes[0] else {
        panic!("expected current parser to emit the truncated CRLF call")
    };
    assert_eq!(source_slice(crlf, *span), ".foo {a \\}");
    assert!(matches!(output.document.nodes[1], Block::Paragraph { .. }));

    for source in [
        escaped_introducer,
        escaped_closing,
        escaped_opening,
        nested_escaped,
        utf8,
        crlf,
    ] {
        let markdown = parse_with_mode(source, Mode::Markdown);
        assert!(!markdown.document.nodes.iter().any(|node| match node {
            Block::DirectiveCall { .. } => true,
            Block::Paragraph { content, .. } => content
                .iter()
                .any(|inline| matches!(inline, Inline::DirectiveCall { .. })),
            _ => false,
        }));
    }
}

#[test]
fn audit_records_current_early_rejection_of_positional_after_named() {
    let source = ".foo named:{x} {y}";
    let output = parse_with_diagnostics(source);
    let diagnostic = output
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "E2001")
        .expect("current parser must record its early positional-after-named rejection");
    assert_eq!(
        diagnostic.message,
        "positional argument after named argument is not allowed"
    );
    assert_eq!(source_slice(source, diagnostic.span), "{");
    assert!(matches!(
        output.document.nodes.first(),
        Some(Block::Unsupported { .. })
    ));
}
