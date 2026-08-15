//! Clean-room Quarkdown call grammar.
//!
//! This crate owns only the dot-call and argument grammar. It intentionally
//! does not depend on Markdown, Rushdown, or Scribium core AST types.

use scribium_source::ByteSpan;

#[derive(Debug, Clone, PartialEq)]
pub struct QuarkdownCall {
    pub name: String,
    pub name_span: ByteSpan,
    /// The exact span of the first call segment, excluding any `::` suffix.
    pub head_span: ByteSpan,
    pub positional_args: Vec<Arg>,
    pub named_args: Vec<NamedArg>,
    /// Subsequent `::name` segments, in source order.
    ///
    /// The grammar crate deliberately preserves the chain structure without
    /// applying Quarkdown's evaluator transformation. That keeps source
    /// identity available to the frontend and leaves evaluation semantics to
    /// the owning later stage.
    pub chain: Vec<CallSegment>,
    pub span: ByteSpan,
    /// The call span before an optional tight-call wrapper is added.
    pub inner_span: ByteSpan,
    /// The complete `{.call ...}` wrapper span, when this is a tight call.
    pub wrapper_span: Option<ByteSpan>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CallSegment {
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

/// A source-backed explicit lambda header used by contextual constructs such
/// as `.function`.
#[derive(Debug, Clone, PartialEq)]
pub struct LambdaHeader {
    pub parameters: Vec<LambdaParameter>,
    pub span: ByteSpan,
}

/// One parameter in a [`LambdaHeader`].
#[derive(Debug, Clone, PartialEq)]
pub struct LambdaParameter {
    pub name: String,
    pub name_span: ByteSpan,
    pub span: ByteSpan,
    pub optional: bool,
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
    let first = parse_segment(source, start, true)?;
    if first.0.name.is_empty() {
        return Ok(None);
    }
    let mut chain = Vec::new();
    let mut cursor = first.1;
    let mut end = first.0.span.end;

    while cursor == end
        && cursor
            .checked_add(2)
            .is_some_and(|limit| source.as_bytes().get(cursor..limit) == Some(b"::"))
    {
        let chain_start = cursor;
        let segment_start = cursor + 2;
        let segment = parse_segment(source, segment_start, false).map_err(|error| {
            if error.code == "E2003" {
                error
            } else {
                ParseError::new(
                    "E2004",
                    "call chain must be followed by a valid call name",
                    ByteSpan::new(chain_start, segment_start.min(source.len())),
                )
            }
        })?;
        if segment.0.name.is_empty() {
            return Err(ParseError::new(
                "E2004",
                "call chain must be followed by a valid call name",
                ByteSpan::new(chain_start, segment_start.min(source.len())),
            ));
        }
        cursor = segment.1;
        end = segment.0.span.end;
        chain.push(segment.0);
    }

    let first = first.0;
    let head_span = first.span;
    if source.as_bytes().get(end).is_some_and(|b| is_word(*b)) {
        return Ok(None);
    }
    let span = ByteSpan::new(first.span.start, end);
    Ok(Some((
        QuarkdownCall {
            name: first.name,
            name_span: first.name_span,
            head_span,
            positional_args: first.positional_args,
            named_args: first.named_args,
            chain,
            span,
            inner_span: span,
            wrapper_span: None,
        },
        end,
    )))
}

/// Parse a tight call of the form `{.call ...}`.
pub fn parse_tight_call(
    source: &str,
    start: usize,
) -> Result<Option<(QuarkdownCall, usize)>, ParseError> {
    let bytes = source.as_bytes();
    if bytes.get(start) != Some(&b'{') {
        return Ok(None);
    }
    let inner_start = start.checked_add(1).ok_or_else(|| {
        ParseError::new(
            "E9002",
            "tight call start overflowed",
            ByteSpan::new(start, start),
        )
    })?;
    let Some((mut call, end)) = parse_directive_at(source, inner_start)? else {
        return Ok(None);
    };
    if bytes.get(end) != Some(&b'}') {
        return Ok(None);
    }
    let wrapper_end = end.checked_add(1).ok_or_else(|| {
        ParseError::new(
            "E9002",
            "tight call end overflowed",
            ByteSpan::new(start, end),
        )
    })?;
    let wrapper_span = ByteSpan::new(start, wrapper_end);
    call.inner_span = call.span;
    call.span = wrapper_span;
    call.wrapper_span = Some(wrapper_span);
    Ok(Some((call, wrapper_end)))
}

/// Return whether a physical line can only be completed by more input.
///
/// This helper is intentionally lexical and conservative. The Markdown
/// lifecycle uses it only to keep an accepted Quarkdown block header alive
/// while Rushdown advances one physical line at a time; actual acceptance
/// remains owned by `parse_call`.
pub fn needs_more_input(source: &str) -> bool {
    let bytes = source.as_bytes();
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for &byte in bytes {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' => depth = depth.saturating_add(1),
            b'}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    if depth != 0 {
        return true;
    }
    has_trailing_continuation(source)
}

/// Return whether the source ends with an unescaped continuation marker.
///
/// The marker must be the last non-newline byte. Horizontal whitespace after
/// it is deliberately not accepted as continuation syntax.
pub fn has_trailing_continuation(source: &str) -> bool {
    let line_end = source.trim_end_matches(['\r', '\n']);
    line_end.as_bytes().last() == Some(&b'\\')
}

/// Parse one contextual lambda header from an original source line.
///
/// The function deliberately does not classify arbitrary call bodies. The
/// caller selects the construct (`.function` for the current slice) and passes
/// the parser-observed body-line span. Returned spans remain absolute in the
/// supplied source; no normalized or generated text is involved.
pub fn parse_lambda_header(
    source: &str,
    line_span: ByteSpan,
) -> Result<Option<LambdaHeader>, ParseError> {
    let Some(line) = line_span.checked_str(source) else {
        return Err(ParseError::new(
            "E9002",
            "lambda header line is outside the source",
            line_span,
        ));
    };
    let line_without_terminator = line.trim_end_matches(['\r', '\n']);
    let Some(colon_offset) = line_without_terminator.rfind(':') else {
        return Ok(None);
    };
    if !line_without_terminator[colon_offset + 1..]
        .trim()
        .is_empty()
    {
        return Ok(None);
    }

    let header_end = line_span
        .start
        .checked_add(colon_offset + 1)
        .ok_or_else(|| ParseError::new("E9002", "lambda header span overflowed", line_span))?;
    let content = &line_without_terminator[..colon_offset];
    let leading = content.len() - content.trim_start_matches([' ', '\t']).len();
    let content = content.trim();
    let header_start = line_span
        .start
        .checked_add(leading)
        .ok_or_else(|| ParseError::new("E9002", "lambda header span overflowed", line_span))?;
    if content.is_empty() {
        return Err(ParseError::new(
            "E2005",
            "lambda header must contain at least one parameter",
            ByteSpan::new(header_start, header_end),
        ));
    }

    let mut parameters = Vec::new();
    let mut cursor = 0usize;
    for token in content.split_whitespace() {
        let token_start = content[cursor..]
            .find(token)
            .and_then(|offset| cursor.checked_add(offset))
            .ok_or_else(|| {
                ParseError::new(
                    "E9002",
                    "lambda parameter span could not be mapped",
                    line_span,
                )
            })?;
        let token_end = token_start.checked_add(token.len()).ok_or_else(|| {
            ParseError::new("E9002", "lambda parameter span overflowed", line_span)
        })?;
        cursor = token_end;

        let optional = token.ends_with('?');
        let name = if optional {
            token.strip_suffix('?').unwrap_or_default()
        } else {
            token
        };
        if !is_valid_normal_call_name(name) {
            let absolute_start = header_start.checked_add(token_start).ok_or_else(|| {
                ParseError::new("E9002", "lambda parameter span overflowed", line_span)
            })?;
            let absolute_end = header_start.checked_add(token_end).ok_or_else(|| {
                ParseError::new("E9002", "lambda parameter span overflowed", line_span)
            })?;
            return Err(ParseError::new(
                "E2005",
                format!("invalid lambda parameter name `{token}`"),
                ByteSpan::new(absolute_start, absolute_end),
            ));
        }
        let absolute_start = header_start.checked_add(token_start).ok_or_else(|| {
            ParseError::new("E9002", "lambda parameter span overflowed", line_span)
        })?;
        let absolute_end = header_start.checked_add(token_end).ok_or_else(|| {
            ParseError::new("E9002", "lambda parameter span overflowed", line_span)
        })?;
        let token_span = ByteSpan::new(absolute_start, absolute_end);
        let name_span = ByteSpan::new(
            token_span.start,
            token_span.end - if optional { 1 } else { 0 },
        );
        parameters.push(LambdaParameter {
            name: name.to_string(),
            name_span,
            span: token_span,
            optional,
        });
    }

    Ok(Some(LambdaHeader {
        parameters,
        span: ByteSpan::new(header_start, header_end),
    }))
}

fn parse_segment(
    source: &str,
    start: usize,
    dotted: bool,
) -> Result<(CallSegment, usize), ParseError> {
    let bytes = source.as_bytes();
    let name_start = if dotted {
        if bytes.get(start) != Some(&b'.') {
            return Ok((
                CallSegment {
                    name: String::new(),
                    name_span: ByteSpan::new(start, start),
                    positional_args: Vec::new(),
                    named_args: Vec::new(),
                    span: ByteSpan::new(start, start),
                },
                start,
            ));
        }
        start + 1
    } else {
        start
    };
    let Some(&first) = bytes.get(name_start) else {
        if dotted {
            return Ok((
                CallSegment {
                    name: String::new(),
                    name_span: ByteSpan::new(start, start),
                    positional_args: Vec::new(),
                    named_args: Vec::new(),
                    span: ByteSpan::new(start, start),
                },
                start,
            ));
        }
        return Err(ParseError::new(
            "E2004",
            "call chain must be followed by a valid call name",
            ByteSpan::new(start.min(source.len()), source.len()),
        ));
    };

    if dotted && first.is_ascii_digit() && first != b'0' {
        let mut name_end = name_start;
        while bytes.get(name_end).is_some_and(|b| b.is_ascii_digit()) {
            name_end += 1;
        }
        if bytes.get(name_end).is_some_and(|b| is_word(*b)) {
            return Ok((
                CallSegment {
                    name: String::new(),
                    name_span: ByteSpan::new(start, start),
                    positional_args: Vec::new(),
                    named_args: Vec::new(),
                    span: ByteSpan::new(start, start),
                },
                start,
            ));
        }
        return Ok((
            CallSegment {
                name: source[name_start..name_end].to_string(),
                name_span: ByteSpan::new(start, name_end),
                positional_args: Vec::new(),
                named_args: Vec::new(),
                span: ByteSpan::new(start, name_end),
            },
            name_end,
        ));
    }
    if !is_name_start(first) {
        if dotted {
            return Ok((
                CallSegment {
                    name: String::new(),
                    name_span: ByteSpan::new(start, start),
                    positional_args: Vec::new(),
                    named_args: Vec::new(),
                    span: ByteSpan::new(start, start),
                },
                start,
            ));
        }
        return Err(ParseError::new(
            "E2004",
            "call chain must be followed by a valid call name",
            ByteSpan::new(start.min(source.len()), (name_start + 1).min(source.len())),
        ));
    }
    let mut name_end = name_start;
    while bytes.get(name_end).is_some_and(|b| is_name_char(*b)) {
        name_end += 1;
    }
    let parsed = parse_arguments(source, name_end)?;
    let span = ByteSpan::new(if dotted { start } else { name_start }, parsed.end);
    Ok((
        CallSegment {
            name: source[name_start..name_end].to_string(),
            name_span: ByteSpan::new(if dotted { start } else { name_start }, name_end),
            positional_args: parsed.positional_args,
            named_args: parsed.named_args,
            span,
        },
        parsed.cursor,
    ))
}

struct ParsedArguments {
    positional_args: Vec<Arg>,
    named_args: Vec<NamedArg>,
    end: usize,
    cursor: usize,
}

fn parse_arguments(source: &str, after_name: usize) -> Result<ParsedArguments, ParseError> {
    let bytes = source.as_bytes();
    let mut cursor = skip_horizontal(bytes, after_name);
    let mut end = after_name;
    let mut positional_args = Vec::new();
    let mut named_args = Vec::new();
    let mut require_argument = false;

    loop {
        let Some(&byte) = bytes.get(cursor) else {
            if require_argument {
                return Err(ParseError::new(
                    "E2004",
                    "line continuation must be followed by an argument",
                    ByteSpan::new(cursor.saturating_sub(1), cursor),
                ));
            }
            break;
        };
        if byte == b'{' {
            if !named_args.is_empty() {
                return Err(ParseError::new(
                    "E2001",
                    "positional argument after named argument is not allowed",
                    ByteSpan::new(cursor, cursor + 1),
                ));
            }
            let arg = parse_braced(source, cursor)?;
            end = arg.span.end;
            cursor = arg.span.end;
            positional_args.push(arg);
        } else {
            let arg_name_start = cursor;
            while bytes.get(cursor).is_some_and(|b| is_name_char(*b)) {
                cursor += 1;
            }
            if cursor == arg_name_start {
                if require_argument {
                    return Err(ParseError::new(
                        "E2004",
                        "line continuation must be followed by an argument",
                        ByteSpan::new(arg_name_start, (arg_name_start + 1).min(source.len())),
                    ));
                }
                break;
            }
            let after_name = skip_horizontal(bytes, cursor);
            if bytes.get(after_name) != Some(&b':') {
                if require_argument {
                    return Err(ParseError::new(
                        "E2004",
                        "line continuation must be followed by an argument",
                        ByteSpan::new(arg_name_start, cursor),
                    ));
                }
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
            end = value.span.end;
            named_args.push(NamedArg {
                name: source[arg_name_start..cursor].to_string(),
                name_span: ByteSpan::new(arg_name_start, cursor),
                value,
                span: ByteSpan::new(arg_name_start, end),
            });
            cursor = end;
        }

        let next = skip_horizontal(bytes, cursor);
        if let Some(after_continuation) = consume_continuation(bytes, next) {
            let next_argument = skip_line_indentation(bytes, after_continuation);
            if next_argument >= bytes.len() || bytes[next_argument] == b'\n' {
                return Err(ParseError::new(
                    "E2004",
                    "line continuation must be followed by an argument",
                    ByteSpan::new(next, (next + 1).min(source.len())),
                ));
            }
            cursor = next_argument;
            require_argument = true;
        } else {
            cursor = next;
            require_argument = false;
            if bytes.get(cursor).is_some_and(|b| *b == b':') {
                break;
            }
        }
    }

    Ok(ParsedArguments {
        positional_args,
        named_args,
        end,
        cursor,
    })
}

fn parse_braced(source: &str, open: usize) -> Result<Arg, ParseError> {
    let bytes = source.as_bytes();
    let mut depth = 1usize;
    let mut cursor = open + 1;
    let mut in_string = false;
    let mut escaped = false;
    while cursor < bytes.len() {
        let byte = bytes[cursor];
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
        } else {
            match byte {
                b'"' => in_string = true,
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
                _ => {}
            }
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

fn skip_line_indentation(bytes: &[u8], mut cursor: usize) -> usize {
    while bytes.get(cursor).is_some_and(|b| matches!(b, b' ' | b'\t')) {
        cursor += 1;
    }
    cursor
}

fn consume_continuation(bytes: &[u8], cursor: usize) -> Option<usize> {
    if bytes.get(cursor) != Some(&b'\\') {
        return None;
    }
    match (bytes.get(cursor + 1), bytes.get(cursor + 2)) {
        (Some(b'\n'), _) => Some(cursor + 2),
        (Some(b'\r'), Some(b'\n')) => Some(cursor + 3),
        _ => None,
    }
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
    fn parses_multiline_nested_arguments_with_original_spans() {
        let source = ".divide {\n  .cos {.pi}\n} by:{\n  .sum {2} {1}\n}";
        let (call, end) = parse_call(source).unwrap().unwrap();
        assert_eq!(end, source.len());
        assert_eq!(call.positional_args.len(), 1);
        assert_eq!(call.named_args.len(), 1);
        assert_eq!(call.span, ByteSpan::new(0, source.len()));
        let ArgContent::Content(content) = call.positional_args[0].content else {
            panic!("expected multiline content argument")
        };
        assert_eq!(&source[content.start..content.end], "\n  .cos {.pi}\n");
        assert_eq!(
            &source[call.named_args[0].value.span.start..call.named_args[0].value.span.end],
            "{\n  .sum {2} {1}\n}"
        );
        for span in [call.span, call.name_span, content, call.named_args[0].span] {
            assert!(span.is_valid_for(source));
        }

        let crlf = concat!(".call {\r\n  한글\r\n} \\", "\r\n  next:{값}");
        let (call, end) = parse_call(crlf).unwrap().unwrap();
        assert_eq!(end, crlf.len());
        assert_eq!(call.named_args.len(), 1);
        assert!(call.span.is_valid_for(crlf));
        let ArgContent::Content(content) = call.positional_args[0].content else {
            panic!("expected CRLF content argument")
        };
        assert_eq!(&crlf[content.start..content.end], "\r\n  한글\r\n");
    }

    #[test]
    fn parses_line_continuations_without_fixed_indentation() {
        for (source, positional, named) in [
            (concat!(".call {a} \\", "\n{b}"), 2, 0),
            (concat!(".call {a} \\", "\n  {b}"), 2, 0),
            (concat!(".call {a} \\", "\n        {b}"), 2, 0),
            (concat!(".call {a} \\", "\n\t{b}"), 2, 0),
            (concat!(".call {a} \\", "\n  {b} \\", "\n  {c}"), 3, 0),
            (concat!(".call first:{a} \\", "\n  second:{b}"), 0, 2),
        ] {
            let (call, end) = parse_call(source).unwrap().unwrap();
            assert_eq!(end, source.len(), "{source:?}");
            assert_eq!(call.positional_args.len(), positional, "{source:?}");
            assert_eq!(call.named_args.len(), named, "{source:?}");
            assert!(call.span.is_valid_for(source), "{source:?}");
        }
    }

    #[test]
    fn parses_chains_as_source_backed_segments_without_rewriting() {
        for source in [
            ".a::b",
            ".a::b::c",
            ".a {x}::b {y}",
            ".a first:{x}::b second:{y}",
        ] {
            let (call, end) = parse_call(source).unwrap().unwrap();
            assert_eq!(end, source.len(), "{source:?}");
            assert_eq!(call.chain.len(), source.matches("::").count());
            assert_eq!(&source[call.span.start..call.span.end], source);
            assert!(call.span.is_valid_for(source));
            for segment in &call.chain {
                assert!(segment.span.is_valid_for(source));
                assert!(segment.name_span.is_valid_for(source));
            }
        }
        let (call, _) = parse_call(".a {x}::b {y}").unwrap().unwrap();
        assert_eq!(call.name, "a");
        assert_eq!(call.head_span, ByteSpan::new(0, 6));
        assert_eq!(
            &".a {x}::b {y}"[call.head_span.start..call.head_span.end],
            ".a {x}"
        );
        assert_eq!(call.chain[0].name, "b");
        assert_eq!(call.chain[0].span, ByteSpan::new(8, 13));
        assert_eq!(call.positional_args.len(), 1);
        assert_eq!(call.chain[0].positional_args.len(), 1);
    }

    #[test]
    fn rejects_malformed_chains_deterministically() {
        for source in [".a::", ".a:::b", ".a:: {x}", ".a::1abc"] {
            let error = parse_call(source).unwrap_err();
            assert_eq!(error.code, "E2004", "{source:?}");
            assert!(error.span.is_valid_for(source), "{source:?}");
        }
        for source in [
            concat!(".call {a} \\", "\n"),
            concat!(".call {a} \\", "\n\nnext"),
        ] {
            let error = parse_call(source).unwrap_err();
            assert_eq!(error.code, "E2004", "{source:?}");
            assert!(error.span.is_valid_for(source), "{source:?}");
        }
    }

    #[test]
    fn parses_tight_calls_and_preserves_inner_provenance() {
        for source in [
            "{.note}",
            "H{.text {2}}O",
            "한{.note}글",
            "{.a::b}",
            "A{.a::b}B",
        ] {
            let start = source.find('{').unwrap();
            let expected_end = source.rfind('}').unwrap() + 1;
            let (call, end) = parse_tight_call(source, start).unwrap().unwrap();
            assert_eq!(end, expected_end);
            let wrapper = call.wrapper_span.expect("tight wrapper");
            assert_eq!(
                &source[wrapper.start..wrapper.end],
                &source[start..expected_end]
            );
            assert_eq!(
                &source[call.inner_span.start..call.inner_span.end],
                &source[start + 1..expected_end - 1]
            );
            assert!(wrapper.is_valid_for(source));
            assert!(call.inner_span.is_valid_for(source));
            assert!(call.name_span.is_valid_for(source));
        }
        assert!(parse_tight_call("{not a call}", 0).unwrap().is_none());
        assert!(parse_tight_call("{.note", 0).unwrap().is_none());
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

    #[test]
    fn parses_contextual_lambda_headers_with_exact_spans() {
        let source = "한글\r\n  alpha beta?:\r\n  body\r\n";
        let line_start = source.find("alpha").unwrap() - 2;
        let line_end = source[line_start..].find("\r\n").unwrap() + line_start;
        let header = parse_lambda_header(source, ByteSpan::new(line_start, line_end))
            .unwrap()
            .unwrap();

        assert_eq!(header.parameters.len(), 2);
        assert_eq!(header.parameters[0].name, "alpha");
        assert_eq!(header.parameters[1].name, "beta");
        assert!(header.parameters[1].optional);
        assert_eq!(
            &source[header.parameters[0].name_span.start..header.parameters[0].name_span.end],
            "alpha"
        );
        assert_eq!(
            &source[header.parameters[1].span.start..header.parameters[1].span.end],
            "beta?"
        );
        assert_eq!(&source[header.span.start..header.span.end], "alpha beta?:");
        for parameter in &header.parameters {
            assert!(parameter.name_span.is_valid_for(source));
            assert!(parameter.span.is_valid_for(source));
        }
    }

    #[test]
    fn lambda_header_parser_is_contextual_and_rejects_malformed_headers() {
        let source = "Hello:\n\nalpha beta??:\n\n:\n";
        let hello_end = source.find('\n').unwrap();
        assert_eq!(
            parse_lambda_header(source, ByteSpan::new(0, hello_end)).unwrap(),
            Some(LambdaHeader {
                parameters: vec![LambdaParameter {
                    name: "Hello".into(),
                    name_span: ByteSpan::new(0, 5),
                    span: ByteSpan::new(0, 5),
                    optional: false,
                }],
                span: ByteSpan::new(0, 6),
            })
        );
        let malformed_start = source.find("alpha").unwrap();
        let malformed_end = source[malformed_start..].find('\n').unwrap() + malformed_start;
        let error =
            parse_lambda_header(source, ByteSpan::new(malformed_start, malformed_end)).unwrap_err();
        assert_eq!(error.code, "E2005");
        let empty_start = source.rfind(':').unwrap();
        let error =
            parse_lambda_header(source, ByteSpan::new(empty_start, source.len())).unwrap_err();
        assert_eq!(error.code, "E2005");
    }

    fn scalar(arg: &Arg) -> Value {
        match &arg.content {
            ArgContent::Scalar(value) => value.clone(),
            other => panic!("expected scalar, got {other:?}"),
        }
    }
}
