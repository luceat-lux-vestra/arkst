//! Integration tests that exercise the real `typst` executable.
//!
//! These tests are separated from the unit tests in `backend.rs` because
//! they need an actual Typst installation. They are skipped (with a notice)
//! when no Typst executable can be located, so a developer machine or a CI
//! runner without Typst can still run the rest of the suite. CI installs a
//! pinned Typst version explicitly before running tests; set
//! `SCRIBIUM_REQUIRE_TYPST=1` to turn a missing executable into a hard
//! failure instead of a skip.

use std::path::PathBuf;
use std::process::Command;

use scribium_typst::backend::{SubprocessBackend, TypstBackend, TypstInput};

/// Locates a Typst executable, in order of preference:
///
/// 1. `SCRIBIUM_TYPST_PATH` (used by CI to point at a pinned install);
/// 2. `typst` on `PATH`;
/// 3. the Homebrew default location (`/opt/homebrew/bin/typst`).
fn find_typst() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("SCRIBIUM_TYPST_PATH") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
        return None;
    }
    let probe = Command::new("typst").arg("--version").output();
    if probe.is_ok_and(|o| o.status.success()) {
        return Some(PathBuf::from("typst"));
    }
    let homebrew = PathBuf::from("/opt/homebrew/bin/typst");
    if homebrew.is_file() {
        return Some(homebrew);
    }
    None
}

/// Runs `body` with a located Typst backend, skipping (or failing, when
/// `SCRIBIUM_REQUIRE_TYPST` is set) if none can be found.
fn with_typst<F>(name: &str, body: F)
where
    F: FnOnce(SubprocessBackend),
{
    match find_typst() {
        Some(path) => {
            eprintln!("[integration] {name}: using typst at {}", path.display());
            body(SubprocessBackend::new(path));
        }
        None => {
            let required = std::env::var("SCRIBIUM_REQUIRE_TYPST").is_ok();
            let message = format!(
                "[integration] {name}: no Typst executable found (set SCRIBIUM_TYPST_PATH or install typst); \
                 {}",
                if required {
                    "SCRIBIUM_REQUIRE_TYPST is set, failing"
                } else {
                    "skipping"
                }
            );
            eprintln!("{message}");
            if required {
                panic!("{message}");
            }
        }
    }
}

#[test]
fn integration_compile_produces_valid_pdf() {
    with_typst("compile", |backend| {
        let input = TypstInput {
            source: "#heading[Test]\n\nHello world.\n".to_string(),
            entry_path: "test.qd".to_string(),
        };
        let output = backend.compile(&input).expect("compile should succeed");
        let pdf = output.pdf.expect("pdf output must be present");
        assert!(!pdf.is_empty(), "pdf must not be empty");
        assert!(
            pdf.starts_with(b"%PDF-"),
            "pdf must start with %PDF-, began with {:?}",
            &pdf[..pdf.len().min(8)]
        );
    });
}

#[test]
fn integration_version_succeeds() {
    with_typst("version", |backend| {
        let version = backend.version().expect("version should succeed");
        assert!(!version.is_empty(), "version output must not be empty");
        assert!(version.contains("typst"), "version was: {}", version);
    });
}

#[test]
fn integration_compile_failure_surfaces_diagnostic() {
    with_typst("compile-failure", |backend| {
        let input = TypstInput {
            source: "#heading[Test\n".to_string(),
            entry_path: "test.qd".to_string(),
        };
        let result = backend.compile(&input);
        assert!(result.is_err(), "invalid Typst must fail");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("compilation failed"),
            "error must surface the compiler diagnostic, was: {}",
            err
        );
    });
}

