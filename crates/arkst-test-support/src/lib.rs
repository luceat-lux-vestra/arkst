//! `arkst-test-support` — Test utilities for Arkst.
//!
//! Provides:
//! - Fixture loading from `fixtures/` directories
//! - Golden test assertion helpers
//! - Temporary project builder for integration tests
//! - Normalized path and output comparison
//! - Quarkdown conformance corpus harness

use arkst_core as core;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    // CARGO_MANIFEST_DIR points to the crate directory (crates/arkst-test-support)
    // Go up two levels to reach the workspace root
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .expect("could not determine workspace root")
}

fn fixtures_dir() -> PathBuf {
    workspace_root().join("fixtures")
}

/// Load a fixture file by name from `fixtures/{category}/`.
pub fn load_fixture(category: &str, name: &str) -> String {
    let path = fixtures_dir().join(category).join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot load fixture {:?}: {}", path, e))
}

/// Assertion helper for golden tests.
/// Compares actual output against an expected file.
pub fn assert_golden(actual: &str, expected_path: &str) {
    let expected = std::fs::read_to_string(expected_path)
        .unwrap_or_else(|e| panic!("cannot read golden {:?}: {}", expected_path, e));
    assert_eq!(actual, expected, "golden mismatch for {}", expected_path);
}

/// The executable policy declared by a Quarkdown conformance case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String")]
pub enum CompatibilityLevel {
    #[serde(rename = "Unsupported")]
    Unsupported,
    #[serde(rename = "Parsed")]
    Parsed,
    #[serde(rename = "Semantically supported")]
    SemanticallySupported,
    #[serde(rename = "Output-equivalent")]
    OutputEquivalent,
    #[serde(rename = "Known divergence")]
    KnownDivergence,
}

impl TryFrom<String> for CompatibilityLevel {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "Unsupported" => Ok(Self::Unsupported),
            "Parsed" => Ok(Self::Parsed),
            "Semantically supported" => Ok(Self::SemanticallySupported),
            "Output-equivalent" => Ok(Self::OutputEquivalent),
            "Known divergence" => Ok(Self::KnownDivergence),
            _ => Err(format!("unknown compatibility level '{value}'")),
        }
    }
}

/// Quarkdown conformance case metadata.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ConformanceCaseMeta {
    pub id: String,
    pub feature: String,
    pub compatibility_level: CompatibilityLevel,
    pub specification_source: String,
    pub description: String,
    #[serde(default)]
    pub known_divergence: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
struct ExpectedSpan {
    start: usize,
    end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
struct ExpectedDiagnostic {
    code: String,
    severity: String,
    primary: Option<ExpectedSpan>,
    #[serde(default)]
    secondary: Vec<ExpectedSpan>,
}

/// A conformance test case from the corpus.
#[derive(Debug, Clone)]
pub struct ConformanceCase {
    pub meta: ConformanceCaseMeta,
    pub input: String,
    pub case_dir: PathBuf,
}

impl ConformanceCase {
    /// Load a conformance case by ID from the corpus.
    pub fn load(case_id: &str) -> Self {
        let cases_dir = fixtures_dir().join("quarkdown-conformance/cases");
        Self::load_from_dir(cases_dir.join(case_id))
    }

    fn load_from_dir(case_dir: PathBuf) -> Self {
        let input_path = case_dir.join("input.qd");

        let directory_id = case_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_else(|| {
                panic!("conformance case directory has no valid id: {:?}", case_dir)
            });

        let meta = Self::read_metadata(&case_dir);

        assert_eq!(
            directory_id, meta.id,
            "conformance case directory id must match metadata id"
        );

        let input = std::fs::read_to_string(&input_path)
            .unwrap_or_else(|e| panic!("cannot read case input {:?}: {}", input_path, e));

        let case = Self {
            meta,
            input,
            case_dir,
        };
        case.validate_expected_artifacts();
        case
    }

    fn read_metadata(case_dir: &Path) -> ConformanceCaseMeta {
        let meta_path = case_dir.join("case.toml");
        let meta_content = std::fs::read_to_string(&meta_path)
            .unwrap_or_else(|e| panic!("cannot read case metadata {:?}: {}", meta_path, e));
        toml::from_str(&meta_content)
            .unwrap_or_else(|e| panic!("cannot parse case metadata {:?}: {}", meta_path, e))
    }

