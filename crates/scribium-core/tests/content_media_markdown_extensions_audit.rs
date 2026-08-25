//! Offline completeness and ownership guard for Issue #154.

use std::collections::{BTreeMap, BTreeSet};

const MANIFEST: &str = include_str!(
    "../../../docs/compatibility/quarkdown/CONTENT_MEDIA_MARKDOWN_EXTENSIONS_AUDIT_MANIFEST.tsv"
);
const AUDIT: &str = include_str!(
    "../../../docs/compatibility/quarkdown/CONTENT_MEDIA_MARKDOWN_EXTENSIONS_AUDIT.md"
);
const TARGET_SHA: &str = "107ec3a9482f10d6f90d7580f8409b46a719d18e";
const BASE_SHA: &str = "7144683346fd6e39c49ef0923733c856a6a55f42";
const STATUSES: &[&str] = &[
    "SUPPORTED_END_TO_END",
    "SUPPORTED_SEMANTICS",
    "PARSED_ONLY",
    "PARTIAL",
    "UNSUPPORTED",
    "DEFERRED",
    "BLOCKED",
    "NOT_APPLICABLE",
    "UNKNOWN",
];
const SUPPORTED_END_TO_END_GAP_PREFIX: &str = "No #154-owned gap";
const SUPPORTED_SEMANTICS_GAP_MARKERS: &[&str] = &["represented", "rendered", "unavailable"];
const REPRESENTATIVE_OWNERSHIP: &[(&str, &str)] = &[
    ("primitive:text", "#184"),
    ("primitive:box", "#184"),
    ("primitive:todo", "#184"),
    ("primitive:collapse", "#184"),
    ("primitive:textcollapse", "#184"),
    ("primitive:clip", "#184"),
    ("primitive:float", "#184"),
    ("primitive:fullspan", "#184"),
    ("primitive:fragment", "#184"),
    ("primitive:speakernote", "#184"),
    ("syntax:qd-inline-math", "#185"),
    ("syntax:qd-display-math", "#185"),
    ("primitive:math", "#185"),
    ("primitive:code", "#185"),
    ("syntax:qd-code-caption", "#185"),
    ("primitive:codespan", "#185"),
    ("syntax:qd-pagebreak", "#185"),
    ("primitive:pagebreak", "#185"),
    ("primitive:heading", "#181"),
    ("primitive:paragraph", "#181"),
    ("primitive:figure", "#181"),
    ("primitive:numbered", "#181"),
    ("primitive:ref", "#181"),
    ("syntax:qd-custom-id", "#181"),
    ("syntax:qd-caption-delimiter", "#181"),
    ("syntax:qd-cross-reference", "#181"),
    ("markdown:footnotes", "#181"),
    ("syntax:qd-image-size", "#182"),
    ("primitive:image", "#182"),
    ("primitive:icon", "#182"),
    ("primitive:emoji", "#182"),
    ("primitive:allemojis", "#182"),
    ("primitive:mermaid", "#182"),
    ("primitive:xychart", "#182"),
    ("primitive:container", "#184"),
    ("primitive:table", "#183"),
    ("primitive:table-sort", "#183"),
    ("primitive:table-filter", "#183"),
    ("primitive:table-compute", "#183"),
    ("primitive:table-column", "#183"),
    ("primitive:table-columns", "#183"),
    ("primitive:table-by-rows", "#183"),
];
const COMPONENT_LOCAL_SURFACES: &[&str] = &[
    "primitive:container",
    "primitive:align",
    "primitive:center",
    "primitive:float",
    "primitive:row",
    "primitive:column",
    "primitive:grid",
    "primitive:landscape",
    "primitive:fullspan",
    "primitive:whitespace",
    "primitive:clip",
    "primitive:box",
    "primitive:collapse",
    "primitive:textcollapse",
    "primitive:todo",
    "primitive:text",
    "primitive:fragment",
    "primitive:speakernote",
];
const REQUIRED_SURFACES: &[&str] = &[
    "markdown:blocks",
    "markdown:inlines",
    "markdown:tables",
    "markdown:code-fences",
    "markdown:images",
    "markdown:footnotes",
    "markdown:raw-html",
    "syntax:qd-image-size",
    "syntax:qd-inline-math",
    "syntax:qd-display-math",
    "syntax:qd-pagebreak",
    "syntax:qd-custom-id",
    "syntax:qd-caption-delimiter",
    "syntax:qd-code-caption",
    "syntax:qd-compact-footnotes",
    "syntax:qd-cross-reference",
    "primitive:heading",
    "primitive:image",
    "primitive:figure",
    "primitive:code",
    "primitive:math",
    "primitive:table",
    "primitive:table-sort",
    "primitive:table-by-rows",
    "primitive:container",
    "primitive:align",
    "primitive:center",
    "primitive:row",
    "primitive:column",
    "primitive:grid",
    "primitive:landscape",
    "primitive:fullspan",
    "primitive:whitespace",
    "primitive:box",
    "primitive:numbered",
    "primitive:br",
    "primitive:markdown",
    "primitive:htmloptions",
    "primitive:html",
    "primitive:ref",
    "primitive:fragment",
    "primitive:speakernote",
    "primitive:mermaid",
    "primitive:subdocumentgraph",
    "state:captionposition",
    "state:numbering",
    "state:pageformat-columns",
    "state:slides",
    "state:texmacro",
    "resource:read",
    "resource:include-subdocument",
    "content:binder-conversion",
    "content:nested-tight-calls",
    "content:inline-markdown-body",
    "content:raw-body",
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
                    .expect("numeric #154 manifest declaration"),
            )
        })
        .collect()
}

