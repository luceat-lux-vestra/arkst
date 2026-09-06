//! Executable evidence guard for Issue #296's pinned Quarkdown v2.5.1
//! resource-permission research slice.

const RESEARCH: &str = include_str!("../../../docs/research/quarkdown-resource-permissions-296.md");
const MANIFEST: &str = include_str!(
    "../../../docs/compatibility/quarkdown/FILESYSTEM_PROJECT_DATA_RESOURCES_AUDIT_MANIFEST.tsv"
);
const TARGET_SHA: &str = "107ec3a9482f10d6f90d7580f8409b46a719d18e";

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
        "Native host resource/loadable-library discovery and ingestion",
        "Public WASM/embedder resource binding and native/WASM parity",
    ] {
        assert!(RESEARCH.contains(invariant), "missing policy: {invariant}");
    }

    assert!(RESEARCH.contains("#298"));
    assert!(RESEARCH.contains("#191"));
    assert!(RESEARCH.contains("#189"));
    assert!(RESEARCH.contains("#190"));
}

#[test]
fn evidence_slice_does_not_overclaim_canonical_support() {
    for surface in [
        "builtin:.read",
        "builtin:.json",
        "builtin:.include",
        "builtin:.includeall",
        "builtin:.pathtoroot",
    ] {
        let row = resource_row(surface);
        assert_eq!(row[17], "PARTIAL", "{surface} must remain PARTIAL");
        assert!(
            row[21].contains("#296"),
            "first #296 evidence slice must not prematurely remove ownership: {surface}"
        );
    }

    assert!(RESEARCH.contains("The canonical #155"));
    assert!(RESEARCH
        .contains("resource audit/manifest and cross-audit reconciliation must consume this"));
    assert!(RESEARCH.contains("before #296 can close"));
}
