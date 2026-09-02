use arkst_markdown::{parse_with_diagnostics, Block, Document, Inline};
use arkst_source::ByteSpan;

fn first_code_span(document: &Document) -> Option<(&str, ByteSpan)> {
    fn inlines(values: &[Inline]) -> Option<(&str, ByteSpan)> {
        values.iter().find_map(|inline| match inline {
            Inline::Code { content, span } => Some((content.as_str(), *span)),
            Inline::Emphasis { content, .. }
            | Inline::Strong { content, .. }
            | Inline::Link { content, .. }
            | Inline::Image { content, .. }
            | Inline::Strikethrough { content, .. } => inlines(content),
            Inline::DirectiveCall { body, .. } => body.as_deref().and_then(inlines),
            _ => None,
        })
    }

    fn blocks(values: &[Block]) -> Option<(&str, ByteSpan)> {
        values.iter().find_map(|block| match block {
            Block::Heading { content, .. } | Block::Paragraph { content, .. } => inlines(content),
            Block::Blockquote { content, .. } => blocks(content),
            Block::UnorderedList { items, .. } | Block::OrderedList { items, .. } => {
                items.iter().find_map(|item| blocks(&item.content))
            }
            Block::Table { header, rows, .. } => header
                .cells
                .iter()
                .find_map(|cell| inlines(&cell.content))
                .or_else(|| {
                    rows.iter()
                        .find_map(|row| row.cells.iter().find_map(|cell| inlines(&cell.content)))
                }),
            Block::DirectiveCall { body, .. } => body.as_deref().and_then(blocks),
            _ => None,
        })
    }

    blocks(&document.nodes)
}

fn assert_code_span(source: &str, expected_content: &str, expected_span: ByteSpan) {
    let output = parse_with_diagnostics(source);
    assert!(
        output.diagnostics.is_empty(),
        "unexpected diagnostics for {source:?}: {:?}",
        output.diagnostics
    );
    let (content, span) = first_code_span(&output.document)
        .unwrap_or_else(|| panic!("expected an inline code span in {source:?}"));

    assert_eq!(content, expected_content);
    assert_eq!(span, expected_span);
    assert!(source.is_char_boundary(span.start));
    assert!(source.is_char_boundary(span.end));
    assert_eq!(
        &source[span.start..span.end],
        &source[expected_span.start..expected_span.end]
    );
}

#[test]
fn single_backtick_exact_run_recovers_the_original_span() {
    let source = "`code`";
    assert_code_span(source, "code", ByteSpan::new(0, source.len()));
}

#[test]
fn multi_backtick_exact_run_recovers_the_original_span() {
    let source = "``foo`bar``";
    assert_code_span(source, "foo`bar", ByteSpan::new(0, source.len()));
}

#[test]
fn longer_backtick_run_does_not_close_a_code_span() {
    let source = "``foo```bar``";
    assert_code_span(source, "foo```bar", ByteSpan::new(0, source.len()));
}

#[test]
fn shorter_backtick_run_does_not_close_a_code_span() {
    let source = "``foo`bar``";
    assert_code_span(source, "foo`bar", ByteSpan::new(0, source.len()));
}

#[test]
fn multiple_nonmatching_runs_are_skipped_before_the_exact_closer() {
    let source = "``a`b```c``";
    assert_code_span(source, "a`b```c", ByteSpan::new(0, source.len()));
}

#[test]
fn adjacent_backticks_are_treated_as_one_maximal_run() {
    let source = "```foo``bar```";
    assert_code_span(source, "foo``bar", ByteSpan::new(0, source.len()));
}

#[test]
fn utf8_before_and_inside_code_span_keeps_byte_provenance() {
    let source = "앞 ``한글```값`` 뒤";
    let start = source.find("``한글").expect("code span opener");
    let end = start + "``한글```값``".len();
    assert_code_span(source, "한글```값", ByteSpan::new(start, end));
}

#[test]
fn crlf_around_code_span_keeps_byte_provenance() {
    let source = "before\r\n``foo```bar``\r\nafter\r\n";
    let start = source.find("``foo").expect("code span opener");
    let end = start + "``foo```bar``".len();
    assert_code_span(source, "foo```bar", ByteSpan::new(start, end));
}

#[test]
fn nested_container_code_span_keeps_original_source_offsets() {
    let source = "- ``foo```bar``\r\n";
    let start = source.find("``foo").expect("code span opener");
    let end = start + "``foo```bar``".len();
    assert_code_span(source, "foo```bar", ByteSpan::new(start, end));
}

#[test]
fn gfm_table_code_span_uses_rushdown_value_and_original_source_span() {
    let source = "| value |\n| --- |\n| `\\|` |\n";
    let output = parse_with_diagnostics(source);
    assert!(output.diagnostics.is_empty(), "{output:?}");
    let Block::Table { rows, .. } = &output.document.nodes[0] else {
        panic!("expected a table")
    };
    let Inline::Code { content, span } = &rows[0].cells[0].content[0] else {
        panic!("expected a code span")
    };
    assert_eq!(content, "|");
    assert_eq!(&source[span.start..span.end], "`\\|`");
}

#[test]
fn code_span_boundary_does_not_emit_zero_width_text() {
    let source = "` `\n`  `\n";
    let output = parse_with_diagnostics(source);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let Block::Paragraph { content, .. } = &output.document.nodes[0] else {
        panic!("expected a paragraph");
    };

    assert!(matches!(
        content.as_slice(),
        [
            Inline::Code { content: first, span: first_span },
            Inline::SoftBreak { span: break_span },
            Inline::Code { content: second, span: second_span },
        ] if first == " "
            && *first_span == ByteSpan::new(0, 4)
            && *break_span == ByteSpan::new(4, 5)
            && second == "  "
            && *second_span == ByteSpan::new(5, 9)
    ));
}
