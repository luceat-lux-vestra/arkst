use arkst_ir::{
    IrComponent, IrDocument, IrDocumentType, IrInline, IrMetadata, IrNode, IrSlidesConfiguration,
    IrSpeakerNoteComponent,
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

fn speaker_note(text: &str, call_span: SourceSpan, child_span: SourceSpan) -> IrNode {
    IrNode::Component {
        component: IrComponent::SpeakerNote(IrSpeakerNoteComponent {
            children: vec![paragraph(text, child_span)],
            span: call_span,
        }),
    }
}

fn document(document_type: IrDocumentType, show_notes: Option<bool>) -> IrDocument {
    let mut metadata = IrMetadata::default();
    metadata.document_state.document_type = document_type;
    metadata.document_state.slides = Some(IrSlidesConfiguration {
        speaker_notes: show_notes,
        ..IrSlidesConfiguration::default()
    });
    IrDocument {
        nodes: vec![
            speaker_note(
                "FIRST-NOTE",
                SourceSpan::new(SourceId(184), 10, 22),
                SourceSpan::new(SourceId(184), 27, 37),
            ),
            speaker_note(
                "SECOND-NOTE",
                SourceSpan::new(SourceId(184), 40, 52),
                SourceSpan::new(SourceId(184), 57, 68),
            ),
        ],
        metadata,
    }
}

#[test]
fn speaker_notes_are_hidden_for_default_false_and_non_slides_output() {
    for document in [
        document(IrDocumentType::Slides, None),
        document(IrDocumentType::Slides, Some(false)),
        document(IrDocumentType::Paged, Some(true)),
    ] {
        let (code, source_map) = lower_to_typst(&document);
        assert!(!code.contains("FIRST-NOTE"), "{code}");
        assert!(!code.contains("SECOND-NOTE"), "{code}");
        assert!(!source_map
            .iter()
            .any(|entry| entry.original.source_id == SourceId(184)));
    }
}

#[test]
fn enabled_speaker_notes_lower_in_source_order_with_nested_provenance() {
    let document = document(IrDocumentType::Slides, Some(true));
    let (code, source_map) = lower_to_typst(&document);
    let first = code.find("FIRST-NOTE").expect("first note output");
    let second = code.find("SECOND-NOTE").expect("second note output");
    assert!(first < second, "{code}");
    assert_eq!(code.matches("#block[\n").count(), 2, "{code}");

    for expected in [
        SourceSpan::new(SourceId(184), 10, 22),
        SourceSpan::new(SourceId(184), 27, 37),
        SourceSpan::new(SourceId(184), 40, 52),
        SourceSpan::new(SourceId(184), 57, 68),
    ] {
        assert!(
            source_map.iter().any(|entry| entry.original == expected),
            "missing {expected:?}: {source_map:?}"
        );
    }
}

#[test]
fn final_slides_state_controls_notes_regardless_of_component_construction_order() {
    let hidden = lower_to_typst_code(&document(IrDocumentType::Slides, Some(false)));
    let shown = lower_to_typst_code(&document(IrDocumentType::Slides, Some(true)));
    assert!(!hidden.contains("FIRST-NOTE"), "{hidden}");
    assert!(shown.contains("FIRST-NOTE"), "{shown}");
}
