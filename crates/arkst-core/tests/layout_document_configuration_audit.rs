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
    assert_eq!(rows.iter().filter(|row| row[5] == "PARTIAL").count(), 9);
    assert_eq!(
        rows.iter().filter(|row| row[5] == "PARSED_ONLY").count(),
        11
    );
    assert!(MANIFEST.contains(BASE_SHA));
    assert!(MANIFEST.contains("captionposition\tcaptionPosition\tcode"));
}

#[test]
fn audit_records_pipeline_boundary_and_state_rendering_separation() {
    assert!(AUDIT.contains("No additional #153-owned public callable was found"));
    assert!(AUDIT.contains("A preserved `IrNode::FunctionCall`"));
    assert!(AUDIT.contains("is not a successful setter, typed node, state mutation"));
    assert!(AUDIT.contains("No #153-owned row has complete v2.5.1 output equivalence"));
    assert!(AUDIT.contains("nine conservative `PARTIAL` rows"));
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
    assert!(AUDIT.contains("typed `left`/`right` page-side selectors"));
    assert!(AUDIT.contains("must not leak into the flattened global fields"));

    let rows = rows();
    let numbering = rows
        .iter()
        .find(|row| row[1] == "numbering")
        .expect("numbering row");
    assert!(numbering[10].contains("all input keys retained in extra"));
    assert!(numbering[11].contains("all-input-keys-in-extra"));
    assert_eq!(numbering[5], "PARTIAL");
    assert!(numbering[9].contains("numbering_state.rs"));
    let nonumbering = rows
        .iter()
        .find(|row| row[1] == "nonumbering")
        .expect("nonumbering row");
    assert_eq!(nonumbering[5], "PARTIAL");

    let font = rows.iter().find(|row| row[1] == "font").expect("font row");
    assert_eq!(font[5], "PARTIAL");
    assert!(font[9].contains("IrFontState"));
    assert!(font[10].contains("size-only"));

    let paragraphstyle = rows
        .iter()
        .find(|row| row[1] == "paragraphstyle")
        .expect("paragraphstyle row");
    assert_eq!(paragraphstyle[5], "PARTIAL");
    assert!(paragraphstyle[9].contains("IrParagraphStyleInfo"));
    assert!(paragraphstyle[10].contains("omission/none preservation"));

    let pageformat = rows
        .iter()
        .find(|row| row[1] == "pageformat")
        .expect("pageformat row");
    assert!(pageformat[9].contains("IrDocumentState::page_columns"));
    assert!(pageformat[9].contains("quarkdown_pageformat_columns.rs"));
    assert!(pageformat[9].contains("IrDocumentState::page_border_widths"));
    assert!(pageformat[9].contains("quarkdown_pageformat_decorations.rs"));
    assert!(pageformat[9].contains("IrDocumentState::page_size"));
    assert!(pageformat[9].contains("quarkdown_pageformat_size_state.rs"));
    assert!(pageformat[9].contains("IrDocumentState::page_margin"));
    assert!(pageformat[9].contains("quarkdown_pageformat_margin.rs"));
    assert!(pageformat[9].contains("IrDocumentState::page_format"));
    assert!(pageformat[9].contains("IrResolvedPageFormat"));
    assert!(pageformat[9].contains("quarkdown_pageformat_layer_state.rs"));
    assert!(pageformat[9].contains("integration_pageformat_columns_lowers_to_valid_typst_and_pdf"));
    assert!(pageformat[9]
        .contains("integration_pageformat_ordered_global_columns_override_stale_flattened_state"));
    assert!(pageformat[9]
        .contains("integration_pageformat_ordered_global_margin_overrides_stale_flattened_state"));
    assert!(pageformat[9]
        .contains("integration_non_paged_page_selector_fails_closed_at_typst_boundary"));
    assert!(pageformat[9].contains(
        "integration_non_paged_selector_free_page_border_fails_closed_at_typst_boundary"
    ));
    assert!(pageformat[9].contains(
        "integration_page_selector_declared_before_paged_doctype_uses_final_document_type"
    ));
    assert!(pageformat[9]
        .contains("integration_selector_scoped_page_layout_fails_closed_at_typst_boundary"));
    assert!(pageformat[9]
        .contains("integration_selector_scoped_page_alignment_fails_closed_at_typst_boundary"));
    assert!(pageformat[9].contains(
        "integration_selector_scoped_margin_fails_closed_even_with_complete_border_path"
    ));
    assert!(
        pageformat[9].contains("integration_selector_scoped_border_defaults_fail_closed_per_page")
    );
    assert!(
        pageformat[9].contains("integration_pageformat_background_lowers_to_valid_typst_and_pdf")
    );
    assert!(pageformat[9].contains(
        "integration_pageformat_ordered_global_background_overrides_stale_flattened_state"
    ));
    assert!(pageformat[9]
        .contains("integration_pageformat_range_background_lowers_to_valid_typst_and_pdf"));
    assert!(
        pageformat[9].contains("integration_pageformat_range_border_lowers_to_valid_typst_and_pdf")
    );
    assert!(pageformat[9]
        .contains("integration_pageformat_side_decoration_lowers_to_valid_typst_and_pdf"));
    assert!(pageformat[9]
        .contains("integration_pageformat_standard_size_lowers_to_valid_typst_and_pdf"));
    assert!(pageformat[9].contains(
        "integration_pageformat_docs_omitted_orientation_fails_closed_after_paged_mutation"
    ));
    assert!(pageformat[9]
        .contains("integration_pageformat_global_single_axis_uses_existing_standard_size_base"));
    assert!(pageformat[9]
        .contains("integration_pageformat_global_single_axis_inherits_explicit_geometry_base"));
    assert!(pageformat[9]
        .contains("integration_pageformat_global_nullable_axes_inherit_existing_dimensions"));
    assert!(pageformat[9].contains(
        "integration_effectless_ordered_pageformat_does_not_revive_stale_flattened_geometry"
    ));
    assert!(pageformat[9]
        .contains("integration_pageformat_nullable_alignment_preserves_final_page_state"));
    assert!(pageformat[9]
        .contains("integration_v260_ordered_global_alignment_overrides_stale_flattened_state"));
    assert!(pageformat[9].contains(
        "integration_effectless_non_paged_page_selector_does_not_trigger_fail_closed_guard"
    ));
    assert!(pageformat[9]
        .contains("integration_pageformat_explicit_border_lowers_to_valid_typst_and_pdf"));
    assert!(pageformat[9]
        .contains("integration_pageformat_ordered_global_border_overrides_stale_flattened_state"));
    assert!(pageformat[10].contains("global positive columns"));
    assert!(pageformat[10].contains("selector-free global background"));
    assert!(pageformat[10].contains("selector-free/global background output"));
    assert!(pageformat[10].contains("merged background as canonical"));
    assert!(pageformat[10].contains("flattened page_background is a legacy fallback only"));
    assert!(pageformat[10].contains("Typst/PDF columns consumer"));
    assert!(pageformat[10].contains("selector-free/global columns output"));
    assert!(pageformat[10].contains("merged positive count as canonical"));
    assert!(pageformat[10].contains("flattened page_columns is a legacy fallback only"));
    assert!(pageformat[10].contains("canonical for global dimensions even when no dimension payload"));
    assert!(pageformat[10].contains("flattened page_geometry/page_size are legacy fallbacks only"));
    assert!(pageformat[10].contains("typed standard size/orientation"));
    assert!(pageformat[10].contains("named size binding"));
    assert!(pageformat[10].contains("selector-free global margin"));
    assert!(pageformat[10].contains("1/2/4-value Sizes shorthand"));
    assert!(pageformat[10].contains("selector-free/global margin output"));
    assert!(pageformat[10].contains("merged margin as canonical"));
    assert!(pageformat[10].contains("flattened page_margin is a legacy fallback only"));
    assert!(pageformat[10].contains("selector-free global border/background"));
    assert!(pageformat[10].contains("partial-side zeroing"));
    assert!(pageformat[10].contains("bordercolor-only width inheritance/no-fabrication"));
    assert!(pageformat[10].contains("Typst/PDF background consumer"));
    assert!(pageformat[10].contains("combined side+range background precedence"));
    assert!(pageformat[10].contains("physical 1-based here().page()"));
    assert!(pageformat[10].contains("left/verso to even physical pages"));
    assert!(pageformat[10].contains("combined side+range margin/border-width/border-color layers"));
    assert!(pageformat[10].contains("independently in source order"));
    assert!(pageformat[10].contains("left/even and right/odd physical-page parity"));
    assert!(pageformat[10].contains("explicit margin + committed widths + explicit color"));
    assert!(pageformat[10].contains("without fabricating renderer defaults"));
    assert!(pageformat[10].contains("ordered page-format state exists"));
    assert!(pageformat[10].contains("exact global selector group in source order"));
    assert!(pageformat[10].contains("flattened border fields are a legacy fallback only"));
    assert!(pageformat[10].contains("gates standard-size output on the final paged/slides"));
    assert!(pageformat[10].contains("explicit physical millimeter bounds"));
    assert!(pageformat[10].contains("omitted-orientation basis"));
    assert!(pageformat[10].contains("omitted docs basis remains fail-closed"));
    assert!(pageformat[10].contains(
        "explicit Typst panic when final paged/slides output cannot resolve concrete global dimensions"
    ));
    assert!(pageformat[10]
        .contains("remaining page-border semantics beyond the selector-free and per-page scoped completeness guards"));
    assert!(pageformat[10].contains(
        "selector-free implicit-margin/width-only/color-only cases are explicitly backend-rejected"
    ));
    assert!(pageformat[10].contains("instead of silently omitting the requested border"));
    assert!(pageformat[10].contains("selector-aware alignment/geometry/size/margin/columns output"));
    assert!(pageformat[10].contains("ordered selector snapshot"));
    assert!(pageformat[10].contains("typed left/right page-side selectors"));
    assert!(pageformat[10]
        .contains("selector state remains retained independently of the call-time document type"));
    assert!(pageformat[10].contains("final document type is plain/slides/docs"));
    assert!(pageformat[10].contains("selector declared before a later doctype:{paged} mutation"));
    assert!(pageformat[10].contains("finite 1-based page ranges"));
    assert!(pageformat[10].contains("left-open ranges normalized to page 1"));
    assert!(pageformat[10]
        .contains("left-open finite page ranges normalize the omitted start to page 1"));
    assert!(pageformat[10].contains("page ranges without a finite end remain fail-closed"));
    assert!(pageformat[10].contains("must not mutate flattened global state"));
    assert!(pageformat[10].contains("state-only prerequisite evidence"));
    assert!(pageformat[10].contains("compose_applicable_page_dimensions"));
    assert!(pageformat[10].contains("resolve_applicable_page_format"));
    assert!(pageformat[10].contains("resolved explicit-page snapshot"));
    assert!(pageformat[9].contains("resolve_concrete_page_geometry"));
    assert!(pageformat[10].contains("resolve_concrete_page_geometry"));
    assert!(pageformat[10].contains("complete explicit axes remain valid for any output type"));
    assert!(pageformat[10].contains("current Typst/PDF global dimension path reuses this helper"));
    assert!(pageformat[10].contains("compose_global_page_dimensions"));
    assert!(pageformat[10].contains("prior ordered global dimension state"));
    assert!(pageformat[10].contains("opposite explicit axis or a concrete standard-size base"));
    assert!(pageformat[10].contains("explicit nullable size:{.none}"));
    assert!(pageformat[10].contains(
        "explicit nullable width/height values contribute no axis override and inherit the previously composed axis"
    ));
    assert!(pageformat[10].contains(
        "semantic None contributes no alignment override, preserving the prior selector-free document alignment and the prior exact same-selector alignment"
    ));
    assert!(pageformat[10].contains(
        "empty/orientation-only/selector-only pageformat calls are retained as effectless ordered layers"
    ));
    assert!(pageformat[10]
        .contains("effectless selectors do not trigger the non-paged Typst selector guard"));
    assert!(
        pageformat[10].contains("truly base-less non-null single-axis calls remain fail-closed")
    );
    assert!(pageformat[10].contains("selector-scoped mixed size+axis"));
    assert!(pageformat[10].contains("selector-scoped positive columns"));
    assert!(pageformat[10].contains("current Typst/PDF column lowering remains global-only"));
    assert!(pageformat[10].contains("selector-scoped alignment/size/width/height/columns"));
    assert!(pageformat[10].contains("generated panic before page setup"));
    assert!(pageformat[10].contains(
        "selector-free page borders on final non-paged documents explicitly backend-fail-closed"
    ));
    assert!(pageformat[10].contains("global row/column inheritance consumer"));
    assert!(pageformat[10].contains("merged alignment as canonical"));
    assert!(pageformat[10].contains("flattened page_alignment remains a legacy fallback only"));
    assert!(pageformat[10].contains(
        "scoped alignment cannot silently disappear through the global-only page_alignment/stack-inheritance consumer"
    ));
    assert!(pageformat[10].contains("every selector-scoped paged margin"));
    assert!(pageformat[10]
        .contains("consuming scoped margin only as border inset would silently lose its content-layout semantics"));
    assert!(pageformat[10]
        .contains("scoped-border path remains supported when margin is inherited from selector-free/global state"));
    assert!(pageformat[10].contains("per-page selector-aware request flag"));
    assert!(pageformat[10]
        .contains("current page requests a border but margin, widths, or color remain unresolved"));
    assert!(pageformat[11].contains("bounded-standard-size-state"));
    assert!(pageformat[11].contains("bounded-global-explicit-axis-inheritance"));
    assert!(pageformat[11].contains("bounded-global-nullable-axis-inheritance"));
    assert!(pageformat[11].contains("bounded-nullable-alignment-inheritance"));
    assert!(pageformat[11].contains("bounded-effectless-pageformat-layers"));
    assert!(pageformat[11].contains("unresolved-global-page-dimensions-backend-fail-closed"));
    assert!(pageformat[11].contains("bounded-global-margin-state"));
    assert!(pageformat[11].contains("bounded-global-decoration-state"));
    assert!(pageformat[11].contains("bounded-range-background-output"));
    assert!(pageformat[11].contains("bounded-range-border-output"));
    assert!(pageformat[11].contains("bounded-side-decoration-output"));
    assert!(pageformat[11].contains("selector-aware-layering"));
    assert!(pageformat[11].contains("scoped-layout-backend-fail-closed"));
    assert!(pageformat[11].contains("scoped-alignment-backend-fail-closed"));
    assert!(pageformat[11].contains("scoped-margin-backend-fail-closed"));
    assert!(pageformat[11].contains("scoped-border-completeness-fail-closed"));
    assert!(pageformat[11].contains("non-paged-selector-backend-fail-closed"));
    assert!(pageformat[11].contains("non-paged-border-backend-fail-closed"));
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
