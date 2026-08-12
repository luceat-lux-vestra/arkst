use rushdown::ast::{Arena, KindData, LinkKind, NodeRef, TypeData};
use rushdown::parser::{Options, Parser};
use rushdown::text::{BasicReader, Index, MultilineValue, Segment, Value};

fn parse(source: &str) -> (Arena, NodeRef) {
    let parser = Parser::with_options(Options::default());
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

fn check_bounds(label: &str, start: usize, stop: usize, source: &str) {
    assert!(start <= stop, "{label}: reversed range {start}..{stop}");
    assert!(
        stop <= source.len(),
        "{label}: range {start}..{stop} exceeds {} bytes",
        source.len()
    );
    assert!(
        source.is_char_boundary(start),
        "{label}: start {start} is not a UTF-8 boundary"
    );
    assert!(
        source.is_char_boundary(stop),
        "{label}: stop {stop} is not a UTF-8 boundary"
    );
}

fn check_index(label: &str, index: &Index, source: &str) {
    check_bounds(label, index.start(), index.stop(), source);
    let _ = index.bytes(source);
    let _ = index.str(source);
}

fn check_segment(label: &str, segment: &Segment, source: &str) {
    check_bounds(label, segment.start(), segment.stop(), source);
    let _ = segment.bytes(source);
    let _ = segment.str(source);
}

fn check_value(label: &str, value: &Value, source: &str) {
    if let Value::Index(index) = value {
        check_index(label, index, source);
    }
    let _ = value.bytes(source);
    let _ = value.str(source);
}

fn check_multiline(label: &str, value: &MultilineValue, source: &str) {
    let _ = (label, value.str(source));
}

fn check_link_kind(label: &str, kind: &LinkKind, source: &str) {
    match kind {
        LinkKind::Inline => {}
        LinkKind::Reference(reference) => check_multiline(label, reference.value(), source),
        LinkKind::Auto(auto) => check_value(label, auto.text(), source),
        _ => {}
    }
}

fn check_node_values(label: &str, kind: &KindData, source: &str) {
    match kind {
        KindData::CodeBlock(block) => {
            if let Some(info) = block.info() {
                check_value(label, info, source);
            }
            for line in block.value().iter(source) {
                let _ = line;
            }
        }
        KindData::HtmlBlock(block) => {
            for line in block.value().iter(source) {
                let _ = line;
            }
        }
        KindData::LinkReferenceDefinition(definition) => {
            check_multiline(label, definition.label(), source);
            check_value(label, definition.destination(), source);
            if let Some(title) = definition.title() {
                check_multiline(label, title, source);
            }
        }
        KindData::Text(text) => {
            if let Some(index) = text.index() {
                check_index(label, index, source);
            }
            let _ = (text.bytes(source), text.str(source));
        }
        KindData::CodeSpan(code_span) => {
            let _ = code_span.str(source);
        }
        KindData::Link(link) => {
            check_value(label, link.destination(), source);
            let _ = link.destination_str(source);
            if let Some(title) = link.title() {
                check_multiline(label, title, source);
            }
            let _ = link.title_str(source);
            check_link_kind(label, link.link_kind(), source);
        }
        KindData::Image(image) => {
            check_value(label, image.destination(), source);
            let _ = image.destination_str(source);
            if let Some(title) = image.title() {
                check_multiline(label, title, source);
            }
            let _ = image.title_str(source);
            check_link_kind(label, image.link_kind(), source);
        }
        KindData::RawHtml(html) => {
            let _ = (html.bytes(source), html.str(source));
            check_multiline(label, html.value(), source);
        }
        _ => {}
    }
}

fn validate_parser_ranges(source: &str) -> usize {
    let (arena, root) = parse(source);
    let mut range_count = 0;

    for (node_number, node_ref) in walk(&arena, root).into_iter().enumerate() {
        let node = &arena[node_ref];
        if let TypeData::Block(block) = node.type_data() {
            for (segment_number, segment) in block.source().iter().enumerate() {
                let label = format!("node {node_number} block segment {segment_number}");
                check_segment(&label, segment, source);
                range_count += 1;
            }
        }
        check_node_values(
            &format!("node {node_number} {}", node.kind_data().kind_name()),
            node.kind_data(),
            source,
        );

        let mut pretty = String::new();
        node.kind_data()
            .pretty_print(&mut pretty, source, 0)
            .expect("Rushdown pretty-printer should accept a parsed node");
    }

    range_count
}

fn corpus() -> Vec<&'static str> {
    vec![
        "",
        "x",
        "한글",
        "a\r\nb\r\n",
        "*한* **字** `emoji 🦀`",
        "e\u{301} *variation \u{2764}\u{fe0f}* 👩\u{200d}💻",
        "[링크](https://example.com \"제목\") ![이미지](img.png)",
        "<https://example.com> <tag>raw</tag>",
        "```rust\nfn main() { println!(\"한글\"); }\n```\n\n    indented 🦀",
        "> 인용\n> - [ ] task\n>   - nested\n\n| a | b |\n| - | - |\n| c | d |\n",
        ".align {center}\n    Body **Markdown**\n.text {빨강}",
    ]
}

