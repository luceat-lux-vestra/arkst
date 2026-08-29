use scribium_markdown::{parse_with_mode, Block, Mode};

fn root_raw_body(source: &str) -> scribium_markdown::ast::RawBody {
    let output = parse_with_mode(source, Mode::Quarkdown);
    assert!(output.diagnostics.is_empty(), "unexpected: {output:?}");
    let Block::DirectiveCall {
        raw_body: Some(raw_body),
        ..
    } = &output.document.nodes[0]
    else {
        panic!(
            "expected a source-backed root call body, got {:?}",
            output.document.nodes
        );
    };
    raw_body.clone()
}

#[test]
fn raw_body_value_matches_trim_indent_trim_end_for_tabs_mixed_indent_and_crlf() {
    let source = ".theme\r\n\t\tα\r\n\t\t  β \r\n\t\t\r\n\t\tγ\t \r\n\r\n";
    let raw_body = root_raw_body(source);

    assert_eq!(
        &source[raw_body.span.start..raw_body.span.end],
        raw_body.source_text
    );
    assert_eq!(
        raw_body.source_text,
        "\r\n\t\tα\r\n\t\t  β \r\n\t\t\r\n\t\tγ\t \r\n\r\n"
    );
    assert_eq!(raw_body.text, "α\n  β \n\nγ");
}

#[test]
fn complete_body_range_preserves_leading_and_trailing_blank_lines() {
    let source = ".docdescription\n\n\n    hello\n\n\n";
    let raw_body = root_raw_body(source);

    assert_eq!(raw_body.source_text, "\n\n\n    hello\n\n\n");
    assert_eq!(
        &source[raw_body.span.start..raw_body.span.end],
        raw_body.source_text
    );
    assert_eq!(raw_body.text, "\n\nhello");
}

#[test]
fn blank_only_body_has_no_body_value_but_is_not_a_parse_error() {
    let output = parse_with_mode(".docdescription\n\n\n", Mode::Quarkdown);
    assert!(output.diagnostics.is_empty(), "unexpected: {output:?}");
    let Block::DirectiveCall { body, raw_body, .. } = &output.document.nodes[0] else {
        panic!("expected a directive call, got {:?}", output.document.nodes);
    };
    assert!(body.is_none());
    assert!(raw_body.is_none());
}

#[test]
fn nested_body_uses_the_same_trimmed_value_independent_of_reader_base_offset() {
    let top_level = root_raw_body(".theme\n\n    alpha\n        beta\n\n");
    let nested_source = ".function {wrap}\n    .theme\n\n        alpha\n            beta\n\n";
    let output = parse_with_mode(nested_source, Mode::Quarkdown);
    assert!(output.diagnostics.is_empty(), "unexpected: {output:?}");
    let Block::DirectiveCall {
        body: Some(body), ..
    } = &output.document.nodes[0]
    else {
        panic!("expected function body, got {:?}", output.document.nodes);
    };
    let [Block::DirectiveCall {
        raw_body: Some(nested),
        ..
    }] = body.as_slice()
    else {
        panic!("expected nested theme body, got {body:?}");
    };

    assert_eq!(nested.text, top_level.text);
    assert_eq!(
        &nested_source[nested.span.start..nested.span.end],
        nested.source_text
    );
}
