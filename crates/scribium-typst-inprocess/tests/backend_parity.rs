//! Cross-backend parity evidence for the optional native in-process adapter.
//!
//! Every fixture below starts as a VirtualProject and goes through the real
//! Scribium compile/lowering path. The subprocess observation receives a
//! temporary read-context populated from that same project at the native host
//! boundary; the in-process observation receives the project directly. The
//! temporary copy is therefore adapter plumbing, not a second fixture source.

use scribium_core::{compile, CompileOptions, VirtualProject, VirtualProjectBuilder};
use scribium_source::{SourceMapEntry, SourceSpan};
use scribium_typst::lowering::lower_to_typst;
use scribium_typst::{TypstBackend, TypstInput, TypstOutput};
use scribium_typst_inprocess::{InProcessBackend, InProcessError};
use scribium_typst_subprocess::{SubprocessBackend, TypstSourceContext};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::{tempdir, TempDir};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutcomeClass {
    Success,
    CompilationFailure,
    ResourceFailure,
    TraversalDenied,
    PackageDenied,
    InvalidInput,
}

impl fmt::Display for OutcomeClass {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Success => "PASS",
            Self::CompilationFailure => "compile-failure",
            Self::ResourceFailure => "resource-failure",
            Self::TraversalDenied => "traversal-denied",
            Self::PackageDenied => "package-denied",
            Self::InvalidInput => "invalid-input",
        };
        formatter.write_str(name)
    }
}

#[derive(Debug, Clone, Copy)]
struct ParityExpectation {
    outcome: OutcomeClass,
    minimum_pages: Option<usize>,
    source_markers: &'static [&'static str],
    require_logical_diagnostic_path: bool,
    require_in_process_span: bool,
    require_in_process_unmapped: bool,
    expected_in_process_span: Option<SourceSpan>,
}

struct ParityFixture {
    name: &'static str,
    project: VirtualProject,
    input: TypstInput,
    source_map: Vec<SourceMapEntry>,
    expectation: ParityExpectation,
}

#[derive(Debug, PartialEq, Eq)]
struct PdfObservation {
    non_empty: bool,
    valid_header: bool,
    valid_eof: bool,
    page_count: Option<usize>,
}

#[derive(Debug)]
struct DiagnosticObservation {
    logical_path: Option<String>,
    original_span: Option<SourceSpan>,
    host_path_leakage: bool,
    remote_url_leakage: bool,
}

#[derive(Debug)]
struct BackendObservation {
    outcome: OutcomeClass,
    pdf: Option<PdfObservation>,
    diagnostics: Vec<DiagnosticObservation>,
}

#[test]
fn backend_parity_corpus_has_one_semantic_oracle_for_both_native_adapters() {
    let Some(typst_path) = required_typst() else {
        return;
    };

    let mut observations = Vec::new();
    for fixture in parity_fixtures() {
        let observation = observe_fixture(&fixture, &typst_path);
        assert_fixture_expectation(&fixture, &observation);
        observations.push((fixture.name, observation));
    }

    for (name, observation) in observations {
        println!(
            "[parity] fixture={name} subprocess={} in-process={} subprocess_pdf={:?} in-process_pdf={:?}",
            observation.subprocess.outcome,
            observation.in_process.outcome,
            observation.subprocess.pdf,
            observation.in_process.pdf,
        );
    }
}

struct ParityObservation {
    subprocess: BackendObservation,
    in_process: BackendObservation,
}

fn observe_fixture(fixture: &ParityFixture, typst_path: &Path) -> ParityObservation {
    for marker in fixture.expectation.source_markers {
        assert!(
            fixture.input.source.contains(marker),
            "fixture '{}' lost generated-source marker {:?}:\n{}",
            fixture.name,
            marker,
            fixture.input.source
        );
    }

    let in_process = observe_in_process(fixture);

    // SubprocessBackend deliberately owns native filesystem staging. Populate
    // that staging context from the already-built VirtualProject so both
    // observations use the same logical sources and assets.
    let project_root = materialize_project(&fixture.project);
    let subprocess_backend = SubprocessBackend::new(typst_path)
        .with_source_context(TypstSourceContext::new(project_root.path()));
    let subprocess = observe_subprocess(fixture, &subprocess_backend);

    ParityObservation {
        subprocess,
        in_process,
    }
}

