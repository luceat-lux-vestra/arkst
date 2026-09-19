use std::fs;
use std::process::{Command, Output};
use tempfile::tempdir;

fn run_build(input: &std::path::Path) -> Output {
    run_build_args(input, &[])
}

fn run_build_args(input: &std::path::Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_arkst"));
    command.arg("build").arg(input);
    command.args(args);
    command.output().expect("arkst process must run")
}

#[test]
fn build_emits_log_to_stdout_in_order_and_keeps_debug_silent() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("main.qd");
    fs::write(
        &input,
        ".log {first}\n.debug {hidden}\n.log {.pair {left} {right}}\nvisible\n",
    )
    .unwrap();

    let output = run_build(&input);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout must be UTF-8");
    let pair = "[DynamicValue(unwrappedValue=left, evaluationContext=null), DynamicValue(unwrappedValue=right, evaluationContext=null)]";
    assert_eq!(stdout, format!("first\n{pair}\n"));
    assert!(!stdout.contains("hidden"));

    let typst = fs::read_to_string(dir.path().join("main.typ")).unwrap();
    assert!(typst.contains("visible"), "{typst}");
}

#[test]
fn build_keeps_prior_log_and_renders_non_strict_error_component() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("main.qd");
    fs::write(&input, ".log {before-error}\n.error {boom}\nafter\n").unwrap();

    let output = run_build(&input);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout must be UTF-8");
    assert_eq!(stdout, "before-error\n");

    let stderr = String::from_utf8_lossy(&output.stderr);
    let resolved_output = dir.path().canonicalize().unwrap().join("main.typ");
    assert_eq!(
        stderr,
        "Cannot call function error(String message) with arguments (boom): boom\nWrote generated Typst to "
            .to_string()
            + &resolved_output.display().to_string()
            + "\n"
    );

    let typst = fs::read_to_string(dir.path().join("main.typ")).unwrap();
    assert!(
        typst.contains("Error: error Cannot call function error"),
        "{typst}"
    );
    assert!(typst.contains("boom"), "{typst}");
    assert!(typst.contains("after"), "{typst}");
}

#[test]
fn build_renders_error_component_returned_through_source_defined_function() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("main.qd");
    fs::write(
        &input,
        ".function {boom}\n    .error {function-error}\nbefore\n.boom\nafter\n",
    )
    .unwrap();

    let output = run_build(&input);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let typst = fs::read_to_string(dir.path().join("main.typ")).unwrap();
    let before = typst.find("before").expect("before");
    let error = typst.find("function-error").expect("error component");
    let after = typst.rfind("after").expect("after");
    assert!(before < error && error < after, "{typst}");
}

#[test]
fn strict_build_reports_first_error_after_preserving_top_level_side_effects() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("main.qd");
    fs::write(
        &input,
        ".function {secondMessage}\n    .log {second-arg-side-effect}\n    second-error\n.log {top-pre}\n.error {first-error}\n.log {top-post}\n.error {.secondMessage}\n.log {after-second}\n",
    )
    .unwrap();

    let output = run_build_args(&input, &["--strict"]);

    assert_eq!(output.status.code(), Some(66));
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout must be UTF-8"),
        "top-pre\ntop-post\nsecond-arg-side-effect\nafter-second\n"
    );
    assert_eq!(
        String::from_utf8(output.stderr).expect("stderr must be UTF-8"),
        "An error occurred while in strict mode (error code 66)\nOriginated from function: error\njava.lang.Exception: first-error\n"
    );
    assert!(!dir.path().join("main.typ").exists());
}

#[test]
fn strict_build_suppresses_function_tail_but_continues_caller() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("main.qd");
    fs::write(
        &input,
        ".function {boom}\n    .log {fn-pre}\n    .error {function-error}\n    .log {fn-post}\n.log {caller-pre}\n.boom\n.log {caller-post}\n",
    )
    .unwrap();

    let output = run_build_args(&input, &["--strict"]);

    assert_eq!(output.status.code(), Some(66));
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout must be UTF-8"),
        "caller-pre\nfn-pre\ncaller-post\n"
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr must be UTF-8");
    assert!(stderr.contains("java.lang.Exception: function-error"), "{stderr}");
    assert!(!dir.path().join("main.typ").exists());
}

