use arkst_markdown::ast::{Block, Document, Inline};
use arkst_markdown::{parse_with_mode, Mode};
use arkst_source::ByteSpan;

fn paragraph(document: &Document) -> (&[Inline], ByteSpan) {
    match &document.nodes[0] {
        Block::Paragraph { content, span } => (content, *span),
        other => panic!("expected paragraph, got {other:?}"),
    }
}

fn parse_markdown(source: &str) -> Document {
    let output = parse_with_mode(source, Mode::Markdown);
    assert!(
        output.diagnostics.is_empty(),
        "unexpected diagnostics for {source:?}: {:?}",
        output.diagnostics
    );
    output.document
}

fn text<'a>(content: &'a [Inline], expected: &str) -> (&'a str, ByteSpan) {
    content
        .iter()
        .find_map(|inline| match inline {
            Inline::Text { content, span } if content == expected => {
                Some((content.as_str(), *span))
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing text {expected:?} in {content:?}"))
}

fn semantic_texts(content: &[Inline]) -> Vec<&str> {
    content
        .iter()
        .filter_map(|inline| match inline {
            Inline::Text { content, .. } if !content.is_empty() => Some(content.as_str()),
            _ => None,
        })
        .collect()
}

fn only_break(content: &[Inline], hard: bool) -> ByteSpan {
    let breaks = content.iter().filter_map(|inline| match inline {
        Inline::HardBreak { span } if hard => Some(*span),
        Inline::SoftBreak { span } if !hard => Some(*span),
        Inline::HardBreak { .. } | Inline::SoftBreak { .. } => None,
        _ => None,
    });
    let spans = breaks.collect::<Vec<_>>();
    assert_eq!(spans.len(), 1, "expected one matching break in {content:?}");
    assert_eq!(
        content
            .iter()
            .filter(|inline| matches!(inline, Inline::HardBreak { .. } | Inline::SoftBreak { .. }))
            .count(),
        1,
        "unexpected break kind in {content:?}"
    );
    spans[0]
}

fn assert_source_span(source: &str, span: ByteSpan, expected: &str) {
    assert!(span.start <= span.end && span.end <= source.len());
    assert!(source.is_char_boundary(span.start));
    assert!(source.is_char_boundary(span.end));
    assert_eq!(&source[span.start..span.end], expected);
}

#[test]
fn exactly_two_trailing_spaces_are_hard_break_syntax_not_text() {
    let source = "foo  \nbar\n";
    let document = parse_markdown(source);
    let (content, span) = paragraph(&document);

    assert_eq!(semantic_texts(content), ["foo", "bar"]);
    let (_, first_span) = text(content, "foo");
    let (_, second_span) = text(content, "bar");
    let break_span = only_break(content, true);
    assert_source_span(source, first_span, "foo");
    assert_source_span(source, break_span, "  \n");
    assert_source_span(source, second_span, "bar");
    assert_eq!(span, ByteSpan::new(0, 9));
    assert_source_span(source, span, "foo  \nbar");
}

#[test]
fn three_or_more_trailing_spaces_are_all_excluded_from_semantic_text() {
    for source in ["foo   \nbar\n", "foo     \nbar\n", "foo       \nbar\n"] {
        let document = parse_markdown(source);
        let (content, paragraph_span) = paragraph(&document);
        assert_eq!(semantic_texts(content), ["foo", "bar"]);
        assert!(content.iter().all(|inline| {
            !matches!(inline, Inline::Text { content, .. } if content.ends_with(' '))
        }));
        let break_span = only_break(content, true);
        assert_source_span(
            source,
            break_span,
            &source[break_span.start..break_span.end],
        );
        assert!(source[break_span.start..break_span.end].contains('\n'));
        assert_source_span(source, paragraph_span, &source[..source.len() - 1]);
    }
}

#[test]
fn pinned_hard_break_inputs_keep_their_semantic_text() {
    for (source, expected) in [
        ("aaa     \nbbb     \n", ["aaa", "bbb"]),
        ("foo       \nbaz\n", ["foo", "baz"]),
    ] {
        let document = parse_markdown(source);
        let (content, _) = paragraph(&document);
        assert_eq!(semantic_texts(content), expected);
        assert_eq!(
            content
                .iter()
                .filter(|inline| matches!(inline, Inline::HardBreak { .. }))
                .count(),
            1
        );
    }
}

#[test]
fn soft_break_and_hard_break_keep_distinct_inline_kinds() {
    let soft_source = "foo\nbar\n";
    let soft_document = parse_markdown(soft_source);
    let (soft_content, _) = paragraph(&soft_document);
    assert_eq!(semantic_texts(soft_content), ["foo", "bar"]);
    assert_source_span(soft_source, only_break(soft_content, false), "\n");

    let whitespace_soft_source = "foo \nbar\n";
    let whitespace_soft_document = parse_markdown(whitespace_soft_source);
    let (whitespace_soft_content, _) = paragraph(&whitespace_soft_document);
    assert_eq!(semantic_texts(whitespace_soft_content), ["foo", "bar"]);
    assert_source_span(
        whitespace_soft_source,
        only_break(whitespace_soft_content, false),
        " \n",
    );
}

#[test]
fn utf8_before_hard_break_keeps_byte_spans_on_character_boundaries() {
    let source = "한글   \n다음\n";
    let document = parse_markdown(source);
    let (content, paragraph_span) = paragraph(&document);
    assert_eq!(semantic_texts(content), ["한글", "다음"]);
    let (_, first_span) = text(content, "한글");
    let (_, second_span) = text(content, "다음");
    let break_span = only_break(content, true);
    assert_source_span(
        source,
        first_span,
        &source[first_span.start..first_span.end],
    );
    assert_source_span(
        source,
        break_span,
        &source[break_span.start..break_span.end],
    );
    assert_source_span(source, second_span, "다음");
    assert_source_span(source, paragraph_span, &source[..source.len() - 1]);
}

#[test]
fn crlf_hard_break_keeps_delimiter_and_global_byte_offsets() {
    let source = "한글  \r\n다음\r\n";
    let document = parse_markdown(source);
    let (content, paragraph_span) = paragraph(&document);
    assert_eq!(semantic_texts(content), ["한글", "다음"]);
    let (_, first_span) = text(content, "한글");
    let (_, second_span) = text(content, "다음");
    let break_span = only_break(content, true);
    assert_source_span(source, first_span, "한글");
    assert_source_span(source, break_span, "  \r\n");
    assert_source_span(source, second_span, "다음");
    assert_eq!(paragraph_span, ByteSpan::new(0, 16));
    assert_source_span(source, paragraph_span, "한글  \r\n다음");
}

#[test]
fn nested_blockquote_preserves_global_hard_break_spans() {
    let source = "> 앞  \n> 뒤\n";
    let document = parse_markdown(source);
    let Block::Blockquote { content, span } = &document.nodes[0] else {
        panic!("expected blockquote, got {:?}", document.nodes[0]);
    };
    let Block::Paragraph {
        content: inline_content,
        span: paragraph_span,
    } = &content[0]
    else {
        panic!("expected blockquote paragraph, got {:?}", content[0]);
    };
    assert_eq!(semantic_texts(inline_content), ["앞", "뒤"]);
    assert_source_span(source, text(inline_content, "앞").1, "앞");
    assert_source_span(source, only_break(inline_content, true), "  \n");
    assert_source_span(source, text(inline_content, "뒤").1, "뒤");
    assert_eq!(*paragraph_span, ByteSpan::new(2, 13));
    assert_eq!(*span, ByteSpan::new(0, 13));
}

#[test]
fn nested_list_item_preserves_global_hard_break_spans() {
    let source = "- 앞  \n  뒤\n";
    let document = parse_markdown(source);
    let Block::UnorderedList { items, span, .. } = &document.nodes[0] else {
        panic!("expected unordered list, got {:?}", document.nodes[0]);
    };
    let Block::Paragraph {
        content,
        span: paragraph_span,
    } = &items[0].content[0]
    else {
        panic!("expected list paragraph, got {:?}", items[0].content[0]);
    };
    assert_eq!(semantic_texts(content), ["앞", "뒤"]);
    assert_source_span(source, text(content, "앞").1, "앞");
    assert_source_span(source, only_break(content, true), "  \n");
    assert_source_span(source, text(content, "뒤").1, "뒤");
    assert_eq!(*paragraph_span, ByteSpan::new(2, 13));
    assert_eq!(*span, ByteSpan::new(0, 13));
}

#[test]
fn multiple_hard_breaks_do_not_retain_delimiter_spaces() {
    let source = "a     \nb     \nc\n";
    let document = parse_markdown(source);
    let (content, paragraph_span) = paragraph(&document);
    assert_eq!(semantic_texts(content), ["a", "b", "c"]);
    assert_eq!(
        content
            .iter()
            .filter(|inline| matches!(inline, Inline::HardBreak { .. }))
            .count(),
        2
    );
    assert!(content.iter().all(|inline| {
        !matches!(inline, Inline::Text { content, .. } if content.ends_with(' '))
    }));
    assert_source_span(source, paragraph_span, &source[..source.len() - 1]);
}

#[test]
fn sibling_inline_nodes_survive_hard_break_conversion() {
    let source = "**굵게**  \n*기울임*\n";
    let document = parse_markdown(source);
    let (content, paragraph_span) = paragraph(&document);
    assert!(matches!(content[0], Inline::Strong { .. }));
    assert!(matches!(content[1], Inline::HardBreak { .. }));
    assert!(matches!(content[2], Inline::Emphasis { .. }));
    let Inline::Strong {
        content: strong, ..
    } = &content[0]
    else {
        unreachable!();
    };
    let Inline::Emphasis {
        content: emphasis, ..
    } = &content[2]
    else {
        unreachable!();
    };
    assert_eq!(semantic_texts(strong), ["굵게"]);
    assert_eq!(semantic_texts(emphasis), ["기울임"]);
    assert_source_span(source, only_break(content, true), "  \n");
    assert_eq!(paragraph_span, ByteSpan::new(0, 24));
    assert_source_span(source, paragraph_span, &source[..source.len() - 1]);
}
