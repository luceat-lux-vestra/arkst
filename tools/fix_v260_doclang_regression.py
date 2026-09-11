from pathlib import Path
import re

path = Path("crates/arkst-core/src/lib.rs")
text = path.read_text(encoding="utf-8")
pattern = re.compile(
    r'''    #\[test\]\n    fn doclang_block_body_uses_raw_target_conversion_before_nested_mutations\(\) \{.*?\n    \}\n\n    #\[test\]\n    fn failed_doclang_resolution_restores_nested_state_mutations\(\) \{''',
    re.S,
)
replacement = r'''    #[test]
    fn doclang_v260_rejects_raw_block_locale_and_preserves_previous_state() {
        let source =
            ".doclang {en}\n.doclang\n    .doclang {it}\n    .uppercase {nested}\n.doclang\n";
        let (result, source_id) = compile_source(source);
        assert_eq!(result.diagnostics.len(), 1, "{result:?}");
        assert_eq!(result.diagnostics[0].code, "E3001", "{result:?}");
        assert_eq!(
            result.diagnostics[0].primary.map(|span| span.source_id),
            Some(source_id),
            "{result:?}"
        );
        assert_eq!(output_text(&result), "English", "{result:?}");
        assert_eq!(
            result.ir.metadata.document_state.locale,
            Some(crate::ir::IrDocumentLocale {
                tag: "en".to_string(),
                localized_name: "English".to_string(),
            })
        );
    }

    #[test]
    fn failed_doclang_resolution_restores_nested_state_mutations() {'''
text, count = pattern.subn(replacement, text, count=1)
assert count == 1, count
path.write_text(text, encoding="utf-8")
print("v2.6 doclang regression expectation updated")
