//! M1/M2 evaluator: resolves Quarkdown conditionals, variables, scoped `.let`
//! calls, user-defined functions, and the first value-flow builtins used by
//! `::` call chains.
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
use crate::ir::{
    IrCallSegment, IrDictionary, IrDocument, IrInline, IrListItem, IrNamedArg, IrNode, IrPair,
    IrParameter, IrRange, IrTableAlignment, IrTableCell, IrTableRow, IrValue,
};
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
    parameters: LambdaParameters,
    body: Vec<IrNode>,
    declaration_span: SourceSpan,
}

/// The parameter mode of a callable body.
///
/// Explicit parameters retain their source-backed names and optionality.
/// Headerless lambdas expose the invocation's positional values through the
/// invocation-local implicit scope. Keeping this distinction in the callable
/// representation lets both modes use the same argument evaluation and body
/// invocation path without aliasing `.1` onto an explicit parameter.
#[derive(Debug, Clone, PartialEq)]
enum LambdaParameters {
    Explicit(Vec<IrParameter>),
    Implicit,
}

impl LambdaParameters {
    #[cfg(test)]
    fn explicit(&self) -> Option<&[IrParameter]> {
        match self {
            Self::Explicit(parameters) => Some(parameters),
            Self::Implicit => None,
        }
    }

    fn description(&self) -> String {
        match self {
            Self::Explicit(parameters) => format!("{} explicit parameter(s)", parameters.len()),
            Self::Implicit => "implicit positional parameters".to_string(),
        }
    }
}

enum BoundLambdaArguments {
    Explicit(Vec<IrValue>),
    Implicit(Vec<IrValue>),
}

