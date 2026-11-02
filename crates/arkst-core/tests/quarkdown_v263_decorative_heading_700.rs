use arkst_core::ir::{IrHeadingKind, IrInline, IrNode};
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

#[test]
fn decorative_heading_interrupts_paragraph_and_reaches_backend_neutral_ir() {
    let source = "Before witness\n##! Decorative *한글*\nAfter witness\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");

    let [IrNode::Paragraph { .. }, IrNode::Heading {
        kind,
        level,
        content,
        span,
    }, IrNode::Paragraph { .. }] = result.ir.nodes.as_slice()
    else {
        panic!("unexpected decorative heading IR: {:?}", result.ir.nodes);
    };
    assert_eq!(*kind, IrHeadingKind::Decorative);
    assert_eq!(*level, 2);
    assert!(matches!(
        content.as_slice(),
        [IrInline::Text { content, .. }, IrInline::Emphasis { .. }] if content == "Decorative "
    ));
    assert_eq!(
        source.get(span.start..span.end),
        Some("##! Decorative *한글*")
    );
}

#[test]
fn decorative_heading_wire_identity_is_explicit_and_standard_shape_stays_compatible() {
    let decorative = compile_source("#! Decorative\n");
    assert!(decorative.diagnostics.is_empty(), "{decorative:?}");
    let json = serde_json::to_string(&decorative.ir).expect("serialize decorative heading");
    assert!(json.contains("\"kind\":\"Decorative\""), "{json}");
    let roundtrip: arkst_core::ir::IrDocument =
        serde_json::from_str(&json).expect("deserialize decorative heading");
    assert_eq!(roundtrip, decorative.ir);

    let standard = compile_source("# Standard\n");
    assert!(standard.diagnostics.is_empty(), "{standard:?}");
    let legacy_shape = serde_json::to_string(&standard.ir).expect("serialize standard heading");
    assert!(!legacy_shape.contains("\"kind\""), "{legacy_shape}");
    let roundtrip: arkst_core::ir::IrDocument =
        serde_json::from_str(&legacy_shape).expect("deserialize legacy heading shape");
    let [IrNode::Heading { kind, .. }] = roundtrip.nodes.as_slice() else {
        panic!("expected standard heading");
    };
    assert_eq!(*kind, IrHeadingKind::Standard);
}

#[test]
fn malformed_decorative_markers_remain_visible_source_text() {
    for source in ["#!NoSpace\n", "#######! Too deep\n"] {
        let result = compile_source(source);
        assert!(result.diagnostics.is_empty(), "{source:?}: {result:?}");
        let [IrNode::Paragraph { content, .. }] = result.ir.nodes.as_slice() else {
            panic!(
                "malformed marker must remain a paragraph: {source:?}: {:?}",
                result.ir.nodes
            );
        };
        assert!(content.iter().any(
            |inline| matches!(inline, IrInline::Text { content, .. } if content.contains('!'))
        ));
    }
}
