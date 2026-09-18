use arkst_core::ir::{IrInline, IrNode};
use arkst_core::{
    compile, compile_with_capabilities_and_environment, compile_with_environment, Capabilities,
    CompileOptions, EnvironmentInputs, Severity, VirtualProjectBuilder,
};

fn project(source: &str) -> arkst_core::VirtualProject {
    VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("valid entry")
        .add_source("main.qd", source)
        .expect("valid source")
        .build()
        .expect("valid project")
}

fn paragraph_texts(result: &arkst_core::CompileResult) -> Vec<String> {
    result
        .ir
        .nodes
        .iter()
        .filter_map(|node| match node {
            IrNode::Paragraph { content, .. } => Some(
                content
                    .iter()
                    .filter_map(|inline| match inline {
                        IrInline::Text { content, .. } => Some(content.as_str()),
                        _ => None,
                    })
                    .collect::<String>(),
            ),
            _ => None,
        })
        .collect()
}

#[test]
fn default_compile_denies_env_instead_of_observing_host_process_state() {
    let result = compile(&project(".env {PATH}"), &CompileOptions::default());

    assert_eq!(result.diagnostics.len(), 1, "{:?}", result.diagnostics);
    assert_eq!(result.diagnostics[0].code, "E3004");
    assert!(matches!(result.diagnostics[0].severity, Severity::Error));
    assert!(result.diagnostics[0]
        .message
        .contains("Process environment capability"));
    assert!(result.ir.nodes.is_empty(), "{:?}", result.ir.nodes);
}

#[test]
fn explicit_environment_snapshot_is_visible_and_absence_is_none() {
    let project = project(
        ".var {value} {.env {ARKST_EXPLICIT}}\nvalue:.value\nmissing:.string {.isnone {.env {ARKST_MISSING}}}",
    );
    let environment =
        EnvironmentInputs::from_iter([("ARKST_EXPLICIT".to_string(), "injected".to_string())]);

    let result = compile_with_environment(&project, &CompileOptions::default(), &environment);

    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(
        paragraph_texts(&result),
        vec!["value:injected", "missing:true"]
    );
}

#[test]
fn empty_explicit_environment_authorizes_lookup_but_returns_none() {
    let result = compile_with_environment(
        &project(".string {.isnone {.env {ARKST_MISSING}}}"),
        &CompileOptions::default(),
        &EnvironmentInputs::default(),
    );

    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(paragraph_texts(&result), vec!["true"]);
}

#[test]
fn environment_authority_is_independent_from_other_evaluator_capabilities() {
    let environment =
        EnvironmentInputs::from_iter([("ARKST_EXPLICIT".to_string(), "injected".to_string())]);
    let result = compile_with_capabilities_and_environment(
        &project(".env {ARKST_EXPLICIT}"),
        &CompileOptions::default(),
        Capabilities::none(),
        &environment,
    );

    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(paragraph_texts(&result), vec!["injected"]);
}

#[test]
fn malformed_env_call_is_a_binding_error_even_without_environment_authority() {
    let result = compile(&project(".env"), &CompileOptions::default());

    assert_eq!(result.diagnostics.len(), 1, "{:?}", result.diagnostics);
    assert_eq!(result.diagnostics[0].code, "E3001");
    assert!(!result.diagnostics[0]
        .message
        .contains("Process environment capability"));
}
