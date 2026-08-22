//! AST-to-IR conversion — translates the parsed Markdown AST into the Scribium IR.
//!
//! This is the bridge between `scribium-markdown` (parser output) and `ir`
//! (evaluator input / lowering input). It preserves semantic values and call
//! structure for the evaluator without rewriting source text.

use crate::diagnostics::{Diagnostic, Severity};
use crate::ir::{
    IrCallSegment, IrDocument, IrInline, IrListItem, IrMetadata, IrNamedArg, IrNode, IrParameter,
    IrRange, IrTableAlignment, IrTableCell, IrTableRow, IrTaskStatus,
};
use crate::SourceMode;
use scribium_markdown::ast::{
    Block, CallSegment, Document, Inline, TableAlignment, TaskStatus, Value,
};
use scribium_project::{ProjectMetadata, ResourceReference};
use scribium_source::{ByteSpan, SourceId, SourceSpan};

/// Convert a parsed Markdown `Document` into an `IrDocument`.
///
/// `source_id` identifies the source file in the span model.
/// `project_metadata` provides project-level defaults that can be overridden
/// by document front matter.
#[cfg(test)]
fn ast_to_ir(
    doc: &Document,
    source_id: SourceId,
    project_metadata: &ProjectMetadata,
) -> IrDocument {
    ast_to_ir_with_diagnostics(doc, source_id, project_metadata).0
}

/// Convert frontend AST to IR while reporting syntax that the current IR and
/// Typst backend cannot represent without changing its meaning.
pub fn ast_to_ir_with_diagnostics(
    doc: &Document,
    source_id: SourceId,
    project_metadata: &ProjectMetadata,
) -> (IrDocument, Vec<Diagnostic>) {
    ast_to_ir_with_diagnostics_for_mode(doc, source_id, project_metadata, SourceMode::Markdown)
}

pub(crate) fn ast_to_ir_with_diagnostics_for_mode(
    doc: &Document,
    source_id: SourceId,
    project_metadata: &ProjectMetadata,
    source_mode: SourceMode,
) -> (IrDocument, Vec<Diagnostic>) {
    let mut diagnostics = Vec::new();
    let nodes: Vec<IrNode> = doc
        .nodes
        .iter()
        .filter_map(|b| block_to_ir(b, source_id, &mut diagnostics, source_mode, false))
        .collect();

    // Start with project metadata as defaults
    let mut title = project_metadata.title().map(|s| s.to_string());
    let mut author = project_metadata.author().map(|s| s.to_string());
    let mut date = project_metadata.date().map(|s| s.to_string());
    let mut raw = project_metadata.fields().to_vec();

    // Override with front matter if present
    if let Some(ref fm) = doc.front_matter {
        for (key, val) in &fm.fields {
            match key.as_str() {
                "title" => title = Some(val.clone()),
                "author" => author = Some(val.clone()),
                "date" => date = Some(val.clone()),
                _ => {
                    // Remove existing custom field with same key
                    raw.retain(|(k, _)| k != key);
                    raw.push((key.clone(), val.clone()));
                }
            }
        }
    }

    // Sort raw metadata by key for deterministic ordering
    raw.sort_by(|a, b| a.0.cmp(&b.0));

    (
        IrDocument {
            nodes,
            metadata: IrMetadata {
                title,
                author,
                date,
                raw,
                document_state: Default::default(),
            },
        },
        diagnostics,
    )
}

fn block_to_ir(
    block: &Block,
    source_id: SourceId,
    diagnostics: &mut Vec<Diagnostic>,
    source_mode: SourceMode,
    function_body: bool,
) -> Option<IrNode> {
    match block {
        Block::Heading {
            level,
            content,
            span,
        } => Some(IrNode::Heading {
            level: *level,
            content: inlines_to_ir(content, source_id, diagnostics, source_mode, function_body),
            span: byte_to_source_span(span, source_id),
        }),
        Block::Paragraph { content, span } => {
            let inlines =
                inlines_to_ir(content, source_id, diagnostics, source_mode, function_body);
            if inlines.is_empty() {
                return None;
            }
            Some(IrNode::Paragraph {
                content: inlines,
                span: byte_to_source_span(span, source_id),
            })
        }
        Block::Blockquote { content, span } => Some(IrNode::Blockquote {
            content: content
                .iter()
                .filter_map(|child| {
                    block_to_ir(child, source_id, diagnostics, source_mode, function_body)
                })
                .collect(),
            span: byte_to_source_span(span, source_id),
        }),
        Block::UnorderedList { items, span } => {
            let ir_items: Vec<IrListItem> = items
                .iter()
                .map(|item| {
                    list_item_to_ir(item, source_id, diagnostics, source_mode, function_body)
                })
                .collect();
            Some(IrNode::UnorderedList {
                items: ir_items,
                span: byte_to_source_span(span, source_id),
            })
        }
        Block::OrderedList { items, start, span } => {
            let ir_items: Vec<IrListItem> = items
                .iter()
                .map(|item| {
                    list_item_to_ir(item, source_id, diagnostics, source_mode, function_body)
                })
                .collect();
            Some(IrNode::OrderedList {
                items: ir_items,
                start: *start,
                span: byte_to_source_span(span, source_id),
            })
        }
        Block::CodeBlock {
            language,
            info,
            source,
            span,
        } => Some(IrNode::CodeBlock {
            language: language.clone(),
            info: info.clone(),
            source: source.clone(),
            span: byte_to_source_span(span, source_id),
        }),
        Block::ThematicBreak { span } => Some(IrNode::ThematicBreak {
            span: byte_to_source_span(span, source_id),
        }),
        Block::DirectiveCall {
            name,
            name_span,
            head_span,
            positional_args,
            named_args,
            chain,
            body,
            lambda_header,
            span,
        } => {
            let ir_positional: Vec<_> = positional_args
                .iter()
                .map(|v| value_to_ir(v, source_id, diagnostics, source_mode))
                .collect();
            let ir_named: Vec<_> = named_args
                .iter()
                .map(|arg| IrNamedArg {
                    name: arg.name.clone(),
                    name_span: byte_to_source_span(&arg.name_span, source_id),
                    value: value_to_ir(&arg.value, source_id, diagnostics, source_mode),
                    span: byte_to_source_span(&arg.span, source_id),
                })
                .collect();
            let ir_body = body.as_ref().map(|blocks| {
                blocks
                    .iter()
                    .filter_map(|b| block_to_ir(b, source_id, diagnostics, source_mode, true))
                    .collect::<Vec<_>>()
            });
            if name == "function" {
                if positional_args.len() != 1 || !named_args.is_empty() || !chain.is_empty() {
                    diagnostics.push(invalid_function_declaration(
                        "`.function` requires exactly one positional name argument, no named arguments, and no chain",
                        span,
                        source_id,
                    ));
                    return None;
                }
                let declaration_name = ir_positional.first().cloned()?;
                let parameters = lambda_header
                    .as_ref()
                    .map(|header| lambda_parameters_to_ir(header, source_id))
                    .unwrap_or_default();
                return Some(IrNode::FunctionDeclaration {
                    name: declaration_name,
                    parameters,
                    body: ir_body.unwrap_or_default(),
                    span: byte_to_source_span(span, source_id),
                });
            }
            if chain.is_empty() {
                Some(IrNode::FunctionCall {
                    name: name.clone(),
                    positional_args: ir_positional,
                    named_args: ir_named,
                    lambda_parameters: lambda_header
                        .as_ref()
                        .map(|header| lambda_parameters_to_ir(header, source_id)),
                    body: ir_body,
                    span: byte_to_source_span(span, source_id),
                })
            } else {
                Some(IrNode::ChainedFunctionCall {
                    head: IrCallSegment {
                        name: name.clone(),
                        name_span: byte_to_source_span(name_span, source_id),
                        positional_args: ir_positional,
                        named_args: ir_named,
                        span: byte_to_source_span(head_span, source_id),
                    },
                    chain: chain
                        .iter()
                        .map(|segment| {
                            call_segment_to_ir(segment, source_id, diagnostics, source_mode)
                        })
                        .collect(),
                    body: ir_body,
                    span: byte_to_source_span(span, source_id),
                })
            }
        }
        Block::Metadata { .. } => {
            // Metadata is handled via front_matter on the Document; skip as a block node.
            None
        }
        Block::Table { header, rows, span } => Some(IrNode::Table {
            header: table_row_to_ir(header, source_id, diagnostics, source_mode, function_body),
            rows: rows
                .iter()
                .map(|row| table_row_to_ir(row, source_id, diagnostics, source_mode, function_body))
                .collect(),
            span: byte_to_source_span(span, source_id),
        }),
        Block::RawHtml { source, span } if function_body => Some(IrNode::RawHtml {
            source: source.clone(),
            span: byte_to_source_span(span, source_id),
        }),
        Block::RawHtml { source, span } => {
            if source_mode == SourceMode::Markdown && is_comment_only_raw_html_block(source) {
                return None;
            }
            push_unsupported(diagnostics, "raw HTML block", span, source_id);
            None
        }
        Block::Unsupported { kind, span } => {
            if !kind.starts_with("malformed Quarkdown") {
                push_unsupported(diagnostics, kind, span, source_id);
            }
            None
        }
    }
}

fn lambda_parameters_to_ir(
    header: &scribium_markdown::ast::LambdaHeader,
    source_id: SourceId,
) -> Vec<IrParameter> {
    header
        .parameters
        .iter()
        .map(|parameter| IrParameter {
            name: parameter.name.clone(),
            name_span: byte_to_source_span(&parameter.name_span, source_id),
            span: byte_to_source_span(&parameter.span, source_id),
            optional: parameter.optional,
        })
        .collect()
}

