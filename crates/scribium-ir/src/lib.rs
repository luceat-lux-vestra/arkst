//! IR (Intermediate Representation) — Scribium's one backend-neutral document model.
//!
//! The same `IrDocument` model is used before and after semantic evaluation.
//! Frontend lowering creates its initial structure, and semantic passes
//! progressively normalize and resolve it while preserving source spans. An IR
//! document may therefore contain completed semantic nodes alongside
//! structurally preserved unresolved invocations. Backend lowering normally
//! receives normalized IR; its defensive handling of manually constructed or
//! unresolved IR is a separate concern from semantic evaluation.

use scribium_source::SourceSpan;
use std::num::NonZeroU32;

/// A compiled document in intermediate representation.
///
/// Produced initially by frontend-to-IR lowering, progressively normalized by
/// semantic evaluation, and consumed by backend lowering. The IR is
/// serializable for `scribium inspect --emit ir` output.
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
    /// Final evaluator-owned document state, represented as immutable
    /// backend-neutral data.
    #[serde(default)]
    pub document_state: IrDocumentState,
}

/// Immutable document state snapshot produced after evaluation.
///
/// This is deliberately plain serializable data. Evaluator runtime carriers,
/// such as shared mutable handles, never cross into the IR boundary.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct IrDocumentState {
    /// The document's current `.docname`, or an empty string when unset.
    pub name: String,
    /// The document's current `.docdescription`, or an empty string when unset.
    pub description: String,
    /// The document's current `.doctype`, defaulting to `plain`.
    #[serde(default)]
    pub document_type: IrDocumentType,
    /// Authors in document-state insertion order.
    #[serde(default)]
    pub authors: Vec<IrDocumentAuthor>,
    /// Keywords in document-state insertion order.
    #[serde(default)]
    pub keywords: Vec<String>,
    /// The document theme selected by `.theme`, if the setter has been called.
    ///
    /// `Some` with two absent components is distinct from `None`: the former
    /// records an explicit empty theme setter, while the latter means that no
    /// `.theme` call has committed state.
    #[serde(default)]
    pub theme: Option<IrDocumentTheme>,
    /// The document locale selected by `.doclang`, if the setter has been
    /// called. This is plain snapshot data; locale resolution is evaluator
    /// owned and no runtime resolver crosses the IR boundary.
    #[serde(default)]
    pub locale: Option<IrDocumentLocale>,
    /// The backend-neutral caption-position state selected by
    /// `.captionposition`. The optional overrides distinguish inherited
    /// positions from explicit values.
    #[serde(default)]
    pub caption_position: IrCaptionPositionInfo,
}

/// Backend-neutral locale data retained by the bounded `.doclang` slice.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IrDocumentLocale {
    /// Canonical BCP 47-style tag selected by the evaluator.
    pub tag: String,
    /// Localized name returned by the `.doclang` getter.
    pub localized_name: String,
}

/// Backend-neutral document theme components.
///
/// This state intentionally carries no theme registry, renderer object, or
/// filesystem identity. Theme existence and rendering remain downstream
/// responsibilities.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IrDocumentTheme {
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub layout: Option<String>,
}

/// A backend-neutral document author with bounded, ordered string metadata.
///
/// The ordered pairs preserve the observable dictionary iteration order from
/// Quarkdown without introducing a backend-specific author representation.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IrDocumentAuthor {
    pub name: String,
    /// Additional string information such as `email` or `website`.
    #[serde(default)]
    pub info: Vec<(String, String)>,
}

/// The closed caption-position enum exposed by Quarkdown's
/// `.captionposition` builtin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum IrCaptionPosition {
    Top,
    #[default]
    Bottom,
}

/// Backend-neutral caption-position state retained by the bounded M3 slice.
///
/// `None` on an element-specific field means that the element inherits the
/// current `default`; it is not equivalent to storing `Bottom` explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct IrCaptionPositionInfo {
    #[serde(default)]
    pub default: IrCaptionPosition,
    #[serde(default)]
    pub figures: Option<IrCaptionPosition>,
    #[serde(default)]
    pub tables: Option<IrCaptionPosition>,
    #[serde(default)]
    pub code_blocks: Option<IrCaptionPosition>,
}

/// The closed document-type enum exposed by Quarkdown's `.doctype` builtin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum IrDocumentType {
    #[default]
    Plain,
    Paged,
    Slides,
    Docs,
}

