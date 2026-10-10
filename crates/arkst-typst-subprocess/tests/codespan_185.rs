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
fn codespan_lowers_to_escaped_typst_raw_and_compiles_to_pdf() {
    let project = VirtualProjectBuilder::new()
        .entry("codespan.qd")
        .expect("entry")
        .add_source("codespan.qd", "before .codespan {alpha} after\n")
        .expect("source")
        .build()
        .expect("project");
    let result = compile(&project, &CompileOptions::default());
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let typst_code = lower_to_typst_code(&result.ir);
    assert!(typst_code.contains("#raw(\"alpha\")"), "{typst_code}");

    let Some(path) = find_typst() else {
        let message = "codespan integration requires a Typst executable";
        if std::env::var_os("ARKST_REQUIRE_TYPST").is_some() {
            panic!("{message}");
        }
        eprintln!("[integration] {message}; skipping");
        return;
    };
    let output = SubprocessBackend::new(path)
        .compile(&TypstInput {
            source: typst_code,
            entry_path: "codespan.qd".into(),
        })
        .expect("code-span Typst output must compile");
    assert!(output.pdf.expect("PDF output").starts_with(b"%PDF-"));
}
