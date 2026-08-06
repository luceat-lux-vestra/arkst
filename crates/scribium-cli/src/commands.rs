use anyhow::Context;
use std::fs;
use std::path::{Component, Path, PathBuf};

use scribium_core::virtual_path::VirtualPathBuf;
use scribium_core::VirtualProjectBuilder;

/// Represents a loaded project with both physical and virtual paths.
struct LoadedProject {
    project: scribium_core::VirtualProject,
    /// The path as requested by the user (logical path for output naming)
    requested_entry: PathBuf,
}
fn os_relative_path_to_virtual(path: &Path) -> anyhow::Result<VirtualPathBuf> {
    let mut components = Vec::new();

    for component in path.components() {
        match component {
            Component::Normal(value) => {
                let value = value.to_str().ok_or_else(|| {
                    anyhow::anyhow!("path is not valid UTF-8: {}", path.display())
                })?;

                components.push(value);
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                anyhow::bail!("path is not project-relative: {}", path.display());
            }
        }
    }

    VirtualPathBuf::parse(components.join("/")).map_err(Into::into)
}

/// Returns the logical project root for an input path.
///
/// The project root is the parent directory of the requested entry. When the
/// request is a bare file name (e.g. `document.qd`) the parent is empty, and
/// the current directory `"."` is used instead. Returned for relative and
/// absolute paths alike; the caller decides how to resolve it.
fn logical_project_root(requested_entry: &Path) -> PathBuf {
    requested_entry
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf()
}

/// Supported input extensions. Typst passthrough (`.typ`) is not implemented
/// yet, so it is deliberately excluded and rejected at the CLI boundary.
const SUPPORTED_INPUT_EXTENSIONS: [&str; 3] = ["qd", "scrib", "md"];

/// Validates that `input` has a supported source extension.
fn validate_input_extension(input: &Path) -> anyhow::Result<()> {
    let ext = input
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default();
    if SUPPORTED_INPUT_EXTENSIONS.contains(&ext) {
        Ok(())
    } else {
        anyhow::bail!(
            "unsupported input format: '{}' (supported: .qd, .scrib, .md)",
            input.display()
        )
    }
}

/// Loads a single file as a VirtualProject.
fn load_single_file_project(input: &Path) -> anyhow::Result<LoadedProject> {
    validate_input_extension(input)?;
    // Store the user-requested path for output naming
    let requested_entry = input.to_path_buf();

    let physical_entry = input
        .canonicalize()
        .with_context(|| format!("cannot resolve {}", input.display()))?;

    // Project root is based on requested path (logical root)
    let logical_project_root = logical_project_root(&requested_entry);

    // Canonicalized project root for symlink containment check
    let canonical_project_root = logical_project_root.canonicalize().with_context(|| {
        format!(
            "cannot resolve project root {}",
            logical_project_root.display()
        )
    })?;

    // Verify the physical entry is within the canonical project root (symlink escape check)
    if !physical_entry.starts_with(&canonical_project_root) {
        return Err(anyhow::anyhow!(
            "input file '{}' resolves to '{}' which is outside project root '{}' (symlink escape)",
            requested_entry.display(),
            physical_entry.display(),
            canonical_project_root.display()
        ));
    }

    // Compute logical virtual entry from requested path (not canonicalized)
    let requested_relative = if requested_entry
        .parent()
        .map(|parent| parent.as_os_str().is_empty())
        .unwrap_or(false)
    {
        // Bare file name: the logical root is "." which has no path components,
        // so strip_prefix would fail. Use the file name directly.
        PathBuf::from(requested_entry.file_name().ok_or_else(|| {
            anyhow::anyhow!("input has no file name: {}", requested_entry.display())
        })?)
    } else {
        requested_entry
            .strip_prefix(&logical_project_root)
            .map_err(|_| {
                anyhow::anyhow!(
                    "input is outside project root: {}",
                    requested_entry.display()
                )
            })?
            .to_path_buf()
    };
    let virtual_entry = os_relative_path_to_virtual(&requested_relative)?;

    let source = fs::read_to_string(&physical_entry)
        .with_context(|| format!("cannot read {}", physical_entry.display()))?;

    let project = VirtualProjectBuilder::new()
        .entry(virtual_entry.as_str())?
        .add_source(virtual_entry.as_str(), source)?
        .build()?;

    Ok(LoadedProject {
        project,
        requested_entry,
    })
}

