use arkst_markdown::ast::{Block, Inline};
use arkst_markdown::{parse_md, parse_qd};

fn inline_raw_html(content: &[Inline]) -> Vec<(&str, arkst_source::ByteSpan)> {
    content
        .iter()
        .filter_map(|inline| match inline {
            Inline::RawHtml { content, span } => Some((content.as_str(), *span)),
            _ => None,
        })
        .collect()
}

fn assert_inline_raw_spans(source: &str, content: &[Inline]) {
    for (raw, span) in inline_raw_html(content) {
        assert_eq!(source.get(span.start..span.end), Some(raw));
    }
}

#[test]
fn inline_html_forms_are_preserved_as_opaque_segments() {
    let source = "prefix <em class='quoted'>italic</em> <strong data-value=unquoted>bold</strong> <br/> <!-- comment --> <?instruction?> <!DOCTYPE html> <![CDATA[cdata]]> suffix\n";
    let document = parse_md(source);
    let Block::Paragraph { content, .. } = &document.nodes[0] else {
        panic!("expected paragraph, got {:?}", document.nodes);
    };

    let raw = inline_raw_html(content);
    assert_eq!(raw.len(), 9, "{raw:?}");
    assert_inline_raw_spans(source, content);
    assert_eq!(
        raw.iter().map(|(value, _)| *value).collect::<Vec<_>>(),
        vec![
            "<em class='quoted'>",
            "</em>",
            "<strong data-value=unquoted>",
            "</strong>",
            "<br/>",
            "<!-- comment -->",
            "<?instruction?>",
            "<!DOCTYPE html>",
            "<![CDATA[cdata]]>",
        ]
    );
}

#[test]
fn block_html_is_one_opaque_source_span_and_does_not_parse_markdown_inside() {
    let source =
        "before\n\n<div class=\"layout\">\n**not Markdown**\n</div>\n\nafter **Markdown**\n";
    let document = parse_md(source);

    let Block::RawHtml { source: raw, span } = &document.nodes[1] else {
        panic!("expected raw HTML block, got {:?}", document.nodes);
    };
    assert_eq!(raw, "<div class=\"layout\">\n**not Markdown**\n</div>\n");
    assert_eq!(source.get(span.start..span.end), Some(raw.as_str()));
    assert!(document.nodes.iter().any(|node| matches!(
        node,
        Block::Paragraph { content, .. }
            if content.iter().any(|inline| matches!(inline, Inline::Strong { .. }))
    )));
}

