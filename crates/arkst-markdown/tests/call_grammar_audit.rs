use arkst_markdown::{
    parse_with_diagnostics, parse_with_mode, Block, CallArgument, Inline, Mode, Value,
};
use arkst_source::ByteSpan;

fn source_slice(source: &str, span: ByteSpan) -> &str {
    assert!(span.is_valid_for(source));
    &source[span.start..span.end]
}

fn positional_args(arguments: &[CallArgument]) -> Vec<&Value> {
    arguments
        .iter()
        .filter_map(|argument| match argument {
            CallArgument::Positional { value, .. } => Some(value),
            CallArgument::Named(_) => None,
        })
        .collect()
}

fn named_args(arguments: &[CallArgument]) -> Vec<&arkst_markdown::ast::NamedArg> {
    arguments
        .iter()
        .filter_map(|argument| match argument {
            CallArgument::Positional { .. } => None,
            CallArgument::Named(argument) => Some(argument),
        })
        .collect()
}

fn first_inline_call(document: &arkst_markdown::Document) -> &Inline {
    let Block::Paragraph { content, .. } = &document.nodes[0] else {
        panic!("expected paragraph, got {:?}", document.nodes)
    };
    content
        .iter()
        .find(|inline| matches!(inline, Inline::DirectiveCall { .. }))
        .expect("expected inline call")
}

fn assert_markdown_isolated(source: &str) {
    let markdown = parse_with_mode(source, Mode::Markdown);
    assert!(!markdown.document.nodes.iter().any(|node| match node {
        Block::DirectiveCall { .. } => true,
        Block::Paragraph { content, .. } => content
            .iter()
            .any(|inline| matches!(inline, Inline::DirectiveCall { .. })),
        _ => false,
    }));
}

struct NestedTightExpectation<'a> {
    outer_call: &'a str,
    outer_argument: &'a str,
    prefix: &'a str,
    wrapper: &'a str,
    head: &'a str,
    nested_argument: &'a str,
    nested_value: &'a str,
    suffix: &'a str,
}

fn assert_nested_tight_content(
    source: &str,
    outer_span: ByteSpan,
    arguments: &[CallArgument],
    expected: NestedTightExpectation<'_>,
) {
    assert_eq!(source_slice(source, outer_span), expected.outer_call);
    let CallArgument::Positional {
        value: Value::Content(content),
        span: outer_argument_span,
    } = &arguments[0]
    else {
        panic!("expected the outer positional content argument")
    };
    assert_eq!(
        source_slice(source, *outer_argument_span),
        expected.outer_argument
    );
    assert!(matches!(
        content.as_slice(),
        [
            Inline::Text { .. },
            Inline::DirectiveCall { .. },
            Inline::Text { .. }
        ]
    ));

    let Inline::Text {
        content: prefix,
        span: prefix_span,
    } = &content[0]
    else {
        unreachable!()
    };
    let Inline::DirectiveCall {
        name,
        name_span,
        head_span,
        arguments: nested_arguments,
        span: nested_span,
        ..
    } = &content[1]
    else {
        unreachable!()
    };
    let Inline::Text {
        content: suffix,
        span: suffix_span,
    } = &content[2]
    else {
        unreachable!()
    };

    assert_eq!(prefix, expected.prefix);
    assert_eq!(source_slice(source, *prefix_span), expected.prefix);
    assert_eq!(name, "inner");
    assert_eq!(source_slice(source, *nested_span), expected.wrapper);
    assert_eq!(source_slice(source, *head_span), expected.head);
    assert_eq!(source_slice(source, *name_span), ".inner");
    assert_eq!(nested_arguments.len(), 1);
    let CallArgument::Positional {
        value,
        span: nested_argument_span,
    } = &nested_arguments[0]
    else {
        panic!("expected the inner positional argument")
    };
    match value {
        Value::Identifier(value) => assert_eq!(value, expected.nested_value),
        Value::Content(content) => {
            let [Inline::Text {
                content: value,
                span,
            }] = content.as_slice()
            else {
                panic!("expected multibyte argument content to remain one text inline")
            };
            assert_eq!(value, expected.nested_value);
            assert_eq!(source_slice(source, *span), expected.nested_value);
        }
        value => panic!("unexpected inner argument value: {value:?}"),
    }
    assert_eq!(
        source_slice(source, *nested_argument_span),
        expected.nested_argument
    );
    assert_eq!(suffix, expected.suffix);
    assert_eq!(source_slice(source, *suffix_span), expected.suffix);
}

