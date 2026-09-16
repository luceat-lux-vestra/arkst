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

fn compile_focus(document_type: &str) -> String {
    let source = format!(
        ".doctype {{{document_type}}}\n.theme {{paperwhite}} layout:{{focus}}\n\n# Probe Title\n\nLead paragraph.\n\n## Section\n\n.code lang:{{text}}\n    hello\n"
    );
    let project = VirtualProjectBuilder::new()
        .entry("focus.qd")
        .expect("valid entry")
        .add_source("focus.qd", source)
        .expect("valid source")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "focus source diagnostics: {:?}",
        result.diagnostics
    );
    assert_eq!(
        result
            .ir
            .metadata
            .document_state
            .theme
            .as_ref()
            .and_then(|theme| theme.layout.as_deref()),
        Some("focus")
    );
    lower_to_typst_code(&result.ir)
}

#[test]
fn focus_plain_and_slides_lower_to_real_typst_pdf() {
    for document_type in ["plain", "slides"] {
        let typst = compile_focus(document_type);
        let focus = typst
            .find("// Arkst Quarkdown v2.6 focus layout\n")
            .expect("focus prelude");
        if document_type == "plain" {
            assert_eq!(focus, 0, "plain focus must preserve the existing output prefix: {typst}");
            assert!(
                !typst.contains("#set page(width: 749.04pt, height: 546pt)"),
                "plain focus must not acquire slides page geometry: {typst}"
            );
        } else {
            let page = typst
                .find("#set page(width: 749.04pt, height: 546pt)\n")
                .expect("slides page prelude");
            assert!(
                page < focus,
                "slides page geometry must be established before focus layout: {typst}"
            );
        }
        assert!(typst.contains("#show heading.where(level: 1)"), "{typst}");
        assert!(typst.contains("#show heading.where(level: 2)"), "{typst}");
        assert!(typst.contains("fill: rgb(27, 24, 24, 90%)"), "{typst}");

        with_typst(&format!("focus-{document_type}"), |backend| {
            let output = backend
                .compile(&TypstInput {
                    source: typst,
                    entry_path: "focus.qd".to_string(),
                })
                .expect("focus Typst must compile");
            let pdf = output.pdf.expect("PDF output must be present");
            assert!(pdf.starts_with(b"%PDF-"), "invalid PDF prefix");
        });
    }
}

#[test]
fn focus_paged_keeps_existing_typst_output_path() {
    let typst = compile_focus("paged");
    assert!(
        !typst.contains("Arkst Quarkdown v2.6 focus layout"),
        "paged black-box evidence did not justify a focus prelude: {typst}"
    );

    with_typst("focus-paged-control", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst,
                entry_path: "focus.qd".to_string(),
            })
            .expect("paged control Typst must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}
