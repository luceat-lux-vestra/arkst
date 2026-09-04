//! Independent semantic witnesses for the bounded #196 localization slice.

use arkst_core::ir::{IrInline, IrNode};
use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};

fn compile_source(source: &str) -> arkst_core::CompileResult {
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("valid entry")
        .add_source("main.qd", source)
        .expect("valid source")
        .build()
        .expect("valid project");
    compile(&project, &CompileOptions::default())
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
