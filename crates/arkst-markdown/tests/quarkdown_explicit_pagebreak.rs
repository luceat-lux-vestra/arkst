use arkst_markdown::{parse_md, parse_qd, Block};

fn page_break_spans(blocks: &[Block]) -> Vec<(usize, usize)> {
    blocks
        .iter()
        .filter_map(|block| match block {
            Block::PageBreak { span } => Some((span.start, span.end)),
            _ => None,
        })
        .collect()
}

#[test]
fn triple_angle_is_quarkdown_only_and_preserves_source_span() {
    let qd = parse_qd("FIRST\n\n<<<\n\nSECOND\n");
    assert_eq!(page_break_spans(&qd.nodes), vec![(7, 10)]);

    let md = parse_md("FIRST\n\n<<<\n\nSECOND\n");
    assert!(
        page_break_spans(&md.nodes).is_empty(),
        "plain Markdown mode must not acquire Quarkdown explicit-break semantics"
    );
}

#[test]
fn consecutive_triple_angle_boundaries_remain_distinct_semantic_breaks() {
    let qd = parse_qd("FIRST\n\n<<<\n<<<\n\nSECOND\n");
    assert_eq!(page_break_spans(&qd.nodes), vec![(7, 10), (11, 14)]);
}

#[test]
fn code_fence_shields_triple_angle_from_pagebreak_recognition() {
    let qd = parse_qd("```text\n<<<\n```\n");
    assert!(page_break_spans(&qd.nodes).is_empty());
    assert!(matches!(qd.nodes.as_slice(), [Block::CodeBlock { .. }]));
}

#[test]
fn thematic_break_remains_thematic_in_quarkdown_mode() {
    let qd = parse_qd("---\n");
    assert!(page_break_spans(&qd.nodes).is_empty());
    assert!(matches!(qd.nodes.as_slice(), [Block::ThematicBreak { .. }]));
}
