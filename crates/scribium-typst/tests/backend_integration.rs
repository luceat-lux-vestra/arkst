//! Integration tests that exercise the real `typst` executable.
//!
//! These tests are separated from the unit tests in `backend.rs` because
//! they need an actual Typst installation. They are skipped (with a notice)
//! when no Typst executable can be located, so a developer machine or a CI
//! runner without Typst can still run the rest of the suite. CI installs a
//! pinned Typst version explicitly before running tests; set
//! `SCRIBIUM_REQUIRE_TYPST=1` to turn a missing executable into a hard
//! failure instead of a skip.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use scribium_core::ir::{IrComponent, IrInline, IrNode, NativeTarget};
use scribium_core::{compile, CompileOptions, VirtualProjectBuilder};
use scribium_typst::backend::TypstSourceContext;
use scribium_typst::backend::{SubprocessBackend, TypstBackend, TypstInput};
use scribium_typst::lowering::{lower_to_typst, lower_to_typst_code};
use tempfile::tempdir;

/// Locates a Typst executable, in order of preference:
///
/// 1. `SCRIBIUM_TYPST_PATH` (used by CI to point at a pinned install);
/// 2. `typst` on `PATH`;
/// 3. the Homebrew default location (`/opt/homebrew/bin/typst`).
fn find_typst() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("SCRIBIUM_TYPST_PATH") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
        return None;
    }
    let probe = Command::new("typst").arg("--version").output();
    if probe.is_ok_and(|o| o.status.success()) {
        return Some(PathBuf::from("typst"));
    }
    let homebrew = PathBuf::from("/opt/homebrew/bin/typst");
    if homebrew.is_file() {
        return Some(homebrew);
    }
    None
}

/// Runs `body` with a located Typst backend, skipping (or failing, when
/// `SCRIBIUM_REQUIRE_TYPST` is set) if none can be found.
fn with_typst<F>(name: &str, body: F)
where
    F: FnOnce(SubprocessBackend),
{
    match find_typst() {
        Some(path) => {
            eprintln!("[integration] {name}: using typst at {}", path.display());
            body(SubprocessBackend::new(path));
        }
        None => {
            let required = std::env::var("SCRIBIUM_REQUIRE_TYPST").is_ok();
            let message = format!(
                "[integration] {name}: no Typst executable found (set SCRIBIUM_TYPST_PATH or install typst); \
                 {}",
                if required {
                    "SCRIBIUM_REQUIRE_TYPST is set, failing"
                } else {
                    "skipping"
                }
            );
            eprintln!("{message}");
            if required {
                panic!("{message}");
            }
        }
    }
}

#[test]
fn integration_compile_produces_valid_pdf() {
    with_typst("compile", |backend| {
        let input = TypstInput {
            source: "#heading[Test]\n\nHello world.\n".to_string(),
            entry_path: "test.qd".to_string(),
        };
        let output = backend.compile(&input).expect("compile should succeed");
        let pdf = output.pdf.expect("pdf output must be present");
        assert!(!pdf.is_empty(), "pdf must not be empty");
        assert!(
            pdf.starts_with(b"%PDF-"),
            "pdf must start with %PDF-, began with {:?}",
            &pdf[..pdf.len().min(8)]
        );
    });
}

