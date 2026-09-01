#!/usr/bin/env python3
from pathlib import Path

path = Path('crates/scribium-engine/src/locale.rs')
s = path.read_text()
s = s.replace(
    'use super::{find_tag_record, parse_language_tag, resolve, string_equals_ignore_case};',
    'use super::{display_candidate_profiles, find_tag_record, parse_language_tag, resolve, string_equals_ignore_case};'
)
s = s.replace('fn name_matching_reuses_unicode_13_characterwise_case()', 'fn name_matching_reuses_pinned_jdk25_characterwise_case()')
needle = '''    #[test]\n    fn rejects_root_only_and_malformed_tags() {'''
test = r'''    #[test]
    fn jdk25_locale_oracle_matches_candidate_graph_and_public_resolution() {
        let Ok(path) = std::env::var("SCRIBIUM_JDK25_LOCALE_ORACLE") else {
            return;
        };
        let oracle = std::fs::read_to_string(path).expect("read transient JDK25 locale oracle");
        let mut checked = 0usize;
        let mut candidate_checked = 0usize;
        for line in oracle.lines() {
            let fields: Vec<_> = line.split('\t').collect();
            assert_eq!(fields.len(), 6, "malformed locale oracle row: {line}");
            assert_eq!(fields[0], "locale");
            let request = fields[1];
            let path_kind = fields[2];
            let expected_tag = fields[3];
            let expected_name = fields[4];
            if path_kind == "tag" {
                let expected_candidates = fields[5]
                    .split('|')
                    .map(|value| if value == "<root>" { String::new() } else { value.to_string() })
                    .collect::<Vec<_>>();
                let parsed = parse_language_tag(request)
                    .unwrap_or_else(|| panic!("oracle tag request should be accepted: {request}"));
                assert_eq!(
                    display_candidate_profiles(&parsed),
                    expected_candidates,
                    "candidate graph mismatch for {request}"
                );
                candidate_checked += 1;
            } else {
                assert_eq!(path_kind, "name", "unknown oracle path: {line}");
                assert!(fields[5].is_empty(), "name path must not expose candidates");
            }
            let actual = resolve(request)
                .unwrap_or_else(|| panic!("oracle request should resolve: {request}"));
            assert_eq!(actual.tag, expected_tag, "canonical tag mismatch for {request}");
            assert_eq!(
                actual.localized_name, expected_name,
                "localized name mismatch for {request}"
            );
            checked += 1;
        }
        assert!(checked >= 1_100, "expected broad available-locale oracle coverage");
        assert!(candidate_checked >= 1_000, "expected broad tag-path candidate coverage");
    }

'''
if needle not in s:
    raise SystemExit('test insertion point not found')
s = s.replace(needle, test + needle, 1)
path.write_text(s)
