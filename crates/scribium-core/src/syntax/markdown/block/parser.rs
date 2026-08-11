//! Single-owner Markdown block parser state.

use super::classify::{
    classify, is_closing_fence, is_same_list_item, is_unordered_item, list_marker_layout,
    BlockStart, ListKind,
};
use super::line::{LineView, SourceLine};
use crate::source::ByteSpan;
use crate::syntax::markdown::ast::{Block, ListItem};
use crate::syntax::markdown::parser::{
    convert_quarkdown_arg, parse_inlines, ParserDiagnostic, MAX_BLOCK_DEPTH,
};

#[derive(Debug)]
pub(crate) struct BlockParser<'source, 'lines, 'diagnostics> {
    source: &'source str,
    lines: &'lines [SourceLine<'source>],
    cursor: usize,
    diagnostics: &'diagnostics mut Vec<ParserDiagnostic>,
    open_containers: Vec<OpenContainer>,
    open_leaf: Option<OpenLeaf>,
    root: Vec<Block>,
}

#[derive(Debug)]
enum OpenContainer {
    List(ListState),
    ListItem(ListItemState),
    ExtensionBody(ExtensionBodyState),
}

#[derive(Debug)]
struct ListState {
    kind: ListKind,
    base_indent: usize,
    start: Option<usize>,
    items: Vec<ListItem>,
}

#[derive(Debug)]
struct ListItemState {
    start: usize,
    content_indent: usize,
    last_end: usize,
    blocks: Vec<Block>,
}

#[derive(Debug)]
struct ExtensionBodyState {
    content_indent: usize,
    call: crate::syntax::quarkdown::QuarkdownCall,
    blocks: Vec<Block>,
}

#[derive(Debug)]
enum OpenLeaf {
    Paragraph {
        start: usize,
        content_end: usize,
        end: usize,
    },
    FencedCode {
        start: usize,
        fence_len: usize,
        language: Option<String>,
        content: Vec<String>,
        end: usize,
    },
}