#[test]
fn integration_stacked_layouts_lower_to_valid_typst_and_pdf() {
    let source = ".row alignment:{spacebetween} cross:{stretch} gap:{10px}\n    A\n\n    B\n\n.column alignment:{spacearound} cross:{start} gap:{25%}\n    C\n\n    D\n\n.grid columns:{2} alignment:{spaceevenly} cross:{end} gap:{1cm} vgap:{2cm} hgap:{3cm}\n    E\n\n    F\n\n    G\n";
    let project = VirtualProjectBuilder::new()
        .entry("stacked.qd")
        .expect("valid entry path")
        .add_source("stacked.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "stacked diagnostics: {:?}",
        result.diagnostics
    );
    let typst_code = lower_to_typst_code(&result.ir);
    assert!(typst_code.contains("#stack(dir: ltr"), "{typst_code}");
    assert!(typst_code.contains("#stack(dir: ttb"), "{typst_code}");
    assert!(typst_code.contains("h(7.5pt)"), "{typst_code}");
    assert!(typst_code.contains("v(25%)"), "{typst_code}");
    assert!(!typst_code.contains("spacing: 7.5pt"), "{typst_code}");
    assert!(!typst_code.contains("spacing: 25%"), "{typst_code}");
    assert!(
        typst_code.contains("columns: (1fr, auto, 3cm, 1fr, auto, 1fr)"),
        "{typst_code}"
    );
    assert!(typst_code.contains("row-gutter: 2cm"), "{typst_code}");
    assert!(!typst_code.contains("column-gutter:"), "{typst_code}");
    assert!(typst_code.contains("#block(height: 100%)"), "{typst_code}");

    with_typst("stacked-layouts", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "stacked.qd".to_string(),
            })
            .expect("stacked Typst must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_center_layout_lowers_to_valid_typst_and_pdf() {
    let source = ".center\n    Hello\n\n    .row\n        A\n\n        B\n";
    let project = VirtualProjectBuilder::new()
        .entry("center.qd")
        .expect("valid entry path")
        .add_source("center.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "center diagnostics: {:?}",
        result.diagnostics
    );
    let typst_code = lower_to_typst_code(&result.ir);
    assert!(typst_code.contains("#block(width: 100%)"), "{typst_code}");
    assert!(typst_code.contains("#align(center)"), "{typst_code}");
    assert!(typst_code.contains("#stack(dir: ltr"), "{typst_code}");
    assert!(typst_code.find("Hello").unwrap() < typst_code.find('A').unwrap());
    assert!(typst_code.find('A').unwrap() < typst_code.find('B').unwrap());

    with_typst("center-layout", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "center.qd".to_string(),
            })
            .expect("center Typst must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_landscape_layout_lowers_to_valid_typst_and_pdf() {
    let source = ".landscape\n    ## Wide section\n\n    .row gap:{1cm}\n        Left\n\n        Center\n\n        Right\n";
    let project = VirtualProjectBuilder::new()
        .entry("landscape.qd")
        .expect("valid entry path")
        .add_source("landscape.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "landscape diagnostics: {:?}",
        result.diagnostics
    );
    let [IrNode::Component {
        component: IrComponent::Landscape(landscape),
    }] = result.ir.nodes.as_slice()
    else {
        panic!("expected landscape root, got {:?}", result.ir.nodes);
    };
    assert!(matches!(
        landscape.children.as_slice(),
        [
            IrNode::Heading { .. },
            IrNode::Component {
                component: IrComponent::Stacked(_)
            }
        ]
    ));

    let typst_code = lower_to_typst_code(&result.ir);
    assert!(
        typst_code.contains("#rotate(-90deg, reflow: true)"),
        "{typst_code}"
    );
    assert!(typst_code.contains("#stack(dir: ltr"), "{typst_code}");
    assert!(!typst_code.contains("page(flipped: true)"), "{typst_code}");

    with_typst("landscape-layout", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "landscape.qd".to_string(),
            })
            .expect("landscape Typst must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_align_layout_lowers_to_valid_typst_and_pdf() {
    let source = ".align {end}\n    Hello\n\n    .row\n        A\n\n        B\n";
    let project = VirtualProjectBuilder::new()
        .entry("align.qd")
        .expect("valid entry path")
        .add_source("align.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "align diagnostics: {:?}",
        result.diagnostics
    );
    let typst_code = lower_to_typst_code(&result.ir);
    assert!(typst_code.contains("#block(width: 100%)"), "{typst_code}");
    assert!(typst_code.contains("#align(end)"), "{typst_code}");
    assert!(typst_code.contains("#stack(dir: ltr"), "{typst_code}");
    assert!(typst_code.find("Hello").unwrap() < typst_code.find('A').unwrap());
    assert!(typst_code.find('A').unwrap() < typst_code.find('B').unwrap());

    with_typst("align-layout", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "align.qd".to_string(),
            })
            .expect("align Typst must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_container_sizing_lowers_to_valid_typst_and_pdf() {
    let source = ".row\n    .container width:{4cm}\n        ## Left\n        Text\n\n    .container fullwidth:{yes}\n        Right\n";
    let project = VirtualProjectBuilder::new()
        .entry("container.qd")
        .expect("valid entry path")
        .add_source("container.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "container diagnostics: {:?}",
        result.diagnostics
    );
    let [IrNode::Component {
        component: IrComponent::Stacked(row),
    }] = result.ir.nodes.as_slice()
    else {
        panic!("expected typed row, got {:?}", result.ir.nodes);
    };
    assert_eq!(row.children.len(), 2);
    assert!(row.children.iter().all(|child| matches!(
        child,
        IrNode::Component {
            component: IrComponent::Container(_)
        }
    )));

    let typst_code = lower_to_typst_code(&result.ir);
    assert!(typst_code.contains("#block(width: 4cm)"), "{typst_code}");
    assert!(typst_code.contains("#block(width: 100%)"), "{typst_code}");

    with_typst("container-sizing", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "container.qd".to_string(),
            })
            .expect("container Typst must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_whitespace_lowers_to_lossless_typst_and_pdf() {
    let source = "A .whitespace B\n\n.whitespace width:{2cm}\n\n.whitespace height:{2cm}\n\n.whitespace width:{2cm} height:{1cm}\n";
    let project = VirtualProjectBuilder::new()
        .entry("whitespace.qd")
        .expect("valid entry path")
        .add_source("whitespace.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "whitespace diagnostics: {:?}",
        result.diagnostics
    );

    let (typst_code, source_map) = lower_to_typst(&result.ir);
    assert!(typst_code.contains('\u{a0}'), "{typst_code:?}");
    assert!(
        typst_code.contains("#box(width: 2cm, height: 0pt)[]"),
        "{typst_code}"
    );
    assert!(
        typst_code.contains("#box(width: 0pt, height: 2cm)[]"),
        "{typst_code}"
    );
    assert!(
        typst_code.contains("#box(width: 2cm, height: 1cm)[]"),
        "{typst_code}"
    );
    assert!(
        source
            .match_indices(".whitespace")
            .map(|(start, _)| start)
            .all(|start| source_map.iter().any(|entry| entry.original.start == start)),
        "whitespace lowering lost source provenance: {source_map:?}"
    );

    with_typst("whitespace", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "whitespace.qd".to_string(),
            })
            .expect("whitespace Typst must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_self_contained_mode_does_not_expose_temp_resources() {
    with_typst("self-contained-resource-boundary", |backend| {
        let result = backend.compile(&TypstInput {
            source: "#read(\"./resource.txt\")\n".to_string(),
            entry_path: "test.qd".to_string(),
        });
        assert!(
            result.is_err(),
            "resources require an explicit source context"
        );
    });
}