impl IrDocumentType {
    pub fn quarkdown_name(self) -> &'static str {
        match self {
            Self::Plain => "plain",
            Self::Paged => "paged",
            Self::Slides => "slides",
            Self::Docs => "docs",
        }
    }
}

/// A backend-neutral numeric size with a closed public unit set.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct IrSize {
    pub value: f64,
    pub unit: IrSizeUnit,
}

/// Units accepted by Quarkdown v2.5.1 size conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum IrSizeUnit {
    Px,
    Pt,
    Cm,
    Mm,
    In,
    Em,
    Percent,
}

/// A backend-neutral RGBA color.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct IrColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    /// Alpha is the upstream 0.0..=1.0 fraction, not a backend byte/string.
    pub alpha: f64,
}

/// A closed, backend-neutral semantic component family.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum IrComponent {
    Stacked(IrStackedComponent),
    Container(IrContainerComponent),
    Landscape(IrLandscapeComponent),
}

impl IrComponent {
    /// Returns the source span of the component-producing call.
    pub fn span(&self) -> SourceSpan {
        match self {
            Self::Stacked(component) => component.span,
            Self::Container(component) => component.span,
            Self::Landscape(component) => component.span,
        }
    }
}

/// The backend-neutral semantic state produced by `.landscape`.
///
/// The 90-degree counter-clockwise transformation is a consumer semantic;
/// angle, page, and backend-specific rendering details stay out of the IR.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct IrLandscapeComponent {
    pub children: Vec<IrNode>,
    pub span: SourceSpan,
}

/// The backend-neutral container state consumed by `.container`, `.center`,
/// and `.align`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct IrContainerComponent {
    /// An explicit width constraint, if one was supplied.
    #[serde(default)]
    pub width: Option<IrSize>,
    /// An explicit height constraint, if one was supplied.
    #[serde(default)]
    pub height: Option<IrSize>,
    pub full_width: bool,
    /// `None` represents an unaligned ordinary container.
    #[serde(default)]
    pub alignment: Option<IrContainerAlignment>,
    pub children: Vec<IrNode>,
    pub span: SourceSpan,
}

/// Logical container alignment used by the bounded `.center` and `.align`
/// consumers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum IrContainerAlignment {
    Start,
    Center,
    End,
}

/// The semantic state shared by row, column, and grid components.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct IrStackedComponent {
    pub layout: IrStackedLayout,
    pub main_axis_alignment: IrMainAxisAlignment,
    pub cross_axis_alignment: IrCrossAxisAlignment,
    pub row_gap: Option<IrSize>,
    pub column_gap: Option<IrSize>,
    pub children: Vec<IrNode>,
    pub span: SourceSpan,
}

/// The closed layout family of a stacked component.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum IrStackedLayout {
    Row,
    Column,
    Grid { columns: NonZeroU32 },
}

/// Main-axis alignment semantics for a stacked component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum IrMainAxisAlignment {
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

/// Cross-axis alignment semantics for a stacked component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum IrCrossAxisAlignment {
    Start,
    Center,
    End,
    Stretch,
}

/// A closed, domain-preserving enum value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum IrEnumValue {
    DocumentType(IrDocumentType),
    CaptionPosition(IrCaptionPosition),
    StackedMainAxisAlignment(IrMainAxisAlignment),
    StackedCrossAxisAlignment(IrCrossAxisAlignment),
    ContainerAlignment(IrContainerAlignment),
}

/// A closed target discriminator for native content that remains opaque until
/// backend selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum NativeTarget {
    Html,
    /// Raw Markdown content, retained for a future Markdown output target.
    Markdown,
}

/// Evaluated target-specific content with source provenance.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TargetSpecificContent {
    pub target: NativeTarget,
    pub content: String,
    pub span: SourceSpan,
}

