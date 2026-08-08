//! Quarkdown-compatible function-call syntax parser.
//!
//! This is a clean-room implementation based on publicly available
//! Quarkdown syntax documentation. See `docs/compatibility/quarkdown/`
//! for provenance records.
//!
//! ## Supported forms
//!
//! - `.note` — a call with no arguments
//! - `.note {hello}` — positional argument
//! - `.range {1} {10}` — multiple positional arguments
//! - `.panel width:{320}` — named argument
//! - `.panel {Intro} width:{320}` — mixed positional/named arguments
//! - `.outer {.inner {value}}` — nested call inside an argument
//!
//! Each argument is wrapped in curly braces `{...}`. An argument may hold a
//! plain scalar value (number, boolean, identifier, or string) or an arbitrary
//! content fragment that may itself contain nested function calls.
//!
//! Indented bodies are NOT part of this parser: they are attached to
//! block-level calls by the Markdown block parser.

use crate::source::ByteSpan;
use crate::syntax::markdown::ast::Value;

/// A parsed Quarkdown function call.
#[derive(Debug, Clone, PartialEq)]
pub struct QuarkdownCall {
    /// Function name (without the leading dot).
    pub name: String,
    /// Positional arguments in source order.
    pub positional_args: Vec<Arg>,
    /// Named arguments in source order.
    pub named_args: Vec<(String, Arg)>,
    /// Span of the entire call (leading dot through the last argument).
    pub span: ByteSpan,
}

/// One parsed argument: the outer braces plus either a scalar value or a
/// raw content span (to be refined by the inline parser).
#[derive(Debug, Clone, PartialEq)]
pub struct Arg {
    pub content: ArgContent,
    /// Span of the argument including surrounding braces.
    pub span: ByteSpan,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArgContent {
    /// A literal scalar value (string, number, boolean, identifier).
    Scalar(Value),
    /// A content fragment that contains inline markup and/or nested calls.
    /// `span` excludes the surrounding braces.
    Content(ByteSpan),
}

/// A structured parser error for malformed function-call syntax.
///
/// The error is recoverable: callers treat it as ordinary text and continue
/// parsing. All malformed-input handling must go through this type — the
/// parser never panics on user input.
#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    /// Stable error code (e.g. `E2001`).
    pub code: &'static str,
    /// Human-readable message describing the problem.
    pub message: String,
    /// Span covering the offending construct.
    pub span: ByteSpan,
}

impl ParseError {
    fn new(code: &'static str, message: impl Into<String>, span: ByteSpan) -> Self {
        Self {
            code,
            message: message.into(),
            span,
        }
    }
}

/// Characters permitted in a function-call name (after the leading dot).
fn is_name_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'-'
}

/// The leading character of a name must be a letter or underscore to avoid
/// confusing `.5` or `.` with function calls.
fn is_name_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_'
}

