use std::path::PathBuf;
use std::process::Command;

use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};
use arkst_typst::lowering::lower_to_typst_code;
use arkst_typst::{TypstBackend, TypstInput};
use arkst_typst_subprocess::SubprocessBackend;

#[test]
fn bold_text_and_linked_bold_text_compile_to_real_pdf() {
    let sources = [
        (".text weight:{bold}\n    Hello world\n", "*Hello world*"),
        (
            ".text weight:{bold} url:{https://example.org/docs}\n    Hello\n",
            "#link(\"https://example.org/docs\")",
        ),
    ];
    let typst_sources = sources
        .iter()
        .map(|(input, expected)| {
            let project = VirtualProjectBuilder::new()
                .entry("text.qd")
                .expect("entry")
                .add_source("text.qd", *input)
                .expect("source")
                .build()
                .expect("project");
            let result = compile(&project, &CompileOptions::default());
            assert!(result.diagnostics.is_empty(), "{result:?}");
            let source = lower_to_typst_code(&result.ir);
            assert!(source.contains(expected), "{source}");
            assert!(source.contains("*Hello"), "{source}");
            source
        })
        .collect::<Vec<_>>();
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
            panic!("bounded bold text integration requires pinned Typst");
        }
        eprintln!("[integration] Typst unavailable; skipping PDF check");
        return;
    };
    for source in typst_sources {
        let output = SubprocessBackend::new(executable.clone())
            .compile(&TypstInput {
                source,
                entry_path: "text.qd".into(),
            })
            .expect("bounded bold text must compile");
        assert!(output.pdf.expect("PDF output").starts_with(b"%PDF-"));
    }
}
