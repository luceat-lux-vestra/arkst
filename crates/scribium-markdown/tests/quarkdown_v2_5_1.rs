use scribium_markdown::ast::{Block, Document, Inline, ListItem};
use scribium_markdown::{parse_md, parse_qd, parse_with_mode, Mode};
use scribium_source::ByteSpan;

fn assert_valid_span(span: ByteSpan, source: &str) {
    assert!(span.start <= span.end, "inverted span {span:?}");
    assert!(span.end <= source.len(), "out-of-bounds span {span:?}");
    assert!(
        source.is_char_boundary(span.start) && source.is_char_boundary(span.end),
        "non-UTF-8-boundary span {span:?} in {source:?}"
    );
    assert!(source.get(span.start..span.end).is_some());
}

fn assert_document_spans(document: &Document, source: &str) {
    for block in &document.nodes {
        assert_block_spans(block, source);
    }
}

fn assert_block_spans(block: &Block, source: &str) {
    let span = match block {
        Block::Heading { span, .. }
        | Block::Paragraph { span, .. }
        | Block::Blockquote { span, .. }
        | Block::UnorderedList { span, .. }
        | Block::OrderedList { span, .. }
        | Block::Table { span, .. }
        | Block::CodeBlock { span, .. }
        | Block::ThematicBreak { span }
        | Block::DirectiveCall { span, .. }
        | Block::Metadata { span, .. }
        | Block::RawHtml { span, .. }
        | Block::Unsupported { span, .. } => span,
    };
    assert_valid_span(*span, source);

    match block {
        Block::Heading { content, .. } | Block::Paragraph { content, .. } => {
            for inline in content {
                assert_inline_spans(inline, source);
            }
        }
        Block::Blockquote { content, .. } => {
            for child in content {
                assert_block_spans(child, source);
            }
        }
        Block::UnorderedList { items, .. } | Block::OrderedList { items, .. } => {
            for item in items {
                assert_valid_span(item.span, source);
                for child in &item.content {
                    assert_block_spans(child, source);
                }
            }
        }
        Block::Table { header, rows, .. } => {
            for row in std::iter::once(header).chain(rows) {
                assert_valid_span(row.span, source);
                for cell in &row.cells {
                    assert_valid_span(cell.span, source);
                    for inline in &cell.content {
                        assert_inline_spans(inline, source);
                    }
                }
            }
        }
        Block::DirectiveCall {
            body,
            positional_args,
            named_args,
            ..
        } => {
            for argument in positional_args {
                if let scribium_markdown::ast::Value::Content(content) = argument {
                    for inline in content {
                        assert_inline_spans(inline, source);
                    }
                }
            }
            for (_, argument) in named_args {
                if let scribium_markdown::ast::Value::Content(content) = argument {
                    for inline in content {
                        assert_inline_spans(inline, source);
                    }
                }
            }
            if let Some(body) = body {
                for child in body {
                    assert_block_spans(child, source);
                }
            }
        }
        _ => {}
    }
}

fn assert_inline_spans(inline: &Inline, source: &str) {
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
    assert_valid_span(*span, source);

    match inline {
        Inline::Emphasis { content, .. }
        | Inline::Strong { content, .. }
        | Inline::Link { content, .. }
        | Inline::Image { content, .. }
        | Inline::Strikethrough { content, .. } => {
            for child in content {
                assert_inline_spans(child, source);
            }
        }
        Inline::DirectiveCall {
            body,
            positional_args,
            named_args,
            ..
        } => {
            for argument in positional_args {
                if let scribium_markdown::ast::Value::Content(content) = argument {
                    for child in content {
                        assert_inline_spans(child, source);
                    }
                }
            }
            for (_, argument) in named_args {
                if let scribium_markdown::ast::Value::Content(content) = argument {
                    for child in content {
                        assert_inline_spans(child, source);
                    }
                }
            }
            if let Some(body) = body {
                for child in body {
                    assert_inline_spans(child, source);
                }
            }
        }
        _ => {}
    }
}

