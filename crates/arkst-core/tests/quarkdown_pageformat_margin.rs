//! Independent regression coverage for the bounded #175 selector-free
//! `.pageformat margin:{...}` document-state slice.

use arkst_core::ir::{IrDocumentState, IrPageMargins, IrSize, IrSizeUnit};
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

fn size(value: f64, unit: IrSizeUnit) -> IrSize {
    IrSize { value, unit }
}

fn margins(top: IrSize, right: IrSize, bottom: IrSize, left: IrSize) -> IrPageMargins {
    IrPageMargins {
        top,
        right,
        bottom,
        left,
    }
}

#[test]
fn one_two_and_four_value_margin_groups_expand_to_explicit_sides() {
    let single = compile_source(".pageformat margin:{1cm}\n");
    assert!(single.diagnostics.is_empty(), "{single:?}");
    assert_eq!(
        single.ir.metadata.document_state.page_margin,
        Some(margins(
            size(1.0, IrSizeUnit::Cm),
            size(1.0, IrSizeUnit::Cm),
            size(1.0, IrSizeUnit::Cm),
            size(1.0, IrSizeUnit::Cm),
        ))
    );
    assert!(single.ir.nodes.is_empty(), "setter must not emit content");

    let two = compile_source(".pageformat margin:{2cm 15mm}\n");
    assert!(two.diagnostics.is_empty(), "{two:?}");
    assert_eq!(
        two.ir.metadata.document_state.page_margin,
        Some(margins(
            size(2.0, IrSizeUnit::Cm),
            size(15.0, IrSizeUnit::Mm),
            size(2.0, IrSizeUnit::Cm),
            size(15.0, IrSizeUnit::Mm),
        ))
    );

    let four = compile_source(".pageformat margin:{2in 1in 3in 2in}\n");
    assert!(four.diagnostics.is_empty(), "{four:?}");
    assert_eq!(
        four.ir.metadata.document_state.page_margin,
        Some(margins(
            size(2.0, IrSizeUnit::In),
            size(1.0, IrSizeUnit::In),
            size(3.0, IrSizeUnit::In),
            size(2.0, IrSizeUnit::In),
        ))
    );
}

#[test]
fn unitless_single_margin_uses_the_existing_size_px_default() {
    let result = compile_source(".pageformat margin:{10}\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(
        result.ir.metadata.document_state.page_margin,
        Some(margins(
            size(10.0, IrSizeUnit::Px),
            size(10.0, IrSizeUnit::Px),
            size(10.0, IrSizeUnit::Px),
            size(10.0, IrSizeUnit::Px),
        ))
    );
}

#[test]
fn later_global_margin_replaces_the_previous_expanded_state() {
    let result =
        compile_source(".pageformat margin:{1cm 2cm 3cm 4cm}\n.pageformat margin:{5mm 6mm}\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(
        result.ir.metadata.document_state.page_margin,
        Some(margins(
            size(5.0, IrSizeUnit::Mm),
            size(6.0, IrSizeUnit::Mm),
            size(5.0, IrSizeUnit::Mm),
            size(6.0, IrSizeUnit::Mm),
        ))
    );
}

#[test]
fn semantic_none_preserves_the_previous_global_margin() {
    let result = compile_source(".pageformat margin:{1cm 2cm}\n.pageformat margin:{.none}\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(
        result.ir.metadata.document_state.page_margin,
        Some(margins(
            size(1.0, IrSizeUnit::Cm),
            size(2.0, IrSizeUnit::Cm),
            size(1.0, IrSizeUnit::Cm),
            size(2.0, IrSizeUnit::Cm),
        ))
    );
}

#[test]
fn invalid_size_groups_fail_without_replacing_committed_margin() {
    for invalid in ["1cm 2cm 3cm", "1cm nope", "1cm 2cm 3cm 4cm 5cm"] {
        let result = compile_source(&format!(
            ".pageformat margin:{{1cm}}\n.pageformat margin:{{{invalid}}}\n"
        ));
        assert!(!result.diagnostics.is_empty(), "{invalid}: {result:?}");
        assert_eq!(
            result.ir.metadata.document_state.page_margin,
            Some(margins(
                size(1.0, IrSizeUnit::Cm),
                size(1.0, IrSizeUnit::Cm),
                size(1.0, IrSizeUnit::Cm),
                size(1.0, IrSizeUnit::Cm),
            )),
            "failed margin conversion replaced committed state: {invalid}"
        );
    }
}

