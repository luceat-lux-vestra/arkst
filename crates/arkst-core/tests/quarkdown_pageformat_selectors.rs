//! Independent regression coverage for the bounded #175 selector-aware
//! `.pageformat` state foundation.

use arkst_core::ir::{
    IrDocumentAlignment, IrDocumentState, IrPageFormatLayer, IrPageFormatSelector, IrPageRange,
    IrPageSide, IrSize, IrSizeUnit,
};
use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};

fn compile_source(source: &str) -> arkst_core::CompileResult {
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("valid entry")
        .add_source("main.qd", source)
        .expect("valid source")
        .build()
        .expect("valid project");
    compile(&project, &CompileOptions::default())
}

fn inches(value: f64) -> IrSize {
    IrSize {
        value,
        unit: IrSizeUnit::In,
    }
}

#[test]
fn side_selector_retains_typed_alignment_without_mutating_global_alignment() {
    for (raw, side) in [("left", IrPageSide::Left), ("RIGHT", IrPageSide::Right)] {
        let result = compile_source(&format!(
            ".pageformat side:{{{raw}}} alignment:{{center}}\n"
        ));
        assert!(result.diagnostics.is_empty(), "{raw}: {result:?}");
        assert!(result.ir.nodes.is_empty(), "setter must not emit content");
        assert_eq!(result.ir.metadata.document_state.page_alignment, None);
        assert_eq!(
            result.ir.metadata.document_state.page_formats.layers,
            vec![IrPageFormatLayer {
                selector: Some(IrPageFormatSelector {
                    side: Some(side),
                    pages: None,
                }),
                page_width: None,
                page_height: None,
                alignment: Some(IrDocumentAlignment::Center),
            }]
        );
    }
}

