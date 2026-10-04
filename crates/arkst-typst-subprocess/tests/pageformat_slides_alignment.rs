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

fn compile_slides_alignment(alignment: &str, center: Option<bool>) -> String {
    let slides = match center {
        Some(true) => ".slides center:{true}\n",
        Some(false) => ".slides center:{false}\n",
        None => "",
    };
    let source = format!(
        ".doctype {{slides}}\n{slides}.pageformat alignment:{{{alignment}}}\n\n# Heading\n\nParagraph body.\n"
    );
    let project = VirtualProjectBuilder::new()
        .entry("slides-alignment.qd")
        .expect("valid entry")
        .add_source("slides-alignment.qd", source)
        .expect("valid source")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "slides/{alignment}/{center:?} diagnostics: {:?}",
        result.diagnostics
    );
    lower_to_typst_code(&result.ir)
}

#[test]
fn slides_global_horizontal_alignment_preserves_renderer_owned_vertical_default() {
    for alignment in ["start", "center", "end"] {
        let typst = compile_slides_alignment(alignment, None);
        assert!(
            typst.contains(&format!("#set align({alignment})\n")),
            "slides/{alignment}: {typst}"
        );
        assert!(!typst.contains(" + top"), "slides/{alignment}: {typst}");
        assert!(
            !typst.contains(" + horizon"),
            "slides/{alignment}: {typst}"
        );

        with_typst(&format!("slides-{alignment}-default-vertical"), |backend| {
            let output = backend
                .compile(&TypstInput {
                    source: typst,
                    entry_path: "slides-alignment.qd".to_string(),
                })
                .expect("slides global horizontal alignment must compile");
            assert!(output
                .pdf
                .expect("PDF output must be present")
                .starts_with(b"%PDF-"));
        });
    }
}

#[test]
fn slides_global_horizontal_alignment_composes_with_explicit_vertical_alignment() {
    for alignment in ["start", "center", "end"] {
        for (center, vertical) in [(true, "horizon"), (false, "top")] {
            let typst = compile_slides_alignment(alignment, Some(center));
            let expected = format!("#set align({alignment} + {vertical})\n");
            assert!(typst.contains(&expected), "slides/{alignment}/{center}: {typst}");
            assert!(
                !typst.contains(&format!("#set align({alignment})\n")),
                "horizontal-only rule must not precede composition: {typst}"
            );
            assert!(
                !typst.contains(&format!("#set align({vertical})\n")),
                "vertical-only rule must not overwrite composition: {typst}"
            );

            with_typst(
                &format!("slides-{alignment}-{vertical}-composition"),
                |backend| {
                    let output = backend
                        .compile(&TypstInput {
                            source: typst,
                            entry_path: "slides-alignment.qd".to_string(),
                        })
                        .expect("composed slides alignment must compile");
                    assert!(output
                        .pdf
                        .expect("PDF output must be present")
                        .starts_with(b"%PDF-"));
                },
            );
        }
    }
}

#[test]
fn slides_justify_remains_local_while_vertical_alignment_stays_independent() {
    let typst = compile_slides_alignment("justify", Some(true));
    assert!(typst.contains("#set par(justify: true)\n"), "{typst}");
    assert!(typst.contains("#set align(horizon)\n"), "{typst}");
    assert!(!typst.contains("justify + horizon"), "{typst}");
}
