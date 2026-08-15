//! Small, deterministic evaluator builtins used by the current semantic slice.

use crate::ir::{IrInline, IrNamedArg, IrNode, IrValue};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BuiltinError {
    pub message: String,
}

/// Returns whether this builtin has an evaluator implementation.
pub(crate) fn is_supported(name: &str) -> bool {
    matches!(name, "sum" | "multiply" | "uppercase" | "lowercase")
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
        _ => Err(error(format!("`.{name}` has no builtin implementation"))),
    }
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
