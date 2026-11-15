//! Cross-crate evaluator-to-lowering regressions.
//!
//! These cases need the core orchestration facade in addition to the pure
//! lowering crate, so they live with the native adapter's integration suite
//! rather than adding an upward production dependency to `arkst-typst`.

use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};
use arkst_typst::lowering::{lower_to_typst, lower_to_typst_code};

#[test]
fn end_to_end_link_compiles_to_typst_link() {
    let source =
        "# Links\n\nVisit [Typst](https://typst.app).\n\nThis is a [**bold link**](https://example.com).\n";
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("valid path")
        .add_source("main.qd", source)
        .expect("valid path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(result.diagnostics.is_empty());
    let code = lower_to_typst_code(&result.ir);
    assert!(code.contains("#link(\"https://typst.app\")[Typst]"));
    assert!(code.contains("#link(\"https://example.com\")[*bold link*]"));
}

#[test]
fn end_to_end_code_span_compiles_to_typst_raw() {
    let source = "# Code\n\nRun `cargo run`.\n\nUse ``foo ` bar`` when discussing backticks.\n\nLiteral syntax: `**not bold**`.\n";
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("valid path")
        .add_source("main.qd", source)
        .expect("valid path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(result.diagnostics.is_empty());
    let code = lower_to_typst_code(&result.ir);
    assert!(code.contains("#raw(\"cargo run\")"));
    assert!(code.contains("#raw(\"foo ` bar\")"));
    assert!(code.contains("#raw(\"**not bold**\")"));
    assert!(!code.contains("#link("));
}

#[test]
fn end_to_end_nested_ordered_list() {
    // Real Markdown source with a nested ordered list inside a parent item.
    // The inner list uses the parent's content column, the second item
    // "2. sibling" is a top-level sibling of the first one.
    let source = "1. parent\n    1. child\n    2. child2\n2. sibling\n";
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("valid path")
        .add_source("main.qd", source)
        .expect("valid path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(result.diagnostics.is_empty());
    let code = lower_to_typst_code(&result.ir);
    // The nested list must be indented inside the first item and must not
    // be flattened into top-level items.
    assert_eq!(
        code,
        "1. parent\n  1. child\n\n  2. child2\n\n\n2. sibling\n\n\n"
    );
}

#[test]
fn evaluated_direct_range_failure_leaves_no_typst_none_placeholder() {
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("valid path")
        .add_source("main.qd", ".var {r} {2..4}\n.r\n")
        .expect("valid path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert_eq!(result.diagnostics.len(), 1);
    let code = lower_to_typst_code(&result.ir);
    assert!(code.is_empty());
    assert!(!code.contains("none"));
}

#[test]
fn source_map_is_independent_of_source_insertion_order() {
    let project1 = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("valid path")
        .add_source("a.qd", "content a")
        .expect("valid path")
        .add_source("b.qd", "content b")
        .expect("valid path")
        .add_source("main.qd", "# Main\n\n{{ a.qd }} {{ b.qd }}")
        .expect("valid path")
        .build()
        .expect("valid project");

    let project2 = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("valid path")
        .add_source("b.qd", "content b")
        .expect("valid path")
        .add_source("a.qd", "content a")
        .expect("valid path")
        .add_source("main.qd", "# Main\n\n{{ a.qd }} {{ b.qd }}")
        .expect("valid path")
        .build()
        .expect("valid project");

    let result1 = compile(&project1, &CompileOptions::default());
    let result2 = compile(&project2, &CompileOptions::default());
    let (typst1, map1) = lower_to_typst(&result1.ir);
    let (typst2, map2) = lower_to_typst(&result2.ir);
    assert_eq!(typst1, typst2);
    assert_eq!(map1, map2);
}

#[test]
fn nested_include_source_map_is_independent_of_source_insertion_order() {
    const MAIN: &str = ".include {partials/child.qd}\n";
    const CHILD: &str = "child text\n";
    const CHILD_PATH: &str = "docs/partials/child.qd";

    let project1 = VirtualProjectBuilder::new()
        .entry("docs/main.qd")
        .expect("valid path")
        .add_source("docs/main.qd", MAIN)
        .expect("valid path")
        .add_source(CHILD_PATH, CHILD)
        .expect("valid path")
        .build()
        .expect("valid project");

    let project2 = VirtualProjectBuilder::new()
        .entry("docs/main.qd")
        .expect("valid path")
        .add_source(CHILD_PATH, CHILD)
        .expect("valid path")
        .add_source("docs/main.qd", MAIN)
        .expect("valid path")
        .build()
        .expect("valid project");

    let result1 = compile(&project1, &CompileOptions::default());
    let result2 = compile(&project2, &CompileOptions::default());
    assert!(result1.diagnostics.is_empty(), "{result1:?}");
    assert!(result2.diagnostics.is_empty(), "{result2:?}");

    let child_id1 = project1
        .sources()
        .iter()
        .find_map(|(source_id, path, _)| (path.as_str() == CHILD_PATH).then_some(source_id))
        .expect("child source id");
    let child_id2 = project2
        .sources()
        .iter()
        .find_map(|(source_id, path, _)| (path.as_str() == CHILD_PATH).then_some(source_id))
        .expect("child source id");

    let (typst1, map1) = lower_to_typst(&result1.ir);
    let (typst2, map2) = lower_to_typst(&result2.ir);
    assert_eq!(typst1, typst2);
    assert_eq!(map1, map2);

    for (child_id, typst, source_map) in [
        (child_id1, typst1.as_str(), map1.as_slice()),
        (child_id2, typst2.as_str(), map2.as_slice()),
    ] {
        let child_entry = source_map
            .iter()
            .find(|entry| entry.original.source_id == child_id)
            .expect("nested child must own a generated source-map range");
        assert!(child_entry.original.start < child_entry.original.end);
        assert!(child_entry.original.end <= CHILD.len());
        assert!(child_entry.generated_start < child_entry.generated_end);
        assert!(child_entry.generated_end <= typst.len());
        assert!(!typst[child_entry.generated_start..child_entry.generated_end].is_empty());
        assert!(!CHILD[child_entry.original.start..child_entry.original.end].is_empty());
    }
}

#[test]
fn conditional_evaluation_before_lowering() {
    let compile_typst = |source: &str| {
        let project = VirtualProjectBuilder::new()
            .entry("main.qd")
            .expect("valid path")
            .add_source("main.qd", source)
            .expect("valid path")
            .build()
            .expect("valid project");
        let result = compile(&project, &CompileOptions::default());
        assert!(result.diagnostics.is_empty());
        lower_to_typst_code(&result.ir)
    };

    let typst = compile_typst(".if {true}\n    kept\n");
    assert!(typst.contains("kept"));
    assert!(!typst.contains("#if"));

    let typst = compile_typst(".if {false}\n    dropped\n");
    assert!(!typst.contains("dropped"));

    let typst = compile_typst(".ifnot {no}\n    kept\n");
    assert!(typst.contains("kept"));

    let typst = compile_typst("before .if {true} {inline} after\n");
    assert!(typst.contains("before"));
    assert!(typst.contains("inline"));
    assert!(typst.contains("after"));
    assert!(!typst.contains("#if"));
}
