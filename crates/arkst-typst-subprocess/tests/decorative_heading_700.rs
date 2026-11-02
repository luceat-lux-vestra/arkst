use std::path::PathBuf;
use std::process::Command;

use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};
use arkst_typst::lowering::lower_to_typst;
use arkst_typst::{TypstBackend, TypstInput};
use arkst_typst_subprocess::SubprocessBackend;

#[test]
fn decorative_heading_lowers_to_valid_typst_and_pdf() {
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
            panic!("decorative heading integration requires pinned Typst");
        }
        eprintln!("[integration] Typst unavailable; skipping PDF check");
        return;
    };

    let input = "Before witness\n##! Decorative *한글*\nAfter witness\n";
    let project = VirtualProjectBuilder::new()
        .entry("decorative.qd")
        .expect("entry")
        .add_source("decorative.qd", input)
        .expect("source")
        .build()
        .expect("project");
    let result = compile(&project, &CompileOptions::default());
    assert!(result.diagnostics.is_empty(), "{result:?}");

    let (source, source_map) = lower_to_typst(&result.ir);
    assert!(
        source.contains(
            "#heading(depth: 2, numbering: none, outlined: false)[Decorative #emph[한글]]"
        ),
        "{source}"
    );
    assert!(source_map.iter().any(|entry| {
        input
            .get(entry.original.start..entry.original.end)
            .is_some_and(|original| original.starts_with("##! Decorative"))
            && entry.generated_start < entry.generated_end
            && entry.generated_end <= source.len()
    }));

    let output = SubprocessBackend::new(path)
        .compile(&TypstInput {
            source,
            entry_path: "decorative.qd".into(),
        })
        .expect("decorative heading must compile with official Typst");
    assert!(output.pdf.expect("PDF output").starts_with(b"%PDF-"));
}
