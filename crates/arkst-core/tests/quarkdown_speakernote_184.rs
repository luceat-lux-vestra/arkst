use arkst_core::ir::{IrComponent, IrInline, IrNode, IrSpeakerNoteComponent};
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

fn speaker_notes(result: &arkst_core::CompileResult) -> Vec<&IrSpeakerNoteComponent> {
    result
        .ir
        .nodes
        .iter()
        .filter_map(|node| match node {
            IrNode::Component {
                component: IrComponent::SpeakerNote(component),
            } => Some(component),
            _ => None,
        })
        .collect()
}

#[test]
fn speaker_note_block_preserves_structured_markdown_and_provenance() {
    let source = ".doctype {slides}\n.speakernote\n    ## Cue\n\n    Remember **this**.\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let notes = speaker_notes(&result);
    let [note] = notes.as_slice() else {
        panic!("expected one speaker note, got {:?}", result.ir.nodes);
    };
    assert!(matches!(
        note.children.as_slice(),
        [IrNode::Heading { level: 2, .. }, IrNode::Paragraph { .. }]
    ));
    let IrNode::Paragraph { content, span } = &note.children[1] else {
        unreachable!();
    };
    assert!(content
        .iter()
        .any(|inline| matches!(inline, IrInline::Strong { .. })));
    assert_eq!(
        source.get(note.span.start..note.span.end),
        Some(".speakernote\n    ## Cue\n\n    Remember **this**.")
    );
    assert_eq!(source.get(span.start..span.end), Some("Remember **this**."));
}

#[test]
fn speaker_note_likely_body_forms_and_multiple_notes_preserve_source_order() {
    let source =
        ".doctype {slides}\n.speakernote {First}\n.speakernote content:{Second **rich**}\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let notes = speaker_notes(&result);
    assert_eq!(notes.len(), 2, "{:?}", result.ir.nodes);

    let [IrNode::Paragraph { content: first, .. }] = notes[0].children.as_slice() else {
        panic!("expected first note paragraph")
    };
    assert!(matches!(first.as_slice(), [IrInline::Text { content, .. }] if content == "First"));

    let [IrNode::Paragraph {
        content: second, ..
    }] = notes[1].children.as_slice()
    else {
        panic!("expected second note paragraph")
    };
    assert!(second
        .iter()
        .any(|inline| matches!(inline, IrInline::Strong { .. })));
}

#[test]
fn speaker_note_gate_fails_before_body_but_hidden_notes_still_evaluate_content() {
    for source in [
        ".speakernote\n    .grid columns:{0}\n",
        ".doctype {paged}\n.speakernote\n    .grid columns:{0}\n",
    ] {
        let result = compile_source(source);
        assert_eq!(result.diagnostics.len(), 1, "{source}: {result:?}");
        assert!(result.diagnostics[0]
            .message
            .contains("only valid for slides documents"));
        assert!(!result.diagnostics[0].message.contains("Column count"));
        assert!(result.ir.nodes.is_empty(), "{source}: {result:?}");
    }

    let hidden = compile_source(".doctype {slides}\n.speakernote\n    .grid columns:{0}\n");
    assert_eq!(hidden.diagnostics.len(), 1, "{hidden:?}");
    assert!(hidden.diagnostics[0].message.contains("Column count"));
    assert!(hidden.ir.nodes.is_empty(), "{hidden:?}");
}

#[test]
fn speaker_note_rejects_missing_and_inline_materialization() {
    let missing = compile_source(".doctype {slides}\n.speakernote\n");
    assert_eq!(missing.diagnostics.len(), 1, "{missing:?}");
    assert!(missing.ir.nodes.is_empty(), "{missing:?}");

    let inline = compile_source(".doctype {slides}\nBefore .speakernote {A} after.\n");
    assert_eq!(inline.diagnostics.len(), 1, "{inline:?}");
    assert!(inline.diagnostics[0].message.contains("block-only"));
    assert!(speaker_notes(&inline).is_empty());

    let duplicate =
        compile_source(".doctype {slides}\n.speakernote {argument}\n    indented body\n");
    assert_eq!(duplicate.diagnostics.len(), 1, "{duplicate:?}");
    assert!(duplicate.diagnostics[0]
        .message
        .contains("received argument `content` more than once"));
    assert!(duplicate.ir.nodes.is_empty(), "{duplicate:?}");
}

#[test]
fn source_defined_speaker_note_keeps_precedence() {
    let result = compile_source(".function {speakernote}\n    custom\n\n.speakernote {authored}\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert!(speaker_notes(&result).is_empty());
    assert!(format!("{:?}", result.ir).contains("custom"));
}

#[test]
fn speaker_note_component_roundtrips_through_document_wire_serde() {
    let result =
        compile_source(".doctype {slides}\n.speakernote\n    ## Cue\n\n    Remember **this**.\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let json = serde_json::to_string(&result.ir).expect("serialize SpeakerNote IR");
    assert!(json.contains("SpeakerNote"), "{json}");
    let roundtrip: arkst_core::ir::IrDocument =
        serde_json::from_str(&json).expect("deserialize SpeakerNote IR");
    assert_eq!(roundtrip, result.ir);
}
