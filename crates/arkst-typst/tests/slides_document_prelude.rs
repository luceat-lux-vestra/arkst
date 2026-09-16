use arkst_ir::{
    IrDocument, IrDocumentTheme, IrDocumentType, IrMetadata, IrNode, IrPageGeometry, IrSize,
    IrSizeUnit, IrSlidesConfiguration,
};
use arkst_source::{SourceId, SourceSpan};
use arkst_typst::lowering::lower_to_typst_code;

fn slides_document(center: Option<bool>, focus: bool) -> IrDocument {
    let mut metadata = IrMetadata::default();
    metadata.document_state.document_type = IrDocumentType::Slides;
    metadata.document_state.slides = Some(IrSlidesConfiguration { center });
    if focus {
        metadata.document_state.theme = Some(IrDocumentTheme {
            color: None,
            layout: Some("focus".to_string()),
        });
    }
    IrDocument {
        nodes: Vec::new(),
        metadata,
    }
}

#[test]
fn slides_default_geometry_is_renderer_owned_and_nullable_center_emits_no_alignment_override() {
    let code = lower_to_typst_code(&slides_document(None, false));
    assert!(
        code.starts_with("#set page(width: 749.04pt, height: 546pt)\n"),
        "{code}"
    );
    assert!(!code.contains("#set align("), "{code}");
}

#[test]
fn explicit_center_values_lower_to_closed_vertical_alignment_choices() {
    let centered = lower_to_typst_code(&slides_document(Some(true), false));
    assert!(centered.contains("#set align(horizon)\n"), "{centered}");

    let top = lower_to_typst_code(&slides_document(Some(false), false));
    assert!(top.contains("#set align(top)\n"), "{top}");
}

#[test]
fn explicit_page_geometry_overrides_slides_default_before_alignment() {
    let mut doc = slides_document(Some(true), false);
    doc.metadata.document_state.page_geometry = Some(IrPageGeometry {
        width: IrSize {
            value: 10.0,
            unit: IrSizeUnit::In,
        },
        height: IrSize {
            value: 5.0,
            unit: IrSizeUnit::In,
        },
    });
    let code = lower_to_typst_code(&doc);
    assert!(code.starts_with("#set page(width: 10in, height: 5in)\n"), "{code}");
    assert!(!code.contains("749.04pt"), "{code}");
    assert!(
        code.find("#set page").expect("page prelude")
            < code.find("#set align(horizon)").expect("alignment prelude"),
        "{code}"
    );
}

#[test]
fn focus_layout_is_applied_after_page_and_center_contracts() {
    let code = lower_to_typst_code(&slides_document(Some(true), true));
    let page = code.find("#set page").expect("page prelude");
    let align = code.find("#set align(horizon)").expect("alignment prelude");
    let focus = code
        .find("// Arkst Quarkdown v2.6 focus layout")
        .expect("focus prelude");
    assert!(page < align && align < focus, "{code}");
}

#[test]
fn semantic_pagebreak_lowers_to_weak_renderer_boundary() {
    let mut doc = slides_document(None, false);
    doc.nodes.push(IrNode::PageBreak {
        span: SourceSpan::new(SourceId(1), 4, 7),
    });
    let code = lower_to_typst_code(&doc);
    assert!(code.contains("#pagebreak(weak: true)\n"), "{code}");
    assert!(!code.contains("#pagebreak(weak: false)"), "{code}");
}
