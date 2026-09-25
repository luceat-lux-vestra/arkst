use arkst_core::ir::{IrInline, IrNode};
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

fn paragraph(result: &arkst_core::CompileResult) -> &[IrInline] {
    let [IrNode::Paragraph { content, .. }] = result.ir.nodes.as_slice() else {
        panic!("expected one paragraph, got {:?}", result.ir.nodes);
    };
    content
}

fn inline_text(content: &[IrInline]) -> String {
    let mut out = String::new();
    for inline in content {
        match inline {
            IrInline::Text { content, .. } | IrInline::Code { content, .. } => {
                out.push_str(content)
            }
            IrInline::Emphasis { content, .. }
            | IrInline::Strong { content, .. }
            | IrInline::Strikethrough { content, .. }
            | IrInline::Link { content, .. }
            | IrInline::Image { content, .. } => out.push_str(&inline_text(content)),
            IrInline::Whitespace { .. } => out.push(' '),
            IrInline::SoftBreak { .. } | IrInline::HardBreak { .. } => out.push('\n'),
            IrInline::DirectiveCall { .. }
            | IrInline::ChainedDirectiveCall { .. }
            | IrInline::ExplicitError { .. }
            | IrInline::RawHtml { .. }
            | IrInline::TargetSpecificContent { .. } => {}
        }
    }
    out
}

fn strong_texts(content: &[IrInline], out: &mut Vec<String>) {
    for inline in content {
        match inline {
            IrInline::Strong { content, .. } => {
                out.push(inline_text(content));
                strong_texts(content, out);
            }
            IrInline::Emphasis { content, .. }
            | IrInline::Strikethrough { content, .. }
            | IrInline::Link { content, .. }
            | IrInline::Image { content, .. } => strong_texts(content, out),
            _ => {}
        }
    }
}

fn has_emphasis(content: &[IrInline]) -> bool {
    content.iter().any(|inline| match inline {
        IrInline::Emphasis { .. } => true,
        IrInline::Strong { content, .. }
        | IrInline::Strikethrough { content, .. }
        | IrInline::Link { content, .. }
        | IrInline::Image { content, .. } => has_emphasis(content),
        _ => false,
    })
}

fn link_destinations(content: &[IrInline], out: &mut Vec<String>) {
    for inline in content {
        match inline {
            IrInline::Link {
                content,
                destination,
                ..
            } => {
                out.push(destination.clone());
                link_destinations(content, out);
            }
            IrInline::Emphasis { content, .. }
            | IrInline::Strong { content, .. }
            | IrInline::Strikethrough { content, .. }
            | IrInline::Image { content, .. } => link_destinations(content, out),
            _ => {}
        }
    }
}

#[test]
fn match_reproduces_the_upstream_sample_and_materializes_callback_markdown() {
    let source = ".match {Quarkdown takes its name from quarks} pattern:{[Qq]uark(down|s)?}\n    match:\n    **.match**\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");

    let content = paragraph(&result);
    assert_eq!(
        inline_text(content),
        "Quarkdown takes its name from quarks",
        "{content:?}"
    );
    let mut strong = Vec::new();
    strong_texts(content, &mut strong);
    assert_eq!(strong, ["Quarkdown", "quarks"], "{content:?}");
}

#[test]
fn match_recurses_through_existing_text_owners_without_a_depth_allowlist() {
    let source = ".match {***alpha*** and [beta](https://example.com)} pattern:{a}\n    match:\n    .match::uppercase\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");

    let content = paragraph(&result);
    assert_eq!(
        inline_text(content),
        concat!("Alp", "hA and betA"),
        "{content:?}"
    );
    assert!(has_emphasis(content), "{content:?}");
    let mut links = Vec::new();
    link_destinations(content, &mut links);
    assert_eq!(links, ["https://example.com"], "{content:?}");
}

#[test]
fn match_uses_a_regex_engine_with_lookaround_support() {
    let source = ".match {wow! ok} pattern:{\\w+(?=!)}\n    match:\n    .match::uppercase\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(inline_text(paragraph(&result)), "WOW! ok");
}

#[test]
fn empty_pattern_and_no_match_do_not_invoke_the_replacement() {
    for source in [
        ".match {abc} pattern:{}\n    match:\n    .sum {true} {2}\n",
        ".match {abc} pattern:{z+}\n    match:\n    .sum {true} {2}\n",
    ] {
        let result = compile_source(source);
        assert!(result.diagnostics.is_empty(), "{source:?}: {result:?}");
        assert_eq!(inline_text(paragraph(&result)), "abc", "{source:?}");
    }
}

#[test]
fn invalid_regex_and_failing_replacement_fail_closed() {
    let invalid = compile_source(".match {abc} pattern:{(}\n    match:\n    .match\n");
    assert_eq!(invalid.diagnostics.len(), 1, "{invalid:?}");
    assert_eq!(invalid.diagnostics[0].code, "E3001");
    assert!(invalid.diagnostics[0]
        .message
        .contains("Invalid regular expression"));
    assert!(invalid.ir.nodes.is_empty(), "{invalid:?}");

    let failing = compile_source(".match {abc} pattern:{a}\n    match:\n    .sum {true} {2}\n");
    assert!(!failing.diagnostics.is_empty(), "{failing:?}");
    assert!(failing.ir.nodes.is_empty(), "{failing:?}");
}

#[test]
fn match_shape_is_preflighted_before_argument_evaluation() {
    let result = compile_source(".match {.sum {true} {2}} pattern:{x}\n");
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert!(
        result.diagnostics[0].message.contains("replacement"),
        "{result:?}"
    );
    assert!(
        !result.diagnostics[0].message.contains("numeric arguments"),
        "{result:?}"
    );
    assert!(result.ir.nodes.is_empty(), "{result:?}");
}

#[test]
fn source_defined_match_keeps_precedence_over_the_native_transform() {
    let source = ".function {match}\n    value:\n    custom .value\n\n.match {ok}\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(inline_text(paragraph(&result)), "custom ok");
}
