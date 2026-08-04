//! Quarkdown-compatible `@`-prefixed directive parser.
//!
//! This is a clean-room implementation based on publicly available
//! Quarkdown syntax documentation. See `docs/compatibility/quarkdown/`
//! for provenance records.
//!
//! ## Supported forms
//!
//! - `@name`
//! - `@name(arg1, arg2, named: value)`
//! - `@name[body content]`
//! - `@name(named: value)[body]`
//!
//! ## Values
//!
//! - String: `"hello"` (double-quoted)
//! - Number: `42`, `3.14`
//! - Boolean: `true`, `false`
//! - Identifier: `name` (unquoted bare word)

use crate::source::{ByteSpan, SourceId};
use crate::syntax::markdown::ast::Value;
use crate::syntax::quarkdown::Directive;

/// Characters that are valid in a directive/identifier name.
fn is_name_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-'
}

/// Try to parse a directive from a byte slice starting at position `start`.
///
/// Returns `Some((directive, end_offset))` on success, `None` if no directive
/// starts at the given position. `end_offset` is the byte offset just past the
/// parsed directive in the source.
pub fn parse_directive_at(
    source: &str,
    start: usize,
    source_id: SourceId,
) -> Option<(Directive, usize)> {
    let bytes = source.as_bytes();
    if start >= bytes.len() || bytes[start] != b'@' {
        return None;
    }

    let after_at = start + 1;
    let rest = &source[after_at..];

    // Parse identifier (name)
    let name_end = rest
        .char_indices()
        .find(|&(_, c)| !is_name_char(c))
        .map(|(i, _)| after_at + i)
        .unwrap_or(source.len());

    let name = &source[after_at..name_end];
    if name.is_empty() {
        // Just '@' with no name — not a directive
        return None;
    }

    let mut cursor = name_end;

    // Parse argument list: `(arg1, arg2, named: val)`
    let mut positional_args = Vec::new();
    let mut named_args = Vec::new();
    let mut has_parens = false;

    if cursor < bytes.len() && bytes[cursor] == b'(' {
        has_parens = true;
        cursor += 1; // skip '('
        cursor = skip_whitespace(source, cursor);

        if cursor < bytes.len() && bytes[cursor] != b')' {
            // Parse first argument
            let (args_consumed, named) = parse_arg_list_body(source, cursor, source_id)?;
            positional_args = args_consumed.0;
            named_args = args_consumed.1;
            cursor += named;
        }

        // Expect ')'
        if cursor >= bytes.len() || bytes[cursor] != b')' {
            return None;
        }
        cursor += 1; // skip ')'
    }

    // Parse body: `[...]`
    let body = if cursor < bytes.len() && bytes[cursor] == b'[' {
        cursor += 1; // skip '['
        let body_start = cursor;
        let mut depth = 1u32;
        let mut body_end = body_start;

        while cursor < bytes.len() {
            match bytes[cursor] {
                b'[' => {
                    depth += 1;
                    cursor += 1;
                }
                b']' => {
                    depth -= 1;
                    if depth == 0 {
                        body_end = cursor;
                        cursor += 1; // skip ']'
                        break;
                    }
                    cursor += 1;
                }
                _ => {
                    cursor += 1;
                }
            }
        }

        if depth != 0 {
            return None; // unclosed bracket
        }

        let body_source = &source[body_start..body_end];
        let body_directive = parse_body_content(body_source, source_id);
        Some(body_directive)
    } else {
        None
    };

    let span_start = start;
    let span_end = cursor;

    let directive = if has_parens || body.is_some() {
        Directive::Call {
            name: name.to_string(),
            positional_args,
            named_args,
            body: body.map(Box::new),
            span: ByteSpan::new(span_start, span_end),
        }
    } else {
        Directive::Variable {
            name: name.to_string(),
            span: ByteSpan::new(span_start, span_end),
        }
    };

    Some((directive, cursor))
}

/// Parse the content inside `(...)` — positionals and named args.
///
/// Returns `( (positionals, named), bytes_consumed )`.
type ArgsResult = (Vec<Value>, Vec<(String, Value)>);

fn parse_arg_list_body(
    source: &str,
    start: usize,
    source_id: SourceId,
) -> Option<(ArgsResult, usize)> {
    let bytes = source.as_bytes();
    let mut cursor = start;
    let mut positional = Vec::new();
    let mut named = Vec::new();

    loop {
        cursor = skip_whitespace(source, cursor);

        if cursor >= bytes.len() || bytes[cursor] == b')' {
            break;
        }

        // Try to parse a value
        let (val, after_val) = parse_value(source, cursor, source_id)?;
        let after_val = skip_whitespace(source, after_val);

        // Check if this is `name: value`
        if let Value::Identifier(ref id) = val {
            if after_val < bytes.len() && bytes[after_val] == b':' {
                cursor = skip_whitespace(source, after_val + 1);
                let (named_val, after_named) = parse_value(source, cursor, source_id)?;
                named.push((id.clone(), named_val));
                cursor = after_named;
            } else {
                positional.push(Value::Identifier(id.clone()));
                cursor = after_val;
            }
        } else {
            positional.push(val);
            cursor = after_val;
        }

        cursor = skip_whitespace(source, cursor);

        if cursor < bytes.len() && bytes[cursor] == b',' {
            cursor += 1;
            cursor = skip_whitespace(source, cursor);
        } else {
            break;
        }
    }

    Some(((positional, named), cursor - start))
}

