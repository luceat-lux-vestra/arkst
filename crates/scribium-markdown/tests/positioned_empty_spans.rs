use scribium_markdown::ast::{Block, Inline};
use scribium_markdown::parse_md;
use scribium_source::ByteSpan;

fn assert_source_span(source: &str, span: ByteSpan, expected: &str) {
    assert!(span.start <= span.end);
    assert!(span.end <= source.len());
    assert!(source.is_char_boundary(span.start));
    assert!(source.is_char_boundary(span.end));
    assert_eq!(source.get(span.start..span.end), Some(expected));
}

#[test]
fn marker_only_blocks_keep_original_line_spans() {
    let source = "intro\n\n  * * *\n\nfinal";
    let document = parse_md(source);
    let Block::ThematicBreak { span } = &document.nodes[1] else {
        panic!("expected thematic break, got {:?}", document.nodes);
    };
    assert_source_span(source, *span, "* * *\n");

    let source = "##\n# \n###\n";
    let document = parse_md(source);
    assert_eq!(document.nodes.len(), 3);
    for (block, expected) in document.nodes.iter().zip(["##\n", "# \n", "###\n"]) {
        let Block::Heading { content, span, .. } = block else {
            panic!("expected empty heading, got {block:?}");
        };
        assert!(content.is_empty());
        assert_source_span(source, *span, expected);
    }
}

#[test]
fn marker_only_blocks_work_in_nested_and_line_ending_contexts() {
    let source = "- first\r\n- * * *\r\n";
    let document = parse_md(source);
    let Block::UnorderedList { items, .. } = &document.nodes[0] else {
        panic!("expected unordered list, got {:?}", document.nodes);
    };
    let Block::ThematicBreak { span } = items[1].content.first().expect("nested break") else {
        panic!("expected nested thematic break, got {:?}", items[1].content);
    };
    assert_source_span(source, *span, "* * *\r\n");

    let source = "한글\r\n\r\n---\r\n끝";
    let document = parse_md(source);
    let Block::ThematicBreak { span } = &document.nodes[1] else {
        panic!("expected UTF-8 thematic break, got {:?}", document.nodes);
    };
    assert_source_span(source, *span, "---\r\n");

    let source = ">\r\n";
    let document = parse_md(source);
    let Block::Blockquote { content, span } = &document.nodes[0] else {
        panic!("expected empty blockquote, got {:?}", document.nodes);
    };
    assert!(content.is_empty());
    assert_source_span(source, *span, ">\r\n");

    let source = "-\n";
    let document = parse_md(source);
    let Block::UnorderedList { items, .. } = &document.nodes[0] else {
        panic!("expected empty list item, got {:?}", document.nodes);
    };
    assert!(items[0].content.is_empty());
    assert_source_span(source, items[0].span, "-\n");
}

#[test]
fn empty_label_links_and_images_keep_adjacent_source_spans() {
    let source = "[](/target) ![](/image)\r\n";
    let document = parse_md(source);
    let Block::Paragraph { content, .. } = &document.nodes[0] else {
        panic!("expected paragraph, got {:?}", document.nodes);
    };

    let mut saw_link = false;
    let mut saw_image = false;
    for inline in content {
        match inline {
            Inline::Link { span, .. } => {
                assert_source_span(source, *span, "[](/target)");
                saw_link = true;
            }
            Inline::Image { span, .. } => {
                assert_source_span(source, *span, "![](/image)");
                saw_image = true;
            }
            _ => {}
        }
    }
    assert!(
        saw_link && saw_image,
        "unexpected inline content: {content:?}"
    );

    let source = "[]() ![]()\n";
    let document = parse_md(source);
    let Block::Paragraph { content, .. } = &document.nodes[0] else {
        panic!(
            "expected empty-destination paragraph, got {:?}",
            document.nodes
        );
    };
    assert!(content.iter().any(|inline| matches!(
        inline,
        Inline::Link { span, .. } if &source[span.start..span.end] == "[]()"
    )));
    assert!(content.iter().any(|inline| matches!(
        inline,
        Inline::Image { span, .. } if &source[span.start..span.end] == "![]()"
    )));
}
