//! Independent semantic witnesses for the bounded #196 localization slice.

use arkst_core::ir::{IrInline, IrNode};
use arkst_core::{compile, CompileOptions, SourceId, SourceSpan, VirtualProjectBuilder};

fn compile_source(source: &str) -> arkst_core::CompileResult {
    compile_source_with_id(source).0
}

fn compile_source_with_id(source: &str) -> (arkst_core::CompileResult, SourceId) {
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("valid entry")
        .add_source("main.qd", source)
        .expect("valid source")
        .build()
        .expect("valid project");
    let source_id = project
        .sources()
        .get_id(project.entry())
        .expect("entry source id");
    (compile(&project, &CompileOptions::default()), source_id)
}

fn output_text(result: &arkst_core::CompileResult) -> String {
    result
        .ir
        .nodes
        .iter()
        .filter_map(|node| match node {
            IrNode::Paragraph { content, .. } => Some(
                content
                    .iter()
                    .filter_map(|inline| match inline {
                        IrInline::Text { content, .. } => Some(content.as_str()),
                        _ => None,
                    })
                    .collect::<String>(),
            ),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn localization_binds_body_and_resolves_canonical_locales() {
    let source = r#".doclang {English}
.localization {ui}
    - English
      - greeting: Hello
      - nested/key: Keep separator
    - Italian
      - greeting: Ciao
.localize {ui:greeting}
.doclang {ITALIAN}
.localize {ui:greeting}
.doclang {English}
.localize {ui/nested/key} separator:{/}
"#;
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(output_text(&result), "Hello\nCiao\nKeep separator");
}

#[test]
fn localization_accepts_named_name_and_explicit_typed_dictionary() {
    let source = r#".var {entries}
    .dictionary
        - en
          - greeting: Hello
.localization name:{ui} contents:{.entries}
.doclang {en}
.localize {ui:greeting}
"#;
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(output_text(&result), "Hello");
}

#[test]
fn localization_merge_true_registers_an_absent_table() {
    let source = r#".var {entries}
    .dictionary
        - en
          - greeting: Hello
.localization name:{ui} merge:{true} contents:{.entries}
.doclang {en}
.localize {ui:greeting}
"#;
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(output_text(&result), "Hello");
}

#[test]
fn localization_merge_adds_a_locale_and_explicit_false_keeps_duplicate_atomic() {
    let source = r#".doclang {en}
.localization {ui}
    - en
      - greeting: Hello
.localization {ui} merge:{false}
    - en
      - greeting: Rejected
.localization {ui} merge:{true}
    - French
      - greeting: Bonjour
.localize {ui:greeting}
.doclang {fr}
.localize {ui:greeting}
"#;
    let result = compile_source(source);
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert_eq!(output_text(&result), "Hello\nBonjour");
}

#[test]
fn localization_merge_overrides_conflicts_and_preserves_seeded_entries() {
    let source = r#".doclang {en}
.localize {std:warning}
.localization {std} merge:{true}
    - en
      - warning: Custom warning
      - custom: Added
.localize {std:warning}
.localize {std:error}
.localize {std:custom}
.localization {std}
.localize {std:warning}
"#;
    let result = compile_source(source);
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert_eq!(
        output_text(&result),
        "Warning\nCustom warning\nError\nAdded\nCustom warning"
    );
}

#[test]
fn seeded_std_table_covers_the_independently_evidenced_public_locales() {
    let cases = [
        ("zh", "警告", "错误"),
        ("en", "Warning", "Error"),
        ("fr", "Attention", "Erreur"),
        ("de", "Warnung", "Fehler"),
        ("it", "Attenzione", "Errore"),
        ("ja", "警告", "エラー"),
        ("pl", "Ostrzeżenie", "Błąd"),
        ("pt", "Aviso", "Erro"),
        ("ru", "Предупреждение", "Ошибка"),
        ("uk", "Попередження", "Помилка"),
    ];
    let source = cases
        .iter()
        .map(|(locale, _, _)| {
            format!(".doclang {{{locale}}}\n.localize {{std:warning}}\n.localize {{std:error}}\n")
        })
        .collect::<String>();
    let expected = cases
        .iter()
        .flat_map(|(_, warning, error)| [*warning, *error])
        .collect::<Vec<_>>()
        .join("\n");
    let result = compile_source(&source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(output_text(&result), expected);
}

#[test]
fn invalid_localization_candidates_publish_nothing() {
    let source = r#".doclang {en}
.localization {candidate}
    - en
      - keep: lost
    - invalid_locale
      - keep: invalid
.localization {candidate}
    - en
      - keep: recovered
.localize {candidate:keep}
"#;
    let result = compile_source(source);
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert_eq!(output_text(&result), "recovered");

    let source = r#".doclang {en}
.localization {candidate}
    - en
      - keep: stable
.localization {candidate} merge:{true}
    - en
      - keep: changed
      - invalid:
          - nested: value
.localize {candidate:keep}
"#;
    let result = compile_source(source);
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert_eq!(output_text(&result), "stable");
}

#[test]
fn localization_rejects_non_dictionary_locale_values() {
    let source = r#".var {bad}
    .dictionary
        - en: true
.localization name:{candidate} contents:{.bad}
.localization {candidate}
    - en
      - key: recovered
.doclang {en}
.localize {candidate:key}
"#;
    let result = compile_source(source);
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert_eq!(output_text(&result), "recovered");
}

#[test]
fn localization_lookup_fails_closed_without_locale_or_exact_entry() {
    let no_locale = compile_source(".localize {std:warning}\n");
    assert_eq!(no_locale.diagnostics.len(), 1, "{no_locale:?}");
    assert!(output_text(&no_locale).is_empty());

    let missing = compile_source(
        r#".doclang {fr-CA}
.localize {std:warning}
.doclang {en}
.localize {missing:key}
.localize {std:missing}
.localize {std} separator:{}
"#,
    );
    assert_eq!(missing.diagnostics.len(), 4, "{missing:?}");
    assert!(output_text(&missing).is_empty());
}

#[test]
fn localize_rejects_bodies_and_malformed_lookup_keys() {
    let result = compile_source(
        r#".doclang {en}
.localize {std}
.localize {std:warning}
    rejected body
"#,
    );
    assert_eq!(result.diagnostics.len(), 2, "{result:?}");
    assert!(output_text(&result).is_empty());
}

#[test]
fn localization_and_localize_use_fail_closed_shared_binding() {
    let invalid_localization_calls = [
        ".localization\n    - en\n      - key: value\n",
        ".localization {ui}\n",
        ".localization {ui} unknown:{true}\n    - en\n      - key: value\n",
        ".localization {ui} name:{other}\n    - en\n      - key: value\n",
        ".localization {ui} {false} {true} {excess}\n",
        ".localization {ui} contents:{.dictionary}\n    - en\n      - key: value\n",
    ];
    for source in invalid_localization_calls {
        let result = compile_source(source);
        assert!(
            !result.diagnostics.is_empty(),
            "accepted invalid call: {source}"
        );
        assert!(
            output_text(&result).is_empty(),
            "published output: {result:?}"
        );
    }

    let invalid_localize_calls = [
        ".localize\n",
        ".localize {std:warning} unknown:{true}\n",
        ".localize {std:warning} {extra}\n",
        ".localize {std:warning} separator:{/} separator:{:}\n",
        ".localize {std:warning} separator:{:}\n    rejected body\n",
    ];
    for source in invalid_localize_calls {
        let result = compile_source(source);
        assert!(
            !result.diagnostics.is_empty(),
            "accepted invalid call: {source}"
        );
        assert!(
            output_text(&result).is_empty(),
            "published output: {result:?}"
        );
    }
}

#[test]
fn failed_callable_localization_does_not_leak_tables() {
    let source = r#".doclang {en}
.localization {outer}
    - en
      - keep: outer value
.function {bad}
    .localization {inner}
        - en
          - keep: leaked
    .localization {outer} merge:{true}
        - en
          - keep: changed
          - invalid:
              - nested: value
.bad
.localize {outer:keep}
.localize {inner:keep}
"#;
    let result = compile_source(source);
    assert_eq!(result.diagnostics.len(), 2, "{result:?}");
    assert_eq!(output_text(&result), "outer value");
}

#[test]
fn successful_callable_localization_shares_document_state() {
    let source = r#".function {define}
    .localization {inner}
        - French
          - greeting: Bonjour
    .localization {second}
        - French
          - greeting: Deuxieme
.define
.doclang {fr}
.localize {inner:greeting}
.localize {second:greeting}
"#;
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(output_text(&result), "Bonjour\nDeuxieme");
}

#[test]
fn source_defined_localization_names_shadow_native_dispatch() {
    let source = r#".function {localization}
    name:
    shadowed localization
.function {localize}
    key:
    shadowed localize
.localization {native}
.localize {native:key}
"#;
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(
        output_text(&result),
        "shadowed localization\nshadowed localize"
    );
}

// Quarkdown v2.5.1's `BaseContext.localize()` performs
// `entries[key.lowercase()] ?: entries[key]`: the pinned Kotlin/JVM-compatible
// lowercase form of the requested key is tried first, and the original key is
// tried only if that lookup misses. The table name and locale tag stay exact
// (`crate::locale::resolve`/`BTreeMap::get`, unchanged by this slice); only
// the entry-key lookup gains the two-step fallback. This is not general
// case-insensitive lookup -- see
// `localize_key_lookup_is_not_general_case_insensitive` below.
#[test]
fn localize_key_lookup_tries_lowercase_before_original() {
    let lowercase_only = compile_source(
        r#".doclang {en}
.localization {ui}
    - en
      - warning: lower
.localize {ui:WARNING}
"#,
    );
    assert!(lowercase_only.diagnostics.is_empty(), "{lowercase_only:?}");
    assert_eq!(output_text(&lowercase_only), "lower");

    let exact_only = compile_source(
        r#".doclang {en}
.localization {ui}
    - en
      - Warning: exact
.localize {ui:Warning}
"#,
    );
    assert!(exact_only.diagnostics.is_empty(), "{exact_only:?}");
    assert_eq!(output_text(&exact_only), "exact");

    let lowercase_wins = compile_source(
        r#".doclang {en}
.localization {ui}
    - en
      - warning: lower
      - Warning: exact
.localize {ui:Warning}
"#,
    );
    assert!(lowercase_wins.diagnostics.is_empty(), "{lowercase_wins:?}");
    assert_eq!(output_text(&lowercase_wins), "lower");
}

#[test]
fn localize_key_lookup_is_not_general_case_insensitive() {
    // Only "Warning" (not "warning") exists. `warning.lowercase()` == "warning"
    // misses, and the original requested key "warning" also misses "Warning" --
    // so the lookup fails closed instead of silently matching by ignoring case.
    let source = r#".doclang {en}
.localization {ui}
    - en
      - Warning: exact
.localize {ui:warning}
"#;
    let (result, source_id) = compile_source_with_id(source);
    assert_eq!(result.diagnostics.len(), 1, "{result:?}");
    assert!(output_text(&result).is_empty());
    assert_eq!(result.diagnostics[0].code, "E3001");
    assert_eq!(
        result.diagnostics[0].message,
        "Could not find localization key `warning` in table `ui` for locale en"
    );
    let key_start = source.find("{ui:warning}").expect("call span");
    assert_eq!(
        result.diagnostics[0].primary,
        Some(SourceSpan::new(
            source_id,
            key_start,
            key_start + "{ui:warning}".len(),
        ))
    );
}

#[test]
fn localize_key_lowercasing_uses_full_unicode_mapping_not_ascii() {
    // U+0130 (LATIN CAPITAL LETTER I WITH DOT ABOVE) lowercases under the
    // pinned Kotlin/JVM Locale.ROOT full mapping to the two scalars U+0069
    // U+0307 ("i" + combining dot above) -- not the single ASCII "i" a
    // byte-wise/ASCII-only lowering would produce. Independently verified
    // against `"İD".toLowerCase(Locale.ROOT)` == "i̇d" on a local
    // JDK, and sourced from the same pinned Temurin 25 oracle table
    // (`crates/arkst-engine/src/unicode_case.rs`, `FULL_LOWERCASE`) already
    // used by `.capitalize`.
    let source = "\
.doclang {en}
.localization {ui}
    - en
      - i\u{307}d: dotted
.localize {ui:\u{130}D}
";
    let result = compile_source(source);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert_eq!(output_text(&result), "dotted");
}

#[test]
fn localize_missing_table_and_key_diagnostics_are_source_backed() {
    let missing_table_source = ".doclang {en}\n.localize {missing:key}\n";
    let (missing_table, missing_table_id) = compile_source_with_id(missing_table_source);
    assert_eq!(missing_table.diagnostics.len(), 1, "{missing_table:?}");
    assert_eq!(missing_table.diagnostics[0].code, "E3001");
    assert_eq!(
        missing_table.diagnostics[0].message,
        "Could not find localization table `missing`"
    );
    let key_start = missing_table_source
        .find("{missing:key}")
        .expect("call span");
    assert_eq!(
        missing_table.diagnostics[0].primary,
        Some(SourceSpan::new(
            missing_table_id,
            key_start,
            key_start + "{missing:key}".len(),
        ))
    );

    let missing_key_source =
        ".doclang {en}\n.localization {ui}\n    - en\n      - key: value\n.localize {ui:absent}\n";
    let (missing_key, missing_key_id) = compile_source_with_id(missing_key_source);
    assert_eq!(missing_key.diagnostics.len(), 1, "{missing_key:?}");
    assert_eq!(missing_key.diagnostics[0].code, "E3001");
    assert_eq!(
        missing_key.diagnostics[0].message,
        "Could not find localization key `absent` in table `ui` for locale en"
    );
    let key_start = missing_key_source.find("{ui:absent}").expect("call span");
    assert_eq!(
        missing_key.diagnostics[0].primary,
        Some(SourceSpan::new(
            missing_key_id,
            key_start,
            key_start + "{ui:absent}".len(),
        ))
    );
}

#[test]
fn localize_binder_failures_are_source_backed() {
    let unknown_source = ".doclang {en}\n.localize {std:warning} unknown:{true}\n";
    let (unknown, unknown_id) = compile_source_with_id(unknown_source);
    assert_eq!(unknown.diagnostics.len(), 1, "{unknown:?}");
    assert_eq!(unknown.diagnostics[0].code, "E3001");
    assert_eq!(
        unknown.diagnostics[0].message,
        "`.localize` does not support named argument `unknown`"
    );
    let name_start = unknown_source.find("unknown:").expect("argument name");
    assert_eq!(
        unknown.diagnostics[0].primary,
        Some(SourceSpan::new(
            unknown_id,
            name_start,
            name_start + "unknown".len(),
        ))
    );

    let duplicate_source = ".doclang {en}\n.localize {std:warning} separator:{/} separator:{:}\n";
    let (duplicate, duplicate_id) = compile_source_with_id(duplicate_source);
    assert_eq!(duplicate.diagnostics.len(), 1, "{duplicate:?}");
    assert_eq!(duplicate.diagnostics[0].code, "E3001");
    assert_eq!(
        duplicate.diagnostics[0].message,
        "`.localize` received the `separator` argument more than once"
    );
    let second_name_start = duplicate_source
        .rfind("separator")
        .expect("second occurrence");
    assert_eq!(
        duplicate.diagnostics[0].primary,
        Some(SourceSpan::new(
            duplicate_id,
            second_name_start,
            second_name_start + "separator".len(),
        ))
    );
    let first_argument_start = duplicate_source
        .find("separator:{/}")
        .expect("first occurrence");
    assert_eq!(
        duplicate.diagnostics[0].secondary,
        vec![SourceSpan::new(
            duplicate_id,
            first_argument_start,
            first_argument_start + "separator:{/}".len(),
        )]
    );
}

#[test]
fn localization_conversion_failure_is_source_backed() {
    // A nested Dictionary is not convertible to the `contents` entry's typed
    // String target; the whole `.localization` candidate is rejected before
    // any state is published (see `localization_lookup_fails_closed...` and
    // `invalid_localization_candidates_publish_nothing` for the surrounding
    // atomicity contract).
    let source = ".doclang {en}\n.localization {ui}\n    - en\n      - keep: kept\n      - invalid:\n          - nested: value\n.localize {ui:keep}\n";
    let (result, source_id) = compile_source_with_id(source);
    assert_eq!(result.diagnostics.len(), 2, "{result:?}");
    assert_eq!(result.diagnostics[0].code, "E3001");
    assert_eq!(
        result.diagnostics[0].message,
        ".localization: unsupported value category for target String for parameter `contents`"
    );
    let candidate_start = source
        .find("- en\n      - keep: kept")
        .expect("candidate span start");
    let candidate = "- en\n      - keep: kept\n      - invalid:\n          - nested: value";
    assert_eq!(
        result.diagnostics[0].primary,
        Some(SourceSpan::new(
            source_id,
            candidate_start,
            candidate_start + candidate.len(),
        ))
    );
    assert_eq!(result.diagnostics[1].code, "E3001");
    assert_eq!(
        result.diagnostics[1].message,
        "Could not find localization table `ui`"
    );
    assert!(output_text(&result).is_empty());
}

#[test]
fn nested_localization_failure_diagnostic_points_at_the_failing_candidate_rolls_back_and_reruns_deterministically(
) {
    let source = r#".doclang {en}
.localization {outer}
    - en
      - keep: outer value
.function {bad}
    .localization {inner}
        - en
          - keep: leaked
    .localization {outer} merge:{true}
        - en
          - keep: changed
          - invalid:
              - nested: value
.bad
.localize {outer:keep}
.localize {inner:keep}
"#;

    let (first, first_id) = compile_source_with_id(source);
    assert_eq!(first.diagnostics.len(), 2, "{first:?}");

    // The causal diagnostic points at the failing merge candidate (the "en"
    // locale entry inside the nested `.localization {outer} merge:{true}`
    // call that carries the non-convertible `invalid` value), not at the
    // outer `.function`/`.bad` call site or an unrelated span.
    assert_eq!(first.diagnostics[0].code, "E3001");
    assert_eq!(
        first.diagnostics[0].message,
        ".localization: unsupported value category for target String for parameter `contents`"
    );
    let candidate =
        "- en\n          - keep: changed\n          - invalid:\n              - nested: value";
    let candidate_start = source.find(candidate).expect("candidate span start");
    assert_eq!(
        first.diagnostics[0].primary,
        Some(SourceSpan::new(
            first_id,
            candidate_start,
            candidate_start + candidate.len(),
        ))
    );

    // `inner` never leaked out of the failed `.function` invocation.
    assert_eq!(first.diagnostics[1].code, "E3001");
    assert_eq!(
        first.diagnostics[1].message,
        "Could not find localization table `inner`"
    );
    let inner_key_start = source.rfind("{inner:keep}").expect("inner call span");
    assert_eq!(
        first.diagnostics[1].primary,
        Some(SourceSpan::new(
            first_id,
            inner_key_start,
            inner_key_start + "{inner:keep}".len(),
        ))
    );

    // Rollback leaves `outer`'s prior state unchanged: the failed `merge:true`
    // candidate did not partially apply, so `keep` is still "outer value".
    assert_eq!(output_text(&first), "outer value");

    // Rerunning the same source produces an equivalent diagnostic
    // classification and provenance (same code/message/primary per
    // diagnostic, in the same order).
    let (second, second_id) = compile_source_with_id(source);
    assert_eq!(second.diagnostics.len(), first.diagnostics.len());
    for (a, b) in first.diagnostics.iter().zip(second.diagnostics.iter()) {
        assert_eq!(a.code, b.code);
        assert_eq!(a.message, b.message);
        assert_eq!(
            a.primary.map(|span| (span.start, span.end)),
            b.primary.map(|span| (span.start, span.end))
        );
    }
    assert_eq!(first_id, second_id);
    assert_eq!(output_text(&second), "outer value");
}
