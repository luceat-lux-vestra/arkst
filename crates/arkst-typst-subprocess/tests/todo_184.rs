use std::path::PathBuf;
use std::process::Command;

use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};
use arkst_typst::lowering::lower_to_typst_code;
use arkst_typst::{TypstBackend, TypstInput};
use arkst_typst_subprocess::SubprocessBackend;

#[test]
fn todo_warning_box_compiles_to_real_typst_pdf() {
    let project = VirtualProjectBuilder::new()
        .entry("todo.qd")
        .expect("entry")
        .add_source("todo.qd", ".todo {Ship this}\n")
        .expect("source")
        .build()
        .expect("project");
    let result = compile(&project, &CompileOptions::default());
    assert!(result.diagnostics.is_empty(), "{result:?}");

    let source = lower_to_typst_code(&result.ir);
    assert!(
        source.contains("#block(width: 100%, inset: 8pt, stroke: 1pt)["),
        "{source}"
    );
    assert!(source.contains("#strong[TO DO]"), "{source}");
    assert!(source.contains("Ship this"), "{source}");

    let path = if let Some(path) = std::env::var_os("ARKST_TYPST_PATH") {
        let path = PathBuf::from(path);
        path.is_file().then_some(path)
    } else if Command::new("typst")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
    {
        Some(PathBuf::from("typst"))
    } else {
        let homebrew = PathBuf::from("/opt/homebrew/bin/typst");
        homebrew.is_file().then_some(homebrew)
    };
    let Some(path) = path else {
        if std::env::var_os("ARKST_REQUIRE_TYPST").is_some() {
            panic!("Todo integration requires pinned Typst");
        }
        eprintln!("[integration] Typst unavailable; skipping PDF check");
        return;
    };

    let output = SubprocessBackend::new(path)
        .compile(&TypstInput {
            source,
            entry_path: "todo.qd".into(),
        })
        .expect("Todo warning box must compile");
    assert!(output.pdf.expect("PDF output").starts_with(b"%PDF-"));
}
