use std::fs;
use std::process::{Command, Output};
use tempfile::tempdir;

fn run(command: &str, input: &std::path::Path, args: &[&str]) -> Output {
    let mut process = Command::new(env!("CARGO_BIN_EXE_arkst"));
    process.arg(command).arg(input).args(args);
    process.output().expect("arkst process must run")
}

#[test]
fn check_keeps_log_sinkless_and_fail_closed() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("main.qd");
    fs::write(&input, ".log {analysis-log}\n").unwrap();

    let output = run("check", &input, &[]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("E3010"), "{stderr}");
    assert!(stderr.contains("found 1 error(s)"), "{stderr}");
    assert!(
        !stderr.lines().any(|line| line == "analysis-log"),
        "{stderr}"
    );
}

#[test]
fn check_and_inspect_keep_explicit_error_fatal_and_non_strict() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("main.qd");
    fs::write(&input, ".error {analysis-error}\n").unwrap();

    for (command, args) in [("check", &[][..]), ("inspect", &["--emit", "typst"][..])] {
        let output = run(command, &input, args);

        assert!(!output.status.success(), "{command} unexpectedly succeeded");
        assert_ne!(
            output.status.code(),
            Some(66),
            "{command} must not use build strict exit"
        );
        assert!(
            output.stdout.is_empty(),
            "{command} emitted analysis output"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("E3011"), "{command}: {stderr}");
        assert!(stderr.contains("analysis-error"), "{command}: {stderr}");
        assert!(stderr.contains("found 1 error(s)"), "{command}: {stderr}");
    }
}

#[test]
fn inspect_exposes_log_capability_failure_before_refusing_output() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("main.qd");
    fs::write(&input, ".log {inspect-log}\n").unwrap();

    let output = run("inspect", &input, &["--emit", "ir"]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("E3010"), "{stderr}");
    assert!(stderr.contains("found 1 error(s)"), "{stderr}");
}

#[test]
fn debug_stays_silent_in_analysis_commands() {
    let dir = tempdir().unwrap();

    let check_input = dir.path().join("check.qd");
    fs::write(&check_input, ".debug {hidden-check}\n").unwrap();
    let check = run("check", &check_input, &[]);
    assert!(
        check.status.success(),
        "{}",
        String::from_utf8_lossy(&check.stderr)
    );
    assert!(check.stdout.is_empty());
    assert!(check.stderr.is_empty());

    let inspect_input = dir.path().join("inspect.qd");
    fs::write(&inspect_input, ".debug {hidden-inspect}\nvisible\n").unwrap();
    let inspect = run("inspect", &inspect_input, &["--emit", "typst"]);
    assert!(
        inspect.status.success(),
        "{}",
        String::from_utf8_lossy(&inspect.stderr)
    );
    assert!(inspect.stderr.is_empty());
    let stdout = String::from_utf8(inspect.stdout).expect("stdout must be UTF-8");
    assert!(stdout.contains("visible"), "{stdout}");
    assert!(!stdout.contains("hidden-inspect"), "{stdout}");
}