/// Try to parse a Quarkdown function call starting at byte offset `start`.
///
/// Returns:
/// - `Ok(Some((call, end)))` — a call was parsed; `end` is the byte offset
///   just past the call (past the last brace or name).
/// - `Ok(None)` — the source does not start a function call at `start`.
/// - `Err(error)` — the source *starts* a call but is malformed.
pub fn parse_directive_at(
    source: &str,
    start: usize,
) -> Result<Option<(QuarkdownCall, usize)>, ParseError> {
    let bytes = source.as_bytes();
    if start >= bytes.len() || bytes[start] != b'.' {
        return Ok(None);
    }
    if start + 1 >= bytes.len() || !is_name_start(bytes[start + 1]) {
        return Ok(None);
    }

    let name_start = start + 1;
    let mut name_end = name_start;
    while name_end < bytes.len() && is_name_char(bytes[name_end]) {
        name_end += 1;
    }
    let name = source[name_start..name_end].to_string();

    let mut cursor = skip_ascii_whitespace(source, name_end);
    let mut positional_args: Vec<Arg> = Vec::new();
    let mut named_args: Vec<(String, Arg)> = Vec::new();

    loop {
        if cursor >= bytes.len() {
            break;
        }

        // End of the call: anything that is not `{arg}` or `name:{arg}`.
        if bytes[cursor] == b'{' {
            if !named_args.is_empty() {
                return Err(ParseError::new(
                    "E2001",
                    "positional argument after named argument is not allowed \
                     in a Quarkdown function call",
                    ByteSpan::new(cursor, cursor + 1),
                ));
            }
            let arg = parse_braced(source, cursor)?;
            let arg_end = arg.span.end;
            positional_args.push(arg);
            cursor = arg_end;
        } else {
            // Attempt a named argument `name:{...}` or `name: {...}`.
            let name_base = cursor;
            let mut name_end_at = name_base;
            while name_end_at < bytes.len() && is_name_char(bytes[name_end_at]) {
                name_end_at += 1;
            }
            if name_end_at == name_base {
                break;
            }
            let after_name = skip_ascii_whitespace(source, name_end_at);
            let maybe_brace = after_name;
            if maybe_brace < bytes.len() && bytes[maybe_brace] == b':' {
                let after_colon = skip_ascii_whitespace(source, maybe_brace + 1);
                if after_colon >= bytes.len() || bytes[after_colon] != b'{' {
                    return Err(ParseError::new(
                        "E2002",
                        format!(
                            "named argument `{}` must be followed by a value in braces `{{...}}`",
                            &source[name_base..name_end_at]
                        ),
                        ByteSpan::new(name_base, maybe_brace + 1),
                    ));
                }
                let arg = parse_braced(source, after_colon)?;
                let arg_end = arg.span.end;
                named_args.push((source[name_base..name_end_at].to_string(), arg));
                cursor = arg_end;
            } else {
                break;
            }
        }
        cursor = skip_ascii_whitespace(source, cursor);
    }

    // Names (including their leading dots) must not be followed by trailing
    // whitespace in the reported span.
    let mut trimmed_end = cursor;
    while trimmed_end > name_start && matches!(bytes[trimmed_end - 1], b' ' | b'\t' | b'\r' | b'\n')
    {
        trimmed_end -= 1;
    }

    let end = trimmed_end;
    Ok(Some((
        QuarkdownCall {
            name,
            positional_args,
            named_args,
            span: ByteSpan::new(start, end),
        },
        end,
    )))
}

/// Parse a single `{...}` argument starting at the opening brace.
fn parse_braced(source: &str, open: usize) -> Result<Arg, ParseError> {
    let bytes = source.as_bytes();
    debug_assert_eq!(bytes[open], b'{');
    let mut depth = 1u32;
    let mut cursor = open + 1;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'{' => {
                depth += 1;
                cursor += 1;
            }
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    let close = cursor;
                    let content = ByteSpan::new(open + 1, close);
                    let kind = if let Some(scalar) = parse_scalar(source, content) {
                        ArgContent::Scalar(scalar)
                    } else {
                        ArgContent::Content(content)
                    };
                    return Ok(Arg {
                        content: kind,
                        span: ByteSpan::new(open, close + 1),
                    });
                }
                cursor += 1;
            }
            // Cross-line arguments are deliberately not supported: an
            // unclosed brace at end of line is a recoverable error.
            b'\n' => {
                return Err(ParseError::new(
                    "E2003",
                    "unclosed `{...}` argument in function call",
                    ByteSpan::new(open, cursor + 1),
                ));
            }
            _ => cursor += 1,
        }
    }
    Err(ParseError::new(
        "E2003",
        "unclosed `{...}` argument in function call",
        ByteSpan::new(open, source.len()),
    ))
}

