fn accepts_root_diagnostic(_: arkst_core::Diagnostic) {}
fn accepts_module_diagnostic(_: arkst_core::diagnostics::Diagnostic) {}
fn accepts_root_severity(_: arkst_core::Severity) {}
fn accepts_module_severity(_: arkst_core::diagnostics::Severity) {}

#[test]
fn diagnostic_and_severity_facades_remain_the_same_types() {
    let _: fn(arkst_core::Diagnostic) = accepts_module_diagnostic;
    let _: fn(arkst_core::diagnostics::Diagnostic) = accepts_root_diagnostic;
    let _: fn(arkst_core::Severity) = accepts_module_severity;
    let _: fn(arkst_core::diagnostics::Severity) = accepts_root_severity;
}
