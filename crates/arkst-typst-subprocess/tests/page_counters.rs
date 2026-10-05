use std::path::PathBuf;
use std::process::Command;

use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};
use arkst_typst::lowering::lower_to_typst_code;
use arkst_typst::{TypstBackend, TypstInput};
use arkst_typst_subprocess::SubprocessBackend;

fn find_typst() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("ARKST_TYPST_PATH") {
        let path = PathBuf::from(path);
        return path.is_file().then_some(path);
    }
    let probe = Command::new("typst").arg("--version").output();
    if probe.is_ok_and(|output| output.status.success()) {
        return Some(PathBuf::from("typst"));
    }
    let homebrew = PathBuf::from("/opt/homebrew/bin/typst");
    homebrew.is_file().then_some(homebrew)
}

fn with_typst<F>(name: &str, body: F)
where
    F: FnOnce(SubprocessBackend),
{
    match find_typst() {
        Some(path) => body(SubprocessBackend::new(path)),
        None if std::env::var("ARKST_REQUIRE_TYPST").is_ok() => {
            panic!("{name}: ARKST_REQUIRE_TYPST is set but no Typst executable was found")
        }
        None => eprintln!("[integration] {name}: no Typst executable found; skipping"),
    }
}

fn lower_document(document_type: &str) -> String {
    let doctype = if document_type == "plain" {
        String::new()
    } else {
        format!(".doctype {{{document_type}}}\n")
    };
    let source =
        format!("{doctype}Page .currentpage of .totalpages.\n\n.pagebreak\n\nNext .currentpage.\n");
    let project = VirtualProjectBuilder::new()
        .entry("page-counters.qd")
        .expect("valid entry")
        .add_source("page-counters.qd", source)
        .expect("valid source")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "{document_type} diagnostics: {:?}",
        result.diagnostics
    );
    lower_to_typst_code(&result.ir)
}

#[test]
fn paged_and_slides_page_counters_lower_to_typst_logical_page_counters() {
    for document_type in ["paged", "slides"] {
        let typst = lower_document(document_type);
        assert!(
            typst.contains("#context counter(page).get().first()"),
            "{document_type}: {typst}"
        );
        assert!(
            typst.contains("#context counter(page).final().first()"),
            "{document_type}: {typst}"
        );

        with_typst(&format!("{document_type}-page-counters"), |backend| {
            let output = backend
                .compile(&TypstInput {
                    source: typst,
                    entry_path: "page-counters.qd".to_string(),
                })
                .expect("logical page-counter output must compile");
            assert!(output
                .pdf
                .expect("PDF output must be present")
                .starts_with(b"%PDF-"));
        });
    }
}

#[test]
fn plain_and_docs_keep_the_pinned_unresolved_page_counter_placeholder() {
    for document_type in ["plain", "docs"] {
        let typst = lower_document(document_type);
        assert!(
            !typst.contains("counter(page)"),
            "{document_type} must not synthesize page runtime semantics: {typst}"
        );
        assert!(typst.contains("Page - of -."), "{document_type}: {typst}");
        assert!(typst.contains("Next -."), "{document_type}: {typst}");

        with_typst(
            &format!("{document_type}-page-counter-placeholder"),
            |backend| {
                let output = backend
                    .compile(&TypstInput {
                        source: typst,
                        entry_path: "page-counters.qd".to_string(),
                    })
                    .expect("plain/docs placeholder output must compile");
                assert!(output
                    .pdf
                    .expect("PDF output must be present")
                    .starts_with(b"%PDF-"));
            },
        );
    }
}


fn lower_marker_document(document_type: &str) -> String {
    let doctype = if document_type == "plain" {
        String::new()
    } else {
        format!(".doctype {{{document_type}}}\n")
    };
    let source = format!(
        "{doctype}Before .currentpage.\n.formatpagenumber {{i}}\nAfter .currentpage.\n.resetpagenumber start:{{7}}\n.resetpagenumber {{0}}\nSame .currentpage.\n\n.pagebreak\n\nNext .currentpage.\n"
    );
    let project = VirtualProjectBuilder::new()
        .entry("page-number-markers.qd")
        .expect("valid entry")
        .add_source("page-number-markers.qd", source)
        .expect("valid source")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "{document_type} diagnostics: {:?}",
        result.diagnostics
    );
    lower_to_typst_code(&result.ir)
}

#[test]
fn paged_and_slides_page_number_markers_lower_to_page_level_query_state_machine() {
    for document_type in ["paged", "slides"] {
        let typst = lower_marker_document(document_type);

        let first_current = typst.find("#context {").expect("current-page query");
        let format_marker = typst
            .find("#metadata(\"i\") <arkst-page-number-format>")
            .expect("format marker");
        assert!(
            first_current < format_marker,
            "marker intentionally follows the first current-page call in source: {typst}"
        );

        assert!(typst.contains(
            "query(<arkst-page-number-format>).filter(it => it.location().page() <= arkst-page)"
        ));
        assert!(typst.contains(
            "query(<arkst-page-number-reset>).filter(it => it.location().page() <= arkst-page and it.value > 0)"
        ));
        assert!(typst.contains("arkst-formats.last().value"));
        assert!(typst.contains("arkst-resets.last()"));
        assert!(typst.contains("#metadata(7) <arkst-page-number-reset>"));
        assert!(typst.contains("#metadata(0) <arkst-page-number-reset>"));
        assert!(typst.contains("str.from-unicode(96 + arkst-number)"));
        assert!(typst.contains("str.from-unicode(64 + arkst-number)"));
        assert!(typst.contains("numbering(\"i\", arkst-number)"));
        assert!(typst.contains("numbering(\"I\", arkst-number)"));
        assert!(typst.contains("else { arkst-format }"));

        with_typst(&format!("{document_type}-page-number-markers"), |backend| {
            let output = backend
                .compile(&TypstInput {
                    source: typst,
                    entry_path: "page-number-markers.qd".to_string(),
                })
                .expect("page-number marker output must compile");
            assert!(output
                .pdf
                .expect("PDF output must be present")
                .starts_with(b"%PDF-"));
        });
    }
}

#[test]
fn plain_and_docs_do_not_synthesize_page_number_marker_runtime() {
    for document_type in ["plain", "docs"] {
        let typst = lower_marker_document(document_type);
        assert!(!typst.contains("arkst-page-number-format"), "{typst}");
        assert!(!typst.contains("arkst-page-number-reset"), "{typst}");
        assert!(!typst.contains("query(<arkst-page-number"), "{typst}");
        assert!(typst.contains("Before -."), "{typst}");
        assert!(typst.contains("After -."), "{typst}");
        assert!(typst.contains("Same -."), "{typst}");
        assert!(typst.contains("Next -."), "{typst}");

        with_typst(
            &format!("{document_type}-page-number-marker-placeholder"),
            |backend| {
                let output = backend
                    .compile(&TypstInput {
                        source: typst,
                        entry_path: "page-number-markers.qd".to_string(),
                    })
                    .expect("plain/docs marker-free placeholder output must compile");
                assert!(output
                    .pdf
                    .expect("PDF output must be present")
                    .starts_with(b"%PDF-"));
            },
        );
    }
}
