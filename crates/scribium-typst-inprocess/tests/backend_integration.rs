use scribium_core::{compile, CompileOptions, VirtualProjectBuilder};
use scribium_typst::{TypstBackend, TypstInput};
use scribium_typst_inprocess::{InProcessBackend, InProcessError};
use scribium_typst_subprocess::SubprocessBackend;
use std::path::PathBuf;
use std::process::Command;

fn compile_project_typst(project: &scribium_project::VirtualProject) -> String {
    let result = compile(project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "Scribium diagnostics: {:?}",
        result.diagnostics
    );
    scribium_typst::lowering::lower_to_typst_code(&result.ir)
}

fn project(source: &str) -> scribium_project::VirtualProject {
    VirtualProjectBuilder::new()
        .entry("docs/main.qd")
        .expect("entry")
        .add_source("docs/main.qd", source)
        .expect("source")
        .build()
        .expect("project")
}

fn find_typst() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("SCRIBIUM_TYPST_PATH") {
        let path = PathBuf::from(path);
        return path.is_file().then_some(path);
    }
    if Command::new("typst")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
    {
        return Some(PathBuf::from("typst"));
    }
    let homebrew = PathBuf::from("/opt/homebrew/bin/typst");
    homebrew.is_file().then_some(homebrew)
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

#[test]
fn actual_scribium_ir_compiles_to_pdf_in_process() {
    let project = project("# In-process\n\nA real Scribium document.\n");
    let source = compile_project_typst(&project);
    let backend = InProcessBackend::new(&project);
    let output = backend
        .compile(&TypstInput {
            source,
            entry_path: "./docs/../docs/main.qd".to_string(),
        })
        .expect("in-process compilation");

    let pdf = output.pdf.expect("PDF output");
    assert!(pdf.starts_with(b"%PDF-"));
    assert!(output.duration.as_nanos() > 0);
}

#[test]
fn generated_typst_has_success_and_page_parity_with_subprocess() {
    let Some(typst_path) = find_typst() else {
        let message = "parity test requires a Typst executable";
        if std::env::var_os("SCRIBIUM_REQUIRE_TYPST").is_some() {
            panic!("{message}");
        }
        eprintln!("[integration] {message}; skipping");
        return;
    };

    let source = format!(
        "# Parity\n\n{}",
        "A repeated paragraph exercises multi-page layout parity.\n\n".repeat(100)
    );
    let project = project(&source);
    let result = compile(&project, &CompileOptions::default());
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let input = TypstInput {
        source: scribium_typst::lowering::lower_to_typst_code(&result.ir),
        entry_path: "docs/main.qd".to_string(),
    };

    let in_process = InProcessBackend::new(&project)
        .compile(&input)
        .expect("in-process backend should compile");
    let subprocess = SubprocessBackend::new(typst_path)
        .compile(&input)
        .expect("subprocess backend should compile");
    let in_process_pdf = in_process.pdf.expect("in-process PDF");
    let subprocess_pdf = subprocess.pdf.expect("subprocess PDF");

    assert!(in_process_pdf.starts_with(b"%PDF-"));
    assert!(subprocess_pdf.starts_with(b"%PDF-"));
    let in_process_pages = page_count(&in_process_pdf);
    let subprocess_pages = page_count(&subprocess_pdf);
    assert!(in_process_pages >= 2, "expected multi-page output");
    assert_eq!(in_process_pages, subprocess_pages);
}

#[test]
fn existing_quarkdown_example_has_success_parity_with_subprocess() {
    let Some(typst_path) = find_typst() else {
        let message = "Quarkdown fixture parity test requires a Typst executable";
        if std::env::var_os("SCRIBIUM_REQUIRE_TYPST").is_some() {
            panic!("{message}");
        }
        eprintln!("[integration] {message}; skipping");
        return;
    };

    let fixture_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/hello/main.qd");
    let fixture = std::fs::read_to_string(&fixture_path).expect("repository Quarkdown fixture");
    let project = project(&fixture);
    let result = compile(&project, &CompileOptions::default());
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let input = TypstInput {
        source: scribium_typst::lowering::lower_to_typst_code(&result.ir),
        entry_path: "docs/main.qd".to_string(),
    };

    let in_process = InProcessBackend::new(&project)
        .compile(&input)
        .expect("in-process backend should compile the fixture");
    let subprocess = SubprocessBackend::new(typst_path)
        .compile(&input)
        .expect("subprocess backend should compile the fixture");

    assert!(in_process
        .pdf
        .as_deref()
        .is_some_and(|pdf| pdf.starts_with(b"%PDF-")));
    assert!(subprocess
        .pdf
        .as_deref()
        .is_some_and(|pdf| pdf.starts_with(b"%PDF-")));
}

