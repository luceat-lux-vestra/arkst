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

fn lower_source(source: &str) -> String {
    let project = VirtualProjectBuilder::new()
        .entry("footer.qd")
        .expect("valid entry")
        .add_source("footer.qd", source)
        .expect("valid source")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    lower_to_typst_code(&result.ir)
}

#[test]
fn paged_footer_uses_persistent_page_query_and_compiles_to_pdf() {
    let typst = lower_source(
        ".doctype {paged}\n.footer\n    First .currentpage\n\nBody\n\n.footer\n    Second\n\n.pagebreak\n\nNext\n",
    );

    assert!(typst.contains("#set page(footer: context {"), "{typst}");
    assert!(typst.contains("let arkst-page = here().page()"), "{typst}");
    assert!(
        typst.contains(
            "query(<arkst-page-margin-bottom-center>).filter(it => it.location().page() <= arkst-page)"
        ),
        "{typst}"
    );
    assert!(typst.contains("arkst-footers.last().value"), "{typst}");
    assert_eq!(
        typst.matches("<arkst-page-margin-bottom-center>").count(),
        3,
        "one query plus two authored footer markers expected: {typst}"
    );
    assert!(
        typst.contains("#metadata([First #context {"),
        "footer keeps structured current-page content inside the marker: {typst}"
    );
    assert!(typst.contains("#metadata([Second"), "{typst}");

    with_typst("paged-footer", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst,
                entry_path: "footer.qd".to_string(),
            })
            .expect("bounded paged footer output must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn non_paged_footer_output_fails_closed_in_the_bounded_slice() {
    for document_type in ["plain", "slides", "docs"] {
        let doctype = if document_type == "plain" {
            String::new()
        } else {
            format!(".doctype {{{document_type}}}\n")
        };
        let typst = lower_source(&format!("{doctype}.footer\n    Footer\n\nBody\n"));
        assert!(
            typst.contains(
                "#panic(\"Arkst bounded footer output currently supports only final paged documents\")"
            ),
            "{document_type}: {typst}"
        );
        assert!(
            !typst.contains("#metadata([Footer]) <arkst-page-margin-bottom-center>"),
            "{document_type}: unsupported output must not publish a footer marker: {typst}"
        );

        with_typst(&format!("{document_type}-footer-fail-closed"), |backend| {
            let error = backend
                .compile(&TypstInput {
                    source: typst,
                    entry_path: "footer.qd".to_string(),
                })
                .expect_err("unsupported bounded footer output must fail")
                .to_string();
            assert!(
                error.contains("bounded footer output currently supports only final paged"),
                "{document_type}: {error}"
            );
        });
    }
}
