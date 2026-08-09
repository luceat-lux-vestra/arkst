//! IR (Intermediate Representation) — the bridge between the evaluator and Typst lowering.
//!
//! The IR is a flat sequence of evaluated nodes. Each node represents an already-resolved
//! content fragment ready for code generation. Source spans are preserved throughout.

use crate::source::SourceSpan;

/// A compiled document in intermediate representation.
///
/// Produced by the evaluator, consumed by Typst lowering. The IR is serializable
/// for `scribium inspect --emit ir` output.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct IrDocument {
    /// Ordered list of IR nodes.
    pub nodes: Vec<IrNode>,
    /// Metadata extracted from front matter or document-level directives.
    pub metadata: IrMetadata,
}

/// Document-level metadata extracted during evaluation.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct IrMetadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub date: Option<String>,
    pub raw: Vec<(String, String)>,
}

/// A block-level IR node — ready for Typst code generation.
///
/// Unlike AST nodes, IR nodes carry no unresolved references, no directive syntax,
/// and no parser-specific structure.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum IrNode {
    /// A heading with evaluated inline content.
    Heading {
        level: usize,
        content: Vec<IrInline>,
        span: SourceSpan,
    },
    /// A paragraph containing zero or more evaluated inline fragments.
    Paragraph {
        content: Vec<IrInline>,
        span: SourceSpan,
    },
    /// Unordered list with one or more items.
    UnorderedList {
        items: Vec<IrListItem>,
        span: SourceSpan,
    },
    /// Ordered list with one or more items.
    OrderedList {
        items: Vec<IrListItem>,
        /// The starting ordinal of the list (typically 1).
        start: usize,
        span: SourceSpan,
    },
    /// A fenced code block with an optional language tag.
    CodeBlock {
        language: Option<String>,
        source: String,
        span: SourceSpan,
    },
    /// Raw Typst source — inserted verbatim into the output.
    RawTypst { source: String, span: SourceSpan },
    /// A function/component call that was resolved during evaluation.
    /// The lowering pass renders this as the corresponding Typst construct.
    FunctionCall {
        name: String,
        positional_args: Vec<IrValue>,
        named_args: Vec<(String, IrValue)>,
        body: Option<Vec<IrNode>>,
        span: SourceSpan,
    },
    /// A thematic break (horizontal rule).
    ThematicBreak { span: SourceSpan },
    /// Math expression (inline or display).
    Math {
        source: String,
        display: bool,
        span: SourceSpan,
    },
}

/// An inline fragment within a block-level IR node.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum IrInline {
    /// Plain text content.
    Text { content: String, span: SourceSpan },
    /// Emphasized (italic) inline fragment.
    Emphasis {
        content: Vec<IrInline>,
        span: SourceSpan,
    },
    /// Strong (bold) inline fragment.
    Strong {
        content: Vec<IrInline>,
        span: SourceSpan,
    },
    /// An inline function call (`.name {arg}` inside a text flow).
    DirectiveCall {
        name: String,
        positional_args: Vec<IrValue>,
        named_args: Vec<(String, IrValue)>,
        body: Option<Vec<IrInline>>,
        span: SourceSpan,
    },
}

/// An evaluated list item in the IR.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct IrListItem {
    pub nodes: Vec<IrNode>,
    pub span: SourceSpan,
}

/// A resolved value used in function call arguments.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum IrValue {
    String(String),
    Number(f64),
    Boolean(bool),
    Identifier(String),
    Content(Vec<IrNode>),
}

/// An entry in the source map linking a range of generated output
/// back to its originating source span.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SourceMapEntry {
    /// Range in the generated Typst source (byte offsets).
    pub generated_start: usize,
    pub generated_end: usize,
    /// The original source span this generated range belongs to.
    pub original: SourceSpan,
}
