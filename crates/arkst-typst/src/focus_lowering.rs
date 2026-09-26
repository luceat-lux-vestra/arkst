//! Quarkdown v2.6 `focus` layout-theme adaptation for Typst output.
//!
//! The generic IR-to-Typst lowering stays renderer-neutral with respect to
//! named Quarkdown themes. This wrapper adds only the output behavior pinned
//! by the clean-room v2.6 `focus` oracle in #328, then shifts the existing
//! source-map ranges by the generated prelude length.

use arkst_ir::{
    IrDocument, IrDocumentType, IrPageOrientation, IrPageSizeFormat, IrPageSizeSelection,
};
use arkst_source::SourceMapEntry;

use crate::lowering_base;

const PRELUDE_MARKER: &str = "// Arkst Quarkdown v2.6 focus layout\n";

/// Lower an Arkst IR document to Typst source, applying the bounded Quarkdown
/// v2.6 `focus` layout adaptation when the evaluated document state requests it.
pub fn lower_to_typst(doc: &IrDocument) -> (String, Vec<SourceMapEntry>) {
    let (body, mut source_map) = lowering_base::lower_to_typst(doc);
    let prelude = document_prelude(doc);
    if prelude.is_empty() {
        return (body, source_map);
    }
    let offset = prelude.len();
    for entry in &mut source_map {
        entry.generated_start += offset;
        entry.generated_end += offset;
    }
    let mut output = String::with_capacity(offset + body.len());
    output.push_str(&prelude);
    output.push_str(&body);
    (output, source_map)
}

/// Lower an Arkst IR document to Typst source without returning its source map.
pub fn lower_to_typst_code(doc: &IrDocument) -> String {
    let body = lowering_base::lower_to_typst_code(doc);
    let prelude = document_prelude(doc);
    if prelude.is_empty() {
        return body;
    }
    let mut output = String::with_capacity(prelude.len() + body.len());
    output.push_str(&prelude);
    output.push_str(&body);
    output
}

// v2.6 distributed-runtime PDF observation; renderer adapter defaults only.
const SLIDES_PAGE_WIDTH_PT: f64 = 749.04;
const SLIDES_PAGE_HEIGHT_PT: f64 = 546.0;

fn standard_page_dimensions_mm(
    selection: IrPageSizeSelection,
    output_document_type: IrDocumentType,
) -> Option<(&'static str, &'static str)> {
    if !matches!(
        output_document_type,
        IrDocumentType::Paged | IrDocumentType::Slides
    ) {
        return None;
    }

    // Resolve the closed Quarkdown standard-size domain to physical bounds at
    // the backend boundary. Keeping the dimensions explicit also covers B0,
    // which has no Typst paper-name alias.
    let portrait = match selection.format {
        IrPageSizeFormat::A0 => ("841", "1189"),
        IrPageSizeFormat::A1 => ("594", "841"),
        IrPageSizeFormat::A2 => ("420", "594"),
        IrPageSizeFormat::A3 => ("297", "420"),
        IrPageSizeFormat::A4 => ("210", "297"),
        IrPageSizeFormat::A5 => ("148", "210"),
        IrPageSizeFormat::A6 => ("105", "148"),
        IrPageSizeFormat::A7 => ("74", "105"),
        IrPageSizeFormat::A8 => ("52", "74"),
        IrPageSizeFormat::A9 => ("37", "52"),
        IrPageSizeFormat::A10 => ("26", "37"),
        IrPageSizeFormat::B0 => ("1000", "1414"),
        IrPageSizeFormat::B1 => ("707", "1000"),
        IrPageSizeFormat::B2 => ("500", "707"),
        IrPageSizeFormat::B3 => ("353", "500"),
        IrPageSizeFormat::B4 => ("250", "353"),
        IrPageSizeFormat::B5 => ("176", "250"),
        IrPageSizeFormat::Letter => ("215.9", "279.4"),
        IrPageSizeFormat::Legal => ("215.9", "355.6"),
        IrPageSizeFormat::Ledger => ("279.4", "431.8"),
    };
    let orientation = match selection.orientation {
        Some(orientation) => orientation,
        None => match selection.document_type {
            IrDocumentType::Slides => IrPageOrientation::Landscape,
            IrDocumentType::Plain | IrDocumentType::Paged => IrPageOrientation::Portrait,
            IrDocumentType::Docs => return None,
        },
    };

    Some(match orientation {
        IrPageOrientation::Portrait => portrait,
        IrPageOrientation::Landscape => (portrait.1, portrait.0),
    })
}

