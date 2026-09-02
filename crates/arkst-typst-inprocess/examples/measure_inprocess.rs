use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};
use arkst_project::VirtualProject;
use arkst_typst::{TypstBackend, TypstInput};
use arkst_typst_inprocess::InProcessBackend;
use std::time::Instant;

const PARAGRAPHS: usize = 100;
const MULTI_DOCUMENTS: usize = 4;

fn main() {
    let runs = std::env::args()
        .nth(1)
        .map(|value| {
            value
                .parse::<usize>()
                .expect("run count must be an integer")
        })
        .unwrap_or(8);
    assert!(runs >= 2, "run count must be at least two");

    let (project, input) = document(0);

    let backend = InProcessBackend::new(&project);
    let in_process_runs = (0..runs)
        .map(|_| {
            let start = Instant::now();
            backend
                .compile(&input)
                .expect("in-process compile")
                .pdf
                .expect("in-process PDF");
            start.elapsed()
        })
        .collect::<Vec<_>>();

    println!("runs={runs}");
    println!("workload=generated Arkst Typst, {PARAGRAPHS} paragraphs");
    println!("inprocess_ms={:?}", millis(&in_process_runs));

    let documents = (0..MULTI_DOCUMENTS).map(document).collect::<Vec<_>>();
    let first_pass = multi_document_pass(&documents);
    let second_pass = multi_document_pass(&documents);
    println!("multi_document_count={MULTI_DOCUMENTS}");
    println!("multi_document_first_pass_ms={:?}", millis(&first_pass));
    println!("multi_document_second_pass_ms={:?}", millis(&second_pass));
}

fn document(index: usize) -> (VirtualProject, TypstInput) {
    let source = format!(
        "# Benchmark {index}\n\n{}",
        format!("Document {index} repeats a paragraph to exercise a multi-document workload.\n\n")
            .repeat(PARAGRAPHS)
    );
    let project = VirtualProjectBuilder::new()
        .entry("docs/main.qd")
        .expect("entry")
        .add_source("docs/main.qd", source)
        .expect("source")
        .build()
        .expect("project");
    let result = compile(&project, &CompileOptions::default());
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let input = TypstInput {
        source: arkst_typst::lowering::lower_to_typst_code(&result.ir),
        entry_path: "docs/main.qd".to_string(),
    };
    (project, input)
}

fn multi_document_pass(documents: &[(VirtualProject, TypstInput)]) -> Vec<std::time::Duration> {
    documents
        .iter()
        .map(|(project, input)| {
            let start = Instant::now();
            InProcessBackend::new(project)
                .compile(input)
                .expect("in-process multi-document compile")
                .pdf
                .expect("in-process multi-document PDF");
            start.elapsed()
        })
        .collect()
}

fn millis(durations: &[std::time::Duration]) -> Vec<u128> {
    durations
        .iter()
        .map(std::time::Duration::as_millis)
        .collect()
}
