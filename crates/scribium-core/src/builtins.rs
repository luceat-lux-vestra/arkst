//! Small, deterministic evaluator builtins used by the current semantic slice.

use crate::ir::{IrInline, IrNamedArg, IrNode, IrValue};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BuiltinError {
    pub message: String,
}

/// Returns whether this builtin has an evaluator implementation.
pub(crate) fn is_supported(name: &str) -> bool {
    matches!(
        name,
        "sum"
            | "subtract"
            | "multiply"
            | "divide"
            | "rem"
            | "pow"
            | "abs"
            | "negate"
            | "sqrt"
            | "logn"
            | "pi"
            | "sin"
            | "cos"
            | "tan"
            | "truncate"
            | "round"
            | "iseven"
            | "string"
            | "concatenate"
            | "uppercase"
            | "lowercase"
            | "capitalize"
            | "isempty"
            | "isnotempty"
            | "startswith"
            | "plaintext"
            | "none"
            | "otherwise"
            | "isnone"
            | "islower"
            | "isgreater"
            | "equals"
            | "not"
    )
}

/// Evaluates one supported builtin without source or backend conversion.
pub(crate) fn evaluate(
    name: &str,
    positional_args: &[IrValue],
    named_args: &[IrNamedArg],
    has_body: bool,
) -> Result<IrValue, BuiltinError> {
    match name {
        "sum" | "subtract" | "multiply" | "divide" | "rem" | "pow" => {
            evaluate_numeric(name, positional_args, named_args, has_body)
        }
        "abs" | "negate" | "sqrt" | "iseven" => {
            evaluate_unary_numeric(name, positional_args, named_args, has_body)
        }
        "logn" | "sin" | "cos" | "tan" => {
            evaluate_transcendental(name, positional_args, named_args, has_body)
        }
        "pi" => evaluate_pi(positional_args, named_args, has_body),
        "truncate" => evaluate_truncate(positional_args, named_args, has_body),
        "round" => evaluate_round(positional_args, named_args, has_body),
        "string" => evaluate_string(positional_args, named_args, has_body),
        "concatenate" => evaluate_concatenate(positional_args, named_args, has_body),
        "uppercase" | "lowercase" | "capitalize" => {
            evaluate_case(name, positional_args, named_args, has_body)
        }
        "isempty" | "isnotempty" => {
            evaluate_empty_check(name, positional_args, named_args, has_body)
        }
        "startswith" => evaluate_startswith(positional_args, named_args, has_body),
        "plaintext" => evaluate_plaintext(positional_args, named_args, has_body),
        "none" => evaluate_none(positional_args, named_args, has_body),
        "otherwise" => evaluate_otherwise(positional_args, named_args, has_body),
        "isnone" => evaluate_isnone(positional_args, named_args, has_body),
        "islower" | "isgreater" => evaluate_ordering(name, positional_args, named_args, has_body),
        "equals" => evaluate_equals(positional_args, named_args, has_body),
        "not" => evaluate_not(positional_args, named_args, has_body),
        _ => Err(error(format!("`.{name}` has no builtin implementation"))),
    }
}

fn evaluate_ordering(
    name: &str,
    positional_args: &[IrValue],
    named_args: &[IrNamedArg],
    has_body: bool,
) -> Result<IrValue, BuiltinError> {
    let mut arguments = bind_arguments(
        name,
        positional_args,
        named_args,
        &["a", "than", "orequals"],
        3,
    )?;
    if has_body {
        return Err(error(format!("`.{name}` does not accept a block body")));
    }

    let a = arguments
        .remove(0)
        .ok_or_else(|| error(format!("`.{name}` requires a first numeric argument")))?;
    let b = arguments
        .remove(0)
        .ok_or_else(|| error(format!("`.{name}` requires a `than` numeric argument")))?;
    let orequals = arguments
        .remove(0)
        .map(|value| boolean_argument(&value, "orequals"))
        .transpose()?
        .unwrap_or(false);
    let a = numeric_argument(&a, "a")?;
    let b = numeric_argument(&b, "than")?;
    let result = if name == "islower" {
        if orequals {
            a <= b
        } else {
            a < b
        }
    } else if orequals {
        a >= b
    } else {
        a > b
    };
    Ok(IrValue::Boolean(result))
}

fn evaluate_equals(
    positional_args: &[IrValue],
    named_args: &[IrNamedArg],
    has_body: bool,
) -> Result<IrValue, BuiltinError> {
    if has_body {
        return Err(error("`.equals` does not accept a block body".to_string()));
    }
    let mut arguments = bind_arguments("equals", positional_args, named_args, &["a", "to"], 2)?;
    let left = arguments
        .remove(0)
        .ok_or_else(|| error("`.equals` requires a first value argument".to_string()))?;
    let right = arguments
        .remove(0)
        .ok_or_else(|| error("`.equals` requires a `to` value argument".to_string()))?;
    Ok(IrValue::Boolean(values_equal(&left, &right)))
}

fn evaluate_not(
    positional_args: &[IrValue],
    named_args: &[IrNamedArg],
    has_body: bool,
) -> Result<IrValue, BuiltinError> {
    if has_body {
        return Err(error("`.not` does not accept a block body".to_string()));
    }
    let mut arguments = bind_arguments("not", positional_args, named_args, &["value"], 1)?;
    let value = arguments
        .remove(0)
        .ok_or_else(|| error("`.not` requires exactly one boolean argument".to_string()))?;
    Ok(IrValue::Boolean(!boolean_argument(&value, "value")?))
}

fn evaluate_string(
    positional_args: &[IrValue],
    named_args: &[IrNamedArg],
    has_body: bool,
) -> Result<IrValue, BuiltinError> {
    if has_body {
        return Err(error("`.string` does not accept a block body".to_string()));
    }
    let mut arguments = bind_arguments("string", positional_args, named_args, &["value"], 1)?;
    let value = arguments
        .remove(0)
        .ok_or_else(|| error("`.string` requires one value argument".to_string()))?;
    let text = adapt_string_argument(&value).ok_or_else(|| {
        error("`.string` requires a scalar value that can adapt to text".to_string())
    })?;
    Ok(IrValue::String(text))
}

fn evaluate_concatenate(
    positional_args: &[IrValue],
    named_args: &[IrNamedArg],
    has_body: bool,
) -> Result<IrValue, BuiltinError> {
    if has_body {
        return Err(error(
            "`.concatenate` does not accept a block body".to_string(),
        ));
    }
    let mut arguments = bind_arguments(
        "concatenate",
        positional_args,
        named_args,
        &["a", "with", "if"],
        3,
    )?;
    let a = arguments
        .remove(0)
        .ok_or_else(|| error("`.concatenate` requires an `a` string argument".to_string()))?;
    let with = arguments
        .remove(0)
        .ok_or_else(|| error("`.concatenate` requires a `with` string argument".to_string()))?;
    let condition = arguments
        .remove(0)
        .map(|value| boolean_argument(&value, "if"))
        .transpose()?
        .unwrap_or(true);
    let a = adapt_string_argument(&a)
        .ok_or_else(|| error("`.concatenate` argument `a` cannot adapt to text".to_string()))?;
    let with = adapt_string_argument(&with)
        .ok_or_else(|| error("`.concatenate` argument `with` cannot adapt to text".to_string()))?;
    if condition {
        Ok(IrValue::String(format!("{a}{with}")))
    } else {
        Ok(IrValue::String(a))
    }
}

fn evaluate_empty_check(
    name: &str,
    positional_args: &[IrValue],
    named_args: &[IrNamedArg],
    has_body: bool,
) -> Result<IrValue, BuiltinError> {
    if has_body {
        return Err(error(format!("`.{name}` does not accept a block body")));
    }
    let mut arguments = bind_arguments(name, positional_args, named_args, &["string"], 1)?;
    let value = arguments
        .remove(0)
        .ok_or_else(|| error(format!("`.{name}` requires one string argument")))?;
    let text = adapt_string_argument(&value).ok_or_else(|| {
        error(format!(
            "`.{name}` requires a scalar value that can adapt to text"
        ))
    })?;
    let is_empty = text.is_empty();
    Ok(IrValue::Boolean(if name == "isempty" {
        is_empty
    } else {
        !is_empty
    }))
}

