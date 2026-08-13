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

/// Returns whether `name` satisfies Scribium's currently supported normal
/// call-name grammar:
/// `[A-Za-z_][A-Za-z0-9_-]*`.
///
/// This is the single public validation helper for consumers that need to
/// validate a name without parsing a complete call. The parser itself uses
/// the same byte-level predicates for call names.
pub fn is_valid_normal_call_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes.next().is_some_and(is_name_start) && bytes.all(is_name_char)
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
    fn empty_and_plain_text_are_not_calls() {
        for source in ["", "hello world", ".", ".0", ".05", ". note", "...ellipsis"] {
            assert!(matches!(parse_call(source), Ok(None)), "{source:?}");
        }
    }

    #[test]
    fn parses_implicit_positional_references_and_boundaries() {
        for (source, expected) in [(".1", "1"), (".2", "2"), (".12", "12")] {
            let (call, end) = parse_call(source).unwrap().unwrap();
            assert_eq!(call.name, expected);
            assert_eq!(call.name_span, ByteSpan::new(0, source.len()));
            assert_eq!(call.span, ByteSpan::new(0, source.len()));
            assert!(call.positional_args.is_empty());
            assert!(call.named_args.is_empty());
            assert_eq!(end, source.len());
        }

        for source in [".1abc", ".12foo", ".1한", ".1e5"] {
            assert!(matches!(parse_call(source), Ok(None)), "{source:?}");
        }
        for source in [".1-1", ".1.", ".1)", ".1!"] {
            let (call, end) = parse_call(source).unwrap().unwrap();
            assert_eq!(call.name, "1");
            assert_eq!(end, 2);
        }
    }

    #[test]
    fn implicit_references_do_not_consume_arguments() {
        let (call, end) = parse_call(".1 {item}").unwrap().unwrap();
        assert_eq!(call.name, "1");
        assert!(call.positional_args.is_empty());
        assert!(call.named_args.is_empty());
        assert_eq!(call.span, ByteSpan::new(0, 2));
        assert_eq!(end, 2);
    }

    #[test]
    fn tight_word_adjacency_and_symbol_boundaries_are_explicit() {
        for source in [".note {x}suffix", ".note {x}B", ".note {x}한"] {
            assert!(matches!(parse_call(source), Ok(None)), "{source:?}");
        }
        for (source, start) in [("A.note {x}", 1), ("한.note {x}", 3), ("..note {x}", 1)] {
            assert!(
                matches!(parse_inline_call(source, start), Ok(None)),
                "{source:?}"
            );
        }

        let (call, end) = parse_inline_call("-.note {x}", 1).unwrap().unwrap();
        assert_eq!(call.name, "note");
        assert_eq!(end, 10);
        let (call, end) = parse_call(".note {x}-").unwrap().unwrap();
        assert_eq!(call.name, "note");
        assert_eq!(end, 9);

        for source in ["0.5", "3.14", "foo.1"] {
            assert!(matches!(parse_call(source), Ok(None)), "{source:?}");
        }
    }

    #[test]
    fn parses_normal_call_names_and_spans() {
        let source = ".panel {한글} width:{320}";
        let (call, end) = parse_call(source).unwrap().unwrap();
        assert_eq!(end, source.len());
        assert_eq!(call.name, "panel");
        assert_eq!(call.positional_args.len(), 1);
        assert_eq!(call.named_args[0].name, "width");
        assert_eq!(call.name_span, ByteSpan::new(0, 6));
        assert_eq!(call.span, ByteSpan::new(0, source.len()));
        assert_eq!(parse_call(".note").unwrap().unwrap().0.name, "note");
        assert_eq!(parse_call(".my_call").unwrap().unwrap().0.name, "my_call");
        assert_eq!(
            parse_call(".my-call {1}").unwrap().unwrap().0.name,
            "my-call"
        );
    }

    #[test]
    fn parses_positional_named_and_mixed_arguments() {
        let call = parse_call(".range {1} {10}").unwrap().unwrap().0;
        assert_eq!(call.positional_args.len(), 2);
        assert_eq!(call.named_args.len(), 0);
        assert_eq!(scalar(&call.positional_args[0]), Value::Number(1.0));
        assert_eq!(scalar(&call.positional_args[1]), Value::Number(10.0));

        let call = parse_call(".panel width:{320} align:{center}")
            .unwrap()
            .unwrap()
            .0;
        assert!(call.positional_args.is_empty());
        assert_eq!(call.named_args.len(), 2);
        assert_eq!(call.named_args[0].name, "width");
        assert_eq!(call.named_args[0].name_span, ByteSpan::new(7, 12));
        assert_eq!(call.named_args[0].span, ByteSpan::new(7, 18));
        assert_eq!(scalar(&call.named_args[0].value), Value::Number(320.0));
        assert_eq!(
            scalar(&call.named_args[1].value),
            Value::Identifier("center".into())
        );

        let call = parse_call(".panel {Introduction} width:{320}")
            .unwrap()
            .unwrap()
            .0;
        assert_eq!(call.positional_args.len(), 1);
        assert_eq!(call.named_args.len(), 1);
        assert_eq!(
            scalar(&call.positional_args[0]),
            Value::Identifier("Introduction".into())
        );

        for source in [".range {1}    {2}", ".range{1}{ 14}"] {
            let call = parse_call(source).unwrap().unwrap().0;
            assert_eq!(call.positional_args.len(), 2, "{source:?}");
        }
        let (call, end) = parse_call(".note and more text").unwrap().unwrap();
        assert_eq!(call.name, "note");
        assert_eq!(end, 5);
    }

    #[test]
    fn parses_nested_content_and_scalar_classification() {
        let nested = parse_call(".outer {.nested {x}}").unwrap().unwrap().0;
        assert!(matches!(
            nested.positional_args[0].content,
            ArgContent::Content(_)
        ));

        let cases = [
            (".foo {hello}", Value::Identifier("hello".into())),
            (
                ".foo {\"hello world\"}",
                Value::String("hello world".into()),
            ),
            (".foo {show_code}", Value::Identifier("show_code".into())),
            (".foo {0.5}", Value::Number(0.5)),
            (".foo {-1}", Value::Number(-1.0)),
            (".foo {true}", Value::Boolean(true)),
            (".foo {false}", Value::Boolean(false)),
        ];
        for (source, expected) in cases {
            let call = parse_call(source).unwrap().unwrap().0;
            assert_eq!(scalar(&call.positional_args[0]), expected, "{source:?}");
        }
        let call = parse_call(".fn {\"hello \\\"world\\\"\"}")
            .unwrap()
            .unwrap()
            .0;
        assert_eq!(
            scalar(&call.positional_args[0]),
            Value::String("hello \"world\"".into())
        );
        for source in [
            ".foo {hello world}",
            ".foo {**bold**}",
            ".foo {.nested {x}}",
        ] {
            let call = parse_call(source).unwrap().unwrap().0;
            assert!(
                matches!(call.positional_args[0].content, ArgContent::Content(_)),
                "{source:?}"
            );
        }
    }

    #[test]
    fn newline_and_crlf_terminate_calls() {
        for source in [".foo\n{bar}", ".foo {a}\n{b}", ".foo {a}\r\n{b}"] {
            let (call, end) = parse_call(source).unwrap().unwrap();
            assert_eq!(call.name, "foo");
            assert_eq!(
                call.positional_args.len(),
                if source.contains("{a}") { 1 } else { 0 }
            );
            assert_eq!(end, if source.starts_with(".foo ") { 8 } else { 4 });
        }
    }

    #[test]
    fn rejects_malformed_and_ordered_arguments_without_panicking() {
        for source in [
            ".panel {unterminated",
            ".foo {",
            ".foo {value",
            ".foo key:{",
            ".foo key:{value",
        ] {
            let error = parse_call(source).unwrap_err();
            assert_eq!(error.code, "E2003", "{source:?}");
        }
        for source in [".foo key:", ".foo key: value"] {
            let error = parse_call(source).unwrap_err();
            assert_eq!(error.code, "E2002", "{source:?}");
        }
        let error = parse_call(".foo width:{\"x\"} {y}").unwrap_err();
        assert_eq!(error.code, "E2001");
    }

    #[test]
    fn public_name_validation_matches_call_name_grammar() {
        for name in [
            "a",
            "name",
            "Name",
            "_",
            "_name",
            "name123",
            "valid-name",
            "valid_name",
            "a-b-c",
        ] {
            assert!(is_valid_normal_call_name(name), "{name}");
        }
        for name in [
            "", "1name", "-name", "name!", "name.", "na me", "na\tme", "na\rme", "na\nme", "한글",
        ] {
            assert!(!is_valid_normal_call_name(name), "{name}");
        }
    }

    #[test]
    fn preserves_utf8_byte_spans() {
        let source = "한글 .text {빨강} 끝";
        let start = source.find('.').unwrap();
        let (call, _) = parse_inline_call(source, start).unwrap().unwrap();
        assert_eq!(&source[call.span.start..call.span.end], ".text {빨강}");
        assert!(source.is_char_boundary(call.name_span.start));
        assert!(source.is_char_boundary(call.name_span.end));
    }

    fn scalar(arg: &Arg) -> Value {
        match &arg.content {
            ArgContent::Scalar(value) => value.clone(),
            other => panic!("expected scalar, got {other:?}"),
        }
    }
}
