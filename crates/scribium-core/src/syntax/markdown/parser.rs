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

use super::ast::{Block, Document, FrontMatter, Inline, ListItem};
use crate::source::ByteSpan;

/// Maximum block-nesting depth before a parse is flattened to paragraphs.
///
/// Guards against stack overflow on pathological input such as thousands of
/// nested list markers.
const MAX_BLOCK_DEPTH: usize = 64;

/// Maximum inline-nesting depth before delimiters are treated as literal text.
const MAX_INLINE_DEPTH: usize = 64;
/// Result of parsing a front matter candidate block.
#[allow(dead_code)]
enum FrontMatterParse {
    /// No front matter candidate found.
    Absent,
    /// Valid front matter with fields.
    Valid {
        fields: Vec<(String, String)>,
        span: ByteSpan,
        consumed_lines: usize,
    },
    /// Malformed front matter block - should be treated as regular Markdown.
    Invalid,
}

/// Parse YAML front matter at document start.
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

/// Parse a Markdown source string into a `Document`.
///
/// Never panics on malformed input; unclosed constructs are parsed
/// deterministically up to the end of the source.
pub fn parse(source: &str) -> Document {
    let lines = split_lines(source);

    // Parse front matter if present at document start
    let (front_matter, front_matter_lines) = parse_front_matter(source, &lines);
    let mut cursor = front_matter_lines;

    let nodes = parse_blocks(source, &lines, &mut cursor, 0);
    let line_count = source.as_bytes().iter().filter(|&&b| b == b'\n').count() + 1;
    Document {
        nodes,
        front_matter,
        line_count,
    }
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
) -> Vec<Block> {
    let mut blocks = Vec::new();
    while *cursor < lines.len() {
        let line = &lines[*cursor];
        if line.is_blank() {
            *cursor += 1;
            continue;
        }
        if let Some(level) = is_heading_text(line.text) {
            blocks.push(parse_heading(source, line, level));
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
                blocks.push(parse_list(source, lines, cursor, marker, depth));
                continue;
            }
        }
        if is_directive_line(line.text) {
            blocks.push(parse_directive_block(source, lines, cursor));
            continue;
        }
        blocks.push(parse_paragraph(source, lines, cursor));
    }
    blocks
}

