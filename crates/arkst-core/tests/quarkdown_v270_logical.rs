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
fn and_or_match_the_v270_truth_table() {
    let result = compile_source(
        ".if {.and {true} {true}}\n    and-tt\n.ifnot {.and {true} {false}}\n    and-tf\n.ifnot {.and {false} {true}}\n    and-ft\n.ifnot {.or {false} {false}}\n    or-ff\n.if {.or {false} {true}}\n    or-ft\n.if {.or {true} {false}}\n    or-tf\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(
        output_text(&result),
        "and-tt\nand-tf\nand-ft\nor-ff\nor-ft\nor-tf"
    );
}

#[test]
fn and_or_support_named_and_chained_binding() {
    let result = compile_source(
        ".if {.and a:{true} b:{true}}\n    named-and\n.if {.or a:{false} b:{true}}\n    named-or\n.var {flag} {true}\n.ifnot {.flag::and {false}}\n    chained-and\n.if {.flag::or {false}}\n    chained-or\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(
        output_text(&result),
        "named-and\nnamed-or\nchained-and\nchained-or"
    );

    let body_result = compile_source(".and {true}\n    false\n");
    assert!(body_result.diagnostics.is_empty(), "{body_result:?}");
}

#[test]
fn and_or_eagerly_evaluate_the_second_candidate() {
    for source in [
        ".if {.and {false} {.error {and-rhs}}}\n    hidden\n",
        ".if {.or {true} {.error {or-rhs}}}\n    hidden\n",
    ] {
        let result = compile_source(source);
        assert_eq!(result.diagnostics.len(), 1, "{result:?}");
        assert!(output_text(&result).is_empty(), "{result:?}");
    }
}

#[test]
fn and_or_reject_invalid_binding_and_conversion_with_provenance() {
    for source in [
        ".and {true}\n",
        ".or {false} {true} {false}\n",
        ".and {true} {false}\n    true\n",
        ".and a:{true} a:{false} b:{true}\n",
        ".or left:{false} b:{true}\n",
    ] {
        let result = compile_source(source);
        assert_eq!(result.diagnostics.len(), 1, "{result:?}");
        assert!(result.ir.nodes.is_empty(), "{result:?}");
    }

    let source = ".and {true} {1}\n";
    let result = compile_source(source);
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    let primary = result.diagnostics[0]
        .primary
        .expect("conversion diagnostic span");
    assert_eq!(&source[primary.start..primary.end], "{1}");
}

#[test]
fn source_defined_and_or_keep_precedence() {
    let result = compile_source(
        ".function {and}\n    a b:\n    custom-and\n.function {or}\n    a b:\n    custom-or\n\n.and {not-a-boolean} {still-custom}\n.or {not-a-boolean} {still-custom}\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(output_text(&result), "custom-and\ncustom-or");
}
