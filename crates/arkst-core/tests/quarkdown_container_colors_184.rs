use arkst_core::ir::{IrColor, IrComponent, IrInline, IrNode};
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
fn container_preserves_typed_background_and_foreground_colors() {
    let source = ".container width:{4cm} background:{purple} foreground:{rgba(1,2,3,0.5)}\n    Rich **body**\n";
    let (result, source_id) = compile_source_with_id(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let component = container(&result);
    assert_eq!(
        component.background.as_deref(),
        Some(&IrColor {
            red: 128,
            green: 0,
            blue: 128,
            alpha: 1.0,
        })
    );
    assert_eq!(
        component.foreground.as_deref(),
        Some(&IrColor {
            red: 1,
            green: 2,
            blue: 3,
            alpha: 0.5,
        })
    );
    assert!(matches!(
        component.children.as_slice(),
        [IrNode::Paragraph { content, .. }]
            if content.iter().any(|inline| matches!(inline, IrInline::Strong { .. }))
    ));
    assert_eq!(component.span.source_id, source_id);
    assert_eq!(
        source.get(component.span.start..component.span.end),
        Some(source.trim_end())
    );
}

#[test]
fn container_explicit_none_keeps_color_overrides_absent() {
    let result = compile_source(".container background:{.none} foreground:{.none}\n    Body\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let component = container(&result);
    assert!(component.background.is_none());
    assert!(component.foreground.is_none());
}

#[test]
fn container_colors_are_named_only_and_fail_atomically_before_body_evaluation() {
    for source in [
        ".container background:{not-a-color}\n    .grid columns:{0}\n        A\n",
        ".container foreground:{.string {red}}\n    .grid columns:{0}\n        A\n",
        ".container background:{red} background:{blue}\n    A\n",
        ".container {1cm} {2cm} {yes} {red}\n    A\n",
    ] {
        let result = compile_source(source);
        assert_eq!(result.diagnostics.len(), 1, "{source:?}: {result:?}");
        assert!(result.ir.nodes.is_empty(), "{source:?}: {result:?}");
        assert!(
            !result.diagnostics[0].message.contains("columns"),
            "body executed after invalid color binding: {:?}",
            result.diagnostics[0]
        );
    }
}

#[test]
fn container_color_diagnostic_retains_argument_provenance() {
    let source = ".container foreground:{not-a-color}\n    Body\n";
    let result = compile_source(source);
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert!(result.ir.nodes.is_empty(), "{result:?}");
    let diagnostic = &result.diagnostics[0];
    assert!(diagnostic.message.contains("foreground"), "{diagnostic:?}");
    let primary = diagnostic.primary.expect("foreground diagnostic span");
    assert_eq!(
        source.get(primary.start..primary.end),
        Some("foreground:{not-a-color}")
    );
}

#[test]
fn container_colors_roundtrip_without_backend_syntax() {
    let result = compile_source(
        ".container background:{blue} foreground:{rgba(250,240,230,0.75)}\n    Body\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let json = serde_json::to_string(&result.ir).expect("serialize Container IR");
    assert!(json.contains("\"background\""), "{json}");
    assert!(json.contains("\"foreground\""), "{json}");
    assert!(!json.contains("typst"), "{json}");
    assert!(!json.contains("#block"), "{json}");
    let roundtrip: arkst_core::ir::IrDocument =
        serde_json::from_str(&json).expect("deserialize Container IR");
    assert_eq!(roundtrip, result.ir);
}

#[test]
fn source_defined_container_still_takes_precedence() {
    let result = compile_source(
        ".function {container}\n    background:\n    custom\n\n.container background:{red}\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert!(result.ir.nodes.iter().all(|node| !matches!(
        node,
        IrNode::Component {
            component: IrComponent::Container(_)
        }
    )));
}