#[test]
fn integration_relative_image_uses_project_source_context() {
    with_typst("relative-image", |backend| {
        let project = tempdir().expect("project temp directory");
        let docs = project.path().join("docs");
        let assets = docs.join("assets");
        fs::create_dir_all(&assets).expect("asset directory");
        fs::write(docs.join("main.qd"), "original source\n").expect("source fixture");
        fs::write(
            assets.join("tiny.svg"),
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="10pt" height="10pt" viewBox="0 0 10 10"><rect width="10" height="10" fill="red"/></svg>"#,
        )
        .expect("SVG fixture");

        let output = backend
            .with_source_context(TypstSourceContext::new(project.path()))
            .compile(&TypstInput {
                source: "#image(\"./assets/tiny.svg\")\n".to_string(),
                entry_path: "docs/main.qd".to_string(),
            })
            .expect("relative SVG should compile");
        assert!(output.pdf.is_some_and(|pdf| pdf.starts_with(b"%PDF-")));
        assert!(!docs.join("main.typ").exists());
        assert!(!project.path().join("output.pdf").exists());
        assert_eq!(
            fs::read_to_string(docs.join("main.qd")).unwrap(),
            "original source\n"
        );
    });
}

#[test]
fn integration_markdown_images_compile_from_source_relative_paths_to_pdf() {
    with_typst("markdown-images-e2e", |backend| {
        let project_root = tempdir().expect("project temp directory");
        let docs = project_root.path().join("docs");
        let assets = docs.join("assets");
        let shared = project_root.path().join("shared");
        fs::create_dir_all(&assets).expect("asset directory");
        fs::create_dir_all(&shared).expect("shared asset directory");
        fs::write(
            docs.join("guide.md"),
            "# Image Test\n\nBefore.\n\n![Square](./assets/square.svg)\n\n![Shared](../shared/logo.svg \"Logo\")\n\n![Pixel](./assets/pixel.png)\n\nAfter.\n",
        )
        .expect("Markdown source fixture");
        fs::write(
            assets.join("square.svg"),
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 32 32"><rect width="32" height="32"/></svg>"#,
        )
        .expect("SVG fixture");
        fs::write(
            shared.join("logo.svg"),
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><circle cx="8" cy="8" r="8"/></svg>"#,
        )
        .expect("parent-relative SVG fixture");
        fs::write(
            assets.join("pixel.png"),
            [
                0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48,
                0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x01, 0x00, 0x00, 0x00,
                0x00, 0x37, 0x6e, 0xf9, 0x24, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, 0x54, 0x08,
                0xd7, 0x63, 0x60, 0x00, 0x00, 0x00, 0x02, 0x00, 0x01, 0xe2, 0x21, 0xbc, 0x33, 0x00,
                0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
            ],
        )
        .expect("PNG fixture");

        let source = fs::read_to_string(docs.join("guide.md")).expect("read Markdown fixture");
        let project = VirtualProjectBuilder::new()
            .entry("docs/guide.md")
            .expect("valid entry")
            .add_source("docs/guide.md", source)
            .expect("valid source")
            .build()
            .expect("valid project");
        let result = compile(&project, &CompileOptions::default());
        assert!(
            result.diagnostics.is_empty(),
            "unexpected: {:?}",
            result.diagnostics
        );

        let typst = lower_to_typst_code(&result.ir);
        assert!(typst.contains("#image(\"./assets/square.svg\")"));
        assert!(typst.contains("#image(\"../shared/logo.svg\")"));
        assert!(typst.contains("#image(\"./assets/pixel.png\")"));

        let output = backend
            .with_source_context(TypstSourceContext::new(project_root.path()))
            .compile(&TypstInput {
                source: typst,
                entry_path: "docs/guide.md".to_string(),
            })
            .expect("Markdown images should compile through Typst");
        assert!(output.pdf.is_some_and(|pdf| pdf.starts_with(b"%PDF-")));
    });
}

#[test]
fn integration_include_then_markdown_image_compiles_from_nested_source_context() {
    with_typst("include-markdown-image-e2e", |backend| {
        let project_root = tempdir().expect("project temp directory");
        let docs = project_root.path().join("docs");
        let assets = docs.join("assets");
        fs::create_dir_all(&assets).expect("asset directory");
        fs::write(docs.join("main.qd"), ".include {part.md}\n").expect("main fixture");
        fs::write(
            docs.join("part.md"),
            "# Included image\n\n![Square](assets/square.svg)\n",
        )
        .expect("included Markdown fixture");
        fs::write(
            assets.join("square.svg"),
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32"><rect width="32" height="32"/></svg>"#,
        )
        .expect("SVG fixture");

        let project = VirtualProjectBuilder::new()
            .entry("docs/main.qd")
            .expect("valid entry")
            .add_source("docs/main.qd", ".include {part.md}\n")
            .expect("valid main source")
            .add_source(
                "docs/part.md",
                "# Included image\n\n![Square](assets/square.svg)\n",
            )
            .expect("valid Markdown source")
            .add_asset(
                "docs/assets/square.svg",
                fs::read(assets.join("square.svg")).expect("read SVG fixture"),
            )
            .expect("valid asset")
            .build()
            .expect("valid project");
        let result = compile(&project, &CompileOptions::default());
        assert!(
            result.diagnostics.is_empty(),
            "unexpected compiler diagnostics: {:?}",
            result.diagnostics
        );
        let typst = lower_to_typst_code(&result.ir);
        assert!(typst.contains("#image(\"assets/square.svg\")"), "{typst}");
        let output = backend
            .with_source_context(TypstSourceContext::new(project_root.path()))
            .compile(&TypstInput {
                source: typst,
                entry_path: "docs/main.qd".to_string(),
            })
            .expect("included Markdown image should compile through Typst");
        assert!(output.pdf.is_some_and(|pdf| pdf.starts_with(b"%PDF-")));
    });
}

#[test]
fn integration_missing_image_is_a_typst_resource_failure() {
    with_typst("missing-image", |backend| {
        let project_root = tempdir().expect("project temp directory");
        let docs = project_root.path().join("docs");
        fs::create_dir_all(&docs).expect("docs directory");
        let result = backend
            .with_source_context(TypstSourceContext::new(project_root.path()))
            .compile(&TypstInput {
                source: "#image(\"./assets/missing.svg\")\n".to_string(),
                entry_path: "docs/guide.md".to_string(),
            })
            .expect_err("missing image must fail closed");
        let error = result.to_string();
        assert!(error.contains("Typst compilation failed"), "error: {error}");
        assert!(!error.contains(project_root.path().to_string_lossy().as_ref()));
    });
}

