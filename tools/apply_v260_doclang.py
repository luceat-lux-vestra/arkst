from __future__ import annotations

import hashlib
import re
import shutil
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
snapshot = Path(sys.argv[1])
raw = snapshot.read_bytes()
expected_sha = "b684f0cd8f6b14537f2857494c967cd73b1a33c9f51938ddcb2fbbcb8525c3a6"
assert hashlib.sha256(raw).hexdigest() == expected_sha
lines = snapshot.read_text(encoding="utf-8").splitlines()
assert len(lines) == 1162
assert sum(line.split("\t", 2)[1] == "1" for line in lines) == 1076
assert sum(line.split("\t", 2)[1] == "0" for line in lines) == 86

data_dir = ROOT / "crates/arkst-engine/data"
data_dir.mkdir(parents=True, exist_ok=True)
shutil.copyfile(snapshot, data_dir / "quarkdown_v260_doclang_snapshot.tsv")


def replace_once(path: str, old: str, new: str) -> None:
    p = ROOT / path
    text = p.read_text(encoding="utf-8")
    count = text.count(old)
    assert count == 1, (path, count, old[:120])
    p.write_text(text.replace(old, new, 1), encoding="utf-8")


def regex_once(path: str, pattern: str, replacement: str) -> None:
    p = ROOT / path
    text = p.read_text(encoding="utf-8")
    updated, count = re.subn(pattern, replacement, text, count=1, flags=re.S)
    assert count == 1, (path, pattern)
    p.write_text(updated, encoding="utf-8")


locale_old = '''pub(crate) fn resolve(identifier: &str) -> Option<IrDocumentLocale> {
    resolve_detailed(identifier).map(|locale| IrDocumentLocale {
        tag: locale.tag,
        localized_name: locale.localized_name,
    })
}
'''
locale_new = locale_old + r'''

// Quarkdown v2.6.0 changed `.doclang` from the v2.5.1 host-JVM locale
// surface to an observable release-specific contract: only known locales are
// accepted and the getter returns the English display name.  This snapshot is
// clean-room black-box data from the official v2.6.0 Linux distribution
// (`quarkdown-linux-x64.zip`, SHA-256
// 5b015e47c820d06ff6774eb700e60d77bc575819d4a0140cc7f1a115e7ce6dc4).
// Its own SHA-256 is
// b684f0cd8f6b14537f2857494c967cd73b1a33c9f51938ddcb2fbbcb8525c3a6.
const DOCLANG_V260_SNAPSHOT: &str =
    include_str!("../data/quarkdown_v260_doclang_snapshot.tsv");

// Modern Java locale parsing canonicalizes these deprecated language codes,
// while Quarkdown v2.6.0 still accepts the suffix-preserving legacy spellings.
// The closure was independently black-box checked for all nine forms.
const DOCLANG_V260_LEGACY_ALIASES: &[(&str, &str)] = &[
    ("in", "Indonesian"),
    ("in-ID", "Indonesian (Indonesia)"),
    ("in-Latn-ID", "Indonesian (Indonesia)"),
    ("iw", "Hebrew"),
    ("iw-Hebr-IL", "Hebrew (Israel)"),
    ("iw-IL", "Hebrew (Israel)"),
    ("ji", "Yiddish"),
    ("ji-Hebr-UA", "Yiddish (Ukraine)"),
    ("ji-UA", "Yiddish (Ukraine)"),
];

fn doclang_v260_snapshot_row(line: &str) -> Option<(&str, bool, &str)> {
    let mut fields = line.splitn(3, '\t');
    let tag = fields.next()?;
    let accepted = match fields.next()? {
        "1" => true,
        "0" => false,
        _ => return None,
    };
    let english_name = fields.next().unwrap_or_default();
    Some((tag, accepted, english_name))
}

/// Resolves the Quarkdown v2.6.0 `.doclang` contract without consulting the
/// host locale database. Name matching remains name-first and
/// case-insensitive; tag matching is ASCII case-insensitive. Rejected and
/// unknown tags fail closed instead of falling through to synthesized BCP 47
/// locales.
pub(crate) fn resolve_doclang_v260(identifier: &str) -> Option<IrDocumentLocale> {
    if let Some((tag, english_name)) = DOCLANG_V260_SNAPSHOT.lines().find_map(|line| {
        let (tag, accepted, english_name) = doclang_v260_snapshot_row(line)?;
        (accepted && string_equals_ignore_case(english_name, identifier))
            .then_some((tag, english_name))
    }) {
        // Preserve the established locale-state model when the older reference
        // snapshot recognizes this English name. If the English spelling is
        // new in v2.6.0, resolve through one accepted tag from the v2.6 oracle.
        let mut locale = LOCALE_NAME_RECORDS
            .iter()
            .find(|record| string_equals_ignore_case(record.display_name, identifier))
            .map(resolved_from_name_record)
            .map(|locale| IrDocumentLocale {
                tag: locale.tag,
                localized_name: locale.localized_name,
            })
            .or_else(|| resolve(tag))?;
        locale.localized_name = english_name.to_string();
        return Some(locale);
    }

    for line in DOCLANG_V260_SNAPSHOT.lines() {
        let (tag, accepted, english_name) = doclang_v260_snapshot_row(line)?;
        if tag.eq_ignore_ascii_case(identifier) {
            if !accepted {
                return None;
            }
            let mut locale = resolve(identifier)?;
            locale.localized_name = english_name.to_string();
            return Some(locale);
        }
    }

    if let Some((alias, english_name)) = DOCLANG_V260_LEGACY_ALIASES
        .iter()
        .find(|(alias, _)| alias.eq_ignore_ascii_case(identifier))
    {
        let mut locale = resolve(alias)?;
        locale.localized_name = (*english_name).to_string();
        return Some(locale);
    }

    None
}
'''
replace_once("crates/arkst-engine/src/locale.rs", locale_old, locale_new)

