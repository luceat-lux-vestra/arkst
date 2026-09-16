from pathlib import Path
import re


def rep(s: str, old: str, new: str, label: str) -> str:
    n = s.count(old)
    if n != 1:
        raise SystemExit(f"{label}: expected 1 anchor, found {n}")
    return s.replace(old, new, 1)


ir_path = Path("crates/arkst-ir/src/lib.rs")
ir = ir_path.read_text()
if "pub page_geometry: Option<IrPageGeometry>" in ir:
    print("bounded page geometry patch already applied")
    raise SystemExit(0)

ir = rep(
    ir,
    "#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]\npub struct IrDocumentState {",
    "#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]\npub struct IrDocumentState {",
    "IrDocumentState derive",
)
marker = '    #[serde(default, skip_serializing_if = "Option::is_none")]\n    pub page_alignment: Option<IrDocumentAlignment>,\n'
ir = rep(
    ir,
    marker,
    marker
    + "    /// Explicit complete page geometry selected by the bounded `.pageformat`\n"
    + "    /// width/height slice. `None` preserves document-type/backend defaults.\n"
    + '    #[serde(default, skip_serializing_if = "Option::is_none")]\n'
    + "    pub page_geometry: Option<IrPageGeometry>,\n",
    "IR page geometry field",
)
marker = "/// Backend-neutral slides document configuration.\n"
geometry_type = (
    "/// Backend-neutral complete page geometry.\n"
    "///\n"
    "/// This stores only explicit semantic sizes. Document-type defaults and\n"
    "/// renderer-specific page dimensions remain backend-owned.\n"
    "#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]\n"
    "pub struct IrPageGeometry {\n"
    "    pub width: IrSize,\n"
    "    pub height: IrSize,\n"
    "}\n\n"
)
ir = rep(ir, marker, geometry_type + marker, "IrPageGeometry type")
ir_path.write_text(ir)

p = Path("crates/arkst-engine/src/evaluator.rs")
s = p.read_text()
s = rep(
    s,
    "    IrRange, IrRawBody, IrSize, IrSizeUnit, IrSlidesConfiguration, IrStackedComponent,\n",
    "    IrPageGeometry, IrRange, IrRawBody, IrSize, IrSizeUnit, IrSlidesConfiguration,\n"
    "    IrStackedComponent,\n",
    "IrPageGeometry import",
)
s = rep(
    s,
    "    page_alignment: Option<IrDocumentAlignment>,\n    slides: Option<IrSlidesConfiguration>,\n",
    "    page_alignment: Option<IrDocumentAlignment>,\n"
    "    page_geometry: Option<IrPageGeometry>,\n"
    "    slides: Option<IrSlidesConfiguration>,\n",
    "DocumentState field",
)
s = rep(
    s,
    "            page_alignment: None,\n            slides: None,\n",
    "            page_alignment: None,\n            page_geometry: None,\n            slides: None,\n",
    "DocumentState default",
)
s = rep(
    s,
    "            page_alignment: snapshot.page_alignment,\n            slides: snapshot.slides,\n",
    "            page_alignment: snapshot.page_alignment,\n"
    "            page_geometry: snapshot.page_geometry.clone(),\n"
    "            slides: snapshot.slides,\n",
    "DocumentState restore",
)
s = rep(
    s,
    "            page_alignment: self.page_alignment,\n            slides: self.slides,\n",
    "            page_alignment: self.page_alignment,\n"
    "            page_geometry: self.page_geometry.clone(),\n"
    "            slides: self.slides,\n",
    "DocumentState snapshot",
)
s = rep(
    s,
    "    PageAlignment,\n    Slides,\n",
    "    PageAlignment,\n    PageGeometry,\n    Slides,\n",
    "DocumentStateField",
)
s = rep(
    s,
    "    PageAlignment(Option<IrDocumentAlignment>),\n    Slides(Option<IrSlidesConfiguration>),\n",
    "    PageAlignment(Option<IrDocumentAlignment>),\n"
    "    PageGeometry(Option<IrPageGeometry>),\n"
    "    Slides(Option<IrSlidesConfiguration>),\n",
    "DocumentStateUndo",
)
s = rep(
    s,
    "            DocumentStateUndo::PageAlignment(previous) => state.page_alignment = previous,\n"
    "            DocumentStateUndo::Slides(previous) => state.slides = previous,\n",
    "            DocumentStateUndo::PageAlignment(previous) => state.page_alignment = previous,\n"
    "            DocumentStateUndo::PageGeometry(previous) => state.page_geometry = previous,\n"
    "            DocumentStateUndo::Slides(previous) => state.slides = previous,\n",
    "undo restore",
)
setter_anchor = "    fn set_slides_configuration(&self, value: Option<IrSlidesConfiguration>) {\n"
setter = (
    "    fn set_page_geometry(&self, value: Option<IrPageGeometry>) {\n"
    "        self.record_document_state_undo(DocumentStateField::PageGeometry, || {\n"
    "            (\n"
    "                DocumentStateUndo::PageGeometry(self.document_state.borrow().page_geometry.clone()),\n"
    "                0,\n"
    "            )\n"
    "        });\n"
    "        self.document_state.borrow_mut().page_geometry = value;\n"
    "    }\n\n"
)
s = rep(s, setter_anchor, setter + setter_anchor, "page geometry setter")

