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
        positional_args: Vec<Value>,
        named_args: Vec<(String, Value)>,
        body: Option<Vec<Block>>,
        span: ByteSpan,
    },
    Metadata {
        fields: Vec<(String, String)>,
        span: ByteSpan,
    },
    Raw {
        source: String,
        span: ByteSpan,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ListItem {
    pub content: Vec<Block>,
    pub span: ByteSpan,
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
        positional_args: Vec<Value>,
        named_args: Vec<(String, Value)>,
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
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    String(String),
    Number(f64),
    Boolean(bool),
    Identifier(String),
    Content(Vec<Inline>),
}