/// Attempt to read the argument content as a scalar literal.
///
/// A scalar has no whitespace, no inline markup and no nested call; it is a
/// single bare token or a double-quoted string. Returns `None` when the
/// content is a general inline fragment that must go through the inline
/// parser.
fn parse_scalar(source: &str, content: ByteSpan) -> Option<Value> {
    let raw = &source[content.start..content.end];
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Some(Value::String(String::new()));
    }

    // Quoted strings are always scalars.
    if trimmed.len() >= 2 && trimmed.starts_with('"') && trimmed.ends_with('"') {
        let inner = &trimmed[1..trimmed.len() - 1];
        let unescaped = inner.replace("\\\"", "\"").replace("\\\\", "\\");
        return Some(Value::String(unescaped));
    }

    // Numbers first: `0.5` and `1e4` are valid scalars even though they
    // contain dots.
    if trimmed.starts_with(|c: char| c.is_ascii_digit() || c == '+' || c == '-')
        && !trimmed.contains(|c: char| c.is_alphabetic() && c != 'e' && c != 'E')
    {
        if let Ok(n) = trimmed.parse::<f64>() {
            return Some(Value::Number(n));
        }
    }

    // A scalar has no inline markup: braces, quotes, emphasis markers or
    // dots make it a content fragment (dots may start nested calls).
    // Underscores are kept: they are part of common identifiers
    // (`show_code`) and only read as emphasis inside multi-word fragments.
    if trimmed.contains(['{', '}', '"', '.', '*']) {
        return None;
    }

    // A multi-word fragment is treated as a plain string.
    if trimmed.contains(char::is_whitespace) {
        return Some(Value::String(trimmed.to_string()));
    }

    // Booleans are named in Quarkdown examples.
    match trimmed {
        "true" => return Some(Value::Boolean(true)),
        "false" => return Some(Value::Boolean(false)),
        _ => {}
    }

    // A bare identifier token: letters, digits, underscore, hyphen,
    // starting with a letter or underscore.
    if trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        && trimmed
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
    {
        return Some(Value::Identifier(trimmed.to_string()));
    }

    None
}

