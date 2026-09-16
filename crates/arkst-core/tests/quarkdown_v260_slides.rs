use arkst_core::ir::{IrDocumentState, IrSlidesConfiguration};
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

fn center(result: &arkst_core::CompileResult) -> Option<bool> {
    result
        .ir
        .metadata
        .document_state
        .slides
        .and_then(|slides| slides.center)
}

#[test]
fn slides_configuration_is_document_type_gated_and_nullable() {
    let omitted = compile_source(".doctype {slides}\n.slides\n");
    assert!(omitted.diagnostics.is_empty(), "{:?}", omitted.diagnostics);
    assert_eq!(
        omitted.ir.metadata.document_state.slides,
        Some(IrSlidesConfiguration { center: None })
    );
    assert!(omitted.ir.nodes.is_empty(), "setter must not emit content");

    let paged = compile_source(".slides center:{true}\n");
    assert!(
        !paged.diagnostics.is_empty(),
        "slides configuration outside a slides document must fail"
    );
    assert!(paged.ir.metadata.document_state.slides.is_none());
}

#[test]
fn slides_center_boolean_values_commit_typed_state() {
    let centered = compile_source(".doctype {slides}\n.slides center:{true}\n");
    assert!(centered.diagnostics.is_empty(), "{:?}", centered.diagnostics);
    assert_eq!(center(&centered), Some(true));

    let top = compile_source(".doctype {slides}\n.slides center:{false}\n");
    assert!(top.diagnostics.is_empty(), "{:?}", top.diagnostics);
    assert_eq!(center(&top), Some(false));
}

#[test]
fn invalid_center_preserves_last_committed_slides_configuration() {
    let result = compile_source(
        ".doctype {slides}\n.slides center:{true}\n.slides center:{INVALID}\n",
    );
    assert!(
        !result.diagnostics.is_empty(),
        "invalid boolean conversion must fail"
    );
    assert_eq!(center(&result), Some(true));
}

#[test]
fn failed_outer_slides_conversion_rolls_back_nested_document_state_writes() {
    let result = compile_source(
        ".doctype {slides}\n.autopagebreak maxdepth:{3}\n.function {badcenter}\n    .autopagebreak maxdepth:{1}\n    invalid\n\n.slides center:{.badcenter}\n",
    );
    assert!(
        !result.diagnostics.is_empty(),
        "nested non-boolean center value must fail"
    );
    assert_eq!(
        result.ir.metadata.document_state.auto_page_break_max_depth,
        Some(3),
        "outer slides failure must roll back nested document-state mutation"
    );
    assert!(result.ir.metadata.document_state.slides.is_none());
}

#[test]
fn source_defined_slides_shadows_the_native_setter() {
    let result = compile_source(
        ".doctype {slides}\n.function {slides}\n    center:\n    SHADOW-SLIDES\n\n.slides center:{true}\n",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert!(result.ir.metadata.document_state.slides.is_none());
    assert!(
        format!("{:?}", result.ir).contains("SHADOW-SLIDES"),
        "source-defined function must retain ownership of the name"
    );
}

#[test]
fn slides_wire_is_backward_compatible_and_explicit_when_set() {
    let state = IrDocumentState::default();
    let legacy_shape = serde_json::to_value(&state).expect("serialize default state");
    assert!(legacy_shape.get("slides").is_none());
    let restored: IrDocumentState =
        serde_json::from_value(legacy_shape).expect("deserialize legacy-shaped state");
    assert!(restored.slides.is_none());

    let explicit = IrDocumentState {
        slides: Some(IrSlidesConfiguration {
            center: Some(true),
        }),
        ..IrDocumentState::default()
    };
    let value = serde_json::to_value(&explicit).expect("serialize explicit state");
    assert_eq!(
        value.get("slides"),
        Some(&serde_json::json!({ "center": true }))
    );
    let restored: IrDocumentState =
        serde_json::from_value(value).expect("round trip explicit state");
    assert_eq!(restored.slides, explicit.slides);
}