#[test]
fn strict_build_suppresses_selected_conditional_tail_but_continues_outer_scope() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("main.qd");
    fs::write(
        &input,
        ".log {outer-pre}\n.if {true}\n    .log {if-pre}\n    .error {conditional-error}\n    .log {if-post}\n.log {outer-post}\n",
    )
    .unwrap();

    let output = run_build_args(&input, &["--strict"]);

    assert_eq!(output.status.code(), Some(66));
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout must be UTF-8"),
        "outer-pre\nif-pre\nouter-post\n"
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr must be UTF-8");
    assert!(stderr.contains("java.lang.Exception: conditional-error"), "{stderr}");
    assert!(!dir.path().join("main.typ").exists());
}

#[test]
fn strict_build_keeps_unselected_conditional_lazy_and_publishes_normally() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("main.qd");
    fs::write(
        &input,
        ".log {lazy-pre}\n.if {false}\n    .error {must-not-run}\n.log {lazy-post}\nvisible\n",
    )
    .unwrap();

    let output = run_build_args(&input, &["--strict"]);

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout must be UTF-8"),
        "lazy-pre\nlazy-post\n"
    );
    let typst = fs::read_to_string(dir.path().join("main.typ")).unwrap();
    assert!(typst.contains("visible"), "{typst}");
    assert!(!typst.contains("must-not-run"), "{typst}");
}

#[test]
fn strict_build_returns_before_pdf_backend_or_artifact_publication() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("main.qd");
    fs::write(&input, ".error {pdf-strict}\n").unwrap();

    let output = run_build_args(
        &input,
        &[
            "--strict",
            "--format",
            "pdf",
            "--typst-path",
            "definitely-missing-typst",
        ],
    );

    assert_eq!(output.status.code(), Some(66));
    let stderr = String::from_utf8(output.stderr).expect("stderr must be UTF-8");
    assert!(stderr.contains("java.lang.Exception: pdf-strict"), "{stderr}");
    assert!(!stderr.contains("PDF compilation failed"), "{stderr}");
    assert!(!dir.path().join("main.pdf").exists());
}

#[test]
fn strict_build_does_not_promote_unmaterialized_explicit_error_to_exit_66() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("main.qd");
    fs::write(&input, ".uppercase {.error {nested-strict-error}}\n").unwrap();

    let output = run_build_args(&input, &["--strict"]);

    assert_ne!(output.status.code(), Some(66));
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("nested-strict-error"), "{stderr}");
    assert!(stderr.contains("found 1 error(s)"), "{stderr}");
    assert!(!dir.path().join("main.typ").exists());
}

#[test]
fn unmaterialized_explicit_error_stays_build_fatal() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("main.qd");
    fs::write(&input, ".uppercase {.error {nested-error}}\n").unwrap();

    let output = run_build(&input);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("nested-error"), "{stderr}");
    assert!(stderr.contains("found 1 error(s)"), "{stderr}");
    assert!(
        !dir.path().join("main.typ").exists(),
        "an explicit error without a rendered component must not publish an artifact"
    );
}

#[test]
fn explicit_error_inside_unevidenced_wrapper_stays_build_fatal() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("main.qd");
    fs::write(&input, ".center\n    .error {nested-output}\n").unwrap();

    let output = run_build(&input);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("nested-output"), "{stderr}");
    assert!(stderr.contains("found 1 error(s)"), "{stderr}");
    assert!(
        !dir.path().join("main.typ").exists(),
        "unevidenced nested error output must not publish an artifact"
    );
}

#[test]
fn non_explicit_error_diagnostics_still_abort_build_without_artifact() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("main.qd");
    fs::write(&input, ".log\n").unwrap();

    let output = run_build(&input);

    assert!(!output.status.success());
    assert!(
        !dir.path().join("main.typ").exists(),
        "ordinary diagnostics must remain build-fatal"
    );
}