/// The implicit-parameter boundary installed for one callable invocation.
///
/// An explicit invocation deliberately masks any outer implicit scope. This
/// prevents `.1` in an explicit lambda from accidentally capturing an outer
/// lambda's argument.
#[derive(Debug, Clone, PartialEq)]
enum LambdaScope {
    Explicit,
    Implicit(Vec<IrValue>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImplicitParameterIndex {
    Valid(usize),
    Overflow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImplicitParameterError {
    Missing,
    Overflow,
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

#[derive(Clone, Copy)]
struct IterationOptions {
    span: SourceSpan,
    allow_destructuring: bool,
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
    fn append_value(
        &mut self,
        value: IrValue,
        span: SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Result<(), CallOutcome> {
        if matches!(self, Self::Empty) {
            *self = Self::Semantic { value, span };
            return Ok(());
        }

        let current = std::mem::replace(self, Self::Empty);
        let mut nodes = current.into_content_nodes(diagnostics)?;
        nodes.extend(value_into_content_nodes(value, span, diagnostics)?);
        *self = Self::Content(nodes);
        Ok(())
    }

    fn finish(self) -> CallOutcome {
        match self {
            Self::Empty => CallOutcome::NoValue,
            Self::Semantic { value, .. } => CallOutcome::Value(value),
            Self::Content(nodes) => CallOutcome::Value(IrValue::Content(nodes)),
        }
    }

    fn into_content_nodes(
        self,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Result<Vec<IrNode>, CallOutcome> {
        match self {
            Self::Empty => Ok(Vec::new()),
            Self::Semantic { value, span } => value_into_content_nodes(value, span, diagnostics),
            Self::Content(nodes) => Ok(nodes),
        }
    }
}

fn value_into_content_nodes(
    value: IrValue,
    span: SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<Vec<IrNode>, CallOutcome> {
    match value {
        IrValue::Content(nodes) => Ok(nodes),
        IrValue::Collection(values) => {
            let mut nodes = Vec::new();
            if let Err(error) = nodes.try_reserve(values.len()) {
                diagnostics.push(iteration_error(
                    format!("collection content cannot be allocated: {error}"),
                    span,
                ));
                return Err(CallOutcome::Failed);
            }
            for value in values {
                nodes.extend(value_into_content_nodes(value, span, diagnostics)?);
            }
            Ok(nodes)
        }
        IrValue::Pair(pair) => pair_into_content_nodes(pair, diagnostics),
        IrValue::Dictionary(dictionary) => {
            dictionary_into_table(dictionary, diagnostics).map(|table| vec![table])
        }
        IrValue::Range(range) => {
            diagnostics.push(iteration_error(
                "Direct Range materialization is deferred; consume the typed Range through iteration first"
                    .to_string(),
                range.span,
            ));
            Err(CallOutcome::Failed)
        }
        scalar => match scalar_to_text(&scalar, span, diagnostics) {
            Ok(content) => Ok(vec![IrNode::Paragraph {
                content: vec![IrInline::Text { content, span }],
                span,
            }]),
            Err(outcome) => Err(outcome),
        },
    }
}

fn pair_into_content_nodes(
    pair: IrPair,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<Vec<IrNode>, CallOutcome> {
    let mut items = Vec::new();
    if let Err(error) = items.try_reserve_exact(2) {
        diagnostics.push(iteration_error(
            format!("pair output collection cannot be allocated: {error}"),
            pair.span,
        ));
        return Err(CallOutcome::Failed);
    }
    for value in [*pair.first, *pair.second] {
        let nodes = value_into_content_nodes(value, pair.span, diagnostics)?;
        items.push(IrListItem {
            nodes,
            task: None,
            span: pair.span,
        });
    }
    Ok(vec![IrNode::OrderedList {
        items,
        start: 1,
        span: pair.span,
    }])
}

fn dictionary_into_table(
    dictionary: IrDictionary,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<IrNode, CallOutcome> {
    let span = dictionary.span;
    let header = IrTableRow {
        cells: vec![table_text_cell("Key", span), table_text_cell("Value", span)],
        span,
    };
    let mut rows = Vec::new();
    if let Err(error) = rows.try_reserve_exact(dictionary.entries.len()) {
        diagnostics.push(iteration_error(
            format!("dictionary output table cannot be allocated: {error}"),
            span,
        ));
        return Err(CallOutcome::Failed);
    }
    for pair in dictionary.entries {
        let IrPair {
            first,
            second,
            span: pair_span,
        } = pair;
        let IrValue::String(key) = *first else {
            diagnostics.push(iteration_error(
                "Dictionary keys must remain typed strings".to_string(),
                pair_span,
            ));
            return Err(CallOutcome::Failed);
        };
        let value = value_into_table_cell(*second, pair_span, diagnostics)?;
        rows.push(IrTableRow {
            cells: vec![
                table_text_cell(&key, pair_span),
                IrTableCell {
                    content: value,
                    alignment: IrTableAlignment::None,
                    span: pair_span,
                },
            ],
            span: pair_span,
        });
    }
    Ok(IrNode::Table { header, rows, span })
}

fn table_text_cell(content: &str, span: SourceSpan) -> IrTableCell {
    IrTableCell {
        content: vec![IrInline::Text {
            content: content.to_string(),
            span,
        }],
        alignment: IrTableAlignment::None,
        span,
    }
}

fn value_into_table_cell(
    value: IrValue,
    span: SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<Vec<IrInline>, CallOutcome> {
    match value {
        IrValue::Content(nodes) => match nodes.as_slice() {
            [IrNode::Paragraph { content, .. }] => Ok(content.clone()),
            _ => {
                diagnostics.push(iteration_error(
                    "Dictionary values must be scalar or exactly one paragraph when rendered as a table cell"
                        .to_string(),
                    span,
                ));
                Err(CallOutcome::Failed)
            }
        },
        IrValue::Collection(values) => {
            let nodes = value_into_content_nodes(IrValue::Collection(values), span, diagnostics)?;
            match nodes.as_slice() {
                [IrNode::Paragraph { content, .. }] => Ok(content.clone()),
                _ => {
                    diagnostics.push(iteration_error(
                        "A multi-value Collection cannot be rendered as one Dictionary table cell"
                            .to_string(),
                        span,
                    ));
                    Err(CallOutcome::Failed)
                }
            }
        }
        IrValue::Pair(_) | IrValue::Dictionary(_) | IrValue::Range(_) => {
            diagnostics.push(iteration_error(
                "Nested Pair, Dictionary, or Range values cannot be rendered as one Dictionary table cell"
                    .to_string(),
                span,
            ));
            Err(CallOutcome::Failed)
        }
        scalar => match scalar_to_text(&scalar, span, diagnostics) {
            Ok(content) => Ok(vec![IrInline::Text { content, span }]),
            Err(outcome) => Err(outcome),
        },
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
    lambda_scope: Option<LambdaScope>,
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
            lambda_scope: None,
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
        parameters: LambdaParameters,
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
            LambdaParameters::Explicit(parameters),
            Vec::new(),
            SourceSpan::new(crate::source::SourceId(0), 0, 0),
        );
    }

    fn set_lambda_scope(&mut self, scope: LambdaScope) {
        self.lambda_scope = Some(scope);
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

    /// Resolves a numeric implicit parameter only inside the nearest lambda
    /// invocation. Explicit lambda scopes are a hard boundary: numeric
    /// references are diagnosed locally instead of falling through to an
    /// outer implicit invocation.
    fn get_implicit_parameter(
        &self,
        name: &str,
    ) -> Option<Result<IrValue, ImplicitParameterError>> {
        let index = implicit_parameter_index(name)?;
        match self.lambda_scope.as_ref() {
            Some(LambdaScope::Explicit) => Some(Err(ImplicitParameterError::Missing)),
            Some(LambdaScope::Implicit(arguments)) => Some(match index {
                ImplicitParameterIndex::Valid(index) => arguments
                    .get(index.saturating_sub(1))
                    .cloned()
                    .ok_or(ImplicitParameterError::Missing),
                ImplicitParameterIndex::Overflow => Err(ImplicitParameterError::Overflow),
            }),
            None => self
                .parent
                .as_deref()
                .and_then(|parent| parent.get_implicit_parameter(name)),
        }
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
                lambda_parameters,
                body,
                span,
            } => match self.evaluate_block_call(
                name,
                positional_args,
                named_args,
                lambda_parameters.as_deref(),
                body.as_deref(),
                span,
                diagnostics,
                context,
            ) {
                CallOutcome::Value(IrValue::Content(nodes)) => nodes,
                CallOutcome::Value(_) | CallOutcome::NoValue | CallOutcome::Failed => Vec::new(),
                CallOutcome::Unresolved => Vec::new(),
            },
            IrNode::ChainedFunctionCall {
                head,
                chain,
                body,
                span,
            } => match self.evaluate_block_chain(head, chain, body, span, diagnostics, context) {
                CallOutcome::Value(IrValue::Content(nodes)) => nodes,
                CallOutcome::Value(_) | CallOutcome::NoValue | CallOutcome::Failed => Vec::new(),
                CallOutcome::Unresolved => Vec::new(),
            },
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
                title,
                span,
            } => vec![IrInline::Link {
                content: self.evaluate_inlines(content, diagnostics, context),
                destination: destination.clone(),
                title: title.clone(),
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
        lambda_parameters: Option<&[IrParameter]>,
        body: Option<&[IrNode]>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> CallOutcome {
        match self.evaluate_call_value(
            name,
            positional_args,
            named_args,
            body.map(CallBody::Block),
            lambda_parameters,
            span,
            diagnostics,
            context,
        ) {
            CallOutcome::Value(value) => {
                match self.materialize_block_value(value, span, diagnostics) {
                    Ok(nodes) => CallOutcome::Value(IrValue::Content(nodes)),
                    Err(outcome) => outcome,
                }
            }
            CallOutcome::NoValue => CallOutcome::NoValue,
            CallOutcome::Failed => CallOutcome::Failed,
            CallOutcome::Unresolved => match self.preserve_block_call(
                name,
                positional_args,
                named_args,
                lambda_parameters,
                body,
                span,
                diagnostics,
                context,
            ) {
                Ok(nodes) => CallOutcome::Value(IrValue::Content(nodes)),
                Err(outcome) => outcome,
            },
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
            None,
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
    ) -> CallOutcome {
        match self.evaluate_chain_value(
            head,
            chain,
            body.as_deref().map(CallBody::Block),
            diagnostics,
            context,
        ) {
            CallOutcome::Value(value) => {
                match self.materialize_block_value(value, span, diagnostics) {
                    Ok(nodes) => CallOutcome::Value(IrValue::Content(nodes)),
                    Err(outcome) => outcome,
                }
            }
            CallOutcome::NoValue => CallOutcome::NoValue,
            CallOutcome::Failed => CallOutcome::Failed,
            CallOutcome::Unresolved => CallOutcome::Unresolved,
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
        lambda_parameters: Option<&[IrParameter]>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> CallOutcome {
        if let Some(result) = context.get_implicit_parameter(name) {
            return match result {
                Ok(value) => CallOutcome::Value(value),
                Err(error) => {
                    diagnostics.push(implicit_parameter_error(name, error, *span));
                    CallOutcome::Failed
                }
            };
        }

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

        if is_let(name) {
            return self.evaluate_let(
                positional_args,
                named_args,
                body,
                lambda_parameters,
                span,
                diagnostics,
                context,
            );
        }

        if is_foreach(name) {
            return self.evaluate_foreach(
                positional_args,
                named_args,
                body,
                lambda_parameters,
                span,
                diagnostics,
                context,
            );
        }

        if is_repeat(name) {
            return self.evaluate_repeat(
                positional_args,
                named_args,
                body,
                lambda_parameters,
                span,
                diagnostics,
                context,
            );
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

        if is_pair(name) {
            return self.evaluate_pair(
                positional_args,
                named_args,
                body,
                span,
                diagnostics,
                context,
            );
        }

        if is_dictionary(name) {
            return self.evaluate_dictionary(
                positional_args,
                named_args,
                body,
                span,
                diagnostics,
                context,
            );
        }

        if is_collection_access(name) {
            if body.is_some() {
                diagnostics.push(iteration_error(
                    format!("`.{name}` does not accept a block body"),
                    *span,
                ));
                return CallOutcome::Failed;
            }
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
            return self.evaluate_collection_access(
                name,
                &evaluated_positional,
                &evaluated_named,
                span,
                diagnostics,
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

    /// Evaluates the bounded Collection access operations through the same
    /// ordered semantic element adaptation used by `.foreach`.
    fn evaluate_collection_access(
        &self,
        name: &str,
        positional_args: &[IrValue],
        named_args: &[IrNamedArg],
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> CallOutcome {
        match name {
            "size" | "first" | "last" => {
                let named_parameter = if name == "size" { "of" } else { "from" };
                let value = match collection_access_operand(
                    name,
                    named_parameter,
                    positional_args,
                    named_args,
                    span,
                    diagnostics,
                ) {
                    Ok(value) => value,
                    Err(outcome) => return outcome,
                };
                let elements = match self.coerce_iterable(value, span, diagnostics) {
                    Ok(elements) => elements,
                    Err(outcome) => return outcome,
                };
                match name {
                    "size" => match exact_collection_length(elements.len(), span, diagnostics) {
                        Ok(length) => CallOutcome::Value(IrValue::Number(length)),
                        Err(outcome) => outcome,
                    },
                    "first" => {
                        CallOutcome::Value(elements.into_iter().next().unwrap_or(IrValue::None))
                    }
                    "last" => {
                        CallOutcome::Value(elements.into_iter().last().unwrap_or(IrValue::None))
                    }
                    _ => unreachable!("collection access operation was prevalidated"),
                }
            }
            "getat" => {
                let (value, index, fallback) =
                    match getat_operands(positional_args, named_args, span, diagnostics) {
                        Ok(operands) => operands,
                        Err(outcome) => return outcome,
                    };
                let elements = match self.coerce_iterable(value, span, diagnostics) {
                    Ok(elements) => elements,
                    Err(outcome) => return outcome,
                };
                let length = match exact_collection_length(elements.len(), span, diagnostics) {
                    Ok(length) => length,
                    Err(outcome) => return outcome,
                };
                let index = match collection_index(&index, length, span, diagnostics) {
                    Ok(index) => index,
                    Err(outcome) => return outcome,
                };
                CallOutcome::Value(
                    index
                        .and_then(|index| elements.get(index).cloned())
                        .unwrap_or(fallback),
                )
            }
            _ => unreachable!("collection access operation was prevalidated"),
        }
    }

    /// Evaluates `.pair` as a typed, recursively valued pair.
    #[allow(clippy::too_many_arguments)]
    fn evaluate_pair(
        &self,
        positional_args: &[IrValue],
        named_args: &[IrNamedArg],
        body: Option<CallBody<'_>>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> CallOutcome {
        if positional_args.len() != 2 {
            diagnostics.push(iteration_error(
                format!(
                    "`.pair` requires exactly two positional values (received {})",
                    positional_args.len()
                ),
                *span,
            ));
            return CallOutcome::Failed;
        }
        if let Some(argument) = named_args.first() {
            diagnostics.push(iteration_error_at(
                format!("Unknown named argument `{}` for `.pair`", argument.name),
                argument.name_span,
            ));
            return CallOutcome::Failed;
        }
        if body.is_some() {
            diagnostics.push(iteration_error(
                "`.pair` does not accept a block body".to_string(),
                *span,
            ));
            return CallOutcome::Failed;
        }
        let values = match self.evaluate_values(positional_args, span, diagnostics, context) {
            Ok(values) => values,
            Err(outcome) => return outcome,
        };
        let mut values = values.into_iter();
        let Some(first) = values.next() else {
            return CallOutcome::Failed;
        };
        let Some(second) = values.next() else {
            return CallOutcome::Failed;
        };
        CallOutcome::Value(IrValue::Pair(IrPair {
            first: Box::new(first),
            second: Box::new(second),
            span: *span,
        }))
    }

    /// Evaluates `.dictionary` from the already parsed Markdown list body.
    /// Entry evaluation is collected privately and published only after all
    /// entries succeed, preserving atomic materialization and source order.
    #[allow(clippy::too_many_arguments)]
    fn evaluate_dictionary(
        &self,
        positional_args: &[IrValue],
        named_args: &[IrNamedArg],
        body: Option<CallBody<'_>>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> CallOutcome {
        if !positional_args.is_empty() {
            diagnostics.push(iteration_error(
                "`.dictionary` accepts its entries as a block body".to_string(),
                *span,
            ));
            return CallOutcome::Failed;
        }
        if let Some(argument) = named_args.first() {
            diagnostics.push(iteration_error_at(
                format!(
                    "Unknown named argument `{}` for `.dictionary`",
                    argument.name
                ),
                argument.name_span,
            ));
            return CallOutcome::Failed;
        }
        let body = match body {
            None => &[][..],
            Some(CallBody::Block(nodes)) => nodes,
            Some(CallBody::Inline(_)) => {
                diagnostics.push(iteration_error(
                    "`.dictionary` requires a Markdown list block body".to_string(),
                    *span,
                ));
                return CallOutcome::Failed;
            }
        };
        let entries = match self.evaluate_dictionary_entries(body, *span, diagnostics, context) {
            Ok(entries) => entries,
            Err(outcome) => return outcome,
        };
        CallOutcome::Value(IrValue::Dictionary(IrDictionary {
            entries,
            span: *span,
        }))
    }

    fn evaluate_dictionary_entries(
        &self,
        nodes: &[IrNode],
        span: SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> Result<Vec<IrPair>, CallOutcome> {
        let list = match nodes {
            [] => return Ok(Vec::new()),
            [IrNode::UnorderedList { items, .. }] | [IrNode::OrderedList { items, .. }] => items,
            _ => {
                diagnostics.push(iteration_error(
                    "`.dictionary` requires exactly one Markdown list body".to_string(),
                    span,
                ));
                return Err(CallOutcome::Failed);
            }
        };
        let mut entries = Vec::new();
        if let Err(error) = entries.try_reserve_exact(list.len()) {
            diagnostics.push(iteration_error(
                format!("dictionary entries cannot be allocated: {error}"),
                span,
            ));
            return Err(CallOutcome::Failed);
        }
        for item in list {
            let (key, value) = self.dictionary_item_parts(item, span, diagnostics, context)?;
            let pair = IrPair {
                first: Box::new(IrValue::String(key.clone())),
                second: Box::new(value),
                span: item.span,
            };
            if let Some(existing) = entries.iter_mut().find(|entry: &&mut IrPair| {
                matches!(entry.first.as_ref(), IrValue::String(existing_key) if existing_key == &key)
            }) {
                // Quarkdown's last-write-wins behavior replaces the value in
                // the original insertion slot, keeping iteration deterministic.
                *existing = pair;
            } else {
                entries.push(pair);
            }
        }
        Ok(entries)
    }

    fn dictionary_item_parts(
        &self,
        item: &IrListItem,
        fallback_span: SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> Result<(String, IrValue), CallOutcome> {
        let Some(IrNode::Paragraph { content, span }) = item.nodes.first() else {
            diagnostics.push(iteration_error(
                "Dictionary entries must start with a Markdown paragraph".to_string(),
                item.span,
            ));
            return Err(CallOutcome::Failed);
        };
        let (key, value_inlines, value_text, value_span) =
            if let Some(parts) = split_dictionary_paragraph(content, *span) {
                parts
            } else if item.nodes.len() > 1 {
                let Some(key) = plain_dictionary_key(content) else {
                    diagnostics.push(iteration_error(
                        "Dictionary entries require a string key".to_string(),
                        item.span,
                    ));
                    return Err(CallOutcome::Failed);
                };
                (key, Vec::new(), String::new(), *span)
            } else {
                diagnostics.push(iteration_error(
                    "Dictionary entries require a string key followed by `:`".to_string(),
                    item.span,
                ));
                return Err(CallOutcome::Failed);
            };
        if key.is_empty() {
            diagnostics.push(iteration_error(
                "Dictionary keys must not be empty".to_string(),
                item.span,
            ));
            return Err(CallOutcome::Failed);
        }

        let value = if value_inlines.is_empty() && value_text.is_empty() {
            let nested = &item.nodes[1..];
            if nested.is_empty() {
                IrValue::String(String::new())
            } else {
                let nested =
                    self.evaluate_dictionary_entries(nested, item.span, diagnostics, context)?;
                IrValue::Dictionary(IrDictionary {
                    entries: nested,
                    span: item.span,
                })
            }
        } else if value_inlines.is_empty() {
            dictionary_scalar_value(&value_text)
        } else {
            let value = dictionary_inline_value(value_inlines, value_span);
            match self.evaluate_value(&value, diagnostics, context) {
                CallOutcome::Value(value) => value,
                CallOutcome::Unresolved => {
                    self.preserve_value_expression(&value, diagnostics, context)?
                }
                CallOutcome::NoValue => {
                    diagnostics.push(no_value_required(value_source_span(&value, &fallback_span)));
                    return Err(CallOutcome::Failed);
                }
                CallOutcome::Failed => return Err(CallOutcome::Failed),
            }
        };
        Ok((key, value))
    }

    /// Evaluates block-form `.let` as a scoped one-argument lambda
    /// invocation. The value is resolved in the caller context exactly once;
    /// only then is the invocation-local child scope created and populated.
    #[allow(clippy::too_many_arguments)]
    fn evaluate_let(
        &self,
        positional_args: &[IrValue],
        named_args: &[IrNamedArg],
        body: Option<CallBody<'_>>,
        lambda_parameters: Option<&[IrParameter]>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> CallOutcome {
        if positional_args.len() != 1 {
            diagnostics.push(let_error(
                format!(
                    "`.let` requires exactly one positional value argument (received {})",
                    positional_args.len()
                ),
                *span,
            ));
            return CallOutcome::Failed;
        }
        if let Some(argument) = named_args.first() {
            diagnostics.push(let_error_at(
                format!("Unknown named argument `{}` for `.let`", argument.name),
                argument.name_span,
            ));
            return CallOutcome::Failed;
        }

        let body = match body {
            Some(CallBody::Block(nodes)) => nodes,
            Some(CallBody::Inline(_)) => {
                diagnostics.push(let_error(
                    "`.let` supports only the block lambda form".to_string(),
                    *span,
                ));
                return CallOutcome::Failed;
            }
            None => {
                diagnostics.push(let_error(
                    "`.let` requires a block lambda body".to_string(),
                    *span,
                ));
                return CallOutcome::Failed;
            }
        };

        if let Some(parameters) = lambda_parameters {
            if parameters.len() != 1 {
                let parameter_span = parameters
                    .first()
                    .map(|parameter| parameter.span)
                    .unwrap_or(*span);
                diagnostics.push(let_error_at(
                    format!(
                        "`.let` requires exactly one explicit lambda parameter (received {})",
                        parameters.len()
                    ),
                    parameter_span,
                ));
                return CallOutcome::Failed;
            }
        }

        let value = match self.evaluate_value(&positional_args[0], diagnostics, context) {
            CallOutcome::Value(value) => value,
            CallOutcome::Unresolved => {
                match self.preserve_value_expression(&positional_args[0], diagnostics, context) {
                    Ok(value) => value,
                    Err(outcome) => return outcome,
                }
            }
            CallOutcome::NoValue => {
                diagnostics.push(no_value_required(value_source_span(
                    &positional_args[0],
                    span,
                )));
                return CallOutcome::Failed;
            }
            CallOutcome::Failed => return CallOutcome::Failed,
        };

        self.invoke_scoped_lambda(
            value,
            lambda_parameters,
            body,
            IterationOptions {
                span: *span,
                allow_destructuring: false,
            },
            diagnostics,
            context,
        )
    }

    /// Evaluates block-form `.foreach` as a typed map over one iterable.
    /// The iterable is resolved before any child scope is created and exactly
    /// once; every mapped element gets a fresh invocation-local child scope.
    #[allow(clippy::too_many_arguments)]
    fn evaluate_foreach(
        &self,
        positional_args: &[IrValue],
        named_args: &[IrNamedArg],
        body: Option<CallBody<'_>>,
        lambda_parameters: Option<&[IrParameter]>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> CallOutcome {
        if positional_args.len() != 1 {
            diagnostics.push(iteration_error(
                format!(
                    "`.foreach` requires exactly one positional iterable argument (received {})",
                    positional_args.len()
                ),
                *span,
            ));
            return CallOutcome::Failed;
        }
        if let Some(argument) = named_args.first() {
            diagnostics.push(iteration_error_at(
                format!("Unknown named argument `{}` for `.foreach`", argument.name),
                argument.name_span,
            ));
            return CallOutcome::Failed;
        }
        let body = match body {
            Some(CallBody::Block(nodes)) => nodes,
            Some(CallBody::Inline(_)) => {
                diagnostics.push(iteration_error(
                    "`.foreach` supports only the block lambda form in this slice".to_string(),
                    *span,
                ));
                return CallOutcome::Failed;
            }
            None => {
                diagnostics.push(iteration_error(
                    "`.foreach` requires a block lambda body".to_string(),
                    *span,
                ));
                return CallOutcome::Failed;
            }
        };
        if !validate_iteration_lambda(lambda_parameters, ".foreach", true, span, diagnostics) {
            return CallOutcome::Failed;
        }

        let value = match self.evaluate_value(&positional_args[0], diagnostics, context) {
            CallOutcome::Value(value) => value,
            CallOutcome::Unresolved => {
                match self.preserve_value_expression(&positional_args[0], diagnostics, context) {
                    Ok(value) => value,
                    Err(outcome) => return outcome,
                }
            }
            CallOutcome::NoValue => {
                diagnostics.push(no_value_required(value_source_span(
                    &positional_args[0],
                    span,
                )));
                return CallOutcome::Failed;
            }
            CallOutcome::Failed => return CallOutcome::Failed,
        };
        let elements = match self.coerce_iterable(value, span, diagnostics) {
            Ok(elements) => elements,
            Err(outcome) => return outcome,
        };
        self.map_iteration_values(
            &elements,
            lambda_parameters,
            body,
            IterationOptions {
                span: *span,
                allow_destructuring: true,
            },
            diagnostics,
            context,
        )
    }

    /// Evaluates `.repeat` through the same iteration engine as `.foreach`.
    /// The count is a checked semantic integer, and indices are one-based.
    #[allow(clippy::too_many_arguments)]
    fn evaluate_repeat(
        &self,
        positional_args: &[IrValue],
        named_args: &[IrNamedArg],
        body: Option<CallBody<'_>>,
        lambda_parameters: Option<&[IrParameter]>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> CallOutcome {
        if positional_args.len() != 1 {
            diagnostics.push(iteration_error(
                format!(
                    "`.repeat` requires exactly one positional count argument (received {})",
                    positional_args.len()
                ),
                *span,
            ));
            return CallOutcome::Failed;
        }
        if let Some(argument) = named_args.first() {
            diagnostics.push(iteration_error_at(
                format!("Unknown named argument `{}` for `.repeat`", argument.name),
                argument.name_span,
            ));
            return CallOutcome::Failed;
        }
        let body = match body {
            Some(CallBody::Block(nodes)) => nodes,
            Some(CallBody::Inline(_)) => {
                diagnostics.push(iteration_error(
                    "`.repeat` supports only the block lambda form in this slice".to_string(),
                    *span,
                ));
                return CallOutcome::Failed;
            }
            None => {
                diagnostics.push(iteration_error(
                    "`.repeat` requires a block lambda body".to_string(),
                    *span,
                ));
                return CallOutcome::Failed;
            }
        };
        if !validate_iteration_lambda(lambda_parameters, ".repeat", false, span, diagnostics) {
            return CallOutcome::Failed;
        }

        let count_value = match self.evaluate_value(&positional_args[0], diagnostics, context) {
            CallOutcome::Value(value) => value,
            CallOutcome::Unresolved => {
                match self.preserve_value_expression(&positional_args[0], diagnostics, context) {
                    Ok(value) => value,
                    Err(outcome) => return outcome,
                }
            }
            CallOutcome::NoValue => {
                diagnostics.push(no_value_required(value_source_span(
                    &positional_args[0],
                    span,
                )));
                return CallOutcome::Failed;
            }
            CallOutcome::Failed => return CallOutcome::Failed,
        };
        let count = match repeat_count(&count_value) {
            Ok(count) => count,
            Err(message) => {
                diagnostics.push(iteration_error(
                    message,
                    value_source_span(&count_value, span),
                ));
                return CallOutcome::Failed;
            }
        };
        let elements = match self.materialize_closed_range(
            IrRange {
                start: Some(1),
                end: Some(count),
                span: *span,
            },
            span,
            diagnostics,
        ) {
            Ok(elements) => elements,
            Err(outcome) => return outcome,
        };
        self.map_iteration_values(
            &elements,
            lambda_parameters,
            body,
            IterationOptions {
                span: *span,
                allow_destructuring: false,
            },
            diagnostics,
            context,
        )
    }

    fn invoke_scoped_lambda(
        &self,
        value: IrValue,
        lambda_parameters: Option<&[IrParameter]>,
        body: &[IrNode],
        options: IterationOptions,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> CallOutcome {
        let mut child = context.child();
        match lambda_parameters {
            Some(parameters) => {
                let bindings = match scoped_parameter_bindings(
                    &value,
                    parameters,
                    options.allow_destructuring,
                    options.span,
                    diagnostics,
                ) {
                    Ok(bindings) => bindings,
                    Err(outcome) => return outcome,
                };
                child.set_lambda_scope(LambdaScope::Explicit);
                for (name, value) in bindings {
                    child.set_value(name, value);
                }
            }
            None => child.set_lambda_scope(LambdaScope::Implicit(vec![value])),
        }
        self.evaluate_callable_body_value(body, diagnostics, &mut child)
    }

    fn map_iteration_values(
        &self,
        elements: &[IrValue],
        lambda_parameters: Option<&[IrParameter]>,
        body: &[IrNode],
        options: IterationOptions,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> CallOutcome {
        let mut results = Vec::new();
        if let Err(error) = results.try_reserve_exact(elements.len()) {
            diagnostics.push(iteration_error(
                format!("iteration result collection cannot be allocated: {error}"),
                options.span,
            ));
            return CallOutcome::Failed;
        }
        for element in elements {
            match self.invoke_scoped_lambda(
                element.clone(),
                lambda_parameters,
                body,
                options,
                diagnostics,
                context,
            ) {
                CallOutcome::Value(value) => results.push(value),
                CallOutcome::NoValue => {
                    diagnostics.push(no_value_required(options.span));
                    return CallOutcome::Failed;
                }
                CallOutcome::Failed => return CallOutcome::Failed,
                CallOutcome::Unresolved => return CallOutcome::Unresolved,
            }
        }
        CallOutcome::Value(IrValue::Collection(results))
    }

    fn coerce_iterable(
        &self,
        value: IrValue,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Result<Vec<IrValue>, CallOutcome> {
        match value {
            IrValue::Collection(values) => Ok(values),
            IrValue::Pair(pair) => Ok(vec![*pair.first, *pair.second]),
            IrValue::Dictionary(dictionary) => {
                Ok(dictionary.entries.into_iter().map(IrValue::Pair).collect())
            }
            IrValue::Range(range) => self.materialize_range(range, span, diagnostics),
            IrValue::Content(nodes) => match nodes.as_slice() {
                [IrNode::UnorderedList { items, .. }] | [IrNode::OrderedList { items, .. }] => {
                    let mut values = Vec::new();
                    if let Err(error) = values.try_reserve_exact(items.len()) {
                        diagnostics.push(iteration_error(
                            format!("list collection cannot be allocated: {error}"),
                            *span,
                        ));
                        return Err(CallOutcome::Failed);
                    }
                    for item in items {
                        values.push(self.list_item_value(item, span, diagnostics)?);
                    }
                    Ok(values)
                }
                _ => {
                    diagnostics.push(iteration_error(
                        "Value is not an iterable Range, Collection, Pair, Dictionary, or exactly one Markdown list"
                            .to_string(),
                        *span,
                    ));
                    Err(CallOutcome::Failed)
                }
            },
            _ => {
                diagnostics.push(iteration_error(
                    "Value is not an iterable Range, Collection, Pair, Dictionary, or exactly one Markdown list"
                        .to_string(),
                    *span,
                ));
                Err(CallOutcome::Failed)
            }
        }
    }

    fn list_item_value(
        &self,
        item: &crate::ir::IrListItem,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Result<IrValue, CallOutcome> {
        match item.nodes.as_slice() {
            [IrNode::UnorderedList { .. }] | [IrNode::OrderedList { .. }] => self
                .coerce_iterable(IrValue::Content(item.nodes.clone()), span, diagnostics)
                .map(IrValue::Collection),
            _ => Ok(IrValue::Content(item.nodes.clone())),
        }
    }

    fn materialize_range(
        &self,
        range: IrRange,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Result<Vec<IrValue>, CallOutcome> {
        let (Some(start), Some(end)) = (range.start, range.end) else {
            diagnostics.push(iteration_error(
                "Open Range iteration is deferred in this Scribium slice".to_string(),
                range.span,
            ));
            return Err(CallOutcome::Failed);
        };
        self.materialize_closed_range(
            IrRange {
                start: Some(start),
                end: Some(end),
                span: range.span,
            },
            span,
            diagnostics,
        )
    }

    fn materialize_closed_range(
        &self,
        range: IrRange,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Result<Vec<IrValue>, CallOutcome> {
        let (Some(start), Some(end)) = (range.start, range.end) else {
            diagnostics.push(iteration_error(
                "Internal error: a closed range requires both endpoints".to_string(),
                *span,
            ));
            return Err(CallOutcome::Failed);
        };
        if start > end {
            // Verified against Quarkdown v2.5.1: Range(4, 2) delegates to
            // Kotlin IntRange(4, 2), whose iterator is empty.
            return Ok(Vec::new());
        }
        const MAX_EXACT_F64_INTEGER: u64 = 1 << 53;
        if start > MAX_EXACT_F64_INTEGER || end > MAX_EXACT_F64_INTEGER {
            diagnostics.push(iteration_error(
                "Closed Range endpoint cannot be represented exactly by the evaluator Number type"
                    .to_string(),
                range.span,
            ));
            return Err(CallOutcome::Failed);
        }
        let Some(count) = end
            .checked_sub(start)
            .and_then(|distance| distance.checked_add(1))
        else {
            diagnostics.push(iteration_error(
                "Closed Range cardinality overflowed the supported integer domain".to_string(),
                range.span,
            ));
            return Err(CallOutcome::Failed);
        };
        let Ok(capacity) = usize::try_from(count) else {
            diagnostics.push(iteration_error(
                "Closed Range is too large to materialize on this target".to_string(),
                range.span,
            ));
            return Err(CallOutcome::Failed);
        };
        let mut values = Vec::new();
        if let Err(error) = values.try_reserve_exact(capacity) {
            diagnostics.push(iteration_error(
                format!("Closed Range cannot be materialized: {error}"),
                range.span,
            ));
            return Err(CallOutcome::Failed);
        }
        let mut current = start;
        loop {
            values.push(IrValue::Number(current as f64));
            if current == end {
                break;
            }
            current = match current.checked_add(1) {
                Some(next) => next,
                None => {
                    diagnostics.push(iteration_error(
                        "Closed Range iteration overflowed its endpoint".to_string(),
                        range.span,
                    ));
                    return Err(CallOutcome::Failed);
                }
            };
        }
        Ok(values)
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
        let bound = match self.bind_callable_arguments(
            &binding.parameters,
            positional,
            named,
            body,
            span,
            diagnostics,
            context,
        ) {
            Ok(bound) => bound,
            Err(outcome) => return outcome,
        };
        let mut child = context.child();
        match bound {
            BoundLambdaArguments::Explicit(values) => {
                child.set_lambda_scope(LambdaScope::Explicit);
                if let LambdaParameters::Explicit(parameters) = &binding.parameters {
                    for (parameter, value) in parameters.iter().zip(values) {
                        child.set_value(parameter.name.clone(), value);
                    }
                }
            }
            BoundLambdaArguments::Implicit(values) => {
                child.set_lambda_scope(LambdaScope::Implicit(values));
            }
        }
        self.evaluate_callable_body_value(&binding.body, diagnostics, &mut child)
    }

    /// Evaluates and binds one callable's arguments for either parameter mode.
    /// The result is consumed by the shared child-scope/body invocation path.
    #[allow(clippy::too_many_arguments)]
    fn bind_callable_arguments(
        &self,
        parameters: &LambdaParameters,
        positional: Vec<IrValue>,
        named: Vec<IrNamedArg>,
        body: Option<CallBody<'_>>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> Result<BoundLambdaArguments, CallOutcome> {
        match parameters {
            LambdaParameters::Implicit => {
                if let Some(argument) = named.first() {
                    diagnostics.push(function_error_at(
                        "Implicit lambda parameters are positional only".to_string(),
                        argument.name_span,
                    ));
                    return Err(CallOutcome::Failed);
                }
                let mut arguments = positional;
                if let Some(body) = body {
                    let value = match self.evaluate_call_body(body, span, diagnostics, context) {
                        CallOutcome::Value(value) => value,
                        CallOutcome::NoValue => return Err(CallOutcome::NoValue),
                        CallOutcome::Failed => return Err(CallOutcome::Failed),
                        CallOutcome::Unresolved => return Err(CallOutcome::Unresolved),
                    };
                    arguments.push(value);
                }
                Ok(BoundLambdaArguments::Implicit(arguments))
            }
            LambdaParameters::Explicit(parameters) => {
                let mut bound: Vec<Option<IrValue>> = vec![None; parameters.len()];
                for (index, value) in positional.into_iter().enumerate() {
                    let Some(slot) = bound.get_mut(index) else {
                        diagnostics.push(function_error(
                            format!(
                                "Function call has too many positional arguments (received at least {})",
                                index + 1
                            ),
                            *span,
                        ));
                        return Err(CallOutcome::Failed);
                    };
                    *slot = Some(value);
                }

                for argument in &named {
                    let Some(index) = parameters
                        .iter()
                        .position(|parameter| parameter.name == argument.name)
                    else {
                        diagnostics.push(function_error_at(
                            format!("Unknown named parameter `{}`", argument.name),
                            argument.name_span,
                        ));
                        return Err(CallOutcome::Failed);
                    };
                    if bound[index].is_some() {
                        diagnostics.push(function_error_at(
                            format!("Parameter `{}` was bound more than once", argument.name),
                            argument.name_span,
                        ));
                        return Err(CallOutcome::Failed);
                    }
                    bound[index] = Some(argument.value.clone());
                }

                if let Some(body) = body {
                    let Some(last) = bound.last() else {
                        diagnostics.push(function_error(
                            "A block argument requires a final function parameter".to_string(),
                            *span,
                        ));
                        return Err(CallOutcome::Failed);
                    };
                    if last.is_some() {
                        diagnostics.push(function_error(
                            "A block argument collides with the function's final parameter binding"
                                .to_string(),
                            *span,
                        ));
                        return Err(CallOutcome::Failed);
                    }
                    let value = match self.evaluate_call_body(body, span, diagnostics, context) {
                        CallOutcome::Value(value) => value,
                        CallOutcome::NoValue => return Err(CallOutcome::NoValue),
                        CallOutcome::Failed => return Err(CallOutcome::Failed),
                        CallOutcome::Unresolved => return Err(CallOutcome::Unresolved),
                    };
                    if let Some(last) = bound.last_mut() {
                        *last = Some(value);
                    }
                }

                for (index, parameter) in parameters.iter().enumerate() {
                    if bound[index].is_none() {
                        if parameter.optional {
                            bound[index] = Some(IrValue::None);
                        } else {
                            diagnostics.push(function_error_at(
                                format!("Missing required argument `{}`", parameter.name),
                                parameter.name_span,
                            ));
                            return Err(CallOutcome::Failed);
                        }
                    }
                }

                Ok(BoundLambdaArguments::Explicit(
                    bound.into_iter().flatten().collect(),
                ))
            }
        }
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
                CallOutcome::Value(value) => {
                    if let Err(outcome) = result.append_value(value, span, diagnostics) {
                        return outcome;
                    }
                }
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
                lambda_parameters,
                body,
                span,
            } => match self.evaluate_call_value(
                name,
                positional_args,
                named_args,
                body.as_deref().map(CallBody::Block),
                lambda_parameters.as_deref(),
                span,
                diagnostics,
                context,
            ) {
                CallOutcome::Unresolved => self
                    .preserve_block_call(
                        name,
                        positional_args,
                        named_args,
                        lambda_parameters.as_deref(),
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
                    None,
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
                        "Function `{}` is visible in this scope but callable function declarations are not implemented yet ({})",
                        segment.name,
                        binding.parameters.description()
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
            CallOutcome::Value(value) => match self.scalar_or_content(value, span, diagnostics) {
                Ok(value) => CallOutcome::Value(value),
                Err(outcome) => outcome,
            },
            CallOutcome::NoValue => {
                diagnostics.push(no_value_required(value_source_span(value, span)));
                CallOutcome::Failed
            }
            CallOutcome::Failed => CallOutcome::Failed,
            CallOutcome::Unresolved => {
                match self.preserve_value_expression(value, diagnostics, context) {
                    Ok(value) => match self.scalar_or_content(value, span, diagnostics) {
                        Ok(value) => CallOutcome::Value(value),
                        Err(outcome) => outcome,
                    },
                    Err(outcome) => outcome,
                }
            }
        }
    }

    fn scalar_or_content(
        &self,
        value: IrValue,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Result<IrValue, CallOutcome> {
        match value {
            IrValue::Content(nodes) => Ok(IrValue::Content(nodes)),
            value => value_into_content_nodes(value, *span, diagnostics).map(IrValue::Content),
        }
    }

    fn validate_preserved_value(
        &self,
        value: &IrValue,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Result<(), CallOutcome> {
        match value {
            IrValue::Range(range) => {
                diagnostics.push(iteration_error(
                    "A typed Range cannot be preserved as an unresolved call argument; consume it through iteration first"
                        .to_string(),
                    range.span,
                ));
                Err(CallOutcome::Failed)
            }
            IrValue::Collection(values) => {
                for value in values {
                    self.validate_preserved_value(value, diagnostics)?;
                }
                Ok(())
            }
            IrValue::Pair(pair) => {
                self.validate_preserved_value(&pair.first, diagnostics)?;
                self.validate_preserved_value(&pair.second, diagnostics)
            }
            IrValue::Dictionary(dictionary) => {
                for pair in &dictionary.entries {
                    self.validate_preserved_value(&pair.first, diagnostics)?;
                    self.validate_preserved_value(&pair.second, diagnostics)?;
                }
                Ok(())
            }
            _ => Ok(()),
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
                    lambda_parameters,
                    body,
                    span,
                }] = nodes.as_slice()
                {
                    return self
                        .preserve_block_call(
                            name,
                            positional_args,
                            named_args,
                            lambda_parameters.as_deref(),
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
            scalar => {
                self.validate_preserved_value(scalar, diagnostics)?;
                Ok(scalar.clone())
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn preserve_block_call(
        &self,
        name: &str,
        positional_args: &[IrValue],
        named_args: &[IrNamedArg],
        lambda_parameters: Option<&[IrParameter]>,
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
            lambda_parameters: lambda_parameters.map(ToOwned::to_owned),
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

    fn materialize_block_value(
        &self,
        value: IrValue,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Result<Vec<IrNode>, CallOutcome> {
        match value {
            IrValue::Content(nodes) => Ok(nodes),
            IrValue::Collection(values) => {
                let mut nodes = Vec::new();
                for value in values {
                    let materialized = self.materialize_block_value(value, span, diagnostics)?;
                    if let Err(error) = nodes.try_reserve(materialized.len()) {
                        diagnostics.push(iteration_error(
                            format!("collection output cannot be allocated: {error}"),
                            *span,
                        ));
                        return Err(CallOutcome::Failed);
                    }
                    nodes.extend(materialized);
                }
                Ok(nodes)
            }
            IrValue::Pair(pair) => pair_into_content_nodes(pair, diagnostics),
            IrValue::Dictionary(dictionary) => {
                dictionary_into_table(dictionary, diagnostics).map(|table| vec![table])
            }
            IrValue::Range(range) => {
                diagnostics.push(iteration_error(
                    "Direct Range materialization is deferred; consume the typed Range through iteration first"
                        .to_string(),
                    range.span,
                ));
                Err(CallOutcome::Failed)
            }
            value => match scalar_to_text(&value, *span, diagnostics) {
                Ok(content) => Ok(vec![IrNode::Paragraph {
                    content: vec![IrInline::Text {
                        content,
                        span: *span,
                    }],
                    span: *span,
                }]),
                Err(outcome) => Err(outcome),
            },
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
            Some(IrValue::Collection(values)) => {
                if values.len() != 1 {
                    diagnostics.push(function_error(
                        "A Collection cannot be flattened into inline content unless it has exactly one element"
                            .to_string(),
                        *span,
                    ));
                    Vec::new()
                } else {
                    self.materialize_inline_value(values.into_iter().next(), span, diagnostics)
                }
            }
            Some(IrValue::Range(range)) => {
                diagnostics.push(iteration_error(
                    "Direct Range materialization is deferred; consume the typed Range through iteration first"
                        .to_string(),
                    range.span,
                ));
                Vec::new()
            }
            Some(value) => match scalar_to_text(&value, *span, diagnostics) {
                Ok(content) => vec![IrInline::Text {
                    content,
                    span: *span,
                }],
                Err(_) => Vec::new(),
            },
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
                    lambda_parameters,
                    body,
                    span,
                }] = nodes.as_slice()
                {
                    return self.evaluate_call_value(
                        name,
                        positional_args,
                        named_args,
                        body.as_deref().map(CallBody::Block),
                        lambda_parameters.as_deref(),
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
        let mut evaluated = Vec::new();
        if let Err(error) = evaluated.try_reserve(values.len()) {
            diagnostics.push(iteration_error(
                format!("call arguments cannot be allocated: {error}"),
                *span,
            ));
            return Err(CallOutcome::Failed);
        }
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

    fn evaluate_named(
        &self,
        named: &[IrNamedArg],
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext,
    ) -> Result<Vec<IrNamedArg>, CallOutcome> {
        let mut evaluated = Vec::new();
        if let Err(error) = evaluated.try_reserve(named.len()) {
            diagnostics.push(iteration_error(
                format!("named call arguments cannot be allocated: {error}"),
                *span,
            ));
            return Err(CallOutcome::Failed);
        }
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
                CallOutcome::Value(value) => {
                    self.validate_preserved_value(&value, diagnostics)?;
                    evaluated.push(value);
                }
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
                CallOutcome::Value(value) => {
                    self.validate_preserved_value(&value, diagnostics)?;
                    value
                }
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

fn split_dictionary_paragraph(
    content: &[IrInline],
    paragraph_span: SourceSpan,
) -> Option<(String, Vec<IrInline>, String, SourceSpan)> {
    let IrInline::Text {
        content: text,
        span: text_span,
    } = content.first()?
    else {
        return None;
    };
    let colon = text.find(':')?;
    let key = text[..colon].trim().to_string();
    let after = &text[colon + 1..];
    let leading = after.len() - after.trim_start().len();
    let trimmed = after.trim();
    let value_start = text_span
        .start
        .checked_add(colon + 1 + leading)
        .unwrap_or(text_span.end);
    let value_end = value_start
        .checked_add(trimmed.len())
        .unwrap_or(value_start);
    let value_span = SourceSpan::new(text_span.source_id, value_start, value_end);
    if !trimmed.is_empty() {
        return Some((key, Vec::new(), trimmed.to_string(), value_span));
    }
    let mut tail = content.get(1..).unwrap_or_default().to_vec();
    if let Some(IrInline::Text {
        content: tail_text,
        span: tail_span,
    }) = tail.first_mut()
    {
        let leading = tail_text.len() - tail_text.trim_start().len();
        let trailing = tail_text.trim().len();
        if leading > 0 || trailing != tail_text.len() {
            let trimmed = tail_text.trim().to_string();
            let start = tail_span
                .start
                .checked_add(leading)
                .unwrap_or(tail_span.end);
            *tail_text = trimmed;
            *tail_span = SourceSpan::new(
                tail_span.source_id,
                start,
                start.saturating_add(tail_text.len()),
            );
        }
    }
    let tail_span = tail
        .first()
        .map(inline_source_span)
        .unwrap_or(paragraph_span);
    Some((key, tail, String::new(), tail_span))
}

fn plain_dictionary_key(content: &[IrInline]) -> Option<String> {
    let [IrInline::Text { content, .. }] = content else {
        return None;
    };
    let key = content.trim();
    (!key.is_empty() && !key.contains(':')).then(|| key.to_string())
}

fn dictionary_scalar_value(text: &str) -> IrValue {
    let text = text.trim();
    if text.len() >= 2 {
        let bytes = text.as_bytes();
        if (bytes[0] == b'"' && bytes[text.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[text.len() - 1] == b'\'')
        {
            return IrValue::String(text[1..text.len() - 1].to_string());
        }
    }
    match text.to_ascii_lowercase().as_str() {
        "true" | "yes" => IrValue::Boolean(true),
        "false" | "no" => IrValue::Boolean(false),
        _ => text
            .parse::<f64>()
            .map_or_else(|_| IrValue::String(text.to_string()), IrValue::Number),
    }
}

fn dictionary_inline_value(inlines: Vec<IrInline>, span: SourceSpan) -> IrValue {
    match inlines.as_slice() {
        [IrInline::DirectiveCall {
            name,
            positional_args,
            named_args,
            body,
            span,
        }] => IrValue::Content(vec![IrNode::FunctionCall {
            name: name.clone(),
            positional_args: positional_args.clone(),
            named_args: named_args.clone(),
            lambda_parameters: None,
            body: body.as_ref().map(|body| {
                vec![IrNode::Paragraph {
                    content: body.clone(),
                    span: *span,
                }]
            }),
            span: *span,
        }]),
        [IrInline::ChainedDirectiveCall {
            head,
            chain,
            body,
            span,
        }] => IrValue::Content(vec![IrNode::ChainedFunctionCall {
            head: head.clone(),
            chain: chain.clone(),
            body: body.as_ref().map(|body| {
                vec![IrNode::Paragraph {
                    content: body.clone(),
                    span: *span,
                }]
            }),
            span: *span,
        }]),
        _ => IrValue::Content(vec![IrNode::Paragraph {
            content: inlines,
            span,
        }]),
    }
}

fn inline_source_span(inline: &IrInline) -> SourceSpan {
    match inline {
        IrInline::Text { span, .. }
        | IrInline::Emphasis { span, .. }
        | IrInline::Strong { span, .. }
        | IrInline::Strikethrough { span, .. }
        | IrInline::DirectiveCall { span, .. }
        | IrInline::ChainedDirectiveCall { span, .. }
        | IrInline::Link { span, .. }
        | IrInline::Image { span, .. }
        | IrInline::Code { span, .. }
        | IrInline::SoftBreak { span }
        | IrInline::HardBreak { span } => *span,
    }
}

/// Returns true for the conditional constructs this evaluator resolves.
fn is_conditional(name: &str) -> bool {
    name == "if" || name == "ifnot"
}

/// Returns true for the scoped `.let` semantic form.
fn is_let(name: &str) -> bool {
    name == "let"
}

fn is_foreach(name: &str) -> bool {
    name == "foreach"
}

fn is_repeat(name: &str) -> bool {
    name == "repeat"
}

fn is_pair(name: &str) -> bool {
    name == "pair"
}

fn is_dictionary(name: &str) -> bool {
    name == "dictionary"
}

fn is_collection_access(name: &str) -> bool {
    matches!(name, "size" | "first" | "last" | "getat")
}

fn collection_access_operand(
    name: &str,
    named_parameter: &str,
    positional_args: &[IrValue],
    named_args: &[IrNamedArg],
    span: &SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<IrValue, CallOutcome> {
    if positional_args.len() > 1 {
        diagnostics.push(iteration_error(
            format!(
                "`.{name}` requires exactly one iterable argument (received {})",
                positional_args.len()
            ),
            *span,
        ));
        return Err(CallOutcome::Failed);
    }

    if let Some(argument) = named_args
        .iter()
        .find(|argument| argument.name != named_parameter)
    {
        diagnostics.push(iteration_error_at(
            format!("Unknown named argument `{}` for `.{name}`", argument.name),
            argument.name_span,
        ));
        return Err(CallOutcome::Failed);
    }
    if let Some(argument) = named_args.get(1) {
        diagnostics.push(iteration_error_at(
            format!("`.{name}` received iterable argument more than once"),
            argument.name_span,
        ));
        return Err(CallOutcome::Failed);
    }
    match (positional_args.first(), named_args.first()) {
        (Some(_), Some(argument)) => {
            diagnostics.push(iteration_error_at(
                format!("`.{name}` received iterable argument more than once"),
                argument.name_span,
            ));
            Err(CallOutcome::Failed)
        }
        (Some(value), None) => Ok(value.clone()),
        (None, Some(argument)) => Ok(argument.value.clone()),
        (None, None) => {
            diagnostics.push(iteration_error(
                format!("`.{name}` requires exactly one iterable argument"),
                *span,
            ));
            Err(CallOutcome::Failed)
        }
    }
}

fn getat_operands(
    positional_args: &[IrValue],
    named_args: &[IrNamedArg],
    span: &SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<(IrValue, IrValue, IrValue), CallOutcome> {
    if positional_args.len() > 2 {
        diagnostics.push(iteration_error(
            format!(
                "`.getat` accepts an iterable and an index (received {} positional arguments)",
                positional_args.len()
            ),
            *span,
        ));
        return Err(CallOutcome::Failed);
    }

    let mut collection = positional_args.first().cloned();
    let mut index = positional_args.get(1).cloned();
    let mut fallback = None;
    for argument in named_args {
        match argument.name.as_str() {
            "from" => {
                if collection.is_some() {
                    diagnostics.push(iteration_error_at(
                        "`.getat` received the iterable argument more than once".to_string(),
                        argument.name_span,
                    ));
                    return Err(CallOutcome::Failed);
                }
                collection = Some(argument.value.clone());
            }
            "index" => {
                if index.is_some() {
                    diagnostics.push(iteration_error_at(
                        "`.getat` received the index argument more than once".to_string(),
                        argument.name_span,
                    ));
                    return Err(CallOutcome::Failed);
                }
                index = Some(argument.value.clone());
            }
            "orelse" => {
                if fallback.is_some() {
                    diagnostics.push(iteration_error_at(
                        "`.getat` received the `orelse` argument more than once".to_string(),
                        argument.name_span,
                    ));
                    return Err(CallOutcome::Failed);
                }
                fallback = Some(argument.value.clone());
            }
            _ => {
                diagnostics.push(iteration_error_at(
                    format!("Unknown named argument `{}` for `.getat`", argument.name),
                    argument.name_span,
                ));
                return Err(CallOutcome::Failed);
            }
        }
    }

    let Some(collection) = collection else {
        diagnostics.push(iteration_error(
            "`.getat` requires an iterable argument".to_string(),
            *span,
        ));
        return Err(CallOutcome::Failed);
    };
    let Some(index) = index else {
        diagnostics.push(iteration_error(
            "`.getat` requires an integer index".to_string(),
            *span,
        ));
        return Err(CallOutcome::Failed);
    };
    Ok((collection, index, fallback.unwrap_or(IrValue::None)))
}

fn exact_collection_length(
    length: usize,
    span: &SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<f64, CallOutcome> {
    const MAX_EXACT_F64_INTEGER: u64 = 1 << 53;
    let Ok(length) = u64::try_from(length) else {
        diagnostics.push(iteration_error(
            "Collection length cannot be represented by the evaluator Number type".to_string(),
            *span,
        ));
        return Err(CallOutcome::Failed);
    };
    if length > MAX_EXACT_F64_INTEGER {
        diagnostics.push(iteration_error(
            "Collection length cannot be represented exactly by the evaluator Number type"
                .to_string(),
            *span,
        ));
        return Err(CallOutcome::Failed);
    }
    Ok(length as f64)
}

fn collection_index(
    value: &IrValue,
    length: f64,
    span: &SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<Option<usize>, CallOutcome> {
    let IrValue::Number(index) = value else {
        diagnostics.push(iteration_error(
            "`.getat` requires an integer numeric index".to_string(),
            *span,
        ));
        return Err(CallOutcome::Failed);
    };
    if !index.is_finite() || index.fract() != 0.0 {
        diagnostics.push(iteration_error(
            "`.getat` requires a finite integer numeric index".to_string(),
            *span,
        ));
        return Err(CallOutcome::Failed);
    }

    // Quarkdown accepts Int values here, but Kotlin's getOrNull makes zero,
    // negative, and values beyond the finite collection bounds ordinary
    // misses. Check the bounds before converting so an f64 cannot truncate or
    // saturate into a valid Rust index.
    if *index < 1.0 || *index > length {
        return Ok(None);
    }
    let zero_based = (*index - 1.0) as u64;
    let Ok(zero_based) = usize::try_from(zero_based) else {
        diagnostics.push(iteration_error(
            "`.getat` index cannot be represented by this target".to_string(),
            *span,
        ));
        return Err(CallOutcome::Failed);
    };
    Ok(Some(zero_based))
}

fn validate_iteration_lambda(
    parameters: Option<&[IrParameter]>,
    name: &str,
    allow_destructuring: bool,
    span: &SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    if let Some(parameters) = parameters {
        let valid = if allow_destructuring {
            matches!(parameters.len(), 1 | 2)
        } else {
            parameters.len() == 1
        };
        if !valid {
            let parameter_span = parameters
                .get(1)
                .or_else(|| parameters.first())
                .map(|parameter| parameter.span)
                .unwrap_or(*span);
            diagnostics.push(iteration_error_at(
                format!(
                    "`.{name}` requires one explicit parameter{}",
                    if allow_destructuring {
                        " or exactly two parameters for Pair destructuring"
                    } else {
                        ""
                    }
                ),
                parameter_span,
            ));
            return false;
        }
    }
    true
}

fn scoped_parameter_bindings(
    value: &IrValue,
    parameters: &[IrParameter],
    allow_destructuring: bool,
    span: SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<Vec<(String, IrValue)>, CallOutcome> {
    match parameters {
        [parameter] => Ok(vec![(parameter.name.clone(), value.clone())]),
        [first, second] if allow_destructuring => {
            let IrValue::Pair(pair) = value else {
                diagnostics.push(iteration_error(
                    format!(
                        "Cannot destructure `.foreach` item as `{}` and `{}`: expected a Pair",
                        first.name, second.name
                    ),
                    value_source_span(value, &span),
                ));
                return Err(CallOutcome::Failed);
            };
            Ok(vec![
                (first.name.clone(), (*pair.first).clone()),
                (second.name.clone(), (*pair.second).clone()),
            ])
        }
        _ => {
            diagnostics.push(iteration_error(
                "Unsupported scoped lambda parameter pattern".to_string(),
                span,
            ));
            Err(CallOutcome::Failed)
        }
    }
}

fn repeat_count(value: &IrValue) -> Result<u64, String> {
    let IrValue::Number(number) = value else {
        return Err("`.repeat` requires a semantic Number count".to_string());
    };
    if !number.is_finite() {
        return Err("`.repeat` count must be finite".to_string());
    }
    if *number < 0.0 {
        return Err("`.repeat` count must not be negative".to_string());
    }
    if number.fract() != 0.0 {
        return Err("`.repeat` count must be an integer".to_string());
    }
    if *number == 0.0 {
        return Ok(0);
    }
    number
        .to_string()
        .parse::<u64>()
        .map_err(|_| "`.repeat` count is outside the supported integer range".to_string())
}

/// Parses the numeric part of a parser-preserved implicit parameter call.
///
/// The frontend already enforces the token boundary and rejects `.0`/leading
/// zero spellings. This checked conversion keeps oversized decimal indices
/// deterministic instead of allowing an integer conversion panic.
fn implicit_parameter_index(name: &str) -> Option<ImplicitParameterIndex> {
    let bytes = name.as_bytes();
    if bytes.is_empty() || bytes[0] == b'0' || !bytes.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let mut index = 0usize;
    for &byte in bytes {
        let digit = usize::from(byte - b'0');
        let Some(next) = index
            .checked_mul(10)
            .and_then(|value| value.checked_add(digit))
        else {
            return Some(ImplicitParameterIndex::Overflow);
        };
        index = next;
    }
    Some(ImplicitParameterIndex::Valid(index))
}

fn implicit_parameter_error(
    name: &str,
    error: ImplicitParameterError,
    span: SourceSpan,
) -> Diagnostic {
    let message = match error {
        ImplicitParameterError::Missing => {
            format!("Implicit lambda parameter `.{name}` is not bound for this invocation")
        }
        ImplicitParameterError::Overflow => {
            format!("Implicit lambda parameter `.{name}` is too large for this evaluator")
        }
    };
    Diagnostic {
        code: "E3003".to_string(),
        severity: Severity::Error,
        message,
        primary: Some(span),
        secondary: Vec::new(),
        hints: vec![
            "Provide the positional argument required by the implicit lambda parameter."
                .to_string(),
        ],
    }
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

fn let_error(message: String, span: SourceSpan) -> Diagnostic {
    let mut diagnostic = function_error_at(message, span);
    diagnostic.hints =
        vec!["`.let` requires one value argument and a block lambda body.".to_string()];
    diagnostic
}

fn let_error_at(message: String, span: SourceSpan) -> Diagnostic {
    let mut diagnostic = let_error(message, span);
    diagnostic.primary = Some(span);
    diagnostic
}

fn value_source_span(value: &IrValue, fallback: &SourceSpan) -> SourceSpan {
    match value {
        IrValue::Pair(pair) => pair.span,
        IrValue::Dictionary(dictionary) => dictionary.span,
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

fn iteration_error(message: String, span: SourceSpan) -> Diagnostic {
    iteration_error_at(message, span)
}

fn iteration_error_at(message: String, span: SourceSpan) -> Diagnostic {
    Diagnostic {
        code: "E3001".to_string(),
        severity: Severity::Error,
        message,
        primary: Some(span),
        secondary: Vec::new(),
        hints: vec![
            "Iteration values remain typed; unsupported or invalid iteration is not fabricated as text."
                .to_string(),
        ],
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
        IrValue::None | IrValue::Pair(_) | IrValue::Dictionary(_) => None,
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
        IrValue::None | IrValue::Pair(_) | IrValue::Dictionary(_) => None,
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
///
/// Range and Collection are semantic values, not scalar text. Reaching this
/// helper with either variant is an explicit materialization failure rather
/// than an empty-string fallback.
fn scalar_to_text(
    value: &IrValue,
    span: SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<String, CallOutcome> {
    match value {
        IrValue::String(text) => Ok(text.clone()),
        IrValue::Number(number) => Ok(number.to_string()),
        IrValue::Boolean(boolean) => Ok(boolean.to_string()),
        IrValue::Identifier(name) => Ok(name.clone()),
        IrValue::None => Ok("None".to_string()),
        IrValue::Content(_) => {
            diagnostics.push(iteration_error(
                "Rich content cannot be rendered as scalar text".to_string(),
                span,
            ));
            Err(CallOutcome::Failed)
        }
        IrValue::Range(range) => {
            diagnostics.push(iteration_error(
                "Direct Range materialization is deferred; consume the typed Range through iteration first"
                    .to_string(),
                range.span,
            ));
            Err(CallOutcome::Failed)
        }
        IrValue::Collection(_) | IrValue::Pair(_) | IrValue::Dictionary(_) => {
            diagnostics.push(iteration_error(
                "Collection, Pair, or Dictionary cannot be rendered as scalar text".to_string(),
                span,
            ));
            Err(CallOutcome::Failed)
        }
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
        let lambda_parameters = if parameters.is_empty() {
            LambdaParameters::Implicit
        } else {
            LambdaParameters::Explicit(parameters.to_vec())
        };
        context.set_function_binding(
            function_name.clone(),
            lambda_parameters,
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
            let outcome = match body {
                CallBody::Block(nodes) => {
                    self.evaluate_callable_body_value(nodes, diagnostics, context)
                }
                CallBody::Inline(inlines) => {
                    self.evaluate_call_body(CallBody::Inline(inlines), span, diagnostics, context)
                }
            };
            match outcome {
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
            lambda_parameters: None,
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
            lambda_parameters: None,
            body: None,
            span: span(0, 1),
        }])
    }

    fn lambda_parameter(name: &str, start: usize) -> IrParameter {
        IrParameter {
            name: name.to_string(),
            name_span: span(start, start + name.len()),
            span: span(start, start + name.len() + 1),
            optional: false,
        }
    }

    fn let_call(
        value: Option<IrValue>,
        lambda_parameters: Option<Vec<IrParameter>>,
        body: Option<Vec<IrNode>>,
    ) -> IrNode {
        IrNode::FunctionCall {
            name: "let".to_string(),
            positional_args: value.into_iter().collect(),
            named_args: Vec::new(),
            lambda_parameters,
            body,
            span: span(0, 10),
        }
    }

    fn let_value(
        value: IrValue,
        lambda_parameters: Option<Vec<IrParameter>>,
        body: Vec<IrNode>,
    ) -> IrValue {
        IrValue::Content(vec![let_call(Some(value), lambda_parameters, Some(body))])
    }

    fn foreach_call(
        value: IrValue,
        lambda_parameters: Option<Vec<IrParameter>>,
        body: Vec<IrNode>,
    ) -> IrNode {
        IrNode::FunctionCall {
            name: "foreach".to_string(),
            positional_args: vec![value],
            named_args: Vec::new(),
            lambda_parameters,
            body: Some(body),
            span: span(0, 20),
        }
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
    fn let_explicit_parameter_returns_scalar() {
        let nodes = evaluate(vec![let_call(
            Some(IrValue::Number(5.0)),
            Some(vec![lambda_parameter("n", 20)]),
            Some(vec![var_ref("n")]),
        )]);
        assert_paragraph_text(&nodes, "5");
    }

    #[test]
    fn let_implicit_parameter_returns_scalar() {
        let nodes = evaluate(vec![let_call(
            Some(IrValue::String("Quarkdown".to_string())),
            None,
            Some(vec![var_ref("1")]),
        )]);
        assert_paragraph_text(&nodes, "Quarkdown");
    }

    #[test]
    fn let_preserves_scalar_result_in_nested_value_context() {
        let outer = IrNode::FunctionCall {
            name: "multiply".to_string(),
            positional_args: vec![
                let_value(
                    IrValue::Number(5.0),
                    Some(vec![lambda_parameter("n", 20)]),
                    vec![var_ref("n")],
                ),
                IrValue::Number(2.0),
            ],
            named_args: Vec::new(),
            lambda_parameters: None,
            body: None,
            span: span(0, 20),
        };
        let nodes = evaluate(vec![outer]);
        assert_paragraph_text(&nodes, "10");
    }

    #[test]
    fn let_returns_structured_content_and_composes_in_source_order() {
        let nodes = evaluate(vec![let_call(
            Some(IrValue::String("value".to_string())),
            Some(vec![lambda_parameter("name", 20)]),
            Some(vec![text_paragraph("First"), text_paragraph("Second")]),
        )]);
        assert_eq!(
            nodes,
            vec![text_paragraph("First"), text_paragraph("Second")]
        );
    }

    #[test]
    fn let_reads_parent_variable_and_function() {
        let declaration = IrNode::FunctionDeclaration {
            name: IrValue::Identifier("decorate".to_string()),
            parameters: vec![lambda_parameter("value", 5)],
            body: vec![IrNode::FunctionCall {
                name: "uppercase".to_string(),
                positional_args: vec![call_value("value", Vec::new())],
                named_args: Vec::new(),
                lambda_parameters: None,
                body: None,
                span: span(0, 1),
            }],
            span: span(0, 1),
        };
        let nodes = evaluate(vec![
            var_declaration("prefix", IrValue::String("Hello".to_string())),
            declaration,
            let_call(
                Some(IrValue::String("world".to_string())),
                Some(vec![lambda_parameter("name", 20)]),
                Some(vec![
                    var_ref("prefix"),
                    IrNode::FunctionCall {
                        name: "decorate".to_string(),
                        positional_args: vec![call_value("name", Vec::new())],
                        named_args: Vec::new(),
                        lambda_parameters: None,
                        body: None,
                        span: span(0, 1),
                    },
                ]),
            ),
        ]);
        assert_eq!(nodes.len(), 2);
        assert_paragraph_text(&nodes[..1], "Hello");
        assert_paragraph_text(&nodes[1..], "WORLD");
    }

    #[test]
    fn let_shadows_parent_and_local_variables_do_not_leak() {
        let nodes = evaluate(vec![
            var_declaration("name", IrValue::String("outer".to_string())),
            let_call(
                Some(IrValue::String("inner".to_string())),
                Some(vec![lambda_parameter("name", 20)]),
                Some(vec![var_ref("name")]),
            ),
            var_ref("name"),
        ]);
        assert_eq!(nodes.len(), 2);
        assert_paragraph_text(&nodes[..1], "inner");
        assert_paragraph_text(&nodes[1..], "outer");

        let nodes = evaluate(vec![
            var_declaration("x", IrValue::String("outer".to_string())),
            let_call(
                Some(IrValue::String("inner".to_string())),
                Some(vec![lambda_parameter("value", 20)]),
                Some(vec![
                    var_declaration("x", IrValue::String("local".to_string())),
                    var_ref("x"),
                ]),
            ),
            var_ref("x"),
        ]);
        assert_eq!(nodes.len(), 2);
        assert_paragraph_text(&nodes[0..1], "local");
        assert_paragraph_text(&nodes[1..2], "outer");
    }

    #[test]
    fn let_local_function_does_not_leak() {
        let local = IrNode::FunctionDeclaration {
            name: IrValue::Identifier("local".to_string()),
            parameters: Vec::new(),
            body: vec![text_paragraph("inside")],
            span: span(30, 35),
        };
        let (nodes, diagnostics) = evaluate_with_diagnostics(vec![let_call(
            Some(IrValue::String("hello".to_string())),
            Some(vec![lambda_parameter("value", 20)]),
            Some(vec![local]),
        )]);
        assert!(nodes.is_empty());
        assert!(diagnostics.is_empty());

        let (nodes, diagnostics) = evaluate_with_diagnostics(vec![
            let_call(
                Some(IrValue::String("hello".to_string())),
                Some(vec![lambda_parameter("value", 20)]),
                Some(vec![IrNode::FunctionDeclaration {
                    name: IrValue::Identifier("local".to_string()),
                    parameters: Vec::new(),
                    body: vec![text_paragraph("inside")],
                    span: span(30, 35),
                }]),
            ),
            var_ref("local"),
        ]);
        assert!(diagnostics.is_empty());
        let [IrNode::FunctionCall { name, .. }] = nodes.as_slice() else {
            panic!("expected unresolved local function reference, got {nodes:?}")
        };
        assert_eq!(name, "local");
    }

    #[test]
    fn nested_let_uses_nearest_implicit_scope() {
        let nested = let_call(
            Some(IrValue::Content(vec![var_ref("1")])),
            None,
            Some(vec![var_ref("1")]),
        );
        let nodes = evaluate(vec![let_call(
            Some(IrValue::String("outer".to_string())),
            None,
            Some(vec![nested]),
        )]);
        assert_paragraph_text(&nodes, "outer");
    }

    #[test]
    fn explicit_let_masks_outer_implicit_scope() {
        let nested = let_call(
            Some(IrValue::String("inner".to_string())),
            Some(vec![lambda_parameter("value", 40)]),
            Some(vec![var_ref("1")]),
        );
        let (nodes, diagnostics) = evaluate_with_diagnostics(vec![let_call(
            Some(IrValue::String("outer".to_string())),
            None,
            Some(vec![nested]),
        )]);
        assert!(nodes.is_empty());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E3003");
    }

    #[test]
    fn foreach_returns_a_typed_collection_before_output_materialization() {
        let evaluator = Evaluator::new();
        let range = IrValue::Range(IrRange {
            start: Some(2),
            end: Some(4),
            span: span(0, 5),
        });
        let body = vec![var_ref("n")];
        let parameters = vec![lambda_parameter("n", 10)];
        let mut diagnostics = Vec::new();
        let mut context = EvaluationContext::new();
        let outcome = evaluator.evaluate_call_value(
            "foreach",
            &[range],
            &[],
            Some(CallBody::Block(&body)),
            Some(&parameters),
            &span(0, 10),
            &mut diagnostics,
            &mut context,
        );
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert!(matches!(
            outcome,
            CallOutcome::Value(IrValue::Collection(values))
                if values == vec![
                    IrValue::Number(2.0),
                    IrValue::Number(3.0),
                    IrValue::Number(4.0),
                ]
        ));
    }

    #[test]
    fn collection_access_operations_preserve_recursive_types_and_dictionary_pairs() {
        let evaluator = Evaluator::new();
        let pair = IrValue::Pair(IrPair {
            first: Box::new(IrValue::String("key".to_string())),
            second: Box::new(IrValue::Boolean(true)),
            span: span(10, 20),
        });
        let dictionary = IrValue::Dictionary(IrDictionary {
            entries: vec![IrPair {
                first: Box::new(IrValue::String("first".to_string())),
                second: Box::new(IrValue::Collection(vec![IrValue::Number(2.0)])),
                span: span(21, 30),
            }],
            span: span(21, 30),
        });
        let collection = IrValue::Collection(vec![
            IrValue::Number(1.0),
            IrValue::Content(vec![text_paragraph("content")]),
            pair.clone(),
            dictionary.clone(),
        ]);
        let operation_span = span(0, 40);

        let mut diagnostics = Vec::new();
        let mut context = EvaluationContext::new();
        let outcome = evaluator.evaluate_call_value(
            "size",
            &[],
            &[named_arg("of", collection.clone())],
            None,
            None,
            &operation_span,
            &mut diagnostics,
            &mut context,
        );
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert!(matches!(outcome, CallOutcome::Value(IrValue::Number(4.0))));

        let outcome = evaluator.evaluate_call_value(
            "first",
            std::slice::from_ref(&collection),
            &[],
            None,
            None,
            &operation_span,
            &mut diagnostics,
            &mut context,
        );
        assert!(matches!(outcome, CallOutcome::Value(IrValue::Number(1.0))));

        let outcome = evaluator.evaluate_call_value(
            "getat",
            &[collection.clone(), IrValue::Number(2.0)],
            &[],
            None,
            None,
            &operation_span,
            &mut diagnostics,
            &mut context,
        );
        assert!(matches!(
            outcome,
            CallOutcome::Value(IrValue::Content(nodes))
                if nodes == vec![text_paragraph("content")]
        ));

        let outcome = evaluator.evaluate_call_value(
            "last",
            &[collection],
            &[],
            None,
            None,
            &operation_span,
            &mut diagnostics,
            &mut context,
        );
        assert!(matches!(outcome, CallOutcome::Value(value) if value == dictionary));

        let outcome = evaluator.evaluate_call_value(
            "getat",
            &[
                IrValue::Dictionary(IrDictionary {
                    entries: vec![IrPair {
                        first: Box::new(IrValue::String("a".to_string())),
                        second: Box::new(IrValue::Number(1.0)),
                        span: span(41, 45),
                    }],
                    span: span(41, 45),
                }),
                IrValue::Number(1.0),
            ],
            &[],
            None,
            None,
            &operation_span,
            &mut diagnostics,
            &mut context,
        );
        let CallOutcome::Value(entry) = outcome else {
            panic!("expected a typed dictionary Pair")
        };
        assert!(matches!(entry, IrValue::Pair(_)));

        let outcome = evaluator.evaluate_call_value(
            "first",
            &[entry],
            &[],
            None,
            None,
            &operation_span,
            &mut diagnostics,
            &mut context,
        );
        assert!(matches!(
            outcome,
            CallOutcome::Value(IrValue::String(value)) if value == "a"
        ));
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn collection_access_indexing_matches_one_based_empty_and_invalid_boundaries() {
        let evaluator = Evaluator::new();
        let values = IrValue::Collection(vec![
            IrValue::String("first".to_string()),
            IrValue::String("second".to_string()),
        ]);
        let operation_span = span(0, 20);

        for index in [0.0, -1.0, 3.0, 9_007_199_254_740_992.0] {
            let mut diagnostics = Vec::new();
            let mut context = EvaluationContext::new();
            let outcome = evaluator.evaluate_call_value(
                "getat",
                &[values.clone(), IrValue::Number(index)],
                &[],
                None,
                None,
                &operation_span,
                &mut diagnostics,
                &mut context,
            );
            assert!(matches!(outcome, CallOutcome::Value(IrValue::None)));
            assert!(diagnostics.is_empty(), "{diagnostics:?}");
        }

        let empty = IrValue::Range(IrRange {
            start: Some(4),
            end: Some(2),
            span: operation_span,
        });
        for name in ["first", "last"] {
            let mut diagnostics = Vec::new();
            let mut context = EvaluationContext::new();
            let outcome = evaluator.evaluate_call_value(
                name,
                std::slice::from_ref(&empty),
                &[],
                None,
                None,
                &operation_span,
                &mut diagnostics,
                &mut context,
            );
            assert!(matches!(outcome, CallOutcome::Value(IrValue::None)));
            assert!(diagnostics.is_empty(), "{diagnostics:?}");
        }

        let mut diagnostics = Vec::new();
        let mut context = EvaluationContext::new();
        let outcome = evaluator.evaluate_call_value(
            "getat",
            &[empty, IrValue::Number(1.0)],
            &[named_arg("orelse", IrValue::Boolean(true))],
            None,
            None,
            &operation_span,
            &mut diagnostics,
            &mut context,
        );
        assert!(matches!(
            outcome,
            CallOutcome::Value(IrValue::Boolean(true))
        ));
        assert!(diagnostics.is_empty(), "{diagnostics:?}");

        for index in [1.5, f64::NAN, f64::INFINITY] {
            let mut diagnostics = Vec::new();
            let mut context = EvaluationContext::new();
            let outcome = evaluator.evaluate_call_value(
                "getat",
                &[values.clone(), IrValue::Number(index)],
                &[],
                None,
                None,
                &operation_span,
                &mut diagnostics,
                &mut context,
            );
            assert!(matches!(outcome, CallOutcome::Failed));
            assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        }
    }

    #[test]
    fn collection_access_reuses_failure_outcomes_and_checks_length_conversion() {
        let evaluator = Evaluator::new();
        let operation_span = span(0, 20);
        let failing = call_value(
            "multiply",
            vec![IrValue::Boolean(true), IrValue::Number(2.0)],
        );
        let unresolved = call_value("unknown", Vec::new());

        for value in [failing, unresolved, IrValue::Boolean(true)] {
            let mut diagnostics = Vec::new();
            let mut context = EvaluationContext::new();
            let outcome = evaluator.evaluate_call_value(
                "size",
                &[value],
                &[],
                None,
                None,
                &operation_span,
                &mut diagnostics,
                &mut context,
            );
            assert!(matches!(outcome, CallOutcome::Failed));
            assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        }

        let mut diagnostics = Vec::new();
        assert!(matches!(
            exact_collection_length(usize::MAX, &operation_span, &mut diagnostics),
            Err(CallOutcome::Failed)
        ));
        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn pair_evaluation_is_typed_recursive_and_atomic_on_child_failure() {
        let evaluator = Evaluator::new();
        let pair_span = span(10, 20);
        let mut diagnostics = Vec::new();
        let mut context = EvaluationContext::new();
        let outcome = evaluator.evaluate_call_value(
            "pair",
            &[
                IrValue::String("key".to_string()),
                IrValue::Collection(vec![IrValue::Number(1.0), IrValue::Boolean(true)]),
            ],
            &[],
            None,
            None,
            &pair_span,
            &mut diagnostics,
            &mut context,
        );
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert!(matches!(
            outcome,
            CallOutcome::Value(IrValue::Pair(IrPair { first, second, span }))
                if *first == IrValue::String("key".to_string())
                    && *second == IrValue::Collection(vec![
                        IrValue::Number(1.0),
                        IrValue::Boolean(true),
                    ])
                    && span == pair_span
        ));

        let failing = call_value(
            "multiply",
            vec![IrValue::Boolean(true), IrValue::Number(2.0)],
        );
        diagnostics.clear();
        let outcome = evaluator.evaluate_call_value(
            "pair",
            &[IrValue::Number(1.0), failing],
            &[],
            None,
            None,
            &pair_span,
            &mut diagnostics,
            &mut context,
        );
        assert!(matches!(outcome, CallOutcome::Failed));
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    }

    #[test]
    fn dictionary_iteration_reuses_pair_items_and_explicit_destructuring() {
        let evaluator = Evaluator::new();
        let dictionary_span = span(0, 30);
        let dictionary = IrValue::Dictionary(IrDictionary {
            entries: vec![
                IrPair {
                    first: Box::new(IrValue::String("a".to_string())),
                    second: Box::new(IrValue::Number(1.0)),
                    span: span(5, 10),
                },
                IrPair {
                    first: Box::new(IrValue::String("b".to_string())),
                    second: Box::new(IrValue::Number(2.0)),
                    span: span(10, 15),
                },
            ],
            span: dictionary_span,
        });
        let parameters = vec![lambda_parameter("key", 20), lambda_parameter("value", 24)];
        let body = vec![var_ref("key"), var_ref("value")];
        let mut diagnostics = Vec::new();
        let mut context = EvaluationContext::new();
        let outcome = evaluator.evaluate_call_value(
            "foreach",
            &[dictionary],
            &[],
            Some(CallBody::Block(&body)),
            Some(&parameters),
            &dictionary_span,
            &mut diagnostics,
            &mut context,
        );
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let CallOutcome::Value(IrValue::Collection(values)) = outcome else {
            panic!("expected typed iteration result")
        };
        assert_eq!(values.len(), 2);
        assert!(matches!(
            &values[0],
            IrValue::Content(nodes) if nodes.len() == 2
        ));
        assert!(matches!(
            &values[1],
            IrValue::Content(nodes) if nodes.len() == 2
        ));
    }

    #[test]
    fn pair_destructuring_rejects_non_pair_items_without_coercion() {
        let evaluator = Evaluator::new();
        let parameters = vec![lambda_parameter("key", 20), lambda_parameter("value", 24)];
        let body = vec![var_ref("key")];
        let mut diagnostics = Vec::new();
        let mut context = EvaluationContext::new();
        let outcome = evaluator.evaluate_call_value(
            "foreach",
            &[IrValue::Collection(vec![IrValue::String(
                "invalid".to_string(),
            )])],
            &[],
            Some(CallBody::Block(&body)),
            Some(&parameters),
            &span(0, 20),
            &mut diagnostics,
            &mut context,
        );
        assert!(matches!(outcome, CallOutcome::Failed));
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert!(diagnostics[0].message.contains("expected a Pair"));
    }

    #[test]
    fn block_materialization_of_mixed_collection_is_fail_fast_and_atomic() {
        let range_span = span(10, 14);
        let value = IrValue::Collection(vec![
            IrValue::Number(1.0),
            IrValue::Range(IrRange {
                start: Some(2),
                end: Some(4),
                span: range_span,
            }),
            IrValue::Number(5.0),
        ]);
        let mut diagnostics = Vec::new();
        let result =
            Evaluator::new().materialize_block_value(value, &span(0, 20), &mut diagnostics);
        assert!(matches!(result, Err(CallOutcome::Failed)));
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].primary, Some(range_span));
    }

    #[test]
    fn block_materialization_of_nested_range_is_fail_fast_and_atomic() {
        let range_span = span(10, 14);
        let value = IrValue::Collection(vec![IrValue::Collection(vec![IrValue::Range(IrRange {
            start: Some(2),
            end: Some(4),
            span: range_span,
        })])]);
        let mut diagnostics = Vec::new();
        let result =
            Evaluator::new().materialize_block_value(value, &span(0, 20), &mut diagnostics);
        assert!(matches!(result, Err(CallOutcome::Failed)));
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].primary, Some(range_span));
    }

    #[test]
    fn block_materialization_of_normal_collection_preserves_order() {
        let value = IrValue::Collection(vec![
            IrValue::Number(1.0),
            IrValue::Number(2.0),
            IrValue::Number(3.0),
        ]);
        let mut diagnostics = Vec::new();
        let nodes =
            match Evaluator::new().materialize_block_value(value, &span(0, 20), &mut diagnostics) {
                Ok(nodes) => nodes,
                Err(_) => panic!("normal Collection should materialize"),
            };
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_eq!(nodes.len(), 3);
        for (node, expected) in nodes.iter().zip(["1", "2", "3"]) {
            let IrNode::Paragraph { content, .. } = node else {
                panic!("expected scalar paragraph, got {node:?}")
            };
            assert!(matches!(
                content.as_slice(),
                [IrInline::Text { content, .. }] if content == expected
            ));
        }
    }

    #[test]
    fn foreach_empty_collection_does_not_invoke_the_body() {
        let (nodes, diagnostics) = evaluate_with_diagnostics(vec![foreach_call(
            IrValue::Collection(Vec::new()),
            None,
            vec![var_ref("2")],
        )]);
        assert!(nodes.is_empty());
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn foreach_nested_iterable_expression_flows_through_one_value_context() {
        let nested = foreach_call(
            IrValue::Range(IrRange {
                start: Some(1),
                end: Some(2),
                span: span(0, 4),
            }),
            None,
            vec![var_ref("1")],
        );
        let outer = foreach_call(IrValue::Content(vec![nested]), None, vec![var_ref("1")]);
        let (nodes, diagnostics) = evaluate_with_diagnostics(vec![outer]);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_paragraph_text(&nodes[0..1], "1");
        assert_paragraph_text(&nodes[1..2], "2");
    }

    #[test]
    fn foreach_local_function_does_not_leak_to_parent() {
        let local = IrNode::FunctionDeclaration {
            name: IrValue::Identifier("local".to_string()),
            parameters: Vec::new(),
            body: vec![text_paragraph("inside")],
            span: span(20, 25),
        };
        let foreach = IrNode::FunctionCall {
            name: "foreach".to_string(),
            positional_args: vec![IrValue::Range(IrRange {
                start: Some(1),
                end: Some(2),
                span: span(0, 4),
            })],
            named_args: Vec::new(),
            lambda_parameters: Some(vec![lambda_parameter("n", 10)]),
            body: Some(vec![local, var_ref("n")]),
            span: span(0, 20),
        };
        let (nodes, diagnostics) = evaluate_with_diagnostics(vec![foreach, var_ref("local")]);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert!(matches!(
            nodes.last(),
            Some(IrNode::FunctionCall { name, .. }) if name == "local"
        ));
    }

    #[test]
    fn let_missing_implicit_parameter_reports_original_span() {
        let (nodes, diagnostics) = evaluate_with_diagnostics(vec![let_call(
            Some(IrValue::String("value".to_string())),
            None,
            Some(vec![var_ref("2")]),
        )]);
        assert!(nodes.is_empty());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E3003");
        assert_eq!(diagnostics[0].primary, Some(span(0, 1)));
    }

    #[test]
    fn let_arity_and_value_errors_are_deterministic() {
        let (nodes, diagnostics) = evaluate_with_diagnostics(vec![let_call(
            None,
            Some(vec![lambda_parameter("value", 20)]),
            Some(vec![var_ref("value")]),
        )]);
        assert!(nodes.is_empty());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].primary, Some(span(0, 10)));

        let call = IrNode::FunctionCall {
            name: "let".to_string(),
            positional_args: vec![IrValue::Number(1.0), IrValue::Number(2.0)],
            named_args: Vec::new(),
            lambda_parameters: Some(vec![lambda_parameter("value", 20)]),
            body: Some(vec![var_ref("value")]),
            span: span(0, 10),
        };
        let (nodes, diagnostics) = evaluate_with_diagnostics(vec![call]);
        assert!(nodes.is_empty());
        assert_eq!(diagnostics.len(), 1);

        let call = let_call(
            Some(IrValue::Number(1.0)),
            Some(vec![
                lambda_parameter("first", 20),
                lambda_parameter("second", 30),
            ]),
            Some(vec![var_ref("first")]),
        );
        let (nodes, diagnostics) = evaluate_with_diagnostics(vec![call]);
        assert!(nodes.is_empty());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].primary, Some(span(20, 26)));
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
            lambda_parameters: None,
            body: None,
            span: span(7, 12),
        }]);
        let outer = IrNode::FunctionCall {
            name: "multiply".to_string(),
            positional_args: vec![nested_reassignment, IrValue::Number(2.0)],
            named_args: Vec::new(),
            lambda_parameters: None,
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
            lambda_parameters: None,
            body: None,
            span: span(9, 14),
        }]);
        let outer = IrNode::FunctionCall {
            name: "multiply".to_string(),
            positional_args: vec![IrValue::Number(2.0)],
            named_args: vec![named_arg("by", nested_reassignment)],
            lambda_parameters: None,
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
            lambda_parameters: None,
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
            lambda_parameters: None,
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
            lambda_parameters: None,
            body: None,
            span: span(7, 18),
        }]);
        let outer = IrNode::FunctionCall {
            name: "multiply".to_string(),
            positional_args: vec![invalid_var, IrValue::Number(2.0)],
            named_args: Vec::new(),
            lambda_parameters: None,
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
            lambda_parameters: None,
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
            lambda_parameters: None,
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
                lambda_parameters: None,
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
                .and_then(|binding| binding.parameters.explicit())
                .map(|parameters| parameters
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
            child
                .get_function("future")
                .and_then(|binding| binding.parameters.explicit())
                .map(|parameters| parameters
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
            child
                .get_function("inherited")
                .and_then(|binding| binding.parameters.explicit())
                .map(|parameters| parameters
                    .iter()
                    .map(|p| p.name.as_str())
                    .collect::<Vec<_>>()),
            Some(vec!["shadowed"])
        );
        assert_eq!(
            parent
                .get_function("inherited")
                .and_then(|binding| binding.parameters.explicit())
                .map(|parameters| parameters
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
            lambda_parameters: None,
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
            lambda_parameters: None,
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
            lambda_parameters: None,
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
            lambda_parameters: None,
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
                title: None,
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
            title: _title,
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
            lambda_parameters: None,
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
            lambda_parameters: None,
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
            lambda_parameters: None,
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
            lambda_parameters: None,
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
                lambda_parameters: None,
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
            lambda_parameters: None,
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
            lambda_parameters: None,
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
            lambda_parameters: None,
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
            lambda_parameters: None,
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
            lambda_parameters: None,
            body: None,
            span: span(0, 1),
        }
    }

    fn var_declaration_with_body(name: &str, body_nodes: Vec<IrNode>) -> IrNode {
        IrNode::FunctionCall {
            name: "var".to_string(),
            positional_args: vec![IrValue::Identifier(name.to_string())],
            named_args: Vec::new(),
            lambda_parameters: None,
            body: Some(body_nodes),
            span: span(0, 1),
        }
    }

    fn var_ref(name: &str) -> IrNode {
        IrNode::FunctionCall {
            name: name.to_string(),
            positional_args: Vec::new(),
            named_args: Vec::new(),
            lambda_parameters: None,
            body: None,
            span: span(0, 1),
        }
    }

    fn var_reassignment(name: &str, value: IrValue) -> IrNode {
        IrNode::FunctionCall {
            name: name.to_string(),
            positional_args: vec![value],
            named_args: Vec::new(),
            lambda_parameters: None,
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
                lambda_parameters: None,
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
                lambda_parameters: None,
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
                lambda_parameters: None,
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
                lambda_parameters: None,
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
            lambda_parameters: None,
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
            lambda_parameters: None,
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
                lambda_parameters: None,
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
            lambda_parameters: None,
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
            lambda_parameters: None,
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
