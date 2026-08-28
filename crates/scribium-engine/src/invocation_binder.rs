//! Shared invocation binding for native, source-defined, and callback calls.
//!
//! Binding is deliberately independent of value conversion. The binder only
//! assigns source-ordered candidates to declared slots and applies omission
//! and body policy; callers select and perform target-driven conversion after
//! this step succeeds.

use scribium_source::SourceSpan;
use std::collections::BTreeMap;

/// Whether a parameter must be supplied or may be omitted by the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OmissionPolicy {
    Required,
    Optional,
    Default,
}

/// Metadata consumed by the shared binder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ParameterMetadata<'a> {
    pub(crate) name: &'a str,
    pub(crate) aliases: &'a [&'a str],
    pub(crate) allows_named: bool,
    pub(crate) omission: OmissionPolicy,
    pub(crate) name_span: Option<SourceSpan>,
}

impl<'a> ParameterMetadata<'a> {
    pub(crate) const fn required(name: &'a str) -> Self {
        Self {
            name,
            aliases: &[],
            allows_named: true,
            omission: OmissionPolicy::Required,
            name_span: None,
        }
    }

    pub(crate) const fn optional(name: &'a str) -> Self {
        Self {
            name,
            aliases: &[],
            allows_named: true,
            omission: OmissionPolicy::Optional,
            name_span: None,
        }
    }

    pub(crate) const fn defaulted(name: &'a str) -> Self {
        Self {
            name,
            aliases: &[],
            allows_named: true,
            omission: OmissionPolicy::Default,
            name_span: None,
        }
    }

    pub(crate) const fn with_aliases(mut self, aliases: &'a [&'a str]) -> Self {
        self.aliases = aliases;
        self
    }

    pub(crate) const fn named(mut self, allowed: bool) -> Self {
        self.allows_named = allowed;
        self
    }

    pub(crate) const fn with_name_span(mut self, span: SourceSpan) -> Self {
        self.name_span = Some(span);
        self
    }
}

/// Body binding behavior for one invocation target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BodyPolicy {
    Reject,
    BindFinal,
    /// The target owns a separate body contract (for example a Markdown
    /// layout body or an iteration callback). The binder validates explicit
    /// slots while leaving body interpretation to that target adapter.
    AllowSeparate,
}

/// One source-ordered invocation candidate.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Candidate<T> {
    Positional {
        value: T,
        span: SourceSpan,
    },
    Named {
        name: String,
        name_span: SourceSpan,
        value: T,
        span: SourceSpan,
    },
}

impl<T> Candidate<T> {
    fn span(&self) -> SourceSpan {
        match self {
            Self::Positional { span, .. } | Self::Named { span, .. } => *span,
        }
    }
}

/// The result of binding one invocation.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BoundInvocation<T> {
    pub(crate) slots: Vec<BoundSlot<T>>,
}

/// A structural binding decision that can be carried across candidate
/// evaluation. The plan owns slot selection; applying it later must not
/// repeat name lookup, duplicate detection, or positional assignment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BindingPlan {
    slots: Vec<PlannedSlot>,
    candidate_count: usize,
    binds_body: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlannedSlot {
    Candidate(usize),
    Body,
    Omitted,
    Defaulted,
}

/// A bound slot retains omission/default classification instead of collapsing
/// it into an explicit `None` value.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum BoundSlot<T> {
    Explicit { value: T, span: SourceSpan },
    Omitted,
    Defaulted,
}

/// A source-backed binding failure. Diagnostic code and user-facing hint are
/// intentionally kept here so every invocation path reports the same
/// structural rule with the same provenance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BindingError {
    pub(crate) message: String,
    pub(crate) primary: SourceSpan,
    pub(crate) secondary: Vec<SourceSpan>,
    pub(crate) hint: String,
}

