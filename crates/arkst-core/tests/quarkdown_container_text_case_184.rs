use arkst_core::ir::{
    IrComponent, IrInline, IrNode, IrTextCase, IrTextSize, IrTextStyle, IrTextVariant, IrTextWeight,
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
fn container_text_case_composes_with_font_styling_and_rich_children() {
    let result = compile_source(
        ".container fontsize:{large} fontweight:{bold} fontstyle:{italic} fontvariant:{smallcaps} textcase:{uppercase}\n    Rich **body**\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let component = container(&result);
    assert_eq!(component.font_size, Some(IrTextSize::Large));
    assert_eq!(component.font_weight, Some(IrTextWeight::Bold));
    assert_eq!(component.font_style, Some(IrTextStyle::Italic));
    assert_eq!(component.font_variant, Some(IrTextVariant::SmallCaps));
    assert_eq!(component.text_case, Some(IrTextCase::Uppercase));
    assert!(matches!(
        component.children.as_slice(),
        [IrNode::Paragraph { content, .. }]
            if content.iter().any(|inline| matches!(inline, IrInline::Strong { .. }))
    ));
}

#[test]
fn container_text_case_accepts_the_complete_closed_nonidentity_domain() {
    for (value, expected) in [
        ("uppercase", IrTextCase::Uppercase),
        ("LOWERCASE", IrTextCase::Lowercase),
        ("Capitalize", IrTextCase::Capitalize),
    ] {
        let source = format!(".container textcase:{{{value}}}\n    Body\n");
        let result = compile_source(&source);
        assert!(result.diagnostics.is_empty(), "{value}: {result:?}");
        assert_eq!(container(&result).text_case, Some(expected), "{value}");
    }
}

#[test]
fn container_none_and_explicit_none_text_case_are_identity() {
    for value in ["none", ".none"] {
        let source = format!(".container textcase:{{{value}}}\n    Body\n");
        let result = compile_source(&source);
        assert!(result.diagnostics.is_empty(), "{value}: {result:?}");
        assert_eq!(container(&result).text_case, None, "{value}");
    }
}

#[test]
fn container_text_case_is_named_only_and_fails_before_body_evaluation() {
    for source in [
        ".container textcase:{titlecase}\n    .grid columns:{0}\n        A\n",
        ".container textcase:{123}\n    .grid columns:{0}\n        A\n",
        ".container textcase:{.string {uppercase}}\n    .grid columns:{0}\n        A\n",
        ".container textcase:{none} textcase:{uppercase}\n    A\n",
        ".container {1cm} {2cm} {yes} {uppercase}\n    A\n",
    ] {
        let result = compile_source(source);
        assert_eq!(result.diagnostics.len(), 1, "{source:?}: {result:?}");
        assert!(result.ir.nodes.is_empty(), "{source:?}: {result:?}");
        assert!(
            !result.diagnostics[0].message.contains("columns"),
            "body executed after invalid text-case binding: {:?}",
            result.diagnostics[0]
        );
    }
}

#[test]
fn container_text_case_diagnostic_retains_argument_provenance() {
    let source = ".container textcase:{titlecase}\n    Body\n";
    let result = compile_source(source);
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert!(result.ir.nodes.is_empty(), "{result:?}");
    let diagnostic = &result.diagnostics[0];
    assert!(diagnostic.message.contains("textcase"), "{diagnostic:?}");
    let primary = diagnostic.primary.expect("text-case diagnostic span");
    assert_eq!(
        source.get(primary.start..primary.end),
        Some("textcase:{titlecase}")
    );
}

#[test]
fn container_text_case_roundtrips_without_backend_syntax() {
    let result = compile_source(".container textcase:{capitalize}\n    Body\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let json = serde_json::to_string(&result.ir).expect("serialize Container IR");
    assert!(json.contains("\"text_case\":\"Capitalize\""), "{json}");
    assert!(!json.contains("typst"), "{json}");
    assert!(!json.contains("show regex"), "{json}");
    let roundtrip: arkst_core::ir::IrDocument =
        serde_json::from_str(&json).expect("deserialize Container IR");
    assert_eq!(roundtrip, result.ir);
}

#[test]
fn container_text_case_preserves_call_and_child_provenance() {
    let source = ".container textcase:{lowercase}\n    Hello\n";
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
fn source_defined_container_text_case_parameter_still_takes_precedence() {
    let result = compile_source(
        ".function {container}\n    textcase:\n    custom\n\n.container textcase:{uppercase}\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert!(result.ir.nodes.iter().all(|node| !matches!(
        node,
        IrNode::Component {
            component: IrComponent::Container(_)
        }
    )));
}
