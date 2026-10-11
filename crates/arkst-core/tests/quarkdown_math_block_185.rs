use arkst_core::ir::IrNode;
use arkst_core::{compile, CompileOptions, SourceId, VirtualProjectBuilder};

fn compile_source(source: &str) -> (arkst_core::CompileResult, SourceId) {
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("entry")
        .add_source("main.qd", source)
        .expect("source")
        .build()
        .expect("project");
    let id = project
        .sources()
        .get_id(project.entry())
        .expect("source id");
    (compile(&project, &CompileOptions::default()), id)
}

fn single_math(source: &str) -> (IrNode, SourceId) {
    let (result, id) = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let [node] = result.ir.nodes.as_slice() else {
        panic!("one math node expected: {:?}", result.ir.nodes);
    };
    assert!(
        matches!(node, IrNode::Math { display: true, .. }),
        "{node:?}"
    );
    (node.clone(), id)
}

#[test]
fn explicitly_block_math_preserves_source_and_provenance() {
    let input = ".math content:{x^2 + y} block:{true}\n";
    let (node, id) = single_math(input);
    let IrNode::Math {
        source,
        display,
        span,
    } = node
    else {
        unreachable!()
    };
    assert_eq!(source, "x^2 + y");
    assert!(display);
    assert_eq!(span.source_id, id);
    assert_eq!(span.start, 0);
    assert_eq!(span.end, input.trim_end().len());
}

#[test]
fn positional_and_raw_first_slot_body_produce_same_math() {
    let (positional, _) = single_math(".math {a + b} block:{true}\n");
    let (raw_body, _) = single_math(".math block:{true}\n    a + b\n");
    assert!(matches!(positional, IrNode::Math { source, .. } if source == "a + b"));
    assert!(matches!(raw_body, IrNode::Math { source, .. } if source == "a + b"));
}

#[test]
fn invalid_modes_tex_or_reference_fail_atomically() {
    for input in [
        ".math {a + b}\n",
        ".math {a + b} block:{false}\n",
        ".math {a + b} block:{yes} ref:{eq}\n",
        ".math {a + b} block:{true} ref:{eq}\n",
        ".math {a + b} block:{true} foreground:{red}\n",
        ".math {\\frac{1}{2}} block:{true}\n",
        ".math {} block:{true}\n",
        ".math {a + b} block:{true} content:{other}\n",
    ] {
        let (result, _) = compile_source(input);
        assert!(!result.diagnostics.is_empty(), "{input:?}: {result:?}");
        assert!(
            result
                .ir
                .nodes
                .iter()
                .all(|n| !matches!(n, IrNode::Math { .. })),
            "{input:?}: {result:?}"
        );
    }
}

#[test]
fn source_defined_math_keeps_existing_precedence() {
    let (result, _) =
        compile_source(".function {math}\n    custom\n\n.math {a + b} block:{true}\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert!(result
        .ir
        .nodes
        .iter()
        .all(|n| !matches!(n, IrNode::Math { .. })));
}

#[test]
fn block_math_ir_serialization_roundtrip() {
    let (node, _) = single_math(".math content:{a + b} block:{true}\n");
    let json = serde_json::to_string(&node).expect("serialize");
    let decoded: IrNode = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(decoded, node);
}
