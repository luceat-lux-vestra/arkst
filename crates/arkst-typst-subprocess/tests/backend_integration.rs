//! Integration tests that exercise the real `typst` executable.
//!
//! These tests are separated from the unit tests in `backend.rs` because
//! they need an actual Typst installation. They are skipped (with a notice)
//! when no Typst executable can be located, so a developer machine or a CI
//! runner without Typst can still run the rest of the suite. CI installs a
//! pinned Typst version explicitly before running tests; set
//! `ARKST_REQUIRE_TYPST=1` to turn a missing executable into a hard
//! failure instead of a skip.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use arkst_core::ir::{
    IrColor, IrComponent, IrDocumentAlignment, IrInline, IrNode, IrPageBorderWidths,
    IrPageGeometry, IrPageMargins, IrSize, IrSizeUnit, NativeTarget,
};
use arkst_core::{compile, CompileOptions, VirtualPathBuf, VirtualProjectBuilder};
use arkst_typst::lowering::{lower_to_typst, lower_to_typst_code};
use arkst_typst::{TypstBackend, TypstInput};
#[cfg(unix)]
use arkst_typst_subprocess::TypstError;
use arkst_typst_subprocess::{SubprocessBackend, TypstSourceContext};
use tempfile::tempdir;

/// Locates a Typst executable, in order of preference:
///
/// 1. `ARKST_TYPST_PATH` (used by CI to point at a pinned install);
/// 2. `typst` on `PATH`;
/// 3. the Homebrew default location (`/opt/homebrew/bin/typst`).
fn find_typst() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("ARKST_TYPST_PATH") {
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
/// `ARKST_REQUIRE_TYPST` is set) if none can be found.
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
            let required = std::env::var("ARKST_REQUIRE_TYPST").is_ok();
            let message = format!(
                "[integration] {name}: no Typst executable found (set ARKST_TYPST_PATH or install typst); \
                 {}",
                if required {
                    "ARKST_REQUIRE_TYPST is set, failing"
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
fn integration_runtime_evaluation_remains_available_to_subprocess() {
    with_typst("runtime-evaluation", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: "#eval(\"1 + 1\", mode: \"code\")\n".to_string(),
                entry_path: "runtime-evaluation.qd".to_string(),
            })
            .expect("static preflight must not disable Typst runtime evaluation");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_explicit_error_lowering_escapes_typst_text_and_compiles_pdf() {
    let project = VirtualProjectBuilder::new()
        .entry("explicit-error.qd")
        .expect("valid entry path")
        .add_source("explicit-error.qd", ".error {boom}\n")
        .expect("valid source path")
        .build()
        .expect("valid project");
    let mut result = compile(&project, &CompileOptions::default());

    assert_eq!(result.diagnostics.len(), 1, "{:?}", result.diagnostics);
    assert_eq!(result.diagnostics[0].code, "E3011");
    let [IrNode::Component {
        component: IrComponent::ExplicitError(error),
    }] = result.ir.nodes.as_mut_slice()
    else {
        panic!("expected explicit error component");
    };
    error.message = "boom [literal] #hash \\ path\nnext".to_string();

    let typst_code = lower_to_typst_code(&result.ir);
    assert!(
        typst_code.contains(r"boom \[literal\] \#hash \\ path"),
        "{typst_code}"
    );
    assert!(typst_code.contains("next"), "{typst_code}");

    with_typst("explicit-error", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "explicit-error.qd".to_string(),
            })
            .expect("explicit-error Typst must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_evidenced_inline_error_owners_lower_to_valid_typst_and_pdf() {
    let cases = [
        (
            "inline-error-heading",
            "# heading-before .error {heading-error} heading-after\n",
        ),
        (
            "inline-error-table",
            "| value |\n| --- |\n| cell-before .error {table-error} cell-after |\n",
        ),
        (
            "inline-error-emphasis",
            "outer-before *em-before .error {emphasis-error} em-after* outer-after\n",
        ),
        (
            "inline-error-strong",
            "outer-before **strong-before .error {strong-error} strong-after** outer-after\n",
        ),
        (
            "inline-error-strike",
            "outer-before ~~strike-before .error {strike-error} strike-after~~ outer-after\n",
        ),
        (
            "inline-error-link",
            "outer-before [link-before .error {link-error} link-after](https://example.com) outer-after\n",
        ),
        (
            "inline-error-list-emphasis",
            "- list-before *em-before .error {list-emphasis-error} em-after* list-after\n",
        ),
    ];

    for (name, source) in cases {
        let entry = format!("{name}.qd");
        let project = VirtualProjectBuilder::new()
            .entry(&entry)
            .expect("valid entry path")
            .add_source(&entry, source)
            .expect("valid source path")
            .build()
            .expect("valid project");
        let result = compile(&project, &CompileOptions::default());

        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "E3011"),
            "{name}: {:?}",
            result.diagnostics
        );
        assert!(
            result
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.code == "E3011"),
            "{name}: unexpected diagnostics: {:?}",
            result.diagnostics
        );

        let typst_code = lower_to_typst_code(&result.ir);
        assert!(
            typst_code.contains("Error: error Cannot call function error"),
            "{name}: {typst_code}"
        );

        with_typst(name, |backend| {
            let output = backend
                .compile(&TypstInput {
                    source: typst_code,
                    entry_path: entry,
                })
                .expect("evidenced inline explicit-error Typst must compile");
            assert!(output
                .pdf
                .expect("PDF output must be present")
                .starts_with(b"%PDF-"));
        });
    }
}

#[test]
fn integration_stacked_layouts_lower_to_valid_typst_and_pdf() {
    let source = ".row alignment:{spacebetween} cross:{stretch} gap:{10px}\n    A\n\n    B\n\n.column alignment:{spacearound} cross:{start} gap:{25%}\n    C\n\n    D\n\n.grid columns:{2} alignment:{spaceevenly} cross:{end} gap:{1cm} vgap:{2cm} hgap:{3cm}\n    E\n\n    F\n\n    G\n";
    let project = VirtualProjectBuilder::new()
        .entry("stacked.qd")
        .expect("valid entry path")
        .add_source("stacked.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "stacked diagnostics: {:?}",
        result.diagnostics
    );
    let typst_code = lower_to_typst_code(&result.ir);
    assert!(typst_code.contains("#stack(dir: ltr"), "{typst_code}");
    assert!(typst_code.contains("#stack(dir: ttb"), "{typst_code}");
    assert!(typst_code.contains("h(7.5pt)"), "{typst_code}");
    assert!(typst_code.contains("v(25%)"), "{typst_code}");
    assert!(!typst_code.contains("spacing: 7.5pt"), "{typst_code}");
    assert!(!typst_code.contains("spacing: 25%"), "{typst_code}");
    assert!(
        typst_code.contains("columns: (1fr, auto, 3cm, 1fr, auto, 1fr)"),
        "{typst_code}"
    );
    assert!(typst_code.contains("row-gutter: 2cm"), "{typst_code}");
    assert!(!typst_code.contains("column-gutter:"), "{typst_code}");
    assert!(typst_code.contains("#block(height: 100%)"), "{typst_code}");

    with_typst("stacked-layouts", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "stacked.qd".to_string(),
            })
            .expect("stacked Typst must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_effectless_non_paged_page_selector_does_not_trigger_fail_closed_guard() {
    let source = ".pageformat side:{left} orientation:{landscape}\nEffectless selector output\n";
    let project = VirtualProjectBuilder::new()
        .entry("pageformat-effectless-non-paged-selector.qd")
        .expect("valid entry path")
        .add_source("pageformat-effectless-non-paged-selector.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "effectless non-paged selector diagnostics: {:?}",
        result.diagnostics
    );

    let typst_code = lower_to_typst_code(&result.ir);
    assert!(
        !typst_code.contains(
            "Arkst cannot lower page side/pages selectors for a non-paged final document"
        ),
        "{typst_code}"
    );

    with_typst("pageformat-effectless-non-paged-selector", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "pageformat-effectless-non-paged-selector.qd".to_string(),
            })
            .expect("effectless non-paged selector Typst must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_non_paged_page_selector_fails_closed_at_typst_boundary() {
    let source = ".pageformat pages:{1..1} background:{red}\nPlain output\n";
    let project = VirtualProjectBuilder::new()
        .entry("pageformat-non-paged-selector.qd")
        .expect("valid entry path")
        .add_source("pageformat-non-paged-selector.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "non-paged selector state diagnostics: {:?}",
        result.diagnostics
    );

    let typst_code = lower_to_typst_code(&result.ir);
    assert!(
        typst_code.starts_with(
            "#panic(\"Arkst cannot lower page side/pages selectors for a non-paged final document\")\n"
        ),
        "{typst_code}"
    );

    with_typst("pageformat-non-paged-selector-fail-closed", |backend| {
        let error = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "pageformat-non-paged-selector.qd".to_string(),
            })
            .expect_err("non-paged final page selector must fail closed in Typst");
        let message = error.to_string();
        assert!(
            message.contains("page side/pages selectors for a non-paged final document"),
            "{message}"
        );
    });
}

