from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    p = Path(path)
    text = p.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one replacement, found {count}")
    p.write_text(text.replace(old, new, 1))


evaluator = "crates/arkst-engine/src/evaluator.rs"
replace_once(
    evaluator,
    '''                        let source_id = span.source_id;
                        let Some(source_mode) = context.source_mode(source_id) else {
                            diagnostics.push(resource_diagnostic(
                                "E9001",
                                format!(
                                    "Markdown subdocument link has no parser-mode provenance for source identity {source_id:?}"
                                ),
                                *span,
                                "Resource-backed evaluation must register the actual parser mode for every source identity before consuming source-backed links.",
                            ));
                            return Vec::new();
                        };
                        if source_mode == Mode::Quarkdown {
                            if reject_host_filesystem_reference_for_subject(
                                "Markdown subdocument link",
                                reference,
                                *span,
                                diagnostics,
                            ) {
                                return Vec::new();
                            }
                            if let Err(error) = provider.read_source(source_id, reference) {
''',
    '''                        let provenance_source_id = span.source_id;
                        let Some(source_mode) = context.source_mode(provenance_source_id) else {
                            diagnostics.push(resource_diagnostic(
                                "E9001",
                                format!(
                                    "Markdown subdocument link has no parser-mode provenance for source identity {provenance_source_id:?}"
                                ),
                                *span,
                                "Resource-backed evaluation must register the actual parser mode for every source identity before consuming source-backed links.",
                            ));
                            return Vec::new();
                        };
                        if source_mode == Mode::Quarkdown {
                            let Some(resource_base_source_id) = context.current_source else {
                                diagnostics.push(resource_diagnostic(
                                    "E9001",
                                    "Markdown subdocument link has no active resource-base source identity",
                                    *span,
                                    "Resource-backed Quarkdown link validation requires an active logical source base; diagnostic provenance may use a distinct source identity.",
                                ));
                                return Vec::new();
                            };
                            if reject_host_filesystem_reference_for_subject(
                                "Markdown subdocument link",
                                reference,
                                *span,
                                diagnostics,
                            ) {
                                return Vec::new();
                            }
                            if let Err(error) = provider.read_source(resource_base_source_id, reference) {
''',
)

engine_test = "crates/arkst-engine/tests/loadable_library_dispatch.rs"
replace_once(
    engine_test,
    '''    ast_to_ir::ast_to_ir_with_diagnostics_for_mode, DocumentMetadataDefaults, IncludedSource,
    LoadableLibraryProvider, LoadableLibrarySource, ResourceAccessError, ResourceProvider,
    ResourceText,
''',
    '''    ast_to_ir::ast_to_ir_with_diagnostics_for_mode, DocumentMetadataDefaults, EvaluationLimits,
    IncludedSource, LoadableLibraryProvider, LoadableLibrarySource, ResourceAccessError,
    ResourceProvider, ResourceText,
''',
)
p = Path(engine_test)
text = p.read_text()
marker = "fn library_static_subdocument_links_use_caller_base_and_keep_library_provenance()"
if marker in text:
    raise SystemExit(f"{engine_test}: regression tests already present")
text += r'''

fn link_destinations(document: &IrDocument) -> Vec<String> {
    document
        .nodes
        .iter()
        .filter_map(|node| match node {
            IrNode::Paragraph { content, .. } => Some(content),
            _ => None,
        })
        .flat_map(|content| content.iter())
        .filter_map(|inline| match inline {
            IrInline::Link { destination, .. } => Some(destination.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn library_static_subdocument_links_use_caller_base_and_keep_library_provenance() {
    let main = SourceId(31);
    let child = SourceId(32);
    let reader = SourceId(130);
    let broken = SourceId(131);
    let mut env = FakeEnvironment::default();
    env.paths.insert(main, "docs/main.qd".into());
    env.paths.insert(child, "docs/child.qd".into());
    env.sources.insert(
        (main, "child.qd".into()),
        IncludedSource {
            path: "docs/child.qd".into(),
            source_id: child,
            text: "target".into(),
        },
    );
    env.libraries.insert(
        "reader".into(),
        LoadableLibrarySource {
            name: "reader".into(),
            source_id: reader,
            text: "[Child](child.qd#intro)".into(),
        },
    );
    env.libraries.insert(
        "broken".into(),
        LoadableLibrarySource {
            name: "broken".into(),
            source_id: broken,
            text: "[Missing](missing.qd)".into(),
        },
    );

    let (result, diagnostics) = evaluate(".include {reader}", main, &env);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(
        env.source_requests.borrow().as_slice(),
        &[(main, "child.qd".into())]
    );
    assert!(!env
        .source_requests
        .borrow()
        .iter()
        .any(|(source_id, _)| *source_id == reader));
    assert_eq!(link_destinations(&result), vec!["child.qd#intro"]);

    env.source_requests.borrow_mut().clear();
    let (_, diagnostics) = evaluate(".include {broken}", main, &env);
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(
        env.source_requests.borrow().as_slice(),
        &[(main, "missing.qd".into())]
    );
    assert_eq!(
        diagnostics[0].primary.map(|span| span.source_id),
        Some(broken)
    );
}

fn evaluate_recursive_library(
    source: &str,
    main: SourceId,
    env: &FakeEnvironment,
) -> Vec<arkst_diagnostics::Diagnostic> {
    let limits = EvaluationLimits {
        max_materialized_elements: 1_024,
        max_evaluation_depth: 8,
    };
    let (_, diagnostics) = arkst_engine::evaluator::Evaluator::with_limits(limits)
        .evaluate_with_resources_and_libraries_for_mode(
            env,
            env,
            main,
            Mode::Quarkdown,
            &document(source, main),
            &DocumentMetadataDefaults::default(),
        );
    diagnostics
}

#[test]
fn loadable_library_recursion_is_bounded_by_evaluation_depth() {
    let main = SourceId(41);
    let a = SourceId(140);
    let b = SourceId(141);

    let mut self_recursive = FakeEnvironment::default();
    self_recursive.paths.insert(main, "main.qd".into());
    self_recursive.libraries.insert(
        "a".into(),
        LoadableLibrarySource {
            name: "a".into(),
            source_id: a,
            text: ".include {a}".into(),
        },
    );
    let diagnostics = evaluate_recursive_library(".include {a}", main, &self_recursive);
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert!(self_recursive.library_requests.borrow().len() <= 8);

    let mut mutual = FakeEnvironment::default();
    mutual.paths.insert(main, "main.qd".into());
    mutual.libraries.insert(
        "a".into(),
        LoadableLibrarySource {
            name: "a".into(),
            source_id: a,
            text: ".include {b}".into(),
        },
    );
    mutual.libraries.insert(
        "b".into(),
        LoadableLibrarySource {
            name: "b".into(),
            source_id: b,
            text: ".include {a}".into(),
        },
    );
    let diagnostics = evaluate_recursive_library(".include {a}", main, &mutual);
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert!(mutual.library_requests.borrow().len() <= 8);
}
'''
p.write_text(text)

