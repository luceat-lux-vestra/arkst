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
    /// A block quotation containing recursively structured block content.
    Blockquote {
        content: Vec<IrNode>,
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
    /// A Markdown table with an explicit header and body rows.
    Table {
        header: IrTableRow,
        rows: Vec<IrTableRow>,
        span: SourceSpan,
    },
    /// A fenced code block with its optional full info string and first-token
    /// language tag.
    CodeBlock {
        language: Option<String>,
        info: Option<String>,
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
        named_args: Vec<IrNamedArg>,
        /// Source-backed explicit lambda parameters for contextual block
        /// calls such as `.let`. `None` represents a headerless implicit
        /// lambda when the callee selects that invocation semantics.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        lambda_parameters: Option<Vec<IrParameter>>,
        body: Option<Vec<IrNode>>,
        span: SourceSpan,
    },
    /// A structurally preserved `::` call chain.
    ///
    /// The evaluator consumes this representation directly and produces an
    /// ordinary evaluated IR node when every segment is executable. Keeping
    /// the structural form preserves source provenance and avoids synthetic
    /// source rewriting; the variant remains available for defensive handling
    /// of manually constructed unresolved IR.
    ChainedFunctionCall {
        head: IrCallSegment,
        chain: Vec<IrCallSegment>,
        body: Option<Vec<IrNode>>,
        span: SourceSpan,
    },
    /// A source-order user-defined function declaration. The evaluator
    /// registers it in the current scope and produces no document output.
    FunctionDeclaration {
        name: IrValue,
        parameters: Vec<IrParameter>,
        body: Vec<IrNode>,
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
    /// A Markdown strikethrough inline fragment.
    Strikethrough {
        content: Vec<IrInline>,
        span: SourceSpan,
    },
    /// An inline function call (`.name {arg}` inside a text flow).
    DirectiveCall {
        name: String,
        positional_args: Vec<IrValue>,
        named_args: Vec<IrNamedArg>,
        body: Option<Vec<IrInline>>,
        span: SourceSpan,
    },
    /// An inline structurally preserved `::` call chain.
    ChainedDirectiveCall {
        head: IrCallSegment,
        chain: Vec<IrCallSegment>,
        body: Option<Vec<IrInline>>,
        span: SourceSpan,
    },
    /// A Markdown inline link (`[label](destination)`).
    ///
    /// The label is kept as inline markup; the destination is preserved
    /// as-is (no normalization or resolution).
    Link {
        content: Vec<IrInline>,
        destination: String,
        title: Option<String>,
        span: SourceSpan,
    },
    /// A Markdown image. Resource resolution is intentionally outside the
    /// backend-neutral IR; the source metadata remains structurally present.
    Image {
        content: Vec<IrInline>,
        destination: String,
        title: Option<String>,
        span: SourceSpan,
    },
    /// An inline code span (`monospace`).
    ///
    /// The content is opaque literal text and is never evaluated or recursed
    /// into. The span covers the whole construct including the backtick
    /// delimiters.
    Code { content: String, span: SourceSpan },
    /// A source-backed soft line break.
    SoftBreak { span: SourceSpan },
    /// A source-backed hard line break.
    HardBreak { span: SourceSpan },
}

/// An evaluated list item in the IR.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct IrListItem {
    pub nodes: Vec<IrNode>,
    pub task: Option<IrTaskStatus>,
    pub span: SourceSpan,
}

/// One source-backed segment of a parser-preserved Quarkdown call chain.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct IrCallSegment {
    pub name: String,
    pub name_span: SourceSpan,
    pub positional_args: Vec<IrValue>,
    pub named_args: Vec<IrNamedArg>,
    pub span: SourceSpan,
}

/// One source-backed named call argument.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct IrNamedArg {
    pub name: String,
    pub name_span: SourceSpan,
    pub value: IrValue,
    pub span: SourceSpan,
}

/// One source-backed explicit parameter in a user-defined function.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct IrParameter {
    pub name: String,
    pub name_span: SourceSpan,
    pub span: SourceSpan,
    pub optional: bool,
}

