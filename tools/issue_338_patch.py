from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected exactly one match, found {count}: {old[:120]!r}")
    file.write_text(text.replace(old, new, 1))


# Backend-neutral IR: keep document-global text/page alignment separate from
# stacked main-axis alignment. None means no explicit page-format alignment
# layer has committed state.
replace_once(
    "crates/arkst-ir/src/lib.rs",
    '''    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_page_break_max_depth: Option<u32>,
}

/// Backend-neutral locale data retained by the bounded `.doclang` slice.
''',
    '''    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_page_break_max_depth: Option<u32>,
    /// Explicit document-global horizontal alignment selected by the bounded
    /// alignment-only `.pageformat` slice. `None` preserves the document and
    /// backend default. This is intentionally distinct from stacked main-axis
    /// alignment because `justify` is valid here but not for `.row`/`.column`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_alignment: Option<IrDocumentAlignment>,
}

/// Backend-neutral document-global horizontal alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum IrDocumentAlignment {
    Start,
    Center,
    End,
    Justify,
}

/// Backend-neutral locale data retained by the bounded `.doclang` slice.
''',
)

# Existing explicit IR-state fixtures must name the new field so the crate's
# own unit tests continue to compile while still exercising deterministic
# serialization of the complete state shape.
replace_once(
    "crates/arkst-ir/src/lib.rs",
    '''                auto_page_break_max_depth: Some(2),
            },
            ..IrMetadata::default()
''',
    '''                auto_page_break_max_depth: Some(2),
                page_alignment: None,
            },
            ..IrMetadata::default()
''',
)
replace_once(
    "crates/arkst-ir/src/lib.rs",
    '''            auto_page_break_max_depth: None,
        };
        let serialized = serde_json::to_string(&state).expect("ordered author state serializes");
''',
    '''            auto_page_break_max_depth: None,
            page_alignment: None,
        };
        let serialized = serde_json::to_string(&state).expect("ordered author state serializes");
''',
)

# Closed conversion domain used only by the bounded page-format state owner.
replace_once(
    "crates/arkst-engine/src/value_conversion.rs",
    '''    IrCallable, IrCaptionPosition, IrColor, IrContainerAlignment, IrCrossAxisAlignment,
    IrDocumentType, IrEnumValue, IrInline, IrMainAxisAlignment, IrNamedArg, IrNode, IrRange,
''',
    '''    IrCallable, IrCaptionPosition, IrColor, IrContainerAlignment, IrCrossAxisAlignment,
    IrDocumentAlignment, IrDocumentType, IrEnumValue, IrInline, IrMainAxisAlignment, IrNamedArg,
    IrNode, IrRange,
''',
)
replace_once(
    "crates/arkst-engine/src/value_conversion.rs",
    '''static STACKED_MAIN_AXIS_SPEC: ClosedEnumSpec<'static, IrMainAxisAlignment> = ClosedEnumSpec {
''',
    '''static DOCUMENT_ALIGNMENT_SPEC: ClosedEnumSpec<'static, IrDocumentAlignment> = ClosedEnumSpec {
    variants: &[
        ClosedEnumVariant {
            declaration_name: "START",
            value: IrDocumentAlignment::Start,
        },
        ClosedEnumVariant {
            declaration_name: "CENTER",
            value: IrDocumentAlignment::Center,
        },
        ClosedEnumVariant {
            declaration_name: "END",
            value: IrDocumentAlignment::End,
        },
        ClosedEnumVariant {
            declaration_name: "JUSTIFY",
            value: IrDocumentAlignment::Justify,
        },
    ],
};

static STACKED_MAIN_AXIS_SPEC: ClosedEnumSpec<'static, IrMainAxisAlignment> = ClosedEnumSpec {
''',
)
replace_once(
    "crates/arkst-engine/src/value_conversion.rs",
    '''/// Converts a Quarkdown `Int` boundary without truncating fractional values.
''',
    '''/// Converts the bounded document-global `.pageformat alignment` domain.
///
/// Dynamic text follows the same origin gate as the existing closed-enum
/// conversion boundary. A nested/static String does not silently acquire a
/// new domain identity.
pub(crate) fn convert_document_alignment_with_origin(
    argument: &InvocationValue,
) -> Result<IrDocumentAlignment, ConversionError> {
    match &argument.value {
        IrValue::String(value) | IrValue::Identifier(value)
            if argument.origin == ValueOrigin::Dynamic =>
        {
            DOCUMENT_ALIGNMENT_SPEC
                .value_for(value)
                .ok_or(ConversionError::InvalidText {
                    target: ConversionTarget::Enum,
                })
        }
        _ => Err(ConversionError::UnsupportedValue {
            target: ConversionTarget::Enum,
        }),
    }
}

/// Converts a Quarkdown `Int` boundary without truncating fractional values.
''',
)

