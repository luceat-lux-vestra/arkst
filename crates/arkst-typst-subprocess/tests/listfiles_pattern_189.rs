use std::path::PathBuf;
use std::process::Command;

use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};
use arkst_typst::lowering::lower_to_typst_code;
use arkst_typst::{TypstBackend, TypstInput};
use arkst_typst_subprocess::SubprocessBackend;

#[test]
fn listfiles_regex_result_compiles_into_real_typst_pdf() {
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("entry")
        .add_source(
            "main.qd",
            ".listfiles {data} directories:{false} fullpath:{false} pattern:{alpha[0-9]+[.]txt} sortby:{name}::getat {1}\n",
        )
        .expect("source")
        .add_asset("data/alpha1.txt", b"x".to_vec())
        .expect("asset")
        .add_asset("data/beta.txt", b"x".to_vec())
        .expect("asset")
        .build()
        .expect("project");
    let result = compile(&project, &CompileOptions::default());
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let source = lower_to_typst_code(&result.ir);
    assert!(source.contains("alpha1.txt"), "{source}");
    assert!(!source.contains("beta.txt"), "{source}");

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
            panic!("listfiles pattern integration requires pinned Typst");
        }
        eprintln!("[integration] Typst unavailable; skipping PDF check");
        return;
    };
    let output = SubprocessBackend::new(path)
        .compile(&TypstInput {
            source,
            entry_path: "main.qd".into(),
        })
        .expect("filtered listing must compile");
    assert!(output.pdf.expect("PDF output").starts_with(b"%PDF-"));
}

#[test]
fn unicode_literal_listing_compiles_to_real_typst_pdf() {
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("entry")
        .add_source(
            "main.qd",
            ".listfiles {data} directories:{false} fullpath:{false} pattern:{café}::getat {1}\n",
        )
        .expect("source")
        .add_asset("data/café", b"match".to_vec())
        .expect("asset")
        .add_asset("data/café2", b"skip".to_vec())
        .expect("asset")
        .build()
        .expect("project");
    let result = compile(&project, &CompileOptions::default());
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let source = lower_to_typst_code(&result.ir);
    assert!(source.contains("café"), "{source}");
    assert!(!source.contains("café2"), "{source}");

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
            panic!("Unicode literal listing integration requires pinned Typst");
        }
        eprintln!("[integration] Typst unavailable; skipping PDF check");
        return;
    };
    let output = SubprocessBackend::new(path)
        .compile(&TypstInput {
            source,
            entry_path: "main.qd".into(),
        })
        .expect("Unicode literal listing must compile");
    assert!(output.pdf.expect("PDF output").starts_with(b"%PDF-"));
}
