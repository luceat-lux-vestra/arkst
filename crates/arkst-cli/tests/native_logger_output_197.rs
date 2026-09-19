use std::fs;
use std::process::{Command, Output};
use tempfile::tempdir;

fn run_build(input: &std::path::Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_arkst"))
        .arg("build")
        .arg(input)
        .output()
        .expect("arkst process must run")
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
    assert_eq!(
        stderr,
        "Cannot call function error(String message) with arguments (boom): boom\nWrote generated Typst to "
            .to_string()
            + &dir.path().join("main.typ").display().to_string()
            + "\n"
    );

    let typst = fs::read_to_string(dir.path().join("main.typ")).unwrap();
    assert!(typst.contains("Error: error Cannot call function error"), "{typst}");
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
