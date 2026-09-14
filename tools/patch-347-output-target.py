from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected exactly 1 match, got {count}")
    return text.replace(old, new, 1)


# subprocess backend
path = Path("crates/arkst-typst-subprocess/src/lib.rs")
text = path.read_text()
text = replace_once(
    text,
    "use arkst_typst::{TypstBackend, TypstInput, TypstOutput};",
    "use arkst_typst::{\n    TypstArtifact, TypstBackend, TypstInput, TypstOutput, TypstOutputTarget,\n};",
    "subprocess import",
)
text = replace_once(
    text,
    "pub enum TypstError {\n    Subprocess(String),",
    "pub enum TypstError {\n    Subprocess(String),\n    UnsupportedTarget(TypstOutputTarget),",
    "subprocess error variant",
)
text = replace_once(
    text,
    '            TypstError::Subprocess(msg) => write!(f, "subprocess error: {}", msg),',
    '            TypstError::Subprocess(msg) => write!(f, "subprocess error: {}", msg),\n            TypstError::UnsupportedTarget(target) => {\n                write!(f, "Typst subprocess target \'{}\' is not implemented", target)\n            }',
    "subprocess error display",
)
text = replace_once(
    text,
    "    fn compile(&self, input: &TypstInput) -> Result<TypstOutput, TypstError> {\n        let start = std::time::Instant::now();",
    "    fn compile_target(\n        &self,\n        input: &TypstInput,\n        target: TypstOutputTarget,\n    ) -> Result<TypstOutput, TypstError> {\n        if !self.supports_target(target) {\n            return Err(TypstError::UnsupportedTarget(target));\n        }\n        let start = std::time::Instant::now();",
    "subprocess compile signature",
)
text = replace_once(
    text,
    '        let pdf_file = temp_dir.path().join("output.pdf");',
    '        let output_path = target_output_path(temp_dir.path(), target)?;',
    "subprocess output path",
)
text = replace_once(
    text,
    '''        // Invoke typst compile
        let mut cmd = Command::new(&self.typst_path);
        cmd.arg("compile");
        if let Some(root) = &typst_root {
            cmd.arg("--root").arg(root);
        }
        cmd.arg(&typst_file).arg(&pdf_file);
''',
    '''        // Invoke Typst for the explicit target. HTML remains an official
        // experimental Typst target and is enabled only at this adapter boundary.
        let mut cmd = Command::new(&self.typst_path);
        cmd.arg("compile");
        if target == TypstOutputTarget::Html {
            cmd.arg("--features").arg("html");
        }
        cmd.arg("--format").arg(target.as_str());
        if let Some(root) = &typst_root {
            cmd.arg("--root").arg(root);
        }
        cmd.arg(&typst_file).arg(&output_path);
''',
    "subprocess command",
)
old = '''        // Read the generated PDF
        let pdf_bytes = fs::read(&pdf_file).map_err(TypstError::Io)?;

        if pdf_bytes.is_empty() {
            return Err(TypstError::Subprocess(
                "Typst produced empty PDF output".into(),
            ));
        }

        // A successful subprocess can still produce a corrupt or non-PDF
        // file; never treat that as success.
        if !pdf_bytes.starts_with(b"%PDF-") {
            return Err(TypstError::Subprocess(
                "Typst produced invalid PDF output: missing %PDF- header".into(),
            ));
        }

        Ok(TypstOutput {
            pdf: Some(pdf_bytes),
            html: None,
            svg: None,
            png: None,
            diagnostics: vec![],
            duration,
        })
    }

    fn version(&self) -> Result<String, TypstError> {
'''
new = '''        let artifacts = collect_target_artifacts(temp_dir.path(), target)?;
        validate_target_artifacts(target, &artifacts)?;

        Ok(TypstOutput::new(target, artifacts, vec![], duration))
    }

    fn supports_target(&self, target: TypstOutputTarget) -> bool {
        !matches!(target, TypstOutputTarget::Bundle)
    }

    fn version(&self) -> Result<String, TypstError> {
'''
text = replace_once(text, old, new, "subprocess output collection")
helpers = r'''
fn target_output_path(
    temp_root: &Path,
    target: TypstOutputTarget,
) -> Result<PathBuf, TypstError> {
    match target {
        TypstOutputTarget::Pdf => Ok(temp_root.join("output.pdf")),
        TypstOutputTarget::Html => Ok(temp_root.join("output.html")),
        TypstOutputTarget::Png => Ok(temp_root.join("output-{0p}.png")),
        TypstOutputTarget::Svg => Ok(temp_root.join("output-{0p}.svg")),
        TypstOutputTarget::Bundle => Err(TypstError::UnsupportedTarget(target)),
    }
}

fn collect_target_artifacts(
    temp_root: &Path,
    target: TypstOutputTarget,
) -> Result<Vec<TypstArtifact>, TypstError> {
    let extension = target
        .extension()
        .ok_or(TypstError::UnsupportedTarget(target))?;
    let mut paths = Vec::new();

    match target {
        TypstOutputTarget::Pdf | TypstOutputTarget::Html => {
            paths.push(temp_root.join(format!("output.{extension}")));
        }
        TypstOutputTarget::Png | TypstOutputTarget::Svg => {
            for entry in fs::read_dir(temp_root).map_err(TypstError::Io)? {
                let entry = entry.map_err(TypstError::Io)?;
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                    continue;
                };
                if name.starts_with("output-")
                    && path.extension().and_then(|value| value.to_str()) == Some(extension)
                {
                    paths.push(path);
                }
            }
            paths.sort_by(|left, right| left.file_name().cmp(&right.file_name()));
        }
        TypstOutputTarget::Bundle => return Err(TypstError::UnsupportedTarget(target)),
    }

    if paths.is_empty() {
        return Err(TypstError::Subprocess(format!(
            "Typst produced no {} artifacts",
            target
        )));
    }

    paths
        .into_iter()
        .map(|path| {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| {
                    TypstError::Subprocess("Typst produced a non-UTF-8 artifact name".into())
                })?
                .to_string();
            let bytes = fs::read(&path).map_err(TypstError::Io)?;
            Ok(TypstArtifact::new(name, bytes))
        })
        .collect()
}

fn validate_target_artifacts(
    target: TypstOutputTarget,
    artifacts: &[TypstArtifact],
) -> Result<(), TypstError> {
    if artifacts.is_empty() {
        return Err(TypstError::Subprocess(format!(
            "Typst produced no {} artifacts",
            target
        )));
    }

    for artifact in artifacts {
        if artifact.bytes.is_empty() {
            return Err(TypstError::Subprocess(format!(
                "Typst produced empty {} artifact '{}'",
                target, artifact.path
            )));
        }
        let valid = match target {
            TypstOutputTarget::Pdf => artifact.bytes.starts_with(b"%PDF-"),
            TypstOutputTarget::Png => artifact.bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
            TypstOutputTarget::Svg => std::str::from_utf8(&artifact.bytes)
                .map(|text| text.contains("<svg"))
                .unwrap_or(false),
            TypstOutputTarget::Html => std::str::from_utf8(&artifact.bytes)
                .map(|text| text.contains("<html") || text.contains("<!DOCTYPE html"))
                .unwrap_or(false),
            TypstOutputTarget::Bundle => false,
        };
        if !valid {
            return Err(TypstError::Subprocess(format!(
                "Typst produced invalid {} artifact '{}'",
                target, artifact.path
            )));
        }
    }
    Ok(())
}

'''
text = replace_once(text, "fn validate_entry_path(raw: &str) -> Result<VirtualPathBuf, TypstError> {", helpers + "fn validate_entry_path(raw: &str) -> Result<VirtualPathBuf, TypstError> {", "subprocess helpers")
text = text.replace(
    "assert_eq!(output.pdf.as_deref(), Some(b\"%PDF-1.7 inert\".as_slice()));",
    "assert_eq!(\n            output.single_artifact().map(|artifact| artifact.bytes.as_slice()),\n            Some(b\"%PDF-1.7 inert\".as_slice())\n        );",
)
path.write_text(text)

