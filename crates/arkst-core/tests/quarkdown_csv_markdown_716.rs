use arkst_core::ir::{IrInline, IrNode, IrTableRow};
use arkst_core::{compile, CompileOptions, VirtualProject, VirtualProjectBuilder};

fn project(entry: &str, source: &str, assets: &[(&str, &[u8])]) -> VirtualProject {
    let mut builder = VirtualProjectBuilder::new()
        .entry(entry)
        .expect("valid entry path")
        .add_source(entry, source)
        .expect("valid source");
    for (path, bytes) in assets {
        builder = builder
            .add_asset(*path, bytes.to_vec())
            .expect("valid asset");
    }
    builder.build().expect("valid project")
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
            IrInline::SoftBreak { .. } | IrInline::HardBreak { .. } => output.push('\n'),
            IrInline::Emphasis { content, .. }
            | IrInline::Strong { content, .. }
            | IrInline::Link { content, .. } => output.push_str(&inline_text(content)),
            _ => {}
        }
    }
    output
}

fn row_text(row: &IrTableRow) -> Vec<String> {
    row.cells.iter().map(|cell| inline_text(&cell.content)).collect()
}

fn only_table(result: &arkst_core::CompileResult) -> (&IrTableRow, &[IrTableRow]) {
    let [IrNode::Table { header, rows, .. }] = result.ir.nodes.as_slice() else {
        panic!("expected one table: {:?}", result.ir.nodes);
    };
    (header, rows)
}

#[test]
fn csv_markdown_materializes_evidenced_rich_inline_headers_and_cells() {
    let p = project(
        "main.qd",
        ".csv {data.csv} mode:{markdown}\n",
        &[("data.csv", b"**Name**,_Note_,Link,Code\n**Alder**,*east*,[site](https://example.com),`x + y`\n")],
    );
    let result = compile_project(&p);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let (header, rows) = only_table(&result);
    assert!(matches!(header.cells[0].content.as_slice(), [IrInline::Strong { .. }]));
    assert!(matches!(header.cells[1].content.as_slice(), [IrInline::Emphasis { .. }]));
    assert!(matches!(rows[0].cells[0].content.as_slice(), [IrInline::Strong { .. }]));
    assert!(matches!(rows[0].cells[1].content.as_slice(), [IrInline::Emphasis { .. }]));
    assert!(matches!(
        rows[0].cells[2].content.as_slice(),
        [IrInline::Link { destination, .. }] if destination == "https://example.com"
    ));
    assert!(matches!(
        rows[0].cells[3].content.as_slice(),
        [IrInline::Code { content, .. }] if content == "x + y"
    ));
}

#[test]
fn csv_markdown_keeps_block_looking_markers_literal_inside_cells() {
    let p = project(
        "main.qd",
        ".csv {data.csv} mode:{markdown}\n",
        &[("data.csv", b"Heading,Listish,Quoteish,Tableish\n\"# Heading\",\"- item\",\"> quote\",\"| A | B |\"\n")],
    );
    let result = compile_project(&p);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let (_, rows) = only_table(&result);
    assert_eq!(row_text(&rows[0]), ["# Heading", "- item", "> quote", "| A | B |"]);
    assert!(rows[0].cells.iter().all(|cell| matches!(cell.content.as_slice(), [IrInline::Text { .. }])));
}

#[test]
fn csv_markdown_preserves_soft_and_hard_inline_break_semantics() {
    let p = project(
        "main.qd",
        ".csv {data.csv} mode:{markdown}\n",
        &[("data.csv", b"Name,Soft,Hard\nAlder,\"first line\n**second line**\",\"first  \nsecond\"\n")],
    );
    let result = compile_project(&p);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let (_, rows) = only_table(&result);
    assert!(rows[0].cells[1].content.iter().any(|x| matches!(x, IrInline::SoftBreak { .. })));
    assert!(rows[0].cells[1].content.iter().any(|x| matches!(x, IrInline::Strong { .. })));
    assert!(rows[0].cells[2].content.iter().any(|x| matches!(x, IrInline::HardBreak { .. })));
}

#[test]
fn csv_markdown_composes_with_sort_while_preserving_rich_cells() {
    let p = project(
        "main.qd",
        ".tablesort {1}\n    .csv {data.csv} mode:{markdown}\n",
        &[("data.csv", b"Name,Amount\n**B10**,2\n*A2*,5\n")],
    );
    let result = compile_project(&p);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let (_, rows) = only_table(&result);
    assert_eq!(row_text(&rows[0]), ["A2", "5"]);
    assert_eq!(row_text(&rows[1]), ["B10", "2"]);
    assert!(matches!(rows[0].cells[0].content.as_slice(), [IrInline::Emphasis { .. }]));
    assert!(matches!(rows[1].cells[0].content.as_slice(), [IrInline::Strong { .. }]));
}

#[test]
fn csv_markdown_empty_and_header_only_keep_the_existing_empty_table_contract() {
    for csv in [b"".as_slice(), b"**Name**,_Value_\n".as_slice()] {
        let p = project("main.qd", ".csv {data.csv} mode:{markdown}\n", &[("data.csv", csv)]);
        let result = compile_project(&p);
        assert!(result.diagnostics.is_empty(), "{result:?}");
        let (header, rows) = only_table(&result);
        assert!(header.cells.is_empty());
        assert!(rows.is_empty());
    }
}

#[test]
fn csv_markdown_resolves_relative_to_the_current_source() {
    let p = project(
        "nested/main.qd",
        ".csv {data.csv} mode:{markdown}\n",
        &[("nested/data.csv", b"Name,Note\n**Moss**,*nested*\n")],
    );
    let result = compile_project(&p);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let (_, rows) = only_table(&result);
    assert!(matches!(rows[0].cells[0].content.as_slice(), [IrInline::Strong { .. }]));
    assert!(matches!(rows[0].cells[1].content.as_slice(), [IrInline::Emphasis { .. }]));
}

#[test]
fn csv_markdown_fails_closed_for_unevidenced_inline_categories() {
    let p = project(
        "main.qd",
        ".csv {data.csv} mode:{markdown}\n",
        &[("data.csv", b"Name,Image\nAlder,![alt](image.png)\n")],
    );
    let result = compile_project(&p);
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert_eq!(result.diagnostics[0].code, "E8001");
    assert!(result.ir.nodes.is_empty());
}