fn list_item_to_ir(
    item: &scribium_markdown::ast::ListItem,
    source_id: SourceId,
    diagnostics: &mut Vec<Diagnostic>,
    source_mode: SourceMode,
    function_body: bool,
) -> IrListItem {
    IrListItem {
        nodes: item
            .content
            .iter()
            .filter_map(|child| {
                block_to_ir(child, source_id, diagnostics, source_mode, function_body)
            })
            .collect(),
        task: item.task.map(task_status_to_ir),
        span: byte_to_source_span(&item.span, source_id),
    }
}

fn task_status_to_ir(status: TaskStatus) -> IrTaskStatus {
    match status {
        TaskStatus::Active => IrTaskStatus::Active,
        TaskStatus::Completed => IrTaskStatus::Completed,
    }
}

fn table_row_to_ir(
    row: &scribium_markdown::ast::TableRow,
    source_id: SourceId,
    diagnostics: &mut Vec<Diagnostic>,
    source_mode: SourceMode,
    function_body: bool,
) -> IrTableRow {
    IrTableRow {
        cells: row
            .cells
            .iter()
            .map(|cell| IrTableCell {
                content: inlines_to_ir(
                    &cell.content,
                    source_id,
                    diagnostics,
                    source_mode,
                    function_body,
                ),
                alignment: table_alignment_to_ir(cell.alignment),
                span: byte_to_source_span(&cell.span, source_id),
            })
            .collect(),
        span: byte_to_source_span(&row.span, source_id),
    }
}

fn table_alignment_to_ir(alignment: TableAlignment) -> IrTableAlignment {
    match alignment {
        TableAlignment::Left => IrTableAlignment::Left,
        TableAlignment::Center => IrTableAlignment::Center,
        TableAlignment::Right => IrTableAlignment::Right,
        TableAlignment::None => IrTableAlignment::None,
    }
}

fn call_segment_to_ir(
    segment: &CallSegment,
    source_id: SourceId,
    diagnostics: &mut Vec<Diagnostic>,
    source_mode: SourceMode,
) -> IrCallSegment {
    IrCallSegment {
        name: segment.name.clone(),
        name_span: byte_to_source_span(&segment.name_span, source_id),
        positional_args: segment
            .positional_args
            .iter()
            .map(|value| value_to_ir(value, source_id, diagnostics, source_mode))
            .collect(),
        named_args: segment
            .named_args
            .iter()
            .map(|arg| IrNamedArg {
                name: arg.name.clone(),
                name_span: byte_to_source_span(&arg.name_span, source_id),
                value: value_to_ir(&arg.value, source_id, diagnostics, source_mode),
                span: byte_to_source_span(&arg.span, source_id),
            })
            .collect(),
        span: byte_to_source_span(&segment.span, source_id),
    }
}

