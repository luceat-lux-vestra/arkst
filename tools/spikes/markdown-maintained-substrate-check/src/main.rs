use std::fmt::{self, Write};

use rushdown::ast::{Arena, KindData, NodeKind, NodeRef, NodeType, PrettyPrint};
use rushdown::parser::{
    parser_extension, BlockParser, Context, InlineParser, NoParserOptions, Options, Parser, State,
    PRIORITY_CODE_SPAN, PRIORITY_FENCED_CODE_BLOCK,
};
use rushdown::text::{BasicReader, BlockReader, Reader, Segment};
use rushdown::{as_extension_data, matches_extension_kind, matches_kind};

const BLOCK_CALL: &[u8] = b".align {center}";
const INLINE_CALL: &[u8] = ".text {빨강}".as_bytes();

#[derive(Debug)]
struct QuarkdownBlock {
    call: Segment,
}

impl NodeKind for QuarkdownBlock {
    fn typ(&self) -> NodeType {
        NodeType::ContainerBlock
    }

    fn kind_name(&self) -> &'static str {
        "QuarkdownBlock"
    }
}

impl PrettyPrint for QuarkdownBlock {
    fn pretty_print(&self, output: &mut dyn Write, source: &str, level: usize) -> fmt::Result {
        writeln!(
            output,
            "{}{:?}",
            "  ".repeat(level),
            self.call.bytes(source)
        )
    }
}

impl From<QuarkdownBlock> for KindData {
    fn from(value: QuarkdownBlock) -> Self {
        KindData::Extension(Box::new(value))
    }
}

#[derive(Debug)]
struct QuarkdownBlockParser;

impl BlockParser for QuarkdownBlockParser {
    fn trigger(&self) -> &[u8] {
        b"."
    }

    fn open(
        &self,
        arena: &mut Arena,
        _parent_ref: NodeRef,
        reader: &mut BasicReader,
        ctx: &mut Context,
    ) -> Option<(NodeRef, State)> {
        let (line, segment) = reader.peek_line_bytes()?;
        let offset = ctx.block_offset()?;
        let rest = line.get(offset..)?;
        if !rest.starts_with(BLOCK_CALL)
            || rest
                .get(BLOCK_CALL.len()..)?
                .iter()
                .any(|byte| !matches!(byte, b' ' | b'\t' | b'\r' | b'\n'))
        {
            return None;
        }

        let start = segment.start() + offset.checked_sub(segment.padding())?;
        let node_ref = arena.new_node(QuarkdownBlock {
            call: Segment::new(start, start + BLOCK_CALL.len()),
        });
        reader.advance_to_eol();
        Some((node_ref, State::HAS_CHILDREN))
    }

    fn cont(
        &self,
        _arena: &mut Arena,
        _node_ref: NodeRef,
        reader: &mut BasicReader,
        _ctx: &mut Context,
    ) -> Option<State> {
        let (line, _) = reader.peek_line_bytes()?;
        line.starts_with(b"    ").then(|| {
            reader.advance(4);
            State::HAS_CHILDREN
        })
    }

    fn can_interrupt_paragraph(&self) -> bool {
        true
    }
}

#[derive(Debug)]
struct QuarkdownInline {
    call: Segment,
    opened_in_link_label: bool,
}

impl NodeKind for QuarkdownInline {
    fn typ(&self) -> NodeType {
        NodeType::Inline
    }

    fn kind_name(&self) -> &'static str {
        "QuarkdownInline"
    }
}

impl PrettyPrint for QuarkdownInline {
    fn pretty_print(&self, output: &mut dyn Write, source: &str, level: usize) -> fmt::Result {
        writeln!(
            output,
            "{}{:?}",
            "  ".repeat(level),
            self.call.bytes(source)
        )
    }
}

impl From<QuarkdownInline> for KindData {
    fn from(value: QuarkdownInline) -> Self {
        KindData::Extension(Box::new(value))
    }
}

#[derive(Debug)]
struct QuarkdownInlineParser;

impl InlineParser for QuarkdownInlineParser {
    fn trigger(&self) -> &[u8] {
        b"."
    }

