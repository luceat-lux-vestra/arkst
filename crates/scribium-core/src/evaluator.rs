//! M1/M2 evaluator: resolves Quarkdown conditionals, variables, user-defined
//! functions, and the first value-flow builtins used by `::` call chains.
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
//! User-defined functions are registered in source order. A call evaluates
//! positional and named arguments first, creates a child scope, binds its
//! parameters, and then evaluates the body statement-by-statement in value
//! context. Outputless statements update the child scope, one substantive
//! semantic value remains an `IrValue` at that boundary, and multiple
//! structured outputs become `IrValue::Content` only when composition requires
//! it.
//!
//! Chain evaluation is structural: the head is invoked first, its semantic
//! `IrValue` becomes the first positional argument of the next segment, and
//! segments continue in source order. No source or backend text is generated
//! during this process.

use crate::builtins;
use crate::diagnostics::{Diagnostic, Severity};
use crate::ir::{IrCallSegment, IrDocument, IrInline, IrNamedArg, IrNode, IrParameter, IrValue};
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
    /// Returns the boolean value if this variable can be interpreted as a boolean.
    fn as_boolean(&self) -> Option<bool> {
        match self {
            VariableValue::Scalar(value) => scalar_boolean_value(value),
            VariableValue::Content(_) => None,
        }
    }
}

/// A source-backed callable definition stored in an evaluator scope.
#[derive(Debug, Clone, PartialEq)]
struct FunctionBinding {
    parameters: Vec<IrParameter>,
    body: Vec<IrNode>,
    declaration_span: SourceSpan,
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
    Failed,
    Unresolved,
}

/// Accumulates the observable result of a callable body without converting a
/// semantic value to document content until a second observable output makes
/// that conversion necessary.
enum CallableBodyValueAccumulator {
    Empty,
    Semantic { value: IrValue, span: SourceSpan },
    Content(Vec<IrNode>),
}

impl CallableBodyValueAccumulator {
    fn append_value(&mut self, value: IrValue, span: SourceSpan) {
        if matches!(self, Self::Empty) {
            *self = Self::Semantic { value, span };
            return;
        }

        let current = std::mem::replace(self, Self::Empty);
        let mut nodes = current.into_content_nodes();
        nodes.extend(value_into_content_nodes(value, span));
        *self = Self::Content(nodes);
    }

    fn finish(self) -> CallOutcome {
        match self {
            Self::Empty => CallOutcome::NoValue,
            Self::Semantic { value, .. } => CallOutcome::Value(value),
            Self::Content(nodes) => CallOutcome::Value(IrValue::Content(nodes)),
        }
    }

    fn into_content_nodes(self) -> Vec<IrNode> {
        match self {
            Self::Empty => Vec::new(),
            Self::Semantic { value, span } => value_into_content_nodes(value, span),
            Self::Content(nodes) => nodes,
        }
    }
}

fn value_into_content_nodes(value: IrValue, span: SourceSpan) -> Vec<IrNode> {
    match value {
        IrValue::Content(nodes) => nodes,
        scalar => vec![IrNode::Paragraph {
            content: vec![IrInline::Text {
                content: scalar_to_text(&scalar),
                span,
            }],
            span,
        }],
    }
}