replace_once(
    "crates/arkst-engine/src/evaluator.rs",
    "let Some(locale) = crate::locale::resolve(&identifier) else {",
    "let Some(locale) = crate::locale::resolve_doclang_v260(&identifier) else {",
)

replace_once(
    "crates/arkst-ir/src/lib.rs",
    "    /// Localized name returned by the `.doclang` getter.\n    pub localized_name: String,",
    "    /// Display name returned by the `.doclang` getter. The serialized field name is\n    /// retained for IR wire compatibility; Quarkdown v2.6.0 returns this value in English.\n    pub localized_name: String,",
)

core = ROOT / "crates/arkst-core/src/lib.rs"
text = core.read_text(encoding="utf-8")
pattern = r'''    #\[test\]\n    fn doclang_resolves_reference_jdk_locale_shapes_and_aliases\(\) \{.*?\n    \}\n\n    #\[test\]\n    fn doclang_preserves_localized_name_and_replaces_previous_locale\(\) \{'''
replacement = r'''    #[test]
    fn doclang_resolves_v260_locale_shapes_and_aliases() {
        let source = concat!(
            ".doclang {Spanish}\n.doclang\n",
            ".doclang {es-MX}\n.doclang\n",
            ".doclang {zh-Hant-TW}\n.doclang\n",
            ".doclang {sr-Cyrl-RS}\n.doclang\n",
            ".doclang {iw}\n.doclang\n",
            ".doclang {in}\n.doclang\n",
            ".doclang {ji}\n.doclang\n",
        );
        let (result, _) = compile_source(source);
        assert!(result.diagnostics.is_empty(), "{result:?}");
        assert_eq!(
            output_text(&result),
            "Spanish\nSpanish (Mexico)\nChinese (Taiwan)\nSerbian (Serbia)\nHebrew\nIndonesian\nYiddish"
        );
        assert_eq!(
            result.ir.metadata.document_state.locale,
            Some(crate::ir::IrDocumentLocale {
                tag: "yi".to_string(),
                localized_name: "Yiddish".to_string(),
            })
        );
    }

    #[test]
    fn doclang_preserves_v260_english_name_and_replaces_previous_locale() {'''
text, count = re.subn(pattern, replacement, text, count=1, flags=re.S)
assert count == 1
text = text.replace('assert_eq!(output_text(&result), "italiano\\nfrançais (Canada)");', 'assert_eq!(output_text(&result), "Italian\\nFrench (Canada)");', 1)
text = text.replace('localized_name: "français (Canada)".to_string(),', 'localized_name: "French (Canada)".to_string(),', 1)
text = text.replace('assert_eq!(output_text(&result), "italiano");', 'assert_eq!(output_text(&result), "Italian");', 1)
root_pattern = r'''    #\[test\]\n    fn doclang_preserves_jdk_root_results_for_blank_or_legacy_inputs\(\) \{.*?\n    \}\n\n    #\[test\]\n    fn doclang_block_body_uses_raw_target_conversion_before_nested_mutations\(\) \{'''
root_replacement = r'''    #[test]
    fn doclang_v260_rejects_unavailable_and_jdk_only_identifiers_atomically() {
        for invalid in [
            "xx-YY",
            "en_US",
            "   ",
            "English (United States, Computer)",
        ] {
            let source = format!(".doclang {{en}}\n.doclang {{{invalid}}}\n.doclang\n");
            let (result, source_id) = compile_source(&source);
            assert_eq!(result.diagnostics.len(), 1, "{invalid:?}: {result:?}");
            assert_eq!(
                result.diagnostics[0].primary.map(|span| span.source_id),
                Some(source_id),
                "{invalid:?}: {result:?}"
            );
            assert_eq!(output_text(&result), "English", "{invalid:?}: {result:?}");
            assert_eq!(
                result.ir.metadata.document_state.locale,
                Some(crate::ir::IrDocumentLocale {
                    tag: "en".to_string(),
                    localized_name: "English".to_string(),
                }),
                "{invalid:?}: {result:?}"
            );
        }
    }

    #[test]
    fn doclang_block_body_uses_raw_target_conversion_before_nested_mutations() {'''
text, count = re.subn(root_pattern, root_replacement, text, count=1, flags=re.S)
assert count == 1
core.write_text(text, encoding="utf-8")

