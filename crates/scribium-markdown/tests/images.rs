use scribium_markdown::ast::{Block, Inline};
use scribium_markdown::parse_md;

fn paragraph(source: &str) -> Vec<Inline> {
    let document = parse_md(source);
    let Block::Paragraph { content, .. } = document.nodes.first().expect("paragraph") else {
        panic!("expected paragraph, got {document:?}")
    };
    content.clone()
}

#[test]
fn inline_images_preserve_destination_title_nested_alt_and_spans() {
    let source = "before ![*formatted* alt](<assets/my image.png> \"Image title\") after\n";
    let content = paragraph(source);
    assert!(matches!(content[0], Inline::Text { .. }));
    assert!(matches!(content[2], Inline::Text { .. }));

    let Inline::Image {
        content: alt,
        destination,
        title,
        span,
    } = &content[1]
    else {
        panic!("expected image, got {content:?}")
    };
    assert_eq!(destination, "assets/my image.png");
    assert_eq!(title.as_deref(), Some("Image title"));
    assert!(matches!(
        alt.as_slice(),
        [Inline::Emphasis { .. }, Inline::Text { .. }]
    ));
    assert_eq!(
        &source[span.start..span.end],
        "![*formatted* alt](<assets/my image.png> \"Image title\")"
    );

    let Inline::Emphasis { span: alt_span, .. } = &alt[0] else {
        panic!("expected formatted alt")
    };
    assert!(source[alt_span.start..alt_span.end].starts_with("*formatted"));
    assert!(alt_span.start < alt_span.end);
    assert!(span.end <= source.len());
}

#[test]
fn empty_alt_and_image_inside_link_are_preserved() {
    let content = paragraph("[![alt](image.png)](page.md) and ![](decorative.svg)\n");
    let Inline::Link {
        content: link_content,
        destination,
        ..
    } = &content[0]
    else {
        panic!("expected link, got {content:?}")
    };
    assert_eq!(destination, "page.md");
    assert!(matches!(link_content.as_slice(), [Inline::Image { .. }]));
    assert!(matches!(
        &content[2],
        Inline::Image { content, destination, .. }
            if content.is_empty() && destination == "decorative.svg"
    ));
}

#[test]
fn reference_images_use_the_shared_reference_destination_and_title() {
    let document = parse_md("![logo][brand]\n\n[brand]: assets/logo.png \"Logo\"\n");
    let Block::Paragraph { content, .. } = &document.nodes[0] else {
        panic!("expected paragraph")
    };
    assert!(matches!(
        content.as_slice(),
        [Inline::Image {
            content,
            destination,
            title: Some(title),
            span,
        }] if content.iter().all(|inline| matches!(inline, Inline::Text { .. }))
            && destination == "assets/logo.png"
            && title == "Logo"
            && span.start == 0
            && span.end == "![logo][brand]".len()
    ));
}

#[test]
fn malformed_image_like_text_does_not_fail_open_into_an_image() {
    for source in ["![alt](", "![alt](foo", "![](foo", "![alt]"] {
        let document = parse_md(source);
        assert!(
            document.nodes.iter().all(|block| match block {
                Block::Paragraph { content, .. } => {
                    !content
                        .iter()
                        .any(|inline| matches!(inline, Inline::Image { .. }))
                }
                _ => true,
            }),
            "source {source:?} produced {document:?}"
        );
    }
}

#[test]
fn image_and_surrounding_text_ranges_are_disjoint_source_ranges() {
    let source = "before ![alt](img.png) after";
    let content = paragraph(source);
    let spans: Vec<_> = content
        .iter()
        .map(|inline| match inline {
            Inline::Text { span, .. }
            | Inline::Image { span, .. }
            | Inline::Emphasis { span, .. }
            | Inline::Strong { span, .. }
            | Inline::DirectiveCall { span, .. }
            | Inline::Link { span, .. }
            | Inline::Code { span, .. }
            | Inline::RawHtml { span, .. }
            | Inline::Strikethrough { span, .. }
            | Inline::HardBreak { span }
            | Inline::SoftBreak { span }
            | Inline::Unsupported { span, .. } => *span,
        })
        .collect();
    assert_eq!(spans.len(), 3);
    assert!(spans
        .windows(2)
        .all(|window| window[0].end <= window[1].start));
    assert_eq!(&source[spans[1].start..spans[1].end], "![alt](img.png)");

    let Inline::Image { content: alt, .. } = &content[1] else {
        panic!("expected image")
    };
    let alt_span = match &alt[0] {
        Inline::Text { span, .. } => *span,
        other => panic!("expected text alt, got {other:?}"),
    };
    assert_eq!(&source[alt_span.start..alt_span.end], "alt");
}
