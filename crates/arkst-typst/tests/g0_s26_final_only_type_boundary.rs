use arkst_ir::{
    IrCallable, IrCallableCapture, IrDocument, IrInline, IrInlineBody, IrMetadata, IrNode, IrValue,
};
use arkst_source::{SourceId, SourceMapEntry, SourceSpan};
use arkst_typst::lowering::lower_to_typst;

#[test]
fn g0_s26_current_public_ir_types_still_admit_executable_state_at_backend_boundary() {
    let span = SourceSpan::new(SourceId(719), 10, 20);
    let nested_inline = IrInline::DirectiveCall {
        name: "nested".to_string(),
        positional_args: Vec::new(),
        named_args: Vec::new(),
        ordered_args: None,
        body: None,
        span,
    };

    let callable = IrValue::Callable(IrCallable {
        parameters: None,
        body: Vec::new(),
        span,
        capture: Some(Box::new(IrCallableCapture {
            variables: Vec::new(),
            functions: Vec::new(),
            resource_context: None,
        })),
    });
    let inline_body = IrValue::InlineBody(IrInlineBody {
        content: Vec::new(),
        parameters: None,
        body: Vec::new(),
        span,
    });

    let document = IrDocument {
        nodes: vec![IrNode::FunctionCall {
            name: "outer".to_string(),
            positional_args: vec![callable, inline_body],
            named_args: Vec::new(),
            ordered_args: None,
            lambda_parameters: None,
            body: Some(vec![IrNode::Paragraph {
                content: vec![nested_inline],
                span,
            }]),
            raw_body: None,
            span,
        }],
        metadata: IrMetadata::default(),
    };

    let _backend_still_accepts_the_mixed_lifecycle_type:
        fn(&IrDocument) -> (String, Vec<SourceMapEntry>) = lower_to_typst;

    let IrNode::FunctionCall {
        positional_args,
        body,
        ..
    } = &document.nodes[0]
    else {
        panic!("public IrDocument must admit the constructed executable block carrier");
    };
    assert!(matches!(positional_args[0], IrValue::Callable(_)));
    assert!(matches!(positional_args[1], IrValue::InlineBody(_)));

    let nested = body.as_ref().expect("constructed executable block body");
    let IrNode::Paragraph { content, .. } = &nested[0] else {
        panic!("constructed body must retain its paragraph");
    };
    assert!(matches!(
        content.first(),
        Some(IrInline::DirectiveCall { .. })
    ));
}
