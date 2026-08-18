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
        typst_code.contains("   ```\ncode\n   ```"),
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
fn integration_bounded_inline_markdown_html_maps_to_ir_typst_and_pdf() {
    use scribium_core::{compile, CompileOptions, VirtualProjectBuilder};

    let source =
        "Before <em>italic <strong>bold</strong></em> <del>removed</del> <s>old</s><br/> next.\n";
    let entry = "raw-html.md";
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
    assert!(typst_code.contains("italic"));
    assert!(typst_code.contains("bold"));
    assert!(typst_code.contains("#strike[removed]"));
    assert!(typst_code.contains("#strike[old]"));
    assert!(typst_code.contains("\\\n next."), "{typst_code:?}");
    assert!(!typst_code.contains("<em>"));
    assert!(!typst_code.contains("<strong>"));

    with_typst(entry, |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: entry.to_string(),
            })
            .expect("bounded HTML Typst must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_commonmark_gfm_baseline_fixture_compiles_to_valid_pdf() {
    use scribium_core::{compile, CompileOptions, VirtualProjectBuilder};

    let source = include_str!("../../../fixtures/markdown/commonmark_gfm_baseline.md");
    let project = VirtualProjectBuilder::new()
        .entry("commonmark_gfm_baseline.md")
        .expect("valid entry path")
        .add_source("commonmark_gfm_baseline.md", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "baseline fixture diagnostics: {:?}",
        result.diagnostics
    );
    let typst_code = scribium_typst::lowering::lower_to_typst_code(&result.ir);
    assert!(typst_code.contains("= Scribium Markdown baseline"));
    assert!(typst_code.contains("== Setext heading"));
    assert!(typst_code.contains("#link(\"https://example.com/docs\")"));
    assert!(typst_code.contains("#strike[strikethrough]"));
    assert!(typst_code.contains("```rust\nfn main()"));
    assert!(typst_code.contains("#table("));
    assert!(typst_code.contains("☐ open task"));
    assert!(typst_code.contains("☑ completed task"));
    assert!(result.ir.nodes.iter().any(|node| matches!(
        node,
        scribium_core::ir::IrNode::CodeBlock {
            info: Some(info),
            ..
        } if info == "rust extra-info"
    )));

    with_typst("commonmark-gfm-baseline", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "commonmark_gfm_baseline.qd".to_string(),
            })
            .expect("CommonMark/GFM baseline Typst must compile");
        let pdf = output.pdf.expect("PDF output must be present");
        assert!(pdf.starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_markdown_utf8_crlf_breaks_lower_and_compile_to_pdf() {
    use scribium_core::{compile, CompileOptions, VirtualProjectBuilder};

    let source = "한글\r\n다음  \r\n끝";
    let project = VirtualProjectBuilder::new()
        .entry("crlf.md")
        .expect("valid entry path")
        .add_source("crlf.md", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "CRLF fixture diagnostics: {:?}",
        result.diagnostics
    );
    let typst_code = scribium_typst::lowering::lower_to_typst_code(&result.ir);
    assert!(typst_code.contains("한글\n다음\\\n끝"), "{typst_code:?}");

    with_typst("markdown-utf8-crlf-breaks", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "crlf.qd".to_string(),
            })
            .expect("UTF-8 CRLF break Typst must compile");
        let pdf = output.pdf.expect("PDF output must be present");
        assert!(pdf.starts_with(b"%PDF-"));
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
    // The source-backed content is retained as literal text; Typst markup
    // delimiters are escaped at the backend boundary rather than being
    // falsely represented as Strong after synthetic reparsing was removed.
    assert!(
        typst_code.contains("\\*\\*Scribium\\*\\*"),
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
fn integration_logical_comparison_evaluation_reaches_typst_and_pdf() {
    use scribium_core::{compile, CompileOptions, VirtualProjectBuilder};

    let source = ".if {.islower {2} than:{3}}\n    selected\n.if {.isgreater {2} than:{3}}\n    suppressed\n";
    let project = VirtualProjectBuilder::new()
        .entry("logical.qd")
        .expect("valid entry path")
        .add_source("logical.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "logical comparison diagnostics: {:?}",
        result.diagnostics
    );
    let typst_code = scribium_typst::lowering::lower_to_typst_code(&result.ir);
    assert!(typst_code.contains("selected"), "{typst_code:?}");
    assert!(!typst_code.contains("suppressed"), "{typst_code:?}");
    assert!(
        !typst_code.contains(".islower"),
        "source call leaked: {typst_code:?}"
    );

    with_typst("logical-comparison", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "logical.qd".to_string(),
            })
            .expect("logical comparison Typst must compile");
        let pdf = output.pdf.expect("PDF output must be present");
        assert!(pdf.starts_with(b"%PDF-"));
    });
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