#[test]
fn audit_preserves_nested_tight_call_wrapper_inside_content_argument() {
    let source = ".outer {H{.inner {x}}O}\n";
    let output = parse_with_diagnostics(source);
    assert!(output.diagnostics.is_empty(), "{output:?}");
    let Block::DirectiveCall {
        arguments, span, ..
    } = &output.document.nodes[0]
    else {
        panic!("expected outer block call, got {:?}", output.document.nodes)
    };
    assert_nested_tight_content(
        source,
        *span,
        arguments,
        NestedTightExpectation {
            outer_call: ".outer {H{.inner {x}}O}",
            outer_argument: "{H{.inner {x}}O}",
            prefix: "H",
            wrapper: "{.inner {x}}",
            head: ".inner {x}",
            nested_argument: "{x}",
            nested_value: "x",
            suffix: "O",
        },
    );
    assert_markdown_isolated(source);

    let inline_source = "prefix .outer {H{.inner {x}}O} suffix\n";
    let inline_output = parse_with_diagnostics(inline_source);
    assert!(inline_output.diagnostics.is_empty(), "{inline_output:?}");
    let Block::Paragraph { content, .. } = &inline_output.document.nodes[0] else {
        panic!("expected inline paragraph")
    };
    let Inline::DirectiveCall {
        arguments, span, ..
    } = content
        .iter()
        .find(|inline| matches!(inline, Inline::DirectiveCall { name, .. } if name == "outer"))
        .expect("expected inline outer call")
    else {
        unreachable!()
    };
    assert_nested_tight_content(
        inline_source,
        *span,
        arguments,
        NestedTightExpectation {
            outer_call: ".outer {H{.inner {x}}O}",
            outer_argument: "{H{.inner {x}}O}",
            prefix: "H",
            wrapper: "{.inner {x}}",
            head: ".inner {x}",
            nested_argument: "{x}",
            nested_value: "x",
            suffix: "O",
        },
    );
    assert_markdown_isolated(inline_source);
}

#[test]
fn audit_preserves_nested_tight_utf8_and_crlf_provenance() {
    let utf8_source = ".outer {앞{.inner {값}}뒤}\n";
    let utf8_output = parse_with_diagnostics(utf8_source);
    assert!(utf8_output.diagnostics.is_empty(), "{utf8_output:?}");
    let Block::DirectiveCall {
        arguments, span, ..
    } = &utf8_output.document.nodes[0]
    else {
        panic!("expected UTF-8 outer block call")
    };
    assert_nested_tight_content(
        utf8_source,
        *span,
        arguments,
        NestedTightExpectation {
            outer_call: ".outer {앞{.inner {값}}뒤}",
            outer_argument: "{앞{.inner {값}}뒤}",
            prefix: "앞",
            wrapper: "{.inner {값}}",
            head: ".inner {값}",
            nested_argument: "{값}",
            nested_value: "값",
            suffix: "뒤",
        },
    );
    assert_eq!(
        source_slice(utf8_source, *span),
        ".outer {앞{.inner {값}}뒤}"
    );
    assert_markdown_isolated(utf8_source);

    let crlf_source = ".outer {H{.inner {x}}O}\r\n";
    assert!(crlf_source
        .as_bytes()
        .windows(2)
        .any(|pair| pair == b"\r\n"));
    let crlf_output = parse_with_diagnostics(crlf_source);
    assert!(crlf_output.diagnostics.is_empty(), "{crlf_output:?}");
    let Block::DirectiveCall {
        arguments, span, ..
    } = &crlf_output.document.nodes[0]
    else {
        panic!("expected CRLF outer block call")
    };
    assert_nested_tight_content(
        crlf_source,
        *span,
        arguments,
        NestedTightExpectation {
            outer_call: ".outer {H{.inner {x}}O}",
            outer_argument: "{H{.inner {x}}O}",
            prefix: "H",
            wrapper: "{.inner {x}}",
            head: ".inner {x}",
            nested_argument: "{x}",
            nested_value: "x",
            suffix: "O",
        },
    );
    assert_eq!(source_slice(crlf_source, *span), ".outer {H{.inner {x}}O}");
    assert_markdown_isolated(crlf_source);
}

#[test]
fn audit_keeps_ordinary_braces_as_content_and_does_not_promote_markdown() {
    let source = ".outer {H{ordinary}O}\n";
    let output = parse_with_diagnostics(source);
    assert!(output.diagnostics.is_empty(), "{output:?}");
    let Block::DirectiveCall { arguments, .. } = &output.document.nodes[0] else {
        panic!("expected outer block call")
    };
    let CallArgument::Positional {
        value: Value::Content(content),
        ..
    } = &arguments[0]
    else {
        panic!("expected outer content argument")
    };
    assert!(content
        .iter()
        .all(|inline| matches!(inline, Inline::Text { .. })));
    let text_slices: Vec<_> = content
        .iter()
        .map(|inline| match inline {
            Inline::Text { span, .. } => source_slice(source, *span),
            _ => unreachable!(),
        })
        .collect();
    assert_eq!(text_slices.concat(), "H{ordinary}O");
    assert_markdown_isolated(source);

    let markdown = parse_with_mode(".outer {H{.inner {x}}O}\n", Mode::Markdown);
    assert!(matches!(
        markdown.document.nodes.as_slice(),
        [Block::Paragraph { content, .. }]
            if content.iter().all(|inline| !matches!(inline, Inline::DirectiveCall { .. }))
    ));
}

