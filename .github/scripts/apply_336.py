from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    target = Path(path)
    text = target.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(
            f"{path}: expected exactly one replacement, found {count}: {old[:120]!r}"
        )
    target.write_text(text.replace(old, new, 1))


def replace_all_checked(path: str, old: str, new: str, expected: int) -> None:
    target = Path(path)
    text = target.read_text()
    count = text.count(old)
    if count != expected:
        raise SystemExit(f"{path}: expected {expected} replacements, found {count}: {old!r}")
    target.write_text(text.replace(old, new))


replace_once(
    "crates/arkst-ir/src/lib.rs",
    "    pub main_axis_alignment: IrMainAxisAlignment,\n    pub cross_axis_alignment: IrCrossAxisAlignment,",
    """    /// Explicit main-axis alignment. `None` means a row/column omitted the
    /// argument and must inherit final document-global alignment at output time.
    /// Grid evaluation always publishes an explicit/defaulted value.
    #[serde(default, skip_serializing_if = \"Option::is_none\")]
    pub main_axis_alignment: Option<IrMainAxisAlignment>,
    pub cross_axis_alignment: IrCrossAxisAlignment,""",
)
replace_once(
    "crates/arkst-ir/src/lib.rs",
    """            IrStackedLayout::Row => (
                IrMainAxisAlignment::Start,
                None,
                Some(IrSize {
                    value: 10.0,
                    unit: IrSizeUnit::Px,
                }),
            ),
            IrStackedLayout::Column => (
                IrMainAxisAlignment::Start,
                Some(IrSize {
                    value: 10.0,
                    unit: IrSizeUnit::Px,
                }),
                None,
            ),
            IrStackedLayout::Grid { .. } => (
                IrMainAxisAlignment::Center,
                Some(IrSize {
                    value: 8.0,
                    unit: IrSizeUnit::Px,
                }),
                Some(IrSize {
                    value: 12.0,
                    unit: IrSizeUnit::Px,
                }),
            ),""",
    """            IrStackedLayout::Row => (
                Some(IrMainAxisAlignment::Start),
                None,
                Some(IrSize {
                    value: 10.0,
                    unit: IrSizeUnit::Px,
                }),
            ),
            IrStackedLayout::Column => (
                Some(IrMainAxisAlignment::Start),
                Some(IrSize {
                    value: 10.0,
                    unit: IrSizeUnit::Px,
                }),
                None,
            ),
            IrStackedLayout::Grid { .. } => (
                Some(IrMainAxisAlignment::Center),
                Some(IrSize {
                    value: 8.0,
                    unit: IrSizeUnit::Px,
                }),
                Some(IrSize {
                    value: 12.0,
                    unit: IrSizeUnit::Px,
                }),
            ),""",
)
replace_once(
    "crates/arkst-ir/src/lib.rs",
    """    #[test]
    fn grid_layout_rejects_zero_columns_during_deserialization() {""",
    """    #[test]
    fn stacked_main_axis_serde_preserves_legacy_explicit_and_inherited_omission() {
        let span = SourceSpan::new(SourceId(11), 0, 8);
        let inherited = IrStackedComponent {
            layout: IrStackedLayout::Row,
            main_axis_alignment: None,
            cross_axis_alignment: IrCrossAxisAlignment::Center,
            row_gap: None,
            column_gap: None,
            children: Vec::new(),
            span,
        };
        let inherited_json = serde_json::to_value(&inherited).expect(\"inherited stack serializes\");
        assert!(inherited_json.get(\"main_axis_alignment\").is_none());
        assert_eq!(
            serde_json::from_value::<IrStackedComponent>(inherited_json)
                .expect(\"inherited stack round trips\")
                .main_axis_alignment,
            None
        );

        let explicit = IrStackedComponent {
            main_axis_alignment: Some(IrMainAxisAlignment::Start),
            ..inherited
        };
        let legacy_shape = serde_json::to_value(&explicit).expect(\"explicit stack serializes\");
        assert_eq!(
            legacy_shape.get(\"main_axis_alignment\"),
            Some(&serde_json::json!(\"Start\"))
        );
        assert_eq!(
            serde_json::from_value::<IrStackedComponent>(legacy_shape)
                .expect(\"old explicit wire shape remains readable\")
                .main_axis_alignment,
            Some(IrMainAxisAlignment::Start)
        );
    }

    #[test]
    fn grid_layout_rejects_zero_columns_during_deserialization() {""",
)