# Add exhaustive v2.6 snapshot checks without disturbing the older locale model tests.
locale = ROOT / "crates/arkst-engine/src/locale.rs"
text = locale.read_text(encoding="utf-8")
assert "mod doclang_v260_snapshot_tests" not in text
text += r'''

#[cfg(test)]
mod doclang_v260_snapshot_tests {
    use super::*;

    #[test]
    fn snapshot_rows_match_the_v260_black_box_contract() {
        let mut rows = 0usize;
        let mut accepted = 0usize;
        let mut rejected = 0usize;
        for line in DOCLANG_V260_SNAPSHOT.lines() {
            let (tag, expected_acceptance, english_name) =
                doclang_v260_snapshot_row(line).expect("valid v2.6 doclang snapshot row");
            rows += 1;
            let resolved = resolve_doclang_v260(tag);
            if expected_acceptance {
                accepted += 1;
                let resolved = resolved.unwrap_or_else(|| panic!("v2.6 accepted tag {tag}"));
                assert_eq!(resolved.localized_name, english_name, "{tag}");
            } else {
                rejected += 1;
                assert!(resolved.is_none(), "v2.6 rejected tag {tag}");
            }
        }
        assert_eq!(rows, 1162);
        assert_eq!(accepted, 1076);
        assert_eq!(rejected, 86);
    }

    #[test]
    fn accepted_v260_english_names_are_name_first_and_case_insensitive() {
        for line in DOCLANG_V260_SNAPSHOT.lines() {
            let (_tag, accepted, english_name) =
                doclang_v260_snapshot_row(line).expect("valid v2.6 doclang snapshot row");
            if !accepted || english_name.is_empty() {
                continue;
            }
            let resolved = resolve_doclang_v260(english_name)
                .unwrap_or_else(|| panic!("v2.6 English name {english_name}"));
            assert_eq!(resolved.localized_name, english_name, "{english_name}");
        }
        assert_eq!(
            resolve_doclang_v260("gErMaN")
                .expect("case-insensitive English name")
                .localized_name,
            "German"
        );
    }

    #[test]
    fn v260_legacy_alias_closure_is_supported() {
        for (alias, english_name) in DOCLANG_V260_LEGACY_ALIASES {
            let resolved = resolve_doclang_v260(alias)
                .unwrap_or_else(|| panic!("v2.6 legacy alias {alias}"));
            assert_eq!(resolved.localized_name, *english_name, "{alias}");
        }
    }
}
'''
locale.write_text(text, encoding="utf-8")

# Keep fixture spans stable by replacing v2.5-only identifiers with accepted
# v2.6 identifiers of identical byte length.
replace_once(
    "fixtures/quarkdown-conformance/cases/doclang-locale-closure/input.qd",
    ".doclang {English (United States, Computer)}",
    ".doclang {ja-JP-u-ca-japanese-x-lvariant-JP}",
)
replace_once(
    "fixtures/quarkdown-conformance/cases/doclang-locale-closure/input.qd",
    ".doclang {xx-YY}\nThe valid unavailable tag is now .doclang",
    ".doclang {de-DE}\nThe accepted regional tag is now .doclang",
)

family = ROOT / "fixtures/quarkdown-conformance/cases/doclang-family/expected/ir.json"
text = family.read_text(encoding="utf-8").replace('"italiano"', '"Italian"')
family.write_text(text, encoding="utf-8")

closure = ROOT / "fixtures/quarkdown-conformance/cases/doclang-locale-closure/expected/ir.json"
text = closure.read_text(encoding="utf-8")
for old, new in [
    ('"español"', '"Spanish"'),
    ('"español (México)"', '"Spanish (Mexico)"'),
    ('"中文 (繁體，台灣)"', '"Chinese (Taiwan)"'),
    ('"српски (ћирилица, Србија)"', '"Serbian (Serbia)"'),
    ('"English (United States, Computer)"', '"Japanese (Japan)"'),
    ('"עברית"', '"Hebrew"'),
    ('"The valid unavailable tag is now "', '"The accepted regional tag is now "'),
    ('"xx (YY)"', '"German (Germany)"'),
    ('"tag": "xx-YY"', '"tag": "de-DE"'),
]:
    assert old in text, old
    text = text.replace(old, new)
closure.write_text(text, encoding="utf-8")

for case in [
    ROOT / "fixtures/quarkdown-conformance/cases/doclang-family/case.toml",
    ROOT / "fixtures/quarkdown-conformance/cases/doclang-locale-closure/case.toml",
]:
    text = case.read_text(encoding="utf-8")
    text = text.replace("quarkdown-v2.5.1-doclang", "quarkdown-v2.6.0-doclang")
    text = text.replace("Final localized", "Final English")
    text = text.replace("JDK locale snapshot coverage", "Quarkdown v2.6.0 locale snapshot coverage")
    text = text.replace("unavailable-tag resolution", "accepted/rejected locale resolution")
    case.write_text(text, encoding="utf-8")

print("v2.6 doclang product patch staged")
