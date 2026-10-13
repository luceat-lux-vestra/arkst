use arkst_core::ir::{IrBoxComponent, IrComponent, IrInline, IrNode};
use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};

fn compile_source(source: &str) -> arkst_core::CompileResult {
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("entry")
        .add_source("main.qd", source)
        .expect("source")
        .build()
        .expect("project");
    compile(&project, &CompileOptions::default())
}

fn box_component(result: &arkst_core::CompileResult) -> &IrBoxComponent {
    let [IrNode::Component {
        component: IrComponent::Box(component),
    }] = result.ir.nodes.as_slice()
    else {
        panic!("expected one Box component, got {:?}", result.ir.nodes);
    };
    component
}

#[test]
fn box_default_callout_preserves_structured_markdown_body() {
    let result = compile_source(".box {Hello, **world**!}\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let component = box_component(&result);
    let [IrNode::Paragraph { content, .. }] = component.children.as_slice() else {
        panic!("expected one paragraph child, got {:?}", component.children);
    };
    assert!(matches!(
        content.as_slice(),
        [
            IrInline::Text { content: prefix, .. },
            IrInline::Strong { .. },
            IrInline::Text { content: suffix, .. }
        ] if prefix == "Hello, " && suffix == "!"
    ));
    assert_eq!(component.span.start, 0);
}

#[test]
fn box_indented_body_preserves_block_structure() {
    let result = compile_source(".box\n    ## Callout\n\n    Read *this*.\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let component = box_component(&result);
    assert!(matches!(
        component.children.as_slice(),
        [IrNode::Heading { level: 2, .. }, IrNode::Paragraph { .. }]
    ));
    let IrNode::Paragraph { content, .. } = &component.children[1] else {
        unreachable!();
    };
    assert!(content
        .iter()
        .any(|inline| matches!(inline, IrInline::Emphasis { .. })));
}

#[test]
fn box_fails_closed_for_locale_driven_title_and_non_default_options() {
    let localized = compile_source(".doclang {english}\n.box {A}\n");
    assert_eq!(localized.diagnostics.len(), 1, "{localized:?}");
    assert!(localized.ir.nodes.is_empty(), "{localized:?}");
    assert!(localized.diagnostics[0]
        .message
        .contains("automatic localized titles are not yet implemented"));

    for source in [
        ".box title:{Heads}\n    A\n",
        ".box type:{warning}\n    A\n",
        ".box padding:{1cm}\n    A\n",
        ".box background:{red}\n    A\n",
        ".box foreground:{blue}\n    A\n",
    ] {
        let result = compile_source(source);
        assert!(!result.diagnostics.is_empty(), "{result:?}");
        assert!(result.ir.nodes.is_empty(), "{result:?}");
    }
}

#[test]
fn box_rejects_missing_or_inline_materialization_without_fabricated_output() {
    let missing = compile_source(".box\n");
    assert_eq!(missing.diagnostics.len(), 1, "{missing:?}");
    assert!(missing.ir.nodes.is_empty(), "{missing:?}");

    let inline = compile_source("before .box {A} after\n");
    assert_eq!(inline.diagnostics.len(), 1, "{inline:?}");
    let [IrNode::Paragraph { content, .. }] = inline.ir.nodes.as_slice() else {
        panic!("expected surrounding paragraph, got {:?}", inline.ir.nodes);
    };
    let text: String = content
        .iter()
        .filter_map(|inline| match inline {
            IrInline::Text { content, .. } => Some(content.as_str()),
            _ => None,
        })
        .collect();
    assert!(text.contains("before"));
    assert!(text.contains("after"));
}

#[test]
fn box_component_roundtrips_through_document_wire_serde() {
    let result = compile_source(".box {Ship it}\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let json = serde_json::to_string(&result.ir).expect("serialize Box IR");
    let roundtrip: arkst_core::ir::IrDocument =
        serde_json::from_str(&json).expect("deserialize Box IR");
    assert_eq!(roundtrip, result.ir);
}
