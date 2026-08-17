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
    let source = r#"[quoted](url "ti&amp;tle\*") and [paren](url (p&#x61;ren)) and [quotes](/url "title \"&quot;") and [escaped-named](url "a\&amp;") and [escaped-numeric](url "b\&#38;") 한글"#;
    let document = parse_md(source);
    let actual = links(paragraph_content(&document));

    assert_eq!(actual.len(), 5);
    assert_eq!(actual[0].1, Some("ti&tle*"));
    assert_eq!(actual[1].1, Some("paren"));
    assert_eq!(actual[2].1, Some("title \"\""));
    assert_eq!(actual[3].1, Some("a&amp;"));
    assert_eq!(actual[4].1, Some("b&#38;"));
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
    assert_eq!(
        &source[actual[3].2.start..actual[3].2.end],
        r#"[escaped-named](url "a\&amp;")"#
    );
    assert_eq!(
        &source[actual[4].2.start..actual[4].2.end],
        r#"[escaped-numeric](url "b\&#38;")"#
    );
}

#[test]
fn metadata_escapes_keep_precedence_and_decode_references_once() {
    let source = r#"[escaped-named](a\&amp;) [escaped-numeric](a\&#38;) [named](a&amp;) [decimal](a&#38;) [hex](a&#x26;) [single-pass](a&#38;amp;) [star](a\*) [plus](a\+) [nonpunct](a\q)"#;
    let document = parse_md(source);
    let actual = links(paragraph_content(&document));

    assert_eq!(actual.len(), 9);
    assert_eq!(actual[0].0, "a&amp;");
    assert_eq!(actual[1].0, "a&#38;");
    assert_eq!(actual[2].0, "a&");
    assert_eq!(actual[3].0, "a&");
    assert_eq!(actual[4].0, "a&");
    assert_eq!(actual[5].0, "a&amp;");
    assert_eq!(actual[6].0, "a*");
    assert_eq!(actual[7].0, "a+");
    assert_eq!(actual[8].0, r"a\q");
}

#[test]
fn text_escapes_decode_references_once_and_keep_escaped_references_literal() {
    let source = "앞 \\&ouml; &ouml; &amp; \\*별표\\* 뒤\r\n";
    let document = parse_md(source);
    let Block::Paragraph { content, .. } = &document.nodes[0] else {
        panic!("expected a paragraph")
    };
    let texts: Vec<_> = content
        .iter()
        .filter_map(|inline| match inline {
            Inline::Text { content, span } => Some((content.as_str(), *span)),
            _ => None,
        })
        .collect();

    let combined = texts
        .iter()
        .map(|(content, _)| *content)
        .collect::<String>();
    assert_eq!(combined, "앞 &ouml; ö & *별표* 뒤");
    for (_, span) in texts {
        assert!(span.start <= span.end);
        assert!(source.is_char_boundary(span.start));
        assert!(source.is_char_boundary(span.end));
        assert!(!source[span.start..span.end].is_empty());
    }
}

#[test]
fn numeric_zero_references_use_the_replacement_character_once() {
    let source = "&#0; &#x0; &#x0000;\r\n";
    let document = parse_md(source);
    let Block::Paragraph { content, .. } = &document.nodes[0] else {
        panic!("expected a paragraph")
    };
    let text = content
        .iter()
        .filter_map(|inline| match inline {
            Inline::Text { content, .. } => Some(content.as_str()),
            _ => None,
        })
        .collect::<String>();
    assert_eq!(text, "� � �");
}

#[test]
fn reference_metadata_uses_the_same_policy_without_rewriting_source() {
    let source = concat!(
        "[ref] and [other] and [escaped] and [numeric]\n\n",
        "[ref]: /a&amp;b \"ti\\*tle\"\n",
        "[other]: /c&#x26;d (pa&amp;ren)\n",
        "[escaped]: /e\\&amp;f \"ti\\&amp;tle\"\n",
        "[numeric]: /n\\&#38;d (pa\\&#x26;ren)\n",
    );
    let document = parse_md(source);
    let actual = links(paragraph_content(&document));

    assert_eq!(actual.len(), 4);
    assert_eq!(actual[0].0, "/a&b");
    assert_eq!(actual[0].1, Some("ti*tle"));
    assert_eq!(actual[1].0, "/c&d");
    assert_eq!(actual[1].1, Some("pa&ren"));
    assert_eq!(actual[2].0, "/e&amp;f");
    assert_eq!(actual[2].1, Some("ti&amp;tle"));
    assert_eq!(actual[3].0, "/n&#38;d");
    assert_eq!(actual[3].1, Some("pa&#x26;ren"));
    assert_eq!(&source[actual[0].2.start..actual[0].2.end], "[ref");
    assert_eq!(&source[actual[1].2.start..actual[1].2.end], "[other");
    assert_eq!(&source[actual[2].2.start..actual[2].2.end], "[escaped");
    assert_eq!(&source[actual[3].2.start..actual[3].2.end], "[numeric");
    assert!(source.contains("/a&amp;b \"ti\\*tle\""));
    assert!(source.contains("/c&#x26;d (pa&amp;ren)"));
    assert!(source.contains("/e\\&amp;f \"ti\\&amp;tle\""));
    assert!(source.contains("/n\\&#38;d (pa\\&#x26;ren)"));
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

    let escaped = "```lang\\&amp; extra second\nbody\n```\n";
    let document = parse_md(escaped);
    let Block::CodeBlock { language, info, .. } = &document.nodes[0] else {
        panic!("expected a fenced code block")
    };
    assert_eq!(info.as_deref(), Some("lang&amp; extra second"));
    assert_eq!(language.as_deref(), Some("lang&amp;"));

    let escaped_numeric = "```lang\\&#38; extra second\nbody\n```\n";
    let document = parse_md(escaped_numeric);
    let Block::CodeBlock { language, info, .. } = &document.nodes[0] else {
        panic!("expected a fenced code block")
    };
    assert_eq!(info.as_deref(), Some("lang&#38; extra second"));
    assert_eq!(language.as_deref(), Some("lang&#38;"));
}
