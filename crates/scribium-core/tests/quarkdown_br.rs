use scribium_core::evaluator::Evaluator;
use scribium_core::ir::{IrInline, IrNode, IrParameter, IrValue};
use scribium_core::{compile, CompileOptions, SourceId, SourceSpan, VirtualProjectBuilder};

fn compile_source(source: &str) -> (scribium_core::CompileResult, SourceId) {
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("valid entry")
        .add_source("main.qd", source)
        .expect("valid source")
        .build()
        .expect("valid project");
    let source_id = project
        .sources()
        .get_id(project.entry())
        .expect("entry source id");
    (compile(&project, &CompileOptions::default()), source_id)
}

fn paragraph(result: &scribium_core::CompileResult) -> &[IrInline] {
    let [IrNode::Paragraph { content, .. }] = result.ir.nodes.as_slice() else {
        panic!("expected one paragraph, got {:?}", result.ir.nodes);
    };
    content
}

fn paragraph_text(node: &IrNode) -> String {
    let IrNode::Paragraph { content, .. } = node else {
        panic!("expected paragraph, got {node:?}");
    };
    content
        .iter()
        .map(|inline| match inline {
            IrInline::Text { content, .. } => content.as_str(),
            other => panic!("expected text, got {other:?}"),
        })
        .collect()
}

#[test]
fn inline_br_produces_one_hard_break_in_source_order_with_call_span() {
    let source = "before .br after\n";
    let (result, source_id) = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");

    let content = paragraph(&result);
    assert_eq!(content.len(), 3, "{content:?}");
    assert!(matches!(
        &content[0],
        IrInline::Text { content, .. } if content == "before "
    ));
    let IrInline::HardBreak { span } = &content[1] else {
        panic!("expected one hard break, got {:?}", content[1]);
    };
    let call_start = source.find(".br").expect("call span");
    assert_eq!(span.source_id, source_id);
    assert_eq!(span.start, call_start);
    assert_eq!(span.end, call_start + ".br".len());
    assert!(matches!(
        &content[2],
        IrInline::Text { content, .. } if content == " after"
    ));
}

