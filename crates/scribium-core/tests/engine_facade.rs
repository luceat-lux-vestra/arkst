fn accepts_core_capabilities(_: scribium_core::Capabilities) {}
fn accepts_engine_capabilities(_: scribium_engine::Capabilities) {}
fn accepts_core_capability(_: scribium_core::Capability) {}
fn accepts_engine_capability(_: scribium_engine::Capability) {}

fn assert_evaluator_traits<T: std::fmt::Debug + Clone + Copy + Default>() {}

type LegacyAstToIr = fn(
    &scribium_markdown::ast::Document,
    scribium_source::SourceId,
    &scribium_core::ProjectMetadata,
) -> (
    scribium_core::ir::IrDocument,
    Vec<scribium_core::Diagnostic>,
);

#[test]
fn core_capability_facades_preserve_engine_type_identity() {
    let _: fn(scribium_core::Capabilities) = accepts_engine_capabilities;
    let _: fn(scribium_engine::Capabilities) = accepts_core_capabilities;
    let _: fn(scribium_core::Capability) = accepts_engine_capability;
    let _: fn(scribium_engine::Capability) = accepts_core_capability;
}

#[test]
fn core_evaluator_facade_exposes_the_engine_constructor() {
    assert_evaluator_traits::<scribium_core::evaluator::Evaluator>();
    let evaluator =
        scribium_core::evaluator::Evaluator::with_capabilities(scribium_core::Capabilities::none());
    let copied = evaluator;
    let _ = format!("{copied:?}");
    let _ = scribium_core::evaluator::Evaluator::default();
}

#[test]
fn core_evaluator_preserves_legacy_project_evaluation_api() {
    let project = scribium_core::VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("valid entry")
        .add_source("main.qd", "")
        .expect("valid source")
        .build()
        .expect("valid project");
    let (_, source_id) = project
        .sources()
        .get_with_id(project.entry())
        .expect("entry source and identity");
    let document = scribium_core::ir::IrDocument {
        nodes: Vec::new(),
        metadata: scribium_core::ir::IrMetadata::default(),
    };
    let evaluator = scribium_core::evaluator::Evaluator::new();
    let (evaluated, diagnostics) = evaluator.evaluate_project(&project, source_id, &document);

    assert_eq!(evaluated, document);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn core_ast_to_ir_facade_preserves_the_legacy_signature() {
    let _: LegacyAstToIr = scribium_core::ast_to_ir::ast_to_ir_with_diagnostics;
}
