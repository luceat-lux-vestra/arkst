use arkst_core::ir::IrNode;
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

fn page_break_count(result: &arkst_core::CompileResult) -> usize {
    result
        .ir
        .nodes
        .iter()
        .filter(|node| matches!(node, IrNode::PageBreak { .. }))
        .count()
}

#[test]
fn pagebreak_builtin_and_triple_angle_share_typed_ir_boundary() {
    let result = compile_source(".pagebreak\n\n<<<\n");
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(page_break_count(&result), 2);
    assert!(matches!(result.ir.nodes[0], IrNode::PageBreak { .. }));
    assert!(matches!(result.ir.nodes[1], IrNode::PageBreak { .. }));
}

#[test]
fn malformed_pagebreak_invocation_fails_closed_without_emitting_boundary() {
    let result = compile_source(".pagebreak unexpected:{value}\n");
    assert!(
        !result.diagnostics.is_empty(),
        "unsupported pagebreak arguments must be rejected"
    );
    assert_eq!(page_break_count(&result), 0);
}

#[test]
fn source_defined_pagebreak_shadows_native_boundary() {
    let result = compile_source(
        ".function {pagebreak}\n    SHADOW-BREAK\n\n.pagebreak\n",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(page_break_count(&result), 0);
    assert!(
        format!("{:?}", result.ir).contains("SHADOW-BREAK"),
        "source-defined function must retain ownership of the name"
    );
}

#[test]
fn pagebreak_ir_round_trips_through_wire_format() {
    let result = compile_source("<<<\n");
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(page_break_count(&result), 1);

    let encoded = serde_json::to_value(&result.ir).expect("serialize pagebreak IR");
    let decoded: arkst_core::ir::IrDocument =
        serde_json::from_value(encoded).expect("deserialize pagebreak IR");
    assert!(matches!(decoded.nodes.as_slice(), [IrNode::PageBreak { .. }]));
}
