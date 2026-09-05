//! Cross-audit guard for the Issue #156 canonical compatibility view.

const RECONCILIATION: &str =
    include_str!("../../../docs/compatibility/quarkdown/RECONCILIATION.md");
const RESOURCE_MANIFEST: &str = include_str!(
    "../../../docs/compatibility/quarkdown/FILESYSTEM_PROJECT_DATA_RESOURCES_AUDIT_MANIFEST.tsv"
);
const STDLIB_AUDIT: &str =
    include_str!("../../../docs/compatibility/quarkdown/STDLIB_BUILTINS_AUDIT.md");
const STDLIB_MANIFEST: &str =
    include_str!("../../../docs/compatibility/quarkdown/STDLIB_BUILTINS_AUDIT_MANIFEST.tsv");
const CONTENT_MANIFEST: &str = include_str!(
    "../../../docs/compatibility/quarkdown/CONTENT_MEDIA_MARKDOWN_EXTENSIONS_AUDIT_MANIFEST.tsv"
);
const TARGET_SHA: &str = "107ec3a9482f10d6f90d7580f8409b46a719d18e";
const BASE_SHA: &str = "4875fb1210f0f9f3fdadc47bf48197b2bdaa17ec";

fn manifest_row<'a>(manifest: &'a str, column: usize, value: &str, label: &str) -> Vec<&'a str> {
    manifest
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .find_map(|line| {
            let fields = line.split('\t').collect::<Vec<_>>();
            (fields.get(column) == Some(&value)).then_some(fields)
        })
        .unwrap_or_else(|| panic!("missing {label}: {value}"))
}

fn resource_row(surface: &str) -> Vec<&str> {
    manifest_row(RESOURCE_MANIFEST, 1, surface, "#155 surface")
}

fn stdlib_row(name: &str) -> Vec<&str> {
    manifest_row(STDLIB_MANIFEST, 0, name, "#151 name")
}

fn content_row(surface: &str) -> Vec<&str> {
    manifest_row(CONTENT_MANIFEST, 1, surface, "#154 surface")
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
        "STDLIB_BUILTINS_AUDIT.md",
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
    assert!(RECONCILIATION.contains("20 cases"));
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
    assert_eq!(wasm_boundary[21], "#191");
    assert!(!wasm_boundary[19].contains("#156"));
    assert!(!wasm_boundary[20].contains("#156"));

    assert_eq!(virtual_project[21], "#182;#187");
    assert!(!virtual_project[21].contains("#156"));
}

#[test]
fn reconciliation_maps_all_unresolved_stdlib_families_to_bounded_owners() {
    for (name, issue, family) in [
        ("libexists", "#195", "library inspection"),
        ("functionexists", "#195", "library inspection"),
        ("libraries", "#195", "library inspection"),
        ("libfunctions", "#195", "library inspection"),
        ("log", "#197", "logger"),
        ("debug", "#197", "logger"),
        ("error", "#197", "logger"),
    ] {
        let row = stdlib_row(name);
        assert_eq!(row[4], "UNSUPPORTED", "wrong #151 status for {name}");
        assert!(STDLIB_AUDIT.contains(name), "#151 audit omits {name}");
        assert!(
            STDLIB_AUDIT.contains(issue),
            "#151 audit omits {issue} for {name}"
        );
        assert!(
            RECONCILIATION.contains(issue),
            "reconciliation omits {issue} for {family}"
        );
        assert!(RECONCILIATION.contains(family));
    }
}

#[test]
fn reconciliation_records_localization_as_semantically_supported() {
    for name in ["localization", "localize"] {
        let row = stdlib_row(name);
        assert_eq!(row[3], "#151");
        assert_eq!(row[4], "SUPPORTED_SEMANTICS");
    }
    assert!(STDLIB_AUDIT.contains("Bounded #196 localization contract"));
    assert!(RECONCILIATION.contains("localization-family"));
}

#[test]
fn reconciliation_records_dictionary_lookup_as_semantically_supported() {
    let row = stdlib_row("get");
    assert_eq!(row[3], "#151");
    assert_eq!(row[4], "SUPPORTED_SEMANTICS");
    assert!(STDLIB_AUDIT.contains("dedicated `DictionaryLookup` native owner"));
    assert!(STDLIB_AUDIT.contains("#194"));
    assert!(RECONCILIATION.contains("#194"));
    assert!(RECONCILIATION.contains("dictionary-get-family"));
}

#[test]
fn reconciliation_assigns_actionable_content_gaps_without_closed_owner_links() {
    let keybinding = content_row("primitive:keybinding");
    assert_eq!(keybinding[26], "UNKNOWN");
    assert_eq!(keybinding[27], "#184");
    assert!(keybinding[28].contains("#184"));

    let loremipsum = content_row("primitive:loremipsum");
    assert_eq!(loremipsum[26], "UNSUPPORTED");
    assert_eq!(loremipsum[27], "#184");
    assert!(loremipsum[28].contains("#184"));

    let matched = content_row("primitive:match");
    assert_eq!(matched[26], "UNSUPPORTED");
    assert_eq!(matched[27], "#198");
    assert!(!matched[27].contains("#181"));
    assert!(!matched[27].contains("#156"));

    for surface in ["primitive:css", "primitive:cssproperties"] {
        let row = content_row(surface);
        assert_eq!(row[26], "UNSUPPORTED");
        assert_eq!(row[27], "DEFERRED_PRODUCT_SURFACE:html-backend");
        assert!(row[28].contains("explicitly deferred"));
        assert!(!row[27].contains("#58"));
        assert!(!row[27].contains("#155"));
        assert!(!row[27].contains("#156"));
    }

    let graph = content_row("primitive:subdocumentgraph");
    assert_eq!(graph[26], "BLOCKED");
    assert_eq!(graph[27], "#188;#199");
    assert!(graph[28].contains("#199"));
    assert!(!graph[27].contains("#155"));
    assert!(!graph[27].contains("#156"));
}

#[test]
fn reconciliation_records_order_without_making_188_a_187_blocker() {
    assert!(RECONCILIATION.contains("#187's strategy decision is complete"));
    assert!(RECONCILIATION.contains("#200 is the explicit-selection follow-up"));
    assert!(RECONCILIATION.contains("not a prerequisite to start #187"));
    assert!(RECONCILIATION.contains("#188 is **after #187**"));
    assert!(RECONCILIATION.contains("#189 is **after #188**"));
    assert!(RECONCILIATION.contains("#190 is a parallel"));
    assert!(RECONCILIATION.contains("#191 is **deferred/milestone-blocked**"));
    assert!(RECONCILIATION
        .contains("Removing subprocess execution or changing the default is not decided"));
}

#[test]
fn reconciliation_covers_open_followups_and_historical_trackers() {
    for issue in [
        157, 158, 159, 160, 162, 163, 164, 165, 166, 167, 169, 172, 173, 175, 176, 177, 178, 180,
        181, 182, 183, 184, 185, 187, 188, 189, 190, 191, 194, 195, 196, 197, 198, 199,
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
    assert!(!RECONCILIATION.contains("must remain Post-#156"));
}
