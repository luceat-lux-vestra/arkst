from pathlib import Path

path = Path(".github/scripts/apply_336.py")
text = path.read_text()


def replace_once(old: str, new: str, label: str) -> None:
    global text
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one occurrence, found {count}")
    text = text.replace(old, new, 1)


replace_once(
    '''replace_once(
    "crates/arkst-core/tests/quarkdown_stacked_layout.rs",
    "    assert_eq!(column.main_axis_alignment, IrMainAxisAlignment::Start);",
    "    assert_eq!(column.main_axis_alignment, None);",
)
''',
    '''replace_once(
    "crates/arkst-core/tests/quarkdown_stacked_layout.rs",
    """    assert_eq!(column.layout, IrStackedLayout::Column);\\n    assert_eq!(column.main_axis_alignment, IrMainAxisAlignment::Start);\\n    assert_eq!(column.cross_axis_alignment, IrCrossAxisAlignment::Center);""",
    """    assert_eq!(column.layout, IrStackedLayout::Column);\\n    assert_eq!(column.main_axis_alignment, None);\\n    assert_eq!(column.cross_axis_alignment, IrCrossAxisAlignment::Center);""",
)
''',
    "narrow column assertion",
)

anchor = '''replace_once(
    "crates/arkst-engine/src/evaluator.rs",
'''
wire_patch = r'''replace_once(
    "crates/arkst-ir/src/lib.rs",
    """struct WireStackedComponent {
    layout: IrStackedLayout,
    main_axis_alignment: IrMainAxisAlignment,
    cross_axis_alignment: IrCrossAxisAlignment,""",
    """struct WireStackedComponent {
    layout: IrStackedLayout,
    // Keep the legacy concrete field on the wire so older readers can still
    // decode newly serialized documents. The additive flag carries the new
    // omission provenance and is ignored by legacy serde readers.
    main_axis_alignment: IrMainAxisAlignment,
    #[serde(default, skip_serializing_if = \"is_false\")]
    main_axis_alignment_inherited: bool,
    cross_axis_alignment: IrCrossAxisAlignment,""",
)
replace_once(
    "crates/arkst-ir/src/lib.rs",
    """#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct WireStackedComponent {""",
    """fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct WireStackedComponent {""",
)
replace_once(
    "crates/arkst-ir/src/lib.rs",
    """        IrComponent::Stacked(component) => WireComponent::Stacked(WireStackedComponent {
            layout: component.layout.clone(),
            main_axis_alignment: component.main_axis_alignment,
            cross_axis_alignment: component.cross_axis_alignment,""",
    """        IrComponent::Stacked(component) => {
            let inherited_main_axis = component.main_axis_alignment.is_none();
            let legacy_main_axis = component.main_axis_alignment.unwrap_or(match component.layout {
                IrStackedLayout::Grid { .. } => IrMainAxisAlignment::Center,
                IrStackedLayout::Row | IrStackedLayout::Column => IrMainAxisAlignment::Start,
            });
            WireComponent::Stacked(WireStackedComponent {
                layout: component.layout.clone(),
                main_axis_alignment: legacy_main_axis,
                main_axis_alignment_inherited: inherited_main_axis,
                cross_axis_alignment: component.cross_axis_alignment,""",
)
replace_once(
    "crates/arkst-ir/src/lib.rs",
    """            children: wire_nodes(&component.children, sources)?,
            span: component.span,
        }),
        IrComponent::Container(component) =>""",
    """                children: wire_nodes(&component.children, sources)?,
                span: component.span,
            })
        }
        IrComponent::Container(component) =>""",
)
replace_once(
    "crates/arkst-ir/src/lib.rs",
    """        WireComponent::Stacked(component) => IrComponent::Stacked(IrStackedComponent {
            layout: component.layout,
            main_axis_alignment: component.main_axis_alignment,
            cross_axis_alignment: component.cross_axis_alignment,""",
    """        WireComponent::Stacked(component) => IrComponent::Stacked(IrStackedComponent {
            layout: component.layout,
            main_axis_alignment: (!component.main_axis_alignment_inherited)
                .then_some(component.main_axis_alignment),
            cross_axis_alignment: component.cross_axis_alignment,""",
)
replace_once(
    "crates/arkst-ir/src/lib.rs",
    """    #[test]
    fn grid_layout_rejects_zero_columns_during_deserialization() {""",
    """    #[test]
    fn document_wire_keeps_legacy_stack_alignment_field_with_additive_inheritance_flag() {
        #[derive(serde::Deserialize)]
        struct LegacyStackShape {
            main_axis_alignment: IrMainAxisAlignment,
        }

        let span = SourceSpan::new(SourceId(12), 0, 8);
        let document = IrDocument {
            nodes: vec![IrNode::Component {
                component: IrComponent::Stacked(IrStackedComponent {
                    layout: IrStackedLayout::Row,
                    main_axis_alignment: None,
                    cross_axis_alignment: IrCrossAxisAlignment::Center,
                    row_gap: None,
                    column_gap: None,
                    children: Vec::new(),
                    span,
                }),
            }],
            metadata: IrMetadata::default(),
        };

        let encoded = serde_json::to_value(&document).expect(\"document wire serializes\");
        let stacked = &encoded[\"nodes\"][0][\"Component\"][\"component\"][\"Stacked\"];
        assert_eq!(stacked.get(\"main_axis_alignment\"), Some(&serde_json::json!(\"Start\")));
        assert_eq!(
            stacked.get(\"main_axis_alignment_inherited\"),
            Some(&serde_json::json!(true))
        );
        let legacy: LegacyStackShape = serde_json::from_value(stacked.clone())
            .expect(\"legacy-shaped reader ignores additive inheritance flag\");
        assert_eq!(legacy.main_axis_alignment, IrMainAxisAlignment::Start);
        assert_eq!(
            serde_json::from_value::<IrDocument>(encoded).expect(\"new document wire round trips\"),
            document
        );

        let explicit = IrDocument {
            nodes: vec![IrNode::Component {
                component: IrComponent::Stacked(IrStackedComponent {
                    layout: IrStackedLayout::Row,
                    main_axis_alignment: Some(IrMainAxisAlignment::Start),
                    cross_axis_alignment: IrCrossAxisAlignment::Center,
                    row_gap: None,
                    column_gap: None,
                    children: Vec::new(),
                    span,
                }),
            }],
            metadata: IrMetadata::default(),
        };
        let explicit_json = serde_json::to_value(&explicit).expect(\"explicit document serializes\");
        let explicit_stack = &explicit_json[\"nodes\"][0][\"Component\"][\"component\"][\"Stacked\"];
        assert!(explicit_stack.get(\"main_axis_alignment_inherited\").is_none());
        assert_eq!(
            serde_json::from_value::<IrDocument>(explicit_json)
                .expect(\"legacy explicit document remains readable\"),
            explicit
        );
    }

    #[test]
    fn grid_layout_rejects_zero_columns_during_deserialization() {""",
)

'''
count = text.count(anchor)
if count != 2:
    raise SystemExit(f"wire patch anchor: expected two evaluator patch anchors, found {count}")
text = text.replace(anchor, wire_patch + anchor, 1)
path.write_text(text)
