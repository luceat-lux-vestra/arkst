use scribium_markdown::ast::{Block, Inline};
use scribium_markdown::parse_md;
use scribium_source::ByteSpan;

fn paragraph_content(document: &scribium_markdown::Document) -> &[Inline] {
    let Block::Paragraph { content, .. } = &document.nodes[0] else {
        panic!("expected a paragraph")
    };
    content
}

fn links(content: &[Inline]) -> Vec<(&str, Option<&str>, ByteSpan)> {
    content
        .iter()
        .filter_map(|inline| match inline {
            Inline::Link {
                destination,
                title,
                span,
                ..
            } => Some((destination.as_str(), title.as_deref(), span)),
            _ => None,
        })
        .map(|(destination, title, span)| (destination, title, *span))
        .collect()
}

#[test]
fn link_metadata_normalizes_values_and_keeps_inline_source_spans() {
    let source = r#"left [escaped](a\*b) middle [named](u&amp;v) [numeric](w&#x26;x) [invalid](z&madeup;) right 한글"#;
    let document = parse_md(source);
    let content = paragraph_content(&document);
    assert!(content.iter().any(|inline| matches!(
        inline,
        Inline::Text { content, .. } if content.contains("left")
    )));
    assert!(content.iter().any(|inline| matches!(
        inline,
        Inline::Text { content, .. } if content.contains("middle")
    )));

    let actual = links(content);
    assert_eq!(actual.len(), 4);
    assert_eq!(actual[0].0, "a*b");
    assert_eq!(actual[1].0, "u&v");
    assert_eq!(actual[2].0, "w&x");
    assert_eq!(actual[3].0, "z&madeup;");

    for (_, _, span) in &actual {
        assert!(span.start <= span.end);
        assert!(source.is_char_boundary(span.start));
        assert!(source.is_char_boundary(span.end));
        assert!(source[span.start..span.end].contains('('));
        assert!(source[span.start..span.end].contains(')'));
    }
    assert_eq!(
        &source[actual[0].2.start..actual[0].2.end],
        r"[escaped](a\*b)"
    );
    assert_eq!(
        &source[actual[1].2.start..actual[1].2.end],
        "[named](u&amp;v)"
    );
    assert_eq!(
        &source[actual[2].2.start..actual[2].2.end],
        "[numeric](w&#x26;x)"
    );
    assert_eq!(
        &source[actual[3].2.start..actual[3].2.end],
        "[invalid](z&madeup;)"
    );
    assert!(source[actual[1].2.start..actual[1].2.end].contains("&amp;"));
}

#[test]
fn link_titles_normalize_quoted_and_parenthesized_metadata() {
    let source = r#"[quoted](url "ti&amp;tle\*") and [paren](url (p&#x61;ren)) and [quotes](/url "title \"&quot;") 한글"#;
    let document = parse_md(source);
    let actual = links(paragraph_content(&document));

    assert_eq!(actual.len(), 3);
    assert_eq!(actual[0].1, Some("ti&tle*"));
    assert_eq!(actual[1].1, Some("paren"));
    assert_eq!(actual[2].1, Some("title \"\""));
    assert_eq!(
        &source[actual[0].2.start..actual[0].2.end],
        r#"[quoted](url "ti&amp;tle\*")"#
    );
    assert_eq!(
        &source[actual[1].2.start..actual[1].2.end],
        "[paren](url (p&#x61;ren))"
    );
    assert_eq!(
        &source[actual[2].2.start..actual[2].2.end],
        r#"[quotes](/url "title \"&quot;")"#
    );
}

#[test]
fn metadata_escapes_keep_non_escapable_text_and_decode_references_once() {
    let source =
        r#"[backslash](a\\*) [nonpunct](a\q) [escaped-amp](a\&amp;) [single-pass](a&#38;amp;)"#;
    let document = parse_md(source);
    let actual = links(paragraph_content(&document));

    assert_eq!(actual.len(), 4);
    assert_eq!(actual[0].0, r"a\*");
    assert_eq!(actual[1].0, r"a\q");
    assert_eq!(actual[2].0, "a&");
    assert_eq!(actual[3].0, "a&amp;");
}

#[test]
fn reference_metadata_uses_the_same_policy_without_rewriting_source() {
    let source = concat!(
        "[ref] and [other]\n\n",
        "[ref]: /a&amp;b \"ti\\*tle\"\n",
        "[other]: /c&#x26;d (pa&amp;ren)\n",
    );
    let document = parse_md(source);
    let actual = links(paragraph_content(&document));

    assert_eq!(actual.len(), 2);
    assert_eq!(actual[0].0, "/a&b");
    assert_eq!(actual[0].1, Some("ti*tle"));
    assert_eq!(actual[1].0, "/c&d");
    assert_eq!(actual[1].1, Some("pa&ren"));
    assert_eq!(&source[actual[0].2.start..actual[0].2.end], "[ref");
    assert_eq!(&source[actual[1].2.start..actual[1].2.end], "[other");
    assert!(source.contains("/a&amp;b \"ti\\*tle\""));
    assert!(source.contains("/c&#x26;d (pa&amp;ren)"));
}

#[test]
fn fenced_code_info_normalizes_before_language_extraction_and_preserves_crlf() {
    let source = "before\r\n```foo\\+bar&amp;x extra\r\n본문\r\n```\r\nafter\r\n";
    let document = parse_md(source);
    let Block::CodeBlock {
        language,
        info,
        span,
        ..
    } = &document.nodes[1]
    else {
        panic!("expected a fenced code block")
    };

    assert_eq!(info.as_deref(), Some("foo+bar&x extra"));
    assert_eq!(language.as_deref(), Some("foo+bar&x"));
    assert_eq!(
        &source[span.start..span.end],
        "```foo\\+bar&amp;x extra\r\n본문\r\n```\r\n"
    );
    assert!(source[span.start..span.end].contains("&amp;"));
    assert!(source.is_char_boundary(span.start));
    assert!(source.is_char_boundary(span.end));

    let numeric = "```lang&#32;extra second\nbody\n```\n";
    let document = parse_md(numeric);
    let Block::CodeBlock { language, info, .. } = &document.nodes[0] else {
        panic!("expected a fenced code block")
    };
    assert_eq!(info.as_deref(), Some("lang extra second"));
    assert_eq!(language.as_deref(), Some("lang"));
}
