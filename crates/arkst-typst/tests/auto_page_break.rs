use arkst_ir::{IrDocument, IrDocumentState, IrDocumentType, IrInline, IrMetadata, IrNode};
use arkst_source::{SourceId, SourceSpan};
use arkst_typst::lowering::lower_to_typst_code;

fn span() -> SourceSpan {
    SourceSpan::new(SourceId(0), 0, 0)
}

fn heading(level: usize, label: &str) -> IrNode {
    IrNode::Heading {
        level,
        content: vec![IrInline::Text {
            content: label.to_string(),
            span: span(),
        }],
        span: span(),
    }
}

fn document(
    document_type: IrDocumentType,
    override_depth: Option<u32>,
    nodes: Vec<IrNode>,
) -> IrDocument {
    IrDocument {
        nodes,
        metadata: IrMetadata {
            document_state: IrDocumentState {
                document_type,
                auto_page_break_max_depth: override_depth,
                ..IrDocumentState::default()
            },
            ..IrMetadata::default()
        },
    }
}

fn break_count(code: &str) -> usize {
    code.matches("#pagebreak(weak: true)").count()
}

#[test]
fn implicit_defaults_match_the_pinned_v260_oracle() {
    let nodes = vec![heading(1, "H1"), heading(2, "H2"), heading(3, "H3")];
    for (document_type, expected) in [
        (IrDocumentType::Plain, 0),
        (IrDocumentType::Paged, 1),
        (IrDocumentType::Slides, 2),
        (IrDocumentType::Docs, 0),
    ] {
        let code = lower_to_typst_code(&document(document_type, None, nodes.clone()));
        assert_eq!(break_count(&code), expected, "{document_type:?}: {code}");
    }
}

#[test]
fn explicit_thresholds_override_document_type_and_zero_disables() {
    let nodes = vec![heading(1, "H1"), heading(2, "H2"), heading(3, "H3")];
    for (depth, expected) in [(0, 0), (1, 1), (2, 2), (3, 3)] {
        let code = lower_to_typst_code(&document(
            IrDocumentType::Slides,
            Some(depth),
            nodes.clone(),
        ));
        assert_eq!(break_count(&code), expected, "depth {depth}: {code}");
    }
}

#[test]
fn only_top_level_headings_create_typst_page_boundaries() {
    let nested = IrNode::Blockquote {
        content: vec![heading(2, "Nested H2")],
        span: span(),
    };
    let code = lower_to_typst_code(&document(
        IrDocumentType::Slides,
        None,
        vec![nested, heading(2, "Top H2")],
    ));
    assert_eq!(break_count(&code), 1, "{code}");
    assert!(code.contains("#quote(block: true)["), "{code}");
}

#[test]
fn emitted_breaks_are_weak_for_heading_at_start_consecutive_and_manual_adjacency_safety() {
    let code = lower_to_typst_code(&document(
        IrDocumentType::Slides,
        None,
        vec![heading(1, "First"), heading(2, "Second")],
    ));
    assert_eq!(break_count(&code), 2, "{code}");
    assert!(!code.contains("#pagebreak()"), "{code}");
}