fn parse_without_diagnostics(source: &str, mode: Mode) -> Document {
    let output = parse_with_mode(source, mode);
    assert!(
        output.diagnostics.is_empty(),
        "unexpected diagnostics: {output:?}"
    );
    assert_document_spans(&output.document, source);
    output.document
}

fn paragraph(document: &Document) -> &[Inline] {
    let Some(Block::Paragraph { content, .. }) = document.nodes.first() else {
        panic!("expected paragraph, got {:?}", document.nodes);
    };
    content
}

fn paragraph_block(document: &Document) -> &Block {
    document
        .nodes
        .first()
        .unwrap_or_else(|| panic!("expected paragraph, got {:?}", document.nodes))
}

fn text(inline: &Inline) -> &str {
    let Inline::Text { content, .. } = inline else {
        panic!("expected text, got {inline:?}");
    };
    content
}

fn text_content(inlines: &[Inline]) -> String {
    inlines.iter().map(text).collect()
}

fn link(inline: &Inline) -> (&str, ByteSpan) {
    let Inline::Link {
        destination, span, ..
    } = inline
    else {
        panic!("expected link, got {inline:?}");
    };
    (destination, *span)
}

fn link_label(inline: &Inline) -> String {
    let Inline::Link { content, .. } = inline else {
        panic!("expected link, got {inline:?}");
    };
    text_content(content)
}

fn image(inline: &Inline) -> (&str, ByteSpan) {
    let Inline::Image {
        destination, span, ..
    } = inline
    else {
        panic!("expected image, got {inline:?}");
    };
    (destination, *span)
}

fn unordered_list(block: &Block) -> &[ListItem] {
    let Block::UnorderedList { items, .. } = block else {
        panic!("expected unordered list, got {block:?}");
    };
    items
}

