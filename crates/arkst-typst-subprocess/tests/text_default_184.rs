use std::path::PathBuf;
use std::process::Command;

use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};
use arkst_typst::lowering::lower_to_typst_code;
use arkst_typst::{TypstBackend, TypstInput};
use arkst_typst_subprocess::SubprocessBackend;

#[test]
fn default_text_inline_content_compiles_into_real_typst_pdf() {
    let project = VirtualProjectBuilder::new()
        .entry("text.qd")
        .expect("entry")
        .add_source("text.qd", "before .text {hello} after\n")
        .expect("source")
        .build()
        .expect("project");
    let result = compile(&project, &CompileOptions::default());
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let source = lower_to_typst_code(&result.ir);
    assert!(source.contains("before "), "{source}");
    assert!(source.contains("hello"), "{source}");
    assert!(source.contains(" after"), "{source}");

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
            panic!("default .text integration requires pinned Typst");
        }
        eprintln!("[integration] Typst unavailable; skipping PDF check");
        return;
    };
    let output = SubprocessBackend::new(path)
        .compile(&TypstInput {
            source,
            entry_path: "text.qd".into(),
        })
        .expect("default .text must compile");
    assert!(output.pdf.expect("PDF output").starts_with(b"%PDF-"));
}