/// A backend-neutral block-level IR node.
///
/// Depending on the pipeline stage, a node may contain evaluated semantic
/// content or a structurally preserved unresolved invocation. IR does not
/// carry frontend directive syntax or parser-specific AST structure, but
/// `FunctionCall` and `ChainedFunctionCall` remain explicit compatibility forms
/// for unresolved structural calls.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum IrNode {
    /// A heading with structured inline content, progressively normalized by
    /// semantic evaluation.
    Heading {
        level: usize,
        content: Vec<IrInline>,
        span: SourceSpan,
    },
    /// A paragraph containing zero or more structured inline fragments.
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
    /// Parser-owned raw HTML retained only while a function body can claim it
    /// as an opaque String argument. Ordinary document raw HTML is rejected
    /// before it reaches normalized IR.
    RawHtml { source: String, span: SourceSpan },
    /// Target-specific content retained until backend selection.
    TargetSpecificContent { content: TargetSpecificContent },
    /// A completed, typed backend-neutral semantic component.
    Component { component: IrComponent },
    /// A structurally preserved function/component call.
    ///
    /// Semantic evaluation normally consumes this form. It may remain in IR as
    /// an unresolved compatibility form, including when IR is inspected or
    /// manually constructed for defensive backend handling.
    FunctionCall {
        name: String,
        positional_args: Vec<IrValue>,
        named_args: Vec<IrNamedArg>,
        /// The source-ordered argument candidates retained until semantic
        /// binding. `None` is accepted for legacy/manually constructed IR;
        /// frontend-produced calls always populate this field.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ordered_args: Option<Vec<IrCallArgument>>,
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
    /// ordinary normalized IR node when every segment is executable. Keeping
    /// the structural form preserves source provenance and avoids synthetic
    /// source rewriting; the variant remains available for unresolved
    /// structural compatibility forms and defensive backend handling.
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
    /// Quarkdown `.whitespace` with an optional fixed inline extent.
    ///
    /// `None` for both dimensions is the non-breaking whitespace form. When
    /// either dimension is present, both dimensions are normalized by the
    /// evaluator and a missing axis is represented by a zero `IrSize`.
    Whitespace {
        width: Option<IrSize>,
        height: Option<IrSize>,
        span: SourceSpan,
    },
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
        /// Source-ordered candidates retained for the engine invocation
        /// binder. See [`IrNode::FunctionCall::ordered_args`].
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ordered_args: Option<Vec<IrCallArgument>>,
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
    /// A Markdown image.
    ///
    /// `destination` remains a logical resource reference. It is never
    /// rewritten to a host path in the backend-neutral IR; a native backend
    /// resolves local references against its explicit source context.
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
    /// Parser-owned raw HTML retained only inside an opaque function body.
    RawHtml { content: String, span: SourceSpan },
    /// Target-specific content retained until backend selection.
    TargetSpecificContent { content: TargetSpecificContent },
}

/// A list item in the backend-neutral IR.
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
    /// Source-ordered candidates retained until semantic binding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ordered_args: Option<Vec<IrCallArgument>>,
    pub span: SourceSpan,
}

/// One source-backed invocation candidate in its original order.
///
/// This is a lightweight structural reference into the sibling positional or
/// named projection on the containing call. It deliberately does not own a
/// second `IrValue` tree: transient provenance such as `ValueOrigin` remains
/// evaluator-owned, and argument values remain owned by the projections.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum IrCallArgument {
    Positional {
        /// Index into the containing call's `positional_args` vector.
        index: usize,
        span: SourceSpan,
    },
    Named {
        /// Index into the containing call's `named_args` vector.
        index: usize,
        name_span: SourceSpan,
        span: SourceSpan,
    },
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
    /// Signed Kotlin-`Int`-compatible endpoint. `None` preserves an open end.
    pub start: Option<i32>,
    pub end: Option<i32>,
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

/// A typed first-class callable value.
///
/// The frontend stores the callable body structurally. The evaluator fills
/// `capture` when the value is materialized, keeping lexical capture as an
/// immutable semantic snapshot rather than a pointer into mutable evaluator
/// state.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct IrCallable {
    pub parameters: Option<Vec<IrParameter>>,
    pub body: Vec<IrNode>,
    pub span: SourceSpan,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capture: Option<Box<IrCallableCapture>>,
}

/// A source-backed inline argument whose callable interpretation is selected
/// by the resolved callee. Native iteration consumes `parameters` and `body`;
/// ordinary positional parameters receive `content` instead.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct IrInlineBody {
    pub content: Vec<IrNode>,
    pub parameters: Option<Vec<IrParameter>>,
    pub body: Vec<IrNode>,
    pub span: SourceSpan,
}

