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
fn build_emits_evidenced_direct_markdown_list_logger_message() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("main.qd");
    fs::write(
        &input,
        ".var {values}\n    - alpha\n    - beta\n.log {.values}\nvisible\n",
    )
    .unwrap();

    let output = run_build(&input);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout must be UTF-8"),
        "- alpha\n- beta\n"
    );
    let typst = fs::read_to_string(dir.path().join("main.typ")).unwrap();
    assert!(typst.contains("visible"), "{typst}");
}

#[test]
fn build_preserves_direct_markdown_list_source_spelling_for_log_and_error() {
    let source = ".var {values}\n    *   alpha\n    * beta *em*\n.log {.values}\n.error {.values}\nvisible\n";
    let expected = "*   alpha\n* beta *em*";

    let default_dir = tempdir().unwrap();
    let default_input = default_dir.path().join("default-source-spelling.qd");
    fs::write(&default_input, source).unwrap();
    let default = run_build(&default_input);
    assert!(
        default.status.success(),
        "{}",
        String::from_utf8_lossy(&default.stderr)
    );
    assert_eq!(
        String::from_utf8(default.stdout).expect("stdout must be UTF-8"),
        format!("{expected}\n")
    );
    let default_stderr = String::from_utf8(default.stderr).expect("stderr must be UTF-8");
    assert!(
        default_stderr.contains(&format!(
            "Cannot call function error(String message) with arguments ({expected}): {expected}"
        )),
        "{default_stderr}"
    );
    let typst = fs::read_to_string(default_dir.path().join("default-source-spelling.typ")).unwrap();
    assert!(
        typst.contains("#raw(\".error {.values}\", block: true)"),
        "{typst}"
    );
    assert!(typst.contains("visible"), "{typst}");

    let strict_dir = tempdir().unwrap();
    let strict_input = strict_dir.path().join("strict-source-spelling.qd");
    fs::write(&strict_input, source).unwrap();
    let strict = run_build_with_args(&strict_input, &["--strict"]);
    assert_eq!(strict.status.code(), Some(66));
    assert_eq!(
        String::from_utf8(strict.stdout).expect("stdout must be UTF-8"),
        format!("{expected}\n")
    );
    assert_eq!(
        String::from_utf8(strict.stderr).expect("stderr must be UTF-8"),
        format!(
            "An error occurred while in strict mode (error code 66)\n\
             Originated from function: error\n\
             java.lang.Exception: {expected}\n"
        )
    );
    assert!(!strict_dir
        .path()
        .join("strict-source-spelling.typ")
        .exists());
}

#[test]
fn build_recovers_direct_markdown_list_error_and_strict_finalizes() {
    let source = ".var {values}\n    - alpha\n    - beta\n.error {.values}\nvisible\n";

    let default_dir = tempdir().unwrap();
    let default_input = default_dir.path().join("default.qd");
    fs::write(&default_input, source).unwrap();
    let default = run_build(&default_input);
    assert!(
        default.status.success(),
        "{}",
        String::from_utf8_lossy(&default.stderr)
    );
    let default_stderr = String::from_utf8(default.stderr).expect("stderr must be UTF-8");
    assert!(
        default_stderr.contains(
            "Cannot call function error(String message) with arguments (- alpha\n- beta): - alpha\n- beta"
        ),
        "{default_stderr}"
    );
    let typst = fs::read_to_string(default_dir.path().join("default.typ")).unwrap();
    assert!(
        typst.contains("#raw(\".error {.values}\", block: true)"),
        "{typst}"
    );
    assert!(typst.contains("alpha") && typst.contains("beta"), "{typst}");
    assert!(typst.contains("visible"), "{typst}");

    let strict_dir = tempdir().unwrap();
    let strict_input = strict_dir.path().join("strict.qd");
    fs::write(&strict_input, source).unwrap();
    let strict = run_build_with_args(&strict_input, &["--strict"]);
    assert_eq!(strict.status.code(), Some(66));
    assert_eq!(
        String::from_utf8(strict.stderr).expect("stderr must be UTF-8"),
        "An error occurred while in strict mode (error code 66)\n\
         Originated from function: error\n\
         java.lang.Exception: - alpha\n- beta\n"
    );
    assert!(!strict_dir.path().join("strict.typ").exists());
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
    assert!(
        typst.contains("#raw(\".error {boom}\", block: true)"),
        "{typst}"
    );
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
    assert!(
        typst.contains("#raw(\".error {function-error}\", block: true)"),
        "{typst}"
    );
    assert!(!typst.contains("#raw(\".boom\", block: true)"), "{typst}");
}