impl<'source, 'lines, 'diagnostics> BlockParser<'source, 'lines, 'diagnostics> {
    pub(crate) fn new(
        source: &'source str,
        lines: &'lines [SourceLine<'source>],
        cursor: usize,
        diagnostics: &'diagnostics mut Vec<ParserDiagnostic>,
    ) -> Self {
        Self {
            source,
            lines,
            cursor,
            diagnostics,
            open_containers: Vec::new(),
            open_leaf: None,
            root: Vec::new(),
        }
    }

    pub(crate) fn parse(mut self) -> Vec<Block> {
        while self.cursor < self.lines.len() {
            self.process_line();
        }
        self.finish();
        self.root
    }

    fn process_line(&mut self) {
        self.reconcile_containers();
        if self.cursor >= self.lines.len() {
            return;
        }

        let view = self.current_view();
        if view.is_blank() {
            self.finish_leaf();
            self.mark_consumed(view.end);
            self.cursor += 1;
            return;
        }

        if let Some(OpenLeaf::FencedCode { fence_len, .. }) = self.open_leaf.as_ref() {
            let fence_len = *fence_len;
            if is_closing_fence(view.text, fence_len) {
                if let Some(OpenLeaf::FencedCode { end, .. }) = self.open_leaf.as_mut() {
                    *end = view.end;
                }
                self.mark_consumed(view.end);
                self.cursor += 1;
                self.finish_leaf();
            } else {
                if let Some(OpenLeaf::FencedCode { content, end, .. }) = self.open_leaf.as_mut() {
                    content.push(view.raw.to_string());
                    *end = view.end;
                }
                self.mark_consumed(view.end);
                self.cursor += 1;
            }
            return;
        }

        let candidate = classify(self.source, view);
        if self.open_leaf.is_some() {
            // A block candidate interrupts the current paragraph. The line
            // is deliberately not consumed until it is reprocessed without
            // the finalized leaf.
            if candidate.is_some() {
                self.finish_leaf();
                return;
            }
            if let Some(OpenLeaf::Paragraph {
                content_end, end, ..
            }) = self.open_leaf.as_mut()
            {
                *content_end = view.content_end();
                *end = view.end;
            }
            self.mark_consumed(view.end);
            self.cursor += 1;
            return;
        }

        self.start_candidate(view, candidate);
    }

    fn start_candidate(&mut self, view: LineView<'source>, candidate: Option<BlockStart>) {
        match candidate {
            Some(BlockStart::Heading { level }) => {
                self.cursor += 1;
                self.mark_consumed(view.end);
                let block = parse_heading(self.source, view, level, self.diagnostics);
                self.emit(block);
            }
            Some(BlockStart::Fence { length }) => {
                self.cursor += 1;
                self.mark_consumed(view.end);
                self.open_leaf = Some(OpenLeaf::FencedCode {
                    start: view.text_start,
                    fence_len: length,
                    language: view.text[length..]
                        .split_whitespace()
                        .next()
                        .map(str::to_string),
                    content: Vec::new(),
                    end: view.end,
                });
            }
            Some(BlockStart::ThematicBreak) => {
                self.cursor += 1;
                self.mark_consumed(view.end);
                self.emit(Block::ThematicBreak {
                    span: ByteSpan::new(view.text_start, view.end),
                });
            }
            Some(candidate @ BlockStart::UnorderedList { .. })
            | Some(candidate @ BlockStart::OrderedList { .. })
                if self.parse_depth() < MAX_BLOCK_DEPTH =>
            {
                self.cursor += 1;
                if self.is_current_list_candidate(view, &candidate) {
                    self.start_item_on_existing_list(view);
                } else {
                    self.start_list_from_view(view, candidate);
                }
            }
            Some(BlockStart::QuarkdownCall { call }) => {
                self.cursor += 1;
                self.mark_consumed(view.end);
                self.start_directive(view, call);
            }
            // Keep the legacy depth guard: a list marker at the limit starts
            // a separate paragraph rather than recursively opening another
            // list container.
            Some(BlockStart::UnorderedList { .. } | BlockStart::OrderedList { .. }) | None => {
                self.open_leaf = Some(OpenLeaf::Paragraph {
                    start: view.text_start,
                    content_end: view.content_end(),
                    end: view.end,
                });
                self.mark_consumed(view.end);
                self.cursor += 1;
            }
        }
    }

    /// Start a list whose marker is represented by `view`. The line has
    /// already been consumed from `cursor`; the helper is also used for a
    /// nested list that begins on the content part of its parent's item line.
    fn start_list_from_view(&mut self, view: LineView<'source>, candidate: BlockStart) {
        let (kind, start) = match candidate {
            BlockStart::UnorderedList { marker } => (ListKind::Unordered(marker), None),
            BlockStart::OrderedList { start, delimiter } => {
                (ListKind::Ordered(delimiter), Some(start))
            }
            _ => return,
        };
        self.open_containers.push(OpenContainer::List(ListState {
            kind,
            base_indent: view.prefix + view.indent,
            start,
            items: Vec::new(),
        }));
        self.start_item_on_existing_list(view);
    }

    fn is_current_list_candidate(&self, view: LineView<'source>, candidate: &BlockStart) -> bool {
        let Some(OpenContainer::List(list)) = self.open_containers.last() else {
            return false;
        };
        match (list.kind, candidate) {
            (ListKind::Unordered(marker), BlockStart::UnorderedList { marker: next }) => {
                marker == *next && view.prefix == list.base_indent
            }
            (
                ListKind::Ordered(delimiter),
                BlockStart::OrderedList {
                    delimiter: next, ..
                },
            ) => delimiter == *next && view.prefix == list.base_indent,
            _ => false,
        }
    }

    fn start_item_on_existing_list(&mut self, view: LineView<'source>) {
        let Some((marker_width, _marker, full_marker_width)) = list_marker_layout(view.text) else {
            self.open_leaf = Some(OpenLeaf::Paragraph {
                start: view.text_start,
                content_end: view.content_end(),
                end: view.end,
            });
            self.mark_consumed(view.end);
            return;
        };
        let whitespace = view.text[marker_width..]
            .bytes()
            .take_while(|byte| *byte == b' ' || *byte == b'\t')
            .count();
        let content_indent = view.prefix + view.indent + full_marker_width + whitespace;
        let content_start = view.text_start + full_marker_width + whitespace;
        self.open_containers
            .push(OpenContainer::ListItem(ListItemState {
                start: view.raw_start,
                content_indent,
                last_end: view.end,
                blocks: Vec::new(),
            }));
        self.mark_consumed(view.end);

        let content_view = if content_start < view.content_end() {
            SourceLine {
                raw: view.raw,
                text: view.text,
                raw_start: view.raw_start,
                text_start: view.text_start,
                term: view.term,
                end: view.end,
            }
            .item_content_view(content_indent, content_start)
        } else {
            return;
        };

        self.process_item_content(content_view);
    }

    fn process_item_content(&mut self, view: LineView<'source>) {
        if view.is_blank() {
            return;
        }
        let candidate = classify(self.source, view);
        match candidate {
            Some(candidate @ BlockStart::UnorderedList { .. })
            | Some(candidate @ BlockStart::OrderedList { .. })
                if self.parse_depth() < MAX_BLOCK_DEPTH =>
            {
                self.start_list_from_view(view, candidate);
            }
            Some(BlockStart::Heading { level }) => {
                let block = parse_heading(self.source, view, level, self.diagnostics);
                self.emit(block);
            }
            Some(BlockStart::Fence { length }) => {
                self.open_leaf = Some(OpenLeaf::FencedCode {
                    start: view.text_start,
                    fence_len: length,
                    language: view.text[length..]
                        .split_whitespace()
                        .next()
                        .map(str::to_string),
                    content: Vec::new(),
                    end: view.end,
                });
            }
            Some(BlockStart::ThematicBreak) => {
                self.emit(Block::ThematicBreak {
                    span: ByteSpan::new(view.text_start, view.end),
                });
            }
            Some(BlockStart::QuarkdownCall { call }) => self.start_directive(view, call),
            Some(BlockStart::UnorderedList { .. } | BlockStart::OrderedList { .. }) | None => {
                self.open_leaf = Some(OpenLeaf::Paragraph {
                    start: view.text_start,
                    content_end: view.content_end(),
                    end: view.end,
                });
            }
        }
    }

    fn start_directive(
        &mut self,
        view: LineView<'source>,
        call: crate::syntax::quarkdown::QuarkdownCall,
    ) {
        if let Some(content_indent) = self.directive_body_indent(view.prefix) {
            self.open_containers
                .push(OpenContainer::ExtensionBody(ExtensionBodyState {
                    content_indent,
                    call,
                    blocks: Vec::new(),
                }));
        } else {
            let block = self.directive_block(call, None);
            self.emit(block);
        }
    }

    fn directive_body_indent(&self, parent_prefix: usize) -> Option<usize> {
        let mut index = self.cursor;
        while index < self.lines.len() && self.lines[index].is_blank() {
            index += 1;
        }
        let line = self.lines.get(index).copied()?;
        let view = line.view(parent_prefix);
        if view.indent >= 2 || view.raw.starts_with('\t') {
            Some(parent_prefix + view.indent)
        } else {
            None
        }
    }

    fn current_view(&self) -> LineView<'source> {
        let prefix = self.current_prefix();
        self.lines[self.cursor].view(prefix)
    }

    fn current_prefix(&self) -> usize {
        self.open_containers
            .last()
            .map(|container| match container {
                OpenContainer::List(state) => state.base_indent,
                OpenContainer::ListItem(state) => state.content_indent,
                OpenContainer::ExtensionBody(state) => state.content_indent,
            })
            .unwrap_or(0)
    }

    fn reconcile_containers(&mut self) {
        loop {
            let Some(index) = self.open_containers.len().checked_sub(1) else {
                return;
            };
            if self.container_continues(index) {
                return;
            }
            self.close_top_container();
        }
    }

    fn container_continues(&self, index: usize) -> bool {
        let line = self.lines[self.cursor];
        match &self.open_containers[index] {
            OpenContainer::ListItem(state) => {
                if line.indent() >= state.content_indent {
                    return true;
                }
                line.is_blank() && self.blank_keeps_item(index, state.content_indent)
            }
            OpenContainer::List(state) => {
                let view = line.view(state.base_indent);
                if view.is_blank() {
                    return false;
                }
                match (state.kind, classify(self.source, view)) {
                    (
                        ListKind::Unordered(marker),
                        Some(BlockStart::UnorderedList { marker: next }),
                    ) => marker == next,
                    (
                        ListKind::Ordered(delimiter),
                        Some(BlockStart::OrderedList {
                            delimiter: next, ..
                        }),
                    ) => delimiter == next,
                    _ => false,
                }
            }
            OpenContainer::ExtensionBody(state) => {
                if line.indent() >= state.content_indent {
                    return true;
                }
                if !line.is_blank() {
                    return false;
                }
                let mut next = self.cursor + 1;
                while next < self.lines.len() && self.lines[next].is_blank() {
                    next += 1;
                }
                next < self.lines.len() && self.lines[next].indent() >= state.content_indent
            }
        }
    }

    fn blank_keeps_item(&self, index: usize, content_indent: usize) -> bool {
        let mut next = self.cursor + 1;
        while next < self.lines.len() && self.lines[next].is_blank() {
            next += 1;
        }
        let Some(line) = self.lines.get(next).copied() else {
            return false;
        };
        if line.indent() >= content_indent {
            return true;
        }
        let Some(OpenContainer::List(list)) = index
            .checked_sub(1)
            .and_then(|i| self.open_containers.get(i))
        else {
            return false;
        };
        let view = line.view(list.base_indent);
        match list.kind {
            ListKind::Unordered(marker) => {
                is_same_list_item(view.text, ListKind::Unordered(marker))
            }
            // Preserve the current parser's ordered-list boundary behavior:
            // an unordered marker after a blank also closes the ordered list,
            // but the blank remains part of the current item span.
            ListKind::Ordered(delimiter) => {
                is_same_list_item(view.text, ListKind::Ordered(delimiter))
                    || is_unordered_item(line.text)
            }
        }
    }

    fn close_top_container(&mut self) {
        self.finish_leaf();
        let Some(container) = self.open_containers.pop() else {
            return;
        };
        match container {
            OpenContainer::ListItem(state) => {
                let item = ListItem {
                    content: state.blocks,
                    span: ByteSpan::new(state.start, state.last_end),
                };
                if let Some(OpenContainer::List(list)) = self.open_containers.last_mut() {
                    list.items.push(item);
                }
            }
            OpenContainer::List(state) => {
                let (span_start, span_end) = match (state.items.first(), state.items.last()) {
                    (Some(first), Some(last)) => (first.span.start, last.span.end),
                    _ => return,
                };
                let span = ByteSpan::new(span_start, span_end);
                let start = state.start.unwrap_or(1);
                let block = match state.kind {
                    ListKind::Unordered(_) => Block::UnorderedList {
                        items: state.items,
                        span,
                    },
                    ListKind::Ordered(_) => Block::OrderedList {
                        items: state.items,
                        start,
                        span,
                    },
                };
                self.emit(block);
            }
            OpenContainer::ExtensionBody(state) => {
                let block = self.directive_block(state.call, Some(state.blocks));
                self.emit(block);
            }
        }
    }

    fn finish_leaf(&mut self) {
        let Some(leaf) = self.open_leaf.take() else {
            return;
        };
        match leaf {
            OpenLeaf::Paragraph {
                start,
                content_end,
                end,
            } => {
                let content = parse_inlines(self.source, start, content_end, 0, self.diagnostics);
                self.emit(Block::Paragraph {
                    content,
                    span: ByteSpan::new(start, end),
                });
            }
            OpenLeaf::FencedCode {
                start,
                language,
                content,
                end,
                ..
            } => self.emit(Block::CodeBlock {
                language,
                source: content.join("\n"),
                span: ByteSpan::new(start, end),
            }),
        }
    }

    fn finish(&mut self) {
        self.finish_leaf();
        while !self.open_containers.is_empty() {
            self.close_top_container();
        }
    }

    fn mark_consumed(&mut self, end: usize) {
        for container in &mut self.open_containers {
            match container {
                OpenContainer::ListItem(state) => state.last_end = end,
                OpenContainer::ExtensionBody(_) | OpenContainer::List(_) => {}
            }
        }
    }

    fn parse_depth(&self) -> usize {
        self.open_containers
            .iter()
            .filter(|container| {
                matches!(
                    container,
                    OpenContainer::List(_) | OpenContainer::ExtensionBody(_)
                )
            })
            .count()
    }

    fn directive_block(
        &mut self,
        call: crate::syntax::quarkdown::QuarkdownCall,
        body: Option<Vec<Block>>,
    ) -> Block {
        let positional_args = call
            .positional_args
            .iter()
            .map(|arg| {
                convert_quarkdown_arg(self.source, arg, self.parse_depth() + 1, self.diagnostics)
            })
            .collect();
        let named_args = call
            .named_args
            .iter()
            .map(|named| {
                (
                    named.name.clone(),
                    convert_quarkdown_arg(
                        self.source,
                        &named.value,
                        self.parse_depth() + 1,
                        self.diagnostics,
                    ),
                )
            })
            .collect();
        let span = block_span_with_body(&call.span, &body);
        Block::DirectiveCall {
            name: call.name,
            positional_args,
            named_args,
            body,
            span,
        }
    }

    fn emit(&mut self, block: Block) {
        match self.open_containers.last_mut() {
            Some(OpenContainer::ListItem(state)) => state.blocks.push(block),
            Some(OpenContainer::ExtensionBody(state)) => state.blocks.push(block),
            Some(OpenContainer::List(_)) | None => self.root.push(block),
        }
    }
}

