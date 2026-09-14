from pathlib import Path


def replace_exact(text: str, old: str, new: str, expected: int = 1) -> str:
    actual = text.count(old)
    if actual != expected:
        raise SystemExit(
            f"replacement count mismatch: expected {expected}, got {actual}: {old[:120]!r}"
        )
    return text.replace(old, new, expected)


# Backend-neutral document state: retain only an explicit override.
path = Path("crates/arkst-ir/src/lib.rs")
text = path.read_text()
text = replace_exact(
    text,
    "    #[serde(default)]\n    pub caption_position: IrCaptionPositionInfo,\n}",
    "    #[serde(default)]\n"
    "    pub caption_position: IrCaptionPositionInfo,\n"
    "    /// Explicit global heading depth selected by `.autopagebreak` or\n"
    "    /// `.noautopagebreak`. `None` preserves the document-type-specific\n"
    "    /// implicit default for the output backend to resolve.\n"
    "    #[serde(default, skip_serializing_if = \"Option::is_none\")]\n"
    "    pub auto_page_break_max_depth: Option<u32>,\n"
    "}",
)
path.write_text(text)

# Evaluator state, transactional rollback, native binding, and source shadowing.
path = Path("crates/arkst-engine/src/evaluator.rs")
text = path.read_text()
text = replace_exact(
    text,
    "    caption_position: IrCaptionPositionInfo,\n    localization_tables: LocalizationTables,",
    "    caption_position: IrCaptionPositionInfo,\n"
    "    auto_page_break_max_depth: Option<u32>,\n"
    "    localization_tables: LocalizationTables,",
)
text = replace_exact(
    text,
    "            caption_position: Default::default(),\n"
    "            localization_tables: seeded_localization_tables(),",
    "            caption_position: Default::default(),\n"
    "            auto_page_break_max_depth: None,\n"
    "            localization_tables: seeded_localization_tables(),",
)
text = replace_exact(
    text,
    "            caption_position: snapshot.caption_position,\n"
    "            localization_tables: seeded_localization_tables(),",
    "            caption_position: snapshot.caption_position,\n"
    "            auto_page_break_max_depth: snapshot.auto_page_break_max_depth,\n"
    "            localization_tables: seeded_localization_tables(),",
)
text = replace_exact(
    text,
    "            caption_position: self.caption_position,\n        }",
    "            caption_position: self.caption_position,\n"
    "            auto_page_break_max_depth: self.auto_page_break_max_depth,\n"
    "        }",
)
text = replace_exact(
    text,
    "    CaptionPosition,\n    LocalizationTables,",
    "    CaptionPosition,\n    AutoPageBreakMaxDepth,\n    LocalizationTables,",
)
text = replace_exact(
    text,
    "    CaptionPosition(IrCaptionPositionInfo),\n    LocalizationTables(LocalizationTableUndo),",
    "    CaptionPosition(IrCaptionPositionInfo),\n"
    "    AutoPageBreakMaxDepth(Option<u32>),\n"
    "    LocalizationTables(LocalizationTableUndo),",
)
text = replace_exact(
    text,
    "            DocumentStateUndo::CaptionPosition(previous) => state.caption_position = previous,\n"
    "            DocumentStateUndo::LocalizationTables(previous) => {",
    "            DocumentStateUndo::CaptionPosition(previous) => state.caption_position = previous,\n"
    "            DocumentStateUndo::AutoPageBreakMaxDepth(previous) => {\n"
    "                state.auto_page_break_max_depth = previous\n"
    "            }\n"
    "            DocumentStateUndo::LocalizationTables(previous) => {",
)

setter_marker = "    fn append_document_author(&self, name: String) {"
if text.count(setter_marker) != 1:
    raise SystemExit("append_document_author marker mismatch")
setter = """    fn set_auto_page_break_max_depth(&self, value: Option<u32>) {
        self.record_document_state_undo(DocumentStateField::AutoPageBreakMaxDepth, || {
            (
                DocumentStateUndo::AutoPageBreakMaxDepth(
                    self.document_state.borrow().auto_page_break_max_depth,
                ),
                0,
            )
        });
        self.document_state.borrow_mut().auto_page_break_max_depth = value;
    }

"""
text = text.replace(setter_marker, setter + setter_marker, 1)

