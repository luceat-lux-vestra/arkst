from pathlib import Path

EVALUATOR = Path("crates/arkst-engine/src/evaluator.rs")
text = EVALUATOR.read_text()

old_static = '''                        if source_mode == Mode::Quarkdown {\n                            if let Err(error) = provider.read_source(source_id, reference) {\n'''
new_static = '''                        if source_mode == Mode::Quarkdown {\n                            if reject_host_filesystem_reference_for_subject(\n                                "Markdown subdocument link",\n                                reference,\n                                *span,\n                                diagnostics,\n                            ) {\n                                return Vec::new();\n                            }\n                            if let Err(error) = provider.read_source(source_id, reference) {\n'''
if text.count(old_static) != 1:
    raise SystemExit(f"expected one static-link provider boundary, got {text.count(old_static)}")
text = text.replace(old_static, new_static, 1)

old_helper = '''fn reject_host_filesystem_reference(\n    builtin: &str,\n    reference: &str,\n    span: SourceSpan,\n    diagnostics: &mut Vec<Diagnostic>,\n) -> bool {\n    if !is_host_filesystem_reference(reference) {\n        return false;\n    }\n\n    diagnostics.push(resource_diagnostic(\n        "E8001",\n        format!("`.{builtin}` cannot access host filesystem paths"),\n        span,\n        "Use a source-relative logical project path; Arkst does not expose host filesystem access or a `global-read` capability.",\n    ));\n    true\n}\n'''
new_helper = '''fn reject_host_filesystem_reference(\n    builtin: &str,\n    reference: &str,\n    span: SourceSpan,\n    diagnostics: &mut Vec<Diagnostic>,\n) -> bool {\n    reject_host_filesystem_reference_for_subject(\n        &format!("`.{builtin}`"),\n        reference,\n        span,\n        diagnostics,\n    )\n}\n\nfn reject_host_filesystem_reference_for_subject(\n    subject: &str,\n    reference: &str,\n    span: SourceSpan,\n    diagnostics: &mut Vec<Diagnostic>,\n) -> bool {\n    if !is_host_filesystem_reference(reference) {\n        return false;\n    }\n\n    diagnostics.push(resource_diagnostic(\n        "E8001",\n        format!("{subject} cannot access host filesystem paths"),\n        span,\n        "Use a source-relative logical project path; Arkst does not expose host filesystem access or a `global-read` capability.",\n    ));\n    true\n}\n'''
if text.count(old_helper) != 1:
    raise SystemExit(f"expected one existing host-path helper, got {text.count(old_helper)}")
text = text.replace(old_helper, new_helper, 1)
EVALUATOR.write_text(text)

TEST = Path("crates/arkst-engine/tests/global_resource_boundary.rs")
test = TEST.read_text()
marker = '''#[test]\nfn logical_traversal_and_uri_references_still_reach_the_provider() {\n'''
if test.count(marker) != 1:
    raise SystemExit(f"expected one regression-test marker, got {test.count(marker)}")
insert = r'''#[test]
fn static_markdown_subdocument_host_paths_fail_before_resource_provider_access() {
    let source_id = SourceId(503);
    let resources = CountingResources::default();

    for secret_path in [
        "/etc/private/child.qd",
        "C:/Users/private/child.md",
        r"safe\..\private\child.qd",
    ] {
        resources.clear_requests();
        let source = format!("[Child]({secret_path})");
        let diagnostics = evaluate(&source, source_id, &resources);
        assert_eq!(diagnostics.len(), 1, "{source}: {diagnostics:?}");
        let diagnostic = &diagnostics[0];
        assert_eq!(diagnostic.code, "E8001", "{source}: {diagnostic:?}");
        assert_eq!(
            diagnostic.message,
            "Markdown subdocument link cannot access host filesystem paths"
        );
        assert_eq!(
            diagnostic.hints,
            vec!["Use a source-relative logical project path; Arkst does not expose host filesystem access or a `global-read` capability.".to_string()]
        );
        let rendered = format!("{diagnostic:?}");
        assert!(
            !rendered.contains(secret_path),
            "host path leaked through diagnostic: {rendered}"
        );
        assert!(
            resources.text_requests.borrow().is_empty(),
            "text provider was consulted for {source}"
        );
        assert!(
            resources.source_requests.borrow().is_empty(),
            "source provider was consulted for {source}"
        );
    }
}

'''
test = test.replace(marker, insert + marker, 1)
TEST.write_text(test)
