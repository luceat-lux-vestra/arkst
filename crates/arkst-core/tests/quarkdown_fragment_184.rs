use arkst_core::evaluator::Evaluator;
use arkst_core::ir::{
    IrComponent, IrDocumentType, IrEnumValue, IrFragmentBehavior, IrFragmentComponent, IrInline,
    IrNode, IrParameter, IrValue,
};
use arkst_core::{compile, CompileOptions, SourceId, SourceSpan, VirtualProjectBuilder};

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

fn fragments(result: &arkst_core::CompileResult) -> Vec<&IrFragmentComponent> {
    result
        .ir
        .nodes
        .iter()
        .filter_map(|node| match node {
            IrNode::Component {
                component: IrComponent::Fragment(component),
            } => Some(component),
            _ => None,
        })
        .collect()
}

#[test]
fn fragment_preserves_closed_behaviors_structured_content_and_provenance() {
    let source = ".doctype {slides}\n.fragment\n    Default **show**.\n.fragment {hide}\n    Hide later.\n.fragment behavior:{semihide}\n    Fade halfway.\n.fragment {showhide}\n    Show then hide.\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let fragments = fragments(&result);
    assert_eq!(fragments.len(), 4, "{:?}", result.ir.nodes);
    assert_eq!(fragments[0].behavior, IrFragmentBehavior::Show);
    assert_eq!(fragments[1].behavior, IrFragmentBehavior::Hide);
    assert_eq!(fragments[2].behavior, IrFragmentBehavior::SemiHide);
    assert_eq!(fragments[3].behavior, IrFragmentBehavior::ShowHide);

    let [IrNode::Paragraph { content, span }] = fragments[0].children.as_slice() else {
        panic!("expected structured fragment paragraph")
    };
    assert!(content
        .iter()
        .any(|inline| matches!(inline, IrInline::Strong { .. })));
    assert_eq!(source.get(span.start..span.end), Some("Default **show**."));
    assert_eq!(
        source.get(fragments[0].span.start..fragments[0].span.end),
        Some(".fragment\n    Default **show**.")
    );
}

#[test]
fn fragment_gate_and_behavior_conversion_precede_body_evaluation() {
    for source in [
        ".fragment\n    .grid columns:{0}\n",
        ".doctype {paged}\n.fragment\n    .grid columns:{0}\n",
    ] {
        let result = compile_source(source);
        assert_eq!(result.diagnostics.len(), 1, "{source}: {result:?}");
        assert!(result.diagnostics[0]
            .message
            .contains("only valid for slides documents"));
        assert!(!result.diagnostics[0].message.contains("Column count"));
        assert!(result.ir.nodes.is_empty(), "{source}: {result:?}");
    }

    let invalid =
        compile_source(".doctype {slides}\n.fragment {unexpected}\n    .grid columns:{0}\n");
    assert_eq!(invalid.diagnostics.len(), 1, "{invalid:?}");
    assert!(invalid.diagnostics[0].message.contains("behavior"));
    assert!(invalid.diagnostics[0].message.contains("showhide"));
    assert!(!invalid.diagnostics[0].message.contains("Column count"));
    assert!(invalid.ir.nodes.is_empty(), "{invalid:?}");
}

#[test]
fn fragment_rejects_invalid_shapes_before_publication() {
    let missing = compile_source(".doctype {slides}\n.fragment\n");
    assert_eq!(missing.diagnostics.len(), 1, "{missing:?}");
    assert!(missing.ir.nodes.is_empty(), "{missing:?}");

    let inline =
        compile_source(".doctype {slides}\nBefore .fragment {show} content:{Body} after.\n");
    assert_eq!(inline.diagnostics.len(), 1, "{inline:?}");
    assert!(inline.diagnostics[0].message.contains("block-only"));
    assert!(fragments(&inline).is_empty());

    let duplicate =
        compile_source(".doctype {slides}\n.fragment content:{argument}\n    indented body\n");
    assert_eq!(duplicate.diagnostics.len(), 1, "{duplicate:?}");
    assert!(duplicate.diagnostics[0]
        .message
        .contains("received argument `content` more than once"));
    assert!(duplicate.ir.nodes.is_empty(), "{duplicate:?}");
}

#[test]
fn fragment_rejects_lambda_body_without_evaluating_it() {
    let span = SourceSpan::new(SourceId(1), 0, 5);
    let mut metadata = arkst_core::ir::IrMetadata::default();
    metadata.document_state.document_type = IrDocumentType::Slides;
    let document = arkst_core::ir::IrDocument {
        nodes: vec![IrNode::FunctionCall {
            name: "fragment".to_string(),
            positional_args: vec![IrValue::Enum(IrEnumValue::FragmentBehavior(
                IrFragmentBehavior::Show,
            ))],
            named_args: Vec::new(),
            ordered_args: None,
            lambda_parameters: Some(vec![IrParameter {
                name: "value".to_string(),
                name_span: span,
                span,
                optional: false,
            }]),
            body: Some(vec![IrNode::FunctionCall {
                name: "grid".to_string(),
                positional_args: Vec::new(),
                named_args: vec![arkst_core::ir::IrNamedArg {
                    name: "columns".to_string(),
                    name_span: span,
                    value: IrValue::Number(0.0),
                    span,
                }],
                ordered_args: None,
                lambda_parameters: None,
                body: None,
                raw_body: None,
                span,
            }]),
            raw_body: None,
            span,
        }],
        metadata,
    };

    let (evaluated, diagnostics) = Evaluator::new().evaluate(&document);
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert!(diagnostics[0].message.contains("not a lambda"));
    assert!(!diagnostics[0].message.contains("Column count"));
    assert!(evaluated.nodes.is_empty());
}

#[test]
fn source_defined_fragment_keeps_precedence() {
    let result = compile_source(".function {fragment}\n    custom\n\n.fragment {authored}\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert!(fragments(&result).is_empty());
    assert!(format!("{:?}", result.ir).contains("custom"));
}

#[test]
fn fragment_component_roundtrips_through_document_wire_serde() {
    let result = compile_source(
        ".doctype {slides}\n.fragment\n    Default show.\n.fragment behavior:{semihide}\n    ## Cue\n\n    Remember **this**.\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let json = serde_json::to_string(&result.ir).expect("serialize Fragment IR");
    assert!(json.contains("Fragment"), "{json}");
    assert!(json.contains("SemiHide"), "{json}");
    assert!(!json.contains("\"behavior\":\"Show\""), "{json}");
    let roundtrip: arkst_core::ir::IrDocument =
        serde_json::from_str(&json).expect("deserialize Fragment IR");
    assert_eq!(roundtrip, result.ir);
}
