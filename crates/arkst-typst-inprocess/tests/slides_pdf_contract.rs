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

#[test]
fn slides_default_has_evidenced_v260_pdf_geometry() {
    let pdf = compile_pdf(".doctype {slides}\n\nSLIDES-DEFAULT-GEOMETRY\n");
    assert_eq!(page_count(&pdf), 1);
    let (width, height) = first_media_box(&pdf);
    assert_close(width, 749.04);
    assert_close(height, 546.0);
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
}

#[test]
fn adjacent_explicit_breaks_do_not_fabricate_an_empty_pdf_page() {
    let pdf = compile_pdf(
        ".doctype {slides}\n\nFIRST\n\n.pagebreak\n.pagebreak\n\nSECOND\n",
    );
    assert_eq!(page_count(&pdf), 2);
}
