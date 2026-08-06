//! AST-to-IR conversion — translates the parsed Markdown AST into the Scribium IR.
//!
//! This is the bridge between `syntax::markdown` (parser output) and `ir`
//! (evaluator input / lowering input). For M1, this is a direct 1:1 mapping
//! since there is no evaluator yet.

use crate::ir::{IrDocument, IrInline, IrListItem, IrMetadata, IrNode};
use crate::source::{SourceId, SourceSpan};
use crate::syntax::markdown::ast::{Block, Document, Inline, Value};
use crate::virtual_project::ProjectMetadata;

/// Convert a parsed Markdown `Document` into an `IrDocument`.
///
/// `source_id` identifies the source file in the span model.
/// `project_metadata` provides project-level defaults that can be overridden
/// by document front matter.
pub fn ast_to_ir(
    doc: &Document,
    source_id: SourceId,
    project_metadata: &ProjectMetadata,
) -> IrDocument {
    let nodes: Vec<IrNode> = doc
        .nodes
        .iter()
        .filter_map(|b| block_to_ir(b, source_id))
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

    IrDocument {
        nodes,
        metadata: IrMetadata {
            title,
            author,
            date,
            raw,
        },
    }
}

fn block_to_ir(block: &Block, source_id: SourceId) -> Option<IrNode> {
    match block {
        Block::Heading {
            level,
            content,
            span,
        } => Some(IrNode::Heading {
            level: *level,
            content: inlines_to_ir(content, source_id),
            span: byte_to_source_span(span, source_id),
        }),
        Block::Paragraph { content, span } => {
            let inlines = inlines_to_ir(content, source_id);
            if inlines.is_empty() {
                return None;
            }
            Some(IrNode::Paragraph {
                content: inlines,
                span: byte_to_source_span(span, source_id),
            })
        }
        Block::UnorderedList { items, span } => {
            let ir_items: Vec<IrListItem> = items
                .iter()
                .map(|item| IrListItem {
                    nodes: item
                        .content
                        .iter()
                        .filter_map(|b| block_to_ir(b, source_id))
                        .collect(),
                    span: byte_to_source_span(&item.span, source_id),
                })
                .collect();
            Some(IrNode::UnorderedList {
                items: ir_items,
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
        Block::BlankLine { .. } => None,
        Block::DirectiveCall {
            name,
            positional_args,
            named_args,
            body,
            span,
        } => {
            let ir_positional: Vec<_> = positional_args.iter().map(value_to_ir).collect();
            let ir_named: Vec<_> = named_args
                .iter()
                .map(|(k, v)| (k.clone(), value_to_ir(v)))
                .collect();
            let ir_body = body.as_ref().and_then(|b| block_to_ir(b, source_id));
            Some(IrNode::FunctionCall {
                name: name.clone(),
                positional_args: ir_positional,
                named_args: ir_named,
                body: ir_body.map(Box::new),
                span: byte_to_source_span(span, source_id),
            })
        }
        Block::Metadata { .. } => {
            // Metadata is handled via front_matter on the Document; skip as a block node.
            None
        }
    }
}

fn inlines_to_ir(inlines: &[Inline], source_id: SourceId) -> Vec<IrInline> {
    inlines
        .iter()
        .filter_map(|inline| inline_to_ir(inline, source_id))
        .collect()
}

fn inline_to_ir(inline: &Inline, source_id: SourceId) -> Option<IrInline> {
    match inline {
        Inline::Text { content, span } => Some(IrInline::Text {
            content: content.clone(),
            span: byte_to_source_span(span, source_id),
        }),
        Inline::Emphasis { content, span } => {
            let children = inlines_to_ir(content, source_id);
            Some(IrInline::Emphasis {
                content: children,
                span: byte_to_source_span(span, source_id),
            })
        }
        Inline::Strong { content, span } => {
            let children = inlines_to_ir(content, source_id);
            Some(IrInline::Strong {
                content: children,
                span: byte_to_source_span(span, source_id),
            })
        }
        Inline::DirectiveCall {
            name,
            positional_args,
            named_args,
            body,
            span,
        } => {
            let ir_positional: Vec<_> = positional_args.iter().map(value_to_ir).collect();
            let ir_named: Vec<_> = named_args
                .iter()
                .map(|(k, v)| (k.clone(), value_to_ir(v)))
                .collect();
            let ir_body = body.as_ref().map(|b| inlines_to_ir(b, source_id));
            Some(IrInline::DirectiveCall {
                name: name.clone(),
                positional_args: ir_positional,
                named_args: ir_named,
                body: ir_body,
                span: byte_to_source_span(span, source_id),
            })
        }
        Inline::HardBreak { span } | Inline::SoftBreak { span } => {
            // Breaks become whitespace in the text flow for M1
            Some(IrInline::Text {
                content: "\n".to_string(),
                span: byte_to_source_span(span, source_id),
            })
        }
    }
}

fn value_to_ir(value: &Value) -> crate::ir::IrValue {
    match value {
        Value::String(s) => crate::ir::IrValue::String(s.clone()),
        Value::Number(n) => crate::ir::IrValue::Number(*n),
        Value::Boolean(b) => crate::ir::IrValue::Boolean(*b),
        Value::Identifier(id) => crate::ir::IrValue::Identifier(id.clone()),
    }
}

fn byte_to_source_span(byte_span: &crate::source::ByteSpan, source_id: SourceId) -> SourceSpan {
    SourceSpan::new(source_id, byte_span.start, byte_span.end)
}
mod tests {
    use super::*;
    use crate::source::{ByteSpan, SourceId};
    #[allow(unused_imports)]
    use crate::syntax::markdown::ast::FrontMatter;

    #[allow(dead_code)]
    fn source_id() -> SourceId {
        SourceId(42)
    }

    #[allow(dead_code)]
    fn bs(start: usize, end: usize) -> ByteSpan {
        ByteSpan::new(start, end)
    }

    #[allow(dead_code)]
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
        use crate::syntax::markdown::ast::ListItem;
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
}