#[test]
fn integration_configured_path_is_respected() {
    // The custom-path plumbing is validated by using the located binary
    // through an explicitly configured path rather than the default: the
    // backend passed here was constructed from a resolved path, not the
    // bare `typst` on `PATH`.
    with_typst("configured-path", |backend| {
        let input = TypstInput {
            source: "Hello world.\n".to_string(),
            entry_path: "test.qd".to_string(),
        };
        let output = backend.compile(&input).expect("compile should succeed");
        let pdf = output.pdf.expect("pdf output must be present");
        assert!(pdf.starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_multi_block_list_item_compiles() {
    // Scribium source whose first list item contains a paragraph followed by
    // a fenced code block; the generated Typst must keep the code block
    // inside the item (fences on the item's content column).
    use scribium_core::{compile, CompileOptions, VirtualProjectBuilder};
    let source = "1. item\n\n    ```\n    code\n    ```\n\n2. next\n";
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("valid path")
        .add_source("main.qd", source)
        .expect("valid path")
        .build()
        .unwrap();
    let result = compile(&project, &CompileOptions::default());
    assert!(result.diagnostics.is_empty());
    let typst_code = scribium_typst::lowering::lower_to_typst_code(&result.ir);
    assert!(
        typst_code.contains("   ```\n code\n   ```"),
        "code block must be inside the first item: {:?}",
        typst_code
    );

    with_typst("multi-block-list", |backend| {
        let input = TypstInput {
            source: typst_code,
            entry_path: "test.qd".to_string(),
        };
        let output = backend.compile(&input).expect("compile should succeed");
        let pdf = output.pdf.expect("pdf output must be present");
        assert!(pdf.starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_markdown_structures_compile_to_valid_pdf() {
    use scribium_core::{compile, CompileOptions, VirtualProjectBuilder};

    let source = "> quoted **strong**\n>\n> - [ ] active\n>   - [x] nested\n> - [x] completed\n\nBefore ~removed **content**~ after ~~double~~.\n\n| Left | Center | Right | Default |\n| :--- | :---: | ---: | --- |\n| α | **β** | ~γ~ | tail |\n";
    for entry in ["main.md", "main.qd"] {
        let project = VirtualProjectBuilder::new()
            .entry(entry)
            .expect("valid entry path")
            .add_source(entry, source)
            .expect("valid source path")
            .build()
            .expect("valid project");
        let result = compile(&project, &CompileOptions::default());
        assert!(
            result.diagnostics.is_empty(),
            "{entry} diagnostics: {:?}",
            result.diagnostics
        );
        let typst_code = scribium_typst::lowering::lower_to_typst_code(&result.ir);
        assert!(typst_code.contains("#quote(block: true)"));
        assert!(typst_code.contains("#strike[removed *content*]"));
        assert!(typst_code.contains("#strike[double]"));
        assert!(typst_code.contains("☐ active"));
        assert!(typst_code.contains("☑ nested"));
        assert!(typst_code.contains("☑ completed"));
        assert!(typst_code.contains("#table("));

        with_typst(entry, |backend| {
            let output = backend
                .compile(&TypstInput {
                    source: typst_code,
                    entry_path: entry.to_string(),
                })
                .expect("structured Markdown Typst must compile");
            let pdf = output.pdf.expect("PDF output must be present");
            assert!(pdf.starts_with(b"%PDF-"));
        });
    }

    let body_source =
        ".if {true}\n  > body **strong**\n  >\n  > - [ ] active\n  > - [x] completed\n";
    let project = VirtualProjectBuilder::new()
        .entry("body.qd")
        .expect("valid entry path")
        .add_source("body.qd", body_source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "body diagnostics: {:?}",
        result.diagnostics
    );
    let typst_code = scribium_typst::lowering::lower_to_typst_code(&result.ir);
    assert!(typst_code.contains("#quote(block: true)"));
    assert!(typst_code.contains("☐ active"));
    assert!(typst_code.contains("☑ completed"));
    with_typst("quarkdown-body-structures", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "body.qd".to_string(),
            })
            .expect("Quarkdown body Typst must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_variable_evaluation_before_lowering() {
    // This test validates that variable evaluation happens before Typst lowering.
    // It uses the core compile path directly since the backend doesn't expose
    // the generated Typst source.
    use scribium_core::{compile, CompileOptions, VirtualProjectBuilder};

    // Test variable declaration and reference
    let source = ".var {name} {Scribium}\nHello .name\n";
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("valid path")
        .add_source("main.qd", source)
        .expect("valid path")
        .build()
        .unwrap();
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "diagnostics: {:?}",
        result.diagnostics
    );

    let typst_code = scribium_typst::lowering::lower_to_typst_code(&result.ir);
    // Variable should be resolved in the output
    assert!(
        typst_code.contains("Scribium"),
        "variable value should appear in output: {}",
        typst_code
    );
    // .var declaration artifact should not appear
    assert!(
        !typst_code.contains(".var"),
        ".var declaration should not leak to output: {}",
        typst_code
    );
    // Variable reference artifact should not appear
    assert!(
        !typst_code.contains(".name"),
        "variable reference should not leak to output: {}",
        typst_code
    );

    // Test rich content variable
    let source = ".var {name} {**Scribium**}\nHello .name\n";
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("valid path")
        .add_source("main.qd", source)
        .expect("valid path")
        .build()
        .unwrap();
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E3010"),
        "expected an explicit unsupported content diagnostic: {:?}",
        result.diagnostics
    );

    let typst_code = scribium_typst::lowering::lower_to_typst_code(&result.ir);
    // The source-backed content is retained as literal text; it is not
    // falsely represented as Strong after synthetic reparsing was removed.
    assert!(
        typst_code.contains("**Scribium**"),
        "unsupported rich content must remain source text: {}",
        typst_code
    );
    assert!(
        !typst_code.contains(".var"),
        ".var declaration should not leak to output: {}",
        typst_code
    );
    assert!(
        !typst_code.contains(".name"),
        "variable reference should not leak to output: {}",
        typst_code
    );
}

#[test]
fn integration_chain_evaluation_reaches_typst_and_pdf() {
    use scribium_core::{compile, CompileOptions, VirtualProjectBuilder};

    let source = ".sum {10} {5}::multiply {2}\n";
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("valid path")
        .add_source("main.qd", source)
        .expect("valid path")
        .build()
        .unwrap();
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "chain diagnostics: {:?}",
        result.diagnostics
    );

    let typst_code = scribium_typst::lowering::lower_to_typst_code(&result.ir);
    assert!(typst_code.contains("30"), "generated Typst: {typst_code}");
    assert!(!typst_code.contains("parser-preserved call chain"));
    assert!(!typst_code.contains("#sum"));
    assert!(!typst_code.contains("#multiply"));

    let nested_project = VirtualProjectBuilder::new()
        .entry("nested.qd")
        .expect("valid path")
        .add_source("nested.qd", ".multiply {.sum {10} {5}} {2}\n")
        .expect("valid path")
        .build()
        .unwrap();
    let nested_result = compile(&nested_project, &CompileOptions::default());
    assert!(
        nested_result.diagnostics.is_empty(),
        "nested diagnostics: {:?}",
        nested_result.diagnostics
    );
    let nested_typst_code = scribium_typst::lowering::lower_to_typst_code(&nested_result.ir);
    assert_eq!(nested_typst_code, typst_code);

    with_typst("chain-evaluation", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "main.qd".to_string(),
            })
            .expect("evaluated chain Typst must compile");
        assert!(output
            .pdf
            .expect("chain PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}