fn evaluate_startswith(
    positional_args: &[IrValue],
    named_args: &[IrNamedArg],
    has_body: bool,
) -> Result<IrValue, BuiltinError> {
    if has_body {
        return Err(error(
            "`.startswith` does not accept a block body".to_string(),
        ));
    }
    let mut arguments = bind_arguments(
        "startswith",
        positional_args,
        named_args,
        &["string", "prefix", "ignorecase"],
        3,
    )?;
    let string = arguments
        .remove(0)
        .ok_or_else(|| error("`.startswith` requires a `string` argument".to_string()))?;
    let prefix = arguments
        .remove(0)
        .ok_or_else(|| error("`.startswith` requires a `prefix` argument".to_string()))?;
    let ignorecase = arguments
        .remove(0)
        .map(|value| boolean_argument(&value, "ignorecase"))
        .transpose()?
        .unwrap_or(false);
    let string = adapt_string_argument(&string)
        .ok_or_else(|| error("`.startswith` argument `string` cannot adapt to text".to_string()))?;
    let prefix = adapt_string_argument(&prefix)
        .ok_or_else(|| error("`.startswith` argument `prefix` cannot adapt to text".to_string()))?;
    let result = if ignorecase {
        string.to_lowercase().starts_with(&prefix.to_lowercase())
    } else {
        string.starts_with(&prefix)
    };
    Ok(IrValue::Boolean(result))
}

fn evaluate_plaintext(
    positional_args: &[IrValue],
    named_args: &[IrNamedArg],
    has_body: bool,
) -> Result<IrValue, BuiltinError> {
    if has_body {
        return Err(error(
            "`.plaintext` body must bind as `content` before evaluation".to_string(),
        ));
    }
    let mut arguments = bind_arguments("plaintext", positional_args, named_args, &["content"], 1)?;
    let content = arguments
        .remove(0)
        .ok_or_else(|| error("`.plaintext` requires one content argument".to_string()))?;
    let text = plain_text_argument(&content).ok_or_else(|| {
        error(
            "`.plaintext` requires already-parsed inline content or a supported scalar value"
                .to_string(),
        )
    })?;
    Ok(IrValue::String(text))
}

fn bind_arguments(
    name: &str,
    positional_args: &[IrValue],
    named_args: &[IrNamedArg],
    parameter_names: &[&str],
    max_arguments: usize,
) -> Result<Vec<Option<IrValue>>, BuiltinError> {
    if positional_args.len() > max_arguments {
        return Err(error(format!(
            "`.{name}` received too many positional arguments"
        )));
    }
    let mut arguments = vec![None; parameter_names.len()];
    for (index, value) in positional_args.iter().enumerate() {
        arguments[index] = Some(value.clone());
    }
    for argument in named_args {
        let Some(index) = parameter_names
            .iter()
            .position(|parameter| *parameter == argument.name)
        else {
            return Err(error(format!(
                "`.{name}` does not support named argument `{}`",
                argument.name
            )));
        };
        if arguments[index].is_some() {
            return Err(error(format!(
                "`.{name}` received argument `{}` more than once",
                argument.name
            )));
        }
        arguments[index] = Some(argument.value.clone());
    }
    Ok(arguments)
}

fn numeric_argument(value: &IrValue, parameter: &str) -> Result<f32, BuiltinError> {
    Ok(numeric_argument_value(value, parameter)? as f32)
}

fn numeric_argument_value(value: &IrValue, parameter: &str) -> Result<f64, BuiltinError> {
    match value {
        IrValue::Number(number) => Ok(*number),
        IrValue::String(value) | IrValue::Identifier(value) => value
            .parse::<i32>()
            .map(f64::from)
            .ok()
            .or_else(|| value.parse::<f32>().ok().map(f64::from))
            .ok_or_else(|| error(format!("`{parameter}` must be numeric"))),
        _ => Err(error(format!("`{parameter}` must be numeric"))),
    }
}

fn boolean_argument(value: &IrValue, parameter: &str) -> Result<bool, BuiltinError> {
    match value {
        IrValue::Boolean(value) => Ok(*value),
        IrValue::String(value) | IrValue::Identifier(value) => {
            match value.to_ascii_lowercase().as_str() {
                "true" | "yes" => Ok(true),
                "false" | "no" => Ok(false),
                _ => Err(error(format!("`{parameter}` must be boolean"))),
            }
        }
        _ => Err(error(format!("`{parameter}` must be boolean"))),
    }
}

fn values_equal(left: &IrValue, right: &IrValue) -> bool {
    if left == right {
        return true;
    }
    match (comparable_plain_text(left), comparable_plain_text(right)) {
        (Some(left), Some(right)) => left == right,
        _ => false,
    }
}

/// Mirrors the public equality helper's plain-text fallback without turning
/// structured values into text for unrelated operations. Rich content is
/// projected only because upstream `.equals` explicitly compares its plain
/// text representation as a final equality branch.
fn comparable_plain_text(value: &IrValue) -> Option<String> {
    match value {
        IrValue::String(value) | IrValue::Identifier(value) => Some(value.clone()),
        IrValue::Number(value) => Some(value.to_string()),
        IrValue::Content(nodes) => {
            let mut text = String::new();
            for node in nodes {
                append_node_plain_text(node, &mut text)?;
            }
            Some(text)
        }
        _ => None,
    }
}

fn append_node_plain_text(node: &IrNode, output: &mut String) -> Option<()> {
    match node {
        IrNode::Paragraph { content, .. } | IrNode::Heading { content, .. } => {
            for inline in content {
                append_inline_plain_text(inline, output)?;
            }
        }
        IrNode::Blockquote { content, .. } => {
            for child in content {
                append_node_plain_text(child, output)?;
            }
        }
        IrNode::UnorderedList { items, .. } | IrNode::OrderedList { items, .. } => {
            for item in items {
                for child in &item.nodes {
                    append_node_plain_text(child, output)?;
                }
            }
        }
        IrNode::Table { header, rows, .. } => {
            append_row_plain_text(header, output)?;
            for row in rows {
                append_row_plain_text(row, output)?;
            }
        }
        IrNode::CodeBlock { source, .. } => output.push_str(source),
        IrNode::FunctionCall { .. }
        | IrNode::ChainedFunctionCall { .. }
        | IrNode::FunctionDeclaration { .. }
        | IrNode::RawTypst { .. }
        | IrNode::RawHtml { .. }
        | IrNode::TargetSpecificContent { .. }
        | IrNode::ThematicBreak { .. }
        | IrNode::Math { .. } => return None,
    }
    Some(())
}

fn append_row_plain_text(row: &crate::ir::IrTableRow, output: &mut String) -> Option<()> {
    for cell in &row.cells {
        for inline in &cell.content {
            append_inline_plain_text(inline, output)?;
        }
    }
    Some(())
}

fn append_inline_plain_text(inline: &IrInline, output: &mut String) -> Option<()> {
    match inline {
        IrInline::Text { content, .. } | IrInline::Code { content, .. } => output.push_str(content),
        IrInline::Emphasis { content, .. }
        | IrInline::Strong { content, .. }
        | IrInline::Strikethrough { content, .. }
        | IrInline::Link { content, .. } => {
            for child in content {
                append_inline_plain_text(child, output)?;
            }
        }
        IrInline::SoftBreak { .. } | IrInline::HardBreak { .. } => output.push('\n'),
        IrInline::DirectiveCall { .. }
        | IrInline::ChainedDirectiveCall { .. }
        | IrInline::RawHtml { .. }
        | IrInline::TargetSpecificContent { .. } => return None,
        IrInline::Image { .. } => return None,
    }
    Some(())
}

/// Projects the bounded `.plaintext` input contract without converting a
/// value back into source text or invoking a Markdown parser.
fn plain_text_argument(value: &IrValue) -> Option<String> {
    match value {
        IrValue::Identifier(value) => Some(value.clone()),
        IrValue::Boolean(value) => Some(value.to_string()),
        IrValue::Number(value) => Some(value.to_string()),
        IrValue::Content(nodes) => plain_text_from_content(nodes),
        IrValue::String(_)
        | IrValue::Range(_)
        | IrValue::Collection(_)
        | IrValue::Pair(_)
        | IrValue::Dictionary(_)
        | IrValue::Callable(_)
        | IrValue::None => None,
    }
}