# Evaluator state + transaction journal.
replace_once(
    "crates/arkst-engine/src/evaluator.rs",
    '''    IrContainerAlignment, IrContainerComponent, IrCrossAxisAlignment, IrDictionary, IrDocument,
    IrDocumentAuthor, IrDocumentTheme, IrEnumValue, IrInline, IrInlineBody, IrLandscapeComponent,
''',
    '''    IrContainerAlignment, IrContainerComponent, IrCrossAxisAlignment, IrDictionary, IrDocument,
    IrDocumentAlignment, IrDocumentAuthor, IrDocumentTheme, IrEnumValue, IrInline, IrInlineBody,
    IrLandscapeComponent,
''',
)
replace_once(
    "crates/arkst-engine/src/evaluator.rs",
    '''    caption_position: IrCaptionPositionInfo,
    auto_page_break_max_depth: Option<u32>,
    localization_tables: LocalizationTables,
''',
    '''    caption_position: IrCaptionPositionInfo,
    auto_page_break_max_depth: Option<u32>,
    page_alignment: Option<IrDocumentAlignment>,
    localization_tables: LocalizationTables,
''',
)
replace_once(
    "crates/arkst-engine/src/evaluator.rs",
    '''            caption_position: Default::default(),
            auto_page_break_max_depth: None,
            localization_tables: seeded_localization_tables(),
''',
    '''            caption_position: Default::default(),
            auto_page_break_max_depth: None,
            page_alignment: None,
            localization_tables: seeded_localization_tables(),
''',
)
replace_once(
    "crates/arkst-engine/src/evaluator.rs",
    '''            caption_position: snapshot.caption_position,
            auto_page_break_max_depth: snapshot.auto_page_break_max_depth,
            localization_tables: seeded_localization_tables(),
''',
    '''            caption_position: snapshot.caption_position,
            auto_page_break_max_depth: snapshot.auto_page_break_max_depth,
            page_alignment: snapshot.page_alignment,
            localization_tables: seeded_localization_tables(),
''',
)
replace_once(
    "crates/arkst-engine/src/evaluator.rs",
    '''            caption_position: self.caption_position,
            auto_page_break_max_depth: self.auto_page_break_max_depth,
        }
''',
    '''            caption_position: self.caption_position,
            auto_page_break_max_depth: self.auto_page_break_max_depth,
            page_alignment: self.page_alignment,
        }
''',
)
replace_once(
    "crates/arkst-engine/src/evaluator.rs",
    '''    CaptionPosition,
    AutoPageBreakMaxDepth,
    LocalizationTables,
''',
    '''    CaptionPosition,
    AutoPageBreakMaxDepth,
    PageAlignment,
    LocalizationTables,
''',
)
replace_once(
    "crates/arkst-engine/src/evaluator.rs",
    '''    CaptionPosition(IrCaptionPositionInfo),
    AutoPageBreakMaxDepth(Option<u32>),
    LocalizationTables(LocalizationTableUndo),
''',
    '''    CaptionPosition(IrCaptionPositionInfo),
    AutoPageBreakMaxDepth(Option<u32>),
    PageAlignment(Option<IrDocumentAlignment>),
    LocalizationTables(LocalizationTableUndo),
''',
)
replace_once(
    "crates/arkst-engine/src/evaluator.rs",
    '''            DocumentStateUndo::AutoPageBreakMaxDepth(previous) => {
                state.auto_page_break_max_depth = previous
            }
            DocumentStateUndo::LocalizationTables(previous) => {
''',
    '''            DocumentStateUndo::AutoPageBreakMaxDepth(previous) => {
                state.auto_page_break_max_depth = previous
            }
            DocumentStateUndo::PageAlignment(previous) => state.page_alignment = previous,
            DocumentStateUndo::LocalizationTables(previous) => {
''',
)
replace_once(
    "crates/arkst-engine/src/evaluator.rs",
    '''    fn append_document_author(&self, name: String) {
''',
    '''    fn set_page_alignment(&self, value: Option<IrDocumentAlignment>) {
        self.record_document_state_undo(DocumentStateField::PageAlignment, || {
            (
                DocumentStateUndo::PageAlignment(self.document_state.borrow().page_alignment),
                0,
            )
        });
        self.document_state.borrow_mut().page_alignment = value;
    }

    fn append_document_author(&self, name: String) {
''',
)

# Bounded dispatch: only alignment-only calls are claimed. Every wider
# `.pageformat` shape remains unresolved and is structurally preserved by the
# existing output-context fallback.
replace_once(
    "crates/arkst-engine/src/evaluator.rs",
    '''        let native_binding_plan = match self.preflight_native_binding(
''',
    '''        if name == "pageformat"
            && context.get_function(name).is_none()
            && positional_args.is_empty()
            && named_args.len() == 1
            && named_args[0].name == "alignment"
            && !matches!(&named_args[0].value, IrValue::None)
            && body.is_none()
            && raw_body.is_none()
            && lambda_parameters.is_none()
        {
            return self.evaluate_page_alignment_builtin(named_args, span, diagnostics, context);
        }

        let native_binding_plan = match self.preflight_native_binding(
''',
)
replace_once(
    "crates/arkst-engine/src/evaluator.rs",
    '''    #[allow(clippy::too_many_arguments)]
    fn evaluate_document_state_builtin(
''',
    '''    fn evaluate_page_alignment_builtin(
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
        let Some(argument) = evaluated_named.into_iter().next() else {
            return CallOutcome::Unresolved;
        };
        let candidate_span = argument.arg.span;
        let value = InvocationValue {
            value: argument.arg.value,
            origin: argument.origin,
        };
        let alignment = match value_conversion::convert_document_alignment_with_origin(&value) {
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
        };
        context.set_page_alignment(Some(alignment));
        CallOutcome::NoValue
    }

    #[allow(clippy::too_many_arguments)]
    fn evaluate_document_state_builtin(
''',
)

print("issue #338 bounded page-alignment patch applied")
