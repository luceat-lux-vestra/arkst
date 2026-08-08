//! Minimal CommonMark-compatible Markdown parser.
//!
//! Produces the Scribium AST with byte-level source spans on every node.
//! Supported constructs (M1 subset):
//!
//! - ATX headings (`#` through `######`)
//! - Paragraphs with soft/hard line breaks
//! - Emphasis (`*text*`, `_text_`) and strong (`**text**`, `__text__`)
//! - Unordered lists (`- `, `* `, `+ `) with nested lists and code blocks
//! - Fenced code blocks (triple backtick, optional language)
//! - Thematic breaks (`---`, `***`, `___`)
//!
//! Delimiter runs of three or more identical characters (`***x***`) are
//! treated as literal text. Setext headings, links, images, blockquotes,
//! code spans, and ordered lists are not part of the M1 subset.

use super::ast::{Block, Document, FrontMatter, Inline, ListItem, Value};
use crate::source::ByteSpan;

/// Maximum block-nesting depth before a parse is flattened to paragraphs.
///
/// Guards against stack overflow on pathological input such as thousands of
/// nested list markers.
const MAX_BLOCK_DEPTH: usize = 64;

/// Maximum inline-nesting depth before delimiters are treated as literal text.
const MAX_INLINE_DEPTH: usize = 64;
/// Parse flat key-value front matter at document start.
///
/// Front matter is a `---`-delimited block of `key: value` lines only. It is
/// not full YAML: nested objects, arrays, block strings, and other YAML
/// features are not supported. The delimiters must start at column 0, and
/// every non-empty metadata line must start at column 0 — leading indentation
/// marks nested structure and rejects the whole block. Keys and values are
/// split on the first colon; duplicate keys use last-wins semantics.
///
/// Returns `(front_matter, lines_consumed)`. If no valid front matter is found
/// at the start, returns `(None, 0)` and the caller should start parsing from line 0.
fn parse_front_matter(_source: &str, lines: &[SourceLine<'_>]) -> (Option<FrontMatter>, usize) {
    if lines.is_empty() {
        return (None, 0);
    }

    // Check if first line is opening delimiter `---`
    // Use raw to reject indented delimiters
    let first = &lines[0];
    if first.raw != "---" {
        return (None, 0);
    }

    // Find closing delimiter
    let mut close_idx = None;
    for (i, line) in lines.iter().enumerate().skip(1) {
        if line.raw == "---" {
            close_idx = Some(i);
            break;
        }
    }

    let close_idx = match close_idx {
        Some(idx) => idx,
        None => {
            // Unclosed front matter - treat as no front matter
            return (None, 0);
        }
    };

    // Parse fields between delimiters, checking for malformed lines
    let mut fields = Vec::new();
    for line in &lines[1..close_idx] {
        let text = line.text;
        if text.is_empty() {
            continue; // Skip empty lines in front matter
        }
        // Metadata lines must start at column 0: leading indentation marks
        // nested (YAML-style) structure, which is not flattened. Reject the
        // whole block so it stays intact as regular Markdown.
        if line.raw != line.text {
            return (None, 0);
        }
        if let Some(colon_pos) = text.find(':') {
            let key = text[..colon_pos].trim();
            let value = text[colon_pos + 1..].trim();
            if key.is_empty() {
                // Empty key - malformed, treat entire block as invalid
                return (None, 0);
            }
            // last-wins: remove existing entry with same key
            fields.retain(|(k, _)| k != key);
            fields.push((key.to_string(), value.to_string()));
        } else {
            // Line without colon - malformed, treat entire block as invalid
            return (None, 0);
        }
    }

    let span = ByteSpan::new(first.raw_start, lines[close_idx].end);
    (Some(FrontMatter { fields, span }), close_idx + 1)
}

/// A parse-level diagnostic produced for malformed but recoverable input
/// (never fatal: parsing continues and the offending text is treated as
/// ordinary content).
#[derive(Debug, Clone, PartialEq)]
pub struct ParserDiagnostic {
    /// Stable error code (e.g. `E2003`).
    pub code: &'static str,
    /// Human-readable description.
    pub message: String,
    /// Source span of the offending construct.
    pub span: ByteSpan,
}

/// Parse a Markdown source string into a `Document`.
///
/// Never panics on malformed input; unclosed constructs are parsed
/// deterministically up to the end of the source.
pub fn parse(source: &str) -> Document {
    parse_with_diagnostics(source).document
}

/// Parse a Markdown source string, returning the document together with the
/// structured diagnostics gathered for malformed-but-recoverable constructs.
pub fn parse_with_diagnostics(source: &str) -> ParseOutput {
    let lines = split_lines(source);
    let mut diagnostics: Vec<ParserDiagnostic> = Vec::new();

    // Parse front matter if present at document start
    let (front_matter, front_matter_lines) = parse_front_matter(source, &lines);
    let mut cursor = front_matter_lines;

    let nodes = parse_blocks(source, &lines, &mut cursor, 0, &mut diagnostics);
    let line_count = source.as_bytes().iter().filter(|&&b| b == b'\n').count() + 1;
    ParseOutput {
        document: Document {
            nodes,
            front_matter,
            line_count,
        },
        diagnostics,
    }
}

/// The result of `parse_with_diagnostics`.
#[derive(Debug, Clone)]
pub struct ParseOutput {
    pub document: Document,
    /// Diagnostics for malformed constructs; the document still parses the
    /// offending text as ordinary content.
    pub diagnostics: Vec<ParserDiagnostic>,
}

/// A logical source line with byte offsets into the original source.
///
/// `raw` is the full line text without its terminator. `text` is `raw` with
/// all leading whitespace removed, positioned at `text_start`.
struct SourceLine<'a> {
    /// Full line text, excluding the line terminator (LF/CRLF normalized).
    raw: &'a str,
    /// Leading-whitespace-stripped line text.
    text: &'a str,
    /// Absolute byte offset of `raw` in the source.
    raw_start: usize,
    /// Absolute byte offset of `text` in the source.
    text_start: usize,
    /// Absolute byte offset of the line terminator (or EOF for the last line).
    term: usize,
    /// Absolute byte offset just past the line terminator (or EOF).
    end: usize,
}

impl SourceLine<'_> {
    /// Byte offset of the line's final content byte.
    fn content_end(&self) -> usize {
        self.raw_start + self.raw.len()
    }

    /// Number of leading whitespace columns in the raw line.
    fn indent(&self) -> usize {
        self.text_start - self.raw_start
    }

    fn is_blank(&self) -> bool {
        self.text.is_empty()
    }

    /// Whether the line starts a new block rather than continuing a paragraph.
    fn starts_block(&self) -> bool {
        is_heading_text(self.text).is_some()
            || fence_length(self.text).is_some()
            || is_thematic_break(self.text)
            || is_list_marker(self.text).is_some()
            || is_directive_line(self.text)
    }
}

/// Split a source string into lines with absolute byte offsets.
fn split_lines(source: &str) -> Vec<SourceLine<'_>> {
    let bytes = source.as_bytes();
    let mut lines = Vec::new();
    let mut line_start = 0;
    let mut pos = 0;
    let len = bytes.len();
    while pos <= len {
        if pos == len || bytes[pos] == b'\n' {
            let mut raw_end = pos;
            if raw_end > line_start && bytes[raw_end - 1] == b'\r' {
                raw_end -= 1;
            }
            let raw = &source[line_start..raw_end];
            let ws = raw
                .bytes()
                .take_while(|b| *b == b' ' || *b == b'\t')
                .count();
            lines.push(SourceLine {
                raw,
                text: &raw[ws..],
                raw_start: line_start,
                text_start: line_start + ws,
                term: pos,
                end: if pos == len { pos } else { pos + 1 },
            });
            line_start = pos + 1;
        }
        pos += 1;
    }
    lines
}