fn nested_unordered_list(item: &ListItem) -> &[ListItem] {
    item.content
        .iter()
        .find_map(|block| match block {
            Block::UnorderedList { items, .. } => Some(items.as_slice()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("expected nested unordered list, got {:?}", item.content))
}

fn item_text(item: &ListItem) -> String {
    let Some(Block::Paragraph { content, .. }) = item.content.first() else {
        panic!("expected list-item paragraph, got {:?}", item.content);
    };
    text_content(content)
}

fn first_link(content: &[Inline]) -> (usize, &str, ByteSpan) {
    content
        .iter()
        .enumerate()
        .find_map(|(index, inline)| {
            matches!(inline, Inline::Link { .. }).then(|| {
                let (destination, span) = link(inline);
                (index, destination, span)
            })
        })
        .unwrap_or_else(|| panic!("expected link, got {content:?}"))
}

fn assert_single_chain_depth(document: &Document, expected_depth: usize) {
    let labels = (1..=expected_depth)
        .map(|depth| format!("level {depth}"))
        .collect::<Vec<_>>();
    let labels = labels.iter().map(String::as_str).collect::<Vec<_>>();
    assert_single_chain(document, &labels);
}

fn assert_single_chain(document: &Document, expected_labels: &[&str]) {
    let mut items = unordered_list(&document.nodes[0]);
    for (depth, expected_label) in expected_labels.iter().enumerate() {
        assert_eq!(items.len(), 1, "unexpected sibling at depth {depth}");
        assert_eq!(item_text(&items[0]), *expected_label);
        if depth + 1 != expected_labels.len() {
            items = nested_unordered_list(&items[0]);
        }
    }
}

#[test]
fn qd251_links_accept_balanced_escaped_and_nested_parentheses() {
    let cases = [
        (
            "[balanced](https://example.test/a(b))",
            "https://example.test/a(b)",
        ),
        (
            r"[escaped](https://example.test/a\(b\))",
            "https://example.test/a(b)",
        ),
        (
            "[nested](https://example.test/a(b(c(d))))",
            "https://example.test/a(b(c(d)))",
        ),
    ];

    for (source, expected_destination) in cases {
        let document = parse_without_diagnostics(source, Mode::Markdown);
        let (index, destination, span) = first_link(paragraph(&document));
        assert_eq!(destination, expected_destination);
        assert_eq!(index, 0);
        assert_eq!(&source[span.start..span.end], source);
    }
}

#[test]
fn qd251_unbalanced_plain_destination_stays_literal() {
    let source = "[unbalanced](destination(a(b))";
    let document = parse_without_diagnostics(source, Mode::Markdown);
    let content = paragraph(&document);
    assert!(!content
        .iter()
        .any(|inline| matches!(inline, Inline::Link { .. })));
    let Block::Paragraph { span, .. } = paragraph_block(&document) else {
        panic!("expected paragraph")
    };
    assert_eq!(&source[span.start..span.end], source);
}

#[test]
fn qd251_trailing_parenthesis_and_surrounding_text_are_not_swallowed() {
    for source in [
        "before [target](url)) trailing",
        "before [target](url), punctuation",
    ] {
        let document = parse_without_diagnostics(source, Mode::Markdown);
        let content = paragraph(&document);
        let (index, destination, span) = first_link(content);
        assert_eq!(destination, "url");
        assert_eq!(&source[span.start..span.end], "[target](url)");
        assert_eq!(text_content(&content[index + 1..]), &source[span.end..]);
    }
}

#[test]
fn qd251_links_preserve_utf8_and_crlf_source_boundaries() {
    let utf8_source = "[한글](https://example.test/경로(문서)) 뒤";
    let utf8 = parse_without_diagnostics(utf8_source, Mode::Markdown);
    let utf8_content = paragraph(&utf8);
    let (index, destination, span) = first_link(utf8_content);
    assert_eq!(destination, "https://example.test/경로(문서)");
    assert_eq!(
        &utf8_source[span.start..span.end],
        "[한글](https://example.test/경로(문서))"
    );
    assert_eq!(text_content(&utf8_content[index + 1..]), " 뒤");

    let crlf_source = "[crlf](https://example.test/a(b)) ) tail\r\n";
    let crlf = parse_without_diagnostics(crlf_source, Mode::Markdown);
    let crlf_content = paragraph(&crlf);
    let (index, _, span) = first_link(crlf_content);
    assert_eq!(
        &crlf_source[span.start..span.end],
        "[crlf](https://example.test/a(b))"
    );
    assert_eq!(text_content(&crlf_content[index + 1..]), " ) tail");
}

#[test]
fn qd251_link_boundary_is_identical_in_md_qd_and_qd_body_modes() {
    let ordinary = "before [target](url)) after\n";
    for document in [parse_md(ordinary), parse_qd(ordinary)] {
        assert_document_spans(&document, ordinary);
        let content = paragraph(&document);
        let (index, _, _) = first_link(content);
        assert_eq!(text_content(&content[index + 1..]), ") after");
    }

    let body_source = ".note\n  before [body](url)) after\n";
    let body_document = parse_qd(body_source);
    assert_document_spans(&body_document, body_source);
    let Block::DirectiveCall {
        body: Some(body), ..
    } = &body_document.nodes[0]
    else {
        panic!("expected Quarkdown body, got {:?}", body_document.nodes);
    };
    let Block::Paragraph { content, .. } = &body[0] else {
        panic!("expected body paragraph, got {body:?}");
    };
    let (index, _, _) = first_link(content);
    assert_eq!(text_content(&content[index + 1..]), ") after");
}

#[test]
fn qd251_link_correction_empty_destinations_have_complete_spans() {
    for source in ["[empty]()", "[empty](   )"] {
        let document = parse_without_diagnostics(source, Mode::Markdown);
        let (index, destination, span) = first_link(paragraph(&document));
        assert_eq!(index, 0);
        assert_eq!(destination, "");
        assert_eq!(&source[span.start..span.end], source);
    }

    let source = "before [empty]()) after";
    let document = parse_without_diagnostics(source, Mode::Markdown);
    let content = paragraph(&document);
    let (index, destination, span) = first_link(content);
    assert_eq!(destination, "");
    assert_eq!(&source[span.start..span.end], "[empty]()");
    assert_eq!(text_content(&content[index + 1..]), ") after");
}

#[test]
fn qd251_link_correction_preserves_angle_and_title_forms() {
    let cases = [
        (
            "[angle](<https://example.test/path>) suffix",
            "https://example.test/path",
            "[angle](<https://example.test/path>)",
        ),
        (
            "[double](url \"title\") suffix",
            "url",
            "[double](url \"title\")",
        ),
        (
            "[single](url 'title') suffix",
            "url",
            "[single](url 'title')",
        ),
        ("[paren](url (title)) suffix", "url", "[paren](url (title))"),
    ];

    for (source, expected_destination, expected_link) in cases {
        let document = parse_without_diagnostics(source, Mode::Markdown);
        let (index, destination, span) = first_link(paragraph(&document));
        assert_eq!(destination, expected_destination);
        assert_eq!(&source[span.start..span.end], expected_link);
        assert_eq!(text_content(&paragraph(&document)[index + 1..]), " suffix");
    }
}

#[test]
fn qd251_link_correction_preserves_multiline_title_span() {
    let source = "[multi](url \"multi\nline\") suffix";
    let document = parse_without_diagnostics(source, Mode::Markdown);
    let (index, destination, span) = first_link(paragraph(&document));
    assert_eq!(destination, "url");
    assert_eq!(
        &source[span.start..span.end],
        "[multi](url \"multi\nline\")"
    );
    assert_eq!(text_content(&paragraph(&document)[index + 1..]), " suffix");
}

#[test]
fn qd251_link_correction_preserves_autolink_backslashes_and_email_semantics() {
    let uri_source = r"before <xx:part\+tail> after";
    let uri_document = parse_without_diagnostics(uri_source, Mode::Markdown);
    let uri_content = paragraph(&uri_document);
    let (index, destination, span) = first_link(uri_content);
    assert_eq!(destination, r"xx:part\+tail");
    assert_eq!(link_label(&uri_content[index]), r"xx:part\+tail");
    assert_eq!(&uri_source[span.start..span.end], r"<xx:part\+tail>");
    assert_eq!(text_content(&uri_content[index + 1..]), " after");

    let email_source = "before foo@bar.example.com after";
    let email_document = parse_without_diagnostics(email_source, Mode::Markdown);
    let email_content = paragraph(&email_document);
    let (index, destination, span) = first_link(email_content);
    assert_eq!(destination, "mailto:foo@bar.example.com");
    assert_eq!(link_label(&email_content[index]), "foo@bar.example.com");
    assert_eq!(&email_source[span.start..span.end], "foo@bar.example.com");
    assert_eq!(text_content(&email_content[index + 1..]), " after");
}

#[test]
fn qd251_link_correction_preserves_reference_and_image_destinations() {
    let reference_source = "[ref]: https://example.test/a\\+b\n\nbefore [reference][ref] after";
    let reference_document = parse_without_diagnostics(reference_source, Mode::Markdown);
    let reference_content = match reference_document.nodes.last() {
        Some(Block::Paragraph { content, .. }) => content,
        other => panic!("expected reference paragraph, got {other:?}"),
    };
    let (_, destination, _) = first_link(reference_content);
    assert_eq!(destination, r"https://example.test/a\+b");

    let image_source = r"![image](https://example.test/a\+b)";
    let image_document = parse_without_diagnostics(image_source, Mode::Markdown);
    let image_content = paragraph(&image_document);
    let Inline::Image { .. } = &image_content[0] else {
        panic!("expected image, got {image_content:?}");
    };
    let (destination, span) = image(&image_content[0]);
    assert_eq!(destination, r"https://example.test/a\+b");
    assert_valid_span(span, image_source);
}

#[test]
fn qd251_link_correction_preserves_utf8_and_crlf_edge_spans() {
    let utf8_source = "[빈](<https://example.test/경로>) 뒤";
    let utf8_document = parse_without_diagnostics(utf8_source, Mode::Markdown);
    let (_, _, utf8_span) = first_link(paragraph(&utf8_document));
    assert_eq!(
        &utf8_source[utf8_span.start..utf8_span.end],
        "[빈](<https://example.test/경로>)"
    );

    let crlf_source = "[crlf](url \"title\") 뒤\r\n";
    let crlf_document = parse_without_diagnostics(crlf_source, Mode::Markdown);
    let (_, _, crlf_span) = first_link(paragraph(&crlf_document));
    assert_eq!(
        &crlf_source[crlf_span.start..crlf_span.end],
        "[crlf](url \"title\")"
    );
}

#[test]
fn qd251_deep_four_space_lists_have_exact_depth_in_md_and_qd() {
    let sources = [
        ("- level 1\n    - level 2\n        - level 3\n", 3),
        (
            "- level 1\n    - level 2\n        - level 3\n            - level 4\n",
            4,
        ),
    ];

    for (source, depth) in sources {
        for mode in [Mode::Markdown, Mode::Quarkdown] {
            let document = parse_without_diagnostics(source, mode);
            assert_single_chain_depth(&document, depth);
        }
    }
}

#[test]
fn qd251_deep_list_preserves_siblings_dedent_and_following_content() {
    let source = "- root\n    - child\n        - grandchild\n    - sibling\n- parent sibling\n\nfollowing paragraph\n";
    let document = parse_without_diagnostics(source, Mode::Markdown);
    let root = unordered_list(&document.nodes[0]);
    assert_eq!(root.len(), 2);
    assert_eq!(item_text(&root[0]), "root");
    assert_eq!(item_text(&root[1]), "parent sibling");

    let nested = nested_unordered_list(&root[0]);
    assert_eq!(nested.len(), 2);
    assert_eq!(item_text(&nested[0]), "child");
    assert_eq!(item_text(&nested[1]), "sibling");
    let grandchild = nested_unordered_list(&nested[0]);
    assert_eq!(grandchild.len(), 1);
    assert_eq!(item_text(&grandchild[0]), "grandchild");

    assert!(matches!(
        document.nodes[1],
        Block::Paragraph { ref content, .. } if text_content(content) == "following paragraph"
    ));
}

#[test]
fn qd251_nested_paragraph_and_list_content_remain_in_their_items() {
    let source = "- root\n\n    child paragraph\n    - nested\n        - grandchild\n\nafter\n";
    let document = parse_without_diagnostics(source, Mode::Markdown);
    let root = unordered_list(&document.nodes[0]);
    assert_eq!(root.len(), 1);
    assert!(root[0].content.iter().any(|block| {
        matches!(block, Block::Paragraph { content, .. } if text_content(content) == "child paragraph")
    }));
    let nested = nested_unordered_list(&root[0]);
    assert_eq!(nested.len(), 1);
    assert_eq!(item_text(&nested[0]), "nested");
    assert_eq!(nested_unordered_list(&nested[0]).len(), 1);
    assert_eq!(
        item_text(&nested_unordered_list(&nested[0])[0]),
        "grandchild"
    );
    assert!(matches!(
        document.nodes[1],
        Block::Paragraph { ref content, .. } if text_content(content) == "after"
    ));
}

#[test]
fn qd251_deep_lists_preserve_utf8_and_crlf_spans() {
    for source in [
        "- 첫\n    - 둘\n        - 셋\n",
        "- 첫\r\n    - 둘\r\n        - 셋\r\n",
    ] {
        let document = parse_without_diagnostics(source, Mode::Markdown);
        assert_single_chain(&document, &["첫", "둘", "셋"]);
    }
}

#[test]
fn qd251_qd_body_uses_dynamic_indent_before_markdown_list_parsing() {
    let source = ".panel\n    - one\n        - two\n            - three\n\noutside\n";
    let document = parse_without_diagnostics(source, Mode::Quarkdown);
    let Block::DirectiveCall {
        body: Some(body), ..
    } = &document.nodes[0]
    else {
        panic!("expected directive body, got {:?}", document.nodes);
    };
    let body_document = Document {
        nodes: body.clone(),
        front_matter: None,
        line_count: 0,
    };
    assert_single_chain(&body_document, &["one", "two", "three"]);
    assert!(matches!(
        document.nodes[1],
        Block::Paragraph { ref content, .. } if text_content(content) == "outside"
    ));
}
