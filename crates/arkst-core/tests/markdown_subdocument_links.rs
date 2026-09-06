use arkst_core::ir::{IrInline, IrNode};
use arkst_core::{compile, CompileOptions, Severity, VirtualProjectBuilder};

fn project(entry: &str, sources: &[(&str, &str)]) -> arkst_core::VirtualProject {
    let mut builder = VirtualProjectBuilder::new()
        .entry(entry)
        .expect("valid entry path");
    for (path, source) in sources {
        builder = builder
            .add_source(*path, *source)
            .expect("valid source path");
    }
    builder.build().expect("valid virtual project")
}

fn compile_project(project: &arkst_core::VirtualProject) -> arkst_core::CompileResult {
    compile(project, &CompileOptions::default())
}

fn link_destinations(result: &arkst_core::CompileResult) -> Vec<&str> {
    result
        .ir
        .nodes
        .iter()
        .filter_map(|node| match node {
            IrNode::Paragraph { content, .. } | IrNode::Heading { content, .. } => Some(content),
            _ => None,
        })
        .flatten()
        .filter_map(|inline| match inline {
            IrInline::Link { destination, .. } => Some(destination.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn real_project_resolves_normalized_target_but_preserves_link_spelling() {
    let project = project(
        "docs/main.qd",
        &[
            ("docs/main.qd", "[Child](parts//tmp/.././child.qd#intro)"),
            ("docs/parts/child.qd", "target"),
        ],
    );

    let result = compile_project(&project);

    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(
        link_destinations(&result),
        vec!["parts//tmp/.././child.qd#intro"]
    );
}

#[test]
fn real_project_static_target_validation_does_not_evaluate_target() {
    let project = project(
        "main.qd",
        &[
            ("main.qd", "[Child](child.qd)"),
            ("child.qd", ".read {missing.txt}"),
        ],
    );

    let result = compile_project(&project);

    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(link_destinations(&result), vec!["child.qd"]);
}

#[test]
fn pure_markdown_entry_does_not_reclassify_local_document_links() {
    let project = project(
        "main.md",
        &[("main.md", "[QD](missing.qd) [MD](missing.md#x)")],
    );

    let result = compile_project(&project);

    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(
        link_destinations(&result),
        vec!["missing.qd", "missing.md#x"]
    );
}

#[test]
fn query_and_similar_suffixes_remain_ordinary_in_quarkdown_sources() {
    let project = project(
        "main.qd",
        &[(
            "main.qd",
            "[Query](missing.qd?download=1) [MDX](missing.mdx) [Bare](missing) [Anchor](#local)",
        )],
    );

    let result = compile_project(&project);

    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(
        link_destinations(&result),
        vec!["missing.qd?download=1", "missing.mdx", "missing", "#local"]
    );
}

#[test]
fn missing_and_disallowed_static_targets_fail_closed_without_host_path_leakage() {
    for reference in [
        "missing.qd",
        "/etc/child.qd",
        "C:/temp/child.qd",
        r"parts\child.qd",
        "https://example.com/child.qd",
        "../../outside.qd",
    ] {
        let source = format!("[Child]({reference})");
        let project = project("docs/main.qd", &[("docs/main.qd", &source)]);
        let result = compile_project(&project);

        assert_eq!(
            result.diagnostics.len(),
            1,
            "{reference}: {:?}",
            result.diagnostics
        );
        assert!(matches!(result.diagnostics[0].severity, Severity::Error));
        assert!(matches!(
            result.diagnostics[0].code.as_str(),
            "E3001" | "E8001"
        ));
        assert!(!result.diagnostics[0].message.contains("/Users/"));
        assert!(!result.diagnostics[0].message.contains("\\Users\\"));
        assert!(link_destinations(&result).is_empty());
    }
}

#[test]
fn included_function_link_resolves_from_its_defining_source_when_called_later() {
    let project = project(
        "docs/main.qd",
        &[
            ("docs/main.qd", ".include {partials/defs.qd}\n.render\n"),
            (
                "docs/partials/defs.qd",
                ".function {render}\n    [Child](data/child.qd#intro)\n",
            ),
            ("docs/partials/data/child.qd", "target"),
        ],
    );

    let result = compile_project(&project);

    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(link_destinations(&result), vec!["data/child.qd#intro"]);
}

#[test]
fn included_markdown_source_keeps_document_links_ordinary() {
    let project = project(
        "main.qd",
        &[
            ("main.qd", ".include {docs/part.md}\n"),
            ("docs/part.md", "[QD](missing.qd) [MD](missing.md#x)"),
        ],
    );

    let result = compile_project(&project);

    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(
        link_destinations(&result),
        vec!["missing.qd", "missing.md#x"]
    );
}
