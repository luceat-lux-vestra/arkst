from pathlib import Path


def replace_once(path, old, new):
    p = Path(path)
    s = p.read_text()
    n = s.count(old)
    if n != 1:
        raise SystemExit(f"{path}: expected 1 occurrence, got {n}: {old[:80]!r}")
    p.write_text(s.replace(old, new, 1))

backend = Path("crates/arkst-typst/src/backend.rs")
backend.write_text(r'''//! Platform-neutral contract for Typst compiler adapters.

use std::time::Duration;

/// Output target requested from an official Typst compiler backend.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TypstTarget {
    Pdf,
    Png,
    Svg,
    Html,
}

impl TypstTarget {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Png => "png",
            Self::Svg => "svg",
            Self::Html => "html",
        }
    }

    pub const fn extension(self) -> &'static str {
        self.as_str()
    }

    pub const fn is_experimental(self) -> bool {
        matches!(self, Self::Html)
    }
}

impl std::fmt::Display for TypstTarget {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// One compiler artifact returned by a Typst backend.
///
/// `path` is a deterministic backend-relative logical artifact path. It is
/// never interpreted as host filesystem authority by this platform-neutral
/// contract. Keeping a logical path lets future multi-file targets (for
/// example Typst bundle export) fit without redesigning the result shape.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypstArtifact {
    pub path: String,
    pub bytes: Vec<u8>,
}

impl TypstArtifact {
    pub fn new(path: impl Into<String>, bytes: Vec<u8>) -> Self {
        Self {
            path: path.into(),
            bytes,
        }
    }
}

/// Abstract interface for a Typst compiler backend.
///
/// Concrete execution errors remain owned by the adapter that produces them;
/// this contract therefore exposes no filesystem, process, or host-path type.
pub trait TypstBackend {
    type Error: std::error::Error;

    /// Return whether this backend currently supports `target`.
    fn supports_target(&self, target: TypstTarget) -> bool;

    /// Compile a Typst source document for one explicit target.
    fn compile(&self, input: &TypstInput, target: TypstTarget) -> Result<TypstOutput, Self::Error>;

    /// Return the Typst compiler version.
    fn version(&self) -> Result<String, Self::Error>;
}

/// Input to a Typst compiler adapter.
pub struct TypstInput {
    pub source: String,
    /// Project-root-relative logical path of the Arkst source entry.
    ///
    /// A native adapter validates this value before interpreting it in a host
    /// project context. Lowering itself treats it as opaque contract data.
    pub entry_path: String,
}

/// Output from one explicit Typst compiler target.
#[derive(Debug)]
pub struct TypstOutput {
    pub target: TypstTarget,
    pub artifacts: Vec<TypstArtifact>,
    pub diagnostics: Vec<String>,
    pub duration: Duration,
}

impl TypstOutput {
    pub fn single_artifact(&self) -> Option<&TypstArtifact> {
        match self.artifacts.as_slice() {
            [artifact] => Some(artifact),
            _ => None,
        }
    }

    pub fn into_single_artifact(self) -> Option<TypstArtifact> {
        if self.artifacts.len() == 1 {
            self.artifacts.into_iter().next()
        } else {
            None
        }
    }
}
''')

replace_once(
    "crates/arkst-typst/src/lib.rs",
    "pub use backend::{TypstBackend, TypstInput, TypstOutput};",
    "pub use backend::{TypstArtifact, TypstBackend, TypstInput, TypstOutput, TypstTarget};",
)