native_start = text.index('const DOCUMENT_STATE_NATIVE_NAMES: &[&str] = &[')
native_end = text.index("];", native_start)
native_segment = text[native_start:native_end]
if native_segment.count('    "doctype",\n') != 1:
    raise SystemExit("document-state native doctype anchor mismatch")
native_segment = native_segment.replace(
    '    "doctype",\n',
    '    "doctype",\n    "autopagebreak",\n    "noautopagebreak",\n',
    1,
)
text = text[:native_start] + native_segment + text[native_end:]

shadowable = (
    '"captionposition" | "docauthor" | "docauthors" | "dockeywords" | "doclang" | "theme"'
)
text = replace_exact(
    text,
    shadowable,
    shadowable + ' | "autopagebreak" | "noautopagebreak"',
    expected=2,
)

binding_anchor = (
    '    let signature = match name {\n'
    '        "docname" | "docdescription" => ('
)
binding_replacement = (
    '    let signature = match name {\n'
    '        "autopagebreak" => (\n'
    '            vec![ParameterMetadata::required("maxdepth")],\n'
    '            BodyPolicy::BindFinal,\n'
    '        ),\n'
    '        "noautopagebreak" => (Vec::new(), BodyPolicy::Reject),\n'
    '        "docname" | "docdescription" => ('
)
text = replace_exact(text, binding_anchor, binding_replacement)

function_start = text.index("    fn evaluate_document_state_builtin(")
getter_marker = (
    "        if body.is_none() && positional_args.is_empty() && named_args.is_empty() {\n"
    "            return CallOutcome::Value(context.document_state_value(name));\n"
    "        }\n"
)
getter_pos = text.index(getter_marker, function_start)
auto_block = """        if name == "noautopagebreak" {
            context.set_auto_page_break_max_depth(Some(0));
            return CallOutcome::NoValue;
        }

        if name == "autopagebreak" {
            let Some(binding_plan) = binding_plan else {
                return CallOutcome::Failed;
            };
            let raw_body_candidate = match source_backed_body_candidate(
                body.as_ref()
                    .map(|body| call_body_source_span(*body, *span)),
                raw_body,
                name,
                diagnostics,
            ) {
                Ok(candidate) => candidate,
                Err(outcome) => return outcome,
            };
            let evaluated_positional = match self.evaluate_invocation_values(
                positional_args,
                span,
                diagnostics,
                context,
                first_origin,
            ) {
                Ok(values) => values,
                Err(outcome) => return outcome,
            };
            let evaluated_named =
                match self.evaluate_invocation_named(named_args, span, diagnostics, context) {
                    Ok(values) => values,
                    Err(outcome) => return outcome,
                };
            let bound = match bind_evaluated_arguments(
                binding_plan,
                evaluated_positional
                    .into_iter()
                    .zip(positional_args.iter())
                    .map(|(value, source)| (value, value_source_span(source, span)))
                    .collect(),
                evaluated_named,
                raw_body_candidate.as_ref(),
                *span,
            ) {
                Ok(bound) => bound,
                Err(error) => {
                    diagnostics.push(binding_diagnostic_with_code(error, "E3003"));
                    return CallOutcome::Failed;
                }
            };
            let parameter_span = bound
                .parameters
                .first()
                .and_then(|parameter| parameter.name_span);
            let Some(BoundSlot::Explicit {
                value: argument,
                span: argument_span,
            }) = bound.slots.into_iter().next()
            else {
                return CallOutcome::Failed;
            };
            let max_depth = match value_conversion::convert_integer_with_origin(&argument) {
                Ok(value) => value,
                Err(error) => {
                    diagnostics.push(conversion_failure_diagnostic(
                        value_conversion::ConversionFailure::new(
                            error,
                            Some(argument_span),
                            Some("maxdepth"),
                            parameter_span,
                            *span,
                        ),
                        Some("`.autopagebreak`"),
                    ));
                    return CallOutcome::Failed;
                }
            };
            if max_depth < 0 {
                diagnostics.push(document_state_conversion_error(
                    "Heading depth cannot be negative.".to_string(),
                    argument_span,
                ));
                return CallOutcome::Failed;
            }
            context.set_auto_page_break_max_depth(Some(max_depth as u32));
            return CallOutcome::NoValue;
        }

"""
text = text[:getter_pos] + auto_block + text[getter_pos:]
path.write_text(text)