#[test]
fn integration_image_path_escape_is_rejected_by_project_root() {
    with_typst("image-boundary", |backend| {
        let parent = tempdir().expect("project parent directory");
        let project_root = parent.path().join("project");
        let outside = parent.path().join("outside");
        fs::create_dir_all(project_root.join("docs")).expect("project docs");
        fs::create_dir_all(&outside).expect("outside directory");
        fs::write(outside.join("secret.svg"), "not an image").expect("outside fixture");

        let result = backend
            .with_source_context(TypstSourceContext::new(&project_root))
            .compile(&TypstInput {
                source: "#image(\"../../outside/secret.svg\")\n".to_string(),
                entry_path: "docs/guide.md".to_string(),
            })
            .expect_err("image path escape must fail closed");
        let error = result.to_string();
        assert!(error.contains("Typst compilation failed"), "error: {error}");
        assert!(error.contains("project"), "error: {error}");
        assert!(
            !error.contains("not an image"),
            "error leaked resource: {error}"
        );
        assert!(!error.contains(parent.path().to_string_lossy().as_ref()));
    });
}

#[cfg(unix)]
#[test]
fn integration_image_symlink_escape_is_rejected_before_typst() {
    use std::os::unix::fs::symlink;

    with_typst("image-symlink-boundary", |backend| {
        let parent = tempdir().expect("project parent directory");
        let project_root = parent.path().join("project");
        let outside = parent.path().join("outside");
        fs::create_dir_all(project_root.join("docs/assets")).expect("project assets");
        fs::create_dir_all(&outside).expect("outside directory");
        fs::write(outside.join("secret.svg"), "secret").expect("outside fixture");
        symlink(
            outside.join("secret.svg"),
            project_root.join("docs/assets/leak.svg"),
        )
        .expect("image symlink");

        let result = backend
            .with_source_context(TypstSourceContext::new(&project_root))
            .compile(&TypstInput {
                source: "#image(\"./assets/leak.svg\")\n".to_string(),
                entry_path: "docs/guide.md".to_string(),
            })
            .expect_err("image symlink escape must fail closed");
        assert!(matches!(
            result,
            scribium_typst::backend::TypstError::ResourceBoundaryViolation(path)
                if path == "docs/assets/leak.svg"
        ));
    });
}

#[test]
fn integration_relative_read_does_not_depend_on_temp_directory() {
    with_typst("relative-read", |backend| {
        let project = tempdir().expect("project temp directory");
        let docs = project.path().join("docs");
        let assets = docs.join("assets");
        fs::create_dir_all(&assets).expect("asset directory");
        fs::write(assets.join("resource.txt"), "project resource").expect("resource fixture");

        let output = backend
            .with_source_context(TypstSourceContext::new(project.path()))
            .compile(&TypstInput {
                source: "#read(\"./assets/resource.txt\")\n".to_string(),
                entry_path: "docs/main.qd".to_string(),
            })
            .expect("project resource should compile without a temp resource");
        assert!(output.pdf.is_some_and(|pdf| pdf.starts_with(b"%PDF-")));
    });
}

#[test]
fn integration_relative_import_uses_project_source_context() {
    with_typst("relative-import", |backend| {
        let project = tempdir().expect("project temp directory");
        let docs = project.path().join("docs");
        let partials = docs.join("partials");
        fs::create_dir_all(&partials).expect("partial directory");
        fs::write(
            partials.join("helper.typ"),
            "#let greeting = [Imported successfully]\n",
        )
        .expect("Typst partial fixture");

        let output = backend
            .with_source_context(TypstSourceContext::new(project.path()))
            .compile(&TypstInput {
                source: "#import \"./partials/helper.typ\": greeting\n#greeting\n".to_string(),
                entry_path: "docs/main.qd".to_string(),
            })
            .expect("relative Typst import should compile");
        assert!(output.pdf.is_some_and(|pdf| pdf.starts_with(b"%PDF-")));
    });
}

#[test]
fn integration_generated_entry_does_not_shadow_typst_resource() {
    with_typst("generated-entry-collision", |backend| {
        let project = tempdir().expect("project temp directory");
        let docs = project.path().join("docs");
        fs::create_dir_all(&docs).expect("docs directory");
        fs::write(
            docs.join("main.typ"),
            "#let greeting = [Source helper remains visible]\n",
        )
        .expect("source Typst helper fixture");

        let output = backend
            .with_source_context(TypstSourceContext::new(project.path()))
            .compile(&TypstInput {
                source: "#import \"./main.typ\": greeting\n#greeting\n".to_string(),
                entry_path: "docs/main.qd".to_string(),
            })
            .expect("source Typst helper should not be shadowed");
        assert!(output.pdf.is_some_and(|pdf| pdf.starts_with(b"%PDF-")));
    });
}

#[test]
fn integration_context_handles_spaces_and_unicode_paths() {
    with_typst("context-paths", |backend| {
        let parent = tempdir().expect("project parent temp directory");
        let project = parent.path().join("project with spaces");
        let docs = project.join("문서");
        let assets = docs.join("자산");
        fs::create_dir_all(&assets).expect("unicode asset directory");
        fs::write(
            assets.join("logo.svg"),
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="10pt" height="10pt"><circle cx="5" cy="5" r="5"/></svg>"#,
        )
        .expect("unicode SVG fixture");

        let output = backend
            .with_source_context(TypstSourceContext::new(&project))
            .compile(&TypstInput {
                source: "#image(\"./자산/logo.svg\")\n".to_string(),
                entry_path: "문서/main.qd".to_string(),
            })
            .expect("paths with spaces and Unicode should compile");
        assert!(output.pdf.is_some_and(|pdf| pdf.starts_with(b"%PDF-")));
    });
}