sub = Path("crates/arkst-typst-subprocess/src/lib.rs")
s = sub.read_text()
s = s.replace(
    "use arkst_typst::{TypstBackend, TypstInput, TypstOutput};",
    "use arkst_typst::{TypstArtifact, TypstBackend, TypstInput, TypstOutput, TypstTarget};",
    1,
)
s = s.replace(
    "    ResourceBoundaryViolation(String),\n}",
    "    ResourceBoundaryViolation(String),\n    UnsupportedTarget(TypstTarget),\n}",
    1,
)
s = s.replace(
    "            TypstError::ResourceBoundaryViolation(path) => write!(\n                f,\n                \"Typst resource path leaves the project boundary: {}\",\n                path\n            ),\n",
    "            TypstError::ResourceBoundaryViolation(path) => write!(\n                f,\n                \"Typst resource path leaves the project boundary: {}\",\n                path\n            ),\n            TypstError::UnsupportedTarget(target) => {\n                write!(f, \"Typst target '{target}' is not enabled by this backend\")\n            }\n",
    1,
)
s = s.replace(
    "    pub source_context: Option<TypstSourceContext>,\n}",
    "    pub source_context: Option<TypstSourceContext>,\n    experimental_html: bool,\n}",
    1,
)
s = s.replace(
    "            source_context: None,\n        }",
    "            source_context: None,\n            experimental_html: false,\n        }",
    1,
)
s = s.replace(
    "    pub fn with_source_context(mut self, source_context: TypstSourceContext) -> Self {\n        self.source_context = Some(source_context);\n        self\n    }\n}",
    "    pub fn with_source_context(mut self, source_context: TypstSourceContext) -> Self {\n        self.source_context = Some(source_context);\n        self\n    }\n\n    /// Enables transport-level access to Typst's experimental HTML target.\n    /// This does not claim Arkst HTML semantic parity; #320 owns that gate.\n    pub fn with_experimental_html(mut self, enabled: bool) -> Self {\n        self.experimental_html = enabled;\n        self\n    }\n}\n",
    1,
)
start = s.index("impl TypstBackend for SubprocessBackend {")
end = s.index("fn validate_entry_path", start)
new_impl = r'''impl TypstBackend for SubprocessBackend {
    type Error = TypstError;

    fn supports_target(&self, target: TypstTarget) -> bool {
        match target {
            TypstTarget::Pdf | TypstTarget::Png | TypstTarget::Svg => true,
            TypstTarget::Html => self.experimental_html,
        }
    }

    fn compile(&self, input: &TypstInput, target: TypstTarget) -> Result<TypstOutput, TypstError> {
        if !self.supports_target(target) {
            return Err(TypstError::UnsupportedTarget(target));
        }

        let start = std::time::Instant::now();
        let entry_path = validate_entry_path(&input.entry_path)?;
        let project_root = self
            .source_context
            .as_ref()
            .map(|context| canonical_project_root(&context.project_root))
            .transpose()?;
        if let Some(project_root) = &project_root {
            validate_reachable_typst_modules(&input.source, &entry_path, project_root)?;
        } else if contains_static_typst_preflight_violation(&input.source) {
            return Err(static_module_preflight_rejected(&entry_path));
        }

        let temp_dir = tempfile::tempdir().map_err(TypstError::Io)?;
        let output_name = match target {
            TypstTarget::Pdf => "output.pdf",
            TypstTarget::Png => "output-{0p}.png",
            TypstTarget::Svg => "output-{0p}.svg",
            TypstTarget::Html => "output.html",
        };
        let output_path = temp_dir.path().join(output_name);

        let (typst_file, typst_root) = if let Some(project_root) = project_root.as_ref() {
            let mirror_root = temp_dir.path().join("project");
            let mut active_directories = BTreeSet::new();
            copy_project_tree(
                project_root,
                &mirror_root,
                project_root,
                &VirtualPathBuf::root(),
                &mut active_directories,
            )?;

            let generated_entry = generated_typst_path(&mirror_root, &entry_path)?;
            if let Some(parent) = generated_entry.parent() {
                fs::create_dir_all(parent).map_err(TypstError::Io)?;
            }
            fs::write(&generated_entry, &input.source).map_err(TypstError::Io)?;
            (generated_entry, Some(mirror_root))
        } else {
            let isolated_root = temp_dir.path().join("self-contained");
            fs::create_dir_all(&isolated_root).map_err(TypstError::Io)?;
            let typst_file = isolated_root.join("input.typ");
            fs::write(&typst_file, &input.source).map_err(TypstError::Io)?;
            (typst_file, Some(isolated_root))
        };

        let mut cmd = Command::new(&self.typst_path);
        cmd.arg("compile");
        if target == TypstTarget::Html {
            cmd.arg("--features").arg("html");
        }
        cmd.arg("--format").arg(target.as_str());
        if let Some(root) = &typst_root {
            cmd.arg("--root").arg(root);
        }
        cmd.arg(&typst_file).arg(&output_path);

        let output = cmd.output().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                TypstError::Subprocess(format!(
                    "Typst executable not found at '{}': {}",
                    self.typst_path.display(),
                    e
                ))
            } else {
                TypstError::Io(e)
            }
        })?;

        let duration = start.elapsed();
        if !output.status.success() {
            let stderr = sanitize_typst_diagnostic(&output.stderr, temp_dir.path());
            return Err(TypstError::Subprocess(format!(
                "Typst compilation failed:\n{}",
                stderr.trim()
            )));
        }

        let artifacts = collect_output_artifacts(temp_dir.path(), target)?;
        Ok(TypstOutput {
            target,
            artifacts,
            diagnostics: vec![],
            duration,
        })
    }

    fn version(&self) -> Result<String, TypstError> {
        let output = Command::new(&self.typst_path)
            .arg("--version")
            .output()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    TypstError::Subprocess(format!(
                        "Typst executable not found at '{}': {}",
                        self.typst_path.display(),
                        e
                    ))
                } else {
                    TypstError::Io(e)
                }
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(TypstError::Subprocess(format!(
                "`typst --version` failed:\n{}",
                stderr.trim()
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.trim().to_string())
    }
}

fn collect_output_artifacts(
    temp_root: &Path,
    target: TypstTarget,
) -> Result<Vec<TypstArtifact>, TypstError> {
    let mut paths = match target {
        TypstTarget::Pdf => vec![temp_root.join("output.pdf")],
        TypstTarget::Html => vec![temp_root.join("output.html")],
        TypstTarget::Png | TypstTarget::Svg => {
            let extension = target.extension();
            let mut paths = fs::read_dir(temp_root)
                .map_err(TypstError::Io)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(TypstError::Io)?
                .into_iter()
                .map(|entry| entry.path())
                .filter(|path| {
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.starts_with("output-") && name.ends_with(extension))
                })
                .collect::<Vec<_>>();
            paths.sort();
            paths
        }
    };

    paths.sort();
    if paths.is_empty() {
        return Err(TypstError::Subprocess(format!(
            "Typst produced no {target} artifacts"
        )));
    }

    let mut artifacts = Vec::with_capacity(paths.len());
    for path in paths {
        let bytes = fs::read(&path).map_err(TypstError::Io)?;
        if bytes.is_empty() {
            return Err(TypstError::Subprocess(format!(
                "Typst produced empty {target} output"
            )));
        }
        if target == TypstTarget::Pdf && !bytes.starts_with(b"%PDF-") {
            return Err(TypstError::Subprocess(
                "Typst produced invalid PDF output: missing %PDF- header".into(),
            ));
        }
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| TypstError::Subprocess("Typst produced a non-UTF-8 artifact name".into()))?;
        artifacts.push(TypstArtifact::new(name, bytes));
    }
    Ok(artifacts)
}

'''
s = s[:start] + new_impl + s[end:]
sub.write_text(s)

