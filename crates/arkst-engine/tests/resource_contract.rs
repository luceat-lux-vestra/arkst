use arkst_diagnostics::{Diagnostic, Severity};
use arkst_engine::{
    ast_to_ir::ast_to_ir_with_diagnostics_for_mode, DocumentMetadataDefaults, IncludedSource,
    ResourceAccessError, ResourceProvider, ResourceRoot, ResourceText,
};
use arkst_ir::{IrDocument, IrInline, IrNode};
use arkst_markdown::Mode;
use arkst_source::{SourceId, SourceSpan};
use std::cell::RefCell;
use std::collections::HashMap;

#[derive(Debug, Default)]
struct FakeResources {
    paths: HashMap<SourceId, String>,
    sources: HashMap<(SourceId, String), IncludedSource>,
    text: HashMap<(SourceId, String), Result<ResourceText, ResourceAccessError>>,
    roots: HashMap<(SourceId, Option<SourceId>), Result<String, ResourceAccessError>>,
    source_requests: RefCell<Vec<(SourceId, String)>>,
    text_requests: RefCell<Vec<(SourceId, String)>>,
    root_requests: RefCell<Vec<(SourceId, ResourceRoot)>>,
}

impl ResourceProvider for FakeResources {
    fn source_path(&self, source_id: SourceId) -> Option<String> {
        self.paths.get(&source_id).cloned()
    }

    fn relative_path_to_root(
        &self,
        source_id: SourceId,
        root: ResourceRoot,
    ) -> Result<String, ResourceAccessError> {
        self.root_requests.borrow_mut().push((source_id, root));
        let root_id = match root {
            ResourceRoot::Project => None,
            ResourceRoot::Source(source_id) => Some(source_id),
        };
        self.roots
            .get(&(source_id, root_id))
            .cloned()
            .unwrap_or(Err(ResourceAccessError::UnknownSource { source_id }))
    }

    fn read_text(
        &self,
        source_id: SourceId,
        reference: &str,
    ) -> Result<ResourceText, ResourceAccessError> {
        self.text_requests
            .borrow_mut()
            .push((source_id, reference.to_string()));
        self.text
            .get(&(source_id, reference.to_string()))
            .cloned()
            .unwrap_or_else(|| {
                Err(ResourceAccessError::NotFound {
                    path: reference.to_string(),
                })
            })
    }

    fn read_source(
        &self,
        source_id: SourceId,
        reference: &str,
    ) -> Result<IncludedSource, ResourceAccessError> {
        self.source_requests
            .borrow_mut()
            .push((source_id, reference.to_string()));
        self.sources
            .get(&(source_id, reference.to_string()))
            .cloned()
            .ok_or_else(|| ResourceAccessError::NotFound {
                path: reference.to_string(),
            })
    }
}

