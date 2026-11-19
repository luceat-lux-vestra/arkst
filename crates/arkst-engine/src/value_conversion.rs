//! Engine-owned target conversion for the evaluator value boundary.
//!
//! Conversion is target-driven, not a general coercion graph. This module
//! keeps the upstream distinction between already typed values, dynamic raw
//! values, and structured Markdown content. Context-sensitive targets return a
//! raw conversion request for the evaluator to execute with its context; they
//! never stringify an evaluated value and feed it back through Markdown.

use arkst_ir::{
    IrBoxType, IrCallable, IrCallableCapture, IrCallableResourceContext, IrCaptionPosition,
    IrCapturedFunction, IrCapturedVariable, IrClipShape, IrColor, IrComponent,
    IrContainerAlignment, IrCrossAxisAlignment, IrDictionary, IrDocumentAlignment, IrDocumentType,
    IrEnumValue, IrFragmentBehavior, IrInline, IrInlineBody, IrMainAxisAlignment, IrNode,
    IrPageMarginPosition, IrPageOrientation, IrPageSide, IrPageSizeFormat, IrPair, IrParameter,
    IrRange, IrRawBody, IrSize, IrSizeUnit, IrSlideTransitionSpeed, IrSlideTransitionStyle,
    IrTextCase, IrTextDecoration, IrTextSize, IrTextStyle, IrTextVariant, IrTextWeight, IrValue,
};
use arkst_source::{SourceId, SourceSpan};
use std::borrow::Cow;

/// Materialize the Quarkdown body `DynamicValue` only at a conversion target
/// that explicitly requests it. The source slice is lossless and shared;
/// this is the sole engine-owned `trimIndent().trimEnd()` derivation used by
/// raw body conversion.
pub(crate) fn raw_body_dynamic_text(raw_body: &IrRawBody) -> Option<String> {
    raw_body
        .source
        .slice(raw_body.span)
        .map(trim_indent_and_end)
}

/// Mirrors Kotlin's `String.trimIndent()` followed by `trimEnd()` for a body
/// DynamicValue. Line endings are normalized to LF by the semantic value;
/// the source-backed representation itself remains lossless.
fn trim_indent_and_end(source: &str) -> String {
    let lines = split_lines(source);
    let mut start = 0;
    let mut end = lines.len();
    if lines.get(start).is_some_and(|line| line.is_blank()) {
        start += 1;
    }
    if end > start && lines.get(end - 1).is_some_and(|line| line.is_blank()) {
        end -= 1;
    }

    let minimum_indent = lines[start..end]
        .iter()
        .filter(|line| !line.is_blank())
        .map(|line| line.leading_whitespace_chars())
        .min()
        .unwrap_or(0);

    let mut result = String::new();
    for (index, line) in lines[start..end].iter().enumerate() {
        if index > 0 {
            result.push('\n');
        }
        result.push_str(drop_chars(line, minimum_indent));
    }
    result.trim_end().to_owned()
}

fn split_lines(source: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut start = 0;
    let bytes = source.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if matches!(bytes[index], b'\n' | b'\r') {
            lines.push(&source[start..index]);
            if bytes[index] == b'\r' && bytes.get(index + 1) == Some(&b'\n') {
                index += 1;
            }
            index += 1;
            start = index;
        } else {
            index += 1;
        }
    }
    lines.push(&source[start..]);
    lines
}

trait BodyLine {
    fn is_blank(&self) -> bool;
    fn leading_whitespace_chars(&self) -> usize;
}

impl BodyLine for &str {
    fn is_blank(&self) -> bool {
        self.chars().all(char::is_whitespace)
    }

    fn leading_whitespace_chars(&self) -> usize {
        self.chars()
            .take_while(|character| character.is_whitespace())
            .count()
    }
}

fn drop_chars(line: &str, count: usize) -> &str {
    line.char_indices()
        .nth(count)
        .map_or("", |(index, _)| &line[index..])
}

/// Origin of a value at a Quarkdown invocation boundary.
///
/// This is evaluator-internal metadata. It is intentionally not part of
/// `IrValue`: the final IR contains only typed semantic values. `Dynamic`
/// corresponds to the upstream `DynamicValue` binder path; `Static` is an
/// already materialized typed value such as the result of a nested
/// `.string` call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValueOrigin {
    Static,
    Dynamic,
}

/// Evaluator-local provenance attached to exactly one runtime value.
///
/// Provenance never forms a parallel aggregate tree: nested runtime values own
/// their own provenance. Durable `IrValue` and backend lowering never observe
/// this metadata.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) enum ValueProvenance {
    #[default]
    Materialized,
    SourceBackedMarkdown {
        source_text: Option<String>,
    },
}

impl ValueProvenance {
    #[allow(dead_code)]
    pub(crate) fn source_backed_markdown() -> Self {
        Self::SourceBackedMarkdown { source_text: None }
    }

    pub(crate) fn source_backed_markdown_text(source_text: impl Into<String>) -> Self {
        Self::SourceBackedMarkdown {
            source_text: Some(source_text.into()),
        }
    }

    pub(crate) fn source_text(&self) -> Option<&str> {
        match self {
            Self::SourceBackedMarkdown {
                source_text: Some(source_text),
            } => Some(source_text),
            _ => None,
        }
    }

    pub(crate) fn is_materialized(&self) -> bool {
        matches!(self, Self::Materialized)
    }
}

#[derive(Debug, Clone, PartialEq)]
struct RuntimePair {
    first: Box<RuntimeValue>,
    second: Box<RuntimeValue>,
    span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq)]
struct RuntimeDictionary {
    entries: Vec<RuntimeValue>,
    span: SourceSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeCallableResourceContext {
    pub(crate) current_source: SourceId,
    pub(crate) subdocument_root: Option<SourceId>,
    pub(crate) loadable_libraries: bool,
}

impl From<IrCallableResourceContext> for RuntimeCallableResourceContext {
    fn from(value: IrCallableResourceContext) -> Self {
        Self {
            current_source: value.current_source,
            subdocument_root: value.subdocument_root,
            loadable_libraries: value.loadable_libraries,
        }
    }
}

impl From<RuntimeCallableResourceContext> for IrCallableResourceContext {
    fn from(value: RuntimeCallableResourceContext) -> Self {
        Self {
            current_source: value.current_source,
            subdocument_root: value.subdocument_root,
            loadable_libraries: value.loadable_libraries,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RuntimeCapturedVariable {
    pub(crate) name: String,
    pub(crate) value: RuntimeValue,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RuntimeCapturedFunction {
    pub(crate) name: String,
    pub(crate) callable: RuntimeCallable,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RuntimeCallableCapture {
    pub(crate) variables: Vec<RuntimeCapturedVariable>,
    pub(crate) functions: Vec<RuntimeCapturedFunction>,
    pub(crate) resource_context: Option<RuntimeCallableResourceContext>,
}

impl RuntimeCallableCapture {
    fn from_ir(value: IrCallableCapture) -> Self {
        Self {
            variables: value
                .variables
                .into_iter()
                .map(|variable| RuntimeCapturedVariable {
                    name: variable.name,
                    value: RuntimeValue::static_value(variable.value),
                })
                .collect(),
            functions: value
                .functions
                .into_iter()
                .map(|function| RuntimeCapturedFunction {
                    name: function.name,
                    callable: RuntimeCallable::from_ir(function.callable),
                })
                .collect(),
            resource_context: value.resource_context.map(Into::into),
        }
    }

    fn to_ir(&self) -> IrCallableCapture {
        IrCallableCapture {
            variables: self
                .variables
                .iter()
                .map(|variable| IrCapturedVariable {
                    name: variable.name.clone(),
                    value: variable.value.to_ir_value(),
                })
                .collect(),
            functions: self
                .functions
                .iter()
                .map(|function| IrCapturedFunction {
                    name: function.name.clone(),
                    callable: function.callable.to_ir(),
                })
                .collect(),
            resource_context: self.resource_context.map(Into::into),
        }
    }

    fn into_ir(self) -> IrCallableCapture {
        IrCallableCapture {
            variables: self
                .variables
                .into_iter()
                .map(|variable| IrCapturedVariable {
                    name: variable.name,
                    value: variable.value.into_ir_value(),
                })
                .collect(),
            functions: self
                .functions
                .into_iter()
                .map(|function| IrCapturedFunction {
                    name: function.name,
                    callable: function.callable.into_ir(),
                })
                .collect(),
            resource_context: self.resource_context.map(Into::into),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RuntimeCallable {
    pub(crate) parameters: Option<Vec<IrParameter>>,
    pub(crate) body: Vec<IrNode>,
    pub(crate) span: SourceSpan,
    pub(crate) capture: Option<Box<RuntimeCallableCapture>>,
}

impl RuntimeCallable {
    pub(crate) fn from_ir(value: IrCallable) -> Self {
        Self {
            parameters: value.parameters,
            body: value.body,
            span: value.span,
            capture: value
                .capture
                .map(|capture| Box::new(RuntimeCallableCapture::from_ir(*capture))),
        }
    }

    pub(crate) fn to_ir(&self) -> IrCallable {
        IrCallable {
            parameters: self.parameters.clone(),
            body: self.body.clone(),
            span: self.span,
            capture: self
                .capture
                .as_deref()
                .map(|capture| Box::new(capture.to_ir())),
        }
    }

    pub(crate) fn into_ir(self) -> IrCallable {
        IrCallable {
            parameters: self.parameters,
            body: self.body,
            span: self.span,
            capture: self.capture.map(|capture| Box::new((*capture).into_ir())),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RuntimeInlineBody {
    pub(crate) content: Vec<IrNode>,
    pub(crate) parameters: Option<Vec<IrParameter>>,
    pub(crate) body: Vec<IrNode>,
    pub(crate) span: SourceSpan,
}

impl RuntimeInlineBody {
    fn from_ir(value: IrInlineBody) -> Self {
        Self {
            content: value.content,
            parameters: value.parameters,
            body: value.body,
            span: value.span,
        }
    }

    fn to_ir(&self) -> IrInlineBody {
        IrInlineBody {
            content: self.content.clone(),
            parameters: self.parameters.clone(),
            body: self.body.clone(),
            span: self.span,
        }
    }

    fn into_ir(self) -> IrInlineBody {
        IrInlineBody {
            content: self.content,
            parameters: self.parameters,
            body: self.body,
            span: self.span,
        }
    }

    fn into_callable(self) -> RuntimeCallable {
        RuntimeCallable {
            parameters: self.parameters,
            body: self.body,
            span: self.span,
            capture: None,
        }
    }
}

/// First evaluator-owned runtime value carrier.
///
/// All runtime value families are evaluator-owned; nested values retain their
/// own origin and provenance. Callable captures recursively retain runtime
/// values and evaluator-only lexical resource identity without storing
/// execution-stage state in durable `IrValue`.
#[derive(Debug, Clone, PartialEq)]
enum RuntimeValueKind {
    Unit,
    None,
    Boolean(bool),
    Number(f64),
    String(String),
    Identifier(String),
    Range(IrRange),
    Size(IrSize),
    Color(IrColor),
    Enum(IrEnumValue),
    Pair(RuntimePair),
    Dictionary(RuntimeDictionary),
    Content(Vec<IrNode>),
    Component(IrComponent),
    Callable(RuntimeCallable),
    InlineBody(RuntimeInlineBody),
    Collection(Vec<RuntimeValue>),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RuntimeValue {
    kind: RuntimeValueKind,
    pub origin: ValueOrigin,
    provenance: ValueProvenance,
}

impl RuntimeValue {
    pub(crate) fn new(value: IrValue, origin: ValueOrigin) -> Self {
        let kind = match value {
            IrValue::Unit => RuntimeValueKind::Unit,
            IrValue::None => RuntimeValueKind::None,
            IrValue::Boolean(value) => RuntimeValueKind::Boolean(value),
            IrValue::Number(value) => RuntimeValueKind::Number(value),
            IrValue::String(value) => RuntimeValueKind::String(value),
            IrValue::Identifier(value) => RuntimeValueKind::Identifier(value),
            IrValue::Range(value) => RuntimeValueKind::Range(value),
            IrValue::Size(value) => RuntimeValueKind::Size(value),
            IrValue::Color(value) => RuntimeValueKind::Color(value),
            IrValue::Enum(value) => RuntimeValueKind::Enum(value),
            IrValue::Pair(value) => {
                let IrPair {
                    first,
                    second,
                    span,
                } = value;
                return Self::pair(
                    Self::new(*first, origin),
                    Self::new(*second, origin),
                    span,
                    origin,
                );
            }
            IrValue::Dictionary(value) => {
                let IrDictionary { entries, span } = value;
                let entries = entries
                    .into_iter()
                    .map(|entry| {
                        let IrPair {
                            first,
                            second,
                            span,
                        } = entry;
                        Self::pair(
                            Self::new(*first, origin),
                            Self::new(*second, origin),
                            span,
                            origin,
                        )
                    })
                    .collect();
                return Self::dictionary(entries, span, origin)
                    .expect("IR dictionary entries are always pairs");
            }
            IrValue::Content(value) => RuntimeValueKind::Content(value),
            IrValue::Component(value) => RuntimeValueKind::Component(value),
            IrValue::Callable(value) => RuntimeValueKind::Callable(RuntimeCallable::from_ir(value)),
            IrValue::InlineBody(value) => {
                RuntimeValueKind::InlineBody(RuntimeInlineBody::from_ir(value))
            }
            IrValue::Collection(values) => {
                return Self::collection(
                    values
                        .into_iter()
                        .map(|value| Self::new(value, origin))
                        .collect(),
                    origin,
                );
            }
        };
        Self {
            kind,
            origin,
            provenance: ValueProvenance::Materialized,
        }
    }

    pub(crate) fn static_value(value: IrValue) -> Self {
        Self::new(value, ValueOrigin::Static)
    }

    #[allow(dead_code)]
    pub(crate) fn dynamic_value(value: IrValue) -> Self {
        Self::new(value, ValueOrigin::Dynamic)
    }

    pub(crate) fn pair(first: Self, second: Self, span: SourceSpan, origin: ValueOrigin) -> Self {
        Self {
            kind: RuntimeValueKind::Pair(RuntimePair {
                first: Box::new(first),
                second: Box::new(second),
                span,
            }),
            origin,
            provenance: ValueProvenance::Materialized,
        }
    }

    pub(crate) fn dictionary(
        entries: Vec<Self>,
        span: SourceSpan,
        origin: ValueOrigin,
    ) -> Option<Self> {
        if entries
            .iter()
            .any(|entry| !matches!(&entry.kind, RuntimeValueKind::Pair(_)))
        {
            return None;
        }
        Some(Self {
            kind: RuntimeValueKind::Dictionary(RuntimeDictionary { entries, span }),
            origin,
            provenance: ValueProvenance::Materialized,
        })
    }

    pub(crate) fn collection(elements: Vec<Self>, origin: ValueOrigin) -> Self {
        Self {
            kind: RuntimeValueKind::Collection(elements),
            origin,
            provenance: ValueProvenance::Materialized,
        }
    }

    pub(crate) fn with_origin(mut self, origin: ValueOrigin) -> Self {
        self.origin = origin;
        self
    }

    #[allow(dead_code)]
    pub(crate) fn with_provenance(mut self, provenance: ValueProvenance) -> Self {
        self.provenance = provenance;
        self
    }

    pub(crate) fn set_provenance(&mut self, provenance: ValueProvenance) {
        self.provenance = provenance;
    }

    pub(crate) fn provenance(&self) -> &ValueProvenance {
        &self.provenance
    }

    pub(crate) fn source_backed_markdown_text(&self) -> Option<&str> {
        self.provenance.source_text()
    }

    pub(crate) fn is_content(&self) -> bool {
        matches!(&self.kind, RuntimeValueKind::Content(_))
    }

    pub(crate) fn is_iterable_aggregate(&self) -> bool {
        matches!(
            &self.kind,
            RuntimeValueKind::Collection(_)
                | RuntimeValueKind::Pair(_)
                | RuntimeValueKind::Dictionary(_)
        )
    }

    pub(crate) fn is_unit(&self) -> bool {
        matches!(&self.kind, RuntimeValueKind::Unit)
    }

    pub(crate) fn into_pair_parts(self) -> Option<(Self, Self, SourceSpan)> {
        let RuntimeValueKind::Pair(pair) = self.kind else {
            return None;
        };
        Some((*pair.first, *pair.second, pair.span))
    }

    pub(crate) fn into_dictionary_entries(self) -> Option<(Vec<Self>, SourceSpan)> {
        let RuntimeValueKind::Dictionary(dictionary) = self.kind else {
            return None;
        };
        Some((dictionary.entries, dictionary.span))
    }

    pub(crate) fn callable(value: RuntimeCallable, origin: ValueOrigin) -> Self {
        Self {
            kind: RuntimeValueKind::Callable(value),
            origin,
            provenance: ValueProvenance::Materialized,
        }
    }

    pub(crate) fn is_callable(&self) -> bool {
        matches!(&self.kind, RuntimeValueKind::Callable(_))
    }

    pub(crate) fn as_callable(&self) -> Option<&RuntimeCallable> {
        let RuntimeValueKind::Callable(value) = &self.kind else {
            return None;
        };
        Some(value)
    }

    pub(crate) fn into_callable(self) -> Option<RuntimeCallable> {
        let RuntimeValueKind::Callable(value) = self.kind else {
            return None;
        };
        Some(value)
    }

    pub(crate) fn into_callable_target(self) -> Option<RuntimeCallable> {
        match self.kind {
            RuntimeValueKind::Callable(value) => Some(value),
            RuntimeValueKind::InlineBody(value) => Some(value.into_callable()),
            _ => None,
        }
    }

    fn to_ir_pair(&self) -> Option<IrPair> {
        let RuntimeValueKind::Pair(pair) = &self.kind else {
            return None;
        };
        Some(IrPair {
            first: Box::new(pair.first.to_ir_value()),
            second: Box::new(pair.second.to_ir_value()),
            span: pair.span,
        })
    }

    fn into_ir_pair(self) -> Option<IrPair> {
        let RuntimeValueKind::Pair(pair) = self.kind else {
            return None;
        };
        Some(IrPair {
            first: Box::new(pair.first.into_ir_value()),
            second: Box::new(pair.second.into_ir_value()),
            span: pair.span,
        })
    }

    /// Project evaluator-owned runtime values back into the transitional
    /// `IrValue` model only when an existing semantic consumer explicitly
    /// requires that boundary during the G1 migration.
    pub(crate) fn semantic_value(&self) -> Cow<'_, IrValue> {
        match &self.kind {
            RuntimeValueKind::Unit => Cow::Owned(IrValue::Unit),
            RuntimeValueKind::None => Cow::Owned(IrValue::None),
            RuntimeValueKind::Boolean(value) => Cow::Owned(IrValue::Boolean(*value)),
            RuntimeValueKind::Number(value) => Cow::Owned(IrValue::Number(*value)),
            RuntimeValueKind::String(value) => Cow::Owned(IrValue::String(value.clone())),
            RuntimeValueKind::Identifier(value) => Cow::Owned(IrValue::Identifier(value.clone())),
            RuntimeValueKind::Range(value) => Cow::Owned(IrValue::Range(value.clone())),
            RuntimeValueKind::Size(value) => Cow::Owned(IrValue::Size(value.clone())),
            RuntimeValueKind::Color(value) => Cow::Owned(IrValue::Color(value.clone())),
            RuntimeValueKind::Enum(value) => Cow::Owned(IrValue::Enum(*value)),
            RuntimeValueKind::Pair(_) => Cow::Owned(IrValue::Pair(
                self.to_ir_pair().expect("runtime pair invariant"),
            )),
            RuntimeValueKind::Dictionary(value) => Cow::Owned(IrValue::Dictionary(IrDictionary {
                entries: value
                    .entries
                    .iter()
                    .map(|entry| {
                        entry
                            .to_ir_pair()
                            .expect("runtime dictionary entry invariant")
                    })
                    .collect(),
                span: value.span,
            })),
            RuntimeValueKind::Content(value) => Cow::Owned(IrValue::Content(value.clone())),
            RuntimeValueKind::Component(value) => Cow::Owned(IrValue::Component(value.clone())),
            RuntimeValueKind::Callable(value) => Cow::Owned(IrValue::Callable(value.to_ir())),
            RuntimeValueKind::InlineBody(value) => Cow::Owned(IrValue::InlineBody(value.to_ir())),
            RuntimeValueKind::Collection(elements) => Cow::Owned(IrValue::Collection(
                elements.iter().map(Self::to_ir_value).collect(),
            )),
        }
    }

    pub(crate) fn is_none(&self) -> bool {
        matches!(&self.kind, RuntimeValueKind::None)
    }

    fn text_identity(&self) -> Option<&str> {
        match &self.kind {
            RuntimeValueKind::String(value) | RuntimeValueKind::Identifier(value) => Some(value),
            _ => None,
        }
    }

    fn dynamic_text(&self) -> Option<&str> {
        if self.origin != ValueOrigin::Dynamic {
            return None;
        }
        self.text_identity()
    }

    pub(crate) fn replace_semantic_value(&mut self, value: IrValue) {
        let origin = self.origin;
        *self = Self::new(value, origin);
    }

    pub(crate) fn to_ir_value(&self) -> IrValue {
        match &self.kind {
            RuntimeValueKind::Unit => IrValue::Unit,
            RuntimeValueKind::None => IrValue::None,
            RuntimeValueKind::Boolean(value) => IrValue::Boolean(*value),
            RuntimeValueKind::Number(value) => IrValue::Number(*value),
            RuntimeValueKind::String(value) => IrValue::String(value.clone()),
            RuntimeValueKind::Identifier(value) => IrValue::Identifier(value.clone()),
            RuntimeValueKind::Range(value) => IrValue::Range(value.clone()),
            RuntimeValueKind::Size(value) => IrValue::Size(value.clone()),
            RuntimeValueKind::Color(value) => IrValue::Color(value.clone()),
            RuntimeValueKind::Enum(value) => IrValue::Enum(*value),
            RuntimeValueKind::Pair(_) => {
                IrValue::Pair(self.to_ir_pair().expect("runtime pair invariant"))
            }
            RuntimeValueKind::Dictionary(value) => IrValue::Dictionary(IrDictionary {
                entries: value
                    .entries
                    .iter()
                    .map(|entry| {
                        entry
                            .to_ir_pair()
                            .expect("runtime dictionary entry invariant")
                    })
                    .collect(),
                span: value.span,
            }),
            RuntimeValueKind::Content(value) => IrValue::Content(value.clone()),
            RuntimeValueKind::Component(value) => IrValue::Component(value.clone()),
            RuntimeValueKind::Callable(value) => IrValue::Callable(value.to_ir()),
            RuntimeValueKind::InlineBody(value) => IrValue::InlineBody(value.to_ir()),
            RuntimeValueKind::Collection(elements) => {
                IrValue::Collection(elements.iter().map(Self::to_ir_value).collect())
            }
        }
    }

    pub(crate) fn into_ir_value(self) -> IrValue {
        match self.kind {
            RuntimeValueKind::Unit => IrValue::Unit,
            RuntimeValueKind::None => IrValue::None,
            RuntimeValueKind::Boolean(value) => IrValue::Boolean(value),
            RuntimeValueKind::Number(value) => IrValue::Number(value),
            RuntimeValueKind::String(value) => IrValue::String(value),
            RuntimeValueKind::Identifier(value) => IrValue::Identifier(value),
            RuntimeValueKind::Range(value) => IrValue::Range(value),
            RuntimeValueKind::Size(value) => IrValue::Size(value),
            RuntimeValueKind::Color(value) => IrValue::Color(value),
            RuntimeValueKind::Enum(value) => IrValue::Enum(value),
            RuntimeValueKind::Pair(pair) => IrValue::Pair(IrPair {
                first: Box::new(pair.first.into_ir_value()),
                second: Box::new(pair.second.into_ir_value()),
                span: pair.span,
            }),
            RuntimeValueKind::Dictionary(value) => IrValue::Dictionary(IrDictionary {
                entries: value
                    .entries
                    .into_iter()
                    .map(|entry| {
                        entry
                            .into_ir_pair()
                            .expect("runtime dictionary entry invariant")
                    })
                    .collect(),
                span: value.span,
            }),
            RuntimeValueKind::Content(value) => IrValue::Content(value),
            RuntimeValueKind::Component(value) => IrValue::Component(value),
            RuntimeValueKind::Callable(value) => IrValue::Callable(value.into_ir()),
            RuntimeValueKind::InlineBody(value) => IrValue::InlineBody(value.into_ir()),
            RuntimeValueKind::Collection(elements) => {
                IrValue::Collection(elements.into_iter().map(Self::into_ir_value).collect())
            }
        }
    }

    pub(crate) fn into_collection(self) -> Option<Vec<Self>> {
        match self {
            Self {
                kind: RuntimeValueKind::Collection(elements),
                ..
            } => Some(elements),
            _ => None,
        }
    }
}

impl From<IrValue> for RuntimeValue {
    fn from(value: IrValue) -> Self {
        Self::static_value(value)
    }
}

impl PartialEq<IrValue> for RuntimeValue {
    fn eq(&self, other: &IrValue) -> bool {
        self.semantic_value().as_ref() == other
    }
}

impl PartialEq<RuntimeValue> for IrValue {
    fn eq(&self, other: &RuntimeValue) -> bool {
        self == other.semantic_value().as_ref()
    }
}

/// One evaluated named argument at the evaluator-owned runtime boundary.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RuntimeNamedArg {
    pub name: String,
    pub name_span: SourceSpan,
    pub value: RuntimeValue,
    pub span: SourceSpan,
}

/// The scalar targets supported by this bounded conversion policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScalarTarget {
    Number,
    Boolean,
    String,
}

/// Conversion targets used when classifying a failure, including the typed
/// Range boundary that is parsed separately because it carries provenance.
/// The content/node/iterable/callable targets are intentionally present even
/// when the current bounded builtin inventory does not consume every one;
/// they keep the shared target model explicit instead of letting consumers
/// invent their own category tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum ConversionTarget {
    Dynamic,
    Number,
    Integer,
    Boolean,
    String,
    Range,
    Size,
    Color,
    Enum,
    InlineContent,
    BlockContent,
    Node,
    Iterable,
    Dictionary,
    Callable,
}

impl ConversionTarget {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Dynamic => "dynamic value",
            Self::Number => "Number",
            Self::Integer => "Integer",
            Self::Boolean => "Boolean",
            Self::String => "String",
            Self::Range => "Range",
            Self::Size => "Size",
            Self::Color => "Color",
            Self::Enum => "closed enum",
            Self::InlineContent => "inline content",
            Self::BlockContent => "block content",
            Self::Node => "Node",
            Self::Iterable => "iterable",
            Self::Dictionary => "dictionary",
            Self::Callable => "callable",
        }
    }
}

impl From<ScalarTarget> for ConversionTarget {
    fn from(target: ScalarTarget) -> Self {
        match target {
            ScalarTarget::Number => Self::Number,
            ScalarTarget::Boolean => Self::Boolean,
            ScalarTarget::String => Self::String,
        }
    }
}

/// A successfully converted scalar value.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ScalarValue {
    Number(f64),
    Boolean(bool),
    String(String),
}

/// Closed enum domains supported by the bounded domain adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClosedEnumTarget {
    DocumentType,
    PageSizeFormat,
    PageOrientation,
    PageSide,
    PageMarginPosition,
    CaptionPosition,
    StackedMainAxisAlignment,
    StackedCrossAxisAlignment,
    ContainerAlignment,
    BoxType,
    ClipShape,
    SlideTransitionStyle,
    SlideTransitionSpeed,
    FragmentBehavior,
}

/// Domain-specific conversion targets kept separate from scalar conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DomainTarget {
    #[allow(dead_code)]
    Size,
    #[allow(dead_code)]
    Color,
    ClosedEnum(ClosedEnumTarget),
}

/// A typed semantic result from a domain conversion.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DomainValue {
    Size(IrSize),
    Color(IrColor),
    Enum(IrEnumValue),
}

/// A closed source-name table. It is intentionally table-driven and has no
/// reflection or stringly-typed enum output.
struct ClosedEnumSpec<'a, T: Copy> {
    variants: &'a [ClosedEnumVariant<T>],
}

struct ClosedEnumVariant<T: Copy> {
    declaration_name: &'static str,
    value: T,
}

impl<T: Copy> ClosedEnumSpec<'_, T> {
    fn value_for(&self, raw: &str) -> Option<T> {
        self.variants
            .iter()
            .find(|variant| quarkdown_name(variant.declaration_name).eq_ignore_ascii_case(raw))
            .map(|variant| variant.value)
    }
}

fn quarkdown_name(name: &str) -> String {
    name.chars()
        .filter(|character| *character != '_')
        .flat_map(char::to_lowercase)
        .collect()
}

static DOCUMENT_TYPE_SPEC: ClosedEnumSpec<'static, IrDocumentType> = ClosedEnumSpec {
    variants: &[
        ClosedEnumVariant {
            declaration_name: "PLAIN",
            value: IrDocumentType::Plain,
        },
        ClosedEnumVariant {
            declaration_name: "PAGED",
            value: IrDocumentType::Paged,
        },
        ClosedEnumVariant {
            declaration_name: "SLIDES",
            value: IrDocumentType::Slides,
        },
        ClosedEnumVariant {
            declaration_name: "DOCS",
            value: IrDocumentType::Docs,
        },
    ],
};

