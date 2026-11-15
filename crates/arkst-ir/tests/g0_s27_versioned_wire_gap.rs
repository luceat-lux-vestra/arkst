use arkst_ir::{
    IrCallable, IrCallableCapture, IrDocument, IrInline, IrInlineBody, IrMetadata, IrNode,
    IrRawBody, IrValue,
};
use arkst_source::{ByteSpan, SourceId, SourceSpan, SourceText};

fn mixed_executable_document() -> IrDocument {
    let span = SourceSpan::new(SourceId(27), 0, 4);
    IrDocument {
        nodes: vec![IrNode::FunctionCall {
            name: "outer".to_string(),
            positional_args: vec![
                IrValue::Callable(IrCallable {
                    parameters: None,
                    body: Vec::new(),
                    span,
                    capture: Some(Box::new(IrCallableCapture {
                        variables: Vec::new(),
                        functions: Vec::new(),
                        resource_context: None,
                    })),
                }),
                IrValue::InlineBody(IrInlineBody {
                    content: Vec::new(),
                    parameters: None,
                    body: Vec::new(),
                    span,
                }),
            ],
            named_args: Vec::new(),
            ordered_args: None,
            lambda_parameters: None,
            body: Some(vec![IrNode::Paragraph {
                content: vec![IrInline::DirectiveCall {
                    name: "nested".to_string(),
                    positional_args: Vec::new(),
                    named_args: Vec::new(),
                    ordered_args: None,
                    body: None,
                    span,
                }],
                span,
            }]),
            raw_body: Some(IrRawBody::new(
                SourceText::new("body"),
                ByteSpan::new(0, 4),
            )),
            span,
        }],
        metadata: IrMetadata::default(),
    }
}

#[test]
fn current_wire_accepts_unversioned_mixed_executable_document_and_unknown_version() {
    let document = mixed_executable_document();
    let mut encoded = serde_json::to_value(&document).expect("current IrDocument serializes");

    assert!(
        encoded.get("version").is_none(),
        "current document wire must remain unversioned for the G0 negative baseline"
    );
    assert!(encoded["nodes"][0].get("FunctionCall").is_some());

    let decoded = serde_json::from_value::<IrDocument>(encoded.clone())
        .expect("current unversioned executable document decodes");
    assert_eq!(decoded, document);

    encoded["version"] = serde_json::json!(999);
    let decoded_with_unknown_version = serde_json::from_value::<IrDocument>(encoded)
        .expect("current decoder ignores an unknown top-level version field");
    assert_eq!(decoded_with_unknown_version, document);
}

#[test]
fn current_wire_accepts_duplicate_noncanonical_source_table() {
    let document = mixed_executable_document();
    let mut encoded = serde_json::to_value(&document).expect("current IrDocument serializes");

    assert_eq!(encoded["sources"], serde_json::json!(["body"]));
    encoded["sources"] = serde_json::json!(["body", "body"]);
    encoded["nodes"][0]["FunctionCall"]["raw_body"]["source_ref"] = serde_json::json!(1);

    let decoded = serde_json::from_value::<IrDocument>(encoded)
        .expect("current decoder accepts a duplicate noncanonical source table");
    assert_eq!(decoded, document);

    let canonicalized =
        serde_json::to_value(decoded).expect("decoded current document reserializes");
    assert_eq!(canonicalized["sources"], serde_json::json!(["body"]));
    assert_eq!(
        canonicalized["nodes"][0]["FunctionCall"]["raw_body"]["source_ref"],
        serde_json::json!(0)
    );
}

#[test]
fn current_wire_rejects_malformed_callable_shape_before_ir_construction() {
    let document = mixed_executable_document();
    let mut encoded = serde_json::to_value(document).expect("current IrDocument serializes");

    encoded["nodes"][0]["FunctionCall"]["positional_args"][0]["Callable"]["body"] =
        serde_json::json!({"not": "a node array"});

    assert!(
        serde_json::from_value::<IrDocument>(encoded).is_err(),
        "derived serde shape must reject a structurally malformed callable body"
    );
}
