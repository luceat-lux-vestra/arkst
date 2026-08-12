fn main() {
    println!("rushdown safe-API safety gate fixture");
}

#[cfg(test)]
mod tests {
    use rushdown::text::{Index, Segment};

    fn assert_valid_range(start: usize, stop: usize, source: &str) {
        assert!(start <= stop);
        assert!(stop <= source.len());
        assert!(source.is_char_boundary(start));
        assert!(source.is_char_boundary(stop));
    }

    #[test]
    fn valid_index_bytes_and_str_ranges() {
        let cases = [
            ("ASCII", 0, 5),
            ("한글", 0, "한글".len()),
            ("字", 0, "字".len()),
            ("🦀", 0, "🦀".len()),
            ("e\u{301}", 0, "e\u{301}".len()),
            ("", 0, 0),
        ];

        for (source, start, stop) in cases {
            assert_valid_range(start, stop, source);
            let index = Index::new(start, stop);
            assert_eq!(index.bytes(source), source.as_bytes());
            assert_eq!(index.str(source), source);
            assert_eq!(index.with_start(start).start(), start);
            assert_eq!(index.with_stop(stop).stop(), stop);
        }
    }

    #[test]
    fn valid_segment_bytes_and_str_ranges() {
        let cases = [
            ("ASCII", 0, 5),
            ("한글", 0, "한글".len()),
            ("字", 0, "字".len()),
            ("🦀", 0, "🦀".len()),
            ("e\u{301}", 0, "e\u{301}".len()),
            ("", 0, 0),
        ];

        for (source, start, stop) in cases {
            assert_valid_range(start, stop, source);
            let segment = Segment::new(start, stop);
            assert_eq!(segment.bytes(source).as_ref(), source.as_bytes());
            assert_eq!(segment.str(source).as_ref(), source);
            assert_eq!(segment.with_start(start).start(), start);
            assert_eq!(segment.with_stop(stop).stop(), stop);
        }
    }

    #[cfg(feature = "invalid-cases")]
    #[test]
    fn index_bytes_interior_boundaries_return_raw_bytes() {
        let source = "한글";
        let start_bytes = Index::new(1, source.len()).bytes(source);
        let stop_bytes = Index::new(0, 1).bytes(source);
        assert!(std::str::from_utf8(start_bytes).is_err());
        assert!(std::str::from_utf8(stop_bytes).is_err());
    }

    #[cfg(feature = "invalid-cases")]
    #[test]
    fn index_bytes_stop_out_of_bounds_panics() {
        let source = "한글";
        let _ = Index::new(0, source.len() + 1).bytes(source);
    }

    #[cfg(feature = "invalid-cases")]
    #[test]
    fn index_bytes_start_out_of_bounds_panics() {
        let source = "한글";
        let _ = Index::new(source.len() + 1, source.len() + 1).bytes(source);
    }

    #[cfg(feature = "invalid-cases")]
    #[test]
    fn index_bytes_reversed_range_panics() {
        let source = "한글";
        let _ = Index::new(4, 2).bytes(source);
    }

    #[cfg(feature = "invalid-cases")]
    #[test]
    fn index_str_interior_boundaries_return_non_utf8_bytes() {
        let source = "한글";
        let start_value = Index::new(1, source.len()).str(source);
        let stop_value = Index::new(0, 1).str(source);
        assert!(std::str::from_utf8(start_value.as_bytes()).is_err());
        assert!(std::str::from_utf8(stop_value.as_bytes()).is_err());
    }

    #[cfg(feature = "invalid-cases")]
    #[test]
    fn index_str_stop_out_of_bounds_is_rejected_by_miri() {
        let source = "한글";
        let _ = Index::new(0, source.len() + 1).str(source);
    }

    #[cfg(feature = "invalid-cases")]
    #[test]
    fn index_str_start_out_of_bounds_is_rejected_by_miri() {
        let source = "한글";
        let _ = Index::new(source.len() + 1, source.len() + 1).str(source);
    }

    #[cfg(feature = "invalid-cases")]
    #[test]
    fn index_str_reversed_range_is_rejected_by_miri() {
        let source = "한글";
        let _ = Index::new(4, 2).str(source);
    }

    #[cfg(feature = "invalid-cases")]
    #[test]
    fn segment_bytes_interior_boundaries_return_raw_bytes() {
        let source = "한글";
        let start_bytes = Segment::new(1, source.len()).bytes(source);
        let stop_bytes = Segment::new(0, 1).bytes(source);
        assert!(std::str::from_utf8(start_bytes.as_ref()).is_err());
        assert!(std::str::from_utf8(stop_bytes.as_ref()).is_err());
    }

    #[cfg(feature = "invalid-cases")]
    #[test]
    fn segment_bytes_stop_out_of_bounds_panics() {
        let source = "한글";
        let _ = Segment::new(0, source.len() + 1).bytes(source);
    }

    #[cfg(feature = "invalid-cases")]
    #[test]
    fn segment_bytes_start_out_of_bounds_panics() {
        let source = "한글";
        let _ = Segment::new(source.len() + 1, source.len() + 1).bytes(source);
    }

    #[cfg(feature = "invalid-cases")]
    #[test]
    fn segment_bytes_reversed_range_panics() {
        let source = "한글";
        let _ = Segment::new(4, 2).bytes(source);
    }

    #[cfg(feature = "invalid-cases")]
    #[test]
    fn segment_str_interior_boundaries_return_non_utf8_bytes() {
        let source = "한글";
        let start_value = Segment::new(1, source.len()).str(source);
        let stop_value = Segment::new(0, 1).str(source);
        assert!(std::str::from_utf8(start_value.as_ref().as_bytes()).is_err());
        assert!(std::str::from_utf8(stop_value.as_ref().as_bytes()).is_err());
    }

    #[cfg(feature = "invalid-cases")]
    #[test]
    fn segment_str_stop_out_of_bounds_is_rejected_by_miri() {
        let source = "한글";
        let _ = Segment::new(0, source.len() + 1).str(source);
    }

    #[cfg(feature = "invalid-cases")]
    #[test]
    fn segment_str_start_out_of_bounds_is_rejected_by_miri() {
        let source = "한글";
        let _ = Segment::new(source.len() + 1, source.len() + 1).str(source);
    }

    #[cfg(feature = "invalid-cases")]
    #[test]
    fn segment_str_reversed_range_is_rejected_by_miri() {
        let source = "한글";
        let _ = Segment::new(4, 2).str(source);
    }
}
