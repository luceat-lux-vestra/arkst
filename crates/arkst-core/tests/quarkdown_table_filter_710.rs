use arkst_core::ir::{IrInline, IrNode, IrTableRow};
use arkst_core::{compile, CompileOptions, EvaluationLimits, VirtualProjectBuilder};

fn compile_source(source: &str) -> arkst_core::CompileResult {
    compile_source_with_options(source, &CompileOptions::default())
}

fn compile_source_with_options(
    source: &str,
    options: &CompileOptions,
) -> arkst_core::CompileResult {
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("valid entry")
        .add_source("main.qd", source)
        .expect("valid source")
        .build()
        .expect("valid project");
    compile(&project, options)
}

fn inline_text(inlines: &[IrInline]) -> String {
    let mut output = String::new();
    for inline in inlines {
        match inline {
            IrInline::Text { content, .. } | IrInline::Code { content, .. } => {
                output.push_str(content);
            }
            IrInline::Emphasis { content, .. }
            | IrInline::Strong { content, .. }
            | IrInline::Strikethrough { content, .. }
            | IrInline::Link { content, .. } => output.push_str(&inline_text(content)),
            _ => {}
        }
    }
    output
}

fn row_text(row: &IrTableRow) -> Vec<String> {
    row.cells
        .iter()
        .map(|cell| inline_text(&cell.content))
        .collect()
}

fn only_table(result: &arkst_core::CompileResult) -> (&IrTableRow, &[IrTableRow]) {
    let [IrNode::Table { header, rows, .. }] = result.ir.nodes.as_slice() else {
        panic!("expected one table: {:?}", result.ir.nodes);
    };
    (header, rows)
}

#[test]
fn tablefilter_keeps_matching_rows_in_source_order_with_original_spans() {
    let source = ".tablefilter {2} {level: .isgreater {.level} than:{10}}\n    | Signal | Level |\n    |--------|-------|\n    | Lumen | 2 |\n    | Nimbus | 11 |\n    | Orbit | 11 |\n    | Prism | 20 |\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let (header, rows) = only_table(&result);
    assert_eq!(row_text(header), ["Signal", "Level"]);
    assert_eq!(
        rows.iter()
            .map(|row| row_text(row)[0].clone())
            .collect::<Vec<_>>(),
        ["Nimbus", "Orbit", "Prism"]
    );
    let nimbus_start = source.find("| Nimbus").expect("row is present");
    assert_eq!(rows[0].span.start, nimbus_start);
    assert_eq!(compile_source(source).ir, result.ir);
}

#[test]
fn tablefilter_named_variable_input_projects_rich_keys_but_preserves_cells() {
    let source = ".var {catalog}\n    | Signal | Code |\n    |--------|------|\n    | Aurora | **keep-2** |\n    | Boreal | skip-1 |\n    | Corona | keep-10 |\n\n.tablefilter column:{2} filter:{code: .startswith {.code} {keep}} table:{.catalog}\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let (_, rows) = only_table(&result);
    assert_eq!(
        rows.iter()
            .map(|row| row_text(row)[0].clone())
            .collect::<Vec<_>>(),
        ["Aurora", "Corona"]
    );
    assert!(matches!(
        rows[0].cells[1].content.as_slice(),
        [IrInline::Strong { .. }]
    ));
}

#[test]
fn tablefilter_treats_missing_and_blank_ragged_cells_as_empty() {
    let source = ".tablefilter {3} {note: .isempty {.note}}\n    | Signal | Code | Note |\n    |--------|------|------|\n    | Aurora | a | present |\n    | Boreal | b |\n    | Corona | c | |\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let (_, rows) = only_table(&result);
    assert_eq!(
        rows.iter()
            .map(|row| row_text(row)[0].clone())
            .collect::<Vec<_>>(),
        ["Boreal", "Corona"]
    );
    assert_eq!(row_text(&rows[0]), ["Boreal", "b", ""]);
}