fn observe_in_process(fixture: &ParityFixture) -> BackendObservation {
    match InProcessBackend::new(&fixture.project)
        .compile_with_source_map(&fixture.input, &fixture.source_map)
    {
        Ok(output) => observation_from_output(output, &fixture.project),
        Err(error) => observation_from_in_process_error(error, &fixture.project),
    }
}

fn observe_subprocess(fixture: &ParityFixture, backend: &SubprocessBackend) -> BackendObservation {
    match backend.compile(&fixture.input) {
        Ok(output) => observation_from_output(output, &fixture.project),
        Err(error) => observation_from_text(
            &error.to_string(),
            OutcomeClass::CompilationFailure,
            &fixture.project,
        ),
    }
}

fn observation_from_output(output: TypstOutput, project: &VirtualProject) -> BackendObservation {
    let diagnostics = output
        .diagnostics
        .iter()
        .map(|message| diagnostic_observation(message, None, project))
        .collect();
    BackendObservation {
        outcome: OutcomeClass::Success,
        pdf: output.pdf.map(|pdf| pdf_observation(&pdf)),
        diagnostics,
    }
}

fn observation_from_in_process_error(
    error: InProcessError,
    project: &VirtualProject,
) -> BackendObservation {
    let rendered = error.to_string();
    let diagnostics = error
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic_observation(&diagnostic.message, diagnostic.primary, project))
        .collect();
    BackendObservation {
        outcome: classify_error(&rendered),
        pdf: None,
        diagnostics,
    }
}

fn observation_from_text(
    text: &str,
    fallback: OutcomeClass,
    project: &VirtualProject,
) -> BackendObservation {
    BackendObservation {
        outcome: classify_error_with_fallback(text, fallback),
        pdf: None,
        diagnostics: vec![diagnostic_observation(text, None, project)],
    }
}

fn classify_error(text: &str) -> OutcomeClass {
    classify_error_with_fallback(text, OutcomeClass::CompilationFailure)
}

fn classify_error_with_fallback(text: &str, fallback: OutcomeClass) -> OutcomeClass {
    let lower = text.to_ascii_lowercase();
    if lower.contains("project boundary")
        || lower.contains("leaves the project")
        || lower.contains("outside.svg")
        || lower.contains("traversal")
    {
        OutcomeClass::TraversalDenied
    } else if lower.contains("package") || lower.contains("@preview") {
        OutcomeClass::PackageDenied
    } else if lower.contains("missing.svg")
        || lower.contains("file not found")
        || lower.contains("resource not found")
    {
        OutcomeClass::ResourceFailure
    } else if lower.contains("invalid typst entry path")
        || lower.contains("invalid in-process typst input")
    {
        OutcomeClass::InvalidInput
    } else if lower.contains("compilation failed") || lower.contains("compile failed") {
        OutcomeClass::CompilationFailure
    } else {
        fallback
    }
}

fn diagnostic_observation(
    text: &str,
    original_span: Option<SourceSpan>,
    project: &VirtualProject,
) -> DiagnosticObservation {
    DiagnosticObservation {
        logical_path: find_logical_path(text, project),
        original_span,
        host_path_leakage: contains_host_path(text),
        remote_url_leakage: text.contains("http://") || text.contains("https://"),
    }
}

fn find_logical_path(text: &str, project: &VirtualProject) -> Option<String> {
    let mut candidates = project
        .sources()
        .iter()
        .map(|(_, path, _)| path.as_str().to_string())
        .chain(
            project
                .assets()
                .iter()
                .map(|(path, _)| path.as_str().to_string()),
        )
        .collect::<Vec<_>>();
    if let Some((_, entry_name)) = project.entry().as_str().rsplit_once('/') {
        let stem = entry_name
            .rsplit_once('.')
            .map_or(entry_name, |(stem, _)| stem);
        let generated_parent = project
            .entry()
            .parent()
            .unwrap_or_else(scribium_project::VirtualPathBuf::root);
        let generated = if generated_parent.is_root() {
            format!("{stem}.typ")
        } else {
            format!("{}/{}.typ", generated_parent.as_str(), stem)
        };
        candidates.push(generated);
    }
    candidates
        .into_iter()
        .find(|path| text.contains(&format!("/{path}")) || text.contains(path))
        .map(|path| format!("/{path}"))
}

