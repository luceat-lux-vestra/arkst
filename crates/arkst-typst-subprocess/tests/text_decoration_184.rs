use std::path::PathBuf;
use std::process::Command;

use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};
use arkst_typst::lowering::lower_to_typst;
use arkst_typst::{TypstBackend, TypstInput};
use arkst_typst_subprocess::SubprocessBackend;

#[test]
fn complete_text_decoration_domain_compiles_to_real_pdf() {
    let inputs = [
        (
            ".text text:{Hello world} decoration:{underline}\n",
            "#underline[Hello world]",
        ),
        (
            ".text text:{Hello world} decoration:{overline}\n",
            "#overline[Hello world]",
        ),
        (
            ".text text:{Hello world} decoration:{underoverline}\n",
            "#underline[#overline[Hello world]]",
        ),
        (
            ".text text:{Hello world} decoration:{all}\n",
            "#underline[#overline[#strike[Hello world]]]",
        ),
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
        let (typst, source_map) = lower_to_typst(&result.ir);
        assert!(typst.contains(expected), "{typst}");
        assert!(source_map.iter().any(|entry| {
            input
                .get(entry.original.start..entry.original.end)
                .is_some_and(|original| original.starts_with(".text"))
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
            .expect("text decoration must compile");
        assert!(output.pdf.expect("PDF output").starts_with(b"%PDF-"));
    }
}