#[test]
fn integration_user_function_evaluation_reaches_typst_and_pdf() {
    use scribium_core::{compile, CompileOptions, VirtualProjectBuilder};

    let declaration = ".function {area}\n    width height:\n    .multiply {.width} by:{.height}\n\n# Result\n\nArea: ";
    let direct_source = format!("{declaration}.area {{4}} {{2}}\n");
    let nested_source = format!("{declaration}.sum {{.area {{4}} {{2}}}} {{1}}\n");
    let chain_source = format!("{declaration}.area {{4}} {{2}}::sum {{1}}\n");

    let compile_source = |entry: &str, source: &str| {
        let project = VirtualProjectBuilder::new()
            .entry(entry)
            .expect("valid path")
            .add_source(entry, source)
            .expect("valid path")
            .build()
            .unwrap();
        compile(&project, &CompileOptions::default())
    };

    let direct = compile_source("direct.qd", &direct_source);
    let nested = compile_source("nested.qd", &nested_source);
    let chained = compile_source("chained.qd", &chain_source);
    assert!(
        direct.diagnostics.is_empty(),
        "direct: {:?}",
        direct.diagnostics
    );
    assert!(
        nested.diagnostics.is_empty(),
        "nested: {:?}",
        nested.diagnostics
    );
    assert!(
        chained.diagnostics.is_empty(),
        "chain: {:?}",
        chained.diagnostics
    );

    let direct_typst = scribium_typst::lowering::lower_to_typst_code(&direct.ir);
    let nested_typst = scribium_typst::lowering::lower_to_typst_code(&nested.ir);
    let chained_typst = scribium_typst::lowering::lower_to_typst_code(&chained.ir);
    assert_eq!(nested_typst, chained_typst);
    assert!(
        direct_typst.contains("Area: 8"),
        "generated Typst: {direct_typst}"
    );
    assert!(
        nested_typst.contains("Area: 9"),
        "generated Typst: {nested_typst}"
    );
    assert!(!direct_typst.contains(".function"));
    assert!(!direct_typst.contains(".area"));

    with_typst("user-function-evaluation", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: direct_typst,
                entry_path: "user-function.qd".to_string(),
            })
            .expect("user-function Typst must compile");
        let pdf = output
            .pdf
            .expect("user-function PDF output must be present");
        assert!(pdf.starts_with(b"%PDF-"), "PDF must start with %PDF-");
    });
}

#[test]
fn integration_optional_function_parameters_reach_typst_and_pdf() {
    use scribium_core::{compile, CompileOptions, VirtualProjectBuilder};

    let source = ".function {greet}\n    name?:\n    Hello, .name::otherwise {anonymous}!\n\n.greet\n.greet {John}\n";
    let project = VirtualProjectBuilder::new()
        .entry("optional.qd")
        .expect("valid path")
        .add_source("optional.qd", source)
        .expect("valid path")
        .build()
        .unwrap();
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "optional diagnostics: {:?}",
        result.diagnostics
    );

    let typst_code = scribium_typst::lowering::lower_to_typst_code(&result.ir);
    assert!(
        typst_code.contains("Hello, anonymous!") && typst_code.contains("Hello, John!"),
        "generated Typst: {typst_code}"
    );

    with_typst("optional-function-parameters", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "optional.qd".to_string(),
            })
            .expect("optional-parameter Typst must compile");
        let pdf = output
            .pdf
            .expect("optional-parameter PDF output must be present");
        assert!(pdf.starts_with(b"%PDF-"), "PDF must start with %PDF-");
    });
}

#[test]
fn integration_implicit_lambda_parameter_reaches_typst_and_pdf() {
    use scribium_core::{compile, CompileOptions, VirtualProjectBuilder};

    let source = ".function {triple}\n    .multiply {.1} {3}\n\nImplicit result: .triple {2}\n";
    let project = VirtualProjectBuilder::new()
        .entry("implicit.qd")
        .expect("valid path")
        .add_source("implicit.qd", source)
        .expect("valid path")
        .build()
        .unwrap();
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "implicit diagnostics: {:?}",
        result.diagnostics
    );

    let typst_code = scribium_typst::lowering::lower_to_typst_code(&result.ir);
    assert!(
        typst_code.contains("Implicit result: 6"),
        "generated Typst: {typst_code}"
    );
    assert!(!typst_code.contains(".triple"));

    with_typst("implicit-lambda-parameter", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "implicit.qd".to_string(),
            })
            .expect("implicit-parameter Typst must compile");
        let pdf = output
            .pdf
            .expect("implicit-parameter PDF output must be present");
        assert!(pdf.starts_with(b"%PDF-"), "PDF must start with %PDF-");
    });
}
