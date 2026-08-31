//! Small, deterministic evaluator builtins used by the current semantic slice.

use crate::invocation_binder::BodyPolicy;
#[cfg(test)]
use crate::invocation_binder::{self, Candidate};
use crate::invocation_binder::{BoundInvocation, BoundSlot, ParameterMetadata};
#[cfg(test)]
use crate::value_conversion::InvocationNamedArg;
#[cfg(test)]
use crate::value_conversion::ValueOrigin;
use crate::value_conversion::{self, InvocationValue, ScalarTarget, ScalarValue};
use scribium_ir::{IrInline, IrNode, IrValue};
#[cfg(test)]
use scribium_source::SourceId;
use scribium_source::SourceSpan;
use unicode_case_mapping::{to_lowercase, to_titlecase, to_uppercase, UNICODE_VERSION};

// Quarkdown v2.5.1 is pinned to a JVM 17 runtime, whose Character mappings
// use Unicode 13.0. Keep the generated mapping table aligned with that
// contract at compile time rather than allowing a dependency upgrade to
// silently widen the supported character set.
const PINNED_JVM_UNICODE_VERSION: (u64, u64, u64) = (13, 0, 0);
const _: () = {
    assert!(UNICODE_VERSION.0 == PINNED_JVM_UNICODE_VERSION.0);
    assert!(UNICODE_VERSION.1 == PINNED_JVM_UNICODE_VERSION.1);
    assert!(UNICODE_VERSION.2 == PINNED_JVM_UNICODE_VERSION.2);
};

#[cfg(test)]
type Arguments<'a> = &'a [InvocationValue];
#[cfg(test)]
type NamedArguments<'a> = &'a [InvocationNamedArg];
type BoundArguments = Vec<Option<InvocationValue>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BuiltinError {
    pub message: String,
    pub(crate) conversion: Option<BuiltinConversionFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BuiltinConversionFailure {
    pub(crate) parameter: String,
    pub(crate) error: value_conversion::ConversionError,
    pub(crate) candidate_span: Option<SourceSpan>,
    pub(crate) parameter_span: Option<SourceSpan>,
}

impl BuiltinError {
    fn with_message(mut self, message: String) -> Self {
        self.message = message;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BuiltinKind {
    Sum,
    Subtract,
    Multiply,
    Divide,
    Rem,
    Pow,
    Abs,
    Negate,
    Sqrt,
    Logn,
    Pi,
    Sin,
    Cos,
    Tan,
    Truncate,
    Round,
    IsEven,
    String,
    Concatenate,
    Uppercase,
    Lowercase,
    Capitalize,
    IsEmpty,
    IsNotEmpty,
    StartsWith,
    Plaintext,
    None,
    Otherwise,
    IsNone,
    IsLower,
    IsGreater,
    Equals,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BuiltinBodyPolicy {
    Reject,
    /// A block body is the final parameter's raw, source-backed dynamic
    /// candidate. The evaluator must not evaluate or stringify its parsed
    /// representation before target conversion.
    BindRaw,
    BindEvaluatedContent,
}

impl BuiltinBodyPolicy {
    pub(crate) const fn binder_policy(self) -> BodyPolicy {
        match self {
            Self::Reject => BodyPolicy::Reject,
            Self::BindRaw | Self::BindEvaluatedContent => BodyPolicy::BindFinal,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BuiltinSignature {
    pub(crate) parameter_names: &'static [&'static str],
    pub(crate) defaulted_parameters: &'static [usize],
    pub(crate) max_positional: usize,
    pub(crate) allows_named: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BuiltinSpec {
    pub(crate) name: &'static str,
    pub(crate) kind: BuiltinKind,
    pub(crate) signature: BuiltinSignature,
    pub(crate) body_policy: BuiltinBodyPolicy,
}

const fn builtin_spec(
    name: &'static str,
    kind: BuiltinKind,
    parameter_names: &'static [&'static str],
    max_positional: usize,
    allows_named: bool,
    body_policy: BuiltinBodyPolicy,
) -> BuiltinSpec {
    builtin_spec_with_defaults(
        name,
        kind,
        parameter_names,
        &[],
        max_positional,
        allows_named,
        body_policy,
    )
}

const fn builtin_spec_with_defaults(
    name: &'static str,
    kind: BuiltinKind,
    parameter_names: &'static [&'static str],
    defaulted_parameters: &'static [usize],
    max_positional: usize,
    allows_named: bool,
    body_policy: BuiltinBodyPolicy,
) -> BuiltinSpec {
    BuiltinSpec {
        name,
        kind,
        signature: BuiltinSignature {
            parameter_names,
            defaulted_parameters,
            max_positional,
            allows_named,
        },
        body_policy,
    }
}

/// The complete regular scalar builtin inventory.
///
/// Names, signatures, body policy, and dispatch identity are deliberately
/// stored together. Bespoke evaluator-owned native calls are not represented
/// here; their ownership remains explicit in `evaluator.rs`.
static REGULAR_BUILTINS: &[BuiltinSpec] = &[
    builtin_spec(
        "sum",
        BuiltinKind::Sum,
        &["a", "b"],
        2,
        true,
        BuiltinBodyPolicy::BindRaw,
    ),
    builtin_spec(
        "subtract",
        BuiltinKind::Subtract,
        &["a", "b"],
        2,
        true,
        BuiltinBodyPolicy::BindRaw,
    ),
    builtin_spec(
        "multiply",
        BuiltinKind::Multiply,
        &["a", "by"],
        2,
        true,
        BuiltinBodyPolicy::BindRaw,
    ),
    builtin_spec(
        "divide",
        BuiltinKind::Divide,
        &["a", "by"],
        2,
        true,
        BuiltinBodyPolicy::BindRaw,
    ),
    builtin_spec(
        "rem",
        BuiltinKind::Rem,
        &["a", "b"],
        2,
        true,
        BuiltinBodyPolicy::BindRaw,
    ),
    builtin_spec(
        "pow",
        BuiltinKind::Pow,
        &["base", "to"],
        2,
        true,
        BuiltinBodyPolicy::BindRaw,
    ),
    builtin_spec(
        "abs",
        BuiltinKind::Abs,
        &["x"],
        1,
        true,
        BuiltinBodyPolicy::BindRaw,
    ),
    builtin_spec(
        "negate",
        BuiltinKind::Negate,
        &["x"],
        1,
        true,
        BuiltinBodyPolicy::BindRaw,
    ),
    builtin_spec(
        "sqrt",
        BuiltinKind::Sqrt,
        &["x"],
        1,
        true,
        BuiltinBodyPolicy::BindRaw,
    ),
    builtin_spec(
        "logn",
        BuiltinKind::Logn,
        &["x"],
        1,
        true,
        BuiltinBodyPolicy::BindRaw,
    ),
    builtin_spec(
        "pi",
        BuiltinKind::Pi,
        &[],
        0,
        true,
        BuiltinBodyPolicy::Reject,
    ),
    builtin_spec(
        "sin",
        BuiltinKind::Sin,
        &["x"],
        1,
        true,
        BuiltinBodyPolicy::BindRaw,
    ),
    builtin_spec(
        "cos",
        BuiltinKind::Cos,
        &["x"],
        1,
        true,
        BuiltinBodyPolicy::BindRaw,
    ),
    builtin_spec(
        "tan",
        BuiltinKind::Tan,
        &["x"],
        1,
        true,
        BuiltinBodyPolicy::BindRaw,
    ),
    builtin_spec(
        "truncate",
        BuiltinKind::Truncate,
        &["x", "decimals"],
        2,
        true,
        BuiltinBodyPolicy::BindRaw,
    ),
    builtin_spec(
        "round",
        BuiltinKind::Round,
        &["x"],
        1,
        true,
        BuiltinBodyPolicy::BindRaw,
    ),
    builtin_spec(
        "iseven",
        BuiltinKind::IsEven,
        &["x"],
        1,
        true,
        BuiltinBodyPolicy::BindRaw,
    ),
    builtin_spec(
        "string",
        BuiltinKind::String,
        &["value"],
        1,
        true,
        BuiltinBodyPolicy::BindRaw,
    ),
    builtin_spec_with_defaults(
        "concatenate",
        BuiltinKind::Concatenate,
        &["a", "with", "if"],
        &[2],
        3,
        true,
        BuiltinBodyPolicy::BindRaw,
    ),
    builtin_spec(
        "uppercase",
        BuiltinKind::Uppercase,
        &["string"],
        1,
        true,
        BuiltinBodyPolicy::BindRaw,
    ),
    builtin_spec(
        "lowercase",
        BuiltinKind::Lowercase,
        &["string"],
        1,
        true,
        BuiltinBodyPolicy::BindRaw,
    ),
    builtin_spec(
        "capitalize",
        BuiltinKind::Capitalize,
        &["string"],
        1,
        true,
        BuiltinBodyPolicy::BindRaw,
    ),
    builtin_spec(
        "isempty",
        BuiltinKind::IsEmpty,
        &["string"],
        1,
        true,
        BuiltinBodyPolicy::BindRaw,
    ),
    builtin_spec(
        "isnotempty",
        BuiltinKind::IsNotEmpty,
        &["string"],
        1,
        true,
        BuiltinBodyPolicy::BindRaw,
    ),
    builtin_spec_with_defaults(
        "startswith",
        BuiltinKind::StartsWith,
        &["string", "prefix", "ignorecase"],
        &[2],
        3,
        true,
        BuiltinBodyPolicy::BindRaw,
    ),
    builtin_spec(
        "plaintext",
        BuiltinKind::Plaintext,
        &["content"],
        1,
        true,
        BuiltinBodyPolicy::BindEvaluatedContent,
    ),
    builtin_spec(
        "none",
        BuiltinKind::None,
        &[],
        0,
        false,
        BuiltinBodyPolicy::Reject,
    ),
    builtin_spec(
        "otherwise",
        BuiltinKind::Otherwise,
        &["value", "fallback"],
        2,
        false,
        BuiltinBodyPolicy::BindRaw,
    ),
    builtin_spec(
        "isnone",
        BuiltinKind::IsNone,
        &["value"],
        1,
        false,
        BuiltinBodyPolicy::BindRaw,
    ),
    builtin_spec_with_defaults(
        "islower",
        BuiltinKind::IsLower,
        &["a", "than", "orequals"],
        &[2],
        3,
        true,
        BuiltinBodyPolicy::BindRaw,
    ),
    builtin_spec_with_defaults(
        "isgreater",
        BuiltinKind::IsGreater,
        &["a", "than", "orequals"],
        &[2],
        3,
        true,
        BuiltinBodyPolicy::BindRaw,
    ),
    builtin_spec(
        "equals",
        BuiltinKind::Equals,
        &["a", "to"],
        2,
        true,
        BuiltinBodyPolicy::BindRaw,
    ),
    builtin_spec(
        "not",
        BuiltinKind::Not,
        &["value"],
        1,
        true,
        BuiltinBodyPolicy::BindRaw,
    ),
];

#[cfg(test)]
pub(crate) fn regular_builtins() -> &'static [BuiltinSpec] {
    REGULAR_BUILTINS
}

pub(crate) fn lookup(name: &str) -> Option<&'static BuiltinSpec> {
    REGULAR_BUILTINS.iter().find(|builtin| builtin.name == name)
}

/// Returns the explicit slot metadata shared with the engine binder. The
/// regular builtin inventory remains the single owner of parameter names,
/// named-argument capability, and omission policy.
pub(crate) fn binding_parameters(builtin: &BuiltinSpec) -> Vec<ParameterMetadata<'static>> {
    builtin
        .signature
        .parameter_names
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let parameter = if builtin.signature.defaulted_parameters.contains(&index) {
                ParameterMetadata::defaulted(name)
            } else {
                ParameterMetadata::required(name)
            };
            parameter.named(builtin.signature.allows_named)
        })
        .collect()
}

