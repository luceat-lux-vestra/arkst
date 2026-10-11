use std::path::PathBuf;
use std::process::Command;

use arkst_core::ir::IrNode;
use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};
use arkst_typst::lowering::lower_to_typst_code;
use arkst_typst::{TypstBackend, TypstInput};
use arkst_typst_subprocess::SubprocessBackend;

#[test]
fn extended_angle_and_callable_pagebreaks_compile_with_typst() {
    let project = VirtualProjectBuilder::new()
        .entry("explicit-break.qd")
        .expect("entry")
        .add_source(
            "explicit-break.qd",
            "First\n\n<<<<\n\nSecond\n\n.pagebreak\n\nThird\n",
        )
        .expect("source")
        .build()
        .expect("project");
    let result = compile(&project, &CompileOptions::default());
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(
        result
            .ir
            .nodes
            .iter()
            .filter(|node| matches!(node, IrNode::PageBreak { .. }))
            .count(),
        2
    );
    let typst_code = lower_to_typst_code(&result.ir);
    assert_eq!(typst_code.matches("#pagebreak(weak: true)").count(), 2);

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
            panic!("extended pagebreak integration requires pinned Typst");
        }
        eprintln!("[integration] Typst unavailable; skipping PDF check");
        return;
    };
    let output = SubprocessBackend::new(path)
        .compile(&TypstInput {
            source: typst_code,
            entry_path: "explicit-break.qd".into(),
        })
        .expect("bounded pagebreak Typst output must compile");
    assert!(output.pdf.expect("PDF output").starts_with(b"%PDF-"));
}
