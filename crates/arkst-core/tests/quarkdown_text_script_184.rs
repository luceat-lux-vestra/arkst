use arkst_core::ir::{IrInline, IrNode, IrTextScript};
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
fn explicit_none_preserves_original_inline_children() {
    let content = single_paragraph(".text text:{Hello} script:{.none}\n");
    assert!(matches!(
        content.as_slice(),
        [IrInline::Text { content, .. }] if content == "Hello"
    ));
}

#[test]
fn complete_script_domain_preserves_typed_semantics() {
    for (name, expected) in [
        ("sub", IrTextScript::Subscript),
        ("sup", IrTextScript::Superscript),
    ] {
        let source = format!(".text text:{{Hello}} script:{{{name}}}\n");
        let content = single_paragraph(&source);
        assert!(matches!(
            content.as_slice(),
            [IrInline::TextScript { script, content, .. }]
                if *script == expected
                    && matches!(content.as_slice(), [IrInline::Text { content, .. }] if content == "Hello")
        ));
    }
}

#[test]
fn script_preserves_rich_children_call_provenance_and_existing_transforms() {
    let source = ".text script:{sup} size:{small} weight:{bold} decoration:{underline}\n    Hello *world* and `code`\n";
    let (result, source_id) = compile_source_with_id(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let [IrNode::Paragraph { content, .. }] = result.ir.nodes.as_slice() else {
        panic!("expected one paragraph: {result:?}");
    };
    let [IrInline::TextScript {
        script: IrTextScript::Superscript,
        content: scripted,
        span,
    }] = content.as_slice()
    else {
        panic!("expected typed script wrapper: {content:?}");
    };
    assert_eq!(span.source_id, source_id);
    assert_eq!(&source[span.start..span.end], source.trim_end());
    let [IrInline::TextSize { content: sized, .. }] = scripted.as_slice() else {
        panic!("expected size inside script: {scripted:?}");
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
fn script_composes_inside_url_and_blank_url_plaintext_projection() {
    let linked = single_paragraph(
        ".text text:{Hello} url:{https://example.org} script:{sub} decoration:{overline}\n",
    );
    assert!(matches!(
        linked.as_slice(),
        [IrInline::Link { destination, content, .. }]
            if destination == "https://example.org"
                && matches!(content.as_slice(), [IrInline::TextScript {
                    script: IrTextScript::Subscript,
                    content,
                    ..
                }] if matches!(content.as_slice(), [IrInline::Overline { .. }]))
    ));

    let derived = single_paragraph(".text url:{   } script:{sup}\n    Hello *world*\n");
    assert!(matches!(
        derived.as_slice(),
        [IrInline::Link { destination, content, .. }]
            if destination == "Hello world"
                && matches!(content.as_slice(), [IrInline::TextScript {
                    script: IrTextScript::Superscript,
                    ..
                }])
    ));
}

#[test]
fn invalid_script_values_and_named_only_boundary_fail_atomically() {
    for source in [
        ".text text:{Hello} script:{123}\n",
        ".text text:{Hello} script:{baseline}\n",
        ".text text:{Hello} script:{sub} script:{sup}\n",
        ".text {Hello} {https://example.org} {sub}\n",
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
