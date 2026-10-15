use arkst_core::evaluator::Evaluator;
use arkst_core::ir::{IrColor, IrDocument, IrInline, IrNamedArg, IrNode, IrValue};
use arkst_core::{compile, CompileOptions, SourceId, SourceSpan, VirtualProjectBuilder};

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
    let parsed: IrDocument = serde_json::from_str(&wire).expect("deserialize");
    assert_eq!(result.ir, parsed);
    content.clone()
}

#[test]
fn explicit_none_preserves_original_inline_children() {
    let content = single_paragraph(".text text:{Hello} color:{.none}\n");
    assert!(matches!(
        content.as_slice(),
        [IrInline::Text { content, .. }] if content == "Hello"
    ));
}

#[test]
fn dynamic_and_typed_colors_preserve_rgba_semantics() {
    let named = single_paragraph(".text text:{Hello} color:{purple}\n");
    assert!(matches!(
        named.as_slice(),
        [IrInline::TextColor {
            color: IrColor {
                red: 128,
                green: 0,
                blue: 128,
                alpha,
            },
            ..
        }] if *alpha == 1.0
    ));

    let span = SourceSpan::new(SourceId(7), 0, 20);
    let color = IrColor {
        red: 1,
        green: 2,
        blue: 3,
        alpha: 0.5,
    };
    let document = IrDocument {
        nodes: vec![IrNode::FunctionCall {
            name: "text".to_string(),
            positional_args: Vec::new(),
            named_args: vec![IrNamedArg {
                name: "color".to_string(),
                name_span: span,
                value: IrValue::Color(color.clone()),
                span,
            }],
            ordered_args: None,
            lambda_parameters: None,
            body: Some(vec![IrNode::Paragraph {
                content: vec![IrInline::Text {
                    content: "Typed".to_string(),
                    span,
                }],
                span,
            }]),
            raw_body: None,
            span,
        }],
        metadata: Default::default(),
    };
    let (evaluated, diagnostics) = Evaluator::new().evaluate(&document);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(matches!(
        evaluated.nodes.as_slice(),
        [IrNode::Paragraph { content, .. }]
            if matches!(content.as_slice(), [IrInline::TextColor {
                color: actual,
                content,
                ..
            }] if actual == &color
                && matches!(content.as_slice(), [IrInline::Text { content, .. }] if content == "Typed"))
    ));
}

#[test]
fn color_preserves_rich_children_call_provenance_and_transform_order() {
    let source = ".text color:{rgba(1,2,3,0.5)} script:{sup} case:{uppercase} size:{small} decoration:{underline}\n    Hello *world* and `code`\n";
    let (result, source_id) = compile_source_with_id(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let [IrNode::Paragraph { content, .. }] = result.ir.nodes.as_slice() else {
        panic!("expected one paragraph: {result:?}");
    };
    let [IrInline::TextColor {
        color,
        content: colored,
        span,
    }] = content.as_slice()
    else {
        panic!("expected typed color wrapper: {content:?}");
    };
    assert_eq!(
        color,
        &IrColor {
            red: 1,
            green: 2,
            blue: 3,
            alpha: 0.5,
        }
    );
    assert_eq!(span.source_id, source_id);
    assert_eq!(&source[span.start..span.end], source.trim_end());
    assert!(matches!(
        colored.as_slice(),
        [IrInline::TextScript { content, .. }]
            if matches!(content.as_slice(), [IrInline::TextCase { content, .. }]
                if matches!(content.as_slice(), [IrInline::TextSize { content, .. }]
                    if matches!(content.as_slice(), [IrInline::Underline { content, .. }]
                        if content.iter().any(|inline| matches!(inline, IrInline::Emphasis { .. }))
                            && content.iter().any(|inline| matches!(inline, IrInline::Code { .. })))))
    ));
}

#[test]
fn color_composes_inside_url_and_blank_url_uses_authored_plaintext() {
    let linked = single_paragraph(
        ".text text:{Hello} url:{https://example.org} color:{#369} script:{sub}\n",
    );
    assert!(matches!(
        linked.as_slice(),
        [IrInline::Link { destination, content, .. }]
            if destination == "https://example.org"
                && matches!(content.as_slice(), [IrInline::TextColor { content, .. }]
                    if matches!(content.as_slice(), [IrInline::TextScript { .. }]))
    ));

    let derived = single_paragraph(".text url:{   } color:{red}\n    Hello *world*\n");
    assert!(matches!(
        derived.as_slice(),
        [IrInline::Link { destination, content, .. }]
            if destination == "Hello world"
                && matches!(content.as_slice(), [IrInline::TextColor { .. }])
    ));
}

#[test]
fn invalid_static_duplicate_positional_and_classname_inputs_fail_atomically() {
    for source in [
        ".text text:{Hello} color:{not-a-color}\n",
        ".text text:{Hello} color:{.string {red}}\n",
        ".text text:{Hello} color:{red} color:{blue}\n",
        ".text {Hello} {https://example.org} {red}\n",
        ".text text:{Hello} classname:{accent}\n",
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