replace_once(
    "crates/arkst-engine/src/evaluator.rs",
    """                let alignment = bound.take(0).unwrap_or_else(|| {
                    default(IrValue::Enum(IrEnumValue::StackedMainAxisAlignment(
                        IrMainAxisAlignment::Start,
                    )))
                });
                let cross = bound.take(1).unwrap_or_else(|| {
                    default(IrValue::Enum(IrEnumValue::StackedCrossAxisAlignment(
                        IrCrossAxisAlignment::Center,
                    )))
                });
                let gap = bound.take(2).unwrap_or_else(|| default(IrValue::None));
                let main_axis = match convert_stacked_main_axis(&alignment.value) {
                    Ok(value) => value,
                    Err(error) => {
                        diagnostics.push(stacked_conversion_error(
                            name,
                            \"alignment\",
                            alignment.span,
                            alignment.parameter_span,
                            error,
                        ));
                        return CallOutcome::Failed;
                    }
                };""",
    """                let alignment = bound.take(0);
                let cross = bound.take(1).unwrap_or_else(|| {
                    default(IrValue::Enum(IrEnumValue::StackedCrossAxisAlignment(
                        IrCrossAxisAlignment::Center,
                    )))
                });
                let gap = bound.take(2).unwrap_or_else(|| default(IrValue::None));
                let main_axis = match alignment.as_ref() {
                    Some(alignment) => match convert_stacked_main_axis(&alignment.value) {
                        Ok(value) => Some(value),
                        Err(error) => {
                            diagnostics.push(stacked_conversion_error(
                                name,
                                \"alignment\",
                                alignment.span,
                                alignment.parameter_span,
                                error,
                            ));
                            return CallOutcome::Failed;
                        }
                    },
                    None => None,
                };""",
)
replace_once(
    "crates/arkst-engine/src/evaluator.rs",
    """                (
                    IrStackedLayout::Grid { columns },
                    main_axis,
                    cross_axis,
                    vgap.or_else(|| gap.clone()),
                    hgap.or(gap),
                )""",
    """                (
                    IrStackedLayout::Grid { columns },
                    Some(main_axis),
                    cross_axis,
                    vgap.or_else(|| gap.clone()),
                    hgap.or(gap),
                )""",
)
replace_all_checked(
    "crates/arkst-engine/src/value_conversion.rs",
    "            main_axis_alignment: IrMainAxisAlignment::Start,",
    "            main_axis_alignment: Some(IrMainAxisAlignment::Start),",
    2,
)

replace_once(
    "crates/arkst-core/tests/quarkdown_stacked_layout.rs",
    "    assert_eq!(row.main_axis_alignment, IrMainAxisAlignment::Start);",
    "    assert_eq!(row.main_axis_alignment, None);",
)
replace_once(
    "crates/arkst-core/tests/quarkdown_stacked_layout.rs",
    "    assert_eq!(column.main_axis_alignment, IrMainAxisAlignment::Start);",
    "    assert_eq!(column.main_axis_alignment, None);",
)
replace_once(
    "crates/arkst-core/tests/quarkdown_stacked_layout.rs",
    "    assert_eq!(grid.main_axis_alignment, IrMainAxisAlignment::Center);",
    "    assert_eq!(grid.main_axis_alignment, Some(IrMainAxisAlignment::Center));",
)
replace_once(
    "crates/arkst-core/tests/quarkdown_stacked_layout.rs",
    "    assert_eq!(row.main_axis_alignment, IrMainAxisAlignment::SpaceBetween);",
    "    assert_eq!(row.main_axis_alignment, Some(IrMainAxisAlignment::SpaceBetween));",
)
replace_once(
    "crates/arkst-core/tests/quarkdown_stacked_layout.rs",
    """    let column = stacked(&result);
    assert_eq!(column.main_axis_alignment, IrMainAxisAlignment::Start);
    assert_eq!(column.cross_axis_alignment, IrCrossAxisAlignment::Stretch);""",
    """    let column = stacked(&result);
    assert_eq!(
        column.main_axis_alignment,
        Some(IrMainAxisAlignment::Start)
    );
    assert_eq!(column.cross_axis_alignment, IrCrossAxisAlignment::Stretch);""",
)