#[test]
fn audit_preserves_multiline_nested_and_named_argument_spans() {
    let source = ".outer {\n  .inner {값}\n} named:{\n  .sum {2} {1}\n}\n";
    let output = parse_with_diagnostics(source);
    assert!(output.diagnostics.is_empty(), "{output:?}");

    let Block::DirectiveCall {
        name,
        name_span,
        arguments,
        span,
        ..
    } = &output.document.nodes[0]
    else {
        panic!("expected block call, got {:?}", output.document.nodes)
    };
    assert_eq!(name, "outer");
    assert_eq!(source_slice(source, *name_span), ".outer");
    assert_eq!(source_slice(source, *span), source.trim_end());
    let positional_args = positional_args(arguments);
    let named_args = named_args(arguments);
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
        arguments,
        span,
        ..
    } = call
    else {
        unreachable!()
    };
    assert_eq!(name, "call");
    let named_args = named_args(arguments);
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

    let prefix_source = ".1abc\n";
    let prefix_output = parse_with_diagnostics(prefix_source);
    assert!(prefix_output.diagnostics.is_empty(), "{prefix_output:?}");
    let Block::Paragraph { content, span } = &prefix_output.document.nodes[0] else {
        panic!("expected numeric call prefix and source remainder")
    };
    assert_eq!(source_slice(prefix_source, *span), ".1abc");
    assert!(content.iter().any(|inline| {
        matches!(inline, Inline::DirectiveCall { name, span, .. }
            if name == "1" && source_slice(prefix_source, *span) == ".1")
    }));
    assert!(content.iter().any(|inline| {
        matches!(inline, Inline::Text { span, .. } if source_slice(prefix_source, *span) == "abc")
    }));

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

fn assert_escaped_argument(
    source: &str,
    arguments: &[CallArgument],
    argument_span: ByteSpan,
    content_span: ByteSpan,
    expected_content: &str,
) {
    let [CallArgument::Positional {
        value: Value::Content(content),
        span,
    }] = arguments
    else {
        panic!("expected one source-backed positional content argument")
    };
    assert_eq!(*span, argument_span);
    assert_eq!(
        source_slice(source, *span),
        &source[argument_span.start..argument_span.end]
    );
    assert_eq!(source_slice(source, content_span), expected_content);
    let mut source_content = String::new();
    for inline in content {
        let span = match inline {
            Inline::Text { span, .. } | Inline::HardBreak { span } | Inline::SoftBreak { span } => {
                *span
            }
            inline => panic!("expected source-backed escaped argument content: {inline:?}"),
        };
        source_content.push_str(source_slice(source, span));
    }
    assert_eq!(source_content, expected_content);
    assert!(argument_span.is_valid_for(source));
    assert!(content_span.is_valid_for(source));
    assert!(source.is_char_boundary(content_span.start));
    assert!(source.is_char_boundary(content_span.end));
}

#[test]
fn audit_verifies_escaped_delimiter_source_preservation_and_mode_isolation() {
    let block_source = ".foo {a \\} b}\ntrailing source\n";
    let block_output = parse_with_diagnostics(block_source);
    assert!(block_output.diagnostics.is_empty(), "{block_output:?}");
    let Block::DirectiveCall {
        name,
        name_span,
        arguments,
        span,
        ..
    } = &block_output.document.nodes[0]
    else {
        panic!("expected complete escaped-closing block call")
    };
    assert_eq!(name, "foo");
    assert_eq!(*name_span, ByteSpan::new(0, 4));
    assert_eq!(*span, ByteSpan::new(0, 13));
    assert_eq!(source_slice(block_source, *span), ".foo {a \\} b}");
    assert_escaped_argument(
        block_source,
        arguments,
        ByteSpan::new(5, 13),
        ByteSpan::new(6, 12),
        r"a \} b",
    );
    assert!(block_output.document.nodes.iter().any(|node| matches!(
        node,
        Block::Paragraph { span, .. }
            if source_slice(block_source, *span).contains("trailing source")
    )));
    assert_markdown_isolated(block_source);

    let cases = [
        ("prefix .foo {a \\{ b} suffix\n", r"a \{ b"),
        (
            "prefix .foo {a \\{ nested \\} b} suffix\n",
            r"a \{ nested \} b",
        ),
        ("prefix .foo {한글 \\} text} suffix\n", "한글 \\} text"),
        ("prefix .foo {a \\}\r\nb} suffix\r\n", "a \\}\r\nb"),
    ];
    for (source, expected_content) in cases {
        let output = parse_with_diagnostics(source);
        assert!(output.diagnostics.is_empty(), "{output:?}");
        let call_start = source.find(".foo").expect("call start");
        let call_end = source.find(" suffix").expect("call end");
        let call_span = ByteSpan::new(call_start, call_end);
        let call = first_inline_call(&output.document);
        let Inline::DirectiveCall {
            name,
            name_span,
            arguments,
            span,
            ..
        } = call
        else {
            unreachable!()
        };
        assert_eq!(name, "foo");
        assert_eq!(*name_span, ByteSpan::new(call_start, call_start + 4));
        assert_eq!(*span, call_span);
        assert_eq!(source_slice(source, *span), &source[call_start..call_end]);
        assert_escaped_argument(
            source,
            arguments,
            ByteSpan::new(call_start + 5, call_end),
            ByteSpan::new(call_start + 6, call_end - 1),
            expected_content,
        );
        let Block::Paragraph { content, .. } = &output.document.nodes[0] else {
            panic!("expected inline paragraph")
        };
        assert!(content.iter().any(|inline| matches!(
            inline,
            Inline::Text { span, .. } if source_slice(source, *span) == "prefix "
        )));
        assert!(content.iter().any(|inline| matches!(
            inline,
            Inline::Text { span, .. } if source_slice(source, *span) == " suffix"
        )));
        assert_markdown_isolated(source);
    }

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
    assert_markdown_isolated(escaped_introducer);
}