#[test]
fn block_comments_processing_instructions_declarations_and_cdata_are_opaque_blocks() {
    let source = "<!-- comment\n-->\n\n<?instruction\n?>\n\n<!DOCTYPE html\n>\n\n<![CDATA[\ncontent\n]]>\n\ntext\n";
    let document = parse_md(source);
    let raw_blocks = document
        .nodes
        .iter()
        .filter_map(|node| match node {
            Block::RawHtml { source, span } => Some((source.as_str(), *span)),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(raw_blocks.len(), 4, "{raw_blocks:?}");
    for (raw, span) in raw_blocks {
        assert_eq!(source.get(span.start..span.end), Some(raw));
    }
    assert!(matches!(
        document.nodes.last(),
        Some(Block::Paragraph { .. })
    ));
}

#[test]
fn nested_inline_html_and_markdown_children_remain_separate_nodes() {
    let source = "prefix <em>**inside**</em> suffix **outside**\n";
    let document = parse_md(source);
    let Block::Paragraph { content, .. } = &document.nodes[0] else {
        panic!("expected paragraph, got {:?}", document.nodes);
    };

    assert_inline_raw_spans(source, content);
    assert_eq!(
        inline_raw_html(content)
            .iter()
            .map(|(value, _)| *value)
            .collect::<Vec<_>>(),
        vec!["<em>", "</em>"]
    );
    assert!(content.iter().any(|inline| matches!(
        inline,
        Inline::Strong { content, .. }
            if content.iter().any(|child| matches!(child, Inline::Text { content, .. } if content == "inside"))
    )));
    assert!(content.iter().any(|inline| matches!(
        inline,
        Inline::Strong { content, .. }
            if content.iter().any(|child| matches!(child, Inline::Text { content, .. } if content == "outside"))
    )));
}

#[test]
fn malformed_or_incomplete_html_is_not_exposed_as_raw_html() {
    for source in [
        "prefix <em\n",
        "prefix <em class=\"unterminated\n",
        "prefix <!-- missing\n",
        "prefix <?instruction\n",
        "prefix <!DOCTYPE\n",
        "prefix <![CDATA[missing\n",
    ] {
        let document = parse_md(source);
        let raw = document.nodes.iter().flat_map(|node| match node {
            Block::Paragraph { content, .. } => inline_raw_html(content),
            _ => Vec::new(),
        });
        assert_eq!(
            raw.count(),
            0,
            "source unexpectedly exposed raw HTML: {source:?}"
        );
    }
}

#[test]
fn utf8_and_crlf_html_spans_remain_source_backed() {
    let source = "한글 <strong>굵게</strong>\r\n다음 <br>\r\n";
    let document = parse_md(source);
    let content = document
        .nodes
        .iter()
        .find_map(|node| match node {
            Block::Paragraph { content, .. } => Some(content.as_slice()),
            _ => None,
        })
        .expect("expected paragraph");

    assert_inline_raw_spans(source, content);
    let raw = inline_raw_html(content);
    assert_eq!(
        raw.iter().map(|(value, _)| *value).collect::<Vec<_>>(),
        vec!["<strong>", "</strong>", "<br>",]
    );
}

#[test]
fn block_html_utf8_and_crlf_span_remains_source_backed() {
    let source = "前\r\n\r\n<div>\r\n한글 **raw**\r\n</div>\r\n\r\n後\r\n";
    let document = parse_md(source);
    let (raw, span) = document
        .nodes
        .iter()
        .find_map(|node| match node {
            Block::RawHtml { source, span } => Some((source, span)),
            _ => None,
        })
        .expect("expected block HTML");
    assert_eq!(source.get(span.start..span.end), Some(raw.as_str()));
    assert_eq!(raw, "<div>\r\n한글 **raw**\r\n</div>\r\n");
}

#[test]
fn quarkdown_mode_keeps_raw_html_at_the_markdown_frontend_boundary() {
    let source = ".text {value} <em>HTML</em>\n";
    let document = parse_qd(source);
    let Block::Paragraph { content, .. } = &document.nodes[0] else {
        panic!("expected paragraph, got {:?}", document.nodes);
    };

    assert!(content
        .iter()
        .any(|inline| matches!(inline, Inline::DirectiveCall { name, .. } if name == "text")));
    assert_eq!(
        inline_raw_html(content)
            .iter()
            .map(|(value, _)| *value)
            .collect::<Vec<_>>(),
        vec!["<em>", "</em>"]
    );
    assert_inline_raw_spans(source, content);
}

#[test]
fn pinned_rushdown_html_comment_shapes_and_spans_are_recorded() {
    for (source, expected) in [
        ("before <!-- note --> after\n", ("<!-- note -->", 7, 20)),
        (
            "before <!-- this is a --\ncomment - with hyphens --> after\n",
            ("<!-- this is a --\ncomment - with hyphens -->", 7, 51),
        ),
        ("before <!--> after\n", ("<!-->", 7, 12)),
        ("before <!---> after\n", ("<!--->", 7, 13)),
        ("한글 <!-- note --> 뒤\r\n", ("<!-- note -->", 7, 20)),
    ] {
        let document = parse_md(source);
        let Block::Paragraph { content, .. } = &document.nodes[0] else {
            panic!("expected paragraph, got {:?}", document.nodes);
        };
        assert_eq!(
            inline_raw_html(content),
            vec![(
                expected.0,
                arkst_source::ByteSpan::new(expected.1, expected.2)
            )]
        );
        assert_inline_raw_spans(source, content);
    }

    for (source, expected) in [
        ("<!-- -->\n", "<!-- -->\n"),
        (
            "<!-- Foo\nbar\n   baz -->\nafter\n",
            "<!-- Foo\nbar\n   baz -->\n",
        ),
        ("  <!-- indented -->\nafter\n", "  <!-- indented -->\n"),
        ("<!-- -->\r\n", "<!-- -->\r\n"),
        (
            "  <!-- Foo\r\nbar\r\n   baz -->\r\n",
            "  <!-- Foo\r\nbar\r\n   baz -->\r\n",
        ),
    ] {
        let document = parse_md(source);
        let (raw, span) = document
            .nodes
            .iter()
            .find_map(|node| match node {
                Block::RawHtml { source, span } => Some((source.as_str(), *span)),
                _ => None,
            })
            .expect("expected parser-owned raw HTML block");
        assert_eq!(raw, expected);
        assert_eq!(source.get(span.start..span.end), Some(expected));
        assert_eq!(span, arkst_source::ByteSpan::new(0, expected.len()));
    }
}

#[test]
fn pinned_rushdown_comments_preserve_list_and_code_block_separators() {
    let lists = parse_md("- foo\n- bar\n\n<!-- -->\n\n- baz\n- bim\n");
    assert!(matches!(
        lists.nodes.as_slice(),
        [
            Block::UnorderedList { .. },
            Block::RawHtml { .. },
            Block::UnorderedList { .. }
        ]
    ));

    let list_and_code = parse_md("- foo\n- bar\n\n<!-- -->\n\n    code\n");
    assert!(matches!(
        list_and_code.nodes.as_slice(),
        [
            Block::UnorderedList { .. },
            Block::RawHtml { .. },
            Block::CodeBlock { .. }
        ]
    ));
}

#[test]
fn pinned_rushdown_comment_like_blocks_remain_one_source_backed_span() {
    for source in [
        "<!-- foo -->*bar*\n",
        "<!--> foo -->\n",
        "<!---> VISIBLE -->\n",
        "<!-- a --><!-- b -->\n",
        "<!-- unterminated\nvisible\n",
    ] {
        let document = parse_md(source);
        assert!(matches!(
            document.nodes.as_slice(),
            [Block::RawHtml { source: raw, span }] if raw == source
                && *span == arkst_source::ByteSpan::new(0, source.len())
        ));
    }
}
