//! Differential Markdown compatibility harness.
//!
//! This binary is intentionally a test/oracle tool. It invokes a pinned
//! cmark-gfm executable, converts its XML tree and Scribium's frontend AST to
//! the same small semantic model, and records the comparison without using
//! HTML strings as an oracle.

use anyhow::{bail, Context, Result};
use quick_xml::events::Event;
use quick_xml::Reader;
use scribium_core::{compile, CompileOptions, VirtualProjectBuilder};
use scribium_markdown::ast::{Block, Document, Inline, ListItem, TableAlignment, TableRow};
use scribium_markdown::{parse_with_markdown_profile, MarkdownProfile};
use scribium_typst::backend::{SubprocessBackend, TypstBackend, TypstInput};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const REPORT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, PartialEq)]
struct CanonicalNode {
    kind: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    attrs: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    children: Vec<CanonicalNode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    value: Option<String>,
}

impl CanonicalNode {
    fn new(kind: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            attrs: BTreeMap::new(),
            children: Vec::new(),
            value: None,
        }
    }

    fn value(kind: impl Into<String>, value: impl Into<String>) -> Self {
        let mut node = Self::new(kind);
        node.value = Some(value.into());
        node
    }

    fn attr(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.attrs.insert(key.into(), value.into());
        self
    }

    fn children(mut self, children: Vec<CanonicalNode>) -> Self {
        self.children = coalesce_adjacent_text(children);
        self
    }
}

fn coalesce_adjacent_text(children: Vec<CanonicalNode>) -> Vec<CanonicalNode> {
    let mut result: Vec<CanonicalNode> = Vec::with_capacity(children.len());
    for child in children {
        if let Some(previous) = result.last_mut() {
            if previous.kind == "text" && child.kind == "text" {
                if let (Some(previous), Some(current)) =
                    (previous.value.as_mut(), child.value.as_ref())
                {
                    previous.push_str(current);
                    continue;
                }
            }
        }
        result.push(child);
    }
    result
}

#[derive(Debug, Clone, Deserialize)]
struct CorpusCase {
    id: String,
    number: u32,
    section: String,
    markdown: String,
}

#[derive(Debug, Clone, Deserialize)]
struct Baseline {
    schema_version: u32,
    reference: BaselineReference,
    #[serde(default)]
    cases: BTreeMap<String, BaselineCase>,
}

