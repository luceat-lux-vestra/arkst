use arkst_core::ir::{IrInline, IrNode};
use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};

#[test]
fn indented_crlf_strong_span_reaches_final_ir_and_typst_source_map() {
    let source = ".text\r\n    **앞 [.uppercase {world}](https://example.com) 뒤**\r\n";
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("entry")
        .add_source("main.qd", source)
        .expect("source")
        .build()
        .expect("project");
    let result = compile(&project, &CompileOptions::default());

    assert!(result.diagnostics.is_empty(), "{result:?}");
    let [IrNode::Paragraph { content, .. }] = result.ir.nodes.as_slice() else {
        panic!("expected one final paragraph: {result:?}");
    };
    let [IrInline::Strong {
        content: strong_content,
        span: strong_span,
    }] = content.as_slice()
    else {
        panic!("expected one strong wrapper: {content:?}");
    };

    assert_eq!(strong_span.start, 11);
    assert_eq!(strong_span.end, 64);
    assert_eq!(
        &source[strong_span.start..strong_span.end],
        "**앞 [.uppercase {world}](https://example.com) 뒤**"
    );
    assert!(source.is_char_boundary(strong_span.start));
    assert!(source.is_char_boundary(strong_span.end));
    assert_eq!(&source[64..66], "\r\n");

    let [IrInline::Text {
        content: prefix, ..
    }, IrInline::Link {
        content: link_content,
        destination,
        ..
    }, IrInline::Text {
        content: suffix, ..
    }] = strong_content.as_slice()
    else {
        panic!("expected text/link/text ordering: {strong_content:?}");
    };
    assert_eq!(prefix, "앞 ");
    assert_eq!(destination, "https://example.com");
    assert!(matches!(
        link_content.as_slice(),
        [IrInline::Text { content, .. }] if content == "WORLD"
    ));
    assert_eq!(suffix, " 뒤");

    let (typst, source_map) = arkst_typst::lower_to_typst(&result.ir);
    assert!(
        typst.contains("*앞 #link(\"https://example.com\")[WORLD] 뒤*"),
        "{typst:?}"
    );
    assert!(
        source_map
            .iter()
            .any(|entry| entry.original == *strong_span),
        "{source_map:?}"
    );
}
