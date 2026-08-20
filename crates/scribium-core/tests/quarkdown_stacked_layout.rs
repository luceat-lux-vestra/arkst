use scribium_core::ir::{
    IrCrossAxisAlignment, IrMainAxisAlignment, IrNode, IrSize, IrSizeUnit, IrStackedLayout,
};
use scribium_core::{compile, CompileOptions, VirtualProjectBuilder};

fn compile_source(source: &str) -> scribium_core::CompileResult {
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("valid entry")
        .add_source("main.qd", source)
        .expect("valid source")
        .build()
        .expect("valid project");
    compile(&project, &CompileOptions::default())
}

fn stacked(result: &scribium_core::CompileResult) -> &scribium_core::ir::IrStackedComponent {
    let [IrNode::Component {
        component: scribium_core::ir::IrComponent::Stacked(component),
    }] = result.ir.nodes.as_slice()
    else {
        panic!("expected one stacked component, got {:?}", result.ir.nodes);
    };
    component
}

#[test]
fn row_column_and_grid_defaults_are_distinct_and_typed() {
    let row = compile_source(".row\n    A\n");
    assert!(row.diagnostics.is_empty(), "{row:?}");
    let row = stacked(&row);
    assert_eq!(row.layout, IrStackedLayout::Row);
    assert_eq!(row.main_axis_alignment, IrMainAxisAlignment::Start);
    assert_eq!(row.cross_axis_alignment, IrCrossAxisAlignment::Center);
    assert_eq!(row.row_gap, None);
    assert_eq!(row.column_gap, None);

    let column = compile_source(".column\n    A\n");
    assert!(column.diagnostics.is_empty(), "{column:?}");
    let column = stacked(&column);
    assert_eq!(column.layout, IrStackedLayout::Column);
    assert_eq!(column.main_axis_alignment, IrMainAxisAlignment::Start);
    assert_eq!(column.cross_axis_alignment, IrCrossAxisAlignment::Center);

    let grid = compile_source(".grid columns:{2}\n    A\n");
    assert!(grid.diagnostics.is_empty(), "{grid:?}");
    let grid = stacked(&grid);
    assert_eq!(
        grid.layout,
        IrStackedLayout::Grid {
            columns: 2.try_into().unwrap()
        }
    );
    assert_eq!(grid.main_axis_alignment, IrMainAxisAlignment::Center);
    assert_eq!(grid.cross_axis_alignment, IrCrossAxisAlignment::Center);
    assert_eq!(grid.row_gap, None);
    assert_eq!(grid.column_gap, None);
}

#[test]
fn row_column_and_grid_bind_typed_arguments_and_preserve_children() {
    let source = ".row alignment:{spacebetween} cross:{start} gap:{10px}\n    A\n\n    B\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let row = stacked(&result);
    assert_eq!(row.main_axis_alignment, IrMainAxisAlignment::SpaceBetween);
    assert_eq!(row.cross_axis_alignment, IrCrossAxisAlignment::Start);
    assert_eq!(
        row.column_gap,
        Some(IrSize {
            value: 10.0,
            unit: IrSizeUnit::Px,
        })
    );
    assert_eq!(row.children.len(), 2);
    assert!(matches!(row.children[0], IrNode::Paragraph { .. }));
    assert!(matches!(row.children[1], IrNode::Paragraph { .. }));

    let source = ".column {start} {stretch} {1cm}\n    A\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let column = stacked(&result);
    assert_eq!(column.main_axis_alignment, IrMainAxisAlignment::Start);
    assert_eq!(column.cross_axis_alignment, IrCrossAxisAlignment::Stretch);
    assert_eq!(
        column.row_gap.as_ref().map(|gap| gap.unit),
        Some(IrSizeUnit::Cm)
    );

    let source = ".grid columns:{2} gap:{1cm} vgap:{2cm} hgap:{3cm}\n    A\n\n    B\n\n    C\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let grid = stacked(&result);
    assert_eq!(grid.row_gap.as_ref().map(|gap| gap.value), Some(2.0));
    assert_eq!(grid.column_gap.as_ref().map(|gap| gap.value), Some(3.0));
    assert_eq!(grid.children.len(), 3);
}

#[test]
fn grid_gap_precedence_and_none_are_source_consumers() {
    for (source, row_gap, column_gap) in [
        (".grid columns:{2} gap:{1cm}\n    A\n", 1.0, 1.0),
        (".grid columns:{2} gap:{1cm} hgap:{2cm}\n    A\n", 1.0, 2.0),
        (".grid columns:{2} gap:{1cm} vgap:{2cm}\n    A\n", 2.0, 1.0),
        (
            ".grid columns:{2} gap:{1cm} vgap:{2cm} hgap:{3cm}\n    A\n",
            2.0,
            3.0,
        ),
        (
            ".grid columns:{2} gap:{1cm} vgap:{.none} hgap:{.none}\n    A\n",
            1.0,
            1.0,
        ),
    ] {
        let result = compile_source(source);
        assert!(result.diagnostics.is_empty(), "{source:?}: {result:?}");
        let grid = stacked(&result);
        assert_eq!(grid.row_gap.as_ref().map(|gap| gap.value), Some(row_gap));
        assert_eq!(
            grid.column_gap.as_ref().map(|gap| gap.value),
            Some(column_gap)
        );
    }
}

