use arkst_ir::{IrCodeCallout, IrDocument, IrMetadata, IrNode};
use arkst_source::{SourceId, SourceSpan};

fn span() -> SourceSpan {
    SourceSpan::new(SourceId(1), 10, 20)
}

#[test]
fn code_callout_fields_roundtrip_when_present() {
    let document = IrDocument {
        nodes: vec![IrNode::CodeBlock {
            language: Some("rust".into()),
            info: None,
            source: "alpha\nbeta".into(),
            line_numbers: Some(false),
            callouts: vec![IrCodeCallout {
                line: 2,
                description: "Second".into(),
            }],
            span: span(),
        }],
        metadata: IrMetadata::default(),
    };

    let encoded = serde_json::to_string(&document).expect("code callout IR serializes");
    let decoded: IrDocument =
        serde_json::from_str(&encoded).expect("code callout IR deserializes");
    assert_eq!(decoded, document);
}

#[test]
fn pre_callout_codeblock_json_defaults_new_fields() {
    let document = IrDocument {
        nodes: vec![IrNode::CodeBlock {
            language: Some("rust".into()),
            info: Some("rust".into()),
            source: "alpha".into(),
            line_numbers: Some(true),
            callouts: vec![IrCodeCallout {
                line: 1,
                description: "First".into(),
            }],
            span: span(),
        }],
        metadata: IrMetadata::default(),
    };

    let mut encoded = serde_json::to_value(&document).expect("IR serializes to JSON value");
    let code_block = encoded["nodes"][0]["CodeBlock"]
        .as_object_mut()
        .expect("externally tagged CodeBlock payload");
    code_block.remove("line_numbers");
    code_block.remove("callouts");

    let decoded: IrDocument =
        serde_json::from_value(encoded).expect("pre-callout CodeBlock JSON remains readable");
    let [IrNode::CodeBlock {
        line_numbers,
        callouts,
        ..
    }] = decoded.nodes.as_slice()
    else {
        panic!("expected one CodeBlock, got {:?}", decoded.nodes);
    };
    assert_eq!(*line_numbers, None);
    assert!(callouts.is_empty());
}