/// Parse a single value from the given position.
fn parse_value(source: &str, start: usize, _source_id: SourceId) -> Option<(Value, usize)> {
    let bytes = source.as_bytes();
    let cursor = skip_whitespace(source, start);

    if cursor >= bytes.len() {
        return None;
    }

    match bytes[cursor] {
        b'"' => {
            // Double-quoted string
            let mut end = cursor + 1;
            while end < bytes.len() && bytes[end] != b'"' {
                if bytes[end] == b'\\' {
                    end += 1; // skip escaped char
                }
                end += 1;
            }
            if end >= bytes.len() {
                return None; // unclosed string
            }
            let raw = &source[cursor + 1..end];
            let unescaped = raw.replace("\\\"", "\"").replace("\\\\", "\\");
            Some((Value::String(unescaped), end + 1))
        }
        b't' if source[cursor..].starts_with("true") => Some((Value::Boolean(true), cursor + 4)),
        b'f' if source[cursor..].starts_with("false") => Some((Value::Boolean(false), cursor + 5)),
        _ => {
            // Number or identifier
            let end = cursor
                + source[cursor..]
                    .char_indices()
                    .find(|&(_, c)| !is_value_char(c))
                    .map(|(i, _)| i)
                    .unwrap_or(source.len() - cursor);

            let token = &source[cursor..end];
            if token.is_empty() {
                return None;
            }

            // Try number first
            if let Ok(n) = token.parse::<f64>() {
                Some((Value::Number(n), end))
            } else {
                Some((Value::Identifier(token.to_string()), end))
            }
        }
    }
}

fn is_value_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-' || c == '.'
}

/// Parse the content inside `[...]` body brackets.
///
/// The body can contain nested directives, plain text, or a mix.
/// For M1, we parse it as a flat sequence: plain text + nested `@` calls.
fn parse_body_content(source: &str, source_id: SourceId) -> Directive {
    let trimmed = source.trim();
    if trimmed.is_empty() {
        return Directive::Value(Value::String(String::new()));
    }

    // Try to parse the entire body as a single nested directive
    if let Some((directive, consumed)) = parse_directive_at(source, 0, source_id) {
        if consumed == source.len() {
            return directive;
        }
    }

    // Otherwise, treat the body as literal string content.
    // Body text will be further processed by the evaluator.
    Directive::Value(Value::String(trimmed.to_string()))
}

/// Parse a directive from a `@`-prefixed source string.
///
/// Returns `None` if the source does not start with a valid directive.
pub fn parse_directive(source: &str, source_id: SourceId) -> Option<Directive> {
    parse_directive_at(source, 0, source_id).map(|(d, _)| d)
}

