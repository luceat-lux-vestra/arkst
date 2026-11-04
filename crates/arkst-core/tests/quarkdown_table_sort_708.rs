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
fn tablesort_uses_stable_ascii_natural_order_and_preserves_rich_cells() {
    let source = ".tablesort {2}\n    | ID | Key |\n    |----|-----|\n    | lower-two | **a2** |\n    | upper-two | A2 |\n    | lower-ten | a10 |\n    | lower-zero-two | a02 |\n    | lower-one | a1 |\n    | bare | a |\n    | dash | a-2 |\n    | dot | a.2 |\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let (header, rows) = only_table(&result);
    assert_eq!(row_text(header), ["ID", "Key"]);
    assert_eq!(
        rows.iter()
            .map(|row| row_text(row)[0].clone())
            .collect::<Vec<_>>(),
        [
            "upper-two",
            "bare",
            "lower-one",
            "lower-two",
            "lower-zero-two",
            "lower-ten",
            "dash",
            "dot",
        ]
    );
    assert!(matches!(
        rows[3].cells[1].content.as_slice(),
        [IrInline::Strong { .. }]
    ));
    let upper_two_start = source.find("| upper-two").expect("row is present");
    assert_eq!(rows[0].span.start, upper_two_start);
    assert_eq!(compile_source(source).ir, result.ir);
}

#[test]
fn tablesort_descending_is_stable_and_treats_missing_ragged_cells_as_empty() {
    let source = ".tablesort {3} order:{descending}\n    | ID | Label | Rank |\n    |----|-------|------|\n    | first-two | **x** | 2 |\n    | missing | y |\n    | one | z | 1 |\n    | second-two | w | 2 |\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let (_, rows) = only_table(&result);
    assert_eq!(
        rows.iter()
            .map(|row| row_text(row)[0].clone())
            .collect::<Vec<_>>(),
        ["first-two", "second-two", "one", "missing"]
    );
    assert_eq!(row_text(&rows[3]), ["missing", "y", ""]);
}

#[test]
fn tablesort_accepts_named_variable_held_tablebyrows_content() {
    let source = ".var {catalog}\n    .tablebyrows\n        - - gamma\n          - 20\n        - - alpha\n          - 3\n        - - beta\n          - 11\n\n.tablesort column:{2} order:{descending} table:{.catalog}\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let (header, rows) = only_table(&result);
    assert!(header.cells.is_empty());
    assert_eq!(
        rows.iter()
            .map(|row| row_text(row)[0].clone())
            .collect::<Vec<_>>(),
        ["gamma", "beta", "alpha"]
    );
}

#[test]
fn tablesort_finds_a_nested_single_table_and_outputs_only_the_table() {
    let source = ".tablesort {2}\n    > | Name | Score |\n    > |------|-------|\n    > | B | 9 |\n    > | A | 1 |\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let (_, rows) = only_table(&result);
    assert_eq!(row_text(&rows[0]), ["A", "1"]);
    assert_eq!(row_text(&rows[1]), ["B", "9"]);
}

#[test]
fn tablesort_rejects_invalid_binding_order_shape_and_indices_atomically() {
    for source in [
        ".tablesort\n",
        ".tablesort {1}\n    no table\n",
        ".tablesort {0}\n    | A |\n    |---|\n    | x |\n",
        ".tablesort {2}\n    | A |\n    |---|\n    | x |\n",
        ".tablesort {1.5}\n    | A |\n    |---|\n    | x |\n",
        ".tablesort {1} order:{sideways}\n    | A |\n    |---|\n    | x |\n",
        ".tablesort {1} order:{.string {ascending}}\n    | A |\n    |---|\n    | x |\n",
        ".tablesort {1} table:{no table} unknown:{x}\n",
        ".tablesort {1} table:{no table} table:{still no table}\n",
    ] {
        let result = compile_source(source);
        assert_eq!(result.diagnostics.len(), 1, "{source:?}: {result:?}");
        assert_eq!(
            result.diagnostics[0].code, "E3001",
            "{source:?}: {result:?}"
        );
        assert!(
            result.diagnostics[0].primary.is_some(),
            "{source:?}: {result:?}"
        );
        assert!(result.ir.nodes.is_empty(), "{source:?}: {result:?}");
    }
}

#[test]
fn tablesort_fails_closed_for_unproven_non_ascii_natural_ordering() {
    let result =
        compile_source(".tablesort {1}\n    | Key |\n    |-----|\n    | é2 |\n    | é10 |\n");
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert_eq!(result.diagnostics[0].code, "E8001", "{result:?}");
    assert!(result.diagnostics[0].message.contains("ASCII"));
    assert!(result.ir.nodes.is_empty());
}

#[test]
fn tablesort_reuses_the_materialization_limit() {
    let options = CompileOptions {
        compatibility_profile: None,
        evaluation_limits: EvaluationLimits {
            max_materialized_elements: 5,
            max_evaluation_depth: 16,
        },
    };
    let result = compile_source_with_options(
        ".tablesort {2}\n    | A | B |\n    |---|---|\n    | 1 | 2 |\n    | 3 | 4 |\n",
        &options,
    );
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert_eq!(result.diagnostics[0].code, "E3005", "{result:?}");
    assert!(result.ir.nodes.is_empty());
}

#[test]
fn source_defined_tablesort_keeps_precedence() {
    let source = ".function {tablesort}\n    column order table:\n    custom-sort\n\n.tablesort {2} {descending} {ignored}\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let [IrNode::Paragraph { content, .. }] = result.ir.nodes.as_slice() else {
        panic!("expected custom paragraph: {:?}", result.ir.nodes);
    };
    assert_eq!(inline_text(content), "custom-sort");
}
