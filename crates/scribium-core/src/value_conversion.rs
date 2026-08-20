//! Bounded, context-free value conversion used by the current evaluator slice.
//!
//! This module deliberately does not model Quarkdown's complete dynamic value
//! hierarchy. It contains only the scalar conversion boundaries that have a
//! concrete Scribium consumer today. Context-sensitive Markdown conversion,
//! collection construction, callable conversion, and layout values remain
//! outside this policy.

use crate::ir::{IrRange, IrValue};
use crate::source::SourceSpan;

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
pub(crate) fn convert_scalar(
    value: &IrValue,
    target: ScalarTarget,
) -> Result<ScalarValue, ConversionError> {
    match target {
        ScalarTarget::Number => match value {
            IrValue::Number(value) => Ok(ScalarValue::Number(*value)),
            IrValue::String(value) | IrValue::Identifier(value) => parse_number(value)
                .map(ScalarValue::Number)
                .ok_or(ConversionError::InvalidText {
                    target: target.into(),
                }),
            _ => Err(ConversionError::UnsupportedValue {
                target: target.into(),
            }),
        },
        ScalarTarget::Boolean => match value {
            IrValue::Boolean(value) => Ok(ScalarValue::Boolean(*value)),
            IrValue::String(value) | IrValue::Identifier(value) => parse_boolean(value)
                .map(ScalarValue::Boolean)
                .ok_or(ConversionError::InvalidText {
                    target: target.into(),
                }),
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
            | IrValue::Callable(_) => Err(ConversionError::UnsupportedValue {
                target: target.into(),
            }),
        },
    }
}

/// Converts an existing or textual range while preserving the source span of
/// an existing typed range or attaching the caller's reliable argument span
/// to a newly parsed textual range.
pub(crate) fn convert_range(value: &IrValue, span: SourceSpan) -> Result<IrRange, ConversionError> {
    match value {
        IrValue::Range(range) => Ok(range.clone()),
        IrValue::String(value) | IrValue::Identifier(value) => {
            parse_range(value, span).ok_or(ConversionError::InvalidText {
                target: ConversionTarget::Range,
            })
        }
        _ => Err(ConversionError::UnsupportedValue {
            target: ConversionTarget::Range,
        }),
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
        convert_range, convert_scalar, ConversionError, ConversionTarget, ScalarTarget, ScalarValue,
    };
    use crate::ir::{IrRange, IrValue};
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
}
