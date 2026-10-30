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

fn paragraph_text(nodes: &[IrNode]) -> Vec<String> {
    nodes
        .iter()
        .filter_map(|node| match node {
            IrNode::Paragraph { content, .. } => Some(inline_text(content)),
            _ => None,
        })
        .collect()
}

fn inline_text(inlines: &[IrInline]) -> String {
    fn collect(inlines: &[IrInline], output: &mut String) {
        for inline in inlines {
            match inline {
                IrInline::Text { content, .. } | IrInline::Code { content, .. } => {
                    output.push_str(content)
                }
                IrInline::Emphasis { content, .. }
                | IrInline::Strong { content, .. }
                | IrInline::Strikethrough { content, .. }
                | IrInline::Link { content, .. } => collect(content, output),
                _ => {}
            }
        }
    }
    let mut output = String::new();
    collect(inlines, &mut output);
    output
}

#[test]
fn join_preserves_default_custom_and_rich_inline_boundaries() {
    let source = ".var {letters}\n    - A\n    - B\n    - C\n\ndefault: .letters::join\n\nrich: .letters::join separator:{**|**}\n\nrange: .range {1} {3}::join separator:{/}\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(
        paragraph_text(&result.ir.nodes),
        ["default: A, B, C", "rich: A|B|C", "range: 1/2/3"]
    );

    let rich = result
        .ir
        .nodes
        .iter()
        .find_map(|node| match node {
            IrNode::Paragraph { content, .. } if inline_text(content).starts_with("rich:") => {
                Some(content)
            }
            _ => None,
        })
        .expect("rich join paragraph");
    assert_eq!(
        rich.iter()
            .filter(|inline| matches!(inline, IrInline::Strong { .. }))
            .count(),
        2
    );
}

#[test]
fn join_callbacks_preserve_rich_and_multiblock_content() {
    let source = "inline: .range {1} {2}::join separator:{/} body:{n: **.n**}\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(paragraph_text(&result.ir.nodes), ["inline: 1/2"]);
    assert_eq!(
        result
            .ir
            .nodes
            .iter()
            .flat_map(|node| match node {
                IrNode::Paragraph { content, .. } => content.iter(),
                _ => [].iter(),
            })
            .filter(|inline| matches!(inline, IrInline::Strong { .. }))
            .count(),
        2
    );

    let block = compile_source(
        ".range {1} {2}::join separator:{/}\n    n:\n    first-.n\n\n    second-.n\n",
    );
    assert!(block.diagnostics.is_empty(), "{block:?}");
    assert_eq!(
        paragraph_text(&block.ir.nodes),
        ["first-1", "second-1/first-2", "second-2"]
    );
}

#[test]
fn join_reuses_pair_dictionary_destructuring_and_scalar_materialization() {
    let source = "pair: .pair {left} {right}::join separator:{|}\n\n.var {table}\n    .dictionary\n        - a: 1\n        - b: 2\n.table::join separator:{;}\n    key value:\n    **.key**=.value\n\nscalars: .pair {.none} {.equals {1} {1}}::join separator:{|}\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(
        paragraph_text(&result.ir.nodes),
        ["pair: left|right", "a=1;b=2", "scalars: None|true"]
    );
}

#[test]
fn join_materializes_dynamic_scalar_and_rich_separator_values() {
    let source = ".var {number} {.sum {3} {4}}\n.var {boolean} {.equals {1} {1}}\n.var {nothing} {.none}\n.var {rich} {**bold**}\n\nnumber: .range {1} {2}::join separator:{.number}\n\nboolean: .range {1} {2}::join separator:{.boolean}\n\nnone: .range {1} {2}::join separator:{.nothing}\n\nrich: .range {1} {2}::join separator:{.rich}\n";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(
        paragraph_text(&result.ir.nodes),
        [
            "number: 172",
            "boolean: 1true2",
            "none: 1None2",
            "rich: 1bold2"
        ]
    );
    let rich = result
        .ir
        .nodes
        .iter()
        .find_map(|node| match node {
            IrNode::Paragraph { content, .. } if inline_text(content).starts_with("rich:") => {
                Some(content)
            }
            _ => None,
        })
        .expect("rich dynamic separator paragraph");
    assert_eq!(
        rich.iter()
            .filter(|inline| matches!(inline, IrInline::Strong { .. }))
            .count(),
        1
    );
}

