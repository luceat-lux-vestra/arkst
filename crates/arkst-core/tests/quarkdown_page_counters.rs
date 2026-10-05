use arkst_core::evaluator::Evaluator;
use arkst_core::ir::{IrInline, IrNode, IrPageCounterTarget, IrParameter, IrValue};
use arkst_core::{compile, CompileOptions, SourceId, SourceSpan, VirtualProjectBuilder};

fn compile_source(source: &str) -> (arkst_core::CompileResult, SourceId) {
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

fn paragraph(result: &arkst_core::CompileResult) -> &[IrInline] {
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
fn page_counter_pair_materializes_typed_inline_targets_in_source_order() {
    let source = "Page .currentpage of .totalpages.\n";
    let (result, source_id) = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");

    let content = paragraph(&result);
    assert_eq!(content.len(), 5, "{content:?}");
    assert!(matches!(
        &content[0],
        IrInline::Text { content, .. } if content == "Page "
    ));
    let IrInline::PageCounter {
        target: IrPageCounterTarget::Current,
        span: current_span,
    } = &content[1]
    else {
        panic!("expected current-page counter, got {:?}", content[1]);
    };
    assert!(matches!(
        &content[2],
        IrInline::Text { content, .. } if content == " of "
    ));
    let IrInline::PageCounter {
        target: IrPageCounterTarget::Total,
        span: total_span,
    } = &content[3]
    else {
        panic!("expected total-page counter, got {:?}", content[3]);
    };
    assert!(matches!(
        &content[4],
        IrInline::Text { content, .. } if content == "."
    ));

    let current_start = source.find(".currentpage").expect("current call");
    assert_eq!(current_span.source_id, source_id);
    assert_eq!(current_span.start, current_start);
    assert_eq!(current_span.end, current_start + ".currentpage".len());

    let total_start = source.find(".totalpages").expect("total call");
    assert_eq!(total_span.source_id, source_id);
    assert_eq!(total_span.start, total_start);
    assert_eq!(total_span.end, total_start + ".totalpages".len());
}

#[test]
fn standalone_page_counters_use_the_inline_materialization_boundary() {
    for (source, expected) in [
        (".currentpage\n", IrPageCounterTarget::Current),
        (".totalpages\n", IrPageCounterTarget::Total),
    ] {
        let (result, _) = compile_source(source);
        assert!(result.diagnostics.is_empty(), "{source:?}: {result:?}");
        assert!(matches!(
            paragraph(&result),
            [IrInline::PageCounter { target, .. }] if *target == expected
        ));
    }
}

#[test]
fn page_counters_reject_arguments_and_bodies_before_nested_evaluation() {
    for source in [
        ".currentpage {value}\n",
        ".totalpages named:{value}\n",
        ".currentpage\n    .grid columns:{0}\n        body\n",
    ] {
        let (result, _) = compile_source(source);
        assert_eq!(result.diagnostics.len(), 1, "{source:?}: {result:?}");
        assert!(result.ir.nodes.is_empty(), "{source:?}: {result:?}");
        assert!(
            !result.diagnostics[0].message.contains("Column count"),
            "{source:?}: {result:?}"
        );
    }
}

#[test]
fn page_counter_rejects_lambda_body_before_evaluating_nested_content() {
    let span = SourceSpan::new(SourceId(7), 10, 20);
    let document = arkst_core::ir::IrDocument {
        nodes: vec![IrNode::FunctionCall {
            name: "currentpage".to_string(),
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
                name: "not".to_string(),
                positional_args: vec![IrValue::Number(1.0)],
                named_args: Vec::new(),
                ordered_args: None,
                lambda_parameters: None,
                body: None,
                raw_body: None,
                span,
            }]),
            raw_body: None,
            span,
        }],
        metadata: arkst_core::ir::IrMetadata::default(),
    };

    let (evaluated, diagnostics) = Evaluator::new().evaluate(&document);
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert!(diagnostics[0].message.contains("lambda"), "{diagnostics:?}");
    assert!(!diagnostics[0].message.contains("boolean"), "{diagnostics:?}");
    assert!(evaluated.nodes.is_empty(), "{evaluated:?}");
}

#[test]
fn source_defined_page_counter_names_keep_precedence() {
    let source = ".function {currentpage}\n    custom-current\n\n.function {totalpages}\n    custom-total\n\n.currentpage .totalpages\n";
    let (result, _) = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(result.ir.nodes.len(), 1, "{result:?}");
    assert_eq!(paragraph_text(&result.ir.nodes[0]), "custom-current custom-total");
}

#[test]
fn page_counter_ir_roundtrips_with_target_and_span() {
    for target in [IrPageCounterTarget::Current, IrPageCounterTarget::Total] {
        let value = IrInline::PageCounter {
            target,
            span: SourceSpan::new(SourceId(3), 4, 16),
        };
        let encoded = serde_json::to_value(&value).expect("page counter serializes");
        let decoded: IrInline = serde_json::from_value(encoded).expect("page counter deserializes");
        assert_eq!(decoded, value);
    }
}
