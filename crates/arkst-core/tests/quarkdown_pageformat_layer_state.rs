//! Regression coverage for the bounded #175 ordered selector-free
//! `.pageformat` layer-state slice.

use arkst_core::ir::{IrDocumentState, IrPageOrientation, IrPageSizeFormat, IrSizeUnit};
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

#[test]
fn ordered_pageformat_layers_preserve_supported_global_source_order() {
    let result = compile_source(
        ".pageformat size:{a4} orientation:{portrait}\n\
         .pageformat width:{10in} height:{5in}\n\
         .pageformat margin:{1cm 2cm}\n\
         .pageformat bordertop:{2pt} bordercolor:{red}\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");

    let layers = &result.ir.metadata.document_state.page_format.layers;
    assert_eq!(layers.len(), 4);

    let size = layers[0].size.expect("size layer");
    assert_eq!(size.format, IrPageSizeFormat::A4);
    assert_eq!(size.orientation, Some(IrPageOrientation::Portrait));
    assert!(layers[0].width.is_none());
    assert!(layers[0].height.is_none());

    let width = layers[1].width.as_ref().expect("width");
    let height = layers[1].height.as_ref().expect("height");
    assert_eq!((width.value, width.unit), (10.0, IrSizeUnit::In));
    assert_eq!((height.value, height.unit), (5.0, IrSizeUnit::In));
    assert!(layers[1].size.is_none());

    let margin = layers[2].margin.as_ref().expect("margin");
    assert_eq!((margin.top.value, margin.top.unit), (1.0, IrSizeUnit::Cm));
    assert_eq!(
        (margin.right.value, margin.right.unit),
        (2.0, IrSizeUnit::Cm)
    );
    assert_eq!(
        (margin.bottom.value, margin.bottom.unit),
        (1.0, IrSizeUnit::Cm)
    );
    assert_eq!((margin.left.value, margin.left.unit), (2.0, IrSizeUnit::Cm));

    let widths = layers[3].border_widths.as_ref().expect("border widths");
    assert_eq!((widths.top.value, widths.top.unit), (2.0, IrSizeUnit::Pt));
    assert_eq!(widths.right.value, 0.0);
    assert_eq!(widths.bottom.value, 0.0);
    assert_eq!(widths.left.value, 0.0);
    assert!(layers[3].border_color.is_some());
}

#[test]
fn ordered_layers_expose_size_vs_geometry_order_without_changing_flat_consumers() {
    let result = compile_source(
        ".pageformat size:{a4}\n\
         .pageformat width:{10in} height:{5in}\n\
         .pageformat size:{legal} orientation:{landscape}\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");

    let state = &result.ir.metadata.document_state;
    let layers = &state.page_format.layers;
    assert_eq!(layers.len(), 3);
    assert_eq!(
        layers[0].size.expect("first size").format,
        IrPageSizeFormat::A4
    );
    assert!(layers[1].width.is_some());
    assert!(layers[1].height.is_some());
    assert_eq!(
        layers[2].size.expect("last size").format,
        IrPageSizeFormat::Legal
    );

    // Existing bounded consumers remain unchanged in this prerequisite slice.
    assert!(state.page_geometry.is_some());
    assert_eq!(
        state.page_size.expect("flattened size").format,
        IrPageSizeFormat::Legal
    );
}

#[test]
fn semantic_none_is_retained_as_an_ordered_noop_layer() {
    let result = compile_source(".pageformat size:{a4}\n.pageformat size:{.none}\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");

    let state = &result.ir.metadata.document_state;
    assert_eq!(state.page_format.layers.len(), 2);
    assert!(state.page_format.layers[0].size.is_some());
    assert_eq!(state.page_format.layers[1], Default::default());
    assert_eq!(
        state.page_size.expect("effective size").format,
        IrPageSizeFormat::A4
    );
}

#[test]
fn failed_pageformat_conversion_rolls_back_ordered_layer_publication() {
    let result = compile_source(
        ".pageformat size:{a4}\n.function {badsize}\n    .pageformat margin:{1cm}\n    not-a-paper\n\n.pageformat size:{.badsize}\n",
    );
    assert!(!result.diagnostics.is_empty(), "invalid size must fail");

    let layers = &result.ir.metadata.document_state.page_format.layers;
    assert_eq!(
        layers.len(),
        1,
        "failed outer call must roll back nested pageformat layer writes"
    );
    assert_eq!(
        layers[0].size.expect("committed size").format,
        IrPageSizeFormat::A4
    );
}

#[test]
fn pageformat_layer_wire_defaults_for_old_ir_and_roundtrips_when_present() {
    let state = IrDocumentState::default();
    let legacy_shape = serde_json::to_value(&state).expect("serialize default state");
    assert!(legacy_shape.get("page_format").is_none());

    let restored: IrDocumentState =
        serde_json::from_value(legacy_shape).expect("deserialize legacy-shaped state");
    assert!(restored.page_format.layers.is_empty());

    let compiled = compile_source(".pageformat size:{letter}\n");
    assert!(compiled.diagnostics.is_empty(), "{compiled:?}");
    let explicit = compiled.ir.metadata.document_state;
    let value = serde_json::to_value(&explicit).expect("serialize pageformat state");
    assert!(value.get("page_format").is_some());
    let restored: IrDocumentState =
        serde_json::from_value(value).expect("round trip pageformat state");
    assert_eq!(restored.page_format, explicit.page_format);
}