    fn validate_expected_artifacts(&self) {
        let expected_dir = self.case_dir.join("expected");
        let require = |name: &str| {
            let path = expected_dir.join(name);
            assert!(
                path.is_file(),
                "case '{}' at level {:?} requires expected artifact {:?}",
                self.meta.id,
                self.meta.compatibility_level,
                path
            );
        };

        match self.meta.compatibility_level {
            CompatibilityLevel::Parsed => {}
            CompatibilityLevel::SemanticallySupported => require("ir.json"),
            CompatibilityLevel::OutputEquivalent => {
                require("ir.json");
                require("typst.typ");
            }
            CompatibilityLevel::Unsupported => require("diagnostics.json"),
            CompatibilityLevel::KnownDivergence => {
                assert!(
                    self.meta
                        .known_divergence
                        .as_deref()
                        .is_some_and(|description| !description.trim().is_empty()),
                    "Known divergence case '{}' must declare a non-empty known_divergence",
                    self.meta.id
                );
                require("ir.json");
            }
        }

        if !matches!(
            self.meta.compatibility_level,
            CompatibilityLevel::KnownDivergence
        ) && self
            .meta
            .known_divergence
            .as_deref()
            .is_some_and(|description| description.trim().is_empty())
        {
            panic!("case '{}' declares an empty known_divergence", self.meta.id);
        }
    }

    /// Get all case IDs in the corpus.
    pub fn list_all() -> Vec<String> {
        let cases_dir = fixtures_dir().join("quarkdown-conformance/cases");
        let mut ids = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&cases_dir) {
            for entry in entries.flatten() {
                if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false)
                    && entry.path().join("case.toml").exists()
                {
                    ids.push(entry.file_name().to_string_lossy().to_string());
                }
            }
        }
        ids.sort();
        ids
    }

    fn validate_corpus_metadata(cases_dir: &Path) {
        let entries = std::fs::read_dir(cases_dir).unwrap_or_else(|e| {
            panic!(
                "cannot read conformance cases directory {:?}: {}",
                cases_dir, e
            )
        });

        let mut cases = Vec::new();
        for entry in entries {
            let entry =
                entry.unwrap_or_else(|e| panic!("cannot read conformance case entry: {}", e));
            let case_dir = entry.path();
            if !entry.file_type().is_ok_and(|file_type| file_type.is_dir())
                || !case_dir.join("case.toml").is_file()
            {
                continue;
            }

            let meta = Self::read_metadata(&case_dir);
            cases.push((case_dir, meta));
        }

        assert!(!cases.is_empty(), "conformance corpus contains no cases");

        let mut metadata_ids = HashSet::new();
        for (_, meta) in &cases {
            assert!(
                metadata_ids.insert(meta.id.clone()),
                "duplicate conformance case metadata id '{}'",
                meta.id
            );
        }

        for (case_dir, _) in cases {
            Self::load_from_dir(case_dir);
        }
    }

    /// Compile the case input using Arkst core and return the result.
    pub fn compile(&self) -> core::CompileResult {
        let project = core::VirtualProjectBuilder::new()
            .entry("main.qd")
            .expect("valid path")
            .add_source("main.qd", &self.input)
            .expect("valid path")
            .build()
            .unwrap();
        core::compile(&project, &core::CompileOptions::default())
    }

    /// Verify the case according to its declared compatibility policy.
    pub fn verify(&self) -> core::CompileResult {
        let result = self.compile();

        match self.meta.compatibility_level {
            CompatibilityLevel::Parsed => assert_parser_diagnostics_absent(self, &result),
            CompatibilityLevel::SemanticallySupported => {
                assert_parser_diagnostics_absent(self, &result);
                assert_no_diagnostics(self, &result);
                assert_ir_matches(self, &result);
            }
            CompatibilityLevel::OutputEquivalent => {
                assert_parser_diagnostics_absent(self, &result);
                assert_no_diagnostics(self, &result);
                assert_ir_matches(self, &result);
                let expected = read_expected_text(self, "typst.typ");
                let actual = arkst_typst::lowering::lower_to_typst_code(&result.ir);
                assert_eq!(
                    actual, expected,
                    "Typst golden mismatch for case '{}'",
                    self.meta.id
                );
            }
            CompatibilityLevel::Unsupported => {
                let expected =
                    read_expected_json::<Vec<ExpectedDiagnostic>>(self, "diagnostics.json");
                assert!(
                    !expected.is_empty(),
                    "Unsupported case '{}' must expect a diagnostic",
                    self.meta.id
                );
                let actual: Vec<_> = result.diagnostics.iter().map(project_diagnostic).collect();
                assert_eq!(
                    actual, expected,
                    "diagnostic golden mismatch for case '{}'",
                    self.meta.id
                );
                assert!(
                    result.diagnostics.iter().any(|diagnostic| {
                        matches!(diagnostic.severity, core::Severity::Error)
                            && !diagnostic.code.starts_with("E2")
                    }),
                    "Unsupported case '{}' must produce a deliberate error diagnostic",
                    self.meta.id
                );
            }
            CompatibilityLevel::KnownDivergence => {
                assert_parser_diagnostics_absent(self, &result);
                assert_no_diagnostics(self, &result);
                assert_ir_matches(self, &result);
            }
        }

        result
    }
}

