use arkst_core::ir::{IrInline, IrNode};
use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};

fn compile_source(source: &str) -> arkst_core::CompileResult {
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("valid entry")
        .add_source("main.qd", source)
        .expect("valid source")
        .build()
        .expect("valid project");
    compile(&project, &CompileOptions::default())
}

fn output_text(result: &arkst_core::CompileResult) -> String {
    result
        .ir
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
fn empty_factories_compose_as_typed_immutable_values() {
    let result = compile_source(
        ".var {items} {.emptycollection}\n.var {dict} {.emptydictionary}\n\n.items::size\n.items::appended {x}::size\n.items::size\n.dict::size\n.dict::get {missing} orelse:{fallback}\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(output_text(&result), "0\n1\n0\n0\nfallback");
}

#[test]
fn empty_factories_keep_collection_dictionary_identity_and_empty_iteration() {
    let result = compile_source(
        ".ifnot {.equals {.emptycollection} to:{.emptydictionary}}\n    distinct-types\n.foreach {.emptycollection}\n    unexpected-collection\n.foreach {.emptydictionary}\n    unexpected-dictionary\nafter\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(output_text(&result), "distinct-types\nafter");
}

#[test]
fn empty_factories_reject_arguments_and_bodies_with_provenance() {
    for source in [
        ".emptycollection {x}\n",
        ".emptycollection\n    x\n",
        ".emptydictionary {x}\n",
        ".emptydictionary\n    x\n",
        ".emptycollection value:{x}\n",
    ] {
        let result = compile_source(source);
        assert_eq!(result.diagnostics.len(), 1, "{source}: {result:?}");
        assert_eq!(result.diagnostics[0].code, "E3001", "{source}: {result:?}");
        let primary = result.diagnostics[0]
            .primary
            .expect("binding diagnostic should retain source provenance");
        assert!(primary.end > primary.start, "{source}: {result:?}");
        assert!(result.ir.nodes.is_empty(), "{source}: {result:?}");
    }
}

#[test]
fn source_defined_empty_factory_names_keep_precedence() {
    let result = compile_source(
        ".function {emptycollection}\n    custom-collection\n.function {emptydictionary}\n    custom-dictionary\n\n.emptycollection\n.emptydictionary\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(output_text(&result), "custom-collection\ncustom-dictionary");
}
