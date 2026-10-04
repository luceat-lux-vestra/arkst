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

fn compile_alignment(document_type: &str, alignment: &str) -> String {
    let source = format!(
        ".doctype {{{document_type}}}\n.pageformat alignment:{{{alignment}}}\n\n# Heading\n\nParagraph body.\n"
    );
    let project = VirtualProjectBuilder::new()
        .entry("alignment.qd")
        .expect("valid entry")
        .add_source("alignment.qd", source)
        .expect("valid source")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "{document_type}/{alignment} diagnostics: {:?}",
        result.diagnostics
    );
    lower_to_typst_code(&result.ir)
}

#[test]
fn selector_free_global_alignment_lowers_for_non_slide_documents_and_compiles_pdf() {
    for document_type in ["plain", "paged", "docs"] {
        for (alignment, typst_alignment) in
            [("start", "start"), ("center", "center"), ("end", "end")]
        {
            let typst = compile_alignment(document_type, alignment);
            assert!(
                typst.contains(&format!("#set align({typst_alignment})\n")),
                "{document_type}/{alignment}: {typst}"
            );
            assert!(
                !typst.contains("#set par(justify: true)"),
                "{document_type}/{alignment}: {typst}"
            );

            with_typst(
                &format!("pageformat-{document_type}-{alignment}-alignment"),
                |backend| {
                    let output = backend
                        .compile(&TypstInput {
                            source: typst,
                            entry_path: "alignment.qd".to_string(),
                        })
                        .expect("global alignment Typst must compile");
                    assert!(output
                        .pdf
                        .expect("PDF output must be present")
                        .starts_with(b"%PDF-"));
                },
            );
        }
    }
}

#[test]
fn justify_remains_local_and_slides_global_horizontal_alignment_remains_bounded() {
    let justify = compile_alignment("paged", "justify");
    assert!(justify.contains("#set par(justify: true)\n"), "{justify}");
    assert!(!justify.contains("#set align(start)\n"), "{justify}");
    assert!(!justify.contains("#set align(center)\n"), "{justify}");
    assert!(!justify.contains("#set align(end)\n"), "{justify}");

    for alignment in ["start", "center", "end"] {
        let slides = compile_alignment("slides", alignment);
        assert!(
            !slides.contains("#set align(start)\n")
                && !slides.contains("#set align(center)\n")
                && !slides.contains("#set align(end)\n"),
            "slides/{alignment}: {slides}"
        );
    }
}