fn assert_parser_diagnostics_absent(case: &ConformanceCase, result: &core::CompileResult) {
    let parser_errors: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code.starts_with("E2"))
        .collect();
    assert!(
        parser_errors.is_empty(),
        "Case '{}' produced parser errors: {:?}",
        case.meta.id,
        parser_errors
    );
}

fn read_expected_text(case: &ConformanceCase, name: &str) -> String {
    let path = case.case_dir.join("expected").join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read expected artifact {:?}: {}", path, e))
}

fn read_expected_json<T: serde::de::DeserializeOwned>(case: &ConformanceCase, name: &str) -> T {
    let path = case.case_dir.join("expected").join(name);
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read expected artifact {:?}: {}", path, e));
    serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("cannot parse expected artifact {:?}: {}", path, e))
}

fn assert_no_diagnostics(case: &ConformanceCase, result: &core::CompileResult) {
    assert!(
        result.diagnostics.is_empty(),
        "Case '{}' produced evaluation/lowering diagnostics: {:?}",
        case.meta.id,
        result.diagnostics
    );
}

fn assert_ir_matches(case: &ConformanceCase, result: &core::CompileResult) {
    let expected = read_expected_json::<core::ir::IrDocument>(case, "ir.json");
    assert_eq!(
        result.ir, expected,
        "semantic IR golden mismatch for case '{}'",
        case.meta.id
    );
}

fn project_diagnostic(diagnostic: &core::Diagnostic) -> ExpectedDiagnostic {
    let span = |span: core::SourceSpan| ExpectedSpan {
        start: span.start,
        end: span.end,
    };
    let severity = match diagnostic.severity {
        core::Severity::Error => "error",
        core::Severity::Warning => "warning",
        core::Severity::Hint => "hint",
    };
    ExpectedDiagnostic {
        code: diagnostic.code.clone(),
        severity: severity.to_string(),
        primary: diagnostic.primary.map(span),
        secondary: diagnostic.secondary.iter().copied().map(span).collect(),
    }
}

