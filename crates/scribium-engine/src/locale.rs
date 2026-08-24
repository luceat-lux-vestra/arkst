//! Deterministic locale records used by the bounded `.doclang` slice.
//!
//! Quarkdown v2.5.1 delegates locale lookup to the JVM's locale database.
//! Scribium's platform-neutral evaluator cannot do that without making
//! compilation host-dependent, so this slice uses only checked-in,
//! evidence-backed records. An identifier not represented here is rejected;
//! it is not passed through or resolved by the host.

use scribium_ir::IrDocumentLocale;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LocaleRecord {
    tag: &'static str,
    localized_name: &'static str,
    english_names: &'static [&'static str],
}

const ENGLISH: &[&str] = &["English"];
const ENGLISH_UNITED_STATES: &[&str] = &["English (United States)"];
const ITALIAN: &[&str] = &["Italian"];
const GERMAN: &[&str] = &["German"];
const FRENCH: &[&str] = &["French"];
const FRENCH_CANADA: &[&str] = &["French (Canada)"];
const CHINESE: &[&str] = &["Chinese"];
const JAPANESE: &[&str] = &["Japanese"];

// The six base locales are the set named by Quarkdown's public localization
// documentation. The two regional records are the exact v2.5.1 LocaleTest
// examples that establish regional canonical-tag and localized-name behavior.
// This is an explicit bounded compatibility set, not a claim to reproduce a
// JVM/CLDR locale database.
const SUPPORTED_LOCALES: &[LocaleRecord] = &[
    LocaleRecord {
        tag: "en",
        localized_name: "English",
        english_names: ENGLISH,
    },
    LocaleRecord {
        tag: "en-US",
        localized_name: "English (United States)",
        english_names: ENGLISH_UNITED_STATES,
    },
    LocaleRecord {
        tag: "it",
        localized_name: "italiano",
        english_names: ITALIAN,
    },
    LocaleRecord {
        tag: "de",
        localized_name: "Deutsch",
        english_names: GERMAN,
    },
    LocaleRecord {
        tag: "fr",
        localized_name: "français",
        english_names: FRENCH,
    },
    LocaleRecord {
        tag: "fr-CA",
        localized_name: "français (Canada)",
        english_names: FRENCH_CANADA,
    },
    LocaleRecord {
        tag: "zh",
        localized_name: "中文",
        english_names: CHINESE,
    },
    LocaleRecord {
        tag: "ja",
        localized_name: "日本語",
        english_names: JAPANESE,
    },
];

/// Resolves an English locale name before a case-insensitive canonical tag.
pub(crate) fn resolve(identifier: &str) -> Option<IrDocumentLocale> {
    let record = SUPPORTED_LOCALES
        .iter()
        .find(|record| {
            record
                .english_names
                .iter()
                .any(|name| name.eq_ignore_ascii_case(identifier))
        })
        .or_else(|| {
            SUPPORTED_LOCALES
                .iter()
                .find(|record| record.tag.eq_ignore_ascii_case(identifier))
        })?;

    Some(IrDocumentLocale {
        tag: record.tag.to_string(),
        localized_name: record.localized_name.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::resolve;

    #[test]
    fn resolves_pinned_tag_and_name_examples() {
        assert_eq!(resolve("en").unwrap().tag, "en");
        assert_eq!(resolve("English").unwrap().tag, "en");
        assert_eq!(resolve("en-US").unwrap().tag, "en-US");
        assert_eq!(resolve("English (United States)").unwrap().tag, "en-US");
        assert_eq!(resolve("En-us").unwrap().tag, "en-US");
    }

    #[test]
    fn preserves_pinned_localized_names() {
        assert_eq!(resolve("en").unwrap().localized_name, "English");
        assert_eq!(resolve("it").unwrap().localized_name, "italiano");
        assert_eq!(
            resolve("fr-CA").unwrap().localized_name,
            "français (Canada)"
        );
    }

    #[test]
    fn rejects_blank_and_unrepresented_identifiers() {
        assert!(resolve("").is_none());
        assert!(resolve("   ").is_none());
        assert!(resolve("en_US").is_none());
        assert!(resolve("nonexistent").is_none());
    }
}
