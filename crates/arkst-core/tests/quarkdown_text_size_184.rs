use arkst_core::ir::{IrInline, IrNode, IrTextSize};
use arkst_core::{compile, CompileOptions, SourceId, VirtualProjectBuilder};

fn compile_source(source: &str) -> arkst_core::CompileResult {
    compile_source_with_id(source).0
}

fn compile_source_with_id(source: &str) -> (arkst_core::CompileResult, SourceId) {
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("entry")
        .add_source("main.qd", source)
        .expect("source")
        .build()
        .expect("project");
    let source_id = project
        .sources()
        .get_id(project.entry())
        .expect("source id");
    (compile(&project, &CompileOptions::default()), source_id)
}

fn single_paragraph(source: &str) -> Vec<IrInline> {
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{source:?}: {result:?}");
    let [IrNode::Paragraph { content, .. }] = result.ir.nodes.as_slice() else {
        panic!("expected one paragraph: {result:?}");
    };
    let wire = serde_json::to_string(&result.ir).expect("serialize");
    let parsed: arkst_core::ir::IrDocument = serde_json::from_str(&wire).expect("deserialize");
    assert_eq!(result.ir, parsed);
    content.clone()
}

#[test]
fn normal_size_and_explicit_none_preserve_original_inline_children() {
    for source in [
        ".text text:{Hello} size:{normal}\n",
        ".text text:{Hello} size:{.none}\n",
    ] {
        let content = single_paragraph(source);
        assert!(
            matches!(content.as_slice(), [IrInline::Text { content, .. }] if content == "Hello")
        );
    }
}

#[test]
fn complete_non_normal_size_domain_preserves_typed_ratios() {
    let cases = [
        ("tiny", IrTextSize::Tiny),
        ("small", IrTextSize::Small),
        ("medium", IrTextSize::Medium),
        ("large", IrTextSize::Large),
        ("larger", IrTextSize::Larger),
        ("huge", IrTextSize::Huge),
    ];

    for (name, expected) in cases {
        let source = format!(".text text:{{Hello}} size:{{{name}}}\n");
        let content = single_paragraph(&source);
        assert!(matches!(
            content.as_slice(),
            [IrInline::TextSize { size, content, .. }]
                if *size == expected
                    && matches!(content.as_slice(), [IrInline::Text { content, .. }] if content == "Hello")
        ));
    }
}

#[test]
fn size_preserves_rich_children_call_provenance_and_existing_transform_order() {
    let source = ".text size:{large} weight:{bold} style:{italic} decoration:{underline}\n    Hello `code`\n";
    let (result, source_id) = compile_source_with_id(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let [IrNode::Paragraph { content, .. }] = result.ir.nodes.as_slice() else {
        panic!("expected one paragraph: {result:?}");
    };
    let [IrInline::TextSize {
        size: IrTextSize::Large,
        content: sized,
        span,
    }] = content.as_slice()
    else {
        panic!("expected typed size wrapper: {content:?}");
    };
    assert_eq!(span.source_id, source_id);
    assert_eq!(&source[span.start..span.end], source.trim_end());
    assert!(
        matches!(sized.as_slice(), [IrInline::Underline { content, .. }]
        if matches!(content.as_slice(), [IrInline::Emphasis { content, .. }]
            if matches!(content.as_slice(), [IrInline::Strong { content, .. }]
                if content.iter().any(|inline| matches!(inline, IrInline::Code { .. })))))
    );
}

#[test]
fn size_composes_inside_existing_url_and_blank_url_plaintext_projection() {
    let linked = single_paragraph(
        ".text text:{Hello} url:{https://example.org} size:{larger} decoration:{overline}\n",
    );
    assert!(matches!(
        linked.as_slice(),
        [IrInline::Link { destination, content, .. }]
            if destination == "https://example.org"
                && matches!(content.as_slice(), [IrInline::TextSize {
                    size: IrTextSize::Larger,
                    content,
                    ..
                }] if matches!(content.as_slice(), [IrInline::Overline { .. }]))
    ));

    let derived = single_paragraph(".text url:{   } size:{tiny}\n    Hello *world*\n");
    assert!(matches!(
        derived.as_slice(),
        [IrInline::Link { destination, content, .. }]
            if destination == "Hello world"
                && matches!(content.as_slice(), [IrInline::TextSize {
                    size: IrTextSize::Tiny,
                    ..
                }])
    ));
}

#[test]
fn invalid_size_values_and_named_only_boundary_fail_atomically() {
    for source in [
        ".text text:{Hello} size:{123}\n",
        ".text text:{Hello} size:{giant}\n",
        ".text text:{Hello} size:{small} size:{large}\n",
        ".text {Hello} {https://example.org} {small}\n",
    ] {
        let result = compile_source(source);
        assert!(!result.diagnostics.is_empty(), "{source:?}: {result:?}");
        assert!(
            result.diagnostics[0].primary.is_some(),
            "{source:?}: {result:?}"
        );
        assert!(result.ir.nodes.is_empty(), "{source:?}: {result:?}");
    }
}
