//! Native Typst CLI subprocess adapter for Scribium.
//!
//! This crate owns process execution, temporary staging, and the explicit
//! project-root security boundary. Pure IR-to-Typst lowering lives in
//! `scribium-typst`.

use scribium_project::VirtualPathBuf;
use scribium_typst::{TypstBackend, TypstInput, TypstOutput};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Filesystem context for a native Typst compilation.
///
/// `project_root` is an explicit read boundary. It is not inferred from the
/// process current directory. The subprocess adapter mirrors this directory
/// into its per-compilation temporary build directory before invoking Typst;
/// the original tree is never used as a write location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypstSourceContext {
    pub project_root: PathBuf,
}

impl TypstSourceContext {
    pub fn new(project_root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
        }
    }
}

/// Errors from the Typst backend.
#[derive(Debug)]
pub enum TypstError {
    Subprocess(String),
    Io(std::io::Error),
    InvalidEntryPath(String),
    InvalidSourceContext(String),
    ResourceBoundaryViolation(String),
}

impl std::fmt::Display for TypstError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TypstError::Subprocess(msg) => write!(f, "subprocess error: {}", msg),
            TypstError::Io(e) => write!(f, "I/O error: {}", e),
            TypstError::InvalidEntryPath(msg) => write!(f, "invalid Typst entry path: {}", msg),
            TypstError::InvalidSourceContext(msg) => {
                write!(f, "invalid Typst source context: {}", msg)
            }
            TypstError::ResourceBoundaryViolation(path) => write!(
                f,
                "Typst resource path leaves the project boundary: {}",
                path
            ),
        }
    }
}

impl std::error::Error for TypstError {}

/// Subprocess backend — calls `typst compile` via CLI.
pub struct SubprocessBackend {
    pub typst_path: PathBuf,
    /// Optional explicit source/read context. `None` keeps self-contained
    /// compilation available and never turns the temporary directory into a
    /// source root implicitly.
    pub source_context: Option<TypstSourceContext>,
}

impl SubprocessBackend {
    pub fn new(typst_path: impl Into<PathBuf>) -> Self {
        Self {
            typst_path: typst_path.into(),
            source_context: None,
        }
    }

    /// Uses an explicit project root as the source/read context for future
    /// compilations by this backend.
    pub fn with_source_context(mut self, source_context: TypstSourceContext) -> Self {
        self.source_context = Some(source_context);
        self
    }
}

impl TypstBackend for SubprocessBackend {
    type Error = TypstError;

