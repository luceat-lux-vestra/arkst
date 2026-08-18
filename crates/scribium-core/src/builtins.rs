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
            | "multiply"
            | "uppercase"
            | "lowercase"
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
        "sum" | "multiply" => evaluate_numeric(name, positional_args, named_args, has_body),
        "uppercase" | "lowercase" => evaluate_case(name, positional_args, named_args, has_body),
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
    let number = match value {
        IrValue::Number(number) => *number,
        IrValue::String(value) | IrValue::Identifier(value) => value
            .parse::<i32>()
            .map(f64::from)
            .ok()
            .or_else(|| value.parse::<f32>().ok().map(f64::from))
            .ok_or_else(|| error(format!("`{parameter}` must be numeric")))?,
        _ => return Err(error(format!("`{parameter}` must be numeric"))),
    };
    Ok(number as f32)
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

    let mut values = Vec::with_capacity(positional_args.len() + named_args.len());
    values.extend(positional_args.iter().cloned());
    for arg in named_args {
        if arg.name != "by" {
            return Err(error(format!(
                "`.{name}` does not support named argument `{}`",
                arg.name
            )));
        }
        values.push(arg.value.clone());
    }
    if values.is_empty() {
        return Err(error(format!(
            "`.{name}` requires at least one numeric argument"
        )));
    }

    let mut numbers = Vec::with_capacity(values.len());
    for value in values {
        match value {
            IrValue::Number(number) => numbers.push(number),
            _ => return Err(error(format!("`.{name}` requires numeric arguments"))),
        }
    }
    let result = if name == "sum" {
        numbers.into_iter().sum()
    } else {
        numbers.into_iter().product()
    };
    Ok(IrValue::Number(result))
}

fn evaluate_case(
    name: &str,
    positional_args: &[IrValue],
    named_args: &[IrNamedArg],
    has_body: bool,
) -> Result<IrValue, BuiltinError> {
    if has_body || !named_args.is_empty() || positional_args.len() != 1 {
        return Err(error(format!(
            "`.{name}` requires exactly one positional text argument"
        )));
    }
    let text = adapt_scalar_to_text(&positional_args[0]).ok_or_else(|| {
        error(format!(
            "`.{name}` requires a scalar value that can adapt to text"
        ))
    })?;
    let transformed = if name == "uppercase" {
        text.to_uppercase()
    } else {
        text.to_lowercase()
    };
    Ok(IrValue::String(transformed))
}

fn error(message: String) -> BuiltinError {
    BuiltinError { message }
}

/// Applies the small invocation-boundary text adaptation contract used by the
/// evidenced case builtins. Plain text content is adapted structurally; rich
/// content is not rendered or round-tripped through a backend.
fn adapt_scalar_to_text(value: &IrValue) -> Option<String> {
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
