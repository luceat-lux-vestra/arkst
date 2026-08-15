//! M1/M2 evaluator: resolves Quarkdown conditionals, variables, and the first
//! value-flow builtins used by `::` call chains.
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
//!
//! Variable evaluation (`.var`) follows Quarkdown document-scope variable
//! semantics: declarations create bindings in a document-wide environment;
//! parameterless calls (`.name`) resolve to the bound value; reassignment
//! is supported via explicit `.var {name} {value}` or variable-name
//! call `.name {value}` (only if `name` is already a variable). Unknown
//! parameterless calls are preserved as function calls, not variable errors.
//!
//! Block variables (`.var {name}\n    body\n.name`) store evaluated content
//! and materialize it at reference sites.
//!
//! Chain evaluation is structural: the head is invoked first, its semantic
//! `IrValue` becomes the first positional argument of the next segment, and
//! segments continue in source order. No source or backend text is generated
//! during this process.

use crate::builtins;
use crate::diagnostics::{Diagnostic, Severity};
use crate::ir::{IrCallSegment, IrDocument, IrInline, IrNode, IrValue};
use crate::source::SourceSpan;
use scribium_quarkdown::is_valid_normal_call_name;
use std::collections::BTreeMap;

/// A resolved variable value stored in the evaluation environment.
///
/// Variables can hold any IR value type. The value is fully evaluated
/// at declaration time (for scalars) or stored as content nodes for
/// block variables.
#[derive(Debug, Clone)]
enum VariableValue {
    Scalar(IrValue),
    Content(Vec<IrNode>),
}

impl VariableValue {
    /// Creates a VariableValue from an evaluated IrValue, preserving content semantics.
    fn from_evaluated_value(value: IrValue) -> Self {
        match value {
            IrValue::Content(nodes) => VariableValue::Content(nodes),
            scalar => VariableValue::Scalar(scalar),
        }
    }

    /// Returns the backend-neutral value used when this variable participates
    /// in a chain.
    fn to_value(&self) -> IrValue {
        match self {
            VariableValue::Scalar(value) => value.clone(),
            VariableValue::Content(nodes) => IrValue::Content(nodes.clone()),
        }
    }
}

impl VariableValue {
    /// Materializes the variable value as inline nodes at a reference site.
    fn materialize_inline(&self, span: &SourceSpan) -> Vec<IrInline> {
        match self {
            VariableValue::Scalar(scalar) => vec![IrInline::Text {
                content: scalar_to_text(scalar),
                span: *span,
            }],
            VariableValue::Content(nodes) => {
                // For inline context, extract text from paragraph content
                let mut result = Vec::new();
                for node in nodes {
                    if let IrNode::Paragraph { content, .. } = node {
                        result.extend(content.clone());
                    }
                }
                result
            }
        }
    }

    /// Returns the boolean value if this variable can be interpreted as a boolean.
    fn as_boolean(&self) -> Option<bool> {
        match self {
            VariableValue::Scalar(value) => scalar_boolean_value(value),
            VariableValue::Content(_) => None,
        }
    }
}

/// A future-compatible callable binding slot for nested evaluator scopes.
///
/// User-facing function declaration is intentionally deferred to a later
/// slice. Keeping parameter metadata in the scope model now means that the
/// later lambda implementation does not need a second environment model.
#[derive(Debug, Clone, PartialEq, Eq)]
struct FunctionBinding {
    parameters: Vec<String>,
}

