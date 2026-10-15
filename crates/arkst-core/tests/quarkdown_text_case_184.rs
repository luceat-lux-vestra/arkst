use arkst_core::ir::{IrInline, IrNode, IrTextCase, IrTextVariant};
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

fn single_paragraph(source: &str) -> Vec<IrInline> {
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{source:?}: {result:?}");
    let [IrNode::Paragraph { content, .. }] = result.ir.nodes.as_slice() else {
        panic!("expected one paragraph: {result:?}");
    };
    let wire = serde_json::to_string(&result.ir).expect("serialize");
    let parsed: arkst_core::ir::IrDocument = serde_json::from_str(&wire).expect("deserialize");
    assert_eq!(result.ir, parsed);
    content.clone()
}

#[test]
fn none_and_explicit_none_preserve_original_inline_children() {
    for text_case in ["none", ".none"] {
        let source = format!(".text text:{{Hello}} case:{{{text_case}}}\n");
        let content = single_paragraph(&source);
        assert!(matches!(
            content.as_slice(),
            [IrInline::Text { content, .. }] if content == "Hello"
        ));
    }
}

#[test]
fn complete_non_default_case_domain_preserves_typed_semantics() {
    let cases = [
        ("uppercase", IrTextCase::Uppercase),
        ("lowercase", IrTextCase::Lowercase),
        ("capitalize", IrTextCase::Capitalize),
    ];

    for (name, expected) in cases {
        let source = format!(".text text:{{Hello world}} case:{{{name}}}\n");
        let content = single_paragraph(&source);
        let [IrInline::TextCase {
            case,
            content: children,
            ..
        }] = content.as_slice()
        else {
            panic!("{name}: expected one typed case wrapper: {content:?}");
        };
        assert_eq!(*case, expected, "{name}: {content:?}");
        let mut authored_text = String::new();
        for child in children {
            let IrInline::Text { content, .. } = child else {
                panic!("{name}: expected text-only children: {children:?}");
            };
            authored_text.push_str(content);
        }
        assert_eq!(authored_text, "Hello world", "{name}: {children:?}");
    }
}

#[test]
fn text_case_preserves_rich_children_call_provenance_and_transform_order() {
    let source = ".text case:{capitalize} variant:{smallcaps} size:{small} weight:{bold} decoration:{underline}\n    hello *world* and `code`\n";
    let (result, source_id) = compile_source_with_id(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let [IrNode::Paragraph { content, .. }] = result.ir.nodes.as_slice() else {
        panic!("expected one paragraph: {result:?}");
    };
    let [IrInline::TextVariant {
        variant: IrTextVariant::SmallCaps,
        content: variant_content,
        ..
    }] = content.as_slice()
    else {
        panic!("expected variant wrapper: {content:?}");
    };
    let [IrInline::TextCase {
        case: IrTextCase::Capitalize,
        content: cased,
        span,
    }] = variant_content.as_slice()
    else {
        panic!("expected typed case wrapper: {variant_content:?}");
    };
    assert_eq!(span.source_id, source_id);
    assert_eq!(&source[span.start..span.end], source.trim_end());
    assert!(matches!(
        cased.as_slice(),
        [IrInline::TextSize { content, .. }]
            if matches!(content.as_slice(), [IrInline::Underline { content, .. }]
                if matches!(content.as_slice(), [IrInline::Strong { content, .. }]
                    if content.iter().any(|inline| matches!(inline, IrInline::Emphasis { .. }))
                        && content.iter().any(|inline| matches!(inline, IrInline::Code { .. }))))
    ));
}

#[test]
fn text_case_composes_inside_url_while_blank_url_uses_authored_plaintext() {
    let linked = single_paragraph(
        ".text text:{Hello} url:{https://example.org} case:{uppercase} decoration:{overline}\n",
    );
    assert!(matches!(
        linked.as_slice(),
        [IrInline::Link { destination, content, .. }]
            if destination == "https://example.org"
                && matches!(content.as_slice(), [IrInline::TextCase {
                    case: IrTextCase::Uppercase,
                    content,
                    ..
                }] if matches!(content.as_slice(), [IrInline::Overline { .. }]))
    ));

    let derived = single_paragraph(".text url:{   } case:{uppercase}\n    Hello *world*\n");
    assert!(matches!(
        derived.as_slice(),
        [IrInline::Link { destination, content, .. }]
            if destination == "Hello world"
                && matches!(content.as_slice(), [IrInline::TextCase {
                    case: IrTextCase::Uppercase,
                    ..
                }])
    ));
}

#[test]
fn invalid_case_values_and_named_only_boundary_fail_atomically() {
    for source in [
        ".text text:{Hello} case:{123}\n",
        ".text text:{Hello} case:{titlecase}\n",
        ".text text:{Hello} case:{uppercase} case:{lowercase}\n",
        ".text {Hello} {https://example.org} {uppercase}\n",
    ] {
        let result = compile_source(source);
        assert!(!result.diagnostics.is_empty(), "{source:?}: {result:?}");
        assert!(
            result.diagnostics[0].primary.is_some(),
            "{source:?}: {result:?}"
        );
        assert!(result.ir.nodes.is_empty(), "{source:?}: {result:?}");
    }
}