fn contains_host_path(text: &str) -> bool {
    [
        "/tmp/",
        "\\tmp\\",
        "/private/var/",
        "/var/folders/",
        "/Users/",
        "\\Users\\",
        "/home/runner/",
        "\\home\\runner\\",
    ]
    .iter()
    .any(|marker| {
        text.match_indices(marker)
            .any(|(index, _)| path_token_boundary(text.as_bytes(), index))
    }) || contains_windows_drive_path(text)
        || contains_unc_path(text)
}

fn contains_windows_drive_path(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.windows(3).enumerate().any(|(index, window)| {
        path_token_boundary(bytes, index)
            && window[0].is_ascii_alphabetic()
            && window[1] == b':'
            && matches!(window[2], b'/' | b'\\')
    })
}

fn contains_unc_path(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.windows(2).enumerate().any(|(index, window)| {
        path_token_boundary(bytes, index) && matches!(window, [b'\\', b'\\'] | [b'/', b'/'])
    })
}

fn path_token_boundary(bytes: &[u8], index: usize) -> bool {
    index == 0
        || bytes[index - 1].is_ascii_whitespace()
        || matches!(
            bytes[index - 1],
            b'(' | b'[' | b'{' | b'<' | b':' | b'=' | b'"' | b'\''
        )
}

#[test]
fn parity_path_oracle_distinguishes_logical_components_from_native_paths() {
    assert!(!contains_host_path(
        "<typst-build>/project/docs/target/Users/main.typ:1:1"
    ));
    assert!(!contains_host_path("/docs/target/Users/main.typ:1:1"));
    assert!(!contains_host_path("docs/target/Users/main.typ:1:1"));
    assert!(contains_host_path("/tmp/build/project/docs/main.typ:1:1"));
    assert!(contains_host_path("C:/Users/runner/main.typ:1:1"));
    assert!(contains_host_path(r"\\server\share\main.typ:1:1"));
}

fn assert_fixture_expectation(fixture: &ParityFixture, observation: &ParityObservation) {
    assert_eq!(
        observation.subprocess.outcome, fixture.expectation.outcome,
        "fixture '{}' subprocess classification",
        fixture.name
    );
    assert_eq!(
        observation.in_process.outcome, fixture.expectation.outcome,
        "fixture '{}' in-process classification",
        fixture.name
    );

    assert_eq!(
        observation.subprocess.outcome, observation.in_process.outcome,
        "fixture '{}' backend outcome parity: subprocess={:?}, in-process={:?}",
        fixture.name, observation.subprocess, observation.in_process
    );

    for (backend, backend_observation) in [
        ("subprocess", &observation.subprocess),
        ("in-process", &observation.in_process),
    ] {
        for diagnostic in &backend_observation.diagnostics {
            assert!(
                !diagnostic.host_path_leakage,
                "fixture '{}' {backend} diagnostic leaked a host path",
                fixture.name
            );
            if fixture.expectation.outcome == OutcomeClass::PackageDenied {
                assert!(
                    !diagnostic.remote_url_leakage,
                    "fixture '{}' {backend} package diagnostic leaked a remote URL",
                    fixture.name
                );
            }
        }
    }

    if fixture.expectation.outcome == OutcomeClass::Success {
        let subprocess_pdf = observation
            .subprocess
            .pdf
            .as_ref()
            .unwrap_or_else(|| panic!("fixture '{}' subprocess produced no PDF", fixture.name));
        let in_process_pdf = observation
            .in_process
            .pdf
            .as_ref()
            .unwrap_or_else(|| panic!("fixture '{}' in-process produced no PDF", fixture.name));
        assert_valid_pdf(fixture.name, "subprocess", subprocess_pdf);
        assert_valid_pdf(fixture.name, "in-process", in_process_pdf);
        assert_eq!(
            subprocess_pdf.page_count, in_process_pdf.page_count,
            "fixture '{}' observable page count diverged",
            fixture.name
        );
        if let Some(minimum_pages) = fixture.expectation.minimum_pages {
            assert!(
                subprocess_pdf
                    .page_count
                    .is_some_and(|count| count >= minimum_pages),
                "fixture '{}' subprocess page count is below {minimum_pages}: {:?}",
                fixture.name,
                subprocess_pdf
            );
            assert!(
                in_process_pdf
                    .page_count
                    .is_some_and(|count| count >= minimum_pages),
                "fixture '{}' in-process page count is below {minimum_pages}: {:?}",
                fixture.name,
                in_process_pdf
            );
        }
    }

    if fixture.expectation.require_logical_diagnostic_path {
        for (backend, backend_observation) in [
            ("subprocess", &observation.subprocess),
            ("in-process", &observation.in_process),
        ] {
            assert!(
                backend_observation
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.logical_path.is_some()),
                "fixture '{}' {backend} diagnostic did not retain a logical path: {:?}",
                fixture.name,
                backend_observation.diagnostics
            );
        }
    }

    if fixture.expectation.require_in_process_span {
        let expected_span = fixture
            .expectation
            .expected_in_process_span
            .expect("mapped fixture must declare its expected original span");
        assert!(
            observation.in_process.diagnostics.iter().any(|diagnostic| {
                diagnostic.original_span == Some(expected_span) && diagnostic.logical_path.is_some()
            }),
            "fixture '{}' did not preserve a mapped original SourceSpan: {:?}",
            fixture.name,
            observation.in_process.diagnostics
        );
        assert!(
            observation
                .subprocess
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.original_span.is_none()),
            "fixture '{}' unexpectedly gave subprocess a structured SourceSpan",
            fixture.name
        );
    }

    if fixture.expectation.require_in_process_unmapped {
        assert!(
            observation
                .in_process
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.original_span.is_none()),
            "fixture '{}' fabricated an unavailable or ambiguous SourceSpan: {:?}",
            fixture.name,
            observation.in_process.diagnostics
        );
    }
}