#[test]
fn integration_slides_selector_free_page_border_defaults_fail_closed_at_typst_boundary() {
    let source = ".doctype {slides}\n\
.pageformat margin:{1cm}\n\
.pageformat bordertop:{1pt}\n\
Incomplete slide border output\n";
    let project = VirtualProjectBuilder::new()
        .entry("pageformat-slides-border-defaults.qd")
        .expect("valid entry path")
        .add_source("pageformat-slides-border-defaults.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "slides selector-free border default diagnostics: {:?}",
        result.diagnostics
    );

    let typst_code = lower_to_typst_code(&result.ir);
    assert!(
        typst_code.starts_with(
            "#panic(\"Arkst cannot lower selector-free page border without explicit margin, border widths, and border color\")\n"
        ),
        "{typst_code}"
    );

    with_typst("pageformat-slides-border-defaults-fail-closed", |backend| {
        let error = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "pageformat-slides-border-defaults.qd".to_string(),
            })
            .expect_err("incomplete selector-free slide border must fail closed");
        let message = error.to_string();
        assert!(
            message.contains(
                "selector-free page border without explicit margin, border widths, and border color"
            ),
            "{message}"
        );
    });
}

#[test]
fn integration_non_paged_selector_free_page_border_fails_closed_at_typst_boundary() {
    let source = ".pageformat margin:{1cm}\n\
.pageformat bordertop:{1pt} borderright:{2pt} borderbottom:{3pt} borderleft:{4pt} bordercolor:{red}\n\
Plain border output\n";
    let project = VirtualProjectBuilder::new()
        .entry("pageformat-non-paged-border.qd")
        .expect("valid entry path")
        .add_source("pageformat-non-paged-border.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "non-paged selector-free border diagnostics: {:?}",
        result.diagnostics
    );

    let typst_code = lower_to_typst_code(&result.ir);
    assert!(
        typst_code.starts_with(
            "#panic(\"Arkst cannot lower selector-free page border for a non-paged final document\")\n"
        ),
        "{typst_code}"
    );

    with_typst("pageformat-non-paged-border-fail-closed", |backend| {
        let error = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "pageformat-non-paged-border.qd".to_string(),
            })
            .expect_err("selector-free border on a non-paged final document must fail closed");
        let message = error.to_string();
        assert!(
            message.contains("selector-free page border for a non-paged final document"),
            "{message}"
        );
    });
}

#[test]
fn integration_page_selector_declared_before_paged_doctype_uses_final_document_type() {
    let source = ".pageformat pages:{2..2} background:{red}\n\
.doctype {paged}\n\
First page\n\
\n\
.pagebreak\n\
\n\
Second page\n";
    let project = VirtualProjectBuilder::new()
        .entry("pageformat-selector-before-paged-doctype.qd")
        .expect("valid entry path")
        .add_source("pageformat-selector-before-paged-doctype.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "selector-before-paged-doctype diagnostics: {:?}",
        result.diagnostics
    );

    let typst_code = lower_to_typst_code(&result.ir);
    assert!(
        !typst_code.starts_with(
            "#panic(\"Arkst cannot lower page side/pages selectors for a non-paged final document\")\n"
        ),
        "{typst_code}"
    );
    assert!(
        typst_code.contains("#set page(background: context {"),
        "{typst_code}"
    );
    assert!(
        typst_code.contains("__arkst_page >= 2 and __arkst_page <= 2"),
        "{typst_code}"
    );

    with_typst("pageformat-selector-before-paged-doctype", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "pageformat-selector-before-paged-doctype.qd".to_string(),
            })
            .expect("final paged selector Typst must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_selector_scoped_page_layout_fails_closed_at_typst_boundary() {
    let source = ".doctype {paged}\n\
.pageformat pages:{2..2} columns:{2}\n\
First page\n\
\n\
.pagebreak\n\
\n\
Second page\n";
    let project = VirtualProjectBuilder::new()
        .entry("pageformat-scoped-layout.qd")
        .expect("valid entry path")
        .add_source("pageformat-scoped-layout.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "selector-scoped page layout diagnostics: {:?}",
        result.diagnostics
    );

    let typst_code = lower_to_typst_code(&result.ir);
    assert!(
        typst_code.starts_with(
            "#panic(\"Arkst cannot lower selector-scoped page alignment/size/width/height/columns to Typst without selector-aware layout output\")\n"
        ),
        "{typst_code}"
    );

    with_typst("pageformat-scoped-layout-fail-closed", |backend| {
        let error = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "pageformat-scoped-layout.qd".to_string(),
            })
            .expect_err("selector-scoped page layout must fail closed in Typst");
        let message = error.to_string();
        assert!(
            message.contains("selector-scoped page alignment/size/width/height/columns"),
            "{message}"
        );
    });
}

#[test]
fn integration_selector_scoped_page_alignment_fails_closed_at_typst_boundary() {
    let source = ".doctype {paged}\n\
.pageformat pages:{2..2} alignment:{center}\n\
First page\n\
\n\
.pagebreak\n\
\n\
Second page\n";
    let project = VirtualProjectBuilder::new()
        .entry("pageformat-scoped-alignment.qd")
        .expect("valid entry path")
        .add_source("pageformat-scoped-alignment.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "selector-scoped alignment diagnostics: {:?}",
        result.diagnostics
    );

    let typst_code = lower_to_typst_code(&result.ir);
    assert!(
        typst_code.starts_with(
            "#panic(\"Arkst cannot lower selector-scoped page alignment/size/width/height/columns to Typst without selector-aware layout output\")\n"
        ),
        "{typst_code}"
    );

    with_typst("pageformat-scoped-alignment-fail-closed", |backend| {
        let error = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "pageformat-scoped-alignment.qd".to_string(),
            })
            .expect_err("selector-scoped page alignment must fail closed in Typst");
        let message = error.to_string();
        assert!(
            message.contains("selector-scoped page alignment/size/width/height/columns"),
            "{message}"
        );
    });
}

#[test]
fn integration_selector_free_page_border_defaults_fail_closed_at_typst_boundary() {
    let source = ".doctype {paged}\n\
.pageformat margin:{1cm}\n\
.pageformat bordercolor:{red}\n\
Border default unresolved\n";
    let project = VirtualProjectBuilder::new()
        .entry("pageformat-border-defaults.qd")
        .expect("valid entry path")
        .add_source("pageformat-border-defaults.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "pageformat unresolved border-default diagnostics: {:?}",
        result.diagnostics
    );

    let typst_code = lower_to_typst_code(&result.ir);
    assert!(
        typst_code.starts_with(
            "#panic(\"Arkst cannot lower selector-free page border without explicit margin, border widths, and border color\")\n"
        ),
        "{typst_code}"
    );

    with_typst("pageformat-border-defaults-fail-closed", |backend| {
        let error = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "pageformat-border-defaults.qd".to_string(),
            })
            .expect_err("unresolved selector-free border defaults must fail closed in Typst");
        let message = error.to_string();
        assert!(
            message.contains("selector-free page border without explicit margin, border widths, and border color"),
            "{message}"
        );
    });
}

#[test]
fn integration_selector_scoped_margin_fails_closed_even_with_complete_border_path() {
    let source = ".doctype {paged}\n\
.pageformat margin:{1cm}\n\
.pageformat bordertop:{1pt} borderright:{2pt} borderbottom:{3pt} borderleft:{4pt} bordercolor:{blue}\n\
.pageformat pages:{2..2} margin:{2cm}\n\
First page\n\
\n\
.pagebreak\n\
\n\
Second page\n";
    let project = VirtualProjectBuilder::new()
        .entry("pageformat-scoped-margin.qd")
        .expect("valid entry path")
        .add_source("pageformat-scoped-margin.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "selector-scoped margin diagnostics: {:?}",
        result.diagnostics
    );

    let typst_code = lower_to_typst_code(&result.ir);
    assert!(
        typst_code.starts_with(
            "#panic(\"Arkst cannot lower selector-scoped page margin to Typst content layout\")\n"
        ),
        "{typst_code}"
    );

    with_typst("pageformat-scoped-margin-fail-closed", |backend| {
        let error = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "pageformat-scoped-margin.qd".to_string(),
            })
            .expect_err("selector-scoped margin must fail closed even with a complete border path");
        let message = error.to_string();
        assert!(
            message.contains("selector-scoped page margin to Typst content layout"),
            "{message}"
        );
    });
}

#[test]
fn integration_selector_scoped_border_defaults_fail_closed_per_page() {
    let source = ".doctype {paged}\n\
.pageformat margin:{1cm}\n\
.pageformat pages:{2..2} bordercolor:{red}\n\
First page\n\
\n\
.pagebreak\n\
\n\
Second page\n";
    let project = VirtualProjectBuilder::new()
        .entry("pageformat-scoped-border-defaults.qd")
        .expect("valid entry path")
        .add_source("pageformat-scoped-border-defaults.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "selector-scoped border-default diagnostics: {:?}",
        result.diagnostics
    );

    let typst_code = lower_to_typst_code(&result.ir);
    assert!(
        typst_code.contains("let __arkst_border_requested = if __arkst_page >= 2 and __arkst_page <= 2 { true } else { false }"),
        "{typst_code}"
    );
    assert!(
        typst_code.contains(
            "panic(\"Arkst cannot lower selector-scoped page border without explicit margin, border widths, and border color\")"
        ),
        "{typst_code}"
    );

    with_typst("pageformat-scoped-border-defaults-fail-closed", |backend| {
        let error = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "pageformat-scoped-border-defaults.qd".to_string(),
            })
            .expect_err("incomplete selector-scoped border must fail closed on the selected page");
        let message = error.to_string();
        assert!(
            message.contains(
                "selector-scoped page border without explicit margin, border widths, and border color"
            ),
            "{message}"
        );
    });
}

