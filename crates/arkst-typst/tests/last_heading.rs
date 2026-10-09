use arkst_ir::{IrDocument, IrDocumentState, IrDocumentType, IrInline, IrMetadata, IrNode};
use arkst_source::{SourceId, SourceSpan};
use arkst_typst::lowering::lower_to_typst_code;

#[test]
fn paged_lastheading_lowers_to_page_aware_heading_query() {
    let span = SourceSpan::new(SourceId(0), 0, 0);
    let document = IrDocument {
        nodes: vec![IrNode::Paragraph {
            content: vec![IrInline::LastHeading { depth: 2, span }],
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
    assert!(typst.contains("#context {"), "{typst}");
    assert!(typst.contains("query(heading)"), "{typst}");
    assert!(typst.contains("it.level == 2"), "{typst}");
    assert!(typst.contains("it.level < 2"), "{typst}");
    assert!(typst.contains("it.location().page() > __arkst_candidate_page"), "{typst}");
    assert!(typst.contains("__arkst_candidate.body"), "{typst}");
    assert!(!typst.contains("#panic("), "{typst}");
}

#[test]
fn paged_lastheading_out_of_runtime_heading_range_uses_empty_fallback() {
    let span = SourceSpan::new(SourceId(0), 0, 0);
    for depth in [0, -2, 7] {
        let document = IrDocument {
            nodes: vec![IrNode::Paragraph {
                content: vec![IrInline::LastHeading { depth, span }],
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

        assert_eq!(lower_to_typst_code(&document).trim(), "[]", "depth={depth}");
    }
}

#[test]
fn lastheading_remains_fail_closed_for_non_paged_typst_output() {
    let span = SourceSpan::new(SourceId(0), 0, 0);
    for document_type in [IrDocumentType::Plain, IrDocumentType::Slides, IrDocumentType::Docs] {
        let document = IrDocument {
            nodes: vec![IrNode::Paragraph {
                content: vec![IrInline::LastHeading { depth: 2, span }],
                span,
            }],
            metadata: IrMetadata {
                document_state: IrDocumentState {
                    document_type,
                    ..IrDocumentState::default()
                },
                ..IrMetadata::default()
            },
        };

        let typst = lower_to_typst_code(&document);
        assert!(typst.contains("#panic("), "{document_type:?}: {typst}");
        assert!(
            typst.contains(".lastheading output currently supports only final paged documents"),
            "{document_type:?}: {typst}"
        );
    }
}