static PAGE_SIZE_FORMAT_SPEC: ClosedEnumSpec<'static, IrPageSizeFormat> = ClosedEnumSpec {
    variants: &[
        ClosedEnumVariant {
            declaration_name: "A0",
            value: IrPageSizeFormat::A0,
        },
        ClosedEnumVariant {
            declaration_name: "A1",
            value: IrPageSizeFormat::A1,
        },
        ClosedEnumVariant {
            declaration_name: "A2",
            value: IrPageSizeFormat::A2,
        },
        ClosedEnumVariant {
            declaration_name: "A3",
            value: IrPageSizeFormat::A3,
        },
        ClosedEnumVariant {
            declaration_name: "A4",
            value: IrPageSizeFormat::A4,
        },
        ClosedEnumVariant {
            declaration_name: "A5",
            value: IrPageSizeFormat::A5,
        },
        ClosedEnumVariant {
            declaration_name: "A6",
            value: IrPageSizeFormat::A6,
        },
        ClosedEnumVariant {
            declaration_name: "A7",
            value: IrPageSizeFormat::A7,
        },
        ClosedEnumVariant {
            declaration_name: "A8",
            value: IrPageSizeFormat::A8,
        },
        ClosedEnumVariant {
            declaration_name: "A9",
            value: IrPageSizeFormat::A9,
        },
        ClosedEnumVariant {
            declaration_name: "A10",
            value: IrPageSizeFormat::A10,
        },
        ClosedEnumVariant {
            declaration_name: "B0",
            value: IrPageSizeFormat::B0,
        },
        ClosedEnumVariant {
            declaration_name: "B1",
            value: IrPageSizeFormat::B1,
        },
        ClosedEnumVariant {
            declaration_name: "B2",
            value: IrPageSizeFormat::B2,
        },
        ClosedEnumVariant {
            declaration_name: "B3",
            value: IrPageSizeFormat::B3,
        },
        ClosedEnumVariant {
            declaration_name: "B4",
            value: IrPageSizeFormat::B4,
        },
        ClosedEnumVariant {
            declaration_name: "B5",
            value: IrPageSizeFormat::B5,
        },
        ClosedEnumVariant {
            declaration_name: "LETTER",
            value: IrPageSizeFormat::Letter,
        },
        ClosedEnumVariant {
            declaration_name: "LEGAL",
            value: IrPageSizeFormat::Legal,
        },
        ClosedEnumVariant {
            declaration_name: "LEDGER",
            value: IrPageSizeFormat::Ledger,
        },
    ],
};

static PAGE_ORIENTATION_SPEC: ClosedEnumSpec<'static, IrPageOrientation> = ClosedEnumSpec {
    variants: &[
        ClosedEnumVariant {
            declaration_name: "PORTRAIT",
            value: IrPageOrientation::Portrait,
        },
        ClosedEnumVariant {
            declaration_name: "LANDSCAPE",
            value: IrPageOrientation::Landscape,
        },
    ],
};

static BOX_TYPE_SPEC: ClosedEnumSpec<'static, IrBoxType> = ClosedEnumSpec {
    variants: &[
        ClosedEnumVariant {
            declaration_name: "CALLOUT",
            value: IrBoxType::Callout,
        },
        ClosedEnumVariant {
            declaration_name: "TIP",
            value: IrBoxType::Tip,
        },
        ClosedEnumVariant {
            declaration_name: "NOTE",
            value: IrBoxType::Note,
        },
        ClosedEnumVariant {
            declaration_name: "WARNING",
            value: IrBoxType::Warning,
        },
        ClosedEnumVariant {
            declaration_name: "ERROR",
            value: IrBoxType::Error,
        },
    ],
};

static CLIP_SHAPE_SPEC: ClosedEnumSpec<'static, IrClipShape> = ClosedEnumSpec {
    variants: &[ClosedEnumVariant {
        declaration_name: "CIRCLE",
        value: IrClipShape::Circle,
    }],
};

static SLIDE_TRANSITION_STYLE_SPEC: ClosedEnumSpec<'static, IrSlideTransitionStyle> =
    ClosedEnumSpec {
        variants: &[
            ClosedEnumVariant {
                declaration_name: "NONE",
                value: IrSlideTransitionStyle::None,
            },
            ClosedEnumVariant {
                declaration_name: "FADE",
                value: IrSlideTransitionStyle::Fade,
            },
            ClosedEnumVariant {
                declaration_name: "SLIDE",
                value: IrSlideTransitionStyle::Slide,
            },
            ClosedEnumVariant {
                declaration_name: "ZOOM",
                value: IrSlideTransitionStyle::Zoom,
            },
        ],
    };

static SLIDE_TRANSITION_SPEED_SPEC: ClosedEnumSpec<'static, IrSlideTransitionSpeed> =
    ClosedEnumSpec {
        variants: &[
            ClosedEnumVariant {
                declaration_name: "DEFAULT",
                value: IrSlideTransitionSpeed::Default,
            },
            ClosedEnumVariant {
                declaration_name: "FAST",
                value: IrSlideTransitionSpeed::Fast,
            },
            ClosedEnumVariant {
                declaration_name: "SLOW",
                value: IrSlideTransitionSpeed::Slow,
            },
        ],
    };

static FRAGMENT_BEHAVIOR_SPEC: ClosedEnumSpec<'static, IrFragmentBehavior> = ClosedEnumSpec {
    variants: &[
        ClosedEnumVariant {
            declaration_name: "SHOW",
            value: IrFragmentBehavior::Show,
        },
        ClosedEnumVariant {
            declaration_name: "HIDE",
            value: IrFragmentBehavior::Hide,
        },
        ClosedEnumVariant {
            declaration_name: "SEMIHIDE",
            value: IrFragmentBehavior::SemiHide,
        },
        ClosedEnumVariant {
            declaration_name: "SHOWHIDE",
            value: IrFragmentBehavior::ShowHide,
        },
    ],
};

static PAGE_SIDE_SPEC: ClosedEnumSpec<'static, IrPageSide> = ClosedEnumSpec {
    variants: &[
        ClosedEnumVariant {
            declaration_name: "LEFT",
            value: IrPageSide::Left,
        },
        ClosedEnumVariant {
            declaration_name: "RIGHT",
            value: IrPageSide::Right,
        },
    ],
};

static PAGE_MARGIN_POSITION_SPEC: ClosedEnumSpec<'static, IrPageMarginPosition> = ClosedEnumSpec {
    variants: &[
        ClosedEnumVariant {
            declaration_name: "TOP_LEFT_CORNER",
            value: IrPageMarginPosition::TopLeftCorner,
        },
        ClosedEnumVariant {
            declaration_name: "TOP_LEFT",
            value: IrPageMarginPosition::TopLeft,
        },
        ClosedEnumVariant {
            declaration_name: "TOP_CENTER",
            value: IrPageMarginPosition::TopCenter,
        },
        ClosedEnumVariant {
            declaration_name: "TOP_RIGHT",
            value: IrPageMarginPosition::TopRight,
        },
        ClosedEnumVariant {
            declaration_name: "TOP_RIGHT_CORNER",
            value: IrPageMarginPosition::TopRightCorner,
        },
        ClosedEnumVariant {
            declaration_name: "RIGHT_TOP",
            value: IrPageMarginPosition::RightTop,
        },
        ClosedEnumVariant {
            declaration_name: "RIGHT_MIDDLE",
            value: IrPageMarginPosition::RightMiddle,
        },
        ClosedEnumVariant {
            declaration_name: "RIGHT_BOTTOM",
            value: IrPageMarginPosition::RightBottom,
        },
        ClosedEnumVariant {
            declaration_name: "BOTTOM_RIGHT_CORNER",
            value: IrPageMarginPosition::BottomRightCorner,
        },
        ClosedEnumVariant {
            declaration_name: "BOTTOM_RIGHT",
            value: IrPageMarginPosition::BottomRight,
        },
        ClosedEnumVariant {
            declaration_name: "BOTTOM_CENTER",
            value: IrPageMarginPosition::BottomCenter,
        },
        ClosedEnumVariant {
            declaration_name: "BOTTOM_LEFT",
            value: IrPageMarginPosition::BottomLeft,
        },
        ClosedEnumVariant {
            declaration_name: "BOTTOM_LEFT_CORNER",
            value: IrPageMarginPosition::BottomLeftCorner,
        },
        ClosedEnumVariant {
            declaration_name: "LEFT_BOTTOM",
            value: IrPageMarginPosition::LeftBottom,
        },
        ClosedEnumVariant {
            declaration_name: "LEFT_MIDDLE",
            value: IrPageMarginPosition::LeftMiddle,
        },
        ClosedEnumVariant {
            declaration_name: "LEFT_TOP",
            value: IrPageMarginPosition::LeftTop,
        },
        ClosedEnumVariant {
            declaration_name: "TOP_OUTSIDE_CORNER",
            value: IrPageMarginPosition::TopOutsideCorner,
        },
        ClosedEnumVariant {
            declaration_name: "TOP_OUTSIDE",
            value: IrPageMarginPosition::TopOutside,
        },
        ClosedEnumVariant {
            declaration_name: "BOTTOM_OUTSIDE_CORNER",
            value: IrPageMarginPosition::BottomOutsideCorner,
        },
        ClosedEnumVariant {
            declaration_name: "BOTTOM_OUTSIDE",
            value: IrPageMarginPosition::BottomOutside,
        },
        ClosedEnumVariant {
            declaration_name: "TOP_INSIDE_CORNER",
            value: IrPageMarginPosition::TopInsideCorner,
        },
        ClosedEnumVariant {
            declaration_name: "TOP_INSIDE",
            value: IrPageMarginPosition::TopInside,
        },
        ClosedEnumVariant {
            declaration_name: "BOTTOM_INSIDE_CORNER",
            value: IrPageMarginPosition::BottomInsideCorner,
        },
        ClosedEnumVariant {
            declaration_name: "BOTTOM_INSIDE",
            value: IrPageMarginPosition::BottomInside,
        },
    ],
};

static CAPTION_POSITION_SPEC: ClosedEnumSpec<'static, IrCaptionPosition> = ClosedEnumSpec {
    variants: &[
        ClosedEnumVariant {
            declaration_name: "TOP",
            value: IrCaptionPosition::Top,
        },
        ClosedEnumVariant {
            declaration_name: "BOTTOM",
            value: IrCaptionPosition::Bottom,
        },
    ],
};

static DOCUMENT_ALIGNMENT_SPEC: ClosedEnumSpec<'static, IrDocumentAlignment> = ClosedEnumSpec {
    variants: &[
        ClosedEnumVariant {
            declaration_name: "START",
            value: IrDocumentAlignment::Start,
        },
        ClosedEnumVariant {
            declaration_name: "CENTER",
            value: IrDocumentAlignment::Center,
        },
        ClosedEnumVariant {
            declaration_name: "END",
            value: IrDocumentAlignment::End,
        },
        ClosedEnumVariant {
            declaration_name: "JUSTIFY",
            value: IrDocumentAlignment::Justify,
        },
    ],
};

static STACKED_MAIN_AXIS_SPEC: ClosedEnumSpec<'static, IrMainAxisAlignment> = ClosedEnumSpec {
    variants: &[
        ClosedEnumVariant {
            declaration_name: "START",
            value: IrMainAxisAlignment::Start,
        },
        ClosedEnumVariant {
            declaration_name: "CENTER",
            value: IrMainAxisAlignment::Center,
        },
        ClosedEnumVariant {
            declaration_name: "END",
            value: IrMainAxisAlignment::End,
        },
        ClosedEnumVariant {
            declaration_name: "SPACE_BETWEEN",
            value: IrMainAxisAlignment::SpaceBetween,
        },
        ClosedEnumVariant {
            declaration_name: "SPACE_AROUND",
            value: IrMainAxisAlignment::SpaceAround,
        },
        ClosedEnumVariant {
            declaration_name: "SPACE_EVENLY",
            value: IrMainAxisAlignment::SpaceEvenly,
        },
    ],
};

static STACKED_CROSS_AXIS_SPEC: ClosedEnumSpec<'static, IrCrossAxisAlignment> = ClosedEnumSpec {
    variants: &[
        ClosedEnumVariant {
            declaration_name: "START",
            value: IrCrossAxisAlignment::Start,
        },
        ClosedEnumVariant {
            declaration_name: "CENTER",
            value: IrCrossAxisAlignment::Center,
        },
        ClosedEnumVariant {
            declaration_name: "END",
            value: IrCrossAxisAlignment::End,
        },
        ClosedEnumVariant {
            declaration_name: "STRETCH",
            value: IrCrossAxisAlignment::Stretch,
        },
    ],
};

static CONTAINER_ALIGNMENT_SPEC: ClosedEnumSpec<'static, IrContainerAlignment> = ClosedEnumSpec {
    variants: &[
        ClosedEnumVariant {
            declaration_name: "START",
            value: IrContainerAlignment::Start,
        },
        ClosedEnumVariant {
            declaration_name: "CENTER",
            value: IrContainerAlignment::Center,
        },
        ClosedEnumVariant {
            declaration_name: "END",
            value: IrContainerAlignment::End,
        },
    ],
};

/// A source-independent conversion failure.
///
/// Callers add the parameter/builtin name and the reliable call or argument
/// span when turning this into a user-facing diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConversionError {
    InvalidText { target: ConversionTarget },
    UnsupportedValue { target: ConversionTarget },
}

/// The complete evaluator-private provenance carried by a failed conversion.
///
/// Conversion itself remains source-independent. This representation is
/// created at the target boundary and is consumed only when the evaluator
/// emits a diagnostic, so builtin adapters cannot accidentally replace the
/// typed reason with a call-level string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConversionFailure {
    pub(crate) error: ConversionError,
    pub(crate) candidate_span: Option<SourceSpan>,
    pub(crate) parameter_name: Option<String>,
    pub(crate) parameter_span: Option<SourceSpan>,
    pub(crate) call_span: SourceSpan,
}

impl ConversionFailure {
    pub(crate) fn new(
        error: ConversionError,
        candidate_span: Option<SourceSpan>,
        parameter_name: Option<impl Into<String>>,
        parameter_span: Option<SourceSpan>,
        call_span: SourceSpan,
    ) -> Self {
        Self {
            error,
            candidate_span,
            parameter_name: parameter_name.map(Into::into),
            parameter_span,
            call_span,
        }
    }
}

