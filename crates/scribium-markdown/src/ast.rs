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
        source: String,
        span: ByteSpan,
    },
    ThematicBreak {
        span: ByteSpan,
    },
    DirectiveCall {
        name: String,
        name_span: ByteSpan,
        positional_args: Vec<Value>,
        named_args: Vec<(String, Value)>,
        chain: Vec<CallSegment>,
        body: Option<Vec<Block>>,
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
        positional_args: Vec<Value>,
        named_args: Vec<(String, Value)>,
        chain: Vec<CallSegment>,
        body: Option<Vec<Inline>>,
        span: ByteSpan,
    },
    Link {
        content: Vec<Inline>,
        destination: String,
        span: ByteSpan,
    },
    Image {
        content: Vec<Inline>,
        destination: String,
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
    Content(Vec<Inline>),
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
    pub positional_args: Vec<Value>,
    pub named_args: Vec<(String, Value)>,
    pub span: ByteSpan,
}
