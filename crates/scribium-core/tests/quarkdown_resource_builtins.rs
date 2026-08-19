use scribium_core::ir::{IrInline, IrNode, IrValue, NativeTarget};
use scribium_core::{compile, CompileOptions, Severity, VirtualProjectBuilder};

fn project(
    entry: &str,
    sources: &[(&str, &str)],
    assets: &[(&str, &[u8])],
) -> scribium_core::VirtualProject {
    let mut builder = VirtualProjectBuilder::new()
        .entry(entry)
        .expect("valid entry path");
    for (path, source) in sources {
        builder = builder
            .add_source(*path, *source)
            .expect("valid source path");
    }
    for (path, asset) in assets {
        builder = builder
            .add_asset(*path, asset.to_vec())
            .expect("valid asset path");
    }
    builder.build().expect("valid virtual project")
}

fn compile_project(project: &scribium_core::VirtualProject) -> scribium_core::CompileResult {
    compile(project, &CompileOptions::default())
}

fn paragraph_text(result: &scribium_core::CompileResult) -> String {
    result
        .ir
        .nodes
        .iter()
        .filter_map(|node| match node {
            IrNode::Paragraph { content, .. } => Some(
                content
                    .iter()
                    .filter_map(|inline| match inline {
                        IrInline::Text { content, .. } => Some(content.as_str()),
                        _ => None,
                    })
                    .collect::<String>(),
            ),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn read_uses_the_calling_source_directory_and_normalizes_line_endings() {
    let project = project(
        "docs/main.qd",
        &[("docs/main.qd", ".read {data/value.txt}\n")],
        &[("docs/data/value.txt", b"one\r\ntwo\rthree")],
    );
    let result = compile_project(&project);
    assert!(
        result.diagnostics.is_empty(),
        "unexpected: {:?}",
        result.diagnostics
    );
    assert_eq!(paragraph_text(&result), "one\ntwo\nthree");
}

#[test]
fn read_supports_parent_relative_resources_and_finite_line_ranges() {
    let project = project(
        "docs/main.qd",
        &[("docs/main.qd", ".read {../shared/value.txt} lines:{2..2}\n")],
        &[("shared/value.txt", b"first\nsecond\nthird")],
    );
    let result = compile_project(&project);
    assert!(
        result.diagnostics.is_empty(),
        "unexpected: {:?}",
        result.diagnostics
    );
    assert_eq!(paragraph_text(&result), "second");
}

#[test]
fn read_rejects_missing_absolute_uri_and_boundary_resources() {
    for reference in [
        "missing.txt",
        "/etc/passwd",
        "https://example.com/value.txt",
        "../../outside.txt",
    ] {
        let source = format!(".read {{{reference}}}\n");
        let project = project("docs/main.qd", &[("docs/main.qd", &source)], &[]);
        let result = compile_project(&project);
        assert_eq!(
            result.diagnostics.len(),
            1,
            "{reference}: {:?}",
            result.diagnostics
        );
        assert!(matches!(result.diagnostics[0].severity, Severity::Error));
        assert_eq!(
            result.diagnostics[0]
                .primary
                .as_ref()
                .map(|span| span.source_id.0),
            Some(1)
        );
        assert!(!result.diagnostics[0].message.contains("/Users/"));
    }
}

#[test]
fn read_rejects_invalid_utf8_without_lossy_conversion() {
    let project = project(
        "main.qd",
        &[("main.qd", ".read {bad.bin}\n")],
        &[("bad.bin", &[0xff, 0xfe])],
    );
    let result = compile_project(&project);
    assert_eq!(result.diagnostics.len(), 1);
    assert!(result.diagnostics[0].message.contains("not valid UTF-8"));
}

#[test]
fn json_is_mapped_to_recursive_typed_evaluator_values() {
    let project = project(
        "main.qd",
        &[("main.qd", ".json {data.json}::getat {1}::second\n")],
        &[(
            "data.json",
            br#"{"enabled":true,"items":[1,2],"missing":null}"#,
        )],
    );
    let result = compile_project(&project);
    assert!(
        result.diagnostics.is_empty(),
        "unexpected: {:?}",
        result.diagnostics
    );
    assert_eq!(paragraph_text(&result), "true");
}

#[test]
fn malformed_json_reports_the_logical_resource_and_call_span() {
    let project = project(
        "main.qd",
        &[("main.qd", ".json {data.json}\n")],
        &[("data.json", br#"{"x":"#)],
    );
    let result = compile_project(&project);
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].code, "E3001");
    assert!(result.diagnostics[0].message.contains("data.json"));
    assert_eq!(
        result.diagnostics[0]
            .primary
            .as_ref()
            .map(|span| span.source_id.0),
        Some(1)
    );
}

#[test]
fn json_rejects_integer_precision_loss_at_the_evaluator_number_boundary() {
    let project = project(
        "main.qd",
        &[("main.qd", ".json {data.json}\n")],
        &[("data.json", br#"{"too_big":9007199254740992}"#)],
    );
    let result = compile_project(&project);
    assert_eq!(result.diagnostics.len(), 1);
    assert!(result.diagnostics[0]
        .message
        .contains("cannot be represented exactly"));
}

#[test]
fn include_changes_the_base_for_nested_source_relative_read() {
    let project = project(
        "docs/main.qd",
        &[
            ("docs/main.qd", ".include {partials/a.qd}\n"),
            ("docs/partials/a.qd", ".read {data/value.txt}\n"),
        ],
        &[("docs/partials/data/value.txt", b"nested value")],
    );
    let result = compile_project(&project);
    assert!(
        result.diagnostics.is_empty(),
        "unexpected: {:?}",
        result.diagnostics
    );
    assert_eq!(paragraph_text(&result), "nested value");
}

#[test]
fn nested_include_reuses_function_lambda_and_resource_context() {
    let project = project(
        "docs/main.qd",
        &[
            ("docs/main.qd", ".include {partials/child.qd}\n"),
            (
                "docs/partials/child.qd",
                ".function {render}\n    text:\n    .ifpresent {.text} {@lambda value: .uppercase {.value}}\n\n.render {.read {data/value.txt}}\n",
            ),
        ],
        &[("docs/partials/data/value.txt", b"nested")],
    );
    let result = compile_project(&project);
    assert!(
        result.diagnostics.is_empty(),
        "unexpected: {:?}",
        result.diagnostics
    );
    assert_eq!(paragraph_text(&result), "NESTED");
}

#[test]
fn nested_include_callback_failure_keeps_child_source_and_atomic_output() {
    let child = ".ifpresent {hello} {@lambda value: .sum {true} {2}}\n";
    let project = project(
        "main.qd",
        &[
            ("main.qd", ".include {partials/child.qd}\n"),
            ("partials/child.qd", child),
        ],
        &[],
    );
    let result = compile_project(&project);
    assert_eq!(
        result.diagnostics.len(),
        1,
        "unexpected: {:?}",
        result.diagnostics
    );
    assert_eq!(result.diagnostics[0].code, "E3001");
    assert_eq!(
        result.diagnostics[0]
            .primary
            .as_ref()
            .map(|span| span.source_id.0),
        Some(2)
    );
    assert_eq!(
        result.diagnostics[0]
            .primary
            .as_ref()
            .map(|span| span.start),
        Some(child.find(".sum").expect("nested callback failure"))
    );
    assert!(result.ir.nodes.is_empty());
}

#[test]
fn nested_resource_failure_keeps_the_included_source_identity() {
    let project = project(
        "docs/main.qd",
        &[
            ("docs/main.qd", ".include {partials/a.qd}\n"),
            ("docs/partials/a.qd", ".read {data/missing.txt}\n"),
        ],
        &[],
    );
    let result = compile_project(&project);
    assert_eq!(result.diagnostics.len(), 1);
    assert!(result.diagnostics[0]
        .message
        .contains("partials/data/missing.txt"));
    assert_eq!(
        result.diagnostics[0]
            .primary
            .as_ref()
            .map(|span| span.source_id.0),
        Some(2)
    );
}

#[test]
fn repeated_include_is_allowed_but_self_and_mutual_cycles_fail() {
    let repeated = project(
        "main.qd",
        &[
            ("main.qd", ".include {common.qd}\n.include {common.qd}\n"),
            ("common.qd", "common\n"),
        ],
        &[],
    );
    let repeated_result = compile_project(&repeated);
    assert!(
        repeated_result.diagnostics.is_empty(),
        "unexpected: {:?}",
        repeated_result.diagnostics
    );
    assert_eq!(paragraph_text(&repeated_result), "common\ncommon");

    let self_cycle = project("main.qd", &[("main.qd", ".include {main.qd}\n")], &[]);
    let self_result = compile_project(&self_cycle);
    assert_eq!(self_result.diagnostics.len(), 1);
    assert!(self_result.diagnostics[0]
        .message
        .contains("cycle detected"));

    let mutual_cycle = project(
        "main.qd",
        &[
            ("main.qd", ".include {a.qd}\n"),
            ("a.qd", ".include {b.qd}\n"),
            ("b.qd", ".include {a.qd}\n"),
        ],
        &[],
    );
    let mutual_result = compile_project(&mutual_cycle);
    assert_eq!(mutual_result.diagnostics.len(), 1);
    assert!(mutual_result.diagnostics[0]
        .message
        .contains("a.qd -> b.qd -> a.qd"));
}

#[test]
fn include_sandbox_modes_match_share_and_scope_visibility() {
    let scope = project(
        "main.qd",
        &[
            (
                "main.qd",
                ".var {value} {main}\n.include {part.qd} sandbox:{scope}\n.value\n",
            ),
            ("part.qd", ".var {value} {part}\n.value\n"),
        ],
        &[],
    );
    let scope_result = compile_project(&scope);
    assert!(
        scope_result.diagnostics.is_empty(),
        "unexpected: {:?}",
        scope_result.diagnostics
    );
    assert_eq!(paragraph_text(&scope_result), "part\nmain");

    let share = project(
        "main.qd",
        &[
            (
                "main.qd",
                ".var {value} {main}\n.include {part.qd}\n.value\n",
            ),
            ("part.qd", ".var {value} {part}\n"),
        ],
        &[],
    );
    let share_result = compile_project(&share);
    assert!(
        share_result.diagnostics.is_empty(),
        "unexpected: {:?}",
        share_result.diagnostics
    );
    assert_eq!(paragraph_text(&share_result), "part");
}

#[test]
fn included_markdown_preserves_source_identity_for_relative_images() {
    let project = project(
        "main.qd",
        &[
            ("main.qd", ".include {docs/part.md}\n"),
            ("docs/part.md", "# Part\n\n![X](assets/x.svg)\n"),
        ],
        &[("docs/assets/x.svg", b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"4\" height=\"4\"><rect width=\"4\" height=\"4\"/></svg>")],
    );
    let result = compile_project(&project);
    assert!(
        result.diagnostics.is_empty(),
        "unexpected: {:?}",
        result.diagnostics
    );
    let images = result
        .ir
        .nodes
        .iter()
        .flat_map(|node| match node {
            IrNode::Paragraph { content, .. } | IrNode::Heading { content, .. } => content,
            _ => &[] as &[IrInline],
        })
        .filter_map(|inline| match inline {
            IrInline::Image {
                destination, span, ..
            } => Some((destination.as_str(), span.source_id.0)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(images, vec![("assets/x.svg", 1)]);
}

#[test]
fn markdown_is_raw_native_content_and_llmstxt_is_explicitly_deferred() {
    let project = project(
        "main.qd",
        &[("main.qd", ".markdown {# raw Markdown}\n.llmstxt\n")],
        &[],
    );
    let result = compile_project(&project);
    assert_eq!(result.diagnostics.len(), 1);
    assert!(result.diagnostics[0].message.contains("llmstxt"));
    assert!(matches!(
        result.ir.nodes.first(),
        Some(IrNode::TargetSpecificContent { content }) if content.target == NativeTarget::Markdown
    ));
    assert!(
        matches!(result.ir.nodes.first(), Some(IrNode::TargetSpecificContent { content }) if matches!(IrValue::String(content.content.clone()), IrValue::String(_)))
    );
}
