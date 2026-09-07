//! Executable evidence guard for Issue #296's pinned Quarkdown v2.5.1
//! resource-permission policy and canonical reconciliation.

const RESEARCH: &str = include_str!("../../../docs/research/quarkdown-resource-permissions-296.md");
const MANIFEST: &str = include_str!(
    "../../../docs/compatibility/quarkdown/FILESYSTEM_PROJECT_DATA_RESOURCES_AUDIT_MANIFEST.tsv"
);
const TARGET_SHA: &str = "107ec3a9482f10d6f90d7580f8409b46a719d18e";
const EVIDENCE_MERGE_SHA: &str = "e7da0718439ecf396fbfeceb133da872a940341a";
const POLICY_MARKER: &str = "POLICY_DIVERGENCE:global-read";
const EXPECTED_MARKER_SURFACES: [(&str, &str); 12] = [
    ("builtin:.read", "PARTIAL"),
    ("builtin:.json", "PARTIAL"),
    ("builtin:.include", "PARTIAL"),
    ("builtin:.includeall", "PARTIAL"),
    ("builtin:.pathtoroot", "PARTIAL"),
    ("builtin:.listfiles", "PARTIAL"),
    ("builtin:.filename", "PARTIAL"),
    ("builtin:.subdocument", "PARTIAL"),
    ("contract:markdown-subdocument-resolution", "PARTIAL"),
    ("contract:logical-path-normalization", "SUPPORTED_SEMANTICS"),
    ("contract:resource-diagnostics-provenance", "PARTIAL"),
    ("contract:nested-resource-identity", "PARTIAL"),
];

fn resource_row(surface: &str) -> Vec<&str> {
    MANIFEST
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .find_map(|line| {
            let fields = line.split('\t').collect::<Vec<_>>();
            (fields.get(1) == Some(&surface)).then_some(fields)
        })
        .unwrap_or_else(|| panic!("missing #155 resource surface: {surface}"))
}

#[test]
fn pinned_upstream_permission_contract_is_explicit() {
    assert!(RESEARCH.contains(TARGET_SHA));
    assert!(RESEARCH.contains(EVIDENCE_MERGE_SHA));
    for source in [
        "Permission.kt",
        "PermissionHolder.kt",
        "MissingPermissionException.kt",
        "ExitCodes.kt",
        "FileSystem.kt",
        "Data.kt",
        "Ecosystem.kt",
    ] {
        assert!(RESEARCH.contains(source), "missing pinned source: {source}");
    }
    for contract in [
        "ProjectRead",
        "project-read",
        "GlobalRead",
        "global-read",
        "Permission.DEFAULT_SET",
        "Permission-before-existence",
        "MISSING_PERMISSION_EXIT_CODE = 72",
        "--allow global-read",
    ] {
        assert!(RESEARCH.contains(contract), "missing contract: {contract}");
    }
    let resolve = RESEARCH
        .find("resolve the supplied relative or absolute path")
        .expect("resolution-order evidence");
    let permission = RESEARCH
        .find("call `context.requireReadPermission(file)`")
        .expect("permission-order evidence");
    let existence = RESEARCH
        .find("check `file.exists()`")
        .expect("existence-order evidence");
    assert!(resolve < permission && permission < existence);
}

#[test]
fn arkst_policy_keeps_global_host_authority_fail_closed() {
    for invariant in [
        "Intentional security divergence",
        "Arkst intentionally does **not** emulate",
        "document input cannot mint `global-read` authority",
        "Absolute/global filesystem authority cannot be encoded",
        "canonical #155 audit has consumed this policy decision",
    ] {
        assert!(RESEARCH.contains(invariant), "missing policy: {invariant}");
    }
    assert!(RESEARCH.contains("#296/#300"));
    assert!(RESEARCH.contains("#191"));
    assert!(RESEARCH.contains("#189"));
    assert!(RESEARCH.contains("#190"));
}

#[test]
fn canonical_rows_keep_status_and_replace_completed_ownership() {
    for surface in [
        "builtin:.read",
        "builtin:.json",
        "builtin:.include",
        "builtin:.includeall",
        "builtin:.pathtoroot",
        "contract:resource-diagnostics-provenance",
    ] {
        let row = resource_row(surface);
        assert_eq!(row[17], "PARTIAL", "{surface} must remain PARTIAL");
        assert!(
            row[21].contains(POLICY_MARKER),
            "missing policy marker: {surface}"
        );
        for (index, label) in [
            (19, "remaining gap"),
            (20, "blocker dependency"),
            (21, "follow-up"),
        ] {
            assert!(
                !row[index].contains("#296"),
                "completed #296 leaked into unresolved {label} for {surface}: {}",
                row[index]
            );
        }
    }
    assert!(RESEARCH.contains("Affected rows remain `PARTIAL`"));
    assert!(RESEARCH.contains(POLICY_MARKER));
}

#[test]
fn completed_296_never_reappears_as_actionable_manifest_ownership() {
    let mut marker_rows = Vec::new();
    for line in MANIFEST
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        let fields = line.split('\t').collect::<Vec<_>>();
        assert_eq!(fields.len(), 24, "wrong #155 manifest column count: {line}");
        for (index, label) in [
            (19, "remaining gap"),
            (20, "blocker dependency"),
            (21, "follow-up"),
        ] {
            assert!(
                !fields[index].contains("#296"),
                "completed #296 leaked into unresolved {label} for {}: {}",
                fields[1],
                fields[index]
            );
        }
        if fields[21].split(';').any(|value| value == POLICY_MARKER) {
            marker_rows.push((fields[1], fields[17]));
        }
    }
    assert_eq!(marker_rows, EXPECTED_MARKER_SURFACES);
    assert!(!MANIFEST.contains(".; docs/research/quarkdown-resource-permissions-296.md"));
    assert!(!MANIFEST.contains("policy divergence work"));
}