fn plain_text_from_content(nodes: &[IrNode]) -> Option<String> {
    let mut output = String::new();
    for node in nodes {
        let IrNode::Paragraph { content, .. } = node else {
            return None;
        };
        plain_text_from_inlines(content, &mut output)?;
    }
    Some(output)
}

fn plain_text_from_inlines(inlines: &[IrInline], output: &mut String) -> Option<()> {
    for inline in inlines {
        match inline {
            IrInline::Text { content, .. } | IrInline::Code { content, .. } => {
                output.push_str(content);
            }
            IrInline::Emphasis { content, .. }
            | IrInline::Strong { content, .. }
            | IrInline::Strikethrough { content, .. }
            | IrInline::Link { content, .. } => plain_text_from_inlines(content, output)?,
            IrInline::SoftBreak { .. } => output.push('\n'),
            // v2.5.1 `NodeUtils.toPlainText()` does not emit hard-break text.
            IrInline::HardBreak { .. } | IrInline::Image { .. } => {}
            IrInline::DirectiveCall { .. }
            | IrInline::ChainedDirectiveCall { .. }
            | IrInline::RawHtml { .. }
            | IrInline::TargetSpecificContent { .. } => {
                return None;
            }
        }
    }
    Some(())
}

fn evaluate_otherwise(
    positional_args: &[IrValue],
    named_args: &[IrNamedArg],
    has_body: bool,
) -> Result<IrValue, BuiltinError> {
    if has_body || !named_args.is_empty() || positional_args.len() != 2 {
        return Err(error(
            "`.otherwise` requires exactly two positional arguments".to_string(),
        ));
    }
    if matches!(positional_args[0], IrValue::None) {
        Ok(positional_args[1].clone())
    } else {
        Ok(positional_args[0].clone())
    }
}

fn evaluate_none(
    positional_args: &[IrValue],
    named_args: &[IrNamedArg],
    has_body: bool,
) -> Result<IrValue, BuiltinError> {
    if has_body || !named_args.is_empty() || !positional_args.is_empty() {
        return Err(error(
            "`.none` does not accept arguments or a block body".to_string(),
        ));
    }
    Ok(IrValue::None)
}

fn evaluate_isnone(
    positional_args: &[IrValue],
    named_args: &[IrNamedArg],
    has_body: bool,
) -> Result<IrValue, BuiltinError> {
    if has_body || !named_args.is_empty() || positional_args.len() != 1 {
        return Err(error(
            "`.isnone` requires exactly one positional argument".to_string(),
        ));
    }
    Ok(IrValue::Boolean(matches!(
        positional_args[0],
        IrValue::None
    )))
}

fn evaluate_numeric(
    name: &str,
    positional_args: &[IrValue],
    named_args: &[IrNamedArg],
    has_body: bool,
) -> Result<IrValue, BuiltinError> {
    if has_body {
        return Err(error(format!(
            "`.{name}` does not accept a block body in this evaluator slice"
        )));
    }

    let parameter_names = match name {
        "sum" | "subtract" | "rem" => ["a", "b"],
        "multiply" | "divide" => ["a", "by"],
        "pow" => ["base", "to"],
        _ => unreachable!("unrecognized binary numeric builtin: {name}"),
    };
    let mut arguments = bind_arguments(name, positional_args, named_args, &parameter_names, 2)?;
    let first = arguments
        .remove(0)
        .ok_or_else(|| error(format!("`.{name}` requires numeric arguments")))?;
    let second = arguments
        .remove(0)
        .ok_or_else(|| error(format!("`.{name}` requires numeric arguments")))?;
    let first = numeric_argument(&first, parameter_names[0])
        .map_err(|_| error(format!("`.{name}` requires numeric arguments")))?;
    let second = numeric_argument(&second, parameter_names[1])
        .map_err(|_| error(format!("`.{name}` requires numeric arguments")))?;

    let result = match name {
        "sum" => first + second,
        "subtract" => first - second,
        "multiply" => first * second,
        "divide" => first / second,
        "rem" => first % second,
        "pow" => first.powi(kotlin_float_to_int(second)),
        _ => unreachable!("unrecognized binary numeric builtin: {name}"),
    };
    Ok(numeric_result(result))
}

fn evaluate_unary_numeric(
    name: &str,
    positional_args: &[IrValue],
    named_args: &[IrNamedArg],
    has_body: bool,
) -> Result<IrValue, BuiltinError> {
    if has_body {
        return Err(error(format!("`.{name}` does not accept a block body")));
    }
    let mut arguments = bind_arguments(name, positional_args, named_args, &["x"], 1)?;
    let value = arguments
        .remove(0)
        .ok_or_else(|| error(format!("`.{name}` requires one numeric argument")))?;
    let value = numeric_argument(&value, "x")
        .map_err(|_| error(format!("`.{name}` requires a numeric argument")))?;

    match name {
        "abs" => Ok(numeric_result(value.abs())),
        "negate" => Ok(numeric_result(-value)),
        "sqrt" => Ok(numeric_result(value.sqrt())),
        "iseven" => Ok(IrValue::Boolean(kotlin_float_to_int(value) % 2 == 0)),
        _ => Err(error(format!(
            "`.{name}` has no unary numeric implementation"
        ))),
    }
}

fn evaluate_transcendental(
    name: &str,
    positional_args: &[IrValue],
    named_args: &[IrNamedArg],
    has_body: bool,
) -> Result<IrValue, BuiltinError> {
    if has_body {
        return Err(error(format!("`.{name}` does not accept a block body")));
    }
    let mut arguments = bind_arguments(name, positional_args, named_args, &["x"], 1)?;
    let value = arguments
        .remove(0)
        .ok_or_else(|| error(format!("`.{name}` requires one numeric argument")))?;
    let value = numeric_argument(&value, "x")
        .map_err(|_| error(format!("`.{name}` requires a numeric argument")))?;
    Ok(numeric_result(deterministic_transcendental(name, value)))
}

fn evaluate_pi(
    positional_args: &[IrValue],
    named_args: &[IrNamedArg],
    has_body: bool,
) -> Result<IrValue, BuiltinError> {
    if has_body {
        return Err(error("`.pi` does not accept a block body".to_string()));
    }
    bind_arguments("pi", positional_args, named_args, &[], 0)?;

    // Quarkdown passes kotlin.math.PI as a Double to NumberValue. Keep this
    // binary64 constant separate from the Float result normalization used by
    // arithmetic and transcendental builtins.
    Ok(IrValue::Number(std::f64::consts::PI))
}

/// Reproduces the JVM observable boundary of Kotlin's Float overloads without
/// calling platform `std` math. Kotlin/JVM first adapts the argument to Float,
/// calls `java.lang.Math` on the widened binary64 value, and narrows the result
/// back to Float. `libm` is pinned and built with no default features so these
/// operations use its pure-Rust software implementations on native and WASM
/// targets rather than an OS libc/libm or a target-specific math intrinsic.
fn deterministic_transcendental(name: &str, value: f32) -> f32 {
    let value = f64::from(value);
    let result = match name {
        "logn" => libm::log(value),
        "sin" => libm::sin(value),
        "cos" => libm::cos(value),
        "tan" => libm::tan(value),
        _ => unreachable!("unrecognized transcendental builtin: {name}"),
    };
    result as f32
}

