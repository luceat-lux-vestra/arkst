use arkst_core::ir::{
    IrComponent, IrInline, IrNode, IrTextSize, IrTextStyle, IrTextVariant, IrTextWeight,
};
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
    let source_id = project
        .sources()
        .get_id(project.entry())
        .expect("source id");
    (compile(&project, &CompileOptions::default()), source_id)
}

fn container(result: &arkst_core::CompileResult) -> &arkst_core::ir::IrContainerComponent {
    let [IrNode::Component {
        component: IrComponent::Container(component),
    }] = result.ir.nodes.as_slice()
    else {
        panic!(
            "expected one Container component, got {:?}",
            result.ir.nodes
        );
    };
    component
}

#[test]
fn container_small_caps_variant_composes_with_font_styling_and_rich_children() {
    let result = compile_source(
        ".container fontsize:{large} fontweight:{bold} fontstyle:{italic} fontvariant:{smallcaps}\n    Rich **body**\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let component = container(&result);
    assert_eq!(component.font_size, Some(IrTextSize::Large));
    assert_eq!(component.font_weight, Some(IrTextWeight::Bold));
    assert_eq!(component.font_style, Some(IrTextStyle::Italic));
    assert_eq!(component.font_variant, Some(IrTextVariant::SmallCaps));
    assert!(matches!(
        component.children.as_slice(),
        [IrNode::Paragraph { content, .. }]
            if content.iter().any(|inline| matches!(inline, IrInline::Strong { .. }))
    ));
}

#[test]
fn container_normal_and_none_font_variant_are_identity() {
    for value in ["normal", ".none"] {
        let source = format!(".container fontvariant:{{{value}}}\n    Body\n");
        let result = compile_source(&source);
        assert!(result.diagnostics.is_empty(), "{value}: {result:?}");
        assert_eq!(container(&result).font_variant, None, "{value}");
    }
}

#[test]
fn container_font_variant_is_named_only_and_fails_before_body_evaluation() {
    for source in [
        ".container fontvariant:{petitecaps}\n    .grid columns:{0}\n        A\n",
        ".container fontvariant:{123}\n    .grid columns:{0}\n        A\n",
        ".container fontvariant:{.string {smallcaps}}\n    .grid columns:{0}\n        A\n",
        ".container fontvariant:{normal} fontvariant:{smallcaps}\n    A\n",
        ".container {1cm} {2cm} {yes} {smallcaps}\n    A\n",
    ] {
        let result = compile_source(source);
        assert_eq!(result.diagnostics.len(), 1, "{source:?}: {result:?}");
        assert!(result.ir.nodes.is_empty(), "{source:?}: {result:?}");
        assert!(
            !result.diagnostics[0].message.contains("columns"),
            "body executed after invalid font-variant binding: {:?}",
            result.diagnostics[0]
        );
    }
}

#[test]
fn container_font_variant_diagnostic_retains_argument_provenance() {
    let source = ".container fontvariant:{petitecaps}\n    Body\n";
    let result = compile_source(source);
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert!(result.ir.nodes.is_empty(), "{result:?}");
    let diagnostic = &result.diagnostics[0];
    assert!(diagnostic.message.contains("fontvariant"), "{diagnostic:?}");
    let primary = diagnostic.primary.expect("font-variant diagnostic span");
    assert_eq!(
        source.get(primary.start..primary.end),
        Some("fontvariant:{petitecaps}")
    );
}

#[test]
fn container_font_variant_roundtrips_without_backend_syntax() {
    let result = compile_source(".container fontvariant:{smallcaps}\n    Body\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let json = serde_json::to_string(&result.ir).expect("serialize Container IR");
    assert!(json.contains("\"font_variant\":\"SmallCaps\""), "{json}");
    assert!(!json.contains("typst"), "{json}");
    assert!(!json.contains("smallcaps["), "{json}");
    let roundtrip: arkst_core::ir::IrDocument =
        serde_json::from_str(&json).expect("deserialize Container IR");
    assert_eq!(roundtrip, result.ir);
}

#[test]
fn container_font_variant_preserves_call_and_child_provenance() {
    let source = ".container fontvariant:{smallcaps}\n    Hello\n";
    let (result, source_id) = compile_source_with_id(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let component = container(&result);
    assert_eq!(component.span.source_id, source_id);
    assert_eq!(
        source.get(component.span.start..component.span.end),
        Some(source.trim_end())
    );
    let [IrNode::Paragraph { span, .. }] = component.children.as_slice() else {
        panic!("expected paragraph child, got {:?}", component.children);
    };
    assert_eq!(source.get(span.start..span.end), Some("Hello"));
}

#[test]
fn source_defined_container_font_variant_parameter_still_takes_precedence() {
    let result = compile_source(
        ".function {container}\n    fontvariant:\n    custom\n\n.container fontvariant:{smallcaps}\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert!(result.ir.nodes.iter().all(|node| !matches!(
        node,
        IrNode::Component {
            component: IrComponent::Container(_)
        }
    )));
}
