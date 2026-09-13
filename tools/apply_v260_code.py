from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def replace_once(path: str, old: str, new: str) -> None:
    p = ROOT / path
    text = p.read_text(encoding="utf-8")
    count = text.count(old)
    assert count == 1, (path, count, old[:160])
    p.write_text(text.replace(old, new, 1), encoding="utf-8")


# IR: keep fenced Markdown and `.code` as one semantic CodeBlock node, with
# optional presentation metadata so old wire payloads remain compatible.
ir = ROOT / "crates/arkst-ir/src/lib.rs"
text = ir.read_text(encoding="utf-8")
marker = "#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]\npub enum IrNode {"
assert marker in text
addition = r'''#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct IrCodeCallout {
    /// 1-based source line associated with this callout.
    pub line: u32,
    /// Plain-text callout description after Quarkdown-compatible projection.
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct IrCodePresentation {
    pub line_numbers: bool,
    pub focus: Option<IrRange>,
    #[serde(default)]
    pub callouts: Vec<IrCodeCallout>,
}

'''
text = text.replace(marker, addition + marker, 1)
old = '''    CodeBlock {
        language: Option<String>,
        info: Option<String>,
        source: String,
        span: SourceSpan,
    },'''
new_ir = '''    CodeBlock {
        language: Option<String>,
        info: Option<String>,
        source: String,
        /// Presentation metadata is present for the `.code` primitive and
        /// absent for ordinary Markdown fenced code blocks.
        presentation: Option<IrCodePresentation>,
        span: SourceSpan,
    },'''
assert text.count(old) >= 2
# First occurrence in WireNode needs serde default, second in IrNode does not.
wire = '''    CodeBlock {
        language: Option<String>,
        info: Option<String>,
        source: String,
        #[serde(default)]
        presentation: Option<IrCodePresentation>,
        span: SourceSpan,
    },'''
text = text.replace(old, wire, 1)
text = text.replace(old, new_ir, 1)
old = '''        IrNode::CodeBlock {
            language,
            info,
            source,
            span,
        } => WireNode::CodeBlock {
            language: language.clone(),
            info: info.clone(),
            source: source.clone(),
            span: *span,
        },'''
new = '''        IrNode::CodeBlock {
            language,
            info,
            source,
            presentation,
            span,
        } => WireNode::CodeBlock {
            language: language.clone(),
            info: info.clone(),
            source: source.clone(),
            presentation: presentation.clone(),
            span: *span,
        },'''
assert old in text
text = text.replace(old, new, 1)
old = '''        WireNode::CodeBlock {
            language,
            info,
            source,
            span,
        } => IrNode::CodeBlock {
            language,
            info,
            source,
            span,
        },'''
new = '''        WireNode::CodeBlock {
            language,
            info,
            source,
            presentation,
            span,
        } => IrNode::CodeBlock {
            language,
            info,
            source,
            presentation,
            span,
        },'''
assert old in text
text = text.replace(old, new, 1)
ir.write_text(text, encoding="utf-8")

# Existing Markdown producer explicitly has no `.code` presentation metadata.
replace_once(
    "crates/arkst-engine/src/ast_to_ir.rs",
    '''        } => Some(IrNode::CodeBlock {
            language: language.clone(),
            info: info.clone(),
            source: source.clone(),
            span: *span,
        }),''',
    '''        } => Some(IrNode::CodeBlock {
            language: language.clone(),
            info: info.clone(),
            source: source.clone(),
            presentation: None,
            span: *span,
        }),''',
)

