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
fn container_preserves_typed_border_color_with_existing_presentation() {
    let source = ".container width:{4cm} background:{white} foreground:{black} border:{rgba(1,2,3,0.5)}\n    Rich **body**\n";
    let (result, source_id) = compile_source_with_id(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let component = container(&result);
    assert_eq!(
        component.border.as_deref(),
        Some(&IrColor {
            red: 1,
            green: 2,
            blue: 3,
            alpha: 0.5,
        })
    );
    assert!(component.background.is_some());
    assert!(component.foreground.is_some());
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
fn container_explicit_none_keeps_border_absent() {
    let result = compile_source(".container border:{.none}\n    Body\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert!(container(&result).border.is_none());
}

#[test]
fn container_border_is_named_only_and_fails_atomically_before_body_evaluation() {
    for source in [
        ".container border:{not-a-color}\n    .grid columns:{0}\n        A\n",
        ".container border:{.string {red}}\n    .grid columns:{0}\n        A\n",
        ".container border:{red} border:{blue}\n    A\n",
        ".container {1cm} {2cm} {yes} {red}\n    A\n",
    ] {
        let result = compile_source(source);
        assert_eq!(result.diagnostics.len(), 1, "{source:?}: {result:?}");
        assert!(result.ir.nodes.is_empty(), "{source:?}: {result:?}");
        assert!(
            !result.diagnostics[0].message.contains("columns"),
            "body executed after invalid border binding: {:?}",
            result.diagnostics[0]
        );
    }
}

#[test]
fn container_border_diagnostic_retains_argument_provenance() {
    let source = ".container border:{not-a-color}\n    Body\n";
    let result = compile_source(source);
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert!(result.ir.nodes.is_empty(), "{result:?}");
    let diagnostic = &result.diagnostics[0];
    assert!(diagnostic.message.contains("border"), "{diagnostic:?}");
    let primary = diagnostic.primary.expect("border diagnostic span");
    assert_eq!(
        source.get(primary.start..primary.end),
        Some("border:{not-a-color}")
    );
}

#[test]
fn container_border_roundtrips_without_backend_syntax() {
    let result = compile_source(".container border:{rgba(250,240,230,0.75)}\n    Body\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let json = serde_json::to_string(&result.ir).expect("serialize Container IR");
    assert!(json.contains("\"border\""), "{json}");
    assert!(!json.contains("typst"), "{json}");
    assert!(!json.contains("stroke"), "{json}");
    let roundtrip: arkst_core::ir::IrDocument =
        serde_json::from_str(&json).expect("deserialize Container IR");
    assert_eq!(roundtrip, result.ir);
}

#[test]
fn source_defined_container_border_parameter_still_takes_precedence() {
    let result = compile_source(
        ".function {container}\n    border:\n    custom\n\n.container border:{red}\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert!(result.ir.nodes.iter().all(|node| !matches!(
        node,
        IrNode::Component {
            component: IrComponent::Container(_)
        }
    )));
}
