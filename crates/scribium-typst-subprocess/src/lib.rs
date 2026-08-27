//! Native Typst CLI subprocess adapter for Scribium.
//!
//! This crate owns process execution, temporary staging, and the explicit
//! project-root staging boundary. Static Typst module preflight is a
//! best-effort validation aid, not a package or network security boundary.
//! Pure IR-to-Typst lowering lives in `scribium-typst`.

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

        // Create a unique temporary directory for the generated Typst source,
        // any source-context mirror, and the output. The returned PDF is the
        // only artifact that escapes this directory.
        let temp_dir = tempfile::tempdir().map_err(TypstError::Io)?;
        let pdf_file = temp_dir.path().join("output.pdf");

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

fn static_module_preflight_rejected(logical_path: &VirtualPathBuf) -> TypstError {
    TypstError::Subprocess(format!(
        "static Typst module preflight rejected a package or dynamic module operand for {}",
        logical_path
    ))
}

/// Applies best-effort static validation to the active local Typst module graph
/// rooted at the generated entry source. The project mirror contains every
/// project file, but only modules reachable through literal project-relative
/// `import` or `include` operands are inspected here. This keeps unused files
/// inert while catching obvious package references in reachable helper modules.
/// It does not prove that runtime evaluation cannot reach a package resolver.
fn validate_reachable_typst_modules(
    source: &str,
    logical_path: &VirtualPathBuf,
    project_root: &Path,
) -> Result<(), TypstError> {
    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    validate_reachable_typst_module(
        source,
        logical_path,
        project_root,
        &mut visiting,
        &mut visited,
    )
}

fn validate_reachable_typst_module(
    source: &str,
    logical_path: &VirtualPathBuf,
    project_root: &Path,
    visiting: &mut BTreeSet<VirtualPathBuf>,
    visited: &mut BTreeSet<VirtualPathBuf>,
) -> Result<(), TypstError> {
    if visited.contains(logical_path) || !visiting.insert(logical_path.clone()) {
        return Ok(());
    }

    let mut local_modules = Vec::new();
    if !collect_static_typst_module_references(source, &mut local_modules) {
        return Err(static_module_preflight_rejected(logical_path));
    }

    visited.insert(logical_path.clone());
    for module_reference in local_modules {
        let module_path = resolve_logical_resource_path(logical_path, &module_reference)?;
        let native_path = native_path_from_virtual(project_root, &module_path);

        // Let Typst report a normal missing-module error. Existing files are
        // the only paths that need policy inspection before the subprocess is
        // spawned.
        if !native_path.exists() {
            continue;
        }
        let metadata = fs::metadata(&native_path).map_err(TypstError::Io)?;
        if !metadata.is_file() {
            continue;
        }
        checked_canonical_target(&native_path, project_root, &module_path)?;
        let module_source = fs::read_to_string(&native_path).map_err(TypstError::Io)?;
        validate_reachable_typst_module(
            &module_source,
            &module_path,
            project_root,
            visiting,
            visited,
        )?;
    }

    visiting.remove(logical_path);
    Ok(())
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
    let mut sanitized = String::with_capacity(diagnostic.len());
    let mut offset = 0;
    while let Some(relative_start) = next_absolute_path_start(&diagnostic[offset..]) {
        let start = offset + relative_start;
        sanitized.push_str(&diagnostic[offset..start]);
        let end = start
            + diagnostic[start..]
                .find(char::is_whitespace)
                .unwrap_or(diagnostic.len() - start);
        let token = &diagnostic[start..end];
        let (path, location) = split_diagnostic_location(token);
        if let Some(logical_path) = logical_path_from_absolute_token(path) {
            sanitized.push_str(&logical_path);
        } else {
            sanitized.push_str("<host-path>");
        }
        sanitized.push_str(location);
        offset = end;
    }
    sanitized.push_str(&diagnostic[offset..]);
    sanitized
}

fn split_diagnostic_location(token: &str) -> (&str, &str) {
    let Some(last_colon) = token.rfind(':') else {
        return (token, "");
    };
    if !token[last_colon + 1..]
        .bytes()
        .all(|byte| byte.is_ascii_digit())
    {
        return (token, "");
    }

    let Some(line_colon) = token[..last_colon].rfind(':') else {
        return (token, "");
    };
    if token[line_colon + 1..last_colon]
        .bytes()
        .all(|byte| byte.is_ascii_digit())
    {
        return (&token[..line_colon], &token[line_colon..]);
    }

    (&token[..last_colon], &token[last_colon..])
}

