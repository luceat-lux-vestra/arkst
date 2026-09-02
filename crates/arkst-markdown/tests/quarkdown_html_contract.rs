use arkst_markdown::ast::{Block, CallArgument, Inline, Value};
use arkst_markdown::{parse_with_mode, Mode};

fn assert_span(source: &str, span: arkst_source::ByteSpan, expected: &str) {
    assert_eq!(&source[span.start..span.end], expected);
}

#[test]
fn html_braced_content_is_a_source_backed_content_argument() {
    let source = ".html {<em>world</em>}\n";
    let output = parse_with_mode(source, Mode::Quarkdown);
    assert!(
        output.diagnostics.is_empty(),
        "unexpected: {:?}",
        output.diagnostics
    );

    let Block::DirectiveCall {
        name,
        arguments,
        body: None,
        span,
        ..
    } = &output.document.nodes[0]
    else {
        panic!("expected .html block call, got {:?}", output.document.nodes);
    };
    assert_eq!(name, "html");
    assert_span(source, *span, ".html {<em>world</em>}");
    let [CallArgument::Positional {
        value: Value::Content(content),
        ..
    }] = arguments.as_slice()
    else {
        panic!("expected one content argument, got {arguments:?}");
    };
    assert!(matches!(
        content.as_slice(),
        [Inline::Text { content, span }]
            if content == "<em>world</em>"
                && &source[span.start..span.end] == "<em>world</em>"
    ));
}

#[test]
fn html_inline_call_preserves_text_call_text_order() {
    let source = "**Hello** .html {<em>world</em>}!\n";
    let output = parse_with_mode(source, Mode::Quarkdown);
    assert!(
        output.diagnostics.is_empty(),
        "unexpected: {:?}",
        output.diagnostics
    );

    let Block::Paragraph { content, .. } = &output.document.nodes[0] else {
        panic!("expected paragraph, got {:?}", output.document.nodes);
    };
    assert!(matches!(
        content.as_slice(),
        [
            Inline::Strong { .. },
            Inline::Text { content: space, .. },
            Inline::DirectiveCall { name, .. },
            Inline::Text { content: exclamation, .. },
        ] if space == " " && name == "html" && exclamation == "!"
    ));
}

#[test]
fn html_angle_text_does_not_widen_e3010_suppression_for_other_arguments() {
    let html = parse_with_mode(".html {<em>world</em>}\n", Mode::Quarkdown);
    assert!(
        html.diagnostics.is_empty(),
        "unexpected .html diagnostics: {:?}",
        html.diagnostics
    );

    for source in [
        ".foo {**hello** <em>world</em>}\n",
        ".foo {2 < 3 and 5 > 4 **hello**}\n",
    ] {
        let output = parse_with_mode(source, Mode::Quarkdown);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "E3010"),
            "expected E3010 for {source:?}, got {:?}",
            output.diagnostics
        );
    }
}

#[test]
fn html_indented_body_remains_parser_owned_raw_html() {
    let source = ".html\n    <div>\n        Hello\n    </div>\n";
    let output = parse_with_mode(source, Mode::Quarkdown);
    assert!(output.diagnostics.is_empty(), "unexpected: {output:?}");

    let Block::DirectiveCall {
        name,
        arguments,
        body: Some(body),
        raw_body: Some(raw_body),
        ..
    } = &output.document.nodes[0]
    else {
        panic!("expected .html body, got {:?}", output.document.nodes);
    };
    assert_eq!(name, "html");
    assert!(arguments.is_empty());
    let [Block::RawHtml { source: raw, span }] = body.as_slice() else {
        panic!("expected one raw HTML body node, got {body:?}");
    };
    assert_eq!(raw, "<div>\n        Hello\n    </div>\n");
    assert_span(source, *span, raw);
    assert_eq!(
        raw_body
            .source
            .slice(raw_body.span)
            .expect("raw body source span"),
        "\n    <div>\n        Hello\n    </div>\n"
    );
    assert_span(
        source,
        raw_body.span,
        "\n    <div>\n        Hello\n    </div>\n",
    );
}

#[test]
fn nested_raw_body_keeps_source_provenance_and_call_owned_indentation() {
    let source = ".function {setauthors}\n    .docauthors\n        - Callable\n            - email: callable@example.com\n\n";
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
        panic!("expected nested docauthors body, got {body:?}");
    };
    assert_eq!(
        raw_body
            .source
            .slice(raw_body.span)
            .expect("raw body source span"),
        "\n        - Callable\n            - email: callable@example.com\n\n"
    );
}
