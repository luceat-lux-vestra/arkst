use arkst_core::ir::{IrDocumentAlignment, IrDocumentState, IrPageGeometry, IrSize, IrSizeUnit};
use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};

fn compile_source(source: &str) -> arkst_core::CompileResult {
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("entry")
        .add_source("main.qd", source)
        .expect("source")
        .build()
        .expect("project");
    compile(&project, &CompileOptions::default())
}

fn assert_geometry(
    result: &arkst_core::CompileResult,
    width: (f64, IrSizeUnit),
    height: (f64, IrSizeUnit),
) {
    let geometry = result
        .ir
        .metadata
        .document_state
        .page_geometry
        .as_ref()
        .expect("explicit complete geometry");
    assert_eq!((geometry.width.value, geometry.width.unit), width);
    assert_eq!((geometry.height.value, geometry.height.unit), height);
}

#[test]
fn complete_width_height_pageformat_commits_typed_geometry() {
    let result = compile_source(".pageformat width:{10in} height:{5in}\n");
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_geometry(&result, (10.0, IrSizeUnit::In), (5.0, IrSizeUnit::In));
    assert!(result.ir.nodes.is_empty(), "setter must not emit content");
}

#[test]
fn later_complete_geometry_replaces_the_previous_pair() {
    let result = compile_source(
        ".pageformat width:{10in} height:{5in}\n.pageformat width:{8in} height:{4in}\n",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_geometry(&result, (8.0, IrSizeUnit::In), (4.0, IrSizeUnit::In));
}

#[test]
fn geometry_and_alignment_can_commit_atomically_in_one_bounded_call() {
    let result = compile_source(".pageformat width:{10in} height:{5in} alignment:{end}\n");
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_geometry(&result, (10.0, IrSizeUnit::In), (5.0, IrSizeUnit::In));
    assert_eq!(
        result.ir.metadata.document_state.page_alignment,
        Some(IrDocumentAlignment::End)
    );
}

#[test]
fn unsupported_partial_or_nullable_geometry_does_not_mutate_the_bounded_state() {
    for unsupported in [
        ".pageformat width:{8in}\n",
        ".pageformat height:{4in}\n",
        ".pageformat width:{none} height:{4in}\n",
        ".pageformat width:{8in} height:{none}\n",
        ".pageformat size:{a4} width:{8in} height:{4in}\n",
    ] {
        let result = compile_source(&format!(
            ".pageformat width:{{10in}} height:{{5in}}\n{unsupported}"
        ));
        assert_geometry(&result, (10.0, IrSizeUnit::In), (5.0, IrSizeUnit::In));
    }
}

#[test]
fn failed_geometry_conversion_rolls_back_nested_document_state_writes() {
    let result = compile_source(
        ".autopagebreak maxdepth:{3}\n.pageformat width:{10in} height:{5in}\n.function {badheight}\n    .autopagebreak maxdepth:{1}\n    invalid\n\n.pageformat width:{8in} height:{.badheight}\n",
    );
    assert!(
        !result.diagnostics.is_empty(),
        "invalid size conversion must fail"
    );
    assert_eq!(
        result.ir.metadata.document_state.auto_page_break_max_depth,
        Some(3),
        "outer pageformat failure must roll back nested document-state mutation"
    );
    assert_geometry(&result, (10.0, IrSizeUnit::In), (5.0, IrSizeUnit::In));
}

#[test]
fn page_geometry_wire_is_backward_compatible_and_explicit_when_set() {
    let state = IrDocumentState::default();
    let legacy_shape = serde_json::to_value(&state).expect("serialize default state");
    assert!(legacy_shape.get("page_geometry").is_none());
    let restored: IrDocumentState =
        serde_json::from_value(legacy_shape).expect("deserialize legacy-shaped state");
    assert!(restored.page_geometry.is_none());

    let explicit = IrDocumentState {
        page_geometry: Some(IrPageGeometry {
            width: IrSize {
                value: 10.0,
                unit: IrSizeUnit::In,
            },
            height: IrSize {
                value: 5.0,
                unit: IrSizeUnit::In,
            },
        }),
        ..IrDocumentState::default()
    };
    let value = serde_json::to_value(&explicit).expect("serialize explicit state");
    assert!(value.get("page_geometry").is_some());
    let restored: IrDocumentState =
        serde_json::from_value(value).expect("round trip explicit state");
    assert_eq!(restored.page_geometry, explicit.page_geometry);
}
