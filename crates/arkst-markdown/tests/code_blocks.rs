use arkst_markdown::{parse_md, Block};
use arkst_source::ByteSpan;

#[test]
fn code_blocks_preserve_segment_newlines_padding_and_source_spans() {
    let source = "Preamble\n\n```rust extra\nlet α = 1;\nlet β = 2;\n```\n\n- nested\n\n  ```\n  line one\n  line two\n  ```\n";
    let document = parse_md(source);

    let Block::CodeBlock {
        source: body, span, ..
    } = &document.nodes[1]
    else {
        panic!("expected top-level fenced code block");
    };
    assert_eq!(body, "let α = 1;\nlet β = 2;\n");
    let start = source.find("```rust").expect("fence start");
    let end = source[start..]
        .find("```\n\n- nested")
        .map(|offset| start + offset + "```\n".len())
        .expect("fence end");
    assert_eq!(*span, ByteSpan::new(start, end));
    assert_eq!(
        source.get(span.start..span.end),
        Some("```rust extra\nlet α = 1;\nlet β = 2;\n```\n")
    );

    let Block::UnorderedList { items, .. } = &document.nodes[2] else {
        panic!("expected nested list");
    };
    let Block::CodeBlock {
        source: nested_body,
        span: nested_span,
        ..
    } = &items[0].content[1]
    else {
        panic!("expected nested fenced code block");
    };
    assert_eq!(nested_body, "line one\nline two\n");
    assert_eq!(
        source.get(nested_span.start..nested_span.end),
        Some("```\n  line one\n  line two\n  ```\n")
    );
}

#[test]
fn code_blocks_keep_utf8_crlf_body_and_original_range() {
    let source = "```rust\r\nα\r\nβ\r\n```\r\n";
    let document = parse_md(source);

    let Block::CodeBlock {
        source: body, span, ..
    } = &document.nodes[0]
    else {
        panic!("expected fenced code block");
    };
    assert_eq!(body, "α\r\nβ\r\n");
    assert_eq!(*span, ByteSpan::new(0, source.len()));
    assert_eq!(source.get(span.start..span.end), Some(source));
}

#[test]
fn empty_and_unclosed_fenced_nodes_keep_original_opening_spans() {
    for (source, expected_span, expected_info) in [
        ("```\n", ByteSpan::new(0, 4), None),
        ("```\n```\n", ByteSpan::new(0, 8), None),
        ("````;\n````\n", ByteSpan::new(0, 11), Some(";")),
    ] {
        let document = parse_md(source);
        let Block::CodeBlock {
            language,
            info,
            source: body,
            span,
        } = &document.nodes[0]
        else {
            panic!("expected an empty fenced code block for {source:?}");
        };
        assert_eq!(*span, expected_span);
        assert_eq!(source.get(span.start..span.end), Some(source));
        assert_eq!(body, "");
        assert_eq!(info.as_deref(), expected_info);
        assert_eq!(language.as_deref(), expected_info);
    }
}

#[test]
fn blockquote_empty_fences_do_not_absorb_sibling_source() {
    let source = "> ```\nfoo\n```\n";
    let document = parse_md(source);

    let Block::Blockquote { content, .. } = &document.nodes[0] else {
        panic!("expected a blockquote");
    };
    let Block::CodeBlock { span, .. } = &content[0] else {
        panic!("expected the blockquote's empty fenced node");
    };
    assert_eq!(*span, ByteSpan::new(2, 6));
    assert_eq!(source.get(span.start..span.end), Some("```\n"));

    let Block::CodeBlock { span, .. } = &document.nodes[2] else {
        panic!("expected the following empty fenced node");
    };
    assert_eq!(*span, ByteSpan::new(10, 14));
    assert_eq!(source.get(span.start..span.end), Some("```\n"));
}
