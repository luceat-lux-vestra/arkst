//! Quarkdown v2.6 `focus` layout-theme adaptation for Typst output.
//!
//! The generic IR-to-Typst lowering stays renderer-neutral with respect to
//! named Quarkdown themes. This wrapper adds only the output behavior pinned
//! by the clean-room v2.6 `focus` oracle in #328, then shifts the existing
//! source-map ranges by the generated prelude length.

use arkst_ir::{IrDocument, IrDocumentType};
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

fn document_prelude(doc: &IrDocument) -> String {
    let state = &doc.metadata.document_state;
    let mut prelude = String::new();
    if let Some(geometry) = state.page_geometry.as_ref() {
        let width = lowering_base::lower_size(&geometry.width);
        let height = lowering_base::lower_size(&geometry.height);
        prelude.push_str(&format!("#set page(width: {width}, height: {height})\n"));
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
    if let Some(columns) = state.page_columns {
        prelude.push_str(&format!("#set page(columns: {columns})\n"));
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
    use arkst_ir::{IrDocumentTheme, IrMetadata, IrNode, IrPageMargins, IrSize, IrSizeUnit};
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
    fn global_page_columns_emit_typed_typst_page_prelude() {
        let mut doc = document(IrDocumentType::Paged, None, None);
        doc.metadata.document_state.page_columns = Some(3);

        let code = lower_to_typst_code(&doc);
        assert!(code.starts_with("#set page(columns: 3)\n"), "{code}");
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
