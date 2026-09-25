use arkst_engine::evaluator::Evaluator;
use arkst_ir::{
    IrDictionary, IrDocument, IrDocumentType, IrMetadata, IrNamedArg, IrNode, IrNumberingToken,
    IrPair, IrValue,
};
use arkst_source::{SourceId, SourceSpan};

fn span(offset: usize) -> SourceSpan {
    SourceSpan::new(SourceId(175), offset, offset + 1)
}

fn pair(key: &str, value: IrValue, offset: usize) -> IrPair {
    IrPair {
        first: Box::new(IrValue::String(key.to_string())),
        second: Box::new(value),
        span: span(offset),
    }
}

fn dictionary(entries: Vec<IrPair>, offset: usize) -> IrValue {
    IrValue::Dictionary(IrDictionary {
        entries,
        span: span(offset),
    })
}

fn numbering_call(merge: Option<bool>, formats: IrValue, offset: usize) -> IrNode {
    let mut named_args = Vec::new();
    if let Some(merge) = merge {
        named_args.push(IrNamedArg {
            name: "merge".to_string(),
            name_span: span(offset),
            value: IrValue::Boolean(merge),
            span: span(offset),
        });
    }
    named_args.push(IrNamedArg {
        name: "formats".to_string(),
        name_span: span(offset + 1),
        value: formats,
        span: span(offset + 1),
    });
    IrNode::FunctionCall {
        name: "numbering".to_string(),
        positional_args: Vec::new(),
        named_args,
        ordered_args: None,
        lambda_parameters: None,
        body: None,
        raw_body: None,
        span: span(offset),
    }
}

fn nonumbering_call(offset: usize) -> IrNode {
    IrNode::FunctionCall {
        name: "nonumbering".to_string(),
        positional_args: Vec::new(),
        named_args: Vec::new(),
        ordered_args: None,
        lambda_parameters: None,
        body: None,
        raw_body: None,
        span: span(offset),
    }
}

fn evaluate(nodes: Vec<IrNode>) -> (IrDocument, Vec<arkst_diagnostics::Diagnostic>) {
    Evaluator::new().evaluate(&IrDocument {
        nodes,
        metadata: IrMetadata::default(),
    })
}

#[test]
fn numbering_records_typed_and_extra_entries_without_output() {
    let formats = dictionary(
        vec![
            pair("headings", IrValue::String("1.".to_string()), 10),
            pair("figures", IrValue::String("A".to_string()), 11),
            pair("custom", IrValue::String("i".to_string()), 12),
        ],
        9,
    );
    let (result, diagnostics) = evaluate(vec![numbering_call(None, formats, 1)]);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(result.nodes.is_empty(), "{:?}", result.nodes);
    let state = &result.metadata.document_state.numbering;
    assert_eq!(state.layers.len(), 1);
    let layer = &state.layers[0];
    assert!(layer.merge);
    assert_eq!(layer.document_type, IrDocumentType::Plain);
    assert_eq!(
        layer.headings.as_ref().unwrap().tokens,
        vec![
            IrNumberingToken::Decimal,
            IrNumberingToken::Literal(".".to_string())
        ]
    );
    assert_eq!(
        layer.figures.as_ref().unwrap().tokens,
        vec![IrNumberingToken::UpperAlpha]
    );
    assert_eq!(
        layer
            .extra
            .iter()
            .map(|(key, _)| key.as_str())
            .collect::<Vec<_>>(),
        vec!["headings", "figures", "custom"]
    );
}

#[test]
fn merge_false_replaces_prior_mutation_history() {
    let first = dictionary(
        vec![pair("headings", IrValue::String("1".to_string()), 10)],
        9,
    );
    let replacement = dictionary(
        vec![pair("figures", IrValue::String("A".to_string()), 20)],
        19,
    );
    let (result, diagnostics) = evaluate(vec![
        numbering_call(None, first, 1),
        numbering_call(Some(false), replacement, 2),
    ]);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let layers = &result.metadata.document_state.numbering.layers;
    assert_eq!(layers.len(), 1);
    assert!(!layers[0].merge);
    assert!(layers[0].headings.is_none());
    assert!(layers[0].figures.is_some());
}

#[test]
fn nonumbering_is_an_empty_replacement_layer() {
    let first = dictionary(
        vec![pair("headings", IrValue::String("1".to_string()), 10)],
        9,
    );
    let (result, diagnostics) = evaluate(vec![numbering_call(None, first, 1), nonumbering_call(2)]);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let layers = &result.metadata.document_state.numbering.layers;
    assert_eq!(layers.len(), 1);
    let layer = &layers[0];
    assert!(!layer.merge);
    assert!(layer.headings.is_none());
    assert!(layer.extra.is_empty());
}

#[test]
fn invalid_later_numbering_call_rolls_back_without_losing_prior_state() {
    let valid = dictionary(
        vec![pair("headings", IrValue::String("1".to_string()), 10)],
        9,
    );
    let invalid = dictionary(
        vec![pair(
            "figures",
            IrValue::Pair(IrPair {
                first: Box::new(IrValue::String("a".to_string())),
                second: Box::new(IrValue::String("b".to_string())),
                span: span(20),
            }),
            20,
        )],
        19,
    );
    let (result, diagnostics) = evaluate(vec![
        numbering_call(None, valid, 1),
        numbering_call(None, invalid, 2),
    ]);

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert!(diagnostics[0].message.contains(".numbering"));
    let layers = &result.metadata.document_state.numbering.layers;
    assert_eq!(layers.len(), 1);
    assert!(layers[0].headings.is_some());
    assert!(layers[0].figures.is_none());
}

#[test]
fn none_string_is_retained_as_explicit_non_counting_format() {
    let formats = dictionary(
        vec![pair("footnotes", IrValue::String("none".to_string()), 10)],
        9,
    );
    let (result, diagnostics) = evaluate(vec![numbering_call(None, formats, 1)]);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let layer = &result.metadata.document_state.numbering.layers[0];
    assert!(layer.footnotes.as_ref().unwrap().tokens.is_empty());
    assert!(layer.extra[0].1.tokens.is_empty());
}