/// Evaluates one supported builtin without source or backend conversion.
#[cfg(test)]
pub(crate) fn evaluate(
    name: &str,
    positional_args: &[IrValue],
    named_args: &[scribium_ir::IrNamedArg],
    has_body: bool,
) -> Result<IrValue, BuiltinError> {
    let positional = positional_args
        .iter()
        .cloned()
        .map(InvocationValue::dynamic_value)
        .collect::<Vec<_>>();
    let named = named_args
        .iter()
        .cloned()
        .map(|arg| InvocationNamedArg::new(arg, ValueOrigin::Dynamic))
        .collect::<Vec<_>>();
    let builtin =
        lookup(name).ok_or_else(|| error(format!("`.{name}` has no builtin implementation")))?;
    evaluate_with_origins(builtin, &positional, &named, has_body)
}

/// Evaluates a builtin with the invocation-time DynamicValue distinction
/// preserved by the evaluator. The public-to-the-crate `evaluate` wrapper
/// above remains useful for focused builtin tests, where raw arguments model
/// Quarkdown's dynamic argument boundary.
#[cfg(test)]
pub(crate) fn evaluate_with_origins(
    builtin: &BuiltinSpec,
    positional_args: Arguments<'_>,
    named_args: NamedArguments<'_>,
    has_body: bool,
) -> Result<IrValue, BuiltinError> {
    let parameters = binding_parameters(builtin);
    let fallback_span = SourceSpan::new(SourceId(0), 0, 0);
    let mut candidates = Vec::with_capacity(positional_args.len() + named_args.len());
    candidates.extend(
        positional_args
            .iter()
            .cloned()
            .map(|value| Candidate::Positional {
                value,
                span: fallback_span,
            }),
    );
    candidates.extend(named_args.iter().map(|argument| Candidate::Named {
        name: argument.name.clone(),
        name_span: argument.name_span,
        value: InvocationValue {
            value: argument.arg.value.clone(),
            origin: argument.origin,
        },
        span: argument.arg.span,
    }));
    let body = has_body.then_some(Candidate::Positional {
        value: InvocationValue::static_value(IrValue::None),
        span: fallback_span,
    });
    let plan = invocation_binder::plan(
        &parameters,
        &candidates,
        body.as_ref(),
        builtin.body_policy.binder_policy(),
        fallback_span,
    )
    .map_err(|failure| error(format!("`.{}` {}", builtin.name, failure.message)))?;
    let bound = plan
        .bind(&candidates, body.as_ref(), fallback_span)
        .map_err(|failure| error(format!("`.{}` {}", builtin.name, failure.message)))?;
    evaluate_bound(builtin, bound)
}

/// Evaluates a builtin after the engine-owned binder has selected every slot.
/// Native handlers receive only target-specific values; they do not perform
/// positional/named assignment or argument-count validation.
pub(crate) fn evaluate_bound(
    builtin: &BuiltinSpec,
    bound: BoundInvocation<InvocationValue>,
) -> Result<IrValue, BuiltinError> {
    let candidate_spans = bound
        .slots
        .iter()
        .map(|slot| match slot {
            BoundSlot::Explicit { span, .. } => Some(*span),
            BoundSlot::Omitted | BoundSlot::Defaulted => None,
        })
        .collect::<Vec<_>>();
    let parameter_spans = bound
        .parameters
        .iter()
        .map(|parameter| parameter.name_span)
        .collect::<Vec<_>>();
    let arguments = bound
        .slots
        .into_iter()
        .map(|slot| match slot {
            BoundSlot::Explicit { value, .. } => Some(value),
            BoundSlot::Omitted | BoundSlot::Defaulted => None,
        })
        .collect::<BoundArguments>();
    let result = match builtin.kind {
        BuiltinKind::Sum
        | BuiltinKind::Subtract
        | BuiltinKind::Multiply
        | BuiltinKind::Divide
        | BuiltinKind::Rem
        | BuiltinKind::Pow => evaluate_numeric(builtin, arguments),
        BuiltinKind::Abs | BuiltinKind::Negate | BuiltinKind::Sqrt | BuiltinKind::IsEven => {
            evaluate_unary_numeric(builtin, arguments)
        }
        BuiltinKind::Logn | BuiltinKind::Sin | BuiltinKind::Cos | BuiltinKind::Tan => {
            evaluate_transcendental(builtin, arguments)
        }
        BuiltinKind::Pi => evaluate_pi(builtin, arguments),
        BuiltinKind::Truncate => evaluate_truncate(builtin, arguments),
        BuiltinKind::Round => evaluate_round(builtin, arguments),
        BuiltinKind::String => evaluate_string(builtin, arguments),
        BuiltinKind::Concatenate => evaluate_concatenate(builtin, arguments),
        BuiltinKind::Uppercase | BuiltinKind::Lowercase | BuiltinKind::Capitalize => {
            evaluate_case(builtin, arguments)
        }
        BuiltinKind::IsEmpty | BuiltinKind::IsNotEmpty => evaluate_empty_check(builtin, arguments),
        BuiltinKind::StartsWith => evaluate_startswith(builtin, arguments),
        BuiltinKind::Plaintext => evaluate_plaintext(builtin, arguments),
        BuiltinKind::None => evaluate_none(builtin, arguments),
        BuiltinKind::Otherwise => evaluate_otherwise(builtin, arguments),
        BuiltinKind::IsNone => evaluate_isnone(builtin, arguments),
        BuiltinKind::IsLower | BuiltinKind::IsGreater => evaluate_ordering(builtin, arguments),
        BuiltinKind::Equals => evaluate_equals(builtin, arguments),
        BuiltinKind::Not => evaluate_not(builtin, arguments),
    };
    result.map_err(|mut error| {
        if let Some(conversion) = error.conversion.as_mut() {
            let parameter_index = builtin
                .signature
                .parameter_names
                .iter()
                .position(|name| *name == conversion.parameter);
            conversion.candidate_span =
                parameter_index.and_then(|index| candidate_spans.get(index).copied().flatten());
            conversion.parameter_span =
                parameter_index.and_then(|index| parameter_spans.get(index).copied().flatten());
        }
        error
    })
}

fn evaluate_ordering(
    builtin: &BuiltinSpec,
    mut arguments: BoundArguments,
) -> Result<IrValue, BuiltinError> {
    let name = builtin.name;

    let a = arguments
        .remove(0)
        .ok_or_else(|| error(format!("`.{name}` requires a first numeric argument")))?;
    let b = arguments
        .remove(0)
        .ok_or_else(|| error(format!("`.{name}` requires a `than` numeric argument")))?;
    let orequals = arguments
        .remove(0)
        .map(|value| boolean_argument(&value, "orequals"))
        .transpose()?
        .unwrap_or(false);
    let a = numeric_argument(&a, "a")?;
    let b = numeric_argument(&b, "than")?;
    let result = if builtin.kind == BuiltinKind::IsLower {
        if orequals {
            a <= b
        } else {
            a < b
        }
    } else if orequals {
        a >= b
    } else {
        a > b
    };
    Ok(IrValue::Boolean(result))
}

fn evaluate_equals(
    _builtin: &BuiltinSpec,
    mut arguments: BoundArguments,
) -> Result<IrValue, BuiltinError> {
    let left = arguments
        .remove(0)
        .ok_or_else(|| error("`.equals` requires a first value argument".to_string()))?;
    let right = arguments
        .remove(0)
        .ok_or_else(|| error("`.equals` requires a `to` value argument".to_string()))?;
    Ok(IrValue::Boolean(values_equal(&left, &right)))
}

fn evaluate_not(
    _builtin: &BuiltinSpec,
    mut arguments: BoundArguments,
) -> Result<IrValue, BuiltinError> {
    let value = arguments
        .remove(0)
        .ok_or_else(|| error("`.not` requires exactly one boolean argument".to_string()))?;
    Ok(IrValue::Boolean(!boolean_argument(&value, "value")?))
}

fn evaluate_string(
    _builtin: &BuiltinSpec,
    mut arguments: BoundArguments,
) -> Result<IrValue, BuiltinError> {
    let value = arguments
        .remove(0)
        .ok_or_else(|| error("`.string` requires one value argument".to_string()))?;
    let text = scalar_string_argument_result(&value, "value").map_err(|error| {
        error.with_message("`.string` requires a scalar value that can adapt to text".to_string())
    })?;
    Ok(IrValue::String(text))
}

fn evaluate_concatenate(
    _builtin: &BuiltinSpec,
    mut arguments: BoundArguments,
) -> Result<IrValue, BuiltinError> {
    let a = arguments
        .remove(0)
        .ok_or_else(|| error("`.concatenate` requires an `a` string argument".to_string()))?;
    let with = arguments
        .remove(0)
        .ok_or_else(|| error("`.concatenate` requires a `with` string argument".to_string()))?;
    let condition = arguments
        .remove(0)
        .map(|value| boolean_argument(&value, "if"))
        .transpose()?
        .unwrap_or(true);
    let a = scalar_string_argument_result(&a, "a").map_err(|error| {
        error.with_message("`.concatenate` argument `a` cannot adapt to text".to_string())
    })?;
    let with = scalar_string_argument_result(&with, "with").map_err(|error| {
        error.with_message("`.concatenate` argument `with` cannot adapt to text".to_string())
    })?;
    if condition {
        Ok(IrValue::String(format!("{a}{with}")))
    } else {
        Ok(IrValue::String(a))
    }
}

fn evaluate_empty_check(
    builtin: &BuiltinSpec,
    mut arguments: BoundArguments,
) -> Result<IrValue, BuiltinError> {
    let name = builtin.name;
    let value = arguments
        .remove(0)
        .ok_or_else(|| error(format!("`.{name}` requires one string argument")))?;
    let text = scalar_string_argument_result(&value, "string").map_err(|error| {
        error.with_message(format!(
            "`.{name}` requires a scalar value that can adapt to text"
        ))
    })?;
    let is_empty = text.is_empty();
    Ok(IrValue::Boolean(if builtin.kind == BuiltinKind::IsEmpty {
        is_empty
    } else {
        !is_empty
    }))
}

