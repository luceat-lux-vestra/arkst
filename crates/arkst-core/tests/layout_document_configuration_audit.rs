//! Offline completeness and ownership guard for Issue #153.

use std::collections::{BTreeMap, BTreeSet};

const MANIFEST: &str = include_str!(
    "../../../docs/compatibility/quarkdown/LAYOUT_DOCUMENT_CONFIGURATION_AUDIT_MANIFEST.tsv"
);
const AUDIT: &str =
    include_str!("../../../docs/compatibility/quarkdown/LAYOUT_DOCUMENT_CONFIGURATION_AUDIT.md");
const TARGET_SHA: &str = "107ec3a9482f10d6f90d7580f8409b46a719d18e";
const BASE_SHA: &str = "4a9112a9ee840374350dd9a90b65f58cce96eb08";
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
const OWNED_NAMES: &[&str] = &[
    "autopagebreak",
    "captionposition",
    "currentpage",
    "font",
    "footer",
    "formatpagenumber",
    "lastheading",
    "marker",
    "navigation",
    "noautopagebreak",
    "nonumbering",
    "numbering",
    "pageformat",
    "pagemargin",
    "paragraphstyle",
    "resetpagenumber",
    "slides",
    "tableofcontents",
    "texmacro",
    "totalpages",
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
                    .expect("numeric #153 manifest declaration"),
            )
        })
        .collect()
}

#[test]
fn manifest_is_complete_and_machine_checkable() {
    let rows = rows();
    let declarations = declarations();
    assert_eq!(declarations.get("total"), Some(&rows.len()));
    assert_eq!(declarations.get("153_owned"), Some(&20));
    assert_eq!(declarations.get("cross_owned"), Some(&27));

    let mut names = BTreeSet::new();
    let mut owned = BTreeSet::new();
    let mut alias_owners = BTreeMap::<&str, &str>::new();

    for row in &rows {
        assert_eq!(
            row.len(),
            12,
            "manifest row has wrong column count: {row:?}"
        );
        assert!(matches!(row[0], "owned" | "cross-owned"));
        assert!(names.insert(row[1]), "duplicate canonical name: {}", row[1]);
        assert!(!row[2].is_empty() && !row[6].is_empty());
        assert!(row[3] == "none" || row[3].split(';').all(|alias| !alias.is_empty()));
        assert!(matches!(row[4], "#153" | "#154"));
        assert!(STATUSES.contains(&row[5]), "invalid status: {}", row[5]);
        assert!(
            row[8].contains(TARGET_SHA),
            "missing pinned provenance: {row:?}"
        );
        assert!(!row[9].is_empty() && !row[10].is_empty() && !row[11].is_empty());

        if row[4] == "#153" {
            assert_eq!(row[0], "owned");
            assert_ne!(row[5], "NOT_APPLICABLE");
            assert!(owned.insert(row[1]), "duplicate owned name: {}", row[1]);
        } else {
            assert_eq!(row[0], "cross-owned");
            assert_eq!(row[5], "NOT_APPLICABLE");
        }

        if row[3] != "none" {
            for alias in row[3].split(';') {
                if alias == row[1] {
                    continue;
                }
                assert_eq!(
                    alias_owners.insert(alias, row[1]),
                    None,
                    "alias creates a duplicate surface: {alias}"
                );
                assert!(
                    !names.contains(alias),
                    "alias duplicates canonical name: {alias}"
                );
            }
        }
    }

    assert_eq!(owned.into_iter().collect::<Vec<_>>(), OWNED_NAMES);
    assert_eq!(rows.len(), 47);
    assert_eq!(rows.iter().filter(|row| row[4] == "#153").count(), 20);
    assert_eq!(rows.iter().filter(|row| row[4] == "#154").count(), 27);
    assert_eq!(rows.iter().filter(|row| row[5] == "PARTIAL").count(), 1);
    assert_eq!(
        rows.iter().filter(|row| row[5] == "PARSED_ONLY").count(),
        19
    );
    assert!(MANIFEST.contains(BASE_SHA));
    assert!(MANIFEST.contains("captionposition\tcaptionPosition\tcode"));
}

#[test]
fn audit_records_pipeline_boundary_and_state_rendering_separation() {
    assert!(AUDIT.contains("No additional #153-owned public callable was found"));
    assert!(AUDIT.contains("A preserved `IrNode::FunctionCall`"));
    assert!(AUDIT.contains("or inline directive is not a successful setter"));
    assert!(AUDIT.contains("No #153-owned row has current Typst/PDF/HTML"));
    assert!(AUDIT.contains("Production semantic/state"));
    assert!(AUDIT.contains("changes: **none**"));
}

