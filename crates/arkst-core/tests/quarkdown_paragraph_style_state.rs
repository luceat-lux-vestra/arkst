use arkst_core::ir::{IrInline, IrNode};
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
fn paragraphstyle_merges_partial_numeric_state_without_output() {
    let result = compile_source(
        ".paragraphstyle lineheight:{1.4} letterspacing:{0.02}\n.paragraphstyle spacing:{1.2}\n.paragraphstyle lineheight:{none} indent:{2}\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert!(result.ir.nodes.is_empty(), "{:?}", result.ir.nodes);

    let style = result.ir.metadata.document_state.paragraph_style;
    assert_eq!(style.line_height, Some(1.4));
    assert_eq!(style.letter_spacing, Some(0.02));
    assert_eq!(style.spacing, Some(1.2));
    assert_eq!(style.indent, Some(2.0));
}

#[test]
fn paragraphstyle_positional_binding_and_none_preserve_existing_fields() {
    let result = compile_source(
        ".paragraphstyle {1.25} {0.1} {1.5} {2}\n.paragraphstyle lineheight:{none} spacing:{3}\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");

    let style = result.ir.metadata.document_state.paragraph_style;
    assert_eq!(style.line_height, Some(1.25));
    assert_eq!(style.letter_spacing, Some(0.1));
    assert_eq!(style.spacing, Some(3.0));
    assert_eq!(style.indent, Some(2.0));
}

#[test]
fn failed_conversion_rolls_back_nested_paragraphstyle_mutation() {
    let result = compile_source(
        ".paragraphstyle lineheight:{1} indent:{2}\n.function {bad}\n    .paragraphstyle indent:{9}\n    .string {invalid}\n.paragraphstyle spacing:{.bad}\n",
    );
    assert!(
        !result.diagnostics.is_empty(),
        "invalid numeric conversion must fail"
    );

    let style = result.ir.metadata.document_state.paragraph_style;
    assert_eq!(style.line_height, Some(1.0));
    assert_eq!(style.indent, Some(2.0));
    assert_eq!(style.spacing, None);
}

#[test]
fn source_defined_paragraphstyle_shadows_native_setter() {
    let result = compile_source(
        ".function {paragraphstyle}\n    shadow paragraph style\n.paragraphstyle\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(output_text(&result), "shadow paragraph style");
    assert!(result.ir.metadata.document_state.paragraph_style.is_empty());
}

#[test]
fn paragraphstyle_mutation_is_shared_across_callable_scope() {
    let result = compile_source(
        ".function {configure}\n    .paragraphstyle lineheight:{1.75} indent:{2}\n.configure\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");

    let style = result.ir.metadata.document_state.paragraph_style;
    assert_eq!(style.line_height, Some(1.75));
    assert_eq!(style.indent, Some(2.0));
}

#[test]
fn paragraphstyle_rejects_unknown_arguments_and_block_bodies() {
    for source in [
        ".paragraphstyle unknown:{1}\n",
        ".paragraphstyle\n    rejected body\n",
    ] {
        let result = compile_source(source);
        assert!(
            !result.diagnostics.is_empty(),
            "accepted invalid paragraphstyle call: {source}"
        );
        assert!(result.ir.metadata.document_state.paragraph_style.is_empty());
    }
}
