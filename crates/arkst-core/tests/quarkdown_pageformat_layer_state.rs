//! Regression coverage for the bounded #175 ordered
//! `.pageformat` layer/selector-state slice.

use arkst_core::ir::{
    IrDocumentState, IrDocumentType, IrPageFormatLayer, IrPageFormatState, IrPageOrientation,
    IrPageSide, IrPageSizeFormat, IrPageSizeSelection, IrSize, IrSizeUnit,
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
fn ordered_layers_compose_global_dimensions_without_rewriting_flattened_state() {
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

    let composed = state
        .page_format
        .compose_global_page_dimensions()
        .expect("global dimension composition");
    assert_eq!(
        composed.size.expect("later global size").format,
        IrPageSizeFormat::Legal
    );
    assert!(
        composed.width.is_none() && composed.height.is_none(),
        "later standard size must clear both earlier explicit axes"
    );

    // The legacy flattened fields remain serialized for backward compatibility,
    // but ordered consumers must no longer derive precedence from them.
    assert!(state.page_geometry.is_some());
    assert_eq!(
        state.page_size.expect("flattened size").format,
        IrPageSizeFormat::Legal
    );
}

#[test]
fn selector_free_mixed_standard_size_and_axis_publish_composable_state() {
    let result = compile_source(".pageformat size:{a4} orientation:{portrait} width:{8in}\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");

    let state = &result.ir.metadata.document_state;
    assert!(
        state.page_geometry.is_none(),
        "a single explicit axis must not fabricate complete flattened geometry"
    );
    assert_eq!(
        state.page_size.expect("flattened standard size").format,
        IrPageSizeFormat::A4
    );

    let layer = state.page_format.layers.last().expect("mixed global layer");
    assert!(layer.selector.is_none());
    assert_eq!(layer.size.expect("size base").format, IrPageSizeFormat::A4);
    assert_eq!(
        layer.size.expect("size base").orientation,
        Some(IrPageOrientation::Portrait)
    );
    let width = layer.width.as_ref().expect("explicit width override");
    assert_eq!((width.value, width.unit), (8.0, IrSizeUnit::In));
    assert!(layer.height.is_none());

    let composed = state
        .page_format
        .compose_global_page_dimensions()
        .expect("mixed global composition");
    assert_eq!(
        composed.size.expect("composed standard size").format,
        IrPageSizeFormat::A4
    );
    assert_eq!(
        (
            composed.width.as_ref().expect("composed width").value,
            composed.width.as_ref().expect("composed width").unit,
        ),
        (8.0, IrSizeUnit::In)
    );
    assert!(composed.height.is_none());
}

#[test]
fn selector_free_single_axis_layers_require_existing_opposite_axis_or_standard_size_base() {
    let rejected = compile_source(".pageformat width:{8in}\n");
    assert!(
        !rejected.diagnostics.is_empty(),
        "single-axis global pageformat without an opposite-axis or standard-size base must fail closed"
    );
    assert!(rejected
        .ir
        .metadata
        .document_state
        .page_format
        .layers
        .is_empty());

    let result = compile_source(
        ".pageformat size:{a4} orientation:{portrait}\n\
         .pageformat width:{8in}\n\
         .pageformat height:{10in}\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");

    let state = &result.ir.metadata.document_state;
    assert!(
        state.page_geometry.is_none(),
        "single-axis layers must not fabricate legacy complete geometry"
    );
    assert_eq!(
        state
            .page_size
            .expect("flattened standard-size fallback")
            .format,
        IrPageSizeFormat::A4
    );

    let layers = &state.page_format.layers;
    assert_eq!(layers.len(), 3);
    assert!(layers[1].selector.is_none());
    assert_eq!(
        (
            layers[1].width.as_ref().expect("width layer").value,
            layers[1].width.as_ref().expect("width layer").unit,
        ),
        (8.0, IrSizeUnit::In)
    );
    assert!(layers[1].height.is_none());
    assert!(layers[2].width.is_none());
    assert_eq!(
        (
            layers[2].height.as_ref().expect("height layer").value,
            layers[2].height.as_ref().expect("height layer").unit,
        ),
        (10.0, IrSizeUnit::In)
    );

    let composed = state
        .page_format
        .compose_global_page_dimensions()
        .expect("global dimension composition");
    assert_eq!(
        composed.size.expect("standard-size base").format,
        IrPageSizeFormat::A4
    );
    assert_eq!(
        (
            composed.width.as_ref().expect("composed width").value,
            composed.width.as_ref().expect("composed width").unit,
        ),
        (8.0, IrSizeUnit::In)
    );
    assert_eq!(
        (
            composed.height.as_ref().expect("composed height").value,
            composed.height.as_ref().expect("composed height").unit,
        ),
        (10.0, IrSizeUnit::In)
    );
}

#[test]
fn selector_free_single_axis_layers_inherit_existing_explicit_geometry() {
    let result = compile_source(
        ".pageformat width:{10in} height:{5in}\n\
         .pageformat width:{8in}\n\
         .pageformat size:{.none} height:{4in}\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");

    let state = &result.ir.metadata.document_state;
    let legacy = state
        .page_geometry
        .as_ref()
        .expect("initial complete geometry remains legacy fallback");
    assert_eq!(
        (legacy.width.value, legacy.width.unit),
        (10.0, IrSizeUnit::In)
    );
    assert_eq!(
        (legacy.height.value, legacy.height.unit),
        (5.0, IrSizeUnit::In)
    );

    let layers = &state.page_format.layers;
    assert_eq!(layers.len(), 3);
    assert!(layers[1].size.is_none());
    assert_eq!(
        (
            layers[1]
                .width
                .as_ref()
                .expect("later width override")
                .value,
            layers[1].width.as_ref().expect("later width override").unit,
        ),
        (8.0, IrSizeUnit::In)
    );
    assert!(layers[1].height.is_none());
    assert!(
        layers[2].size.is_none(),
        "explicit nullable size must not fabricate a base"
    );
    assert!(layers[2].width.is_none());
    assert_eq!(
        (
            layers[2]
                .height
                .as_ref()
                .expect("nullable-size height override")
                .value,
            layers[2]
                .height
                .as_ref()
                .expect("nullable-size height override")
                .unit,
        ),
        (4.0, IrSizeUnit::In)
    );

    let composed = state
        .page_format
        .compose_global_page_dimensions()
        .expect("explicit geometry composition");
    assert!(composed.size.is_none());
    assert_eq!(
        (
            composed.width.as_ref().expect("composed width").value,
            composed.width.as_ref().expect("composed width").unit,
        ),
        (8.0, IrSizeUnit::In)
    );
    assert_eq!(
        (
            composed.height.as_ref().expect("composed height").value,
            composed.height.as_ref().expect("composed height").unit,
        ),
        (4.0, IrSizeUnit::In)
    );
}

#[test]
fn selector_scoped_margin_retains_side_and_pages_without_flattening_global_state() {
    let result = compile_source(
        ".pageformat margin:{1cm}\n\
         .pageformat side:{LEFT} pages:{2..5} margin:{2cm}\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");

    let state = &result.ir.metadata.document_state;
    assert_eq!(state.page_format.layers.len(), 2);

    let scoped = &state.page_format.layers[1];
    let selector = scoped.selector.expect("selector");
    assert_eq!(selector.side, Some(IrPageSide::Left));
    let pages = selector.pages.expect("finite page range");
    assert_eq!((pages.start, pages.end), (2, 5));

    let scoped_margin = scoped.margin.as_ref().expect("scoped margin");
    assert_eq!(
        (scoped_margin.top.value, scoped_margin.top.unit),
        (2.0, IrSizeUnit::Cm)
    );

    let global_margin = state.page_margin.as_ref().expect("global margin");
    assert_eq!(
        (global_margin.top.value, global_margin.top.unit),
        (1.0, IrSizeUnit::Cm)
    );
}

#[test]
fn selector_scoped_columns_publish_ordered_state_without_flattening_global_columns() {
    let result = compile_source(
        ".pageformat columns:{2}\n\
         .pageformat side:{left} pages:{2..4} columns:{3}\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");

    let state = &result.ir.metadata.document_state;
    assert_eq!(state.page_columns, Some(2));
    assert_eq!(state.page_format.layers.len(), 2);

    let scoped = &state.page_format.layers[1];
    let selector = scoped.selector.expect("selector");
    assert_eq!(selector.side, Some(IrPageSide::Left));
    let pages = selector.pages.expect("finite page range");
    assert_eq!((pages.start, pages.end), (2, 4));
    assert_eq!(scoped.columns, Some(3));

    let left_page_three = state
        .page_format
        .merge_applicable_fields_for_page(3, IrPageSide::Left)
        .expect("left page fields");
    assert_eq!(left_page_three.columns, Some(3));

    let right_page_three = state
        .page_format
        .merge_applicable_fields_for_page(3, IrPageSide::Right)
        .expect("right page fields");
    assert_eq!(
        right_page_three.columns,
        Some(2),
        "selector-scoped columns must not apply outside their selector"
    );

    let non_positive = compile_source(
        ".pageformat columns:{2}\n\
         .pageformat side:{left} columns:{0}\n",
    );
    assert!(non_positive.diagnostics.is_empty(), "{non_positive:?}");
    let non_positive_state = &non_positive.ir.metadata.document_state;
    assert_eq!(non_positive_state.page_columns, Some(2));
    assert_eq!(non_positive_state.page_format.layers.len(), 2);
    assert!(non_positive_state.page_format.layers[1].columns.is_none());
    assert_eq!(
        non_positive_state
            .page_format
            .merge_applicable_fields_for_page(1, IrPageSide::Left)
            .expect("left page fields")
            .columns,
        Some(2),
        "non-positive scoped columns must not erase the inherited global value"
    );
}

#[test]
fn selector_scoped_size_is_retained_without_replacing_flattened_global_size() {
    let result = compile_source(
        ".pageformat size:{a4}\n\
         .pageformat side:{right} size:{legal} orientation:{landscape}\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");

    let state = &result.ir.metadata.document_state;
    assert_eq!(state.page_format.layers.len(), 2);
    let scoped = &state.page_format.layers[1];
    assert_eq!(
        scoped.selector.expect("selector").side,
        Some(IrPageSide::Right)
    );
    assert_eq!(
        scoped.size.expect("scoped size").format,
        IrPageSizeFormat::Legal
    );
    assert_eq!(
        state.page_size.expect("global size").format,
        IrPageSizeFormat::A4,
        "selector-scoped size must not leak into the current global consumer"
    );
}

#[test]
fn left_open_page_selector_normalizes_to_first_page_and_preserves_scope() {
    let result = compile_source(
        ".pageformat margin:{1cm}\n\
         .pageformat side:{left} pages:{..2} margin:{2cm}\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");

    let state = &result.ir.metadata.document_state;
    assert_eq!(state.page_format.layers.len(), 2);

    let scoped = &state.page_format.layers[1];
    let selector = scoped.selector.expect("selector");
    assert_eq!(selector.side, Some(IrPageSide::Left));
    let pages = selector.pages.expect("normalized page range");
    assert_eq!((pages.start, pages.end), (1, 2));

    let page_one_left = state
        .page_format
        .merge_applicable_fields_for_page(1, IrPageSide::Left)
        .expect("left first-page fields");
    assert_eq!(
        page_one_left
            .margin
            .as_ref()
            .expect("scoped margin")
            .top
            .value,
        2.0
    );

    let page_one_right = state
        .page_format
        .merge_applicable_fields_for_page(1, IrPageSide::Right)
        .expect("right first-page fields");
    assert_eq!(
        page_one_right
            .margin
            .as_ref()
            .expect("global margin")
            .top
            .value,
        1.0,
        "combined side + left-open range must not leak across page sides"
    );

    let page_three_left = state
        .page_format
        .merge_applicable_fields_for_page(3, IrPageSide::Left)
        .expect("left third-page fields");
    assert_eq!(
        page_three_left
            .margin
            .as_ref()
            .expect("global margin")
            .top
            .value,
        1.0,
        "left-open selector must stop at its finite end"
    );
}

#[test]
fn invalid_or_unbounded_page_selectors_fail_before_layer_publication() {
    for invalid_selector in [
        "pages:{2..}",
        "pages:{..}",
        "pages:{0..2}",
        "side:{diagonal}",
    ] {
        let result = compile_source(&format!(
            ".pageformat margin:{{1cm}}\n.pageformat {invalid_selector} margin:{{2cm}}\n"
        ));
        assert!(
            !result.diagnostics.is_empty(),
            "{invalid_selector} must fail closed"
        );

        let state = &result.ir.metadata.document_state;
        assert_eq!(
            state.page_format.layers.len(),
            1,
            "{invalid_selector} published a selector layer"
        );
        let global_margin = state.page_margin.as_ref().expect("global margin");
        assert_eq!(global_margin.top.value, 1.0);
    }
}

#[test]
fn exact_selector_resolution_merges_non_null_fields_without_cross_selector_precedence() {
    let result = compile_source(
        ".pageformat margin:{4cm}\n\
         .pageformat side:{left} pages:{2..5} margin:{1cm}\n\
         .pageformat side:{right} pages:{2..5} margin:{9cm}\n\
         .pageformat side:{left} pages:{2..5} bordertop:{2pt}\n\
         .pageformat side:{left} pages:{2..5} bordercolor:{red}\n\
         .pageformat side:{left} pages:{2..5} margin:{3cm}\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");

    let state = &result.ir.metadata.document_state;
    assert_eq!(state.page_format.layers.len(), 6);

    let global = state
        .page_format
        .resolve_exact_selector(None)
        .expect("global selector group");
    assert_eq!(
        global.margin.as_ref().expect("global margin").top.value,
        4.0
    );
    assert!(global.border_widths.is_none());

    let left_selector = state.page_format.layers[1].selector;
    let right_selector = state.page_format.layers[2].selector;
    assert_eq!(
        left_selector.expect("left selector").side,
        Some(IrPageSide::Left)
    );
    assert_eq!(
        right_selector.expect("right selector").side,
        Some(IrPageSide::Right)
    );

    let left = state
        .page_format
        .resolve_exact_selector(left_selector)
        .expect("left selector group");
    assert_eq!(left.selector, left_selector);
    assert_eq!(
        left.margin.as_ref().expect("left margin").top.value,
        3.0,
        "later non-null margin must replace the earlier same-selector value"
    );
    let widths = left.border_widths.as_ref().expect("left border widths");
    assert_eq!((widths.top.value, widths.top.unit), (2.0, IrSizeUnit::Pt));
    assert_eq!(widths.right.value, 0.0);
    assert_eq!(widths.bottom.value, 0.0);
    assert_eq!(widths.left.value, 0.0);
    assert!(
        left.border_color.is_some(),
        "color-only later layer must inherit the same-selector widths"
    );

    let right = state
        .page_format
        .resolve_exact_selector(right_selector)
        .expect("right selector group");
    assert_eq!(right.selector, right_selector);
    assert_eq!(right.margin.as_ref().expect("right margin").top.value, 9.0);
    assert!(right.border_widths.is_none());
    assert!(right.border_color.is_none());
}

#[test]
fn page_applicability_filters_selectors_in_source_order_without_merging() {
    let result = compile_source(
        ".pageformat margin:{1cm}\n\
         .pageformat side:{left} margin:{2cm}\n\
         .pageformat pages:{2..4} margin:{3cm}\n\
         .pageformat side:{left} pages:{2..4} margin:{4cm}\n\
         .pageformat side:{right} pages:{2..4} margin:{5cm}\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");

    let state = &result.ir.metadata.document_state;
    assert_eq!(state.page_format.layers.len(), 5);

    let left_page_three = state
        .page_format
        .applicable_layers_for_page(3, IrPageSide::Left);
    assert_eq!(
        left_page_three
            .iter()
            .map(|layer| layer.margin.as_ref().expect("margin").top.value)
            .collect::<Vec<_>>(),
        vec![1.0, 2.0, 3.0, 4.0]
    );

    let right_page_three = state
        .page_format
        .applicable_layers_for_page(3, IrPageSide::Right);
    assert_eq!(
        right_page_three
            .iter()
            .map(|layer| layer.margin.as_ref().expect("margin").top.value)
            .collect::<Vec<_>>(),
        vec![1.0, 3.0, 5.0]
    );

    let left_page_five = state
        .page_format
        .applicable_layers_for_page(5, IrPageSide::Left);
    assert_eq!(
        left_page_five
            .iter()
            .map(|layer| layer.margin.as_ref().expect("margin").top.value)
            .collect::<Vec<_>>(),
        vec![1.0, 2.0]
    );

    assert!(
        state
            .page_format
            .applicable_layers_for_page(0, IrPageSide::Left)
            .is_empty(),
        "non-positive page numbers must fail closed"
    );
}

#[test]
fn page_field_merge_uses_source_order_across_selector_scopes() {
    let scoped = compile_source(
        ".pageformat size:{a4}\n\
         .pageformat margin:{1cm}\n\
         .pageformat side:{left} margin:{2cm}\n\
         .pageformat pages:{2..4} margin:{3cm}\n\
         .pageformat side:{left} pages:{2..4} margin:{4cm}\n",
    );
    assert!(scoped.diagnostics.is_empty(), "{scoped:?}");

    let scoped_fields = scoped
        .ir
        .metadata
        .document_state
        .page_format
        .merge_applicable_fields_for_page(3, IrPageSide::Left)
        .expect("left page fields");
    assert_eq!(
        scoped_fields.margin.as_ref().expect("margin").top.value,
        4.0,
        "latest applicable scoped margin must win"
    );
    assert_eq!(
        scoped_fields.size.expect("global size must inherit").format,
        IrPageSizeFormat::A4
    );

    let later_global = compile_source(
        ".pageformat size:{a4}\n\
         .pageformat margin:{1cm}\n\
         .pageformat side:{left} margin:{2cm}\n\
         .pageformat pages:{2..4} margin:{3cm}\n\
         .pageformat side:{left} pages:{2..4} margin:{4cm}\n\
         .pageformat margin:{6cm}\n",
    );
    assert!(later_global.diagnostics.is_empty(), "{later_global:?}");

    let later_global_fields = later_global
        .ir
        .metadata
        .document_state
        .page_format
        .merge_applicable_fields_for_page(3, IrPageSide::Left)
        .expect("left page fields");
    assert_eq!(
        later_global_fields
            .margin
            .as_ref()
            .expect("margin")
            .top
            .value,
        6.0,
        "later global layer must override an earlier matching scoped layer"
    );
    assert_eq!(
        later_global_fields
            .size
            .expect("omitted size must inherit")
            .format,
        IrPageSizeFormat::A4
    );

    assert!(
        later_global
            .ir
            .metadata
            .document_state
            .page_format
            .merge_applicable_fields_for_page(0, IrPageSide::Left)
            .is_none(),
        "non-positive page numbers must fail closed"
    );
}

#[test]
fn selector_scoped_mixed_and_single_axis_dimensions_publish_state_only() {
    let result = compile_source(
        ".pageformat size:{a4}\n\
         .pageformat width:{10in} height:{5in}\n\
         .pageformat side:{left} pages:{2..4} size:{letter} orientation:{landscape} width:{8in}\n\
         .pageformat side:{right} height:{6in}\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");

    let state = &result.ir.metadata.document_state;
    assert_eq!(state.page_format.layers.len(), 4);

    let mixed = &state.page_format.layers[2];
    let mixed_selector = mixed.selector.expect("mixed selector");
    assert_eq!(mixed_selector.side, Some(IrPageSide::Left));
    assert_eq!(mixed_selector.pages.expect("mixed finite range").start, 2);
    assert_eq!(mixed_selector.pages.expect("mixed finite range").end, 4);
    let mixed_size = mixed.size.expect("mixed size");
    assert_eq!(mixed_size.format, IrPageSizeFormat::Letter);
    assert_eq!(mixed_size.orientation, Some(IrPageOrientation::Landscape));
    let mixed_width = mixed.width.as_ref().expect("mixed width");
    assert_eq!((mixed_width.value, mixed_width.unit), (8.0, IrSizeUnit::In));
    assert!(mixed.height.is_none());

    let partial = &state.page_format.layers[3];
    assert_eq!(
        partial.selector.expect("partial selector").side,
        Some(IrPageSide::Right)
    );
    assert!(partial.width.is_none());
    let partial_height = partial.height.as_ref().expect("partial height");
    assert_eq!(
        (partial_height.value, partial_height.unit),
        (6.0, IrSizeUnit::In)
    );

    assert_eq!(
        state.page_size.expect("flattened global size").format,
        IrPageSizeFormat::A4,
        "selector-scoped mixed dimensions must not leak into flattened page size"
    );
    let geometry = state
        .page_geometry
        .as_ref()
        .expect("flattened global geometry");
    assert_eq!(
        (geometry.width.value, geometry.width.unit),
        (10.0, IrSizeUnit::In)
    );
    assert_eq!(
        (geometry.height.value, geometry.height.unit),
        (5.0, IrSizeUnit::In)
    );

    let left = state
        .page_format
        .compose_applicable_page_dimensions(3, IrPageSide::Left)
        .expect("left page dimensions");
    assert_eq!(
        left.size.expect("mixed standard base").format,
        IrPageSizeFormat::Letter
    );
    let left_width = left.width.as_ref().expect("mixed width override");
    assert_eq!((left_width.value, left_width.unit), (8.0, IrSizeUnit::In));
    assert!(
        left.height.is_none(),
        "mixed size must clear the earlier global explicit height"
    );

    let right = state
        .page_format
        .compose_applicable_page_dimensions(1, IrPageSide::Right)
        .expect("right page dimensions");
    assert_eq!(
        right.size.expect("inherited global size").format,
        IrPageSizeFormat::A4
    );
    let right_width = right.width.as_ref().expect("inherited global width");
    let right_height = right.height.as_ref().expect("scoped height override");
    assert_eq!(
        (right_width.value, right_width.unit),
        (10.0, IrSizeUnit::In)
    );
    assert_eq!(
        (right_height.value, right_height.unit),
        (6.0, IrSizeUnit::In)
    );
}

#[test]
fn page_dimension_composition_respects_layer_order_and_per_axis_overrides() {
    // Use only currently bounded source-level shapes here: standard-size-only
    // layers and complete explicit width+height pairs.
    let result = compile_source(
        ".pageformat width:{10in} height:{5in}\n\
         .pageformat pages:{2..4} size:{a4}\n\
         .pageformat side:{left} pages:{2..4} width:{8in} height:{4in}\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");

    let left = result
        .ir
        .metadata
        .document_state
        .page_format
        .compose_applicable_page_dimensions(3, IrPageSide::Left)
        .expect("left page dimensions");
    assert_eq!(
        left.size.expect("range size base").format,
        IrPageSizeFormat::A4
    );
    let width = left.width.as_ref().expect("explicit width");
    let height = left.height.as_ref().expect("explicit height");
    assert_eq!((width.value, width.unit), (8.0, IrSizeUnit::In));
    assert_eq!((height.value, height.unit), (4.0, IrSizeUnit::In));

    let right = result
        .ir
        .metadata
        .document_state
        .page_format
        .compose_applicable_page_dimensions(3, IrPageSide::Right)
        .expect("right page dimensions");
    assert_eq!(
        right.size.expect("later range size").format,
        IrPageSizeFormat::A4
    );
    assert!(
        right.width.is_none() && right.height.is_none(),
        "later size must replace both earlier explicit axes before downstream physical resolution"
    );

    // The IR composition helper is intentionally a little more general than
    // the current source-level bounded shape. Prove the per-axis rule directly
    // without claiming that mixed size+axis or single-axis source calls are
    // newly accepted by the evaluator.
    let ir_only = IrPageFormatState {
        layers: vec![
            IrPageFormatLayer {
                size: Some(IrPageSizeSelection {
                    format: IrPageSizeFormat::Letter,
                    orientation: None,
                    document_type: IrDocumentType::Plain,
                }),
                width: Some(IrSize {
                    value: 7.0,
                    unit: IrSizeUnit::In,
                }),
                ..IrPageFormatLayer::default()
            },
            IrPageFormatLayer {
                height: Some(IrSize {
                    value: 4.0,
                    unit: IrSizeUnit::In,
                }),
                ..IrPageFormatLayer::default()
            },
        ],
    };
    let dimensions = ir_only
        .compose_applicable_page_dimensions(1, IrPageSide::Right)
        .expect("IR-only composed dimensions");
    assert_eq!(
        dimensions.size.expect("standard base").format,
        IrPageSizeFormat::Letter
    );
    let width = dimensions.width.as_ref().expect("width override");
    let height = dimensions.height.as_ref().expect("height override");
    assert_eq!((width.value, width.unit), (7.0, IrSizeUnit::In));
    assert_eq!((height.value, height.unit), (4.0, IrSizeUnit::In));

    assert!(
        ir_only
            .compose_applicable_page_dimensions(0, IrPageSide::Right)
            .is_none(),
        "non-positive page numbers must fail closed"
    );
}

#[test]
fn explicit_page_resolution_combines_field_merge_with_dimension_composition() {
    let result = compile_source(
        ".pageformat size:{a4}\n\
         .pageformat width:{10in} height:{5in}\n\
         .pageformat margin:{1cm}\n\
         .pageformat columns:{2}\n\
         .pageformat side:{left} pages:{2..4} size:{letter} width:{8in}\n\
         .pageformat side:{left} pages:{2..4} margin:{3cm}\n\
         .pageformat pages:{3..3} background:{red}\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");

    let state = &result.ir.metadata.document_state.page_format;
    let left = state
        .resolve_applicable_page_format(3, IrPageSide::Left)
        .expect("left page format");

    assert_eq!(
        left.dimensions.size.expect("scoped size").format,
        IrPageSizeFormat::Letter
    );
    let left_width = left.dimensions.width.as_ref().expect("scoped width");
    assert_eq!((left_width.value, left_width.unit), (8.0, IrSizeUnit::In));
    assert!(
        left.dimensions.height.is_none(),
        "later scoped standard size must clear the earlier global height"
    );
    assert_eq!(left.margin.as_ref().expect("scoped margin").top.value, 3.0);
    assert_eq!(left.columns, Some(2));
    assert!(left.background.is_some());

    let right = state
        .resolve_applicable_page_format(3, IrPageSide::Right)
        .expect("right page format");
    assert_eq!(
        right.dimensions.size.expect("global size").format,
        IrPageSizeFormat::A4
    );
    let right_width = right.dimensions.width.as_ref().expect("global width");
    let right_height = right.dimensions.height.as_ref().expect("global height");
    assert_eq!(
        (right_width.value, right_width.unit),
        (10.0, IrSizeUnit::In)
    );
    assert_eq!(
        (right_height.value, right_height.unit),
        (5.0, IrSizeUnit::In)
    );
    assert_eq!(right.margin.as_ref().expect("global margin").top.value, 1.0);
    assert_eq!(right.columns, Some(2));
    assert!(right.background.is_some());

    assert!(
        state
            .resolve_applicable_page_format(0, IrPageSide::Left)
            .is_none(),
        "non-positive page numbers must fail closed"
    );
}

#[test]
fn resolved_page_dimensions_materialize_standard_base_without_widening_selector_output() {
    let result = compile_source(
        ".doctype {paged}\n\
         .pageformat size:{a4}\n\
         .pageformat side:{left} pages:{2..4} size:{letter} width:{8in}\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");

    let state = &result.ir.metadata.document_state.page_format;
    let left = state
        .resolve_applicable_page_format(3, IrPageSide::Left)
        .expect("left page format");
    let left_geometry = left
        .dimensions
        .resolve_concrete_page_geometry(IrDocumentType::Paged)
        .expect("left concrete geometry");
    assert_eq!(
        (left_geometry.width.value, left_geometry.width.unit),
        (8.0, IrSizeUnit::In)
    );
    assert_eq!(
        (left_geometry.height.value, left_geometry.height.unit),
        (279.4, IrSizeUnit::Mm)
    );

    let right = state
        .resolve_applicable_page_format(3, IrPageSide::Right)
        .expect("right page format");
    let right_geometry = right
        .dimensions
        .resolve_concrete_page_geometry(IrDocumentType::Paged)
        .expect("right concrete geometry");
    assert_eq!(
        (right_geometry.width.value, right_geometry.width.unit),
        (210.0, IrSizeUnit::Mm)
    );
    assert_eq!(
        (right_geometry.height.value, right_geometry.height.unit),
        (297.0, IrSizeUnit::Mm)
    );

    let docs_basis = arkst_core::ir::IrComposedPageDimensions {
        size: Some(IrPageSizeSelection {
            format: IrPageSizeFormat::A4,
            orientation: None,
            document_type: IrDocumentType::Docs,
        }),
        ..Default::default()
    };
    assert!(
        docs_basis
            .resolve_concrete_page_geometry(IrDocumentType::Paged)
            .is_none(),
        "omitted docs orientation basis must remain fail-closed"
    );

    let explicit = arkst_core::ir::IrComposedPageDimensions {
        width: Some(IrSize {
            value: 7.0,
            unit: IrSizeUnit::In,
        }),
        height: Some(IrSize {
            value: 9.0,
            unit: IrSizeUnit::In,
        }),
        ..Default::default()
    };
    let explicit_geometry = explicit
        .resolve_concrete_page_geometry(IrDocumentType::Docs)
        .expect("complete explicit geometry");
    assert_eq!(explicit_geometry.width.value, 7.0);
    assert_eq!(explicit_geometry.height.value, 9.0);
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

    let compiled = compile_source(".pageformat side:{left} pages:{2..3} margin:{1cm}\n");
    assert!(compiled.diagnostics.is_empty(), "{compiled:?}");
    let explicit = compiled.ir.metadata.document_state;
    let value = serde_json::to_value(&explicit).expect("serialize pageformat state");
    assert!(value.get("page_format").is_some());
    let restored: IrDocumentState =
        serde_json::from_value(value).expect("round trip pageformat state");
    assert_eq!(restored.page_format, explicit.page_format);
}
