use scribium_markdown::{
    parse_inline_with_mode, parse_with_diagnostics, parse_with_mode, Block, CallArgument, Inline,
    Mode, Value,
};
use scribium_source::ByteSpan;

fn source_slice(source: &str, span: ByteSpan) -> &str {
    assert!(span.is_valid_for(source));
    &source[span.start..span.end]
}

fn content_argument_for(source: &str) -> (Vec<Inline>, ByteSpan) {
    let output = parse_with_diagnostics(source);
    assert!(output.diagnostics.is_empty(), "{output:?}");
    let Block::DirectiveCall { arguments, .. } = &output.document.nodes[0] else {
        panic!(
            "expected outer directive call, got {:?}",
            output.document.nodes
        );
    };
    let CallArgument::Positional {
        value: Value::Content(content),
        span,
    } = &arguments[0]
    else {
        panic!("expected positional content argument");
    };
    (content.clone(), *span)
}

fn content_for(source: &str) -> Vec<Inline> {
    content_argument_for(source).0
}

fn assert_inline_spans(source: &str, inlines: &[Inline]) {
    for inline in inlines {
        let span = match inline {
            Inline::Text { span, .. }
            | Inline::Emphasis { span, .. }
            | Inline::Strong { span, .. }
            | Inline::DirectiveCall { span, .. }
            | Inline::Link { span, .. }
            | Inline::Image { span, .. }
            | Inline::Code { span, .. }
            | Inline::RawHtml { span, .. }
            | Inline::Strikethrough { span, .. }
            | Inline::Unsupported { span, .. }
            | Inline::HardBreak { span }
            | Inline::SoftBreak { span } => *span,
        };
        source_slice(source, span);
        match inline {
            Inline::Emphasis { content, .. }
            | Inline::Strong { content, .. }
            | Inline::Link { content, .. }
            | Inline::Image { content, .. }
            | Inline::Strikethrough { content, .. } => assert_inline_spans(source, content),
            Inline::DirectiveCall {
                name_span,
                head_span,
                arguments,
                body,
                chain,
                ..
            } => {
                source_slice(source, *name_span);
                source_slice(source, *head_span);
                for argument in arguments {
                    match argument {
                        CallArgument::Positional { value, span } => {
                            source_slice(source, *span);
                            if let Value::Content(content) = value {
                                assert_inline_spans(source, content);
                            }
                        }
                        CallArgument::Named(argument) => {
                            source_slice(source, argument.name_span);
                            source_slice(source, argument.value_span);
                            source_slice(source, argument.span);
                            if let Value::Content(content) = &argument.value {
                                assert_inline_spans(source, content);
                            }
                        }
                    }
                }
                for segment in chain {
                    source_slice(source, segment.name_span);
                    source_slice(source, segment.span);
                }
                if let Some(body) = body {
                    assert_inline_spans(source, body);
                }
            }
            _ => {}
        }
    }
}

#[test]
fn content_arguments_preserve_basic_markdown_inline_nodes() {
    let emphasis = ".outer {*emphasis*}";
    let content = content_for(emphasis);
    let [Inline::Emphasis { content, span }] = content.as_slice() else {
        panic!("expected emphasis, got {content:?}");
    };
    assert_eq!(source_slice(emphasis, *span), "*emphasis*");
    assert_eq!(
        source_slice(emphasis, span_of_text(&content[0])),
        "emphasis"
    );

    let strong = ".outer {**strong**}";
    let content = content_for(strong);
    let [Inline::Strong { content, span }] = content.as_slice() else {
        panic!("expected strong, got {content:?}");
    };
    assert_eq!(source_slice(strong, *span), "**strong**");
    assert_eq!(source_slice(strong, span_of_text(&content[0])), "strong");

    let code = ".outer {`code`}";
    let content = content_for(code);
    let [Inline::Code {
        content: value,
        span,
    }] = content.as_slice()
    else {
        panic!("expected code, got {content:?}");
    };
    assert_eq!(value, "code");
    assert_eq!(source_slice(code, *span), "`code`");

    let link = ".outer {[link](target)}";
    let content = content_for(link);
    let [Inline::Link {
        content,
        destination,
        span,
        ..
    }] = content.as_slice()
    else {
        panic!("expected link, got {content:?}");
    };
    assert_eq!(destination, "target");
    assert_eq!(source_slice(link, *span), "[link](target)");
    assert_eq!(source_slice(link, span_of_text(&content[0])), "link");

    let image = ".outer {![alt](image.png)}";
    let content = content_for(image);
    let [Inline::Image {
        content,
        destination,
        span,
        ..
    }] = content.as_slice()
    else {
        panic!("expected image, got {content:?}");
    };
    assert_eq!(destination, "image.png");
    assert_eq!(source_slice(image, *span), "![alt](image.png)");
    assert_eq!(source_slice(image, span_of_text(&content[0])), "alt");
}