#[test]
fn integration_pageformat_ordered_global_columns_override_stale_flattened_state() {
    let source =
        ".doctype {paged}\n.pageformat columns:{2}\n.pageformat columns:{4}\nColumn output\n";
    let project = VirtualProjectBuilder::new()
        .entry("pageformat-ordered-columns.qd")
        .expect("valid entry path")
        .add_source("pageformat-ordered-columns.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let mut result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "pageformat ordered columns diagnostics: {:?}",
        result.diagnostics
    );

    result.ir.metadata.document_state.page_columns = Some(9);

    let typst_code = lower_to_typst_code(&result.ir);
    assert!(typst_code.contains("#set page(columns: 4)"), "{typst_code}");
    assert!(
        !typst_code.contains("#set page(columns: 9)"),
        "{typst_code}"
    );

    with_typst("pageformat-ordered-columns", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "pageformat-ordered-columns.qd".to_string(),
            })
            .expect("ordered pageformat columns Typst must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_pageformat_columns_lowers_to_valid_typst_and_pdf() {
    let source = ".pageformat columns:{2}\nColumn output\n";
    let project = VirtualProjectBuilder::new()
        .entry("pageformat-columns.qd")
        .expect("valid entry path")
        .add_source("pageformat-columns.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "pageformat columns diagnostics: {:?}",
        result.diagnostics
    );

    let typst_code = lower_to_typst_code(&result.ir);
    assert!(typst_code.contains("#set page(columns: 2)"), "{typst_code}");

    with_typst("pageformat-columns", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "pageformat-columns.qd".to_string(),
            })
            .expect("pageformat columns Typst must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_pageformat_standard_size_lowers_to_valid_typst_and_pdf() {
    let source =
        ".doctype {paged}\n.pageformat size:{a4} orientation:{landscape}\nStandard size output\n";
    let project = VirtualProjectBuilder::new()
        .entry("pageformat-standard-size.qd")
        .expect("valid entry path")
        .add_source("pageformat-standard-size.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "pageformat standard-size diagnostics: {:?}",
        result.diagnostics
    );

    let typst_code = lower_to_typst_code(&result.ir);
    assert!(
        typst_code.contains("#set page(width: 297mm, height: 210mm)"),
        "{typst_code}"
    );

    with_typst("pageformat-standard-size", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "pageformat-standard-size.qd".to_string(),
            })
            .expect("pageformat standard size Typst must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_pageformat_standard_size_lowers_in_final_plain_output() {
    let source =
        ".doctype {plain}\n.pageformat size:{a4} orientation:{landscape}\nPlain standard size output\n";
    let project = VirtualProjectBuilder::new()
        .entry("pageformat-standard-size-plain.qd")
        .expect("valid entry path")
        .add_source("pageformat-standard-size-plain.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "pageformat final-plain standard-size diagnostics: {:?}",
        result.diagnostics
    );

    let typst_code = lower_to_typst_code(&result.ir);
    assert!(
        typst_code.contains("#set page(width: 297mm, height: 210mm)"),
        "{typst_code}"
    );

    with_typst("pageformat-standard-size-plain", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "pageformat-standard-size-plain.qd".to_string(),
            })
            .expect("final plain standard size Typst must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_pageformat_docs_omitted_orientation_uses_portrait_after_paged_mutation() {
    let source = ".doctype {docs}\n\
.pageformat size:{a4}\n\
.doctype {paged}\n\
Docs orientation basis output\n";
    let project = VirtualProjectBuilder::new()
        .entry("pageformat-docs-orientation-basis.qd")
        .expect("valid entry path")
        .add_source("pageformat-docs-orientation-basis.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "pageformat docs-orientation diagnostics: {:?}",
        result.diagnostics
    );

    let typst_code = lower_to_typst_code(&result.ir);
    assert!(
        typst_code.contains("#set page(width: 210mm, height: 297mm)"),
        "{typst_code}"
    );

    with_typst("pageformat-docs-orientation-basis", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "pageformat-docs-orientation-basis.qd".to_string(),
            })
            .expect("docs portrait orientation basis must lower to Typst");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_pageformat_ordered_global_dimensions_lower_to_valid_typst_and_pdf() {
    let source = ".doctype {paged}\n\
.pageformat width:{10in} height:{5in}\n\
.pageformat size:{a4} orientation:{portrait} width:{8in}\n\
Ordered dimensions output\n";
    let project = VirtualProjectBuilder::new()
        .entry("pageformat-ordered-dimensions.qd")
        .expect("valid entry path")
        .add_source("pageformat-ordered-dimensions.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "pageformat ordered-dimensions diagnostics: {:?}",
        result.diagnostics
    );

    let typst_code = lower_to_typst_code(&result.ir);
    assert!(
        typst_code.contains("#set page(width: 8in, height: 297mm)"),
        "{typst_code}"
    );
    assert!(
        !typst_code.contains("#set page(width: 10in, height: 5in)"),
        "{typst_code}"
    );

    with_typst("pageformat-ordered-dimensions", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "pageformat-ordered-dimensions.qd".to_string(),
            })
            .expect("pageformat ordered dimensions Typst must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_pageformat_paged_single_axis_uses_initial_a4_portrait_base() {
    let source = ".doctype {paged}\n.pageformat width:{8in}\nPaged default single-axis output\n";
    let project = VirtualProjectBuilder::new()
        .entry("pageformat-paged-default-single-axis.qd")
        .expect("valid entry path")
        .add_source("pageformat-paged-default-single-axis.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "pageformat paged-default single-axis diagnostics: {:?}",
        result.diagnostics
    );

    let typst_code = lower_to_typst_code(&result.ir);
    assert!(
        typst_code.contains("#set page(width: 8in, height: 297mm)"),
        "{typst_code}"
    );

    with_typst("pageformat-paged-default-single-axis", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "pageformat-paged-default-single-axis.qd".to_string(),
            })
            .expect("paged default single-axis Typst must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_pageformat_single_axis_declared_before_paged_uses_final_default() {
    let source =
        ".pageformat width:{8in}\n.doctype {paged}\nFinal paged default single-axis output\n";
    let project = VirtualProjectBuilder::new()
        .entry("pageformat-final-paged-default-single-axis.qd")
        .expect("valid entry path")
        .add_source("pageformat-final-paged-default-single-axis.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "final paged default single-axis diagnostics: {:?}",
        result.diagnostics
    );

    let typst_code = lower_to_typst_code(&result.ir);
    assert!(
        typst_code.contains("#set page(width: 8in, height: 297mm)"),
        "{typst_code}"
    );

    with_typst("pageformat-final-paged-default-single-axis", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "pageformat-final-paged-default-single-axis.qd".to_string(),
            })
            .expect("final paged default single-axis Typst must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_pageformat_unresolved_single_axis_final_plain_fails_closed() {
    let source = ".pageformat width:{8in}\nFinal plain incomplete dimensions\n";
    let project = VirtualProjectBuilder::new()
        .entry("pageformat-final-plain-single-axis.qd")
        .expect("valid entry path")
        .add_source("pageformat-final-plain-single-axis.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "final plain single-axis diagnostics: {:?}",
        result.diagnostics
    );

    let typst_code = lower_to_typst_code(&result.ir);
    assert!(
        typst_code.starts_with(
            "#panic(\"Arkst cannot lower selector-free page dimensions without complete explicit axes or a resolvable standard-size base\")\n"
        ),
        "{typst_code}"
    );

    with_typst("pageformat-final-plain-single-axis", |backend| {
        let error = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "pageformat-final-plain-single-axis.qd".to_string(),
            })
            .expect_err("final plain incomplete single-axis geometry must fail closed");
        assert!(
            error
                .to_string()
                .contains("selector-free page dimensions without complete explicit axes"),
            "{error}"
        );
    });
}

#[test]
fn integration_pageformat_paged_default_single_axis_does_not_cross_into_slides() {
    let source =
        ".doctype {paged}\n.pageformat width:{8in}\n.doctype {slides}\nCross-doctype output\n";
    let project = VirtualProjectBuilder::new()
        .entry("pageformat-paged-default-single-axis-cross-doctype.qd")
        .expect("valid entry path")
        .add_source(
            "pageformat-paged-default-single-axis-cross-doctype.qd",
            source,
        )
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "pageformat cross-doctype diagnostics: {:?}",
        result.diagnostics
    );

    let typst_code = lower_to_typst_code(&result.ir);
    assert!(
        typst_code.starts_with(
            "#panic(\"Arkst cannot lower selector-free page dimensions without complete explicit axes or a resolvable standard-size base\")\n"
        ),
        "{typst_code}"
    );

    with_typst(
        "pageformat-paged-default-single-axis-cross-doctype",
        |backend| {
            let error = backend
                .compile(&TypstInput {
                    source: typst_code,
                    entry_path: "pageformat-paged-default-single-axis-cross-doctype.qd".to_string(),
                })
                .expect_err("cross-doctype single-axis default must remain fail-closed");
            assert!(
                error
                    .to_string()
                    .contains("selector-free page dimensions without complete explicit axes"),
                "{error}"
            );
        },
    );
}

