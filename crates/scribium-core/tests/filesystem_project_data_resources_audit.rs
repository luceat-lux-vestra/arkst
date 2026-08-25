//! Offline completeness and ownership guard for Issue #155.

use std::collections::{BTreeMap, BTreeSet};

const MANIFEST: &str = include_str!(
    "../../../docs/compatibility/quarkdown/FILESYSTEM_PROJECT_DATA_RESOURCES_AUDIT_MANIFEST.tsv"
);
const AUDIT: &str = include_str!(
    "../../../docs/compatibility/quarkdown/FILESYSTEM_PROJECT_DATA_RESOURCES_AUDIT.md"
);
const TARGET_SHA: &str = "107ec3a9482f10d6f90d7580f8409b46a719d18e";
const BASE_SHA: &str = "1a1fc7b1a978baa23d5eb0bfbef83ec49af5253f";
const COLUMN_NAMES: &[&str] = &[
    "ownership",
    "canonical_surface",
    "aliases",
    "category",
    "exact_upstream_declaration_or_syntax",
    "upstream_provenance",
    "accepted_arguments_and_return_behavior",
    "reference_and_relative_semantics",
    "nested_load_and_normalization",
    "missing_inaccessible_and_upstream_diagnostics",
    "boundary_traversal_behavior",
    "remote_network_behavior",
    "deterministic_external_inputs",
    "current_scribium_path",
    "resource_identity_and_roots",
    "security_host_and_platform_semantics",
    "current_evidence",
    "status",
    "supported_subset",
    "remaining_gap",
    "blocker_dependency",
    "follow_up",
    "disposition",
    "wasm_implications",
];
const STATUSES: &[&str] = &[
    "SUPPORTED_END_TO_END",
    "SUPPORTED_SEMANTICS",
    "PARSED_ONLY",
    "PARTIAL",
    "UNSUPPORTED",
    "DEFERRED",
    "BLOCKED",
    "UNKNOWN",
    "NOT_APPLICABLE",
];
const REQUIRED_SURFACES: &[&str] = &[
    "builtin:.read",
    "builtin:.json",
    "builtin:.include",
    "builtin:.includeall",
    "builtin:.pathtoroot",
    "builtin:.listfiles",
    "builtin:.filename",
    "builtin:.csv",
    "builtin:.bibliography",
    "builtin:.subdocument",
    "builtin:.env",
    "contract:markdown-subdocument-resolution",
    "contract:virtual-project-resource-model",
    "contract:logical-path-normalization",
    "contract:resource-diagnostics-provenance",
    "contract:project-boundary-enforcement",
    "contract:remote-resource-policy",
    "contract:nested-resource-identity",
    "contract:typst-entry-source-context",
    "contract:host-determinism-isolation",
    "contract:wasm-resource-boundary",
    "builtin:.image",
    "syntax:markdown-image-resource",
    "builtin:.font",
    "builtin:.link",
    "builtin:.markdown",
    "builtin:.llmstxt",
    "builtin:subdocumentgraph",
    "builtin:filetree",
];

fn rows() -> Vec<Vec<&'static str>> {
    MANIFEST
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| line.split('\t').collect())
        .collect()
}

fn declarations() -> BTreeMap<&'static str, usize> {
    MANIFEST
        .lines()
        .filter_map(|line| line.strip_prefix("# declared_"))
        .filter_map(|line| line.split_once('='))
        .map(|(name, value)| {
            (
                name,
                value
                    .parse::<usize>()
                    .expect("numeric #155 manifest declaration"),
            )
        })
        .collect()
}

