use scribium_core::ir::{IrInline, IrNode, NativeTarget};
use scribium_core::{
    compile, compile_with_capabilities, Capabilities, Capability, CompileOptions, Severity,
    SourceId, SourceSpan, VirtualProjectBuilder,
};

fn project_for(source: &str) -> scribium_core::VirtualProject {
    VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("valid path")
        .add_source("main.qd", source)
        .expect("valid source")
        .build()
        .expect("valid project")
}

fn compile_source_with_capabilities(
    source: &str,
    capabilities: Capabilities,
) -> scribium_core::CompileResult {
    let project = project_for(source);
    compile_with_capabilities(&project, &CompileOptions::default(), capabilities)
}

fn compile_source(source: &str) -> scribium_core::CompileResult {
    let project = project_for(source);
    compile(&project, &CompileOptions::default())
}

#[test]
fn capability_defaults_are_explicit_and_closed() {
    assert!(Capabilities::default().allows(Capability::NativeContent));
    assert!(!Capabilities::none().allows(Capability::NativeContent));
}

fn target_block(node: &IrNode) -> (&str, scribium_core::SourceSpan) {
    let IrNode::TargetSpecificContent { content } = node else {
        panic!("expected target-specific block, got {node:?}");
    };
    assert_eq!(content.target, NativeTarget::Html);
    (&content.content, content.span)
}

#[test]
fn default_compile_grants_native_content_and_evaluates_block_html() {
    let source = ".html {<div>Hello</div>}\n";
    let result = compile_source(source);

    assert!(
        result.diagnostics.is_empty(),
        "unexpected: {:?}",
        result.diagnostics
    );
    assert_eq!(result.ir.nodes.len(), 1);
    let (content, span) = target_block(&result.ir.nodes[0]);
    assert_eq!(content, "<div>Hello</div>");
    assert_eq!(&source[span.start..span.end], ".html {<div>Hello</div>}");
}

#[test]
fn inline_html_preserves_order_and_utf8_surrounding_content() {
    let source = "한글 **Hello** .html {<em>world</em>}!\n";
    let result = compile_source(source);

    assert!(
        result.diagnostics.is_empty(),
        "unexpected: {:?}",
        result.diagnostics
    );
    let [IrNode::Paragraph { content, .. }] = result.ir.nodes.as_slice() else {
        panic!("expected one paragraph, got {:?}", result.ir.nodes);
    };
    assert!(
        matches!(
            content.as_slice(),
            [
                IrInline::Text { .. },
                IrInline::Strong { .. },
                IrInline::Text { content: before, .. },
                IrInline::TargetSpecificContent { content },
                IrInline::Text { content: after, .. },
            ] if before == " "
                && after == "!"
                && content.target == NativeTarget::Html
                && content.content == "<em>world</em>"
        ),
        "content: {content:?}"
    );
}

#[test]
fn html_named_and_nested_arguments_use_the_normal_string_boundary() {
    let result = compile_source(".html content:{before .uppercase {world} after}\n");

    assert!(
        result.diagnostics.is_empty(),
        "unexpected: {:?}",
        result.diagnostics
    );
    let (content, _) = target_block(&result.ir.nodes[0]);
    assert_eq!(content, "before WORLD after");

    let result = compile_source(".html content:{<em>world</em>}\n");
    assert!(
        result.diagnostics.is_empty(),
        "unexpected: {:?}",
        result.diagnostics
    );
    let (content, _) = target_block(&result.ir.nodes[0]);
    assert_eq!(content, "<em>world</em>");
}

#[test]
fn html_crlf_and_utf8_provenance_remain_source_backed() {
    let source = "한글\r\n.html {<em>세계</em>}\r\n";
    let result = compile_source(source);

    assert!(
        result.diagnostics.is_empty(),
        "unexpected: {:?}",
        result.diagnostics
    );
    let IrNode::TargetSpecificContent { content } = &result.ir.nodes[1] else {
        panic!("expected target-specific block, got {:?}", result.ir.nodes);
    };
    assert_eq!(content.content, "<em>세계</em>");
    assert_eq!(
        &source[content.span.start..content.span.end],
        ".html {<em>세계</em>}"
    );
}