# Typst lowering consumes final document-global state and emits only top-level weak breaks.
path = Path("crates/arkst-typst/src/lowering.rs")
text = path.read_text()
text = replace_exact(
    text,
    "    IrDocument, IrInline, IrLandscapeComponent, IrMainAxisAlignment, IrNode, IrSize, IrSizeUnit,",
    "    IrDocument, IrDocumentType, IrInline, IrLandscapeComponent, IrMainAxisAlignment, IrNode, IrSize, IrSizeUnit,",
)
loop_anchor = (
    "        for node in &doc.nodes {\n"
    "            let before = self.output.len();\n"
    "            self.lower_node(node);"
)
loop_replacement = """        let auto_page_break_max_depth = doc
            .metadata
            .document_state
            .auto_page_break_max_depth
            .unwrap_or(match doc.metadata.document_state.document_type {
                IrDocumentType::Plain => 0,
                IrDocumentType::Paged => 1,
                IrDocumentType::Slides => 2,
                IrDocumentType::Docs => 0,
            });

        for node in &doc.nodes {
            if auto_page_break_max_depth > 0
                && matches!(
                    node,
                    IrNode::Heading { level, .. }
                        if (*level as u32) <= auto_page_break_max_depth
                )
            {
                self.push_str("#pagebreak(weak: true)\\n");
            }
            let before = self.output.len();
            self.lower_node(node);"""
text = replace_exact(text, loop_anchor, loop_replacement)
path.write_text(text)

CORE_TEST = r'''use arkst_core::ir::{IrDocumentState, IrDocumentType};
use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};

fn compile_source(source: &str) -> arkst_core::CompileResult {
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("entry")
        .add_source("main.qd", source)
        .expect("source")
        .build()
        .expect("project");
    compile(&project, &CompileOptions::default())
}

#[test]
fn explicit_overrides_are_final_document_state() {
    let result = compile_source(
        ".doctype {slides}\n.autopagebreak maxdepth:{3}\n# Earlier\n.noautopagebreak\n# Later\n",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.ir.metadata.document_state.document_type, IrDocumentType::Slides);
    assert_eq!(result.ir.metadata.document_state.auto_page_break_max_depth, Some(0));

    let result = compile_source(
        ".autopagebreak maxdepth:{3}\n## Earlier\n.autopagebreak maxdepth:{1}\n## Later\n",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.ir.metadata.document_state.auto_page_break_max_depth, Some(1));
}

#[test]
fn negative_depth_fails_before_mutation() {
    let result = compile_source(
        ".autopagebreak maxdepth:{2}\n.autopagebreak maxdepth:{-1}\n",
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("Heading depth cannot be negative")),
        "{:?}",
        result.diagnostics
    );
    assert_eq!(
        result.ir.metadata.document_state.auto_page_break_max_depth,
        Some(2),
        "failed mutation must preserve the last committed state"
    );
}

#[test]
fn source_defined_functions_shadow_both_natives() {
    let result = compile_source(
        ".function {autopagebreak}\n    maxdepth:\n    SHADOW-AUTO\n\n.doctype {slides}\n.autopagebreak maxdepth:{0}\n",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.ir.metadata.document_state.auto_page_break_max_depth, None);
    assert!(format!("{:?}", result.ir).contains("SHADOW-AUTO"));

    let result = compile_source(
        ".function {noautopagebreak}\n    SHADOW-NO-AUTO\n\n.doctype {slides}\n.noautopagebreak\n",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.ir.metadata.document_state.auto_page_break_max_depth, None);
    assert!(format!("{:?}", result.ir).contains("SHADOW-NO-AUTO"));
}

#[test]
fn document_state_wire_is_backward_compatible_and_explicit_when_overridden() {
    let state = IrDocumentState::default();
    let legacy_shape = serde_json::to_value(&state).expect("serialize default state");
    assert!(legacy_shape.get("auto_page_break_max_depth").is_none());
    let restored: IrDocumentState =
        serde_json::from_value(legacy_shape).expect("deserialize legacy-shaped state");
    assert_eq!(restored.auto_page_break_max_depth, None);

    let explicit = IrDocumentState {
        auto_page_break_max_depth: Some(2),
        ..IrDocumentState::default()
    };
    let value = serde_json::to_value(&explicit).expect("serialize explicit state");
    assert_eq!(
        value.get("auto_page_break_max_depth"),
        Some(&serde_json::json!(2))
    );
    let restored: IrDocumentState =
        serde_json::from_value(value).expect("round trip explicit state");
    assert_eq!(restored.auto_page_break_max_depth, Some(2));
}
'''
Path("crates/arkst-core/tests/quarkdown_v260_autopagebreak.rs").write_text(CORE_TEST)