#[test]
fn audit_escaped_opening_brace_keeps_block_body_lifecycle_and_spans() {
    let source = ".foo {a \\{ b}\n  body\n";
    let output = parse_with_diagnostics(source);
    assert!(output.diagnostics.is_empty(), "{output:?}");
    let Block::DirectiveCall {
        name,
        name_span,
        arguments,
        body: Some(body),
        raw_body: Some(raw_body),
        span,
        ..
    } = &output.document.nodes[0]
    else {
        panic!("expected escaped-opening block call with an owned body")
    };
    assert_eq!(name, "foo");
    assert_eq!(*name_span, ByteSpan::new(0, 4));
    assert_eq!(*span, ByteSpan::new(0, 20));
    assert_eq!(source_slice(source, *span), ".foo {a \\{ b}\n  body");
    assert_escaped_argument(
        source,
        arguments,
        ByteSpan::new(5, 13),
        ByteSpan::new(6, 12),
        r"a \{ b",
    );
    assert_eq!(raw_body.span, ByteSpan::new(13, 21));
    assert_eq!(
        raw_body
            .source
            .slice(raw_body.span)
            .expect("raw body source"),
        "\n  body\n"
    );
    let [Block::Paragraph {
        content,
        span: body_span,
    }] = body.as_slice()
    else {
        panic!("expected one source-backed body paragraph")
    };
    assert_eq!(*body_span, ByteSpan::new(16, 20));
    let argument_span = match &arguments[0] {
        CallArgument::Positional { span, .. } => *span,
        CallArgument::Named(_) => panic!("expected positional escaped argument"),
    };
    let [Inline::Text {
        span: text_span, ..
    }] = content.as_slice()
    else {
        panic!("expected one source-backed body text node")
    };
    assert_eq!(*text_span, ByteSpan::new(16, 20));
    assert_eq!(source_slice(source, *body_span), "body");
    assert_eq!(source_slice(source, *text_span), "body");
    for span in [*span, argument_span, raw_body.span, *body_span, *text_span] {
        assert!(span.is_valid_for(source));
        assert!(source.is_char_boundary(span.start));
        assert!(source.is_char_boundary(span.end));
    }
    assert_markdown_isolated(source);
}

#[derive(Debug, Clone, Copy)]
enum ExpectedArgument {
    Positional {
        span: ByteSpan,
    },
    Named {
        name: &'static str,
        name_span: ByteSpan,
        value_span: ByteSpan,
        span: ByteSpan,
    },
}

fn assert_ordered_arguments(
    source: &str,
    arguments: &[CallArgument],
    expected: &[ExpectedArgument],
) {
    assert_eq!(arguments.len(), expected.len(), "{source:?}");
    for (argument, expected) in arguments.iter().zip(expected) {
        match (argument, expected) {
            (
                CallArgument::Positional { span, .. },
                ExpectedArgument::Positional {
                    span: expected_span,
                },
            ) => assert_eq!(*span, *expected_span, "{source:?}"),
            (
                CallArgument::Named(argument),
                ExpectedArgument::Named {
                    name,
                    name_span,
                    value_span,
                    span,
                },
            ) => {
                assert_eq!(argument.name, *name, "{source:?}");
                assert_eq!(argument.name_span, *name_span, "{source:?}");
                assert_eq!(argument.value_span, *value_span, "{source:?}");
                assert_eq!(argument.span, *span, "{source:?}");
                assert_eq!(source_slice(source, argument.name_span), *name);
            }
            (actual, expected) => panic!("argument shape mismatch: {actual:?} != {expected:?}"),
        }
    }
}

