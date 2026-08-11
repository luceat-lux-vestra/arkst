//! Physical source lines and container-relative line views.

/// A physical source line with byte offsets into the original source.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SourceLine<'a> {
    /// Full line text without its terminator.
    pub(crate) raw: &'a str,
    /// Leading-whitespace-stripped line text.
    pub(crate) text: &'a str,
    /// Absolute byte offset of `raw` in the source.
    pub(crate) raw_start: usize,
    /// Absolute byte offset of `text` in the source.
    pub(crate) text_start: usize,
    /// Absolute byte offset of the line terminator (or EOF).
    pub(crate) term: usize,
    /// Absolute byte offset just past the line terminator (or EOF).
    pub(crate) end: usize,
}

impl<'a> SourceLine<'a> {
    pub(super) fn content_end(self) -> usize {
        self.raw_start + self.raw.len()
    }

    pub(super) fn indent(self) -> usize {
        self.text_start - self.raw_start
    }

    pub(super) fn is_blank(self) -> bool {
        self.text.is_empty()
    }

    /// View this line after an already-open container has consumed `prefix`
    /// leading columns. The prefix is a source-shape operation only; the
    /// parser still retains the original line and all absolute offsets.
    pub(super) fn view(self, prefix: usize) -> LineView<'a> {
        let consumed = prefix.min(self.raw.len());
        let raw = &self.raw[consumed..];
        let whitespace = raw
            .bytes()
            .take_while(|byte| *byte == b' ' || *byte == b'\t')
            .count();
        LineView {
            raw,
            text: &raw[whitespace..],
            raw_start: self.raw_start + consumed,
            text_start: self.raw_start + consumed + whitespace,
            raw_end: self.content_end(),
            term: self.term,
            end: self.end,
            prefix,
            indent: whitespace,
        }
    }

    /// View the first content line of a list item. Unlike `view`, the
    /// consumed prefix includes the list marker itself, not only indentation.
    pub(super) fn item_content_view(self, prefix: usize, content_start: usize) -> LineView<'a> {
        let relative = content_start
            .saturating_sub(self.raw_start)
            .min(self.raw.len());
        let raw = &self.raw[relative..];
        LineView {
            raw,
            text: raw,
            raw_start: content_start.min(self.content_end()),
            text_start: content_start.min(self.content_end()),
            raw_end: self.content_end(),
            term: self.term,
            end: self.end,
            prefix,
            indent: 0,
        }
    }
}

/// A view of one physical line after the currently active container prefix.
#[derive(Debug, Clone, Copy)]
pub(super) struct LineView<'a> {
    pub(super) raw: &'a str,
    pub(super) text: &'a str,
    pub(super) raw_start: usize,
    pub(super) text_start: usize,
    pub(super) raw_end: usize,
    pub(super) term: usize,
    pub(super) end: usize,
    /// Number of source columns consumed by open containers.
    pub(super) prefix: usize,
    /// Leading whitespace remaining after `prefix` was consumed.
    pub(super) indent: usize,
}

impl LineView<'_> {
    pub(super) fn is_blank(self) -> bool {
        self.text.is_empty()
    }

    pub(super) fn content_end(self) -> usize {
        self.raw_end
    }
}

/// Split a source string into physical lines while retaining absolute byte
/// offsets. CRLF is normalized only at this physical-line boundary; the
/// source offsets continue to refer to the original bytes.
pub(crate) fn split_lines(source: &str) -> Vec<SourceLine<'_>> {
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
            let whitespace = raw
                .bytes()
                .take_while(|byte| *byte == b' ' || *byte == b'\t')
                .count();
            lines.push(SourceLine {
                raw,
                text: &raw[whitespace..],
                raw_start: line_start,
                text_start: line_start + whitespace,
                term: pos,
                end: if pos == len { pos } else { pos + 1 },
            });
            line_start = pos + 1;
        }
        pos += 1;
    }

    lines
}