fn span_of_text(inline: &Inline) -> ByteSpan {
    let Inline::Text { span, .. } = inline else {
        panic!("expected text child, got {inline:?}");
    };
    *span
}

#[test]
fn content_arguments_preserve_mixed_order_and_surrounding_text() {
    let source = ".outer {A **bold** and [link](target) with `code` Z}";
    let (content, argument_span) = content_argument_for(source);
    assert_eq!(
        source_slice(source, argument_span),
        "{A **bold** and [link](target) with `code` Z}"
    );
    assert_inline_spans(source, &content);
    assert!(matches!(
        content.as_slice(),
        [
            Inline::Text { .. },
            Inline::Strong { .. },
            Inline::Text { .. },
            Inline::Link { .. },
            Inline::Text { .. },
            Inline::Code { .. },
            Inline::Text { .. }
        ]
    ));
    let slices: Vec<_> = content
        .iter()
        .map(|inline| match inline {
            Inline::Text { span, .. }
            | Inline::Strong { span, .. }
            | Inline::Link { span, .. }
            | Inline::Code { span, .. } => source_slice(source, *span),
            inline => panic!("unexpected inline {inline:?}"),
        })
        .collect();
    assert_eq!(
        slices,
        [
            "A ",
            "**bold**",
            " and ",
            "[link](target)",
            " with ",
            "`code`",
            " Z"
        ]
    );
}

#[test]
fn content_arguments_preserve_nested_markdown_structure() {
    let source = ".outer {**bold and *emphasis***}";
    let content = content_for(source);
    let [Inline::Strong {
        content: strong,
        span: strong_span,
    }] = content.as_slice()
    else {
        panic!("expected strong, got {content:?}");
    };
    assert_eq!(
        source_slice(source, *strong_span),
        "**bold and *emphasis***"
    );
    assert!(strong
        .iter()
        .any(|inline| matches!(inline, Inline::Emphasis { .. })));
    let Inline::Emphasis {
        content: emphasis,
        span: emphasis_span,
    } = strong
        .iter()
        .find(|inline| matches!(inline, Inline::Emphasis { .. }))
        .expect("expected nested emphasis")
    else {
        unreachable!()
    };
    assert_eq!(source_slice(source, *emphasis_span), "*emphasis*");
    assert_eq!(source_slice(source, span_of_text(&emphasis[0])), "emphasis");
    assert_inline_spans(source, strong);
}

#[test]
fn content_arguments_preserve_inline_and_tight_nested_calls() {
    let inline_source = ".outer {before .inner {x} after}";
    let inline_content = content_for(inline_source);
    assert!(matches!(
        inline_content.as_slice(),
        [
            Inline::Text { .. },
            Inline::DirectiveCall { .. },
            Inline::Text { .. }
        ]
    ));
    assert_inline_spans(inline_source, &inline_content);

    let tight_source = ".outer {before {.inner {x}} after}";
    let tight_content = content_for(tight_source);
    assert!(matches!(
        tight_content.as_slice(),
        [
            Inline::Text { .. },
            Inline::DirectiveCall { .. },
            Inline::Text { .. }
        ]
    ));
    let Inline::DirectiveCall {
        span,
        name_span,
        head_span,
        arguments,
        ..
    } = &tight_content[1]
    else {
        unreachable!()
    };
    assert_eq!(source_slice(tight_source, *span), "{.inner {x}}");
    assert_eq!(source_slice(tight_source, *name_span), ".inner");
    assert_eq!(source_slice(tight_source, *head_span), ".inner {x}");
    assert_eq!(source_slice(tight_source, arguments[0].span()), "{x}");
    assert_inline_spans(tight_source, &tight_content);
}

#[test]
fn markdown_structure_and_tight_calls_can_coexist() {
    let source = ".outer {**before {.inner {x}} after**}";
    let content = content_for(source);
    let [Inline::Strong { content, span }] = content.as_slice() else {
        panic!("expected strong, got {content:?}");
    };
    assert_eq!(source_slice(source, *span), "**before {.inner {x}} after**");
    assert!(content
        .iter()
        .any(|inline| matches!(inline, Inline::DirectiveCall { .. })));
    assert_inline_spans(source, content);
}