fn assert_valid_pdf(name: &str, backend: &str, pdf: &PdfObservation) {
    assert!(pdf.non_empty, "fixture '{}' {backend} PDF is empty", name);
    assert!(
        pdf.valid_header,
        "fixture '{}' {backend} PDF has no %PDF- header",
        name
    );
    assert!(
        pdf.valid_eof,
        "fixture '{}' {backend} PDF has no %%EOF marker",
        name
    );
    assert!(
        pdf.page_count.is_some(),
        "fixture '{}' {backend} PDF has no observable page count",
        name
    );
}

fn pdf_observation(pdf: &[u8]) -> PdfObservation {
    PdfObservation {
        non_empty: !pdf.is_empty(),
        valid_header: pdf.starts_with(b"%PDF-"),
        valid_eof: pdf.windows(b"%%EOF".len()).any(|window| window == b"%%EOF"),
        page_count: pdf_page_count(pdf),
    }
}

fn pdf_page_count(pdf: &[u8]) -> Option<usize> {
    let marker = b"/Type/Pages/Count";
    let start = pdf
        .windows(marker.len())
        .position(|window| window == marker)?
        + marker.len();
    let digits = pdf
        .get(start..)?
        .iter()
        .skip_while(|byte| byte.is_ascii_whitespace());
    let mut count = 0_usize;
    let mut found_digit = false;
    for byte in digits {
        if byte.is_ascii_digit() {
            found_digit = true;
            count = count
                .checked_mul(10)?
                .checked_add(usize::from(*byte - b'0'))?;
        } else {
            break;
        }
    }
    found_digit.then_some(count)
}

fn required_typst() -> Option<PathBuf> {
    let path = if let Some(path) = std::env::var_os("SCRIBIUM_TYPST_PATH") {
        let path = PathBuf::from(path);
        if !path.is_file() {
            return missing_typst("SCRIBIUM_TYPST_PATH does not name a file");
        }
        path
    } else if Command::new("typst")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
    {
        PathBuf::from("typst")
    } else {
        let homebrew = PathBuf::from("/opt/homebrew/bin/typst");
        if homebrew.is_file() {
            homebrew
        } else {
            return missing_typst("no Typst executable found");
        }
    };

    let output = Command::new(&path)
        .arg("--version")
        .output()
        .unwrap_or_else(|error| panic!("cannot execute Typst at '{}': {error}", path.display()));
    let version = String::from_utf8_lossy(&output.stdout);
    assert!(
        version.trim_start().starts_with("typst 0.15.1"),
        "parity suite requires Typst 0.15.1, found {:?} at {}",
        version.trim(),
        path.display()
    );
    eprintln!("[parity] using pinned Typst 0.15.1 at {}", path.display());
    Some(path)
}

