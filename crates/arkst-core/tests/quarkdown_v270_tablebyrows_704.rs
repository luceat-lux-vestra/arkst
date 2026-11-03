use arkst_core::ir::{IrInline, IrNode, IrTableCell, IrTableRow};
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

fn cell_text(cell: &IrTableCell) -> String {
    inline_text(&cell.content)
}

fn row_text(row: &IrTableRow) -> Vec<String> {
    row.cells.iter().map(cell_text).collect()
}

fn only_table(result: &arkst_core::CompileResult) -> (&IrTableRow, &[IrTableRow]) {
    let [IrNode::Table { header, rows, .. }] = result.ir.nodes.as_slice() else {
        panic!("expected one table, got {:?}", result.ir.nodes);
    };
    (header, rows)
}

#[test]
fn tablebyrows_materializes_dynamic_repeat_rows_and_named_headers() {
    let source = ".var {generated}\n    .repeat {3}\n        index:\n        - - Orbit .index:X\n          - Orbit .index:Y\n\n.var {labels}\n    - Label\n    - Index\n\n.tablebyrows headers:{.labels} rows:{.generated}\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");

    let (header, rows) = only_table(&result);
    assert_eq!(row_text(header), ["Label", "Index"]);
    assert_eq!(rows.len(), 3);
    assert_eq!(row_text(&rows[0]), ["Orbit 1:X", "Orbit 1:Y"]);
    assert_eq!(row_text(&rows[1]), ["Orbit 2:X", "Orbit 2:Y"]);
    assert_eq!(row_text(&rows[2]), ["Orbit 3:X", "Orbit 3:Y"]);
    assert_eq!(compile_source(source).ir, result.ir);
}

#[test]
fn tablebyrows_accepts_block_rows_and_preserves_ragged_structure() {
    let source = ".var {labels}\n    - Left\n    - Right\n\n.tablebyrows {.labels}\n    - - A\n      - B\n    - - C\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");

    let (header, rows) = only_table(&result);
    assert_eq!(row_text(header), ["Left", "Right"]);
    assert_eq!(rows.len(), 2);
    assert_eq!(row_text(&rows[0]), ["A", "B"]);
    assert_eq!(row_text(&rows[1]), ["C"]);
}

#[test]
fn tablebyrows_accepts_empty_headers_and_rows() {
    let result = compile_source(".tablebyrows rows:{.emptycollection}\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let (header, rows) = only_table(&result);
    assert!(header.cells.is_empty());
    assert!(rows.is_empty());
}

#[test]
fn tablebyrows_retains_row_and_cell_provenance() {
    let source = ".tablebyrows headers:{.emptycollection}\n    - - Alpha\n      - Beta\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let (_, rows) = only_table(&result);
    let [row] = rows else {
        panic!("expected one row: {rows:?}");
    };
    assert!(source[row.span.start..row.span.end].contains("Alpha"));
    assert!(source[row.cells[0].span.start..row.cells[0].span.end].contains("Alpha"));
    assert!(source[row.cells[1].span.start..row.cells[1].span.end].contains("Beta"));
}

#[test]
fn tablebyrows_rejects_invalid_binding_and_non_iterable_rows_atomically() {
    for source in [
        ".tablebyrows\n",
        ".tablebyrows rows:{not-iterable}\n",
        ".tablebyrows rows:{.range {1} {2}}\n",
        ".tablebyrows rows:{.emptycollection} unknown:{x}\n",
        ".tablebyrows rows:{.emptycollection} rows:{.emptycollection}\n",
        ".tablebyrows {.emptycollection} {.emptycollection} {extra}\n",
        ".tablebyrows rows:{.emptycollection} body:{@lambda .1}\n",
        ".tablebyrows\n    row:\n    - - .row\n",
        ".tablebyrows headers:{.emptycollection}\n    - - - A\n        - B\n",
    ] {
        let result = compile_source(source);
        assert_eq!(result.diagnostics.len(), 1, "{source}: {result:?}");
        assert_eq!(result.diagnostics[0].code, "E3001", "{source}: {result:?}");
        let primary = result.diagnostics[0]
            .primary
            .expect("tablebyrows failure should retain provenance");
        assert!(primary.end > primary.start, "{source}: {result:?}");
        assert!(result.ir.nodes.is_empty(), "{source}: {result:?}");
    }
}

#[test]
fn tablebyrows_evaluates_explicit_arguments_in_source_order_and_keeps_rows_body_lazy() {
    for (source, expected, rejected) in [
        (
            ".tablebyrows headers:{.error {headers-first}} rows:{.error {rows-second}}\n",
            "headers-first",
            "rows-second",
        ),
        (
            ".tablebyrows rows:{.error {rows-first}} headers:{.error {headers-second}}\n",
            "rows-first",
            "headers-second",
        ),
        (
            ".tablebyrows headers:{not-iterable}\n    .error {body-must-stay-lazy}\n",
            "Value is not an iterable",
            "body-must-stay-lazy",
        ),
    ] {
        let result = compile_source(source);
        assert_eq!(result.diagnostics.len(), 1, "{source}: {result:?}");
        assert!(
            result.diagnostics[0].message.contains(expected),
            "{source}: {result:?}"
        );
        assert!(
            !result.diagnostics[0].message.contains(rejected),
            "{source}: {result:?}"
        );
        assert!(result.ir.nodes.is_empty(), "{source}: {result:?}");
    }
}

#[test]
fn tablebyrows_counts_headers_and_cells_against_the_materialization_limit() {
    let options = CompileOptions {
        compatibility_profile: None,
        evaluation_limits: EvaluationLimits {
            max_materialized_elements: 3,
            max_evaluation_depth: 16,
        },
    };
    let source = ".var {labels}\n    - H1\n    - H2\n\n.var {rows}\n    - - A\n      - B\n\n.tablebyrows headers:{.labels} rows:{.rows}\n";
    let result = compile_source_with_options(source, &options);
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert_eq!(result.diagnostics[0].code, "E3005", "{result:?}");
    assert!(result.ir.nodes.is_empty(), "{result:?}");
}

#[test]
fn source_defined_tablebyrows_keeps_precedence() {
    let source = ".function {tablebyrows}\n    headers? rows:\n    custom-.rows::size\n\n.tablebyrows rows:{.emptycollection}\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let [IrNode::Paragraph { content, .. }] = result.ir.nodes.as_slice() else {
        panic!("expected source-defined output: {:?}", result.ir.nodes);
    };
    assert_eq!(inline_text(content), "custom-0");
}
