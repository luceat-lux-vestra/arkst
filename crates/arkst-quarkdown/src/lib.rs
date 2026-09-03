//! Clean-room Quarkdown call grammar.
//!
//! This crate owns only the dot-call and argument grammar. It intentionally
//! does not depend on Markdown, Rushdown, or Arkst core AST types.

use arkst_source::ByteSpan;

#[derive(Debug, Clone, PartialEq)]
pub struct QuarkdownCall {
    pub name: String,
    pub name_span: ByteSpan,
    /// The exact span of the first call segment, excluding any `::` suffix.
    pub head_span: ByteSpan,
    /// Arguments in their original source order.
    ///
    /// The grammar deliberately does not validate positional/named ordering;
    /// that is a binder concern. Keeping one sequence here prevents the
    /// frontend from losing the shape needed by that later validation.
    pub arguments: Vec<CallArgument>,
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
    /// Arguments in their original source order.
    pub arguments: Vec<CallArgument>,
    pub span: ByteSpan,
}

impl QuarkdownCall {
    /// Returns the positional subset as a derived compatibility projection.
    /// The ordered [`Self::arguments`] sequence remains canonical.
    pub fn positional_args(&self) -> Vec<&Arg> {
        self.arguments
            .iter()
            .filter_map(|argument| match argument {
                CallArgument::Positional(argument) => Some(argument),
                CallArgument::Named(_) => None,
            })
            .collect()
    }

    /// Returns the named subset as a derived compatibility projection.
    /// The ordered [`Self::arguments`] sequence remains canonical.
    pub fn named_args(&self) -> Vec<&NamedArg> {
        self.arguments
            .iter()
            .filter_map(|argument| match argument {
                CallArgument::Positional(_) => None,
                CallArgument::Named(argument) => Some(argument),
            })
            .collect()
    }
}

impl CallSegment {
    /// Returns the positional subset as a derived compatibility projection.
    /// The ordered [`Self::arguments`] sequence remains canonical.
    pub fn positional_args(&self) -> Vec<&Arg> {
        self.arguments
            .iter()
            .filter_map(|argument| match argument {
                CallArgument::Positional(argument) => Some(argument),
                CallArgument::Named(_) => None,
            })
            .collect()
    }

    /// Returns the named subset as a derived compatibility projection.
    /// The ordered [`Self::arguments`] sequence remains canonical.
    pub fn named_args(&self) -> Vec<&NamedArg> {
        self.arguments
            .iter()
            .filter_map(|argument| match argument {
                CallArgument::Positional(_) => None,
                CallArgument::Named(argument) => Some(argument),
            })
            .collect()
    }
}

/// One source-backed argument in a call, retained in source order.
#[derive(Debug, Clone, PartialEq)]
pub enum CallArgument {
    Positional(Arg),
    Named(NamedArg),
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

/// A source-backed inline lambda expression.
#[derive(Debug, Clone, PartialEq)]
pub struct InlineLambda {
    pub parameters: Vec<LambdaParameter>,
    pub implicit: bool,
    pub body: ByteSpan,
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
    Range(RangeValue),
}

/// A source-backed integer range literal.
///
/// The endpoints are optional because Quarkdown accepts open range syntax.
/// Range consumption, including whether an open range can be made finite, is
/// a semantic concern of the evaluator rather than this grammar crate.
#[derive(Debug, Clone, PartialEq)]
pub struct RangeValue {
    pub start: Option<u64>,
    pub end: Option<u64>,
    pub span: ByteSpan,
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

