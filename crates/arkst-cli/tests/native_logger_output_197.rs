use std::fs;
use std::process::{Command, Output};
use tempfile::tempdir;

fn run_build_with_args(input: &std::path::Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_arkst"))
        .arg("build")
        .arg(input)
        .args(args)
        .output()
        .expect("arkst process must run")
}

fn run_build(input: &std::path::Path) -> Output {
    run_build_with_args(input, &[])
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
fn strict_build_finalizes_first_error_after_top_level_evaluation_and_publishes_no_artifact() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("main.qd");
    fs::write(
        &input,
        ".function {secondMessage}\n    .log {strict-second-arg-side-effect}\n    strict-second-error\n.log {strict-top-pre}\n.error {strict-first-error}\n.log {strict-top-post}\n.error {.secondMessage}\n.log {strict-after-second}\n",
    )
    .unwrap();

    let output = run_build_with_args(&input, &["--strict"]);

    assert_eq!(output.status.code(), Some(66), "{output:?}");
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout must be UTF-8"),
        "strict-top-pre\nstrict-top-post\nstrict-second-arg-side-effect\nstrict-after-second\n"
    );
    assert_eq!(
        String::from_utf8(output.stderr).expect("stderr must be UTF-8"),
        "An error occurred while in strict mode (error code 66)\n\
         Originated from function: error\n\
         java.lang.Exception: strict-first-error\n"
    );
    assert!(!dir.path().join("main.typ").exists());
}

#[test]
fn strict_build_preserves_callable_and_selected_conditional_continuation_boundaries() {
    let dir = tempdir().unwrap();

    let function_input = dir.path().join("function.qd");
    fs::write(
        &function_input,
        ".function {boom}\n    .log {strict-fn-pre}\n    .error {strict-fn-error}\n    .log {strict-fn-post}\n.log {strict-caller-pre}\n.boom\n.log {strict-caller-post}\n",
    )
    .unwrap();
    let function = run_build_with_args(&function_input, &["--strict"]);
    assert_eq!(function.status.code(), Some(66));
    assert_eq!(
        String::from_utf8(function.stdout).unwrap(),
        "strict-caller-pre\nstrict-fn-pre\nstrict-caller-post\n"
    );
    assert_eq!(
        String::from_utf8(function.stderr).unwrap(),
        "An error occurred while in strict mode (error code 66)\n\
         Originated from function: error\n\
         java.lang.Exception: strict-fn-error\n"
    );
    assert!(!dir.path().join("function.typ").exists());

    let conditional_input = dir.path().join("conditional.qd");
    fs::write(
        &conditional_input,
        ".log {strict-if-outer-pre}\n.if {true}\n    .log {strict-if-pre}\n    .error {strict-if-error}\n    .log {strict-if-post}\n.log {strict-if-outer-post}\n",
    )
    .unwrap();
    let conditional = run_build_with_args(&conditional_input, &["--strict"]);
    assert_eq!(conditional.status.code(), Some(66));
    assert_eq!(
        String::from_utf8(conditional.stdout).unwrap(),
        "strict-if-outer-pre\nstrict-if-pre\nstrict-if-outer-post\n"
    );
    assert_eq!(
        String::from_utf8(conditional.stderr).unwrap(),
        "An error occurred while in strict mode (error code 66)\n\
         Originated from function: error\n\
         java.lang.Exception: strict-if-error\n"
    );
    assert!(!dir.path().join("conditional.typ").exists());
}

#[test]
fn strict_build_keeps_unselected_error_lazy_and_succeeds_normally() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("main.qd");
    fs::write(
        &input,
        ".log {strict-lazy-pre}\n.if {false}\n    .error {strict-lazy-must-not-run}\n.log {strict-lazy-post}\n",
    )
    .unwrap();

    let output = run_build_with_args(&input, &["--strict"]);

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "strict-lazy-pre\nstrict-lazy-post\n"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("strict-lazy-must-not-run"), "{stderr}");
    assert!(dir.path().join("main.typ").exists());
}

#[test]
fn strict_build_does_not_promote_unevidenced_value_context_error_to_exit_66() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("main.qd");
    fs::write(&input, ".uppercase {.error {strict-nested-error}}\n").unwrap();

    let output = run_build_with_args(&input, &["--strict"]);

    assert!(!output.status.success());
    assert_ne!(output.status.code(), Some(66));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("strict-nested-error"), "{stderr}");
    assert!(stderr.contains("found 1 error(s)"), "{stderr}");
    assert!(!dir.path().join("main.typ").exists());
}

#[test]
fn strict_build_keeps_unevidenced_nested_output_on_ordinary_fatal_path() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("main.qd");
    fs::write(&input, ".center\n    .error {strict-nested-output}\n").unwrap();

    let output = run_build_with_args(&input, &["--strict"]);

    assert!(!output.status.success());
    assert_ne!(output.status.code(), Some(66));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("strict-nested-output"), "{stderr}");
    assert!(stderr.contains("found 1 error(s)"), "{stderr}");
    assert!(!dir.path().join("main.typ").exists());
}

#[test]
fn strict_build_does_not_hide_an_ordinary_fatal_error_behind_exit_66() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("main.qd");
    fs::write(&input, ".error {strict-paired-error}\n.log\n").unwrap();

    let output = run_build_with_args(&input, &["--strict"]);

    assert!(!output.status.success());
    assert_ne!(output.status.code(), Some(66));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("strict-paired-error"), "{stderr}");
    assert!(stderr.contains("found 1 error(s)"), "{stderr}");
    assert!(!dir.path().join("main.typ").exists());
}

#[test]
fn strict_build_fails_before_pdf_backend_or_artifact_publication() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("main.qd");
    fs::write(&input, ".error {strict-pdf-error}\n").unwrap();

    let output = run_build_with_args(
        &input,
        &[
            "--strict",
            "--format",
            "pdf",
            "--typst-path",
            "definitely-missing-typst-binary",
        ],
    );

    assert_eq!(output.status.code(), Some(66), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("java.lang.Exception: strict-pdf-error")
    );
    assert!(!dir.path().join("main.pdf").exists());
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