/// A context-sensitive conversion result. `Value` is already typed or
/// structurally preserved. `RawMarkdown` is an explicit request for the
/// evaluator to parse the original dynamic text in the selected target
/// context; the conversion module does not perform that evaluation itself.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum TargetValue {
    Value(IrValue),
    RawMarkdown {
        target: RawMarkdownTarget,
        text: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RawMarkdownTarget {
    Inline,
    Block,
    Iterable,
    Dictionary,
    Callable,
}

/// Applies the one engine-owned target conversion policy used by value
/// consumers. The returned raw requests correspond to the pinned
/// `DynamicValueConverter` branches and preserve the value's dynamic origin.
pub(crate) fn convert_target_with_origin(
    argument: &RuntimeValue,
    target: ConversionTarget,
    span: SourceSpan,
) -> Result<TargetValue, ConversionError> {
    match target {
        ConversionTarget::Dynamic => Ok(TargetValue::Value(argument.to_ir_value())),
        ConversionTarget::Number => {
            convert_scalar_with_origin(argument, ScalarTarget::Number).map(scalar_value_to_ir)
        }
        ConversionTarget::Integer => convert_integer_with_origin(argument)
            .map(|value| TargetValue::Value(IrValue::Number(f64::from(value)))),
        ConversionTarget::Boolean => {
            convert_scalar_with_origin(argument, ScalarTarget::Boolean).map(scalar_value_to_ir)
        }
        ConversionTarget::String => {
            convert_scalar_with_origin(argument, ScalarTarget::String).map(scalar_value_to_ir)
        }
        ConversionTarget::Range => convert_range_with_origin(argument, span)
            .map(|value| TargetValue::Value(IrValue::Range(value))),
        ConversionTarget::Size => {
            convert_domain_with_origin(argument, DomainTarget::Size).map(domain_value_to_ir)
        }
        ConversionTarget::Color => {
            convert_domain_with_origin(argument, DomainTarget::Color).map(domain_value_to_ir)
        }
        ConversionTarget::Enum => Err(ConversionError::UnsupportedValue {
            target: ConversionTarget::Enum,
        }),
        ConversionTarget::InlineContent => {
            convert_content_target(argument, RawMarkdownTarget::Inline, span, false)
        }
        ConversionTarget::BlockContent => {
            convert_content_target(argument, RawMarkdownTarget::Block, span, true)
        }
        ConversionTarget::Node => match &argument.kind {
            RuntimeValueKind::Component(component) => {
                Ok(TargetValue::Value(IrValue::Component(component.clone())))
            }
            _ => Err(ConversionError::UnsupportedValue {
                target: ConversionTarget::Node,
            }),
        },
        ConversionTarget::Iterable => convert_structured_target(
            argument,
            RawMarkdownTarget::Iterable,
            &[
                IrValueKind::Collection,
                IrValueKind::Pair,
                IrValueKind::Dictionary,
                IrValueKind::Range,
            ],
        ),
        ConversionTarget::Dictionary => convert_structured_target(
            argument,
            RawMarkdownTarget::Dictionary,
            &[IrValueKind::Dictionary],
        ),
        ConversionTarget::Callable => match &argument.kind {
            RuntimeValueKind::Callable(value) => {
                Ok(TargetValue::Value(IrValue::Callable(value.to_ir())))
            }
            RuntimeValueKind::InlineBody(body) => Ok(TargetValue::Value(IrValue::Callable(
                body.clone().into_callable().into_ir(),
            ))),
            _ => match argument.dynamic_text() {
                Some(value) => Ok(TargetValue::RawMarkdown {
                    target: RawMarkdownTarget::Callable,
                    text: value.to_owned(),
                }),
                None => Err(ConversionError::UnsupportedValue {
                    target: ConversionTarget::Callable,
                }),
            },
        },
    }
}

fn scalar_value_to_ir(value: ScalarValue) -> TargetValue {
    match value {
        ScalarValue::Number(value) => TargetValue::Value(IrValue::Number(value)),
        ScalarValue::Boolean(value) => TargetValue::Value(IrValue::Boolean(value)),
        ScalarValue::String(value) => TargetValue::Value(IrValue::String(value)),
    }
}

fn domain_value_to_ir(value: DomainValue) -> TargetValue {
    match value {
        DomainValue::Size(value) => TargetValue::Value(IrValue::Size(value)),
        DomainValue::Color(value) => TargetValue::Value(IrValue::Color(value)),
        DomainValue::Enum(value) => TargetValue::Value(IrValue::Enum(value)),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IrValueKind {
    Collection,
    Pair,
    Dictionary,
    Range,
}

fn convert_content_target(
    argument: &RuntimeValue,
    raw_target: RawMarkdownTarget,
    span: SourceSpan,
    block_target: bool,
) -> Result<TargetValue, ConversionError> {
    match &argument.kind {
        // MarkdownContentValue adapts to InlineMarkdownContentValue in the
        // pinned value model. The IR keeps one structured content carrier, so
        // both content targets preserve that carrier without flattening it.
        RuntimeValueKind::Content(nodes) => Ok(TargetValue::Value(IrValue::Content(nodes.clone()))),
        // An inline contextual body is not a block body. It remains inline
        // content until the selected target asks for it.
        RuntimeValueKind::InlineBody(body) if !block_target => {
            Ok(TargetValue::Value(IrValue::Content(body.content.clone())))
        }
        RuntimeValueKind::String(value)
            if block_target || argument.origin == ValueOrigin::Dynamic =>
        {
            Ok(TargetValue::RawMarkdown {
                target: raw_target,
                text: value.clone(),
            })
        }
        RuntimeValueKind::Identifier(value) if argument.origin == ValueOrigin::Dynamic => {
            Ok(TargetValue::RawMarkdown {
                target: raw_target,
                text: value.clone(),
            })
        }
        // The block Markdown target renders scalar results as literal text;
        // it does not feed their textual representation back through the
        // Markdown parser. This keeps already-evaluated values typed while
        // matching the target's observable paragraph materialization.
        RuntimeValueKind::Number(value) if block_target => {
            Ok(literal_content_value(number_to_text(*value), span))
        }
        RuntimeValueKind::Boolean(value) if block_target => {
            Ok(literal_content_value(value.to_string(), span))
        }
        // A non-null MarkdownContent target observes None as the literal
        // value name. Nullable consumers must intercept None before selecting
        // this target; `.container body:{.none}` is one such boundary.
        RuntimeValueKind::None if block_target => {
            Ok(literal_content_value("None".to_string(), span))
        }
        RuntimeValueKind::String(value) if !block_target => {
            Ok(TargetValue::Value(IrValue::Content(vec![
                IrNode::Paragraph {
                    content: vec![IrInline::Text {
                        content: value.clone(),
                        span,
                    }],
                    span,
                },
            ])))
        }
        _ => Err(ConversionError::UnsupportedValue {
            target: if block_target {
                ConversionTarget::BlockContent
            } else {
                ConversionTarget::InlineContent
            },
        }),
    }
}

fn literal_content_value(text: String, span: SourceSpan) -> TargetValue {
    TargetValue::Value(IrValue::Content(vec![IrNode::Paragraph {
        content: vec![IrInline::Text {
            content: text,
            span,
        }],
        span,
    }]))
}

fn convert_structured_target(
    argument: &RuntimeValue,
    raw_target: RawMarkdownTarget,
    accepted: &[IrValueKind],
) -> Result<TargetValue, ConversionError> {
    let kind = match &argument.kind {
        RuntimeValueKind::Collection(_) => IrValueKind::Collection,
        RuntimeValueKind::Pair(_) => IrValueKind::Pair,
        RuntimeValueKind::Dictionary(_) => IrValueKind::Dictionary,
        RuntimeValueKind::Range(_) => IrValueKind::Range,
        // A parsed Markdown list is the frontend representation of the
        // upstream dynamic iterable adapter. Keep it structured here so the
        // evaluator can materialize its elements without reparsing or
        // stringifying the already parsed list.
        RuntimeValueKind::Content(nodes) if raw_target == RawMarkdownTarget::Iterable => {
            return Ok(TargetValue::Value(IrValue::Content(nodes.clone())));
        }
        _ => {
            if let Some(value) = argument.dynamic_text() {
                return Ok(TargetValue::RawMarkdown {
                    target: raw_target,
                    text: value.to_owned(),
                });
            }
            return Err(ConversionError::UnsupportedValue {
                target: match raw_target {
                    RawMarkdownTarget::Dictionary => ConversionTarget::Dictionary,
                    _ => ConversionTarget::Iterable,
                },
            });
        }
    };
    if !accepted.contains(&kind) {
        return Err(ConversionError::UnsupportedValue {
            target: match raw_target {
                RawMarkdownTarget::Dictionary => ConversionTarget::Dictionary,
                _ => ConversionTarget::Iterable,
            },
        });
    }
    Ok(TargetValue::Value(argument.to_ir_value()))
}

/// Converts a typed evaluator value to one of the supported scalar targets.
///
/// Text parsing intentionally follows the reviewed v2.5.1 `ValueFactory`
/// order: integer parsing is attempted before floating-point parsing, and no
/// whitespace normalization or truthiness coercion is added.
#[cfg(test)]
pub(crate) fn convert_scalar(
    value: &IrValue,
    target: ScalarTarget,
) -> Result<ScalarValue, ConversionError> {
    convert_scalar_with_origin(&RuntimeValue::dynamic_value(value.clone()), target)
}

/// Converts an invocation argument using upstream's DynamicValue gate.
///
/// Text parsing for Number and Boolean is available only for a Dynamic
/// argument. Static StringValue-shaped values remain String values and do not
/// acquire unrelated numeric, boolean, or iterable meaning.
pub(crate) fn convert_scalar_with_origin(
    argument: &RuntimeValue,
    target: ScalarTarget,
) -> Result<ScalarValue, ConversionError> {
    match target {
        ScalarTarget::Number => {
            if let RuntimeValueKind::Number(value) = &argument.kind {
                return Ok(ScalarValue::Number(*value));
            }
            let Some(value) = argument.dynamic_text() else {
                return Err(ConversionError::UnsupportedValue {
                    target: target.into(),
                });
            };
            parse_number(value)
                .map(ScalarValue::Number)
                .ok_or(ConversionError::InvalidText {
                    target: target.into(),
                })
        }
        ScalarTarget::Boolean => {
            if let RuntimeValueKind::Boolean(value) = &argument.kind {
                return Ok(ScalarValue::Boolean(*value));
            }
            let Some(value) = argument.dynamic_text() else {
                return Err(ConversionError::UnsupportedValue {
                    target: target.into(),
                });
            };
            parse_boolean(value)
                .map(ScalarValue::Boolean)
                .ok_or(ConversionError::InvalidText {
                    target: target.into(),
                })
        }
        ScalarTarget::String => {
            if let Some(value) = argument.text_identity() {
                return Ok(ScalarValue::String(value.to_owned()));
            }
            match &argument.kind {
                RuntimeValueKind::Number(value) => Ok(ScalarValue::String(number_to_text(*value))),
                RuntimeValueKind::Boolean(value) => Ok(ScalarValue::String(value.to_string())),
                RuntimeValueKind::Range(value) => Ok(ScalarValue::String(range_to_text(value))),
                _ => Err(ConversionError::UnsupportedValue {
                    target: target.into(),
                }),
            }
        }
    }
}

/// Converts an existing or textual range while preserving the source span of
/// an existing typed range or attaching the caller's reliable argument span
/// to a newly parsed textual range.
pub(crate) fn convert_range_with_origin(
    argument: &RuntimeValue,
    span: SourceSpan,
) -> Result<IrRange, ConversionError> {
    if let RuntimeValueKind::Range(range) = &argument.kind {
        return Ok(range.clone());
    }
    let Some(value) = argument.dynamic_text() else {
        return Err(ConversionError::UnsupportedValue {
            target: ConversionTarget::Range,
        });
    };
    parse_range(value, span).ok_or(ConversionError::InvalidText {
        target: ConversionTarget::Range,
    })
}

#[cfg(test)]
pub(crate) fn convert_range(value: &IrValue, span: SourceSpan) -> Result<IrRange, ConversionError> {
    convert_range_with_origin(&RuntimeValue::dynamic_value(value.clone()), span)
}

/// Converts a public Color argument while preserving the dynamic-value
/// boundary used by Quarkdown presentation primitives.
///
/// Dynamic calls such as `rgba(...)` may evaluate to one plain content
/// paragraph before target conversion. Only that plain Text-only shape is
/// projected back to dynamic scalar text; static/rich content remains closed.
pub(crate) fn convert_color_with_origin(
    argument: &RuntimeValue,
) -> Result<IrColor, ConversionError> {
    let convert = |candidate: &RuntimeValue| match convert_domain_with_origin(
        candidate,
        DomainTarget::Color,
    )? {
        DomainValue::Color(color) => Ok(color),
        _ => Err(ConversionError::UnsupportedValue {
            target: ConversionTarget::Color,
        }),
    };

    convert(argument).or_else(|error| {
        if argument.origin != ValueOrigin::Dynamic {
            return Err(error);
        }
        let RuntimeValueKind::Content(nodes) = &argument.kind else {
            return Err(error);
        };
        let Some(text) = plain_dynamic_color_content(nodes) else {
            return Err(error);
        };
        convert(&RuntimeValue::dynamic_value(IrValue::String(text)))
    })
}

fn plain_dynamic_color_content(nodes: &[IrNode]) -> Option<String> {
    let mut text = String::new();
    for node in nodes {
        let IrNode::Paragraph { content, .. } = node else {
            return None;
        };
        for inline in content {
            let IrInline::Text { content, .. } = inline else {
                return None;
            };
            text.push_str(content);
        }
    }
    Some(text)
}

/// Converts an invocation value into a typed, backend-neutral domain value.
///
/// This is deliberately not a generalized scalar coercion engine. Existing
/// typed domain values retain their domain, while textual parsing is gated by
/// the invocation origin exactly like the reviewed DynamicValue boundary.
pub(crate) fn convert_domain_with_origin(
    argument: &RuntimeValue,
    target: DomainTarget,
) -> Result<DomainValue, ConversionError> {
    match target {
        DomainTarget::Size => {
            match &argument.kind {
                RuntimeValueKind::Size(value) => Ok(DomainValue::Size(value.clone())),
                // ValueFactory.size accepts a dynamic numeric value through its
                // textual representation. Non-finite values still fail the
                // decimal-only parser below.
                RuntimeValueKind::Number(value) => parse_size(&number_to_text(*value))
                    .map(DomainValue::Size)
                    .ok_or(ConversionError::InvalidText {
                        target: ConversionTarget::Size,
                    }),
                RuntimeValueKind::String(value) | RuntimeValueKind::Identifier(value)
                    if argument.origin == ValueOrigin::Dynamic =>
                {
                    parse_size(value)
                        .map(DomainValue::Size)
                        .ok_or(ConversionError::InvalidText {
                            target: ConversionTarget::Size,
                        })
                }
                _ => Err(ConversionError::UnsupportedValue {
                    target: ConversionTarget::Size,
                }),
            }
        }
        DomainTarget::Color => match &argument.kind {
            RuntimeValueKind::Color(value) => Ok(DomainValue::Color(value.clone())),
            RuntimeValueKind::String(value) | RuntimeValueKind::Identifier(value)
                if argument.origin == ValueOrigin::Dynamic =>
            {
                parse_color(value)
                    .map(DomainValue::Color)
                    .ok_or(ConversionError::InvalidText {
                        target: ConversionTarget::Color,
                    })
            }
            _ => Err(ConversionError::UnsupportedValue {
                target: ConversionTarget::Color,
            }),
        },
        DomainTarget::ClosedEnum(enum_target) => match enum_target {
            ClosedEnumTarget::DocumentType => match &argument.kind {
                RuntimeValueKind::Enum(IrEnumValue::DocumentType(value)) => {
                    Ok(DomainValue::Enum(IrEnumValue::DocumentType(*value)))
                }
                RuntimeValueKind::String(value) | RuntimeValueKind::Identifier(value)
                    if argument.origin == ValueOrigin::Dynamic =>
                {
                    parse_document_type(value)
                        .map(|value| DomainValue::Enum(IrEnumValue::DocumentType(value)))
                        .ok_or(ConversionError::InvalidText {
                            target: ConversionTarget::Enum,
                        })
                }
                _ => Err(ConversionError::UnsupportedValue {
                    target: ConversionTarget::Enum,
                }),
            },
            ClosedEnumTarget::PageSizeFormat => match &argument.kind {
                RuntimeValueKind::Enum(IrEnumValue::PageSizeFormat(value)) => {
                    Ok(DomainValue::Enum(IrEnumValue::PageSizeFormat(*value)))
                }
                RuntimeValueKind::String(value) | RuntimeValueKind::Identifier(value)
                    if argument.origin == ValueOrigin::Dynamic =>
                {
                    PAGE_SIZE_FORMAT_SPEC
                        .value_for(value)
                        .map(|value| DomainValue::Enum(IrEnumValue::PageSizeFormat(value)))
                        .ok_or(ConversionError::InvalidText {
                            target: ConversionTarget::Enum,
                        })
                }
                _ => Err(ConversionError::UnsupportedValue {
                    target: ConversionTarget::Enum,
                }),
            },
            ClosedEnumTarget::PageOrientation => match &argument.kind {
                RuntimeValueKind::Enum(IrEnumValue::PageOrientation(value)) => {
                    Ok(DomainValue::Enum(IrEnumValue::PageOrientation(*value)))
                }
                RuntimeValueKind::String(value) | RuntimeValueKind::Identifier(value)
                    if argument.origin == ValueOrigin::Dynamic =>
                {
                    PAGE_ORIENTATION_SPEC
                        .value_for(value)
                        .map(|value| DomainValue::Enum(IrEnumValue::PageOrientation(value)))
                        .ok_or(ConversionError::InvalidText {
                            target: ConversionTarget::Enum,
                        })
                }
                _ => Err(ConversionError::UnsupportedValue {
                    target: ConversionTarget::Enum,
                }),
            },
            ClosedEnumTarget::PageSide => match &argument.kind {
                RuntimeValueKind::Enum(IrEnumValue::PageSide(value)) => {
                    Ok(DomainValue::Enum(IrEnumValue::PageSide(*value)))
                }
                RuntimeValueKind::String(value) | RuntimeValueKind::Identifier(value)
                    if argument.origin == ValueOrigin::Dynamic =>
                {
                    PAGE_SIDE_SPEC
                        .value_for(value)
                        .map(|value| DomainValue::Enum(IrEnumValue::PageSide(value)))
                        .ok_or(ConversionError::InvalidText {
                            target: ConversionTarget::Enum,
                        })
                }
                _ => Err(ConversionError::UnsupportedValue {
                    target: ConversionTarget::Enum,
                }),
            },
            ClosedEnumTarget::PageMarginPosition => match &argument.kind {
                RuntimeValueKind::Enum(IrEnumValue::PageMarginPosition(value)) => {
                    Ok(DomainValue::Enum(IrEnumValue::PageMarginPosition(*value)))
                }
                RuntimeValueKind::String(value) | RuntimeValueKind::Identifier(value)
                    if argument.origin == ValueOrigin::Dynamic =>
                {
                    PAGE_MARGIN_POSITION_SPEC
                        .value_for(value)
                        .map(|value| DomainValue::Enum(IrEnumValue::PageMarginPosition(value)))
                        .ok_or(ConversionError::InvalidText {
                            target: ConversionTarget::Enum,
                        })
                }
                _ => Err(ConversionError::UnsupportedValue {
                    target: ConversionTarget::Enum,
                }),
            },
            ClosedEnumTarget::CaptionPosition => match &argument.kind {
                RuntimeValueKind::Enum(IrEnumValue::CaptionPosition(value)) => {
                    Ok(DomainValue::Enum(IrEnumValue::CaptionPosition(*value)))
                }
                RuntimeValueKind::String(value) | RuntimeValueKind::Identifier(value)
                    if argument.origin == ValueOrigin::Dynamic =>
                {
                    CAPTION_POSITION_SPEC
                        .value_for(value)
                        .map(|value| DomainValue::Enum(IrEnumValue::CaptionPosition(value)))
                        .ok_or(ConversionError::InvalidText {
                            target: ConversionTarget::Enum,
                        })
                }
                _ => Err(ConversionError::UnsupportedValue {
                    target: ConversionTarget::Enum,
                }),
            },
            ClosedEnumTarget::StackedMainAxisAlignment => match &argument.kind {
                RuntimeValueKind::Enum(IrEnumValue::StackedMainAxisAlignment(value)) => Ok(
                    DomainValue::Enum(IrEnumValue::StackedMainAxisAlignment(*value)),
                ),
                RuntimeValueKind::String(value) | RuntimeValueKind::Identifier(value)
                    if argument.origin == ValueOrigin::Dynamic =>
                {
                    STACKED_MAIN_AXIS_SPEC
                        .value_for(value)
                        .map(|value| {
                            DomainValue::Enum(IrEnumValue::StackedMainAxisAlignment(value))
                        })
                        .ok_or(ConversionError::InvalidText {
                            target: ConversionTarget::Enum,
                        })
                }
                _ => Err(ConversionError::UnsupportedValue {
                    target: ConversionTarget::Enum,
                }),
            },
            ClosedEnumTarget::StackedCrossAxisAlignment => match &argument.kind {
                RuntimeValueKind::Enum(IrEnumValue::StackedCrossAxisAlignment(value)) => Ok(
                    DomainValue::Enum(IrEnumValue::StackedCrossAxisAlignment(*value)),
                ),
                RuntimeValueKind::String(value) | RuntimeValueKind::Identifier(value)
                    if argument.origin == ValueOrigin::Dynamic =>
                {
                    STACKED_CROSS_AXIS_SPEC
                        .value_for(value)
                        .map(|value| {
                            DomainValue::Enum(IrEnumValue::StackedCrossAxisAlignment(value))
                        })
                        .ok_or(ConversionError::InvalidText {
                            target: ConversionTarget::Enum,
                        })
                }
                _ => Err(ConversionError::UnsupportedValue {
                    target: ConversionTarget::Enum,
                }),
            },
            ClosedEnumTarget::ContainerAlignment => match &argument.kind {
                RuntimeValueKind::Enum(IrEnumValue::ContainerAlignment(value)) => {
                    Ok(DomainValue::Enum(IrEnumValue::ContainerAlignment(*value)))
                }
                RuntimeValueKind::String(value) | RuntimeValueKind::Identifier(value)
                    if argument.origin == ValueOrigin::Dynamic =>
                {
                    CONTAINER_ALIGNMENT_SPEC
                        .value_for(value)
                        .map(|value| DomainValue::Enum(IrEnumValue::ContainerAlignment(value)))
                        .ok_or(ConversionError::InvalidText {
                            target: ConversionTarget::Enum,
                        })
                }
                _ => Err(ConversionError::UnsupportedValue {
                    target: ConversionTarget::Enum,
                }),
            },
            ClosedEnumTarget::BoxType => match &argument.kind {
                RuntimeValueKind::Enum(IrEnumValue::BoxType(value)) => {
                    Ok(DomainValue::Enum(IrEnumValue::BoxType(*value)))
                }
                RuntimeValueKind::String(value) | RuntimeValueKind::Identifier(value)
                    if argument.origin == ValueOrigin::Dynamic =>
                {
                    BOX_TYPE_SPEC
                        .value_for(value)
                        .map(|value| DomainValue::Enum(IrEnumValue::BoxType(value)))
                        .ok_or(ConversionError::InvalidText {
                            target: ConversionTarget::Enum,
                        })
                }
                _ => Err(ConversionError::UnsupportedValue {
                    target: ConversionTarget::Enum,
                }),
            },
            ClosedEnumTarget::ClipShape => match &argument.kind {
                RuntimeValueKind::Enum(IrEnumValue::ClipShape(value)) => {
                    Ok(DomainValue::Enum(IrEnumValue::ClipShape(*value)))
                }
                RuntimeValueKind::String(value) | RuntimeValueKind::Identifier(value)
                    if argument.origin == ValueOrigin::Dynamic =>
                {
                    CLIP_SHAPE_SPEC
                        .value_for(value)
                        .map(|value| DomainValue::Enum(IrEnumValue::ClipShape(value)))
                        .ok_or(ConversionError::InvalidText {
                            target: ConversionTarget::Enum,
                        })
                }
                _ => Err(ConversionError::UnsupportedValue {
                    target: ConversionTarget::Enum,
                }),
            },
            ClosedEnumTarget::SlideTransitionStyle => match &argument.kind {
                RuntimeValueKind::Enum(IrEnumValue::SlideTransitionStyle(value)) => {
                    Ok(DomainValue::Enum(IrEnumValue::SlideTransitionStyle(*value)))
                }
                RuntimeValueKind::String(value) | RuntimeValueKind::Identifier(value)
                    if argument.origin == ValueOrigin::Dynamic =>
                {
                    SLIDE_TRANSITION_STYLE_SPEC
                        .value_for(value)
                        .map(|value| DomainValue::Enum(IrEnumValue::SlideTransitionStyle(value)))
                        .ok_or(ConversionError::InvalidText {
                            target: ConversionTarget::Enum,
                        })
                }
                _ => Err(ConversionError::UnsupportedValue {
                    target: ConversionTarget::Enum,
                }),
            },
            ClosedEnumTarget::SlideTransitionSpeed => match &argument.kind {
                RuntimeValueKind::Enum(IrEnumValue::SlideTransitionSpeed(value)) => {
                    Ok(DomainValue::Enum(IrEnumValue::SlideTransitionSpeed(*value)))
                }
                RuntimeValueKind::String(value) | RuntimeValueKind::Identifier(value)
                    if argument.origin == ValueOrigin::Dynamic =>
                {
                    SLIDE_TRANSITION_SPEED_SPEC
                        .value_for(value)
                        .map(|value| DomainValue::Enum(IrEnumValue::SlideTransitionSpeed(value)))
                        .ok_or(ConversionError::InvalidText {
                            target: ConversionTarget::Enum,
                        })
                }
                _ => Err(ConversionError::UnsupportedValue {
                    target: ConversionTarget::Enum,
                }),
            },
            ClosedEnumTarget::FragmentBehavior => match &argument.kind {
                RuntimeValueKind::Enum(IrEnumValue::FragmentBehavior(value)) => {
                    Ok(DomainValue::Enum(IrEnumValue::FragmentBehavior(*value)))
                }
                RuntimeValueKind::String(value) | RuntimeValueKind::Identifier(value)
                    if argument.origin == ValueOrigin::Dynamic =>
                {
                    FRAGMENT_BEHAVIOR_SPEC
                        .value_for(value)
                        .map(|value| DomainValue::Enum(IrEnumValue::FragmentBehavior(value)))
                        .ok_or(ConversionError::InvalidText {
                            target: ConversionTarget::Enum,
                        })
                }
                _ => Err(ConversionError::UnsupportedValue {
                    target: ConversionTarget::Enum,
                }),
            },
        },
    }
}

/// Converts the closed relative text-size domain while preserving the
/// invocation-origin boundary used by typed component arguments.
///
/// `normal` and explicit `None` are semantic identity. Textual enum names are
/// accepted only from a dynamic invocation value; a static String produced by
/// another call cannot silently acquire enum meaning.
pub(crate) fn convert_text_size_with_origin(
    argument: &RuntimeValue,
) -> Result<Option<IrTextSize>, ConversionError> {
    if argument.is_none() {
        return Ok(None);
    }
    let Some(value) = argument.dynamic_text() else {
        return Err(ConversionError::UnsupportedValue {
            target: ConversionTarget::Enum,
        });
    };
    parse_text_size(value).ok_or(ConversionError::InvalidText {
        target: ConversionTarget::Enum,
    })
}

/// Parses the single reviewed relative text-size table shared by `.text` and
/// component-local font-size consumers. The outer `Option` represents parse
/// success; the inner `Option` preserves `normal` as semantic identity.
pub(crate) fn parse_text_size(value: &str) -> Option<Option<IrTextSize>> {
    match value.to_ascii_lowercase().as_str() {
        "tiny" => Some(Some(IrTextSize::Tiny)),
        "small" => Some(Some(IrTextSize::Small)),
        "normal" => Some(None),
        "medium" => Some(Some(IrTextSize::Medium)),
        "large" => Some(Some(IrTextSize::Large)),
        "larger" => Some(Some(IrTextSize::Larger)),
        "huge" => Some(Some(IrTextSize::Huge)),
        _ => None,
    }
}

/// Converts the closed font-weight domain while preserving invocation origin.
///
/// `normal` and explicit `None` are semantic identity. Textual enum names are
/// accepted only from a dynamic invocation value.
pub(crate) fn convert_text_weight_with_origin(
    argument: &RuntimeValue,
) -> Result<Option<IrTextWeight>, ConversionError> {
    if argument.is_none() {
        return Ok(None);
    }
    let Some(value) = argument.dynamic_text() else {
        return Err(ConversionError::UnsupportedValue {
            target: ConversionTarget::Enum,
        });
    };
    parse_text_weight(value).ok_or(ConversionError::InvalidText {
        target: ConversionTarget::Enum,
    })
}

/// Parses the reviewed font-weight table shared by `.text` and component-local
/// font-weight consumers. The outer `Option` represents parse success; the
/// inner `Option` preserves `normal` as semantic identity.
pub(crate) fn parse_text_weight(value: &str) -> Option<Option<IrTextWeight>> {
    match value.to_ascii_lowercase().as_str() {
        "normal" => Some(None),
        "bold" => Some(Some(IrTextWeight::Bold)),
        _ => None,
    }
}

/// Converts the closed font-style domain while preserving invocation origin.
///
/// `normal` and explicit `None` are semantic identity. Textual enum names are
/// accepted only from a dynamic invocation value.
pub(crate) fn convert_text_style_with_origin(
    argument: &RuntimeValue,
) -> Result<Option<IrTextStyle>, ConversionError> {
    if argument.is_none() {
        return Ok(None);
    }
    let Some(value) = argument.dynamic_text() else {
        return Err(ConversionError::UnsupportedValue {
            target: ConversionTarget::Enum,
        });
    };
    parse_text_style(value).ok_or(ConversionError::InvalidText {
        target: ConversionTarget::Enum,
    })
}

/// Parses the reviewed font-style table shared by `.text` and component-local
/// font-style consumers. The outer `Option` represents parse success; the
/// inner `Option` preserves `normal` as semantic identity.
pub(crate) fn parse_text_style(value: &str) -> Option<Option<IrTextStyle>> {
    match value.to_ascii_lowercase().as_str() {
        "normal" => Some(None),
        "italic" => Some(Some(IrTextStyle::Italic)),
        _ => None,
    }
}

/// Converts the closed text-decoration domain while preserving invocation
/// origin.
///
/// `none` and explicit `None` are semantic identity. Textual enum names are
/// accepted only from a dynamic invocation value.
pub(crate) fn convert_text_decoration_with_origin(
    argument: &RuntimeValue,
) -> Result<Option<IrTextDecoration>, ConversionError> {
    if argument.is_none() {
        return Ok(None);
    }
    let Some(value) = argument.dynamic_text() else {
        return Err(ConversionError::UnsupportedValue {
            target: ConversionTarget::Enum,
        });
    };
    parse_text_decoration(value).ok_or(ConversionError::InvalidText {
        target: ConversionTarget::Enum,
    })
}

/// Parses the reviewed text-decoration table shared by `.text` and
/// component-local text-decoration consumers. The outer `Option` represents
/// parse success; the inner `Option` preserves `none` as semantic identity.
pub(crate) fn parse_text_decoration(value: &str) -> Option<Option<IrTextDecoration>> {
    match value.to_ascii_lowercase().as_str() {
        "none" => Some(None),
        "underline" => Some(Some(IrTextDecoration::Underline)),
        "overline" => Some(Some(IrTextDecoration::Overline)),
        "underoverline" => Some(Some(IrTextDecoration::UnderOverline)),
        "strikethrough" => Some(Some(IrTextDecoration::Strikethrough)),
        "all" => Some(Some(IrTextDecoration::All)),
        _ => None,
    }
}

/// Converts the closed text-case domain while preserving invocation origin.
///
/// `none` and explicit `None` are semantic identity. Textual enum names are
/// accepted only from a dynamic invocation value.
pub(crate) fn convert_text_case_with_origin(
    argument: &RuntimeValue,
) -> Result<Option<IrTextCase>, ConversionError> {
    if argument.is_none() {
        return Ok(None);
    }
    let Some(value) = argument.dynamic_text() else {
        return Err(ConversionError::UnsupportedValue {
            target: ConversionTarget::Enum,
        });
    };
    parse_text_case(value).ok_or(ConversionError::InvalidText {
        target: ConversionTarget::Enum,
    })
}

/// Parses the reviewed text-case table shared by `.text` and component-local
/// text-case consumers. The outer `Option` represents parse success; the inner
/// `Option` preserves `none` as semantic identity.
pub(crate) fn parse_text_case(value: &str) -> Option<Option<IrTextCase>> {
    match value.to_ascii_lowercase().as_str() {
        "none" => Some(None),
        "uppercase" => Some(Some(IrTextCase::Uppercase)),
        "lowercase" => Some(Some(IrTextCase::Lowercase)),
        "capitalize" => Some(Some(IrTextCase::Capitalize)),
        _ => None,
    }
}

/// Converts the closed font-variant domain while preserving invocation origin.
///
/// `normal` and explicit `None` are semantic identity. Textual enum names are
/// accepted only from a dynamic invocation value.
pub(crate) fn convert_text_variant_with_origin(
    argument: &RuntimeValue,
) -> Result<Option<IrTextVariant>, ConversionError> {
    if argument.is_none() {
        return Ok(None);
    }
    let Some(value) = argument.dynamic_text() else {
        return Err(ConversionError::UnsupportedValue {
            target: ConversionTarget::Enum,
        });
    };
    parse_text_variant(value).ok_or(ConversionError::InvalidText {
        target: ConversionTarget::Enum,
    })
}

/// Parses the reviewed font-variant table shared by `.text` and
/// component-local font-variant consumers. The outer `Option` represents
/// parse success; the inner `Option` preserves `normal` as semantic identity.
pub(crate) fn parse_text_variant(value: &str) -> Option<Option<IrTextVariant>> {
    match value.to_ascii_lowercase().as_str() {
        "normal" => Some(None),
        "smallcaps" => Some(Some(IrTextVariant::SmallCaps)),
        _ => None,
    }
}

/// Converts the bounded document-global `.pageformat alignment` domain.
///
/// Dynamic text follows the same origin gate as the existing closed-enum
/// conversion boundary. A nested/static String does not silently acquire a
/// new domain identity.
pub(crate) fn convert_document_alignment_with_origin(
    argument: &RuntimeValue,
) -> Result<IrDocumentAlignment, ConversionError> {
    let Some(value) = argument.dynamic_text() else {
        return Err(ConversionError::UnsupportedValue {
            target: ConversionTarget::Enum,
        });
    };
    DOCUMENT_ALIGNMENT_SPEC
        .value_for(value)
        .ok_or(ConversionError::InvalidText {
            target: ConversionTarget::Enum,
        })
}

/// Converts the nullable four-value text-alignment domain used by
/// component-local style consumers. Explicit `None` is semantic identity.
pub(crate) fn convert_text_alignment_with_origin(
    argument: &RuntimeValue,
) -> Result<Option<IrDocumentAlignment>, ConversionError> {
    if argument.is_none() {
        return Ok(None);
    }
    convert_document_alignment_with_origin(argument).map(Some)
}

/// Converts a Quarkdown `Int` boundary without truncating fractional values.
/// Dynamic text follows the reviewed NumberValue parsing order, while static
/// strings remain strings and cannot acquire integer meaning.
pub(crate) fn convert_integer_with_origin(argument: &RuntimeValue) -> Result<i32, ConversionError> {
    let number = match &argument.kind {
        RuntimeValueKind::Number(value) => Some(*value),
        _ => argument.dynamic_text().and_then(parse_number),
    };
    let Some(number) = number else {
        return Err(ConversionError::UnsupportedValue {
            target: ConversionTarget::Integer,
        });
    };
    if !number.is_finite()
        || number.fract() != 0.0
        || number < f64::from(i32::MIN)
        || number > f64::from(i32::MAX)
    {
        return Err(ConversionError::InvalidText {
            target: ConversionTarget::Integer,
        });
    }
    Ok(number as i32)
}

fn parse_size(value: &str) -> Option<IrSize> {
    let unit_start = value
        .find(|character: char| character.is_ascii_alphabetic() || character == '%')
        .unwrap_or(value.len());
    let (number, raw_unit) = value.split_at(unit_start);
    let value = parse_decimal(number)?;
    let unit = match raw_unit {
        "" => IrSizeUnit::Px,
        unit if unit.eq_ignore_ascii_case("px") => IrSizeUnit::Px,
        unit if unit.eq_ignore_ascii_case("pt") => IrSizeUnit::Pt,
        unit if unit.eq_ignore_ascii_case("cm") => IrSizeUnit::Cm,
        unit if unit.eq_ignore_ascii_case("mm") => IrSizeUnit::Mm,
        unit if unit.eq_ignore_ascii_case("in") => IrSizeUnit::In,
        unit if unit.eq_ignore_ascii_case("em") => IrSizeUnit::Em,
        "%" => IrSizeUnit::Percent,
        _ => return None,
    };
    Some(IrSize { value, unit })
}

fn parse_decimal(value: &str) -> Option<f64> {
    let bytes = value.as_bytes();
    let mut index = usize::from(bytes.first() == Some(&b'-'));
    let integer_start = index;
    while bytes.get(index).is_some_and(u8::is_ascii_digit) {
        index += 1;
    }
    if index == integer_start {
        return None;
    }
    if bytes.get(index) == Some(&b'.') {
        index += 1;
        let fraction_start = index;
        while bytes.get(index).is_some_and(u8::is_ascii_digit) {
            index += 1;
        }
        if index == fraction_start {
            return None;
        }
    }
    (index == bytes.len())
        .then(|| value.parse::<f64>().ok())
        .flatten()
        .filter(|value| value.is_finite())
}

fn parse_color(value: &str) -> Option<IrColor> {
    parse_hex_color(value)
        .or_else(|| parse_rgb_color(value))
        .or_else(|| parse_rgba_color(value))
        .or_else(|| parse_hsv_hsl_color(value))
        .or_else(|| parse_named_color(value))
}

fn parse_hex_color(value: &str) -> Option<IrColor> {
    let digits = value.strip_prefix('#')?;
    match digits.len() {
        3 => {
            let red = hex_pair(digits.get(0..1)?, digits.get(0..1)?)?;
            let green = hex_pair(digits.get(1..2)?, digits.get(1..2)?)?;
            let blue = hex_pair(digits.get(2..3)?, digits.get(2..3)?)?;
            Some(IrColor {
                red,
                green,
                blue,
                alpha: 1.0,
            })
        }
        4 => {
            let red = hex_pair(digits.get(0..1)?, digits.get(0..1)?)?;
            let green = hex_pair(digits.get(1..2)?, digits.get(1..2)?)?;
            let blue = hex_pair(digits.get(2..3)?, digits.get(2..3)?)?;
            hex_digit(digits.as_bytes().get(3)?.to_ascii_lowercase())?;
            Some(IrColor {
                red,
                green,
                blue,
                alpha: 1.0,
            })
        }
        6 => Some(IrColor {
            red: hex_byte(digits.get(0..2)?)?,
            green: hex_byte(digits.get(2..4)?)?,
            blue: hex_byte(digits.get(4..6)?)?,
            alpha: 1.0,
        }),
        8 => {
            let red = hex_byte(digits.get(0..2)?)?;
            let green = hex_byte(digits.get(2..4)?)?;
            let blue = hex_byte(digits.get(4..6)?)?;
            hex_byte(digits.get(6..8)?)?;
            Some(IrColor {
                red,
                green,
                blue,
                alpha: 1.0,
            })
        }
        _ => None,
    }
}

fn hex_pair(first: &str, second: &str) -> Option<u8> {
    let first = first.as_bytes().first().copied()?.to_ascii_lowercase();
    let second = second.as_bytes().first().copied()?.to_ascii_lowercase();
    Some(hex_digit(first)? * 16 + hex_digit(second)?)
}

fn hex_byte(value: &str) -> Option<u8> {
    let bytes = value.as_bytes();
    Some(
        hex_digit(bytes.first().copied()?.to_ascii_lowercase())? * 16
            + hex_digit(bytes.get(1).copied()?.to_ascii_lowercase())?,
    )
}

fn hex_digit(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

fn parse_rgb_color(value: &str) -> Option<IrColor> {
    let rest = value.strip_prefix("rgb(")?;
    let (red, rest) = parse_color_channel(rest)?;
    let rest = optional_single_space(rest.strip_prefix(',')?);
    let (green, rest) = parse_color_channel(rest)?;
    let rest = optional_single_space(rest.strip_prefix(',')?);
    let (blue, rest) = parse_color_channel(rest)?;
    rest.strip_prefix(')')?;
    Some(IrColor {
        red,
        green,
        blue,
        alpha: 1.0,
    })
}

fn parse_rgba_color(value: &str) -> Option<IrColor> {
    let rest = value.strip_prefix("rgba(")?;
    let (red, rest) = parse_color_channel(rest)?;
    let rest = optional_single_space(rest.strip_prefix(',')?);
    let (green, rest) = parse_color_channel(rest)?;
    let rest = optional_single_space(rest.strip_prefix(',')?);
    let (blue, rest) = parse_color_channel(rest)?;
    let rest = optional_single_space(rest.strip_prefix(',')?);
    let close = rest.find(')')?;
    let alpha_text = rest.get(..close)?;
    if alpha_text.is_empty()
        || !alpha_text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte == b'.')
    {
        return None;
    }
    let alpha = alpha_text.parse::<f64>().ok()?;
    (alpha <= 1.0).then_some(IrColor {
        red,
        green,
        blue,
        alpha,
    })
}

fn optional_single_space(value: &str) -> &str {
    value.strip_prefix(' ').unwrap_or(value)
}

fn parse_color_channel(value: &str) -> Option<(u8, &str)> {
    let digits = value
        .as_bytes()
        .iter()
        .take(3)
        .take_while(|byte| byte.is_ascii_digit())
        .count();
    if digits == 0 || value.as_bytes().get(digits).is_some_and(u8::is_ascii_digit) {
        return None;
    }
    let channel = value.get(..digits)?.parse::<u16>().ok()?;
    (channel <= 255).then_some((channel as u8, value.get(digits..)?))
}

fn parse_hsv_hsl_color(value: &str) -> Option<IrColor> {
    let (kind, rest) = if let Some(rest) = value.strip_prefix("hsv(") {
        ('v', rest)
    } else {
        ('l', value.strip_prefix("hsl(")?)
    };
    let (hue, rest) = parse_hue_component(rest)?;
    let rest = optional_single_space(rest.strip_prefix(',')?);
    let (saturation, rest) = parse_percentage_component(rest)?;
    let rest = optional_single_space(rest.strip_prefix(',')?);
    let (lightness_or_value, rest) = parse_percentage_component(rest)?;
    rest.strip_prefix(')')?;
    let hue = (hue % 360) as f64 / 360.0;
    let saturation = saturation as f64 / 100.0;
    let lightness_or_value = lightness_or_value as f64 / 100.0;
    let (red, green, blue) = if kind == 'v' {
        hsv_to_rgb(hue, saturation, lightness_or_value)
    } else {
        hsl_to_rgb(hue, saturation, lightness_or_value)
    };
    Some(IrColor {
        red: (red * 255.0) as u8,
        green: (green * 255.0) as u8,
        blue: (blue * 255.0) as u8,
        alpha: 1.0,
    })
}

fn parse_hue_component(value: &str) -> Option<(u16, &str)> {
    let (hue, rest) = parse_unsigned_component(value)?;
    Some((hue, rest))
}

fn parse_percentage_component(value: &str) -> Option<(u16, &str)> {
    let (component, rest) = parse_unsigned_component(value)?;
    (component <= 100).then_some((component, rest))
}

fn parse_unsigned_component(value: &str) -> Option<(u16, &str)> {
    let digits = value
        .as_bytes()
        .iter()
        .take(3)
        .take_while(|byte| byte.is_ascii_digit())
        .count();
    if digits == 0 || value.as_bytes().get(digits).is_some_and(u8::is_ascii_digit) {
        return None;
    }
    Some((value.get(..digits)?.parse().ok()?, value.get(digits..)?))
}

fn hsv_to_rgb(hue: f64, saturation: f64, value: f64) -> (f64, f64, f64) {
    let h = hue * 6.0;
    let index = h.floor() as i32;
    let fraction = h - index as f64;
    let p = value * (1.0 - saturation);
    let q = value * (1.0 - fraction * saturation);
    let t = value * (1.0 - (1.0 - fraction) * saturation);
    match index.rem_euclid(6) {
        0 => (value, t, p),
        1 => (q, value, p),
        2 => (p, value, t),
        3 => (p, q, value),
        4 => (t, p, value),
        _ => (value, p, q),
    }
}

fn hsl_to_rgb(hue: f64, saturation: f64, lightness: f64) -> (f64, f64, f64) {
    if saturation == 0.0 {
        return (lightness, lightness, lightness);
    }
    let q = if lightness < 0.5 {
        lightness * (1.0 + saturation)
    } else {
        lightness + saturation - lightness * saturation
    };
    let p = 2.0 * lightness - q;
    (
        hsl_channel(p, q, hue + 1.0 / 3.0),
        hsl_channel(p, q, hue),
        hsl_channel(p, q, hue - 1.0 / 3.0),
    )
}

fn hsl_channel(p: f64, q: f64, mut hue: f64) -> f64 {
    if hue < 0.0 {
        hue += 1.0;
    }
    if hue > 1.0 {
        hue -= 1.0;
    }
    if hue < 1.0 / 6.0 {
        p + (q - p) * 6.0 * hue
    } else if hue < 1.0 / 2.0 {
        q
    } else if hue < 2.0 / 3.0 {
        p + (q - p) * (2.0 / 3.0 - hue) * 6.0
    } else {
        p
    }
}

fn parse_document_type(value: &str) -> Option<IrDocumentType> {
    DOCUMENT_TYPE_SPEC.value_for(value)
}

fn parse_named_color(value: &str) -> Option<IrColor> {
    let value = value.to_ascii_lowercase();
    match value.as_str() {
        "aliceblue" => Some(IrColor {
            red: 240,
            green: 248,
            blue: 255,
            alpha: 1.0,
        }),
        "antiquewhite" => Some(IrColor {
            red: 250,
            green: 235,
            blue: 215,
            alpha: 1.0,
        }),
        "aqua" => Some(IrColor {
            red: 0,
            green: 255,
            blue: 255,
            alpha: 1.0,
        }),
        "aquamarine" => Some(IrColor {
            red: 127,
            green: 255,
            blue: 212,
            alpha: 1.0,
        }),
        "azure" => Some(IrColor {
            red: 240,
            green: 255,
            blue: 255,
            alpha: 1.0,
        }),
        "beige" => Some(IrColor {
            red: 245,
            green: 245,
            blue: 220,
            alpha: 1.0,
        }),
        "bisque" => Some(IrColor {
            red: 255,
            green: 228,
            blue: 196,
            alpha: 1.0,
        }),
        "black" => Some(IrColor {
            red: 0,
            green: 0,
            blue: 0,
            alpha: 1.0,
        }),
        "blanchedalmond" => Some(IrColor {
            red: 255,
            green: 235,
            blue: 205,
            alpha: 1.0,
        }),
        "blue" => Some(IrColor {
            red: 0,
            green: 0,
            blue: 255,
            alpha: 1.0,
        }),
        "blueviolet" => Some(IrColor {
            red: 138,
            green: 43,
            blue: 226,
            alpha: 1.0,
        }),
        "brown" => Some(IrColor {
            red: 165,
            green: 42,
            blue: 42,
            alpha: 1.0,
        }),
        "burlywood" => Some(IrColor {
            red: 222,
            green: 184,
            blue: 135,
            alpha: 1.0,
        }),
        "cadetblue" => Some(IrColor {
            red: 95,
            green: 158,
            blue: 160,
            alpha: 1.0,
        }),
        "chartreuse" => Some(IrColor {
            red: 127,
            green: 255,
            blue: 0,
            alpha: 1.0,
        }),
        "chocolate" => Some(IrColor {
            red: 210,
            green: 105,
            blue: 30,
            alpha: 1.0,
        }),
        "coral" => Some(IrColor {
            red: 255,
            green: 127,
            blue: 80,
            alpha: 1.0,
        }),
        "cornflowerblue" => Some(IrColor {
            red: 100,
            green: 149,
            blue: 237,
            alpha: 1.0,
        }),
        "cornsilk" => Some(IrColor {
            red: 255,
            green: 248,
            blue: 220,
            alpha: 1.0,
        }),
        "crimson" => Some(IrColor {
            red: 220,
            green: 20,
            blue: 60,
            alpha: 1.0,
        }),
        "cyan" => Some(IrColor {
            red: 0,
            green: 255,
            blue: 255,
            alpha: 1.0,
        }),
        "darkblue" => Some(IrColor {
            red: 0,
            green: 0,
            blue: 139,
            alpha: 1.0,
        }),
        "darkcyan" => Some(IrColor {
            red: 0,
            green: 139,
            blue: 139,
            alpha: 1.0,
        }),
        "darkgoldenrod" => Some(IrColor {
            red: 184,
            green: 134,
            blue: 11,
            alpha: 1.0,
        }),
        "darkgray" | "darkgrey" => Some(IrColor {
            red: 169,
            green: 169,
            blue: 169,
            alpha: 1.0,
        }),
        "darkgreen" => Some(IrColor {
            red: 0,
            green: 100,
            blue: 0,
            alpha: 1.0,
        }),
        "darkkhaki" => Some(IrColor {
            red: 189,
            green: 183,
            blue: 107,
            alpha: 1.0,
        }),
        "darkmagenta" => Some(IrColor {
            red: 139,
            green: 0,
            blue: 139,
            alpha: 1.0,
        }),
        "darkolivegreen" => Some(IrColor {
            red: 85,
            green: 107,
            blue: 47,
            alpha: 1.0,
        }),
        "darkorange" => Some(IrColor {
            red: 255,
            green: 140,
            blue: 0,
            alpha: 1.0,
        }),
        "darkorchid" => Some(IrColor {
            red: 153,
            green: 50,
            blue: 204,
            alpha: 1.0,
        }),
        "darkred" => Some(IrColor {
            red: 139,
            green: 0,
            blue: 0,
            alpha: 1.0,
        }),
        "darksalmon" => Some(IrColor {
            red: 233,
            green: 150,
            blue: 122,
            alpha: 1.0,
        }),
        "darkseagreen" => Some(IrColor {
            red: 143,
            green: 188,
            blue: 143,
            alpha: 1.0,
        }),
        "darkslateblue" => Some(IrColor {
            red: 72,
            green: 61,
            blue: 139,
            alpha: 1.0,
        }),
        "darkslategray" | "darkslategrey" => Some(IrColor {
            red: 47,
            green: 79,
            blue: 79,
            alpha: 1.0,
        }),
        "darkturquoise" => Some(IrColor {
            red: 0,
            green: 206,
            blue: 209,
            alpha: 1.0,
        }),
        "darkviolet" => Some(IrColor {
            red: 148,
            green: 0,
            blue: 211,
            alpha: 1.0,
        }),
        "deeppink" => Some(IrColor {
            red: 255,
            green: 20,
            blue: 147,
            alpha: 1.0,
        }),
        "deepskyblue" => Some(IrColor {
            red: 0,
            green: 191,
            blue: 255,
            alpha: 1.0,
        }),
        "dimgray" | "dimgrey" => Some(IrColor {
            red: 105,
            green: 105,
            blue: 105,
            alpha: 1.0,
        }),
        "dodgerblue" => Some(IrColor {
            red: 30,
            green: 144,
            blue: 255,
            alpha: 1.0,
        }),
        "firebrick" => Some(IrColor {
            red: 178,
            green: 34,
            blue: 34,
            alpha: 1.0,
        }),
        "floralwhite" => Some(IrColor {
            red: 255,
            green: 250,
            blue: 240,
            alpha: 1.0,
        }),
        "forestgreen" => Some(IrColor {
            red: 34,
            green: 139,
            blue: 34,
            alpha: 1.0,
        }),
        "fuchsia" => Some(IrColor {
            red: 255,
            green: 0,
            blue: 255,
            alpha: 1.0,
        }),
        "gainsboro" => Some(IrColor {
            red: 220,
            green: 220,
            blue: 220,
            alpha: 1.0,
        }),
        "ghostwhite" => Some(IrColor {
            red: 248,
            green: 248,
            blue: 255,
            alpha: 1.0,
        }),
        "gold" => Some(IrColor {
            red: 255,
            green: 215,
            blue: 0,
            alpha: 1.0,
        }),
        "goldenrod" => Some(IrColor {
            red: 218,
            green: 165,
            blue: 32,
            alpha: 1.0,
        }),
        "gray" => Some(IrColor {
            red: 128,
            green: 128,
            blue: 128,
            alpha: 1.0,
        }),
        "green" => Some(IrColor {
            red: 0,
            green: 128,
            blue: 0,
            alpha: 1.0,
        }),
        "greenyellow" => Some(IrColor {
            red: 173,
            green: 255,
            blue: 47,
            alpha: 1.0,
        }),
        "grey" => Some(IrColor {
            red: 128,
            green: 128,
            blue: 128,
            alpha: 1.0,
        }),
        "honeydew" => Some(IrColor {
            red: 240,
            green: 255,
            blue: 240,
            alpha: 1.0,
        }),
        "hotpink" => Some(IrColor {
            red: 255,
            green: 105,
            blue: 180,
            alpha: 1.0,
        }),
        "indianred" => Some(IrColor {
            red: 205,
            green: 92,
            blue: 92,
            alpha: 1.0,
        }),
        "indigo" => Some(IrColor {
            red: 75,
            green: 0,
            blue: 130,
            alpha: 1.0,
        }),
        "ivory" => Some(IrColor {
            red: 255,
            green: 255,
            blue: 240,
            alpha: 1.0,
        }),
        "khaki" => Some(IrColor {
            red: 240,
            green: 230,
            blue: 140,
            alpha: 1.0,
        }),
        "lavender" => Some(IrColor {
            red: 230,
            green: 230,
            blue: 250,
            alpha: 1.0,
        }),
        "lavenderblush" => Some(IrColor {
            red: 255,
            green: 240,
            blue: 245,
            alpha: 1.0,
        }),
        "lawngreen" => Some(IrColor {
            red: 124,
            green: 252,
            blue: 0,
            alpha: 1.0,
        }),
        "lemonchiffon" => Some(IrColor {
            red: 255,
            green: 250,
            blue: 205,
            alpha: 1.0,
        }),
        "lightblue" => Some(IrColor {
            red: 173,
            green: 216,
            blue: 230,
            alpha: 1.0,
        }),
        "lightcoral" => Some(IrColor {
            red: 240,
            green: 128,
            blue: 128,
            alpha: 1.0,
        }),
        "lightcyan" => Some(IrColor {
            red: 224,
            green: 255,
            blue: 255,
            alpha: 1.0,
        }),
        "lightgoldenrodyellow" => Some(IrColor {
            red: 250,
            green: 250,
            blue: 210,
            alpha: 1.0,
        }),
        "lightgray" | "lightgrey" => Some(IrColor {
            red: 211,
            green: 211,
            blue: 211,
            alpha: 1.0,
        }),
        "lightgreen" => Some(IrColor {
            red: 144,
            green: 238,
            blue: 144,
            alpha: 1.0,
        }),
        "lightpink" => Some(IrColor {
            red: 255,
            green: 182,
            blue: 193,
            alpha: 1.0,
        }),
        "lightsalmon" => Some(IrColor {
            red: 255,
            green: 160,
            blue: 122,
            alpha: 1.0,
        }),
        "lightseagreen" => Some(IrColor {
            red: 32,
            green: 178,
            blue: 170,
            alpha: 1.0,
        }),
        "lightskyblue" => Some(IrColor {
            red: 135,
            green: 206,
            blue: 250,
            alpha: 1.0,
        }),
        "lightslategray" | "lightslategrey" => Some(IrColor {
            red: 119,
            green: 136,
            blue: 153,
            alpha: 1.0,
        }),
        "lightsteelblue" => Some(IrColor {
            red: 176,
            green: 196,
            blue: 222,
            alpha: 1.0,
        }),
        "lightyellow" => Some(IrColor {
            red: 255,
            green: 255,
            blue: 224,
            alpha: 1.0,
        }),
        "lime" => Some(IrColor {
            red: 0,
            green: 255,
            blue: 0,
            alpha: 1.0,
        }),
        "limegreen" => Some(IrColor {
            red: 50,
            green: 205,
            blue: 50,
            alpha: 1.0,
        }),
        "linen" => Some(IrColor {
            red: 250,
            green: 240,
            blue: 230,
            alpha: 1.0,
        }),
        "magenta" => Some(IrColor {
            red: 255,
            green: 0,
            blue: 255,
            alpha: 1.0,
        }),
        "maroon" => Some(IrColor {
            red: 128,
            green: 0,
            blue: 0,
            alpha: 1.0,
        }),
        "mediumaquamarine" => Some(IrColor {
            red: 102,
            green: 205,
            blue: 170,
            alpha: 1.0,
        }),
        "mediumblue" => Some(IrColor {
            red: 0,
            green: 0,
            blue: 205,
            alpha: 1.0,
        }),
        "mediumorchid" => Some(IrColor {
            red: 186,
            green: 85,
            blue: 211,
            alpha: 1.0,
        }),
        "mediumpurple" => Some(IrColor {
            red: 147,
            green: 112,
            blue: 219,
            alpha: 1.0,
        }),
        "mediumseagreen" => Some(IrColor {
            red: 60,
            green: 179,
            blue: 113,
            alpha: 1.0,
        }),
        "mediumslateblue" => Some(IrColor {
            red: 123,
            green: 104,
            blue: 238,
            alpha: 1.0,
        }),
        "mediumspringgreen" => Some(IrColor {
            red: 0,
            green: 250,
            blue: 154,
            alpha: 1.0,
        }),
        "mediumturquoise" => Some(IrColor {
            red: 72,
            green: 209,
            blue: 204,
            alpha: 1.0,
        }),
        "mediumvioletred" => Some(IrColor {
            red: 199,
            green: 21,
            blue: 133,
            alpha: 1.0,
        }),
        "midnightblue" => Some(IrColor {
            red: 25,
            green: 25,
            blue: 112,
            alpha: 1.0,
        }),
        "mintcream" => Some(IrColor {
            red: 245,
            green: 255,
            blue: 250,
            alpha: 1.0,
        }),
        "mistyrose" => Some(IrColor {
            red: 255,
            green: 228,
            blue: 225,
            alpha: 1.0,
        }),
        "moccasin" => Some(IrColor {
            red: 255,
            green: 228,
            blue: 181,
            alpha: 1.0,
        }),
        "navajowhite" => Some(IrColor {
            red: 255,
            green: 222,
            blue: 173,
            alpha: 1.0,
        }),
        "navy" => Some(IrColor {
            red: 0,
            green: 0,
            blue: 128,
            alpha: 1.0,
        }),
        "oldlace" => Some(IrColor {
            red: 253,
            green: 245,
            blue: 230,
            alpha: 1.0,
        }),
        "olive" => Some(IrColor {
            red: 128,
            green: 128,
            blue: 0,
            alpha: 1.0,
        }),
        "olivedrab" => Some(IrColor {
            red: 107,
            green: 142,
            blue: 35,
            alpha: 1.0,
        }),
        "orange" => Some(IrColor {
            red: 255,
            green: 165,
            blue: 0,
            alpha: 1.0,
        }),
        "orangered" => Some(IrColor {
            red: 255,
            green: 69,
            blue: 0,
            alpha: 1.0,
        }),
        "orchid" => Some(IrColor {
            red: 218,
            green: 112,
            blue: 214,
            alpha: 1.0,
        }),
        "palegoldenrod" => Some(IrColor {
            red: 238,
            green: 232,
            blue: 170,
            alpha: 1.0,
        }),
        "palegreen" => Some(IrColor {
            red: 152,
            green: 251,
            blue: 152,
            alpha: 1.0,
        }),
        "paleturquoise" => Some(IrColor {
            red: 175,
            green: 238,
            blue: 238,
            alpha: 1.0,
        }),
        "palevioletred" => Some(IrColor {
            red: 219,
            green: 112,
            blue: 147,
            alpha: 1.0,
        }),
        "papayawhip" => Some(IrColor {
            red: 255,
            green: 239,
            blue: 213,
            alpha: 1.0,
        }),
        "peachpuff" => Some(IrColor {
            red: 255,
            green: 218,
            blue: 185,
            alpha: 1.0,
        }),
        "peru" => Some(IrColor {
            red: 205,
            green: 133,
            blue: 63,
            alpha: 1.0,
        }),
        "pink" => Some(IrColor {
            red: 255,
            green: 192,
            blue: 203,
            alpha: 1.0,
        }),
        "plum" => Some(IrColor {
            red: 221,
            green: 160,
            blue: 221,
            alpha: 1.0,
        }),
        "powderblue" => Some(IrColor {
            red: 176,
            green: 224,
            blue: 230,
            alpha: 1.0,
        }),
        "purple" => Some(IrColor {
            red: 128,
            green: 0,
            blue: 128,
            alpha: 1.0,
        }),
        "rebeccapurple" => Some(IrColor {
            red: 102,
            green: 51,
            blue: 153,
            alpha: 1.0,
        }),
        "red" => Some(IrColor {
            red: 255,
            green: 0,
            blue: 0,
            alpha: 1.0,
        }),
        "rosybrown" => Some(IrColor {
            red: 188,
            green: 143,
            blue: 143,
            alpha: 1.0,
        }),
        "royalblue" => Some(IrColor {
            red: 65,
            green: 105,
            blue: 225,
            alpha: 1.0,
        }),
        "saddlebrown" => Some(IrColor {
            red: 139,
            green: 69,
            blue: 19,
            alpha: 1.0,
        }),
        "salmon" => Some(IrColor {
            red: 250,
            green: 128,
            blue: 114,
            alpha: 1.0,
        }),
        "sandybrown" => Some(IrColor {
            red: 244,
            green: 164,
            blue: 96,
            alpha: 1.0,
        }),
        "seagreen" => Some(IrColor {
            red: 46,
            green: 139,
            blue: 87,
            alpha: 1.0,
        }),
        "seashell" => Some(IrColor {
            red: 255,
            green: 245,
            blue: 238,
            alpha: 1.0,
        }),
        "sienna" => Some(IrColor {
            red: 160,
            green: 82,
            blue: 45,
            alpha: 1.0,
        }),
        "silver" => Some(IrColor {
            red: 192,
            green: 192,
            blue: 192,
            alpha: 1.0,
        }),
        "skyblue" => Some(IrColor {
            red: 135,
            green: 206,
            blue: 235,
            alpha: 1.0,
        }),
        "slateblue" => Some(IrColor {
            red: 106,
            green: 90,
            blue: 205,
            alpha: 1.0,
        }),
        "slategray" | "slategrey" => Some(IrColor {
            red: 112,
            green: 128,
            blue: 144,
            alpha: 1.0,
        }),
        "snow" => Some(IrColor {
            red: 255,
            green: 250,
            blue: 250,
            alpha: 1.0,
        }),
        "springgreen" => Some(IrColor {
            red: 0,
            green: 255,
            blue: 127,
            alpha: 1.0,
        }),
        "steelblue" => Some(IrColor {
            red: 70,
            green: 130,
            blue: 180,
            alpha: 1.0,
        }),
        "tan" => Some(IrColor {
            red: 210,
            green: 180,
            blue: 140,
            alpha: 1.0,
        }),
        "teal" => Some(IrColor {
            red: 0,
            green: 128,
            blue: 128,
            alpha: 1.0,
        }),
        "thistle" => Some(IrColor {
            red: 216,
            green: 191,
            blue: 216,
            alpha: 1.0,
        }),
        "tomato" => Some(IrColor {
            red: 255,
            green: 99,
            blue: 71,
            alpha: 1.0,
        }),
        "turquoise" => Some(IrColor {
            red: 64,
            green: 224,
            blue: 208,
            alpha: 1.0,
        }),
        "violet" => Some(IrColor {
            red: 238,
            green: 130,
            blue: 238,
            alpha: 1.0,
        }),
        "wheat" => Some(IrColor {
            red: 245,
            green: 222,
            blue: 179,
            alpha: 1.0,
        }),
        "white" => Some(IrColor {
            red: 255,
            green: 255,
            blue: 255,
            alpha: 1.0,
        }),
        "whitesmoke" => Some(IrColor {
            red: 245,
            green: 245,
            blue: 245,
            alpha: 1.0,
        }),
        "yellow" => Some(IrColor {
            red: 255,
            green: 255,
            blue: 0,
            alpha: 1.0,
        }),
        "yellowgreen" => Some(IrColor {
            red: 154,
            green: 205,
            blue: 50,
            alpha: 1.0,
        }),
        _ => None,
    }
}

fn parse_number(value: &str) -> Option<f64> {
    value
        .parse::<i32>()
        .map(f64::from)
        .ok()
        .or_else(|| value.parse::<f32>().ok().map(f64::from))
}

fn parse_boolean(value: &str) -> Option<bool> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "yes" => Some(true),
        "false" | "no" => Some(false),
        _ => None,
    }
}

/// Parses the v2.5.1 textual range grammar without invoking the source
/// parser. Endpoints are unsigned decimal text; an endpoint that exceeds the
/// upstream `Int` domain becomes an omitted endpoint, matching
/// `toIntOrNull()` in `ValueFactory.range`.
fn parse_range(value: &str, span: SourceSpan) -> Option<IrRange> {
    let (start, end) = value.split_once("..")?;
    if start.contains('.') || end.contains('.') {
        return None;
    }
    if !start.is_empty() && !start.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    if !end.is_empty() && !end.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    Some(IrRange {
        start: parse_range_endpoint(start),
        end: parse_range_endpoint(end),
        span,
    })
}

fn parse_range_endpoint(value: &str) -> Option<i32> {
    (!value.is_empty())
        .then(|| value.parse::<i32>().ok())
        .flatten()
}

/// Mirrors the reviewed `NumberValue`/Float boundary used by Arkst's
/// numeric output materialization.
pub(crate) fn number_to_text(number: f64) -> String {
    if number.is_finite() {
        let float = number as f32;
        if f64::from(float) == number {
            return float.to_string();
        }
    }
    number.to_string()
}

fn range_to_text(range: &IrRange) -> String {
    format!(
        "{}..{}",
        range
            .start
            .map_or_else(String::new, |value| value.to_string()),
        range
            .end
            .map_or_else(String::new, |value| value.to_string())
    )
}

#[cfg(test)]
mod tests {
    use super::{
        convert_domain_with_origin, convert_integer_with_origin, convert_range,
        convert_range_with_origin, convert_scalar, convert_scalar_with_origin,
        convert_target_with_origin, convert_text_alignment_with_origin,
        convert_text_case_with_origin, convert_text_decoration_with_origin,
        convert_text_size_with_origin, convert_text_style_with_origin,
        convert_text_variant_with_origin, convert_text_weight_with_origin, raw_body_dynamic_text,
        ClosedEnumSpec, ClosedEnumTarget, ClosedEnumVariant, ConversionError, ConversionTarget,
        DomainTarget, DomainValue, RawMarkdownTarget, RuntimeValue, RuntimeValueKind, ScalarTarget,
        ScalarValue, TargetValue, ValueOrigin, ValueProvenance,
    };
    use arkst_ir::{
        IrBoxType, IrCallable, IrCallableCapture, IrCallableResourceContext, IrCaptionPosition,
        IrCapturedFunction, IrCapturedVariable, IrColor, IrComponent, IrContainerAlignment,
        IrCrossAxisAlignment, IrDictionary, IrDocumentAlignment, IrDocumentType, IrEnumValue,
        IrInline, IrInlineBody, IrMainAxisAlignment, IrNode, IrPair, IrRange, IrRawBody, IrSize,
        IrSizeUnit, IrSlideTransitionSpeed, IrSlideTransitionStyle, IrStackedComponent,
        IrStackedLayout, IrTextCase, IrTextDecoration, IrTextSize, IrTextStyle, IrTextVariant,
        IrTextWeight, IrValue,
    };
    use arkst_source::{ByteSpan, SourceId, SourceSpan, SourceText};

    fn span() -> SourceSpan {
        SourceSpan::new(SourceId(7), 10, 16)
    }

    fn raw_body(source: &str) -> IrRawBody {
        IrRawBody::new(
            SourceText::new(source.to_owned()),
            ByteSpan::new(0, source.len()),
        )
    }

    #[test]
    fn runtime_text_leaves_are_engine_owned_and_roundtrip_exact_kinds() {
        let text = " \t한글🙂 ";
        let identifier = "MiXeD_식별자";

        let runtime_string = RuntimeValue::static_value(IrValue::String(text.into()));
        let runtime_identifier =
            RuntimeValue::dynamic_value(IrValue::Identifier(identifier.into()));

        assert!(matches!(
            &runtime_string.kind,
            RuntimeValueKind::String(value) if value == text
        ));
        assert!(matches!(
            &runtime_identifier.kind,
            RuntimeValueKind::Identifier(value) if value == identifier
        ));
        assert_eq!(runtime_string.text_identity(), Some(text));
        assert_eq!(runtime_string.dynamic_text(), None);
        assert_eq!(runtime_identifier.text_identity(), Some(identifier));
        assert_eq!(runtime_identifier.dynamic_text(), Some(identifier));
        assert_eq!(
            runtime_string.semantic_value().as_ref(),
            &IrValue::String(text.into())
        );
        assert_eq!(
            runtime_identifier.to_ir_value(),
            IrValue::Identifier(identifier.into())
        );
        assert_eq!(runtime_string.into_ir_value(), IrValue::String(text.into()));
        assert_eq!(
            runtime_identifier.into_ir_value(),
            IrValue::Identifier(identifier.into())
        );
    }

    #[test]
    fn runtime_component_leaf_is_engine_owned_and_roundtrips_exact_semantics() {
        let component = IrComponent::Stacked(IrStackedComponent {
            layout: IrStackedLayout::Row,
            main_axis_alignment: Some(IrMainAxisAlignment::Start),
            cross_axis_alignment: IrCrossAxisAlignment::Stretch,
            row_gap: None,
            column_gap: None,
            children: vec![IrNode::Paragraph {
                content: vec![IrInline::Text {
                    content: "child".into(),
                    span: span(),
                }],
                span: span(),
            }],
            span: span(),
        });
        let runtime = RuntimeValue::dynamic_value(IrValue::Component(component.clone()))
            .with_provenance(ValueProvenance::source_backed_markdown_text(
                "component-source",
            ));

        assert!(matches!(
            &runtime.kind,
            RuntimeValueKind::Component(value) if value == &component
        ));
        assert_eq!(runtime.origin, ValueOrigin::Dynamic);
        assert_eq!(
            runtime.source_backed_markdown_text(),
            Some("component-source")
        );
        assert_eq!(
            runtime.semantic_value().as_ref(),
            &IrValue::Component(component.clone())
        );
        assert_eq!(runtime.to_ir_value(), IrValue::Component(component.clone()));
        assert_eq!(runtime.into_ir_value(), IrValue::Component(component));
    }

    #[test]
    fn runtime_component_collection_replacement_and_node_adaptation_preserve_contracts() {
        let component = IrComponent::Stacked(IrStackedComponent {
            layout: IrStackedLayout::Column,
            main_axis_alignment: None,
            cross_axis_alignment: IrCrossAxisAlignment::Stretch,
            row_gap: None,
            column_gap: None,
            children: Vec::new(),
            span: span(),
        });
        let child = RuntimeValue::dynamic_value(IrValue::Component(component.clone()))
            .with_provenance(ValueProvenance::source_backed_markdown_text(
                "component-source",
            ));
        let collection = RuntimeValue::collection(vec![child], ValueOrigin::Static);
        let elements = collection.into_collection().expect("runtime collection");

        assert_eq!(elements.len(), 1);
        assert_eq!(elements[0].origin, ValueOrigin::Dynamic);
        assert_eq!(
            elements[0].source_backed_markdown_text(),
            Some("component-source")
        );
        assert!(matches!(
            &elements[0].kind,
            RuntimeValueKind::Component(value) if value == &component
        ));

        let mut replaced = RuntimeValue::dynamic_value(IrValue::String("discarded".into()))
            .with_provenance(ValueProvenance::source_backed_markdown_text("discarded"));
        replaced.replace_semantic_value(IrValue::Component(component.clone()));

        assert_eq!(replaced.origin, ValueOrigin::Dynamic);
        assert!(replaced.provenance().is_materialized());
        assert!(matches!(
            &replaced.kind,
            RuntimeValueKind::Component(value) if value == &component
        ));
        assert_eq!(
            convert_target_with_origin(&replaced, ConversionTarget::Node, span()),
            Ok(TargetValue::Value(IrValue::Component(component)))
        );

        for target in [
            ConversionTarget::InlineContent,
            ConversionTarget::BlockContent,
            ConversionTarget::Iterable,
            ConversionTarget::Dictionary,
            ConversionTarget::Callable,
        ] {
            assert_eq!(
                convert_target_with_origin(&replaced, target, span()),
                Err(ConversionError::UnsupportedValue { target })
            );
        }
    }

    #[test]
    fn runtime_callable_inline_body_are_engine_owned_and_roundtrip_recursive_capture() {
        let callable_span = SourceSpan::new(SourceId(7), 20, 80);
        let nested_span = SourceSpan::new(SourceId(8), 2, 12);
        let nested = IrCallable {
            parameters: None,
            body: vec![IrNode::Paragraph {
                content: vec![IrInline::Text {
                    content: "nested".into(),
                    span: nested_span,
                }],
                span: nested_span,
            }],
            span: nested_span,
            capture: None,
        };
        let callable = IrCallable {
            parameters: None,
            body: vec![IrNode::Paragraph {
                content: vec![IrInline::Text {
                    content: "body".into(),
                    span: callable_span,
                }],
                span: callable_span,
            }],
            span: callable_span,
            capture: Some(Box::new(IrCallableCapture {
                variables: vec![IrCapturedVariable {
                    name: "captured".into(),
                    value: IrValue::Range(IrRange {
                        start: Some(1),
                        end: Some(4),
                        span: nested_span,
                    }),
                }],
                functions: vec![
                    IrCapturedFunction {
                        name: "zeta".into(),
                        callable: nested.clone(),
                    },
                    IrCapturedFunction {
                        name: "alpha".into(),
                        callable: nested,
                    },
                ],
                resource_context: Some(IrCallableResourceContext {
                    current_source: SourceId(7),
                    subdocument_root: Some(SourceId(8)),
                    loadable_libraries: true,
                }),
            })),
        };
        let value = IrValue::Callable(callable.clone());
        let runtime = RuntimeValue::dynamic_value(value.clone());

        let RuntimeValueKind::Callable(runtime_callable) = &runtime.kind else {
            panic!("callable must be engine-owned");
        };
        let capture = runtime_callable
            .capture
            .as_deref()
            .expect("legacy callable capture must decompose recursively");
        assert_eq!(capture.variables.len(), 1);
        assert_eq!(capture.variables[0].name, "captured");
        assert_eq!(capture.variables[0].value.origin, ValueOrigin::Static);
        assert!(capture.variables[0].value.provenance().is_materialized());
        assert!(matches!(
            &capture.variables[0].value.kind,
            RuntimeValueKind::Range(range) if range.span == nested_span
        ));
        assert_eq!(
            capture
                .functions
                .iter()
                .map(|function| function.name.as_str())
                .collect::<Vec<_>>(),
            vec!["zeta", "alpha"]
        );
        assert_eq!(
            capture.resource_context,
            Some(super::RuntimeCallableResourceContext {
                current_source: SourceId(7),
                subdocument_root: Some(SourceId(8)),
                loadable_libraries: true,
            })
        );
        assert_eq!(runtime.semantic_value().as_ref(), &value);
        assert_eq!(runtime.to_ir_value(), value);
        assert_eq!(runtime.into_ir_value(), IrValue::Callable(callable));

        let content_span = SourceSpan::new(SourceId(9), 0, 9);
        let body_span = SourceSpan::new(SourceId(9), 10, 19);
        let inline = IrInlineBody {
            content: vec![IrNode::Paragraph {
                content: vec![IrInline::Text {
                    content: "content".into(),
                    span: content_span,
                }],
                span: content_span,
            }],
            parameters: None,
            body: vec![IrNode::Paragraph {
                content: vec![IrInline::Text {
                    content: "callable".into(),
                    span: body_span,
                }],
                span: body_span,
            }],
            span: SourceSpan::new(SourceId(9), 0, 19),
        };
        let runtime = RuntimeValue::static_value(IrValue::InlineBody(inline.clone()));
        assert!(matches!(
            &runtime.kind,
            RuntimeValueKind::InlineBody(value) if value.to_ir() == inline
        ));
        assert_eq!(
            convert_target_with_origin(&runtime, ConversionTarget::InlineContent, content_span),
            Ok(TargetValue::Value(IrValue::Content(inline.content.clone())))
        );
        assert_eq!(
            convert_target_with_origin(&runtime, ConversionTarget::Callable, body_span),
            Ok(TargetValue::Value(IrValue::Callable(IrCallable {
                parameters: inline.parameters.clone(),
                body: inline.body.clone(),
                span: inline.span,
                capture: None,
            })))
        );
        assert_eq!(runtime.into_ir_value(), IrValue::InlineBody(inline));
    }

    #[test]
    fn runtime_callable_capture_retains_nested_runtime_provenance_until_ir_projection() {
        let captured = RuntimeValue::dynamic_value(IrValue::Content(vec![IrNode::Paragraph {
            content: vec![IrInline::Text {
                content: "captured".into(),
                span: span(),
            }],
            span: span(),
        }]))
        .with_provenance(ValueProvenance::source_backed_markdown_text(
            "captured-source",
        ));
        let callable = super::RuntimeCallable {
            parameters: None,
            body: Vec::new(),
            span: span(),
            capture: Some(Box::new(super::RuntimeCallableCapture {
                variables: vec![super::RuntimeCapturedVariable {
                    name: "value".into(),
                    value: captured,
                }],
                functions: Vec::new(),
                resource_context: None,
            })),
        };
        let runtime = RuntimeValue::callable(callable, ValueOrigin::Static);
        let captured = &runtime
            .as_callable()
            .expect("runtime callable")
            .capture
            .as_deref()
            .expect("runtime capture")
            .variables[0]
            .value;

        assert_eq!(captured.origin, ValueOrigin::Dynamic);
        assert_eq!(
            captured.source_backed_markdown_text(),
            Some("captured-source")
        );
        assert!(matches!(&captured.kind, RuntimeValueKind::Content(_)));

        let IrValue::Callable(projected) = runtime.to_ir_value() else {
            panic!("temporary projection must remain a callable");
        };
        let projected_capture = projected.capture.expect("projected capture");
        let projected_value = &projected_capture.variables[0].value;
        assert!(matches!(projected_value, IrValue::Content(_)));
    }

    #[test]
    fn runtime_content_is_engine_owned_and_roundtrips_exact_tree() {
        let call_span = SourceSpan::new(SourceId(7), 30, 48);
        let nested_span = SourceSpan::new(SourceId(7), 49, 60);
        let content = vec![
            IrNode::Paragraph {
                content: vec![IrInline::Emphasis {
                    content: vec![IrInline::Text {
                        content: "nested".into(),
                        span: nested_span,
                    }],
                    span: nested_span,
                }],
                span: nested_span,
            },
            IrNode::FunctionCall {
                name: "unresolved".into(),
                positional_args: vec![IrValue::String("arg".into())],
                named_args: Vec::new(),
                ordered_args: None,
                lambda_parameters: None,
                body: Some(vec![IrNode::Paragraph {
                    content: vec![IrInline::Text {
                        content: "body".into(),
                        span: call_span,
                    }],
                    span: call_span,
                }]),
                raw_body: None,
                span: call_span,
            },
        ];
        let value = IrValue::Content(content.clone());
        let runtime = RuntimeValue::dynamic_value(value.clone()).with_provenance(
            ValueProvenance::source_backed_markdown_text("source-backed-content"),
        );

        assert!(runtime.is_content());
        assert!(matches!(
            &runtime.kind,
            RuntimeValueKind::Content(nodes) if nodes == &content
        ));
        assert_eq!(runtime.origin, ValueOrigin::Dynamic);
        assert_eq!(
            runtime.source_backed_markdown_text(),
            Some("source-backed-content")
        );
        assert_eq!(runtime.semantic_value().as_ref(), &value);
        assert_eq!(runtime.to_ir_value(), value);
        assert_eq!(runtime.into_ir_value(), IrValue::Content(content));
    }

    #[test]
    fn runtime_content_collection_and_replacement_preserve_provenance_contracts() {
        let source_backed =
            RuntimeValue::dynamic_value(IrValue::Content(vec![IrNode::Paragraph {
                content: vec![IrInline::Text {
                    content: "source".into(),
                    span: span(),
                }],
                span: span(),
            }]))
            .with_provenance(ValueProvenance::source_backed_markdown_text("source"));
        let materialized = RuntimeValue::static_value(IrValue::Content(Vec::new()));
        let collection =
            RuntimeValue::collection(vec![source_backed, materialized], ValueOrigin::Static);
        let elements = collection.into_collection().expect("runtime collection");

        assert_eq!(elements.len(), 2);
        assert!(matches!(&elements[0].kind, RuntimeValueKind::Content(_)));
        assert_eq!(elements[0].origin, ValueOrigin::Dynamic);
        assert_eq!(elements[0].source_backed_markdown_text(), Some("source"));
        assert!(matches!(&elements[1].kind, RuntimeValueKind::Content(_)));
        assert_eq!(elements[1].origin, ValueOrigin::Static);
        assert!(elements[1].provenance().is_materialized());

        let replacement = IrValue::Content(vec![IrNode::Paragraph {
            content: vec![IrInline::Text {
                content: "replacement".into(),
                span: span(),
            }],
            span: span(),
        }]);
        let mut replaced = RuntimeValue::dynamic_value(IrValue::Identifier("discarded".into()))
            .with_provenance(ValueProvenance::source_backed_markdown_text("discarded"));
        replaced.replace_semantic_value(replacement.clone());

        assert_eq!(replaced.origin, ValueOrigin::Dynamic);
        assert!(replaced.provenance().is_materialized());
        assert!(matches!(&replaced.kind, RuntimeValueKind::Content(_)));
        assert_eq!(replaced.to_ir_value(), replacement);
    }

    #[test]
    fn runtime_content_targets_and_color_adaptation_preserve_existing_contracts() {
        let plain = IrValue::Content(vec![IrNode::Paragraph {
            content: vec![IrInline::Text {
                content: "rgba(1,2,3,0.5)".into(),
                span: span(),
            }],
            span: span(),
        }]);
        let source_backed = RuntimeValue::dynamic_value(plain.clone()).with_provenance(
            ValueProvenance::source_backed_markdown_text("rgba(1,2,3,0.5)"),
        );

        for target in [
            ConversionTarget::InlineContent,
            ConversionTarget::BlockContent,
            ConversionTarget::Iterable,
        ] {
            assert_eq!(
                convert_target_with_origin(&source_backed, target, span()),
                Ok(TargetValue::Value(plain.clone()))
            );
        }
        assert_eq!(
            super::convert_color_with_origin(&source_backed),
            Ok(IrColor {
                red: 1,
                green: 2,
                blue: 3,
                alpha: 0.5,
            })
        );
        assert!(
            super::convert_color_with_origin(&RuntimeValue::static_value(plain.clone())).is_err()
        );

        let rich = RuntimeValue::dynamic_value(IrValue::Content(vec![IrNode::Paragraph {
            content: vec![IrInline::Emphasis {
                content: vec![IrInline::Text {
                    content: "red".into(),
                    span: span(),
                }],
                span: span(),
            }],
            span: span(),
        }]));
        assert!(super::convert_color_with_origin(&rich).is_err());

        for target in [
            ConversionTarget::Number,
            ConversionTarget::String,
            ConversionTarget::Range,
            ConversionTarget::Size,
            ConversionTarget::Enum,
            ConversionTarget::Node,
            ConversionTarget::Dictionary,
            ConversionTarget::Callable,
        ] {
            assert!(
                convert_target_with_origin(&source_backed, target, span()).is_err(),
                "Content unexpectedly converted to {target:?}"
            );
        }
    }

    #[test]
    fn runtime_pair_dictionary_are_engine_owned_and_roundtrip_nested_semantics() {
        let entry_span = SourceSpan::new(SourceId(7), 20, 30);
        let nested_span = SourceSpan::new(SourceId(7), 31, 40);
        let dictionary_span = SourceSpan::new(SourceId(7), 20, 60);
        let dictionary = IrValue::Dictionary(IrDictionary {
            entries: vec![
                IrPair {
                    first: Box::new(IrValue::String("dup".into())),
                    second: Box::new(IrValue::Pair(IrPair {
                        first: Box::new(IrValue::Number(1.0)),
                        second: Box::new(IrValue::Range(IrRange {
                            start: Some(-2),
                            end: Some(4),
                            span: nested_span,
                        })),
                        span: nested_span,
                    })),
                    span: entry_span,
                },
                IrPair {
                    first: Box::new(IrValue::String("dup".into())),
                    second: Box::new(IrValue::Number(2.0)),
                    span: span(),
                },
            ],
            span: dictionary_span,
        });
        let runtime = RuntimeValue::dynamic_value(dictionary.clone());

        let RuntimeValueKind::Dictionary(runtime_dictionary) = &runtime.kind else {
            panic!("dictionary must be engine-owned");
        };
        assert_eq!(runtime_dictionary.span, dictionary_span);
        assert_eq!(runtime_dictionary.entries.len(), 2);
        let RuntimeValueKind::Pair(first_entry) = &runtime_dictionary.entries[0].kind else {
            panic!("dictionary entry must remain a runtime Pair");
        };
        assert_eq!(first_entry.span, entry_span);
        assert!(matches!(
            &first_entry.first.kind,
            RuntimeValueKind::String(value) if value == "dup"
        ));
        let RuntimeValueKind::Pair(nested_pair) = &first_entry.second.kind else {
            panic!("nested Pair must remain engine-owned");
        };
        assert_eq!(nested_pair.span, nested_span);
        assert!(matches!(
            &nested_pair.first.kind,
            RuntimeValueKind::Number(value) if *value == 1.0
        ));
        assert!(matches!(
            &nested_pair.second.kind,
            RuntimeValueKind::Range(value)
                if value.start == Some(-2)
                    && value.end == Some(4)
                    && value.span == nested_span
        ));
        assert!(matches!(
            &runtime_dictionary.entries[1].kind,
            RuntimeValueKind::Pair(_)
        ));
        assert_eq!(runtime.semantic_value().as_ref(), &dictionary);
        assert_eq!(runtime.to_ir_value(), dictionary);
        assert_eq!(runtime.clone().into_ir_value(), dictionary);
    }

    #[test]
    fn runtime_pair_dictionary_constructors_preserve_nested_origin_and_provenance() {
        let pair_span = SourceSpan::new(SourceId(7), 70, 80);
        let dictionary_span = SourceSpan::new(SourceId(7), 70, 90);
        let first = RuntimeValue::dynamic_value(IrValue::String("left".into()))
            .with_provenance(ValueProvenance::source_backed_markdown_text("left-source"));
        let second = RuntimeValue::static_value(IrValue::Range(IrRange {
            start: Some(1),
            end: Some(3),
            span: pair_span,
        }));
        let pair = RuntimeValue::pair(first, second, pair_span, ValueOrigin::Dynamic)
            .with_provenance(ValueProvenance::source_backed_markdown_text("pair-source"));
        let dictionary = RuntimeValue::dictionary(vec![pair], dictionary_span, ValueOrigin::Static)
            .expect("runtime dictionary accepts Pair entries");

        assert_eq!(dictionary.origin, ValueOrigin::Static);
        assert!(dictionary.provenance().is_materialized());
        let RuntimeValueKind::Dictionary(runtime_dictionary) = &dictionary.kind else {
            panic!("dictionary must be engine-owned");
        };
        let entry = &runtime_dictionary.entries[0];
        assert_eq!(entry.origin, ValueOrigin::Dynamic);
        assert_eq!(entry.source_backed_markdown_text(), Some("pair-source"));
        let RuntimeValueKind::Pair(pair) = &entry.kind else {
            panic!("dictionary entry must remain a runtime Pair");
        };
        assert_eq!(pair.span, pair_span);
        assert_eq!(pair.first.origin, ValueOrigin::Dynamic);
        assert_eq!(
            pair.first.source_backed_markdown_text(),
            Some("left-source")
        );
        assert!(matches!(
            &pair.first.kind,
            RuntimeValueKind::String(value) if value == "left"
        ));
        assert_eq!(pair.second.origin, ValueOrigin::Static);
        assert!(pair.second.provenance().is_materialized());
        assert!(matches!(
            &pair.second.kind,
            RuntimeValueKind::Range(value) if value.span == pair_span
        ));

        assert!(RuntimeValue::dictionary(
            vec![RuntimeValue::static_value(IrValue::String(
                "not-pair".into()
            ))],
            dictionary_span,
            ValueOrigin::Static,
        )
        .is_none());
    }

    #[test]
    fn runtime_pair_dictionary_replacement_and_targets_preserve_existing_contracts() {
        let dictionary = IrValue::Dictionary(IrDictionary {
            entries: vec![IrPair {
                first: Box::new(IrValue::String("key".into())),
                second: Box::new(IrValue::Number(1.0)),
                span: span(),
            }],
            span: span(),
        });
        let mut replaced = RuntimeValue::dynamic_value(IrValue::Identifier("discarded".into()))
            .with_provenance(ValueProvenance::source_backed_markdown_text("discarded"));
        replaced.replace_semantic_value(dictionary.clone());

        assert_eq!(replaced.origin, ValueOrigin::Dynamic);
        assert!(replaced.provenance().is_materialized());
        let RuntimeValueKind::Dictionary(runtime_dictionary) = &replaced.kind else {
            panic!("replacement must reclassify Dictionary");
        };
        assert_eq!(runtime_dictionary.entries.len(), 1);
        let entry = &runtime_dictionary.entries[0];
        assert_eq!(entry.origin, ValueOrigin::Dynamic);
        assert!(entry.provenance().is_materialized());
        let RuntimeValueKind::Pair(pair) = &entry.kind else {
            panic!("replacement dictionary entry must be runtime Pair");
        };
        assert_eq!(pair.first.origin, ValueOrigin::Dynamic);
        assert!(pair.first.provenance().is_materialized());
        assert_eq!(pair.second.origin, ValueOrigin::Dynamic);
        assert!(pair.second.provenance().is_materialized());

        for target in [ConversionTarget::Iterable, ConversionTarget::Dictionary] {
            assert_eq!(
                convert_target_with_origin(&replaced, target, span()),
                Ok(TargetValue::Value(dictionary.clone()))
            );
        }
        for target in [
            ConversionTarget::Number,
            ConversionTarget::String,
            ConversionTarget::InlineContent,
            ConversionTarget::BlockContent,
            ConversionTarget::Node,
            ConversionTarget::Callable,
        ] {
            assert!(
                convert_target_with_origin(&replaced, target, span()).is_err(),
                "Dictionary unexpectedly converted to {target:?}"
            );
        }

        let pair_value = IrValue::Pair(IrPair {
            first: Box::new(IrValue::String("left".into())),
            second: Box::new(IrValue::Number(2.0)),
            span: span(),
        });
        let pair = RuntimeValue::static_value(pair_value.clone());
        assert!(matches!(&pair.kind, RuntimeValueKind::Pair(_)));
        assert_eq!(
            convert_target_with_origin(&pair, ConversionTarget::Iterable, span()),
            Ok(TargetValue::Value(pair_value))
        );
        assert_eq!(
            convert_target_with_origin(&pair, ConversionTarget::Dictionary, span()),
            Err(ConversionError::UnsupportedValue {
                target: ConversionTarget::Dictionary,
            })
        );
        assert!(convert_target_with_origin(&pair, ConversionTarget::String, span()).is_err());
    }

    #[test]
    fn runtime_text_collection_children_keep_independent_origin_provenance_and_kind() {
        let string = RuntimeValue::dynamic_value(IrValue::String("source text".into()))
            .with_provenance(ValueProvenance::source_backed_markdown_text("source text"));
        let identifier = RuntimeValue::static_value(IrValue::Identifier("literal".into()));
        let collection = RuntimeValue::collection(vec![string, identifier], ValueOrigin::Static);

        let elements = collection.into_collection().expect("runtime collection");
        assert_eq!(elements.len(), 2);
        assert_eq!(elements[0].origin, ValueOrigin::Dynamic);
        assert_eq!(
            elements[0].source_backed_markdown_text(),
            Some("source text")
        );
        assert!(matches!(
            &elements[0].kind,
            RuntimeValueKind::String(value) if value == "source text"
        ));
        assert_eq!(elements[1].origin, ValueOrigin::Static);
        assert!(elements[1].provenance().is_materialized());
        assert!(matches!(
            &elements[1].kind,
            RuntimeValueKind::Identifier(value) if value == "literal"
        ));
    }

    #[test]
    fn runtime_text_replacement_and_direct_adaptation_preserve_existing_contracts() {
        let mut value = RuntimeValue::dynamic_value(IrValue::Identifier("discarded".into()))
            .with_provenance(ValueProvenance::source_backed_markdown_text("discarded"));
        value.replace_semantic_value(IrValue::String("2..4".into()));

        assert!(matches!(
            &value.kind,
            RuntimeValueKind::String(text) if text == "2..4"
        ));
        assert_eq!(value.origin, ValueOrigin::Dynamic);
        assert!(value.provenance().is_materialized());
        assert_eq!(value.dynamic_text(), Some("2..4"));

        let parsed = convert_range_with_origin(&value, span()).expect("dynamic textual range");
        assert_eq!(parsed.start, Some(2));
        assert_eq!(parsed.end, Some(4));
        assert_eq!(parsed.span, span());

        let number = RuntimeValue::dynamic_value(IrValue::Identifier("-3.5".into()));
        assert_eq!(
            convert_scalar_with_origin(&number, ScalarTarget::Number),
            Ok(ScalarValue::Number(-3.5))
        );

        let size = RuntimeValue::dynamic_value(IrValue::String("12px".into()));
        assert_eq!(
            convert_domain_with_origin(&size, DomainTarget::Size),
            Ok(DomainValue::Size(IrSize {
                value: 12.0,
                unit: IrSizeUnit::Px,
            }))
        );
    }

    #[test]
    fn runtime_text_content_targets_preserve_static_string_identifier_asymmetry() {
        let static_string = RuntimeValue::static_value(IrValue::String("*literal*".into()));
        let static_identifier = RuntimeValue::static_value(IrValue::Identifier("*literal*".into()));
        let dynamic_identifier =
            RuntimeValue::dynamic_value(IrValue::Identifier("*dynamic*".into()));

        assert_eq!(
            convert_target_with_origin(&static_string, ConversionTarget::InlineContent, span()),
            Ok(TargetValue::Value(IrValue::Content(vec![
                IrNode::Paragraph {
                    content: vec![IrInline::Text {
                        content: "*literal*".into(),
                        span: span(),
                    }],
                    span: span(),
                }
            ])))
        );
        assert_eq!(
            convert_target_with_origin(&static_string, ConversionTarget::BlockContent, span()),
            Ok(TargetValue::RawMarkdown {
                target: RawMarkdownTarget::Block,
                text: "*literal*".into(),
            })
        );
        for target in [
            ConversionTarget::InlineContent,
            ConversionTarget::BlockContent,
        ] {
            assert_eq!(
                convert_target_with_origin(&static_identifier, target, span()),
                Err(ConversionError::UnsupportedValue { target })
            );
        }
        assert_eq!(
            convert_target_with_origin(
                &dynamic_identifier,
                ConversionTarget::InlineContent,
                span(),
            ),
            Ok(TargetValue::RawMarkdown {
                target: RawMarkdownTarget::Inline,
                text: "*dynamic*".into(),
            })
        );
    }

    #[test]
    fn runtime_primitive_leaves_are_engine_owned_and_preserve_unit_none_distinction() {
        let unit = RuntimeValue::static_value(IrValue::Unit);
        let none = RuntimeValue::dynamic_value(IrValue::None);
        let boolean = RuntimeValue::static_value(IrValue::Boolean(true));
        let negative_zero = RuntimeValue::dynamic_value(IrValue::Number(-0.0));

        assert!(matches!(&unit.kind, RuntimeValueKind::Unit));
        assert!(matches!(&none.kind, RuntimeValueKind::None));
        assert!(matches!(&boolean.kind, RuntimeValueKind::Boolean(true)));
        assert!(matches!(
            &negative_zero.kind,
            RuntimeValueKind::Number(value)
                if value.to_bits() == (-0.0_f64).to_bits()
        ));
        assert!(!unit.is_none());
        assert!(none.is_none());
        assert!(!boolean.is_none());
        assert_eq!(unit.semantic_value().as_ref(), &IrValue::Unit);
        assert_eq!(none.semantic_value().as_ref(), &IrValue::None);
        assert_eq!(boolean.semantic_value().as_ref(), &IrValue::Boolean(true));
        assert_eq!(negative_zero.origin, ValueOrigin::Dynamic);
        assert!(matches!(
            negative_zero.to_ir_value(),
            IrValue::Number(value) if value.to_bits() == (-0.0_f64).to_bits()
        ));
        assert!(matches!(
            negative_zero.into_ir_value(),
            IrValue::Number(value) if value.to_bits() == (-0.0_f64).to_bits()
        ));
    }

    #[test]
    fn runtime_primitive_collection_children_keep_independent_origin_and_provenance() {
        let expected = IrValue::Collection(vec![
            IrValue::Unit,
            IrValue::None,
            IrValue::Boolean(false),
            IrValue::Number(7.5),
            IrValue::Collection(vec![IrValue::Boolean(true), IrValue::Number(-2.0)]),
        ]);
        let runtime = RuntimeValue::new(expected.clone(), ValueOrigin::Dynamic);
        assert_eq!(runtime.to_ir_value(), expected);

        let elements = runtime.into_collection().expect("runtime collection");
        assert_eq!(elements.len(), 5);
        assert!(elements
            .iter()
            .all(|value| value.origin == ValueOrigin::Dynamic));
        assert!(elements
            .iter()
            .all(|value| value.provenance().is_materialized()));
        assert!(matches!(&elements[0].kind, RuntimeValueKind::Unit));
        assert!(matches!(&elements[1].kind, RuntimeValueKind::None));
        assert!(matches!(
            &elements[2].kind,
            RuntimeValueKind::Boolean(false)
        ));
        assert!(matches!(
            &elements[3].kind,
            RuntimeValueKind::Number(value) if value.to_bits() == 7.5_f64.to_bits()
        ));

        let nested = elements[4]
            .clone()
            .into_collection()
            .expect("nested collection");
        assert!(matches!(&nested[0].kind, RuntimeValueKind::Boolean(true)));
        assert!(matches!(
            &nested[1].kind,
            RuntimeValueKind::Number(value) if value.to_bits() == (-2.0_f64).to_bits()
        ));
        assert!(nested
            .iter()
            .all(|value| value.origin == ValueOrigin::Dynamic));
    }

    #[test]
    fn primitive_replacement_and_target_conversion_do_not_widen_string_policy() {
        let mut value = RuntimeValue::dynamic_value(IrValue::Boolean(false))
            .with_provenance(ValueProvenance::source_backed_markdown_text("false"));
        value.replace_semantic_value(IrValue::Number(4.0));
        assert!(matches!(
            &value.kind,
            RuntimeValueKind::Number(number) if number.to_bits() == 4.0_f64.to_bits()
        ));
        assert_eq!(value.origin, ValueOrigin::Dynamic);
        assert!(value.provenance().is_materialized());
        assert_eq!(
            convert_scalar_with_origin(&value, ScalarTarget::Number),
            Ok(ScalarValue::Number(4.0))
        );
        assert_eq!(
            convert_scalar_with_origin(&value, ScalarTarget::String),
            Ok(ScalarValue::String("4".into()))
        );
        assert_eq!(convert_integer_with_origin(&value), Ok(4));

        for rejected in [IrValue::Unit, IrValue::None] {
            let argument = RuntimeValue::static_value(rejected);
            assert_eq!(
                convert_scalar_with_origin(&argument, ScalarTarget::String),
                Err(ConversionError::UnsupportedValue {
                    target: ConversionTarget::String,
                })
            );
        }
        let static_text = RuntimeValue::static_value(IrValue::String("4".into()));
        assert!(convert_scalar_with_origin(&static_text, ScalarTarget::Number).is_err());
    }

    #[test]
    fn runtime_range_leaf_is_engine_owned_and_roundtrips_exact_span() {
        let original = IrRange {
            start: None,
            end: Some(5),
            span: SourceSpan::new(SourceId(11), 3, 7),
        };
        let runtime = RuntimeValue::dynamic_value(IrValue::Range(original.clone()))
            .with_provenance(ValueProvenance::source_backed_markdown_text("..5"));

        assert!(matches!(
            &runtime.kind,
            RuntimeValueKind::Range(range) if range == &original
        ));
        assert_eq!(runtime.origin, ValueOrigin::Dynamic);
        assert_eq!(runtime.source_backed_markdown_text(), Some("..5"));
        assert_eq!(
            runtime.semantic_value().as_ref(),
            &IrValue::Range(original.clone())
        );
        assert_eq!(runtime.to_ir_value(), IrValue::Range(original.clone()));
        assert_eq!(runtime.into_ir_value(), IrValue::Range(original));
    }

    #[test]
    fn runtime_range_collection_child_keeps_origin_provenance_and_span() {
        let range = IrRange {
            start: Some(-2),
            end: None,
            span: SourceSpan::new(SourceId(13), 4, 8),
        };
        let child = RuntimeValue::dynamic_value(IrValue::Range(range.clone()))
            .with_provenance(ValueProvenance::source_backed_markdown_text("-2.."));
        let collection = RuntimeValue::collection(vec![child], ValueOrigin::Static);

        let elements = collection.into_collection().expect("runtime collection");
        assert_eq!(elements.len(), 1);
        assert_eq!(elements[0].origin, ValueOrigin::Dynamic);
        assert_eq!(elements[0].source_backed_markdown_text(), Some("-2.."));
        assert!(matches!(
            &elements[0].kind,
            RuntimeValueKind::Range(actual) if actual == &range
        ));
    }

    #[test]
    fn runtime_range_replacement_and_conversion_preserve_existing_contracts() {
        let original = IrRange {
            start: Some(1),
            end: Some(3),
            span: SourceSpan::new(SourceId(15), 2, 6),
        };
        let caller_span = SourceSpan::new(SourceId(99), 0, 1);
        let mut runtime = RuntimeValue::dynamic_value(IrValue::String("discarded".into()))
            .with_provenance(ValueProvenance::source_backed_markdown_text("discarded"));

        runtime.replace_semantic_value(IrValue::Range(original.clone()));

        assert!(matches!(
            &runtime.kind,
            RuntimeValueKind::Range(actual) if actual == &original
        ));
        assert_eq!(runtime.origin, ValueOrigin::Dynamic);
        assert!(runtime.provenance().is_materialized());
        assert_eq!(
            convert_range_with_origin(&runtime, caller_span),
            Ok(original.clone())
        );
        assert_eq!(
            convert_scalar_with_origin(&runtime, ScalarTarget::String),
            Ok(ScalarValue::String("1..3".into()))
        );

        let parsed = convert_range_with_origin(
            &RuntimeValue::dynamic_value(IrValue::String("2..".into())),
            caller_span,
        )
        .expect("dynamic textual range");
        assert_eq!(parsed.start, Some(2));
        assert_eq!(parsed.end, None);
        assert_eq!(parsed.span, caller_span);
    }

    #[test]
    fn runtime_domain_leaves_are_engine_owned_and_roundtrip_exact_values() {
        let size = IrSize {
            value: -0.0,
            unit: IrSizeUnit::Em,
        };
        let color = IrColor {
            red: 1,
            green: 2,
            blue: 3,
            alpha: -0.0,
        };
        let enumeration = IrEnumValue::ContainerAlignment(IrContainerAlignment::End);

        let runtime_size = RuntimeValue::static_value(IrValue::Size(size.clone()));
        let runtime_color = RuntimeValue::dynamic_value(IrValue::Color(color.clone()));
        let runtime_enum = RuntimeValue::static_value(IrValue::Enum(enumeration));

        assert!(matches!(
            &runtime_size.kind,
            RuntimeValueKind::Size(actual)
                if actual.value.to_bits() == (-0.0_f64).to_bits()
                    && actual.unit == IrSizeUnit::Em
        ));
        assert!(matches!(
            &runtime_color.kind,
            RuntimeValueKind::Color(actual)
                if actual.red == 1
                    && actual.green == 2
                    && actual.blue == 3
                    && actual.alpha.to_bits() == (-0.0_f64).to_bits()
        ));
        assert!(matches!(
            &runtime_enum.kind,
            RuntimeValueKind::Enum(actual) if *actual == enumeration
        ));

        let IrValue::Size(projected_size) = runtime_size.to_ir_value() else {
            panic!("expected typed Size projection");
        };
        assert_eq!(projected_size.value.to_bits(), (-0.0_f64).to_bits());
        assert_eq!(projected_size.unit, IrSizeUnit::Em);

        let IrValue::Color(projected_color) = runtime_color.to_ir_value() else {
            panic!("expected typed Color projection");
        };
        assert_eq!(projected_color.alpha.to_bits(), (-0.0_f64).to_bits());
        assert_eq!(runtime_enum.into_ir_value(), IrValue::Enum(enumeration));
    }

    #[test]
    fn runtime_domain_collection_children_keep_independent_origin_and_provenance() {
        let size = RuntimeValue::dynamic_value(IrValue::Size(IrSize {
            value: 12.5,
            unit: IrSizeUnit::Cm,
        }))
        .with_provenance(ValueProvenance::source_backed_markdown_text("12.5cm"));
        let color = RuntimeValue::static_value(IrValue::Color(IrColor {
            red: 51,
            green: 102,
            blue: 153,
            alpha: 0.5,
        }));
        let enumeration = RuntimeValue::dynamic_value(IrValue::Enum(IrEnumValue::CaptionPosition(
            IrCaptionPosition::Top,
        )))
        .with_provenance(ValueProvenance::source_backed_markdown_text("top"));

        let collection =
            RuntimeValue::collection(vec![size, color, enumeration], ValueOrigin::Static);
        let elements = collection.into_collection().expect("runtime collection");

        assert_eq!(elements.len(), 3);
        assert_eq!(elements[0].origin, ValueOrigin::Dynamic);
        assert_eq!(elements[0].source_backed_markdown_text(), Some("12.5cm"));
        assert!(matches!(&elements[0].kind, RuntimeValueKind::Size(_)));
        assert_eq!(elements[1].origin, ValueOrigin::Static);
        assert!(elements[1].provenance().is_materialized());
        assert!(matches!(&elements[1].kind, RuntimeValueKind::Color(_)));
        assert_eq!(elements[2].origin, ValueOrigin::Dynamic);
        assert_eq!(elements[2].source_backed_markdown_text(), Some("top"));
        assert!(matches!(
            &elements[2].kind,
            RuntimeValueKind::Enum(IrEnumValue::CaptionPosition(IrCaptionPosition::Top))
        ));
    }

    #[test]
    fn runtime_domain_replacement_and_direct_conversion_preserve_closed_contracts() {
        let mut size = RuntimeValue::dynamic_value(IrValue::String("discarded".into()))
            .with_provenance(ValueProvenance::source_backed_markdown_text("discarded"));
        let expected_size = IrSize {
            value: 8.0,
            unit: IrSizeUnit::Pt,
        };
        size.replace_semantic_value(IrValue::Size(expected_size.clone()));
        assert!(matches!(&size.kind, RuntimeValueKind::Size(_)));
        assert_eq!(size.origin, ValueOrigin::Dynamic);
        assert!(size.provenance().is_materialized());
        assert_eq!(
            convert_domain_with_origin(&size, DomainTarget::Size),
            Ok(DomainValue::Size(expected_size))
        );

        let expected_color = IrColor {
            red: 255,
            green: 0,
            blue: 17,
            alpha: 0.25,
        };
        let color = RuntimeValue::static_value(IrValue::Color(expected_color.clone()));
        assert_eq!(
            convert_domain_with_origin(&color, DomainTarget::Color),
            Ok(DomainValue::Color(expected_color))
        );

        let enumeration = RuntimeValue::static_value(IrValue::Enum(
            IrEnumValue::ContainerAlignment(IrContainerAlignment::Center),
        ));
        assert_eq!(
            convert_domain_with_origin(
                &enumeration,
                DomainTarget::ClosedEnum(ClosedEnumTarget::ContainerAlignment),
            ),
            Ok(DomainValue::Enum(IrEnumValue::ContainerAlignment(
                IrContainerAlignment::Center,
            )))
        );
        assert_eq!(
            convert_domain_with_origin(
                &enumeration,
                DomainTarget::ClosedEnum(ClosedEnumTarget::StackedMainAxisAlignment),
            ),
            Err(ConversionError::UnsupportedValue {
                target: ConversionTarget::Enum,
            })
        );
    }

    #[test]
    fn runtime_value_tree_owns_nested_collection_provenance() {
        let direct = RuntimeValue::dynamic_value(IrValue::Content(Vec::new()))
            .with_provenance(ValueProvenance::source_backed_markdown_text("* alpha"));
        assert_eq!(direct.origin, ValueOrigin::Dynamic);
        assert_eq!(direct.source_backed_markdown_text(), Some("* alpha"));

        let repeated = RuntimeValue::collection(
            vec![
                RuntimeValue::static_value(IrValue::Content(Vec::new()))
                    .with_provenance(ValueProvenance::source_backed_markdown()),
                RuntimeValue::static_value(IrValue::Content(Vec::new())),
            ],
            ValueOrigin::Static,
        );
        let elements = repeated
            .clone()
            .into_collection()
            .expect("runtime Collection keeps child values");
        assert!(matches!(
            elements[0].provenance(),
            ValueProvenance::SourceBackedMarkdown { source_text: None }
        ));
        assert!(elements[1].provenance().is_materialized());

        let rematerialized = RuntimeValue::static_value(repeated.into_ir_value());
        let elements = rematerialized
            .into_collection()
            .expect("materialized Collection remains a runtime Collection");
        assert!(elements
            .iter()
            .all(|element| element.provenance().is_materialized()));

        let generated_table = RuntimeValue::static_value(IrValue::Content(Vec::new()));
        assert!(generated_table.provenance().is_materialized());
    }

    #[test]
    fn raw_body_dynamic_text_matches_trim_indent_trim_end_without_losing_leading_blank_lines() {
        let body = raw_body("\n\n\t\tα\r\n\t\t  β \r\n\t\t\r\n");
        assert_eq!(raw_body_dynamic_text(&body).as_deref(), Some("\nα\n  β"));
    }

    #[test]
    fn raw_body_dynamic_text_applies_trim_indent_and_trim_end_to_the_full_token() {
        let body = raw_body("\n\n\n  hello  \n\n\n");
        // The first blank line is the body delimiter consumed by Kotlin's
        // trimIndent convention; additional leading blank lines remain.
        // trimEnd removes all trailing blank-line whitespace after the
        // common indentation has been removed.
        assert_eq!(raw_body_dynamic_text(&body).as_deref(), Some("\n\nhello"));
    }

    #[test]
    fn raw_body_dynamic_text_uses_the_minimum_indent_of_all_body_lines() {
        let source = ".theme\n    first\n  second\n";
        let body_end = source.len();
        let body = IrRawBody::new(
            arkst_source::SourceText::new(source),
            ByteSpan::new(7, body_end),
        );

        assert_eq!(
            raw_body_dynamic_text(&body).as_deref(),
            Some("  first\nsecond")
        );
    }

    #[test]
    fn raw_body_dynamic_text_is_derived_only_from_a_valid_source_span() {
        let body = IrRawBody::new(SourceText::new("body".to_string()), ByteSpan::new(1, 5));
        assert!(raw_body_dynamic_text(&body).is_none());
    }

    #[test]
    fn target_conversion_keeps_scalar_content_and_origin_ordering_explicit() {
        assert_eq!(
            convert_target_with_origin(
                &RuntimeValue::dynamic_value(IrValue::String("2".into())),
                ConversionTarget::Number,
                span(),
            ),
            Ok(TargetValue::Value(IrValue::Number(2.0)))
        );
        assert!(matches!(
            convert_target_with_origin(
                &RuntimeValue::static_value(IrValue::String("2".into())),
                ConversionTarget::Number,
                span(),
            ),
            Err(ConversionError::UnsupportedValue {
                target: ConversionTarget::Number
            })
        ));

        let static_string = convert_target_with_origin(
            &RuntimeValue::static_value(IrValue::String("*text*".into())),
            ConversionTarget::InlineContent,
            span(),
        )
        .expect("static String adapts to plain inline content");
        assert!(matches!(
            static_string,
            TargetValue::Value(IrValue::Content(nodes))
                if matches!(nodes.as_slice(), [IrNode::Paragraph { content, .. }] if matches!(content.as_slice(), [IrInline::Text { content, .. }] if content == "*text*"))
        ));

        assert_eq!(
            convert_target_with_origin(
                &RuntimeValue::dynamic_value(IrValue::String("*text*".into())),
                ConversionTarget::InlineContent,
                span(),
            ),
            Ok(TargetValue::RawMarkdown {
                target: RawMarkdownTarget::Inline,
                text: "*text*".into(),
            })
        );
        assert_eq!(
            convert_target_with_origin(
                &RuntimeValue::static_value(IrValue::String("**text**".into())),
                ConversionTarget::BlockContent,
                span(),
            ),
            Ok(TargetValue::RawMarkdown {
                target: RawMarkdownTarget::Block,
                text: "**text**".into(),
            })
        );
        for (value, expected) in [
            (IrValue::Number(2.0), "2"),
            (IrValue::Boolean(true), "true"),
        ] {
            let converted = convert_target_with_origin(
                &RuntimeValue::static_value(value),
                ConversionTarget::BlockContent,
                span(),
            )
            .expect("scalar result adapts to literal block content");
            assert!(matches!(
                converted,
                TargetValue::Value(IrValue::Content(nodes))
                    if matches!(nodes.as_slice(), [IrNode::Paragraph { content, .. }]
                        if matches!(content.as_slice(), [IrInline::Text { content, .. }]
                            if content == expected))
            ));
        }
        for value in [IrValue::Number(2.0), IrValue::Boolean(true)] {
            assert!(matches!(
                convert_target_with_origin(
                    &RuntimeValue::static_value(value),
                    ConversionTarget::InlineContent,
                    span(),
                ),
                Err(ConversionError::UnsupportedValue {
                    target: ConversionTarget::InlineContent
                })
            ));
        }
    }

    #[test]
    fn target_conversion_preserves_node_callable_iterable_dictionary_and_none() {
        let component = IrValue::Component(IrComponent::Stacked(IrStackedComponent {
            layout: IrStackedLayout::Row,
            main_axis_alignment: Some(IrMainAxisAlignment::Start),
            cross_axis_alignment: IrCrossAxisAlignment::Stretch,
            row_gap: None,
            column_gap: None,
            children: Vec::new(),
            span: span(),
        }));
        assert_eq!(
            convert_target_with_origin(
                &RuntimeValue::static_value(component.clone()),
                ConversionTarget::Node,
                span(),
            ),
            Ok(TargetValue::Value(component))
        );

        let inline_body = IrValue::InlineBody(IrInlineBody {
            content: vec![IrNode::Paragraph {
                content: vec![IrInline::Text {
                    content: "body".into(),
                    span: span(),
                }],
                span: span(),
            }],
            parameters: None,
            body: Vec::new(),
            span: span(),
        });
        assert!(matches!(
            convert_target_with_origin(
                &RuntimeValue::static_value(inline_body),
                ConversionTarget::Callable,
                span(),
            ),
            Ok(TargetValue::Value(IrValue::Callable(_)))
        ));

        let pair = IrValue::Pair(IrPair {
            first: Box::new(IrValue::String("key".into())),
            second: Box::new(IrValue::Number(1.0)),
            span: span(),
        });
        assert!(matches!(
            convert_target_with_origin(
                &RuntimeValue::static_value(pair),
                ConversionTarget::Iterable,
                span(),
            ),
            Ok(TargetValue::Value(IrValue::Pair(_)))
        ));

        let dictionary = IrValue::Dictionary(arkst_ir::IrDictionary {
            entries: Vec::new(),
            span: span(),
        });
        assert!(matches!(
            convert_target_with_origin(
                &RuntimeValue::static_value(dictionary.clone()),
                ConversionTarget::Dictionary,
                span(),
            ),
            Ok(TargetValue::Value(IrValue::Dictionary(_)))
        ));
        assert!(matches!(
            convert_target_with_origin(
                &RuntimeValue::dynamic_value(IrValue::String("- key: value".into())),
                ConversionTarget::Dictionary,
                span(),
            ),
            Ok(TargetValue::RawMarkdown {
                target: RawMarkdownTarget::Dictionary,
                ..
            })
        ));

        let none = convert_target_with_origin(
            &RuntimeValue::static_value(IrValue::None),
            ConversionTarget::BlockContent,
            span(),
        )
        .expect("None adapts to literal non-null block content");
        assert!(matches!(
            none,
            TargetValue::Value(IrValue::Content(nodes))
                if matches!(nodes.as_slice(), [IrNode::Paragraph { content, .. }]
                    if matches!(content.as_slice(), [IrInline::Text { content, .. }]
                        if content == "None"))
        ));

        for target in [
            ConversionTarget::Number,
            ConversionTarget::InlineContent,
            ConversionTarget::Node,
            ConversionTarget::Iterable,
            ConversionTarget::Dictionary,
            ConversionTarget::Callable,
        ] {
            assert!(
                convert_target_with_origin(
                    &RuntimeValue::static_value(IrValue::None),
                    target,
                    span(),
                )
                .is_err(),
                "None unexpectedly converted to {target:?}"
            );
        }
    }

    #[test]
    fn number_conversion_uses_integer_then_float_and_no_trimming() {
        assert_eq!(
            convert_scalar(&IrValue::String("12".into()), ScalarTarget::Number),
            Ok(ScalarValue::Number(12.0))
        );
        assert_eq!(
            convert_scalar(&IrValue::String("-2.5".into()), ScalarTarget::Number),
            Ok(ScalarValue::Number(-2.5))
        );
        assert_eq!(
            convert_scalar(&IrValue::Number(-2.5), ScalarTarget::Number),
            Ok(ScalarValue::Number(-2.5))
        );
        assert_eq!(
            convert_scalar(&IrValue::String(" 12".into()), ScalarTarget::Number),
            Err(ConversionError::InvalidText {
                target: ConversionTarget::Number
            })
        );
    }

    #[test]
    fn boolean_conversion_is_explicit_and_case_insensitive() {
        for value in ["true", "TRUE", "yes", "Yes"] {
            assert_eq!(
                convert_scalar(&IrValue::String(value.into()), ScalarTarget::Boolean),
                Ok(ScalarValue::Boolean(true))
            );
        }
        for value in ["false", "FALSE", "no", "No"] {
            assert_eq!(
                convert_scalar(&IrValue::String(value.into()), ScalarTarget::Boolean),
                Ok(ScalarValue::Boolean(false))
            );
        }
        assert_eq!(
            convert_scalar(&IrValue::Boolean(true), ScalarTarget::Boolean),
            Ok(ScalarValue::Boolean(true))
        );
        assert!(convert_scalar(&IrValue::String("maybe".into()), ScalarTarget::Boolean).is_err());
        assert!(convert_scalar(&IrValue::Number(1.0), ScalarTarget::Boolean).is_err());
    }

    #[test]
    fn range_conversion_accepts_only_the_reviewed_textual_forms() {
        for (text, start, end) in [
            ("2..5", Some(2), Some(5)),
            ("..5", None, Some(5)),
            ("2..", Some(2), None),
            ("..", None, None),
        ] {
            assert_eq!(
                convert_range(&IrValue::String(text.into()), span()),
                Ok(IrRange {
                    start,
                    end,
                    span: span()
                })
            );
        }
        assert!(convert_range(&IrValue::String("-2..5".into()), span()).is_err());
        assert!(convert_range(&IrValue::String("2 .. 5".into()), span()).is_err());
        assert!(convert_range(&IrValue::String("2...5".into()), span()).is_err());
    }

    #[test]
    fn string_conversion_is_scalar_and_range_aware_but_not_generic() {
        assert_eq!(
            convert_scalar(&IrValue::String("text".into()), ScalarTarget::String),
            Ok(ScalarValue::String("text".into()))
        );
        assert_eq!(
            convert_scalar(
                &IrValue::Range(IrRange {
                    start: Some(2),
                    end: None,
                    span: span()
                }),
                ScalarTarget::String
            ),
            Ok(ScalarValue::String("2..".into()))
        );
        assert!(convert_scalar(&IrValue::Unit, ScalarTarget::String).is_err());
        assert!(convert_scalar(&IrValue::None, ScalarTarget::String).is_err());
        assert!(convert_scalar(&IrValue::Collection(Vec::new()), ScalarTarget::String).is_err());
    }

    #[test]
    fn none_is_not_a_scalar_or_range_conversion() {
        for target in [
            ScalarTarget::Number,
            ScalarTarget::Boolean,
            ScalarTarget::String,
        ] {
            assert_eq!(
                convert_scalar(&IrValue::None, target),
                Err(ConversionError::UnsupportedValue {
                    target: target.into()
                })
            );
        }
        assert_eq!(
            convert_range(&IrValue::None, span()),
            Err(ConversionError::UnsupportedValue {
                target: ConversionTarget::Range
            })
        );
    }

    #[test]
    fn typed_range_identity_preserves_its_original_provenance() {
        let original = IrRange {
            start: Some(1),
            end: Some(3),
            span: span(),
        };
        assert_eq!(
            convert_range(
                &IrValue::Range(original.clone()),
                SourceSpan::new(SourceId(9), 0, 1)
            ),
            Ok(original)
        );
    }

    #[test]
    fn conversion_requires_dynamic_origin_for_textual_target_adaptation() {
        let static_number = RuntimeValue::static_value(IrValue::String("-3.5".into()));
        let dynamic_number = RuntimeValue::dynamic_value(IrValue::String("-3.5".into()));
        assert!(convert_scalar_with_origin(&static_number, ScalarTarget::Number).is_err());
        assert_eq!(
            convert_scalar_with_origin(&dynamic_number, ScalarTarget::Number),
            Ok(ScalarValue::Number(-3.5))
        );

        let static_boolean = RuntimeValue::static_value(IrValue::String("YES".into()));
        let dynamic_boolean = RuntimeValue::dynamic_value(IrValue::String("YES".into()));
        assert!(convert_scalar_with_origin(&static_boolean, ScalarTarget::Boolean).is_err());
        assert_eq!(
            convert_scalar_with_origin(&dynamic_boolean, ScalarTarget::Boolean),
            Ok(ScalarValue::Boolean(true))
        );

        let static_range = RuntimeValue::static_value(IrValue::String("2..4".into()));
        let dynamic_range = RuntimeValue::dynamic_value(IrValue::String("2..4".into()));
        assert!(convert_range_with_origin(&static_range, span()).is_err());
        assert_eq!(
            convert_range_with_origin(&dynamic_range, span()),
            Ok(IrRange {
                start: Some(2),
                end: Some(4),
                span: span(),
            })
        );
    }

    #[test]
    fn component_is_rejected_by_scalar_range_and_domain_converters() {
        let component = IrValue::Component(IrComponent::Stacked(IrStackedComponent {
            layout: IrStackedLayout::Row,
            main_axis_alignment: Some(IrMainAxisAlignment::Start),
            cross_axis_alignment: IrCrossAxisAlignment::Stretch,
            row_gap: None,
            column_gap: None,
            children: Vec::new(),
            span: span(),
        }));
        let argument = RuntimeValue::dynamic_value(component);

        for target in [
            ScalarTarget::Number,
            ScalarTarget::Boolean,
            ScalarTarget::String,
        ] {
            assert_eq!(
                convert_scalar_with_origin(&argument, target),
                Err(ConversionError::UnsupportedValue {
                    target: target.into()
                })
            );
        }
        assert_eq!(
            convert_range_with_origin(&argument, span()),
            Err(ConversionError::UnsupportedValue {
                target: ConversionTarget::Range
            })
        );
        for (target, conversion_target) in [
            (DomainTarget::Size, ConversionTarget::Size),
            (DomainTarget::Color, ConversionTarget::Color),
            (
                DomainTarget::ClosedEnum(ClosedEnumTarget::DocumentType),
                ConversionTarget::Enum,
            ),
        ] {
            assert_eq!(
                convert_domain_with_origin(&argument, target),
                Err(ConversionError::UnsupportedValue {
                    target: conversion_target
                })
            );
        }
    }

    #[test]
    fn size_conversion_matches_the_closed_v251_decimal_unit_grammar() {
        for (text, value, unit) in [
            ("10", 10.0, IrSizeUnit::Px),
            ("10px", 10.0, IrSizeUnit::Px),
            ("10PX", 10.0, IrSizeUnit::Px),
            ("12.5cm", 12.5, IrSizeUnit::Cm),
            ("-3in", -3.0, IrSizeUnit::In),
            ("0%", 0.0, IrSizeUnit::Percent),
            ("1.25em", 1.25, IrSizeUnit::Em),
            ("8pt", 8.0, IrSizeUnit::Pt),
            ("2mm", 2.0, IrSizeUnit::Mm),
        ] {
            assert_eq!(
                convert_domain_with_origin(
                    &RuntimeValue::dynamic_value(IrValue::String(text.into())),
                    DomainTarget::Size,
                ),
                Ok(DomainValue::Size(IrSize { value, unit }))
            );
        }

        for text in [
            "10 px", " 10px", "10px ", "foo", "1.2.3cm", "px", "1e3", ".5px", "10.", "10vw",
        ] {
            assert!(
                convert_domain_with_origin(
                    &RuntimeValue::dynamic_value(IrValue::String(text.into())),
                    DomainTarget::Size,
                )
                .is_err(),
                "unexpectedly accepted {text:?}"
            );
        }
    }

    #[test]
    fn size_and_color_typed_values_are_identity_only_and_static_text_is_rejected() {
        let size = IrSize {
            value: 12.5,
            unit: IrSizeUnit::Cm,
        };
        assert_eq!(
            convert_domain_with_origin(
                &RuntimeValue::static_value(IrValue::Size(size.clone())),
                DomainTarget::Size,
            ),
            Ok(DomainValue::Size(size))
        );
        assert!(convert_domain_with_origin(
            &RuntimeValue::static_value(IrValue::String("10px".into())),
            DomainTarget::Size,
        )
        .is_err());

        let color = IrColor {
            red: 1,
            green: 2,
            blue: 3,
            alpha: 0.5,
        };
        assert_eq!(
            convert_domain_with_origin(
                &RuntimeValue::static_value(IrValue::Color(color.clone())),
                DomainTarget::Color,
            ),
            Ok(DomainValue::Color(color))
        );
        assert!(convert_domain_with_origin(
            &RuntimeValue::static_value(IrValue::String("red".into())),
            DomainTarget::Color,
        )
        .is_err());
    }

    #[test]
    fn color_conversion_uses_the_v251_decoder_order_and_channel_boundaries() {
        for (text, expected) in [
            (
                "#FF0000",
                IrColor {
                    red: 255,
                    green: 0,
                    blue: 0,
                    alpha: 1.0,
                },
            ),
            (
                "#369",
                IrColor {
                    red: 51,
                    green: 102,
                    blue: 153,
                    alpha: 1.0,
                },
            ),
            (
                "#369f",
                IrColor {
                    red: 51,
                    green: 102,
                    blue: 153,
                    alpha: 1.0,
                },
            ),
            (
                "#336699",
                IrColor {
                    red: 51,
                    green: 102,
                    blue: 153,
                    alpha: 1.0,
                },
            ),
            (
                "#33669980",
                IrColor {
                    red: 51,
                    green: 102,
                    blue: 153,
                    alpha: 1.0,
                },
            ),
            (
                "#aBcD",
                IrColor {
                    red: 170,
                    green: 187,
                    blue: 204,
                    alpha: 1.0,
                },
            ),
            (
                "#aAbBcCdD",
                IrColor {
                    red: 170,
                    green: 187,
                    blue: 204,
                    alpha: 1.0,
                },
            ),
            (
                "#33669900",
                IrColor {
                    red: 51,
                    green: 102,
                    blue: 153,
                    alpha: 1.0,
                },
            ),
            (
                "#3690",
                IrColor {
                    red: 51,
                    green: 102,
                    blue: 153,
                    alpha: 1.0,
                },
            ),
            (
                "#336699ff",
                IrColor {
                    red: 51,
                    green: 102,
                    blue: 153,
                    alpha: 1.0,
                },
            ),
            (
                "rgb(145, 168, 50)",
                IrColor {
                    red: 145,
                    green: 168,
                    blue: 50,
                    alpha: 1.0,
                },
            ),
            (
                "rgba(120,111,93, 0.5)",
                IrColor {
                    red: 120,
                    green: 111,
                    blue: 93,
                    alpha: 0.5,
                },
            ),
            (
                "hsv(208, 70, 66)",
                IrColor {
                    red: 50,
                    green: 113,
                    blue: 168,
                    alpha: 1.0,
                },
            ),
            (
                "hsl(208, 54, 43)",
                IrColor {
                    red: 50,
                    green: 113,
                    blue: 168,
                    alpha: 1.0,
                },
            ),
            (
                "ToMaTo",
                IrColor {
                    red: 255,
                    green: 99,
                    blue: 71,
                    alpha: 1.0,
                },
            ),
        ] {
            assert_eq!(
                convert_domain_with_origin(
                    &RuntimeValue::dynamic_value(IrValue::String(text.into())),
                    DomainTarget::Color,
                ),
                Ok(DomainValue::Color(expected)),
                "color fixture {text:?}"
            );
        }

        for text in [
            "#hello",
            "#12",
            "#12345",
            "#1234567",
            "#123456789",
            "#GGG",
            "#369x",
            "#33669Z",
            "#336699ZZ",
            "rgb(300, 0, 0)",
            "rgba(100, 200, 200, 1.5)",
            "rgba(100, 200, 200, -0.5)",
            "rgba(100, 200, 200, +0.5)",
            "hsl(120, 105, 20)",
            "hsv(120, 10,200)",
            "hsv(20, 10, 50, 10)",
            "rgb(1 2 3)",
            "rebeccapurplex",
        ] {
            assert!(
                convert_domain_with_origin(
                    &RuntimeValue::dynamic_value(IrValue::String(text.into())),
                    DomainTarget::Color,
                )
                .is_err(),
                "unexpectedly accepted {text:?}"
            );
        }
    }

    #[test]
    fn color_argument_conversion_reuses_dynamic_plain_content_without_widening_static_content() {
        let source_span = span();
        let content = IrValue::Content(vec![IrNode::Paragraph {
            content: vec![IrInline::Text {
                content: "rgba(1,2,3,0.5)".to_string(),
                span: source_span,
            }],
            span: source_span,
        }]);
        assert_eq!(
            super::convert_color_with_origin(&RuntimeValue::dynamic_value(content.clone())),
            Ok(IrColor {
                red: 1,
                green: 2,
                blue: 3,
                alpha: 0.5,
            })
        );
        assert!(super::convert_color_with_origin(&RuntimeValue::static_value(content)).is_err());

        let rich = IrValue::Content(vec![IrNode::Paragraph {
            content: vec![IrInline::Emphasis {
                content: vec![IrInline::Text {
                    content: "red".to_string(),
                    span: source_span,
                }],
                span: source_span,
            }],
            span: source_span,
        }]);
        assert!(super::convert_color_with_origin(&RuntimeValue::dynamic_value(rich)).is_err());
    }

    #[test]
    fn text_size_conversion_is_closed_identity_aware_and_origin_gated() {
        for (text, expected) in [
            ("tiny", Some(IrTextSize::Tiny)),
            ("SMALL", Some(IrTextSize::Small)),
            ("normal", None),
            ("Medium", Some(IrTextSize::Medium)),
            ("large", Some(IrTextSize::Large)),
            ("larger", Some(IrTextSize::Larger)),
            ("huge", Some(IrTextSize::Huge)),
        ] {
            assert_eq!(
                convert_text_size_with_origin(&RuntimeValue::dynamic_value(IrValue::String(
                    text.to_string(),
                ))),
                Ok(expected),
                "{text}"
            );
        }

        assert_eq!(
            convert_text_size_with_origin(&RuntimeValue::static_value(IrValue::None)),
            Ok(None)
        );
        assert_eq!(
            convert_text_size_with_origin(&RuntimeValue::dynamic_value(IrValue::String(
                "giant".to_string(),
            ))),
            Err(ConversionError::InvalidText {
                target: ConversionTarget::Enum,
            })
        );
        assert_eq!(
            convert_text_size_with_origin(&RuntimeValue::static_value(IrValue::String(
                "large".to_string(),
            ))),
            Err(ConversionError::UnsupportedValue {
                target: ConversionTarget::Enum,
            })
        );
    }

    #[test]
    fn text_weight_conversion_is_closed_identity_aware_and_origin_gated() {
        for (text, expected) in [
            ("normal", None),
            ("bold", Some(IrTextWeight::Bold)),
            ("BOLD", Some(IrTextWeight::Bold)),
        ] {
            assert_eq!(
                convert_text_weight_with_origin(&RuntimeValue::dynamic_value(IrValue::String(
                    text.to_string()
                ),)),
                Ok(expected),
                "{text}"
            );
        }

        assert_eq!(
            convert_text_weight_with_origin(&RuntimeValue::static_value(IrValue::None)),
            Ok(None)
        );
        assert_eq!(
            convert_text_weight_with_origin(&RuntimeValue::dynamic_value(IrValue::String(
                "heavy".to_string(),
            ))),
            Err(ConversionError::InvalidText {
                target: ConversionTarget::Enum,
            })
        );
        assert_eq!(
            convert_text_weight_with_origin(&RuntimeValue::static_value(IrValue::String(
                "bold".to_string(),
            ))),
            Err(ConversionError::UnsupportedValue {
                target: ConversionTarget::Enum,
            })
        );
    }

    #[test]
    fn text_style_conversion_is_closed_identity_aware_and_origin_gated() {
        for (text, expected) in [
            ("normal", None),
            ("italic", Some(IrTextStyle::Italic)),
            ("ITALIC", Some(IrTextStyle::Italic)),
        ] {
            assert_eq!(
                convert_text_style_with_origin(&RuntimeValue::dynamic_value(IrValue::String(
                    text.to_string()
                ),)),
                Ok(expected),
                "{text}"
            );
        }

        assert_eq!(
            convert_text_style_with_origin(&RuntimeValue::static_value(IrValue::None)),
            Ok(None)
        );
        assert_eq!(
            convert_text_style_with_origin(&RuntimeValue::dynamic_value(IrValue::String(
                "oblique".to_string(),
            ))),
            Err(ConversionError::InvalidText {
                target: ConversionTarget::Enum,
            })
        );
        assert_eq!(
            convert_text_style_with_origin(&RuntimeValue::static_value(IrValue::String(
                "italic".to_string(),
            ))),
            Err(ConversionError::UnsupportedValue {
                target: ConversionTarget::Enum,
            })
        );
    }

    #[test]
    fn text_decoration_conversion_is_closed_identity_aware_and_origin_gated() {
        for (text, expected) in [
            ("none", None),
            ("underline", Some(IrTextDecoration::Underline)),
            ("OVERLINE", Some(IrTextDecoration::Overline)),
            ("UnderOverline", Some(IrTextDecoration::UnderOverline)),
            ("strikethrough", Some(IrTextDecoration::Strikethrough)),
            ("all", Some(IrTextDecoration::All)),
        ] {
            assert_eq!(
                convert_text_decoration_with_origin(&RuntimeValue::dynamic_value(IrValue::String(
                    text.to_string()
                ),)),
                Ok(expected),
                "{text}"
            );
        }

        assert_eq!(
            convert_text_decoration_with_origin(&RuntimeValue::static_value(IrValue::None)),
            Ok(None)
        );
        assert_eq!(
            convert_text_decoration_with_origin(&RuntimeValue::dynamic_value(IrValue::String(
                "blink".to_string(),
            ))),
            Err(ConversionError::InvalidText {
                target: ConversionTarget::Enum,
            })
        );
        assert_eq!(
            convert_text_decoration_with_origin(&RuntimeValue::static_value(IrValue::String(
                "underline".to_string(),
            ))),
            Err(ConversionError::UnsupportedValue {
                target: ConversionTarget::Enum,
            })
        );
    }

    #[test]
    fn text_alignment_conversion_is_closed_nullable_and_origin_gated() {
        for (text, expected) in [
            ("start", IrDocumentAlignment::Start),
            ("CENTER", IrDocumentAlignment::Center),
            ("End", IrDocumentAlignment::End),
            ("justify", IrDocumentAlignment::Justify),
        ] {
            assert_eq!(
                convert_text_alignment_with_origin(&RuntimeValue::dynamic_value(IrValue::String(
                    text.to_string()
                ),)),
                Ok(Some(expected)),
                "{text}"
            );
        }

        assert_eq!(
            convert_text_alignment_with_origin(&RuntimeValue::static_value(IrValue::None)),
            Ok(None)
        );
        assert_eq!(
            convert_text_alignment_with_origin(&RuntimeValue::dynamic_value(IrValue::String(
                "sideways".to_string(),
            ))),
            Err(ConversionError::InvalidText {
                target: ConversionTarget::Enum,
            })
        );
        assert_eq!(
            convert_text_alignment_with_origin(&RuntimeValue::static_value(IrValue::String(
                "center".to_string(),
            ))),
            Err(ConversionError::UnsupportedValue {
                target: ConversionTarget::Enum,
            })
        );
    }

    #[test]
    fn text_case_conversion_is_closed_identity_aware_and_origin_gated() {
        for (text, expected) in [
            ("none", None),
            ("uppercase", Some(IrTextCase::Uppercase)),
            ("LOWERCASE", Some(IrTextCase::Lowercase)),
            ("Capitalize", Some(IrTextCase::Capitalize)),
        ] {
            assert_eq!(
                convert_text_case_with_origin(&RuntimeValue::dynamic_value(IrValue::String(
                    text.to_string()
                ),)),
                Ok(expected),
                "{text}"
            );
        }

        assert_eq!(
            convert_text_case_with_origin(&RuntimeValue::static_value(IrValue::None)),
            Ok(None)
        );
        assert_eq!(
            convert_text_case_with_origin(&RuntimeValue::dynamic_value(IrValue::String(
                "titlecase".to_string(),
            ))),
            Err(ConversionError::InvalidText {
                target: ConversionTarget::Enum,
            })
        );
        assert_eq!(
            convert_text_case_with_origin(&RuntimeValue::static_value(IrValue::String(
                "uppercase".to_string(),
            ))),
            Err(ConversionError::UnsupportedValue {
                target: ConversionTarget::Enum,
            })
        );
    }

    #[test]
    fn text_variant_conversion_is_closed_identity_aware_and_origin_gated() {
        for (text, expected) in [
            ("normal", None),
            ("smallcaps", Some(IrTextVariant::SmallCaps)),
            ("SMALLCAPS", Some(IrTextVariant::SmallCaps)),
        ] {
            assert_eq!(
                convert_text_variant_with_origin(&RuntimeValue::dynamic_value(IrValue::String(
                    text.to_string()
                ),)),
                Ok(expected),
                "{text}"
            );
        }

        assert_eq!(
            convert_text_variant_with_origin(&RuntimeValue::static_value(IrValue::None)),
            Ok(None)
        );
        assert_eq!(
            convert_text_variant_with_origin(&RuntimeValue::dynamic_value(IrValue::String(
                "petitecaps".to_string(),
            ))),
            Err(ConversionError::InvalidText {
                target: ConversionTarget::Enum,
            })
        );
        assert_eq!(
            convert_text_variant_with_origin(&RuntimeValue::static_value(IrValue::String(
                "smallcaps".to_string(),
            ))),
            Err(ConversionError::UnsupportedValue {
                target: ConversionTarget::Enum,
            })
        );
    }

    #[test]
    fn closed_enum_conversion_preserves_domain_and_applies_only_declared_name_normalization() {
        for text in ["plain", "PLAIN", "Plain"] {
            assert_eq!(
                convert_domain_with_origin(
                    &RuntimeValue::dynamic_value(IrValue::String(text.into())),
                    DomainTarget::ClosedEnum(ClosedEnumTarget::DocumentType),
                ),
                Ok(DomainValue::Enum(IrEnumValue::DocumentType(
                    IrDocumentType::Plain,
                )))
            );
        }
        let typed = IrEnumValue::DocumentType(IrDocumentType::Paged);
        assert_eq!(
            convert_domain_with_origin(
                &RuntimeValue::static_value(IrValue::Enum(typed)),
                DomainTarget::ClosedEnum(ClosedEnumTarget::DocumentType),
            ),
            Ok(DomainValue::Enum(typed))
        );
        assert!(convert_domain_with_origin(
            &RuntimeValue::static_value(IrValue::String("paged".into())),
            DomainTarget::ClosedEnum(ClosedEnumTarget::DocumentType),
        )
        .is_err());
        assert!(convert_domain_with_origin(
            &RuntimeValue::dynamic_value(IrValue::String("page_d".into())),
            DomainTarget::ClosedEnum(ClosedEnumTarget::DocumentType),
        )
        .is_err());

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum Alignment {
            SpaceBetween,
        }
        let variants = [ClosedEnumVariant {
            declaration_name: "SPACE_BETWEEN",
            value: Alignment::SpaceBetween,
        }];
        let spec = ClosedEnumSpec {
            variants: &variants,
        };
        for text in ["spacebetween", "SPACEBETWEEN", "SpaceBetween"] {
            assert_eq!(spec.value_for(text), Some(Alignment::SpaceBetween));
        }
        assert_eq!(spec.value_for("space_between"), None);
    }

    #[test]
    fn caption_position_conversion_is_closed_case_insensitive_and_typed() {
        for (text, expected) in [
            ("top", IrCaptionPosition::Top),
            ("TOP", IrCaptionPosition::Top),
            ("Bottom", IrCaptionPosition::Bottom),
        ] {
            assert_eq!(
                convert_domain_with_origin(
                    &RuntimeValue::dynamic_value(IrValue::Identifier(text.into())),
                    DomainTarget::ClosedEnum(ClosedEnumTarget::CaptionPosition),
                ),
                Ok(DomainValue::Enum(IrEnumValue::CaptionPosition(expected)))
            );
        }

        let typed = IrEnumValue::CaptionPosition(IrCaptionPosition::Top);
        assert_eq!(
            convert_domain_with_origin(
                &RuntimeValue::static_value(IrValue::Enum(typed)),
                DomainTarget::ClosedEnum(ClosedEnumTarget::CaptionPosition),
            ),
            Ok(DomainValue::Enum(typed))
        );
        assert!(matches!(
            convert_domain_with_origin(
                &RuntimeValue::dynamic_value(IrValue::String("middle".into())),
                DomainTarget::ClosedEnum(ClosedEnumTarget::CaptionPosition),
            ),
            Err(ConversionError::InvalidText {
                target: ConversionTarget::Enum
            })
        ));
        assert!(matches!(
            convert_domain_with_origin(
                &RuntimeValue::static_value(IrValue::String("top".into())),
                DomainTarget::ClosedEnum(ClosedEnumTarget::CaptionPosition),
            ),
            Err(ConversionError::UnsupportedValue {
                target: ConversionTarget::Enum
            })
        ));
    }

    #[test]
    fn slide_transition_domains_are_closed_case_insensitive_and_typed() {
        for (text, expected) in [
            ("none", IrSlideTransitionStyle::None),
            ("FADE", IrSlideTransitionStyle::Fade),
            ("Slide", IrSlideTransitionStyle::Slide),
            ("zoom", IrSlideTransitionStyle::Zoom),
        ] {
            assert_eq!(
                convert_domain_with_origin(
                    &RuntimeValue::dynamic_value(IrValue::Identifier(text.into())),
                    DomainTarget::ClosedEnum(ClosedEnumTarget::SlideTransitionStyle),
                ),
                Ok(DomainValue::Enum(IrEnumValue::SlideTransitionStyle(
                    expected
                )))
            );
        }
        for (text, expected) in [
            ("default", IrSlideTransitionSpeed::Default),
            ("FAST", IrSlideTransitionSpeed::Fast),
            ("Slow", IrSlideTransitionSpeed::Slow),
        ] {
            assert_eq!(
                convert_domain_with_origin(
                    &RuntimeValue::dynamic_value(IrValue::Identifier(text.into())),
                    DomainTarget::ClosedEnum(ClosedEnumTarget::SlideTransitionSpeed),
                ),
                Ok(DomainValue::Enum(IrEnumValue::SlideTransitionSpeed(
                    expected
                )))
            );
        }

        let typed = IrEnumValue::SlideTransitionStyle(IrSlideTransitionStyle::Fade);
        assert_eq!(
            convert_domain_with_origin(
                &RuntimeValue::static_value(IrValue::Enum(typed)),
                DomainTarget::ClosedEnum(ClosedEnumTarget::SlideTransitionStyle),
            ),
            Ok(DomainValue::Enum(typed))
        );
        assert!(matches!(
            convert_domain_with_origin(
                &RuntimeValue::dynamic_value(IrValue::String("spin".into())),
                DomainTarget::ClosedEnum(ClosedEnumTarget::SlideTransitionStyle),
            ),
            Err(ConversionError::InvalidText {
                target: ConversionTarget::Enum
            })
        ));
        assert!(matches!(
            convert_domain_with_origin(
                &RuntimeValue::static_value(IrValue::String("fast".into())),
                DomainTarget::ClosedEnum(ClosedEnumTarget::SlideTransitionSpeed),
            ),
            Err(ConversionError::UnsupportedValue {
                target: ConversionTarget::Enum
            })
        ));
    }

    #[test]
    fn box_type_conversion_is_closed_case_insensitive_and_typed() {
        for (text, expected) in [
            ("callout", IrBoxType::Callout),
            ("TIP", IrBoxType::Tip),
            ("Note", IrBoxType::Note),
            ("warning", IrBoxType::Warning),
            ("ERROR", IrBoxType::Error),
        ] {
            assert_eq!(
                convert_domain_with_origin(
                    &RuntimeValue::dynamic_value(IrValue::Identifier(text.into())),
                    DomainTarget::ClosedEnum(ClosedEnumTarget::BoxType),
                ),
                Ok(DomainValue::Enum(IrEnumValue::BoxType(expected)))
            );
        }

        let typed = IrEnumValue::BoxType(IrBoxType::Warning);
        assert_eq!(
            convert_domain_with_origin(
                &RuntimeValue::static_value(IrValue::Enum(typed)),
                DomainTarget::ClosedEnum(ClosedEnumTarget::BoxType),
            ),
            Ok(DomainValue::Enum(typed))
        );
        assert!(matches!(
            convert_domain_with_origin(
                &RuntimeValue::dynamic_value(IrValue::String("urgent".into())),
                DomainTarget::ClosedEnum(ClosedEnumTarget::BoxType),
            ),
            Err(ConversionError::InvalidText {
                target: ConversionTarget::Enum
            })
        ));
        assert!(matches!(
            convert_domain_with_origin(
                &RuntimeValue::static_value(IrValue::String("warning".into())),
                DomainTarget::ClosedEnum(ClosedEnumTarget::BoxType),
            ),
            Err(ConversionError::UnsupportedValue {
                target: ConversionTarget::Enum
            })
        ));
    }

    #[test]
    fn stacked_enum_domains_and_integer_boundary_remain_typed_and_origin_aware() {
        for text in ["start", "CENTER", "spacebetween", "SpaceEvenly"] {
            assert!(matches!(
                convert_domain_with_origin(
                    &RuntimeValue::dynamic_value(IrValue::String(text.into())),
                    DomainTarget::ClosedEnum(ClosedEnumTarget::StackedMainAxisAlignment),
                ),
                Ok(DomainValue::Enum(IrEnumValue::StackedMainAxisAlignment(_)))
            ));
        }
        for text in ["start", "center", "end", "stretch"] {
            assert!(matches!(
                convert_domain_with_origin(
                    &RuntimeValue::dynamic_value(IrValue::String(text.into())),
                    DomainTarget::ClosedEnum(ClosedEnumTarget::StackedCrossAxisAlignment),
                ),
                Ok(DomainValue::Enum(IrEnumValue::StackedCrossAxisAlignment(_)))
            ));
        }
        assert!(convert_domain_with_origin(
            &RuntimeValue::dynamic_value(IrValue::String("spacebetween".into())),
            DomainTarget::ClosedEnum(ClosedEnumTarget::StackedCrossAxisAlignment),
        )
        .is_err());
        assert!(convert_domain_with_origin(
            &RuntimeValue::static_value(IrValue::Enum(IrEnumValue::StackedMainAxisAlignment(
                IrMainAxisAlignment::Center
            ),)),
            DomainTarget::ClosedEnum(ClosedEnumTarget::StackedCrossAxisAlignment),
        )
        .is_err());

        for value in [
            RuntimeValue::dynamic_value(IrValue::Number(2.0)),
            RuntimeValue::dynamic_value(IrValue::String("2".into())),
            RuntimeValue::dynamic_value(IrValue::String("2.0".into())),
        ] {
            assert_eq!(convert_integer_with_origin(&value), Ok(2));
        }
        assert_eq!(
            convert_integer_with_origin(&RuntimeValue::dynamic_value(IrValue::Number(-1.0))),
            Ok(-1)
        );
        assert!(matches!(
            convert_integer_with_origin(&RuntimeValue::dynamic_value(IrValue::Number(2.5))),
            Err(ConversionError::InvalidText {
                target: ConversionTarget::Integer
            })
        ));
        assert!(matches!(
            convert_integer_with_origin(&RuntimeValue::static_value(IrValue::String("2".into()))),
            Err(ConversionError::UnsupportedValue {
                target: ConversionTarget::Integer
            })
        ));
    }

    #[test]
    fn container_alignment_enum_is_closed_typed_and_origin_aware() {
        for (text, expected) in [
            ("start", IrContainerAlignment::Start),
            ("CENTER", IrContainerAlignment::Center),
            ("End", IrContainerAlignment::End),
        ] {
            assert_eq!(
                convert_domain_with_origin(
                    &RuntimeValue::dynamic_value(IrValue::String(text.into())),
                    DomainTarget::ClosedEnum(ClosedEnumTarget::ContainerAlignment),
                ),
                Ok(DomainValue::Enum(IrEnumValue::ContainerAlignment(expected)))
            );
        }

        let typed = IrEnumValue::ContainerAlignment(IrContainerAlignment::Center);
        assert_eq!(
            convert_domain_with_origin(
                &RuntimeValue::static_value(IrValue::Enum(typed)),
                DomainTarget::ClosedEnum(ClosedEnumTarget::ContainerAlignment),
            ),
            Ok(DomainValue::Enum(typed))
        );
        assert!(matches!(
            convert_domain_with_origin(
                &RuntimeValue::static_value(IrValue::String("center".into())),
                DomainTarget::ClosedEnum(ClosedEnumTarget::ContainerAlignment),
            ),
            Err(ConversionError::UnsupportedValue {
                target: ConversionTarget::Enum
            })
        ));
        assert!(matches!(
            convert_domain_with_origin(
                &RuntimeValue::dynamic_value(IrValue::String("middle".into())),
                DomainTarget::ClosedEnum(ClosedEnumTarget::ContainerAlignment),
            ),
            Err(ConversionError::InvalidText {
                target: ConversionTarget::Enum
            })
        ));
        assert!(matches!(
            convert_domain_with_origin(
                &RuntimeValue::static_value(IrValue::Enum(IrEnumValue::StackedMainAxisAlignment(
                    IrMainAxisAlignment::Center
                ))),
                DomainTarget::ClosedEnum(ClosedEnumTarget::ContainerAlignment),
            ),
            Err(ConversionError::UnsupportedValue {
                target: ConversionTarget::Enum
            })
        ));
    }
}
