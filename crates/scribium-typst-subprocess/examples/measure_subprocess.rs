use scribium_core::{compile, CompileOptions, VirtualProjectBuilder};
use scribium_typst::{TypstBackend, TypstInput};
use scribium_typst_subprocess::SubprocessBackend;
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

fn find_typst() -> PathBuf {
    if let Some(path) = std::env::var_os("SCRIBIUM_TYPST_PATH") {
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
    println!("workload=generated Scribium Typst, 100 paragraphs");
    println!("subprocess_ms={:?}", millis(&durations));
}

fn millis(durations: &[std::time::Duration]) -> Vec<u128> {
    durations
        .iter()
        .map(std::time::Duration::as_millis)
        .collect()
}