#[test]
fn audit_records_numbering_extra_and_pageformat_border_contracts() {
    assert!(AUDIT.contains("every input pair is reparsed into `extra`"));
    assert!(AUDIT.contains("can be present in both the typed fields and `extra`"));
    assert!(AUDIT.contains("`hasBorder` is true"));
    assert!(AUDIT.contains("omitted side fields are"));
    assert!(AUDIT.contains("explicitly `Size.ZERO`"));
    assert!(AUDIT.contains("`contentBorderWidth` is null"));
    assert!(AUDIT.contains("`bordercolor` is independent from that `hasBorder` calculation"));
    assert!(AUDIT.contains("`--qd-page-content-border-width` remains at its renderer/CSS default"));

    let rows = rows();
    let numbering = rows
        .iter()
        .find(|row| row[1] == "numbering")
        .expect("numbering row");
    assert!(numbering[10].contains("every input key"));
    assert!(numbering[11].contains("all-input-keys-in-extra"));

    let pageformat = rows
        .iter()
        .find(|row| row[1] == "pageformat")
        .expect("pageformat row");
    assert!(pageformat[10].contains("partial-side input zeroes omitted sides"));
    assert!(pageformat[10].contains("bordercolor-only"));
    assert!(pageformat[11].contains("border-side-zeroing"));
    assert!(pageformat[11].contains("bordercolor-only-width-inheritance"));
}

#[test]
fn audit_records_pinned_pagination_renderer_divergences() {
    assert!(AUDIT.contains("does not implement that full grammar"));
    assert!(AUDIT.contains("transforms only the exact strings"));
    for format in ["`1`", "`a`", "`A`", "`i`", "`I`"] {
        assert!(
            AUDIT.contains(format),
            "missing page-number format: {format}"
        );
    }
    assert!(AUDIT.contains("Zero or negative values are ignored at render"));
    assert!(AUDIT.contains("performs no range check"));
    assert!(AUDIT.contains("not an upstream call-time validation rule"));

    let rows = rows();
    let formatter = rows
        .iter()
        .find(|row| row[1] == "formatpagenumber")
        .expect("formatpagenumber row");
    assert!(formatter[8].contains("page-numbers.ts@"));
    assert!(formatter[8].contains("numbering.ts@"));
    assert!(formatter[10].contains("last-marker-wins"));
    assert!(formatter[11].contains("page-level-formatter-divergence"));

    let reset = rows
        .iter()
        .find(|row| row[1] == "resetpagenumber")
        .expect("resetpagenumber row");
    assert!(reset[10].contains("ignores non-positive values"));
    assert!(reset[11].contains("page-level-reset-filtering"));

    let last_heading = rows
        .iter()
        .find(|row| row[1] == "lastheading")
        .expect("lastheading row");
    assert!(last_heading[8].contains("persistent-headings.ts@"));
    assert!(last_heading[10].contains("no call-time depth range validation"));
    assert!(last_heading[11].contains("documented-vs-runtime-depth"));
}

#[test]
fn audit_records_texmacro_follow_up_ownership() {
    let texmacro = rows()
        .into_iter()
        .find(|row| row[1] == "texmacro")
        .expect("texmacro row");
    assert!(texmacro[11].contains("#180"));
    assert!(!texmacro[11].contains("#175"));
    assert!(AUDIT.contains("assigned to #180"));
    assert!(AUDIT.contains("[#180](https://github.com/luceat-lux-vestra/arkst/issues/180)"));
}

#[test]
fn ownership_handoffs_and_prior_corrections_remain_intact() {
    let document_state_manifest =
        include_str!("../../../docs/compatibility/quarkdown/DOCUMENT_STATE_AUDIT_MANIFEST.tsv");
    let document_state_audit =
        include_str!("../../../docs/compatibility/quarkdown/DOCUMENT_STATE_AUDIT.md");

    assert!(document_state_manifest.contains("localization\tnone\t#151\tNOT_APPLICABLE"));
    assert!(document_state_manifest.contains("localize\tnone\t#151\tNOT_APPLICABLE"));
    for name in [
        "doctype",
        "docname",
        "docdescription",
        "docauthor",
        "docauthors",
        "dockeywords",
        "doclang",
        "theme",
    ] {
        assert!(
            document_state_manifest.contains(&format!("owned\t{name}")),
            "#152 row missing: {name}"
        );
    }
    assert!(document_state_audit.contains("doclang(locale: String? = null)"));
    assert!(!document_state_audit.contains("doclang(language: String? = null)"));
    assert!(document_state_audit.contains("localization and localize are retained as #151-owned"));
    assert!(document_state_manifest.contains("lib/localization.qd@"));

    let rows = rows();
    assert!(rows.iter().all(|row| row[4] != "#151" && row[4] != "#152"));
    assert!(rows
        .iter()
        .filter(|row| row[4] == "#154")
        .all(|row| row[5] == "NOT_APPLICABLE"));
}

#[test]
fn captionposition_revalidation_links_existing_slice() {
    let caption = rows()
        .into_iter()
        .find(|row| row[1] == "captionposition")
        .expect("captionposition row");
    assert_eq!(caption[4], "#153");
    assert_eq!(caption[5], "PARTIAL");
    assert!(caption[9].contains("captionposition_*"));
    assert!(caption[9].contains("document_state_roundtrips_deterministically"));
    assert!(caption[11].contains("#145/#146"));
    assert!(AUDIT.contains("The existing #145 / PR #146 slice"));
    assert!(AUDIT.contains("Canonical status: `PARTIAL`."));
}
