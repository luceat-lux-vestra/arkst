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
            | "iseven"
            | "string"
            | "concatenate"
            | "uppercase"
            | "lowercase"
            | "capitalize"
            | "isempty"
            | "isnotempty"
            | "startswith"
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
        "string" => evaluate_string(positional_args, named_args, has_body),
        "concatenate" => evaluate_concatenate(positional_args, named_args, has_body),
        "uppercase" | "lowercase" | "capitalize" => {
            evaluate_case(name, positional_args, named_args, has_body)
        }
        "isempty" | "isnotempty" => {
            evaluate_empty_check(name, positional_args, named_args, has_body)
        }
        "startswith" => evaluate_startswith(positional_args, named_args, has_body),
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
        IrInline::DirectiveCall { .. } | IrInline::ChainedDirectiveCall { .. } => return None,
        IrInline::Image { .. } => return None,
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
fn adapt_string_argument(value: &IrValue) -> Option<String> {
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
    use super::{evaluate, is_supported};
    use crate::ir::{IrInline, IrNode, IrValue};

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
            "sum", "subtract", "multiply", "divide", "rem", "pow", "abs", "negate", "sqrt",
            "iseven",
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
        let unary = ["abs", "negate", "sqrt", "iseven"];

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
