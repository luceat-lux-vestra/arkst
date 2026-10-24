use arkst_core::ir::{IrComponent, IrInline, IrNode, IrTextSize};
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
fn container_font_size_preserves_the_complete_non_normal_domain() {
    for (name, expected) in [
        ("tiny", IrTextSize::Tiny),
        ("small", IrTextSize::Small),
        ("medium", IrTextSize::Medium),
        ("large", IrTextSize::Large),
        ("larger", IrTextSize::Larger),
        ("huge", IrTextSize::Huge),
    ] {
        let source = format!(
            ".container width:{{4cm}} foreground:{{purple}} fontsize:{{{name}}}\n    Rich **body**\n"
        );
        let result = compile_source(&source);
        assert!(result.diagnostics.is_empty(), "{name}: {result:?}");
        let component = container(&result);
        assert_eq!(component.font_size, Some(expected), "{name}");
        assert!(component.foreground.is_some(), "{name}");
        assert!(matches!(
            component.children.as_slice(),
            [IrNode::Paragraph { content, .. }]
                if content.iter().any(|inline| matches!(inline, IrInline::Strong { .. }))
        ));
    }
}

#[test]
fn container_normal_and_none_font_size_are_identity() {
    for value in ["normal", ".none"] {
        let source = format!(".container fontsize:{{{value}}}\n    Body\n");
        let result = compile_source(&source);
        assert!(result.diagnostics.is_empty(), "{value}: {result:?}");
        assert_eq!(container(&result).font_size, None, "{value}");
    }
}

#[test]
fn container_font_size_is_named_only_and_fails_atomically_before_body_evaluation() {
    for source in [
        ".container fontsize:{giant}\n    .grid columns:{0}\n        A\n",
        ".container fontsize:{123}\n    .grid columns:{0}\n        A\n",
        ".container fontsize:{.string {large}}\n    .grid columns:{0}\n        A\n",
        ".container fontsize:{small} fontsize:{large}\n    A\n",
        ".container {1cm} {2cm} {yes} {large}\n    A\n",
    ] {
        let result = compile_source(source);
        assert_eq!(result.diagnostics.len(), 1, "{source:?}: {result:?}");
        assert!(result.ir.nodes.is_empty(), "{source:?}: {result:?}");
        assert!(
            !result.diagnostics[0].message.contains("columns"),
            "body executed after invalid font-size binding: {:?}",
            result.diagnostics[0]
        );
    }
}

#[test]
fn container_font_size_diagnostic_retains_argument_provenance() {
    let source = ".container fontsize:{giant}\n    Body\n";
    let result = compile_source(source);
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert!(result.ir.nodes.is_empty(), "{result:?}");
    let diagnostic = &result.diagnostics[0];
    assert!(diagnostic.message.contains("fontsize"), "{diagnostic:?}");
    let primary = diagnostic.primary.expect("font-size diagnostic span");
    assert_eq!(
        source.get(primary.start..primary.end),
        Some("fontsize:{giant}")
    );
}

#[test]
fn container_font_size_roundtrips_without_backend_syntax() {
    let result = compile_source(".container fontsize:{large}\n    Body\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let json = serde_json::to_string(&result.ir).expect("serialize Container IR");
    assert!(json.contains("\"font_size\":\"Large\""), "{json}");
    assert!(!json.contains("typst"), "{json}");
    assert!(!json.contains("1.5em"), "{json}");
    let roundtrip: arkst_core::ir::IrDocument =
        serde_json::from_str(&json).expect("deserialize Container IR");
    assert_eq!(roundtrip, result.ir);
}

#[test]
fn container_font_size_preserves_call_and_child_provenance() {
    let source = ".container fontsize:{larger}\n    Hello\n";
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
fn source_defined_container_font_size_parameter_still_takes_precedence() {
    let result = compile_source(
        ".function {container}\n    fontsize:\n    custom\n\n.container fontsize:{large}\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert!(result.ir.nodes.iter().all(|node| !matches!(
        node,
        IrNode::Component {
            component: IrComponent::Container(_)
        }
    )));
}
