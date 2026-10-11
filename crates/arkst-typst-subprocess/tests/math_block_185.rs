use std::path::PathBuf;
use std::process::Command;

use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};
use arkst_typst::lowering::lower_to_typst_code;
use arkst_typst::{TypstBackend, TypstInput};
use arkst_typst_subprocess::SubprocessBackend;

#[test]
fn explicit_and_inferred_block_math_compile_to_real_typst_pdf() {
    let mut typst_sources = Vec::new();
    for (input, expected) in [
        (".math {x^2 + y} block:{true}\n", "$ x^2 + y $"),
        (".math\n    a + b\n", "$ a + b $"),
    ] {
        let project = VirtualProjectBuilder::new()
            .entry("math.qd")
            .expect("entry")
            .add_source("math.qd", input)
            .expect("source")
            .build()
            .expect("project");
        let result = compile(&project, &CompileOptions::default());
        assert!(result.diagnostics.is_empty(), "{result:?}");
        let source = lower_to_typst_code(&result.ir);
        assert!(source.contains(expected), "{source}");
        typst_sources.push(source);
    }

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
            panic!("math block integration requires pinned Typst");
        }
        eprintln!("[integration] Typst unavailable; skipping PDF check");
        return;
    };
    for source in typst_sources {
        let output = SubprocessBackend::new(path.clone())
            .compile(&TypstInput {
                source,
                entry_path: "math.qd".into(),
            })
            .expect("block math must compile");
        assert!(output.pdf.expect("PDF output").starts_with(b"%PDF-"));
    }
}
