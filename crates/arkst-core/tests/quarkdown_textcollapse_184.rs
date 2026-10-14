use arkst_core::ir::{IrInline, IrNode};
use arkst_core::{compile, CompileOptions, SourceId, VirtualProjectBuilder};

fn compile_source(source: &str) -> arkst_core::CompileResult {
    compile_source_with_id(source).0
}

fn compile_source_with_id(source: &str) -> (arkst_core::CompileResult, SourceId) {
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("entry")
        .add_source("main.qd", source)
        .expect("source")
        .build()
        .expect("project");
    let id = project
        .sources()
        .get_id(project.entry())
        .expect("source id");
    (compile(&project, &CompileOptions::default()), id)
}

fn text_collapse(result: &arkst_core::CompileResult) -> (&[IrInline], &[IrInline], bool) {
    let [IrNode::Paragraph { content, .. }] = result.ir.nodes.as_slice() else {
        panic!("expected one paragraph, got {:?}", result.ir.nodes);
    };
    let Some(IrInline::InlineCollapse {
        full,
        short,
        is_open,
        ..
    }) = content
        .iter()
        .find(|inline| matches!(inline, IrInline::InlineCollapse { .. }))
    else {
        panic!("expected InlineCollapse, got {content:?}");
    };
    (full, short, *is_open)
}

#[test]
fn textcollapse_default_closed_preserves_both_rich_inline_alternatives() {
    let source = "Before .textcollapse {Full **expanded** text} {Short *summary*} after.\n";
    let (result, source_id) = compile_source_with_id(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");

    let (full, short, is_open) = text_collapse(&result);
    assert!(!is_open);
    assert!(full
        .iter()
        .any(|inline| matches!(inline, IrInline::Strong { .. })));
    assert!(short
        .iter()
        .any(|inline| matches!(inline, IrInline::Emphasis { .. })));

    let [IrNode::Paragraph { content, .. }] = result.ir.nodes.as_slice() else {
        unreachable!();
    };
    let span = content
        .iter()
        .find_map(|inline| match inline {
            IrInline::InlineCollapse { span, .. } => Some(*span),
            _ => None,
        })
        .expect("inline collapse span");
    assert_eq!(span.source_id, source_id);
    assert_eq!(
        &source[span.start..span.end],
        ".textcollapse {Full **expanded** text} {Short *summary*}"
    );
}

#[test]
fn textcollapse_named_open_boolean_preserves_initial_state() {
    for (value, expected) in [
        ("yes", true),
        ("true", true),
        ("no", false),
        ("false", false),
    ] {
        let source = format!(
            "Before .textcollapse full:{{Expanded}} short:{{Summary}} open:{{{value}}} after.\n"
        );
        let result = compile_source(&source);
        assert!(result.diagnostics.is_empty(), "{result:?}");
        assert_eq!(text_collapse(&result).2, expected, "{source}");
    }
}

#[test]
fn textcollapse_rejects_incomplete_invalid_and_body_forms_atomically() {
    for source in [
        "Before .textcollapse {Full} after.\n",
        "Before .textcollapse {Full} {Short} open:{maybe} after.\n",
        ".textcollapse {Full} {Short}\n    Body\n",
    ] {
        let result = compile_source(source);
        assert!(!result.diagnostics.is_empty(), "{result:?}");
        assert!(
            result.ir.nodes.iter().all(|node| match node {
                IrNode::Paragraph { content, .. } => content
                    .iter()
                    .all(|inline| !matches!(inline, IrInline::InlineCollapse { .. })),
                _ => true,
            }),
            "{result:?}"
        );
    }
}

#[test]
fn textcollapse_roundtrips_through_document_wire_serde() {
    let result =
        compile_source("Before .textcollapse {Full **rich**} {Short *rich*} open:{yes} after.\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let json = serde_json::to_string(&result.ir).expect("serialize InlineCollapse IR");
    let roundtrip: arkst_core::ir::IrDocument =
        serde_json::from_str(&json).expect("deserialize InlineCollapse IR");
    assert_eq!(roundtrip, result.ir);
}

#[test]
fn source_defined_textcollapse_keeps_precedence() {
    let result =
        compile_source(".function {textcollapse}\n    custom\n\n.textcollapse {Full} {Short}\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert!(!format!("{:?}", result.ir).contains("InlineCollapse"));
}