fn next_absolute_path_start(diagnostic: &str) -> Option<usize> {
    let bytes = diagnostic.as_bytes();
    for (index, byte) in bytes.iter().copied().enumerate() {
        if !path_token_boundary(bytes, index) {
            continue;
        }

        if index + 2 < bytes.len()
            && byte.is_ascii_alphabetic()
            && bytes[index + 1] == b':'
            && matches!(bytes[index + 2], b'/' | b'\\')
        {
            return Some(index);
        }

        if index + 1 < bytes.len() && matches!(byte, b'/' | b'\\') && bytes[index + 1] == byte {
            return Some(index);
        }

        // A single leading backslash is a rooted Windows path. Relative
        // components such as `target\\main.typ` do not reach this branch.
        if byte == b'\\' && bytes.get(index + 1).is_some_and(|next| *next != b'\\') {
            return Some(index);
        }

        // A Unix absolute path. `//...` was handled as UNC above, while
        // `<typst-build>/project/...` is intentionally not a token boundary
        // at the slash after the synthetic marker.
        if byte == b'/'
            && bytes
                .get(index + 1)
                .is_some_and(|next| !next.is_ascii_whitespace())
        {
            return Some(index);
        }
    }
    None
}

fn path_token_boundary(bytes: &[u8], index: usize) -> bool {
    index == 0
        || bytes[index - 1].is_ascii_whitespace()
        || matches!(
            bytes[index - 1],
            b'(' | b'[' | b'{' | b'<' | b':' | b'=' | b'"' | b'\''
        )
}

fn logical_path_from_absolute_token(token: &str) -> Option<String> {
    let normalized = token.replace('\\', "/");
    let (_, logical_path) = normalized.split_once("/project/")?;
    Some(format!("/{logical_path}"))
}

/// Detects obvious static module references for best-effort preflight without
/// embedding the Typst compiler or evaluator. The syntax-only parser
/// identifies active `import`/`include` nodes, so comments, raw text, and
/// ordinary strings are not mistaken for module sources. Literal package
/// specifications use Typst's own grammar; dynamic module expressions are
/// rejected because this adapter cannot prove that they remain project-local
/// without evaluating code. Runtime evaluation is deliberately outside this
/// check: this function does not inspect identifiers such as `eval` or claim
/// to make runtime package access impossible.
fn contains_static_typst_preflight_violation(source: &str) -> bool {
    let mut local_modules = Vec::new();
    !collect_static_typst_module_references(source, &mut local_modules)
}

fn collect_static_typst_module_references(source: &str, local_modules: &mut Vec<String>) -> bool {
    let root = typst_syntax::parse(source);
    collect_static_typst_module_references_from_node(&root, local_modules)
}

fn collect_static_typst_module_references_from_node(
    node: &typst_syntax::SyntaxNode,
    local_modules: &mut Vec<String>,
) -> bool {
    let module_source = match node.kind() {
        typst_syntax::SyntaxKind::ModuleImport => node
            .cast::<typst_syntax::ast::ModuleImport>()
            .map(|module| module.source()),
        typst_syntax::SyntaxKind::ModuleInclude => node
            .cast::<typst_syntax::ast::ModuleInclude>()
            .map(|module| module.source()),
        _ => None,
    };

    if let Some(module_source) = module_source {
        if module_source_requires_static_rejection(module_source) {
            return false;
        }
        let typst_syntax::ast::Expr::Str(string) = module_source else {
            return false;
        };
        local_modules.push(string.get().to_string());
    }

    node.children()
        .all(|child| collect_static_typst_module_references_from_node(child, local_modules))
}