replace_once(
    "crates/arkst-typst/src/lowering.rs",
    """    IrCallSegment, IrComponent, IrContainerAlignment, IrContainerComponent, IrCrossAxisAlignment,
    IrDocument, IrDocumentType, IrInline, IrLandscapeComponent, IrMainAxisAlignment, IrNode,""",
    """    IrCallSegment, IrComponent, IrContainerAlignment, IrContainerComponent, IrCrossAxisAlignment,
    IrDocument, IrDocumentAlignment, IrDocumentType, IrInline, IrLandscapeComponent,
    IrMainAxisAlignment, IrNode,""",
)
replace_once(
    "crates/arkst-typst/src/lowering.rs",
    """    /// Whether the raw pushed text must not receive the continuation
    /// indent (verbatim code/raw content inside a list item).
    verbatim: bool,
}""",
    """    /// Whether the raw pushed text must not receive the continuation
    /// indent (verbatim code/raw content inside a list item).
    verbatim: bool,
    /// Final document-global alignment consumed only by row/column components
    /// whose main-axis alignment remained omitted through semantic evaluation.
    inherited_stack_main_axis: IrMainAxisAlignment,
}""",
)
replace_once(
    "crates/arkst-typst/src/lowering.rs",
    """            list_item_indent: String::new(),
            at_line_start: false,
            verbatim: false,
        }""",
    """            list_item_indent: String::new(),
            at_line_start: false,
            verbatim: false,
            inherited_stack_main_axis: IrMainAxisAlignment::Start,
        }""",
)
replace_once(
    "crates/arkst-typst/src/lowering.rs",
    """    fn lower_document(&mut self, doc: &IrDocument) {
        // Emit metadata as Typst set-rules""",
    """    fn lower_document(&mut self, doc: &IrDocument) {
        self.inherited_stack_main_axis = match doc.metadata.document_state.page_alignment {
            Some(IrDocumentAlignment::Center) => IrMainAxisAlignment::Center,
            Some(IrDocumentAlignment::End) => IrMainAxisAlignment::End,
            Some(IrDocumentAlignment::Start) | Some(IrDocumentAlignment::Justify) | None => {
                IrMainAxisAlignment::Start
            }
        };

        // Emit metadata as Typst set-rules""",
)
replace_once(
    "crates/arkst-typst/src/lowering.rs",
    "        let distribution = component.main_axis_alignment;",
    """        let distribution = component
            .main_axis_alignment
            .unwrap_or(self.inherited_stack_main_axis);""",
)
replace_once(
    "crates/arkst-typst/src/lowering.rs",
    """    fn lower_grid(&mut self, component: &IrStackedComponent, columns: u32) {
        let main_alignment = match component.main_axis_alignment {""",
    """    fn lower_grid(&mut self, component: &IrStackedComponent, columns: u32) {
        let distribution = component
            .main_axis_alignment
            .unwrap_or(IrMainAxisAlignment::Center);
        let main_alignment = match distribution {""",
)
replace_once(
    "crates/arkst-typst/src/lowering.rs",
    """        let distributed = matches!(
            component.main_axis_alignment,
            IrMainAxisAlignment::SpaceBetween
                | IrMainAxisAlignment::SpaceAround
                | IrMainAxisAlignment::SpaceEvenly
        );
        if distributed {
            self.lower_distributed_grid(component, columns);""",
    """        let distributed = matches!(
            distribution,
            IrMainAxisAlignment::SpaceBetween
                | IrMainAxisAlignment::SpaceAround
                | IrMainAxisAlignment::SpaceEvenly
        );
        if distributed {
            self.lower_distributed_grid(component, columns, distribution);""",
)
replace_once(
    "crates/arkst-typst/src/lowering.rs",
    """    fn lower_distributed_grid(&mut self, component: &IrStackedComponent, columns: u32) {
        let logical_columns = columns as usize;
        let edge_fraction = match component.main_axis_alignment {""",
    """    fn lower_distributed_grid(
        &mut self,
        component: &IrStackedComponent,
        columns: u32,
        distribution: IrMainAxisAlignment,
    ) {
        let logical_columns = columns as usize;
        let edge_fraction = match distribution {""",
)
replace_once(
    "crates/arkst-typst/src/lowering.rs",
    """                layout,
                main_axis_alignment,
                cross_axis_alignment,""",
    """                layout,
                main_axis_alignment: Some(main_axis_alignment),
                cross_axis_alignment,""",
)

replace_once(
    "crates/arkst-typst-subprocess/tests/backend_integration.rs",
    """#[test]
fn integration_center_layout_lowers_to_valid_typst_and_pdf() {""",
    r'''#[test]
fn integration_v260_omitted_stack_alignment_uses_final_page_state() {
    let source = ".row\n    A\n\n    B\n\n.pageformat alignment:{center}\n.column\n    C\n\n    D\n";
    let project = VirtualProjectBuilder::new()
        .entry("v260-stack-inherit.qd")
        .expect("valid entry path")
        .add_source("v260-stack-inherit.qd", source)
        .expect("valid source path")
        .build()
        .expect("valid project");
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "v2.6 stack inheritance diagnostics: {:?}",
        result.diagnostics
    );
    let typst_code = lower_to_typst_code(&result.ir);
    assert_eq!(typst_code.matches("h(1fr)").count(), 2, "{typst_code}");
    assert_eq!(typst_code.matches("v(1fr)").count(), 2, "{typst_code}");

    with_typst("v260-stack-alignment-inheritance", |backend| {
        let output = backend
            .compile(&TypstInput {
                source: typst_code,
                entry_path: "v260-stack-inherit.qd".to_string(),
            })
            .expect("inherited stack Typst must compile");
        assert!(output
            .pdf
            .expect("PDF output must be present")
            .starts_with(b"%PDF-"));
    });
}

#[test]
fn integration_center_layout_lowers_to_valid_typst_and_pdf() {''',
)