#[test]
fn join_evaluates_arguments_in_source_order_before_target_conversion() {
    for (source, expected) in [
        (
            ".join {not-iterable} separator:{.error {separator-evaluated}}\n",
            "separator-evaluated",
        ),
        (
            ".join separator:{.error {separator-first}} iterable:{.error {iterable-second}}\n",
            "separator-first",
        ),
        (
            ".join iterable:{.error {iterable-first}} separator:{.error {separator-second}}\n",
            "iterable-first",
        ),
    ] {
        let result = compile_source(source);
        assert_eq!(result.diagnostics.len(), 1, "{source}: {result:?}");
        assert!(
            result.diagnostics[0].message.contains(expected),
            "{source}: {result:?}"
        );
        assert!(result.ir.nodes.is_empty(), "{source}: {result:?}");
    }
}

#[test]
fn join_empty_and_singleton_inputs_keep_eager_separator_and_lazy_body() {
    let successful = compile_source(
        "empty: .emptycollection::join body:{@lambda .error {must-not-run}}\n\nsingle: .range {1} {1}::join separator:{**never**} body:{n: **.n**}\n",
    );
    assert!(successful.diagnostics.is_empty(), "{successful:?}");
    assert_eq!(
        paragraph_text(&successful.ir.nodes),
        ["empty: ", "single: 1"]
    );
    assert!(successful.ir.nodes.iter().all(|node| {
        !matches!(node, IrNode::Paragraph { content, .. } if inline_text(content).contains("never"))
    }));

    for source in [
        ".emptycollection::join separator:{.error {separator-evaluated}}\n",
        ".range {1} {1}::join separator:{.error {separator-evaluated}}\n",
    ] {
        let result = compile_source(source);
        assert_eq!(result.diagnostics.len(), 1, "{source}: {result:?}");
        assert!(result.ir.nodes.is_empty(), "{source}: {result:?}");
    }
}

#[test]
fn join_rejects_invalid_binding_conversion_and_callbacks_atomically() {
    for source in [
        ".join\n",
        ".join {not-iterable}\n",
        ".range {1} {2}::join unknown:{x}\n",
        ".range {1} {2}::join separator:{|} separator:{/}\n",
        ".range {1} {2}::join body:{@lambda .1} body:{@lambda .1}\n",
        ".join {.range {1} {2}} {|} {@lambda .1} {extra}\n",
        ".range {1} {2}::join body:{a b: .a}\n",
        ".range {1} {2}::join body:{n: .error {boom-.n}}\n",
    ] {
        let result = compile_source(source);
        assert_eq!(result.diagnostics.len(), 1, "{source}: {result:?}");
        let primary = result.diagnostics[0]
            .primary
            .expect("join failure should retain source provenance");
        assert!(primary.end > primary.start, "{source}: {result:?}");
        assert!(result.ir.nodes.is_empty(), "{source}: {result:?}");
    }
}

#[test]
fn join_reuses_materialization_limits() {
    let options = CompileOptions {
        compatibility_profile: None,
        evaluation_limits: EvaluationLimits {
            max_materialized_elements: 2,
            max_evaluation_depth: 16,
        },
    };
    let result = compile_source_with_options(".range {1} {3}::join\n", &options);
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert_eq!(result.diagnostics[0].code, "E3005");
    assert!(result.ir.nodes.is_empty(), "{result:?}");
}

#[test]
fn source_defined_join_keeps_direct_and_chained_precedence() {
    let result = compile_source(
        ".function {join}\n    iterable separator? body?:\n    custom-.iterable::size\n\n.var {values} {.range {1} {2}}\ndirect: .join {.values}\n\nchained: .values::join\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(
        paragraph_text(&result.ir.nodes),
        ["direct: custom-2", "chained: custom-2"]
    );
}