# Evaluator ownership/signature/dispatch and semantic producer.
ev = ROOT / "crates/arkst-engine/src/evaluator.rs"
text = ev.read_text(encoding="utf-8")
text = text.replace(
    "IrCaptionPositionInfo, IrCapturedFunction, IrCapturedVariable, IrComponent,",
    "IrCaptionPositionInfo, IrCapturedFunction, IrCapturedVariable, IrCodeCallout, IrCodePresentation, IrComponent,",
    1,
)
text = text.replace("    Markdown,\n    Resource,", "    Markdown,\n    CodePresentation,\n    Resource,", 1)
text = text.replace(
    'const MARKDOWN_NATIVE_NAMES: &[&str] = &["markdown"];',
    'const MARKDOWN_NATIVE_NAMES: &[&str] = &["markdown"];\nconst CODE_PRESENTATION_NATIVE_NAMES: &[&str] = &["code"];',
    1,
)
text = text.replace(
    '''    NativeOwnerInventory {
        owner: NativeDispatchOwner::Markdown,
        names: MARKDOWN_NATIVE_NAMES,
    },''',
    '''    NativeOwnerInventory {
        owner: NativeDispatchOwner::Markdown,
        names: MARKDOWN_NATIVE_NAMES,
    },
    NativeOwnerInventory {
        owner: NativeDispatchOwner::CodePresentation,
        names: CODE_PRESENTATION_NATIVE_NAMES,
    },''',
    1,
)
text = text.replace(
    '''fn is_markdown(name: &str) -> bool {
    has_native_owner(name, NativeDispatchOwner::Markdown)
}

fn is_resource''',
    '''fn is_markdown(name: &str) -> bool {
    has_native_owner(name, NativeDispatchOwner::Markdown)
}

fn is_code_presentation(name: &str) -> bool {
    has_native_owner(name, NativeDispatchOwner::CodePresentation)
}

fn is_resource''',
    1,
)
text = text.replace(
    '''        "html" | "markdown" => (
            vec![ParameterMetadata::required("content")],
            BodyPolicy::BindFinal,
        ),
        "read" => (''',
    '''        "html" | "markdown" => (
            vec![ParameterMetadata::required("content")],
            BodyPolicy::BindFinal,
        ),
        "code" => (
            vec![
                ParameterMetadata::optional("lang"),
                ParameterMetadata::optional("caption"),
                ParameterMetadata::defaulted("linenumbers"),
                ParameterMetadata::optional("focus"),
                ParameterMetadata::defaulted("callouts"),
                ParameterMetadata::optional("ref"),
                ParameterMetadata::required("code"),
            ],
            BodyPolicy::BindFinal,
        ),
        "read" => (''',
    1,
)
text = text.replace(
    '''        if is_markdown(name) {
            return self.evaluate_markdown(
                positional_args,
                named_args,
                body,
                raw_body,
                span,
                diagnostics,
                context,
                native_binding_plan.as_ref(),
            );
        }

        // Adding a native resource name''',
    '''        if is_markdown(name) {
            return self.evaluate_markdown(
                positional_args,
                named_args,
                body,
                raw_body,
                span,
                diagnostics,
                context,
                native_binding_plan.as_ref(),
            );
        }

        // `.code` is a newly implemented producer. Preserve Arkst's established
        // source-defined-before-new-native precedence so adding compatibility
        // support cannot steal a previously valid user function.
        if is_code_presentation(name) && context.get_function(name).is_none() {
            return self.evaluate_code_presentation(
                positional_args,
                named_args,
                body,
                raw_body,
                span,
                diagnostics,
                context,
                native_binding_plan.as_ref(),
                first_origin,
            );
        }

        // Adding a native resource name''',
    1,
)
method_anchor = '''    /// Evaluates Quarkdown's raw native Markdown-content builtin. This is
    /// intentionally not a file loader: the v2.5.1 contract accepts Markdown
    /// content and returns an opaque native-content node.
'''
assert method_anchor in text
method = r'''    #[allow(clippy::too_many_arguments)]
    fn evaluate_code_presentation(
        &self,
        positional_args: &[IrValue],
        named_args: &[IrNamedArg],
        body: Option<CallBody<'_>>,
        raw_body: Option<&IrRawBody>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
        binding_plan: Option<&BindingPlan>,
        first_origin: Option<ValueOrigin>,
    ) -> CallOutcome {
        let Some(binding_plan) = binding_plan else {
            return CallOutcome::Failed;
        };
        let positional_values = match self.evaluate_invocation_values(
            positional_args,
            span,
            diagnostics,
            context,
            first_origin,
        ) {
            Ok(values) => values,
            Err(outcome) => return outcome,
        };
        let named_values = match self.evaluate_invocation_named(named_args, span, diagnostics, context)
        {
            Ok(values) => values,
            Err(outcome) => return outcome,
        };
        let candidates = invocation_candidates(
            positional_values
                .into_iter()
                .zip(positional_args.iter())
                .map(|(value, source)| (value, value_source_span(source, span)))
                .collect(),
            named_values,
        );
        let body_candidate = match body {
            Some(body) => {
                let Some(raw_body) = raw_body else {
                    diagnostics.push(resource_diagnostic(
                        "E3003",
                        "`.code` block body must preserve its raw source text".to_string(),
                        call_body_source_span(body, *span),
                        "Use the ordinary `.code` block-body form or pass the final `code` argument explicitly.",
                    ));
                    return CallOutcome::Failed;
                };
                let Some(code) = value_conversion::raw_body_dynamic_text(raw_body) else {
                    diagnostics.push(resource_diagnostic(
                        "E3003",
                        "`.code` block body could not be materialized as EvaluableString".to_string(),
                        call_body_source_span(body, *span),
                        "The raw body span must remain inside its immutable source buffer.",
                    ));
                    return CallOutcome::Failed;
                };
                Some(Candidate::Positional {
                    value: InvocationValue::dynamic_value(IrValue::String(code)),
                    span: call_body_source_span(body, *span),
                })
            }
            None => None,
        };
        let bound = match binding_plan.bind(&candidates, body_candidate.as_ref(), *span) {
            Ok(bound) => bound,
            Err(error) => {
                diagnostics.push(binding_diagnostic_with_code(error, "E3003"));
                return CallOutcome::Failed;
            }
        };
        let parameters = bound.parameters;
        let mut slots = bound.slots.into_iter().enumerate();

        let language = match slots.next() {
            Some((_, BoundSlot::Explicit { value, span })) if !matches!(value.value, IrValue::None) => {
                match value_conversion::convert_target_with_origin(
                    &value,
                    value_conversion::ConversionTarget::String,
                    span,
                ) {
                    Ok(value_conversion::TargetValue::Value(IrValue::String(value))) => Some(value),
                    _ => {
                        diagnostics.push(resource_diagnostic(
                            "E3003",
                            "`.code` `lang` must adapt to String or None".to_string(),
                            span,
                            "Pass a language tag such as `rust`, or omit the argument.",
                        ));
                        return CallOutcome::Failed;
                    }
                }
            }
            Some((_, BoundSlot::Explicit { .. } | BoundSlot::Omitted | BoundSlot::Defaulted)) | None => None,
        };

        match slots.next() {
            Some((_, BoundSlot::Explicit { value, span })) if !matches!(value.value, IrValue::None) => {
                diagnostics.push(resource_diagnostic(
                    "E8001",
                    "`.code` `caption` requires the shared caption infrastructure owned by #181".to_string(),
                    span,
                    "Caption content is rejected rather than silently dropped until the shared caption/index contract is implemented.",
                ));
                return CallOutcome::Failed;
            }
            _ => {}
        }

        let line_numbers = match slots.next() {
            Some((_, BoundSlot::Explicit { value, span })) => match value_conversion::convert_target_with_origin(
                &value,
                value_conversion::ConversionTarget::Boolean,
                span,
            ) {
                Ok(value_conversion::TargetValue::Value(IrValue::Boolean(value))) => value,
                _ => {
                    diagnostics.push(resource_diagnostic(
                        "E3003",
                        "`.code` `linenumbers` must adapt to Boolean".to_string(),
                        span,
                        "Use `yes`/`no` or another supported Boolean value.",
                    ));
                    return CallOutcome::Failed;
                }
            },
            _ => true,
        };

        let focus = match slots.next() {
            Some((_, BoundSlot::Explicit { value, span })) if !matches!(value.value, IrValue::None) => {
                match value_conversion::convert_target_with_origin(
                    &value,
                    value_conversion::ConversionTarget::Range,
                    span,
                ) {
                    Ok(value_conversion::TargetValue::Value(IrValue::Range(value))) => Some(value),
                    _ => {
                        diagnostics.push(resource_diagnostic(
                            "E3003",
                            "`.code` `focus` must adapt to Range or None".to_string(),
                            span,
                            "Use a 1-based range such as `2..4`, or omit the argument.",
                        ));
                        return CallOutcome::Failed;
                    }
                }
            }
            _ => None,
        };

        let callouts = match slots.next() {
            Some((_, BoundSlot::Explicit { value, span: callouts_span })) => {
                let dictionary = match value_conversion::convert_target_with_origin(
                    &value,
                    value_conversion::ConversionTarget::Dictionary,
                    callouts_span,
                ) {
                    Ok(value_conversion::TargetValue::Value(IrValue::Dictionary(dictionary))) => dictionary,
                    Ok(value_conversion::TargetValue::RawMarkdown { text, .. }) => {
                        let nodes = match self.parse_dynamic_markdown_content(
                            &text,
                            callouts_span,
                            value_conversion::RawMarkdownTarget::Dictionary,
                            diagnostics,
                        ) {
                            Ok(nodes) => nodes,
                            Err(outcome) => return outcome,
                        };
                        let entries = match self.evaluate_dictionary_entries(
                            &nodes,
                            callouts_span,
                            diagnostics,
                            context,
                            ".code",
                        ) {
                            Ok(entries) => entries,
                            Err(outcome) => return outcome,
                        };
                        IrDictionary {
                            entries,
                            span: callouts_span,
                        }
                    }
                    _ => {
                        diagnostics.push(resource_diagnostic(
                            "E3003",
                            "`.code` `callouts` must adapt to Dictionary".to_string(),
                            callouts_span,
                            "Map positive 1-based source line numbers to callout descriptions.",
                        ));
                        return CallOutcome::Failed;
                    }
                };
                let mut converted = Vec::with_capacity(dictionary.entries.len());
                for pair in dictionary.entries {
                    let Some(raw_line) = builtins::adapt_string_argument(pair.first.as_ref()) else {
                        diagnostics.push(resource_diagnostic(
                            "E3003",
                            "Callout keys must be integers".to_string(),
                            pair.span,
                            "Use positive 1-based line numbers such as `1` or `3`.",
                        ));
                        return CallOutcome::Failed;
                    };
                    let Ok(line) = raw_line.parse::<i64>() else {
                        diagnostics.push(resource_diagnostic(
                            "E3003",
                            format!("Callout keys must be integers, but got '{raw_line}'."),
                            pair.span,
                            "Use positive 1-based line numbers such as `1` or `3`.",
                        ));
                        return CallOutcome::Failed;
                    };
                    if line <= 0 || line > i64::from(u32::MAX) {
                        diagnostics.push(resource_diagnostic(
                            "E3003",
                            format!("Callout line numbers must be positive, but got {line}."),
                            pair.span,
                            "Line numbers are 1-based; values beyond the current code length remain valid upstream.",
                        ));
                        return CallOutcome::Failed;
                    }
                    let Some(content) = builtins::plain_text_argument(pair.second.as_ref()) else {
                        diagnostics.push(resource_diagnostic(
                            "E3003",
                            "`.code` callout descriptions must project to plain text".to_string(),
                            pair.span,
                            "Use scalar or Markdown content with a deterministic plain-text projection.",
                        ));
                        return CallOutcome::Failed;
                    };
                    converted.push(IrCodeCallout {
                        line: line as u32,
                        content,
                    });
                }
                converted
            }
            _ => Vec::new(),
        };

        match slots.next() {
            Some((_, BoundSlot::Explicit { value, span })) if !matches!(value.value, IrValue::None) => {
                diagnostics.push(resource_diagnostic(
                    "E8001",
                    "`.code` `ref` requires the shared identifier/reference infrastructure owned by #181".to_string(),
                    span,
                    "References are rejected rather than emitted without an indexable anchor.",
                ));
                return CallOutcome::Failed;
            }
            _ => {}
        }

        let Some((code_index, BoundSlot::Explicit { value, span: code_span })) = slots.next() else {
            return CallOutcome::Failed;
        };
        let code = match value_conversion::convert_target_with_origin(
            &value,
            value_conversion::ConversionTarget::String,
            code_span,
        ) {
            Ok(value_conversion::TargetValue::Value(IrValue::String(value))) => value,
            _ => {
                diagnostics.push(resource_diagnostic(
                    "E3003",
                    "`.code` body must adapt to EvaluableString".to_string(),
                    code_span,
                    format!(
                        "The final `{}` parameter must preserve code text.",
                        parameters
                            .get(code_index)
                            .map_or("code", |parameter| parameter.name)
                    ),
                ));
                return CallOutcome::Failed;
            }
        };

        CallOutcome::Value(IrValue::Content(vec![IrNode::CodeBlock {
            language,
            info: None,
            source: code,
            presentation: Some(IrCodePresentation {
                line_numbers,
                focus,
                callouts,
            }),
            span: *span,
        }]))
    }

'''
text = text.replace(method_anchor, method + method_anchor, 1)
ev.write_text(text, encoding="utf-8")