fn evaluate_truncate(
    positional_args: &[IrValue],
    named_args: &[IrNamedArg],
    has_body: bool,
) -> Result<IrValue, BuiltinError> {
    if has_body {
        return Err(error(
            "`.truncate` does not accept a block body".to_string(),
        ));
    }
    let mut arguments = bind_arguments(
        "truncate",
        positional_args,
        named_args,
        &["x", "decimals"],
        2,
    )?;
    let value = arguments
        .remove(0)
        .ok_or_else(|| error("`.truncate` requires a numeric `x` argument".to_string()))?;
    let decimals = arguments
        .remove(0)
        .ok_or_else(|| error("`.truncate` requires an integer `decimals` argument".to_string()))?;
    let value = numeric_argument(&value, "x")
        .map_err(|_| error("`.truncate` requires a numeric `x` argument".to_string()))?;
    let decimals = integer_argument(&decimals, "decimals")?;
    if decimals < 0 {
        return Err(error(
            "`.truncate` requires non-negative `decimals`".to_string(),
        ));
    }

    // NumberValue turns every integral Float (including infinities after
    // Kotlin's clamped toInt conversion) into an Int before truncate sees it.
    // Keep that branch separate from the floating post-processing formula.
    if decimals == 0 || number_value_is_integral(value) {
        return Ok(IrValue::Number(f64::from(kotlin_float_to_int(value))));
    }

    // This deliberately follows Math.kt's observable boundaries:
    // x.toFloat() * (10.0.pow(decimals)) is Double arithmetic, the product is
    // converted with Double.toInt(), and the final division is Float.
    let multiplier = 10.0_f64.powi(decimals);
    let product = f64::from(value) * multiplier;
    let truncated = kotlin_double_to_int(product);
    let result = truncated as f32 / multiplier as f32;
    Ok(numeric_result(result))
}

fn evaluate_round(
    positional_args: &[IrValue],
    named_args: &[IrNamedArg],
    has_body: bool,
) -> Result<IrValue, BuiltinError> {
    if has_body {
        return Err(error("`.round` does not accept a block body".to_string()));
    }
    let mut arguments = bind_arguments("round", positional_args, named_args, &["x"], 1)?;
    let value = arguments
        .remove(0)
        .ok_or_else(|| error("`.round` requires one numeric argument".to_string()))?;
    let value = numeric_argument(&value, "x")
        .map_err(|_| error("`.round` requires a numeric argument".to_string()))?;
    Ok(IrValue::Number(f64::from(kotlin_round_to_int(value))))
}

/// Applies the `NumberValue` normalization visible at the upstream output
/// boundary: integral Float values become Int values, including Kotlin's
/// clamped conversions for infinities and out-of-range finite values.
fn numeric_result(value: f32) -> IrValue {
    if value.is_nan() {
        return IrValue::Number(f64::NAN);
    }
    if value.ceil() == value.floor() {
        return IrValue::Number(f64::from(kotlin_float_to_int(value)));
    }
    IrValue::Number(f64::from(value))
}

/// Mirrors Kotlin `Float.toInt()`: truncate toward zero, map NaN to zero,
/// and clamp finite/infinite values to the signed Int range.
fn kotlin_float_to_int(value: f32) -> i32 {
    let value = f64::from(value);
    if value.is_nan() {
        0
    } else if value <= f64::from(i32::MIN) {
        i32::MIN
    } else if value >= f64::from(i32::MAX) {
        i32::MAX
    } else {
        value.trunc() as i32
    }
}

/// Mirrors Kotlin `Double.toInt()` for the truncate multiplier product.
fn kotlin_double_to_int(value: f64) -> i32 {
    if value.is_nan() {
        0
    } else if value <= f64::from(i32::MIN) {
        i32::MIN
    } else if value >= f64::from(i32::MAX) {
        i32::MAX
    } else {
        value.trunc() as i32
    }
}

/// NumberValue's normalization predicate, including its NaN/Infinity edges.
fn number_value_is_integral(value: f32) -> bool {
    value.ceil() == value.floor()
}

/// Strictly adapts the narrow `decimals: Int` parameter boundary.
///
/// Quarkdown accepts an integral numeric representation such as `2` or `2.0`
/// after NumberValue normalization, but a fractional NumberValue and quoted
/// text are not silently converted to an Int.
fn integer_argument(value: &IrValue, parameter: &str) -> Result<i32, BuiltinError> {
    let number = match value {
        IrValue::Number(value) => *value as f32,
        IrValue::Identifier(value) => value
            .parse::<i32>()
            .map(|value| value as f32)
            .ok()
            .or_else(|| value.parse::<f32>().ok())
            .ok_or_else(|| error(format!("`{parameter}` must be an integer")))?,
        _ => return Err(error(format!("`{parameter}` must be an integer"))),
    };
    if !number_value_is_integral(number) {
        return Err(error(format!("`{parameter}` must be an integer")));
    }
    Ok(kotlin_float_to_int(number))
}

/// Reproduces Kotlin's `kotlin.math.round(Float)` followed by `toInt()`.
/// Kotlin uses ties-to-even; Rust's default `round` is not used here.
fn kotlin_round_to_int(value: f32) -> i32 {
    if !value.is_finite() || number_value_is_integral(value) {
        return kotlin_float_to_int(value);
    }

    let lower = value.floor();
    let fraction = value - lower;
    let rounded = if fraction < 0.5 {
        lower
    } else if fraction > 0.5 || lower % 2.0 != 0.0 {
        lower + 1.0
    } else {
        lower
    };
    kotlin_float_to_int(rounded)
}

fn evaluate_case(
    name: &str,
    positional_args: &[IrValue],
    named_args: &[IrNamedArg],
    has_body: bool,
) -> Result<IrValue, BuiltinError> {
    if has_body {
        return Err(error(format!("`.{name}` does not accept a block body")));
    }
    let mut arguments = bind_arguments(name, positional_args, named_args, &["string"], 1)?;
    let value = arguments
        .remove(0)
        .ok_or_else(|| error(format!("`.{name}` requires one string argument")))?;
    let text = adapt_string_argument(&value).ok_or_else(|| {
        error(format!(
            "`.{name}` requires a scalar value that can adapt to text"
        ))
    })?;
    let transformed = match name {
        "uppercase" => text.to_uppercase(),
        "lowercase" => text.to_lowercase(),
        "capitalize" => {
            let mut characters = text.chars();
            let Some(first) = characters.next() else {
                return Ok(IrValue::String(text));
            };
            let mut result = first.to_uppercase().collect::<String>();
            result.push_str(characters.as_str());
            result
        }
        _ => return Err(error(format!("`.{name}` has no case transformation"))),
    };
    Ok(IrValue::String(transformed))
}

fn error(message: String) -> BuiltinError {
    BuiltinError { message }
}

