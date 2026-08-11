//! Inline parsing over contiguous source ranges.

use crate::source::ByteSpan;
use crate::syntax::markdown::ast::Inline;
use crate::syntax::markdown::parser::{convert_quarkdown_arg, ParserDiagnostic, MAX_INLINE_DEPTH};

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
    let mut content = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\n' => content.push(' '),
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                content.push(' ');
            }
            ch => content.push(ch),
        }
    }
    let all_spaces = content.bytes().all(|b| b == b' ');
    if !all_spaces && content.starts_with(' ') && content.ends_with(' ') {
        content.remove(0);
        content.pop();
    }
    content
}

/// Parse inline nodes from the contiguous source slice `[start, end)`.
pub(crate) fn parse_inlines(
    source: &str,
    start: usize,
    end: usize,
    depth: usize,
    diagnostics: &mut Vec<ParserDiagnostic>,
) -> Vec<Inline> {
    InlineParser::new(source, start, end, depth, diagnostics).parse()
}
