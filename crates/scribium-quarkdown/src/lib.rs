//! Clean-room Quarkdown call grammar.
//!
//! This crate owns only the dot-call and argument grammar. It intentionally
//! does not depend on Markdown, Rushdown, or Scribium core AST types.

use scribium_source::ByteSpan;

#[derive(Debug, Clone, PartialEq)]
pub struct QuarkdownCall {
    pub name: String,
    pub name_span: ByteSpan,
    pub positional_args: Vec<Arg>,
    pub named_args: Vec<NamedArg>,
    pub span: ByteSpan,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Arg {
    pub content: ArgContent,
    pub span: ByteSpan,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArgContent {
    Scalar(Value),
    Content(ByteSpan),
}

#[derive(Debug, Clone, PartialEq)]
pub struct NamedArg {
    pub name: String,
    pub name_span: ByteSpan,
    pub value: Arg,
    pub span: ByteSpan,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    String(String),
    Number(f64),
    Boolean(bool),
    Identifier(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub code: &'static str,
    pub message: String,
    pub span: ByteSpan,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ParseError {}

pub fn parse_call(source: &str) -> Result<Option<(QuarkdownCall, usize)>, ParseError> {
    parse_directive_at(source, 0)
}

pub fn parse_inline_call(
    source: &str,
    start: usize,
) -> Result<Option<(QuarkdownCall, usize)>, ParseError> {
    parse_directive_at(source, start)
}

pub fn parse_directive_at(
    source: &str,
    start: usize,
) -> Result<Option<(QuarkdownCall, usize)>, ParseError> {
    let bytes = source.as_bytes();
    if start >= bytes.len() || bytes[start] != b'.' || !valid_boundary(bytes, start) {
        return Ok(None);
    }
    let name_start = start + 1;
    let Some(&first) = bytes.get(name_start) else {
        return Ok(None);
    };
    let mut name_end = name_start;
    if first.is_ascii_digit() && first != b'0' {
        while bytes.get(name_end).is_some_and(|b| b.is_ascii_digit()) {
            name_end += 1;
        }
        if bytes.get(name_end).is_some_and(|b| is_word(*b)) {
            return Ok(None);
        }
        return Ok(Some((
            QuarkdownCall {
                name: source[name_start..name_end].to_string(),
                name_span: ByteSpan::new(start, name_end),
                positional_args: Vec::new(),
                named_args: Vec::new(),
                span: ByteSpan::new(start, name_end),
            },
            name_end,
        )));
    }
    if !is_name_start(first) {
        return Ok(None);
    }
    while bytes.get(name_end).is_some_and(|b| is_name_char(*b)) {
        name_end += 1;
    }

    let mut cursor = skip_horizontal(bytes, name_end);
    let mut call_end = name_end;
    let mut positional_args = Vec::new();
    let mut named_args = Vec::new();
    while let Some(&byte) = bytes.get(cursor) {
        if byte == b'{' {
            if !named_args.is_empty() {
                return Err(ParseError::new(
                    "E2001",
                    "positional argument after named argument is not allowed",
                    ByteSpan::new(cursor, cursor + 1),
                ));
            }
            let arg = parse_braced(source, cursor)?;
            cursor = skip_horizontal(bytes, arg.span.end);
            call_end = arg.span.end;
            positional_args.push(arg);
            continue;
        }
        let arg_name_start = cursor;
        while bytes.get(cursor).is_some_and(|b| is_name_char(*b)) {
            cursor += 1;
        }
        if cursor == arg_name_start {
            break;
        }
        let after_name = skip_horizontal(bytes, cursor);
        if bytes.get(after_name) != Some(&b':') {
            break;
        }
        let open = skip_horizontal(bytes, after_name + 1);
        if bytes.get(open) != Some(&b'{') {
            return Err(ParseError::new(
                "E2002",
                "named argument must be followed by a braced value",
                ByteSpan::new(arg_name_start, after_name + 1),
            ));
        }
        let value = parse_braced(source, open)?;
        let end = value.span.end;
        named_args.push(NamedArg {
            name: source[arg_name_start..cursor].to_string(),
            name_span: ByteSpan::new(arg_name_start, cursor),
            value,
            span: ByteSpan::new(arg_name_start, end),
        });
        call_end = end;
        cursor = skip_horizontal(bytes, end);
    }
    if bytes.get(call_end).is_some_and(|b| is_word(*b)) {
        return Ok(None);
    }
    let end = call_end;
    Ok(Some((
        QuarkdownCall {
            name: source[name_start..name_end].to_string(),
            name_span: ByteSpan::new(start, name_end),
            positional_args,
            named_args,
            span: ByteSpan::new(start, end),
        },
        end,
    )))
}

fn parse_braced(source: &str, open: usize) -> Result<Arg, ParseError> {
    let bytes = source.as_bytes();
    let mut depth = 1usize;
    let mut cursor = open + 1;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    let content = ByteSpan::new(open + 1, cursor);
                    let content_kind = parse_scalar(source, content)
                        .map(ArgContent::Scalar)
                        .unwrap_or(ArgContent::Content(content));
                    return Ok(Arg {
                        content: content_kind,
                        span: ByteSpan::new(open, cursor + 1),
                    });
                }
            }
            b'\n' => {
                return Err(ParseError::new(
                    "E2003",
                    "unclosed `{...}` argument",
                    ByteSpan::new(open, cursor + 1),
                ));
            }
            _ => {}
        }
        cursor += 1;
    }
    Err(ParseError::new(
        "E2003",
        "unclosed `{...}` argument",
        ByteSpan::new(open, source.len()),
    ))
}

