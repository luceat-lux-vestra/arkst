use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};
use arkst_typst::{TypstBackend, TypstInput};
use arkst_typst_subprocess::SubprocessBackend;
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

const PARAGRAPHS: usize = 100;
const MULTI_DOCUMENTS: usize = 4;

fn find_typst() -> PathBuf {
    if let Some(path) = std::env::var_os("ARKST_TYPST_PATH") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return path;
        }
    }
    if Command::new("typst")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
    {
        return PathBuf::from("typst");
    }
    let homebrew = PathBuf::from("/opt/homebrew/bin/typst");
    assert!(homebrew.is_file(), "a Typst executable is required");
    homebrew
}

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

    let input = document(0);

    let backend = SubprocessBackend::new(find_typst());
    let durations = (0..runs)
        .map(|_| {
            let start = Instant::now();
            backend
                .compile(&input)
                .expect("subprocess compile")
                .pdf
                .expect("subprocess PDF");
            start.elapsed()
        })
        .collect::<Vec<_>>();

    println!("runs={}", durations.len());
    println!("workload=generated Arkst Typst, {PARAGRAPHS} paragraphs");
    println!("subprocess_ms={:?}", millis(&durations));

    let documents = (0..MULTI_DOCUMENTS).map(document).collect::<Vec<_>>();
    let first_pass = multi_document_pass(&documents, &backend);
    let second_pass = multi_document_pass(&documents, &backend);
    println!("multi_document_count={MULTI_DOCUMENTS}");
    println!("multi_document_first_pass_ms={:?}", millis(&first_pass));
    println!("multi_document_second_pass_ms={:?}", millis(&second_pass));
}

fn document(index: usize) -> TypstInput {
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
    TypstInput {
        source: arkst_typst::lowering::lower_to_typst_code(&result.ir),
        entry_path: "docs/main.qd".to_string(),
    }
}

fn multi_document_pass(
    documents: &[TypstInput],
    backend: &SubprocessBackend,
) -> Vec<std::time::Duration> {
    documents
        .iter()
        .map(|input| {
            let start = Instant::now();
            backend
                .compile(input)
                .expect("subprocess multi-document compile")
                .pdf
                .expect("subprocess multi-document PDF");
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