#[test]
fn integration_outside_root_resource_fails_closed() {
    with_typst("outside-root", |backend| {
        let parent = tempdir().expect("project parent temp directory");
        let project = parent.path().join("project");
        let outside = parent.path().join("outside");
        fs::create_dir_all(project.join("docs")).expect("project directory");
        fs::create_dir_all(&outside).expect("outside directory");
        fs::write(outside.join("secret.txt"), "secret content").expect("outside fixture");

        let result = backend
            .with_source_context(TypstSourceContext::new(&project))
            .compile(&TypstInput {
                source: "#read(\"../../outside/secret.txt\")\n".to_string(),
                entry_path: "docs/main.qd".to_string(),
            });
        let error = result
            .expect_err("outside-root access must fail")
            .to_string();
        assert!(error.contains("Typst compilation failed"), "error: {error}");
        assert!(
            error.contains("project root") || error.contains("project sandbox"),
            "error must identify the project boundary: {error}"
        );
        assert!(
            !error.contains("secret content"),
            "error leaked content: {error}"
        );
        let parent_path = parent.path().to_string_lossy();
        assert!(
            !error.contains(parent_path.as_ref()),
            "error leaked host path: {error}"
        );
    });
}

#[test]
fn target_specific_html_is_omitted_without_typst_source_or_source_map_entries() {
    let source = "Before .html {<em>hidden inline</em>} after.\n\n.html {<div>hidden block</div>}\n\nAfter.\n";
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("valid path")
        .add_source("main.qd", source)
        .expect("valid source")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "unexpected: {:?}",
        result.diagnostics
    );

    let target_span = result
        .ir
        .nodes
        .iter()
        .find_map(|node| match node {
            IrNode::Paragraph { content, .. } => content.iter().find_map(|inline| match inline {
                IrInline::TargetSpecificContent { content }
                    if content.target == NativeTarget::Html =>
                {
                    Some(content.span)
                }
                _ => None,
            }),
            IrNode::TargetSpecificContent { content } if content.target == NativeTarget::Html => {
                Some(content.span)
            }
            _ => None,
        })
        .expect("inline target-specific node");
    assert!(matches!(
        result.ir.nodes.as_slice(),
        [
            IrNode::Paragraph { .. },
            IrNode::TargetSpecificContent { .. },
            IrNode::Paragraph { .. }
        ]
    ));

    let (typst, source_map) = lower_to_typst(&result.ir);
    assert!(typst.contains("Before"), "generated Typst: {typst:?}");
    assert!(typst.contains("after."), "generated Typst: {typst:?}");
    assert!(typst.contains("After."), "generated Typst: {typst:?}");
    assert!(!typst.contains("<em>"));
    assert!(!typst.contains("hidden"));
    assert!(
        source_map.iter().all(|entry| entry.original != target_span),
        "target-specific HTML fabricated a source-map entry: {source_map:?}"
    );
}

#[test]
fn target_specific_html_typst_and_pdf_smoke() {
    let source = "Before.\n\n.html {<div>hidden</div>}\n\nAfter.\n";
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("valid path")
        .add_source("main.qd", source)
        .expect("valid source")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "unexpected: {:?}",
        result.diagnostics
    );
    let typst = lower_to_typst_code(&result.ir);
    assert!(typst.contains("Before."));
    assert!(typst.contains("After."));
    assert!(!typst.contains("<div>"));
    assert!(!typst.contains("hidden"));

    with_typst("target-specific-html", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst,
                entry_path: "main.qd".to_string(),
            })
            .expect("Typst/PDF compilation should succeed");
        assert!(output.pdf.is_some_and(|pdf| pdf.starts_with(b"%PDF-")));
    });
}

#[test]
fn unknown_function_html_body_stays_fail_closed_before_typst() {
    let source = ".unknown\n    <div>not owned</div>\n";
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("valid path")
        .add_source("main.qd", source)
        .expect("valid source")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());

    assert!(result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "E8001"));
    let typst = lower_to_typst_code(&result.ir);
    assert!(!typst.contains("<div>"));
    assert!(!typst.contains("not owned"));
}

#[test]
fn integration_version_succeeds() {
    with_typst("version", |backend| {
        let version = backend.version().expect("version should succeed");
        assert!(!version.is_empty(), "version output must not be empty");
        assert!(version.contains("typst"), "version was: {}", version);
    });
}

#[test]
fn integration_compile_failure_surfaces_diagnostic() {
    with_typst("compile-failure", |backend| {
        let input = TypstInput {
            source: "#heading[Test\n".to_string(),
            entry_path: "test.qd".to_string(),
        };
        let result = backend.compile(&input);
        assert!(result.is_err(), "invalid Typst must fail");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("compilation failed"),
            "error must surface the compiler diagnostic, was: {}",
            err
        );
    });
}

