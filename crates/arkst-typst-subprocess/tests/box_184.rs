use std::path::PathBuf;
use std::process::Command;

use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};
use arkst_typst::lowering::lower_to_typst_code;
use arkst_typst::{TypstBackend, TypstInput};
use arkst_typst_subprocess::SubprocessBackend;

#[test]
fn default_callout_box_compiles_to_real_typst_pdf() {
    let project = VirtualProjectBuilder::new()
        .entry("box.qd")
        .expect("entry")
        .add_source("box.qd", ".box {Ship this}\n")
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
    assert!(source.contains("Ship this"), "{source}");
    assert!(!source.contains("TO DO"), "{source}");

    let path = if Command::new("typst")
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
            panic!("Box integration requires pinned Typst");
        }
        eprintln!("[integration] Typst unavailable; skipping PDF check");
        return;
    };

    let output = SubprocessBackend::new(path)
        .compile(&TypstInput {
            source,
            entry_path: "box.qd".into(),
        })
        .expect("default callout box must compile");
    assert!(output.pdf.expect("PDF output").starts_with(b"%PDF-"));
}
