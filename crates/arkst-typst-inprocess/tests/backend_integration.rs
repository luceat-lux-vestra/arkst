use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};
use arkst_source::{SourceMapEntry, SourceSpan};
use arkst_typst::{TypstBackend, TypstInput};
use arkst_typst_inprocess::{InProcessBackend, InProcessError};
use arkst_typst_subprocess::SubprocessBackend;
use std::path::PathBuf;
use std::process::Command;

fn compile_project_typst(project: &arkst_project::VirtualProject) -> String {
    let result = compile(project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "Arkst diagnostics: {:?}",
        result.diagnostics
    );
    arkst_typst::lowering::lower_to_typst_code(&result.ir)
}

fn project(source: &str) -> arkst_project::VirtualProject {
    VirtualProjectBuilder::new()
        .entry("docs/main.qd")
        .expect("entry")
        .add_source("docs/main.qd", source)
        .expect("source")
        .build()
        .expect("project")
}

fn find_typst() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("ARKST_TYPST_PATH") {
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
fn actual_arkst_ir_compiles_to_pdf_in_process() {
    let project = project("# In-process\n\nA real Arkst document.\n");
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
        if std::env::var_os("ARKST_REQUIRE_TYPST").is_some() {
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
        source: arkst_typst::lowering::lower_to_typst_code(&result.ir),
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
        if std::env::var_os("ARKST_REQUIRE_TYPST").is_some() {
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
        source: arkst_typst::lowering::lower_to_typst_code(&result.ir),
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
        if std::env::var_os("ARKST_REQUIRE_TYPST").is_some() {
            panic!("{message}");
        }
        eprintln!("[integration] {message}; skipping");
        return;
    };

    let project = project("valid Arkst input");
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
fn datetime_today_is_fail_closed_for_default_and_explicit_offsets() {
    let project = project("valid Arkst input");
    for source in [
        "#datetime.today()\n",
        "#datetime.today(offset: 9)\n",
        "#datetime.today(offset: -8)\n",
        "#datetime.today(offset: duration(minutes: 30))\n",
    ] {
        let error = InProcessBackend::new(&project)
            .compile(&TypstInput {
                source: source.to_string(),
                entry_path: "docs/main.qd".to_string(),
            })
            .expect_err("unavailable date capability must fail closed");
        assert!(error.to_string().contains("current date"), "{error}");
        assert!(!error.to_string().contains("1970"), "{error}");
    }
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
    let project = project("valid Arkst input");
    let error = InProcessBackend::new(&project)
        .compile(&TypstInput {
            source: "#image(\"./assets/missing.svg\")\n".to_string(),
            entry_path: "docs/main.qd".to_string(),
        })
        .expect_err("missing resource must fail");
    let rendered = error.to_string();
    assert!(rendered.contains("compilation failed") || rendered.contains("PDF export failed"));
    assert!(rendered.contains("missing.svg"), "{rendered}");
    assert!(!rendered.contains("/tmp/"), "{rendered}");
    assert!(!error.diagnostics().is_empty());
}

#[test]
fn package_resolution_is_explicitly_denied_without_network_access() {
    let project = project("valid Arkst input");
    let error = InProcessBackend::new(&project)
        .compile(&TypstInput {
            source: "#import \"@preview/not-present:1.0.0\": *\n".to_string(),
            entry_path: "docs/main.qd".to_string(),
        })
        .expect_err("package access must be denied");
    let rendered = error.to_string();
    assert!(rendered.contains("compilation failed"));
    assert!(!rendered.contains("http://") && !rendered.contains("https://"));
    assert!(!rendered.contains("/tmp/"));
}

#[test]
fn runtime_generated_package_is_denied_by_the_world_capability_boundary() {
    let project = project("valid Arkst input");
    let error = InProcessBackend::new(&project)
        .compile(&TypstInput {
            source: "#let package = \"@preview/\" + \"not-present:1.0.0\"\n#eval(\"import \\\"\" + package + \"\\\": *\", mode: \"code\")\n"
                .to_string(),
            entry_path: "docs/main.qd".to_string(),
        })
        .expect_err("runtime-generated package access must fail at the World boundary");
    let rendered = error.to_string();
    assert!(
        rendered.contains("package") || rendered.contains("@preview"),
        "{rendered}"
    );
    assert!(!rendered.contains("http://") && !rendered.contains("https://"));
    assert!(!rendered.contains("/tmp/"));
}

#[test]
fn generated_typst_diagnostics_preserve_path_without_fabricating_source_span() {
    let project = project("valid Arkst input");
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
    let project = project("valid Arkst input");
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
    let project = project("valid Arkst input");
    let error = InProcessBackend::new(&project)
        .compile(&TypstInput {
            source: "Hello\n".to_string(),
            entry_path: "other.qd".to_string(),
        })
        .expect_err("mismatched entry must fail");
    assert!(matches!(error, InProcessError::InvalidInput(_)));
}

#[test]
fn invalid_entry_path_is_rejected_before_typst_execution() {
    let project = project("valid Arkst input");
    let error = InProcessBackend::new(&project)
        .compile(&TypstInput {
            source: "Hello\n".to_string(),
            entry_path: "../docs/main.qd".to_string(),
        })
        .expect_err("root-escaping entry must fail");
    assert!(matches!(error, InProcessError::InvalidInput(_)));
    assert!(error.to_string().contains("entry path"));
}

#[test]
fn repeated_resource_loads_use_the_same_virtual_project_boundary() {
    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><rect width="20" height="20" fill="red"/></svg>"#;
    let project = VirtualProjectBuilder::new()
        .entry("docs/main.qd")
        .expect("entry")
        .add_source("docs/main.qd", "repeated resources")
        .expect("source")
        .add_asset("docs/assets/logo.svg", svg.to_vec())
        .expect("asset")
        .build()
        .expect("project");
    let output = InProcessBackend::new(&project)
        .compile(&TypstInput {
            source: "#image(\"./assets/logo.svg\")\n#image(\"./assets/logo.svg\")\n".to_string(),
            entry_path: "docs/main.qd".to_string(),
        })
        .expect("repeated AssetStore loads must compile");
    assert!(output.pdf.expect("PDF output").starts_with(b"%PDF-"));
}

#[test]
fn source_map_handoff_preserves_a_reliable_original_span() {
    let project_source = "valid Arkst input";
    let project = project(project_source);
    let generated = "#unknown-function()\n";
    let source_id = project
        .sources()
        .get_id(project.entry())
        .expect("entry source id");
    let original = SourceSpan::new(source_id, 0, project_source.len());
    let source_map = [SourceMapEntry {
        generated_start: 0,
        generated_end: generated.len(),
        original,
    }];

    let error = InProcessBackend::new(&project)
        .compile_with_source_map(
            &TypstInput {
                source: generated.to_string(),
                entry_path: "docs/main.qd".to_string(),
            },
            &source_map,
        )
        .expect_err("invalid generated Typst must fail");
    let diagnostic = error.diagnostics().first().expect("diagnostic");
    assert_eq!(diagnostic.primary, Some(original));
    assert!(diagnostic.message.contains("/docs/main.qd"));
    assert!(!diagnostic.message.contains("/docs/main.typ"));
    assert!(!diagnostic.message.contains("/tmp/"));
}

#[test]
fn incomplete_source_map_does_not_fabricate_a_span() {
    let project_source = "valid Arkst input";
    let project = project(project_source);
    let generated = "#unknown-function()\n";
    let source_id = project
        .sources()
        .get_id(project.entry())
        .expect("entry source id");
    let source_map = [SourceMapEntry {
        generated_start: generated.len(),
        generated_end: generated.len() + 1,
        original: SourceSpan::new(source_id, 0, project_source.len()),
    }];

    let error = InProcessBackend::new(&project)
        .compile_with_source_map(
            &TypstInput {
                source: generated.to_string(),
                entry_path: "docs/main.qd".to_string(),
            },
            &source_map,
        )
        .expect_err("invalid generated Typst must fail");
    assert!(error
        .diagnostics()
        .first()
        .expect("diagnostic")
        .primary
        .is_none());
}

#[test]
fn ambiguous_source_map_does_not_fabricate_a_span() {
    let project_source = "valid Arkst input";
    let project = VirtualProjectBuilder::new()
        .entry("docs/main.qd")
        .expect("entry")
        .add_source("docs/main.qd", project_source)
        .expect("source")
        .add_source("docs/other.qd", "another valid source")
        .expect("other source")
        .build()
        .expect("project");
    let generated = "#unknown-function()\n";
    let main_id = project
        .sources()
        .get_id(&arkst_project::VirtualPathBuf::parse("docs/main.qd").expect("path"))
        .expect("main source id");
    let other_id = project
        .sources()
        .get_id(&arkst_project::VirtualPathBuf::parse("docs/other.qd").expect("path"))
        .expect("other source id");
    let source_map = [
        SourceMapEntry {
            generated_start: 0,
            generated_end: generated.len(),
            original: SourceSpan::new(main_id, 0, project_source.len()),
        },
        SourceMapEntry {
            generated_start: 0,
            generated_end: generated.len(),
            original: SourceSpan::new(other_id, 0, "another valid source".len()),
        },
    ];

    let error = InProcessBackend::new(&project)
        .compile_with_source_map(
            &TypstInput {
                source: generated.to_string(),
                entry_path: "docs/main.qd".to_string(),
            },
            &source_map,
        )
        .expect_err("invalid generated Typst must fail");
    assert!(error
        .diagnostics()
        .first()
        .expect("diagnostic")
        .primary
        .is_none());
}
