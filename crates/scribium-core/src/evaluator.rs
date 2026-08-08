//! M1 evaluator: resolves Quarkdown conditional constructs (`.if` / `.ifnot`).
//!
//! Evaluation runs after parsing and `ast_to_ir` and before Typst lowering.
//! It operates on the IR: a `FunctionCall` / `DirectiveCall` named `if` or
//! `ifnot` is replaced by its content when its boolean condition holds,
//! otherwise it is removed (Quarkdown conditional-statements semantics,
//! wiki badged `v2.5.0`, accessed 2026-08-08).
//!
//! The condition is the first positional argument and must be one of the
//! boolean literals documented for the Quarkdown Boolean value type:
//! `true` / `yes` for true and `false` / `no` for false, case-insensitive.
//! Any other condition (or a missing one) is reported with the `E3001`
//! evaluation error and the construct is treated as false, keeping output
//! deterministic.
//!
//! The content of a conditional is, in order of priority: the indented
//! block body, the second positional argument when it is a content value,
//! or a bare scalar argument rendered as text.

use crate::diagnostics::{Diagnostic, Severity};
use crate::ir::{IrDocument, IrInline, IrNode, IrValue};
use crate::source::SourceSpan;

/// Evaluates Quarkdown conditional constructs in the IR.
#[derive(Debug, Default)]
pub struct Evaluator {}

impl Evaluator {
    /// Creates a new evaluator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Evaluates the document, resolving `.if` / `.ifnot` constructs.
    ///
    /// Returns the resolved document and any evaluation diagnostics.
    pub fn evaluate(&self, document: &IrDocument) -> (IrDocument, Vec<Diagnostic>) {
        let mut diagnostics = Vec::new();
        let nodes = self.evaluate_nodes(&document.nodes, &mut diagnostics);
        (
            IrDocument {
                nodes,
                metadata: document.metadata.clone(),
            },
            diagnostics,
        )
    }

    /// Evaluates a list of block nodes, collecting any diagnostics.
    fn evaluate_nodes(&self, nodes: &[IrNode], diagnostics: &mut Vec<Diagnostic>) -> Vec<IrNode> {
        let mut out = Vec::new();
        for node in nodes {
            out.extend(self.evaluate_node(node, diagnostics));
        }
        out
    }

    /// Evaluates a single block node.
    fn evaluate_node(&self, node: &IrNode, diagnostics: &mut Vec<Diagnostic>) -> Vec<IrNode> {
        match node {
            IrNode::FunctionCall {
                name,
                positional_args,
                named_args,
                body,
                span,
            } => {
                if is_conditional(name) {
                    let condition =
                        resolve_condition(name, positional_args, named_args, span, diagnostics);
                    let take = take_branch(name, condition);
                    if take {
                        self.conditional_block_content(
                            positional_args,
                            named_args,
                            body,
                            span,
                            diagnostics,
                        )
                    } else {
                        Vec::new()
                    }
                } else {
                    vec![IrNode::FunctionCall {
                        name: name.clone(),
                        positional_args: self.evaluate_values(positional_args, diagnostics),
                        named_args: self.evaluate_named(named_args, diagnostics),
                        body: body
                            .as_ref()
                            .map(|nodes| self.evaluate_nodes(nodes, diagnostics)),
                        span: *span,
                    }]
                }
            }
            IrNode::Heading {
                level,
                content,
                span,
            } => vec![IrNode::Heading {
                level: *level,
                content: self.evaluate_inlines(content, diagnostics),
                span: *span,
            }],
            IrNode::Paragraph { content, span } => vec![IrNode::Paragraph {
                content: self.evaluate_inlines(content, diagnostics),
                span: *span,
            }],
            IrNode::UnorderedList { items, span } => {
                let items = items
                    .iter()
                    .map(|item| crate::ir::IrListItem {
                        nodes: self.evaluate_nodes(&item.nodes, diagnostics),
                        span: item.span,
                    })
                    .collect();
                vec![IrNode::UnorderedList { items, span: *span }]
            }
            other => vec![other.clone()],
        }
    }