fn module_source_requires_static_rejection(source: typst_syntax::ast::Expr<'_>) -> bool {
    match source {
        typst_syntax::ast::Expr::Str(string) => string
            .get()
            .parse::<typst_syntax::package::PackageSpec>()
            .is_ok(),
        _ => true,
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
    fn subprocess_backend_rejects_static_package_namespaces_before_execution() {
        let backend = SubprocessBackend::new("/nonexistent/typst");
        for source in [
            "#import \"@preview/not-present:1.0.0\": *\n",
            "#import \"@local/company-package:1.0.0\": *\n",
            "#include \"@company/internal-package:2.3.4\"\n",
            "#{ import \"@workspace/private-package:0.1.0\" }\n",
        ] {
            let result = backend.compile(&TypstInput {
                source: source.to_string(),
                entry_path: "docs/main.qd".to_string(),
            });

            let error = result
                .expect_err("static package access must be rejected by preflight")
                .to_string();
            assert!(error.contains("static Typst module preflight rejected"));
            assert!(!error.contains("http://") && !error.contains("https://"));
            assert!(!error.contains("not found at"));
        }
    }

    #[cfg(unix)]
    #[test]
    fn subprocess_backend_ignores_package_looking_inert_text() {
        let dir = tempfile::tempdir().unwrap();
        let _spawn_guard = FAKE_TYPST_SPAWN_LOCK.lock().unwrap();
        let fake = write_fake_typst(dir.path(), "%PDF-1.7 inert", "", 0);
        let source = r###"// #import "@preview/comment:1.0.0": *
/* nested /* #include "@local/comment:1.0.0" */ comment */
`#import "@preview/inline-raw:1.0.0": *`
```typst
#import "@workspace/raw-block:1.0.0": *
```
#let example = "@company/example:1.0.0"
#raw("#import \"@preview/raw-string:1.0.0\": *")
\#import "@preview/escaped-hash:1.0.0": *
import "@preview/markup-text:1.0.0"
"###;
        let output = SubprocessBackend::new(fake)
            .compile(&TypstInput {
                source: source.to_string(),
                entry_path: "docs/main.qd".to_string(),
            })
            .expect("inert package-looking text must not be denied");
        assert_eq!(output.pdf.as_deref(), Some(b"%PDF-1.7 inert".as_slice()));
    }

    #[cfg(unix)]
    #[test]
    fn source_context_preflight_follows_reachable_local_modules() {
        let project_parent = tempfile::tempdir().unwrap();
        let project_root = project_parent.path().join("project");
        fs::create_dir_all(project_root.join("docs")).unwrap();
        let backend = SubprocessBackend::new("/nonexistent/typst")
            .with_source_context(TypstSourceContext::new(&project_root));

        for package in [
            "@preview/nested-package:1.0.0",
            "@local/nested-package:1.0.0",
        ] {
            fs::write(
                project_root.join("docs/helper.typ"),
                format!("#import \"{package}\": *\n"),
            )
            .unwrap();
            let error = backend
                .compile(&TypstInput {
                    source: "#import \"./helper.typ\": *\n".to_string(),
                    entry_path: "docs/main.qd".to_string(),
                })
                .expect_err("a reachable helper package must be rejected by static preflight");
            let error = error.to_string();
            assert!(
                error.contains("static Typst module preflight rejected"),
                "{error}"
            );
            assert!(!error.contains("not found at"), "{error}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn source_context_module_preflight_is_cycle_safe() {
        let project_parent = tempfile::tempdir().unwrap();
        let project_root = project_parent.path().join("project");
        fs::create_dir_all(project_root.join("docs")).unwrap();
        fs::write(
            project_root.join("docs/helper.typ"),
            "#import \"./helper.typ\": *\n",
        )
        .unwrap();
        let project_root = canonical_project_root(&project_root).unwrap();

        validate_reachable_typst_modules(
            "#import \"./helper.typ\": *\n",
            &VirtualPathBuf::parse("docs/main.qd").unwrap(),
            &project_root,
        )
        .unwrap();
    }

    #[test]
    fn typst_diagnostics_sanitize_native_and_slash_temp_paths() {
        let temporary_root = Path::new("D:\\a\\_temp\\scribium");
        let stderr = "error: D:/a/_temp/scribium/project/docs/main.typ:1:1\n".to_string()
            + "error: \\\\?\\D:\\a\\_temp\\scribium\\project\\docs\\main.typ:2:1\n"
            + "error: C:/Users/runneradmin/AppData/Temp/project/docs/target/Users/main.typ:3:1\n"
            + "error: <typst-build>/project/docs/target/Users/main.typ:4:1\n"
            + "error: \\\\server\\share\\outside\\main.typ:5:1\n"
            + "error: \\\\server\\share\\project\\docs\\target\\Users\\main.typ:6:1\n";

        let sanitized = sanitize_typst_diagnostic(stderr.as_bytes(), temporary_root);

        assert_eq!(
            sanitized,
            "error: <typst-build>/project/docs/main.typ:1:1\n".to_string()
                + "error: <typst-build>/project/docs/main.typ:2:1\n"
                + "error: /docs/target/Users/main.typ:3:1\n"
                + "error: <typst-build>/project/docs/target/Users/main.typ:4:1\n"
                + "error: <host-path>:5:1\n"
                + "error: /docs/target/Users/main.typ:6:1\n"
        );
        assert!(!sanitized.contains("D:/a/"));
        assert!(!sanitized.contains("D:\\a\\"));
        assert!(!sanitized.contains("C:/Users/"));
        assert!(!sanitized.contains("\\\\?\\"));
    }

    #[test]
    fn static_package_preflight_only_matches_active_module_operands() {
        assert!(contains_static_typst_preflight_violation(
            "#import \"@preview/pkg:1.0.0\": *"
        ));
        assert!(contains_static_typst_preflight_violation(
            "#include \"@local/pkg:1.0.0\""
        ));
        assert!(contains_static_typst_preflight_violation(
            "#{ import \"@company/pkg:1.0.0\" }"
        ));
        assert!(contains_static_typst_preflight_violation(
            "#let module = { import \"@workspace/pkg:1.0.0\" }"
        ));
        assert!(contains_static_typst_preflight_violation(
            "#let package = \"@local/pkg:1.0.0\"\n#import package"
        ));

        for source in [
            "// #import \"@preview/pkg:1.0.0\": *",
            "/* #include \"@local/pkg:1.0.0\" */",
            "`#import \"@company/pkg:1.0.0\"`",
            "```typst\n#import \"@company/pkg:1.0.0\"\n```",
            "#let text = \"@preview/pkg:1.0.0\"",
            "#raw(\"#import \\\"@preview/pkg:1.0.0\\\": *\")",
            "\\#import \"@preview/pkg:1.0.0\"",
            "import \"@preview/pkg:1.0.0\"",
            "text { import \"@preview/pkg:1.0.0\" }",
        ] {
            assert!(
                !contains_static_typst_preflight_violation(source),
                "inert text was classified as a package reference: {source:?}"
            );
        }
    }

    #[test]
    fn static_package_preflight_does_not_block_runtime_evaluation() {
        for source in [
            "#eval(\"import \\\"@preview/pkg:1.0.0\\\": *\", mode: \"code\")",
            "#let package = \"@preview/\" + \"pkg:1.0.0\"\n#eval(\"import \\\"\" + package + \"\\\": *\", mode: \"code\")",
            "#let runtime_eval = eval\n#runtime_eval(\"import \\\"@preview/pkg:1.0.0\\\": *\", mode: \"code\")",
            "#let runtime_eval = std.eval\n#runtime_eval(\"import \\\"@preview/pkg:1.0.0\\\": *\", mode: \"code\")",
        ] {
            assert!(
                !contains_static_typst_preflight_violation(source),
                "runtime evaluation must not be rejected by static preflight: {source:?}"
            );
        }

        for source in [
            "// #eval(\"#import \\\"@preview/pkg:1.0.0\\\": *\")",
            "`#eval(\"@preview/pkg:1.0.0\")`",
            "```typst\n#eval(\"@preview/pkg:1.0.0\")\n```",
            "#let text = \"eval(\\\"@preview/pkg:1.0.0\\\")\"",
            "#raw(\"#eval(\\\"@preview/pkg:1.0.0\\\")\")",
        ] {
            assert!(
                !contains_static_typst_preflight_violation(source),
                "inert runtime-evaluation text was rejected: {source:?}"
            );
        }
    }

    #[test]
    fn logical_path_components_are_not_host_path_markers() {
        let diagnostic = "<typst-build>/project/docs/target/Users/main.typ:1:1";
        assert_eq!(sanitize_absolute_path_tokens(diagnostic), diagnostic);
        assert_eq!(
            sanitize_absolute_path_tokens("/tmp/build/project/docs/target/Users/main.typ:1:1"),
            "/docs/target/Users/main.typ:1:1"
        );
        assert_eq!(
            sanitize_absolute_path_tokens("relative/target/Users/main.typ:1:1"),
            "relative/target/Users/main.typ:1:1"
        );
        assert_eq!(
            sanitize_absolute_path_tokens(r"relative\target\Users\main.typ:1:1"),
            r"relative\target\Users\main.typ:1:1"
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
