use arkst_core::ir::{IrDocumentState, IrDocumentType};
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

#[test]
fn explicit_overrides_are_final_document_state() {
    let result = compile_source(
        ".doctype {slides}\n.autopagebreak maxdepth:{3}\n# Earlier\n.noautopagebreak\n# Later\n",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(
        result.ir.metadata.document_state.document_type,
        IrDocumentType::Slides
    );
    assert_eq!(
        result.ir.metadata.document_state.auto_page_break_max_depth,
        Some(0)
    );

    let result = compile_source(
        ".autopagebreak maxdepth:{3}\n## Earlier\n.autopagebreak maxdepth:{1}\n## Later\n",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(
        result.ir.metadata.document_state.auto_page_break_max_depth,
        Some(1)
    );
}

#[test]
fn negative_depth_fails_before_mutation() {
    let result = compile_source(".autopagebreak maxdepth:{2}\n.autopagebreak maxdepth:{-1}\n");
    assert!(
        result.diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("Heading depth cannot be negative")),
        "{:?}",
        result.diagnostics
    );
    assert_eq!(
        result.ir.metadata.document_state.auto_page_break_max_depth,
        Some(2),
        "failed mutation must preserve the last committed state"
    );
}

#[test]
fn source_defined_functions_shadow_both_natives() {
    let result = compile_source(
        ".function {autopagebreak}\n    maxdepth:\n    SHADOW-AUTO\n\n.doctype {slides}\n.autopagebreak maxdepth:{0}\n",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(
        result.ir.metadata.document_state.auto_page_break_max_depth,
        None
    );
    assert!(format!("{:?}", result.ir).contains("SHADOW-AUTO"));

    let result = compile_source(
        ".function {noautopagebreak}\n    SHADOW-NO-AUTO\n\n.doctype {slides}\n.noautopagebreak\n",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(
        result.ir.metadata.document_state.auto_page_break_max_depth,
        None
    );
    assert!(format!("{:?}", result.ir).contains("SHADOW-NO-AUTO"));
}

#[test]
fn document_state_wire_is_backward_compatible_and_explicit_when_overridden() {
    let state = IrDocumentState::default();
    let legacy_shape = serde_json::to_value(&state).expect("serialize default state");
    assert!(legacy_shape.get("auto_page_break_max_depth").is_none());
    let restored: IrDocumentState =
        serde_json::from_value(legacy_shape).expect("deserialize legacy-shaped state");
    assert_eq!(restored.auto_page_break_max_depth, None);

    let explicit = IrDocumentState {
        auto_page_break_max_depth: Some(2),
        ..IrDocumentState::default()
    };
    let value = serde_json::to_value(&explicit).expect("serialize explicit state");
    assert_eq!(
        value.get("auto_page_break_max_depth"),
        Some(&serde_json::json!(2))
    );
    let restored: IrDocumentState =
        serde_json::from_value(value).expect("round trip explicit state");
    assert_eq!(restored.auto_page_break_max_depth, Some(2));
}
