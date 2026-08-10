//! Minimal CommonMark-compatible Markdown parser.
//!
//! Produces the Scribium AST with byte-level source spans on every node.
//! Supported constructs (M1 subset):
//!
//! - ATX headings (`#` through `######`)
//! - Paragraphs with soft/hard line breaks
//! - Emphasis (`*text*`, `_text_`) and strong (`**text**`, `__text__`)
//! - Unordered lists (`- `, `* `, `+ `) with nested lists and code blocks
//! - Ordered lists (`1. `, `1) `) with nested lists
//! - Fenced code blocks (triple backtick, optional language)
//! - Thematic breaks (`---`, `***`, `___`)
//!
//! M2 additions:
//!
//! - Inline links (`[label](destination)`) with inline markup in the label
//! - Inline code spans (`\`code\``, or any matching backtick-run length),
//!   with opaque literal contents and CommonMark line-ending and
//!   surrounding-space normalization
//! - Block quotes (`> text`, nested via `>>`), with lazy continuation of
//!   open paragraphs and inline elements spanning multiple quoted lines
//!
//! Delimiter runs of three or more identical characters (`***x***`) are
//! treated as literal text. Setext headings and images are not part of the
//! M1/M2 subset, and reference-style links are not
//! part of the M2 subset.

use super::ast::{Block, Document, FrontMatter, Inline, ListItem, Value};
use crate::source::ByteSpan;

/// Maximum block-nesting depth before a parse is flattened to paragraphs.
///
/// Guards against stack overflow on pathological input such as thousands of
/// nested list markers.
const MAX_BLOCK_DEPTH: usize = 64;

/// Minimum indentation (in leading spaces/tabs) at which a line can no
/// longer start a new block.
///
/// Per CommonMark, block-starting constructs (headings, fenced code,
/// thematic breaks, list items, block quotes, directives) may begin with at
/// most three leading spaces of indentation. A line indented by at least
/// four columns therefore can only be paragraph continuation text in this
/// parser (which has no indented code blocks). `trimmed text alone` never
/// decides block interruption here; the raw/text offset difference does.
const MIN_BLOCK_INDENT: usize = 4;

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

