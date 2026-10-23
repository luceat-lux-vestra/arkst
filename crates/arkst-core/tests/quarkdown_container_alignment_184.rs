use arkst_core::ir::{IrComponent, IrContainerAlignment, IrInline, IrNode};
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
fn container_preserves_the_closed_alignment_domain_with_existing_presentation() {
    for (name, expected) in [
        ("start", IrContainerAlignment::Start),
        ("center", IrContainerAlignment::Center),
        ("end", IrContainerAlignment::End),
    ] {
        let source = format!(
            ".container width:{{4cm}} background:{{white}} alignment:{{{name}}}\n    Rich **body**\n"
        );
        let result = compile_source(&source);
        assert!(result.diagnostics.is_empty(), "{name}: {result:?}");
        let component = container(&result);
        assert_eq!(component.alignment, Some(expected), "{name}");
        assert!(component.background.is_some(), "{name}");
        assert!(matches!(
            component.children.as_slice(),
            [IrNode::Paragraph { content, .. }]
                if content.iter().any(|inline| matches!(inline, IrInline::Strong { .. }))
        ));
    }
}

#[test]
fn container_explicit_none_keeps_alignment_absent() {
    let result = compile_source(".container alignment:{.none}\n    Body\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(container(&result).alignment, None);
}

#[test]
fn container_alignment_is_named_only_and_fails_atomically_before_body_evaluation() {
    for source in [
        ".container alignment:{sideways}\n    .grid columns:{0}\n        A\n",
        ".container alignment:{.string {center}}\n    .grid columns:{0}\n        A\n",
        ".container alignment:{start} alignment:{end}\n    A\n",
        ".container {1cm} {2cm} {yes} {center}\n    A\n",
    ] {
        let result = compile_source(source);
        assert_eq!(result.diagnostics.len(), 1, "{source:?}: {result:?}");
        assert!(result.ir.nodes.is_empty(), "{source:?}: {result:?}");
        assert!(
            !result.diagnostics[0].message.contains("columns"),
            "body executed after invalid alignment binding: {:?}",
            result.diagnostics[0]
        );
    }
}

#[test]
fn container_alignment_diagnostic_retains_argument_provenance() {
    let source = ".container alignment:{sideways}\n    Body\n";
    let result = compile_source(source);
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert!(result.ir.nodes.is_empty(), "{result:?}");
    let diagnostic = &result.diagnostics[0];
    assert!(diagnostic.message.contains("alignment"), "{diagnostic:?}");
    let primary = diagnostic.primary.expect("alignment diagnostic span");
    assert_eq!(
        source.get(primary.start..primary.end),
        Some("alignment:{sideways}")
    );
}

#[test]
fn container_alignment_roundtrips_without_backend_syntax() {
    let result = compile_source(".container alignment:{center}\n    Body\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let json = serde_json::to_string(&result.ir).expect("serialize Container IR");
    assert!(json.contains("\"alignment\":\"Center\""), "{json}");
    assert!(!json.contains("typst"), "{json}");
    assert!(!json.contains("#align"), "{json}");
    let roundtrip: arkst_core::ir::IrDocument =
        serde_json::from_str(&json).expect("deserialize Container IR");
    assert_eq!(roundtrip, result.ir);
}

#[test]
fn container_alignment_preserves_call_and_child_provenance() {
    let source = ".container alignment:{end}\n    Hello\n";
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
fn source_defined_container_alignment_parameter_still_takes_precedence() {
    let result = compile_source(
        ".function {container}\n    alignment:\n    custom\n\n.container alignment:{center}\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert!(result.ir.nodes.iter().all(|node| !matches!(
        node,
        IrNode::Component {
            component: IrComponent::Container(_)
        }
    )));
}
