use std::path::PathBuf;
use std::process::Command;

use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};
use arkst_typst::lowering::lower_to_typst_code;
use arkst_typst::{TypstBackend, TypstInput};
use arkst_typst_subprocess::SubprocessBackend;

fn find_typst() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("ARKST_TYPST_PATH") {
        let path = PathBuf::from(path);
        return path.is_file().then_some(path);
    }
    let probe = Command::new("typst").arg("--version").output();
    if probe.is_ok_and(|output| output.status.success()) {
        return Some(PathBuf::from("typst"));
    }
    let homebrew = PathBuf::from("/opt/homebrew/bin/typst");
    homebrew.is_file().then_some(homebrew)
}

fn with_typst<F>(name: &str, body: F)
where
    F: FnOnce(SubprocessBackend),
{
    match find_typst() {
        Some(path) => body(SubprocessBackend::new(path)),
        None if std::env::var("ARKST_REQUIRE_TYPST").is_ok() => {
            panic!("{name}: ARKST_REQUIRE_TYPST is set but no Typst executable was found")
        }
        None => eprintln!("[integration] {name}: no Typst executable found; skipping"),
    }
}

fn lower(source: &str) -> String {
    let project = VirtualProjectBuilder::new()
        .entry("last-heading-output.qd")
        .expect("valid entry")
        .add_source("last-heading-output.qd", source)
        .expect("valid source")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    lower_to_typst_code(&result.ir)
}

#[test]
fn paged_lastheading_history_query_compiles_with_pinned_typst() {
    let typst = lower(
        ".doctype {paged}\n.pagemargin {topcenter}\n    .lastheading depth:{2}\n\n## First\nBody\n.pagebreak\n# Shallower\nBody\n",
    );

    assert!(typst.contains("query(heading)"), "{typst}");
    assert!(typst.contains("it.level == 2"), "{typst}");
    assert!(typst.contains("it.level < 2"), "{typst}");
    assert!(typst.contains("it.location().page() > __arkst_candidate_page"), "{typst}");
    assert!(typst.contains("__arkst_candidate.body"), "{typst}");

    with_typst("paged-lastheading-history", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst,
                entry_path: "last-heading-output.qd".to_string(),
            })
            .expect("paged lastheading history query must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}