#[test]
fn unsupported_mixed_or_selector_shapes_do_not_mutate_bounded_margin_state() {
    for unsupported in [
        ".pageformat margin:{2cm} columns:{2}\n",
        ".pageformat margin:{2cm} background:{red}\n",
        ".pageformat side:{left} margin:{2cm}\n",
        ".pageformat pages:{1..2} margin:{2cm}\n",
    ] {
        let result = compile_source(&format!(".pageformat margin:{{1cm}}\n{unsupported}"));
        assert_eq!(
            result.ir.metadata.document_state.page_margin,
            Some(margins(
                size(1.0, IrSizeUnit::Cm),
                size(1.0, IrSizeUnit::Cm),
                size(1.0, IrSizeUnit::Cm),
                size(1.0, IrSizeUnit::Cm),
            )),
            "unsupported shape mutated bounded margin state: {unsupported:?}; diagnostics={:?}; IR={:?}",
            result.diagnostics,
            result.ir
        );
    }
}

#[test]
fn failed_margin_conversion_rolls_back_nested_document_state_writes() {
    let result = compile_source(
        ".autopagebreak maxdepth:{3}\n.pageformat margin:{1cm}\n.function {badmargin}\n    .autopagebreak maxdepth:{1}\n    1cm 2cm nope 4cm\n\n.pageformat margin:{.badmargin}\n",
    );
    assert!(!result.diagnostics.is_empty(), "invalid margin must fail");
    assert_eq!(
        result.ir.metadata.document_state.auto_page_break_max_depth,
        Some(3),
        "outer pageformat failure leaked nested document-state mutation"
    );
    assert_eq!(
        result.ir.metadata.document_state.page_margin,
        Some(margins(
            size(1.0, IrSizeUnit::Cm),
            size(1.0, IrSizeUnit::Cm),
            size(1.0, IrSizeUnit::Cm),
            size(1.0, IrSizeUnit::Cm),
        ))
    );
}

#[test]
fn rich_margin_content_remains_fail_closed() {
    for source in [
        ".pageformat margin:{*1cm* 2cm}\n",
        ".pageformat margin:{`1cm` 2cm}\n",
    ] {
        let result = compile_source(source);
        assert!(
            !result.diagnostics.is_empty(),
            "rich margin content must remain unsupported: {source:?}; {result:?}"
        );
        assert_eq!(
            result.ir.metadata.document_state.page_margin, None,
            "rich margin content must not be flattened into Sizes: {source:?}"
        );
    }
}

#[test]
fn source_defined_pageformat_shadows_the_bounded_margin_builtin() {
    let result = compile_source(
        ".function {pageformat}\n    margin:\n    SHADOW-PAGEFORMAT-MARGIN\n\n.pageformat margin:{1cm}\n",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.ir.metadata.document_state.page_margin, None);
    assert!(
        format!("{:?}", result.ir).contains("SHADOW-PAGEFORMAT-MARGIN"),
        "source-defined function must retain ownership of the name"
    );
}

#[test]
fn margin_wire_is_backward_compatible_and_explicit_when_set() {
    let state = IrDocumentState::default();
    let legacy_shape = serde_json::to_value(&state).expect("serialize default state");
    assert!(legacy_shape.get("page_margin").is_none());

    let restored: IrDocumentState =
        serde_json::from_value(legacy_shape).expect("deserialize legacy-shaped state");
    assert_eq!(restored.page_margin, None);

    let explicit = IrDocumentState {
        page_margin: Some(margins(
            size(1.0, IrSizeUnit::Cm),
            size(2.0, IrSizeUnit::Cm),
            size(3.0, IrSizeUnit::Cm),
            size(4.0, IrSizeUnit::Cm),
        )),
        ..IrDocumentState::default()
    };
    let value = serde_json::to_value(&explicit).expect("serialize explicit state");
    assert!(value.get("page_margin").is_some());

    let restored: IrDocumentState =
        serde_json::from_value(value).expect("round trip explicit state");
    assert_eq!(restored, explicit);
}