fn parse_scalar(source: &str, span: ByteSpan) -> Option<Value> {
    let raw = source.get(span.start..span.end)?.trim();
    if raw.is_empty() {
        return Some(Value::String(String::new()));
    }
    if raw.len() >= 2 && raw.starts_with('"') && raw.ends_with('"') {
        return Some(Value::String(
            raw[1..raw.len() - 1]
                .replace("\\\"", "\"")
                .replace("\\\\", "\\"),
        ));
    }
    if let Ok(number) = raw.parse::<f64>() {
        return Some(Value::Number(number));
    }
    match raw {
        "true" => return Some(Value::Boolean(true)),
        "false" => return Some(Value::Boolean(false)),
        _ => {}
    }
    if raw
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
        && raw
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
    {
        return Some(Value::Identifier(raw.to_string()));
    }
    None
}

fn skip_horizontal(bytes: &[u8], mut cursor: usize) -> usize {
    while bytes.get(cursor).is_some_and(|b| matches!(b, b' ' | b'\t')) {
        cursor += 1;
    }
    cursor
}

fn is_name_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

fn is_name_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')
}

fn is_word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || !byte.is_ascii()
}

fn valid_boundary(bytes: &[u8], start: usize) -> bool {
    start == 0 || (bytes[start - 1] != b'.' && !is_word(bytes[start - 1]))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mixed_arguments() {
        let source = ".panel {한글} width:{320}";
        let (call, end) = parse_call(source).unwrap().unwrap();
        assert_eq!(end, source.len());
        assert_eq!(call.name, "panel");
        assert_eq!(call.positional_args.len(), 1);
        assert_eq!(call.named_args[0].name, "width");
    }

    #[test]
    fn rejects_malformed_arguments_without_panicking() {
        let error = parse_call(".panel {unterminated").unwrap_err();
        assert_eq!(error.code, "E2003");
    }

    #[test]
    fn preserves_non_ascii_byte_offsets() {
        let source = "한글 .text {빨강} 끝";
        let start = source.find('.').unwrap();
        let (call, _) = parse_inline_call(source, start).unwrap().unwrap();
        assert_eq!(&source[call.span.start..call.span.end], ".text {빨강}");
    }
}
