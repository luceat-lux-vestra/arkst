use std::path::PathBuf;
use std::process::Command;

use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};
use arkst_typst::lowering::lower_to_typst_code;
use arkst_typst::{TypstBackend, TypstInput};
use arkst_typst_subprocess::SubprocessBackend;

#[test]
fn keybinding_generates_typed_keycaps_and_real_typst_pdf() {
    let project = VirtualProjectBuilder::new()
        .entry("keys.qd")
        .expect("entry")
        .add_source("keys.qd", "Press .keybinding {Cmd+Shift+K} to continue.\n")
        .expect("source")
        .build()
        .expect("project");
    let result = compile(&project, &CompileOptions::default());
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let source = lower_to_typst_code(&result.ir);
    assert_eq!(source.matches("#box(stroke: 0.5pt").count(), 3, "{source}");

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
            panic!("keybinding integration requires pinned Typst");
        }
        eprintln!("[integration] Typst unavailable; skipping PDF check");
        return;
    };
    let output = SubprocessBackend::new(path)
        .compile(&TypstInput {
            source,
            entry_path: "keys.qd".into(),
        })
        .expect("keybinding source must compile");
    assert!(output.pdf.expect("PDF output").starts_with(b"%PDF-"));
}
