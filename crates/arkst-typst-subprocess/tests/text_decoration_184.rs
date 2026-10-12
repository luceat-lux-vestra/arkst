use std::path::PathBuf;
use std::process::Command;

use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};
use arkst_typst::lowering::lower_to_typst_code;
use arkst_typst::{TypstBackend, TypstInput};
use arkst_typst_subprocess::SubprocessBackend;

#[test]
fn bounded_strikethrough_decoration_compiles_to_real_pdf() {
    let inputs = [
        (
            ".text decoration:{strikethrough}\n    Hello world\n",
            "#strike[Hello world]",
        ),
        (
            ".text {Hello} {https://example.org/docs} weight:{bold} style:{italic} decoration:{strikethrough}\n",
            "#link(\"https://example.org/docs\")[#strike[#emph[*Hello*]]]",
        ),
    ];
    let mut sources = Vec::new();
    for (input, expected) in inputs {
        let project = VirtualProjectBuilder::new()
            .entry("text.qd")
            .expect("entry")
            .add_source("text.qd", input)
            .expect("source")
            .build()
            .expect("project");
        let result = compile(&project, &CompileOptions::default());
        assert!(result.diagnostics.is_empty(), "{result:?}");
        let typst = lower_to_typst_code(&result.ir);
        assert!(typst.contains(expected), "{typst}");
        sources.push(typst);
    }

    let executable = if let Some(path) = std::env::var_os("ARKST_TYPST_PATH") {
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
    let Some(executable) = executable else {
        if std::env::var_os("ARKST_REQUIRE_TYPST").is_some() {
            panic!("strikethrough integration requires pinned Typst");
        }
        eprintln!("[integration] Typst unavailable; skipping PDF check");
        return;
    };
    for source in sources {
        let output = SubprocessBackend::new(executable.clone())
            .compile(&TypstInput {
                source,
                entry_path: "text.qd".into(),
            })
            .expect("strikethrough must compile");
        assert!(output.pdf.expect("PDF output").starts_with(b"%PDF-"));
    }
}