fn evaluate_startswith(
    _builtin: &BuiltinSpec,
    mut arguments: BoundArguments,
) -> Result<IrValue, BuiltinError> {
    let string = arguments
        .remove(0)
        .ok_or_else(|| error("`.startswith` requires a `string` argument".to_string()))?;
    let prefix = arguments
        .remove(0)
        .ok_or_else(|| error("`.startswith` requires a `prefix` argument".to_string()))?;
    let ignorecase = arguments
        .remove(0)
        .map(|value| boolean_argument(&value, "ignorecase"))
        .transpose()?
        .unwrap_or(false);
    let string = scalar_string_argument_result(&string, "string").map_err(|error| {
        error.with_message("`.startswith` argument `string` cannot adapt to text".to_string())
    })?;
    let prefix = scalar_string_argument_result(&prefix, "prefix").map_err(|error| {
        error.with_message("`.startswith` argument `prefix` cannot adapt to text".to_string())
    })?;
    let result = if ignorecase {
        starts_with_ignore_case(&string, &prefix)
    } else {
        string.starts_with(&prefix)
    };
    Ok(IrValue::Boolean(result))
}

/// Reproduces the pinned JVM `String.startsWith(prefix, ignoreCase)` contract:
/// a prefix is compared one Unicode character at a time using the pinned
/// JVM's simple case relation. For valid UTF-8, this is equivalent to the
/// JVM's code-point-aware `regionMatches` path. In particular, this does not
/// apply full-string case conversion or Unicode normalization.
fn starts_with_ignore_case(string: &str, prefix: &str) -> bool {
    let mut string_characters = string.chars();
    for prefix_character in prefix.chars() {
        let Some(string_character) = string_characters.next() else {
            return false;
        };
        if !kotlin_char_equals(string_character, prefix_character) {
            return false;
        }
    }
    true
}

/// Kotlin/JVM's case-insensitive character comparison compares the simple
/// uppercase forms first, then the simple lowercase forms of those results.
fn kotlin_char_equals(left: char, right: char) -> bool {
    if left == right {
        return true;
    }

    let left_upper = simple_uppercase(left);
    let right_upper = simple_uppercase(right);
    left_upper == right_upper || simple_lowercase(left_upper) == simple_lowercase(right_upper)
}

/// The Unicode mapping crate exposes full mappings, while Kotlin's
/// `uppercaseChar` is explicitly one-to-one. A multi-code-point uppercase
/// mapping therefore has no simple uppercase equivalent and retains the
/// original character for this comparison.
fn simple_uppercase(character: char) -> char {
    let mapping = to_uppercase(character);
    if mapping[0] != 0 && mapping[1] == 0 {
        if let Some(mapped) = char::from_u32(mapping[0]) {
            return mapped;
        }
    }
    character
}

/// Return the one-to-one lowercase mapping used by Kotlin's
/// `lowercaseChar`. The pinned Unicode data has one invariant multi-code-point
/// lowercase mapping (`İ`), whose first scalar is its JVM simple mapping.
fn simple_lowercase(character: char) -> char {
    let mapping = to_lowercase(character);
    if mapping[0] != 0 {
        if let Some(mapped) = char::from_u32(mapping[0]) {
            return mapped;
        }
    }
    character
}

/// Apply Kotlin's `Char.titlecase()` mapping, preserving the distinction from
/// uppercase and retaining all original trailing scalars at the caller.
fn unicode_titlecase(character: char) -> String {
    if let Some(mapped) = unicode_mapping_to_string(&to_titlecase(character)) {
        if !mapped.is_empty() {
            return mapped;
        }
    }
    if let Some(mapped) = unicode_mapping_to_string(&to_uppercase(character)) {
        if !mapped.is_empty() {
            return mapped;
        }
    }
    character.to_string()
}

fn unicode_mapping_to_string(mapping: &[u32]) -> Option<String> {
    let mut result = String::new();
    for &codepoint in mapping {
        if codepoint == 0 {
            break;
        }
        result.push(char::from_u32(codepoint)?);
    }
    Some(result)
}

fn evaluate_plaintext(
    _builtin: &BuiltinSpec,
    mut arguments: BoundArguments,
) -> Result<IrValue, BuiltinError> {
    let content = arguments
        .remove(0)
        .ok_or_else(|| error("`.plaintext` requires one content argument".to_string()))?;
    let text = plain_text_argument(&content).ok_or_else(|| {
        error(
            "`.plaintext` requires already-parsed inline content or a supported scalar value"
                .to_string(),
        )
    })?;
    Ok(IrValue::String(text))
}

fn numeric_argument(value: &InvocationValue, parameter: &str) -> Result<f32, BuiltinError> {
    Ok(numeric_argument_value(value, parameter)? as f32)
}

fn numeric_argument_value(value: &InvocationValue, parameter: &str) -> Result<f64, BuiltinError> {
    match value_conversion::convert_scalar_with_origin(value, ScalarTarget::Number) {
        Ok(ScalarValue::Number(number)) => Ok(number),
        Ok(_) => Err(conversion_error(
            format!("`{parameter}` must be numeric"),
            parameter,
            value_conversion::ConversionError::UnsupportedValue {
                target: value_conversion::ConversionTarget::Number,
            },
        )),
        Err(error) => Err(conversion_error(
            format!("`{parameter}` must be numeric"),
            parameter,
            error,
        )),
    }
}

fn boolean_argument(value: &InvocationValue, parameter: &str) -> Result<bool, BuiltinError> {
    match value_conversion::convert_scalar_with_origin(value, ScalarTarget::Boolean) {
        Ok(ScalarValue::Boolean(value)) => Ok(value),
        Ok(_) => Err(conversion_error(
            format!("`{parameter}` must be boolean"),
            parameter,
            value_conversion::ConversionError::UnsupportedValue {
                target: value_conversion::ConversionTarget::Boolean,
            },
        )),
        Err(error) => Err(conversion_error(
            format!("`{parameter}` must be boolean"),
            parameter,
            error,
        )),
    }
}

fn values_equal(left: &IrValue, right: &IrValue) -> bool {
    if left == right {
        return true;
    }
    match (comparable_plain_text(left), comparable_plain_text(right)) {
        (Some(left), Some(right)) => left == right,
        _ => false,
    }
}

/// Mirrors the public equality helper's plain-text fallback without turning
/// structured values into text for unrelated operations. Rich content is
/// projected only because upstream `.equals` explicitly compares its plain
/// text representation as a final equality branch.
fn comparable_plain_text(value: &IrValue) -> Option<String> {
    match value {
        IrValue::String(value) | IrValue::Identifier(value) => Some(value.clone()),
        IrValue::Number(value) => Some(value.to_string()),
        IrValue::Content(nodes) => {
            let mut text = String::new();
            for node in nodes {
                append_node_plain_text(node, &mut text)?;
            }
            Some(text)
        }
        _ => None,
    }
}

fn append_node_plain_text(node: &IrNode, output: &mut String) -> Option<()> {
    match node {
        IrNode::Paragraph { content, .. } | IrNode::Heading { content, .. } => {
            for inline in content {
                append_inline_plain_text(inline, output)?;
            }
        }
        IrNode::Blockquote { content, .. } => {
            for child in content {
                append_node_plain_text(child, output)?;
            }
        }
        IrNode::UnorderedList { items, .. } | IrNode::OrderedList { items, .. } => {
            for item in items {
                for child in &item.nodes {
                    append_node_plain_text(child, output)?;
                }
            }
        }
        IrNode::Table { header, rows, .. } => {
            append_row_plain_text(header, output)?;
            for row in rows {
                append_row_plain_text(row, output)?;
            }
        }
        IrNode::CodeBlock { source, .. } => output.push_str(source),
        IrNode::FunctionCall { .. }
        | IrNode::ChainedFunctionCall { .. }
        | IrNode::FunctionDeclaration { .. }
        | IrNode::Component { .. }
        | IrNode::RawHtml { .. }
        | IrNode::TargetSpecificContent { .. }
        | IrNode::ThematicBreak { .. }
        | IrNode::Math { .. } => return None,
    }
    Some(())
}

fn append_row_plain_text(row: &scribium_ir::IrTableRow, output: &mut String) -> Option<()> {
    for cell in &row.cells {
        for inline in &cell.content {
            append_inline_plain_text(inline, output)?;
        }
    }
    Some(())
}

fn append_inline_plain_text(inline: &IrInline, output: &mut String) -> Option<()> {
    match inline {
        IrInline::Text { content, .. } | IrInline::Code { content, .. } => output.push_str(content),
        // v2.5.1 `NodeUtils.toPlainText()` omits both forms of whitespace.
        IrInline::Whitespace { .. } => {}
        IrInline::Emphasis { content, .. }
        | IrInline::Strong { content, .. }
        | IrInline::Strikethrough { content, .. }
        | IrInline::Link { content, .. } => {
            for child in content {
                append_inline_plain_text(child, output)?;
            }
        }
        IrInline::SoftBreak { .. } | IrInline::HardBreak { .. } => output.push('\n'),
        IrInline::DirectiveCall { .. }
        | IrInline::ChainedDirectiveCall { .. }
        | IrInline::RawHtml { .. }
        | IrInline::TargetSpecificContent { .. } => return None,
        IrInline::Image { .. } => return None,
    }
    Some(())
}

/// Projects the bounded `.plaintext` input contract without converting a
/// value back into source text or invoking a Markdown parser.
pub(crate) fn plain_text_argument(value: &IrValue) -> Option<String> {
    match value {
        IrValue::Identifier(value) => Some(value.clone()),
        IrValue::Boolean(value) => Some(value.to_string()),
        IrValue::Number(value) => Some(value.to_string()),
        IrValue::Content(nodes) => plain_text_from_content(nodes),
        IrValue::String(_)
        | IrValue::Range(_)
        | IrValue::Collection(_)
        | IrValue::Pair(_)
        | IrValue::Dictionary(_)
        | IrValue::Callable(_)
        | IrValue::InlineBody(_)
        | IrValue::Size(_)
        | IrValue::Color(_)
        | IrValue::Enum(_)
        | IrValue::Component(_)
        | IrValue::None => None,
    }
}

fn plain_text_from_content(nodes: &[IrNode]) -> Option<String> {
    let mut output = String::new();
    for node in nodes {
        let IrNode::Paragraph { content, .. } = node else {
            return None;
        };
        plain_text_from_inlines(content, &mut output)?;
    }
    Some(output)
}

fn plain_text_from_inlines(inlines: &[IrInline], output: &mut String) -> Option<()> {
    for inline in inlines {
        match inline {
            IrInline::Text { content, .. } | IrInline::Code { content, .. } => {
                output.push_str(content);
            }
            IrInline::Emphasis { content, .. }
            | IrInline::Strong { content, .. }
            | IrInline::Strikethrough { content, .. }
            | IrInline::Link { content, .. } => plain_text_from_inlines(content, output)?,
            IrInline::SoftBreak { .. } => output.push('\n'),
            // v2.5.1 `NodeUtils.toPlainText()` does not emit hard-break text.
            IrInline::HardBreak { .. } | IrInline::Image { .. } | IrInline::Whitespace { .. } => {}
            IrInline::DirectiveCall { .. }
            | IrInline::ChainedDirectiveCall { .. }
            | IrInline::RawHtml { .. }
            | IrInline::TargetSpecificContent { .. } => {
                return None;
            }
        }
    }
    Some(())
}

