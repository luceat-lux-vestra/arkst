fn accepts_core_evaluator(_: scribium_core::evaluator::Evaluator) {}
fn accepts_engine_evaluator(_: scribium_engine::evaluator::Evaluator) {}
fn accepts_core_capabilities(_: scribium_core::Capabilities) {}
fn accepts_engine_capabilities(_: scribium_engine::Capabilities) {}
fn accepts_core_capability(_: scribium_core::Capability) {}
fn accepts_engine_capability(_: scribium_engine::Capability) {}

type LegacyAstToIr = fn(
    &scribium_markdown::ast::Document,
    scribium_source::SourceId,
    &scribium_core::ProjectMetadata,
) -> (
    scribium_core::ir::IrDocument,
    Vec<scribium_core::Diagnostic>,
);

#[test]
fn core_engine_facades_preserve_physical_type_identity() {
    let _: fn(scribium_core::evaluator::Evaluator) = accepts_engine_evaluator;
    let _: fn(scribium_engine::evaluator::Evaluator) = accepts_core_evaluator;
    let _: fn(scribium_core::Capabilities) = accepts_engine_capabilities;
    let _: fn(scribium_engine::Capabilities) = accepts_core_capabilities;
    let _: fn(scribium_core::Capability) = accepts_engine_capability;
    let _: fn(scribium_engine::Capability) = accepts_core_capability;
}

#[test]
fn core_evaluator_facade_exposes_the_engine_constructor() {
    let evaluator =
        scribium_core::evaluator::Evaluator::with_capabilities(scribium_core::Capabilities::none());
    let _ = evaluator;
}

#[test]
fn core_ast_to_ir_facade_preserves_the_legacy_signature() {
    let _: LegacyAstToIr = scribium_core::ast_to_ir::ast_to_ir_with_diagnostics;
}
