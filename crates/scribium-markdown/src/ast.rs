use scribium_source::ByteSpan;

#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    pub nodes: Vec<Block>,
    pub front_matter: Option<FrontMatter>,
    pub line_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FrontMatter {
    pub fields: Vec<(String, String)>,
    pub span: ByteSpan,
}

/// The source-backed body representation used by engine conversion.
///
/// `source_text` is the exact source slice for the body. `text` is the same
/// `trimIndent().trimEnd()` body string that Quarkdown's body refiner supplies
/// to a `DynamicValue`. `native_text` retains the existing source-backed
/// native-content contract for `.html`; it is not a target-conversion value.
/// `span` identifies the original source range. Parsed body nodes are
/// deliberately kept separately so a target can choose lazy structured
/// evaluation without reconstructing this text from an evaluated value.
#[derive(Debug, Clone, PartialEq)]
pub struct RawBody {
    pub source_text: String,
    pub text: String,
    pub native_text: String,
    pub span: ByteSpan,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Heading {
        level: usize,
        content: Vec<Inline>,
        span: ByteSpan,
    },
    Paragraph {
        content: Vec<Inline>,
        span: ByteSpan,
    },
    Blockquote {
        content: Vec<Block>,
        span: ByteSpan,
    },
    UnorderedList {
        items: Vec<ListItem>,
        span: ByteSpan,
    },
    OrderedList {
        items: Vec<ListItem>,
        start: usize,
        span: ByteSpan,
    },
    Table {
        header: TableRow,
        rows: Vec<TableRow>,
        span: ByteSpan,
    },
    CodeBlock {
        language: Option<String>,
        /// The complete fenced-code info string, when present. `language` is
        /// its first whitespace-delimited token for backend compatibility.
        info: Option<String>,
        source: String,
        span: ByteSpan,
    },
    ThematicBreak {
        span: ByteSpan,
    },
    DirectiveCall {
        name: String,
        name_span: ByteSpan,
        head_span: ByteSpan,
        /// Arguments in their original source order.
        arguments: Vec<CallArgument>,
        chain: Vec<CallSegment>,
        body: Option<Vec<Block>>,
        raw_body: Option<RawBody>,
        /// Contextual lambda metadata for calls with lambda body semantics
        /// (`.function`, `.let`, `.foreach`, and `.repeat`). Other call bodies deliberately
        /// remain ordinary Markdown structures.
        lambda_header: Option<LambdaHeader>,
        span: ByteSpan,
    },
    Metadata {
        fields: Vec<(String, String)>,
        span: ByteSpan,
    },
    RawHtml {
        source: String,
        span: ByteSpan,
    },
    Unsupported {
        kind: String,
        span: ByteSpan,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ListItem {
    pub content: Vec<Block>,
    pub span: ByteSpan,
    pub task: Option<TaskStatus>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Active,
    Completed,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TableRow {
    pub cells: Vec<TableCell>,
    pub span: ByteSpan,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TableCell {
    pub content: Vec<Inline>,
    pub alignment: TableAlignment,
    pub span: ByteSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableAlignment {
    Left,
    Center,
    Right,
    None,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Inline {
    Text {
        content: String,
        span: ByteSpan,
    },
    Emphasis {
        content: Vec<Inline>,
        span: ByteSpan,
    },
    Strong {
        content: Vec<Inline>,
        span: ByteSpan,
    },
    DirectiveCall {
        name: String,
        name_span: ByteSpan,
        head_span: ByteSpan,
        /// Arguments in their original source order.
        arguments: Vec<CallArgument>,
        chain: Vec<CallSegment>,
        body: Option<Vec<Inline>>,
        span: ByteSpan,
    },
    Link {
        content: Vec<Inline>,
        destination: String,
        title: Option<String>,
        span: ByteSpan,
    },
    Image {
        content: Vec<Inline>,
        destination: String,
        title: Option<String>,
        span: ByteSpan,
    },
    Code {
        content: String,
        span: ByteSpan,
    },
    RawHtml {
        content: String,
        span: ByteSpan,
    },
    Strikethrough {
        content: Vec<Inline>,
        span: ByteSpan,
    },
    HardBreak {
        span: ByteSpan,
    },
    SoftBreak {
        span: ByteSpan,
    },
    Unsupported {
        kind: String,
        span: ByteSpan,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    String(String),
    Number(f64),
    Boolean(bool),
    Identifier(String),
    Range(RangeValue),
    Content(Vec<Inline>),
    /// A parser-preserved inline argument that may become a callable only
    /// after the callee's parameter contract has been resolved. The complete
    /// content remains available for ordinary positional parameters.
    InlineBody {
        content: Vec<Inline>,
        parameters: Option<LambdaHeader>,
        body: Vec<Inline>,
        span: ByteSpan,
    },
    /// A source-backed first-class inline lambda. Its body has already passed
    /// through the Rushdown-backed inline frontend.
    Lambda {
        parameters: Option<LambdaHeader>,
        body: Vec<Inline>,
        span: ByteSpan,
    },
}

/// A source-backed integer range literal from a Quarkdown value argument.
#[derive(Debug, Clone, PartialEq)]
pub struct RangeValue {
    pub start: Option<u64>,
    pub end: Option<u64>,
    pub span: ByteSpan,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NamedArg {
    pub name: String,
    pub name_span: ByteSpan,
    pub value: Value,
    /// The exact source span of the braced value, including its delimiters.
    pub value_span: ByteSpan,
    pub span: ByteSpan,
}

/// One source-backed call argument, retained in source order.
#[derive(Debug, Clone, PartialEq)]
pub enum CallArgument {
    Positional { value: Value, span: ByteSpan },
    Named(NamedArg),
}

#[derive(Debug, Clone, PartialEq)]
pub struct LambdaHeader {
    pub parameters: Vec<LambdaParameter>,
    pub span: ByteSpan,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LambdaParameter {
    pub name: String,
    pub name_span: ByteSpan,
    pub span: ByteSpan,
    pub optional: bool,
}

/// A parser-preserved `::` call-chain segment.
///
/// This frontend representation records syntax and source provenance only.
/// Evaluation of the chained result remains outside the grammar adaptation
/// slice.
#[derive(Debug, Clone, PartialEq)]
pub struct CallSegment {
    pub name: String,
    pub name_span: ByteSpan,
    /// Arguments in their original source order.
    pub arguments: Vec<CallArgument>,
    pub span: ByteSpan,
}