fn evaluate_otherwise(
    _builtin: &BuiltinSpec,
    mut arguments: BoundArguments,
) -> Result<IrValue, BuiltinError> {
    let value = arguments
        .remove(0)
        .ok_or_else(|| error("`.otherwise` requires a value argument".to_string()))?;
    let fallback = arguments
        .remove(0)
        .ok_or_else(|| error("`.otherwise` requires a fallback argument".to_string()))?;
    if matches!(value.value, IrValue::None) {
        Ok(fallback.value)
    } else {
        Ok(value.value)
    }
}

fn evaluate_none(
    _builtin: &BuiltinSpec,
    _arguments: BoundArguments,
) -> Result<IrValue, BuiltinError> {
    Ok(IrValue::None)
}

fn evaluate_isnone(
    _builtin: &BuiltinSpec,
    mut arguments: BoundArguments,
) -> Result<IrValue, BuiltinError> {
    let value = arguments
        .remove(0)
        .ok_or_else(|| error("`.isnone` requires one value argument".to_string()))?;
    Ok(IrValue::Boolean(matches!(value.value, IrValue::None)))
}

fn evaluate_numeric(
    builtin: &BuiltinSpec,
    mut arguments: BoundArguments,
) -> Result<IrValue, BuiltinError> {
    let name = builtin.name;
    let first = arguments
        .remove(0)
        .ok_or_else(|| error(format!("`.{name}` requires numeric arguments")))?;
    let second = arguments
        .remove(0)
        .ok_or_else(|| error(format!("`.{name}` requires numeric arguments")))?;
    let first = numeric_argument(&first, builtin.signature.parameter_names[0])
        .map_err(|error| error.with_message(format!("`.{name}` requires numeric arguments")))?;
    let second = numeric_argument(&second, builtin.signature.parameter_names[1])
        .map_err(|error| error.with_message(format!("`.{name}` requires numeric arguments")))?;

    let result = match builtin.kind {
        BuiltinKind::Sum => first + second,
        BuiltinKind::Subtract => first - second,
        BuiltinKind::Multiply => first * second,
        BuiltinKind::Divide => first / second,
        BuiltinKind::Rem => first % second,
        BuiltinKind::Pow => first.powi(kotlin_float_to_int(second)),
        _ => unreachable!("unrecognized binary numeric builtin: {name}"),
    };
    Ok(numeric_result(result))
}

fn evaluate_unary_numeric(
    builtin: &BuiltinSpec,
    mut arguments: BoundArguments,
) -> Result<IrValue, BuiltinError> {
    let name = builtin.name;
    let value = arguments
        .remove(0)
        .ok_or_else(|| error(format!("`.{name}` requires one numeric argument")))?;
    let value = numeric_argument(&value, "x")
        .map_err(|error| error.with_message(format!("`.{name}` requires a numeric argument")))?;

    match builtin.kind {
        BuiltinKind::Abs => Ok(numeric_result(value.abs())),
        BuiltinKind::Negate => Ok(numeric_result(-value)),
        BuiltinKind::Sqrt => Ok(numeric_result(value.sqrt())),
        BuiltinKind::IsEven => Ok(IrValue::Boolean(kotlin_float_to_int(value) % 2 == 0)),
        _ => Err(error(format!(
            "`.{name}` has no unary numeric implementation"
        ))),
    }
}

fn evaluate_transcendental(
    builtin: &BuiltinSpec,
    mut arguments: BoundArguments,
) -> Result<IrValue, BuiltinError> {
    let name = builtin.name;
    let value = arguments
        .remove(0)
        .ok_or_else(|| error(format!("`.{name}` requires one numeric argument")))?;
    let value = numeric_argument(&value, "x")
        .map_err(|error| error.with_message(format!("`.{name}` requires a numeric argument")))?;
    Ok(numeric_result(deterministic_transcendental(
        builtin.kind,
        value,
    )))
}

fn evaluate_pi(_builtin: &BuiltinSpec, arguments: BoundArguments) -> Result<IrValue, BuiltinError> {
    debug_assert!(arguments.is_empty());

    // Quarkdown passes kotlin.math.PI as a Double to NumberValue. Keep this
    // binary64 constant separate from the Float result normalization used by
    // arithmetic and transcendental builtins.
    Ok(IrValue::Number(std::f64::consts::PI))
}

/// Reproduces the JVM observable boundary of Kotlin's Float overloads without
/// calling platform `std` math. Kotlin/JVM first adapts the argument to Float,
/// calls `java.lang.Math` on the widened binary64 value, and narrows the result
/// back to Float. `libm` is pinned and built with no default features so these
/// operations use its pure-Rust software implementations on native and WASM
/// targets rather than an OS libc/libm or a target-specific math intrinsic.
fn deterministic_transcendental(kind: BuiltinKind, value: f32) -> f32 {
    let value = f64::from(value);
    let result = match kind {
        BuiltinKind::Logn => libm::log(value),
        BuiltinKind::Sin => libm::sin(value),
        BuiltinKind::Cos => libm::cos(value),
        BuiltinKind::Tan => libm::tan(value),
        _ => unreachable!("unrecognized transcendental builtin"),
    };
    result as f32
}

fn evaluate_truncate(
    _builtin: &BuiltinSpec,
    mut arguments: BoundArguments,
) -> Result<IrValue, BuiltinError> {
    let value = arguments
        .remove(0)
        .ok_or_else(|| error("`.truncate` requires a numeric `x` argument".to_string()))?;
    let decimals = arguments
        .remove(0)
        .ok_or_else(|| error("`.truncate` requires an integer `decimals` argument".to_string()))?;
    let value = numeric_argument(&value, "x").map_err(|error| {
        error.with_message("`.truncate` requires a numeric `x` argument".to_string())
    })?;
    let decimals = integer_argument(&decimals, "decimals")?;
    if decimals < 0 {
        return Err(error(
            "`.truncate` requires non-negative `decimals`".to_string(),
        ));
    }

    // NumberValue turns every integral Float (including infinities after
    // Kotlin's clamped toInt conversion) into an Int before truncate sees it.
    // Keep that branch separate from the floating post-processing formula.
    if decimals == 0 || number_value_is_integral(value) {
        return Ok(IrValue::Number(f64::from(kotlin_float_to_int(value))));
    }

    // This deliberately follows Math.kt's observable boundaries:
    // x.toFloat() * (10.0.pow(decimals)) is Double arithmetic, the product is
    // converted with Double.toInt(), and the final division is Float.
    let multiplier = 10.0_f64.powi(decimals);
    let product = f64::from(value) * multiplier;
    let truncated = kotlin_double_to_int(product);
    let result = truncated as f32 / multiplier as f32;
    Ok(numeric_result(result))
}

fn evaluate_round(
    _builtin: &BuiltinSpec,
    mut arguments: BoundArguments,
) -> Result<IrValue, BuiltinError> {
    let value = arguments
        .remove(0)
        .ok_or_else(|| error("`.round` requires one numeric argument".to_string()))?;
    let value = numeric_argument(&value, "x")
        .map_err(|error| error.with_message("`.round` requires a numeric argument".to_string()))?;
    Ok(IrValue::Number(f64::from(kotlin_round_to_int(value))))
}

/// Applies the `NumberValue` normalization visible at the upstream output
/// boundary: integral Float values become Int values, including Kotlin's
/// clamped conversions for infinities and out-of-range finite values.
fn numeric_result(value: f32) -> IrValue {
    if value.is_nan() {
        return IrValue::Number(f64::NAN);
    }
    if value.ceil() == value.floor() {
        return IrValue::Number(f64::from(kotlin_float_to_int(value)));
    }
    IrValue::Number(f64::from(value))
}

/// Mirrors Kotlin `Float.toInt()`: truncate toward zero, map NaN to zero,
/// and clamp finite/infinite values to the signed Int range.
fn kotlin_float_to_int(value: f32) -> i32 {
    let value = f64::from(value);
    if value.is_nan() {
        0
    } else if value <= f64::from(i32::MIN) {
        i32::MIN
    } else if value >= f64::from(i32::MAX) {
        i32::MAX
    } else {
        value.trunc() as i32
    }
}

/// Mirrors Kotlin `Double.toInt()` for the truncate multiplier product.
fn kotlin_double_to_int(value: f64) -> i32 {
    if value.is_nan() {
        0
    } else if value <= f64::from(i32::MIN) {
        i32::MIN
    } else if value >= f64::from(i32::MAX) {
        i32::MAX
    } else {
        value.trunc() as i32
    }
}

/// NumberValue's normalization predicate, including its NaN/Infinity edges.
fn number_value_is_integral(value: f32) -> bool {
    value.ceil() == value.floor()
}

/// Strictly adapts the narrow `decimals: Int` parameter boundary.
///
/// Quarkdown accepts an integral numeric representation such as `2` or `2.0`
/// after the DynamicValue Number conversion and NumberValue normalization.
/// Static StringValue text and fractional NumberValue results are not silently
/// converted to an Int.
fn integer_argument(value: &InvocationValue, parameter: &str) -> Result<i32, BuiltinError> {
    let number = match &value.value {
        IrValue::Number(value) => Some(*value as f32),
        IrValue::String(text) | IrValue::Identifier(text)
            if value.origin == crate::value_conversion::ValueOrigin::Dynamic =>
        {
            text.parse::<i32>()
                .map(|value| value as f32)
                .ok()
                .or_else(|| text.parse::<f32>().ok())
        }
        _ => None,
    };
    let Some(number) = number else {
        return Err(conversion_error(
            format!("`{parameter}` must be an integer"),
            parameter,
            value_conversion::ConversionError::UnsupportedValue {
                target: value_conversion::ConversionTarget::Integer,
            },
        ));
    };
    if !number_value_is_integral(number) {
        return Err(conversion_error(
            format!("`{parameter}` must be an integer"),
            parameter,
            value_conversion::ConversionError::InvalidText {
                target: value_conversion::ConversionTarget::Integer,
            },
        ));
    }
    Ok(kotlin_float_to_int(number))
}

/// Reproduces Kotlin's `kotlin.math.round(Float)` followed by `toInt()`.
/// Kotlin uses ties-to-even; Rust's default `round` is not used here.
fn kotlin_round_to_int(value: f32) -> i32 {
    if !value.is_finite() || number_value_is_integral(value) {
        return kotlin_float_to_int(value);
    }

    let lower = value.floor();
    let fraction = value - lower;
    let rounded = if fraction < 0.5 {
        lower
    } else if fraction > 0.5 || lower % 2.0 != 0.0 {
        lower + 1.0
    } else {
        lower
    };
    kotlin_float_to_int(rounded)
}

fn evaluate_case(
    builtin: &BuiltinSpec,
    mut arguments: BoundArguments,
) -> Result<IrValue, BuiltinError> {
    let name = builtin.name;
    let value = arguments
        .remove(0)
        .ok_or_else(|| error(format!("`.{name}` requires one string argument")))?;
    let text = scalar_string_argument_result(&value, "string").map_err(|error| {
        error.with_message(format!(
            "`.{name}` requires a scalar value that can adapt to text"
        ))
    })?;
    let transformed = match builtin.kind {
        BuiltinKind::Uppercase => text.to_uppercase(),
        BuiltinKind::Lowercase => text.to_lowercase(),
        BuiltinKind::Capitalize => {
            let mut characters = text.chars();
            let Some(first) = characters.next() else {
                return Ok(IrValue::String(text));
            };
            let mut result = if first.len_utf16() == 1 {
                unicode_titlecase(first)
            } else {
                // Kotlin's `replaceFirstChar` passes the leading UTF-16
                // surrogate to `Char.titlecase`, which leaves supplementary
                // input unchanged.
                first.to_string()
            };
            result.push_str(characters.as_str());
            result
        }
        _ => return Err(error(format!("`.{name}` has no case transformation"))),
    };
    Ok(IrValue::String(transformed))
}

