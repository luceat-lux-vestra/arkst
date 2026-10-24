use arkst_core::ir::{IrComponent, IrContainerBorderWidths, IrInline, IrNode, IrSize, IrSizeUnit};
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
fn container_border_width_expands_documented_one_two_and_four_value_forms() {
    for (source_value, expected) in [
        (
            "1cm",
            IrContainerBorderWidths {
                top: size(1.0, IrSizeUnit::Cm),
                right: size(1.0, IrSizeUnit::Cm),
                bottom: size(1.0, IrSizeUnit::Cm),
                left: size(1.0, IrSizeUnit::Cm),
            },
        ),
        (
            "1cm 2mm",
            IrContainerBorderWidths {
                top: size(1.0, IrSizeUnit::Cm),
                right: size(2.0, IrSizeUnit::Mm),
                bottom: size(1.0, IrSizeUnit::Cm),
                left: size(2.0, IrSizeUnit::Mm),
            },
        ),
        (
            "1cm 2mm 3pt 8px",
            IrContainerBorderWidths {
                top: size(1.0, IrSizeUnit::Cm),
                right: size(2.0, IrSizeUnit::Mm),
                bottom: size(3.0, IrSizeUnit::Pt),
                left: size(8.0, IrSizeUnit::Px),
            },
        ),
    ] {
        let source = format!(
            ".container width:{{4cm}} border:{{purple}} borderwidth:{{{source_value}}}\n    Rich **body**\n"
        );
        let result = compile_source(&source);
        assert!(result.diagnostics.is_empty(), "{source_value}: {result:?}");
        let component = container(&result);
        assert_eq!(
            component.border_widths.as_deref(),
            Some(&expected),
            "{source_value}"
        );
        assert!(component.border.is_some(), "{source_value}");
        assert!(matches!(
            component.children.as_slice(),
            [IrNode::Paragraph { content, .. }]
                if content.iter().any(|inline| matches!(inline, IrInline::Strong { .. }))
        ));
    }
}

#[test]
fn container_explicit_none_keeps_border_width_absent() {
    let result = compile_source(".container borderwidth:{.none}\n    Body\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert!(container(&result).border_widths.is_none());
}

#[test]
fn container_border_width_is_named_only_and_fails_atomically_before_body_evaluation() {
    for source in [
        ".container borderwidth:{1cm 2cm 3cm}\n    .grid columns:{0}\n        A\n",
        ".container borderwidth:{1cm nope}\n    .grid columns:{0}\n        A\n",
        ".container borderwidth:{.string {1cm}}\n    .grid columns:{0}\n        A\n",
        ".container borderwidth:{1cm} borderwidth:{2cm}\n    A\n",
        ".container {1cm} {2cm} {yes} {3cm}\n    A\n",
    ] {
        let result = compile_source(source);
        assert_eq!(result.diagnostics.len(), 1, "{source:?}: {result:?}");
        assert!(result.ir.nodes.is_empty(), "{source:?}: {result:?}");
        assert!(
            !result.diagnostics[0].message.contains("columns"),
            "body executed after invalid border-width binding: {:?}",
            result.diagnostics[0]
        );
    }
}

#[test]
fn container_border_width_diagnostic_retains_argument_provenance() {
    let source = ".container borderwidth:{1cm 2cm 3cm}\n    Body\n";
    let result = compile_source(source);
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert!(result.ir.nodes.is_empty(), "{result:?}");
    let diagnostic = &result.diagnostics[0];
    assert!(diagnostic.message.contains("borderwidth"), "{diagnostic:?}");
    let primary = diagnostic.primary.expect("border-width diagnostic span");
    assert_eq!(
        source.get(primary.start..primary.end),
        Some("borderwidth:{1cm 2cm 3cm}")
    );
}

#[test]
fn container_border_width_roundtrips_without_backend_syntax() {
    let result = compile_source(".container borderwidth:{1cm 2mm 3pt 8px}\n    Body\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let json = serde_json::to_string(&result.ir).expect("serialize Container IR");
    assert!(json.contains("\"border_widths\""), "{json}");
    assert!(json.contains("\"top\""), "{json}");
    assert!(!json.contains("typst"), "{json}");
    let roundtrip: arkst_core::ir::IrDocument =
        serde_json::from_str(&json).expect("deserialize Container IR");
    assert_eq!(roundtrip, result.ir);
}

#[test]
fn container_border_width_preserves_call_and_child_provenance() {
    let source = ".container borderwidth:{1cm 2mm}\n    Hello\n";
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
fn source_defined_container_border_width_parameter_still_takes_precedence() {
    let result = compile_source(
        ".function {container}\n    borderwidth:\n    custom\n\n.container borderwidth:{1cm}\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert!(result.ir.nodes.iter().all(|node| !matches!(
        node,
        IrNode::Component {
            component: IrComponent::Container(_)
        }
    )));
}