# Typst lowering: retain one raw block and customize its synthesized raw.line
# elements in a local scope, preserving syntax highlighting.
lower = ROOT / "crates/arkst-typst/src/lowering.rs"
text = lower.read_text(encoding="utf-8")
text = text.replace(
    "IrDocument, IrInline, IrLandscapeComponent, IrMainAxisAlignment, IrNode, IrSize, IrSizeUnit,",
    "IrCodePresentation, IrDocument, IrInline, IrLandscapeComponent, IrMainAxisAlignment, IrNode, IrSize, IrSizeUnit,",
    1,
)
old = '''            IrNode::CodeBlock {
                language,
                info: _info,
                source,
                span,
            } => {
                let before = self.output.len();
                self.push_str("```");
                if let Some(lang) = language {
                    self.push_str(lang);
                }
                self.push('\\n');
                let was_verbatim = std::mem::replace(&mut self.verbatim, true);
                self.push_str(source);
                if !source.ends_with('\\n') {
                    self.push('\\n');
                }
                self.verbatim = was_verbatim;
                self.push_str("```\\n");
                if span.source_id != SourceId(0) {
                    self.record_span(*span, self.output.len() - before);
                }
            }'''
new = '''            IrNode::CodeBlock {
                language,
                info: _info,
                source,
                presentation,
                span,
            } => {
                let before = self.output.len();
                if let Some(presentation) = presentation {
                    self.lower_presented_code(language.as_deref(), source, presentation);
                } else {
                    self.push_str("```");
                    if let Some(lang) = language {
                        self.push_str(lang);
                    }
                    self.push('\\n');
                    let was_verbatim = std::mem::replace(&mut self.verbatim, true);
                    self.push_str(source);
                    if !source.ends_with('\\n') {
                        self.push('\\n');
                    }
                    self.verbatim = was_verbatim;
                    self.push_str("```\\n");
                }
                if span.source_id != SourceId(0) {
                    self.record_span(*span, self.output.len() - before);
                }
            }'''
