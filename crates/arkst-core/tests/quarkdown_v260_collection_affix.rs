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
fn collection_affixes_are_non_mutating_and_preserve_order() {
    let result = compile_source(
        ".var {values}\n    - B\n    - C\n\n.values::prepended {A}::first\n.values::prepended {A}::last\n.values::appended {D}::first\n.values::appended {D}::last\n.values::first\n.values::last\n.values::prepended {A}::size\n.values::appended {D}::size\n.values::size\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(output_text(&result), "A\nC\nB\nD\nB\nC\n3\n3\n2");
}

#[test]
fn collection_affixes_support_direct_and_named_invocation() {
    let result = compile_source(
        ".var {values}\n    - B\n    - C\n\n.prepended {.values} {A}::first\n.appended {.values} {D}::last\n.prepended to:{.values} value:{A}::first\n.appended to:{.values} value:{D}::last\n.range {1} {0}::prepended {A}::first\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(output_text(&result), "A\nD\nA\nD\nA");
}

#[test]
fn collection_affixes_reject_wrong_arity_and_wrong_named_parameter() {
    for source in [
        ".var {values}\n    - B\n    - C\n.prepended {.values}\n",
        ".var {values}\n    - B\n    - C\n.prepended {.values} {A} {X}\n",
        ".var {values}\n    - B\n    - C\n.prepended from:{.values} value:{A}\n",
    ] {
        let result = compile_source(source);
        assert_eq!(result.diagnostics.len(), 1, "{result:?}");
        assert_eq!(result.diagnostics[0].code, "E3001", "{result:?}");
    }
}
