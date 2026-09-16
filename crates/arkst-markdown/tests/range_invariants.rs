use arkst_markdown::{Block, Document, Inline, ListItem, Parser, ParserOptions, Value};
use arkst_source::ByteSpan;
use proptest::prelude::*;

fn valid(span: ByteSpan, source: &str) -> bool {
    span.start <= span.end && span.end <= source.len()
}

fn check_value(value: &Value, source: &str) {
    match value {
        Value::Content(inlines) => {
            for inline in inlines {
                check_inline(inline, source);
            }
        }
        Value::InlineBody {
            content,
            parameters,
            body,
            span,
        } => {
            assert!(valid(*span, source));
            for inline in content {
                check_inline(inline, source);
            }
            if let Some(parameters) = parameters {
                assert!(valid(parameters.span, source));
                for parameter in &parameters.parameters {
                    assert!(valid(parameter.name_span, source));
                    assert!(valid(parameter.span, source));
                }
            }
            for inline in body {
                check_inline(inline, source);
            }
        }
        Value::Lambda {
            parameters,
            body,
            span,
        } => {
            assert!(valid(*span, source));
            if let Some(parameters) = parameters {
                assert!(valid(parameters.span, source));
                for parameter in &parameters.parameters {
                    assert!(valid(parameter.name_span, source));
                    assert!(valid(parameter.span, source));
                }
            }
            for inline in body {
                check_inline(inline, source);
            }
        }
        _ => {}
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
        | Inline::HardBreak { span }
        | Inline::SoftBreak { span }
        | Inline::Unsupported { span, .. } => span,
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
                    arkst_markdown::CallArgument::Positional { value, span } => {
                        assert!(valid(*span, source));
                        check_value(value, source);
                    }
                    arkst_markdown::CallArgument::Named(argument) => {
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
        | Block::PageBreak { span }
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
                check_list_item(item, source);
            }
        }
        Block::Table { header, rows, .. } => {
            for cell in &header.cells {
                assert!(valid(cell.span, source));
                for inline in &cell.content {
                    check_inline(inline, source);
                }
            }
            for row in rows {
                assert!(valid(row.span, source));
                for cell in &row.cells {
                    assert!(valid(cell.span, source));
                    for inline in &cell.content {
                        check_inline(inline, source);
                    }
                }
            }
        }
        Block::DirectiveCall {
            arguments,
            chain,
            body,
            raw_body,
            lambda_header,
            ..
        } => {
            for argument in arguments {
                match argument {
                    arkst_markdown::CallArgument::Positional { value, span } => {
                        assert!(valid(*span, source));
                        check_value(value, source);
                    }
                    arkst_markdown::CallArgument::Named(argument) => {
                        assert!(valid(argument.name_span, source));
                        assert!(valid(argument.value_span, source));
                        assert!(valid(argument.span, source));
                        check_value(&argument.value, source);
                    }
                }
            }
            for segment in chain {
                assert!(valid(segment.span, source));
                assert!(valid(segment.name_span, source));
                assert!(valid(segment.head_span, source));
                for argument in &segment.arguments {
                    match argument {
                        arkst_markdown::CallArgument::Positional { value, span } => {
                            assert!(valid(*span, source));
                            check_value(value, source);
                        }
                        arkst_markdown::CallArgument::Named(argument) => {
                            assert!(valid(argument.name_span, source));
                            assert!(valid(argument.value_span, source));
                            assert!(valid(argument.span, source));
                            check_value(&argument.value, source);
                        }
                    }
                }
            }
            if let Some(body) = body {
                for child in body {
                    check_block(child, source);
                }
            }
            if let Some(raw_body) = raw_body {
                assert!(valid(raw_body.span, raw_body.source.as_str()));
            }
            if let Some(lambda_header) = lambda_header {
                assert!(valid(lambda_header.span, source));
                for parameter in &lambda_header.parameters {
                    assert!(valid(parameter.name_span, source));
                    assert!(valid(parameter.span, source));
                }
            }
        }
        _ => {}
    }
}

fn check_list_item(item: &ListItem, source: &str) {
    assert!(valid(item.span, source));
    for child in &item.content {
        check_block(child, source);
    }
}

fn check_document(document: &Document, source: &str) {
    if let Some(front_matter) = &document.front_matter {
        assert!(valid(front_matter.span, source));
    }
    for block in &document.nodes {
        check_block(block, source);
    }
}

proptest! {
    #[test]
    fn parser_ranges_stay_within_source(source in ".{0,512}") {
        let parser = Parser::new(ParserOptions::default());
        let result = parser.parse(&source);
        check_document(&result.document, &source);
    }
}
