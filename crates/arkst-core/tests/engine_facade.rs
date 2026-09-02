fn accepts_core_capabilities(_: arkst_core::Capabilities) {}
fn accepts_engine_capabilities(_: arkst_engine::Capabilities) {}
fn accepts_core_capability(_: arkst_core::Capability) {}
fn accepts_engine_capability(_: arkst_engine::Capability) {}

fn assert_evaluator_traits<T: std::fmt::Debug + Clone + Copy + Default>() {}

type LegacyAstToIr = fn(
    &arkst_markdown::ast::Document,
    arkst_source::SourceId,
    &arkst_core::ProjectMetadata,
) -> (arkst_core::ir::IrDocument, Vec<arkst_core::Diagnostic>);

#[test]
fn core_capability_facades_preserve_engine_type_identity() {
    let _: fn(arkst_core::Capabilities) = accepts_engine_capabilities;
    let _: fn(arkst_engine::Capabilities) = accepts_core_capabilities;
    let _: fn(arkst_core::Capability) = accepts_engine_capability;
    let _: fn(arkst_engine::Capability) = accepts_core_capability;
}

#[test]
fn core_evaluator_facade_exposes_the_engine_constructor() {
    assert_evaluator_traits::<arkst_core::evaluator::Evaluator>();
    let evaluator =
        arkst_core::evaluator::Evaluator::with_capabilities(arkst_core::Capabilities::none());
    let copied = evaluator;
    let _ = format!("{copied:?}");
    let _ = arkst_core::evaluator::Evaluator::default();
}

#[test]
fn core_evaluator_preserves_legacy_project_evaluation_api() {
    let project = arkst_core::VirtualProjectBuilder::new()
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
    let document = arkst_core::ir::IrDocument {
        nodes: Vec::new(),
        metadata: arkst_core::ir::IrMetadata::default(),
    };
    let evaluator = arkst_core::evaluator::Evaluator::new();
    let (evaluated, diagnostics) = evaluator.evaluate_project(&project, source_id, &document);

    assert_eq!(evaluated, document);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn core_ast_to_ir_facade_preserves_the_legacy_signature() {
    let _: LegacyAstToIr = arkst_core::ast_to_ir::ast_to_ir_with_diagnostics;
}