    /// Evaluates inline content, collecting any diagnostics.
    fn evaluate_inlines(
        &self,
        inlines: &[IrInline],
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Vec<IrInline> {
        let mut out = Vec::new();
        for inline in inlines {
            out.extend(self.evaluate_inline(inline, diagnostics));
        }
        out
    }

    /// Evaluates a single inline node.
    fn evaluate_inline(
        &self,
        inline: &IrInline,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Vec<IrInline> {
        match inline {
            IrInline::Emphasis { content, span } => vec![IrInline::Emphasis {
                content: self.evaluate_inlines(content, diagnostics),
                span: *span,
            }],
            IrInline::Strong { content, span } => vec![IrInline::Strong {
                content: self.evaluate_inlines(content, diagnostics),
                span: *span,
            }],
            IrInline::DirectiveCall {
                name,
                positional_args,
                named_args,
                body,
                span,
            } => {
                if is_conditional(name) {
                    let condition =
                        resolve_condition(name, positional_args, named_args, span, diagnostics);
                    let take = if *name == "if" { condition } else { !condition };
                    if take {
                        self.conditional_inline_content(
                            positional_args,
                            named_args,
                            body,
                            span,
                            diagnostics,
                        )
                    } else {
                        Vec::new()
                    }
                } else {
                    vec![IrInline::DirectiveCall {
                        name: name.clone(),
                        positional_args: self.evaluate_values(positional_args, diagnostics),
                        named_args: self.evaluate_named(named_args, diagnostics),
                        body: body
                            .as_ref()
                            .map(|inlines| self.evaluate_inlines(inlines, diagnostics)),
                        span: *span,
                    }]
                }
            }
            other => vec![other.clone()],
        }
    }

    /// Evaluates value arguments (recursing into content values).
    fn evaluate_values(
        &self,
        values: &[IrValue],
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Vec<IrValue> {
        values
            .iter()
            .map(|value| match value {
                IrValue::Content(nodes) => {
                    IrValue::Content(self.evaluate_nodes(nodes, diagnostics))
                }
                other => other.clone(),
            })
            .collect()
    }

    /// Evaluates named arguments (recursing into content values).
    fn evaluate_named(
        &self,
        named: &[(String, IrValue)],
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Vec<(String, IrValue)> {
        named
            .iter()
            .map(|(key, value)| {
                (
                    key.clone(),
                    match value {
                        IrValue::Content(nodes) => {
                            IrValue::Content(self.evaluate_nodes(nodes, diagnostics))
                        }
                        other => other.clone(),
                    },
                )
            })
            .collect()
    }

    /// Content of a conditional block call: the body if present, otherwise
    /// the named `body` argument if present, otherwise the second positional
    /// argument (content or scalar), otherwise nothing.
    fn conditional_block_content(
        &self,
        positional_args: &[IrValue],
        named_args: &[(String, IrValue)],
        body: &Option<Vec<IrNode>>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Vec<IrNode> {
        if let Some(nodes) = body {
            return self.evaluate_nodes(nodes, diagnostics);
        }
        // Check named "body" argument
        if let Some((_, IrValue::Content(nodes))) = named_args.iter().find(|(k, _)| k == "body") {
            return self.evaluate_nodes(nodes, diagnostics);
        }
        if let Some((_, scalar)) = named_args.iter().find(|(k, _)| k == "body") {
            return vec![IrNode::Paragraph {
                content: vec![IrInline::Text {
                    content: scalar_to_text(scalar),
                    span: *span,
                }],
                span: *span,
            }];
        }
        match positional_args.get(1) {
            Some(IrValue::Content(nodes)) => self.evaluate_nodes(nodes, diagnostics),
            Some(scalar) => vec![IrNode::Paragraph {
                content: vec![IrInline::Text {
                    content: scalar_to_text(scalar),
                    span: *span,
                }],
                span: *span,
            }],
            None => Vec::new(),
        }
    }

    /// Content of a conditional inline call: the body if present, otherwise
    /// the named `body` argument if present, otherwise the second positional
    /// argument (a single-paragraph content value or a bare scalar),
    /// otherwise nothing.
    fn conditional_inline_content(
        &self,
        positional_args: &[IrValue],
        named_args: &[(String, IrValue)],
        body: &Option<Vec<IrInline>>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Vec<IrInline> {
        if let Some(inlines) = body {
            return self.evaluate_inlines(inlines, diagnostics);
        }
        // Check named "body" argument
        if let Some((_, IrValue::Content(nodes))) = named_args.iter().find(|(k, _)| k == "body") {
            return match nodes.as_slice() {
                [IrNode::Paragraph { content, .. }] => self.evaluate_inlines(content, diagnostics),
                _ => Vec::new(),
            };
        }
        if let Some((_, scalar)) = named_args.iter().find(|(k, _)| k == "body") {
            return vec![IrInline::Text {
                content: scalar_to_text(scalar),
                span: *span,
            }];
        }
        match positional_args.get(1) {
            Some(IrValue::Content(nodes)) => match nodes.as_slice() {
                [IrNode::Paragraph { content, .. }] => self.evaluate_inlines(content, diagnostics),
                _ => Vec::new(),
            },
            Some(scalar) => vec![IrInline::Text {
                content: scalar_to_text(scalar),
                span: *span,
            }],
            None => Vec::new(),
        }
    }
}

/// Returns true for the conditional constructs this evaluator resolves.
fn is_conditional(name: &str) -> bool {
    name == "if" || name == "ifnot"
}

/// Resolves the condition of a conditional call.
///
/// A missing or non-boolean condition produces an `E3001` diagnostic and
/// is treated as `false` (deterministic output).
/// The condition can be provided as the first positional argument or as
/// a named argument `condition`.
fn resolve_condition(
    name: &str,
    positional_args: &[IrValue],
    named_args: &[(String, IrValue)],
    span: &SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    // First check named argument "condition"
    if let Some((_, value)) = named_args.iter().find(|(k, _)| k == "condition") {
        return match boolean_value(value) {
            Some(value) => value,
            None => {
                diagnostics.push(unresolvable_condition(name, span));
                false
            }
        };
    }
    // Fall back to first positional argument
    match positional_args.first() {
        Some(value) => match boolean_value(value) {
            Some(value) => value,
            None => {
                diagnostics.push(unresolvable_condition(name, span));
                false
            }
        },
        None => {
            diagnostics.push(unresolvable_condition(name, span));
            false
        }
    }
}

/// Decides whether a conditional's content is taken.
fn take_branch(name: &str, condition: bool) -> bool {
    if name == "if" {
        condition
    } else {
        !condition
    }
}

/// Maps a value to its boolean meaning, if one exists.
///
/// Supports the Quarkdown boolean literals `true`/`yes` and `false`/`no`,
/// case-insensitive (Quarkdown "Boolean" documentation, badged `v2.5.0`).
fn boolean_value(value: &IrValue) -> Option<bool> {
    match value {
        IrValue::Boolean(value) => Some(*value),
        IrValue::Identifier(name) => match name.to_lowercase().as_str() {
            "true" | "yes" => Some(true),
            "false" | "no" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

/// Renders a scalar argument as plain text.
fn scalar_to_text(value: &IrValue) -> String {
    match value {
        IrValue::String(text) => text.clone(),
        IrValue::Number(number) => number.to_string(),
        IrValue::Boolean(boolean) => boolean.to_string(),
        IrValue::Identifier(name) => name.clone(),
        IrValue::Content(_) => String::new(),
    }
}

/// Builds the `E3001` diagnostic for an unresolvable condition.
fn unresolvable_condition(name: &str, span: &SourceSpan) -> Diagnostic {
    Diagnostic {
        code: "E3001".to_string(),
        severity: Severity::Error,
        message: format!(
            "`{name}` requires a boolean condition (literals `true`, `false`, `yes`, or `no`) as its `condition` argument"
        ),
        primary: Some(*span),
        secondary: Vec::new(),
        hints: vec!["Conditional evaluation currently supports boolean literal conditions only.".to_string()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::SourceId;

    fn span(start: usize, end: usize) -> SourceSpan {
        SourceSpan::new(SourceId(1), start, end)
    }

    fn text_paragraph(content: &str) -> IrNode {
        IrNode::Paragraph {
            content: vec![IrInline::Text {
                content: content.to_string(),
                span: span(0, content.len()),
            }],
            span: span(0, content.len()),
        }
    }

    fn text_inline(content: &str) -> IrInline {
        IrInline::Text {
            content: content.to_string(),
            span: span(0, content.len()),
        }
    }

    fn if_call(name: &str, condition: IrValue, body: Vec<IrNode>) -> IrNode {
        IrNode::FunctionCall {
            name: name.to_string(),
            positional_args: vec![condition],
            named_args: Vec::new(),
            body: Some(body),
            span: span(0, 1),
        }
    }

    fn inline_if_call(name: &str, condition: IrValue, inline_body: Vec<IrInline>) -> IrInline {
        IrInline::DirectiveCall {
            name: name.to_string(),
            positional_args: vec![condition],
            named_args: Vec::new(),
            body: Some(inline_body),
            span: span(0, 1),
        }
    }

    fn doc(nodes: Vec<IrNode>) -> IrDocument {
        IrDocument {
            nodes,
            metadata: crate::ir::IrMetadata::default(),
        }
    }

    fn evaluate(nodes: Vec<IrNode>) -> Vec<IrNode> {
        Evaluator::new().evaluate(&doc(nodes)).0.nodes
    }

    #[test]
    fn if_true_keeps_block_body() {
        let nodes = evaluate(vec![if_call(
            "if",
            IrValue::Boolean(true),
            vec![text_paragraph("kept")],
        )]);
        assert_eq!(nodes, vec![text_paragraph("kept")]);
    }

    #[test]
    fn if_false_drops_block_body() {
        let nodes = evaluate(vec![if_call(
            "if",
            IrValue::Boolean(false),
            vec![text_paragraph("dropped")],
        )]);
        assert!(nodes.is_empty());
    }

    #[test]
    fn ifnot_true_drops_and_ifnot_false_keeps() {
        let keep = evaluate(vec![if_call(
            "ifnot",
            IrValue::Boolean(false),
            vec![text_paragraph("kept")],
        )]);
        assert_eq!(keep, vec![text_paragraph("kept")]);

        let drop = evaluate(vec![if_call(
            "ifnot",
            IrValue::Boolean(true),
            vec![text_paragraph("dropped")],
        )]);
        assert!(drop.is_empty());
    }

    #[test]
    fn boolean_identifiers_yes_no_true_false_case_insensitive() {
        for (literal, expected) in [
            ("yes", true),
            ("YES", true),
            ("true", true),
            ("True", true),
            ("no", false),
            ("No", false),
            ("false", false),
            ("FALSE", false),
        ] {
            let nodes = evaluate(vec![if_call(
                "if",
                IrValue::Identifier(literal.to_string()),
                vec![text_paragraph("content")],
            )]);
            if expected {
                assert_eq!(nodes, vec![text_paragraph("content")], "literal {literal}");
            } else {
                assert!(nodes.is_empty(), "literal {literal}");
            }
        }
    }

    #[test]
    fn missing_condition_reports_e3001_and_drops() {
        let call = IrNode::FunctionCall {
            name: "if".to_string(),
            positional_args: Vec::new(),
            named_args: Vec::new(),
            body: Some(vec![text_paragraph("content")]),
            span: span(3, 6),
        };
        let (result, diagnostics) = Evaluator::new().evaluate(&doc(vec![call]));
        assert!(result.nodes.is_empty());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E3001");
        assert!(matches!(diagnostics[0].severity, Severity::Error));
        assert_eq!(diagnostics[0].primary, Some(span(3, 6)));
    }

    #[test]
    fn unresolvable_condition_reports_diagnostic() {
        for condition in [
            IrValue::Number(3.0),
            IrValue::String("maybe".to_string()),
            IrValue::Identifier("unknown".to_string()),
            IrValue::Content(vec![text_paragraph("content")]),
        ] {
            let display = format!("{condition:?}");
            let (result, diagnostics) = Evaluator::new().evaluate(&doc(vec![if_call(
                "if",
                condition.clone(),
                vec![text_paragraph("body")],
            )]));
            assert!(result.nodes.is_empty(), "condition {display}");
            assert_eq!(diagnostics.len(), 1, "condition {display}");
            assert_eq!(diagnostics[0].code, "E3001");
        }
    }

    #[test]
    fn nested_if_inside_block_body_is_evaluated() {
        let body = vec![
            text_paragraph("before"),
            if_call(
                "if",
                IrValue::Boolean(false),
                vec![text_paragraph("inner-dropped")],
            ),
            if_call(
                "if",
                IrValue::Boolean(true),
                vec![text_paragraph("inner-kept")],
            ),
        ];
        let nodes = evaluate(vec![if_call("if", IrValue::Boolean(true), body)]);
        assert_eq!(
            nodes,
            vec![text_paragraph("before"), text_paragraph("inner-kept"),]
        );
    }

    #[test]
    fn content_value_second_argument_replaces_call() {
        let call = IrNode::FunctionCall {
            name: "if".to_string(),
            positional_args: vec![
                IrValue::Boolean(true),
                IrValue::Content(vec![text_paragraph("arg content")]),
            ],
            named_args: Vec::new(),
            body: None,
            span: span(0, 1),
        };
        let nodes = evaluate(vec![call]);
        assert_eq!(nodes, vec![text_paragraph("arg content")]);
    }

    #[test]
    fn scalar_second_argument_becomes_text() {
        let call = IrNode::FunctionCall {
            name: "if".to_string(),
            positional_args: vec![
                IrValue::Boolean(true),
                IrValue::String("inline text".to_string()),
            ],
            named_args: Vec::new(),
            body: None,
            span: span(0, 1),
        };
        let nodes = evaluate(vec![call]);
        assert_eq!(
            nodes,
            vec![IrNode::Paragraph {
                content: vec![IrInline::Text {
                    content: "inline text".to_string(),
                    span: span(0, 1),
                }],
                span: span(0, 1),
            }]
        );
    }

    #[test]
    fn block_body_takes_priority_over_positional_content() {
        let call = IrNode::FunctionCall {
            name: "if".to_string(),
            positional_args: vec![
                IrValue::Boolean(true),
                IrValue::Content(vec![text_paragraph("from arg")]),
            ],
            named_args: Vec::new(),
            body: Some(vec![text_paragraph("from body")]),
            span: span(0, 1),
        };
        let nodes = evaluate(vec![call]);
        assert_eq!(nodes, vec![text_paragraph("from body")]);
    }

    #[test]
    fn inline_if_replaces_call_with_inline_body_or_content() {
        let paragraph = IrNode::Paragraph {
            content: vec![
                text_inline("before "),
                inline_if_call("if", IrValue::Boolean(true), vec![text_inline("kept")]),
                text_inline(" after"),
            ],
            span: span(0, 1),
        };
        let nodes = evaluate(vec![paragraph]);
        let IrNode::Paragraph { content, .. } = &nodes[0] else {
            panic!("expected paragraph");
        };
        let rendered: Vec<&str> = content
            .iter()
            .map(|i| match i {
                IrInline::Text { content, .. } => content.as_str(),
                other => panic!("unexpected inline {other:?}"),
            })
            .collect();
        assert_eq!(rendered, vec!["before ", "kept", " after"]);
    }

    #[test]
    fn inline_if_false_drops_call() {
        let paragraph = IrNode::Paragraph {
            content: vec![
                text_inline("before "),
                inline_if_call(
                    "ifnot",
                    IrValue::Boolean(true),
                    vec![text_inline("dropped")],
                ),
                text_inline(" after"),
            ],
            span: span(0, 1),
        };
        let nodes = evaluate(vec![paragraph]);
        let IrNode::Paragraph { content, .. } = &nodes[0] else {
            panic!("expected paragraph");
        };
        assert_eq!(
            content,
            &vec![text_inline("before "), text_inline(" after")]
        );
    }

    #[test]
    fn inline_call_scalar_second_argument_becomes_text() {
        let call = IrInline::DirectiveCall {
            name: "if".to_string(),
            positional_args: vec![IrValue::Boolean(true), IrValue::String("shown".to_string())],
            named_args: Vec::new(),
            body: None,
            span: span(0, 1),
        };
        let paragraph = IrNode::Paragraph {
            content: vec![text_inline("x "), call],
            span: span(0, 1),
        };
        let nodes = evaluate(vec![paragraph]);
        let IrNode::Paragraph { content, .. } = &nodes[0] else {
            panic!("expected paragraph");
        };
        assert_eq!(
            content,
            &vec![
                text_inline("x "),
                IrInline::Text {
                    content: "shown".to_string(),
                    span: span(0, 1),
                }
            ]
        );
    }

    #[test]
    fn non_conditional_calls_are_preserved_with_evaluated_bodies() {
        let call = IrNode::FunctionCall {
            name: "foo".to_string(),
            positional_args: Vec::new(),
            named_args: Vec::new(),
            body: Some(vec![if_call(
                "if",
                IrValue::Boolean(false),
                vec![text_paragraph("dropped")],
            )]),
            span: span(0, 1),
        };
        let nodes = evaluate(vec![call]);
        let IrNode::FunctionCall { name, body, .. } = &nodes[0] else {
            panic!("expected function call");
        };
        assert_eq!(name, "foo");
        assert!(body.as_ref().unwrap().is_empty());
    }

    #[test]
    fn evaluation_is_immutable_and_deterministic() {
        let call = if_call("if", IrValue::Boolean(true), vec![text_paragraph("kept")]);
        let input = doc(vec![call.clone()]);
        let first = Evaluator::new().evaluate(&input);
        assert_eq!(input.nodes, vec![call]);
        let second = Evaluator::new().evaluate(&input);
        assert_eq!(first.0, second.0);
        assert!(first.1.is_empty() && second.1.is_empty());
    }

    #[test]
    fn named_condition_argument_works() {
        let call = IrNode::FunctionCall {
            name: "if".to_string(),
            positional_args: Vec::new(),
            named_args: vec![("condition".to_string(), IrValue::Boolean(true))],
            body: Some(vec![text_paragraph("kept")]),
            span: span(0, 1),
        };
        let nodes = evaluate(vec![call]);
        assert_eq!(nodes, vec![text_paragraph("kept")]);
    }

    #[test]
    fn named_condition_false_drops_body() {
        let call = IrNode::FunctionCall {
            name: "if".to_string(),
            positional_args: Vec::new(),
            named_args: vec![("condition".to_string(), IrValue::Boolean(false))],
            body: Some(vec![text_paragraph("dropped")]),
            span: span(0, 1),
        };
        let nodes = evaluate(vec![call]);
        assert!(nodes.is_empty());
    }

    #[test]
    fn named_condition_ifnot_inverts() {
        let call = IrNode::FunctionCall {
            name: "ifnot".to_string(),
            positional_args: Vec::new(),
            named_args: vec![("condition".to_string(), IrValue::Boolean(false))],
            body: Some(vec![text_paragraph("kept")]),
            span: span(0, 1),
        };
        let nodes = evaluate(vec![call]);
        assert_eq!(nodes, vec![text_paragraph("kept")]);
    }

    #[test]
    fn named_condition_identifier_yes_no() {
        for (ident, expected) in [("yes", true), ("YES", true), ("no", false), ("No", false)] {
            let call = IrNode::FunctionCall {
                name: "if".to_string(),
                positional_args: Vec::new(),
                named_args: vec![(
                    "condition".to_string(),
                    IrValue::Identifier(ident.to_string()),
                )],
                body: Some(vec![text_paragraph("content")]),
                span: span(0, 1),
            };
            let nodes = evaluate(vec![call]);
            if expected {
                assert_eq!(nodes, vec![text_paragraph("content")], "ident {ident}");
            } else {
                assert!(nodes.is_empty(), "ident {ident}");
            }
        }
    }

    #[test]
    fn named_body_argument_works() {
        let call = IrNode::FunctionCall {
            name: "if".to_string(),
            positional_args: vec![IrValue::Boolean(true)],
            named_args: vec![(
                "body".to_string(),
                IrValue::Content(vec![text_paragraph("from named body")]),
            )],
            body: None,
            span: span(0, 1),
        };
        let nodes = evaluate(vec![call]);
        assert_eq!(nodes, vec![text_paragraph("from named body")]);
    }

    #[test]
    fn named_body_scalar_argument_works() {
        let call = IrNode::FunctionCall {
            name: "if".to_string(),
            positional_args: vec![IrValue::Boolean(true)],
            named_args: vec![(
                "body".to_string(),
                IrValue::String("scalar body".to_string()),
            )],
            body: None,
            span: span(0, 1),
        };
        let nodes = evaluate(vec![call]);
        assert_eq!(
            nodes,
            vec![IrNode::Paragraph {
                content: vec![IrInline::Text {
                    content: "scalar body".to_string(),
                    span: span(0, 1),
                }],
                span: span(0, 1),
            }]
        );
    }

    #[test]
    fn block_body_priority_over_named_body() {
        let call = IrNode::FunctionCall {
            name: "if".to_string(),
            positional_args: vec![IrValue::Boolean(true)],
            named_args: vec![(
                "body".to_string(),
                IrValue::Content(vec![text_paragraph("from named body")]),
            )],
            body: Some(vec![text_paragraph("from indented body")]),
            span: span(0, 1),
        };
        let nodes = evaluate(vec![call]);
        assert_eq!(nodes, vec![text_paragraph("from indented body")]);
    }

    #[test]
    fn inline_named_condition_works() {
        let paragraph = IrNode::Paragraph {
            content: vec![
                text_inline("before "),
                IrInline::DirectiveCall {
                    name: "if".to_string(),
                    positional_args: Vec::new(),
                    named_args: vec![("condition".to_string(), IrValue::Boolean(true))],
                    body: Some(vec![text_inline("kept")]),
                    span: span(0, 1),
                },
                text_inline(" after"),
            ],
            span: span(0, 1),
        };
        let nodes = evaluate(vec![paragraph]);
        let IrNode::Paragraph { content, .. } = &nodes[0] else {
            panic!()
        };
        let rendered: Vec<&str> = content
            .iter()
            .map(|i| match i {
                IrInline::Text { content, .. } => content.as_str(),
                _ => panic!(),
            })
            .collect();
        assert_eq!(rendered, vec!["before ", "kept", " after"]);
    }

    #[test]
    fn inline_named_body_works() {
        let call = IrInline::DirectiveCall {
            name: "if".to_string(),
            positional_args: vec![IrValue::Boolean(true)],
            named_args: vec![(
                "body".to_string(),
                IrValue::String("inline shown".to_string()),
            )],
            body: None,
            span: span(0, 1),
        };
        let paragraph = IrNode::Paragraph {
            content: vec![text_inline("x "), call],
            span: span(0, 1),
        };
        let nodes = evaluate(vec![paragraph]);
        let IrNode::Paragraph { content, .. } = &nodes[0] else {
            panic!()
        };
        assert_eq!(
            content,
            &vec![
                text_inline("x "),
                IrInline::Text {
                    content: "inline shown".to_string(),
                    span: span(0, 1),
                }
            ]
        );
    }

    #[test]
    fn named_condition_unresolvable_reports_e3001() {
        let call = IrNode::FunctionCall {
            name: "if".to_string(),
            positional_args: Vec::new(),
            named_args: vec![("condition".to_string(), IrValue::Number(3.0))],
            body: Some(vec![text_paragraph("body")]),
            span: span(3, 6),
        };
        let (result, diagnostics) = Evaluator::new().evaluate(&doc(vec![call]));
        assert!(result.nodes.is_empty());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E3001");
    }
}
