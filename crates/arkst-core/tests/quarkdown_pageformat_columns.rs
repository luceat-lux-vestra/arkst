//! Independent regression coverage for the bounded #175 global `.pageformat columns` state slice.

use arkst_core::ir::IrDocumentState;
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
fn positive_pageformat_columns_commits_document_wide_state_without_output() {
    let result = compile_source(".pageformat columns:{2}\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert!(result.ir.nodes.is_empty(), "setter must not emit content");
    assert_eq!(result.ir.metadata.document_state.page_columns, Some(2));
}

#[test]
fn later_positive_pageformat_columns_replaces_the_previous_global_value() {
    let result = compile_source(".pageformat columns:{2}\n.pageformat columns:{4}\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(result.ir.metadata.document_state.page_columns, Some(4));
}

#[test]
fn non_positive_and_none_columns_are_discarded_without_erasing_prior_state() {
    for source in [
        ".pageformat columns:{2}\n.pageformat columns:{0}\n",
        ".pageformat columns:{2}\n.pageformat columns:{.none}\n",
    ] {
        let result = compile_source(source);
        assert!(result.diagnostics.is_empty(), "{source}: {result:?}");
        assert_eq!(
            result.ir.metadata.document_state.page_columns,
            Some(2),
            "discarded columns input changed effective state: {source}"
        );
    }
}

#[test]
fn invalid_or_fractional_columns_fail_without_replacing_committed_state() {
    for invalid in ["invalid", "1.5"] {
        let result = compile_source(&format!(
            ".pageformat columns:{{2}}\n.pageformat columns:{{{invalid}}}\n"
        ));
        assert!(
            !result.diagnostics.is_empty(),
            "invalid columns unexpectedly succeeded: {invalid}"
        );
        assert_eq!(
            result.ir.metadata.document_state.page_columns,
            Some(2),
            "failed columns conversion changed committed state: {invalid}"
        );
    }
}

#[test]
fn failed_columns_conversion_rolls_back_nested_document_state_writes() {
    let result = compile_source(
        ".autopagebreak maxdepth:{3}\n.function {badcolumns}\n    .autopagebreak maxdepth:{1}\n    invalid\n\n.pageformat columns:{.badcolumns}\n",
    );
    assert!(!result.diagnostics.is_empty(), "invalid columns must fail");
    assert_eq!(
        result.ir.metadata.document_state.auto_page_break_max_depth,
        Some(3),
        "outer pageformat failure leaked nested document-state mutation"
    );
    assert_eq!(result.ir.metadata.document_state.page_columns, None);
}

#[test]
fn page_columns_wire_is_backward_compatible_and_explicit_when_set() {
    let state = IrDocumentState::default();
    let legacy_shape = serde_json::to_value(&state).expect("serialize default state");
    assert!(legacy_shape.get("page_columns").is_none());
    let restored: IrDocumentState =
        serde_json::from_value(legacy_shape).expect("deserialize legacy-shaped state");
    assert_eq!(restored.page_columns, None);

    let explicit = IrDocumentState {
        page_columns: Some(3),
        ..IrDocumentState::default()
    };
    let value = serde_json::to_value(&explicit).expect("serialize explicit state");
    assert_eq!(value.get("page_columns"), Some(&serde_json::json!(3)));
    let restored: IrDocumentState =
        serde_json::from_value(value).expect("round trip explicit state");
    assert_eq!(restored.page_columns, Some(3));
}
