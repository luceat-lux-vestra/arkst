//! Quarkdown v2.6.0-specific `.doclang` compatibility resolver.
//!
//! This module deliberately stays separate from `locale.rs`: that file is the
//! pinned Temurin-25 semantic verifier for the older JVM-observable locale
//! model. v2.6.0 changed the public `.doclang` acceptance/name surface, so its
//! clean-room black-box oracle is owned here instead of changing the JDK25
//! verifier identity.

use crate::locale;
use crate::unicode_case::{simple_lowercase, simple_uppercase};
use arkst_ir::IrDocumentLocale;

// Official Quarkdown v2.6.0 Linux distribution:
// quarkdown-linux-x64.zip
// SHA-256 5b015e47c820d06ff6774eb700e60d77bc575819d4a0140cc7f1a115e7ce6dc4
//
// Independently captured `.doclang` black-box snapshot:
// SHA-256 b684f0cd8f6b14537f2857494c967cd73b1a33c9f51938ddcb2fbbcb8525c3a6
const SNAPSHOT: &str = include_str!("../data/quarkdown_v260_doclang_snapshot.tsv");

const LEGACY_ALIASES: &[(&str, &str)] = &[
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

fn snapshot_row(line: &str) -> Option<(&str, bool, &str)> {
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

/// Resolves only the observable Quarkdown v2.6.0 `.doclang` surface.
///
/// English display names are matched before tags, case-insensitively. For a
/// colliding English name, the existing deterministic JDK25 name-first result
/// is retained only when its canonical tag is one of the v2.6.0 candidates for
/// that exact English name. This prevents an older/JDK-only match from leaking
/// through while preserving already-established deterministic collision
/// ownership. If no compatible old-name winner exists, the first matching
/// v2.6.0 snapshot row is the deterministic candidate.
///
/// Tags are ASCII-case-insensitive and must be explicitly accepted by the
/// v2.6.0 snapshot. Unknown/rejected structurally-valid BCP-47 strings do not
/// fall through to the broader JDK25 synthesizer.
pub(crate) fn resolve(identifier: &str) -> Option<IrDocumentLocale> {
    let mut first_name_match = None;
    let mut old_name_candidate = locale::resolve(identifier);
    for line in SNAPSHOT.lines() {
        let (tag, accepted, english_name) = snapshot_row(line)?;
        if !accepted || !string_equals_ignore_case(english_name, identifier) {
            continue;
        }
        first_name_match.get_or_insert((tag, english_name));
        if old_name_candidate
            .as_ref()
            .is_some_and(|candidate| candidate.tag.eq_ignore_ascii_case(tag))
        {
            let mut locale = old_name_candidate.take()?;
            locale.localized_name = english_name.to_string();
            return Some(locale);
        }
    }
    if let Some((tag, english_name)) = first_name_match {
        let mut locale = locale::resolve(tag)?;
        locale.localized_name = english_name.to_string();
        return Some(locale);
    }

    for line in SNAPSHOT.lines() {
        let (tag, accepted, english_name) = snapshot_row(line)?;
        if tag.eq_ignore_ascii_case(identifier) {
            if !accepted {
                return None;
            }
            let mut locale = locale::resolve(identifier)?;
            locale.localized_name = english_name.to_string();
            return Some(locale);
        }
    }

    if let Some((alias, english_name)) = LEGACY_ALIASES
        .iter()
        .find(|(alias, _)| alias.eq_ignore_ascii_case(identifier))
    {
        let mut locale = locale::resolve(alias)?;
        locale.localized_name = (*english_name).to_string();
        return Some(locale);
    }

    None
}

fn string_equals_ignore_case(left: &str, right: &str) -> bool {
    let mut left_chars = left.chars();
    let mut right_chars = right.chars();
    loop {
        match (left_chars.next(), right_chars.next()) {
            (Some(left), Some(right)) if char_equals_ignore_case(left, right) => {}
            (None, None) => return true,
            _ => return false,
        }
    }
}

fn char_equals_ignore_case(left: char, right: char) -> bool {
    if left == right {
        return true;
    }
    let left_upper = simple_uppercase(left);
    let right_upper = simple_uppercase(right);
    left_upper == right_upper || simple_lowercase(left_upper) == simple_lowercase(right_upper)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_rows_match_the_v260_black_box_contract() {
        let mut rows = 0usize;
        let mut accepted = 0usize;
        let mut rejected = 0usize;
        for line in SNAPSHOT.lines() {
            let (tag, expected_acceptance, english_name) =
                snapshot_row(line).expect("valid v2.6 doclang snapshot row");
            rows += 1;
            let resolved = resolve(tag);
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
        for line in SNAPSHOT.lines() {
            let (_tag, accepted, english_name) =
                snapshot_row(line).expect("valid v2.6 doclang snapshot row");
            if !accepted || english_name.is_empty() {
                continue;
            }
            let resolved =
                resolve(english_name).unwrap_or_else(|| panic!("v2.6 English name {english_name}"));
            assert_eq!(resolved.localized_name, english_name, "{english_name}");
        }
        assert_eq!(
            resolve("gErMaN")
                .expect("case-insensitive English name")
                .localized_name,
            "German"
        );
    }

    #[test]
    fn legacy_alias_closure_is_supported() {
        for (alias, english_name) in LEGACY_ALIASES {
            let resolved = resolve(alias).unwrap_or_else(|| panic!("v2.6 legacy alias {alias}"));
            assert_eq!(resolved.localized_name, *english_name, "{alias}");
        }
    }

    #[test]
    fn rejected_or_unknown_tags_do_not_fall_through_to_jdk25() {
        for identifier in ["xx-YY", "en_US", "   ", "English (United States, Computer)"] {
            assert!(resolve(identifier).is_none(), "{identifier:?}");
        }
    }
}
