/// Typst backend trait and adapters.
use std::fs;
use std::path::PathBuf;
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
    pub typst_path: PathBuf,
}

impl SubprocessBackend {
    pub fn new(typst_path: impl Into<PathBuf>) -> Self {
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
                    self.typst_path.display(),
                    e
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

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::fs;
    use std::sync::Mutex;

    /// Serializes write-then-spawn of the fake Typst executables.
    ///
    /// Cargo runs tests as threads in one process. Linux `execve(2)` returns
    /// `ETXTBSY` ("Text file busy") when a file is executed while any task —
    /// including a child forked by a parallel test's `Command::spawn` — still
    /// holds it open for writing, which races the freshly written fake
    /// scripts under CI load. macOS and Windows do not enforce this at exec
    /// time.
    static FAKE_TYPST_SPAWN_LOCK: Mutex<()> = Mutex::new(());

    /// Writes a fake Typst executable to `dir` and returns its path.
    ///
    /// The script honours the subprocess protocol used by the backend:
    /// `compile <input.typ> <output.pdf>` and `--version`. `pdf_body` is
    /// written verbatim to the third argument when invoked as `compile`;
    /// `stderr` (when non-empty) is written to stderr and the process exits
    /// with `status` instead. Version invocations always succeed.
    ///
    /// The fixture is a small shell script spawned by the backend directly
    /// via `std::process::Command` — the script is a stand-in for the real
    /// Typst binary, not a command wrapper — so the "no shell invocation"
    /// rule is unaffected. It is unix-only: Windows `CreateProcess` cannot
    /// execute `.cmd`/`.bat` files, so executable-spawning tests run only on
    /// unix, while the real-Typst integration tests
    /// (`tests/backend_integration.rs`) cover every OS in CI.
    #[cfg(unix)]
    fn write_fake_typst(
        dir: &std::path::Path,
        pdf_body: &str,
        stderr: &str,
        status: i32,
    ) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let mut script = String::new();
        script.push_str("#!/bin/sh\n");
        if stderr.is_empty() {
            script.push_str("if [ \"$1\" = \"compile\" ]; then\n");
            script.push_str(&format!("  printf '%s' '{}' > \"$3\"\n", pdf_body));
            script.push_str("  exit 0\n");
            script.push_str("fi\n");
            script.push_str("printf '%s\\n' 'typst fake 0.15.1'\n");
        } else {
            script.push_str(&format!("printf '%s\\n' '{}' >&2\n", stderr));
            script.push_str(&format!("exit {}\n", status));
        }
        let path = dir.join("fake_typst");
        fs::write(&path, script).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    #[cfg(unix)]
    fn compile_with(fake: &std::path::Path) -> Result<TypstOutput, TypstError> {
        let backend = SubprocessBackend::new(fake);
        let input = TypstInput {
            source: "#heading[Test]\n\nHello world.\n".to_string(),
            entry_path: "test.qd".to_string(),
        };
        backend.compile(&input)
    }

    #[cfg(unix)]
    #[test]
    fn subprocess_backend_version() {
        let dir = tempfile::tempdir().unwrap();
        let _spawn_guard = FAKE_TYPST_SPAWN_LOCK.lock().unwrap();
        let fake = write_fake_typst(dir.path(), "%PDF-1.7 fake", "", 0);
        let backend = SubprocessBackend::new(fake);
        let version = backend.version().expect("version should succeed");
        assert!(version.contains("typst"), "version was: {}", version);
        assert!(version.contains("0.15.1"), "version was: {}", version);
    }

    #[test]
    fn subprocess_backend_missing_executable() {
        let _spawn_guard = FAKE_TYPST_SPAWN_LOCK.lock().unwrap();
        let backend = SubprocessBackend::new("/nonexistent/typst");
        let result = backend.version();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not found"), "error was: {}", err);
        assert!(
            err.contains("nonexistent"),
            "error must name the configured path: {}",
            err
        );
    }

    #[cfg(unix)]
    #[test]
    fn subprocess_backend_compile_success() {
        let dir = tempfile::tempdir().unwrap();
        let _spawn_guard = FAKE_TYPST_SPAWN_LOCK.lock().unwrap();
        let fake = write_fake_typst(dir.path(), "%PDF-1.7 fake", "", 0);
        let output = compile_with(&fake).expect("compile should succeed");
        let pdf = output.pdf.expect("pdf output must be present");
        assert!(!pdf.is_empty());
        assert!(pdf.starts_with(b"%PDF-"), "pdf header was: {:?}", &pdf[..8]);
        assert_eq!(pdf, b"%PDF-1.7 fake");
    }

    #[cfg(unix)]
    #[test]
    fn subprocess_backend_invalid_pdf_header_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let _spawn_guard = FAKE_TYPST_SPAWN_LOCK.lock().unwrap();
        // The fake exits successfully but writes a non-PDF file.
        let fake = write_fake_typst(dir.path(), "garbage not a pdf", "", 0);
        let result = compile_with(&fake);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("invalid PDF output: missing %PDF- header"),
            "error was: {}",
            err
        );
    }

    #[cfg(unix)]
    #[test]
    fn subprocess_backend_compile_failure_surfaces_stderr() {
        let dir = tempfile::tempdir().unwrap();
        let _spawn_guard = FAKE_TYPST_SPAWN_LOCK.lock().unwrap();
        let fake = write_fake_typst(dir.path(), "", "fake typst error: bad syntax", 1);
        let result = compile_with(&fake);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Typst compilation failed"),
            "error was: {}",
            err
        );
        assert!(err.contains("fake typst error"), "error was: {}", err);
    }
}