#[test]
fn integration_pageformat_global_single_axis_uses_existing_standard_size_base() {
    let source = ".doctype {paged}\n\
.pageformat size:{a4} orientation:{portrait}\n\
.pageformat width:{8in}\n\
Single-axis dimensions output\n";
    let project = VirtualProjectBuilder::new()
        .entry("pageformat-global-single-axis.qd")
        .expect("valid entry path")
        .add_source("pageformat-global-single-axis.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "pageformat global single-axis diagnostics: {:?}",
        result.diagnostics
    );

    let typst_code = lower_to_typst_code(&result.ir);
    assert!(
        typst_code.contains("#set page(width: 8in, height: 297mm)"),
        "{typst_code}"
    );

    with_typst("pageformat-global-single-axis", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "pageformat-global-single-axis.qd".to_string(),
            })
            .expect("pageformat global single-axis Typst must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_pageformat_global_single_axis_inherits_explicit_geometry_base() {
    let source = ".doctype {paged}\n\
.pageformat width:{10in} height:{5in}\n\
.pageformat size:{.none} width:{8in}\n\
Explicit-base single-axis output\n";
    let project = VirtualProjectBuilder::new()
        .entry("pageformat-global-single-axis-explicit-base.qd")
        .expect("valid entry path")
        .add_source("pageformat-global-single-axis-explicit-base.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "pageformat explicit-base single-axis diagnostics: {:?}",
        result.diagnostics
    );

    let typst_code = lower_to_typst_code(&result.ir);
    assert!(
        typst_code.contains("#set page(width: 8in, height: 5in)"),
        "{typst_code}"
    );

    with_typst("pageformat-global-single-axis-explicit-base", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "pageformat-global-single-axis-explicit-base.qd".to_string(),
            })
            .expect("explicit-base single-axis Typst must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_effectless_ordered_pageformat_does_not_revive_stale_flattened_geometry() {
    let source = ".doctype {paged}\n.pageformat\nEffectless dimension output\n";
    let project = VirtualProjectBuilder::new()
        .entry("pageformat-effectless-dimension-fallback.qd")
        .expect("valid entry path")
        .add_source("pageformat-effectless-dimension-fallback.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let mut result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "pageformat effectless dimension diagnostics: {:?}",
        result.diagnostics
    );
    assert!(
        !result
            .ir
            .metadata
            .document_state
            .page_format
            .layers
            .is_empty(),
        "effectless pageformat must retain ordered state"
    );

    result.ir.metadata.document_state.page_geometry = Some(IrPageGeometry {
        width: IrSize {
            value: 10.0,
            unit: IrSizeUnit::In,
        },
        height: IrSize {
            value: 5.0,
            unit: IrSizeUnit::In,
        },
    });

    let typst_code = lower_to_typst_code(&result.ir);
    assert!(
        !typst_code.contains("#set page(width: 10in, height: 5in)"),
        "{typst_code}"
    );

    with_typst("pageformat-effectless-dimension-fallback", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "pageformat-effectless-dimension-fallback.qd".to_string(),
            })
            .expect("effectless pageformat Typst must compile without stale dimensions");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_pageformat_global_nullable_axes_inherit_existing_dimensions() {
    let source = ".doctype {paged}\n\
.pageformat width:{10in} height:{5in}\n\
.pageformat width:{.none} height:{4in}\n\
.pageformat width:{8in} height:{.none}\n\
Nullable axis output\n";
    let project = VirtualProjectBuilder::new()
        .entry("pageformat-nullable-axis-inheritance.qd")
        .expect("valid entry path")
        .add_source("pageformat-nullable-axis-inheritance.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "pageformat nullable-axis diagnostics: {:?}",
        result.diagnostics
    );

    let typst_code = lower_to_typst_code(&result.ir);
    assert!(
        typst_code.contains("#set page(width: 8in, height: 4in)"),
        "{typst_code}"
    );
    assert!(
        !typst_code.contains("#set page(width: 10in, height: 5in)"),
        "{typst_code}"
    );

    with_typst("pageformat-nullable-axis-inheritance", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "pageformat-nullable-axis-inheritance.qd".to_string(),
            })
            .expect("nullable page-axis inheritance Typst must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_pageformat_explicit_border_lowers_on_slides_to_valid_typst_and_pdf() {
    let source = ".doctype {slides}\n\
.pageformat margin:{1cm 2cm 3cm 4cm}\n\
.pageformat bordertop:{1pt} borderright:{2pt} borderbottom:{3pt} borderleft:{4pt} bordercolor:{red}\n\
Slide border output\n";
    let project = VirtualProjectBuilder::new()
        .entry("pageformat-slides-border.qd")
        .expect("valid entry path")
        .add_source("pageformat-slides-border.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "pageformat slides border diagnostics: {:?}",
        result.diagnostics
    );

    let typst_code = lower_to_typst_code(&result.ir);
    assert!(
        typst_code.contains("#set page(foreground: place("),
        "{typst_code}"
    );
    assert!(
        typst_code.contains("width: (100% - 4cm - 2cm)"),
        "{typst_code}"
    );
    assert!(
        typst_code.contains("top: (paint: rgb(255, 0, 0, 100%), thickness: 1pt)"),
        "{typst_code}"
    );

    with_typst("pageformat-slides-border", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "pageformat-slides-border.qd".to_string(),
            })
            .expect("pageformat slides border Typst must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_pageformat_explicit_border_lowers_to_valid_typst_and_pdf() {
    let source = ".doctype {paged}\n\
.pageformat margin:{1cm 2cm 3cm 4cm}\n\
.pageformat bordertop:{1pt} borderright:{2pt} borderbottom:{3pt} borderleft:{4pt} bordercolor:{red}\n\
Border output\n";
    let project = VirtualProjectBuilder::new()
        .entry("pageformat-border.qd")
        .expect("valid entry path")
        .add_source("pageformat-border.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "pageformat border diagnostics: {:?}",
        result.diagnostics
    );

    let typst_code = lower_to_typst_code(&result.ir);
    assert!(
        typst_code.contains("#set page(foreground: place("),
        "{typst_code}"
    );
    assert!(
        typst_code.contains("width: (100% - 4cm - 2cm)"),
        "{typst_code}"
    );
    assert!(
        typst_code.contains("top: (paint: rgb(255, 0, 0, 100%), thickness: 1pt)"),
        "{typst_code}"
    );

    with_typst("pageformat-border", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "pageformat-border.qd".to_string(),
            })
            .expect("pageformat border Typst must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_pageformat_ordered_global_background_overrides_stale_flattened_state() {
    let source = ".doctype {paged}\n.pageformat background:{blue}\nBackground output\n";
    let project = VirtualProjectBuilder::new()
        .entry("pageformat-ordered-background.qd")
        .expect("valid entry path")
        .add_source("pageformat-ordered-background.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let mut result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "pageformat ordered background diagnostics: {:?}",
        result.diagnostics
    );

    result.ir.metadata.document_state.page_background = Some(IrColor {
        red: 9,
        green: 9,
        blue: 9,
        alpha: 1.0,
    });

    let typst_code = lower_to_typst_code(&result.ir);
    assert!(
        typst_code.contains("#set page(fill: rgb(0, 0, 255, 100%))"),
        "{typst_code}"
    );
    assert!(!typst_code.contains("rgb(9, 9, 9, 100%)"), "{typst_code}");

    with_typst("pageformat-ordered-background", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "pageformat-ordered-background.qd".to_string(),
            })
            .expect("ordered pageformat background Typst must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_pageformat_background_lowers_to_valid_typst_and_pdf() {
    let source = ".pageformat background:{blue}\nBackground output\n";
    let project = VirtualProjectBuilder::new()
        .entry("pageformat-background.qd")
        .expect("valid entry path")
        .add_source("pageformat-background.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "pageformat background diagnostics: {:?}",
        result.diagnostics
    );

    let typst_code = lower_to_typst_code(&result.ir);
    assert!(
        typst_code.contains("#set page(fill: rgb(0, 0, 255, 100%))"),
        "{typst_code}"
    );

    with_typst("pageformat-background", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "pageformat-background.qd".to_string(),
            })
            .expect("pageformat background Typst must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_pageformat_range_background_lowers_to_valid_typst_and_pdf() {
    let source = ".doctype {paged}\n\
.pageformat background:{blue}\n\
.pageformat pages:{2..2} background:{red}\n\
First page\n\
\n\
.pagebreak\n\
\n\
Second page\n";
    let project = VirtualProjectBuilder::new()
        .entry("pageformat-range-background.qd")
        .expect("valid entry path")
        .add_source("pageformat-range-background.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "pageformat range background diagnostics: {:?}",
        result.diagnostics
    );

    let typst_code = lower_to_typst_code(&result.ir);
    assert!(
        typst_code.contains("#set page(background: context {"),
        "{typst_code}"
    );
    assert!(
        typst_code.contains("let __arkst_page = here().page()"),
        "{typst_code}"
    );
    assert!(
        typst_code.contains("__arkst_page >= 2 and __arkst_page <= 2"),
        "{typst_code}"
    );
    assert!(
        typst_code.contains("fill: rgb(255, 0, 0, 100%)"),
        "{typst_code}"
    );
    assert!(
        typst_code.contains("fill: rgb(0, 0, 255, 100%)"),
        "{typst_code}"
    );

    with_typst("pageformat-range-background", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "pageformat-range-background.qd".to_string(),
            })
            .expect("pageformat range background Typst must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_pageformat_ordered_global_border_overrides_stale_flattened_state() {
    let source = ".doctype {paged}\n\
.pageformat margin:{1cm 2cm 3cm 4cm}\n\
.pageformat bordertop:{1pt} borderright:{2pt} borderbottom:{3pt} borderleft:{4pt} bordercolor:{red}\n\
Border output\n";
    let project = VirtualProjectBuilder::new()
        .entry("pageformat-ordered-border.qd")
        .expect("valid entry path")
        .add_source("pageformat-ordered-border.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let mut result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "pageformat ordered border diagnostics: {:?}",
        result.diagnostics
    );

    result.ir.metadata.document_state.page_margin = Some(IrPageMargins {
        top: IrSize {
            value: 9.0,
            unit: IrSizeUnit::Pt,
        },
        right: IrSize {
            value: 9.0,
            unit: IrSizeUnit::Pt,
        },
        bottom: IrSize {
            value: 9.0,
            unit: IrSizeUnit::Pt,
        },
        left: IrSize {
            value: 9.0,
            unit: IrSizeUnit::Pt,
        },
    });
    result.ir.metadata.document_state.page_border_widths = Some(IrPageBorderWidths {
        top: IrSize {
            value: 9.0,
            unit: IrSizeUnit::Pt,
        },
        right: IrSize {
            value: 9.0,
            unit: IrSizeUnit::Pt,
        },
        bottom: IrSize {
            value: 9.0,
            unit: IrSizeUnit::Pt,
        },
        left: IrSize {
            value: 9.0,
            unit: IrSizeUnit::Pt,
        },
    });
    result.ir.metadata.document_state.page_border_color = Some(IrColor {
        red: 9,
        green: 9,
        blue: 9,
        alpha: 1.0,
    });

    let typst_code = lower_to_typst_code(&result.ir);
    assert!(typst_code.contains("dx: 4cm"), "{typst_code}");
    assert!(typst_code.contains("dy: 1cm"), "{typst_code}");
    assert!(
        typst_code.contains("top: (paint: rgb(255, 0, 0, 100%), thickness: 1pt)"),
        "{typst_code}"
    );
    assert!(!typst_code.contains("rgb(9, 9, 9, 100%)"), "{typst_code}");
    assert!(!typst_code.contains("dx: 9pt"), "{typst_code}");

    with_typst("pageformat-ordered-border", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "pageformat-ordered-border.qd".to_string(),
            })
            .expect("ordered pageformat border Typst must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_pageformat_range_border_lowers_to_valid_typst_and_pdf() {
    let source = ".doctype {paged}\n\
.pageformat margin:{1cm}\n\
.pageformat bordertop:{1pt} borderright:{2pt} borderbottom:{3pt} borderleft:{4pt} bordercolor:{blue}\n\
.pageformat pages:{2..2} bordercolor:{red}\n\
First page\n\
\n\
.pagebreak\n\
\n\
Second page\n";
    let project = VirtualProjectBuilder::new()
        .entry("pageformat-range-border.qd")
        .expect("valid entry path")
        .add_source("pageformat-range-border.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "pageformat range border diagnostics: {:?}",
        result.diagnostics
    );

    let typst_code = lower_to_typst_code(&result.ir);
    assert!(
        typst_code.contains("#set page(foreground: context {"),
        "{typst_code}"
    );
    assert!(
        typst_code.contains("let __arkst_page = here().page()"),
        "{typst_code}"
    );
    assert!(
        typst_code.contains("__arkst_page >= 2 and __arkst_page <= 2"),
        "{typst_code}"
    );
    assert!(typst_code.contains("rgb(255, 0, 0, 100%)"), "{typst_code}");
    assert!(typst_code.contains("rgb(0, 0, 255, 100%)"), "{typst_code}");
    assert!(
        typst_code.contains("thickness: __arkst_border_widths.left"),
        "{typst_code}"
    );
    assert!(
        typst_code.contains("let __arkst_border_requested ="),
        "{typst_code}"
    );

    with_typst("pageformat-range-border", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "pageformat-range-border.qd".to_string(),
            })
            .expect("pageformat range border Typst must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_pageformat_side_decoration_lowers_to_valid_typst_and_pdf() {
    let source = ".doctype {paged}\n\
.pageformat margin:{1cm}\n\
.pageformat bordertop:{1pt} borderright:{2pt} borderbottom:{3pt} borderleft:{4pt} bordercolor:{blue} background:{blue}\n\
.pageformat side:{left} pages:{2..2} bordercolor:{green} background:{red}\n\
First page\n\
\n\
.pagebreak\n\
\n\
Second page\n";
    let project = VirtualProjectBuilder::new()
        .entry("pageformat-side-decoration.qd")
        .expect("valid entry path")
        .add_source("pageformat-side-decoration.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "pageformat side decoration diagnostics: {:?}",
        result.diagnostics
    );

    let typst_code = lower_to_typst_code(&result.ir);
    let combined = "calc.even(__arkst_page) and __arkst_page >= 2 and __arkst_page <= 2";
    assert!(typst_code.contains(combined), "{typst_code}");
    assert!(
        typst_code.contains("#set page(background: context {"),
        "{typst_code}"
    );
    assert!(
        typst_code.contains("#set page(foreground: context {"),
        "{typst_code}"
    );
    assert!(
        typst_code.contains("fill: rgb(255, 0, 0, 100%)"),
        "{typst_code}"
    );
    assert!(typst_code.contains("rgb(0, 128, 0, 100%)"), "{typst_code}");

    with_typst("pageformat-side-decoration", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "pageformat-side-decoration.qd".to_string(),
            })
            .expect("pageformat side decoration Typst must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_pageformat_ordered_global_margin_overrides_stale_flattened_state() {
    let source = ".doctype {paged}\n.pageformat margin:{1cm 2mm 3pt 8px}\nMargin output\n";
    let project = VirtualProjectBuilder::new()
        .entry("pageformat-ordered-margin.qd")
        .expect("valid entry path")
        .add_source("pageformat-ordered-margin.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let mut result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "pageformat ordered margin diagnostics: {:?}",
        result.diagnostics
    );

    result.ir.metadata.document_state.page_margin = Some(IrPageMargins {
        top: IrSize {
            value: 9.0,
            unit: IrSizeUnit::Pt,
        },
        right: IrSize {
            value: 9.0,
            unit: IrSizeUnit::Pt,
        },
        bottom: IrSize {
            value: 9.0,
            unit: IrSizeUnit::Pt,
        },
        left: IrSize {
            value: 9.0,
            unit: IrSizeUnit::Pt,
        },
    });

    let typst_code = lower_to_typst_code(&result.ir);
    assert!(
        typst_code.contains("#set page(margin: (top: 1cm, right: 2mm, bottom: 3pt, left: 6pt))"),
        "{typst_code}"
    );
    assert!(!typst_code.contains("top: 9pt"), "{typst_code}");

    with_typst("pageformat-ordered-margin", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "pageformat-ordered-margin.qd".to_string(),
            })
            .expect("ordered pageformat margin Typst must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_pageformat_margin_lowers_to_valid_typst_and_pdf() {
    let source = ".pageformat margin:{1cm 2mm 3pt 8px}\nMargin output\n";
    let project = VirtualProjectBuilder::new()
        .entry("pageformat-margin.qd")
        .expect("valid entry path")
        .add_source("pageformat-margin.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "pageformat margin diagnostics: {:?}",
        result.diagnostics
    );

    let typst_code = lower_to_typst_code(&result.ir);
    assert!(
        typst_code.contains("#set page(margin: (top: 1cm, right: 2mm, bottom: 3pt, left: 6pt))"),
        "{typst_code}"
    );

    with_typst("pageformat-margin", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "pageformat-margin.qd".to_string(),
            })
            .expect("pageformat margin Typst must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_v260_ordered_global_alignment_overrides_stale_flattened_state() {
    let source =
        ".row\n    A\n\n    B\n\n.pageformat alignment:{center}\n.column\n    C\n\n    D\n";
    let project = VirtualProjectBuilder::new()
        .entry("v260-ordered-stack-inherit.qd")
        .expect("valid entry path")
        .add_source("v260-ordered-stack-inherit.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let mut result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "v2.6 ordered stack inheritance diagnostics: {:?}",
        result.diagnostics
    );

    result.ir.metadata.document_state.page_alignment = Some(IrDocumentAlignment::End);

    let typst_code = lower_to_typst_code(&result.ir);
    assert_eq!(typst_code.matches("h(1fr)").count(), 2, "{typst_code}");
    assert_eq!(typst_code.matches("v(1fr)").count(), 2, "{typst_code}");

    with_typst("v260-ordered-stack-alignment-inheritance", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "v260-ordered-stack-inherit.qd".to_string(),
            })
            .expect("ordered inherited stack Typst must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_v260_omitted_stack_alignment_uses_final_page_state() {
    let source =
        ".row\n    A\n\n    B\n\n.pageformat alignment:{center}\n.column\n    C\n\n    D\n";
    let project = VirtualProjectBuilder::new()
        .entry("v260-stack-inherit.qd")
        .expect("valid entry path")
        .add_source("v260-stack-inherit.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "v2.6 stack inheritance diagnostics: {:?}",
        result.diagnostics
    );
    let typst_code = lower_to_typst_code(&result.ir);
    assert_eq!(typst_code.matches("h(1fr)").count(), 2, "{typst_code}");
    assert_eq!(typst_code.matches("v(1fr)").count(), 2, "{typst_code}");

    with_typst("v260-stack-alignment-inheritance", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "v260-stack-inherit.qd".to_string(),
            })
            .expect("inherited stack Typst must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_pageformat_nullable_alignment_preserves_final_page_state() {
    let source = ".row\n    A\n\n    B\n\n.pageformat alignment:{center}\n.pageformat alignment:{.none}\n.column\n    C\n\n    D\n";
    let project = VirtualProjectBuilder::new()
        .entry("pageformat-nullable-alignment.qd")
        .expect("valid entry path")
        .add_source("pageformat-nullable-alignment.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "nullable page-alignment diagnostics: {:?}",
        result.diagnostics
    );
    let typst_code = lower_to_typst_code(&result.ir);
    assert_eq!(typst_code.matches("h(1fr)").count(), 2, "{typst_code}");
    assert_eq!(typst_code.matches("v(1fr)").count(), 2, "{typst_code}");

    with_typst("pageformat-nullable-alignment", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "pageformat-nullable-alignment.qd".to_string(),
            })
            .expect("nullable page-alignment Typst must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_center_layout_lowers_to_valid_typst_and_pdf() {
    let source = ".center\n    Hello\n\n    .row\n        A\n\n        B\n";
    let project = VirtualProjectBuilder::new()
        .entry("center.qd")
        .expect("valid entry path")
        .add_source("center.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "center diagnostics: {:?}",
        result.diagnostics
    );
    let typst_code = lower_to_typst_code(&result.ir);
    assert!(typst_code.contains("#block(width: 100%)"), "{typst_code}");
    assert!(typst_code.contains("#align(center)"), "{typst_code}");
    assert!(typst_code.contains("#stack(dir: ltr"), "{typst_code}");
    assert!(typst_code.find("Hello").unwrap() < typst_code.find('A').unwrap());
    assert!(typst_code.find('A').unwrap() < typst_code.find('B').unwrap());

    with_typst("center-layout", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "center.qd".to_string(),
            })
            .expect("center Typst must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_landscape_layout_lowers_to_valid_typst_and_pdf() {
    let source = ".landscape\n    ## Wide section\n\n    .row gap:{1cm}\n        Left\n\n        Center\n\n        Right\n";
    let project = VirtualProjectBuilder::new()
        .entry("landscape.qd")
        .expect("valid entry path")
        .add_source("landscape.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "landscape diagnostics: {:?}",
        result.diagnostics
    );
    let [IrNode::Component {
        component: IrComponent::Landscape(landscape),
    }] = result.ir.nodes.as_slice()
    else {
        panic!("expected landscape root, got {:?}", result.ir.nodes);
    };
    assert!(matches!(
        landscape.children.as_slice(),
        [
            IrNode::Heading { .. },
            IrNode::Component {
                component: IrComponent::Stacked(_)
            }
        ]
    ));

    let typst_code = lower_to_typst_code(&result.ir);
    assert!(
        typst_code.contains("#rotate(-90deg, reflow: true)"),
        "{typst_code}"
    );
    assert!(typst_code.contains("#stack(dir: ltr"), "{typst_code}");
    assert!(!typst_code.contains("page(flipped: true)"), "{typst_code}");

    with_typst("landscape-layout", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "landscape.qd".to_string(),
            })
            .expect("landscape Typst must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_align_layout_lowers_to_valid_typst_and_pdf() {
    let source = ".align {end}\n    Hello\n\n    .row\n        A\n\n        B\n";
    let project = VirtualProjectBuilder::new()
        .entry("align.qd")
        .expect("valid entry path")
        .add_source("align.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "align diagnostics: {:?}",
        result.diagnostics
    );
    let typst_code = lower_to_typst_code(&result.ir);
    assert!(typst_code.contains("#block(width: 100%)"), "{typst_code}");
    assert!(typst_code.contains("#align(end)"), "{typst_code}");
    assert!(typst_code.contains("#stack(dir: ltr"), "{typst_code}");
    assert!(typst_code.find("Hello").unwrap() < typst_code.find('A').unwrap());
    assert!(typst_code.find('A').unwrap() < typst_code.find('B').unwrap());

    with_typst("align-layout", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "align.qd".to_string(),
            })
            .expect("align Typst must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_container_sizing_lowers_to_valid_typst_and_pdf() {
    let source = ".row\n    .container width:{4cm}\n        ## Left\n        Text\n\n    .container fullwidth:{yes}\n        Right\n";
    let project = VirtualProjectBuilder::new()
        .entry("container.qd")
        .expect("valid entry path")
        .add_source("container.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "container diagnostics: {:?}",
        result.diagnostics
    );
    let [IrNode::Component {
        component: IrComponent::Stacked(row),
    }] = result.ir.nodes.as_slice()
    else {
        panic!("expected typed row, got {:?}", result.ir.nodes);
    };
    assert_eq!(row.children.len(), 2);
    assert!(row.children.iter().all(|child| matches!(
        child,
        IrNode::Component {
            component: IrComponent::Container(_)
        }
    )));

    let typst_code = lower_to_typst_code(&result.ir);
    assert!(typst_code.contains("#block(width: 4cm)"), "{typst_code}");
    assert!(typst_code.contains("#block(width: 100%)"), "{typst_code}");

    with_typst("container-sizing", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "container.qd".to_string(),
            })
            .expect("container Typst must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_whitespace_lowers_to_lossless_typst_and_pdf() {
    let source = "A .whitespace B\n\n.whitespace width:{2cm}\n\n.whitespace height:{2cm}\n\n.whitespace width:{2cm} height:{1cm}\n";
    let project = VirtualProjectBuilder::new()
        .entry("whitespace.qd")
        .expect("valid entry path")
        .add_source("whitespace.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "whitespace diagnostics: {:?}",
        result.diagnostics
    );

    let (typst_code, source_map) = lower_to_typst(&result.ir);
    assert!(typst_code.contains('\u{a0}'), "{typst_code:?}");
    assert!(
        typst_code.contains("#box(width: 2cm, height: 0pt)[]"),
        "{typst_code}"
    );
    assert!(
        typst_code.contains("#box(width: 0pt, height: 2cm)[]"),
        "{typst_code}"
    );
    assert!(
        typst_code.contains("#box(width: 2cm, height: 1cm)[]"),
        "{typst_code}"
    );
    assert!(
        source
            .match_indices(".whitespace")
            .map(|(start, _)| start)
            .all(|start| source_map.iter().any(|entry| entry.original.start == start)),
        "whitespace lowering lost source provenance: {source_map:?}"
    );

    with_typst("whitespace", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "whitespace.qd".to_string(),
            })
            .expect("whitespace Typst must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_br_uses_existing_hard_break_lowering_and_pdf() {
    let source = "before .br after\n";
    let project = VirtualProjectBuilder::new()
        .entry("br.qd")
        .expect("valid entry path")
        .add_source("br.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "br diagnostics: {:?}",
        result.diagnostics
    );

    let (typst_code, source_map) = lower_to_typst(&result.ir);
    assert!(typst_code.contains("before "), "{typst_code}");
    assert!(typst_code.contains("\\\n"), "{typst_code}");
    assert!(!typst_code.contains("#br"), "{typst_code}");
    let call_start = source.find(".br").expect("call span");
    assert!(
        source_map
            .iter()
            .any(|entry| entry.original.start == call_start),
        "br lowering lost call provenance: {source_map:?}"
    );

    with_typst("br-line-break", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "br.qd".to_string(),
            })
            .expect("br Typst must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_self_contained_mode_does_not_expose_temp_resources() {
    with_typst("self-contained-resource-boundary", |backend| {
        let result = backend.compile(&TypstInput {
            source: "#read(\"./resource.txt\")\n".to_string(),
            entry_path: "test.qd".to_string(),
        });
        assert!(
            result.is_err(),
            "resources require an explicit source context"
        );
    });
}

