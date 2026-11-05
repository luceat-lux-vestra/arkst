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
fn tablecompute_appends_one_aggregate_row_without_rewriting_source_rows() {
    let source = ".tablecompute {2} {values: .values::sumall}\n    | Signal | Level | Note |\n    |--------|-------|------|\n    | Lumen | 3 | a |\n    | Nimbus | 7 | b |\n    | Orbit | 12 | c |\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let (header, rows) = only_table(&result);
    assert_eq!(row_text(header), ["Signal", "Level", "Note"]);
    assert_eq!(rows.len(), 4);
    assert_eq!(row_text(&rows[0]), ["Lumen", "3", "a"]);
    assert_eq!(row_text(&rows[1]), ["Nimbus", "7", "b"]);
    assert_eq!(row_text(&rows[2]), ["Orbit", "12", "c"]);
    assert_eq!(row_text(&rows[3]), ["", "22", ""]);
    assert_eq!(
        rows[0].span.start,
        source.find("| Lumen").expect("source row")
    );
    assert_eq!(compile_source(source).ir, result.ir);
}

#[test]
fn tablecompute_named_variable_input_preserves_rich_cells_and_collects_all_rows_once() {
    let source = ".var {catalog}\n    | Signal | Code |\n    |--------|------|\n    | Aurora | **keep-2** |\n    | Boreal | skip-1 |\n\n.tablecompute column:{2} compute:{values: .values::size} table:{.catalog}\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let (_, rows) = only_table(&result);
    assert_eq!(rows.len(), 3);
    assert!(matches!(
        rows[0].cells[1].content.as_slice(),
        [IrInline::Strong { .. }]
    ));
    assert_eq!(row_text(&rows[2]), ["", "2"]);
}

#[test]
fn tablecompute_treats_header_only_tables_as_an_empty_input_collection() {
    let source =
        ".tablecompute {2} {values: .values::size}\n    | Signal | Level |\n    |--------|-------|\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let (header, rows) = only_table(&result);
    assert_eq!(row_text(header), ["Signal", "Level"]);
    assert_eq!(rows.len(), 1);
    assert_eq!(row_text(&rows[0]), ["", "0"]);
}

#[test]
fn tablecompute_accepts_generated_nested_and_composed_tables() {
    let generated = compile_source(
        ".tablecompute {2} {values: .values::sumall}\n    .tablebyrows\n        - - alpha\n          - 3\n        - - beta\n          - 12\n",
    );
    assert!(generated.diagnostics.is_empty(), "{generated:?}");
    let (header, rows) = only_table(&generated);
    assert!(header.cells.is_empty());
    assert_eq!(row_text(&rows[2]), ["", "15"]);

    let nested = compile_source(
        ".tablecompute {2} {values: .values::size}\n    > | Signal | Level |\n    > |--------|-------|\n    > | Low | 3 |\n    > | High | 9 |\n",
    );
    assert!(nested.diagnostics.is_empty(), "{nested:?}");
    let (_, rows) = only_table(&nested);
    assert_eq!(row_text(&rows[2]), ["", "2"]);

    let composed = compile_source(
        ".tablecompute {2} {values: .values::sumall}\n    .tablefilter {2} {level: .isgreater {.level} than:{5}}\n        | Signal | Level |\n        |--------|-------|\n        | Low | 3 |\n        | Mid | 7 |\n        | High | 9 |\n",
    );
    assert!(composed.diagnostics.is_empty(), "{composed:?}");
    let (_, rows) = only_table(&composed);
    assert_eq!(row_text(&rows[0]), ["Mid", "7"]);
    assert_eq!(row_text(&rows[1]), ["High", "9"]);
    assert_eq!(row_text(&rows[2]), ["", "16"]);
}

