from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"expected exactly one {label} replacement, found {count}")
    return text.replace(old, new, 1)


path = Path("crates/arkst-engine/src/evaluator.rs")
text = path.read_text(encoding="utf-8")

text = replace_once(
    text,
    '''const COLLECTION_ACCESS_NATIVE_NAMES: &[&str] = &[
    "size",
    "first",
    "second",
    "third",
    "last",
    "getat",
    "sumall",
    "average",
    "distinct",
    "reversed",
    "groupvalues",
];''',
    '''const COLLECTION_ACCESS_NATIVE_NAMES: &[&str] = &[
    "size",
    "first",
    "second",
    "third",
    "last",
    "getat",
    "sumall",
    "average",
    "distinct",
    "reversed",
    "groupvalues",
    "prepended",
    "appended",
];''',
    "collection owner",
)

text = replace_once(
    text,
    '''        "first" | "second" | "third" | "last" | "sumall" | "average" | "distinct" | "reversed"
        | "groupvalues" => (
            vec![ParameterMetadata::required("from")],
            BodyPolicy::Reject,
        ),
        "getat" => (''',
    '''        "first" | "second" | "third" | "last" | "sumall" | "average" | "distinct" | "reversed"
        | "groupvalues" => (
            vec![ParameterMetadata::required("from")],
            BodyPolicy::Reject,
        ),
        "prepended" | "appended" => (
            vec![
                ParameterMetadata::required("to"),
                ParameterMetadata::required("value"),
            ],
            BodyPolicy::Reject,
        ),
        "getat" => (''',
    "binding signature",
)

marker = '''            "getat" => {
                let (value, index, fallback) = match getat_operands('''
affix_arm = '''            "prepended" | "appended" => {
                let (collection, value) = match collection_affix_operands(
                    name,
                    positional_args,
                    named_args,
                    binding_plan,
                    span,
                    diagnostics,
                ) {
                    Ok(operands) => operands,
                    Err(outcome) => return outcome,
                };
                let mut elements = match self.coerce_iterable(collection, span, diagnostics, context) {
                    Ok(elements) => elements,
                    Err(outcome) => return outcome,
                };
                if let Err(error) = elements.try_reserve(1) {
                    diagnostics.push(iteration_error(
                        format!("`.{name}` collection cannot be allocated: {error}"),
                        *span,
                    ));
                    return CallOutcome::Failed;
                }
                if name == "prepended" {
                    elements.insert(0, value.value);
                } else {
                    elements.push(value.value);
                }
                CallOutcome::Value(IrValue::Collection(elements))
            }
'''
text = replace_once(text, marker, affix_arm + marker, "collection dispatch")

helper_marker = '''fn range_arguments(
    positional_args: &[IrValue],'''
helper = '''fn collection_affix_operands(
    name: &str,
    positional_args: &[InvocationValue],
    named_args: &[InvocationNamedArg],
    binding_plan: &BindingPlan,
    span: &SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<(InvocationValue, InvocationValue), CallOutcome> {
    let candidates = invocation_candidates(
        positional_args
            .iter()
            .map(|argument| (argument.clone(), value_source_span(&argument.value, span)))
            .collect(),
        named_args.to_vec(),
    );
    let bound = binding_plan
        .bind(&candidates, None, *span)
        .map_err(|error| {
            let message = if let Some(argument_name) = error.message.strip_prefix("unknown named argument ") {
                format!(
                    "Unknown named argument `{}` for `.{name}`",
                    argument_name.trim_matches('`')
                )
            } else if error.message == "received too many positional arguments" {
                format!(
                    "`.{name}` requires exactly two arguments (received {})",
                    positional_args.len()
                )
            } else if error.message.starts_with("missing required argument `to`") {
                format!("`.{name}` requires an iterable `to` argument")
            } else if error.message.starts_with("missing required argument `value`") {
                format!("`.{name}` requires a `value` argument")
            } else if let Some(parameter) = error
                .message
                .strip_prefix("parameter ")
                .and_then(|message| message.strip_suffix(" collides with an already bound argument"))
            {
                format!("`.{name}` received the {parameter} argument more than once")
            } else {
                error.message.clone()
            };
            let mut diagnostic = binding_diagnostic_with_code(error, "E3001");
            diagnostic.message = message;
            diagnostics.push(diagnostic);
            CallOutcome::Failed
        })?;
    let mut slots = bound.slots.into_iter();
    let Some(BoundSlot::Explicit { value: collection, .. }) = slots.next() else {
        return Err(CallOutcome::Failed);
    };
    let Some(BoundSlot::Explicit { value, .. }) = slots.next() else {
        return Err(CallOutcome::Failed);
    };
    Ok((collection, value))
}

'''
text = replace_once(text, helper_marker, helper + helper_marker, "affix helper")
path.write_text(text, encoding="utf-8")

docs = Path("docs/SYNTAX.md")
syntax = docs.read_text(encoding="utf-8")
syntax = replace_once(
    syntax,
    '''fallback. `.distinct` keeps the first occurrence. `.reversed` returns a new
typed Collection. `.groupvalues` returns a Collection of Collections in
first-seen group order, preserving order inside each group.''',
    '''fallback. `.distinct` keeps the first occurrence. `.reversed` returns a new
typed Collection. Quarkdown v2.6.0 `.prepended` and `.appended` return new
typed Collections with the supplied value at the front or back, leaving the
source iterable unchanged. `.groupvalues` returns a Collection of Collections in
first-seen group order, preserving order inside each group.''',
    "syntax docs",
)
docs.write_text(syntax, encoding="utf-8")

test = Path("crates/arkst-core/tests/quarkdown_v260_collection_affix.rs")
test.write_text(r'''use arkst_core::ir::{IrInline, IrNode};
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
''', encoding="utf-8")