    loop {
        let separator = scan_argument_separator(bytes, cursor);
        let chain_start = separator.end;
        if !chain_start
            .checked_add(2)
            .is_some_and(|limit| source.as_bytes().get(chain_start..limit) == Some(b"::"))
        {
            break;
        }
        let segment_start = chain_start + 2;
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
    let span = ByteSpan::new(first.span.start, end);
    Ok(Some((
        QuarkdownCall {
            name: first.name,
            name_span: first.name_span,
            head_span,
            arguments: first.arguments,
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
    for (cursor, &byte) in bytes.iter().enumerate() {
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
            b'{' if !is_escaped_delimiter(bytes, cursor) => depth = depth.saturating_add(1),
            b'}' if !is_escaped_delimiter(bytes, cursor) => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    if depth != 0 {
        return true;
    }
    has_trailing_continuation(source)
}

fn is_escaped_delimiter(bytes: &[u8], cursor: usize) -> bool {
    cursor > 0 && bytes[cursor - 1] == b'\\'
}

/// Return whether the source ends with an unescaped line continuation.
///
/// The continuation marker must be immediately followed by LF or CRLF. The
/// optional indentation after that line ending is part of the continuation.
pub fn has_trailing_continuation(source: &str) -> bool {
    let bytes = source.as_bytes();
    let mut end = bytes.len();
    while end > 0 && matches!(bytes[end - 1], b' ' | b'\t') {
        end -= 1;
    }
    let Some(&last) = end.checked_sub(1).and_then(|index| bytes.get(index)) else {
        return false;
    };
    if last != b'\n' {
        return false;
    }
    let mut continuation_end = end - 1;
    if continuation_end > 0 && bytes[continuation_end - 1] == b'\r' {
        continuation_end -= 1;
    }
    continuation_end > 0 && bytes[continuation_end - 1] == b'\\'
}

/// Parse one contextual lambda header from an original source line.
///
/// The function deliberately does not classify arbitrary call bodies. The
/// caller selects the construct (`.function` or block-form `.let` in the
/// current slice) and passes the parser-observed body-line span. Returned
/// spans remain absolute in the supplied source; no normalized or generated
/// text is involved.
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

/// Parses the explicitly marked inline lambda form used as a first-class
/// value, for example `@lambda item: .item` or `@lambda .1`.
///
/// Unmarked content is intentionally not classified here. Ordinary content
/// arguments such as `key: value` must remain content unless a surrounding
/// callable construct has selected block-lambda semantics.
pub fn parse_inline_lambda(
    source: &str,
    span: ByteSpan,
) -> Result<Option<InlineLambda>, ParseError> {
    parse_inline_lambda_inner(source, span, true)
}

/// Parses a lambda in a callback argument selected by a transform builtin.
///
/// Quarkdown's documented inline callback form does not require the legacy
/// `@lambda` marker. The caller must provide the surrounding semantic context;
/// ordinary content arguments are never sent through this entry point.
pub fn parse_callback_lambda(
    source: &str,
    span: ByteSpan,
) -> Result<Option<InlineLambda>, ParseError> {
    parse_inline_lambda_inner(source, span, false)
}

fn parse_inline_lambda_inner(
    source: &str,
    span: ByteSpan,
    marker_required: bool,
) -> Result<Option<InlineLambda>, ParseError> {
    let Some(raw) = span.checked_str(source) else {
        return Err(ParseError::new(
            "E9002",
            "inline lambda is outside the source",
            span,
        ));
    };
    let leading = raw.len() - raw.trim_start_matches([' ', '\t', '\r', '\n']).len();
    let marker_start = span
        .start
        .checked_add(leading)
        .ok_or_else(|| ParseError::new("E9002", "inline lambda span overflowed", span))?;
    let (expression, expression_start) =
        if let Some(marked) = raw[leading..].strip_prefix("@lambda") {
            if marked.as_bytes().first().is_some_and(|byte| is_word(*byte)) {
                return Ok(None);
            }
            let marker_end = marker_start
                .checked_add("@lambda".len())
                .ok_or_else(|| ParseError::new("E9002", "inline lambda span overflowed", span))?;
            let expression_leading = marked.len() - marked.trim_start_matches([' ', '\t']).len();
            let expression_start = marker_end
                .checked_add(expression_leading)
                .ok_or_else(|| ParseError::new("E9002", "inline lambda span overflowed", span))?;
            (&marked[expression_leading..], expression_start)
        } else if marker_required {
            return Ok(None);
        } else {
            let expression = raw[leading..].trim_start_matches([' ', '\t']);
            let expression_start = marker_start
                .checked_add(raw[leading..].len() - expression.len())
                .ok_or_else(|| ParseError::new("E9002", "inline lambda span overflowed", span))?;
            (expression, expression_start)
        };

    // Quarkdown's lambda grammar attempts the explicit header form first,
    // then treats the complete expression as an implicit body when no valid
    // parameter-list/colon prefix exists. This matters for bodies containing
    // named arguments, such as `.predicate {.1} than:{5}`: the colon belongs
    // to the nested call, not to the lambda header.
    let Some(colon) = expression.match_indices(':').find_map(|(colon, _)| {
        let header = expression[..colon].trim();
        (!header.is_empty()
            && header.split_whitespace().all(|token| {
                let name = token.strip_suffix('?').unwrap_or(token);
                is_valid_normal_call_name(name)
            }))
        .then_some(colon)
    }) else {
        return Ok(Some(InlineLambda {
            parameters: Vec::new(),
            implicit: true,
            body: ByteSpan::new(expression_start, span.end),
            span,
        }));
    };
    let header = expression[..colon].trim();
    let body_text = &expression[colon + 1..];
    let body_leading =
        body_text.len() - body_text.trim_start_matches([' ', '\t', '\r', '\n']).len();
    let body_start = expression_start
        .checked_add(colon + 1)
        .and_then(|value| value.checked_add(body_leading))
        .ok_or_else(|| ParseError::new("E9002", "inline lambda span overflowed", span))?;
    let header_leading = expression[..colon].len() - expression[..colon].trim_start().len();
    let header_start = expression_start
        .checked_add(header_leading)
        .ok_or_else(|| ParseError::new("E9002", "inline lambda span overflowed", span))?;
    let mut parameters = Vec::new();
    let mut cursor = 0usize;
    for token in header.split_whitespace() {
        let token_start = header[cursor..]
            .find(token)
            .and_then(|offset| cursor.checked_add(offset))
            .ok_or_else(|| {
                ParseError::new(
                    "E9002",
                    "inline lambda parameter span could not be mapped",
                    span,
                )
            })?;
        let token_end = token_start
            .checked_add(token.len())
            .ok_or_else(|| ParseError::new("E9002", "inline lambda span overflowed", span))?;
        cursor = token_end;
        let optional = token.ends_with('?');
        let name = token.strip_suffix('?').unwrap_or(token);
        let absolute_start = header_start
            .checked_add(token_start)
            .ok_or_else(|| ParseError::new("E9002", "inline lambda span overflowed", span))?;
        let absolute_end = header_start
            .checked_add(token_end)
            .ok_or_else(|| ParseError::new("E9002", "inline lambda span overflowed", span))?;
        if !is_valid_normal_call_name(name) {
            return Err(ParseError::new(
                "E2005",
                format!("invalid inline lambda parameter name `{token}`"),
                ByteSpan::new(absolute_start, absolute_end),
            ));
        }
        let token_span = ByteSpan::new(absolute_start, absolute_end);
        parameters.push(LambdaParameter {
            name: name.to_string(),
            name_span: ByteSpan::new(absolute_start, absolute_end - usize::from(optional)),
            span: token_span,
            optional,
        });
    }
    Ok(Some(InlineLambda {
        parameters,
        implicit: false,
        body: ByteSpan::new(body_start, span.end),
        span,
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
                    arguments: Vec::new(),
                    span: ByteSpan::new(start, start),
                },
                start,
            ));
        }
        start + 1
    } else {
        start
    };
    let Some(_) = bytes.get(name_start) else {
        if dotted {
            return Ok((
                CallSegment {
                    name: String::new(),
                    name_span: ByteSpan::new(start, start),
                    arguments: Vec::new(),
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

    let Some(name_end) = scan_identifier(bytes, name_start) else {
        if dotted {
            return Ok((
                CallSegment {
                    name: String::new(),
                    name_span: ByteSpan::new(start, start),
                    arguments: Vec::new(),
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
    };
    let parsed = parse_arguments(source, name_end)?;
    let span = ByteSpan::new(if dotted { start } else { name_start }, parsed.end);
    Ok((
        CallSegment {
            name: source[name_start..name_end].to_string(),
            name_span: ByteSpan::new(if dotted { start } else { name_start }, name_end),
            arguments: parsed.arguments,
            span,
        },
        parsed.cursor,
    ))
}

struct ParsedArguments {
    arguments: Vec<CallArgument>,
    end: usize,
    cursor: usize,
}

fn parse_arguments(source: &str, after_name: usize) -> Result<ParsedArguments, ParseError> {
    let bytes = source.as_bytes();
    let mut cursor = after_name;
    let mut end = after_name;
    let mut arguments = Vec::new();

    loop {
        let separator = scan_argument_separator(bytes, cursor);
        let argument_start = separator.end;
        let Some(&byte) = bytes.get(argument_start) else {
            if separator.continuation_start.is_some() {
                // `trailingLineContinuation` is a separate valid grammar
                // production. It consumes the continuation and any optional
                // indentation without fabricating an argument.
                end = argument_start;
                cursor = argument_start;
            }
            break;
        };
        let malformed_continuation = || {
            ParseError::new(
                "E2004",
                "line continuation must be followed by an argument or chain",
                ByteSpan::new(
                    separator
                        .continuation_start
                        .unwrap_or(argument_start)
                        .min(source.len()),
                    separator
                        .continuation_start
                        .unwrap_or(argument_start)
                        .saturating_add(1)
                        .min(source.len()),
                ),
            )
        };
        if byte == b'{' {
            let arg = parse_braced(source, argument_start)?;
            end = arg.span.end;
            cursor = arg.span.end;
            arguments.push(CallArgument::Positional(arg));
            continue;
        }

        let arg_name_start = argument_start;
        let Some(arg_name_end) = scan_identifier(bytes, arg_name_start) else {
            if separator.continuation_start.is_some()
                && !is_chain_separator_at(bytes, argument_start)
            {
                return Err(malformed_continuation());
            }
            break;
        };
        if bytes.get(arg_name_end) != Some(&b':') {
            if separator.continuation_start.is_some()
                && !is_chain_separator_at(bytes, argument_start)
            {
                return Err(malformed_continuation());
            }
            break;
        }
        let open = arg_name_end + 1;
        if bytes.get(open) != Some(&b'{') {
            // The named-argument parser is one optional argument in the
            // surrounding repeat. If its braced-value boundary does not
            // match, leave the entire candidate for the caller's
            // remainder path instead of fabricating a diagnostic, unless
            // a continuation explicitly promised another argument.
            if separator.continuation_start.is_some() {
                return Err(malformed_continuation());
            }
            break;
        }
        let value = parse_braced(source, open)?;
        end = value.span.end;
        arguments.push(CallArgument::Named(NamedArg {
            name: source[arg_name_start..arg_name_end].to_string(),
            name_span: ByteSpan::new(arg_name_start, arg_name_end),
            value,
            span: ByteSpan::new(arg_name_start, end),
        }));
        cursor = end;
    }

    Ok(ParsedArguments {
        arguments,
        end,
        cursor,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ArgumentSeparator {
    end: usize,
    continuation_start: Option<usize>,
}

/// Consume the bounded equivalent of Quarkdown's reusable `argumentSeparator`:
/// horizontal whitespace, followed by zero or more line continuations and
/// their optional indentation. The caller decides whether the separator is
/// part of an accepted argument, chain, or trailing continuation.
fn scan_argument_separator(bytes: &[u8], start: usize) -> ArgumentSeparator {
    let mut cursor = start;
    let mut continuation_start = None;
    loop {
        cursor = skip_horizontal(bytes, cursor);
        let Some(after_continuation) = consume_continuation(bytes, cursor) else {
            break;
        };
        continuation_start.get_or_insert(cursor);
        cursor = skip_line_indentation(bytes, after_continuation);
    }
    ArgumentSeparator {
        end: cursor,
        continuation_start,
    }
}

fn is_chain_separator_at(bytes: &[u8], cursor: usize) -> bool {
    cursor
        .checked_add(2)
        .is_some_and(|limit| bytes.get(cursor..limit) == Some(b"::"))
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
                b'{' if !is_escaped_delimiter(bytes, cursor) => depth += 1,
                b'}' if !is_escaped_delimiter(bytes, cursor) => {
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
    let raw_source = source.get(span.start..span.end)?;
    let raw = raw_source.trim();
    let leading = raw_source
        .len()
        .saturating_sub(raw_source.trim_start().len());
    let trailing = raw_source
        .len()
        .saturating_sub(leading.saturating_add(raw.len()));
    let range_span = ByteSpan::new(
        span.start.checked_add(leading)?,
        span.end.checked_sub(trailing)?,
    );
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
    // Numeric dot identifiers such as `.1` and `.01` are implicit lambda
    // references even when nested in a braced value. Keep them as content so
    // the frontend can preserve the call node and the evaluator can resolve
    // them semantically; do not let the generic floating-point parser
    // classify `.1` as `0.1`.
    if !is_implicit_positional_reference_token(raw) {
        if let Some(range) = parse_range_literal(raw, range_span) {
            return Some(Value::Range(range));
        }
        if let Ok(number) = raw.parse::<f64>() {
            return Some(Value::Number(number));
        }
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
    if is_dimension_token(raw) {
        return Some(Value::Identifier(raw.to_string()));
    }
    None
}

fn is_dimension_token(raw: &str) -> bool {
    let bytes = raw.as_bytes();
    let mut index = usize::from(bytes.first() == Some(&b'-'));
    let digit_start = index;
    while bytes.get(index).is_some_and(u8::is_ascii_digit) {
        index += 1;
    }
    if index == digit_start {
        return false;
    }
    if bytes.get(index) == Some(&b'.') {
        index += 1;
        while bytes.get(index).is_some_and(u8::is_ascii_digit) {
            index += 1;
        }
    }
    index < bytes.len()
        && raw[index..]
            .chars()
            .all(|character| character.is_ascii_alphabetic() || character == '%')
}

fn parse_range_literal(raw: &str, span: ByteSpan) -> Option<RangeValue> {
    let mut parts = raw.split("..");
    let start = parse_range_endpoint(parts.next()?)?;
    let end = parse_range_endpoint(parts.next()?)?;
    if parts.next().is_some() {
        return None;
    }
    Some(RangeValue { start, end, span })
}

fn parse_range_endpoint(raw: &str) -> Option<Option<u64>> {
    if raw.is_empty() {
        return Some(None);
    }
    if !raw.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    Some(Some(raw.parse().ok()?))
}

fn is_implicit_positional_reference_token(raw: &str) -> bool {
    let bytes = raw.as_bytes();
    bytes.len() >= 2
        && bytes[0] == b'.'
        && bytes[1].is_ascii_digit()
        && bytes[2..].iter().all(u8::is_ascii_digit)
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

/// Scan one call-grammar identifier using the pinned v2.5.1 alternatives:
/// `[A-Za-z][A-Za-z0-9]*|[0-9]+`.
fn scan_identifier(bytes: &[u8], start: usize) -> Option<usize> {
    let first = *bytes.get(start)?;
    let numeric = if first.is_ascii_alphabetic() {
        false
    } else if first.is_ascii_digit() {
        true
    } else {
        return None;
    };
    let mut end = start + 1;
    while bytes.get(end).is_some_and(|byte| {
        if numeric {
            byte.is_ascii_digit()
        } else {
            byte.is_ascii_alphanumeric()
        }
    }) {
        end += 1;
    }
    Some(end)
}

/// Returns whether `name` satisfies Arkst's declaration-name grammar:
/// `[A-Za-z_][A-Za-z0-9_-]*`.
///
/// This is the single public validation helper for consumers that need to
/// validate a declared function or variable name without parsing a complete
/// call. The call lexer uses the private pinned identifier scanner instead;
/// declaration names are a semantic contract owned by the evaluator.
pub fn is_valid_normal_call_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes.next().is_some_and(is_name_start) && bytes.all(is_name_char)
}

fn is_word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || !byte.is_ascii()
}

fn is_name_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

fn is_name_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')
}

fn valid_boundary(bytes: &[u8], start: usize) -> bool {
    start == 0
        || (!matches!(bytes[start - 1], b'\\' | b'.') && !bytes[start - 1].is_ascii_alphanumeric())
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
        for source in ["", "hello world", ".", ". note", "...ellipsis"] {
            assert!(matches!(parse_call(source), Ok(None)), "{source:?}");
        }
    }

    #[test]
    fn parses_implicit_positional_references_and_boundaries() {
        for (source, expected) in [
            (".0", "0"),
            (".01", "01"),
            (".1", "1"),
            (".2", "2"),
            (".12", "12"),
        ] {
            let (call, end) = parse_call(source).unwrap().unwrap();
            assert_eq!(call.name, expected);
            assert_eq!(call.name_span, ByteSpan::new(0, source.len()));
            assert_eq!(call.span, ByteSpan::new(0, source.len()));
            assert!(call.positional_args().is_empty());
            assert!(call.named_args().is_empty());
            assert_eq!(end, source.len());
        }

        for (source, expected_name, expected_end) in
            [(".1abc", "1", 2), (".12foo", "12", 3), (".1e5", "1", 2)]
        {
            let (call, end) = parse_call(source).unwrap().unwrap();
            assert_eq!(call.name, expected_name, "{source:?}");
            assert_eq!(call.span, ByteSpan::new(0, expected_end), "{source:?}");
            assert_eq!(end, expected_end, "{source:?}");
        }
        for source in [".1-1", ".1.", ".1)", ".1!", ".1한"] {
            let (call, end) = parse_call(source).unwrap().unwrap();
            assert_eq!(call.name, "1");
            assert_eq!(end, 2);
        }
    }

    #[test]
    fn numeric_identifiers_share_the_argument_grammar() {
        let (call, end) = parse_call(".1 {item}").unwrap().unwrap();
        assert_eq!(call.name, "1");
        assert_eq!(call.positional_args().len(), 1);
        assert!(call.named_args().is_empty());
        assert_eq!(call.span, ByteSpan::new(0, 9));
        assert_eq!(end, 9);
    }

    #[test]
    fn braced_implicit_reference_is_not_classified_as_a_decimal() {
        let (call, end) = parse_call(".multiply {.1} {3}").unwrap().unwrap();
        assert_eq!(end, ".multiply {.1} {3}".len());
        assert!(matches!(
            call.positional_args().first().map(|argument| &argument.content),
            Some(ArgContent::Content(span)) if *span == ByteSpan::new(11, 13)
        ));
        assert!(matches!(
            call.positional_args().get(1).map(|argument| &argument.content),
            Some(ArgContent::Scalar(Value::Number(value))) if *value == 3.0
        ));

        let (call, _) = parse_call(".multiply {.01}").unwrap().unwrap();
        assert!(matches!(
            call.positional_args().first().map(|argument| &argument.content),
            Some(ArgContent::Content(span)) if *span == ByteSpan::new(11, 14)
        ));
    }

    #[test]
    fn tight_word_adjacency_and_symbol_boundaries_are_explicit() {
        for source in [".note {x}suffix", ".note {x}B", ".note {x}한"] {
            let (call, end) = parse_call(source).unwrap().unwrap();
            assert_eq!(call.name, "note", "{source:?}");
            assert_eq!(call.span, ByteSpan::new(0, 9), "{source:?}");
            assert_eq!(end, 9, "{source:?}");
        }
        for (source, start) in [
            ("A.note {x}", 1),
            ("word.note {x}", 4),
            ("..note {x}", 1),
            (r"\.note {x}", 1),
        ] {
            assert!(
                matches!(parse_inline_call(source, start), Ok(None)),
                "{source:?}"
            );
        }

        let (call, end) = parse_inline_call("한.note {x}", 3).unwrap().unwrap();
        assert_eq!(call.name, "note");
        assert_eq!(call.span, ByteSpan::new(3, 12));
        assert_eq!(end, 12);

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
        assert_eq!(call.positional_args().len(), 1);
        assert_eq!(call.named_args()[0].name, "width");
        assert_eq!(call.name_span, ByteSpan::new(0, 6));
        assert_eq!(call.span, ByteSpan::new(0, source.len()));
        assert_eq!(parse_call(".note").unwrap().unwrap().0.name, "note");
        assert_eq!(parse_call(".my123").unwrap().unwrap().0.name, "my123");
    }

    #[test]
    fn call_and_named_identifiers_share_the_pinned_scanner() {
        for (source, expected_name, expected_span) in [
            (".alpha123", "alpha123", ByteSpan::new(0, 9)),
            (".0", "0", ByteSpan::new(0, 2)),
            (".01", "01", ByteSpan::new(0, 3)),
        ] {
            let (call, end) = parse_call(source).unwrap().unwrap();
            assert_eq!(call.name, expected_name, "{source:?}");
            assert_eq!(call.name_span, expected_span, "{source:?}");
            assert_eq!(call.span, expected_span, "{source:?}");
            assert_eq!(end, expected_span.end, "{source:?}");
        }

        for source in [".foo _:{x}", ".foo -:{x}", ".foo name-1:{x}"] {
            let (call, end) = parse_call(source).unwrap().unwrap();
            assert_eq!(call.name, "foo", "{source:?}");
            assert!(call.named_args().is_empty(), "{source:?}");
            assert_eq!(call.span, ByteSpan::new(0, 4), "{source:?}");
            assert_eq!(&source[end..], &source[4..], "{source:?}");
        }

        for (source, expected_name, expected_name_span) in [
            (".foo 1:{x}", "1", ByteSpan::new(5, 6)),
            (".foo 10:{x}", "10", ByteSpan::new(5, 7)),
        ] {
            let call = parse_call(source).unwrap().unwrap().0;
            assert_eq!(call.named_args().len(), 1, "{source:?}");
            assert_eq!(call.named_args()[0].name, expected_name, "{source:?}");
            assert_eq!(
                call.named_args()[0].name_span,
                expected_name_span,
                "{source:?}"
            );
        }

        for (source, expected_name, expected_end) in
            [(".my_call {x}", "my", 3), (".my-call {x}", "my", 3)]
        {
            let (call, end) = parse_call(source).unwrap().unwrap();
            assert_eq!(call.name, expected_name, "{source:?}");
            assert_eq!(call.span, ByteSpan::new(0, expected_end), "{source:?}");
            assert_eq!(end, expected_end, "{source:?}");
            assert_eq!(&source[end..], &source[expected_end..], "{source:?}");
        }
    }

    #[test]
    fn parses_positional_named_and_mixed_arguments() {
        let call = parse_call(".range {1} {10}").unwrap().unwrap().0;
        assert_eq!(call.positional_args().len(), 2);
        assert_eq!(call.named_args().len(), 0);
        assert_eq!(scalar(call.positional_args()[0]), Value::Number(1.0));
        assert_eq!(scalar(call.positional_args()[1]), Value::Number(10.0));

        let call = parse_call(".panel width:{320} align:{center}")
            .unwrap()
            .unwrap()
            .0;
        assert!(call.positional_args().is_empty());
        assert_eq!(call.named_args().len(), 2);
        assert_eq!(call.named_args()[0].name, "width");
        assert_eq!(call.named_args()[0].name_span, ByteSpan::new(7, 12));
        assert_eq!(call.named_args()[0].span, ByteSpan::new(7, 18));
        assert_eq!(scalar(&call.named_args()[0].value), Value::Number(320.0));
        assert_eq!(
            scalar(&call.named_args()[1].value),
            Value::Identifier("center".into())
        );

        let call = parse_call(".panel {Introduction} width:{320}")
            .unwrap()
            .unwrap()
            .0;
        assert_eq!(call.positional_args().len(), 1);
        assert_eq!(call.named_args().len(), 1);
        assert_eq!(
            scalar(call.positional_args()[0]),
            Value::Identifier("Introduction".into())
        );

        for source in [".range {1}    {2}", ".range{1}{ 14}"] {
            let call = parse_call(source).unwrap().unwrap().0;
            assert_eq!(call.positional_args().len(), 2, "{source:?}");
        }
        let (call, end) = parse_call(".note and more text").unwrap().unwrap();
        assert_eq!(call.name, "note");
        assert_eq!(end, 5);
    }

    #[test]
    fn parses_nested_content_and_scalar_classification() {
        let nested = parse_call(".outer {.nested {x}}").unwrap().unwrap().0;
        assert!(matches!(
            nested.positional_args()[0].content,
            ArgContent::Content(_)
        ));

        let cases = [
            (".foo {hello}", Value::Identifier("hello".into())),
            (
                ".foo {\"hello world\"}",
                Value::String("hello world".into()),
            ),
            (
                ".foo {\"  hello world  \"}",
                Value::String("  hello world  ".into()),
            ),
            (".foo {show_code}", Value::Identifier("show_code".into())),
            (".foo {0.5}", Value::Number(0.5)),
            (".foo {-1}", Value::Number(-1.0)),
            (".foo {true}", Value::Boolean(true)),
            (".foo {false}", Value::Boolean(false)),
        ];
        for (source, expected) in cases {
            let call = parse_call(source).unwrap().unwrap().0;
            assert_eq!(scalar(call.positional_args()[0]), expected, "{source:?}");
        }
        let call = parse_call(".fn {\"hello \\\"world\\\"\"}")
            .unwrap()
            .unwrap()
            .0;
        assert_eq!(
            scalar(call.positional_args()[0]),
            Value::String("hello \"world\"".into())
        );
        for source in [
            ".foo {hello world}",
            ".foo {**bold**}",
            ".foo {.nested {x}}",
        ] {
            let call = parse_call(source).unwrap().unwrap().0;
            assert!(
                matches!(call.positional_args()[0].content, ArgContent::Content(_)),
                "{source:?}"
            );
        }
    }

    #[test]
    fn escaped_brace_delimiters_remain_source_backed_content() {
        let cases = [
            (
                r".foo {a \} b}",
                ByteSpan::new(5, 13),
                ByteSpan::new(6, 12),
                r"a \} b",
            ),
            (
                r".foo {a \{ b}",
                ByteSpan::new(5, 13),
                ByteSpan::new(6, 12),
                r"a \{ b",
            ),
            (
                r".foo {a \{ nested \} b}",
                ByteSpan::new(5, 23),
                ByteSpan::new(6, 22),
                r"a \{ nested \} b",
            ),
            (
                ".foo {한글 \\} text}",
                ByteSpan::new(5, 21),
                ByteSpan::new(6, 20),
                "한글 \\} text",
            ),
            (
                ".foo {a \\}\r\nb}",
                ByteSpan::new(5, 14),
                ByteSpan::new(6, 13),
                "a \\}\r\nb",
            ),
        ];

        for (source, argument_span, content_span, expected_content) in cases {
            let (call, end) = parse_call(source)
                .unwrap_or_else(|error| panic!("unexpected {error:?} for {source:?}"))
                .expect("expected complete call");
            assert_eq!(end, source.len(), "{source:?}");
            assert_eq!(call.span, ByteSpan::new(0, source.len()), "{source:?}");
            assert_eq!(call.name_span, ByteSpan::new(0, 4), "{source:?}");
            assert_eq!(call.arguments.len(), 1, "{source:?}");

            let CallArgument::Positional(argument) = &call.arguments[0] else {
                panic!("expected positional argument for {source:?}")
            };
            assert_eq!(argument.span, argument_span, "{source:?}");
            let ArgContent::Content(content) = &argument.content else {
                panic!("expected source-backed content for {source:?}")
            };
            assert_eq!(*content, content_span, "{source:?}");
            assert_eq!(&source[call.span.start..call.span.end], source);
            assert_eq!(
                &source[argument_span.start..argument_span.end],
                &source[5..]
            );
            assert_eq!(&source[content.start..content.end], expected_content);
            assert!(call.span.is_valid_for(source));
            assert!(argument_span.is_valid_for(source));
            assert!(content.is_valid_for(source));
            assert!(source.is_char_boundary(content.start));
            assert!(source.is_char_boundary(content.end));
        }

        let crlf = ".foo {a \\}\r\nb}";
        assert!(crlf.as_bytes().windows(2).any(|pair| pair == b"\r\n"));
    }

    #[test]
    fn needs_more_input_uses_escaped_brace_delimiter_depth() {
        assert!(!needs_more_input(r".foo {a \{ b}"));
        assert!(!needs_more_input(r".foo {a \} b}"));
        assert!(!needs_more_input(r#".foo {"{"}"#));
        assert!(needs_more_input(r".foo {a { b}"));
    }

    #[test]
    fn parses_typed_ranges_without_confusing_numbers_or_references() {
        for (source, start, end) in [
            (".foo {2..4}", Some(2), Some(4)),
            (".foo {0..0}", Some(0), Some(0)),
            (".foo {2..}", Some(2), None),
            (".foo {..4}", None, Some(4)),
            (".foo {..}", None, None),
        ] {
            let call = parse_call(source).unwrap().unwrap().0;
            assert_eq!(
                scalar(call.positional_args()[0]),
                Value::Range(RangeValue {
                    start,
                    end,
                    span: ByteSpan::new(6, source.len() - 1),
                }),
                "{source:?}"
            );
        }

        for source in [
            ".foo {1.5}",
            ".foo {.1}",
            ".foo {abc}",
            ".foo {1...2}",
            ".foo {1..2..3}",
            ".foo {a..b}",
            ".foo {-1..3}",
        ] {
            let call = parse_call(source).unwrap().unwrap().0;
            assert!(
                !matches!(
                    call.positional_args()[0].content,
                    ArgContent::Scalar(Value::Range(_))
                ),
                "{source:?}"
            );
        }
    }

    #[test]
    fn range_span_preserves_utf8_and_crlf_surroundings() {
        let source = "한글\r\n.foo { 2..4 }\r\n";
        let start = source.find(".foo").expect("directive start");
        let (call, _) = parse_inline_call(source, start).unwrap().unwrap();
        let Value::Range(range) = scalar(call.positional_args()[0]) else {
            panic!("expected typed range")
        };
        assert_eq!(range.span, ByteSpan::new(start + 7, start + 11));
        assert_eq!(&source[range.span.start..range.span.end], "2..4");
    }

    #[test]
    fn parses_multiline_nested_arguments_with_original_spans() {
        let source = ".divide {\n  .cos {.pi}\n} by:{\n  .sum {2} {1}\n}";
        let (call, end) = parse_call(source).unwrap().unwrap();
        assert_eq!(end, source.len());
        assert_eq!(call.positional_args().len(), 1);
        assert_eq!(call.named_args().len(), 1);
        assert_eq!(call.span, ByteSpan::new(0, source.len()));
        let ArgContent::Content(content) = call.positional_args()[0].content else {
            panic!("expected multiline content argument")
        };
        assert_eq!(&source[content.start..content.end], "\n  .cos {.pi}\n");
        assert_eq!(
            &source[call.named_args()[0].value.span.start..call.named_args()[0].value.span.end],
            "{\n  .sum {2} {1}\n}"
        );
        for span in [
            call.span,
            call.name_span,
            content,
            call.named_args()[0].span,
        ] {
            assert!(span.is_valid_for(source));
        }

        let crlf = concat!(".call {\r\n  한글\r\n} \\", "\r\n  next:{값}");
        let (call, end) = parse_call(crlf).unwrap().unwrap();
        assert_eq!(end, crlf.len());
        assert_eq!(call.named_args().len(), 1);
        assert!(call.span.is_valid_for(crlf));
        let ArgContent::Content(content) = call.positional_args()[0].content else {
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
            assert_eq!(call.positional_args().len(), positional, "{source:?}");
            assert_eq!(call.named_args().len(), named, "{source:?}");
            assert!(call.span.is_valid_for(source), "{source:?}");
        }
    }

    #[test]
    fn parses_argument_separators_before_first_argument_and_at_trailing_edge() {
        let cases = [
            (".foo \\\n{x}", ByteSpan::new(7, 10), None),
            (
                ".foo \\\nname:{x}",
                ByteSpan::new(7, 15),
                Some(ByteSpan::new(7, 11)),
            ),
            (".foo {x} \\\n", ByteSpan::new(5, 8), None),
        ];

        for (source, argument_span, named_name_span) in cases {
            let (call, end) = parse_call(source)
                .unwrap_or_else(|error| panic!("unexpected {error:?} for {source:?}"))
                .expect("expected complete call");
            assert_eq!(end, source.len(), "{source:?}");
            assert_eq!(call.span, ByteSpan::new(0, source.len()), "{source:?}");
            assert_eq!(call.head_span, ByteSpan::new(0, source.len()), "{source:?}");
            assert_eq!(call.arguments.len(), 1, "{source:?}");
            let argument = &call.arguments[0];
            let argument_span_actual = match argument {
                CallArgument::Positional(argument) => argument.span,
                CallArgument::Named(argument) => argument.span,
            };
            assert_eq!(argument_span_actual, argument_span, "{source:?}");
            if let Some(name_span) = named_name_span {
                let CallArgument::Named(named) = argument else {
                    panic!("expected named argument for {source:?}")
                };
                assert_eq!(named.name_span, name_span, "{source:?}");
                assert_eq!(named.value.span, ByteSpan::new(12, 15), "{source:?}");
                assert_eq!(
                    &source[name_span.start..name_span.end],
                    "name",
                    "{source:?}"
                );
                assert_eq!(&source[named.value.span.start..named.value.span.end], "{x}");
            } else {
                assert_eq!(&source[argument_span.start..argument_span.end], "{x}");
            }
            assert!(call.span.is_valid_for(source));
            assert!(argument_span.is_valid_for(source));
        }
    }

    #[test]
    fn parses_separator_before_chain_without_changing_segment_spans() {
        let cases = [
            (".a {x} ::b {y}", vec![(9, 14)]),
            (".a {x} \\\n::b {y}", vec![(11, 16)]),
            (".a {x} ::b {y} \\\n::c {z}", vec![(9, 14), (19, 24)]),
        ];

        for (source, expected_segments) in cases {
            let (call, end) = parse_call(source)
                .unwrap_or_else(|error| panic!("unexpected {error:?} for {source:?}"))
                .expect("expected complete chain");
            assert_eq!(end, source.len(), "{source:?}");
            assert_eq!(call.span, ByteSpan::new(0, source.len()), "{source:?}");
            assert_eq!(call.head_span, ByteSpan::new(0, 6), "{source:?}");
            assert_eq!(call.chain.len(), expected_segments.len(), "{source:?}");
            for (segment, (start, end)) in call.chain.iter().zip(expected_segments) {
                assert_eq!(segment.span, ByteSpan::new(start, end), "{source:?}");
                assert!(source.is_char_boundary(segment.span.start));
                assert!(source.is_char_boundary(segment.span.end));
            }
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
        assert_eq!(call.positional_args().len(), 1);
        assert_eq!(call.chain[0].positional_args().len(), 1);
    }

    #[test]
    fn rejects_malformed_chains_deterministically() {
        for source in [".a::", ".a:::b", ".a:: {x}", ".foo {x}::"] {
            let error = parse_call(source).unwrap_err();
            assert_eq!(error.code, "E2004", "{source:?}");
            assert!(error.span.is_valid_for(source), "{source:?}");
        }
        let (call, end) = parse_call(".a::1abc").unwrap().unwrap();
        assert_eq!(call.chain[0].name, "1");
        assert_eq!(call.span, ByteSpan::new(0, 5));
        assert_eq!(end, 5);
        let source = concat!(".call {a} \\", "\n\nnext");
        {
            let error = parse_call(source).unwrap_err();
            assert_eq!(error.code, "E2004", "{source:?}");
            assert!(error.span.is_valid_for(source), "{source:?}");
        }
    }

    #[test]
    fn trailing_continuation_accepts_optional_indentation_without_extra_argument() {
        for source in [
            concat!(".foo {x} \\", "\n"),
            concat!(".foo {x} \\", "\r\n\t  "),
        ] {
            let (call, end) = parse_call(source).unwrap().unwrap();
            assert_eq!(end, source.len(), "{source:?}");
            assert_eq!(call.span, ByteSpan::new(0, source.len()), "{source:?}");
            assert_eq!(call.arguments.len(), 1, "{source:?}");
            assert_eq!(
                call.positional_args()[0].span,
                ByteSpan::new(5, 8),
                "{source:?}"
            );
            assert!(call.span.is_valid_for(source), "{source:?}");
            assert!(has_trailing_continuation(source), "{source:?}");
        }
        assert!(!has_trailing_continuation(".foo {x} \\ \n"));
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

        let source = concat!("{.foo \\", "\n{x}}");
        let (call, end) = parse_tight_call(source, 0).unwrap().unwrap();
        assert_eq!(end, source.len());
        assert_eq!(call.inner_span, ByteSpan::new(1, source.len() - 1));
        assert_eq!(call.wrapper_span, Some(ByteSpan::new(0, source.len())));
        assert_eq!(call.positional_args()[0].span, ByteSpan::new(8, 11));
        assert_eq!(&source[call.span.start..call.span.end], source);
    }

    #[test]
    fn newline_and_crlf_terminate_calls() {
        for source in [".foo\n{bar}", ".foo {a}\n{b}", ".foo {a}\r\n{b}"] {
            let (call, end) = parse_call(source).unwrap().unwrap();
            assert_eq!(call.name, "foo");
            assert_eq!(
                call.positional_args().len(),
                if source.contains("{a}") { 1 } else { 0 }
            );
            assert_eq!(end, if source.starts_with(".foo ") { 8 } else { 4 });
        }
    }

    #[test]
    fn rejects_malformed_arguments_and_preserves_unmatched_named_candidates() {
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
        for source in [".foo key:", ".foo key: value", ".foo key: {value}"] {
            let (call, end) = parse_call(source).unwrap().unwrap();
            assert_eq!(call.name, "foo", "{source:?}");
            assert!(call.named_args().is_empty(), "{source:?}");
            assert_eq!(call.span, ByteSpan::new(0, 4), "{source:?}");
            assert_eq!(end, 4, "{source:?}");
            assert_eq!(&source[end..], &source[4..], "{source:?}");
        }
        let (call, end) = parse_call(".foo width:{\"x\"} {y}").unwrap().unwrap();
        assert_eq!(end, ".foo width:{\"x\"} {y}".len());
        assert!(matches!(
            call.arguments.as_slice(),
            [
                CallArgument::Named(NamedArg { name, .. }),
                CallArgument::Positional(Arg { .. }),
            ] if name == "width"
        ));
    }

    #[test]
    fn public_declaration_name_validation_matches_declaration_grammar() {
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

    #[test]
    fn parses_marked_inline_lambdas_without_rewriting_source() {
        let source = "prefix @lambda key value?: .value suffix";
        let start = source.find("@lambda").unwrap();
        let lambda = parse_inline_lambda(source, ByteSpan::new(start, source.len()))
            .unwrap()
            .unwrap();
        assert!(!lambda.implicit);
        assert_eq!(lambda.parameters.len(), 2);
        assert_eq!(lambda.parameters[0].name, "key");
        assert!(lambda.parameters[1].optional);
        assert_eq!(&source[lambda.body.start..lambda.body.end], ".value suffix");
        assert_eq!(
            &source[lambda.span.start..lambda.span.end],
            "@lambda key value?: .value suffix"
        );
    }

    #[test]
    fn parses_marked_inline_implicit_lambdas() {
        let source = "@lambda .1";
        let lambda = parse_inline_lambda(source, ByteSpan::new(0, source.len()))
            .unwrap()
            .unwrap();
        assert!(lambda.implicit);
        assert!(lambda.parameters.is_empty());
        assert_eq!(&source[lambda.body.start..lambda.body.end], ".1");
    }

    fn scalar(arg: &Arg) -> Value {
        match &arg.content {
            ArgContent::Scalar(value) => value.clone(),
            other => panic!("expected scalar, got {other:?}"),
        }
    }
}
