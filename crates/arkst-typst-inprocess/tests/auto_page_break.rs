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

#[test]
fn slides_default_and_disable_have_real_typst_page_count_evidence() {
    let source = ".doctype {slides}\n\nIntro.\n\n# H1\n\nA.\n\n## H2\n\nB.\n\n### H3\n\nC.\n";
    assert_eq!(page_count(&compile_pdf(source)), 3);

    let disabled = format!(".noautopagebreak\n{source}");
    assert_eq!(page_count(&compile_pdf(&disabled)), 1);
}

#[test]
fn weak_break_at_document_start_does_not_create_an_empty_leading_page() {
    let source = ".doctype {slides}\n\n# First\n\n## Second\n";
    assert_eq!(page_count(&compile_pdf(source)), 2);
}

#[test]
fn nested_heading_does_not_attempt_a_container_pagebreak() {
    let source =
        ".doctype {slides}\n\nIntro.\n\n> Quote lead.\n>\n> ## Nested H2\n>\n> Quote tail.\n";
    assert_eq!(page_count(&compile_pdf(source)), 1);
}