fn page_border_foreground(doc: &IrDocument) -> Option<String> {
    let state = &doc.metadata.document_state;
    if state.document_type != IrDocumentType::Paged {
        return None;
    }

    // Typst's page element has no stroke parameter. The bounded selector-free
    // border path therefore uses page foreground coordinates, but only when
    // every value needed to locate and paint the content-area rectangle is
    // explicit. Missing margin, width, or color stays fail-closed rather than
    // inventing renderer defaults.
    let margin = state.page_margin.as_ref()?;
    let widths = state.page_border_widths.as_ref()?;
    let color = state.page_border_color.as_ref()?;

    let margin_top = lowering_base::lower_size(&margin.top);
    let margin_right = lowering_base::lower_size(&margin.right);
    let margin_bottom = lowering_base::lower_size(&margin.bottom);
    let margin_left = lowering_base::lower_size(&margin.left);
    let border_top = lowering_base::lower_size(&widths.top);
    let border_right = lowering_base::lower_size(&widths.right);
    let border_bottom = lowering_base::lower_size(&widths.bottom);
    let border_left = lowering_base::lower_size(&widths.left);
    let paint = lowering_base::lower_color(color);

    Some(format!(
        "#set page(foreground: place(\n\
  top + left,\n\
  dx: {margin_left},\n\
  dy: {margin_top},\n\
  rect(\n\
    width: (100% - {margin_left} - {margin_right}),\n\
    height: (100% - {margin_top} - {margin_bottom}),\n\
    stroke: (\n\
      top: (paint: {paint}, thickness: {border_top}),\n\
      right: (paint: {paint}, thickness: {border_right}),\n\
      bottom: (paint: {paint}, thickness: {border_bottom}),\n\
      left: (paint: {paint}, thickness: {border_left}),\n\
    ),\n\
    inset: 0pt,\n\
  ),\n\
))\n"
    ))
}

fn document_prelude(doc: &IrDocument) -> String {
    let state = &doc.metadata.document_state;
    let mut prelude = String::new();
    if let Some(geometry) = state.page_geometry.as_ref() {
        let width = lowering_base::lower_size(&geometry.width);
        let height = lowering_base::lower_size(&geometry.height);
        prelude.push_str(&format!("#set page(width: {width}, height: {height})\n"));
    } else if let Some((width, height)) = state
        .page_size
        .and_then(|selection| standard_page_dimensions_mm(selection, state.document_type))
    {
        prelude.push_str(&format!(
            "#set page(width: {width}mm, height: {height}mm)\n"
        ));
    } else if state.document_type == IrDocumentType::Slides {
        prelude.push_str(&format!(
            "#set page(width: {SLIDES_PAGE_WIDTH_PT}pt, height: {SLIDES_PAGE_HEIGHT_PT}pt)\n"
        ));
    }
    if let Some(margin) = state.page_margin.as_ref() {
        let top = lowering_base::lower_size(&margin.top);
        let right = lowering_base::lower_size(&margin.right);
        let bottom = lowering_base::lower_size(&margin.bottom);
        let left = lowering_base::lower_size(&margin.left);
        prelude.push_str(&format!(
            "#set page(margin: (top: {top}, right: {right}, bottom: {bottom}, left: {left}))\n"
        ));
    }
    if let Some(border) = page_border_foreground(doc) {
        prelude.push_str(&border);
    }
    if let Some(columns) = state.page_columns {
        prelude.push_str(&format!("#set page(columns: {columns})\n"));
    }
    if let Some(background) = state.page_background.as_ref() {
        let fill = lowering_base::lower_color(background);
        prelude.push_str(&format!("#set page(fill: {fill})\n"));
    }
    if state.document_type == IrDocumentType::Slides {
        match state.slides.and_then(|slides| slides.center) {
            Some(true) => prelude.push_str("#set align(horizon)\n"),
            Some(false) => prelude.push_str("#set align(top)\n"),
            None => {}
        }
    }
    if let Some(focus) = focus_prelude(doc) {
        prelude.push_str(&focus);
    }
    prelude
}