assert old in text
text = text.replace(old, new, 1)
anchor = "    fn lower_component(&mut self, component: &IrComponent) {"
assert anchor in text
helper = r'''    fn lower_presented_code(
        &mut self,
        language: Option<&str>,
        source: &str,
        presentation: &IrCodePresentation,
    ) {
        self.push_str("#block[\n");
        self.push_str("#show raw.line: it => {\n");
        self.push_str("  let arkst-body = it.body\n");
        if let Some(focus) = &presentation.focus {
            if let (Some(start), Some(end)) = (focus.start, focus.end) {
                self.push_str("  if it.number >= ");
                self.push_str(&start.to_string());
                self.push_str(" and it.number <= ");
                self.push_str(&end.to_string());
                self.push_str(" { arkst-body = highlight(arkst-body) }\n");
            }
        }
        self.push_str("  grid(columns: (");
        if presentation.line_numbers {
            self.push_str("2.2em, 1fr, auto), column-gutter: 0.6em, align(right)[#it.number], arkst-body, ");
        } else {
            self.push_str("1fr, auto), column-gutter: 0.6em, arkst-body, ");
        }
        if presentation.callouts.is_empty() {
            self.push_str("none");
        } else {
            self.push_str("if false { none }");
            for (index, callout) in presentation.callouts.iter().enumerate() {
                self.push_str(" else if it.number == ");
                self.push_str(&callout.line.to_string());
                self.push_str(" { super[");
                self.push_str(&(index + 1).to_string());
                self.push_str("] }");
            }
            self.push_str(" else { none }");
        }
        self.push_str(")\n}\n");
        self.push_str("#raw(\"");
        self.push_str(&escape_typst_string(source));
        self.push_str("\", block: true");
        if let Some(language) = language {
            self.push_str(", lang: \"");
            self.push_str(&escape_typst_string(language));
            self.push('"');
        }
        self.push_str(")\n");
        if !presentation.callouts.is_empty() {
            self.push_str("#enum(\n");
            for callout in &presentation.callouts {
                self.push_str("  [");
                self.push_str(&escape_typst_text(&callout.content));
                self.push_str("],\n");
            }
            self.push_str(")\n");
        }
        self.push_str("]\n");
    }

'''
text = text.replace(anchor, helper + anchor, 1)
lower.write_text(text, encoding="utf-8")