/// A call body that has not been evaluated yet.
///
/// Keeping the source body in this form lets the callee decide whether it is
/// eager or lazy. In particular, conditionals must inspect their condition
/// before evaluating an unreachable body.
#[derive(Clone, Copy)]
enum CallBody<'a> {
    Block(&'a [IrNode]),
    Inline(&'a [IrInline]),
}

/// Result of invoking a call in value context.
///
/// `Unresolved` is distinct from an empty content value: an ordinary output
/// context may preserve it, while a chain must reject it because it cannot
/// inject a fabricated intermediate value.
enum CallOutcome {
    Value(IrValue),
    NoValue,
    Unresolved,
}

/// Evaluation context with explicit parent visibility and local bindings.
///
/// Created fresh per `evaluate()` call to ensure isolation and determinism.
/// Lookups walk the parent chain without cloning it. A child scope snapshots
/// the visible parent context at creation time and local writes stay in the
/// child. The snapshot is deliberate: a lambda observes the bindings visible
/// when it is entered, while its local declarations cannot leak back.
#[derive(Debug, Clone, Default)]
struct EvaluationContext {
    parent: Option<Box<EvaluationContext>>,
    variables: BTreeMap<String, VariableValue>,
    functions: BTreeMap<String, FunctionBinding>,
}

impl EvaluationContext {
    fn new() -> Self {
        Self::default()
    }

    /// Creates a child scope with parent-visible bindings and isolated locals.
    #[allow(dead_code)]
    fn child(&self) -> Self {
        Self {
            parent: Some(Box::new(self.clone())),
            variables: BTreeMap::new(),
            functions: BTreeMap::new(),
        }
    }

    /// Declares or reassigns a variable with content (block variable).
    fn set_content(&mut self, name: String, content: Vec<IrNode>) {
        self.variables.insert(name, VariableValue::Content(content));
    }

    /// Declares or reassigns a variable from an evaluated IrValue, preserving content semantics.
    fn set_value(&mut self, name: String, value: IrValue) {
        self.variables
            .insert(name, VariableValue::from_evaluated_value(value));
    }

    /// Installs a future user-function binding in the current local scope.
    #[cfg_attr(not(test), allow(dead_code))]
    fn set_function(&mut self, name: String, parameters: Vec<String>) {
        self.functions.insert(name, FunctionBinding { parameters });
    }

    /// Gets a variable value if it exists.
    fn get(&self, name: &str) -> Option<&VariableValue> {
        self.variables
            .get(name)
            .or_else(|| self.parent.as_deref().and_then(|parent| parent.get(name)))
    }

    /// Checks if a name is bound as a variable.
    fn contains(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    /// Looks up a function binding through the visible scope chain.
    fn get_function(&self, name: &str) -> Option<&FunctionBinding> {
        self.functions.get(name).or_else(|| {
            self.parent
                .as_deref()
                .and_then(|parent| parent.get_function(name))
        })
    }
}

/// Evaluates Quarkdown conditionals, document-scope variables, and the
/// currently supported semantic chain builtins in the IR.
#[derive(Debug, Default)]
pub struct Evaluator {}

impl Evaluator {
    /// Creates a new evaluator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Evaluates the document, resolving conditionals, variables, and chains.
    ///
    /// Returns the resolved document and any evaluation diagnostics.
    pub fn evaluate(&self, document: &IrDocument) -> (IrDocument, Vec<Diagnostic>) {
        let mut diagnostics = Vec::new();
        let mut context = EvaluationContext::new();
        let nodes = self.evaluate_nodes(&document.nodes, &mut diagnostics, &mut context);
        (
            IrDocument {
                nodes,
                metadata: document.metadata.clone(),
            },
            diagnostics,
        )
    }

    /// Evaluates a list of block nodes, collecting any diagnostics.
    fn evaluate_nodes(
        &self,
        nodes: &[IrNode],
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> Vec<IrNode> {
        let mut out = Vec::new();
        for node in nodes {
            out.extend(self.evaluate_node(node, diagnostics, context));
        }
        out
    }

    /// Evaluates a single block node.
    fn evaluate_node(
        &self,
        node: &IrNode,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> Vec<IrNode> {
        match node {
            IrNode::FunctionCall {
                name,
                positional_args,
                named_args,
                body,
                span,
            } => self.evaluate_block_call(
                name,
                positional_args,
                named_args,
                body.as_deref(),
                span,
                diagnostics,
                context,
            ),
            IrNode::ChainedFunctionCall {
                head,
                chain,
                body,
                span,
            } => self.evaluate_block_chain(head, chain, body, span, diagnostics, context),
            IrNode::Heading {
                level,
                content,
                span,
            } => vec![IrNode::Heading {
                level: *level,
                content: self.evaluate_inlines(content, diagnostics, context),
                span: *span,
            }],
            IrNode::Paragraph { content, span } => vec![IrNode::Paragraph {
                content: self.evaluate_inlines(content, diagnostics, context),
                span: *span,
            }],
            IrNode::Blockquote { content, span } => vec![IrNode::Blockquote {
                content: self.evaluate_nodes(content, diagnostics, context),
                span: *span,
            }],
            IrNode::UnorderedList { items, span } => {
                let items = items
                    .iter()
                    .map(|item| crate::ir::IrListItem {
                        nodes: self.evaluate_nodes(&item.nodes, diagnostics, context),
                        task: item.task,
                        span: item.span,
                    })
                    .collect();
                vec![IrNode::UnorderedList { items, span: *span }]
            }
            IrNode::OrderedList { items, start, span } => {
                let items = items
                    .iter()
                    .map(|item| crate::ir::IrListItem {
                        nodes: self.evaluate_nodes(&item.nodes, diagnostics, context),
                        task: item.task,
                        span: item.span,
                    })
                    .collect();
                vec![IrNode::OrderedList {
                    items,
                    start: *start,
                    span: *span,
                }]
            }
            IrNode::Table { header, rows, span } => vec![IrNode::Table {
                header: crate::ir::IrTableRow {
                    cells: header
                        .cells
                        .iter()
                        .map(|cell| crate::ir::IrTableCell {
                            content: self.evaluate_inlines(&cell.content, diagnostics, context),
                            alignment: cell.alignment,
                            span: cell.span,
                        })
                        .collect(),
                    span: header.span,
                },
                rows: rows
                    .iter()
                    .map(|row| crate::ir::IrTableRow {
                        cells: row
                            .cells
                            .iter()
                            .map(|cell| crate::ir::IrTableCell {
                                content: self.evaluate_inlines(&cell.content, diagnostics, context),
                                alignment: cell.alignment,
                                span: cell.span,
                            })
                            .collect(),
                        span: row.span,
                    })
                    .collect(),
                span: *span,
            }],
            other => vec![other.clone()],
        }
    }

    /// Evaluates inline content, collecting any diagnostics.
    fn evaluate_inlines(
        &self,
        inlines: &[IrInline],
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> Vec<IrInline> {
        let mut out = Vec::new();
        for inline in inlines {
            out.extend(self.evaluate_inline(inline, diagnostics, context));
        }
        out
    }

    /// Evaluates a single inline node.
    fn evaluate_inline(
        &self,
        inline: &IrInline,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> Vec<IrInline> {
        match inline {
            IrInline::Emphasis { content, span } => vec![IrInline::Emphasis {
                content: self.evaluate_inlines(content, diagnostics, context),
                span: *span,
            }],
            IrInline::Strong { content, span } => vec![IrInline::Strong {
                content: self.evaluate_inlines(content, diagnostics, context),
                span: *span,
            }],
            IrInline::Strikethrough { content, span } => vec![IrInline::Strikethrough {
                content: self.evaluate_inlines(content, diagnostics, context),
                span: *span,
            }],
            IrInline::Link {
                content,
                destination,
                span,
            } => vec![IrInline::Link {
                content: self.evaluate_inlines(content, diagnostics, context),
                destination: destination.clone(),
                span: *span,
            }],
            IrInline::DirectiveCall {
                name,
                positional_args,
                named_args,
                body,
                span,
            } => self.evaluate_inline_call(
                name,
                positional_args,
                named_args,
                body.as_deref(),
                span,
                diagnostics,
                context,
            ),
            IrInline::ChainedDirectiveCall {
                head,
                chain,
                body,
                span,
            } => self.evaluate_inline_chain(head, chain, body, span, diagnostics, context),
            IrInline::Code { content, span } => {
                // Code spans are opaque: the content is never resolved,
                // recursed into, or evaluated. It passes straight through.
                vec![IrInline::Code {
                    content: content.clone(),
                    span: *span,
                }]
            }
            other => vec![other.clone()],
        }
    }

    /// Evaluates an ordinary block call in output context.
    #[allow(clippy::too_many_arguments)]
    fn evaluate_block_call(
        &self,
        name: &str,
        positional_args: &[IrValue],
        named_args: &[(String, IrValue)],
        body: Option<&[IrNode]>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> Vec<IrNode> {
        match self.evaluate_call_value(
            name,
            positional_args,
            named_args,
            body.map(CallBody::Block),
            span,
            diagnostics,
            context,
        ) {
            CallOutcome::Value(value) => self.materialize_block_value(Some(value), span),
            CallOutcome::NoValue => Vec::new(),
            CallOutcome::Unresolved => self.preserve_block_call(
                name,
                positional_args,
                named_args,
                body,
                span,
                diagnostics,
                context,
            ),
        }
    }

    /// Evaluates an ordinary inline call in output context.
    #[allow(clippy::too_many_arguments)]
    fn evaluate_inline_call(
        &self,
        name: &str,
        positional_args: &[IrValue],
        named_args: &[(String, IrValue)],
        body: Option<&[IrInline]>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> Vec<IrInline> {
        match self.evaluate_call_value(
            name,
            positional_args,
            named_args,
            body.map(CallBody::Inline),
            span,
            diagnostics,
            context,
        ) {
            CallOutcome::Value(value) => self.materialize_inline_value(Some(value), span),
            CallOutcome::NoValue => Vec::new(),
            CallOutcome::Unresolved => self.preserve_inline_call(
                name,
                positional_args,
                named_args,
                body,
                span,
                diagnostics,
                context,
            ),
        }
    }

    /// Evaluates a block chain and materializes its final semantic value.
    fn evaluate_block_chain(
        &self,
        head: &IrCallSegment,
        chain: &[IrCallSegment],
        body: &Option<Vec<IrNode>>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> Vec<IrNode> {
        self.materialize_block_value(
            self.evaluate_chain_value(
                head,
                chain,
                body.as_deref().map(CallBody::Block),
                diagnostics,
                context,
            ),
            span,
        )
    }

    /// Evaluates an inline chain and materializes its final semantic value.
    fn evaluate_inline_chain(
        &self,
        head: &IrCallSegment,
        chain: &[IrCallSegment],
        body: &Option<Vec<IrInline>>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> Vec<IrInline> {
        self.materialize_inline_value(
            self.evaluate_chain_value(
                head,
                chain,
                body.as_deref().map(CallBody::Inline),
                diagnostics,
                context,
            ),
            span,
        )
    }

    /// Invokes a call in value context. Ordinary nested calls and chain
    /// segments use this exact contract; only their surrounding syntax differs.
    /// Bodies remain unevaluated until the callee selects an evaluation policy.
    #[allow(clippy::too_many_arguments)]
    fn evaluate_call_value(
        &self,
        name: &str,
        positional_args: &[IrValue],
        named_args: &[(String, IrValue)],
        body: Option<CallBody<'_>>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> CallOutcome {
        if is_conditional(name) {
            let condition = self.resolve_call_condition(
                name,
                positional_args,
                named_args,
                span,
                diagnostics,
                context,
            );
            return CallOutcome::Value(if take_branch(name, condition) {
                self.conditional_content_value(
                    positional_args,
                    named_args,
                    body,
                    span,
                    diagnostics,
                    context,
                )
            } else {
                IrValue::Content(Vec::new())
            });
        }

        if is_var_declaration(name) {
            return self.handle_var_declaration(
                positional_args,
                named_args,
                body,
                span,
                diagnostics,
                context,
            );
        }

        if is_variable_reference_call(name, positional_args, named_args, body, context) {
            return context
                .get(name)
                .map(|value| CallOutcome::Value(value.to_value()))
                .unwrap_or(CallOutcome::NoValue);
        }

        if is_variable_reassignment_call(name, positional_args, named_args, body, context) {
            return self.handle_variable_reassignment_value(
                name,
                positional_args,
                diagnostics,
                context,
            );
        }

        if builtins::is_supported(name) {
            let Some(evaluated_positional) =
                self.evaluate_values(positional_args, diagnostics, context)
            else {
                return CallOutcome::NoValue;
            };
            let Some(evaluated_named) = self.evaluate_named(named_args, diagnostics, context)
            else {
                return CallOutcome::NoValue;
            };
            return match builtins::evaluate(
                name,
                &evaluated_positional,
                &evaluated_named,
                body.is_some(),
            ) {
                Ok(value) => CallOutcome::Value(value),
                Err(error) => {
                    diagnostics.push(chain_evaluation_error(error.message, *span));
                    CallOutcome::NoValue
                }
            };
        }

        // Ordinary output context preserves unresolved calls. A chain wrapper
        // converts this outcome into an explicit source-backed E3001 instead.
        CallOutcome::Unresolved
    }

    /// Evaluates a chain strictly left-to-right using semantic `IrValue`s.
    fn evaluate_chain_value(
        &self,
        head: &IrCallSegment,
        chain: &[IrCallSegment],
        body: Option<CallBody<'_>>,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> Option<IrValue> {
        let mut value = self.chain_outcome(
            self.evaluate_call_value(
                &head.name,
                &head.positional_args,
                &head.named_args,
                None,
                &head.span,
                diagnostics,
                context,
            ),
            head,
            diagnostics,
            context,
        )?;

        for (index, source_segment) in chain.iter().enumerate() {
            let mut positional_args = Vec::with_capacity(1 + source_segment.positional_args.len());
            // The previous result is always first. Explicit positional
            // arguments follow it in their original order; named arguments
            // remain in the named collection untouched.
            positional_args.push(value);
            positional_args.extend(source_segment.positional_args.iter().cloned());
            let final_body = (index + 1 == chain.len()).then_some(body).flatten();
            value = self.chain_outcome(
                self.evaluate_call_value(
                    &source_segment.name,
                    &positional_args,
                    &source_segment.named_args,
                    final_body,
                    &source_segment.span,
                    diagnostics,
                    context,
                ),
                source_segment,
                diagnostics,
                context,
            )?;
        }

        Some(value)
    }

    fn chain_outcome(
        &self,
        outcome: CallOutcome,
        segment: &IrCallSegment,
        diagnostics: &mut Vec<Diagnostic>,
        context: &EvaluationContext,
    ) -> Option<IrValue> {
        match outcome {
            CallOutcome::Value(value) => Some(value),
            CallOutcome::NoValue => None,
            CallOutcome::Unresolved => {
                let message = if let Some(binding) = context.get_function(&segment.name) {
                    format!(
                        "Function `{}` is visible in this scope but callable function declarations are not implemented yet ({} parameter(s))",
                        segment.name,
                        binding.parameters.len()
                    )
                } else {
                    format!(
                        "Cannot evaluate chained call segment `.{}`: no semantic implementation is available",
                        segment.name
                    )
                };
                diagnostics.push(chain_evaluation_error(message, segment.name_span));
                None
            }
        }
    }

    /// Evaluates a call body only after its callee has selected that strategy.
    fn evaluate_call_body(
        &self,
        body: CallBody<'_>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> IrValue {
        match body {
            CallBody::Block(nodes) => {
                IrValue::Content(self.evaluate_nodes(nodes, diagnostics, context))
            }
            CallBody::Inline(inlines) => IrValue::Content(vec![IrNode::Paragraph {
                content: self.evaluate_inlines(inlines, diagnostics, context),
                span: *span,
            }]),
        }
    }

    /// Resolves only a conditional's condition. Body and content arguments
    /// remain lazy until the branch is known.
    fn resolve_call_condition(
        &self,
        name: &str,
        positional_args: &[IrValue],
        named_args: &[(String, IrValue)],
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> bool {
        let raw_condition = named_args
            .iter()
            .find(|(key, _)| key == "condition")
            .map(|(_, value)| value)
            .or_else(|| positional_args.first());
        let Some(raw_condition) = raw_condition else {
            diagnostics.push(unresolvable_condition(name, span));
            return false;
        };
        let Some(condition) = self.evaluate_value(raw_condition, diagnostics, context) else {
            diagnostics.push(unresolvable_condition(name, span));
            return false;
        };
        match resolve_boolean_value(&condition, context) {
            Some(value) => value,
            None => {
                diagnostics.push(unresolvable_condition(name, span));
                false
            }
        }
    }

    /// Produces conditional content after the condition has selected the
    /// branch. The body and body-like arguments are evaluated here, not before
    /// dispatch.
    fn conditional_content_value(
        &self,
        positional_args: &[IrValue],
        named_args: &[(String, IrValue)],
        body: Option<CallBody<'_>>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> IrValue {
        if let Some(body) = body {
            return self.evaluate_call_body(body, span, diagnostics, context);
        }
        if let Some((_, value)) = named_args.iter().find(|(key, _)| key == "body") {
            return self
                .evaluate_value(value, diagnostics, context)
                .map_or_else(
                    || IrValue::Content(Vec::new()),
                    |value| self.scalar_or_content(value, span),
                );
        }
        if let Some(value) = positional_args.get(1) {
            return self
                .evaluate_value(value, diagnostics, context)
                .map_or_else(
                    || IrValue::Content(Vec::new()),
                    |value| self.scalar_or_content(value, span),
                );
        }
        IrValue::Content(Vec::new())
    }

    fn scalar_or_content(&self, value: IrValue, span: &SourceSpan) -> IrValue {
        match value {
            IrValue::Content(nodes) => IrValue::Content(nodes),
            scalar => IrValue::Content(vec![IrNode::Paragraph {
                content: vec![IrInline::Text {
                    content: scalar_to_text(&scalar),
                    span: *span,
                }],
                span: *span,
            }]),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn preserve_block_call(
        &self,
        name: &str,
        positional_args: &[IrValue],
        named_args: &[(String, IrValue)],
        body: Option<&[IrNode]>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> Vec<IrNode> {
        let positional_args =
            self.evaluate_values_for_preservation(positional_args, diagnostics, context);
        let named_args = self.evaluate_named_for_preservation(named_args, diagnostics, context);
        vec![IrNode::FunctionCall {
            name: name.to_string(),
            positional_args,
            named_args,
            body: body.map(|nodes| self.evaluate_nodes(nodes, diagnostics, context)),
            span: *span,
        }]
    }

    #[allow(clippy::too_many_arguments)]
    fn preserve_inline_call(
        &self,
        name: &str,
        positional_args: &[IrValue],
        named_args: &[(String, IrValue)],
        body: Option<&[IrInline]>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> Vec<IrInline> {
        let positional_args =
            self.evaluate_values_for_preservation(positional_args, diagnostics, context);
        let named_args = self.evaluate_named_for_preservation(named_args, diagnostics, context);
        vec![IrInline::DirectiveCall {
            name: name.to_string(),
            positional_args,
            named_args,
            body: body.map(|inlines| self.evaluate_inlines(inlines, diagnostics, context)),
            span: *span,
        }]
    }

    fn materialize_block_value(&self, value: Option<IrValue>, span: &SourceSpan) -> Vec<IrNode> {
        match value {
            Some(IrValue::Content(nodes)) => nodes,
            Some(value) => vec![IrNode::Paragraph {
                content: vec![IrInline::Text {
                    content: scalar_to_text(&value),
                    span: *span,
                }],
                span: *span,
            }],
            None => Vec::new(),
        }
    }

    fn materialize_inline_value(&self, value: Option<IrValue>, span: &SourceSpan) -> Vec<IrInline> {
        match value {
            Some(IrValue::Content(nodes)) => VariableValue::Content(nodes).materialize_inline(span),
            Some(value) => vec![IrInline::Text {
                content: scalar_to_text(&value),
                span: *span,
            }],
            None => Vec::new(),
        }
    }

    /// Evaluates a value without entering document-output context.
    fn evaluate_value(
        &self,
        value: &IrValue,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> Option<IrValue> {
        match value {
            IrValue::Content(nodes) => {
                if let [IrNode::FunctionCall {
                    name,
                    positional_args,
                    named_args,
                    body,
                    span,
                }] = nodes.as_slice()
                {
                    return match self.evaluate_call_value(
                        name,
                        positional_args,
                        named_args,
                        body.as_deref().map(CallBody::Block),
                        span,
                        diagnostics,
                        context,
                    ) {
                        CallOutcome::Value(value) => Some(value),
                        CallOutcome::NoValue => None,
                        CallOutcome::Unresolved => {
                            Some(IrValue::Content(self.preserve_block_call(
                                name,
                                positional_args,
                                named_args,
                                body.as_deref(),
                                span,
                                diagnostics,
                                context,
                            )))
                        }
                    };
                }
                if let [IrNode::ChainedFunctionCall {
                    head, chain, body, ..
                }] = nodes.as_slice()
                {
                    return self.evaluate_chain_value(
                        head,
                        chain,
                        body.as_deref().map(CallBody::Block),
                        diagnostics,
                        context,
                    );
                }
                Some(IrValue::Content(self.evaluate_nodes(
                    nodes,
                    diagnostics,
                    context,
                )))
            }
            scalar => Some(scalar.clone()),
        }
    }

    fn evaluate_values(
        &self,
        values: &[IrValue],
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> Option<Vec<IrValue>> {
        values
            .iter()
            .map(|value| self.evaluate_value(value, diagnostics, context))
            .collect()
    }

    fn evaluate_named(
        &self,
        named: &[(String, IrValue)],
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> Option<Vec<(String, IrValue)>> {
        named
            .iter()
            .map(|(key, value)| {
                self.evaluate_value(value, diagnostics, context)
                    .map(|value| (key.clone(), value))
            })
            .collect()
    }

    fn evaluate_values_for_preservation(
        &self,
        values: &[IrValue],
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> Vec<IrValue> {
        values
            .iter()
            .map(|value| {
                self.evaluate_value(value, diagnostics, context)
                    .unwrap_or_else(|| IrValue::Content(Vec::new()))
            })
            .collect()
    }

    fn evaluate_named_for_preservation(
        &self,
        named: &[(String, IrValue)],
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> Vec<(String, IrValue)> {
        named
            .iter()
            .map(|(key, value)| {
                (
                    key.clone(),
                    self.evaluate_value(value, diagnostics, context)
                        .unwrap_or_else(|| IrValue::Content(Vec::new())),
                )
            })
            .collect()
    }
}

/// Returns true for the conditional constructs this evaluator resolves.
fn is_conditional(name: &str) -> bool {
    name == "if" || name == "ifnot"
}

fn chain_evaluation_error(message: String, span: SourceSpan) -> Diagnostic {
    Diagnostic {
        code: "E3001".to_string(),
        severity: Severity::Error,
        message,
        primary: Some(span),
        secondary: Vec::new(),
        hints: vec!["The evaluator did not fabricate a value for the failed call.".to_string()],
    }
}

/// Resolves a value to a boolean, handling variable references.
fn resolve_boolean_value(value: &IrValue, context: &EvaluationContext) -> Option<bool> {
    match value {
        IrValue::Boolean(v) => Some(*v),
        IrValue::Identifier(name) => match name.to_lowercase().as_str() {
            "true" | "yes" => Some(true),
            "false" | "no" => Some(false),
            _ => {
                // Check if it's a variable reference
                if let Some(var) = context.get(name) {
                    var.as_boolean()
                } else {
                    None
                }
            }
        },
        IrValue::Content(nodes) => {
            // Check if the content is a single parameterless function call to a known variable
            if let [IrNode::FunctionCall {
                name,
                positional_args,
                named_args,
                body,
                ..
            }] = nodes.as_slice()
            {
                if positional_args.is_empty() && named_args.is_empty() && body.is_none() {
                    if let Some(var) = context.get(name) {
                        return var.as_boolean();
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// Maps a scalar value to its boolean meaning (without variable resolution).
/// Supports the Quarkdown boolean literals `true`/`yes` and `false`/`no`,
/// case-insensitive (Quarkdown "Boolean" documentation, badged `v2.5.0`).
fn scalar_boolean_value(value: &IrValue) -> Option<bool> {
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

/// Decides whether a conditional's content is taken.
fn take_branch(name: &str, condition: bool) -> bool {
    if name == "if" {
        condition
    } else {
        !condition
    }
}

/// Returns true for `.var` declarations.
fn is_var_declaration(name: &str) -> bool {
    name == "var"
}

/// Returns true if a call is a variable reference (parameterless call to a known variable).
fn is_variable_reference_call(
    name: &str,
    positional_args: &[IrValue],
    named_args: &[(String, IrValue)],
    body: Option<CallBody<'_>>,
    context: &EvaluationContext,
) -> bool {
    // Variable reference: parameterless call (no positional args, no named args, no body)
    // to a name that exists in the variable environment.
    positional_args.is_empty() && named_args.is_empty() && body.is_none() && context.contains(name)
}

/// Returns true if a call is a variable reassignment (`.name {value}` where `name` is a known variable).
fn is_variable_reassignment_call(
    name: &str,
    positional_args: &[IrValue],
    named_args: &[(String, IrValue)],
    body: Option<CallBody<'_>>,
    context: &EvaluationContext,
) -> bool {
    // Variable reassignment: call to a known variable name with exactly one
    // positional argument (the new value), no named args, no body.
    context.contains(name) && positional_args.len() == 1 && named_args.is_empty() && body.is_none()
}

/// Builds the `E3002` diagnostic for an invalid variable declaration.
fn invalid_var_declaration(span: &SourceSpan) -> Diagnostic {
    Diagnostic {
        code: "E3002".to_string(),
        severity: Severity::Error,
        message: "`.var` declaration requires a name and a value (body, second positional argument, or named `value`/`body` argument)".to_string(),
        primary: Some(*span),
        secondary: Vec::new(),
        hints: vec![
            "Use `.var {name} {value}` or `.var {name}\n    content` for block variables.".to_string(),
        ],
    }
}

/// Builds the `E3002` diagnostic for an invalid variable name.
fn invalid_var_name(name: &str, span: &SourceSpan) -> Diagnostic {
    Diagnostic {
        code: "E3002".to_string(),
        severity: Severity::Error,
        message: format!("Invalid variable name `{name}`: must match `[A-Za-z_][A-Za-z0-9_-]*`"),
        primary: Some(*span),
        secondary: Vec::new(),
        hints: vec!["Variable names must start with a letter or underscore, followed by letters, digits, underscores, or hyphens.".to_string()],
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
            "`{name}` requires a boolean-compatible condition (literals `true`, `false`, `yes`, `no`, or variable reference `.name`) as its `condition` argument"
        ),
        primary: Some(*span),
        secondary: Vec::new(),
        hints: vec!["Condition must be a boolean literal or a variable reference that resolves to a boolean.".to_string()],
    }
}

impl Evaluator {
    // Variable handling methods

    /// Handles a `.var` declaration in value context.
    #[allow(clippy::too_many_arguments)]
    fn handle_var_declaration(
        &self,
        positional_args: &[IrValue],
        named_args: &[(String, IrValue)],
        body: Option<CallBody<'_>>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> CallOutcome {
        // Check for malformed declaration: must have a name (first positional arg)
        let var_name = match positional_args.first() {
            Some(IrValue::Identifier(name)) => name.clone(),
            Some(IrValue::String(name)) => name.clone(),
            _ => {
                diagnostics.push(invalid_var_declaration(span));
                return CallOutcome::NoValue;
            }
        };
        // Validate variable name
        if !is_valid_normal_call_name(&var_name) {
            diagnostics.push(invalid_var_name(&var_name, span));
            return CallOutcome::NoValue;
        }

        // Determine the value: body > named "body" > second positional > named "value" > empty
        if let Some(body) = body {
            if let IrValue::Content(nodes) =
                self.evaluate_call_body(body, span, diagnostics, context)
            {
                context.set_content(var_name, nodes);
                return CallOutcome::NoValue;
            }
        }

        // Check named "body" argument
        if let Some((_, value)) = named_args.iter().find(|(key, _)| key == "body") {
            if let Some(value) = self.evaluate_value(value, diagnostics, context) {
                match value {
                    IrValue::Content(nodes) => context.set_content(var_name, nodes),
                    scalar => context.set_value(var_name, scalar),
                }
                return CallOutcome::NoValue;
            }
        }

        // Check named "value" argument
        if let Some((_, value)) = named_args.iter().find(|(key, _)| key == "value") {
            if let Some(value) = self.evaluate_value(value, diagnostics, context) {
                context.set_value(var_name, value);
                return CallOutcome::NoValue;
            }
        }

        // Fall back to second positional argument
        if let Some(value) = positional_args.get(1) {
            if let Some(value) = self.evaluate_value(value, diagnostics, context) {
                context.set_value(var_name, value);
                return CallOutcome::NoValue;
            }
        }

        // No value provided - invalid declaration
        diagnostics.push(invalid_var_declaration(span));
        CallOutcome::NoValue
    }

    /// Handles a variable reassignment in value context.
    fn handle_variable_reassignment_value(
        &self,
        name: &str,
        positional_args: &[IrValue],
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> CallOutcome {
        let Some(new_value) = self.evaluate_value(&positional_args[0], diagnostics, context) else {
            return CallOutcome::NoValue;
        };
        context.set_value(name.to_string(), new_value);
        CallOutcome::NoValue
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::IrListItem;
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

    fn chain_segment(
        name: &str,
        start: usize,
        end: usize,
        positional_args: Vec<IrValue>,
    ) -> IrCallSegment {
        IrCallSegment {
            name: name.to_string(),
            name_span: span(start, start + name.len() + usize::from(start == 0)),
            positional_args,
            named_args: Vec::new(),
            span: span(start, end),
        }
    }

    fn chain_node(head: IrCallSegment, chain: Vec<IrCallSegment>) -> IrNode {
        let span = span(
            head.span.start,
            chain
                .last()
                .map_or(head.span.end, |segment| segment.span.end),
        );
        IrNode::ChainedFunctionCall {
            head,
            chain,
            body: None,
            span,
        }
    }

    fn chain_node_with_body(
        head: IrCallSegment,
        chain: Vec<IrCallSegment>,
        body: Vec<IrNode>,
    ) -> IrNode {
        let span = span(
            head.span.start,
            chain
                .last()
                .map_or(head.span.end, |segment| segment.span.end),
        );
        IrNode::ChainedFunctionCall {
            head,
            chain,
            body: Some(body),
            span,
        }
    }

    fn call_value(name: &str, positional_args: Vec<IrValue>) -> IrValue {
        IrValue::Content(vec![IrNode::FunctionCall {
            name: name.to_string(),
            positional_args,
            named_args: Vec::new(),
            body: None,
            span: span(0, 1),
        }])
    }

    fn assert_paragraph_text(nodes: &[IrNode], expected: &str) {
        let [IrNode::Paragraph { content, .. }] = nodes else {
            panic!("expected one paragraph, got {nodes:?}");
        };
        let [IrInline::Text { content, .. }] = content.as_slice() else {
            panic!("expected one text fragment, got {content:?}");
        };
        assert_eq!(content, expected);
    }

    #[test]
    fn unknown_chain_callee_reports_a_segment_diagnostic() {
        let whole = span(0, 13);
        let (nodes, diagnostics) = evaluate_with_diagnostics(vec![IrNode::ChainedFunctionCall {
            head: IrCallSegment {
                name: "a".into(),
                name_span: span(0, 2),
                positional_args: vec![IrValue::Identifier("x".into())],
                named_args: Vec::new(),
                span: whole,
            },
            chain: vec![IrCallSegment {
                name: "b".into(),
                name_span: span(8, 9),
                positional_args: vec![IrValue::Identifier("y".into())],
                named_args: Vec::new(),
                span: span(8, 13),
            }],
            body: None,
            span: whole,
        }]);
        assert!(nodes.is_empty());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E3001");
        assert_eq!(diagnostics[0].primary, Some(span(0, 2)));
    }

    #[test]
    fn unknown_middle_and_tail_segments_fail_at_their_names() {
        let cases = [
            (
                vec![
                    chain_segment(
                        "uppercase",
                        0,
                        17,
                        vec![IrValue::Identifier("hello".into())],
                    ),
                    chain_segment("unknown", 19, 28, Vec::new()),
                    chain_segment("lowercase", 30, 39, Vec::new()),
                ],
                span(19, 26),
            ),
            (
                vec![
                    chain_segment(
                        "uppercase",
                        0,
                        17,
                        vec![IrValue::Identifier("hello".into())],
                    ),
                    chain_segment("lowercase", 19, 29, Vec::new()),
                    chain_segment("unknown", 31, 40, Vec::new()),
                ],
                span(31, 38),
            ),
        ];

        for (segments, expected_span) in cases {
            let (nodes, diagnostics) = evaluate_with_diagnostics(vec![chain_node(
                segments[0].clone(),
                segments[1..].to_vec(),
            )]);
            assert!(nodes.is_empty());
            assert_eq!(diagnostics.len(), 1);
            assert_eq!(diagnostics[0].code, "E3001");
            assert_eq!(diagnostics[0].primary, Some(expected_span));
        }
    }

    #[test]
    fn chain_arity_and_type_failures_are_deterministic() {
        let cases = [
            chain_node(
                chain_segment("uppercase", 0, 10, Vec::new()),
                vec![chain_segment("lowercase", 12, 21, Vec::new())],
            ),
            chain_node(
                chain_segment("sum", 0, 8, vec![IrValue::Boolean(true)]),
                vec![chain_segment(
                    "multiply",
                    10,
                    19,
                    vec![IrValue::Number(2.0)],
                )],
            ),
        ];
        for input in cases {
            let first = Evaluator::new().evaluate(&doc(vec![input.clone()]));
            let second = Evaluator::new().evaluate(&doc(vec![input]));
            assert!(first.0.nodes.is_empty());
            assert_eq!(first.1.len(), 1);
            assert_eq!(second.1.len(), 1);
            assert_eq!(first.1[0].code, "E3001");
            assert_eq!(first.1[0].message, second.1[0].message);
            assert_eq!(first.1[0].primary, second.1[0].primary);
        }
    }

    #[test]
    fn chain_value_flow_is_left_to_right_and_injects_first() {
        let (nodes, diagnostics) = evaluate_with_diagnostics(vec![chain_node(
            chain_segment(
                "sum",
                0,
                12,
                vec![IrValue::Number(10.0), IrValue::Number(5.0)],
            ),
            vec![chain_segment(
                "multiply",
                14,
                27,
                vec![IrValue::Number(2.0)],
            )],
        )]);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_paragraph_text(&nodes, "30");
    }

    #[test]
    fn chain_zero_argument_segments_compose_scalar_values() {
        let (nodes, diagnostics) = evaluate_with_diagnostics(vec![chain_node(
            chain_segment(
                "uppercase",
                0,
                17,
                vec![IrValue::Identifier("hello".into())],
            ),
            vec![
                chain_segment("uppercase", 19, 28, Vec::new()),
                chain_segment("lowercase", 30, 39, Vec::new()),
            ],
        )]);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_paragraph_text(&nodes, "hello");
    }

    #[test]
    fn chain_preserves_explicit_positional_arguments_after_previous_value() {
        let (nodes, diagnostics) = evaluate_with_diagnostics(vec![chain_node(
            chain_segment(
                "sum",
                0,
                12,
                vec![IrValue::Number(10.0), IrValue::Number(5.0)],
            ),
            vec![chain_segment(
                "multiply",
                14,
                27,
                vec![IrValue::Number(2.0)],
            )],
        )]);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        // multiply receives [sum(10, 5), 2], not [2, sum(10, 5)].
        assert_paragraph_text(&nodes, "30");
    }

    #[test]
    fn chain_keeps_named_arguments_named_while_injecting_previous_value() {
        let mut segment = chain_segment("multiply", 14, 29, Vec::new());
        segment.named_args.push(("by".into(), IrValue::Number(2.0)));
        let (nodes, diagnostics) = evaluate_with_diagnostics(vec![chain_node(
            chain_segment(
                "sum",
                0,
                12,
                vec![IrValue::Number(10.0), IrValue::Number(5.0)],
            ),
            vec![segment],
        )]);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_paragraph_text(&nodes, "30");
    }

    #[test]
    fn nested_call_and_chain_share_the_same_value_context() {
        let nested = IrNode::FunctionCall {
            name: "multiply".into(),
            positional_args: vec![
                call_value("sum", vec![IrValue::Number(10.0), IrValue::Number(5.0)]),
                IrValue::Number(2.0),
            ],
            named_args: Vec::new(),
            body: None,
            span: span(0, 1),
        };
        let chain = chain_node(
            chain_segment(
                "sum",
                0,
                12,
                vec![IrValue::Number(10.0), IrValue::Number(5.0)],
            ),
            vec![chain_segment(
                "multiply",
                14,
                27,
                vec![IrValue::Number(2.0)],
            )],
        );
        let (nested_nodes, nested_diagnostics) = Evaluator::new().evaluate(&doc(vec![nested]));
        let (chain_nodes, chain_diagnostics) = Evaluator::new().evaluate(&doc(vec![chain]));
        assert!(nested_diagnostics.is_empty(), "{nested_diagnostics:?}");
        assert!(chain_diagnostics.is_empty(), "{chain_diagnostics:?}");
        assert_paragraph_text(&nested_nodes.nodes, "30");
        assert_paragraph_text(&chain_nodes.nodes, "30");
    }

    #[test]
    fn nested_and_chained_case_transforms_share_dynamic_scalar_adaptation() {
        let nested = IrNode::FunctionCall {
            name: "lowercase".into(),
            positional_args: vec![call_value(
                "uppercase",
                vec![IrValue::Identifier("hello".into())],
            )],
            named_args: Vec::new(),
            body: None,
            span: span(0, 1),
        };
        let chain = chain_node(
            chain_segment(
                "uppercase",
                0,
                17,
                vec![IrValue::Identifier("hello".into())],
            ),
            vec![chain_segment("lowercase", 19, 28, Vec::new())],
        );
        let (nested_nodes, nested_diagnostics) = Evaluator::new().evaluate(&doc(vec![nested]));
        let (chain_nodes, chain_diagnostics) = Evaluator::new().evaluate(&doc(vec![chain]));
        assert!(nested_diagnostics.is_empty(), "{nested_diagnostics:?}");
        assert!(chain_diagnostics.is_empty(), "{chain_diagnostics:?}");
        assert_paragraph_text(&nested_nodes.nodes, "hello");
        assert_paragraph_text(&chain_nodes.nodes, "hello");
    }

    #[test]
    fn variable_values_remain_semantic_through_nested_and_chained_calls() {
        let nested = vec![
            var_declaration("myvar", IrValue::Boolean(true)),
            IrNode::FunctionCall {
                name: "uppercase".into(),
                positional_args: vec![call_value("myvar", Vec::new())],
                named_args: Vec::new(),
                body: None,
                span: span(0, 1),
            },
        ];
        let chained = vec![
            var_declaration("myvar", IrValue::Boolean(true)),
            chain_node(
                chain_segment("myvar", 0, 6, Vec::new()),
                vec![chain_segment("uppercase", 8, 18, Vec::new())],
            ),
        ];
        let (nested_nodes, nested_diagnostics) = Evaluator::new().evaluate(&doc(nested));
        let (chain_nodes, chain_diagnostics) = Evaluator::new().evaluate(&doc(chained));
        assert!(nested_diagnostics.is_empty(), "{nested_diagnostics:?}");
        assert!(chain_diagnostics.is_empty(), "{chain_diagnostics:?}");
        assert_paragraph_text(&nested_nodes.nodes, "TRUE");
        assert_paragraph_text(&chain_nodes.nodes, "TRUE");
    }

    #[test]
    fn false_final_conditional_chain_does_not_evaluate_its_body() {
        let chain = vec![
            var_declaration("flag", IrValue::Boolean(false)),
            var_declaration("x", IrValue::Identifier("before".into())),
            chain_node_with_body(
                chain_segment("flag", 0, 5, Vec::new()),
                vec![chain_segment("if", 7, 10, Vec::new())],
                vec![var_reassignment("x", IrValue::Identifier("after".into()))],
            ),
            var_ref("x"),
        ];
        let ordinary = vec![
            var_declaration("flag", IrValue::Boolean(false)),
            var_declaration("x", IrValue::Identifier("before".into())),
            if_call(
                "if",
                IrValue::Boolean(false),
                vec![var_reassignment("x", IrValue::Identifier("after".into()))],
            ),
            var_ref("x"),
        ];
        let (chain_nodes, chain_diagnostics) = Evaluator::new().evaluate(&doc(chain));
        let (ordinary_nodes, ordinary_diagnostics) = Evaluator::new().evaluate(&doc(ordinary));
        assert!(chain_diagnostics.is_empty(), "{chain_diagnostics:?}");
        assert!(ordinary_diagnostics.is_empty(), "{ordinary_diagnostics:?}");
        assert_paragraph_text(&chain_nodes.nodes, "before");
        assert_paragraph_text(&ordinary_nodes.nodes, "before");
    }

    #[test]
    fn false_final_inline_conditional_chain_does_not_evaluate_its_body() {
        let chain = vec![
            var_declaration("flag", IrValue::Boolean(false)),
            var_declaration("x", IrValue::Identifier("before".into())),
            IrNode::Paragraph {
                content: vec![
                    IrInline::ChainedDirectiveCall {
                        head: chain_segment("flag", 0, 5, Vec::new()),
                        chain: vec![chain_segment("if", 7, 10, Vec::new())],
                        body: Some(vec![IrInline::DirectiveCall {
                            name: "x".into(),
                            positional_args: vec![IrValue::Identifier("after".into())],
                            named_args: Vec::new(),
                            body: None,
                            span: span(0, 1),
                        }]),
                        span: span(0, 10),
                    },
                    inline_var_ref("x"),
                ],
                span: span(0, 10),
            },
        ];
        let ordinary = vec![
            var_declaration("flag", IrValue::Boolean(false)),
            var_declaration("x", IrValue::Identifier("before".into())),
            IrNode::Paragraph {
                content: vec![
                    inline_if_call(
                        "if",
                        IrValue::Boolean(false),
                        vec![IrInline::DirectiveCall {
                            name: "x".into(),
                            positional_args: vec![IrValue::Identifier("after".into())],
                            named_args: Vec::new(),
                            body: None,
                            span: span(0, 1),
                        }],
                    ),
                    inline_var_ref("x"),
                ],
                span: span(0, 10),
            },
        ];
        let (chain_nodes, chain_diagnostics) = Evaluator::new().evaluate(&doc(chain));
        let (ordinary_nodes, ordinary_diagnostics) = Evaluator::new().evaluate(&doc(ordinary));
        assert!(chain_diagnostics.is_empty(), "{chain_diagnostics:?}");
        assert!(ordinary_diagnostics.is_empty(), "{ordinary_diagnostics:?}");
        let text = |nodes: &IrDocument| match &nodes.nodes[0] {
            IrNode::Paragraph { content, .. } => content
                .iter()
                .filter_map(|inline| match inline {
                    IrInline::Text { content, .. } => Some(content.as_str()),
                    _ => None,
                })
                .collect::<String>(),
            _ => String::new(),
        };
        assert_eq!(text(&chain_nodes), "before");
        assert_eq!(text(&ordinary_nodes), "before");
    }

    #[test]
    fn child_scope_inherits_parent_and_isolates_local_bindings() {
        let mut parent = EvaluationContext::new();
        parent.set_value("visible".into(), IrValue::String("parent".into()));
        parent.set_function("inherited".into(), vec!["value".into()]);

        let mut child = parent.child();
        assert_eq!(
            child.get("visible").map(VariableValue::to_value),
            Some(IrValue::String("parent".into()))
        );
        assert_eq!(
            child
                .get_function("inherited")
                .map(|binding| binding.parameters.as_slice()),
            Some(["value".to_string()].as_slice())
        );
        child.set_value("local".into(), IrValue::String("child".into()));
        child.set_function("future".into(), vec!["value".into()]);

        assert!(parent.get("local").is_none());
        assert_eq!(
            child.get("local").map(VariableValue::to_value),
            Some(IrValue::String("child".into()))
        );
        assert_eq!(
            child
                .get_function("future")
                .map(|binding| binding.parameters.as_slice()),
            Some(["value".to_string()].as_slice())
        );
        child.set_value("visible".into(), IrValue::String("shadowed".into()));
        child.set_function("inherited".into(), vec!["shadowed".into()]);
        assert_eq!(
            child.get("visible").map(VariableValue::to_value),
            Some(IrValue::String("shadowed".into()))
        );
        assert_eq!(
            parent.get("visible").map(VariableValue::to_value),
            Some(IrValue::String("parent".into()))
        );
        assert_eq!(
            child
                .get_function("inherited")
                .map(|binding| binding.parameters.as_slice()),
            Some(["shadowed".to_string()].as_slice())
        );
        assert_eq!(
            parent
                .get_function("inherited")
                .map(|binding| binding.parameters.as_slice()),
            Some(["value".to_string()].as_slice())
        );
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
    fn link_evaluates_content_inside_label() {
        let paragraph = IrNode::Paragraph {
            content: vec![IrInline::Link {
                content: vec![
                    text_inline("before "),
                    inline_if_call("if", IrValue::Boolean(true), vec![text_inline("kept")]),
                    text_inline(" after"),
                ],
                destination: "https://example.com".to_string(),
                span: span(0, 1),
            }],
            span: span(0, 1),
        };
        let nodes = evaluate(vec![paragraph]);
        let IrNode::Paragraph { content, .. } = &nodes[0] else {
            panic!("expected paragraph");
        };
        assert_eq!(content.len(), 1);
        let IrInline::Link {
            content,
            destination,
            span: link_span,
        } = &content[0]
        else {
            panic!("expected link");
        };
        assert_eq!(destination, "https://example.com");
        assert_eq!(*link_span, span(0, 1));
        assert_eq!(
            content,
            &vec![
                text_inline("before "),
                text_inline("kept"),
                text_inline(" after")
            ]
        );
    }

    #[test]
    fn structures_recurse_through_evaluator_without_losing_semantics() {
        let document = doc(vec![
            IrNode::Blockquote {
                content: vec![if_call(
                    "if",
                    IrValue::Boolean(true),
                    vec![text_paragraph("quoted")],
                )],
                span: span(0, 10),
            },
            IrNode::UnorderedList {
                items: vec![IrListItem {
                    nodes: vec![if_call(
                        "if",
                        IrValue::Boolean(true),
                        vec![text_paragraph("task content")],
                    )],
                    task: Some(crate::ir::IrTaskStatus::Completed),
                    span: span(10, 30),
                }],
                span: span(10, 30),
            },
            IrNode::Paragraph {
                content: vec![IrInline::Strikethrough {
                    content: vec![inline_if_call(
                        "if",
                        IrValue::Boolean(true),
                        vec![text_inline("struck")],
                    )],
                    span: span(30, 40),
                }],
                span: span(30, 40),
            },
            IrNode::Table {
                header: crate::ir::IrTableRow {
                    cells: vec![crate::ir::IrTableCell {
                        content: vec![text_inline("Header")],
                        alignment: crate::ir::IrTableAlignment::Center,
                        span: span(40, 46),
                    }],
                    span: span(40, 46),
                },
                rows: vec![crate::ir::IrTableRow {
                    cells: vec![crate::ir::IrTableCell {
                        content: vec![inline_if_call(
                            "if",
                            IrValue::Boolean(true),
                            vec![text_inline("cell")],
                        )],
                        alignment: crate::ir::IrTableAlignment::None,
                        span: span(46, 50),
                    }],
                    span: span(46, 50),
                }],
                span: span(40, 50),
            },
        ]);

        let (evaluated, diagnostics) = Evaluator::new().evaluate(&document);
        assert!(
            diagnostics.is_empty(),
            "unexpected diagnostics: {diagnostics:?}"
        );

        let IrNode::Blockquote { content, .. } = &evaluated.nodes[0] else {
            panic!("expected blockquote")
        };
        assert_eq!(content, &vec![text_paragraph("quoted")]);

        let IrNode::UnorderedList { items, .. } = &evaluated.nodes[1] else {
            panic!("expected list")
        };
        assert_eq!(items[0].task, Some(crate::ir::IrTaskStatus::Completed));
        assert_eq!(items[0].nodes, vec![text_paragraph("task content")]);

        let IrNode::Paragraph { content, .. } = &evaluated.nodes[2] else {
            panic!("expected paragraph")
        };
        assert_eq!(
            content,
            &vec![IrInline::Strikethrough {
                content: vec![text_inline("struck")],
                span: span(30, 40),
            }]
        );

        let IrNode::Table { header, rows, .. } = &evaluated.nodes[3] else {
            panic!("expected table")
        };
        assert_eq!(header.cells[0].content, vec![text_inline("Header")]);
        assert_eq!(rows[0].cells[0].content, vec![text_inline("cell")]);
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

    // =========================================================================
    // Variable evaluation tests (M2)
    // =========================================================================

    fn var_declaration(name: &str, value: IrValue) -> IrNode {
        IrNode::FunctionCall {
            name: "var".to_string(),
            positional_args: vec![IrValue::Identifier(name.to_string()), value],
            named_args: Vec::new(),
            body: None,
            span: span(0, 1),
        }
    }

    fn var_declaration_with_body(name: &str, body_nodes: Vec<IrNode>) -> IrNode {
        IrNode::FunctionCall {
            name: "var".to_string(),
            positional_args: vec![IrValue::Identifier(name.to_string())],
            named_args: Vec::new(),
            body: Some(body_nodes),
            span: span(0, 1),
        }
    }

    fn var_ref(name: &str) -> IrNode {
        IrNode::FunctionCall {
            name: name.to_string(),
            positional_args: Vec::new(),
            named_args: Vec::new(),
            body: None,
            span: span(0, 1),
        }
    }

    fn var_reassignment(name: &str, value: IrValue) -> IrNode {
        IrNode::FunctionCall {
            name: name.to_string(),
            positional_args: vec![value],
            named_args: Vec::new(),
            body: None,
            span: span(0, 1),
        }
    }

    fn inline_var_ref(name: &str) -> IrInline {
        IrInline::DirectiveCall {
            name: name.to_string(),
            positional_args: Vec::new(),
            named_args: Vec::new(),
            body: None,
            span: span(0, 1),
        }
    }

    fn evaluate_with_diagnostics(nodes: Vec<IrNode>) -> (Vec<IrNode>, Vec<Diagnostic>) {
        let (result, diagnostics) = Evaluator::new().evaluate(&doc(nodes));
        (result.nodes, diagnostics)
    }

    #[test]
    fn var_scalar_definition_and_reference() {
        let nodes = evaluate(vec![
            var_declaration("name", IrValue::String("Scribium".to_string())),
            var_ref("name"),
        ]);
        assert_eq!(nodes.len(), 1);
        let IrNode::Paragraph { content, .. } = &nodes[0] else {
            panic!()
        };
        let IrInline::Text { content: text, .. } = &content[0] else {
            panic!()
        };
        assert_eq!(text, "Scribium");
    }

    #[test]
    fn var_boolean_reference_in_conditional() {
        let (nodes, diagnostics) = evaluate_with_diagnostics(vec![
            var_declaration("enabled", IrValue::Identifier("yes".to_string())),
            IrNode::FunctionCall {
                name: "if".to_string(),
                positional_args: vec![IrValue::Identifier("enabled".to_string())],
                named_args: Vec::new(),
                body: Some(vec![text_paragraph("visible")]),
                span: span(0, 1),
            },
        ]);
        assert!(diagnostics.is_empty());
        assert_eq!(nodes.len(), 1);
        let IrNode::Paragraph { content, .. } = &nodes[0] else {
            panic!()
        };
        let IrInline::Text { content: text, .. } = &content[0] else {
            panic!()
        };
        assert_eq!(text, "visible");
    }

    #[test]
    fn var_false_boolean_drops_conditional() {
        let nodes = evaluate(vec![
            var_declaration("enabled", IrValue::Identifier("no".to_string())),
            IrNode::FunctionCall {
                name: "if".to_string(),
                positional_args: vec![IrValue::Identifier("enabled".to_string())],
                named_args: Vec::new(),
                body: Some(vec![text_paragraph("hidden")]),
                span: span(0, 1),
            },
        ]);
        assert!(nodes.is_empty());
    }

    #[test]
    fn var_ifnot_with_variable() {
        let nodes = evaluate(vec![
            var_declaration("enabled", IrValue::Identifier("no".to_string())),
            IrNode::FunctionCall {
                name: "ifnot".to_string(),
                positional_args: vec![IrValue::Identifier("enabled".to_string())],
                named_args: Vec::new(),
                body: Some(vec![text_paragraph("visible")]),
                span: span(0, 1),
            },
        ]);
        assert_eq!(nodes.len(), 1);
        let IrNode::Paragraph { content, .. } = &nodes[0] else {
            panic!()
        };
        let IrInline::Text { content: text, .. } = &content[0] else {
            panic!()
        };
        assert_eq!(text, "visible");
    }

    #[test]
    fn var_explicit_reassignment() {
        let nodes = evaluate(vec![
            var_declaration("name", IrValue::String("A".to_string())),
            var_declaration("name", IrValue::String("B".to_string())),
            var_ref("name"),
        ]);
        assert_eq!(nodes.len(), 1);
        let IrNode::Paragraph { content, .. } = &nodes[0] else {
            panic!()
        };
        let IrInline::Text { content: text, .. } = &content[0] else {
            panic!()
        };
        assert_eq!(text, "B");
    }

    #[test]
    fn var_variable_name_reassignment() {
        let (nodes, diagnostics) = evaluate_with_diagnostics(vec![
            var_declaration("name", IrValue::String("A".to_string())),
            var_ref("name"),
            var_reassignment("name", IrValue::String("B".to_string())),
            var_ref("name"),
        ]);
        assert!(diagnostics.is_empty());
        assert_eq!(nodes.len(), 2);
        let IrNode::Paragraph { content, .. } = &nodes[0] else {
            panic!()
        };
        let IrInline::Text { content: text, .. } = &content[0] else {
            panic!()
        };
        assert_eq!(text, "A");
        let IrNode::Paragraph { content, .. } = &nodes[1] else {
            panic!()
        };
        let IrInline::Text { content: text, .. } = &content[0] else {
            panic!()
        };
        assert_eq!(text, "B");
    }

    #[test]
    fn var_reassignment_produces_no_output() {
        let nodes = evaluate(vec![
            var_declaration("name", IrValue::String("A".to_string())),
            var_reassignment("name", IrValue::String("B".to_string())),
        ]);
        assert!(nodes.is_empty());
    }

    #[test]
    fn var_inline_use() {
        let paragraph = IrNode::Paragraph {
            content: vec![
                text_inline("Hello "),
                inline_var_ref("name"),
                text_inline("!"),
            ],
            span: span(0, 1),
        };
        let nodes = evaluate(vec![
            var_declaration("name", IrValue::String("world".to_string())),
            paragraph,
        ]);
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
        assert_eq!(rendered, vec!["Hello ", "world", "!"]);
    }

    #[test]
    fn var_block_variable() {
        let body = vec![
            IrNode::Heading {
                level: 1,
                content: vec![text_inline("Title")],
                span: span(0, 1),
            },
            text_paragraph("body"),
        ];
        let nodes = evaluate(vec![
            var_declaration_with_body("section", body),
            var_ref("section"),
        ]);
        assert_eq!(nodes.len(), 2);
        let IrNode::Heading { content, .. } = &nodes[0] else {
            panic!()
        };
        let IrInline::Text { content: text, .. } = &content[0] else {
            panic!()
        };
        assert_eq!(text, "Title");
        let IrNode::Paragraph { content, .. } = &nodes[1] else {
            panic!()
        };
        let IrInline::Text { content: text, .. } = &content[0] else {
            panic!()
        };
        assert_eq!(text, "body");
    }

    #[test]
    fn var_conditional_declaration_execution_order() {
        let (nodes, diagnostics) = evaluate_with_diagnostics(vec![
            IrNode::FunctionCall {
                name: "if".to_string(),
                positional_args: vec![IrValue::Boolean(false)],
                named_args: Vec::new(),
                body: Some(vec![var_declaration(
                    "x",
                    IrValue::String("hidden".to_string()),
                )]),
                span: span(0, 1),
            },
            var_ref("x"),
        ]);
        assert!(diagnostics.is_empty());
        // x should not be declared, so var_ref("x") is preserved as function call
        assert_eq!(nodes.len(), 1);
        let IrNode::FunctionCall { name, .. } = &nodes[0] else {
            panic!()
        };
        assert_eq!(name, "x");
    }

    #[test]
    fn var_unknown_call_preserved() {
        let nodes = evaluate(vec![var_ref("unknown")]);
        assert_eq!(nodes.len(), 1);
        let IrNode::FunctionCall { name, .. } = &nodes[0] else {
            panic!()
        };
        assert_eq!(name, "unknown");
    }

    #[test]
    fn var_malformed_declaration_reports_e3002() {
        let call = IrNode::FunctionCall {
            name: "var".to_string(),
            positional_args: Vec::new(),
            named_args: Vec::new(),
            body: None,
            span: span(3, 6),
        };
        let (result, diagnostics) = Evaluator::new().evaluate(&doc(vec![call]));
        assert!(result.nodes.is_empty());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E3002");
        assert!(matches!(diagnostics[0].severity, Severity::Error));
        assert_eq!(diagnostics[0].primary, Some(span(3, 6)));
    }

    #[test]
    fn var_nested_evaluation_in_block_variable() {
        let body = vec![IrNode::FunctionCall {
            name: "if".to_string(),
            positional_args: vec![IrValue::Boolean(true)],
            named_args: Vec::new(),
            body: Some(vec![text_paragraph("nested visible")]),
            span: span(0, 1),
        }];
        let nodes = evaluate(vec![
            var_declaration_with_body("section", body),
            var_ref("section"),
        ]);
        assert_eq!(nodes.len(), 1);
        let IrNode::Paragraph { content, .. } = &nodes[0] else {
            panic!()
        };
        let IrInline::Text { content: text, .. } = &content[0] else {
            panic!()
        };
        assert_eq!(text, "nested visible");
    }

    #[test]
    fn var_evaluation_immutable_and_deterministic() {
        let input = doc(vec![
            var_declaration("name", IrValue::String("A".to_string())),
            var_ref("name"),
        ]);
        let first = Evaluator::new().evaluate(&input);
        assert_eq!(input.nodes.len(), 2);
        let second = Evaluator::new().evaluate(&input);
        assert_eq!(first.0, second.0);
        assert!(first.1.is_empty() && second.1.is_empty());
    }

    #[test]
    fn var_content_value_block_reference() {
        // .var {x} {**hello**} should preserve the strong content
        let strong_hello = IrNode::Paragraph {
            content: vec![IrInline::Strong {
                content: vec![IrInline::Text {
                    content: "hello".to_string(),
                    span: span(0, 5),
                }],
                span: span(0, 5),
            }],
            span: span(0, 11),
        };
        let nodes = evaluate(vec![
            var_declaration("x", IrValue::Content(vec![strong_hello.clone()])),
            var_ref("x"),
        ]);
        assert_eq!(nodes.len(), 1);
        let IrNode::Paragraph { content, .. } = &nodes[0] else {
            panic!()
        };
        let IrInline::Strong {
            content: strong_content,
            ..
        } = &content[0]
        else {
            panic!()
        };
        let IrInline::Text { content: text, .. } = &strong_content[0] else {
            panic!()
        };
        assert_eq!(text, "hello");
    }

    #[test]
    fn var_content_value_inline_reference() {
        // .var {x} {**world**} / Hello .x should preserve strong in inline context
        let strong_world = IrInline::Strong {
            content: vec![IrInline::Text {
                content: "world".to_string(),
                span: span(0, 5),
            }],
            span: span(0, 5),
        };
        let paragraph = IrNode::Paragraph {
            content: vec![
                IrInline::Text {
                    content: "Hello ".to_string(),
                    span: span(0, 6),
                },
                inline_var_ref("x"),
            ],
            span: span(0, 1),
        };
        let nodes = evaluate(vec![
            var_declaration(
                "x",
                IrValue::Content(vec![IrNode::Paragraph {
                    content: vec![strong_world.clone()],
                    span: span(0, 1),
                }]),
            ),
            paragraph,
        ]);
        let IrNode::Paragraph { content, .. } = &nodes[0] else {
            panic!()
        };
        assert_eq!(content.len(), 2);
        let IrInline::Text { content: text, .. } = &content[0] else {
            panic!()
        };
        assert_eq!(text, "Hello ");
        let IrInline::Strong {
            content: strong_content,
            ..
        } = &content[1]
        else {
            panic!()
        };
        let IrInline::Text { content: text, .. } = &strong_content[0] else {
            panic!()
        };
        assert_eq!(text, "world");
    }

    #[test]
    fn var_reference_with_body_is_not_variable_reference() {
        // .var {foo} {value} / .foo { body } should preserve the call with body
        let body = vec![text_paragraph("body")];
        let nodes = evaluate(vec![
            var_declaration("foo", IrValue::String("value".to_string())),
            IrNode::FunctionCall {
                name: "foo".to_string(),
                positional_args: Vec::new(),
                named_args: Vec::new(),
                body: Some(body),
                span: span(0, 1),
            },
        ]);
        // Should be preserved as function call, not variable reference
        assert_eq!(nodes.len(), 1);
        let IrNode::FunctionCall {
            name,
            body: call_body,
            ..
        } = &nodes[0]
        else {
            panic!()
        };
        assert_eq!(name, "foo");
        assert!(call_body.is_some());
        assert_eq!(call_body.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn var_invalid_name_reports_e3002() {
        // .var {"bad name"} {hello} should report E3002
        let call = IrNode::FunctionCall {
            name: "var".to_string(),
            positional_args: vec![
                IrValue::String("bad name".to_string()),
                IrValue::String("hello".to_string()),
            ],
            named_args: Vec::new(),
            body: None,
            span: span(0, 25),
        };
        let (result, diagnostics) = Evaluator::new().evaluate(&doc(vec![call]));
        assert!(result.nodes.is_empty());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E3002");
        assert!(matches!(diagnostics[0].severity, Severity::Error));
        assert!(diagnostics[0].message.contains("Invalid variable name"));
    }

    #[test]
    fn var_empty_name_reports_e3002() {
        // .var {""} {hello} should report E3002
        let call = IrNode::FunctionCall {
            name: "var".to_string(),
            positional_args: vec![
                IrValue::String("".to_string()),
                IrValue::String("hello".to_string()),
            ],
            named_args: Vec::new(),
            body: None,
            span: span(0, 17),
        };
        let (result, diagnostics) = Evaluator::new().evaluate(&doc(vec![call]));
        assert!(result.nodes.is_empty());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E3002");
        assert!(diagnostics[0].message.contains("Invalid variable name"));
    }
}
