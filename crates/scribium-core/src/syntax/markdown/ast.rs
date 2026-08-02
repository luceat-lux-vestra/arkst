use crate::source::ByteSpan;

/// A Scribium Markdown/AST document.
///
/// The root of every parsed Scribium source. Contains an ordered list of
/// block-level nodes and associates each node with the original source bytes.
#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    pub nodes: Vec<Block>,
    /// Number of lines in the source document.
    pub line_count: usize,
}

/// Block-level AST nodes.
#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    /// ATX heading (`#` to `######`).
    Heading {
        level: usize,
        content: Vec<Inline>,
        span: ByteSpan,
    },
    /// Paragraph — a sequence of inline nodes separated by blank lines.
    Paragraph {
        content: Vec<Inline>,
        span: ByteSpan,
    },
    /// Unordered list.
    UnorderedList {
        items: Vec<ListItem>,
        span: ByteSpan,
    },
    /// Fenced code block (triple backtick).
    CodeBlock {
        /// Optional language identifier after the opening backticks.
        language: Option<String>,
        /// Raw code content (verbatim text between fences).
        source: String,
        span: ByteSpan,
    },
    /// Thematic break (--- or ***)
    ThematicBreak { span: ByteSpan },
    /// Blank line / vertical space.
    /// Preserved for round-trip accuracy even though it carries no semantic meaning.
    BlankLine { span: ByteSpan },
}

/// An item in an unordered list.
#[derive(Debug, Clone, PartialEq)]
pub struct ListItem {
    pub content: Vec<Block>,
    pub span: ByteSpan,
}

/// Inline-level AST nodes.
#[derive(Debug, Clone, PartialEq)]
pub enum Inline {
    /// Plain text content.
    Text { content: String, span: ByteSpan },
    /// Emphasized text (`*text*` or `_text_`).
    Emphasis {
        content: Vec<Inline>,
        span: ByteSpan,
    },
    /// Strong text (`**text**` or `__text__`).
    Strong {
        content: Vec<Inline>,
        span: ByteSpan,
    },
    /// Hard line break (trailing two spaces + newline, or backslash at end of line).
    HardBreak { span: ByteSpan },
    /// Soft line break (adjacent lines without a blank line).
    SoftBreak { span: ByteSpan },
}
