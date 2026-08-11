//! Minimal CommonMark-compatible Markdown parser.
//!
//! Produces the Scribium AST with byte-level source spans on every node.
//! Supported constructs (M1 subset):
//!
//! - ATX headings (`#` through `######`)
//! - Paragraphs with soft/hard line breaks
//! - Emphasis (`*text*`, `_text_`) and strong (`**text**`, `__text__`)
//! - Unordered lists (`- `, `* `, `+ `) with nested lists and code blocks
//! - Ordered lists (`1. `, `1) `) with nested lists
//! - Fenced code blocks (triple backtick, optional language)
//! - Thematic breaks (`---`, `***`, `___`)
//!
//! M2 additions:
//!
//! - Inline links (`[label](destination)`) with inline markup in the label
//! - Inline code spans (`\`code\``, or any matching backtick-run length),
//!   with opaque literal contents and CommonMark line-ending and
//!   surrounding-space normalization
//!
//! Delimiter runs of three or more identical characters (`***x***`) are
//! treated as literal text. Setext headings, images, and blockquotes are
//! not part of the M1 subset, and reference-style links are not
//! part of the M2 subset.

#[cfg(test)]
use super::ast::Block;
use super::ast::{Document, FrontMatter, Inline, Value};
use super::block::{BlockParser, SourceLine as BlockSourceLine};
use crate::source::ByteSpan;

/// Maximum block-nesting depth before a parse is flattened to paragraphs.
///
/// Guards against stack overflow on pathological input such as thousands of
/// nested list markers.
pub(super) const MAX_BLOCK_DEPTH: usize = 64;

