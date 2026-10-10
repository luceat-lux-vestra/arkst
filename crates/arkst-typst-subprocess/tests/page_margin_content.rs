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
        .entry("page-margin-content.qd")
        .expect("valid entry")
        .add_source("page-margin-content.qd", source)
        .expect("valid source")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    lower_to_typst_code(&result.ir)
}

#[test]
fn paged_top_and_bottom_center_use_persistent_page_query_state() {
    let typst = lower(
        ".doctype {paged}\n.pagemargin {topcenter}\n    Header A\n.footer\n    Footer A\n\n.pagebreak\n\n.pagemargin {topcenter}\n    Header B\n.footer\n    Footer B\nBody\n",
    );

    assert!(
        typst.contains("#set page(header: grid(columns: (1fr, 1fr, 1fr), none, context {"),
        "{typst}"
    );
    assert!(
        typst.contains("#set page(footer: grid(columns: (1fr, 1fr, 1fr), none, context {"),
        "{typst}"
    );
    assert!(typst.contains(
        "query(<arkst-page-margin-top-center>).filter(it => it.location().page() <= __arkst_page)"
    ));
    assert!(typst.contains(
        "query(<arkst-page-margin-bottom-center>).filter(it => it.location().page() <= __arkst_page)"
    ));
    assert!(typst.contains("__arkst_margin.last().value"));
    assert_eq!(
        typst.matches("<arkst-page-margin-top-center>").count(),
        3,
        "one query label plus two source-order initializers: {typst}"
    );
    assert_eq!(
        typst.matches("<arkst-page-margin-bottom-center>").count(),
        3,
        "one query label plus two source-order initializers: {typst}"
    );

    let first_header = typst
        .find("#metadata([\nHeader A")
        .expect("first header marker");
    let break_pos = typst.find("#pagebreak").expect("page break");
    let second_header = typst
        .find("#metadata([\nHeader B")
        .expect("second header marker");
    assert!(
        first_header < break_pos && break_pos < second_header,
        "{typst}"
    );

    with_typst("paged-central-page-margin", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst,
                entry_path: "page-margin-content.qd".to_string(),
            })
            .expect("paged repeated central margins must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn paged_top_row_composes_left_center_right_slots_without_overwrite() {
    let typst = lower(
        ".doctype {paged}\n.pagemargin {topleft}\n    Left A\n.pagemargin {topcenter}\n    Center A\n.pagemargin {topright}\n    Right A\n\n.pagebreak\n\n.pagemargin {topleft}\n    Left B\nBody\n",
    );

    assert!(
        typst.contains("#set page(header: grid(columns: (1fr, 1fr, 1fr), context {"),
        "{typst}"
    );
    assert_eq!(
        typst.matches("#set page(header:").count(),
        1,
        "top slots must compose into one header channel: {typst}"
    );
    for (label, alignment) in [
        ("arkst-page-margin-top-left", "left"),
        ("arkst-page-margin-top-center", "center"),
        ("arkst-page-margin-top-right", "right"),
    ] {
        assert!(
            typst.contains(&format!(
                "query(<{label}>).filter(it => it.location().page() <= __arkst_page)"
            )),
            "{typst}"
        );
        assert!(
            typst.contains(&format!("align({alignment}, __arkst_margin.last().value)")),
            "{typst}"
        );
    }
    assert_eq!(
        typst.matches("<arkst-page-margin-top-left>").count(),
        3,
        "one query label plus two left initializers: {typst}"
    );
    assert_eq!(
        typst.matches("<arkst-page-margin-top-center>").count(),
        2,
        "one query label plus one center initializer: {typst}"
    );
    assert_eq!(
        typst.matches("<arkst-page-margin-top-right>").count(),
        2,
        "one query label plus one right initializer: {typst}"
    );

    with_typst("paged-top-row-page-margin", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst,
                entry_path: "page-margin-content.qd".to_string(),
            })
            .expect("paged fixed top-row margins must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn paged_bottom_row_composes_left_center_right_slots_and_footer_sugar() {
    let typst = lower(
        ".doctype {paged}\n.pagemargin {bottomleft}\n    Left A\n.footer\n    Center A\n.pagemargin {bottomright}\n    Right A\n\n.pagebreak\n\n.pagemargin {bottomright}\n    Right B\nBody\n",
    );

    assert!(
        typst.contains("#set page(footer: grid(columns: (1fr, 1fr, 1fr), context {"),
        "{typst}"
    );
    assert_eq!(
        typst.matches("#set page(footer:").count(),
        1,
        "bottom slots must compose into one footer channel: {typst}"
    );
    for (label, alignment) in [
        ("arkst-page-margin-bottom-left", "left"),
        ("arkst-page-margin-bottom-center", "center"),
        ("arkst-page-margin-bottom-right", "right"),
    ] {
        assert!(
            typst.contains(&format!(
                "query(<{label}>).filter(it => it.location().page() <= __arkst_page)"
            )),
            "{typst}"
        );
        assert!(
            typst.contains(&format!("align({alignment}, __arkst_margin.last().value)")),
            "{typst}"
        );
    }
    assert_eq!(typst.matches("<arkst-page-margin-bottom-left>").count(), 2);
    assert_eq!(
        typst.matches("<arkst-page-margin-bottom-center>").count(),
        2
    );
    assert_eq!(typst.matches("<arkst-page-margin-bottom-right>").count(), 3);
    assert!(!typst.contains("#set page(header:"));
    let first_right = typst
        .find("#metadata([\nRight A")
        .expect("first right marker");
    let break_pos = typst.find("#pagebreak").expect("page break");
    let second_right = typst
        .find("#metadata([\nRight B")
        .expect("second right marker");
    assert!(
        first_right < break_pos && break_pos < second_right,
        "{typst}"
    );

    with_typst("paged-bottom-row-page-margin", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst,
                entry_path: "page-margin-content.qd".to_string(),
            })
            .expect("paged fixed bottom-row margins must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn paged_bottom_left_only_does_not_invent_other_slots() {
    let typst = lower(".doctype {paged}\n.pagemargin {bottomleft}\n    Left only\nBody\n");
    assert!(
        typst.contains("#set page(footer: grid(columns: (1fr, 1fr, 1fr), context {"),
        "{typst}"
    );
    assert!(typst.contains("else { none }\n}, none, none))"), "{typst}");
    assert!(!typst.contains("#set page(header:"));
    assert!(!typst.contains("query(<arkst-page-margin-bottom-center>)"));
    assert!(!typst.contains("query(<arkst-page-margin-bottom-right>)"));
}

#[test]
fn unsupported_page_margin_positions_and_document_types_fail_closed() {
    for (name, source) in [
        (
            "paged-corner",
            ".doctype {paged}\n.pagemargin {topleftcorner}\n    Unsupported\nBody\n",
        ),
        (
            "slides-footer",
            ".doctype {slides}\n.footer\n    Unsupported\nBody\n",
        ),
        ("plain-footer", ".footer\n    Unsupported\nBody\n"),
        (
            "docs-footer",
            ".doctype {docs}\n.footer\n    Unsupported\nBody\n",
        ),
    ] {
        let typst = lower(source);
        assert!(
            typst.starts_with(
                "#panic(\"Arkst page-margin output currently supports only paged fixed top and bottom left/center/right positions\")"
            ),
            "{name}: {typst}"
        );

        with_typst(name, |backend| {
            let error = backend
                .compile(&TypstInput {
                    source: typst,
                    entry_path: "page-margin-content.qd".to_string(),
                })
                .expect_err("unsupported page-margin output must fail closed");
            assert!(!error.to_string().is_empty());
        });
    }
}
