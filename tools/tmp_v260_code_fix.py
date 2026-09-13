from pathlib import Path

p = Path("crates/arkst-engine/src/evaluator.rs")
text = p.read_text(encoding="utf-8")
start = text.index("    #[allow(clippy::too_many_arguments)]\n    fn evaluate_code(")
end = text.index("    #[allow(clippy::too_many_arguments)]\n    fn evaluate_center(", start)
method = r'''    #[allow(clippy::too_many_arguments)]
    fn evaluate_code(
        &self,
        _ordered_args: Option<&[IrCallArgument]>,
        positional_args: &[IrValue],
        named_args: &[IrNamedArg],
        body: Option<CallBody<'_>>,
        raw_body: Option<&IrRawBody>,
        lambda_parameters: Option<&[IrParameter]>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
        binding_plan: Option<&BindingPlan>,
        _first_origin: Option<ValueOrigin>,
        implicit_argument: Option<&InvocationValue>,
    ) -> CallOutcome {
        let Some(binding_plan) = binding_plan else {
            return CallOutcome::Failed;
        };
        if implicit_argument.is_some() {
            diagnostics.push(chain_evaluation_error(
                "chained input into `.code` is outside the bounded v2.6 callouts slice".to_string(),
                *span,
            ));
            return CallOutcome::Failed;
        }
        if let Some(parameters) = lambda_parameters {
            diagnostics.push(chain_evaluation_error(
                "`.code` body is an evaluable string, not a lambda".to_string(),
                parameters.first().map_or(*span, |parameter| parameter.span),
            ));
            return CallOutcome::Failed;
        }

        let candidates = raw_invocation_locations(positional_args, named_args, *span);
        let body_candidate = body.map(|body| Candidate::Positional {
            value: InvocationArgumentLocation::Body,
            span: call_body_source_span(body, *span),
        });
        let bound = match binding_plan.bind(&candidates, body_candidate.as_ref(), *span) {
            Ok(bound) => bound,
            Err(error) => {
                diagnostics.push(binding_diagnostic_with_code(error, "E3001"));
                return CallOutcome::Failed;
            }
        };

        macro_rules! evaluate_location {
            ($location:expr) => {{
                match $location {
                    InvocationArgumentLocation::Body => {
                        let Some(raw_body) = raw_body else {
                            diagnostics.push(chain_evaluation_error(
                                "`.code` body requires source-backed text".to_string(),
                                *span,
                            ));
                            return CallOutcome::Failed;
                        };
                        let Some(text) = value_conversion::raw_body_dynamic_text(raw_body) else {
                            diagnostics.push(chain_evaluation_error(
                                "`.code` body has an invalid source span".to_string(),
                                *span,
                            ));
                            return CallOutcome::Failed;
                        };
                        InvocationValue::dynamic_value(IrValue::String(text))
                    }
                    InvocationArgumentLocation::Positional(index) => {
                        let Some(source) = positional_args.get(index) else {
                            return CallOutcome::Failed;
                        };
                        let value = match self.evaluate_value(source, diagnostics, context) {
                            CallOutcome::Value(value) => value,
                            CallOutcome::Unresolved => {
                                match self.preserve_value_expression(source, diagnostics, context) {
                                    Ok(value) => value,
                                    Err(outcome) => return outcome,
                                }
                            }
                            CallOutcome::NoValue => {
                                diagnostics.push(no_value_required(value_source_span(source, span)));
                                return CallOutcome::Failed;
                            }
                            CallOutcome::Failed => return CallOutcome::Failed,
                        };
                        InvocationValue::dynamic_value(value)
                    }
                    InvocationArgumentLocation::Named(index) => {
                        let Some(argument) = named_args.get(index) else {
                            return CallOutcome::Failed;
                        };
                        let value = match self.evaluate_value(&argument.value, diagnostics, context) {
                            CallOutcome::Value(value) => value,
                            CallOutcome::Unresolved => {
                                match self.preserve_value_expression(&argument.value, diagnostics, context) {
                                    Ok(value) => value,
                                    Err(outcome) => return outcome,
                                }
                            }
                            CallOutcome::NoValue => {
                                diagnostics.push(no_value_required(value_source_span(&argument.value, span)));
                                return CallOutcome::Failed;
                            }
                            CallOutcome::Failed => return CallOutcome::Failed,
                        };
                        InvocationValue::dynamic_value(value)
                    }
                }
            }};
        }

        macro_rules! raw_value {
            ($location:expr) => {{
                match $location {
                    InvocationArgumentLocation::Positional(index) => positional_args.get(index),
                    InvocationArgumentLocation::Named(index) => {
                        named_args.get(index).map(|argument| &argument.value)
                    }
                    InvocationArgumentLocation::Body => None,
                }
            }};
        }

        let language = match bound.slots.first() {
            Some(BoundSlot::Explicit { value: location, span: argument_span }) => {
                let value = evaluate_location!(*location);
                if matches!(value.value, IrValue::None) {
                    None
                } else {
                    match value_conversion::convert_scalar_with_origin(&value, ScalarTarget::String) {
                        Ok(ScalarValue::String(value)) => Some(value),
                        _ => {
                            diagnostics.push(chain_evaluation_error(
                                "`.code` `lang` must be text or None".to_string(),
                                *argument_span,
                            ));
                            return CallOutcome::Failed;
                        }
                    }
                }
            }
            Some(BoundSlot::Omitted | BoundSlot::Defaulted) | None => None,
        };

        for (index, parameter) in [(1usize, "caption"), (3, "focus"), (5, "ref")] {
            if let Some(BoundSlot::Explicit { value: location, span: argument_span }) = bound.slots.get(index) {
                let literal_none = raw_value!(*location).is_some_and(|value| matches!(value, IrValue::None));
                if !literal_none {
                    diagnostics.push(chain_evaluation_error(
                        format!("`.code` parameter `{parameter}` remains outside the bounded v2.6 callouts slice"),
                        *argument_span,
                    ));
                    return CallOutcome::Failed;
                }
            }
        }

        let line_numbers = match bound.slots.get(2) {
            Some(BoundSlot::Explicit { value: location, span: argument_span }) => {
                let value = evaluate_location!(*location);
                match value_conversion::convert_scalar_with_origin(&value, ScalarTarget::Boolean) {
                    Ok(ScalarValue::Boolean(value)) => value,
                    _ => {
                        diagnostics.push(chain_evaluation_error(
                            "`.code` `linenumbers` must be Boolean".to_string(),
                            *argument_span,
                        ));
                        return CallOutcome::Failed;
                    }
                }
            }
            Some(BoundSlot::Defaulted) | Some(BoundSlot::Omitted) | None => true,
        };

        let callouts = match bound.slots.get(4) {
            Some(BoundSlot::Explicit { value: location, span: argument_span }) => {
                let raw = raw_value!(*location);
                let value = if let Some(raw) = raw.filter(|value| code_value_text(value).is_some()) {
                    InvocationValue::dynamic_value(raw.clone())
                } else {
                    evaluate_location!(*location)
                };
                match parse_code_callouts(&value.value, *argument_span) {
                    Ok(callouts) => callouts,
                    Err(message) => {
                        diagnostics.push(chain_evaluation_error(message, *argument_span));
                        return CallOutcome::Failed;
                    }
                }
            }
            Some(BoundSlot::Defaulted) | Some(BoundSlot::Omitted) | None => Vec::new(),
        };

        let source = match bound.slots.get(6) {
            Some(BoundSlot::Explicit { value: location, span: argument_span }) => {
                let value = evaluate_location!(*location);
                let Some(text) = code_value_text(&value.value) else {
                    diagnostics.push(chain_evaluation_error(
                        "`.code` `code` must resolve to evaluable text".to_string(),
                        *argument_span,
                    ));
                    return CallOutcome::Failed;
                };
                text
            }
            _ => {
                diagnostics.push(chain_evaluation_error(
                    "`.code` requires `code`".to_string(),
                    *span,
                ));
                return CallOutcome::Failed;
            }
        };

        CallOutcome::Value(IrValue::Content(vec![IrNode::CodeBlock {
            language,
            info: None,
            source,
            line_numbers: Some(line_numbers),
            callouts,
            span: *span,
        }]))
    }

'''
p.write_text(text[:start] + method + text[end:], encoding="utf-8")