/// Validate the source-order invariants that do not depend on a target's
/// parameter table. This is used at the call boundary before any candidate is
/// evaluated, so invalid mixed calls cannot execute nested values or bodies.
pub(crate) fn validate_order<T>(candidates: &[Candidate<T>]) -> Result<(), BindingError> {
    let mut named_started = None;
    let mut seen_named = BTreeMap::new();
    for candidate in candidates {
        match candidate {
            Candidate::Positional { span, .. } => {
                if let Some(named_span) = named_started {
                    return Err(BindingError {
                        message: "positional argument after named argument is not allowed"
                            .to_string(),
                        primary: *span,
                        secondary: vec![named_span],
                        hint: "Move all positional arguments before named arguments.".to_string(),
                    });
                }
            }
            Candidate::Named {
                name,
                name_span,
                span,
                ..
            } => {
                named_started.get_or_insert(*span);
                if let Some(previous_span) = seen_named.insert(name.as_str(), *span) {
                    return Err(BindingError {
                        message: format!("named argument `{name}` was supplied more than once"),
                        primary: *name_span,
                        secondary: vec![previous_span],
                        hint: "Use one value for each named parameter.".to_string(),
                    });
                }
            }
        }
    }
    Ok(())
}

/// Validate a headerless callable's positional-only invocation contract.
/// Such callables have no finite parameter table, but they still share the
/// source-order and named-argument rules with every other invocation path.
pub(crate) fn validate_implicit<T>(candidates: &[Candidate<T>]) -> Result<(), BindingError> {
    validate_order(candidates)?;
    if let Some(name_span) = candidates.iter().find_map(|candidate| match candidate {
        Candidate::Named { name_span, .. } => Some(*name_span),
        Candidate::Positional { .. } => None,
    }) {
        return Err(BindingError {
            message: "implicit lambda parameters are positional only".to_string(),
            primary: name_span,
            secondary: Vec::new(),
            hint: "Pass implicit lambda values positionally.".to_string(),
        });
    }
    Ok(())
}