/// Parse a block-level `@` directive line.
///
/// Consumes only the directive line itself. Multi-line bodies are an M1+ extension.
fn parse_directive_block(source: &str, lines: &[SourceLine<'_>], cursor: &mut usize) -> Block {
    let line = &lines[*cursor];
    *cursor += 1;
    let line_source = &source[line.text_start..line.content_end()];

    if let Some((directive, _consumed)) = crate::syntax::quarkdown::parser::parse_directive_at(
        line_source,
        0,
        crate::source::SourceId(0),
    ) {
        match directive {
            crate::syntax::quarkdown::Directive::Call {
                name,
                positional_args,
                named_args,
                body,
                ..
            } => {
                let body_block = body.and_then(|b| match *b {
                    crate::syntax::quarkdown::Directive::Value(
                        crate::syntax::markdown::ast::Value::String(s),
                    ) => {
                        // Parse body content as inline nodes
                        if s.is_empty() {
                            None
                        } else {
                            Some(Box::new(Block::Paragraph {
                                content: parse_inlines(&s, 0, s.len(), 0),
                                span: ByteSpan::new(line.text_start, line.end),
                            }))
                        }
                    }
                    crate::syntax::quarkdown::Directive::Call { .. } => {
                        // Nested directive: wrap in a DirectiveCall block
                        Some(Box::new(Block::DirectiveCall {
                            name: String::new(), // placeholder
                            positional_args: vec![],
                            named_args: vec![],
                            body: None,
                            span: ByteSpan::new(line.text_start, line.end),
                        }))
                    }
                    _ => None,
                });

                return Block::DirectiveCall {
                    name,
                    positional_args,
                    named_args,
                    body: body_block,
                    span: ByteSpan::new(line.text_start, line.end),
                };
            }
            crate::syntax::quarkdown::Directive::Variable { name, .. } => {
                // A bare @name at block level becomes a directive call with no args
                return Block::DirectiveCall {
                    name,
                    positional_args: vec![],
                    named_args: vec![],
                    body: None,
                    span: ByteSpan::new(line.text_start, line.end),
                };
            }
            _ => {}
        }
    }

    // Fallback: parse as paragraph
    parse_paragraph(source, lines, cursor)
}
fn parse_paragraph(source: &str, lines: &[SourceLine<'_>], cursor: &mut usize) -> Block {
    let first = &lines[*cursor];
    loop {
        *cursor += 1;
        if *cursor >= lines.len() || lines[*cursor].is_blank() || lines[*cursor].starts_block() {
            break;
        }
    }
    let last = &lines[*cursor - 1];
    let content = parse_inlines(source, first.text_start, last.content_end(), 0);
    Block::Paragraph {
        content,
        span: ByteSpan::new(first.text_start, last.end),
    }
}

/// Parse an ATX heading line.
fn parse_heading(source: &str, line: &SourceLine<'_>, level: usize) -> Block {
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
        parse_inlines(source, content_start, content_end, 0)
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
        let content = parse_blocks(source, &item_lines, &mut inner_cursor, depth + 1);
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

/// Whether the text starts a `@`-prefixed directive (block-level).
fn is_directive_line(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.first() != Some(&b'@') {
        return false;
    }
    // Must have a name character after @
    bytes.len() > 1 && (bytes[1].is_ascii_alphabetic() || bytes[1] == b'_')
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
}

impl<'a> InlineParser<'a> {
    fn new(source: &'a str, start: usize, end: usize, depth: usize) -> Self {
        Self {
            source,
            pos: start,
            end,
            depth,
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
                b'@' => self.parse_directive_inline(&mut inlines),
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

    /// Parse an inline `@` directive (e.g. `@strong[text]`, `@fn()`).
    fn parse_directive_inline(&mut self, inlines: &mut Vec<Inline>) {
        let start = self.pos;
        let remaining = &self.source[self.pos..self.end];
        if let Some((directive, consumed)) = crate::syntax::quarkdown::parser::parse_directive_at(
            remaining,
            0,
            crate::source::SourceId(0),
        ) {
            let end_pos = self.pos + consumed;
            match directive {
                crate::syntax::quarkdown::Directive::Call {
                    name,
                    positional_args,
                    named_args,
                    body,
                    ..
                } => {
                    let body_inlines = body.and_then(|b| match *b {
                        crate::syntax::quarkdown::Directive::Value(
                            crate::syntax::markdown::ast::Value::String(s),
                        ) => {
                            if s.is_empty() {
                                None
                            } else {
                                Some(parse_inlines(&s, 0, s.len(), self.depth + 1))
                            }
                        }
                        _ => None,
                    });

                    inlines.push(Inline::DirectiveCall {
                        name,
                        positional_args,
                        named_args,
                        body: body_inlines,
                        span: ByteSpan::new(start, end_pos),
                    });
                    self.pos = end_pos;
                }
                crate::syntax::quarkdown::Directive::Variable { name, .. } => {
                    inlines.push(Inline::DirectiveCall {
                        name,
                        positional_args: vec![],
                        named_args: vec![],
                        body: None,
                        span: ByteSpan::new(start, end_pos),
                    });
                    self.pos = end_pos;
                }
                _ => {
                    inlines.push(Inline::Text {
                        content: self.source[start..end_pos].to_string(),
                        span: ByteSpan::new(start, end_pos),
                    });
                    self.pos = end_pos;
                }
            }
        } else {
            inlines.push(Inline::Text {
                content: "@".to_string(),
                span: ByteSpan::new(start, start + 1),
            });
            self.pos += 1;
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
        let content = InlineParser::new(self.source, open_end, close_start, self.depth + 1).parse();
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
        let content = InlineParser::new(self.source, open_end, close_start, self.depth + 1).parse();
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
            if b == b'*' || b == b'_' || b == b'\n' || b == b'@' {
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

/// Parse inline nodes from the contiguous source slice `[start, end)`.
fn parse_inlines(source: &str, start: usize, end: usize, depth: usize) -> Vec<Inline> {
    InlineParser::new(source, start, end, depth).parse()
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
}
