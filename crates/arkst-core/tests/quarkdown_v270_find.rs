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
fn find_preserves_collection_order_duplicates_and_first_match() {
    let result = compile_source(
        ".var {numbers}\n    - 3\n    - 8\n    - 8\n    - 12\n\nfirst=.numbers::findfirst {n: .isgreater {.n} than:{5}}\n\nall:\n.numbers::findall condition:{n: .isgreater {.n} than:{5}}::foreach\n    .1\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(output_text(&result), "first=8\nall:\n8\n8\n12");
}

#[test]
fn find_reuses_typed_range_pair_and_ordered_dictionary_values() {
    let result = compile_source(
        "range-sum=.range {1} {5}::findall {n: .isgreater {.n} than:{2}}::sumall\n\npair=.pair {1} {3}::findfirst {.isgreater {.1} than:{1}}\n\n.var {table}\n    .dictionary\n        - a: 1\n        - b: 2\n        - c: 3\n.table::findall {.not {.isnone {.1}}}::foreach\n    key=.1::first value=.1::second\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(
        output_text(&result),
        "range-sum=12\npair=3\nkey=a value=1\nkey=b value=2\nkey=c value=3"
    );
}

#[test]
fn findfirst_short_circuits_and_empty_inputs_skip_the_condition() {
    let result = compile_source(
        ".var {mixed}\n    - 2\n    - nested\n        - value\n\nfirst=.mixed::findfirst {n: .isgreater {.n} than:{1}}\n\nempty-first=.emptycollection::findfirst {@lambda .error {must-not-run}}::isnone\n\nempty-all=.emptycollection::findall {@lambda .error {must-not-run}}::size\n\nno-match=.range {1} {2}::findfirst {n: .isgreater {.n} than:{5}}::isnone\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(
        output_text(&result),
        "first=2\nempty-first=true\nempty-all=0\nno-match=true"
    );
}

#[test]
fn find_rejects_invalid_binding_iterables_and_conditions_with_provenance() {
    for source in [
        ".findfirst {1..3}\n",
        ".findall {1..3} {@lambda true} {@lambda true}\n",
        ".findfirst from:{1..3} from:{1..3} condition:{@lambda true}\n",
        ".findall {1..3} condition:{@lambda true}\n    true\n",
        ".findfirst {not-iterable} {@lambda true}\n",
        ".findall {1..3} {@lambda 1}\n",
        ".findall {1..3} {n: .error {boom}}\n",
        ".var {mixed}\n    - 0\n    - 1\n    - nested\n        - value\n\n.mixed::findall {n: .isgreater {.n} than:{5}}\n",
    ] {
        let result = compile_source(source);
        assert_eq!(result.diagnostics.len(), 1, "{source}: {result:?}");
        let primary = result.diagnostics[0]
            .primary
            .expect("diagnostic should retain source provenance");
        assert!(primary.end > primary.start, "{source}: {result:?}");
        assert!(result.ir.nodes.is_empty(), "{source}: {result:?}");
    }
}

#[test]
fn find_reuses_materialization_limits() {
    let options = CompileOptions {
        compatibility_profile: None,
        evaluation_limits: EvaluationLimits {
            max_materialized_elements: 2,
            max_evaluation_depth: 16,
        },
    };
    let result = compile_source_with_options(".range {1} {3}::findall {@lambda true}\n", &options);
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert_eq!(result.diagnostics[0].code, "E3005");
    assert!(result.ir.nodes.is_empty(), "{result:?}");
}

#[test]
fn source_defined_find_names_keep_precedence() {
    let result = compile_source(
        ".function {findfirst}\n    from condition:\n    .from\n.function {findall}\n    from condition:\n    .from\n.var {chain} {shadow-chain}\n\n.findfirst {shadow-first} {not-a-lambda}\n.findall {shadow-all} {not-a-lambda}\n.chain::findfirst {not-a-lambda}\n",
    );
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(
        output_text(&result),
        "shadow-first\nshadow-all\nshadow-chain"
    );
}