/// Run all conformance cases in the corpus.
pub fn run_all_conformance_cases() {
    let cases_dir = fixtures_dir().join("quarkdown-conformance/cases");
    ConformanceCase::validate_corpus_metadata(&cases_dir);
    for case_id in ConformanceCase::list_all() {
        let case = ConformanceCase::load(&case_id);
        let _result = case.verify();
        println!("✓ {} ({})", case.meta.id, case.meta.feature);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_conformance_cases() {
        let cases = ConformanceCase::list_all();
        assert!(
            !cases.is_empty(),
            "should have at least one conformance case"
        );
        println!("Found cases: {:?}", cases);
    }

    #[test]
    fn test_load_call_dot_prefixed_basic() {
        let case = ConformanceCase::load("call-dot-prefixed-basic");
        assert_eq!(case.meta.id, "call-dot-prefixed-basic");
        assert_eq!(case.meta.feature, "dot-prefixed-call");
        assert_eq!(case.meta.compatibility_level, CompatibilityLevel::Parsed);
        assert!(!case.input.is_empty());
    }

    #[test]
    fn test_load_call_positional_basic() {
        let case = ConformanceCase::load("call-positional-basic");
        assert_eq!(case.meta.id, "call-positional-basic");
        assert_eq!(case.meta.feature, "positional-arguments");
        assert_eq!(case.meta.compatibility_level, CompatibilityLevel::Parsed);
        assert!(!case.input.is_empty());
    }

    #[test]
    fn test_load_call_indented_body_basic() {
        let case = ConformanceCase::load("call-indented-body-basic");
        assert_eq!(case.meta.id, "call-indented-body-basic");
        assert_eq!(case.meta.feature, "indented-body");
        assert_eq!(case.meta.compatibility_level, CompatibilityLevel::Parsed);
        assert!(!case.input.is_empty());
    }

    #[test]
    fn test_load_doclang_document_state_fixture() {
        let case = ConformanceCase::load("doclang-family");
        assert_eq!(case.meta.id, "doclang-family");
        assert_eq!(case.meta.feature, "document-state-doclang");
        assert_eq!(
            case.meta.compatibility_level,
            CompatibilityLevel::SemanticallySupported
        );
        assert!(!case.input.is_empty());
    }

    #[test]
    fn test_load_doclang_locale_closure_fixture() {
        let case = ConformanceCase::load("doclang-locale-closure");
        assert_eq!(case.meta.id, "doclang-locale-closure");
        assert_eq!(case.meta.feature, "document-state-doclang-locale-closure");
        assert_eq!(
            case.meta.compatibility_level,
            CompatibilityLevel::SemanticallySupported
        );
        assert!(!case.input.is_empty());
    }

    #[test]
    fn quarkdown_conformance_corpus_obeys_declared_levels() {
        run_all_conformance_cases();
    }

    #[test]
    fn compatibility_level_deserializes_only_the_fixture_vocabulary() {
        let cases = [
            ("Unsupported", CompatibilityLevel::Unsupported),
            ("Parsed", CompatibilityLevel::Parsed),
            (
                "Semantically supported",
                CompatibilityLevel::SemanticallySupported,
            ),
            ("Output-equivalent", CompatibilityLevel::OutputEquivalent),
            ("Known divergence", CompatibilityLevel::KnownDivergence),
        ];
        for (value, expected) in cases {
            let parsed: CompatibilityLevel = serde_json::from_str(&format!("\"{value}\""))
                .expect("declared fixture level should deserialize");
            assert_eq!(parsed, expected);
        }
    }

    #[test]
    fn unknown_compatibility_level_fails_during_load() {
        let (root, case_dir) = temporary_case(
            "case-id",
            "Semantically Supported by accident",
            "hello",
            &[],
        );
        assert_panics_with(
            move || {
                ConformanceCase::load_from_dir(case_dir);
            },
            "unknown compatibility level",
        );
        drop(root);
    }

    #[test]
    fn parsed_level_only_requires_parser_acceptance() {
        let (root, case_dir) = temporary_case("case-id", "Parsed", ".abs {invalid}", &[]);
        let case = ConformanceCase::load_from_dir(case_dir);
        let result = case.verify();
        assert!(result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E3001"));
        drop(root);
    }

    #[test]
    fn semantic_level_requires_ir_artifact() {
        let (root, case_dir) = temporary_case("case-id", "Semantically supported", "hello", &[]);
        assert_panics_with(
            move || {
                ConformanceCase::load_from_dir(case_dir);
            },
            "requires expected artifact",
        );
        drop(root);
    }

    #[test]
    fn semantic_level_rejects_an_intentionally_wrong_ir() {
        let (root, case_dir) = temporary_case(
            "case-id",
            "Semantically supported",
            "hello",
            &[("expected/ir.json", EMPTY_IR)],
        );
        let case = ConformanceCase::load_from_dir(case_dir);
        assert_panics_with(
            move || {
                case.verify();
            },
            "semantic IR golden mismatch",
        );
        drop(root);
    }

    #[test]
    fn output_equivalent_requires_typst_artifact() {
        let (root, case_dir) = temporary_case(
            "case-id",
            "Output-equivalent",
            "hello",
            &[("expected/ir.json", EMPTY_IR)],
        );
        assert_panics_with(
            move || {
                ConformanceCase::load_from_dir(case_dir);
            },
            "requires expected artifact",
        );
        drop(root);
    }

    #[test]
    fn unsupported_requires_diagnostic_expectation() {
        let (root, case_dir) = temporary_case("case-id", "Unsupported", ".unknown", &[]);
        assert_panics_with(
            move || {
                ConformanceCase::load_from_dir(case_dir);
            },
            "requires expected artifact",
        );
        drop(root);
    }

    #[test]
    fn known_divergence_requires_explanation() {
        let (root, case_dir) = temporary_case(
            "case-id",
            "Known divergence",
            "hello",
            &[("expected/ir.json", EMPTY_IR)],
        );
        assert_panics_with(
            move || {
                ConformanceCase::load_from_dir(case_dir);
            },
            "must declare a non-empty known_divergence",
        );
        drop(root);
    }

    #[test]
    fn directory_name_must_match_metadata_id() {
        let (root, case_dir) =
            temporary_case_with_directory("directory-id", "metadata-id", "Parsed", "hello", &[]);
        assert_panics_with(
            move || {
                ConformanceCase::load_from_dir(case_dir);
            },
            "conformance case directory id must match metadata id",
        );
        drop(root);
    }

    #[test]
    fn duplicate_metadata_ids_fail_corpus_validation() {
        let root = tempfile::tempdir().expect("temporary corpus");
        for directory in ["first", "second"] {
            let case_dir = root.path().join(directory);
            std::fs::create_dir_all(&case_dir).expect("case directory");
            std::fs::write(
                case_dir.join("case.toml"),
                "id = \"duplicate\"\nfeature = \"test\"\ncompatibility_level = \"Parsed\"\nspecification_source = \"test\"\ndescription = \"test\"\n",
            )
            .expect("metadata");
            std::fs::write(case_dir.join("input.qd"), "hello").expect("input");
        }
        assert_panics_with(
            || ConformanceCase::validate_corpus_metadata(root.path()),
            "duplicate conformance case metadata id",
        );
    }

    const EMPTY_IR: &str = r#"{"nodes":[],"metadata":{"title":null,"author":null,"date":null,"raw":[],"document_state":{"name":"","description":"","document_type":"Plain","authors":[],"keywords":[],"theme":null}}}"#;

    fn temporary_case(
        directory_id: &str,
        level: &str,
        input: &str,
        artifacts: &[(&str, &str)],
    ) -> (tempfile::TempDir, std::path::PathBuf) {
        temporary_case_with_directory(directory_id, directory_id, level, input, artifacts)
    }

    fn temporary_case_with_directory(
        directory_id: &str,
        metadata_id: &str,
        level: &str,
        input: &str,
        artifacts: &[(&str, &str)],
    ) -> (tempfile::TempDir, std::path::PathBuf) {
        let root = tempfile::tempdir().expect("temporary corpus");
        let case_dir = root.path().join(directory_id);
        std::fs::create_dir_all(&case_dir).expect("case directory");
        std::fs::write(
            case_dir.join("case.toml"),
            format!(
                "id = \"{metadata_id}\"\nfeature = \"test\"\ncompatibility_level = \"{level}\"\nspecification_source = \"test\"\ndescription = \"test\"\n"
            ),
        )
        .expect("metadata");
        std::fs::write(case_dir.join("input.qd"), input).expect("input");
        for (relative_path, content) in artifacts {
            let path = case_dir.join(relative_path);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).expect("artifact directory");
            }
            std::fs::write(path, content).expect("artifact");
        }
        (root, case_dir)
    }

    fn assert_panics_with<F>(action: F, expected: &str)
    where
        F: FnOnce() + std::panic::UnwindSafe,
    {
        let panic = std::panic::catch_unwind(action).expect_err("expected the action to panic");
        let message = panic_message(panic);
        assert!(
            message.contains(expected),
            "unexpected panic: {message}; expected phrase: {expected}"
        );
    }

    fn panic_message(panic: Box<dyn std::any::Any + Send>) -> String {
        if let Some(message) = panic.downcast_ref::<String>() {
            message.clone()
        } else if let Some(message) = panic.downcast_ref::<&str>() {
            (*message).to_owned()
        } else {
            "non-string panic payload".to_owned()
        }
    }

    #[test]
    fn test_baseline_consistency() {
        // Verify that the verified baseline in upstream.toml matches
        // the explicitly declared reference version in documentation files.
        // This test extracts the declared baseline from specific patterns
        // to avoid false positives from historical version references.
        let root = workspace_root();

        // Read upstream.toml (authoritative source)
        let upstream_toml = root.join("docs/compatibility/quarkdown/upstream.toml");
        let upstream_content = std::fs::read_to_string(&upstream_toml)
            .unwrap_or_else(|e| panic!("cannot read upstream.toml: {}", e));
        let upstream: toml::Value = toml::from_str(&upstream_content)
            .unwrap_or_else(|e| panic!("cannot parse upstream.toml: {}", e));
        let baseline_from_toml = upstream["upstream"]["supported_baseline"]
            .as_str()
            .expect("upstream.supported_baseline not found in upstream.toml");

        // Helper to extract declared baseline from a document
        let version_re = regex::Regex::new(r"v\d+\.\d+\.\d+").unwrap();
        fn extract_declared_baseline(
            content: &str,
            patterns: &[&str],
            version_re: &regex::Regex,
        ) -> Option<String> {
            for pattern in patterns {
                if let Some(idx) = content.find(pattern) {
                    let after = &content[idx + pattern.len()..];
                    if let Some(mat) = version_re.find(after) {
                        return Some(mat.as_str().to_string());
                    }
                }
            }
            None
        }

        // SPEC_SOURCES.md: "Reference version: Quarkdown **vX.Y.Z**"
        let spec_sources = root.join("docs/compatibility/quarkdown/SPEC_SOURCES.md");
        let spec_content = std::fs::read_to_string(&spec_sources)
            .unwrap_or_else(|e| panic!("cannot read SPEC_SOURCES.md: {}", e));
        let spec_patterns = ["Reference version:", "Reference baseline:"];
        let spec_baseline = extract_declared_baseline(&spec_content, &spec_patterns, &version_re)
            .expect("SPEC_SOURCES.md should declare a reference baseline");
        assert_eq!(
            spec_baseline, baseline_from_toml,
            "SPEC_SOURCES.md declared baseline ({}) should match upstream.toml ({})",
            spec_baseline, baseline_from_toml
        );

        // README.md: "reference baseline vX.Y.Z" or "Reference upstream: Quarkdown vX.Y.Z"
        let readme = root.join("docs/compatibility/quarkdown/README.md");
        let readme_content = std::fs::read_to_string(&readme)
            .unwrap_or_else(|e| panic!("cannot read compatibility README.md: {}", e));
        let readme_patterns = ["reference baseline", "Reference upstream:"];
        let readme_baseline =
            extract_declared_baseline(&readme_content, &readme_patterns, &version_re)
                .expect("compatibility README.md should declare a reference baseline");
        assert_eq!(
            readme_baseline, baseline_from_toml,
            "compatibility README.md declared baseline ({}) should match upstream.toml ({})",
            readme_baseline, baseline_from_toml
        );

        // Root README.md
        let root_readme = root.join("README.md");
        let root_readme_content = std::fs::read_to_string(&root_readme)
            .unwrap_or_else(|e| panic!("cannot read root README.md: {}", e));
        // Use only the Quarkdown-specific pattern to avoid matching Arkst milestone versions
        // Pattern excludes the "v" prefix since the version regex expects it
        let root_patterns = ["referenced against Quarkdown "];
        let root_baseline =
            extract_declared_baseline(&root_readme_content, &root_patterns, &version_re)
                .expect("root README.md should declare a reference baseline");
        assert_eq!(
            root_baseline, baseline_from_toml,
            "root README.md declared baseline ({}) should match upstream.toml ({})",
            root_baseline, baseline_from_toml
        );

        println!("✓ Baseline consistency verified: {}", baseline_from_toml);
    }
}
