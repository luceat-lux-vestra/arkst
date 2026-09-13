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
    if Command::new("typst")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
    {
        return Some(PathBuf::from("typst"));
    }
    let homebrew = PathBuf::from("/opt/homebrew/bin/typst");
    homebrew.is_file().then_some(homebrew)
}

#[test]
fn v260_code_callouts_lower_to_typst_that_compiles_to_pdf() {
    let source =
        ".code lang:{rust} callouts:{\n    - 2: Second\n    - 1: First\n}\n    alpha\n    beta\n";
    let project = VirtualProjectBuilder::new()
        .entry("code-callouts.qd")
        .expect("valid entry")
        .add_source("code-callouts.qd", source)
        .expect("valid source")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        result.diagnostics
    );

    let typst_code = lower_to_typst_code(&result.ir);
    assert!(typst_code.contains("#grid("), "{typst_code}");
    assert!(typst_code.contains("#enum("), "{typst_code}");
    assert!(typst_code.contains("[First]"), "{typst_code}");
    assert!(typst_code.contains("[Second]"), "{typst_code}");

    let Some(path) = find_typst() else {
        let message = "code callout integration test requires a Typst executable";
        if std::env::var_os("ARKST_REQUIRE_TYPST").is_some() {
            panic!("{message}");
        }
        eprintln!("[integration] {message}; skipping");
        return;
    };

    let output = SubprocessBackend::new(path)
        .compile(&TypstInput {
            source: typst_code,
            entry_path: "code-callouts.qd".into(),
        })
        .expect("generated code-callout Typst must compile");
    let pdf = output.pdf.expect("PDF output must be present");
    assert!(pdf.starts_with(b"%PDF-"), "output must be a PDF");
}
