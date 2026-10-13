use std::path::PathBuf;
use std::process::Command;

use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};
use arkst_typst::lowering::lower_to_typst_code;
use arkst_typst::{TypstBackend, TypstInput};
use arkst_typst_subprocess::SubprocessBackend;

fn compile_source(source: &str) -> String {
    let project = VirtualProjectBuilder::new()
        .entry("collapse.qd")
        .expect("entry")
        .add_source("collapse.qd", source)
        .expect("source")
        .build()
        .expect("project");
    let result = compile(&project, &CompileOptions::default());
    assert!(result.diagnostics.is_empty(), "{result:?}");
    lower_to_typst_code(&result.ir)
}

#[test]
fn collapse_static_initial_state_compiles_to_real_typst_pdf() {
    let closed = compile_source(".collapse {Summary}\n    Hidden body\n");
    assert!(closed.contains("Summary"), "{closed}");
    assert!(!closed.contains("Hidden body"), "{closed}");

    let open = compile_source(".collapse {Summary} open:{yes}\n    Visible body\n");
    assert!(open.contains("Summary"), "{open}");
    assert!(open.contains("Visible body"), "{open}");

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
            panic!("Collapse integration requires pinned Typst");
        }
        eprintln!("[integration] Typst unavailable; skipping PDF check");
        return;
    };

    for (source, name) in [(closed, "closed.qd"), (open, "open.qd")] {
        let output = SubprocessBackend::new(path.clone())
            .compile(&TypstInput {
                source,
                entry_path: name.into(),
            })
            .expect("Collapse initial state must compile");
        assert!(output.pdf.expect("PDF output").starts_with(b"%PDF-"));
    }
}
