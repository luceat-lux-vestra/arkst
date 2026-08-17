use scribium_markdown::ast::{Block, Document, Inline, ListItem, TableRow};
use scribium_markdown::parse_md;
use scribium_source::ByteSpan;

fn assert_inline_spans(source: &str, inline: &Inline) {
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
        | Inline::HardBreak { span }
        | Inline::SoftBreak { span }
        | Inline::Unsupported { span, .. } => *span,
    };
    assert_source_span(source, span);

    match inline {
        Inline::Emphasis { content, .. }
        | Inline::Strong { content, .. }
        | Inline::Link { content, .. }
        | Inline::Image { content, .. }
        | Inline::Strikethrough { content, .. } => {
            for child in content {
                assert_inline_spans(source, child);
            }
        }
        Inline::DirectiveCall { body, .. } => {
            if let Some(body) = body {
                for child in body {
                    assert_inline_spans(source, child);
                }
            }
        }
        Inline::Text { .. }
        | Inline::Code { .. }
        | Inline::RawHtml { .. }
        | Inline::HardBreak { .. }
        | Inline::SoftBreak { .. }
        | Inline::Unsupported { .. } => {}
    }
}

fn assert_block(source: &str, block: &Block) {
    match block {
        Block::Heading { content, span, .. } | Block::Paragraph { content, span } => {
            assert_source_span(source, *span);
            for inline in content {
                assert!(!matches!(inline, Inline::Text { content, .. } if content.is_empty()));
                assert_inline_spans(source, inline);
            }
        }
        Block::Blockquote { content, span }
        | Block::DirectiveCall {
            body: Some(content),
            span,
            ..
        } => {
            assert_source_span(source, *span);
            for child in content {
                assert_block(source, child);
            }
        }
        Block::UnorderedList { items, span } | Block::OrderedList { items, span, .. } => {
            assert_source_span(source, *span);
            for item in items {
                assert_list_item(source, item);
            }
        }
        Block::Table { header, rows, span } => {
            assert_source_span(source, *span);
            assert_row(source, header);
            for row in rows {
                assert_row(source, row);
            }
        }
        Block::CodeBlock { span, .. }
        | Block::ThematicBreak { span }
        | Block::RawHtml { span, .. }
        | Block::Unsupported { span, .. }
        | Block::Metadata { span, .. }
        | Block::DirectiveCall {
            body: None, span, ..
        } => assert_source_span(source, *span),
    }
}

fn assert_list_item(source: &str, item: &ListItem) {
    assert_source_span(source, item.span);
    for block in &item.content {
        assert_block(source, block);
    }
}

fn assert_row(source: &str, row: &TableRow) {
    assert_source_span(source, row.span);
    for cell in &row.cells {
        assert_source_span(source, cell.span);
        for inline in &cell.content {
            assert!(!matches!(inline, Inline::Text { content, .. } if content.is_empty()));
            assert_inline_spans(source, inline);
        }
    }
}

fn assert_source_span(source: &str, span: ByteSpan) {
    assert!(span.start <= span.end);
    assert!(span.end <= source.len());
    assert!(source.is_char_boundary(span.start));
    assert!(source.is_char_boundary(span.end));
}

#[test]
fn zero_length_text_segments_are_not_materialized_in_nested_inline_contexts() {
    let sources = [
        "앞 ``` ```\r\n뒤\r\n",
        "앞 **foo *bar **baz**\n값* bop**\r\n뒤\r\n",
        "- [링크](/url)  \r\n  ![이미지](/image)\r\n",
        "> Foo\r\n> <a href=\"bar\">\r\n> 값\r\n",
    ];

    for source in sources {
        let document: Document = parse_md(source);
        for block in &document.nodes {
            assert_block(source, block);
        }
    }
}
