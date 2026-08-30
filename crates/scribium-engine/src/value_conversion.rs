//! Engine-owned target conversion for the evaluator value boundary.
//!
//! Conversion is target-driven, not a general coercion graph. This module
//! keeps the upstream distinction between already typed values, dynamic raw
//! values, and structured Markdown content. Context-sensitive targets return a
//! raw conversion request for the evaluator to execute with its context; they
//! never stringify an evaluated value and feed it back through Markdown.

use scribium_ir::{
    IrCallable, IrCaptionPosition, IrColor, IrContainerAlignment, IrCrossAxisAlignment,
    IrDocumentType, IrEnumValue, IrInline, IrMainAxisAlignment, IrNamedArg, IrNode, IrRange,
    IrRawBody, IrSize, IrSizeUnit, IrValue,
};
use scribium_source::SourceSpan;
use std::ops::Deref;

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

/// An evaluated invocation argument together with the origin used by the
/// target-driven conversion boundary.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct InvocationValue {
    pub value: IrValue,
    pub origin: ValueOrigin,
}

impl InvocationValue {
    pub(crate) fn static_value(value: IrValue) -> Self {
        Self {
            value,
            origin: ValueOrigin::Static,
        }
    }

    pub(crate) fn dynamic_value(value: IrValue) -> Self {
        Self {
            value,
            origin: ValueOrigin::Dynamic,
        }
    }
}

impl Deref for InvocationValue {
    type Target = IrValue;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

/// Named invocation argument carrying the same evaluator-local origin bit.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct InvocationNamedArg {
    pub arg: IrNamedArg,
    pub origin: ValueOrigin,
}

impl InvocationNamedArg {
    pub(crate) fn new(arg: IrNamedArg, origin: ValueOrigin) -> Self {
        Self { arg, origin }
    }
}

impl Deref for InvocationNamedArg {
    type Target = IrNamedArg;