/// Build the structural binding decision for one ordered candidate sequence.
pub(crate) fn plan<T>(
    parameters: &[ParameterMetadata<'_>],
    candidates: &[Candidate<T>],
    body: Option<&Candidate<T>>,
    body_policy: BodyPolicy,
    call_span: SourceSpan,
) -> Result<BindingPlan, BindingError> {
    validate_order(candidates)?;
    let mut named_parameters = BTreeMap::new();
    for (index, parameter) in parameters.iter().enumerate() {
        named_parameters.entry(parameter.name).or_insert(index);
        for alias in parameter.aliases {
            named_parameters.entry(*alias).or_insert(index);
        }
    }
    let mut slots: Vec<Option<PlannedSlot>> = vec![None; parameters.len()];
    let mut bound_spans: Vec<Option<SourceSpan>> = vec![None; parameters.len()];
    let mut next_positional = 0;

    for (candidate_index, candidate) in candidates.iter().enumerate() {
        match candidate {
            Candidate::Positional { span, .. } => {
                while next_positional < parameters.len() && bound_spans[next_positional].is_some() {
                    next_positional += 1;
                }
                let index = next_positional;
                if index == parameters.len() {
                    return Err(BindingError {
                        message: "received too many positional arguments".to_string(),
                        primary: *span,
                        secondary: Vec::new(),
                        hint: "Remove the excess positional argument or bind it to a declared parameter.".to_string(),
                    });
                }
                slots[index] = Some(PlannedSlot::Candidate(candidate_index));
                bound_spans[index] = Some(*span);
                next_positional += 1;
            }
            Candidate::Named {
                name,
                name_span,
                span,
                ..
            } => {
                let Some(&index) = named_parameters.get(name.as_str()) else {
                    return Err(BindingError {
                        message: format!("unknown named argument `{name}`"),
                        primary: *name_span,
                        secondary: Vec::new(),
                        hint: "Use the parameter's declared name or an explicitly supported alias."
                            .to_string(),
                    });
                };
                let parameter = &parameters[index];
                if !parameter.allows_named {
                    return Err(BindingError {
                        message: format!("named argument `{name}` is not supported"),
                        primary: *name_span,
                        secondary: Vec::new(),
                        hint: "Pass this value positionally.".to_string(),
                    });
                }
                if let Some(previous_span) = bound_spans[index] {
                    return Err(BindingError {
                        message: format!(
                            "parameter `{}` collides with an already bound argument",
                            parameter.name
                        ),
                        primary: *span,
                        secondary: vec![previous_span],
                        hint: "Remove the positional or named value that already fills this parameter.".to_string(),
                    });
                }
                slots[index] = Some(PlannedSlot::Candidate(candidate_index));
                bound_spans[index] = Some(*span);
            }
        }
    }

    match (body, body_policy) {
        (Some(body), BodyPolicy::Reject) => {
            return Err(BindingError {
                message: "this invocation does not accept a body".to_string(),
                primary: body.span(),
                secondary: Vec::new(),
                hint: "Remove the body or use the target's supported body form.".to_string(),
            });
        }
        (Some(body), BodyPolicy::BindFinal) => {
            let Some(index) = parameters.len().checked_sub(1) else {
                return Err(BindingError {
                    message: "a body requires a final parameter".to_string(),
                    primary: body.span(),
                    secondary: Vec::new(),
                    hint: "Add a final body parameter or remove the body.".to_string(),
                });
            };
            if let Some(previous_span) = bound_spans[index] {
                return Err(BindingError {
                    message: format!(
                        "parameter `{}` collides with an already bound argument",
                        parameters[index].name
                    ),
                    primary: body.span(),
                    secondary: vec![previous_span],
                    hint: "Remove the final explicit value when using a body fallback.".to_string(),
                });
            }
            slots[index] = Some(PlannedSlot::Body);
            bound_spans[index] = Some(body.span());
        }
        (Some(_), BodyPolicy::AllowSeparate) | (None, BodyPolicy::AllowSeparate) => {}
        (None, BodyPolicy::Reject | BodyPolicy::BindFinal) => {}
    }

    let mut planned = Vec::with_capacity(parameters.len());
    for (index, parameter) in parameters.iter().enumerate() {
        match slots[index].take() {
            Some(source) => planned.push(source),
            None => match parameter.omission {
                OmissionPolicy::Required => {
                    return Err(BindingError {
                        message: format!("missing required argument `{}`", parameter.name),
                        primary: parameter.name_span.unwrap_or(call_span),
                        secondary: Vec::new(),
                        hint: "Provide a value for every required parameter.".to_string(),
                    });
                }
                OmissionPolicy::Optional => planned.push(PlannedSlot::Omitted),
                OmissionPolicy::Default => planned.push(PlannedSlot::Defaulted),
            },
        }
    }
    Ok(BindingPlan {
        slots: planned,
        candidate_count: candidates.len(),
        binds_body: matches!(body_policy, BodyPolicy::BindFinal) && body.is_some(),
    })
}

impl BindingPlan {
    /// Apply a previously validated structural plan to evaluated candidates.
    /// This is deliberately the only post-evaluation slot assignment path.
    pub(crate) fn bind<T: Clone>(
        &self,
        candidates: &[Candidate<T>],
        body: Option<&Candidate<T>>,
        call_span: SourceSpan,
    ) -> Result<BoundInvocation<T>, BindingError> {
        if candidates.len() != self.candidate_count || body.is_some() != self.binds_body {
            return Err(plan_mismatch(call_span));
        }
        let mut slots = Vec::with_capacity(self.slots.len());
        for source in &self.slots {
            match source {
                PlannedSlot::Candidate(index) => {
                    let Some(candidate) = candidates.get(*index) else {
                        return Err(plan_mismatch(call_span));
                    };
                    let (value, span) = match candidate {
                        Candidate::Positional { value, span }
                        | Candidate::Named { value, span, .. } => (value.clone(), *span),
                    };
                    slots.push(BoundSlot::Explicit { value, span });
                }
                PlannedSlot::Body => {
                    let Some(candidate) = body else {
                        return Err(plan_mismatch(call_span));
                    };
                    let (value, span) = match candidate {
                        Candidate::Positional { value, span }
                        | Candidate::Named { value, span, .. } => (value.clone(), *span),
                    };
                    slots.push(BoundSlot::Explicit { value, span });
                }
                PlannedSlot::Omitted => slots.push(BoundSlot::Omitted),
                PlannedSlot::Defaulted => slots.push(BoundSlot::Defaulted),
            }
        }
        Ok(BoundInvocation { slots })
    }
}

fn plan_mismatch(call_span: SourceSpan) -> BindingError {
    BindingError {
        message: "evaluated invocation did not match its binding plan".to_string(),
        primary: call_span,
        secondary: Vec::new(),
        hint: "Keep the invocation candidates unchanged after binding validation.".to_string(),
    }
}

/// Bind one ordered candidate sequence against one explicit signature.
pub(crate) fn bind<T: Clone>(
    parameters: &[ParameterMetadata<'_>],
    candidates: &[Candidate<T>],
    body: Option<Candidate<T>>,
    body_policy: BodyPolicy,
    call_span: SourceSpan,
) -> Result<BoundInvocation<T>, BindingError> {
    let plan = plan(
        parameters,
        candidates,
        body.as_ref(),
        body_policy,
        call_span,
    )?;
    plan.bind(candidates, body.as_ref(), call_span)
}

#[cfg(test)]
mod tests {
    use super::*;
    use scribium_source::{SourceId, SourceSpan};

    fn span(start: usize, end: usize) -> SourceSpan {
        SourceSpan::new(SourceId(1), start, end)
    }

    fn positional(value: u8, start: usize) -> Candidate<u8> {
        Candidate::Positional {
            value,
            span: span(start, start + 1),
        }
    }

    fn named(name: &str, value: u8, start: usize) -> Candidate<u8> {
        Candidate::Named {
            name: name.to_string(),
            name_span: span(start, start + 1),
            value,
            span: span(start, start + 2),
        }
    }

    #[test]
    fn binds_mixed_arguments_in_source_order_and_preserves_omission() {
        let parameters = [
            ParameterMetadata::required("first"),
            ParameterMetadata::optional("second"),
        ];
        let bound = bind(
            &parameters,
            &[positional(1, 10), named("second", 2, 20)],
            None,
            BodyPolicy::Reject,
            span(0, 30),
        )
        .expect("valid binding");
        assert_eq!(
            bound.slots,
            [
                BoundSlot::Explicit {
                    value: 1,
                    span: span(10, 11)
                },
                BoundSlot::Explicit {
                    value: 2,
                    span: span(20, 22)
                }
            ]
        );

        let omitted = bind(
            &parameters,
            &[positional(1, 10)],
            None,
            BodyPolicy::Reject,
            span(0, 30),
        )
        .expect("optional omission");
        assert_eq!(
            omitted.slots,
            [
                BoundSlot::Explicit {
                    value: 1,
                    span: span(10, 11)
                },
                BoundSlot::Omitted
            ]
        );

        let defaulted = bind::<u8>(
            &[ParameterMetadata::defaulted("value")],
            &[],
            None,
            BodyPolicy::Reject,
            span(0, 30),
        )
        .expect("default omission");
        assert_eq!(defaulted.slots, [BoundSlot::Defaulted]);
    }

    #[test]
    fn preserves_explicit_none_and_explicit_alias_metadata() {
        let parameters = [ParameterMetadata::optional("value").with_aliases(&["type"])];
        let explicit_none = bind(
            &parameters,
            &[Candidate::Positional {
                value: None::<u8>,
                span: span(10, 15),
            }],
            None,
            BodyPolicy::Reject,
            span(0, 20),
        )
        .expect("explicit value");
        assert_eq!(
            explicit_none.slots,
            [BoundSlot::Explicit {
                value: None,
                span: span(10, 15)
            }]
        );

        let aliased = bind(
            &parameters,
            &[named("type", 1, 20)],
            None,
            BodyPolicy::Reject,
            span(0, 30),
        )
        .expect("explicit alias");
        assert_eq!(
            aliased.slots[0],
            BoundSlot::Explicit {
                value: 1,
                span: span(20, 22)
            }
        );
    }

    #[test]
    fn reports_conflict_and_body_provenance() {
        let parameters = [ParameterMetadata::required("value")];
        let conflict = bind(
            &parameters,
            &[positional(1, 10), named("value", 2, 20)],
            None,
            BodyPolicy::Reject,
            span(0, 30),
        )
        .expect_err("collision");
        assert_eq!(conflict.primary, span(20, 22));
        assert_eq!(conflict.secondary, [span(10, 11)]);

        let body = bind(
            &parameters,
            &[],
            Some(Candidate::Positional {
                value: 3,
                span: span(30, 35),
            }),
            BodyPolicy::Reject,
            span(0, 40),
        )
        .expect_err("body rejection");
        assert_eq!(body.primary, span(30, 35));

        let duplicate = bind(
            &[
                ParameterMetadata::optional("value"),
                ParameterMetadata::optional("other"),
            ],
            &[named("value", 1, 40), named("value", 2, 50)],
            None,
            BodyPolicy::Reject,
            span(0, 60),
        )
        .expect_err("duplicate named argument");
        assert_eq!(duplicate.primary, span(50, 51));
        assert_eq!(duplicate.secondary, [span(40, 42)]);
    }

    #[test]
    fn rejects_order_duplicates_collisions_unknown_and_missing() {
        let parameters = [
            ParameterMetadata::required("first"),
            ParameterMetadata::required("second"),
        ];
        let cases = [
            (
                &[named("first", 1, 10), positional(2, 20)][..],
                "positional argument after named",
            ),
            (
                &[named("first", 1, 10), named("first", 2, 20)][..],
                "more than once",
            ),
            (&[positional(1, 10), named("first", 2, 20)][..], "collides"),
            (&[named("unknown", 1, 10)][..], "unknown named"),
            (
                &[positional(1, 10), positional(2, 20), positional(3, 30)][..],
                "too many",
            ),
            (&[positional(1, 10)][..], "missing required"),
        ];
        for (candidates, expected) in cases {
            let error = bind(
                &parameters,
                candidates,
                None,
                BodyPolicy::Reject,
                span(0, 40),
            )
            .expect_err("invalid binding");
            assert!(error.message.contains(expected), "{}: {error:?}", expected);
        }
    }

    #[test]
    fn body_fallback_is_a_binding_policy() {
        let parameters = [ParameterMetadata::required("content")];
        let bound = bind(
            &parameters,
            &[],
            Some(positional(7, 10)),
            BodyPolicy::BindFinal,
            span(0, 20),
        )
        .expect("body fallback");
        assert_eq!(
            bound.slots,
            [BoundSlot::Explicit {
                value: 7,
                span: span(10, 11)
            }]
        );
    }

    #[test]
    fn binding_plan_is_reused_for_evaluated_values() {
        let parameters = [
            ParameterMetadata::required("first"),
            ParameterMetadata::optional("second"),
        ];
        let structural = [
            Candidate::Positional {
                value: (),
                span: span(10, 13),
            },
            Candidate::Named {
                name: "second".to_string(),
                name_span: span(20, 26),
                value: (),
                span: span(20, 29),
            },
        ];
        let plan = plan(
            &parameters,
            &structural,
            None,
            BodyPolicy::Reject,
            span(0, 30),
        )
        .expect("structural plan");
        let evaluated = [positional(11, 10), named("second", 22, 20)];
        let bound = plan
            .bind(&evaluated, None, span(0, 30))
            .expect("evaluated values follow the plan");
        assert_eq!(
            bound.slots,
            [
                BoundSlot::Explicit {
                    value: 11,
                    span: span(10, 11)
                },
                BoundSlot::Explicit {
                    value: 22,
                    span: span(20, 22)
                },
            ]
        );
    }

    #[test]
    fn headerless_callable_binding_rejects_named_values() {
        let error = validate_implicit(&[named("value", 1, 10)])
            .expect_err("headerless callable parameters are positional only");
        assert_eq!(error.primary, span(10, 11));
        assert!(error.message.contains("positional only"));
    }
}