TYPST_TEST = r'''use arkst_ir::{
    IrDocument, IrDocumentState, IrDocumentType, IrInline, IrMetadata, IrNode,
};
use arkst_source::{SourceId, SourceSpan};
use arkst_typst::lowering::lower_to_typst_code;

fn span() -> SourceSpan {
    SourceSpan::new(SourceId(0), 0, 0)
}

fn heading(level: usize, label: &str) -> IrNode {
    IrNode::Heading {
        level,
        content: vec![IrInline::Text {
            content: label.to_string(),
            span: span(),
        }],
        span: span(),
    }
}

fn document(
    document_type: IrDocumentType,
    override_depth: Option<u32>,
    nodes: Vec<IrNode>,
) -> IrDocument {
    IrDocument {
        nodes,
        metadata: IrMetadata {
            document_state: IrDocumentState {
                document_type,
                auto_page_break_max_depth: override_depth,
                ..IrDocumentState::default()
            },
            ..IrMetadata::default()
        },
    }
}

fn break_count(code: &str) -> usize {
    code.matches("#pagebreak(weak: true)").count()
}

#[test]
fn implicit_defaults_match_the_pinned_v260_oracle() {
    let nodes = vec![heading(1, "H1"), heading(2, "H2"), heading(3, "H3")];
    for (document_type, expected) in [
        (IrDocumentType::Plain, 0),
        (IrDocumentType::Paged, 1),
        (IrDocumentType::Slides, 2),
        (IrDocumentType::Docs, 0),
    ] {
        let code = lower_to_typst_code(&document(document_type, None, nodes.clone()));
        assert_eq!(break_count(&code), expected, "{document_type:?}: {code}");
    }
}

#[test]
fn explicit_thresholds_override_document_type_and_zero_disables() {
    let nodes = vec![heading(1, "H1"), heading(2, "H2"), heading(3, "H3")];
    for (depth, expected) in [(0, 0), (1, 1), (2, 2), (3, 3)] {
        let code = lower_to_typst_code(&document(
            IrDocumentType::Slides,
            Some(depth),
            nodes.clone(),
        ));
        assert_eq!(break_count(&code), expected, "depth {depth}: {code}");
    }
}

#[test]
fn only_top_level_headings_create_typst_page_boundaries() {
    let nested = IrNode::Blockquote {
        content: vec![heading(2, "Nested H2")],
        span: span(),
    };
    let code = lower_to_typst_code(&document(
        IrDocumentType::Slides,
        None,
        vec![nested, heading(2, "Top H2")],
    ));
    assert_eq!(break_count(&code), 1, "{code}");
    assert!(code.contains("#quote(block: true)["), "{code}");
}

#[test]
fn emitted_breaks_are_weak_for_heading_at_start_consecutive_and_manual_adjacency_safety() {
    let code = lower_to_typst_code(&document(
        IrDocumentType::Slides,
        None,
        vec![heading(1, "First"), heading(2, "Second")],
    ));
    assert_eq!(break_count(&code), 2, "{code}");
    assert!(!code.contains("#pagebreak()"), "{code}");
}
'''
Path("crates/arkst-typst/tests/auto_page_break.rs").write_text(TYPST_TEST)