    fn parse(
        &self,
        arena: &mut Arena,
        _parent_ref: NodeRef,
        reader: &mut BlockReader,
        ctx: &mut Context,
    ) -> Option<NodeRef> {
        let (line, segment) = reader.peek_line_bytes()?;
        if !line.starts_with(INLINE_CALL) {
            return None;
        }
        let call = Segment::new(segment.start(), segment.start() + INLINE_CALL.len());
        reader.advance(INLINE_CALL.len());
        Some(arena.new_node(QuarkdownInline {
            call,
            opened_in_link_label: ctx.is_in_link_label(),
        }))
    }
}

fn parser(enabled: bool) -> Parser {
    if !enabled {
        return Parser::new();
    }

    Parser::with_extensions(
        Options::default(),
        parser_extension(|parser| {
            parser.add_block_parser(
                || -> Box<dyn BlockParser> { Box::new(QuarkdownBlockParser) },
                NoParserOptions,
                PRIORITY_FENCED_CODE_BLOCK + 1,
            );
            parser.add_inline_parser(
                || -> Box<dyn InlineParser> { Box::new(QuarkdownInlineParser) },
                NoParserOptions,
                PRIORITY_CODE_SPAN + 1,
            );
        }),
    )
}

fn parse(source: &str, enabled: bool) -> (Arena, NodeRef) {
    let parser = parser(enabled);
    let mut reader = BasicReader::new(source);
    parser.parse(&mut reader)
}

fn walk(arena: &Arena, root: NodeRef) -> Vec<NodeRef> {
    fn visit(arena: &Arena, node: NodeRef, output: &mut Vec<NodeRef>) {
        output.push(node);
        for child in arena[node].children(arena) {
            visit(arena, child, output);
        }
    }

    let mut output = Vec::new();
    visit(arena, root, &mut output);
    output
}

fn custom_blocks(arena: &Arena, root: NodeRef) -> Vec<NodeRef> {
    walk(arena, root)
        .into_iter()
        .filter(|node| matches_extension_kind!(arena, *node, QuarkdownBlock))
        .collect()
}

fn custom_inlines(arena: &Arena, root: NodeRef) -> Vec<NodeRef> {
    walk(arena, root)
        .into_iter()
        .filter(|node| matches_extension_kind!(arena, *node, QuarkdownInline))
        .collect()
}

fn has_ancestor(arena: &Arena, mut node: NodeRef, kind: fn(&Arena, NodeRef) -> bool) -> bool {
    while let Some(parent) = arena[node].parent() {
        if kind(arena, parent) {
            return true;
        }
        node = parent;
    }
    false
}

fn ancestor_count(arena: &Arena, mut node: NodeRef, kind: fn(&Arena, NodeRef) -> bool) -> usize {
    let mut count = 0;
    while let Some(parent) = arena[node].parent() {
        if kind(arena, parent) {
            count += 1;
        }
        node = parent;
    }
    count
}

fn verify_block_context(label: &str, source: &str, expected_ancestors: &[&str]) {
    let (arena, root) = parse(source, true);
    let blocks = custom_blocks(&arena, root);
    assert_eq!(blocks.len(), 1, "{label}: custom block count");
    let block = blocks[0];
    let value = as_extension_data!(arena, block, QuarkdownBlock);
    assert_eq!(
        value.call.bytes(source).as_ref(),
        BLOCK_CALL,
        "{label}: span"
    );
    for expected in expected_ancestors {
        let present = match *expected {
            "Blockquote" => has_ancestor(&arena, block, |a, n| matches_kind!(a, n, Blockquote)),
            "List" => has_ancestor(&arena, block, |a, n| matches_kind!(a, n, List)),
            _ => false,
        };
        assert!(present, "{label}: missing {expected} ancestor");
    }
    assert!(walk(&arena, block)
        .into_iter()
        .any(|node| matches_kind!(arena, node, Strong)));

    if label == "nested-list" {
        assert!(ancestor_count(&arena, block, |a, n| matches_kind!(a, n, List)) >= 2);
    }
}