fn error(message: String) -> BuiltinError {
    BuiltinError {
        message,
        conversion: None,
    }
}

fn conversion_error(
    message: String,
    parameter: &str,
    error: value_conversion::ConversionError,
) -> BuiltinError {
    BuiltinError {
        message,
        conversion: Some(BuiltinConversionFailure {
            parameter: parameter.to_string(),
            error,
            candidate_span: None,
            parameter_span: None,
        }),
    }
}

/// Applies the context-free String conversion boundary used by scalar string
/// builtins. Rich content remains the separate `.plaintext`/native-content
/// path; it is not silently serialized or reparsed here.
pub(crate) fn scalar_string_conversion(
    value: &InvocationValue,
) -> Result<String, value_conversion::ConversionError> {
    match value_conversion::convert_scalar_with_origin(value, ScalarTarget::String) {
        Ok(ScalarValue::String(value)) => Ok(value),
        Ok(_) => Err(value_conversion::ConversionError::UnsupportedValue {
            target: value_conversion::ConversionTarget::String,
        }),
        Err(error) => match &value.value {
            IrValue::Content(nodes) => plain_scalar_content_argument(nodes).ok_or(error),
            _ => Err(error),
        },
    }
}

fn scalar_string_argument_result(
    value: &InvocationValue,
    parameter: &str,
) -> Result<String, BuiltinError> {
    scalar_string_conversion(value).map_err(|error| {
        conversion_error(
            format!("`{parameter}` cannot adapt to text"),
            parameter,
            error,
        )
    })
}

