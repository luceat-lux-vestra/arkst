//! AST-to-IR conversion — translates the parsed Markdown AST into the Scribium IR.
//!
//! This is the bridge between `scribium-markdown` (parser output) and `ir`
//! (evaluator input / lowering input). It preserves semantic values and call
//! structure for the evaluator without rewriting source text.

use crate::diagnostics::{Diagnostic, Severity};
use crate::ir::{
    IrCallSegment, IrDocument, IrInline, IrListItem, IrMetadata, IrNamedArg, IrNode, IrParameter,
    IrTableAlignment, IrTableCell, IrTableRow, IrTaskStatus,
};
use crate::source::{SourceId, SourceSpan};
use crate::virtual_project::ProjectMetadata;
use scribium_markdown::ast::{
    Block, CallSegment, Document, Inline, TableAlignment, TaskStatus, Value,
};

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
    let mut diagnostics = Vec::new();
    let nodes: Vec<IrNode> = doc
        .nodes
        .iter()
        .filter_map(|b| block_to_ir(b, source_id, &mut diagnostics))
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
            },
        },
        diagnostics,
    )
}

fn block_to_ir(
    block: &Block,
    source_id: SourceId,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<IrNode> {
    match block {
        Block::Heading {
            level,
            content,
            span,
        } => Some(IrNode::Heading {
            level: *level,
            content: inlines_to_ir(content, source_id, diagnostics),
            span: byte_to_source_span(span, source_id),
        }),
        Block::Paragraph { content, span } => {
            let inlines = inlines_to_ir(content, source_id, diagnostics);
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
                .filter_map(|child| block_to_ir(child, source_id, diagnostics))
                .collect(),
            span: byte_to_source_span(span, source_id),
        }),
        Block::UnorderedList { items, span } => {
            let ir_items: Vec<IrListItem> = items
                .iter()
                .map(|item| list_item_to_ir(item, source_id, diagnostics))
                .collect();
            Some(IrNode::UnorderedList {
                items: ir_items,
                span: byte_to_source_span(span, source_id),
            })
        }
        Block::OrderedList { items, start, span } => {
            let ir_items: Vec<IrListItem> = items
                .iter()
                .map(|item| list_item_to_ir(item, source_id, diagnostics))
                .collect();
            Some(IrNode::OrderedList {
                items: ir_items,
                start: *start,
                span: byte_to_source_span(span, source_id),
            })
        }
        Block::CodeBlock {
            language,
            source,
            span,
        } => Some(IrNode::CodeBlock {
            language: language.clone(),
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
                .map(|v| value_to_ir(v, source_id, diagnostics))
                .collect();
            let ir_named: Vec<_> = named_args
                .iter()
                .map(|arg| IrNamedArg {
                    name: arg.name.clone(),
                    name_span: byte_to_source_span(&arg.name_span, source_id),
                    value: value_to_ir(&arg.value, source_id, diagnostics),
                    span: byte_to_source_span(&arg.span, source_id),
                })
                .collect();
            let ir_body = body.as_ref().map(|blocks| {
                blocks
                    .iter()
                    .filter_map(|b| block_to_ir(b, source_id, diagnostics))
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
                    .map(|header| {
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
                    })
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
                        .map(|segment| call_segment_to_ir(segment, source_id, diagnostics))
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
            header: table_row_to_ir(header, source_id, diagnostics),
            rows: rows
                .iter()
                .map(|row| table_row_to_ir(row, source_id, diagnostics))
                .collect(),
            span: byte_to_source_span(span, source_id),
        }),
        Block::RawHtml { span, .. } => {
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

fn list_item_to_ir(
    item: &scribium_markdown::ast::ListItem,
    source_id: SourceId,
    diagnostics: &mut Vec<Diagnostic>,
) -> IrListItem {
    IrListItem {
        nodes: item
            .content
            .iter()
            .filter_map(|child| block_to_ir(child, source_id, diagnostics))
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
) -> IrTableRow {
    IrTableRow {
        cells: row
            .cells
            .iter()
            .map(|cell| IrTableCell {
                content: inlines_to_ir(&cell.content, source_id, diagnostics),
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
) -> IrCallSegment {
    IrCallSegment {
        name: segment.name.clone(),
        name_span: byte_to_source_span(&segment.name_span, source_id),
        positional_args: segment
            .positional_args
            .iter()
            .map(|value| value_to_ir(value, source_id, diagnostics))
            .collect(),
        named_args: segment
            .named_args
            .iter()
            .map(|arg| IrNamedArg {
                name: arg.name.clone(),
                name_span: byte_to_source_span(&arg.name_span, source_id),
                value: value_to_ir(&arg.value, source_id, diagnostics),
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
) -> Vec<IrInline> {
    inlines
        .iter()
        .filter_map(|inline| inline_to_ir(inline, source_id, diagnostics))
        .collect()
}

fn inline_to_ir(
    inline: &Inline,
    source_id: SourceId,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<IrInline> {
    match inline {
        Inline::Text { content, span } => Some(IrInline::Text {
            content: content.clone(),
            span: byte_to_source_span(span, source_id),
        }),
        Inline::Emphasis { content, span } => {
            let children = inlines_to_ir(content, source_id, diagnostics);
            Some(IrInline::Emphasis {
                content: children,
                span: byte_to_source_span(span, source_id),
            })
        }
        Inline::Strong { content, span } => {
            let children = inlines_to_ir(content, source_id, diagnostics);
            Some(IrInline::Strong {
                content: children,
                span: byte_to_source_span(span, source_id),
            })
        }
        Inline::Strikethrough { content, span } => Some(IrInline::Strikethrough {
            content: inlines_to_ir(content, source_id, diagnostics),
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
                .map(|v| value_to_ir(v, source_id, diagnostics))
                .collect();
            let ir_named: Vec<_> = named_args
                .iter()
                .map(|arg| IrNamedArg {
                    name: arg.name.clone(),
                    name_span: byte_to_source_span(&arg.name_span, source_id),
                    value: value_to_ir(&arg.value, source_id, diagnostics),
                    span: byte_to_source_span(&arg.span, source_id),
                })
                .collect();
            let ir_body = body
                .as_ref()
                .map(|b| inlines_to_ir(b, source_id, diagnostics));
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
                        .map(|segment| call_segment_to_ir(segment, source_id, diagnostics))
                        .collect(),
                    body: ir_body,
                    span: byte_to_source_span(span, source_id),
                })
            }
        }
        Inline::HardBreak { span } | Inline::SoftBreak { span } => {
            // Breaks become whitespace in the text flow for M1
            Some(IrInline::Text {
                content: "\n".to_string(),
                span: byte_to_source_span(span, source_id),
            })
        }
        Inline::Link {
            content,
            destination,
            span,
        } => Some(IrInline::Link {
            content: inlines_to_ir(content, source_id, diagnostics),
            destination: destination.clone(),
            span: byte_to_source_span(span, source_id),
        }),
        Inline::Image { span, .. } => {
            push_unsupported(diagnostics, "image", span, source_id);
            None
        }
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
) -> crate::ir::IrValue {
    match value {
        Value::String(s) => crate::ir::IrValue::String(s.clone()),
        Value::Number(n) => crate::ir::IrValue::Number(*n),
        Value::Boolean(b) => crate::ir::IrValue::Boolean(*b),
        Value::Identifier(id) => crate::ir::IrValue::Identifier(id.clone()),
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
                    .map(|v| value_to_ir(v, source_id, diagnostics))
                    .collect();
                let ir_named: Vec<_> = named_args
                    .iter()
                    .map(|arg| IrNamedArg {
                        name: arg.name.clone(),
                        name_span: byte_to_source_span(&arg.name_span, source_id),
                        value: value_to_ir(&arg.value, source_id, diagnostics),
                        span: byte_to_source_span(&arg.span, source_id),
                    })
                    .collect();
                let ir_body = body.as_ref().map(|b| {
                    vec![IrNode::Paragraph {
                        content: inlines_to_ir(b, source_id, diagnostics),
                        span: byte_to_source_span(span, source_id),
                    }]
                });
                if chain.is_empty() {
                    crate::ir::IrValue::Content(vec![IrNode::FunctionCall {
                        name: name.clone(),
                        positional_args: ir_positional,
                        named_args: ir_named,
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
                            .map(|segment| call_segment_to_ir(segment, source_id, diagnostics))
                            .collect(),
                        body: ir_body,
                        span: byte_to_source_span(span, source_id),
                    }])
                }
            } else {
                let start = inlines.first().map(inline_span_start);
                let end = inlines.last().map(inline_span_end);
                let span = crate::source::ByteSpan::new(start.unwrap_or(0), end.unwrap_or(0));
                crate::ir::IrValue::Content(vec![IrNode::Paragraph {
                    content: inlines_to_ir(inlines, source_id, diagnostics),
                    span: byte_to_source_span(&span, source_id),
                }])
            }
        }
    }
}

fn push_unsupported(
    diagnostics: &mut Vec<Diagnostic>,
    feature: &str,
    span: &crate::source::ByteSpan,
    source_id: SourceId,
) {
    diagnostics.push(Diagnostic {
        code: "E8001".to_string(),
        severity: Severity::Error,
        message: format!(
            "Markdown syntax `{feature}` was parsed and preserved by the frontend but is not supported by the current IR/Typst lowering"
        ),
        primary: Some(byte_to_source_span(span, source_id)),
        secondary: Vec::new(),
        hints: vec![
            "The source semantics were not coerced into a different Markdown node.".to_string(),
        ],
    });
}

fn invalid_function_declaration(
    message: &str,
    span: &crate::source::ByteSpan,
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

fn byte_to_source_span(byte_span: &crate::source::ByteSpan, source_id: SourceId) -> SourceSpan {
    SourceSpan::new(source_id, byte_span.start, byte_span.end)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::{ByteSpan, SourceId};
    use scribium_markdown::ast::{FrontMatter, TableAlignment, TableCell, TableRow};

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
        let (ir, diagnostics) =
            ast_to_ir_with_diagnostics(&document, source_id(), &empty_project_metadata());
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
                    IrInline::Text { content, span } => {
                        assert_eq!(content, "\n");
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
                    IrInline::Text { content, span } => {
                        assert_eq!(content, "\n");
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
                    IrInline::Text { content, span } => {
                        assert_eq!(content, "\n");
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

        let (ir, diagnostics) =
            ast_to_ir_with_diagnostics(&doc, source_id(), &empty_project_metadata());
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
                            span: bs(17, 32),
                        },
                        Inline::Image {
                            content: vec![Inline::Text {
                                content: "image".into(),
                                span: bs(35, 40),
                            }],
                            destination: "image.png".into(),
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

        let (ir, diagnostics) =
            ast_to_ir_with_diagnostics(&doc, source_id(), &empty_project_metadata());

        let paragraph = ir
            .nodes
            .iter()
            .find_map(|node| match node {
                IrNode::Paragraph { content, .. } => Some(content),
                _ => None,
            })
            .expect("the supported link paragraph remains");
        assert_eq!(paragraph.len(), 2);
        assert!(matches!(paragraph[0], IrInline::Link { .. }));
        assert!(matches!(paragraph[1], IrInline::Strikethrough { .. }));
        assert_eq!(ir.nodes.len(), 3);
        assert!(matches!(ir.nodes[0], IrNode::Blockquote { .. }));
        assert!(matches!(ir.nodes[2], IrNode::Table { .. }));

        let messages: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect();
        assert!(messages.iter().any(|message| message.contains("image")));
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
}
