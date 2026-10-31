use arkst_core::ir::{IrColor, IrInline, IrNode};
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

fn paragraph_text(content: &[IrInline]) -> String {
    content
        .iter()
        .filter_map(|inline| match inline {
            IrInline::Text { content, .. } => Some(content.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn nested_indented_bodies_bind_scalar_direct_and_chained_typed_calls() {
    let result = compile_source(
        ".text {.uppercase\n    hello}\n\
         .text {direct-color} color:{.color {red}\n    0.5}\n\
         .var {base} {blue}\n\
         .text {chained-color} color:{.base::color\n    0.25}\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");

    let paragraphs: Vec<_> = result
        .ir
        .nodes
        .iter()
        .filter_map(|node| match node {
            IrNode::Paragraph { content, .. } => Some(content.as_slice()),
            _ => None,
        })
        .collect();
    assert_eq!(paragraph_text(paragraphs[0]), "HELLO");
    assert!(matches!(
        paragraphs[1],
        [IrInline::TextColor {
            color: IrColor { red: 255, green: 0, blue: 0, alpha },
            content,
            ..
        }] if *alpha == 0.5 && paragraph_text(content) == "direct-color"
    ));
    assert!(matches!(
        paragraphs[2],
        [IrInline::TextColor {
            color: IrColor { red: 0, green: 0, blue: 255, alpha },
            content,
            ..
        }] if *alpha == 0.25 && paragraph_text(content) == "chained-color"
    ));
}

#[test]
fn source_defined_function_keeps_precedence_with_a_nested_indented_body() {
    let result = compile_source(
        r#".function {echo}
    value:
    .value
.text {.echo
    nested}
"#,
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert!(matches!(
        result.ir.nodes.as_slice(),
        [IrNode::Paragraph { content, .. }] if paragraph_text(content) == "nested"
    ));
}
