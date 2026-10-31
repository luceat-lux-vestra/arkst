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

fn text_colors(result: &arkst_core::CompileResult) -> Vec<(String, IrColor)> {
    result
        .ir
        .nodes
        .iter()
        .filter_map(|node| match node {
            IrNode::Paragraph { content, .. } => match content.as_slice() {
                [IrInline::TextColor { color, content, .. }] => match content.as_slice() {
                    [IrInline::Text { content, .. }] => Some((content.clone(), color.clone())),
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        })
        .collect()
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
fn color_builds_defaults_components_and_derived_typed_values() {
    let result = compile_source(
        r#".text {default} color:{.color}
.text {components} color:{.color red:{66} green:{144} blue:{245}}
.text {lower-bound} color:{.color red:{0} green:{0} blue:{0} alpha:{0}}
.text {upper-bound} color:{.color red:{255} green:{255} blue:{255} alpha:{1}}
.text {derived} color:{.color {#4290F5} alpha:{0.5}}
.text {named-base} color:{.color base:{rgba(10, 20, 30, 0.75)} green:{99}}
"#,
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(
        text_colors(&result),
        [
            (
                "default".to_string(),
                IrColor {
                    red: 0,
                    green: 0,
                    blue: 0,
                    alpha: 1.0,
                },
            ),
            (
                "components".to_string(),
                IrColor {
                    red: 66,
                    green: 144,
                    blue: 245,
                    alpha: 1.0,
                },
            ),
            (
                "lower-bound".to_string(),
                IrColor {
                    red: 0,
                    green: 0,
                    blue: 0,
                    alpha: 0.0,
                },
            ),
            (
                "upper-bound".to_string(),
                IrColor {
                    red: 255,
                    green: 255,
                    blue: 255,
                    alpha: 1.0,
                },
            ),
            (
                "derived".to_string(),
                IrColor {
                    red: 66,
                    green: 144,
                    blue: 245,
                    alpha: 0.5,
                },
            ),
            (
                "named-base".to_string(),
                IrColor {
                    red: 10,
                    green: 99,
                    blue: 30,
                    alpha: 0.75,
                },
            ),
        ]
    );
}

#[test]
fn color_matches_positional_chained_mixed_and_final_body_binding() {
    let result = compile_source(
        r#".text {all-positional} color:{.color {#010203} {10} {20} {30} {0.4}}
.var {shade}
    .color {red}
        0.5
.text {final-body-alpha} color:{.shade}
.var {brand} {#4290F5}
.text {chain-positional} color:{.brand::color {7} {8} {9} {0.6}}
.text {mixed} color:{.color {#010203} blue:{30} red:{10} green:{20} alpha:{0.4}}
"#,
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let alpha_04 = f64::from(0.4_f32);
    let alpha_06 = f64::from(0.6_f32);
    assert_eq!(
        text_colors(&result),
        [
            (
                "all-positional".to_string(),
                IrColor {
                    red: 10,
                    green: 20,
                    blue: 30,
                    alpha: alpha_04,
                },
            ),
            (
                "final-body-alpha".to_string(),
                IrColor {
                    red: 255,
                    green: 0,
                    blue: 0,
                    alpha: 0.5,
                },
            ),
            (
                "chain-positional".to_string(),
                IrColor {
                    red: 7,
                    green: 8,
                    blue: 9,
                    alpha: alpha_06,
                },
            ),
            (
                "mixed".to_string(),
                IrColor {
                    red: 10,
                    green: 20,
                    blue: 30,
                    alpha: alpha_04,
                },
            ),
        ]
    );
}

#[test]
fn color_rejects_invalid_binding_conversion_and_ranges_with_provenance() {
    for source in [
        ".color red:{-1}\n",
        ".color green:{256}\n",
        ".color blue:{1.5}\n",
        ".color alpha:{-0.01}\n",
        ".color alpha:{1.01}\n",
        ".color base:{not-a-color}\n",
        ".color base:{.none}\n",
        ".color red:{1} red:{2}\n",
        ".color {red} {1} {2} {3} {0.5} {extra}\n",
        ".color unknown:{1}\n",
    ] {
        let result = compile_source(source);
        assert_eq!(result.diagnostics.len(), 1, "{source}: {result:?}");
        let primary = result.diagnostics[0]
            .primary
            .expect("color failure should retain source provenance");
        assert!(primary.end > primary.start, "{source}: {result:?}");
        assert!(result.ir.nodes.is_empty(), "{source}: {result:?}");
    }
}

#[test]
fn failed_color_conversion_rolls_back_nested_state_mutation() {
    let result = compile_source(
        r#".autopagebreak maxdepth:{3}
.function {mutate}
    .autopagebreak maxdepth:{1}
    10
.color red:{.mutate} green:{256}
"#,
    );
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert_eq!(
        result.ir.metadata.document_state.auto_page_break_max_depth,
        Some(3),
        "failed color construction must roll back nested document-state writes"
    );
    assert!(result.ir.nodes.is_empty(), "{result:?}");
}

#[test]
fn source_defined_color_keeps_direct_and_chained_precedence() {
    let result = compile_source(
        r#".function {color}
    base? red? green? blue? alpha?:
    custom-color
direct: .color {not-a-color}
.var {base} {still-not-a-color}
chained: .base::color
"#,
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(
        output_text(&result),
        "direct: custom-color\nchained: custom-color"
    );
}