fn skip_whitespace(source: &str, start: usize) -> usize {
    source[start..]
        .char_indices()
        .find(|&(_, c)| !c.is_ascii_whitespace())
        .map(|(i, _)| start + i)
        .unwrap_or(source.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::SourceId;

    fn sid() -> SourceId {
        SourceId(1)
    }

    #[test]
    fn parse_empty_returns_none() {
        assert!(parse_directive("", sid()).is_none());
    }

    #[test]
    fn parse_plain_text_returns_none() {
        assert!(parse_directive("hello world", sid()).is_none());
    }

    #[test]
    fn parse_just_at_sign() {
        assert!(parse_directive("@", sid()).is_none());
    }

    #[test]
    fn parse_simple_variable() {
        let d = parse_directive("@name", sid()).unwrap();
        assert_eq!(
            d,
            Directive::Variable {
                name: "name".into(),
                span: ByteSpan::new(0, 5),
            }
        );
    }

    #[test]
    fn parse_variable_with_hyphen() {
        let d = parse_directive("@my-var", sid()).unwrap();
        assert_eq!(
            d,
            Directive::Variable {
                name: "my-var".into(),
                span: ByteSpan::new(0, 7),
            }
        );
    }

    #[test]
    fn parse_call_no_args_no_body() {
        let d = parse_directive("@fn()", sid()).unwrap();
        match d {
            Directive::Call {
                name,
                positional_args,
                named_args,
                body,
                ..
            } => {
                assert_eq!(name, "fn");
                assert!(positional_args.is_empty());
                assert!(named_args.is_empty());
                assert!(body.is_none());
            }
            _ => panic!("expected Call"),
        }
    }

    #[test]
    fn parse_call_positional_args() {
        let d = parse_directive("@fn(42, \"hello\")", sid()).unwrap();
        match d {
            Directive::Call {
                positional_args,
                named_args,
                body,
                ..
            } => {
                assert_eq!(positional_args.len(), 2);
                assert_eq!(positional_args[0], Value::Number(42.0));
                assert_eq!(positional_args[1], Value::String("hello".into()));
                assert!(named_args.is_empty());
                assert!(body.is_none());
            }
            _ => panic!("expected Call"),
        }
    }

    #[test]
    fn parse_call_named_args() {
        let d = parse_directive("@fn(level: 1, title: \"Hi\")", sid()).unwrap();
        match d {
            Directive::Call { named_args, .. } => {
                assert_eq!(named_args.len(), 2);
                assert_eq!(named_args[0], ("level".into(), Value::Number(1.0)));
                assert_eq!(named_args[1], ("title".into(), Value::String("Hi".into())));
            }
            _ => panic!("expected Call"),
        }
    }

    #[test]
    fn parse_call_with_body() {
        let d = parse_directive("@fn[body text]", sid()).unwrap();
        match d {
            Directive::Call {
                positional_args,
                named_args,
                body,
                ..
            } => {
                assert!(positional_args.is_empty());
                assert!(named_args.is_empty());
                let body = *body.unwrap();
                assert_eq!(body, Directive::Value(Value::String("body text".into())));
            }
            _ => panic!("expected Call"),
        }
    }

    #[test]
    fn parse_call_args_and_body() {
        let d = parse_directive("@heading(level: 1)[Title]", sid()).unwrap();
        match d {
            Directive::Call {
                name,
                named_args,
                body,
                ..
            } => {
                assert_eq!(name, "heading");
                assert_eq!(named_args.len(), 1);
                assert_eq!(named_args[0], ("level".into(), Value::Number(1.0)));
                let body = *body.unwrap();
                assert_eq!(body, Directive::Value(Value::String("Title".into())));
            }
            _ => panic!("expected Call"),
        }
    }

    #[test]
    fn parse_nested_directive_in_body() {
        let d = parse_directive("@fn[@inner(42)]", sid()).unwrap();
        match d {
            Directive::Call { body, .. } => {
                let body = *body.unwrap();
                match body {
                    Directive::Call {
                        name,
                        positional_args,
                        ..
                    } => {
                        assert_eq!(name, "inner");
                        assert_eq!(positional_args.len(), 1);
                        assert_eq!(positional_args[0], Value::Number(42.0));
                    }
                    _ => panic!("expected nested Call"),
                }
            }
            _ => panic!("expected Call"),
        }
    }

    #[test]
    fn parse_call_string_with_escaped_quotes() {
        let d = parse_directive("@fn(\"hello \\\"world\\\"\")", sid()).unwrap();
        match d {
            Directive::Call {
                positional_args, ..
            } => {
                assert_eq!(positional_args.len(), 1);
                assert_eq!(positional_args[0], Value::String("hello \"world\"".into()));
            }
            _ => panic!("expected Call"),
        }
    }

    #[test]
    fn parse_call_boolean_args() {
        let d = parse_directive("@fn(true, false)", sid()).unwrap();
        match d {
            Directive::Call {
                positional_args, ..
            } => {
                assert_eq!(positional_args.len(), 2);
                assert_eq!(positional_args[0], Value::Boolean(true));
                assert_eq!(positional_args[1], Value::Boolean(false));
            }
            _ => panic!("expected Call"),
        }
    }

    #[test]
    fn parse_call_mixed_args() {
        let d = parse_directive("@fn(42, name: \"val\")", sid()).unwrap();
        match d {
            Directive::Call {
                positional_args,
                named_args,
                ..
            } => {
                assert_eq!(positional_args.len(), 1);
                assert_eq!(positional_args[0], Value::Number(42.0));
                assert_eq!(named_args.len(), 1);
                assert_eq!(named_args[0], ("name".into(), Value::String("val".into())));
            }
            _ => panic!("expected Call"),
        }
    }

    #[test]
    fn parse_call_after_text() {
        // Should fail — directive must start with @
        assert!(parse_directive("text@fn()", sid()).is_none());
    }

    #[test]
    fn parse_call_with_whitespace() {
        let d = parse_directive("@fn( 42 , name : \"val\" )", sid()).unwrap();
        match d {
            Directive::Call {
                positional_args,
                named_args,
                ..
            } => {
                assert_eq!(positional_args.len(), 1);
                assert_eq!(positional_args[0], Value::Number(42.0));
                assert_eq!(named_args.len(), 1);
                assert_eq!(named_args[0], ("name".into(), Value::String("val".into())));
            }
            _ => panic!("expected Call"),
        }
    }

    #[test]
    fn parse_variable_underscore_name() {
        let d = parse_directive("@my_var", sid()).unwrap();
        assert_eq!(
            d,
            Directive::Variable {
                name: "my_var".into(),
                span: ByteSpan::new(0, 7),
            }
        );
    }

    #[test]
    fn parse_conditional_style_not_supported_as_syntax() {
        // @if is parsed as a regular function call for now;
        // Conditional semantic handling is done by the evaluator.
        let d = parse_directive("@if(cond)[then][else]", sid());
        assert!(d.is_some());
        // Just verifies it parses — conditional syntax evaluation is an M1+ concern
    }
}