#[derive(Debug, Clone, Deserialize)]
struct BaselineReference {
    revision: String,
    #[serde(default)]
    parser_revision: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct BaselineCase {
    classification: String,
    reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Classification {
    Pass,
    KnownMismatch,
    Unsupported,
}

impl Classification {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "PASS" => Some(Self::Pass),
            "KNOWN_MISMATCH" => Some(Self::KnownMismatch),
            "UNSUPPORTED" => Some(Self::Unsupported),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::KnownMismatch => "KNOWN_MISMATCH",
            Self::Unsupported => "UNSUPPORTED",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BaselineGateStatus {
    Accepted,
    NewMismatch,
    StaleException,
    ClassificationChanged,
    InvalidBaseline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BaselineGateDecision {
    status: BaselineGateStatus,
    new_mismatch: bool,
    improvement: bool,
}

fn evaluate_baseline_gate(
    baseline_classification: Option<&str>,
    current: Classification,
) -> BaselineGateDecision {
    let Some(baseline_classification) = baseline_classification else {
        return BaselineGateDecision {
            status: if current == Classification::Pass {
                BaselineGateStatus::Accepted
            } else {
                BaselineGateStatus::NewMismatch
            },
            new_mismatch: current != Classification::Pass,
            improvement: false,
        };
    };

    let Some(expected) = Classification::parse(baseline_classification) else {
        return BaselineGateDecision {
            status: BaselineGateStatus::InvalidBaseline,
            new_mismatch: false,
            improvement: false,
        };
    };
    if expected == Classification::Pass {
        return BaselineGateDecision {
            status: BaselineGateStatus::InvalidBaseline,
            new_mismatch: false,
            improvement: false,
        };
    }
    if current == Classification::Pass {
        return BaselineGateDecision {
            status: BaselineGateStatus::StaleException,
            new_mismatch: false,
            improvement: true,
        };
    }
    if expected == current {
        return BaselineGateDecision {
            status: BaselineGateStatus::Accepted,
            new_mismatch: false,
            improvement: false,
        };
    }

    BaselineGateDecision {
        status: BaselineGateStatus::ClassificationChanged,
        new_mismatch: true,
        improvement: false,
    }
}

fn missing_baseline_case_ids<'a>(
    baseline_ids: impl Iterator<Item = &'a String>,
    corpus_ids: &BTreeSet<String>,
) -> Vec<String> {
    baseline_ids
        .filter(|id| !corpus_ids.contains(*id))
        .cloned()
        .collect()
}

#[derive(Debug, Clone, Deserialize)]
struct ReferenceConfig {
    schema_version: u32,
    commonmark: Reference,
    cmark: Reference,
    cmark_gfm: Reference,
}

#[derive(Debug, Clone, Deserialize)]
struct Reference {
    repository: String,
    version: String,
    revision: String,
    corpus_path: String,
    license: String,
}

#[derive(Debug, Clone, Deserialize)]
struct RealManifest {
    schema_version: u32,
    documents: Vec<RealDocumentSpec>,
}

#[derive(Debug, Clone, Deserialize)]
struct RealDocumentSpec {
    id: String,
    path: String,
    expected: String,
    #[serde(default)]
    markers: Vec<String>,
    #[serde(default)]
    diagnostics: Vec<String>,
    #[serde(default)]
    html_policy: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct SuiteSummary {
    total: usize,
    pass: usize,
    known_mismatch: usize,
    unsupported: usize,
    harness_error: usize,
    new_mismatch: usize,
    improvements: usize,
}

impl SuiteSummary {
    fn new() -> Self {
        Self {
            total: 0,
            pass: 0,
            known_mismatch: 0,
            unsupported: 0,
            harness_error: 0,
            new_mismatch: 0,
            improvements: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct CaseReport {
    id: String,
    number: u32,
    section: String,
    markdown: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    reference: Option<CanonicalNode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scribium: Option<CanonicalNode>,
    result: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    baseline_classification: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    baseline_reason: Option<String>,
    new_mismatch: bool,
    improvement: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    diff: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct SuiteReport {
    name: String,
    reference_version: String,
    reference_revision: String,
    summary: SuiteSummary,
    cases: Vec<CaseReport>,
    baseline_errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct RealDocumentReport {
    id: String,
    path: String,
    expected: String,
    result: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    html_policy: Option<String>,
    diagnostics: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    typst_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pdf_path: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    missing_markers: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct RealSummary {
    total: usize,
    successful_pdf: usize,
    expected_unsupported: usize,
    harness_error: usize,
}

#[derive(Debug, Clone, Serialize)]
struct RealReport {
    summary: RealSummary,
    documents: Vec<RealDocumentReport>,
}

#[derive(Debug, Clone, Serialize)]
struct FullReport {
    schema_version: u32,
    reference_config: String,
    suites: Vec<SuiteReport>,
    real_documents: RealReport,
    errors: Vec<String>,
}

#[derive(Debug, Clone)]
struct Args {
    commonmark_cmark: PathBuf,
    gfm_cmark: PathBuf,
    commonmark_corpus: PathBuf,
    gfm_corpus: PathBuf,
    commonmark_baseline: PathBuf,
    gfm_baseline: PathBuf,
    real_manifest: PathBuf,
    real_root: PathBuf,
    references: PathBuf,
    output_dir: PathBuf,
    typst: PathBuf,
}

impl Args {
    fn parse() -> Result<Self> {
        let mut values = BTreeMap::new();
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            let key = arg
                .strip_prefix("--")
                .with_context(|| format!("unknown argument {arg}"))?;
            let value = args
                .next()
                .with_context(|| format!("missing value for --{key}"))?;
            values.insert(key.to_string(), PathBuf::from(value));
        }

        let path = |key: &str, default: &str| {
            values
                .get(key)
                .cloned()
                .unwrap_or_else(|| PathBuf::from(default))
        };

        Ok(Self {
            commonmark_cmark: values
                .get("commonmark-cmark")
                .cloned()
                .context("--commonmark-cmark is required")?,
            gfm_cmark: values
                .get("gfm-cmark")
                .cloned()
                .context("--gfm-cmark is required")?,
            commonmark_corpus: path("commonmark-corpus", "tests/compat/corpus/commonmark.json"),
            gfm_corpus: path("gfm-corpus", "tests/compat/corpus/gfm.json"),
            commonmark_baseline: path(
                "commonmark-baseline",
                "tests/compat/baselines/commonmark.json",
            ),
            gfm_baseline: path("gfm-baseline", "tests/compat/baselines/gfm.json"),
            real_manifest: path("real-manifest", "fixtures/markdown/real/manifest.json"),
            real_root: path("real-root", "fixtures/markdown/real"),
            references: path("references", "tests/compat/references.toml"),
            output_dir: path("output-dir", "target/markdown-compat"),
            typst: path("typst", "typst"),
        })
    }
}

fn main() -> Result<()> {
    let args = Args::parse()?;
    fs::create_dir_all(&args.output_dir)
        .with_context(|| format!("cannot create {}", args.output_dir.display()))?;
    fs::create_dir_all(args.output_dir.join("failed-case-diffs"))?;
    fs::create_dir_all(args.output_dir.join("real-typst"))?;
    fs::create_dir_all(args.output_dir.join("real-pdf"))?;

    let references: ReferenceConfig = read_toml(&args.references)?;
    validate_references(&references)?;
    verify_cmark(&args.commonmark_cmark, &references.cmark.version)?;
    verify_cmark(&args.gfm_cmark, &references.cmark_gfm.version)?;

    let commonmark = run_suite(
        "CommonMark",
        &references.commonmark,
        &args.commonmark_corpus,
        &args.commonmark_baseline,
        &args.commonmark_cmark,
        &references.cmark,
        false,
        &args.output_dir,
    )?;
    let gfm = run_suite(
        "GFM",
        &references.cmark_gfm,
        &args.gfm_corpus,
        &args.gfm_baseline,
        &args.gfm_cmark,
        &references.cmark_gfm,
        true,
        &args.output_dir,
    )?;
    let real = run_real_documents(&args)?;

    let mut errors = Vec::new();
    for suite in [&commonmark, &gfm] {
        errors.extend(suite.baseline_errors.iter().cloned());
        for case in &suite.cases {
            if case.new_mismatch || case.error.is_some() || case.result == "HARNESS_ERROR" {
                errors.push(format!(
                    "{} {}: {}",
                    suite.name,
                    case.id,
                    case.error
                        .as_deref()
                        .or(case.diff.as_deref())
                        .unwrap_or("compatibility failure")
                ));
            }
        }
    }
    for document in &real.documents {
        if document.result == "HARNESS_ERROR" {
            errors.push(format!(
                "real document {}: {}",
                document.id,
                document.error.as_deref().unwrap_or("validation failure")
            ));
        }
    }

    let report = FullReport {
        schema_version: REPORT_SCHEMA_VERSION,
        reference_config: args.references.display().to_string(),
        suites: vec![commonmark, gfm],
        real_documents: real,
        errors,
    };
    write_report(&args.output_dir, &report)?;
    print_summary(&report);

    if !report.errors.is_empty() {
        bail!(
            "Markdown compatibility harness failed; see {} and {}",
            args.output_dir.join("compatibility-report.json").display(),
            args.output_dir.join("compatibility-report.md").display()
        );
    }
    Ok(())
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("cannot read JSON file {}", path.display()))?;
    serde_json::from_str(&content)
        .with_context(|| format!("cannot parse JSON file {}", path.display()))
}

fn read_toml<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("cannot read TOML file {}", path.display()))?;
    toml::from_str(&content).with_context(|| format!("cannot parse TOML file {}", path.display()))
}

fn validate_references(references: &ReferenceConfig) -> Result<()> {
    if references.schema_version != 1 {
        bail!(
            "unsupported reference config schema {}",
            references.schema_version
        );
    }
    for (name, reference) in [
        ("commonmark", &references.commonmark),
        ("cmark", &references.cmark),
        ("cmark_gfm", &references.cmark_gfm),
    ] {
        if reference.repository.is_empty()
            || reference.version.is_empty()
            || reference.revision.len() != 40
            || reference.corpus_path.is_empty()
            || reference.license.is_empty()
        {
            bail!("incomplete pinned reference metadata for {name}");
        }
    }
    Ok(())
}

fn verify_cmark(path: &Path, version: &str) -> Result<()> {
    let output = Command::new(path)
        .arg("--version")
        .output()
        .with_context(|| format!("cannot execute reference parser at {}", path.display()))?;
    if !output.status.success() {
        bail!("reference parser --version failed with {}", output.status);
    }
    let actual = String::from_utf8_lossy(&output.stdout);
    if !actual.contains(version) {
        bail!(
            "reference parser version mismatch: expected {version}, got {}",
            actual.trim()
        );
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_suite(
    name: &str,
    reference: &Reference,
    corpus_path: &Path,
    baseline_path: &Path,
    cmark: &Path,
    parser_reference: &Reference,
    gfm_extensions: bool,
    output_dir: &Path,
) -> Result<SuiteReport> {
    let corpus: Vec<CorpusCase> = read_json(corpus_path)?;
    let baseline: Baseline = read_json(baseline_path)?;
    if baseline.schema_version != 1 {
        bail!(
            "unsupported {name} baseline schema {}",
            baseline.schema_version
        );
    }
    if baseline.reference.revision != reference.revision {
        bail!(
            "{name} baseline revision {} does not match pinned reference {}",
            baseline.reference.revision,
            reference.revision
        );
    }
    if baseline.reference.parser_revision.as_deref() != Some(parser_reference.revision.as_str()) {
        bail!(
            "{name} baseline parser revision does not match pinned reference {}",
            parser_reference.revision
        );
    }

    let corpus_ids: BTreeSet<String> = corpus.iter().map(|case| case.id.clone()).collect();
    let mut baseline_errors = Vec::new();
    for baseline_id in missing_baseline_case_ids(baseline.cases.keys(), &corpus_ids) {
        baseline_errors.push(format!(
            "{name} baseline case {baseline_id} is absent from the pinned corpus"
        ));
    }

    let mut summary = SuiteSummary::new();
    let mut cases = Vec::with_capacity(corpus.len());
    for case in corpus {
        summary.total += 1;
        let baseline_case = baseline.cases.get(&case.id);
        let comparison = compare_case(&case, cmark, gfm_extensions);
        let report = match comparison {
            Ok((reference_tree, scribium_tree, unsupported)) => {
                let equal = reference_tree == scribium_tree;
                let current = if !equal && unsupported {
                    "UNSUPPORTED"
                } else if !equal {
                    "KNOWN_MISMATCH"
                } else if unsupported {
                    "UNSUPPORTED"
                } else {
                    "PASS"
                };
                let current = Classification::parse(current).context("invalid current result")?;
                let gate = evaluate_baseline_gate(
                    baseline_case.map(|entry| entry.classification.as_str()),
                    current,
                );
                let diff = if equal {
                    None
                } else {
                    Some(compact_diff(&reference_tree, &scribium_tree))
                };
                let gate_error = match gate.status {
                    BaselineGateStatus::Accepted => None,
                    BaselineGateStatus::NewMismatch => None,
                    BaselineGateStatus::StaleException => Some(format!(
                        "stale baseline exception for {}: current result is PASS; remove the baseline entry",
                        case.id
                    )),
                    BaselineGateStatus::ClassificationChanged => Some(format!(
                        "baseline classification changed from {} to {}",
                        baseline_case
                            .map(|entry| entry.classification.as_str())
                            .unwrap_or("<missing>"),
                        current.as_str()
                    )),
                    BaselineGateStatus::InvalidBaseline => Some(format!(
                        "baseline entry must be an accepted non-PASS classification, got {}",
                        baseline_case
                            .map(|entry| entry.classification.as_str())
                            .unwrap_or("<missing>")
                    )),
                };
                CaseReport {
                    id: case.id.clone(),
                    number: case.number,
                    section: case.section.clone(),
                    markdown: case.markdown.clone(),
                    reference: Some(reference_tree),
                    scribium: Some(scribium_tree),
                    result: current.as_str().to_string(),
                    baseline_classification: baseline_case
                        .map(|entry| entry.classification.clone()),
                    baseline_reason: baseline_case.map(|entry| entry.reason.clone()),
                    new_mismatch: gate.new_mismatch,
                    improvement: gate.improvement,
                    diff,
                    error: gate_error,
                }
            }
            Err(error) => CaseReport {
                id: case.id.clone(),
                number: case.number,
                section: case.section.clone(),
                markdown: case.markdown.clone(),
                reference: None,
                scribium: None,
                result: "HARNESS_ERROR".to_string(),
                baseline_classification: baseline_case.map(|entry| entry.classification.clone()),
                baseline_reason: baseline_case.map(|entry| entry.reason.clone()),
                new_mismatch: true,
                improvement: false,
                diff: None,
                error: Some(error.to_string()),
            },
        };

        if report.result == "PASS" {
            summary.pass += 1;
        } else if report.result == "KNOWN_MISMATCH" {
            summary.known_mismatch += 1;
        } else if report.result == "UNSUPPORTED" {
            summary.unsupported += 1;
        } else {
            summary.harness_error += 1;
        }
        if report.new_mismatch {
            summary.new_mismatch += 1;
        }
        if report.improvement {
            summary.improvements += 1;
        }

        if report.result != "PASS" && report.result != "HARNESS_ERROR" {
            if let Some(diff) = &report.diff {
                let diff_path = output_dir
                    .join("failed-case-diffs")
                    .join(format!("{}.diff", report.id));
                fs::write(
                    &diff_path,
                    format!(
                        "{} {} — {}\n\n{}\n\nMarkdown:\n{}\n",
                        name, report.id, report.section, diff, report.markdown
                    ),
                )?;
            }
        }
        cases.push(report);
    }

    Ok(SuiteReport {
        name: name.to_string(),
        reference_version: reference.version.clone(),
        reference_revision: reference.revision.clone(),
        summary,
        cases,
        baseline_errors,
    })
}

type Comparison = (CanonicalNode, CanonicalNode, bool);

fn compare_case(case: &CorpusCase, cmark: &Path, gfm_extensions: bool) -> Result<Comparison> {
    let xml = run_cmark(cmark, &case.markdown, gfm_extensions)?;
    let reference = canonicalize_xml(&xml)?;
    let profile = if gfm_extensions {
        MarkdownProfile::Gfm
    } else {
        MarkdownProfile::CommonMark
    };
    let parsed = parse_with_markdown_profile(&case.markdown, profile);
    let scribium = canonicalize_document(&parsed.document);
    let unsupported = !parsed.diagnostics.is_empty() || contains_unsupported(&scribium);
    Ok((reference, scribium, unsupported))
}

fn run_cmark(path: &Path, source: &str, gfm_extensions: bool) -> Result<String> {
    let mut command = Command::new(path);
    command.arg("--to").arg("xml");
    if gfm_extensions {
        command.arg("--full-info-string");
        for extension in ["table", "strikethrough", "tasklist", "autolink"] {
            command.arg("--extension").arg(extension);
        }
    }
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .context("failed to start reference parser")?;
    child
        .stdin
        .as_mut()
        .context("reference parser stdin was not available")?
        .write_all(source.as_bytes())?;
    let output = child.wait_with_output()?;
    if !output.status.success() {
        bail!(
            "reference parser failed with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    String::from_utf8(output.stdout).context("reference parser emitted non-UTF-8 XML")
}

#[derive(Debug)]
struct XmlNode {
    name: String,
    attrs: BTreeMap<String, String>,
    children: Vec<XmlNode>,
    text: String,
}

fn canonicalize_xml(xml: &str) -> Result<CanonicalNode> {
    let root = parse_xml(xml)?;
    if root.name != "document" {
        bail!("cmark XML root was {}, expected document", root.name);
    }
    Ok(CanonicalNode::new("document").children(
        root.children
            .iter()
            .map(xml_block)
            .collect::<Result<Vec<_>>>()?,
    ))
}

fn parse_xml(xml: &str) -> Result<XmlNode> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut stack: Vec<XmlNode> = Vec::new();
    let mut root = None;
    loop {
        match reader.read_event()? {
            Event::Start(event) => {
                stack.push(XmlNode {
                    name: String::from_utf8(event.local_name().as_ref().to_vec())?,
                    attrs: read_attrs(&event)?,
                    children: Vec::new(),
                    text: String::new(),
                });
            }
            Event::Empty(event) => {
                let node = XmlNode {
                    name: String::from_utf8(event.local_name().as_ref().to_vec())?,
                    attrs: read_attrs(&event)?,
                    children: Vec::new(),
                    text: String::new(),
                };
                append_xml_node(&mut stack, &mut root, node)?;
            }
            Event::Text(event) => {
                if let Some(node) = stack.last_mut() {
                    let decoded = event.decode()?;
                    node.text.push_str(&quick_xml::escape::unescape(&decoded)?);
                }
            }
            Event::CData(event) => {
                if let Some(node) = stack.last_mut() {
                    node.text.push_str(&String::from_utf8_lossy(event.as_ref()));
                }
            }
            Event::GeneralRef(event) => {
                if let Some(node) = stack.last_mut() {
                    let reference = format!("&{};", event.decode()?);
                    node.text
                        .push_str(&quick_xml::escape::unescape(&reference)?);
                }
            }
            Event::End(_) => {
                let node = stack.pop().context("cmark XML closed an empty stack")?;
                append_xml_node(&mut stack, &mut root, node)?;
            }
            Event::Eof => break,
            Event::Decl(_) | Event::DocType(_) | Event::Comment(_) | Event::PI(_) => {}
        }
    }
    if !stack.is_empty() {
        bail!("cmark XML ended with unclosed elements");
    }
    root.context("cmark XML did not contain a document")
}

fn read_attrs(event: &quick_xml::events::BytesStart<'_>) -> Result<BTreeMap<String, String>> {
    let mut attrs = BTreeMap::new();
    for attr in event.attributes() {
        let attr = attr?;
        let key = String::from_utf8(attr.key.local_name().as_ref().to_vec())?;
        let raw_value = String::from_utf8(attr.value.to_vec())?;
        let value = quick_xml::escape::unescape(&raw_value)?.into_owned();
        attrs.insert(key, value);
    }
    Ok(attrs)
}

fn append_xml_node(stack: &mut [XmlNode], root: &mut Option<XmlNode>, node: XmlNode) -> Result<()> {
    if let Some(parent) = stack.last_mut() {
        parent.children.push(node);
    } else if root.is_some() {
        bail!("cmark XML contained multiple roots");
    } else {
        *root = Some(node);
    }
    Ok(())
}

fn xml_attr<'a>(node: &'a XmlNode, name: &str) -> Option<&'a str> {
    node.attrs.get(name).map(String::as_str)
}

fn xml_block(node: &XmlNode) -> Result<CanonicalNode> {
    let mapped = match node.name.as_str() {
        "heading" => CanonicalNode::new("heading")
            .attr("level", xml_attr(node, "level").unwrap_or("1"))
            .children(xml_inlines(&node.children)?),
        "paragraph" => CanonicalNode::new("paragraph").children(xml_inlines(&node.children)?),
        "block_quote" => CanonicalNode::new("blockquote").children(
            node.children
                .iter()
                .map(xml_block)
                .collect::<Result<Vec<_>>>()?,
        ),
        "list" => {
            let ordered = xml_attr(node, "type") == Some("ordered");
            let mut list = CanonicalNode::new("list")
                .attr("ordered", ordered.to_string())
                .attr("start", xml_attr(node, "start").unwrap_or("1"));
            list.children = node
                .children
                .iter()
                .map(|child| {
                    if child.name == "tasklist" {
                        Ok(CanonicalNode::new("list_item")
                            .attr(
                                "task",
                                if xml_attr(child, "completed") == Some("true") {
                                    "completed"
                                } else {
                                    "active"
                                },
                            )
                            .children(
                                child
                                    .children
                                    .iter()
                                    .map(xml_block)
                                    .collect::<Result<Vec<_>>>()?,
                            ))
                    } else {
                        xml_block(child)
                    }
                })
                .collect::<Result<Vec<_>>>()?;
            list
        }
        "item" => CanonicalNode::new("list_item").children(
            node.children
                .iter()
                .map(xml_block)
                .collect::<Result<Vec<_>>>()?,
        ),
        "table" => CanonicalNode::new("table").children(
            node.children
                .iter()
                .map(xml_block)
                .collect::<Result<Vec<_>>>()?,
        ),
        "table_header" => CanonicalNode::new("table_header").children(
            node.children
                .iter()
                .map(xml_table_cell)
                .collect::<Result<Vec<_>>>()?,
        ),
        "table_row" => CanonicalNode::new("table_row").children(
            node.children
                .iter()
                .map(xml_table_cell)
                .collect::<Result<Vec<_>>>()?,
        ),
        "code_block" => CanonicalNode::value("code_block", node.text.clone())
            .attr("info", xml_attr(node, "info").unwrap_or("").to_string()),
        "thematic_break" => CanonicalNode::new("thematic_break"),
        "html_block" => {
            CanonicalNode::value("raw_html", node.text.clone()).attr("context", "block")
        }
        other => CanonicalNode::new("unsupported").attr("name", other),
    };
    Ok(mapped)
}

fn xml_table_cell(node: &XmlNode) -> Result<CanonicalNode> {
    Ok(CanonicalNode::new("table_cell")
        .attr("align", xml_attr(node, "align").unwrap_or("none"))
        .children(xml_inlines(&node.children)?))
}

fn xml_inlines(nodes: &[XmlNode]) -> Result<Vec<CanonicalNode>> {
    nodes.iter().try_fold(Vec::new(), |mut result, node| {
        result.extend(xml_inline(node)?);
        Ok(result)
    })
}

fn xml_inline(node: &XmlNode) -> Result<Vec<CanonicalNode>> {
    let mapped = match node.name.as_str() {
        "text" => return Ok(xml_text_nodes(&node.text)),
        "emph" => CanonicalNode::new("emphasis").children(xml_inlines(&node.children)?),
        "strong" => CanonicalNode::new("strong").children(xml_inlines(&node.children)?),
        "strikethrough" => {
            CanonicalNode::new("strikethrough").children(xml_inlines(&node.children)?)
        }
        "code" => CanonicalNode::value("inline_code", node.text.clone()),
        "link" => link_node(node, "link")?,
        "image" => link_node(node, "image")?,
        "softbreak" => CanonicalNode::new("soft_break"),
        "linebreak" => CanonicalNode::new("hard_break"),
        "html_inline" => {
            CanonicalNode::value("raw_html", node.text.clone()).attr("context", "inline")
        }
        other => CanonicalNode::new("unsupported").attr("name", other),
    };
    Ok(vec![mapped])
}

fn xml_text_nodes(value: &str) -> Vec<CanonicalNode> {
    let parts: Vec<_> = value.split('\n').collect();
    let mut nodes = Vec::new();
    for (index, part) in parts.iter().enumerate() {
        if !part.is_empty() {
            nodes.push(CanonicalNode::value("text", *part));
        }
        if index + 1 < parts.len() {
            nodes.push(CanonicalNode::new("soft_break"));
        }
    }
    nodes
}

fn link_node(node: &XmlNode, kind: &str) -> Result<CanonicalNode> {
    let mut mapped = CanonicalNode::new(kind)
        .attr("destination", xml_attr(node, "destination").unwrap_or(""))
        .children(xml_inlines(&node.children)?);
    if let Some(title) = xml_attr(node, "title").filter(|title| !title.is_empty()) {
        mapped.attrs.insert("title".to_string(), title.to_string());
    }
    Ok(mapped)
}

fn canonicalize_document(document: &Document) -> CanonicalNode {
    CanonicalNode::new("document")
        .children(document.nodes.iter().filter_map(canonical_block).collect())
}

fn canonical_block(block: &Block) -> Option<CanonicalNode> {
    let node = match block {
        Block::Heading { level, content, .. } => CanonicalNode::new("heading")
            .attr("level", level.to_string())
            .children(canonical_inlines(content)),
        Block::Paragraph { content, .. } => {
            CanonicalNode::new("paragraph").children(canonical_inlines(content))
        }
        Block::Blockquote { content, .. } => CanonicalNode::new("blockquote")
            .children(content.iter().filter_map(canonical_block).collect()),
        Block::UnorderedList { items, .. } => canonical_list(false, 1, items),
        Block::OrderedList { items, start, .. } => canonical_list(true, *start, items),
        Block::Table { header, rows, .. } => {
            let mut children = vec![canonical_table_row(header, true)];
            children.extend(rows.iter().map(|row| canonical_table_row(row, false)));
            CanonicalNode::new("table").children(children)
        }
        Block::CodeBlock { info, source, .. } => CanonicalNode::value("code_block", source.clone())
            .attr("info", info.clone().unwrap_or_default()),
        Block::ThematicBreak { .. } => CanonicalNode::new("thematic_break"),
        Block::RawHtml { source, .. } => {
            CanonicalNode::value("raw_html", source.clone()).attr("context", "block")
        }
        Block::Unsupported { kind, .. } => CanonicalNode::new("unsupported").attr("name", kind),
        Block::DirectiveCall { .. } => {
            CanonicalNode::new("unsupported").attr("name", "directive_call")
        }
        Block::Metadata { .. } => return None,
    };
    Some(node)
}

fn canonical_list(ordered: bool, start: usize, items: &[ListItem]) -> CanonicalNode {
    CanonicalNode::new("list")
        .attr("ordered", ordered.to_string())
        .attr("start", start.to_string())
        .children(items.iter().map(canonical_list_item).collect())
}

fn canonical_list_item(item: &ListItem) -> CanonicalNode {
    let mut node = CanonicalNode::new("list_item");
    if let Some(task) = item.task {
        node.attrs.insert(
            "task".to_string(),
            match task {
                scribium_markdown::ast::TaskStatus::Active => "active",
                scribium_markdown::ast::TaskStatus::Completed => "completed",
            }
            .to_string(),
        );
    }
    node.children = item.content.iter().filter_map(canonical_block).collect();
    node
}

fn canonical_table_row(row: &TableRow, header: bool) -> CanonicalNode {
    CanonicalNode::new(if header { "table_header" } else { "table_row" }).children(
        row.cells
            .iter()
            .map(|cell| {
                let alignment = if header {
                    table_alignment(cell.alignment)
                } else {
                    "none".to_string()
                };
                CanonicalNode::new("table_cell")
                    .attr("align", alignment)
                    .children(canonical_inlines(&cell.content))
            })
            .collect(),
    )
}

fn table_alignment(alignment: TableAlignment) -> String {
    match alignment {
        TableAlignment::Left => "left",
        TableAlignment::Center => "center",
        TableAlignment::Right => "right",
        TableAlignment::None => "none",
    }
    .to_string()
}

fn canonical_inlines(inlines: &[Inline]) -> Vec<CanonicalNode> {
    inlines.iter().filter_map(canonical_inline).collect()
}

fn canonical_inline(inline: &Inline) -> Option<CanonicalNode> {
    let node = match inline {
        Inline::Text { content, .. } => CanonicalNode::value("text", content.clone()),
        Inline::Emphasis { content, .. } => {
            CanonicalNode::new("emphasis").children(canonical_inlines(content))
        }
        Inline::Strong { content, .. } => {
            CanonicalNode::new("strong").children(canonical_inlines(content))
        }
        Inline::Strikethrough { content, .. } => {
            CanonicalNode::new("strikethrough").children(canonical_inlines(content))
        }
        Inline::Link {
            content,
            destination,
            title,
            ..
        } => canonical_link("link", content, destination, title.as_deref()),
        Inline::Image {
            content,
            destination,
            title,
            ..
        } => canonical_link("image", content, destination, title.as_deref()),
        Inline::Code { content, .. } => CanonicalNode::value("inline_code", content.clone()),
        Inline::RawHtml { content, .. } => {
            CanonicalNode::value("raw_html", content.clone()).attr("context", "inline")
        }
        Inline::HardBreak { .. } => CanonicalNode::new("hard_break"),
        Inline::SoftBreak { .. } => CanonicalNode::new("soft_break"),
        Inline::Unsupported { kind, .. } => CanonicalNode::new("unsupported").attr("name", kind),
        Inline::DirectiveCall { .. } => {
            CanonicalNode::new("unsupported").attr("name", "directive_call")
        }
    };
    Some(node)
}

fn canonical_link(
    kind: &str,
    content: &[Inline],
    destination: &str,
    title: Option<&str>,
) -> CanonicalNode {
    let mut node = CanonicalNode::new(kind)
        .attr("destination", destination)
        .children(canonical_inlines(content));
    if let Some(title) = title {
        node.attrs.insert("title".to_string(), title.to_string());
    }
    node
}

fn contains_unsupported(node: &CanonicalNode) -> bool {
    node.kind == "unsupported" || node.children.iter().any(contains_unsupported)
}

fn compact_diff(reference: &CanonicalNode, actual: &CanonicalNode) -> String {
    let reference = serde_json::to_value(reference).unwrap_or_default();
    let actual = serde_json::to_value(actual).unwrap_or_default();
    first_json_diff(&reference, &actual, "$").unwrap_or_else(|| "trees differ".to_string())
}

fn first_json_diff(
    reference: &serde_json::Value,
    actual: &serde_json::Value,
    path: &str,
) -> Option<String> {
    match (reference, actual) {
        (serde_json::Value::Object(left), serde_json::Value::Object(right)) => {
            let keys: BTreeSet<_> = left.keys().chain(right.keys()).collect();
            for key in keys {
                let child_path = format!("{path}.{key}");
                match (left.get(key), right.get(key)) {
                    (Some(left), Some(right)) => {
                        if let Some(diff) = first_json_diff(left, right, &child_path) {
                            return Some(diff);
                        }
                    }
                    (left, right) => {
                        return Some(format!(
                            "{child_path}: reference={} scribium={}",
                            short_json(left),
                            short_json(right)
                        ));
                    }
                }
            }
            None
        }
        (serde_json::Value::Array(left), serde_json::Value::Array(right)) => {
            if left.len() != right.len() {
                return Some(format!(
                    "{path}.length: reference={} scribium={}",
                    left.len(),
                    right.len()
                ));
            }
            for (index, (left, right)) in left.iter().zip(right).enumerate() {
                if let Some(diff) = first_json_diff(left, right, &format!("{path}[{index}]")) {
                    return Some(diff);
                }
            }
            None
        }
        (left, right) if left != right => Some(format!(
            "{path}: reference={} scribium={}",
            short_json(Some(left)),
            short_json(Some(right))
        )),
        _ => None,
    }
}

fn short_json(value: Option<&serde_json::Value>) -> String {
    let mut text = value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "<missing>".to_string());
    if text.len() > 240 {
        text.truncate(237);
        text.push_str("...");
    }
    text
}

#[cfg(test)]
mod profile_tests {
    use super::*;

    #[test]
    fn commonmark_profile_does_not_enable_gfm_linkify() {
        let source = "https://example.com\n";
        let commonmark = parse_with_markdown_profile(source, MarkdownProfile::CommonMark);
        let gfm = parse_with_markdown_profile(source, MarkdownProfile::Gfm);

        let commonmark_tree = canonicalize_document(&commonmark.document);
        let gfm_tree = canonicalize_document(&gfm.document);
        assert_eq!(commonmark_tree.children[0].children[0].kind, "text");
        assert_eq!(gfm_tree.children[0].children[0].kind, "link");
    }

    #[test]
    fn table_body_alignment_is_a_reference_projection_detail() {
        let source = "| abc | defghi |\n:-: | -----------:\nbar | baz\n";
        let parsed = parse_with_markdown_profile(source, MarkdownProfile::Gfm);
        let tree = canonicalize_document(&parsed.document);
        let body = &tree.children[0].children[1];
        assert_eq!(body.children[0].attrs["align"], "none");
        assert_eq!(body.children[1].attrs["align"], "none");
    }
}

fn run_real_documents(args: &Args) -> Result<RealReport> {
    let manifest: RealManifest = read_json(&args.real_manifest)?;
    if manifest.schema_version != 1 {
        bail!("unsupported real corpus schema {}", manifest.schema_version);
    }
    let backend = SubprocessBackend::new(&args.typst);
    let mut documents = Vec::with_capacity(manifest.documents.len());
    let mut summary = RealSummary {
        total: manifest.documents.len(),
        successful_pdf: 0,
        expected_unsupported: 0,
        harness_error: 0,
    };

    for spec in manifest.documents {
        let source_path = args.real_root.join(&spec.path);
        let source = match fs::read_to_string(&source_path) {
            Ok(source) => source,
            Err(error) => {
                summary.harness_error += 1;
                documents.push(RealDocumentReport {
                    id: spec.id,
                    path: spec.path,
                    expected: spec.expected,
                    result: "HARNESS_ERROR".to_string(),
                    html_policy: spec.html_policy,
                    diagnostics: Vec::new(),
                    typst_path: None,
                    pdf_path: None,
                    missing_markers: Vec::new(),
                    error: Some(error.to_string()),
                });
                continue;
            }
        };

        let entry = spec.path.clone();
        let project = VirtualProjectBuilder::new()
            .entry(&entry)
            .with_context(|| format!("invalid real corpus entry {}", spec.path))?
            .add_source(&entry, &source)
            .with_context(|| format!("invalid real corpus source {}", spec.path))?
            .build()
            .with_context(|| format!("cannot build real corpus project {}", spec.path))?;
        let result = compile(&project, &CompileOptions::default());
        let diagnostics: Vec<String> = result
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.clone())
            .collect();

        if spec.expected == "unsupported" {
            let missing_diagnostics: Vec<_> = spec
                .diagnostics
                .iter()
                .filter(|expected| !diagnostics.iter().any(|actual| actual == *expected))
                .cloned()
                .collect();
            if missing_diagnostics.is_empty() {
                summary.expected_unsupported += 1;
                documents.push(RealDocumentReport {
                    id: spec.id,
                    path: spec.path,
                    expected: spec.expected,
                    result: "EXPECTED_UNSUPPORTED".to_string(),
                    html_policy: spec.html_policy,
                    diagnostics,
                    typst_path: None,
                    pdf_path: None,
                    missing_markers: Vec::new(),
                    error: None,
                });
            } else {
                summary.harness_error += 1;
                documents.push(RealDocumentReport {
                    id: spec.id,
                    path: spec.path,
                    expected: spec.expected,
                    result: "HARNESS_ERROR".to_string(),
                    html_policy: spec.html_policy,
                    diagnostics,
                    typst_path: None,
                    pdf_path: None,
                    missing_markers: Vec::new(),
                    error: Some(format!(
                        "missing expected diagnostics: {}",
                        missing_diagnostics.join(", ")
                    )),
                });
            }
            continue;
        }

        if spec.expected != "pdf" {
            bail!(
                "real document {} has unknown expected result {}",
                spec.id,
                spec.expected
            );
        }
        if !diagnostics.is_empty() {
            summary.harness_error += 1;
            documents.push(RealDocumentReport {
                id: spec.id,
                path: spec.path,
                expected: spec.expected,
                result: "HARNESS_ERROR".to_string(),
                html_policy: spec.html_policy,
                diagnostics,
                typst_path: None,
                pdf_path: None,
                missing_markers: Vec::new(),
                error: Some("supported document produced diagnostics".to_string()),
            });
            continue;
        }

        let typst = scribium_typst::lowering::lower_to_typst_code(&result.ir);
        let missing_markers: Vec<String> = spec
            .markers
            .iter()
            .filter(|marker| !typst.contains(marker.as_str()))
            .cloned()
            .collect();
        let typst_path = args
            .output_dir
            .join("real-typst")
            .join(format!("{}.typ", spec.id));
        fs::write(&typst_path, &typst)?;
        if !missing_markers.is_empty() || typst.is_empty() {
            summary.harness_error += 1;
            documents.push(RealDocumentReport {
                id: spec.id,
                path: spec.path,
                expected: spec.expected,
                result: "HARNESS_ERROR".to_string(),
                html_policy: spec.html_policy,
                diagnostics,
                typst_path: Some(typst_path.display().to_string()),
                pdf_path: None,
                missing_markers,
                error: Some("generated Typst was empty or missed markers".to_string()),
            });
            continue;
        }

        match backend.compile(&TypstInput {
            source: typst,
            entry_path: spec.path.clone(),
        }) {
            Ok(output) => {
                let pdf = output.pdf.context("Typst backend returned no PDF")?;
                if pdf.is_empty() || !pdf.starts_with(b"%PDF-") {
                    summary.harness_error += 1;
                    documents.push(RealDocumentReport {
                        id: spec.id,
                        path: spec.path,
                        expected: spec.expected,
                        result: "HARNESS_ERROR".to_string(),
                        html_policy: spec.html_policy,
                        diagnostics,
                        typst_path: Some(typst_path.display().to_string()),
                        pdf_path: None,
                        missing_markers,
                        error: Some("Typst produced an empty or invalid PDF".to_string()),
                    });
                } else {
                    let pdf_path = args
                        .output_dir
                        .join("real-pdf")
                        .join(format!("{}.pdf", spec.id));
                    fs::write(&pdf_path, pdf)?;
                    summary.successful_pdf += 1;
                    documents.push(RealDocumentReport {
                        id: spec.id,
                        path: spec.path,
                        expected: spec.expected,
                        result: "PASS".to_string(),
                        html_policy: spec.html_policy,
                        diagnostics,
                        typst_path: Some(typst_path.display().to_string()),
                        pdf_path: Some(pdf_path.display().to_string()),
                        missing_markers,
                        error: None,
                    });
                }
            }
            Err(error) => {
                summary.harness_error += 1;
                documents.push(RealDocumentReport {
                    id: spec.id,
                    path: spec.path,
                    expected: spec.expected,
                    result: "HARNESS_ERROR".to_string(),
                    html_policy: spec.html_policy,
                    diagnostics,
                    typst_path: Some(typst_path.display().to_string()),
                    pdf_path: None,
                    missing_markers,
                    error: Some(error.to_string()),
                });
            }
        }
    }

    Ok(RealReport { summary, documents })
}

fn write_report(output_dir: &Path, report: &FullReport) -> Result<()> {
    let json = serde_json::to_string_pretty(report)? + "\n";
    fs::write(output_dir.join("compatibility-report.json"), json)?;
    fs::write(
        output_dir.join("compatibility-report.md"),
        markdown_report(report),
    )?;
    Ok(())
}

fn markdown_report(report: &FullReport) -> String {
    let mut output = String::new();
    output.push_str("# Markdown compatibility report\n\n");
    for suite in &report.suites {
        output.push_str(&format!(
            "## {}\n\nReference: `{}` at `{}`\n\n",
            suite.name, suite.reference_version, suite.reference_revision
        ));
        output.push_str(&format_suite_summary(&suite.summary));
        output.push('\n');
        for case in &suite.cases {
            if case.result != "PASS" || case.improvement {
                output.push_str(&format!(
                    "- `{}` — {} — {}{}\n",
                    case.id,
                    case.section,
                    case.result,
                    if case.improvement {
                        " (improvement)"
                    } else {
                        ""
                    }
                ));
                if let Some(diff) = &case.diff {
                    output.push_str(&format!("  - diff: `{}`\n", diff));
                }
            }
        }
        output.push('\n');
    }
    output.push_str("## Real documents\n\n");
    output.push_str(&format_real_summary(&report.real_documents.summary));
    output.push('\n');
    for document in &report.real_documents.documents {
        output.push_str(&format!(
            "- `{}` (`{}`) — {}{}\n",
            document.id,
            document.path,
            document.result,
            document
                .html_policy
                .as_deref()
                .map(|policy| format!("; HTML policy: {policy}"))
                .unwrap_or_default()
        ));
        if !document.diagnostics.is_empty() {
            output.push_str(&format!(
                "  - diagnostics: `{}`\n",
                document.diagnostics.join(", ")
            ));
        }
        if let Some(error) = &document.error {
            output.push_str(&format!("  - error: {error}\n"));
        }
    }
    if !report.errors.is_empty() {
        output.push_str("\n## Failures\n\n");
        for error in &report.errors {
            output.push_str(&format!("- {error}\n"));
        }
    }
    output
}

fn format_suite_summary(summary: &SuiteSummary) -> String {
    format!(
        "- total: {}\n- pass: {}\n- known mismatch: {}\n- unsupported: {}\n- harness error: {}\n- new mismatch: {}\n- improvement: {}\n",
        summary.total,
        summary.pass,
        summary.known_mismatch,
        summary.unsupported,
        summary.harness_error,
        summary.new_mismatch,
        summary.improvements
    )
}

fn format_real_summary(summary: &RealSummary) -> String {
    format!(
        "- total: {}\n- successful PDF: {}\n- expected unsupported: {}\n- harness error: {}\n",
        summary.total, summary.successful_pdf, summary.expected_unsupported, summary.harness_error
    )
}

fn print_summary(report: &FullReport) {
    for suite in &report.suites {
        println!(
            "{}: total={} pass={} known_mismatch={} unsupported={} new_mismatch={}",
            suite.name,
            suite.summary.total,
            suite.summary.pass,
            suite.summary.known_mismatch,
            suite.summary.unsupported,
            suite.summary.new_mismatch
        );
        for case in &suite.cases {
            if case.new_mismatch || case.result == "HARNESS_ERROR" {
                eprintln!(
                    "{} {} (example {}, {}): {}",
                    suite.name,
                    case.id,
                    case.number,
                    case.section,
                    case.error
                        .as_deref()
                        .or(case.diff.as_deref())
                        .unwrap_or("compatibility failure")
                );
            }
        }
    }
    println!(
        "Real documents: total={} successful_pdf={} expected_unsupported={} harness_error={}",
        report.real_documents.summary.total,
        report.real_documents.summary.successful_pdf,
        report.real_documents.summary.expected_unsupported,
        report.real_documents.summary.harness_error
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pass_without_baseline_is_ok() {
        let decision = evaluate_baseline_gate(None, Classification::Pass);
        assert_eq!(decision.status, BaselineGateStatus::Accepted);
        assert!(!decision.new_mismatch);
        assert!(!decision.improvement);
    }

    #[test]
    fn new_mismatch_without_baseline_fails() {
        let decision = evaluate_baseline_gate(None, Classification::KnownMismatch);
        assert_eq!(decision.status, BaselineGateStatus::NewMismatch);
        assert!(decision.new_mismatch);
        assert!(!decision.improvement);
    }

    #[test]
    fn existing_known_mismatch_is_accepted() {
        let decision =
            evaluate_baseline_gate(Some("KNOWN_MISMATCH"), Classification::KnownMismatch);
        assert_eq!(decision.status, BaselineGateStatus::Accepted);
        assert!(!decision.new_mismatch);
        assert!(!decision.improvement);
    }

    #[test]
    fn resolved_known_mismatch_is_detected_as_stale() {
        let decision = evaluate_baseline_gate(Some("KNOWN_MISMATCH"), Classification::Pass);
        assert_eq!(decision.status, BaselineGateStatus::StaleException);
        assert!(!decision.new_mismatch);
        assert!(decision.improvement);
    }

    #[test]
    fn resolved_case_then_regressed_is_rejected() {
        let resolved = evaluate_baseline_gate(None, Classification::Pass);
        let regressed = evaluate_baseline_gate(None, Classification::KnownMismatch);
        assert_eq!(resolved.status, BaselineGateStatus::Accepted);
        assert_eq!(regressed.status, BaselineGateStatus::NewMismatch);
        assert!(regressed.new_mismatch);
    }

    #[test]
    fn mismatch_classification_change_is_rejected() {
        let known_to_unsupported =
            evaluate_baseline_gate(Some("KNOWN_MISMATCH"), Classification::Unsupported);
        let unsupported_to_known =
            evaluate_baseline_gate(Some("UNSUPPORTED"), Classification::KnownMismatch);
        assert_eq!(
            known_to_unsupported.status,
            BaselineGateStatus::ClassificationChanged
        );
        assert_eq!(
            unsupported_to_known.status,
            BaselineGateStatus::ClassificationChanged
        );
        assert!(known_to_unsupported.new_mismatch);
        assert!(unsupported_to_known.new_mismatch);
    }

    #[test]
    fn removed_corpus_case_in_baseline_is_rejected() {
        let baseline_ids = ["present".to_string(), "removed".to_string()];
        let corpus_ids = BTreeSet::from(["present".to_string()]);
        assert_eq!(
            missing_baseline_case_ids(baseline_ids.iter(), &corpus_ids),
            vec!["removed".to_string()]
        );
    }
}
