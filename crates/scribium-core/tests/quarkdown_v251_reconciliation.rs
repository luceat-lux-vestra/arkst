//! Cross-audit guard for the Issue #156 canonical compatibility view.

const RECONCILIATION: &str =
    include_str!("../../../docs/compatibility/quarkdown/RECONCILIATION.md");
const RESOURCE_MANIFEST: &str = include_str!(
    "../../../docs/compatibility/quarkdown/FILESYSTEM_PROJECT_DATA_RESOURCES_AUDIT_MANIFEST.tsv"
);
const TARGET_SHA: &str = "107ec3a9482f10d6f90d7580f8409b46a719d18e";
const BASE_SHA: &str = "4875fb1210f0f9f3fdadc47bf48197b2bdaa17ec";

fn resource_row(surface: &str) -> Vec<&str> {
    RESOURCE_MANIFEST
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .find_map(|line| {
            let fields = line.split('\t').collect::<Vec<_>>();
            (fields.get(1) == Some(&surface)).then_some(fields)
        })
        .unwrap_or_else(|| panic!("missing #155 surface: {surface}"))
}

#[test]
fn reconciliation_records_the_pinned_target_and_baseline() {
    assert!(RECONCILIATION.contains("#156"));
    assert!(RECONCILIATION.contains(TARGET_SHA));
    assert!(RECONCILIATION.contains(BASE_SHA));
    assert!(RECONCILIATION.contains("2026-08-26"));
    assert!(RECONCILIATION.contains("#148"));
}

#[test]
fn reconciliation_enumerates_each_audit_artifact_and_corpus_boundary() {
    for artifact in [
        "CALL_GRAMMAR_AUDIT.md",
        "VALUE_MODEL_AUDIT.md",
        "PROGRAMMABLE_SEMANTICS_AUDIT.md",
        "STDLIB_BUILTINS_AUDIT_MANIFEST.tsv",
        "DOCUMENT_STATE_AUDIT_MANIFEST.tsv",
        "LAYOUT_DOCUMENT_CONFIGURATION_AUDIT_MANIFEST.tsv",
        "CONTENT_MEDIA_MARKDOWN_EXTENSIONS_AUDIT_MANIFEST.tsv",
        "FILESYSTEM_PROJECT_DATA_RESOURCES_AUDIT_MANIFEST.tsv",
    ] {
        assert!(
            RECONCILIATION.contains(artifact),
            "missing artifact: {artifact}"
        );
    }
    assert!(RECONCILIATION.contains("17 cases"));
    assert!(RECONCILIATION.contains("No Quarkdown source, test, or fixture was copied"));
    assert!(RECONCILIATION.contains("SUPPORTED_END_TO_END"));
    assert!(RECONCILIATION.contains("SUPPORTED_SEMANTICS"));
    assert!(RECONCILIATION.contains("PARSED_ONLY"));
    assert!(RECONCILIATION.contains("NOT_APPLICABLE"));
}

#[test]
fn reconciliation_keeps_resource_statuses_and_ownership_single_sourced() {
    for (surface, follow_up) in [
        ("builtin:.read", "#188"),
        ("builtin:.json", "#188;#149"),
        ("builtin:.include", "#188"),
    ] {
        let row = resource_row(surface);
        assert_eq!(row[17], "PARTIAL", "wrong canonical status for {surface}");
        assert_eq!(row[21], follow_up, "wrong follow-up for {surface}");
    }

    let virtual_project = resource_row("contract:virtual-project-resource-model");
    assert_eq!(virtual_project[17], "SUPPORTED_SEMANTICS");

    let typst_context = resource_row("contract:typst-entry-source-context");
    assert_eq!(typst_context[17], "PARTIAL");
    assert_eq!(typst_context[21], "#187");

    let wasm_boundary = resource_row("contract:wasm-resource-boundary");
    assert_eq!(wasm_boundary[17], "DEFERRED");
    assert_eq!(wasm_boundary[21], "#191;#156");
}

#[test]
fn reconciliation_records_order_without_making_188_a_187_blocker() {
    assert!(RECONCILIATION.contains("**Immediate next technical work.**"));
    assert!(RECONCILIATION.contains("not a prerequisite to start #187"));
    assert!(RECONCILIATION.contains("#188 is **after #187**"));
    assert!(RECONCILIATION.contains("#189 is **after #188**"));
    assert!(RECONCILIATION.contains("#190 is a parallel"));
    assert!(RECONCILIATION.contains("#191 is **deferred/milestone-blocked**"));
    assert!(RECONCILIATION.contains("Removing subprocess execution is not decided"));
}

#[test]
fn reconciliation_covers_open_followups_and_historical_trackers() {
    for issue in [
        157, 158, 159, 160, 162, 163, 164, 165, 166, 167, 169, 172, 173, 175, 176, 177, 178, 180,
        181, 182, 183, 184, 185, 187, 188, 189, 190, 191,
    ] {
        assert!(
            RECONCILIATION.contains(&format!("#{issue}")),
            "missing issue #{issue}"
        );
    }
    for issue in [24, 56, 60, 61, 62, 63] {
        assert!(
            RECONCILIATION.contains(&format!("#{issue}")),
            "missing historical issue #{issue}"
        );
    }
    assert!(RECONCILIATION.contains("#147 remains open"));
    assert!(!RECONCILIATION.contains("frozen until #156"));
}
