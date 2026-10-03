use arkst_ir::{
    IrComponent, IrCrossAxisAlignment, IrDocument, IrDocumentAlignment, IrDocumentState, IrInline,
    IrMainAxisAlignment, IrMetadata, IrNode, IrPageFormatLayer, IrStackedComponent,
    IrStackedLayout,
};
use arkst_source::{SourceId, SourceSpan};
use arkst_typst::lowering::lower_to_typst_code;

fn span() -> SourceSpan {
    SourceSpan::new(SourceId(0), 0, 0)
}

fn paragraph(text: &str) -> IrNode {
    IrNode::Paragraph {
        content: vec![IrInline::Text {
            content: text.to_string(),
            span: span(),
        }],
        span: span(),
    }
}

fn stack(layout: IrStackedLayout, alignment: Option<IrMainAxisAlignment>) -> IrNode {
    IrNode::Component {
        component: IrComponent::Stacked(IrStackedComponent {
            layout,
            main_axis_alignment: alignment,
            cross_axis_alignment: IrCrossAxisAlignment::Center,
            row_gap: None,
            column_gap: None,
            children: vec![paragraph("A"), paragraph("B")],
            span: span(),
        }),
    }
}

fn lower(page_alignment: Option<IrDocumentAlignment>, node: IrNode) -> String {
    lower_to_typst_code(&IrDocument {
        nodes: vec![node],
        metadata: IrMetadata {
            document_state: IrDocumentState {
                page_alignment,
                ..IrDocumentState::default()
            },
            ..IrMetadata::default()
        },
    })
}

#[test]
fn omitted_row_and_column_consume_final_document_alignment() {
    let row = lower(
        Some(IrDocumentAlignment::Center),
        stack(IrStackedLayout::Row, None),
    );
    assert_eq!(row.matches("h(1fr)").count(), 2, "{row}");

    let column = lower(
        Some(IrDocumentAlignment::Center),
        stack(IrStackedLayout::Column, None),
    );
    assert_eq!(column.matches("v(1fr)").count(), 2, "{column}");

    let end = lower(
        Some(IrDocumentAlignment::End),
        stack(IrStackedLayout::Row, None),
    );
    assert_eq!(end.matches("h(1fr)").count(), 1, "{end}");
}

#[test]
fn explicit_alignment_wins_and_justify_falls_back_to_start() {
    let explicit = lower(
        Some(IrDocumentAlignment::Center),
        stack(IrStackedLayout::Row, Some(IrMainAxisAlignment::Start)),
    );
    assert_eq!(explicit.matches("h(1fr)").count(), 0, "{explicit}");

    let justify = lower(
        Some(IrDocumentAlignment::Justify),
        stack(IrStackedLayout::Row, None),
    );
    assert_eq!(justify.matches("h(1fr)").count(), 0, "{justify}");
}

#[test]
fn ordered_global_alignment_overrides_stale_flattened_page_alignment() {
    let mut state = IrDocumentState {
        page_alignment: Some(IrDocumentAlignment::End),
        ..IrDocumentState::default()
    };
    state.page_format.layers = vec![
        IrPageFormatLayer {
            alignment: Some(IrDocumentAlignment::Start),
            ..IrPageFormatLayer::default()
        },
        IrPageFormatLayer {
            alignment: Some(IrDocumentAlignment::Center),
            ..IrPageFormatLayer::default()
        },
    ];

    let row = lower_to_typst_code(&IrDocument {
        nodes: vec![stack(IrStackedLayout::Row, None)],
        metadata: IrMetadata {
            document_state: state,
            ..IrMetadata::default()
        },
    });

    assert_eq!(row.matches("h(1fr)").count(), 2, "{row}");
}

#[test]
fn effectless_ordered_state_does_not_revive_stale_flattened_page_alignment() {
    let mut state = IrDocumentState {
        page_alignment: Some(IrDocumentAlignment::Center),
        ..IrDocumentState::default()
    };
    state.page_format.layers = vec![IrPageFormatLayer::default()];

    let row = lower_to_typst_code(&IrDocument {
        nodes: vec![stack(IrStackedLayout::Row, None)],
        metadata: IrMetadata {
            document_state: state,
            ..IrMetadata::default()
        },
    });

    assert_eq!(row.matches("h(1fr)").count(), 0, "{row}");
}

#[test]
fn manual_grid_omission_keeps_grid_default_instead_of_page_inheritance() {
    let grid = lower(
        Some(IrDocumentAlignment::End),
        stack(
            IrStackedLayout::Grid {
                columns: 2.try_into().unwrap(),
            },
            None,
        ),
    );
    assert!(grid.contains("#align(center)["), "{grid}");
    assert!(!grid.contains("#align(right)["), "{grid}");
}
