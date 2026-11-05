use arkst_core::ir::{IrInline, IrNode, IrTableRow};
use arkst_core::{
    compile, CompileOptions, EvaluationLimits, VirtualProject, VirtualProjectBuilder,
};

fn project(entry: &str, sources: &[(&str, &str)], assets: &[(&str, &[u8])]) -> VirtualProject {
    let mut builder = VirtualProjectBuilder::new()
        .entry(entry)
        .expect("valid entry path");
    for (path, source) in sources {
        builder = builder
            .add_source(*path, *source)
            .expect("valid source path");
    }
    for (path, asset) in assets {
        builder = builder
            .add_asset(*path, asset.to_vec())
            .expect("valid asset path");
    }
    builder.build().expect("valid virtual project")
}

fn compile_project(project: &VirtualProject) -> arkst_core::CompileResult {
    compile(project, &CompileOptions::default())
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
fn csv_default_and_explicit_plain_materialize_literal_text_in_existing_table_ir() {
    for source in [".csv {data.csv}\n", ".csv {data.csv} mode:{plain}\n"] {
        let project = project(
            "main.qd",
            &[("main.qd", source)],
            &[("data.csv", b" Signal , Note\n Ember , **bold**\n")],
        );
        let result = compile_project(&project);
        assert!(result.diagnostics.is_empty(), "{source:?}: {result:?}");
        let (header, rows) = only_table(&result);
        assert_eq!(row_text(header), ["Signal", "Note"]);
        assert_eq!(row_text(&rows[0]), ["Ember", "**bold**"]);
        assert!(matches!(
            rows[0].cells[1].content.as_slice(),
            [IrInline::Text { content, .. }] if content == "**bold**"
        ));
    }
}

#[test]
fn csv_empty_and_header_only_resources_still_materialize_empty_table_nodes() {
    for csv in [b"".as_slice(), b"Signal,Amount\n".as_slice()] {
        let project = project(
            "main.qd",
            &[("main.qd", ".csv {data.csv}\n")],
            &[("data.csv", csv)],
        );
        let result = compile_project(&project);
        assert!(result.diagnostics.is_empty(), "{result:?}");
        let (header, rows) = only_table(&result);
        assert!(header.cells.is_empty());
        assert!(rows.is_empty());
    }
}

#[test]
fn csv_resolves_from_the_current_source_and_composes_with_tablecompute() {
    let nested = project(
        "nested/main.qd",
        &[("nested/main.qd", ".csv {data.csv}\n")],
        &[("nested/data.csv", b"Signal,Amount\nMoss,13\nNova,17\n")],
    );
    let result = compile_project(&nested);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let (header, rows) = only_table(&result);
    assert_eq!(row_text(header), ["Signal", "Amount"]);
    assert_eq!(row_text(&rows[0]), ["Moss", "13"]);
    assert_eq!(row_text(&rows[1]), ["Nova", "17"]);

    let composed = project(
        "main.qd",
        &[(
            "main.qd",
            ".tablecompute {2} {values: .values::sumall}\n    .csv {data.csv}\n",
        )],
        &[("data.csv", b"Signal,Amount\nJuniper,5\nKestrel,11\n")],
    );
    let result = compile_project(&composed);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let (_, rows) = only_table(&result);
    assert_eq!(row_text(&rows[0]), ["Juniper", "5"]);
    assert_eq!(row_text(&rows[1]), ["Kestrel", "11"]);
    assert_eq!(row_text(&rows[2]), ["", "16"]);
}

#[test]
fn csv_preserves_the_bounded_quoted_parser_contract() {
    let project = project(
        "main.qd",
        &[("main.qd", ".csv {data.csv}\n")],
        &[(
            "data.csv",
            b"Name,Value,Note\n\"Harbor, Jr.\",7,\"a \"\"quote\"\"\"\nIris,3,tail\n",
        )],
    );
    let result = compile_project(&project);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let (header, rows) = only_table(&result);
    assert_eq!(row_text(header), ["Name", "Value", "Note"]);
    assert_eq!(row_text(&rows[0]), ["Harbor, Jr.", "7", "a \"quote\""]);
    assert_eq!(row_text(&rows[1]), ["Iris", "3", "tail"]);
}

#[test]
fn csv_fails_closed_for_unclaimed_caption_ref_and_host_paths() {
    for (source, expected_code) in [
        (".csv {data.csv} caption:{Report}\n", "E8001"),
        (".csv {data.csv} ref:{signals}\n", "E8001"),
        (".csv {data.csv} mode:{unknown}\n", "E3003"),
        (".csv {/tmp/data.csv}\n", "E8001"),
    ] {
        let project = project(
            "main.qd",
            &[("main.qd", source)],
            &[("data.csv", b"Signal,Amount\nLumen,2\n")],
        );
        let result = compile_project(&project);
        assert_eq!(result.diagnostics.len(), 1, "{source:?}: {result:?}");
        assert_eq!(result.diagnostics[0].code, expected_code, "{source:?}");
        assert!(result.ir.nodes.is_empty(), "{source:?}: {result:?}");
    }
}

#[test]
fn csv_parser_failures_are_source_backed_and_atomic() {
    let project = project(
        "main.qd",
        &[("main.qd", ".csv {data.csv}\n")],
        &[("data.csv", b"Signal,Amount\nAlder\n")],
    );
    let result = compile_project(&project);
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert_eq!(result.diagnostics[0].code, "E3001");
    assert!(result.diagnostics[0].message.contains("data.csv"));
    assert!(result.diagnostics[0].primary.is_some());
    assert!(result.ir.nodes.is_empty());
}

#[test]
fn csv_reuses_the_evaluator_materialization_limit() {
    let project = project(
        "main.qd",
        &[("main.qd", ".csv {data.csv}\n")],
        &[("data.csv", b"Signal,Amount\nAlder,4\nBirch,9\n")],
    );
    let options = CompileOptions {
        compatibility_profile: None,
        evaluation_limits: EvaluationLimits {
            max_materialized_elements: 5,
            max_evaluation_depth: 16,
        },
    };
    let result = compile(&project, &options);
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert_eq!(result.diagnostics[0].code, "E3005");
    assert!(result.ir.nodes.is_empty());
}

#[test]
fn source_defined_csv_keeps_precedence_over_the_new_resource_native() {
    let project = project(
        "main.qd",
        &[(
            "main.qd",
            ".function {csv}\n    path:\n    custom-csv\n\n.csv {ignored}\n",
        )],
        &[],
    );
    let result = compile_project(&project);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let [IrNode::Paragraph { content, .. }] = result.ir.nodes.as_slice() else {
        panic!("expected custom csv output: {:?}", result.ir.nodes);
    };
    assert_eq!(inline_text(content), "custom-csv");
}