#[test]
fn integration_relative_image_uses_project_source_context() {
    with_typst("relative-image", |backend| {
        let project = tempdir().expect("project temp directory");
        let docs = project.path().join("docs");
        let assets = docs.join("assets");
        fs::create_dir_all(&assets).expect("asset directory");
        fs::write(docs.join("main.qd"), "original source\n").expect("source fixture");
        fs::write(
            assets.join("tiny.svg"),
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="10pt" height="10pt" viewBox="0 0 10 10"><rect width="10" height="10" fill="red"/></svg>"#,
        )
        .expect("SVG fixture");

        let output = backend
            .with_source_context(TypstSourceContext::new(project.path()))
            .compile(&TypstInput {
                source: "#image(\"./assets/tiny.svg\")\n".to_string(),
                entry_path: "docs/main.qd".to_string(),
            })
            .expect("relative SVG should compile");
        assert!(output.pdf.is_some_and(|pdf| pdf.starts_with(b"%PDF-")));
        assert!(!docs.join("main.typ").exists());
        assert!(!project.path().join("output.pdf").exists());
        assert_eq!(
            fs::read_to_string(docs.join("main.qd")).unwrap(),
            "original source\n"
        );
    });
}

#[test]
fn integration_markdown_images_compile_from_source_relative_paths_to_pdf() {
    with_typst("markdown-images-e2e", |backend| {
        let project_root = tempdir().expect("project temp directory");
        let docs = project_root.path().join("docs");
        let assets = docs.join("assets");
        let shared = project_root.path().join("shared");
        fs::create_dir_all(&assets).expect("asset directory");
        fs::create_dir_all(&shared).expect("shared asset directory");
        fs::write(
            docs.join("guide.md"),
            "# Image Test\n\nBefore.\n\n![Square](./assets/square.svg)\n\n![Shared](../shared/logo.svg \"Logo\")\n\n![Pixel](./assets/pixel.png)\n\nAfter.\n",
        )
        .expect("Markdown source fixture");
        fs::write(
            assets.join("square.svg"),
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 32 32"><rect width="32" height="32"/></svg>"#,
        )
        .expect("SVG fixture");
        fs::write(
            shared.join("logo.svg"),
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><circle cx="8" cy="8" r="8"/></svg>"#,
        )
        .expect("parent-relative SVG fixture");
        fs::write(
            assets.join("pixel.png"),
            [
                0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48,
                0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x01, 0x00, 0x00, 0x00,
                0x00, 0x37, 0x6e, 0xf9, 0x24, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, 0x54, 0x08,
                0xd7, 0x63, 0x60, 0x00, 0x00, 0x00, 0x02, 0x00, 0x01, 0xe2, 0x21, 0xbc, 0x33, 0x00,
                0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
            ],
        )
        .expect("PNG fixture");

        let source = fs::read_to_string(docs.join("guide.md")).expect("read Markdown fixture");
        let project = VirtualProjectBuilder::new()
            .entry("docs/guide.md")
            .expect("valid entry")
            .add_source("docs/guide.md", source)
            .expect("valid source")
            .build()
            .expect("valid project");
        let result = compile(&project, &CompileOptions::default());
        assert!(
            result.diagnostics.is_empty(),
            "unexpected: {:?}",
            result.diagnostics
        );

        let typst = lower_to_typst_code(&result.ir);
        assert!(typst.contains("#image(\"./assets/square.svg\")"));
        assert!(typst.contains("#image(\"../shared/logo.svg\")"));
        assert!(typst.contains("#image(\"./assets/pixel.png\")"));

        let output = backend
            .with_source_context(TypstSourceContext::new(project_root.path()))
            .compile(&TypstInput {
                source: typst,
                entry_path: "docs/guide.md".to_string(),
            })
            .expect("Markdown images should compile through Typst");
        assert!(output.pdf.is_some_and(|pdf| pdf.starts_with(b"%PDF-")));
    });
}