/// Compiles a pre-loaded VirtualProject.
fn compile_project(
    project: &scribium_core::VirtualProject,
) -> anyhow::Result<scribium_core::CompileResult> {
    let options = scribium_core::CompileOptions {
        compatibility_profile: None,
    };
    Ok(scribium_core::compile(project, &options))
}
/// Returns an error if any diagnostic has Severity::Error.
fn ensure_no_errors(diagnostics: &[scribium_core::Diagnostic]) -> anyhow::Result<()> {
    let error_count = diagnostics
        .iter()
        .filter(|d| matches!(&d.severity, scribium_core::Severity::Error))
        .count();
    if error_count > 0 {
        anyhow::bail!("found {} error(s)", error_count);
    }
    Ok(())
}

/// Execute the `build` command: compile input to output format(s).
pub fn build(input: &str, formats: &[String], output: Option<&Path>) -> anyhow::Result<()> {
    let unsupported: Vec<&String> = formats.iter().filter(|f| f.as_str() != "typst").collect();
    if let Some(format) = unsupported.first() {
        anyhow::bail!(
            "output format '{}' is not yet implemented (supported: typst)",
            format
        );
    }
    if !formats.iter().any(|f| f.as_str() == "typst") {
        anyhow::bail!("no writable output format requested");
    }

    let input = Path::new(input);
    let loaded = load_single_file_project(input)?;

    let out_path = match output {
        Some(path) => path.to_path_buf(),
        None => default_typst_output_path(&loaded.requested_entry),
    };
    ensure_distinct_output(&loaded.requested_entry, &out_path)?;

    let result = compile_project(&loaded.project)?;

    for diag in &result.diagnostics {
        eprintln!("{:?}", diag);
    }

    // Fail on error diagnostics before writing output
    ensure_no_errors(&result.diagnostics)?;

    // Re-verify immediately before writing: the output path may have appeared
    // or been replaced since the initial check (TOCTOU window).
    ensure_distinct_output(&loaded.requested_entry, &out_path)?;

    let typst_code = scribium_typst::lowering::lower_to_typst_code(&result.ir);
    fs::write(&out_path, &typst_code)
        .map_err(|e| anyhow::anyhow!("cannot write {}: {}", out_path.display(), e))?;
    eprintln!("Wrote generated Typst to {}", out_path.display());

    // TODO: invoke Typst backend for pdf/html/svg/png
    Ok(())
}

/// Returns the default output path for Typst output.
/// Replaces the extension with `.typ`.
fn default_typst_output_path(requested_entry: &Path) -> PathBuf {
    requested_entry.with_extension("typ")
}

/// Bails when `output` refers to the same file as `input`.
fn ensure_distinct_output(input: &Path, output: &Path) -> anyhow::Result<()> {
    if same_file_paths(input, output) {
        anyhow::bail!(
            "refusing to overwrite the input source file: input '{}' maps to output '{}'",
            input.display(),
            output.display()
        );
    }
    Ok(())
}

/// Returns whether two paths refer to the same file.
///
/// When the output already exists, real file identity is compared via
/// `same-file` (device/inode on Unix, file index on Windows): this detects
/// hard links and symlinks that alias the input, whatever the path spelling.
/// When the output does not exist, the parent directory of each path is
/// canonicalized (the input parent always exists) and the file names are
/// compared, which resolves `.`/`..`/relative forms without requiring the
/// output to exist. A dangling symlink is resolved manually so that a link
/// pointing at the input is still detected.
fn same_file_paths(a: &Path, b: &Path) -> bool {
    // Output exists: compare actual file identity.
    if b.exists() {
        return same_file::is_same_file(a, b).unwrap_or(false);
    }
    // A dangling symlink still creates a directory entry; writing through it
    // would create the link target. Resolve the link and compare against the
    // input before falling back to path comparison.
    if let Ok(meta) = fs::symlink_metadata(b) {
        if meta.file_type().is_symlink() {
            if let Ok(target) = fs::read_link(b) {
                let resolved = if target.is_absolute() {
                    target
                } else {
                    b.parent()
                        .filter(|p| !p.as_os_str().is_empty())
                        .unwrap_or_else(|| Path::new("."))
                        .join(target)
                };
                return same_file::is_same_file(a, &resolved).unwrap_or(false);
            }
        }
    }
    // Output does not exist: normalize the parent directories and compare the
    // file names.
    match (canonical_parent(a), canonical_parent(b)) {
        (Some(parent_a), Some(parent_b)) => {
            parent_a == parent_b && same_file_name(a.file_name(), b.file_name())
        }
        _ => false,
    }
}

/// Compares file names for the not-yet-existing output case.
///
/// Windows filesystems are case-insensitive, so two names differing only in
/// case would still collide there; other platforms compare byte-exact.
#[cfg(windows)]
fn same_file_name(a: Option<&std::ffi::OsStr>, b: Option<&std::ffi::OsStr>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a
            .to_string_lossy()
            .eq_ignore_ascii_case(&b.to_string_lossy()),
        _ => false,
    }
}

