use std::fs;
use std::process::{Command, Output};

use tempfile::tempdir;

fn run(args: &[&std::ffi::OsStr]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_arkst"));
    command.args(args);
    command.output().expect("arkst process must run")
}

fn os(value: &str) -> &std::ffi::OsStr {
    std::ffi::OsStr::new(value)
}

#[test]
fn build_writes_log_messages_to_stdout_in_evaluation_order() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("main.qd");
    fs::write(
        &input,
        ".log {first}\n.log {42}\n.debug {hidden}\nvisible body\n",
    )
    .unwrap();

    let result = run(&[os("build"), input.as_os_str(), os("--format"), os("typst")]);

    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&result.stdout), "first\n42\n");

    let typst = fs::read_to_string(dir.path().join("main.typ")).unwrap();
    assert!(typst.contains("visible body"), "{typst}");
    assert!(!typst.contains("first"), "{typst}");
    assert!(!typst.contains("hidden"), "{typst}");
}

#[test]
fn build_preserves_prior_log_before_later_diagnostic_failure_without_artifact() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("main.qd");
    fs::write(&input, ".log {before-error}\n.error {boom}\nafter\n").unwrap();

    let result = run(&[os("build"), input.as_os_str(), os("--format"), os("typst")]);

    assert!(!result.status.success());
    assert_eq!(String::from_utf8_lossy(&result.stdout), "before-error\n");
    assert!(
        !dir.path().join("main.typ").exists(),
        "diagnostic failure must stop before artifact publication"
    );
}

#[test]
fn check_remains_fail_closed_without_native_logger_authority() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("main.qd");
    fs::write(&input, ".log {build-only}\n").unwrap();

    let result = run(&[os("check"), input.as_os_str()]);

    assert!(!result.status.success());
    assert!(result.stdout.is_empty(), "{:?}", result.stdout);
    assert!(String::from_utf8_lossy(&result.stderr).contains("E3010"));
}

#[test]
fn inspect_keeps_machine_output_unpolluted_by_unevidenced_logger_authority() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("main.qd");
    fs::write(&input, ".log {must-not-prefix-ir}\n").unwrap();

    let result = run(&[os("inspect"), input.as_os_str(), os("--emit"), os("ir")]);

    assert!(!result.status.success());
    assert!(result.stdout.is_empty(), "{:?}", result.stdout);
}

#[test]
fn malformed_log_emits_no_stdout_before_cli_failure() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("main.qd");
    fs::write(&input, ".log\n").unwrap();

    let result = run(&[os("check"), input.as_os_str()]);

    assert!(!result.status.success());
    assert!(result.stdout.is_empty(), "{:?}", result.stdout);
}
