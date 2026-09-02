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

use crate::invocation_binder::{
    self, BindingPlan, BodyPolicy, BoundSlot, Candidate, ParameterMetadata,
};
use crate::value_conversion::{
    self, InvocationNamedArg, InvocationValue, ScalarTarget, ScalarValue, ValueOrigin,
};
use crate::{ast_to_ir, builtins};
use crate::{
    Capabilities, Capability, EvaluationLimits, IncludedSource, ResourceAccessError,
    ResourceProvider, ResourceText,
};
use arkst_diagnostics::{Diagnostic, Severity};
use arkst_ir::{
    IrCallArgument, IrCallSegment, IrCallable, IrCallableCapture, IrCaptionPositionInfo,
    IrCapturedFunction, IrCapturedVariable, IrComponent, IrContainerAlignment,
    IrContainerComponent, IrCrossAxisAlignment, IrDictionary, IrDocument, IrDocumentAuthor,
    IrDocumentTheme, IrEnumValue, IrInline, IrInlineBody, IrLandscapeComponent, IrListItem,
    IrMainAxisAlignment, IrNamedArg, IrNode, IrPair, IrParameter, IrRange, IrRawBody, IrSize,
    IrSizeUnit, IrStackedComponent, IrStackedLayout, IrTableAlignment, IrTableCell, IrTableRow,
    IrValue, NativeTarget, TargetSpecificContent,
};
use arkst_markdown::Mode;
use arkst_quarkdown::is_valid_normal_call_name;
use arkst_source::{SourceId, SourceSpan};
use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU32;
use std::rc::{Rc, Weak};

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

/// A source-backed callable definition stored in an evaluator scope.
#[derive(Debug, Clone, PartialEq)]
struct FunctionBinding {
    parameters: LambdaParameters,
    body: Vec<IrNode>,
    declaration_span: SourceSpan,
    capture: Option<Box<IrCallableCapture>>,
    extension: Option<Rc<FunctionExtension>>,
}

impl FunctionBinding {
    fn as_callable(&self) -> IrCallable {
        IrCallable {
            parameters: self.parameters.to_ir(),
            body: self.body.clone(),
            span: self.declaration_span,
            capture: self.capture.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ExtensionId(u64);

/// The evaluator-owned link used by a function extension. The wrapper body
/// remains an ordinary `FunctionBinding`; only its immutable declaration-time
/// parent target is extra runtime state. Scope-local chain overlays live on
/// `EvaluationContext`, so chained calls share callable identity without
/// mutating a link that another invocation may still own.
#[derive(Debug, Clone, PartialEq)]
struct FunctionExtension {
    id: ExtensionId,
    condition: Option<IrCallable>,
    super_target: FunctionTarget,
    body_policy: ExtensionBodyPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExtensionBodyPolicy {
    Reject,
    BindRaw,
    BindEvaluatedContent,
    AllowSeparate,
}

impl ExtensionBodyPolicy {
    fn binder_policy(self) -> BodyPolicy {
        match self {
            Self::Reject => BodyPolicy::Reject,
            Self::BindRaw | Self::BindEvaluatedContent => BodyPolicy::BindFinal,
            Self::AllowSeparate => BodyPolicy::AllowSeparate,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum FunctionTarget {
    Binding(Rc<FunctionBinding>),
    Native(String),
}

/// A scope-local successor for an extension link. The stable ID is the map
/// identity; the weak owner prevents an unrelated link from being accepted if
/// a corrupted or mixed context presents the same ID. The target stays strong
/// while the link is live so chained wrappers remain callable. Replaced root
/// bindings explicitly retire their reachable overlays, which breaks that
/// otherwise unbounded retention chain.
#[derive(Debug, Clone)]
struct ExtensionOverlay {
    owner: Weak<FunctionExtension>,
    target: FunctionTarget,
}

/// Source-backed call data retained while an extension body evaluates. The
/// body is cloned once into an invocation-local `Rc`; it is never reparsed or
/// used as a transaction snapshot. This lets `.super` preserve the original
/// caller body without evaluating it before a conditional extension selects
/// a branch.
#[derive(Clone)]
enum OwnedCallBody {
    Block(Rc<[IrNode]>),
    Inline(Rc<[IrInline]>),
}

#[derive(Clone)]
struct ExtensionInvocation {
    target: FunctionTarget,
    parameters: LambdaParameters,
    forwarded: Vec<Candidate<InvocationValue>>,
    body: Option<OwnedCallBody>,
    raw_body: Option<IrRawBody>,
}

/// Controls whether an ordinary callable invocation receives the active
/// extension's dynamic `.super` binding. An extension invocation supplies a
/// replacement binding explicitly; only condition lambdas suppress the
/// inherited binding, matching their separate upstream invocation context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExtensionContextMode {
    Inherit,
    Suppress,
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

    fn to_ir(&self) -> Option<Vec<IrParameter>> {
        match self {
            Self::Explicit(parameters) => Some(parameters.clone()),
            Self::Implicit => None,
        }
    }

    fn from_ir(parameters: Option<Vec<IrParameter>>) -> Self {
        parameters.map_or(Self::Implicit, Self::Explicit)
    }

    fn last_name(&self) -> Option<&IrParameter> {
        match self {
            Self::Explicit(parameters) => parameters.last(),
            Self::Implicit => None,
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

enum IterationBody<'a> {
    Block(&'a [IrNode]),
    Inline(IrCallable),
}

#[derive(Clone)]
struct StackedArgument {
    value: InvocationValue,
    span: SourceSpan,
    parameter_span: Option<SourceSpan>,
}

struct BoundStackedArguments {
    values: Vec<Option<StackedArgument>>,
}

#[derive(Clone)]
struct AlignArgument {
    value: InvocationValue,
    span: SourceSpan,
    parameter_span: Option<SourceSpan>,
}

#[derive(Clone)]
struct ContainerArgument {
    value: InvocationValue,
    span: SourceSpan,
    parameter_span: Option<SourceSpan>,
}

#[derive(Clone)]
struct WhitespaceArgument {
    value: InvocationValue,
    span: SourceSpan,
    parameter_span: Option<SourceSpan>,
}

struct BoundContainerArguments {
    width: Option<ContainerArgument>,
    height: Option<ContainerArgument>,
    full_width: Option<ContainerArgument>,
}

struct BoundWhitespaceArguments {
    width: Option<WhitespaceArgument>,
    height: Option<WhitespaceArgument>,
}

impl BoundStackedArguments {
    fn take(&mut self, index: usize) -> Option<StackedArgument> {
        self.values.get_mut(index).and_then(Option::take)
    }
}

#[derive(Clone, Copy)]
struct IterationOptions {
    span: SourceSpan,
    allow_destructuring: bool,
}

#[derive(Debug, Clone, PartialEq)]
enum SortKey {
    Number(f64),
    String(String),
    Boolean(bool),
}

impl SortKey {
    fn try_from_value(value: &IrValue) -> Result<Self, String> {
        match value {
            IrValue::Number(value) => Ok(Self::Number(*value)),
            IrValue::String(value) => Ok(Self::String(value.clone())),
            IrValue::Boolean(value) => Ok(Self::Boolean(*value)),
            IrValue::None => Err("`.sorted` cannot compare a None value".to_string()),
            _ => Err("`.sorted` key has no supported natural ordering".to_string()),
        }
    }

    fn same_kind(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::Number(_), Self::Number(_))
                | (Self::String(_), Self::String(_))
                | (Self::Boolean(_), Self::Boolean(_))
        )
    }
}

impl Eq for SortKey {}

impl Ord for SortKey {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Number(left), Self::Number(right)) => match (left.is_nan(), right.is_nan()) {
                (true, true) => Ordering::Equal,
                (true, false) => Ordering::Greater,
                (false, true) => Ordering::Less,
                (false, false) => left.total_cmp(right),
            },
            (Self::String(left), Self::String(right)) => left.cmp(right),
            (Self::Boolean(left), Self::Boolean(right)) => left.cmp(right),
            _ => Ordering::Equal,
        }
    }
}

impl PartialOrd for SortKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Result of invoking a call in value context.
///
/// `Unresolved` is distinct from an empty content value: an ordinary output
/// context may preserve it, while a chain must reject it because it cannot
/// inject a fabricated intermediate value.
#[derive(Debug, PartialEq)]
enum CallOutcome {
    Value(IrValue),
    NoValue,
    Failed,
    Unresolved,
}

/// Mutable evaluator-only document state. Its final form is copied into the
/// serializable IR snapshot after evaluation completes.
#[derive(Debug, Clone, Default)]
struct DocumentState {
    name: String,
    description: String,
    document_type: arkst_ir::IrDocumentType,
    authors: Vec<IrDocumentAuthor>,
    keywords: Vec<String>,
    theme: Option<IrDocumentTheme>,
    locale: Option<arkst_ir::IrDocumentLocale>,
    caption_position: IrCaptionPositionInfo,
}

impl DocumentState {
    fn from_snapshot(snapshot: &arkst_ir::IrDocumentState) -> Self {
        Self {
            name: snapshot.name.clone(),
            description: snapshot.description.clone(),
            document_type: snapshot.document_type,
            authors: snapshot.authors.clone(),
            keywords: snapshot.keywords.clone(),
            theme: snapshot.theme.clone(),
            locale: snapshot.locale.clone(),
            caption_position: snapshot.caption_position,
        }
    }

    fn snapshot(&self) -> arkst_ir::IrDocumentState {
        arkst_ir::IrDocumentState {
            name: self.name.clone(),
            description: self.description.clone(),
            document_type: self.document_type,
            authors: self.authors.clone(),
            keywords: self.keywords.clone(),
            theme: self.theme.clone(),
            locale: self.locale.clone(),
            caption_position: self.caption_position,
        }
    }
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
        IrValue::Component(component) => Ok(vec![IrNode::Component { component }]),
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
        | IrNode::RawHtml { span, .. }
        | IrNode::FunctionCall { span, .. }
        | IrNode::ChainedFunctionCall { span, .. }
        | IrNode::FunctionDeclaration { span, .. }
        | IrNode::ThematicBreak { span }
        | IrNode::Math { span, .. } => *span,
        IrNode::TargetSpecificContent { content } => content.span,
        IrNode::Component { component } => component.span(),
    }
}

#[derive(Debug, Default)]
struct EvaluationRuntime {
    active_evaluation_depth: usize,
}

/// Releases one active evaluator frame even when evaluation returns early.
struct EvaluationDepthGuard {
    runtime: Rc<RefCell<EvaluationRuntime>>,
}

impl Drop for EvaluationDepthGuard {
    fn drop(&mut self) {
        let mut runtime = self.runtime.borrow_mut();
        debug_assert!(runtime.active_evaluation_depth > 0);
        runtime.active_evaluation_depth = runtime.active_evaluation_depth.saturating_sub(1);
    }
}

/// Evaluation context with explicit parent visibility and local bindings.
///
/// Created fresh per `evaluate()` call to ensure isolation and determinism.
/// Lookups walk the parent chain without cloning it. A child scope snapshots
/// the visible parent context at creation time; new local declarations stay in
/// the child, while successful writes to caller-visible variables are
/// published at the callable boundary. The snapshot is deliberate: a lambda
/// observes the bindings visible when it is entered, while new locals cannot
/// leak back.
struct EvaluationContext<'a> {
    parent: Option<Box<EvaluationContext<'a>>>,
    variables: BTreeMap<String, VariableValue>,
    functions: BTreeMap<String, Rc<FunctionBinding>>,
    extension_targets: BTreeMap<ExtensionId, ExtensionOverlay>,
    lambda_scope: Option<LambdaScope>,
    extension_invocation: Option<Rc<ExtensionInvocation>>,
    resources: Option<&'a dyn ResourceProvider>,
    metadata_defaults: crate::DocumentMetadataDefaults,
    current_source: Option<SourceId>,
    active_sources: Vec<SourceId>,
    document_state: Rc<RefCell<DocumentState>>,
    limits: EvaluationLimits,
    runtime: Rc<RefCell<EvaluationRuntime>>,
    invocation_depth: Rc<RefCell<usize>>,
    transaction: Rc<RefCell<InvocationTransaction>>,
    scope_identity: Rc<ScopeIdentity>,
    /// The savepoint at which this scope was created as an ephemeral scope.
    /// Writes made by that scope at its own savepoint are dead with the scope,
    /// while writes made from a deeper savepoint can still need rollback while
    /// this scope remains live.
    journal_floor: Option<usize>,
    assigned_variables: BTreeMap<String, IrValue>,
    variable_owners: BTreeSet<String>,
    forwarded_variable_owners: BTreeSet<String>,
    parameter_names: BTreeSet<String>,
}

struct ScopeIdentity {
    key: usize,
}

#[derive(Default)]
struct InvocationTransaction {
    savepoints: Vec<InvocationSavepoint>,
    next_scope_key: usize,
    next_extension_id: u64,
    #[cfg(test)]
    document_state_copy_work: usize,
}

#[derive(Default)]
struct InvocationSavepoint {
    entries: Vec<InvocationUndo>,
    first_writes: BTreeSet<UndoKey>,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum UndoKey {
    DocumentState {
        field: DocumentStateField,
    },
    Variable {
        scope: usize,
        name: String,
    },
    AssignedVariable {
        scope: usize,
        name: String,
    },
    VariableOwner {
        scope: usize,
        name: String,
    },
    Function {
        scope: usize,
        name: String,
    },
    ExtensionTarget {
        scope: usize,
        extension: ExtensionId,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum DocumentStateField {
    Name,
    Description,
    DocumentType,
    Authors,
    Keywords,
    Theme,
    Locale,
    CaptionPosition,
}

enum DocumentStateUndo {
    Name(String),
    Description(String),
    DocumentType(arkst_ir::IrDocumentType),
    AuthorsLen(usize),
    Keywords(Vec<String>),
    Theme(Option<IrDocumentTheme>),
    Locale(Option<arkst_ir::IrDocumentLocale>),
    CaptionPosition(IrCaptionPositionInfo),
}

enum InvocationUndo {
    DocumentState {
        field: DocumentStateField,
        previous: DocumentStateUndo,
    },
    Variable {
        scope: usize,
        name: String,
        journal_floor: Option<usize>,
        previous: Option<VariableValue>,
    },
    AssignedVariable {
        scope: usize,
        name: String,
        journal_floor: Option<usize>,
        previous: Option<IrValue>,
    },
    VariableOwner {
        scope: usize,
        name: String,
        journal_floor: Option<usize>,
        was_present: bool,
    },
    Function {
        scope: usize,
        name: String,
        journal_floor: Option<usize>,
        previous: Option<Rc<FunctionBinding>>,
    },
    ExtensionTarget {
        scope: usize,
        extension: ExtensionId,
        journal_floor: Option<usize>,
        previous: Option<ExtensionOverlay>,
    },
}

impl InvocationUndo {
    fn key(&self) -> UndoKey {
        match self {
            Self::DocumentState { field, .. } => UndoKey::DocumentState { field: *field },
            Self::Variable { scope, name, .. } => UndoKey::Variable {
                scope: *scope,
                name: name.clone(),
            },
            Self::AssignedVariable { scope, name, .. } => UndoKey::AssignedVariable {
                scope: *scope,
                name: name.clone(),
            },
            Self::VariableOwner { scope, name, .. } => UndoKey::VariableOwner {
                scope: *scope,
                name: name.clone(),
            },
            Self::Function { scope, name, .. } => UndoKey::Function {
                scope: *scope,
                name: name.clone(),
            },
            Self::ExtensionTarget {
                scope, extension, ..
            } => UndoKey::ExtensionTarget {
                scope: *scope,
                extension: *extension,
            },
        }
    }

    fn scope(&self) -> Option<usize> {
        match self {
            Self::DocumentState { .. } => None,
            Self::Variable { scope, .. }
            | Self::AssignedVariable { scope, .. }
            | Self::VariableOwner { scope, .. }
            | Self::Function { scope, .. } => Some(*scope),
            Self::ExtensionTarget { scope, .. } => Some(*scope),
        }
    }

    fn journal_floor(&self) -> Option<usize> {
        match self {
            Self::DocumentState { .. } => None,
            Self::Variable { journal_floor, .. }
            | Self::AssignedVariable { journal_floor, .. }
            | Self::VariableOwner { journal_floor, .. }
            | Self::Function { journal_floor, .. } => *journal_floor,
            Self::ExtensionTarget { journal_floor, .. } => *journal_floor,
        }
    }

    fn is_dead_at(&self, savepoint: usize) -> bool {
        self.journal_floor() == Some(savepoint)
    }
}

impl InvocationTransaction {
    fn allocate_scope_key(&mut self) -> usize {
        let key = self.next_scope_key;
        self.next_scope_key += 1;
        key
    }

    fn allocate_extension_id(&mut self) -> Option<ExtensionId> {
        let id = self.next_extension_id;
        self.next_extension_id = self.next_extension_id.checked_add(1)?;
        Some(ExtensionId(id))
    }

    fn begin(&mut self, outermost: bool) {
        debug_assert_eq!(self.savepoints.is_empty(), outermost);
        self.savepoints.push(InvocationSavepoint::default());
    }

    fn current_savepoint_index(&self) -> Option<usize> {
        self.savepoints.len().checked_sub(1)
    }

    fn first_write(&mut self, key: UndoKey) -> bool {
        self.savepoints
            .last_mut()
            .is_some_and(|savepoint| savepoint.first_writes.insert(key))
    }

    fn push(&mut self, entry: InvocationUndo) {
        if let Some(savepoint) = self.savepoints.last_mut() {
            savepoint.entries.push(entry);
        }
    }

    fn commit_savepoint(&mut self) {
        let Some(child) = self.savepoints.pop() else {
            debug_assert!(false, "invocation savepoint must be active");
            return;
        };
        let Some(parent_index) = self.savepoints.len().checked_sub(1) else {
            return;
        };
        let Some(parent) = self.savepoints.last_mut() else {
            return;
        };
        for entry in child.entries {
            if entry.is_dead_at(parent_index) {
                continue;
            }
            if parent.first_writes.insert(entry.key()) {
                parent.entries.push(entry);
            }
        }
    }

    fn rollback_savepoint(&mut self) -> Vec<InvocationUndo> {
        let Some(savepoint) = self.savepoints.pop() else {
            debug_assert!(false, "invocation savepoint must be active");
            return Vec::new();
        };
        savepoint.entries
    }

    #[cfg(test)]
    fn pending_entry_count(&self) -> usize {
        self.savepoints
            .iter()
            .map(|savepoint| savepoint.entries.len())
            .sum()
    }

    #[cfg(test)]
    fn record_document_state_copy_work(&mut self, units: usize) {
        self.document_state_copy_work = self.document_state_copy_work.saturating_add(units);
    }

    #[cfg(test)]
    fn document_state_copy_work(&self) -> usize {
        self.document_state_copy_work
    }
}

/// Evaluator-private invocation savepoint. Mutations are recorded only on the
/// first write in each savepoint, and nested success merges only writes not
/// already owned by the parent. Nested failure therefore restores just its
/// own mutations, while the outer invocation can continue successfully.
struct InvocationCheckpoint {}

impl InvocationCheckpoint {
    fn capture() -> Self {
        Self {}
    }

    fn restore(self, context: &mut EvaluationContext<'_>) {
        for entry in context.rollback_transaction().into_iter().rev() {
            context.restore_undo(entry);
        }
    }

    fn commit(self, context: &EvaluationContext<'_>) {
        context.commit_transaction();
    }
}

impl<'a> EvaluationContext<'a> {
    fn new() -> Self {
        Self::with_limits(EvaluationLimits::default())
    }

    fn with_limits(limits: EvaluationLimits) -> Self {
        let transaction = Rc::new(RefCell::new(InvocationTransaction::default()));
        let scope_identity = Self::new_scope_identity(&transaction);
        Self {
            parent: None,
            variables: BTreeMap::new(),
            functions: BTreeMap::new(),
            extension_targets: BTreeMap::new(),
            lambda_scope: None,
            extension_invocation: None,
            resources: None,
            metadata_defaults: crate::DocumentMetadataDefaults::default(),
            current_source: None,
            active_sources: Vec::new(),
            document_state: Rc::new(RefCell::new(DocumentState::default())),
            limits,
            runtime: Rc::new(RefCell::new(EvaluationRuntime::default())),
            invocation_depth: Rc::new(RefCell::new(0)),
            transaction,
            scope_identity,
            journal_floor: None,
            assigned_variables: BTreeMap::new(),
            variable_owners: BTreeSet::new(),
            forwarded_variable_owners: BTreeSet::new(),
            parameter_names: BTreeSet::new(),
        }
    }

    /// Creates a child scope with parent-visible bindings and isolated locals.
    #[allow(dead_code)]
    fn child(&self) -> Self {
        self.child_with_journal_floor(self.journal_floor)
    }

    /// Creates a scope whose local bindings cannot outlive this invocation.
    /// Caller-visible owner writes still travel to their real owner and are
    /// journaled there; only dead local bindings skip the transaction log.
    fn ephemeral_child(&self) -> Self {
        let journal_floor = self.transaction.borrow().current_savepoint_index();
        self.child_with_journal_floor(journal_floor)
    }

    fn child_with_journal_floor(&self, journal_floor: Option<usize>) -> Self {
        Self {
            parent: Some(Box::new(self.clone_scope_tree())),
            variables: BTreeMap::new(),
            functions: BTreeMap::new(),
            extension_targets: BTreeMap::new(),
            lambda_scope: None,
            extension_invocation: self.extension_invocation.clone(),
            resources: self.resources,
            metadata_defaults: self.metadata_defaults.clone(),
            current_source: self.current_source,
            active_sources: self.active_sources.clone(),
            document_state: Rc::clone(&self.document_state),
            limits: self.limits,
            runtime: Rc::clone(&self.runtime),
            invocation_depth: Rc::clone(&self.invocation_depth),
            transaction: Rc::clone(&self.transaction),
            scope_identity: Self::new_scope_identity(&self.transaction),
            journal_floor,
            assigned_variables: BTreeMap::new(),
            variable_owners: BTreeSet::new(),
            forwarded_variable_owners: BTreeSet::new(),
            parameter_names: BTreeSet::new(),
        }
    }

    fn with_resources(
        resources: &'a dyn ResourceProvider,
        source_id: SourceId,
        metadata_defaults: &crate::DocumentMetadataDefaults,
        limits: EvaluationLimits,
    ) -> Self {
        Self {
            resources: Some(resources),
            metadata_defaults: metadata_defaults.clone(),
            current_source: Some(source_id),
            active_sources: vec![source_id],
            ..Self::with_limits(limits)
        }
    }

    fn enter_evaluation_depth(
        &self,
        span: SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Result<EvaluationDepthGuard, CallOutcome> {
        let mut runtime = self.runtime.borrow_mut();
        if runtime.active_evaluation_depth >= self.limits.max_evaluation_depth {
            diagnostics.push(evaluation_depth_limit_error(
                self.limits.max_evaluation_depth,
                span,
            ));
            return Err(CallOutcome::Failed);
        }
        runtime.active_evaluation_depth += 1;
        drop(runtime);
        Ok(EvaluationDepthGuard {
            runtime: Rc::clone(&self.runtime),
        })
    }

    fn begin_invocation(&self) {
        let mut depth = self.invocation_depth.borrow_mut();
        let outermost = *depth == 0;
        self.transaction.borrow_mut().begin(outermost);
        *depth += 1;
    }

    fn end_invocation(&self) {
        let mut depth = self.invocation_depth.borrow_mut();
        debug_assert!(*depth > 0);
        *depth = depth.saturating_sub(1);
    }

    fn commit_transaction(&self) {
        self.transaction.borrow_mut().commit_savepoint();
    }

    fn rollback_transaction(&mut self) -> Vec<InvocationUndo> {
        self.transaction.borrow_mut().rollback_savepoint()
    }

    fn clone_scope_tree(&self) -> Self {
        Self {
            parent: self
                .parent
                .as_deref()
                .map(|parent| Box::new(parent.clone_scope_tree())),
            variables: self.variables.clone(),
            functions: self.functions.clone(),
            extension_targets: self.extension_targets.clone(),
            lambda_scope: self.lambda_scope.clone(),
            extension_invocation: self.extension_invocation.clone(),
            resources: self.resources,
            metadata_defaults: self.metadata_defaults.clone(),
            current_source: self.current_source,
            active_sources: self.active_sources.clone(),
            document_state: Rc::clone(&self.document_state),
            limits: self.limits,
            runtime: Rc::clone(&self.runtime),
            invocation_depth: Rc::clone(&self.invocation_depth),
            transaction: Rc::clone(&self.transaction),
            scope_identity: Self::new_scope_identity(&self.transaction),
            journal_floor: self.journal_floor,
            assigned_variables: self.assigned_variables.clone(),
            variable_owners: self.variable_owners.clone(),
            forwarded_variable_owners: self.forwarded_variable_owners.clone(),
            parameter_names: self.parameter_names.clone(),
        }
    }

    fn scope_key(&self) -> usize {
        self.scope_identity.key
    }

    #[cfg(test)]
    fn extension_target_count(&self) -> usize {
        self.extension_targets.len()
    }

    fn new_scope_identity(transaction: &Rc<RefCell<InvocationTransaction>>) -> Rc<ScopeIdentity> {
        let key = transaction.borrow_mut().allocate_scope_key();
        Rc::new(ScopeIdentity { key })
    }

    fn allocate_extension_id(&self) -> Option<ExtensionId> {
        self.transaction.borrow_mut().allocate_extension_id()
    }

    fn should_journal_scope_write(&self) -> bool {
        let Some(floor) = self.journal_floor else {
            return true;
        };
        self.transaction
            .borrow()
            .current_savepoint_index()
            .is_some_and(|current| current > floor)
    }

    fn record_variable_before(&self, name: &str) {
        if !self.should_journal_scope_write() {
            return;
        }
        let scope = self.scope_key();
        let journal_floor = self.journal_floor;
        let key = UndoKey::Variable {
            scope,
            name: name.to_string(),
        };
        if self.transaction.borrow_mut().first_write(key) {
            let previous = self.variables.get(name).cloned();
            self.transaction
                .borrow_mut()
                .push(InvocationUndo::Variable {
                    scope,
                    name: name.to_string(),
                    journal_floor,
                    previous,
                });
        }
    }

    fn record_assigned_variable_before(&self, name: &str) {
        if !self.should_journal_scope_write() {
            return;
        }
        let scope = self.scope_key();
        let journal_floor = self.journal_floor;
        let key = UndoKey::AssignedVariable {
            scope,
            name: name.to_string(),
        };
        if self.transaction.borrow_mut().first_write(key) {
            let previous = self.assigned_variables.get(name).cloned();
            self.transaction
                .borrow_mut()
                .push(InvocationUndo::AssignedVariable {
                    scope,
                    name: name.to_string(),
                    journal_floor,
                    previous,
                });
        }
    }

    fn record_variable_owner_before(&self, name: &str) {
        if !self.should_journal_scope_write() {
            return;
        }
        let scope = self.scope_key();
        let journal_floor = self.journal_floor;
        let key = UndoKey::VariableOwner {
            scope,
            name: name.to_string(),
        };
        if self.transaction.borrow_mut().first_write(key) {
            let was_present = self.variable_owners.contains(name);
            self.transaction
                .borrow_mut()
                .push(InvocationUndo::VariableOwner {
                    scope,
                    name: name.to_string(),
                    journal_floor,
                    was_present,
                });
        }
    }

    fn record_function_before(&self, name: &str) {
        if !self.should_journal_scope_write() {
            return;
        }
        let scope = self.scope_key();
        let journal_floor = self.journal_floor;
        let key = UndoKey::Function {
            scope,
            name: name.to_string(),
        };
        if self.transaction.borrow_mut().first_write(key) {
            let previous = self.functions.get(name).cloned();
            self.transaction
                .borrow_mut()
                .push(InvocationUndo::Function {
                    scope,
                    name: name.to_string(),
                    journal_floor,
                    previous,
                });
        }
    }

    fn get_extension_target(&self, extension: &Rc<FunctionExtension>) -> FunctionTarget {
        let key = extension.id;
        self.extension_targets
            .get(&key)
            .and_then(|overlay| {
                overlay
                    .owner
                    .upgrade()
                    .filter(|owner| Rc::ptr_eq(owner, extension))
                    .map(|_| overlay.target.clone())
            })
            .or_else(|| {
                self.parent
                    .as_deref()
                    .map(|parent| parent.get_extension_target(extension))
            })
            .unwrap_or_else(|| extension.super_target.clone())
    }

    fn record_extension_target_before_id(&self, extension: ExtensionId) {
        if !self.should_journal_scope_write() {
            return;
        }
        let scope = self.scope_key();
        let journal_floor = self.journal_floor;
        let key = UndoKey::ExtensionTarget { scope, extension };
        if self.transaction.borrow_mut().first_write(key) {
            let previous = self.extension_targets.get(&extension).cloned();
            self.transaction
                .borrow_mut()
                .push(InvocationUndo::ExtensionTarget {
                    scope,
                    extension,
                    journal_floor,
                    previous,
                });
        }
    }

    fn record_extension_target_before(&self, extension: &Rc<FunctionExtension>) {
        self.record_extension_target_before_id(extension.id);
    }

    fn set_extension_target(&mut self, extension: &Rc<FunctionExtension>, target: FunctionTarget) {
        self.record_extension_target_before(extension);
        self.extension_targets.insert(
            extension.id,
            ExtensionOverlay {
                owner: Rc::downgrade(extension),
                target,
            },
        );
    }

    fn remove_extension_target(&mut self, extension: ExtensionId) {
        if self.extension_targets.contains_key(&extension) {
            self.record_extension_target_before_id(extension);
            self.extension_targets.remove(&extension);
        }
    }

    /// Retires the overlays that make an old root binding reach its chained
    /// wrappers. Function replacement is the lifecycle boundary: after it,
    /// the old root is no longer visible from this scope, so retaining those
    /// overlays would keep the old chain alive indefinitely.
    fn retire_extension_chain(&mut self, root: &Rc<FunctionBinding>) {
        let mut current = FunctionTarget::Binding(Rc::clone(root));
        let mut extension_ids = BTreeSet::new();
        while let FunctionTarget::Binding(binding) = current {
            let Some(extension) = binding.extension.as_ref() else {
                break;
            };
            if !extension_ids.insert(extension.id) {
                break;
            }
            current = self.get_extension_target(extension);
        }
        for extension in extension_ids {
            self.remove_extension_target(extension);
        }
    }

    fn restore_undo(&mut self, undo: InvocationUndo) -> bool {
        if let Some(scope) = undo.scope() {
            if self.scope_key() != scope {
                // Callable-local scope trees may be dropped before the outer
                // invocation rolls back. Those writes are already
                // unreachable; caller-visible scopes remain rooted at this
                // context and are restored by their stable scope identity.
                return self
                    .parent
                    .as_mut()
                    .is_some_and(|parent| parent.restore_undo(undo));
            }
        }
        match undo {
            InvocationUndo::DocumentState { previous, .. } => {
                self.restore_document_state_undo(previous);
            }
            InvocationUndo::Variable { name, previous, .. } => match previous {
                Some(previous) => {
                    self.variables.insert(name, previous);
                }
                None => {
                    self.variables.remove(&name);
                }
            },
            InvocationUndo::AssignedVariable { name, previous, .. } => match previous {
                Some(previous) => {
                    self.assigned_variables.insert(name, previous);
                }
                None => {
                    self.assigned_variables.remove(&name);
                }
            },
            InvocationUndo::VariableOwner {
                name, was_present, ..
            } => {
                if was_present {
                    self.variable_owners.insert(name);
                } else {
                    self.variable_owners.remove(&name);
                }
            }
            InvocationUndo::Function { name, previous, .. } => match previous {
                Some(previous) => {
                    self.functions.insert(name, previous);
                }
                None => {
                    self.functions.remove(&name);
                }
            },
            InvocationUndo::ExtensionTarget {
                extension,
                previous,
                ..
            } => match previous {
                Some(previous) => {
                    self.extension_targets.insert(extension, previous);
                }
                None => {
                    self.extension_targets.remove(&extension);
                }
            },
        }
        true
    }

    fn record_document_state_undo<F>(&self, field: DocumentStateField, previous: F)
    where
        F: FnOnce() -> (DocumentStateUndo, usize),
    {
        let key = UndoKey::DocumentState { field };
        if self.transaction.borrow_mut().first_write(key) {
            let (previous, copied_units) = previous();
            #[cfg(test)]
            self.transaction
                .borrow_mut()
                .record_document_state_copy_work(copied_units);
            #[cfg(not(test))]
            let _ = copied_units;
            self.transaction
                .borrow_mut()
                .push(InvocationUndo::DocumentState { field, previous });
        }
    }

    /// Declares or reassigns a variable from an evaluated IrValue, preserving content semantics.
    fn set_value(&mut self, name: String, value: IrValue) {
        self.record_variable_before(&name);
        self.variables
            .insert(name, VariableValue::from_evaluated_value(value));
    }

    /// Records a variable write made by semantic `.var` handling.
    ///
    /// Callable invocation uses this small write set to distinguish an
    /// existing caller owner that was reassigned from a declaration that
    /// should remain local to the invocation scope. Parameter and capture
    /// installation deliberately use `set_value` directly and are therefore
    /// not treated as user assignments.
    fn assign_value(&mut self, name: String, value: IrValue) {
        let has_forwarded_owner = self.has_forwarded_variable_owner(&name);
        if !self.assign_to_real_owner(&name, value.clone()) {
            // A lambda parameter is a lookup binding, not a variable owner.
            // Keep it shadowing a same-named `.var` declaration while still
            // recording the assignment for a real owner at the boundary.
            if !self.parameter_names.contains(&name) {
                self.set_value(name.clone(), value.clone());
            }
            if !has_forwarded_owner {
                self.record_variable_owner_before(&name);
                self.variable_owners.insert(name.clone());
            }
        }
        self.record_assigned_variable_before(&name);
        self.assigned_variables.insert(name, value);
    }

    fn assigned_values(&self) -> BTreeMap<String, IrValue> {
        self.assigned_variables.clone()
    }

    /// Publishes a successful nested callable assignment into an actual
    /// variable owner visible from the caller. Lookup-only bindings such as
    /// parameters, captures, and copied caller overlays are never owners by
    /// themselves. A forwarded overlay is updated as a relay so an outer
    /// callable boundary can continue the writeback to its real caller.
    fn apply_callable_assignment(&mut self, name: String, value: IrValue) -> bool {
        if !self.assign_to_owner(&name, value.clone()) {
            return false;
        }
        self.record_assigned_variable_before(&name);
        self.assigned_variables.insert(name, value);
        true
    }

    /// Assigns directly to the outermost real `.var` owner in this parent
    /// chain. This is used for ordinary child scopes, not copied callable
    /// overlays.
    fn assign_to_real_owner(&mut self, name: &str, value: IrValue) -> bool {
        if let Some(parent) = self.parent.as_mut() {
            if parent.assign_to_real_owner(name, value.clone()) {
                return true;
            }
        }
        if self.variable_owners.contains(name) {
            self.set_value(name.to_string(), value);
            return true;
        }
        false
    }

    /// Assigns to the outermost real or forwarded owner. Forwarded owners
    /// represent caller libraries copied into an invocation overlay; they are
    /// relays rather than proof that the overlay itself owns the variable.
    fn assign_to_owner(&mut self, name: &str, value: IrValue) -> bool {
        if let Some(parent) = self.parent.as_mut() {
            if parent.assign_to_owner(name, value.clone()) {
                return true;
            }
        }
        if self.variable_owners.contains(name) || self.forwarded_variable_owners.contains(name) {
            self.set_value(name.to_string(), value);
            return true;
        }
        false
    }

    fn has_forwarded_variable_owner(&self, name: &str) -> bool {
        self.forwarded_variable_owners.contains(name)
            || self
                .parent
                .as_deref()
                .is_some_and(|parent| parent.has_forwarded_variable_owner(name))
    }

    fn collect_variable_owners(&self, owners: &mut BTreeSet<String>) {
        if let Some(parent) = self.parent.as_deref() {
            parent.collect_variable_owners(owners);
        }
        owners.extend(self.variable_owners.iter().cloned());
        owners.extend(self.forwarded_variable_owners.iter().cloned());
    }

    /// Installs a user-function binding in the current local scope.
    fn set_function_binding(
        &mut self,
        name: String,
        parameters: LambdaParameters,
        body: Vec<IrNode>,
        declaration_span: SourceSpan,
        capture: Option<Box<IrCallableCapture>>,
    ) {
        let binding = Rc::new(FunctionBinding {
            parameters,
            body,
            declaration_span,
            capture,
            extension: None,
        });
        self.replace_function_binding(name, binding);
    }

    fn replace_function_binding(&mut self, name: String, binding: Rc<FunctionBinding>) {
        let previous = self.functions.get(&name).cloned();
        self.record_function_before(&name);
        if let Some(previous) = previous.as_ref() {
            self.retire_extension_chain(previous);
        }
        self.functions.insert(name, binding);
    }

    fn capture_snapshot(&self) -> IrCallableCapture {
        let mut variables = BTreeMap::new();
        let mut functions = BTreeMap::new();
        self.collect_bindings(&mut variables, &mut functions);
        IrCallableCapture {
            variables: variables
                .into_iter()
                .map(|(name, value)| IrCapturedVariable { name, value })
                .collect(),
            functions: functions
                .into_iter()
                .map(|(name, binding)| IrCapturedFunction {
                    name,
                    callable: binding.as_callable(),
                })
                .collect(),
        }
    }

    fn collect_bindings(
        &self,
        variables: &mut BTreeMap<String, IrValue>,
        functions: &mut BTreeMap<String, Rc<FunctionBinding>>,
    ) {
        if let Some(parent) = self.parent.as_deref() {
            parent.collect_bindings(variables, functions);
        }
        variables.extend(
            self.variables
                .iter()
                .map(|(name, value)| (name.clone(), value.to_value())),
        );
        functions.extend(self.functions.clone());
    }

    fn collect_extension_targets(&self, targets: &mut BTreeMap<ExtensionId, ExtensionOverlay>) {
        if let Some(parent) = self.parent.as_deref() {
            parent.collect_extension_targets(targets);
        }
        targets.extend(self.extension_targets.clone());
    }

    fn from_capture(capture: &IrCallableCapture) -> Self {
        let mut context = Self::new();
        for variable in &capture.variables {
            context.set_value(variable.name.clone(), variable.value.clone());
        }
        for function in &capture.functions {
            context.functions.insert(
                function.name.clone(),
                Rc::new(FunctionBinding {
                    parameters: LambdaParameters::from_ir(function.callable.parameters.clone()),
                    body: function.callable.body.clone(),
                    declaration_span: function.callable.span,
                    capture: function.callable.capture.clone(),
                    extension: None,
                }),
            );
        }
        context
    }

    /// Composes a callable's definition environment with the bindings visible
    /// at its call site. The definition context remains the parent layer, so
    /// caller-visible variables/functions supplement it without replacing the
    /// lexical capture or becoming part of that capture.
    fn with_caller_overlay(definition_context: Self, caller_context: &Self) -> Self {
        let mut variables = BTreeMap::new();
        let mut functions = BTreeMap::new();
        let mut extension_targets = BTreeMap::new();
        caller_context.collect_bindings(&mut variables, &mut functions);
        caller_context.collect_extension_targets(&mut extension_targets);
        let mut forwarded_variable_owners = BTreeSet::new();
        caller_context.collect_variable_owners(&mut forwarded_variable_owners);
        let journal_floor = caller_context
            .transaction
            .borrow()
            .current_savepoint_index();

        Self {
            parent: Some(Box::new(definition_context)),
            variables: variables
                .into_iter()
                .map(|(name, value)| (name, VariableValue::from_evaluated_value(value)))
                .collect(),
            functions,
            extension_targets,
            lambda_scope: caller_context.visible_lambda_scope(),
            extension_invocation: caller_context.extension_invocation.clone(),
            // Runtime/compiler state is intentionally not copied into this
            // lookup-only layer. Document state is the one explicit shared
            // exception required by the document-state contract.
            resources: None,
            metadata_defaults: Default::default(),
            current_source: None,
            active_sources: Vec::new(),
            document_state: Rc::clone(&caller_context.document_state),
            limits: caller_context.limits,
            runtime: Rc::clone(&caller_context.runtime),
            invocation_depth: Rc::clone(&caller_context.invocation_depth),
            transaction: Rc::clone(&caller_context.transaction),
            scope_identity: Self::new_scope_identity(&caller_context.transaction),
            journal_floor,
            assigned_variables: BTreeMap::new(),
            variable_owners: BTreeSet::new(),
            forwarded_variable_owners,
            parameter_names: BTreeSet::new(),
        }
    }

    #[cfg(test)]
    fn set_function(&mut self, name: String, parameters: Vec<String>) {
        let parameters = parameters
            .into_iter()
            .map(|name| IrParameter {
                name,
                name_span: SourceSpan::new(arkst_source::SourceId(0), 0, 0),
                span: SourceSpan::new(arkst_source::SourceId(0), 0, 0),
                optional: false,
            })
            .collect();
        self.set_function_binding(
            name,
            LambdaParameters::Explicit(parameters),
            Vec::new(),
            SourceSpan::new(arkst_source::SourceId(0), 0, 0),
            None,
        );
    }

    fn set_lambda_scope(&mut self, scope: LambdaScope) {
        self.lambda_scope = Some(scope);
    }

    fn initialize_document_state(&mut self, snapshot: &arkst_ir::IrDocumentState) {
        self.document_state = Rc::new(RefCell::new(DocumentState::from_snapshot(snapshot)));
    }

    fn document_state_snapshot(&self) -> arkst_ir::IrDocumentState {
        self.document_state.borrow().snapshot()
    }

    fn document_state_value(&self, name: &str) -> IrValue {
        let state = self.document_state.borrow();
        match name {
            "docname" => IrValue::String(state.name.clone()),
            "docdescription" => IrValue::String(state.description.clone()),
            "doctype" => IrValue::String(state.document_type.quarkdown_name().to_string()),
            "docauthor" => IrValue::String(
                state
                    .authors
                    .first()
                    .map(|author| author.name.clone())
                    .unwrap_or_default(),
            ),
            "dockeywords" => IrValue::Collection(
                state
                    .keywords
                    .iter()
                    .cloned()
                    .map(IrValue::String)
                    .collect(),
            ),
            "doclang" => IrValue::String(
                state
                    .locale
                    .as_ref()
                    .map(|locale| locale.localized_name.clone())
                    .unwrap_or_default(),
            ),
            _ => unreachable!("document state field must be validated by the caller"),
        }
    }

    fn document_authors_snapshot(&self) -> Vec<IrDocumentAuthor> {
        self.document_state.borrow().authors.clone()
    }

    fn restore_document_state_undo(&self, undo: DocumentStateUndo) {
        let mut state = self.document_state.borrow_mut();
        match undo {
            DocumentStateUndo::Name(previous) => state.name = previous,
            DocumentStateUndo::Description(previous) => state.description = previous,
            DocumentStateUndo::DocumentType(previous) => state.document_type = previous,
            DocumentStateUndo::AuthorsLen(previous) => state.authors.truncate(previous),
            DocumentStateUndo::Keywords(previous) => state.keywords = previous,
            DocumentStateUndo::Theme(previous) => state.theme = previous,
            DocumentStateUndo::Locale(previous) => state.locale = previous,
            DocumentStateUndo::CaptionPosition(previous) => state.caption_position = previous,
        }
    }

    fn set_document_state_value(&self, name: &str, value: String) {
        match name {
            "docname" => {
                self.record_document_state_undo(DocumentStateField::Name, || {
                    let previous = self.document_state.borrow().name.clone();
                    let copied_units = previous.len();
                    (DocumentStateUndo::Name(previous), copied_units)
                });
                self.document_state.borrow_mut().name = value;
            }
            "docdescription" => {
                self.record_document_state_undo(DocumentStateField::Description, || {
                    let previous = self.document_state.borrow().description.clone();
                    let copied_units = previous.len();
                    (DocumentStateUndo::Description(previous), copied_units)
                });
                self.document_state.borrow_mut().description = value;
            }
            _ => unreachable!("document state field must be validated by the caller"),
        }
    }

    fn set_document_type(&self, value: arkst_ir::IrDocumentType) {
        self.record_document_state_undo(DocumentStateField::DocumentType, || {
            (
                DocumentStateUndo::DocumentType(self.document_state.borrow().document_type),
                0,
            )
        });
        self.document_state.borrow_mut().document_type = value;
    }

    fn append_document_author(&self, name: String) {
        self.record_document_state_undo(DocumentStateField::Authors, || {
            (
                DocumentStateUndo::AuthorsLen(self.document_state.borrow().authors.len()),
                0,
            )
        });
        self.document_state
            .borrow_mut()
            .authors
            .push(IrDocumentAuthor {
                name,
                info: Vec::new(),
            });
    }

    fn replace_document_keywords(&self, keywords: Vec<String>) {
        let first_write = self
            .transaction
            .borrow_mut()
            .first_write(UndoKey::DocumentState {
                field: DocumentStateField::Keywords,
            });
        let previous = {
            let mut state = self.document_state.borrow_mut();
            if first_write {
                Some(std::mem::replace(&mut state.keywords, keywords))
            } else {
                state.keywords = keywords;
                None
            }
        };
        if let Some(previous) = previous {
            #[cfg(test)]
            self.transaction
                .borrow_mut()
                .record_document_state_copy_work(0);
            self.transaction
                .borrow_mut()
                .push(InvocationUndo::DocumentState {
                    field: DocumentStateField::Keywords,
                    previous: DocumentStateUndo::Keywords(previous),
                });
        }
    }

    fn set_document_theme(&self, theme: IrDocumentTheme) {
        self.record_document_state_undo(DocumentStateField::Theme, || {
            (
                DocumentStateUndo::Theme(self.document_state.borrow().theme.clone()),
                0,
            )
        });
        self.document_state.borrow_mut().theme = Some(theme);
    }

    fn set_document_locale(&self, locale: arkst_ir::IrDocumentLocale) {
        self.record_document_state_undo(DocumentStateField::Locale, || {
            (
                DocumentStateUndo::Locale(self.document_state.borrow().locale.clone()),
                0,
            )
        });
        self.document_state.borrow_mut().locale = Some(locale);
    }

    fn set_caption_position(&self, caption_position: IrCaptionPositionInfo) {
        self.record_document_state_undo(DocumentStateField::CaptionPosition, || {
            (
                DocumentStateUndo::CaptionPosition(self.document_state.borrow().caption_position),
                0,
            )
        });
        self.document_state.borrow_mut().caption_position = caption_position;
    }

    fn append_document_authors(&self, authors: Vec<IrDocumentAuthor>) -> Result<(), String> {
        self.record_document_state_undo(DocumentStateField::Authors, || {
            (
                DocumentStateUndo::AuthorsLen(self.document_state.borrow().authors.len()),
                0,
            )
        });
        let mut state = self.document_state.borrow_mut();
        state
            .authors
            .try_reserve(authors.len())
            .map_err(|error| format!("document authors cannot be allocated: {error}"))?;
        state.authors.extend(authors);
        Ok(())
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
    fn get_function(&self, name: &str) -> Option<&Rc<FunctionBinding>> {
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
            Some(LambdaScope::Implicit(arguments)) => match index {
                ImplicitParameterIndex::Valid(index) => {
                    let resolved = arguments
                        .get(index.saturating_sub(1))
                        .cloned()
                        .map(Ok)
                        .or_else(|| {
                            self.parent
                                .as_deref()
                                .and_then(|parent| parent.get_implicit_parameter(name))
                        });
                    Some(resolved.unwrap_or(Err(ImplicitParameterError::Missing)))
                }
                ImplicitParameterIndex::Overflow => Some(Err(ImplicitParameterError::Overflow)),
            },
            None => self
                .parent
                .as_deref()
                .and_then(|parent| parent.get_implicit_parameter(name)),
        }
    }

    /// Returns the nearest lambda scope that is visible from this context.
    /// The caller overlay copies this one scope as lookup state; it does not
    /// retain a reference to the mutable caller context.
    fn visible_lambda_scope(&self) -> Option<LambdaScope> {
        self.lambda_scope.clone().or_else(|| {
            self.parent
                .as_deref()
                .and_then(|parent| parent.visible_lambda_scope())
        })
    }
}

/// Evaluates Quarkdown conditionals, variables, user-defined functions, and
/// the currently supported semantic chain builtins in the IR.
#[derive(Debug, Clone, Copy)]
pub struct Evaluator {
    capabilities: Capabilities,
    limits: EvaluationLimits,
}

impl Default for Evaluator {
    fn default() -> Self {
        Self::new()
    }
}

impl Evaluator {
    /// Creates a new evaluator.
    pub fn new() -> Self {
        Self::with_capabilities_and_limits(Capabilities::default(), EvaluationLimits::default())
    }

    /// Creates an evaluator with the explicit capabilities for one compile.
    pub fn with_capabilities(capabilities: Capabilities) -> Self {
        Self::with_capabilities_and_limits(capabilities, EvaluationLimits::default())
    }

    /// Creates an evaluator with explicit semantic resource limits.
    pub fn with_limits(limits: EvaluationLimits) -> Self {
        Self::with_capabilities_and_limits(Capabilities::default(), limits)
    }

    /// Creates an evaluator with explicit capabilities and semantic resource
    /// limits for one compilation.
    pub fn with_capabilities_and_limits(
        capabilities: Capabilities,
        limits: EvaluationLimits,
    ) -> Self {
        Self {
            capabilities,
            limits,
        }
    }

    /// Evaluates the document, resolving conditionals, variables, and chains.
    ///
    /// Returns the resolved document and any evaluation diagnostics.
    pub fn evaluate(&self, document: &IrDocument) -> (IrDocument, Vec<Diagnostic>) {
        let mut diagnostics = Vec::new();
        let mut context = EvaluationContext::with_limits(self.limits);
        self.evaluate_with_context(document, &mut diagnostics, &mut context)
    }

    /// Evaluates an IR document with access to an explicit semantic resource
    /// provider. The provider is retained only for this evaluation; the
    /// engine performs no filesystem or network I/O.
    pub fn evaluate_project<R: ResourceProvider>(
        &self,
        resources: &R,
        source_id: SourceId,
        document: &IrDocument,
    ) -> (IrDocument, Vec<Diagnostic>) {
        let mut diagnostics = Vec::new();
        let mut context = EvaluationContext::with_resources(
            resources,
            source_id,
            &crate::DocumentMetadataDefaults::default(),
            self.limits,
        );
        self.evaluate_with_context(document, &mut diagnostics, &mut context)
    }

    /// Alias naming the engine-neutral input boundary explicitly.
    pub fn evaluate_with_resources<R: ResourceProvider>(
        &self,
        resources: &R,
        source_id: SourceId,
        document: &IrDocument,
        metadata_defaults: &crate::DocumentMetadataDefaults,
    ) -> (IrDocument, Vec<Diagnostic>) {
        let mut diagnostics = Vec::new();
        let mut context =
            EvaluationContext::with_resources(resources, source_id, metadata_defaults, self.limits);
        self.evaluate_with_context(document, &mut diagnostics, &mut context)
    }

    fn evaluate_with_context(
        &self,
        document: &IrDocument,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
    ) -> (IrDocument, Vec<Diagnostic>) {
        context.initialize_document_state(&document.metadata.document_state);
        let nodes = self.evaluate_nodes(&document.nodes, diagnostics, context);
        (
            IrDocument {
                nodes,
                metadata: arkst_ir::IrMetadata {
                    document_state: context.document_state_snapshot(),
                    ..document.metadata.clone()
                },
            },
            std::mem::take(diagnostics),
        )
    }

    /// Evaluates a list of block nodes, collecting any diagnostics.
    fn evaluate_nodes(
        &self,
        nodes: &[IrNode],
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
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
        context: &mut EvaluationContext<'_>,
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
                ordered_args,
                lambda_parameters,
                body,
                raw_body,
                span,
                ..
            } => match self.evaluate_block_call(
                name,
                ordered_args.as_deref(),
                positional_args,
                named_args,
                lambda_parameters.as_deref(),
                body.as_deref(),
                raw_body.as_ref(),
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
                raw_body,
                span,
            } => match self.evaluate_block_chain(
                head,
                chain,
                body,
                raw_body.as_ref(),
                span,
                diagnostics,
                context,
            ) {
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
                    .map(|item| arkst_ir::IrListItem {
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
                    .map(|item| arkst_ir::IrListItem {
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
                header: arkst_ir::IrTableRow {
                    cells: header
                        .cells
                        .iter()
                        .map(|cell| arkst_ir::IrTableCell {
                            content: self.evaluate_inlines(&cell.content, diagnostics, context),
                            alignment: cell.alignment,
                            span: cell.span,
                        })
                        .collect(),
                    span: header.span,
                },
                rows: rows
                    .iter()
                    .map(|row| arkst_ir::IrTableRow {
                        cells: row
                            .cells
                            .iter()
                            .map(|cell| arkst_ir::IrTableCell {
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
            IrNode::RawHtml { span, .. } => {
                diagnostics.push(unsupported_raw_html(*span));
                Vec::new()
            }
            IrNode::TargetSpecificContent { content } => {
                vec![IrNode::TargetSpecificContent {
                    content: content.clone(),
                }]
            }
            other => vec![other.clone()],
        }
    }

    /// Evaluates inline content, collecting any diagnostics.
    fn evaluate_inlines(
        &self,
        inlines: &[IrInline],
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
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
        context: &mut EvaluationContext<'_>,
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
                ordered_args,
                body,
                span,
                ..
            } => self.evaluate_inline_call(
                name,
                ordered_args.as_deref(),
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
            IrInline::RawHtml { span, .. } => {
                diagnostics.push(unsupported_raw_html(*span));
                Vec::new()
            }
            IrInline::TargetSpecificContent { content } => {
                vec![IrInline::TargetSpecificContent {
                    content: content.clone(),
                }]
            }
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
        ordered_args: Option<&[IrCallArgument]>,
        positional_args: &[IrValue],
        named_args: &[IrNamedArg],
        lambda_parameters: Option<&[IrParameter]>,
        body: Option<&[IrNode]>,
        raw_body: Option<&IrRawBody>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
    ) -> CallOutcome {
        match self.evaluate_call_value_with_ordered(
            name,
            ordered_args,
            positional_args,
            named_args,
            body.map(CallBody::Block),
            raw_body,
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
                ordered_args,
                positional_args,
                named_args,
                lambda_parameters,
                body,
                raw_body,
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
        ordered_args: Option<&[IrCallArgument]>,
        positional_args: &[IrValue],
        named_args: &[IrNamedArg],
        body: Option<&[IrInline]>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
    ) -> Vec<IrInline> {
        if is_stacked_layout(name) {
            diagnostics.push(stacked_inline_materialization_error(*span));
            return Vec::new();
        }
        if is_center(name) && context.get_function(name).is_none() {
            diagnostics.push(center_inline_materialization_error(*span));
            return Vec::new();
        }
        if is_align(name) && context.get_function(name).is_none() {
            diagnostics.push(align_inline_materialization_error(*span));
            return Vec::new();
        }
        if is_container(name) && context.get_function(name).is_none() {
            diagnostics.push(container_inline_materialization_error(*span));
            return Vec::new();
        }
        if is_landscape(name) && context.get_function(name).is_none() {
            diagnostics.push(landscape_inline_materialization_error(*span));
            return Vec::new();
        }
        match self.evaluate_call_value_with_ordered(
            name,
            ordered_args,
            positional_args,
            named_args,
            body.map(CallBody::Inline),
            None,
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
                    ordered_args,
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

    #[allow(clippy::too_many_arguments)]
    fn preflight_binding(
        &self,
        parameters: &[ParameterMetadata<'_>],
        ordered_args: Option<&[IrCallArgument]>,
        positional_args: &[IrValue],
        named_args: &[IrNamedArg],
        body: Option<CallBody<'_>>,
        body_policy: BodyPolicy,
        span: SourceSpan,
        diagnostic_code: &str,
        native_name: Option<&str>,
        lambda_body_span: Option<SourceSpan>,
        user_function: bool,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Result<BindingPlan, ()> {
        let candidates = structural_candidates(ordered_args, positional_args, named_args, span);
        let body = body.map(|body| body_candidate_shape(body, span));
        match invocation_binder::plan(parameters, &candidates, body.as_ref(), body_policy, span) {
            Ok(plan) => Ok(plan),
            Err(mut error) => {
                if let Some(name) = native_name {
                    if let Some(lambda_body_span) = lambda_body_span {
                        if error.message == "this invocation does not accept a body" {
                            error.message = format!("`.{name}` does not accept a lambda body");
                            error.primary = lambda_body_span;
                            error.hint =
                                format!("Remove the lambda body; `.{name}` does not accept one.");
                        }
                    }
                    if error.message.starts_with("missing required")
                        && matches!(
                            name,
                            "sum"
                                | "subtract"
                                | "multiply"
                                | "divide"
                                | "rem"
                                | "pow"
                                | "abs"
                                | "negate"
                                | "sqrt"
                                | "logn"
                                | "sin"
                                | "cos"
                                | "tan"
                                | "truncate"
                                | "round"
                                | "iseven"
                                | "islower"
                                | "isgreater"
                        )
                    {
                        error.message = format!("`.{name}` requires numeric arguments");
                    }
                    error.message = native_binding_message(name, error.message);
                    if matches!(name, "html" | "markdown")
                        && error.hint
                            == "Remove the final explicit value when using a body fallback."
                    {
                        error.message = format!(
                            "`.{name}` received both a body and an explicit `content` argument"
                        );
                    }
                }
                if user_function {
                    error.message = callable_binding_message(error.message, &error.hint);
                }
                let diagnostic_code =
                    if error.message == "positional argument after named argument is not allowed" {
                        "E3003"
                    } else {
                        diagnostic_code
                    };
                diagnostics.push(binding_diagnostic_with_code(error, diagnostic_code));
                Err(())
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn preflight_native_binding(
        &self,
        name: &str,
        ordered_args: Option<&[IrCallArgument]>,
        positional_args: &[IrValue],
        named_args: &[IrNamedArg],
        body: Option<CallBody<'_>>,
        lambda_parameters: Option<&[IrParameter]>,
        span: SourceSpan,
        context: &EvaluationContext<'_>,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Result<Option<BindingPlan>, ()> {
        // A parameterless call to a captured/assigned name is a value
        // reference, not the collection builtin with the same spelling (for
        // example `.first` inside a function that declares `first`). Likewise
        // preserve the existing reassignment dispatch before native metadata
        // can claim the name.
        if is_variable_reference_call(name, positional_args, named_args, body, context)
            || is_variable_reassignment_call(name, positional_args, named_args, body, context)
        {
            return Ok(None);
        }
        // A source-defined function owns its name once normal dispatch has
        // reached it. The documented state exceptions are native-owned unless
        // shadowed explicitly by the evaluator's existing precedence rule.
        let state_shadowed = matches!(
            name,
            "captionposition" | "docauthor" | "docauthors" | "dockeywords" | "doclang" | "theme"
        ) && context.get_function(name).is_some();
        if context.get_function(name).is_some() && !is_document_state(name) {
            return Ok(None);
        }
        if state_shadowed {
            return Ok(None);
        }

        if let Some(builtin) = builtins::lookup(name) {
            let parameters = builtins::binding_parameters(builtin);
            return self
                .preflight_binding(
                    &parameters,
                    ordered_args,
                    positional_args,
                    named_args,
                    body,
                    builtin.body_policy.binder_policy(),
                    span,
                    "E3001",
                    Some(name),
                    lambda_body_span(name, lambda_parameters),
                    false,
                    diagnostics,
                )
                .map(Some);
        }

        let Some((parameters, body_policy)) = native_binding_parameters(name) else {
            return Ok(None);
        };
        let diagnostic_code = if name == "var" {
            "E3002"
        } else if is_document_state(name) || matches!(name, "let" | "br" | "html" | "markdown") {
            "E3003"
        } else {
            "E3001"
        };
        self.preflight_binding(
            &parameters,
            ordered_args,
            positional_args,
            named_args,
            body,
            body_policy,
            span,
            diagnostic_code,
            Some(name),
            lambda_body_span(name, lambda_parameters),
            false,
            diagnostics,
        )
        .map(Some)
    }

    /// Evaluates a block chain and materializes its final semantic value.
    #[allow(clippy::too_many_arguments)]
    fn evaluate_block_chain(
        &self,
        head: &IrCallSegment,
        chain: &[IrCallSegment],
        body: &Option<Vec<IrNode>>,
        raw_body: Option<&IrRawBody>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
    ) -> CallOutcome {
        match self.evaluate_chain_value(
            head,
            chain,
            body.as_deref().map(CallBody::Block),
            raw_body,
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
        context: &mut EvaluationContext<'_>,
    ) -> Vec<IrInline> {
        match self.evaluate_chain_value(
            head,
            chain,
            body.as_deref().map(CallBody::Inline),
            None,
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
    #[allow(dead_code)]
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
        context: &mut EvaluationContext<'_>,
    ) -> CallOutcome {
        self.evaluate_call_value_with_first_origin(
            name,
            positional_args,
            named_args,
            body,
            None,
            lambda_parameters,
            span,
            diagnostics,
            context,
            None,
            None,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn evaluate_call_value_with_ordered(
        &self,
        name: &str,
        ordered_args: Option<&[IrCallArgument]>,
        positional_args: &[IrValue],
        named_args: &[IrNamedArg],
        body: Option<CallBody<'_>>,
        raw_body: Option<&IrRawBody>,
        lambda_parameters: Option<&[IrParameter]>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
    ) -> CallOutcome {
        self.evaluate_call_value_with_first_origin(
            name,
            positional_args,
            named_args,
            body,
            raw_body,
            lambda_parameters,
            span,
            diagnostics,
            context,
            None,
            ordered_args,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn evaluate_call_value_with_first_origin(
        &self,
        name: &str,
        positional_args: &[IrValue],
        named_args: &[IrNamedArg],
        body: Option<CallBody<'_>>,
        raw_body: Option<&IrRawBody>,
        lambda_parameters: Option<&[IrParameter]>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
        first_origin: Option<ValueOrigin>,
        ordered_args: Option<&[IrCallArgument]>,
        implicit_argument: Option<InvocationValue>,
    ) -> CallOutcome {
        let _depth = match context.enter_evaluation_depth(*span, diagnostics) {
            Ok(depth) => depth,
            Err(outcome) => return outcome,
        };
        context.begin_invocation();
        let checkpoint = InvocationCheckpoint::capture();
        let outcome = self.evaluate_call_value_with_first_origin_inner(
            name,
            positional_args,
            named_args,
            body,
            raw_body,
            lambda_parameters,
            span,
            diagnostics,
            context,
            first_origin,
            ordered_args,
            implicit_argument,
        );
        if matches!(outcome, CallOutcome::Failed | CallOutcome::Unresolved) {
            checkpoint.restore(context);
        } else {
            checkpoint.commit(context);
        }
        context.end_invocation();
        outcome
    }

    #[allow(clippy::too_many_arguments)]
    fn evaluate_call_value_with_first_origin_inner(
        &self,
        name: &str,
        positional_args: &[IrValue],
        named_args: &[IrNamedArg],
        body: Option<CallBody<'_>>,
        raw_body: Option<&IrRawBody>,
        lambda_parameters: Option<&[IrParameter]>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
        first_origin: Option<ValueOrigin>,
        ordered_args: Option<&[IrCallArgument]>,
        implicit_argument: Option<InvocationValue>,
    ) -> CallOutcome {
        if let Err(outcome) = validate_ordered_invocation(
            name,
            ordered_args,
            positional_args,
            named_args,
            *span,
            diagnostics,
        ) {
            return outcome;
        }
        // Quarkdown injects `.super` only into the active extension body. A
        // source-defined callable with this name remains ordinary and must
        // be resolved through the normal function ownership path outside an
        // extension invocation.
        if name == "super" && context.extension_invocation.is_some() {
            return self.evaluate_super_call(
                ordered_args,
                positional_args,
                named_args,
                body,
                lambda_parameters,
                span,
                diagnostics,
                context,
            );
        }
        if let Some(result) = context.get_implicit_parameter(name) {
            return match result {
                Ok(value) => CallOutcome::Value(value),
                Err(error) => {
                    diagnostics.push(implicit_parameter_error(name, error, *span));
                    CallOutcome::Failed
                }
            };
        }
        if name == "super" && context.get_function(name).is_none() {
            diagnostics.push(extension_error(
                "`.super` is only available inside an extension body".to_string(),
                *span,
                None,
            ));
            return CallOutcome::Failed;
        }

        let native_binding_plan = match self.preflight_native_binding(
            name,
            ordered_args,
            positional_args,
            named_args,
            body,
            lambda_parameters,
            *span,
            context,
            diagnostics,
        ) {
            Ok(plan) => plan,
            Err(()) => return CallOutcome::Failed,
        };

        if is_conditional(name) {
            let Some(binding_plan) = native_binding_plan.as_ref() else {
                return CallOutcome::Failed;
            };
            let raw_candidates = raw_invocation_candidates(positional_args, named_args, *span);
            let bound = match binding_plan.bind(&raw_candidates, None, *span) {
                Ok(bound) => bound,
                Err(error) => {
                    diagnostics.push(binding_diagnostic_with_code(error, "E3003"));
                    return CallOutcome::Failed;
                }
            };
            let condition = match self.resolve_call_condition(
                name,
                bound.slots.first(),
                bound.parameters.first(),
                span,
                diagnostics,
                context,
                first_origin,
            ) {
                Ok(condition) => condition,
                Err(outcome) => return outcome,
            };
            return if take_branch(name, condition) {
                self.conditional_content_value(&bound, body, span, diagnostics, context)
            } else {
                CallOutcome::Value(IrValue::Content(Vec::new()))
            };
        }

        if name == "extend" && context.get_function(name).is_none() {
            return self.evaluate_extend(
                ordered_args,
                positional_args,
                named_args,
                body,
                lambda_parameters,
                span,
                diagnostics,
                context,
                native_binding_plan.as_ref(),
            );
        }

        let source_defined_shadowable_document_state = matches!(
            name,
            "captionposition" | "docauthor" | "docauthors" | "dockeywords" | "doclang" | "theme"
        ) && context.get_function(name).is_some();
        if is_document_state(name) && !source_defined_shadowable_document_state {
            return self.evaluate_document_state_builtin(
                name,
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

        if is_html(name) {
            return self.evaluate_html(
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

        if is_markdown(name) {
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

        if is_resource(name) {
            return self.evaluate_resource_builtin(
                name,
                positional_args,
                named_args,
                body,
                span,
                diagnostics,
                context,
                native_binding_plan.as_ref(),
            );
        }

        if is_deferred(name) {
            diagnostics.push(resource_diagnostic(
                "E8001",
                "`.llmstxt` is not part of the tracked Quarkdown v2.5.1 standard builtin surface",
                *span,
                "This resource/document feature remains deferred until an evidenced upstream contract is available.",
            ));
            return CallOutcome::Failed;
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
                native_binding_plan.as_ref(),
            );
        }

        // Inline iteration bodies remain target-sensitive: a source-defined
        // `foreach`/`repeat` binding must resolve before the native adapter
        // can interpret an InlineBody as a callable. Other native dispatch
        // keeps its existing precedence.
        if matches!(name, "foreach" | "repeat") {
            if let Some(binding) = context.get_function(name).cloned() {
                return self.evaluate_function_binding(
                    &binding,
                    ordered_args,
                    positional_args,
                    named_args,
                    body,
                    raw_body,
                    span,
                    diagnostics,
                    context,
                    implicit_argument,
                );
            }
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
                native_binding_plan.as_ref(),
                first_origin,
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
                native_binding_plan.as_ref(),
            );
        }

        if is_optionality_callback(name) {
            return self.evaluate_optionality_callback(
                name,
                positional_args,
                named_args,
                body,
                lambda_parameters,
                span,
                diagnostics,
                context,
                native_binding_plan.as_ref(),
            );
        }

        if is_collection_transform(name) {
            return self.evaluate_collection_transform(
                name,
                positional_args,
                named_args,
                body,
                lambda_parameters,
                span,
                diagnostics,
                context,
                native_binding_plan.as_ref(),
                first_origin,
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
                native_binding_plan.as_ref(),
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

        // Preserve the existing fallback for source-defined names that are
        // not handled by an earlier native branch. Iteration shadowing is
        // resolved explicitly above because its inline body is contextual.
        if let Some(binding) = context.get_function(name).cloned() {
            return self.evaluate_function_binding(
                &binding,
                ordered_args,
                positional_args,
                named_args,
                body,
                raw_body,
                span,
                diagnostics,
                context,
                implicit_argument,
            );
        }

        if is_center(name) {
            return self.evaluate_center(
                positional_args,
                named_args,
                body,
                lambda_parameters,
                span,
                diagnostics,
                context,
                native_binding_plan.as_ref(),
            );
        }

        if is_align(name) {
            return self.evaluate_align(
                positional_args,
                named_args,
                body,
                lambda_parameters,
                span,
                diagnostics,
                context,
                native_binding_plan.as_ref(),
                first_origin,
            );
        }

        if is_container(name) {
            return self.evaluate_container(
                positional_args,
                named_args,
                body,
                lambda_parameters,
                span,
                diagnostics,
                context,
                native_binding_plan.as_ref(),
                first_origin,
            );
        }

        if is_landscape(name) {
            return self.evaluate_landscape(
                positional_args,
                named_args,
                body,
                lambda_parameters,
                span,
                diagnostics,
                context,
                native_binding_plan.as_ref(),
            );
        }

        if is_br(name) {
            return self.evaluate_br(
                positional_args,
                named_args,
                body,
                lambda_parameters,
                span,
                diagnostics,
                native_binding_plan.as_ref(),
            );
        }

        if is_whitespace(name) {
            return self.evaluate_whitespace(
                positional_args,
                named_args,
                body,
                lambda_parameters,
                span,
                diagnostics,
                context,
                native_binding_plan.as_ref(),
                first_origin,
            );
        }

        if is_stacked_layout(name) {
            return self.evaluate_stacked_layout(
                name,
                positional_args,
                named_args,
                body,
                lambda_parameters,
                span,
                diagnostics,
                context,
                native_binding_plan.as_ref(),
                first_origin,
            );
        }

        if is_range(name) {
            return self.evaluate_range(
                positional_args,
                named_args,
                body,
                span,
                diagnostics,
                context,
                native_binding_plan.as_ref(),
                first_origin,
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
                native_binding_plan.as_ref(),
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
                native_binding_plan.as_ref(),
            );
        }

        if is_collection_access(name) {
            let Some(binding_plan) = native_binding_plan.as_ref() else {
                return CallOutcome::Failed;
            };
            let evaluated_positional = match self.evaluate_invocation_values(
                positional_args,
                span,
                diagnostics,
                context,
                first_origin,
            ) {
                Ok(values) => values,
                Err(outcome) => return outcome,
            };
            let evaluated_named =
                match self.evaluate_invocation_named(named_args, span, diagnostics, context) {
                    Ok(values) => values,
                    Err(outcome) => return outcome,
                };
            return self.evaluate_collection_access(
                name,
                &evaluated_positional,
                &evaluated_named,
                span,
                diagnostics,
                context,
                binding_plan,
            );
        }

        if let Some(builtin) = builtins::lookup(name) {
            let Some(binding_plan) = native_binding_plan.as_ref() else {
                return CallOutcome::Failed;
            };
            let evaluated_candidates = match self.evaluate_invocation_candidates(
                ordered_args,
                positional_args,
                named_args,
                span,
                diagnostics,
                context,
                first_origin,
                implicit_argument.as_ref(),
            ) {
                Ok(values) => values,
                Err(outcome) => return outcome,
            };
            return self.evaluate_builtin_with_candidates(
                builtin,
                binding_plan,
                evaluated_candidates,
                body,
                raw_body,
                *span,
                diagnostics,
                context,
            );
        }

        // Ordinary output context preserves unresolved calls. A chain wrapper
        // converts this outcome into an explicit source-backed E3001 instead.
        CallOutcome::Unresolved
    }

    #[allow(clippy::too_many_arguments)]
    fn evaluate_builtin_with_candidates(
        &self,
        builtin: &builtins::BuiltinSpec,
        binding_plan: &BindingPlan,
        candidates: Vec<Candidate<InvocationValue>>,
        body: Option<CallBody<'_>>,
        raw_body: Option<&IrRawBody>,
        span: SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
    ) -> CallOutcome {
        let body_value = match builtin.body_policy {
            builtins::BuiltinBodyPolicy::Reject => None,
            builtins::BuiltinBodyPolicy::BindRaw => {
                if let Some(call_body) = body {
                    let Some(raw_body) = raw_body else {
                        diagnostics.push(chain_evaluation_error(
                            "This body conversion requires source-backed raw body text".to_string(),
                            call_body_source_span(call_body, span),
                        ));
                        return CallOutcome::Failed;
                    };
                    let Some(body_text) = value_conversion::raw_body_dynamic_text(raw_body) else {
                        diagnostics.push(chain_evaluation_error(
                            "This body conversion requires a valid source-backed body span"
                                .to_string(),
                            call_body_source_span(call_body, span),
                        ));
                        return CallOutcome::Failed;
                    };
                    Some(Candidate::Positional {
                        value: InvocationValue::dynamic_value(IrValue::String(body_text)),
                        // `IrRawBody::span` is local to its source. Binding
                        // diagnostics use the containing call span, matching
                        // the ordinary builtin invocation path.
                        span,
                    })
                } else {
                    None
                }
            }
            builtins::BuiltinBodyPolicy::BindEvaluatedContent => {
                if let Some(call_body) = body {
                    let body = match self.evaluate_call_body(call_body, &span, diagnostics, context)
                    {
                        CallOutcome::Value(value) => value,
                        outcome => return outcome,
                    };
                    Some(Candidate::Positional {
                        value: InvocationValue::static_value(body),
                        span: call_body_source_span(call_body, span),
                    })
                } else {
                    None
                }
            }
        };
        let bound = match binding_plan.bind(&candidates, body_value.as_ref(), span) {
            Ok(bound) => bound,
            Err(error) => {
                diagnostics.push(binding_diagnostic_with_code(error, "E3001"));
                return CallOutcome::Failed;
            }
        };
        let bound = match self.convert_builtin_targets(builtin, bound, span, diagnostics, context) {
            Ok(bound) => bound,
            Err(outcome) => return outcome,
        };
        match builtins::evaluate_bound(builtin, bound) {
            Ok(value) => CallOutcome::Value(value),
            Err(error) => {
                if let Some(conversion) = error.conversion {
                    diagnostics.push(conversion_failure_diagnostic(
                        value_conversion::ConversionFailure::new(
                            conversion.error,
                            conversion.candidate_span,
                            Some(conversion.parameter),
                            conversion.parameter_span,
                            span,
                        ),
                        Some(error.message.as_str()),
                    ));
                } else {
                    diagnostics.push(chain_evaluation_error(error.message, span));
                }
                CallOutcome::Failed
            }
        }
    }

    /// Applies target-driven conversion after structural binding and after
    /// ordinary candidates have been evaluated. This is the one regular
    /// builtin integration point for context-sensitive content conversion;
    /// individual builtins receive the resulting semantic value only.
    fn convert_builtin_targets(
        &self,
        builtin: &builtins::BuiltinSpec,
        mut bound: invocation_binder::BoundInvocation<InvocationValue>,
        call_span: SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
    ) -> Result<invocation_binder::BoundInvocation<InvocationValue>, CallOutcome> {
        if builtin.kind != builtins::BuiltinKind::Plaintext {
            return Ok(bound);
        }

        for (index, slot) in bound.slots.iter_mut().enumerate() {
            let BoundSlot::Explicit { value, span } = slot else {
                continue;
            };
            let converted = match value_conversion::convert_target_with_origin(
                value,
                value_conversion::ConversionTarget::InlineContent,
                *span,
            ) {
                Ok(value) => value,
                Err(error) => {
                    let parameter = bound.parameters.get(index);
                    diagnostics.push(conversion_failure_diagnostic(
                        value_conversion::ConversionFailure::new(
                            error,
                            Some(*span),
                            parameter.map(|parameter| parameter.name.clone()),
                            parameter.and_then(|parameter| parameter.name_span),
                            call_span,
                        ),
                        Some("`.plaintext` content"),
                    ));
                    return Err(CallOutcome::Failed);
                }
            };
            value.value =
                self.resolve_target_value(converted, *span, call_span, diagnostics, context)?;
        }
        Ok(bound)
    }

    /// Resolves a conversion request that needs the evaluator's Markdown and
    /// call context. A source-backed raw call body is handled by its owning
    /// builtin before this function; this path is only for a dynamic value
    /// whose target conversion explicitly requests Markdown interpretation.
    fn resolve_target_value(
        &self,
        converted: value_conversion::TargetValue,
        argument_span: SourceSpan,
        _call_span: SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
    ) -> Result<IrValue, CallOutcome> {
        match converted {
            value_conversion::TargetValue::Value(value) => Ok(value),
            value_conversion::TargetValue::RawMarkdown { target, text } => {
                if !matches!(
                    target,
                    value_conversion::RawMarkdownTarget::Inline
                        | value_conversion::RawMarkdownTarget::Block
                ) {
                    diagnostics.push(target_conversion_error_message(
                        "dynamic target conversion",
                        argument_span,
                        "this target is not materialized by the current builtin boundary"
                            .to_string(),
                    ));
                    return Err(CallOutcome::Failed);
                }
                let nodes =
                    self.parse_dynamic_markdown_content(&text, argument_span, target, diagnostics)?;
                match target {
                    value_conversion::RawMarkdownTarget::Inline
                    | value_conversion::RawMarkdownTarget::Block => {
                        let before = diagnostics.len();
                        let nodes = self.evaluate_nodes(&nodes, diagnostics, context);
                        if diagnostics.len() == before {
                            Ok(IrValue::Content(nodes))
                        } else {
                            Err(CallOutcome::Failed)
                        }
                    }
                    value_conversion::RawMarkdownTarget::Iterable
                    | value_conversion::RawMarkdownTarget::Dictionary
                    | value_conversion::RawMarkdownTarget::Callable => unreachable!(
                        "unsupported raw target was rejected before dynamic Markdown parsing"
                    ),
                }
            }
        }
    }

    fn parse_dynamic_markdown_content(
        &self,
        text: &str,
        span: SourceSpan,
        target: value_conversion::RawMarkdownTarget,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Result<Vec<IrNode>, CallOutcome> {
        // Dynamic text is reparsed only because the selected upstream target
        // is a Markdown/content factory. Source-backed call bodies never use
        // this path: their lossless source span is retained in `IrRawBody` and
        // its target value is derived directly at the target boundary.
        let parsed = match target {
            value_conversion::RawMarkdownTarget::Inline => {
                arkst_markdown::parse_inline_with_mode(text, Mode::Quarkdown)
            }
            value_conversion::RawMarkdownTarget::Block
            | value_conversion::RawMarkdownTarget::Iterable
            | value_conversion::RawMarkdownTarget::Dictionary
            | value_conversion::RawMarkdownTarget::Callable => {
                arkst_markdown::parse_with_mode(text, Mode::Quarkdown)
            }
        };
        if !parsed.diagnostics.is_empty() {
            for diagnostic in parsed.diagnostics {
                diagnostics.push(target_conversion_error_message(
                    "dynamic Markdown content",
                    span,
                    diagnostic.message,
                ));
            }
            return Err(CallOutcome::Failed);
        }
        let (document, conversion_diagnostics) = ast_to_ir::ast_to_ir_with_diagnostics_for_mode(
            &parsed.document,
            span.source_id,
            &crate::DocumentMetadataDefaults::default(),
            Mode::Quarkdown,
        );
        if !conversion_diagnostics.is_empty() {
            for diagnostic in conversion_diagnostics {
                diagnostics.push(target_conversion_error_message(
                    "dynamic Markdown content",
                    span,
                    diagnostic.message,
                ));
            }
            return Err(CallOutcome::Failed);
        }
        let mut nodes = document.nodes;
        rebase_dynamic_nodes(&mut nodes, span);
        Ok(nodes)
    }

    #[allow(clippy::too_many_arguments)]
    fn evaluate_center(
        &self,
        _positional_args: &[IrValue],
        _named_args: &[IrNamedArg],
        body: Option<CallBody<'_>>,
        lambda_parameters: Option<&[IrParameter]>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
        binding_plan: Option<&BindingPlan>,
    ) -> CallOutcome {
        let Some(binding_plan) = binding_plan else {
            return CallOutcome::Failed;
        };
        if binding_plan
            .bind::<InvocationValue>(&[], None, *span)
            .is_err()
        {
            return CallOutcome::Failed;
        }
        if let Some(parameters) = lambda_parameters {
            let diagnostic_span = parameters.first().map_or(*span, |parameter| parameter.span);
            diagnostics.push(center_argument_error(
                "`.center` body is a Markdown block, not a lambda",
                diagnostic_span,
            ));
            return CallOutcome::Failed;
        }

        let children = match body {
            Some(CallBody::Block(nodes)) => {
                match self.evaluate_call_body(CallBody::Block(nodes), span, diagnostics, context) {
                    CallOutcome::Value(IrValue::Content(nodes)) => nodes,
                    outcome => return outcome,
                }
            }
            Some(CallBody::Inline(_)) => {
                diagnostics.push(center_argument_error("`.center` is block-only", *span));
                return CallOutcome::Failed;
            }
            None => {
                diagnostics.push(center_argument_error(
                    "`.center` requires a Markdown block body",
                    *span,
                ));
                return CallOutcome::Failed;
            }
        };

        CallOutcome::Value(IrValue::Component(IrComponent::Container(
            IrContainerComponent {
                width: None,
                height: None,
                full_width: true,
                alignment: Some(IrContainerAlignment::Center),
                children,
                span: *span,
            },
        )))
    }

    #[allow(clippy::too_many_arguments)]
    fn evaluate_landscape(
        &self,
        _positional_args: &[IrValue],
        _named_args: &[IrNamedArg],
        body: Option<CallBody<'_>>,
        lambda_parameters: Option<&[IrParameter]>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
        binding_plan: Option<&BindingPlan>,
    ) -> CallOutcome {
        let Some(binding_plan) = binding_plan else {
            return CallOutcome::Failed;
        };
        if binding_plan
            .bind::<InvocationValue>(&[], None, *span)
            .is_err()
        {
            return CallOutcome::Failed;
        }
        if let Some(parameters) = lambda_parameters {
            let diagnostic_span = parameters.first().map_or(*span, |parameter| parameter.span);
            diagnostics.push(landscape_argument_error(
                "`.landscape` body is a Markdown block, not a lambda",
                diagnostic_span,
            ));
            return CallOutcome::Failed;
        }

        let children = match body {
            Some(CallBody::Block(nodes)) => {
                match self.evaluate_call_body(CallBody::Block(nodes), span, diagnostics, context) {
                    CallOutcome::Value(IrValue::Content(nodes)) => nodes,
                    outcome => return outcome,
                }
            }
            Some(CallBody::Inline(_)) => {
                diagnostics.push(landscape_argument_error(
                    "`.landscape` is block-only",
                    *span,
                ));
                return CallOutcome::Failed;
            }
            None => {
                diagnostics.push(landscape_argument_error(
                    "`.landscape` requires a Markdown block body",
                    *span,
                ));
                return CallOutcome::Failed;
            }
        };

        CallOutcome::Value(IrValue::Component(IrComponent::Landscape(
            IrLandscapeComponent {
                children,
                span: *span,
            },
        )))
    }

    #[allow(clippy::too_many_arguments)]
    fn evaluate_br(
        &self,
        _positional_args: &[IrValue],
        _named_args: &[IrNamedArg],
        _body: Option<CallBody<'_>>,
        lambda_parameters: Option<&[IrParameter]>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        binding_plan: Option<&BindingPlan>,
    ) -> CallOutcome {
        let Some(binding_plan) = binding_plan else {
            return CallOutcome::Failed;
        };
        if binding_plan
            .bind::<InvocationValue>(&[], None, *span)
            .is_err()
        {
            return CallOutcome::Failed;
        }
        if let Some(parameters) = lambda_parameters {
            let diagnostic_span = parameters.first().map_or(*span, |parameter| parameter.span);
            diagnostics.push(br_argument_error(
                "`.br` does not accept a lambda body",
                diagnostic_span,
            ));
            return CallOutcome::Failed;
        }
        CallOutcome::Value(IrValue::Content(vec![IrNode::Paragraph {
            content: vec![IrInline::HardBreak { span: *span }],
            span: *span,
        }]))
    }

    #[allow(clippy::too_many_arguments)]
    fn evaluate_whitespace(
        &self,
        positional_args: &[IrValue],
        named_args: &[IrNamedArg],
        _body: Option<CallBody<'_>>,
        lambda_parameters: Option<&[IrParameter]>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
        binding_plan: Option<&BindingPlan>,
        first_origin: Option<ValueOrigin>,
    ) -> CallOutcome {
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
        let named_values =
            match self.evaluate_invocation_named(named_args, span, diagnostics, context) {
                Ok(values) => values,
                Err(outcome) => return outcome,
            };
        let positional = positional_values
            .into_iter()
            .zip(positional_args.iter())
            .map(|(value, source)| WhitespaceArgument {
                value,
                span: value_source_span(source, span),
                parameter_span: None,
            })
            .collect();
        let Some(binding_plan) = binding_plan else {
            return CallOutcome::Failed;
        };
        let bound = match bind_whitespace_arguments(
            binding_plan,
            positional,
            named_values,
            span,
            diagnostics,
        ) {
            Ok(bound) => bound,
            Err(outcome) => return outcome,
        };

        let width = match bound.width.as_ref() {
            Some(argument) => match convert_whitespace_size(&argument.value) {
                Ok(value) => value,
                Err(error) => {
                    diagnostics.push(whitespace_conversion_error(
                        "width",
                        argument.span,
                        argument.parameter_span,
                        error,
                    ));
                    return CallOutcome::Failed;
                }
            },
            None => None,
        };
        let height = match bound.height.as_ref() {
            Some(argument) => match convert_whitespace_size(&argument.value) {
                Ok(value) => value,
                Err(error) => {
                    diagnostics.push(whitespace_conversion_error(
                        "height",
                        argument.span,
                        argument.parameter_span,
                        error,
                    ));
                    return CallOutcome::Failed;
                }
            },
            None => None,
        };

        if let Some(parameters) = lambda_parameters {
            let diagnostic_span = parameters.first().map_or(*span, |parameter| parameter.span);
            diagnostics.push(whitespace_argument_error(
                "`.whitespace` does not accept a lambda body",
                diagnostic_span,
            ));
            return CallOutcome::Failed;
        }
        let (width, height) = match (width, height) {
            (None, None) => (None, None),
            (width, height) => (
                Some(width.unwrap_or_else(zero_whitespace_size)),
                Some(height.unwrap_or_else(zero_whitespace_size)),
            ),
        };
        CallOutcome::Value(IrValue::Content(vec![IrNode::Paragraph {
            content: vec![IrInline::Whitespace {
                width,
                height,
                span: *span,
            }],
            span: *span,
        }]))
    }

    #[allow(clippy::too_many_arguments)]
    fn evaluate_align(
        &self,
        positional_args: &[IrValue],
        named_args: &[IrNamedArg],
        body: Option<CallBody<'_>>,
        lambda_parameters: Option<&[IrParameter]>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
        binding_plan: Option<&BindingPlan>,
        first_origin: Option<ValueOrigin>,
    ) -> CallOutcome {
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
        let named_values =
            match self.evaluate_invocation_named(named_args, span, diagnostics, context) {
                Ok(values) => values,
                Err(outcome) => return outcome,
            };
        let positional = positional_values
            .into_iter()
            .zip(positional_args.iter())
            .map(|(value, source)| AlignArgument {
                value,
                span: value_source_span(source, span),
                parameter_span: None,
            })
            .collect();
        let Some(binding_plan) = binding_plan else {
            return CallOutcome::Failed;
        };
        let alignment =
            match bind_align_argument(binding_plan, positional, named_values, span, diagnostics) {
                Ok(argument) => argument,
                Err(outcome) => return outcome,
            };
        let alignment = match convert_align_alignment(&alignment.value) {
            Ok(value) => value,
            Err(error) => {
                diagnostics.push(align_conversion_error(
                    alignment.span,
                    alignment.parameter_span,
                    error,
                ));
                return CallOutcome::Failed;
            }
        };

        if let Some(parameters) = lambda_parameters {
            let diagnostic_span = parameters.first().map_or(*span, |parameter| parameter.span);
            diagnostics.push(align_argument_error(
                "`.align` body is a Markdown block, not a lambda",
                diagnostic_span,
            ));
            return CallOutcome::Failed;
        }
        let children = match body {
            Some(CallBody::Block(nodes)) => {
                match self.evaluate_call_body(CallBody::Block(nodes), span, diagnostics, context) {
                    CallOutcome::Value(IrValue::Content(nodes)) => nodes,
                    outcome => return outcome,
                }
            }
            Some(CallBody::Inline(_)) => {
                diagnostics.push(align_argument_error("`.align` is block-only", *span));
                return CallOutcome::Failed;
            }
            None => {
                diagnostics.push(align_argument_error(
                    "`.align` requires a Markdown block body",
                    *span,
                ));
                return CallOutcome::Failed;
            }
        };

        CallOutcome::Value(IrValue::Component(IrComponent::Container(
            IrContainerComponent {
                width: None,
                height: None,
                full_width: true,
                alignment: Some(alignment),
                children,
                span: *span,
            },
        )))
    }

    #[allow(clippy::too_many_arguments)]
    fn evaluate_container(
        &self,
        positional_args: &[IrValue],
        named_args: &[IrNamedArg],
        body: Option<CallBody<'_>>,
        lambda_parameters: Option<&[IrParameter]>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
        binding_plan: Option<&BindingPlan>,
        first_origin: Option<ValueOrigin>,
    ) -> CallOutcome {
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
        let named_values =
            match self.evaluate_invocation_named(named_args, span, diagnostics, context) {
                Ok(values) => values,
                Err(outcome) => return outcome,
            };
        let positional = positional_values
            .into_iter()
            .zip(positional_args.iter())
            .map(|(value, source)| ContainerArgument {
                value,
                span: value_source_span(source, span),
                parameter_span: None,
            })
            .collect();
        let Some(binding_plan) = binding_plan else {
            return CallOutcome::Failed;
        };
        let bound = match bind_container_arguments(
            binding_plan,
            positional,
            named_values,
            span,
            diagnostics,
        ) {
            Ok(bound) => bound,
            Err(outcome) => return outcome,
        };

        let width = match bound.width.as_ref() {
            Some(argument) if matches!(&argument.value.value, IrValue::None) => None,
            Some(argument) => match convert_container_size(&argument.value) {
                Ok(value) => Some(value),
                Err(error) => {
                    diagnostics.push(container_conversion_error(
                        "width",
                        argument.span,
                        argument.parameter_span,
                        error,
                    ));
                    return CallOutcome::Failed;
                }
            },
            None => None,
        };
        let height = match bound.height.as_ref() {
            Some(argument) if matches!(&argument.value.value, IrValue::None) => None,
            Some(argument) => match convert_container_size(&argument.value) {
                Ok(value) => Some(value),
                Err(error) => {
                    diagnostics.push(container_conversion_error(
                        "height",
                        argument.span,
                        argument.parameter_span,
                        error,
                    ));
                    return CallOutcome::Failed;
                }
            },
            None => None,
        };
        let full_width = match bound.full_width.as_ref() {
            Some(argument) => match convert_container_boolean(&argument.value) {
                Ok(value) => value,
                Err(error) => {
                    diagnostics.push(container_conversion_error(
                        "fullwidth",
                        argument.span,
                        argument.parameter_span,
                        error,
                    ));
                    return CallOutcome::Failed;
                }
            },
            None => false,
        };

        if let Some(parameters) = lambda_parameters {
            let diagnostic_span = parameters.first().map_or(*span, |parameter| parameter.span);
            diagnostics.push(container_argument_error_at(
                "`.container` body is a Markdown block, not a lambda".to_string(),
                diagnostic_span,
            ));
            return CallOutcome::Failed;
        }

        let children = match body {
            Some(CallBody::Block(nodes)) => {
                match self.evaluate_call_body(CallBody::Block(nodes), span, diagnostics, context) {
                    CallOutcome::Value(IrValue::Content(nodes)) => nodes,
                    outcome => return outcome,
                }
            }
            Some(CallBody::Inline(_)) => {
                diagnostics.push(container_argument_error(
                    "`.container` is block-only",
                    *span,
                ));
                return CallOutcome::Failed;
            }
            None => Vec::new(),
        };

        CallOutcome::Value(IrValue::Component(IrComponent::Container(
            IrContainerComponent {
                width,
                height,
                full_width,
                alignment: None,
                children,
                span: *span,
            },
        )))
    }

    #[allow(clippy::too_many_arguments)]
    fn evaluate_stacked_layout(
        &self,
        name: &str,
        positional_args: &[IrValue],
        named_args: &[IrNamedArg],
        body: Option<CallBody<'_>>,
        lambda_parameters: Option<&[IrParameter]>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
        binding_plan: Option<&BindingPlan>,
        first_origin: Option<ValueOrigin>,
    ) -> CallOutcome {
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
        let named_values =
            match self.evaluate_invocation_named(named_args, span, diagnostics, context) {
                Ok(values) => values,
                Err(outcome) => return outcome,
            };
        let positional = positional_values
            .into_iter()
            .zip(positional_args.iter())
            .map(|(value, source)| StackedArgument {
                value,
                span: value_source_span(source, span),
                parameter_span: None,
            })
            .collect();
        let Some(binding_plan) = binding_plan else {
            return CallOutcome::Failed;
        };
        let bound = match bind_stacked_arguments(
            binding_plan,
            name,
            positional,
            named_values,
            span,
            diagnostics,
        ) {
            Ok(bound) => bound,
            Err(outcome) => return outcome,
        };
        let mut bound = bound;
        let default = |value| StackedArgument {
            value: InvocationValue::static_value(value),
            span: *span,
            parameter_span: None,
        };

        let (layout, main_axis, cross_axis, row_gap, column_gap) = match name {
            "row" | "column" => {
                let alignment = bound.take(0).unwrap_or_else(|| {
                    default(IrValue::Enum(IrEnumValue::StackedMainAxisAlignment(
                        IrMainAxisAlignment::Start,
                    )))
                });
                let cross = bound.take(1).unwrap_or_else(|| {
                    default(IrValue::Enum(IrEnumValue::StackedCrossAxisAlignment(
                        IrCrossAxisAlignment::Center,
                    )))
                });
                let gap = bound.take(2).unwrap_or_else(|| default(IrValue::None));
                let main_axis = match convert_stacked_main_axis(&alignment.value) {
                    Ok(value) => value,
                    Err(error) => {
                        diagnostics.push(stacked_conversion_error(
                            name,
                            "alignment",
                            alignment.span,
                            alignment.parameter_span,
                            error,
                        ));
                        return CallOutcome::Failed;
                    }
                };
                let cross_axis = match convert_stacked_cross_axis(&cross.value) {
                    Ok(value) => value,
                    Err(error) => {
                        diagnostics.push(stacked_conversion_error(
                            name,
                            "cross",
                            cross.span,
                            cross.parameter_span,
                            error,
                        ));
                        return CallOutcome::Failed;
                    }
                };
                let gap = match convert_optional_stacked_size(&gap.value) {
                    Ok(value) => value,
                    Err(error) => {
                        diagnostics.push(stacked_conversion_error(
                            name,
                            "gap",
                            gap.span,
                            gap.parameter_span,
                            error,
                        ));
                        return CallOutcome::Failed;
                    }
                };
                let (layout, row_gap, column_gap) = if name == "row" {
                    (IrStackedLayout::Row, None, gap)
                } else {
                    (IrStackedLayout::Column, gap, None)
                };
                (layout, main_axis, cross_axis, row_gap, column_gap)
            }
            "grid" => {
                let Some(columns) = bound.take(0) else {
                    diagnostics.push(stacked_argument_error(
                        name,
                        "columns",
                        *span,
                        "required argument is missing",
                    ));
                    return CallOutcome::Failed;
                };
                let alignment = bound.take(1).unwrap_or_else(|| {
                    default(IrValue::Enum(IrEnumValue::StackedMainAxisAlignment(
                        IrMainAxisAlignment::Center,
                    )))
                });
                let cross = bound.take(2).unwrap_or_else(|| {
                    default(IrValue::Enum(IrEnumValue::StackedCrossAxisAlignment(
                        IrCrossAxisAlignment::Center,
                    )))
                });
                let gap = bound.take(3).unwrap_or_else(|| default(IrValue::None));
                let vgap = bound.take(4).unwrap_or_else(|| default(IrValue::None));
                let hgap = bound.take(5).unwrap_or_else(|| default(IrValue::None));
                let columns = match value_conversion::convert_integer_with_origin(&columns.value) {
                    Ok(value) => value,
                    Err(error) => {
                        diagnostics.push(stacked_conversion_error(
                            name,
                            "columns",
                            columns.span,
                            columns.parameter_span,
                            error,
                        ));
                        return CallOutcome::Failed;
                    }
                };
                if columns <= 0 {
                    diagnostics.push(stacked_argument_error(
                        name,
                        "columns",
                        *span,
                        "Column count must be at least 1",
                    ));
                    return CallOutcome::Failed;
                }
                let Some(columns) = NonZeroU32::new(columns as u32) else {
                    diagnostics.push(stacked_argument_error(
                        name,
                        "columns",
                        *span,
                        "Column count must be at least 1",
                    ));
                    return CallOutcome::Failed;
                };
                let main_axis = match convert_stacked_main_axis(&alignment.value) {
                    Ok(value) => value,
                    Err(error) => {
                        diagnostics.push(stacked_conversion_error(
                            name,
                            "alignment",
                            alignment.span,
                            alignment.parameter_span,
                            error,
                        ));
                        return CallOutcome::Failed;
                    }
                };
                let cross_axis = match convert_stacked_cross_axis(&cross.value) {
                    Ok(value) => value,
                    Err(error) => {
                        diagnostics.push(stacked_conversion_error(
                            name,
                            "cross",
                            cross.span,
                            cross.parameter_span,
                            error,
                        ));
                        return CallOutcome::Failed;
                    }
                };
                let gap = match convert_optional_stacked_size(&gap.value) {
                    Ok(value) => value,
                    Err(error) => {
                        diagnostics.push(stacked_conversion_error(
                            name,
                            "gap",
                            gap.span,
                            gap.parameter_span,
                            error,
                        ));
                        return CallOutcome::Failed;
                    }
                };
                let vgap = match convert_optional_stacked_size(&vgap.value) {
                    Ok(value) => value,
                    Err(error) => {
                        diagnostics.push(stacked_conversion_error(
                            name,
                            "vgap",
                            vgap.span,
                            vgap.parameter_span,
                            error,
                        ));
                        return CallOutcome::Failed;
                    }
                };
                let hgap = match convert_optional_stacked_size(&hgap.value) {
                    Ok(value) => value,
                    Err(error) => {
                        diagnostics.push(stacked_conversion_error(
                            name,
                            "hgap",
                            hgap.span,
                            hgap.parameter_span,
                            error,
                        ));
                        return CallOutcome::Failed;
                    }
                };
                (
                    IrStackedLayout::Grid { columns },
                    main_axis,
                    cross_axis,
                    vgap.or_else(|| gap.clone()),
                    hgap.or(gap),
                )
            }
            _ => return CallOutcome::Unresolved,
        };

        if let Some(parameters) = lambda_parameters {
            let diagnostic_span = parameters.first().map_or(*span, |parameter| parameter.span);
            diagnostics.push(stacked_argument_error(
                name,
                "body",
                diagnostic_span,
                "Stacked layout bodies are Markdown blocks, not lambda parameters",
            ));
            return CallOutcome::Failed;
        }
        let children = match body {
            Some(CallBody::Block(nodes)) => {
                match self.evaluate_call_body(CallBody::Block(nodes), span, diagnostics, context) {
                    CallOutcome::Value(IrValue::Content(nodes)) => nodes,
                    outcome => return outcome,
                }
            }
            Some(CallBody::Inline(_)) => {
                diagnostics.push(stacked_argument_error(
                    name,
                    "body",
                    *span,
                    "Stacked layout is block-only",
                ));
                return CallOutcome::Failed;
            }
            None => {
                diagnostics.push(stacked_argument_error(
                    name,
                    "body",
                    *span,
                    "A Markdown block body is required",
                ));
                return CallOutcome::Failed;
            }
        };

        CallOutcome::Value(IrValue::Component(IrComponent::Stacked(
            IrStackedComponent {
                layout,
                main_axis_alignment: main_axis,
                cross_axis_alignment: cross_axis,
                row_gap,
                column_gap,
                children,
                span: *span,
            },
        )))
    }

    /// Implements the read/write document-state builtins without changing
    /// the ordinary lexical scope maps. Argument evaluation and bounded
    /// String conversion complete before the shared state is mutated.
    #[allow(clippy::too_many_arguments)]
    fn evaluate_document_state_builtin(
        &self,
        name: &str,
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
        if name == "docauthors" {
            return self.evaluate_document_authors_builtin(
                positional_args,
                named_args,
                body,
                raw_body,
                span,
                diagnostics,
                context,
                binding_plan,
                first_origin,
            );
        }

        if name == "dockeywords" {
            return self.evaluate_document_keywords_builtin(
                positional_args,
                named_args,
                body,
                raw_body,
                span,
                diagnostics,
                context,
                binding_plan,
                first_origin,
            );
        }

        if name == "theme" {
            return self.evaluate_document_theme_builtin(
                positional_args,
                named_args,
                body,
                raw_body,
                span,
                diagnostics,
                context,
                binding_plan,
                first_origin,
            );
        }

        if name == "captionposition" {
            return self.evaluate_caption_position_builtin(
                positional_args,
                named_args,
                body,
                raw_body,
                span,
                diagnostics,
                context,
                binding_plan,
                first_origin,
            );
        }

        if name == "doclang" {
            return self.evaluate_document_language_builtin(
                positional_args,
                named_args,
                body,
                raw_body,
                span,
                diagnostics,
                context,
                binding_plan,
                first_origin,
            );
        }

        if body.is_none() && positional_args.is_empty() && named_args.is_empty() {
            return CallOutcome::Value(context.document_state_value(name));
        }
        let Some(binding_plan) = binding_plan else {
            return CallOutcome::Failed;
        };
        let raw_body_candidate = match source_backed_body_candidate(
            body.as_ref()
                .map(|body| call_body_source_span(*body, *span)),
            raw_body,
            name,
            diagnostics,
        ) {
            Ok(candidate) => candidate,
            Err(outcome) => return outcome,
        };

        if name == "docauthor" {
            let evaluated_positional = match self.evaluate_invocation_values(
                positional_args,
                span,
                diagnostics,
                context,
                first_origin,
            ) {
                Ok(values) => values,
                Err(outcome) => return outcome,
            };
            let evaluated_named =
                match self.evaluate_invocation_named(named_args, span, diagnostics, context) {
                    Ok(values) => values,
                    Err(outcome) => return outcome,
                };
            let bound = match bind_evaluated_arguments(
                binding_plan,
                evaluated_positional
                    .into_iter()
                    .zip(positional_args.iter())
                    .map(|(value, source)| (value, value_source_span(source, span)))
                    .collect(),
                evaluated_named,
                raw_body_candidate.as_ref(),
                *span,
            ) {
                Ok(bound) => bound,
                Err(error) => {
                    diagnostics.push(binding_diagnostic_with_code(error, "E3003"));
                    return CallOutcome::Failed;
                }
            };
            let parameter_span = bound
                .parameters
                .first()
                .and_then(|parameter| parameter.name_span);
            let Some(BoundSlot::Explicit {
                value: argument,
                span: argument_span,
            }) = bound.slots.into_iter().next()
            else {
                return CallOutcome::Failed;
            };

            let value = match builtins::scalar_string_conversion(&argument) {
                Ok(value) => value,
                Err(error) => {
                    diagnostics.push(conversion_failure_diagnostic(
                        value_conversion::ConversionFailure::new(
                            error,
                            Some(argument_span),
                            Some("value"),
                            parameter_span,
                            *span,
                        ),
                        Some("`.docauthor`"),
                    ));
                    return CallOutcome::Failed;
                }
            };

            context.append_document_author(value);
            return CallOutcome::NoValue;
        }

        let evaluated_positional = match self.evaluate_invocation_values(
            positional_args,
            span,
            diagnostics,
            context,
            first_origin,
        ) {
            Ok(values) => values,
            Err(outcome) => return outcome,
        };
        let evaluated_named =
            match self.evaluate_invocation_named(named_args, span, diagnostics, context) {
                Ok(values) => values,
                Err(outcome) => return outcome,
            };

        if name == "doctype" {
            let bound = match bind_evaluated_arguments(
                binding_plan,
                evaluated_positional
                    .into_iter()
                    .zip(positional_args.iter())
                    .map(|(value, source)| (value, value_source_span(source, span)))
                    .collect(),
                evaluated_named,
                raw_body_candidate.as_ref(),
                *span,
            ) {
                Ok(bound) => bound,
                Err(error) => {
                    diagnostics.push(binding_diagnostic_with_code(error, "E3003"));
                    return CallOutcome::Failed;
                }
            };
            let parameter_span = bound
                .parameters
                .first()
                .and_then(|parameter| parameter.name_span);
            let Some(BoundSlot::Explicit {
                value: argument,
                span: argument_span,
            }) = bound.slots.into_iter().next()
            else {
                return CallOutcome::Failed;
            };

            let document_type = match value_conversion::convert_domain_with_origin(
                &argument,
                value_conversion::DomainTarget::ClosedEnum(
                    value_conversion::ClosedEnumTarget::DocumentType,
                ),
            ) {
                Ok(value_conversion::DomainValue::Enum(arkst_ir::IrEnumValue::DocumentType(
                    value,
                ))) => value,
                Ok(_) => {
                    diagnostics.push(document_state_conversion_error(
                        "`.doctype` produced an unexpected enum value".to_string(),
                        argument_span,
                    ));
                    return CallOutcome::Failed;
                }
                Err(error) => {
                    diagnostics.push(conversion_failure_diagnostic(
                        value_conversion::ConversionFailure::new(
                            error,
                            Some(argument_span),
                            Some("value"),
                            parameter_span,
                            *span,
                        ),
                        Some("`.doctype`"),
                    ));
                    return CallOutcome::Failed;
                }
            };
            context.set_document_type(document_type);
            return CallOutcome::NoValue;
        }

        let bound = match bind_evaluated_arguments(
            binding_plan,
            evaluated_positional
                .into_iter()
                .zip(positional_args.iter())
                .map(|(value, source)| (value, value_source_span(source, span)))
                .collect(),
            evaluated_named,
            raw_body_candidate.as_ref(),
            *span,
        ) {
            Ok(bound) => bound,
            Err(error) => {
                diagnostics.push(binding_diagnostic_with_code(error, "E3003"));
                return CallOutcome::Failed;
            }
        };
        let parameter_span = bound
            .parameters
            .first()
            .and_then(|parameter| parameter.name_span);
        let Some(BoundSlot::Explicit {
            value: argument,
            span: argument_span,
        }) = bound.slots.into_iter().next()
        else {
            return CallOutcome::Failed;
        };
        let value = match builtins::scalar_string_conversion(&argument) {
            Ok(value) => value,
            Err(error) => {
                diagnostics.push(conversion_failure_diagnostic(
                    value_conversion::ConversionFailure::new(
                        error,
                        Some(argument_span),
                        Some("value"),
                        parameter_span,
                        *span,
                    ),
                    Some(name),
                ));
                return CallOutcome::Failed;
            }
        };

        if name == "docname" && value.trim().is_empty() {
            diagnostics.push(document_state_conversion_error(
                "`.docname` cannot be blank".to_string(),
                argument_span,
            ));
            return CallOutcome::Failed;
        }

        context.set_document_state_value(name, value);
        CallOutcome::NoValue
    }

    /// Implements the bounded `.doclang` read/write contract.
    ///
    /// The evaluator resolves only checked-in locale records. This keeps the
    /// semantic result deterministic and WASM-compatible instead of consulting
    /// a host/JVM locale database. Binding, candidate evaluation, conversion,
    /// resolution, and validation complete before one shared-state commit.
    #[allow(clippy::too_many_arguments)]
    fn evaluate_document_language_builtin(
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
        if body.is_none() && positional_args.is_empty() && named_args.is_empty() {
            return CallOutcome::Value(context.document_state_value("doclang"));
        }
        let Some(binding_plan) = binding_plan else {
            return CallOutcome::Failed;
        };
        let raw_body_candidate = match source_backed_body_candidate(
            body.as_ref()
                .map(|body| call_body_source_span(*body, *span)),
            raw_body,
            "doclang",
            diagnostics,
        ) {
            Ok(candidate) => candidate,
            Err(outcome) => return outcome,
        };

        let evaluated_positional = match self.evaluate_invocation_values(
            positional_args,
            span,
            diagnostics,
            context,
            first_origin,
        ) {
            Ok(values) => values,
            Err(outcome) => return outcome,
        };
        let evaluated_named =
            match self.evaluate_invocation_named(named_args, span, diagnostics, context) {
                Ok(values) => values,
                Err(outcome) => return outcome,
            };
        let bound = match bind_evaluated_arguments(
            binding_plan,
            evaluated_positional
                .into_iter()
                .zip(positional_args.iter())
                .map(|(value, source)| (value, value_source_span(source, span)))
                .collect(),
            evaluated_named,
            raw_body_candidate.as_ref(),
            *span,
        ) {
            Ok(bound) => bound,
            Err(error) => {
                diagnostics.push(binding_diagnostic_with_code(error, "E3003"));
                return CallOutcome::Failed;
            }
        };
        let parameter_span = bound
            .parameters
            .first()
            .and_then(|parameter| parameter.name_span);
        let Some(BoundSlot::Explicit {
            value: argument,
            span: argument_span,
        }) = bound.slots.into_iter().next()
        else {
            return CallOutcome::Failed;
        };

        // Upstream's nullable String parameter receives `.none` as null, and
        // the shared modify-or-echo helper therefore takes its getter path.
        if matches!(&argument.value, IrValue::None) {
            return CallOutcome::Value(context.document_state_value("doclang"));
        }

        let identifier = match builtins::scalar_string_conversion(&argument) {
            Ok(identifier) => identifier,
            Err(error) => {
                diagnostics.push(conversion_failure_diagnostic(
                    value_conversion::ConversionFailure::new(
                        error,
                        Some(argument_span),
                        Some("value"),
                        parameter_span,
                        *span,
                    ),
                    Some("`.doclang`"),
                ));
                return CallOutcome::Failed;
            }
        };

        let Some(locale) = crate::locale::resolve(&identifier) else {
            diagnostics.push(document_state_conversion_error(
                format!("`.doclang` locale `{identifier}` was not found"),
                argument_span,
            ));
            return CallOutcome::Failed;
        };

        context.set_document_locale(locale);
        CallOutcome::NoValue
    }

    /// Implements the bounded `.docauthors` dictionary read/write contract.
    /// Candidate authors are fully evaluated and validated before the shared
    /// document state is changed. The prior state is restored if evaluating
    /// the candidate itself produces a failure or a nested state mutation is
    /// followed by invalid author data.
    #[allow(clippy::too_many_arguments)]
    fn evaluate_document_authors_builtin(
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
        if body.is_none() && positional_args.is_empty() && named_args.is_empty() {
            return self.document_authors_value(*span, diagnostics, context);
        }
        let Some(binding_plan) = binding_plan else {
            return CallOutcome::Failed;
        };

        let (candidate, candidate_span, parameter_span) = if body.is_some() {
            let body_candidate = match source_backed_body_candidate(
                body.as_ref()
                    .map(|body| call_body_source_span(*body, *span)),
                raw_body,
                "docauthors",
                diagnostics,
            ) {
                Ok(Some(candidate)) => candidate,
                Ok(None) | Err(_) => {
                    return CallOutcome::Failed;
                }
            };
            let bound = match binding_plan.bind(&[], Some(&body_candidate), *span) {
                Ok(bound) => bound,
                Err(error) => {
                    diagnostics.push(binding_diagnostic_with_code(error, "E3003"));
                    return CallOutcome::Failed;
                }
            };
            let parameter_span = bound
                .parameters
                .first()
                .and_then(|parameter| parameter.name_span);
            let Some(BoundSlot::Explicit {
                value,
                span: candidate_span,
            }) = bound.slots.into_iter().next()
            else {
                return CallOutcome::Failed;
            };
            let candidate = match value_conversion::convert_target_with_origin(
                &value,
                value_conversion::ConversionTarget::Dictionary,
                candidate_span,
            ) {
                Ok(value_conversion::TargetValue::Value(value)) => value,
                Ok(value_conversion::TargetValue::RawMarkdown { text, .. }) => {
                    let nodes = match self.parse_dynamic_markdown_content(
                        &text,
                        candidate_span,
                        value_conversion::RawMarkdownTarget::Dictionary,
                        diagnostics,
                    ) {
                        Ok(nodes) => nodes,
                        Err(outcome) => {
                            return outcome;
                        }
                    };
                    let entries = match self.evaluate_dictionary_entries(
                        &nodes,
                        candidate_span,
                        diagnostics,
                        context,
                        ".docauthors",
                    ) {
                        Ok(entries) => entries,
                        Err(outcome) => {
                            return outcome;
                        }
                    };
                    IrValue::Dictionary(IrDictionary {
                        entries,
                        span: candidate_span,
                    })
                }
                Err(error) => {
                    diagnostics.push(conversion_failure_diagnostic(
                        value_conversion::ConversionFailure::new(
                            error,
                            Some(candidate_span),
                            Some("authors"),
                            parameter_span,
                            *span,
                        ),
                        Some("`.docauthors`"),
                    ));
                    return CallOutcome::Failed;
                }
            };
            (candidate, candidate_span, parameter_span)
        } else {
            let evaluated_positional = match self.evaluate_invocation_values(
                positional_args,
                span,
                diagnostics,
                context,
                first_origin,
            ) {
                Ok(values) => values,
                Err(outcome) => {
                    return outcome;
                }
            };
            let evaluated_named =
                match self.evaluate_invocation_named(named_args, span, diagnostics, context) {
                    Ok(values) => values,
                    Err(outcome) => {
                        return outcome;
                    }
                };
            let bound = match bind_evaluated_arguments(
                binding_plan,
                evaluated_positional
                    .into_iter()
                    .zip(positional_args.iter())
                    .map(|(value, source)| (value, value_source_span(source, span)))
                    .collect(),
                evaluated_named,
                None,
                *span,
            ) {
                Ok(bound) => bound,
                Err(error) => {
                    diagnostics.push(binding_diagnostic_with_code(error, "E3003"));
                    return CallOutcome::Failed;
                }
            };
            let parameter_span = bound
                .parameters
                .first()
                .and_then(|parameter| parameter.name_span);
            let Some(BoundSlot::Explicit { value, span }) = bound.slots.into_iter().next() else {
                return CallOutcome::Failed;
            };
            (value.value, span, parameter_span)
        };

        let authors = match self.validate_document_authors(
            candidate,
            candidate_span,
            parameter_span,
            *span,
            diagnostics,
        ) {
            Ok(authors) => authors,
            Err(outcome) => {
                return outcome;
            }
        };

        if let Err(error) = context.append_document_authors(authors) {
            diagnostics.push(document_state_conversion_error(error, *span));
            return CallOutcome::Failed;
        }
        CallOutcome::NoValue
    }

    /// Implements the bounded `.dockeywords` iterable read/write contract.
    /// Candidate elements are materialized, converted, and validated before
    /// replacing the shared keyword state. Unlike the author builtins, this
    /// state is deliberately replace-on-write rather than append-only.
    #[allow(clippy::too_many_arguments)]
    fn evaluate_document_keywords_builtin(
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
        if body.is_none() && positional_args.is_empty() && named_args.is_empty() {
            return CallOutcome::Value(context.document_state_value("dockeywords"));
        }
        let Some(binding_plan) = binding_plan else {
            return CallOutcome::Failed;
        };

        let (candidate, candidate_span, parameter_span) = if body.is_some() {
            let raw_body_candidate = match source_backed_body_candidate(
                body.as_ref()
                    .map(|body| call_body_source_span(*body, *span)),
                raw_body,
                "dockeywords",
                diagnostics,
            ) {
                Ok(Some(candidate)) => candidate,
                Ok(None) | Err(_) => {
                    return CallOutcome::Failed;
                }
            };
            let bound = match binding_plan.bind(&[], Some(&raw_body_candidate), *span) {
                Ok(bound) => bound,
                Err(error) => {
                    diagnostics.push(binding_diagnostic_with_code(error, "E3003"));
                    return CallOutcome::Failed;
                }
            };
            let parameter_span = bound
                .parameters
                .first()
                .and_then(|parameter| parameter.name_span);
            let Some(BoundSlot::Explicit { value, span }) = bound.slots.into_iter().next() else {
                return CallOutcome::Failed;
            };
            let values = match self.coerce_iterable(value, &span, diagnostics, context) {
                Ok(values) => values.into_iter().map(|value| (value, span)).collect(),
                Err(outcome) => {
                    return outcome;
                }
            };
            (values, span, parameter_span)
        } else {
            let evaluated_positional = match self.evaluate_invocation_values(
                positional_args,
                span,
                diagnostics,
                context,
                first_origin,
            ) {
                Ok(values) => values,
                Err(outcome) => {
                    return outcome;
                }
            };
            let evaluated_named =
                match self.evaluate_invocation_named(named_args, span, diagnostics, context) {
                    Ok(values) => values,
                    Err(outcome) => {
                        return outcome;
                    }
                };
            let bound = match bind_evaluated_arguments(
                binding_plan,
                evaluated_positional
                    .into_iter()
                    .zip(positional_args.iter())
                    .map(|(value, source)| (value, value_source_span(source, span)))
                    .collect(),
                evaluated_named,
                None,
                *span,
            ) {
                Ok(bound) => bound,
                Err(error) => {
                    diagnostics.push(binding_diagnostic_with_code(error, "E3003"));
                    return CallOutcome::Failed;
                }
            };
            let parameter_span = bound
                .parameters
                .first()
                .and_then(|parameter| parameter.name_span);
            let Some(BoundSlot::Explicit {
                value: argument,
                span: argument_span,
            }) = bound.slots.into_iter().next()
            else {
                return CallOutcome::Failed;
            };
            let values = match self.coerce_iterable(argument, &argument_span, diagnostics, context)
            {
                Ok(values) => values,
                Err(outcome) => {
                    return outcome;
                }
            };
            (
                values
                    .into_iter()
                    .map(|value| {
                        let value_span = value_source_span(&value, &argument_span);
                        (value, value_span)
                    })
                    .collect(),
                argument_span,
                parameter_span,
            )
        };

        let keywords = match self.validate_document_keywords(
            candidate,
            candidate_span,
            parameter_span,
            *span,
            diagnostics,
        ) {
            Ok(keywords) => keywords,
            Err(outcome) => {
                return outcome;
            }
        };
        context.replace_document_keywords(keywords);
        CallOutcome::NoValue
    }

    /// Implements the bounded `.captionposition` document-state setter.
    ///
    /// The upstream function accepts four nullable regular parameters. A
    /// successful invocation contributes only the parameters that are present
    /// and merges them into the current state; omitted and nullable `.none`
    /// values preserve the existing field-specific state. Binding is checked
    /// before any candidate expression is evaluated, and the complete
    /// candidate is committed exactly once.
    #[allow(clippy::too_many_arguments)]
    fn evaluate_caption_position_builtin(
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

        let body_location_candidate = match (body.as_ref(), raw_body) {
            (None, _) => None,
            (Some(_), Some(_raw_body)) => Some(Candidate::Positional {
                value: CaptionPositionArgumentLocation::Body,
                // The candidate's provenance is the containing call. The
                // raw body's own span may belong to a separate dynamic
                // SourceText and is only valid for slicing that source.
                span: *span,
            }),
            (Some(body), None) => {
                diagnostics.push(target_conversion_error_message(
                    "captionposition",
                    call_body_source_span(*body, *span),
                    "requires a source-backed block body".to_string(),
                ));
                return CallOutcome::Failed;
            }
        };

        let bindings = match bind_caption_position_arguments(
            binding_plan,
            positional_args,
            named_args,
            body_location_candidate.as_ref(),
            span,
            diagnostics,
        ) {
            Ok(bindings) => bindings,
            Err(outcome) => return outcome,
        };

        let evaluated_positional = match self.evaluate_invocation_values(
            positional_args,
            span,
            diagnostics,
            context,
            first_origin,
        ) {
            Ok(values) => values,
            Err(outcome) => {
                return outcome;
            }
        };
        let evaluated_named =
            match self.evaluate_invocation_named(named_args, span, diagnostics, context) {
                Ok(values) => values,
                Err(outcome) => {
                    return outcome;
                }
            };

        // Nested argument evaluation shares the document state handle. Use the
        // post-evaluation state as the merge base so a successful inner
        // `.captionposition` mutation is preserved by the outer commit. The
        // invocation checkpoint in the central call boundary is the rollback
        // target for any later conversion failure.
        let mut candidate = context.document_state.borrow().caption_position;
        for (parameter, location) in [
            (CaptionPositionParameter::Default, bindings.default),
            (CaptionPositionParameter::Figures, bindings.figures),
            (CaptionPositionParameter::Tables, bindings.tables),
            (CaptionPositionParameter::CodeBlocks, bindings.code_blocks),
        ] {
            let Some(location) = location else {
                continue;
            };
            let (argument, argument_span, parameter_span) = match location {
                CaptionPositionArgumentLocation::Positional(index) => (
                    evaluated_positional[index].clone(),
                    value_source_span(&positional_args[index], span),
                    None,
                ),
                CaptionPositionArgumentLocation::Named(index) => {
                    let argument = &evaluated_named[index];
                    (
                        InvocationValue {
                            value: argument.value.clone(),
                            origin: argument.origin,
                        },
                        argument.span,
                        Some(argument.name_span),
                    )
                }
                CaptionPositionArgumentLocation::Body => {
                    let Some(raw_body) = raw_body else {
                        return CallOutcome::Failed;
                    };
                    let Some(body_text) = value_conversion::raw_body_dynamic_text(raw_body) else {
                        return CallOutcome::Failed;
                    };
                    (
                        InvocationValue::dynamic_value(IrValue::String(body_text)),
                        *span,
                        None,
                    )
                }
            };

            let value = match argument.value {
                IrValue::None => None,
                _ => match value_conversion::convert_domain_with_origin(
                    &argument,
                    value_conversion::DomainTarget::ClosedEnum(
                        value_conversion::ClosedEnumTarget::CaptionPosition,
                    ),
                ) {
                    Ok(value_conversion::DomainValue::Enum(IrEnumValue::CaptionPosition(
                        value,
                    ))) => Some(value),
                    Ok(_) => {
                        diagnostics.push(document_state_conversion_error(
                            format!(
                                "`.captionposition` {} must be `top` or `bottom`",
                                parameter.name()
                            ),
                            argument_span,
                        ));
                        return CallOutcome::Failed;
                    }
                    Err(error) => {
                        diagnostics.push(conversion_failure_diagnostic_with_detail(
                            value_conversion::ConversionFailure::new(
                                error,
                                Some(argument_span),
                                Some(parameter.name()),
                                parameter_span,
                                *span,
                            ),
                            Some("`.captionposition`"),
                            Some("allowed values are `top` or `bottom`"),
                        ));
                        return CallOutcome::Failed;
                    }
                },
            };

            match parameter {
                CaptionPositionParameter::Default => {
                    if let Some(value) = value {
                        candidate.default = value;
                    }
                }
                CaptionPositionParameter::Figures => {
                    if let Some(value) = value {
                        candidate.figures = Some(value);
                    }
                }
                CaptionPositionParameter::Tables => {
                    if let Some(value) = value {
                        candidate.tables = Some(value);
                    }
                }
                CaptionPositionParameter::CodeBlocks => {
                    if let Some(value) = value {
                        candidate.code_blocks = Some(value);
                    }
                }
            }
        }

        context.set_caption_position(candidate);
        CallOutcome::NoValue
    }

    /// Implements the bounded `.theme` document-state setter.
    ///
    /// Unlike the read/write document metadata builtins, `.theme` is always a
    /// setter. Every invocation replaces the complete theme, including an
    /// empty invocation that commits `Some(IrDocumentTheme { color: None,
    /// layout: None })`. Binding and conversion happen before the single state
    /// commit; nested argument mutations are restored if the invocation fails.
    #[allow(clippy::too_many_arguments)]
    fn evaluate_document_theme_builtin(
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
        let raw_body_candidate = match source_backed_body_candidate(
            body.as_ref()
                .map(|body| call_body_source_span(*body, *span)),
            raw_body,
            "theme",
            diagnostics,
        ) {
            Ok(candidate) => candidate,
            Err(outcome) => return outcome,
        };

        let evaluated_positional = match self.evaluate_invocation_values(
            positional_args,
            span,
            diagnostics,
            context,
            first_origin,
        ) {
            Ok(values) => values,
            Err(outcome) => {
                return outcome;
            }
        };
        let evaluated_named =
            match self.evaluate_invocation_named(named_args, span, diagnostics, context) {
                Ok(values) => values,
                Err(outcome) => {
                    return outcome;
                }
            };

        let candidates = invocation_candidates(
            evaluated_positional
                .into_iter()
                .zip(positional_args.iter())
                .map(|(value, source)| (value, value_source_span(source, span)))
                .collect(),
            evaluated_named,
        );
        let bound = match binding_plan.bind(&candidates, raw_body_candidate.as_ref(), *span) {
            Ok(bound) => bound,
            Err(error) => {
                let message =
                    if let Some(name) = error.message.strip_prefix("unknown named argument ") {
                        format!("Unknown named argument {name} for `.theme`")
                    } else if error.message == "received too many positional arguments" {
                        "`.theme` accepts at most two positional arguments (color and layout)"
                            .to_string()
                    } else if let Some(parameter) = error
                        .message
                        .strip_prefix("parameter ")
                        .and_then(|message| {
                            message.strip_suffix(" collides with an already bound argument")
                        })
                    {
                        format!("`.theme` received the {parameter} argument more than once")
                    } else if let Some(name) = error
                        .message
                        .strip_prefix("named argument `")
                        .and_then(|message| message.strip_suffix("` was supplied more than once"))
                    {
                        format!("`.theme` received the `{name}` argument more than once")
                    } else {
                        error.message.clone()
                    };
                let mut diagnostic = binding_diagnostic_with_code(error, "E3003");
                diagnostic.message = message;
                diagnostics.push(diagnostic);
                return CallOutcome::Failed;
            }
        };
        let parameters = bound.parameters;
        let mut slots = bound.slots.into_iter().enumerate();
        let to_argument = |slot: Option<(usize, BoundSlot<InvocationValue>)>| match slot {
            Some((index, BoundSlot::Explicit { value, span })) => Some((
                value,
                span,
                parameters
                    .get(index)
                    .and_then(|parameter| parameter.name_span),
            )),
            Some((_, BoundSlot::Omitted | BoundSlot::Defaulted)) | None => None,
        };
        let color = to_argument(slots.next());
        let layout = to_argument(slots.next());

        let normalize = |argument: InvocationValue| {
            if matches!(argument.value, IrValue::None) {
                return Ok(None);
            }
            if !matches!(
                argument.value,
                IrValue::String(_)
                    | IrValue::Identifier(_)
                    | IrValue::Number(_)
                    | IrValue::Boolean(_)
            ) {
                return Err(value_conversion::ConversionError::UnsupportedValue {
                    target: value_conversion::ConversionTarget::String,
                });
            }
            builtins::scalar_string_conversion(&argument).map(|value| Some(value.to_lowercase()))
        };

        let color = match color {
            Some((argument, argument_span, parameter_span)) => match normalize(argument) {
                Ok(value) => Some(value),
                Err(error) => {
                    diagnostics.push(conversion_failure_diagnostic(
                        value_conversion::ConversionFailure::new(
                            error,
                            Some(argument_span),
                            Some("color"),
                            parameter_span,
                            *span,
                        ),
                        Some("`.theme`"),
                    ));
                    return CallOutcome::Failed;
                }
            },
            None => None,
        }
        .flatten();
        let layout = match layout {
            Some((argument, argument_span, parameter_span)) => match normalize(argument) {
                Ok(value) => Some(value),
                Err(error) => {
                    diagnostics.push(conversion_failure_diagnostic(
                        value_conversion::ConversionFailure::new(
                            error,
                            Some(argument_span),
                            Some("layout"),
                            parameter_span,
                            *span,
                        ),
                        Some("`.theme`"),
                    ));
                    return CallOutcome::Failed;
                }
            },
            None => None,
        }
        .flatten();

        context.set_document_theme(IrDocumentTheme { color, layout });
        CallOutcome::NoValue
    }

    fn validate_document_keywords(
        &self,
        values: Vec<(IrValue, SourceSpan)>,
        candidate_span: SourceSpan,
        parameter_span: Option<SourceSpan>,
        call_span: SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Result<Vec<String>, CallOutcome> {
        self.check_materialized_elements_len(values.len(), candidate_span, diagnostics)?;
        let mut keywords = Vec::new();
        if let Err(error) = keywords.try_reserve_exact(values.len()) {
            diagnostics.push(document_state_conversion_error(
                format!("document keywords cannot be allocated: {error}"),
                candidate_span,
            ));
            return Err(CallOutcome::Failed);
        }
        for (value, value_span) in values {
            let keyword = match bounded_document_keyword_string(&value) {
                Ok(keyword) => keyword,
                Err(error) => {
                    diagnostics.push(conversion_failure_diagnostic(
                        value_conversion::ConversionFailure::new(
                            error,
                            Some(value_span),
                            Some("keywords"),
                            parameter_span,
                            call_span,
                        ),
                        Some("`.dockeywords`"),
                    ));
                    return Err(CallOutcome::Failed);
                }
            };
            keywords.push(keyword);
        }
        Ok(keywords)
    }

    fn validate_document_authors(
        &self,
        value: IrValue,
        value_span: SourceSpan,
        parameter_span: Option<SourceSpan>,
        call_span: SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Result<Vec<IrDocumentAuthor>, CallOutcome> {
        let candidate_span = value_source_span(&value, &value_span);
        let IrValue::Dictionary(dictionary) = value else {
            diagnostics.push(conversion_failure_diagnostic(
                value_conversion::ConversionFailure::new(
                    value_conversion::ConversionError::UnsupportedValue {
                        target: value_conversion::ConversionTarget::Dictionary,
                    },
                    Some(candidate_span),
                    Some("authors"),
                    parameter_span,
                    call_span,
                ),
                Some("`.docauthors`"),
            ));
            return Err(CallOutcome::Failed);
        };
        self.check_materialized_elements_len(dictionary.entries.len(), value_span, diagnostics)?;
        let mut authors = Vec::new();
        if let Err(error) = authors.try_reserve_exact(dictionary.entries.len()) {
            diagnostics.push(document_state_conversion_error(
                format!("document authors cannot be allocated: {error}"),
                value_span,
            ));
            return Err(CallOutcome::Failed);
        }

        for pair in dictionary.entries {
            let author_name = match pair.first.as_ref() {
                IrValue::String(name) if !name.is_empty() => name.clone(),
                IrValue::String(_) => {
                    diagnostics.push(document_state_conversion_error(
                        "`.docauthors` author keys must be non-empty strings".to_string(),
                        value_source_span(pair.first.as_ref(), &pair.span),
                    ));
                    return Err(CallOutcome::Failed);
                }
                _ => {
                    diagnostics.push(conversion_failure_diagnostic(
                        value_conversion::ConversionFailure::new(
                            value_conversion::ConversionError::UnsupportedValue {
                                target: value_conversion::ConversionTarget::String,
                            },
                            Some(value_source_span(pair.first.as_ref(), &pair.span)),
                            Some("authors"),
                            parameter_span,
                            call_span,
                        ),
                        Some("`.docauthors`"),
                    ));
                    return Err(CallOutcome::Failed);
                }
            };
            let IrValue::Dictionary(info_dictionary) = pair.second.as_ref() else {
                diagnostics.push(conversion_failure_diagnostic(
                    value_conversion::ConversionFailure::new(
                        value_conversion::ConversionError::UnsupportedValue {
                            target: value_conversion::ConversionTarget::Dictionary,
                        },
                        Some(value_source_span(pair.second.as_ref(), &pair.span)),
                        Some("authors"),
                        parameter_span,
                        call_span,
                    ),
                    Some("`.docauthors`"),
                ));
                return Err(CallOutcome::Failed);
            };
            self.check_materialized_elements_len(
                info_dictionary.entries.len(),
                pair.span,
                diagnostics,
            )?;
            let mut info = Vec::new();
            if let Err(error) = info.try_reserve_exact(info_dictionary.entries.len()) {
                diagnostics.push(document_state_conversion_error(
                    format!("author information cannot be allocated: {error}"),
                    pair.span,
                ));
                return Err(CallOutcome::Failed);
            }
            for info_pair in &info_dictionary.entries {
                let info_name = match info_pair.first.as_ref() {
                    IrValue::String(name) if !name.is_empty() => name.clone(),
                    IrValue::String(_) => {
                        diagnostics.push(document_state_conversion_error(
                            "`.docauthors` information keys must be non-empty strings".to_string(),
                            value_source_span(info_pair.first.as_ref(), &info_pair.span),
                        ));
                        return Err(CallOutcome::Failed);
                    }
                    _ => {
                        diagnostics.push(conversion_failure_diagnostic(
                            value_conversion::ConversionFailure::new(
                                value_conversion::ConversionError::UnsupportedValue {
                                    target: value_conversion::ConversionTarget::String,
                                },
                                Some(value_source_span(info_pair.first.as_ref(), &info_pair.span)),
                                Some("authors"),
                                parameter_span,
                                call_span,
                            ),
                            Some("`.docauthors`"),
                        ));
                        return Err(CallOutcome::Failed);
                    }
                };
                let info_value_span = value_source_span(info_pair.second.as_ref(), &info_pair.span);
                let info_value = match bounded_document_author_string(info_pair.second.as_ref()) {
                    Ok(value) => value,
                    Err(error) => {
                        diagnostics.push(conversion_failure_diagnostic(
                            value_conversion::ConversionFailure::new(
                                error,
                                Some(info_value_span),
                                Some("authors"),
                                parameter_span,
                                call_span,
                            ),
                            Some("`.docauthors`"),
                        ));
                        return Err(CallOutcome::Failed);
                    }
                };
                upsert_ordered_string_pair(&mut info, info_name, info_value);
            }
            upsert_ordered_author(
                &mut authors,
                IrDocumentAuthor {
                    name: author_name,
                    info,
                },
            );
        }
        Ok(authors)
    }

    fn document_authors_value(
        &self,
        span: SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &EvaluationContext<'_>,
    ) -> CallOutcome {
        let authors = context.document_authors_snapshot();
        if let Err(outcome) = self.check_materialized_elements_len(authors.len(), span, diagnostics)
        {
            return outcome;
        }
        let mut entries = Vec::new();
        if let Err(error) = entries.try_reserve_exact(authors.len()) {
            diagnostics.push(document_state_conversion_error(
                format!("document author dictionary cannot be allocated: {error}"),
                span,
            ));
            return CallOutcome::Failed;
        }

        for author in authors {
            if let Err(outcome) =
                self.check_materialized_elements_len(author.info.len(), span, diagnostics)
            {
                return outcome;
            }
            let mut info_entries = Vec::new();
            if let Err(error) = info_entries.try_reserve_exact(author.info.len()) {
                diagnostics.push(document_state_conversion_error(
                    format!("author information dictionary cannot be allocated: {error}"),
                    span,
                ));
                return CallOutcome::Failed;
            }
            for (key, value) in author.info {
                upsert_ordered_pair(
                    &mut info_entries,
                    IrPair {
                        first: Box::new(IrValue::String(key)),
                        second: Box::new(IrValue::String(value)),
                        span,
                    },
                );
            }
            upsert_ordered_pair(
                &mut entries,
                IrPair {
                    first: Box::new(IrValue::String(author.name)),
                    second: Box::new(IrValue::Dictionary(IrDictionary {
                        entries: info_entries,
                        span,
                    })),
                    span,
                },
            );
        }
        CallOutcome::Value(IrValue::Dictionary(IrDictionary { entries, span }))
    }

    /// Evaluates the closed Quarkdown `.html(content: String)` builtin.
    ///
    /// The result is kept in an ordinary content value so the existing block
    /// and inline materialization paths preserve placement independently.
    #[allow(clippy::too_many_arguments)]
    fn evaluate_html(
        &self,
        positional_args: &[IrValue],
        named_args: &[IrNamedArg],
        body: Option<CallBody<'_>>,
        raw_body: Option<&IrRawBody>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
        binding_plan: Option<&BindingPlan>,
    ) -> CallOutcome {
        if !self.capabilities.allows(Capability::NativeContent) {
            diagnostics.push(native_content_denied(*span));
            return CallOutcome::Failed;
        }

        let Some(binding_plan) = binding_plan else {
            return CallOutcome::Failed;
        };
        let candidates = raw_invocation_locations(positional_args, named_args, *span);
        let body_candidate = body.map(|body| Candidate::Positional {
            value: InvocationArgumentLocation::Body,
            span: call_body_source_span(body, *span),
        });
        let bound = match binding_plan.bind(&candidates, body_candidate.as_ref(), *span) {
            Ok(bound) => bound,
            Err(error) => {
                let mut diagnostic = binding_diagnostic_with_code(error, "E3003");
                diagnostic.message = native_binding_message("html", diagnostic.message);
                diagnostics.push(diagnostic);
                return CallOutcome::Failed;
            }
        };
        let Some(BoundSlot::Explicit {
            value: location, ..
        }) = bound.slots.into_iter().next()
        else {
            return CallOutcome::Failed;
        };

        let content = match location {
            InvocationArgumentLocation::Body => {
                let Some(body) = body else {
                    return CallOutcome::Failed;
                };
                match self.evaluate_html_body(body, raw_body, span, diagnostics, context) {
                    CallOutcome::Value(value) => value,
                    outcome => return outcome,
                }
            }
            InvocationArgumentLocation::Positional(index) => {
                let Some(value) = positional_args.get(index) else {
                    return CallOutcome::Failed;
                };
                match self.evaluate_value(value, diagnostics, context) {
                    CallOutcome::Value(value) => value,
                    CallOutcome::Unresolved => {
                        match self.preserve_value_expression(value, diagnostics, context) {
                            Ok(value) => value,
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
            InvocationArgumentLocation::Named(index) => {
                let Some(argument) = named_args.get(index) else {
                    return CallOutcome::Failed;
                };
                match self.evaluate_value(&argument.value, diagnostics, context) {
                    CallOutcome::Value(value) => value,
                    CallOutcome::Unresolved => {
                        match self.preserve_value_expression(&argument.value, diagnostics, context)
                        {
                            Ok(value) => value,
                            Err(outcome) => return outcome,
                        }
                    }
                    CallOutcome::NoValue => {
                        diagnostics
                            .push(no_value_required(value_source_span(&argument.value, span)));
                        return CallOutcome::Failed;
                    }
                    CallOutcome::Failed => return CallOutcome::Failed,
                }
            }
        };

        let Some(content) = builtins::adapt_string_argument(&content) else {
            diagnostics.push(html_argument_error(
                "`.html` content must adapt to the supported String boundary",
                *span,
            ));
            return CallOutcome::Failed;
        };

        CallOutcome::Value(IrValue::Content(vec![IrNode::TargetSpecificContent {
            content: TargetSpecificContent {
                target: NativeTarget::Html,
                content,
                span: *span,
            },
        }]))
    }

    fn evaluate_html_body(
        &self,
        body: CallBody<'_>,
        raw_body: Option<&IrRawBody>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
    ) -> CallOutcome {
        if let Some(raw_body) = raw_body {
            return match raw_native_body_string_value(raw_body) {
                Ok(value) => CallOutcome::Value(IrValue::String(value)),
                Err(error) => {
                    diagnostics.push(target_conversion_error("`.html` body", *span, error));
                    CallOutcome::Failed
                }
            };
        }
        match body {
            CallBody::Block(nodes) if body_contains_raw_html(nodes) => {
                match opaque_html_body_string(nodes) {
                    Some(content) => CallOutcome::Value(IrValue::String(content)),
                    None => {
                        diagnostics.push(html_argument_error(
                            "`.html` body contains structure that cannot adapt to String",
                            *span,
                        ));
                        CallOutcome::Failed
                    }
                }
            }
            body => self.evaluate_call_body(body, span, diagnostics, context),
        }
    }

    /// Evaluates Quarkdown's raw native Markdown-content builtin. This is
    /// intentionally not a file loader: the v2.5.1 contract accepts Markdown
    /// content and returns an opaque native-content node.
    #[allow(clippy::too_many_arguments)]
    fn evaluate_markdown(
        &self,
        positional_args: &[IrValue],
        named_args: &[IrNamedArg],
        body: Option<CallBody<'_>>,
        raw_body: Option<&IrRawBody>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
        binding_plan: Option<&BindingPlan>,
    ) -> CallOutcome {
        if !self.capabilities.allows(Capability::NativeContent) {
            diagnostics.push(Diagnostic {
                code: "E3004".to_string(),
                severity: Severity::Error,
                message: "NativeContent capability is required for `.markdown`".to_string(),
                primary: Some(*span),
                secondary: Vec::new(),
                hints: vec![
                    "Grant the NativeContent capability for this compilation to enable `.markdown`."
                        .to_string(),
                ],
            });
            return CallOutcome::Failed;
        }
        let Some(binding_plan) = binding_plan else {
            return CallOutcome::Failed;
        };
        let candidates = raw_invocation_locations(positional_args, named_args, *span);
        let body_candidate = body.map(|body| Candidate::Positional {
            value: InvocationArgumentLocation::Body,
            span: call_body_source_span(body, *span),
        });
        let bound = match binding_plan.bind(&candidates, body_candidate.as_ref(), *span) {
            Ok(bound) => bound,
            Err(error) => {
                let mut diagnostic = binding_diagnostic_with_code(error, "E3003");
                diagnostic.message = native_binding_message("markdown", diagnostic.message);
                diagnostics.push(diagnostic);
                return CallOutcome::Failed;
            }
        };
        let Some(BoundSlot::Explicit {
            value: location, ..
        }) = bound.slots.into_iter().next()
        else {
            return CallOutcome::Failed;
        };
        let content = match location {
            InvocationArgumentLocation::Body => {
                let Some(body) = body else {
                    return CallOutcome::Failed;
                };
                if let Some(raw_body) = raw_body {
                    let Some(body_text) = value_conversion::raw_body_dynamic_text(raw_body) else {
                        return CallOutcome::Failed;
                    };
                    IrValue::String(body_text)
                } else {
                    match self.evaluate_call_body(body, span, diagnostics, context) {
                        CallOutcome::Value(value) => value,
                        outcome => return outcome,
                    }
                }
            }
            InvocationArgumentLocation::Positional(index) => {
                let Some(value) = positional_args.get(index) else {
                    return CallOutcome::Failed;
                };
                match self.evaluate_value(value, diagnostics, context) {
                    CallOutcome::Value(value) => value,
                    CallOutcome::Unresolved => {
                        match self.preserve_value_expression(value, diagnostics, context) {
                            Ok(value) => value,
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
            InvocationArgumentLocation::Named(index) => {
                let Some(argument) = named_args.get(index) else {
                    return CallOutcome::Failed;
                };
                match self.evaluate_value(&argument.value, diagnostics, context) {
                    CallOutcome::Value(value) => value,
                    CallOutcome::Unresolved => {
                        match self.preserve_value_expression(&argument.value, diagnostics, context)
                        {
                            Ok(value) => value,
                            Err(outcome) => return outcome,
                        }
                    }
                    CallOutcome::NoValue => {
                        diagnostics
                            .push(no_value_required(value_source_span(&argument.value, span)));
                        return CallOutcome::Failed;
                    }
                    CallOutcome::Failed => return CallOutcome::Failed,
                }
            }
        };
        let Some(content) = builtins::adapt_string_argument(&content) else {
            diagnostics.push(resource_diagnostic(
                "E3003",
                "`.markdown` content must adapt to the supported String boundary".to_string(),
                *span,
                "Rich semantic values are not silently rendered into native Markdown text.",
            ));
            return CallOutcome::Failed;
        };
        CallOutcome::Value(IrValue::Content(vec![IrNode::TargetSpecificContent {
            content: TargetSpecificContent {
                target: NativeTarget::Markdown,
                content,
                span: *span,
            },
        }]))
    }

    /// Evaluates the resource-backed subset of the Quarkdown standard library.
    ///
    /// Resource access is deliberately routed through the host-supplied
    /// semantic provider. The evaluator never receives a native path and
    /// never performs filesystem or network I/O itself.
    #[allow(clippy::too_many_arguments)]
    fn evaluate_resource_builtin(
        &self,
        name: &str,
        positional_args: &[IrValue],
        named_args: &[IrNamedArg],
        _body: Option<CallBody<'_>>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
        binding_plan: Option<&BindingPlan>,
    ) -> CallOutcome {
        let Some(binding_plan) = binding_plan else {
            return CallOutcome::Failed;
        };

        let evaluated_positional =
            match self.evaluate_values(positional_args, span, diagnostics, context) {
                Ok(values) => values,
                Err(outcome) => return outcome,
            };
        let evaluated_named = match self.evaluate_named(named_args, span, diagnostics, context) {
            Ok(values) => values,
            Err(outcome) => return outcome,
        };

        match name {
            "read" => self.evaluate_read(
                &evaluated_positional,
                &evaluated_named,
                span,
                diagnostics,
                context,
                binding_plan,
            ),
            "json" => self.evaluate_json(
                &evaluated_positional,
                &evaluated_named,
                span,
                diagnostics,
                context,
                binding_plan,
            ),
            "include" => self.evaluate_include(
                &evaluated_positional,
                &evaluated_named,
                span,
                diagnostics,
                context,
                binding_plan,
            ),
            _ => CallOutcome::Failed,
        }
    }

    fn evaluate_read(
        &self,
        positional_args: &[IrValue],
        named_args: &[IrNamedArg],
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &EvaluationContext<'_>,
        binding_plan: &BindingPlan,
    ) -> CallOutcome {
        let Some(reference) = resource_path_argument(
            "read",
            positional_args,
            named_args,
            binding_plan,
            span,
            diagnostics,
        ) else {
            return CallOutcome::Failed;
        };
        let lines = match resource_lines_argument(named_args, span, diagnostics) {
            Ok(lines) => lines,
            Err(()) => return CallOutcome::Failed,
        };
        let Some((provider, source_id)) = resource_context(context, span, diagnostics) else {
            return CallOutcome::Failed;
        };
        let ResourceText { path, text } = match provider.read_text(source_id, &reference) {
            Ok(value) => value,
            Err(error) => {
                diagnostics.push(resource_access_diagnostic("read", error, *span));
                return CallOutcome::Failed;
            }
        };
        let value = match lines {
            None => normalize_line_separators(&text),
            Some(range) => match select_lines(&text, range) {
                Ok(value) => value,
                Err(message) => {
                    diagnostics.push(resource_diagnostic(
                        "E3001",
                        format!("`.read` cannot select lines from `{path}`: {message}"),
                        *span,
                        "Use a one-based, inclusive line range within the resource.",
                    ));
                    return CallOutcome::Failed;
                }
            },
        };
        CallOutcome::Value(IrValue::String(value))
    }

    fn evaluate_json(
        &self,
        positional_args: &[IrValue],
        named_args: &[IrNamedArg],
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &EvaluationContext<'_>,
        binding_plan: &BindingPlan,
    ) -> CallOutcome {
        let Some(reference) = resource_path_argument(
            "json",
            positional_args,
            named_args,
            binding_plan,
            span,
            diagnostics,
        ) else {
            return CallOutcome::Failed;
        };
        let Some((provider, source_id)) = resource_context(context, span, diagnostics) else {
            return CallOutcome::Failed;
        };
        let ResourceText { path, text } = match provider.read_text(source_id, &reference) {
            Ok(value) => value,
            Err(error) => {
                diagnostics.push(resource_access_diagnostic("json", error, *span));
                return CallOutcome::Failed;
            }
        };
        let value = match serde_json::from_str::<serde_json::Value>(&text) {
            Ok(value) => value,
            Err(error) => {
                diagnostics.push(resource_diagnostic(
                    "E3001",
                    format!("`.json` could not parse `{path}`: {error}"),
                    *span,
                    "Provide valid UTF-8 JSON in the logical project resource.",
                ));
                return CallOutcome::Failed;
            }
        };
        match json_value_to_ir(&value, *span) {
            Ok(value) => CallOutcome::Value(value),
            Err(message) => {
                diagnostics.push(resource_diagnostic(
                    "E3001",
                    format!("`.json` value in `{path}` is unsupported: {message}"),
                    *span,
                    "Use JSON values representable by Arkst's typed evaluator model.",
                ));
                CallOutcome::Failed
            }
        }
    }

    fn evaluate_include(
        &self,
        positional_args: &[IrValue],
        named_args: &[IrNamedArg],
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
        binding_plan: &BindingPlan,
    ) -> CallOutcome {
        let Some(reference) = resource_path_argument(
            "include",
            positional_args,
            named_args,
            binding_plan,
            span,
            diagnostics,
        ) else {
            return CallOutcome::Failed;
        };
        let sandbox = match include_sandbox_argument(named_args, span, diagnostics) {
            Ok(sandbox) => sandbox,
            Err(()) => return CallOutcome::Failed,
        };
        let Some((provider, source_id)) = resource_context(context, span, diagnostics) else {
            return CallOutcome::Failed;
        };
        let IncludedSource {
            path,
            source_id: target_id,
            text: source,
        } = match provider.read_source(source_id, &reference) {
            Ok(source) => source,
            Err(ResourceAccessError::NotFound { path }) => {
                diagnostics.push(resource_diagnostic(
                    "E3001",
                    format!("`.include` resource not found: `{path}`"),
                    *span,
                    "Add the target source to the VirtualProject supplied by the host.",
                ));
                return CallOutcome::Failed;
            }
            Err(error) => {
                diagnostics.push(resource_access_diagnostic("include", error, *span));
                return CallOutcome::Failed;
            }
        };
        if let Some(position) = context
            .active_sources
            .iter()
            .position(|id| *id == target_id)
        {
            let mut chain = context.active_sources[position..]
                .iter()
                .filter_map(|id| provider.source_path(*id))
                .collect::<Vec<_>>();
            chain.push(path.to_string());
            diagnostics.push(resource_diagnostic(
                "E3001",
                format!("`.include` cycle detected: {}", chain.join(" -> ")),
                *span,
                "An active include may not include a source already on its call stack.",
            ));
            return CallOutcome::Failed;
        }

        let mode = source_mode_for_resource_path(&path);
        let include_diagnostics_start = diagnostics.len();
        let parsed = arkst_markdown::parse_with_mode(&source, mode);
        for diagnostic in parsed.diagnostics {
            diagnostics.push(Diagnostic {
                code: diagnostic.code.to_string(),
                severity: Severity::Error,
                message: diagnostic.message,
                primary: Some(SourceSpan {
                    source_id: target_id,
                    start: diagnostic.span.start,
                    end: diagnostic.span.end,
                }),
                secondary: Vec::new(),
                hints: Vec::new(),
            });
        }
        if diagnostics.len() != include_diagnostics_start {
            return CallOutcome::Failed;
        }
        let (document, lowering_diagnostics) = ast_to_ir::ast_to_ir_with_diagnostics_for_mode(
            &parsed.document,
            target_id,
            &context.metadata_defaults,
            mode,
        );
        diagnostics.extend(lowering_diagnostics);
        if diagnostics.len() != include_diagnostics_start {
            return CallOutcome::Failed;
        }

        let previous_source = context.current_source;
        let previous_active = context.active_sources.clone();
        context.active_sources.push(target_id);
        let evaluation_diagnostics_start = diagnostics.len();
        let result = match sandbox {
            IncludeSandbox::Share => {
                context.current_source = Some(target_id);
                let result = self.evaluate_nodes(&document.nodes, diagnostics, context);
                context.current_source = previous_source;
                result
            }
            IncludeSandbox::Scope | IncludeSandbox::Subdocument => {
                let mut child = context.ephemeral_child();
                child.current_source = Some(target_id);
                child.active_sources = context.active_sources.clone();
                self.evaluate_nodes(&document.nodes, diagnostics, &mut child)
            }
        };
        context.active_sources = previous_active;
        if diagnostics.len() != evaluation_diagnostics_start {
            return CallOutcome::Failed;
        }
        CallOutcome::Value(IrValue::Content(result))
    }

    /// Evaluates the bounded Collection access operations through the same
    /// ordered semantic element adaptation used by `.foreach`.
    #[allow(clippy::too_many_arguments)]
    fn evaluate_collection_access(
        &self,
        name: &str,
        positional_args: &[InvocationValue],
        named_args: &[InvocationNamedArg],
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
        binding_plan: &BindingPlan,
    ) -> CallOutcome {
        match name {
            "size" | "first" | "second" | "third" | "last" | "sumall" | "average" | "distinct"
            | "reversed" | "groupvalues" => {
                let named_parameter = if name == "size" { "of" } else { "from" };
                let value = match collection_access_operand(
                    name,
                    named_parameter,
                    positional_args,
                    named_args,
                    binding_plan,
                    span,
                    diagnostics,
                ) {
                    Ok(value) => value,
                    Err(outcome) => return outcome,
                };
                let elements = match self.coerce_iterable(value, span, diagnostics, context) {
                    Ok(elements) => elements,
                    Err(outcome) => return outcome,
                };
                match name {
                    "size" => match exact_collection_length(elements.len(), span, diagnostics) {
                        Ok(length) => CallOutcome::Value(IrValue::Number(length)),
                        Err(outcome) => outcome,
                    },
                    "first" => {
                        CallOutcome::Value(elements.first().cloned().unwrap_or(IrValue::None))
                    }
                    "second" => {
                        CallOutcome::Value(elements.get(1).cloned().unwrap_or(IrValue::None))
                    }
                    "third" => {
                        CallOutcome::Value(elements.get(2).cloned().unwrap_or(IrValue::None))
                    }
                    "last" => CallOutcome::Value(elements.last().cloned().unwrap_or(IrValue::None)),
                    "sumall" => CallOutcome::Value(IrValue::Number(collection_sum_all(&elements))),
                    "average" => match collection_average(&elements, span, diagnostics) {
                        Ok(average) => CallOutcome::Value(IrValue::Number(average)),
                        Err(outcome) => outcome,
                    },
                    "distinct" => distinct_collection_values(elements, *span, diagnostics),
                    "reversed" => {
                        let mut reversed = elements;
                        reversed.reverse();
                        CallOutcome::Value(IrValue::Collection(reversed))
                    }
                    "groupvalues" => group_collection_values(elements, *span, diagnostics),
                    _ => unreachable!("collection access operation was prevalidated"),
                }
            }
            "getat" => {
                let (value, index, fallback) = match getat_operands(
                    positional_args,
                    named_args,
                    binding_plan,
                    span,
                    diagnostics,
                ) {
                    Ok(operands) => operands,
                    Err(outcome) => return outcome,
                };
                let elements = match self.coerce_iterable(value, span, diagnostics, context) {
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
        _body: Option<CallBody<'_>>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
        binding_plan: Option<&BindingPlan>,
    ) -> CallOutcome {
        let Some(binding_plan) = binding_plan else {
            return CallOutcome::Failed;
        };
        let raw_candidates = raw_invocation_candidates(positional_args, named_args, *span);
        let bound = match binding_plan.bind(&raw_candidates, None, *span) {
            Ok(bound) => bound,
            Err(error) => {
                diagnostics.push(binding_diagnostic_with_code(error, "E3001"));
                return CallOutcome::Failed;
            }
        };
        let values = bound
            .slots
            .into_iter()
            .filter_map(|slot| match slot {
                BoundSlot::Explicit { value, .. } => Some(value),
                BoundSlot::Omitted | BoundSlot::Defaulted => None,
            })
            .collect::<Vec<_>>();
        let values = match self.evaluate_values(&values, span, diagnostics, context) {
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
        _positional_args: &[IrValue],
        _named_args: &[IrNamedArg],
        body: Option<CallBody<'_>>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
        binding_plan: Option<&BindingPlan>,
    ) -> CallOutcome {
        let Some(binding_plan) = binding_plan else {
            return CallOutcome::Failed;
        };
        if binding_plan
            .bind::<InvocationValue>(&[], None, *span)
            .is_err()
        {
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
        let entries = match self.evaluate_dictionary_entries(
            body,
            *span,
            diagnostics,
            context,
            ".dictionary",
        ) {
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
        context: &mut EvaluationContext<'_>,
        function_name: &str,
    ) -> Result<Vec<IrPair>, CallOutcome> {
        let list = match nodes {
            [] => return Ok(Vec::new()),
            [IrNode::UnorderedList { items, .. }] | [IrNode::OrderedList { items, .. }] => items,
            _ => {
                diagnostics.push(iteration_error(
                    format!("`{function_name}` requires exactly one Markdown list body"),
                    span,
                ));
                return Err(CallOutcome::Failed);
            }
        };
        self.check_materialized_elements_len(list.len(), span, diagnostics)?;
        let mut entries = Vec::new();
        if let Err(error) = entries.try_reserve_exact(list.len()) {
            diagnostics.push(iteration_error(
                format!("dictionary entries cannot be allocated: {error}"),
                span,
            ));
            return Err(CallOutcome::Failed);
        }
        for item in list {
            let (key, value) =
                self.dictionary_item_parts(item, span, diagnostics, context, function_name)?;
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
        context: &mut EvaluationContext<'_>,
        function_name: &str,
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
            } else if let Some(key) = plain_dictionary_key(content) {
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
                IrValue::Dictionary(IrDictionary {
                    entries: Vec::new(),
                    span: item.span,
                })
            } else {
                let nested = self.evaluate_dictionary_entries(
                    nested,
                    item.span,
                    diagnostics,
                    context,
                    function_name,
                )?;
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
        context: &mut EvaluationContext<'_>,
        binding_plan: Option<&BindingPlan>,
    ) -> CallOutcome {
        let Some(binding_plan) = binding_plan else {
            return CallOutcome::Failed;
        };
        let raw_candidates = raw_invocation_candidates(positional_args, named_args, *span);
        let bound = match binding_plan.bind(&raw_candidates, None, *span) {
            Ok(bound) => bound,
            Err(error) => {
                diagnostics.push(binding_diagnostic_with_code(error, "E3003"));
                return CallOutcome::Failed;
            }
        };
        let Some(BoundSlot::Explicit { value, .. }) = bound.slots.into_iter().next() else {
            return CallOutcome::Failed;
        };
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

        let value = match self.evaluate_value(&value, diagnostics, context) {
            CallOutcome::Value(value) => value,
            CallOutcome::Unresolved => {
                match self.preserve_value_expression(&value, diagnostics, context) {
                    Ok(value) => value,
                    Err(outcome) => return outcome,
                }
            }
            CallOutcome::NoValue => {
                diagnostics.push(no_value_required(value_source_span(&value, span)));
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
        context: &mut EvaluationContext<'_>,
        binding_plan: Option<&BindingPlan>,
        first_origin: Option<ValueOrigin>,
    ) -> CallOutcome {
        let Some(binding_plan) = binding_plan else {
            return CallOutcome::Failed;
        };
        let raw_candidates = raw_invocation_candidates(positional_args, named_args, *span);
        let bound = match binding_plan.bind(&raw_candidates, None, *span) {
            Ok(bound) => bound,
            Err(error) => {
                diagnostics.push(binding_diagnostic_with_code(error, "E3001"));
                return CallOutcome::Failed;
            }
        };
        let mut slots = bound.slots.into_iter();
        let Some(BoundSlot::Explicit {
            value: iterable_argument,
            ..
        }) = slots.next()
        else {
            return CallOutcome::Failed;
        };
        let callback_argument = match slots.next() {
            Some(BoundSlot::Explicit { value, .. }) => Some(value),
            Some(BoundSlot::Omitted | BoundSlot::Defaulted) | None => None,
        };
        let iteration_body = match body {
            Some(CallBody::Block(nodes)) => IterationBody::Block(nodes),
            Some(CallBody::Inline(_)) => {
                diagnostics.push(iteration_error(
                    "`.foreach` does not accept an ordinary inline content body".to_string(),
                    *span,
                ));
                return CallOutcome::Failed;
            }
            None => {
                let Some(callback) = callback_argument else {
                    diagnostics.push(iteration_error(
                        "`.foreach` requires a block or inline callable body".to_string(),
                        *span,
                    ));
                    return CallOutcome::Failed;
                };
                let callable = match callback {
                    IrValue::Callable(callable) => callable,
                    IrValue::InlineBody(IrInlineBody {
                        parameters,
                        body,
                        span,
                        ..
                    }) => IrCallable {
                        parameters,
                        body,
                        span,
                        capture: None,
                    },
                    value => {
                        diagnostics.push(iteration_error(
                            "`.foreach` inline body must be a callable".to_string(),
                            value_source_span(&value, span),
                        ));
                        return CallOutcome::Failed;
                    }
                };
                IterationBody::Inline(callable)
            }
        };
        let callable_parameters = match &iteration_body {
            IterationBody::Block(_) => lambda_parameters,
            IterationBody::Inline(callable) => callable.parameters.as_deref(),
        };
        if !validate_iteration_lambda(callable_parameters, ".foreach", true, span, diagnostics) {
            return CallOutcome::Failed;
        }

        let value = match self
            .evaluate_invocation_values(
                std::slice::from_ref(&iterable_argument),
                span,
                diagnostics,
                context,
                first_origin,
            )
            .and_then(|mut values| values.pop().ok_or(CallOutcome::Failed))
        {
            Ok(value) => value,
            Err(outcome) => return outcome,
        };
        let elements = match self.coerce_iterable(value, span, diagnostics, context) {
            Ok(elements) => elements,
            Err(outcome) => return outcome,
        };
        let callable = match self.materialize_iteration_callable(
            iteration_body,
            lambda_parameters,
            *span,
            diagnostics,
            context,
        ) {
            Ok(callable) => callable,
            Err(outcome) => return outcome,
        };
        self.map_callable_values(
            &elements,
            &callable,
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
        context: &mut EvaluationContext<'_>,
        binding_plan: Option<&BindingPlan>,
    ) -> CallOutcome {
        let Some(binding_plan) = binding_plan else {
            return CallOutcome::Failed;
        };
        let raw_candidates = raw_invocation_candidates(positional_args, named_args, *span);
        let bound = match binding_plan.bind(&raw_candidates, None, *span) {
            Ok(bound) => bound,
            Err(error) => {
                diagnostics.push(binding_diagnostic_with_code(error, "E3001"));
                return CallOutcome::Failed;
            }
        };
        let mut slots = bound.slots.into_iter();
        let Some(BoundSlot::Explicit {
            value: count_argument,
            ..
        }) = slots.next()
        else {
            return CallOutcome::Failed;
        };
        let callback_argument = match slots.next() {
            Some(BoundSlot::Explicit { value, .. }) => Some(value),
            Some(BoundSlot::Omitted | BoundSlot::Defaulted) | None => None,
        };
        let iteration_body = match body {
            Some(CallBody::Block(nodes)) => IterationBody::Block(nodes),
            Some(CallBody::Inline(_)) => {
                diagnostics.push(iteration_error(
                    "`.repeat` does not accept an ordinary inline content body".to_string(),
                    *span,
                ));
                return CallOutcome::Failed;
            }
            None => {
                let Some(callback) = callback_argument else {
                    diagnostics.push(iteration_error(
                        "`.repeat` requires a block or inline callable body".to_string(),
                        *span,
                    ));
                    return CallOutcome::Failed;
                };
                let callable = match callback {
                    IrValue::Callable(callable) => callable,
                    IrValue::InlineBody(IrInlineBody {
                        parameters,
                        body,
                        span,
                        ..
                    }) => IrCallable {
                        parameters,
                        body,
                        span,
                        capture: None,
                    },
                    value => {
                        diagnostics.push(iteration_error(
                            "`.repeat` inline body must be a callable".to_string(),
                            value_source_span(&value, span),
                        ));
                        return CallOutcome::Failed;
                    }
                };
                IterationBody::Inline(callable)
            }
        };
        let callable_parameters = match &iteration_body {
            IterationBody::Block(_) => lambda_parameters,
            IterationBody::Inline(callable) => callable.parameters.as_deref(),
        };
        if !validate_iteration_lambda(callable_parameters, ".repeat", false, span, diagnostics) {
            return CallOutcome::Failed;
        }

        let count_value = match self.evaluate_value(&count_argument, diagnostics, context) {
            CallOutcome::Value(value) => value,
            CallOutcome::Unresolved => {
                match self.preserve_value_expression(&count_argument, diagnostics, context) {
                    Ok(value) => value,
                    Err(outcome) => return outcome,
                }
            }
            CallOutcome::NoValue => {
                diagnostics.push(no_value_required(value_source_span(&count_argument, span)));
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
        let callable = match self.materialize_iteration_callable(
            iteration_body,
            lambda_parameters,
            *span,
            diagnostics,
            context,
        ) {
            Ok(callable) => callable,
            Err(outcome) => return outcome,
        };
        self.map_callable_values(
            &elements,
            &callable,
            IterationOptions {
                span: *span,
                allow_destructuring: false,
            },
            diagnostics,
            context,
        )
    }

    /// Evaluates `.map`, `.filter`, and `.sorted` through the same typed
    /// iterable and callable machinery used by `.foreach`.
    #[allow(clippy::too_many_arguments)]
    fn evaluate_collection_transform(
        &self,
        name: &str,
        positional_args: &[IrValue],
        named_args: &[IrNamedArg],
        body: Option<CallBody<'_>>,
        lambda_parameters: Option<&[IrParameter]>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
        binding_plan: Option<&BindingPlan>,
        first_origin: Option<ValueOrigin>,
    ) -> CallOutcome {
        let (raw_collection, raw_callback) = match transform_operands(
            name,
            positional_args,
            named_args,
            body.is_some(),
            binding_plan,
            *span,
            diagnostics,
        ) {
            Ok(operands) => operands,
            Err(outcome) => return outcome,
        };
        let collection = match self
            .evaluate_invocation_values(
                std::slice::from_ref(&raw_collection),
                span,
                diagnostics,
                context,
                first_origin,
            )
            .and_then(|mut values| values.pop().ok_or(CallOutcome::Failed))
        {
            Ok(value) => value,
            Err(outcome) => return outcome,
        };
        let elements = match self.coerce_iterable(collection, span, diagnostics, context) {
            Ok(elements) => elements,
            Err(outcome) => return outcome,
        };

        let callable = match body {
            Some(CallBody::Block(nodes)) => {
                Some(self.make_callable(lambda_parameters, nodes, *span, context))
            }
            Some(CallBody::Inline(_)) => {
                diagnostics.push(iteration_error(
                    format!("`.{name}` requires a block or first-class lambda callback"),
                    *span,
                ));
                return CallOutcome::Failed;
            }
            None => match raw_callback {
                Some(value) => {
                    let value = match self.evaluate_value(&value, diagnostics, context) {
                        CallOutcome::Value(value) => value,
                        CallOutcome::Unresolved => {
                            match self.preserve_value_expression(&value, diagnostics, context) {
                                Ok(value) => value,
                                Err(outcome) => return outcome,
                            }
                        }
                        CallOutcome::NoValue => {
                            diagnostics.push(no_value_required(value_source_span(&value, span)));
                            return CallOutcome::Failed;
                        }
                        CallOutcome::Failed => return CallOutcome::Failed,
                    };
                    match value {
                        IrValue::Callable(callable) => Some(callable),
                        _ => {
                            diagnostics.push(iteration_error(
                                format!("`.{name}` callback must be a first-class callable"),
                                value_source_span(&value, span),
                            ));
                            return CallOutcome::Failed;
                        }
                    }
                }
                None if name == "sorted" => None,
                None => {
                    diagnostics.push(iteration_error(
                        format!("`.{name}` requires a callback lambda"),
                        *span,
                    ));
                    return CallOutcome::Failed;
                }
            },
        };

        match name {
            "map" => {
                let Some(callable) = callable.as_ref() else {
                    diagnostics.push(iteration_error(
                        "`.map` requires a callback lambda".to_string(),
                        *span,
                    ));
                    return CallOutcome::Failed;
                };
                self.map_callable_values(
                    &elements,
                    callable,
                    IterationOptions {
                        span: *span,
                        allow_destructuring: true,
                    },
                    diagnostics,
                    context,
                )
            }
            "filter" => {
                let Some(callable) = callable.as_ref() else {
                    diagnostics.push(iteration_error(
                        "`.filter` requires a predicate lambda".to_string(),
                        *span,
                    ));
                    return CallOutcome::Failed;
                };
                self.filter_callable_values(&elements, callable, *span, diagnostics, context)
            }
            "sorted" => {
                self.sort_iterable_values(elements, callable.as_ref(), *span, diagnostics, context)
            }
            _ => {
                diagnostics.push(iteration_error(
                    format!("Unsupported collection transform `.{name}`"),
                    *span,
                ));
                CallOutcome::Failed
            }
        }
    }

    /// Evaluates the bounded callback-based optionality functions from the
    /// v2.5.1 Optionality module. The value is resolved before a callback is
    /// invoked. `.ifpresent` skips its callback for semantic `None`, while
    /// `.takeif` still invokes its predicate with `None`, matching the
    /// distinct upstream callback contracts.
    #[allow(clippy::too_many_arguments)]
    fn evaluate_optionality_callback(
        &self,
        name: &str,
        positional_args: &[IrValue],
        named_args: &[IrNamedArg],
        body: Option<CallBody<'_>>,
        lambda_parameters: Option<&[IrParameter]>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
        binding_plan: Option<&BindingPlan>,
    ) -> CallOutcome {
        let (raw_value, raw_callback) = match optionality_operands(
            name,
            positional_args,
            named_args,
            body.is_some(),
            binding_plan,
            *span,
            diagnostics,
        ) {
            Ok(operands) => operands,
            Err(outcome) => return outcome,
        };
        let value = match self.evaluate_value(&raw_value, diagnostics, context) {
            CallOutcome::Value(value) => value,
            CallOutcome::Unresolved => {
                match self.preserve_value_expression(&raw_value, diagnostics, context) {
                    Ok(value) => value,
                    Err(outcome) => return outcome,
                }
            }
            CallOutcome::NoValue => {
                diagnostics.push(no_value_required(value_source_span(&raw_value, span)));
                return CallOutcome::Failed;
            }
            CallOutcome::Failed => return CallOutcome::Failed,
        };

        if name == "ifpresent" && matches!(value, IrValue::None) {
            return CallOutcome::Value(IrValue::None);
        }

        let callable = match body {
            Some(CallBody::Block(nodes)) => {
                self.make_callable(lambda_parameters, nodes, *span, context)
            }
            Some(CallBody::Inline(_)) => {
                diagnostics.push(function_error(
                    format!("`.{name}` requires a block or first-class lambda callback"),
                    *span,
                ));
                return CallOutcome::Failed;
            }
            None => {
                let Some(raw_callback) = raw_callback else {
                    diagnostics.push(function_error(
                        format!("`.{name}` requires a callback lambda"),
                        *span,
                    ));
                    return CallOutcome::Failed;
                };
                let callback = match self.evaluate_value(&raw_callback, diagnostics, context) {
                    CallOutcome::Value(IrValue::Callable(callable)) => callable,
                    CallOutcome::Value(value) => {
                        diagnostics.push(iteration_error(
                            format!("`.{name}` callback must be a first-class callable"),
                            value_source_span(&value, span),
                        ));
                        return CallOutcome::Failed;
                    }
                    CallOutcome::Unresolved => {
                        diagnostics.push(iteration_error(
                            format!("`.{name}` callback must be a first-class callable"),
                            value_source_span(&raw_callback, span),
                        ));
                        return CallOutcome::Failed;
                    }
                    CallOutcome::NoValue => {
                        diagnostics.push(no_value_required(value_source_span(&raw_callback, span)));
                        return CallOutcome::Failed;
                    }
                    CallOutcome::Failed => return CallOutcome::Failed,
                };
                callback
            }
        };
        let callback_result = match self.invoke_callable(
            &callable,
            vec![value.clone()],
            IterationOptions {
                span: *span,
                allow_destructuring: false,
            },
            diagnostics,
            context,
        ) {
            CallOutcome::Value(value) => value,
            CallOutcome::NoValue => {
                diagnostics.push(no_value_required(callable.span));
                return CallOutcome::Failed;
            }
            CallOutcome::Failed => return CallOutcome::Failed,
            CallOutcome::Unresolved => return CallOutcome::Unresolved,
        };

        if name == "ifpresent" {
            return CallOutcome::Value(callback_result);
        }

        let Some(condition) = scalar_boolean_value(&callback_result) else {
            diagnostics.push(iteration_error(
                "`.takeif` condition must return Boolean".to_string(),
                value_source_span(&callback_result, &callable.span),
            ));
            return CallOutcome::Failed;
        };
        if condition {
            CallOutcome::Value(value)
        } else {
            CallOutcome::Value(IrValue::None)
        }
    }

    /// Evaluates `.range` into the same typed Range representation used by
    /// literal range values. Bounds are evaluated through the ordinary value
    /// path before the upstream Number-to-Int-compatible conversion.
    #[allow(clippy::too_many_arguments)]
    fn evaluate_range(
        &self,
        positional_args: &[IrValue],
        named_args: &[IrNamedArg],
        _body: Option<CallBody<'_>>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
        binding_plan: Option<&BindingPlan>,
        first_origin: Option<ValueOrigin>,
    ) -> CallOutcome {
        let Some(binding_plan) = binding_plan else {
            return CallOutcome::Failed;
        };
        let (start, end) =
            match range_arguments(positional_args, named_args, binding_plan, span, diagnostics) {
                Ok(arguments) => arguments,
                Err(outcome) => return outcome,
            };
        let start = match start {
            Some(value) => {
                match self.evaluate_range_endpoint(
                    &value,
                    "start",
                    span,
                    diagnostics,
                    context,
                    first_origin,
                ) {
                    Ok(value) => Some(value),
                    Err(outcome) => return outcome,
                }
            }
            None => None,
        };
        let end = match end {
            Some(value) => {
                match self.evaluate_range_endpoint(&value, "end", span, diagnostics, context, None)
                {
                    Ok(value) => Some(value),
                    Err(outcome) => return outcome,
                }
            }
            None => None,
        };
        CallOutcome::Value(IrValue::Range(IrRange {
            start,
            end,
            span: *span,
        }))
    }

    fn evaluate_range_endpoint(
        &self,
        value: &IrValue,
        parameter: &str,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
        origin: Option<ValueOrigin>,
    ) -> Result<i32, CallOutcome> {
        let evaluated = self
            .evaluate_invocation_values(
                std::slice::from_ref(value),
                span,
                diagnostics,
                context,
                origin,
            )?
            .into_iter()
            .next()
            .ok_or(CallOutcome::Failed)?;
        number_to_range_endpoint(&evaluated).map_err(|error| {
            let candidate_span = value_source_span(value, span);
            diagnostics.push(conversion_failure_diagnostic(
                value_conversion::ConversionFailure::new(
                    error,
                    Some(candidate_span),
                    Some(parameter),
                    None,
                    *span,
                ),
                Some("`.range`"),
            ));
            CallOutcome::Failed
        })
    }

    fn invoke_scoped_lambda(
        &self,
        value: IrValue,
        lambda_parameters: Option<&[IrParameter]>,
        body: &[IrNode],
        options: IterationOptions,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
    ) -> CallOutcome {
        let callable = self.make_callable(lambda_parameters, body, options.span, context);
        self.invoke_callable(&callable, vec![value], options, diagnostics, context)
    }

    fn materialize_iteration_callable(
        &self,
        body: IterationBody<'_>,
        lambda_parameters: Option<&[IrParameter]>,
        span: SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
    ) -> Result<IrCallable, CallOutcome> {
        match body {
            IterationBody::Block(nodes) => {
                Ok(self.make_callable(lambda_parameters, nodes, span, context))
            }
            IterationBody::Inline(callable) => {
                let callable_span = callable.span;
                match self.evaluate_value(&IrValue::Callable(callable), diagnostics, context) {
                    CallOutcome::Value(IrValue::Callable(callable)) => Ok(callable),
                    CallOutcome::Value(_) | CallOutcome::Unresolved => {
                        diagnostics.push(iteration_error(
                            "inline iteration body did not resolve to a callable".to_string(),
                            callable_span,
                        ));
                        Err(CallOutcome::Failed)
                    }
                    CallOutcome::NoValue => {
                        diagnostics.push(no_value_required(callable_span));
                        Err(CallOutcome::Failed)
                    }
                    CallOutcome::Failed => Err(CallOutcome::Failed),
                }
            }
        }
    }

    fn make_callable(
        &self,
        parameters: Option<&[IrParameter]>,
        body: &[IrNode],
        span: SourceSpan,
        context: &EvaluationContext<'_>,
    ) -> IrCallable {
        IrCallable {
            parameters: parameters.map(ToOwned::to_owned),
            body: body.to_vec(),
            span,
            capture: Some(Box::new(context.capture_snapshot())),
        }
    }

    /// Shared first-class callable invocation path for loops, transforms, and
    /// user-defined callables. Successful assignments to existing caller
    /// variables are published after the callable completes; new variables
    /// remain invocation-local.
    fn invoke_callable(
        &self,
        callable: &IrCallable,
        arguments: Vec<IrValue>,
        options: IterationOptions,
        diagnostics: &mut Vec<Diagnostic>,
        caller_context: &mut EvaluationContext<'_>,
    ) -> CallOutcome {
        self.invoke_callable_with_extension_mode(
            callable,
            arguments,
            options,
            diagnostics,
            caller_context,
            ExtensionContextMode::Inherit,
        )
    }

    fn invoke_callable_without_extension_context(
        &self,
        callable: &IrCallable,
        arguments: Vec<IrValue>,
        options: IterationOptions,
        diagnostics: &mut Vec<Diagnostic>,
        caller_context: &mut EvaluationContext<'_>,
    ) -> CallOutcome {
        self.invoke_callable_with_extension_mode(
            callable,
            arguments,
            options,
            diagnostics,
            caller_context,
            ExtensionContextMode::Suppress,
        )
    }

    fn invoke_callable_with_extension_mode(
        &self,
        callable: &IrCallable,
        arguments: Vec<IrValue>,
        options: IterationOptions,
        diagnostics: &mut Vec<Diagnostic>,
        caller_context: &mut EvaluationContext<'_>,
        extension_context_mode: ExtensionContextMode,
    ) -> CallOutcome {
        let _depth = match caller_context.enter_evaluation_depth(options.span, diagnostics) {
            Ok(depth) => depth,
            Err(outcome) => return outcome,
        };
        let bound = match bind_invocation_arguments(
            callable.parameters.as_deref(),
            arguments,
            options.allow_destructuring,
            options.span,
            diagnostics,
        ) {
            Ok(bound) => bound,
            Err(outcome) => return outcome,
        };
        self.invoke_bound_callable_with_extension_mode(
            callable,
            bound,
            diagnostics,
            caller_context,
            None,
            extension_context_mode,
        )
    }

    fn invoke_bound_callable(
        &self,
        callable: &IrCallable,
        bound: BoundLambdaArguments,
        _options: IterationOptions,
        diagnostics: &mut Vec<Diagnostic>,
        caller_context: &mut EvaluationContext<'_>,
    ) -> CallOutcome {
        self.invoke_bound_callable_with_extension_mode(
            callable,
            bound,
            diagnostics,
            caller_context,
            None,
            ExtensionContextMode::Inherit,
        )
    }

    fn invoke_bound_callable_with_extension(
        &self,
        callable: &IrCallable,
        bound: BoundLambdaArguments,
        diagnostics: &mut Vec<Diagnostic>,
        caller_context: &mut EvaluationContext<'_>,
        extension_invocation: Option<Rc<ExtensionInvocation>>,
    ) -> CallOutcome {
        self.invoke_bound_callable_with_extension_mode(
            callable,
            bound,
            diagnostics,
            caller_context,
            extension_invocation,
            ExtensionContextMode::Suppress,
        )
    }

    fn invoke_bound_callable_with_extension_mode(
        &self,
        callable: &IrCallable,
        bound: BoundLambdaArguments,
        diagnostics: &mut Vec<Diagnostic>,
        caller_context: &mut EvaluationContext<'_>,
        extension_invocation: Option<Rc<ExtensionInvocation>>,
        extension_context_mode: ExtensionContextMode,
    ) -> CallOutcome {
        caller_context.begin_invocation();
        let checkpoint = InvocationCheckpoint::capture();
        let outcome = {
            let definition_context = callable
                .capture
                .as_deref()
                .map(EvaluationContext::from_capture)
                .unwrap_or_else(EvaluationContext::new);
            // Preserve the definition snapshot as the lexical base, then add
            // only caller-visible lookup bindings. Invocation parameters are
            // installed in the child below, after both layers, so they have
            // highest precedence. Document state is shared separately by the
            // overlay.
            let invocation_base =
                EvaluationContext::with_caller_overlay(definition_context, caller_context);
            let mut child = invocation_base.child();
            child.extension_invocation =
                extension_invocation.or_else(|| match extension_context_mode {
                    ExtensionContextMode::Inherit => caller_context.extension_invocation.clone(),
                    ExtensionContextMode::Suppress => None,
                });
            match bound {
                BoundLambdaArguments::Explicit(values) => {
                    child.set_lambda_scope(LambdaScope::Explicit);
                    if let Some(parameters) = callable.parameters.as_deref() {
                        for (parameter, value) in parameters.iter().zip(values) {
                            child.parameter_names.insert(parameter.name.clone());
                            child.set_value(parameter.name.clone(), value);
                        }
                    }
                }
                BoundLambdaArguments::Implicit(values) => {
                    child.set_lambda_scope(LambdaScope::Implicit(values));
                }
            }
            let outcome =
                self.evaluate_callable_body_value(&callable.body, diagnostics, &mut child);
            if matches!(outcome, CallOutcome::Value(_) | CallOutcome::NoValue) {
                for (name, value) in child.assigned_values() {
                    caller_context.apply_callable_assignment(name, value);
                }
            }
            outcome
        };
        if matches!(outcome, CallOutcome::Failed | CallOutcome::Unresolved) {
            checkpoint.restore(caller_context);
        } else {
            checkpoint.commit(caller_context);
        }
        caller_context.end_invocation();
        outcome
    }

    fn map_callable_values(
        &self,
        elements: &[IrValue],
        callable: &IrCallable,
        options: IterationOptions,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
    ) -> CallOutcome {
        if let Err(outcome) =
            self.check_materialized_elements_len(elements.len(), options.span, diagnostics)
        {
            return outcome;
        }
        let mut results = Vec::new();
        if let Err(error) = results.try_reserve_exact(elements.len()) {
            diagnostics.push(iteration_error(
                format!("iteration result collection cannot be allocated: {error}"),
                options.span,
            ));
            return CallOutcome::Failed;
        }
        for element in elements {
            match self.invoke_callable(
                callable,
                vec![element.clone()],
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

    fn filter_callable_values(
        &self,
        elements: &[IrValue],
        callable: &IrCallable,
        span: SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
    ) -> CallOutcome {
        if let Err(outcome) =
            self.check_materialized_elements_len(elements.len(), span, diagnostics)
        {
            return outcome;
        }
        let mut results = Vec::new();
        if let Err(error) = results.try_reserve_exact(elements.len()) {
            diagnostics.push(iteration_error(
                format!("filter result collection cannot be allocated: {error}"),
                span,
            ));
            return CallOutcome::Failed;
        }
        for element in elements {
            let predicate = match self.invoke_callable(
                callable,
                vec![element.clone()],
                IterationOptions {
                    span,
                    allow_destructuring: true,
                },
                diagnostics,
                context,
            ) {
                CallOutcome::Value(value) => value,
                CallOutcome::NoValue => {
                    diagnostics.push(no_value_required(callable.span));
                    return CallOutcome::Failed;
                }
                CallOutcome::Failed => return CallOutcome::Failed,
                CallOutcome::Unresolved => return CallOutcome::Unresolved,
            };
            let Some(keep) = scalar_boolean_value(&predicate) else {
                diagnostics.push(iteration_error(
                    "`.filter` predicate must return Boolean".to_string(),
                    value_source_span(&predicate, &callable.span),
                ));
                return CallOutcome::Failed;
            };
            if keep {
                results.push(element.clone());
            }
        }
        CallOutcome::Value(IrValue::Collection(results))
    }

    fn sort_iterable_values(
        &self,
        elements: Vec<IrValue>,
        callable: Option<&IrCallable>,
        span: SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
    ) -> CallOutcome {
        if let Err(outcome) =
            self.check_materialized_elements_len(elements.len(), span, diagnostics)
        {
            return outcome;
        }
        let mut keyed = Vec::new();
        if let Err(error) = keyed.try_reserve_exact(elements.len()) {
            diagnostics.push(iteration_error(
                format!("sorted collection cannot be allocated: {error}"),
                span,
            ));
            return CallOutcome::Failed;
        }
        for element in elements {
            let key = match callable {
                Some(callable) => match self.invoke_callable(
                    callable,
                    vec![element.clone()],
                    IterationOptions {
                        span,
                        allow_destructuring: true,
                    },
                    diagnostics,
                    context,
                ) {
                    CallOutcome::Value(value) => value,
                    CallOutcome::NoValue => {
                        diagnostics.push(no_value_required(callable.span));
                        return CallOutcome::Failed;
                    }
                    CallOutcome::Failed => return CallOutcome::Failed,
                    CallOutcome::Unresolved => return CallOutcome::Unresolved,
                },
                None => element.clone(),
            };
            let key = match SortKey::try_from_value(&key) {
                Ok(key) => key,
                Err(message) => {
                    diagnostics.push(iteration_error(message, value_source_span(&key, &span)));
                    return CallOutcome::Failed;
                }
            };
            keyed.push((element, key));
        }
        if let Some((_, first_key)) = keyed.first() {
            if keyed
                .iter()
                .skip(1)
                .any(|(_, key)| !first_key.same_kind(key))
            {
                diagnostics.push(iteration_error(
                    "`.sorted` does not compare heterogeneous key types".to_string(),
                    span,
                ));
                return CallOutcome::Failed;
            }
        }
        keyed.sort_by(|(_, left), (_, right)| left.cmp(right));
        let mut sorted = Vec::new();
        if let Err(error) = sorted.try_reserve_exact(keyed.len()) {
            diagnostics.push(iteration_error(
                format!("sorted result collection cannot be allocated: {error}"),
                span,
            ));
            return CallOutcome::Failed;
        }
        sorted.extend(keyed.into_iter().map(|(value, _)| value));
        CallOutcome::Value(IrValue::Collection(sorted))
    }

    fn coerce_iterable(
        &self,
        value: InvocationValue,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
    ) -> Result<Vec<IrValue>, CallOutcome> {
        let origin = value.origin;
        let value = match value_conversion::convert_target_with_origin(
            &value,
            value_conversion::ConversionTarget::Iterable,
            *span,
        ) {
            Ok(value_conversion::TargetValue::Value(value)) => value,
            Ok(value_conversion::TargetValue::RawMarkdown { text, .. }) => {
                if let Ok(range) = value_conversion::convert_range_with_origin(&value, *span) {
                    return self.materialize_range(range, span, diagnostics);
                }
                let nodes = self.parse_dynamic_markdown_content(
                    &text,
                    *span,
                    value_conversion::RawMarkdownTarget::Iterable,
                    diagnostics,
                )?;
                let fallback = IrValue::Content(nodes.clone());
                // `ValueFactory.iterable` first evaluates one raw expression
                // in the current context. A typed iterable result wins. If
                // the expression is not iterable, fall back to the parsed
                // Markdown list without evaluating that same expression a
                // second time. A non-expression Markdown body is evaluated
                // once as the list fallback so nested calls retain their
                // normal value-context semantics.
                let is_expression = matches!(
                    nodes.as_slice(),
                    [IrNode::FunctionCall { .. } | IrNode::ChainedFunctionCall { .. }]
                );
                if is_expression {
                    match self.evaluate_value(&fallback, diagnostics, context) {
                        CallOutcome::Value(
                            value @ (IrValue::Collection(_)
                            | IrValue::Pair(_)
                            | IrValue::Dictionary(_)
                            | IrValue::Range(_)),
                        ) => value,
                        CallOutcome::Value(_) | CallOutcome::Unresolved => fallback,
                        CallOutcome::NoValue => {
                            diagnostics.push(no_value_required(*span));
                            return Err(CallOutcome::Failed);
                        }
                        CallOutcome::Failed => return Err(CallOutcome::Failed),
                    }
                } else {
                    match self.evaluate_value(&fallback, diagnostics, context) {
                        CallOutcome::Value(value) => value,
                        CallOutcome::Unresolved => fallback,
                        CallOutcome::NoValue => {
                            diagnostics.push(no_value_required(*span));
                            return Err(CallOutcome::Failed);
                        }
                        CallOutcome::Failed => return Err(CallOutcome::Failed),
                    }
                }
            }
            Err(error) => {
                diagnostics.push(conversion_failure_diagnostic(
                    value_conversion::ConversionFailure::new(
                        error,
                        Some(*span),
                        None::<String>,
                        None,
                        *span,
                    ),
                    Some("iterable target"),
                ));
                return Err(CallOutcome::Failed);
            }
        };
        match value {
            IrValue::Collection(values) => {
                self.check_materialized_elements_len(values.len(), *span, diagnostics)?;
                Ok(values)
            }
            IrValue::Pair(pair) => {
                self.check_materialized_elements_len(2, pair.span, diagnostics)?;
                let mut values = Vec::new();
                if let Err(error) = values.try_reserve_exact(2) {
                    diagnostics.push(iteration_error(
                        format!("Pair iterable cannot be allocated: {error}"),
                        pair.span,
                    ));
                    return Err(CallOutcome::Failed);
                }
                values.push(*pair.first);
                values.push(*pair.second);
                Ok(values)
            }
            IrValue::Dictionary(dictionary) => {
                self.check_materialized_elements_len(
                    dictionary.entries.len(),
                    dictionary.span,
                    diagnostics,
                )?;
                let mut values = Vec::new();
                if let Err(error) = values.try_reserve_exact(dictionary.entries.len()) {
                    diagnostics.push(iteration_error(
                        format!("Dictionary iterable cannot be allocated: {error}"),
                        dictionary.span,
                    ));
                    return Err(CallOutcome::Failed);
                }
                values.extend(dictionary.entries.into_iter().map(IrValue::Pair));
                Ok(values)
            }
            IrValue::Range(range) => self.materialize_range(range, span, diagnostics),
            value @ (IrValue::String(_) | IrValue::Identifier(_)) => {
                let argument = InvocationValue { value, origin };
                match value_conversion::convert_range_with_origin(&argument, *span) {
                    Ok(range) => self.materialize_range(range, span, diagnostics),
                    Err(_) => {
                        diagnostics.push(iteration_error(
                            "Value is not an iterable Range, Collection, Pair, Dictionary, or exactly one Markdown list"
                                .to_string(),
                            *span,
                        ));
                        Err(CallOutcome::Failed)
                    }
                }
            }
            IrValue::Content(nodes) => match nodes.as_slice() {
                [IrNode::UnorderedList { items, .. }] | [IrNode::OrderedList { items, .. }] => {
                    self.check_materialized_elements_len(items.len(), *span, diagnostics)?;
                    let mut values = Vec::new();
                    if let Err(error) = values.try_reserve_exact(items.len()) {
                        diagnostics.push(iteration_error(
                            format!("list collection cannot be allocated: {error}"),
                            *span,
                        ));
                        return Err(CallOutcome::Failed);
                    }
                    for item in items {
                        values.push(self.list_item_value(item, span, diagnostics, context)?);
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
        item: &arkst_ir::IrListItem,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
    ) -> Result<IrValue, CallOutcome> {
        match item.nodes.as_slice() {
            [IrNode::UnorderedList { .. }] | [IrNode::OrderedList { .. }] => self
                .coerce_iterable(
                    InvocationValue::static_value(IrValue::Content(item.nodes.clone())),
                    span,
                    diagnostics,
                    context,
                )
                .map(IrValue::Collection),
            [IrNode::Paragraph { content, .. }] => {
                let mut text = String::new();
                for inline in content {
                    let IrInline::Text { content, .. } = inline else {
                        return Ok(IrValue::Content(item.nodes.clone()));
                    };
                    text.push_str(content);
                }
                Ok(IrValue::String(text))
            }
            _ => Ok(IrValue::Content(item.nodes.clone())),
        }
    }

    fn check_materialized_elements(
        &self,
        requested: u64,
        span: SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Result<usize, CallOutcome> {
        let limit = self.limits.max_materialized_elements as u64;
        if requested > limit {
            diagnostics.push(materialized_elements_limit_error(
                requested,
                self.limits.max_materialized_elements,
                span,
            ));
            return Err(CallOutcome::Failed);
        }
        usize::try_from(requested).map_err(|_| {
            diagnostics.push(iteration_error(
                "Materialized element count is too large for this target".to_string(),
                span,
            ));
            CallOutcome::Failed
        })
    }

    fn check_materialized_elements_len(
        &self,
        requested: usize,
        span: SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Result<(), CallOutcome> {
        self.check_materialized_elements(requested as u64, span, diagnostics)
            .map(|_| ())
    }

    fn materialize_range(
        &self,
        range: IrRange,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Result<Vec<IrValue>, CallOutcome> {
        let Some(end) = range.end else {
            diagnostics.push(iteration_error(
                "Cannot iterate through an endless Range".to_string(),
                range.span,
            ));
            return Err(CallOutcome::Failed);
        };
        let start = range.start.unwrap_or(1);
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
        let Some(count) = i64::from(end)
            .checked_sub(i64::from(start))
            .and_then(|distance| distance.checked_add(1))
        else {
            diagnostics.push(iteration_error(
                "Closed Range cardinality overflowed the supported integer domain".to_string(),
                range.span,
            ));
            return Err(CallOutcome::Failed);
        };
        let capacity = self.check_materialized_elements(count as u64, range.span, diagnostics)?;
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
        ordered_args: Option<&[IrCallArgument]>,
        positional_args: &[IrValue],
        named_args: &[IrNamedArg],
        body: Option<CallBody<'_>>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
        implicit_argument: Option<InvocationValue>,
    ) -> CallOutcome {
        let binding_plan = if let LambdaParameters::Explicit(parameters) = &binding.parameters {
            let metadata = parameters
                .iter()
                .map(|parameter| {
                    let metadata = if parameter.optional {
                        ParameterMetadata::optional(&parameter.name)
                    } else {
                        ParameterMetadata::required(&parameter.name)
                    };
                    metadata.with_name_span(parameter.name_span)
                })
                .collect::<Vec<_>>();
            match self.preflight_binding(
                &metadata,
                ordered_args,
                positional_args,
                named_args,
                body,
                BodyPolicy::BindFinal,
                *span,
                "E3003",
                None,
                None,
                true,
                diagnostics,
            ) {
                Ok(plan) => Some(plan),
                Err(()) => return CallOutcome::Failed,
            }
        } else {
            let candidates =
                structural_candidates(ordered_args, positional_args, named_args, *span);
            if let Err(error) = invocation_binder::validate_implicit(&candidates) {
                diagnostics.push(binding_diagnostic_with_message(
                    error,
                    callable_binding_message,
                ));
                return CallOutcome::Failed;
            }
            None
        };
        // Caller arguments are evaluated before any callee scope is created.
        let candidates = match self.evaluate_invocation_candidates(
            ordered_args,
            positional_args,
            named_args,
            span,
            diagnostics,
            context,
            None,
            implicit_argument.as_ref(),
        ) {
            Ok(values) => values,
            Err(outcome) => return outcome,
        };
        let bound = match self.bind_callable_arguments(
            &binding.parameters,
            binding_plan.as_ref(),
            candidates,
            body,
            span,
            diagnostics,
            context,
        ) {
            Ok(bound) => bound,
            Err(outcome) => return outcome,
        };
        let callable = binding.as_callable();
        self.invoke_bound_callable(
            &callable,
            bound,
            IterationOptions {
                span: *span,
                allow_destructuring: false,
            },
            diagnostics,
            context,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn evaluate_function_binding(
        &self,
        binding: &FunctionBinding,
        ordered_args: Option<&[IrCallArgument]>,
        positional_args: &[IrValue],
        named_args: &[IrNamedArg],
        body: Option<CallBody<'_>>,
        raw_body: Option<&IrRawBody>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
        implicit_argument: Option<InvocationValue>,
    ) -> CallOutcome {
        if binding.extension.is_some() {
            self.evaluate_extension_call(
                binding,
                ordered_args,
                positional_args,
                named_args,
                body,
                raw_body,
                span,
                diagnostics,
                context,
            )
        } else {
            self.evaluate_user_function(
                binding,
                ordered_args,
                positional_args,
                named_args,
                body,
                span,
                diagnostics,
                context,
                implicit_argument,
            )
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn evaluate_extend(
        &self,
        ordered_args: Option<&[IrCallArgument]>,
        positional_args: &[IrValue],
        named_args: &[IrNamedArg],
        body: Option<CallBody<'_>>,
        lambda_parameters: Option<&[IrParameter]>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
        binding_plan: Option<&BindingPlan>,
    ) -> CallOutcome {
        let Some(binding_plan) = binding_plan else {
            return CallOutcome::Failed;
        };
        let Some(body) = body else {
            diagnostics.push(extension_error(
                "`.extend` requires a non-empty wrapper body".to_string(),
                *span,
                None,
            ));
            return CallOutcome::Failed;
        };
        let CallBody::Block(body_nodes) = body else {
            diagnostics.push(extension_error(
                "`.extend` is block-only and requires a wrapper body".to_string(),
                call_body_source_span(body, *span),
                None,
            ));
            return CallOutcome::Failed;
        };
        if body_nodes.is_empty() {
            diagnostics.push(extension_error(
                "`.extend` requires a non-empty wrapper body".to_string(),
                *span,
                None,
            ));
            return CallOutcome::Failed;
        }

        let candidates = match self.evaluate_invocation_candidates(
            ordered_args,
            positional_args,
            named_args,
            span,
            diagnostics,
            context,
            None,
            None,
        ) {
            Ok(candidates) => candidates,
            Err(outcome) => return outcome,
        };
        let bound = match binding_plan.bind(&candidates, None, *span) {
            Ok(bound) => bound,
            Err(error) => {
                diagnostics.push(binding_diagnostic_with_code(error, "E3001"));
                return CallOutcome::Failed;
            }
        };
        let Some(BoundSlot::Explicit {
            value: target_value,
            span: target_span,
        }) = bound.slots.first()
        else {
            diagnostics.push(extension_error(
                "`.extend` requires a callable target name".to_string(),
                *span,
                None,
            ));
            return CallOutcome::Failed;
        };
        let target_name = match &target_value.value {
            IrValue::Identifier(name) | IrValue::String(name)
                if is_valid_normal_call_name(name) =>
            {
                name.clone()
            }
            _ => {
                diagnostics.push(extension_error(
                    "`.extend` target must be a normal callable name".to_string(),
                    *target_span,
                    None,
                ));
                return CallOutcome::Failed;
            }
        };
        let Some(existing_target) = resolve_function_target(&target_name, context) else {
            diagnostics.push(extension_error(
                format!("Cannot extend `{target_name}` because no callable target is visible"),
                *target_span,
                None,
            ));
            return CallOutcome::Failed;
        };
        let Some((original_parameters, body_policy)) =
            function_target_contract(&existing_target, *target_span)
        else {
            diagnostics.push(extension_error(
                format!("Cannot extend `{target_name}` because its callable contract is invalid"),
                *target_span,
                None,
            ));
            return CallOutcome::Failed;
        };
        let wrapper_parameters = extension_wrapper_parameters(&original_parameters, *span);
        let target_names = lambda_parameter_names(&original_parameters);

        if let Some(parameters) = lambda_parameters {
            if let Some(parameter) = parameters
                .iter()
                .find(|parameter| !target_names.contains(&parameter.name))
            {
                diagnostics.push(extension_parameter_error(
                    format!(
                        "Extension parameter `{}` is not part of `{target_name}`",
                        parameter.name
                    ),
                    parameter.span,
                    *target_span,
                ));
                return CallOutcome::Failed;
            }
            let mut seen = BTreeSet::new();
            if let Some(parameter) = parameters
                .iter()
                .find(|parameter| !seen.insert(parameter.name.as_str()))
            {
                diagnostics.push(extension_parameter_error(
                    format!("Duplicate extension parameter `{}`", parameter.name),
                    parameter.span,
                    *target_span,
                ));
                return CallOutcome::Failed;
            }
        }

        let condition = match bound.slots.get(1) {
            Some(BoundSlot::Explicit { value, span }) => match &value.value {
                IrValue::Callable(callable) => {
                    if let Some(parameters) = callable.parameters.as_deref() {
                        if let Some(parameter) = parameters
                            .iter()
                            .find(|parameter| !target_names.contains(&parameter.name))
                        {
                            diagnostics.push(extension_parameter_error(
                                format!(
                                    "Extension condition parameter `{}` is not part of `{target_name}`",
                                    parameter.name
                                ),
                                parameter.span,
                                *span,
                            ));
                            return CallOutcome::Failed;
                        }
                    }
                    let mut condition = callable.clone();
                    condition.parameters = wrapper_parameters.to_ir();
                    Some(condition)
                }
                _ => {
                    diagnostics.push(extension_error(
                        "`.extend` `where` must be a callable condition".to_string(),
                        *span,
                        Some(*target_span),
                    ));
                    return CallOutcome::Failed;
                }
            },
            _ => None,
        };

        let (super_target, innermost) = split_extension_target(existing_target, context);
        let Some(id) = context.allocate_extension_id() else {
            diagnostics.push(extension_error(
                "`.extend` exceeded the evaluator's extension identity limit".to_string(),
                *span,
                Some(*target_span),
            ));
            return CallOutcome::Failed;
        };
        let extension = Rc::new(FunctionExtension {
            id,
            condition,
            super_target,
            body_policy,
        });
        let wrapper = Rc::new(FunctionBinding {
            parameters: wrapper_parameters,
            body: body_nodes.to_vec(),
            declaration_span: *span,
            capture: Some(Box::new(context.capture_snapshot())),
            extension: Some(Rc::clone(&extension)),
        });

        if let Some(innermost) = innermost {
            context.set_extension_target(&innermost, FunctionTarget::Binding(wrapper));
        } else {
            context.replace_function_binding(target_name, wrapper);
        }
        CallOutcome::NoValue
    }

    #[allow(clippy::too_many_arguments)]
    fn evaluate_extension_call(
        &self,
        binding: &FunctionBinding,
        ordered_args: Option<&[IrCallArgument]>,
        positional_args: &[IrValue],
        named_args: &[IrNamedArg],
        body: Option<CallBody<'_>>,
        raw_body: Option<&IrRawBody>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
    ) -> CallOutcome {
        let structural_candidates =
            structural_candidates(ordered_args, positional_args, named_args, *span);
        let body_shape = body.map(|body| body_candidate_shape(body, *span));
        let structural_plan = match self.preflight_extension_binding(
            binding,
            &structural_candidates,
            body_shape.as_ref(),
            *span,
            diagnostics,
        ) {
            Ok(plan) => plan,
            Err(()) => return CallOutcome::Failed,
        };
        let candidates = match self.evaluate_invocation_candidates(
            ordered_args,
            positional_args,
            named_args,
            span,
            diagnostics,
            context,
            None,
            None,
        ) {
            Ok(candidates) => candidates,
            Err(outcome) => return outcome,
        };
        let body = body.map(|body| owned_call_body(body));
        self.evaluate_extension_call_with_candidates(
            binding,
            candidates,
            body.as_ref(),
            raw_body,
            *span,
            diagnostics,
            context,
            structural_plan.as_ref(),
        )
    }

    fn preflight_extension_binding(
        &self,
        binding: &FunctionBinding,
        candidates: &[Candidate<()>],
        body: Option<&Candidate<()>>,
        span: SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Result<Option<BindingPlan>, ()> {
        let Some(extension) = binding.extension.as_ref() else {
            return Err(());
        };
        let metadata = lambda_binding_metadata(&binding.parameters);
        match &binding.parameters {
            LambdaParameters::Explicit(_) => {
                let structural_plan = match invocation_binder::plan(
                    &metadata,
                    candidates,
                    None,
                    BodyPolicy::AllowSeparate,
                    span,
                ) {
                    Ok(plan) => plan,
                    Err(error) => {
                        diagnostics.push(binding_diagnostic_with_message(
                            error,
                            callable_binding_message,
                        ));
                        return Err(());
                    }
                };
                if let Err(error) = invocation_binder::plan(
                    &metadata,
                    candidates,
                    body,
                    extension.body_policy.binder_policy(),
                    span,
                ) {
                    diagnostics.push(binding_diagnostic_with_message(
                        error,
                        callable_binding_message,
                    ));
                    return Err(());
                }
                Ok(Some(structural_plan))
            }
            LambdaParameters::Implicit => {
                if let Err(error) = invocation_binder::validate_implicit(candidates) {
                    diagnostics.push(binding_diagnostic_with_message(
                        error,
                        callable_binding_message,
                    ));
                    return Err(());
                }
                Ok(None)
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn evaluate_extension_call_with_candidates(
        &self,
        binding: &FunctionBinding,
        candidates: Vec<Candidate<InvocationValue>>,
        body: Option<&OwnedCallBody>,
        raw_body: Option<&IrRawBody>,
        span: SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
        structural_plan: Option<&BindingPlan>,
    ) -> CallOutcome {
        let Some(extension) = binding.extension.as_ref() else {
            return CallOutcome::Failed;
        };
        let owned_structural_plan;
        let structural_plan = match structural_plan {
            Some(plan) => Some(plan),
            None => {
                let body_shape = body.map(|body| owned_body_candidate_shape(body, span));
                owned_structural_plan = match self.preflight_extension_binding(
                    binding,
                    &candidate_shapes(&candidates),
                    body_shape.as_ref(),
                    span,
                    diagnostics,
                ) {
                    Ok(plan) => plan,
                    Err(()) => return CallOutcome::Failed,
                };
                owned_structural_plan.as_ref()
            }
        };

        let body_value = match extension_body_value(
            self,
            body,
            raw_body,
            extension.body_policy,
            span,
            diagnostics,
            context,
        ) {
            Ok(value) => value,
            Err(outcome) => return outcome,
        };
        let (values, forwarded) = match (&binding.parameters, structural_plan) {
            (LambdaParameters::Explicit(_), Some(structural_plan)) => {
                let bound = match structural_plan.bind(&candidates, None, span) {
                    Ok(bound) => bound,
                    Err(error) => {
                        diagnostics.push(binding_diagnostic_with_message(
                            error,
                            callable_binding_message,
                        ));
                        return CallOutcome::Failed;
                    }
                };
                let mut values = bound
                    .slots
                    .iter()
                    .map(|slot| match slot {
                        BoundSlot::Explicit { value, .. } => value.value.clone(),
                        BoundSlot::Omitted | BoundSlot::Defaulted => IrValue::None,
                    })
                    .collect::<Vec<_>>();
                let mut forwarded = forwarded_extension_candidates(&bound);
                if let Some(body_value) = &body_value {
                    let Some(last) = values.last_mut() else {
                        diagnostics.push(extension_error(
                            "`.extend` target body has no parameter slot".to_string(),
                            span,
                            None,
                        ));
                        return CallOutcome::Failed;
                    };
                    *last = body_value.value.clone();
                    if let Some(parameter) = binding.parameters.last_name() {
                        let body_span =
                            extension_body_candidate_span(extension.body_policy, body, span);
                        forwarded.push(Candidate::Named {
                            name: parameter.name.clone(),
                            name_span: parameter.name_span,
                            value: body_value.clone(),
                            span: body_span,
                        });
                    }
                }
                (values, forwarded)
            }
            (LambdaParameters::Implicit, None) => {
                let mut values = candidates
                    .iter()
                    .filter_map(|candidate| match candidate {
                        Candidate::Positional { value, .. } => Some(value.value.clone()),
                        Candidate::Named { .. } => None,
                    })
                    .collect::<Vec<_>>();
                let mut forwarded = candidates;
                if let Some(body_value) = &body_value {
                    values.push(body_value.value.clone());
                    forwarded.push(Candidate::Positional {
                        value: body_value.clone(),
                        span: extension_body_candidate_span(extension.body_policy, body, span),
                    });
                }
                (values, forwarded)
            }
            _ => return CallOutcome::Failed,
        };
        let state = Rc::new(ExtensionInvocation {
            target: context.get_extension_target(extension),
            parameters: binding.parameters.clone(),
            forwarded,
            body: body_value.is_none().then(|| body.cloned()).flatten(),
            raw_body: body_value.is_none().then(|| raw_body.cloned()).flatten(),
        });

        if let Some(condition) = &extension.condition {
            let condition_result = match self.invoke_callable_without_extension_context(
                condition,
                values.clone(),
                IterationOptions {
                    span,
                    allow_destructuring: false,
                },
                diagnostics,
                context,
            ) {
                CallOutcome::Value(value) => value,
                CallOutcome::NoValue => {
                    diagnostics.push(no_value_required(condition.span));
                    return CallOutcome::Failed;
                }
                CallOutcome::Failed => return CallOutcome::Failed,
                CallOutcome::Unresolved => return CallOutcome::Unresolved,
            };
            let condition = match resolve_boolean_value(&InvocationValue::static_value(
                condition_result.clone(),
            )) {
                Ok(value) => value,
                Err(error) => {
                    diagnostics.push(conversion_failure_diagnostic(
                        value_conversion::ConversionFailure::new(
                            error,
                            Some(value_source_span(&condition_result, &condition.span)),
                            None::<String>,
                            None,
                            span,
                        ),
                        Some("`.extend` condition"),
                    ));
                    return CallOutcome::Failed;
                }
            };
            if !condition {
                return self.invoke_function_target(
                    &state.target,
                    state.forwarded.clone(),
                    state.body.as_ref(),
                    state.raw_body.as_ref(),
                    span,
                    diagnostics,
                    context,
                );
            }
        }

        let callable = binding.as_callable();
        let bound = match binding.parameters {
            LambdaParameters::Explicit(_) => BoundLambdaArguments::Explicit(values),
            LambdaParameters::Implicit => BoundLambdaArguments::Implicit(values),
        };
        self.invoke_bound_callable_with_extension(
            &callable,
            bound,
            diagnostics,
            context,
            Some(state),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn evaluate_super_call(
        &self,
        ordered_args: Option<&[IrCallArgument]>,
        positional_args: &[IrValue],
        named_args: &[IrNamedArg],
        body: Option<CallBody<'_>>,
        lambda_parameters: Option<&[IrParameter]>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
    ) -> CallOutcome {
        let Some(state) = context.extension_invocation.as_ref().cloned() else {
            diagnostics.push(extension_error(
                "`.super` is only available inside an extension body".to_string(),
                *span,
                None,
            ));
            return CallOutcome::Failed;
        };
        if body.is_some() || lambda_parameters.is_some() {
            diagnostics.push(extension_error(
                "`.super` does not accept a body or lambda parameters".to_string(),
                body.map_or(*span, |body| call_body_source_span(body, *span)),
                None,
            ));
            return CallOutcome::Failed;
        }
        let structural_candidates =
            structural_candidates(ordered_args, positional_args, named_args, *span);
        let structural_plan = match &state.parameters {
            LambdaParameters::Explicit(_) => match invocation_binder::plan(
                &lambda_binding_metadata(&state.parameters),
                &structural_candidates,
                None,
                BodyPolicy::Reject,
                *span,
            ) {
                Ok(plan) => Some(plan),
                Err(error) => {
                    diagnostics.push(binding_diagnostic_with_message(
                        error,
                        callable_binding_message,
                    ));
                    return CallOutcome::Failed;
                }
            },
            LambdaParameters::Implicit => {
                if let Err(error) = invocation_binder::validate_implicit(&structural_candidates) {
                    diagnostics.push(binding_diagnostic_with_message(
                        error,
                        callable_binding_message,
                    ));
                    return CallOutcome::Failed;
                }
                None
            }
        };
        let candidates = match self.evaluate_invocation_candidates(
            ordered_args,
            positional_args,
            named_args,
            span,
            diagnostics,
            context,
            None,
            None,
        ) {
            Ok(candidates) => candidates,
            Err(outcome) => return outcome,
        };
        let overrides = match &state.parameters {
            LambdaParameters::Explicit(_) => {
                let Some(plan) = structural_plan.as_ref() else {
                    return CallOutcome::Failed;
                };
                let bound = match plan.bind(&candidates, None, *span) {
                    Ok(bound) => bound,
                    Err(error) => {
                        diagnostics.push(binding_diagnostic_with_message(
                            error,
                            callable_binding_message,
                        ));
                        return CallOutcome::Failed;
                    }
                };
                forwarded_extension_candidates(&bound)
            }
            LambdaParameters::Implicit => candidates,
        };
        let merged = merge_extension_candidates(&state.forwarded, &overrides, &state.parameters);
        self.invoke_function_target(
            &state.target,
            merged,
            state.body.as_ref(),
            state.raw_body.as_ref(),
            *span,
            diagnostics,
            context,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn invoke_function_target(
        &self,
        target: &FunctionTarget,
        candidates: Vec<Candidate<InvocationValue>>,
        body: Option<&OwnedCallBody>,
        raw_body: Option<&IrRawBody>,
        span: SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
    ) -> CallOutcome {
        let _depth = match context.enter_evaluation_depth(span, diagnostics) {
            Ok(depth) => depth,
            Err(outcome) => return outcome,
        };
        context.begin_invocation();
        let checkpoint = InvocationCheckpoint::capture();
        let outcome = match target {
            FunctionTarget::Binding(binding) => {
                if binding.extension.is_some() {
                    self.evaluate_extension_call_with_candidates(
                        binding,
                        candidates,
                        body,
                        raw_body,
                        span,
                        diagnostics,
                        context,
                        None,
                    )
                } else {
                    let body = body.map(owned_body_as_call_body);
                    self.evaluate_user_function_with_candidates(
                        binding,
                        candidates,
                        body.as_ref(),
                        span,
                        diagnostics,
                        context,
                    )
                }
            }
            FunctionTarget::Native(name) => {
                let body = body.map(owned_body_as_call_body);
                self.evaluate_native_target_with_candidates(
                    name,
                    candidates,
                    body.as_ref(),
                    raw_body,
                    span,
                    diagnostics,
                    context,
                )
            }
        };
        if matches!(outcome, CallOutcome::Failed | CallOutcome::Unresolved) {
            checkpoint.restore(context);
        } else {
            checkpoint.commit(context);
        }
        context.end_invocation();
        outcome
    }

    #[allow(clippy::too_many_arguments)]
    fn evaluate_user_function_with_candidates(
        &self,
        binding: &FunctionBinding,
        candidates: Vec<Candidate<InvocationValue>>,
        body: Option<&CallBody<'_>>,
        span: SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
    ) -> CallOutcome {
        match &binding.parameters {
            LambdaParameters::Implicit => {
                if candidates
                    .iter()
                    .any(|candidate| matches!(candidate, Candidate::Named { .. }))
                {
                    diagnostics.push(extension_error(
                        "Implicit callable parameters are positional only".to_string(),
                        span,
                        None,
                    ));
                    return CallOutcome::Failed;
                }
                let mut values = candidates
                    .into_iter()
                    .filter_map(|candidate| match candidate {
                        Candidate::Positional { value, .. } => Some(value.value),
                        Candidate::Named { .. } => None,
                    })
                    .collect::<Vec<_>>();
                if let Some(body) = body {
                    match self.evaluate_call_body(*body, &span, diagnostics, context) {
                        CallOutcome::Value(value) => values.push(value),
                        CallOutcome::NoValue => return CallOutcome::NoValue,
                        CallOutcome::Failed => return CallOutcome::Failed,
                        CallOutcome::Unresolved => return CallOutcome::Unresolved,
                    }
                }
                let callable = binding.as_callable();
                self.invoke_bound_callable(
                    &callable,
                    BoundLambdaArguments::Implicit(values),
                    IterationOptions {
                        span,
                        allow_destructuring: false,
                    },
                    diagnostics,
                    context,
                )
            }
            LambdaParameters::Explicit(parameters) => {
                let metadata = parameters
                    .iter()
                    .map(|parameter| {
                        let omission = if parameter.optional {
                            invocation_binder::OmissionPolicy::Optional
                        } else {
                            invocation_binder::OmissionPolicy::Required
                        };
                        ParameterMetadata {
                            name: &parameter.name,
                            aliases: &[],
                            allows_named: true,
                            omission,
                            name_span: Some(parameter.name_span),
                        }
                    })
                    .collect::<Vec<_>>();
                let body_value = body.map(|body| {
                    match self.evaluate_call_body(*body, &span, diagnostics, context) {
                        CallOutcome::Value(value) => Ok(Candidate::Positional {
                            value: InvocationValue::static_value(value),
                            span: call_body_source_span(*body, span),
                        }),
                        CallOutcome::NoValue => Err(CallOutcome::NoValue),
                        CallOutcome::Failed => Err(CallOutcome::Failed),
                        CallOutcome::Unresolved => Err(CallOutcome::Unresolved),
                    }
                });
                let body_value = match body_value {
                    Some(Ok(value)) => Some(value),
                    Some(Err(outcome)) => return outcome,
                    None => None,
                };
                let body_shape = body_value.as_ref().map(candidate_shape);
                let plan = match invocation_binder::plan(
                    &metadata,
                    &candidate_shapes(&candidates),
                    body_shape.as_ref(),
                    BodyPolicy::BindFinal,
                    span,
                ) {
                    Ok(plan) => plan,
                    Err(error) => {
                        diagnostics.push(binding_diagnostic_with_message(
                            error,
                            callable_binding_message,
                        ));
                        return CallOutcome::Failed;
                    }
                };
                let bound = match plan.bind(&candidates, body_value.as_ref(), span) {
                    Ok(bound) => bound,
                    Err(error) => {
                        diagnostics.push(binding_diagnostic_with_message(
                            error,
                            callable_binding_message,
                        ));
                        return CallOutcome::Failed;
                    }
                };
                let values = bound
                    .slots
                    .into_iter()
                    .map(|slot| match slot {
                        BoundSlot::Explicit { value, .. } => value.value,
                        BoundSlot::Omitted | BoundSlot::Defaulted => IrValue::None,
                    })
                    .collect::<Vec<_>>();
                let callable = binding.as_callable();
                self.invoke_bound_callable(
                    &callable,
                    BoundLambdaArguments::Explicit(values),
                    IterationOptions {
                        span,
                        allow_destructuring: false,
                    },
                    diagnostics,
                    context,
                )
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn evaluate_native_target_with_candidates(
        &self,
        name: &str,
        candidates: Vec<Candidate<InvocationValue>>,
        body: Option<&CallBody<'_>>,
        raw_body: Option<&IrRawBody>,
        span: SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
    ) -> CallOutcome {
        let Some(builtin) = builtins::lookup(name) else {
            diagnostics.push(extension_error(
                format!("`.super` target `.{name}` has no supported evaluator path"),
                span,
                None,
            ));
            return CallOutcome::Failed;
        };
        let parameters = builtins::binding_parameters(builtin);
        let shapes = candidate_shapes(&candidates);
        let body_shape = body.map(|body| body_candidate_shape(*body, span));
        let plan = match invocation_binder::plan(
            &parameters,
            &shapes,
            body_shape.as_ref(),
            builtin.body_policy.binder_policy(),
            span,
        ) {
            Ok(plan) => plan,
            Err(error) => {
                diagnostics.push(binding_diagnostic_with_code(error, "E3001"));
                return CallOutcome::Failed;
            }
        };
        self.evaluate_builtin_with_candidates(
            builtin,
            &plan,
            candidates,
            body.copied(),
            raw_body,
            span,
            diagnostics,
            context,
        )
    }

    /// Evaluates and binds one callable's arguments for either parameter mode.
    /// The result is consumed by the shared child-scope/body invocation path.
    #[allow(clippy::too_many_arguments)]
    fn bind_callable_arguments(
        &self,
        parameters: &LambdaParameters,
        binding_plan: Option<&BindingPlan>,
        candidates: Vec<Candidate<InvocationValue>>,
        body: Option<CallBody<'_>>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
    ) -> Result<BoundLambdaArguments, CallOutcome> {
        match parameters {
            LambdaParameters::Implicit => {
                let mut arguments = candidates
                    .into_iter()
                    .filter_map(|candidate| match candidate {
                        Candidate::Positional { value, .. } => Some(value.value),
                        Candidate::Named { .. } => None,
                    })
                    .collect::<Vec<_>>();
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
            LambdaParameters::Explicit(_parameters) => {
                let body_value = if let Some(body) = body {
                    let value = match self.evaluate_call_body(body, span, diagnostics, context) {
                        CallOutcome::Value(value) => value,
                        CallOutcome::NoValue => return Err(CallOutcome::NoValue),
                        CallOutcome::Failed => return Err(CallOutcome::Failed),
                        CallOutcome::Unresolved => return Err(CallOutcome::Unresolved),
                    };
                    Some(Candidate::Positional {
                        value: InvocationValue::static_value(value),
                        span: call_body_source_span(body, *span),
                    })
                } else {
                    None
                };
                let Some(binding_plan) = binding_plan else {
                    return Err(CallOutcome::Failed);
                };
                let bound = binding_plan
                    .bind(&candidates, body_value.as_ref(), *span)
                    .map_err(|error| {
                        diagnostics.push(binding_diagnostic_with_message(
                            error,
                            callable_binding_message,
                        ));
                        CallOutcome::Failed
                    })?;
                Ok(BoundLambdaArguments::Explicit(
                    bound
                        .slots
                        .into_iter()
                        .map(|slot| match slot {
                            BoundSlot::Explicit { value, .. } => value.value,
                            BoundSlot::Omitted | BoundSlot::Defaulted => IrValue::None,
                        })
                        .collect(),
                ))
            }
        }
    }

    fn evaluate_callable_body_value(
        &self,
        nodes: &[IrNode],
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
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
        context: &mut EvaluationContext<'_>,
    ) -> CallOutcome {
        match node {
            IrNode::FunctionCall {
                name,
                positional_args,
                named_args,
                ordered_args,
                lambda_parameters,
                body,
                raw_body,
                span,
                ..
            } => {
                match self.evaluate_call_value_with_ordered(
                    name,
                    ordered_args.as_deref(),
                    positional_args,
                    named_args,
                    body.as_deref().map(CallBody::Block),
                    raw_body.as_ref(),
                    lambda_parameters.as_deref(),
                    span,
                    diagnostics,
                    context,
                ) {
                    // Keep the callable invocation unresolved so its
                    // savepoint rolls back. The enclosing value/output
                    // boundary owns preservation of an unresolved call;
                    // preserving it here would make a failed callable
                    // look successful and publish its earlier writes.
                    CallOutcome::Unresolved => CallOutcome::Unresolved,
                    outcome => outcome,
                }
            }
            IrNode::ChainedFunctionCall {
                head,
                chain,
                body,
                raw_body,
                ..
            } => self.evaluate_chain_value(
                head,
                chain,
                body.as_deref().map(CallBody::Block),
                raw_body.as_ref(),
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
        raw_body: Option<&IrRawBody>,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
    ) -> CallOutcome {
        let mut value_origin = call_result_origin(&head.name, context);
        let mut value = match self.chain_outcome(
            self.evaluate_call_value_with_ordered(
                &head.name,
                head.ordered_args.as_deref(),
                &head.positional_args,
                &head.named_args,
                None,
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
            let implicit_argument = InvocationValue {
                value: value.clone(),
                origin: value_origin,
            };
            positional_args.push(value);
            positional_args.extend(source_segment.positional_args.iter().cloned());
            let mut ordered_args = Vec::with_capacity(
                source_segment
                    .ordered_args
                    .as_ref()
                    .map_or(1, |arguments| arguments.len() + 1),
            );
            ordered_args.push(IrCallArgument::Positional {
                index: 0,
                span: source_segment.span,
            });
            if let Some(arguments) = &source_segment.ordered_args {
                for argument in arguments {
                    let Some(argument) = offset_ordered_argument(argument, 1) else {
                        diagnostics.push(chain_evaluation_error(
                            "Chained call argument reference is invalid".to_string(),
                            source_segment.span,
                        ));
                        return CallOutcome::Failed;
                    };
                    ordered_args.push(argument);
                }
            } else {
                ordered_args.extend(source_segment.positional_args.iter().enumerate().map(
                    |(index, _value)| IrCallArgument::Positional {
                        index: index + 1,
                        span: source_segment.span,
                    },
                ));
                ordered_args.extend(source_segment.named_args.iter().enumerate().map(
                    |(index, argument)| IrCallArgument::Named {
                        index,
                        name_span: argument.name_span,
                        span: argument.span,
                    },
                ));
            }
            let final_body = (index + 1 == chain.len()).then_some(body).flatten();
            let outcome = self.chain_outcome(
                self.evaluate_call_value_with_first_origin(
                    &source_segment.name,
                    &positional_args,
                    &source_segment.named_args,
                    final_body,
                    (index + 1 == chain.len()).then_some(raw_body).flatten(),
                    None,
                    &source_segment.span,
                    diagnostics,
                    context,
                    Some(value_origin),
                    Some(&ordered_args),
                    Some(implicit_argument),
                ),
                source_segment,
                index + 1 < chain.len(),
                diagnostics,
                context,
            );
            match outcome {
                CallOutcome::Value(next_value) => {
                    value = next_value;
                    value_origin = call_result_origin(&source_segment.name, context);
                }
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
        context: &EvaluationContext<'_>,
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
        context: &mut EvaluationContext<'_>,
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
    #[allow(clippy::too_many_arguments)]
    fn resolve_call_condition(
        &self,
        name: &str,
        condition_slot: Option<&BoundSlot<IrValue>>,
        parameter: Option<&invocation_binder::BoundParameter>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
        first_origin: Option<ValueOrigin>,
    ) -> Result<bool, CallOutcome> {
        let Some(BoundSlot::Explicit {
            value: raw_condition,
            span: condition_span,
        }) = condition_slot
        else {
            diagnostics.push(unresolvable_condition(name, span));
            return Err(CallOutcome::Failed);
        };
        let condition = if let IrValue::Identifier(name) = raw_condition {
            context
                .get(name)
                .map(|value| InvocationValue::dynamic_value(value.to_value()))
                .unwrap_or_else(|| InvocationValue {
                    value: raw_condition.clone(),
                    origin: first_origin.unwrap_or(ValueOrigin::Dynamic),
                })
        } else {
            let Some(condition) = self
                .evaluate_invocation_values(
                    std::slice::from_ref(raw_condition),
                    span,
                    diagnostics,
                    context,
                    first_origin,
                )?
                .into_iter()
                .next()
            else {
                diagnostics.push(unresolvable_condition(name, span));
                return Err(CallOutcome::Failed);
            };
            condition
        };
        match resolve_boolean_value(&condition) {
            Ok(value) => Ok(value),
            Err(error) => {
                diagnostics.push(conversion_failure_diagnostic(
                    value_conversion::ConversionFailure::new(
                        error,
                        Some(*condition_span),
                        parameter.map(|parameter| parameter.name.clone()),
                        parameter.and_then(|parameter| parameter.name_span),
                        *span,
                    ),
                    Some(name),
                ));
                Err(CallOutcome::Failed)
            }
        }
    }

    /// Produces conditional content after the condition has selected the
    /// branch. The body and body-like arguments are evaluated here, not before
    /// dispatch.
    fn conditional_content_value(
        &self,
        bound: &invocation_binder::BoundInvocation<IrValue>,
        body: Option<CallBody<'_>>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
    ) -> CallOutcome {
        if let Some(body) = body {
            return self.evaluate_call_body(body, span, diagnostics, context);
        }
        if let Some(BoundSlot::Explicit { value, .. }) = bound.slots.get(1) {
            return self.evaluate_content_argument(value, span, diagnostics, context);
        }
        CallOutcome::Value(IrValue::Content(Vec::new()))
    }

    fn evaluate_content_argument(
        &self,
        value: &IrValue,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
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
            IrValue::InlineBody(body) => {
                self.validate_preserved_value(&IrValue::Content(body.content.clone()), diagnostics)
            }
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
            IrValue::Callable(callable) => {
                diagnostics.push(iteration_error(
                    "A callable cannot be preserved as an unresolved call argument".to_string(),
                    callable.span,
                ));
                Err(CallOutcome::Failed)
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
        context: &mut EvaluationContext<'_>,
    ) -> Result<IrValue, CallOutcome> {
        match value {
            IrValue::InlineBody(body) => self.preserve_value_expression(
                &IrValue::Content(body.content.clone()),
                diagnostics,
                context,
            ),
            IrValue::Content(nodes) => {
                if let [IrNode::FunctionCall {
                    name,
                    positional_args,
                    named_args,
                    ordered_args,
                    lambda_parameters,
                    body,
                    raw_body,
                    span,
                    ..
                }] = nodes.as_slice()
                {
                    return self
                        .preserve_block_call(
                            name,
                            ordered_args.as_deref(),
                            positional_args,
                            named_args,
                            lambda_parameters.as_deref(),
                            body.as_deref(),
                            raw_body.as_ref(),
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
        ordered_args: Option<&[IrCallArgument]>,
        positional_args: &[IrValue],
        named_args: &[IrNamedArg],
        lambda_parameters: Option<&[IrParameter]>,
        body: Option<&[IrNode]>,
        raw_body: Option<&IrRawBody>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
    ) -> Result<Vec<IrNode>, CallOutcome> {
        // Preservation evaluates nested argument/body syntax to retain the
        // same structured form as ordinary output evaluation. It must not
        // publish mutations from an invocation that already returned
        // `Unresolved`; keep those evaluation effects in a disposable
        // savepoint even when preservation itself succeeds.
        context.begin_invocation();
        let checkpoint = InvocationCheckpoint::capture();
        let result = self.preserve_block_call_inner(
            name,
            ordered_args,
            positional_args,
            named_args,
            lambda_parameters,
            body,
            raw_body,
            span,
            diagnostics,
            context,
        );
        checkpoint.restore(context);
        context.end_invocation();
        result
    }

    #[allow(clippy::too_many_arguments)]
    fn preserve_block_call_inner(
        &self,
        name: &str,
        ordered_args: Option<&[IrCallArgument]>,
        positional_args: &[IrValue],
        named_args: &[IrNamedArg],
        lambda_parameters: Option<&[IrParameter]>,
        body: Option<&[IrNode]>,
        raw_body: Option<&IrRawBody>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
    ) -> Result<Vec<IrNode>, CallOutcome> {
        validate_ordered_invocation(
            name,
            ordered_args,
            positional_args,
            named_args,
            *span,
            diagnostics,
        )?;
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
            ordered_args: ordered_args.map(ToOwned::to_owned),
            lambda_parameters: lambda_parameters.map(ToOwned::to_owned),
            body,
            raw_body: raw_body.cloned(),
            span: *span,
        }])
    }

    #[allow(clippy::too_many_arguments)]
    fn preserve_inline_call(
        &self,
        name: &str,
        ordered_args: Option<&[IrCallArgument]>,
        positional_args: &[IrValue],
        named_args: &[IrNamedArg],
        body: Option<&[IrInline]>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
    ) -> Result<Vec<IrInline>, CallOutcome> {
        context.begin_invocation();
        let checkpoint = InvocationCheckpoint::capture();
        let result = self.preserve_inline_call_inner(
            name,
            ordered_args,
            positional_args,
            named_args,
            body,
            span,
            diagnostics,
            context,
        );
        checkpoint.restore(context);
        context.end_invocation();
        result
    }

    #[allow(clippy::too_many_arguments)]
    fn preserve_inline_call_inner(
        &self,
        name: &str,
        ordered_args: Option<&[IrCallArgument]>,
        positional_args: &[IrValue],
        named_args: &[IrNamedArg],
        body: Option<&[IrInline]>,
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
    ) -> Result<Vec<IrInline>, CallOutcome> {
        validate_ordered_invocation(
            name,
            ordered_args,
            positional_args,
            named_args,
            *span,
            diagnostics,
        )?;
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
            ordered_args: ordered_args.map(ToOwned::to_owned),
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
            IrValue::Component(component) => Ok(vec![IrNode::Component { component }]),
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
            Some(IrValue::Component(component)) => {
                diagnostics.push(component_inline_materialization_error(component.span()));
                Vec::new()
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
            IrNode::TargetSpecificContent { content } => {
                vec![IrInline::TargetSpecificContent { content }]
            }
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
        context: &mut EvaluationContext<'_>,
    ) -> CallOutcome {
        match value {
            IrValue::InlineBody(body) => self.evaluate_value(
                &IrValue::Content(body.content.clone()),
                diagnostics,
                context,
            ),
            IrValue::Content(nodes) => {
                if let [IrNode::FunctionCall {
                    name,
                    positional_args,
                    named_args,
                    ordered_args,
                    lambda_parameters,
                    body,
                    raw_body,
                    span,
                    ..
                }] = nodes.as_slice()
                {
                    return self.evaluate_call_value_with_ordered(
                        name,
                        ordered_args.as_deref(),
                        positional_args,
                        named_args,
                        body.as_deref().map(CallBody::Block),
                        raw_body.as_ref(),
                        lambda_parameters.as_deref(),
                        span,
                        diagnostics,
                        context,
                    );
                }
                if let [IrNode::ChainedFunctionCall {
                    head,
                    chain,
                    body,
                    raw_body,
                    ..
                }] = nodes.as_slice()
                {
                    return self.evaluate_chain_value(
                        head,
                        chain,
                        body.as_deref().map(CallBody::Block),
                        raw_body.as_ref(),
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
            IrValue::Callable(callable) => {
                if callable.capture.is_some() {
                    CallOutcome::Value(value.clone())
                } else {
                    let mut callable = callable.clone();
                    callable.capture = Some(Box::new(context.capture_snapshot()));
                    CallOutcome::Value(IrValue::Callable(callable))
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
        context: &mut EvaluationContext<'_>,
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

    /// Evaluates arguments while preserving the invocation-time distinction
    /// used by Quarkdown's `RegularArgumentsBinder`. A raw scalar or a
    /// variable/custom-function reference is dynamic; a nested builtin result
    /// such as `.string` is already a static semantic value.
    fn evaluate_invocation_values(
        &self,
        values: &[IrValue],
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
        first_origin: Option<ValueOrigin>,
    ) -> Result<Vec<InvocationValue>, CallOutcome> {
        let mut evaluated = Vec::new();
        if let Err(error) = evaluated.try_reserve(values.len()) {
            diagnostics.push(iteration_error(
                format!("call arguments cannot be allocated: {error}"),
                *span,
            ));
            return Err(CallOutcome::Failed);
        }
        for (index, value) in values.iter().enumerate() {
            let origin = if index == 0 {
                first_origin.unwrap_or_else(|| invocation_origin(value, context))
            } else {
                invocation_origin(value, context)
            };
            let evaluated_value = match self.evaluate_value(value, diagnostics, context) {
                CallOutcome::Value(value) => value,
                CallOutcome::Unresolved => {
                    self.preserve_value_expression(value, diagnostics, context)?
                }
                CallOutcome::NoValue => {
                    diagnostics.push(no_value_required(value_source_span(value, span)));
                    return Err(CallOutcome::Failed);
                }
                CallOutcome::Failed => return Err(CallOutcome::Failed),
            };
            evaluated.push(InvocationValue {
                value: evaluated_value,
                origin,
            });
        }
        Ok(evaluated)
    }

    /// Evaluates source-ordered candidates after binding preflight. The
    /// result is later projected for target-specific conversion, but the
    /// evaluation itself remains in source order.
    #[allow(clippy::too_many_arguments)]
    fn evaluate_invocation_candidates(
        &self,
        ordered: Option<&[IrCallArgument]>,
        positional: &[IrValue],
        named: &[IrNamedArg],
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
        first_origin: Option<ValueOrigin>,
        implicit_argument: Option<&InvocationValue>,
    ) -> Result<Vec<Candidate<InvocationValue>>, CallOutcome> {
        let Some(ordered) = ordered else {
            let evaluated_positional = self.evaluate_invocation_values(
                positional,
                span,
                diagnostics,
                context,
                first_origin,
            )?;
            let evaluated_named =
                self.evaluate_invocation_named(named, span, diagnostics, context)?;
            let mut candidates =
                Vec::with_capacity(evaluated_positional.len() + evaluated_named.len());
            candidates.extend(positional.iter().zip(evaluated_positional).map(
                |(source, value)| Candidate::Positional {
                    span: value_source_span(source, span),
                    value,
                },
            ));
            candidates.extend(
                evaluated_named
                    .into_iter()
                    .map(|argument| Candidate::Named {
                        name: argument.arg.name.clone(),
                        name_span: argument.arg.name_span,
                        span: argument.arg.span,
                        value: InvocationValue {
                            value: argument.arg.value,
                            origin: argument.origin,
                        },
                    }),
            );
            return Ok(candidates);
        };

        let mut candidates = Vec::new();
        if let Err(error) = candidates.try_reserve(ordered.len()) {
            diagnostics.push(iteration_error(
                format!("call arguments cannot be allocated: {error}"),
                *span,
            ));
            return Err(CallOutcome::Failed);
        }
        let mut first_positional = implicit_argument.is_none();
        for (index, argument) in ordered.iter().enumerate() {
            if index == 0 {
                if let Some(implicit_argument) = implicit_argument {
                    let IrCallArgument::Positional { index: 0, span } = argument else {
                        return Err(CallOutcome::Failed);
                    };
                    candidates.push(Candidate::Positional {
                        value: implicit_argument.clone(),
                        span: *span,
                    });
                    continue;
                }
            }
            let (raw, candidate) = match argument {
                IrCallArgument::Positional {
                    index,
                    span: arg_span,
                } => {
                    let Some(value) = positional.get(*index) else {
                        return Err(CallOutcome::Failed);
                    };
                    (value, CandidateKind::Positional(*arg_span))
                }
                IrCallArgument::Named {
                    index,
                    name_span,
                    span: arg_span,
                } => {
                    let Some(argument) = named.get(*index) else {
                        return Err(CallOutcome::Failed);
                    };
                    (
                        &argument.value,
                        CandidateKind::Named(&argument.name, *name_span, *arg_span),
                    )
                }
            };
            let origin = match candidate {
                CandidateKind::Positional(_) if first_positional => {
                    first_positional = false;
                    first_origin.unwrap_or_else(|| invocation_origin(raw, context))
                }
                _ => invocation_origin(raw, context),
            };
            let value = match self.evaluate_value(raw, diagnostics, context) {
                CallOutcome::Value(value) => value,
                CallOutcome::Unresolved => {
                    self.preserve_value_expression(raw, diagnostics, context)?
                }
                CallOutcome::NoValue => {
                    diagnostics.push(no_value_required(value_source_span(raw, span)));
                    return Err(CallOutcome::Failed);
                }
                CallOutcome::Failed => return Err(CallOutcome::Failed),
            };
            let value = InvocationValue { value, origin };
            candidates.push(match candidate {
                CandidateKind::Positional(arg_span) => Candidate::Positional {
                    value,
                    span: arg_span,
                },
                CandidateKind::Named(name, name_span, arg_span) => Candidate::Named {
                    name: name.clone(),
                    name_span,
                    value,
                    span: arg_span,
                },
            });
        }
        Ok(candidates)
    }

    fn evaluate_invocation_named(
        &self,
        named: &[IrNamedArg],
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
    ) -> Result<Vec<InvocationNamedArg>, CallOutcome> {
        let mut evaluated = Vec::new();
        if let Err(error) = evaluated.try_reserve(named.len()) {
            diagnostics.push(iteration_error(
                format!("named call arguments cannot be allocated: {error}"),
                *span,
            ));
            return Err(CallOutcome::Failed);
        }
        for argument in named {
            let origin = invocation_origin(&argument.value, context);
            let value = match self.evaluate_value(&argument.value, diagnostics, context) {
                CallOutcome::Value(value) => value,
                CallOutcome::Unresolved => {
                    self.preserve_value_expression(&argument.value, diagnostics, context)?
                }
                CallOutcome::NoValue => {
                    diagnostics.push(no_value_required(value_source_span(&argument.value, span)));
                    return Err(CallOutcome::Failed);
                }
                CallOutcome::Failed => return Err(CallOutcome::Failed),
            };
            evaluated.push(InvocationNamedArg::new(
                IrNamedArg {
                    name: argument.name.clone(),
                    name_span: argument.name_span,
                    value,
                    span: argument.span,
                },
                origin,
            ));
        }
        Ok(evaluated)
    }

    fn evaluate_named(
        &self,
        named: &[IrNamedArg],
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
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
        context: &mut EvaluationContext<'_>,
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
        context: &mut EvaluationContext<'_>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CaptionPositionParameter {
    Default,
    Figures,
    Tables,
    CodeBlocks,
}

impl CaptionPositionParameter {
    fn name(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Figures => "figures",
            Self::Tables => "tables",
            Self::CodeBlocks => "code",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CaptionPositionArgumentLocation {
    Positional(usize),
    Named(usize),
    Body,
}

#[derive(Debug, Clone, Copy, Default)]
struct CaptionPositionBindings {
    default: Option<CaptionPositionArgumentLocation>,
    figures: Option<CaptionPositionArgumentLocation>,
    tables: Option<CaptionPositionArgumentLocation>,
    code_blocks: Option<CaptionPositionArgumentLocation>,
}

impl CaptionPositionBindings {
    fn slot_mut(
        &mut self,
        parameter: CaptionPositionParameter,
    ) -> &mut Option<CaptionPositionArgumentLocation> {
        match parameter {
            CaptionPositionParameter::Default => &mut self.default,
            CaptionPositionParameter::Figures => &mut self.figures,
            CaptionPositionParameter::Tables => &mut self.tables,
            CaptionPositionParameter::CodeBlocks => &mut self.code_blocks,
        }
    }
}

fn bind_caption_position_arguments(
    plan: &BindingPlan,
    positional_args: &[IrValue],
    named_args: &[IrNamedArg],
    body: Option<&Candidate<CaptionPositionArgumentLocation>>,
    span: &SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<CaptionPositionBindings, CallOutcome> {
    let mut candidates = Vec::with_capacity(positional_args.len() + named_args.len());
    candidates.extend(positional_args.iter().enumerate().map(|(index, value)| {
        Candidate::Positional {
            value: CaptionPositionArgumentLocation::Positional(index),
            span: value_source_span(value, span),
        }
    }));
    candidates.extend(
        named_args
            .iter()
            .enumerate()
            .map(|(index, argument)| Candidate::Named {
                name: argument.name.clone(),
                name_span: argument.name_span,
                value: CaptionPositionArgumentLocation::Named(index),
                span: argument.span,
            }),
    );
    let bound = plan.bind(&candidates, body, *span).map_err(|error| {
        let message = if let Some(name) = error.message.strip_prefix("unknown named argument ") {
            format!("Unknown named argument {name} for `.captionposition`")
        } else if error.message == "received too many positional arguments" {
            "`.captionposition` accepts at most four positional arguments".to_string()
        } else if let Some(parameter) = error
            .message
            .strip_prefix("parameter ")
            .and_then(|message| message.strip_suffix(" collides with an already bound argument"))
        {
            format!("`.captionposition` received the {parameter} argument more than once")
        } else if let Some(name) = error
            .message
            .strip_prefix("named argument `")
            .and_then(|message| message.strip_suffix("` was supplied more than once"))
        {
            format!("`.captionposition` received the `{name}` argument more than once")
        } else {
            error.message.clone()
        };
        let mut diagnostic = binding_diagnostic_with_code(error, "E3003");
        diagnostic.message = message;
        diagnostics.push(diagnostic);
        CallOutcome::Failed
    })?;

    let mut bindings = CaptionPositionBindings::default();
    for (parameter, slot) in [
        CaptionPositionParameter::Default,
        CaptionPositionParameter::Figures,
        CaptionPositionParameter::Tables,
        CaptionPositionParameter::CodeBlocks,
    ]
    .into_iter()
    .zip(bound.slots)
    {
        if let BoundSlot::Explicit { value, .. } = slot {
            *bindings.slot_mut(parameter) = Some(value);
        }
    }
    Ok(bindings)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NativeDispatchOwner {
    #[cfg(test)]
    RegularScalar,
    Conditional,
    DocumentState,
    Html,
    Markdown,
    Resource,
    Let,
    Foreach,
    Repeat,
    OptionalityCallback,
    VariableState,
    Center,
    Align,
    Container,
    Landscape,
    Br,
    Whitespace,
    StackedLayout,
    Range,
    Pair,
    Dictionary,
    CollectionAccess,
    CollectionTransform,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NativeOwnerInventory {
    pub(crate) owner: NativeDispatchOwner,
    pub(crate) names: &'static [&'static str],
}

const CONDITIONAL_NATIVE_NAMES: &[&str] = &["if", "ifnot"];
const DOCUMENT_STATE_NATIVE_NAMES: &[&str] = &[
    "docname",
    "docdescription",
    "doctype",
    "docauthor",
    "docauthors",
    "dockeywords",
    "doclang",
    "theme",
    "captionposition",
];
const HTML_NATIVE_NAMES: &[&str] = &["html"];
const MARKDOWN_NATIVE_NAMES: &[&str] = &["markdown"];
const RESOURCE_NATIVE_NAMES: &[&str] = &["read", "json", "include"];
const LET_NATIVE_NAMES: &[&str] = &["let"];
const FOREACH_NATIVE_NAMES: &[&str] = &["foreach"];
const REPEAT_NATIVE_NAMES: &[&str] = &["repeat"];
const OPTIONALITY_CALLBACK_NATIVE_NAMES: &[&str] = &["ifpresent", "takeif"];
const VARIABLE_STATE_NATIVE_NAMES: &[&str] = &["var"];
const CENTER_NATIVE_NAMES: &[&str] = &["center"];
const ALIGN_NATIVE_NAMES: &[&str] = &["align"];
const CONTAINER_NATIVE_NAMES: &[&str] = &["container"];
const LANDSCAPE_NATIVE_NAMES: &[&str] = &["landscape"];
const BR_NATIVE_NAMES: &[&str] = &["br"];
const WHITESPACE_NATIVE_NAMES: &[&str] = &["whitespace"];
const STACKED_LAYOUT_NATIVE_NAMES: &[&str] = &["row", "column", "grid"];
const RANGE_NATIVE_NAMES: &[&str] = &["range"];
const PAIR_NATIVE_NAMES: &[&str] = &["pair"];
const DICTIONARY_NATIVE_NAMES: &[&str] = &["dictionary"];
const COLLECTION_ACCESS_NATIVE_NAMES: &[&str] = &[
    "size",
    "first",
    "second",
    "third",
    "last",
    "getat",
    "sumall",
    "average",
    "distinct",
    "reversed",
    "groupvalues",
];
const COLLECTION_TRANSFORM_NATIVE_NAMES: &[&str] = &["map", "filter", "sorted"];
const DEFERRED_NATIVE_NAMES: &[&str] = &["llmstxt"];

static BESPOKE_NATIVE_OWNERS: &[NativeOwnerInventory] = &[
    NativeOwnerInventory {
        owner: NativeDispatchOwner::Conditional,
        names: CONDITIONAL_NATIVE_NAMES,
    },
    NativeOwnerInventory {
        owner: NativeDispatchOwner::DocumentState,
        names: DOCUMENT_STATE_NATIVE_NAMES,
    },
    NativeOwnerInventory {
        owner: NativeDispatchOwner::Html,
        names: HTML_NATIVE_NAMES,
    },
    NativeOwnerInventory {
        owner: NativeDispatchOwner::Markdown,
        names: MARKDOWN_NATIVE_NAMES,
    },
    NativeOwnerInventory {
        owner: NativeDispatchOwner::Resource,
        names: RESOURCE_NATIVE_NAMES,
    },
    NativeOwnerInventory {
        owner: NativeDispatchOwner::Let,
        names: LET_NATIVE_NAMES,
    },
    NativeOwnerInventory {
        owner: NativeDispatchOwner::Foreach,
        names: FOREACH_NATIVE_NAMES,
    },
    NativeOwnerInventory {
        owner: NativeDispatchOwner::Repeat,
        names: REPEAT_NATIVE_NAMES,
    },
    NativeOwnerInventory {
        owner: NativeDispatchOwner::OptionalityCallback,
        names: OPTIONALITY_CALLBACK_NATIVE_NAMES,
    },
    NativeOwnerInventory {
        owner: NativeDispatchOwner::VariableState,
        names: VARIABLE_STATE_NATIVE_NAMES,
    },
    NativeOwnerInventory {
        owner: NativeDispatchOwner::Center,
        names: CENTER_NATIVE_NAMES,
    },
    NativeOwnerInventory {
        owner: NativeDispatchOwner::Align,
        names: ALIGN_NATIVE_NAMES,
    },
    NativeOwnerInventory {
        owner: NativeDispatchOwner::Container,
        names: CONTAINER_NATIVE_NAMES,
    },
    NativeOwnerInventory {
        owner: NativeDispatchOwner::Landscape,
        names: LANDSCAPE_NATIVE_NAMES,
    },
    NativeOwnerInventory {
        owner: NativeDispatchOwner::Br,
        names: BR_NATIVE_NAMES,
    },
    NativeOwnerInventory {
        owner: NativeDispatchOwner::Whitespace,
        names: WHITESPACE_NATIVE_NAMES,
    },
    NativeOwnerInventory {
        owner: NativeDispatchOwner::StackedLayout,
        names: STACKED_LAYOUT_NATIVE_NAMES,
    },
    NativeOwnerInventory {
        owner: NativeDispatchOwner::Range,
        names: RANGE_NATIVE_NAMES,
    },
    NativeOwnerInventory {
        owner: NativeDispatchOwner::Pair,
        names: PAIR_NATIVE_NAMES,
    },
    NativeOwnerInventory {
        owner: NativeDispatchOwner::Dictionary,
        names: DICTIONARY_NATIVE_NAMES,
    },
    NativeOwnerInventory {
        owner: NativeDispatchOwner::CollectionAccess,
        names: COLLECTION_ACCESS_NATIVE_NAMES,
    },
    NativeOwnerInventory {
        owner: NativeDispatchOwner::CollectionTransform,
        names: COLLECTION_TRANSFORM_NATIVE_NAMES,
    },
];

#[cfg(test)]
pub(crate) fn bespoke_native_owners() -> &'static [NativeOwnerInventory] {
    BESPOKE_NATIVE_OWNERS
}

#[cfg(test)]
pub(crate) fn deferred_native_names() -> &'static [&'static str] {
    DEFERRED_NATIVE_NAMES
}

#[cfg(test)]
pub(crate) fn native_dispatch_owner(name: &str) -> Option<NativeDispatchOwner> {
    let regular = builtins::lookup(name).is_some();
    let bespoke = BESPOKE_NATIVE_OWNERS
        .iter()
        .filter(|inventory| inventory.names.contains(&name))
        .map(|inventory| inventory.owner)
        .collect::<Vec<_>>();
    if regular && bespoke.is_empty() {
        return Some(NativeDispatchOwner::RegularScalar);
    }
    if !regular && bespoke.len() == 1 {
        return bespoke.into_iter().next();
    }
    None
}

fn has_native_owner(name: &str, owner: NativeDispatchOwner) -> bool {
    BESPOKE_NATIVE_OWNERS
        .iter()
        .any(|inventory| inventory.owner == owner && inventory.names.contains(&name))
}

fn is_stacked_layout(name: &str) -> bool {
    has_native_owner(name, NativeDispatchOwner::StackedLayout)
}

fn is_center(name: &str) -> bool {
    has_native_owner(name, NativeDispatchOwner::Center)
}

fn is_align(name: &str) -> bool {
    has_native_owner(name, NativeDispatchOwner::Align)
}

fn is_container(name: &str) -> bool {
    has_native_owner(name, NativeDispatchOwner::Container)
}

fn is_landscape(name: &str) -> bool {
    has_native_owner(name, NativeDispatchOwner::Landscape)
}

fn is_br(name: &str) -> bool {
    has_native_owner(name, NativeDispatchOwner::Br)
}

fn is_whitespace(name: &str) -> bool {
    has_native_owner(name, NativeDispatchOwner::Whitespace)
}

fn is_document_state(name: &str) -> bool {
    has_native_owner(name, NativeDispatchOwner::DocumentState)
}

fn is_html(name: &str) -> bool {
    has_native_owner(name, NativeDispatchOwner::Html)
}

fn is_markdown(name: &str) -> bool {
    has_native_owner(name, NativeDispatchOwner::Markdown)
}

fn is_resource(name: &str) -> bool {
    has_native_owner(name, NativeDispatchOwner::Resource)
}

fn is_deferred(name: &str) -> bool {
    DEFERRED_NATIVE_NAMES.contains(&name)
}

fn bind_whitespace_arguments(
    plan: &BindingPlan,
    positional: Vec<WhitespaceArgument>,
    named: Vec<InvocationNamedArg>,
    span: &SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<BoundWhitespaceArguments, CallOutcome> {
    let mut candidates = Vec::with_capacity(positional.len() + named.len());
    candidates.extend(
        positional
            .into_iter()
            .map(|argument| Candidate::Positional {
                value: argument.value,
                span: argument.span,
            }),
    );
    candidates.extend(named.into_iter().map(|argument| Candidate::Named {
        name: argument.arg.name,
        name_span: argument.arg.name_span,
        value: InvocationValue {
            value: argument.arg.value,
            origin: argument.origin,
        },
        span: argument.arg.span,
    }));
    let bound = plan.bind(&candidates, None, *span).map_err(|error| {
        diagnostics.push(binding_diagnostic(error));
        CallOutcome::Failed
    })?;
    let mut slots = bound.slots.into_iter().enumerate();
    let parameters = bound.parameters;
    let to_argument = |(index, slot): (usize, BoundSlot<InvocationValue>)| match slot {
        BoundSlot::Explicit { value, span } => Some(WhitespaceArgument {
            value,
            span,
            parameter_span: parameters
                .get(index)
                .and_then(|parameter| parameter.name_span),
        }),
        BoundSlot::Omitted | BoundSlot::Defaulted => None,
    };
    Ok(BoundWhitespaceArguments {
        width: slots.next().and_then(to_argument),
        height: slots.next().and_then(to_argument),
    })
}

fn convert_whitespace_size(
    argument: &InvocationValue,
) -> Result<Option<IrSize>, value_conversion::ConversionError> {
    if matches!(&argument.value, IrValue::None) {
        return Ok(None);
    }
    match value_conversion::convert_domain_with_origin(
        argument,
        value_conversion::DomainTarget::Size,
    )? {
        value_conversion::DomainValue::Size(value) => Ok(Some(value)),
        _ => Err(value_conversion::ConversionError::UnsupportedValue {
            target: value_conversion::ConversionTarget::Size,
        }),
    }
}

fn zero_whitespace_size() -> IrSize {
    IrSize {
        value: 0.0,
        unit: IrSizeUnit::Px,
    }
}

fn bind_container_arguments(
    plan: &BindingPlan,
    positional: Vec<ContainerArgument>,
    named: Vec<InvocationNamedArg>,
    span: &SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<BoundContainerArguments, CallOutcome> {
    let candidates = invocation_candidates(
        positional
            .into_iter()
            .map(|argument| (argument.value, argument.span))
            .collect(),
        named,
    );
    let bound = plan.bind(&candidates, None, *span).map_err(|error| {
        let message = if let Some(name) = error.message.strip_prefix("unknown named argument ") {
            format!("Unknown named argument {name}")
        } else if error.message == "received too many positional arguments" {
            "`.container` accepts at most three positional arguments".to_string()
        } else if let Some(parameter) = error
            .message
            .strip_prefix("parameter ")
            .and_then(|message| message.strip_suffix(" collides with an already bound argument"))
        {
            format!("Argument {parameter} was bound more than once")
        } else {
            error.message.clone()
        };
        let mut diagnostic = binding_diagnostic_with_code(error, "E3001");
        diagnostic.message = message;
        diagnostics.push(diagnostic);
        CallOutcome::Failed
    })?;
    let mut slots = bound.slots.into_iter().enumerate();
    let parameters = bound.parameters;
    let take = |slot: Option<(usize, BoundSlot<InvocationValue>)>| match slot {
        Some((index, BoundSlot::Explicit { value, span })) => Some(ContainerArgument {
            value,
            span,
            parameter_span: parameters
                .get(index)
                .and_then(|parameter| parameter.name_span),
        }),
        Some((_, BoundSlot::Omitted | BoundSlot::Defaulted)) | None => None,
    };
    let width = take(slots.next());
    let height = take(slots.next());
    let full_width = take(slots.next());
    Ok(BoundContainerArguments {
        width,
        height,
        full_width,
    })
}

fn convert_container_size(
    argument: &InvocationValue,
) -> Result<IrSize, value_conversion::ConversionError> {
    if matches!(&argument.value, IrValue::None) {
        return Err(value_conversion::ConversionError::UnsupportedValue {
            target: value_conversion::ConversionTarget::Size,
        });
    }
    match value_conversion::convert_domain_with_origin(
        argument,
        value_conversion::DomainTarget::Size,
    )? {
        value_conversion::DomainValue::Size(value) => Ok(value),
        _ => Err(value_conversion::ConversionError::UnsupportedValue {
            target: value_conversion::ConversionTarget::Size,
        }),
    }
}

fn convert_container_boolean(
    argument: &InvocationValue,
) -> Result<bool, value_conversion::ConversionError> {
    match value_conversion::convert_scalar_with_origin(argument, ScalarTarget::Boolean)? {
        ScalarValue::Boolean(value) => Ok(value),
        _ => Err(value_conversion::ConversionError::UnsupportedValue {
            target: value_conversion::ConversionTarget::Boolean,
        }),
    }
}

fn container_conversion_error(
    parameter: &str,
    span: SourceSpan,
    parameter_span: Option<SourceSpan>,
    error: value_conversion::ConversionError,
) -> Diagnostic {
    conversion_failure_diagnostic(
        value_conversion::ConversionFailure::new(
            error,
            Some(span),
            Some(parameter),
            parameter_span,
            span,
        ),
        Some("`.container`"),
    )
}

fn container_argument_error(message: &str, span: SourceSpan) -> Diagnostic {
    container_argument_error_at(message.to_string(), span)
}

fn container_argument_error_at(message: String, span: SourceSpan) -> Diagnostic {
    Diagnostic {
        code: "E3001".to_string(),
        severity: Severity::Error,
        message,
        primary: Some(span),
        secondary: Vec::new(),
        hints: vec![
            "Container arguments are validated before the Markdown body is evaluated.".to_string(),
        ],
    }
}

fn bind_align_argument(
    plan: &BindingPlan,
    positional: Vec<AlignArgument>,
    named: Vec<InvocationNamedArg>,
    span: &SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<AlignArgument, CallOutcome> {
    let candidates = invocation_candidates(
        positional
            .into_iter()
            .map(|argument| (argument.value, argument.span))
            .collect(),
        named,
    );
    let bound = plan.bind(&candidates, None, *span).map_err(|error| {
        let message = if let Some(name) = error.message.strip_prefix("unknown named argument ") {
            format!("Unknown named argument {name}")
        } else if error.message == "received too many positional arguments" {
            "`.align` accepts exactly one `alignment` argument".to_string()
        } else if error.message.starts_with("missing required argument") {
            "`.align` requires the `alignment` argument".to_string()
        } else if let Some(parameter) = error
            .message
            .strip_prefix("parameter ")
            .and_then(|message| message.strip_suffix(" collides with an already bound argument"))
        {
            format!("Argument {parameter} was bound more than once")
        } else {
            error.message.clone()
        };
        let mut diagnostic = binding_diagnostic_with_code(error, "E3001");
        diagnostic.message = message;
        diagnostics.push(diagnostic);
        CallOutcome::Failed
    })?;
    let parameter_span = bound
        .parameters
        .first()
        .and_then(|parameter| parameter.name_span);
    match bound.slots.into_iter().next() {
        Some(BoundSlot::Explicit { value, span }) => Ok(AlignArgument {
            value,
            span,
            parameter_span,
        }),
        _ => Err(CallOutcome::Failed),
    }
}

fn convert_align_alignment(
    argument: &InvocationValue,
) -> Result<IrContainerAlignment, value_conversion::ConversionError> {
    match value_conversion::convert_domain_with_origin(
        argument,
        value_conversion::DomainTarget::ClosedEnum(
            value_conversion::ClosedEnumTarget::ContainerAlignment,
        ),
    )? {
        value_conversion::DomainValue::Enum(IrEnumValue::ContainerAlignment(value)) => Ok(value),
        value_conversion::DomainValue::Enum(_) => {
            Err(value_conversion::ConversionError::UnsupportedValue {
                target: value_conversion::ConversionTarget::Enum,
            })
        }
        _ => Err(value_conversion::ConversionError::UnsupportedValue {
            target: value_conversion::ConversionTarget::Enum,
        }),
    }
}

fn bind_stacked_arguments(
    plan: &BindingPlan,
    name: &str,
    positional: Vec<StackedArgument>,
    named: Vec<InvocationNamedArg>,
    span: &SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<BoundStackedArguments, CallOutcome> {
    let candidates = invocation_candidates(
        positional
            .into_iter()
            .map(|argument| (argument.value, argument.span))
            .collect(),
        named,
    );
    let bound = plan.bind(&candidates, None, *span).map_err(|error| {
        let message = if let Some(name) = error.message.strip_prefix("unknown named argument ") {
            format!("Unknown named argument {name}")
        } else if error.message == "received too many positional arguments" {
            "too many positional arguments".to_string()
        } else if let Some(parameter) = error
            .message
            .strip_prefix("parameter ")
            .and_then(|message| message.strip_suffix(" collides with an already bound argument"))
        {
            format!("Argument {parameter} was bound more than once")
        } else {
            error.message.clone()
        };
        let mut diagnostic = binding_diagnostic_with_code(error, "E3001");
        diagnostic.message = format!("`.{name}` {message}");
        diagnostics.push(diagnostic);
        CallOutcome::Failed
    })?;
    let parameters = bound.parameters;
    let values = bound
        .slots
        .into_iter()
        .enumerate()
        .map(|(index, slot)| match slot {
            BoundSlot::Explicit { value, span } => Some(StackedArgument {
                value,
                span,
                parameter_span: parameters
                    .get(index)
                    .and_then(|parameter| parameter.name_span),
            }),
            BoundSlot::Omitted | BoundSlot::Defaulted => None,
        })
        .collect();
    Ok(BoundStackedArguments { values })
}

fn convert_stacked_main_axis(
    argument: &InvocationValue,
) -> Result<IrMainAxisAlignment, value_conversion::ConversionError> {
    match value_conversion::convert_domain_with_origin(
        argument,
        value_conversion::DomainTarget::ClosedEnum(
            value_conversion::ClosedEnumTarget::StackedMainAxisAlignment,
        ),
    )? {
        value_conversion::DomainValue::Enum(IrEnumValue::StackedMainAxisAlignment(value)) => {
            Ok(value)
        }
        value_conversion::DomainValue::Enum(_) => {
            Err(value_conversion::ConversionError::UnsupportedValue {
                target: value_conversion::ConversionTarget::Enum,
            })
        }
        _ => Err(value_conversion::ConversionError::UnsupportedValue {
            target: value_conversion::ConversionTarget::Enum,
        }),
    }
}

fn convert_stacked_cross_axis(
    argument: &InvocationValue,
) -> Result<IrCrossAxisAlignment, value_conversion::ConversionError> {
    match value_conversion::convert_domain_with_origin(
        argument,
        value_conversion::DomainTarget::ClosedEnum(
            value_conversion::ClosedEnumTarget::StackedCrossAxisAlignment,
        ),
    )? {
        value_conversion::DomainValue::Enum(IrEnumValue::StackedCrossAxisAlignment(value)) => {
            Ok(value)
        }
        value_conversion::DomainValue::Enum(_) => {
            Err(value_conversion::ConversionError::UnsupportedValue {
                target: value_conversion::ConversionTarget::Enum,
            })
        }
        _ => Err(value_conversion::ConversionError::UnsupportedValue {
            target: value_conversion::ConversionTarget::Enum,
        }),
    }
}

fn convert_stacked_size(
    argument: &InvocationValue,
) -> Result<IrSize, value_conversion::ConversionError> {
    match value_conversion::convert_domain_with_origin(
        argument,
        value_conversion::DomainTarget::Size,
    )? {
        value_conversion::DomainValue::Size(value) => Ok(value),
        _ => Err(value_conversion::ConversionError::UnsupportedValue {
            target: value_conversion::ConversionTarget::Size,
        }),
    }
}

fn convert_optional_stacked_size(
    argument: &InvocationValue,
) -> Result<Option<IrSize>, value_conversion::ConversionError> {
    if matches!(argument.value, IrValue::None) {
        Ok(None)
    } else {
        convert_stacked_size(argument).map(Some)
    }
}

fn stacked_conversion_error(
    name: &str,
    parameter: &str,
    span: SourceSpan,
    parameter_span: Option<SourceSpan>,
    error: value_conversion::ConversionError,
) -> Diagnostic {
    let context = format!("`.{name}`");
    conversion_failure_diagnostic(
        value_conversion::ConversionFailure::new(
            error,
            Some(span),
            Some(parameter),
            parameter_span,
            span,
        ),
        Some(context.as_str()),
    )
}

fn stacked_argument_error(
    name: &str,
    parameter: &str,
    span: SourceSpan,
    message: &str,
) -> Diagnostic {
    stacked_argument_error_at(
        format!("`.{name}` parameter `{parameter}`: {message}"),
        span,
    )
}

fn stacked_argument_error_at(message: String, span: SourceSpan) -> Diagnostic {
    Diagnostic {
        code: "E3001".to_string(),
        severity: Severity::Error,
        message,
        primary: Some(span),
        secondary: Vec::new(),
        hints: vec![
            "Stacked layout arguments are validated before the Markdown body is evaluated."
                .to_string(),
        ],
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
        let tail = content.get(1..).unwrap_or_default();
        if tail.is_empty() {
            return Some((key, Vec::new(), trimmed.to_string(), value_span));
        }
        let mut value_inlines = Vec::with_capacity(tail.len() + 1);
        value_inlines.push(IrInline::Text {
            content: trimmed.to_string(),
            span: value_span,
        });
        value_inlines.extend_from_slice(tail);
        return Some((key, value_inlines, String::new(), value_span));
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

fn upsert_ordered_string_pair(entries: &mut Vec<(String, String)>, key: String, value: String) {
    if let Some(existing) = entries.iter_mut().find(|(existing, _)| existing == &key) {
        existing.1 = value;
    } else {
        entries.push((key, value));
    }
}

fn upsert_ordered_author(entries: &mut Vec<IrDocumentAuthor>, author: IrDocumentAuthor) {
    if let Some(existing) = entries
        .iter_mut()
        .find(|existing| existing.name == author.name)
    {
        *existing = author;
    } else {
        entries.push(author);
    }
}

fn upsert_ordered_pair(entries: &mut Vec<IrPair>, pair: IrPair) {
    let key = match pair.first.as_ref() {
        IrValue::String(key) => key,
        _ => {
            entries.push(pair);
            return;
        }
    };
    if let Some(existing) = entries.iter_mut().find(|existing| {
        matches!(existing.first.as_ref(), IrValue::String(existing_key) if existing_key == key)
    }) {
        *existing = pair;
    } else {
        entries.push(pair);
    }
}

/// Applies the bounded scalar String boundary to author information without
/// widening it to rich content, components, callables, ranges, or collections.
fn bounded_document_author_string(
    value: &IrValue,
) -> Result<String, value_conversion::ConversionError> {
    if matches!(value, IrValue::Content(_)) {
        return builtins::plain_text_argument(value).ok_or(
            value_conversion::ConversionError::UnsupportedValue {
                target: value_conversion::ConversionTarget::String,
            },
        );
    }
    if !matches!(
        value,
        IrValue::String(_)
            | IrValue::Identifier(_)
            | IrValue::Number(_)
            | IrValue::Boolean(_)
            | IrValue::Content(_)
    ) {
        return Err(value_conversion::ConversionError::UnsupportedValue {
            target: value_conversion::ConversionTarget::String,
        });
    }
    builtins::scalar_string_conversion(&InvocationValue::static_value(value.clone()))
}

/// Applies only the scalar-to-string families already evidenced by the
/// evaluator. Ranges, collections, rich content, and other semantic values
/// remain unsupported for the bounded `.dockeywords` adapter.
fn bounded_document_keyword_string(
    value: &IrValue,
) -> Result<String, value_conversion::ConversionError> {
    if !matches!(
        value,
        IrValue::String(_) | IrValue::Identifier(_) | IrValue::Number(_) | IrValue::Boolean(_)
    ) {
        return Err(value_conversion::ConversionError::UnsupportedValue {
            target: value_conversion::ConversionTarget::String,
        });
    }
    builtins::scalar_string_conversion(&InvocationValue::static_value(value.clone()))
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
            ordered_args,
        }] => IrValue::Content(vec![IrNode::FunctionCall {
            name: name.clone(),
            positional_args: positional_args.clone(),
            named_args: named_args.clone(),
            ordered_args: ordered_args.clone(),
            lambda_parameters: None,
            body: body.as_ref().map(|body| {
                vec![IrNode::Paragraph {
                    content: body.clone(),
                    span: *span,
                }]
            }),
            raw_body: None,
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
            raw_body: None,
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
        | IrInline::Whitespace { span, .. }
        | IrInline::Emphasis { span, .. }
        | IrInline::Strong { span, .. }
        | IrInline::Strikethrough { span, .. }
        | IrInline::DirectiveCall { span, .. }
        | IrInline::ChainedDirectiveCall { span, .. }
        | IrInline::Link { span, .. }
        | IrInline::Image { span, .. }
        | IrInline::Code { span, .. }
        | IrInline::SoftBreak { span }
        | IrInline::HardBreak { span }
        | IrInline::RawHtml { span, .. } => *span,
        IrInline::TargetSpecificContent { content } => content.span,
    }
}

fn body_contains_raw_html(nodes: &[IrNode]) -> bool {
    nodes.iter().any(|node| match node {
        IrNode::RawHtml { .. } => true,
        IrNode::Paragraph { content, .. } | IrNode::Heading { content, .. } => {
            content.iter().any(|inline| {
                matches!(inline, IrInline::RawHtml { .. })
                    || match inline {
                        IrInline::Emphasis { content, .. }
                        | IrInline::Strong { content, .. }
                        | IrInline::Strikethrough { content, .. }
                        | IrInline::Link { content, .. }
                        | IrInline::Image { content, .. } => content
                            .iter()
                            .any(|child| matches!(child, IrInline::RawHtml { .. })),
                        _ => false,
                    }
            })
        }
        IrNode::Blockquote { content, .. } => body_contains_raw_html(content),
        IrNode::UnorderedList { items, .. } | IrNode::OrderedList { items, .. } => {
            items.iter().any(|item| body_contains_raw_html(&item.nodes))
        }
        IrNode::Table { header, rows, .. } => header
            .cells
            .iter()
            .chain(rows.iter().flat_map(|row| row.cells.iter()))
            .any(|cell| {
                cell.content
                    .iter()
                    .any(|inline| matches!(inline, IrInline::RawHtml { .. }))
            }),
        _ => false,
    })
}

fn opaque_html_body_string(nodes: &[IrNode]) -> Option<String> {
    let mut output = String::new();
    for node in nodes {
        match node {
            IrNode::RawHtml { source, .. } => output.push_str(source),
            IrNode::Paragraph { content, .. } | IrNode::Heading { content, .. } => {
                for inline in content {
                    append_opaque_html_inline(inline, &mut output)?;
                }
            }
            _ => return None,
        }
    }
    Some(output)
}

fn append_opaque_html_inline(inline: &IrInline, output: &mut String) -> Option<()> {
    match inline {
        IrInline::Text { content, .. } | IrInline::RawHtml { content, .. } => {
            output.push_str(content);
        }
        IrInline::SoftBreak { .. } | IrInline::HardBreak { .. } => output.push('\n'),
        IrInline::Emphasis { .. }
        | IrInline::Strong { .. }
        | IrInline::Strikethrough { .. }
        | IrInline::DirectiveCall { .. }
        | IrInline::ChainedDirectiveCall { .. }
        | IrInline::Link { .. }
        | IrInline::Image { .. }
        | IrInline::Code { .. }
        | IrInline::Whitespace { .. }
        | IrInline::TargetSpecificContent { .. } => return None,
    }
    Some(())
}

/// Returns true for the conditional constructs this evaluator resolves.
fn is_conditional(name: &str) -> bool {
    has_native_owner(name, NativeDispatchOwner::Conditional)
}

/// Returns true for the scoped `.let` semantic form.
fn is_let(name: &str) -> bool {
    has_native_owner(name, NativeDispatchOwner::Let)
}

fn is_foreach(name: &str) -> bool {
    has_native_owner(name, NativeDispatchOwner::Foreach)
}

fn is_repeat(name: &str) -> bool {
    has_native_owner(name, NativeDispatchOwner::Repeat)
}

fn is_optionality_callback(name: &str) -> bool {
    has_native_owner(name, NativeDispatchOwner::OptionalityCallback)
}

fn is_range(name: &str) -> bool {
    has_native_owner(name, NativeDispatchOwner::Range)
}

fn is_pair(name: &str) -> bool {
    has_native_owner(name, NativeDispatchOwner::Pair)
}

fn is_dictionary(name: &str) -> bool {
    has_native_owner(name, NativeDispatchOwner::Dictionary)
}

fn is_collection_access(name: &str) -> bool {
    has_native_owner(name, NativeDispatchOwner::CollectionAccess)
}

fn is_collection_transform(name: &str) -> bool {
    has_native_owner(name, NativeDispatchOwner::CollectionTransform)
}

fn native_binding_parameters(name: &str) -> Option<(Vec<ParameterMetadata<'static>>, BodyPolicy)> {
    const AUTHOR_ALIASES: &[&str] = &["author"];
    const AUTHORS_ALIASES: &[&str] = &["authors"];
    const KEYWORDS_ALIASES: &[&str] = &["keywords"];
    const LOCALE_ALIASES: &[&str] = &["locale"];
    const TYPE_ALIASES: &[&str] = &["type"];
    const VAR_VALUE_ALIASES: &[&str] = &["body"];
    const CALLBACK_ALIASES: &[&str] = &["by"];

    let signature = match name {
        "docname" | "docdescription" => (
            vec![ParameterMetadata::optional("value").named(false)],
            BodyPolicy::BindFinal,
        ),
        "doctype" => (
            vec![ParameterMetadata::optional("value").with_aliases(TYPE_ALIASES)],
            BodyPolicy::BindFinal,
        ),
        "docauthor" => (
            vec![ParameterMetadata::optional("value").with_aliases(AUTHOR_ALIASES)],
            BodyPolicy::BindFinal,
        ),
        "docauthors" => (
            vec![ParameterMetadata::optional("authors").with_aliases(AUTHORS_ALIASES)],
            BodyPolicy::BindFinal,
        ),
        "dockeywords" => (
            vec![ParameterMetadata::optional("keywords").with_aliases(KEYWORDS_ALIASES)],
            BodyPolicy::BindFinal,
        ),
        "doclang" => (
            vec![ParameterMetadata::optional("locale").with_aliases(LOCALE_ALIASES)],
            BodyPolicy::BindFinal,
        ),
        "theme" => (
            vec![
                ParameterMetadata::optional("color"),
                ParameterMetadata::optional("layout"),
            ],
            BodyPolicy::BindFinal,
        ),
        "if" | "ifnot" => (
            vec![
                ParameterMetadata::required("condition"),
                ParameterMetadata::optional("body"),
            ],
            BodyPolicy::AllowSeparate,
        ),
        "extend" => (
            vec![
                ParameterMetadata::required("target").named(false),
                ParameterMetadata::optional("where"),
            ],
            BodyPolicy::AllowSeparate,
        ),
        "let" => (
            vec![ParameterMetadata::required("value").named(false)],
            BodyPolicy::AllowSeparate,
        ),
        "foreach" | "repeat" => (
            vec![
                ParameterMetadata::required("iterable").named(false),
                ParameterMetadata::optional("callback").named(false),
            ],
            BodyPolicy::AllowSeparate,
        ),
        "dictionary" => (Vec::new(), BodyPolicy::AllowSeparate),
        "center" | "landscape" => (Vec::new(), BodyPolicy::AllowSeparate),
        "br" => (Vec::new(), BodyPolicy::Reject),
        "align" => (
            vec![ParameterMetadata::required("alignment")],
            BodyPolicy::AllowSeparate,
        ),
        "container" => (
            vec![
                ParameterMetadata::optional("width"),
                ParameterMetadata::optional("height"),
                ParameterMetadata::optional("fullwidth"),
            ],
            BodyPolicy::AllowSeparate,
        ),
        "row" | "column" => (
            vec![
                ParameterMetadata::defaulted("alignment"),
                ParameterMetadata::defaulted("cross"),
                ParameterMetadata::defaulted("gap"),
            ],
            BodyPolicy::AllowSeparate,
        ),
        "grid" => (
            vec![
                ParameterMetadata::required("columns"),
                ParameterMetadata::defaulted("alignment"),
                ParameterMetadata::defaulted("cross"),
                ParameterMetadata::defaulted("gap"),
                ParameterMetadata::defaulted("vgap"),
                ParameterMetadata::defaulted("hgap"),
            ],
            BodyPolicy::AllowSeparate,
        ),
        "captionposition" => (
            vec![
                ParameterMetadata::optional("default"),
                ParameterMetadata::optional("figures"),
                ParameterMetadata::optional("tables"),
                ParameterMetadata::optional("code"),
            ],
            BodyPolicy::BindFinal,
        ),
        "html" | "markdown" => (
            vec![ParameterMetadata::required("content")],
            BodyPolicy::BindFinal,
        ),
        "read" => (
            vec![
                ParameterMetadata::required("path"),
                ParameterMetadata::optional("lines"),
            ],
            BodyPolicy::Reject,
        ),
        "json" => (
            vec![ParameterMetadata::required("path").named(false)],
            BodyPolicy::Reject,
        ),
        "include" => (
            vec![
                ParameterMetadata::required("path"),
                ParameterMetadata::optional("sandbox"),
            ],
            BodyPolicy::Reject,
        ),
        "var" => (
            vec![
                ParameterMetadata::required("name"),
                ParameterMetadata::optional("value").with_aliases(VAR_VALUE_ALIASES),
            ],
            BodyPolicy::BindFinal,
        ),
        "pair" => (
            vec![
                ParameterMetadata::required("first").named(false),
                ParameterMetadata::required("second").named(false),
            ],
            BodyPolicy::Reject,
        ),
        "range" => (
            vec![
                ParameterMetadata::optional("from"),
                ParameterMetadata::optional("to"),
            ],
            BodyPolicy::Reject,
        ),
        "whitespace" => (
            vec![
                ParameterMetadata::optional("width"),
                ParameterMetadata::optional("height"),
            ],
            BodyPolicy::Reject,
        ),
        "map" | "filter" | "sorted" => (
            vec![
                ParameterMetadata::required("from"),
                ParameterMetadata::optional("callback").with_aliases(CALLBACK_ALIASES),
            ],
            BodyPolicy::BindFinal,
        ),
        "ifpresent" => (
            vec![
                ParameterMetadata::required("value"),
                ParameterMetadata::optional("mapping"),
            ],
            BodyPolicy::BindFinal,
        ),
        "takeif" => (
            vec![
                ParameterMetadata::required("value"),
                ParameterMetadata::optional("condition"),
            ],
            BodyPolicy::BindFinal,
        ),
        "size" => (vec![ParameterMetadata::required("of")], BodyPolicy::Reject),
        "first" | "second" | "third" | "last" | "sumall" | "average" | "distinct" | "reversed"
        | "groupvalues" => (
            vec![ParameterMetadata::required("from")],
            BodyPolicy::Reject,
        ),
        "getat" => (
            vec![
                ParameterMetadata::required("from"),
                ParameterMetadata::required("index"),
                ParameterMetadata::optional("orelse"),
            ],
            BodyPolicy::Reject,
        ),
        _ => return None,
    };
    let (parameters, body_policy) = signature;
    Some((parameters, body_policy))
}

fn lambda_body_span(name: &str, parameters: Option<&[IrParameter]>) -> Option<SourceSpan> {
    (name == "br")
        .then(|| {
            parameters.and_then(|parameters| parameters.first().map(|parameter| parameter.span))
        })
        .flatten()
}

fn transform_operands(
    _name: &str,
    positional_args: &[IrValue],
    named_args: &[IrNamedArg],
    has_body: bool,
    binding_plan: Option<&BindingPlan>,
    span: SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<(IrValue, Option<IrValue>), CallOutcome> {
    let candidates = raw_invocation_candidates(positional_args, named_args, span);
    let Some(binding_plan) = binding_plan else {
        return Err(CallOutcome::Failed);
    };
    let body = has_body.then_some(Candidate::Positional {
        value: IrValue::None,
        span,
    });
    let bound = binding_plan
        .bind(&candidates, body.as_ref(), span)
        .map_err(|error| {
            diagnostics.push(binding_diagnostic_with_code(error, "E3001"));
            CallOutcome::Failed
        })?;
    let mut slots = bound.slots.into_iter();
    let Some(BoundSlot::Explicit {
        value: collection, ..
    }) = slots.next()
    else {
        return Err(CallOutcome::Failed);
    };
    let callback = match slots.next() {
        Some(BoundSlot::Explicit { value, .. }) => Some(value),
        Some(BoundSlot::Omitted | BoundSlot::Defaulted) | None => None,
    };
    Ok((collection, callback))
}

fn optionality_operands(
    _name: &str,
    positional_args: &[IrValue],
    named_args: &[IrNamedArg],
    has_body: bool,
    binding_plan: Option<&BindingPlan>,
    span: SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<(IrValue, Option<IrValue>), CallOutcome> {
    let Some(binding_plan) = binding_plan else {
        return Err(CallOutcome::Failed);
    };
    let candidates = raw_invocation_candidates(positional_args, named_args, span);
    let body = has_body.then_some(Candidate::Positional {
        value: IrValue::None,
        span,
    });
    let bound = binding_plan
        .bind(&candidates, body.as_ref(), span)
        .map_err(|error| {
            diagnostics.push(binding_diagnostic_with_code(error, "E3003"));
            CallOutcome::Failed
        })?;
    let mut slots = bound.slots.into_iter();
    let Some(BoundSlot::Explicit { value, .. }) = slots.next() else {
        return Err(CallOutcome::Failed);
    };
    let callback = match slots.next() {
        Some(BoundSlot::Explicit { .. }) if has_body => None,
        Some(BoundSlot::Explicit { value, .. }) => Some(value),
        Some(BoundSlot::Omitted | BoundSlot::Defaulted) | None => None,
    };
    Ok((value, callback))
}

fn collection_access_operand(
    name: &str,
    _named_parameter: &str,
    positional_args: &[InvocationValue],
    named_args: &[InvocationNamedArg],
    binding_plan: &BindingPlan,
    span: &SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<InvocationValue, CallOutcome> {
    let candidates = invocation_candidates(
        positional_args
            .iter()
            .map(|argument| (argument.clone(), value_source_span(&argument.value, span)))
            .collect(),
        named_args.to_vec(),
    );
    let bound = binding_plan
        .bind(&candidates, None, *span)
        .map_err(|error| {
            let message = if let Some(argument_name) =
                error.message.strip_prefix("unknown named argument ")
            {
                format!(
                    "Unknown named argument `{}` for `.{name}`",
                    argument_name.trim_matches('`')
                )
            } else if error.message == "received too many positional arguments" {
                format!(
                    "`.{name}` requires exactly one iterable argument (received {})",
                    positional_args.len()
                )
            } else if error.message.starts_with("parameter ") {
                format!("`.{name}` received iterable argument more than once")
            } else if error.message.starts_with("missing required") {
                format!("`.{name}` requires exactly one iterable argument")
            } else {
                error.message.clone()
            };
            let mut diagnostic = binding_diagnostic_with_code(error, "E3001");
            diagnostic.message = message;
            diagnostics.push(diagnostic);
            CallOutcome::Failed
        })?;
    match bound.slots.into_iter().next() {
        Some(BoundSlot::Explicit { value, .. }) => Ok(value),
        _ => Err(CallOutcome::Failed),
    }
}

fn range_arguments(
    positional_args: &[IrValue],
    named_args: &[IrNamedArg],
    binding_plan: &BindingPlan,
    span: &SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<(Option<IrValue>, Option<IrValue>), CallOutcome> {
    let candidates = raw_invocation_candidates(positional_args, named_args, *span);
    let bound = binding_plan
        .bind(&candidates, None, *span)
        .map_err(|error| {
            let message = if let Some(argument_name) =
                error.message.strip_prefix("unknown named argument ")
            {
                format!("Unknown named argument `{argument_name}` for `.range`")
            } else if error.message == "received too many positional arguments" {
                format!(
                    "`.range` accepts at most two positional bounds (received {})",
                    positional_args.len()
                )
            } else if let Some(parameter) =
                error
                    .message
                    .strip_prefix("parameter ")
                    .and_then(|message| {
                        message.strip_suffix(" collides with an already bound argument")
                    })
            {
                format!("`.range` received {parameter} more than once")
            } else {
                error.message.clone()
            };
            let mut diagnostic = binding_diagnostic_with_code(error, "E3001");
            diagnostic.message = message;
            diagnostics.push(diagnostic);
            CallOutcome::Failed
        })?;
    let mut slots = bound.slots.into_iter();
    let take = |slot: Option<BoundSlot<IrValue>>| match slot {
        Some(BoundSlot::Explicit { value, .. }) => Some(value),
        Some(BoundSlot::Omitted | BoundSlot::Defaulted) | None => None,
    };
    Ok((take(slots.next()), take(slots.next())))
}

fn getat_operands(
    positional_args: &[InvocationValue],
    named_args: &[InvocationNamedArg],
    binding_plan: &BindingPlan,
    span: &SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<(InvocationValue, InvocationValue, IrValue), CallOutcome> {
    let candidates = invocation_candidates(
        positional_args
            .iter()
            .map(|argument| (argument.clone(), value_source_span(&argument.value, span)))
            .collect(),
        named_args.to_vec(),
    );
    let bound = binding_plan
        .bind(&candidates, None, *span)
        .map_err(|error| {
            let message = if let Some(argument_name) =
                error.message.strip_prefix("unknown named argument ")
            {
                format!("Unknown named argument `{argument_name}` for `.getat`")
            } else if error.message == "received too many positional arguments" {
                format!(
                    "`.getat` accepts an iterable and an index (received {} positional arguments)",
                    positional_args.len()
                )
            } else if error
                .message
                .starts_with("missing required argument `from`")
            {
                "`.getat` requires an iterable argument".to_string()
            } else if error
                .message
                .starts_with("missing required argument `index`")
            {
                "`.getat` requires an integer index".to_string()
            } else if let Some(parameter) =
                error
                    .message
                    .strip_prefix("parameter ")
                    .and_then(|message| {
                        message.strip_suffix(" collides with an already bound argument")
                    })
            {
                format!("`.getat` received the {parameter} argument more than once")
            } else {
                error.message.clone()
            };
            let mut diagnostic = binding_diagnostic_with_code(error, "E3001");
            diagnostic.message = message;
            diagnostics.push(diagnostic);
            CallOutcome::Failed
        })?;
    let mut slots = bound.slots.into_iter();
    let Some(BoundSlot::Explicit {
        value: collection, ..
    }) = slots.next()
    else {
        return Err(CallOutcome::Failed);
    };
    let Some(BoundSlot::Explicit { value: index, .. }) = slots.next() else {
        return Err(CallOutcome::Failed);
    };
    let fallback = match slots.next() {
        Some(BoundSlot::Explicit { value, .. }) => value.value,
        Some(BoundSlot::Omitted | BoundSlot::Defaulted) | None => IrValue::None,
    };
    Ok((collection, index, fallback))
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

/// Applies Quarkdown's `Value.asDouble()` conversion at the evaluator value
/// boundary. Non-numeric values become zero; String values are parsed when
/// possible, while Boolean, None, structured values, and callables stringify
/// to non-numeric values in the upstream implementation and therefore also
/// become zero.
fn collection_value_as_double(value: &IrValue) -> f64 {
    match value {
        IrValue::Number(value) => *value,
        IrValue::String(value) | IrValue::Identifier(value) => {
            value.trim().parse::<f64>().ok().unwrap_or(0.0)
        }
        _ => 0.0,
    }
}

fn collection_sum_all(elements: &[IrValue]) -> f64 {
    elements
        .iter()
        .fold(0.0, |sum, value| sum + collection_value_as_double(value))
}

fn collection_average(
    elements: &[IrValue],
    span: &SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<f64, CallOutcome> {
    let length = exact_collection_length(elements.len(), span, diagnostics)?;
    if elements.is_empty() {
        return Ok(f64::NAN);
    }
    Ok(collection_sum_all(elements) / length)
}

/// Value equality used by `.distinct` and `.groupvalues`.
///
/// This is deliberately linear and typed. It does not derive an ordering or
/// hash from debug output, and source spans are ignored for semantic values
/// whose upstream wrappers compare their contained values. Content keeps its
/// structural IR equality, which retains the source-backed identity of rich
/// nodes while allowing plain Markdown list text to be represented as String.
fn collection_values_equal(left: &IrValue, right: &IrValue) -> bool {
    match (left, right) {
        (IrValue::String(left), IrValue::String(right))
        | (IrValue::Identifier(left), IrValue::Identifier(right)) => left == right,
        (IrValue::Number(left), IrValue::Number(right)) => {
            (left.is_nan() && right.is_nan()) || left.total_cmp(right) == Ordering::Equal
        }
        (IrValue::Boolean(left), IrValue::Boolean(right)) => left == right,
        (IrValue::None, IrValue::None) => true,
        (IrValue::Range(left), IrValue::Range(right)) => {
            left.start == right.start && left.end == right.end
        }
        (IrValue::Collection(left), IrValue::Collection(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(left, right)| collection_values_equal(left, right))
        }
        (IrValue::Pair(left), IrValue::Pair(right)) => {
            collection_values_equal(&left.first, &right.first)
                && collection_values_equal(&left.second, &right.second)
        }
        (IrValue::Dictionary(left), IrValue::Dictionary(right)) => {
            left.entries.len() == right.entries.len()
                && left.entries.iter().all(|left_entry| {
                    right.entries.iter().any(|right_entry| {
                        collection_values_equal(&left_entry.first, &right_entry.first)
                            && collection_values_equal(&left_entry.second, &right_entry.second)
                    })
                })
        }
        (IrValue::Content(left), IrValue::Content(right)) => left == right,
        (IrValue::Callable(left), IrValue::Callable(right)) => left == right,
        (IrValue::InlineBody(left), IrValue::InlineBody(right)) => left == right,
        _ => false,
    }
}

fn distinct_collection_values(
    elements: Vec<IrValue>,
    span: SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> CallOutcome {
    let mut distinct = Vec::new();
    if let Err(error) = distinct.try_reserve_exact(elements.len()) {
        diagnostics.push(iteration_error(
            format!("distinct collection cannot be allocated: {error}"),
            span,
        ));
        return CallOutcome::Failed;
    }
    for element in elements {
        if !distinct
            .iter()
            .any(|existing| collection_values_equal(existing, &element))
        {
            distinct.push(element);
        }
    }
    CallOutcome::Value(IrValue::Collection(distinct))
}

fn group_collection_values(
    elements: Vec<IrValue>,
    span: SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> CallOutcome {
    let mut groups: Vec<Vec<IrValue>> = Vec::new();
    if let Err(error) = groups.try_reserve_exact(elements.len()) {
        diagnostics.push(iteration_error(
            format!("grouped collection cannot be allocated: {error}"),
            span,
        ));
        return CallOutcome::Failed;
    }

    for element in elements {
        let group_index = groups.iter().position(|group| {
            group
                .first()
                .is_some_and(|first| collection_values_equal(first, &element))
        });
        match group_index {
            Some(index) => {
                if let Err(error) = groups[index].try_reserve(1) {
                    diagnostics.push(iteration_error(
                        format!("grouped collection cannot be allocated: {error}"),
                        span,
                    ));
                    return CallOutcome::Failed;
                }
                groups[index].push(element);
            }
            None => {
                let mut group = Vec::new();
                if let Err(error) = group.try_reserve_exact(1) {
                    diagnostics.push(iteration_error(
                        format!("grouped collection cannot be allocated: {error}"),
                        span,
                    ));
                    return CallOutcome::Failed;
                }
                group.push(element);
                groups.push(group);
            }
        }
    }

    let mut grouped = Vec::new();
    if let Err(error) = grouped.try_reserve_exact(groups.len()) {
        diagnostics.push(iteration_error(
            format!("grouped collection result cannot be allocated: {error}"),
            span,
        ));
        return CallOutcome::Failed;
    }
    grouped.extend(groups.into_iter().map(IrValue::Collection));
    CallOutcome::Value(IrValue::Collection(grouped))
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

fn bind_invocation_arguments(
    parameters: Option<&[IrParameter]>,
    arguments: Vec<IrValue>,
    allow_destructuring: bool,
    span: SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<BoundLambdaArguments, CallOutcome> {
    let Some(parameters) = parameters else {
        return Ok(BoundLambdaArguments::Implicit(arguments));
    };

    if allow_destructuring && parameters.len() > 1 && arguments.len() == 1 {
        let bindings =
            scoped_parameter_bindings(&arguments[0], parameters, true, span, diagnostics)?;
        return Ok(BoundLambdaArguments::Explicit(
            bindings.into_iter().map(|(_, value)| value).collect(),
        ));
    }

    let metadata = parameters
        .iter()
        .map(|parameter| {
            let metadata = if parameter.optional {
                ParameterMetadata::optional(&parameter.name)
            } else {
                ParameterMetadata::required(&parameter.name)
            };
            metadata.with_name_span(parameter.name_span)
        })
        .collect::<Vec<_>>();
    let candidates = arguments
        .iter()
        .map(|value| Candidate::Positional {
            value: value.clone(),
            span: value_source_span(value, &span),
        })
        .collect::<Vec<_>>();
    let bound =
        match invocation_binder::bind(&metadata, &candidates, None, BodyPolicy::Reject, span) {
            Ok(bound) => bound,
            Err(error) => {
                diagnostics.push(binding_diagnostic_with_message(
                    error,
                    callable_binding_message,
                ));
                return Err(CallOutcome::Failed);
            }
        };
    Ok(BoundLambdaArguments::Explicit(
        bound
            .slots
            .into_iter()
            .map(|slot| match slot {
                BoundSlot::Explicit { value, .. } => value,
                BoundSlot::Omitted | BoundSlot::Defaulted => IrValue::None,
            })
            .collect(),
    ))
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

fn repeat_count(value: &IrValue) -> Result<i32, String> {
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
        .parse::<i64>()
        .ok()
        .and_then(|count| i32::try_from(count).ok())
        .ok_or_else(|| "`.repeat` count is outside the supported integer range".to_string())
}

/// Converts an evaluator Number to the endpoint type used by Quarkdown's
/// `Number.toInt()` call. Kotlin truncates toward zero, maps NaN to zero, and
/// clamps finite or infinite values outside Int's domain to the nearest Int
/// boundary. The explicit comparisons avoid relying on Rust's float-to-int
/// cast behavior as language semantics.
fn number_to_range_endpoint(
    value: &InvocationValue,
) -> Result<i32, value_conversion::ConversionError> {
    let number = match value_conversion::convert_scalar_with_origin(value, ScalarTarget::Number)? {
        ScalarValue::Number(number) => number,
        ScalarValue::Boolean(_) | ScalarValue::String(_) => {
            return Err(value_conversion::ConversionError::UnsupportedValue {
                target: value_conversion::ConversionTarget::Number,
            });
        }
    };
    if number.is_nan() {
        return Ok(0);
    }
    if number <= f64::from(i32::MIN) {
        return Ok(i32::MIN);
    }
    if number >= f64::from(i32::MAX) {
        return Ok(i32::MAX);
    }
    Ok(number.trunc() as i32)
}

/// Parses the numeric part of a parser-preserved implicit parameter call.
///
/// The frontend preserves the numeric spelling; this evaluator policy accepts
/// only 1-based indices without a leading zero. This checked conversion keeps
/// oversized decimal indices deterministic instead of allowing an integer
/// conversion panic.
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

fn resource_diagnostic(
    code: &str,
    message: impl Into<String>,
    span: SourceSpan,
    hint: impl Into<String>,
) -> Diagnostic {
    Diagnostic {
        code: code.to_string(),
        severity: Severity::Error,
        message: message.into(),
        primary: Some(span),
        secondary: Vec::new(),
        hints: vec![hint.into()],
    }
}

fn resource_access_diagnostic(
    builtin: &str,
    error: ResourceAccessError,
    span: SourceSpan,
) -> Diagnostic {
    match error {
        ResourceAccessError::UnsupportedReference { reference } => resource_diagnostic(
            "E8001",
            format!("`.{builtin}` does not support non-local resource reference `{reference}`"),
            span,
            "Only source-relative paths inside the supplied VirtualProject are available; network fetching is disabled.",
        ),
        ResourceAccessError::UnknownSource { source_id } => resource_diagnostic(
            "E9001",
            format!("`.{builtin}` cannot resolve the current source identity {source_id:?}"),
            span,
            "The host must provide the calling source through the VirtualProject SourceStore.",
        ),
        ResourceAccessError::Boundary { message } => resource_diagnostic(
            "E8001",
            format!("`.{builtin}` resource path is outside the project boundary: {message}"),
            span,
            "Use a source-relative path that remains inside the supplied VirtualProject.",
        ),
        ResourceAccessError::NotFound { path } => resource_diagnostic(
            "E3001",
            format!("`.{builtin}` resource not found: `{path}`"),
            span,
            "Add the logical resource to the VirtualProject supplied by the host.",
        ),
        ResourceAccessError::InvalidUtf8 { path, message } => resource_diagnostic(
            "E3001",
            format!("`.{builtin}` resource `{path}` is not valid UTF-8: {message}"),
            span,
            "Text resource builtins require valid UTF-8 and do not perform lossy decoding.",
        ),
    }
}

fn resource_context<'a>(
    context: &'a EvaluationContext<'_>,
    span: &SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<(&'a dyn ResourceProvider, SourceId)> {
    let Some(provider) = context.resources else {
        diagnostics.push(resource_diagnostic(
            "E8001",
            "Resource builtin requires a host-supplied VirtualProject".to_string(),
            *span,
            "Compile through the project API so logical resources are supplied explicitly.",
        ));
        return None;
    };
    let Some(source_id) = context.current_source else {
        diagnostics.push(resource_diagnostic(
            "E9001",
            "Resource builtin has no current source identity".to_string(),
            *span,
            "The evaluator must retain the logical source identity of the current document.",
        ));
        return None;
    };
    Some((provider, source_id))
}

fn resource_path_argument(
    builtin: &str,
    positional_args: &[IrValue],
    named_args: &[IrNamedArg],
    binding_plan: &BindingPlan,
    span: &SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<String> {
    let candidates = raw_invocation_candidates(positional_args, named_args, *span);
    let bound = match binding_plan.bind(&candidates, None, *span) {
        Ok(bound) => bound,
        Err(error) => {
            diagnostics.push(resource_diagnostic(
                "E3003",
                error.message,
                error.primary,
                "Pass one source-relative logical resource path and only the builtin's documented optional arguments.",
            ));
            return None;
        }
    };
    let Some(BoundSlot::Explicit {
        value,
        span: value_span,
    }) = bound.slots.into_iter().next()
    else {
        return None;
    };
    let Some(path) = builtins::adapt_string_argument(&value) else {
        diagnostics.push(resource_diagnostic(
            "E3003",
            format!("`.{builtin}` resource path must adapt to String"),
            value_span,
            "Use a scalar or plain-text path value.",
        ));
        return None;
    };
    Some(path)
}

fn resource_lines_argument(
    named_args: &[IrNamedArg],
    span: &SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<Option<IrRange>, ()> {
    let Some(argument) = named_args.iter().find(|argument| argument.name == "lines") else {
        let _ = span;
        return Ok(None);
    };
    let IrValue::Range(range) = &argument.value else {
        diagnostics.push(resource_diagnostic(
            "E3003",
            "`.read` named argument `lines` must be a typed Range".to_string(),
            argument.span,
            "Use a one-based inclusive range such as `1..3`.",
        ));
        return Err(());
    };
    let _ = span;
    Ok(Some(range.clone()))
}

#[derive(Debug, Clone, Copy)]
enum IncludeSandbox {
    Share,
    Scope,
    Subdocument,
}

fn include_sandbox_argument(
    named_args: &[IrNamedArg],
    span: &SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<IncludeSandbox, ()> {
    let sandbox = named_args
        .iter()
        .find(|argument| argument.name == "sandbox")
        .map(|argument| {
            let Some(value) = builtins::adapt_string_argument(&argument.value) else {
                diagnostics.push(resource_diagnostic(
                    "E3003",
                    "`.include` `sandbox` must be a String".to_string(),
                    argument.span,
                    "Use `share`, `scope`, or `subdocument`.",
                ));
                return Err(());
            };
            match value.to_ascii_lowercase().as_str() {
                "share" => Ok(IncludeSandbox::Share),
                "scope" => Ok(IncludeSandbox::Scope),
                "subdocument" => Ok(IncludeSandbox::Subdocument),
                _ => {
                    diagnostics.push(resource_diagnostic(
                        "E3003",
                        format!("unsupported `.include` sandbox `{value}`"),
                        argument.span,
                        "Use `share`, `scope`, or `subdocument`.",
                    ));
                    Err(())
                }
            }
        })
        .transpose()?;
    let _ = span;
    Ok(sandbox.unwrap_or(IncludeSandbox::Share))
}

fn normalize_line_separators(text: &str) -> String {
    let mut normalized = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(character) = chars.next() {
        if character == '\r' {
            if chars.as_str().starts_with('\n') {
                let _ = chars.next();
            }
            normalized.push('\n');
        } else {
            normalized.push(character);
        }
    }
    normalized
}

fn select_lines(text: &str, range: IrRange) -> Result<String, String> {
    let start = range.start.unwrap_or(1);
    let normalized = normalize_line_separators(text);
    let lines = normalized.lines().collect::<Vec<_>>();
    let end = range.end.unwrap_or(lines.len() as i32);
    if start < 1 || end < start || end as usize > lines.len() {
        return Err(format!(
            "range {start}..{end} is outside 1..{}",
            lines.len()
        ));
    }
    Ok(lines[(start as usize - 1)..end as usize].join("\n"))
}

fn json_value_to_ir(value: &serde_json::Value, span: SourceSpan) -> Result<IrValue, String> {
    match value {
        serde_json::Value::Null => Ok(IrValue::None),
        serde_json::Value::Bool(value) => Ok(IrValue::Boolean(*value)),
        serde_json::Value::String(value) => Ok(IrValue::String(value.clone())),
        serde_json::Value::Number(value) => json_number_to_ir(value),
        serde_json::Value::Array(values) => values
            .iter()
            .map(|value| json_value_to_ir(value, span))
            .collect::<Result<Vec<_>, _>>()
            .map(IrValue::Collection),
        serde_json::Value::Object(entries) => entries
            .iter()
            .map(|(key, value)| {
                Ok(IrPair {
                    first: Box::new(IrValue::String(key.clone())),
                    second: Box::new(json_value_to_ir(value, span)?),
                    span,
                })
            })
            .collect::<Result<Vec<_>, String>>()
            .map(|entries| IrValue::Dictionary(IrDictionary { entries, span })),
    }
}

fn json_number_to_ir(value: &serde_json::Number) -> Result<IrValue, String> {
    const MAX_EXACT_F64_INTEGER: u64 = 9_007_199_254_740_991;
    if let Some(value) = value.as_i64() {
        if value.unsigned_abs() > MAX_EXACT_F64_INTEGER {
            return Err(format!(
                "integer {value} cannot be represented exactly by evaluator Number"
            ));
        }
        return Ok(IrValue::Number(value as f64));
    }
    if let Some(value) = value.as_u64() {
        if value > MAX_EXACT_F64_INTEGER {
            return Err(format!(
                "integer {value} cannot be represented exactly by evaluator Number"
            ));
        }
        return Ok(IrValue::Number(value as f64));
    }
    let value = value
        .as_f64()
        .ok_or_else(|| "JSON number cannot be represented by evaluator Number".to_string())?;
    if !value.is_finite() {
        return Err("JSON number is not finite".to_string());
    }
    Ok(IrValue::Number(value))
}

fn source_mode_for_resource_path(path: &str) -> Mode {
    let file_name = path.rsplit('/').next().unwrap_or(path);
    let is_markdown = file_name
        .rsplit_once('.')
        .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("md"));
    if is_markdown {
        Mode::Markdown
    } else {
        Mode::Quarkdown
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

fn extension_error(
    message: String,
    primary: SourceSpan,
    secondary: Option<SourceSpan>,
) -> Diagnostic {
    Diagnostic {
        code: "E3001".to_string(),
        severity: Severity::Error,
        message,
        primary: Some(primary),
        secondary: secondary.into_iter().collect(),
        hints: vec![
            "Extension targets, parameters, and `.super` calls must resolve within the current callable scope."
                .to_string(),
        ],
    }
}

fn extension_parameter_error(
    message: String,
    primary: SourceSpan,
    target_span: SourceSpan,
) -> Diagnostic {
    extension_error(message, primary, Some(target_span))
}

fn binding_diagnostic(error: invocation_binder::BindingError) -> Diagnostic {
    binding_diagnostic_with_code(error, "E3003")
}

fn binding_diagnostic_with_message(
    mut error: invocation_binder::BindingError,
    message: fn(String, &str) -> String,
) -> Diagnostic {
    error.message = message(error.message, &error.hint);
    binding_diagnostic(error)
}

fn callable_binding_message(message: String, hint: &str) -> String {
    if hint == "Remove the final explicit value when using a body fallback." {
        return "A block argument collides with the function's final parameter binding".to_string();
    }
    if let Some(parameter) = message.strip_prefix("missing required argument ") {
        return format!("Missing required argument {parameter}");
    }
    if message == "received too many positional arguments" {
        return "Function call has too many positional arguments".to_string();
    }
    if let Some(name) = message.strip_prefix("unknown named argument ") {
        return format!("Unknown named parameter {name}");
    }
    if let Some(parameter) = message
        .strip_prefix("parameter ")
        .and_then(|message| message.strip_suffix(" collides with an already bound argument"))
    {
        return format!("Parameter {parameter} was bound more than once");
    }
    message
}

fn native_binding_message(name: &str, message: String) -> String {
    if message == "missing required argument `content`" {
        return if name == "html" {
            "`.html` requires one `content` argument or body".to_string()
        } else if name == "markdown" {
            "`.markdown` requires Markdown content".to_string()
        } else {
            message
        };
    }
    if let Some(parameter) = message
        .strip_prefix("parameter ")
        .and_then(|message| message.strip_suffix(" collides with an already bound argument"))
    {
        if matches!(name, "html" | "markdown") && parameter == "`content`" {
            return format!("`.{name}` received `content` more than once");
        }
        if name == "container" {
            return format!("Argument {parameter} was bound more than once");
        }
        return format!("`.{name}` received argument {parameter} more than once");
    }
    if let Some(argument) = message
        .strip_prefix("named argument `")
        .and_then(|message| message.strip_suffix("` was supplied more than once"))
    {
        if name == "container" {
            return format!("Argument `{argument}` was bound more than once");
        }
        return format!("`.{name}` received the `{argument}` argument more than once");
    }
    if let Some(argument) = message.strip_prefix("unknown named argument ") {
        if name == "container"
            && matches!(
                argument.trim_matches('`'),
                "float"
                    | "fullspan"
                    | "classname"
                    | "foreground"
                    | "background"
                    | "border"
                    | "borderwidth"
                    | "borderstyle"
                    | "alignment"
                    | "textalignment"
                    | "margin"
                    | "padding"
                    | "radius"
                    | "fontsize"
                    | "fontweight"
                    | "fontstyle"
                    | "fontvariant"
                    | "textdecoration"
                    | "textcase"
            )
        {
            return format!(
                "`.container` parameter {argument} is not supported by the bounded container sizing slice"
            );
        }
        if name == "container" {
            return format!("Unknown named argument {argument}");
        }
        return format!("`.{name}` does not support named argument {argument}");
    }
    if message == "received too many positional arguments" {
        if name == "html" {
            return "`.html` accepts exactly one `content` argument".to_string();
        }
        if name == "markdown" {
            return "`.markdown` accepts exactly one `content` argument".to_string();
        }
        if name == "container" {
            return "`.container` accepts at most three positional arguments".to_string();
        }
        return format!("`.{name}` received too many positional arguments");
    }
    if message == "a body requires a final parameter" {
        return format!("`.{name}` does not have a final body parameter");
    }
    message
}

fn binding_diagnostic_with_code(error: invocation_binder::BindingError, code: &str) -> Diagnostic {
    Diagnostic {
        code: code.to_string(),
        severity: Severity::Error,
        message: error.message,
        primary: Some(error.primary),
        secondary: error.secondary,
        hints: vec![error.hint],
    }
}

fn structural_candidates(
    ordered: Option<&[IrCallArgument]>,
    positional: &[IrValue],
    named: &[IrNamedArg],
    fallback_span: SourceSpan,
) -> Vec<Candidate<()>> {
    if let Some(ordered) = ordered {
        return ordered
            .iter()
            .map(|argument| match argument {
                IrCallArgument::Positional { span, .. } => Candidate::Positional {
                    value: (),
                    span: *span,
                },
                IrCallArgument::Named {
                    index,
                    name_span,
                    span,
                } => {
                    let name = named
                        .get(*index)
                        .map_or_else(String::new, |argument| argument.name.clone());
                    Candidate::Named {
                        name,
                        name_span: *name_span,
                        value: (),
                        span: *span,
                    }
                }
            })
            .collect();
    }
    let mut candidates = Vec::with_capacity(positional.len() + named.len());
    candidates.extend(positional.iter().map(|value| Candidate::Positional {
        value: (),
        span: value_source_span(value, &fallback_span),
    }));
    candidates.extend(named.iter().map(|argument| Candidate::Named {
        name: argument.name.clone(),
        name_span: argument.name_span,
        value: (),
        span: argument.span,
    }));
    candidates
}

/// Enforce the universal source-order invariant before target lookup or any
/// candidate/body evaluation. Legacy manually constructed IR without an
/// ordered projection cannot recover mixed-kind source order and therefore
/// keeps its historical grouped representation.
fn validate_ordered_invocation(
    name: &str,
    ordered: Option<&[IrCallArgument]>,
    positional: &[IrValue],
    named: &[IrNamedArg],
    span: SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<(), CallOutcome> {
    let Some(ordered) = ordered else {
        return Ok(());
    };
    let candidates = structural_candidates(Some(ordered), positional, named, span);
    if let Err(mut error) = invocation_binder::validate_order(&candidates) {
        let native_code = native_binding_diagnostic_code(name);
        let is_duplicate_named = error.message.starts_with("named argument ");
        if native_code.is_some() {
            error.message = native_binding_message(name, error.message);
        }
        let code = if is_duplicate_named {
            native_code.map_or("E3003", |code| code)
        } else {
            "E3003"
        };
        diagnostics.push(binding_diagnostic_with_code(error, code));
        return Err(CallOutcome::Failed);
    }
    Ok(())
}

fn native_binding_diagnostic_code(name: &str) -> Option<&'static str> {
    if builtins::lookup(name).is_some() {
        return Some("E3001");
    }
    native_binding_parameters(name)?;
    Some(if name == "var" {
        "E3002"
    } else if is_document_state(name) || matches!(name, "let" | "br" | "html" | "markdown") {
        "E3003"
    } else {
        "E3001"
    })
}

fn offset_ordered_argument(
    argument: &IrCallArgument,
    positional_offset: usize,
) -> Option<IrCallArgument> {
    match argument {
        IrCallArgument::Positional { index, span } => Some(IrCallArgument::Positional {
            index: index.checked_add(positional_offset)?,
            span: *span,
        }),
        IrCallArgument::Named {
            index,
            name_span,
            span,
        } => Some(IrCallArgument::Named {
            index: *index,
            name_span: *name_span,
            span: *span,
        }),
    }
}

#[derive(Clone, Copy)]
enum CandidateKind<'a> {
    Positional(SourceSpan),
    Named(&'a String, SourceSpan, SourceSpan),
}

fn body_candidate_shape(body: CallBody<'_>, span: SourceSpan) -> Candidate<()> {
    Candidate::Positional {
        value: (),
        span: call_body_source_span(body, span),
    }
}

fn call_body_source_span(body: CallBody<'_>, fallback: SourceSpan) -> SourceSpan {
    let (first, last) = match body {
        CallBody::Block(nodes) => (
            nodes.first().map(ir_node_source_span),
            nodes.last().map(ir_node_source_span),
        ),
        CallBody::Inline(inlines) => (
            inlines.first().map(inline_source_span),
            inlines.last().map(inline_source_span),
        ),
    };
    match (first, last) {
        (Some(first), Some(last)) if first.source_id == last.source_id => SourceSpan {
            source_id: first.source_id,
            start: first.start,
            end: last.end,
        },
        _ => fallback,
    }
}

fn invocation_candidates(
    positional: Vec<(InvocationValue, SourceSpan)>,
    named: Vec<InvocationNamedArg>,
) -> Vec<Candidate<InvocationValue>> {
    let mut candidates = Vec::with_capacity(positional.len() + named.len());
    candidates.extend(
        positional
            .into_iter()
            .map(|(value, span)| Candidate::Positional { value, span }),
    );
    candidates.extend(named.into_iter().map(|argument| Candidate::Named {
        name: argument.arg.name,
        name_span: argument.arg.name_span,
        value: InvocationValue {
            value: argument.arg.value,
            origin: argument.origin,
        },
        span: argument.arg.span,
    }));
    candidates
}

fn raw_invocation_candidates(
    positional: &[IrValue],
    named: &[IrNamedArg],
    fallback_span: SourceSpan,
) -> Vec<Candidate<IrValue>> {
    let mut candidates = Vec::with_capacity(positional.len() + named.len());
    candidates.extend(positional.iter().map(|value| Candidate::Positional {
        value: value.clone(),
        span: value_source_span(value, &fallback_span),
    }));
    candidates.extend(named.iter().map(|argument| Candidate::Named {
        name: argument.name.clone(),
        name_span: argument.name_span,
        value: argument.value.clone(),
        span: argument.span,
    }));
    candidates
}

fn candidate_shape<T>(candidate: &Candidate<T>) -> Candidate<()> {
    match candidate {
        Candidate::Positional { span, .. } => Candidate::Positional {
            value: (),
            span: *span,
        },
        Candidate::Named {
            name,
            name_span,
            span,
            ..
        } => Candidate::Named {
            name: name.clone(),
            name_span: *name_span,
            value: (),
            span: *span,
        },
    }
}

fn candidate_shapes<T>(candidates: &[Candidate<T>]) -> Vec<Candidate<()>> {
    candidates.iter().map(candidate_shape).collect()
}

fn owned_call_body(body: CallBody<'_>) -> OwnedCallBody {
    match body {
        CallBody::Block(nodes) => OwnedCallBody::Block(Rc::from(nodes)),
        CallBody::Inline(inlines) => OwnedCallBody::Inline(Rc::from(inlines)),
    }
}

fn owned_body_as_call_body(body: &OwnedCallBody) -> CallBody<'_> {
    match body {
        OwnedCallBody::Block(nodes) => CallBody::Block(nodes),
        OwnedCallBody::Inline(inlines) => CallBody::Inline(inlines),
    }
}

fn owned_body_candidate_shape(body: &OwnedCallBody, span: SourceSpan) -> Candidate<()> {
    body_candidate_shape(owned_body_as_call_body(body), span)
}

fn lambda_parameter_names(parameters: &LambdaParameters) -> BTreeSet<String> {
    match parameters {
        LambdaParameters::Explicit(parameters) => parameters
            .iter()
            .map(|parameter| parameter.name.clone())
            .collect(),
        LambdaParameters::Implicit => BTreeSet::new(),
    }
}

fn extension_wrapper_parameters(
    parameters: &LambdaParameters,
    fallback_span: SourceSpan,
) -> LambdaParameters {
    match parameters {
        LambdaParameters::Explicit(parameters) => LambdaParameters::Explicit(
            parameters
                .iter()
                .map(|parameter| IrParameter {
                    name: parameter.name.clone(),
                    name_span: parameter.name_span,
                    span: parameter.span,
                    optional: true,
                })
                .map(|mut parameter| {
                    if parameter.span.start == 0 && parameter.span.end == 0 {
                        parameter.span = fallback_span;
                        parameter.name_span = fallback_span;
                    }
                    parameter
                })
                .collect(),
        ),
        LambdaParameters::Implicit => LambdaParameters::Implicit,
    }
}

fn lambda_binding_metadata(parameters: &LambdaParameters) -> Vec<ParameterMetadata<'_>> {
    match parameters {
        LambdaParameters::Explicit(parameters) => parameters
            .iter()
            .map(|parameter| ParameterMetadata {
                name: &parameter.name,
                aliases: &[],
                allows_named: true,
                omission: if parameter.optional {
                    invocation_binder::OmissionPolicy::Optional
                } else {
                    invocation_binder::OmissionPolicy::Required
                },
                name_span: Some(parameter.name_span),
            })
            .collect(),
        LambdaParameters::Implicit => Vec::new(),
    }
}

fn target_parameters_from_metadata(
    parameters: Vec<ParameterMetadata<'static>>,
    fallback_span: SourceSpan,
) -> LambdaParameters {
    LambdaParameters::Explicit(
        parameters
            .into_iter()
            .map(|parameter| IrParameter {
                name: parameter.name.to_string(),
                name_span: parameter.name_span.unwrap_or(fallback_span),
                span: parameter.name_span.unwrap_or(fallback_span),
                optional: matches!(
                    parameter.omission,
                    invocation_binder::OmissionPolicy::Optional
                        | invocation_binder::OmissionPolicy::Default
                ),
            })
            .collect(),
    )
}

fn builtin_extension_body_policy(policy: builtins::BuiltinBodyPolicy) -> ExtensionBodyPolicy {
    match policy {
        builtins::BuiltinBodyPolicy::Reject => ExtensionBodyPolicy::Reject,
        builtins::BuiltinBodyPolicy::BindRaw => ExtensionBodyPolicy::BindRaw,
        builtins::BuiltinBodyPolicy::BindEvaluatedContent => {
            ExtensionBodyPolicy::BindEvaluatedContent
        }
    }
}

fn native_extension_body_policy(policy: BodyPolicy) -> ExtensionBodyPolicy {
    match policy {
        BodyPolicy::Reject => ExtensionBodyPolicy::Reject,
        BodyPolicy::BindFinal => ExtensionBodyPolicy::BindEvaluatedContent,
        BodyPolicy::AllowSeparate => ExtensionBodyPolicy::AllowSeparate,
    }
}

fn function_target_contract(
    target: &FunctionTarget,
    fallback_span: SourceSpan,
) -> Option<(LambdaParameters, ExtensionBodyPolicy)> {
    match target {
        FunctionTarget::Binding(binding) => match binding.extension.as_ref() {
            Some(extension) => function_target_contract(&extension.super_target, fallback_span),
            None => Some((
                binding.parameters.clone(),
                ExtensionBodyPolicy::BindEvaluatedContent,
            )),
        },
        FunctionTarget::Native(name) => {
            if let Some(builtin) = builtins::lookup(name) {
                Some((
                    target_parameters_from_metadata(
                        builtins::binding_parameters(builtin),
                        fallback_span,
                    ),
                    builtin_extension_body_policy(builtin.body_policy),
                ))
            } else {
                native_binding_parameters(name).map(|(parameters, policy)| {
                    (
                        target_parameters_from_metadata(parameters, fallback_span),
                        native_extension_body_policy(policy),
                    )
                })
            }
        }
    }
}

fn resolve_function_target(name: &str, context: &EvaluationContext<'_>) -> Option<FunctionTarget> {
    if let Some(binding) = context.get_function(name) {
        return Some(FunctionTarget::Binding(Rc::clone(binding)));
    }
    // This bounded slice can re-dispatch regular scalar builtins through the
    // same binder/conversion boundary as ordinary calls. Bespoke native
    // owners (layout, resource, document-state, and callbacks) remain outside
    // #169 rather than being claimed as extension targets without a matching
    // forced-call path.
    if builtins::lookup(name).is_some() {
        return Some(FunctionTarget::Native(name.to_string()));
    }
    None
}

fn split_extension_target(
    target: FunctionTarget,
    context: &EvaluationContext<'_>,
) -> (FunctionTarget, Option<Rc<FunctionExtension>>) {
    let mut current = target;
    loop {
        let FunctionTarget::Binding(binding) = &current else {
            return (current, None);
        };
        let Some(extension) = binding.extension.as_ref() else {
            return (current, None);
        };
        let next = context.get_extension_target(extension);
        match &next {
            FunctionTarget::Binding(next_binding) if next_binding.extension.is_some() => {
                current = next;
            }
            _ => return (next, Some(Rc::clone(extension))),
        }
    }
}

fn forwarded_extension_candidates(
    bound: &invocation_binder::BoundInvocation<InvocationValue>,
) -> Vec<Candidate<InvocationValue>> {
    bound
        .slots
        .iter()
        .zip(bound.parameters.iter())
        .filter_map(|(slot, parameter)| match slot {
            BoundSlot::Explicit { value, span } => Some(Candidate::Named {
                name: parameter.name.clone(),
                name_span: parameter.name_span.unwrap_or(*span),
                value: value.clone(),
                span: *span,
            }),
            BoundSlot::Omitted | BoundSlot::Defaulted => None,
        })
        .collect()
}

fn merge_extension_candidates(
    outer: &[Candidate<InvocationValue>],
    overrides: &[Candidate<InvocationValue>],
    parameters: &LambdaParameters,
) -> Vec<Candidate<InvocationValue>> {
    if matches!(parameters, LambdaParameters::Implicit) {
        return outer
            .iter()
            .cloned()
            .chain(overrides.iter().cloned())
            .collect();
    }
    let mut merged = BTreeMap::<String, Candidate<InvocationValue>>::new();
    for candidate in outer.iter().chain(overrides) {
        let name = match candidate {
            Candidate::Named { name, .. } => name.clone(),
            Candidate::Positional { .. } => continue,
        };
        merged.insert(name, candidate.clone());
    }
    merged.into_values().collect()
}

fn extension_body_candidate_span(
    policy: ExtensionBodyPolicy,
    body: Option<&OwnedCallBody>,
    call_span: SourceSpan,
) -> SourceSpan {
    if matches!(policy, ExtensionBodyPolicy::BindRaw) {
        // `IrRawBody::span` is local to its source. The regular native
        // BindRaw path uses the containing call span for the generated
        // candidate, so an extension forwarding the same raw body must keep
        // that diagnostic provenance.
        return call_span;
    }
    body.map(|body| call_body_source_span(owned_body_as_call_body(body), call_span))
        .unwrap_or(call_span)
}

#[allow(clippy::too_many_arguments)]
fn extension_body_value(
    evaluator: &Evaluator,
    body: Option<&OwnedCallBody>,
    raw_body: Option<&IrRawBody>,
    policy: ExtensionBodyPolicy,
    span: SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
    context: &mut EvaluationContext<'_>,
) -> Result<Option<InvocationValue>, CallOutcome> {
    let Some(body) = body else {
        return Ok(None);
    };
    match policy {
        ExtensionBodyPolicy::Reject => Ok(None),
        ExtensionBodyPolicy::AllowSeparate => Ok(None),
        ExtensionBodyPolicy::BindEvaluatedContent => {
            match evaluator.evaluate_call_body(
                owned_body_as_call_body(body),
                &span,
                diagnostics,
                context,
            ) {
                CallOutcome::Value(value) => Ok(Some(InvocationValue::static_value(value))),
                CallOutcome::NoValue => Err(CallOutcome::NoValue),
                CallOutcome::Failed => Err(CallOutcome::Failed),
                CallOutcome::Unresolved => Err(CallOutcome::Unresolved),
            }
        }
        ExtensionBodyPolicy::BindRaw => {
            let Some(raw_body) = raw_body else {
                diagnostics.push(chain_evaluation_error(
                    "This body conversion requires source-backed raw body text".to_string(),
                    call_body_source_span(owned_body_as_call_body(body), span),
                ));
                return Err(CallOutcome::Failed);
            };
            let Some(text) = value_conversion::raw_body_dynamic_text(raw_body) else {
                diagnostics.push(chain_evaluation_error(
                    "This body conversion requires a valid source-backed body span".to_string(),
                    call_body_source_span(owned_body_as_call_body(body), span),
                ));
                return Err(CallOutcome::Failed);
            };
            Ok(Some(InvocationValue::dynamic_value(IrValue::String(text))))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InvocationArgumentLocation {
    Positional(usize),
    Named(usize),
    Body,
}

fn raw_invocation_locations(
    positional: &[IrValue],
    named: &[IrNamedArg],
    fallback_span: SourceSpan,
) -> Vec<Candidate<InvocationArgumentLocation>> {
    let mut candidates = Vec::with_capacity(positional.len() + named.len());
    candidates.extend(
        positional
            .iter()
            .enumerate()
            .map(|(index, value)| Candidate::Positional {
                value: InvocationArgumentLocation::Positional(index),
                span: value_source_span(value, &fallback_span),
            }),
    );
    candidates.extend(
        named
            .iter()
            .enumerate()
            .map(|(index, argument)| Candidate::Named {
                name: argument.name.clone(),
                name_span: argument.name_span,
                value: InvocationArgumentLocation::Named(index),
                span: argument.span,
            }),
    );
    candidates
}

fn bind_evaluated_arguments(
    plan: &BindingPlan,
    positional: Vec<(InvocationValue, SourceSpan)>,
    named: Vec<InvocationNamedArg>,
    body: Option<&Candidate<InvocationValue>>,
    call_span: SourceSpan,
) -> Result<invocation_binder::BoundInvocation<InvocationValue>, invocation_binder::BindingError> {
    let candidates = invocation_candidates(positional, named);
    plan.bind(&candidates, body, call_span)
}

fn source_backed_body_candidate(
    body_span: Option<SourceSpan>,
    raw_body: Option<&IrRawBody>,
    target: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<Option<Candidate<InvocationValue>>, CallOutcome> {
    match (body_span, raw_body) {
        (None, _) => Ok(None),
        (Some(span), Some(raw_body)) => {
            let Some(body_text) = value_conversion::raw_body_dynamic_text(raw_body) else {
                diagnostics.push(target_conversion_error_message(
                    target,
                    span,
                    "requires a valid source-backed body span".to_string(),
                ));
                return Err(CallOutcome::Failed);
            };
            Ok(Some(Candidate::Positional {
                value: InvocationValue::dynamic_value(IrValue::String(body_text)),
                // `raw_body.span` is source-local. The containing call span
                // remains the evaluator-side diagnostic provenance.
                span,
            }))
        }
        (Some(span), None) => {
            diagnostics.push(target_conversion_error_message(
                target,
                span,
                "requires a source-backed block body".to_string(),
            ));
            Err(CallOutcome::Failed)
        }
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

fn document_state_conversion_error(message: String, span: SourceSpan) -> Diagnostic {
    Diagnostic {
        code: "E3001".to_string(),
        severity: Severity::Error,
        message,
        primary: Some(span),
        secondary: Vec::new(),
        hints: vec![
            "Document-state mutation is committed only after argument conversion and validation succeed."
                .to_string(),
        ],
    }
}

fn target_conversion_error(
    target: &str,
    span: SourceSpan,
    error: value_conversion::ConversionError,
) -> Diagnostic {
    conversion_failure_diagnostic(
        value_conversion::ConversionFailure::new(error, Some(span), None::<String>, None, span),
        Some(target),
    )
}

/// Emits every typed conversion failure through one provenance and
/// classification policy. The diagnostic code remains E3001; the typed
/// `InvalidText`/`UnsupportedValue` distinction is part of the message and
/// is never replaced by a consumer-specific category error.
fn conversion_failure_diagnostic(
    failure: value_conversion::ConversionFailure,
    context: Option<&str>,
) -> Diagnostic {
    conversion_failure_diagnostic_with_detail(failure, context, None)
}

fn conversion_failure_diagnostic_with_detail(
    failure: value_conversion::ConversionFailure,
    context: Option<&str>,
    detail: Option<&str>,
) -> Diagnostic {
    let (reason, target) = match failure.error {
        value_conversion::ConversionError::InvalidText { target } => {
            ("invalid text", target.label())
        }
        value_conversion::ConversionError::UnsupportedValue { target } => {
            ("unsupported value category", target.label())
        }
    };
    let parameter = failure
        .parameter_name
        .as_deref()
        .map(|name| format!(" for parameter `{name}`"))
        .unwrap_or_default();
    let detail = detail
        .map(|detail| format!("; {detail}"))
        .unwrap_or_default();
    let message = match context {
        Some(context) => {
            format!("{context}: {reason} for target {target}{parameter}{detail}")
        }
        None => format!("target conversion: {reason} for target {target}{parameter}{detail}"),
    };
    let primary = failure
        .candidate_span
        .or(failure.parameter_span)
        .unwrap_or(failure.call_span);
    let secondary = failure
        .parameter_span
        .filter(|span| *span != primary)
        .into_iter()
        .collect();
    Diagnostic {
        code: "E3001".to_string(),
        severity: Severity::Error,
        message,
        primary: Some(primary),
        secondary,
        hints: vec![
            "Target conversion preserves its typed classification and candidate provenance; no generic coercion is applied."
                .to_string(),
        ],
    }
}

fn target_conversion_error_message(target: &str, span: SourceSpan, message: String) -> Diagnostic {
    Diagnostic {
        code: "E3001".to_string(),
        severity: Severity::Error,
        message: format!("{target}: {message}"),
        primary: Some(span),
        secondary: Vec::new(),
        hints: vec![
            "Target conversion preserves the value category and does not apply a generic string fallback."
                .to_string(),
        ],
    }
}

fn raw_native_body_string_value(
    raw_body: &IrRawBody,
) -> Result<String, value_conversion::ConversionError> {
    let Some(source) = raw_body.source.slice(raw_body.span) else {
        return Err(value_conversion::ConversionError::UnsupportedValue {
            target: value_conversion::ConversionTarget::String,
        });
    };
    let native = normalize_html_native_body(source);
    if !native.is_empty() {
        return Ok(native);
    }
    value_conversion::raw_body_dynamic_text(raw_body).ok_or(
        value_conversion::ConversionError::UnsupportedValue {
            target: value_conversion::ConversionTarget::String,
        },
    )
}

fn normalize_html_native_body(source: &str) -> String {
    // `.html` retains its pre-existing native-content contract: the body
    // token's first line terminator is the call/body delimiter, surrounding
    // blank lines are omitted, and only the first native line's indentation
    // is removed. This target-local normalization must not shape RawBody.
    let source = if let Some(source) = source.strip_prefix("\r\n") {
        source
    } else {
        source.strip_prefix('\n').unwrap_or(source)
    };
    let lines = native_source_line_ranges(source);
    let Some(first) = lines.iter().find(|line| !line.is_blank(source)) else {
        return String::new();
    };
    let Some(last) = lines.iter().rfind(|line| !line.is_blank(source)) else {
        return String::new();
    };
    let source = &source[first.start..last.end];
    let first_line_end = source.find('\n').map_or(source.len(), |index| index + 1);
    let first_line = &source[..first_line_end];
    let first_content = first_line.strip_suffix('\n').unwrap_or(first_line);
    let first_content = first_content.strip_suffix('\r').unwrap_or(first_content);
    let indentation = first_content
        .chars()
        .take_while(|character| *character == ' ' || *character == '\t')
        .map(char::len_utf8)
        .sum();
    let mut normalized = String::with_capacity(source.len().saturating_sub(indentation));
    normalized.push_str(&first_line[indentation..]);
    normalized.push_str(&source[first_line_end..]);
    normalized
}

#[derive(Debug, Clone, Copy)]
struct NativeSourceLine {
    start: usize,
    content_end: usize,
    end: usize,
}

impl NativeSourceLine {
    fn is_blank(self, source: &str) -> bool {
        source[self.start..self.content_end]
            .chars()
            .all(char::is_whitespace)
    }
}

fn native_source_line_ranges(source: &str) -> Vec<NativeSourceLine> {
    let mut ranges = Vec::new();
    let bytes = source.as_bytes();
    let mut start = 0;
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\n' {
            let content_end = if index > start && bytes[index - 1] == b'\r' {
                index - 1
            } else {
                index
            };
            ranges.push(NativeSourceLine {
                start,
                content_end,
                end: index + 1,
            });
            start = index + 1;
        } else if bytes[index] == b'\r' {
            let (content_end, end) = if bytes.get(index + 1) == Some(&b'\n') {
                (index, index + 2)
            } else {
                (index, index + 1)
            };
            ranges.push(NativeSourceLine {
                start,
                content_end,
                end,
            });
            start = end;
            index = end;
            continue;
        }
        index += 1;
    }
    if start < source.len() || source.is_empty() {
        ranges.push(NativeSourceLine {
            start,
            content_end: source.len(),
            end: source.len(),
        });
    }
    ranges
}

fn html_argument_error(message: &str, span: SourceSpan) -> Diagnostic {
    html_argument_error_at(message.to_string(), span)
}

fn html_argument_error_at(message: String, span: SourceSpan) -> Diagnostic {
    Diagnostic {
        code: "E3003".to_string(),
        severity: Severity::Error,
        message,
        primary: Some(span),
        secondary: Vec::new(),
        hints: vec!["`.html` accepts exactly one regular `content` String argument.".to_string()],
    }
}

fn native_content_denied(span: SourceSpan) -> Diagnostic {
    Diagnostic {
        code: "E3004".to_string(),
        severity: Severity::Error,
        message: "NativeContent capability is required for `.html`".to_string(),
        primary: Some(span),
        secondary: Vec::new(),
        hints: vec![
            "Grant the NativeContent capability for this compilation to enable `.html`."
                .to_string(),
        ],
    }
}

fn unsupported_raw_html(span: SourceSpan) -> Diagnostic {
    Diagnostic {
        code: "E8001".to_string(),
        severity: Severity::Error,
        message: "Raw HTML is unsupported outside an owning target-specific function argument"
            .to_string(),
        primary: Some(span),
        secondary: Vec::new(),
        hints: vec![
            "Use Quarkdown `.html` for target-specific HTML content; ordinary mixed raw HTML remains unsupported."
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
        IrValue::Range(range) => range.span,
        IrValue::Callable(callable) => callable.span,
        IrValue::InlineBody(body) => body.span,
        IrValue::Component(component) => component.span(),
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

fn materialized_elements_limit_error(requested: u64, limit: usize, span: SourceSpan) -> Diagnostic {
    Diagnostic {
        code: "E3005".to_string(),
        severity: Severity::Error,
        message: format!(
            "materialized element limit exceeded: requested {requested}, maximum is {limit}"
        ),
        primary: Some(span),
        secondary: Vec::new(),
        hints: vec![
            "Reduce the size of this range or iterable, or configure a higher evaluator materialization limit."
                .to_string(),
        ],
    }
}

fn evaluation_depth_limit_error(limit: usize, span: SourceSpan) -> Diagnostic {
    Diagnostic {
        code: "E3005".to_string(),
        severity: Severity::Error,
        message: format!(
            "evaluation depth limit exceeded: maximum is {limit} active evaluator frame(s)"
        ),
        primary: Some(span),
        secondary: Vec::new(),
        hints: vec![
            "Reduce recursive or nested function/callback evaluation, or configure a higher evaluator depth limit."
                .to_string(),
        ],
    }
}

fn stacked_inline_materialization_error(span: SourceSpan) -> Diagnostic {
    Diagnostic {
        code: "E3001".to_string(),
        severity: Severity::Error,
        message: "Stacked layout is block-only".to_string(),
        primary: Some(span),
        secondary: Vec::new(),
        hints: vec![
            "Use `.row`, `.column`, or `.grid` as a block call with a Markdown body.".to_string(),
        ],
    }
}

fn component_inline_materialization_error(span: SourceSpan) -> Diagnostic {
    Diagnostic {
        code: "E3001".to_string(),
        severity: Severity::Error,
        message: "Semantic component is block-only".to_string(),
        primary: Some(span),
        secondary: Vec::new(),
        hints: vec!["Use the component as a block call with a Markdown body.".to_string()],
    }
}

fn center_argument_error(message: &str, span: SourceSpan) -> Diagnostic {
    Diagnostic {
        code: "E3003".to_string(),
        severity: Severity::Error,
        message: message.to_string(),
        primary: Some(span),
        secondary: Vec::new(),
        hints: vec![
            "`.center` accepts exactly one required Markdown block body and no arguments."
                .to_string(),
        ],
    }
}

fn center_inline_materialization_error(span: SourceSpan) -> Diagnostic {
    Diagnostic {
        code: "E3001".to_string(),
        severity: Severity::Error,
        message: "`.center` is block-only".to_string(),
        primary: Some(span),
        secondary: Vec::new(),
        hints: vec!["Use `.center` as a block call with a Markdown body.".to_string()],
    }
}

fn align_argument_error(message: &str, span: SourceSpan) -> Diagnostic {
    align_argument_error_at(message.to_string(), span)
}

fn align_argument_error_at(message: String, span: SourceSpan) -> Diagnostic {
    Diagnostic {
        code: "E3003".to_string(),
        severity: Severity::Error,
        message,
        primary: Some(span),
        secondary: Vec::new(),
        hints: vec![
            "`.align` accepts one required alignment argument and one Markdown block body."
                .to_string(),
        ],
    }
}

fn align_conversion_error(
    span: SourceSpan,
    parameter_span: Option<SourceSpan>,
    error: value_conversion::ConversionError,
) -> Diagnostic {
    conversion_failure_diagnostic(
        value_conversion::ConversionFailure::new(
            error,
            Some(span),
            Some("alignment"),
            parameter_span,
            span,
        ),
        Some("`.align`"),
    )
}

fn align_inline_materialization_error(span: SourceSpan) -> Diagnostic {
    Diagnostic {
        code: "E3001".to_string(),
        severity: Severity::Error,
        message: "`.align` is block-only".to_string(),
        primary: Some(span),
        secondary: Vec::new(),
        hints: vec!["Use `.align` as a block call with a Markdown body.".to_string()],
    }
}

fn container_inline_materialization_error(span: SourceSpan) -> Diagnostic {
    Diagnostic {
        code: "E3001".to_string(),
        severity: Severity::Error,
        message: "`.container` is block-only".to_string(),
        primary: Some(span),
        secondary: Vec::new(),
        hints: vec!["Use `.container` as a block call with an optional Markdown body.".to_string()],
    }
}

fn landscape_argument_error(message: &str, span: SourceSpan) -> Diagnostic {
    Diagnostic {
        code: "E3003".to_string(),
        severity: Severity::Error,
        message: message.to_string(),
        primary: Some(span),
        secondary: Vec::new(),
        hints: vec![
            "`.landscape` accepts exactly one required Markdown block body and no arguments."
                .to_string(),
        ],
    }
}

fn landscape_inline_materialization_error(span: SourceSpan) -> Diagnostic {
    Diagnostic {
        code: "E3001".to_string(),
        severity: Severity::Error,
        message: "`.landscape` is block-only".to_string(),
        primary: Some(span),
        secondary: Vec::new(),
        hints: vec!["Use `.landscape` as a block call with a Markdown body.".to_string()],
    }
}

fn br_argument_error(message: &str, span: SourceSpan) -> Diagnostic {
    Diagnostic {
        code: "E3003".to_string(),
        severity: Severity::Error,
        message: message.to_string(),
        primary: Some(span),
        secondary: Vec::new(),
        hints: vec!["`.br` accepts no arguments and no body.".to_string()],
    }
}

fn whitespace_argument_error(message: &str, span: SourceSpan) -> Diagnostic {
    whitespace_argument_error_at(message.to_string(), span)
}

fn whitespace_argument_error_at(message: String, span: SourceSpan) -> Diagnostic {
    Diagnostic {
        code: "E3003".to_string(),
        severity: Severity::Error,
        message,
        primary: Some(span),
        secondary: Vec::new(),
        hints: vec![
            "`.whitespace` accepts optional `width` and `height` Size arguments and no body."
                .to_string(),
        ],
    }
}

fn whitespace_conversion_error(
    parameter: &str,
    span: SourceSpan,
    parameter_span: Option<SourceSpan>,
    error: value_conversion::ConversionError,
) -> Diagnostic {
    conversion_failure_diagnostic(
        value_conversion::ConversionFailure::new(
            error,
            Some(span),
            Some(parameter),
            parameter_span,
            span,
        ),
        Some("`.whitespace`"),
    )
}

/// Resolves a value to a boolean, handling variable references.
fn resolve_boolean_value(
    value: &InvocationValue,
) -> Result<bool, value_conversion::ConversionError> {
    match value_conversion::convert_scalar_with_origin(value, ScalarTarget::Boolean) {
        Ok(ScalarValue::Boolean(value)) => Ok(value),
        Ok(_) => Err(value_conversion::ConversionError::UnsupportedValue {
            target: value_conversion::ConversionTarget::Boolean,
        }),
        Err(error) => Err(error),
    }
}

/// Maps a scalar value to its boolean meaning (without variable resolution).
/// Supports the Quarkdown boolean literals `true`/`yes` and `false`/`no`,
/// case-insensitive (Quarkdown "Boolean" documentation, badged `v2.5.0`).
fn scalar_boolean_value(value: &IrValue) -> Option<bool> {
    match value {
        IrValue::Boolean(value) => Some(*value),
        _ => None,
    }
}

/// Classifies the value expression at the Quarkdown invocation boundary.
///
/// Raw scalar arguments and references to variables or user functions enter
/// the upstream DynamicValue binder path. A nested builtin such as
/// `.string`, a typed range, or a resource result is already a materialized
/// semantic value and must not be reinterpreted by unrelated target types.
fn invocation_origin(value: &IrValue, context: &EvaluationContext<'_>) -> ValueOrigin {
    match value {
        IrValue::String(_) | IrValue::Identifier(_) => ValueOrigin::Dynamic,
        IrValue::Content(nodes) => match nodes.as_slice() {
            [IrNode::FunctionCall { name, .. }]
            | [IrNode::ChainedFunctionCall {
                head: IrCallSegment { name, .. },
                ..
            }] if context.contains(name) || context.get_function(name).is_some() => {
                ValueOrigin::Dynamic
            }
            _ => ValueOrigin::Static,
        },
        _ => ValueOrigin::Static,
    }
}

fn call_result_origin(name: &str, context: &EvaluationContext<'_>) -> ValueOrigin {
    if context.contains(name) || context.get_function(name).is_some() {
        ValueOrigin::Dynamic
    } else {
        ValueOrigin::Static
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
    has_native_owner(name, NativeDispatchOwner::VariableState)
}

/// Returns true if a call is a variable reference (parameterless call to a known variable).
fn is_variable_reference_call(
    name: &str,
    positional_args: &[IrValue],
    named_args: &[IrNamedArg],
    body: Option<CallBody<'_>>,
    context: &EvaluationContext<'_>,
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
    context: &EvaluationContext<'_>,
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
        IrValue::Number(number) => Ok(scalar_number_to_text(*number)),
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
        IrValue::Size(_) | IrValue::Color(_) | IrValue::Enum(_) => {
            diagnostics.push(iteration_error(
                "Domain values cannot be rendered as scalar text without a domain consumer"
                    .to_string(),
                span,
            ));
            Err(CallOutcome::Failed)
        }
        IrValue::Component(component) => {
            diagnostics.push(iteration_error(
                "A semantic component cannot be rendered as scalar text".to_string(),
                component.span(),
            ));
            Err(CallOutcome::Failed)
        }
        IrValue::Callable(_) => {
            diagnostics.push(iteration_error(
                "A callable cannot be rendered as scalar text".to_string(),
                span,
            ));
            Err(CallOutcome::Failed)
        }
        IrValue::InlineBody(_) => {
            diagnostics.push(iteration_error(
                "A contextual inline body cannot be rendered as scalar text".to_string(),
                span,
            ));
            Err(CallOutcome::Failed)
        }
    }
}

/// Keeps the shortest decimal representation of numeric builtin results that
/// crossed the upstream `Float` boundary, while preserving f64-only values
/// originating elsewhere in the IR.
fn scalar_number_to_text(number: f64) -> String {
    value_conversion::number_to_text(number)
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
        context: &mut EvaluationContext<'_>,
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
        let capture = Some(Box::new(context.capture_snapshot()));
        context.set_function_binding(
            function_name.clone(),
            lambda_parameters,
            body.to_vec(),
            *span,
            capture,
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
        context: &mut EvaluationContext<'_>,
        binding_plan: Option<&BindingPlan>,
    ) -> CallOutcome {
        let Some(binding_plan) = binding_plan else {
            return CallOutcome::Failed;
        };
        let body_placeholder = body.map(|body| Candidate::Positional {
            value: IrValue::None,
            span: call_body_source_span(body, *span),
        });
        let raw_candidates = raw_invocation_candidates(positional_args, named_args, *span);
        let bound = match binding_plan.bind(&raw_candidates, body_placeholder.as_ref(), *span) {
            Ok(bound) => bound,
            Err(error) => {
                diagnostics.push(binding_diagnostic_with_code(error, "E3003"));
                return CallOutcome::Failed;
            }
        };
        let Some(BoundSlot::Explicit {
            value: raw_name, ..
        }) = bound.slots.first()
        else {
            diagnostics.push(invalid_var_declaration(span));
            return CallOutcome::Failed;
        };
        let var_name = match raw_name {
            IrValue::Identifier(name) | IrValue::String(name) => name.clone(),
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
                    context.assign_value(var_name, value);
                    return CallOutcome::NoValue;
                }
                CallOutcome::Failed => return CallOutcome::Failed,
                CallOutcome::NoValue | CallOutcome::Unresolved => {
                    return CallOutcome::Failed;
                }
            }
        }
        if let Some(BoundSlot::Explicit { value, .. }) = bound.slots.get(1) {
            match self.evaluate_value(value, diagnostics, context) {
                CallOutcome::Value(value) => {
                    context.assign_value(var_name, value);
                    return CallOutcome::NoValue;
                }
                CallOutcome::Unresolved => {
                    match self.preserve_value_expression(value, diagnostics, context) {
                        Ok(value) => {
                            context.assign_value(var_name, value);
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
        context: &mut EvaluationContext<'_>,
    ) -> CallOutcome {
        let value = &positional_args[0];
        match self.evaluate_value(value, diagnostics, context) {
            CallOutcome::Value(value) => {
                context.assign_value(name.to_string(), value);
                CallOutcome::NoValue
            }
            CallOutcome::Unresolved => {
                match self.preserve_value_expression(value, diagnostics, context) {
                    Ok(value) => {
                        context.assign_value(name.to_string(), value);
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

/// Dynamic Markdown text has one reliable provenance point: the source
/// expression that supplied the text. The text itself is not a source buffer,
/// so parser offsets inside it must not be presented as offsets in the
/// caller's document. Rebase the parsed tree to that expression span before
/// evaluation while retaining all structural content.
fn rebase_dynamic_nodes(nodes: &mut [IrNode], source_span: SourceSpan) {
    for node in nodes {
        rebase_dynamic_node(node, source_span);
    }
}

fn rebase_dynamic_node(node: &mut IrNode, source_span: SourceSpan) {
    match node {
        IrNode::Heading { content, span, .. } | IrNode::Paragraph { content, span, .. } => {
            *span = source_span;
            rebase_dynamic_inlines(content, source_span);
        }
        IrNode::Blockquote { content, span } => {
            *span = source_span;
            rebase_dynamic_nodes(content, source_span);
        }
        IrNode::UnorderedList { items, span } | IrNode::OrderedList { items, span, .. } => {
            *span = source_span;
            for item in items {
                item.span = source_span;
                rebase_dynamic_nodes(&mut item.nodes, source_span);
            }
        }
        IrNode::Table { header, rows, span } => {
            *span = source_span;
            rebase_dynamic_table_row(header, source_span);
            for row in rows {
                rebase_dynamic_table_row(row, source_span);
            }
        }
        IrNode::CodeBlock { span, .. }
        | IrNode::RawHtml { span, .. }
        | IrNode::ThematicBreak { span }
        | IrNode::Math { span, .. } => *span = source_span,
        IrNode::TargetSpecificContent { content } => content.span = source_span,
        IrNode::Component { component } => rebase_dynamic_component(component, source_span),
        IrNode::FunctionCall {
            positional_args,
            named_args,
            ordered_args,
            lambda_parameters,
            body,
            span,
            ..
        } => {
            *span = source_span;
            for value in positional_args {
                rebase_dynamic_value(value, source_span);
            }
            rebase_dynamic_named_args(named_args, source_span);
            rebase_dynamic_ordered_args(ordered_args, source_span);
            rebase_dynamic_parameters(lambda_parameters, source_span);
            if let Some(body) = body {
                rebase_dynamic_nodes(body, source_span);
            }
            // `raw_body.span` is local to the temporary SourceText created
            // for this dynamic parse. Keep that source-local range intact;
            // the enclosing call span is the evaluator-only provenance used
            // for diagnostics and binding errors.
        }
        IrNode::ChainedFunctionCall {
            head,
            chain,
            body,
            span,
            ..
        } => {
            *span = source_span;
            rebase_dynamic_segment(head, source_span);
            for segment in chain {
                rebase_dynamic_segment(segment, source_span);
            }
            if let Some(body) = body {
                rebase_dynamic_nodes(body, source_span);
            }
            // A dynamic raw body's span belongs to its own source buffer and
            // must not be rebased into the caller's document coordinate space.
        }
        IrNode::FunctionDeclaration {
            name,
            parameters,
            body,
            span,
            ..
        } => {
            *span = source_span;
            rebase_dynamic_value(name, source_span);
            for parameter in parameters {
                parameter.span = source_span;
                parameter.name_span = source_span;
            }
            rebase_dynamic_nodes(body, source_span);
        }
    }
}

fn rebase_dynamic_inlines(inlines: &mut [IrInline], source_span: SourceSpan) {
    for inline in inlines {
        match inline {
            IrInline::Text { span, .. }
            | IrInline::Whitespace { span, .. }
            | IrInline::Code { span, .. }
            | IrInline::SoftBreak { span }
            | IrInline::HardBreak { span }
            | IrInline::RawHtml { span, .. } => *span = source_span,
            IrInline::Emphasis { content, span }
            | IrInline::Strong { content, span }
            | IrInline::Strikethrough { content, span } => {
                *span = source_span;
                rebase_dynamic_inlines(content, source_span);
            }
            IrInline::DirectiveCall {
                positional_args,
                named_args,
                ordered_args,
                body,
                span,
                ..
            } => {
                *span = source_span;
                for value in positional_args {
                    rebase_dynamic_value(value, source_span);
                }
                rebase_dynamic_named_args(named_args, source_span);
                rebase_dynamic_ordered_args(ordered_args, source_span);
                if let Some(body) = body {
                    rebase_dynamic_inlines(body, source_span);
                }
            }
            IrInline::ChainedDirectiveCall {
                head,
                chain,
                body,
                span,
            } => {
                *span = source_span;
                rebase_dynamic_segment(head, source_span);
                for segment in chain {
                    rebase_dynamic_segment(segment, source_span);
                }
                if let Some(body) = body {
                    rebase_dynamic_inlines(body, source_span);
                }
            }
            IrInline::Link { content, span, .. } | IrInline::Image { content, span, .. } => {
                *span = source_span;
                rebase_dynamic_inlines(content, source_span);
            }
            IrInline::TargetSpecificContent { content } => content.span = source_span,
        }
    }
}

fn rebase_dynamic_table_row(row: &mut IrTableRow, source_span: SourceSpan) {
    row.span = source_span;
    for cell in &mut row.cells {
        cell.span = source_span;
        rebase_dynamic_inlines(&mut cell.content, source_span);
    }
}

fn rebase_dynamic_named_args(args: &mut [IrNamedArg], source_span: SourceSpan) {
    for argument in args {
        argument.span = source_span;
        argument.name_span = source_span;
        rebase_dynamic_value(&mut argument.value, source_span);
    }
}

fn rebase_dynamic_ordered_args(args: &mut Option<Vec<IrCallArgument>>, source_span: SourceSpan) {
    if let Some(args) = args {
        for argument in args {
            match argument {
                IrCallArgument::Positional { span, .. } => *span = source_span,
                IrCallArgument::Named {
                    name_span, span, ..
                } => {
                    *name_span = source_span;
                    *span = source_span;
                }
            }
        }
    }
}

fn rebase_dynamic_parameters(parameters: &mut Option<Vec<IrParameter>>, source_span: SourceSpan) {
    if let Some(parameters) = parameters {
        for parameter in parameters {
            parameter.span = source_span;
            parameter.name_span = source_span;
        }
    }
}

fn rebase_dynamic_segment(segment: &mut IrCallSegment, source_span: SourceSpan) {
    segment.span = source_span;
    segment.name_span = source_span;
    for value in &mut segment.positional_args {
        rebase_dynamic_value(value, source_span);
    }
    rebase_dynamic_named_args(&mut segment.named_args, source_span);
    rebase_dynamic_ordered_args(&mut segment.ordered_args, source_span);
}

fn rebase_dynamic_value(value: &mut IrValue, source_span: SourceSpan) {
    match value {
        IrValue::Range(range) => range.span = source_span,
        IrValue::Collection(values) => {
            for value in values {
                rebase_dynamic_value(value, source_span);
            }
        }
        IrValue::Pair(pair) => {
            pair.span = source_span;
            rebase_dynamic_value(&mut pair.first, source_span);
            rebase_dynamic_value(&mut pair.second, source_span);
        }
        IrValue::Dictionary(dictionary) => {
            dictionary.span = source_span;
            for pair in &mut dictionary.entries {
                rebase_dynamic_value(&mut pair.first, source_span);
                rebase_dynamic_value(&mut pair.second, source_span);
                pair.span = source_span;
            }
        }
        IrValue::Content(nodes) => rebase_dynamic_nodes(nodes, source_span),
        IrValue::Component(component) => rebase_dynamic_component(component, source_span),
        IrValue::Callable(callable) => {
            callable.span = source_span;
            rebase_dynamic_parameters(&mut callable.parameters, source_span);
            rebase_dynamic_nodes(&mut callable.body, source_span);
            if let Some(capture) = &mut callable.capture {
                for variable in &mut capture.variables {
                    rebase_dynamic_value(&mut variable.value, source_span);
                }
                for function in &mut capture.functions {
                    function.callable.span = source_span;
                    rebase_dynamic_parameters(&mut function.callable.parameters, source_span);
                    rebase_dynamic_nodes(&mut function.callable.body, source_span);
                }
            }
        }
        IrValue::InlineBody(body) => {
            body.span = source_span;
            rebase_dynamic_parameters(&mut body.parameters, source_span);
            rebase_dynamic_nodes(&mut body.content, source_span);
            rebase_dynamic_nodes(&mut body.body, source_span);
        }
        IrValue::String(_)
        | IrValue::Number(_)
        | IrValue::Boolean(_)
        | IrValue::Identifier(_)
        | IrValue::Size(_)
        | IrValue::Color(_)
        | IrValue::Enum(_)
        | IrValue::None => {}
    }
}

fn rebase_dynamic_component(component: &mut IrComponent, source_span: SourceSpan) {
    match component {
        IrComponent::Stacked(component) => {
            component.span = source_span;
            rebase_dynamic_nodes(&mut component.children, source_span);
        }
        IrComponent::Container(component) => {
            component.span = source_span;
            rebase_dynamic_nodes(&mut component.children, source_span);
        }
        IrComponent::Landscape(component) => {
            component.span = source_span;
            rebase_dynamic_nodes(&mut component.children, source_span);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arkst_ir::{
        IrCaptionPosition, IrColor, IrComponent, IrCrossAxisAlignment, IrListItem,
        IrMainAxisAlignment, IrSize, IrSizeUnit, IrStackedComponent, IrStackedLayout,
    };
    use arkst_source::SourceId;

    fn span(start: usize, end: usize) -> SourceSpan {
        SourceSpan::new(SourceId(1), start, end)
    }

    #[test]
    fn dynamic_markdown_reparse_keeps_raw_body_spans_source_local() {
        let dynamic = ".theme\n  .docname\n    nested\n";
        let parsed = arkst_markdown::parse_with_mode(dynamic, Mode::Quarkdown);
        assert!(
            parsed.diagnostics.is_empty(),
            "unexpected: {:?}",
            parsed.diagnostics
        );
        let (document, conversion_diagnostics) = ast_to_ir::ast_to_ir_with_diagnostics_for_mode(
            &parsed.document,
            SourceId(9),
            &crate::DocumentMetadataDefaults::default(),
            Mode::Quarkdown,
        );
        assert!(
            conversion_diagnostics.is_empty(),
            "unexpected: {conversion_diagnostics:?}"
        );

        let caller_span = SourceSpan::new(SourceId(1), 500, 512);
        let mut nodes = document.nodes;
        rebase_dynamic_nodes(&mut nodes, caller_span);

        let IrNode::FunctionCall {
            raw_body: Some(outer_raw),
            body: Some(body),
            span: outer_span,
            ..
        } = &nodes[0]
        else {
            panic!("expected a reparsed block call: {nodes:?}");
        };
        assert_eq!(*outer_span, caller_span);
        assert!(outer_raw.source.slice(outer_raw.span).is_some());

        let IrNode::FunctionCall {
            raw_body: Some(nested_raw),
            span: nested_span,
            ..
        } = &body[0]
        else {
            panic!("expected a nested reparsed block call: {body:?}");
        };
        assert_eq!(*nested_span, caller_span);
        assert!(nested_raw.source.slice(nested_raw.span).is_some());

        let mut diagnostics = Vec::new();
        let candidate = source_backed_body_candidate(
            Some(caller_span),
            Some(nested_raw),
            "dynamic nested target",
            &mut diagnostics,
        )
        .expect("valid source-local span")
        .expect("body candidate");
        let Candidate::Positional { span, .. } = candidate else {
            panic!("expected positional body candidate");
        };
        assert_eq!(span, caller_span);
        assert!(diagnostics.is_empty());

        let roundtripped = serde_json::from_value::<IrDocument>(
            serde_json::to_value(IrDocument {
                nodes,
                metadata: arkst_ir::IrMetadata::default(),
            })
            .expect("dynamic IR serializes"),
        )
        .expect("dynamic IR deserializes");
        let IrNode::FunctionCall {
            body: Some(body), ..
        } = &roundtripped.nodes[0]
        else {
            panic!("expected a roundtripped dynamic call");
        };
        let IrNode::FunctionCall {
            raw_body: Some(nested_raw),
            ..
        } = &body[0]
        else {
            panic!("expected a roundtripped nested call");
        };
        assert!(nested_raw.source.slice(nested_raw.span).is_some());
    }

    fn named_arg(name: &str, value: IrValue) -> IrNamedArg {
        IrNamedArg {
            name: name.to_string(),
            name_span: span(0, name.len()),
            value,
            span: span(0, name.len()),
        }
    }

    fn named_arg_at(
        name: &str,
        value: IrValue,
        name_span: SourceSpan,
        argument_span: SourceSpan,
    ) -> IrNamedArg {
        IrNamedArg {
            name: name.to_string(),
            name_span,
            value,
            span: argument_span,
        }
    }

    #[test]
    fn native_dispatch_inventory_has_exactly_one_owner() {
        let mut registered = Vec::new();

        for builtin in builtins::regular_builtins() {
            assert_eq!(
                native_dispatch_owner(builtin.name),
                Some(NativeDispatchOwner::RegularScalar),
                "regular builtin {} has no unique owner",
                builtin.name
            );
            assert!(
                registered.iter().all(|(name, _)| *name != builtin.name),
                "{} is registered by more than one native owner",
                builtin.name
            );
            registered.push((builtin.name, NativeDispatchOwner::RegularScalar));
        }

        for inventory in bespoke_native_owners() {
            for &name in inventory.names {
                assert_eq!(
                    native_dispatch_owner(name),
                    Some(inventory.owner),
                    "{name} does not have one unique native owner"
                );
                assert!(
                    registered
                        .iter()
                        .all(|(registered_name, _)| *registered_name != name),
                    "{name} is registered by more than one native owner"
                );
                registered.push((name, inventory.owner));
            }
        }

        for &name in deferred_native_names() {
            assert_eq!(
                native_dispatch_owner(name),
                None,
                "deferred name {name} must not be a supported native owner"
            );
        }
    }

    fn collection_call(
        evaluator: &Evaluator,
        name: &str,
        positional_args: &[IrValue],
        named_args: &[IrNamedArg],
        operation_span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
    ) -> CallOutcome {
        evaluator.evaluate_call_value(
            name,
            positional_args,
            named_args,
            None,
            None,
            operation_span,
            diagnostics,
            context,
        )
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
            ordered_args: None,
            lambda_parameters: None,
            body: Some(body),
            raw_body: None,
            span: span(0, 1),
        }
    }

    fn inline_if_call(name: &str, condition: IrValue, inline_body: Vec<IrInline>) -> IrInline {
        IrInline::DirectiveCall {
            name: name.to_string(),
            positional_args: vec![condition],
            named_args: Vec::new(),
            ordered_args: None,
            body: Some(inline_body),
            span: span(0, 1),
        }
    }

    fn doc(nodes: Vec<IrNode>) -> IrDocument {
        IrDocument {
            nodes,
            metadata: arkst_ir::IrMetadata::default(),
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
            ordered_args: None,
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
            raw_body: None,
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
            raw_body: None,
            span,
        }
    }

    fn call_value(name: &str, positional_args: Vec<IrValue>) -> IrValue {
        IrValue::Content(vec![IrNode::FunctionCall {
            name: name.to_string(),
            positional_args,
            named_args: Vec::new(),
            ordered_args: None,
            lambda_parameters: None,
            body: None,
            raw_body: None,
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
            ordered_args: None,
            lambda_parameters,
            body,
            raw_body: None,
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
            ordered_args: None,
            lambda_parameters,
            body: Some(body),
            raw_body: None,
            span: span(0, 20),
        }
    }

    fn transform_callable(parameters: Option<Vec<IrParameter>>, body: Vec<IrNode>) -> IrValue {
        IrValue::Callable(IrCallable {
            parameters,
            body,
            span: span(50, 60),
            capture: None,
        })
    }

    fn component_value(component_span: SourceSpan) -> IrValue {
        IrValue::Component(IrComponent::Stacked(IrStackedComponent {
            layout: IrStackedLayout::Column,
            main_axis_alignment: IrMainAxisAlignment::Start,
            cross_axis_alignment: IrCrossAxisAlignment::Center,
            row_gap: Some(IrSize {
                value: 10.0,
                unit: IrSizeUnit::Px,
            }),
            column_gap: None,
            children: vec![text_paragraph("component child")],
            span: component_span,
        }))
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
            ordered_args: None,
            lambda_parameters: None,
            body: None,
            raw_body: None,
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
                ordered_args: None,
                lambda_parameters: None,
                body: None,
                raw_body: None,
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
                        ordered_args: None,
                        lambda_parameters: None,
                        body: None,
                        raw_body: None,
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
    fn let_parameter_shadows_parent_and_var_reassignment_updates_parent() {
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
        assert_paragraph_text(&nodes[1..2], "local");
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
    fn collection_transforms_share_typed_iterable_and_callable_paths() {
        let evaluator = Evaluator::new();
        let operation_span = span(0, 40);
        let range = IrValue::Range(IrRange {
            start: Some(-2),
            end: Some(2),
            span: span(1, 6),
        });
        let identity = transform_callable(None, vec![var_ref("1")]);
        let mut diagnostics = Vec::new();
        let mut context = EvaluationContext::new();

        let mapped = evaluator.evaluate_call_value(
            "map",
            std::slice::from_ref(&range),
            &[named_arg("by", identity.clone())],
            None,
            None,
            &operation_span,
            &mut diagnostics,
            &mut context,
        );
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert!(matches!(
            mapped,
            CallOutcome::Value(IrValue::Collection(values))
                if values == (-2..=2).map(|value| IrValue::Number(f64::from(value))).collect::<Vec<_>>()
        ));

        let predicate = transform_callable(
            None,
            vec![IrNode::FunctionCall {
                name: "isnone".to_string(),
                positional_args: vec![call_value("1", Vec::new())],
                named_args: Vec::new(),
                ordered_args: None,
                lambda_parameters: None,
                body: None,
                raw_body: None,
                span: span(10, 20),
            }],
        );
        let filter_input =
            IrValue::Collection(vec![IrValue::None, IrValue::Number(-1.0), IrValue::None]);
        let filtered = evaluator.evaluate_call_value(
            "filter",
            std::slice::from_ref(&filter_input),
            &[named_arg("by", predicate)],
            None,
            None,
            &operation_span,
            &mut diagnostics,
            &mut context,
        );
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert!(matches!(
            filtered,
            CallOutcome::Value(IrValue::Collection(values))
                if values == vec![IrValue::None, IrValue::None]
        ));

        let sorted = evaluator.evaluate_call_value(
            "sorted",
            &[IrValue::Collection(vec![
                IrValue::Number(3.0),
                IrValue::Number(1.0),
                IrValue::Number(2.0),
                IrValue::Number(1.0),
            ])],
            &[],
            None,
            None,
            &operation_span,
            &mut diagnostics,
            &mut context,
        );
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert!(matches!(
            sorted,
            CallOutcome::Value(IrValue::Collection(values))
                if values == vec![
                    IrValue::Number(1.0),
                    IrValue::Number(1.0),
                    IrValue::Number(2.0),
                    IrValue::Number(3.0),
                ]
        ));
    }

    #[test]
    fn transforms_support_pair_dictionary_and_nested_typed_values() {
        let evaluator = Evaluator::new();
        let dictionary = IrValue::Dictionary(IrDictionary {
            entries: vec![
                IrPair {
                    first: Box::new(IrValue::String("a".to_string())),
                    second: Box::new(IrValue::Number(3.0)),
                    span: span(1, 5),
                },
                IrPair {
                    first: Box::new(IrValue::String("b".to_string())),
                    second: Box::new(IrValue::Number(1.0)),
                    span: span(6, 10),
                },
                IrPair {
                    first: Box::new(IrValue::String("c".to_string())),
                    second: Box::new(IrValue::Number(1.0)),
                    span: span(11, 15),
                },
            ],
            span: span(0, 10),
        });
        let parameters = vec![lambda_parameter("key", 20), lambda_parameter("value", 24)];
        let mut diagnostics = Vec::new();
        let mut context = EvaluationContext::new();
        let mapped = evaluator.evaluate_call_value(
            "map",
            std::slice::from_ref(&dictionary),
            &[],
            Some(CallBody::Block(&[var_ref("value")])),
            Some(&parameters),
            &span(0, 30),
            &mut diagnostics,
            &mut context,
        );
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert!(matches!(
            mapped,
            CallOutcome::Value(IrValue::Collection(values))
                if values == vec![
                    IrValue::Number(3.0),
                    IrValue::Number(1.0),
                    IrValue::Number(1.0),
                ]
        ));

        let sorted = evaluator.evaluate_call_value(
            "sorted",
            &[dictionary],
            &[named_arg(
                "by",
                transform_callable(Some(parameters), vec![var_ref("value")]),
            )],
            None,
            None,
            &span(0, 30),
            &mut diagnostics,
            &mut context,
        );
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let CallOutcome::Value(IrValue::Collection(values)) = sorted else {
            panic!("expected sorted collection")
        };
        assert!(matches!(values[0], IrValue::Pair(_)));
        assert!(matches!(values[1], IrValue::Pair(_)));
        let IrValue::Pair(first) = &values[0] else {
            unreachable!()
        };
        assert_eq!(*first.second, IrValue::Number(1.0));
        let IrValue::Pair(second) = &values[1] else {
            unreachable!()
        };
        let IrValue::Pair(third) = &values[2] else {
            unreachable!()
        };
        assert_eq!(*second.first, IrValue::String("c".to_string()));
        assert_eq!(*third.first, IrValue::String("a".to_string()));
    }

    #[test]
    fn sorted_supports_typed_keys_and_fails_closed_for_unsupported_keys() {
        let evaluator = Evaluator::new();
        let operation_span = span(0, 40);
        let mut diagnostics = Vec::new();
        let mut context = EvaluationContext::new();

        let strings = evaluator.evaluate_call_value(
            "sorted",
            &[IrValue::Collection(vec![
                IrValue::String("b".to_string()),
                IrValue::String("a".to_string()),
            ])],
            &[],
            None,
            None,
            &operation_span,
            &mut diagnostics,
            &mut context,
        );
        assert!(matches!(
            strings,
            CallOutcome::Value(IrValue::Collection(values))
                if values == vec![
                    IrValue::String("a".to_string()),
                    IrValue::String("b".to_string()),
                ]
        ));

        let nan_sorted = evaluator.evaluate_call_value(
            "sorted",
            &[IrValue::Collection(vec![
                IrValue::Number(1.0),
                IrValue::Number(f64::NAN),
                IrValue::Number(0.0),
            ])],
            &[],
            None,
            None,
            &operation_span,
            &mut diagnostics,
            &mut context,
        );
        let CallOutcome::Value(IrValue::Collection(values)) = nan_sorted else {
            panic!("expected NaN sort result")
        };
        assert_eq!(values[0], IrValue::Number(0.0));
        assert_eq!(values[1], IrValue::Number(1.0));
        assert!(matches!(values[2], IrValue::Number(value) if value.is_nan()));

        diagnostics.clear();
        let mixed = evaluator.evaluate_call_value(
            "sorted",
            &[IrValue::Collection(vec![
                IrValue::Number(1.0),
                IrValue::String("1".to_string()),
            ])],
            &[],
            None,
            None,
            &operation_span,
            &mut diagnostics,
            &mut context,
        );
        assert!(matches!(mixed, CallOutcome::Failed));
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("heterogeneous"));

        diagnostics.clear();
        let none = evaluator.evaluate_call_value(
            "sorted",
            &[IrValue::Collection(vec![IrValue::None])],
            &[],
            None,
            None,
            &operation_span,
            &mut diagnostics,
            &mut context,
        );
        assert!(matches!(none, CallOutcome::Failed));
        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn transform_failures_are_atomic_and_predicates_are_boolean_only() {
        let evaluator = Evaluator::new();
        let operation_span = span(0, 40);
        let failing = transform_callable(
            None,
            vec![IrNode::FunctionCall {
                name: "multiply".to_string(),
                positional_args: vec![IrValue::Boolean(true), call_value("1", Vec::new())],
                named_args: Vec::new(),
                ordered_args: None,
                lambda_parameters: None,
                body: None,
                raw_body: None,
                span: span(12, 20),
            }],
        );
        let mut diagnostics = Vec::new();
        let mut context = EvaluationContext::new();
        let mapped = evaluator.evaluate_call_value(
            "map",
            &[IrValue::Collection(vec![
                IrValue::Number(1.0),
                IrValue::Number(2.0),
            ])],
            &[named_arg("by", failing)],
            None,
            None,
            &operation_span,
            &mut diagnostics,
            &mut context,
        );
        assert!(matches!(mapped, CallOutcome::Failed));
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");

        diagnostics.clear();
        let invalid_predicate = transform_callable(
            None,
            vec![IrNode::Paragraph {
                content: vec![text_inline("not boolean")],
                span: span(20, 31),
            }],
        );
        let filtered = evaluator.evaluate_call_value(
            "filter",
            &[IrValue::Collection(vec![IrValue::Number(1.0)])],
            &[named_arg("by", invalid_predicate)],
            None,
            None,
            &operation_span,
            &mut diagnostics,
            &mut context,
        );
        assert!(matches!(filtered, CallOutcome::Failed));
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert!(diagnostics[0].message.contains("Boolean"));

        diagnostics.clear();
        let endless = evaluator.evaluate_call_value(
            "map",
            &[IrValue::Range(IrRange {
                start: Some(1),
                end: None,
                span: span(5, 8),
            })],
            &[named_arg(
                "by",
                transform_callable(None, vec![var_ref("1")]),
            )],
            None,
            None,
            &operation_span,
            &mut diagnostics,
            &mut context,
        );
        assert!(matches!(endless, CallOutcome::Failed));
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].primary, Some(span(5, 8)));
    }

    #[test]
    fn first_class_callable_captures_definition_values_and_applies_caller_overlay() {
        let evaluator = Evaluator::new();
        let mut diagnostics = Vec::new();
        let mut context = EvaluationContext::new();
        let span = span(0, 50);
        assert!(matches!(
            evaluator.evaluate_call_value(
                "var",
                &[
                    IrValue::Identifier("offset".to_string()),
                    IrValue::Number(10.0)
                ],
                &[],
                None,
                None,
                &span,
                &mut diagnostics,
                &mut context,
            ),
            CallOutcome::NoValue
        ));
        let callable = transform_callable(
            None,
            vec![IrNode::FunctionCall {
                name: "sum".to_string(),
                positional_args: vec![
                    call_value("1", Vec::new()),
                    call_value("offset", Vec::new()),
                ],
                named_args: Vec::new(),
                ordered_args: None,
                lambda_parameters: None,
                body: None,
                raw_body: None,
                span,
            }],
        );
        assert!(matches!(
            evaluator.evaluate_call_value(
                "var",
                &[IrValue::Identifier("add_offset".to_string()), callable],
                &[],
                None,
                None,
                &span,
                &mut diagnostics,
                &mut context,
            ),
            CallOutcome::NoValue
        ));
        assert!(matches!(
            evaluator.evaluate_call_value(
                "offset",
                &[IrValue::Number(20.0)],
                &[],
                None,
                None,
                &span,
                &mut diagnostics,
                &mut context,
            ),
            CallOutcome::NoValue
        ));
        let result = evaluator.evaluate_call_value(
            "map",
            &[IrValue::Collection(vec![IrValue::Number(1.0)])],
            &[named_arg("by", call_value("add_offset", Vec::new()))],
            None,
            None,
            &span,
            &mut diagnostics,
            &mut context,
        );
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert!(matches!(
            result,
            CallOutcome::Value(IrValue::Collection(values))
                if values == vec![IrValue::Number(21.0)]
        ));

        let wrong_arity = evaluator.evaluate_call_value(
            "map",
            &[IrValue::Collection(vec![IrValue::Number(1.0)])],
            &[named_arg(
                "by",
                transform_callable(
                    Some(vec![lambda_parameter("a", 1), lambda_parameter("b", 2)]),
                    vec![var_ref("a")],
                ),
            )],
            None,
            None,
            &span,
            &mut diagnostics,
            &mut context,
        );
        assert!(matches!(wrong_arity, CallOutcome::Failed));
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    }

    #[test]
    fn dynamic_range_returns_typed_signed_truncated_endpoints() {
        let evaluator = Evaluator::new();
        let operation_span = span(0, 20);
        let cases = [
            (
                vec![IrValue::Number(1.9), IrValue::Number(3.9)],
                Some(1),
                Some(3),
            ),
            (
                vec![IrValue::Number(-3.9), IrValue::Number(-1.1)],
                Some(-3),
                Some(-1),
            ),
            (
                vec![IrValue::Number(-0.9), IrValue::Number(0.9)],
                Some(0),
                Some(0),
            ),
        ];
        for (positional, start, end) in cases {
            let mut diagnostics = Vec::new();
            let mut context = EvaluationContext::new();
            let outcome = evaluator.evaluate_call_value(
                "range",
                &positional,
                &[],
                None,
                None,
                &operation_span,
                &mut diagnostics,
                &mut context,
            );
            assert!(diagnostics.is_empty(), "{diagnostics:?}");
            assert!(matches!(
                outcome,
                CallOutcome::Value(IrValue::Range(IrRange { start: actual_start, end: actual_end, .. }))
                    if actual_start == start && actual_end == end
            ));
        }

        let mut diagnostics = Vec::new();
        let mut context = EvaluationContext::new();
        let outcome = evaluator.evaluate_call_value(
            "range",
            &[],
            &[named_arg("to", IrValue::Number(3.0))],
            None,
            None,
            &operation_span,
            &mut diagnostics,
            &mut context,
        );
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert!(matches!(
            outcome,
            CallOutcome::Value(IrValue::Range(IrRange {
                start: None,
                end: Some(3),
                ..
            }))
        ));

        let equivalent_forms = [
            (vec![IrValue::Number(2.0), IrValue::Number(4.0)], Vec::new()),
            (
                vec![IrValue::Number(2.0)],
                vec![named_arg("to", IrValue::Number(4.0))],
            ),
            (
                Vec::new(),
                vec![
                    named_arg("from", IrValue::Number(2.0)),
                    named_arg("to", IrValue::Number(4.0)),
                ],
            ),
        ];
        for (positional, named) in equivalent_forms {
            let mut diagnostics = Vec::new();
            let mut context = EvaluationContext::new();
            let outcome = evaluator.evaluate_call_value(
                "range",
                &positional,
                &named,
                None,
                None,
                &operation_span,
                &mut diagnostics,
                &mut context,
            );
            assert!(diagnostics.is_empty(), "{diagnostics:?}");
            assert!(matches!(
                outcome,
                CallOutcome::Value(IrValue::Range(IrRange {
                    start: Some(2),
                    end: Some(4),
                    ..
                }))
            ));
        }

        for (positional, named, expected) in [
            (
                Vec::new(),
                Vec::new(),
                IrRange {
                    start: None,
                    end: None,
                    span: operation_span,
                },
            ),
            (
                vec![IrValue::Number(2.0)],
                Vec::new(),
                IrRange {
                    start: Some(2),
                    end: None,
                    span: operation_span,
                },
            ),
            (
                Vec::new(),
                vec![named_arg("from", IrValue::Number(2.0))],
                IrRange {
                    start: Some(2),
                    end: None,
                    span: operation_span,
                },
            ),
        ] {
            let mut diagnostics = Vec::new();
            let mut context = EvaluationContext::new();
            let outcome = evaluator.evaluate_call_value(
                "range",
                &positional,
                &named,
                None,
                None,
                &operation_span,
                &mut diagnostics,
                &mut context,
            );
            assert!(diagnostics.is_empty(), "{diagnostics:?}");
            let CallOutcome::Value(IrValue::Range(actual)) = outcome else {
                panic!("expected typed Range")
            };
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn dynamic_range_number_conversion_matches_upstream_edges() {
        for (number, expected) in [
            (f64::NAN, 0),
            (f64::NEG_INFINITY, i32::MIN),
            (f64::INFINITY, i32::MAX),
            ((i32::MIN as f64) - 1.0, i32::MIN),
            ((i32::MAX as f64) + 1.0, i32::MAX),
            (f64::from(i32::MIN), i32::MIN),
            (f64::from(i32::MAX), i32::MAX),
        ] {
            assert_eq!(
                number_to_range_endpoint(&InvocationValue::static_value(IrValue::Number(number))),
                Ok(expected)
            );
        }
        assert!(
            number_to_range_endpoint(&InvocationValue::static_value(IrValue::Boolean(true)))
                .is_err()
        );
        assert_eq!(
            number_to_range_endpoint(&InvocationValue::dynamic_value(IrValue::String(
                "3".to_string()
            ))),
            Ok(3)
        );
    }

    #[test]
    fn range_materialization_handles_signed_and_left_open_bounds_once() {
        let evaluator = Evaluator::new();
        let mut context = EvaluationContext::new();
        for (range, expected) in [
            (
                IrRange {
                    start: Some(-3),
                    end: Some(-1),
                    span: span(0, 5),
                },
                vec![-3.0, -2.0, -1.0],
            ),
            (
                IrRange {
                    start: Some(-3),
                    end: Some(3),
                    span: span(0, 5),
                },
                (-3..=3).map(f64::from).collect(),
            ),
            (
                IrRange {
                    start: None,
                    end: Some(3),
                    span: span(0, 4),
                },
                vec![1.0, 2.0, 3.0],
            ),
        ] {
            let mut diagnostics = Vec::new();
            let Ok(elements) = evaluator.coerce_iterable(
                InvocationValue::static_value(IrValue::Range(range)),
                &span(0, 10),
                &mut diagnostics,
                &mut context,
            ) else {
                panic!("finite ranges materialize");
            };
            assert!(diagnostics.is_empty(), "{diagnostics:?}");
            assert_eq!(
                elements,
                expected
                    .into_iter()
                    .map(IrValue::Number)
                    .collect::<Vec<_>>()
            );
        }

        for range in [
            IrRange {
                start: None,
                end: Some(0),
                span: span(0, 3),
            },
            IrRange {
                start: None,
                end: Some(-2),
                span: span(0, 4),
            },
            IrRange {
                start: Some(4),
                end: Some(2),
                span: span(0, 4),
            },
        ] {
            let mut diagnostics = Vec::new();
            let Ok(elements) = evaluator.coerce_iterable(
                InvocationValue::static_value(IrValue::Range(range)),
                &span(0, 10),
                &mut diagnostics,
                &mut context,
            ) else {
                panic!("descending or below-default ranges are empty");
            };
            assert!(elements.is_empty());
            assert!(diagnostics.is_empty(), "{diagnostics:?}");
        }

        let mut diagnostics = Vec::new();
        let result = evaluator.coerce_iterable(
            InvocationValue::static_value(IrValue::Range(IrRange {
                start: Some(3),
                end: None,
                span: span(10, 13),
            })),
            &span(0, 20),
            &mut diagnostics,
            &mut context,
        );
        assert!(matches!(result, Err(CallOutcome::Failed)));
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].primary, Some(span(10, 13)));
    }

    #[test]
    fn materialization_limit_is_checked_before_range_allocation() {
        let evaluator = Evaluator::with_limits(EvaluationLimits {
            max_materialized_elements: 3,
            max_evaluation_depth: 256,
        });
        let mut context = EvaluationContext::new();

        let mut diagnostics = Vec::new();
        let at_limit = evaluator
            .coerce_iterable(
                InvocationValue::static_value(IrValue::Range(IrRange {
                    start: Some(1),
                    end: Some(3),
                    span: span(10, 15),
                })),
                &span(0, 20),
                &mut diagnostics,
                &mut context,
            )
            .expect("the exact materialization limit is valid");
        assert_eq!(at_limit.len(), 3);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");

        let repeated_independent = evaluator
            .coerce_iterable(
                InvocationValue::static_value(IrValue::Range(IrRange {
                    start: Some(10),
                    end: Some(12),
                    span: span(30, 35),
                })),
                &span(0, 40),
                &mut diagnostics,
                &mut context,
            )
            .expect("per-operation limits reset for an independent range");
        assert_eq!(repeated_independent.len(), 3);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");

        let mut diagnostics = Vec::new();
        let over_limit = evaluator.coerce_iterable(
            InvocationValue::static_value(IrValue::Range(IrRange {
                start: Some(1),
                end: Some(4),
                span: span(50, 55),
            })),
            &span(0, 60),
            &mut diagnostics,
            &mut context,
        );
        assert!(matches!(over_limit, Err(CallOutcome::Failed)));
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E3005");
        assert_eq!(
            diagnostics[0].message,
            "materialized element limit exceeded: requested 4, maximum is 3"
        );
        assert_eq!(diagnostics[0].primary, Some(span(50, 55)));

        let mut diagnostics = Vec::new();
        let huge = evaluator.coerce_iterable(
            InvocationValue::static_value(IrValue::Range(IrRange {
                start: Some(i32::MIN),
                end: Some(i32::MAX),
                span: span(70, 80),
            })),
            &span(0, 90),
            &mut diagnostics,
            &mut context,
        );
        assert!(matches!(huge, Err(CallOutcome::Failed)));
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E3005");
        assert_eq!(diagnostics[0].primary, Some(span(70, 80)));
    }

    #[test]
    fn descending_empty_range_passes_even_when_materialization_limit_is_zero() {
        let evaluator = Evaluator::with_limits(EvaluationLimits {
            max_materialized_elements: 0,
            max_evaluation_depth: 256,
        });
        let mut context = EvaluationContext::new();
        let mut diagnostics = Vec::new();
        let values = evaluator
            .coerce_iterable(
                InvocationValue::static_value(IrValue::Range(IrRange {
                    start: Some(3),
                    end: Some(1),
                    span: span(0, 4),
                })),
                &span(0, 4),
                &mut diagnostics,
                &mut context,
            )
            .expect("descending ranges retain their empty semantics");
        assert!(values.is_empty());
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    fn function_declaration(name: &str, body: Vec<IrNode>, start: usize) -> IrNode {
        IrNode::FunctionDeclaration {
            name: IrValue::Identifier(name.to_string()),
            parameters: Vec::new(),
            body,
            span: span(start, start + name.len()),
        }
    }

    fn function_call(name: &str, start: usize) -> IrNode {
        IrNode::FunctionCall {
            name: name.to_string(),
            positional_args: Vec::new(),
            named_args: Vec::new(),
            ordered_args: None,
            lambda_parameters: None,
            body: None,
            raw_body: None,
            span: span(start, start + name.len()),
        }
    }

    #[test]
    fn nested_function_evaluation_at_depth_limit_passes() {
        let evaluator = Evaluator::with_limits(EvaluationLimits {
            max_materialized_elements: 16,
            max_evaluation_depth: 2,
        });
        let document = doc(vec![
            function_declaration("outer", vec![function_call("inner", 20)], 0),
            function_declaration("inner", vec![text_paragraph("ok")], 10),
            function_call("outer", 30),
        ]);

        let (result, diagnostics) = evaluator.evaluate(&document);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_paragraph_text(&result.nodes, "ok");
    }

    #[test]
    fn direct_and_indirect_recursion_fail_at_depth_limit_and_restore_siblings() {
        let evaluator = Evaluator::with_limits(EvaluationLimits {
            max_materialized_elements: 16,
            max_evaluation_depth: 3,
        });
        let direct = doc(vec![
            function_declaration("loop", vec![function_call("loop", 10)], 0),
            function_call("loop", 20),
            var_declaration("after", IrValue::String("usable".to_string())),
            var_ref("after"),
        ]);
        let (direct_result, direct_diagnostics) = evaluator.evaluate(&direct);
        assert_eq!(direct_diagnostics.len(), 1, "{direct_diagnostics:?}");
        assert_eq!(direct_diagnostics[0].code, "E3005");
        assert_eq!(direct_diagnostics[0].primary, Some(span(10, 14)));
        assert_paragraph_text(&direct_result.nodes, "usable");

        let indirect = doc(vec![
            function_declaration("first", vec![function_call("second", 40)], 30),
            function_declaration("second", vec![function_call("first", 50)], 45),
            function_call("first", 60),
        ]);
        let (indirect_result, indirect_diagnostics) = evaluator.evaluate(&indirect);
        assert!(indirect_result.nodes.is_empty());
        assert_eq!(indirect_diagnostics.len(), 1, "{indirect_diagnostics:?}");
        assert_eq!(indirect_diagnostics[0].code, "E3005");
        assert_eq!(indirect_diagnostics[0].primary, Some(span(40, 46)));
    }

    #[test]
    fn dynamic_range_remains_typed_inside_collection_and_pair_values() {
        let evaluator = Evaluator::new();
        let range = IrValue::Range(IrRange {
            start: Some(2),
            end: Some(4),
            span: span(0, 5),
        });
        let collection = IrValue::Collection(vec![range.clone()]);
        let mut diagnostics = Vec::new();
        let mut context = EvaluationContext::new();
        let outcome = evaluator.evaluate_call_value(
            "getat",
            &[collection, IrValue::Number(1.0)],
            &[],
            None,
            None,
            &span(0, 10),
            &mut diagnostics,
            &mut context,
        );
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert!(matches!(outcome, CallOutcome::Value(value) if value == range));

        let pair = IrValue::Pair(IrPair {
            first: Box::new(range),
            second: Box::new(IrValue::String("value".to_string())),
            span: span(0, 10),
        });
        let outcome = evaluator.evaluate_call_value(
            "first",
            &[pair],
            &[],
            None,
            None,
            &span(0, 10),
            &mut diagnostics,
            &mut context,
        );
        assert!(matches!(outcome, CallOutcome::Value(IrValue::Range(_))));
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn dynamic_range_argument_binding_is_checked_before_evaluation() {
        let evaluator = Evaluator::new();
        for named in [
            vec![named_arg("unknown", IrValue::Number(1.0))],
            vec![
                named_arg("from", IrValue::Number(1.0)),
                named_arg("from", IrValue::Number(2.0)),
            ],
        ] {
            let mut diagnostics = Vec::new();
            let mut context = EvaluationContext::new();
            let outcome = evaluator.evaluate_call_value(
                "range",
                &[],
                &named,
                None,
                None,
                &span(0, 20),
                &mut diagnostics,
                &mut context,
            );
            assert!(matches!(outcome, CallOutcome::Failed));
            assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        }

        let failing = call_value(
            "multiply",
            vec![IrValue::Boolean(true), IrValue::Number(2.0)],
        );
        let mut diagnostics = Vec::new();
        let mut context = EvaluationContext::new();
        let outcome = evaluator.evaluate_call_value(
            "range",
            &[failing],
            &[],
            None,
            None,
            &span(0, 20),
            &mut diagnostics,
            &mut context,
        );
        assert!(matches!(outcome, CallOutcome::Failed));
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
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
    fn collection_second_and_third_share_one_based_iterable_access() {
        let evaluator = Evaluator::new();
        let operation_span = span(0, 20);
        let values = IrValue::Collection(vec![
            IrValue::String("one".to_string()),
            IrValue::Number(2.0),
            IrValue::Boolean(true),
        ]);
        let mut diagnostics = Vec::new();
        let mut context = EvaluationContext::new();

        for (name, expected) in [
            ("second", IrValue::Number(2.0)),
            ("third", IrValue::Boolean(true)),
        ] {
            let outcome = collection_call(
                &evaluator,
                name,
                std::slice::from_ref(&values),
                &[],
                &operation_span,
                &mut diagnostics,
                &mut context,
            );
            assert!(matches!(outcome, CallOutcome::Value(value) if value == expected));
            assert!(diagnostics.is_empty(), "{diagnostics:?}");
        }

        for value in [
            IrValue::Collection(Vec::new()),
            IrValue::Collection(vec![IrValue::Number(1.0)]),
        ] {
            for name in ["second", "third"] {
                let outcome = collection_call(
                    &evaluator,
                    name,
                    std::slice::from_ref(&value),
                    &[],
                    &operation_span,
                    &mut diagnostics,
                    &mut context,
                );
                assert!(matches!(outcome, CallOutcome::Value(IrValue::None)));
                assert!(diagnostics.is_empty(), "{diagnostics:?}");
            }
        }

        let pair = IrValue::Pair(IrPair {
            first: Box::new(IrValue::String("key".to_string())),
            second: Box::new(IrValue::Boolean(true)),
            span: span(21, 31),
        });
        let outcome = collection_call(
            &evaluator,
            "second",
            std::slice::from_ref(&pair),
            &[],
            &operation_span,
            &mut diagnostics,
            &mut context,
        );
        assert!(matches!(
            outcome,
            CallOutcome::Value(IrValue::Boolean(true))
        ));

        let getat = collection_call(
            &evaluator,
            "getat",
            &[values.clone(), IrValue::Number(2.0)],
            &[],
            &operation_span,
            &mut diagnostics,
            &mut context,
        );
        assert!(matches!(
            getat,
            CallOutcome::Value(IrValue::Number(value)) if value == 2.0
        ));
        let getat = collection_call(
            &evaluator,
            "getat",
            &[values.clone(), IrValue::Number(3.0)],
            &[],
            &operation_span,
            &mut diagnostics,
            &mut context,
        );
        assert!(matches!(getat, CallOutcome::Value(IrValue::Boolean(true))));

        let dictionary = IrValue::Dictionary(IrDictionary {
            entries: vec![
                IrPair {
                    first: Box::new(IrValue::String("a".to_string())),
                    second: Box::new(IrValue::Number(1.0)),
                    span: span(32, 36),
                },
                IrPair {
                    first: Box::new(IrValue::String("b".to_string())),
                    second: Box::new(IrValue::Number(2.0)),
                    span: span(37, 41),
                },
            ],
            span: span(32, 41),
        });
        let outcome = collection_call(
            &evaluator,
            "third",
            std::slice::from_ref(&dictionary),
            &[],
            &operation_span,
            &mut diagnostics,
            &mut context,
        );
        assert!(matches!(outcome, CallOutcome::Value(IrValue::None)));
        let outcome = collection_call(
            &evaluator,
            "second",
            std::slice::from_ref(&dictionary),
            &[],
            &operation_span,
            &mut diagnostics,
            &mut context,
        );
        assert!(matches!(
            outcome,
            CallOutcome::Value(IrValue::Pair(pair))
                if matches!(*pair.second, IrValue::Number(value) if value == 2.0)
        ));

        for (range, expected) in [
            (
                IrValue::Range(IrRange {
                    start: Some(-2),
                    end: Some(1),
                    span: span(42, 47),
                }),
                IrValue::Number(-1.0),
            ),
            (
                IrValue::Range(IrRange {
                    start: None,
                    end: Some(3),
                    span: span(48, 51),
                }),
                IrValue::Number(2.0),
            ),
        ] {
            let outcome = collection_call(
                &evaluator,
                "second",
                std::slice::from_ref(&range),
                &[],
                &operation_span,
                &mut diagnostics,
                &mut context,
            );
            assert!(matches!(outcome, CallOutcome::Value(value) if value == expected));
        }
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn collection_distinct_and_groupvalues_are_stable_and_typed() {
        let evaluator = Evaluator::new();
        let operation_span = span(0, 80);
        let pair_one = IrValue::Pair(IrPair {
            first: Box::new(IrValue::String("key".to_string())),
            second: Box::new(IrValue::Number(1.0)),
            span: span(1, 5),
        });
        let pair_two = IrValue::Pair(IrPair {
            first: Box::new(IrValue::String("key".to_string())),
            second: Box::new(IrValue::Number(1.0)),
            span: span(20, 24),
        });
        let dictionary_one = IrValue::Dictionary(IrDictionary {
            entries: vec![IrPair {
                first: Box::new(IrValue::String("a".to_string())),
                second: Box::new(IrValue::Number(1.0)),
                span: span(25, 29),
            }],
            span: span(25, 29),
        });
        let dictionary_two = IrValue::Dictionary(IrDictionary {
            entries: vec![IrPair {
                first: Box::new(IrValue::String("a".to_string())),
                second: Box::new(IrValue::Number(1.0)),
                span: span(30, 34),
            }],
            span: span(30, 34),
        });
        let nested = IrValue::Collection(vec![IrValue::String("nested".to_string())]);
        let input = IrValue::Collection(vec![
            IrValue::Number(1.0),
            IrValue::Number(1.0),
            IrValue::String("1".to_string()),
            IrValue::Boolean(true),
            IrValue::None,
            IrValue::Number(f64::NAN),
            IrValue::Number(f64::NAN),
            IrValue::Number(-0.0),
            IrValue::Number(0.0),
            pair_one.clone(),
            pair_two,
            nested.clone(),
            nested,
            dictionary_one.clone(),
            dictionary_two,
        ]);
        let mut diagnostics = Vec::new();
        let mut context = EvaluationContext::new();
        let empty_distinct = collection_call(
            &evaluator,
            "distinct",
            &[IrValue::Collection(Vec::new())],
            &[],
            &operation_span,
            &mut diagnostics,
            &mut context,
        );
        assert!(matches!(
            empty_distinct,
            CallOutcome::Value(IrValue::Collection(values)) if values.is_empty()
        ));
        let distinct = collection_call(
            &evaluator,
            "distinct",
            std::slice::from_ref(&input),
            &[],
            &operation_span,
            &mut diagnostics,
            &mut context,
        );
        let CallOutcome::Value(IrValue::Collection(distinct_values)) = distinct else {
            panic!("expected distinct collection")
        };
        assert_eq!(distinct_values.len(), 10);
        assert!(matches!(distinct_values[0], IrValue::Number(1.0)));
        assert!(matches!(distinct_values[1], IrValue::String(ref value) if value == "1"));
        assert!(matches!(distinct_values[2], IrValue::Boolean(true)));
        assert!(matches!(distinct_values[3], IrValue::None));
        assert!(matches!(distinct_values[4], IrValue::Number(value) if value.is_nan()));
        assert!(matches!(distinct_values[5], IrValue::Number(value) if value == -0.0));
        assert!(matches!(distinct_values[6], IrValue::Number(value) if value == 0.0));
        assert_eq!(distinct_values[7], pair_one);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");

        let dictionary_input = IrValue::Dictionary(IrDictionary {
            entries: vec![
                IrPair {
                    first: Box::new(IrValue::String("a".to_string())),
                    second: Box::new(IrValue::Number(1.0)),
                    span: span(35, 39),
                },
                IrPair {
                    first: Box::new(IrValue::String("b".to_string())),
                    second: Box::new(IrValue::Number(2.0)),
                    span: span(40, 44),
                },
            ],
            span: span(35, 44),
        });
        let distinct_dictionary = collection_call(
            &evaluator,
            "distinct",
            std::slice::from_ref(&dictionary_input),
            &[],
            &operation_span,
            &mut diagnostics,
            &mut context,
        );
        assert!(
            matches!(distinct_dictionary, CallOutcome::Value(IrValue::Collection(values)) if values.len() == 2 && matches!(&values[0], IrValue::Pair(pair) if matches!(*pair.first, IrValue::String(ref value) if value == "a")))
        );

        let grouped_dictionary = collection_call(
            &evaluator,
            "groupvalues",
            std::slice::from_ref(&dictionary_input),
            &[],
            &operation_span,
            &mut diagnostics,
            &mut context,
        );
        assert!(
            matches!(grouped_dictionary, CallOutcome::Value(IrValue::Collection(groups)) if groups.len() == 2 && groups.iter().all(|group| matches!(group, IrValue::Collection(values) if values.len() == 1)))
        );

        let range = IrValue::Range(IrRange {
            start: Some(1),
            end: Some(3),
            span: span(40, 44),
        });
        let range_distinct = collection_call(
            &evaluator,
            "distinct",
            std::slice::from_ref(&range),
            &[],
            &operation_span,
            &mut diagnostics,
            &mut context,
        );
        assert!(
            matches!(range_distinct, CallOutcome::Value(IrValue::Collection(values)) if values == [IrValue::Number(1.0), IrValue::Number(2.0), IrValue::Number(3.0)])
        );
        let range_groups = collection_call(
            &evaluator,
            "groupvalues",
            std::slice::from_ref(&range),
            &[],
            &operation_span,
            &mut diagnostics,
            &mut context,
        );
        assert!(
            matches!(range_groups, CallOutcome::Value(IrValue::Collection(groups)) if groups.len() == 3 && groups.iter().all(|group| matches!(group, IrValue::Collection(values) if values.len() == 1)))
        );

        let callable = IrValue::Callable(IrCallable {
            parameters: None,
            body: Vec::new(),
            span: span(45, 49),
            capture: None,
        });
        let callable_distinct = collection_call(
            &evaluator,
            "distinct",
            &[IrValue::Collection(vec![
                callable.clone(),
                callable.clone(),
            ])],
            &[],
            &operation_span,
            &mut diagnostics,
            &mut context,
        );
        assert!(
            matches!(callable_distinct, CallOutcome::Value(IrValue::Collection(values)) if values.len() == 1)
        );
        let content_distinct = collection_call(
            &evaluator,
            "distinct",
            &[IrValue::Collection(vec![
                IrValue::Content(Vec::new()),
                IrValue::Content(Vec::new()),
            ])],
            &[],
            &operation_span,
            &mut diagnostics,
            &mut context,
        );
        assert!(
            matches!(content_distinct, CallOutcome::Value(IrValue::Collection(values)) if values.len() == 1)
        );

        let grouped_input = IrValue::Collection(vec![
            IrValue::String("A".to_string()),
            IrValue::String("B".to_string()),
            IrValue::String("A".to_string()),
            IrValue::String("C".to_string()),
            IrValue::String("B".to_string()),
        ]);
        let grouped = collection_call(
            &evaluator,
            "groupvalues",
            std::slice::from_ref(&grouped_input),
            &[],
            &operation_span,
            &mut diagnostics,
            &mut context,
        );
        assert!(
            matches!(grouped, CallOutcome::Value(IrValue::Collection(ref groups)) if groups == &[
                IrValue::Collection(vec![
                    IrValue::String("A".to_string()),
                    IrValue::String("A".to_string()),
                ]),
                IrValue::Collection(vec![
                    IrValue::String("B".to_string()),
                    IrValue::String("B".to_string()),
                ]),
                IrValue::Collection(vec![IrValue::String("C".to_string())]),
            ])
        );
        assert!(diagnostics.is_empty(), "{diagnostics:?}");

        let repeated = collection_call(
            &evaluator,
            "distinct",
            std::slice::from_ref(&grouped_input),
            &[],
            &operation_span,
            &mut diagnostics,
            &mut context,
        );
        let repeated = match repeated {
            CallOutcome::Value(value) => value,
            _ => panic!("expected repeated distinct result"),
        };
        let repeated_again = collection_call(
            &evaluator,
            "distinct",
            std::slice::from_ref(&grouped_input),
            &[],
            &operation_span,
            &mut diagnostics,
            &mut context,
        );
        assert_eq!(
            repeated,
            match repeated_again {
                CallOutcome::Value(value) => value,
                _ => panic!("expected deterministic distinct result"),
            }
        );
        assert!(diagnostics.is_empty(), "{diagnostics:?}");

        let pair_groups = collection_call(
            &evaluator,
            "groupvalues",
            &[IrValue::Pair(IrPair {
                first: Box::new(IrValue::String("same".to_string())),
                second: Box::new(IrValue::String("same".to_string())),
                span: span(81, 86),
            })],
            &[],
            &operation_span,
            &mut diagnostics,
            &mut context,
        );
        assert!(
            matches!(pair_groups, CallOutcome::Value(IrValue::Collection(ref groups)) if groups.len() == 1 && matches!(&groups[0], IrValue::Collection(values) if values.len() == 2))
        );

        let empty_groups = collection_call(
            &evaluator,
            "groupvalues",
            &[IrValue::Collection(Vec::new())],
            &[],
            &operation_span,
            &mut diagnostics,
            &mut context,
        );
        assert!(matches!(
            empty_groups,
            CallOutcome::Value(IrValue::Collection(values)) if values.is_empty()
        ));
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn collection_reversed_uses_the_shared_materialized_sequence() {
        let evaluator = Evaluator::new();
        let operation_span = span(0, 30);
        let mut diagnostics = Vec::new();
        let mut context = EvaluationContext::new();
        let cases = [
            (
                IrValue::Collection(Vec::new()),
                IrValue::Collection(Vec::new()),
            ),
            (
                IrValue::Collection(vec![IrValue::String("one".to_string())]),
                IrValue::Collection(vec![IrValue::String("one".to_string())]),
            ),
            (
                IrValue::Collection(vec![
                    IrValue::Collection(vec![IrValue::Number(1.0)]),
                    IrValue::Number(2.0),
                ]),
                IrValue::Collection(vec![
                    IrValue::Number(2.0),
                    IrValue::Collection(vec![IrValue::Number(1.0)]),
                ]),
            ),
            (
                IrValue::Pair(IrPair {
                    first: Box::new(IrValue::String("a".to_string())),
                    second: Box::new(IrValue::String("b".to_string())),
                    span: span(31, 36),
                }),
                IrValue::Collection(vec![
                    IrValue::String("b".to_string()),
                    IrValue::String("a".to_string()),
                ]),
            ),
            (
                IrValue::Dictionary(IrDictionary {
                    entries: vec![
                        IrPair {
                            first: Box::new(IrValue::String("a".to_string())),
                            second: Box::new(IrValue::Number(1.0)),
                            span: span(53, 57),
                        },
                        IrPair {
                            first: Box::new(IrValue::String("b".to_string())),
                            second: Box::new(IrValue::Number(2.0)),
                            span: span(58, 62),
                        },
                    ],
                    span: span(53, 62),
                }),
                IrValue::Collection(vec![
                    IrValue::Pair(IrPair {
                        first: Box::new(IrValue::String("b".to_string())),
                        second: Box::new(IrValue::Number(2.0)),
                        span: span(58, 62),
                    }),
                    IrValue::Pair(IrPair {
                        first: Box::new(IrValue::String("a".to_string())),
                        second: Box::new(IrValue::Number(1.0)),
                        span: span(53, 57),
                    }),
                ]),
            ),
            (
                IrValue::Range(IrRange {
                    start: Some(-2),
                    end: Some(0),
                    span: span(37, 42),
                }),
                IrValue::Collection(vec![
                    IrValue::Number(0.0),
                    IrValue::Number(-1.0),
                    IrValue::Number(-2.0),
                ]),
            ),
            (
                IrValue::Range(IrRange {
                    start: None,
                    end: Some(3),
                    span: span(43, 46),
                }),
                IrValue::Collection(vec![
                    IrValue::Number(3.0),
                    IrValue::Number(2.0),
                    IrValue::Number(1.0),
                ]),
            ),
            (
                IrValue::Range(IrRange {
                    start: Some(4),
                    end: Some(2),
                    span: span(47, 52),
                }),
                IrValue::Collection(Vec::new()),
            ),
        ];
        for (input, expected) in cases {
            let outcome = collection_call(
                &evaluator,
                "reversed",
                std::slice::from_ref(&input),
                &[],
                &operation_span,
                &mut diagnostics,
                &mut context,
            );
            assert!(matches!(outcome, CallOutcome::Value(value) if value == expected));
            assert!(diagnostics.is_empty(), "{diagnostics:?}");
        }

        let endless_span = span(53, 56);
        let outcome = collection_call(
            &evaluator,
            "reversed",
            &[IrValue::Range(IrRange {
                start: Some(1),
                end: None,
                span: endless_span,
            })],
            &[],
            &operation_span,
            &mut diagnostics,
            &mut context,
        );
        assert!(matches!(outcome, CallOutcome::Failed));
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].primary, Some(endless_span));
    }

    #[test]
    fn collection_sumall_and_average_follow_as_double_and_kotlin_average() {
        let evaluator = Evaluator::new();
        let operation_span = span(0, 30);
        let mut diagnostics = Vec::new();
        let mut context = EvaluationContext::new();
        let mixed = IrValue::Collection(vec![
            IrValue::Number(1.5),
            IrValue::Number(-2.0),
            IrValue::String("3.5".to_string()),
            IrValue::Boolean(true),
            IrValue::None,
            IrValue::String("invalid".to_string()),
        ]);

        let sum = collection_call(
            &evaluator,
            "sumall",
            std::slice::from_ref(&mixed),
            &[],
            &operation_span,
            &mut diagnostics,
            &mut context,
        );
        assert!(matches!(sum, CallOutcome::Value(IrValue::Number(value)) if value == 3.0));
        let average = collection_call(
            &evaluator,
            "average",
            std::slice::from_ref(&mixed),
            &[],
            &operation_span,
            &mut diagnostics,
            &mut context,
        );
        assert!(matches!(average, CallOutcome::Value(IrValue::Number(value)) if value == 0.5));
        assert!(diagnostics.is_empty(), "{diagnostics:?}");

        let empty = IrValue::Collection(Vec::new());
        let sum = collection_call(
            &evaluator,
            "sumall",
            std::slice::from_ref(&empty),
            &[],
            &operation_span,
            &mut diagnostics,
            &mut context,
        );
        assert!(matches!(sum, CallOutcome::Value(IrValue::Number(value)) if value == 0.0));
        let average = collection_call(
            &evaluator,
            "average",
            std::slice::from_ref(&empty),
            &[],
            &operation_span,
            &mut diagnostics,
            &mut context,
        );
        assert!(matches!(average, CallOutcome::Value(IrValue::Number(value)) if value.is_nan()));

        let special = IrValue::Collection(vec![
            IrValue::Number(f64::INFINITY),
            IrValue::Number(f64::NEG_INFINITY),
        ]);
        let sum = collection_call(
            &evaluator,
            "sumall",
            std::slice::from_ref(&special),
            &[],
            &operation_span,
            &mut diagnostics,
            &mut context,
        );
        assert!(matches!(sum, CallOutcome::Value(IrValue::Number(value)) if value.is_nan()));
        let average = collection_call(
            &evaluator,
            "average",
            &[IrValue::Collection(vec![IrValue::Number(f64::INFINITY)])],
            &[],
            &operation_span,
            &mut diagnostics,
            &mut context,
        );
        assert!(
            matches!(average, CallOutcome::Value(IrValue::Number(value)) if value.is_infinite() && value.is_sign_positive())
        );
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
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
    fn component_remains_typed_in_value_context_and_preserves_source_span() {
        let component_span = span(20, 44);
        let component = component_value(component_span);
        let mut diagnostics = Vec::new();
        let mut context = EvaluationContext::new();
        let outcome = Evaluator::new().evaluate_value(&component, &mut diagnostics, &mut context);

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_eq!(outcome, CallOutcome::Value(component.clone()));
        assert_eq!(value_source_span(&component, &span(0, 1)), component_span);
    }

    #[test]
    fn component_survives_variable_and_callable_value_flow() {
        let component = component_value(span(20, 44));
        let mut context = EvaluationContext::new();
        context.set_value("component".to_string(), component.clone());
        context.set_function_binding(
            "make".to_string(),
            LambdaParameters::Implicit,
            vec![var_ref("component")],
            span(50, 54),
            None,
        );
        let evaluator = Evaluator::new();
        let mut diagnostics = Vec::new();

        let variable_reference = call_value("component", Vec::new());
        assert_eq!(
            evaluator.evaluate_value(&variable_reference, &mut diagnostics, &mut context),
            CallOutcome::Value(component.clone())
        );
        assert!(diagnostics.is_empty(), "{diagnostics:?}");

        let outcome = evaluator.evaluate_call_value(
            "make",
            &[],
            &[],
            None,
            None,
            &span(60, 64),
            &mut diagnostics,
            &mut context,
        );
        assert_eq!(outcome, CallOutcome::Value(component));
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn single_callable_component_result_remains_a_value() {
        let component = component_value(span(20, 44));
        let mut context = EvaluationContext::new();
        context.set_value("component".to_string(), component.clone());
        let mut diagnostics = Vec::new();

        let outcome = Evaluator::new().evaluate_callable_body_value(
            &[var_ref("component")],
            &mut diagnostics,
            &mut context,
        );

        assert_eq!(outcome, CallOutcome::Value(component));
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn component_block_materialization_publishes_one_typed_node() {
        let component_span = span(20, 44);
        let component = component_value(component_span);
        let mut diagnostics = Vec::new();
        let result =
            Evaluator::new().materialize_block_value(component, &span(0, 1), &mut diagnostics);

        let Ok([IrNode::Component { component }]) = result.as_deref() else {
            panic!("expected one typed component node, got {result:?}");
        };
        assert_eq!(component.span(), component_span);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn component_variable_at_document_output_boundary_materializes_as_a_node() {
        let component_span = span(20, 44);
        let (nodes, diagnostics) = Evaluator::new().evaluate(&doc(vec![
            var_declaration("component", component_value(component_span)),
            var_ref("component"),
        ]));

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let [IrNode::Component { component }] = nodes.nodes.as_slice() else {
            panic!("expected one component node, got {:?}", nodes.nodes);
        };
        assert_eq!(component.span(), component_span);
    }

    #[test]
    fn component_inline_materialization_fails_with_empty_output() {
        let component_span = span(20, 44);
        let component = component_value(component_span);
        let mut diagnostics = Vec::new();
        let inlines = Evaluator::new().materialize_inline_value(
            Some(component),
            &span(0, 1),
            &mut diagnostics,
        );

        assert!(inlines.is_empty());
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].primary, Some(component_span));
        assert!(diagnostics[0].message.contains("block-only"));
    }

    #[test]
    fn component_and_second_callable_output_preserve_both_nodes() {
        let component_span = span(20, 44);
        let component = component_value(component_span);
        let mut context = EvaluationContext::new();
        context.set_value("component".to_string(), component);
        let mut diagnostics = Vec::new();

        let outcome = Evaluator::new().evaluate_callable_body_value(
            &[var_ref("component"), text_paragraph("later output")],
            &mut diagnostics,
            &mut context,
        );

        let CallOutcome::Value(IrValue::Content(nodes)) = outcome else {
            panic!("expected composed content");
        };
        assert_eq!(nodes.len(), 2);
        assert!(matches!(nodes[0], IrNode::Component { .. }));
        assert!(matches!(nodes[1], IrNode::Paragraph { .. }));
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn component_is_rejected_by_scalar_text_materialization() {
        let component_span = span(20, 44);
        let component = component_value(component_span);
        let mut diagnostics = Vec::new();
        let result = scalar_to_text(&component, span(0, 1), &mut diagnostics);

        assert!(matches!(result, Err(CallOutcome::Failed)));
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].primary, Some(component_span));
        assert!(diagnostics[0].message.contains("scalar text"));
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
    fn foreach_reassignment_updates_existing_caller_variable_but_new_locals_stay_local() {
        let evaluator = Evaluator::new();
        let operation_span = span(0, 40);
        let mut diagnostics = Vec::new();
        let mut context = EvaluationContext::new();
        assert!(matches!(
            evaluator.evaluate_call_value(
                "var",
                &[IrValue::Identifier("total".into()), IrValue::Number(0.0)],
                &[],
                None,
                None,
                &operation_span,
                &mut diagnostics,
                &mut context,
            ),
            CallOutcome::NoValue
        ));

        let increment = call_value(
            "sum",
            vec![call_value("total", Vec::new()), IrValue::Number(1.0)],
        );
        let outcome = evaluator.evaluate_call_value(
            "foreach",
            &[IrValue::Range(IrRange {
                start: Some(1),
                end: Some(2),
                span: span(1, 5),
            })],
            &[],
            Some(CallBody::Block(&[
                var_reassignment("total", increment),
                var_declaration("local", IrValue::String("only here".into())),
                var_ref("total"),
            ])),
            None,
            &operation_span,
            &mut diagnostics,
            &mut context,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert!(matches!(
            outcome,
            CallOutcome::Value(IrValue::Collection(values))
                if values == vec![IrValue::Number(1.0), IrValue::Number(2.0)]
        ));
        assert_eq!(
            context.get("total").map(VariableValue::to_value),
            Some(IrValue::Number(2.0))
        );
        assert!(context.get("local").is_none());
    }

    #[test]
    fn nested_callable_reassignment_reaches_the_outer_caller() {
        let evaluator = Evaluator::new();
        let operation_span = span(0, 40);
        let mut diagnostics = Vec::new();
        let mut context = EvaluationContext::new();
        context.assign_value("total".into(), IrValue::Number(0.0));

        let increment = call_value(
            "sum",
            vec![call_value("total", Vec::new()), IrValue::Number(1.0)],
        );
        let inner = foreach_call(
            IrValue::Range(IrRange {
                start: Some(1),
                end: Some(2),
                span: span(1, 5),
            }),
            None,
            vec![var_reassignment("total", increment), var_ref("total")],
        );
        let capture = context.capture_snapshot();
        context.set_function_binding(
            "bump".into(),
            LambdaParameters::Explicit(Vec::new()),
            vec![inner],
            span(20, 25),
            Some(Box::new(capture)),
        );

        let outcome = evaluator.evaluate_call_value(
            "bump",
            &[],
            &[],
            None,
            None,
            &operation_span,
            &mut diagnostics,
            &mut context,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert!(matches!(
            outcome,
            CallOutcome::Value(IrValue::Collection(_))
        ));
        assert_eq!(
            context.get("total").map(VariableValue::to_value),
            Some(IrValue::Number(2.0))
        );
    }

    #[test]
    fn failed_callable_reassignment_is_atomic_and_keeps_the_inner_span() {
        let evaluator = Evaluator::new();
        let operation_span = span(0, 40);
        let failure_span = span(30, 40);
        let mut diagnostics = Vec::new();
        let mut context = EvaluationContext::new();
        context.set_value("total".into(), IrValue::Number(0.0));
        let failing = IrNode::FunctionCall {
            name: "multiply".to_string(),
            positional_args: vec![IrValue::Boolean(true), IrValue::Number(2.0)],
            named_args: Vec::new(),
            ordered_args: None,
            lambda_parameters: None,
            body: None,
            raw_body: None,
            span: failure_span,
        };
        let outcome = evaluator.evaluate_call_value(
            "foreach",
            &[IrValue::Range(IrRange {
                start: Some(1),
                end: Some(1),
                span: span(1, 5),
            })],
            &[],
            Some(CallBody::Block(&[
                var_reassignment(
                    "total",
                    call_value(
                        "sum",
                        vec![call_value("total", Vec::new()), IrValue::Number(1.0)],
                    ),
                ),
                failing,
            ])),
            None,
            &operation_span,
            &mut diagnostics,
            &mut context,
        );

        assert!(matches!(outcome, CallOutcome::Failed));
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].primary, Some(failure_span));
        assert_eq!(
            context.get("total").map(VariableValue::to_value),
            Some(IrValue::Number(0.0))
        );
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
            ordered_args: None,
            lambda_parameters: Some(vec![lambda_parameter("n", 10)]),
            body: Some(vec![local, var_ref("n")]),
            raw_body: None,
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
            ordered_args: None,
            lambda_parameters: Some(vec![lambda_parameter("value", 20)]),
            body: Some(vec![var_ref("value")]),
            raw_body: None,
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
                ordered_args: None,
                span: whole,
            },
            chain: vec![IrCallSegment {
                name: "b".into(),
                name_span: span(8, 9),
                positional_args: vec![IrValue::Identifier("y".into())],
                named_args: Vec::new(),
                ordered_args: None,
                span: span(8, 13),
            }],
            body: None,
            raw_body: None,
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
            ordered_args: None,
            lambda_parameters: None,
            body: None,
            raw_body: None,
            span: span(7, 12),
        }]);
        let outer = IrNode::FunctionCall {
            name: "multiply".to_string(),
            positional_args: vec![nested_reassignment, IrValue::Number(2.0)],
            named_args: Vec::new(),
            ordered_args: None,
            lambda_parameters: None,
            body: None,
            raw_body: None,
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
        assert_paragraph_text(&nodes, "0");
    }

    #[test]
    fn nested_no_value_named_argument_reports_e3001_without_invoking_outer_call() {
        let nested_reassignment = IrValue::Content(vec![IrNode::FunctionCall {
            name: "x".to_string(),
            positional_args: vec![IrValue::Number(3.0)],
            named_args: Vec::new(),
            ordered_args: None,
            lambda_parameters: None,
            body: None,
            raw_body: None,
            span: span(9, 14),
        }]);
        let outer = IrNode::FunctionCall {
            name: "multiply".to_string(),
            positional_args: vec![IrValue::Number(2.0)],
            named_args: vec![named_arg("by", nested_reassignment)],
            ordered_args: None,
            lambda_parameters: None,
            body: None,
            raw_body: None,
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
        assert_paragraph_text(&nodes, "0");
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
            ordered_args: None,
            lambda_parameters: None,
            body: None,
            raw_body: None,
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
            ordered_args: None,
            lambda_parameters: None,
            body: None,
            raw_body: None,
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
            ordered_args: None,
            lambda_parameters: None,
            body: None,
            raw_body: None,
            span: span(7, 18),
        }]);
        let outer = IrNode::FunctionCall {
            name: "multiply".to_string(),
            positional_args: vec![invalid_var, IrValue::Number(2.0)],
            named_args: Vec::new(),
            ordered_args: None,
            lambda_parameters: None,
            body: None,
            raw_body: None,
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
            ordered_args: None,
            lambda_parameters: None,
            body: None,
            raw_body: None,
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
            ordered_args: None,
            lambda_parameters: None,
            body: None,
            raw_body: None,
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
                ordered_args: None,
                lambda_parameters: None,
                body: None,
                raw_body: None,
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
                            ordered_args: None,
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
                            ordered_args: None,
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
            ordered_args: None,
            lambda_parameters: None,
            body: Some(vec![text_paragraph("content")]),
            raw_body: None,
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
            ordered_args: None,
            lambda_parameters: None,
            body: None,
            raw_body: None,
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
            ordered_args: None,
            lambda_parameters: None,
            body: None,
            raw_body: None,
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
            ordered_args: None,
            lambda_parameters: None,
            body: Some(vec![text_paragraph("from body")]),
            raw_body: None,
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
                    task: Some(arkst_ir::IrTaskStatus::Completed),
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
                header: arkst_ir::IrTableRow {
                    cells: vec![arkst_ir::IrTableCell {
                        content: vec![text_inline("Header")],
                        alignment: arkst_ir::IrTableAlignment::Center,
                        span: span(40, 46),
                    }],
                    span: span(40, 46),
                },
                rows: vec![arkst_ir::IrTableRow {
                    cells: vec![arkst_ir::IrTableCell {
                        content: vec![inline_if_call(
                            "if",
                            IrValue::Boolean(true),
                            vec![text_inline("cell")],
                        )],
                        alignment: arkst_ir::IrTableAlignment::None,
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
        assert_eq!(items[0].task, Some(arkst_ir::IrTaskStatus::Completed));
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
            ordered_args: None,
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
            ordered_args: None,
            lambda_parameters: None,
            body: Some(vec![if_call(
                "if",
                IrValue::Boolean(false),
                vec![text_paragraph("dropped")],
            )]),
            raw_body: None,
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
            ordered_args: None,
            lambda_parameters: None,
            body: Some(vec![text_paragraph("kept")]),
            raw_body: None,
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
            ordered_args: None,
            lambda_parameters: None,
            body: Some(vec![text_paragraph("dropped")]),
            raw_body: None,
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
            ordered_args: None,
            lambda_parameters: None,
            body: Some(vec![text_paragraph("kept")]),
            raw_body: None,
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
                ordered_args: None,
                lambda_parameters: None,
                body: Some(vec![text_paragraph("content")]),
                raw_body: None,
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
            ordered_args: None,
            lambda_parameters: None,
            body: None,
            raw_body: None,
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
            ordered_args: None,
            lambda_parameters: None,
            body: None,
            raw_body: None,
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
            ordered_args: None,
            lambda_parameters: None,
            body: Some(vec![text_paragraph("from indented body")]),
            raw_body: None,
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
                    ordered_args: None,
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
            ordered_args: None,
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
            ordered_args: None,
            lambda_parameters: None,
            body: Some(vec![text_paragraph("body")]),
            raw_body: None,
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
            ordered_args: None,
            lambda_parameters: None,
            body: None,
            raw_body: None,
            span: span(0, 1),
        }
    }

    fn var_declaration_with_body(name: &str, body_nodes: Vec<IrNode>) -> IrNode {
        IrNode::FunctionCall {
            name: "var".to_string(),
            positional_args: vec![IrValue::Identifier(name.to_string())],
            named_args: Vec::new(),
            ordered_args: None,
            lambda_parameters: None,
            body: Some(body_nodes),
            raw_body: None,
            span: span(0, 1),
        }
    }

    fn var_ref(name: &str) -> IrNode {
        IrNode::FunctionCall {
            name: name.to_string(),
            positional_args: Vec::new(),
            named_args: Vec::new(),
            ordered_args: None,
            lambda_parameters: None,
            body: None,
            raw_body: None,
            span: span(0, 1),
        }
    }

    fn var_reassignment(name: &str, value: IrValue) -> IrNode {
        IrNode::FunctionCall {
            name: name.to_string(),
            positional_args: vec![value],
            named_args: Vec::new(),
            ordered_args: None,
            lambda_parameters: None,
            body: None,
            raw_body: None,
            span: span(0, 1),
        }
    }

    fn inline_var_ref(name: &str) -> IrInline {
        IrInline::DirectiveCall {
            name: name.to_string(),
            positional_args: Vec::new(),
            named_args: Vec::new(),
            ordered_args: None,
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
            var_declaration("name", IrValue::String("Arkst".to_string())),
            var_ref("name"),
        ]);
        assert_eq!(nodes.len(), 1);
        let IrNode::Paragraph { content, .. } = &nodes[0] else {
            panic!()
        };
        let IrInline::Text { content: text, .. } = &content[0] else {
            panic!()
        };
        assert_eq!(text, "Arkst");
    }

    #[test]
    fn var_boolean_reference_in_conditional() {
        let (nodes, diagnostics) = evaluate_with_diagnostics(vec![
            var_declaration("enabled", IrValue::Identifier("yes".to_string())),
            IrNode::FunctionCall {
                name: "if".to_string(),
                positional_args: vec![IrValue::Identifier("enabled".to_string())],
                named_args: Vec::new(),
                ordered_args: None,
                lambda_parameters: None,
                body: Some(vec![text_paragraph("visible")]),
                raw_body: None,
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
                ordered_args: None,
                lambda_parameters: None,
                body: Some(vec![text_paragraph("hidden")]),
                raw_body: None,
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
                ordered_args: None,
                lambda_parameters: None,
                body: Some(vec![text_paragraph("visible")]),
                raw_body: None,
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
                ordered_args: None,
                lambda_parameters: None,
                body: Some(vec![var_declaration(
                    "x",
                    IrValue::String("hidden".to_string()),
                )]),
                raw_body: None,
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
            ordered_args: None,
            lambda_parameters: None,
            body: None,
            raw_body: None,
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
            ordered_args: None,
            lambda_parameters: None,
            body: Some(vec![text_paragraph("nested visible")]),
            raw_body: None,
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
                ordered_args: None,
                lambda_parameters: None,
                body: Some(body),
                raw_body: None,
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
            ordered_args: None,
            lambda_parameters: None,
            body: None,
            raw_body: None,
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
            ordered_args: None,
            lambda_parameters: None,
            body: None,
            raw_body: None,
            span: span(0, 17),
        };
        let (result, diagnostics) = Evaluator::new().evaluate(&doc(vec![call]));
        assert!(result.nodes.is_empty());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E3002");
        assert!(diagnostics[0].message.contains("Invalid variable name"));
    }

    #[test]
    fn caller_overlay_failure_does_not_mutate_capture_or_caller_context() {
        let capture = IrCallableCapture {
            variables: vec![IrCapturedVariable {
                name: "value".to_string(),
                value: IrValue::String("definition".to_string()),
            }],
            functions: Vec::new(),
        };
        let callable = IrCallable {
            parameters: None,
            body: vec![IrNode::FunctionCall {
                name: "multiply".to_string(),
                positional_args: vec![IrValue::Boolean(true), IrValue::Number(2.0)],
                named_args: Vec::new(),
                ordered_args: None,
                lambda_parameters: None,
                body: None,
                raw_body: None,
                span: span(10, 20),
            }],
            span: span(0, 20),
            capture: Some(Box::new(capture)),
        };
        let original_capture = callable.capture.clone();
        let mut caller_context = EvaluationContext::new();
        caller_context.set_value("value".to_string(), IrValue::String("caller".to_string()));
        let mut diagnostics = Vec::new();

        let outcome = Evaluator::new().invoke_callable(
            &callable,
            Vec::new(),
            IterationOptions {
                span: span(0, 20),
                allow_destructuring: false,
            },
            &mut diagnostics,
            &mut caller_context,
        );

        assert!(matches!(outcome, CallOutcome::Failed));
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(callable.capture, original_capture);
        assert_eq!(
            caller_context.get("value").map(VariableValue::to_value),
            Some(IrValue::String("caller".to_string()))
        );
    }

    #[test]
    fn issue_167_scalar_conversion_keeps_reason_and_named_provenance() {
        let evaluator = Evaluator::new();
        let call_span = span(0, 40);
        let parameter_span = span(20, 22);
        let candidate_span = span(20, 34);
        let named = named_arg_at(
            "by",
            IrValue::String("not-a-number".to_string()),
            parameter_span,
            candidate_span,
        );
        let mut diagnostics = Vec::new();
        let mut context = EvaluationContext::new();
        let before = context.document_state_snapshot();

        let outcome = evaluator.evaluate_call_value(
            "multiply",
            &[IrValue::Number(2.0)],
            &[named],
            None,
            None,
            &call_span,
            &mut diagnostics,
            &mut context,
        );

        assert!(matches!(outcome, CallOutcome::Failed));
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].primary, Some(candidate_span));
        assert_eq!(diagnostics[0].secondary, vec![parameter_span]);
        assert!(diagnostics[0].message.contains("invalid text"));
        assert!(diagnostics[0].message.contains("Number"));
        assert_eq!(context.document_state_snapshot(), before);

        diagnostics.clear();
        let wrong_domain = named_arg_at(
            "by",
            IrValue::Color(IrColor {
                red: 1,
                green: 2,
                blue: 3,
                alpha: 1.0,
            }),
            parameter_span,
            candidate_span,
        );
        let outcome = evaluator.evaluate_call_value(
            "multiply",
            &[IrValue::Number(2.0)],
            &[wrong_domain],
            None,
            None,
            &call_span,
            &mut diagnostics,
            &mut context,
        );
        assert!(matches!(outcome, CallOutcome::Failed));
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert!(diagnostics[0]
            .message
            .contains("unsupported value category"));
        assert_eq!(diagnostics[0].primary, Some(candidate_span));
    }

    #[test]
    fn issue_167_domain_conversion_distinguishes_invalid_text_and_wrong_domain() {
        let evaluator = Evaluator::new();
        let call_span = span(0, 50);
        let parameter_span = span(7, 12);
        let candidate_span = span(7, 24);
        let mut diagnostics = Vec::new();
        let mut context = EvaluationContext::new();

        let invalid_size = evaluator.evaluate_call_value(
            "container",
            &[],
            &[named_arg_at(
                "width",
                IrValue::String("not-a-size".to_string()),
                parameter_span,
                candidate_span,
            )],
            None,
            None,
            &call_span,
            &mut diagnostics,
            &mut context,
        );
        assert!(matches!(invalid_size, CallOutcome::Failed));
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert!(diagnostics[0].message.contains("invalid text"));
        assert!(diagnostics[0].message.contains("Size"));
        assert_eq!(diagnostics[0].primary, Some(candidate_span));
        assert_eq!(diagnostics[0].secondary, vec![parameter_span]);

        diagnostics.clear();
        let invalid_enum = evaluator.evaluate_call_value(
            "align",
            &[],
            &[named_arg_at(
                "alignment",
                IrValue::String("diagonal".to_string()),
                parameter_span,
                candidate_span,
            )],
            None,
            None,
            &call_span,
            &mut diagnostics,
            &mut context,
        );
        assert!(matches!(invalid_enum, CallOutcome::Failed));
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert!(diagnostics[0].message.contains("invalid text"));
        assert!(diagnostics[0].message.contains("closed enum"));
        assert_eq!(diagnostics[0].primary, Some(candidate_span));

        diagnostics.clear();
        let invalid_caption = evaluator.evaluate_call_value(
            "captionposition",
            &[],
            &[named_arg_at(
                "default",
                IrValue::String("middle".to_string()),
                parameter_span,
                candidate_span,
            )],
            None,
            None,
            &call_span,
            &mut diagnostics,
            &mut context,
        );
        assert!(matches!(invalid_caption, CallOutcome::Failed));
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert!(diagnostics[0].message.contains("invalid text"));
        assert!(diagnostics[0]
            .message
            .contains("allowed values are `top` or `bottom`"));
        assert_eq!(diagnostics[0].primary, Some(candidate_span));

        diagnostics.clear();
        let wrong_domain = evaluator.evaluate_call_value(
            "align",
            &[],
            &[named_arg_at(
                "alignment",
                IrValue::Enum(IrEnumValue::CaptionPosition(IrCaptionPosition::Top)),
                parameter_span,
                candidate_span,
            )],
            None,
            None,
            &call_span,
            &mut diagnostics,
            &mut context,
        );
        assert!(matches!(wrong_domain, CallOutcome::Failed));
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert!(diagnostics[0]
            .message
            .contains("unsupported value category"));
        assert_eq!(diagnostics[0].primary, Some(candidate_span));
    }

    #[test]
    fn issue_167_collection_failure_does_not_publish_partial_keywords() {
        let evaluator = Evaluator::new();
        let call_span = span(0, 40);
        let mut diagnostics = Vec::new();
        let mut context = EvaluationContext::new();
        context.replace_document_keywords(vec!["before".to_string()]);
        let before = context.document_state_snapshot();

        let outcome = evaluator.evaluate_call_value(
            "dockeywords",
            &[IrValue::Collection(vec![
                IrValue::String("first".to_string()),
                IrValue::None,
            ])],
            &[],
            None,
            None,
            &call_span,
            &mut diagnostics,
            &mut context,
        );

        assert!(matches!(outcome, CallOutcome::Failed));
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert!(diagnostics[0]
            .message
            .contains("unsupported value category"));
        assert_eq!(context.document_state_snapshot(), before);
    }

    #[test]
    fn issue_167_function_bindings_roll_back_in_local_and_parent_scopes() {
        let evaluator = Evaluator::new();
        let call_span = span(0, 80);
        let mut diagnostics = Vec::new();
        let mut parent = EvaluationContext::new();
        parent.set_function("parent_fn".to_string(), Vec::new());
        let mut context = parent.child();
        context.set_function("local_fn".to_string(), Vec::new());
        let body = vec![
            IrNode::FunctionDeclaration {
                name: IrValue::Identifier("leaked".to_string()),
                parameters: Vec::new(),
                body: vec![text_paragraph("leaked")],
                span: span(10, 20),
            },
            IrNode::FunctionDeclaration {
                name: IrValue::Identifier("local_fn".to_string()),
                parameters: Vec::new(),
                body: vec![text_paragraph("replacement")],
                span: span(21, 31),
            },
            IrNode::FunctionCall {
                name: "sum".to_string(),
                positional_args: vec![IrValue::Boolean(true), IrValue::Number(2.0)],
                named_args: Vec::new(),
                ordered_args: None,
                lambda_parameters: None,
                body: None,
                raw_body: None,
                span: span(32, 48),
            },
        ];

        let outcome = evaluator.evaluate_call_value(
            "container",
            &[],
            &[],
            Some(CallBody::Block(&body)),
            None,
            &call_span,
            &mut diagnostics,
            &mut context,
        );

        assert!(matches!(outcome, CallOutcome::Failed));
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert!(context.get_function("leaked").is_none());
        assert_eq!(
            context.get_function("local_fn").unwrap().declaration_span,
            SourceSpan::new(SourceId(0), 0, 0)
        );
        assert!(context.get_function("parent_fn").is_some());
        assert!(parent.get_function("leaked").is_none());
        assert!(parent.get_function("parent_fn").is_some());
    }

    #[test]
    fn issue_167_function_binding_undo_reaches_parent_scope() {
        let mut context = EvaluationContext::new().child();
        context.begin_invocation();
        let checkpoint = InvocationCheckpoint::capture();
        context
            .parent
            .as_mut()
            .expect("child has a parent scope")
            .set_function("parent_leaked".to_string(), Vec::new());

        checkpoint.restore(&mut context);
        context.end_invocation();

        assert!(context.get_function("parent_leaked").is_none());
        assert!(context
            .parent
            .as_deref()
            .is_some_and(|parent| parent.get_function("parent_leaked").is_none()));
    }

    #[test]
    fn issue_167_nested_unresolved_callable_does_not_publish_document_state() {
        let evaluator = Evaluator::new();
        let mut diagnostics = Vec::new();
        let mut context = EvaluationContext::new();
        context.set_document_state_value("docname", "before".to_string());
        let declaration = IrNode::FunctionDeclaration {
            name: IrValue::Identifier("mutate_then_unresolved".to_string()),
            parameters: Vec::new(),
            body: vec![
                IrNode::FunctionCall {
                    name: "docname".to_string(),
                    positional_args: vec![IrValue::String("mutated".to_string())],
                    named_args: Vec::new(),
                    ordered_args: None,
                    lambda_parameters: None,
                    body: None,
                    raw_body: None,
                    span: span(10, 20),
                },
                IrNode::FunctionCall {
                    name: "not_implemented".to_string(),
                    positional_args: Vec::new(),
                    named_args: Vec::new(),
                    ordered_args: None,
                    lambda_parameters: None,
                    body: None,
                    raw_body: None,
                    span: span(21, 35),
                },
            ],
            span: span(5, 35),
        };
        evaluator.evaluate_node(&declaration, &mut diagnostics, &mut context);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let before = context.document_state_snapshot();

        let outcome = evaluator.evaluate_call_value(
            "otherwise",
            &[
                call_value("mutate_then_unresolved", Vec::new()),
                IrValue::String("fallback".into()),
            ],
            &[],
            None,
            None,
            &span(0, 50),
            &mut diagnostics,
            &mut context,
        );

        assert!(matches!(
            outcome,
            CallOutcome::Value(IrValue::Content(nodes))
                if matches!(nodes.as_slice(), [IrNode::FunctionCall { name, .. }] if name == "mutate_then_unresolved")
        ));
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_eq!(context.document_state_snapshot(), before);
    }

    #[test]
    fn issue_167_nested_unresolved_transform_restores_live_ephemeral_owner() {
        let evaluator = Evaluator::new();
        let mut diagnostics = Vec::new();
        let mut context = EvaluationContext::new();
        context.assign_value("observed".to_string(), IrValue::Number(-1.0));

        let unresolved_callback = IrNode::FunctionCall {
            name: "not_implemented".to_string(),
            positional_args: Vec::new(),
            named_args: Vec::new(),
            ordered_args: None,
            lambda_parameters: None,
            body: None,
            raw_body: None,
            span: span(30, 45),
        };
        let callback = IrCallable {
            parameters: Some(vec![lambda_parameter("item", 10)]),
            body: vec![
                var_reassignment("x", IrValue::Number(1.0)),
                IrNode::FunctionCall {
                    name: "ifpresent".to_string(),
                    positional_args: vec![call_value("item", Vec::new())],
                    named_args: Vec::new(),
                    ordered_args: None,
                    lambda_parameters: None,
                    body: Some(vec![unresolved_callback]),
                    raw_body: None,
                    span: span(20, 45),
                },
            ],
            span: span(10, 45),
            capture: None,
        };
        let transform = IrValue::Content(vec![foreach_call(
            IrValue::Collection(vec![IrValue::None, IrValue::String("trigger".to_string())]),
            Some(callback.parameters.clone().unwrap()),
            callback.body.clone(),
        )]);
        let otherwise = IrNode::FunctionCall {
            name: "otherwise".to_string(),
            positional_args: vec![transform, IrValue::String("fallback".to_string())],
            named_args: Vec::new(),
            ordered_args: None,
            lambda_parameters: None,
            body: None,
            raw_body: None,
            span: span(0, 50),
        };
        context.set_function_binding(
            "outer".to_string(),
            LambdaParameters::Explicit(Vec::new()),
            vec![
                var_declaration("x", IrValue::Number(0.0)),
                otherwise,
                var_reassignment("observed", call_value("x", Vec::new())),
            ],
            span(0, 60),
            None,
        );

        let outcome = evaluator.evaluate_call_value(
            "outer",
            &[],
            &[],
            None,
            None,
            &span(0, 60),
            &mut diagnostics,
            &mut context,
        );

        assert!(matches!(outcome, CallOutcome::Value(_)));
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_eq!(
            context.get("observed").map(VariableValue::to_value),
            Some(IrValue::Number(0.0))
        );
    }

    #[test]
    fn issue_167_discarded_callable_locals_do_not_grow_transaction_metadata() {
        let evaluator = Evaluator::new();
        let span = span(0, 20);
        let callable = IrCallable {
            parameters: Some(vec![lambda_parameter("value", 4)]),
            body: vec![var_ref("value")],
            span,
            capture: None,
        };
        let elements = (0..4096)
            .map(|value| IrValue::Number(value as f64))
            .collect::<Vec<_>>();
        let mut diagnostics = Vec::new();
        let mut context = EvaluationContext::new();

        context.begin_invocation();
        let checkpoint = InvocationCheckpoint::capture();
        let outcome = evaluator.map_callable_values(
            &elements,
            &callable,
            IterationOptions {
                span,
                allow_destructuring: false,
            },
            &mut diagnostics,
            &mut context,
        );

        let CallOutcome::Value(IrValue::Collection(values)) = outcome else {
            panic!("expected a materialized callback result: {outcome:?}");
        };
        assert_eq!(values.len(), elements.len());
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_eq!(context.transaction.borrow().pending_entry_count(), 0);
        checkpoint.commit(&context);
        context.end_invocation();
    }

    #[test]
    fn issue_169_repeated_chained_extensions_do_not_retain_dead_undo_metadata() {
        let evaluator = Evaluator::new();
        let mut diagnostics = Vec::new();
        let mut context = EvaluationContext::new();
        let declaration = IrNode::FunctionDeclaration {
            name: IrValue::Identifier("repeatable".to_string()),
            parameters: vec![lambda_parameter("value", 4)],
            body: vec![var_ref("value")],
            span: span(0, 20),
        };
        evaluator.evaluate_node(&declaration, &mut diagnostics, &mut context);

        let extension_parameters = vec![lambda_parameter("value", 24)];
        let extension_body = vec![IrNode::FunctionCall {
            name: "super".to_string(),
            positional_args: Vec::new(),
            named_args: Vec::new(),
            ordered_args: None,
            lambda_parameters: None,
            body: None,
            raw_body: None,
            span: span(30, 36),
        }];
        for _ in 0..3 {
            let outcome = evaluator.evaluate_call_value(
                "extend",
                &[IrValue::Identifier("repeatable".to_string())],
                &[],
                Some(CallBody::Block(&extension_body)),
                Some(&extension_parameters),
                &span(20, 40),
                &mut diagnostics,
                &mut context,
            );
            assert!(matches!(outcome, CallOutcome::NoValue));
        }
        assert_eq!(context.extension_target_count(), 2);

        for _ in 0..4096 {
            let outcome = evaluator.evaluate_call_value(
                "repeatable",
                &[IrValue::String("value".to_string())],
                &[],
                None,
                None,
                &span(40, 55),
                &mut diagnostics,
                &mut context,
            );
            assert!(matches!(
                outcome,
                CallOutcome::Value(IrValue::String(value)) if value == "value"
            ));
        }
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_eq!(context.extension_target_count(), 2);
        assert_eq!(context.transaction.borrow().pending_entry_count(), 0);
    }

    #[test]
    fn issue_169_replaced_function_prunes_old_extension_overlays() {
        let evaluator = Evaluator::new();
        let mut diagnostics = Vec::new();
        let mut context = EvaluationContext::new();
        let extension_parameters = vec![lambda_parameter("value", 24)];
        let extension_body = vec![IrNode::FunctionCall {
            name: "super".to_string(),
            positional_args: Vec::new(),
            named_args: Vec::new(),
            ordered_args: None,
            lambda_parameters: None,
            body: None,
            raw_body: None,
            span: span(30, 36),
        }];

        for _ in 0..1024 {
            context.set_function_binding(
                "repeatable".to_string(),
                LambdaParameters::Explicit(vec![lambda_parameter("value", 4)]),
                vec![var_ref("value")],
                span(0, 20),
                None,
            );
            assert_eq!(context.extension_target_count(), 0);
            for _ in 0..3 {
                let outcome = evaluator.evaluate_call_value(
                    "extend",
                    &[IrValue::Identifier("repeatable".to_string())],
                    &[],
                    Some(CallBody::Block(&extension_body)),
                    Some(&extension_parameters),
                    &span(20, 40),
                    &mut diagnostics,
                    &mut context,
                );
                assert!(matches!(outcome, CallOutcome::NoValue));
            }
            assert_eq!(context.extension_target_count(), 2);
            assert_eq!(context.transaction.borrow().pending_entry_count(), 0);
        }

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn issue_167_repeated_document_state_appends_use_bounded_undo_metadata() {
        let evaluator = Evaluator::new();
        let span = span(0, 20);
        let callable = IrCallable {
            parameters: Some(vec![lambda_parameter("value", 4)]),
            body: vec![
                IrNode::FunctionCall {
                    name: "docauthor".to_string(),
                    positional_args: vec![IrValue::String("author".to_string())],
                    named_args: Vec::new(),
                    ordered_args: None,
                    lambda_parameters: None,
                    body: None,
                    raw_body: None,
                    span,
                },
                var_ref("value"),
            ],
            span,
            capture: None,
        };
        let elements = (0..4096)
            .map(|value| IrValue::Number(value as f64))
            .collect::<Vec<_>>();
        let mut diagnostics = Vec::new();
        let mut context = EvaluationContext::new();

        context.begin_invocation();
        let checkpoint = InvocationCheckpoint::capture();
        let copy_work_before = context.transaction.borrow().document_state_copy_work();
        let outcome = evaluator.map_callable_values(
            &elements,
            &callable,
            IterationOptions {
                span,
                allow_destructuring: false,
            },
            &mut diagnostics,
            &mut context,
        );

        let CallOutcome::Value(IrValue::Collection(values)) = outcome else {
            panic!("expected a materialized callback result: {outcome:?}");
        };
        assert_eq!(values.len(), elements.len());
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_eq!(context.document_authors_snapshot().len(), elements.len());
        assert_eq!(
            context.transaction.borrow().document_state_copy_work() - copy_work_before,
            0,
            "author appends must journal only the pre-append length"
        );
        assert_eq!(context.transaction.borrow().pending_entry_count(), 1);
        checkpoint.commit(&context);
        context.end_invocation();
    }

    fn mutating_typed_candidate(variable_name: &str) -> IrValue {
        IrValue::Content(vec![let_call(
            Some(IrValue::None),
            None,
            Some(vec![
                IrNode::FunctionCall {
                    name: "docname".to_string(),
                    positional_args: vec![IrValue::String("mutated".to_string())],
                    named_args: Vec::new(),
                    ordered_args: None,
                    lambda_parameters: None,
                    body: None,
                    raw_body: None,
                    span: span(10, 20),
                },
                var_ref(variable_name),
            ]),
        )])
    }

    #[test]
    fn issue_167_document_state_conversion_rolls_back_nested_candidate_mutation() {
        let evaluator = Evaluator::new();
        let call_span = span(0, 70);
        let parameter_span = span(32, 39);
        let candidate_span = span(32, 48);
        let mut diagnostics = Vec::new();
        let mut context = EvaluationContext::new();
        context.set_document_state_value("docname", "before".to_string());
        context.set_value(
            "caption_value".to_string(),
            IrValue::Enum(IrEnumValue::CaptionPosition(IrCaptionPosition::Top)),
        );
        let before = context.document_state_snapshot();

        let outcome = evaluator.evaluate_call_value(
            "captionposition",
            &[mutating_typed_candidate("caption_value")],
            &[named_arg_at(
                "figures",
                IrValue::String("diagonal".to_string()),
                parameter_span,
                candidate_span,
            )],
            None,
            None,
            &call_span,
            &mut diagnostics,
            &mut context,
        );

        assert!(matches!(outcome, CallOutcome::Failed));
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert!(diagnostics[0].message.contains("invalid text"));
        assert_eq!(diagnostics[0].primary, Some(candidate_span));
        assert_eq!(diagnostics[0].secondary, vec![parameter_span]);
        assert_eq!(context.document_state_snapshot(), before);
    }

    #[test]
    fn issue_167_component_conversion_is_validate_then_commit() {
        let evaluator = Evaluator::new();
        let call_span = span(0, 70);
        let parameter_span = span(40, 46);
        let candidate_span = span(40, 58);
        let mut diagnostics = Vec::new();
        let mut context = EvaluationContext::new();
        context.set_document_state_value("docname", "before".to_string());
        context.set_value(
            "container_size".to_string(),
            IrValue::Size(IrSize {
                value: 12.0,
                unit: IrSizeUnit::Px,
            }),
        );
        let before = context.document_state_snapshot();

        let outcome = evaluator.evaluate_call_value(
            "container",
            &[],
            &[
                named_arg_at(
                    "width",
                    mutating_typed_candidate("container_size"),
                    span(7, 12),
                    span(7, 32),
                ),
                named_arg_at(
                    "height",
                    IrValue::String("not-a-size".to_string()),
                    parameter_span,
                    candidate_span,
                ),
            ],
            None,
            None,
            &call_span,
            &mut diagnostics,
            &mut context,
        );

        assert!(matches!(outcome, CallOutcome::Failed));
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert!(diagnostics[0].message.contains("invalid text"));
        assert_eq!(diagnostics[0].primary, Some(candidate_span));
        assert_eq!(context.document_state_snapshot(), before);
    }

    #[test]
    fn issue_167_body_failure_rolls_back_state_after_lazy_body_selection() {
        let evaluator = Evaluator::new();
        let call_span = span(0, 60);
        let mut diagnostics = Vec::new();
        let mut context = EvaluationContext::new();
        context.set_document_state_value("docname", "before".to_string());
        let before = context.document_state_snapshot();
        let body = vec![
            IrNode::FunctionCall {
                name: "docname".to_string(),
                positional_args: vec![IrValue::String("body mutation".to_string())],
                named_args: Vec::new(),
                ordered_args: None,
                lambda_parameters: None,
                body: None,
                raw_body: None,
                span: span(10, 25),
            },
            IrNode::FunctionCall {
                name: "sum".to_string(),
                positional_args: vec![IrValue::Boolean(true), IrValue::Number(2.0)],
                named_args: Vec::new(),
                ordered_args: None,
                lambda_parameters: None,
                body: None,
                raw_body: None,
                span: span(26, 42),
            },
        ];

        let outcome = evaluator.evaluate_call_value(
            "container",
            &[],
            &[],
            Some(CallBody::Block(&body)),
            None,
            &call_span,
            &mut diagnostics,
            &mut context,
        );

        assert!(matches!(outcome, CallOutcome::Failed));
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(context.document_state_snapshot(), before);
    }

    #[test]
    fn issue_167_nested_failure_emits_only_the_causal_diagnostic() {
        let evaluator = Evaluator::new();
        let nested_span = span(22, 36);
        let nested = IrValue::Content(vec![IrNode::FunctionCall {
            name: "multiply".to_string(),
            positional_args: vec![IrValue::Boolean(true), IrValue::Number(2.0)],
            named_args: Vec::new(),
            ordered_args: None,
            lambda_parameters: None,
            body: None,
            raw_body: None,
            span: nested_span,
        }]);
        let mut diagnostics = Vec::new();
        let mut context = EvaluationContext::new();

        let outcome = evaluator.evaluate_call_value(
            "sum",
            &[nested, IrValue::Number(1.0)],
            &[],
            None,
            None,
            &span(0, 45),
            &mut diagnostics,
            &mut context,
        );

        assert!(matches!(outcome, CallOutcome::Failed));
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].primary, Some(nested_span));
        assert!(diagnostics[0]
            .message
            .contains("unsupported value category"));
    }

    #[test]
    fn issue_167_utf8_candidate_provenance_uses_byte_offsets() {
        let source = "文 .container width=not-a-size";
        let candidate_start = source.find("width").expect("candidate in source");
        let candidate_end = source.len();
        let name_end = candidate_start + "width".len();
        let source_id = SourceId(167);
        let call_span = SourceSpan::new(source_id, 0, source.len());
        let candidate_span = SourceSpan::new(source_id, candidate_start, candidate_end);
        let parameter_span = SourceSpan::new(source_id, candidate_start, name_end);
        let mut diagnostics = Vec::new();
        let mut context = EvaluationContext::new();

        let outcome = Evaluator::new().evaluate_call_value(
            "container",
            &[],
            &[named_arg_at(
                "width",
                IrValue::String("not-a-size".to_string()),
                parameter_span,
                candidate_span,
            )],
            None,
            None,
            &call_span,
            &mut diagnostics,
            &mut context,
        );

        assert!(matches!(outcome, CallOutcome::Failed));
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].primary, Some(candidate_span));
        assert_eq!(diagnostics[0].secondary, vec![parameter_span]);
        assert_eq!(
            candidate_start, 15,
            "the leading character is three UTF-8 bytes"
        );
    }

    #[test]
    fn issue_167_crlf_candidate_provenance_keeps_original_coordinates() {
        let source = ".container width=not-a-size\r\n  body";
        let candidate_start = source.find("width").expect("candidate in source");
        let candidate_end = source.find("\r\n").expect("CRLF in source");
        let name_end = candidate_start + "width".len();
        let source_id = SourceId(168);
        let call_span = SourceSpan::new(source_id, 0, source.len());
        let candidate_span = SourceSpan::new(source_id, candidate_start, candidate_end);
        let parameter_span = SourceSpan::new(source_id, candidate_start, name_end);
        let mut diagnostics = Vec::new();
        let mut context = EvaluationContext::new();

        let outcome = Evaluator::new().evaluate_call_value(
            "container",
            &[],
            &[named_arg_at(
                "width",
                IrValue::String("not-a-size".to_string()),
                parameter_span,
                candidate_span,
            )],
            None,
            None,
            &call_span,
            &mut diagnostics,
            &mut context,
        );

        assert!(matches!(outcome, CallOutcome::Failed));
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].primary, Some(candidate_span));
        assert_eq!(diagnostics[0].secondary, vec![parameter_span]);
        assert_eq!((candidate_start, candidate_end), (11, 27));
    }
}
