use arkst_core::ir::IrNode;
use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};
use std::fs;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn g0_s28_inspect_ir_publishes_public_core_serialization_without_version_envelope() {
    let dir = tempdir().expect("temporary project");
    let input = dir.path().join("main.qd");
    let source = "S28 CLI publication\n";
    fs::write(&input, source).expect("write input");

    let output = Command::new(env!("CARGO_BIN_EXE_arkst"))
        .arg("inspect")
        .arg(&input)
        .args(["--emit", "ir"])
        .output()
        .expect("arkst inspect process must run");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "{:?}", output.stderr);

    let cli_json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("CLI stdout must be JSON");
    assert!(
        cli_json.get("version").is_none(),
        "current inspect IR publication must remain unversioned for the G0 baseline"
    );

    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("valid entry")
        .add_source("main.qd", source)
        .expect("valid source")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(result.diagnostics.is_empty(), "{result:?}");

    let public_core_json = serde_json::to_value(&result.ir).expect("public core IR must serialize");
    assert_eq!(cli_json, public_core_json);

    let [IrNode::Paragraph { span, .. }] = result.ir.nodes.as_slice() else {
        panic!("expected one semantic paragraph: {result:?}");
    };
    let first_cli_node = cli_json["nodes"][0]
        .as_object()
        .and_then(|node| node.values().next())
        .expect("first CLI node payload");
    assert_eq!(
        first_cli_node["span"],
        serde_json::to_value(span).expect("source span must serialize")
    );
}