/// A source-backed integer range value.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct IrRange {
    pub start: Option<u64>,
    pub end: Option<u64>,
    pub span: SourceSpan,
}

/// A source-backed pair of recursive semantic values.
///
/// Pairs are first-class evaluator values. The span covers the source
/// expression that produced the pair; nested values retain their own
/// provenance wherever their representation carries one.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct IrPair {
    pub first: Box<IrValue>,
    pub second: Box<IrValue>,
    pub span: SourceSpan,
}

/// A source-backed ordered dictionary.
///
/// The evaluator owns duplicate-key handling and preserves the order of the
/// first insertion of every surviving key. Entries are pairs so dictionary
/// iteration can reuse the ordinary iterable and scoped-call machinery.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct IrDictionary {
    pub entries: Vec<IrPair>,
    pub span: SourceSpan,
}

/// Semantic state for a GFM task-list item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum IrTaskStatus {
    Active,
    Completed,
}

/// A table row with source provenance for the complete row.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct IrTableRow {
    pub cells: Vec<IrTableCell>,
    pub span: SourceSpan,
}

/// A table cell with evaluated inline content and source provenance.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct IrTableCell {
    pub content: Vec<IrInline>,
    pub alignment: IrTableAlignment,
    pub span: SourceSpan,
}

/// Backend-neutral table alignment semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum IrTableAlignment {
    Left,
    Center,
    Right,
    None,
}

/// A resolved value used in function call arguments.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum IrValue {
    String(String),
    Number(f64),
    Boolean(bool),
    Identifier(String),
    /// A typed Quarkdown integer range. Open endpoints remain explicit until
    /// an iterable consumer chooses whether it can handle them.
    Range(IrRange),
    /// An ordered recursive collection of semantic values.
    Collection(Vec<IrValue>),
    /// A recursive pair value. Pairs are also valid iterable items.
    Pair(IrPair),
    /// An ordered recursive dictionary whose entries are key/value pairs.
    Dictionary(IrDictionary),
    Content(Vec<IrNode>),
    /// The Quarkdown language's explicit absence value.
    ///
    /// This is a semantic value, distinct from an evaluator `NoValue`
    /// outcome. It remains typed until an output boundary materializes it.
    None,
}

#[cfg(test)]
mod tests {
    use super::{IrDictionary, IrPair, IrRange, IrValue};
    use crate::source::{SourceId, SourceSpan};

    #[test]
    fn none_uses_the_stable_externally_tagged_serde_variant() {
        let encoded = serde_json::to_value(IrValue::None).expect("IrValue serializes");
        assert_eq!(encoded, serde_json::json!("None"));
        assert_eq!(
            serde_json::from_value::<IrValue>(encoded).expect("IrValue deserializes"),
            IrValue::None
        );
    }

    #[test]
    fn range_and_nested_collection_roundtrip_serde() {
        let value = IrValue::Collection(vec![
            IrValue::Range(IrRange {
                start: Some(2),
                end: Some(4),
                span: SourceSpan::new(SourceId(1), 3, 7),
            }),
            IrValue::Collection(vec![IrValue::Boolean(true), IrValue::None]),
        ]);
        let encoded = serde_json::to_value(&value).expect("IrValue serializes");
        assert_eq!(
            serde_json::from_value::<IrValue>(encoded).expect("IrValue deserializes"),
            value
        );
    }

    #[test]
    fn pair_and_dictionary_roundtrip_serde_preserves_recursive_values() {
        let span = SourceSpan::new(SourceId(1), 3, 8);
        let value = IrValue::Dictionary(IrDictionary {
            entries: vec![IrPair {
                first: Box::new(IrValue::String("a".to_string())),
                second: Box::new(IrValue::Pair(IrPair {
                    first: Box::new(IrValue::Number(1.0)),
                    second: Box::new(IrValue::Collection(vec![IrValue::Boolean(true)])),
                    span,
                })),
                span,
            }],
            span,
        });
        let encoded = serde_json::to_value(&value).expect("structured values serialize");
        assert_eq!(
            serde_json::from_value::<IrValue>(encoded).expect("structured values deserialize"),
            value
        );
    }
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