pattern = re.compile(
    r'        if name == "pageformat"\n.*?            return self\.evaluate_page_alignment_builtin\(named_args, span, diagnostics, context\);\n        }\n',
    re.S,
)
replacement = (
    '        if name == "pageformat"\n'
    "            && context.get_function(name).is_none()\n"
    "            && positional_args.is_empty()\n"
    "            && bounded_pageformat_shape(named_args)\n"
    "            && body.is_none()\n"
    "            && raw_body.is_none()\n"
    "            && lambda_parameters.is_none()\n"
    "        {\n"
    "            return self.evaluate_page_format_builtin(named_args, span, diagnostics, context);\n"
    "        }\n"
)
s, n = pattern.subn(replacement, s, count=1)
if n != 1:
    raise SystemExit(f"pageformat dispatch: expected 1 replacement, found {n}")

start = s.find("    fn evaluate_page_alignment_builtin(")
end = s.find("    #[allow(clippy::too_many_arguments)]", start)
if start < 0 or end < 0:
    raise SystemExit("pageformat evaluator function boundary not found")
function = '''    fn evaluate_page_format_builtin(
        &self,
        named_args: &[IrNamedArg],
        span: &SourceSpan,
        diagnostics: &mut Vec<Diagnostic>,
        context: &mut EvaluationContext<'_>,
    ) -> CallOutcome {
        let evaluated_named =
            match self.evaluate_invocation_named(named_args, span, diagnostics, context) {
                Ok(values) => values,
                Err(outcome) => return outcome,
            };

        let mut alignment = None;
        let mut width = None;
        let mut height = None;
        for argument in evaluated_named {
            let candidate_span = argument.arg.span;
            let parameter = argument.arg.name.clone();
            let value = InvocationValue {
                value: argument.arg.value,
                origin: argument.origin,
            };
            match parameter.as_str() {
                "alignment" => {
                    alignment = Some(
                        match value_conversion::convert_document_alignment_with_origin(&value) {
                            Ok(value) => value,
                            Err(error) => {
                                diagnostics.push(conversion_failure_diagnostic(
                                    value_conversion::ConversionFailure::new(
                                        error,
                                        Some(candidate_span),
                                        Some("alignment"),
                                        None,
                                        *span,
                                    ),
                                    Some("`.pageformat`"),
                                ));
                                return CallOutcome::Failed;
                            }
                        },
                    );
                }
                "width" | "height" => {
                    let size = match value_conversion::convert_domain_with_origin(
                        &value,
                        value_conversion::DomainTarget::Size,
                    ) {
                        Ok(value_conversion::DomainValue::Size(value)) => value,
                        Ok(_) => {
                            diagnostics.push(conversion_failure_diagnostic(
                                value_conversion::ConversionFailure::new(
                                    value_conversion::ConversionError::UnsupportedValue {
                                        target: value_conversion::ConversionTarget::Size,
                                    },
                                    Some(candidate_span),
                                    Some(parameter.clone()),
                                    None,
                                    *span,
                                ),
                                Some("`.pageformat`"),
                            ));
                            return CallOutcome::Failed;
                        }
                        Err(error) => {
                            diagnostics.push(conversion_failure_diagnostic(
                                value_conversion::ConversionFailure::new(
                                    error,
                                    Some(candidate_span),
                                    Some(parameter.clone()),
                                    None,
                                    *span,
                                ),
                                Some("`.pageformat`"),
                            ));
                            return CallOutcome::Failed;
                        }
                    };
                    if parameter == "width" {
                        width = Some(size);
                    } else {
                        height = Some(size);
                    }
                }
                _ => unreachable!("bounded pageformat shape rejected unknown parameter"),
            }
        }

        // Candidate evaluation and conversion complete before either bounded
        // state slot is published. The outer invocation transaction restores
        // nested document-state writes on any failure.
        if let Some(alignment) = alignment {
            context.set_page_alignment(Some(alignment));
        }
        if let (Some(width), Some(height)) = (width, height) {
            context.set_page_geometry(Some(IrPageGeometry { width, height }));
        }
        CallOutcome::NoValue
    }

'''
s = s[:start] + function + s[end:]