fn focus_prelude(doc: &IrDocument) -> Option<String> {
    let state = &doc.metadata.document_state;
    let theme = state.theme.as_ref()?;
    if theme.layout.as_deref() != Some("focus") {
        return None;
    }

    // The v2.6 black-box oracle showed no observed computed-style/geometry
    // delta between `latex` and `focus` for the paged fixture. `docs` was not
    // part of the pinned public/black-box applicability evidence. Both remain
    // unchanged rather than inventing renderer semantics.
    let kind = match state.document_type {
        IrDocumentType::Plain => FocusDocumentKind::Plain,
        IrDocumentType::Slides => FocusDocumentKind::Slides,
        IrDocumentType::Paged | IrDocumentType::Docs => return None,
    };

    // The oracle showed omitted color and explicit `paperwhite` producing the
    // same computed focus result. Other color-theme combinations were not
    // pinned, so they receive focus geometry/scale but not invented colors.
    let paperwhite = matches!(theme.color.as_deref(), None | Some("paperwhite"));
    Some(render_focus_prelude(kind, paperwhite))
}

#[derive(Clone, Copy)]
enum FocusDocumentKind {
    Plain,
    Slides,
}

fn render_focus_prelude(kind: FocusDocumentKind, paperwhite: bool) -> String {
    let (h1_above, h1_top_inset, h2_above) = match kind {
        FocusDocumentKind::Plain => ("6.36em", "8.18em", "2.27em"),
        FocusDocumentKind::Slides => ("0em", "10.91em", "0em"),
    };
    let heading_fill = if paperwhite {
        "  fill: rgb(27, 24, 24, 90%),\n"
    } else {
        ""
    };
    let h1_text_fill = if paperwhite { "fill: white, " } else { "" };
    let h2_text_fill = h1_text_fill;

    format!(
        "{PRELUDE_MARKER}\
// Observable v2.6 focus body scale: 17.6px vs 16px in the control.\n\
#set text(size: 1.1em)\n\
// Typst raw defaults to 0.8em; focus code observed at 0.75 of body text.\n\
#show raw.where(block: true): set text(size: 0.75em / 0.8)\n\
// Upstream focus uses Source Sans Pro/Fira Sans/Noto Sans Mono. Arkst's\n\
// deterministic in-process world does not guarantee those sans families, so\n\
// font-family substitution is intentionally not encoded here.\n\
#show heading.where(level: 1): it => block(\n\
  width: 100%,\n\
  above: {h1_above},\n\
  below: 3.64em,\n\
  inset: (top: {h1_top_inset}, bottom: 0.91em, left: 0.75em, right: 0.75em),\n\
{heading_fill}\
  text({h1_text_fill}size: 2.75em, weight: \"bold\", it.body),\n\
)\n\
#show heading.where(level: 2): it => block(\n\
  width: 100%,\n\
  above: {h2_above},\n\
  below: 1.82em,\n\
  inset: (top: 0.8em, bottom: 0.8em, left: 0.75em, right: 0.75em),\n\
{heading_fill}\
  text({h2_text_fill}size: 1.87em, weight: \"regular\", it.body),\n\
)\n\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use arkst_ir::{
        IrColor, IrDocumentTheme, IrMetadata, IrNode, IrPageBorderWidths, IrPageGeometry,
        IrPageMargins, IrPageOrientation, IrPageSizeFormat, IrPageSizeSelection, IrSize,
        IrSizeUnit,
    };
    use arkst_source::{SourceId, SourceSpan};

    fn document(
        document_type: IrDocumentType,
        color: Option<&str>,
        layout: Option<&str>,
    ) -> IrDocument {
        let mut metadata = IrMetadata::default();
        metadata.document_state.document_type = document_type;
        metadata.document_state.theme = Some(IrDocumentTheme {
            color: color.map(str::to_string),
            layout: layout.map(str::to_string),
        });
        IrDocument {
            nodes: vec![IrNode::CodeBlock {
                language: Some("text".to_string()),
                info: Some("text".to_string()),
                source: "hello".to_string(),
                line_numbers: None,
                callouts: Vec::new(),
                span: SourceSpan::new(SourceId(7), 10, 20),
            }],
            metadata,
        }
    }

    #[test]
    fn standard_page_formats_resolve_to_expected_portrait_dimensions() {
        let cases = [
            (IrPageSizeFormat::A0, ("841", "1189")),
            (IrPageSizeFormat::A1, ("594", "841")),
            (IrPageSizeFormat::A2, ("420", "594")),
            (IrPageSizeFormat::A3, ("297", "420")),
            (IrPageSizeFormat::A4, ("210", "297")),
            (IrPageSizeFormat::A5, ("148", "210")),
            (IrPageSizeFormat::A6, ("105", "148")),
            (IrPageSizeFormat::A7, ("74", "105")),
            (IrPageSizeFormat::A8, ("52", "74")),
            (IrPageSizeFormat::A9, ("37", "52")),
            (IrPageSizeFormat::A10, ("26", "37")),
            (IrPageSizeFormat::B0, ("1000", "1414")),
            (IrPageSizeFormat::B1, ("707", "1000")),
            (IrPageSizeFormat::B2, ("500", "707")),
            (IrPageSizeFormat::B3, ("353", "500")),
            (IrPageSizeFormat::B4, ("250", "353")),
            (IrPageSizeFormat::B5, ("176", "250")),
            (IrPageSizeFormat::Letter, ("215.9", "279.4")),
            (IrPageSizeFormat::Legal, ("215.9", "355.6")),
            (IrPageSizeFormat::Ledger, ("279.4", "431.8")),
        ];

        for (format, expected) in cases {
            assert_eq!(
                standard_page_dimensions_mm(
                    IrPageSizeSelection {
                        format,
                        orientation: Some(IrPageOrientation::Portrait),
                        document_type: IrDocumentType::Paged,
                    },
                    IrDocumentType::Paged,
                ),
                Some(expected),
                "{format:?}"
            );
        }
    }

    #[test]
    fn global_standard_page_size_emits_oriented_physical_typst_dimensions() {
        let mut doc = document(IrDocumentType::Paged, None, None);
        doc.metadata.document_state.page_size = Some(IrPageSizeSelection {
            format: IrPageSizeFormat::A4,
            orientation: Some(IrPageOrientation::Landscape),
            document_type: IrDocumentType::Paged,
        });

        let code = lower_to_typst_code(&doc);
        assert!(
            code.starts_with("#set page(width: 297mm, height: 210mm)\n"),
            "{code}"
        );
    }

    #[test]
    fn standard_size_applicability_uses_output_type_but_default_orientation_uses_call_time_basis() {
        let mut doc = document(IrDocumentType::Paged, None, None);
        doc.metadata.document_state.page_size = Some(IrPageSizeSelection {
            format: IrPageSizeFormat::A4,
            orientation: None,
            document_type: IrDocumentType::Plain,
        });

        let code = lower_to_typst_code(&doc);
        assert!(
            code.starts_with("#set page(width: 210mm, height: 297mm)\n"),
            "{code}"
        );

        doc.metadata.document_state.document_type = IrDocumentType::Slides;
        let code = lower_to_typst_code(&doc);
        assert!(
            code.starts_with("#set page(width: 210mm, height: 297mm)\n"),
            "captured plain orientation basis must remain portrait after later doctype mutation: {code}"
        );

        doc.metadata.document_state.document_type = IrDocumentType::Plain;
        doc.metadata.document_state.page_size = Some(IrPageSizeSelection {
            format: IrPageSizeFormat::A4,
            orientation: Some(IrPageOrientation::Landscape),
            document_type: IrDocumentType::Paged,
        });
        let code = lower_to_typst_code(&doc);
        assert!(
            !code.starts_with("#set page(width: 297mm, height: 210mm)\n"),
            "standard size must not apply to a final plain document: {code}"
        );
    }

    #[test]
    fn omitted_docs_orientation_basis_remains_fail_closed() {
        let selection = IrPageSizeSelection {
            format: IrPageSizeFormat::A4,
            orientation: None,
            document_type: IrDocumentType::Docs,
        };
        assert_eq!(
            standard_page_dimensions_mm(selection, IrDocumentType::Paged),
            None,
            "public evidence does not define an omitted docs orientation for this cross-doctype edge"
        );
    }

    #[test]
    fn explicit_geometry_keeps_precedence_until_layer_ordering_is_represented() {
        let mut doc = document(IrDocumentType::Paged, None, None);
        doc.metadata.document_state.page_geometry = Some(IrPageGeometry {
            width: IrSize {
                value: 10.0,
                unit: IrSizeUnit::In,
            },
            height: IrSize {
                value: 5.0,
                unit: IrSizeUnit::In,
            },
        });
        doc.metadata.document_state.page_size = Some(IrPageSizeSelection {
            format: IrPageSizeFormat::A4,
            orientation: Some(IrPageOrientation::Landscape),
            document_type: IrDocumentType::Paged,
        });

        let code = lower_to_typst_code(&doc);
        assert!(
            code.starts_with("#set page(width: 10in, height: 5in)\n"),
            "{code}"
        );
        assert!(!code.contains("297mm"), "{code}");
    }

    #[test]
    fn global_page_columns_emit_typed_typst_page_prelude() {
        let mut doc = document(IrDocumentType::Paged, None, None);
        doc.metadata.document_state.page_columns = Some(3);

        let code = lower_to_typst_code(&doc);
        assert!(code.starts_with("#set page(columns: 3)\n"), "{code}");
    }

    #[test]
    fn explicit_paged_border_emits_content_area_foreground() {
        let mut doc = document(IrDocumentType::Paged, None, None);
        doc.metadata.document_state.page_margin = Some(IrPageMargins {
            top: IrSize {
                value: 1.0,
                unit: IrSizeUnit::Pt,
            },
            right: IrSize {
                value: 2.0,
                unit: IrSizeUnit::Pt,
            },
            bottom: IrSize {
                value: 3.0,
                unit: IrSizeUnit::Pt,
            },
            left: IrSize {
                value: 4.0,
                unit: IrSizeUnit::Pt,
            },
        });
        doc.metadata.document_state.page_border_widths = Some(IrPageBorderWidths {
            top: IrSize {
                value: 5.0,
                unit: IrSizeUnit::Pt,
            },
            right: IrSize {
                value: 6.0,
                unit: IrSizeUnit::Pt,
            },
            bottom: IrSize {
                value: 7.0,
                unit: IrSizeUnit::Pt,
            },
            left: IrSize {
                value: 8.0,
                unit: IrSizeUnit::Pt,
            },
        });
        doc.metadata.document_state.page_border_color = Some(IrColor {
            red: 10,
            green: 20,
            blue: 30,
            alpha: 0.5,
        });

        let code = lower_to_typst_code(&doc);
        assert!(code.contains("#set page(foreground: place("), "{code}");
        assert!(code.contains("dx: 4pt"), "{code}");
        assert!(code.contains("dy: 1pt"), "{code}");
        assert!(code.contains("width: (100% - 4pt - 2pt)"), "{code}");
        assert!(code.contains("height: (100% - 1pt - 3pt)"), "{code}");
        assert!(
            code.contains("top: (paint: rgb(10, 20, 30, 50%), thickness: 5pt)"),
            "{code}"
        );
        assert!(
            code.contains("left: (paint: rgb(10, 20, 30, 50%), thickness: 8pt)"),
            "{code}"
        );
    }

    #[test]
    fn page_border_output_keeps_unresolved_defaults_fail_closed() {
        let margins = IrPageMargins {
            top: IrSize {
                value: 1.0,
                unit: IrSizeUnit::Pt,
            },
            right: IrSize {
                value: 1.0,
                unit: IrSizeUnit::Pt,
            },
            bottom: IrSize {
                value: 1.0,
                unit: IrSizeUnit::Pt,
            },
            left: IrSize {
                value: 1.0,
                unit: IrSizeUnit::Pt,
            },
        };
        let widths = IrPageBorderWidths {
            top: IrSize {
                value: 1.0,
                unit: IrSizeUnit::Pt,
            },
            right: IrSize {
                value: 1.0,
                unit: IrSizeUnit::Pt,
            },
            bottom: IrSize {
                value: 1.0,
                unit: IrSizeUnit::Pt,
            },
            left: IrSize {
                value: 1.0,
                unit: IrSizeUnit::Pt,
            },
        };
        let color = IrColor {
            red: 255,
            green: 0,
            blue: 0,
            alpha: 1.0,
        };

        let mut doc = document(IrDocumentType::Paged, None, None);
        doc.metadata.document_state.page_margin = Some(margins.clone());
        doc.metadata.document_state.page_border_widths = Some(widths.clone());
        assert!(!lower_to_typst_code(&doc).contains("page(foreground:"));

        doc.metadata.document_state.page_border_widths = None;
        doc.metadata.document_state.page_border_color = Some(color.clone());
        assert!(!lower_to_typst_code(&doc).contains("page(foreground:"));

        doc.metadata.document_state.page_margin = None;
        doc.metadata.document_state.page_border_widths = Some(widths.clone());
        assert!(!lower_to_typst_code(&doc).contains("page(foreground:"));

        doc.metadata.document_state.page_margin = Some(margins);
        doc.metadata.document_state.document_type = IrDocumentType::Slides;
        assert!(!lower_to_typst_code(&doc).contains("page(foreground:"));
    }

    #[test]
    fn global_page_background_emits_typed_typst_page_fill() {
        let mut doc = document(IrDocumentType::Paged, None, None);
        doc.metadata.document_state.page_background = Some(IrColor {
            red: 1,
            green: 2,
            blue: 3,
            alpha: 0.5,
        });

        let code = lower_to_typst_code(&doc);
        assert!(
            code.starts_with("#set page(fill: rgb(1, 2, 3, 50%))\n"),
            "{code}"
        );
    }

    #[test]
    fn global_page_margin_emits_typed_typst_page_prelude() {
        let mut doc = document(IrDocumentType::Paged, None, None);
        doc.metadata.document_state.page_margin = Some(IrPageMargins {
            top: IrSize {
                value: 1.0,
                unit: IrSizeUnit::Cm,
            },
            right: IrSize {
                value: 2.0,
                unit: IrSizeUnit::Mm,
            },
            bottom: IrSize {
                value: 3.0,
                unit: IrSizeUnit::Pt,
            },
            left: IrSize {
                value: 8.0,
                unit: IrSizeUnit::Px,
            },
        });

        let code = lower_to_typst_code(&doc);
        assert!(
            code.starts_with("#set page(margin: (top: 1cm, right: 2mm, bottom: 3pt, left: 6pt))\n"),
            "{code}"
        );
    }

    #[test]
    fn plain_focus_adds_evidenced_prelude_and_shifts_source_map() {
        let doc = document(IrDocumentType::Plain, Some("paperwhite"), Some("focus"));
        let (code, map) = lower_to_typst(&doc);
        let prelude_len = focus_prelude(&doc).expect("focus prelude").len();

        assert!(code.starts_with(PRELUDE_MARKER), "{code}");
        assert!(code.contains("#set text(size: 1.1em)"), "{code}");
        assert!(code.contains("fill: rgb(27, 24, 24, 90%)"), "{code}");
        assert!(code.contains("size: 2.75em"), "{code}");
        assert!(code.contains("size: 1.87em"), "{code}");
        assert!(code.contains("0.75em / 0.8"), "{code}");
        assert!(code.ends_with("```text\nhello\n```\n\n"), "{code}");

        assert_eq!(map.len(), 1, "{map:?}");
        assert!(map[0].generated_start >= prelude_len, "{map:?}");
        assert_eq!(map[0].original, SourceSpan::new(SourceId(7), 10, 20));
    }

    #[test]
    fn omitted_color_matches_explicit_paperwhite_focus_prelude() {
        let omitted = document(IrDocumentType::Plain, None, Some("focus"));
        let explicit = document(IrDocumentType::Plain, Some("paperwhite"), Some("focus"));
        assert_eq!(focus_prelude(&omitted), focus_prelude(&explicit));
    }

    #[test]
    fn slides_focus_uses_slide_specific_observed_heading_geometry() {
        let doc = document(IrDocumentType::Slides, Some("paperwhite"), Some("focus"));
        let code = lower_to_typst_code(&doc);
        assert!(code.contains("above: 0em"), "{code}");
        assert!(code.contains("top: 10.91em"), "{code}");
    }

    #[test]
    fn paged_focus_and_non_focus_layouts_preserve_legacy_lowering() {
        for doc in [
            document(IrDocumentType::Paged, Some("paperwhite"), Some("focus")),
            document(IrDocumentType::Plain, Some("paperwhite"), Some("latex")),
        ] {
            let code = lower_to_typst_code(&doc);
            assert!(!code.contains(PRELUDE_MARKER), "{code}");
            assert_eq!(code, "```text\nhello\n```\n\n");
        }
    }

    #[test]
    fn unpinned_color_theme_keeps_focus_geometry_without_inventing_colors() {
        let doc = document(IrDocumentType::Plain, Some("custom"), Some("focus"));
        let code = lower_to_typst_code(&doc);
        assert!(code.starts_with(PRELUDE_MARKER), "{code}");
        assert!(code.contains("size: 2.75em"), "{code}");
        assert!(!code.contains("fill: rgb(27, 24, 24, 90%)"), "{code}");
        assert!(!code.contains("fill: white"), "{code}");
    }
}