/// Maximum inline-nesting depth before delimiters are treated as literal text.
pub(crate) const MAX_INLINE_DEPTH: usize = 64;
/// Parse flat key-value front matter at document start.
///
/// Front matter is a `---`-delimited block of `key: value` lines only. It is
/// not full YAML: nested objects, arrays, block strings, and other YAML
/// features are not supported. The delimiters must start at column 0, and
/// every non-empty metadata line must start at column 0 — leading indentation
/// marks nested structure and rejects the whole block. Keys and values are
/// split on the first colon; duplicate keys use last-wins semantics.
///
/// Returns `(front_matter, lines_consumed)`. If no valid front matter is found
/// at the start, returns `(None, 0)` and the caller should start parsing from line 0.
fn parse_front_matter(
    _source: &str,
    lines: &[BlockSourceLine<'_>],
) -> (Option<FrontMatter>, usize) {
    if lines.is_empty() {
        return (None, 0);
    }

    // Check if first line is opening delimiter `---`
    // Use raw to reject indented delimiters
    let first = &lines[0];
    if first.raw != "---" {
        return (None, 0);
    }

    // Find closing delimiter
    let mut close_idx = None;
    for (i, line) in lines.iter().enumerate().skip(1) {
        if line.raw == "---" {
            close_idx = Some(i);
            break;
        }
    }

    let close_idx = match close_idx {
        Some(idx) => idx,
        None => {
            // Unclosed front matter - treat as no front matter
            return (None, 0);
        }
    };

    // Parse fields between delimiters, checking for malformed lines
    let mut fields = Vec::new();
    for line in &lines[1..close_idx] {
        let text = line.text;
        if text.is_empty() {
            continue; // Skip empty lines in front matter
        }
        // Metadata lines must start at column 0: leading indentation marks
        // nested (YAML-style) structure, which is not flattened. Reject the
        // whole block so it stays intact as regular Markdown.
        if line.raw != line.text {
            return (None, 0);
        }
        if let Some(colon_pos) = text.find(':') {
            let key = text[..colon_pos].trim();
            let value = text[colon_pos + 1..].trim();
            if key.is_empty() {
                // Empty key - malformed, treat entire block as invalid
                return (None, 0);
            }
            // last-wins: remove existing entry with same key
            fields.retain(|(k, _)| k != key);
            fields.push((key.to_string(), value.to_string()));
        } else {
            // Line without colon - malformed, treat entire block as invalid
            return (None, 0);
        }
    }

    let span = ByteSpan::new(first.raw_start, lines[close_idx].end);
    (Some(FrontMatter { fields, span }), close_idx + 1)
}

/// A parse-level diagnostic produced for malformed but recoverable input
/// (never fatal: parsing continues and the offending text is treated as
/// ordinary content).
#[derive(Debug, Clone, PartialEq)]
pub struct ParserDiagnostic {
    /// Stable error code (e.g. `E2003`).
    pub code: &'static str,
    /// Human-readable description.
    pub message: String,
    /// Source span of the offending construct.
    pub span: ByteSpan,
}

/// Parse a Markdown source string into a `Document`.
///
/// Never panics on malformed input; unclosed constructs are parsed
/// deterministically up to the end of the source.
pub fn parse(source: &str) -> Document {
    parse_with_diagnostics(source).document
}

/// Parse a Markdown source string, returning the document together with the
/// structured diagnostics gathered for malformed-but-recoverable constructs.
pub fn parse_with_diagnostics(source: &str) -> ParseOutput {
    let lines = super::block::split_lines(source);
    let mut diagnostics: Vec<ParserDiagnostic> = Vec::new();

    // Parse front matter if present at document start
    let (front_matter, front_matter_lines) = parse_front_matter(source, &lines);
    let cursor = front_matter_lines;

    let nodes = BlockParser::new(source, &lines, cursor, &mut diagnostics).parse();
    let line_count = source.as_bytes().iter().filter(|&&b| b == b'\n').count() + 1;
    ParseOutput {
        document: Document {
            nodes,
            front_matter,
            line_count,
        },
        diagnostics,
    }
}

/// The result of `parse_with_diagnostics`.
#[derive(Debug, Clone)]
pub struct ParseOutput {
    pub document: Document,
    /// Diagnostics for malformed constructs; the document still parses the
    /// offending text as ordinary content.
    pub diagnostics: Vec<ParserDiagnostic>,
}

/// Convert a Quarkdown-layer argument into a Markdown `Value`.
///
/// Scalars map directly; content fragments are run through the inline parser
/// so that nested calls and inline markup inside the argument are preserved.
pub(crate) fn convert_quarkdown_arg(
    source: &str,
    arg: &crate::syntax::quarkdown::Arg,
    depth: usize,
    diagnostics: &mut Vec<ParserDiagnostic>,
) -> Value {
    match &arg.content {
        crate::syntax::quarkdown::ArgContent::Scalar(value) => value.clone(),
        crate::syntax::quarkdown::ArgContent::Content(span) => Value::Content(parse_inlines(
            source,
            span.start,
            span.end,
            depth,
            diagnostics,
        )),
    }
}

pub(crate) fn parse_inlines(
    source: &str,
    start: usize,
    end: usize,
    depth: usize,
    diagnostics: &mut Vec<ParserDiagnostic>,
) -> Vec<Inline> {
    super::inline::parse_inlines(source, start, end, depth, diagnostics)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_text(inline: &Inline, expected: &str) {
        match inline {
            Inline::Text { content, .. } => assert_eq!(content, expected),
            other => panic!("expected Text({expected:?}), got {other:?}"),
        }
    }

    /// Concatenate all text in a flat list of inline nodes, rendering
    /// soft/hard breaks as `\n`, links in their source form
    /// (`[label](destination)`), and code spans as `` `content` ``.
    /// Panics when any other inline kind appears, so tests can use it to
    /// assert that a span contains only prose.
    fn joined_text(inlines: &[Inline]) -> String {
        let mut out = String::new();
        for inline in inlines {
            match inline {
                Inline::Text { content, .. } => out.push_str(content),
                Inline::SoftBreak { .. } | Inline::HardBreak { .. } => out.push('\n'),
                Inline::Link {
                    content,
                    destination,
                    ..
                } => {
                    out.push('[');
                    out.push_str(&joined_text(content));
                    out.push_str("](");
                    out.push_str(destination);
                    out.push(')');
                }
                Inline::Code { content, .. } => {
                    out.push('`');
                    out.push_str(content);
                    out.push('`');
                }
                other => panic!("expected Text, break, link, or code span, got {other:?}"),
            }
        }
        out
    }

    /// Whether a link node appears anywhere in the inline tree, including
    /// inside emphasis/strong content and directive bodies.
    fn contains_link(inlines: &[Inline]) -> bool {
        inlines.iter().any(|inline| match inline {
            Inline::Link { .. } => true,
            Inline::Emphasis { content, .. } | Inline::Strong { content, .. } => {
                contains_link(content)
            }
            Inline::DirectiveCall {
                body: Some(body), ..
            } => contains_link(body),
            _ => false,
        })
    }

    fn paragraph_inlines(doc: &Document) -> &[Inline] {
        match &doc.nodes[0] {
            Block::Paragraph { content, .. } => content,
            other => panic!("expected paragraph, got {other:?}"),
        }
    }

    #[test]
    fn acceptance_two_blocks_for_heading_and_paragraph() {
        let doc = parse("# Hello\n\nWorld");
        assert_eq!(doc.nodes.len(), 2);
    }

    #[test]
    fn empty_source_produces_empty_document() {
        let doc = parse("");
        assert!(doc.nodes.is_empty());
    }

    #[test]
    fn whitespace_only_source_produces_empty_document() {
        let doc = parse("  \n\t\n");
        assert!(doc.nodes.is_empty());
    }

    #[test]
    fn heading_levels_one_through_six() {
        for level in 1..=6 {
            let input = format!("{} Heading {}", "#".repeat(level), "#".repeat(level));
            let doc = parse(&input);
            match &doc.nodes[0] {
                Block::Heading {
                    level: l,
                    content,
                    span,
                } => {
                    assert_eq!(*l, level);
                    assert_text(&content[0], "Heading");
                    assert_eq!(*span, ByteSpan::new(0, input.len()));
                }
                other => panic!("expected heading, got {other:?}"),
            }
        }
    }

    #[test]
    fn heading_requires_space_after_hashes() {
        let doc = parse("#Not a heading");
        assert_text(&paragraph_inlines(&doc)[0], "#Not a heading");
    }

    #[test]
    fn seven_hashes_are_not_a_heading() {
        let doc = parse("####### x");
        assert_text(&paragraph_inlines(&doc)[0], "####### x");
    }

    #[test]
    fn heading_without_trailing_hashes() {
        let doc = parse("# Heading");
        match &doc.nodes[0] {
            Block::Heading { content, .. } => assert_text(&content[0], "Heading"),
            other => panic!("expected heading, got {other:?}"),
        }
    }

    #[test]
    fn heading_with_inline_emphasis() {
        let doc = parse("# *Hi*");
        match &doc.nodes[0] {
            Block::Heading { content, .. } => {
                assert!(matches!(content[0], Inline::Emphasis { .. }));
            }
            other => panic!("expected heading, got {other:?}"),
        }
    }

    #[test]
    fn heading_span_covers_line_and_newline() {
        let doc = parse("# Hi\nnext");
        match &doc.nodes[0] {
            Block::Heading { span, .. } => assert_eq!(*span, ByteSpan::new(0, 5)),
            other => panic!("expected heading, got {other:?}"),
        }
    }

    #[test]
    fn paragraph_joins_lines_with_soft_break() {
        let doc = parse("line one\nline two");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 3);
        assert_text(&content[0], "line one");
        assert!(matches!(content[1], Inline::SoftBreak { .. }));
        assert_text(&content[2], "line two");
    }

    #[test]
    fn hard_break_from_trailing_spaces() {
        let doc = parse("line one  \nline two");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 3);
        assert_text(&content[0], "line one");
        assert!(matches!(content[1], Inline::HardBreak { .. }));
        assert_text(&content[2], "line two");
    }

    #[test]
    fn hard_break_from_backslash() {
        let doc = parse("line one\\\nline two");
        let content = paragraph_inlines(&doc);
        assert!(matches!(content[1], Inline::HardBreak { .. }));
    }

    #[test]
    fn single_trailing_space_is_a_soft_break() {
        let doc = parse("line one \nline two");
        let content = paragraph_inlines(&doc);
        assert!(matches!(content[1], Inline::SoftBreak { .. }));
    }

    #[test]
    fn emphasis_star_and_underscore() {
        for input in ["*italic*", "_italic_"] {
            let doc = parse(input);
            let content = paragraph_inlines(&doc);
            assert_eq!(content.len(), 1, "input: {input}");
            match &content[0] {
                Inline::Emphasis { content, span } => {
                    assert_text(&content[0], "italic");
                    assert_eq!(*span, ByteSpan::new(0, input.len()));
                }
                other => panic!("expected emphasis, got {other:?}"),
            }
        }
    }

    #[test]
    fn strong_star_and_underscore() {
        for input in ["**bold**", "__bold__"] {
            let doc = parse(input);
            let content = paragraph_inlines(&doc);
            assert_eq!(content.len(), 1, "input: {input}");
            match &content[0] {
                Inline::Strong { content, span } => {
                    assert_text(&content[0], "bold");
                    assert_eq!(*span, ByteSpan::new(0, input.len()));
                }
                other => panic!("expected strong, got {other:?}"),
            }
        }
    }

    #[test]
    fn link_basic() {
        let doc = parse("[example](https://example.com)");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 1);
        match &content[0] {
            Inline::Link {
                content,
                destination,
                span,
            } => {
                assert_text(&content[0], "example");
                assert_eq!(destination, "https://example.com");
                assert_eq!(*span, ByteSpan::new(0, 30));
            }
            other => panic!("expected link, got {other:?}"),
        }
    }

    #[test]
    fn link_inside_sentence() {
        let doc = parse("See [example](https://example.com) now.");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 3);
        assert_text(&content[0], "See ");
        match &content[1] {
            Inline::Link {
                content,
                destination,
                span,
            } => {
                assert_text(&content[0], "example");
                assert_eq!(destination, "https://example.com");
                assert_eq!(*span, ByteSpan::new(4, 34));
            }
            other => panic!("expected link, got {other:?}"),
        }
        assert_text(&content[2], " now.");
    }

    #[test]
    fn label_with_strong_and_text() {
        let doc = parse("[**bold** text](https://example.com)");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 1);
        match &content[0] {
            Inline::Link {
                content,
                destination,
                span,
            } => {
                assert_eq!(destination, "https://example.com");
                assert_eq!(*span, ByteSpan::new(0, 36));
                assert_eq!(content.len(), 2);
                match &content[0] {
                    Inline::Strong { content, .. } => assert_text(&content[0], "bold"),
                    other => panic!("expected strong, got {other:?}"),
                }
                assert_text(&content[1], " text");
            }
            other => panic!("expected link, got {other:?}"),
        }
    }

    #[test]
    fn label_with_quarkdown_inline_call() {
        let doc = parse("[.strong {hello}](https://example.com)");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 1);
        match &content[0] {
            Inline::Link {
                content,
                destination,
                span,
            } => {
                assert_eq!(destination, "https://example.com");
                assert_eq!(*span, ByteSpan::new(0, 38));
                assert_eq!(content.len(), 1);
                match &content[0] {
                    Inline::DirectiveCall {
                        name,
                        positional_args,
                        ..
                    } => {
                        assert_eq!(name, "strong");
                        assert_eq!(positional_args.len(), 1);
                        assert_eq!(positional_args[0], Value::Identifier("hello".into()));
                    }
                    other => panic!("expected directive call, got {other:?}"),
                }
            }
            other => panic!("expected link, got {other:?}"),
        }
    }

    #[test]
    fn link_fragment_and_relative_destinations() {
        for (input, dest) in [
            ("[section](#intro)", "#intro"),
            ("[file](docs/page.html)", "docs/page.html"),
            ("[guide](./guide.html)", "./guide.html"),
            ("[up](../assets/x.pdf)", "../assets/x.pdf"),
            ("[root](/absolute/path)", "/absolute/path"),
        ] {
            let doc = parse(input);
            let content = paragraph_inlines(&doc);
            assert_eq!(content.len(), 1, "input: {input}");
            match &content[0] {
                Inline::Link {
                    content,
                    destination,
                    span,
                } => {
                    assert_text(&content[0], &input[1..input.len() - (dest.len() + 3)]);
                    assert_eq!(destination, dest, "input: {input}");
                    assert_eq!(*span, ByteSpan::new(0, input.len()), "input: {input}");
                }
                other => panic!("expected link, got {other:?}"),
            }
        }
    }

    #[test]
    fn link_unicode_label() {
        let input = "[문서](https://example.com)";
        let doc = parse(input);
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 1);
        match &content[0] {
            Inline::Link {
                content,
                destination,
                span,
            } => {
                assert_eq!(content.len(), 1);
                match &content[0] {
                    Inline::Text { content, span } => {
                        assert_eq!(content, "문서");
                        assert_eq!(*span, ByteSpan::new(1, 7));
                    }
                    other => panic!("expected text, got {other:?}"),
                }
                assert_eq!(destination, "https://example.com");
                assert_eq!(*span, ByteSpan::new(0, input.len()));
            }
            other => panic!("expected link, got {other:?}"),
        }
    }

    #[test]
    fn two_links_keep_ordering() {
        let doc = parse("[a](https://a.example) and [b](https://b.example)");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 3);
        match &content[0] {
            Inline::Link {
                destination, span, ..
            } => {
                assert_eq!(destination, "https://a.example");
                assert_eq!(*span, ByteSpan::new(0, 22));
            }
            other => panic!("expected link, got {other:?}"),
        }
        assert_text(&content[1], " and ");
        match &content[2] {
            Inline::Link {
                destination, span, ..
            } => {
                assert_eq!(destination, "https://b.example");
                assert_eq!(*span, ByteSpan::new(27, 49));
            }
            other => panic!("expected link, got {other:?}"),
        }
    }

    #[test]
    fn link_spans_preserve_inner_offsets() {
        let doc = parse("before [hello **world**](https://example.com) after");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 3);
        assert_text(&content[0], "before ");
        match &content[1] {
            Inline::Link {
                content,
                destination,
                span,
            } => {
                assert_eq!(*span, ByteSpan::new(7, 45));
                assert_eq!(destination, "https://example.com");
                assert_eq!(content.len(), 2);
                match &content[0] {
                    Inline::Text { content, span } => {
                        assert_eq!(content, "hello ");
                        assert_eq!(*span, ByteSpan::new(8, 14));
                    }
                    other => panic!("expected text, got {other:?}"),
                }
                match &content[1] {
                    Inline::Strong { content, span } => {
                        assert_eq!(*span, ByteSpan::new(14, 23));
                        assert_eq!(content.len(), 1);
                        match &content[0] {
                            Inline::Text { content, span } => {
                                assert_eq!(content, "world");
                                assert_eq!(*span, ByteSpan::new(16, 21));
                            }
                            other => panic!("expected text, got {other:?}"),
                        }
                    }
                    other => panic!("expected strong, got {other:?}"),
                }
            }
            other => panic!("expected link, got {other:?}"),
        }
        assert_text(&content[2], " after");
    }

    #[test]
    fn link_destination_with_balanced_parens() {
        let doc = parse("[x](a(b)c)");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 1);
        match &content[0] {
            Inline::Link { destination, .. } => assert_eq!(destination, "a(b)c"),
            other => panic!("expected link, got {other:?}"),
        }
    }

    #[test]
    fn malformed_links_recover_as_literal_text() {
        for input in [
            "[text](",
            "[text](url",
            "[text]",
            "[](url)",
            "[text]()",
            "[text]( )",
            "[text](url \"title\")",
            "[text](a b)",
            "[",
            "[text",
        ] {
            let doc = parse(input);
            let content = paragraph_inlines(&doc);
            assert_eq!(joined_text(content), input, "input: {input}");
            assert!(!contains_link(content), "input: {input}");
        }
    }

    #[test]
    fn image_syntax_is_not_a_link() {
        let input = "![alt](image.png)";
        let doc = parse(input);
        let content = paragraph_inlines(&doc);
        assert_eq!(joined_text(content), input);
        assert!(!contains_link(content));
    }

    #[test]
    fn nested_bracket_label_ends_at_first_bracket() {
        let doc = parse("[a [b](c)](d)");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 2);
        match &content[0] {
            Inline::Link {
                content,
                destination,
                ..
            } => {
                assert_eq!(destination, "c");
                assert_eq!(joined_text(content), "a [b");
            }
            other => panic!("expected link, got {other:?}"),
        }
        assert_text(&content[1], "](d)");
    }

    #[test]
    fn nested_emphasis_inside_strong() {
        let doc = parse("**outer *inner* end**");
        let content = paragraph_inlines(&doc);
        match &content[0] {
            Inline::Strong { content, .. } => {
                assert_text(&content[0], "outer ");
                match &content[1] {
                    Inline::Emphasis { content, .. } => assert_text(&content[0], "inner"),
                    other => panic!("expected emphasis, got {other:?}"),
                }
                assert_text(&content[2], " end");
            }
            other => panic!("expected strong, got {other:?}"),
        }
    }

    #[test]
    fn adjacent_text_nodes_join_to_source() {
        let doc = parse("***not emphasized***");
        assert_eq!(joined_text(paragraph_inlines(&doc)), "***not emphasized***");
    }

    #[test]
    fn underscore_inside_word_is_literal() {
        let doc = parse("foo_bar_baz");
        let content = paragraph_inlines(&doc);
        assert_eq!(joined_text(content), "foo_bar_baz");
        assert!(content.iter().all(|i| matches!(i, Inline::Text { .. })));
    }

    #[test]
    fn empty_delimiters_are_literal() {
        let doc = parse("** ** and * *");
        let content = paragraph_inlines(&doc);
        assert_eq!(joined_text(content), "** ** and * *");
        assert!(content.iter().all(|i| matches!(i, Inline::Text { .. })));
    }

    #[test]
    fn unclosed_emphasis_does_not_panic() {
        let doc = parse("*unclosed");
        assert_eq!(joined_text(paragraph_inlines(&doc)), "*unclosed");
    }

    #[test]
    fn unclosed_strong_does_not_panic() {
        let doc = parse("**unclosed");
        assert_eq!(joined_text(paragraph_inlines(&doc)), "**unclosed");
    }

    #[test]
    fn unordered_list_single_item() {
        let doc = parse("- item");
        match &doc.nodes[0] {
            Block::UnorderedList { items, span } => {
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].content.len(), 1);
                assert_eq!(items[0].span, ByteSpan::new(0, 6));
                assert_eq!(*span, ByteSpan::new(0, 6));
            }
            other => panic!("expected list, got {other:?}"),
        }
    }

    #[test]
    fn unordered_list_multiple_items() {
        let doc = parse("- one\n- two\n- three");
        match &doc.nodes[0] {
            Block::UnorderedList { items, .. } => {
                assert_eq!(items.len(), 3);
                for (i, item) in items.iter().enumerate() {
                    match &item.content[0] {
                        Block::Paragraph { content, .. } => {
                            assert_text(&content[0], ["one", "two", "three"][i]);
                        }
                        other => panic!("expected paragraph, got {other:?}"),
                    }
                }
            }
            other => panic!("expected list, got {other:?}"),
        }
    }

    #[test]
    fn all_marker_characters_start_lists() {
        for marker in ['-', '*', '+'] {
            let doc = parse(&format!("{marker} item"));
            match &doc.nodes[0] {
                Block::UnorderedList { items, .. } => assert_eq!(items.len(), 1),
                other => panic!("expected list, got {other:?}"),
            }
        }
    }

    #[test]
    fn marker_without_space_is_a_paragraph() {
        let doc = parse("-item\n*item");
        assert_text(&paragraph_inlines(&doc)[0], "-item");
    }

    #[test]
    fn different_markers_split_into_separate_lists() {
        let doc = parse("- one\n* two");
        assert_eq!(doc.nodes.len(), 2);
        for node in &doc.nodes {
            assert!(matches!(node, Block::UnorderedList { .. }));
        }
    }

    #[test]
    fn item_continuation_lines_join_paragraph() {
        let doc = parse("- first line\n  second line");
        match &doc.nodes[0] {
            Block::UnorderedList { items, .. } => match &items[0].content[0] {
                Block::Paragraph { content, .. } => {
                    assert_eq!(content.len(), 3);
                    assert_text(&content[0], "first line");
                    assert!(matches!(content[1], Inline::SoftBreak { .. }));
                    assert_text(&content[2], "second line");
                }
                other => panic!("expected paragraph, got {other:?}"),
            },
            other => panic!("expected list, got {other:?}"),
        }
    }

    #[test]
    fn nested_list() {
        let doc = parse("- outer\n  - inner");
        match &doc.nodes[0] {
            Block::UnorderedList { items, .. } => {
                assert_eq!(items.len(), 1);
                match &items[0].content[1] {
                    Block::UnorderedList { items, .. } => assert_eq!(items.len(), 1),
                    other => panic!("expected nested list, got {other:?}"),
                }
            }
            other => panic!("expected list, got {other:?}"),
        }
    }

    #[test]
    fn blank_lines_between_items_keep_single_list() {
        let doc = parse("- one\n\n- two");
        match &doc.nodes[0] {
            Block::UnorderedList { items, .. } => assert_eq!(items.len(), 2),
            other => panic!("expected list, got {other:?}"),
        }
    }

    #[test]
    fn item_blank_line_before_continuation() {
        let doc = parse("- one\n\n  continued");
        match &doc.nodes[0] {
            Block::UnorderedList { items, .. } => assert_eq!(items[0].content.len(), 2),
            other => panic!("expected list, got {other:?}"),
        }
    }

    #[test]
    fn unindented_line_after_item_ends_list() {
        let doc = parse("- one\nplain");
        assert_eq!(doc.nodes.len(), 2);
        assert!(matches!(doc.nodes[0], Block::UnorderedList { .. }));
        assert!(matches!(doc.nodes[1], Block::Paragraph { .. }));
    }

    #[test]
    fn item_containing_code_block() {
        let doc = parse("- item\n  ```\n  code\n  ```");
        match &doc.nodes[0] {
            Block::UnorderedList { items, .. } => match &items[0].content[1] {
                Block::CodeBlock { source, .. } => assert_eq!(source, "code"),
                other => panic!("expected code block, got {other:?}"),
            },
            other => panic!("expected list, got {other:?}"),
        }
    }

    #[test]
    fn ordered_list_single_item() {
        let doc = parse("1. item");
        match &doc.nodes[0] {
            Block::OrderedList { items, start, span } => {
                assert_eq!(items.len(), 1);
                assert_eq!(*start, 1);
                assert_eq!(items[0].content.len(), 1);
                assert_eq!(items[0].span, ByteSpan::new(0, 7));
                assert_eq!(*span, ByteSpan::new(0, 7));
            }
            other => panic!("expected ordered list, got {other:?}"),
        }
    }

    #[test]
    fn ordered_list_multiple_items() {
        let doc = parse("1. one\n2. two\n3. three");
        match &doc.nodes[0] {
            Block::OrderedList { items, start, .. } => {
                assert_eq!(items.len(), 3);
                assert_eq!(*start, 1);
                for (i, item) in items.iter().enumerate() {
                    match &item.content[0] {
                        Block::Paragraph { content, .. } => {
                            assert_text(&content[0], ["one", "two", "three"][i]);
                        }
                        other => panic!("expected paragraph, got {other:?}"),
                    }
                }
            }
            other => panic!("expected ordered list, got {other:?}"),
        }
    }

    #[test]
    fn ordered_list_non_one_start() {
        let doc = parse("3. three\n4. four");
        match &doc.nodes[0] {
            Block::OrderedList { items, start, .. } => {
                assert_eq!(items.len(), 2);
                assert_eq!(*start, 3);
            }
            other => panic!("expected ordered list, got {other:?}"),
        }
    }

    #[test]
    fn ordered_list_parentheses_marker() {
        let doc = parse("1) one\n2) two");
        match &doc.nodes[0] {
            Block::OrderedList { items, start, .. } => {
                assert_eq!(items.len(), 2);
                assert_eq!(*start, 1);
            }
            other => panic!("expected ordered list, got {other:?}"),
        }
    }

    #[test]
    fn ordered_list_without_space_is_paragraph() {
        let doc = parse("1.item\n2.item");
        assert_text(&paragraph_inlines(&doc)[0], "1.item");
    }

    #[test]
    fn nested_ordered_list() {
        // Content column is derived from each item's own marker; "1. " puts
        // content at column 4, so the nested item needs 4 leading spaces
        let doc = parse("1. outer\n   1. inner");
        match &doc.nodes[0] {
            Block::OrderedList { items, .. } => {
                assert_eq!(items.len(), 1);
                match &items[0].content[1] {
                    Block::OrderedList { items, .. } => assert_eq!(items.len(), 1),
                    other => panic!("expected nested ordered list, got {other:?}"),
                }
            }
            other => panic!("expected ordered list, got {other:?}"),
        }
    }

    #[test]
    fn ordered_then_unordered_nested() {
        // Content column is derived from each item's own marker; "1. " puts
        // content at column 4, so the nested items need 4 leading spaces
        let doc = parse("1. ordered\n    - unordered\n    - another");
        match &doc.nodes[0] {
            Block::OrderedList { items, .. } => {
                assert_eq!(items.len(), 1);
                match &items[0].content[1] {
                    Block::UnorderedList { items, .. } => assert_eq!(items.len(), 2),
                    other => panic!("expected nested unordered list, got {other:?}"),
                }
            }
            other => panic!("expected ordered list, got {other:?}"),
        }
    }
    #[test]
    fn unordered_then_ordered_nested() {
        // Content column is derived from each item's own marker; "- " puts
        // content at column 2, so 2+ spaces suffice for the nested items
        let doc = parse("- unordered\n  1. ordered\n  2. ordered");
        match &doc.nodes[0] {
            Block::UnorderedList { items, .. } => {
                assert_eq!(items.len(), 1);
                match &items[0].content[1] {
                    Block::OrderedList { items, .. } => assert_eq!(items.len(), 2),
                    other => panic!("expected nested ordered list, got {other:?}"),
                }
            }
            other => panic!("expected unordered list, got {other:?}"),
        }
    }

    #[test]
    fn number_followed_by_text_is_paragraph() {
        let doc = parse("123abc");
        assert_text(&paragraph_inlines(&doc)[0], "123abc");
    }

    #[test]
    fn decimal_number_is_paragraph() {
        let doc = parse("1.23");
        assert_text(&paragraph_inlines(&doc)[0], "1.23");
    }

    #[test]
    fn blank_lines_between_ordered_items() {
        let doc = parse("1. one\n\n2. two");
        match &doc.nodes[0] {
            Block::OrderedList { items, .. } => assert_eq!(items.len(), 2),
            other => panic!("expected ordered list, got {other:?}"),
        }
    }

    #[test]
    fn ordered_list_item_continuation() {
        // Content column is 3 for ordered lists
        let doc = parse("1. first line\n   second line");
        match &doc.nodes[0] {
            Block::OrderedList { items, .. } => match &items[0].content[0] {
                Block::Paragraph { content, .. } => {
                    assert_eq!(content.len(), 3);
                    assert_text(&content[0], "first line");
                    assert!(matches!(content[1], Inline::SoftBreak { .. }));
                    assert_text(&content[2], "second line");
                }
                other => panic!("expected paragraph, got {other:?}"),
            },
            other => panic!("expected ordered list, got {other:?}"),
        }
    }

    #[test]
    fn ordered_list_with_code_block() {
        // Content column is 3 for ordered lists
        let doc = parse("1. item\n   ```\n   code\n   ```");
        match &doc.nodes[0] {
            Block::OrderedList { items, .. } => match &items[0].content[1] {
                Block::CodeBlock { source, .. } => assert_eq!(source, "code"),
                other => panic!("expected code block, got {other:?}"),
            },
            other => panic!("expected ordered list, got {other:?}"),
        }
    }

    #[test]
    fn ordered_list_source_spans() {
        let doc = parse("1. first\n2. second");
        match &doc.nodes[0] {
            Block::OrderedList { items, span, .. } => {
                // First item "1. first" (8 bytes) + newline (1 byte) = 0..9
                // Second item "2. second" (9 bytes) = 9..18
                assert_eq!(items[0].span, ByteSpan::new(0, 9));
                assert_eq!(items[1].span, ByteSpan::new(9, 18));
                assert_eq!(*span, ByteSpan::new(0, 18));
                assert_eq!(items[0].span, ByteSpan::new(0, 9));
                assert_eq!(items[1].span, ByteSpan::new(9, 18));
                assert_eq!(*span, ByteSpan::new(0, 18));
            }
            other => panic!("expected ordered list, got {other:?}"),
        }
    }

    #[test]
    fn ordered_list_repeated_marker_numbers() {
        // Repeated marker numbers form one list; only first ordinal determines start
        let doc = parse("1. A\n1. B\n1. C");
        match &doc.nodes[0] {
            Block::OrderedList { items, start, .. } => {
                assert_eq!(items.len(), 3);
                assert_eq!(*start, 1);
            }
            other => panic!("expected ordered list, got {other:?}"),
        }
    }

    #[test]
    fn ordered_list_arbitrary_subsequent_numbers() {
        // Non-sequential source ordinals form one list; only first ordinal determines start
        let doc = parse("3. A\n8. B\n42. C");
        match &doc.nodes[0] {
            Block::OrderedList { items, start, .. } => {
                assert_eq!(items.len(), 3);
                assert_eq!(*start, 3);
            }
            other => panic!("expected ordered list, got {other:?}"),
        }
    }

    #[test]
    fn ordered_list_delimiter_boundary_dot_then_paren() {
        // Different delimiters break the list
        let doc = parse("1. A\n2) B");
        assert_eq!(doc.nodes.len(), 2);
        assert!(matches!(doc.nodes[0], Block::OrderedList { .. }));
        assert!(matches!(doc.nodes[1], Block::OrderedList { .. }));
    }

    #[test]
    fn ordered_list_delimiter_boundary_paren_then_dot() {
        // Different delimiters break the list (reverse)
        let doc = parse("1) A\n2. B");
        assert_eq!(doc.nodes.len(), 2);
        assert!(matches!(doc.nodes[0], Block::OrderedList { .. }));
        assert!(matches!(doc.nodes[1], Block::OrderedList { .. }));
    }

    #[test]
    fn ordered_list_same_parenthesis_delimiter() {
        // Same parenthesis delimiter forms one list
        let doc = parse("3) A\n9) B");
        match &doc.nodes[0] {
            Block::OrderedList { items, start, .. } => {
                assert_eq!(items.len(), 2);
                assert_eq!(*start, 3);
            }
            other => panic!("expected ordered list, got {other:?}"),
        }
    }

    #[test]
    fn ordered_list_marker_width_transition() {
        // 9. and 10. have different marker widths; continuation content uses item's own column
        // Continuation lines are joined into the same paragraph with soft breaks
        let doc = parse("9. parent\n10. second\n    nested content");
        match &doc.nodes[0] {
            Block::OrderedList { items, .. } => {
                assert_eq!(items.len(), 2);
                // Second item should have "second" and "nested content" in same paragraph with soft break
                match &items[1].content[0] {
                    Block::Paragraph { content, .. } => {
                        assert_eq!(content.len(), 3);
                        assert_text(&content[0], "second");
                        assert!(matches!(content[1], Inline::SoftBreak { .. }));
                        assert_text(&content[2], "nested content");
                    }
                    other => panic!("expected paragraph with soft break, got {other:?}"),
                }
            }
            other => panic!("expected ordered list, got {other:?}"),
        }
    }

    #[test]
    fn ordered_list_digit_limit_9_digits() {
        // 9 digits should be recognized as ordered list marker
        let doc = parse("123456789. item");
        assert!(matches!(doc.nodes[0], Block::OrderedList { .. }));
    }

    #[test]
    fn ordered_list_digit_limit_10_digits() {
        // 10 digits should NOT be recognized as ordered list marker (paragraph)
        let doc = parse("1234567890. item");
        assert_text(&paragraph_inlines(&doc)[0], "1234567890. item");
    }

    #[test]
    fn nested_ordered_list_hierarchy() {
        // 1. parent
        //    1. child
        //    2. child
        // 2. sibling
        let doc = parse("1. parent\n   1. child\n   2. child\n2. sibling");
        match &doc.nodes[0] {
            Block::OrderedList { items, .. } => {
                assert_eq!(items.len(), 2);
                // First item has nested ordered list with 2 items
                match &items[0].content[1] {
                    Block::OrderedList { items, .. } => assert_eq!(items.len(), 2),
                    other => panic!("expected nested ordered list, got {other:?}"),
                }
                // Second item is sibling
                assert_eq!(items[1].content.len(), 1);
            }
            other => panic!("expected ordered list, got {other:?}"),
        }
    }

    #[test]
    fn mixed_nesting_ordered_then_unordered() {
        // 1. ordered
        //    - unordered
        //    - another
        // 2. ordered
        let doc = parse("1. ordered\n   - unordered\n   - another\n2. ordered");
        match &doc.nodes[0] {
            Block::OrderedList { items, .. } => {
                assert_eq!(items.len(), 2);
                match &items[0].content[1] {
                    Block::UnorderedList { items, .. } => assert_eq!(items.len(), 2),
                    other => panic!("expected nested unordered list, got {other:?}"),
                }
            }
            other => panic!("expected ordered list, got {other:?}"),
        }
    }

    #[test]
    fn mixed_nesting_unordered_then_ordered() {
        // - unordered
        //   1. ordered
        //   2. ordered
        // - unordered
        let doc = parse("- unordered\n  1. ordered\n  2. ordered\n- unordered");
        match &doc.nodes[0] {
            Block::UnorderedList { items, .. } => {
                assert_eq!(items.len(), 2);
                match &items[0].content[1] {
                    Block::OrderedList { items, .. } => assert_eq!(items.len(), 2),
                    other => panic!("expected nested ordered list, got {other:?}"),
                }
            }
            other => panic!("expected unordered list, got {other:?}"),
        }
    }

    #[test]
    fn fenced_code_block_with_language() {
        let doc = parse("```rust\nfn main() {}\n```");
        match &doc.nodes[0] {
            Block::CodeBlock {
                language,
                source,
                span,
            } => {
                assert_eq!(language.as_deref(), Some("rust"));
                assert_eq!(source, "fn main() {}");
                assert_eq!(*span, ByteSpan::new(0, 24));
            }
            other => panic!("expected code block, got {other:?}"),
        }
    }

    #[test]
    fn fenced_code_block_preserves_indentation() {
        let doc = parse("```\n  indented\n    deeper\n```");
        match &doc.nodes[0] {
            Block::CodeBlock { source, .. } => assert_eq!(source, "  indented\n    deeper"),
            other => panic!("expected code block, got {other:?}"),
        }
    }

    #[test]
    fn unclosed_code_block_runs_to_end() {
        let doc = parse("```\nnever closed");
        match &doc.nodes[0] {
            Block::CodeBlock { source, .. } => assert_eq!(source, "never closed"),
            other => panic!("expected code block, got {other:?}"),
        }
    }

    #[test]
    fn closing_fence_shorter_than_opening_does_not_close() {
        let doc = parse("````\n```\nstill code\n````");
        match &doc.nodes[0] {
            Block::CodeBlock { source, .. } => assert_eq!(source, "```\nstill code"),
            other => panic!("expected code block, got {other:?}"),
        }
    }

    #[test]
    fn closing_fence_cannot_have_info_string() {
        let doc = parse("```\ncode\n```rust\nmore\n```");
        match &doc.nodes[0] {
            Block::CodeBlock { source, .. } => assert_eq!(source, "code\n```rust\nmore"),
            other => panic!("expected code block, got {other:?}"),
        }
    }

    #[test]
    fn thematic_break_variants() {
        for input in ["---", "***", "___", "- - -", "* * *", "--- --- ---"] {
            let doc = parse(input);
            assert!(
                matches!(doc.nodes[0], Block::ThematicBreak { .. }),
                "input: {input}"
            );
        }
    }

    #[test]
    fn two_dashes_are_not_a_thematic_break() {
        let doc = parse("--");
        assert!(matches!(doc.nodes[0], Block::Paragraph { .. }));
    }

    #[test]
    fn mixed_markers_are_not_a_thematic_break() {
        let doc = parse("- -*");
        assert!(matches!(doc.nodes[0], Block::UnorderedList { .. }));
    }

    #[test]
    fn thematic_break_before_list_marker() {
        let doc = parse("- - -");
        assert!(matches!(doc.nodes[0], Block::ThematicBreak { .. }));
    }

    #[test]
    fn paragraph_ends_at_thematic_break() {
        let doc = parse("text\n---");
        assert_eq!(doc.nodes.len(), 2);
        assert!(matches!(doc.nodes[0], Block::Paragraph { .. }));
        assert!(matches!(doc.nodes[1], Block::ThematicBreak { .. }));
    }

    #[test]
    fn crlf_input_is_normalized() {
        let doc = parse("# Hi\r\n\r\nWorld\r\n");
        assert_eq!(doc.nodes.len(), 2);
        match &doc.nodes[0] {
            Block::Heading { content, .. } => assert_text(&content[0], "Hi"),
            other => panic!("expected heading, got {other:?}"),
        }
    }

    #[test]
    fn multibyte_spans_are_byte_accurate() {
        let doc = parse("# 한국어 제목\n\n본문 내용");
        match &doc.nodes[0] {
            Block::Heading { content, span, .. } => {
                assert_text(&content[0], "한국어 제목");
                assert_eq!(*span, ByteSpan::new(0, 19));
            }
            other => panic!("expected heading, got {other:?}"),
        }
        match &doc.nodes[1] {
            Block::Paragraph { span, .. } => assert_eq!(*span, ByteSpan::new(20, 33)),
            other => panic!("expected paragraph, got {other:?}"),
        }
    }

    #[test]
    fn break_spans_after_multibyte_are_byte_accurate() {
        let doc = parse("안녕\n세상");
        let content = paragraph_inlines(&doc);
        assert_text(&content[0], "안녕");
        match &content[1] {
            Inline::SoftBreak { span } => {
                // "안녕" is 6 UTF-8 bytes; the newline starts at byte offset 6.
                assert_eq!(*span, ByteSpan::new(6, 7));
            }
            other => panic!("expected SoftBreak, got {other:?}"),
        }
        assert_text(&content[2], "세상");

        let doc = parse("안녕  \n세상");
        let content = paragraph_inlines(&doc);
        assert_text(&content[0], "안녕");
        match &content[1] {
            Inline::HardBreak { span } => {
                // Two trailing spaces occupy bytes 6..8, newline at byte 8.
                assert_eq!(*span, ByteSpan::new(6, 9));
            }
            other => panic!("expected HardBreak, got {other:?}"),
        }
        assert_text(&content[2], "세상");
    }

    #[test]
    fn emphasis_with_unicode_content() {
        let doc = parse("*강조*");
        let content = paragraph_inlines(&doc);
        match &content[0] {
            Inline::Emphasis { content, span } => {
                assert_text(&content[0], "강조");
                assert_eq!(*span, ByteSpan::new(0, 8));
            }
            other => panic!("expected emphasis, got {other:?}"),
        }
    }

    #[test]
    fn line_count_tracks_newlines() {
        assert_eq!(parse("").line_count, 1);
        assert_eq!(parse("a\nb\nc").line_count, 3);
        assert_eq!(parse("a\n").line_count, 2);
    }

    #[test]
    fn deeply_nested_lists_do_not_panic() {
        let mut input = String::from("- top\n");
        for _ in 0..200 {
            input.push_str("  - nested\n");
        }
        let doc = parse(&input);
        assert_eq!(doc.nodes.len(), 1);
        assert!(matches!(doc.nodes[0], Block::UnorderedList { .. }));
    }

    #[test]
    fn deeply_nested_emphasis_does_not_panic() {
        let mut input = String::new();
        for _ in 0..200 {
            input.push_str("*a ");
        }
        input.push('x');
        for _ in 0..200 {
            input.push_str(" a*");
        }
        let doc = parse(&input);
        assert_eq!(doc.nodes.len(), 1);
    }

    #[test]
    fn code_span_basic() {
        let doc = parse("`code`");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 1);
        match &content[0] {
            Inline::Code { content, span } => {
                assert_eq!(content, "code");
                assert_eq!(*span, ByteSpan::new(0, 6));
            }
            other => panic!("expected code span, got {other:?}"),
        }
    }

    #[test]
    fn code_span_embedded_in_prose() {
        let doc = parse("Use `cargo test` before pushing.");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 3);
        assert_text(&content[0], "Use ");
        match &content[1] {
            Inline::Code { content, span } => {
                assert_eq!(content, "cargo test");
                assert_eq!(*span, ByteSpan::new(4, 16));
            }
            other => panic!("expected code span, got {other:?}"),
        }
        assert_text(&content[2], " before pushing.");
    }

    #[test]
    fn multiple_code_spans_keep_ordering() {
        let doc = parse("`foo` and `bar`");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 3);
        match &content[0] {
            Inline::Code { content, span } => {
                assert_eq!(content, "foo");
                assert_eq!(*span, ByteSpan::new(0, 5));
            }
            other => panic!("expected code span, got {other:?}"),
        }
        assert_text(&content[1], " and ");
        match &content[2] {
            Inline::Code { content, span } => {
                assert_eq!(content, "bar");
                assert_eq!(*span, ByteSpan::new(10, 15));
            }
            other => panic!("expected code span, got {other:?}"),
        }
    }

    #[test]
    fn code_span_empty_and_minimal_constructs() {
        // A single backtick run without a closer stays literal.
        let doc = parse("`");
        assert_eq!(joined_text(paragraph_inlines(&doc)), "`");
        // Two adjacent backticks form one maximal run; nothing closes it.
        let doc = parse("``");
        assert_eq!(joined_text(paragraph_inlines(&doc)), "``");
        // A run followed only by a run of different length never closes.
        let doc = parse("` ``");
        assert_eq!(joined_text(paragraph_inlines(&doc)), "` ``");
        // Content containing a backtick keeps it when the delimiters are
        // longer; joined_text renders the node back with its delimiters.
        let doc = parse("`` ` ``");
        let content = paragraph_inlines(&doc);
        match &content[0] {
            Inline::Code { content, span } => {
                assert_eq!(content, "`");
                assert_eq!(*span, ByteSpan::new(0, 7));
            }
            other => panic!("expected code span, got {other:?}"),
        }
        // A single space between same-length delimiters is preserved
        // (all-space content keeps its spaces).
        let doc = parse("`` ``");
        let content = paragraph_inlines(&doc);
        match &content[0] {
            Inline::Code { content, .. } => assert_eq!(content, " "),
            other => panic!("expected code span, got {other:?}"),
        }
        let doc = parse("` `");
        let content = paragraph_inlines(&doc);
        match &content[0] {
            Inline::Code { content, .. } => assert_eq!(content, " "),
            other => panic!("expected code span, got {other:?}"),
        }
    }

    #[test]
    fn code_span_variable_length_delimiters() {
        let doc = parse("``foo ` bar``");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 1);
        match &content[0] {
            Inline::Code { content, span } => {
                assert_eq!(content, "foo ` bar");
                assert_eq!(*span, ByteSpan::new(0, 13));
            }
            other => panic!("expected code span, got {other:?}"),
        }
        // Longer delimiters protect runs of any other length. (Triple-backtick
        // input must not start at line start, where it is a fenced code block.)
        let doc = parse("x ```code ` `` ``` y");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 3);
        match &content[1] {
            Inline::Code { content, span } => {
                assert_eq!(content, "code ` `` ");
                assert_eq!(*span, ByteSpan::new(2, 18));
            }
            other => panic!("expected code span, got {other:?}"),
        }
    }

    #[test]
    fn code_span_mismatched_delimiter_lengths_do_not_close() {
        // `foo``bar`: the run of two backticks does not close the single
        // backtick span; the final single backtick does.
        let doc = parse("`foo``bar`");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 1);
        match &content[0] {
            Inline::Code { content, .. } => assert_eq!(content, "foo``bar"),
            other => panic!("expected code span, got {other:?}"),
        }
        // ``foo`bar``: the single backtick inside is literal content.
        let doc = parse("``foo`bar``");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 1);
        match &content[0] {
            Inline::Code { content, .. } => assert_eq!(content, "foo`bar"),
            other => panic!("expected code span, got {other:?}"),
        }
    }

    #[test]
    fn code_span_keeps_markdown_literal() {
        let doc = parse("`**bold**`");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 1);
        match &content[0] {
            Inline::Code { content, .. } => assert_eq!(content, "**bold**"),
            other => panic!("expected code span, got {other:?}"),
        }
    }

    #[test]
    fn code_span_keeps_link_literal() {
        let doc = parse("`[link](https://example.com)`");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 1);
        match &content[0] {
            Inline::Code { content, .. } => {
                assert_eq!(content, "[link](https://example.com)")
            }
            other => panic!("expected code span, got {other:?}"),
        }
    }

    #[test]
    fn code_span_keeps_quarkdown_call_literal() {
        let doc = parse("`.strong {hello}`");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 1);
        match &content[0] {
            Inline::Code { content, .. } => assert_eq!(content, ".strong {hello}"),
            other => panic!("expected code span, got {other:?}"),
        }
    }

    #[test]
    fn code_span_backslashes_are_literal() {
        let doc = parse("`a\\bc`");
        let content = paragraph_inlines(&doc);
        match &content[0] {
            Inline::Code { content, .. } => assert_eq!(content, "a\\bc"),
            other => panic!("expected code span, got {other:?}"),
        }
        // A backslash-space must not become an escape or a hard break.
        let doc = parse("`a\\ b`");
        let content = paragraph_inlines(&doc);
        match &content[0] {
            Inline::Code { content, .. } => assert_eq!(content, "a\\ b"),
            other => panic!("expected code span, got {other:?}"),
        }
    }

    #[test]
    fn code_span_multiline_becomes_single_space() {
        let doc = parse("`foo\nbar`");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 1);
        match &content[0] {
            Inline::Code { content, span } => {
                assert_eq!(content, "foo bar");
                assert_eq!(*span, ByteSpan::new(0, 9));
            }
            other => panic!("expected code span, got {other:?}"),
        }
        let doc = parse("`foo\r\nbar`");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 1);
        match &content[0] {
            Inline::Code { content, .. } => assert_eq!(content, "foo bar"),
            other => panic!("expected code span, got {other:?}"),
        }
    }

    #[test]
    fn code_span_surrounding_space_normalization() {
        let cases = [
            ("` foo `", "foo"),
            ("`  foo  `", " foo "),
            ("`  `", "  "),
            ("` `", " "),
            ("`foo `", "foo "),
            ("` foo`", " foo"),
            ("`\u{a0}foo\u{a0}`", "\u{a0}foo\u{a0}"),
        ];
        for (input, expected) in cases {
            let doc = parse(input);
            let content = paragraph_inlines(&doc);
            assert_eq!(content.len(), 1, "input {input:?}");
            match &content[0] {
                Inline::Code { content, .. } => {
                    assert_eq!(content, expected, "input {input:?}")
                }
                other => panic!("input {input:?}: expected code span, got {other:?}"),
            }
        }
    }

    #[test]
    fn code_span_unicode_content_and_spans() {
        let doc = parse("`한글 λ Rust 🦀`");
        let content = paragraph_inlines(&doc);
        match &content[0] {
            Inline::Code { content, span } => {
                assert_eq!(content, "한글 λ Rust 🦀");
                assert_eq!(*span, ByteSpan::new(0, 1 + 6 + 1 + 2 + 1 + 4 + 1 + 4 + 1));
            }
            other => panic!("expected code span, got {other:?}"),
        }
        // UTF-8 offsets relative to surrounding text stay byte-exact:
        // "abc " is 3 ASCII bytes + 1 space; 한글 occupies 6 bytes.
        let doc = parse("abc `한글` def");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 3);
        match &content[1] {
            Inline::Code { span, .. } => assert_eq!(*span, ByteSpan::new(4, 12)),
            other => panic!("expected code span, got {other:?}"),
        }
    }

    #[test]
    fn code_span_in_heading() {
        let doc = parse("# Run `cargo test`\n");
        match &doc.nodes[0] {
            Block::Heading { content, .. } => {
                assert_eq!(content.len(), 2);
                match &content[1] {
                    Inline::Code { content, .. } => assert_eq!(content, "cargo test"),
                    other => panic!("expected code span, got {other:?}"),
                }
            }
            other => panic!("expected heading, got {other:?}"),
        }
    }

    #[test]
    fn unclosed_code_span_recovers_as_literal_without_loss() {
        // The scope's examples: `` `foo `` and `` ``foo` `` have no matching
        // closer and must stay literal text with no partial code node.
        for input in ["`foo", "``foo`"] {
            let output = parse_with_diagnostics(input);
            assert!(output.diagnostics.is_empty(), "input {input:?}");
            match &output.document.nodes[0] {
                Block::Paragraph { content, .. } => {
                    assert_eq!(joined_text(content), input, "input {input:?}");
                    assert!(
                        !content
                            .iter()
                            .any(|inline| matches!(inline, Inline::Code { .. })),
                        "input {input:?} must not produce code nodes"
                    );
                }
                other => panic!("input {input:?}: expected paragraph, got {other:?}"),
            }
        }

        // A multi-line construct without a closer also stays literal,
        // including when it spans a soft break.
        let output = parse_with_diagnostics("`foo\nbar\n`` baz");
        assert!(output.diagnostics.is_empty());
        match &output.document.nodes[0] {
            Block::Paragraph { content, .. } => {
                assert_eq!(joined_text(content), "`foo\nbar\n`` baz");
            }
            other => panic!("expected paragraph, got {other:?}"),
        }
    }

    #[test]
    fn document_snapshot_code_spans() {
        insta::assert_debug_snapshot!(
            "code_spans",
            parse("Run `cargo test` and ``foo ` bar``.\n\nLiteral: `**bold**` and `[x](y)` and `.s {v}`.\n\nUnclosed `oops.\n")
        );
    }

    #[test]
    fn document_snapshot_mixed() {
        insta::assert_debug_snapshot!(
            "mixed_document",
            parse("# Title\n\nIntro *em* and **strong**.\n\n- one\n- two\n")
        );
    }

    #[test]
    fn document_snapshot_code_and_break() {
        insta::assert_debug_snapshot!(
            "code_and_break",
            parse("```rust\nlet x = 1;\n```\n\nEnd  \nof line.\n")
        );
    }

    #[test]
    fn document_snapshot_links() {
        insta::assert_debug_snapshot!(
            "links",
            parse("Visit [Typst](https://typst.app).\n\nSee [**M2** docs](#intro) or [file](docs/page.html).\n")
        );
    }

    #[test]
    fn parse_front_matter_at_document_start() {
        let doc = parse("---\ntitle: Hello\nauthor: World\n---\n\n# Heading\n");
        assert!(doc.front_matter.is_some());
        let fm = doc.front_matter.unwrap();
        assert_eq!(fm.fields.len(), 2);
        assert_eq!(fm.fields[0], ("title".into(), "Hello".into()));
        assert_eq!(fm.fields[1], ("author".into(), "World".into()));
        // Front matter span covers from start of first --- to end of second ---
        assert!(fm.span.start == 0);
        assert!(fm.span.end > fm.span.start);
    }

    #[test]
    fn front_matter_is_not_emitted_as_content_blocks() {
        let doc = parse("---\ntitle: Hello\n---\n\n# Heading\n");
        // Only heading block, no blocks for front matter delimiters
        assert_eq!(doc.nodes.len(), 1);
        assert!(matches!(doc.nodes[0], Block::Heading { .. }));
    }

    #[test]
    fn thematic_break_after_content_is_not_front_matter() {
        let doc = parse("# Title\n\n---\n\nContent\n");
        // Front matter only at document start
        assert!(doc.front_matter.is_none());
        assert_eq!(doc.nodes.len(), 3); // heading, thematic break, paragraph
        assert!(matches!(doc.nodes[1], Block::ThematicBreak { .. }));
    }

    #[test]
    fn parse_front_matter_with_crlf() {
        let doc = parse("---\r\ntitle: Hello\r\n---\r\n\r\n# Heading\r\n");
        assert!(doc.front_matter.is_some());
        let fm = doc.front_matter.unwrap();
        assert_eq!(fm.fields.len(), 1);
        assert_eq!(fm.fields[0], ("title".into(), "Hello".into()));
    }

    #[test]
    fn indented_front_matter_opening_delimiter_not_recognized() {
        let doc = parse("  ---\ntitle: Hello\n---\n\n# Heading\n");
        // Indented opening delimiter is not recognized
        assert!(doc.front_matter.is_none());
        // Should be treated as paragraph or thematic break
        assert!(!doc.nodes.is_empty());
    }

    #[test]
    fn indented_front_matter_closing_delimiter_not_recognized() {
        let doc = parse("---\ntitle: Hello\n  ---\n\n# Heading\n");
        // Indented closing delimiter is not recognized
        assert!(doc.front_matter.is_none());
        // Should be treated as unclosed front matter, so content is parsed as blocks
        assert!(!doc.nodes.is_empty());
    }

    /// Returns whether any paragraph in the document contains `needle` text.
    fn has_paragraph_text(doc: &Document, needle: &str) -> bool {
        doc.nodes.iter().any(|node| {
            matches!(
                node,
                Block::Paragraph { content, .. }
                    if content.iter().any(|inline| matches!(
                        inline,
                        Inline::Text { content, .. } if content.contains(needle)
                    ))
            )
        })
    }

    #[test]
    fn indented_key_rejects_front_matter_block() {
        let doc = parse("---\n  title: Hello\n---\n\n# Heading\n");
        // Indented metadata lines are not valid flat key: value front matter
        assert!(doc.front_matter.is_none());
        // The malformed block is preserved as regular Markdown body text
        assert!(has_paragraph_text(&doc, "title: Hello"));
        assert!(doc.nodes.iter().any(|n| matches!(n, Block::Heading { .. })));
    }

    #[test]
    fn nested_object_rejects_front_matter_block() {
        let doc = parse("---\nauthor:\n  name: Alice\n---\n\n# Heading\n");
        // Nested object shape is not flattened into metadata
        assert!(doc.front_matter.is_none());
        assert!(has_paragraph_text(&doc, "name: Alice"));
        assert!(doc.nodes.iter().any(|n| matches!(n, Block::Heading { .. })));
    }

    #[test]
    fn duplicate_custom_key_last_wins() {
        let doc = parse("---\ncustom: First\ncustom: Second\n---\n\n# Heading\n");
        // Duplicate custom key: last-wins, single field
        assert!(doc.front_matter.is_some());
        let fm = doc.front_matter.unwrap();
        assert_eq!(fm.fields.len(), 1);
        assert_eq!(fm.fields[0], ("custom".into(), "Second".into()));
    }

    #[test]
    fn malformed_front_matter_line_rejects_block() {
        let doc = parse("---\ntitle: Hello\ninvalid line\n---\n\n# Heading\n");
        // Malformed line (no colon) causes entire front matter block to be rejected
        // Content is parsed as regular Markdown
        assert!(doc.front_matter.is_none());
        assert!(!doc.nodes.is_empty());
        // The heading should be parsed
        assert!(doc.nodes.iter().any(|n| matches!(n, Block::Heading { .. })));
    }

    #[test]
    fn empty_key_in_front_matter_rejects_block() {
        let doc = parse("---\n: value\n---\n\n# Heading\n");
        // Empty key causes entire block to be rejected
        assert!(doc.front_matter.is_none());
        assert!(!doc.nodes.is_empty());
        assert!(doc.nodes.iter().any(|n| matches!(n, Block::Heading { .. })));
    }

    #[test]
    fn malformed_line_before_valid_field_rejects_block() {
        let doc = parse("---\ninvalid line\ntitle: Hello\n---\n\n# Heading\n");
        // Malformed line before valid field still rejects entire block
        assert!(doc.front_matter.is_none());
        assert!(!doc.nodes.is_empty());
        assert!(doc.nodes.iter().any(|n| matches!(n, Block::Heading { .. })));
    }

    #[test]
    fn partial_front_matter_no_partial_result() {
        let doc = parse("---\ntitle: Hello\ninvalid line\n---\n\n# Heading\n");
        // No partial metadata should be generated
        assert!(doc.front_matter.is_none());
        // But content should be parsed
        assert!(doc.nodes.iter().any(|n| matches!(n, Block::Heading { .. })));
    }

    #[test]
    fn front_matter_value_with_colon() {
        let doc = parse("---\ntitle: Hello: World\n---\n\n# Heading\n");
        // Value can contain colon
        assert!(doc.front_matter.is_some());
        let fm = doc.front_matter.unwrap();
        assert_eq!(fm.fields.len(), 1);
        assert_eq!(fm.fields[0], ("title".into(), "Hello: World".into()));
    }

    #[test]
    fn duplicate_front_matter_key_last_wins() {
        let doc = parse("---\ntitle: First\ntitle: Second\n---\n\n# Heading\n");
        // Duplicate key: last-wins
        assert!(doc.front_matter.is_some());
        let fm = doc.front_matter.unwrap();
        assert_eq!(fm.fields.len(), 1);
        assert_eq!(fm.fields[0], ("title".into(), "Second".into()));
    }

    #[test]
    fn empty_front_matter() {
        let doc = parse("---\n---\n\n# Heading\n");
        // Empty front matter is valid
        assert!(doc.front_matter.is_some());
        let fm = doc.front_matter.unwrap();
        assert_eq!(fm.fields.len(), 0);
    }

    // ------------------------------------------------------------------
    // Quarkdown dot-call syntax
    // ------------------------------------------------------------------

    #[test]
    fn block_call_no_arguments() {
        let doc = parse(".note\n");
        match &doc.nodes[0] {
            Block::DirectiveCall {
                name,
                positional_args,
                named_args,
                body,
                span,
            } => {
                assert_eq!(name, "note");
                assert!(positional_args.is_empty());
                assert!(named_args.is_empty());
                assert!(body.is_none());
                assert_eq!(*span, ByteSpan::new(0, 5));
            }
            other => panic!("expected directive call, got {other:?}"),
        }
    }

    #[test]
    fn block_call_positional_args() {
        let doc = parse(".range {1} {10}\n");
        match &doc.nodes[0] {
            Block::DirectiveCall {
                name,
                positional_args,
                body,
                span,
                ..
            } => {
                assert_eq!(name, "range");
                assert_eq!(positional_args.len(), 2);
                assert_eq!(positional_args[0], Value::Number(1.0));
                assert_eq!(positional_args[1], Value::Number(10.0));
                assert!(body.is_none());
                assert_eq!(*span, ByteSpan::new(0, 15));
            }
            other => panic!("expected directive call, got {other:?}"),
        }
    }

    #[test]
    fn block_call_named_args() {
        let doc = parse(".panel width:{320} align:{center}\n");
        match &doc.nodes[0] {
            Block::DirectiveCall {
                named_args,
                positional_args,
                ..
            } => {
                assert!(positional_args.is_empty());
                assert_eq!(named_args.len(), 2);
                assert_eq!(named_args[0].0, "width");
                assert_eq!(named_args[0].1, Value::Number(320.0));
                assert_eq!(named_args[1].0, "align");
                assert_eq!(named_args[1].1, Value::Identifier("center".into()));
            }
            other => panic!("expected directive call, got {other:?}"),
        }
    }

    #[test]
    fn block_call_mixed_args() {
        let doc = parse(".panel {Introduction} width:{320}\n");
        match &doc.nodes[0] {
            Block::DirectiveCall {
                positional_args,
                named_args,
                ..
            } => {
                assert_eq!(positional_args.len(), 1);
                assert_eq!(positional_args[0], Value::Identifier("Introduction".into()));
                assert_eq!(named_args.len(), 1);
                assert_eq!(named_args[0].0, "width");
                assert_eq!(named_args[0].1, Value::Number(320.0));
            }
            other => panic!("expected directive call, got {other:?}"),
        }
    }

    #[test]
    fn block_call_with_indented_body() {
        let doc = parse(".panel {Intro}\n    Hello world\n");
        match &doc.nodes[0] {
            Block::DirectiveCall { name, body, .. } => {
                assert_eq!(name, "panel");
                let body_blocks = body.as_ref().expect("body");
                assert_eq!(body_blocks.len(), 1);
                match &body_blocks[0] {
                    Block::Paragraph { content, span } => {
                        assert_text(&content[0], "Hello world");
                        // Body text starts at the 4-column indentation (line 1, col 4);
                        // paragraph span runs to the end of the body line.
                        assert_eq!(*span, ByteSpan::new(19, 31));
                    }
                    other => panic!("expected body paragraph, got {other:?}"),
                }
            }
            other => panic!("expected directive call, got {other:?}"),
        }
    }

    #[test]
    fn block_call_body_span_covers_indented_lines() {
        let doc = parse(".note {A}\n  line one\n  line two\n");
        match &doc.nodes[0] {
            Block::DirectiveCall { body, .. } => {
                let body_blocks = body.as_ref().expect("body");
                assert_eq!(body_blocks.len(), 1);
                match &body_blocks[0] {
                    Block::Paragraph { span, .. } => {
                        // Body covers both lines: from first content byte to line end.
                        assert_eq!(*span, ByteSpan::new(12, 32));
                    }
                    other => panic!("expected paragraph, got {other:?}"),
                }
            }
            other => panic!("expected call, got {other:?}"),
        }
    }

    #[test]
    fn block_body_may_contain_markdown_and_nested_calls() {
        let doc = parse(".panel {Outer}\n    Hello\n\n    .note {Nested}\n        Nested body\n");
        match &doc.nodes[0] {
            Block::DirectiveCall { name, body, .. } => {
                assert_eq!(name, "panel");
                let body_blocks = body.as_ref().expect("body");
                assert_eq!(body_blocks.len(), 2, "expected paragraph + nested call");
                match &body_blocks[0] {
                    Block::Paragraph { content, .. } => assert_text(&content[0], "Hello"),
                    other => panic!("expected paragraph, got {other:?}"),
                }
                match &body_blocks[1] {
                    Block::DirectiveCall {
                        name,
                        positional_args,
                        body,
                        span,
                        ..
                    } => {
                        assert_eq!(name, "note");
                        assert_eq!(positional_args.len(), 1);
                        assert_eq!(positional_args[0], Value::Identifier("Nested".into()));
                        // Nested call span covers its header AND its body: starts after its
                        // indentation (30) and ends after "Nested body" (65).
                        assert_eq!(*span, ByteSpan::new(30, 65));
                        let nested_body = body.as_ref().expect("nested body");
                        assert_eq!(nested_body.len(), 1);
                        match &nested_body[0] {
                            Block::Paragraph { content, .. } => {
                                assert_text(&content[0], "Nested body")
                            }
                            other => panic!("expected paragraph, got {other:?}"),
                        }
                    }
                    other => panic!("expected nested call, got {other:?}"),
                }
            }
            other => panic!("expected call, got {other:?}"),
        }
    }

    #[test]
    fn body_requires_minimum_indentation() {
        // A non-indented following line is not a body part.
        let doc = parse(".note\nplain text\n");
        match &doc.nodes[0] {
            Block::DirectiveCall { body, .. } => assert!(body.is_none()),
            other => panic!("expected call, got {other:?}"),
        }
        assert!(matches!(doc.nodes[1], Block::Paragraph { .. }));
    }

    #[test]
    fn body_single_tab_counts_as_body() {
        let doc = parse(".note\n\ttabbed body\n");
        match &doc.nodes[0] {
            Block::DirectiveCall { body, .. } => {
                let body_blocks = body.as_ref().expect("body");
                assert_eq!(body_blocks.len(), 1);
                match &body_blocks[0] {
                    Block::Paragraph { content, .. } => assert_text(&content[0], "tabbed body"),
                    other => panic!("expected paragraph, got {other:?}"),
                }
            }
            other => panic!("expected call, got {other:?}"),
        }
    }

    #[test]
    fn body_stops_at_less_indented_line() {
        let doc = parse(".panel\n    indented\nnot indented\n");
        match &doc.nodes[0] {
            Block::DirectiveCall { body, .. } => {
                let body_blocks = body.as_ref().expect("body");
                assert_eq!(body_blocks.len(), 1);
            }
            other => panic!("expected call, got {other:?}"),
        }
        assert!(
            matches!(doc.nodes[1], Block::Paragraph { .. }),
            "second node"
        );
    }

    #[test]
    fn call_with_trailing_text_is_inline_call() {
        let doc = parse(".note trailing text here\n");
        // The call does not own the line, so the whole line is a paragraph
        // containing an inline call.
        match &doc.nodes[0] {
            Block::Paragraph { content, .. } => {
                match &content[0] {
                    Inline::DirectiveCall { name, span, .. } => {
                        assert_eq!(name, "note");
                        assert_eq!(*span, ByteSpan::new(0, 5));
                    }
                    other => panic!("expected inline call, got {other:?}"),
                }
                assert_text(&content[1], " trailing text here");
            }
            other => panic!("expected paragraph, got {other:?}"),
        }
    }

    #[test]
    fn inline_call_in_sentence() {
        let doc = parse("See .note {x} for details.\n");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 3);
        assert_text(&content[0], "See ");
        match &content[1] {
            Inline::DirectiveCall {
                name,
                positional_args,
                span,
                ..
            } => {
                assert_eq!(name, "note");
                assert_eq!(positional_args.len(), 1);
                assert_eq!(positional_args[0], Value::Identifier("x".into()));
                assert_eq!(*span, ByteSpan::new(4, 13));
            }
            other => panic!("expected inline call, got {other:?}"),
        }
        assert_text(&content[2], " for details.");
    }

    #[test]
    fn inline_call_does_not_parse_in_numbers() {
        let doc = parse("pi is 3.14 exactly\n");
        assert_eq!(joined_text(paragraph_inlines(&doc)), "pi is 3.14 exactly");
    }

    #[test]
    fn ellipsis_is_literal_text() {
        let doc = parse("...and more\n");
        assert_eq!(joined_text(paragraph_inlines(&doc)), "...and more");
    }

    #[test]
    fn nested_call_inside_argument() {
        let doc = parse(".outer {.inner {value}}\n");
        match &doc.nodes[0] {
            Block::DirectiveCall {
                positional_args, ..
            } => {
                assert_eq!(positional_args.len(), 1);
                match &positional_args[0] {
                    Value::Content(content) => {
                        assert_eq!(content.len(), 1);
                        match &content[0] {
                            Inline::DirectiveCall {
                                name,
                                positional_args,
                                span,
                                ..
                            } => {
                                assert_eq!(name, "inner");
                                assert_eq!(positional_args.len(), 1);
                                assert_eq!(positional_args[0], Value::Identifier("value".into()));
                                assert_eq!(*span, ByteSpan::new(8, 22));
                            }
                            other => panic!("expected nested call, got {other:?}"),
                        }
                    }
                    other => panic!("expected content value, got {other:?}"),
                }
            }
            other => panic!("expected call, got {other:?}"),
        }
    }

    #[test]
    fn argument_with_markdown_is_content() {
        let doc = parse(".fn {some *text* here}\n");
        match &doc.nodes[0] {
            Block::DirectiveCall {
                positional_args, ..
            } => match &positional_args[0] {
                Value::Content(content) => {
                    assert_eq!(content.len(), 3);
                    assert_text(&content[0], "some ");
                    assert!(matches!(content[1], Inline::Emphasis { .. }));
                    assert_text(&content[2], " here");
                }
                other => panic!("expected content value, got {other:?}"),
            },
            other => panic!("expected call, got {other:?}"),
        }
    }

    #[test]
    fn malformed_calls_do_not_panic_and_fall_back_to_paragraph() {
        for input in [
            ".foo {",
            ".foo {value",
            ".foo key:{",
            ".foo key:{value",
            ".foo width:{x} {y}",
        ] {
            let doc = parse(input);
            // Recoverable: the line becomes a paragraph; no panic.
            assert!(
                matches!(doc.nodes[0], Block::Paragraph { .. }),
                "input {input:?} should fall back to paragraph"
            );
        }
    }

    #[test]
    fn malformed_unclosed_body_brace() {
        let doc = parse(".foo {\ntext\n");
        assert!(matches!(doc.nodes[0], Block::Paragraph { .. }));
        let content = paragraph_inlines(&doc);
        // The failed call is recovered as literal characters.
        assert_eq!(content.len(), 4);
        assert_text(&content[0], ".");
        assert_text(&content[1], "foo {");
        assert!(matches!(content[2], Inline::SoftBreak { .. }));
        assert_text(&content[3], "text");
    }

    #[test]
    fn dot_without_name_is_literal_text() {
        let doc = parse("like . this\n");
        let content = paragraph_inlines(&doc);
        assert_eq!(joined_text(content), "like . this");
    }

    #[test]
    fn block_call_then_blank_then_text_not_body() {
        let doc = parse(".note\n\nParagraph after\n");
        assert!(matches!(doc.nodes[0], Block::DirectiveCall { .. }));
        assert!(matches!(doc.nodes[1], Block::Paragraph { .. }));
    }

    #[test]
    fn block_call_underscore_name() {
        let doc = parse(".my_call {v}\n");
        assert!(matches!(
            &doc.nodes[0],
            Block::DirectiveCall { name, .. } if name == "my_call"
        ));
    }

    #[test]
    fn malformed_calls_produce_structured_diagnostics() {
        for (input, expected_code) in [
            (".foo {", "E2003"),
            (".foo {value", "E2003"),
            (".foo key:{", "E2003"),
            (".foo key:{value", "E2003"),
            (".foo key:", "E2002"),
            (".foo width:{x} {y}", "E2001"),
        ] {
            let output = parse_with_diagnostics(input);
            assert_eq!(
                output.diagnostics.len(),
                1,
                "input {input:?} should yield exactly one diagnostic"
            );
            assert_eq!(output.diagnostics[0].code, expected_code, "input {input:?}");
            assert!(
                output.diagnostics[0].span.start <= output.diagnostics[0].span.end,
                "input {input:?}"
            );
            assert!(!output.diagnostics[0].message.is_empty(), "input {input:?}");
            assert!(
                matches!(output.document.nodes[0], Block::Paragraph { .. }),
                "input {input:?} should fall back to paragraph"
            );
        }
    }

    #[test]
    fn valid_calls_produce_no_diagnostics() {
        for input in [".foo {bar}\n", ".foo key:{value}\n", ".1\n"] {
            let output = parse_with_diagnostics(input);
            assert!(output.diagnostics.is_empty(), "input {input:?}");
            assert!(matches!(
                output.document.nodes[0],
                Block::DirectiveCall { .. }
            ));
        }
    }

    #[test]
    fn implicit_reference_call_at_block_level() {
        let doc = parse(".1\n");
        assert!(matches!(
            &doc.nodes[0],
            Block::DirectiveCall { name, .. } if name == "1"
        ));
    }

    #[test]
    fn implicit_reference_is_not_a_call_with_positional_argument() {
        for input in [".1 {item}\n", ".12foo\n", ".1abc\n"] {
            let doc = parse(input);
            // Block level: the line must not become a directive block that
            // turned `.1 {item}` into a call with a positional argument.
            assert!(
                matches!(&doc.nodes[0], Block::Paragraph { .. }),
                "input {input:?} should stay a paragraph"
            );
            assert!(
                !matches!(
                    &doc.nodes[0],
                    Block::DirectiveCall { positional_args, .. }
                        if !positional_args.is_empty()
                ),
                "input {input:?} must not be a call with positional args"
            );
        }
        // Inline level: `.1abc` / `.12foo` must not split into `ref + text`.
        let doc = parse("see .1abc\n");
        match &doc.nodes[0] {
            Block::Paragraph { content, .. } => {
                let joined = joined_text(content);
                assert_eq!(joined, "see .1abc");
            }
            other => panic!("expected paragraph, got {other:?}"),
        }
    }

    #[test]
    fn implicit_reference_inline_boundaries() {
        // Punctuation/whitespace after the reference keeps it a call.
        let doc = parse("The value is .1.\n");
        let content = paragraph_inlines(&doc);
        assert!(content.iter().any(
            |inline| matches!(inline, Inline::DirectiveCall { name, positional_args, .. }
                if name == "1" && positional_args.is_empty())
        ));
    }

    #[test]
    fn inline_call_at_line_start_continues_paragraph() {
        // A call with trailing inline content is not a block directive, so
        // it must not terminate the surrounding paragraph.
        let doc = parse("before\n.note {x} after\nend\n");
        assert_eq!(doc.nodes.len(), 1, "expected a single paragraph");
        let content = paragraph_inlines(&doc);
        assert_eq!(content.len(), 6);
        assert_text(&content[0], "before");
        assert!(matches!(content[1], Inline::SoftBreak { .. }));
        match &content[2] {
            Inline::DirectiveCall {
                name,
                positional_args,
                ..
            } => {
                assert_eq!(name, "note");
                assert_eq!(positional_args.len(), 1);
            }
            other => panic!("expected inline call, got {other:?}"),
        }
        assert_text(&content[3], " after");
        assert!(matches!(content[4], Inline::SoftBreak { .. }));
        assert!(matches!(&content[5], Inline::Text { .. }));
    }

    #[test]
    fn invalid_implicit_reference_does_not_split_paragraph() {
        // `.1abc` is ordinary text and must not split the paragraph.
        let doc = parse("before\n.1abc\nafter\n");
        assert_eq!(doc.nodes.len(), 1);
        assert_eq!(
            joined_text(paragraph_inlines(&doc)),
            "before\n.1abc\nafter",
            "no call may appear inside the paragraph"
        );
        let output = parse_with_diagnostics("before\n.1abc\nafter\n");
        assert!(output.diagnostics.is_empty());
    }

    #[test]
    fn isolated_call_line_still_starts_block() {
        let doc = parse("before\n.note {x}\nafter\n");
        assert_eq!(doc.nodes.len(), 3);
        assert!(matches!(doc.nodes[0], Block::Paragraph { .. }));
        assert!(matches!(
            &doc.nodes[1],
            Block::DirectiveCall { name, .. } if name == "note"
        ));
        assert!(matches!(doc.nodes[2], Block::Paragraph { .. }));
    }

    #[test]
    fn block_body_still_works_with_semantic_classification() {
        let doc = parse("before\n\n.note {x}\n    body\n\nafter\n");
        assert_eq!(doc.nodes.len(), 3);
        match &doc.nodes[1] {
            Block::DirectiveCall { name, body, .. } => {
                assert_eq!(name, "note");
                let blocks = body.as_ref().expect("expected an indented body");
                assert_eq!(blocks.len(), 1);
                match &blocks[0] {
                    Block::Paragraph { content, .. } => {
                        assert_eq!(joined_text(content), "body");
                    }
                    other => panic!("expected body paragraph, got {other:?}"),
                }
            }
            other => panic!("expected directive call, got {other:?}"),
        }
    }

    #[test]
    fn tight_call_boundary_rejects_trailing_word() {
        // `before .note {x}suffix` must NOT produce a call: the suffix
        // glues to the call, so the whole construct is prose.
        let doc = parse("before .note {x}suffix\n");
        assert_eq!(
            joined_text(paragraph_inlines(&doc)),
            "before .note {x}suffix",
            "tight trailing word must keep the whole construct ordinary text"
        );
        // A spaced suffix is a legal boundary.
        let doc = parse("before .note {x} suffix\n");
        let content = paragraph_inlines(&doc);
        assert!(
            matches!(&content[1], Inline::DirectiveCall { name, .. } if name == "note"),
            "a space after the call is a valid boundary"
        );
    }

    #[test]
    fn tight_call_hyphen_boundaries_are_valid() {
        // The hyphen is a documented symbol boundary on both sides.
        let doc = parse("before-.note {x}-after\n");
        let content = paragraph_inlines(&doc);
        assert!(content.iter().any(|inline| matches!(
            inline,
            Inline::DirectiveCall { name, .. } if name == "note"
        )));
    }

    #[test]
    fn unicode_word_characters_are_tight_adjacency() {
        // Non-ASCII letters are word characters, not symbols: a call glued
        // to Korean script must not be recognized.
        let doc = parse("한.note {x}\n");
        assert_eq!(joined_text(paragraph_inlines(&doc)), "한.note {x}");
        let doc = parse(".note {x}한\n");
        assert_eq!(joined_text(paragraph_inlines(&doc)), ".note {x}한");
    }

    #[test]
    fn nested_call_with_tight_suffix_is_not_a_call() {
        // Inside an argument the same boundary rules apply: `.inner`
        // followed by a word character must not become a nested call.
        let doc = parse(".outer {prefix .inner {x}suffix}\n");
        match &doc.nodes[0] {
            Block::DirectiveCall {
                name,
                positional_args,
                ..
            } => {
                assert_eq!(name, "outer");
                match &positional_args[0] {
                    Value::Content(content) => {
                        assert!(!content
                            .iter()
                            .any(|inline| matches!(inline, Inline::DirectiveCall { .. })));
                    }
                    other => panic!("expected content argument, got {other:?}"),
                }
            }
            other => panic!("expected outer call, got {other:?}"),
        }
    }
}