core_test = "crates/arkst-core/tests/loadable_library_compile.rs"
p = Path(core_test)
text = p.read_text()
marker = "fn compile_library_static_subdocument_link_uses_includer_base()"
if marker in text:
    raise SystemExit(f"{core_test}: regression test already present")
text += r'''

fn link_destinations(document: &arkst_core::ir::IrDocument) -> Vec<String> {
    document
        .nodes
        .iter()
        .filter_map(|node| match node {
            IrNode::Paragraph { content, .. } => Some(content),
            _ => None,
        })
        .flat_map(|content| content.iter())
        .filter_map(|inline| match inline {
            IrInline::Link { destination, .. } => Some(destination.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn compile_library_static_subdocument_link_uses_includer_base() {
    let project = VirtualProjectBuilder::new()
        .entry("docs/main.qd")
        .unwrap()
        .add_source("docs/main.qd", ".include {reader}")
        .unwrap()
        .add_source("docs/child.qd", "target")
        .unwrap()
        .add_loadable_library("reader", "[Child](child.qd#intro)")
        .build()
        .unwrap();
    let result = compile(&project, &CompileOptions::default());
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(link_destinations(&result.ir), vec!["child.qd#intro"]);
}
'''
p.write_text(text)

audit = "docs/compatibility/quarkdown/FILESYSTEM_PROJECT_DATA_RESOURCES_AUDIT.md"
replace_once(
    audit,
    '''Quarkdown-mode literal `.qd`/`.md` destinations use their defining
`SourceSpan.source_id`, consult evaluator-retained parser-mode provenance, strip
only the anchor for lookup, preserve output spelling, and never parse/evaluate
or register the target.''',
    '''Quarkdown-mode literal `.qd`/`.md` destinations retain their defining
`SourceSpan.source_id` for parser-mode and diagnostic provenance, but resolve
through the evaluator's active `current_source` resource base; they strip only
the anchor for lookup, preserve output spelling, and never parse/evaluate or
register the target.''',
)

manifest = "docs/compatibility/quarkdown/FILESYSTEM_PROJECT_DATA_RESOURCES_AUDIT_MANIFEST.tsv"
replace_once(
    manifest,
    "validates through read_source(span.source_id, target),",
    "uses SourceSpan.source_id only for parser-mode/diagnostic provenance and validates through read_source(active current_source, target),",
)
replace_once(
    manifest,
    "The defining SourceSpan.source_id plus its canonical VirtualProject path is the resolution authority;",
    "The defining SourceSpan.source_id remains provenance identity; the evaluator's active current_source plus its canonical VirtualProject path is the resource-resolution authority;",
)
replace_once(
    manifest,
    "validate through the defining source's logical read_source base; nested includes and later invocation of defining-source functions preserve that authority;",
    "validate through the evaluator's active logical read_source base while retaining defining-span provenance; ordinary nested includes use the included target base and loadable libraries use the caller base;",
)
replace_once(
    manifest,
    "The bounded local static resolver subset now works with explicit parser-mode provenance; canonical graph target persistence, target parse/evaluation, graph registration, graph-aware destination rewriting, and complete upstream output mapping remain outside this slice. Included `*.md` sources still use Arkst's Markdown-mode path policy even though pinned upstream executes Quarkdown calls there, so that include-mode parity edge also remains open.",
    "The bounded local static resolver subset now works with explicit parser-mode provenance and an independent active resource base; resource-backed static-link validation inside source-defined callable captures is not retained by the current callable context, while canonical graph target persistence, target parse/evaluation, graph registration, graph-aware destination rewriting, and complete upstream output mapping remain outside this slice.",
)