fn missing_typst(reason: &str) -> Option<PathBuf> {
    if std::env::var_os("SCRIBIUM_REQUIRE_TYPST").is_some() {
        panic!("parity suite requires Typst 0.15.1: {reason}");
    }
    eprintln!("[parity] {reason}; skipping");
    None
}

fn parity_fixtures() -> Vec<ParityFixture> {
    vec![
        lowered_fixture(
            "generated-simple",
            "fixtures/parity/simple.qd",
            "# Simple document\n\nA generated Scribium document.\n",
            &[],
            success_expectation(&["Simple document", "generated Scribium"]),
        ),
        lowered_fixture(
            "nested-inline-block",
            "fixtures/parity/nested.qd",
            "# Nested document\n\n> **outer _nested_ text**\n>\n> - first\n> - second\n\n- parent\n  - child\n",
            &[],
            success_expectation(&["outer", "#quote"]),
        ),
        lowered_fixture(
            "real-quarkdown",
            "examples/hello/main.qd",
            include_str!("../../../examples/hello/main.qd"),
            &[],
            success_expectation(&["Hello, Scribium", "Hello world"]),
        ),
        lowered_fixture(
            "real-markdown",
            "examples/markdown/basic.md",
            include_str!("../../../examples/markdown/basic.md"),
            &[],
            success_expectation(&["Scribium Markdown example", "strong text"]),
        ),
        lowered_fixture(
            "real-gfm",
            "examples/markdown/gfm.md",
            include_str!("../../../examples/markdown/gfm.md"),
            &[],
            success_expectation(&["GFM example", "Tables"]),
        ),
        lowered_fixture(
            "real-bounded-html",
            "examples/markdown/bounded-html.md",
            include_str!("../../../examples/markdown/bounded-html.md"),
            &[],
            success_expectation(&["Bounded inline HTML example", "Emphasis"]),
        ),
        lowered_fixture(
            "multi-page",
            "fixtures/parity/multi-page.qd",
            &format!(
                "# Multi-page document\n\n{}",
                "A repeated paragraph exercises the paged document oracle.\n\n".repeat(100)
            ),
            &[],
            ParityExpectation {
                outcome: OutcomeClass::Success,
                minimum_pages: Some(2),
                source_markers: &["Multi-page document", "paged document oracle"],
                require_logical_diagnostic_path: false,
                require_in_process_span: false,
                require_in_process_unmapped: false,
                expected_in_process_span: None,
            },
        ),
        lowered_fixture(
            "image-resource",
            "docs/main.md",
            "# Image resource\n\n![logo](./assets/logo.svg)\n",
            &[("docs/assets/logo.svg", SVG_FIXTURE)],
            success_expectation(&["Image resource", "#image(\"./assets/logo.svg\")"]),
        ),
        lowered_fixture(
            "repeated-resource",
            "docs/main.md",
            "# Repeated resource\n\n![one](./assets/logo.svg)\n\n![two](./assets/logo.svg)\n",
            &[("docs/assets/logo.svg", SVG_FIXTURE)],
            success_expectation(&["#image(\"./assets/logo.svg\")"]),
        ),
        font_fixture(),
        lowered_fixture(
            "missing-resource",
            "docs/main.md",
            "# Missing resource\n\n![missing](./assets/missing.svg)\n",
            &[],
            failure_expectation(OutcomeClass::ResourceFailure),
        ),
        lowered_fixture(
            "traversal",
            "docs/main.md",
            "# Traversal\n\n![outside](../../outside.svg)\n",
            &[],
            failure_expectation(OutcomeClass::TraversalDenied),
        ),
        typst_override_fixture(
            "package-denial-preview",
            "docs/main.qd",
            "# Package denial\n",
            "#import \"@preview/not-present:1.0.0\": *\n",
            failure_expectation(OutcomeClass::PackageDenied),
        ),
        typst_override_fixture(
            "package-denial-local",
            "docs/main.qd",
            "# Package denial\n",
            "#import \"@local/company-package:1.0.0\": *\n",
            failure_expectation(OutcomeClass::PackageDenied),
        ),
        typst_override_fixture(
            "package-denial-arbitrary-namespace",
            "docs/main.qd",
            "# Package denial\n",
            "#include \"@company/internal-package:2.3.4\"\n",
            failure_expectation(OutcomeClass::PackageDenied),
        ),
        typst_override_fixture(
            "package-looking-inert-text",
            "docs/main.qd",
            "# Package-looking inert text\n",
            "```typst\n#import \"@preview/raw-block:1.0.0\": *\n```\n#let text = \"@local/example:1.0.0\"\n#raw(\"#import \\\"@company/example:1.0.0\\\": *\")\n",
            success_expectation(&["@preview/raw-block", "@local/example", "@company/example"]),
        ),
        typst_override_fixture_with_sources(
            "nested-local-module-package-preview",
            "docs/main.qd",
            "# Nested package denial\n",
            "#import \"./helper.typ\": *\n",
            &[(
                "docs/helper.typ",
                "#import \"@preview/nested-package:1.0.0\": *\n",
            )],
            failure_expectation(OutcomeClass::PackageDenied),
        ),
        typst_override_fixture_with_sources(
            "nested-local-module-package-local",
            "docs/main.qd",
            "# Nested package denial\n",
            "#import \"./helper.typ\": *\n",
            &[(
                "docs/helper.typ",
                "#import \"@local/nested-package:1.0.0\": *\n",
            )],
            failure_expectation(OutcomeClass::PackageDenied),
        ),
        typst_override_fixture_with_sources(
            "nested-local-module-inert-package-looking-text",
            "docs/main.qd",
            "# Nested inert module\n",
            "#import \"./helper.typ\": *\n#helper_value\n",
            &[
                (
                    "docs/helper.typ",
                    "// #import \"@preview/inert:1.0.0\": *\n#let helper_value = \"@local/inert:1.0.0\"\n",
                ),
                (
                    "docs/unused.typ",
                    "#import \"@company/unused:1.0.0\": *\n",
                ),
            ],
            success_expectation(&[]),
        ),
        invalid_generated_fixture("invalid-generated", false, false),
        invalid_generated_fixture("mapped-diagnostic", true, false),
        invalid_generated_fixture("ambiguous-diagnostic", true, true),
    ]
}