#[test]
fn build_preserves_evidenced_dynamic_named_and_spacing_error_source_echo() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("main.qd");
    fs::write(
        &input,
        ".var {msg} {dynamic-error}\n.error {.msg}\n.error message:{named-error}\n.error    {spacing-error}\n",
    )
    .unwrap();

    let output = run_build(&input);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let typst = fs::read_to_string(dir.path().join("main.typ")).unwrap();
    for expected in [
        "#raw(\".error {.msg}\", block: true)",
        "#raw(\".error message:{named-error}\", block: true)",
        "#raw(\".error    {spacing-error}\", block: true)",
    ] {
        assert!(typst.contains(expected), "missing {expected:?}: {typst}");
    }
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
fn strict_build_promotes_evidenced_structural_nested_errors_to_exit_66() {
    let cases = [
        (
            "center",
            "outer-before\n.center\n    center-before\n    .error {center-error}\n    center-after\nouter-after\n",
            "center-error",
        ),
        (
            "align",
            "outer-before\n.align {center}\n    align-before\n    .error {align-error}\n    align-after\nouter-after\n",
            "align-error",
        ),
        (
            "container",
            "outer-before\n.container\n    container-before\n    .error {container-error}\n    container-after\nouter-after\n",
            "container-error",
        ),
        (
            "unordered-list",
            "outer-before\n\n- list-before .error {list-error} list-after\n\nouter-after\n",
            "list-error",
        ),
        (
            "blockquote",
            "outer-before\n\n> quote-before .error {quote-error} quote-after\n\nouter-after\n",
            "quote-error",
        ),
        (
            "ordered-list",
            "outer-before\n\n1. ordered-before .error {ordered-error} ordered-after\n\nouter-after\n",
            "ordered-error",
        ),
        (
            "row",
            "outer-before\n.row\n    row-before\n    .error {row-error}\n    row-after\nouter-after\n",
            "row-error",
        ),
        (
            "column",
            "outer-before\n.column\n    column-before\n    .error {column-error}\n    column-after\nouter-after\n",
            "column-error",
        ),
        (
            "grid",
            "outer-before\n.grid columns:{2}\n    grid-before\n    .error {grid-error}\n    grid-after\nouter-after\n",
            "grid-error",
        ),
        (
            "landscape",
            "outer-before\n.landscape\n    landscape-before\n    .error {landscape-error}\n    landscape-after\nouter-after\n",
            "landscape-error",
        ),
        (
            "deeper-center-row",
            "outer-before\n.center\n    center-before\n    .row\n        deep-before\n        .error {deep-error}\n        deep-after\n    center-after\nouter-after\n",
            "deep-error",
        ),
        (
            "top-inline",
            "outer-before\ninline-before .error {top-inline-error} inline-after\nouter-after\n",
            "top-inline-error",
        ),
    ];

    for (name, source, message) in cases {
        let dir = tempdir().unwrap();
        let input = dir.path().join(format!("{name}.qd"));
        fs::write(&input, source).unwrap();

        let output = run_build_with_args(&input, &["--strict"]);

        assert_eq!(output.status.code(), Some(66), "{name}: {output:?}");
        assert_eq!(
            String::from_utf8(output.stderr).unwrap(),
            format!(
                "An error occurred while in strict mode (error code 66)\n\
                 Originated from function: error\n\
                 java.lang.Exception: {message}\n"
            ),
            "{name}"
        );
        assert!(
            !dir.path().join(format!("{name}.typ")).exists(),
            "{name}: strict mode must publish no artifact"
        );
    }
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
fn default_build_recovers_evidenced_structural_nested_errors_and_preserves_siblings() {
    let cases = [
        (
            "center",
            "outer-before\n.center\n    center-before\n    .error {center-error}\n    center-after\nouter-after\n",
            "center-error",
            "center-before",
            "center-after",
        ),
        (
            "align",
            "outer-before\n.align {center}\n    align-before\n    .error {align-error}\n    align-after\nouter-after\n",
            "align-error",
            "align-before",
            "align-after",
        ),
        (
            "container",
            "outer-before\n.container\n    container-before\n    .error {container-error}\n    container-after\nouter-after\n",
            "container-error",
            "container-before",
            "container-after",
        ),
        (
            "unordered-list",
            "outer-before\n\n- list-before .error {list-error} list-after\n\nouter-after\n",
            "list-error",
            "list-before",
            "list-after",
        ),
        (
            "blockquote",
            "outer-before\n\n> quote-before .error {quote-error} quote-after\n\nouter-after\n",
            "quote-error",
            "quote-before",
            "quote-after",
        ),
        (
            "ordered-list",
            "outer-before\n\n1. ordered-before .error {ordered-error} ordered-after\n\nouter-after\n",
            "ordered-error",
            "ordered-before",
            "ordered-after",
        ),
        (
            "row",
            "outer-before\n.row\n    row-before\n    .error {row-error}\n    row-after\nouter-after\n",
            "row-error",
            "row-before",
            "row-after",
        ),
        (
            "column",
            "outer-before\n.column\n    column-before\n    .error {column-error}\n    column-after\nouter-after\n",
            "column-error",
            "column-before",
            "column-after",
        ),
        (
            "grid",
            "outer-before\n.grid columns:{2}\n    grid-before\n    .error {grid-error}\n    grid-after\nouter-after\n",
            "grid-error",
            "grid-before",
            "grid-after",
        ),
        (
            "landscape",
            "outer-before\n.landscape\n    landscape-before\n    .error {landscape-error}\n    landscape-after\nouter-after\n",
            "landscape-error",
            "landscape-before",
            "landscape-after",
        ),
        (
            "deeper-center-row",
            "outer-before\n.center\n    center-before\n    .row\n        deep-before\n        .error {deep-error}\n        deep-after\n    center-after\nouter-after\n",
            "deep-error",
            "deep-before",
            "deep-after",
        ),
        (
            "top-inline",
            "outer-before\ninline-before .error {top-inline-error} inline-after\nouter-after\n",
            "top-inline-error",
            "inline-before",
            "inline-after",
        ),
    ];

    for (name, source, message, local_before, local_after) in cases {
        let dir = tempdir().unwrap();
        let input = dir.path().join(format!("{name}.qd"));
        fs::write(&input, source).unwrap();

        let output = run_build(&input);
        assert!(
            output.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(&format!(
                "Cannot call function error(String message) with arguments ({message}): {message}"
            )),
            "{name}: {stderr}"
        );

        let typst = fs::read_to_string(dir.path().join(format!("{name}.typ"))).unwrap();
        let outer_before = typst.find("outer-before").expect("outer-before");
        let local_before_index = typst.find(local_before).expect("local-before");
        let error = typst.find(message).expect("error");
        let local_after_index = typst.rfind(local_after).expect("local-after");
        let outer_after = typst.rfind("outer-after").expect("outer-after");
        let expected_source_echo = format!("#raw(\".error {{{message}}}\", block: true)");
        assert!(
            typst.contains(&expected_source_echo),
            "{name}: missing {expected_source_echo:?}: {typst}"
        );
        assert!(
            outer_before < local_before_index
                && local_before_index < error
                && error < local_after_index
                && local_after_index < outer_after,
            "{name}: {typst}"
        );
    }
}

#[test]
fn unevidenced_structural_compositions_remain_build_fatal() {
    let cases = [
        (
            "too-deep",
            ".center\n    .row\n        .landscape\n            .error {too-deep-error}\n",
            "too-deep-error",
        ),
        (
            "align-row",
            ".align {center}\n    .row\n        .error {align-row-error}\n",
            "align-row-error",
        ),
        (
            "container-row",
            ".container\n    .row\n        .error {container-row-error}\n",
            "container-row-error",
        ),
    ];

    for (name, source, message) in cases {
        let dir = tempdir().unwrap();
        let input = dir.path().join(format!("{name}.qd"));
        fs::write(&input, source).unwrap();

        let output = run_build(&input);

        assert!(!output.status.success(), "{name}: {output:?}");
        assert_ne!(output.status.code(), Some(66), "{name}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(message), "{name}: {stderr}");
        assert!(stderr.contains("found 1 error(s)"), "{name}: {stderr}");
        assert!(
            !dir.path().join(format!("{name}.typ")).exists(),
            "{name}: unevidenced output must not publish an artifact"
        );
    }
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

#[test]
fn build_recovers_evidenced_root_inline_owner_errors_and_strict_suppresses_artifacts() {
    let cases = [
        (
            "heading",
            "# heading-before .error {heading-error} heading-after\n",
            "heading-error",
            "heading-before",
            "heading-after",
        ),
        (
            "table",
            "| value |\n| --- |\n| cell-before .error {table-error} cell-after |\n",
            "table-error",
            "cell-before",
            "cell-after",
        ),
        (
            "emphasis",
            "outer-before *em-before .error {emphasis-error} em-after* outer-after\n",
            "emphasis-error",
            "em-before",
            "em-after",
        ),
        (
            "strong",
            "outer-before **strong-before .error {strong-error} strong-after** outer-after\n",
            "strong-error",
            "strong-before",
            "strong-after",
        ),
        (
            "strike",
            "outer-before ~~strike-before .error {strike-error} strike-after~~ outer-after\n",
            "strike-error",
            "strike-before",
            "strike-after",
        ),
        (
            "link",
            "outer-before [link-before .error {link-error} link-after](https://example.com) outer-after\n",
            "link-error",
            "link-before",
            "link-after",
        ),
    ];

    for (name, source, message, before_text, after_text) in cases {
        let default_dir = tempdir().unwrap();
        let default_input = default_dir.path().join(format!("{name}.qd"));
        fs::write(&default_input, source).unwrap();

        let default = run_build(&default_input);
        assert!(
            default.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&default.stderr)
        );
        let stderr = String::from_utf8(default.stderr).expect("stderr must be UTF-8");
        assert!(
            stderr.contains(&format!(
                "Cannot call function error(String message) with arguments ({message}): {message}"
            )),
            "{name}: {stderr}"
        );

        let typst = fs::read_to_string(default_dir.path().join(format!("{name}.typ"))).unwrap();
        let before = typst.find(before_text).expect("owner prefix");
        let error = typst.find(message).expect("explicit error message");
        let after = typst.rfind(after_text).expect("owner suffix");
        assert!(before < error && error < after, "{name}: {typst}");
        assert!(
            typst.contains(&format!("#raw(\".error {{{message}}}\")")),
            "{name}: {typst}"
        );

        let strict_dir = tempdir().unwrap();
        let strict_input = strict_dir.path().join(format!("{name}.qd"));
        fs::write(&strict_input, source).unwrap();
        let strict = run_build_with_args(&strict_input, &["--strict"]);
        assert_eq!(
            strict.status.code(),
            Some(66),
            "{name}: {}",
            String::from_utf8_lossy(&strict.stderr)
        );
        assert_eq!(
            String::from_utf8(strict.stderr).unwrap(),
            format!(
                "An error occurred while in strict mode (error code 66)\n\
                 Originated from function: error\n\
                 java.lang.Exception: {message}\n"
            ),
            "{name}"
        );
        assert!(
            !strict_dir.path().join(format!("{name}.typ")).exists(),
            "{name}: strict mode must publish no artifact"
        );
    }
}

#[test]
fn nested_unprobed_inline_owner_composition_remains_on_ordinary_fatal_path() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("nested-inline-owner.qd");
    fs::write(&input, "- *before .error {nested-inline-error} after*\n").unwrap();

    let output = run_build(&input);
    assert!(!output.status.success(), "{output:?}");
    assert_ne!(output.status.code(), Some(66));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("nested-inline-error"), "{stderr}");
    assert!(
        !dir.path().join("nested-inline-owner.typ").exists(),
        "unevidenced nested inline-owner composition must publish no artifact"
    );
}
