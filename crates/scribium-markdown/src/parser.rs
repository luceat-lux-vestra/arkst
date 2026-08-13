use std::fmt::{self, Write};
use std::panic::{catch_unwind, AssertUnwindSafe};

use rushdown::ast::{
    Arena, CodeBlockKind, KindData, NodeKind, NodeRef, NodeType, PrettyPrint, TypeData,
};
use rushdown::parser::{
    gfm, parser_extension, BlockParser, Context, GfmOptions, InlineParser, NoParserOptions,
    Options, Parser, ParserExtension, State, PRIORITY_CODE_SPAN, PRIORITY_FENCED_CODE_BLOCK,
};
use rushdown::text::{BasicReader, BlockReader, Lines, Reader, Segment};
use rushdown::{as_extension_data, matches_extension_kind};
use scribium_quarkdown::{Arg, ArgContent, QuarkdownCall, Value as QuarkdownValue};
use scribium_source::ByteSpan;

use crate::ast::{Block, Document, FrontMatter, Inline, ListItem, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Markdown,
    Quarkdown,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParserDiagnostic {
    pub code: &'static str,
    pub message: String,
    pub span: ByteSpan,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParseOutput {
    pub document: Document,
    pub diagnostics: Vec<ParserDiagnostic>,
}

#[derive(Debug)]
struct QuarkdownBlock {
    call: Segment,
    indent: usize,
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
    fn pretty_print(&self, output: &mut dyn Write, _source: &str, level: usize) -> fmt::Result {
        writeln!(output, "{}QuarkdownBlock", "  ".repeat(level))
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
        let start = segment.start().checked_add(offset)?;
        let end = start.checked_add(line.len().saturating_sub(offset))?;
        let source = reader.source();
        let line_end = end - start;
        let line_source = source.get(start..end)?;
        let call_segment = match scribium_quarkdown::parse_call(line_source) {
            Ok(Some((call, call_end))) => {
                if line_source.as_bytes()[call_end..]
                    .iter()
                    .any(|byte| !byte.is_ascii_whitespace())
                {
                    return None;
                }
                Segment::new(call.span.start + start, call.span.end + start)
            }
            Ok(None) => return None,
            Err(_) => Segment::new(start, start + line_end),
        };
        let node_ref = arena.new_node(QuarkdownBlock {
            call: call_segment,
            indent: physical_indent(source, segment.start()).max(leading_indent(&line)),
        });
        reader.advance_to_eol();
        Some((node_ref, State::HAS_CHILDREN))
    }

    fn cont(
        &self,
        arena: &mut Arena,
        node_ref: NodeRef,
        reader: &mut BasicReader,
        _ctx: &mut Context,
    ) -> Option<State> {
        let (line, segment) = reader.peek_line_bytes()?;
        let indent = as_extension_data!(arena, node_ref, QuarkdownBlock).indent;
        let current_indent =
            physical_indent(reader.source(), segment.start()).max(leading_indent(&line));
        let body_indent = indent.saturating_add(4);
        if current_indent < body_indent {
            return None;
        }
        Some(if current_indent == body_indent {
            reader.advance(body_indent);
            State::HAS_CHILDREN
        } else {
            State::NO_CHILDREN
        })
    }

    fn can_interrupt_paragraph(&self) -> bool {
        true
    }
}

fn physical_indent(source: &str, position: usize) -> usize {
    let line_start = source[..position.min(source.len())]
        .rfind('\n')
        .map_or(0, |index| index + 1);
    position.saturating_sub(line_start)
}

fn leading_indent(line: &[u8]) -> usize {
    line.iter()
        .take_while(|byte| matches!(byte, b' ' | b'\t'))
        .map(|byte| if *byte == b'\t' { 4 } else { 1 })
        .sum()
}

#[derive(Debug)]
struct QuarkdownInline {
    call: Segment,
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
    fn pretty_print(&self, output: &mut dyn Write, _source: &str, level: usize) -> fmt::Result {
        writeln!(output, "{}QuarkdownInline", "  ".repeat(level))
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
        _ctx: &mut Context,
    ) -> Option<NodeRef> {
        let (_, segment) = reader.peek_line_bytes()?;
        let source = reader.source();
        let parsed = scribium_quarkdown::parse_inline_call(source, segment.start());
        let (call_segment, consumed) = match parsed {
            Ok(Some((call, end))) if end <= segment.stop() => (
                Segment::new(call.span.start, call.span.end),
                end - segment.start(),
            ),
            Ok(_) => return None,
            Err(_) => (
                Segment::new(segment.start(), segment.stop()),
                segment.stop() - segment.start(),
            ),
        };
        reader.advance(consumed);
        Some(arena.new_node(QuarkdownInline { call: call_segment }))
    }
}

fn parser(mode: Mode) -> Parser {
    if mode == Mode::Markdown {
        return Parser::with_extensions(Options::default(), gfm(GfmOptions::default()));
    }
    Parser::with_extensions(
        Options::default(),
        gfm(GfmOptions::default()).and(parser_extension(|parser| {
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
        })),
    )
}

pub fn parse(source: &str) -> Document {
    parse_with_diagnostics(source).document
}

pub fn parse_md(source: &str) -> Document {
    parse_with_mode(source, Mode::Markdown).document
}

pub fn parse_qd(source: &str) -> Document {
    parse_with_mode(source, Mode::Quarkdown).document
}

pub fn parse_with_diagnostics(source: &str) -> ParseOutput {
    parse_with_mode(source, Mode::Quarkdown)
}

pub fn parse_with_mode(source: &str, mode: Mode) -> ParseOutput {
    let (front_matter, body_start) = parse_front_matter(source);
    let body = &source[body_start..];
    let parser = parser(mode);
    let mut reader = BasicReader::new(body);
    let mut diagnostics = Vec::new();
    let parsed = catch_unwind(AssertUnwindSafe(|| parser.parse(&mut reader)));
    let Ok((arena, root)) = parsed else {
        diagnostics.push(ParserDiagnostic {
            code: "E9003",
            message: "Rushdown panicked while parsing the document".to_string(),
            span: ByteSpan::new(0, source.len()),
        });
        return ParseOutput {
            document: Document {
                nodes: Vec::new(),
                front_matter,
                line_count: source.lines().count(),
            },
            diagnostics,
        };
    };
    let mut nodes = Vec::new();
    for child in arena[root].children(&arena) {
        if let Some(node) = convert_block(&arena, child, body, body_start, &mut diagnostics) {
            nodes.push(node);
        }
    }
    if mode == Mode::Quarkdown {
        let original = std::mem::take(&mut nodes);
        for mut node in original {
            nodes.extend(normalize_block(&mut node, source));
        }
    }
    ParseOutput {
        document: Document {
            nodes,
            front_matter,
            line_count: source.lines().count(),
        },
        diagnostics,
    }
}

fn normalize_block(block: &mut Block, source: &str) -> Vec<Block> {
    match block {
        Block::DirectiveCall {
            body: Some(body),
            span,
            ..
        } => {
            let children = std::mem::take(body);
            let mut normalized_children = Vec::new();
            for mut child in children {
                normalized_children.extend(normalize_block(&mut child, source));
            }
            let body_indent = line_indent(source, span.start).saturating_add(4);
            let mut kept = Vec::new();
            let mut promoted = Vec::new();
            for child in normalized_children {
                for piece in split_block_by_indent(child, source, body_indent) {
                    if line_indent(source, block_start(&piece)) < body_indent {
                        promoted.push(piece);
                    } else {
                        kept.push(piece);
                    }
                }
            }
            *body = kept;
            let mut result = vec![block.clone()];
            result.extend(promoted);
            result
        }
        Block::Blockquote { content, .. } => {
            normalize_children(content, source);
            vec![block.clone()]
        }
        Block::UnorderedList { items, .. } | Block::OrderedList { items, .. } => {
            for item in items {
                normalize_children(&mut item.content, source);
            }
            vec![block.clone()]
        }
        _ => vec![block.clone()],
    }
}

fn normalize_children(children: &mut Vec<Block>, source: &str) {
    let original = std::mem::take(children);
    for mut child in original {
        children.extend(normalize_block(&mut child, source));
    }
}

fn split_block_by_indent(block: Block, source: &str, body_indent: usize) -> Vec<Block> {
    let Block::Paragraph { content, span } = block else {
        return vec![block];
    };
    if content.len() < 2 {
        return vec![Block::Paragraph { content, span }];
    }
    let mut groups: Vec<Vec<Inline>> = Vec::new();
    let mut promoted = None;
    for inline in content {
        let is_promoted = line_indent(source, inline_start(&inline)) < body_indent;
        if promoted != Some(is_promoted) {
            groups.push(Vec::new());
            promoted = Some(is_promoted);
        }
        groups.last_mut().expect("group created").push(inline);
    }
    groups
        .into_iter()
        .map(|content| Block::Paragraph {
            span: paragraph_span(&content),
            content,
        })
        .collect()
}

fn paragraph_span(content: &[Inline]) -> ByteSpan {
    let start = content.first().map(inline_start).unwrap_or(0);
    let end = content.last().map(inline_end).unwrap_or(start);
    ByteSpan::new(start, end)
}

fn block_start(block: &Block) -> usize {
    match block {
        Block::Heading { span, .. }
        | Block::Paragraph { span, .. }
        | Block::Blockquote { span, .. }
        | Block::UnorderedList { span, .. }
        | Block::OrderedList { span, .. }
        | Block::CodeBlock { span, .. }
        | Block::ThematicBreak { span }
        | Block::DirectiveCall { span, .. }
        | Block::Metadata { span, .. }
        | Block::Raw { span, .. } => span.start,
    }
}

fn line_indent(source: &str, position: usize) -> usize {
    let line_start = source[..position.min(source.len())]
        .rfind('\n')
        .map_or(0, |index| index + 1);
    source[line_start..]
        .bytes()
        .take_while(|byte| matches!(byte, b' ' | b'\t'))
        .map(|byte| if byte == b'\t' { 4 } else { 1 })
        .sum()
}

fn parse_front_matter(source: &str) -> (Option<FrontMatter>, usize) {
    if !source.starts_with("---\n") && !source.starts_with("---\r\n") {
        return (None, 0);
    }
    let mut cursor = source.find('\n').map_or(0, |index| index + 1);
    let mut fields = Vec::new();
    while cursor < source.len() {
        let line_end = source[cursor..]
            .find('\n')
            .map_or(source.len(), |offset| cursor + offset + 1);
        let line = &source[cursor..line_end];
        if line.trim_end_matches(['\r', '\n']) == "---" {
            return (
                Some(FrontMatter {
                    fields,
                    span: ByteSpan::new(0, line_end),
                }),
                line_end,
            );
        }
        if let Some((key, value)) = line.trim_end_matches(['\r', '\n']).split_once(':') {
            fields.push((key.trim().to_string(), value.trim().to_string()));
        }
        cursor = line_end;
    }
    (None, 0)
}

fn convert_block(
    arena: &Arena,
    node: NodeRef,
    source: &str,
    base: usize,
    diagnostics: &mut Vec<ParserDiagnostic>,
) -> Option<Block> {
    let span = node_span(arena, node, source).map(|span| add_base(span, base))?;
    match arena[node].kind_data() {
        KindData::Paragraph(_) => Some(Block::Paragraph {
            content: convert_inlines(arena, node, source, base, diagnostics),
            span,
        }),
        KindData::Heading(heading) => Some(Block::Heading {
            level: heading.level() as usize,
            content: convert_inlines(arena, node, source, base, diagnostics),
            span,
        }),
        KindData::ThematicBreak(_) => Some(Block::ThematicBreak { span }),
        KindData::Blockquote(_) => Some(Block::Blockquote {
            content: convert_children_blocks(arena, node, source, base, diagnostics),
            span,
        }),
        KindData::List(list) => {
            let items = arena[node]
                .children(arena)
                .filter_map(|child| match arena[child].kind_data() {
                    KindData::ListItem(_) => Some(ListItem {
                        content: convert_children_blocks(arena, child, source, base, diagnostics),
                        span: node_span(arena, child, source)
                            .map(|value| add_base(value, base))
                            .unwrap_or(span),
                    }),
                    _ => None,
                })
                .collect();
            if list.is_ordered() {
                Some(Block::OrderedList {
                    items,
                    start: list.start() as usize,
                    span,
                })
            } else {
                Some(Block::UnorderedList { items, span })
            }
        }
        KindData::CodeBlock(code) => {
            let raw = source.get(span.start.saturating_sub(base)..span.end.saturating_sub(base))?;
            let language = code_language(code, source);
            let body =
                code_block_source(arena, node, code, source).unwrap_or_else(|| code_source(raw));
            Some(Block::CodeBlock {
                language,
                source: body,
                span,
            })
        }
        KindData::Extension(_) if matches_extension_kind!(arena, node, QuarkdownBlock) => {
            let extension = as_extension_data!(arena, node, QuarkdownBlock);
            let call_span = checked_segment(extension.call, source)?;
            match scribium_quarkdown::parse_call(source.get(call_span.start..call_span.end)?) {
                Ok(Some((call, _))) => Some(directive_block(
                    call,
                    arena,
                    node,
                    source,
                    base,
                    call_span.start,
                    diagnostics,
                )),
                Ok(None) => Some(Block::Raw {
                    source: source.get(call_span.start..call_span.end)?.to_string(),
                    span,
                }),
                Err(error) => {
                    diagnostics.push(ParserDiagnostic {
                        code: error.code,
                        message: error.message,
                        span: add_base(error.span, base),
                    });
                    Some(Block::Raw {
                        source: source.get(call_span.start..call_span.end)?.to_string(),
                        span,
                    })
                }
            }
        }
        KindData::HtmlBlock(_) => Some(Block::Raw {
            source: source
                .get(span.start.saturating_sub(base)..span.end.saturating_sub(base))?
                .to_string(),
            span,
        }),
        _ => {
            let children = convert_children_blocks(arena, node, source, base, diagnostics);
            if children.is_empty() {
                None
            } else {
                Some(Block::Blockquote {
                    content: children,
                    span,
                })
            }
        }
    }
}

fn directive_block(
    call: QuarkdownCall,
    arena: &Arena,
    node: NodeRef,
    source: &str,
    base: usize,
    call_base: usize,
    diagnostics: &mut Vec<ParserDiagnostic>,
) -> Block {
    let span = node_span(arena, node, source)
        .map(|value| add_base(value, base))
        .unwrap_or_else(|| add_base(call.span, call_base));
    let body_nodes = convert_children_blocks(arena, node, source, base, diagnostics);
    Block::DirectiveCall {
        name: call.name,
        positional_args: call
            .positional_args
            .iter()
            .map(|arg| convert_arg(arg, source, base, call_base, diagnostics))
            .collect(),
        named_args: call
            .named_args
            .iter()
            .map(|arg| {
                (
                    arg.name.clone(),
                    convert_arg(&arg.value, source, base, call_base, diagnostics),
                )
            })
            .collect(),
        body: (!body_nodes.is_empty()).then_some(body_nodes),
        span,
    }
}

fn convert_children_blocks(
    arena: &Arena,
    node: NodeRef,
    source: &str,
    base: usize,
    diagnostics: &mut Vec<ParserDiagnostic>,
) -> Vec<Block> {
    arena[node]
        .children(arena)
        .filter_map(|child| convert_block(arena, child, source, base, diagnostics))
        .collect()
}

fn convert_inlines(
    arena: &Arena,
    node: NodeRef,
    source: &str,
    base: usize,
    diagnostics: &mut Vec<ParserDiagnostic>,
) -> Vec<Inline> {
    arena[node]
        .children(arena)
        .filter_map(|child| convert_inline(arena, child, source, base, diagnostics))
        .collect()
}

fn convert_inline(
    arena: &Arena,
    node: NodeRef,
    source: &str,
    base: usize,
    diagnostics: &mut Vec<ParserDiagnostic>,
) -> Option<Inline> {
    let local_span = if matches!(arena[node].kind_data(), KindData::CodeSpan(_)) {
        code_span_span(arena, node, source)
    } else {
        node_span(arena, node, source)
    }?;
    let span = add_base(local_span, base);
    match arena[node].kind_data() {
        KindData::Text(text) => {
            let local = text
                .index()
                .and_then(|index| checked_index(*index, source))
                .or_else(|| node_span(arena, node, source))?;
            Some(Inline::Text {
                content: source.get(local.start..local.end)?.to_string(),
                span: add_base(local, base),
            })
        }
        KindData::Emphasis(_) => Some(Inline::Emphasis {
            content: convert_inlines(arena, node, source, base, diagnostics),
            span,
        }),
        KindData::Strong(_) => Some(Inline::Strong {
            content: convert_inlines(arena, node, source, base, diagnostics),
            span,
        }),
        KindData::CodeSpan(_code) => Some(Inline::Code {
            content: code_span_content(
                source.get(span.start.saturating_sub(base)..span.end.saturating_sub(base))?,
            ),
            span,
        }),
        KindData::Link(link) => Some(Inline::Link {
            content: convert_inlines(arena, node, source, base, diagnostics),
            destination: checked_value(link.destination(), source)?.to_string(),
            span,
        }),
        KindData::Image(image) => Some(Inline::Image {
            content: convert_inlines(arena, node, source, base, diagnostics),
            destination: checked_value(image.destination(), source)?.to_string(),
            span,
        }),
        KindData::RawHtml(_html) => Some(Inline::RawHtml {
            content: source
                .get(span.start.saturating_sub(base)..span.end.saturating_sub(base))?
                .to_string(),
            span,
        }),
        KindData::Strikethrough(_) => Some(Inline::Strikethrough {
            content: convert_inlines(arena, node, source, base, diagnostics),
            span,
        }),
        KindData::Extension(_) if matches_extension_kind!(arena, node, QuarkdownInline) => {
            let extension = as_extension_data!(arena, node, QuarkdownInline);
            let call_span = checked_segment(extension.call, source)?;
            let parsed = scribium_quarkdown::parse_inline_call(source, call_span.start);
            let call = match parsed {
                Ok(Some((call, _))) => call,
                Ok(None) => {
                    return Some(Inline::Text {
                        content: source.get(call_span.start..call_span.end)?.to_string(),
                        span,
                    })
                }
                Err(error) => {
                    diagnostics.push(ParserDiagnostic {
                        code: error.code,
                        message: error.message,
                        span: add_base(error.span, base),
                    });
                    return None;
                }
            };
            Some(Inline::DirectiveCall {
                name: call.name,
                positional_args: call
                    .positional_args
                    .iter()
                    .map(|arg| convert_arg(arg, source, base, 0, diagnostics))
                    .collect(),
                named_args: call
                    .named_args
                    .iter()
                    .map(|arg| {
                        (
                            arg.name.clone(),
                            convert_arg(&arg.value, source, base, 0, diagnostics),
                        )
                    })
                    .collect(),
                body: None,
                span,
            })
        }
        _ => {
            let local = span.start.saturating_sub(base)..span.end.saturating_sub(base);
            Some(Inline::Text {
                content: source.get(local)?.to_string(),
                span,
            })
        }
    }
}

fn convert_arg(
    arg: &Arg,
    source: &str,
    base: usize,
    call_base: usize,
    diagnostics: &mut Vec<ParserDiagnostic>,
) -> Value {
    match &arg.content {
        ArgContent::Scalar(value) => convert_value(value),
        ArgContent::Content(content) => {
            let span = add_base(*content, call_base);
            let Some(span) = checked_local_span(span, source) else {
                diagnostics.push(ParserDiagnostic {
                    code: "E9002",
                    message: "Quarkdown content argument is outside the source".to_string(),
                    span: add_base(span, base),
                });
                return Value::String(String::new());
            };
            Value::Content(parse_inline_fragment(source, span, base, diagnostics))
        }
    }
}

fn parse_inline_fragment(
    source: &str,
    span: ByteSpan,
    base: usize,
    diagnostics: &mut Vec<ParserDiagnostic>,
) -> Vec<Inline> {
    let Some(fragment) = source.get(span.start..span.end) else {
        return Vec::new();
    };
    // A bare call at the beginning of a document is a block candidate in
    // Rushdown. Prefixing a sentinel paragraph byte forces the same inline
    // lifecycle used by a real argument label without changing source spans.
    let wrapped = format!("x {fragment}");
    let parser = parser(Mode::Quarkdown);
    let mut reader = BasicReader::new(&wrapped);
    let fragment_start = base + span.start;
    let wrapped_base = fragment_start.saturating_sub(2);
    let parsed = catch_unwind(AssertUnwindSafe(|| parser.parse(&mut reader)));
    let Ok((arena, root)) = parsed else {
        diagnostics.push(ParserDiagnostic {
            code: "E9003",
            message: "Rushdown panicked while parsing a Quarkdown content argument".to_string(),
            span,
        });
        return Vec::new();
    };
    arena[root]
        .children(&arena)
        .filter_map(|child| {
            if matches!(arena[child].kind_data(), KindData::Paragraph(_)) {
                Some(convert_inlines(
                    &arena,
                    child,
                    &wrapped,
                    wrapped_base,
                    diagnostics,
                ))
            } else {
                None
            }
        })
        .flatten()
        .filter(|inline| inline_start(inline) >= fragment_start)
        .collect()
}

fn inline_start(inline: &Inline) -> usize {
    match inline {
        Inline::Text { span, .. }
        | Inline::Emphasis { span, .. }
        | Inline::Strong { span, .. }
        | Inline::DirectiveCall { span, .. }
        | Inline::Link { span, .. }
        | Inline::Image { span, .. }
        | Inline::Code { span, .. }
        | Inline::RawHtml { span, .. }
        | Inline::Strikethrough { span, .. }
        | Inline::HardBreak { span }
        | Inline::SoftBreak { span } => span.start,
    }
}

fn inline_end(inline: &Inline) -> usize {
    match inline {
        Inline::Text { span, .. }
        | Inline::Emphasis { span, .. }
        | Inline::Strong { span, .. }
        | Inline::DirectiveCall { span, .. }
        | Inline::Link { span, .. }
        | Inline::Image { span, .. }
        | Inline::Code { span, .. }
        | Inline::RawHtml { span, .. }
        | Inline::Strikethrough { span, .. }
        | Inline::HardBreak { span }
        | Inline::SoftBreak { span } => span.end,
    }
}

fn convert_value(value: &QuarkdownValue) -> Value {
    match value {
        QuarkdownValue::String(value) => Value::String(value.clone()),
        QuarkdownValue::Number(value) => Value::Number(*value),
        QuarkdownValue::Boolean(value) => Value::Boolean(*value),
        QuarkdownValue::Identifier(value) => Value::Identifier(value.clone()),
    }
}

fn node_span(arena: &Arena, node: NodeRef, source: &str) -> Option<ByteSpan> {
    if let KindData::Text(text) = arena[node].kind_data() {
        if let Some(index) = text.index() {
            return checked_index(*index, source);
        }
    }
    if let KindData::CodeBlock(code) = arena[node].kind_data() {
        return code_block_span(arena, node, code, source);
    }
    let mut start: Option<usize> = arena[node].pos();
    let mut end: Option<usize> = None;
    if matches_extension_kind!(arena, node, QuarkdownBlock) {
        let extension = as_extension_data!(arena, node, QuarkdownBlock);
        let call_span = checked_segment(extension.call, source)?;
        start = Some(start.map_or(call_span.start, |value| value.min(call_span.start)));
        end = Some(call_span.end);
    } else if matches_extension_kind!(arena, node, QuarkdownInline) {
        let extension = as_extension_data!(arena, node, QuarkdownInline);
        let call_span = checked_segment(extension.call, source)?;
        start = Some(start.map_or(call_span.start, |value| value.min(call_span.start)));
        end = Some(call_span.end);
    }
    if let TypeData::Block(block) = arena[node].type_data() {
        for segment in block.source() {
            let span = checked_segment(*segment, source)?;
            start = Some(start.map_or(span.start, |value| value.min(span.start)));
            end = Some(end.map_or(span.end, |value| value.max(span.end)));
        }
    }
    for child in arena[node].children(arena) {
        if let Some(child_span) = node_span(arena, child, source) {
            start = Some(start.map_or(child_span.start, |value| value.min(child_span.start)));
            end = Some(end.map_or(child_span.end, |value| value.max(child_span.end)));
        }
    }
    let span = ByteSpan::new(start?, end?);
    span.is_valid_for(source).then_some(span)
}

fn code_block_span(
    arena: &Arena,
    node: NodeRef,
    code: &rushdown::ast::CodeBlock,
    source: &str,
) -> Option<ByteSpan> {
    let start = arena[node].pos()?;
    let end = match code.value() {
        Lines::Segments(segments) => segments.iter().map(Segment::stop).max(),
        _ => None,
    }?;
    let end = if code.code_block_kind() == CodeBlockKind::Fenced {
        fenced_code_end(source, start).unwrap_or(end)
    } else {
        end
    };
    let span = ByteSpan::new(start, end);
    span.is_valid_for(source).then_some(span)
}

fn fenced_code_end(source: &str, start: usize) -> Option<usize> {
    let first_end = source
        .get(start..)?
        .find('\n')
        .map_or(source.len(), |offset| start + offset + 1);
    let opening = source
        .get(start..first_end)?
        .trim_start_matches([' ', '\t']);
    let marker = opening.as_bytes().first().copied()?;
    if !matches!(marker, b'`' | b'~') {
        return None;
    }
    let length = opening.bytes().take_while(|byte| *byte == marker).count();
    if length < 3 {
        return None;
    }
    let mut line_start = first_end;
    while line_start < source.len() {
        let line_end = source
            .get(line_start..)?
            .find('\n')
            .map_or(source.len(), |offset| line_start + offset + 1);
        let line = source
            .get(line_start..line_end)?
            .trim_start_matches([' ', '\t']);
        if line.bytes().take_while(|byte| *byte == marker).count() >= length
            && line
                .bytes()
                .skip_while(|byte| *byte == marker)
                .all(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n'))
        {
            return Some(line_end);
        }
        line_start = line_end;
    }
    None
}

fn checked_index(index: rushdown::text::Index, source: &str) -> Option<ByteSpan> {
    let span = ByteSpan::new(index.start(), index.stop());
    span.is_valid_for(source).then_some(span)
}

fn checked_segment(segment: Segment, source: &str) -> Option<ByteSpan> {
    let span = ByteSpan::new(segment.start(), segment.stop());
    span.is_valid_for(source).then_some(span)
}

fn checked_local_span(span: ByteSpan, source: &str) -> Option<ByteSpan> {
    span.is_valid_for(source).then_some(span)
}

fn checked_value<'a>(value: &'a rushdown::text::Value, source: &'a str) -> Option<&'a str> {
    match value {
        rushdown::text::Value::Index(index) => checked_index(*index, source)?.checked_str(source),
        rushdown::text::Value::String(value) => Some(value.as_str()),
        _ => None,
    }
}

fn add_base(span: ByteSpan, base: usize) -> ByteSpan {
    ByteSpan::new(span.start + base, span.end + base)
}

fn code_language(code: &rushdown::ast::CodeBlock, source: &str) -> Option<String> {
    checked_value(code.info()?, source)
        .and_then(|value| value.split_whitespace().next())
        .map(ToOwned::to_owned)
}

fn code_block_source(
    arena: &Arena,
    node: NodeRef,
    code: &rushdown::ast::CodeBlock,
    source: &str,
) -> Option<String> {
    let Lines::Segments(segments) = code.value() else {
        return None;
    };
    let lines = segments
        .iter()
        .map(|segment| {
            checked_segment(*segment, source)?
                .checked_str(source)
                .map(str::to_owned)
        })
        .collect::<Option<Vec<_>>>()?;
    let content = lines.join("\n");
    if is_nested_in_list(arena, node) {
        Some(
            content
                .lines()
                .map(|line| format!(" {line}"))
                .collect::<Vec<_>>()
                .join("\n"),
        )
    } else {
        Some(content)
    }
}

fn is_nested_in_list(arena: &Arena, node: NodeRef) -> bool {
    let mut parent = arena[node].parent();
    while let Some(parent_ref) = parent {
        if matches!(arena[parent_ref].kind_data(), KindData::ListItem(_)) {
            return true;
        }
        parent = arena[parent_ref].parent();
    }
    false
}

fn code_source(raw: &str) -> String {
    let Some(first_newline) = raw.find('\n') else {
        return String::new();
    };
    let body_start = first_newline + 1;
    let body_end = raw.rfind("```").unwrap_or(raw.len());
    raw.get(body_start..body_end)
        .unwrap_or_default()
        .to_string()
}

fn code_span_content(raw: &str) -> String {
    let delimiter_len = raw.bytes().take_while(|byte| *byte == b'`').count();
    if delimiter_len == 0 || raw.len() < delimiter_len * 2 {
        return raw.to_string();
    }
    let closing_start = raw.len() - delimiter_len;
    if raw.as_bytes()[closing_start..]
        .iter()
        .any(|byte| *byte != b'`')
    {
        return raw.to_string();
    }
    let inner = &raw[delimiter_len..closing_start];
    let normalized = inner.replace("\r\n", " ").replace(['\r', '\n'], " ");
    if normalized.starts_with(' ')
        && normalized.ends_with(' ')
        && normalized.chars().any(|character| character != ' ')
    {
        normalized[1..normalized.len() - 1].to_string()
    } else {
        normalized
    }
}

fn code_span_span(arena: &Arena, node: NodeRef, source: &str) -> Option<ByteSpan> {
    let start = arena[node].pos()?;
    let bytes = source.as_bytes();
    let delimiter_len = bytes
        .get(start..)?
        .iter()
        .take_while(|byte| **byte == b'`')
        .count();
    if delimiter_len == 0 {
        return None;
    }
    let mut cursor = start + delimiter_len;
    while cursor + delimiter_len <= bytes.len() {
        if bytes[cursor..].starts_with(&bytes[start..start + delimiter_len])
            && !bytes
                .get(cursor + delimiter_len)
                .is_some_and(|byte| *byte == b'`')
        {
            let span = ByteSpan::new(start, cursor + delimiter_len);
            return span.is_valid_for(source).then_some(span);
        }
        cursor += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_mode_keeps_quarkdown_as_text() {
        let document = parse_md(".foo {bar}\n");
        assert!(matches!(
            document.nodes.first(),
            Some(Block::Paragraph { .. })
        ));
    }

    #[test]
    fn qd_mode_preserves_nested_body_and_utf8_spans() {
        let source = ".align {center}\n    한글 **본문**\n";
        let output = parse_with_diagnostics(source);
        assert!(output.diagnostics.is_empty());
        match &output.document.nodes[0] {
            Block::DirectiveCall { body, .. } => {
                assert!(body.as_ref().is_some_and(|body| !body.is_empty()));
            }
            other => panic!("unexpected node: {other:?}"),
        }
    }

    #[test]
    fn code_shields_quarkdown_extension() {
        let document = parse_qd("```text\n.foo {bar}\n```\n");
        fn count_calls(blocks: &[Block]) -> usize {
            blocks
                .iter()
                .map(|block| match block {
                    Block::DirectiveCall { .. } => 1,
                    Block::Blockquote { content, .. } => count_calls(content),
                    Block::UnorderedList { items, .. } | Block::OrderedList { items, .. } => {
                        items.iter().map(|item| count_calls(&item.content)).sum()
                    }
                    _ => 0,
                })
                .sum()
        }
        assert_eq!(count_calls(&document.nodes), 0);
    }

    #[test]
    fn code_span_content_excludes_delimiters() {
        let document = parse_md("`code`\n");
        let Block::Paragraph { content, .. } = &document.nodes[0] else {
            panic!()
        };
        let Inline::Code { content, .. } = &content[0] else {
            panic!()
        };
        assert_eq!(content, "code");
    }
}
