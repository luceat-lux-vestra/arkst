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
    assert_eq!(rows.iter().filter(|row| row[5] == "PARTIAL").count(), 15);
    assert_eq!(rows.iter().filter(|row| row[5] == "PARSED_ONLY").count(), 5);
    assert!(MANIFEST.contains(BASE_SHA));
    assert!(MANIFEST.contains("captionposition\tcaptionPosition\tcode"));
}

#[test]
fn audit_records_pipeline_boundary_and_state_rendering_separation() {
    assert!(AUDIT.contains("No additional #153-owned public callable was found"));
    assert!(AUDIT.contains("A preserved `IrNode::FunctionCall`"));
    assert!(AUDIT.contains("is not a successful setter, typed node, state mutation"));
    assert!(
        AUDIT.contains("No #153-owned row currently carries a `SUPPORTED_END_TO_END` v2.5.1 claim")
    );
    assert!(AUDIT.contains("15 conservative `PARTIAL` rows"));
}

#[test]
fn audit_records_bounded_page_counter_and_marker_semantics() {
    let rows = rows();
    for (name, target) in [("currentpage", "CURRENT"), ("totalpages", "TOTAL")] {
        let row = rows
            .iter()
            .find(|row| row[1] == name)
            .unwrap_or_else(|| panic!("missing page-counter row: {name}"));
        assert_eq!(row[5], "PARTIAL");
        assert!(row[9].contains("IrPageCounterTarget"));
        assert!(row[9].contains("quarkdown_page_counters.rs"));
        assert!(row[9].contains("typst-subprocess/tests/page_counters.rs"));
        assert!(row[10].contains(target));
        assert!(row[10].contains("Final paged/slides"));
        assert!(row[10].contains("plain/docs"));
        assert!(row[11].contains("bounded-page-counter-pair"));
    }

    let format = rows
        .iter()
        .find(|row| row[1] == "formatpagenumber")
        .expect("formatpagenumber row");
    assert_eq!(format[5], "PARTIAL");
    assert!(format[9].contains("IrInline::PageNumberFormat"));
    assert!(format[9].contains("quarkdown_page_number_markers.rs"));
    assert!(format[10].contains("last marker in document order"));
    assert!(format[10].contains("unknown formats literally"));
    assert!(format[11].contains("same-page-last-marker-wins"));

    let reset = rows
        .iter()
        .find(|row| row[1] == "resetpagenumber")
        .expect("resetpagenumber row");
    assert_eq!(reset[5], "PARTIAL");
    assert!(reset[9].contains("IrInline::PageNumberReset"));
    assert!(reset[10].contains("render time to positive values"));
    assert!(reset[10].contains("non-positive markers are retained but ignored"));
    assert!(reset[11].contains("same-page-last-valid-reset-wins"));

    assert!(AUDIT.contains("IrInline::PageCounter"));
    assert!(AUDIT.contains("IrInline::PageNumberFormat"));
    assert!(AUDIT.contains("IrInline::PageNumberReset"));
    assert!(AUDIT.contains("physical `location().page()`"));
    assert!(AUDIT.contains("later in source on the same"));
    assert!(AUDIT.contains("last positive reset"));
    assert!(AUDIT.contains("String.fromCharCode"));
    assert!(AUDIT.contains("move from `PARSED_ONLY` to"));
    assert!(AUDIT.contains("Final `plain` and `docs` emit neither marker runtime"));
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
        .contains("integration_pageformat_docs_columns_fails_closed_at_typst_boundary"));
    assert!(pageformat[9]
        .contains("integration_pageformat_ordered_global_columns_override_stale_flattened_state"));
    assert!(pageformat[9]
        .contains("integration_pageformat_ordered_global_margin_overrides_stale_flattened_state"));
    assert!(
        pageformat[9].contains("integration_pageformat_docs_margin_fails_closed_at_typst_boundary")
    );
    assert!(pageformat[9]
        .contains("integration_non_paged_page_selector_fails_closed_at_typst_boundary"));
    assert!(pageformat[9]
        .contains("integration_pageformat_explicit_border_lowers_on_plain_to_valid_typst_and_pdf"));
    assert!(pageformat[9]
        .contains("integration_docs_selector_free_page_border_fails_closed_at_typst_boundary"));
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
    assert!(
        pageformat[9].contains("integration_pageformat_standard_size_lowers_in_final_plain_output")
    );
    assert!(pageformat[9].contains(
        "integration_pageformat_docs_omitted_orientation_uses_portrait_after_paged_mutation"
    ));
    assert!(pageformat[9]
        .contains("integration_pageformat_paged_single_axis_uses_initial_a4_portrait_base"));
    assert!(pageformat[9]
        .contains("integration_pageformat_single_axis_declared_before_paged_uses_final_default"));
    assert!(pageformat[9]
        .contains("integration_pageformat_unresolved_single_axis_final_plain_fails_closed"));
    assert!(pageformat[9]
        .contains("integration_pageformat_paged_default_single_axis_does_not_cross_into_slides"));
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
    assert!(pageformat[9].contains(
        "integration_pageformat_explicit_border_lowers_on_slides_to_valid_typst_and_pdf"
    ));
    assert!(pageformat[9].contains("integration_pageformat_slides_global_margin_is_renderer_noop"));
    assert!(pageformat[9].contains(
        "integration_slides_selector_free_page_border_defaults_fail_closed_at_typst_boundary"
    ));
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
    assert!(
        pageformat[10].contains("canonical for global dimensions even when no dimension payload")
    );
    assert!(pageformat[10].contains("flattened page_geometry/page_size are legacy fallbacks only"));
    assert!(pageformat[10].contains("typed standard size/orientation"));
    assert!(pageformat[10].contains("named size binding"));
    assert!(pageformat[10].contains("selector-free global margin"));
    assert!(pageformat[10].contains("1/2/4-value Sizes shorthand"));
    assert!(pageformat[10].contains("selector-free/global margin output"));
    assert!(pageformat[10].contains("slide-margin documentation/runtime divergence"));
    assert!(pageformat[10].contains("does not reinterpret it as a Typst page inset"));
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
    assert!(pageformat[10].contains("plain/paged require explicit margin, widths, and color"));
    assert!(pageformat[10].contains("slides requires explicit widths and color"));
    assert!(pageformat[10].contains("full Reveal frame with zero inset"));
    assert!(pageformat[10].contains("without fabricating unresolved renderer defaults"));
    assert!(pageformat[10].contains("ordered page-format state exists"));
    assert!(pageformat[10].contains("exact global selector group in source order"));
    assert!(pageformat[10].contains("flattened border fields are a legacy fallback only"));
    assert!(pageformat[10]
        .contains("resolves standard-size selections to explicit physical millimeter bounds for every final document type"));
    assert!(
        pageformat[10].contains("pinned setter's concrete pageWidth/pageHeight materialization")
    );
    assert!(pageformat[10]
        .contains("omitted orientation uses the captured call-time document type preference"));
    assert!(pageformat[10].contains("including docs portrait"));
    assert!(pageformat[10].contains(
        "selector-aware alignment/geometry/size/margin/columns output is deliberately unsupported at the current Typst boundary"
    ));
    assert!(pageformat[10].contains("must not be approximated with here().page()"));
    assert!(pageformat[10].contains(
        "Color-only or otherwise implicit border-width output is likewise intentionally fail-closed"
    ));
    assert!(pageformat[10].contains("must not fabricate a Typst thickness"));
    assert!(pageformat[10].contains(
        "No further bounded pageformat output widening is authorized under #175 without new backend capability or pinned evidence"
    ));
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
    assert!(pageformat[10]
        .contains("pinned final rendering prepends the final document type's default page format"));
    assert!(pageformat[10].contains(
        "final paged output therefore inherits the missing axis from A4 portrait regardless of call-time doctype"
    ));
    assert!(pageformat[10].contains("without fabricating size into the stored layer"));
    assert!(pageformat[10].contains(
        "final plain/docs selector-free width-only output is lowered without fabricating height"
    ));
    assert!(pageformat[10].contains("height-only remains fail-closed"));
    assert!(pageformat[10].contains("selector-scoped mixed size+axis"));
    assert!(pageformat[10].contains("selector-scoped positive columns"));
    assert!(pageformat[10].contains("current Typst/PDF column lowering remains global-only"));
    assert!(pageformat[10].contains(
        "Final docs output rejects selector-free global margin/columns before page setup"
    ));
    assert!(
        pageformat[10].contains("pinned applicability limits those fields to plain/paged/slides")
    );
    assert!(pageformat[10].contains("selector-scoped alignment/size/width/height/columns"));
    assert!(pageformat[10].contains("generated panic before output"));
    assert!(pageformat[10].contains(
        "selector-free explicit-value page borders are supported for final plain, paged, and slides output"
    ));
    assert!(
        pageformat[10].contains("slides requires widths+color, uses a zero-inset full-slide frame")
    );
    assert!(pageformat[10].contains("final docs selector-free borders remain backend-fail-closed"));
    assert!(pageformat[10]
        .contains("pinned TextAlignment separates local justify from global start/center/end"));
    assert!(pageformat[10].contains(
        "selector-free justify lowers to Typst paragraph justification without fabricating document-global alignment"
    ));
    assert!(pageformat[10].contains(
        "selector-free start/center/end lower to Typst global alignment for every final document type"
    ));
    assert!(pageformat[10].contains(
        "final slides composes horizontal start/center/end with explicit .slides center true/false vertical horizon/top"
    ));
    assert!(pageformat[10].contains("both bounded selector-free alignment consumers"));
    assert!(pageformat[10].contains("merged alignment as canonical"));
    assert!(pageformat[10].contains("flattened page_alignment remains a legacy fallback only"));
    assert!(pageformat[10].contains(
        "scoped alignment cannot silently disappear through the selector-free-only alignment consumers"
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
    assert!(pageformat[11].contains("standard-size-all-final-types"));
    assert!(pageformat[11].contains("bounded-global-explicit-axis-inheritance"));
    assert!(pageformat[11].contains("bounded-paged-default-single-axis-base"));
    assert!(pageformat[11].contains("final-type-default-axis-merge"));
    assert!(pageformat[11].contains("bounded-global-nullable-axis-inheritance"));
    assert!(pageformat[11].contains("bounded-nullable-alignment-inheritance"));
    assert!(pageformat[11].contains("bounded-global-justify-text-output"));
    assert!(pageformat[11].contains("bounded-effectless-pageformat-layers"));
    assert!(pageformat[11].contains("unresolved-global-page-dimensions-backend-fail-closed"));
    assert!(pageformat[11].contains("bounded-global-margin-state"));
    assert!(pageformat[11].contains("docs-global-layout-applicability-fail-closed"));
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
    assert!(pageformat[11].contains("slides-global-border-output"));
    assert!(pageformat[11].contains("slides-global-margin-runtime-noop"));
    assert!(pageformat[11].contains("plain-global-border-output"));
    assert!(pageformat[11].contains("docs-border-backend-fail-closed"));
}

#[test]
fn audit_final_175_status_reconciliation_is_pinned() {
    let rows = rows();
    for name in [
        "numbering",
        "nonumbering",
        "font",
        "paragraphstyle",
        "pageformat",
        "autopagebreak",
        "noautopagebreak",
    ] {
        let row = rows
            .iter()
            .find(|row| row[1] == name)
            .unwrap_or_else(|| panic!("missing #175 row: {name}"));
        assert_eq!(
            row[5], "PARTIAL",
            "{name} must remain conservatively PARTIAL"
        );
        assert!(
            row[11].contains("final-175-status-reconciliation"),
            "{name} missing final #175 reconciliation marker"
        );
    }

    let auto = rows
        .iter()
        .find(|row| row[1] == "autopagebreak")
        .expect("autopagebreak row");
    assert!(auto[9].contains("typst-inprocess/tests/auto_page_break.rs"));
    assert!(auto[10].contains("global autoPageBreakHeadingMaxDepth=1"));
    assert!(auto[10].contains("plain=0/paged=1/slides=2/docs=0"));
    assert!(auto[10].contains("implicit-default version divergence"));

    let no_auto = rows
        .iter()
        .find(|row| row[1] == "noautopagebreak")
        .expect("noautopagebreak row");
    assert!(no_auto[10].contains("exact shorthand for explicit zero"));
    assert!(no_auto[10].contains("global threshold 1"));

    assert!(AUDIT.contains("### #175 final reconciliation snapshot — 2026-10-04"));
    assert!(AUDIT.contains("MutableContextOptions.autoPageBreakHeadingMaxDepth"));
    assert!(AUDIT.contains("non-null `Int` initialized to `1`"));
    assert!(AUDIT.contains("`DocumentType` has no per-document-type"));
    assert!(AUDIT.contains("However, later v2.6"));
    assert!(AUDIT.contains("adaptation changed the implicit default contract"));
    assert!(
        AUDIT.contains("No #153-owned row currently carries a `SUPPORTED_END_TO_END` v2.5.1 claim")
    );
    assert!(AUDIT.contains("ordered font"));
    assert!(AUDIT.contains("paragraph style"));
    assert!(AUDIT.contains("ordered page-format"));
}

#[test]
fn audit_records_bounded_page_margin_content_slice() {
    let rows = rows();
    for name in ["pagemargin", "footer"] {
        let row = rows
            .iter()
            .find(|row| row[1] == name)
            .unwrap_or_else(|| panic!("missing page-margin row: {name}"));
        assert_eq!(row[5], "PARTIAL");
        assert!(row[9].contains("quarkdown_page_margin_content.rs"));
        assert!(row[9].contains("typst-subprocess/tests/page_margin_content.rs"));
        assert!(row[11].contains("bounded-paged-central-repeated-content"));
    }

    let pagemargin = rows
        .iter()
        .find(|row| row[1] == "pagemargin")
        .expect("pagemargin row");
    assert!(pagemargin[9].contains("IrPageMarginPosition"));
    assert!(pagemargin[9].contains("IrNode::PageMarginContent"));
    assert!(pagemargin[10].contains("All 24 public positions"));
    assert!(pagemargin[10].contains("same-page last-wins"));
    assert!(pagemargin[10].contains("other 22 positions"));

    let footer = rows
        .iter()
        .find(|row| row[1] == "footer")
        .expect("footer row");
    assert!(footer[10].contains("BottomCenter"));

    assert!(AUDIT.contains("16 fixed corner/edge positions plus eight mirrored"));
    assert!(AUDIT.contains("last same-page initializer wins"));
    assert!(AUDIT.contains("location().page()"));
    assert!(AUDIT.contains("current physical `here().page()`"));
    assert!(AUDIT.contains("other 22"));
    assert!(AUDIT.contains("final `plain`/`slides`/`docs`"));
    assert!(AUDIT.contains("15 conservative `PARTIAL` rows"));
    assert!(AUDIT.contains("plus 5"));
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
    assert!(formatter[11].contains("same-page-last-marker-wins"));
    assert!(formatter[11].contains("page-level-formatter-divergence"));

    let reset = rows
        .iter()
        .find(|row| row[1] == "resetpagenumber")
        .expect("resetpagenumber row");
    assert!(reset[10].contains("non-positive markers are retained but ignored"));
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

#[test]
fn audit_pageformat_plain_docs_width_only_contract_is_explicit() {
    let pageformat = rows()
        .into_iter()
        .find(|row| row[1] == "pageformat")
        .expect("pageformat row");
    assert!(pageformat[10].contains("final plain/docs selector-free width-only output is lowered"));
    assert!(pageformat[11].contains("bounded-plain-docs-width-only-output"));
    assert!(AUDIT.contains(
        "pinned public applicability table explicitly permits width while excluding height"
    ));
    assert!(AUDIT.contains("height-only remains fail-closed"));
    assert!(!AUDIT.contains("plain/docs/slides output has no compatible page-format default"));
}

#[test]
fn audit_pageformat_standard_size_contract_matches_post_510_semantics() {
    assert!(AUDIT.contains("A standard-size base likewise"));
    assert!(AUDIT.contains(
        "resolves to closed-domain physical millimeter geometry for every final document type"
    ));
    assert!(AUDIT.contains("(`plain`/`paged`/`docs` portrait, `slides` landscape)"));
    assert!(AUDIT.contains(
        "The unresolved-dimension panic is reserved for genuinely unsupported or incomplete"
    ));
    assert!(AUDIT.contains(
        "`paged`/`slides` still require complete geometry or a resolvable standard-size base"
    ));
    assert!(!AUDIT.contains("the standard-size base on final `paged`/`slides` applicability"));
    assert!(!AUDIT.contains("keeps an omitted `docs` basis fail-closed"));
}