#[test]
fn integration_included_markdown_image_fails_before_subprocess_boundary() {
    let project = VirtualProjectBuilder::new()
        .entry("docs/main.qd")
        .expect("valid entry")
        .add_source("docs/main.qd", ".include {part.md}\n")
        .expect("valid main source")
        .add_source(
            "docs/part.md",
            "# Included image\n\n![Square](assets/square.svg)\n",
        )
        .expect("valid included source")
        .add_asset(
            "docs/assets/square.svg",
            br#"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32"><rect width="32" height="32"/></svg>"#.to_vec(),
        )
        .expect("valid asset")
        .build()
        .expect("valid project");
    let included_source_id = project
        .sources()
        .get_id(&VirtualPathBuf::parse("docs/part.md").expect("valid logical path"))
        .expect("included source exists");

    let result = compile(&project, &CompileOptions::default());

    assert_eq!(result.diagnostics.len(), 1, "{:?}", result.diagnostics);
    assert_eq!(result.diagnostics[0].code, "E8001");
    assert!(result.diagnostics[0].message.contains("image"));
    assert_eq!(
        result.diagnostics[0].primary.map(|span| span.source_id),
        Some(included_source_id)
    );
    assert!(!result.diagnostics[0].message.contains("/Users/"));
    assert!(!result.diagnostics[0].message.contains(r"\Users\"));

    // #182 owns Quarkdown image/media production. Until that producer exists,
    // compiler diagnostics deliberately stop this path before Typst subprocess use.
}

#[test]
fn integration_missing_image_is_a_typst_resource_failure() {
    with_typst("missing-image", |backend| {
        let project_root = tempdir().expect("project temp directory");
        let docs = project_root.path().join("docs");
        fs::create_dir_all(&docs).expect("docs directory");
        let result = backend
            .with_source_context(TypstSourceContext::new(project_root.path()))
            .compile(&TypstInput {
                source: "#image(\"./assets/missing.svg\")\n".to_string(),
                entry_path: "docs/guide.md".to_string(),
            })
            .expect_err("missing image must fail closed");
        let error = result.to_string();
        assert!(error.contains("Typst compilation failed"), "error: {error}");
        assert!(!error.contains(project_root.path().to_string_lossy().as_ref()));
    });
}

#[test]
fn integration_image_path_escape_is_rejected_by_project_root() {
    with_typst("image-boundary", |backend| {
        let parent = tempdir().expect("project parent directory");
        let project_root = parent.path().join("project");
        let outside = parent.path().join("outside");
        fs::create_dir_all(project_root.join("docs")).expect("project docs");
        fs::create_dir_all(&outside).expect("outside directory");
        fs::write(outside.join("secret.svg"), "not an image").expect("outside fixture");

        let result = backend
            .with_source_context(TypstSourceContext::new(&project_root))
            .compile(&TypstInput {
                source: "#image(\"../../outside/secret.svg\")\n".to_string(),
                entry_path: "docs/guide.md".to_string(),
            })
            .expect_err("image path escape must fail closed");
        let error = result.to_string();
        assert!(error.contains("Typst compilation failed"), "error: {error}");
        assert!(error.contains("project"), "error: {error}");
        assert!(
            !error.contains("not an image"),
            "error leaked resource: {error}"
        );
        assert!(!error.contains(parent.path().to_string_lossy().as_ref()));
    });
}

#[cfg(unix)]
#[test]
fn integration_image_symlink_escape_is_rejected_before_typst() {
    use std::os::unix::fs::symlink;

    with_typst("image-symlink-boundary", |backend| {
        let parent = tempdir().expect("project parent directory");
        let project_root = parent.path().join("project");
        let outside = parent.path().join("outside");
        fs::create_dir_all(project_root.join("docs/assets")).expect("project assets");
        fs::create_dir_all(&outside).expect("outside directory");
        fs::write(outside.join("secret.svg"), "secret").expect("outside fixture");
        symlink(
            outside.join("secret.svg"),
            project_root.join("docs/assets/leak.svg"),
        )
        .expect("image symlink");

        let result = backend
            .with_source_context(TypstSourceContext::new(&project_root))
            .compile(&TypstInput {
                source: "#image(\"./assets/leak.svg\")\n".to_string(),
                entry_path: "docs/guide.md".to_string(),
            })
            .expect_err("image symlink escape must fail closed");
        assert!(matches!(
            result,
            TypstError::ResourceBoundaryViolation(path)
                if path == "docs/assets/leak.svg"
        ));
    });
}

#[test]
fn integration_relative_read_does_not_depend_on_temp_directory() {
    with_typst("relative-read", |backend| {
        let project = tempdir().expect("project temp directory");
        let docs = project.path().join("docs");
        let assets = docs.join("assets");
        fs::create_dir_all(&assets).expect("asset directory");
        fs::write(assets.join("resource.txt"), "project resource").expect("resource fixture");

        let output = backend
            .with_source_context(TypstSourceContext::new(project.path()))
            .compile(&TypstInput {
                source: "#read(\"./assets/resource.txt\")\n".to_string(),
                entry_path: "docs/main.qd".to_string(),
            })
            .expect("project resource should compile without a temp resource");
        assert!(output.pdf.is_some_and(|pdf| pdf.starts_with(b"%PDF-")));
    });
}

#[test]
fn integration_relative_import_uses_project_source_context() {
    with_typst("relative-import", |backend| {
        let project = tempdir().expect("project temp directory");
        let docs = project.path().join("docs");
        let partials = docs.join("partials");
        fs::create_dir_all(&partials).expect("partial directory");
        fs::write(
            partials.join("helper.typ"),
            "#let greeting = [Imported successfully]\n",
        )
        .expect("Typst partial fixture");

        let output = backend
            .with_source_context(TypstSourceContext::new(project.path()))
            .compile(&TypstInput {
                source: "#import \"./partials/helper.typ\": greeting\n#greeting\n".to_string(),
                entry_path: "docs/main.qd".to_string(),
            })
            .expect("relative Typst import should compile");
        assert!(output.pdf.is_some_and(|pdf| pdf.starts_with(b"%PDF-")));
    });
}

#[test]
fn integration_generated_entry_does_not_shadow_typst_resource() {
    with_typst("generated-entry-collision", |backend| {
        let project = tempdir().expect("project temp directory");
        let docs = project.path().join("docs");
        fs::create_dir_all(&docs).expect("docs directory");
        fs::write(
            docs.join("main.typ"),
            "#let greeting = [Source helper remains visible]\n",
        )
        .expect("source Typst helper fixture");

        let output = backend
            .with_source_context(TypstSourceContext::new(project.path()))
            .compile(&TypstInput {
                source: "#import \"./main.typ\": greeting\n#greeting\n".to_string(),
                entry_path: "docs/main.qd".to_string(),
            })
            .expect("source Typst helper should not be shadowed");
        assert!(output.pdf.is_some_and(|pdf| pdf.starts_with(b"%PDF-")));
    });
}

#[test]
fn integration_context_handles_spaces_and_unicode_paths() {
    with_typst("context-paths", |backend| {
        let parent = tempdir().expect("project parent temp directory");
        let project = parent.path().join("project with spaces");
        let docs = project.join("문서");
        let assets = docs.join("자산");
        fs::create_dir_all(&assets).expect("unicode asset directory");
        fs::write(
            assets.join("logo.svg"),
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="10pt" height="10pt"><circle cx="5" cy="5" r="5"/></svg>"#,
        )
        .expect("unicode SVG fixture");

        let output = backend
            .with_source_context(TypstSourceContext::new(&project))
            .compile(&TypstInput {
                source: "#image(\"./자산/logo.svg\")\n".to_string(),
                entry_path: "문서/main.qd".to_string(),
            })
            .expect("paths with spaces and Unicode should compile");
        assert!(output.pdf.is_some_and(|pdf| pdf.starts_with(b"%PDF-")));
    });
}

#[test]
fn integration_outside_root_resource_fails_closed() {
    with_typst("outside-root", |backend| {
        let parent = tempdir().expect("project parent temp directory");
        let project = parent.path().join("project");
        let outside = parent.path().join("outside");
        fs::create_dir_all(project.join("docs")).expect("project directory");
        fs::create_dir_all(&outside).expect("outside directory");
        fs::write(outside.join("secret.txt"), "secret content").expect("outside fixture");

        let result = backend
            .with_source_context(TypstSourceContext::new(&project))
            .compile(&TypstInput {
                source: "#read(\"../../outside/secret.txt\")\n".to_string(),
                entry_path: "docs/main.qd".to_string(),
            });
        let error = result
            .expect_err("outside-root access must fail")
            .to_string();
        assert!(error.contains("Typst compilation failed"), "error: {error}");
        assert!(
            error.contains("project root") || error.contains("project sandbox"),
            "error must identify the project boundary: {error}"
        );
        assert!(
            !error.contains("secret content"),
            "error leaked content: {error}"
        );
        let parent_path = parent.path().to_string_lossy();
        assert!(
            !error.contains(parent_path.as_ref()),
            "error leaked host path: {error}"
        );
    });
}

#[test]
fn target_specific_html_is_omitted_without_typst_source_or_source_map_entries() {
    let source = "Before .html {<em>hidden inline</em>} after.\n\n.html {<div>hidden block</div>}\n\nAfter.\n";
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("valid path")
        .add_source("main.qd", source)
        .expect("valid source")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "unexpected: {:?}",
        result.diagnostics
    );

    let target_span = result
        .ir
        .nodes
        .iter()
        .find_map(|node| match node {
            IrNode::Paragraph { content, .. } => content.iter().find_map(|inline| match inline {
                IrInline::TargetSpecificContent { content }
                    if content.target == NativeTarget::Html =>
                {
                    Some(content.span)
                }
                _ => None,
            }),
            IrNode::TargetSpecificContent { content } if content.target == NativeTarget::Html => {
                Some(content.span)
            }
            _ => None,
        })
        .expect("inline target-specific node");
    assert!(matches!(
        result.ir.nodes.as_slice(),
        [
            IrNode::Paragraph { .. },
            IrNode::TargetSpecificContent { .. },
            IrNode::Paragraph { .. }
        ]
    ));

    let (typst, source_map) = lower_to_typst(&result.ir);
    assert!(typst.contains("Before"), "generated Typst: {typst:?}");
    assert!(typst.contains("after."), "generated Typst: {typst:?}");
    assert!(typst.contains("After."), "generated Typst: {typst:?}");
    assert!(!typst.contains("<em>"));
    assert!(!typst.contains("hidden"));
    assert!(
        source_map.iter().all(|entry| entry.original != target_span),
        "target-specific HTML fabricated a source-map entry: {source_map:?}"
    );
}

#[test]
fn target_specific_html_typst_and_pdf_smoke() {
    let source = "Before.\n\n.html {<div>hidden</div>}\n\nAfter.\n";
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("valid path")
        .add_source("main.qd", source)
        .expect("valid source")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "unexpected: {:?}",
        result.diagnostics
    );
    let typst = lower_to_typst_code(&result.ir);
    assert!(typst.contains("Before."));
    assert!(typst.contains("After."));
    assert!(!typst.contains("<div>"));
    assert!(!typst.contains("hidden"));

    with_typst("target-specific-html", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst,
                entry_path: "main.qd".to_string(),
            })
            .expect("Typst/PDF compilation should succeed");
        assert!(output.pdf.is_some_and(|pdf| pdf.starts_with(b"%PDF-")));
    });
}