/// Whether `text` starts a new block rather than continuing a paragraph.
///
/// The contract matches what `SourceLine::starts_block` classifies: `text`
/// has the `indent` columns of leading whitespace already removed, and
/// `indent` is the effective indentation of the line. Passing a
/// whitespace-bearing `text` together with its indent would misclassify
/// indented headings (e.g. `"  # h"` with indent 2) as paragraph text.
/// Blockquote markers are already stripped before this check.
fn text_starts_block(text: &str, indent: usize) -> bool {
    if indent >= MIN_BLOCK_INDENT {
        // A line indented at least four columns cannot start a block.
        // It is always paragraph text / continuation (no indented code
        // blocks in this parser).
        return false;
    }
    is_heading_text(text).is_some()
        || fence_length(text).is_some()
        || is_thematic_break(text)
        || is_list_marker(text).is_some()
        || is_ordered_list_marker(text).is_some()
        || is_block_directive_line(text)
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
        text_starts_block(self.text, self.indent()) || is_blockquote_marker(self.raw)
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
        if let Some((start, _delimiter)) = is_ordered_list_marker(line.text) {
            if depth < MAX_BLOCK_DEPTH {
                blocks.push(parse_ordered_list(
                    source,
                    lines,
                    cursor,
                    start,
                    depth,
                    diagnostics,
                ));
                continue;
            }
        }
        if is_block_directive_line(line.text) {
            blocks.push(parse_directive_block(
                source,
                lines,
                cursor,
                depth,
                diagnostics,
            ));
            continue;
        }
        if is_blockquote_marker(line.raw) && depth < MAX_BLOCK_DEPTH {
            blocks.push(parse_blockquote(source, lines, cursor, depth, diagnostics));
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
        Block::OrderedList { span, .. } => *span,
        Block::CodeBlock { span, .. } => *span,
        Block::ThematicBreak { span } => *span,
        Block::BlankLine { span } => *span,
        Block::DirectiveCall { span, .. } => *span,
        Block::Metadata { span, .. } => *span,
        Block::BlockQuote { span, .. } => *span,
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
/// Map an offset in the logical (joined) paragraph buffer onto the original
/// source. Each collected line maps onto segment `(buffer_start,
/// source_start)`; the offset must lie within the buffer. Span *starts* are
/// inclusive of the byte at the offset, so a start on a segment boundary
/// belongs to the following segment.
fn translate_start(offset: usize, segments: &[(usize, usize)]) -> usize {
    let idx = segments.partition_point(|&(buffer_start, _)| buffer_start <= offset) - 1;
    segments[idx].1 + (offset - segments[idx].0)
}

/// Map a span *end* offset: exclusive, so an end falling on a segment
/// boundary terminates the byte before that boundary, i.e. the preceding
/// segment (a soft/hard break ending at a line terminator stays inside the
/// terminator's own segment).
fn translate_end(offset: usize, segments: &[(usize, usize)]) -> usize {
    let idx = segments.partition_point(|&(buffer_start, _)| buffer_start < offset);
    let idx = idx.saturating_sub(1);
    segments[idx].1 + (offset - segments[idx].0)
}

/// Translate a span over the logical buffer into original source offsets.
fn translate_span(span: ByteSpan, segments: &[(usize, usize)]) -> ByteSpan {
    ByteSpan::new(
        translate_start(span.start, segments),
        translate_end(span.end, segments),
    )
}

/// Recursively translate every inline span (including spans inside emphasis,
/// strong, directive bodies/arguments, and link labels) into original source
/// offsets.
fn remap_inline_spans(inlines: &mut [Inline], segments: &[(usize, usize)]) {
    for inline in inlines {
        match inline {
            Inline::Text { span, .. }
            | Inline::HardBreak { span }
            | Inline::SoftBreak { span }
            | Inline::Code { span, .. } => *span = translate_span(*span, segments),
            Inline::Emphasis { content, span } | Inline::Strong { content, span } => {
                remap_inline_spans(content, segments);
                *span = translate_span(*span, segments);
            }
            Inline::Link { content, span, .. } => {
                remap_inline_spans(content, segments);
                *span = translate_span(*span, segments);
            }
            Inline::DirectiveCall {
                positional_args,
                named_args,
                body,
                span,
                ..
            } => {
                for arg in positional_args
                    .iter_mut()
                    .chain(named_args.iter_mut().map(|(_, value)| value))
                {
                    if let Value::Content(inlines) = arg {
                        remap_inline_spans(inlines, segments);
                    }
                }
                if let Some(body) = body {
                    remap_inline_spans(body, segments);
                }
                *span = translate_span(*span, segments);
            }
        }
    }
}

fn parse_paragraph(
    source: &str,
    lines: &[SourceLine<'_>],
    cursor: &mut usize,
    diagnostics: &mut Vec<ParserDiagnostic>,
) -> Block {
    let start_idx = *cursor;
    let first = &lines[start_idx];
    let mut last_idx = start_idx;
    loop {
        *cursor += 1;
        if *cursor >= lines.len() || lines[*cursor].is_blank() || lines[*cursor].starts_block() {
            break;
        }
        last_idx = *cursor;
    }
    let last = &lines[last_idx];

    // Check if the paragraph lines are contiguous in the source.
    // For regular paragraphs, consecutive lines are contiguous (line[i].end == line[i+1].raw_start).
    // For blockquote content (after stripping '>' markers), lines are NOT contiguous.
    // In the non-contiguous case, parse each line's inlines separately and join with soft breaks.
    let mut contiguous = true;
    for i in start_idx..last_idx {
        let current_end = lines[i].end;
        let next_start = lines[i + 1].raw_start;
        if current_end != next_start {
            contiguous = false;
            break;
        }
    }

    let content = if contiguous {
        // Fast path: lines are contiguous in source, parse as single inline span.
        parse_inlines(source, first.text_start, last.content_end(), 0, diagnostics)
    } else {
        // Slow path: lines are not contiguous (e.g., blockquote content).
        // The fragments are joined into a single logical buffer so the
        // inline parser sees the same newline semantics as an unquoted
        // paragraph (two-space HardBreak, backslash HardBreak, SoftBreak)
        // and inline elements may span multiple lines: code spans,
        // emphasis/strong, and link labels behave exactly as in a plain
        // paragraph. The final line excludes the terminator, otherwise a
        // trailing soft break would be emitted. Every resulting span, and
        // any diagnostic raised during this synthetic parse, is translated
        // back onto the original source via the segment table so public
        // spans never reference the buffer.
        let mut buffer = String::new();
        let mut segments = Vec::with_capacity(last_idx + 1 - start_idx);
        for (i, line) in lines.iter().enumerate().take(last_idx + 1).skip(start_idx) {
            // The paragraph head starts at `text_start`: its indentation is
            // block syntax, not inline content. Continuation lines may carry
            // whitespace that *is* inline content (e.g. the text of a code
            // span opening on an earlier line): the container parser only
            // stripped the container syntax, so later segments must start at
            // `raw_start` to preserve that whitespace.
            let segment_start = if i == start_idx {
                line.text_start
            } else {
                line.raw_start
            };
            let end = if i < last_idx {
                line.end
            } else {
                line.content_end()
            };
            segments.push((buffer.len(), segment_start));
            buffer.push_str(&source[segment_start..end]);
        }
        let diag_start = diagnostics.len();
        let mut content = parse_inlines(&buffer, 0, buffer.len(), 0, diagnostics);
        for diagnostic in &mut diagnostics[diag_start..] {
            diagnostic.span = translate_span(diagnostic.span, &segments);
        }
        remap_inline_spans(&mut content, &segments);
        content
    };

    Block::Paragraph {
        content,
        span: ByteSpan::new(first.text_start, last.end),
    }
}

/// Marker depth of a stripped content line (number of leading block quote
/// markers, mirroring how `parse_blockquote` strips one `>` marker plus the
/// optional single space after it per nesting level), whether the line is
/// effectively a blank line at that depth, and the remaining content after
/// all markers are stripped.
fn split_markers(raw: &str) -> (usize, bool, &str) {
    let mut depth = 0;
    let mut rest = raw;
    while let Some(pos) = find_blockquote_marker(rest) {
        depth += 1;
        let after_marker = pos + 1;
        rest = if after_marker < rest.len() && rest.as_bytes()[after_marker] == b' ' {
            &rest[after_marker + 1..]
        } else {
            &rest[after_marker..]
        };
    }
    (
        depth,
        rest.trim_matches(|b| b == ' ' || b == '\t').is_empty(),
        rest,
    )
}

/// Effective classified view of a collected content remainder: `strip_cols`
/// columns are removed from its front (mirroring how the list parsers strip
/// item-content lines via `strip_indent`), then the remaining leading
/// whitespace is stripped. The result is the `(text, indent)` pair that
/// `text_starts_block` classifies, identical to what `SourceLine` derives
/// for the same content.
fn effective_remainder_view(rest: &str, strip_cols: usize) -> (&str, usize) {
    let strip = strip_cols.min(rest.len());
    let after = &rest[strip..];
    let ws = after.len() - after.trim_start_matches([' ', '\t']).len();
    (&after[ws..], ws)
}

/// Whether the stripped content of a collected line starts a new block at
/// its effective depth rather than being paragraph continuation text.
///
/// The classification receives exactly what `SourceLine::starts_block` sees
/// for the same line: leading whitespace is stripped from the remainder and
/// the stripped column count becomes the effective indentation.
fn remainder_starts_block(raw: &str) -> bool {
    let (_, _, rest) = split_markers(raw);
    let (text, indent) = effective_remainder_view(rest, 0);
    text_starts_block(text, indent)
}

/// Item-relative classification of a collected content line at or beyond
/// the trailing list item's content column: `content_col` columns of raw
/// are removed exactly like the list parsers strip item-content lines
/// (`strip_indent`), and the stripped line is classified with the same
/// rules the nested `parse_blocks` pass applies (including a block-quote
/// marker for nested quotes opened inside the item).
fn item_relative_starts_block(raw: &str, content_col: usize) -> bool {
    let (text, indent) = effective_remainder_view(raw, content_col);
    text_starts_block(text, indent) || is_blockquote_marker(&raw[content_col.min(raw.len())..])
}

/// Incremental lazy-continuation state for the `parse_blockquote` collection
/// loop, maintained purely from the collected content lines.
///
/// A marker-less candidate line is accepted as a lazy paragraph continuation
/// iff the trailing container chain ends in an open paragraph that is still
/// being fed by the collected lines:
///
/// * at least one non-blank content line has been collected (`has_feed`),
/// * the last collected line is not blank at any depth (`trailing_blank`),
/// * the trailing leaf is a paragraph: the deepest content line either
///   reaches the deepest active depth and is plain text, or is depth-0 line
///   joining an open chain as lazy continuation rather than starting a fresh
///   sibling block (`leaf_starts_block`),
/// * no fenced code block is open: fence content lines look like plain text
///   but never carry a paragraph (`fence_open`).
///
/// Container-depth transitions:
///
/// * lines deeper than the active leaf enter a deeper container and
///   redefine the leaf (`depth >= 1`),
/// * a marker-less line preserves the deeper chain only while the deeper
///   leaf is an open paragraph (CommonMark 250/251); when the deeper leaf
///   ended in a non-paragraph block (heading, list, fence), the line leaves
///   the deeper containers and opens a paragraph at the shallower depth,
/// * a fence is owned by the container at its marker depth: a shallower
///   line ends that container and the fence state dies with it,
/// * a trailing list item is tracked separately: lines at or beyond the
///   item's content column are item content (the list parsers absorb them
///   as continuation, whether or not they start a new marker), classified
///   in item-relative coordinates with fences opened by such lines tracked
///   on the item, while lines shallower than the content column end the
///   list and start a fresh block at their own depth.
#[derive(Default)]
struct QuoteContinuation {
    has_feed: bool,
    leaf_depth: usize,
    leaf_paragraph: bool,
    trailing_blank: bool,
    fence_open: Option<(usize, usize)>,
    list_item: Option<ListItemTrailing>,
}

/// The trailing open list item of the collected content, mirroring the
/// list parsers: for unordered lists the content column is fixed on the
/// first item, for ordered lists it is recomputed per item.
#[derive(Default)]
struct ListItemTrailing {
    /// Unordered marker (`-`/`*`/`+`) or the ordered delimiter (`.`/`)`).
    marker: u8,
    /// Whether the item belongs to an ordered list (the content column
    /// is recomputed per item in that case).
    ordered: bool,
    /// Column (relative to the quote content) at which item content
    /// starts; continuation lines must be indented at least to this
    /// column.
    content_col: usize,
    /// Length of a fence opened by the item's blocks, if any; while set,
    /// item content is fence content and never opens a paragraph.
    fence: Option<usize>,
}

impl QuoteContinuation {
    fn record(&mut self, raw: &str) {
        let (depth, blank, rest) = split_markers(raw);
        let indent = rest.len() - rest.trim_start_matches([' ', '\t']).len();
        let text = &rest[indent..];

        // While a fence is open, every following line is fence content
        // (blanks included) and opens no paragraph: the continuation state
        // stays frozen until a structural closer ends the fence. A closer
        // is itself the trailing block, so it leaves no open paragraph.
        //
        // The fence is owned by the container at its marker depth: a line
        // at a shallower depth means that container has ended, and the
        // unclosed fence dies with it instead of blocking the shallower
        // chain (leaving a container never leaves an open paragraph).
        if let Some((fence_len, fence_depth)) = self.fence_open {
            if depth < fence_depth {
                self.fence_open = None;
            } else if indent < MIN_BLOCK_INDENT
                && depth == fence_depth
                && is_closing_fence(text, fence_len)
            {
                self.fence_open = None;
                self.leaf_depth = depth;
                self.leaf_paragraph = false;
                self.has_feed = true;
                self.trailing_blank = false;
                return;
            } else {
                return;
            }
        }

        if blank {
            self.trailing_blank = true;
            return;
        }
        let after_blank = self.trailing_blank;
        self.trailing_blank = false;
        self.has_feed = true;

        // Structural lines that start a list item. Only lines with fewer
        // than `MIN_BLOCK_INDENT` columns of indent are candidates,
        // exactly as in the block classification behind the list parsers.
        let list_kind = if indent < MIN_BLOCK_INDENT {
            is_ordered_list_marker(text)
                .map(|(_, delimiter)| (true, delimiter))
                .or_else(|| is_list_marker(text).map(|marker| (false, marker)))
        } else {
            None
        };

        if let Some((ordered, marker)) = list_kind {
            // Marker position and the whitespace after it, mirroring the
            // content-column rules of `parse_list` / `parse_ordered_list`
            // (for unordered lists the marker sits at column 0).
            let marker_pos = if ordered {
                text.bytes().position(|b| b == marker).unwrap_or(0)
            } else {
                0
            };
            let ws_after = text[marker_pos + 1..]
                .bytes()
                .take_while(|b| *b == b' ' || *b == b'\t')
                .count();
            let content_col = indent + marker_pos + 1 + ws_after;
            let remainder = &text[marker_pos + 1 + ws_after..];

            if let Some(item) = &self.list_item {
                if indent >= item.content_col {
                    // At or beyond the content column the list parsers
                    // consume the line as content of the *current* item in
                    // item-relative coordinates: `strip_indent` removes the
                    // item's content column and the nested parse
                    // classifies the rest. The item stays open and the leaf
                    // follows the stripped line (fences it opens are
                    // tracked on the item).
                    self.record_item_content_line(raw, depth);
                    return;
                }
                if item.marker == marker && item.ordered == ordered {
                    // A new item of the same list. Unordered lists keep
                    // the first item's content column, ordered lists
                    // recompute it per item.
                    let content_col = if ordered {
                        content_col
                    } else {
                        item.content_col
                    };
                    let fence = fence_length(remainder);
                    self.list_item = Some(ListItemTrailing {
                        marker,
                        ordered,
                        content_col,
                        fence,
                    });
                    self.leaf_depth = depth;
                    self.leaf_paragraph = fence.is_none() && !text_starts_block(remainder, 0);
                    return;
                }
            }

            // No trailing list, or a different marker at a sibling
            // position: a fresh list opens at this line and becomes the
            // trailing leaf (a non-paragraph block unless the item's own
            // first block is an open paragraph).
            let fence = fence_length(remainder);
            self.list_item = Some(ListItemTrailing {
                marker,
                ordered,
                content_col,
                fence,
            });
            self.leaf_depth = depth;
            self.leaf_paragraph = fence.is_none() && !text_starts_block(remainder, 0);
            return;
        }

        // Plain (marker-less) line.
        if let Some(item) = &self.list_item {
            if indent >= item.content_col {
                // Item content in item-relative coordinates (see above): a
                // fence this line opens is tracked on the item and the
                // leaf follows the stripped line.
                self.record_item_content_line(raw, depth);
                return;
            }
            // Too shallow for the item: the list ends here and the line
            // opens a fresh block at its own depth.
            self.list_item = None;
        }

        // A fence-opening line replaces whatever came before: the fence
        // is now the trailing block and absorbs all following lines.
        // Fences opened inside a list item are tracked on the item
        // itself and handled above.
        if indent < MIN_BLOCK_INDENT {
            if let Some(fence_len) = fence_length(text) {
                self.fence_open = Some((fence_len, depth));
                self.leaf_paragraph = false;
            }
        }

        // Container-depth transitions of the active leaf. `depth` is the
        // marker depth of the current line relative to the quote being
        // collected.
        if depth >= 1 {
            // The line is structurally anchored to the container at
            // `depth` (it still carries markers). Entering a deeper
            // container, or a deeper-or-equal line after the previous leaf
            // ended, always redefines the active leaf: a marker-carrying
            // line can never lazily omit the markers of the chain below it.
            self.leaf_depth = depth;
            self.leaf_paragraph = !remainder_starts_block(raw);
        } else {
            // Marker-less line: it is either lazy continuation text of the
            // deeper chain or the head of a new block at the top of the
            // quote.
            let starts = remainder_starts_block(raw);
            let continue_deeper =
                !after_blank && self.leaf_depth > 0 && self.leaf_paragraph && !starts;
            if continue_deeper {
                // The deeper leaf is still an open paragraph being fed, no
                // blank line terminated the chain, and this line cannot
                // start a block: keep the deeper chain (CommonMark 250/251).
            } else {
                // Leaving the deeper containers: the deeper leaf ended in a
                // non-paragraph block (heading/list/fence), a blank line
                // terminated the chain, or the line starts a block of its
                // own. Either way the line opens a fresh block/paragraph at
                // the current (shallower) depth, discarding the stale
                // deeper leaf.
                self.leaf_depth = 0;
                self.leaf_paragraph = !starts;
            }
        }
    }

    fn can_lazy_continue(&self, indent: usize) -> bool {
        if !self.has_feed || self.trailing_blank || self.fence_open.is_some() {
            return false;
        }
        match &self.list_item {
            Some(item) => self.leaf_paragraph && indent >= item.content_col,
            None => self.leaf_paragraph,
        }
    }

    /// Advance the trailing list item on a line the list parsers absorb as
    /// item content (indent at or beyond the item's content column). The
    /// line is classified in item-relative coordinates (exactly like the
    /// nested `parse_blocks` pass sees it after `strip_indent`): fences it
    /// opens are tracked on the item, and fence content never opens a
    /// paragraph.
    fn record_item_content_line(&mut self, raw: &str, depth: usize) {
        self.leaf_depth = depth;
        let item = self.list_item.as_mut().unwrap();
        let (relative_text, _) = effective_remainder_view(raw, item.content_col);
        if let Some(fence_len) = item.fence {
            if is_closing_fence(relative_text, fence_len) {
                item.fence = None;
            }
            self.leaf_paragraph = false;
        } else {
            item.fence = fence_length(relative_text);
            self.leaf_paragraph =
                item.fence.is_none() && !item_relative_starts_block(raw, item.content_col);
        }
    }
}

/// Parse a block quote starting at the cursor.
///
/// A block quote consists of consecutive lines starting with `>` optionally
/// followed by a space. The `>` marker is stripped and the remaining content
/// is parsed as nested blocks. Blank lines within a block quote (lines with
/// only `>` or `> `) are preserved as blank lines in the content.
///
/// Per CommonMark:
/// - Up to 3 spaces of indentation are allowed before the `>` marker.
/// - A blank line containing only the block quote marker (or marker + space)
///   does not end the block quote but separates paragraphs within it.
/// - An unquoted blank line ends the block quote.
/// - Lazy continuation: non-blockquote lines that follow an open paragraph
///   inside a block quote (including paragraphs nested in deeper quotes) are
///   treated as continuations of that paragraph.
fn parse_blockquote(
    source: &str,
    lines: &[SourceLine<'_>],
    cursor: &mut usize,
    depth: usize,
    diagnostics: &mut Vec<ParserDiagnostic>,
) -> Block {
    let start_line = *cursor;
    let first = &lines[start_line];
    let mut content_lines = Vec::new();
    let mut end_span = first.end;
    // Incremental state over the collected content lines deciding whether a
    // marker-less candidate line may still lazily continue the open
    // paragraph chain (see `QuoteContinuation`).
    let mut continuation = QuoteContinuation::default();

    // Collect all consecutive blockquote lines and lazy continuations
    while *cursor < lines.len() {
        let line = &lines[*cursor];

        // Check for blockquote marker in the raw line (including leading whitespace)
        if let Some(marker_pos) = find_blockquote_marker(line.raw) {
            // This is a blockquote marker line. Strip the `>` marker and
            // the optional single space after it, then classify the
            // remaining content: a quoted blank line (marker whose
            // remaining content is empty or whitespace-only) is recorded
            // as a blank line inside the quote so no marker-less line may
            // lazily continue across it, while real content lines feed the
            // continuation state.

            // Calculate content start: after marker, skip optional single space
            let content_start = marker_pos + 1;
            let has_space_after_marker =
                content_start < line.raw.len() && line.raw.as_bytes()[content_start] == b' ';
            let content_raw = if has_space_after_marker {
                &line.raw[content_start + 1..]
            } else {
                &line.raw[content_start..]
            };

            // Calculate absolute offsets in the original source
            // If we skipped a space after marker, add 1 more to the offset
            let content_raw_start =
                line.raw_start + content_start + if has_space_after_marker { 1 } else { 0 };

            // text is content_raw with leading whitespace stripped
            let ws = content_raw
                .bytes()
                .take_while(|b| *b == b' ' || *b == b'\t')
                .count();
            let content_text = &content_raw[ws..];
            let content_text_start = content_raw_start + ws;

            if content_text.is_empty() {
                // Quoted blank line (e.g. ">", "> ", ">   "): keep the
                // quote open but record that its content currently ends
                // with a blank line.
                content_lines.push(SourceLine {
                    raw: "",
                    text: "",
                    raw_start: content_raw_start,
                    text_start: content_text_start,
                    term: line.term,
                    end: line.end,
                });
                continuation.record("");
            } else {
                // Real content line feeds the continuation state.
                continuation.record(content_raw);
                content_lines.push(SourceLine {
                    raw: content_raw,
                    text: content_text,
                    raw_start: content_raw_start,
                    text_start: content_text_start,
                    term: line.term,
                    end: line.end,
                });
            }
            end_span = line.end;
            *cursor += 1;
        } else if line.is_blank() {
            // Unquoted blank line: ends the block quote. A quoted blank
            // line always carries a marker and is handled above.
            break;
        } else if continuation.can_lazy_continue(line.indent())
            && (line.indent() >= MIN_BLOCK_INDENT || !line.starts_block())
        {
            // Lazy continuation: a non-blockquote, non-blank line is kept
            // inside the quote only when the currently open block is a real
            // Paragraph and the line cannot start a new block. Indentation
            // participates in the classification: at least `MIN_BLOCK_INDENT`
            // leading spaces means the line cannot interrupt the paragraph
            // (CommonMark Example 238 preserves "    - bar" as text), even
            // though its trimmed text looks like a list marker.
            content_lines.push(SourceLine {
                raw: line.raw,
                text: line.text,
                raw_start: line.raw_start,
                text_start: line.text_start,
                term: line.term,
                end: line.end,
            });
            continuation.record(line.raw);
            end_span = line.end;
            *cursor += 1;
        } else {
            // Non-blockquote line and not in a paragraph (or starts a new block):
            // ends the block quote
            break;
        }
    }

    // Parse the content lines as blocks
    let mut content_cursor = 0;
    let content = parse_blocks(
        source,
        &content_lines,
        &mut content_cursor,
        depth + 1,
        diagnostics,
    );

    Block::BlockQuote {
        content,
        span: ByteSpan::new(first.raw_start, end_span),
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

/// Parse an ordered list starting at the cursor.
/// `start` is the ordinal of the first item.
/// The delimiter ('.' or ')') must be consistent across all items.
fn parse_ordered_list(
    source: &str,
    lines: &[SourceLine<'_>],
    cursor: &mut usize,
    start: usize,
    depth: usize,
    diagnostics: &mut Vec<ParserDiagnostic>,
) -> Block {
    let mut items = Vec::new();
    // Get the delimiter from the first item
    let first = &lines[*cursor];
    let first_marker = is_ordered_list_marker(first.text).expect("first item must have marker");
    let delimiter = first_marker.1;

    while *cursor < lines.len() {
        let line = &lines[*cursor];
        // Check if this line starts an ordered list item with the same delimiter
        let Some((_ordinal, marker)) = is_ordered_list_marker(line.text) else {
            break;
        };
        if marker != delimiter {
            // Different delimiter - end of this ordered list
            break;
        }
        // Allow any ordinal - only first item's ordinal determines start
        // (Markdown doesn't require sequential source ordinals)
        let item_start = line.raw_start;
        let mut item_lines = Vec::new();
        // Find marker position for this line
        let line_marker_pos = line.text.bytes().position(|b| b == delimiter).unwrap_or(0);
        let ws_after_marker = line.text[line_marker_pos + 1..]
            .bytes()
            .take_while(|b| *b == b' ' || *b == b'\t')
            .count();
        // Calculate content column for THIS item based on its marker width
        let item_content_col = line.indent() + line_marker_pos + 1 + ws_after_marker;
        item_lines.push(SourceLine {
            raw: &line.text[line_marker_pos + 1 + ws_after_marker..],
            text: &line.text[line_marker_pos + 1 + ws_after_marker..],
            raw_start: line.text_start + line_marker_pos + 1 + ws_after_marker,
            text_start: line.text_start + line_marker_pos + 1 + ws_after_marker,
            term: line.term,
            end: line.end,
        });
        *cursor += 1;
        loop {
            if *cursor >= lines.len() {
                break;
            }
            let next = &lines[*cursor];
            // Use the current item's content column for continuation/nested content
            if next.indent() >= item_content_col {
                item_lines.push(strip_indent(next, item_content_col));
                *cursor += 1;
            } else if next.is_blank() {
                let mut lookahead = *cursor + 1;
                while lookahead < lines.len() && lines[lookahead].is_blank() {
                    lookahead += 1;
                }
                if lookahead < lines.len()
                    && (lines[lookahead].indent() >= item_content_col
                        || is_ordered_list_marker(lines[lookahead].text)
                            .map(|(_, m)| m == delimiter)
                            .unwrap_or(false)
                        || is_list_marker(lines[lookahead].text).is_some())
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
    Block::OrderedList {
        items,
        start,
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

/// Whether the line is an *isolated* block-level Quarkdown function call:
/// a syntactically valid call that consumes the entire meaningful content
/// of the line (trailing horizontal whitespace allowed).
///
/// The actual parse is performed again by `parse_directive_block`; this
/// pre-flight must agree with that parse so that a line with trailing
/// inline content (`.note {x} continues`) is never misclassified as a block
/// and does not terminate the surrounding paragraph. The call is parsed on
/// the line-local slice, so reported spans are already line-relative. The
/// parse may report an error; that error is intentionally dropped here and
/// re-reported by the inline fallback, which parses the line exactly once.
fn is_block_directive_line(text: &str) -> bool {
    match crate::syntax::quarkdown::parse_directive_at(text, 0) {
        Ok(Some((_, consumed))) => text[consumed..].bytes().all(|b| b == b' ' || b == b'\t'),
        _ => false,
    }
}

/// Whether the line starts a list item with the given marker.
fn is_item_start(text: &str, marker: u8) -> bool {
    let bytes = text.as_bytes();
    bytes.first() == Some(&marker) && bytes.len() > 1 && matches!(bytes[1], b' ' | b'\t')
}

/// Whether the line starts a block quote (`>` followed by optional space).
/// Returns the byte offset of the `>` marker in the original line text (including leading whitespace),
/// or None if not a block quote marker.
/// Per CommonMark, up to 3 spaces of indentation are allowed before the `>`.
fn find_blockquote_marker(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    // Check up to 3 leading spaces
    let mut i = 0;
    while i < bytes.len() && i < 3 && bytes[i] == b' ' {
        i += 1;
    }
    if i < bytes.len() && bytes[i] == b'>' {
        Some(i)
    } else {
        None
    }
}

/// Whether the line starts a block quote (`>` followed by optional space).
/// Uses `find_blockquote_marker` to check the raw line text (with leading whitespace).
fn is_blockquote_marker(text: &str) -> bool {
    find_blockquote_marker(text).is_some()
}

/// The starting ordinal and delimiter if the line starts an ordered list item (N. or N)).
/// Returns (ordinal, delimiter_byte) if valid, None otherwise.
/// Enforces 1-9 digit limit per CommonMark.
fn is_ordered_list_marker(text: &str) -> Option<(usize, u8)> {
    let bytes = text.as_bytes();
    if bytes.is_empty() || !bytes[0].is_ascii_digit() {
        return None;
    }
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == 0 || i >= bytes.len() {
        return None;
    }
    // Enforce 1-9 digit limit
    if i > 9 {
        return None;
    }
    // Must be followed by '.' or ')' and then space or tab
    let marker = bytes[i];
    if marker != b'.' && marker != b')' {
        return None;
    }
    if i + 1 >= bytes.len() {
        return None;
    }
    if !matches!(bytes[i + 1], b' ' | b'\t') {
        return None;
    }
    // Parse the ordinal
    let ordinal_str = std::str::from_utf8(&bytes[..i]).ok()?;
    ordinal_str.parse::<usize>().ok().map(|ord| (ord, marker))
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
                b'[' => self.parse_link(&mut inlines),
                b'`' => self.parse_code_span(&mut inlines),
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

    /// Parse a Markdown inline link (`[label](destination)`), falling back
    /// to literal text when the input is not a valid link.
    ///
    /// The supported subset is deliberately small and deterministic:
    ///
    /// - The label runs from `[` to the *first* `]`; it is parsed with the
    ///   regular inline parser, so emphasis, strong text, and Quarkdown
    ///   inline calls work inside it. Nested brackets in the label are not
    ///   supported.
    /// - The destination runs from `(` to the first matching `)`, allowing
    ///   balanced parentheses inside (`[x](a(b)c)`). It must be non-empty
    ///   and free of ASCII whitespace and control characters; destinations
    ///   containing whitespace (such as `[text]( )` or a link title in
    ///   `[text](url "title")`) are not links and fall back to literal text.
    /// - Link titles, reference links (`[x][id]`, `[id]: url`), autolinks,
    ///   and images are not part of the subset.
    ///
    /// Malformed input (`[text](`, `[text]`, `[](url)`, ...) is recovered
    /// by emitting the `[` as literal text; no diagnostic is produced.
    fn parse_link(&mut self, inlines: &mut Vec<Inline>) {
        let start = self.pos;
        let bytes = self.bytes();
        if start > 0 && bytes[start - 1] == b'!' {
            // `![...]` is image syntax, which is not supported: keep the
            // whole thing as literal text.
            self.literal_bracket(start, inlines);
            return;
        }
        if self.depth >= MAX_INLINE_DEPTH {
            self.literal_bracket(start, inlines);
            return;
        }
        let Some(close) = bytes[start + 1..self.end].iter().position(|&b| b == b']') else {
            self.literal_bracket(start, inlines);
            return;
        };
        let close = start + 1 + close;
        if close == start + 1 || close + 1 >= self.end || bytes[close + 1] != b'(' {
            self.literal_bracket(start, inlines);
            return;
        }
        let dest_start = close + 2;
        let mut depth = 0usize;
        let mut dest_end = None;
        for (i, &b) in bytes.iter().enumerate().take(self.end).skip(dest_start) {
            match b {
                b'(' => depth += 1,
                b')' if depth == 0 => {
                    dest_end = Some(i);
                    break;
                }
                b')' => depth -= 1,
                _ => {}
            }
        }
        let Some(dest_end) = dest_end else {
            self.literal_bracket(start, inlines);
            return;
        };
        let destination = &self.source[dest_start..dest_end];
        if destination.is_empty()
            || destination
                .bytes()
                .any(|b| b.is_ascii_whitespace() || b.is_ascii_control())
        {
            self.literal_bracket(start, inlines);
            return;
        }
        let content = InlineParser::new(
            self.source,
            start + 1,
            close,
            self.depth + 1,
            self.diagnostics,
        )
        .parse();
        inlines.push(Inline::Link {
            content,
            destination: destination.to_string(),
            span: ByteSpan::new(start, dest_end + 1),
        });
        self.pos = dest_end + 1;
    }

    fn literal_bracket(&mut self, start: usize, inlines: &mut Vec<Inline>) {
        inlines.push(Inline::Text {
            content: "[".to_string(),
            span: ByteSpan::new(start, start + 1),
        });
        self.pos = start + 1;
    }

    /// Parse a Markdown code span (`` `code` ``), falling back to literal
    /// text when the input is not a valid code span.
    ///
    /// The supported subset follows CommonMark:
    ///
    /// - The opening delimiter is a run of one or more backticks; the
    ///   closing delimiter must be a run of *exactly* the same length.
    ///   Runs of a different length do not close the span.
    /// - The contents are opaque: no Markdown or Quarkdown syntax inside
    ///   is parsed (no emphasis, strong text, links, calls, or breaks).
    /// - Line endings inside the span become ordinary spaces.
    /// - If the content begins and ends with an ASCII space but is not
    ///   composed entirely of spaces, exactly one leading and one trailing
    ///   space is removed.
    ///
    /// Malformed input (no closer of the same run length) is recovered
    /// deterministically by emitting the whole opening run as literal
    /// text; no diagnostic is produced.
    fn parse_code_span(&mut self, inlines: &mut Vec<Inline>) {
        let start = self.pos;
        let bytes = self.bytes();
        let opener_len = count_backticks(bytes, start, self.end);
        match find_closing_run(bytes, start + opener_len, opener_len, self.end) {
            Some(close_start) => {
                let raw_content = &self.source[start + opener_len..close_start];
                inlines.push(Inline::Code {
                    content: normalize_code_content(raw_content),
                    span: ByteSpan::new(start, close_start + opener_len),
                });
                self.pos = close_start + opener_len;
            }
            None => {
                let run_end = start + opener_len;
                inlines.push(Inline::Text {
                    content: self.source[start..run_end].to_string(),
                    span: ByteSpan::new(start, run_end),
                });
                self.pos = run_end;
            }
        }
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
            if b == b'*' || b == b'_' || b == b'\n' || b == b'[' || b == b'`' {
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

/// Whether the dot at `pos` can start a function call: the byte after it is
/// a valid name start, and the byte before it satisfies the shared tight-call
/// boundary rule (no word character and no other dot: `3.14`, `foo.bar`,
/// `...` and `한.note` do not start calls).
fn is_call_dot(bytes: &[u8], pos: usize, end: usize) -> bool {
    if pos + 1 >= end {
        return false;
    }
    let next = bytes[pos + 1];
    if !(next.is_ascii_alphabetic() || next == b'_' || matches!(next, b'1'..=b'9')) {
        return false;
    }
    crate::syntax::quarkdown::has_valid_call_boundary(bytes, pos)
}

/// Number of consecutive backticks starting at `from`.
fn count_backticks(bytes: &[u8], from: usize, end: usize) -> usize {
    let mut n = 0;
    while from + n < end && bytes[from + n] == b'`' {
        n += 1;
    }
    n
}

/// Find the start of a backtick run of *exactly* `len` at or after `from`.
///
/// Runs of a different length are skipped over without matching, so a
/// candidate closer can never close a span opened with a different
/// delimiter length. The scan is linear in the slice length.
fn find_closing_run(bytes: &[u8], from: usize, len: usize, end: usize) -> Option<usize> {
    let mut i = from;
    while i < end {
        if bytes[i] != b'`' {
            i += 1;
            continue;
        }
        let run = count_backticks(bytes, i, end);
        if run == len {
            return Some(i);
        }
        i += run;
    }
    None
}

/// Normalize the raw contents of a code span per CommonMark:
///
/// 1. Line endings (`\n`, `\r\n`, lone `\r`) become ordinary spaces.
/// 2. If the content begins and ends with an ASCII space but is not
///    composed entirely of spaces, exactly one leading and one trailing
///    space is removed. NBSP and other Unicode whitespace are untouched.
fn normalize_code_content(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\n' => out.push(b' '),
            b'\r' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
                    i += 1;
                }
                out.push(b' ');
            }
            b => out.push(b),
        }
        i += 1;
    }
    let mut content = String::from_utf8(out).expect("code span content stays valid UTF-8");
    let all_spaces = content.bytes().all(|b| b == b' ');
    if !all_spaces && content.starts_with(' ') && content.ends_with(' ') {
        content.remove(0);
        content.pop();
    }
    content
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

    /// Concatenate all text in a flat list of inline nodes, rendering
    /// soft/hard breaks as `\n`, links in their source form
    /// (`[label](destination)`), and code spans as `` `content` ``.
    /// Panics when any other inline kind appears, so tests can use it to
    /// assert that a span contains only prose.
    fn joined_text(inlines: &[Inline]) -> String {
        let mut out = String::new();
        for inline in inlines {
            match inline {
                Inline::Text { content, .. } => out.push_str(content),
                Inline::SoftBreak { .. } | Inline::HardBreak { .. } => out.push('\n'),
                Inline::Link {
                    content,
                    destination,
                    ..
                } => {
                    out.push('[');
                    out.push_str(&joined_text(content));
                    out.push_str("](");
                    out.push_str(destination);
                    out.push(')');
                }
                Inline::Code { content, .. } => {
                    out.push('`');
                    out.push_str(content);
                    out.push('`');
                }
                other => panic!("expected Text, break, link, or code span, got {other:?}"),
            }
        }
        out
    }

    /// Whether a link node appears anywhere in the inline tree, including
    /// inside emphasis/strong content and directive bodies.
    fn contains_link(inlines: &[Inline]) -> bool {
        inlines.iter().any(|inline| match inline {
            Inline::Link { .. } => true,
            Inline::Emphasis { content, .. } | Inline::Strong { content, .. } => {
                contains_link(content)
            }
            Inline::DirectiveCall {
                body: Some(body), ..
            } => contains_link(body),
            _ => false,
        })
    }

    /// Flattened inline tree where each element is the source slice of the
    /// element's own span (absolute in its document's source), tagged with
    /// its kind, so two documents can be compared structurally and
    /// span-wise at once.
    fn inline_span_profile(inlines: &[Inline], source: &str) -> Vec<(&'static str, String)> {
        let mut out = Vec::new();
        for inline in inlines {
            match inline {
                Inline::Text { span, .. } => {
                    out.push(("text", source[span.start..span.end].to_string()));
                }
                Inline::SoftBreak { span } => {
                    out.push(("soft", source[span.start..span.end].to_string()));
                }
                Inline::HardBreak { span } => {
                    out.push(("hard", source[span.start..span.end].to_string()));
                }
                Inline::Code { span, .. } => {
                    out.push(("code", source[span.start..span.end].to_string()));
                }
                Inline::Emphasis { content, span } => {
                    out.push(("em", source[span.start..span.end].to_string()));
                    out.extend(inline_span_profile(content, source));
                }
                Inline::Strong { content, span } => {
                    out.push(("strong", source[span.start..span.end].to_string()));
                    out.extend(inline_span_profile(content, source));
                }
                Inline::Link { content, span, .. } => {
                    out.push(("link", source[span.start..span.end].to_string()));
                    out.extend(inline_span_profile(content, source));
                }
                Inline::DirectiveCall { body, span, .. } => {
                    out.push(("directive", source[span.start..span.end].to_string()));
                    if let Some(body) = body {
                        out.extend(inline_span_profile(body, source));
                    }
                }
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
    fn link_basic() {
        let doc = parse("[example](https://example.com)");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 1);
        match &content[0] {
            Inline::Link {
                content,
                destination,
                span,
            } => {
                assert_text(&content[0], "example");
                assert_eq!(destination, "https://example.com");
                assert_eq!(*span, ByteSpan::new(0, 30));
            }
            other => panic!("expected link, got {other:?}"),
        }
    }

    #[test]
    fn link_inside_sentence() {
        let doc = parse("See [example](https://example.com) now.");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 3);
        assert_text(&content[0], "See ");
        match &content[1] {
            Inline::Link {
                content,
                destination,
                span,
            } => {
                assert_text(&content[0], "example");
                assert_eq!(destination, "https://example.com");
                assert_eq!(*span, ByteSpan::new(4, 34));
            }
            other => panic!("expected link, got {other:?}"),
        }
        assert_text(&content[2], " now.");
    }

    #[test]
    fn label_with_strong_and_text() {
        let doc = parse("[**bold** text](https://example.com)");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 1);
        match &content[0] {
            Inline::Link {
                content,
                destination,
                span,
            } => {
                assert_eq!(destination, "https://example.com");
                assert_eq!(*span, ByteSpan::new(0, 36));
                assert_eq!(content.len(), 2);
                match &content[0] {
                    Inline::Strong { content, .. } => assert_text(&content[0], "bold"),
                    other => panic!("expected strong, got {other:?}"),
                }
                assert_text(&content[1], " text");
            }
            other => panic!("expected link, got {other:?}"),
        }
    }

    #[test]
    fn label_with_quarkdown_inline_call() {
        let doc = parse("[.strong {hello}](https://example.com)");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 1);
        match &content[0] {
            Inline::Link {
                content,
                destination,
                span,
            } => {
                assert_eq!(destination, "https://example.com");
                assert_eq!(*span, ByteSpan::new(0, 38));
                assert_eq!(content.len(), 1);
                match &content[0] {
                    Inline::DirectiveCall {
                        name,
                        positional_args,
                        ..
                    } => {
                        assert_eq!(name, "strong");
                        assert_eq!(positional_args.len(), 1);
                        assert_eq!(positional_args[0], Value::Identifier("hello".into()));
                    }
                    other => panic!("expected directive call, got {other:?}"),
                }
            }
            other => panic!("expected link, got {other:?}"),
        }
    }

    #[test]
    fn link_fragment_and_relative_destinations() {
        for (input, dest) in [
            ("[section](#intro)", "#intro"),
            ("[file](docs/page.html)", "docs/page.html"),
            ("[guide](./guide.html)", "./guide.html"),
            ("[up](../assets/x.pdf)", "../assets/x.pdf"),
            ("[root](/absolute/path)", "/absolute/path"),
        ] {
            let doc = parse(input);
            let content = paragraph_inlines(&doc);
            assert_eq!(content.len(), 1, "input: {input}");
            match &content[0] {
                Inline::Link {
                    content,
                    destination,
                    span,
                } => {
                    assert_text(&content[0], &input[1..input.len() - (dest.len() + 3)]);
                    assert_eq!(destination, dest, "input: {input}");
                    assert_eq!(*span, ByteSpan::new(0, input.len()), "input: {input}");
                }
                other => panic!("expected link, got {other:?}"),
            }
        }
    }

    #[test]
    fn link_unicode_label() {
        let input = "[문서](https://example.com)";
        let doc = parse(input);
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 1);
        match &content[0] {
            Inline::Link {
                content,
                destination,
                span,
            } => {
                assert_eq!(content.len(), 1);
                match &content[0] {
                    Inline::Text { content, span } => {
                        assert_eq!(content, "문서");
                        assert_eq!(*span, ByteSpan::new(1, 7));
                    }
                    other => panic!("expected text, got {other:?}"),
                }
                assert_eq!(destination, "https://example.com");
                assert_eq!(*span, ByteSpan::new(0, input.len()));
            }
            other => panic!("expected link, got {other:?}"),
        }
    }

    #[test]
    fn two_links_keep_ordering() {
        let doc = parse("[a](https://a.example) and [b](https://b.example)");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 3);
        match &content[0] {
            Inline::Link {
                destination, span, ..
            } => {
                assert_eq!(destination, "https://a.example");
                assert_eq!(*span, ByteSpan::new(0, 22));
            }
            other => panic!("expected link, got {other:?}"),
        }
        assert_text(&content[1], " and ");
        match &content[2] {
            Inline::Link {
                destination, span, ..
            } => {
                assert_eq!(destination, "https://b.example");
                assert_eq!(*span, ByteSpan::new(27, 49));
            }
            other => panic!("expected link, got {other:?}"),
        }
    }

    #[test]
    fn link_spans_preserve_inner_offsets() {
        let doc = parse("before [hello **world**](https://example.com) after");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 3);
        assert_text(&content[0], "before ");
        match &content[1] {
            Inline::Link {
                content,
                destination,
                span,
            } => {
                assert_eq!(*span, ByteSpan::new(7, 45));
                assert_eq!(destination, "https://example.com");
                assert_eq!(content.len(), 2);
                match &content[0] {
                    Inline::Text { content, span } => {
                        assert_eq!(content, "hello ");
                        assert_eq!(*span, ByteSpan::new(8, 14));
                    }
                    other => panic!("expected text, got {other:?}"),
                }
                match &content[1] {
                    Inline::Strong { content, span } => {
                        assert_eq!(*span, ByteSpan::new(14, 23));
                        assert_eq!(content.len(), 1);
                        match &content[0] {
                            Inline::Text { content, span } => {
                                assert_eq!(content, "world");
                                assert_eq!(*span, ByteSpan::new(16, 21));
                            }
                            other => panic!("expected text, got {other:?}"),
                        }
                    }
                    other => panic!("expected strong, got {other:?}"),
                }
            }
            other => panic!("expected link, got {other:?}"),
        }
        assert_text(&content[2], " after");
    }

    #[test]
    fn link_destination_with_balanced_parens() {
        let doc = parse("[x](a(b)c)");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 1);
        match &content[0] {
            Inline::Link { destination, .. } => assert_eq!(destination, "a(b)c"),
            other => panic!("expected link, got {other:?}"),
        }
    }

    #[test]
    fn malformed_links_recover_as_literal_text() {
        for input in [
            "[text](",
            "[text](url",
            "[text]",
            "[](url)",
            "[text]()",
            "[text]( )",
            "[text](url \"title\")",
            "[text](a b)",
            "[",
            "[text",
        ] {
            let doc = parse(input);
            let content = paragraph_inlines(&doc);
            assert_eq!(joined_text(content), input, "input: {input}");
            assert!(!contains_link(content), "input: {input}");
        }
    }

    #[test]
    fn image_syntax_is_not_a_link() {
        let input = "![alt](image.png)";
        let doc = parse(input);
        let content = paragraph_inlines(&doc);
        assert_eq!(joined_text(content), input);
        assert!(!contains_link(content));
    }

    #[test]
    fn nested_bracket_label_ends_at_first_bracket() {
        let doc = parse("[a [b](c)](d)");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 2);
        match &content[0] {
            Inline::Link {
                content,
                destination,
                ..
            } => {
                assert_eq!(destination, "c");
                assert_eq!(joined_text(content), "a [b");
            }
            other => panic!("expected link, got {other:?}"),
        }
        assert_text(&content[1], "](d)");
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
    fn ordered_list_single_item() {
        let doc = parse("1. item");
        match &doc.nodes[0] {
            Block::OrderedList { items, start, span } => {
                assert_eq!(items.len(), 1);
                assert_eq!(*start, 1);
                assert_eq!(items[0].content.len(), 1);
                assert_eq!(items[0].span, ByteSpan::new(0, 7));
                assert_eq!(*span, ByteSpan::new(0, 7));
            }
            other => panic!("expected ordered list, got {other:?}"),
        }
    }

    #[test]
    fn ordered_list_multiple_items() {
        let doc = parse("1. one\n2. two\n3. three");
        match &doc.nodes[0] {
            Block::OrderedList { items, start, .. } => {
                assert_eq!(items.len(), 3);
                assert_eq!(*start, 1);
                for (i, item) in items.iter().enumerate() {
                    match &item.content[0] {
                        Block::Paragraph { content, .. } => {
                            assert_text(&content[0], ["one", "two", "three"][i]);
                        }
                        other => panic!("expected paragraph, got {other:?}"),
                    }
                }
            }
            other => panic!("expected ordered list, got {other:?}"),
        }
    }

    #[test]
    fn ordered_list_non_one_start() {
        let doc = parse("3. three\n4. four");
        match &doc.nodes[0] {
            Block::OrderedList { items, start, .. } => {
                assert_eq!(items.len(), 2);
                assert_eq!(*start, 3);
            }
            other => panic!("expected ordered list, got {other:?}"),
        }
    }

    #[test]
    fn ordered_list_parentheses_marker() {
        let doc = parse("1) one\n2) two");
        match &doc.nodes[0] {
            Block::OrderedList { items, start, .. } => {
                assert_eq!(items.len(), 2);
                assert_eq!(*start, 1);
            }
            other => panic!("expected ordered list, got {other:?}"),
        }
    }

    #[test]
    fn ordered_list_without_space_is_paragraph() {
        let doc = parse("1.item\n2.item");
        assert_text(&paragraph_inlines(&doc)[0], "1.item");
    }

    #[test]
    fn nested_ordered_list() {
        // Content column is derived from each item's own marker; "1. " puts
        // content at column 4, so the nested item needs 4 leading spaces
        let doc = parse("1. outer\n   1. inner");
        match &doc.nodes[0] {
            Block::OrderedList { items, .. } => {
                assert_eq!(items.len(), 1);
                match &items[0].content[1] {
                    Block::OrderedList { items, .. } => assert_eq!(items.len(), 1),
                    other => panic!("expected nested ordered list, got {other:?}"),
                }
            }
            other => panic!("expected ordered list, got {other:?}"),
        }
    }

    #[test]
    fn ordered_then_unordered_nested() {
        // Content column is derived from each item's own marker; "1. " puts
        // content at column 4, so the nested items need 4 leading spaces
        let doc = parse("1. ordered\n    - unordered\n    - another");
        match &doc.nodes[0] {
            Block::OrderedList { items, .. } => {
                assert_eq!(items.len(), 1);
                match &items[0].content[1] {
                    Block::UnorderedList { items, .. } => assert_eq!(items.len(), 2),
                    other => panic!("expected nested unordered list, got {other:?}"),
                }
            }
            other => panic!("expected ordered list, got {other:?}"),
        }
    }
    #[test]
    fn unordered_then_ordered_nested() {
        // Content column is derived from each item's own marker; "- " puts
        // content at column 2, so 2+ spaces suffice for the nested items
        let doc = parse("- unordered\n  1. ordered\n  2. ordered");
        match &doc.nodes[0] {
            Block::UnorderedList { items, .. } => {
                assert_eq!(items.len(), 1);
                match &items[0].content[1] {
                    Block::OrderedList { items, .. } => assert_eq!(items.len(), 2),
                    other => panic!("expected nested ordered list, got {other:?}"),
                }
            }
            other => panic!("expected unordered list, got {other:?}"),
        }
    }

    #[test]
    fn number_followed_by_text_is_paragraph() {
        let doc = parse("123abc");
        assert_text(&paragraph_inlines(&doc)[0], "123abc");
    }

    #[test]
    fn decimal_number_is_paragraph() {
        let doc = parse("1.23");
        assert_text(&paragraph_inlines(&doc)[0], "1.23");
    }

    #[test]
    fn blank_lines_between_ordered_items() {
        let doc = parse("1. one\n\n2. two");
        match &doc.nodes[0] {
            Block::OrderedList { items, .. } => assert_eq!(items.len(), 2),
            other => panic!("expected ordered list, got {other:?}"),
        }
    }

    #[test]
    fn ordered_list_item_continuation() {
        // Content column is 3 for ordered lists
        let doc = parse("1. first line\n   second line");
        match &doc.nodes[0] {
            Block::OrderedList { items, .. } => match &items[0].content[0] {
                Block::Paragraph { content, .. } => {
                    assert_eq!(content.len(), 3);
                    assert_text(&content[0], "first line");
                    assert!(matches!(content[1], Inline::SoftBreak { .. }));
                    assert_text(&content[2], "second line");
                }
                other => panic!("expected paragraph, got {other:?}"),
            },
            other => panic!("expected ordered list, got {other:?}"),
        }
    }

    #[test]
    fn ordered_list_with_code_block() {
        // Content column is 3 for ordered lists
        let doc = parse("1. item\n   ```\n   code\n   ```");
        match &doc.nodes[0] {
            Block::OrderedList { items, .. } => match &items[0].content[1] {
                Block::CodeBlock { source, .. } => assert_eq!(source, "code"),
                other => panic!("expected code block, got {other:?}"),
            },
            other => panic!("expected ordered list, got {other:?}"),
        }
    }

    #[test]
    fn ordered_list_source_spans() {
        let doc = parse("1. first\n2. second");
        match &doc.nodes[0] {
            Block::OrderedList { items, span, .. } => {
                // First item "1. first" (8 bytes) + newline (1 byte) = 0..9
                // Second item "2. second" (9 bytes) = 9..18
                assert_eq!(items[0].span, ByteSpan::new(0, 9));
                assert_eq!(items[1].span, ByteSpan::new(9, 18));
                assert_eq!(*span, ByteSpan::new(0, 18));
                assert_eq!(items[0].span, ByteSpan::new(0, 9));
                assert_eq!(items[1].span, ByteSpan::new(9, 18));
                assert_eq!(*span, ByteSpan::new(0, 18));
            }
            other => panic!("expected ordered list, got {other:?}"),
        }
    }

    #[test]
    fn ordered_list_repeated_marker_numbers() {
        // Repeated marker numbers form one list; only first ordinal determines start
        let doc = parse("1. A\n1. B\n1. C");
        match &doc.nodes[0] {
            Block::OrderedList { items, start, .. } => {
                assert_eq!(items.len(), 3);
                assert_eq!(*start, 1);
            }
            other => panic!("expected ordered list, got {other:?}"),
        }
    }

    #[test]
    fn ordered_list_arbitrary_subsequent_numbers() {
        // Non-sequential source ordinals form one list; only first ordinal determines start
        let doc = parse("3. A\n8. B\n42. C");
        match &doc.nodes[0] {
            Block::OrderedList { items, start, .. } => {
                assert_eq!(items.len(), 3);
                assert_eq!(*start, 3);
            }
            other => panic!("expected ordered list, got {other:?}"),
        }
    }

    #[test]
    fn ordered_list_delimiter_boundary_dot_then_paren() {
        // Different delimiters break the list
        let doc = parse("1. A\n2) B");
        assert_eq!(doc.nodes.len(), 2);
        assert!(matches!(doc.nodes[0], Block::OrderedList { .. }));
        assert!(matches!(doc.nodes[1], Block::OrderedList { .. }));
    }

    #[test]
    fn ordered_list_delimiter_boundary_paren_then_dot() {
        // Different delimiters break the list (reverse)
        let doc = parse("1) A\n2. B");
        assert_eq!(doc.nodes.len(), 2);
        assert!(matches!(doc.nodes[0], Block::OrderedList { .. }));
        assert!(matches!(doc.nodes[1], Block::OrderedList { .. }));
    }

    #[test]
    fn ordered_list_same_parenthesis_delimiter() {
        // Same parenthesis delimiter forms one list
        let doc = parse("3) A\n9) B");
        match &doc.nodes[0] {
            Block::OrderedList { items, start, .. } => {
                assert_eq!(items.len(), 2);
                assert_eq!(*start, 3);
            }
            other => panic!("expected ordered list, got {other:?}"),
        }
    }

    #[test]
    fn ordered_list_marker_width_transition() {
        // 9. and 10. have different marker widths; continuation content uses item's own column
        // Continuation lines are joined into the same paragraph with soft breaks
        let doc = parse("9. parent\n10. second\n    nested content");
        match &doc.nodes[0] {
            Block::OrderedList { items, .. } => {
                assert_eq!(items.len(), 2);
                // Second item should have "second" and "nested content" in same paragraph with soft break
                match &items[1].content[0] {
                    Block::Paragraph { content, .. } => {
                        assert_eq!(content.len(), 3);
                        assert_text(&content[0], "second");
                        assert!(matches!(content[1], Inline::SoftBreak { .. }));
                        assert_text(&content[2], "nested content");
                    }
                    other => panic!("expected paragraph with soft break, got {other:?}"),
                }
            }
            other => panic!("expected ordered list, got {other:?}"),
        }
    }

    #[test]
    fn ordered_list_digit_limit_9_digits() {
        // 9 digits should be recognized as ordered list marker
        let doc = parse("123456789. item");
        assert!(matches!(doc.nodes[0], Block::OrderedList { .. }));
    }

    #[test]
    fn ordered_list_digit_limit_10_digits() {
        // 10 digits should NOT be recognized as ordered list marker (paragraph)
        let doc = parse("1234567890. item");
        assert_text(&paragraph_inlines(&doc)[0], "1234567890. item");
    }

    #[test]
    fn nested_ordered_list_hierarchy() {
        // 1. parent
        //    1. child
        //    2. child
        // 2. sibling
        let doc = parse("1. parent\n   1. child\n   2. child\n2. sibling");
        match &doc.nodes[0] {
            Block::OrderedList { items, .. } => {
                assert_eq!(items.len(), 2);
                // First item has nested ordered list with 2 items
                match &items[0].content[1] {
                    Block::OrderedList { items, .. } => assert_eq!(items.len(), 2),
                    other => panic!("expected nested ordered list, got {other:?}"),
                }
                // Second item is sibling
                assert_eq!(items[1].content.len(), 1);
            }
            other => panic!("expected ordered list, got {other:?}"),
        }
    }

    #[test]
    fn mixed_nesting_ordered_then_unordered() {
        // 1. ordered
        //    - unordered
        //    - another
        // 2. ordered
        let doc = parse("1. ordered\n   - unordered\n   - another\n2. ordered");
        match &doc.nodes[0] {
            Block::OrderedList { items, .. } => {
                assert_eq!(items.len(), 2);
                match &items[0].content[1] {
                    Block::UnorderedList { items, .. } => assert_eq!(items.len(), 2),
                    other => panic!("expected nested unordered list, got {other:?}"),
                }
            }
            other => panic!("expected ordered list, got {other:?}"),
        }
    }

    #[test]
    fn mixed_nesting_unordered_then_ordered() {
        // - unordered
        //   1. ordered
        //   2. ordered
        // - unordered
        let doc = parse("- unordered\n  1. ordered\n  2. ordered\n- unordered");
        match &doc.nodes[0] {
            Block::UnorderedList { items, .. } => {
                assert_eq!(items.len(), 2);
                match &items[0].content[1] {
                    Block::OrderedList { items, .. } => assert_eq!(items.len(), 2),
                    other => panic!("expected nested ordered list, got {other:?}"),
                }
            }
            other => panic!("expected unordered list, got {other:?}"),
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
    fn code_span_basic() {
        let doc = parse("`code`");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 1);
        match &content[0] {
            Inline::Code { content, span } => {
                assert_eq!(content, "code");
                assert_eq!(*span, ByteSpan::new(0, 6));
            }
            other => panic!("expected code span, got {other:?}"),
        }
    }

    #[test]
    fn code_span_embedded_in_prose() {
        let doc = parse("Use `cargo test` before pushing.");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 3);
        assert_text(&content[0], "Use ");
        match &content[1] {
            Inline::Code { content, span } => {
                assert_eq!(content, "cargo test");
                assert_eq!(*span, ByteSpan::new(4, 16));
            }
            other => panic!("expected code span, got {other:?}"),
        }
        assert_text(&content[2], " before pushing.");
    }

    #[test]
    fn multiple_code_spans_keep_ordering() {
        let doc = parse("`foo` and `bar`");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 3);
        match &content[0] {
            Inline::Code { content, span } => {
                assert_eq!(content, "foo");
                assert_eq!(*span, ByteSpan::new(0, 5));
            }
            other => panic!("expected code span, got {other:?}"),
        }
        assert_text(&content[1], " and ");
        match &content[2] {
            Inline::Code { content, span } => {
                assert_eq!(content, "bar");
                assert_eq!(*span, ByteSpan::new(10, 15));
            }
            other => panic!("expected code span, got {other:?}"),
        }
    }

    #[test]
    fn code_span_empty_and_minimal_constructs() {
        // A single backtick run without a closer stays literal.
        let doc = parse("`");
        assert_eq!(joined_text(paragraph_inlines(&doc)), "`");
        // Two adjacent backticks form one maximal run; nothing closes it.
        let doc = parse("``");
        assert_eq!(joined_text(paragraph_inlines(&doc)), "``");
        // A run followed only by a run of different length never closes.
        let doc = parse("` ``");
        assert_eq!(joined_text(paragraph_inlines(&doc)), "` ``");
        // Content containing a backtick keeps it when the delimiters are
        // longer; joined_text renders the node back with its delimiters.
        let doc = parse("`` ` ``");
        let content = paragraph_inlines(&doc);
        match &content[0] {
            Inline::Code { content, span } => {
                assert_eq!(content, "`");
                assert_eq!(*span, ByteSpan::new(0, 7));
            }
            other => panic!("expected code span, got {other:?}"),
        }
        // A single space between same-length delimiters is preserved
        // (all-space content keeps its spaces).
        let doc = parse("`` ``");
        let content = paragraph_inlines(&doc);
        match &content[0] {
            Inline::Code { content, .. } => assert_eq!(content, " "),
            other => panic!("expected code span, got {other:?}"),
        }
        let doc = parse("` `");
        let content = paragraph_inlines(&doc);
        match &content[0] {
            Inline::Code { content, .. } => assert_eq!(content, " "),
            other => panic!("expected code span, got {other:?}"),
        }
    }

    #[test]
    fn code_span_variable_length_delimiters() {
        let doc = parse("``foo ` bar``");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 1);
        match &content[0] {
            Inline::Code { content, span } => {
                assert_eq!(content, "foo ` bar");
                assert_eq!(*span, ByteSpan::new(0, 13));
            }
            other => panic!("expected code span, got {other:?}"),
        }
        // Longer delimiters protect runs of any other length. (Triple-backtick
        // input must not start at line start, where it is a fenced code block.)
        let doc = parse("x ```code ` `` ``` y");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 3);
        match &content[1] {
            Inline::Code { content, span } => {
                assert_eq!(content, "code ` `` ");
                assert_eq!(*span, ByteSpan::new(2, 18));
            }
            other => panic!("expected code span, got {other:?}"),
        }
    }

    #[test]
    fn code_span_mismatched_delimiter_lengths_do_not_close() {
        // `foo``bar`: the run of two backticks does not close the single
        // backtick span; the final single backtick does.
        let doc = parse("`foo``bar`");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 1);
        match &content[0] {
            Inline::Code { content, .. } => assert_eq!(content, "foo``bar"),
            other => panic!("expected code span, got {other:?}"),
        }
        // ``foo`bar``: the single backtick inside is literal content.
        let doc = parse("``foo`bar``");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 1);
        match &content[0] {
            Inline::Code { content, .. } => assert_eq!(content, "foo`bar"),
            other => panic!("expected code span, got {other:?}"),
        }
    }

    #[test]
    fn code_span_keeps_markdown_literal() {
        let doc = parse("`**bold**`");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 1);
        match &content[0] {
            Inline::Code { content, .. } => assert_eq!(content, "**bold**"),
            other => panic!("expected code span, got {other:?}"),
        }
    }

    #[test]
    fn code_span_keeps_link_literal() {
        let doc = parse("`[link](https://example.com)`");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 1);
        match &content[0] {
            Inline::Code { content, .. } => {
                assert_eq!(content, "[link](https://example.com)")
            }
            other => panic!("expected code span, got {other:?}"),
        }
    }

    #[test]
    fn code_span_keeps_quarkdown_call_literal() {
        let doc = parse("`.strong {hello}`");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 1);
        match &content[0] {
            Inline::Code { content, .. } => assert_eq!(content, ".strong {hello}"),
            other => panic!("expected code span, got {other:?}"),
        }
    }

    #[test]
    fn code_span_backslashes_are_literal() {
        let doc = parse("`a\\bc`");
        let content = paragraph_inlines(&doc);
        match &content[0] {
            Inline::Code { content, .. } => assert_eq!(content, "a\\bc"),
            other => panic!("expected code span, got {other:?}"),
        }
        // A backslash-space must not become an escape or a hard break.
        let doc = parse("`a\\ b`");
        let content = paragraph_inlines(&doc);
        match &content[0] {
            Inline::Code { content, .. } => assert_eq!(content, "a\\ b"),
            other => panic!("expected code span, got {other:?}"),
        }
    }

    #[test]
    fn code_span_multiline_becomes_single_space() {
        let doc = parse("`foo\nbar`");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 1);
        match &content[0] {
            Inline::Code { content, span } => {
                assert_eq!(content, "foo bar");
                assert_eq!(*span, ByteSpan::new(0, 9));
            }
            other => panic!("expected code span, got {other:?}"),
        }
        let doc = parse("`foo\r\nbar`");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 1);
        match &content[0] {
            Inline::Code { content, .. } => assert_eq!(content, "foo bar"),
            other => panic!("expected code span, got {other:?}"),
        }
    }

    #[test]
    fn code_span_surrounding_space_normalization() {
        let cases = [
            ("` foo `", "foo"),
            ("`  foo  `", " foo "),
            ("`  `", "  "),
            ("` `", " "),
            ("`foo `", "foo "),
            ("` foo`", " foo"),
            ("`\u{a0}foo\u{a0}`", "\u{a0}foo\u{a0}"),
        ];
        for (input, expected) in cases {
            let doc = parse(input);
            let content = paragraph_inlines(&doc);
            assert_eq!(content.len(), 1, "input {input:?}");
            match &content[0] {
                Inline::Code { content, .. } => {
                    assert_eq!(content, expected, "input {input:?}")
                }
                other => panic!("input {input:?}: expected code span, got {other:?}"),
            }
        }
    }

    #[test]
    fn code_span_unicode_content_and_spans() {
        let doc = parse("`한글 λ Rust 🦀`");
        let content = paragraph_inlines(&doc);
        match &content[0] {
            Inline::Code { content, span } => {
                assert_eq!(content, "한글 λ Rust 🦀");
                assert_eq!(*span, ByteSpan::new(0, 1 + 6 + 1 + 2 + 1 + 4 + 1 + 4 + 1));
            }
            other => panic!("expected code span, got {other:?}"),
        }
        // UTF-8 offsets relative to surrounding text stay byte-exact:
        // "abc " is 3 ASCII bytes + 1 space; 한글 occupies 6 bytes.
        let doc = parse("abc `한글` def");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 3);
        match &content[1] {
            Inline::Code { span, .. } => assert_eq!(*span, ByteSpan::new(4, 12)),
            other => panic!("expected code span, got {other:?}"),
        }
    }

    #[test]
    fn code_span_in_heading() {
        let doc = parse("# Run `cargo test`\n");
        match &doc.nodes[0] {
            Block::Heading { content, .. } => {
                assert_eq!(content.len(), 2);
                match &content[1] {
                    Inline::Code { content, .. } => assert_eq!(content, "cargo test"),
                    other => panic!("expected code span, got {other:?}"),
                }
            }
            other => panic!("expected heading, got {other:?}"),
        }
    }

    #[test]
    fn unclosed_code_span_recovers_as_literal_without_loss() {
        // The scope's examples: `` `foo `` and `` ``foo` `` have no matching
        // closer and must stay literal text with no partial code node.
        for input in ["`foo", "``foo`"] {
            let output = parse_with_diagnostics(input);
            assert!(output.diagnostics.is_empty(), "input {input:?}");
            match &output.document.nodes[0] {
                Block::Paragraph { content, .. } => {
                    assert_eq!(joined_text(content), input, "input {input:?}");
                    assert!(
                        !content
                            .iter()
                            .any(|inline| matches!(inline, Inline::Code { .. })),
                        "input {input:?} must not produce code nodes"
                    );
                }
                other => panic!("input {input:?}: expected paragraph, got {other:?}"),
            }
        }

        // A multi-line construct without a closer also stays literal,
        // including when it spans a soft break.
        let output = parse_with_diagnostics("`foo\nbar\n`` baz");
        assert!(output.diagnostics.is_empty());
        match &output.document.nodes[0] {
            Block::Paragraph { content, .. } => {
                assert_eq!(joined_text(content), "`foo\nbar\n`` baz");
            }
            other => panic!("expected paragraph, got {other:?}"),
        }
    }

    #[test]
    fn document_snapshot_code_spans() {
        insta::assert_debug_snapshot!(
            "code_spans",
            parse("Run `cargo test` and ``foo ` bar``.\n\nLiteral: `**bold**` and `[x](y)` and `.s {v}`.\n\nUnclosed `oops.\n")
        );
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
    fn document_snapshot_links() {
        insta::assert_debug_snapshot!(
            "links",
            parse("Visit [Typst](https://typst.app).\n\nSee [**M2** docs](#intro) or [file](docs/page.html).\n")
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
            (".foo key:", "E2002"),
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
        for input in [".foo {bar}\n", ".foo key:{value}\n", ".1\n"] {
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
        let doc = parse(".1\n");
        assert!(matches!(
            &doc.nodes[0],
            Block::DirectiveCall { name, .. } if name == "1"
        ));
    }

    #[test]
    fn implicit_reference_is_not_a_call_with_positional_argument() {
        for input in [".1 {item}\n", ".12foo\n", ".1abc\n"] {
            let doc = parse(input);
            // Block level: the line must not become a directive block that
            // turned `.1 {item}` into a call with a positional argument.
            assert!(
                matches!(&doc.nodes[0], Block::Paragraph { .. }),
                "input {input:?} should stay a paragraph"
            );
            assert!(
                !matches!(
                    &doc.nodes[0],
                    Block::DirectiveCall { positional_args, .. }
                        if !positional_args.is_empty()
                ),
                "input {input:?} must not be a call with positional args"
            );
        }
        // Inline level: `.1abc` / `.12foo` must not split into `ref + text`.
        let doc = parse("see .1abc\n");
        match &doc.nodes[0] {
            Block::Paragraph { content, .. } => {
                let joined = joined_text(content);
                assert_eq!(joined, "see .1abc");
            }
            other => panic!("expected paragraph, got {other:?}"),
        }
    }

    #[test]
    fn implicit_reference_inline_boundaries() {
        // Punctuation/whitespace after the reference keeps it a call.
        let doc = parse("The value is .1.\n");
        let content = paragraph_inlines(&doc);
        assert!(content.iter().any(
            |inline| matches!(inline, Inline::DirectiveCall { name, positional_args, .. }
                if name == "1" && positional_args.is_empty())
        ));
    }

    #[test]
    fn inline_call_at_line_start_continues_paragraph() {
        // A call with trailing inline content is not a block directive, so
        // it must not terminate the surrounding paragraph.
        let doc = parse("before\n.note {x} after\nend\n");
        assert_eq!(doc.nodes.len(), 1, "expected a single paragraph");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 6);
        assert_text(&content[0], "before");
        assert!(matches!(content[1], Inline::SoftBreak { .. }));
        match &content[2] {
            Inline::DirectiveCall {
                name,
                positional_args,
                ..
            } => {
                assert_eq!(name, "note");
                assert_eq!(positional_args.len(), 1);
            }
            other => panic!("expected inline call, got {other:?}"),
        }
        assert_text(&content[3], " after");
        assert!(matches!(content[4], Inline::SoftBreak { .. }));
        assert!(matches!(&content[5], Inline::Text { .. }));
    }

    #[test]
    fn invalid_implicit_reference_does_not_split_paragraph() {
        // `.1abc` is ordinary text and must not split the paragraph.
        let doc = parse("before\n.1abc\nafter\n");
        assert_eq!(doc.nodes.len(), 1);
        assert_eq!(
            joined_text(paragraph_inlines(&doc)),
            "before\n.1abc\nafter",
            "no call may appear inside the paragraph"
        );
        let output = parse_with_diagnostics("before\n.1abc\nafter\n");
        assert!(output.diagnostics.is_empty());
    }

    #[test]
    fn isolated_call_line_still_starts_block() {
        let doc = parse("before\n.note {x}\nafter\n");
        assert_eq!(doc.nodes.len(), 3);
        assert!(matches!(doc.nodes[0], Block::Paragraph { .. }));
        assert!(matches!(
            &doc.nodes[1],
            Block::DirectiveCall { name, .. } if name == "note"
        ));
        assert!(matches!(doc.nodes[2], Block::Paragraph { .. }));
    }

    #[test]
    fn block_body_still_works_with_semantic_classification() {
        let doc = parse("before\n\n.note {x}\n    body\n\nafter\n");
        assert_eq!(doc.nodes.len(), 3);
        match &doc.nodes[1] {
            Block::DirectiveCall { name, body, .. } => {
                assert_eq!(name, "note");
                let blocks = body.as_ref().expect("expected an indented body");
                assert_eq!(blocks.len(), 1);
                match &blocks[0] {
                    Block::Paragraph { content, .. } => {
                        assert_eq!(joined_text(content), "body");
                    }
                    other => panic!("expected body paragraph, got {other:?}"),
                }
            }
            other => panic!("expected directive call, got {other:?}"),
        }
    }

    #[test]
    fn tight_call_boundary_rejects_trailing_word() {
        // `before .note {x}suffix` must NOT produce a call: the suffix
        // glues to the call, so the whole construct is prose.
        let doc = parse("before .note {x}suffix\n");
        assert_eq!(
            joined_text(paragraph_inlines(&doc)),
            "before .note {x}suffix",
            "tight trailing word must keep the whole construct ordinary text"
        );
        // A spaced suffix is a legal boundary.
        let doc = parse("before .note {x} suffix\n");
        let content = paragraph_inlines(&doc);
        assert!(
            matches!(&content[1], Inline::DirectiveCall { name, .. } if name == "note"),
            "a space after the call is a valid boundary"
        );
    }

    #[test]
    fn tight_call_hyphen_boundaries_are_valid() {
        // The hyphen is a documented symbol boundary on both sides.
        let doc = parse("before-.note {x}-after\n");
        let content = paragraph_inlines(&doc);
        assert!(content.iter().any(|inline| matches!(
            inline,
            Inline::DirectiveCall { name, .. } if name == "note"
        )));
    }

    #[test]
    fn unicode_word_characters_are_tight_adjacency() {
        // Non-ASCII letters are word characters, not symbols: a call glued
        // to Korean script must not be recognized.
        let doc = parse("한.note {x}\n");
        assert_eq!(joined_text(paragraph_inlines(&doc)), "한.note {x}");
        let doc = parse(".note {x}한\n");
        assert_eq!(joined_text(paragraph_inlines(&doc)), ".note {x}한");
    }

    #[test]
    fn nested_call_with_tight_suffix_is_not_a_call() {
        // Inside an argument the same boundary rules apply: `.inner`
        // followed by a word character must not become a nested call.
        let doc = parse(".outer {prefix .inner {x}suffix}\n");
        match &doc.nodes[0] {
            Block::DirectiveCall {
                name,
                positional_args,
                ..
            } => {
                assert_eq!(name, "outer");
                match &positional_args[0] {
                    Value::Content(content) => {
                        assert!(!content
                            .iter()
                            .any(|inline| matches!(inline, Inline::DirectiveCall { .. })));
                    }
                    other => panic!("expected content argument, got {other:?}"),
                }
            }
            other => panic!("expected outer call, got {other:?}"),
        }
    }

    #[test]
    fn blockquote_basic() {
        // Basic blockquote with > marker
        let doc = parse("> foo\n");
        match &doc.nodes[0] {
            Block::BlockQuote { content, .. } => {
                assert_eq!(content.len(), 1);
                match &content[0] {
                    Block::Paragraph { content, .. } => {
                        assert_eq!(joined_text(content), "foo");
                    }
                    other => panic!("expected paragraph, got {other:?}"),
                }
            }
            other => panic!("expected blockquote, got {other:?}"),
        }
    }

    #[test]
    fn blockquote_multiline() {
        // Multiple consecutive quoted lines form a single blockquote
        let doc = parse("> foo\n> bar\n> baz\n");
        match &doc.nodes[0] {
            Block::BlockQuote { content, .. } => {
                assert_eq!(content.len(), 1);
                match &content[0] {
                    Block::Paragraph { content, .. } => {
                        assert_eq!(joined_text(content), "foo\nbar\nbaz");
                    }
                    other => panic!("expected paragraph, got {other:?}"),
                }
            }
            other => panic!("expected blockquote, got {other:?}"),
        }
    }

    #[test]
    fn blockquote_marker_with_space() {
        // > with optional space
        let doc = parse("> foo\n>bar\n");
        match &doc.nodes[0] {
            Block::BlockQuote { content, .. } => {
                assert_eq!(content.len(), 1);
                match &content[0] {
                    Block::Paragraph { content, .. } => {
                        assert_eq!(joined_text(content), "foo\nbar");
                    }
                    other => panic!("expected paragraph, got {other:?}"),
                }
            }
            other => panic!("expected blockquote, got {other:?}"),
        }
    }

    #[test]
    fn blockquote_indented_0_to_3_spaces() {
        // CommonMark allows 0-3 spaces before >
        for (input, desc) in [
            ("> foo\n", "0 spaces"),
            (" > foo\n", "1 space"),
            ("  > foo\n", "2 spaces"),
            ("   > foo\n", "3 spaces"),
        ] {
            let doc = parse(input);
            assert_eq!(doc.nodes.len(), 1, "failed for {desc}");
            match &doc.nodes[0] {
                Block::BlockQuote { content, .. } => {
                    assert_eq!(content.len(), 1, "failed for {desc}");
                    match &content[0] {
                        Block::Paragraph { content, .. } => {
                            assert_eq!(joined_text(content), "foo", "failed for {desc}");
                        }
                        other => panic!("failed for {desc}: expected paragraph, got {other:?}"),
                    }
                }
                other => panic!("failed for {desc}: expected blockquote, got {other:?}"),
            }
        }
    }

    #[test]
    fn blockquote_not_indented_4_spaces() {
        // 4 spaces before > is NOT a blockquote marker (indented code/paragraph)
        let doc = parse("    > foo\n");
        assert_eq!(doc.nodes.len(), 1);
        match &doc.nodes[0] {
            Block::Paragraph { content, .. } => {
                assert_eq!(joined_text(content), "> foo");
            }
            other => panic!("expected paragraph, got {other:?}"),
        }
    }

    #[test]
    fn blockquote_lazy_continuation() {
        // Non-quoted line after quoted paragraph continues the paragraph (lazy continuation)
        let doc = parse("> foo\nbar\n");
        match &doc.nodes[0] {
            Block::BlockQuote { content, .. } => {
                assert_eq!(content.len(), 1);
                match &content[0] {
                    Block::Paragraph { content, .. } => {
                        assert_eq!(joined_text(content), "foo\nbar");
                    }
                    other => panic!("expected paragraph, got {other:?}"),
                }
            }
            other => panic!("expected blockquote, got {other:?}"),
        }
    }

    #[test]
    fn blockquote_quoted_blank_line_separates_paragraphs() {
        // Quoted blank line (> or > ) separates paragraphs within same blockquote
        let doc = parse("> foo\n>\n> bar\n");
        match &doc.nodes[0] {
            Block::BlockQuote { content, .. } => {
                assert_eq!(content.len(), 2);
                match (&content[0], &content[1]) {
                    (
                        Block::Paragraph { content: c1, .. },
                        Block::Paragraph { content: c2, .. },
                    ) => {
                        assert_eq!(joined_text(c1), "foo");
                        assert_eq!(joined_text(c2), "bar");
                    }
                    other => panic!("expected two paragraphs, got {other:?}"),
                }
            }
            other => panic!("expected blockquote, got {other:?}"),
        }
    }

    #[test]
    fn blockquote_unquoted_blank_line_terminates() {
        // Unquoted blank line ends the blockquote
        let doc = parse("> foo\n\n> bar\n");
        assert_eq!(doc.nodes.len(), 2);
        match (&doc.nodes[0], &doc.nodes[1]) {
            (Block::BlockQuote { content: c1, .. }, Block::BlockQuote { content: c2, .. }) => {
                assert_eq!(c1.len(), 1);
                assert_eq!(c2.len(), 1);
                match (&c1[0], &c2[0]) {
                    (
                        Block::Paragraph { content: p1, .. },
                        Block::Paragraph { content: p2, .. },
                    ) => {
                        assert_eq!(joined_text(p1), "foo");
                        assert_eq!(joined_text(p2), "bar");
                    }
                    other => panic!("expected paragraphs, got {other:?}"),
                }
            }
            other => panic!("expected two blockquotes, got {other:?}"),
        }
    }

    /// Assert that `source` parses to exactly one blockquote containing a
    /// single paragraph (`quoted_text`), followed by one outside paragraph
    /// (`outside_text`). Used by the quoted-blank lazy-continuation tests.
    fn assert_quote_with_single_paragraph_then_paragraph(
        source: &str,
        quoted_text: &str,
        outside_text: &str,
    ) {
        let doc = parse(source);
        assert_eq!(doc.nodes.len(), 2, "expected 2 top-level nodes");
        match (&doc.nodes[0], &doc.nodes[1]) {
            (
                Block::BlockQuote { content, .. },
                Block::Paragraph {
                    content: outside, ..
                },
            ) => {
                assert_eq!(content.len(), 1, "expected single quoted block");
                match &content[0] {
                    Block::Paragraph { content, .. } => {
                        assert_eq!(joined_text(content), quoted_text);
                    }
                    other => panic!("expected quoted paragraph, got {other:?}"),
                }
                assert_eq!(joined_text(outside), outside_text);
            }
            other => panic!("expected blockquote + paragraph, got {other:?}"),
        }
    }

    #[test]
    fn blockquote_lazy_continuation_quoted_blank_negative() {
        // CommonMark Example 249: a quoted blank line (">") inside the
        // blockquote forbids lazy continuation of the following
        // marker-less line, so "baz" is a paragraph outside the quote.
        assert_quote_with_single_paragraph_then_paragraph("> bar\n>\nbaz\n", "bar", "baz");
    }

    #[test]
    fn blockquote_lazy_continuation_quoted_blank_space_variant() {
        // "> " (marker + one space) is also a quoted blank line.
        assert_quote_with_single_paragraph_then_paragraph("> bar\n> \nbaz\n", "bar", "baz");
    }

    #[test]
    fn blockquote_lazy_continuation_quoted_blank_spaces_variant() {
        // ">   " (marker + whitespace-only content) is also a quoted blank
        // line and forbids lazy continuation the same way.
        assert_quote_with_single_paragraph_then_paragraph("> bar\n>   \nbaz\n", "bar", "baz");
    }

    /// Assert that `source` parses to exactly one top-level block whose
    /// structure is `levels` nested `BlockQuote`s wrapping a single
    /// `Paragraph` whose logical text equals `expected`.
    fn assert_nested_blockquote_paragraph(source: &str, levels: usize, expected: &str) {
        let doc = parse(source);
        assert_eq!(doc.nodes.len(), 1, "expected one top-level node");
        let mut node = &doc.nodes[0];
        for _ in 0..levels {
            match node {
                Block::BlockQuote { content, .. } => {
                    assert_eq!(content.len(), 1, "expected single block at each level");
                    node = &content[0];
                }
                other => panic!("expected blockquote, got {other:?}"),
            }
        }
        match node {
            Block::Paragraph { content, .. } => assert_eq!(joined_text(content), expected),
            other => panic!("expected paragraph, got {other:?}"),
        }
    }

    #[test]
    fn blockquote_lazy_continuation_nested_example_250() {
        // CommonMark Example 250: a marker-less line after three nested
        // markers continues the deepest open paragraph, so "bar" must not
        // escape to a top-level paragraph.
        assert_nested_blockquote_paragraph("> > > foo\nbar\n", 3, "foo\nbar");
    }

    #[test]
    fn blockquote_lazy_continuation_nested_example_251() {
        // CommonMark Example 251: once a nested paragraph is open, markers
        // are optional on each line regardless of which container depth the
        // line actually carries; all three lines join the deepest paragraph.
        assert_nested_blockquote_paragraph(">>> foo\n> bar\n>>baz\n", 3, "foo\nbar\nbaz");
    }

    #[test]
    fn blockquote_lazy_continuation_nested_blank_negative() {
        // A quoted blank inside the inner quote (`> >`) ends the inner
        // paragraph, so the marker-less "bar" cannot lazily continue it and
        // becomes a top-level paragraph.
        let doc = parse("> > foo\n> >\nbar\n");
        assert_eq!(doc.nodes.len(), 2);
        match (&doc.nodes[0], &doc.nodes[1]) {
            (Block::BlockQuote { content: quote, .. }, Block::Paragraph { content: p, .. }) => {
                assert_eq!(quote.len(), 1);
                match &quote[0] {
                    Block::BlockQuote { content, .. } => {
                        assert_eq!(content.len(), 1);
                        match &content[0] {
                            Block::Paragraph { content, .. } => {
                                assert_eq!(joined_text(content), "foo");
                            }
                            other => panic!("expected inner paragraph, got {other:?}"),
                        }
                    }
                    other => panic!("expected inner blockquote, got {other:?}"),
                }
                assert_eq!(joined_text(p), "bar");
            }
            other => panic!("expected quote + outside paragraph, got {other:?}"),
        }
    }

    #[test]
    fn blockquote_lazy_continuation_partial_container() {
        // "> > foo\n> >\n> bar": the inner quote ends after its blank line
        // while the outer quote stays open and owns "bar" as its own
        // paragraph (deeper containers are not implicitly re-fed).
        let doc = parse("> > foo\n> >\n> bar\n");
        assert_eq!(doc.nodes.len(), 1);
        match &doc.nodes[0] {
            Block::BlockQuote { content, .. } => {
                assert_eq!(content.len(), 2);
                match (&content[0], &content[1]) {
                    (
                        Block::BlockQuote { content: inner, .. },
                        Block::Paragraph { content: p, .. },
                    ) => {
                        assert_eq!(inner.len(), 1);
                        match &inner[0] {
                            Block::Paragraph { content, .. } => {
                                assert_eq!(joined_text(content), "foo");
                            }
                            other => panic!("expected inner paragraph, got {other:?}"),
                        }
                        assert_eq!(joined_text(p), "bar");
                    }
                    other => panic!("expected inner quote + outer paragraph, got {other:?}"),
                }
            }
            other => panic!("expected blockquote, got {other:?}"),
        }
    }

    #[test]
    fn blockquote_nested_heading_outer_paragraph_transition() {
        // Container-depth transition: the nested quote ends in a heading
        // (non-paragraph leaf), so the shallower marker line "outer" opens
        // a fresh paragraph at the outer depth and the marker-less
        // "outside" lazily continues it.
        let doc = parse("> > # h\n> outer\noutside\n");
        assert_eq!(doc.nodes.len(), 1);
        match &doc.nodes[0] {
            Block::BlockQuote { content, .. } => {
                assert_eq!(content.len(), 2);
                match (&content[0], &content[1]) {
                    (
                        Block::BlockQuote { content: inner, .. },
                        Block::Paragraph { content: p, .. },
                    ) => {
                        assert_eq!(inner.len(), 1);
                        match &inner[0] {
                            Block::Heading {
                                level, content: h, ..
                            } => {
                                assert_eq!(*level, 1);
                                assert_eq!(joined_text(h), "h");
                            }
                            other => panic!("expected inner heading, got {other:?}"),
                        }
                        assert_eq!(joined_text(p), "outer\noutside");
                    }
                    other => panic!("expected inner quote + outer paragraph, got {other:?}"),
                }
            }
            other => panic!("expected blockquote, got {other:?}"),
        }
    }

    #[test]
    fn blockquote_nested_open_fence_outer_paragraph_transition() {
        // The inner quote's unclosed fence is owned by the inner container:
        // "outer" ends that container, so the fence must not keep blocking
        // the outer paragraph chain.
        let doc = parse("> > ```\n> > code\n> outer\noutside\n");
        assert_eq!(doc.nodes.len(), 1);
        match &doc.nodes[0] {
            Block::BlockQuote { content, .. } => {
                assert_eq!(content.len(), 2);
                match (&content[0], &content[1]) {
                    (
                        Block::BlockQuote { content: inner, .. },
                        Block::Paragraph { content: p, .. },
                    ) => {
                        assert_eq!(inner.len(), 1);
                        match &inner[0] {
                            Block::CodeBlock { source, .. } => assert_eq!(source, "code"),
                            other => panic!("expected inner code block, got {other:?}"),
                        }
                        assert_eq!(joined_text(p), "outer\noutside");
                    }
                    other => panic!("expected inner quote + outer paragraph, got {other:?}"),
                }
            }
            other => panic!("expected blockquote, got {other:?}"),
        }
    }

    #[test]
    fn blockquote_nested_closed_fence_outer_paragraph_transition() {
        // The nested quote ends in a closed fenced code block; the closing
        // fence leaves a non-paragraph leaf that must not swallow the outer
        // paragraph.
        let doc = parse("> > ```\n> > code\n> > ```\n> outer\noutside\n");
        assert_eq!(doc.nodes.len(), 1);
        match &doc.nodes[0] {
            Block::BlockQuote { content, .. } => {
                assert_eq!(content.len(), 2);
                match (&content[0], &content[1]) {
                    (
                        Block::BlockQuote { content: inner, .. },
                        Block::Paragraph { content: p, .. },
                    ) => {
                        assert_eq!(inner.len(), 1);
                        match &inner[0] {
                            Block::CodeBlock { source, .. } => assert_eq!(source, "code"),
                            other => panic!("expected inner code block, got {other:?}"),
                        }
                        assert_eq!(joined_text(p), "outer\noutside");
                    }
                    other => panic!("expected inner quote + outer paragraph, got {other:?}"),
                }
            }
            other => panic!("expected blockquote, got {other:?}"),
        }
    }

    #[test]
    fn blockquote_nested_list_outer_paragraph_transition() {
        // Generic non-paragraph transition: the nested quote ends in a
        // list, and the shallower line must start an outer paragraph
        // instead of continuing the list chain.
        let doc = parse("> > - item\n> outer\noutside\n");
        assert_eq!(doc.nodes.len(), 1);
        match &doc.nodes[0] {
            Block::BlockQuote { content, .. } => {
                assert_eq!(content.len(), 2);
                match (&content[0], &content[1]) {
                    (
                        Block::BlockQuote { content: inner, .. },
                        Block::Paragraph { content: p, .. },
                    ) => {
                        assert_eq!(inner.len(), 1);
                        match &inner[0] {
                            Block::UnorderedList { items, .. } => assert_eq!(items.len(), 1),
                            other => panic!("expected inner list, got {other:?}"),
                        }
                        assert_eq!(joined_text(p), "outer\noutside");
                    }
                    other => panic!("expected inner quote + outer paragraph, got {other:?}"),
                }
            }
            other => panic!("expected blockquote, got {other:?}"),
        }
    }

    #[test]
    fn blockquote_nested() {
        // Nested blockquotes with >> marker
        let doc = parse("> outer\n>> inner\n");
        match &doc.nodes[0] {
            Block::BlockQuote { content, .. } => {
                assert_eq!(content.len(), 2);
                match (&content[0], &content[1]) {
                    (
                        Block::Paragraph { content: c1, .. },
                        Block::BlockQuote { content: c2, .. },
                    ) => {
                        assert_eq!(joined_text(c1), "outer");
                        assert_eq!(c2.len(), 1);
                        match &c2[0] {
                            Block::Paragraph { content: c3, .. } => {
                                assert_eq!(joined_text(c3), "inner");
                            }
                            _ => panic!("expected paragraph"),
                        }
                    }
                    _ => panic!("expected paragraph + blockquote"),
                }
            }
            other => panic!("expected blockquote, got {other:?}"),
        }
    }

    #[test]
    fn blockquote_inline_parsing() {
        // Inline parsing works inside blockquotes
        let doc = parse("> **bold** and `code` and [link](url)\n");
        match &doc.nodes[0] {
            Block::BlockQuote { content, .. } => {
                assert_eq!(content.len(), 1);
                match &content[0] {
                    Block::Paragraph { content, .. } => {
                        // Should have Strong, Code, Link inlines
                        let has_strong = content.iter().any(|i| matches!(i, Inline::Strong { .. }));
                        let has_code = content.iter().any(|i| matches!(i, Inline::Code { .. }));
                        let has_link = content.iter().any(|i| matches!(i, Inline::Link { .. }));
                        assert!(has_strong, "missing strong");
                        assert!(has_code, "missing code");
                        assert!(has_link, "missing link");
                    }
                    other => panic!("expected paragraph, got {other:?}"),
                }
            }
            other => panic!("expected blockquote, got {other:?}"),
        }
    }

    #[test]
    fn blockquote_with_list() {
        // Lists work inside blockquotes
        let doc = parse("> - one\n> - two\n");
        match &doc.nodes[0] {
            Block::BlockQuote { content, .. } => {
                assert_eq!(content.len(), 1);
                match &content[0] {
                    Block::UnorderedList { items, .. } => {
                        assert_eq!(items.len(), 2);
                    }
                    other => panic!("expected unordered list, got {other:?}"),
                }
            }
            other => panic!("expected blockquote, got {other:?}"),
        }
    }

    #[test]
    fn blockquote_with_ordered_list() {
        // Ordered lists work inside blockquotes
        let doc = parse("> 1. one\n> 2. two\n");
        match &doc.nodes[0] {
            Block::BlockQuote { content, .. } => {
                assert_eq!(content.len(), 1);
                match &content[0] {
                    Block::OrderedList { items, .. } => {
                        assert_eq!(items.len(), 2);
                    }
                    other => panic!("expected ordered list, got {other:?}"),
                }
            }
            other => panic!("expected blockquote, got {other:?}"),
        }
    }

    #[test]
    fn blockquote_source_spans_correct() {
        // Source spans should be correct for indented blockquotes
        let doc = parse(" > foo\n");
        match &doc.nodes[0] {
            Block::BlockQuote { content, span, .. } => {
                // Blockquote span should cover the entire line including marker
                assert!(span.start <= span.end);
                // Content paragraph span should point to "foo" (after marker)
                if let Block::Paragraph { span: p_span, .. } = &content[0] {
                    assert!(p_span.start >= 2); // after " >"
                    assert!(p_span.end <= 7); // "foo" ends at 7
                }
            }
            other => panic!("expected blockquote, got {other:?}"),
        }
    }

    #[test]
    fn blockquote_utf8_content() {
        // UTF-8 content in blockquotes
        let doc = parse("> 한글 테스트\n> hello 😀 world\n");
        match &doc.nodes[0] {
            Block::BlockQuote { content, .. } => {
                assert_eq!(content.len(), 1);
                match &content[0] {
                    Block::Paragraph { content, .. } => {
                        let text = joined_text(content);
                        assert!(text.contains("한글"));
                        assert!(text.contains("😀"));
                    }
                    other => panic!("expected paragraph, got {other:?}"),
                }
            }
            other => panic!("expected blockquote, got {other:?}"),
        }
    }

    #[test]
    fn blockquote_crlf() {
        // CRLF line endings. The quoted result must be structurally identical
        // to the unquoted CRLF paragraph: the `\r` byte of the terminator is
        // folded into the preceding text node, and the SoftBreak spans the
        // `\n` byte alone (differential parity with unquoted input).
        let doc = parse("> foo\r\n> bar\r\n");
        match &doc.nodes[0] {
            Block::BlockQuote { content, .. } => {
                assert_eq!(content.len(), 1);
                match &content[0] {
                    Block::Paragraph { content, .. } => {
                        assert_eq!(joined_text(content), "foo\r\nbar");
                    }
                    other => panic!("expected paragraph, got {other:?}"),
                }
            }
            other => panic!("expected blockquote, got {other:?}"),
        }
    }

    #[test]
    fn blockquote_transition_to_heading() {
        // Blockquote ends before a heading
        let doc = parse("> foo\n# heading\n");
        assert_eq!(doc.nodes.len(), 2);
        match (&doc.nodes[0], &doc.nodes[1]) {
            (Block::BlockQuote { content, .. }, Block::Heading { .. }) => {
                assert_eq!(content.len(), 1);
                match &content[0] {
                    Block::Paragraph { content, .. } => {
                        assert_eq!(joined_text(content), "foo");
                    }
                    other => panic!("expected paragraph, got {other:?}"),
                }
            }
            other => panic!("expected blockquote + heading, got {other:?}"),
        }
    }

    #[test]
    fn blockquote_transition_to_list() {
        // Blockquote ends before a list
        let doc = parse("> foo\n- item\n");
        assert_eq!(doc.nodes.len(), 2);
        match (&doc.nodes[0], &doc.nodes[1]) {
            (Block::BlockQuote { content, .. }, Block::UnorderedList { .. }) => {
                assert_eq!(content.len(), 1);
            }
            other => panic!("expected blockquote + list, got {other:?}"),
        }
    }

    #[test]
    fn blockquote_lazy_continuation_heading_negative() {
        // CommonMark: lazy continuation only applies to paragraphs, not headings
        // > # heading
        // plain text
        // Expected: BlockQuote(Heading) + Paragraph (not absorbed)
        let doc = parse("> # heading\nplain text\n");
        assert_eq!(doc.nodes.len(), 2);
        match (&doc.nodes[0], &doc.nodes[1]) {
            (
                Block::BlockQuote {
                    content: bq_content,
                    ..
                },
                Block::Paragraph {
                    content: para_content,
                    ..
                },
            ) => {
                assert_eq!(bq_content.len(), 1);
                match &bq_content[0] {
                    Block::Heading { .. } => {} // heading inside blockquote
                    other => panic!("expected heading inside blockquote, got {other:?}"),
                }
                assert_eq!(joined_text(para_content), "plain text"); // outside paragraph
            }
            other => panic!("expected blockquote + paragraph, got {other:?}"),
        }
    }

    #[test]
    fn blockquote_lazy_continuation_fenced_code_negative() {
        // CommonMark Example 237: fenced code blocks are not lazy continuation
        // > ```
        // foo
        // Expected: BlockQuote(CodeBlock) + Paragraph (not absorbed)
        let doc = parse("> ```\nfoo\n");
        assert_eq!(doc.nodes.len(), 2);
        match (&doc.nodes[0], &doc.nodes[1]) {
            (
                Block::BlockQuote {
                    content: bq_content,
                    ..
                },
                Block::Paragraph {
                    content: para_content,
                    ..
                },
            ) => {
                assert_eq!(bq_content.len(), 1);
                match &bq_content[0] {
                    Block::CodeBlock { .. } => {} // code block inside blockquote
                    other => panic!("expected code block inside blockquote, got {other:?}"),
                }
                assert_eq!(joined_text(para_content), "foo"); // outside paragraph
            }
            other => panic!("expected blockquote + paragraph, got {other:?}"),
        }
    }

    #[test]
    fn blockquote_lazy_continuation_positive_exact() {
        // Positive case: paragraph continuation should work
        let doc = parse("> foo\nbar\n");
        assert_eq!(doc.nodes.len(), 1);
        match &doc.nodes[0] {
            Block::BlockQuote { content, .. } => {
                assert_eq!(content.len(), 1);
                match &content[0] {
                    Block::Paragraph { content, span, .. } => {
                        assert_eq!(joined_text(content), "foo\nbar");
                        // Verify source spans: blockquote spans full input
                        // Blockquote span starts at '>' (position 0) but content is at position 2
                        // The BlockQuote span itself covers from first.raw_start to end_span
                        assert!(span.end > span.start);
                    }
                    other => panic!("expected paragraph, got {other:?}"),
                }
            }
            other => panic!("expected blockquote, got {other:?}"),
        }
    }

    #[test]
    fn blockquote_softbreak_source_span_lf() {
        // SoftBreak span should point to actual LF in source, not across stripped markers
        let source = "> foo\n> bar\n";
        let doc = parse(source);
        match &doc.nodes[0] {
            Block::BlockQuote { content, .. } => {
                match &content[0] {
                    Block::Paragraph { content, .. } => {
                        // Find the SoftBreak
                        let soft_breaks: Vec<_> = content
                            .iter()
                            .filter_map(|i| {
                                if let Inline::SoftBreak { span } = i {
                                    Some(span)
                                } else {
                                    None
                                }
                            })
                            .collect();
                        assert_eq!(soft_breaks.len(), 1);
                        let sb = soft_breaks[0];
                        // Span should point to "\n" at position 6..7 in source
                        assert_eq!(&source[sb.start..sb.end], "\n");
                    }
                    other => panic!("expected paragraph, got {other:?}"),
                }
            }
            other => panic!("expected blockquote, got {other:?}"),
        }
    }

    #[test]
    fn blockquote_softbreak_source_span_crlf() {
        // CRLF line endings: SoftBreak should point to normalized LF (parser normalizes CRLF)
        let source = "> foo\r\n> bar\r\n";
        let doc = parse(source);
        match &doc.nodes[0] {
            Block::BlockQuote { content, .. } => {
                match &content[0] {
                    Block::Paragraph { content, .. } => {
                        let soft_breaks: Vec<_> = content
                            .iter()
                            .filter_map(|i| {
                                if let Inline::SoftBreak { span } = i {
                                    Some(span)
                                } else {
                                    None
                                }
                            })
                            .collect();
                        assert_eq!(soft_breaks.len(), 1);
                        let sb = soft_breaks[0];
                        // Parser normalizes CRLF to LF, so span points to "\n"
                        assert_eq!(&source[sb.start..sb.end], "\n");
                    }
                    other => panic!("expected paragraph, got {other:?}"),
                }
            }
            other => panic!("expected blockquote, got {other:?}"),
        }
    }

    #[test]
    fn blockquote_indented_content_span_exact() {
        // Content span should point exactly to "foo" after " > " marker
        // Note: paragraph span includes the line ending
        let source = " > foo\n";
        let doc = parse(source);
        match &doc.nodes[0] {
            Block::BlockQuote { content, .. } => match &content[0] {
                Block::Paragraph { span, .. } => {
                    let fragment = &source[span.start..span.end];
                    assert_eq!(fragment.trim_end(), "foo");
                }
                other => panic!("expected paragraph, got {other:?}"),
            },
            other => panic!("expected blockquote, got {other:?}"),
        }
    }

    #[test]
    fn blockquote_multiline_code_span_joins_and_source_span_exact() {
        // A code span delimited inside a block quote may span quoted lines:
        // the fragments are joined into one logical inline run, so the span
        // covers the whole `..`..`` construct in the original source.
        let source = "> a ``x\n> y``\n";
        let doc = parse(source);
        match &doc.nodes[0] {
            Block::BlockQuote { content, .. } => match &content[0] {
                Block::Paragraph { content, .. } => {
                    assert_eq!(content.len(), 2);
                    match &content[1] {
                        Inline::Code { content, span } => {
                            assert_eq!(content, "x y");
                            // The source span of a multi-line element covers
                            // the whole construct, including the `> ` marker
                            // of the referenced quoted lines.
                            assert_eq!(&source[span.start..span.end], "``x\n> y``");
                        }
                        other => panic!("expected code span, got {other:?}"),
                    }
                }
                other => panic!("expected paragraph, got {other:?}"),
            },
            other => panic!("expected blockquote, got {other:?}"),
        }
    }

    #[test]
    fn blockquote_multiline_emphasis_source_span_exact() {
        let source = "> *foo\n> bar*\n";
        let doc = parse(source);
        match &doc.nodes[0] {
            Block::BlockQuote { content, .. } => match &content[0] {
                Block::Paragraph { content, .. } => match &content[0] {
                    Inline::Emphasis {
                        content: inner,
                        span,
                    } => {
                        assert_eq!(joined_text(inner), "foo\nbar");
                        assert_eq!(&source[span.start..span.end], "*foo\n> bar*");
                    }
                    other => panic!("expected emphasis, got {other:?}"),
                },
                other => panic!("expected paragraph, got {other:?}"),
            },
            other => panic!("expected blockquote, got {other:?}"),
        }
    }

    #[test]
    fn blockquote_multiline_strong_source_span_exact() {
        let source = "> **foo\n> bar**\n";
        let doc = parse(source);
        match &doc.nodes[0] {
            Block::BlockQuote { content, .. } => match &content[0] {
                Block::Paragraph { content, .. } => match &content[0] {
                    Inline::Strong {
                        content: inner,
                        span,
                    } => {
                        assert_eq!(joined_text(inner), "foo\nbar");
                        assert_eq!(&source[span.start..span.end], "**foo\n> bar**");
                    }
                    other => panic!("expected strong, got {other:?}"),
                },
                other => panic!("expected paragraph, got {other:?}"),
            },
            other => panic!("expected blockquote, got {other:?}"),
        }
    }

    #[test]
    fn blockquote_multiline_link_label_source_span_exact() {
        let source = "> [foo\n> bar](https://example.com)\n";
        let doc = parse(source);
        match &doc.nodes[0] {
            Block::BlockQuote { content, .. } => match &content[0] {
                Block::Paragraph { content, .. } => match &content[0] {
                    Inline::Link {
                        content: inner,
                        destination,
                        span,
                    } => {
                        assert_eq!(joined_text(inner), "foo\nbar");
                        assert_eq!(destination, "https://example.com");
                        assert_eq!(
                            &source[span.start..span.end],
                            "[foo\n> bar](https://example.com)"
                        );
                    }
                    other => panic!("expected link, got {other:?}"),
                },
                other => panic!("expected paragraph, got {other:?}"),
            },
            other => panic!("expected blockquote, got {other:?}"),
        }
    }

    #[test]
    fn blockquote_multiline_inlines_match_unquoted_paragraph() {
        // Differential check: quoting a paragraph must not change its inline
        // structure (kinds and joined text), and the source span of every
        // inline element must slice to the same bytes as in the unquoted
        // paragraph once the quote markers are skipped.
        for (quoted, plain) in [
            ("> a *b* c\n> d\n", "a *b* c\nd\n"),
            ("> a ``x\n> y`` z\n", "a ``x\ny`` z\n"),
            ("> *x y\n> z*\n", "*x y\nz*\n"),
            ("> [a\n> b](u)\n", "[a\nb](u)\n"),
            ("> x  \n> y\n", "x  \ny\n"),
            ("> x\\\n> y\n", "x\\\ny\n"),
            // Leading whitespace on continuation lines is inline content
            // (e.g. the text of a code span that spans lines), not block
            // syntax: the quoted paragraph must carry it exactly like the
            // unquoted one.
            ("> `foo\n>   bar`\n", "`foo\n  bar`\n"),
            ("> **foo\n>   bar**\n", "**foo\n  bar**\n"),
            ("> foo\n>   bar\n", "foo\n  bar\n"),
            ("> *a\n>     b*\n", "*a\n    b*\n"),
            ("> `코드\n>   이모지 😀`\n", "`코드\n  이모지 😀`\n"),
        ] {
            let plain_doc = parse(plain);
            let quoted_doc = parse(quoted);
            let plain_inlines = match &plain_doc.nodes[0] {
                Block::Paragraph { content, .. } => content,
                other => panic!("expected paragraph, got {other:?}"),
            };
            let quoted_paragraph = match &quoted_doc.nodes[0] {
                Block::BlockQuote { content, .. } => &content[0],
                other => panic!("expected blockquote, got {other:?}"),
            };
            let Block::Paragraph {
                content: quoted_inlines,
                ..
            } = quoted_paragraph
            else {
                panic!("expected paragraph, got {quoted_paragraph:?}");
            };

            let plain_profile = inline_span_profile(plain_inlines, plain);
            // Multi-line elements spliced across quoted lines carry the
            // `> ` marker bytes of the joined lines inside their source
            // span; stripping those per-line prefixes reproduces the plain
            // fragment the element covers in the unquoted paragraph.
            let quoted_profile = inline_span_profile(quoted_inlines, quoted)
                .into_iter()
                .map(|(kind, slice)| (kind, slice.replace("\r\n> ", "\r\n").replace("\n> ", "\n")))
                .collect::<Vec<_>>();
            assert_eq!(quoted_profile, plain_profile, "source: {quoted:?}");
        }
    }

    #[test]
    fn blockquote_code_span_multiline_whitespace_content() {
        // Differential content check: the code text of a span that crosses
        // quoted lines must equal the unquoted one - the whitespace after the
        // stripped quote markers is inline content (code text), not paragraph
        // indentation, and must survive quoting exactly once.
        for (quoted, plain, expected) in [
            ("> `foo\n>   bar`\n", "`foo\n  bar`\n", "foo   bar"),
            (
                "> `코드\n>   이모지 😀`\n",
                "`코드\n  이모지 😀`\n",
                "코드   이모지 😀",
            ),
        ] {
            let quoted_doc = parse(quoted);
            let quoted_code = match &quoted_doc.nodes[0] {
                Block::BlockQuote { content, .. } => &content[0],
                other => panic!("expected blockquote, got {other:?}"),
            };
            let Block::Paragraph { content, .. } = quoted_code else {
                panic!("expected paragraph, got {quoted_code:?}");
            };
            let [Inline::Code { content: c1, .. }] = &content[..] else {
                panic!("expected single code span, got {content:?}");
            };
            let plain_doc = parse(plain);
            let Block::Paragraph { content, .. } = &plain_doc.nodes[0] else {
                panic!("expected paragraph, got {:?}", plain_doc.nodes[0]);
            };
            let [Inline::Code { content: c2, .. }] = &content[..] else {
                panic!("expected single code span, got {content:?}");
            };
            assert_eq!(c1, expected, "source: {quoted:?}");
            assert_eq!(c2, expected, "source: {plain:?}");
        }
    }

    #[test]
    fn blockquote_list_item_lazy_continuation_absorbed() {
        // A marker-less line at (or beyond) the item's content column is
        // valid paragraph continuation text for the item paragraph and is
        // absorbed; the content column itself is stripped by the list parser.
        let doc = parse("> - foo\n  bar\n");
        assert_eq!(doc.nodes.len(), 1);
        match &doc.nodes[0] {
            Block::BlockQuote { content, .. } => match &content[0] {
                Block::UnorderedList { items, .. } => {
                    assert_eq!(items.len(), 1);
                    match &items[0].content[0] {
                        Block::Paragraph { content, .. } => {
                            assert_eq!(joined_text(content), "foo\nbar");
                        }
                        other => panic!("expected item paragraph, got {other:?}"),
                    }
                }
                other => panic!("expected list, got {other:?}"),
            },
            other => panic!("expected blockquote, got {other:?}"),
        }
    }

    #[test]
    fn blockquote_sibling_list_item_lazy_rejected() {
        // A marker-less line that starts a sibling list block is not valid
        // paragraph continuation text: it stays outside the quote.
        let doc = parse("> - foo\n- bar\n");
        assert_eq!(doc.nodes.len(), 2);
        match (&doc.nodes[0], &doc.nodes[1]) {
            (Block::BlockQuote { content, .. }, Block::UnorderedList { items, .. }) => {
                assert_eq!(content.len(), 1);
                assert_eq!(items.len(), 1);
            }
            other => panic!("expected blockquote + list, got {other:?}"),
        }
    }

    #[test]
    fn blockquote_sibling_ordered_item_lazy_rejected() {
        // "  2. bar" after "> 1. foo" starts a new ordered-list
        // item/block candidate (not paragraph continuation text) and stays
        // outside the quote as its own ordered list with start = 2.
        let doc = parse("> 1. foo\n  2. bar\n");
        assert_eq!(doc.nodes.len(), 2);
        match (&doc.nodes[0], &doc.nodes[1]) {
            (Block::BlockQuote { content, .. }, Block::OrderedList { items, start, .. }) => {
                assert_eq!(content.len(), 1);
                assert_eq!(*start, 2);
                assert_eq!(items.len(), 1);
            }
            other => panic!("expected blockquote + ordered list, got {other:?}"),
        }
    }

    #[test]
    fn blockquote_nested_quote_list_item_lazy_continuation_absorbed() {
        // The content-column rule holds across quote nesting: a marker-less
        // line at the item column continues the item inside the nested quote.
        let doc = parse("> > - foo\n  bar\n");
        assert_eq!(doc.nodes.len(), 1);
        match &doc.nodes[0] {
            Block::BlockQuote { content, .. } => match &content[0] {
                Block::BlockQuote { content: inner, .. } => match &inner[0] {
                    Block::UnorderedList { items, .. } => {
                        assert_eq!(items.len(), 1);
                        match &items[0].content[0] {
                            Block::Paragraph { content, .. } => {
                                assert_eq!(joined_text(content), "foo\nbar");
                            }
                            other => panic!("expected item paragraph, got {other:?}"),
                        }
                    }
                    other => panic!("expected list, got {other:?}"),
                },
                other => panic!("expected nested quote, got {other:?}"),
            },
            other => panic!("expected blockquote, got {other:?}"),
        }
    }

    #[test]
    fn blockquote_list_item_non_paragraph_leaf_lazy_rejected() {
        // When the trailing item's leaf is not an open paragraph (here: a
        // heading), a marker-less line is not valid continuation text and
        // stays outside the quote.
        let doc = parse("> - # h\nbar\n");
        assert_eq!(doc.nodes.len(), 2);
        match (&doc.nodes[0], &doc.nodes[1]) {
            (Block::BlockQuote { content, .. }, Block::Paragraph { content: p, .. }) => {
                assert_eq!(content.len(), 1);
                match &content[0] {
                    Block::UnorderedList { items, .. } => {
                        assert_eq!(items.len(), 1);
                        match &items[0].content[0] {
                            Block::Heading { level, .. } => assert_eq!(*level, 1),
                            other => panic!("expected heading, got {other:?}"),
                        }
                    }
                    other => panic!("expected list, got {other:?}"),
                }
                assert_eq!(joined_text(p), "bar");
            }
            other => panic!("expected blockquote + paragraph, got {other:?}"),
        }
    }

    #[test]
    fn blockquote_list_different_marker_structural_sibling() {
        // A quoted line with a different marker ends the trailing list and
        // opens a fresh list block at the quote level.
        let doc = parse("> - a\n> * b\n");
        match &doc.nodes[0] {
            Block::BlockQuote { content, .. } => {
                assert_eq!(content.len(), 2);
                match (&content[0], &content[1]) {
                    (
                        Block::UnorderedList { items: i0, .. },
                        Block::UnorderedList { items: i1, .. },
                    ) => {
                        assert_eq!(i0.len(), 1);
                        assert_eq!(i1.len(), 1);
                        let joined = |items: &[ListItem]| match &items[0].content[0] {
                            Block::Paragraph { content, .. } => joined_text(content),
                            other => panic!("expected paragraph, got {other:?}"),
                        };
                        assert_eq!(joined(i0), "a");
                        assert_eq!(joined(i1), "b");
                    }
                    other => panic!("expected two lists, got {other:?}"),
                }
            }
            other => panic!("expected blockquote, got {other:?}"),
        }
    }

    #[test]
    fn blockquote_ordered_list_same_delimiter_structural_items() {
        // Quoted marker lines with the same delimiter continue the same
        // ordered list; the item ordinals may differ from the start.
        let doc = parse("> 1. a\n> 9. b\n");
        match &doc.nodes[0] {
            Block::BlockQuote { content, .. } => match &content[0] {
                Block::OrderedList { items, start, .. } => {
                    assert_eq!(*start, 1);
                    assert_eq!(items.len(), 2);
                }
                other => panic!("expected ordered list, got {other:?}"),
            },
            other => panic!("expected blockquote, got {other:?}"),
        }
    }

    #[test]
    fn blockquote_list_item_fenced_block() {
        // A fence opened by the item's first block is owned by the item:
        // item content lines are fence content until the indented structural
        // closer, and a plain item line after the fence reopens a paragraph.
        let doc = parse("> - ```\n>   x\n>   ```\n>   y\n");
        match &doc.nodes[0] {
            Block::BlockQuote { content, .. } => match &content[0] {
                Block::UnorderedList { items, .. } => {
                    assert_eq!(items.len(), 1);
                    assert_eq!(items[0].content.len(), 2);
                    match &items[0].content[0] {
                        Block::CodeBlock { source, .. } => assert_eq!(source, "x"),
                        other => panic!("expected code block, got {other:?}"),
                    }
                    match &items[0].content[1] {
                        Block::Paragraph { content, .. } => {
                            assert_eq!(joined_text(content), "y");
                        }
                        other => panic!("expected paragraph, got {other:?}"),
                    }
                }
                other => panic!("expected list, got {other:?}"),
            },
            other => panic!("expected blockquote, got {other:?}"),
        }
    }

    #[test]
    fn blockquote_lazy_line_flips_leaf_after_deeper_list() {
        // A depth-0 line starting a sibling block (a list) ends the deeper
        // open paragraph: a marker-less line after it must stay outside the
        // quote, mirroring the trailing-chain classification.
        let doc = parse("> > foo\n> x\n> - a\nbar\n");
        assert_eq!(doc.nodes.len(), 2);
        match (&doc.nodes[0], &doc.nodes[1]) {
            (Block::BlockQuote { content, .. }, Block::Paragraph { content: p, .. }) => {
                assert_eq!(content.len(), 2);
                assert_eq!(joined_text(p), "bar");
            }
            other => panic!("expected blockquote + paragraph, got {other:?}"),
        }
    }

    #[test]
    fn blockquote_fenced_code_multiline_lazy_negative() {
        // CommonMark Example 237 family: content lines inside an open quoted
        // fenced code block must not flip the parser back into paragraph
        // state, so a marker-less line after them stays outside the quote.
        let doc = parse("> ```\n> code\noutside\n");
        assert_eq!(doc.nodes.len(), 2);
        match (&doc.nodes[0], &doc.nodes[1]) {
            (Block::BlockQuote { content, .. }, Block::Paragraph { content: p, .. }) => {
                assert_eq!(content.len(), 1);
                match &content[0] {
                    Block::CodeBlock { source, .. } => assert_eq!(source, "code"),
                    other => panic!("expected code block inside blockquote, got {other:?}"),
                }
                assert_eq!(joined_text(p), "outside");
            }
            other => panic!("expected blockquote + paragraph, got {other:?}"),
        }
    }

    #[test]
    fn blockquote_fenced_code_closed_lazy_negative() {
        // A closed quoted fence still leaves FencedCodeBlock open when a
        // marker-less line follows: the trailer must not be absorbed.
        let doc = parse("> ```\n> code\n> ```\noutside\n");
        assert_eq!(doc.nodes.len(), 2);
        match (&doc.nodes[0], &doc.nodes[1]) {
            (Block::BlockQuote { content, .. }, Block::Paragraph { content: p, .. }) => {
                assert_eq!(content.len(), 1);
                match &content[0] {
                    Block::CodeBlock { source, .. } => assert_eq!(source, "code"),
                    other => panic!("expected code block inside blockquote, got {other:?}"),
                }
                assert_eq!(joined_text(p), "outside");
            }
            other => panic!("expected blockquote + paragraph, got {other:?}"),
        }
    }

    #[test]
    fn blockquote_lazy_continuation_commonmark_example_238() {
        // CommonMark Example 238: indentation participates in continuation
        // classification. "    - bar" is indented four columns, so it cannot
        // start a new block and continues the quoted "foo" paragraph as
        // plain text instead of beginning a list item.
        let doc = parse("> foo\n    - bar\n");
        assert_eq!(doc.nodes.len(), 1);
        match &doc.nodes[0] {
            Block::BlockQuote { content, .. } => {
                assert_eq!(content.len(), 1);
                match &content[0] {
                    Block::Paragraph { content, .. } => {
                        assert_eq!(joined_text(content), "foo\n- bar");
                    }
                    other => panic!("expected paragraph, got {other:?}"),
                }
            }
            other => panic!("expected blockquote, got {other:?}"),
        }
    }

    #[test]
    fn blockquote_hardbreak_two_spaces_span_exact() {
        // Two trailing spaces in a quoted line must yield a HardBreak whose
        // span covers exactly the trailing spaces and the LF (CommonMark
        // paragraph semantics, unquoted contract).
        let source = "> foo  \n> bar\n";
        let doc = parse(source);
        match &doc.nodes[0] {
            Block::BlockQuote { content, .. } => match &content[0] {
                Block::Paragraph { content, .. } => {
                    assert_eq!(content.len(), 3);
                    assert_text(&content[0], "foo");
                    match &content[1] {
                        Inline::HardBreak { span } => {
                            assert_eq!(&source[span.start..span.end], "  \n");
                        }
                        other => panic!("expected HardBreak, got {other:?}"),
                    }
                    assert_text(&content[2], "bar");
                }
                other => panic!("expected paragraph, got {other:?}"),
            },
            other => panic!("expected blockquote, got {other:?}"),
        }
    }

    #[test]
    fn blockquote_hardbreak_backslash_span_exact() {
        // A trailing backslash in a quoted line must yield a HardBreak whose
        // span covers exactly the backslash and the LF.
        let source = "> foo\\\n> bar\n";
        let doc = parse(source);
        match &doc.nodes[0] {
            Block::BlockQuote { content, .. } => match &content[0] {
                Block::Paragraph { content, .. } => {
                    assert_eq!(content.len(), 3);
                    assert_text(&content[0], "foo");
                    match &content[1] {
                        Inline::HardBreak { span } => {
                            assert_eq!(&source[span.start..span.end], "\\\n");
                        }
                        other => panic!("expected HardBreak, got {other:?}"),
                    }
                    assert_text(&content[2], "bar");
                }
                other => panic!("expected paragraph, got {other:?}"),
            },
            other => panic!("expected blockquote, got {other:?}"),
        }
    }

    /// Normalize an inline sequence to (kind, text) labels so quoted and
    /// unquoted paragraphs can be compared structurally.
    fn break_kinds(inlines: &[Inline]) -> Vec<String> {
        inlines
            .iter()
            .map(|inline| match inline {
                Inline::Text { content, .. } => format!("T({content})"),
                Inline::HardBreak { .. } => "HB".to_string(),
                Inline::SoftBreak { .. } => "SB".to_string(),
                other => panic!("unexpected inline for differential comparison: {other:?}"),
            })
            .collect()
    }

    #[test]
    fn blockquote_break_differential_unquoted_quoted() {
        // Quoted multiline paragraphs must produce the same inline node
        // sequence as their unquoted twins (two-space HardBreak, backslash
        // HardBreak, plain SoftBreak); only source span offsets differ.
        for (unquoted, quoted) in [
            ("foo  \nbar\n", "> foo  \n> bar\n"),
            ("foo\\\nbar\n", "> foo\\\n> bar\n"),
            ("foo\nbar\n", "> foo\n> bar\n"),
        ] {
            let unquoted_doc = parse(unquoted);
            let unquoted_content = paragraph_inlines(&unquoted_doc);
            let quoted_doc = parse(quoted);
            match &quoted_doc.nodes[0] {
                Block::BlockQuote { content, .. } => {
                    let quoted_content = match &content[0] {
                        Block::Paragraph { content, .. } => content,
                        other => panic!("expected paragraph, got {other:?}"),
                    };
                    assert_eq!(
                        break_kinds(quoted_content),
                        break_kinds(unquoted_content),
                        "mismatch for unquoted={unquoted:?} quoted={quoted:?}"
                    );
                }
                other => panic!("expected blockquote, got {other:?}"),
            }
        }
    }

    #[test]
    fn blockquote_crlf_differs_only_in_span_offsets() {
        // CRLF multiline quotes follow the unquoted CRLF contract exactly:
        // the `\r` byte is folded into the preceding text node and the
        // SoftBreak spans only the `\n` byte (items 13 and 16).
        let unquoted = parse("foo\r\nbar\r\n");
        let quoted = parse("> foo\r\n> bar\r\n");

        let unquoted_content = paragraph_inlines(&unquoted);
        assert_eq!(joined_text(unquoted_content), "foo\r\nbar");

        let quoted_content = match &quoted.nodes[0] {
            Block::BlockQuote { content, .. } => match &content[0] {
                Block::Paragraph { content, .. } => content,
                other => panic!("expected paragraph, got {other:?}"),
            },
            other => panic!("expected blockquote, got {other:?}"),
        };
        assert_eq!(joined_text(quoted_content), "foo\r\nbar");
        assert_eq!(break_kinds(quoted_content), break_kinds(unquoted_content));

        // Both SoftBreak spans point at the single LF byte.
        let unquoted_sb = match &unquoted_content[1] {
            Inline::SoftBreak { span } => *span,
            other => panic!("expected SoftBreak in unquoted paragraph, got {other:?}"),
        };
        let quoted_sb = match &quoted_content[1] {
            Inline::SoftBreak { span } => *span,
            other => panic!("expected SoftBreak in quoted paragraph, got {other:?}"),
        };
        assert_eq!(&"foo\r\nbar\r\n"[unquoted_sb.start..unquoted_sb.end], "\n");
        assert_eq!(&"> foo\r\n> bar\r\n"[quoted_sb.start..quoted_sb.end], "\n");
    }

    #[test]
    fn blockquote_indented_heading_lazy_rejected() {
        // A heading indented 1-3 spaces inside the quote still starts a
        // block (the same classification `SourceLine::starts_block` gives
        // its content line), so a following marker-less line is not
        // paragraph continuation text and stays outside the quote.
        for spaces in 0..=3usize {
            let source = format!("> foo\n> {}# h\nbar\n", " ".repeat(spaces));
            let doc = parse(&source);
            assert_eq!(
                doc.nodes.len(),
                2,
                "indent {spaces}: expected blockquote + outside paragraph for {source:?}"
            );
            match (&doc.nodes[0], &doc.nodes[1]) {
                (Block::BlockQuote { content, .. }, Block::Paragraph { content: p, .. }) => {
                    assert_eq!(content.len(), 2);
                    match (&content[0], &content[1]) {
                        (Block::Paragraph { content: p0, .. }, Block::Heading { level, .. }) => {
                            assert_eq!(joined_text(p0), "foo");
                            assert_eq!(*level, 1);
                        }
                        other => panic!("expected foo paragraph + heading, got {other:?}"),
                    }
                    assert_eq!(joined_text(p), "bar");
                }
                other => panic!("expected blockquote + paragraph, got {other:?}"),
            }
        }
    }

    #[test]
    fn blockquote_indented_heading_four_spaces_stays_paragraph() {
        // At four spaces of indent the content line cannot start a block
        // (this parser has no indented code blocks), so the marker-less
        // line stays a valid paragraph continuation, exactly as in the
        // unquoted equivalent (CommonMark 238).
        let unquoted = parse("foo\n    # h\nbar\n");
        let quoted = parse("> foo\n>     # h\nbar\n");
        let quoted_paragraph = match &quoted.nodes[0] {
            Block::BlockQuote { content, .. } => match &content[0] {
                Block::Paragraph { content, .. } => content,
                other => panic!("expected paragraph, got {other:?}"),
            },
            other => panic!("expected blockquote, got {other:?}"),
        };
        assert_eq!(
            joined_text(quoted_paragraph),
            joined_text(paragraph_inlines(&unquoted))
        );
        assert_eq!(quoted_paragraph.len(), paragraph_inlines(&unquoted).len());
    }

    #[test]
    fn blockquote_list_item_fence_opened_after_paragraph_lazy_rejected() {
        // A fence opened by a later item-content line is tracked on the
        // item: the following line is fence content (never a paragraph),
        // and the marker-less candidate cannot lazily continue it.
        let doc = parse("> - foo\n>   ```\n>   code\n  outside\n");
        assert_eq!(doc.nodes.len(), 2);
        match (&doc.nodes[0], &doc.nodes[1]) {
            (Block::BlockQuote { content, .. }, Block::Paragraph { content: p, .. }) => {
                assert_eq!(content.len(), 1);
                match &content[0] {
                    Block::UnorderedList { items, .. } => {
                        assert_eq!(items.len(), 1);
                        assert_eq!(items[0].content.len(), 2);
                        match (&items[0].content[0], &items[0].content[1]) {
                            (
                                Block::Paragraph { content: p0, .. },
                                Block::CodeBlock { source, .. },
                            ) => {
                                assert_eq!(joined_text(p0), "foo");
                                assert_eq!(*source, "code");
                            }
                            other => panic!("expected paragraph + code block, got {other:?}"),
                        }
                    }
                    other => panic!("expected list, got {other:?}"),
                }
                assert_eq!(joined_text(p), "outside");
            }
            other => panic!("expected blockquote + paragraph, got {other:?}"),
        }
    }

    #[test]
    fn blockquote_list_item_indented_heading_lazy_rejected() {
        // Item content is classified in item-relative coordinates: after
        // the item's content column is stripped, "  # h" keeps two columns
        // of indent and is a heading inside the item (exactly what the
        // global list parser produces for the unquoted input), so the
        // marker-less line is not a paragraph continuation.
        let unquoted = parse("- foo\n    # h\n");
        let quoted = parse("> - foo\n>     # h\n  outside\n");
        assert_eq!(quoted.nodes.len(), 2);
        match (&quoted.nodes[0], &quoted.nodes[1]) {
            (Block::BlockQuote { content, .. }, Block::Paragraph { content: p, .. }) => {
                assert_eq!(content.len(), 1);
                match &content[0] {
                    Block::UnorderedList { items, .. } => {
                        assert_eq!(items.len(), 1);
                        assert_eq!(items[0].content.len(), 2);
                        match (&items[0].content[0], &items[0].content[1]) {
                            (
                                Block::Paragraph { content: p0, .. },
                                Block::Heading { level, .. },
                            ) => {
                                assert_eq!(joined_text(p0), "foo");
                                assert_eq!(*level, 1);
                            }
                            other => panic!("expected paragraph + heading, got {other:?}"),
                        }
                    }
                    other => panic!("expected list, got {other:?}"),
                }
                assert_eq!(joined_text(p), "outside");
            }
            other => panic!("expected blockquote + paragraph, got {other:?}"),
        }
        // The quote content must match the unquoted global list parse
        // heading-for-heading (blockquote-specific semantics are not
        // introduced).
        let unquoted_list = match &unquoted.nodes[0] {
            Block::UnorderedList { items, .. } => items,
            other => panic!("expected list, got {other:?}"),
        };
        let quoted_list = match &quoted.nodes[0] {
            Block::BlockQuote { content, .. } => match &content[0] {
                Block::UnorderedList { items, .. } => items,
                other => panic!("expected list, got {other:?}"),
            },
            other => panic!("expected blockquote, got {other:?}"),
        };
        assert_eq!(unquoted_list[0].content.len(), quoted_list[0].content.len());
    }

    #[test]
    fn blockquote_ordered_list_item_lazy_continuation_absorbed() {
        // "   bar" (three spaces) reaches the ordered content column of
        // "1. foo" (indent 0 + delimiter at 1 + 1 + 1 space = 3), so the
        // marker-less line is valid paragraph continuation text and is
        // absorbed, matching the global ordered-list parser.
        let doc = parse("> 1. foo\n   bar\n");
        assert_eq!(doc.nodes.len(), 1);
        match &doc.nodes[0] {
            Block::BlockQuote { content, .. } => match &content[0] {
                Block::OrderedList { items, start, .. } => {
                    assert_eq!(*start, 1);
                    assert_eq!(items.len(), 1);
                    match &items[0].content[0] {
                        Block::Paragraph { content, .. } => {
                            assert_eq!(joined_text(content), "foo\nbar");
                        }
                        other => panic!("expected item paragraph, got {other:?}"),
                    }
                }
                other => panic!("expected ordered list, got {other:?}"),
            },
            other => panic!("expected blockquote, got {other:?}"),
        }
    }

    /// Feed collected content lines through `QuoteContinuation` exactly as
    /// `parse_blockquote` does and decide whether a marker-less candidate
    /// at the trailing item's content column would be absorbed.
    fn continuation_would_absorb(lines: &[&str]) -> bool {
        let mut continuation = QuoteContinuation::default();
        for line in lines {
            continuation.record(line);
        }
        match &continuation.list_item {
            Some(item) => continuation.can_lazy_continue(item.content_col),
            None => continuation.can_lazy_continue(0),
        }
    }

    /// Deepest trailing block of a block tree (peeling block quotes and
    /// list items), as the parser oracle for continuation-state parity.
    fn trailing_leaf_block(node: &Block) -> &Block {
        match node {
            Block::BlockQuote { content, .. } => trailing_leaf_block(content.last().unwrap()),
            Block::UnorderedList { items, .. } | Block::OrderedList { items, .. } => {
                trailing_leaf_block(items.last().unwrap().content.last().unwrap())
            }
            other => other,
        }
    }

    #[test]
    fn blockquote_continuation_classifier_parity() {
        // For every fixture the incremental continuation state must
        // classify the trailing leaf exactly like the actual block parser:
        // a marker-less lazy candidate is accepted iff the parser's
        // trailing block after the same collected lines is an open
        // paragraph.
        let fixtures: &[&[&str]] = &[
            &["> foo"],
            &["> # h"],
            &[">  # h"],
            &[">   # h"],
            &["> foo", ">   # h"],
            &["> ```"],
            &["> ```", "> code"],
            &["> ```", "> code", "> ```"],
            &["> ---"],
            &["> - foo"],
            &["> - # h"],
            &["> - foo", ">   # h"],
            &["> - foo", ">   ```"],
            &["> - foo", ">   ```", ">   code"],
            &["> - foo", ">   ```", ">   code", ">   ```", ">   y"],
            &["> 1. foo"],
            &["> .note {x}"],
        ];
        for lines in fixtures {
            let source = lines.join("\n");
            let doc = parse(&source);
            let trailing_is_paragraph = matches!(
                trailing_leaf_block(doc.nodes.last().unwrap()),
                Block::Paragraph { .. }
            );
            assert_eq!(
                continuation_would_absorb(lines),
                trailing_is_paragraph,
                "continuation state diverges from the parser for {lines:?}",
            );
        }
    }
}
