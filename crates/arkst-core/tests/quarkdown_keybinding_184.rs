use arkst_core::ir::{IrInline, IrKeybindingPart, IrNode};
use arkst_core::{compile, CompileOptions, SourceId, VirtualProjectBuilder};

fn compile_source(source: &str) -> (arkst_core::CompileResult, SourceId) {
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("entry")
        .add_source("main.qd", source)
        .expect("source")
        .build()
        .expect("project");
    let id = project.sources().get_id(project.entry()).expect("source id");
    (compile(&project, &CompileOptions::default()), id)
}

fn key_parts(input: &str) -> Vec<IrKeybindingPart> {
    let (result, _) = compile_source(&format!(".keybinding {{{input}}}\n"));
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let [IrNode::Paragraph { content, .. }] = result.ir.nodes.as_slice() else {
        panic!("expected paragraph: {:?}", result.ir.nodes);
    };
    let [IrInline::Keybinding { parts, .. }] = content.as_slice() else {
        panic!("expected keybinding: {content:?}");
    };
    parts.clone()
}

#[test]
fn keybinding_preserves_modifier_identity_and_inline_source_span() {
    let source = "Press .keybinding {Cmd+Shift+K} to continue.\n";
    let (result, id) = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let [IrNode::Paragraph { content, .. }] = result.ir.nodes.as_slice() else {
        panic!("expected paragraph: {:?}", result.ir.nodes);
    };
    let IrInline::Keybinding { parts, span } = &content[1] else {
        panic!("expected inline keybinding: {content:?}");
    };
    assert_eq!(
        parts,
        &vec![
            IrKeybindingPart::PrimaryModifier,
            IrKeybindingPart::ShiftModifier,
            IrKeybindingPart::Key("K".into()),
        ]
    );
    assert_eq!(span.source_id, id);
    assert_eq!(span.start, source.find(".keybinding").expect("start"));
    assert_eq!(span.end, source.find(" to continue").expect("end"));
    let serialized = serde_json::to_value(&result.ir).expect("serialize");
    let restored: arkst_core::ir::IrDocument =
        serde_json::from_value(serialized).expect("deserialize");
    assert_eq!(restored, result.ir);
}

#[test]
fn keybinding_parses_aliases_literal_keys_and_delimiters() {
    use IrKeybindingPart::{AltModifier, CtrlModifier, Key, PrimaryModifier, ShiftModifier};
    assert_eq!(key_parts("Ctrl+plus"), vec![CtrlModifier, Key("+".into())]);
    assert_eq!(key_parts("Alt,F4"), vec![AltModifier, Key("F4".into())]);
    assert_eq!(key_parts("shift-a"), vec![ShiftModifier, Key("A".into())]);
    assert_eq!(
        key_parts("meta+option+period"),
        vec![PrimaryModifier, AltModifier, Key(".".into())]
    );
    assert_eq!(key_parts("control+minus"), vec![CtrlModifier, Key("-".into())]);
    assert_eq!(key_parts("Cmd+comma"), vec![PrimaryModifier, Key(",".into())]);
    assert_eq!(key_parts("command+dot"), vec![PrimaryModifier, Key(".".into())]);
}

#[test]
fn keybinding_raw_body_uses_shared_binding_and_rejects_invalid_input() {
    let (result, _) = compile_source(".keybinding\n    Ctrl + Shift + K\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert!(matches!(
        &result.ir.nodes[0],
        IrNode::Paragraph { content, .. }
            if matches!(&content[0], IrInline::Keybinding { parts, .. } if parts.len() == 3)
    ));
    for source in [
        ".keybinding\n",
        ".keybinding { + }\n",
        ".keybinding {Ctrl} {K}\n",
        ".keybinding wrong:{K}\n",
    ] {
        let (result, _) = compile_source(source);
        assert!(!result.diagnostics.is_empty(), "{source}: {result:?}");
        assert!(result.ir.nodes.is_empty(), "{source}: {result:?}");
    }
}

#[test]
fn source_defined_keybinding_has_precedence() {
    let (result, _) = compile_source(".function {keybinding}\n    custom\n\n.keybinding {K}\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert!(!format!("{:?}", result.ir).contains("Keybinding"));
}