#[test]
fn unknown_function_html_body_stays_fail_closed_before_typst() {
    let source = ".unknown\n    <div>not owned</div>\n";
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("valid path")
        .add_source("main.qd", source)
        .expect("valid source")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());

    assert!(result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "E8001"));
    let typst = lower_to_typst_code(&result.ir);
    assert!(!typst.contains("<div>"));
    assert!(!typst.contains("not owned"));
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
    // Arkst source whose first list item contains a paragraph followed by
    // a fenced code block; the generated Typst must keep the code block
    // inside the item (fences on the item's content column).
    use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};
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
    let typst_code = arkst_typst::lowering::lower_to_typst_code(&result.ir);
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
    use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};

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
        let typst_code = arkst_typst::lowering::lower_to_typst_code(&result.ir);
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
    let typst_code = arkst_typst::lowering::lower_to_typst_code(&result.ir);
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
    use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};

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
    let typst_code = arkst_typst::lowering::lower_to_typst_code(&result.ir);
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
fn integration_markdown_html_comments_are_semantic_noops_in_typst_and_pdf() {
    use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};

    let source = "Before <!-- hidden inline --> visible.\n\n<!-- hidden block -->\n\nAfter.\n";
    let entry = "comments.md";
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

    let typst_code = arkst_typst::lowering::lower_to_typst_code(&result.ir);
    assert!(typst_code.contains("Before"));
    assert!(typst_code.contains("visible."));
    assert!(typst_code.contains("After."));
    assert!(!typst_code.contains("<!--"));
    assert!(!typst_code.contains("hidden inline"));
    assert!(!typst_code.contains("hidden block"));

    with_typst(entry, |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: entry.to_string(),
            })
            .expect("comment-free Markdown Typst must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_commonmark_gfm_baseline_fixture_compiles_to_valid_pdf() {
    use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};

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
    let typst_code = arkst_typst::lowering::lower_to_typst_code(&result.ir);
    assert!(typst_code.contains("= Arkst Markdown baseline"));
    assert!(typst_code.contains("== Setext heading"));
    assert!(typst_code.contains("#link(\"https://example.com/docs\")"));
    assert!(typst_code.contains("#strike[strikethrough]"));
    assert!(typst_code.contains("```rust\nfn main()"));
    assert!(typst_code.contains("#table("));
    assert!(typst_code.contains("☐ open task"));
    assert!(typst_code.contains("☑ completed task"));
    assert!(result.ir.nodes.iter().any(|node| matches!(
        node,
        arkst_core::ir::IrNode::CodeBlock {
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
    use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};

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
    let typst_code = arkst_typst::lowering::lower_to_typst_code(&result.ir);
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
    use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};

    // Test variable declaration and reference
    let source = ".var {name} {Arkst}\nHello .name\n";
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

    let typst_code = arkst_typst::lowering::lower_to_typst_code(&result.ir);
    // Variable should be resolved in the output
    assert!(
        typst_code.contains("Arkst"),
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
    let source = ".var {name} {**Arkst**}\nHello .name\n";
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
        "unexpected diagnostics for source-backed rich content: {:?}",
        result.diagnostics
    );

    let typst_code = arkst_typst::lowering::lower_to_typst_code(&result.ir);
    assert!(
        typst_code.contains("Arkst"),
        "source-backed rich content should reach the existing lowering path: {}",
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
    use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};

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
    let typst_code = arkst_typst::lowering::lower_to_typst_code(&result.ir);
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
    use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};

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

    let typst_code = arkst_typst::lowering::lower_to_typst_code(&result.ir);
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
    let nested_typst_code = arkst_typst::lowering::lower_to_typst_code(&nested_result.ir);
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
    use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};

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

    let direct_typst = arkst_typst::lowering::lower_to_typst_code(&direct.ir);
    let nested_typst = arkst_typst::lowering::lower_to_typst_code(&nested.ir);
    let chained_typst = arkst_typst::lowering::lower_to_typst_code(&chained.ir);
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
    use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};

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

    let typst_code = arkst_typst::lowering::lower_to_typst_code(&result.ir);
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
    use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};

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

    let typst_code = arkst_typst::lowering::lower_to_typst_code(&result.ir);
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
