use arkst_markdown::{parse_md, parse_qd, Block, HeadingKind, Inline};

fn decorative_heading(block: &Block) -> (usize, &[Inline], arkst_source::ByteSpan) {
    let Block::Heading {
        kind: HeadingKind::Decorative,
        level,
        content,
        span,
    } = block
    else {
        panic!("expected decorative heading, got {block:?}");
    };
    (*level, content, *span)
}

#[test]
fn decorative_heading_interrupts_paragraph_and_retains_identity_and_provenance() {
    let source = "Before witness\n##! Decorative *한글*\nAfter witness\n";
    let document = parse_qd(source);

    let [Block::Paragraph { .. }, heading, Block::Paragraph { .. }] = document.nodes.as_slice()
    else {
        panic!(
            "decorative heading must interrupt the paragraph: {:?}",
            document.nodes
        );
    };
    let (level, content, span) = decorative_heading(heading);
    assert_eq!(level, 2);
    assert!(matches!(
        content,
        [Inline::Text { content, .. }, Inline::Emphasis { .. }] if content == "Decorative "
    ));
    assert_eq!(
        source.get(span.start..span.end),
        Some("##! Decorative *한글*")
    );

    let markdown = parse_md(source);
    assert!(markdown.nodes.iter().all(|block| !matches!(
        block,
        Block::Heading {
            kind: HeadingKind::Decorative,
            ..
        }
    )));
}

#[test]
fn decorative_heading_matches_the_closed_marker_and_container_boundaries() {
    let source = "#! Zero\n\n #! One\n\n  #! Two\n\n   #! Three\n\n    #! Four\n\n> #! Quote\n\n- #! List\n\n#!NoSpace\n\n#######! Too deep\n\n#!\r\n\n##! UTF-8 제목\r\n";
    let document = parse_qd(source);

    for (index, level) in [(0, 1), (1, 1), (2, 1), (3, 1)] {
        assert_eq!(decorative_heading(&document.nodes[index]).0, level);
    }
    assert!(matches!(document.nodes[4], Block::CodeBlock { .. }));
    let Block::Blockquote { content, .. } = &document.nodes[5] else {
        panic!("expected blockquote: {:?}", document.nodes);
    };
    assert_eq!(decorative_heading(&content[0]).0, 1);
    let Block::UnorderedList { items, .. } = &document.nodes[6] else {
        panic!("expected list: {:?}", document.nodes);
    };
    assert_eq!(decorative_heading(&items[0].content[0]).0, 1);
    assert!(matches!(document.nodes[7], Block::Paragraph { .. }));
    assert!(matches!(document.nodes[8], Block::Paragraph { .. }));
    let (empty_level, empty, _) = decorative_heading(&document.nodes[9]);
    assert_eq!(empty_level, 1);
    assert!(empty.is_empty());
    let (utf8_level, utf8, _) = decorative_heading(&document.nodes[10]);
    assert_eq!(utf8_level, 2);
    assert!(utf8
        .iter()
        .any(|inline| matches!(inline, Inline::Text { content, .. } if content.contains("제목"))));
}

#[test]
fn decorative_heading_uses_atx_closing_marker_semantics() {
    let source = "#! Heading #\n\n##! Heading ###\n\n#! #\n\n#! Heading###\n";
    let document = parse_qd(source);
    assert_eq!(document.nodes.len(), 4, "{:?}", document.nodes);

    for (index, level) in [(0, 1), (1, 2)] {
        let (actual_level, content, _) = decorative_heading(&document.nodes[index]);
        assert_eq!(actual_level, level);
        assert!(matches!(
            content,
            [Inline::Text { content, .. }] if content == "Heading"
        ));
    }
    assert!(decorative_heading(&document.nodes[2]).1.is_empty());
    assert!(matches!(
        decorative_heading(&document.nodes[3]).1,
        [Inline::Text { content, .. }] if content == "Heading###"
    ));
}

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
        ..
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
        ..
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