fn skip_ascii_whitespace(source: &str, start: usize) -> usize {
    let bytes = source.as_bytes();
    let mut i = start;
    while i < bytes.len() && matches!(bytes[i], b' ' | b'\t' | b'\r' | b'\n') {
        i += 1;
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(src: &str) -> QuarkdownCall {
        match parse_directive_at(src, 0) {
            Ok(Some((c, end))) => {
                assert_eq!(end, src.len(), "call must consume the source");
                c
            }
            Ok(None) => panic!("no call parsed from {src:?}"),
            Err(e) => panic!("parse error for {src:?}: {e:?}"),
        }
    }
    fn no_call(src: &str) {
        assert!(
            matches!(parse_directive_at(src, 0), Ok(None)),
            "expected no call for {src:?}"
        );
    }
    fn err(src: &str, code: &'static str) {
        match parse_directive_at(src, 0) {
            Err(e) => assert_eq!(e.code, code, "wrong code for {src:?}"),
            other => panic!("expected error for {src:?}, got {other:?}"),
        }
    }
    fn scalar(kind: ArgContent) -> Value {
        match kind {
            ArgContent::Scalar(v) => v,
            other => panic!("expected scalar, got {other:?}"),
        }
    }

    #[test]
    fn empty_and_plain_text_are_not_calls() {
        no_call("");
        no_call("hello world");
        no_call(".");
        no_call(".5");
        no_call(". note");
        no_call("...ellipsis");
    }

    #[test]
    fn parse_call_no_args() {
        let d = call(".note");
        assert_eq!(d.name, "note");
        assert!(d.positional_args.is_empty());
        assert!(d.named_args.is_empty());
    }

    #[test]
    fn parse_call_underscore_name() {
        let d = call(".my_call");
        assert_eq!(d.name, "my_call");
    }

    #[test]
    fn parse_call_hyphen_name() {
        let d = call(".my-call {1}");
        assert_eq!(d.name, "my-call");
    }

    #[test]
    fn parse_call_span_covers_name_and_args() {
        let d = call(".note {hello} width:{320}");
        assert_eq!(d.span, ByteSpan::new(0, 25));
    }

    #[test]
    fn parse_call_positional_scalar() {
        let d = call(".range {1} {10}");
        assert_eq!(d.positional_args.len(), 2);
        assert_eq!(
            scalar(d.positional_args[0].content.clone()),
            Value::Number(1.0)
        );
        assert_eq!(
            scalar(d.positional_args[1].content.clone()),
            Value::Number(10.0)
        );
    }

    #[test]
    fn parse_call_positional_string() {
        let d = call(".note {hello world}");
        assert_eq!(d.positional_args.len(), 1);
        assert_eq!(
            scalar(d.positional_args[0].content.clone()),
            Value::String("hello world".into())
        );
    }

    #[test]
    fn parse_call_named_args() {
        let d = call(".panel width:{320} align:{center}");
        assert_eq!(d.positional_args.len(), 0);
        assert_eq!(d.named_args.len(), 2);
        assert_eq!(d.named_args[0].0, "width");
        assert_eq!(
            scalar(d.named_args[0].1.content.clone()),
            Value::Number(320.0)
        );
        assert_eq!(d.named_args[1].0, "align");
        assert_eq!(
            scalar(d.named_args[1].1.content.clone()),
            Value::Identifier("center".into())
        );
    }

    #[test]
    fn parse_mixed_args() {
        let d = call(".panel {Introduction} width:{320}");
        assert_eq!(d.positional_args.len(), 1);
        assert_eq!(d.named_args.len(), 1);
        assert_eq!(
            scalar(d.positional_args[0].content.clone()),
            Value::Identifier("Introduction".into())
        );
    }

    #[test]
    fn parse_nested_call_in_argument() {
        let d = call(".outer {.inner {value}}");
        assert_eq!(d.positional_args.len(), 1);
        match &d.positional_args[0].content {
            ArgContent::Content(_) => {}
            other => panic!("expected content argument, got {other:?}"),
        }
    }

    #[test]
    fn parse_number_with_decimal_point() {
        let d = call(".panel width:{0.5}");
        assert_eq!(
            scalar(d.named_args[0].1.content.clone()),
            Value::Number(0.5)
        );
    }

    #[test]
    fn parse_identifier_with_underscore() {
        let d = call(".if {show_code}");
        assert_eq!(
            scalar(d.positional_args[0].content.clone()),
            Value::Identifier("show_code".into())
        );
    }

    #[test]
    fn parse_call_quoted_string_arg() {
        let d = call(".fn {\"hello \\\"world\\\"\"}");
        assert_eq!(
            scalar(d.positional_args[0].content.clone()),
            Value::String("hello \"world\"".into())
        );
    }

    #[test]
    fn parse_call_boolean_args() {
        let d = call(".fn {true} {false}");
        assert_eq!(
            scalar(d.positional_args[0].content.clone()),
            Value::Boolean(true)
        );
        assert_eq!(
            scalar(d.positional_args[1].content.clone()),
            Value::Boolean(false)
        );
    }

    #[test]
    fn positional_after_named_is_rejected() {
        err(".foo width:{\"x\"} {y}", "E2001");
    }

    #[test]
    fn unclosed_argument_is_error() {
        err(".foo {", "E2003");
        err(".foo {value", "E2003");
        err(".foo key:{", "E2003");
        err(".foo key:{value", "E2003");
    }

    #[test]
    fn named_argument_without_braces_is_error() {
        err(".foo key:", "E2002");
        err(".foo key: value", "E2002");
    }

    #[test]
    fn call_stops_at_non_argument() {
        let src = ".note and more text";
        assert!(
            matches!(parse_directive_at(src, 0), Ok(Some((c, end))) if c.name == "note" && end == 5)
        );
    }

    #[test]
    fn multiple_args_with_various_whitespace() {
        let d = call(".range {1}    {2}");
        assert_eq!(d.positional_args.len(), 2);
        let d2 = call(".range{1}{ 14}");
        assert_eq!(d2.positional_args.len(), 2);
    }
}