#[test]
fn content_arguments_preserve_utf8_and_crlf_source_provenance() {
    let utf8 = ".outer {앞 **중간** 뒤}";
    let content = content_for(utf8);
    let strong = content
        .iter()
        .find(|inline| matches!(inline, Inline::Strong { .. }))
        .expect("expected UTF-8 strong inline");
    let Inline::Strong { span, content } = strong else {
        unreachable!()
    };
    assert_eq!(source_slice(utf8, *span), "**중간**");
    assert_eq!(source_slice(utf8, span_of_text(&content[0])), "중간");
    assert_inline_spans(utf8, &content_for(utf8));

    let crlf = ".outer {A **강조**\r\n B}";
    assert!(crlf.as_bytes().windows(2).any(|pair| pair == b"\r\n"));
    let content = content_for(crlf);
    let strong = content
        .iter()
        .find(|inline| matches!(inline, Inline::Strong { .. }))
        .expect("expected CRLF strong inline");
    let Inline::Strong { span, .. } = strong else {
        unreachable!()
    };
    assert_eq!(source_slice(crlf, *span), "**강조**");
    assert!(content.iter().any(|inline| {
        matches!(inline, Inline::SoftBreak { span } if source_slice(crlf, *span) == "\r\n")
    }));
    assert_inline_spans(crlf, &content);
}

#[test]
fn malformed_markdown_delimiters_fall_back_without_invalid_spans() {
    for source in [
        ".outer {*unclosed}",
        ".outer {[unclosed](target}",
        ".outer {`unclosed}",
    ] {
        let output = parse_with_diagnostics(source);
        assert!(output.diagnostics.is_empty(), "{source:?}: {output:?}");
        let content = content_for(source);
        assert_inline_spans(source, &content);
        assert!(!content.iter().any(|inline| matches!(
            inline,
            Inline::Emphasis { .. }
                | Inline::Strong { .. }
                | Inline::Link { .. }
                | Inline::Code { .. }
        )));
    }
}

#[test]
fn markdown_mode_does_not_gain_quarkdown_content_directives() {
    let source = ".outer {**bold {.inner {x}}**}";
    let output = parse_with_mode(source, Mode::Markdown);
    assert!(output.diagnostics.is_empty(), "{output:?}");
    assert!(!output.document.nodes.iter().any(|block| match block {
        Block::DirectiveCall { .. } => true,
        Block::Paragraph { content, .. } => content.iter().any(contains_directive_call),
        _ => false,
    }));
}

fn contains_directive_call(inline: &Inline) -> bool {
    match inline {
        Inline::DirectiveCall { .. } => true,
        Inline::Emphasis { content, .. }
        | Inline::Strong { content, .. }
        | Inline::Link { content, .. }
        | Inline::Image { content, .. }
        | Inline::Strikethrough { content, .. } => content.iter().any(contains_directive_call),
        _ => false,
    }
}

#[test]
fn ordinary_braces_remain_text_inside_content_arguments() {
    let source = ".outer {H{ordinary}O}";
    let content = content_for(source);
    assert!(!content
        .iter()
        .any(|inline| matches!(inline, Inline::DirectiveCall { .. })));
    let text_slices: Vec<_> = content
        .iter()
        .map(|inline| match inline {
            Inline::Text { span, .. } => source_slice(source, *span),
            inline => panic!("ordinary braces were promoted: {inline:?}"),
        })
        .collect();
    assert_eq!(text_slices.concat(), "H{ordinary}O");
}

#[test]
fn delimiter_completion_is_scoped_to_content_arguments() {
    let source = "**normal** and *emphasis*\n";
    for mode in [Mode::Markdown, Mode::Quarkdown] {
        let output = parse_with_mode(source, mode);
        assert!(output.diagnostics.is_empty(), "{output:?}");
        let Block::Paragraph { content, .. } = &output.document.nodes[0] else {
            panic!("expected paragraph, got {:?}", output.document.nodes);
        };
        let spans: Vec<_> = content
            .iter()
            .map(|inline| match inline {
                Inline::Strong { span, .. }
                | Inline::Emphasis { span, .. }
                | Inline::Text { span, .. } => source_slice(source, *span),
                inline => panic!("unexpected inline {inline:?}"),
            })
            .collect();
        assert_eq!(spans, ["**normal", " and ", "*emphasis"]);
    }
}

#[test]
fn dynamic_inline_target_fragment_uses_the_existing_inline_parser() {
    let output = parse_inline_with_mode("before .uppercase {world} after", Mode::Quarkdown);
    assert!(
        output.diagnostics.is_empty(),
        "unexpected: {:?}",
        output.diagnostics
    );
    let [Block::Paragraph { content, .. }] = output.document.nodes.as_slice() else {
        panic!(
            "expected one inline paragraph, got {:?}",
            output.document.nodes
        );
    };
    assert!(matches!(
        content.as_slice(),
        [
            Inline::Text { content: before, .. },
            Inline::DirectiveCall { name, .. },
            Inline::Text { content: after, .. },
        ] if before == "before " && name == "uppercase" && after == " after"
    ));
}

trait ArgumentSpan {
    fn span(&self) -> ByteSpan;
}

impl ArgumentSpan for CallArgument {
    fn span(&self) -> ByteSpan {
        match self {
            CallArgument::Positional { span, .. } => *span,
            CallArgument::Named(argument) => argument.span,
        }
    }
}
