use arkst_core::ir::{IrInline, IrNode, IrPageCounterTarget, IrPageMarginPosition};
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

fn page_margin(result: &arkst_core::CompileResult) -> (&IrPageMarginPosition, &[IrNode]) {
    let [IrNode::PageMarginContent {
        position, children, ..
    }] = result.ir.nodes.as_slice()
    else {
        panic!(
            "expected one page-margin initializer, got {:?}",
            result.ir.nodes
        );
    };
    (position, children)
}

#[test]
fn page_margin_position_domain_materializes_all_pinned_variants() {
    let cases = [
        ("topleftcorner", IrPageMarginPosition::TopLeftCorner),
        ("topleft", IrPageMarginPosition::TopLeft),
        ("topcenter", IrPageMarginPosition::TopCenter),
        ("topright", IrPageMarginPosition::TopRight),
        ("toprightcorner", IrPageMarginPosition::TopRightCorner),
        ("righttop", IrPageMarginPosition::RightTop),
        ("rightmiddle", IrPageMarginPosition::RightMiddle),
        ("rightbottom", IrPageMarginPosition::RightBottom),
        ("bottomrightcorner", IrPageMarginPosition::BottomRightCorner),
        ("bottomright", IrPageMarginPosition::BottomRight),
        ("bottomcenter", IrPageMarginPosition::BottomCenter),
        ("bottomleft", IrPageMarginPosition::BottomLeft),
        ("bottomleftcorner", IrPageMarginPosition::BottomLeftCorner),
        ("leftbottom", IrPageMarginPosition::LeftBottom),
        ("leftmiddle", IrPageMarginPosition::LeftMiddle),
        ("lefttop", IrPageMarginPosition::LeftTop),
        ("topoutsidecorner", IrPageMarginPosition::TopOutsideCorner),
        ("topoutside", IrPageMarginPosition::TopOutside),
        (
            "bottomoutsidecorner",
            IrPageMarginPosition::BottomOutsideCorner,
        ),
        ("bottomoutside", IrPageMarginPosition::BottomOutside),
        ("topinsidecorner", IrPageMarginPosition::TopInsideCorner),
        ("topinside", IrPageMarginPosition::TopInside),
        (
            "bottominsidecorner",
            IrPageMarginPosition::BottomInsideCorner,
        ),
        ("bottominside", IrPageMarginPosition::BottomInside),
    ];

    for (name, expected) in cases {
        let source = format!(".pagemargin {{{name}}}\n    body\n");
        let result = compile_source(&source);
        assert!(result.diagnostics.is_empty(), "{name}: {result:?}");
        let (position, children) = page_margin(&result);
        assert_eq!(*position, expected, "{name}");
        assert!(!children.is_empty(), "{name}: {children:?}");
    }
}

#[test]
fn footer_is_exact_bottom_center_sugar_and_body_is_evaluated_structurally() {
    let result = compile_source(".doctype {paged}\n.footer\n    Page .currentpage\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let (position, children) = page_margin(&result);
    assert_eq!(*position, IrPageMarginPosition::BottomCenter);
    let [IrNode::Paragraph { content, .. }] = children else {
        panic!("expected footer paragraph, got {children:?}");
    };
    assert!(content.iter().any(|inline| matches!(
        inline,
        IrInline::PageCounter {
            target: IrPageCounterTarget::Current,
            ..
        }
    )));
}

#[test]
fn page_margin_rejects_missing_body_invalid_position_and_inline_body_shape() {
    for source in [
        ".pagemargin {topcenter}\n",
        ".pagemargin {unknown}\n    body\n",
        ".footer\n",
        "prefix .footer {body}\n",
    ] {
        let result = compile_source(source);
        assert!(!result.diagnostics.is_empty(), "{source:?}: {result:?}");
        assert!(
            !result
                .ir
                .nodes
                .iter()
                .any(|node| matches!(node, IrNode::PageMarginContent { .. })),
            "{source:?}: {result:?}"
        );
    }
}

#[test]
fn source_defined_page_margin_names_keep_precedence() {
    let source = ".function {footer}\n    custom-footer\n\n.footer\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert!(!result
        .ir
        .nodes
        .iter()
        .any(|node| matches!(node, IrNode::PageMarginContent { .. })));
    assert!(result.ir.nodes.iter().any(|node| matches!(
        node,
        IrNode::Paragraph { content, .. }
            if content.iter().any(|inline| matches!(
                inline,
                IrInline::Text { content, .. } if content.contains("custom-footer")
            ))
    )));
}

#[test]
fn page_margin_ir_roundtrips_position_children_and_span() {
    let result = compile_source(".footer\n    footer body\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let node = result.ir.nodes[0].clone();
    let encoded = serde_json::to_value(&node).expect("page-margin node serializes");
    let decoded: IrNode = serde_json::from_value(encoded).expect("page-margin node deserializes");
    assert_eq!(decoded, node);
}
