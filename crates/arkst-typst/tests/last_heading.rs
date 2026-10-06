use arkst_ir::{IrDocument, IrDocumentState, IrDocumentType, IrInline, IrMetadata, IrNode};
use arkst_source::{SourceId, SourceSpan};
use arkst_typst::lowering::lower_to_typst_code;

#[test]
fn lastheading_remains_explicitly_fail_closed_at_typst_boundary() {
    let span = SourceSpan::new(SourceId(0), 0, 0);
    let document = IrDocument {
        nodes: vec![IrNode::Paragraph {
            content: vec![IrInline::LastHeading { depth: 7, span }],
            span,
        }],
        metadata: IrMetadata {
            document_state: IrDocumentState {
                document_type: IrDocumentType::Paged,
                ..IrDocumentState::default()
            },
            ..IrMetadata::default()
        },
    };

    let typst = lower_to_typst_code(&document);
    assert!(typst.contains("#panic("), "{typst}");
    assert!(
        typst.contains(".lastheading requires page-aware heading history"),
        "{typst}"
    );
}