INPROCESS_TEST = r'''use arkst_core::{compile, CompileOptions, VirtualProjectBuilder};
use arkst_typst::{TypstBackend, TypstInput};
use arkst_typst_inprocess::InProcessBackend;

fn project(source: &str) -> arkst_project::VirtualProject {
    VirtualProjectBuilder::new()
        .entry("main.qd")
        .expect("entry")
        .add_source("main.qd", source)
        .expect("source")
        .build()
        .expect("project")
}

fn compile_pdf(source: &str) -> Vec<u8> {
    let project = project(source);
    let result = compile(&project, &CompileOptions::default());
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let typst = arkst_typst::lowering::lower_to_typst_code(&result.ir);
    InProcessBackend::new(&project)
        .compile(&TypstInput {
            source: typst,
            entry_path: "main.qd".to_string(),
        })
        .expect("pinned Typst in-process compilation")
        .pdf
        .expect("PDF output")
}

fn page_count(pdf: &[u8]) -> usize {
    let pages = pdf
        .windows(b"/Type/Page".len())
        .filter(|window| *window == b"/Type/Page")
        .count();
    let page_tree_nodes = pdf
        .windows(b"/Type/Pages".len())
        .filter(|window| *window == b"/Type/Pages")
        .count();
    pages.saturating_sub(page_tree_nodes)
}

#[test]
fn slides_default_and_disable_have_real_typst_page_count_evidence() {
    let source = ".doctype {slides}\n\nIntro.\n\n# H1\n\nA.\n\n## H2\n\nB.\n\n### H3\n\nC.\n";
    assert_eq!(page_count(&compile_pdf(source)), 3);

    let disabled = format!(".noautopagebreak\n{source}");
    assert_eq!(page_count(&compile_pdf(&disabled)), 1);
}

#[test]
fn weak_break_at_document_start_does_not_create_an_empty_leading_page() {
    let source = ".doctype {slides}\n\n# First\n\n## Second\n";
    assert_eq!(page_count(&compile_pdf(source)), 2);
}

#[test]
fn nested_heading_does_not_attempt_a_container_pagebreak() {
    let source = ".doctype {slides}\n\nIntro.\n\n> Quote lead.\n>\n> ## Nested H2\n>\n> Quote tail.\n";
    assert_eq!(page_count(&compile_pdf(source)), 1);
}
'''
Path("crates/arkst-typst-inprocess/tests/auto_page_break.rs").write_text(INPROCESS_TEST)

DOC = r'''# Quarkdown v2.6 auto page-break compatibility

Issue: #332. Parent migration checklist: #311. Existing configuration owner: #175.

## Clean-room oracle

The compatibility contract was observed with the official Quarkdown v2.6.0 Linux x64 distribution (`quarkdown version 2.6.0`), SHA-256 `5b015e47c820d06ff6774eb700e60d77bc575819d4a0140cc7f1a115e7ce6dc4`, using independently authored fixtures. The disposable probe PR is #333 and was closed unmerged after evidence capture.

Observed final-document semantics:

- implicit maximum heading depth is 0 for `plain`, 1 for `paged`, 2 for `slides`, and 0 for `docs`;
- explicit `.autopagebreak maxdepth:{0,1,2,3}` selects none / H1 / H1+H2 / H1+H2+H3 respectively;
- `.noautopagebreak` is the zero-depth shorthand;
- the final global setting applies to headings that occur both before and after the setter in source order;
- a final `.doctype {slides}` likewise applies the slides implicit default to preceding headings;
- negative depth fails before state mutation;
- source-defined `autopagebreak` and `noautopagebreak` functions shadow the native names;
- nested H2 elements receive the reference HTML marker but do not create a rendered Reveal `.slides > section`; top-level H2 does;
- heading-at-start and adjacent automatic/manual break observations do not create visible empty slides, motivating weak backend breaks.

## Arkst ownership

`IrDocumentState.auto_page_break_max_depth` stores only an explicit backend-neutral override. `None` means that the output path derives the implicit threshold from the final `IrDocumentType`; it does not mean zero.

The evaluator owns binding, integer conversion, non-negative validation, transactional publication, and source-function shadowing. It does not stamp heading-local history into IR because the oracle proves that the reference behavior uses final document configuration.

Typst lowering owns page-boundary emission. It emits `#pagebreak(weak: true)` only before qualifying top-level headings. Nested headings are deliberately not given Typst page breaks: the observed rendered slides do not split there, and Typst page breaks are not valid inside containers.

## Verification

The bounded regression suite covers final-state ordering, explicit thresholds, `.noautopagebreak`, negative rollback, source shadowing, backward-compatible serde omission, document-type defaults, top-level-only lowering, weak break emission, and real in-process Typst PDF page counts. Existing CI supplies the repository's pinned Typst 0.15.1 dependency and platform/WASM checks.

Manual `.pagebreak` / `<<<` syntax remains owned by the existing content/page primitive work; this slice does not broaden that primitive. The automatic break output is weak so an adjacent backend/manual page boundary can collapse without creating an empty page.
'''
Path("docs/compatibility/quarkdown/V260_AUTOPAGEBREAK.md").write_text(DOC)