inp = Path("crates/arkst-typst-inprocess/src/lib.rs")
s = inp.read_text()
s = s.replace(
    "use arkst_typst::{TypstBackend, TypstInput, TypstOutput};",
    "use arkst_typst::{TypstArtifact, TypstBackend, TypstInput, TypstOutput, TypstTarget};",
    1,
)
s = s.replace(
    "        source_map: &[SourceMapEntry],\n    ) -> Result<TypstOutput, InProcessError> {\n        let start = Instant::now();",
    "        source_map: &[SourceMapEntry],\n        target: TypstTarget,\n    ) -> Result<TypstOutput, InProcessError> {\n        if target != TypstTarget::Pdf {\n            return Err(InProcessError::UnsupportedTarget(target));\n        }\n        let start = Instant::now();",
    1,
)
s = s.replace(
    "        Ok(TypstOutput {\n            pdf: Some(pdf),\n            html: None,\n            svg: None,\n            png: None,\n            diagnostics: warnings,\n            duration: start.elapsed(),\n        })",
    "        Ok(TypstOutput {\n            target,\n            artifacts: vec![TypstArtifact::new(\"output.pdf\", pdf)],\n            diagnostics: warnings,\n            duration: start.elapsed(),\n        })",
    1,
)
s = s.replace(
    "impl TypstBackend for InProcessBackend<'_> {\n    type Error = InProcessError;\n\n    fn compile(&self, input: &TypstInput) -> Result<TypstOutput, Self::Error> {\n        self.compile_with_source_map(input, &[])\n    }",
    "impl TypstBackend for InProcessBackend<'_> {\n    type Error = InProcessError;\n\n    fn supports_target(&self, target: TypstTarget) -> bool {\n        target == TypstTarget::Pdf\n    }\n\n    fn compile(&self, input: &TypstInput, target: TypstTarget) -> Result<TypstOutput, Self::Error> {\n        self.compile_with_source_map(input, &[], target)\n    }",
    1,
)
s = s.replace(
    "    CompilerPanic(String),\n}",
    "    CompilerPanic(String),\n    UnsupportedTarget(TypstTarget),\n}",
    1,
)
s = s.replace(
    "            Self::InvalidInput(_) | Self::InvalidOutput(_) | Self::CompilerPanic(_) => &[],",
    "            Self::InvalidInput(_)\n            | Self::InvalidOutput(_)\n            | Self::CompilerPanic(_)\n            | Self::UnsupportedTarget(_) => &[],",
    1,
)
s = s.replace(
    "            Self::CompilerPanic(message) => {\n                write!(formatter, \"Typst compiler panicked: {message}\")\n            }",
    "            Self::CompilerPanic(message) => {\n                write!(formatter, \"Typst compiler panicked: {message}\")\n            }\n            Self::UnsupportedTarget(target) => {\n                write!(formatter, \"in-process Typst target '{target}' is not supported\")\n            }",
    1,
)
inp.write_text(s)