fn plain_scalar_content_argument(nodes: &[IrNode]) -> Option<String> {
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

/// Applies the existing structural text boundary used by resource and native
/// content consumers. Plain paragraph content is adapted structurally; rich
/// content is not rendered or round-tripped through a backend. Scalar builtin
/// conversion uses [`scalar_string_conversion`] instead.
pub(crate) fn adapt_string_argument(value: &IrValue) -> Option<String> {
    match value {
        IrValue::String(text) | IrValue::Identifier(text) => Some(text.clone()),
        IrValue::Boolean(value) => Some(value.to_string()),
        IrValue::Number(value) => Some(value.to_string()),
        IrValue::None => None,
        IrValue::Range(_)
        | IrValue::Collection(_)
        | IrValue::Pair(_)
        | IrValue::Dictionary(_)
        | IrValue::Callable(_)
        | IrValue::InlineBody(_)
        | IrValue::Size(_)
        | IrValue::Color(_)
        | IrValue::Enum(_)
        | IrValue::Component(_) => None,
        IrValue::Content(nodes) => plain_scalar_content_argument(nodes),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        deterministic_transcendental, evaluate, evaluate_with_origins, lookup, regular_builtins,
        BuiltinBodyPolicy, PINNED_JVM_UNICODE_VERSION, UNICODE_VERSION,
    };
    use crate::value_conversion::InvocationValue;
    use scribium_ir::{
        IrCallable, IrDictionary, IrInline, IrNode, IrPair, IrRange, IrSize, IrSizeUnit, IrValue,
    };
    use scribium_source::{SourceId, SourceSpan};

    fn number(value: f64) -> IrValue {
        IrValue::Number(value)
    }

    fn named_arg(name: &str, value: IrValue) -> scribium_ir::IrNamedArg {
        scribium_ir::IrNamedArg {
            name: name.to_string(),
            name_span: scribium_source::SourceSpan::new(scribium_source::SourceId(0), 0, 0),
            value,
            span: scribium_source::SourceSpan::new(scribium_source::SourceId(0), 0, 0),
        }
    }

    #[test]
    fn regular_inventory_is_unique_complete_and_round_trips() {
        let inventory = regular_builtins();
        assert!(!inventory.is_empty());
        let mut dispatch_kinds = Vec::new();

        for (index, builtin) in inventory.iter().enumerate() {
            assert_eq!(lookup(builtin.name), Some(builtin));
            assert_eq!(
                builtin.signature.max_positional,
                builtin.signature.parameter_names.len()
            );
            for (parameter_index, parameter) in builtin.signature.parameter_names.iter().enumerate()
            {
                assert!(
                    !builtin.signature.parameter_names[..parameter_index].contains(parameter),
                    "{}.{} is duplicated in its signature",
                    builtin.name,
                    parameter
                );
            }
            assert!(
                inventory[index + 1..]
                    .iter()
                    .all(|other| other.name != builtin.name),
                "{} is registered more than once",
                builtin.name
            );
            assert!(
                !dispatch_kinds.contains(&builtin.kind),
                "{} reuses another builtin dispatch identity",
                builtin.name
            );
            dispatch_kinds.push(builtin.kind);
        }
    }

    #[test]
    fn regular_body_fallback_policy_is_declared_per_signature() {
        for builtin in regular_builtins() {
            let expected = if builtin.name == "plaintext" {
                BuiltinBodyPolicy::BindEvaluatedContent
            } else if builtin.signature.parameter_names.is_empty() {
                BuiltinBodyPolicy::Reject
            } else {
                BuiltinBodyPolicy::BindRaw
            };
            assert_eq!(
                builtin.body_policy, expected,
                "unexpected body policy for .{}",
                builtin.name
            );
        }
    }

    #[test]
    fn numeric_surface_is_registered_and_preserves_typed_results() {
        for name in [
            "sum", "subtract", "multiply", "divide", "rem", "pow", "abs", "negate", "sqrt", "logn",
            "pi", "sin", "cos", "tan", "truncate", "round", "iseven",
        ] {
            assert_eq!(lookup(name).map(|builtin| builtin.name), Some(name));
        }

        assert_eq!(
            evaluate(
                "subtract",
                &[number(10.0)],
                &[named_arg("b", number(3.0))],
                false
            )
            .expect("named subtraction should evaluate"),
            number(7.0)
        );
        assert_eq!(
            evaluate(
                "multiply",
                &[number(-2.0)],
                &[named_arg("by", number(2.5))],
                false,
            )
            .expect("mixed multiplication should evaluate"),
            number(f64::from((-2.0_f32) * 2.5_f32))
        );
        assert_eq!(
            evaluate(
                "divide",
                &[],
                &[named_arg("a", number(7.0)), named_arg("by", number(2.0))],
                false,
            )
            .expect("named division should evaluate"),
            number(f64::from(3.5_f32))
        );
        assert_eq!(
            evaluate(
                "sum",
                &[IrValue::Identifier("10".into())],
                &[named_arg("b", IrValue::String("-2.5".into()))],
                false,
            )
            .expect("numeric scalar text should adapt"),
            number(f64::from(7.5_f32))
        );
        assert_eq!(
            evaluate(
                "abs",
                &[],
                &[named_arg("x", IrValue::String("-3.5".into()))],
                false,
            )
            .expect("numeric text should adapt for unary operations"),
            number(f64::from(3.5_f32))
        );
        assert_eq!(
            evaluate("negate", &[number(-2.5)], &[], false)
                .expect("negation should remain numeric"),
            number(f64::from(2.5_f32))
        );
        assert_eq!(
            evaluate("sqrt", &[number(9.0)], &[], false).expect("square root should evaluate"),
            number(3.0)
        );
        assert_eq!(
            evaluate("iseven", &[IrValue::Identifier("-4".into())], &[], false)
                .expect("evenness should return a typed boolean"),
            IrValue::Boolean(true)
        );
    }

    #[test]
    fn decimal_numeric_surface_matches_upstream_boundaries() {
        assert_eq!(
            evaluate("truncate", &[number(201.06194), number(2.0)], &[], false)
                .expect("positional decimals should bind"),
            number(f64::from(201.06_f32))
        );
        assert_eq!(
            evaluate(
                "truncate",
                &[number(201.06194)],
                &[named_arg("decimals", number(1.0))],
                false,
            )
            .expect("named decimals should bind"),
            number(201.0)
        );
        assert_eq!(
            evaluate("truncate", &[number(-1.29), number(1.0)], &[], false)
                .expect("negative truncation should use zero direction"),
            number(f64::from(-1.2_f32))
        );
        assert_eq!(
            evaluate("truncate", &[number(201.06194), number(0.0)], &[], false)
                .expect("zero decimals should use toInt"),
            number(201.0)
        );
        assert_eq!(
            evaluate("truncate", &[number(201.0), number(2.0)], &[], false)
                .expect("integral values should remain integral"),
            number(201.0)
        );
        assert_eq!(
            evaluate(
                "truncate",
                &[IrValue::String("-1.29".into()), number(1.0)],
                &[],
                false,
            )
            .expect("numeric text x should use the existing Number boundary"),
            number(f64::from(-1.2_f32))
        );
        assert_eq!(
            evaluate(
                "truncate",
                &[
                    evaluate("sum", &[number(100.0), number(1.06194)], &[], false)
                        .expect("nested numeric x should evaluate")
                ],
                &[named_arg("decimals", number(2.0))],
                false,
            )
            .expect("nested numeric result should bind as x"),
            number(f64::from(101.06_f32))
        );

        assert!(evaluate("truncate", &[number(1.0), number(1.5)], &[], false).is_err());
        assert!(evaluate("truncate", &[], &[], false).is_err());
        assert!(evaluate(
            "truncate",
            &[number(1.0), number(2.0), number(3.0)],
            &[],
            false,
        )
        .is_err());
        assert_eq!(
            evaluate(
                "truncate",
                &[number(1.0)],
                &[named_arg("decimals", IrValue::String("2".into()))],
                false,
            )
            .expect("dynamic String decimals should use Number conversion"),
            number(1.0)
        );
        assert_eq!(
            evaluate(
                "truncate",
                &[number(1.0)],
                &[named_arg("decimals", IrValue::String("2.0".into()))],
                false,
            )
            .expect("integral dynamic Float text should normalize to Int"),
            number(1.0)
        );
        assert!(evaluate(
            "truncate",
            &[number(1.0)],
            &[named_arg("decimals", IrValue::String("1.5".into()))],
            false,
        )
        .is_err());
        assert!(evaluate_with_origins(
            lookup("truncate").expect("truncate is in the regular inventory"),
            &[
                InvocationValue::dynamic_value(number(1.0)),
                InvocationValue::static_value(IrValue::String("2".into())),
            ],
            &[],
            false,
        )
        .is_err());
        assert!(evaluate("truncate", &[number(1.0), number(-1.0)], &[], false).is_err());
        assert!(evaluate("truncate", &[number(1.0), number(-1.5)], &[], false).is_err());
        assert!(evaluate("truncate", &[number(1.0), number(f64::NAN)], &[], false).is_err());
        assert_eq!(
            evaluate(
                "truncate",
                &[number(1.25), number(f64::INFINITY)],
                &[],
                false
            )
            .expect("infinite decimals should follow Int normalization"),
            number(0.0)
        );
        assert_eq!(
            evaluate(
                "truncate",
                &[number(1.25), number(f64::from(i32::MAX))],
                &[],
                false,
            )
            .expect("large decimals should not use a Scribium-only limit"),
            number(0.0)
        );
        assert!(evaluate(
            "truncate",
            &[number(1.0)],
            &[named_arg("unknown", number(2.0))],
            false
        )
        .is_err());
        assert!(evaluate(
            "truncate",
            &[number(1.0), number(2.0)],
            &[named_arg("x", number(3.0))],
            false
        )
        .is_err());
        assert!(evaluate("truncate", &[number(1.0), number(2.0)], &[], true).is_err());

        assert_eq!(
            evaluate("round", &[number(2.5)], &[], false).expect("2.5 should round"),
            number(2.0)
        );
        assert_eq!(
            evaluate("round", &[number(3.5)], &[], false).expect("3.5 should round"),
            number(4.0)
        );
        assert_eq!(
            evaluate("round", &[number(-2.5)], &[], false).expect("-2.5 should round"),
            number(-2.0)
        );
        assert_eq!(
            evaluate("round", &[number(-3.5)], &[], false).expect("-3.5 should round"),
            number(-4.0)
        );
        for (input, expected) in [(2.49, 2.0), (2.51, 3.0), (-2.49, -2.0), (-2.51, -3.0)] {
            assert_eq!(
                evaluate("round", &[number(input)], &[], false).expect("non-tie should round"),
                number(expected)
            );
        }
        assert_eq!(
            evaluate("round", &[], &[named_arg("x", number(2.5))], false)
                .expect("named x should bind"),
            number(2.0)
        );
        assert_eq!(
            evaluate("round", &[number(f64::NAN)], &[], false)
                .expect("NaN should follow Kotlin Float.toInt"),
            number(0.0)
        );
        assert_eq!(
            evaluate("round", &[number(f64::INFINITY)], &[], false)
                .expect("positive infinity should clamp"),
            number(f64::from(i32::MAX))
        );
        assert_eq!(
            evaluate("round", &[number(f64::NEG_INFINITY)], &[], false)
                .expect("negative infinity should clamp"),
            number(f64::from(i32::MIN))
        );
        assert_eq!(
            evaluate("round", &[number(1.0e30)], &[], false)
                .expect("large finite values should clamp"),
            number(f64::from(i32::MAX))
        );
        assert!(evaluate("round", &[IrValue::Boolean(true)], &[], false).is_err());
        assert!(evaluate("round", &[], &[], false).is_err());
        assert!(evaluate("round", &[number(1.0), number(2.0)], &[], false).is_err());
        assert!(evaluate(
            "round",
            &[number(1.0)],
            &[named_arg("x", number(2.0))],
            false
        )
        .is_err());
        assert!(evaluate(
            "round",
            &[number(1.0)],
            &[named_arg("unknown", number(2.0))],
            false
        )
        .is_err());
        assert!(evaluate("round", &[number(1.0)], &[], true).is_err());
    }

    #[test]
    fn transcendental_numeric_surface_matches_upstream_boundaries() {
        assert_eq!(
            evaluate("pi", &[], &[], false).expect("pi has zero arguments"),
            number(std::f64::consts::PI)
        );
        assert_eq!(
            evaluate("logn", &[number(1.0)], &[], false).expect("ln(1) should normalize to zero"),
            number(0.0)
        );
        assert_eq!(
            evaluate("sin", &[number(0.0)], &[], false).expect("sin(0) should normalize to zero"),
            number(0.0)
        );
        assert_eq!(
            evaluate("cos", &[number(0.0)], &[], false).expect("cos(0) should normalize to one"),
            number(1.0)
        );
        assert_eq!(
            evaluate("tan", &[number(0.0)], &[], false).expect("tan(0) should normalize to zero"),
            number(0.0)
        );
        assert_eq!(
            evaluate("cos", &[number(std::f64::consts::PI)], &[], false)
                .expect("cos(pi) should use the Float-adapted pi"),
            number(-1.0)
        );
        assert_eq!(
            evaluate(
                "cos",
                &[],
                &[named_arg("x", number(std::f64::consts::PI))],
                false,
            )
            .expect("named x should bind for cosine"),
            number(-1.0)
        );
        assert_eq!(
            evaluate(
                "logn",
                &[evaluate("sum", &[number(1.0), number(1.0)], &[], false)
                    .expect("nested arithmetic should produce a number")],
                &[],
                false,
            )
            .expect("nested numeric results should remain typed"),
            number(f64::from(libm::log(2.0_f64) as f32))
        );
        assert_eq!(
            evaluate("pi", &[], &[], false)
                .and_then(|value| evaluate("multiply", &[value, number(2.0)], &[], false))
                .and_then(|value| evaluate("cos", &[value], &[], false))
                .expect("pi::multiply {2}::cos should chain as typed values"),
            number(1.0)
        );
        assert_eq!(
            evaluate("sin", &[IrValue::String("1".into())], &[], false)
                .expect("numeric text should use numeric_argument"),
            number(f64::from(libm::sin(1.0_f64) as f32))
        );

        assert!(matches!(
            evaluate("logn", &[number(0.0)], &[], false),
            Ok(IrValue::Number(value)) if value == f64::from(i32::MIN)
        ));
        assert!(matches!(
            evaluate("logn", &[number(-1.0)], &[], false),
            Ok(IrValue::Number(value)) if value.is_nan()
        ));
        assert!(matches!(
            evaluate("sin", &[number(f64::INFINITY)], &[], false),
            Ok(IrValue::Number(value)) if value.is_nan()
        ));
        assert!(matches!(
            evaluate("cos", &[number(f64::NEG_INFINITY)], &[], false),
            Ok(IrValue::Number(value)) if value.is_nan()
        ));
        assert!(matches!(
            evaluate("tan", &[number(f64::NAN)], &[], false),
            Ok(IrValue::Number(value)) if value.is_nan()
        ));
    }

    #[test]
    fn deterministic_transcendental_math_has_stable_representative_bits() {
        for (name, input, expected_bits) in [
            ("logn", 2.0_f32, 0x3f31_7218),
            ("logn", f32::from_bits(0x402d_f854), 0x3f7f_ffff),
            ("sin", 1.0_f32, 0x3f57_6aa4),
            ("sin", f32::from_bits(0x4049_0fdb), 0xb3bb_bd2e),
            ("cos", 1.0_f32, 0x3f0a_5140),
            ("cos", f32::from_bits(0x4049_0fdb), 0xbf80_0000),
            ("tan", 1.0_f32, 0x3fc7_5923),
            ("tan", f32::from_bits(0x4049_0fdb), 0x33bb_bd2e),
        ] {
            assert_eq!(
                deterministic_transcendental(
                    lookup(name)
                        .expect("transcendental builtin is registered")
                        .kind,
                    input,
                )
                .to_bits(),
                expected_bits,
                "{name}({input:?}) changed"
            );
        }

        assert_eq!(
            deterministic_transcendental(lookup("sin").expect("sin is registered").kind, -0.0,)
                .to_bits(),
            (-0.0_f32).to_bits()
        );
        assert_eq!(
            deterministic_transcendental(lookup("tan").expect("tan is registered").kind, -0.0,)
                .to_bits(),
            (-0.0_f32).to_bits()
        );
        assert_eq!(
            deterministic_transcendental(lookup("cos").expect("cos is registered").kind, -0.0,)
                .to_bits(),
            1.0_f32.to_bits()
        );
        assert_eq!(
            deterministic_transcendental(lookup("logn").expect("logn is registered").kind, 0.0,)
                .to_bits(),
            f32::NEG_INFINITY.to_bits()
        );
        assert!(deterministic_transcendental(lookup("logn").unwrap().kind, -1.0).is_nan());
        assert!(deterministic_transcendental(lookup("sin").unwrap().kind, f32::INFINITY).is_nan());
    }

    #[test]
    fn numeric_arithmetic_matches_upstream_float_and_integer_boundaries() {
        assert_eq!(
            evaluate("subtract", &[number(-2.0), number(5.0)], &[], false)
                .expect("negative subtraction should evaluate"),
            number(-7.0)
        );
        assert_eq!(
            evaluate("divide", &[number(-7.0), number(2.0)], &[], false)
                .expect("negative division should evaluate"),
            number(f64::from(-3.5_f32))
        );
        assert!(matches!(
            evaluate("divide", &[number(1.0), number(0.0)], &[], false),
            Ok(IrValue::Number(value)) if value == f64::from(i32::MAX)
        ));
        assert!(matches!(
            evaluate("divide", &[number(0.0), number(0.0)], &[], false),
            Ok(IrValue::Number(value)) if value.is_nan()
        ));
        assert_eq!(
            evaluate("rem", &[number(-5.0), number(2.0)], &[], false)
                .expect("negative left remainder should evaluate"),
            number(-1.0)
        );
        assert_eq!(
            evaluate("rem", &[number(5.0), number(-2.0)], &[], false)
                .expect("negative right remainder should evaluate"),
            number(1.0)
        );
        assert!(matches!(
            evaluate("rem", &[number(1.5), number(0.0)], &[], false),
            Ok(IrValue::Number(value)) if value.is_nan()
        ));
        assert_eq!(
            evaluate(
                "pow",
                &[number(2.0)],
                &[named_arg("to", number(-2.0))],
                false
            )
            .expect("negative exponent should evaluate"),
            number(f64::from((2.0_f32).powi(-2)))
        );
        assert_eq!(
            evaluate(
                "pow",
                &[number(-2.0)],
                &[named_arg("to", number(0.5))],
                false
            )
            .expect("fractional exponent should truncate before pow"),
            number(1.0)
        );
        assert_eq!(
            evaluate(
                "pow",
                &[number(0.0)],
                &[named_arg("to", number(0.0))],
                false
            )
            .expect("zero exponent should use Kotlin Float.pow semantics"),
            number(1.0)
        );
        assert_eq!(
            evaluate("abs", &[number(-0.0)], &[], false).expect("absolute zero should evaluate"),
            number(0.0)
        );
        assert!(matches!(
            evaluate("negate", &[number(0.0)], &[], false),
            Ok(IrValue::Number(value)) if value == 0.0 && value.is_sign_positive()
        ));
        assert!(matches!(
            evaluate("sqrt", &[number(-1.0)], &[], false),
            Ok(IrValue::Number(value)) if value.is_nan()
        ));
        assert_eq!(
            evaluate("iseven", &[number(-3.9)], &[], false)
                .expect("non-integral values should truncate toward zero"),
            IrValue::Boolean(false)
        );
        assert_eq!(
            evaluate("iseven", &[number(-4.9)], &[], false)
                .expect("negative non-integral values should truncate toward zero"),
            IrValue::Boolean(true)
        );
        assert_eq!(
            evaluate("iseven", &[number(f64::NAN)], &[], false)
                .expect("NaN should follow Kotlin Float.toInt"),
            IrValue::Boolean(true)
        );
    }

    #[test]
    fn numeric_builtins_share_argument_binding_and_fail_closed() {
        let binary = ["sum", "subtract", "multiply", "divide", "rem", "pow"];
        let unary = [
            "abs", "negate", "sqrt", "iseven", "logn", "sin", "cos", "tan",
        ];

        for name in binary {
            assert!(
                evaluate(name, &[number(1.0)], &[], false).is_err(),
                "{name} missing"
            );
            assert!(
                evaluate(name, &[number(1.0), number(2.0), number(3.0)], &[], false).is_err(),
                "{name} too many"
            );
            assert!(
                evaluate(
                    name,
                    &[number(1.0), number(2.0)],
                    &[named_arg("unknown", number(3.0))],
                    false
                )
                .is_err(),
                "{name} unknown named"
            );
            assert!(
                evaluate(
                    name,
                    &[number(1.0), number(2.0)],
                    &[named_arg("a", number(3.0))],
                    false
                )
                .is_err(),
                "{name} duplicate binding"
            );
            assert!(
                evaluate(name, &[IrValue::Boolean(true), number(2.0)], &[], false).is_err(),
                "{name} invalid conversion"
            );
            assert!(
                evaluate(name, &[number(1.0), number(2.0)], &[], true).is_err(),
                "{name} block body"
            );
        }

        for name in unary {
            assert!(evaluate(name, &[], &[], false).is_err(), "{name} missing");
            assert!(
                evaluate(name, &[number(1.0), number(2.0)], &[], false).is_err(),
                "{name} too many"
            );
            assert!(
                evaluate(
                    name,
                    &[number(1.0)],
                    &[named_arg("unknown", number(2.0))],
                    false
                )
                .is_err(),
                "{name} unknown named"
            );
            assert!(
                evaluate(name, &[number(1.0)], &[named_arg("x", number(2.0))], false).is_err(),
                "{name} duplicate binding"
            );
            assert!(
                evaluate(name, &[IrValue::Boolean(true)], &[], false).is_err(),
                "{name} invalid conversion"
            );
            assert!(
                evaluate(name, &[number(1.0)], &[], true).is_err(),
                "{name} block body"
            );
        }

        assert!(evaluate("pi", &[number(1.0)], &[], false).is_err());
        assert!(evaluate("pi", &[], &[named_arg("x", number(1.0))], false).is_err());
        assert!(evaluate("pi", &[], &[], true).is_err());
    }

    #[test]
    fn string_surface_is_registered_and_returns_typed_values() {
        for name in [
            "string",
            "concatenate",
            "uppercase",
            "lowercase",
            "capitalize",
            "isempty",
            "isnotempty",
            "startswith",
            "plaintext",
        ] {
            assert_eq!(lookup(name).map(|builtin| builtin.name), Some(name));
        }

        assert_eq!(
            evaluate("string", &[IrValue::String("  Hello  ".into())], &[], false,)
                .expect("quoted scalar should preserve inner whitespace"),
            IrValue::String("  Hello  ".into())
        );
        assert_eq!(
            evaluate("string", &[number(3.5)], &[], false).expect("number adapts to text"),
            IrValue::String("3.5".into())
        );
        assert_eq!(
            evaluate(
                "string",
                &[],
                &[named_arg("value", IrValue::Boolean(true))],
                false,
            )
            .expect("named string argument should bind"),
            IrValue::String("true".into())
        );
    }

    #[test]
    fn plaintext_projects_evaluated_inline_structure() {
        let span = |start, end| SourceSpan::new(SourceId(0), start, end);
        let content = IrValue::Content(vec![IrNode::Paragraph {
            content: vec![
                IrInline::Text {
                    content: "one ".into(),
                    span: span(0, 4),
                },
                IrInline::Strong {
                    content: vec![IrInline::Emphasis {
                        content: vec![IrInline::Strikethrough {
                            content: vec![IrInline::Text {
                                content: "two".into(),
                                span: span(4, 7),
                            }],
                            span: span(4, 7),
                        }],
                        span: span(4, 7),
                    }],
                    span: span(4, 7),
                },
                IrInline::Text {
                    content: " ".into(),
                    span: span(7, 8),
                },
                IrInline::Code {
                    content: "cargo test".into(),
                    span: span(8, 20),
                },
                IrInline::Text {
                    content: " ".into(),
                    span: span(20, 21),
                },
                IrInline::Link {
                    content: vec![IrInline::Text {
                        content: "label".into(),
                        span: span(21, 26),
                    }],
                    destination: "https://example.com".into(),
                    title: None,
                    span: span(21, 50),
                },
                IrInline::SoftBreak { span: span(50, 51) },
                IrInline::Text {
                    content: "next".into(),
                    span: span(51, 55),
                },
                IrInline::HardBreak { span: span(55, 57) },
                IrInline::Image {
                    content: vec![IrInline::Text {
                        content: "alt must be skipped".into(),
                        span: span(57, 76),
                    }],
                    destination: "image.png".into(),
                    title: None,
                    span: span(57, 76),
                },
            ],
            span: span(0, 76),
        }]);

        assert_eq!(
            evaluate("plaintext", &[content], &[], false).expect("content projects"),
            IrValue::String("one two cargo test label\nnext".into())
        );
        assert_eq!(
            evaluate("plaintext", &[IrValue::Content(Vec::new())], &[], false,)
                .expect("empty content projects to an empty string"),
            IrValue::String(String::new())
        );
        assert_eq!(
            evaluate(
                "plaintext",
                &[IrValue::Identifier("hello".into())],
                &[],
                false
            )
            .expect("identifier scalar is supported"),
            IrValue::String("hello".into())
        );
    }

    #[test]
    fn plaintext_omits_dimensionless_and_dimensioned_whitespace() {
        let span = SourceSpan::new(SourceId(0), 0, 1);
        let dimensionless = IrValue::Content(vec![IrNode::Paragraph {
            content: vec![
                IrInline::Text {
                    content: "A".into(),
                    span,
                },
                IrInline::Whitespace {
                    width: None,
                    height: None,
                    span,
                },
                IrInline::Text {
                    content: "B".into(),
                    span,
                },
            ],
            span,
        }]);
        let dimensioned = IrValue::Content(vec![IrNode::Paragraph {
            content: vec![
                IrInline::Text {
                    content: "A".into(),
                    span,
                },
                IrInline::Whitespace {
                    width: Some(IrSize {
                        value: 2.0,
                        unit: IrSizeUnit::Cm,
                    }),
                    height: Some(IrSize {
                        value: 1.0,
                        unit: IrSizeUnit::Pt,
                    }),
                    span,
                },
                IrInline::Text {
                    content: "B".into(),
                    span,
                },
            ],
            span,
        }]);

        assert_eq!(
            evaluate("plaintext", &[dimensionless], &[], false)
                .expect("dimensionless whitespace should be omitted"),
            IrValue::String("AB".into())
        );
        assert_eq!(
            evaluate("plaintext", &[dimensioned], &[], false)
                .expect("dimensioned whitespace should be omitted"),
            IrValue::String("AB".into())
        );
    }

    #[test]
    fn plain_text_fallback_omits_whitespace_without_rejecting_dimensioned_form() {
        let span = SourceSpan::new(SourceId(0), 0, 1);
        let content = IrValue::Content(vec![IrNode::Paragraph {
            content: vec![
                IrInline::Text {
                    content: "A".into(),
                    span,
                },
                IrInline::Whitespace {
                    width: Some(IrSize {
                        value: 2.0,
                        unit: IrSizeUnit::Cm,
                    }),
                    height: None,
                    span,
                },
                IrInline::Text {
                    content: "B".into(),
                    span,
                },
            ],
            span,
        }]);

        assert_eq!(
            evaluate(
                "equals",
                &[content, IrValue::String("AB".into())],
                &[],
                false
            )
            .expect("dimensioned whitespace should not reject plain-text fallback"),
            IrValue::Boolean(true)
        );
    }

    #[test]
    fn plaintext_rejects_reparse_and_unsupported_values() {
        let span = SourceSpan::new(SourceId(0), 0, 1);
        let values = [
            IrValue::String("**not parsed**".into()),
            IrValue::None,
            IrValue::Collection(Vec::new()),
            IrValue::Range(IrRange {
                start: Some(1),
                end: Some(2),
                span,
            }),
            IrValue::Pair(IrPair {
                first: Box::new(IrValue::Identifier("a".into())),
                second: Box::new(IrValue::Identifier("b".into())),
                span,
            }),
            IrValue::Dictionary(IrDictionary {
                entries: Vec::new(),
                span,
            }),
            IrValue::Callable(IrCallable {
                parameters: None,
                body: Vec::new(),
                span,
                capture: None,
            }),
        ];

        for value in values {
            assert!(
                evaluate("plaintext", &[value], &[], false).is_err(),
                "unsupported value must fail closed"
            );
        }

        let unresolved = IrValue::Content(vec![IrNode::Paragraph {
            content: vec![IrInline::DirectiveCall {
                name: "unknown".into(),
                positional_args: Vec::new(),
                named_args: Vec::new(),
                ordered_args: None,
                body: None,
                span,
            }],
            span,
        }]);
        assert!(evaluate("plaintext", &[unresolved], &[], false).is_err());
        assert!(evaluate("plaintext", &[], &[], true).is_err());
    }

    #[test]
    fn plaintext_reuses_single_content_argument_binding() {
        let content = IrValue::Identifier("hello".into());
        assert!(evaluate("plaintext", &[], &[], false).is_err());
        assert!(evaluate("plaintext", &[content.clone(), content.clone()], &[], false).is_err());
        assert!(evaluate(
            "plaintext",
            std::slice::from_ref(&content),
            &[named_arg("unknown", content.clone())],
            false
        )
        .is_err());
        assert!(evaluate(
            "plaintext",
            std::slice::from_ref(&content),
            &[named_arg("content", content.clone())],
            false
        )
        .is_err());
        assert_eq!(
            evaluate(
                "plaintext",
                &[],
                &[named_arg("content", IrValue::Identifier("hello".into()))],
                false
            )
            .expect("named content should bind"),
            IrValue::String("hello".into())
        );
    }

    #[test]
    fn string_operations_bind_named_arguments_and_defaults() {
        assert_eq!(
            evaluate(
                "concatenate",
                &[IrValue::Identifier("abc".into())],
                &[named_arg("with", IrValue::Identifier("def".into()))],
                false,
            )
            .expect("named concatenate argument should bind"),
            IrValue::String("abcdef".into())
        );
        assert_eq!(
            evaluate(
                "concatenate",
                &[
                    IrValue::Identifier("abc".into()),
                    IrValue::Identifier("def".into()),
                    IrValue::Identifier("no".into()),
                ],
                &[],
                false,
            )
            .expect("boolean identifier should adapt at the invocation boundary"),
            IrValue::String("abc".into())
        );
        assert_eq!(
            evaluate(
                "startswith",
                &[
                    IrValue::String("Hello".into()),
                    IrValue::String("He".into())
                ],
                &[],
                false,
            )
            .expect("startswith should use a false default"),
            IrValue::Boolean(true)
        );
        assert_eq!(
            evaluate(
                "startswith",
                &[
                    IrValue::String("Hello".into()),
                    IrValue::String("he".into()),
                ],
                &[named_arg("ignorecase", IrValue::Identifier("yes".into()))],
                false,
            )
            .expect("named ignorecase should bind"),
            IrValue::Boolean(true)
        );
    }

    #[test]
    fn string_case_and_empty_operations_cover_unicode_and_boundaries() {
        for name in ["uppercase", "lowercase"] {
            assert!(matches!(
                evaluate(
                    name,
                    &[],
                    &[named_arg("string", IrValue::Identifier("Hello".into()))],
                    false,
                ),
                Ok(IrValue::String(_))
            ));
        }
        assert_eq!(
            evaluate(
                "capitalize",
                &[IrValue::Identifier("hello, world!".into())],
                &[],
                false,
            )
            .expect("capitalize should adapt a scalar identifier"),
            IrValue::String("Hello, world!".into())
        );
        assert_eq!(
            evaluate("capitalize", &[IrValue::String(String::new())], &[], false)
                .expect("empty capitalization should succeed"),
            IrValue::String(String::new())
        );
        assert_eq!(
            evaluate("capitalize", &[IrValue::String("É".into())], &[], false)
                .expect("one-character capitalization should succeed"),
            IrValue::String("É".into())
        );
        assert_eq!(
            evaluate(
                "capitalize",
                &[IrValue::String("éclair".into())],
                &[],
                false
            )
            .expect("Unicode capitalization should succeed"),
            IrValue::String("Éclair".into())
        );
        for (input, expected) in [
            ("hello", "Hello"),
            ("Hello", "Hello"),
            ("ǳabc", "ǲabc"),
            ("ßabc", "Ssabc"),
            ("ﬀabc", "Ffabc"),
            ("é—ßabc", "É—ßabc"),
            // `replaceFirstChar` receives a UTF-16 Char, so a supplementary
            // first character is unchanged by the pinned Kotlin contract.
            ("𐐨abc", "𐐨abc"),
            // U+A7D0/U+A7D1 were added after Unicode 13. The pinned JDK 17
            // contract therefore leaves this unmapped character unchanged.
            ("ꟑabc", "ꟑabc"),
        ] {
            assert_eq!(
                evaluate("capitalize", &[IrValue::String(input.into())], &[], false)
                    .expect("titlecase capitalization should succeed"),
                IrValue::String(expected.into()),
                "capitalize({input:?})"
            );
        }
        assert_eq!(
            evaluate("isempty", &[IrValue::String(String::new())], &[], false)
                .expect("empty string should be accepted"),
            IrValue::Boolean(true)
        );
        assert_eq!(
            evaluate("isnotempty", &[IrValue::String(" ".into())], &[], false)
                .expect("whitespace is not empty"),
            IrValue::Boolean(true)
        );
        assert_eq!(
            evaluate("isempty", &[IrValue::String(" ".into())], &[], false)
                .expect("whitespace should not be trimmed"),
            IrValue::Boolean(false)
        );
        for value in ["", " ", "value", "값"] {
            let empty = evaluate("isempty", &[IrValue::String(value.to_string())], &[], false)
                .expect("isempty should evaluate");
            let not_empty = evaluate(
                "isnotempty",
                &[IrValue::String(value.to_string())],
                &[],
                false,
            )
            .expect("isnotempty should evaluate");
            assert_eq!(
                not_empty,
                IrValue::Boolean(!matches!(empty, IrValue::Boolean(true))),
                "predicate complement for {value:?}"
            );
        }
    }

    #[test]
    fn unicode_case_mapping_version_is_pinned_to_jdk_17_data() {
        assert_eq!(
            UNICODE_VERSION, PINNED_JVM_UNICODE_VERSION,
            "case mappings must stay on the pinned JDK 17 Unicode version"
        );
    }

    #[test]
    fn startswith_matches_kotlin_character_case_contract_without_normalization() {
        for (string, prefix, ignorecase, expected) in [
            ("Hello", "He", false, true),
            ("Hello", "he", false, false),
            ("Hello", "", false, true),
            ("Hello", "he", true, true),
            ("Hello", "Ho", true, false),
            ("Hello", "", true, true),
            ("Hi", "Hello", false, false),
            ("Hi", "HELLO", true, false),
            ("Σigma", "ς", true, true),
            ("Σιγμα", "ςΙ", true, true),
            ("ſound", "S", true, true),
            ("İstanbul", "i", true, true),
            // The JVM's regionMatches path compares supplementary code points
            // with simple mappings, without full case-fold expansion.
            ("𐐀nicode", "𐐨", true, true),
            // Full case folding would match this, but Kotlin's character-wise
            // comparison does not expand ß to the two-character string "ss".
            ("ßeta", "ss", true, false),
            // Case comparison does not normalize a decomposed prefix to NFC.
            ("Éclair", "e\u{301}", true, false),
            // U+A7D0/U+A7D1 were added after Unicode 13 and must not acquire
            // a case pair from a newer mapping table.
            ("ꟑabc", "Ꟑ", true, false),
        ] {
            assert_eq!(
                evaluate(
                    "startswith",
                    &[
                        IrValue::String(string.into()),
                        IrValue::String(prefix.into()),
                    ],
                    &[named_arg("ignorecase", IrValue::Boolean(ignorecase))],
                    false,
                )
                .expect("startswith should evaluate"),
                IrValue::Boolean(expected),
                "startswith({string:?}, {prefix:?}, ignorecase={ignorecase})"
            );
        }
    }

    #[test]
    fn string_operations_reject_unsupported_values_and_invalid_bindings() {
        for name in [
            "string",
            "uppercase",
            "lowercase",
            "capitalize",
            "isempty",
            "isnotempty",
            "startswith",
        ] {
            let arguments = if name == "startswith" {
                vec![IrValue::String("value".into()), IrValue::None]
            } else {
                vec![IrValue::None]
            };
            assert!(evaluate(name, &arguments, &[], false).is_err(), "{name}");
        }
        assert!(evaluate("string", &[IrValue::Collection(Vec::new())], &[], false).is_err());
        let rich_content = IrValue::Content(vec![IrNode::Paragraph {
            content: vec![IrInline::Strong {
                content: vec![IrInline::Text {
                    content: "rich".into(),
                    span: scribium_source::SourceSpan::new(scribium_source::SourceId(0), 0, 4),
                }],
                span: scribium_source::SourceSpan::new(scribium_source::SourceId(0), 0, 4),
            }],
            span: scribium_source::SourceSpan::new(scribium_source::SourceId(0), 0, 4),
        }]);
        assert!(evaluate("string", &[rich_content], &[], false).is_err());
        assert!(evaluate(
            "concatenate",
            &[
                IrValue::Identifier("a".into()),
                IrValue::Identifier("b".into())
            ],
            &[named_arg("if", IrValue::Identifier("maybe".into()))],
            false,
        )
        .is_err());
        assert!(evaluate(
            "startswith",
            &[
                IrValue::String("Hello".into()),
                IrValue::String("he".into())
            ],
            &[named_arg("ignorecase", IrValue::Identifier("maybe".into()))],
            false,
        )
        .is_err());
        assert!(evaluate("capitalize", &[], &[], false).is_err());
        assert!(evaluate(
            "capitalize",
            &[IrValue::Identifier("a".into())],
            &[named_arg("string", IrValue::Identifier("b".into()))],
            false,
        )
        .is_err());
        assert!(evaluate("isnotempty", &[IrValue::Identifier("a".into())], &[], true,).is_err());
        assert!(evaluate(
            "concatenate",
            &[
                IrValue::Identifier("a".into()),
                IrValue::Identifier("b".into())
            ],
            &[named_arg("unknown", IrValue::Identifier("c".into()))],
            false,
        )
        .is_err());
        assert!(evaluate(
            "concatenate",
            &[
                IrValue::Identifier("a".into()),
                IrValue::Identifier("b".into())
            ],
            &[named_arg("a", IrValue::Identifier("c".into()))],
            false,
        )
        .is_err());
        assert!(evaluate(
            "startswith",
            &[IrValue::String("a".into()), IrValue::String("b".into())],
            &[named_arg("ignorecase", IrValue::Boolean(false))],
            true,
        )
        .is_err());
    }

    #[test]
    fn logical_surface_is_registered_and_evaluates_typed_results() {
        for name in ["islower", "isgreater", "equals", "not"] {
            assert_eq!(lookup(name).map(|builtin| builtin.name), Some(name));
        }

        assert_eq!(
            evaluate(
                "islower",
                &[number(2.0)],
                &[scribium_ir::IrNamedArg {
                    name: "than".into(),
                    name_span: scribium_source::SourceSpan::new(scribium_source::SourceId(0), 0, 0),
                    value: number(3.0),
                    span: scribium_source::SourceSpan::new(scribium_source::SourceId(0), 0, 0),
                }],
                false,
            )
            .expect("comparison should evaluate"),
            IrValue::Boolean(true)
        );
        assert_eq!(
            evaluate(
                "isgreater",
                &[number(3.0), number(3.0), IrValue::Boolean(true)],
                &[],
                false
            )
            .expect("inclusive comparison should evaluate"),
            IrValue::Boolean(true)
        );
        assert_eq!(
            evaluate("not", &[IrValue::Identifier("yes".into())], &[], false)
                .expect("boolean negation should evaluate"),
            IrValue::Boolean(false)
        );
    }

    #[test]
    fn equality_preserves_types_and_uses_upstream_plain_text_fallback() {
        assert_eq!(
            evaluate(
                "equals",
                &[number(2.0), IrValue::String("2".into())],
                &[],
                false
            )
            .expect("numeric text equality should evaluate"),
            IrValue::Boolean(true)
        );
        assert_eq!(
            evaluate(
                "equals",
                &[
                    IrValue::Content(vec![IrNode::Paragraph {
                        content: vec![IrInline::Strong {
                            content: vec![IrInline::Text {
                                content: "same".into(),
                                span: scribium_source::SourceSpan::new(
                                    scribium_source::SourceId(0),
                                    0,
                                    4,
                                ),
                            }],
                            span: scribium_source::SourceSpan::new(
                                scribium_source::SourceId(0),
                                0,
                                4,
                            ),
                        }],
                        span: scribium_source::SourceSpan::new(scribium_source::SourceId(0), 0, 4),
                    }]),
                    IrValue::String("same".into()),
                ],
                &[],
                false,
            )
            .expect("rich plain-text equality should evaluate"),
            IrValue::Boolean(true)
        );
        assert_eq!(
            evaluate(
                "equals",
                &[IrValue::Boolean(true), IrValue::String("true".into())],
                &[],
                false,
            )
            .expect("typed equality should evaluate"),
            IrValue::Boolean(false)
        );
    }

    #[test]
    fn logical_builtins_reject_invalid_values_and_duplicate_bindings() {
        assert!(evaluate("not", &[number(1.0)], &[], false).is_err());
        assert!(evaluate(
            "islower",
            &[number(1.0), IrValue::Boolean(true)],
            &[],
            false
        )
        .is_err());
        assert!(evaluate("equals", &[number(1.0)], &[], false).is_err());
        assert!(evaluate("equals", &[number(1.0), number(1.0)], &[], true).is_err());
    }
}