#[test]
fn standalone_br_uses_the_existing_inline_materialization_boundary() {
    let (result, _) = compile_source(".br\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert!(matches!(paragraph(&result), [IrInline::HardBreak { .. }]));
}

#[test]
fn br_plaintext_omits_the_line_break_like_upstream_to_plain_text() {
    let (result, _) = compile_source(".plaintext\n    before .br after\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(result.ir.nodes.len(), 1);
    assert_eq!(paragraph_text(&result.ir.nodes[0]), "before  after");
}

#[test]
fn br_rejects_positional_named_and_multiple_arguments_atomically() {
    for source in [".br {value}\n", ".br named:{value}\n", ".br {one} {two}\n"] {
        let (result, _) = compile_source(source);
        assert_eq!(result.diagnostics.len(), 1, "{source:?}: {result:?}");
        assert_eq!(result.diagnostics[0].code, "E3003");
        assert!(result.diagnostics[0].primary.is_some(), "{result:?}");
        assert!(result.ir.nodes.is_empty(), "{source:?}: {result:?}");
    }
}

#[test]
fn br_signature_validation_does_not_evaluate_invalid_arguments_or_body() {
    let (argument, _) = compile_source(".br {.grid columns:{0}}\n");
    assert_eq!(argument.diagnostics.len(), 1, "{argument:?}");
    assert!(argument.diagnostics[0].message.contains("positional"));
    assert!(!argument.diagnostics[0].message.contains("Column count"));
    assert!(argument.ir.nodes.is_empty(), "{argument:?}");

    let (body, _) = compile_source(".br\n    .grid columns:{0}\n        body\n");
    assert_eq!(body.diagnostics.len(), 1, "{body:?}");
    assert!(body.diagnostics[0].message.contains("body"));
    assert!(!body.diagnostics[0].message.contains("Column count"));
    assert!(body.ir.nodes.is_empty(), "{body:?}");
}

#[test]
fn br_rejects_inline_body_before_evaluating_its_contents() {
    let span = SourceSpan::new(SourceId(7), 10, 20);
    let document = scribium_core::ir::IrDocument {
        nodes: vec![IrNode::Paragraph {
            content: vec![IrInline::DirectiveCall {
                name: "br".to_string(),
                positional_args: Vec::new(),
                named_args: Vec::new(),
                ordered_args: None,
                body: Some(vec![IrInline::DirectiveCall {
                    name: "not".to_string(),
                    positional_args: vec![IrValue::Number(1.0)],
                    named_args: Vec::new(),
                    ordered_args: None,
                    body: None,
                    span,
                }]),
                span,
            }],
            span,
        }],
        metadata: scribium_core::ir::IrMetadata::default(),
    };

    let (evaluated, diagnostics) = Evaluator::new().evaluate(&document);
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert!(diagnostics[0].message.contains("body"));
    assert!(!diagnostics[0].message.contains("boolean"));
    assert!(evaluated.nodes.iter().all(|node| {
        !matches!(
            node,
            IrNode::Paragraph { content, .. }
                if content.iter().any(|inline| matches!(inline, IrInline::HardBreak { .. }))
        )
    }));
}

#[test]
fn br_rejects_lambda_body_before_evaluating_its_contents() {
    let span = SourceSpan::new(SourceId(7), 10, 20);
    let document = scribium_core::ir::IrDocument {
        nodes: vec![IrNode::FunctionCall {
            name: "br".to_string(),
            positional_args: Vec::new(),
            named_args: Vec::new(),
            ordered_args: None,
            lambda_parameters: Some(vec![IrParameter {
                name: "value".to_string(),
                name_span: span,
                span,
                optional: false,
            }]),
            body: Some(vec![IrNode::FunctionCall {
                name: "grid".to_string(),
                positional_args: Vec::new(),
                named_args: vec![scribium_core::ir::IrNamedArg {
                    name: "columns".to_string(),
                    name_span: span,
                    value: IrValue::Number(0.0),
                    span,
                }],
                ordered_args: None,
                lambda_parameters: None,
                body: None,
                span,
            }]),
            span,
        }],
        metadata: scribium_core::ir::IrMetadata::default(),
    };

    let (evaluated, diagnostics) = Evaluator::new().evaluate(&document);
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert!(diagnostics[0].message.contains("lambda"));
    assert!(!diagnostics[0].message.contains("Column count"));
    assert!(evaluated.nodes.is_empty(), "{evaluated:?}");
}

#[test]
fn invalid_br_invocation_does_not_publish_a_partial_hard_break() {
    let (result, _) = compile_source("before .br {value} after\n");
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    let content = paragraph(&result);
    assert!(content
        .iter()
        .all(|inline| !matches!(inline, IrInline::HardBreak { .. })));
    assert_eq!(paragraph_text(&result.ir.nodes[0]), "before  after");
}

#[test]
fn source_defined_br_keeps_precedence_in_block_and_inline_contexts() {
    let (result, _) = compile_source(".function {br}\n    custom\n\n.br\n\nbefore .br after\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(result.ir.nodes.len(), 2, "{result:?}");
    assert_eq!(paragraph_text(&result.ir.nodes[0]), "custom");
    assert_eq!(paragraph_text(&result.ir.nodes[1]), "before custom after");
}

#[test]
fn existing_hard_break_serde_path_remains_unchanged() {
    let value = IrInline::HardBreak {
        span: SourceSpan::new(SourceId(3), 4, 8),
    };
    let encoded = serde_json::to_value(&value).expect("hard break serializes");
    let decoded: IrInline = serde_json::from_value(encoded).expect("hard break deserializes");
    assert_eq!(decoded, value);
}
