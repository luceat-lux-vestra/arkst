use crate::source::ByteSpan;

/// A Scribium Markdown/AST document.
///
/// The root of every parsed Scribium source. Contains an ordered list of
/// block-level nodes and associates each node with the original source bytes.
#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    pub nodes: Vec<Block>,
    /// Parsed front matter, if present (typically `---`-delimited).
    /// Flat `key: value` entries only; not full YAML.
    pub front_matter: Option<FrontMatter>,
    /// Number of lines in the source document.
    pub line_count: usize,
}

/// Flat key-value front matter block at the start of a document.
///
/// Supports only `key: value` lines (split on the first colon). Nested
/// objects, arrays, and block strings are not supported. Duplicate keys
/// use last-wins semantics.
#[derive(Debug, Clone, PartialEq)]
pub struct FrontMatter {
    pub fields: Vec<(String, String)>,
    pub span: ByteSpan,
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
    /// A Quarkdown-compatible function call (`.name {arg} key:{value}`).
    DirectiveCall {
        name: String,
        positional_args: Vec<Value>,
        named_args: Vec<(String, Value)>,
        body: Option<Vec<Block>>,
        span: ByteSpan,
    },
    /// Metadata block (flat key-value front matter embedded inline).
    Metadata {
        fields: Vec<(String, String)>,
        span: ByteSpan,
    },
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
    /// An inline Quarkdown function call (`.name {arg}` inside a text flow).
    DirectiveCall {
        name: String,
        positional_args: Vec<Value>,
        named_args: Vec<(String, Value)>,
        body: Option<Vec<Inline>>,
        span: ByteSpan,
    },
    /// Hard line break (trailing two spaces + newline, or backslash at end of line).
    HardBreak { span: ByteSpan },
    /// Soft line break (adjacent lines without a blank line).
    SoftBreak { span: ByteSpan },
}

/// A literal value in a directive or expression context.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    String(String),
    Number(f64),
    Boolean(bool),
    Identifier(String),
    /// A content fragment: inline markup and/or nested function calls, as
    /// found inside an argument (`{.inner {value}}`) or a body.
    Content(Vec<Inline>),
}