fn verify_blocks() {
    verify_block_context("root", ".align {center}\n    Body **Markdown**\n", &[]);
    verify_block_context(
        "blockquote",
        "> .align {center}\n>     Body **Markdown**\n",
        &["Blockquote"],
    );
    verify_block_context(
        "list",
        "- .align {center}\n      Body **Markdown**\n",
        &["List"],
    );
    verify_block_context(
        "nested-list",
        "- outer\n  - .align {center}\n        Body **Markdown**\n",
        &["List"],
    );
    verify_block_context(
        "list-to-blockquote",
        "- > .align {center}\n  >     Body **Markdown**\n",
        &["List", "Blockquote"],
    );
    verify_block_context(
        "blockquote-to-list",
        "> - .align {center}\n>       Body **Markdown**\n",
        &["Blockquote", "List"],
    );

    let interruption = "paragraph before\n.align {center}\n    Body **Markdown**\n";
    let (arena, root) = parse(interruption, true);
    let block = custom_blocks(&arena, root)[0];
    assert!(walk(&arena, block)
        .into_iter()
        .any(|node| matches_kind!(arena, node, Strong)));

    let lazy = "> quote\nlazy continuation\n\n.align {center}\n    Body\n";
    let (arena, root) = parse(lazy, true);
    let block = custom_blocks(&arena, root)[0];
    assert_eq!(
        walk(&arena, root)
            .into_iter()
            .filter(|node| matches_kind!(arena, *node, Blockquote))
            .count(),
        1
    );
    assert!(!has_ancestor(&arena, block, |a, n| matches_kind!(
        a, n, Blockquote
    )));

    for source in ["    .align {center}\n", "```text\n.align {center}\n```\n"] {
        let (arena, root) = parse(source, true);
        assert!(custom_blocks(&arena, root).is_empty());
        assert!(custom_inlines(&arena, root).is_empty());
    }
}

fn verify_inline_context(
    label: &str,
    source: &str,
    expected_ancestor: Option<fn(&Arena, NodeRef) -> bool>,
) -> (usize, usize) {
    let (arena, root) = parse(source, true);
    let inlines = custom_inlines(&arena, root);
    assert_eq!(inlines.len(), 1, "{label}: custom inline count");
    let inline = inlines[0];
    let value = as_extension_data!(arena, inline, QuarkdownInline);
    assert_eq!(
        value.call.bytes(source).as_ref(),
        INLINE_CALL,
        "{label}: span"
    );
    if let Some(expected) = expected_ancestor {
        assert!(has_ancestor(&arena, inline, expected), "{label}: ancestor");
        assert!(value.opened_in_link_label, "{label}: link context signal");
    }
    (value.call.start(), value.call.stop())
}

fn verify_inlines() {
    let utf8 = "한글 .text {빨강} 끝\n";
    let span = verify_inline_context("utf8", utf8, None);
    assert_eq!(span, ("한글 ".len(), "한글 ".len() + INLINE_CALL.len()));

    let crlf = "한글 .text {빨강} 끝\r\n";
    assert_eq!(verify_inline_context("crlf", crlf, None), span);

    verify_inline_context("emphasis-adjacency", "**앞**.text {빨강}*끝*\n", None);
    verify_inline_context(
        "link",
        "[한글 .text {빨강} 끝](https://example.com)\n",
        Some(|arena, node| matches_kind!(arena, node, Link)),
    );
    verify_inline_context(
        "image",
        "![한글 .text {빨강} 끝](image.png)\n",
        Some(|arena, node| matches_kind!(arena, node, Image)),
    );

    for source in [
        "`한글 .text {빨강} 끝`\n",
        "```text\n한글 .text {빨강} 끝\n```\n",
        "    한글 .text {빨강} 끝\n",
    ] {
        let (arena, root) = parse(source, true);
        assert!(custom_inlines(&arena, root).is_empty());
    }
}

fn verify_disabled_isolation() {
    let source = ".align {center}\n    Body **Markdown**\n\n한글 .text {빨강} 끝\n";
    let (arena, root) = parse(source, false);
    assert!(custom_blocks(&arena, root).is_empty());
    assert!(custom_inlines(&arena, root).is_empty());
}

fn main() {
    verify_blocks();
    verify_inlines();
    verify_disabled_isolation();
    println!("RUSHDOWN_EXTENSION_MATRIX_PASS");
}