fn parse_heading(
    source: &str,
    view: LineView<'_>,
    level: usize,
    diagnostics: &mut Vec<ParserDiagnostic>,
) -> Block {
    let content_start = view.text_start + level;
    let rest = &view.text[level..];
    let trim = rest
        .bytes()
        .take_while(|byte| *byte == b' ' || *byte == b'\t')
        .count();
    let content_start = content_start + trim;
    let content_end = trailing_hash_run_start(source, view.text_start, view.content_end())
        .unwrap_or_else(|| view.content_end());
    let content = if content_start < content_end {
        parse_inlines(source, content_start, content_end, 0, diagnostics)
    } else {
        Vec::new()
    };
    Block::Heading {
        level,
        content,
        span: ByteSpan::new(view.text_start, view.end),
    }
}

fn trailing_hash_run_start(source: &str, start: usize, end: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut position = end;
    while position > start && bytes[position - 1] == b'#' {
        position -= 1;
    }
    if position == end {
        return None;
    }
    if position > start && matches!(bytes[position - 1], b' ' | b'\t') {
        Some(position - 1)
    } else {
        None
    }
}

fn block_span(block: &Block) -> ByteSpan {
    match block {
        Block::Heading { span, .. }
        | Block::Paragraph { span, .. }
        | Block::UnorderedList { span, .. }
        | Block::OrderedList { span, .. }
        | Block::CodeBlock { span, .. }
        | Block::ThematicBreak { span }
        | Block::BlankLine { span }
        | Block::DirectiveCall { span, .. }
        | Block::Metadata { span, .. } => *span,
    }
}

fn block_span_with_body(header: &ByteSpan, body: &Option<Vec<Block>>) -> ByteSpan {
    match body {
        Some(blocks) => {
            let end = blocks
                .last()
                .map(block_span)
                .map(|span| span.end)
                .unwrap_or(header.end);
            ByteSpan::new(header.start, end.max(header.end))
        }
        None => *header,
    }
}