#[test]
fn html_indented_body_is_owned_as_an_opaque_function_string() {
    let source = ".html\n    <div>\n        Hello\n    </div>\n";
    let result = compile_source(source);

    assert!(
        result.diagnostics.is_empty(),
        "unexpected: {:?}",
        result.diagnostics
    );
    let (content, span) = target_block(&result.ir.nodes[0]);
    assert_eq!(content, "<div>\n        Hello\n    </div>\n");
    assert_eq!(&source[span.start..span.end], source);
    assert!(!format!("{:?}", result.ir).contains("RawHtml"));
}

#[test]
fn html_argument_binding_reports_required_failures_without_unresolved_duplicates() {
    for (source, expected_message) in [
        (".html\n", "requires one `content` argument"),
        (
            ".html {one} {two}\n",
            "accepts exactly one `content` argument",
        ),
        (
            ".html {one} content:{two}\n",
            "received `content` more than once",
        ),
        (
            ".html other:{value}\n",
            "does not support named argument `other`",
        ),
    ] {
        let result = compile_source(source);
        assert_eq!(result.ir.nodes, Vec::<IrNode>::new(), "source: {source:?}");
        assert_eq!(result.diagnostics.len(), 1, "source: {source:?}");
        assert_eq!(result.diagnostics[0].code, "E3003");
        assert!(
            result.diagnostics[0].message.contains(expected_message),
            "diagnostic: {:?}",
            result.diagnostics[0]
        );
    }

    let result = compile_source(".html {explicit}\n    body\n");
    assert_eq!(result.ir.nodes, Vec::<IrNode>::new());
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].code, "E3003");
    assert!(result.diagnostics[0].message.contains("both a body"));
}

#[test]
fn native_content_denial_is_one_source_backed_error_before_node_creation() {
    let source = ".html {<em>denied</em>}\n";
    let result = compile_source_with_capabilities(source, Capabilities::none());

    assert_eq!(result.ir.nodes, Vec::<IrNode>::new());
    assert_eq!(result.diagnostics.len(), 1);
    let diagnostic = &result.diagnostics[0];
    assert_eq!(diagnostic.code, "E3004");
    assert!(matches!(diagnostic.severity, Severity::Error));
    assert_eq!(
        diagnostic.primary,
        Some(SourceSpan::new(SourceId(1), 0, source.trim_end().len()))
    );
    assert!(diagnostic.message.contains("NativeContent"));
    assert!(diagnostic.hints.iter().any(|hint| hint.contains("Grant")));
    assert!(!format!("{:?}", result.ir).contains("denied"));
}

#[test]
fn native_content_denial_owns_the_indented_html_body_boundary() {
    let source = ".html\n    <div>denied</div>\n";
    let result = compile_source_with_capabilities(source, Capabilities::none());

    assert_eq!(result.ir.nodes, Vec::<IrNode>::new());
    assert_eq!(
        result.diagnostics.len(),
        1,
        "diagnostics: {:?}",
        result.diagnostics
    );
    assert_eq!(result.diagnostics[0].code, "E3004");
    assert!(!result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "E3010"));
    assert!(!result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "E8001"));
    assert!(!format!("{:?}", result.ir).contains("TargetSpecificContent"));
    assert!(!format!("{:?}", result.ir).contains("denied"));
}

#[test]
fn parser_owned_html_body_is_not_a_generic_raw_html_escape_hatch() {
    let result = compile_source(".unknown\n    <div>not owned</div>\n");

    assert!(result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "E8001"));
    assert!(!result
        .ir
        .nodes
        .iter()
        .any(|node| matches!(node, IrNode::TargetSpecificContent { .. })));
    assert!(!format!("{:?}", result.ir).contains("not owned"));
}

#[test]
fn ordinary_mixed_quarkdown_raw_html_remains_fail_closed_and_css_is_unresolved() {
    for source in ["<em>x</em>\n", "<!-- comment -->\n"] {
        let result = compile_source(source);
        assert!(result.diagnostics.iter().any(|diag| diag.code == "E8001"));
        assert!(
            result
                .ir
                .nodes
                .iter()
                .all(|node| !format!("{node:?}").contains("TargetSpecificContent")),
            "source: {source:?}, ir: {:?}",
            result.ir
        );
    }

    let result = compile_source(".css {body { color: red; }}\n");
    assert!(
        result.diagnostics.is_empty(),
        "unexpected: {:?}",
        result.diagnostics
    );
    assert!(matches!(
        result.ir.nodes.as_slice(),
        [IrNode::FunctionCall { name, .. }] if name == "css"
    ));
    assert!(!format!("{:?}", result.ir).contains("TargetSpecificContent"));
}
