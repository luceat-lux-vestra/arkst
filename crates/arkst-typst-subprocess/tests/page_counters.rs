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
    let source = format!(
        "{doctype}Page .currentpage of .totalpages.\n\n.pagebreak\n\nNext .currentpage.\n"
    );
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