fn inlines_to_ir(
    inlines: &[Inline],
    source_id: SourceId,
    diagnostics: &mut Vec<Diagnostic>,
    source_mode: SourceMode,
    function_body: bool,
) -> Vec<IrInline> {
    let mut output = Vec::new();
    let html_pairs = (source_mode == SourceMode::Markdown).then(|| raw_html_pairs(inlines));
    let mut index = 0;
    while index < inlines.len() {
        if source_mode == SourceMode::Markdown {
            if let Inline::RawHtml { content, span } = &inlines[index] {
                match classify_raw_html(content) {
                    Some(RawHtmlToken::Comment) => {
                        index += 1;
                        continue;
                    }
                    Some(RawHtmlToken::HardBreak) => {
                        output.push(IrInline::HardBreak {
                            span: byte_to_source_span(span, source_id),
                        });
                        index += 1;
                        continue;
                    }
                    Some(RawHtmlToken::Open(tag)) => {
                        if let Some((_, close_index, _)) = html_pairs.as_ref().and_then(|pairs| {
                            pairs.iter().find(|(open_index, _, pair_tag)| {
                                *open_index == index && *pair_tag == tag
                            })
                        }) {
                            let content = inlines_to_ir(
                                &inlines[index + 1..*close_index],
                                source_id,
                                diagnostics,
                                source_mode,
                                function_body,
                            );
                            let span =
                                ByteSpan::new(span.start, inline_span_end(&inlines[*close_index]));
                            output.push(match tag {
                                RawHtmlTag::Em => IrInline::Emphasis {
                                    content,
                                    span: byte_to_source_span(&span, source_id),
                                },
                                RawHtmlTag::Strong => IrInline::Strong {
                                    content,
                                    span: byte_to_source_span(&span, source_id),
                                },
                                RawHtmlTag::Del | RawHtmlTag::S => IrInline::Strikethrough {
                                    content,
                                    span: byte_to_source_span(&span, source_id),
                                },
                            });
                            index = *close_index + 1;
                            continue;
                        }
                    }
                    Some(RawHtmlToken::Close(_)) | None => {}
                }
            }
        }

        if let Some(inline) = inline_to_ir(
            &inlines[index],
            source_id,
            diagnostics,
            source_mode,
            function_body,
        ) {
            output.push(inline);
        }
        index += 1;
    }
    output
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RawHtmlTag {
    Em,
    Strong,
    Del,
    S,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RawHtmlToken {
    Comment,
    Open(RawHtmlTag),
    Close(RawHtmlTag),
    HardBreak,
}

/// Return the end of the first parser-recognized HTML comment token.
///
/// CommonMark 0.31.2 has two short comment tokens (`<!-->` and `<!--->`) in
/// addition to the ordinary `<!-- ... -->` form. The ordinary form ends at
/// the first `-->`; the body is otherwise intentionally opaque here.
fn comment_token_end(content: &str) -> Option<usize> {
    if content.starts_with("<!-->") {
        return Some("<!-->".len());
    }
    if content.starts_with("<!--->") {
        return Some("<!--->".len());
    }
    if !content.starts_with("<!--") {
        return None;
    }
    content["<!--".len()..]
        .find("-->")
        .map(|offset| "<!--".len() + offset + "-->".len())
}

fn classify_comment_raw_html(content: &str) -> Option<RawHtmlToken> {
    (comment_token_end(content) == Some(content.len())).then_some(RawHtmlToken::Comment)
}

/// A Rushdown HTML block includes the source line's permitted indentation and
/// line ending. Accept only that parser-owned boundary material around one
/// complete comment; visible same-line content remains unsupported.
fn is_comment_only_raw_html_block(source: &str) -> bool {
    let bytes = source.as_bytes();
    let mut content_start = 0;
    while content_start < bytes.len() && content_start < 3 && bytes[content_start] == b' ' {
        content_start += 1;
    }

    let content = &source[content_start..];
    let Some(comment_end) = comment_token_end(content) else {
        return false;
    };
    if !is_comment_raw_html_boundary(&content[comment_end..]) {
        return false;
    }
    classify_comment_raw_html(&content[..comment_end]).is_some()
}

fn is_comment_raw_html_boundary(suffix: &str) -> bool {
    let bytes = suffix.as_bytes();
    let mut boundary_end = 0;
    while boundary_end < bytes.len() && matches!(bytes[boundary_end], b' ' | b'\t') {
        boundary_end += 1;
    }
    matches!(&bytes[boundary_end..], [] | [b'\n'] | [b'\r', b'\n'])
}

/// Classify only exact, attribute-free tags whose existing IR already has the
/// same document meaning. This is a whitelist, not an HTML parser: all other
/// opaque Rushdown segments remain unsupported and source-backed.
fn classify_raw_html(content: &str) -> Option<RawHtmlToken> {
    if let Some(comment) = classify_comment_raw_html(content) {
        return Some(comment);
    }
    for (opening, tag) in [
        ("<em>", RawHtmlTag::Em),
        ("<strong>", RawHtmlTag::Strong),
        ("<del>", RawHtmlTag::Del),
        ("<s>", RawHtmlTag::S),
    ] {
        if content.eq_ignore_ascii_case(opening) {
            return Some(RawHtmlToken::Open(tag));
        }
    }
    for (closing, tag) in [
        ("</em>", RawHtmlTag::Em),
        ("</strong>", RawHtmlTag::Strong),
        ("</del>", RawHtmlTag::Del),
        ("</s>", RawHtmlTag::S),
    ] {
        if content.eq_ignore_ascii_case(closing) {
            return Some(RawHtmlToken::Close(tag));
        }
    }
    if ["<br>", "<br/>", "<br />"]
        .iter()
        .any(|break_tag| content.eq_ignore_ascii_case(break_tag))
    {
        return Some(RawHtmlToken::HardBreak);
    }
    None
}

type RawHtmlPair = (usize, usize, RawHtmlTag);

struct RawHtmlFrame {
    index: usize,
    tag: RawHtmlTag,
    invalid: bool,
    pairs: Vec<RawHtmlPair>,
}

fn raw_html_pairs(inlines: &[Inline]) -> Vec<RawHtmlPair> {
    let mut stack: Vec<RawHtmlFrame> = Vec::new();
    let mut pairs = Vec::new();
    for (index, inline) in inlines.iter().enumerate() {
        let Inline::RawHtml { content, .. } = inline else {
            continue;
        };
        match classify_raw_html(content) {
            Some(RawHtmlToken::Open(tag)) => stack.push(RawHtmlFrame {
                index,
                tag,
                invalid: false,
                pairs: Vec::new(),
            }),
            Some(RawHtmlToken::Close(tag)) => {
                let Some(frame) = stack.pop() else {
                    continue;
                };
                if frame.tag != tag {
                    for parent in &mut stack {
                        parent.invalid = true;
                    }
                    continue;
                }
                if frame.invalid {
                    continue;
                }
                let mut completed = frame.pairs;
                completed.push((frame.index, index, frame.tag));
                if let Some(parent) = stack.last_mut() {
                    parent.pairs.extend(completed);
                } else {
                    pairs.extend(completed);
                }
            }
            Some(RawHtmlToken::HardBreak) => {}
            Some(RawHtmlToken::Comment) => {}
            None => {
                for frame in &mut stack {
                    frame.invalid = true;
                }
            }
        }
    }
    pairs
}

fn inline_to_ir(
    inline: &Inline,
    source_id: SourceId,
    diagnostics: &mut Vec<Diagnostic>,
    source_mode: SourceMode,
    function_body: bool,
) -> Option<IrInline> {
    match inline {
        Inline::Text { content, span } => Some(IrInline::Text {
            content: content.clone(),
            span: byte_to_source_span(span, source_id),
        }),
        Inline::Emphasis { content, span } => {
            let children =
                inlines_to_ir(content, source_id, diagnostics, source_mode, function_body);
            Some(IrInline::Emphasis {
                content: children,
                span: byte_to_source_span(span, source_id),
            })
        }
        Inline::Strong { content, span } => {
            let children =
                inlines_to_ir(content, source_id, diagnostics, source_mode, function_body);
            Some(IrInline::Strong {
                content: children,
                span: byte_to_source_span(span, source_id),
            })
        }
        Inline::Strikethrough { content, span } => Some(IrInline::Strikethrough {
            content: inlines_to_ir(content, source_id, diagnostics, source_mode, function_body),
            span: byte_to_source_span(span, source_id),
        }),
        Inline::DirectiveCall {
            name,
            name_span,
            head_span,
            positional_args,
            named_args,
            chain,
            body,
            span,
        } => {
            let ir_positional: Vec<_> = positional_args
                .iter()
                .map(|v| value_to_ir(v, source_id, diagnostics, source_mode))
                .collect();
            let ir_named: Vec<_> = named_args
                .iter()
                .map(|arg| IrNamedArg {
                    name: arg.name.clone(),
                    name_span: byte_to_source_span(&arg.name_span, source_id),
                    value: value_to_ir(&arg.value, source_id, diagnostics, source_mode),
                    span: byte_to_source_span(&arg.span, source_id),
                })
                .collect();
            let ir_body = body
                .as_ref()
                .map(|b| inlines_to_ir(b, source_id, diagnostics, source_mode, function_body));
            if chain.is_empty() {
                Some(IrInline::DirectiveCall {
                    name: name.clone(),
                    positional_args: ir_positional,
                    named_args: ir_named,
                    body: ir_body,
                    span: byte_to_source_span(span, source_id),
                })
            } else {
                Some(IrInline::ChainedDirectiveCall {
                    head: IrCallSegment {
                        name: name.clone(),
                        name_span: byte_to_source_span(name_span, source_id),
                        positional_args: ir_positional,
                        named_args: ir_named,
                        span: byte_to_source_span(head_span, source_id),
                    },
                    chain: chain
                        .iter()
                        .map(|segment| {
                            call_segment_to_ir(segment, source_id, diagnostics, source_mode)
                        })
                        .collect(),
                    body: ir_body,
                    span: byte_to_source_span(span, source_id),
                })
            }
        }
        Inline::HardBreak { span } => Some(IrInline::HardBreak {
            span: byte_to_source_span(span, source_id),
        }),
        Inline::SoftBreak { span } => Some(IrInline::SoftBreak {
            span: byte_to_source_span(span, source_id),
        }),
        Inline::Link {
            content,
            destination,
            title,
            span,
        } => Some(IrInline::Link {
            content: inlines_to_ir(content, source_id, diagnostics, source_mode, function_body),
            destination: destination.clone(),
            title: title.clone(),
            span: byte_to_source_span(span, source_id),
        }),
        Inline::Image {
            content,
            destination,
            title,
            span,
        } => {
            if source_mode == SourceMode::Markdown {
                if let Some(diagnostic) = image_resource_diagnostic(destination, span, source_id) {
                    diagnostics.push(diagnostic);
                }
            } else {
                push_unsupported(diagnostics, "image", span, source_id);
            }
            Some(IrInline::Image {
                content: inlines_to_ir(content, source_id, diagnostics, source_mode, function_body),
                destination: destination.clone(),
                title: title.clone(),
                span: byte_to_source_span(span, source_id),
            })
        }
        Inline::RawHtml { content, span } if function_body => Some(IrInline::RawHtml {
            content: content.clone(),
            span: byte_to_source_span(span, source_id),
        }),
        Inline::RawHtml { span, .. } => {
            push_unsupported(diagnostics, "raw HTML inline", span, source_id);
            None
        }
        Inline::Unsupported { kind, span } => {
            push_unsupported(diagnostics, kind, span, source_id);
            None
        }
        Inline::Code { content, span } => Some(IrInline::Code {
            content: content.clone(),
            span: byte_to_source_span(span, source_id),
        }),
    }
}

fn value_to_ir(
    value: &Value,
    source_id: SourceId,
    diagnostics: &mut Vec<Diagnostic>,
    source_mode: SourceMode,
) -> crate::ir::IrValue {
    match value {
        Value::String(s) => crate::ir::IrValue::String(s.clone()),
        Value::Number(n) => crate::ir::IrValue::Number(*n),
        Value::Boolean(b) => crate::ir::IrValue::Boolean(*b),
        Value::Identifier(id) => crate::ir::IrValue::Identifier(id.clone()),
        Value::Range(range) => {
            // The literal grammar intentionally accepts only non-negative
            // decimal endpoints. Quarkdown's v2.5.1 Range factory stores
            // endpoints through `toIntOrNull`: an out-of-domain literal does
            // not wrap, but becomes an open endpoint. Preserve that observed
            // behavior with an explicit checked conversion.
            crate::ir::IrValue::Range(IrRange {
                start: range.start.and_then(|value| i32::try_from(value).ok()),
                end: range.end.and_then(|value| i32::try_from(value).ok()),
                span: byte_to_source_span(&range.span, source_id),
            })
        }
        Value::Lambda {
            parameters,
            body,
            span,
        } => crate::ir::IrValue::Callable(crate::ir::IrCallable {
            parameters: parameters
                .as_ref()
                .map(|header| lambda_parameters_to_ir(header, source_id)),
            body: inline_lambda_body_to_ir(body, source_id, diagnostics, source_mode, *span),
            span: byte_to_source_span(span, source_id),
            capture: None,
        }),
        Value::Content(inlines) => {
            if let [Inline::DirectiveCall {
                name,
                name_span,
                head_span,
                positional_args,
                named_args,
                chain,
                body,
                span,
            }] = inlines.as_slice()
            {
                let ir_positional: Vec<_> = positional_args
                    .iter()
                    .map(|v| value_to_ir(v, source_id, diagnostics, source_mode))
                    .collect();
                let ir_named: Vec<_> = named_args
                    .iter()
                    .map(|arg| IrNamedArg {
                        name: arg.name.clone(),
                        name_span: byte_to_source_span(&arg.name_span, source_id),
                        value: value_to_ir(&arg.value, source_id, diagnostics, source_mode),
                        span: byte_to_source_span(&arg.span, source_id),
                    })
                    .collect();
                let ir_body = body.as_ref().map(|b| {
                    vec![IrNode::Paragraph {
                        content: inlines_to_ir(b, source_id, diagnostics, source_mode, false),
                        span: byte_to_source_span(span, source_id),
                    }]
                });
                if chain.is_empty() {
                    crate::ir::IrValue::Content(vec![IrNode::FunctionCall {
                        name: name.clone(),
                        positional_args: ir_positional,
                        named_args: ir_named,
                        lambda_parameters: None,
                        body: ir_body,
                        span: byte_to_source_span(span, source_id),
                    }])
                } else {
                    crate::ir::IrValue::Content(vec![IrNode::ChainedFunctionCall {
                        head: IrCallSegment {
                            name: name.clone(),
                            name_span: byte_to_source_span(name_span, source_id),
                            positional_args: ir_positional,
                            named_args: ir_named,
                            span: byte_to_source_span(head_span, source_id),
                        },
                        chain: chain
                            .iter()
                            .map(|segment| {
                                call_segment_to_ir(segment, source_id, diagnostics, source_mode)
                            })
                            .collect(),
                        body: ir_body,
                        span: byte_to_source_span(span, source_id),
                    }])
                }
            } else {
                let start = inlines.first().map(inline_span_start);
                let end = inlines.last().map(inline_span_end);
                let span = scribium_source::ByteSpan::new(start.unwrap_or(0), end.unwrap_or(0));
                crate::ir::IrValue::Content(vec![IrNode::Paragraph {
                    content: inlines_to_ir(inlines, source_id, diagnostics, source_mode, false),
                    span: byte_to_source_span(&span, source_id),
                }])
            }
        }
    }
}

fn inline_lambda_body_to_ir(
    body: &[Inline],
    source_id: SourceId,
    diagnostics: &mut Vec<Diagnostic>,
    source_mode: SourceMode,
    fallback_span: ByteSpan,
) -> Vec<IrNode> {
    match body {
        [] => Vec::new(),
        [Inline::DirectiveCall {
            name,
            name_span,
            head_span,
            positional_args,
            named_args,
            chain,
            body,
            span,
        }] => {
            let positional_args = positional_args
                .iter()
                .map(|value| value_to_ir(value, source_id, diagnostics, source_mode))
                .collect();
            let named_args = named_args
                .iter()
                .map(|argument| IrNamedArg {
                    name: argument.name.clone(),
                    name_span: byte_to_source_span(&argument.name_span, source_id),
                    value: value_to_ir(&argument.value, source_id, diagnostics, source_mode),
                    span: byte_to_source_span(&argument.span, source_id),
                })
                .collect();
            let body = body.as_ref().map(|inlines| {
                vec![IrNode::Paragraph {
                    content: inlines_to_ir(inlines, source_id, diagnostics, source_mode, false),
                    span: byte_to_source_span(span, source_id),
                }]
            });
            if chain.is_empty() {
                vec![IrNode::FunctionCall {
                    name: name.clone(),
                    positional_args,
                    named_args,
                    lambda_parameters: None,
                    body,
                    span: byte_to_source_span(span, source_id),
                }]
            } else {
                vec![IrNode::ChainedFunctionCall {
                    head: IrCallSegment {
                        name: name.clone(),
                        name_span: byte_to_source_span(name_span, source_id),
                        positional_args,
                        named_args,
                        span: byte_to_source_span(head_span, source_id),
                    },
                    chain: chain
                        .iter()
                        .map(|segment| {
                            call_segment_to_ir(segment, source_id, diagnostics, source_mode)
                        })
                        .collect(),
                    body,
                    span: byte_to_source_span(span, source_id),
                }]
            }
        }
        _ => vec![IrNode::Paragraph {
            content: inlines_to_ir(body, source_id, diagnostics, source_mode, false),
            span: byte_to_source_span(&fallback_span, source_id),
        }],
    }
}

fn push_unsupported(
    diagnostics: &mut Vec<Diagnostic>,
    feature: &str,
    span: &scribium_source::ByteSpan,
    source_id: SourceId,
) {
    diagnostics.push(Diagnostic {
        code: "E8001".to_string(),
        severity: Severity::Error,
        message: format!(
            "Markdown syntax `{feature}` was parsed and preserved by the frontend but is not supported by the current document output path"
        ),
        primary: Some(byte_to_source_span(span, source_id)),
        secondary: Vec::new(),
        hints: vec![
            "The source semantics were not coerced into a different Markdown node.".to_string(),
        ],
    });
}

fn image_resource_diagnostic(
    destination: &str,
    span: &scribium_source::ByteSpan,
    source_id: SourceId,
) -> Option<Diagnostic> {
    let reference = ResourceReference::classify(destination);
    let message = match reference {
        ResourceReference::LocalPath(_) => return None,
        ResourceReference::AbsolutePath(_) => {
            "absolute or platform-specific Markdown image resource paths are not allowed"
                .to_string()
        }
        ResourceReference::Uri { scheme, .. } => {
            format!("unsupported Markdown image resource scheme: {scheme}")
        }
    };
    Some(Diagnostic {
        code: "E8001".to_string(),
        severity: Severity::Error,
        message,
        primary: Some(byte_to_source_span(span, source_id)),
        secondary: Vec::new(),
        hints: vec![
            "Use a project-relative local image path; network and absolute resources are not fetched by Scribium."
                .to_string(),
        ],
    })
}

fn invalid_function_declaration(
    message: &str,
    span: &scribium_source::ByteSpan,
    source_id: SourceId,
) -> Diagnostic {
    Diagnostic {
        code: "E3003".to_string(),
        severity: Severity::Error,
        message: message.to_string(),
        primary: Some(byte_to_source_span(span, source_id)),
        secondary: Vec::new(),
        hints: vec!["A user-defined function needs one positional name argument.".to_string()],
    }
}

fn inline_span_start(inline: &Inline) -> usize {
    match inline {
        Inline::Text { span, .. }
        | Inline::Emphasis { span, .. }
        | Inline::Strong { span, .. }
        | Inline::DirectiveCall { span, .. }
        | Inline::Link { span, .. }
        | Inline::Image { span, .. }
        | Inline::RawHtml { span, .. }
        | Inline::Strikethrough { span, .. }
        | Inline::Unsupported { span, .. }
        | Inline::Code { span, .. }
        | Inline::HardBreak { span }
        | Inline::SoftBreak { span } => span.start,
    }
}

fn inline_span_end(inline: &Inline) -> usize {
    match inline {
        Inline::Text { span, .. }
        | Inline::Emphasis { span, .. }
        | Inline::Strong { span, .. }
        | Inline::DirectiveCall { span, .. }
        | Inline::Link { span, .. }
        | Inline::Image { span, .. }
        | Inline::RawHtml { span, .. }
        | Inline::Strikethrough { span, .. }
        | Inline::Unsupported { span, .. }
        | Inline::Code { span, .. }
        | Inline::HardBreak { span }
        | Inline::SoftBreak { span } => span.end,
    }
}

fn byte_to_source_span(byte_span: &scribium_source::ByteSpan, source_id: SourceId) -> SourceSpan {
    SourceSpan::new(source_id, byte_span.start, byte_span.end)
}
#[cfg(test)]
mod tests {
    use super::*;
    use scribium_markdown::ast::{FrontMatter, TableAlignment, TableCell, TableRow};
    use scribium_source::{ByteSpan, SourceId};

    fn source_id() -> SourceId {
        SourceId(42)
    }

    fn bs(start: usize, end: usize) -> ByteSpan {
        ByteSpan::new(start, end)
    }

    fn empty_project_metadata() -> ProjectMetadata {
        ProjectMetadata::default()
    }

    #[test]
    fn convert_empty_document() {
        let doc = Document {
            nodes: vec![],
            front_matter: None,
            line_count: 0,
        };
        let ir = ast_to_ir(&doc, source_id(), &empty_project_metadata());
        assert!(ir.nodes.is_empty());
        assert_eq!(ir.metadata.title, None);
    }

    #[test]
    fn convert_heading() {
        let doc = Document {
            nodes: vec![Block::Heading {
                level: 1,
                content: vec![Inline::Text {
                    content: "Title".into(),
                    span: bs(2, 7),
                }],
                span: bs(0, 7),
            }],
            front_matter: None,
            line_count: 1,
        };
        let ir = ast_to_ir(&doc, source_id(), &empty_project_metadata());
        assert_eq!(ir.nodes.len(), 1);
        match &ir.nodes[0] {
            IrNode::Heading {
                level,
                content,
                span,
            } => {
                assert_eq!(*level, 1);
                assert_eq!(content.len(), 1);
                assert_eq!(span.source_id, SourceId(42));
                assert_eq!(span.start, 0);
                assert_eq!(span.end, 7);
            }
            _ => panic!("expected Heading"),
        }
    }

    #[test]
    fn preserve_call_chain_segments_and_provenance_in_ir() {
        let source = ".a {x}::b {y}\n";
        let document = scribium_markdown::parse_qd(source);
        let (ir, diagnostics) = ast_to_ir_with_diagnostics_for_mode(
            &document,
            source_id(),
            &empty_project_metadata(),
            SourceMode::Quarkdown,
        );
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let IrNode::ChainedFunctionCall {
            head, chain, span, ..
        } = &ir.nodes[0]
        else {
            panic!("expected parser-preserved chain, got {:?}", ir.nodes)
        };
        assert_eq!(head.name, "a");
        assert_eq!(chain.len(), 1);
        assert_eq!(chain[0].name, "b");
        assert_eq!(*span, SourceSpan::new(source_id(), 0, source.len() - 1));
        assert_eq!(head.name_span, SourceSpan::new(source_id(), 0, 2));
        assert_eq!(head.span, SourceSpan::new(source_id(), 0, 6));
        assert_eq!(chain[0].name_span, SourceSpan::new(source_id(), 8, 9));
        assert_eq!(chain[0].span, SourceSpan::new(source_id(), 8, 13));
        assert_eq!(&source[head.span.start..head.span.end], ".a {x}");
        assert_eq!(&source[chain[0].span.start..chain[0].span.end], "b {y}");
        assert_eq!(&source[span.start..span.end], ".a {x}::b {y}");
    }

    #[test]
    fn let_lambda_metadata_survives_ast_to_ir_with_original_spans() {
        let source = ".let {값}\r\n\tname:\r\n\t안녕, .name!\r\n";
        let document = scribium_markdown::parse_qd(source);
        let (ir, diagnostics) = ast_to_ir_with_diagnostics_for_mode(
            &document,
            source_id(),
            &empty_project_metadata(),
            SourceMode::Quarkdown,
        );
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let IrNode::FunctionCall {
            name,
            lambda_parameters: Some(parameters),
            body: Some(body),
            ..
        } = &ir.nodes[0]
        else {
            panic!("expected let call with explicit lambda metadata")
        };
        assert_eq!(name, "let");
        assert_eq!(parameters.len(), 1);
        assert_eq!(parameters[0].name, "name");
        assert_eq!(
            parameters[0].name_span,
            SourceSpan::new(source_id(), 13, 17)
        );
        assert_eq!(parameters[0].span, SourceSpan::new(source_id(), 13, 17));
        let IrNode::Paragraph { span, .. } = &body[0] else {
            panic!("expected stripped let header")
        };
        assert_eq!(&source[span.start..span.end], "안녕, .name!");
        assert!(!source[span.start..span.end].contains("name:"));
    }

    #[test]
    fn let_implicit_lambda_metadata_is_absent_in_ir() {
        let source = ".let {값}\n    .1\n";
        let document = scribium_markdown::parse_qd(source);
        let (ir, diagnostics) = ast_to_ir_with_diagnostics_for_mode(
            &document,
            source_id(),
            &empty_project_metadata(),
            SourceMode::Quarkdown,
        );
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let IrNode::FunctionCall {
            name,
            lambda_parameters,
            body: Some(body),
            ..
        } = &ir.nodes[0]
        else {
            panic!("expected implicit let call")
        };
        assert_eq!(name, "let");
        assert!(lambda_parameters.is_none());
        assert!(matches!(
            body.as_slice(),
            [IrNode::FunctionCall { name, .. }] if name == "1"
        ));
    }

    #[test]
    fn range_survives_ast_to_ir_as_a_typed_source_backed_value() {
        for (source, expected_start, expected_end, expected_span) in [
            (".foreach {2..4}\n    .1\n", Some(2), Some(4), (10, 14)),
            (".foreach {2..}\n    .1\n", Some(2), None, (10, 13)),
            (".foreach {..4}\n    .1\n", None, Some(4), (10, 13)),
            (".foreach {..}\n    .1\n", None, None, (10, 12)),
        ] {
            let document = scribium_markdown::parse_qd(source);
            let (ir, diagnostics) = ast_to_ir_with_diagnostics_for_mode(
                &document,
                source_id(),
                &empty_project_metadata(),
                SourceMode::Quarkdown,
            );
            assert!(diagnostics.is_empty(), "{diagnostics:?}");
            let IrNode::FunctionCall {
                positional_args,
                body: Some(_),
                ..
            } = &ir.nodes[0]
            else {
                panic!("expected foreach call")
            };
            assert!(matches!(
                positional_args.as_slice(),
                [crate::ir::IrValue::Range(crate::ir::IrRange {
                    start,
                    end,
                    span,
                })] if *start == expected_start
                    && *end == expected_end
                    && *span == SourceSpan::new(source_id(), expected_span.0, expected_span.1)
            ));
        }
    }

    #[test]
    fn literal_range_endpoint_conversion_is_checked_at_the_signed_boundary() {
        for (source, expected_start, expected_end) in [
            (
                ".foreach {2147483647..2147483647}\n    .1\n",
                Some(i32::MAX),
                Some(i32::MAX),
            ),
            (".foreach {2147483648..2147483648}\n    .1\n", None, None),
            (".foreach {4294967296..4294967296}\n    .1\n", None, None),
        ] {
            let document = scribium_markdown::parse_qd(source);
            let (ir, diagnostics) = ast_to_ir_with_diagnostics_for_mode(
                &document,
                source_id(),
                &empty_project_metadata(),
                SourceMode::Quarkdown,
            );
            assert!(diagnostics.is_empty(), "{diagnostics:?}");
            let IrNode::FunctionCall {
                positional_args, ..
            } = &ir.nodes[0]
            else {
                panic!("expected foreach call")
            };
            assert!(matches!(
                positional_args.as_slice(),
                [crate::ir::IrValue::Range(crate::ir::IrRange { start, end, .. })]
                    if *start == expected_start && *end == expected_end
            ));
        }
    }

    #[test]
    fn range_span_rebasing_is_preserved_from_markdown_ast_to_ir() {
        fn ast_range_span(document: &Document) -> ByteSpan {
            for block in &document.nodes {
                match block {
                    Block::DirectiveCall {
                        positional_args, ..
                    } => {
                        if let Some(Value::Range(range)) = positional_args.first() {
                            return range.span;
                        }
                    }
                    Block::Paragraph { content, .. } => {
                        for inline in content {
                            if let Inline::DirectiveCall {
                                positional_args, ..
                            } = inline
                            {
                                if let Some(Value::Range(range)) = positional_args.first() {
                                    return range.span;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            panic!("expected Range in Markdown AST")
        }

        fn ir_range_span(document: &crate::ir::IrDocument) -> SourceSpan {
            for node in &document.nodes {
                match node {
                    IrNode::FunctionCall {
                        positional_args, ..
                    } => {
                        if let Some(crate::ir::IrValue::Range(range)) = positional_args.first() {
                            return range.span;
                        }
                    }
                    IrNode::Paragraph { content, .. } => {
                        for inline in content {
                            if let IrInline::DirectiveCall {
                                positional_args, ..
                            } = inline
                            {
                                if let Some(crate::ir::IrValue::Range(range)) =
                                    positional_args.first()
                                {
                                    return range.span;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            panic!("expected Range in IR")
        }

        for source in [
            "앞 문장\r\n.foreach {2..4}\r\n    .1\r\n",
            "---\r\ntitle: 값\r\n---\r\n\r\n앞 문장\r\n.foreach {2..4}\r\n    .1\r\n",
            "앞 .foo {2..4} 뒤\n",
            "앞 H{.foo {2..4}}O\n",
        ] {
            let document = scribium_markdown::parse_qd(source);
            let ast_span = ast_range_span(&document);
            assert_eq!(&source[ast_span.start..ast_span.end], "2..4");
            let (ir, diagnostics) = ast_to_ir_with_diagnostics_for_mode(
                &document,
                source_id(),
                &empty_project_metadata(),
                SourceMode::Quarkdown,
            );
            assert!(diagnostics.is_empty(), "{diagnostics:?}");
            let ir_span = ir_range_span(&ir);
            assert_eq!(
                ir_span,
                SourceSpan::new(source_id(), ast_span.start, ast_span.end)
            );
            assert_eq!(&source[ir_span.start..ir_span.end], "2..4");
        }
    }

    #[test]
    fn convert_paragraph_with_emphasis() {
        let doc = Document {
            nodes: vec![Block::Paragraph {
                content: vec![
                    Inline::Text {
                        content: "Hello ".into(),
                        span: bs(0, 6),
                    },
                    Inline::Emphasis {
                        content: vec![Inline::Text {
                            content: "world".into(),
                            span: bs(7, 12),
                        }],
                        span: bs(6, 13),
                    },
                ],
                span: bs(0, 13),
            }],
            front_matter: None,
            line_count: 1,
        };
        let ir = ast_to_ir(&doc, source_id(), &empty_project_metadata());
        assert_eq!(ir.nodes.len(), 1);
        match &ir.nodes[0] {
            IrNode::Paragraph { content, .. } => {
                assert_eq!(content.len(), 2);
                assert!(matches!(content[0], IrInline::Text { .. }));
                assert!(matches!(content[1], IrInline::Emphasis { .. }));
            }
            _ => panic!("expected Paragraph"),
        }
    }

    #[test]
    fn convert_paragraph_with_link() {
        let doc = Document {
            nodes: vec![Block::Paragraph {
                content: vec![Inline::Link {
                    content: vec![Inline::Text {
                        content: "abc".into(),
                        span: bs(1, 4),
                    }],
                    destination: "https://x".into(),
                    title: None,
                    span: bs(0, 20),
                }],
                span: bs(0, 20),
            }],
            front_matter: None,
            line_count: 1,
        };
        let ir = ast_to_ir(&doc, source_id(), &empty_project_metadata());
        assert_eq!(ir.nodes.len(), 1);
        match &ir.nodes[0] {
            IrNode::Paragraph { content, .. } => {
                assert_eq!(content.len(), 1);
                match &content[0] {
                    IrInline::Link {
                        content,
                        destination,
                        title: _title,
                        span,
                    } => {
                        assert_eq!(destination, "https://x");
                        assert_eq!(*span, SourceSpan::new(SourceId(42), 0, 20));
                        assert_eq!(content.len(), 1);
                        match &content[0] {
                            IrInline::Text { content, span } => {
                                assert_eq!(content, "abc");
                                assert_eq!(*span, SourceSpan::new(SourceId(42), 1, 4));
                            }
                            other => panic!("expected Text, got {other:?}"),
                        }
                    }
                    other => panic!("expected Link, got {other:?}"),
                }
            }
            _ => panic!("expected Paragraph"),
        }
    }

    #[test]
    fn convert_paragraph_with_code_span() {
        let doc = Document {
            nodes: vec![Block::Paragraph {
                content: vec![Inline::Code {
                    content: "a ` b".into(),
                    span: bs(4, 13),
                }],
                span: bs(4, 13),
            }],
            front_matter: None,
            line_count: 1,
        };
        let ir = ast_to_ir(&doc, source_id(), &empty_project_metadata());
        assert_eq!(ir.nodes.len(), 1);
        match &ir.nodes[0] {
            IrNode::Paragraph { content, .. } => {
                assert_eq!(content.len(), 1);
                match &content[0] {
                    IrInline::Code { content, span } => {
                        assert_eq!(content, "a ` b");
                        assert_eq!(*span, SourceSpan::new(SourceId(42), 4, 13));
                    }
                    other => panic!("expected Code, got {other:?}"),
                }
            }
            _ => panic!("expected Paragraph"),
        }
    }

    #[test]
    fn soft_break_preserves_source_span() {
        let doc = Document {
            nodes: vec![Block::Paragraph {
                content: vec![
                    Inline::Text {
                        content: "Hello".into(),
                        span: bs(0, 5),
                    },
                    Inline::SoftBreak { span: bs(5, 6) },
                    Inline::Text {
                        content: "world".into(),
                        span: bs(6, 11),
                    },
                ],
                span: bs(0, 11),
            }],
            front_matter: None,
            line_count: 1,
        };
        let ir = ast_to_ir(&doc, source_id(), &empty_project_metadata());
        match &ir.nodes[0] {
            IrNode::Paragraph { content, .. } => {
                assert_eq!(content.len(), 3);
                match &content[1] {
                    IrInline::SoftBreak { span } => {
                        assert_eq!(*span, SourceSpan::new(SourceId(42), 5, 6));
                    }
                    other => panic!("expected Text, got {other:?}"),
                }
            }
            _ => panic!("expected Paragraph"),
        }
    }

    #[test]
    fn hard_break_preserves_source_span() {
        let doc = Document {
            nodes: vec![Block::Paragraph {
                content: vec![
                    Inline::Text {
                        content: "Hello".into(),
                        span: bs(0, 5),
                    },
                    Inline::HardBreak { span: bs(5, 7) },
                    Inline::Text {
                        content: "world".into(),
                        span: bs(7, 12),
                    },
                ],
                span: bs(0, 12),
            }],
            front_matter: None,
            line_count: 1,
        };
        let ir = ast_to_ir(&doc, source_id(), &empty_project_metadata());
        match &ir.nodes[0] {
            IrNode::Paragraph { content, .. } => {
                assert_eq!(content.len(), 3);
                match &content[1] {
                    IrInline::HardBreak { span } => {
                        assert_eq!(*span, SourceSpan::new(SourceId(42), 5, 7));
                    }
                    other => panic!("expected Text, got {other:?}"),
                }
            }
            _ => panic!("expected Paragraph"),
        }
    }

    #[test]
    fn breaks_do_not_report_zero_span_when_mid_document() {
        let doc = Document {
            nodes: vec![Block::Paragraph {
                content: vec![
                    Inline::Text {
                        content: "Hello world ".into(),
                        span: bs(0, 12),
                    },
                    Inline::SoftBreak { span: bs(12, 13) },
                ],
                span: bs(0, 13),
            }],
            front_matter: None,
            line_count: 1,
        };
        let ir = ast_to_ir(&doc, source_id(), &empty_project_metadata());
        match &ir.nodes[0] {
            IrNode::Paragraph { content, .. } => {
                match &content[1] {
                    IrInline::SoftBreak { span } => {
                        // The break is not at the document start: the span must
                        // point at the real source position, never synthesize 0..0.
                        assert_eq!(*span, SourceSpan::new(SourceId(42), 12, 13));
                        assert!(span.start != 0 || span.end != 0);
                    }
                    other => panic!("expected Text, got {other:?}"),
                }
            }
            _ => panic!("expected Paragraph"),
        }
    }

    #[test]
    fn convert_front_matter() {
        let doc = Document {
            nodes: vec![],
            front_matter: Some(FrontMatter {
                fields: vec![
                    ("title".into(), "My Doc".into()),
                    ("author".into(), "Alice".into()),
                    ("format".into(), "slides".into()),
                ],
                span: bs(0, 30),
            }),
            line_count: 4,
        };
        let ir = ast_to_ir(&doc, source_id(), &empty_project_metadata());
        assert_eq!(ir.metadata.title.as_deref(), Some("My Doc"));
        assert_eq!(ir.metadata.author.as_deref(), Some("Alice"));
        assert_eq!(ir.metadata.date, None);
        assert_eq!(ir.metadata.raw.len(), 1);
        assert_eq!(ir.metadata.raw[0], ("format".into(), "slides".into()));
    }

    #[test]
    fn convert_unordered_list() {
        use scribium_markdown::ast::ListItem;
        let doc = Document {
            nodes: vec![Block::UnorderedList {
                items: vec![
                    ListItem {
                        content: vec![Block::Paragraph {
                            content: vec![Inline::Text {
                                content: "A".into(),
                                span: bs(2, 3),
                            }],
                            span: bs(2, 3),
                        }],
                        span: bs(0, 3),
                        task: None,
                    },
                    ListItem {
                        content: vec![Block::Paragraph {
                            content: vec![Inline::Text {
                                content: "B".into(),
                                span: bs(7, 8),
                            }],
                            span: bs(7, 8),
                        }],
                        span: bs(5, 8),
                        task: None,
                    },
                ],
                span: bs(0, 8),
            }],
            front_matter: None,
            line_count: 2,
        };
        let ir = ast_to_ir(&doc, source_id(), &empty_project_metadata());
        assert_eq!(ir.nodes.len(), 1);
        match &ir.nodes[0] {
            IrNode::UnorderedList { items, .. } => {
                assert_eq!(items.len(), 2);
            }
            _ => panic!("expected UnorderedList"),
        }
    }

    #[test]
    fn convert_structures_preserves_task_table_and_nested_spans() {
        use scribium_markdown::ast::ListItem;
        let doc = Document {
            nodes: vec![
                Block::Blockquote {
                    content: vec![Block::Paragraph {
                        content: vec![Inline::Strikethrough {
                            content: vec![Inline::Strong {
                                content: vec![Inline::Text {
                                    content: "removed".into(),
                                    span: bs(4, 11),
                                }],
                                span: bs(2, 13),
                            }],
                            span: bs(0, 15),
                        }],
                        span: bs(0, 15),
                    }],
                    span: bs(0, 15),
                },
                Block::UnorderedList {
                    items: vec![
                        ListItem {
                            content: vec![Block::Paragraph {
                                content: vec![Inline::Text {
                                    content: "active".into(),
                                    span: bs(17, 23),
                                }],
                                span: bs(17, 23),
                            }],
                            span: bs(15, 23),
                            task: Some(TaskStatus::Active),
                        },
                        ListItem {
                            content: vec![Block::Paragraph {
                                content: vec![Inline::Text {
                                    content: "done".into(),
                                    span: bs(25, 29),
                                }],
                                span: bs(25, 29),
                            }],
                            span: bs(23, 29),
                            task: Some(TaskStatus::Completed),
                        },
                    ],
                    span: bs(15, 29),
                },
                Block::Table {
                    header: TableRow {
                        cells: vec![
                            TableCell {
                                content: vec![Inline::Text {
                                    content: "Name".into(),
                                    span: bs(31, 35),
                                }],
                                alignment: TableAlignment::Left,
                                span: bs(31, 35),
                            },
                            TableCell {
                                content: vec![Inline::Text {
                                    content: "Value".into(),
                                    span: bs(37, 42),
                                }],
                                alignment: TableAlignment::Center,
                                span: bs(37, 42),
                            },
                        ],
                        span: bs(31, 42),
                    },
                    rows: vec![TableRow {
                        cells: vec![
                            TableCell {
                                content: vec![Inline::Text {
                                    content: "α".into(),
                                    span: bs(44, 46),
                                }],
                                alignment: TableAlignment::Right,
                                span: bs(44, 46),
                            },
                            TableCell {
                                content: vec![Inline::Text {
                                    content: "β".into(),
                                    span: bs(48, 50),
                                }],
                                alignment: TableAlignment::None,
                                span: bs(48, 50),
                            },
                        ],
                        span: bs(44, 50),
                    }],
                    span: bs(31, 50),
                },
            ],
            front_matter: None,
            line_count: 8,
        };

        let (ir, diagnostics) = ast_to_ir_with_diagnostics_for_mode(
            &doc,
            source_id(),
            &empty_project_metadata(),
            SourceMode::Markdown,
        );
        assert!(
            diagnostics.is_empty(),
            "unexpected diagnostics: {diagnostics:?}"
        );

        let IrNode::Blockquote { content, span } = &ir.nodes[0] else {
            panic!("expected blockquote")
        };
        assert_eq!(*span, SourceSpan::new(source_id(), 0, 15));
        let IrNode::Paragraph { content, .. } = &content[0] else {
            panic!("expected quoted paragraph")
        };
        let IrInline::Strikethrough { content, span } = &content[0] else {
            panic!("expected strikethrough")
        };
        assert_eq!(*span, SourceSpan::new(source_id(), 0, 15));
        assert!(matches!(content[0], IrInline::Strong { .. }));

        let IrNode::UnorderedList { items, .. } = &ir.nodes[1] else {
            panic!("expected task list")
        };
        assert_eq!(items[0].task, Some(IrTaskStatus::Active));
        assert_eq!(items[1].task, Some(IrTaskStatus::Completed));
        assert_eq!(items[0].span, SourceSpan::new(source_id(), 15, 23));

        let IrNode::Table { header, rows, .. } = &ir.nodes[2] else {
            panic!("expected table")
        };
        assert_eq!(header.cells[0].alignment, IrTableAlignment::Left);
        assert_eq!(header.cells[1].alignment, IrTableAlignment::Center);
        assert_eq!(rows[0].cells[0].alignment, IrTableAlignment::Right);
        assert_eq!(rows[0].cells[1].alignment, IrTableAlignment::None);
        assert_eq!(rows[0].cells[0].span, SourceSpan::new(source_id(), 44, 46));
    }

    #[test]
    fn supported_markdown_structures_are_converted_and_unsupported_descendants_remain_explicit() {
        let doc = Document {
            nodes: vec![
                Block::Blockquote {
                    content: vec![
                        Block::Paragraph {
                            content: vec![Inline::Text {
                                content: "first".into(),
                                span: bs(2, 7),
                            }],
                            span: bs(2, 7),
                        },
                        Block::Paragraph {
                            content: vec![Inline::Text {
                                content: "second".into(),
                                span: bs(10, 16),
                            }],
                            span: bs(10, 16),
                        },
                    ],
                    span: bs(0, 16),
                },
                Block::Paragraph {
                    content: vec![
                        Inline::Link {
                            content: vec![Inline::Text {
                                content: "link".into(),
                                span: bs(18, 22),
                            }],
                            destination: "page.md".into(),
                            title: Some("link title".into()),
                            span: bs(17, 32),
                        },
                        Inline::Image {
                            content: vec![Inline::Text {
                                content: "image".into(),
                                span: bs(35, 40),
                            }],
                            destination: "image.png".into(),
                            title: Some("image title".into()),
                            span: bs(34, 51),
                        },
                        Inline::Strikethrough {
                            content: vec![
                                Inline::Text {
                                    content: "hello ".into(),
                                    span: bs(53, 59),
                                },
                                Inline::Strong {
                                    content: vec![Inline::Text {
                                        content: "world".into(),
                                        span: bs(61, 66),
                                    }],
                                    span: bs(59, 68),
                                },
                                Inline::Code {
                                    content: "code".into(),
                                    span: bs(69, 75),
                                },
                            ],
                            span: bs(52, 77),
                        },
                        Inline::RawHtml {
                            content: "<em>html</em>".into(),
                            span: bs(78, 92),
                        },
                    ],
                    span: bs(17, 92),
                },
                Block::Table {
                    header: TableRow {
                        cells: vec![TableCell {
                            content: vec![Inline::Text {
                                content: "A".into(),
                                span: bs(94, 95),
                            }],
                            alignment: TableAlignment::None,
                            span: bs(94, 95),
                        }],
                        span: bs(94, 95),
                    },
                    rows: vec![TableRow {
                        cells: vec![TableCell {
                            content: vec![Inline::Text {
                                content: "1".into(),
                                span: bs(96, 97),
                            }],
                            alignment: TableAlignment::None,
                            span: bs(96, 97),
                        }],
                        span: bs(96, 97),
                    }],
                    span: bs(94, 97),
                },
            ],
            front_matter: None,
            line_count: 1,
        };

        let (ir, diagnostics) = ast_to_ir_with_diagnostics_for_mode(
            &doc,
            source_id(),
            &empty_project_metadata(),
            SourceMode::Markdown,
        );

        let paragraph = ir
            .nodes
            .iter()
            .find_map(|node| match node {
                IrNode::Paragraph { content, .. } => Some(content),
                _ => None,
            })
            .expect("the supported link paragraph remains");
        assert_eq!(paragraph.len(), 3);
        assert!(matches!(
            &paragraph[0],
            IrInline::Link {
                title: Some(title),
                ..
            } if title == "link title"
        ));
        assert!(matches!(
            &paragraph[1],
            IrInline::Image {
                title: Some(title),
                ..
            } if title == "image title"
        ));
        assert!(matches!(paragraph[2], IrInline::Strikethrough { .. }));
        assert_eq!(ir.nodes.len(), 3);
        assert!(matches!(ir.nodes[0], IrNode::Blockquote { .. }));
        assert!(matches!(ir.nodes[2], IrNode::Table { .. }));

        let messages: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect();
        assert!(!messages.iter().any(|message| message.contains("image")));
        assert!(messages
            .iter()
            .any(|message| message.contains("raw HTML inline")));
        assert!(!messages
            .iter()
            .any(|message| message.contains("blockquote")));
        assert!(!messages
            .iter()
            .any(|message| message.contains("strikethrough")));
        assert!(!messages.iter().any(|message| message.contains("GFM table")));
    }

    #[test]
    fn markdown_images_preserve_nested_alt_and_classify_nonlocal_resources() {
        let source = "before ![*formatted* alt](<./assets/my image.png> \"Image title\") after\n";
        let document = scribium_markdown::parse_md(source);
        let (ir, diagnostics) = ast_to_ir_with_diagnostics_for_mode(
            &document,
            source_id(),
            &empty_project_metadata(),
            SourceMode::Markdown,
        );
        assert!(
            diagnostics.is_empty(),
            "unexpected diagnostics: {diagnostics:?}"
        );

        let IrNode::Paragraph { content, .. } = &ir.nodes[0] else {
            panic!("expected image paragraph")
        };
        let IrInline::Image {
            content: alt,
            destination,
            title,
            span,
        } = &content[1]
        else {
            panic!("expected image, got {content:?}")
        };
        assert_eq!(destination, "./assets/my image.png");
        assert_eq!(title.as_deref(), Some("Image title"));
        assert!(matches!(alt[0], IrInline::Emphasis { .. }));
        assert_eq!(
            &source[span.start..span.end],
            "![*formatted* alt](<./assets/my image.png> \"Image title\")"
        );

        for destination in [
            "https://example.com/image.png",
            "data:image/svg+xml;base64,abc",
            "/etc/passwd",
            r"C:\\Users\\foo\\image.png",
        ] {
            let document = scribium_markdown::parse_md(&format!("![alt]({destination})\n"));
            let (_, diagnostics) = ast_to_ir_with_diagnostics_for_mode(
                &document,
                source_id(),
                &empty_project_metadata(),
                SourceMode::Markdown,
            );
            assert_eq!(diagnostics.len(), 1, "destination {destination:?}");
            assert_eq!(diagnostics[0].code, "E8001");
            assert!(diagnostics[0].message.contains("image"));
        }
    }

    #[test]
    fn quarkdown_images_keep_the_existing_unsupported_boundary() {
        let document = scribium_markdown::parse_qd("![alt](image.png)\n");
        let (_, diagnostics) = ast_to_ir_with_diagnostics_for_mode(
            &document,
            source_id(),
            &empty_project_metadata(),
            SourceMode::Quarkdown,
        );
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "E8001" && diagnostic.message.contains("image")
        }));
    }

    #[test]
    fn bounded_attribute_free_inline_html_maps_to_existing_ir_semantics() {
        let source =
            "before <em>italic <strong>bold</strong></em> <del>gone</del> <s>old</s><br /> after\n";
        let document = scribium_markdown::parse_md(source);
        let (ir, diagnostics) = ast_to_ir_with_diagnostics_for_mode(
            &document,
            source_id(),
            &empty_project_metadata(),
            SourceMode::Markdown,
        );
        assert!(
            diagnostics.is_empty(),
            "unexpected diagnostics: {diagnostics:?}"
        );

        let IrNode::Paragraph { content, .. } = &ir.nodes[0] else {
            panic!("expected paragraph, got {:?}", ir.nodes);
        };
        let IrInline::Emphasis {
            content: emphasis,
            span: emphasis_span,
        } = &content[1]
        else {
            panic!("expected HTML emphasis, got {content:?}");
        };
        assert_eq!(
            *emphasis_span,
            SourceSpan::new(
                source_id(),
                source.find("<em>").expect("opening emphasis"),
                source.find("</em>").expect("closing emphasis") + "</em>".len(),
            )
        );
        assert!(matches!(emphasis[0], IrInline::Text { .. }));
        assert!(matches!(emphasis[1], IrInline::Strong { .. }));
        assert!(matches!(content[3], IrInline::Strikethrough { .. }));
        assert!(matches!(content[5], IrInline::Strikethrough { .. }));
        assert!(matches!(content[6], IrInline::HardBreak { .. }));
    }

    #[test]
    fn strikethrough_html_pairs_preserve_del_and_s_tag_identity() {
        for source in ["<del>x</del>\n", "<s>x</s>\n"] {
            let document = scribium_markdown::parse_md(source);
            let (ir, diagnostics) = ast_to_ir_with_diagnostics_for_mode(
                &document,
                source_id(),
                &empty_project_metadata(),
                SourceMode::Markdown,
            );
            assert!(diagnostics.is_empty(), "{source:?}: {diagnostics:?}");

            let IrNode::Paragraph { content, .. } = &ir.nodes[0] else {
                panic!("expected paragraph, got {:?}", ir.nodes);
            };
            assert_eq!(content.len(), 1, "{source:?}: {content:?}");
            assert!(matches!(content[0], IrInline::Strikethrough { .. }));
        }

        let nested_source = "<del><s>x</s></del>\n";
        let nested_document = scribium_markdown::parse_md(nested_source);
        let (nested_ir, nested_diagnostics) = ast_to_ir_with_diagnostics_for_mode(
            &nested_document,
            source_id(),
            &empty_project_metadata(),
            SourceMode::Markdown,
        );
        assert!(nested_diagnostics.is_empty(), "{nested_diagnostics:?}");
        let IrNode::Paragraph {
            content: nested_content,
            ..
        } = &nested_ir.nodes[0]
        else {
            panic!("expected nested paragraph, got {:?}", nested_ir.nodes);
        };
        let IrInline::Strikethrough {
            content: outer_content,
            ..
        } = &nested_content[0]
        else {
            panic!("expected outer strikethrough, got {nested_content:?}");
        };
        assert!(matches!(
            outer_content.as_slice(),
            [IrInline::Strikethrough { .. }]
        ));
    }

    #[test]
    fn mismatched_strikethrough_html_tags_remain_unsupported() {
        for (source, expected_raw) in [
            ("<del>x</s>\n", vec!["<del>", "</s>"]),
            ("<s>x</del>\n", vec!["<s>", "</del>"]),
            (
                "<del><s>x</del></s>\n",
                vec!["<del>", "<s>", "</del>", "</s>"],
            ),
        ] {
            let document = scribium_markdown::parse_md(source);
            let (ir, diagnostics) = ast_to_ir_with_diagnostics_for_mode(
                &document,
                source_id(),
                &empty_project_metadata(),
                SourceMode::Markdown,
            );
            assert_eq!(diagnostics.len(), expected_raw.len(), "{source:?}");
            assert!(
                diagnostics
                    .iter()
                    .all(|diagnostic| diagnostic.code == "E8001"),
                "{source:?}: {diagnostics:?}"
            );
            let diagnostic_raw = diagnostics
                .iter()
                .map(|diagnostic| {
                    let span = diagnostic.primary.expect("HTML diagnostic span");
                    source[span.start..span.end].to_string()
                })
                .collect::<Vec<_>>();
            assert_eq!(diagnostic_raw, expected_raw, "{source:?}");

            let IrNode::Paragraph { content, .. } = &ir.nodes[0] else {
                panic!("expected paragraph, got {:?}", ir.nodes);
            };
            assert!(
                content
                    .iter()
                    .all(|inline| !matches!(inline, IrInline::Strikethrough { .. })),
                "mismatched tags must not lower to strikethrough: {source:?}: {content:?}"
            );
        }
    }

    #[test]
    fn markdown_html_comments_are_inline_semantic_noops_and_neutral_for_pairs() {
        for source in [
            "before <!-- note --> after\n",
            "before <!-- this is a --\ncomment - with hyphens --> after\n",
            "before <!--> after\n",
            "before <!---> after\n",
        ] {
            let document = scribium_markdown::parse_md(source);
            let (ir, diagnostics) = ast_to_ir_with_diagnostics_for_mode(
                &document,
                source_id(),
                &empty_project_metadata(),
                SourceMode::Markdown,
            );
            assert!(diagnostics.is_empty(), "{source:?}: {diagnostics:?}");
            let IrNode::Paragraph { content, .. } = &ir.nodes[0] else {
                panic!("expected paragraph, got {:?}", ir.nodes);
            };
            for inline in content {
                if let IrInline::Text { content, span } = inline {
                    assert_eq!(span.source_id, source_id());
                    assert_eq!(source.get(span.start..span.end), Some(content.as_str()));
                }
            }
            assert_eq!(
                content
                    .iter()
                    .filter_map(|inline| match inline {
                        IrInline::Text { content, .. } => Some(content.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>(),
                vec!["before ", " after"]
            );
        }

        for source in [
            "<em>before <!-- note --> after</em>\n",
            "<em>a <strong>b<!-- note -->c</strong> d</em>\n",
        ] {
            let document = scribium_markdown::parse_md(source);
            let (ir, diagnostics) = ast_to_ir_with_diagnostics_for_mode(
                &document,
                source_id(),
                &empty_project_metadata(),
                SourceMode::Markdown,
            );
            assert!(diagnostics.is_empty(), "{source:?}: {diagnostics:?}");
            assert!(matches!(
                ir.nodes.first(),
                Some(IrNode::Paragraph { content, .. })
                    if content.iter().any(|inline| matches!(inline, IrInline::Emphasis { .. }))
            ));
        }

        let unknown_source = "<em>before <span>unknown</span> after</em>\n";
        let document = scribium_markdown::parse_md(unknown_source);
        let (ir, diagnostics) = ast_to_ir_with_diagnostics_for_mode(
            &document,
            source_id(),
            &empty_project_metadata(),
            SourceMode::Markdown,
        );
        assert_eq!(diagnostics.len(), 4, "{diagnostics:?}");
        assert!(diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code == "E8001"));
        assert!(
            matches!(ir.nodes.first(), Some(IrNode::Paragraph { content, .. }) if content
            .iter()
            .all(|inline| !matches!(inline, IrInline::Emphasis { .. })))
        );
    }

    #[test]
    fn markdown_comment_only_blocks_drop_without_widening_raw_html_support() {
        for source in [
            "<!-- note -->\n",
            "<!--\nnote\n-->\n",
            "  <!-- indented -->\r\n",
        ] {
            let document = scribium_markdown::parse_md(source);
            let (ir, diagnostics) = ast_to_ir_with_diagnostics_for_mode(
                &document,
                source_id(),
                &empty_project_metadata(),
                SourceMode::Markdown,
            );
            assert!(diagnostics.is_empty(), "{source:?}: {diagnostics:?}");
            assert!(ir.nodes.is_empty(), "{source:?}: {:?}", ir.nodes);
        }

        for source in [
            "<!-- note -->VISIBLE\n",
            "<!--> VISIBLE -->\n",
            "<!---> VISIBLE -->\n",
            "<!-- one --><!-- two -->\n",
            "<!-- unterminated\nvisible\n",
        ] {
            let document = scribium_markdown::parse_md(source);
            let (ir, diagnostics) = ast_to_ir_with_diagnostics_for_mode(
                &document,
                source_id(),
                &empty_project_metadata(),
                SourceMode::Markdown,
            );
            assert_eq!(ir.nodes.len(), 0, "{source:?}: {:?}", ir.nodes);
            assert_eq!(diagnostics.len(), 1, "{source:?}: {diagnostics:?}");
            assert_eq!(diagnostics[0].code, "E8001");
            let span = diagnostics[0].primary.expect("raw HTML span");
            assert_eq!(span.source_id, source_id());
            assert_eq!(span.start, 0);
            assert_eq!(span.end, source.len());
        }

        for source in [
            "<?processing instruction?>\n",
            "<!DOCTYPE html>\n",
            "<![CDATA[content]]>\n",
            "<span>x</span>\n",
            "<div>\nvisible\n</div>\n",
        ] {
            let document = scribium_markdown::parse_md(source);
            let (ir, diagnostics) = ast_to_ir_with_diagnostics_for_mode(
                &document,
                source_id(),
                &empty_project_metadata(),
                SourceMode::Markdown,
            );
            if source.starts_with("<span>") {
                assert!(matches!(
                    ir.nodes.as_slice(),
                    [IrNode::Paragraph { content, .. }]
                        if matches!(content.as_slice(), [IrInline::Text { content, .. }] if content == "x")
                ));
            } else {
                assert!(ir.nodes.is_empty(), "{source:?}: {:?}", ir.nodes);
            }
            assert!(!diagnostics.is_empty(), "{source:?} unexpectedly succeeded");
            assert!(diagnostics
                .iter()
                .all(|diagnostic| diagnostic.code == "E8001"));
            assert!(diagnostics.iter().all(|diagnostic| {
                diagnostic
                    .primary
                    .is_some_and(|span| span.source_id == source_id() && span.end > span.start)
            }));
        }
    }

    #[test]
    fn markdown_comment_separator_preserves_distinct_list_and_code_nodes() {
        let lists = scribium_markdown::parse_md("- foo\n- bar\n\n<!-- -->\n\n- baz\n- bim\n");
        let (list_ir, list_diagnostics) = ast_to_ir_with_diagnostics_for_mode(
            &lists,
            source_id(),
            &empty_project_metadata(),
            SourceMode::Markdown,
        );
        assert!(list_diagnostics.is_empty(), "{list_diagnostics:?}");
        assert!(matches!(
            list_ir.nodes.as_slice(),
            [IrNode::UnorderedList { .. }, IrNode::UnorderedList { .. }]
        ));

        let list_and_code = scribium_markdown::parse_md("- foo\n- bar\n\n<!-- -->\n\n    code\n");
        let (code_ir, code_diagnostics) = ast_to_ir_with_diagnostics_for_mode(
            &list_and_code,
            source_id(),
            &empty_project_metadata(),
            SourceMode::Markdown,
        );
        assert!(code_diagnostics.is_empty(), "{code_diagnostics:?}");
        assert!(matches!(
            code_ir.nodes.as_slice(),
            [IrNode::UnorderedList { .. }, IrNode::CodeBlock { .. }]
        ));
    }

    #[test]
    fn unsupported_html_keeps_deterministic_diagnostics_and_original_spans() {
        let inline_source = "before <span class=\"layout\">x</span> after\n";
        let inline_document = scribium_markdown::parse_md(inline_source);
        let (inline_ir, inline_diagnostics) = ast_to_ir_with_diagnostics_for_mode(
            &inline_document,
            source_id(),
            &empty_project_metadata(),
            SourceMode::Markdown,
        );
        assert_eq!(inline_diagnostics.len(), 2, "{inline_diagnostics:?}");
        assert!(inline_diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code == "E8001"));
        let inline_spans = inline_diagnostics
            .iter()
            .map(|diagnostic| {
                let span = diagnostic.primary.expect("HTML diagnostic span");
                inline_source[span.start..span.end].to_string()
            })
            .collect::<Vec<_>>();
        assert_eq!(inline_spans, vec!["<span class=\"layout\">", "</span>"]);
        assert!(matches!(inline_ir.nodes[0], IrNode::Paragraph { .. }));

        let block_source = "<div>\n**not Markdown**\n</div>\n\ntext\n";
        let block_document = scribium_markdown::parse_md(block_source);
        let (_, block_diagnostics) = ast_to_ir_with_diagnostics_for_mode(
            &block_document,
            source_id(),
            &empty_project_metadata(),
            SourceMode::Markdown,
        );
        assert_eq!(block_diagnostics.len(), 1, "{block_diagnostics:?}");
        let block_span = block_diagnostics[0]
            .primary
            .expect("block HTML diagnostic span");
        assert_eq!(
            block_source.get(block_span.start..block_span.end),
            Some("<div>\n**not Markdown**\n</div>\n"),
        );
        assert_eq!(block_diagnostics[0].code, "E8001");

        let ambiguous_source = "before <em>outer <strong>inner</strong> after\n";
        let ambiguous_document = scribium_markdown::parse_md(ambiguous_source);
        let (ambiguous_ir, ambiguous_diagnostics) = ast_to_ir_with_diagnostics_for_mode(
            &ambiguous_document,
            source_id(),
            &empty_project_metadata(),
            SourceMode::Markdown,
        );
        assert_eq!(ambiguous_diagnostics.len(), 3, "{ambiguous_diagnostics:?}");
        let IrNode::Paragraph { content, .. } = &ambiguous_ir.nodes[0] else {
            panic!("expected ambiguous HTML paragraph");
        };
        assert!(content.iter().all(|inline| matches!(
            inline,
            IrInline::Text { .. } | IrInline::SoftBreak { .. } | IrInline::HardBreak { .. }
        )));
    }
}