fn row<'a>(rows: &'a [Vec<&'a str>], name: &str) -> &'a Vec<&'a str> {
    rows.iter()
        .find(|candidate| candidate[1] == name)
        .unwrap_or_else(|| panic!("missing #154 surface: {name}"))
}

#[test]
fn manifest_is_complete_pinned_and_machine_checkable() {
    let rows = rows();
    let declarations = declarations();
    assert_eq!(declarations.get("total"), Some(&83));
    assert_eq!(declarations.get("owned"), Some(&71));
    assert_eq!(declarations.get("handoffs"), Some(&12));
    assert_eq!(rows.len(), 83);
    assert_eq!(MANIFEST.matches(TARGET_SHA).count(), 84);
    assert!(MANIFEST.contains(BASE_SHA));

    let mut names = BTreeSet::new();
    let mut aliases = BTreeMap::<&str, &str>::new();
    let mut statuses = BTreeMap::<&str, usize>::new();

    for surface in &rows {
        assert!(
            names.insert(surface[1]),
            "duplicate canonical name: {}",
            surface[1]
        );
    }

    for surface in &rows {
        assert_eq!(surface.len(), 30, "wrong #154 column count: {surface:?}");
        assert!(!surface[2].is_empty() && !surface[3].is_empty());
        assert!(!surface[4].is_empty() && !surface[5].is_empty());
        assert!(
            surface[5].contains(TARGET_SHA),
            "unpinned provenance: {surface:?}"
        );
        assert!(
            !surface[28].contains("The pinned contract is not represented at the required semantic and output layers."),
            "generic remaining gap hides the row contract: {}",
            surface[1]
        );
        assert!(
            STATUSES.contains(&surface[26]),
            "invalid status: {}",
            surface[26]
        );
        assert!(!surface[27].is_empty() && !surface[28].is_empty());
        *statuses.entry(surface[26]).or_default() += 1;

        if surface[0] == "owned" {
            assert_ne!(surface[26], "NOT_APPLICABLE");
            if matches!(
                surface[26],
                "PARTIAL" | "UNSUPPORTED" | "DEFERRED" | "BLOCKED" | "UNKNOWN"
            ) {
                assert!(
                    surface[27].contains('#'),
                    "actionable row has no bounded follow-up: {}",
                    surface[1]
                );
            }
        } else {
            assert!(surface[0].starts_with("handoff:#"));
            assert_eq!(surface[26], "NOT_APPLICABLE");
        }

        match surface[26] {
            "SUPPORTED_END_TO_END" => {
                assert!(
                    surface[28].starts_with(SUPPORTED_END_TO_END_GAP_PREFIX),
                    "end-to-end support needs an exact bounded no-gap statement: {}",
                    surface[1]
                );
                assert!(
                    surface[15].contains("lowering")
                        || surface[16].contains("output")
                        || surface[16].contains("Typst/PDF"),
                    "end-to-end row lacks output evidence: {}",
                    surface[1]
                );
                assert!(
                    surface[25].contains("tests"),
                    "end-to-end row lacks current test evidence: {}",
                    surface[1]
                );
            }
            "SUPPORTED_SEMANTICS" => {
                for marker in SUPPORTED_SEMANTICS_GAP_MARKERS {
                    assert!(
                        surface[28].contains(marker),
                        "semantic-support gap lacks {marker:?}: {}",
                        surface[1]
                    );
                }
                assert!(
                    surface[16].contains("Semantic support only"),
                    "semantic-support row overclaims output: {}",
                    surface[1]
                );
            }
            "PARTIAL" => assert!(
                surface[28].contains("but")
                    || surface[28].contains("remains")
                    || surface[28].contains("remain")
                    || surface[28].contains("not represented"),
                "partial row lacks a bounded supported/remaining split: {}",
                surface[1]
            ),
            "UNSUPPORTED" => assert!(
                surface[28].contains("No ")
                    || surface[28].contains("not represented")
                    || surface[28].contains("does not"),
                "unsupported row lacks an explicit absent contract: {}",
                surface[1]
            ),
            "DEFERRED" => assert!(
                surface[28].contains("deferred"),
                "deferred row lacks deferred rationale: {}",
                surface[1]
            ),
            "BLOCKED" => assert!(
                surface[28].contains("blocked"),
                "blocked row lacks blocker rationale: {}",
                surface[1]
            ),
            "UNKNOWN" => assert!(
                surface[28].contains("UNKNOWN") || surface[28].contains("not established"),
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
            "required #154 surface is absent: {name}"
        );
    }

    assert_eq!(statuses.get("SUPPORTED_END_TO_END"), Some(&13));
    assert_eq!(statuses.get("SUPPORTED_SEMANTICS"), Some(&3));
    assert_eq!(statuses.get("PARSED_ONLY"), Some(&1));
    assert_eq!(statuses.get("PARTIAL"), Some(&13));
    assert_eq!(statuses.get("UNSUPPORTED"), Some(&37));
    assert_eq!(statuses.get("DEFERRED"), Some(&2));
    assert_eq!(statuses.get("BLOCKED"), Some(&1));
    assert_eq!(statuses.get("UNKNOWN"), Some(&1));
    assert_eq!(statuses.get("NOT_APPLICABLE"), Some(&12));
}

#[test]
fn markdown_and_quarkdown_layers_are_not_promoted() {
    let rows = rows();
    assert_eq!(row(&rows, "markdown:images")[26], "PARTIAL");
    assert_eq!(row(&rows, "syntax:qd-image-size")[26], "UNSUPPORTED");
    assert_eq!(
        row(&rows, "markdown:code-fences")[26],
        "SUPPORTED_END_TO_END"
    );
    assert_eq!(row(&rows, "syntax:qd-code-caption")[26], "PARTIAL");
    assert_eq!(row(&rows, "markdown:raw-html")[26], "PARTIAL");
    assert_eq!(row(&rows, "primitive:html")[26], "SUPPORTED_SEMANTICS");
    assert_eq!(row(&rows, "primitive:markdown")[26], "SUPPORTED_SEMANTICS");
    assert!(AUDIT.contains("Markdown baseline versus Quarkdown semantics"));
    assert!(AUDIT.contains("parser retention and source provenance"));
    assert!(AUDIT.contains("Typst lowering and rendered-output fidelity"));
}

#[test]
fn ownership_handoffs_and_frozen_scope_are_explicit() {
    let rows = rows();
    for (name, owner, dependency) in [
        ("state:captionposition", "handoff:#153", "#153"),
        ("state:numbering", "handoff:#153", "#153"),
        ("state:pageformat-columns", "handoff:#153", "#153"),
        ("state:slides", "handoff:#153", "#153"),
        ("state:texmacro", "handoff:#153", "#180"),
        ("resource:read", "handoff:#155", "#155"),
        ("content:binder-conversion", "handoff:#149", "#149"),
        ("content:nested-tight-calls", "handoff:#158", "#158"),
        ("content:inline-markdown-body", "handoff:#160", "#160"),
        ("content:raw-body", "handoff:#166", "#166"),
    ] {
        let surface = row(&rows, name);
        assert_eq!(surface[0], owner, "wrong owner for {name}");
        assert!(
            surface[27].contains(dependency),
            "missing dependency for {name}"
        );
    }
    assert!(AUDIT.contains("#153-owned global state"));
    assert!(AUDIT.contains("#155"));
    assert!(AUDIT.contains("No .texmacro"));
    assert!(AUDIT.contains("Production semantic, state, parser"));
    assert!(AUDIT.contains("implementation remains frozen"));
}

#[test]
fn canonical_follow_up_ownership_and_producer_boundaries_are_reconciled() {
    let rows = rows();

    for (surface_name, expected_issue) in REPRESENTATIVE_OWNERSHIP {
        let surface = row(&rows, surface_name);
        assert!(
            surface[27].contains(expected_issue),
            "wrong canonical follow-up for {surface_name}: expected {expected_issue}, got {}",
            surface[27]
        );
    }

    for surface_name in COMPONENT_LOCAL_SURFACES {
        let surface = row(&rows, surface_name);
        assert!(
            !surface[27].contains("#175"),
            "document-wide #175 must not own component-local {surface_name}: {}",
            surface[27]
        );
    }

    for (surface_name, producer_issue) in [
        ("primitive:code", "#185 (producer)"),
        ("primitive:math", "#185 (producer)"),
        ("primitive:table", "#183 (producer)"),
    ] {
        let surface = row(&rows, surface_name);
        assert!(
            surface[27].starts_with(producer_issue),
            "producer ownership is not canonical for {surface_name}: {}",
            surface[27]
        );
        assert!(
            surface[27].contains("#181 (shared caption/reference/index)"),
            "shared caption/reference/index dependency is missing for {surface_name}: {}",
            surface[27]
        );
    }

    assert!(AUDIT.contains("component-local producer family"));
    assert!(AUDIT.contains("icon/emoji catalog"));
    assert!(AUDIT.contains("diagram") && AUDIT.contains("chart"));
    assert!(AUDIT.contains("remaining component-local `.container` subcontracts"));
    assert!(
        AUDIT.contains("#181 owns the shared caption/identifier/reference/index infrastructure")
    );
    assert!(AUDIT.contains("producer implementation is assigned to #185"));
    assert!(AUDIT.contains("#175 remains document-wide only"));
}

#[test]
fn actionable_rows_are_grouped_into_bounded_follow_ups() {
    let rows = rows();
    for surface in rows.iter().filter(|surface| surface[0] == "owned") {
        if matches!(
            surface[26],
            "PARTIAL" | "UNSUPPORTED" | "DEFERRED" | "BLOCKED" | "UNKNOWN"
        ) {
            assert!(
                surface[27].contains('#'),
                "missing issue linkage: {}",
                surface[1]
            );
        }
    }
    assert!(AUDIT.contains("No one-issue-per-surface fragmentation"));
    for issue in ["#181", "#182", "#183", "#184", "#185", "#155", "#180"] {
        assert!(AUDIT.contains(issue), "missing bounded follow-up: {issue}");
    }
}