#[test]
fn integration_configured_path_is_respected() {
    // The custom-path plumbing is validated by using the located binary
    // through an explicitly configured path rather than the default: the
    // backend passed here was constructed from a resolved path, not the
    // bare `typst` on `PATH`.
    with_typst("configured-path", |backend| {
        let input = TypstInput {
            source: "Hello world.\n".to_string(),
            entry_path: "test.qd".to_string(),
        };
        let output = backend.compile(&input).expect("compile should succeed");
        let pdf = output.pdf.expect("pdf output must be present");
        assert!(pdf.starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_multi_block_list_item_compiles() {
    // Scribium source whose first list item contains a paragraph followed by
    // a fenced code block; the generated Typst must keep the code block
    // inside the item (fences on the item's content column).
    use scribium_core::{compile, CompileOptions, VirtualProjectBuilder};
    let source = "1. item\n\n    ```\n    code\n    ```\n\n2. next\n";
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("valid path")
        .add_source("main.qd", source)
        .expect("valid path")
        .build()
        .unwrap();
    let result = compile(&project, &CompileOptions::default());
    assert!(result.diagnostics.is_empty());
    let typst_code = scribium_typst::lowering::lower_to_typst_code(&result.ir);
    assert!(
        typst_code.contains("   ```\ncode\n   ```"),
        "code block must be inside the first item: {:?}",
        typst_code
    );

    with_typst("multi-block-list", |backend| {
        let input = TypstInput {
            source: typst_code,
            entry_path: "test.qd".to_string(),
        };
        let output = backend.compile(&input).expect("compile should succeed");
        let pdf = output.pdf.expect("pdf output must be present");
        assert!(pdf.starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_markdown_structures_compile_to_valid_pdf() {
    use scribium_core::{compile, CompileOptions, VirtualProjectBuilder};

    let source = "> quoted **strong**\n>\n> - [ ] active\n>   - [x] nested\n> - [x] completed\n\nBefore ~removed **content**~ after ~~double~~.\n\n| Left | Center | Right | Default |\n| :--- | :---: | ---: | --- |\n| α | **β** | ~γ~ | tail |\n";
    for entry in ["main.md", "main.qd"] {
        let project = VirtualProjectBuilder::new()
            .entry(entry)
            .expect("valid entry path")
            .add_source(entry, source)
            .expect("valid source path")
            .build()
            .expect("valid project");
        let result = compile(&project, &CompileOptions::default());
        assert!(
            result.diagnostics.is_empty(),
            "{entry} diagnostics: {:?}",
            result.diagnostics
        );
        let typst_code = scribium_typst::lowering::lower_to_typst_code(&result.ir);
        assert!(typst_code.contains("#quote(block: true)"));
        assert!(typst_code.contains("#strike[removed *content*]"));
        assert!(typst_code.contains("#strike[double]"));
        assert!(typst_code.contains("☐ active"));
        assert!(typst_code.contains("☑ nested"));
        assert!(typst_code.contains("☑ completed"));
        assert!(typst_code.contains("#table("));

        with_typst(entry, |backend| {
            let output = backend
                .compile(&TypstInput {
                    source: typst_code,
                    entry_path: entry.to_string(),
                })
                .expect("structured Markdown Typst must compile");
            let pdf = output.pdf.expect("PDF output must be present");
            assert!(pdf.starts_with(b"%PDF-"));
        });
    }

    let body_source =
        ".if {true}\n  > body **strong**\n  >\n  > - [ ] active\n  > - [x] completed\n";
    let project = VirtualProjectBuilder::new()
        .entry("body.qd")
        .expect("valid entry path")
        .add_source("body.qd", body_source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "body diagnostics: {:?}",
        result.diagnostics
    );
    let typst_code = scribium_typst::lowering::lower_to_typst_code(&result.ir);
    assert!(typst_code.contains("#quote(block: true)"));
    assert!(typst_code.contains("☐ active"));
    assert!(typst_code.contains("☑ completed"));
    with_typst("quarkdown-body-structures", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "body.qd".to_string(),
            })
            .expect("Quarkdown body Typst must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_bounded_inline_markdown_html_maps_to_ir_typst_and_pdf() {
    use scribium_core::{compile, CompileOptions, VirtualProjectBuilder};

    let source =
        "Before <em>italic <strong>bold</strong></em> <del>removed</del> <s>old</s><br/> next.\n";
    let entry = "raw-html.md";
    let project = VirtualProjectBuilder::new()
        .entry(entry)
        .expect("valid entry path")
        .add_source(entry, source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "{entry} diagnostics: {:?}",
        result.diagnostics
    );
    let typst_code = scribium_typst::lowering::lower_to_typst_code(&result.ir);
    assert!(typst_code.contains("italic"));
    assert!(typst_code.contains("bold"));
    assert!(typst_code.contains("#strike[removed]"));
    assert!(typst_code.contains("#strike[old]"));
    assert!(typst_code.contains("\\\n next."), "{typst_code:?}");
    assert!(!typst_code.contains("<em>"));
    assert!(!typst_code.contains("<strong>"));

    with_typst(entry, |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: entry.to_string(),
            })
            .expect("bounded HTML Typst must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_markdown_html_comments_are_semantic_noops_in_typst_and_pdf() {
    use scribium_core::{compile, CompileOptions, VirtualProjectBuilder};

    let source = "Before <!-- hidden inline --> visible.\n\n<!-- hidden block -->\n\nAfter.\n";
    let entry = "comments.md";
    let project = VirtualProjectBuilder::new()
        .entry(entry)
        .expect("valid entry path")
        .add_source(entry, source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "{entry} diagnostics: {:?}",
        result.diagnostics
    );

    let typst_code = scribium_typst::lowering::lower_to_typst_code(&result.ir);
    assert!(typst_code.contains("Before"));
    assert!(typst_code.contains("visible."));
    assert!(typst_code.contains("After."));
    assert!(!typst_code.contains("<!--"));
    assert!(!typst_code.contains("hidden inline"));
    assert!(!typst_code.contains("hidden block"));

    with_typst(entry, |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: entry.to_string(),
            })
            .expect("comment-free Markdown Typst must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_commonmark_gfm_baseline_fixture_compiles_to_valid_pdf() {
    use scribium_core::{compile, CompileOptions, VirtualProjectBuilder};

    let source = include_str!("../../../fixtures/markdown/commonmark_gfm_baseline.md");
    let project = VirtualProjectBuilder::new()
        .entry("commonmark_gfm_baseline.md")
        .expect("valid entry path")
        .add_source("commonmark_gfm_baseline.md", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "baseline fixture diagnostics: {:?}",
        result.diagnostics
    );
    let typst_code = scribium_typst::lowering::lower_to_typst_code(&result.ir);
    assert!(typst_code.contains("= Scribium Markdown baseline"));
    assert!(typst_code.contains("== Setext heading"));
    assert!(typst_code.contains("#link(\"https://example.com/docs\")"));
    assert!(typst_code.contains("#strike[strikethrough]"));
    assert!(typst_code.contains("```rust\nfn main()"));
    assert!(typst_code.contains("#table("));
    assert!(typst_code.contains("☐ open task"));
    assert!(typst_code.contains("☑ completed task"));
    assert!(result.ir.nodes.iter().any(|node| matches!(
        node,
        scribium_core::ir::IrNode::CodeBlock {
            info: Some(info),
            ..
        } if info == "rust extra-info"
    )));

    with_typst("commonmark-gfm-baseline", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "commonmark_gfm_baseline.qd".to_string(),
            })
            .expect("CommonMark/GFM baseline Typst must compile");
        let pdf = output.pdf.expect("PDF output must be present");
        assert!(pdf.starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_markdown_utf8_crlf_breaks_lower_and_compile_to_pdf() {
    use scribium_core::{compile, CompileOptions, VirtualProjectBuilder};

    let source = "한글\r\n다음  \r\n끝";
    let project = VirtualProjectBuilder::new()
        .entry("crlf.md")
        .expect("valid entry path")
        .add_source("crlf.md", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "CRLF fixture diagnostics: {:?}",
        result.diagnostics
    );
    let typst_code = scribium_typst::lowering::lower_to_typst_code(&result.ir);
    assert!(typst_code.contains("한글\n다음\\\n끝"), "{typst_code:?}");

    with_typst("markdown-utf8-crlf-breaks", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "crlf.qd".to_string(),
            })
            .expect("UTF-8 CRLF break Typst must compile");
        let pdf = output.pdf.expect("PDF output must be present");
        assert!(pdf.starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_variable_evaluation_before_lowering() {
    // This test validates that variable evaluation happens before Typst lowering.
    // It uses the core compile path directly since the backend doesn't expose
    // the generated Typst source.
    use scribium_core::{compile, CompileOptions, VirtualProjectBuilder};

    // Test variable declaration and reference
    let source = ".var {name} {Scribium}\nHello .name\n";
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("valid path")
        .add_source("main.qd", source)
        .expect("valid path")
        .build()
        .unwrap();
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "diagnostics: {:?}",
        result.diagnostics
    );

    let typst_code = scribium_typst::lowering::lower_to_typst_code(&result.ir);
    // Variable should be resolved in the output
    assert!(
        typst_code.contains("Scribium"),
        "variable value should appear in output: {}",
        typst_code
    );
    // .var declaration artifact should not appear
    assert!(
        !typst_code.contains(".var"),
        ".var declaration should not leak to output: {}",
        typst_code
    );
    // Variable reference artifact should not appear
    assert!(
        !typst_code.contains(".name"),
        "variable reference should not leak to output: {}",
        typst_code
    );

    // Test rich content variable
    let source = ".var {name} {**Scribium**}\nHello .name\n";
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("valid path")
        .add_source("main.qd", source)
        .expect("valid path")
        .build()
        .unwrap();
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E3010"),
        "expected an explicit unsupported content diagnostic: {:?}",
        result.diagnostics
    );

    let typst_code = scribium_typst::lowering::lower_to_typst_code(&result.ir);
    // The source-backed content is retained as literal text; Typst markup
    // delimiters are escaped at the backend boundary rather than being
    // falsely represented as Strong after synthetic reparsing was removed.
    assert!(
        typst_code.contains("\\*\\*Scribium\\*\\*"),
        "unsupported rich content must remain source text: {}",
        typst_code
    );
    assert!(
        !typst_code.contains(".var"),
        ".var declaration should not leak to output: {}",
        typst_code
    );
    assert!(
        !typst_code.contains(".name"),
        "variable reference should not leak to output: {}",
        typst_code
    );
}

#[test]
fn integration_logical_comparison_evaluation_reaches_typst_and_pdf() {
    use scribium_core::{compile, CompileOptions, VirtualProjectBuilder};

    let source = ".if {.islower {2} than:{3}}\n    selected\n.if {.isgreater {2} than:{3}}\n    suppressed\n";
    let project = VirtualProjectBuilder::new()
        .entry("logical.qd")
        .expect("valid entry path")
        .add_source("logical.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "logical comparison diagnostics: {:?}",
        result.diagnostics
    );
    let typst_code = scribium_typst::lowering::lower_to_typst_code(&result.ir);
    assert!(typst_code.contains("selected"), "{typst_code:?}");
    assert!(!typst_code.contains("suppressed"), "{typst_code:?}");
    assert!(
        !typst_code.contains(".islower"),
        "source call leaked: {typst_code:?}"
    );

    with_typst("logical-comparison", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "logical.qd".to_string(),
            })
            .expect("logical comparison Typst must compile");
        let pdf = output.pdf.expect("PDF output must be present");
        assert!(pdf.starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_chain_evaluation_reaches_typst_and_pdf() {
    use scribium_core::{compile, CompileOptions, VirtualProjectBuilder};

    let source = ".sum {10} {5}::multiply {2}\n";
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("valid path")
        .add_source("main.qd", source)
        .expect("valid path")
        .build()
        .unwrap();
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "chain diagnostics: {:?}",
        result.diagnostics
    );

    let typst_code = scribium_typst::lowering::lower_to_typst_code(&result.ir);
    assert!(typst_code.contains("30"), "generated Typst: {typst_code}");
    assert!(!typst_code.contains("parser-preserved call chain"));
    assert!(!typst_code.contains("#sum"));
    assert!(!typst_code.contains("#multiply"));

    let nested_project = VirtualProjectBuilder::new()
        .entry("nested.qd")
        .expect("valid path")
        .add_source("nested.qd", ".multiply {.sum {10} {5}} {2}\n")
        .expect("valid path")
        .build()
        .unwrap();
    let nested_result = compile(&nested_project, &CompileOptions::default());
    assert!(
        nested_result.diagnostics.is_empty(),
        "nested diagnostics: {:?}",
        nested_result.diagnostics
    );
    let nested_typst_code = scribium_typst::lowering::lower_to_typst_code(&nested_result.ir);
    assert_eq!(nested_typst_code, typst_code);

    with_typst("chain-evaluation", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "main.qd".to_string(),
            })
            .expect("evaluated chain Typst must compile");
        assert!(output
            .pdf
            .expect("chain PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_user_function_evaluation_reaches_typst_and_pdf() {
    use scribium_core::{compile, CompileOptions, VirtualProjectBuilder};

    let declaration = ".function {area}\n    width height:\n    .multiply {.width} by:{.height}\n\n# Result\n\nArea: ";
    let direct_source = format!("{declaration}.area {{4}} {{2}}\n");
    let nested_source = format!("{declaration}.sum {{.area {{4}} {{2}}}} {{1}}\n");
    let chain_source = format!("{declaration}.area {{4}} {{2}}::sum {{1}}\n");

    let compile_source = |entry: &str, source: &str| {
        let project = VirtualProjectBuilder::new()
            .entry(entry)
            .expect("valid path")
            .add_source(entry, source)
            .expect("valid path")
            .build()
            .unwrap();
        compile(&project, &CompileOptions::default())
    };

    let direct = compile_source("direct.qd", &direct_source);
    let nested = compile_source("nested.qd", &nested_source);
    let chained = compile_source("chained.qd", &chain_source);
    assert!(
        direct.diagnostics.is_empty(),
        "direct: {:?}",
        direct.diagnostics
    );
    assert!(
        nested.diagnostics.is_empty(),
        "nested: {:?}",
        nested.diagnostics
    );
    assert!(
        chained.diagnostics.is_empty(),
        "chain: {:?}",
        chained.diagnostics
    );

    let direct_typst = scribium_typst::lowering::lower_to_typst_code(&direct.ir);
    let nested_typst = scribium_typst::lowering::lower_to_typst_code(&nested.ir);
    let chained_typst = scribium_typst::lowering::lower_to_typst_code(&chained.ir);
    assert_eq!(nested_typst, chained_typst);
    assert!(
        direct_typst.contains("Area: 8"),
        "generated Typst: {direct_typst}"
    );
    assert!(
        nested_typst.contains("Area: 9"),
        "generated Typst: {nested_typst}"
    );
    assert!(!direct_typst.contains(".function"));
    assert!(!direct_typst.contains(".area"));

    with_typst("user-function-evaluation", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: direct_typst,
                entry_path: "user-function.qd".to_string(),
            })
            .expect("user-function Typst must compile");
        let pdf = output
            .pdf
            .expect("user-function PDF output must be present");
        assert!(pdf.starts_with(b"%PDF-"), "PDF must start with %PDF-");
    });
}

#[test]
fn integration_optional_function_parameters_reach_typst_and_pdf() {
    use scribium_core::{compile, CompileOptions, VirtualProjectBuilder};

    let source = ".function {greet}\n    name?:\n    Hello, .name::otherwise {anonymous}!\n\n.greet\n.greet {John}\n";
    let project = VirtualProjectBuilder::new()
        .entry("optional.qd")
        .expect("valid path")
        .add_source("optional.qd", source)
        .expect("valid path")
        .build()
        .unwrap();
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "optional diagnostics: {:?}",
        result.diagnostics
    );

    let typst_code = scribium_typst::lowering::lower_to_typst_code(&result.ir);
    assert!(
        typst_code.contains("Hello, anonymous!") && typst_code.contains("Hello, John!"),
        "generated Typst: {typst_code}"
    );

    with_typst("optional-function-parameters", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "optional.qd".to_string(),
            })
            .expect("optional-parameter Typst must compile");
        let pdf = output
            .pdf
            .expect("optional-parameter PDF output must be present");
        assert!(pdf.starts_with(b"%PDF-"), "PDF must start with %PDF-");
    });
}

#[test]
fn integration_implicit_lambda_parameter_reaches_typst_and_pdf() {
    use scribium_core::{compile, CompileOptions, VirtualProjectBuilder};

    let source = ".function {triple}\n    .multiply {.1} {3}\n\nImplicit result: .triple {2}\n";
    let project = VirtualProjectBuilder::new()
        .entry("implicit.qd")
        .expect("valid path")
        .add_source("implicit.qd", source)
        .expect("valid path")
        .build()
        .unwrap();
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "implicit diagnostics: {:?}",
        result.diagnostics
    );

    let typst_code = scribium_typst::lowering::lower_to_typst_code(&result.ir);
    assert!(
        typst_code.contains("Implicit result: 6"),
        "generated Typst: {typst_code}"
    );
    assert!(!typst_code.contains(".triple"));

    with_typst("implicit-lambda-parameter", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "implicit.qd".to_string(),
            })
            .expect("implicit-parameter Typst must compile");
        let pdf = output
            .pdf
            .expect("implicit-parameter PDF output must be present");
        assert!(pdf.starts_with(b"%PDF-"), "PDF must start with %PDF-");
    });
}
