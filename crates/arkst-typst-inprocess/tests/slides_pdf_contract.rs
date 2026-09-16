use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};
use arkst_typst::{TypstBackend, TypstInput};
use arkst_typst_inprocess::InProcessBackend;

fn project(source: &str) -> arkst_project::VirtualProject {
    VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("entry")
        .add_source("main.qd", source)
        .expect("source")
        .build()
        .expect("project")
}

fn compile_pdf(source: &str) -> Vec<u8> {
    let project = project(source);
    let result = compile(&project, &CompileOptions::default());
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let typst = arkst_typst::lowering::lower_to_typst_code(&result.ir);
    InProcessBackend::new(&project)
        .compile(&TypstInput {
            source: typst,
            entry_path: "main.qd".to_string(),
        })
        .expect("pinned Typst in-process compilation")
        .pdf
        .expect("PDF output")
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
    let marker = "/MediaBox";
    let start = text.find(marker).expect("PDF MediaBox") + marker.len();
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

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 0.05,
        "expected {expected}, got {actual}"
    );
}

fn assert_default_slide_geometry(pdf: &[u8]) {
    let (width, height) = first_media_box(pdf);
    assert_close(width, 749.04);
    assert_close(height, 546.0);
}

#[test]
fn slides_default_has_evidenced_v260_pdf_geometry() {
    let pdf = compile_pdf(".doctype {slides}\n\nSLIDES-DEFAULT-GEOMETRY\n");
    assert_eq!(page_count(&pdf), 1);
    assert_default_slide_geometry(&pdf);
}

#[test]
fn explicit_complete_pageformat_geometry_overrides_slides_default() {
    let pdf = compile_pdf(
        ".doctype {slides}\n.pageformat width:{10in} height:{5in}\n\nSLIDES-EXPLICIT-GEOMETRY\n",
    );
    assert_eq!(page_count(&pdf), 1);
    let (width, height) = first_media_box(&pdf);
    assert_close(width, 720.0);
    assert_close(height, 360.0);
}

#[test]
fn explicit_pagebreak_and_triple_angle_boundaries_produce_real_pdf_pages() {
    let pdf = compile_pdf(
        ".doctype {slides}\n\nFIRST\n\n.pagebreak\n\nSECOND\n\n<<<\n\nTHIRD\n",
    );
    assert_eq!(page_count(&pdf), 3);
    assert_default_slide_geometry(&pdf);
}

#[test]
fn adjacent_explicit_breaks_do_not_fabricate_an_empty_pdf_page() {
    let pdf = compile_pdf(
        ".doctype {slides}\n\nFIRST\n\n.pagebreak\n.pagebreak\n\nSECOND\n",
    );
    assert_eq!(page_count(&pdf), 2);
}

#[test]
fn heading_start_and_automatic_h1_h2_boundaries_have_no_leading_empty_page() {
    let pdf = compile_pdf(".doctype {slides}\n\n# FIRST\n\n## SECOND\n");
    assert_eq!(page_count(&pdf), 2);
    assert_default_slide_geometry(&pdf);
}

#[test]
fn first_and_consecutive_interior_body_only_slides_have_one_page_each() {
    let pdf = compile_pdf(
        ".doctype {slides}\n\nFIRST-BODY\n\n<<<\n\nINTERIOR-A\n\n<<<\n\nINTERIOR-B\n\n<<<\n\nLAST-BODY\n",
    );
    assert_eq!(page_count(&pdf), 4);
    assert_default_slide_geometry(&pdf);
}

#[test]
fn short_and_long_body_only_slides_do_not_gain_accidental_pages() {
    let long_body = std::iter::repeat_n("LONG-BODY-CONTENT", 80)
        .collect::<Vec<_>>()
        .join(" ");
    let source = format!(
        ".doctype {{slides}}\n\nSHORT\n\n<<<\n\n{long_body}\n"
    );
    let pdf = compile_pdf(&source);
    assert_eq!(page_count(&pdf), 2);
    assert_default_slide_geometry(&pdf);
}

#[test]
fn default_focus_and_nullable_center_variants_preserve_slide_page_contract() {
    for source in [
        ".doctype {slides}\n\nDEFAULT\n",
        ".doctype {slides}\n.slides\n\nNULLABLE-CENTER\n",
        ".doctype {slides}\n.slides center:{true}\n\nCENTERED\n",
        ".doctype {slides}\n.slides center:{false}\n\nTOP\n",
        ".doctype {slides}\n.theme layout:{focus}\n\n# FOCUS\n",
    ] {
        let pdf = compile_pdf(source);
        assert_eq!(page_count(&pdf), 1, "source:\n{source}");
        assert_default_slide_geometry(&pdf);
    }
}