fn ir_node_source_span(node: &IrNode) -> SourceSpan {
    match node {
        IrNode::Heading { span, .. }
        | IrNode::Paragraph { span, .. }
        | IrNode::Blockquote { span, .. }
        | IrNode::UnorderedList { span, .. }
        | IrNode::OrderedList { span, .. }
        | IrNode::Table { span, .. }
        | IrNode::CodeBlock { span, .. }
        | IrNode::RawTypst { span, .. }
        | IrNode::FunctionCall { span, .. }
        | IrNode::ChainedFunctionCall { span, .. }
        | IrNode::FunctionDeclaration { span, .. }
        | IrNode::ThematicBreak { span }
        | IrNode::Math { span, .. } => *span,
    }
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

    /// Declares or reassigns a variable from an evaluated IrValue, preserving content semantics.
    fn set_value(&mut self, name: String, value: IrValue) {
        self.variables
            .insert(name, VariableValue::from_evaluated_value(value));
    }

    /// Installs a user-function binding in the current local scope.
    fn set_function_binding(
        &mut self,
        name: String,
        parameters: Vec<IrParameter>,
        body: Vec<IrNode>,
        declaration_span: SourceSpan,
    ) {
        self.functions.insert(
            name,
            FunctionBinding {
                parameters,
                body,
                declaration_span,
            },
        );
    }

    #[cfg(test)]
    fn set_function(&mut self, name: String, parameters: Vec<String>) {
        let parameters = parameters
            .into_iter()
            .map(|name| IrParameter {
                name,
                name_span: SourceSpan::new(crate::source::SourceId(0), 0, 0),
                span: SourceSpan::new(crate::source::SourceId(0), 0, 0),
                optional: false,
            })
            .collect();
        self.set_function_binding(
            name,
            parameters,
            Vec::new(),
            SourceSpan::new(crate::source::SourceId(0), 0, 0),
        );
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

/// Evaluates Quarkdown conditionals, variables, user-defined functions, and
/// the currently supported semantic chain builtins in the IR.
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
            IrNode::FunctionDeclaration {
                name,
                parameters,
                body,
                span,
            } => {
                self.handle_function_declaration(
                    name,
                    parameters,
                    body,
                    span,
                    diagnostics,
                    context,
                );
                Vec::new()
            }
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
        named_args: &[IrNamedArg],
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
            CallOutcome::Failed => Vec::new(),
            CallOutcome::Unresolved => self
                .preserve_block_call(
                    name,
                    positional_args,
                    named_args,
                    body,
                    span,
                    diagnostics,
                    context,
                )
                .unwrap_or_default(),
        }
    }

    /// Evaluates an ordinary inline call in output context.
    #[allow(clippy::too_many_arguments)]
    fn evaluate_inline_call(
        &self,
        name: &str,
        positional_args: &[IrValue],
        named_args: &[IrNamedArg],
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
            CallOutcome::Value(value) => {
                self.materialize_inline_value(Some(value), span, diagnostics)
            }
            CallOutcome::NoValue => Vec::new(),
            CallOutcome::Failed => Vec::new(),
            CallOutcome::Unresolved => self
                .preserve_inline_call(
                    name,
                    positional_args,
                    named_args,
                    body,
                    span,
                    diagnostics,
                    context,
                )
                .unwrap_or_default(),
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
        match self.evaluate_chain_value(
            head,
            chain,
            body.as_deref().map(CallBody::Block),
            diagnostics,
            context,
        ) {
            CallOutcome::Value(value) => self.materialize_block_value(Some(value), span),
            CallOutcome::NoValue | CallOutcome::Failed | CallOutcome::Unresolved => Vec::new(),
        }
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
        match self.evaluate_chain_value(
            head,
            chain,
            body.as_deref().map(CallBody::Inline),
            diagnostics,
            context,
        ) {
            CallOutcome::Value(value) => {
                self.materialize_inline_value(Some(value), span, diagnostics)
            }
            CallOutcome::NoValue | CallOutcome::Failed | CallOutcome::Unresolved => Vec::new(),
        }
    }

    /// Invokes a call in value context. Ordinary nested calls and chain
    /// segments use this exact contract; only their surrounding syntax differs.
    /// Bodies remain unevaluated until the callee selects an evaluation policy.
    #[allow(clippy::too_many_arguments)]
    fn evaluate_call_value(
        &self,
        name: &str,
        positional_args: &[IrValue],
        named_args: &[IrNamedArg],
        body: Option<CallBody<'_>>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> CallOutcome {
        if is_conditional(name) {
            let condition = match self.resolve_call_condition(
                name,
                positional_args,
                named_args,
                span,
                diagnostics,
                context,
            ) {
                Ok(condition) => condition,
                Err(outcome) => return outcome,
            };
            return if take_branch(name, condition) {
                self.conditional_content_value(
                    positional_args,
                    named_args,
                    body,
                    span,
                    diagnostics,
                    context,
                )
            } else {
                CallOutcome::Value(IrValue::Content(Vec::new()))
            };
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
                span,
                diagnostics,
                context,
            );
        }

        // A source-defined binding takes precedence over an evidenced native
        // builtin after its declaration has executed. The same value-context
        // dispatch is used by ordinary calls, nested arguments, and chains.
        if let Some(binding) = context.get_function(name).cloned() {
            return self.evaluate_user_function(
                &binding,
                positional_args,
                named_args,
                body,
                span,
                diagnostics,
                context,
            );
        }

        if builtins::is_supported(name) {
            let evaluated_positional =
                match self.evaluate_values(positional_args, span, diagnostics, context) {
                    Ok(values) => values,
                    Err(outcome) => return outcome,
                };
            let evaluated_named = match self.evaluate_named(named_args, span, diagnostics, context)
            {
                Ok(values) => values,
                Err(outcome) => return outcome,
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
                    CallOutcome::Failed
                }
            };
        }

        // Ordinary output context preserves unresolved calls. A chain wrapper
        // converts this outcome into an explicit source-backed E3001 instead.
        CallOutcome::Unresolved
    }

    #[allow(clippy::too_many_arguments)]
    fn evaluate_user_function(
        &self,
        binding: &FunctionBinding,
        positional_args: &[IrValue],
        named_args: &[IrNamedArg],
        body: Option<CallBody<'_>>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> CallOutcome {
        // Caller arguments are evaluated before any callee scope is created.
        // The parser guarantees that positional arguments precede named ones,
        // so these two passes preserve source order for the supported grammar.
        let positional = match self.evaluate_values(positional_args, span, diagnostics, context) {
            Ok(values) => values,
            Err(outcome) => return outcome,
        };
        let named = match self.evaluate_named(named_args, span, diagnostics, context) {
            Ok(values) => values,
            Err(outcome) => return outcome,
        };

        let mut bound: Vec<Option<IrValue>> = vec![None; binding.parameters.len()];
        for (index, value) in positional.into_iter().enumerate() {
            let Some(slot) = bound.get_mut(index) else {
                diagnostics.push(function_error(
                    format!(
                        "Function call has too many positional arguments (received at least {})",
                        index + 1
                    ),
                    *span,
                ));
                return CallOutcome::Failed;
            };
            *slot = Some(value);
        }

        for argument in &named {
            let Some(index) = binding
                .parameters
                .iter()
                .position(|parameter| parameter.name == argument.name)
            else {
                diagnostics.push(function_error_at(
                    format!("Unknown named parameter `{}`", argument.name),
                    argument.name_span,
                ));
                return CallOutcome::Failed;
            };
            if bound[index].is_some() {
                diagnostics.push(function_error_at(
                    format!("Parameter `{}` was bound more than once", argument.name),
                    argument.name_span,
                ));
                return CallOutcome::Failed;
            }
            bound[index] = Some(argument.value.clone());
        }

        let body_value = if let Some(body) = body {
            let Some(last) = bound.last() else {
                diagnostics.push(function_error(
                    "A block argument requires a final function parameter".to_string(),
                    *span,
                ));
                return CallOutcome::Failed;
            };
            if last.is_some() {
                diagnostics.push(function_error(
                    "A block argument collides with the function's final parameter binding"
                        .to_string(),
                    *span,
                ));
                return CallOutcome::Failed;
            }
            match self.evaluate_call_body(body, span, diagnostics, context) {
                CallOutcome::Value(value) => Some(value),
                CallOutcome::NoValue => return CallOutcome::NoValue,
                CallOutcome::Failed => return CallOutcome::Failed,
                CallOutcome::Unresolved => return CallOutcome::Unresolved,
            }
        } else {
            None
        };

        if let Some(value) = body_value {
            if let Some(last) = bound.last_mut() {
                *last = Some(value);
            }
        }

        for (index, parameter) in binding.parameters.iter().enumerate() {
            if bound[index].is_none() {
                if parameter.optional {
                    bound[index] = Some(IrValue::None);
                } else {
                    diagnostics.push(function_error_at(
                        format!("Missing required argument `{}`", parameter.name),
                        parameter.name_span,
                    ));
                    return CallOutcome::Failed;
                }
            }
        }

        let mut child = context.child();
        for (parameter, value) in binding.parameters.iter().zip(bound) {
            if let Some(value) = value {
                child.set_value(parameter.name.clone(), value);
            }
        }
        self.evaluate_callable_body_value(&binding.body, diagnostics, &mut child)
    }

    fn evaluate_callable_body_value(
        &self,
        nodes: &[IrNode],
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> CallOutcome {
        let mut result = CallableBodyValueAccumulator::Empty;
        for node in nodes {
            let span = ir_node_source_span(node);
            match self.evaluate_callable_statement_value(node, diagnostics, context) {
                CallOutcome::Value(value) => result.append_value(value, span),
                CallOutcome::NoValue => {}
                CallOutcome::Failed => return CallOutcome::Failed,
                CallOutcome::Unresolved => return CallOutcome::Unresolved,
            }
        }
        result.finish()
    }

    /// Evaluates one callable-body statement in semantic value context.
    ///
    /// Function calls and chains use the same shared dispatch as every other
    /// call site. Markdown nodes are retained as structured content, while
    /// declarations and outputless calls contribute state without becoming a
    /// fabricated empty value.
    fn evaluate_callable_statement_value(
        &self,
        node: &IrNode,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> CallOutcome {
        match node {
            IrNode::FunctionCall {
                name,
                positional_args,
                named_args,
                body,
                span,
            } => match self.evaluate_call_value(
                name,
                positional_args,
                named_args,
                body.as_deref().map(CallBody::Block),
                span,
                diagnostics,
                context,
            ) {
                CallOutcome::Unresolved => self
                    .preserve_block_call(
                        name,
                        positional_args,
                        named_args,
                        body.as_deref(),
                        span,
                        diagnostics,
                        context,
                    )
                    .map(IrValue::Content)
                    .map_or(CallOutcome::Failed, CallOutcome::Value),
                outcome => outcome,
            },
            IrNode::ChainedFunctionCall {
                head, chain, body, ..
            } => self.evaluate_chain_value(
                head,
                chain,
                body.as_deref().map(CallBody::Block),
                diagnostics,
                context,
            ),
            _ => {
                let before = diagnostics.len();
                let evaluated = self.evaluate_node(node, diagnostics, context);
                if diagnostics.len() != before {
                    CallOutcome::Failed
                } else if evaluated.is_empty() {
                    CallOutcome::NoValue
                } else {
                    CallOutcome::Value(IrValue::Content(evaluated))
                }
            }
        }
    }

    /// Evaluates a chain strictly left-to-right using semantic `IrValue`s.
    fn evaluate_chain_value(
        &self,
        head: &IrCallSegment,
        chain: &[IrCallSegment],
        body: Option<CallBody<'_>>,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> CallOutcome {
        let mut value = match self.chain_outcome(
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
            !chain.is_empty(),
            diagnostics,
            context,
        ) {
            CallOutcome::Value(value) => value,
            outcome => return outcome,
        };

        for (index, source_segment) in chain.iter().enumerate() {
            let mut positional_args = Vec::with_capacity(1 + source_segment.positional_args.len());
            // The previous result is always first. Explicit positional
            // arguments follow it in their original order; named arguments
            // remain in the named collection untouched.
            positional_args.push(value);
            positional_args.extend(source_segment.positional_args.iter().cloned());
            let final_body = (index + 1 == chain.len()).then_some(body).flatten();
            let outcome = self.chain_outcome(
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
                index + 1 < chain.len(),
                diagnostics,
                context,
            );
            match outcome {
                CallOutcome::Value(next_value) => value = next_value,
                outcome => return outcome,
            }
        }

        CallOutcome::Value(value)
    }

    fn chain_outcome(
        &self,
        outcome: CallOutcome,
        segment: &IrCallSegment,
        value_required: bool,
        diagnostics: &mut Vec<Diagnostic>,
        context: &EvaluationContext,
    ) -> CallOutcome {
        match outcome {
            CallOutcome::Value(value) => CallOutcome::Value(value),
            CallOutcome::NoValue if value_required => {
                diagnostics.push(chain_evaluation_error(
                    format!(
                        "Chained call segment `.{}` produced no value required by a later segment",
                        segment.name
                    ),
                    segment.name_span,
                ));
                CallOutcome::Failed
            }
            CallOutcome::NoValue => CallOutcome::NoValue,
            CallOutcome::Failed => CallOutcome::Failed,
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
                CallOutcome::Failed
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
    ) -> CallOutcome {
        let before = diagnostics.len();
        let value = match body {
            CallBody::Block(nodes) => {
                IrValue::Content(self.evaluate_nodes(nodes, diagnostics, context))
            }
            CallBody::Inline(inlines) => IrValue::Content(vec![IrNode::Paragraph {
                content: self.evaluate_inlines(inlines, diagnostics, context),
                span: *span,
            }]),
        };
        if diagnostics.len() == before {
            CallOutcome::Value(value)
        } else {
            CallOutcome::Failed
        }
    }

    /// Resolves only a conditional's condition. Body and content arguments
    /// remain lazy until the branch is known.
    fn resolve_call_condition(
        &self,
        name: &str,
        positional_args: &[IrValue],
        named_args: &[IrNamedArg],
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> Result<bool, CallOutcome> {
        let raw_condition = named_args
            .iter()
            .find(|arg| arg.name == "condition")
            .map(|arg| &arg.value)
            .or_else(|| positional_args.first());
        let Some(raw_condition) = raw_condition else {
            diagnostics.push(unresolvable_condition(name, span));
            return Err(CallOutcome::Failed);
        };
        let condition = match self.evaluate_value(raw_condition, diagnostics, context) {
            CallOutcome::Value(condition) => condition,
            CallOutcome::NoValue => {
                diagnostics.push(no_value_required(value_source_span(raw_condition, span)));
                return Err(CallOutcome::Failed);
            }
            CallOutcome::Failed => return Err(CallOutcome::Failed),
            CallOutcome::Unresolved => {
                self.preserve_value_expression(raw_condition, diagnostics, context)?
            }
        };
        match resolve_boolean_value(&condition, context) {
            Some(value) => Ok(value),
            None => {
                diagnostics.push(unresolvable_condition(name, span));
                Err(CallOutcome::Failed)
            }
        }
    }

    /// Produces conditional content after the condition has selected the
    /// branch. The body and body-like arguments are evaluated here, not before
    /// dispatch.
    fn conditional_content_value(
        &self,
        positional_args: &[IrValue],
        named_args: &[IrNamedArg],
        body: Option<CallBody<'_>>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> CallOutcome {
        if let Some(body) = body {
            return self.evaluate_call_body(body, span, diagnostics, context);
        }
        if let Some(arg) = named_args.iter().find(|arg| arg.name == "body") {
            let value = &arg.value;
            return self.evaluate_content_argument(value, span, diagnostics, context);
        }
        if let Some(value) = positional_args.get(1) {
            return self.evaluate_content_argument(value, span, diagnostics, context);
        }
        CallOutcome::Value(IrValue::Content(Vec::new()))
    }

    fn evaluate_content_argument(
        &self,
        value: &IrValue,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> CallOutcome {
        match self.evaluate_value(value, diagnostics, context) {
            CallOutcome::Value(value) => CallOutcome::Value(self.scalar_or_content(value, span)),
            CallOutcome::NoValue => {
                diagnostics.push(no_value_required(value_source_span(value, span)));
                CallOutcome::Failed
            }
            CallOutcome::Failed => CallOutcome::Failed,
            CallOutcome::Unresolved => {
                match self.preserve_value_expression(value, diagnostics, context) {
                    Ok(value) => CallOutcome::Value(self.scalar_or_content(value, span)),
                    Err(outcome) => outcome,
                }
            }
        }
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

    /// Retains an unresolved value expression without turning it into an
    /// empty successful value. Its nested arguments still run through the
    /// value-required preservation path, so failures cannot be erased.
    fn preserve_value_expression(
        &self,
        value: &IrValue,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> Result<IrValue, CallOutcome> {
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
                    return self
                        .preserve_block_call(
                            name,
                            positional_args,
                            named_args,
                            body.as_deref(),
                            span,
                            diagnostics,
                            context,
                        )
                        .map(IrValue::Content);
                }
                let before = diagnostics.len();
                let nodes = self.evaluate_nodes(nodes, diagnostics, context);
                if diagnostics.len() == before {
                    Ok(IrValue::Content(nodes))
                } else {
                    Err(CallOutcome::Failed)
                }
            }
            scalar => Ok(scalar.clone()),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn preserve_block_call(
        &self,
        name: &str,
        positional_args: &[IrValue],
        named_args: &[IrNamedArg],
        body: Option<&[IrNode]>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> Result<Vec<IrNode>, CallOutcome> {
        let positional_args =
            self.evaluate_values_for_preservation(positional_args, span, diagnostics, context)?;
        let named_args =
            self.evaluate_named_for_preservation(named_args, span, diagnostics, context)?;
        let body = if let Some(nodes) = body {
            let before = diagnostics.len();
            let body = self.evaluate_nodes(nodes, diagnostics, context);
            if diagnostics.len() != before {
                return Err(CallOutcome::Failed);
            }
            Some(body)
        } else {
            None
        };
        Ok(vec![IrNode::FunctionCall {
            name: name.to_string(),
            positional_args,
            named_args,
            body,
            span: *span,
        }])
    }

    #[allow(clippy::too_many_arguments)]
    fn preserve_inline_call(
        &self,
        name: &str,
        positional_args: &[IrValue],
        named_args: &[IrNamedArg],
        body: Option<&[IrInline]>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> Result<Vec<IrInline>, CallOutcome> {
        let positional_args =
            self.evaluate_values_for_preservation(positional_args, span, diagnostics, context)?;
        let named_args =
            self.evaluate_named_for_preservation(named_args, span, diagnostics, context)?;
        let body = if let Some(inlines) = body {
            let before = diagnostics.len();
            let body = self.evaluate_inlines(inlines, diagnostics, context);
            if diagnostics.len() != before {
                return Err(CallOutcome::Failed);
            }
            Some(body)
        } else {
            None
        };
        Ok(vec![IrInline::DirectiveCall {
            name: name.to_string(),
            positional_args,
            named_args,
            body,
            span: *span,
        }])
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

    fn materialize_inline_value(
        &self,
        value: Option<IrValue>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Vec<IrInline> {
        match value {
            Some(IrValue::Content(nodes)) => {
                self.materialize_inline_content(nodes, span, diagnostics)
            }
            Some(value) => vec![IrInline::Text {
                content: scalar_to_text(&value),
                span: *span,
            }],
            None => Vec::new(),
        }
    }

    /// Materializes only content that has an unambiguous inline shape.
    ///
    /// A paragraph boundary or any other block node must remain observable;
    /// silently concatenating or dropping it would change the document.
    fn materialize_inline_content(
        &self,
        nodes: Vec<IrNode>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Vec<IrInline> {
        let mut nodes = nodes.into_iter();
        let Some(first) = nodes.next() else {
            return Vec::new();
        };
        if nodes.next().is_some() {
            diagnostics.push(function_error(
                "Rich block content cannot be inserted into an inline context unless it is exactly one paragraph".to_string(),
                *span,
            ));
            return Vec::new();
        }
        match first {
            IrNode::Paragraph { content, .. } => content,
            _ => {
                diagnostics.push(function_error(
                    "Rich block content cannot be inserted into an inline context unless it is exactly one paragraph".to_string(),
                    *span,
                ));
                Vec::new()
            }
        }
    }

    /// Evaluates a value without entering document-output context.
    fn evaluate_value(
        &self,
        value: &IrValue,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> CallOutcome {
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
                    return self.evaluate_call_value(
                        name,
                        positional_args,
                        named_args,
                        body.as_deref().map(CallBody::Block),
                        span,
                        diagnostics,
                        context,
                    );
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
                let before = diagnostics.len();
                let contains_declaration = nodes
                    .iter()
                    .any(|node| matches!(node, IrNode::FunctionDeclaration { .. }));
                let nodes = self.evaluate_nodes(nodes, diagnostics, context);
                if diagnostics.len() == before {
                    if nodes.is_empty() && contains_declaration {
                        CallOutcome::NoValue
                    } else {
                        CallOutcome::Value(IrValue::Content(nodes))
                    }
                } else {
                    CallOutcome::Failed
                }
            }
            scalar => CallOutcome::Value(scalar.clone()),
        }
    }

    fn evaluate_values(
        &self,
        values: &[IrValue],
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> Result<Vec<IrValue>, CallOutcome> {
        self.evaluate_values_for_preservation(values, span, diagnostics, context)
    }

    fn evaluate_named(
        &self,
        named: &[IrNamedArg],
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> Result<Vec<IrNamedArg>, CallOutcome> {
        self.evaluate_named_for_preservation(named, span, diagnostics, context)
    }

    fn evaluate_values_for_preservation(
        &self,
        values: &[IrValue],
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> Result<Vec<IrValue>, CallOutcome> {
        let mut evaluated = Vec::with_capacity(values.len());
        for value in values {
            match self.evaluate_value(value, diagnostics, context) {
                CallOutcome::Value(value) => evaluated.push(value),
                CallOutcome::Unresolved => {
                    evaluated.push(self.preserve_value_expression(value, diagnostics, context)?)
                }
                CallOutcome::NoValue => {
                    diagnostics.push(no_value_required(value_source_span(value, span)));
                    return Err(CallOutcome::Failed);
                }
                CallOutcome::Failed => return Err(CallOutcome::Failed),
            }
        }
        Ok(evaluated)
    }

    fn evaluate_named_for_preservation(
        &self,
        named: &[IrNamedArg],
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> Result<Vec<IrNamedArg>, CallOutcome> {
        let mut evaluated = Vec::with_capacity(named.len());
        for arg in named {
            let value = match self.evaluate_value(&arg.value, diagnostics, context) {
                CallOutcome::Value(value) => value,
                CallOutcome::Unresolved => {
                    self.preserve_value_expression(&arg.value, diagnostics, context)?
                }
                CallOutcome::NoValue => {
                    diagnostics.push(no_value_required(value_source_span(&arg.value, span)));
                    return Err(CallOutcome::Failed);
                }
                CallOutcome::Failed => return Err(CallOutcome::Failed),
            };
            evaluated.push(IrNamedArg {
                name: arg.name.clone(),
                name_span: arg.name_span,
                value,
                span: arg.span,
            });
        }
        Ok(evaluated)
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

fn function_error(message: String, span: SourceSpan) -> Diagnostic {
    function_error_at(message, span)
}

fn function_error_at(message: String, span: SourceSpan) -> Diagnostic {
    Diagnostic {
        code: "E3003".to_string(),
        severity: Severity::Error,
        message,
        primary: Some(span),
        secondary: Vec::new(),
        hints: vec![
            "Function declarations and calls must satisfy the supported required-parameter contract."
                .to_string(),
        ],
    }
}

fn value_source_span(value: &IrValue, fallback: &SourceSpan) -> SourceSpan {
    match value {
        IrValue::Content(nodes) => match nodes.as_slice() {
            [IrNode::FunctionCall { span, .. }] | [IrNode::ChainedFunctionCall { span, .. }] => {
                *span
            }
            [IrNode::FunctionDeclaration { span, .. }] => *span,
            _ => *fallback,
        },
        _ => *fallback,
    }
}

fn no_value_required(span: SourceSpan) -> Diagnostic {
    chain_evaluation_error(
        "Call produced no value where a value is required for semantic composition".to_string(),
        span,
    )
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
        IrValue::None => None,
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
        IrValue::None => None,
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
    named_args: &[IrNamedArg],
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
    named_args: &[IrNamedArg],
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
        IrValue::None => "None".to_string(),
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
    fn handle_function_declaration(
        &self,
        name: &IrValue,
        parameters: &[IrParameter],
        body: &[IrNode],
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) {
        let function_name = match name {
            IrValue::Identifier(name) | IrValue::String(name) => name,
            _ => {
                diagnostics.push(function_error(
                    "Function name must be a normal identifier".to_string(),
                    *span,
                ));
                return;
            }
        };
        if !is_valid_normal_call_name(function_name) {
            diagnostics.push(function_error(
                format!("Invalid function name `{function_name}`"),
                *span,
            ));
            return;
        }
        if body.is_empty() {
            diagnostics.push(function_error(
                "Function declaration requires a non-empty body".to_string(),
                *span,
            ));
            return;
        }
        let mut seen = BTreeMap::new();
        for parameter in parameters {
            if seen
                .insert(parameter.name.clone(), parameter.span)
                .is_some()
            {
                diagnostics.push(function_error_at(
                    format!("Duplicate function parameter `{}`", parameter.name),
                    parameter.span,
                ));
                return;
            }
        }
        context.set_function_binding(
            function_name.clone(),
            parameters.to_vec(),
            body.to_vec(),
            *span,
        );
    }

    // Variable handling methods

    /// Handles a `.var` declaration in value context.
    #[allow(clippy::too_many_arguments)]
    fn handle_var_declaration(
        &self,
        positional_args: &[IrValue],
        named_args: &[IrNamedArg],
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
                return CallOutcome::Failed;
            }
        };
        // Validate variable name
        if !is_valid_normal_call_name(&var_name) {
            diagnostics.push(invalid_var_name(&var_name, span));
            return CallOutcome::Failed;
        }

        // Determine the value: body > named "body" > second positional > named "value" > empty
        if let Some(body) = body {
            match self.evaluate_call_body(body, span, diagnostics, context) {
                CallOutcome::Value(value) => {
                    context.set_value(var_name, value);
                    return CallOutcome::NoValue;
                }
                CallOutcome::Failed => return CallOutcome::Failed,
                CallOutcome::NoValue | CallOutcome::Unresolved => {
                    return CallOutcome::Failed;
                }
            }
        }

        // Check named "body" argument
        if let Some(arg) = named_args.iter().find(|arg| arg.name == "body") {
            let value = &arg.value;
            match self.evaluate_value(value, diagnostics, context) {
                CallOutcome::Value(value) => {
                    context.set_value(var_name, value);
                    return CallOutcome::NoValue;
                }
                CallOutcome::Unresolved => {
                    match self.preserve_value_expression(value, diagnostics, context) {
                        Ok(value) => {
                            context.set_value(var_name, value);
                            return CallOutcome::NoValue;
                        }
                        Err(outcome) => return outcome,
                    }
                }
                CallOutcome::NoValue => {
                    diagnostics.push(no_value_required(value_source_span(value, span)));
                    return CallOutcome::Failed;
                }
                CallOutcome::Failed => return CallOutcome::Failed,
            }
        }

        // Check named "value" argument
        if let Some(arg) = named_args.iter().find(|arg| arg.name == "value") {
            let value = &arg.value;
            match self.evaluate_value(value, diagnostics, context) {
                CallOutcome::Value(value) => {
                    context.set_value(var_name, value);
                    return CallOutcome::NoValue;
                }
                CallOutcome::Unresolved => {
                    match self.preserve_value_expression(value, diagnostics, context) {
                        Ok(value) => {
                            context.set_value(var_name, value);
                            return CallOutcome::NoValue;
                        }
                        Err(outcome) => return outcome,
                    }
                }
                CallOutcome::NoValue => {
                    diagnostics.push(no_value_required(value_source_span(value, span)));
                    return CallOutcome::Failed;
                }
                CallOutcome::Failed => return CallOutcome::Failed,
            }
        }

        // Fall back to second positional argument
        if let Some(value) = positional_args.get(1) {
            match self.evaluate_value(value, diagnostics, context) {
                CallOutcome::Value(value) => {
                    context.set_value(var_name, value);
                    return CallOutcome::NoValue;
                }
                CallOutcome::Unresolved => {
                    match self.preserve_value_expression(value, diagnostics, context) {
                        Ok(value) => {
                            context.set_value(var_name, value);
                            return CallOutcome::NoValue;
                        }
                        Err(outcome) => return outcome,
                    }
                }
                CallOutcome::NoValue => {
                    diagnostics.push(no_value_required(value_source_span(value, span)));
                    return CallOutcome::Failed;
                }
                CallOutcome::Failed => return CallOutcome::Failed,
            }
        }

        // No value provided - invalid declaration
        diagnostics.push(invalid_var_declaration(span));
        CallOutcome::Failed
    }

    /// Handles a variable reassignment in value context.
    fn handle_variable_reassignment_value(
        &self,
        name: &str,
        positional_args: &[IrValue],
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> CallOutcome {
        let value = &positional_args[0];
        match self.evaluate_value(value, diagnostics, context) {
            CallOutcome::Value(value) => {
                context.set_value(name.to_string(), value);
                CallOutcome::NoValue
            }
            CallOutcome::Unresolved => {
                match self.preserve_value_expression(value, diagnostics, context) {
                    Ok(value) => {
                        context.set_value(name.to_string(), value);
                        CallOutcome::NoValue
                    }
                    Err(outcome) => outcome,
                }
            }
            CallOutcome::NoValue => {
                diagnostics.push(no_value_required(value_source_span(value, span)));
                CallOutcome::Failed
            }
            CallOutcome::Failed => CallOutcome::Failed,
        }
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

    fn named_arg(name: &str, value: IrValue) -> IrNamedArg {
        IrNamedArg {
            name: name.to_string(),
            name_span: span(0, name.len()),
            value,
            span: span(0, name.len()),
        }
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
        segment
            .named_args
            .push(named_arg("by", IrValue::Number(2.0)));
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
    fn final_chain_reassignment_is_a_legal_no_value_result() {
        let (nodes, diagnostics) = evaluate_with_diagnostics(vec![
            var_declaration("x", IrValue::Number(0.0)),
            chain_node(
                chain_segment(
                    "sum",
                    0,
                    12,
                    vec![IrValue::Number(1.0), IrValue::Number(2.0)],
                ),
                vec![chain_segment("x", 14, 15, Vec::new())],
            ),
            var_ref("x"),
        ]);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_paragraph_text(&nodes, "3");
    }

    #[test]
    fn non_final_chain_reassignment_reports_no_value_and_stops() {
        let (nodes, diagnostics) = evaluate_with_diagnostics(vec![
            var_declaration("x", IrValue::Number(0.0)),
            chain_node(
                chain_segment(
                    "sum",
                    0,
                    12,
                    vec![IrValue::Number(1.0), IrValue::Number(2.0)],
                ),
                vec![
                    chain_segment("x", 14, 15, Vec::new()),
                    chain_segment("sum", 17, 25, vec![IrValue::Number(1.0)]),
                ],
            ),
            var_ref("x"),
        ]);
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E3001");
        assert_eq!(diagnostics[0].primary, Some(span(14, 15)));
        assert_paragraph_text(&nodes, "3");
    }

    #[test]
    fn nested_no_value_argument_reports_e3001_without_invoking_outer_call() {
        let nested_reassignment = IrValue::Content(vec![IrNode::FunctionCall {
            name: "x".to_string(),
            positional_args: vec![IrValue::Number(3.0)],
            named_args: Vec::new(),
            body: None,
            span: span(7, 12),
        }]);
        let outer = IrNode::FunctionCall {
            name: "multiply".to_string(),
            positional_args: vec![nested_reassignment, IrValue::Number(2.0)],
            named_args: Vec::new(),
            body: None,
            span: span(0, 20),
        };
        let (nodes, diagnostics) = evaluate_with_diagnostics(vec![
            var_declaration("x", IrValue::Number(0.0)),
            outer,
            var_ref("x"),
        ]);
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E3001");
        assert_eq!(diagnostics[0].primary, Some(span(7, 12)));
        assert_paragraph_text(&nodes, "3");
    }

    #[test]
    fn nested_no_value_named_argument_reports_e3001_without_invoking_outer_call() {
        let nested_reassignment = IrValue::Content(vec![IrNode::FunctionCall {
            name: "x".to_string(),
            positional_args: vec![IrValue::Number(3.0)],
            named_args: Vec::new(),
            body: None,
            span: span(9, 14),
        }]);
        let outer = IrNode::FunctionCall {
            name: "multiply".to_string(),
            positional_args: vec![IrValue::Number(2.0)],
            named_args: vec![named_arg("by", nested_reassignment)],
            body: None,
            span: span(0, 22),
        };
        let (nodes, diagnostics) = evaluate_with_diagnostics(vec![
            var_declaration("x", IrValue::Number(0.0)),
            outer,
            var_ref("x"),
        ]);
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E3001");
        assert_eq!(diagnostics[0].primary, Some(span(9, 14)));
        assert_paragraph_text(&nodes, "3");
    }

    #[test]
    fn nested_function_declaration_reports_no_value_once_at_its_span() {
        let declaration_span = span(10, 24);
        let declaration = IrValue::Content(vec![IrNode::FunctionDeclaration {
            name: IrValue::Identifier("declared".to_string()),
            parameters: Vec::new(),
            body: vec![text_paragraph("body")],
            span: declaration_span,
        }]);
        let outer = IrNode::FunctionCall {
            name: "sum".to_string(),
            positional_args: vec![declaration, IrValue::Number(1.0)],
            named_args: Vec::new(),
            body: None,
            span: span(0, 30),
        };

        let (nodes, diagnostics) = evaluate_with_diagnostics(vec![outer]);
        assert!(nodes.is_empty());
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E3001");
        assert_eq!(diagnostics[0].primary, Some(declaration_span));
        assert!(diagnostics[0].message.contains("no value"));
    }

    #[test]
    fn failed_nested_call_propagates_without_a_duplicate_no_value_error() {
        let invalid_sum = call_value("sum", vec![IrValue::Boolean(true)]);
        let outer = IrNode::FunctionCall {
            name: "multiply".to_string(),
            positional_args: vec![invalid_sum, IrValue::Number(2.0)],
            named_args: Vec::new(),
            body: None,
            span: span(0, 20),
        };
        let (nodes, diagnostics) = evaluate_with_diagnostics(vec![outer]);
        assert!(nodes.is_empty());
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E3001");
        assert!(diagnostics[0]
            .message
            .contains("requires numeric arguments"));
    }

    #[test]
    fn malformed_nested_var_propagates_its_original_diagnostic_only() {
        let invalid_var = IrValue::Content(vec![IrNode::FunctionCall {
            name: "var".to_string(),
            positional_args: vec![
                IrValue::Identifier("bad name".to_string()),
                IrValue::Number(1.0),
            ],
            named_args: Vec::new(),
            body: None,
            span: span(7, 18),
        }]);
        let outer = IrNode::FunctionCall {
            name: "multiply".to_string(),
            positional_args: vec![invalid_var, IrValue::Number(2.0)],
            named_args: Vec::new(),
            body: None,
            span: span(0, 20),
        };
        let (nodes, diagnostics) = evaluate_with_diagnostics(vec![outer]);
        assert!(nodes.is_empty());
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E3002");
        assert_eq!(diagnostics[0].primary, Some(span(7, 18)));
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
            child.get_function("inherited").map(|binding| binding
                .parameters
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>()),
            Some(vec!["value"])
        );
        child.set_value("local".into(), IrValue::String("child".into()));
        child.set_function("future".into(), vec!["value".into()]);

        assert!(parent.get("local").is_none());
        assert_eq!(
            child.get("local").map(VariableValue::to_value),
            Some(IrValue::String("child".into()))
        );
        assert_eq!(
            child.get_function("future").map(|binding| binding
                .parameters
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>()),
            Some(vec!["value"])
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
            child.get_function("inherited").map(|binding| binding
                .parameters
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>()),
            Some(vec!["shadowed"])
        );
        assert_eq!(
            parent.get_function("inherited").map(|binding| binding
                .parameters
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>()),
            Some(vec!["value"])
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
            named_args: vec![named_arg("condition", IrValue::Boolean(true))],
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
            named_args: vec![named_arg("condition", IrValue::Boolean(false))],
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
            named_args: vec![named_arg("condition", IrValue::Boolean(false))],
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
                named_args: vec![named_arg(
                    "condition",
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
            named_args: vec![named_arg(
                "body",
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
            named_args: vec![named_arg(
                "body",
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
            named_args: vec![named_arg(
                "body",
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
                    named_args: vec![named_arg("condition", IrValue::Boolean(true))],
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
            named_args: vec![named_arg(
                "body",
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
            named_args: vec![named_arg("condition", IrValue::Number(3.0))],
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
