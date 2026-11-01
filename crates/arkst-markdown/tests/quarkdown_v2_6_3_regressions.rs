use arkst_markdown::{parse_qd, Block, Inline};

#[test]
fn nested_emphasis_and_deep_link_destination_retain_inline_structure() {
    let source =
        "***Atlas core** trailing veil*\n\n[Portal witness](https://example.test/a(b(c(d))))\n";
    let document = parse_qd(source);

    let [Block::Paragraph {
        content: emphasis, ..
    }, Block::Paragraph { content: link, .. }] = document.nodes.as_slice()
    else {
        panic!("unexpected v2.6.3 inline shape: {:?}", document.nodes);
    };
    let [Inline::Emphasis {
        content: emphasized,
        span: emphasis_span,
    }] = emphasis.as_slice()
    else {
        panic!("outer emphasis must remain open: {emphasis:?}");
    };
    assert!(matches!(
        emphasized.as_slice(),
        [Inline::Strong { .. }, Inline::Text { content, .. }] if content == " trailing veil"
    ));
    assert_eq!(
        source.get(emphasis_span.start..emphasis_span.end),
        Some("***Atlas core** trailing veil")
    );

    let [Inline::Link {
        destination,
        span: link_span,
        ..
    }] = link.as_slice()
    else {
        panic!("nested destination must remain one link: {link:?}");
    };
    assert_eq!(destination, "https://example.test/a(b(c(d)))");
    assert_eq!(
        source.get(link_span.start..link_span.end),
        Some("[Portal witness](https://example.test/a(b(c(d))))")
    );
}

#[test]
fn ordered_bare_heading_and_tabs_only_boundaries_remain_separate_blocks() {
    let source = "17. First witness\n18. Second witness\n\n## Parent witness\n###\nAfter bare heading\n\nTab first\n\t\t\nTab second\n";
    let document = parse_qd(source);

    let [Block::OrderedList { start, items, .. }, Block::Heading { level: 2, .. }, Block::Heading {
        level: 3,
        content: empty,
        span: bare_span,
    }, Block::Paragraph { .. }, Block::Paragraph {
        content: first_tab, ..
    }, Block::Paragraph {
        content: second_tab,
        ..
    }] = document.nodes.as_slice()
    else {
        panic!("unexpected v2.6.3 block shape: {:?}", document.nodes);
    };

    assert_eq!(*start, 17);
    assert_eq!(items.len(), 2);
    assert!(empty.is_empty());
    assert_eq!(source.get(bare_span.start..bare_span.end), Some("###\n"));
    assert!(!first_tab.is_empty());
    assert!(first_tab
        .iter()
        .all(|inline| matches!(inline, Inline::Text { .. })));
    assert!(!second_tab.is_empty());
    assert!(second_tab
        .iter()
        .all(|inline| matches!(inline, Inline::Text { .. })));
}

#[test]
fn multiline_setext_heading_keeps_every_line_and_source_break() {
    let source = "North line\nsouth line\n===\n";
    let document = parse_qd(source);

    let [Block::Heading {
        level: 1,
        content,
        span,
    }] = document.nodes.as_slice()
    else {
        panic!(
            "multiline setext text must remain one heading: {:?}",
            document.nodes
        );
    };
    assert!(matches!(
        content.as_slice(),
        [
            Inline::Text { .. },
            Inline::Text { .. },
            Inline::SoftBreak { .. },
            Inline::Text { .. },
            Inline::Text { .. },
        ]
    ));
    assert_eq!(
        source.get(span.start..span.end),
        Some("North line\nsouth line")
    );
}
