use arkst_core::ir::{IrInline, IrNode};
use arkst_core::{compile, CompileOptions, SourceId, VirtualProjectBuilder};

fn compile_source(source: &str) -> (arkst_core::CompileResult, SourceId) {
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("entry")
        .add_source("main.qd", source)
        .expect("source")
        .build()
        .expect("project");
    let source_id = project
        .sources()
        .get_id(project.entry())
        .expect("source id");
    (compile(&project, &CompileOptions::default()), source_id)
}

fn single_code(source: &str) -> (IrInline, SourceId) {
    let (result, source_id) = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let [IrNode::Paragraph { content, .. }] = result.ir.nodes.as_slice() else {
        panic!("one paragraph expected: {:?}", result.ir.nodes);
    };
    let [inline] = content.as_slice() else {
        panic!("one inline expected: {content:?}");
    };
    assert!(matches!(inline, IrInline::Code { .. }), "{inline:?}");
    (inline.clone(), source_id)
}

#[test]
fn codespan_uses_existing_inline_code_with_call_provenance() {
    let source = ".codespan {a\\b}\n";
    let (code, id) = single_code(source);
    let IrInline::Code { content, span } = code else {
        unreachable!()
    };
    assert_eq!(content, "a\\b");
    assert_eq!(span.source_id, id);
    assert_eq!(span.start, 0);
    assert_eq!(span.end, source.trim_end().len());
}

#[test]
fn named_argument_uses_same_semantic_node() {
    let (code, _) = single_code(".codespan text:{alpha}\n");
    assert!(matches!(code, IrInline::Code { content, .. } if content == "alpha"));
}

#[test]
fn codespan_preserves_surrounding_inline_text() {
    let (result, _) = compile_source("before .codespan {alpha} after\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let [IrNode::Paragraph { content, .. }] = result.ir.nodes.as_slice() else {
        panic!("expected paragraph: {:?}", result.ir.nodes);
    };
    assert!(matches!(&content[0], IrInline::Text { content, .. } if content == "before "));
    assert!(content
        .iter()
        .any(|node| matches!(node, IrInline::Code { content, .. } if content == "alpha")));
    assert!(matches!(content.last(), Some(IrInline::Text { content, .. }) if content == " after"));
}

#[test]
fn codespan_invalid_calls_remain_fail_closed_and_source_backed() {
    for source in [
        ".codespan\n",
        ".codespan {one} {two}\n",
        ".codespan wrong:{one}\n",
        ".codespan {one} text:{two}\n",
        ".codespan\n    nested body\n",
    ] {
        let (result, _) = compile_source(source);
        assert!(!result.diagnostics.is_empty(), "{source:?}: {result:?}");
        assert!(
            result.diagnostics[0].primary.is_some(),
            "{source:?}: {result:?}"
        );
        assert!(
            result.ir.nodes.iter().all(|node| {
                !matches!(node, IrNode::Paragraph { content, .. }
                    if content.iter().any(|inline| matches!(inline, IrInline::Code { .. })))
            }),
            "{source:?}: {result:?}"
        );
    }
}

#[test]
fn user_defined_codespan_keeps_source_precedence() {
    let (result, _) = compile_source(".function {codespan}\n    custom\n\n.codespan {not-code}\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert!(
        result.ir.nodes.iter().all(|node| {
            !matches!(node, IrNode::Paragraph { content, .. }
                if content.iter().any(|inline| matches!(inline, IrInline::Code { .. })))
        }),
        "{result:?}"
    );
}

#[test]
fn existing_code_serde_is_reused() {
    let (code, _) = single_code(".codespan {plain}\n");
    let json = serde_json::to_string(&code).expect("serialize");
    let decoded: IrInline = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(decoded, code);
}
