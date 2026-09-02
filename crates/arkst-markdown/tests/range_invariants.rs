use arkst_markdown::ast::{Block, CallArgument, Document, Inline, Value};
use arkst_markdown::{parse_with_mode, Mode};
use arkst_source::ByteSpan;
use proptest::prelude::*;

fn valid(span: ByteSpan, source: &str) -> bool {
    span.start <= span.end
        && span.end <= source.len()
        && source.is_char_boundary(span.start)
        && source.is_char_boundary(span.end)
}

fn check_value(value: &Value, source: &str) {
    if let Value::Content(inlines) = value {
        for inline in inlines {
            check_inline(inline, source);
        }
    }
}

fn check_inline(inline: &Inline, source: &str) {
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
        | Inline::SoftBreak { span } => span,
    };
    assert!(
        valid(*span, source),
        "invalid inline span {span:?} for {source:?}"
    );
    match inline {
        Inline::Emphasis { content, .. }
        | Inline::Strong { content, .. }
        | Inline::Strikethrough { content, .. }
        | Inline::Link { content, .. }
        | Inline::Image { content, .. } => {
            for child in content {
                check_inline(child, source);
            }
        }
        Inline::DirectiveCall {
            arguments, body, ..
        } => {
            for argument in arguments {
                match argument {
                    CallArgument::Positional { value, span } => {
                        assert!(valid(*span, source));
                        check_value(value, source);
                    }
                    CallArgument::Named(argument) => {
                        assert!(valid(argument.name_span, source));
                        assert!(valid(argument.value_span, source));
                        assert!(valid(argument.span, source));
                        check_value(&argument.value, source);
                    }
                }
            }
            if let Some(body) = body {
                for child in body {
                    check_inline(child, source);
                }
            }
        }
        _ => {}
    }
}

fn check_block(block: &Block, source: &str) {
    let span = match block {
        Block::Heading { span, .. }
        | Block::Paragraph { span, .. }
        | Block::Blockquote { span, .. }
        | Block::UnorderedList { span, .. }
        | Block::OrderedList { span, .. }
        | Block::CodeBlock { span, .. }
        | Block::ThematicBreak { span }
        | Block::DirectiveCall { span, .. }
        | Block::Metadata { span, .. }
        | Block::Table { span, .. }
        | Block::RawHtml { span, .. }
        | Block::Unsupported { span, .. } => span,
    };
    assert!(
        valid(*span, source),
        "invalid block span {span:?} for {source:?}"
    );
    match block {
        Block::Heading { content, .. } | Block::Paragraph { content, .. } => {
            for inline in content {
                check_inline(inline, source);
            }
        }
        Block::Blockquote { content, .. } => {
            for child in content {
                check_block(child, source);
            }
        }
        Block::UnorderedList { items, .. } | Block::OrderedList { items, .. } => {
            for item in items {
                assert!(valid(item.span, source));
                for child in &item.content {
                    check_block(child, source);
                }
            }
        }
        Block::Table { header, rows, .. } => {
            check_table_row(header, source);
            for row in rows {
                check_table_row(row, source);
            }
        }
        Block::DirectiveCall {
            arguments,
            body,
            lambda_header,
            ..
        } => {
            for argument in arguments {
                match argument {
                    CallArgument::Positional { value, span } => {
                        assert!(valid(*span, source));
                        check_value(value, source);
                    }
                    CallArgument::Named(argument) => {
                        assert!(valid(argument.name_span, source));
                        assert!(valid(argument.value_span, source));
                        assert!(valid(argument.span, source));
                        check_value(&argument.value, source);
                    }
                }
            }
            if let Some(header) = lambda_header {
                assert!(valid(header.span, source));
                for parameter in &header.parameters {
                    assert!(valid(parameter.name_span, source));
                    assert!(valid(parameter.span, source));
                }
            }
            if let Some(body) = body {
                for child in body {
                    check_block(child, source);
                }
            }
        }
        _ => {}
    }
}

fn check_table_row(row: &arkst_markdown::ast::TableRow, source: &str) {
    assert!(valid(row.span, source));
    for cell in &row.cells {
        assert!(valid(cell.span, source));
        for inline in &cell.content {
            check_inline(inline, source);
        }
    }
}

fn check_document(document: &Document, source: &str) {
    for block in &document.nodes {
        check_block(block, source);
    }
    if let Some(front_matter) = &document.front_matter {
        assert!(valid(front_matter.span, source));
    }
}

#[test]
fn adversarial_utf8_and_markdown_ranges_are_source_backed() {
    let inputs = [
        "한글 **강조**\n",
        "界 .text {赤} 😀\r\n",
        "e\u{301} *combining*\n",
        "👩‍🔬 ` .foo {bar} `\n",
        "> - .align {center}\n>     **본문**\n",
        "- one\n  - two\n    - three\n",
        "[한글](https://example.com) ![그림](image.png)\n",
        "| a | b |\n| - | - |\n| 1 | 2 |\n",
        "```text\n.foo {bar}\n```\n",
        ".align {center}\n    한글 **본문**\n",
        "<span>raw</span> **x**\n",
        "*** unmatched ** [bad](\n",
        "",
    ];
    for source in inputs {
        for mode in [Mode::Markdown, Mode::Quarkdown] {
            let output = parse_with_mode(source, mode);
            check_document(&output.document, source);
        }
    }
}

proptest! {
    #[test]
    fn arbitrary_valid_utf8_has_only_valid_source_ranges(
        source in proptest::collection::vec(any::<char>(), 0..=64)
            .prop_map(|characters| characters.into_iter().collect::<String>())
    ) {
        let output = parse_with_mode(&source, Mode::Quarkdown);
        check_document(&output.document, &source);
    }
}
