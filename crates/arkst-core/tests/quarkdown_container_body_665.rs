use arkst_core::ir::{IrComponent, IrInline, IrNode};
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

fn containers(result: &arkst_core::CompileResult) -> Vec<&arkst_core::ir::IrContainerComponent> {
    result
        .ir
        .nodes
        .iter()
        .filter_map(|node| match node {
            IrNode::Component {
                component: IrComponent::Container(component),
            } => Some(component),
            _ => None,
        })
        .collect()
}

fn paragraph_inlines(component: &arkst_core::ir::IrContainerComponent) -> &[IrInline] {
    let [IrNode::Paragraph { content, .. }] = component.children.as_slice() else {
        panic!("expected one paragraph, got {:?}", component.children);
    };
    content
}

fn inline_text(inlines: &[IrInline]) -> String {
    let mut text = String::new();
    for inline in inlines {
        match inline {
            IrInline::Text { content, .. } => text.push_str(content),
            IrInline::Strong { content, .. } => text.push_str(&inline_text(content)),
            other => panic!("unexpected inline {other:?}"),
        }
    }
    text
}

fn only_paragraph_text(nodes: &[IrNode]) -> String {
    let [IrNode::Paragraph { content, .. }] = nodes else {
        panic!("expected one paragraph, got {nodes:?}");
    };
    inline_text(content)
}

#[test]
fn container_named_body_parses_literal_and_generated_strings_as_rich_block_markdown() {
    let result = compile_source(
        ".container body:{Named **Markdown** body.}\n\n.container body:{.string {\"Generated **Markdown** body.\"}}\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");

    let components = containers(&result);
    assert_eq!(components.len(), 2, "{result:?}");
    assert_eq!(
        inline_text(paragraph_inlines(components[0])),
        "Named Markdown body."
    );
    assert!(paragraph_inlines(components[0])
        .iter()
        .any(|inline| matches!(inline, IrInline::Strong { .. })));
    assert_eq!(
        inline_text(paragraph_inlines(components[1])),
        "Generated Markdown body."
    );
    assert!(paragraph_inlines(components[1])
        .iter()
        .any(|inline| matches!(inline, IrInline::Strong { .. })));
}

#[test]
fn container_named_body_preserves_variable_and_callable_rich_content() {
    let result = compile_source(
        ".var {bodytext} {Variable **Markdown** body.}\n\
         .function {makebody}\n    Callable **Markdown** body.\n\n\
         .container body:{.bodytext}\n\n.container body:{.makebody}\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");

    let components = containers(&result);
    assert_eq!(components.len(), 2, "{result:?}");
    assert_eq!(
        inline_text(paragraph_inlines(components[0])),
        "Variable Markdown body."
    );
    assert_eq!(
        inline_text(paragraph_inlines(components[1])),
        "Callable Markdown body."
    );
}

#[test]
fn container_named_body_materializes_numbers_booleans_and_none() {
    let result = compile_source(
        ".container body:{.sum {1} {2}}\n\n\
         .container body:{.iseven {2}}\n\n\
         .container body:{.none}\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");

    let components = containers(&result);
    assert_eq!(components.len(), 3, "{result:?}");
    assert_eq!(inline_text(paragraph_inlines(components[0])), "3");
    assert_eq!(inline_text(paragraph_inlines(components[1])), "true");
    assert!(components[2].children.is_empty());
}

#[test]
fn shared_block_target_keeps_nullable_container_none_distinct_from_required_content_none() {
    let container_result = compile_source(".container body:{.none}\n");
    assert!(
        container_result.diagnostics.is_empty(),
        "{container_result:?}"
    );
    assert!(containers(&container_result)[0].children.is_empty());

    let box_result = compile_source(".box body:{.none}\n");
    assert!(box_result.diagnostics.is_empty(), "{box_result:?}");
    let [IrNode::Component {
        component: IrComponent::Box(component),
    }] = box_result.ir.nodes.as_slice()
    else {
        panic!("expected one box, got {:?}", box_result.ir.nodes);
    };
    assert_eq!(only_paragraph_text(&component.children), "None");
}

#[test]
fn container_rejects_explicit_and_indented_body_collision_before_evaluation() {
    let result = compile_source(
        ".container body:{Named body}\n    .grid columns:{0}\n        must not run\n",
    );
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert!(result.ir.nodes.is_empty(), "{result:?}");
    assert!(
        result.diagnostics[0]
            .message
            .contains("Argument `body` was bound more than once"),
        "{result:?}"
    );
    assert!(!result.diagnostics[0].message.contains("columns"));
}

#[test]
fn container_rejects_unverified_structured_body_conversion_with_argument_provenance() {
    let source = ".container body:{.range {1} {3}}\n";
    let result = compile_source(source);
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert!(result.ir.nodes.is_empty(), "{result:?}");
    let diagnostic = &result.diagnostics[0];
    assert!(
        diagnostic.message.contains("parameter `body`"),
        "{result:?}"
    );
    let primary = diagnostic.primary.expect("body conversion primary span");
    assert_eq!(&source[primary.start..primary.end], "body:{.range {1} {3}}");
}
