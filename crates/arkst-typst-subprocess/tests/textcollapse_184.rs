use std::path::PathBuf;
use std::process::Command;

use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};
use arkst_typst::lowering::lower_to_typst;
use arkst_typst::{TypstBackend, TypstInput};
use arkst_typst_subprocess::SubprocessBackend;

fn compile_source(source: &str) -> String {
    let project = VirtualProjectBuilder::new()
        .entry("textcollapse.qd")
        .expect("entry")
        .add_source("textcollapse.qd", source)
        .expect("source")
        .build()
        .expect("project");
    let result = compile(&project, &CompileOptions::default());
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let (typst, source_map) = lower_to_typst(&result.ir);
    assert!(source_map.iter().any(|entry| {
        source
            .get(entry.original.start..entry.original.end)
            .is_some_and(|original| original.starts_with(".textcollapse"))
            && entry.generated_start < entry.generated_end
            && entry.generated_end <= typst.len()
    }));
    typst
}

#[test]
fn textcollapse_static_initial_state_compiles_to_real_typst_pdf() {
    let closed = compile_source("Before .textcollapse {Expanded} {Summary} after.\n");
    assert!(closed.contains("Summary"), "{closed}");
    assert!(!closed.contains("Expanded"), "{closed}");

    let open =
        compile_source("Before .textcollapse {Expanded **content**} {Summary} open:{yes} after.\n");
    assert!(open.contains("Expanded"), "{open}");
    assert!(open.contains("*content*"), "{open}");
    assert!(!open.contains("Summary"), "{open}");

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
            panic!("TextCollapse integration requires pinned Typst");
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
            .expect("TextCollapse initial state must compile");
        assert!(output.pdf.expect("PDF output").starts_with(b"%PDF-"));
    }
}
