use arkst_core::ir::{IrInline, IrNode};
use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};

fn paragraph_text(document: &arkst_core::ir::IrDocument) -> String {
    document
        .nodes
        .iter()
        .filter_map(|node| match node {
            IrNode::Paragraph { content, .. } => Some(
                content
                    .iter()
                    .filter_map(|inline| match inline {
                        IrInline::Text { content, .. } => Some(content.as_str()),
                        _ => None,
                    })
                    .collect::<String>(),
            ),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn compile_wires_virtual_project_library_registry_before_local_file_lookup() {
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .unwrap()
        .add_source("main.qd", ".include {hello} sandbox:{scope}\n.libvalue")
        .unwrap()
        .add_source("hello", "wrong file")
        .unwrap()
        .add_loadable_library("hello", ".var {libvalue} {library}")
        .build()
        .unwrap();
    let result = compile(&project, &CompileOptions::default());
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(paragraph_text(&result.ir), "library");
}

#[test]
fn compile_library_reads_from_the_includer_resource_base() {
    let project = VirtualProjectBuilder::new()
        .entry("docs/main.qd")
        .unwrap()
        .add_source("docs/main.qd", ".include {reader} sandbox:{subdocument}")
        .unwrap()
        .add_asset("docs/data.txt", b"caller data".to_vec())
        .unwrap()
        .add_loadable_library("reader", ".read {data.txt}")
        .build()
        .unwrap();
    let result = compile(&project, &CompileOptions::default());
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(paragraph_text(&result.ir), "caller data");
}