#[test]
fn alignments_are_closed_case_insensitive_domains_without_underscore_normalization() {
    for alignment in [
        "start",
        "center",
        "end",
        "spacebetween",
        "SPACEBETWEEN",
        "SpaceBetween",
    ] {
        let source = format!(".row alignment:{{{alignment}}}\n    A\n");
        let result = compile_source(&source);
        assert!(result.diagnostics.is_empty(), "{source:?}: {result:?}");
    }
    for cross in ["start", "center", "end", "stretch"] {
        let source = format!(".column cross:{{{cross}}}\n    A\n");
        let result = compile_source(&source);
        assert!(result.diagnostics.is_empty(), "{source:?}: {result:?}");
    }
    for invalid in ["space_between", "space-between", "between"] {
        let source = format!(".row alignment:{{{invalid}}}\n    A\n");
        let result = compile_source(&source);
        assert_eq!(result.diagnostics.len(), 1, "{source:?}: {result:?}");
        assert!(result.ir.nodes.is_empty());
    }
    let invalid_cross = compile_source(".row cross:{spacebetween}\n    A\n");
    assert_eq!(invalid_cross.diagnostics.len(), 1);
    assert!(invalid_cross.ir.nodes.is_empty());
}

#[test]
fn integer_boundary_is_integral_positive_and_origin_aware() {
    for columns in ["1", "2", "3", "2.0"] {
        let source = format!(".grid columns:{{{columns}}}\n    A\n");
        let result = compile_source(&source);
        assert!(result.diagnostics.is_empty(), "{source:?}: {result:?}");
    }
    for columns in ["0", "-1", "2.5"] {
        let source = format!(".grid columns:{{{columns}}}\n    A\n");
        let result = compile_source(&source);
        assert_eq!(result.diagnostics.len(), 1, "{source:?}: {result:?}");
        assert!(result.ir.nodes.is_empty());
    }
    let static_text = compile_source(".grid columns:{.string {2}}\n    A\n");
    assert_eq!(static_text.diagnostics.len(), 1);
    assert!(static_text.ir.nodes.is_empty());
}

#[test]
fn duplicate_unknown_missing_and_invalid_arguments_fail_before_body() {
    for source in [
        ".grid {2} columns:{3}\n    A\n",
        ".row {start} alignment:{end}\n    A\n",
        ".row foo:{bar}\n    A\n",
        ".grid columns:{2} horizontalgap:{1cm}\n    A\n",
        ".row\n",
        ".column alignment:{center}\n",
        ".grid columns:{2}\n",
        ".row alignment:{.string {center}}\n    A\n",
        ".row gap:{wat}\n    A\n",
    ] {
        let result = compile_source(source);
        assert_eq!(result.diagnostics.len(), 1, "{source:?}: {result:?}");
        assert!(
            result.ir.nodes.is_empty(),
            "{source:?}: {:?}",
            result.ir.nodes
        );
    }
}

#[test]
fn body_is_lazy_and_nested_failures_do_not_publish_outer_components() {
    let result =
        compile_source(".docname {before}\n.grid columns:{0}\n    .docname {after}\n.docname\n");
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert_eq!(result.ir.nodes.len(), 1);
    let IrNode::Paragraph { content, .. } = &result.ir.nodes[0] else {
        panic!("expected document-state read");
    };
    assert!(
        matches!(content.first(), Some(scribium_core::ir::IrInline::Text { content, .. }) if content == "before")
    );

    let result = compile_source(".row\n    A\n\n    .grid columns:{0}\n        B\n\n    C\n");
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert!(result.ir.nodes.is_empty());
}

#[test]
fn component_values_compose_in_functions_and_custom_row_shadows_native_row() {
    let result = compile_source(".function {layout}\n    .row\n        A\n\n.layout\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert!(matches!(
        result.ir.nodes.as_slice(),
        [IrNode::Component { .. }]
    ));

    let result = compile_source(".function {row}\n    shadowed\n\n.row\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert!(matches!(
        result.ir.nodes.as_slice(),
        [IrNode::Paragraph { .. }]
    ));
}

#[test]
fn inline_stacked_calls_fail_without_fabricated_text() {
    let result = compile_source("prefix .row suffix\n");
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    let [IrNode::Paragraph { content, .. }] = result.ir.nodes.as_slice() else {
        panic!(
            "expected surrounding paragraph to remain, got {:?}",
            result.ir.nodes
        );
    };
    assert!(content
        .iter()
        .all(|inline| { matches!(inline, scribium_core::ir::IrInline::Text { .. }) }));
    assert!(!format!("{content:?}").contains("row"));
}