fn row<'a>(rows: &'a [Vec<&'a str>], name: &str) -> &'a Vec<&'a str> {
    rows.iter()
        .find(|candidate| candidate[1] == name)
        .unwrap_or_else(|| panic!("missing #155 surface: {name}"))
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

#[test]
fn manifest_is_complete_pinned_and_machine_checkable() {
    let rows = rows();
    let declarations = declarations();

    assert_eq!(declarations.get("total"), Some(&29));
    assert_eq!(declarations.get("owned"), Some(&21));
    assert_eq!(declarations.get("handoffs"), Some(&8));
    assert_eq!(rows.len(), 29);
    assert!(MANIFEST.contains("# target_sha="));
    assert!(MANIFEST.contains(&format!("# target_sha={TARGET_SHA}")));
    assert!(MANIFEST.contains(&format!("# audit_base={BASE_SHA}")));

    let columns = MANIFEST
        .lines()
        .find_map(|line| line.strip_prefix("# Columns: "))
        .expect("#155 manifest columns declaration");
    assert_eq!(columns.split('\t').collect::<Vec<_>>(), COLUMN_NAMES);

    let mut names = BTreeSet::new();
    let mut aliases = BTreeMap::<&str, &str>::new();
    let mut status_counts = BTreeMap::<&str, usize>::new();
    let mut owned = 0;
    let mut handoffs = 0;

    for surface in &rows {
        assert_eq!(
            surface.len(),
            COLUMN_NAMES.len(),
            "wrong #155 column count: {surface:?}"
        );
        assert!(
            names.insert(surface[1]),
            "duplicate canonical name: {}",
            surface[1]
        );
        assert!(!surface[0].is_empty() && !surface[2].is_empty());
        assert!(!surface[4].is_empty() && !surface[5].is_empty());
        assert!(
            surface[5].contains(TARGET_SHA),
            "unpinned upstream provenance: {}",
            surface[1]
        );
        assert!(
            surface[5].contains("https://github.com/iamgio/quarkdown/"),
            "missing pinned Quarkdown source URL: {}",
            surface[1]
        );
        assert!(
            surface[5].contains(TARGET_SHA),
            "upstream target omitted from provenance: {}",
            surface[1]
        );
        for (index, field) in surface.iter().enumerate() {
            assert!(
                !field.trim().is_empty(),
                "empty field {index} in {}",
                surface[1]
            );
        }
        assert!(
            STATUSES.contains(&surface[17]),
            "invalid status: {}",
            surface[17]
        );
        assert!(!contains_any(
            surface[18],
            &["TODO", "TBD", "placeholder", "generic gap"]
        ));
        assert!(!contains_any(
            surface[19],
            &["TODO", "TBD", "placeholder", "generic gap"]
        ));
        assert!(
            contains_any(
                surface[7],
                &["relative", "Relative", "not applicable", "No path"]
            ),
            "source/project-relative classification is missing: {}",
            surface[1]
        );
        assert!(
            contains_any(
                surface[10],
                &["boundary", "Boundary", "traversal", "not applicable"]
            ),
            "boundary/traversal classification is missing: {}",
            surface[1]
        );
        assert!(
            !surface[9].is_empty(),
            "error behavior is missing: {}",
            surface[1]
        );
        assert!(
            !surface[11].is_empty(),
            "network behavior is missing: {}",
            surface[1]
        );
        assert!(
            !surface[12].is_empty(),
            "determinism behavior is missing: {}",
            surface[1]
        );
        assert!(
            !surface[23].is_empty(),
            "WASM implications are missing: {}",
            surface[1]
        );

        *status_counts.entry(surface[17]).or_default() += 1;

        if surface[0] == "owned" {
            owned += 1;
            assert_ne!(surface[17], "NOT_APPLICABLE");
            if matches!(
                surface[17],
                "PARTIAL" | "UNSUPPORTED" | "DEFERRED" | "BLOCKED" | "UNKNOWN"
            ) {
                assert!(
                    surface[21].contains('#'),
                    "actionable owned row has no bounded follow-up: {}",
                    surface[1]
                );
                assert!(!surface[20].is_empty());
            }
        } else {
            handoffs += 1;
            assert!(surface[0].starts_with("handoff:#"));
            assert_eq!(surface[17], "NOT_APPLICABLE");
            assert!(
                surface[21].contains('#'),
                "handoff has no owner: {}",
                surface[1]
            );
            assert!(surface[22].contains("handoff"));
        }

        match surface[17] {
            "SUPPORTED_END_TO_END" => {
                assert!(!contains_any(
                    surface[19],
                    &["unsupported", "not represented", "missing", "absent"]
                ));
            }
            "SUPPORTED_SEMANTICS" => {
                assert!(
                    contains_any(
                        surface[19],
                        &["not represented", "not rendered", "unavailable"]
                    ),
                    "semantic support needs an explicit layer gap: {}",
                    surface[1]
                );
            }
            "PARTIAL" => assert!(
                contains_any(surface[19], &["but", "remain", "absent"]),
                "partial row lacks supported/missing contract split: {}",
                surface[1]
            ),
            "UNSUPPORTED" => assert!(
                contains_any(surface[19], &["No ", "no ", "not represented", "does not"]),
                "unsupported row lacks an explicit absent contract: {}",
                surface[1]
            ),
            "DEFERRED" => assert!(
                surface[19].to_ascii_lowercase().contains("deferred"),
                "deferred row lacks rationale: {}",
                surface[1]
            ),
            "BLOCKED" => assert!(
                surface[19].to_ascii_lowercase().contains("blocked"),
                "blocked row lacks rationale: {}",
                surface[1]
            ),
            "UNKNOWN" => assert!(
                surface[19].contains("UNKNOWN") || surface[19].contains("not established"),
                "unknown row lacks uncertainty rationale: {}",
                surface[1]
            ),
            "PARSED_ONLY" | "NOT_APPLICABLE" => {}
            _ => unreachable!(),
        }

        if surface[2] != "none" {
            for alias in surface[2].split(';') {
                assert!(!alias.is_empty());
                assert!(
                    !names.contains(alias),
                    "alias duplicates canonical name: {alias}"
                );
                assert_eq!(
                    aliases.insert(alias, surface[1]),
                    None,
                    "duplicate alias: {alias}"
                );
            }
        }
    }

    for name in REQUIRED_SURFACES {
        assert!(
            names.contains(name),
            "required #155 surface is absent: {name}"
        );
    }
    assert_eq!(owned, declarations["owned"]);
    assert_eq!(handoffs, declarations["handoffs"]);
    assert_eq!(status_counts.values().sum::<usize>(), declarations["total"]);
}

#[test]
fn resource_architecture_and_historical_reconciliation_are_explicit() {
    let rows = rows();
    assert_eq!(row(&rows, "builtin:.read")[17], "PARTIAL");
    assert_eq!(row(&rows, "builtin:.include")[17], "PARTIAL");
    assert_eq!(
        row(&rows, "contract:virtual-project-resource-model")[17],
        "SUPPORTED_SEMANTICS"
    );
    assert_eq!(
        row(&rows, "contract:typst-entry-source-context")[17],
        "PARTIAL"
    );
    assert_eq!(
        row(&rows, "contract:wasm-resource-boundary")[17],
        "DEFERRED"
    );
    assert_eq!(row(&rows, "builtin:.image")[17], "NOT_APPLICABLE");
    assert_eq!(row(&rows, "builtin:filetree")[17], "NOT_APPLICABLE");

    assert!(MANIFEST.contains("#24"));
    assert!(MANIFEST.contains("#62"));
    assert!(AUDIT.contains("#24"));
    assert!(AUDIT.contains("#62"));
    assert!(AUDIT.contains("VirtualProject"));
    assert!(AUDIT.contains("source-relative"));
    assert!(AUDIT.contains("project-relative"));
    assert!(AUDIT.contains("nested"));
    assert!(AUDIT.contains("network"));
    assert!(AUDIT.contains("WASM"));
    assert!(AUDIT.contains("cwd"));
    assert!(AUDIT.contains("temp"));
}

#[test]
fn remote_and_deterministic_surfaces_cannot_disappear() {
    let rows = rows();
    let manifest_lower = MANIFEST.to_ascii_lowercase();
    for forbidden in ["todo", "tbd", "placeholder", "generic gap"] {
        assert!(
            !manifest_lower.contains(forbidden),
            "forbidden placeholder language: {forbidden}"
        );
    }
    for name in [
        "contract:remote-resource-policy",
        "builtin:.image",
        "syntax:markdown-image-resource",
        "builtin:.font",
        "builtin:.link",
        "builtin:.llmstxt",
        "builtin:.env",
    ] {
        let surface = row(&rows, name);
        assert!(
            !surface[11].is_empty(),
            "network classification missing: {name}"
        );
        assert!(
            !surface[12].is_empty(),
            "determinism classification missing: {name}"
        );
    }
    assert!(row(&rows, "contract:remote-resource-policy")[19]
        .to_ascii_lowercase()
        .contains("remote"));
    assert!(row(&rows, "builtin:.env")[17] == "UNSUPPORTED");
    assert!(row(&rows, "contract:wasm-resource-boundary")[21].contains("#191"));
}