const SVG_FIXTURE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><rect width="20" height="20" fill="red"/></svg>"#;

fn success_expectation(source_markers: &'static [&'static str]) -> ParityExpectation {
    ParityExpectation {
        outcome: OutcomeClass::Success,
        minimum_pages: None,
        source_markers,
        require_logical_diagnostic_path: false,
        require_in_process_span: false,
        require_in_process_unmapped: false,
        expected_in_process_span: None,
    }
}

fn failure_expectation(outcome: OutcomeClass) -> ParityExpectation {
    ParityExpectation {
        outcome,
        minimum_pages: None,
        source_markers: &[],
        require_logical_diagnostic_path: true,
        require_in_process_span: false,
        require_in_process_unmapped: false,
        expected_in_process_span: None,
    }
}

fn lowered_fixture(
    name: &'static str,
    entry: &'static str,
    source: &str,
    assets: &[(&str, &[u8])],
    expectation: ParityExpectation,
) -> ParityFixture {
    lowered_fixture_with_sources(name, entry, source, assets, &[], expectation)
}

fn lowered_fixture_with_sources(
    name: &'static str,
    entry: &'static str,
    source: &str,
    assets: &[(&str, &[u8])],
    additional_sources: &[(&str, &str)],
    expectation: ParityExpectation,
) -> ParityFixture {
    let project = project(entry, source, assets, additional_sources);
    let result = compile(&project, &CompileOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "fixture '{}' did not compile through Scribium: {:?}",
        name,
        result.diagnostics
    );
    let (generated_source, source_map) = lower_to_typst(&result.ir);
    ParityFixture {
        name,
        input: TypstInput {
            source: generated_source,
            entry_path: project.entry().as_str().to_string(),
        },
        project,
        source_map,
        expectation,
    }
}

fn typst_override_fixture(
    name: &'static str,
    entry: &'static str,
    source: &str,
    generated_source: &str,
    expectation: ParityExpectation,
) -> ParityFixture {
    typst_override_fixture_with_sources(name, entry, source, generated_source, &[], expectation)
}

fn typst_override_fixture_with_sources(
    name: &'static str,
    entry: &'static str,
    source: &str,
    generated_source: &str,
    additional_sources: &[(&str, &str)],
    mut expectation: ParityExpectation,
) -> ParityFixture {
    let mut fixture = lowered_fixture_with_sources(
        name,
        entry,
        source,
        &[],
        additional_sources,
        success_expectation(&[]),
    );
    expectation.source_markers = &[];
    fixture.input.source = generated_source.to_string();
    fixture.source_map.clear();
    fixture.expectation = expectation;
    fixture
}

