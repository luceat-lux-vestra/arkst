use arkst_markdown::{parse_with_mode, Mode};
use rushdown::text::{Index, Segment};

#[test]
fn valid_index_ranges_are_checked_before_safe_accessors() {
    let source = "한글😀";
    for (start, stop) in [(0, 0), (0, 3), (3, 6), (6, source.len())] {
        let index = Index::new(start, stop);
        assert_eq!(
            index.bytes(source),
            source.get(start..stop).unwrap().as_bytes()
        );
        assert_eq!(index.str(source), source.get(start..stop).unwrap());
    }
}

#[test]
fn valid_segment_ranges_are_checked_before_safe_accessors() {
    let source = "CJK 界 😀";
    for (start, stop) in [(0, 0), (0, 4), (4, 8), (8, source.len())] {
        let segment = Segment::new(start, stop);
        assert_eq!(
            segment.bytes(source),
            source.get(start..stop).unwrap().as_bytes()
        );
        assert_eq!(segment.str(source), source.get(start..stop).unwrap());
    }
}

#[test]
fn parser_output_is_the_only_source_range_surface() {
    for source in [
        "한글 **강조**\n",
        ".align {center}\n    😀 **본문**\r\n",
        "> - [링크](https://example.com)\n",
        "```text\n.foo {bar}\n```\n",
    ] {
        let output = parse_with_mode(source, Mode::Quarkdown);
        assert!(output
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != "E9002"));
    }
}
