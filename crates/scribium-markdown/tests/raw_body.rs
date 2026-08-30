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

fn raw_source(raw_body: &scribium_markdown::ast::RawBody) -> &str {
    raw_body
        .source
        .slice(raw_body.span)
        .expect("raw body span must address its source buffer")
}

#[test]
fn raw_body_keeps_one_shared_lossless_source_for_utf8_tabs_and_crlf() {
    let source = ".theme\r\n\t\tα\r\n\t\t  β \r\n\t\t\r\n\t\tγ\t \r\n\r\n";
    let raw_body = root_raw_body(source);

    assert_eq!(
        raw_source(&raw_body),
        &source[raw_body.span.start..raw_body.span.end]
    );
    assert_eq!(
        raw_source(&raw_body),
        "\r\n\t\tα\r\n\t\t  β \r\n\t\t\r\n\t\tγ\t \r\n\r\n"
    );
}

#[test]
fn complete_body_range_preserves_leading_and_trailing_blank_lines() {
    let source = ".docdescription\n\n\n    hello\n\n\n";
    let raw_body = root_raw_body(source);

    assert_eq!(raw_source(&raw_body), "\n\n\n    hello\n\n\n");
    assert_eq!(
        raw_source(&raw_body),
        &source[raw_body.span.start..raw_body.span.end]
    );
}

#[test]
fn body_token_starts_after_header_spaces_and_tabs() {
    for (source, expected) in [
        (".docdescription   \n    hello\n", "\n    hello\n"),
        (".docdescription\t\t\n    hello\n", "\n    hello\n"),
        (".docdescription   \r\n    hello\r\n", "\r\n    hello\r\n"),
    ] {
        let raw_body = root_raw_body(source);
        assert_eq!(raw_source(&raw_body), expected, "source: {source:?}");
        assert_eq!(
            raw_source(&raw_body),
            &source[raw_body.span.start..raw_body.span.end],
            "source: {source:?}"
        );
    }
}

#[test]
fn body_ownership_requires_a_literal_two_space_or_tab_prefix() {
    for (prefix, has_body) in [("  ", true), ("\t", true), (" \t", false), (" ", false)] {
        let source = format!(".theme\n{prefix}value\n");
        let output = parse_with_mode(&source, Mode::Quarkdown);
        assert!(output.diagnostics.is_empty(), "unexpected: {output:?}");
        let Block::DirectiveCall { raw_body, .. } = &output.document.nodes[0] else {
            panic!("expected directive call, got {:?}", output.document.nodes);
        };
        assert_eq!(raw_body.is_some(), has_body, "prefix: {prefix:?}");
    }
}

#[test]
fn continued_header_uses_the_final_separator_before_the_body_token() {
    let source = concat!(
        ".docdescription {title} \\",
        "\r\n    {subtitle}   \r\n",
        "    hello\r\n",
    );
    let raw_body = root_raw_body(source);

    assert_eq!(raw_source(&raw_body), "\r\n    hello\r\n");
    assert_eq!(
        raw_source(&raw_body),
        &source[raw_body.span.start..raw_body.span.end]
    );
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
fn nested_body_preserves_the_same_source_token_shape_independent_of_reader_base_offset() {
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

    assert_eq!(
        raw_source(nested),
        "\n\n        alpha\n            beta\n\n"
    );
    assert_eq!(
        raw_source(nested),
        &nested_source[nested.span.start..nested.span.end]
    );
}

#[test]
fn body_continuation_uses_each_line_indent_independently() {
    for (line_ending, first_indent, second_indent) in [("\n", "    ", "  "), ("\r\n", "\t", "  ")] {
        let source = format!(
            ".theme{line_ending}{first_indent}first{line_ending}{second_indent}second{line_ending}"
        );
        let raw_body = root_raw_body(&source);
        assert_eq!(
            raw_source(&raw_body),
            format!(
                "{line_ending}{first_indent}first{line_ending}{second_indent}second{line_ending}"
            )
        );
        assert_eq!(
            raw_source(&raw_body),
            &source[raw_body.span.start..raw_body.span.end]
        );
    }
}

#[test]
fn body_continuation_allows_indent_decrease_after_blank_lines() {
    let source = ".theme\n    first\n\n  second\n\n\tthird\n";
    let raw_body = root_raw_body(source);

    assert_eq!(
        raw_source(&raw_body),
        "\n    first\n\n  second\n\n\tthird\n"
    );
    assert_eq!(
        raw_source(&raw_body),
        &source[raw_body.span.start..raw_body.span.end]
    );
}

#[test]
fn body_continuation_matches_trim_indent_after_an_indent_decrease() {
    let source = ".theme\n    first\n  second\n";
    let raw_body = root_raw_body(source);

    assert_eq!(raw_source(&raw_body), "\n    first\n  second\n");
}

#[test]
fn structured_body_keeps_relative_indentation_after_a_shallower_line() {
    let source = ".theme\n      first\n        nested\n  sibling\n";
    let output = parse_with_mode(source, Mode::Quarkdown);
    assert!(output.diagnostics.is_empty(), "unexpected: {output:?}");
    let Block::DirectiveCall {
        body: Some(body), ..
    } = &output.document.nodes[0]
    else {
        panic!("expected structured body, got {:?}", output.document.nodes);
    };

    // The semantic body is equivalent to `trimIndent()` on the complete raw
    // token: the first line keeps four spaces relative to `sibling`, rather
    // than being flattened to the first line's indentation.
    assert!(
        matches!(body.first(), Some(Block::CodeBlock { source, .. }) if source == "first\n  nested\n")
    );
    assert!(matches!(body.last(), Some(Block::Paragraph { content, .. }) if content.len() == 1));
}

#[test]
fn nested_body_continuation_keeps_a_shallower_line_in_the_same_token() {
    let source = ".function {wrap}\n    .theme\n\n        first\n      second\n\n";
    let output = parse_with_mode(source, Mode::Quarkdown);
    assert!(output.diagnostics.is_empty(), "unexpected: {output:?}");
    let Block::DirectiveCall {
        body: Some(body), ..
    } = &output.document.nodes[0]
    else {
        panic!("expected function body, got {:?}", output.document.nodes);
    };
    let [Block::DirectiveCall {
        raw_body: Some(raw_body),
        ..
    }] = body.as_slice()
    else {
        panic!("expected nested theme body, got {body:?}");
    };

    assert_eq!(raw_source(raw_body), "\n\n        first\n      second\n\n");
    assert_eq!(
        raw_source(raw_body),
        &source[raw_body.span.start..raw_body.span.end]
    );
}

#[test]
fn nested_body_continuation_keeps_literal_prefix_rules_with_crlf() {
    let source = concat!(
        ".function {wrap}\r\n",
        "    .theme\r\n",
        "\r\n",
        "        first\r\n",
        "      second\r\n",
        "\r\n",
    );
    let output = parse_with_mode(source, Mode::Quarkdown);
    assert!(output.diagnostics.is_empty(), "unexpected: {output:?}");
    let Block::DirectiveCall {
        body: Some(body), ..
    } = &output.document.nodes[0]
    else {
        panic!("expected function body, got {:?}", output.document.nodes);
    };
    let [Block::DirectiveCall {
        raw_body: Some(raw_body),
        ..
    }] = body.as_slice()
    else {
        panic!("expected nested theme body, got {body:?}");
    };

    assert_eq!(
        raw_source(raw_body),
        "\r\n\r\n        first\r\n      second\r\n\r\n"
    );
    assert_eq!(
        raw_source(raw_body),
        &source[raw_body.span.start..raw_body.span.end]
    );
}
