use arkst_core::ir::{IrInline, IrNode};
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

fn output_text(result: &arkst_core::CompileResult) -> String {
    result
        .ir
        .nodes
        .iter()
        .filter_map(|node| match node {
            IrNode::Paragraph { content, .. } => Some(inline_text(content)),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn tablecolumn_excludes_headers_and_projects_rich_cells_to_strings() {
    let source = ".var {ages}\n    .tablecolumn {2}\n        | Name | Age |\n        |------|-----|\n        | John | 25  |\n        | Lisa | 32  |\n\n.var {labels}\n    .tablecolumn {1}\n        | Label |\n        |-------|\n        | **Bold** and *soft* |\n\nfirst=.ages::first size=.ages::size sum=.ages::sumall label=.labels::first\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(
        output_text(&result),
        "first=25 size=2 sum=57 label=Bold and soft"
    );
    let [IrNode::Paragraph { content, .. }] = result.ir.nodes.as_slice() else {
        panic!("expected one output paragraph: {:?}", result.ir.nodes);
    };
    assert!(content
        .iter()
        .all(|inline| !matches!(inline, IrInline::Strong { .. } | IrInline::Emphasis { .. })));
    assert_eq!(compile_source(source).ir, result.ir);
}

#[test]
fn tablecolumns_preserves_order_and_pads_ragged_body_rows() {
    let source = ".var {columns}\n    .tablecolumns\n        | A | B | C |\n        |---|---|---|\n        | 1 | 2 |\n        | 3 | 4 | 5 |\n\ncount=.columns::size sizes=.columns::first::size,.columns::second::size,.columns::third::size first=.columns::first::first third=.columns::third::first\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(output_text(&result), "count=3 sizes=2,2,2 first=1 third=");
}

#[test]
fn tablecolumns_finds_the_first_table_inside_structured_content() {
    let source = ".var {columns}\n    .tablecolumns\n        > | A | B |\n        > |---|---|\n        > | x | y |\n\nleft=.columns::first::first right=.columns::second::first\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(output_text(&result), "left=x right=y");
}

#[test]
fn tablecolumns_consumes_tablebyrows_without_a_second_table_representation() {
    let source = ".var {columns}\n    .tablecolumns\n        .tablebyrows\n            - - 2\n              - **ten**\n            - - 3\n\ncount=.columns::size sizes=.columns::first::size,.columns::second::size first=.columns::first::first second=.columns::second::first missing=.columns::second::second\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(
        output_text(&result),
        "count=2 sizes=2,2 first=2 second=ten missing="
    );
}

#[test]
fn tablecolumn_accepts_named_and_variable_held_table_content() {
    let source = ".var {table}\n    | A | B |\n    |---|---|\n    | x | y |\n\n.tablecolumn column:{2} of:{.table}::first\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(output_text(&result), "y");
}

#[test]
fn table_column_extraction_rejects_invalid_shapes_and_indices_atomically() {
    for source in [
        ".tablecolumn\n",
        ".tablecolumn {1}\n    no table\n",
        ".tablecolumn {0}\n    | A |\n    |---|\n    | x |\n",
        ".tablecolumn {2}\n    | A |\n    |---|\n    | x |\n",
        ".tablecolumn {1.5}\n    | A |\n    |---|\n    | x |\n",
        ".tablecolumn {1} of:{no table} unknown:{x}\n",
        ".tablecolumns\n    no table\n",
        ".tablecolumns of:{no table} of:{still no table}\n",
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
fn table_column_extraction_reuses_the_materialization_limit() {
    let options = CompileOptions {
        compatibility_profile: None,
        evaluation_limits: EvaluationLimits {
            max_materialized_elements: 3,
            max_evaluation_depth: 16,
        },
    };
    let source = ".tablecolumns\n    | A | B |\n    |---|---|\n    | 1 | 2 |\n    | 3 | 4 |\n";
    let result = compile_source_with_options(source, &options);
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert_eq!(result.diagnostics[0].code, "E3005", "{result:?}");
    assert!(result.ir.nodes.is_empty());
}

#[test]
fn source_defined_table_column_functions_keep_precedence() {
    let source = ".function {tablecolumn}\n    column of:\n    one-custom\n\n.function {tablecolumns}\n    of:\n    all-custom\n\n.tablecolumn {1} {ignored}\n.tablecolumns {ignored}\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(output_text(&result), "one-custom\nall-custom");
}
