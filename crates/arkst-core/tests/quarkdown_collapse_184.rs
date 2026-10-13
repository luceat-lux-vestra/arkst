use arkst_core::ir::{IrCollapseComponent, IrComponent, IrInline, IrNode};
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

fn collapse(result: &arkst_core::CompileResult) -> &IrCollapseComponent {
    let [IrNode::Component {
        component: IrComponent::Collapse(component),
    }] = result.ir.nodes.as_slice()
    else {
        panic!("expected one Collapse component, got {:?}", result.ir.nodes);
    };
    component
}

#[test]
fn collapse_default_closed_preserves_rich_title_and_structured_body() {
    let result = compile_source(
        ".collapse {A *collapsible* title}\n    ## Hidden\n\n    Body **content**.\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let component = collapse(&result);
    assert!(!component.is_open);
    assert!(component
        .title
        .iter()
        .any(|inline| matches!(inline, IrInline::Emphasis { .. })));
    assert!(matches!(
        component.children.as_slice(),
        [IrNode::Heading { level: 2, .. }, IrNode::Paragraph { .. }]
    ));
    let IrNode::Paragraph { content, .. } = &component.children[1] else {
        unreachable!();
    };
    assert!(content
        .iter()
        .any(|inline| matches!(inline, IrInline::Strong { .. })));
    assert_eq!(component.span.start, 0);
}

#[test]
fn collapse_open_boolean_preserves_initial_state() {
    for (value, expected) in [
        ("yes", true),
        ("true", true),
        ("no", false),
        ("false", false),
    ] {
        let source = format!(".collapse {{Summary}} open:{{{value}}}\n    Body\n");
        let result = compile_source(&source);
        assert!(result.diagnostics.is_empty(), "{result:?}");
        assert_eq!(collapse(&result).is_open, expected, "{source}");
    }
}

#[test]
fn collapse_rejects_missing_title_body_invalid_open_and_inline_materialization() {
    for source in [
        ".collapse\n    Body\n",
        ".collapse {Summary}\n",
        ".collapse {Summary} open:{maybe}\n    Body\n",
    ] {
        let result = compile_source(source);
        assert!(!result.diagnostics.is_empty(), "{result:?}");
        assert!(result.ir.nodes.is_empty(), "{result:?}");
    }

    let inline = compile_source("before .collapse {Summary} after\n");
    assert!(!inline.diagnostics.is_empty(), "{inline:?}");
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
fn collapse_component_roundtrips_through_document_wire_serde() {
    let result = compile_source(".collapse {Summary **rich**} open:{yes}\n    Body\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let json = serde_json::to_string(&result.ir).expect("serialize Collapse IR");
    let roundtrip: arkst_core::ir::IrDocument =
        serde_json::from_str(&json).expect("deserialize Collapse IR");
    assert_eq!(roundtrip, result.ir);
}
