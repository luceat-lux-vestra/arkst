//! Independent regression coverage for the bounded #175 `.font` size-state slice.
//! Font-family resource classification/registration and renderer lowering remain outside this boundary.

use arkst_core::ir::{IrInline, IrNode, IrSizeUnit};
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
fn font_size_calls_append_ordered_typed_layers_without_output() {
    let result = compile_source(".font size:{10pt}\n.font size:{12pt}\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert!(result.ir.nodes.is_empty(), "{:?}", result.ir.nodes);

    let layers = &result.ir.metadata.document_state.font.layers;
    assert_eq!(layers.len(), 2);
    let first = layers[0].size.as_ref().expect("first size");
    assert_eq!((first.value, first.unit), (10.0, IrSizeUnit::Pt));
    let second = layers[1].size.as_ref().expect("second size");
    assert_eq!((second.value, second.unit), (12.0, IrSizeUnit::Pt));
}

#[test]
fn font_none_or_omitted_size_keeps_an_absent_field_in_each_new_layer() {
    let result = compile_source(".font\n.font size:{.none}\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let layers = &result.ir.metadata.document_state.font.layers;
    assert_eq!(layers.len(), 2);
    assert!(layers.iter().all(|layer| layer.size.is_none()));
}

#[test]
fn font_family_arguments_fail_closed_without_publishing_ambiguous_state() {
    for parameter in ["main", "heading", "code"] {
        let source = format!(".font size:{{10pt}}\n.font {parameter}:{{Inter}}\n");
        let result = compile_source(&source);
        assert!(!result.diagnostics.is_empty());
        let layers = &result.ir.metadata.document_state.font.layers;
        assert_eq!(layers.len(), 1, "failed call mutated font state: {parameter}");
    }
}

#[test]
fn failed_size_conversion_rolls_back_nested_font_state_writes() {
    let result = compile_source(
        ".font size:{10pt}\n.function {bad}\n    .font size:{14pt}\n    .string {invalid}\n.font size:{.bad}\n",
    );
    assert!(!result.diagnostics.is_empty());
    let layers = &result.ir.metadata.document_state.font.layers;
    assert_eq!(layers.len(), 1, "nested failed call leaked font state");
}

#[test]
fn source_defined_font_shadows_the_bounded_native_setter() {
    let result = compile_source(".function {font}\n    shadow font\n.font\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(output_text(&result), "shadow font");
    assert!(result.ir.metadata.document_state.font.is_empty());
}

#[test]
fn font_mutation_is_shared_across_callable_scope() {
    let result = compile_source(".function {configure}\n    .font size:{11pt}\n.configure\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(result.ir.metadata.document_state.font.layers.len(), 1);
}

#[test]
fn font_rejects_unknown_arguments_and_block_bodies() {
    for source in [".font unknown:{1}\n", ".font\n    rejected body\n"] {
        let result = compile_source(source);
        assert!(!result.diagnostics.is_empty(), "accepted invalid font call: {source}");
        assert!(result.ir.metadata.document_state.font.is_empty());
    }
}
