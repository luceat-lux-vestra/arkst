use scribium_core::{compile, CompileOptions, VirtualProjectBuilder};
use scribium_typst::{TypstBackend, TypstInput};
use scribium_typst_inprocess::InProcessBackend;
use std::time::Instant;

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

    let source = format!(
        "# Benchmark\n\n{}",
        "A repeated paragraph exercises the same generated Typst workload.\n\n".repeat(100)
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
        source: scribium_typst::lowering::lower_to_typst_code(&result.ir),
        entry_path: "docs/main.qd".to_string(),
    };

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
    println!("workload=generated Scribium Typst, 100 paragraphs");
    println!("inprocess_ms={:?}", millis(&in_process_runs));
}

fn millis(durations: &[std::time::Duration]) -> Vec<u128> {
    durations
        .iter()
        .map(std::time::Duration::as_millis)
        .collect()
}