#[test]
fn generated_typst_failure_has_equivalent_classification_and_better_structure() {
    let Some(typst_path) = find_typst() else {
        let message = "diagnostic parity test requires a Typst executable";
        if std::env::var_os("SCRIBIUM_REQUIRE_TYPST").is_some() {
            panic!("{message}");
        }
        eprintln!("[integration] {message}; skipping");
        return;
    };

    let project = project("valid Scribium input");
    let input = TypstInput {
        source: "#unknown-function()\n".to_string(),
        entry_path: "docs/main.qd".to_string(),
    };
    let in_process = InProcessBackend::new(&project)
        .compile(&input)
        .expect_err("in-process failure expected");
    let subprocess = SubprocessBackend::new(typst_path)
        .compile(&input)
        .expect_err("subprocess failure expected");

    assert!(!in_process.diagnostics().is_empty());
    assert_eq!(in_process.diagnostics()[0].code, "E5001");
    assert!(in_process.to_string().contains("/docs/main.typ"));
    assert!(subprocess.to_string().contains("Typst compilation failed"));
    assert!(!subprocess.to_string().contains("/tmp/"));
}

#[test]
fn asset_store_images_are_loaded_without_host_filesystem_access() {
    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><rect width="20" height="20" fill="red"/></svg>"#;
    let project = VirtualProjectBuilder::new()
        .entry("docs/main.qd")
        .expect("entry")
        .add_source("docs/main.qd", "![logo](./assets/logo.svg)")
        .expect("source")
        .add_asset("docs/assets/logo.svg", svg.to_vec())
        .expect("asset")
        .build()
        .expect("project");
    let output = InProcessBackend::new(&project)
        .compile(&TypstInput {
            source: "#image(\"./assets/logo.svg\")\n".to_string(),
            entry_path: "docs/main.qd".to_string(),
        })
        .expect("AssetStore image must compile");
    assert!(output.pdf.expect("PDF output").starts_with(b"%PDF-"));
}

#[test]
fn project_font_asset_is_accepted_by_the_font_policy() {
    let font = typst_assets::fonts().next().expect("embedded font fixture");
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("entry")
        .add_source("main.qd", "Hello with a project font.")
        .expect("source")
        .add_asset("fonts/project.otf", font.to_vec())
        .expect("font asset")
        .build()
        .expect("project");

    let output = InProcessBackend::new(&project)
        .compile(&TypstInput {
            source: "#set text(font: \"Libertinus Serif\")\nHello\n".to_string(),
            entry_path: "main.qd".to_string(),
        })
        .expect("font-backed compilation");
    assert!(output.pdf.expect("PDF output").starts_with(b"%PDF-"));
}

#[test]
fn missing_resources_fail_closed_with_virtual_paths() {
    let project = project("valid Scribium input");
    let error = InProcessBackend::new(&project)
        .compile(&TypstInput {
            source: "#image(\"./assets/missing.svg\")\n".to_string(),
            entry_path: "docs/main.qd".to_string(),
        })
        .expect_err("missing resource must fail");
    let rendered = error.to_string();
    assert!(rendered.contains("compilation failed") || rendered.contains("PDF export failed"));
    assert!(rendered.contains("missing.svg"), "{rendered}");
    assert!(
        !rendered.contains("/tmp/") && !rendered.contains("target/"),
        "{rendered}"
    );
    assert!(!error.diagnostics().is_empty());
}

#[test]
fn package_resolution_is_explicitly_denied_without_network_access() {
    let project = project("valid Scribium input");
    let error = InProcessBackend::new(&project)
        .compile(&TypstInput {
            source: "#import \"@preview/not-present:1.0.0\": *\n".to_string(),
            entry_path: "docs/main.qd".to_string(),
        })
        .expect_err("package access must be denied");
    let rendered = error.to_string();
    assert!(rendered.contains("compilation failed"));
    assert!(!rendered.contains("http://") && !rendered.contains("https://"));
    assert!(!rendered.contains("/tmp/") && !rendered.contains("target/"));
}

#[test]
fn generated_typst_diagnostics_preserve_path_without_fabricating_source_span() {
    let project = project("valid Scribium input");
    let error = InProcessBackend::new(&project)
        .compile(&TypstInput {
            source: "#unknown-function()\n".to_string(),
            entry_path: "docs/main.qd".to_string(),
        })
        .expect_err("invalid generated Typst must fail");
    let diagnostic = error.diagnostics().first().expect("structured diagnostic");
    assert_eq!(diagnostic.code, "E5001");
    assert!(diagnostic.message.contains("/docs/main.typ"));
    assert!(diagnostic.primary.is_none(), "no source map was supplied");
    assert!(!diagnostic.message.contains("/tmp/"));
}

#[test]
fn traversal_is_rejected_by_typst_virtual_paths() {
    let project = project("valid Scribium input");
    let error = InProcessBackend::new(&project)
        .compile(&TypstInput {
            source: "#image(\"../../outside.svg\")\n".to_string(),
            entry_path: "docs/main.qd".to_string(),
        })
        .expect_err("project escape must fail");
    let rendered = error.to_string();
    assert!(rendered.contains("outside.svg") || rendered.contains("project"));
}

#[test]
fn entry_mismatch_is_rejected_before_typst_execution() {
    let project = project("valid Scribium input");
    let error = InProcessBackend::new(&project)
        .compile(&TypstInput {
            source: "Hello\n".to_string(),
            entry_path: "other.qd".to_string(),
        })
        .expect_err("mismatched entry must fail");
    assert!(matches!(error, InProcessError::InvalidInput(_)));
}