fn document(source: &str, source_id: SourceId) -> IrDocument {
    let parsed = arkst_markdown::parse_with_mode(source, Mode::Quarkdown);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let (document, diagnostics) = ast_to_ir_with_diagnostics_for_mode(
        &parsed.document,
        source_id,
        &DocumentMetadataDefaults::default(),
        Mode::Quarkdown,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    document
}

fn evaluate(
    source: &str,
    source_id: SourceId,
    resources: &FakeResources,
) -> (IrDocument, Vec<arkst_diagnostics::Diagnostic>) {
    arkst_engine::evaluator::Evaluator::new().evaluate_with_resources(
        resources,
        source_id,
        &document(source, source_id),
        &DocumentMetadataDefaults::default(),
    )
}

fn assert_exact_resource_diagnostic(
    diagnostic: &Diagnostic,
    code: &str,
    message: &str,
    source_id: SourceId,
    end: usize,
    hint: &str,
) {
    assert_eq!(diagnostic.code, code);
    assert!(matches!(diagnostic.severity, Severity::Error));
    assert_eq!(diagnostic.message, message);
    assert_eq!(diagnostic.primary, Some(SourceSpan::new(source_id, 0, end)));
    assert!(diagnostic.secondary.is_empty());
    assert_eq!(diagnostic.hints, vec![hint.to_string()]);
}

#[test]
fn nested_include_uses_included_source_identity_for_following_resources() {
    let main = SourceId(1);
    let first = SourceId(2);
    let nested = SourceId(3);
    let mut resources = FakeResources::default();
    resources.paths.insert(main, "main.qd".into());
    resources.paths.insert(first, "chapter/one.qd".into());
    resources.paths.insert(nested, "chapter/two.qd".into());
    resources.sources.insert(
        (main, "chapter/one.qd".into()),
        IncludedSource {
            path: "chapter/one.qd".into(),
            source_id: first,
            text: ".include {two.qd}".into(),
        },
    );
    resources.sources.insert(
        (first, "two.qd".into()),
        IncludedSource {
            path: "chapter/two.qd".into(),
            source_id: nested,
            text: ".read {data.txt}".into(),
        },
    );
    resources.text.insert(
        (nested, "data.txt".into()),
        Ok(ResourceText {
            path: "chapter/data.txt".into(),
            text: "nested text".into(),
        }),
    );

    let (result, diagnostics) = evaluate(".include {chapter/one.qd}", main, &resources);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(
        resources.source_requests.borrow().as_slice(),
        &[(main, "chapter/one.qd".into()), (first, "two.qd".into()),]
    );
    assert_eq!(
        resources.text_requests.borrow().as_slice(),
        &[(nested, "data.txt".into())]
    );
    assert_eq!(result.nodes.len(), 1);
    let IrNode::Paragraph { content, .. } = &result.nodes[0] else {
        panic!("expected included read paragraph")
    };
    assert!(
        matches!(&content[0], IrInline::Text { content, span } if content == "nested text" && span.source_id == nested)
    );
}

#[test]
fn include_cycles_are_reported_without_suppressing_completed_includes() {
    let main = SourceId(1);
    let child = SourceId(2);
    let mut resources = FakeResources::default();
    resources.paths.insert(main, "main.qd".into());
    resources.paths.insert(child, "child.qd".into());
    resources.sources.insert(
        (main, "child.qd".into()),
        IncludedSource {
            path: "child.qd".into(),
            source_id: child,
            text: ".include {main.qd}".into(),
        },
    );
    resources.sources.insert(
        (child, "main.qd".into()),
        IncludedSource {
            path: "main.qd".into(),
            source_id: main,
            text: "cycle".into(),
        },
    );

    let (_, diagnostics) = evaluate(".include {child.qd}", main, &resources);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E3001");
    assert!(diagnostics[0]
        .message
        .contains("main.qd -> child.qd -> main.qd"));
}

#[test]
fn completed_repeated_includes_are_not_globally_suppressed() {
    let main = SourceId(11);
    let child = SourceId(12);
    let mut resources = FakeResources::default();
    resources.paths.insert(main, "main.qd".into());
    resources.paths.insert(child, "child.qd".into());
    resources.sources.insert(
        (main, "child.qd".into()),
        IncludedSource {
            path: "child.qd".into(),
            source_id: child,
            text: "repeated".into(),
        },
    );

    let (result, diagnostics) =
        evaluate(".include {child.qd}\n.include {child.qd}", main, &resources);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(result.nodes.len(), 2);
}

#[test]
fn resource_failures_preserve_engine_diagnostic_categories_and_paths() {
    let source_id = SourceId(7);
    let mut resources = FakeResources::default();
    resources.paths.insert(source_id, "main.qd".into());
    resources.text.insert(
        (source_id, "unsupported".into()),
        Err(ResourceAccessError::UnsupportedReference {
            reference: "https://example.test/data".into(),
        }),
    );
    resources.text.insert(
        (source_id, "bad.bin".into()),
        Err(ResourceAccessError::InvalidUtf8 {
            path: "bad.bin".into(),
            message: "invalid utf-8 sequence".into(),
        }),
    );

    let (_, unsupported) = evaluate(".read {unsupported}", source_id, &resources);
    assert_exact_resource_diagnostic(
        &unsupported[0],
        "E8001",
        "`.read` does not support non-local resource reference `https://example.test/data`",
        source_id,
        19,
        "Only source-relative paths inside the supplied VirtualProject are available; network fetching is disabled.",
    );

    let (_, invalid_utf8) = evaluate(".read {bad.bin}", source_id, &resources);
    assert_exact_resource_diagnostic(
        &invalid_utf8[0],
        "E3001",
        "`.read` resource `bad.bin` is not valid UTF-8: invalid utf-8 sequence",
        source_id,
        15,
        "Text resource builtins require valid UTF-8 and do not perform lossy decoding.",
    );
}

#[test]
fn missing_resource_is_a_source_backed_failure() {
    let source_id = SourceId(9);
    let resources = FakeResources {
        paths: [(source_id, "main.qd".into())].into_iter().collect(),
        ..FakeResources::default()
    };
    let (_, diagnostics) = evaluate(".read {missing.txt}", source_id, &resources);
    assert_exact_resource_diagnostic(
        &diagnostics[0],
        "E3001",
        "`.read` resource not found: `missing.txt`",
        source_id,
        19,
        "Add the logical resource to the VirtualProject supplied by the host.",
    );
}

#[test]
fn missing_include_source_keeps_its_legacy_diagnostic_hint() {
    let source_id = SourceId(10);
    let resources = FakeResources {
        paths: [(source_id, "main.qd".into())].into_iter().collect(),
        ..FakeResources::default()
    };
    let (_, diagnostics) = evaluate(".include {missing.qd}", source_id, &resources);

    assert_exact_resource_diagnostic(
        &diagnostics[0],
        "E3001",
        "`.include` resource not found: `missing.qd`",
        source_id,
        21,
        "Add the target source to the VirtualProject supplied by the host.",
    );
}

#[test]
fn resource_builtin_without_provider_keeps_its_legacy_context_diagnostic() {
    let source_id = SourceId(11);
    let document = document(".read {missing.txt}", source_id);
    let (_, diagnostics) = arkst_engine::evaluator::Evaluator::new().evaluate(&document);

    assert_exact_resource_diagnostic(
        &diagnostics[0],
        "E8001",
        "Resource builtin requires a host-supplied VirtualProject",
        source_id,
        19,
        "Compile through the project API so logical resources are supplied explicitly.",
    );
}

#[test]
fn pathtoroot_tracks_project_and_nested_subdocument_roots_without_host_paths() {
    let main = SourceId(21);
    let subdocument = SourceId(22);
    let utility = SourceId(23);
    let mut resources = FakeResources::default();
    resources.paths.insert(main, "main.qd".into());
    resources
        .paths
        .insert(subdocument, "subdocuments/subdocument.qd".into());
    resources.paths.insert(utility, "utils/example.qd".into());
    resources.sources.insert(
        (main, "subdocuments/subdocument.qd".into()),
        IncludedSource {
            path: "subdocuments/subdocument.qd".into(),
            source_id: subdocument,
            text: ".include {../utils/example.qd}".into(),
        },
    );
    resources.sources.insert(
        (subdocument, "../utils/example.qd".into()),
        IncludedSource {
            path: "utils/example.qd".into(),
            source_id: utility,
            text: ".pathtoroot\n.pathtoroot granularity:{subdocument}".into(),
        },
    );
    resources.roots.insert((utility, None), Ok("..".into()));
    resources
        .roots
        .insert((utility, Some(subdocument)), Ok("../subdocuments".into()));

    let (result, diagnostics) = evaluate(
        ".include {subdocuments/subdocument.qd} sandbox:{subdocument}",
        main,
        &resources,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(
        resources.root_requests.borrow().as_slice(),
        &[
            (utility, ResourceRoot::Project),
            (utility, ResourceRoot::Source(subdocument)),
        ]
    );
    assert_eq!(result.nodes.len(), 2);
}

#[test]
fn pathtoroot_subdocument_granularity_falls_back_to_project_root_at_top_level() {
    let main = SourceId(31);
    let mut resources = FakeResources::default();
    resources.paths.insert(main, "main.qd".into());
    resources.roots.insert((main, None), Ok(".".into()));

    let (result, diagnostics) = evaluate(".pathtoroot granularity:{subdocument}", main, &resources);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(
        resources.root_requests.borrow().as_slice(),
        &[(main, ResourceRoot::Project)]
    );
    assert_eq!(result.nodes.len(), 1);
}

#[test]
fn pathtoroot_rejects_unknown_granularity_before_requesting_a_root() {
    let main = SourceId(41);
    let resources = FakeResources {
        paths: [(main, "main.qd".into())].into_iter().collect(),
        ..FakeResources::default()
    };

    let (_, diagnostics) = evaluate(".pathtoroot granularity:{workspace}", main, &resources);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E3003");
    assert!(diagnostics[0].message.contains("workspace"));
    assert!(resources.root_requests.borrow().is_empty());
}

fn document_paragraph_text(document: &IrDocument) -> String {
    document
        .nodes
        .iter()
        .filter_map(|node| match node {
            IrNode::Paragraph { content, .. } => Some(
                content
                    .iter()
                    .filter_map(|inline| match inline {
                        IrInline::Text { content, .. } => Some(content.as_str()),
                        _ => None,
                    })
                    .collect::<String>(),
            ),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn includeall_body_list_preserves_order_and_shared_state() {
    let main = SourceId(51);
    let first = SourceId(52);
    let second = SourceId(53);
    let mut resources = FakeResources::default();
    resources.paths.insert(main, "main.qd".into());
    resources.paths.insert(first, "first.qd".into());
    resources.paths.insert(second, "second.qd".into());
    resources.sources.insert(
        (main, "first.qd".into()),
        IncludedSource {
            path: "first.qd".into(),
            source_id: first,
            text: ".var {shared} {first}\nfirst".into(),
        },
    );
    resources.sources.insert(
        (main, "second.qd".into()),
        IncludedSource {
            path: "second.qd".into(),
            source_id: second,
            text: ".shared\n.var {shared} {second}\nsecond".into(),
        },
    );

    let (result, diagnostics) = evaluate(
        ".includeall\n    - first.qd\n    - second.qd\n.shared",
        main,
        &resources,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(
        resources.source_requests.borrow().as_slice(),
        &[(main, "first.qd".into()), (main, "second.qd".into())]
    );
    assert_eq!(
        document_paragraph_text(&result),
        "first\nfirst\nsecond\nsecond"
    );
}

#[test]
fn includeall_is_fail_fast_after_processing_prior_shared_include() {
    let main = SourceId(61);
    let first = SourceId(62);
    let third = SourceId(63);
    let mut resources = FakeResources::default();
    resources.paths.insert(main, "main.qd".into());
    resources.paths.insert(first, "first.qd".into());
    resources.paths.insert(third, "third.qd".into());
    resources.sources.insert(
        (main, "first.qd".into()),
        IncludedSource {
            path: "first.qd".into(),
            source_id: first,
            text: ".var {shared} {first}".into(),
        },
    );
    resources.sources.insert(
        (main, "third.qd".into()),
        IncludedSource {
            path: "third.qd".into(),
            source_id: third,
            text: ".var {shared} {third}".into(),
        },
    );

    let (result, diagnostics) = evaluate(
        ".includeall\n    - first.qd\n    - missing.qd\n    - third.qd\n.shared",
        main,
        &resources,
    );
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E3001");
    assert_eq!(
        resources.source_requests.borrow().as_slice(),
        &[(main, "first.qd".into()), (main, "missing.qd".into())]
    );
    assert_eq!(document_paragraph_text(&result), "");
}

#[test]
fn includeall_rejects_non_iterable_before_requesting_sources() {
    let main = SourceId(71);
    let resources = FakeResources {
        paths: [(main, "main.qd".into())].into_iter().collect(),
        ..FakeResources::default()
    };
    let (_, diagnostics) = evaluate(".includeall {first.qd}", main, &resources);
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert!(resources.source_requests.borrow().is_empty());
}

#[test]
fn includeall_binding_collision_is_rejected_before_body_resource_access() {
    let main = SourceId(81);
    let side = SourceId(82);
    let mut resources = FakeResources::default();
    resources.paths.insert(main, "main.qd".into());
    resources.paths.insert(side, "side.qd".into());
    resources.sources.insert(
        (main, "side.qd".into()),
        IncludedSource {
            path: "side.qd".into(),
            source_id: side,
            text: "side".into(),
        },
    );

    let (_, diagnostics) = evaluate(
        ".includeall {ignored.qd}\n    - .include {side.qd}",
        main,
        &resources,
    );
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E3001");
    assert!(resources.source_requests.borrow().is_empty());
}

#[test]
fn subdocument_resolves_extensionless_target_and_preserves_visible_link_arguments() {
    let main = SourceId(71);
    let target = SourceId(72);
    let mut resources = FakeResources::default();
    resources.paths.insert(main, "docs/main.qd".into());
    resources.paths.insert(target, "docs/chapters/intro".into());
    resources.sources.insert(
        (main, "chapters/intro".into()),
        IncludedSource {
            path: "docs/chapters/intro".into(),
            source_id: target,
            text: "target body".into(),
        },
    );

    let (result, diagnostics) = evaluate(
        ".subdocument path:{chapters/intro} label:{Introduction} anchor:{start}",
        main,
        &resources,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(
        resources.source_requests.borrow().as_slice(),
        &[(main, "chapters/intro".into())]
    );
    let [IrNode::Paragraph { content, .. }] = result.nodes.as_slice() else {
        panic!("expected one visible link paragraph: {:?}", result.nodes);
    };
    let [IrInline::Link {
        content: label,
        destination,
        title,
        ..
    }] = content.as_slice()
    else {
        panic!("expected one subdocument link: {content:?}");
    };
    assert_eq!(destination, "chapters/intro#start");
    assert!(title.is_none());
    assert!(matches!(
        label.as_slice(),
        [IrInline::Text { content, .. }] if content == "Introduction"
    ));
}

#[test]
fn unlabeled_subdocument_resolves_target_and_emits_empty_label_link() {
    let main = SourceId(73);
    let target = SourceId(74);
    let mut resources = FakeResources::default();
    resources.paths.insert(main, "main.qd".into());
    resources.paths.insert(target, "child.qd".into());
    resources.sources.insert(
        (main, "child.qd".into()),
        IncludedSource {
            path: "child.qd".into(),
            source_id: target,
            text: "child".into(),
        },
    );

    let (result, diagnostics) = evaluate(".subdocument {child.qd}", main, &resources);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(
        resources.source_requests.borrow().as_slice(),
        &[(main, "child.qd".into())]
    );
    let [IrNode::Paragraph { content, .. }] = result.nodes.as_slice() else {
        panic!(
            "expected one empty-label link paragraph: {:?}",
            result.nodes
        );
    };
    let [IrInline::Link {
        content: label,
        destination,
        title,
        ..
    }] = content.as_slice()
    else {
        panic!("expected one subdocument link: {content:?}");
    };
    assert!(label.is_empty(), "{label:?}");
    assert_eq!(destination, "child.qd");
    assert!(title.is_none());
}

#[test]
fn nested_subdocument_resolution_uses_current_included_source_as_base() {
    let main = SourceId(75);
    let included = SourceId(76);
    let target = SourceId(77);
    let mut resources = FakeResources::default();
    resources.paths.insert(main, "docs/main.qd".into());
    resources.paths.insert(included, "docs/parts/one.qd".into());
    resources.paths.insert(target, "docs/parts/child".into());
    resources.sources.insert(
        (main, "parts/one.qd".into()),
        IncludedSource {
            path: "docs/parts/one.qd".into(),
            source_id: included,
            text: ".subdocument {child} {Child}".into(),
        },
    );
    resources.sources.insert(
        (included, "child".into()),
        IncludedSource {
            path: "docs/parts/child".into(),
            source_id: target,
            text: "target".into(),
        },
    );

    let (_, diagnostics) = evaluate(".include {parts/one.qd}", main, &resources);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(
        resources.source_requests.borrow().as_slice(),
        &[(main, "parts/one.qd".into()), (included, "child".into())]
    );
}

#[test]
fn subdocument_resource_failures_are_source_backed_and_fail_closed() {
    let main = SourceId(78);
    let mut resources = FakeResources::default();
    resources.paths.insert(main, "main.qd".into());
    resources.sources.insert(
        (main, "remote".into()),
        IncludedSource {
            path: "unused".into(),
            source_id: SourceId(79),
            text: String::new(),
        },
    );

    let (_, missing) = evaluate(".subdocument {missing}", main, &resources);
    assert_eq!(missing.len(), 1, "{missing:?}");
    assert_eq!(missing[0].code, "E3001");
    assert!(missing[0].message.contains("`.subdocument`"));
    assert!(missing[0].message.contains("missing"));
    assert_eq!(missing[0].primary.map(|span| span.source_id), Some(main));
}

#[test]
fn subdocument_composes_inline_with_surrounding_text() {
    let main = SourceId(81);
    let target = SourceId(82);
    let mut resources = FakeResources::default();
    resources.paths.insert(main, "main.qd".into());
    resources.paths.insert(target, "child.qd".into());
    resources.sources.insert(
        (main, "child.qd".into()),
        IncludedSource {
            path: "child.qd".into(),
            source_id: target,
            text: "child".into(),
        },
    );

    let (result, diagnostics) = evaluate(
        "The link is: .subdocument {child.qd} {Child} after",
        main,
        &resources,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(
        resources.source_requests.borrow().as_slice(),
        &[(main, "child.qd".into())]
    );
    let [IrNode::Paragraph { content, .. }] = result.nodes.as_slice() else {
        panic!("expected one composed paragraph: {:?}", result.nodes);
    };
    assert!(
        matches!(
            content.as_slice(),
            [
                IrInline::Text { content: before, .. },
                IrInline::Link {
                    content: label,
                    destination,
                    ..
                },
                IrInline::Text { content: after, .. }
            ] if before == "The link is: "
                && matches!(label.as_slice(), [IrInline::Text { content, .. }] if content == "Child")
                && destination == "child.qd"
                && after == " after"
        ),
        "unexpected inline composition: {content:?}"
    );
}

#[test]
fn source_defined_subdocument_shadows_native_without_resource_access() {
    let main = SourceId(83);
    let resources = FakeResources {
        paths: [(main, "main.qd".into())].into_iter().collect(),
        ..FakeResources::default()
    };

    let (result, diagnostics) = evaluate(
        ".function {subdocument}\n    path:\n    .path\n\n.subdocument {shadow-value}",
        main,
        &resources,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(
        resources.source_requests.borrow().is_empty(),
        "source-defined callable must not invoke the resource provider"
    );
    let [IrNode::Paragraph { content, .. }] = result.nodes.as_slice() else {
        panic!(
            "expected source-defined function output: {:?}",
            result.nodes
        );
    };
    assert!(matches!(
        content.as_slice(),
        [IrInline::Text { content, .. }] if content == "shadow-value"
    ));
}

#[test]
fn subdocument_rejects_body_before_requesting_provider() {
    let main = SourceId(80);
    let resources = FakeResources {
        paths: [(main, "main.qd".into())].into_iter().collect(),
        ..FakeResources::default()
    };

    let (_, diagnostics) = evaluate(
        ".subdocument {child.qd}\n    body is not accepted",
        main,
        &resources,
    );
    assert!(!diagnostics.is_empty());
    assert!(resources.source_requests.borrow().is_empty());
}