#[test]
fn tablefilter_accepts_generated_headerless_and_nested_tables() {
    let generated = compile_source(
        ".tablefilter {2} {level: .isgreater {.level} than:{5}}\n    .tablebyrows\n        - - alpha\n          - 3\n        - - beta\n          - 12\n        - - gamma\n          - 6\n",
    );
    assert!(generated.diagnostics.is_empty(), "{generated:?}");
    let (header, rows) = only_table(&generated);
    assert!(header.cells.is_empty());
    assert_eq!(row_text(&rows[0]), ["beta", "12"]);
    assert_eq!(row_text(&rows[1]), ["gamma", "6"]);

    let nested = compile_source(
        ".tablefilter {2} {level: .isgreater {.level} than:{5}}\n    > | Signal | Level |\n    > |--------|-------|\n    > | Low | 3 |\n    > | High | 9 |\n",
    );
    assert!(nested.diagnostics.is_empty(), "{nested:?}");
    let (_, rows) = only_table(&nested);
    assert_eq!(rows.len(), 1);
    assert_eq!(row_text(&rows[0]), ["High", "9"]);
}

#[test]
fn tablefilter_skips_the_predicate_for_header_only_tables_and_can_keep_none() {
    let empty = compile_source(
        ".tablefilter {1} {@lambda .error {must-not-run}}\n    | Signal |\n    |--------|\n",
    );
    assert!(empty.diagnostics.is_empty(), "{empty:?}");
    let (header, rows) = only_table(&empty);
    assert_eq!(row_text(header), ["Signal"]);
    assert!(rows.is_empty());

    let none = compile_source(
        ".tablefilter {1} {cell: .iseven {1}}\n    | Signal |\n    |--------|\n    | Aurora |\n    | Boreal |\n",
    );
    assert!(none.diagnostics.is_empty(), "{none:?}");
    let (_, rows) = only_table(&none);
    assert!(rows.is_empty());
}

#[test]
fn tablefilter_rejects_invalid_calls_and_predicate_results_atomically() {
    for source in [
        ".tablefilter\n",
        ".tablefilter {0} {@lambda true}\n    | A |\n    |---|\n    | x |\n",
        ".tablefilter {2} {@lambda true}\n    | A |\n    |---|\n    | x |\n",
        ".tablefilter {1.5} {@lambda true}\n    | A |\n    |---|\n    | x |\n",
        ".tablefilter {1} {not-callable} {not-a-table}\n",
        ".tablefilter {1} {left right: true}\n    | A |\n    |---|\n    | x |\n",
        ".tablefilter {1} {@lambda 1}\n    | A |\n    |---|\n    | x |\n",
        ".tablefilter {1} {@lambda .var {local} {x}}\n    | A |\n    |---|\n    | x |\n",
        ".tablefilter {1} {cell: .error {boom}}\n    | A |\n    |---|\n    | x |\n",
        ".tablefilter {1} {cell: .unknown {.cell}}\n    | A |\n    |---|\n    | x |\n",
        ".tablefilter {1} {@lambda true} table:{not-a-table}\n",
        ".tablefilter {1} {@lambda true} table:{not-a-table} unknown:{x}\n",
    ] {
        let result = compile_source(source);
        assert_eq!(result.diagnostics.len(), 1, "{source:?}: {result:?}");
        assert!(
            result.diagnostics[0].primary.is_some(),
            "{source:?}: {result:?}"
        );
        assert!(result.ir.nodes.is_empty(), "{source:?}: {result:?}");
    }
}

#[test]
fn tablefilter_reuses_the_materialization_limit() {
    let options = CompileOptions {
        compatibility_profile: None,
        evaluation_limits: EvaluationLimits {
            max_materialized_elements: 5,
            max_evaluation_depth: 16,
        },
    };
    let result = compile_source_with_options(
        ".tablefilter {2} {@lambda true}\n    | A | B |\n    |---|---|\n    | 1 | 2 |\n    | 3 | 4 |\n",
        &options,
    );
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert_eq!(result.diagnostics[0].code, "E3005", "{result:?}");
    assert!(result.ir.nodes.is_empty());
}

#[test]
fn source_defined_tablefilter_keeps_precedence() {
    let source = ".function {tablefilter}\n    column filter table:\n    custom-filter\n\n.tablefilter {2} {value: true} {ignored}\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let [IrNode::Paragraph { content, .. }] = result.ir.nodes.as_slice() else {
        panic!("expected custom paragraph: {:?}", result.ir.nodes);
    };
    assert_eq!(inline_text(content), "custom-filter");
}