    fn deref(&self) -> &Self::Target {
        &self.arg
    }
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
    CaptionPosition,
    StackedMainAxisAlignment,
    StackedCrossAxisAlignment,
    ContainerAlignment,
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
    argument: &InvocationValue,
    target: ConversionTarget,
    span: SourceSpan,
) -> Result<TargetValue, ConversionError> {
    match target {
        ConversionTarget::Dynamic => Ok(TargetValue::Value(argument.value.clone())),
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
        ConversionTarget::Node => match &argument.value {
            IrValue::Component(component) => {
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
        ConversionTarget::Callable => match &argument.value {
            IrValue::Callable(value) => Ok(TargetValue::Value(IrValue::Callable(value.clone()))),
            IrValue::InlineBody(body) => Ok(TargetValue::Value(IrValue::Callable(IrCallable {
                parameters: body.parameters.clone(),
                body: body.body.clone(),
                span: body.span,
                capture: None,
            }))),
            IrValue::String(value) | IrValue::Identifier(value)
                if argument.origin == ValueOrigin::Dynamic =>
            {
                Ok(TargetValue::RawMarkdown {
                    target: RawMarkdownTarget::Callable,
                    text: value.clone(),
                })
            }
            _ => Err(ConversionError::UnsupportedValue {
                target: ConversionTarget::Callable,
            }),
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
    argument: &InvocationValue,
    raw_target: RawMarkdownTarget,
    span: SourceSpan,
    block_target: bool,
) -> Result<TargetValue, ConversionError> {
    match &argument.value {
        // MarkdownContentValue adapts to InlineMarkdownContentValue in the
        // pinned value model. The IR keeps one structured content carrier, so
        // both content targets preserve that carrier without flattening it.
        IrValue::Content(nodes) => Ok(TargetValue::Value(IrValue::Content(nodes.clone()))),
        // An inline contextual body is not a block body. It remains inline
        // content until the selected target asks for it.
        IrValue::InlineBody(body) if !block_target => {
            Ok(TargetValue::Value(IrValue::Content(body.content.clone())))
        }
        IrValue::String(value) | IrValue::Identifier(value)
            if argument.origin == ValueOrigin::Dynamic =>
        {
            Ok(TargetValue::RawMarkdown {
                target: raw_target,
                text: value.clone(),
            })
        }
        IrValue::String(value) if !block_target => Ok(TargetValue::Value(IrValue::Content(vec![
            IrNode::Paragraph {
                content: vec![IrInline::Text {
                    content: value.clone(),
                    span,
                }],
                span,
            },
        ]))),
        _ => Err(ConversionError::UnsupportedValue {
            target: if block_target {
                ConversionTarget::BlockContent
            } else {
                ConversionTarget::InlineContent
            },
        }),
    }
}

fn convert_structured_target(
    argument: &InvocationValue,
    raw_target: RawMarkdownTarget,
    accepted: &[IrValueKind],
) -> Result<TargetValue, ConversionError> {
    let kind = match &argument.value {
        IrValue::Collection(_) => IrValueKind::Collection,
        IrValue::Pair(_) => IrValueKind::Pair,
        IrValue::Dictionary(_) => IrValueKind::Dictionary,
        IrValue::Range(_) => IrValueKind::Range,
        // A parsed Markdown list is the frontend representation of the
        // upstream dynamic iterable adapter. Keep it structured here so the
        // evaluator can materialize its elements without reparsing or
        // stringifying the already parsed list.
        IrValue::Content(_) if raw_target == RawMarkdownTarget::Iterable => {
            return Ok(TargetValue::Value(argument.value.clone()));
        }
        _ => {
            if let IrValue::String(value) | IrValue::Identifier(value) = &argument.value {
                if argument.origin == ValueOrigin::Dynamic {
                    return Ok(TargetValue::RawMarkdown {
                        target: raw_target,
                        text: value.clone(),
                    });
                }
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
    Ok(TargetValue::Value(argument.value.clone()))
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
    convert_scalar_with_origin(&InvocationValue::dynamic_value(value.clone()), target)
}

/// Converts an invocation argument using upstream's DynamicValue gate.
///
/// Text parsing for Number and Boolean is available only for a Dynamic
/// argument. Static StringValue-shaped values remain String values and do not
/// acquire unrelated numeric, boolean, or iterable meaning.
pub(crate) fn convert_scalar_with_origin(
    argument: &InvocationValue,
    target: ScalarTarget,
) -> Result<ScalarValue, ConversionError> {
    let value = &argument.value;
    match target {
        ScalarTarget::Number => match value {
            IrValue::Number(value) => Ok(ScalarValue::Number(*value)),
            IrValue::String(value) | IrValue::Identifier(value)
                if argument.origin == ValueOrigin::Dynamic =>
            {
                parse_number(value)
                    .map(ScalarValue::Number)
                    .ok_or(ConversionError::InvalidText {
                        target: target.into(),
                    })
            }
            _ => Err(ConversionError::UnsupportedValue {
                target: target.into(),
            }),
        },
        ScalarTarget::Boolean => match value {
            IrValue::Boolean(value) => Ok(ScalarValue::Boolean(*value)),
            IrValue::String(value) | IrValue::Identifier(value)
                if argument.origin == ValueOrigin::Dynamic =>
            {
                parse_boolean(value)
                    .map(ScalarValue::Boolean)
                    .ok_or(ConversionError::InvalidText {
                        target: target.into(),
                    })
            }
            _ => Err(ConversionError::UnsupportedValue {
                target: target.into(),
            }),
        },
        ScalarTarget::String => match value {
            IrValue::String(value) | IrValue::Identifier(value) => {
                Ok(ScalarValue::String(value.clone()))
            }
            IrValue::Number(value) => Ok(ScalarValue::String(number_to_text(*value))),
            IrValue::Boolean(value) => Ok(ScalarValue::String(value.to_string())),
            IrValue::Range(value) => Ok(ScalarValue::String(range_to_text(value))),
            IrValue::None
            | IrValue::Collection(_)
            | IrValue::Pair(_)
            | IrValue::Dictionary(_)
            | IrValue::Content(_)
            | IrValue::Callable(_)
            | IrValue::InlineBody(_)
            | IrValue::Size(_)
            | IrValue::Color(_)
            | IrValue::Enum(_)
            | IrValue::Component(_) => Err(ConversionError::UnsupportedValue {
                target: target.into(),
            }),
        },
    }
}

/// Converts an existing or textual range while preserving the source span of
/// an existing typed range or attaching the caller's reliable argument span
/// to a newly parsed textual range.
pub(crate) fn convert_range_with_origin(
    argument: &InvocationValue,
    span: SourceSpan,
) -> Result<IrRange, ConversionError> {
    match &argument.value {
        IrValue::Range(range) => Ok(range.clone()),
        IrValue::String(value) | IrValue::Identifier(value)
            if argument.origin == ValueOrigin::Dynamic =>
        {
            parse_range(value, span).ok_or(ConversionError::InvalidText {
                target: ConversionTarget::Range,
            })
        }
        _ => Err(ConversionError::UnsupportedValue {
            target: ConversionTarget::Range,
        }),
    }
}

#[cfg(test)]
pub(crate) fn convert_range(value: &IrValue, span: SourceSpan) -> Result<IrRange, ConversionError> {
    convert_range_with_origin(&InvocationValue::dynamic_value(value.clone()), span)
}

/// Converts an invocation value into a typed, backend-neutral domain value.
///
/// This is deliberately not a generalized scalar coercion engine. Existing
/// typed domain values retain their domain, while textual parsing is gated by
/// the invocation origin exactly like the reviewed DynamicValue boundary.
pub(crate) fn convert_domain_with_origin(
    argument: &InvocationValue,
    target: DomainTarget,
) -> Result<DomainValue, ConversionError> {
    match target {
        DomainTarget::Size => match &argument.value {
            IrValue::Size(value) => Ok(DomainValue::Size(value.clone())),
            // ValueFactory.size accepts a dynamic numeric value through its
            // textual representation. Non-finite values still fail the
            // decimal-only parser below.
            IrValue::Number(value) => parse_size(&number_to_text(*value))
                .map(DomainValue::Size)
                .ok_or(ConversionError::InvalidText {
                    target: ConversionTarget::Size,
                }),
            IrValue::String(value) | IrValue::Identifier(value)
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
        },
        DomainTarget::Color => match &argument.value {
            IrValue::Color(value) => Ok(DomainValue::Color(value.clone())),
            IrValue::String(value) | IrValue::Identifier(value)
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
            ClosedEnumTarget::DocumentType => match &argument.value {
                IrValue::Enum(IrEnumValue::DocumentType(value)) => {
                    Ok(DomainValue::Enum(IrEnumValue::DocumentType(*value)))
                }
                IrValue::String(value) | IrValue::Identifier(value)
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
            ClosedEnumTarget::CaptionPosition => match &argument.value {
                IrValue::Enum(IrEnumValue::CaptionPosition(value)) => {
                    Ok(DomainValue::Enum(IrEnumValue::CaptionPosition(*value)))
                }
                IrValue::String(value) | IrValue::Identifier(value)
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
            ClosedEnumTarget::StackedMainAxisAlignment => match &argument.value {
                IrValue::Enum(IrEnumValue::StackedMainAxisAlignment(value)) => Ok(
                    DomainValue::Enum(IrEnumValue::StackedMainAxisAlignment(*value)),
                ),
                IrValue::String(value) | IrValue::Identifier(value)
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
            ClosedEnumTarget::StackedCrossAxisAlignment => match &argument.value {
                IrValue::Enum(IrEnumValue::StackedCrossAxisAlignment(value)) => Ok(
                    DomainValue::Enum(IrEnumValue::StackedCrossAxisAlignment(*value)),
                ),
                IrValue::String(value) | IrValue::Identifier(value)
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
            ClosedEnumTarget::ContainerAlignment => match &argument.value {
                IrValue::Enum(IrEnumValue::ContainerAlignment(value)) => {
                    Ok(DomainValue::Enum(IrEnumValue::ContainerAlignment(*value)))
                }
                IrValue::String(value) | IrValue::Identifier(value)
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
        },
    }
}

/// Converts a Quarkdown `Int` boundary without truncating fractional values.
/// Dynamic text follows the reviewed NumberValue parsing order, while static
/// strings remain strings and cannot acquire integer meaning.
pub(crate) fn convert_integer_with_origin(
    argument: &InvocationValue,
) -> Result<i32, ConversionError> {
    let number = match &argument.value {
        IrValue::Number(value) => Some(*value),
        IrValue::String(value) | IrValue::Identifier(value)
            if argument.origin == ValueOrigin::Dynamic =>
        {
            parse_number(value)
        }
        _ => None,
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

/// Mirrors the reviewed `NumberValue`/Float boundary used by Scribium's
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
        convert_target_with_origin, raw_body_dynamic_text, ClosedEnumSpec, ClosedEnumTarget,
        ClosedEnumVariant, ConversionError, ConversionTarget, DomainTarget, DomainValue,
        InvocationValue, RawMarkdownTarget, ScalarTarget, ScalarValue, TargetValue,
    };
    use scribium_ir::{
        IrCaptionPosition, IrColor, IrComponent, IrContainerAlignment, IrCrossAxisAlignment,
        IrDocumentType, IrEnumValue, IrInline, IrInlineBody, IrMainAxisAlignment, IrNode, IrPair,
        IrRange, IrRawBody, IrSize, IrSizeUnit, IrStackedComponent, IrStackedLayout, IrValue,
    };
    use scribium_source::{ByteSpan, SourceId, SourceSpan, SourceText};

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
            scribium_source::SourceText::new(source),
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
                &InvocationValue::dynamic_value(IrValue::String("2".into())),
                ConversionTarget::Number,
                span(),
            ),
            Ok(TargetValue::Value(IrValue::Number(2.0)))
        );
        assert!(matches!(
            convert_target_with_origin(
                &InvocationValue::static_value(IrValue::String("2".into())),
                ConversionTarget::Number,
                span(),
            ),
            Err(ConversionError::UnsupportedValue {
                target: ConversionTarget::Number
            })
        ));

        let static_string = convert_target_with_origin(
            &InvocationValue::static_value(IrValue::String("*text*".into())),
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
                &InvocationValue::dynamic_value(IrValue::String("*text*".into())),
                ConversionTarget::InlineContent,
                span(),
            ),
            Ok(TargetValue::RawMarkdown {
                target: RawMarkdownTarget::Inline,
                text: "*text*".into(),
            })
        );
        assert!(matches!(
            convert_target_with_origin(
                &InvocationValue::static_value(IrValue::String("text".into())),
                ConversionTarget::BlockContent,
                span(),
            ),
            Err(ConversionError::UnsupportedValue {
                target: ConversionTarget::BlockContent
            })
        ));
        for value in [IrValue::Number(2.0), IrValue::Boolean(true)] {
            assert!(matches!(
                convert_target_with_origin(
                    &InvocationValue::static_value(value),
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
            main_axis_alignment: IrMainAxisAlignment::Start,
            cross_axis_alignment: IrCrossAxisAlignment::Stretch,
            row_gap: None,
            column_gap: None,
            children: Vec::new(),
            span: span(),
        }));
        assert_eq!(
            convert_target_with_origin(
                &InvocationValue::static_value(component.clone()),
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
                &InvocationValue::static_value(inline_body),
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
                &InvocationValue::static_value(pair),
                ConversionTarget::Iterable,
                span(),
            ),
            Ok(TargetValue::Value(IrValue::Pair(_)))
        ));

        let dictionary = IrValue::Dictionary(scribium_ir::IrDictionary {
            entries: Vec::new(),
            span: span(),
        });
        assert!(matches!(
            convert_target_with_origin(
                &InvocationValue::static_value(dictionary.clone()),
                ConversionTarget::Dictionary,
                span(),
            ),
            Ok(TargetValue::Value(IrValue::Dictionary(_)))
        ));
        assert!(matches!(
            convert_target_with_origin(
                &InvocationValue::dynamic_value(IrValue::String("- key: value".into())),
                ConversionTarget::Dictionary,
                span(),
            ),
            Ok(TargetValue::RawMarkdown {
                target: RawMarkdownTarget::Dictionary,
                ..
            })
        ));

        for target in [
            ConversionTarget::Number,
            ConversionTarget::InlineContent,
            ConversionTarget::BlockContent,
            ConversionTarget::Node,
            ConversionTarget::Iterable,
            ConversionTarget::Dictionary,
            ConversionTarget::Callable,
        ] {
            assert!(
                convert_target_with_origin(
                    &InvocationValue::static_value(IrValue::None),
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
        let static_number = InvocationValue::static_value(IrValue::String("-3.5".into()));
        let dynamic_number = InvocationValue::dynamic_value(IrValue::String("-3.5".into()));
        assert!(convert_scalar_with_origin(&static_number, ScalarTarget::Number).is_err());
        assert_eq!(
            convert_scalar_with_origin(&dynamic_number, ScalarTarget::Number),
            Ok(ScalarValue::Number(-3.5))
        );

        let static_boolean = InvocationValue::static_value(IrValue::String("YES".into()));
        let dynamic_boolean = InvocationValue::dynamic_value(IrValue::String("YES".into()));
        assert!(convert_scalar_with_origin(&static_boolean, ScalarTarget::Boolean).is_err());
        assert_eq!(
            convert_scalar_with_origin(&dynamic_boolean, ScalarTarget::Boolean),
            Ok(ScalarValue::Boolean(true))
        );

        let static_range = InvocationValue::static_value(IrValue::String("2..4".into()));
        let dynamic_range = InvocationValue::dynamic_value(IrValue::String("2..4".into()));
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
            main_axis_alignment: IrMainAxisAlignment::Start,
            cross_axis_alignment: IrCrossAxisAlignment::Stretch,
            row_gap: None,
            column_gap: None,
            children: Vec::new(),
            span: span(),
        }));
        let argument = InvocationValue::dynamic_value(component);

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
                    &InvocationValue::dynamic_value(IrValue::String(text.into())),
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
                    &InvocationValue::dynamic_value(IrValue::String(text.into())),
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
                &InvocationValue::static_value(IrValue::Size(size.clone())),
                DomainTarget::Size,
            ),
            Ok(DomainValue::Size(size))
        );
        assert!(convert_domain_with_origin(
            &InvocationValue::static_value(IrValue::String("10px".into())),
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
                &InvocationValue::static_value(IrValue::Color(color.clone())),
                DomainTarget::Color,
            ),
            Ok(DomainValue::Color(color))
        );
        assert!(convert_domain_with_origin(
            &InvocationValue::static_value(IrValue::String("red".into())),
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
                    &InvocationValue::dynamic_value(IrValue::String(text.into())),
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
                    &InvocationValue::dynamic_value(IrValue::String(text.into())),
                    DomainTarget::Color,
                )
                .is_err(),
                "unexpectedly accepted {text:?}"
            );
        }
    }

    #[test]
    fn closed_enum_conversion_preserves_domain_and_applies_only_declared_name_normalization() {
        for text in ["plain", "PLAIN", "Plain"] {
            assert_eq!(
                convert_domain_with_origin(
                    &InvocationValue::dynamic_value(IrValue::String(text.into())),
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
                &InvocationValue::static_value(IrValue::Enum(typed)),
                DomainTarget::ClosedEnum(ClosedEnumTarget::DocumentType),
            ),
            Ok(DomainValue::Enum(typed))
        );
        assert!(convert_domain_with_origin(
            &InvocationValue::static_value(IrValue::String("paged".into())),
            DomainTarget::ClosedEnum(ClosedEnumTarget::DocumentType),
        )
        .is_err());
        assert!(convert_domain_with_origin(
            &InvocationValue::dynamic_value(IrValue::String("page_d".into())),
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
                    &InvocationValue::dynamic_value(IrValue::Identifier(text.into())),
                    DomainTarget::ClosedEnum(ClosedEnumTarget::CaptionPosition),
                ),
                Ok(DomainValue::Enum(IrEnumValue::CaptionPosition(expected)))
            );
        }

        let typed = IrEnumValue::CaptionPosition(IrCaptionPosition::Top);
        assert_eq!(
            convert_domain_with_origin(
                &InvocationValue::static_value(IrValue::Enum(typed)),
                DomainTarget::ClosedEnum(ClosedEnumTarget::CaptionPosition),
            ),
            Ok(DomainValue::Enum(typed))
        );
        assert!(matches!(
            convert_domain_with_origin(
                &InvocationValue::dynamic_value(IrValue::String("middle".into())),
                DomainTarget::ClosedEnum(ClosedEnumTarget::CaptionPosition),
            ),
            Err(ConversionError::InvalidText {
                target: ConversionTarget::Enum
            })
        ));
        assert!(matches!(
            convert_domain_with_origin(
                &InvocationValue::static_value(IrValue::String("top".into())),
                DomainTarget::ClosedEnum(ClosedEnumTarget::CaptionPosition),
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
                    &InvocationValue::dynamic_value(IrValue::String(text.into())),
                    DomainTarget::ClosedEnum(ClosedEnumTarget::StackedMainAxisAlignment),
                ),
                Ok(DomainValue::Enum(IrEnumValue::StackedMainAxisAlignment(_)))
            ));
        }
        for text in ["start", "center", "end", "stretch"] {
            assert!(matches!(
                convert_domain_with_origin(
                    &InvocationValue::dynamic_value(IrValue::String(text.into())),
                    DomainTarget::ClosedEnum(ClosedEnumTarget::StackedCrossAxisAlignment),
                ),
                Ok(DomainValue::Enum(IrEnumValue::StackedCrossAxisAlignment(_)))
            ));
        }
        assert!(convert_domain_with_origin(
            &InvocationValue::dynamic_value(IrValue::String("spacebetween".into())),
            DomainTarget::ClosedEnum(ClosedEnumTarget::StackedCrossAxisAlignment),
        )
        .is_err());
        assert!(convert_domain_with_origin(
            &InvocationValue::static_value(IrValue::Enum(IrEnumValue::StackedMainAxisAlignment(
                IrMainAxisAlignment::Center
            ),)),
            DomainTarget::ClosedEnum(ClosedEnumTarget::StackedCrossAxisAlignment),
        )
        .is_err());

        for value in [
            InvocationValue::dynamic_value(IrValue::Number(2.0)),
            InvocationValue::dynamic_value(IrValue::String("2".into())),
            InvocationValue::dynamic_value(IrValue::String("2.0".into())),
        ] {
            assert_eq!(convert_integer_with_origin(&value), Ok(2));
        }
        assert_eq!(
            convert_integer_with_origin(&InvocationValue::dynamic_value(IrValue::Number(-1.0))),
            Ok(-1)
        );
        assert!(matches!(
            convert_integer_with_origin(&InvocationValue::dynamic_value(IrValue::Number(2.5))),
            Err(ConversionError::InvalidText {
                target: ConversionTarget::Integer
            })
        ));
        assert!(matches!(
            convert_integer_with_origin(&InvocationValue::static_value(IrValue::String(
                "2".into()
            ))),
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
                    &InvocationValue::dynamic_value(IrValue::String(text.into())),
                    DomainTarget::ClosedEnum(ClosedEnumTarget::ContainerAlignment),
                ),
                Ok(DomainValue::Enum(IrEnumValue::ContainerAlignment(expected)))
            );
        }

        let typed = IrEnumValue::ContainerAlignment(IrContainerAlignment::Center);
        assert_eq!(
            convert_domain_with_origin(
                &InvocationValue::static_value(IrValue::Enum(typed)),
                DomainTarget::ClosedEnum(ClosedEnumTarget::ContainerAlignment),
            ),
            Ok(DomainValue::Enum(typed))
        );
        assert!(matches!(
            convert_domain_with_origin(
                &InvocationValue::static_value(IrValue::String("center".into())),
                DomainTarget::ClosedEnum(ClosedEnumTarget::ContainerAlignment),
            ),
            Err(ConversionError::UnsupportedValue {
                target: ConversionTarget::Enum
            })
        ));
        assert!(matches!(
            convert_domain_with_origin(
                &InvocationValue::dynamic_value(IrValue::String("middle".into())),
                DomainTarget::ClosedEnum(ClosedEnumTarget::ContainerAlignment),
            ),
            Err(ConversionError::InvalidText {
                target: ConversionTarget::Enum
            })
        ));
        assert!(matches!(
            convert_domain_with_origin(
                &InvocationValue::static_value(IrValue::Enum(
                    IrEnumValue::StackedMainAxisAlignment(IrMainAxisAlignment::Center)
                )),
                DomainTarget::ClosedEnum(ClosedEnumTarget::ContainerAlignment),
            ),
            Err(ConversionError::UnsupportedValue {
                target: ConversionTarget::Enum
            })
        ));
    }
}