#[test]
fn finite_pages_selector_retains_partial_geometry_without_leaking_global_geometry() {
    let result = compile_source(
        ".pageformat pages:{1..3} width:{8in}\n.pageformat pages:{2..4} height:{11in}\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(result.ir.metadata.document_state.page_geometry, None);
    assert_eq!(
        result.ir.metadata.document_state.page_formats.layers,
        vec![
            IrPageFormatLayer {
                selector: Some(IrPageFormatSelector {
                    side: None,
                    pages: Some(IrPageRange {
                        start: Some(1),
                        end: 3,
                    }),
                }),
                page_width: Some(inches(8.0)),
                page_height: None,
                alignment: None,
            },
            IrPageFormatLayer {
                selector: Some(IrPageFormatSelector {
                    side: None,
                    pages: Some(IrPageRange {
                        start: Some(2),
                        end: 4,
                    }),
                }),
                page_width: None,
                page_height: Some(inches(11.0)),
                alignment: None,
            },
        ]
    );
}

#[test]
fn selector_layers_preserve_source_order_and_can_combine_side_with_pages() {
    let result = compile_source(
        ".pageformat side:{left} pages:{1..2} alignment:{start}\n\
         .pageformat side:{right} width:{7in}\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let layers = &result.ir.metadata.document_state.page_formats.layers;
    assert_eq!(layers.len(), 2);
    assert_eq!(
        layers[0].selector,
        Some(IrPageFormatSelector {
            side: Some(IrPageSide::Left),
            pages: Some(IrPageRange {
                start: Some(1),
                end: 2,
            }),
        })
    );
    assert_eq!(layers[0].alignment, Some(IrDocumentAlignment::Start));
    assert_eq!(
        layers[1].selector,
        Some(IrPageFormatSelector {
            side: Some(IrPageSide::Right),
            pages: None,
        })
    );
    assert_eq!(layers[1].page_width, Some(inches(7.0)));
}

#[test]
fn invalid_side_and_invalid_page_ranges_fail_without_publishing_a_layer() {
    for source in [
        ".pageformat side:{diagonal} alignment:{start}\n",
        ".pageformat pages:{1..} width:{8in}\n",
        ".pageformat pages:{0..2} width:{8in}\n",
        ".pageformat pages:{-1..2} width:{8in}\n",
    ] {
        let result = compile_source(source);
        assert!(
            !result.diagnostics.is_empty(),
            "invalid selector unexpectedly succeeded: {source}"
        );
        assert!(
            result
                .ir
                .metadata
                .document_state
                .page_formats
                .layers
                .is_empty(),
            "invalid selector published state: {source}: {:?}",
            result.ir.metadata.document_state.page_formats
        );
    }
}

#[test]
fn selector_slice_does_not_claim_columns_size_or_decoration_combinations() {
    for source in [
        ".pageformat side:{left} columns:{2}\n",
        ".pageformat pages:{1..2} size:{a4}\n",
        ".pageformat side:{right} background:{red}\n",
        ".pageformat pages:{1..2} bordertop:{1px}\n",
    ] {
        let result = compile_source(source);
        assert!(
            result
                .ir
                .metadata
                .document_state
                .page_formats
                .layers
                .is_empty(),
            "unsupported selector combination published selector state: {source}: {:?}",
            result.ir.metadata.document_state.page_formats
        );
        assert_eq!(
            result.ir.metadata.document_state.page_columns, None,
            "scoped columns leaked into document-global state: {source}"
        );
        assert_eq!(
            result.ir.metadata.document_state.page_size, None,
            "scoped size leaked into document-global state: {source}"
        );
        assert_eq!(
            result.ir.metadata.document_state.page_background, None,
            "scoped decoration leaked into document-global state: {source}"
        );
    }
}

#[test]
fn failed_selector_conversion_rolls_back_nested_document_state_writes() {
    let result = compile_source(
        ".autopagebreak maxdepth:{3}\n.function {badside}\n    .autopagebreak maxdepth:{1}\n    diagonal\n\n.pageformat side:{.badside} alignment:{start}\n",
    );
    assert!(!result.diagnostics.is_empty(), "invalid side must fail");
    assert_eq!(
        result.ir.metadata.document_state.auto_page_break_max_depth,
        Some(3),
        "outer pageformat failure leaked nested document-state mutation"
    );
    assert!(result
        .ir
        .metadata
        .document_state
        .page_formats
        .layers
        .is_empty());
}

#[test]
fn source_defined_pageformat_shadows_selector_builtin() {
    let result = compile_source(
        ".function {pageformat}\n    side:\n    SHADOW-PAGEFORMAT-SELECTOR\n\n.pageformat side:{left} alignment:{center}\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert!(result
        .ir
        .metadata
        .document_state
        .page_formats
        .layers
        .is_empty());
    assert!(
        format!("{:?}", result.ir).contains("SHADOW-PAGEFORMAT-SELECTOR"),
        "source-defined function must retain ownership"
    );
}

#[test]
fn selector_state_wire_is_backward_compatible_and_roundtrips() {
    let state = IrDocumentState::default();
    let legacy_shape = serde_json::to_value(&state).expect("serialize default state");
    assert!(legacy_shape.get("page_formats").is_none());
    let restored: IrDocumentState =
        serde_json::from_value(legacy_shape).expect("deserialize legacy-shaped state");
    assert!(restored.page_formats.layers.is_empty());

    let explicit = IrDocumentState {
        page_formats: arkst_core::ir::IrPageFormatState {
            layers: vec![IrPageFormatLayer {
                selector: Some(IrPageFormatSelector {
                    side: Some(IrPageSide::Left),
                    pages: Some(IrPageRange {
                        start: Some(1),
                        end: 2,
                    }),
                }),
                page_width: Some(inches(8.0)),
                page_height: None,
                alignment: Some(IrDocumentAlignment::Justify),
            }],
        },
        ..IrDocumentState::default()
    };
    let value = serde_json::to_value(&explicit).expect("serialize selector state");
    assert!(value.get("page_formats").is_some());
    let restored: IrDocumentState =
        serde_json::from_value(value).expect("round trip selector state");
    assert_eq!(restored, explicit);
}
