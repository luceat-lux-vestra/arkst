use arkst_ir::{
    IrComponent, IrDocument, IrDocumentType, IrFragmentBehavior, IrFragmentComponent, IrInline,
    IrMetadata, IrNode,
};
use arkst_source::{SourceId, SourceSpan};
use arkst_typst::lowering::{lower_to_typst, lower_to_typst_code};

fn paragraph(text: &str, span: SourceSpan) -> IrNode {
    IrNode::Paragraph {
        content: vec![IrInline::Text {
            content: text.to_string(),
            span,
        }],
        span,
    }
}

fn fragment(
    text: &str,
    behavior: IrFragmentBehavior,
    call_span: SourceSpan,
    child_span: SourceSpan,
) -> IrNode {
    IrNode::Component {
        component: IrComponent::Fragment(IrFragmentComponent {
            behavior,
            children: vec![paragraph(text, child_span)],
            span: call_span,
        }),
    }
}

fn slides_document(nodes: Vec<IrNode>) -> IrDocument {
    let mut metadata = IrMetadata::default();
    metadata.document_state.document_type = IrDocumentType::Slides;
    IrDocument { nodes, metadata }
}

fn marker_visibility(page: &str, marker: &str) -> &'static str {
    let marker_index = page.find(marker).expect("fragment marker");
    let block_index = page[..marker_index]
        .rfind("#block[\n")
        .expect("fragment block");
    let prefix = &page[block_index..marker_index];
    if prefix.contains("#hide[\n") {
        "hidden"
    } else if prefix.contains("transparentize(50%)") {
        "semihidden"
    } else {
        "visible"
    }
}

#[test]
fn fragment_behaviors_expand_one_slide_into_ordered_layout_preserving_snapshots() {
    let behaviors = [
        ("SHOW-FRAGMENT", IrFragmentBehavior::Show),
        ("HIDE-FRAGMENT", IrFragmentBehavior::Hide),
        ("SEMIHIDE-FRAGMENT", IrFragmentBehavior::SemiHide),
        ("SHOWHIDE-FRAGMENT", IrFragmentBehavior::ShowHide),
    ];
    let mut nodes = vec![paragraph(
        "SLIDE-CONTEXT",
        SourceSpan::new(SourceId(184), 0, 9),
    )];
    let mut call_spans = Vec::new();
    for (index, (text, behavior)) in behaviors.into_iter().enumerate() {
        let start = 20 + index * 20;
        let call_span = SourceSpan::new(SourceId(184), start, start + 10);
        call_spans.push(call_span);
        nodes.push(fragment(
            text,
            behavior,
            call_span,
            SourceSpan::new(SourceId(184), start + 11, start + 19),
        ));
    }

    let (code, source_map) = lower_to_typst(&slides_document(nodes));
    let pages = code.split("#pagebreak(weak: true)\n").collect::<Vec<_>>();
    assert_eq!(pages.len(), 6, "{code}");
    assert!(pages.iter().all(|page| page.contains("SLIDE-CONTEXT")));

    let expected = [
        ["hidden", "visible", "visible", "hidden"],
        ["visible", "visible", "visible", "hidden"],
        ["visible", "hidden", "visible", "hidden"],
        ["visible", "hidden", "semihidden", "hidden"],
        ["visible", "hidden", "semihidden", "visible"],
        ["visible", "hidden", "semihidden", "hidden"],
    ];
    for (page, expected) in pages.iter().zip(expected) {
        for ((marker, _), expected) in behaviors.iter().zip(expected) {
            assert_eq!(marker_visibility(page, marker), expected, "{page}");
        }
    }

    for span in call_spans {
        assert_eq!(
            source_map
                .iter()
                .filter(|entry| entry.original == span)
                .count(),
            6,
            "{source_map:?}"
        );
    }
}

#[test]
fn fragment_expansion_resets_at_explicit_slide_boundaries() {
    let document = slides_document(vec![
        fragment(
            "FIRST-SLIDE-FRAGMENT",
            IrFragmentBehavior::Show,
            SourceSpan::new(SourceId(184), 0, 10),
            SourceSpan::new(SourceId(184), 11, 20),
        ),
        IrNode::PageBreak {
            span: SourceSpan::new(SourceId(184), 21, 24),
        },
        fragment(
            "SECOND-SLIDE-FRAGMENT",
            IrFragmentBehavior::ShowHide,
            SourceSpan::new(SourceId(184), 25, 35),
            SourceSpan::new(SourceId(184), 36, 45),
        ),
    ]);
    let code = lower_to_typst_code(&document);
    assert_eq!(code.matches("FIRST-SLIDE-FRAGMENT").count(), 2, "{code}");
    assert_eq!(code.matches("SECOND-SLIDE-FRAGMENT").count(), 3, "{code}");
    assert_eq!(code.matches("#pagebreak(weak: true)").count(), 4, "{code}");
}
