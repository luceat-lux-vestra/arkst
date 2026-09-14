use arkst_core::ir::{IrDocumentAlignment, IrDocumentState, IrNode};
use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};

fn compile_source(source: &str) -> arkst_core::CompileResult {
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("entry")
        .add_source("main.qd", source)
        .expect("source")
        .build()
        .expect("project");
    compile(&project, &CompileOptions::default())
}

#[test]
fn alignment_only_pageformat_commits_final_document_state() {
    for (raw, expected) in [
        ("start", IrDocumentAlignment::Start),
        ("center", IrDocumentAlignment::Center),
        ("end", IrDocumentAlignment::End),
        ("justify", IrDocumentAlignment::Justify),
    ] {
        let result = compile_source(&format!(".pageformat alignment:{{{raw}}}\n"));
        assert!(result.diagnostics.is_empty(), "{raw}: {:?}", result.diagnostics);
        assert_eq!(
            result.ir.metadata.document_state.page_alignment,
            Some(expected),
            "{raw}"
        );
        assert!(result.ir.nodes.is_empty(), "setter must not emit content");
    }

    let result = compile_source(
        ".pageformat alignment:{start}\n# Content before the final setter\n.pageformat alignment:{center}\n",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(
        result.ir.metadata.document_state.page_alignment,
        Some(IrDocumentAlignment::Center),
        "the final document-global setter must win even when it occurs after content"
    );
}

#[test]
fn unsupported_pageformat_shapes_remain_structurally_preserved() {
    for source in [
        ".pageformat side:{left} alignment:{end}\n",
        ".pageformat pages:{1..2} alignment:{end}\n",
        ".pageformat columns:{2} alignment:{end}\n",
        ".pageformat alignment:{none}\n",
        ".pageformat\n",
    ] {
        let result = compile_source(&format!(
            ".pageformat alignment:{{center}}\n{source}"
        ));
        assert_eq!(
            result.ir.metadata.document_state.page_alignment,
            Some(IrDocumentAlignment::Center),
            "unsupported partial pageformat must not mutate the bounded global state: {source:?}"
        );
        assert!(
            result.ir.nodes.iter().any(
                |node| matches!(node, IrNode::FunctionCall { name, .. } if name == "pageformat")
            ),
            "unsupported pageformat must remain structurally preserved: {source:?}; IR={:?}",
            result.ir
        );
    }
}

#[test]
fn invalid_alignment_fails_without_replacing_last_committed_state() {
    let result = compile_source(
        ".pageformat alignment:{center}\n.pageformat alignment:{INVALID}\n",
    );
    assert!(
        !result.diagnostics.is_empty(),
        "invalid closed-enum text must fail conversion"
    );
    assert_eq!(
        result.ir.metadata.document_state.page_alignment,
        Some(IrDocumentAlignment::Center),
        "failed conversion must preserve the previous committed alignment"
    );
}

#[test]
fn failed_outer_alignment_conversion_rolls_back_nested_document_state_writes() {
    let result = compile_source(
        ".autopagebreak maxdepth:{3}\n\
.function {badalignment}\n\
    .autopagebreak maxdepth:{1}\n\
    invalid\n\
\n\
.pageformat alignment:{.badalignment}\n",
    );
    assert!(
        !result.diagnostics.is_empty(),
        "the nested result is not a valid document alignment"
    );
    assert_eq!(
        result.ir.metadata.document_state.auto_page_break_max_depth,
        Some(3),
        "outer pageformat failure must roll back nested document-state mutation"
    );
    assert_eq!(
        result.ir.metadata.document_state.page_alignment,
        None,
        "failed pageformat must not publish alignment state"
    );
}

#[test]
fn source_defined_pageformat_shadows_the_bounded_native() {
    let result = compile_source(
        ".function {pageformat}\n\
    alignment:\n\
    SHADOW-PAGEFORMAT\n\
\n\
.pageformat alignment:{center}\n",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.ir.metadata.document_state.page_alignment, None);
    assert!(
        format!("{:?}", result.ir).contains("SHADOW-PAGEFORMAT"),
        "source-defined function must retain ownership of the name"
    );
}

#[test]
fn page_alignment_wire_is_backward_compatible_and_explicit_when_set() {
    let state = IrDocumentState::default();
    let legacy_shape = serde_json::to_value(&state).expect("serialize default state");
    assert!(legacy_shape.get("page_alignment").is_none());
    let restored: IrDocumentState =
        serde_json::from_value(legacy_shape).expect("deserialize legacy-shaped state");
    assert_eq!(restored.page_alignment, None);

    let explicit = IrDocumentState {
        page_alignment: Some(IrDocumentAlignment::Justify),
        ..IrDocumentState::default()
    };
    let value = serde_json::to_value(&explicit).expect("serialize explicit state");
    assert_eq!(
        value.get("page_alignment"),
        Some(&serde_json::json!("Justify"))
    );
    let restored: IrDocumentState =
        serde_json::from_value(value).expect("round trip explicit state");
    assert_eq!(
        restored.page_alignment,
        Some(IrDocumentAlignment::Justify)
    );
}