fn main() {
    let mut total = 0;
    for source in corpus() {
        total += validate_parser_ranges(source);
    }
    println!(
        "validated {total} parser-produced source ranges across {} corpus inputs",
        corpus().len()
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn valid_index_cases() {
        for source in ["ASCII", "한글", "🦀", "e\u{301}", ""] {
            let index = Index::new(0, source.len());
            check_index("valid index", &index, source);
            assert_eq!(index.with_start(0).str(source), source);
            assert_eq!(index.with_stop(source.len()).str(source), source);
        }
    }

    #[test]
    fn valid_segment_cases() {
        for source in ["ASCII", "한글", "🦀", "e\u{301}", ""] {
            let segment = Segment::new(0, source.len());
            check_segment("valid segment", &segment, source);
            assert_eq!(segment.with_start(0).str(source), source);
            assert_eq!(segment.with_stop(source.len()).str(source), source);
        }
    }

    #[test]
    fn adversarial_corpus_has_valid_parser_ranges() {
        for source in corpus() {
            validate_parser_ranges(source);
        }
    }

    proptest! {
        #[test]
        fn arbitrary_valid_utf8_has_valid_parser_ranges(chars in prop::collection::vec(any::<char>(), 0..64)) {
            let source: String = chars.into_iter().collect();
            let _ = validate_parser_ranges(&source);
        }
    }

    #[cfg(feature = "invalid-cases")]
    #[test]
    fn index_invalid_start_boundary() {
        let source = "한글";
        let value = Index::new(1, source.len()).str(source);
        assert!(std::str::from_utf8(value.as_bytes()).is_err());
    }

    #[cfg(feature = "invalid-cases")]
    #[test]
    fn index_invalid_stop_boundary() {
        let source = "한글";
        let value = Index::new(0, 1).str(source);
        assert!(std::str::from_utf8(value.as_bytes()).is_err());
    }

    #[cfg(feature = "invalid-cases")]
    #[test]
    fn index_stop_out_of_bounds() {
        let source = "한글";
        let _ = Index::new(0, source.len() + 1).str(source);
    }

    #[cfg(feature = "invalid-cases")]
    #[test]
    fn index_start_out_of_bounds() {
        let source = "한글";
        let _ = Index::new(source.len() + 1, source.len() + 1).str(source);
    }

    #[cfg(feature = "invalid-cases")]
    #[test]
    fn index_reversed_range() {
        let source = "한글";
        let _ = Index::new(4, 2).str(source);
    }

    #[cfg(feature = "invalid-cases")]
    #[test]
    fn segment_invalid_start_boundary() {
        let source = "한글";
        let value = Segment::new(1, source.len()).str(source);
        assert!(std::str::from_utf8(value.as_bytes()).is_err());
    }

    #[cfg(feature = "invalid-cases")]
    #[test]
    fn segment_invalid_stop_boundary() {
        let source = "한글";
        let value = Segment::new(0, 1).str(source);
        assert!(std::str::from_utf8(value.as_bytes()).is_err());
    }

    #[cfg(feature = "invalid-cases")]
    #[test]
    fn segment_stop_out_of_bounds() {
        let source = "한글";
        let _ = Segment::new(0, source.len() + 1).str(source);
    }

    #[cfg(feature = "invalid-cases")]
    #[test]
    fn segment_start_out_of_bounds() {
        let source = "한글";
        let _ = Segment::new(source.len() + 1, source.len() + 1).str(source);
    }

    #[cfg(feature = "invalid-cases")]
    #[test]
    fn segment_reversed_range() {
        let source = "한글";
        let _ = Segment::new(4, 2).str(source);
    }
}
