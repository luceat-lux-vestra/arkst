//! Bounded, context-free value conversion used by the current evaluator slice.
//!
//! This module deliberately does not model Quarkdown's complete dynamic value
//! hierarchy. It contains the scalar conversion boundaries and the bounded
//! domain adapters that have a concrete Scribium consumer or an explicit live
//! evaluator path today. Context-sensitive Markdown conversion, collection
//! construction, callable conversion, and layout consumers remain outside this
//! policy.

use crate::ir::{
    IrColor, IrDocumentType, IrEnumValue, IrNamedArg, IrRange, IrSize, IrSizeUnit, IrValue,
};
use crate::source::SourceSpan;
use std::ops::Deref;

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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConversionTarget {
    Number,
    Boolean,
    String,
    Range,
    Size,
    Color,
    Enum,
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

/// A source-independent conversion failure.
///
/// Callers add the parameter/builtin name and the reliable call or argument
/// span when turning this into a user-facing diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConversionError {
    InvalidText { target: ConversionTarget },
    UnsupportedValue { target: ConversionTarget },
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
            | IrValue::Size(_)
            | IrValue::Color(_)
            | IrValue::Enum(_) => Err(ConversionError::UnsupportedValue {
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
        },
    }
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
        6 => Some(IrColor {
            red: hex_byte(digits.get(0..2)?)?,
            green: hex_byte(digits.get(2..4)?)?,
            blue: hex_byte(digits.get(4..6)?)?,
            alpha: 1.0,
        }),
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
        convert_domain_with_origin, convert_range, convert_range_with_origin, convert_scalar,
        convert_scalar_with_origin, ClosedEnumSpec, ClosedEnumTarget, ClosedEnumVariant,
        ConversionError, ConversionTarget, DomainTarget, DomainValue, InvocationValue,
        ScalarTarget, ScalarValue,
    };
    use crate::ir::{IrColor, IrDocumentType, IrEnumValue, IrRange, IrSize, IrSizeUnit, IrValue};
    use crate::source::{SourceId, SourceSpan};

    fn span() -> SourceSpan {
        SourceSpan::new(SourceId(7), 10, 16)
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
            "#1234",
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
}
