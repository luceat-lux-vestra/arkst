//! Pure block-start classification over a container-relative line view.

use super::line::LineView;

/// A block candidate. Candidates describe syntax only; `BlockParser` owns
/// interruption, container lifecycle, cursor movement, and emission.
#[derive(Debug, Clone)]
pub(super) enum BlockStart {
    Heading {
        level: usize,
    },
    Fence {
        length: usize,
    },
    ThematicBreak,
    UnorderedList {
        marker: u8,
    },
    OrderedList {
        start: usize,
        delimiter: u8,
    },
    QuarkdownCall {
        call: crate::syntax::quarkdown::QuarkdownCall,
    },
}

/// Classify the remaining content of a physical line in the same order as
/// the legacy parser's block dispatch. This function never mutates parser
/// state and never consumes the line.
pub(super) fn classify(source: &str, view: LineView<'_>) -> Option<BlockStart> {
    if view.is_blank() {
        return None;
    }
    if let Some(level) = is_heading_text(view.text) {
        return Some(BlockStart::Heading { level });
    }
    if let Some(length) = fence_length(view.text) {
        return Some(BlockStart::Fence { length });
    }
    if is_thematic_break(view.text) {
        return Some(BlockStart::ThematicBreak);
    }
    if let Some(marker) = is_list_marker(view.text) {
        return Some(BlockStart::UnorderedList { marker });
    }
    if let Some((start, delimiter)) = is_ordered_list_marker(view.text) {
        return Some(BlockStart::OrderedList { start, delimiter });
    }
    if let Ok(Some((call, _consumed))) =
        crate::syntax::quarkdown::parse_directive_at(source, view.text_start)
    {
        if source
            .get(call.span.end..view.content_end())
            .is_some_and(|rest| rest.bytes().all(|byte| byte == b' ' || byte == b'\t'))
        {
            return Some(BlockStart::QuarkdownCall { call });
        }
    }
    None
}

pub(super) fn list_marker_layout(text: &str) -> Option<(usize, u8, usize)> {
    if let Some(marker) = is_list_marker(text) {
        return Some((1, marker, 1));
    }
    let (_, delimiter) = is_ordered_list_marker(text)?;
    let marker_end = text.bytes().position(|byte| byte == delimiter)?;
    Some((marker_end + 1, delimiter, marker_end + 1))
}

pub(super) fn is_same_list_item(text: &str, kind: ListKind) -> bool {
    match kind {
        ListKind::Unordered(marker) => is_item_start(text, marker),
        ListKind::Ordered(delimiter) => is_ordered_list_marker(text)
            .map(|(_, candidate)| candidate == delimiter)
            .unwrap_or(false),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ListKind {
    Unordered(u8),
    Ordered(u8),
}

pub(super) fn is_unordered_item(text: &str) -> bool {
    is_list_marker(text).is_some()
}

pub(super) fn is_closing_fence(text: &str, fence_len: usize) -> bool {
    !text.is_empty() && text.len() >= fence_len && text.bytes().all(|byte| byte == b'`')
}

fn fence_length(text: &str) -> Option<usize> {
    let length = text.bytes().take_while(|byte| *byte == b'`').count();
    (length >= 3).then_some(length)
}

fn is_heading_text(text: &str) -> Option<usize> {
    let hashes = text.bytes().take_while(|byte| *byte == b'#').count();
    if !(1..=6).contains(&hashes) {
        return None;
    }
    let rest = &text[hashes..];
    if rest.is_empty() || matches!(rest.as_bytes()[0], b' ' | b'\t') {
        Some(hashes)
    } else {
        None
    }
}

fn is_thematic_break(text: &str) -> bool {
    let mut marker = 0u8;
    let mut count = 0usize;
    for byte in text.bytes() {
        match byte {
            b' ' | b'\t' => {}
            b'-' | b'*' | b'_' => {
                if marker == 0 {
                    marker = byte;
                }
                if byte != marker {
                    return false;
                }
                count += 1;
            }
            _ => return false,
        }
    }
    count >= 3
}

fn is_list_marker(text: &str) -> Option<u8> {
    let marker = text.as_bytes().first().copied()?;
    if matches!(marker, b'-' | b'*' | b'+') && is_item_start(text, marker) {
        Some(marker)
    } else {
        None
    }
}

fn is_item_start(text: &str, marker: u8) -> bool {
    let bytes = text.as_bytes();
    bytes.first() == Some(&marker) && bytes.len() > 1 && matches!(bytes[1], b' ' | b'\t')
}

fn is_ordered_list_marker(text: &str) -> Option<(usize, u8)> {
    let bytes = text.as_bytes();
    if bytes.is_empty() || !bytes[0].is_ascii_digit() {
        return None;
    }
    let mut index = 0;
    while index < bytes.len() && bytes[index].is_ascii_digit() {
        index += 1;
    }
    if index == 0 || index >= bytes.len() || index > 9 {
        return None;
    }
    let delimiter = bytes[index];
    if !matches!(delimiter, b'.' | b')')
        || index + 1 >= bytes.len()
        || !matches!(bytes[index + 1], b' ' | b'\t')
    {
        return None;
    }
    let start = std::str::from_utf8(&bytes[..index])
        .ok()?
        .parse::<usize>()
        .ok()?;
    Some((start, delimiter))
}