#[test]
fn audit_preserves_ordered_mixed_arguments_until_binder_validation() {
    let block_cases = [
        (
            ".foo first:{x} {y}",
            vec![
                ExpectedArgument::Named {
                    name: "first",
                    name_span: ByteSpan::new(5, 10),
                    value_span: ByteSpan::new(11, 14),
                    span: ByteSpan::new(5, 14),
                },
                ExpectedArgument::Positional {
                    span: ByteSpan::new(15, 18),
                },
            ],
        ),
        (
            ".foo {x} second:{y}",
            vec![
                ExpectedArgument::Positional {
                    span: ByteSpan::new(5, 8),
                },
                ExpectedArgument::Named {
                    name: "second",
                    name_span: ByteSpan::new(9, 15),
                    value_span: ByteSpan::new(16, 19),
                    span: ByteSpan::new(9, 19),
                },
            ],
        ),
        (
            ".foo {a} {b}",
            vec![
                ExpectedArgument::Positional {
                    span: ByteSpan::new(5, 8),
                },
                ExpectedArgument::Positional {
                    span: ByteSpan::new(9, 12),
                },
            ],
        ),
        (
            ".foo first:{a} second:{b}",
            vec![
                ExpectedArgument::Named {
                    name: "first",
                    name_span: ByteSpan::new(5, 10),
                    value_span: ByteSpan::new(11, 14),
                    span: ByteSpan::new(5, 14),
                },
                ExpectedArgument::Named {
                    name: "second",
                    name_span: ByteSpan::new(15, 21),
                    value_span: ByteSpan::new(22, 25),
                    span: ByteSpan::new(15, 25),
                },
            ],
        ),
        (
            ".foo a:{1} {2} b:{3} {4}",
            vec![
                ExpectedArgument::Named {
                    name: "a",
                    name_span: ByteSpan::new(5, 6),
                    value_span: ByteSpan::new(7, 10),
                    span: ByteSpan::new(5, 10),
                },
                ExpectedArgument::Positional {
                    span: ByteSpan::new(11, 14),
                },
                ExpectedArgument::Named {
                    name: "b",
                    name_span: ByteSpan::new(15, 16),
                    value_span: ByteSpan::new(17, 20),
                    span: ByteSpan::new(15, 20),
                },
                ExpectedArgument::Positional {
                    span: ByteSpan::new(21, 24),
                },
            ],
        ),
    ];

    for (source, expected) in block_cases {
        let output = parse_with_diagnostics(source);
        assert!(output.diagnostics.is_empty(), "{output:?}");
        let Block::DirectiveCall {
            name,
            name_span,
            arguments,
            span,
            ..
        } = &output.document.nodes[0]
        else {
            panic!("expected block call, got {:?}", output.document.nodes)
        };
        assert_eq!(name, "foo");
        assert_eq!(*name_span, ByteSpan::new(0, 4));
        assert_eq!(*span, ByteSpan::new(0, source.len()));
        assert_eq!(source_slice(source, *span), source);
        assert_ordered_arguments(source, arguments, &expected);
        assert_markdown_isolated(source);
    }

    let inline_source = "prefix .foo first:{x} {y} suffix\n";
    let inline_output = parse_with_diagnostics(inline_source);
    assert!(inline_output.diagnostics.is_empty(), "{inline_output:?}");
    let inline_call = first_inline_call(&inline_output.document);
    let Inline::DirectiveCall {
        name,
        name_span,
        arguments,
        span,
        ..
    } = inline_call
    else {
        unreachable!()
    };
    assert_eq!(name, "foo");
    assert_eq!(source_slice(inline_source, *name_span), ".foo");
    assert_eq!(source_slice(inline_source, *span), ".foo first:{x} {y}");
    assert_ordered_arguments(
        inline_source,
        arguments,
        &[
            ExpectedArgument::Named {
                name: "first",
                name_span: ByteSpan::new(12, 17),
                value_span: ByteSpan::new(18, 21),
                span: ByteSpan::new(12, 21),
            },
            ExpectedArgument::Positional {
                span: ByteSpan::new(22, 25),
            },
        ],
    );
    let Block::Paragraph { content, .. } = &inline_output.document.nodes[0] else {
        panic!("expected inline paragraph")
    };
    assert!(content.iter().any(|inline| matches!(
        inline,
        Inline::Text { span, .. } if source_slice(inline_source, *span) == "prefix "
    )));
    assert!(content.iter().any(|inline| matches!(
        inline,
        Inline::Text { span, .. } if source_slice(inline_source, *span) == " suffix"
    )));
    assert_markdown_isolated(inline_source);

    let chain_source = ".a {x}::b first:{y} {z}";
    let chain_output = parse_with_diagnostics(chain_source);
    assert!(chain_output.diagnostics.is_empty(), "{chain_output:?}");
    let Block::DirectiveCall {
        arguments: head_arguments,
        head_span,
        chain,
        span,
        ..
    } = &chain_output.document.nodes[0]
    else {
        panic!("expected chained block call")
    };
    assert_eq!(*head_span, ByteSpan::new(0, 6));
    assert_eq!(*span, ByteSpan::new(0, chain_source.len()));
    assert_ordered_arguments(
        chain_source,
        head_arguments,
        &[ExpectedArgument::Positional {
            span: ByteSpan::new(3, 6),
        }],
    );
    assert_eq!(chain.len(), 1);
    assert_eq!(chain[0].name, "b");
    assert_eq!(chain[0].name_span, ByteSpan::new(8, 9));
    assert_eq!(chain[0].span, ByteSpan::new(8, chain_source.len()));
    assert_ordered_arguments(
        chain_source,
        &chain[0].arguments,
        &[
            ExpectedArgument::Named {
                name: "first",
                name_span: ByteSpan::new(10, 15),
                value_span: ByteSpan::new(16, 19),
                span: ByteSpan::new(10, 19),
            },
            ExpectedArgument::Positional {
                span: ByteSpan::new(20, 23),
            },
        ],
    );
    assert_markdown_isolated(chain_source);

    let crlf_source = ".foo first:{한글} {값}\r\n";
    let crlf_output = parse_with_diagnostics(crlf_source);
    assert!(crlf_output.diagnostics.is_empty(), "{crlf_output:?}");
    let Block::DirectiveCall {
        name_span,
        arguments,
        span,
        ..
    } = &crlf_output.document.nodes[0]
    else {
        panic!("expected CRLF mixed call")
    };
    assert_eq!(*name_span, ByteSpan::new(0, 4));
    assert_eq!(*span, ByteSpan::new(0, 25));
    assert_eq!(source_slice(crlf_source, *span), ".foo first:{한글} {값}");
    assert_ordered_arguments(
        crlf_source,
        arguments,
        &[
            ExpectedArgument::Named {
                name: "first",
                name_span: ByteSpan::new(5, 10),
                value_span: ByteSpan::new(11, 19),
                span: ByteSpan::new(5, 19),
            },
            ExpectedArgument::Positional {
                span: ByteSpan::new(20, 25),
            },
        ],
    );
    assert_markdown_isolated(crlf_source);
}

