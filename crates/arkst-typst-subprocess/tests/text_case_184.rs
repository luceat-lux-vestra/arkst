use std::path::PathBuf;
use std::process::Command;

use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};
use arkst_typst::lowering::lower_to_typst;
use arkst_typst::{TypstBackend, TypstInput};
use arkst_typst_subprocess::SubprocessBackend;

#[test]
fn complete_text_case_domain_compiles_to_real_pdf() {
    let inputs = [
        ("none", "Hello *world* #emph[again]", None),
        (
            "uppercase",
            "#upper[Hello *world* #emph[again]]",
            Some("#upper["),
        ),
        (
            "lowercase",
            "#lower[Hello *world* #emph[again]]",
            Some("#lower["),
        ),
        (
            "capitalize",
            "#{ show regex(\"\\\\b\\\\p{L}\"): upper; [Hello *world* #emph[again]] }",
            Some("show regex("),
        ),
    ];
    let mut sources = Vec::new();
    for (text_case, expected, wrapper) in inputs {
        let input = format!(".text case:{{{text_case}}}\n    Hello **world** *again*\n");
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
        if let Some(wrapper) = wrapper {
            assert!(typst.contains(wrapper), "{typst}");
        } else {
            assert!(!typst.contains("#upper["), "{typst}");
            assert!(!typst.contains("#lower["), "{typst}");
            assert!(!typst.contains("show regex("), "{typst}");
        }
        assert!(source_map.iter().any(|entry| {
            input
                .get(entry.original.start..entry.original.end)
                .is_some_and(|original| original.starts_with(".text") || original.contains("Hello"))
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
            panic!("text case integration requires pinned Typst");
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
            .expect("text case must compile");
        assert!(output.pdf.expect("PDF output").starts_with(b"%PDF-"));
    }
}