/// Immutable lexical bindings captured by a first-class callable.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct IrCallableCapture {
    pub variables: Vec<IrCapturedVariable>,
    pub functions: Vec<IrCapturedFunction>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct IrCapturedVariable {
    pub name: String,
    pub value: IrValue,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct IrCapturedFunction {
    pub name: String,
    pub callable: IrCallable,
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

/// A table cell with structured inline content and source provenance.
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

/// A semantic value used in function call arguments and output materialization.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum IrValue {
    String(String),
    Number(f64),
    Boolean(bool),
    Identifier(String),
    Size(IrSize),
    Color(IrColor),
    Enum(IrEnumValue),
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
    /// A completed backend-neutral semantic component. Components remain
    /// typed values until an output boundary can materialize them losslessly.
    Component(IrComponent),
    /// The Quarkdown language's explicit absence value.
    ///
    /// This is a semantic value, distinct from an evaluator `NoValue`
    /// outcome. It remains typed until an output boundary materializes it.
    None,
    /// A first-class typed callable. It is consumed by the evaluator and must
    /// never be lowered as a backend expression.
    Callable(IrCallable),
    /// A contextual inline argument. It is resolved to ordinary content or a
    /// callable by the evaluator after the callee binding is selected.
    InlineBody(IrInlineBody),
}

#[cfg(test)]
mod tests {
    use super::{
        IrCaptionPosition, IrCaptionPositionInfo, IrComponent, IrContainerAlignment,
        IrContainerComponent, IrCrossAxisAlignment, IrDictionary, IrDocumentAuthor,
        IrDocumentLocale, IrDocumentState, IrDocumentTheme, IrDocumentType, IrInline,
        IrLandscapeComponent, IrMainAxisAlignment, IrMetadata, IrNode, IrPair, IrRange, IrSize,
        IrSizeUnit, IrStackedComponent, IrStackedLayout, IrValue, NativeTarget,
        TargetSpecificContent,
    };
    use scribium_source::{SourceId, SourceSpan};
    use std::num::NonZeroU32;

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

    fn stacked_value(layout: IrStackedLayout) -> IrValue {
        let child_span = SourceSpan::new(SourceId(7), 12, 19);
        let (main_axis_alignment, row_gap, column_gap) = match &layout {
            IrStackedLayout::Row => (
                IrMainAxisAlignment::Start,
                None,
                Some(IrSize {
                    value: 10.0,
                    unit: IrSizeUnit::Px,
                }),
            ),
            IrStackedLayout::Column => (
                IrMainAxisAlignment::Start,
                Some(IrSize {
                    value: 10.0,
                    unit: IrSizeUnit::Px,
                }),
                None,
            ),
            IrStackedLayout::Grid { .. } => (
                IrMainAxisAlignment::Center,
                Some(IrSize {
                    value: 8.0,
                    unit: IrSizeUnit::Px,
                }),
                Some(IrSize {
                    value: 12.0,
                    unit: IrSizeUnit::Px,
                }),
            ),
        };
        IrValue::Component(IrComponent::Stacked(IrStackedComponent {
            layout,
            main_axis_alignment,
            cross_axis_alignment: IrCrossAxisAlignment::Center,
            row_gap,
            column_gap,
            children: vec![IrNode::Paragraph {
                content: vec![IrInline::Text {
                    content: "child".to_string(),
                    span: child_span,
                }],
                span: child_span,
            }],
            span: SourceSpan::new(SourceId(7), 0, 24),
        }))
    }

    fn container_value() -> IrValue {
        let child_span = SourceSpan::new(SourceId(8), 14, 19);
        IrValue::Component(IrComponent::Container(IrContainerComponent {
            width: None,
            height: None,
            full_width: true,
            alignment: Some(IrContainerAlignment::Center),
            children: vec![IrNode::Paragraph {
                content: vec![IrInline::Text {
                    content: "child".to_string(),
                    span: child_span,
                }],
                span: child_span,
            }],
            span: SourceSpan::new(SourceId(8), 0, 28),
        }))
    }

    fn landscape_value() -> IrValue {
        let child_span = SourceSpan::new(SourceId(10), 12, 17);
        IrValue::Component(IrComponent::Landscape(IrLandscapeComponent {
            children: vec![IrNode::Paragraph {
                content: vec![IrInline::Text {
                    content: "child".to_string(),
                    span: child_span,
                }],
                span: child_span,
            }],
            span: SourceSpan::new(SourceId(10), 0, 20),
        }))
    }

    #[test]
    fn stacked_components_roundtrip_deterministically_for_row_column_and_grid() {
        let values = [
            stacked_value(IrStackedLayout::Row),
            stacked_value(IrStackedLayout::Column),
            stacked_value(IrStackedLayout::Grid {
                columns: NonZeroU32::new(3).expect("test grid columns are non-zero"),
            }),
        ];

        for value in values {
            let first = serde_json::to_string(&value).expect("component serializes");
            let second = serde_json::to_string(&value).expect("component serializes");
            assert_eq!(first, second);
            assert!(!first.contains("typst"));
            assert!(!first.contains("stack("));
            assert!(!first.contains("grid("));
            assert!(!first.contains("#stack"));
            assert!(!first.contains("#grid"));
            assert!(!first.contains("gutter"));
            assert!(!first.contains("align("));
            assert!(!first.contains("dir:"));
            assert!(!first.contains("ltr"));
            assert!(!first.contains("ttb"));
            assert_eq!(
                serde_json::from_str::<IrValue>(&first).expect("component deserializes"),
                value
            );
        }
    }

    #[test]
    fn grid_layout_rejects_zero_columns_during_deserialization() {
        let result = serde_json::from_value::<IrStackedLayout>(serde_json::json!({
            "Grid": { "columns": 0 }
        }));
        assert!(result.is_err(), "zero grid columns must be rejected");
    }

    #[test]
    fn component_roundtrip_preserves_component_and_child_provenance() {
        let value = stacked_value(IrStackedLayout::Row);
        let encoded = serde_json::to_value(&value).expect("component serializes");
        let decoded = serde_json::from_value::<IrValue>(encoded).expect("component deserializes");
        assert_eq!(decoded, value);

        let IrValue::Component(IrComponent::Stacked(component)) = decoded else {
            panic!("expected a stacked component");
        };
        assert_eq!(component.span, SourceSpan::new(SourceId(7), 0, 24));
        assert_eq!(
            component.children[0],
            IrNode::Paragraph {
                content: vec![IrInline::Text {
                    content: "child".to_string(),
                    span: SourceSpan::new(SourceId(7), 12, 19),
                }],
                span: SourceSpan::new(SourceId(7), 12, 19),
            }
        );
    }

    #[test]
    fn container_component_serde_roundtrip() {
        let value = container_value();
        let first = serde_json::to_string(&value).expect("container serializes");
        let second = serde_json::to_string(&value).expect("container serializes");
        assert_eq!(first, second);
        assert!(!first.contains("typst"));
        assert!(!first.contains("#block"));
        assert!(!first.contains("#align"));
        assert!(!first.contains("width: 100%"));
        assert!(!first.contains("center("));
        assert_eq!(
            serde_json::from_str::<IrValue>(&first).expect("container deserializes"),
            value
        );
    }

    #[test]
    fn plain_and_sized_container_serde_roundtrip_deterministically() {
        let span = SourceSpan::new(SourceId(9), 0, 12);
        let values = [
            IrValue::Component(IrComponent::Container(IrContainerComponent {
                width: None,
                height: None,
                full_width: false,
                alignment: None,
                children: Vec::new(),
                span,
            })),
            IrValue::Component(IrComponent::Container(IrContainerComponent {
                width: Some(IrSize {
                    value: 4.0,
                    unit: IrSizeUnit::Cm,
                }),
                height: Some(IrSize {
                    value: 2.0,
                    unit: IrSizeUnit::Cm,
                }),
                full_width: true,
                alignment: None,
                children: Vec::new(),
                span,
            })),
        ];

        for value in values {
            let first = serde_json::to_string(&value).expect("container serializes");
            let second = serde_json::to_string(&value).expect("container serializes");
            assert_eq!(first, second);
            assert_eq!(serde_json::from_str::<IrValue>(&first).unwrap(), value);
        }
    }

    #[test]
    fn pre_sizing_container_serde_defaults_new_fields_and_preserves_alignment() {
        let value = container_value();
        let mut old = serde_json::to_value(&value).expect("container serializes");
        let container = old
            .get_mut("Component")
            .and_then(|component| component.get_mut("Container"))
            .and_then(serde_json::Value::as_object_mut)
            .expect("externally tagged container object");
        container.remove("width");
        container.remove("height");

        let decoded = serde_json::from_value::<IrValue>(old).expect("old container deserializes");
        assert_eq!(decoded, value);
        let IrValue::Component(IrComponent::Container(component)) = decoded else {
            panic!("expected container component");
        };
        assert_eq!(component.width, None);
        assert_eq!(component.height, None);
        assert_eq!(component.alignment, Some(IrContainerAlignment::Center));
    }

    #[test]
    fn container_component_preserves_child_and_call_spans() {
        let IrValue::Component(IrComponent::Container(component)) = container_value() else {
            panic!("expected a container component");
        };
        assert_eq!(component.span, SourceSpan::new(SourceId(8), 0, 28));
        assert_eq!(
            component.children[0],
            IrNode::Paragraph {
                content: vec![IrInline::Text {
                    content: "child".to_string(),
                    span: SourceSpan::new(SourceId(8), 14, 19),
                }],
                span: SourceSpan::new(SourceId(8), 14, 19),
            }
        );
    }

    #[test]
    fn landscape_component_serde_roundtrips_deterministically() {
        let value = landscape_value();
        let first = serde_json::to_string(&value).expect("landscape serializes");
        let second = serde_json::to_string(&value).expect("landscape serializes");
        assert_eq!(first, second);
        assert!(!first.contains("typst"));
        assert!(!first.contains("rotate"));
        assert_eq!(serde_json::from_str::<IrValue>(&first).unwrap(), value);
    }

    #[test]
    fn landscape_component_preserves_child_and_call_spans() {
        let IrValue::Component(IrComponent::Landscape(component)) = landscape_value() else {
            panic!("expected landscape component");
        };
        assert_eq!(component.span, SourceSpan::new(SourceId(10), 0, 20));
        assert_eq!(
            component.children[0],
            IrNode::Paragraph {
                content: vec![IrInline::Text {
                    content: "child".to_string(),
                    span: SourceSpan::new(SourceId(10), 12, 17),
                }],
                span: SourceSpan::new(SourceId(10), 12, 17),
            }
        );
        assert_eq!(
            IrComponent::Landscape(component).span(),
            SourceSpan::new(SourceId(10), 0, 20)
        );
    }

    #[test]
    fn target_specific_content_roundtrips_in_block_and_inline_carriers() {
        let content = TargetSpecificContent {
            target: NativeTarget::Html,
            content: "<em>world</em>".to_string(),
            span: SourceSpan::new(SourceId(1), 4, 18),
        };
        let block = IrNode::TargetSpecificContent {
            content: content.clone(),
        };
        let inline = IrInline::TargetSpecificContent { content };

        let block_json = serde_json::to_value(&block).expect("block target content serializes");
        let inline_json = serde_json::to_value(&inline).expect("inline target content serializes");
        assert_eq!(serde_json::from_value::<IrNode>(block_json).unwrap(), block);
        assert_eq!(
            serde_json::from_value::<IrInline>(inline_json).unwrap(),
            inline
        );
    }

    #[test]
    fn document_state_roundtrips_deterministically_and_defaults_for_old_ir() {
        assert!(IrDocumentState::default().keywords.is_empty());
        assert!(IrDocumentState::default().theme.is_none());
        assert!(IrDocumentState::default().locale.is_none());
        assert_eq!(
            IrDocumentState::default().caption_position,
            IrCaptionPositionInfo::default()
        );
        let metadata = IrMetadata {
            document_state: IrDocumentState {
                name: "Document".to_string(),
                description: "Description".to_string(),
                document_type: IrDocumentType::Paged,
                authors: vec![
                    IrDocumentAuthor {
                        name: "Alice".to_string(),
                        info: vec![
                            ("email".to_string(), "alice@example.com".to_string()),
                            ("website".to_string(), "alice.example".to_string()),
                        ],
                    },
                    IrDocumentAuthor {
                        name: "Bob".to_string(),
                        info: Vec::new(),
                    },
                ],
                keywords: vec!["quarkdown".to_string(), "documents".to_string()],
                theme: Some(IrDocumentTheme {
                    color: Some("dark".to_string()),
                    layout: Some("compact".to_string()),
                }),
                locale: Some(IrDocumentLocale {
                    tag: "en-US".to_string(),
                    localized_name: "English (United States)".to_string(),
                }),
                caption_position: IrCaptionPositionInfo {
                    default: IrCaptionPosition::Top,
                    figures: Some(IrCaptionPosition::Bottom),
                    tables: None,
                    code_blocks: Some(IrCaptionPosition::Top),
                },
            },
            ..IrMetadata::default()
        };
        let first = serde_json::to_string(&metadata).expect("document metadata serializes");
        let second = serde_json::to_string(&metadata).expect("document metadata serializes");
        assert_eq!(first, second);
        assert_eq!(
            serde_json::from_str::<IrMetadata>(&first).expect("document metadata deserializes"),
            metadata
        );

        let old_metadata = serde_json::json!({
            "title": null,
            "author": null,
            "date": null,
            "raw": []
        });
        assert_eq!(
            serde_json::from_value::<IrMetadata>(old_metadata)
                .expect("old metadata remains readable")
                .document_state,
            IrDocumentState::default()
        );

        let old_state = serde_json::json!({
            "name": "Document",
            "description": "Description"
        });
        assert_eq!(
            serde_json::from_value::<IrDocumentState>(old_state)
                .expect("old document state remains readable")
                .document_type,
            IrDocumentType::Plain
        );
        assert_eq!(
            serde_json::from_value::<IrDocumentState>(serde_json::json!({
                "name": "Document",
                "description": "Description"
            }))
            .expect("old caption-position-less document state remains readable")
            .caption_position,
            IrCaptionPositionInfo::default()
        );
        assert!(
            serde_json::from_value::<IrDocumentState>(serde_json::json!({
                "name": "Document",
                "description": "Description",
                "document_type": "Plain",
                "authors": [],
                "keywords": []
            }))
            .expect("old theme-less document state remains readable")
            .theme
            .is_none()
        );
        assert_eq!(
            serde_json::from_value::<IrDocumentState>(serde_json::json!({
                "name": "Document",
                "description": "Description",
                "document_type": "Plain",
                "authors": [],
                "keywords": [],
                "theme": null
            }))
            .expect("old locale-less document state remains readable")
            .locale,
            None
        );

        assert_eq!(
            serde_json::from_value::<IrDocumentState>(serde_json::json!({
                "name": "Document",
                "description": "Description",
                "document_type": "Plain"
            }))
            .expect("old author-less document state remains readable")
            .authors,
            Vec::new()
        );
        assert_eq!(
            serde_json::from_value::<IrDocumentState>(serde_json::json!({
                "name": "Document",
                "description": "Description",
                "document_type": "Plain",
                "authors": []
            }))
            .expect("old keyword-less document state remains readable")
            .keywords,
            Vec::<String>::new()
        );

        let old_author = serde_json::json!({ "name": "Alice" });
        assert_eq!(
            serde_json::from_value::<IrDocumentAuthor>(old_author)
                .expect("old author objects remain readable")
                .info,
            Vec::<(String, String)>::new()
        );

        let state = IrDocumentState {
            name: String::new(),
            description: String::new(),
            document_type: IrDocumentType::Plain,
            authors: vec![IrDocumentAuthor {
                name: "Ordered".to_string(),
                info: vec![
                    ("first".to_string(), "one".to_string()),
                    ("second".to_string(), "two".to_string()),
                ],
            }],
            keywords: vec!["first".to_string(), "second".to_string()],
            theme: Some(IrDocumentTheme {
                color: None,
                layout: None,
            }),
            locale: Some(IrDocumentLocale {
                tag: "it".to_string(),
                localized_name: "italiano".to_string(),
            }),
            caption_position: IrCaptionPositionInfo {
                default: IrCaptionPosition::Top,
                figures: None,
                tables: Some(IrCaptionPosition::Bottom),
                code_blocks: None,
            },
        };
        let serialized = serde_json::to_string(&state).expect("ordered author state serializes");
        assert_eq!(
            serde_json::from_str::<IrDocumentState>(&serialized)
                .expect("ordered author state deserializes"),
            state
        );
    }

    #[test]
    fn document_theme_component_shapes_roundtrip_without_collapsing_empty_state() {
        let shapes = [
            IrDocumentTheme {
                color: Some("dark".to_string()),
                layout: None,
            },
            IrDocumentTheme {
                color: None,
                layout: Some("compact".to_string()),
            },
            IrDocumentTheme {
                color: Some("dark".to_string()),
                layout: Some("compact".to_string()),
            },
            IrDocumentTheme {
                color: None,
                layout: None,
            },
        ];

        for theme in shapes {
            let state = IrDocumentState {
                theme: Some(theme),
                ..IrDocumentState::default()
            };
            let serialized = serde_json::to_string(&state).expect("theme state serializes");
            assert_eq!(
                serde_json::from_str::<IrDocumentState>(&serialized)
                    .expect("theme state deserializes"),
                state
            );
        }

        assert_ne!(
            Some(IrDocumentTheme {
                color: None,
                layout: None,
            }),
            IrDocumentState::default().theme
        );
    }
}