# Update every existing CodeBlock constructor/pattern that compiler ownership
# requires only if it does not already use `..`; focused cargo check will catch
# any remaining exhaustive site rather than silently applying a broad rewrite.

# Focused compatibility tests.
test = ROOT / "crates/arkst-core/tests/quarkdown_v260_code_callouts.rs"
test.write_text(r'''use arkst_core::ir::IrNode;
use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};

fn compile_source(source: &str) -> arkst_core::CompileResult {
    let project = VirtualProjectBuilder::new()
        .entry("main.qd").expect("entry")
        .add_source("main.qd", source).expect("source")
        .build().expect("project");
    compile(&project, &CompileOptions::default())
}

fn code_block(result: &arkst_core::CompileResult) -> (&str, &arkst_core::ir::IrCodePresentation) {
    result.ir.nodes.iter().find_map(|node| match node {
        IrNode::CodeBlock { source, presentation: Some(presentation), .. } => Some((source.as_str(), presentation)),
        _ => None,
    }).expect("semantic .code block")
}

#[test]
fn code_callouts_preserve_source_lines_order_and_out_of_range_entries() {
    let result = compile_source(r#".code lang:{javascript} callouts:{
    - 1: Declares the function.
    - 3: Builds the greeting.
    - 99: Far line.
}
    function greet() {
        const name = \"World\";
        return name;
    }
"#);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let (source, presentation) = code_block(&result);
    assert_eq!(source, "function greet() {\n    const name = \\\"World\\\";\n    return name;\n}");
    assert!(presentation.line_numbers);
    assert_eq!(presentation.callouts.iter().map(|c| (c.line, c.content.as_str())).collect::<Vec<_>>(), vec![
        (1, "Declares the function."),
        (3, "Builds the greeting."),
        (99, "Far line."),
    ]);
}

#[test]
fn code_focus_and_line_number_toggle_are_typed() {
    let result = compile_source(".code linenumbers:{no} focus:{2..3}\n    alpha\n    beta\n    gamma\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let (_, presentation) = code_block(&result);
    assert!(!presentation.line_numbers);
    let focus = presentation.focus.as_ref().expect("focus");
    assert_eq!((focus.start, focus.end), (Some(2), Some(3)));
}

#[test]
fn code_rejects_non_integer_and_non_positive_callout_keys() {
    for source in [
        ".code callouts:{\n    - nope: Invalid.\n}\n    alpha\n",
        ".code callouts:{\n    - 0: Invalid.\n}\n    alpha\n",
    ] {
        let result = compile_source(source);
        assert_eq!(result.diagnostics.len(), 1, "{result:?}");
        assert_eq!(result.diagnostics[0].code, "E3003", "{result:?}");
    }
}

#[test]
fn code_caption_and_ref_fail_closed_until_shared_index_infrastructure_exists() {
    for source in [
        ".code caption:{Caption}\n    alpha\n",
        ".code ref:{sample}\n    alpha\n",
    ] {
        let result = compile_source(source);
        assert_eq!(result.diagnostics.len(), 1, "{result:?}");
        assert_eq!(result.diagnostics[0].code, "E8001", "{result:?}");
    }
}

#[test]
fn source_defined_code_shadows_new_native_producer() {
    let result = compile_source(".function {code}\n    value:\n    .value\n.code {shadow}\n");
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert!(!result.ir.nodes.iter().any(|node| matches!(node, IrNode::CodeBlock { presentation: Some(_), .. })));
}
''', encoding="utf-8")

print("v2.6 code presentation patch staged")
