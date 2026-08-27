use std::fmt::{self, Write};
use std::panic::{catch_unwind, AssertUnwindSafe};

use rushdown::ast::{
    Arena, CodeBlockKind, KindData, NodeKind, NodeRef, NodeType, PrettyPrint, TableCellAlignment,
    Task, TextQualifier, TypeData,
};
use rushdown::parser::{
    gfm, parser_extension, BlockParser, Context, GfmOptions, InlineParser, NoParserOptions,
    Options, Parser, ParserExtension, State, PRIORITY_CODE_SPAN, PRIORITY_FENCED_CODE_BLOCK,
};
use rushdown::text::{BasicReader, BlockReader, Lines, Reader, Segment};
use rushdown::util::{indent_position, indent_width, is_blank};
use rushdown::{as_extension_data, as_extension_data_mut, matches_extension_kind};
use scribium_quarkdown::{Arg, ArgContent, QuarkdownCall, Value as QuarkdownValue};
use scribium_source::ByteSpan;

use crate::ast::{
    Block, CallSegment, Document, FrontMatter, Inline, ListItem, NamedArg, RangeValue, TableCell,
    TableRow, TaskStatus, Value,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Markdown,
    Quarkdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkdownProfile {
    CommonMark,
    Gfm,
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

type BodyLineRanges = Vec<(ByteSpan, Vec<ByteSpan>)>;

#[derive(Debug)]
struct QuarkdownBlock {
    call: Segment,
    call_start: usize,
    header_pending: bool,
    continuation_pending: bool,
    /// The first qualifying body's visual indentation in the current reader
    /// context. This is never an absolute source-column measurement.
    body_indent: Option<usize>,
    /// Original reader segments accepted as body lines. These preserve parser
    /// ownership for the frontend's lazy-paragraph normalization.
    body_lines: Vec<Segment>,
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
        let needs_more_input = scribium_quarkdown::needs_more_input(line_source);
        let continuation_pending = scribium_quarkdown::has_trailing_continuation(line_source);
        let (call_segment, header_pending) = match scribium_quarkdown::parse_call(line_source) {
            Ok(Some((_call, _call_end))) if needs_more_input => {
                (Segment::new(start, segment.stop()), true)
            }
            Ok(Some((call, call_end))) => {
                if line_source.as_bytes()[call_end..]
                    .iter()
                    .any(|byte| !byte.is_ascii_whitespace())
                {
                    return None;
                }
                (
                    Segment::new(call.span.start + start, call.span.end + start),
                    false,
                )
            }
            Ok(None) => return None,
            Err(_) if needs_more_input => (Segment::new(start, segment.stop()), true),
            Err(_) => (Segment::new(start, start + line_end), false),
        };
        let node_ref = arena.new_node(QuarkdownBlock {
            call: call_segment,
            call_start: start,
            header_pending,
            continuation_pending: header_pending && continuation_pending,
            body_indent: None,
            body_lines: Vec::new(),
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

        if as_extension_data!(arena, node_ref, QuarkdownBlock).header_pending {
            let source = reader.source();
            let call_start = as_extension_data!(arena, node_ref, QuarkdownBlock).call_start;
            let candidate_end = segment.stop().min(source.len());
            let candidate = source.get(call_start..candidate_end)?;
            let continuation_pending =
                as_extension_data!(arena, node_ref, QuarkdownBlock).continuation_pending;
            // Let the Quarkdown grammar classify the complete original
            // candidate. A second line-prefix scanner would drift from the
            // shared identifier and delimiter contract.
            match scribium_quarkdown::parse_call(candidate) {
                Ok(Some((call, _))) => {
                    let call_end = call.span.end.checked_add(call_start)?;
                    let trailing = source.get(call_end..candidate_end)?;
                    let has_continuation = scribium_quarkdown::has_trailing_continuation(candidate);
                    if trailing.bytes().all(|byte| byte.is_ascii_whitespace()) {
                        let block = as_extension_data_mut!(arena, node_ref, QuarkdownBlock);
                        block.call = Segment::new(call_start, call_end);
                        block.header_pending = false;
                        block.continuation_pending = false;
                    } else if has_continuation {
                        as_extension_data_mut!(arena, node_ref, QuarkdownBlock)
                            .continuation_pending = true;
                    } else {
                        return None;
                    }
                }
                Err(error)
                    if error.code == "E2003"
                        || (continuation_pending
                            && scribium_quarkdown::has_trailing_continuation(candidate)) => {}
                _ => return None,
            }
            reader.advance_to_eol();
            return Some(State::HAS_CHILDREN);
        }

        if is_blank(&line) {
            reader.advance_to_eol();
            return Some(State::HAS_CHILDREN);
        }

        let (actual_indent, _) = indent_width(&line, reader.line_offset());
        let body_indent = {
            let block = as_extension_data!(arena, node_ref, QuarkdownBlock);
            if let Some(body_indent) = block.body_indent {
                if actual_indent < body_indent {
                    return None;
                }
                body_indent
            } else {
                let has_minimum_indent = actual_indent >= 2 || line.first() == Some(&b'\t');
                if !has_minimum_indent {
                    return None;
                }
                actual_indent
            }
        };

        if as_extension_data!(arena, node_ref, QuarkdownBlock)
            .body_indent
            .is_none()
        {
            as_extension_data_mut!(arena, node_ref, QuarkdownBlock).body_indent =
                Some(actual_indent);
        }

        as_extension_data_mut!(arena, node_ref, QuarkdownBlock)
            .body_lines
            .push(segment);

        let (position, padding) = indent_position(&line, reader.line_offset(), body_indent)?;
        reader.advance_and_set_padding(position, padding);
        Some(State::HAS_CHILDREN)
    }

    fn can_interrupt_paragraph(&self) -> bool {
        true
    }
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
            Ok(Some((call, end))) => (
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

#[derive(Debug)]
struct QuarkdownTightInlineParser;

impl InlineParser for QuarkdownTightInlineParser {
    fn trigger(&self) -> &[u8] {
        b"{"
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
        let (call, end) = match scribium_quarkdown::parse_tight_call(source, segment.start()) {
            Ok(Some((call, end))) => (call, end),
            Ok(None) | Err(_) => return None,
        };
        let consumed = end.checked_sub(segment.start())?;
        reader.advance(consumed);
        Some(arena.new_node(QuarkdownInline {
            call: Segment::new(call.span.start, call.span.end),
        }))
    }
}

fn parser(mode: Mode, profile: MarkdownProfile) -> Parser {
    if mode == Mode::Markdown && profile == MarkdownProfile::CommonMark {
        return Parser::with_options(Options::default());
    }
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
            parser.add_inline_parser(
                || -> Box<dyn InlineParser> { Box::new(QuarkdownTightInlineParser) },
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
    parse_source(source, mode, MarkdownProfile::Gfm)
}

pub fn parse_with_markdown_profile(source: &str, profile: MarkdownProfile) -> ParseOutput {
    parse_source(source, Mode::Markdown, profile)
}

fn parse_source(source: &str, mode: Mode, profile: MarkdownProfile) -> ParseOutput {
    let (front_matter, body_start) = parse_front_matter(source);
    let body = &source[body_start..];
    let parser = parser(mode, profile);
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
    let mut body_line_ranges = Vec::new();
    for child in arena[root].children(&arena) {
        if let Some(node) = convert_block(
            &arena,
            child,
            body,
            body_start,
            &mut diagnostics,
            &mut body_line_ranges,
        ) {
            nodes.push(node);
        }
    }
    if mode == Mode::Quarkdown {
        let original = std::mem::take(&mut nodes);
        for mut node in original {
            nodes.extend(normalize_block(
                &mut node,
                &body_line_ranges,
                source,
                &mut diagnostics,
            ));
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

fn normalize_block(
    block: &mut Block,
    body_line_ranges: &BodyLineRanges,
    source: &str,
    diagnostics: &mut Vec<ParserDiagnostic>,
) -> Vec<Block> {
    match block {
        Block::DirectiveCall {
            name,
            body: Some(body),
            span,
            ..
        } => {
            let accepted_lines = body_line_ranges
                .iter()
                .find(|(owner, _)| owner == span)
                .map(|(_, lines)| lines.as_slice());
            let children = std::mem::take(body);
            let mut normalized_children = Vec::new();
            for mut child in children {
                normalized_children.extend(normalize_block(
                    &mut child,
                    body_line_ranges,
                    source,
                    diagnostics,
                ));
            }

            let Some(accepted_lines) = accepted_lines else {
                *body = normalized_children;
                return vec![block.clone()];
            };

            let mut kept = Vec::new();
            let mut promoted = Vec::new();
            for child in normalized_children {
                for (is_body, piece) in split_block_by_body_lines(child, accepted_lines) {
                    if is_body {
                        kept.push(piece);
                    } else {
                        promoted.push(piece);
                    }
                }
            }
            *body = kept;
            if has_lambda_body_semantics(name) {
                contextualize_lambda_body(block, accepted_lines, source, diagnostics);
            }
            let mut result = vec![block.clone()];
            result.extend(promoted);
            result
        }
        Block::Blockquote { content, .. } => {
            normalize_children(content, body_line_ranges, source, diagnostics);
            vec![block.clone()]
        }
        Block::UnorderedList { items, .. } | Block::OrderedList { items, .. } => {
            for item in items {
                normalize_children(&mut item.content, body_line_ranges, source, diagnostics);
            }
            vec![block.clone()]
        }
        _ => vec![block.clone()],
    }
}

fn normalize_children(
    children: &mut Vec<Block>,
    body_line_ranges: &BodyLineRanges,
    source: &str,
    diagnostics: &mut Vec<ParserDiagnostic>,
) {
    let original = std::mem::take(children);
    for mut child in original {
        children.extend(normalize_block(
            &mut child,
            body_line_ranges,
            source,
            diagnostics,
        ));
    }
}

/// Returns the small set of block calls whose first body line is a
/// source-backed lambda header rather than ordinary Markdown content.
///
/// This must remain contextual. Treating every `name:` body line as a lambda
/// header would change the meaning of ordinary calls such as `.container`.
fn has_lambda_body_semantics(name: &str) -> bool {
    matches!(
        name,
        "function" | "let" | "foreach" | "repeat" | "map" | "filter" | "sorted"
    )
}

fn contextualize_lambda_body(
    block: &mut Block,
    accepted_lines: &[ByteSpan],
    source: &str,
    diagnostics: &mut Vec<ParserDiagnostic>,
) {
    let Some(first_line) = accepted_lines.first().copied() else {
        return;
    };
    let header = match scribium_quarkdown::parse_lambda_header(source, first_line) {
        Ok(header) => header,
        Err(error) => {
            diagnostics.push(ParserDiagnostic {
                code: error.code,
                message: error.message,
                span: error.span,
            });
            return;
        }
    };
    let Some(header) = header else {
        return;
    };

    let frontend_header = crate::ast::LambdaHeader {
        parameters: header
            .parameters
            .into_iter()
            .map(|parameter| crate::ast::LambdaParameter {
                name: parameter.name,
                name_span: parameter.name_span,
                span: parameter.span,
                optional: parameter.optional,
            })
            .collect(),
        span: header.span,
    };
    let line_end = line_end_with_terminator(first_line, source);
    let Block::DirectiveCall {
        body: Some(body),
        lambda_header,
        ..
    } = block
    else {
        return;
    };
    *lambda_header = Some(frontend_header);
    strip_lambda_header_line(body, line_end, source);
}

fn line_end_with_terminator(line: ByteSpan, source: &str) -> usize {
    let mut end = line.end.min(source.len());
    if source
        .get(end..)
        .is_some_and(|rest| rest.starts_with("\r\n"))
    {
        end += 2;
    } else if source.get(end..).is_some_and(|rest| rest.starts_with('\n')) {
        end += 1;
    }
    end
}

fn strip_lambda_header_line(body: &mut Vec<Block>, line_end: usize, source: &str) {
    let Some(Block::Paragraph { content, span }) = body.first_mut() else {
        return;
    };
    let original = std::mem::take(content);
    let mut kept = Vec::new();
    for inline in original {
        let start = inline_start(&inline);
        let end = inline_end(&inline);
        if end <= line_end {
            continue;
        }
        if start < line_end {
            if let Inline::Text { content, span } = inline {
                if line_end <= span.end && source.is_char_boundary(line_end) {
                    if let Some(suffix) = source.get(line_end..span.end) {
                        kept.push(Inline::Text {
                            content: suffix.to_string(),
                            span: ByteSpan::new(line_end, span.end),
                        });
                    }
                } else if !content.is_empty() {
                    kept.push(Inline::Text { content, span });
                }
            }
            continue;
        }
        kept.push(inline);
    }
    *content = kept;
    if content.is_empty() {
        body.remove(0);
    } else {
        // The paragraph originally covered the header and the surviving body
        // text. Re-anchor it to the original spans of the remaining inline
        // nodes so provenance does not retain the removed header bytes.
        *span = paragraph_span(content);
    }
}

fn split_block_by_body_lines(block: Block, accepted_lines: &[ByteSpan]) -> Vec<(bool, Block)> {
    let Block::Paragraph { content, span } = block else {
        return vec![(true, block)];
    };
    if content.is_empty() {
        return vec![(true, Block::Paragraph { content, span })];
    }

    let mut groups: Vec<(bool, Vec<Inline>)> = Vec::new();
    for inline in content {
        let is_body = accepted_lines
            .iter()
            .any(|line| line.start <= inline_start(&inline) && inline_start(&inline) < line.end);
        if groups
            .last()
            .is_none_or(|(previous, _)| *previous != is_body)
        {
            groups.push((is_body, Vec::new()));
        }
        groups.last_mut().expect("group created").1.push(inline);
    }

    groups
        .into_iter()
        .map(|(is_body, content)| {
            let span = paragraph_span(&content);
            (is_body, Block::Paragraph { content, span })
        })
        .collect()
}

fn paragraph_span(content: &[Inline]) -> ByteSpan {
    let start = content.first().map(inline_start).unwrap_or(0);
    let end = content.last().map(inline_end).unwrap_or(start);
    ByteSpan::new(start, end)
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
    body_line_ranges: &mut BodyLineRanges,
) -> Option<Block> {
    let span = node_span(arena, node, source).and_then(|span| offset_span(span, base))?;
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
            content: convert_children_blocks(
                arena,
                node,
                source,
                base,
                diagnostics,
                body_line_ranges,
            ),
            span,
        }),
        KindData::List(list) => {
            let items = arena[node]
                .children(arena)
                .filter_map(|child| match arena[child].kind_data() {
                    KindData::ListItem(item) => Some(ListItem {
                        content: convert_children_blocks(
                            arena,
                            child,
                            source,
                            base,
                            diagnostics,
                            body_line_ranges,
                        ),
                        span: node_span(arena, child, source)
                            .and_then(|value| offset_span(value, base))
                            .unwrap_or(span),
                        task: item.task().map(|task| match task {
                            Task::Active => TaskStatus::Active,
                            Task::Completed => TaskStatus::Completed,
                            _ => TaskStatus::Active,
                        }),
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
        KindData::Table(_) => Some(convert_table(arena, node, source, base, span, diagnostics)),
        KindData::CodeBlock(code) => {
            let raw = source.get(span.start.saturating_sub(base)..span.end.saturating_sub(base))?;
            let info = code_info(code, source)
                .map(|value| normalize_metadata(&value, MetadataKind::CodeInfo));
            let language = info
                .as_deref()
                .and_then(|value| value.split_whitespace().next())
                .map(ToOwned::to_owned);
            let body = code_block_source(code, source).unwrap_or_else(|| code_source(raw));
            Some(Block::CodeBlock {
                language,
                info,
                source: body,
                span,
            })
        }
        KindData::Extension(_) if matches_extension_kind!(arena, node, QuarkdownBlock) => {
            let extension = as_extension_data!(arena, node, QuarkdownBlock);
            let ranges = extension
                .body_lines
                .iter()
                .map(|segment| offset_span(ByteSpan::new(segment.start(), segment.stop()), base))
                .collect::<Option<Vec<_>>>()?;
            if !ranges.is_empty() {
                body_line_ranges.push((span, ranges));
            }
            let call_span = checked_segment(extension.call, source)?;
            match scribium_quarkdown::parse_call(source.get(call_span.start..call_span.end)?) {
                Ok(Some((call, _))) => Some(directive_block(
                    call,
                    arena,
                    node,
                    source,
                    base,
                    call_span.start,
                    ConversionState {
                        diagnostics,
                        body_line_ranges,
                    },
                )),
                Ok(None) => Some(Block::Unsupported {
                    kind: "malformed Quarkdown block call".to_string(),
                    span,
                }),
                Err(error) => {
                    let diagnostic_span = offset_span(error.span, call_span.start)
                        .and_then(|body_span| offset_span(body_span, base))
                        .unwrap_or(span);
                    diagnostics.push(ParserDiagnostic {
                        code: error.code,
                        message: error.message,
                        span: diagnostic_span,
                    });
                    Some(Block::Unsupported {
                        kind: "malformed Quarkdown block call".to_string(),
                        span,
                    })
                }
            }
        }
        KindData::HtmlBlock(_) => Some(Block::RawHtml {
            source: source
                .get(span.start.saturating_sub(base)..span.end.saturating_sub(base))?
                .to_string(),
            span,
        }),
        KindData::LinkReferenceDefinition(_) => None,
        _ => Some(Block::Unsupported {
            kind: arena[node].kind_data().kind_name().to_string(),
            span,
        }),
    }
}

fn convert_table(
    arena: &Arena,
    node: NodeRef,
    source: &str,
    base: usize,
    span: ByteSpan,
    diagnostics: &mut Vec<ParserDiagnostic>,
) -> Block {
    let mut header = None;
    let mut rows = Vec::new();
    for child in arena[node].children(arena) {
        match arena[child].kind_data() {
            KindData::TableHeader(_) => {
                header = arena[child]
                    .children(arena)
                    .find_map(|row| convert_table_row(arena, row, source, base, diagnostics));
            }
            KindData::TableBody(_) => {
                rows.extend(
                    arena[child]
                        .children(arena)
                        .filter_map(|row| convert_table_row(arena, row, source, base, diagnostics)),
                );
            }
            _ => diagnostics.push(ParserDiagnostic {
                code: "E3011",
                message: "Rushdown table contains an unsupported structural node".to_string(),
                span,
            }),
        }
    }
    Block::Table {
        header: header.unwrap_or(TableRow {
            cells: Vec::new(),
            span,
        }),
        rows,
        span,
    }
}

fn convert_table_row(
    arena: &Arena,
    node: NodeRef,
    source: &str,
    base: usize,
    diagnostics: &mut Vec<ParserDiagnostic>,
) -> Option<TableRow> {
    if !matches!(arena[node].kind_data(), KindData::TableRow(_)) {
        return None;
    }
    let span = node_span(arena, node, source).and_then(|value| offset_span(value, base))?;
    let cells = arena[node]
        .children(arena)
        .filter_map(|cell| match arena[cell].kind_data() {
            KindData::TableCell(table_cell) => Some(TableCell {
                content: convert_inlines(arena, cell, source, base, diagnostics),
                alignment: match table_cell.alignment() {
                    TableCellAlignment::Left => crate::ast::TableAlignment::Left,
                    TableCellAlignment::Center => crate::ast::TableAlignment::Center,
                    TableCellAlignment::Right => crate::ast::TableAlignment::Right,
                    TableCellAlignment::None => crate::ast::TableAlignment::None,
                    _ => crate::ast::TableAlignment::None,
                },
                span: node_span(arena, cell, source)
                    .and_then(|value| offset_span(value, base))
                    .unwrap_or(span),
            }),
            _ => None,
        })
        .collect();
    Some(TableRow { cells, span })
}

struct ConversionState<'a> {
    diagnostics: &'a mut Vec<ParserDiagnostic>,
    body_line_ranges: &'a mut BodyLineRanges,
}

fn directive_block(
    call: QuarkdownCall,
    arena: &Arena,
    node: NodeRef,
    source: &str,
    base: usize,
    call_base: usize,
    state: ConversionState<'_>,
) -> Block {
    let ConversionState {
        diagnostics,
        body_line_ranges,
    } = state;
    let span = node_span(arena, node, source)
        .and_then(|value| offset_span(value, base))
        .or_else(|| offset_span(call.span, base.checked_add(call_base)?))
        .unwrap_or(ByteSpan::new(0, 0));
    let span_base = base.checked_add(call_base).unwrap_or(base);
    let call_name = call.name.clone();
    let body_nodes =
        convert_children_blocks(arena, node, source, base, diagnostics, body_line_ranges);
    Block::DirectiveCall {
        name: call.name,
        name_span: offset_span(call.name_span, span_base).unwrap_or(ByteSpan::new(0, 0)),
        head_span: offset_span(call.head_span, span_base).unwrap_or(ByteSpan::new(0, 0)),
        positional_args: call
            .positional_args
            .iter()
            .enumerate()
            .map(|(index, arg)| {
                convert_arg_with_mode(
                    arg,
                    source,
                    base,
                    call_base,
                    diagnostics,
                    is_contextual_inline_body_position(&call_name, index),
                    is_contextual_inline_body_position(&call_name, index),
                )
            })
            .collect(),
        named_args: call
            .named_args
            .iter()
            .map(|arg| {
                convert_named_arg(
                    arg,
                    source,
                    base,
                    call_base,
                    diagnostics,
                    Some(call_name.as_str()),
                )
            })
            .collect(),
        chain: call
            .chain
            .iter()
            .map(|segment| convert_call_segment(segment, source, base, call_base, diagnostics))
            .collect(),
        body: (!body_nodes.is_empty()).then_some(body_nodes),
        lambda_header: None,
        span,
    }
}

fn convert_children_blocks(
    arena: &Arena,
    node: NodeRef,
    source: &str,
    base: usize,
    diagnostics: &mut Vec<ParserDiagnostic>,
    body_line_ranges: &mut BodyLineRanges,
) -> Vec<Block> {
    arena[node]
        .children(arena)
        .filter_map(|child| {
            convert_block(arena, child, source, base, diagnostics, body_line_ranges)
        })
        .collect()
}

fn convert_inlines(
    arena: &Arena,
    node: NodeRef,
    source: &str,
    base: usize,
    diagnostics: &mut Vec<ParserDiagnostic>,
) -> Vec<Inline> {
    let mut inlines = Vec::new();
    let mut previous = None;
    let mut children = arena[node].children(arena).peekable();
    while let Some(child) = children.next() {
        let next_is_code_span = children
            .peek()
            .is_some_and(|next| matches!(arena[*next].kind_data(), KindData::CodeSpan(_)));
        if duplicate_autolink_closer(arena, previous, child, source) {
            previous = Some(child);
            continue;
        }
        if let Some(inline) = convert_inline(arena, child, source, base, diagnostics) {
            // Rushdown represents the line break between adjacent code spans
            // as a zero-width text node plus a soft-break qualifier. It has no
            // semantic text and belongs to neither code span.
            let is_zero_width_code_boundary = inlines.last().is_some_and(|previous| {
                matches!(previous, Inline::Code { .. })
                    && matches!(&inline, Inline::Text { span, .. } if span.start == span.end)
                    && next_is_code_span
            });
            let is_empty_text =
                matches!(&inline, Inline::Text { content, .. } if content.is_empty());
            if !is_zero_width_code_boundary && !is_empty_text {
                inlines.push(inline);
            }
        }
        let line_break = text_line_break(arena, child, source, base);
        if matches!(line_break, Some(Inline::HardBreak { .. })) {
            exclude_hard_break_delimiter_whitespace(&mut inlines, arena, child, source, base);
        }
        if let Some(line_break) = line_break {
            inlines.push(line_break);
        }
        previous = Some(child);
    }
    inlines
}

/// Remove only the source spaces that Rushdown classified as a hard-break
/// delimiter from semantic text. The text span remains unchanged so the
/// original delimiter bytes stay represented by the source-backed AST.
fn exclude_hard_break_delimiter_whitespace(
    inlines: &mut [Inline],
    arena: &Arena,
    node: NodeRef,
    source: &str,
    base: usize,
) {
    let KindData::Text(text) = arena[node].kind_data() else {
        return;
    };
    if !text.has_qualifiers(TextQualifier::HARD_LINE_BREAK) {
        return;
    }
    let Some(index) = text.index() else {
        return;
    };
    let delimiter_end = index.stop();
    let Some((content, span)) = inlines.iter_mut().rev().find_map(|inline| match inline {
        Inline::Text { content, span }
            if span.start >= base
                && span.end >= base
                && span.end > span.start
                && span.end - base == delimiter_end =>
        {
            Some((content, *span))
        }
        _ => None,
    }) else {
        return;
    };
    let local_start = span.start - base;
    let local_end = span.end - base;
    if source.get(local_start..local_end).is_none() {
        return;
    }
    let mut content_end = local_end;
    while content_end > local_start && source.as_bytes()[content_end - 1] == b' ' {
        content_end -= 1;
    }
    if content_end == local_end {
        return;
    }
    if let Some(raw_content) = source.get(local_start..content_end) {
        *content = normalize_text_content(raw_content);
    }
}

fn text_line_break(arena: &Arena, node: NodeRef, source: &str, base: usize) -> Option<Inline> {
    let KindData::Text(text) = arena[node].kind_data() else {
        return None;
    };
    let qualifier = if text.has_qualifiers(TextQualifier::HARD_LINE_BREAK) {
        Some(true)
    } else if text.has_qualifiers(TextQualifier::SOFT_LINE_BREAK) {
        Some(false)
    } else {
        None
    }?;
    let local = text
        .index()
        .and_then(|index| checked_index(*index, source))?;
    let newline_offset = source.get(local.end..)?.find('\n')?;
    let end = local.end.checked_add(newline_offset)?.checked_add(1)?;
    let span = offset_span(ByteSpan::new(local.end, end), base)?;
    if qualifier {
        Some(Inline::HardBreak { span })
    } else {
        Some(Inline::SoftBreak { span })
    }
}

/// Adapt Rushdown's parser-owned text into the semantic text that enters the
/// Scribium AST. The source span remains the original byte range. The
/// source-backed pass preserves escaped references while applying exactly one
/// reference normalization at the original source position.
fn normalize_text_content(raw: &str) -> String {
    normalize_source_value(raw, false)
}

#[derive(Debug, Clone, Copy)]
enum MetadataKind {
    LinkDestination,
    LinkTitle,
    CodeInfo,
}

#[derive(Debug, Clone, Copy)]
enum MetadataReferenceKind {
    Named,
    Numeric,
}

/// Normalize a source-backed semantic metadata value without changing its
/// source span. The three modes are explicit because their Markdown grammar
/// ownership differs, even though the pinned CommonMark/GFM behavior currently
/// gives them the same narrow escape/entity policy.
fn normalize_metadata(raw: &str, kind: MetadataKind) -> String {
    let escaped_space = match kind {
        MetadataKind::LinkDestination | MetadataKind::LinkTitle | MetadataKind::CodeInfo => false,
    };
    normalize_source_value(raw, escaped_space)
}

fn normalize_source_value(raw: &str, escaped_space: bool) -> String {
    let source = raw.as_bytes();
    let mut normalized = Vec::with_capacity(source.len());
    let mut cursor = 0;

    // Keep this as one pass over the original stream. In particular, an
    // escaped '&' is emitted as a literal byte and the following `amp;` or
    // numeric-looking suffix is then copied as ordinary text; it can never
    // become a reference because it was not reference syntax at its original
    // source position.
    while cursor < source.len() {
        if source[cursor] == b'\\' {
            if let Some(&next) = source.get(cursor + 1) {
                if rushdown::util::is_punct(next) {
                    normalized.push(next);
                    cursor += 2;
                    continue;
                }
                if escaped_space && next == b' ' {
                    cursor += 2;
                    continue;
                }
            }
            normalized.push(source[cursor]);
            cursor += 1;
            continue;
        }

        if source[cursor] == b'&' {
            if let Some((end, reference_kind)) = metadata_reference_end(source, cursor) {
                let reference = &source[cursor..end];
                let replacement = match reference_kind {
                    MetadataReferenceKind::Named => {
                        rushdown::util::resolve_entity_references(reference)
                    }
                    MetadataReferenceKind::Numeric => {
                        let replacement = rushdown::util::resolve_numeric_references(reference);
                        if replacement.as_ref() == [0] {
                            std::borrow::Cow::Borrowed("�".as_bytes())
                        } else {
                            replacement
                        }
                    }
                };
                normalized.extend_from_slice(replacement.as_ref());
                cursor = end;
                continue;
            }
        }

        normalized.push(source[cursor]);
        cursor += 1;
    }

    String::from_utf8_lossy(&normalized).into_owned()
}

fn metadata_reference_end(value: &[u8], start: usize) -> Option<(usize, MetadataReferenceKind)> {
    if value.get(start) != Some(&b'&') {
        return None;
    }
    let mut cursor = start.checked_add(1)?;
    match value.get(cursor).copied() {
        Some(b'#') => {
            cursor = cursor.checked_add(1)?;
            let hexadecimal = matches!(value.get(cursor), Some(b'x' | b'X'));
            if hexadecimal {
                cursor = cursor.checked_add(1)?;
            }
            let digit_start = cursor;
            while let Some(byte) = value.get(cursor) {
                let is_digit = if hexadecimal {
                    byte.is_ascii_hexdigit()
                } else {
                    byte.is_ascii_digit()
                };
                if !is_digit {
                    break;
                }
                cursor = cursor.checked_add(1)?;
            }
            (cursor > digit_start && value.get(cursor) == Some(&b';'))
                .then_some((cursor.checked_add(1)?, MetadataReferenceKind::Numeric))
        }
        Some(byte) if byte.is_ascii_alphanumeric() => {
            let name_start = cursor;
            while value
                .get(cursor)
                .is_some_and(|byte| byte.is_ascii_alphanumeric())
            {
                cursor = cursor.checked_add(1)?;
            }
            if cursor == name_start || value.get(cursor) != Some(&b';') {
                return None;
            }
            let name = std::str::from_utf8(value.get(name_start..cursor)?).ok()?;
            rushdown::util::look_up_html5_entity_by_name(name)?;
            Some((cursor.checked_add(1)?, MetadataReferenceKind::Named))
        }
        _ => None,
    }
}

fn convert_inline(
    arena: &Arena,
    node: NodeRef,
    source: &str,
    base: usize,
    diagnostics: &mut Vec<ParserDiagnostic>,
) -> Option<Inline> {
    let local_span = match arena[node].kind_data() {
        KindData::CodeSpan(_) => code_span_span(arena, node, source),
        KindData::RawHtml(html) => raw_html_span(arena, node, html, source),
        KindData::Link(link) => link_span(arena, node, link, source),
        KindData::Image(image) => image_span(arena, node, image, source),
        KindData::Strikethrough(_) => strikethrough_span(arena, node, source),
        _ => node_span(arena, node, source),
    }?;
    let span = offset_span(local_span, base)?;
    match arena[node].kind_data() {
        KindData::Text(text) => {
            let local = text
                .index()
                .and_then(|index| checked_index(*index, source))
                .or_else(|| node_span(arena, node, source))?;
            Some(Inline::Text {
                content: normalize_text_content(source.get(local.start..local.end)?),
                span: offset_span(local, base)?,
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
        KindData::CodeSpan(code) => Some(Inline::Code {
            content: code.str(source).into_owned(),
            span,
        }),
        KindData::Link(link) => Some(Inline::Link {
            content: {
                let content = convert_inlines(arena, node, source, base, diagnostics);
                if matches!(link.link_kind(), rushdown::ast::LinkKind::Auto(_)) {
                    auto_link_content(link, source, base).unwrap_or(content)
                } else {
                    content
                }
            },
            destination: convert_link_destination(link, source)?,
            title: link
                .title_str(source)
                .map(|title| normalize_metadata(title.as_ref(), MetadataKind::LinkTitle)),
            span,
        }),
        KindData::Image(image) => Some(Inline::Image {
            content: convert_inlines(arena, node, source, base, diagnostics),
            destination: checked_value(image.destination(), source)?.to_string(),
            title: image.title_str(source).map(|title| title.into_owned()),
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
            let parsed = if source.as_bytes().get(call_span.start) == Some(&b'{') {
                scribium_quarkdown::parse_tight_call(source, call_span.start)
            } else {
                scribium_quarkdown::parse_inline_call(source, call_span.start)
            };
            let call = match parsed {
                Ok(Some((call, _))) => call,
                Ok(None) => {
                    return Some(Inline::Unsupported {
                        kind: "malformed Quarkdown inline call".to_string(),
                        span,
                    })
                }
                Err(error) => {
                    diagnostics.push(ParserDiagnostic {
                        code: error.code,
                        message: error.message,
                        span: offset_span(error.span, base).unwrap_or(span),
                    });
                    return None;
                }
            };
            let call_name = call.name.clone();
            Some(Inline::DirectiveCall {
                name: call.name,
                name_span: offset_span(call.name_span, base).unwrap_or(ByteSpan::new(0, 0)),
                head_span: offset_span(call.head_span, base).unwrap_or(ByteSpan::new(0, 0)),
                positional_args: call
                    .positional_args
                    .iter()
                    .enumerate()
                    .map(|(index, arg)| {
                        convert_arg_with_mode(
                            arg,
                            source,
                            base,
                            0,
                            diagnostics,
                            is_contextual_inline_body_position(&call_name, index),
                            is_contextual_inline_body_position(&call_name, index),
                        )
                    })
                    .collect(),
                named_args: call
                    .named_args
                    .iter()
                    .map(|arg| {
                        convert_named_arg(
                            arg,
                            source,
                            base,
                            0,
                            diagnostics,
                            Some(call_name.as_str()),
                        )
                    })
                    .collect(),
                chain: call
                    .chain
                    .iter()
                    .map(|segment| convert_call_segment(segment, source, base, 0, diagnostics))
                    .collect(),
                body: None,
                span,
            })
        }
        _ => Some(Inline::Unsupported {
            kind: arena[node].kind_data().kind_name().to_string(),
            span,
        }),
    }
}

fn is_contextual_inline_body_position(call_name: &str, positional_index: usize) -> bool {
    positional_index == 1 && matches!(call_name, "foreach" | "repeat")
}

fn is_chained_contextual_inline_body_position(call_name: &str, positional_index: usize) -> bool {
    positional_index == 0 && matches!(call_name, "foreach" | "repeat")
}

fn convert_arg_with_mode(
    arg: &Arg,
    source: &str,
    base: usize,
    call_base: usize,
    diagnostics: &mut Vec<ParserDiagnostic>,
    allow_unmarked_lambda: bool,
    contextual_inline_body: bool,
) -> Value {
    match &arg.content {
        ArgContent::Scalar(value) => convert_value(value, arg.span, base, call_base, diagnostics),
        ArgContent::Content(content) => {
            let span = offset_span(*content, call_base);
            let Some(span) = span.and_then(|value| checked_local_span(value, source)) else {
                diagnostics.push(ParserDiagnostic {
                    code: "E9002",
                    message: "Quarkdown content argument is outside the source".to_string(),
                    span: span
                        .and_then(|value| offset_span(value, base))
                        .unwrap_or(ByteSpan::new(0, 0)),
                });
                return Value::String(String::new());
            };
            let parsed_lambda = if allow_unmarked_lambda {
                scribium_quarkdown::parse_callback_lambda(source, span)
            } else {
                scribium_quarkdown::parse_inline_lambda(source, span)
            };
            match parsed_lambda {
                Ok(Some(lambda)) => {
                    let parameters = (!lambda.implicit).then(|| crate::ast::LambdaHeader {
                        parameters: lambda
                            .parameters
                            .into_iter()
                            .map(|parameter| crate::ast::LambdaParameter {
                                name: parameter.name,
                                name_span: offset_span(parameter.name_span, base)
                                    .unwrap_or(parameter.name_span),
                                span: offset_span(parameter.span, base).unwrap_or(parameter.span),
                                optional: parameter.optional,
                            })
                            .collect(),
                        span: offset_span(
                            ByteSpan::new(lambda.span.start, lambda.body.start),
                            base,
                        )
                        .unwrap_or(ByteSpan::new(lambda.span.start, lambda.body.start)),
                    });
                    let content = parse_original_content(source, span, base, diagnostics);
                    let body = parse_original_content(source, lambda.body, base, diagnostics);
                    if contextual_inline_body {
                        Value::InlineBody {
                            content,
                            parameters,
                            body,
                            span: offset_span(lambda.span, base).unwrap_or(lambda.span),
                        }
                    } else {
                        Value::Lambda {
                            parameters,
                            body,
                            span: offset_span(lambda.span, base).unwrap_or(lambda.span),
                        }
                    }
                }
                Ok(None) => Value::Content(parse_original_content(source, span, base, diagnostics)),
                Err(error) => {
                    diagnostics.push(ParserDiagnostic {
                        code: error.code,
                        message: error.message,
                        span: offset_span(error.span, base).unwrap_or(error.span),
                    });
                    Value::Content(parse_original_content(source, span, base, diagnostics))
                }
            }
        }
    }
}

fn parse_original_content(
    source: &str,
    span: ByteSpan,
    base: usize,
    diagnostics: &mut Vec<ParserDiagnostic>,
) -> Vec<Inline> {
    let Some(_) = source.get(span.start..span.end) else {
        return Vec::new();
    };
    let mut inlines = Vec::new();
    let mut cursor = span.start;
    let mut text_start = cursor;
    let mut has_unsupported_markdown = false;
    while cursor < span.end {
        let byte = source.as_bytes()[cursor];
        if byte == b'`' {
            has_unsupported_markdown = true;
            let delimiter_len = source.as_bytes()[cursor..span.end]
                .iter()
                .take_while(|byte| **byte == b'`')
                .count();
            if delimiter_len == 0 {
                cursor += 1;
                continue;
            }
            cursor += delimiter_len;
            while cursor + delimiter_len <= span.end {
                if source.as_bytes()[cursor..span.end]
                    .starts_with(&source.as_bytes()[cursor - delimiter_len..cursor])
                {
                    cursor += delimiter_len;
                    break;
                }
                cursor += source[cursor..].chars().next().map_or(1, char::len_utf8);
            }
            continue;
        }
        if byte == b'.' {
            match scribium_quarkdown::parse_inline_call(source, cursor) {
                Ok(Some((call, end))) if end <= span.end => {
                    push_content_text(&mut inlines, source, text_start, cursor, base);
                    inlines.push(convert_content_call(call, source, base, diagnostics));
                    cursor = end;
                    text_start = cursor;
                    continue;
                }
                Err(error) => diagnostics.push(ParserDiagnostic {
                    code: error.code,
                    message: error.message,
                    span: offset_span(error.span, base).unwrap_or(ByteSpan::new(0, 0)),
                }),
                _ => {}
            }
        }
        // Angle-bracket text remains an exact source-backed String boundary;
        // it does not require the unavailable Quarkdown inline-fragment
        // parser. Keep E3010 for Markdown constructs whose structure would be
        // lost by preserving the original text.
        if matches!(byte, b'*' | b'_' | b'[' | b']' | b'~') {
            has_unsupported_markdown = true;
        }
        cursor += source[cursor..].chars().next().map_or(1, char::len_utf8);
    }
    push_content_text(&mut inlines, source, text_start, span.end, base);
    if has_unsupported_markdown {
        diagnostics.push(ParserDiagnostic {
            code: "E3010",
            message: "Markdown inline syntax in a Quarkdown content argument is preserved as original text but is not lowered because Rushdown exposes no original-span inline-fragment parser".to_string(),
            span: offset_span(span, base).unwrap_or(ByteSpan::new(0, 0)),
        });
    }
    inlines
}

fn push_content_text(
    inlines: &mut Vec<Inline>,
    source: &str,
    start: usize,
    end: usize,
    base: usize,
) {
    if start >= end {
        return;
    }
    if let Some(content) = source.get(start..end) {
        if let Some(span) = offset_span(ByteSpan::new(start, end), base) {
            inlines.push(Inline::Text {
                content: content.to_string(),
                span,
            });
        }
    }
}

fn convert_content_call(
    call: QuarkdownCall,
    source: &str,
    base: usize,
    diagnostics: &mut Vec<ParserDiagnostic>,
) -> Inline {
    let span = offset_span(call.span, base).unwrap_or(ByteSpan::new(0, 0));
    let call_name = call.name.clone();
    Inline::DirectiveCall {
        name: call.name,
        name_span: offset_span(call.name_span, base).unwrap_or(ByteSpan::new(0, 0)),
        head_span: offset_span(call.head_span, base).unwrap_or(ByteSpan::new(0, 0)),
        positional_args: call
            .positional_args
            .iter()
            .enumerate()
            .map(|(index, arg)| {
                convert_arg_with_mode(
                    arg,
                    source,
                    base,
                    0,
                    diagnostics,
                    is_contextual_inline_body_position(&call_name, index),
                    is_contextual_inline_body_position(&call_name, index),
                )
            })
            .collect(),
        named_args: call
            .named_args
            .iter()
            .map(|arg| {
                convert_named_arg(arg, source, base, 0, diagnostics, Some(call_name.as_str()))
            })
            .collect(),
        chain: call
            .chain
            .iter()
            .map(|segment| convert_call_segment(segment, source, base, 0, diagnostics))
            .collect(),
        body: None,
        span,
    }
}

fn convert_call_segment(
    segment: &scribium_quarkdown::CallSegment,
    source: &str,
    base: usize,
    call_base: usize,
    diagnostics: &mut Vec<ParserDiagnostic>,
) -> CallSegment {
    let call_name = segment.name.clone();
    CallSegment {
        name: segment.name.clone(),
        name_span: offset_span(
            segment.name_span,
            base.checked_add(call_base).unwrap_or(base),
        )
        .unwrap_or(ByteSpan::new(0, 0)),
        positional_args: segment
            .positional_args
            .iter()
            .enumerate()
            .map(|(index, arg)| {
                convert_arg_with_mode(
                    arg,
                    source,
                    base,
                    call_base,
                    diagnostics,
                    is_chained_contextual_inline_body_position(&call_name, index),
                    is_chained_contextual_inline_body_position(&call_name, index),
                )
            })
            .collect(),
        named_args: segment
            .named_args
            .iter()
            .map(|arg| {
                convert_named_arg(
                    arg,
                    source,
                    base,
                    call_base,
                    diagnostics,
                    Some(call_name.as_str()),
                )
            })
            .collect(),
        span: offset_span(segment.span, base.checked_add(call_base).unwrap_or(base))
            .unwrap_or(ByteSpan::new(0, 0)),
    }
}

fn convert_named_arg(
    arg: &scribium_quarkdown::NamedArg,
    source: &str,
    base: usize,
    call_base: usize,
    diagnostics: &mut Vec<ParserDiagnostic>,
    call_name: Option<&str>,
) -> NamedArg {
    let offset = base.checked_add(call_base).unwrap_or(base);
    let callback_lambda = arg.name == "by"
        && call_name.is_some_and(|name| matches!(name, "map" | "filter" | "sorted"));
    NamedArg {
        name: arg.name.clone(),
        name_span: offset_span(arg.name_span, offset).unwrap_or(ByteSpan::new(0, 0)),
        value: convert_arg_with_mode(
            &arg.value,
            source,
            base,
            call_base,
            diagnostics,
            callback_lambda,
            false,
        ),
        span: offset_span(arg.span, offset).unwrap_or(ByteSpan::new(0, 0)),
    }
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
        | Inline::Unsupported { span, .. }
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
        | Inline::Unsupported { span, .. }
        | Inline::HardBreak { span }
        | Inline::SoftBreak { span } => span.end,
    }
}

/// Converts a grammar value while applying the parser's existing coordinate
/// contract. Grammar values from block calls are relative to the reparsed
/// call substring (`call_base`), while inline/tight calls are already relative
/// to the body passed to the grammar (`call_base == 0`). The document body
/// base is applied exactly once in both cases.
fn convert_value(
    value: &QuarkdownValue,
    arg_span: ByteSpan,
    base: usize,
    call_base: usize,
    diagnostics: &mut Vec<ParserDiagnostic>,
) -> Value {
    match value {
        QuarkdownValue::String(value) => Value::String(value.clone()),
        QuarkdownValue::Number(value) => Value::Number(*value),
        QuarkdownValue::Boolean(value) => Value::Boolean(*value),
        QuarkdownValue::Identifier(value) => Value::Identifier(value.clone()),
        QuarkdownValue::Range(value) => {
            let Some(offset) = base.checked_add(call_base) else {
                diagnostics.push(ParserDiagnostic {
                    code: "E9002",
                    message: "Quarkdown Range span overflowed the document coordinate space"
                        .to_string(),
                    span: arg_span,
                });
                return Value::String(String::new());
            };
            let Some(span) = offset_span(value.span, offset) else {
                diagnostics.push(ParserDiagnostic {
                    code: "E9002",
                    message: "Quarkdown Range span overflowed the document coordinate space"
                        .to_string(),
                    span: arg_span,
                });
                return Value::String(String::new());
            };
            Value::Range(RangeValue {
                start: value.start,
                end: value.end,
                span,
            })
        }
    }
}

fn convert_link_destination(link: &rushdown::ast::Link, source: &str) -> Option<String> {
    let raw = checked_value(link.destination(), source)?;
    let destination = if matches!(link.link_kind(), rushdown::ast::LinkKind::Auto(_)) {
        raw.to_string()
    } else {
        normalize_metadata(raw, MetadataKind::LinkDestination)
    };
    Some(destination)
}

fn auto_link_content(link: &rushdown::ast::Link, source: &str, base: usize) -> Option<Vec<Inline>> {
    let rushdown::ast::LinkKind::Auto(auto) = link.link_kind() else {
        return None;
    };
    let rushdown::text::Value::Index(text) = auto.text() else {
        return None;
    };
    let text = checked_index(*text, source)?;
    let start = if source.as_bytes().get(text.start) == Some(&b'<') {
        text.start.checked_add(1)?
    } else {
        text.start
    };
    let end = if text.end > start && source.as_bytes().get(text.end - 1) == Some(&b'>') {
        text.end - 1
    } else {
        text.end
    };
    let span = ByteSpan::new(start, end);
    if !span.is_valid_for(source) {
        return None;
    }
    Some(vec![Inline::Text {
        content: source.get(start..end)?.to_string(),
        span: offset_span(span, base)?,
    }])
}

fn node_span(arena: &Arena, node: NodeRef, source: &str) -> Option<ByteSpan> {
    if let KindData::Text(text) = arena[node].kind_data() {
        if let Some(index) = text.index() {
            return checked_index(*index, source);
        }
    }
    if let KindData::HtmlBlock(html) = arena[node].kind_data() {
        return html_block_span(arena, node, html, source);
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
    if end.is_none() {
        if let (Some(start), TypeData::Block(_)) = (start, arena[node].type_data()) {
            return positioned_block_span(start, source);
        }
    }
    let span = ByteSpan::new(start?, end?);
    span.is_valid_for(source).then_some(span)
}

fn positioned_block_span(start: usize, source: &str) -> Option<ByteSpan> {
    let end = source
        .get(start..)?
        .find('\n')
        .map_or(source.len(), |offset| start + offset + 1);
    let span = ByteSpan::new(start, end);
    span.is_valid_for(source).then_some(span)
}

fn strikethrough_span(arena: &Arena, node: NodeRef, source: &str) -> Option<ByteSpan> {
    let span = node_span(arena, node, source)?;
    // Rushdown exposes the accepted node but not its delimiter width. Derive
    // that width only from the node's source opener, then match the same
    // complete run within the parser-owned parent boundary.
    let delimiter_width = tilde_run_width_at(source, span.start).unwrap_or(0);
    if delimiter_width == 0 {
        return Some(span);
    }
    let limit = arena[node]
        .parent()
        .and_then(|parent| node_span(arena, parent, source))
        .map_or(source.len(), |parent| parent.end);
    if has_tilde_run_ending_at(source, span.end, delimiter_width) {
        return Some(span);
    }
    let end = find_tilde_closer(source, span.end, limit, delimiter_width)
        .and_then(|end| end.checked_add(delimiter_width));
    let complete = end.map(|end| ByteSpan::new(span.start, end));
    // Keep an accepted node visible if a conservative span completion cannot
    // be proven. In particular, never consume an unrelated later delimiter.
    complete
        .filter(|candidate| candidate.is_valid_for(source))
        .or(Some(span))
}

fn tilde_run_width_at(source: &str, start: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    if bytes.get(start) != Some(&b'~')
        || start.checked_sub(1).and_then(|index| bytes.get(index)) == Some(&b'~')
    {
        return None;
    }
    let mut end = start;
    while bytes.get(end) == Some(&b'~') {
        end = end.checked_add(1)?;
    }
    end.checked_sub(start)
}

fn has_tilde_run_ending_at(source: &str, end: usize, width: usize) -> bool {
    let Some(start) = end.checked_sub(width) else {
        return false;
    };
    let bytes = source.as_bytes();
    bytes.get(start..end).is_some_and(|run| {
        run.iter().all(|byte| *byte == b'~')
            && (start == 0 || bytes.get(start - 1) != Some(&b'~'))
            && bytes.get(end) != Some(&b'~')
    })
}

fn find_tilde_closer(source: &str, start: usize, limit: usize, width: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let limit = limit.min(bytes.len());
    if start > limit {
        return None;
    }
    let mut cursor = start.min(limit);
    while cursor.checked_add(width)? <= limit {
        if tilde_run_width_at(source, cursor) == Some(width)
            && bytes
                .get(cursor..cursor.checked_add(width)?)?
                .iter()
                .all(|byte| *byte == b'~')
        {
            return Some(cursor);
        }
        cursor = cursor.checked_add(1)?;
    }
    None
}

fn link_span(
    arena: &Arena,
    node: NodeRef,
    link: &rushdown::ast::Link,
    source: &str,
) -> Option<ByteSpan> {
    let span = match link.link_kind() {
        rushdown::ast::LinkKind::Auto(auto) => match auto.text() {
            rushdown::text::Value::Index(text) => {
                checked_index(*text, source).or_else(|| node_span(arena, node, source))
            }
            _ => node_span(arena, node, source),
        },
        _ => node_span(arena, node, source).or_else(|| {
            (arena[node].children(arena).next().is_none())
                .then(|| empty_label_base_span(arena[node].pos(), source))
                .flatten()
        }),
    }?;
    if matches!(link.link_kind(), rushdown::ast::LinkKind::Auto(_)) {
        let end = if source.as_bytes().get(span.start) == Some(&b'<')
            && source.as_bytes().get(span.end) == Some(&b'>')
        {
            span.end.checked_add(1)?
        } else {
            span.end
        };
        let span = ByteSpan::new(span.start, end);
        return span.is_valid_for(source).then_some(span);
    }
    if !matches!(link.link_kind(), rushdown::ast::LinkKind::Inline) {
        return Some(span);
    }

    complete_inline_destination_span(span, link.destination(), link.title().is_some(), source)
}

fn complete_inline_destination_span(
    span: ByteSpan,
    destination: &rushdown::text::Value,
    has_title: bool,
    source: &str,
) -> Option<ByteSpan> {
    let bytes = source.as_bytes();
    let mut cursor = match destination {
        rushdown::text::Value::Index(destination) => {
            let destination = checked_index(*destination, source)?;
            destination.end
        }
        rushdown::text::Value::String(destination) if destination.is_empty() => {
            let Some(cursor) = empty_inline_destination_cursor(span, bytes) else {
                return Some(span);
            };
            cursor
        }
        _ => return Some(span),
    };

    if bytes.get(cursor) == Some(&b'>') {
        cursor += 1;
    }
    cursor = skip_link_spaces(bytes, cursor);

    if has_title {
        let opener = *bytes.get(cursor)?;
        let closer = if opener == b'(' { b')' } else { opener };
        cursor += 1;
        let mut closed = false;
        while cursor < bytes.len() {
            if bytes[cursor] == b'\\' {
                cursor = cursor.saturating_add(2);
                continue;
            }
            if bytes[cursor] == closer {
                cursor += 1;
                closed = true;
                break;
            }
            cursor += 1;
        }
        if !closed {
            return Some(span);
        }
        cursor = skip_link_spaces(bytes, cursor);
    }

    if bytes.get(cursor) != Some(&b')') {
        return Some(span);
    }
    let span = ByteSpan::new(span.start, cursor + 1);
    span.is_valid_for(source).then_some(span)
}

fn empty_label_base_span(start: Option<usize>, source: &str) -> Option<ByteSpan> {
    let start = start?;
    let end = start.checked_add(2)?;
    (source.as_bytes().get(start..end) == Some(b"[]"))
        .then(|| ByteSpan::new(start, start + 1))
        .filter(|span| span.is_valid_for(source))
}

fn empty_label_destination_span(
    start: usize,
    prefix_len: usize,
    destination: &rushdown::text::Value,
    has_title: bool,
    source: &str,
) -> Option<ByteSpan> {
    let bytes = source.as_bytes();
    let open_paren = start.checked_add(prefix_len)?.checked_add(2)?;
    if bytes.get(open_paren) != Some(&b'(') {
        return None;
    }
    let mut cursor = open_paren.checked_add(1)?;
    cursor = match destination {
        rushdown::text::Value::Index(destination) => destination.stop(),
        rushdown::text::Value::String(value) if value.is_empty() => cursor,
        _ => return None,
    };
    if bytes.get(cursor) == Some(&b'>') {
        cursor += 1;
    }
    cursor = skip_link_spaces(bytes, cursor);

    if has_title {
        let opener = *bytes.get(cursor)?;
        let closer = if opener == b'(' { b')' } else { opener };
        cursor += 1;
        let mut closed = false;
        while cursor < bytes.len() {
            if bytes[cursor] == b'\\' {
                cursor = cursor.saturating_add(2);
                continue;
            }
            if bytes[cursor] == closer {
                cursor += 1;
                closed = true;
                break;
            }
            cursor += 1;
        }
        if !closed {
            return None;
        }
        cursor = skip_link_spaces(bytes, cursor);
    }

    (bytes.get(cursor) == Some(&b')'))
        .then(|| ByteSpan::new(start, cursor + 1))
        .filter(|span| span.is_valid_for(source))
}

fn image_span(
    arena: &Arena,
    node: NodeRef,
    image: &rushdown::ast::Image,
    source: &str,
) -> Option<ByteSpan> {
    if let Some(span) = node_span(arena, node, source) {
        match image.link_kind() {
            rushdown::ast::LinkKind::Inline => {
                return complete_inline_destination_span(
                    span,
                    image.destination(),
                    image.title().is_some(),
                    source,
                );
            }
            rushdown::ast::LinkKind::Reference(_) => {
                return complete_reference_image_span(span, source);
            }
            _ => return Some(span),
        }
    }
    if arena[node].children(arena).next().is_some() {
        return None;
    }
    let start = arena[node].pos()?;
    let end = start.checked_add(3)?;
    if source.as_bytes().get(start..end) != Some(b"![]") {
        return None;
    }
    empty_label_destination_span(
        start,
        1,
        image.destination(),
        image.title().is_some(),
        source,
    )
}

fn complete_reference_image_span(span: ByteSpan, source: &str) -> Option<ByteSpan> {
    let bytes = source.as_bytes();
    let mut cursor = span.end;
    if bytes.get(cursor) != Some(&b']') {
        return Some(span);
    }
    cursor += 1;
    if bytes.get(cursor) != Some(&b'[') {
        return ByteSpan::new(span.start, cursor)
            .is_valid_for(source)
            .then_some(ByteSpan::new(span.start, cursor));
    }
    cursor += 1;
    while cursor < bytes.len() {
        if bytes[cursor] == b'\\' {
            cursor = cursor.saturating_add(2);
            continue;
        }
        if bytes[cursor] == b']' {
            cursor += 1;
            let span = ByteSpan::new(span.start, cursor);
            return span.is_valid_for(source).then_some(span);
        }
        cursor += 1;
    }
    Some(span)
}

fn empty_inline_destination_cursor(span: ByteSpan, bytes: &[u8]) -> Option<usize> {
    let mut cursor = span.end;
    if bytes.get(cursor) != Some(&b']') {
        return None;
    }
    cursor += 1;
    if bytes.get(cursor) != Some(&b'(') {
        return None;
    }
    cursor += 1;
    Some(skip_link_spaces(bytes, cursor))
}

fn duplicate_autolink_closer(
    arena: &Arena,
    previous: Option<NodeRef>,
    current: NodeRef,
    source: &str,
) -> bool {
    let Some(previous) = previous else {
        return false;
    };
    let KindData::Link(link) = arena[previous].kind_data() else {
        return false;
    };
    if !matches!(link.link_kind(), rushdown::ast::LinkKind::Auto(_)) {
        return false;
    }
    let KindData::Text(text) = arena[current].kind_data() else {
        return false;
    };
    let Some(text) = text.index().and_then(|index| checked_index(*index, source)) else {
        return false;
    };
    if source.get(text.start..text.end) != Some(">") {
        return false;
    }
    let Some(link_span) = link_span(arena, previous, link, source) else {
        return false;
    };
    text.start.checked_add(1) == Some(link_span.end)
}

fn skip_link_spaces(bytes: &[u8], mut cursor: usize) -> usize {
    while bytes
        .get(cursor)
        .is_some_and(|byte| matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c))
    {
        cursor += 1;
    }
    cursor
}

fn raw_html_span(
    arena: &Arena,
    node: NodeRef,
    html: &rushdown::ast::RawHtml,
    source: &str,
) -> Option<ByteSpan> {
    let parent = arena[node].parent()?;
    let parent_span = node_span(arena, parent, source)?;
    let search_start = arena[node]
        .previous_sibling()
        .and_then(|previous| node_span(arena, previous, source))
        .map_or(parent_span.start, |span| span.end);
    let raw = html.bytes(source);
    let raw = raw.as_ref();
    let haystack = source.get(search_start..parent_span.end)?.as_bytes();
    let offset = haystack
        .windows(raw.len())
        .position(|window| window == raw)?;
    let span = ByteSpan::new(search_start + offset, search_start + offset + raw.len());
    span.is_valid_for(source).then_some(span)
}

fn code_block_span(
    arena: &Arena,
    node: NodeRef,
    code: &rushdown::ast::CodeBlock,
    source: &str,
) -> Option<ByteSpan> {
    let start = arena[node].pos()?;
    let body_end = match code.value() {
        Lines::Segments(segments) => segments.iter().map(Segment::stop).max(),
        _ => None,
    };
    let end = if code.code_block_kind() == CodeBlockKind::Fenced {
        let limit = next_node_boundary(arena, node).unwrap_or(source.len());
        fenced_code_end(source, start, limit)
            .or(body_end)
            .or_else(|| fenced_opening_end(source, start))
    } else {
        body_end
    }?;
    let span = ByteSpan::new(start, end);
    span.is_valid_for(source).then_some(span)
}

fn next_node_boundary(arena: &Arena, node: NodeRef) -> Option<usize> {
    let mut current = node;
    loop {
        if let Some(next) = arena[current].next_sibling() {
            return arena[next].pos();
        }
        current = arena[current].parent()?;
    }
}

fn html_block_span(
    arena: &Arena,
    node: NodeRef,
    html: &rushdown::ast::HtmlBlock,
    source: &str,
) -> Option<ByteSpan> {
    let span = match html.value() {
        Lines::Segments(segments) => {
            let mut start: Option<usize> = None;
            let mut end: Option<usize> = None;
            for segment in segments {
                let segment = checked_segment(*segment, source)?;
                start = Some(start.map_or(segment.start, |value| value.min(segment.start)));
                end = Some(end.map_or(segment.end, |value| value.max(segment.end)));
            }
            ByteSpan::new(start?, end?)
        }
        Lines::String(raw) => {
            let start = arena[node].pos()?;
            let end = start.checked_add(raw.len())?;
            if source.get(start..end) != Some(raw.as_str()) {
                return None;
            }
            ByteSpan::new(start, end)
        }
        _ => return None,
    };
    span.is_valid_for(source).then_some(span)
}

fn fenced_code_end(source: &str, start: usize, limit: usize) -> Option<usize> {
    let first_end = source
        .get(start..)?
        .find('\n')
        .map_or(source.len(), |offset| start + offset + 1);
    if first_end > limit {
        return None;
    }
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
    while line_start < limit {
        let line_end = source
            .get(line_start..)?
            .find('\n')
            .map_or(source.len(), |offset| line_start + offset + 1);
        if line_end > limit {
            return None;
        }
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

fn fenced_opening_end(source: &str, start: usize) -> Option<usize> {
    let line_end = source
        .get(start..)?
        .find('\n')
        .map_or(source.len(), |offset| start + offset + 1);
    source.get(start..line_end)?;
    Some(line_end)
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

fn offset_span(span: ByteSpan, offset: usize) -> Option<ByteSpan> {
    Some(ByteSpan::new(
        span.start.checked_add(offset)?,
        span.end.checked_add(offset)?,
    ))
}

fn code_info(code: &rushdown::ast::CodeBlock, source: &str) -> Option<String> {
    checked_value(code.info()?, source).map(ToOwned::to_owned)
}

fn code_block_source(code: &rushdown::ast::CodeBlock, source: &str) -> Option<String> {
    let Lines::Segments(segments) = code.value() else {
        return None;
    };
    segments
        .iter()
        .map(|segment| {
            checked_segment(*segment, source)?;
            String::from_utf8(segment.bytes(source).into_owned()).ok()
        })
        .collect::<Option<Vec<_>>>()
        .map(|lines| lines.concat())
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

fn code_span_span(arena: &Arena, node: NodeRef, source: &str) -> Option<ByteSpan> {
    let start = arena[node].pos()?;
    let bytes = source.as_bytes();
    let delimiter_len = backtick_run_len(bytes, start, bytes.len());
    if delimiter_len == 0 {
        return None;
    }
    let closing_start = find_exact_backtick_run(
        bytes,
        start.checked_add(delimiter_len)?,
        bytes.len(),
        delimiter_len,
    )?;
    let end = closing_start.checked_add(delimiter_len)?;
    let span = ByteSpan::new(start, end);
    span.is_valid_for(source).then_some(span)
}

fn backtick_run_len(source: &[u8], start: usize, limit: usize) -> usize {
    let Some(run) = source.get(start..limit.min(source.len())) else {
        return 0;
    };
    run.iter().take_while(|byte| **byte == b'`').count()
}

fn find_exact_backtick_run(
    source: &[u8],
    from: usize,
    to: usize,
    expected_len: usize,
) -> Option<usize> {
    if expected_len == 0 {
        return None;
    }

    let limit = to.min(source.len());
    let mut cursor = from.min(limit);
    while cursor < limit {
        if source[cursor] != b'`' {
            cursor += 1;
            continue;
        }

        let run_start = cursor;
        let run_len = backtick_run_len(source, run_start, limit);
        if run_len == expected_len {
            return Some(run_start);
        }
        cursor = run_start + run_len;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::TableAlignment;

    fn assert_malformed_argument_span(source: &str) {
        let output = parse_with_diagnostics(source);
        let diagnostic = output
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "E2003")
            .unwrap_or_else(|| panic!("expected E2003 diagnostic, got {:?}", output.diagnostics));
        let expected_start = source.find('{').expect("malformed argument opening");
        assert_eq!(diagnostic.span, ByteSpan::new(expected_start, source.len()));
        assert!(diagnostic.span.start <= diagnostic.span.end);
        assert!(diagnostic.span.end <= source.len());
        assert!(source.is_char_boundary(diagnostic.span.start));
        assert!(source.is_char_boundary(diagnostic.span.end));
        assert!(source
            .get(diagnostic.span.start..diagnostic.span.end)
            .is_some());
        assert_eq!(
            source.get(diagnostic.span.start..diagnostic.span.end),
            Some("{unterminated")
        );
    }

    #[test]
    fn markdown_mode_keeps_quarkdown_as_text() {
        let document = parse_md(".foo {bar}\n");
        assert!(matches!(
            document.nodes.first(),
            Some(Block::Paragraph { .. })
        ));
    }

    #[test]
    fn qd_multiline_arguments_and_continuations_keep_header_body_boundary() {
        let source = ".divide {\n  .cos {.pi}\n} by:{\n  .sum {2} {1}\n}\n  body\n";
        let output = parse_with_diagnostics(source);
        assert!(output.diagnostics.is_empty(), "{output:?}");
        let Block::DirectiveCall {
            name,
            positional_args,
            named_args,
            body,
            span,
            ..
        } = &output.document.nodes[0]
        else {
            panic!(
                "expected multiline Quarkdown call, got {:?}",
                output.document.nodes
            )
        };
        assert_eq!(name, "divide");
        assert_eq!(positional_args.len(), 1);
        assert_eq!(named_args.len(), 1);
        assert_eq!(&source[span.start..span.end], source.trim_end());
        let body = body.as_ref().expect("body after multiline header");
        assert_eq!(paragraph_text(&body[0]), "body");

        let continuation = concat!(
            ".container alignment:{center} \\",
            "\n  background:{red} \\",
            "\n  padding:{1px}\n"
        );
        let output = parse_with_diagnostics(continuation);
        assert!(output.diagnostics.is_empty(), "{output:?}");
        let Block::DirectiveCall {
            named_args, span, ..
        } = &output.document.nodes[0]
        else {
            panic!("expected continued call")
        };
        assert_eq!(named_args.len(), 3);
        assert_eq!(&continuation[span.start..span.end], continuation.trim_end());
    }

    #[test]
    fn qd_inline_continuation_and_tight_calls_preserve_text_and_spans() {
        let source = concat!(
            "Before .call {a} \\",
            "\n  second:{b} after H{.text {2}}O\n"
        );
        let output = parse_with_diagnostics(source);
        assert!(output.diagnostics.is_empty(), "{output:?}");
        let Block::Paragraph { content, .. } = &output.document.nodes[0] else {
            panic!("expected paragraph")
        };
        assert!(content.iter().any(|inline| matches!(
            inline,
            Inline::DirectiveCall { span, .. }
                if &source[span.start..span.end] == concat!(".call {a} \\", "\n  second:{b}")
        )));
        let tight = content
            .iter()
            .find_map(|inline| match inline {
                Inline::DirectiveCall { span, .. }
                    if &source[span.start..span.end] == "{.text {2}}" =>
                {
                    Some(span)
                }
                _ => None,
            })
            .expect("tight call");
        assert!(source.is_char_boundary(tight.start));
        assert!(source.is_char_boundary(tight.end));
        assert!(content.iter().any(|inline| matches!(
            inline,
            Inline::Text { content, .. } if content == "Before "
        )));
        assert!(content.iter().any(|inline| matches!(
            inline,
            Inline::Text { content, .. } if content == "O"
        )));

        let chain_source = "prefix .a {x}::b {y} suffix\n";
        let chain = parse_with_diagnostics(chain_source);
        assert!(chain.diagnostics.is_empty(), "{chain:?}");
        let Block::Paragraph { content, .. } = &chain.document.nodes[0] else {
            panic!("expected chain paragraph")
        };
        let Inline::DirectiveCall {
            name,
            name_span,
            head_span,
            chain: segments,
            span,
            ..
        } = content
            .iter()
            .find(|inline| matches!(inline, Inline::DirectiveCall { .. }))
            .expect("chain call")
        else {
            unreachable!()
        };
        assert_eq!(name, "a");
        assert_eq!(&chain_source[name_span.start..name_span.end], ".a");
        assert_eq!(&chain_source[head_span.start..head_span.end], ".a {x}");
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].name, "b");
        assert_eq!(
            &chain_source[segments[0].span.start..segments[0].span.end],
            "b {y}"
        );
        assert_eq!(
            &chain_source[segments[0].name_span.start..segments[0].name_span.end],
            "b"
        );
        assert_eq!(&chain_source[span.start..span.end], ".a {x}::b {y}");

        let with_front_matter = "---\ntitle: spans\n---\n.a::b\n";
        let front_matter_output = parse_with_diagnostics(with_front_matter);
        assert!(front_matter_output.diagnostics.is_empty());
        let Block::DirectiveCall {
            name_span,
            chain: segments,
            span,
            ..
        } = &front_matter_output.document.nodes[0]
        else {
            panic!("expected front-matter call")
        };
        assert_eq!(&with_front_matter[span.start..span.end], ".a::b");
        assert_eq!(&with_front_matter[name_span.start..name_span.end], ".a");
        assert_eq!(
            &with_front_matter[segments[0].name_span.start..segments[0].name_span.end],
            "b"
        );

        let markdown = parse_md("H{.text {2}}O\n");
        assert!(!markdown.nodes.iter().any(|block| match block {
            Block::Paragraph { content, .. } => content
                .iter()
                .any(|inline| matches!(inline, Inline::DirectiveCall { .. })),
            _ => false,
        }));
        let markdown = parse_md(".a::b\n{.text}\n");
        assert!(!markdown.nodes.iter().any(|block| match block {
            Block::Paragraph { content, .. } => content
                .iter()
                .any(|inline| matches!(inline, Inline::DirectiveCall { .. })),
            _ => matches!(block, Block::DirectiveCall { .. }),
        }));

        let crlf = ".call {\r\n  한글\r\n}\r\n";
        let crlf_output = parse_with_diagnostics(crlf);
        assert!(crlf_output.diagnostics.is_empty(), "{crlf_output:?}");
        let Block::DirectiveCall {
            positional_args,
            span,
            name_span,
            ..
        } = &crlf_output.document.nodes[0]
        else {
            panic!("expected CRLF multiline call")
        };
        assert_eq!(&crlf[span.start..span.end], crlf.trim_end());
        assert_eq!(&crlf[name_span.start..name_span.end], ".call");
        assert!(crlf.is_char_boundary(span.start));
        assert!(crlf.is_char_boundary(span.end));
        let Value::Content(content) = &positional_args[0] else {
            panic!("expected CRLF content argument")
        };
        assert_eq!(
            content[0],
            Inline::Text {
                content: "\r\n  한글\r\n".to_string(),
                span: ByteSpan::new(7, 19),
            }
        );

        let malformed = concat!(".call {a} \\", "\n\nfollowing\n");
        let malformed_output = parse_with_diagnostics(malformed);
        assert!(malformed_output.document.nodes.iter().any(|block| {
            matches!(block, Block::Paragraph { .. } if paragraph_text(block) == "following")
        }));
    }

    #[test]
    fn range_value_spans_are_document_absolute_in_block_inline_and_tight_calls() {
        fn assert_block_range(source: &str, node: &Block) {
            let Block::DirectiveCall {
                positional_args, ..
            } = node
            else {
                panic!("expected block call, got {node:?}")
            };
            let Value::Range(range) = &positional_args[0] else {
                panic!("expected typed Range argument")
            };
            assert_eq!(&source[range.span.start..range.span.end], "2..4");
        }

        for source in [
            "앞 문장\r\n.foreach {2..4}\r\n    .1\r\n",
            "---\r\ntitle: 값\r\n---\r\n\r\n앞 문장\r\n.foreach {2..4}\r\n    .1\r\n",
        ] {
            let output = parse_with_diagnostics(source);
            assert!(output.diagnostics.is_empty(), "{output:?}");
            let block = output
                .document
                .nodes
                .iter()
                .find(|node| matches!(node, Block::DirectiveCall { name, .. } if name == "foreach"))
                .expect("foreach block");
            assert_block_range(source, block);
        }

        for source in ["앞 .foo {2..4} 뒤\n", "앞 H{.foo {2..4}}O\n"] {
            let output = parse_with_diagnostics(source);
            assert!(output.diagnostics.is_empty(), "{output:?}");
            let Block::Paragraph { content, .. } = &output.document.nodes[0] else {
                panic!("expected paragraph")
            };
            let call = content
                .iter()
                .find_map(|inline| match inline {
                    Inline::DirectiveCall {
                        positional_args, ..
                    } => Some(positional_args),
                    _ => None,
                })
                .expect("inline or tight call");
            let Value::Range(range) = &call[0] else {
                panic!("expected typed Range argument")
            };
            assert_eq!(&source[range.span.start..range.span.end], "2..4");
        }
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
    fn qd_mode_parses_root_and_inline_calls_with_crlf_provenance() {
        let root_source = ".note {hello}\r\n";
        let root = parse_with_diagnostics(root_source);
        assert!(root.diagnostics.is_empty(), "{:?}", root.diagnostics);
        let Block::DirectiveCall { span, .. } = &root.document.nodes[0] else {
            panic!("expected root directive call")
        };
        assert_eq!(&root_source[span.start..span.end], ".note {hello}");

        let inline = parse_with_diagnostics("before .note {x} after\n");
        assert!(inline.diagnostics.is_empty(), "{:?}", inline.diagnostics);
        let Block::Paragraph { content, .. } = &inline.document.nodes[0] else {
            panic!("expected inline paragraph")
        };
        assert!(content
            .iter()
            .any(|item| matches!(item, Inline::DirectiveCall { name, .. } if name == "note")));
    }

    #[test]
    fn malformed_root_block_reports_argument_span() {
        assert_malformed_argument_span(".foo {unterminated");
    }

    #[test]
    fn malformed_block_restores_preceding_content_offset() {
        assert_malformed_argument_span("# heading\n.foo {unterminated");
    }

    #[test]
    fn malformed_named_argument_restores_preceding_content_offset() {
        let source = "# heading\n.foo name:{unterminated";
        let output = parse_with_diagnostics(source);
        let diagnostic = output
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "E2003")
            .expect("expected malformed named argument diagnostic");
        let expected_start = source.find('{').unwrap();
        assert_eq!(diagnostic.span, ByteSpan::new(expected_start, source.len()));
        assert!(source
            .get(diagnostic.span.start..diagnostic.span.end)
            .is_some());
    }

    #[test]
    fn malformed_block_restores_front_matter_and_body_offsets() {
        assert_malformed_argument_span("---\ntitle: test\n---\n# heading\n.foo {unterminated");
    }

    #[test]
    fn malformed_block_restores_front_matter_offset() {
        assert_malformed_argument_span("---\ntitle: test\n---\n.foo {unterminated");
    }

    #[test]
    fn malformed_inline_call_preserves_full_source_offset() {
        let source = "prefix .foo {unterminated";
        let output = parse_with_diagnostics(source);
        let diagnostic = output
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "E2003")
            .expect("expected malformed inline call diagnostic");
        let expected_start = source.find('{').unwrap();
        assert_eq!(diagnostic.span, ByteSpan::new(expected_start, source.len()));
        assert!(diagnostic.span.start <= diagnostic.span.end);
        assert!(diagnostic.span.end <= source.len());
        assert!(source.is_char_boundary(diagnostic.span.start));
        assert!(source.is_char_boundary(diagnostic.span.end));
        assert_eq!(
            source.get(diagnostic.span.start..diagnostic.span.end),
            Some("{unterminated")
        );
    }

    #[test]
    fn malformed_nested_block_restores_container_offset() {
        assert_malformed_argument_span("> .foo {unterminated");
    }

    #[test]
    fn malformed_utf8_block_span_is_source_backed() {
        assert_malformed_argument_span("# 한글 제목\n.foo {unterminated");
    }

    #[test]
    fn malformed_crlf_block_span_is_source_backed() {
        assert_malformed_argument_span("# heading\r\n.foo {unterminated");
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

    #[test]
    fn content_argument_preserves_original_span_and_reports_markdown_gap() {
        let source = ".text {**한글**}\n";
        let content_start = source.find("**").unwrap();
        let content_end = content_start + "**한글**".len();
        let output = parse_with_diagnostics(source);
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "E3010"
                && diagnostic.span == ByteSpan::new(content_start, content_end)
                && diagnostic.message.contains("original text")
        }));
        let Block::DirectiveCall {
            positional_args, ..
        } = &output.document.nodes[0]
        else {
            panic!("expected directive block")
        };
        let Value::Content(content) = &positional_args[0] else {
            panic!("expected content argument")
        };
        assert!(matches!(content.as_slice(), [Inline::Text { .. }]));
        let Inline::Text { span, content } = &content[0] else {
            unreachable!()
        };
        assert_eq!(*span, ByteSpan::new(content_start, content_end));
        assert_eq!(content, "**한글**");
        assert_eq!(&source[span.start..span.end], content);
    }

    #[test]
    fn nested_content_calls_keep_prefix_suffix_and_original_spans() {
        let source = ".panel {prefix .text {red} suffix}\n";
        let output = parse_with_diagnostics(source);
        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let Block::DirectiveCall {
            positional_args, ..
        } = &output.document.nodes[0]
        else {
            panic!("expected directive block")
        };
        let Value::Content(content) = &positional_args[0] else {
            panic!("expected content argument")
        };
        assert!(matches!(
            content.as_slice(),
            [
                Inline::Text { .. },
                Inline::DirectiveCall { .. },
                Inline::Text { .. }
            ]
        ));
        let Inline::Text {
            content: prefix,
            span: prefix_span,
        } = &content[0]
        else {
            unreachable!()
        };
        let Inline::DirectiveCall {
            name,
            span: nested_span,
            ..
        } = &content[1]
        else {
            unreachable!()
        };
        let Inline::Text {
            content: suffix,
            span: suffix_span,
        } = &content[2]
        else {
            unreachable!()
        };
        assert_eq!(prefix, "prefix ");
        assert_eq!(suffix, " suffix");
        assert_eq!(name, "text");
        assert_eq!(&source[prefix_span.start..prefix_span.end], prefix);
        assert_eq!(&source[nested_span.start..nested_span.end], ".text {red}");
        assert_eq!(&source[suffix_span.start..suffix_span.end], suffix);
    }

    #[test]
    fn gfm_table_is_preserved_as_frontend_table() {
        let document = parse_md("| A | B |\n|---|---|\n| 1 | 2 |\n");
        let Block::Table { header, rows, .. } = &document.nodes[0] else {
            panic!("expected table, got {:?}", document.nodes)
        };
        assert_eq!(header.cells.len(), 2);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].cells.len(), 2);
    }

    #[test]
    fn task_list_status_is_preserved_in_frontend_ast() {
        let document = parse_md("- [ ] todo\n- [x] done\n");
        let Block::UnorderedList { items, .. } = &document.nodes[0] else {
            panic!("expected list")
        };
        assert_eq!(items[0].task, Some(crate::ast::TaskStatus::Active));
        assert_eq!(items[1].task, Some(crate::ast::TaskStatus::Completed));
    }

    #[test]
    fn image_strikethrough_and_html_keep_distinct_frontend_nodes() {
        let document = parse_md(
            "[link](page.md) ![image](image.png) ~~strike **strong** `code`~~ <em>x</em>\n",
        );
        let Block::Paragraph { content, .. } = &document.nodes[0] else {
            panic!("expected paragraph")
        };
        assert!(content
            .iter()
            .any(|inline| matches!(inline, Inline::Link { .. })));
        assert!(content
            .iter()
            .any(|inline| matches!(inline, Inline::Image { .. })));
        assert!(content
            .iter()
            .any(|inline| matches!(inline, Inline::Strikethrough { .. })));
        assert!(
            content
                .iter()
                .any(|inline| matches!(inline, Inline::RawHtml { .. })),
            "{content:?}"
        );
    }

    #[test]
    fn commonmark_semantics_preserve_breaks_entities_titles_and_code_info() {
        let source = "# ATX\n\nSetext\n=======\n\nText &amp; &#x41; \\*escaped\\*\nsoft\nhard  \nnext\n\n[link](https://example.test \"title\") ![alt](image.png \"image title\")\n\n```rust extra-info\nfn main() {}\n```\n";
        let document = parse_md(source);
        assert!(matches!(document.nodes[0], Block::Heading { level: 1, .. }));
        assert!(matches!(document.nodes[1], Block::Heading { level: 1, .. }));

        let Block::Paragraph { content, .. } = &document.nodes[2] else {
            panic!("expected paragraph")
        };
        assert!(
            content.iter().any(|inline| matches!(
                inline,
                Inline::Text { content, .. } if content == "Text & A"
            )),
            "{content:?}"
        );
        assert!(
            content.iter().any(|inline| matches!(
                inline,
                Inline::Text { content, .. } if content == " *escaped*"
            )),
            "{content:?}"
        );
        assert!(content
            .iter()
            .any(|inline| matches!(inline, Inline::SoftBreak { .. })));
        assert!(content
            .iter()
            .any(|inline| matches!(inline, Inline::HardBreak { .. })));

        let Block::Paragraph { content, .. } = &document.nodes[3] else {
            panic!("expected link paragraph")
        };
        assert!(content.iter().any(|inline| matches!(
            inline,
            Inline::Link { title: Some(title), .. } if title == "title"
        )));
        assert!(content.iter().any(|inline| matches!(
            inline,
            Inline::Image { title: Some(title), .. } if title == "image title"
        )));

        let Block::CodeBlock { language, info, .. } = &document.nodes[4] else {
            panic!("expected fenced code block")
        };
        assert_eq!(language.as_deref(), Some("rust"));
        assert_eq!(info.as_deref(), Some("rust extra-info"));
    }

    #[test]
    fn utf8_crlf_breaks_keep_semantic_nodes_and_original_byte_spans() {
        let source = "한글\r\n다음  \r\n끝";
        let document = parse_md(source);
        let Block::Paragraph { content, .. } = &document.nodes[0] else {
            panic!("expected paragraph")
        };
        match content.as_slice() {
            [Inline::Text {
                content: first,
                span: first_span,
            }, Inline::SoftBreak { span: soft_span }, Inline::Text {
                content: second,
                span: second_span,
            }, Inline::HardBreak { span: hard_span }, Inline::Text {
                content: third,
                span: third_span,
            }] => {
                assert_eq!(first, "한글");
                assert_eq!(second, "다음");
                assert_eq!(third, "끝");
                assert_eq!(*first_span, ByteSpan::new(0, 6));
                assert_eq!(*soft_span, ByteSpan::new(6, 8));
                assert_eq!(*second_span, ByteSpan::new(8, 14));
                assert_eq!(*hard_span, ByteSpan::new(14, 18));
                assert_eq!(*third_span, ByteSpan::new(18, 21));
                assert_eq!(&source[soft_span.start..soft_span.end], "\r\n");
                assert_eq!(&source[hard_span.start..hard_span.end], "  \r\n");
            }
            other => panic!("unexpected inline structure: {other:?}"),
        }
    }

    #[test]
    fn blockquote_preserves_all_child_blocks() {
        let document = parse_md("> first\n>\n> second\n>\n> third\n");
        let Block::Blockquote { content, .. } = &document.nodes[0] else {
            panic!("expected blockquote, got {:?}", document.nodes)
        };
        assert_eq!(content.len(), 3);
        for (block, expected) in content.iter().zip(["first", "second", "third"]) {
            let Block::Paragraph { content, .. } = block else {
                panic!("expected paragraph")
            };
            let Inline::Text { content, .. } = &content[0] else {
                panic!("expected text")
            };
            assert_eq!(content, expected);
        }
    }

    #[test]
    fn preserved_markdown_structures_keep_nested_semantics_and_source_spans() {
        let source = "> quoted **strong**\n>\n> - [ ] active\n> - [x] done\n\nBefore ~~removed *content*~~ after ~~later~~\n\n| Left | Center | Right | Default |\n| :--- | :---: | ---: | --- |\n| α | **β** | ~γ~ | tail |\n";
        let output = parse_with_mode(source, Mode::Markdown);
        assert!(output.diagnostics.is_empty(), "{output:?}");
        let document = output.document;
        assert_eq!(document.nodes.len(), 3);

        let Block::Blockquote {
            content: quote_content,
            span: quote_span,
        } = &document.nodes[0]
        else {
            panic!("expected blockquote")
        };
        assert_eq!(
            &source[quote_span.start..quote_span.end],
            source.lines().take(4).collect::<Vec<_>>().join("\n")
        );
        assert!(matches!(quote_content[0], Block::Paragraph { .. }));
        let Block::UnorderedList { items, .. } = &quote_content[1] else {
            panic!("expected list inside blockquote")
        };
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].task, Some(TaskStatus::Active));
        assert_eq!(items[1].task, Some(TaskStatus::Completed));
        for item in items {
            assert!(source.is_char_boundary(item.span.start));
            assert!(source.is_char_boundary(item.span.end));
        }

        let Block::Paragraph { content, .. } = &document.nodes[1] else {
            panic!("expected strikethrough paragraph")
        };
        let strike = content
            .iter()
            .find_map(|inline| match inline {
                Inline::Strikethrough { span, content } => Some((span, content)),
                _ => None,
            })
            .expect("expected strikethrough");
        assert_eq!(
            &source[strike.0.start..strike.0.end],
            "~~removed *content*~~"
        );
        assert!(matches!(strike.1[1], Inline::Emphasis { .. }));

        let Block::Table { header, rows, span } = &document.nodes[2] else {
            panic!("expected table")
        };
        assert_eq!(header.cells.len(), 4);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].cells.len(), 4);
        assert_eq!(header.cells[0].alignment, TableAlignment::Left);
        assert_eq!(header.cells[1].alignment, TableAlignment::Center);
        assert_eq!(header.cells[2].alignment, TableAlignment::Right);
        assert_eq!(header.cells[3].alignment, TableAlignment::None);
        assert!(matches!(header.cells[1].content[0], Inline::Text { .. }));
        assert!(matches!(rows[0].cells[1].content[0], Inline::Strong { .. }));
        assert!(matches!(
            rows[0].cells[2].content[0],
            Inline::Strikethrough { .. }
        ));
        assert!(source.is_char_boundary(span.start));
        assert!(source.is_char_boundary(span.end));

        let crlf_source = "> 한글 **강조**\r\n>\r\n> - [x] 완료\r\n";
        let crlf = parse_with_mode(crlf_source, Mode::Markdown);
        assert!(crlf.diagnostics.is_empty(), "{crlf:?}");
        let Block::Blockquote { content, span } = &crlf.document.nodes[0] else {
            panic!("expected CRLF blockquote")
        };
        assert!(!content.is_empty());
        assert!(crlf_source.is_char_boundary(span.start));
        assert!(crlf_source.is_char_boundary(span.end));

        let qd = parse_with_diagnostics(".note\n  > body **strong**\n");
        assert!(qd.diagnostics.is_empty(), "{qd:?}");
        let Block::DirectiveCall {
            body: Some(body), ..
        } = &qd.document.nodes[0]
        else {
            panic!("expected Quarkdown body")
        };
        assert!(matches!(body[0], Block::Blockquote { .. }));
    }

    #[test]
    fn strikethrough_delimiter_width_is_preserved_across_siblings() {
        for (source, expected) in [
            ("~Hi~", vec!["~Hi~"]),
            ("~~Hi~~", vec!["~~Hi~~"]),
            ("~one~ and ~~two~~", vec!["~one~", "~~two~~"]),
            ("~~one~~ and ~two~", vec!["~~one~~", "~two~"]),
            ("~one~ and ~two~", vec!["~one~", "~two~"]),
            ("~~one~~ and ~~two~~", vec!["~~one~~", "~~two~~"]),
        ] {
            let output = parse_with_mode(source, Mode::Markdown);
            assert!(output.diagnostics.is_empty(), "{source:?}: {output:?}");
            let Block::Paragraph { content, .. } = &output.document.nodes[0] else {
                panic!("expected paragraph for {source:?}")
            };
            let spans: Vec<_> = content
                .iter()
                .filter_map(|inline| match inline {
                    Inline::Strikethrough { span, .. } => Some(&source[span.start..span.end]),
                    _ => None,
                })
                .collect();
            let mut previous_end = 0;
            for inline in content {
                if let Inline::Strikethrough { span, .. } = inline {
                    assert!(span.start >= previous_end, "overlapping span in {source:?}");
                    assert!(source.is_char_boundary(span.start));
                    assert!(source.is_char_boundary(span.end));
                    previous_end = span.end;
                }
            }
            assert_eq!(
                spans, expected,
                "source: {source:?}, document: {:?}",
                output.document
            );
        }

        for source in ["~removed *content*~", "~~removed *content*~~"] {
            let output = parse_with_mode(source, Mode::Markdown);
            assert!(output.diagnostics.is_empty(), "{source:?}: {output:?}");
            let Block::Paragraph { content, .. } = &output.document.nodes[0] else {
                panic!("expected paragraph for {source:?}")
            };
            let Inline::Strikethrough {
                content: strike_content,
                span,
            } = content.first().expect("expected strikethrough")
            else {
                panic!("expected strikethrough for {source:?}")
            };
            assert_eq!(&source[span.start..span.end], source);
            assert!(strike_content
                .iter()
                .any(|inline| matches!(inline, Inline::Emphasis { .. })));
        }

        for source in ["This will ~~~not~~~ strike.", "~~~Hi~~~ Hello, world!"] {
            let output = parse_with_mode(source, Mode::Markdown);
            assert!(output.diagnostics.is_empty(), "{source:?}: {output:?}");
            assert!(!output.document.nodes.iter().any(|block| match block {
                Block::Paragraph { content, .. } | Block::Heading { content, .. } => content
                    .iter()
                    .any(|inline| matches!(inline, Inline::Strikethrough { .. })),
                _ => false,
            }));
        }
    }

    #[test]
    fn strikethrough_spans_preserve_utf8_crlf_modes_and_containers() {
        let source = "Before ~한글~ after.\r\n";
        let output = parse_with_mode(source, Mode::Markdown);
        assert!(output.diagnostics.is_empty(), "{output:?}");
        let Block::Paragraph { content, .. } = &output.document.nodes[0] else {
            panic!("expected paragraph")
        };
        let Inline::Strikethrough {
            content: strike_content,
            span,
        } = content
            .iter()
            .find(|inline| matches!(inline, Inline::Strikethrough { .. }))
            .expect("expected UTF-8 strikethrough")
        else {
            unreachable!()
        };
        assert_eq!(&source[span.start..span.end], "~한글~");
        assert!(source.is_char_boundary(span.start));
        assert!(source.is_char_boundary(span.end));
        assert_eq!(
            strike_content.iter().find_map(|inline| match inline {
                Inline::Text { content, .. } => Some(content.as_str()),
                _ => None,
            }),
            Some("한글")
        );

        for (mode_source, mode) in [
            ("Before ~body~ after.\n", Mode::Markdown),
            ("Before ~body~ after.\n", Mode::Quarkdown),
        ] {
            let output = parse_with_mode(mode_source, mode);
            assert!(output.diagnostics.is_empty(), "{output:?}");
            let Block::Paragraph { content, .. } = &output.document.nodes[0] else {
                panic!("expected mode paragraph")
            };
            assert!(content.iter().any(|inline| matches!(
                inline,
                Inline::Strikethrough { span, .. }
                    if &mode_source[span.start..span.end] == "~body~"
            )));
        }

        let body_source = ".if {true}\n  ~body~\n";
        let body = parse_with_diagnostics(body_source);
        assert!(body.diagnostics.is_empty(), "{body:?}");
        let Block::DirectiveCall {
            body: Some(body), ..
        } = &body.document.nodes[0]
        else {
            panic!("expected Quarkdown body")
        };
        let Block::Paragraph { content, .. } = &body[0] else {
            panic!("expected body paragraph")
        };
        assert!(content.iter().any(|inline| matches!(
            inline,
            Inline::Strikethrough { span, .. }
                if span.end <= body_source.len()
                    && &body_source[span.start..span.end] == "~body~"
        )));
    }

    fn paragraph_text(block: &Block) -> String {
        let Block::Paragraph { content, .. } = block else {
            panic!("expected paragraph, got {block:?}")
        };
        content
            .iter()
            .map(|inline| match inline {
                Inline::Text { content, .. } | Inline::Code { content, .. } => content.clone(),
                Inline::Strong { content, .. } | Inline::Emphasis { content, .. } => content
                    .iter()
                    .map(|child| match child {
                        Inline::Text { content, .. } => content.clone(),
                        other => panic!("unexpected nested inline {other:?}"),
                    })
                    .collect(),
                Inline::DirectiveCall { name, .. } => format!(".{name}"),
                Inline::HardBreak { .. } | Inline::SoftBreak { .. } => String::new(),
                other => panic!("unexpected inline {other:?}"),
            })
            .collect()
    }

    fn directive_body(document: &Document) -> &Vec<Block> {
        let Block::DirectiveCall { body, .. } = &document.nodes[0] else {
            panic!("expected directive, got {:?}", document.nodes)
        };
        body.as_ref().expect("expected directive body")
    }

    #[test]
    fn quarkdown_body_uses_first_body_line_indent_not_fixed_width() {
        for indent in ["  ", "   ", "    ", "        ", "\t"] {
            let source = format!(".note\n{indent}body\n");
            let output = parse_with_diagnostics(&source);
            assert!(output.diagnostics.is_empty(), "{output:?}");
            assert_eq!(paragraph_text(&directive_body(&output.document)[0]), "body");
        }
    }

    #[test]
    fn function_body_uses_contextual_source_backed_lambda_header() {
        let source = ".function {greet}\r\n\tto from?:\r\n\tHello, .to from .from!\r\n";
        let output = parse_with_diagnostics(source);
        assert!(output.diagnostics.is_empty(), "{output:?}");
        let Block::DirectiveCall {
            lambda_header: Some(header),
            body: Some(body),
            ..
        } = &output.document.nodes[0]
        else {
            panic!("expected function lambda metadata")
        };
        assert_eq!(header.parameters.len(), 2);
        assert_eq!(header.parameters[0].name, "to");
        assert_eq!(header.parameters[1].name, "from");
        assert!(header.parameters[1].optional);
        assert_eq!(
            &source[header.parameters[0].name_span.start..header.parameters[0].name_span.end],
            "to"
        );
        assert_eq!(
            &source[header.parameters[1].span.start..header.parameters[1].span.end],
            "from?"
        );
        assert_eq!(&source[header.span.start..header.span.end], "to from?:");
        assert_eq!(paragraph_text(&body[0]), "Hello, .to from .from!");
        let Block::Paragraph { span, .. } = &body[0] else {
            panic!("expected surviving lambda body paragraph")
        };
        assert_eq!(&source[span.start..span.end], "Hello, .to from .from!");
        assert!(source.is_char_boundary(header.span.start));
        assert!(source.is_char_boundary(header.span.end));
        assert!(source.is_char_boundary(span.start));
        assert!(source.is_char_boundary(span.end));
    }

    #[test]
    fn let_explicit_lambda_header_is_source_backed_and_stripped() {
        let source = ".let {Quarkdown}\n    name:\n    Hello, **.name**!\n";
        let output = parse_with_diagnostics(source);
        assert!(output.diagnostics.is_empty(), "{output:?}");
        let Block::DirectiveCall {
            name,
            lambda_header: Some(header),
            body: Some(body),
            ..
        } = &output.document.nodes[0]
        else {
            panic!("expected explicit let lambda metadata")
        };
        assert_eq!(name, "let");
        assert_eq!(header.parameters.len(), 1);
        assert_eq!(header.parameters[0].name, "name");
        assert_eq!(&source[header.span.start..header.span.end], "name:");
        let Block::Paragraph { span, .. } = &body[0] else {
            panic!("expected surviving explicit let paragraph")
        };
        assert_eq!(&source[span.start..span.end], "Hello, **.name**!");
    }

    #[test]
    fn let_implicit_lambda_body_keeps_implicit_reference() {
        let source = ".let {Quarkdown}\n    .uppercase {.1}\n";
        let output = parse_with_diagnostics(source);
        assert!(output.diagnostics.is_empty(), "{output:?}");
        let Block::DirectiveCall {
            lambda_header,
            body: Some(body),
            ..
        } = &output.document.nodes[0]
        else {
            panic!("expected implicit let body")
        };
        assert!(lambda_header.is_none());
        let Block::DirectiveCall { name, .. } = &body[0] else {
            panic!("expected implicit let call body")
        };
        assert_eq!(name, "uppercase");
    }

    #[test]
    fn iteration_lambda_headers_are_contextual_and_source_backed() {
        for (name, parameter, body_text) in [
            ("foreach", "number", ".number"),
            ("repeat", "index", ".index"),
        ] {
            let source = format!(".{name} {{3}}\n    {parameter}:\n    {body_text}\n");
            let output = parse_with_diagnostics(&source);
            assert!(output.diagnostics.is_empty(), "{output:?}");
            let Block::DirectiveCall {
                name: actual_name,
                lambda_header: Some(header),
                body: Some(body),
                ..
            } = &output.document.nodes[0]
            else {
                panic!("expected contextual {name} lambda")
            };
            assert_eq!(actual_name, name);
            assert_eq!(header.parameters.len(), 1);
            assert_eq!(header.parameters[0].name, parameter);
            assert!(body.iter().all(|block| {
                !matches!(
                    block,
                    Block::Paragraph { content, .. }
                        if content.iter().any(|inline| matches!(
                            inline,
                            Inline::Text { content, .. } if content.contains(":")
                        ))
                )
            }));
            assert!(body.iter().any(|block| {
                matches!(
                    block,
                    Block::DirectiveCall { name, .. } if name == parameter
                ) || matches!(
                    block,
                    Block::Paragraph { content, .. }
                        if content.iter().any(|inline| matches!(
                            inline,
                            Inline::DirectiveCall { name, .. } if name == parameter
                        ))
                )
            }));
        }

        for name in ["foreach", "repeat"] {
            let source = format!(".{name} {{3}}\n    .1\n");
            let output = parse_with_diagnostics(&source);
            assert!(output.diagnostics.is_empty(), "{output:?}");
            let Block::DirectiveCall {
                lambda_header,
                body: Some(body),
                ..
            } = &output.document.nodes[0]
            else {
                panic!("expected implicit {name} lambda")
            };
            assert!(lambda_header.is_none());
            assert!(body.iter().any(|block| {
                matches!(
                    block,
                    Block::DirectiveCall { name, .. } if name == "1"
                ) || matches!(
                    block,
                    Block::Paragraph { content, .. }
                        if content.iter().any(|inline| matches!(
                            inline,
                            Inline::DirectiveCall { name, .. } if name == "1"
                        ))
                )
            }));
        }
    }

    #[test]
    fn marked_inline_lambda_is_structural_and_source_backed() {
        let output = parse_with_diagnostics(".sorted {1..3} by:{@lambda .1}\n");
        assert!(output.diagnostics.is_empty(), "{output:?}");
        let Block::DirectiveCall { named_args, .. } = &output.document.nodes[0] else {
            panic!("expected sorted directive")
        };
        assert!(matches!(
            named_args.first().map(|argument| &argument.value),
            Some(Value::Lambda {
                parameters: None,
                body,
                span,
            }) if !body.is_empty() && span.start < span.end
        ));
    }

    #[test]
    fn transform_callback_lambda_uses_contextual_unmarked_form() {
        let output = parse_with_diagnostics(".map {1..3} by:{value: .value}\n");
        assert!(output.diagnostics.is_empty(), "{output:?}");
        let Block::DirectiveCall { named_args, .. } = &output.document.nodes[0] else {
            panic!("expected map directive")
        };
        assert!(matches!(
            named_args.first().map(|argument| &argument.value),
            Some(Value::Lambda {
                parameters: Some(header),
                body,
                ..
            }) if header.parameters[0].name == "value" && !body.is_empty()
        ));
    }

    #[test]
    fn iteration_inline_body_preserves_contextual_metadata_without_eager_lambda_coercion() {
        let output = parse_with_diagnostics(".foreach {1..3} {item: .item}\n");
        assert!(output.diagnostics.is_empty(), "{output:?}");
        let Block::DirectiveCall {
            positional_args, ..
        } = &output.document.nodes[0]
        else {
            panic!("expected foreach directive")
        };
        assert!(matches!(
            positional_args.get(1),
            Some(Value::InlineBody {
                parameters: Some(header),
                body,
                ..
            }) if header.parameters[0].name == "item" && !body.is_empty()
        ));
    }

    #[test]
    fn implicit_iteration_body_keeps_nested_named_arguments_in_the_body() {
        let output = parse_with_diagnostics(".foreach {1..3} {.islower {.1} than:{5}}\n");
        assert!(output.diagnostics.is_empty(), "{output:?}");
        let Block::DirectiveCall {
            positional_args, ..
        } = &output.document.nodes[0]
        else {
            panic!("expected foreach directive")
        };
        assert!(matches!(
            positional_args.get(1),
            Some(Value::InlineBody {
                parameters: None,
                body,
                ..
            }) if body.iter().any(|inline| matches!(
                inline,
                Inline::DirectiveCall { name, .. } if name == "islower"
            ))
        ));
    }

    #[test]
    fn let_header_utf8_span_is_exact_for_crlf_source() {
        let source = ".let {값}\r\n\tname:\r\n\t안녕, .name!\r\n";
        let output = parse_with_diagnostics(source);
        assert!(output.diagnostics.is_empty(), "{output:?}");
        let Block::DirectiveCall {
            lambda_header: Some(header),
            body: Some(body),
            ..
        } = &output.document.nodes[0]
        else {
            panic!("expected UTF-8 let header")
        };
        assert_eq!(
            &source[header.parameters[0].name_span.start..header.parameters[0].name_span.end],
            "name"
        );
        assert_eq!(&source[header.span.start..header.span.end], "name:");
        let Block::Paragraph { span, .. } = &body[0] else {
            panic!("expected surviving let body")
        };
        assert_eq!(&source[span.start..span.end], "안녕, .name!");
        assert!(source.is_char_boundary(header.span.start));
        assert!(source.is_char_boundary(header.span.end));
        assert!(source.is_char_boundary(span.start));
        assert!(source.is_char_boundary(span.end));
    }

    #[test]
    fn let_nested_container_span_keeps_original_body_ranges() {
        let source = "- .let {값}\n    name:\n    안녕, .name!\n";
        let output = parse_with_diagnostics(source);
        assert!(output.diagnostics.is_empty(), "{output:?}");
        let Block::UnorderedList { items, .. } = &output.document.nodes[0] else {
            panic!("expected list container")
        };
        let Block::DirectiveCall {
            lambda_header: Some(header),
            body: Some(body),
            ..
        } = &items[0].content[0]
        else {
            panic!("expected nested let")
        };
        assert_eq!(header.parameters[0].name, "name");
        let body_start = source.find("안녕").expect("nested let body");
        let Block::Paragraph { span, .. } = &body[0] else {
            panic!("expected nested let paragraph")
        };
        assert_eq!(span.start, body_start);
        assert_eq!(&source[span.start..span.end], "안녕, .name!");
    }

    #[test]
    fn ordinary_non_lambda_body_with_colon_is_not_stripped() {
        let output = parse_with_diagnostics(".container\n  label:\n  ordinary content\n");
        assert!(output.diagnostics.is_empty(), "{output:?}");
        let Block::DirectiveCall {
            lambda_header,
            body,
            ..
        } = &output.document.nodes[0]
        else {
            panic!("expected ordinary call")
        };
        assert!(lambda_header.is_none());
        assert_eq!(
            paragraph_text(&body.as_ref().unwrap()[0]),
            "label:ordinary content"
        );
    }

    #[test]
    fn function_lambda_header_keeps_container_relative_body_indentation() {
        let list_source = "- .function {greet}\n    name:\n    Hello, .name!\n";
        let list = parse_with_diagnostics(list_source);
        assert!(list.diagnostics.is_empty(), "{list:?}");
        let Block::UnorderedList { items, .. } = &list.document.nodes[0] else {
            panic!("expected list")
        };
        let Block::DirectiveCall {
            lambda_header: Some(header),
            body: Some(body),
            ..
        } = &items[0].content[0]
        else {
            panic!("expected function declaration inside list")
        };
        assert_eq!(header.parameters[0].name, "name");
        assert_eq!(paragraph_text(&body[0]), "Hello, .name!");
        let Block::Paragraph { span, .. } = &body[0] else {
            panic!("expected list lambda body paragraph")
        };
        let body_start = list_source.find("Hello").expect("list body text");
        assert_eq!(&list_source[span.start..span.end], "Hello, .name!");
        assert_eq!(span.start, body_start);

        let quote_source = "> .function {greet}\n>   name:\n>   Hello, .name!\n";
        let quote = parse_with_diagnostics(quote_source);
        assert!(quote.diagnostics.is_empty(), "{quote:?}");
        let Block::Blockquote { content, .. } = &quote.document.nodes[0] else {
            panic!("expected blockquote")
        };
        let Block::DirectiveCall {
            lambda_header: Some(header),
            body: Some(body),
            ..
        } = &content[0]
        else {
            panic!("expected function declaration inside blockquote")
        };
        assert_eq!(header.parameters[0].name, "name");
        assert_eq!(paragraph_text(&body[0]), "Hello, .name!");
        let Block::Paragraph { span, .. } = &body[0] else {
            panic!("expected blockquote lambda body paragraph")
        };
        let body_start = quote_source.find("Hello").expect("quote body text");
        assert_eq!(&quote_source[span.start..span.end], "Hello, .name!");
        assert_eq!(span.start, body_start);
    }

    #[test]
    fn function_lambda_header_reanchors_surviving_utf8_body_span() {
        for ending in ["\n", "\r\n"] {
            for indent in ["  ", "   ", "    ", "        ", "\t"] {
                let source = format!(
                    ".function {{greet}}{ending}{indent}name:{ending}{indent}안녕, .name!{ending}"
                );
                let output = parse_with_diagnostics(&source);
                assert!(output.diagnostics.is_empty(), "{output:?}");
                let Block::DirectiveCall {
                    body: Some(body), ..
                } = &output.document.nodes[0]
                else {
                    panic!("expected function body")
                };
                let Block::Paragraph { span, .. } = &body[0] else {
                    panic!("expected surviving body paragraph")
                };
                let start = source.find("안녕").expect("body text");
                let end = start + "안녕, .name!".len();
                assert_eq!((span.start, span.end), (start, end));
                assert_eq!(&source[span.start..span.end], "안녕, .name!");
                assert!(source.is_char_boundary(span.start));
                assert!(source.is_char_boundary(span.end));
            }
        }
    }

    #[test]
    fn quarkdown_body_rejects_one_space() {
        let output = parse_with_diagnostics(".note\n body\n");
        let Block::DirectiveCall { body, .. } = &output.document.nodes[0] else {
            panic!("expected directive")
        };
        assert!(body.is_none());
        assert_eq!(paragraph_text(&output.document.nodes[1]), "body");
    }

    #[test]
    fn quarkdown_body_tab_preserves_text_and_utf8_spans() {
        let source = ".note\n\t한글 body\n";
        let output = parse_with_diagnostics(source);
        assert!(output.diagnostics.is_empty(), "{output:?}");
        let Block::DirectiveCall {
            body: Some(body),
            span: directive_span,
            ..
        } = &output.document.nodes[0]
        else {
            panic!("expected directive body")
        };
        let Block::Paragraph {
            content: paragraph_content,
            span: paragraph_span,
        } = &body[0]
        else {
            panic!("expected body paragraph")
        };
        assert_eq!(paragraph_text(&body[0]), "한글 body");
        let Inline::Text {
            content: text,
            span: text_span,
        } = &paragraph_content[0]
        else {
            panic!("expected body text")
        };
        assert_eq!(text, "한글");
        for span in [directive_span, paragraph_span, text_span] {
            assert!(span.start <= span.end);
            assert!(span.end <= source.len());
            assert!(source.is_char_boundary(span.start));
            assert!(source.is_char_boundary(span.end));
            assert!(source.get(span.start..span.end).is_some());
        }
    }

    #[test]
    fn quarkdown_body_accepts_mixed_indentation_without_byte_width_assumptions() {
        for source in [".note\n\t  body\n", ".note\n  \tbody\n"] {
            let output = parse_with_diagnostics(source);
            assert!(output.diagnostics.is_empty(), "{output:?}");
            assert_eq!(paragraph_text(&directive_body(&output.document)[0]), "body");
        }
    }

    #[test]
    fn quarkdown_body_dedent_terminates_body_and_shallower_lines_are_not_absorbed() {
        let output = parse_with_diagnostics(".note\n    first\n  second\n\noutside\n");
        let Block::DirectiveCall {
            body: Some(body), ..
        } = &output.document.nodes[0]
        else {
            panic!("expected directive body")
        };
        assert_eq!(body.len(), 1);
        assert_eq!(paragraph_text(&body[0]), "first");
        assert_eq!(paragraph_text(&output.document.nodes[1]), "second");
        assert_eq!(paragraph_text(&output.document.nodes[2]), "outside");
    }

    #[test]
    fn quarkdown_body_blank_lines_preserve_body_lifecycle() {
        let before_body = parse_with_diagnostics(".note\n\n    body\n");
        assert_eq!(
            paragraph_text(&directive_body(&before_body.document)[0]),
            "body"
        );

        let inside_body = parse_with_diagnostics(".note\n    first\n\n    second\n");
        let body = directive_body(&inside_body.document);
        assert_eq!(body.len(), 2);
        assert_eq!(paragraph_text(&body[0]), "first");
        assert_eq!(paragraph_text(&body[1]), "second");
    }

    #[test]
    fn quarkdown_body_preserves_nested_markdown() {
        let output = parse_with_diagnostics(".panel\n  **strong** and `code`\n");
        let Block::Paragraph { content, .. } = &directive_body(&output.document)[0] else {
            panic!("expected paragraph")
        };
        assert!(content
            .iter()
            .any(|inline| matches!(inline, Inline::Strong { .. })));
        assert!(content
            .iter()
            .any(|inline| matches!(inline, Inline::Code { content, .. } if content == "code")));
    }

    #[test]
    fn quarkdown_body_preserves_nested_quarkdown_blocks() {
        let output = parse_with_diagnostics(".panel\n  .note\n    inner\n");
        let Block::DirectiveCall {
            name: outer_name,
            body: Some(outer_body),
            ..
        } = &output.document.nodes[0]
        else {
            panic!("expected outer directive")
        };
        assert_eq!(outer_name, "panel");
        let Block::DirectiveCall {
            name: inner_name,
            body: Some(inner_body),
            ..
        } = &outer_body[0]
        else {
            panic!("expected nested directive")
        };
        assert_eq!(inner_name, "note");
        assert_eq!(paragraph_text(&inner_body[0]), "inner");
    }

    #[test]
    fn quarkdown_body_preserves_inline_quarkdown_calls() {
        let output = parse_with_diagnostics(".panel\n  before .text {red} after\n");
        let Block::Paragraph { content, .. } = &directive_body(&output.document)[0] else {
            panic!("expected paragraph")
        };
        assert!(matches!(
            content.get(1),
            Some(Inline::DirectiveCall { name, .. }) if name == "text"
        ));
    }

    #[test]
    fn quarkdown_body_is_container_relative_in_lists_and_blockquotes() {
        let list = parse_with_diagnostics("- .panel\n    body\n");
        let Block::UnorderedList { items, .. } = &list.document.nodes[0] else {
            panic!("expected list")
        };
        let Block::DirectiveCall { body, .. } = &items[0].content[0] else {
            panic!("expected directive inside list")
        };
        assert_eq!(paragraph_text(&body.as_ref().unwrap()[0]), "body");

        let quote = parse_with_diagnostics("> .panel\n>   body\n");
        let Block::Blockquote { content, .. } = &quote.document.nodes[0] else {
            panic!("expected blockquote")
        };
        let Block::DirectiveCall { body, .. } = &content[0] else {
            panic!("expected directive inside blockquote")
        };
        assert_eq!(paragraph_text(&body.as_ref().unwrap()[0]), "body");
    }

    #[test]
    fn quarkdown_body_has_same_semantics_for_lf_and_crlf() {
        let lf = parse_with_diagnostics(".note\n  body\n");
        let crlf = parse_with_diagnostics(".note\r\n  body\r\n");
        assert_eq!(paragraph_text(&directive_body(&lf.document)[0]), "body");
        assert_eq!(paragraph_text(&directive_body(&crlf.document)[0]), "body");
        assert_eq!(lf.diagnostics, crlf.diagnostics);
    }
}