#[test]
fn audit_aligns_named_argument_identifier_lexing_and_spans() {
    for (source, expected_name, expected_name_span, expected_argument_span) in [
        (
            ".foo name:{x}\n",
            "name",
            ByteSpan::new(5, 9),
            ByteSpan::new(5, 13),
        ),
        (
            ".foo 1:{x}\n",
            "1",
            ByteSpan::new(5, 6),
            ByteSpan::new(5, 10),
        ),
        (
            ".foo 10:{x}\n",
            "10",
            ByteSpan::new(5, 7),
            ByteSpan::new(5, 11),
        ),
        (
            ".foo name:{한글}\r\n",
            "name",
            ByteSpan::new(5, 9),
            ByteSpan::new(5, 18),
        ),
    ] {
        let output = parse_with_diagnostics(source);
        assert!(output.diagnostics.is_empty(), "{output:?}");
        let Block::DirectiveCall { arguments, .. } = &output.document.nodes[0] else {
            panic!("expected named argument call for {source:?}")
        };
        let named_args = named_args(arguments);
        assert_eq!(named_args.len(), 1, "{source:?}");
        assert_eq!(named_args[0].name, expected_name, "{source:?}");
        assert_eq!(named_args[0].name_span, expected_name_span, "{source:?}");
        assert_eq!(named_args[0].span, expected_argument_span, "{source:?}");
        assert_eq!(
            source_slice(source, named_args[0].name_span),
            expected_name,
            "{source:?}"
        );

        let markdown = parse_with_mode(source, Mode::Markdown);
        assert!(!markdown.document.nodes.iter().any(|node| match node {
            Block::DirectiveCall { .. } => true,
            Block::Paragraph { content, .. } => content
                .iter()
                .any(|inline| matches!(inline, Inline::DirectiveCall { .. })),
            _ => false,
        }));
    }

    for source in [".foo _:{x}\n", ".foo -:{x}\n", ".foo name-1:{x}\n"] {
        let output = parse_with_diagnostics(source);
        assert!(output.diagnostics.is_empty(), "{output:?}");
        let Block::Paragraph { content, span } = &output.document.nodes[0] else {
            panic!("expected invalid named argument to remain paragraph text: {source:?}")
        };
        assert_eq!(source_slice(source, *span), source.trim_end());
        let call = content
            .iter()
            .find_map(|inline| match inline {
                Inline::DirectiveCall {
                    arguments, span, ..
                } => Some((named_args(arguments), *span)),
                _ => None,
            })
            .expect("expected the valid `.foo` prefix to remain a call");
        assert!(call.0.is_empty(), "{source:?}");
        assert_eq!(source_slice(source, call.1), ".foo", "{source:?}");
        assert_markdown_isolated(source);
    }
}

