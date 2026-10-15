use arkst_core::ir::{IrInline, IrNode, IrTextVariant};
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
fn normal_and_none_preserve_original_inline_children() {
    for variant in ["normal", ".none"] {
        let source = format!(".text text:{{Hello}} variant:{{{variant}}}\n");
        let content = single_paragraph(&source);
        assert!(matches!(
            content.as_slice(),
            [IrInline::Text { content, .. }] if content == "Hello"
        ));
    }
}

#[test]
fn smallcaps_preserves_typed_semantics_and_wire_serde() {
    let content = single_paragraph(".text text:{Hello} variant:{smallcaps}\n");
    assert!(matches!(
        content.as_slice(),
        [IrInline::TextVariant {
            variant: IrTextVariant::SmallCaps,
            content,
            ..
        }] if matches!(content.as_slice(), [IrInline::Text { content, .. }] if content == "Hello")
    ));
}

#[test]
fn smallcaps_preserves_rich_children_call_provenance_and_existing_transforms() {
    let source = ".text variant:{smallcaps} script:{sup} size:{small} weight:{bold} decoration:{underline}\n    Hello *world* and `code`\n";
    let (result, source_id) = compile_source_with_id(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let [IrNode::Paragraph { content, .. }] = result.ir.nodes.as_slice() else {
        panic!("expected one paragraph: {result:?}");
    };
    let [IrInline::TextScript {
        content: scripted, ..
    }] = content.as_slice()
    else {
        panic!("expected script wrapper: {content:?}");
    };
    let [IrInline::TextVariant {
        variant: IrTextVariant::SmallCaps,
        content: smallcaps,
        span,
    }] = scripted.as_slice()
    else {
        panic!("expected typed variant wrapper: {scripted:?}");
    };
    assert_eq!(span.source_id, source_id);
    assert_eq!(&source[span.start..span.end], source.trim_end());
    let [IrInline::TextSize { content: sized, .. }] = smallcaps.as_slice() else {
        panic!("expected size inside variant: {smallcaps:?}");
    };
    let [IrInline::Underline {
        content: underlined,
        ..
    }] = sized.as_slice()
    else {
        panic!("expected underline inside size: {sized:?}");
    };
    let [IrInline::Strong { content: rich, .. }] = underlined.as_slice() else {
        panic!("expected strong inside underline: {underlined:?}");
    };
    assert!(rich
        .iter()
        .any(|inline| matches!(inline, IrInline::Emphasis { .. })));
    assert!(rich
        .iter()
        .any(|inline| matches!(inline, IrInline::Code { .. })));
}

#[test]
fn smallcaps_composes_inside_url_and_blank_url_plaintext_projection() {
    let linked = single_paragraph(
        ".text text:{Hello} url:{https://example.org} variant:{smallcaps} decoration:{overline}\n",
    );
    assert!(matches!(
        linked.as_slice(),
        [IrInline::Link { destination, content, .. }]
            if destination == "https://example.org"
                && matches!(content.as_slice(), [IrInline::TextVariant {
                    variant: IrTextVariant::SmallCaps,
                    content,
                    ..
                }] if matches!(content.as_slice(), [IrInline::Overline { .. }]))
    ));

    let derived = single_paragraph(".text url:{   } variant:{smallcaps}\n    Hello *world*\n");
    assert!(matches!(
        derived.as_slice(),
        [IrInline::Link { destination, content, .. }]
            if destination == "Hello world"
                && matches!(content.as_slice(), [IrInline::TextVariant {
                    variant: IrTextVariant::SmallCaps,
                    ..
                }])
    ));
}

#[test]
fn invalid_variant_values_and_named_only_boundary_fail_atomically() {
    for source in [
        ".text text:{Hello} variant:{123}\n",
        ".text text:{Hello} variant:{petitecaps}\n",
        ".text text:{Hello} variant:{normal} variant:{smallcaps}\n",
        ".text {Hello} {https://example.org} {smallcaps}\n",
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
