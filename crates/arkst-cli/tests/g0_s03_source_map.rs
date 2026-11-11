use arkst_core::ir::{IrInline, IrNode};
use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};

fn assert_s03_source_map(source: &str) {
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("valid entry")
        .add_source("main.qd", source)
        .expect("valid source")
        .build()
        .expect("valid project");
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
        panic!("expected strong wrapper: {content:?}");
    };
    let [
        IrInline::Text {
            content: prefix, ..
        },
        IrInline::Link {
            content: link_content,
            destination,
            span: link_span,
            ..
        },
        IrInline::Text {
            content: suffix, ..
        },
    ] = strong_content.as_slice()
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
    assert_eq!(
        &source[strong_span.start..strong_span.end],
        "**앞 [.uppercase {world}](https://example.com) 뒤**"
    );
    assert_eq!(
        &source[link_span.start..link_span.end],
        "[.uppercase {world}](https://example.com)"
    );

    let (typst, source_map) = arkst_typst::lower_to_typst(&result.ir);
    assert!(
        typst.contains("*앞 #link(\"https://example.com\")[WORLD] 뒤*"),
        "{typst:?}"
    );
    assert!(
        source_map.iter().any(|entry| entry.original == *strong_span),
        "{source_map:?}"
    );
    assert!(
        source_map.iter().any(|entry| entry.original == *link_span),
        "{source_map:?}"
    );
    assert!(source_map.iter().all(|entry| {
        entry.generated_start < entry.generated_end && entry.generated_end <= typst.len()
    }));
}

#[test]
fn g0_s03_normal_compile_to_typst_source_map_preserves_tight_and_indented_crlf_spans() {
    assert_s03_source_map(".text {**앞 [.uppercase {world}](https://example.com) 뒤**}\r\n");
    assert_s03_source_map(".text\r\n    **앞 [.uppercase {world}](https://example.com) 뒤**\r\n");
}