#[test]
fn audit_records_current_continuation_before_first_argument_gap() {
    let block_source = concat!(".foo ", "\\", "\nname:{x}\n");
    let block_output = parse_with_diagnostics(block_source);
    assert!(block_output.diagnostics.is_empty(), "{block_output:?}");
    let Block::DirectiveCall {
        arguments, span, ..
    } = &block_output.document.nodes[0]
    else {
        panic!("expected the current block parser to stop at the call name")
    };
    assert!(named_args(arguments).is_empty());
    assert_eq!(
        source_slice(block_source, *span),
        concat!(".foo ", "\\", "\n")
    );
    assert!(matches!(
        block_output.document.nodes.get(1),
        Some(Block::Paragraph { .. })
    ));

    let inline_source = concat!("prefix .foo ", "\\", "\nname:{x} suffix\n");
    let inline_output = parse_with_diagnostics(inline_source);
    assert!(inline_output.diagnostics.is_empty(), "{inline_output:?}");
    let call = first_inline_call(&inline_output.document);
    let Inline::DirectiveCall {
        name,
        arguments,
        span,
        ..
    } = call
    else {
        unreachable!()
    };
    assert_eq!(name, "foo");
    assert!(named_args(arguments).is_empty());
    assert_eq!(source_slice(inline_source, *span), ".foo");

    let crlf_source = ".foo \\\r\nname:{한글}\r\n";
    let crlf_output = parse_with_diagnostics(crlf_source);
    assert!(crlf_output.diagnostics.is_empty(), "{crlf_output:?}");
    let Block::DirectiveCall {
        arguments, span, ..
    } = &crlf_output.document.nodes[0]
    else {
        panic!("expected the current CRLF parser to stop at the call name")
    };
    assert!(named_args(arguments).is_empty());
    assert_eq!(source_slice(crlf_source, *span), ".foo \\\r\n");
    for source in [block_source, inline_source, crlf_source] {
        assert_markdown_isolated(source);
    }
}

#[test]
fn audit_records_current_trailing_continuation_gap() {
    let block_source = concat!(".foo {x} ", "\\", "\n");
    let block_output = parse_with_diagnostics(block_source);
    let block_diagnostic = block_output
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "E2004")
        .expect("expected current trailing-continuation E2004");
    assert_eq!(source_slice(block_source, block_diagnostic.span), "\\");
    assert!(matches!(
        block_output.document.nodes.first(),
        Some(Block::Unsupported { .. })
    ));

    let inline_source = concat!("prefix .foo {x} ", "\\", "\n");
    let inline_output = parse_with_diagnostics(inline_source);
    let inline_diagnostic = inline_output
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "E2004")
        .expect("expected current inline trailing-continuation E2004");
    assert_eq!(source_slice(inline_source, inline_diagnostic.span), "\\");
    assert!(!inline_output.document.nodes.iter().any(|node| match node {
        Block::DirectiveCall { .. } => true,
        Block::Paragraph { content, .. } => content
            .iter()
            .any(|inline| matches!(inline, Inline::DirectiveCall { .. })),
        _ => false,
    }));
    assert_markdown_isolated(block_source);
    assert_markdown_isolated(inline_source);
}

#[test]
fn audit_records_current_chain_separator_placement_gap() {
    for source in [".a {x} ::b {y}\n", "prefix .a {x} ::b {y} suffix\n"] {
        let output = parse_with_diagnostics(source);
        assert!(output.diagnostics.is_empty(), "{output:?}");
        let call = first_inline_call(&output.document);
        let Inline::DirectiveCall { chain, span, .. } = call else {
            unreachable!()
        };
        assert!(chain.is_empty(), "{source:?}");
        assert_eq!(source_slice(source, *span), ".a {x}");
    }

    let block_source = concat!(".a {x} ", "\\", "\n::b {y}\n");
    let block_output = parse_with_diagnostics(block_source);
    let block_diagnostic = block_output
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "E2004")
        .expect("expected current block chain-continuation E2004");
    assert_eq!(source_slice(block_source, block_diagnostic.span), "\\");
    assert!(matches!(
        block_output.document.nodes.first(),
        Some(Block::Unsupported { .. })
    ));

    let inline_source = concat!("prefix .a {x} ", "\\", "\n::b {y} suffix\n");
    let inline_output = parse_with_diagnostics(inline_source);
    let inline_diagnostic = inline_output
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "E2004")
        .expect("expected current inline chain-continuation E2004");
    assert_eq!(source_slice(inline_source, inline_diagnostic.span), ":");
    assert!(!inline_output.document.nodes.iter().any(|node| match node {
        Block::DirectiveCall { .. } => true,
        Block::Paragraph { content, .. } => content
            .iter()
            .any(|inline| matches!(inline, Inline::DirectiveCall { .. })),
        _ => false,
    }));
    for source in [
        ".a {x} ::b {y}\n",
        "prefix .a {x} ::b {y} suffix\n",
        block_source,
        inline_source,
    ] {
        assert_markdown_isolated(source);
    }
}