helper_anchor = "fn convert_whitespace_size(\n"
helper = '''fn bounded_pageformat_shape(named_args: &[IrNamedArg]) -> bool {
    let mut width = false;
    let mut height = false;
    let mut alignment = false;
    for argument in named_args {
        let seen = match argument.name.as_str() {
            "width" => &mut width,
            "height" => &mut height,
            "alignment" => &mut alignment,
            _ => return false,
        };
        if *seen || matches!(&argument.value, IrValue::None) {
            return false;
        }
        *seen = true;
    }
    (width && height) || (alignment && !width && !height)
}

'''
s = rep(s, helper_anchor, helper + helper_anchor, "bounded pageformat shape")
p.write_text(s)

p = Path("crates/arkst-typst/src/lowering.rs")
s = p.read_text()
s = rep(
    s,
    "fn lower_size(size: &IrSize) -> String {",
    "pub(crate) fn lower_size(size: &IrSize) -> String {",
    "lower_size visibility",
)
p.write_text(s)

p = Path("crates/arkst-typst/src/focus_lowering.rs")
s = p.read_text()
old = '''    if state.document_type == IrDocumentType::Slides {
        prelude.push_str(&format!(
            "#set page(width: {SLIDES_PAGE_WIDTH_PT}pt, height: {SLIDES_PAGE_HEIGHT_PT}pt)\\n"
        ));
        match state.slides.and_then(|slides| slides.center) {
            Some(true) => prelude.push_str("#set align(horizon)\\n"),
            Some(false) => prelude.push_str("#set align(top)\\n"),
            None => {}
        }
    }
'''
new = '''    if let Some(geometry) = state.page_geometry.as_ref() {
        let width = lowering_base::lower_size(&geometry.width);
        let height = lowering_base::lower_size(&geometry.height);
        prelude.push_str(&format!("#set page(width: {width}, height: {height})\\n"));
    } else if state.document_type == IrDocumentType::Slides {
        prelude.push_str(&format!(
            "#set page(width: {SLIDES_PAGE_WIDTH_PT}pt, height: {SLIDES_PAGE_HEIGHT_PT}pt)\\n"
        ));
    }
    if state.document_type == IrDocumentType::Slides {
        match state.slides.and_then(|slides| slides.center) {
            Some(true) => prelude.push_str("#set align(horizon)\\n"),
            Some(false) => prelude.push_str("#set align(top)\\n"),
            None => {}
        }
    }
'''
s = rep(s, old, new, "Typst geometry precedence")
p.write_text(s)
