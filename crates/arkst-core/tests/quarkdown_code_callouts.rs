use arkst_core::ir::{IrCodeCallout, IrNode};
use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};

fn compile_source(source: &str) -> arkst_core::CompileResult {
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .unwrap()
        .add_source("main.qd", source)
        .unwrap()
        .build()
        .unwrap();
    compile(&project, &CompileOptions::default())
}

#[test]
fn code_callouts_are_typed_sorted_and_do_not_require_in_range_lines() {
    let result = compile_source(".code lang:{rust} callouts:{\n    - 3: Third *item*\n    - 1: First\n    - 9: Beyond\n}\n    alpha\n    beta\n    gamma\n");
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let [IrNode::CodeBlock {
        language,
        source,
        line_numbers,
        callouts,
        ..
    }] = result.ir.nodes.as_slice()
    else {
        panic!("{:?}", result.ir.nodes);
    };
    assert_eq!(language.as_deref(), Some("rust"));
    assert_eq!(source, "alpha\nbeta\ngamma");
    assert_eq!(*line_numbers, Some(true));
    assert_eq!(
        callouts,
        &vec![
            IrCodeCallout {
                line: 1,
                description: "First".into()
            },
            IrCodeCallout {
                line: 3,
                description: "Third item".into()
            },
            IrCodeCallout {
                line: 9,
                description: "Beyond".into()
            },
        ]
    );
}

#[test]
fn code_linenumbers_no_and_empty_callouts_are_supported() {
    let result = compile_source(".code linenumbers:{no} callouts:{}\n    alpha\n");
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let [IrNode::CodeBlock {
        line_numbers,
        callouts,
        ..
    }] = result.ir.nodes.as_slice()
    else {
        panic!("{:?}", result.ir.nodes);
    };
    assert_eq!(*line_numbers, Some(false));
    assert!(callouts.is_empty());
}

#[test]
fn code_callout_keys_fail_closed() {
    for (source, message) in [
        (
            ".code callouts:{\n    - 0: zero\n}\n    alpha\n",
            "must be positive",
        ),
        (
            ".code callouts:{\n    - nope: text\n}\n    alpha\n",
            "must be integers",
        ),
    ] {
        let result = compile_source(source);
        assert!(
            result.ir.nodes.is_empty(),
            "{source}: {:?}",
            result.ir.nodes
        );
        assert!(
            result
                .diagnostics
                .iter()
                .any(|d| d.message.contains(message)),
            "{source}: {:?}",
            result.diagnostics
        );
    }
}

#[test]
fn code_binding_rejects_wrong_alias_and_baseline_options_are_explicitly_deferred() {
    let wrong = compile_source(".code callout:{}\n    alpha\n");
    assert!(wrong.ir.nodes.is_empty());
    assert!(!wrong.diagnostics.is_empty());

    for source in [
        ".code caption:{x}\n    alpha\n",
        ".code focus:{1..2}\n    alpha\n",
        ".code ref:{x}\n    alpha\n",
    ] {
        let result = compile_source(source);
        assert!(result.ir.nodes.is_empty(), "{source}");
        assert!(
            result.diagnostics.iter().any(|d| d
                .message
                .contains("outside the bounded v2.6 callouts slice")),
            "{:?}",
            result.diagnostics
        );
    }
}

#[test]
fn source_defined_code_still_shadows_native() {
    let result = compile_source(".function {code}\n    callouts code:\n    SHADOWED\n\n.code callouts:{\n    - 1: ignored\n}\n    alpha\n");
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert!(!result
        .ir
        .nodes
        .iter()
        .any(|node| matches!(node, IrNode::CodeBlock { .. })));
    assert!(format!("{:?}", result.ir).contains("SHADOWED"));
}

#[test]
fn markdown_fenced_code_keeps_legacy_backend_contract() {
    let result = compile_source("```rust\nfn main() {}\n```\n");
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let [IrNode::CodeBlock {
        line_numbers,
        callouts,
        ..
    }] = result.ir.nodes.as_slice()
    else {
        panic!("{:?}", result.ir.nodes);
    };
    assert_eq!(*line_numbers, None);
    assert!(callouts.is_empty());
}
