/// Typst backend trait and adapters.
use std::fs;
use std::process::Command;
use std::time::Duration;

/// Abstract interface for the Typst compiler backend.
pub trait TypstBackend {
    /// Compile a Typst source document.
    fn compile(&self, input: &TypstInput) -> Result<TypstOutput, TypstError>;
    /// Return the Typst compiler version.
    fn version(&self) -> Result<String, TypstError>;
}

/// Input to the Typst compiler.
pub struct TypstInput {
    pub source: String,
    pub entry_path: String,
}

/// Output from the Typst compiler.
#[derive(Debug)]
pub struct TypstOutput {
    pub pdf: Option<Vec<u8>>,
    pub html: Option<String>,
    pub svg: Option<Vec<u8>>,
    pub png: Option<Vec<u8>>,
    pub diagnostics: Vec<String>,
    pub duration: Duration,
}

/// Errors from the Typst backend.
#[derive(Debug)]
pub enum TypstError {
    Subprocess(String),
    Io(std::io::Error),
}

impl std::fmt::Display for TypstError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TypstError::Subprocess(msg) => write!(f, "subprocess error: {}", msg),
            TypstError::Io(e) => write!(f, "I/O error: {}", e),
        }
    }
}

impl std::error::Error for TypstError {}

/// Subprocess backend — calls `typst compile` via CLI.
pub struct SubprocessBackend {
    pub typst_path: String,
}

impl SubprocessBackend {
    pub fn new(typst_path: impl Into<String>) -> Self {
        Self {
            typst_path: typst_path.into(),
        }
    }
}

impl TypstBackend for SubprocessBackend {
    fn compile(&self, input: &TypstInput) -> Result<TypstOutput, TypstError> {
        let start = std::time::Instant::now();

        // Create a temporary directory for the Typst source and output
        let temp_dir = tempfile::tempdir().map_err(TypstError::Io)?;
        let typst_file = temp_dir.path().join("input.typ");
        let pdf_file = temp_dir.path().join("output.pdf");

        // Write the Typst source to the temporary file
        fs::write(&typst_file, &input.source).map_err(TypstError::Io)?;

        // Invoke typst compile
        let mut cmd = Command::new(&self.typst_path);
        cmd.arg("compile").arg(&typst_file).arg(&pdf_file);

        let output = cmd.output().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                TypstError::Subprocess(format!(
                    "Typst executable not found at '{}': {}",
                    self.typst_path, e
                ))
            } else {
                TypstError::Io(e)
            }
        })?;

        let duration = start.elapsed();

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(TypstError::Subprocess(format!(
                "Typst compilation failed:\n{}",
                stderr.trim()
            )));
        }

        // Read the generated PDF
        let pdf_bytes = fs::read(&pdf_file).map_err(TypstError::Io)?;

        if pdf_bytes.is_empty() {
            return Err(TypstError::Subprocess(
                "Typst produced empty PDF output".into(),
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
        let output = Command::new(&self.typst_path)
            .arg("--version")
            .output()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    TypstError::Subprocess(format!(
                        "Typst executable not found at '{}': {}",
                        self.typst_path, e
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subprocess_backend_version() {
        let backend = SubprocessBackend::new("typst");
        let version = backend.version().expect("version should succeed");
        assert!(version.contains("typst") || version.contains("0."));
    }

    #[test]
    fn subprocess_backend_missing_executable() {
        let backend = SubprocessBackend::new("/nonexistent/typst");
        let result = backend.version();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not found"));
    }

    #[test]
    fn subprocess_backend_compile_success() {
        let backend = SubprocessBackend::new("typst");
        let input = TypstInput {
            source: "#heading[Test]\n\nHello world.\n".to_string(),
            entry_path: "test.qd".to_string(),
        };
        let output = backend.compile(&input).expect("compile should succeed");
        assert!(output.pdf.is_some());
        let pdf = output.pdf.unwrap();
        assert!(!pdf.is_empty());
        assert!(pdf.starts_with(b"%PDF-"));
    }

    #[test]
    fn subprocess_backend_compile_failure() {
        let backend = SubprocessBackend::new("typst");
        // Invalid Typst syntax - unmatched bracket
        let input = TypstInput {
            source: "#heading[Test\n".to_string(),
            entry_path: "test.qd".to_string(),
        };
        let result = backend.compile(&input);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("compilation failed") || err.contains("error"));
    }
}
