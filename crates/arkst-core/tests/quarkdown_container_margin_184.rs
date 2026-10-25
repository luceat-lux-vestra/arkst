use arkst_core::ir::{IrComponent, IrContainerMargin, IrInline, IrNode, IrSize, IrSizeUnit};
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

fn size(value: f64, unit: IrSizeUnit) -> IrSize {
    IrSize { value, unit }
}

#[test]
fn container_margin_expands_documented_one_two_and_four_value_forms() {
    for (source_value, expected) in [
        (
            "1cm",
            IrContainerMargin {
                top: size(1.0, IrSizeUnit::Cm),
                right: size(1.0, IrSizeUnit::Cm),
                bottom: size(1.0, IrSizeUnit::Cm),
                left: size(1.0, IrSizeUnit::Cm),
            },
        ),
        (
            "1cm 2mm",
            IrContainerMargin {
                top: size(1.0, IrSizeUnit::Cm),
                right: size(2.0, IrSizeUnit::Mm),
                bottom: size(1.0, IrSizeUnit::Cm),
                left: size(2.0, IrSizeUnit::Mm),
            },
        ),
        (
            "1cm 2mm 3pt 8px",
            IrContainerMargin {
                top: size(1.0, IrSizeUnit::Cm),
                right: size(2.0, IrSizeUnit::Mm),
                bottom: size(3.0, IrSizeUnit::Pt),
                left: size(8.0, IrSizeUnit::Px),
            },
        ),
    ] {
        let source = format!(
            ".container width:{{4cm}} margin:{{{source_value}}} padding:{{2mm}}\n    Rich **body**\n"
        );
        let result = compile_source(&source);
        assert!(result.diagnostics.is_empty(), "{source_value}: {result:?}");
        let component = container(&result);
        assert_eq!(
            component.margin.as_deref(),
            Some(&expected),
            "{source_value}"
        );
        assert!(component.padding.is_some(), "{source_value}");
        assert!(matches!(
            component.children.as_slice(),
            [IrNode::Paragraph { content, .. }]
                if content.iter().any(|inline| matches!(inline, IrInline::Strong { .. }))
        ));
    }
}

#[test]
fn container_explicit_none_keeps_margin_absent() {
    let result = compile_source(".container margin:{.none}\n    Body\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert!(container(&result).margin.is_none());
}

#[test]
fn container_margin_is_named_only_and_fails_atomically_before_body_evaluation() {
    for source in [
        ".container margin:{1cm 2cm 3cm}\n    .grid columns:{0}\n        A\n",
        ".container margin:{1cm nope}\n    .grid columns:{0}\n        A\n",
        ".container margin:{.string {1cm}}\n    .grid columns:{0}\n        A\n",
        ".container margin:{1cm} margin:{2cm}\n    A\n",
        ".container {1cm} {2cm} {yes} {3cm}\n    A\n",
    ] {
        let result = compile_source(source);
        assert_eq!(result.diagnostics.len(), 1, "{source:?}: {result:?}");
        assert!(result.ir.nodes.is_empty(), "{source:?}: {result:?}");
        assert!(
            !result.diagnostics[0].message.contains("columns"),
            "body executed after invalid margin binding: {:?}",
            result.diagnostics[0]
        );
    }
}

#[test]
fn container_margin_diagnostic_retains_argument_provenance() {
    let source = ".container margin:{1cm 2cm 3cm}\n    Body\n";
    let result = compile_source(source);
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert!(result.ir.nodes.is_empty(), "{result:?}");
    let diagnostic = &result.diagnostics[0];
    assert!(diagnostic.message.contains("margin"), "{diagnostic:?}");
    let primary = diagnostic.primary.expect("margin diagnostic span");
    assert_eq!(
        source.get(primary.start..primary.end),
        Some("margin:{1cm 2cm 3cm}")
    );
}

#[test]
fn container_margin_roundtrips_without_backend_syntax() {
    let result = compile_source(".container margin:{1cm 2mm 3pt 8px}\n    Body\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let json = serde_json::to_string(&result.ir).expect("serialize Container IR");
    assert!(json.contains("\"margin\""), "{json}");
    assert!(!json.contains("typst"), "{json}");
    assert!(!json.contains("pad("), "{json}");
    let roundtrip: arkst_core::ir::IrDocument =
        serde_json::from_str(&json).expect("deserialize Container IR");
    assert_eq!(roundtrip, result.ir);
}

#[test]
fn container_margin_preserves_call_and_child_provenance() {
    let source = ".container margin:{1cm 2mm}\n    Hello\n";
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
fn source_defined_container_margin_parameter_still_takes_precedence() {
    let result = compile_source(
        ".function {container}\n    margin:\n    custom\n\n.container margin:{1cm}\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert!(result.ir.nodes.iter().all(|node| !matches!(
        node,
        IrNode::Component {
            component: IrComponent::Container(_)
        }
    )));
}