# in-process backend
path = Path("crates/arkst-typst-inprocess/src/lib.rs")
text = path.read_text()
text = replace_once(
    text,
    "use arkst_typst::{TypstBackend, TypstInput, TypstOutput};",
    "use arkst_typst::{\n    TypstArtifact, TypstBackend, TypstInput, TypstOutput, TypstOutputTarget,\n};",
    "inprocess import",
)
old_sig = '''    pub fn compile_with_source_map(
        &self,
        input: &TypstInput,
        source_map: &[SourceMapEntry],
    ) -> Result<TypstOutput, InProcessError> {
        let start = Instant::now();
'''
new_sig = '''    pub fn compile_with_source_map(
        &self,
        input: &TypstInput,
        source_map: &[SourceMapEntry],
    ) -> Result<TypstOutput, InProcessError> {
        self.compile_target_with_source_map(input, TypstOutputTarget::Pdf, source_map)
    }

    pub fn compile_target_with_source_map(
        &self,
        input: &TypstInput,
        target: TypstOutputTarget,
        source_map: &[SourceMapEntry],
    ) -> Result<TypstOutput, InProcessError> {
        if target != TypstOutputTarget::Pdf {
            return Err(InProcessError::UnsupportedTarget(target));
        }
        let start = Instant::now();
'''
text = replace_once(text, old_sig, new_sig, "inprocess source-map signature")
old_output = '''        Ok(TypstOutput {
            pdf: Some(pdf),
            html: None,
            svg: None,
            png: None,
            diagnostics: warnings,
            duration: start.elapsed(),
        })
'''
new_output = '''        Ok(TypstOutput::new(
            TypstOutputTarget::Pdf,
            vec![TypstArtifact::new("output.pdf", pdf)],
            warnings,
            start.elapsed(),
        ))
'''
text = replace_once(text, old_output, new_output, "inprocess output")
old_trait = '''impl TypstBackend for InProcessBackend<'_> {
    type Error = InProcessError;

    fn compile(&self, input: &TypstInput) -> Result<TypstOutput, Self::Error> {
        self.compile_with_source_map(input, &[])
    }

    fn version(&self) -> Result<String, Self::Error> {
'''
new_trait = '''impl TypstBackend for InProcessBackend<'_> {
    type Error = InProcessError;

    fn compile_target(
        &self,
        input: &TypstInput,
        target: TypstOutputTarget,
    ) -> Result<TypstOutput, Self::Error> {
        self.compile_target_with_source_map(input, target, &[])
    }

    fn supports_target(&self, target: TypstOutputTarget) -> bool {
        target == TypstOutputTarget::Pdf
    }

    fn version(&self) -> Result<String, Self::Error> {
'''
text = replace_once(text, old_trait, new_trait, "inprocess trait")
text = replace_once(
    text,
    "pub enum InProcessError {\n    InvalidInput(String),",
    "pub enum InProcessError {\n    InvalidInput(String),\n    UnsupportedTarget(TypstOutputTarget),",
    "inprocess error variant",
)
text = replace_once(
    text,
    '''            Self::InvalidInput(_) | Self::InvalidOutput(_) | Self::CompilerPanic(_) => &[],
''',
    '''            Self::InvalidInput(_)
            | Self::UnsupportedTarget(_)
            | Self::InvalidOutput(_)
            | Self::CompilerPanic(_) => &[],
''',
    "inprocess diagnostics",
)
text = replace_once(
    text,
    '''            Self::InvalidInput(message) => {
                write!(formatter, "invalid in-process Typst input: {message}")
            }
''',
    '''            Self::InvalidInput(message) => {
                write!(formatter, "invalid in-process Typst input: {message}")
            }
            Self::UnsupportedTarget(target) => {
                write!(formatter, "in-process Typst target '{target}' is not implemented")
            }
''',
    "inprocess display",
)
path.write_text(text)

# CLI keeps its current PDF-only surface in this bounded slice.
path = Path("crates/arkst-cli/src/commands.rs")
text = path.read_text()
text = replace_once(
    text,
    "use arkst_typst::{TypstBackend, TypstInput};",
    "use arkst_typst::{TypstBackend, TypstInput, TypstOutputTarget};",
    "cli typst import",
)
old_pdf = '''                if let Some(pdf_bytes) = typst_output.pdf {
                    write_output_atomically(&resolved_out_path, &pdf_bytes)?;
                    eprintln!("Wrote PDF to {}", resolved_out_path.display());
                } else {
                    anyhow::bail!("PDF backend did not produce output");
                }
'''
new_pdf = '''                if typst_output.target != TypstOutputTarget::Pdf {
                    anyhow::bail!("PDF backend returned unexpected '{}' target", typst_output.target);
                }
                let artifact = typst_output
                    .single_artifact()
                    .ok_or_else(|| anyhow::anyhow!("PDF backend did not produce exactly one artifact"))?;
                write_output_atomically(&resolved_out_path, &artifact.bytes)?;
                eprintln!("Wrote PDF to {}", resolved_out_path.display());
'''
text = replace_once(text, old_pdf, new_pdf, "cli pdf output")
path.write_text(text)
