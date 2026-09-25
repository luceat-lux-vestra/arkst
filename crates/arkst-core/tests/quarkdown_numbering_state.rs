//! Independent source-level witnesses for the bounded #175 numbering-state slice.

use arkst_core::ir::{IrDocumentType, IrInline, IrNode, IrNumberingToken};
use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};

fn compile_source(source: &str) -> arkst_core::CompileResult {
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("valid entry")
        .add_source("main.qd", source)
        .expect("valid source")
        .build()
        .expect("valid project");
    compile(&project, &CompileOptions::default())
}

fn output_text(result: &arkst_core::CompileResult) -> String {
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
fn numbering_binds_source_backed_body_dictionary() {
    let source = r#".numbering
    - headings: 1.
    - figures: A
    - custom: i
"#;
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert!(result.ir.nodes.is_empty(), "{:?}", result.ir.nodes);

    let state = &result.ir.metadata.document_state.numbering;
    assert_eq!(state.layers.len(), 1);
    let layer = &state.layers[0];
    assert!(layer.merge);
    assert_eq!(layer.document_type, IrDocumentType::Plain);
    assert_eq!(
        layer.headings.as_ref().unwrap().tokens,
        vec![
            IrNumberingToken::Decimal,
            IrNumberingToken::Literal(".".to_string())
        ]
    );
    assert_eq!(
        layer
            .extra
            .iter()
            .map(|(key, _)| key.as_str())
            .collect::<Vec<_>>(),
        vec!["headings", "figures", "custom"]
    );
}

#[test]
fn numbering_false_and_nonumbering_replace_prior_state() {
    let replacement = compile_source(
        r#".numbering
    - headings: 1
.numbering merge:{false}
    - figures: A
"#,
    );
    assert!(replacement.diagnostics.is_empty(), "{replacement:?}");
    let layers = &replacement.ir.metadata.document_state.numbering.layers;
    assert_eq!(layers.len(), 1);
    assert!(!layers[0].merge);
    assert!(layers[0].headings.is_none());
    assert!(layers[0].figures.is_some());

    let disabled = compile_source(
        r#".numbering
    - headings: 1
.nonumbering
"#,
    );
    assert!(disabled.diagnostics.is_empty(), "{disabled:?}");
    let layers = &disabled.ir.metadata.document_state.numbering.layers;
    assert_eq!(layers.len(), 1);
    assert!(!layers[0].merge);
    assert!(layers[0].extra.is_empty());
}

#[test]
fn failed_numbering_candidate_is_atomic() {
    let result = compile_source(
        r#".numbering
    - headings: 1
.numbering
    - figures:
        - nested: invalid
"#,
    );
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    let layers = &result.ir.metadata.document_state.numbering.layers;
    assert_eq!(layers.len(), 1);
    assert!(layers[0].headings.is_some());
    assert!(layers[0].figures.is_none());
}

#[test]
fn source_defined_numbering_names_shadow_native_dispatch() {
    let result = compile_source(
        r#".function {numbering}
    shadow numbering
.function {nonumbering}
    shadow nonumbering
.numbering
.nonumbering
"#,
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(output_text(&result), "shadow numbering\nshadow nonumbering");
    assert!(result
        .ir
        .metadata
        .document_state
        .numbering
        .layers
        .is_empty());
}

#[test]
fn numbering_rejects_missing_formats_unknown_arguments_and_bodies_on_nonumbering() {
    for source in [
        ".numbering\n",
        ".numbering unknown:{true}\n    - headings: 1\n",
        ".nonumbering value:{true}\n",
        ".nonumbering\n    rejected body\n",
    ] {
        let result = compile_source(source);
        assert!(
            !result.diagnostics.is_empty(),
            "accepted invalid numbering call: {source}"
        );
        assert!(result
            .ir
            .metadata
            .document_state
            .numbering
            .layers
            .is_empty());
    }
}

#[test]
fn numbering_freezes_document_type_at_commit_time() {
    let result = compile_source(
        r#".doctype {paged}
.numbering
    - headings: 1
.doctype {slides}
"#,
    );

    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(
        result.ir.metadata.document_state.document_type,
        IrDocumentType::Slides
    );
    let layers = &result.ir.metadata.document_state.numbering.layers;
    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].document_type, IrDocumentType::Paged);
}