/// Parse a sequence of lines into block nodes.
///
/// `cursor` tracks the current line position within `lines`. `depth` bounds
/// recursion for nested lists; beyond `MAX_BLOCK_DEPTH` lines are flattened
/// into paragraphs.
fn parse_blocks(
    source: &str,
    lines: &[SourceLine<'_>],
    cursor: &mut usize,
    depth: usize,
    diagnostics: &mut Vec<ParserDiagnostic>,
) -> Vec<Block> {
    let mut blocks = Vec::new();
    while *cursor < lines.len() {
        let line = &lines[*cursor];
        if line.is_blank() {
            *cursor += 1;
            continue;
        }
        if let Some(level) = is_heading_text(line.text) {
            blocks.push(parse_heading(source, line, level, diagnostics));
            *cursor += 1;
            continue;
        }
        if let Some(fence_len) = fence_length(line.text) {
            blocks.push(parse_code_block(lines, cursor, fence_len));
            continue;
        }
        if is_thematic_break(line.text) {
            blocks.push(Block::ThematicBreak {
                span: ByteSpan::new(line.text_start, line.end),
            });
            *cursor += 1;
            continue;
        }
        if let Some(marker) = is_list_marker(line.text) {
            if depth < MAX_BLOCK_DEPTH {
                blocks.push(parse_list(
                    source,
                    lines,
                    cursor,
                    marker,
                    depth,
                    diagnostics,
                ));
                continue;
            }
        }
        if is_directive_line(line.text) {
            blocks.push(parse_directive_block(
                source,
                lines,
                cursor,
                depth,
                diagnostics,
            ));
            continue;
        }
        blocks.push(parse_paragraph(source, lines, cursor, diagnostics));
    }
    blocks
}

/// Minimum indentation that makes a line part of a call's indented body:
/// two spaces or a single tab.
const MIN_BODY_INDENT: usize = 2;

/// Parse a block-level Quarkdown function call (`.name {arg} key:{value}`).
///
/// A line only starts a *block* call when the call consumes the entire line
/// (trailing whitespace allowed). Otherwise the line is parsed as a paragraph
/// containing an inline call.
///
/// When the call does become a block call, the following lines are part of
/// its body when they are blank or indented by at least `MIN_BODY_INDENT`
/// columns (or start with a tab). The body must share the same indentation:
/// a line indented less than the first body line terminates the body.
fn parse_directive_block(
    source: &str,
    lines: &[SourceLine<'_>],
    cursor: &mut usize,
    depth: usize,
    diagnostics: &mut Vec<ParserDiagnostic>,
) -> Block {
    let line = &lines[*cursor];
    let mut parsed = None;
    match crate::syntax::quarkdown::parse_directive_at(source, line.text_start) {
        Ok(Some((call, consumed))) => parse_directive_block_ok(
            source,
            lines,
            cursor,
            depth,
            diagnostics,
            call,
            consumed,
            &mut parsed,
        ),
        Err(e) => {
            // Malformed call: recover by parsing the line as an ordinary
            // paragraph. The inline fallback reports the structured
            // diagnostic exactly once (same `parse_directive_at` error).
            let _ = e;
        }
        Ok(None) => {}
    }
    parsed.unwrap_or_else(|| parse_paragraph(source, lines, cursor, diagnostics))
}

#[allow(clippy::too_many_arguments)]
fn parse_directive_block_ok(
    source: &str,
    lines: &[SourceLine<'_>],
    cursor: &mut usize,
    depth: usize,
    diagnostics: &mut Vec<ParserDiagnostic>,
    call: crate::syntax::quarkdown::QuarkdownCall,
    consumed: usize,
    parsed: &mut Option<Block>,
) {
    let line = &lines[*cursor];
    // Trailing non-whitespace content on the same line turns the call
    // into an inline call within a paragraph.
    let trailing_ok = source
        .get(consumed..line.content_end())
        .is_some_and(|rest| rest.chars().all(|c| c.is_ascii_whitespace()));
    if trailing_ok {
        *cursor += 1;
        let body = collect_directive_body(source, lines, cursor, depth, diagnostics);
        let span = block_span_with_body(&call.span, &body);
        *parsed = Some(Block::DirectiveCall {
            name: call.name,
            positional_args: call
                .positional_args
                .iter()
                .map(|arg| convert_quarkdown_arg(source, arg, depth + 1, diagnostics))
                .collect(),
            named_args: call
                .named_args
                .iter()
                .map(|named| {
                    (
                        named.name.clone(),
                        convert_quarkdown_arg(source, &named.value, depth + 1, diagnostics),
                    )
                })
                .collect(),
            body,
            // The block span covers the call header AND the indented
            // body; `call.span` by itself covers only the header.
            span,
        });
    }
}

/// Collect the indented body of a block call starting at the line right
/// after the call (`*cursor`). Advances `*cursor` past the body.
fn collect_directive_body(
    source: &str,
    lines: &[SourceLine<'_>],
    cursor: &mut usize,
    depth: usize,
    diagnostics: &mut Vec<ParserDiagnostic>,
) -> Option<Vec<Block>> {
    let mut start = *cursor;
    while start < lines.len() && lines[start].is_blank() {
        start += 1;
    }
    if start >= lines.len() {
        return None;
    }
    let first = &lines[start];
    let body_indented = first.indent() >= MIN_BODY_INDENT || first.raw.starts_with('\t');
    if !body_indented {
        return None;
    }
    let body_indent = first.indent();

    let mut last = start;
    let mut probe = start + 1;
    while probe < lines.len() {
        let ln = &lines[probe];
        if ln.is_blank() {
            probe += 1;
            continue;
        }
        if ln.indent() >= body_indent {
            last = probe;
            probe += 1;
            continue;
        }
        break;
    }
    let end = last + 1;

    // Re-parse the body lines as blocks. `SourceLine` carries absolute
    // offsets, so the resulting nodes keep accurate document spans and may
    // themselves contain nested calls with their own bodies.
    let mut local_cursor = 0usize;
    let blocks = parse_blocks(
        source,
        &lines[start..end],
        &mut local_cursor,
        depth + 1,
        diagnostics,
    );
    *cursor = end;
    if blocks.is_empty() {
        None
    } else {
        Some(blocks)
    }
}

/// Convert a Quarkdown-layer argument into a Markdown `Value`.
///
/// Scalars map directly; content fragments are run through the inline parser
/// so that nested calls and inline markup inside the argument are preserved.
fn convert_quarkdown_arg(
    source: &str,
    arg: &crate::syntax::quarkdown::Arg,
    depth: usize,
    diagnostics: &mut Vec<ParserDiagnostic>,
) -> Value {
    match &arg.content {
        crate::syntax::quarkdown::ArgContent::Scalar(value) => value.clone(),
        crate::syntax::quarkdown::ArgContent::Content(span) => Value::Content(parse_inlines(
            source,
            span.start,
            span.end,
            depth,
            diagnostics,
        )),
    }
}

/// The span of a `Block` node, extracted from whichever variant it is.
fn block_span(block: &Block) -> ByteSpan {
    match block {
        Block::Heading { span, .. } => *span,
        Block::Paragraph { span, .. } => *span,
        Block::UnorderedList { span, .. } => *span,
        Block::CodeBlock { span, .. } => *span,
        Block::ThematicBreak { span } => *span,
        Block::BlankLine { span } => *span,
        Block::DirectiveCall { span, .. } => *span,
        Block::Metadata { span, .. } => *span,
    }
}

/// The complete span of a block directive: the header plus the body, so
/// the outer span ends at the last body source.
fn block_span_with_body(header: &ByteSpan, body: &Option<Vec<Block>>) -> ByteSpan {
    match body {
        Some(blocks) => {
            let end = blocks
                .last()
                .map(block_span)
                .map(|s| s.end)
                .unwrap_or(header.end);
            ByteSpan::new(header.start, end.max(header.end))
        }
        None => *header,
    }
}
fn parse_paragraph(
    source: &str,
    lines: &[SourceLine<'_>],
    cursor: &mut usize,
    diagnostics: &mut Vec<ParserDiagnostic>,
) -> Block {
    let first = &lines[*cursor];
    loop {
        *cursor += 1;
        if *cursor >= lines.len() || lines[*cursor].is_blank() || lines[*cursor].starts_block() {
            break;
        }
    }
    let last = &lines[*cursor - 1];
    let content = parse_inlines(source, first.text_start, last.content_end(), 0, diagnostics);
    Block::Paragraph {
        content,
        span: ByteSpan::new(first.text_start, last.end),
    }
}

/// Parse an ATX heading line.
fn parse_heading(
    source: &str,
    line: &SourceLine<'_>,
    level: usize,
    diagnostics: &mut Vec<ParserDiagnostic>,
) -> Block {
    let content_start = line.text_start + level;
    let rest = &line.text[level..];
    let content_trim = rest
        .bytes()
        .take_while(|b| *b == b' ' || *b == b'\t')
        .count();
    let content_start = content_start + content_trim;
    let content_end = match trailing_hash_run_start(source, line.text_start, line.content_end()) {
        Some(run_start) => run_start,
        None => line.content_end(),
    };
    let content = if content_start < content_end {
        parse_inlines(source, content_start, content_end, 0, diagnostics)
    } else {
        Vec::new()
    };
    Block::Heading {
        level,
        content,
        span: ByteSpan::new(line.text_start, line.end),
    }
}

/// Find the start of a trailing `#` run preceded by a space, if any.
///
/// CommonMark strips a closing hash sequence that is preceded by whitespace.
fn trailing_hash_run_start(source: &str, start: usize, end: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut pos = end;
    while pos > start && bytes[pos - 1] == b'#' {
        pos -= 1;
    }
    if pos == end {
        return None;
    }
    if pos > start && matches!(bytes[pos - 1], b' ' | b'\t') {
        Some(pos - 1)
    } else {
        None
    }
}

/// Parse a fenced code block starting at the cursor.
///
/// Consumes lines until a closing fence with at least `fence_len` backticks,
/// or the end of the source when unclosed. Content preserves relative line
/// indentation.
fn parse_code_block(lines: &[SourceLine<'_>], cursor: &mut usize, fence_len: usize) -> Block {
    let fence_line = &lines[*cursor];
    let info = fence_line.text[fence_len..].trim();
    let language = info.split_whitespace().next().map(str::to_string);
    *cursor += 1;
    let mut content_lines = Vec::new();
    let mut end_span = fence_line.end;
    while *cursor < lines.len() {
        let line = &lines[*cursor];
        if is_closing_fence(line.text, fence_len) {
            end_span = line.end;
            *cursor += 1;
            break;
        }
        content_lines.push(line.raw);
        end_span = line.end;
        *cursor += 1;
    }
    Block::CodeBlock {
        language,
        source: content_lines.join("\n"),
        span: ByteSpan::new(fence_line.text_start, end_span),
    }
}

/// Parse an unordered list starting at the cursor.
///
/// Items are lines beginning with the same marker character. Continuation
/// lines are those indented by at least the item content column, plus blank
/// lines that are followed by a continuation line. Nested lists are parsed
/// recursively from indented item content.
fn parse_list(
    source: &str,
    lines: &[SourceLine<'_>],
    cursor: &mut usize,
    marker: u8,
    depth: usize,
    diagnostics: &mut Vec<ParserDiagnostic>,
) -> Block {
    let mut items = Vec::new();
    let content_col = {
        let first = &lines[*cursor];
        let ws_after_marker = first.text[1..]
            .bytes()
            .take_while(|b| *b == b' ' || *b == b'\t')
            .count();
        first.indent() + 1 + ws_after_marker
    };
    while *cursor < lines.len() {
        let line = &lines[*cursor];
        if !is_item_start(line.text, marker) {
            break;
        }
        let item_start = line.raw_start;
        let mut item_lines = Vec::new();
        let ws_after_marker = line.text[1..]
            .bytes()
            .take_while(|b| *b == b' ' || *b == b'\t')
            .count();
        item_lines.push(SourceLine {
            raw: &line.text[1 + ws_after_marker..],
            text: &line.text[1 + ws_after_marker..],
            raw_start: line.text_start + 1 + ws_after_marker,
            text_start: line.text_start + 1 + ws_after_marker,
            term: line.term,
            end: line.end,
        });
        *cursor += 1;
        loop {
            if *cursor >= lines.len() {
                break;
            }
            let next = &lines[*cursor];
            if next.indent() >= content_col {
                item_lines.push(strip_indent(next, content_col));
                *cursor += 1;
            } else if next.is_blank() {
                let mut lookahead = *cursor + 1;
                while lookahead < lines.len() && lines[lookahead].is_blank() {
                    lookahead += 1;
                }
                if lookahead < lines.len()
                    && (lines[lookahead].indent() >= content_col
                        || is_item_start(lines[lookahead].text, marker))
                {
                    item_lines.push(SourceLine {
                        raw: "",
                        text: "",
                        raw_start: next.raw_start,
                        text_start: next.raw_start,
                        term: next.term,
                        end: next.end,
                    });
                    *cursor += 1;
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        let mut inner_cursor = 0;
        let content = parse_blocks(
            source,
            &item_lines,
            &mut inner_cursor,
            depth + 1,
            diagnostics,
        );
        let last_line = &item_lines[item_lines.len() - 1];
        items.push(ListItem {
            content,
            span: ByteSpan::new(item_start, last_line.end),
        });
    }
    let span_start = items[0].span.start;
    let span_end = items[items.len() - 1].span.end;
    Block::UnorderedList {
        items,
        span: ByteSpan::new(span_start, span_end),
    }
}

/// Build an inner line with the first `content_col` whitespace columns removed.
fn strip_indent<'a>(line: &SourceLine<'a>, content_col: usize) -> SourceLine<'a> {
    let strip = content_col.min(line.raw.len());
    let raw = &line.raw[strip..];
    let ws = raw
        .bytes()
        .take_while(|b| *b == b' ' || *b == b'\t')
        .count();
    SourceLine {
        raw,
        text: &raw[ws..],
        raw_start: line.raw_start + strip,
        text_start: line.raw_start + strip + ws,
        term: line.term,
        end: line.end,
    }
}

/// Number of leading backticks if the line opens a fenced code block.
fn fence_length(text: &str) -> Option<usize> {
    let len = text.bytes().take_while(|b| *b == b'`').count();
    if len >= 3 {
        Some(len)
    } else {
        None
    }
}

/// Whether the line is a closing fence with at least `fence_len` backticks.
fn is_closing_fence(text: &str, fence_len: usize) -> bool {
    !text.is_empty() && text.len() >= fence_len && text.bytes().all(|b| b == b'`')
}

/// Heading level if the line is an ATX heading.
fn is_heading_text(text: &str) -> Option<usize> {
    let hashes = text.bytes().take_while(|b| *b == b'#').count();
    if !(1..=6).contains(&hashes) {
        return None;
    }
    let rest = &text[hashes..];
    if rest.is_empty() {
        return Some(hashes);
    }
    if matches!(rest.as_bytes()[0], b' ' | b'\t') {
        Some(hashes)
    } else {
        None
    }
}

/// Whether the line is a thematic break (three or more identical markers).
fn is_thematic_break(text: &str) -> bool {
    let mut marker = 0u8;
    let mut count = 0usize;
    for b in text.bytes() {
        match b {
            b' ' | b'\t' => {}
            b'-' | b'*' | b'_' => {
                if marker == 0 {
                    marker = b;
                }
                if b != marker {
                    return false;
                }
                count += 1;
            }
            _ => return false,
        }
    }
    count >= 3
}

/// The list marker byte if the line starts an unordered list item.
fn is_list_marker(text: &str) -> Option<u8> {
    let b = text.as_bytes().first().copied()?;
    if matches!(b, b'-' | b'*' | b'+') && is_item_start(text, b) {
        Some(b)
    } else {
        None
    }
}

/// Whether the text starts a Quarkdown function call (block-level).
fn is_directive_line(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.first() == Some(&b'.')
        && bytes.len() > 1
        && (bytes[1].is_ascii_alphabetic() || bytes[1] == b'_' || matches!(bytes[1], b'1'..=b'9'))
}

/// Whether the line starts a list item with the given marker.
fn is_item_start(text: &str, marker: u8) -> bool {
    let bytes = text.as_bytes();
    bytes.first() == Some(&marker) && bytes.len() > 1 && matches!(bytes[1], b' ' | b'\t')
}

/// A byte-oriented inline parser over a contiguous source slice.
struct InlineParser<'a> {
    source: &'a str,
    pos: usize,
    end: usize,
    depth: usize,
    diagnostics: &'a mut Vec<ParserDiagnostic>,
}

impl<'a> InlineParser<'a> {
    fn new(
        source: &'a str,
        start: usize,
        end: usize,
        depth: usize,
        diagnostics: &'a mut Vec<ParserDiagnostic>,
    ) -> Self {
        Self {
            source,
            pos: start,
            end,
            depth,
            diagnostics,
        }
    }

    fn bytes(&self) -> &[u8] {
        self.source.as_bytes()
    }

    /// Parse all inline nodes in the slice.
    fn parse(&mut self) -> Vec<Inline> {
        let mut inlines = Vec::new();
        while self.pos < self.end {
            match self.bytes()[self.pos] {
                b'\n' => self.parse_break(&mut inlines),
                b'*' | b'_' => self.parse_delimiter(&mut inlines),
                b'\\' if self.pos + 1 < self.end && self.bytes()[self.pos + 1] == b'\n' => {
                    inlines.push(Inline::HardBreak {
                        span: ByteSpan::new(self.pos, self.pos + 2),
                    });
                    self.pos += 2;
                }
                b'.' => self.parse_dot_inline(&mut inlines),
                _ => self.parse_text(&mut inlines),
            }
        }
        inlines
    }

    /// Parse a soft or hard line break.
    fn parse_break(&mut self, inlines: &mut Vec<Inline>) {
        let mut spaces = 0;
        while spaces < self.pos && self.bytes()[self.pos - 1 - spaces] == b' ' {
            spaces += 1;
        }
        if spaces >= 2 {
            inlines.push(Inline::HardBreak {
                span: ByteSpan::new(self.pos - spaces, self.pos + 1),
            });
            self.pos += 1;
        } else {
            inlines.push(Inline::SoftBreak {
                span: ByteSpan::new(self.pos, self.pos + 1),
            });
            self.pos += 1;
            while self.pos < self.end && matches!(self.bytes()[self.pos], b' ' | b'\t') {
                self.pos += 1;
            }
        }
    }

    /// Parse an inline Quarkdown function call (`.note {arg} ...`).
    ///
    /// Only a dot that can actually start a call is delegated here: the
    /// following byte must be a valid name start and the preceding byte must
    /// not be a word character or another dot (`3.14`, `foo.bar`, `...`).
    fn parse_dot_inline(&mut self, inlines: &mut Vec<Inline>) {
        let start = self.pos;
        if start + 1 >= self.end || !is_call_dot(self.bytes(), self.pos, self.end) {
            self.literal_dot(start, inlines);
            return;
        }
        match crate::syntax::quarkdown::parse_directive_at(self.source, start) {
            Ok(Some((call, consumed))) => {
                inlines.push(Inline::DirectiveCall {
                    name: call.name,
                    positional_args: call
                        .positional_args
                        .iter()
                        .map(|arg| {
                            convert_quarkdown_arg(
                                self.source,
                                arg,
                                self.depth + 1,
                                self.diagnostics,
                            )
                        })
                        .collect(),
                    named_args: call
                        .named_args
                        .iter()
                        .map(|named| {
                            (
                                named.name.clone(),
                                convert_quarkdown_arg(
                                    self.source,
                                    &named.value,
                                    self.depth + 1,
                                    self.diagnostics,
                                ),
                            )
                        })
                        .collect(),
                    body: None,
                    span: ByteSpan::new(start, consumed),
                });
                self.pos = consumed;
            }
            Err(e) => {
                // Malformed inline call: record the diagnostic and recover
                // by emitting the dot as literal text.
                self.diagnostics.push(ParserDiagnostic {
                    code: e.code,
                    message: e.message,
                    span: e.span,
                });
                self.literal_dot(start, inlines);
            }
            Ok(None) => self.literal_dot(start, inlines),
        }
    }

    fn literal_dot(&mut self, start: usize, inlines: &mut Vec<Inline>) {
        inlines.push(Inline::Text {
            content: ".".to_string(),
            span: ByteSpan::new(start, start + 1),
        });
        self.pos = start + 1;
    }

    /// Parse emphasis or strong delimiters, falling back to literal text.
    fn parse_delimiter(&mut self, inlines: &mut Vec<Inline>) {
        let marker = self.bytes()[self.pos];
        let strong = self.pos + 1 < self.end && self.bytes()[self.pos + 1] == marker;
        if self.depth < MAX_INLINE_DEPTH {
            if strong && self.try_strong(marker, inlines) {
                return;
            }
            if !strong && self.try_emphasis(marker, inlines) {
                return;
            }
        }
        let width = if strong { 2 } else { 1 };
        inlines.push(Inline::Text {
            content: self.source[self.pos..self.pos + width].to_string(),
            span: ByteSpan::new(self.pos, self.pos + width),
        });
        self.pos += width;
    }

    /// Attempt to parse `**text**` or `__text__` at the current position.
    fn try_strong(&mut self, marker: u8, inlines: &mut Vec<Inline>) -> bool {
        let open = self.pos;
        let open_end = open + 2;
        if open_end >= self.end {
            return false;
        }
        let after_open = self.bytes()[open_end];
        if after_open.is_ascii_whitespace() || after_open == marker {
            return false;
        }
        if marker == b'_' && open > 0 && self.bytes()[open - 1].is_ascii_alphanumeric() {
            return false;
        }
        let Some(close_start) = self.find_close(open_end, marker, 2) else {
            return false;
        };
        if self.is_whitespace_only(open_end, close_start) {
            return false;
        }
        let content = InlineParser::new(
            self.source,
            open_end,
            close_start,
            self.depth + 1,
            self.diagnostics,
        )
        .parse();
        inlines.push(Inline::Strong {
            content,
            span: ByteSpan::new(open, close_start + 2),
        });
        self.pos = close_start + 2;
        true
    }

    /// Attempt to parse `*text*` or `_text_` at the current position.
    fn try_emphasis(&mut self, marker: u8, inlines: &mut Vec<Inline>) -> bool {
        let open = self.pos;
        let open_end = open + 1;
        if open_end >= self.end {
            return false;
        }
        let after_open = self.bytes()[open_end];
        if after_open.is_ascii_whitespace() {
            return false;
        }
        if marker == b'_' && open > 0 && self.bytes()[open - 1].is_ascii_alphanumeric() {
            return false;
        }
        let Some(close_start) = self.find_close(open_end, marker, 1) else {
            return false;
        };
        if self.is_whitespace_only(open_end, close_start) {
            return false;
        }
        let content = InlineParser::new(
            self.source,
            open_end,
            close_start,
            self.depth + 1,
            self.diagnostics,
        )
        .parse();
        inlines.push(Inline::Emphasis {
            content,
            span: ByteSpan::new(open, close_start + 1),
        });
        self.pos = close_start + 1;
        true
    }

    /// Find the first valid closing delimiter of the given length.
    ///
    /// A candidate is rejected when it is adjacent to the same marker on
    /// either side, when its preceding character is whitespace, or for
    /// `_` delimiters, when the character after it is alphanumeric.
    fn find_close(&self, from: usize, marker: u8, len: usize) -> Option<usize> {
        let bytes = self.bytes();
        let mut pos = from;
        while pos + len <= self.end {
            if bytes[pos] != marker {
                pos += 1;
                continue;
            }
            if len == 2 && (pos + 1 >= self.end || bytes[pos + 1] != marker) {
                pos += 1;
                continue;
            }
            if pos > 0 {
                let prev = bytes[pos - 1];
                if prev.is_ascii_whitespace() || prev == marker {
                    pos += 1;
                    continue;
                }
            }
            let after = pos + len;
            if after < self.end {
                let next = bytes[after];
                if next == marker || (marker == b'_' && next.is_ascii_alphanumeric()) {
                    pos += 1;
                    continue;
                }
            }
            return Some(pos);
        }
        None
    }

    /// Whether the given range contains only whitespace.
    fn is_whitespace_only(&self, start: usize, end: usize) -> bool {
        self.bytes()[start..end]
            .iter()
            .all(|b| b.is_ascii_whitespace())
    }

    /// Parse a run of literal text up to the next special byte.
    ///
    /// When the run ends at a newline preceded by two or more spaces, the
    /// trailing spaces are trimmed here and the following hard break is
    /// emitted by `parse_break`.
    fn parse_text(&mut self, inlines: &mut Vec<Inline>) {
        let start = self.pos;
        let bytes = self.source.as_bytes();
        while self.pos < self.end {
            let b = bytes[self.pos];
            if b == b'\r' {
                self.pos += 1;
                continue;
            }
            if b == b'*' || b == b'_' || b == b'\n' {
                break;
            }
            if b == b'.' && is_call_dot(bytes, self.pos, self.end) {
                break;
            }
            if b == b'\\' && self.pos + 1 < self.end && bytes[self.pos + 1] == b'\n' {
                break;
            }
            self.pos += 1;
        }
        if self.pos < self.end && bytes[self.pos] == b'\n' {
            let mut trail = 0;
            while self.pos - 1 - trail >= start && bytes[self.pos - 1 - trail] == b' ' {
                trail += 1;
            }
            if trail >= 2 {
                let end = self.pos - trail;
                if end > start {
                    inlines.push(Inline::Text {
                        content: self.source[start..end].to_string(),
                        span: ByteSpan::new(start, end),
                    });
                }
                return;
            }
        }
        if self.pos > start {
            inlines.push(Inline::Text {
                content: self.source[start..self.pos].to_string(),
                span: ByteSpan::new(start, self.pos),
            });
        }
    }
}

/// Whether the dot at `pos` can start a function call: the byte after it is a
/// valid name start, and the byte before it is not a word character or
/// another dot (`3.14`, `foo.bar`, `...` do not start calls).
fn is_call_dot(bytes: &[u8], pos: usize, end: usize) -> bool {
    if pos + 1 >= end {
        return false;
    }
    let next = bytes[pos + 1];
    if !(next.is_ascii_alphabetic() || next == b'_' || matches!(next, b'1'..=b'9')) {
        return false;
    }
    if pos == 0 {
        return true;
    }
    let prev = bytes[pos - 1];
    !(prev.is_ascii_alphanumeric() || prev == b'_' || prev == b'-' || prev == b'.')
}

/// Parse inline nodes from the contiguous source slice `[start, end)`.
fn parse_inlines(
    source: &str,
    start: usize,
    end: usize,
    depth: usize,
    diagnostics: &mut Vec<ParserDiagnostic>,
) -> Vec<Inline> {
    InlineParser::new(source, start, end, depth, diagnostics).parse()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_text(inline: &Inline, expected: &str) {
        match inline {
            Inline::Text { content, .. } => assert_eq!(content, expected),
            other => panic!("expected Text({expected:?}), got {other:?}"),
        }
    }

    /// Concatenate all text in a flat list of inline nodes.
    fn joined_text(inlines: &[Inline]) -> String {
        let mut out = String::new();
        for inline in inlines {
            match inline {
                Inline::Text { content, .. } => out.push_str(content),
                other => panic!("expected Text, got {other:?}"),
            }
        }
        out
    }

    fn paragraph_inlines(doc: &Document) -> &[Inline] {
        match &doc.nodes[0] {
            Block::Paragraph { content, .. } => content,
            other => panic!("expected paragraph, got {other:?}"),
        }
    }

    #[test]
    fn acceptance_two_blocks_for_heading_and_paragraph() {
        let doc = parse("# Hello\n\nWorld");
        assert_eq!(doc.nodes.len(), 2);
    }

    #[test]
    fn empty_source_produces_empty_document() {
        let doc = parse("");
        assert!(doc.nodes.is_empty());
    }

    #[test]
    fn whitespace_only_source_produces_empty_document() {
        let doc = parse("  \n\t\n");
        assert!(doc.nodes.is_empty());
    }

    #[test]
    fn heading_levels_one_through_six() {
        for level in 1..=6 {
            let input = format!("{} Heading {}", "#".repeat(level), "#".repeat(level));
            let doc = parse(&input);
            match &doc.nodes[0] {
                Block::Heading {
                    level: l,
                    content,
                    span,
                } => {
                    assert_eq!(*l, level);
                    assert_text(&content[0], "Heading");
                    assert_eq!(*span, ByteSpan::new(0, input.len()));
                }
                other => panic!("expected heading, got {other:?}"),
            }
        }
    }

    #[test]
    fn heading_requires_space_after_hashes() {
        let doc = parse("#Not a heading");
        assert_text(&paragraph_inlines(&doc)[0], "#Not a heading");
    }

    #[test]
    fn seven_hashes_are_not_a_heading() {
        let doc = parse("####### x");
        assert_text(&paragraph_inlines(&doc)[0], "####### x");
    }

    #[test]
    fn heading_without_trailing_hashes() {
        let doc = parse("# Heading");
        match &doc.nodes[0] {
            Block::Heading { content, .. } => assert_text(&content[0], "Heading"),
            other => panic!("expected heading, got {other:?}"),
        }
    }

    #[test]
    fn heading_with_inline_emphasis() {
        let doc = parse("# *Hi*");
        match &doc.nodes[0] {
            Block::Heading { content, .. } => {
                assert!(matches!(content[0], Inline::Emphasis { .. }));
            }
            other => panic!("expected heading, got {other:?}"),
        }
    }

    #[test]
    fn heading_span_covers_line_and_newline() {
        let doc = parse("# Hi\nnext");
        match &doc.nodes[0] {
            Block::Heading { span, .. } => assert_eq!(*span, ByteSpan::new(0, 5)),
            other => panic!("expected heading, got {other:?}"),
        }
    }

    #[test]
    fn paragraph_joins_lines_with_soft_break() {
        let doc = parse("line one\nline two");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 3);
        assert_text(&content[0], "line one");
        assert!(matches!(content[1], Inline::SoftBreak { .. }));
        assert_text(&content[2], "line two");
    }

    #[test]
    fn hard_break_from_trailing_spaces() {
        let doc = parse("line one  \nline two");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 3);
        assert_text(&content[0], "line one");
        assert!(matches!(content[1], Inline::HardBreak { .. }));
        assert_text(&content[2], "line two");
    }

    #[test]
    fn hard_break_from_backslash() {
        let doc = parse("line one\\\nline two");
        let content = paragraph_inlines(&doc);
        assert!(matches!(content[1], Inline::HardBreak { .. }));
    }

    #[test]
    fn single_trailing_space_is_a_soft_break() {
        let doc = parse("line one \nline two");
        let content = paragraph_inlines(&doc);
        assert!(matches!(content[1], Inline::SoftBreak { .. }));
    }

    #[test]
    fn emphasis_star_and_underscore() {
        for input in ["*italic*", "_italic_"] {
            let doc = parse(input);
            let content = paragraph_inlines(&doc);
            assert_eq!(content.len(), 1, "input: {input}");
            match &content[0] {
                Inline::Emphasis { content, span } => {
                    assert_text(&content[0], "italic");
                    assert_eq!(*span, ByteSpan::new(0, input.len()));
                }
                other => panic!("expected emphasis, got {other:?}"),
            }
        }
    }

    #[test]
    fn strong_star_and_underscore() {
        for input in ["**bold**", "__bold__"] {
            let doc = parse(input);
            let content = paragraph_inlines(&doc);
            assert_eq!(content.len(), 1, "input: {input}");
            match &content[0] {
                Inline::Strong { content, span } => {
                    assert_text(&content[0], "bold");
                    assert_eq!(*span, ByteSpan::new(0, input.len()));
                }
                other => panic!("expected strong, got {other:?}"),
            }
        }
    }

    #[test]
    fn nested_emphasis_inside_strong() {
        let doc = parse("**outer *inner* end**");
        let content = paragraph_inlines(&doc);
        match &content[0] {
            Inline::Strong { content, .. } => {
                assert_text(&content[0], "outer ");
                match &content[1] {
                    Inline::Emphasis { content, .. } => assert_text(&content[0], "inner"),
                    other => panic!("expected emphasis, got {other:?}"),
                }
                assert_text(&content[2], " end");
            }
            other => panic!("expected strong, got {other:?}"),
        }
    }

    #[test]
    fn adjacent_text_nodes_join_to_source() {
        let doc = parse("***not emphasized***");
        assert_eq!(joined_text(paragraph_inlines(&doc)), "***not emphasized***");
    }

    #[test]
    fn underscore_inside_word_is_literal() {
        let doc = parse("foo_bar_baz");
        let content = paragraph_inlines(&doc);
        assert_eq!(joined_text(content), "foo_bar_baz");
        assert!(content.iter().all(|i| matches!(i, Inline::Text { .. })));
    }

    #[test]
    fn empty_delimiters_are_literal() {
        let doc = parse("** ** and * *");
        let content = paragraph_inlines(&doc);
        assert_eq!(joined_text(content), "** ** and * *");
        assert!(content.iter().all(|i| matches!(i, Inline::Text { .. })));
    }

    #[test]
    fn unclosed_emphasis_does_not_panic() {
        let doc = parse("*unclosed");
        assert_eq!(joined_text(paragraph_inlines(&doc)), "*unclosed");
    }

    #[test]
    fn unclosed_strong_does_not_panic() {
        let doc = parse("**unclosed");
        assert_eq!(joined_text(paragraph_inlines(&doc)), "**unclosed");
    }

    #[test]
    fn unordered_list_single_item() {
        let doc = parse("- item");
        match &doc.nodes[0] {
            Block::UnorderedList { items, span } => {
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].content.len(), 1);
                assert_eq!(items[0].span, ByteSpan::new(0, 6));
                assert_eq!(*span, ByteSpan::new(0, 6));
            }
            other => panic!("expected list, got {other:?}"),
        }
    }

    #[test]
    fn unordered_list_multiple_items() {
        let doc = parse("- one\n- two\n- three");
        match &doc.nodes[0] {
            Block::UnorderedList { items, .. } => {
                assert_eq!(items.len(), 3);
                for (i, item) in items.iter().enumerate() {
                    match &item.content[0] {
                        Block::Paragraph { content, .. } => {
                            assert_text(&content[0], ["one", "two", "three"][i]);
                        }
                        other => panic!("expected paragraph, got {other:?}"),
                    }
                }
            }
            other => panic!("expected list, got {other:?}"),
        }
    }

    #[test]
    fn all_marker_characters_start_lists() {
        for marker in ['-', '*', '+'] {
            let doc = parse(&format!("{marker} item"));
            match &doc.nodes[0] {
                Block::UnorderedList { items, .. } => assert_eq!(items.len(), 1),
                other => panic!("expected list, got {other:?}"),
            }
        }
    }

    #[test]
    fn marker_without_space_is_a_paragraph() {
        let doc = parse("-item\n*item");
        assert_text(&paragraph_inlines(&doc)[0], "-item");
    }

    #[test]
    fn different_markers_split_into_separate_lists() {
        let doc = parse("- one\n* two");
        assert_eq!(doc.nodes.len(), 2);
        for node in &doc.nodes {
            assert!(matches!(node, Block::UnorderedList { .. }));
        }
    }

    #[test]
    fn item_continuation_lines_join_paragraph() {
        let doc = parse("- first line\n  second line");
        match &doc.nodes[0] {
            Block::UnorderedList { items, .. } => match &items[0].content[0] {
                Block::Paragraph { content, .. } => {
                    assert_eq!(content.len(), 3);
                    assert_text(&content[0], "first line");
                    assert!(matches!(content[1], Inline::SoftBreak { .. }));
                    assert_text(&content[2], "second line");
                }
                other => panic!("expected paragraph, got {other:?}"),
            },
            other => panic!("expected list, got {other:?}"),
        }
    }

    #[test]
    fn nested_list() {
        let doc = parse("- outer\n  - inner");
        match &doc.nodes[0] {
            Block::UnorderedList { items, .. } => {
                assert_eq!(items.len(), 1);
                match &items[0].content[1] {
                    Block::UnorderedList { items, .. } => assert_eq!(items.len(), 1),
                    other => panic!("expected nested list, got {other:?}"),
                }
            }
            other => panic!("expected list, got {other:?}"),
        }
    }

    #[test]
    fn blank_lines_between_items_keep_single_list() {
        let doc = parse("- one\n\n- two");
        match &doc.nodes[0] {
            Block::UnorderedList { items, .. } => assert_eq!(items.len(), 2),
            other => panic!("expected list, got {other:?}"),
        }
    }

    #[test]
    fn item_blank_line_before_continuation() {
        let doc = parse("- one\n\n  continued");
        match &doc.nodes[0] {
            Block::UnorderedList { items, .. } => assert_eq!(items[0].content.len(), 2),
            other => panic!("expected list, got {other:?}"),
        }
    }

    #[test]
    fn unindented_line_after_item_ends_list() {
        let doc = parse("- one\nplain");
        assert_eq!(doc.nodes.len(), 2);
        assert!(matches!(doc.nodes[0], Block::UnorderedList { .. }));
        assert!(matches!(doc.nodes[1], Block::Paragraph { .. }));
    }

    #[test]
    fn item_containing_code_block() {
        let doc = parse("- item\n  ```\n  code\n  ```");
        match &doc.nodes[0] {
            Block::UnorderedList { items, .. } => match &items[0].content[1] {
                Block::CodeBlock { source, .. } => assert_eq!(source, "code"),
                other => panic!("expected code block, got {other:?}"),
            },
            other => panic!("expected list, got {other:?}"),
        }
    }

    #[test]
    fn fenced_code_block_with_language() {
        let doc = parse("```rust\nfn main() {}\n```");
        match &doc.nodes[0] {
            Block::CodeBlock {
                language,
                source,
                span,
            } => {
                assert_eq!(language.as_deref(), Some("rust"));
                assert_eq!(source, "fn main() {}");
                assert_eq!(*span, ByteSpan::new(0, 24));
            }
            other => panic!("expected code block, got {other:?}"),
        }
    }

    #[test]
    fn fenced_code_block_preserves_indentation() {
        let doc = parse("```\n  indented\n    deeper\n```");
        match &doc.nodes[0] {
            Block::CodeBlock { source, .. } => assert_eq!(source, "  indented\n    deeper"),
            other => panic!("expected code block, got {other:?}"),
        }
    }

    #[test]
    fn unclosed_code_block_runs_to_end() {
        let doc = parse("```\nnever closed");
        match &doc.nodes[0] {
            Block::CodeBlock { source, .. } => assert_eq!(source, "never closed"),
            other => panic!("expected code block, got {other:?}"),
        }
    }

    #[test]
    fn closing_fence_shorter_than_opening_does_not_close() {
        let doc = parse("````\n```\nstill code\n````");
        match &doc.nodes[0] {
            Block::CodeBlock { source, .. } => assert_eq!(source, "```\nstill code"),
            other => panic!("expected code block, got {other:?}"),
        }
    }

    #[test]
    fn closing_fence_cannot_have_info_string() {
        let doc = parse("```\ncode\n```rust\nmore\n```");
        match &doc.nodes[0] {
            Block::CodeBlock { source, .. } => assert_eq!(source, "code\n```rust\nmore"),
            other => panic!("expected code block, got {other:?}"),
        }
    }

    #[test]
    fn thematic_break_variants() {
        for input in ["---", "***", "___", "- - -", "* * *", "--- --- ---"] {
            let doc = parse(input);
            assert!(
                matches!(doc.nodes[0], Block::ThematicBreak { .. }),
                "input: {input}"
            );
        }
    }

    #[test]
    fn two_dashes_are_not_a_thematic_break() {
        let doc = parse("--");
        assert!(matches!(doc.nodes[0], Block::Paragraph { .. }));
    }

    #[test]
    fn mixed_markers_are_not_a_thematic_break() {
        let doc = parse("- -*");
        assert!(matches!(doc.nodes[0], Block::UnorderedList { .. }));
    }

    #[test]
    fn thematic_break_before_list_marker() {
        let doc = parse("- - -");
        assert!(matches!(doc.nodes[0], Block::ThematicBreak { .. }));
    }

    #[test]
    fn paragraph_ends_at_thematic_break() {
        let doc = parse("text\n---");
        assert_eq!(doc.nodes.len(), 2);
        assert!(matches!(doc.nodes[0], Block::Paragraph { .. }));
        assert!(matches!(doc.nodes[1], Block::ThematicBreak { .. }));
    }

    #[test]
    fn crlf_input_is_normalized() {
        let doc = parse("# Hi\r\n\r\nWorld\r\n");
        assert_eq!(doc.nodes.len(), 2);
        match &doc.nodes[0] {
            Block::Heading { content, .. } => assert_text(&content[0], "Hi"),
            other => panic!("expected heading, got {other:?}"),
        }
    }

    #[test]
    fn multibyte_spans_are_byte_accurate() {
        let doc = parse("# 한국어 제목\n\n본문 내용");
        match &doc.nodes[0] {
            Block::Heading { content, span, .. } => {
                assert_text(&content[0], "한국어 제목");
                assert_eq!(*span, ByteSpan::new(0, 19));
            }
            other => panic!("expected heading, got {other:?}"),
        }
        match &doc.nodes[1] {
            Block::Paragraph { span, .. } => assert_eq!(*span, ByteSpan::new(20, 33)),
            other => panic!("expected paragraph, got {other:?}"),
        }
    }

    #[test]
    fn break_spans_after_multibyte_are_byte_accurate() {
        let doc = parse("안녕\n세상");
        let content = paragraph_inlines(&doc);
        assert_text(&content[0], "안녕");
        match &content[1] {
            Inline::SoftBreak { span } => {
                // "안녕" is 6 UTF-8 bytes; the newline starts at byte offset 6.
                assert_eq!(*span, ByteSpan::new(6, 7));
            }
            other => panic!("expected SoftBreak, got {other:?}"),
        }
        assert_text(&content[2], "세상");

        let doc = parse("안녕  \n세상");
        let content = paragraph_inlines(&doc);
        assert_text(&content[0], "안녕");
        match &content[1] {
            Inline::HardBreak { span } => {
                // Two trailing spaces occupy bytes 6..8, newline at byte 8.
                assert_eq!(*span, ByteSpan::new(6, 9));
            }
            other => panic!("expected HardBreak, got {other:?}"),
        }
        assert_text(&content[2], "세상");
    }

    #[test]
    fn emphasis_with_unicode_content() {
        let doc = parse("*강조*");
        let content = paragraph_inlines(&doc);
        match &content[0] {
            Inline::Emphasis { content, span } => {
                assert_text(&content[0], "강조");
                assert_eq!(*span, ByteSpan::new(0, 8));
            }
            other => panic!("expected emphasis, got {other:?}"),
        }
    }

    #[test]
    fn line_count_tracks_newlines() {
        assert_eq!(parse("").line_count, 1);
        assert_eq!(parse("a\nb\nc").line_count, 3);
        assert_eq!(parse("a\n").line_count, 2);
    }

    #[test]
    fn deeply_nested_lists_do_not_panic() {
        let mut input = String::from("- top\n");
        for _ in 0..200 {
            input.push_str("  - nested\n");
        }
        let doc = parse(&input);
        assert_eq!(doc.nodes.len(), 1);
        assert!(matches!(doc.nodes[0], Block::UnorderedList { .. }));
    }

    #[test]
    fn deeply_nested_emphasis_does_not_panic() {
        let mut input = String::new();
        for _ in 0..200 {
            input.push_str("*a ");
        }
        input.push('x');
        for _ in 0..200 {
            input.push_str(" a*");
        }
        let doc = parse(&input);
        assert_eq!(doc.nodes.len(), 1);
    }

    #[test]
    fn document_snapshot_mixed() {
        insta::assert_debug_snapshot!(
            "mixed_document",
            parse("# Title\n\nIntro *em* and **strong**.\n\n- one\n- two\n")
        );
    }

    #[test]
    fn document_snapshot_code_and_break() {
        insta::assert_debug_snapshot!(
            "code_and_break",
            parse("```rust\nlet x = 1;\n```\n\nEnd  \nof line.\n")
        );
    }

    #[test]
    fn parse_front_matter_at_document_start() {
        let doc = parse("---\ntitle: Hello\nauthor: World\n---\n\n# Heading\n");
        assert!(doc.front_matter.is_some());
        let fm = doc.front_matter.unwrap();
        assert_eq!(fm.fields.len(), 2);
        assert_eq!(fm.fields[0], ("title".into(), "Hello".into()));
        assert_eq!(fm.fields[1], ("author".into(), "World".into()));
        // Front matter span covers from start of first --- to end of second ---
        assert!(fm.span.start == 0);
        assert!(fm.span.end > fm.span.start);
    }

    #[test]
    fn front_matter_is_not_emitted_as_content_blocks() {
        let doc = parse("---\ntitle: Hello\n---\n\n# Heading\n");
        // Only heading block, no blocks for front matter delimiters
        assert_eq!(doc.nodes.len(), 1);
        assert!(matches!(doc.nodes[0], Block::Heading { .. }));
    }

    #[test]
    fn thematic_break_after_content_is_not_front_matter() {
        let doc = parse("# Title\n\n---\n\nContent\n");
        // Front matter only at document start
        assert!(doc.front_matter.is_none());
        assert_eq!(doc.nodes.len(), 3); // heading, thematic break, paragraph
        assert!(matches!(doc.nodes[1], Block::ThematicBreak { .. }));
    }

    #[test]
    fn parse_front_matter_with_crlf() {
        let doc = parse("---\r\ntitle: Hello\r\n---\r\n\r\n# Heading\r\n");
        assert!(doc.front_matter.is_some());
        let fm = doc.front_matter.unwrap();
        assert_eq!(fm.fields.len(), 1);
        assert_eq!(fm.fields[0], ("title".into(), "Hello".into()));
    }

    #[test]
    fn indented_front_matter_opening_delimiter_not_recognized() {
        let doc = parse("  ---\ntitle: Hello\n---\n\n# Heading\n");
        // Indented opening delimiter is not recognized
        assert!(doc.front_matter.is_none());
        // Should be treated as paragraph or thematic break
        assert!(!doc.nodes.is_empty());
    }

    #[test]
    fn indented_front_matter_closing_delimiter_not_recognized() {
        let doc = parse("---\ntitle: Hello\n  ---\n\n# Heading\n");
        // Indented closing delimiter is not recognized
        assert!(doc.front_matter.is_none());
        // Should be treated as unclosed front matter, so content is parsed as blocks
        assert!(!doc.nodes.is_empty());
    }

    /// Returns whether any paragraph in the document contains `needle` text.
    fn has_paragraph_text(doc: &Document, needle: &str) -> bool {
        doc.nodes.iter().any(|node| {
            matches!(
                node,
                Block::Paragraph { content, .. }
                    if content.iter().any(|inline| matches!(
                        inline,
                        Inline::Text { content, .. } if content.contains(needle)
                    ))
            )
        })
    }

    #[test]
    fn indented_key_rejects_front_matter_block() {
        let doc = parse("---\n  title: Hello\n---\n\n# Heading\n");
        // Indented metadata lines are not valid flat key: value front matter
        assert!(doc.front_matter.is_none());
        // The malformed block is preserved as regular Markdown body text
        assert!(has_paragraph_text(&doc, "title: Hello"));
        assert!(doc.nodes.iter().any(|n| matches!(n, Block::Heading { .. })));
    }

    #[test]
    fn nested_object_rejects_front_matter_block() {
        let doc = parse("---\nauthor:\n  name: Alice\n---\n\n# Heading\n");
        // Nested object shape is not flattened into metadata
        assert!(doc.front_matter.is_none());
        assert!(has_paragraph_text(&doc, "name: Alice"));
        assert!(doc.nodes.iter().any(|n| matches!(n, Block::Heading { .. })));
    }

    #[test]
    fn duplicate_custom_key_last_wins() {
        let doc = parse("---\ncustom: First\ncustom: Second\n---\n\n# Heading\n");
        // Duplicate custom key: last-wins, single field
        assert!(doc.front_matter.is_some());
        let fm = doc.front_matter.unwrap();
        assert_eq!(fm.fields.len(), 1);
        assert_eq!(fm.fields[0], ("custom".into(), "Second".into()));
    }

    #[test]
    fn malformed_front_matter_line_rejects_block() {
        let doc = parse("---\ntitle: Hello\ninvalid line\n---\n\n# Heading\n");
        // Malformed line (no colon) causes entire front matter block to be rejected
        // Content is parsed as regular Markdown
        assert!(doc.front_matter.is_none());
        assert!(!doc.nodes.is_empty());
        // The heading should be parsed
        assert!(doc.nodes.iter().any(|n| matches!(n, Block::Heading { .. })));
    }

    #[test]
    fn empty_key_in_front_matter_rejects_block() {
        let doc = parse("---\n: value\n---\n\n# Heading\n");
        // Empty key causes entire block to be rejected
        assert!(doc.front_matter.is_none());
        assert!(!doc.nodes.is_empty());
        assert!(doc.nodes.iter().any(|n| matches!(n, Block::Heading { .. })));
    }

    #[test]
    fn malformed_line_before_valid_field_rejects_block() {
        let doc = parse("---\ninvalid line\ntitle: Hello\n---\n\n# Heading\n");
        // Malformed line before valid field still rejects entire block
        assert!(doc.front_matter.is_none());
        assert!(!doc.nodes.is_empty());
        assert!(doc.nodes.iter().any(|n| matches!(n, Block::Heading { .. })));
    }

    #[test]
    fn partial_front_matter_no_partial_result() {
        let doc = parse("---\ntitle: Hello\ninvalid line\n---\n\n# Heading\n");
        // No partial metadata should be generated
        assert!(doc.front_matter.is_none());
        // But content should be parsed
        assert!(doc.nodes.iter().any(|n| matches!(n, Block::Heading { .. })));
    }

    #[test]
    fn front_matter_value_with_colon() {
        let doc = parse("---\ntitle: Hello: World\n---\n\n# Heading\n");
        // Value can contain colon
        assert!(doc.front_matter.is_some());
        let fm = doc.front_matter.unwrap();
        assert_eq!(fm.fields.len(), 1);
        assert_eq!(fm.fields[0], ("title".into(), "Hello: World".into()));
    }

    #[test]
    fn duplicate_front_matter_key_last_wins() {
        let doc = parse("---\ntitle: First\ntitle: Second\n---\n\n# Heading\n");
        // Duplicate key: last-wins
        assert!(doc.front_matter.is_some());
        let fm = doc.front_matter.unwrap();
        assert_eq!(fm.fields.len(), 1);
        assert_eq!(fm.fields[0], ("title".into(), "Second".into()));
    }

    #[test]
    fn empty_front_matter() {
        let doc = parse("---\n---\n\n# Heading\n");
        // Empty front matter is valid
        assert!(doc.front_matter.is_some());
        let fm = doc.front_matter.unwrap();
        assert_eq!(fm.fields.len(), 0);
    }

    // ------------------------------------------------------------------
    // Quarkdown dot-call syntax
    // ------------------------------------------------------------------

    #[test]
    fn block_call_no_arguments() {
        let doc = parse(".note\n");
        match &doc.nodes[0] {
            Block::DirectiveCall {
                name,
                positional_args,
                named_args,
                body,
                span,
            } => {
                assert_eq!(name, "note");
                assert!(positional_args.is_empty());
                assert!(named_args.is_empty());
                assert!(body.is_none());
                assert_eq!(*span, ByteSpan::new(0, 5));
            }
            other => panic!("expected directive call, got {other:?}"),
        }
    }

    #[test]
    fn block_call_positional_args() {
        let doc = parse(".range {1} {10}\n");
        match &doc.nodes[0] {
            Block::DirectiveCall {
                name,
                positional_args,
                body,
                span,
                ..
            } => {
                assert_eq!(name, "range");
                assert_eq!(positional_args.len(), 2);
                assert_eq!(positional_args[0], Value::Number(1.0));
                assert_eq!(positional_args[1], Value::Number(10.0));
                assert!(body.is_none());
                assert_eq!(*span, ByteSpan::new(0, 15));
            }
            other => panic!("expected directive call, got {other:?}"),
        }
    }

    #[test]
    fn block_call_named_args() {
        let doc = parse(".panel width:{320} align:{center}\n");
        match &doc.nodes[0] {
            Block::DirectiveCall {
                named_args,
                positional_args,
                ..
            } => {
                assert!(positional_args.is_empty());
                assert_eq!(named_args.len(), 2);
                assert_eq!(named_args[0].0, "width");
                assert_eq!(named_args[0].1, Value::Number(320.0));
                assert_eq!(named_args[1].0, "align");
                assert_eq!(named_args[1].1, Value::Identifier("center".into()));
            }
            other => panic!("expected directive call, got {other:?}"),
        }
    }

    #[test]
    fn block_call_mixed_args() {
        let doc = parse(".panel {Introduction} width:{320}\n");
        match &doc.nodes[0] {
            Block::DirectiveCall {
                positional_args,
                named_args,
                ..
            } => {
                assert_eq!(positional_args.len(), 1);
                assert_eq!(positional_args[0], Value::Identifier("Introduction".into()));
                assert_eq!(named_args.len(), 1);
                assert_eq!(named_args[0].0, "width");
                assert_eq!(named_args[0].1, Value::Number(320.0));
            }
            other => panic!("expected directive call, got {other:?}"),
        }
    }

    #[test]
    fn block_call_with_indented_body() {
        let doc = parse(".panel {Intro}\n    Hello world\n");
        match &doc.nodes[0] {
            Block::DirectiveCall { name, body, .. } => {
                assert_eq!(name, "panel");
                let body_blocks = body.as_ref().expect("body");
                assert_eq!(body_blocks.len(), 1);
                match &body_blocks[0] {
                    Block::Paragraph { content, span } => {
                        assert_text(&content[0], "Hello world");
                        // Body text starts at the 4-column indentation (line 1, col 4);
                        // paragraph span runs to the end of the body line.
                        assert_eq!(*span, ByteSpan::new(19, 31));
                    }
                    other => panic!("expected body paragraph, got {other:?}"),
                }
            }
            other => panic!("expected directive call, got {other:?}"),
        }
    }

    #[test]
    fn block_call_body_span_covers_indented_lines() {
        let doc = parse(".note {A}\n  line one\n  line two\n");
        match &doc.nodes[0] {
            Block::DirectiveCall { body, .. } => {
                let body_blocks = body.as_ref().expect("body");
                assert_eq!(body_blocks.len(), 1);
                match &body_blocks[0] {
                    Block::Paragraph { span, .. } => {
                        // Body covers both lines: from first content byte to line end.
                        assert_eq!(*span, ByteSpan::new(12, 32));
                    }
                    other => panic!("expected paragraph, got {other:?}"),
                }
            }
            other => panic!("expected call, got {other:?}"),
        }
    }

    #[test]
    fn block_body_may_contain_markdown_and_nested_calls() {
        let doc = parse(".panel {Outer}\n    Hello\n\n    .note {Nested}\n        Nested body\n");
        match &doc.nodes[0] {
            Block::DirectiveCall { name, body, .. } => {
                assert_eq!(name, "panel");
                let body_blocks = body.as_ref().expect("body");
                assert_eq!(body_blocks.len(), 2, "expected paragraph + nested call");
                match &body_blocks[0] {
                    Block::Paragraph { content, .. } => assert_text(&content[0], "Hello"),
                    other => panic!("expected paragraph, got {other:?}"),
                }
                match &body_blocks[1] {
                    Block::DirectiveCall {
                        name,
                        positional_args,
                        body,
                        span,
                        ..
                    } => {
                        assert_eq!(name, "note");
                        assert_eq!(positional_args.len(), 1);
                        assert_eq!(positional_args[0], Value::Identifier("Nested".into()));
                        // Nested call span covers its header AND its body: starts after its
                        // indentation (30) and ends after "Nested body" (65).
                        assert_eq!(*span, ByteSpan::new(30, 65));
                        let nested_body = body.as_ref().expect("nested body");
                        assert_eq!(nested_body.len(), 1);
                        match &nested_body[0] {
                            Block::Paragraph { content, .. } => {
                                assert_text(&content[0], "Nested body")
                            }
                            other => panic!("expected paragraph, got {other:?}"),
                        }
                    }
                    other => panic!("expected nested call, got {other:?}"),
                }
            }
            other => panic!("expected call, got {other:?}"),
        }
    }

    #[test]
    fn body_requires_minimum_indentation() {
        // A non-indented following line is not a body part.
        let doc = parse(".note\nplain text\n");
        match &doc.nodes[0] {
            Block::DirectiveCall { body, .. } => assert!(body.is_none()),
            other => panic!("expected call, got {other:?}"),
        }
        assert!(matches!(doc.nodes[1], Block::Paragraph { .. }));
    }

    #[test]
    fn body_single_tab_counts_as_body() {
        let doc = parse(".note\n\ttabbed body\n");
        match &doc.nodes[0] {
            Block::DirectiveCall { body, .. } => {
                let body_blocks = body.as_ref().expect("body");
                assert_eq!(body_blocks.len(), 1);
                match &body_blocks[0] {
                    Block::Paragraph { content, .. } => assert_text(&content[0], "tabbed body"),
                    other => panic!("expected paragraph, got {other:?}"),
                }
            }
            other => panic!("expected call, got {other:?}"),
        }
    }

    #[test]
    fn body_stops_at_less_indented_line() {
        let doc = parse(".panel\n    indented\nnot indented\n");
        match &doc.nodes[0] {
            Block::DirectiveCall { body, .. } => {
                let body_blocks = body.as_ref().expect("body");
                assert_eq!(body_blocks.len(), 1);
            }
            other => panic!("expected call, got {other:?}"),
        }
        assert!(
            matches!(doc.nodes[1], Block::Paragraph { .. }),
            "second node"
        );
    }

    #[test]
    fn call_with_trailing_text_is_inline_call() {
        let doc = parse(".note trailing text here\n");
        // The call does not own the line, so the whole line is a paragraph
        // containing an inline call.
        match &doc.nodes[0] {
            Block::Paragraph { content, .. } => {
                match &content[0] {
                    Inline::DirectiveCall { name, span, .. } => {
                        assert_eq!(name, "note");
                        assert_eq!(*span, ByteSpan::new(0, 5));
                    }
                    other => panic!("expected inline call, got {other:?}"),
                }
                assert_text(&content[1], " trailing text here");
            }
            other => panic!("expected paragraph, got {other:?}"),
        }
    }

    #[test]
    fn inline_call_in_sentence() {
        let doc = parse("See .note {x} for details.\n");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 3);
        assert_text(&content[0], "See ");
        match &content[1] {
            Inline::DirectiveCall {
                name,
                positional_args,
                span,
                ..
            } => {
                assert_eq!(name, "note");
                assert_eq!(positional_args.len(), 1);
                assert_eq!(positional_args[0], Value::Identifier("x".into()));
                assert_eq!(*span, ByteSpan::new(4, 13));
            }
            other => panic!("expected inline call, got {other:?}"),
        }
        assert_text(&content[2], " for details.");
    }

    #[test]
    fn inline_call_does_not_parse_in_numbers() {
        let doc = parse("pi is 3.14 exactly\n");
        assert_eq!(joined_text(paragraph_inlines(&doc)), "pi is 3.14 exactly");
    }

    #[test]
    fn ellipsis_is_literal_text() {
        let doc = parse("...and more\n");
        assert_eq!(joined_text(paragraph_inlines(&doc)), "...and more");
    }

    #[test]
    fn nested_call_inside_argument() {
        let doc = parse(".outer {.inner {value}}\n");
        match &doc.nodes[0] {
            Block::DirectiveCall {
                positional_args, ..
            } => {
                assert_eq!(positional_args.len(), 1);
                match &positional_args[0] {
                    Value::Content(content) => {
                        assert_eq!(content.len(), 1);
                        match &content[0] {
                            Inline::DirectiveCall {
                                name,
                                positional_args,
                                span,
                                ..
                            } => {
                                assert_eq!(name, "inner");
                                assert_eq!(positional_args.len(), 1);
                                assert_eq!(positional_args[0], Value::Identifier("value".into()));
                                assert_eq!(*span, ByteSpan::new(8, 22));
                            }
                            other => panic!("expected nested call, got {other:?}"),
                        }
                    }
                    other => panic!("expected content value, got {other:?}"),
                }
            }
            other => panic!("expected call, got {other:?}"),
        }
    }

    #[test]
    fn argument_with_markdown_is_content() {
        let doc = parse(".fn {some *text* here}\n");
        match &doc.nodes[0] {
            Block::DirectiveCall {
                positional_args, ..
            } => match &positional_args[0] {
                Value::Content(content) => {
                    assert_eq!(content.len(), 3);
                    assert_text(&content[0], "some ");
                    assert!(matches!(content[1], Inline::Emphasis { .. }));
                    assert_text(&content[2], " here");
                }
                other => panic!("expected content value, got {other:?}"),
            },
            other => panic!("expected call, got {other:?}"),
        }
    }

    #[test]
    fn malformed_calls_do_not_panic_and_fall_back_to_paragraph() {
        for input in [
            ".foo {",
            ".foo {value",
            ".foo key:{",
            ".foo key:{value",
            ".foo width:{x} {y}",
        ] {
            let doc = parse(input);
            // Recoverable: the line becomes a paragraph; no panic.
            assert!(
                matches!(doc.nodes[0], Block::Paragraph { .. }),
                "input {input:?} should fall back to paragraph"
            );
        }
    }

    #[test]
    fn malformed_unclosed_body_brace() {
        let doc = parse(".foo {\ntext\n");
        assert!(matches!(doc.nodes[0], Block::Paragraph { .. }));
        let content = paragraph_inlines(&doc);
        // The failed call is recovered as literal characters.
        assert_eq!(content.len(), 4);
        assert_text(&content[0], ".");
        assert_text(&content[1], "foo {");
        assert!(matches!(content[2], Inline::SoftBreak { .. }));
        assert_text(&content[3], "text");
    }

    #[test]
    fn dot_without_name_is_literal_text() {
        let doc = parse("like . this\n");
        let content = paragraph_inlines(&doc);
        assert_eq!(joined_text(content), "like . this");
    }

    #[test]
    fn block_call_then_blank_then_text_not_body() {
        let doc = parse(".note\n\nParagraph after\n");
        assert!(matches!(doc.nodes[0], Block::DirectiveCall { .. }));
        assert!(matches!(doc.nodes[1], Block::Paragraph { .. }));
    }

    #[test]
    fn block_call_underscore_name() {
        let doc = parse(".my_call {v}\n");
        assert!(matches!(
            &doc.nodes[0],
            Block::DirectiveCall { name, .. } if name == "my_call"
        ));
    }

    #[test]
    fn malformed_calls_produce_structured_diagnostics() {
        for (input, expected_code) in [
            (".foo {", "E2003"),
            (".foo {value", "E2003"),
            (".foo key:{", "E2003"),
            (".foo key:{value", "E2003"),
            (".foo width:{x} {y}", "E2001"),
        ] {
            let output = parse_with_diagnostics(input);
            assert_eq!(
                output.diagnostics.len(),
                1,
                "input {input:?} should yield exactly one diagnostic"
            );
            assert_eq!(output.diagnostics[0].code, expected_code, "input {input:?}");
            assert!(
                output.diagnostics[0].span.start <= output.diagnostics[0].span.end,
                "input {input:?}"
            );
            assert!(!output.diagnostics[0].message.is_empty(), "input {input:?}");
            assert!(
                matches!(output.document.nodes[0], Block::Paragraph { .. }),
                "input {input:?} should fall back to paragraph"
            );
        }
    }

    #[test]
    fn valid_calls_produce_no_diagnostics() {
        for input in [".foo {bar}\n", ".foo key:{value}\n", ".1 {item}\n"] {
            let output = parse_with_diagnostics(input);
            assert!(output.diagnostics.is_empty(), "input {input:?}");
            assert!(matches!(
                output.document.nodes[0],
                Block::DirectiveCall { .. }
            ));
        }
    }

    #[test]
    fn implicit_reference_call_at_block_level() {
        let doc = parse(".1 {item}\n");
        assert!(matches!(
            &doc.nodes[0],
            Block::DirectiveCall { name, .. } if name == "1"
        ));
    }
}