/// Applies the small invocation-boundary text adaptation contract used by the
/// evidenced string builtins. Plain text content is adapted structurally; rich
/// content is not rendered or round-tripped through a backend.
pub(crate) fn adapt_string_argument(value: &IrValue) -> Option<String> {
    match value {
        IrValue::String(text) | IrValue::Identifier(text) => Some(text.clone()),
        IrValue::Boolean(value) => Some(value.to_string()),
        IrValue::Number(value) => Some(value.to_string()),
        IrValue::None => None,
        IrValue::Range(_)
        | IrValue::Collection(_)
        | IrValue::Pair(_)
        | IrValue::Dictionary(_)
        | IrValue::Callable(_) => None,
        IrValue::Content(nodes) => {
            let mut text = String::new();
            for node in nodes {
                let IrNode::Paragraph { content, .. } = node else {
                    return None;
                };
                for inline in content {
                    let IrInline::Text { content, .. } = inline else {
                        return None;
                    };
                    text.push_str(content);
                }
            }
            Some(text)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{deterministic_transcendental, evaluate, is_supported};
    use crate::ir::{IrCallable, IrDictionary, IrInline, IrNode, IrPair, IrRange, IrValue};
    use crate::source::{SourceId, SourceSpan};

    fn number(value: f64) -> IrValue {
        IrValue::Number(value)
    }

    fn named_arg(name: &str, value: IrValue) -> crate::ir::IrNamedArg {
        crate::ir::IrNamedArg {
            name: name.to_string(),
            name_span: crate::source::SourceSpan::new(crate::source::SourceId(0), 0, 0),
            value,
            span: crate::source::SourceSpan::new(crate::source::SourceId(0), 0, 0),
        }
    }

    #[test]
    fn numeric_surface_is_registered_and_preserves_typed_results() {
        for name in [
            "sum", "subtract", "multiply", "divide", "rem", "pow", "abs", "negate", "sqrt", "logn",
            "pi", "sin", "cos", "tan", "truncate", "round", "iseven",
        ] {
            assert!(is_supported(name), "{name} should be supported");
        }

        assert_eq!(
            evaluate(
                "subtract",
                &[number(10.0)],
                &[named_arg("b", number(3.0))],
                false
            )
            .expect("named subtraction should evaluate"),
            number(7.0)
        );
        assert_eq!(
            evaluate(
                "multiply",
                &[number(-2.0)],
                &[named_arg("by", number(2.5))],
                false,
            )
            .expect("mixed multiplication should evaluate"),
            number(f64::from((-2.0_f32) * 2.5_f32))
        );
        assert_eq!(
            evaluate(
                "divide",
                &[],
                &[named_arg("a", number(7.0)), named_arg("by", number(2.0))],
                false,
            )
            .expect("named division should evaluate"),
            number(f64::from(3.5_f32))
        );
        assert_eq!(
            evaluate(
                "sum",
                &[IrValue::Identifier("10".into())],
                &[named_arg("b", IrValue::String("-2.5".into()))],
                false,
            )
            .expect("numeric scalar text should adapt"),
            number(f64::from(7.5_f32))
        );
        assert_eq!(
            evaluate(
                "abs",
                &[],
                &[named_arg("x", IrValue::String("-3.5".into()))],
                false,
            )
            .expect("numeric text should adapt for unary operations"),
            number(f64::from(3.5_f32))
        );
        assert_eq!(
            evaluate("negate", &[number(-2.5)], &[], false)
                .expect("negation should remain numeric"),
            number(f64::from(2.5_f32))
        );
        assert_eq!(
            evaluate("sqrt", &[number(9.0)], &[], false).expect("square root should evaluate"),
            number(3.0)
        );
        assert_eq!(
            evaluate("iseven", &[IrValue::Identifier("-4".into())], &[], false)
                .expect("evenness should return a typed boolean"),
            IrValue::Boolean(true)
        );
    }

    #[test]
    fn decimal_numeric_surface_matches_upstream_boundaries() {
        assert_eq!(
            evaluate("truncate", &[number(201.06194), number(2.0)], &[], false)
                .expect("positional decimals should bind"),
            number(f64::from(201.06_f32))
        );
        assert_eq!(
            evaluate(
                "truncate",
                &[number(201.06194)],
                &[named_arg("decimals", number(1.0))],
                false,
            )
            .expect("named decimals should bind"),
            number(201.0)
        );
        assert_eq!(
            evaluate("truncate", &[number(-1.29), number(1.0)], &[], false)
                .expect("negative truncation should use zero direction"),
            number(f64::from(-1.2_f32))
        );
        assert_eq!(
            evaluate("truncate", &[number(201.06194), number(0.0)], &[], false)
                .expect("zero decimals should use toInt"),
            number(201.0)
        );
        assert_eq!(
            evaluate("truncate", &[number(201.0), number(2.0)], &[], false)
                .expect("integral values should remain integral"),
            number(201.0)
        );
        assert_eq!(
            evaluate(
                "truncate",
                &[IrValue::String("-1.29".into()), number(1.0)],
                &[],
                false,
            )
            .expect("numeric text x should use the existing Number boundary"),
            number(f64::from(-1.2_f32))
        );
        assert_eq!(
            evaluate(
                "truncate",
                &[
                    evaluate("sum", &[number(100.0), number(1.06194)], &[], false)
                        .expect("nested numeric x should evaluate")
                ],
                &[named_arg("decimals", number(2.0))],
                false,
            )
            .expect("nested numeric result should bind as x"),
            number(f64::from(101.06_f32))
        );

        assert!(evaluate("truncate", &[number(1.0), number(1.5)], &[], false).is_err());
        assert!(evaluate("truncate", &[], &[], false).is_err());
        assert!(evaluate(
            "truncate",
            &[number(1.0), number(2.0), number(3.0)],
            &[],
            false,
        )
        .is_err());
        assert!(evaluate(
            "truncate",
            &[number(1.0)],
            &[named_arg("decimals", IrValue::String("2".into()))],
            false,
        )
        .is_err());
        assert!(evaluate(
            "truncate",
            &[number(1.0)],
            &[named_arg("decimals", IrValue::String("2.0".into()))],
            false,
        )
        .is_err());
        assert!(evaluate(
            "truncate",
            &[number(1.0)],
            &[named_arg("decimals", IrValue::String("1.5".into()))],
            false,
        )
        .is_err());
        assert!(evaluate("truncate", &[number(1.0), number(-1.0)], &[], false).is_err());
        assert!(evaluate("truncate", &[number(1.0), number(-1.5)], &[], false).is_err());
        assert!(evaluate("truncate", &[number(1.0), number(f64::NAN)], &[], false).is_err());
        assert_eq!(
            evaluate(
                "truncate",
                &[number(1.25), number(f64::INFINITY)],
                &[],
                false
            )
            .expect("infinite decimals should follow Int normalization"),
            number(0.0)
        );
        assert_eq!(
            evaluate(
                "truncate",
                &[number(1.25), number(f64::from(i32::MAX))],
                &[],
                false,
            )
            .expect("large decimals should not use a Scribium-only limit"),
            number(0.0)
        );
        assert!(evaluate(
            "truncate",
            &[number(1.0)],
            &[named_arg("unknown", number(2.0))],
            false
        )
        .is_err());
        assert!(evaluate(
            "truncate",
            &[number(1.0), number(2.0)],
            &[named_arg("x", number(3.0))],
            false
        )
        .is_err());
        assert!(evaluate("truncate", &[number(1.0), number(2.0)], &[], true).is_err());

        assert_eq!(
            evaluate("round", &[number(2.5)], &[], false).expect("2.5 should round"),
            number(2.0)
        );
        assert_eq!(
            evaluate("round", &[number(3.5)], &[], false).expect("3.5 should round"),
            number(4.0)
        );
        assert_eq!(
            evaluate("round", &[number(-2.5)], &[], false).expect("-2.5 should round"),
            number(-2.0)
        );
        assert_eq!(
            evaluate("round", &[number(-3.5)], &[], false).expect("-3.5 should round"),
            number(-4.0)
        );
        for (input, expected) in [(2.49, 2.0), (2.51, 3.0), (-2.49, -2.0), (-2.51, -3.0)] {
            assert_eq!(
                evaluate("round", &[number(input)], &[], false).expect("non-tie should round"),
                number(expected)
            );
        }
        assert_eq!(
            evaluate("round", &[], &[named_arg("x", number(2.5))], false)
                .expect("named x should bind"),
            number(2.0)
        );
        assert_eq!(
            evaluate("round", &[number(f64::NAN)], &[], false)
                .expect("NaN should follow Kotlin Float.toInt"),
            number(0.0)
        );
        assert_eq!(
            evaluate("round", &[number(f64::INFINITY)], &[], false)
                .expect("positive infinity should clamp"),
            number(f64::from(i32::MAX))
        );
        assert_eq!(
            evaluate("round", &[number(f64::NEG_INFINITY)], &[], false)
                .expect("negative infinity should clamp"),
            number(f64::from(i32::MIN))
        );
        assert_eq!(
            evaluate("round", &[number(1.0e30)], &[], false)
                .expect("large finite values should clamp"),
            number(f64::from(i32::MAX))
        );
        assert!(evaluate("round", &[IrValue::Boolean(true)], &[], false).is_err());
        assert!(evaluate("round", &[], &[], false).is_err());
        assert!(evaluate("round", &[number(1.0), number(2.0)], &[], false).is_err());
        assert!(evaluate(
            "round",
            &[number(1.0)],
            &[named_arg("x", number(2.0))],
            false
        )
        .is_err());
        assert!(evaluate(
            "round",
            &[number(1.0)],
            &[named_arg("unknown", number(2.0))],
            false
        )
        .is_err());
        assert!(evaluate("round", &[number(1.0)], &[], true).is_err());
    }

    #[test]
    fn transcendental_numeric_surface_matches_upstream_boundaries() {
        assert_eq!(
            evaluate("pi", &[], &[], false).expect("pi has zero arguments"),
            number(std::f64::consts::PI)
        );
        assert_eq!(
            evaluate("logn", &[number(1.0)], &[], false).expect("ln(1) should normalize to zero"),
            number(0.0)
        );
        assert_eq!(
            evaluate("sin", &[number(0.0)], &[], false).expect("sin(0) should normalize to zero"),
            number(0.0)
        );
        assert_eq!(
            evaluate("cos", &[number(0.0)], &[], false).expect("cos(0) should normalize to one"),
            number(1.0)
        );
        assert_eq!(
            evaluate("tan", &[number(0.0)], &[], false).expect("tan(0) should normalize to zero"),
            number(0.0)
        );
        assert_eq!(
            evaluate("cos", &[number(std::f64::consts::PI)], &[], false)
                .expect("cos(pi) should use the Float-adapted pi"),
            number(-1.0)
        );
        assert_eq!(
            evaluate(
                "cos",
                &[],
                &[named_arg("x", number(std::f64::consts::PI))],
                false,
            )
            .expect("named x should bind for cosine"),
            number(-1.0)
        );
        assert_eq!(
            evaluate(
                "logn",
                &[evaluate("sum", &[number(1.0), number(1.0)], &[], false)
                    .expect("nested arithmetic should produce a number")],
                &[],
                false,
            )
            .expect("nested numeric results should remain typed"),
            number(f64::from(libm::log(2.0_f64) as f32))
        );
        assert_eq!(
            evaluate("pi", &[], &[], false)
                .and_then(|value| evaluate("multiply", &[value, number(2.0)], &[], false))
                .and_then(|value| evaluate("cos", &[value], &[], false))
                .expect("pi::multiply {2}::cos should chain as typed values"),
            number(1.0)
        );
        assert_eq!(
            evaluate("sin", &[IrValue::String("1".into())], &[], false)
                .expect("numeric text should use numeric_argument"),
            number(f64::from(libm::sin(1.0_f64) as f32))
        );

        assert!(matches!(
            evaluate("logn", &[number(0.0)], &[], false),
            Ok(IrValue::Number(value)) if value == f64::from(i32::MIN)
        ));
        assert!(matches!(
            evaluate("logn", &[number(-1.0)], &[], false),
            Ok(IrValue::Number(value)) if value.is_nan()
        ));
        assert!(matches!(
            evaluate("sin", &[number(f64::INFINITY)], &[], false),
            Ok(IrValue::Number(value)) if value.is_nan()
        ));
        assert!(matches!(
            evaluate("cos", &[number(f64::NEG_INFINITY)], &[], false),
            Ok(IrValue::Number(value)) if value.is_nan()
        ));
        assert!(matches!(
            evaluate("tan", &[number(f64::NAN)], &[], false),
            Ok(IrValue::Number(value)) if value.is_nan()
        ));
    }

    #[test]
    fn deterministic_transcendental_math_has_stable_representative_bits() {
        for (name, input, expected_bits) in [
            ("logn", 2.0_f32, 0x3f31_7218),
            ("logn", f32::from_bits(0x402d_f854), 0x3f7f_ffff),
            ("sin", 1.0_f32, 0x3f57_6aa4),
            ("sin", f32::from_bits(0x4049_0fdb), 0xb3bb_bd2e),
            ("cos", 1.0_f32, 0x3f0a_5140),
            ("cos", f32::from_bits(0x4049_0fdb), 0xbf80_0000),
            ("tan", 1.0_f32, 0x3fc7_5923),
            ("tan", f32::from_bits(0x4049_0fdb), 0x33bb_bd2e),
        ] {
            assert_eq!(
                deterministic_transcendental(name, input).to_bits(),
                expected_bits,
                "{name}({input:?}) changed"
            );
        }

        assert_eq!(
            deterministic_transcendental("sin", -0.0).to_bits(),
            (-0.0_f32).to_bits()
        );
        assert_eq!(
            deterministic_transcendental("tan", -0.0).to_bits(),
            (-0.0_f32).to_bits()
        );
        assert_eq!(
            deterministic_transcendental("cos", -0.0).to_bits(),
            1.0_f32.to_bits()
        );
        assert_eq!(
            deterministic_transcendental("logn", 0.0).to_bits(),
            f32::NEG_INFINITY.to_bits()
        );
        assert!(deterministic_transcendental("logn", -1.0).is_nan());
        assert!(deterministic_transcendental("sin", f32::INFINITY).is_nan());
    }

    #[test]
    fn numeric_arithmetic_matches_upstream_float_and_integer_boundaries() {
        assert_eq!(
            evaluate("subtract", &[number(-2.0), number(5.0)], &[], false)
                .expect("negative subtraction should evaluate"),
            number(-7.0)
        );
        assert_eq!(
            evaluate("divide", &[number(-7.0), number(2.0)], &[], false)
                .expect("negative division should evaluate"),
            number(f64::from(-3.5_f32))
        );
        assert!(matches!(
            evaluate("divide", &[number(1.0), number(0.0)], &[], false),
            Ok(IrValue::Number(value)) if value == f64::from(i32::MAX)
        ));
        assert!(matches!(
            evaluate("divide", &[number(0.0), number(0.0)], &[], false),
            Ok(IrValue::Number(value)) if value.is_nan()
        ));
        assert_eq!(
            evaluate("rem", &[number(-5.0), number(2.0)], &[], false)
                .expect("negative left remainder should evaluate"),
            number(-1.0)
        );
        assert_eq!(
            evaluate("rem", &[number(5.0), number(-2.0)], &[], false)
                .expect("negative right remainder should evaluate"),
            number(1.0)
        );
        assert!(matches!(
            evaluate("rem", &[number(1.5), number(0.0)], &[], false),
            Ok(IrValue::Number(value)) if value.is_nan()
        ));
        assert_eq!(
            evaluate(
                "pow",
                &[number(2.0)],
                &[named_arg("to", number(-2.0))],
                false
            )
            .expect("negative exponent should evaluate"),
            number(f64::from((2.0_f32).powi(-2)))
        );
        assert_eq!(
            evaluate(
                "pow",
                &[number(-2.0)],
                &[named_arg("to", number(0.5))],
                false
            )
            .expect("fractional exponent should truncate before pow"),
            number(1.0)
        );
        assert_eq!(
            evaluate(
                "pow",
                &[number(0.0)],
                &[named_arg("to", number(0.0))],
                false
            )
            .expect("zero exponent should use Kotlin Float.pow semantics"),
            number(1.0)
        );
        assert_eq!(
            evaluate("abs", &[number(-0.0)], &[], false).expect("absolute zero should evaluate"),
            number(0.0)
        );
        assert!(matches!(
            evaluate("negate", &[number(0.0)], &[], false),
            Ok(IrValue::Number(value)) if value == 0.0 && value.is_sign_positive()
        ));
        assert!(matches!(
            evaluate("sqrt", &[number(-1.0)], &[], false),
            Ok(IrValue::Number(value)) if value.is_nan()
        ));
        assert_eq!(
            evaluate("iseven", &[number(-3.9)], &[], false)
                .expect("non-integral values should truncate toward zero"),
            IrValue::Boolean(false)
        );
        assert_eq!(
            evaluate("iseven", &[number(-4.9)], &[], false)
                .expect("negative non-integral values should truncate toward zero"),
            IrValue::Boolean(true)
        );
        assert_eq!(
            evaluate("iseven", &[number(f64::NAN)], &[], false)
                .expect("NaN should follow Kotlin Float.toInt"),
            IrValue::Boolean(true)
        );
    }

    #[test]
    fn numeric_builtins_share_argument_binding_and_fail_closed() {
        let binary = ["sum", "subtract", "multiply", "divide", "rem", "pow"];
        let unary = [
            "abs", "negate", "sqrt", "iseven", "logn", "sin", "cos", "tan",
        ];

        for name in binary {
            assert!(
                evaluate(name, &[number(1.0)], &[], false).is_err(),
                "{name} missing"
            );
            assert!(
                evaluate(name, &[number(1.0), number(2.0), number(3.0)], &[], false).is_err(),
                "{name} too many"
            );
            assert!(
                evaluate(
                    name,
                    &[number(1.0), number(2.0)],
                    &[named_arg("unknown", number(3.0))],
                    false
                )
                .is_err(),
                "{name} unknown named"
            );
            assert!(
                evaluate(
                    name,
                    &[number(1.0), number(2.0)],
                    &[named_arg("a", number(3.0))],
                    false
                )
                .is_err(),
                "{name} duplicate binding"
            );
            assert!(
                evaluate(name, &[IrValue::Boolean(true), number(2.0)], &[], false).is_err(),
                "{name} invalid conversion"
            );
            assert!(
                evaluate(name, &[number(1.0), number(2.0)], &[], true).is_err(),
                "{name} block body"
            );
        }

        for name in unary {
            assert!(evaluate(name, &[], &[], false).is_err(), "{name} missing");
            assert!(
                evaluate(name, &[number(1.0), number(2.0)], &[], false).is_err(),
                "{name} too many"
            );
            assert!(
                evaluate(
                    name,
                    &[number(1.0)],
                    &[named_arg("unknown", number(2.0))],
                    false
                )
                .is_err(),
                "{name} unknown named"
            );
            assert!(
                evaluate(name, &[number(1.0)], &[named_arg("x", number(2.0))], false).is_err(),
                "{name} duplicate binding"
            );
            assert!(
                evaluate(name, &[IrValue::Boolean(true)], &[], false).is_err(),
                "{name} invalid conversion"
            );
            assert!(
                evaluate(name, &[number(1.0)], &[], true).is_err(),
                "{name} block body"
            );
        }

        assert!(evaluate("pi", &[number(1.0)], &[], false).is_err());
        assert!(evaluate("pi", &[], &[named_arg("x", number(1.0))], false).is_err());
        assert!(evaluate("pi", &[], &[], true).is_err());
    }

    #[test]
    fn string_surface_is_registered_and_returns_typed_values() {
        for name in [
            "string",
            "concatenate",
            "uppercase",
            "lowercase",
            "capitalize",
            "isempty",
            "isnotempty",
            "startswith",
            "plaintext",
        ] {
            assert!(is_supported(name), "{name} should be supported");
        }

        assert_eq!(
            evaluate("string", &[IrValue::String("  Hello  ".into())], &[], false,)
                .expect("quoted scalar should preserve inner whitespace"),
            IrValue::String("  Hello  ".into())
        );
        assert_eq!(
            evaluate("string", &[number(3.5)], &[], false).expect("number adapts to text"),
            IrValue::String("3.5".into())
        );
        assert_eq!(
            evaluate(
                "string",
                &[],
                &[named_arg("value", IrValue::Boolean(true))],
                false,
            )
            .expect("named string argument should bind"),
            IrValue::String("true".into())
        );
    }

    #[test]
    fn plaintext_projects_evaluated_inline_structure() {
        let span = |start, end| SourceSpan::new(SourceId(0), start, end);
        let content = IrValue::Content(vec![IrNode::Paragraph {
            content: vec![
                IrInline::Text {
                    content: "one ".into(),
                    span: span(0, 4),
                },
                IrInline::Strong {
                    content: vec![IrInline::Emphasis {
                        content: vec![IrInline::Strikethrough {
                            content: vec![IrInline::Text {
                                content: "two".into(),
                                span: span(4, 7),
                            }],
                            span: span(4, 7),
                        }],
                        span: span(4, 7),
                    }],
                    span: span(4, 7),
                },
                IrInline::Text {
                    content: " ".into(),
                    span: span(7, 8),
                },
                IrInline::Code {
                    content: "cargo test".into(),
                    span: span(8, 20),
                },
                IrInline::Text {
                    content: " ".into(),
                    span: span(20, 21),
                },
                IrInline::Link {
                    content: vec![IrInline::Text {
                        content: "label".into(),
                        span: span(21, 26),
                    }],
                    destination: "https://example.com".into(),
                    title: None,
                    span: span(21, 50),
                },
                IrInline::SoftBreak { span: span(50, 51) },
                IrInline::Text {
                    content: "next".into(),
                    span: span(51, 55),
                },
                IrInline::HardBreak { span: span(55, 57) },
                IrInline::Image {
                    content: vec![IrInline::Text {
                        content: "alt must be skipped".into(),
                        span: span(57, 76),
                    }],
                    destination: "image.png".into(),
                    title: None,
                    span: span(57, 76),
                },
            ],
            span: span(0, 76),
        }]);

        assert_eq!(
            evaluate("plaintext", &[content], &[], false).expect("content projects"),
            IrValue::String("one two cargo test label\nnext".into())
        );
        assert_eq!(
            evaluate("plaintext", &[IrValue::Content(Vec::new())], &[], false,)
                .expect("empty content projects to an empty string"),
            IrValue::String(String::new())
        );
        assert_eq!(
            evaluate(
                "plaintext",
                &[IrValue::Identifier("hello".into())],
                &[],
                false
            )
            .expect("identifier scalar is supported"),
            IrValue::String("hello".into())
        );
        assert_eq!(
            evaluate("plaintext", &[number(123.0)], &[], false)
                .expect("number scalar is supported"),
            IrValue::String("123".into())
        );
        assert_eq!(
            evaluate("plaintext", &[IrValue::Boolean(true)], &[], false)
                .expect("boolean scalar is supported"),
            IrValue::String("true".into())
        );
    }

    #[test]
    fn plaintext_rejects_reparse_and_unsupported_values() {
        let span = SourceSpan::new(SourceId(0), 0, 1);
        let values = [
            IrValue::String("**not parsed**".into()),
            IrValue::None,
            IrValue::Collection(Vec::new()),
            IrValue::Range(IrRange {
                start: Some(1),
                end: Some(2),
                span,
            }),
            IrValue::Pair(IrPair {
                first: Box::new(IrValue::Identifier("a".into())),
                second: Box::new(IrValue::Identifier("b".into())),
                span,
            }),
            IrValue::Dictionary(IrDictionary {
                entries: Vec::new(),
                span,
            }),
            IrValue::Callable(IrCallable {
                parameters: None,
                body: Vec::new(),
                span,
                capture: None,
            }),
        ];

        for value in values {
            assert!(
                evaluate("plaintext", &[value], &[], false).is_err(),
                "unsupported value must fail closed"
            );
        }

        let unresolved = IrValue::Content(vec![IrNode::Paragraph {
            content: vec![IrInline::DirectiveCall {
                name: "unknown".into(),
                positional_args: Vec::new(),
                named_args: Vec::new(),
                body: None,
                span,
            }],
            span,
        }]);
        assert!(evaluate("plaintext", &[unresolved], &[], false).is_err());
        assert!(evaluate("plaintext", &[], &[], true).is_err());
    }

    #[test]
    fn plaintext_reuses_single_content_argument_binding() {
        let content = IrValue::Identifier("hello".into());
        assert!(evaluate("plaintext", &[], &[], false).is_err());
        assert!(evaluate("plaintext", &[content.clone(), content.clone()], &[], false).is_err());
        assert!(evaluate(
            "plaintext",
            std::slice::from_ref(&content),
            &[named_arg("unknown", content.clone())],
            false
        )
        .is_err());
        assert!(evaluate(
            "plaintext",
            std::slice::from_ref(&content),
            &[named_arg("content", content.clone())],
            false
        )
        .is_err());
        assert_eq!(
            evaluate(
                "plaintext",
                &[],
                &[named_arg("content", IrValue::Identifier("hello".into()))],
                false
            )
            .expect("named content should bind"),
            IrValue::String("hello".into())
        );
    }

    #[test]
    fn string_operations_bind_named_arguments_and_defaults() {
        assert_eq!(
            evaluate(
                "concatenate",
                &[IrValue::Identifier("abc".into())],
                &[named_arg("with", IrValue::Identifier("def".into()))],
                false,
            )
            .expect("named concatenate argument should bind"),
            IrValue::String("abcdef".into())
        );
        assert_eq!(
            evaluate(
                "concatenate",
                &[
                    IrValue::Identifier("abc".into()),
                    IrValue::Identifier("def".into()),
                    IrValue::Identifier("no".into()),
                ],
                &[],
                false,
            )
            .expect("boolean identifier should adapt at the invocation boundary"),
            IrValue::String("abc".into())
        );
        assert_eq!(
            evaluate(
                "startswith",
                &[
                    IrValue::String("Hello".into()),
                    IrValue::String("He".into())
                ],
                &[],
                false,
            )
            .expect("startswith should use a false default"),
            IrValue::Boolean(true)
        );
        assert_eq!(
            evaluate(
                "startswith",
                &[
                    IrValue::String("Hello".into()),
                    IrValue::String("he".into()),
                ],
                &[named_arg("ignorecase", IrValue::Identifier("yes".into()))],
                false,
            )
            .expect("named ignorecase should bind"),
            IrValue::Boolean(true)
        );
    }

    #[test]
    fn string_case_and_empty_operations_cover_unicode_and_boundaries() {
        for name in ["uppercase", "lowercase"] {
            assert!(matches!(
                evaluate(
                    name,
                    &[],
                    &[named_arg("string", IrValue::Identifier("Hello".into()))],
                    false,
                ),
                Ok(IrValue::String(_))
            ));
        }
        assert_eq!(
            evaluate(
                "capitalize",
                &[IrValue::Identifier("hello, world!".into())],
                &[],
                false,
            )
            .expect("capitalize should adapt a scalar identifier"),
            IrValue::String("Hello, world!".into())
        );
        assert_eq!(
            evaluate("capitalize", &[IrValue::String(String::new())], &[], false)
                .expect("empty capitalization should succeed"),
            IrValue::String(String::new())
        );
        assert_eq!(
            evaluate("capitalize", &[IrValue::String("É".into())], &[], false)
                .expect("one-character capitalization should succeed"),
            IrValue::String("É".into())
        );
        assert_eq!(
            evaluate(
                "capitalize",
                &[IrValue::String("éclair".into())],
                &[],
                false
            )
            .expect("Unicode capitalization should succeed"),
            IrValue::String("Éclair".into())
        );
        assert_eq!(
            evaluate("isempty", &[IrValue::String(String::new())], &[], false)
                .expect("empty string should be accepted"),
            IrValue::Boolean(true)
        );
        assert_eq!(
            evaluate("isnotempty", &[IrValue::String(" ".into())], &[], false)
                .expect("whitespace is not empty"),
            IrValue::Boolean(true)
        );
        assert_eq!(
            evaluate("isempty", &[IrValue::String(" ".into())], &[], false)
                .expect("whitespace should not be trimmed"),
            IrValue::Boolean(false)
        );
        for value in ["", " ", "value", "값"] {
            let empty = evaluate("isempty", &[IrValue::String(value.to_string())], &[], false)
                .expect("isempty should evaluate");
            let not_empty = evaluate(
                "isnotempty",
                &[IrValue::String(value.to_string())],
                &[],
                false,
            )
            .expect("isnotempty should evaluate");
            assert_eq!(
                not_empty,
                IrValue::Boolean(!matches!(empty, IrValue::Boolean(true))),
                "predicate complement for {value:?}"
            );
        }
    }

    #[test]
    fn string_operations_reject_unsupported_values_and_invalid_bindings() {
        for name in [
            "string",
            "uppercase",
            "lowercase",
            "capitalize",
            "isempty",
            "isnotempty",
            "startswith",
        ] {
            let arguments = if name == "startswith" {
                vec![IrValue::String("value".into()), IrValue::None]
            } else {
                vec![IrValue::None]
            };
            assert!(evaluate(name, &arguments, &[], false).is_err(), "{name}");
        }
        assert!(evaluate("string", &[IrValue::Collection(Vec::new())], &[], false).is_err());
        let rich_content = IrValue::Content(vec![IrNode::Paragraph {
            content: vec![IrInline::Strong {
                content: vec![IrInline::Text {
                    content: "rich".into(),
                    span: crate::source::SourceSpan::new(crate::source::SourceId(0), 0, 4),
                }],
                span: crate::source::SourceSpan::new(crate::source::SourceId(0), 0, 4),
            }],
            span: crate::source::SourceSpan::new(crate::source::SourceId(0), 0, 4),
        }]);
        assert!(evaluate("string", &[rich_content], &[], false).is_err());
        assert!(evaluate(
            "concatenate",
            &[
                IrValue::Identifier("a".into()),
                IrValue::Identifier("b".into())
            ],
            &[named_arg("if", IrValue::Identifier("maybe".into()))],
            false,
        )
        .is_err());
        assert!(evaluate(
            "startswith",
            &[
                IrValue::String("Hello".into()),
                IrValue::String("he".into())
            ],
            &[named_arg("ignorecase", IrValue::Identifier("maybe".into()))],
            false,
        )
        .is_err());
        assert!(evaluate("capitalize", &[], &[], false).is_err());
        assert!(evaluate(
            "capitalize",
            &[IrValue::Identifier("a".into())],
            &[named_arg("string", IrValue::Identifier("b".into()))],
            false,
        )
        .is_err());
        assert!(evaluate("isnotempty", &[IrValue::Identifier("a".into())], &[], true,).is_err());
        assert!(evaluate(
            "concatenate",
            &[
                IrValue::Identifier("a".into()),
                IrValue::Identifier("b".into())
            ],
            &[named_arg("unknown", IrValue::Identifier("c".into()))],
            false,
        )
        .is_err());
        assert!(evaluate(
            "concatenate",
            &[
                IrValue::Identifier("a".into()),
                IrValue::Identifier("b".into())
            ],
            &[named_arg("a", IrValue::Identifier("c".into()))],
            false,
        )
        .is_err());
        assert!(evaluate(
            "startswith",
            &[IrValue::String("a".into()), IrValue::String("b".into())],
            &[named_arg("ignorecase", IrValue::Boolean(false))],
            true,
        )
        .is_err());
    }

    #[test]
    fn logical_surface_is_registered_and_evaluates_typed_results() {
        for name in ["islower", "isgreater", "equals", "not"] {
            assert!(is_supported(name));
        }

        assert_eq!(
            evaluate(
                "islower",
                &[number(2.0)],
                &[crate::ir::IrNamedArg {
                    name: "than".into(),
                    name_span: crate::source::SourceSpan::new(crate::source::SourceId(0), 0, 0),
                    value: number(3.0),
                    span: crate::source::SourceSpan::new(crate::source::SourceId(0), 0, 0),
                }],
                false,
            )
            .expect("comparison should evaluate"),
            IrValue::Boolean(true)
        );
        assert_eq!(
            evaluate(
                "isgreater",
                &[number(3.0), number(3.0), IrValue::Boolean(true)],
                &[],
                false
            )
            .expect("inclusive comparison should evaluate"),
            IrValue::Boolean(true)
        );
        assert_eq!(
            evaluate("not", &[IrValue::Identifier("yes".into())], &[], false)
                .expect("boolean negation should evaluate"),
            IrValue::Boolean(false)
        );
    }

    #[test]
    fn equality_preserves_types_and_uses_upstream_plain_text_fallback() {
        assert_eq!(
            evaluate(
                "equals",
                &[number(2.0), IrValue::String("2".into())],
                &[],
                false
            )
            .expect("numeric text equality should evaluate"),
            IrValue::Boolean(true)
        );
        assert_eq!(
            evaluate(
                "equals",
                &[
                    IrValue::Content(vec![IrNode::Paragraph {
                        content: vec![IrInline::Strong {
                            content: vec![IrInline::Text {
                                content: "same".into(),
                                span: crate::source::SourceSpan::new(
                                    crate::source::SourceId(0),
                                    0,
                                    4,
                                ),
                            }],
                            span: crate::source::SourceSpan::new(crate::source::SourceId(0), 0, 4,),
                        }],
                        span: crate::source::SourceSpan::new(crate::source::SourceId(0), 0, 4),
                    }]),
                    IrValue::String("same".into()),
                ],
                &[],
                false,
            )
            .expect("rich plain-text equality should evaluate"),
            IrValue::Boolean(true)
        );
        assert_eq!(
            evaluate(
                "equals",
                &[IrValue::Boolean(true), IrValue::String("true".into())],
                &[],
                false,
            )
            .expect("typed equality should evaluate"),
            IrValue::Boolean(false)
        );
    }

    #[test]
    fn logical_builtins_reject_invalid_values_and_duplicate_bindings() {
        assert!(evaluate("not", &[number(1.0)], &[], false).is_err());
        assert!(evaluate(
            "islower",
            &[number(1.0), IrValue::Boolean(true)],
            &[],
            false
        )
        .is_err());
        assert!(evaluate("equals", &[number(1.0)], &[], false).is_err());
        assert!(evaluate("equals", &[number(1.0), number(1.0)], &[], true).is_err());
    }
}
