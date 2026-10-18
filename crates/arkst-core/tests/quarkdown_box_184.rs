use arkst_core::ir::{
    IrBoxComponent, IrBoxType, IrColor, IrComponent, IrInline, IrNode, IrSizeUnit,
};
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

fn box_component(result: &arkst_core::CompileResult) -> &IrBoxComponent {
    let [IrNode::Component {
        component: IrComponent::Box(component),
    }] = result.ir.nodes.as_slice()
    else {
        panic!("expected one Box component, got {:?}", result.ir.nodes);
    };
    component
}

#[test]
fn box_default_callout_preserves_structured_markdown_body() {
    let result = compile_source(".box {Hello, **world**!}\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let component = box_component(&result);
    assert_eq!(component.box_type, IrBoxType::Callout);
    assert!(component.title.is_none());
    assert!(component.padding.is_none());
    assert!(component.background.is_none());
    assert!(component.foreground.is_none());
    let [IrNode::Paragraph { content, .. }] = component.children.as_slice() else {
        panic!("expected one paragraph child, got {:?}", component.children);
    };
    assert!(matches!(
        content.as_slice(),
        [
            IrInline::Text { content: prefix, .. },
            IrInline::Strong { .. },
            IrInline::Text { content: suffix, .. }
        ] if prefix == "Hello, " && suffix == "!"
    ));
    assert_eq!(component.span.start, 0);
}

#[test]
fn box_preserves_the_closed_type_domain_and_rejects_invalid_values() {
    for (name, expected) in [
        ("callout", IrBoxType::Callout),
        ("tip", IrBoxType::Tip),
        ("note", IrBoxType::Note),
        ("warning", IrBoxType::Warning),
        ("error", IrBoxType::Error),
        ("WARNING", IrBoxType::Warning),
    ] {
        let source = format!(".box type:{{{name}}}\n    Body\n");
        let result = compile_source(&source);
        assert!(result.diagnostics.is_empty(), "{source:?}: {result:?}");
        assert_eq!(box_component(&result).box_type, expected, "{source:?}");
    }

    let static_text = compile_source(".box type:{.string {note}}\n    Body\n");
    assert_eq!(static_text.diagnostics.len(), 1, "{static_text:?}");
    assert!(static_text.ir.nodes.is_empty(), "{static_text:?}");

    let invalid_source = ".box type:{urgent}\n    Body\n";
    let invalid = compile_source(invalid_source);
    assert_eq!(invalid.diagnostics.len(), 1, "{invalid:?}");
    assert!(invalid.ir.nodes.is_empty(), "{invalid:?}");
    let primary = invalid.diagnostics[0].primary.expect("invalid type span");
    assert_eq!(
        invalid_source.get(primary.start..primary.end),
        Some("type:{urgent}")
    );
    assert!(invalid.diagnostics[0]
        .message
        .contains("callout, tip, note, warning, or error"));
}

#[test]
fn box_indented_body_preserves_block_structure() {
    let result = compile_source(".box\n    ## Callout\n\n    Read *this*.\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let component = box_component(&result);
    assert!(matches!(
        component.children.as_slice(),
        [IrNode::Heading { level: 2, .. }, IrNode::Paragraph { .. }]
    ));
    let IrNode::Paragraph { content, .. } = &component.children[1] else {
        unreachable!();
    };
    assert!(content
        .iter()
        .any(|inline| matches!(inline, IrInline::Emphasis { .. })));
}

#[test]
fn box_preserves_positional_and_named_rich_titles_with_provenance() {
    for source in [
        ".box {Read **this**}\n    Body *copy*.\n",
        ".box title:{Read **this**}\n    Body *copy*.\n",
    ] {
        let (result, source_id) = compile_source_with_id(source);
        assert!(result.diagnostics.is_empty(), "{source:?}: {result:?}");
        let component = box_component(&result);
        let Some(title) = &component.title else {
            panic!("expected authored title: {component:?}");
        };
        assert!(matches!(
            title.as_slice(),
            [IrInline::Text { content: prefix, span }, IrInline::Strong { .. }]
                if prefix == "Read "
                    && span.source_id == source_id
                    && source.get(span.start..span.end) == Some("Read ")
        ));
        assert!(matches!(
            component.children.as_slice(),
            [IrNode::Paragraph { content, .. }]
                if content.iter().any(|inline| matches!(inline, IrInline::Emphasis { .. }))
        ));
        assert_eq!(component.span.source_id, source_id);
        assert_eq!(
            source.get(component.span.start..component.span.end),
            Some(source.trim_end())
        );
    }
}

#[test]
fn explicit_none_is_untitled_and_authored_title_bypasses_automatic_localization() {
    let none = compile_source(".box title:{.none}\n    A\n");
    assert!(none.diagnostics.is_empty(), "{none:?}");
    assert!(box_component(&none).title.is_none());

    let authored = compile_source(".doclang {english}\n.box {Heads}\n    A\n");
    assert!(authored.diagnostics.is_empty(), "{authored:?}");
    assert!(box_component(&authored).title.is_some());
}

#[test]
fn box_preserves_typed_presentation_overrides_and_explicit_none_identity() {
    let source = ".box title:{Heads} type:{warning} padding:{1cm} background:{purple} foreground:{rgba(1,2,3,0.5)}\n    Body *copy*.\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let component = box_component(&result);
    assert_eq!(component.box_type, IrBoxType::Warning);
    assert!(component.title.is_some());
    let padding = component.padding.as_deref().expect("typed padding");
    assert_eq!(padding.value, 1.0);
    assert_eq!(padding.unit, IrSizeUnit::Cm);
    assert_eq!(
        component.background.as_deref(),
        Some(&IrColor {
            red: 128,
            green: 0,
            blue: 128,
            alpha: 1.0,
        })
    );
    assert_eq!(
        component.foreground.as_deref(),
        Some(&IrColor {
            red: 1,
            green: 2,
            blue: 3,
            alpha: 0.5,
        })
    );
    assert!(matches!(
        component.children.as_slice(),
        [IrNode::Paragraph { content, .. }]
            if content.iter().any(|inline| matches!(inline, IrInline::Emphasis { .. }))
    ));

    let none =
        compile_source(".box padding:{.none} background:{.none} foreground:{.none}\n    A\n");
    assert!(none.diagnostics.is_empty(), "{none:?}");
    let component = box_component(&none);
    assert!(component.padding.is_none());
    assert!(component.background.is_none());
    assert!(component.foreground.is_none());
}

#[test]
fn box_fails_closed_for_automatic_localization_and_invalid_presentation_inputs() {
    let localized = compile_source(".doclang {english}\n.box {A}\n");
    assert_eq!(localized.diagnostics.len(), 1, "{localized:?}");
    assert!(localized.ir.nodes.is_empty(), "{localized:?}");
    assert!(localized.diagnostics[0]
        .message
        .contains("automatic localized titles are not yet implemented"));

    for source in [
        ".box padding:{wide}\n    A\n",
        ".box background:{not-a-color}\n    A\n",
        ".box foreground:{not-a-color}\n    A\n",
        ".box padding:{.string {1cm}}\n    A\n",
        ".box foreground:{.string {red}}\n    A\n",
        ".box padding:{1cm} padding:{2cm}\n    A\n",
        ".box {Title} {warning}\n    A\n",
        ".box title:{.range {1} {2}}\n    A\n",
        ".box {One} title:{Two}\n    A\n",
        ".box title:{Heads}\n",
    ] {
        let result = compile_source(source);
        assert!(!result.diagnostics.is_empty(), "{source:?}: {result:?}");
        assert!(result.ir.nodes.is_empty(), "{source:?}: {result:?}");
    }
}

#[test]
fn box_body_conversion_diagnostic_uses_final_body_parameter_provenance() {
    let source = ".box type:{warning} body:{.range {1} {2}}\n";
    let result = compile_source(source);
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert!(result.ir.nodes.is_empty(), "{result:?}");
    let diagnostic = &result.diagnostics[0];
    assert!(diagnostic.message.contains("`.box` body"), "{diagnostic:?}");
    let primary = diagnostic.primary.expect("body diagnostic span");
    assert_eq!(
        source.get(primary.start..primary.end),
        Some("body:{.range {1} {2}}")
    );
}

#[test]
fn box_rejects_missing_or_inline_materialization_without_fabricated_output() {
    let missing = compile_source(".box\n");
    assert_eq!(missing.diagnostics.len(), 1, "{missing:?}");
    assert!(missing.ir.nodes.is_empty(), "{missing:?}");

    let inline = compile_source("before .box {A} after\n");
    assert_eq!(inline.diagnostics.len(), 1, "{inline:?}");
    let [IrNode::Paragraph { content, .. }] = inline.ir.nodes.as_slice() else {
        panic!("expected surrounding paragraph, got {:?}", inline.ir.nodes);
    };
    let text: String = content
        .iter()
        .filter_map(|inline| match inline {
            IrInline::Text { content, .. } => Some(content.as_str()),
            _ => None,
        })
        .collect();
    assert!(text.contains("before"));
    assert!(text.contains("after"));
}

#[test]
fn box_component_roundtrips_through_document_wire_serde() {
    let result = compile_source(
        ".box {Release **note**} type:{warning} padding:{1cm} background:{purple} foreground:{rgba(1,2,3,0.5)}\n    Ship it\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert!(box_component(&result).title.is_some());
    assert_eq!(box_component(&result).box_type, IrBoxType::Warning);
    assert!(box_component(&result).padding.is_some());
    assert!(box_component(&result).background.is_some());
    assert!(box_component(&result).foreground.is_some());
    let json = serde_json::to_string(&result.ir).expect("serialize Box IR");
    assert!(json.contains("\"box_type\":\"Warning\""), "{json}");
    assert!(json.contains("\"padding\""), "{json}");
    assert!(json.contains("\"background\""), "{json}");
    assert!(json.contains("\"foreground\""), "{json}");
    let roundtrip: arkst_core::ir::IrDocument =
        serde_json::from_str(&json).expect("deserialize Box IR");
    assert_eq!(roundtrip, result.ir);

    let legacy = compile_source(".box {Ship it}\n");
    assert!(legacy.diagnostics.is_empty(), "{legacy:?}");
    let legacy_json = serde_json::to_string(&legacy.ir).expect("serialize default Box IR");
    assert!(!legacy_json.contains("box_type"), "{legacy_json}");
    let legacy_roundtrip: arkst_core::ir::IrDocument =
        serde_json::from_str(&legacy_json).expect("deserialize legacy-shape Box IR");
    let [IrNode::Component {
        component: IrComponent::Box(legacy_component),
    }] = legacy_roundtrip.nodes.as_slice()
    else {
        panic!(
            "expected legacy Box component, got {:?}",
            legacy_roundtrip.nodes
        );
    };
    assert_eq!(legacy_component.box_type, IrBoxType::Callout);
    assert!(legacy_component.padding.is_none());
    assert!(legacy_component.background.is_none());
    assert!(legacy_component.foreground.is_none());
    assert!(!legacy_json.contains("\"padding\""), "{legacy_json}");
    assert!(!legacy_json.contains("\"background\""), "{legacy_json}");
    assert!(!legacy_json.contains("\"foreground\""), "{legacy_json}");
}
