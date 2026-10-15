use std::path::PathBuf;
use std::process::Command;

use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};
use arkst_typst::lowering::lower_to_typst;
use arkst_typst::{TypstBackend, TypstInput};
use arkst_typst_subprocess::SubprocessBackend;

#[test]
fn complete_text_size_domain_compiles_to_real_pdf() {
    let inputs = [
        ("tiny", "#text(size: 0.5em)[Hello world]", true),
        ("small", "#text(size: 0.75em)[Hello world]", true),
        ("normal", "Hello world", false),
        ("medium", "#text(size: 1.25em)[Hello world]", true),
        ("large", "#text(size: 1.5em)[Hello world]", true),
        ("larger", "#text(size: 2em)[Hello world]", true),
        ("huge", "#text(size: 3em)[Hello world]", true),
    ];
    let mut sources = Vec::new();
    for (size, expected, has_size_wrapper) in inputs {
        let input = format!(".text text:{{Hello world}} size:{{{size}}}\n");
        let project = VirtualProjectBuilder::new()
            .entry("text.qd")
            .expect("entry")
            .add_source("text.qd", input.clone())
            .expect("source")
            .build()
            .expect("project");
        let result = compile(&project, &CompileOptions::default());
        assert!(result.diagnostics.is_empty(), "{result:?}");
        let (typst, source_map) = lower_to_typst(&result.ir);
        assert!(typst.contains(expected), "{typst}");
        assert_eq!(typst.contains("#text(size:"), has_size_wrapper, "{typst}");
        assert!(source_map.iter().any(|entry| {
            input
                .get(entry.original.start..entry.original.end)
                .is_some_and(|original| {
                    original.starts_with(".text") || original.contains("Hello world")
                })
                && entry.generated_start < entry.generated_end
                && entry.generated_end <= typst.len()
        }));
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
            panic!("text size integration requires pinned Typst");
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
            .expect("text size must compile");
        assert!(output.pdf.expect("PDF output").starts_with(b"%PDF-"));
    }
}
