use arkst_ir::{IrCodeCallout, IrDocument, IrMetadata, IrNode};
use arkst_source::{SourceId, SourceSpan};
use arkst_typst::lowering::lower_to_typst_code;

#[test]
fn native_code_callouts_lower_without_changing_legacy_fences() {
    let span = SourceSpan::new(SourceId(0), 0, 0);
    let native = IrNode::CodeBlock {
        language: Some("rust".into()),
        info: None,
        source: "alpha\nbeta\ngamma".into(),
        line_numbers: Some(true),
        callouts: vec![
            IrCodeCallout {
                line: 1,
                description: "First".into(),
            },
            IrCodeCallout {
                line: 3,
                description: "Third".into(),
            },
        ],
        span,
    };
    let code = lower_to_typst_code(&IrDocument {
        nodes: vec![native],
        metadata: IrMetadata::default(),
    });
    assert!(code.contains("#grid("), "{code}");
    assert!(code.contains("#raw(\"alpha\", lang: \"rust\")"), "{code}");
    assert!(code.contains("#enum("), "{code}");
    assert!(code.contains("[First]"), "{code}");
    assert!(code.contains("[Third]"), "{code}");

    let legacy = IrNode::CodeBlock {
        language: Some("rust".into()),
        info: Some("rust".into()),
        source: "alpha".into(),
        line_numbers: None,
        callouts: vec![],
        span,
    };
    let legacy = lower_to_typst_code(&IrDocument {
        nodes: vec![legacy],
        metadata: IrMetadata::default(),
    });
    assert_eq!(legacy, "```rust\nalpha\n```\n\n");
}
