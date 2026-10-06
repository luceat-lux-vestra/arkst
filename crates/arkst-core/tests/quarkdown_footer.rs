use arkst_core::ir::{
    IrComponent, IrDocument, IrInline, IrNode, IrPageCounterTarget, IrPageMarginComponent,
    IrPageMarginPosition, IrValue,
};
use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};

fn compile_source(source: &str) -> arkst_core::CompileResult {
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("valid entry")
        .add_source("main.qd", source)
        .expect("valid source")
        .build()
        .expect("valid project");
    compile(&project, &CompileOptions::default())
}

fn footer(result: &arkst_core::CompileResult) -> &IrPageMarginComponent {
    let [IrNode::Component {
        component: IrComponent::PageMargin(component),
    }] = result.ir.nodes.as_slice()
    else {
        panic!(
            "expected one page-margin component, got {:?}",
            result.ir.nodes
        );
    };
    component
}

#[test]
fn footer_constructs_bottom_center_page_margin_with_structured_body() {
    let source = ".footer\n    Footer .currentpage\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");

    let component = footer(&result);
    assert_eq!(component.position, IrPageMarginPosition::BottomCenter);
    assert_eq!(component.children.len(), 1);
    let IrNode::Paragraph { content, .. } = &component.children[0] else {
        panic!("expected footer paragraph");
    };
    assert!(matches!(
        content.as_slice(),
        [
            IrInline::Text { content, .. },
            IrInline::PageCounter {
                target: IrPageCounterTarget::Current,
                ..
            }
        ] if content == "Footer "
    ));
    assert_eq!(component.span.start, 0);
    assert_eq!(component.span.end, source.len() - 1);
}

#[test]
fn footer_requires_a_block_body() {
    let result = compile_source(".footer\n");
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert!(result.ir.nodes.is_empty(), "{result:?}");
}

#[test]
fn footer_rejects_explicit_arguments_in_the_bounded_body_only_slice() {
    for source in [".footer {text}\n", ".footer content:{text}\n"] {
        let result = compile_source(source);
        assert_eq!(result.diagnostics.len(), 1, "{source}: {result:?}");
        assert!(result.ir.nodes.is_empty(), "{source}: {result:?}");
    }
}

#[test]
fn footer_body_failure_is_atomic() {
    let result = compile_source(
        ".footer\n    before\n\n    .grid columns:{0}\n        broken\n\n    after\n",
    );
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert!(result.ir.nodes.is_empty(), "{result:?}");
}

#[test]
fn source_defined_footer_shadows_the_native_footer() {
    let result = compile_source(".function {footer}\n    custom\n\n.footer\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let [IrNode::Paragraph { content, .. }] = result.ir.nodes.as_slice() else {
        panic!("expected custom footer output, got {:?}", result.ir.nodes);
    };
    assert!(matches!(
        content.as_slice(),
        [IrInline::Text { content, .. }] if content == "custom"
    ));
}

#[test]
fn inline_native_footer_fails_closed_without_fabricated_content() {
    let result = compile_source("before .footer after\n");
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    let [IrNode::Paragraph { content, .. }] = result.ir.nodes.as_slice() else {
        panic!("expected surrounding paragraph, got {:?}", result.ir.nodes);
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
    assert!(!text.contains("footer"));
}

#[test]
fn footer_component_roundtrips_without_backend_state() {
    let result = compile_source(".footer\n    Footer\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let value = IrValue::Component(IrComponent::PageMargin(footer(&result).clone()));
    let encoded = serde_json::to_string(&value).expect("footer component serializes");
    assert!(!encoded.contains("typst"));
    assert!(!encoded.contains("arkst-page-margin-bottom-center"));
    let decoded = serde_json::from_str::<IrValue>(&encoded).expect("footer component deserializes");
    assert_eq!(decoded, value);

    let document = IrDocument {
        nodes: vec![IrNode::Component {
            component: IrComponent::PageMargin(footer(&result).clone()),
        }],
        metadata: result.ir.metadata.clone(),
    };
    let encoded = serde_json::to_string(&document).expect("footer document serializes");
    let decoded =
        serde_json::from_str::<IrDocument>(&encoded).expect("footer document deserializes");
    assert_eq!(decoded, document);
}