fn invalid_generated_fixture(name: &'static str, mapped: bool, ambiguous: bool) -> ParityFixture {
    let source = "# Diagnostic source\n\nThis is a valid Scribium source.\n";
    let mut fixture = lowered_fixture(
        name,
        "docs/target/Users/main.qd",
        source,
        &[],
        failure_expectation(OutcomeClass::CompilationFailure),
    );
    let invalid_start = fixture.input.source.len() + 1;
    fixture.input.source.push_str("\n#unknown-function()\n");

    if mapped {
        let main_id = fixture
            .project
            .sources()
            .get_id(fixture.project.entry())
            .expect("entry source id");
        let mapped_original = SourceSpan::new(main_id, 0, source.len());
        fixture.source_map.push(SourceMapEntry {
            generated_start: invalid_start,
            generated_end: fixture.input.source.len(),
            original: mapped_original,
        });

        if ambiguous {
            let project = project(
                "docs/target/Users/main.qd",
                source,
                &[],
                &[("docs/target/Users/other.qd", "another valid source")],
            );
            let other_id = project
                .sources()
                .get_id(
                    &scribium_project::VirtualPathBuf::parse("docs/target/Users/other.qd")
                        .expect("path"),
                )
                .expect("other source id");
            fixture.project = project;
            fixture.source_map.push(SourceMapEntry {
                generated_start: invalid_start,
                generated_end: fixture.input.source.len(),
                original: SourceSpan::new(other_id, 0, "another valid source".len()),
            });
        }
    } else {
        fixture.source_map.clear();
    }

    fixture.expectation.require_logical_diagnostic_path = true;
    fixture.expectation.require_in_process_span = mapped && !ambiguous;
    fixture.expectation.require_in_process_unmapped = !mapped || ambiguous;
    if mapped && !ambiguous {
        fixture.expectation.expected_in_process_span =
            fixture.source_map.last().map(|entry| entry.original);
    }
    fixture
}

fn font_fixture() -> ParityFixture {
    let font = typst_assets::fonts()
        .next()
        .expect("Typst embedded font fixture")
        .to_vec();
    let mut fixture = lowered_fixture(
        "project-font",
        "docs/main.qd",
        "# Project font\n\nThis fixture supplies the same font bytes through VirtualProject.\n",
        &[("fonts/project.otf", &font)],
        success_expectation(&["Project font"]),
    );
    // Font selection is not yet a Scribium semantic, so this deterministic
    // adapter fixture adds the Typst text rule after real lowering. The font
    // bytes are still owned by VirtualProject and no system-font name is used.
    fixture.input.source = format!(
        "#set text(font: \"Libertinus Serif\")\n{}",
        fixture.input.source
    );
    fixture.expectation.source_markers = &["#set text(font: \"Libertinus Serif\")", "Project font"];
    fixture
}

fn project(
    entry: &str,
    entry_source: &str,
    assets: &[(&str, &[u8])],
    additional_sources: &[(&str, &str)],
) -> VirtualProject {
    let mut builder = VirtualProjectBuilder::new()
        .entry(entry)
        .expect("valid parity entry")
        .add_source(entry, entry_source)
        .expect("valid parity source");
    for (path, source) in additional_sources {
        builder = builder
            .add_source(path, *source)
            .expect("valid additional parity source");
    }
    for (path, data) in assets {
        builder = builder
            .add_asset(path, data.to_vec())
            .expect("valid parity asset");
    }
    builder.build().expect("valid parity project")
}

fn materialize_project(project: &VirtualProject) -> TempDir {
    let directory = tempdir().expect("parity project temp directory");
    for (_, path, source) in project.sources().iter() {
        write_logical_file(directory.path(), path.as_str(), source.as_bytes());
    }
    for (path, data) in project.assets().iter() {
        write_logical_file(directory.path(), path.as_str(), data);
    }
    directory
}

fn write_logical_file(root: &Path, logical_path: &str, data: &[u8]) {
    let mut native_path = root.to_path_buf();
    for component in logical_path.split('/') {
        native_path.push(component);
    }
    if let Some(parent) = native_path.parent() {
        fs::create_dir_all(parent).expect("parity fixture directory");
    }
    fs::write(native_path, data).expect("parity fixture file");
}