#[test]
fn tablecompute_matches_evidenced_result_text_and_ragged_input_projections() {
    for (source, expected) in [
        (
            ".tablecompute {2} {values: .isgreater {.values::size} than:{1}}\n    | Signal | Amount |\n    |--------|--------|\n    | Torch | 2 |\n",
            "false",
        ),
        (
            ".tablecompute {2} {values: .range {2} {4}}\n    | Signal | Amount |\n    |--------|--------|\n    | Union | 2 |\n",
            "2..4",
        ),
        (
            ".tablecompute {2} {values: .none}\n    | Signal | Amount |\n    |--------|--------|\n    | Vector | 2 |\n",
            "None",
        ),
        (
            ".tablecompute {2} {values: .emptycollection}\n    | Signal | Amount |\n    |--------|--------|\n    | Tide | 2 |\n",
            "[]",
        ),
        (
            ".tablecompute {2} {values: .var {local} {x}}\n    | Signal | Amount |\n    |--------|--------|\n    | Umber | 2 |\n",
            "kotlin.Unit",
        ),
    ] {
        let result = compile_source(source);
        assert!(result.diagnostics.is_empty(), "{source:?}: {result:?}");
        let (_, rows) = only_table(&result);
        assert_eq!(
            row_text(rows.last().expect("computed row"))[1],
            expected,
            "{source:?}"
        );
    }

    let ragged = compile_source(
        ".tablecompute {3} {values: .values::getat {2}::isempty}\n    | Signal | Code | Note |\n    |--------|------|------|\n    | Delta | d | present |\n    | Epsilon | e |\n    | Zeta | z | tail |\n",
    );
    assert!(ragged.diagnostics.is_empty(), "{ragged:?}");
    let (_, rows) = only_table(&ragged);
    assert_eq!(
        row_text(rows.last().expect("computed row")),
        ["", "", "true"]
    );
}

#[test]
fn tablecompute_fails_closed_for_unstable_content_backed_result_coercions() {
    for source in [
        ".tablecompute {1} {values: .values::join separator:{+}}\n    | Signal |\n    |--------|\n    | Pine |\n    | Quartz |\n",
        ".tablecompute {2} {values: **count=.values::size**}\n    | Signal | Amount |\n    |--------|--------|\n    | Reed | 2 |\n    | Stone | 8 |\n",
    ] {
        let result = compile_source(source);
        assert_eq!(result.diagnostics.len(), 1, "{source:?}: {result:?}");
        assert_eq!(result.diagnostics[0].code, "E8001", "{source:?}: {result:?}");
        assert!(result.ir.nodes.is_empty(), "{source:?}: {result:?}");
    }
}

#[test]
fn tablecompute_rejects_invalid_calls_and_callback_failures_atomically() {
    for source in [
        ".tablecompute\n",
        ".tablecompute {0} {@lambda 1}\n    | A |\n    |---|\n    | x |\n",
        ".tablecompute {2} {@lambda 1}\n    | A |\n    |---|\n    | x |\n",
        ".tablecompute {1.5} {@lambda 1}\n    | A |\n    |---|\n    | x |\n",
        ".tablecompute {1} {not-callable} {not-a-table}\n",
        ".tablecompute {1} {left right: 1}\n    | A |\n    |---|\n    | x |\n",
        ".tablecompute {1} {values: .error {boom}}\n    | A |\n    |---|\n    | x |\n",
        ".tablecompute {1} {values: .unknown {.values}}\n    | A |\n    |---|\n    | x |\n",
        ".tablecompute {1} {@lambda 1} table:{not-a-table}\n",
        ".tablecompute {1} {@lambda 1} table:{not-a-table} unknown:{x}\n",
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
fn tablecompute_reuses_the_materialization_limit_for_the_appended_row() {
    let options = CompileOptions {
        compatibility_profile: None,
        evaluation_limits: EvaluationLimits {
            max_materialized_elements: 7,
            max_evaluation_depth: 16,
        },
    };
    let result = compile_source_with_options(
        ".tablecompute {2} {values: .values::size}\n    | A | B |\n    |---|---|\n    | 1 | 2 |\n    | 3 | 4 |\n",
        &options,
    );
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert_eq!(result.diagnostics[0].code, "E3005", "{result:?}");
    assert!(result.ir.nodes.is_empty());
}

#[test]
fn source_defined_tablecompute_keeps_precedence() {
    let source = ".function {tablecompute}\n    column compute table:\n    custom-compute\n\n.tablecompute {2} {values: .values::size} {ignored}\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let [IrNode::Paragraph { content, .. }] = result.ir.nodes.as_slice() else {
        panic!("expected custom paragraph: {:?}", result.ir.nodes);
    };
    assert_eq!(inline_text(content), "custom-compute");
}