    fn compile(&self, input: &TypstInput) -> Result<TypstOutput, TypstError> {
        let start = std::time::Instant::now();
        let entry_path = validate_entry_path(&input.entry_path)?;
        if input.source.contains("@preview/") {
            return Err(TypstError::Subprocess(
                "package resolution is denied by Scribium".into(),
            ));
        }

        // Create a unique temporary directory for the generated Typst source,
        // any source-context mirror, and the output. The returned PDF is the
        // only artifact that escapes this directory.
        let temp_dir = tempfile::tempdir().map_err(TypstError::Io)?;
        let pdf_file = temp_dir.path().join("output.pdf");

        let (typst_file, typst_root) = if let Some(source_context) = &self.source_context {
            let project_root = canonical_project_root(&source_context.project_root)?;
            let mirror_root = temp_dir.path().join("project");
            let mut active_directories = BTreeSet::new();
            copy_project_tree(
                &project_root,
                &mirror_root,
                &project_root,
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
            // Keep self-contained compilation available, but give Typst an
            // empty temporary sandbox rather than making the build directory
            // an accidental source/resource root.
            let isolated_root = temp_dir.path().join("self-contained");
            fs::create_dir_all(&isolated_root).map_err(TypstError::Io)?;
            let typst_file = isolated_root.join("input.typ");
            fs::write(&typst_file, &input.source).map_err(TypstError::Io)?;
            (typst_file, Some(isolated_root))
        };

        // Invoke typst compile
        let mut cmd = Command::new(&self.typst_path);
        cmd.arg("compile");
        if let Some(root) = &typst_root {
            cmd.arg("--root").arg(root);
        }
        cmd.arg(&typst_file).arg(&pdf_file);

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

fn validate_entry_path(raw: &str) -> Result<VirtualPathBuf, TypstError> {
    let path = VirtualPathBuf::parse(raw)
        .map_err(|error| TypstError::InvalidEntryPath(error.to_string()))?;
    if path.is_root() {
        return Err(TypstError::InvalidEntryPath(
            "the entry path must name a document".to_string(),
        ));
    }
    Ok(path)
}

/// Resolves a resource path using Scribium's logical source semantics.
///
/// This helper is intentionally independent of generated Typst parsing. The
/// generated source remains Typst source, while native resource access is
/// bounded by the staged project root and Typst's `--root` option.
#[cfg(test)]
fn resolve_logical_resource_path(
    entry_path: &VirtualPathBuf,
    resource: &str,
) -> Result<VirtualPathBuf, TypstError> {
    entry_path
        .parent()
        .unwrap_or_else(VirtualPathBuf::root)
        .join(resource)
        .map_err(|_| TypstError::ResourceBoundaryViolation(resource.to_string()))
}

fn canonical_project_root(project_root: &Path) -> Result<PathBuf, TypstError> {
    let canonical = project_root.canonicalize().map_err(|error| {
        TypstError::InvalidSourceContext(format!("project root cannot be resolved: {error}"))
    })?;
    let metadata = fs::metadata(&canonical).map_err(TypstError::Io)?;
    if !metadata.is_dir() {
        return Err(TypstError::InvalidSourceContext(
            "project root is not a directory".to_string(),
        ));
    }
    Ok(canonical)
}

fn generated_typst_path(
    mirror_root: &Path,
    entry_path: &VirtualPathBuf,
) -> Result<PathBuf, TypstError> {
    let file_name = entry_path
        .file_name()
        .ok_or_else(|| TypstError::InvalidEntryPath("missing file name".to_string()))?;
    let generated_file_name = Path::new(file_name).with_extension("typ");
    let generated_file_name = generated_file_name
        .file_name()
        .ok_or_else(|| TypstError::InvalidEntryPath("missing file name".to_string()))?;
    let generated_file_name = generated_file_name.to_string_lossy().into_owned();
    let parent = entry_path.parent().unwrap_or_else(VirtualPathBuf::root);
    let preferred_path = native_path_from_virtual(
        mirror_root,
        &parent
            .join(&generated_file_name)
            .map_err(|error| TypstError::InvalidEntryPath(error.to_string()))?,
    );
    if !preferred_path.exists() {
        return Ok(preferred_path);
    }

    // Preserve an existing source-side `.typ` resource instead of making the
    // generated entry shadow it. The mirror is unique, so this name selection
    // is deterministic for a given source tree and remains isolated per build.
    let stem = Path::new(file_name)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| TypstError::InvalidEntryPath("missing file stem".to_string()))?;
    let mut suffix = 0_u64;
    loop {
        let candidate_name = if suffix == 0 {
            format!("{stem}.scribium.typ")
        } else {
            format!("{stem}.scribium-{suffix}.typ")
        };
        let candidate = native_path_from_virtual(
            mirror_root,
            &parent
                .join(&candidate_name)
                .map_err(|error| TypstError::InvalidEntryPath(error.to_string()))?,
        );
        if !candidate.exists() {
            return Ok(candidate);
        }
        suffix = suffix.checked_add(1).ok_or_else(|| {
            TypstError::InvalidSourceContext(
                "could not select a collision-free generated Typst path".to_string(),
            )
        })?;
    }
}

fn native_path_from_virtual(root: &Path, path: &VirtualPathBuf) -> PathBuf {
    let mut native = root.to_path_buf();
    if !path.is_root() {
        for component in path.as_str().split('/') {
            native.push(component);
        }
    }
    native
}

fn copy_project_tree(
    source_root: &Path,
    mirror_root: &Path,
    project_root: &Path,
    logical_directory: &VirtualPathBuf,
    active_directories: &mut BTreeSet<PathBuf>,
) -> Result<(), TypstError> {
    let canonical_directory =
        checked_canonical_target(source_root, project_root, logical_directory)?;
    if !active_directories.insert(canonical_directory.clone()) {
        return Err(TypstError::InvalidSourceContext(
            "project tree contains a directory symlink cycle".to_string(),
        ));
    }

    fs::create_dir_all(mirror_root).map_err(TypstError::Io)?;
    let mut entries = fs::read_dir(source_root)
        .map_err(TypstError::Io)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(TypstError::Io)?;
    entries.sort_by_key(|entry| entry.file_name().to_string_lossy().into_owned());

    for entry in entries {
        let source_path = entry.path();
        let file_name = entry.file_name();
        let file_name = file_name.to_str().ok_or_else(|| {
            TypstError::InvalidSourceContext(
                "project contains a non-UTF-8 path component".to_string(),
            )
        })?;
        let logical_path = if logical_directory.is_root() {
            VirtualPathBuf::parse(file_name)
        } else {
            logical_directory.join(file_name)
        }
        .map_err(|error| TypstError::InvalidSourceContext(error.to_string()))?;
        let mirror_path = mirror_root.join(file_name);
        let file_type = fs::symlink_metadata(&source_path)
            .map_err(TypstError::Io)?
            .file_type();

        if file_type.is_dir()
            || file_type.is_symlink()
                && fs::metadata(&source_path).map_err(TypstError::Io)?.is_dir()
        {
            copy_project_tree(
                &source_path,
                &mirror_path,
                project_root,
                &logical_path,
                active_directories,
            )?;
        } else if file_type.is_file() || file_type.is_symlink() {
            checked_canonical_target(&source_path, project_root, &logical_path)?;
            let target_metadata = fs::metadata(&source_path).map_err(TypstError::Io)?;
            if !target_metadata.is_file() {
                return Err(TypstError::InvalidSourceContext(format!(
                    "unsupported filesystem entry at logical path {}",
                    logical_path
                )));
            }
            fs::copy(&source_path, &mirror_path).map_err(TypstError::Io)?;
        } else {
            return Err(TypstError::InvalidSourceContext(format!(
                "unsupported filesystem entry at logical path {}",
                logical_path
            )));
        }
    }

    active_directories.remove(&canonical_directory);
    Ok(())
}

fn checked_canonical_target(
    path: &Path,
    project_root: &Path,
    logical_path: &VirtualPathBuf,
) -> Result<PathBuf, TypstError> {
    let canonical = fs::canonicalize(path).map_err(TypstError::Io)?;
    if canonical != project_root && !canonical.starts_with(project_root) {
        return Err(TypstError::ResourceBoundaryViolation(
            logical_path.to_string(),
        ));
    }
    Ok(canonical)
}

fn sanitize_typst_diagnostic(stderr: &[u8], temporary_root: &Path) -> String {
    let mut diagnostic = String::from_utf8_lossy(stderr).into_owned();
    let temporary_root = temporary_root.to_string_lossy();
    let native_root = temporary_root.to_string();
    let forward_slash_root = temporary_root.replace('\\', "/");
    let backslash_root = temporary_root.replace('/', "\\");
    let root_variants = [
        format!(r"\\?\{native_root}"),
        format!(r"\\?\{forward_slash_root}"),
        format!("//?/{native_root}"),
        format!("//?/{forward_slash_root}"),
        native_root,
        forward_slash_root,
        backslash_root,
    ];

    for root in root_variants {
        diagnostic = diagnostic.replace(&root, "<typst-build>");
    }

    let marker = "<typst-build>";
    let mut normalized = String::with_capacity(diagnostic.len());
    let mut offset = 0;
    while let Some(relative_start) = diagnostic[offset..].find(marker) {
        let marker_start = offset + relative_start;
        normalized.push_str(&diagnostic[offset..marker_start]);
        normalized.push_str(marker);
        let path_start = marker_start + marker.len();
        let path_end = path_start
            + diagnostic[path_start..]
                .find(char::is_whitespace)
                .unwrap_or(diagnostic.len() - path_start);
        normalized.push_str(&diagnostic[path_start..path_end].replace('\\', "/"));
        offset = path_end;
    }
    normalized.push_str(&diagnostic[offset..]);
    sanitize_absolute_path_tokens(&normalized)
}

fn sanitize_absolute_path_tokens(diagnostic: &str) -> String {
    let mut starts = BTreeSet::new();
    let bytes = diagnostic.as_bytes();
    for (index, window) in bytes.windows(3).enumerate() {
        let preceded_by_token = index > 0 && bytes[index - 1].is_ascii_alphanumeric();
        if !preceded_by_token
            && window[0].is_ascii_alphabetic()
            && window[1] == b':'
            && matches!(window[2], b'/' | b'\\')
        {
            starts.insert(index);
        }
    }
    for (index, _) in diagnostic.match_indices("\\\\") {
        starts.insert(index);
    }
    for marker in [
        "/tmp/",
        "/private/var/",
        "/var/folders/",
        "/Users/",
        "\\tmp\\",
        "\\Users\\",
        "/home/runner/",
        "\\home\\runner\\",
        "runner.workspace",
        "github.workspace",
        "target/",
        "\\target\\",
    ] {
        for (index, _) in diagnostic.match_indices(marker) {
            starts.insert(index);
        }
    }

    let mut sanitized = String::with_capacity(diagnostic.len());
    let mut offset = 0;
    for start in starts {
        if start < offset {
            continue;
        }
        sanitized.push_str(&diagnostic[offset..start]);
        let end = start
            + diagnostic[start..]
                .find(char::is_whitespace)
                .unwrap_or(diagnostic.len() - start);
        let token = &diagnostic[start..end];
        if let Some(logical_path) = logical_path_from_absolute_token(token) {
            sanitized.push_str(&logical_path);
        } else {
            sanitized.push_str("<host-path>");
        }
        offset = end;
    }
    sanitized.push_str(&diagnostic[offset..]);
    sanitized
}

fn logical_path_from_absolute_token(token: &str) -> Option<String> {
    let normalized = token.replace('\\', "/");
    let (_, logical_path) = normalized.split_once("/project/")?;
    Some(format!("/{logical_path}"))
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
            script.push_str(
                "  if [ \"$2\" = \"--root\" ]; then output=\"$5\"; else output=\"$3\"; fi\n",
            );
            script.push_str(&format!("  printf '%s' '{}' > \"$output\"\n", pdf_body));
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
    fn write_recording_fake_typst(dir: &std::path::Path, pdf_body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let script = format!(
            "#!/bin/sh\nif [ \"$1\" = \"compile\" ]; then\n  printf '%s\\n' \"$@\" > \"$0.args\"\n  if [ \"$2\" = \"--root\" ]; then output=\"$5\"; else output=\"$3\"; fi\n  printf '%s' '{}' > \"$output\"\n  exit 0\nfi\nprintf '%s\\n' 'typst fake 0.15.1'\n",
            pdf_body
        );
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

    #[test]
    fn subprocess_backend_denies_preview_packages_before_execution() {
        let backend = SubprocessBackend::new("/nonexistent/typst");
        let result = backend.compile(&TypstInput {
            source: "#import \"@preview/not-present:1.0.0\": *\n".to_string(),
            entry_path: "docs/main.qd".to_string(),
        });

        let error = result
            .expect_err("package access must be denied")
            .to_string();
        assert!(error.contains("package resolution is denied"));
        assert!(!error.contains("http://") && !error.contains("https://"));
        assert!(!error.contains("not found at"));
    }

    #[test]
    fn typst_diagnostics_sanitize_native_and_slash_temp_paths() {
        let temporary_root = Path::new("D:\\a\\_temp\\scribium");
        let stderr = "error: D:/a/_temp/scribium/project/docs/main.typ:1:1\n".to_string()
            + "error: \\\\?\\D:\\a\\_temp\\scribium\\project\\docs\\main.typ:2:1\n"
            + "error: C:/Users/runneradmin/AppData/Temp/project/docs/main.typ:3:1\n";

        let sanitized = sanitize_typst_diagnostic(stderr.as_bytes(), temporary_root);

        assert_eq!(
            sanitized,
            "error: <typst-build>/project/docs/main.typ:1:1\n".to_string()
                + "error: <typst-build>/project/docs/main.typ:2:1\n"
                + "error: /docs/main.typ:3:1\n"
        );
        assert!(!sanitized.contains("D:/a/"));
        assert!(!sanitized.contains("D:\\a\\"));
        assert!(!sanitized.contains("C:/Users/"));
        assert!(!sanitized.contains("\\\\?\\"));
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

    #[test]
    fn entry_path_is_normalized_and_project_relative() {
        assert_eq!(
            validate_entry_path("./docs/../docs/main.qd")
                .unwrap()
                .as_str(),
            "docs/main.qd"
        );

        for invalid in [
            "",
            "../main.qd",
            "/absolute/main.qd",
            "C:\\absolute\\main.qd",
        ] {
            assert!(
                matches!(
                    validate_entry_path(invalid),
                    Err(TypstError::InvalidEntryPath(_))
                ),
                "entry path should be rejected: {invalid:?}"
            );
        }
    }

    #[test]
    fn relative_resource_resolution_is_bounded_by_project_root() {
        let entry = validate_entry_path("manual/chapter1/main.qd").unwrap();
        assert_eq!(
            resolve_logical_resource_path(&entry, "./assets/a.png")
                .unwrap()
                .as_str(),
            "manual/chapter1/assets/a.png"
        );
        assert_eq!(
            resolve_logical_resource_path(&entry, "../shared/header.typ")
                .unwrap()
                .as_str(),
            "manual/shared/header.typ"
        );
        assert_eq!(
            resolve_logical_resource_path(&entry, "../../shared/header.typ")
                .unwrap()
                .as_str(),
            "shared/header.typ"
        );

        for invalid in [
            "../../../secret.txt",
            "/etc/passwd",
            "C:\\Users\\foo\\secret.txt",
        ] {
            assert!(
                matches!(
                    resolve_logical_resource_path(&entry, invalid),
                    Err(TypstError::ResourceBoundaryViolation(_))
                ),
                "resource path should be rejected: {invalid:?}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn source_context_uses_a_temp_mirror_and_preserves_the_source_tree() {
        let project_parent = tempfile::tempdir().unwrap();
        let project_root = project_parent.path().join("project with spaces");
        fs::create_dir_all(project_root.join("docs")).unwrap();
        fs::write(project_root.join("docs/main.qd"), "original source\n").unwrap();

        let fake_parent = tempfile::tempdir().unwrap();
        let _spawn_guard = FAKE_TYPST_SPAWN_LOCK.lock().unwrap();
        let fake = write_recording_fake_typst(fake_parent.path(), "%PDF-1.7 fake");
        let backend = SubprocessBackend::new(&fake)
            .with_source_context(TypstSourceContext::new(&project_root));
        let output = backend
            .compile(&TypstInput {
                source: "Hello from generated source\n".to_string(),
                entry_path: "docs/main.qd".to_string(),
            })
            .unwrap();
        assert!(output.pdf.is_some_and(|pdf| pdf.starts_with(b"%PDF-")));

        let args = fs::read_to_string(fake_parent.path().join("fake_typst.args")).unwrap();
        let args: Vec<_> = args.lines().collect();
        assert_eq!(args[0], "compile");
        assert_eq!(args[1], "--root");
        let mirror_root = PathBuf::from(args[2]);
        let generated_input = PathBuf::from(args[3]);
        let output_path = PathBuf::from(args[4]);
        assert_ne!(mirror_root, project_root);
        assert_eq!(generated_input, mirror_root.join("docs/main.typ"));
        assert_ne!(output_path, project_root.join("output.pdf"));
        assert!(
            !generated_input.exists(),
            "temporary mirror must be cleaned"
        );
        assert!(!project_root.join("docs/main.typ").exists());
        assert!(!project_root.join("output.pdf").exists());
        assert_eq!(
            fs::read_to_string(project_root.join("docs/main.qd")).unwrap(),
            "original source\n"
        );
    }

    #[cfg(unix)]
    #[test]
    fn source_context_rejects_file_symlink_escape() {
        use std::os::unix::fs::symlink;

        let project_parent = tempfile::tempdir().unwrap();
        let project_root = project_parent.path().join("project");
        let outside = project_parent.path().join("outside");
        fs::create_dir_all(project_root.join("assets")).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("secret.txt"), "secret").unwrap();
        symlink(
            outside.join("secret.txt"),
            project_root.join("assets/leak.txt"),
        )
        .unwrap();

        let backend = SubprocessBackend::new("/nonexistent/typst")
            .with_source_context(TypstSourceContext::new(&project_root));
        let result = backend.compile(&TypstInput {
            source: "Hello\n".to_string(),
            entry_path: "main.qd".to_string(),
        });
        assert!(matches!(
            result,
            Err(TypstError::ResourceBoundaryViolation(path)) if path == "assets/leak.txt"
        ));
    }

    #[cfg(unix)]
    #[test]
    fn source_context_rejects_directory_symlink_escape() {
        use std::os::unix::fs::symlink;

        let project_parent = tempfile::tempdir().unwrap();
        let project_root = project_parent.path().join("project");
        let outside = project_parent.path().join("outside");
        fs::create_dir_all(&project_root).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("secret.txt"), "secret").unwrap();
        symlink(&outside, project_root.join("assets")).unwrap();

        let backend = SubprocessBackend::new("/nonexistent/typst")
            .with_source_context(TypstSourceContext::new(&project_root));
        let result = backend.compile(&TypstInput {
            source: "Hello\n".to_string(),
            entry_path: "main.qd".to_string(),
        });
        assert!(matches!(
            result,
            Err(TypstError::ResourceBoundaryViolation(path)) if path == "assets"
        ));
    }
}
