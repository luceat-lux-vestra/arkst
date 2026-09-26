//! Independent regression coverage for the bounded #175 selector-free
//! `.pageformat` border/background state slice.

use arkst_core::ir::{IrColor, IrDocumentState, IrPageBorderWidths, IrSize, IrSizeUnit};
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

fn px(value: f64) -> IrSize {
    IrSize {
        value,
        unit: IrSizeUnit::Px,
    }
}

fn color(red: u8, green: u8, blue: u8) -> IrColor {
    IrColor {
        red,
        green,
        blue,
        alpha: 1.0,
    }
}

#[test]
fn partial_border_input_materializes_omitted_sides_as_zero() {
    let result = compile_source(".pageformat bordertop:{2px}\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert!(result.ir.nodes.is_empty(), "setter must not emit content");
    assert_eq!(
        result.ir.metadata.document_state.page_border_widths,
        Some(IrPageBorderWidths {
            top: px(2.0),
            right: px(0.0),
            bottom: px(0.0),
            left: px(0.0),
        })
    );
}

#[test]
fn later_partial_border_replaces_previous_width_structure_and_zeroes_omitted_sides() {
    let result = compile_source(
        ".pageformat bordertop:{1px} borderright:{2px} borderbottom:{3px} borderleft:{4px}\n\
         .pageformat bordertop:{5px}\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(
        result.ir.metadata.document_state.page_border_widths,
        Some(IrPageBorderWidths {
            top: px(5.0),
            right: px(0.0),
            bottom: px(0.0),
            left: px(0.0),
        })
    );
}

#[test]
fn bordercolor_only_preserves_existing_widths_and_never_fabricates_new_widths() {
    let with_width = compile_source(".pageformat bordertop:{1px}\n.pageformat bordercolor:{red}\n");
    assert!(with_width.diagnostics.is_empty(), "{with_width:?}");
    assert_eq!(
        with_width.ir.metadata.document_state.page_border_widths,
        Some(IrPageBorderWidths {
            top: px(1.0),
            right: px(0.0),
            bottom: px(0.0),
            left: px(0.0),
        })
    );
    assert_eq!(
        with_width.ir.metadata.document_state.page_border_color,
        Some(color(255, 0, 0))
    );

    let color_only = compile_source(".pageformat bordercolor:{blue}\n");
    assert!(color_only.diagnostics.is_empty(), "{color_only:?}");
    assert_eq!(
        color_only.ir.metadata.document_state.page_border_widths, None,
        "color-only input must not invent a default border width"
    );
    assert_eq!(
        color_only.ir.metadata.document_state.page_border_color,
        Some(color(0, 0, 255))
    );
}

#[test]
fn semantic_none_preserves_existing_decoration_state() {
    let result = compile_source(
        ".pageformat bordertop:{2px} bordercolor:{red} background:{white}\n\
         .pageformat bordertop:{.none} bordercolor:{.none} background:{.none}\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(
        result.ir.metadata.document_state.page_border_widths,
        Some(IrPageBorderWidths {
            top: px(2.0),
            right: px(0.0),
            bottom: px(0.0),
            left: px(0.0),
        })
    );
    assert_eq!(
        result.ir.metadata.document_state.page_border_color,
        Some(color(255, 0, 0))
    );
    assert_eq!(
        result.ir.metadata.document_state.page_background,
        Some(color(255, 255, 255))
    );
}

#[test]
fn later_background_and_border_color_replace_only_their_own_global_fields() {
    let result = compile_source(
        ".pageformat bordertop:{3px} bordercolor:{red} background:{white}\n\
         .pageformat bordercolor:{blue} background:{black}\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(
        result.ir.metadata.document_state.page_border_widths,
        Some(IrPageBorderWidths {
            top: px(3.0),
            right: px(0.0),
            bottom: px(0.0),
            left: px(0.0),
        })
    );
    assert_eq!(
        result.ir.metadata.document_state.page_border_color,
        Some(color(0, 0, 255))
    );
    assert_eq!(
        result.ir.metadata.document_state.page_background,
        Some(color(0, 0, 0))
    );
}

#[test]
fn failed_decoration_conversion_rolls_back_nested_document_state_writes() {
    let result = compile_source(
        ".autopagebreak maxdepth:{3}\n.function {badcolor}\n    .autopagebreak maxdepth:{1}\n    notacolor\n\n.pageformat background:{.badcolor}\n",
    );
    assert!(!result.diagnostics.is_empty(), "invalid color must fail");
    assert_eq!(
        result.ir.metadata.document_state.auto_page_break_max_depth,
        Some(3),
        "outer pageformat failure leaked nested document-state mutation"
    );
    assert_eq!(result.ir.metadata.document_state.page_background, None);
}

#[test]
fn decoration_wire_is_backward_compatible_and_explicit_when_set() {
    let state = IrDocumentState::default();
    let legacy_shape = serde_json::to_value(&state).expect("serialize default state");
    assert!(legacy_shape.get("page_border_widths").is_none());
    assert!(legacy_shape.get("page_border_color").is_none());
    assert!(legacy_shape.get("page_background").is_none());

    let restored: IrDocumentState =
        serde_json::from_value(legacy_shape).expect("deserialize legacy-shaped state");
    assert_eq!(restored.page_border_widths, None);
    assert_eq!(restored.page_border_color, None);
    assert_eq!(restored.page_background, None);

    let explicit = IrDocumentState {
        page_border_widths: Some(IrPageBorderWidths {
            top: px(1.0),
            right: px(2.0),
            bottom: px(3.0),
            left: px(4.0),
        }),
        page_border_color: Some(color(255, 0, 0)),
        page_background: Some(color(0, 0, 255)),
        ..IrDocumentState::default()
    };
    let value = serde_json::to_value(&explicit).expect("serialize explicit state");
    assert!(value.get("page_border_widths").is_some());
    assert!(value.get("page_border_color").is_some());
    assert!(value.get("page_background").is_some());

    let restored: IrDocumentState =
        serde_json::from_value(value).expect("round trip explicit state");
    assert_eq!(restored, explicit);
}
