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
fn withindices_uses_one_based_typed_keys_and_preserves_duplicates() {
    let result = compile_source(
        ".var {letters}\n    - A\n    - A\n    - B\n\n.letters::withindices::foreach\n    index=.1::first next=.1::first::sum {1} value=.1::second\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(
        output_text(&result),
        "index=1 next=2 value=A\nindex=2 next=3 value=A\nindex=3 next=4 value=B"
    );
}

#[test]
fn withindices_reuses_range_pair_and_ordered_dictionary_iteration() {
    let result = compile_source(
        "range:\n.range {2} {4}::withindices::foreach\n    [.1::first=.1::second]\n\npair:\n.pair {left} {right}::withindices::foreach\n    [.1::first=.1::second]\n\n.var {table}\n    .dictionary\n        - a: A\n        - b: B\n.table::withindices::foreach\n    index=.1::first key=.1::second::first value=.1::second::second\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(
        output_text(&result),
        "range:\n[1=2]\n[2=3]\n[3=4]\npair:\n[1=left]\n[2=right]\nindex=1 key=a value=A\nindex=2 key=b value=B"
    );
}

#[test]
fn withindices_accepts_empty_and_source_backed_list_body_inputs() {
    let result = compile_source(
        "empty=.emptycollection::withindices::size\n\n.var {indexed}\n    .withindices\n        - A\n        - B\n.indexed::foreach\n    [.1::first=.1::second]\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(output_text(&result), "empty=0\n[1=A]\n[2=B]");
}

#[test]
fn withindices_rejects_invalid_binding_and_iterable_inputs_with_provenance() {
    for source in [
        ".withindices\n",
        ".withindices {abc}\n",
        ".withindices {.emptycollection} {.emptycollection}\n",
        ".withindices from:{.emptycollection} from:{.emptycollection}\n",
        ".withindices {.emptycollection}\n    - A\n",
    ] {
        let result = compile_source(source);
        assert_eq!(result.diagnostics.len(), 1, "{source}: {result:?}");
        assert_eq!(result.diagnostics[0].code, "E3001", "{source}: {result:?}");
        let primary = result.diagnostics[0]
            .primary
            .expect("diagnostic should retain source provenance");
        assert!(primary.end > primary.start, "{source}: {result:?}");
        assert!(result.ir.nodes.is_empty(), "{source}: {result:?}");
    }
}

#[test]
fn source_defined_withindices_keeps_precedence() {
    let result = compile_source(
        ".function {withindices}\n    from:\n    shadow-.from::size\n\n.withindices {.emptycollection}\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(output_text(&result), "shadow-0");
}