#[test]
fn audit_requires_adjacent_named_argument_delimiters_and_preserves_source() {
    for (source, expected_argument_span) in [
        (".foo name:{x}\n", ByteSpan::new(5, 13)),
        (".foo name:{한글}\r\n", ByteSpan::new(5, 18)),
    ] {
        let output = parse_with_diagnostics(source);
        assert!(output.diagnostics.is_empty(), "{output:?}");
        let Block::DirectiveCall {
            arguments, span, ..
        } = &output.document.nodes[0]
        else {
            panic!("expected pinned named-argument acceptance for {source:?}")
        };
        let named_args = named_args(arguments);
        assert_eq!(named_args.len(), 1, "{source:?}");
        assert_eq!(named_args[0].name, "name", "{source:?}");
        assert_eq!(named_args[0].name_span, ByteSpan::new(5, 9), "{source:?}");
        assert_eq!(source_slice(source, named_args[0].name_span), "name");
        assert_eq!(named_args[0].span, expected_argument_span, "{source:?}");
        assert_eq!(
            source_slice(source, named_args[0].span),
            &source[5..expected_argument_span.end]
        );
        assert!(source_slice(source, *span).starts_with(".foo name"));

        let markdown = parse_with_mode(source, Mode::Markdown);
        assert!(!markdown.document.nodes.iter().any(|node| match node {
            Block::DirectiveCall { .. } => true,
            Block::Paragraph { content, .. } => content
                .iter()
                .any(|inline| matches!(inline, Inline::DirectiveCall { .. })),
            _ => false,
        }));
    }

    for source in [".foo name :{x}\n", ".foo name: {x}\n", ".foo name : {x}\n"] {
        let output = parse_with_diagnostics(source);
        assert!(output.diagnostics.is_empty(), "{output:?}");
        let Block::Paragraph { content, span } = &output.document.nodes[0] else {
            panic!("expected non-adjacent named syntax to remain source text: {source:?}")
        };
        assert_eq!(source_slice(source, *span), source.trim_end());
        let (named_args, call_span) = content
            .iter()
            .find_map(|inline| match inline {
                Inline::DirectiveCall {
                    arguments, span, ..
                } => Some((named_args(arguments), *span)),
                _ => None,
            })
            .expect("expected the valid `.foo` prefix to remain a call");
        assert!(named_args.is_empty(), "{source:?}");
        assert_eq!(source_slice(source, call_span), ".foo", "{source:?}");
        assert!(content.iter().any(|inline| {
            matches!(inline, Inline::Text { span, .. } if source_slice(source, *span).contains("name"))
        }));
        assert_markdown_isolated(source);
    }
}

#[test]
fn audit_aligns_call_boundaries_across_utf8_crlf_and_modes() {
    let source = "앞 word.foo {x} 중 .foo {x}한글 끝\r\n";
    let output = parse_with_diagnostics(source);
    assert!(output.diagnostics.is_empty(), "{output:?}");
    let Block::Paragraph { content, span } = &output.document.nodes[0] else {
        panic!("expected inline calls and ordinary text")
    };
    assert_eq!(source_slice(source, *span), source.trim_end());
    assert_eq!(
        content
            .iter()
            .filter(|inline| matches!(inline, Inline::DirectiveCall { name, .. } if name == "foo"))
            .count(),
        1
    );
    let call = content
        .iter()
        .find_map(|inline| match inline {
            Inline::DirectiveCall {
                name,
                name_span,
                span,
                arguments,
                ..
            } if name == "foo" => Some((*name_span, *span, positional_args(arguments))),
            _ => None,
        })
        .expect("expected the UTF-8-surrounded call");
    assert_eq!(source_slice(source, call.0), ".foo");
    assert_eq!(source_slice(source, call.1), ".foo {x}");
    assert_eq!(call.2.len(), 1);
    assert!(source_slice(source, *span).contains("word.foo {x}"));
    assert!(source_slice(source, *span).ends_with("끝"));

    let block_source = ".foo {x}\r\n";
    let block_output = parse_with_diagnostics(block_source);
    assert!(block_output.diagnostics.is_empty(), "{block_output:?}");
    let Block::DirectiveCall {
        name_span, span, ..
    } = &block_output.document.nodes[0]
    else {
        panic!("expected an isolated Quarkdown block call")
    };
    assert_eq!(source_slice(block_source, *name_span), ".foo");
    assert_eq!(source_slice(block_source, *span), ".foo {x}");

    for isolated in [source, block_source, "한글 .foo 1:{x}\r\n"] {
        assert_markdown_isolated(isolated);
    }
}

#[test]
fn audit_numeric_named_arguments_cross_the_block_continuation_boundary() {
    let source = ".foo {x} \\\r\n\t10:{한글}\r\n";
    let output = parse_with_diagnostics(source);
    assert!(output.diagnostics.is_empty(), "{output:?}");
    let Block::DirectiveCall {
        arguments, span, ..
    } = &output.document.nodes[0]
    else {
        panic!("expected numeric named argument in a continued block call")
    };
    assert_eq!(source_slice(source, *span), source.trim_end());
    let named_args = named_args(arguments);
    assert_eq!(named_args.len(), 1);
    assert_eq!(named_args[0].name, "10");
    assert_eq!(source_slice(source, named_args[0].name_span), "10");
    assert_eq!(source_slice(source, named_args[0].span), "10:{한글}");
}