#[cfg(not(windows))]
fn same_file_name(a: Option<&std::ffi::OsStr>, b: Option<&std::ffi::OsStr>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

/// Canonicalizes the parent directory of `path`, treating an empty parent as
/// the current directory. Returns `None` when the parent cannot be resolved.
fn canonical_parent(path: &Path) -> Option<PathBuf> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    parent.canonicalize().ok()
}

/// Execute the `check` command: validate input without producing output.
pub fn check(input: &str) -> anyhow::Result<()> {
    let input = Path::new(input);
    let loaded = load_single_file_project(input)?;
    let result = compile_project(&loaded.project)?;

    for diag in &result.diagnostics {
        eprintln!("{:?}", diag);
    }

    ensure_no_errors(&result.diagnostics)?;

    Ok(())
}

/// Execute the `inspect` command: show intermediate representation(s).
pub fn inspect(input: &str, emit: &str) -> anyhow::Result<()> {
    let input = Path::new(input);
    let loaded = load_single_file_project(input)?;
    let result = compile_project(&loaded.project)?;

    // Fail on error diagnostics
    ensure_no_errors(&result.diagnostics)?;

    match emit {
        "typst" => {
            let typst_code = scribium_typst::lowering::lower_to_typst_code(&result.ir);
            println!("{}", typst_code);
        }
        "ir" => {
            let json =
                serde_json::to_string_pretty(&result.ir).map_err(|e| anyhow::anyhow!("{}", e))?;
            println!("{}", json);
        }
        "ast" | "semantic" | "source-map" => {
            println!("[{} emit not yet implemented]", emit);
        }
        _ => anyhow::bail!("unknown emit target: {}", emit),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    #[cfg(unix)]
    fn symlink_input_preserves_logical_output_path() {
        use std::os::unix::fs::symlink;
        let dir = tempdir().unwrap();
        let link_dir = dir.path().join("link_dir");
        fs::create_dir(&link_dir).unwrap();

        // Create real file inside project root
        let real_file = link_dir.join("real.qd");
        fs::write(&real_file, "---\ntitle: Symlink Test\n---\n\n# Hello\n").unwrap();

        // Create symlink inside project root pointing to file inside project root
        let link_file = link_dir.join("link.qd");
        symlink(&real_file, &link_file).unwrap();

        // Build through CLI using the symlink path
        let result = build(&link_file.to_string_lossy(), &["typst".to_string()], None);
        assert!(result.is_ok(), "Build failed: {:?}", result);

        // Verify VirtualProject entry is logical path
        let loaded = load_single_file_project(&link_file).unwrap();
        assert_eq!(loaded.project.entry().as_str(), "link.qd");

        // Verify source store entry
        let entry = loaded.project.entry();
        let source_id = loaded
            .project
            .sources()
            .get_id(entry)
            .expect("entry source must exist");

        assert_eq!(
            loaded.project.sources().path_by_id(source_id).unwrap(),
            entry
        );

        // Output should be at link_dir/link.typ (logical path)
        let expected_output = default_typst_output_path(&link_file);
        assert!(
            expected_output.exists(),
            "output file should exist at logical path: {:?}",
            expected_output
        );

        // Verify content
        let content = fs::read_to_string(&expected_output).unwrap();
        assert!(
            content.contains("Title: Symlink Test"),
            "content was: {}",
            content
        );
        assert!(content.contains("= Hello"), "content was: {}", content);
    }

    #[test]
    #[cfg(unix)]
    fn symlink_outside_project_root_is_rejected() {
        use std::os::unix::fs::symlink;
        let dir = tempdir().unwrap();
        let link_dir = dir.path().join("link_dir");
        let external_dir = dir.path().join("external");
        fs::create_dir(&link_dir).unwrap();
        fs::create_dir(&external_dir).unwrap();

        // Create real file outside project root
        let real_file = external_dir.join("real.qd");
        fs::write(&real_file, "---\ntitle: Symlink Test\n---\n\n# Hello\n").unwrap();

        // Create symlink inside project root pointing outside
        let link_file = link_dir.join("link.qd");
        symlink(&real_file, &link_file).unwrap();

        // Build through CLI using the symlink path - should fail
        let result = build(&link_file.to_string_lossy(), &["typst".to_string()], None);
        assert!(result.is_err());
        let error = result.unwrap_err().to_string();
        assert!(error.contains("symlink escape"));
        assert!(error.contains("outside project root"));

        // Ensure no output file was created
        let unexpected_output = default_typst_output_path(&link_file);
        assert!(!unexpected_output.exists());
    }

    #[test]
    fn output_path_qd_extension() {
        let input = Path::new("document.qd");
        let out = default_typst_output_path(input);
        assert_eq!(out.to_str(), Some("document.typ"));
    }

    #[test]
    fn output_path_no_extension() {
        let input = Path::new("document");
        let out = default_typst_output_path(input);
        assert_eq!(out.to_str(), Some("document.typ"));
    }

    #[test]
    fn output_path_multiple_dots() {
        let input = Path::new("chapter.en.qd");
        let out = default_typst_output_path(input);
        assert_eq!(out.to_str(), Some("chapter.en.typ"));
    }

    #[test]
    fn output_path_hidden_file() {
        let input = Path::new(".hidden");
        let out = default_typst_output_path(input);
        assert_eq!(out.to_str(), Some(".hidden.typ"));
    }

    #[test]
    fn output_path_subdirectory() {
        let input = Path::new("src/main.qd");
        let out = default_typst_output_path(input);
        assert_eq!(out.to_str(), Some("src/main.typ"));
    }
    #[test]
    fn ensure_no_errors_fails_on_error() {
        let diagnostics = vec![scribium_core::Diagnostic {
            code: "E0001".to_string(),
            severity: scribium_core::Severity::Error,
            message: "Test error".to_string(),
            primary: None,
            secondary: vec![],
            hints: vec![],
        }];
        let result = ensure_no_errors(&diagnostics);
        assert!(result.is_err());
        let error = result.unwrap_err().to_string();
        assert!(error.contains("1 error"));
    }

    #[test]
    fn ensure_no_errors_passes_on_warning() {
        let diagnostics = vec![scribium_core::Diagnostic {
            code: "W0001".to_string(),
            severity: scribium_core::Severity::Warning,
            message: "Test warning".to_string(),
            primary: None,
            secondary: vec![],
            hints: vec![],
        }];
        let result = ensure_no_errors(&diagnostics);
        assert!(result.is_ok());
    }

    #[test]
    fn ensure_no_errors_passes_on_empty() {
        let diagnostics: Vec<scribium_core::Diagnostic> = vec![];
        let result = ensure_no_errors(&diagnostics);
        assert!(result.is_ok());
    }

    #[test]
    fn logical_root_for_bare_filename() {
        assert_eq!(
            logical_project_root(Path::new("document.qd")),
            PathBuf::from(".")
        );
    }

    #[test]
    fn logical_root_for_dot_prefixed_filename() {
        assert_eq!(
            logical_project_root(Path::new("./document.qd")),
            PathBuf::from(".")
        );
    }

    #[test]
    fn logical_root_for_nested_directory() {
        assert_eq!(
            logical_project_root(Path::new("docs/document.qd")),
            PathBuf::from("docs")
        );
    }

    #[test]
    fn logical_root_for_absolute_path() {
        assert_eq!(
            logical_project_root(Path::new("/abs/dir/document.qd")),
            PathBuf::from("/abs/dir")
        );
    }

    #[test]
    fn same_file_paths_relative_vs_absolute() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("document.qd");
        fs::write(&input, "# Hello\n").unwrap();

        // Same file: one path starts with a `.` parent, the other is absolute.
        let dotted = dir.path().join(".").join("document.qd");
        assert!(same_file_paths(&input, &dotted));
        assert!(same_file_paths(&input, &input));
        assert!(!same_file_paths(&input, &dir.path().join("document.md")));
    }

    #[test]
    fn same_file_paths_with_dotdot_components() {
        let dir = tempdir().unwrap();
        let sub = dir.path().join("sub");
        fs::create_dir(&sub).unwrap();
        let input = dir.path().join("document.qd");
        fs::write(&input, "# Hello\n").unwrap();

        // `sub/..` resolves to the input's directory, so both paths are the same file.
        let dotdot = sub.join("..").join("document.qd");
        assert!(same_file_paths(&input, &dotdot));
    }

    #[test]
    fn same_file_paths_nonexistent_output_is_never_the_input() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("document.qd");
        fs::write(&input, "# Hello\n").unwrap();

        // Output does not exist: same directory, different name is a different file.
        assert!(!same_file_paths(&input, &dir.path().join("out.typ")));
        // Same name in a different (existing) directory is a different file.
        let other = dir.path().join("other");
        fs::create_dir(&other).unwrap();
        assert!(!same_file_paths(&input, &other.join("document.qd")));
    }

    #[test]
    fn typ_input_is_rejected_as_unsupported_format() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("document.typ");
        fs::write(&input, "# Hello\n").unwrap();

        let result = build(&input.to_string_lossy(), &["typst".to_string()], None);
        assert!(result.is_err());
        let error = result.unwrap_err().to_string();
        assert!(
            error.contains("unsupported input format"),
            "error was: {}",
            error
        );
        assert!(error.contains(".qd, .scrib, .md"), "error was: {}", error);
    }

    #[test]
    fn explicit_output_equal_to_input_is_rejected() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("document.qd");
        fs::write(&input, "# Hello\n").unwrap();

        let result = build(
            &input.to_string_lossy(),
            &["typst".to_string()],
            Some(&input),
        );
        assert!(result.is_err());
        let error = result.unwrap_err().to_string();
        assert!(
            error.contains("refusing to overwrite the input source file"),
            "error was: {}",
            error
        );
    }

    #[test]
    fn dotdot_output_equal_to_input_is_rejected() {
        let dir = tempdir().unwrap();
        let sub = dir.path().join("sub");
        fs::create_dir(&sub).unwrap();
        let input = dir.path().join("document.qd");
        fs::write(&input, "# Hello\n").unwrap();
        let before = fs::read(&input).unwrap();

        let output = sub.join("..").join("document.qd");
        let result = build(
            &input.to_string_lossy(),
            &["typst".to_string()],
            Some(&output),
        );
        assert!(result.is_err());
        let error = result.unwrap_err().to_string();
        assert!(
            error.contains("refusing to overwrite the input source file"),
            "error was: {}",
            error
        );
        assert_eq!(
            fs::read(&input).unwrap(),
            before,
            "input bytes must not change"
        );
    }

    #[test]
    #[cfg(unix)]
    fn output_symlink_to_input_is_rejected() {
        use std::os::unix::fs::symlink;
        let dir = tempdir().unwrap();
        let input = dir.path().join("document.qd");
        fs::write(&input, "# Hello\n").unwrap();
        let before = fs::read(&input).unwrap();

        // The output path is a symlink pointing at the input file.
        let output = dir.path().join("out.typ");
        symlink(&input, &output).unwrap();

        let result = build(
            &input.to_string_lossy(),
            &["typst".to_string()],
            Some(&output),
        );
        assert!(result.is_err());
        let error = result.unwrap_err().to_string();
        assert!(
            error.contains("refusing to overwrite the input source file"),
            "error was: {}",
            error
        );
        assert_eq!(
            fs::read(&input).unwrap(),
            before,
            "input bytes must not change"
        );
    }

    #[test]
    fn output_hardlink_to_input_is_rejected() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("document.qd");
        fs::write(&input, "# Hello\n").unwrap();
        let before = fs::read(&input).unwrap();

        // The output path is a hard link to the input file (same inode).
        let output = dir.path().join("out.typ");
        fs::hard_link(&input, &output).unwrap();

        let result = build(
            &input.to_string_lossy(),
            &["typst".to_string()],
            Some(&output),
        );
        assert!(result.is_err());
        let error = result.unwrap_err().to_string();
        assert!(
            error.contains("refusing to overwrite the input source file"),
            "error was: {}",
            error
        );
        assert_eq!(
            fs::read(&input).unwrap(),
            before,
            "input bytes must not change"
        );
    }

    #[test]
    fn rejected_build_does_not_modify_input() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("document.qd");
        fs::write(&input, "original source\n").unwrap();
        let before = fs::read(&input).unwrap();

        let result = build(
            &input.to_string_lossy(),
            &["typst".to_string()],
            Some(&input),
        );
        assert!(result.is_err());

        let after = fs::read(&input).unwrap();
        assert_eq!(before, after, "input bytes must not change on rejection");
    }

    #[test]
    fn qd_input_defaults_to_typ_output() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("document.qd");
        fs::write(&input, "# Hello\n").unwrap();

        let result = build(&input.to_string_lossy(), &["typst".to_string()], None);
        assert!(result.is_ok(), "Build failed: {:?}", result);

        let expected = dir.path().join("document.typ");
        assert!(expected.exists(), "expected output {:?} to exist", expected);
    }

    #[test]
    fn nonexistent_sibling_output_is_written() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("document.qd");
        fs::write(&input, "# Hello\n").unwrap();

        let output = dir.path().join("out.typ");
        assert!(!output.exists());
        let result = build(
            &input.to_string_lossy(),
            &["typst".to_string()],
            Some(&output),
        );
        assert!(result.is_ok(), "Build failed: {:?}", result);
        assert!(output.exists(), "expected output {:?} to exist", output);
    }
}
