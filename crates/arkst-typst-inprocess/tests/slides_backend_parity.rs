use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};
use arkst_typst::{TypstBackend, TypstInput};
use arkst_typst_inprocess::InProcessBackend;
use arkst_typst_subprocess::SubprocessBackend;
use std::path::PathBuf;
use std::process::Command;

fn project(source: &str) -> arkst_project::VirtualProject {
    VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("entry")
        .add_source("main.qd", source)
        .expect("source")
        .build()
        .expect("project")
}

fn page_count(pdf: &[u8]) -> usize {
    let pages = pdf
        .windows(b"/Type/Page".len())
        .filter(|window| *window == b"/Type/Page")
        .count();
    let page_tree_nodes = pdf
        .windows(b"/Type/Pages".len())
        .filter(|window| *window == b"/Type/Pages")
        .count();
    pages.saturating_sub(page_tree_nodes)
}

fn first_media_box(pdf: &[u8]) -> (f64, f64) {
    let text = String::from_utf8_lossy(pdf);
    let start = text.find("/MediaBox").expect("PDF MediaBox") + "/MediaBox".len();
    let rest = &text[start..];
    let open = rest.find('[').expect("MediaBox opening bracket");
    let close = rest[open + 1..]
        .find(']')
        .map(|offset| open + 1 + offset)
        .expect("MediaBox closing bracket");
    let values = rest[open + 1..close]
        .split_whitespace()
        .map(|value| value.parse::<f64>().expect("numeric MediaBox component"))
        .collect::<Vec<_>>();
    assert_eq!(values.len(), 4, "unexpected MediaBox: {values:?}");
    (values[2] - values[0], values[3] - values[1])
}

fn typst_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("ARKST_TYPST_PATH") {
        return Some(PathBuf::from(path));
    }
    let probe = Command::new("typst").arg("--version").output();
    match probe {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout);
            assert!(
                version.contains("0.15.1"),
                "slides parity is pinned to Typst 0.15.1, got {version:?}"
            );
            Some(PathBuf::from("typst"))
        }
        _ if std::env::var_os("ARKST_REQUIRE_TYPST").is_some() => {
            panic!("slides parity requires pinned Typst 0.15.1")
        }
        _ => None,
    }
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 0.05,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn slides_pdf_contract_matches_subprocess_and_inprocess_backends() {
    let Some(typst_path) = typst_path() else {
        return;
    };

    for source in [
        ".doctype {slides}\n\nFIRST\n\n<<<\n\nSECOND\n",
        ".doctype {slides}\n.slides center:{true}\n\nCENTERED\n",
        ".doctype {slides}\n.theme layout:{focus}\n\n# FOCUS\n",
        ".doctype {slides}\n.pageformat width:{10in} height:{5in}\n\nEXPLICIT\n",
    ] {
        let project = project(source);
        let result = compile(&project, &CompileOptions::default());
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        let typst_source = arkst_typst::lowering::lower_to_typst_code(&result.ir);
        let input = TypstInput {
            source: typst_source,
            entry_path: "main.qd".to_string(),
        };

        let in_process = InProcessBackend::new(&project)
            .compile(&input)
            .expect("in-process slides PDF")
            .pdf
            .expect("in-process PDF");
        let subprocess = SubprocessBackend::new(&typst_path)
            .compile(&input)
            .expect("subprocess slides PDF")
            .pdf
            .expect("subprocess PDF");

        assert_eq!(
            page_count(&subprocess),
            page_count(&in_process),
            "backend page-count divergence for source:\n{source}"
        );
        let subprocess_box = first_media_box(&subprocess);
        let in_process_box = first_media_box(&in_process);
        assert_close(subprocess_box.0, in_process_box.0);
        assert_close(subprocess_box.1, in_process_box.1);
    }
}
