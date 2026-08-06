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
/// Loads a single file as a VirtualProject.
fn load_single_file_project(input: &Path) -> anyhow::Result<LoadedProject> {
    // Store the user-requested path for output naming
    let requested_entry = input.to_path_buf();

    let physical_entry = input
        .canonicalize()
        .with_context(|| format!("cannot resolve {}", input.display()))?;

    // Project root is based on requested path (logical root)
    let logical_project_root = requested_entry
        .parent()
        .ok_or_else(|| {
            anyhow::anyhow!(
                "input has no parent directory: {}",
                requested_entry.display()
            )
        })?
        .to_path_buf();

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
    let requested_relative = requested_entry
        .strip_prefix(&logical_project_root)
        .map_err(|_| {
            anyhow::anyhow!(
                "input is outside project root: {}",
                requested_entry.display()
            )
        })?;
    let virtual_entry = os_relative_path_to_virtual(requested_relative)?;

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
pub fn build(input: &str, formats: &[String]) -> anyhow::Result<()> {
    let input = Path::new(input);
    let loaded = load_single_file_project(input)?;
    let result = compile_project(&loaded.project)?;

    for diag in &result.diagnostics {
        eprintln!("{:?}", diag);
    }

    // Fail on error diagnostics before writing output
    ensure_no_errors(&result.diagnostics)?;

    if formats.iter().any(|f| f == "typst") {
        let typst_code = scribium_typst::lowering::lower_to_typst_code(&result.ir);
        let out_path = default_typst_output_path(&loaded.requested_entry);
        fs::write(&out_path, &typst_code)
            .map_err(|e| anyhow::anyhow!("cannot write {}: {}", out_path.display(), e))?;
        eprintln!("Wrote generated Typst to {}", out_path.display());
    }

    // TODO: invoke Typst backend for pdf/html/svg/png
    Ok(())
}

/// Returns the default output path for Typst output.
/// Replaces the extension with `.typ`.
fn default_typst_output_path(requested_entry: &Path) -> PathBuf {
    requested_entry.with_extension("typ")
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
        let result = build(&link_file.to_string_lossy(), &["typst".to_string()]);
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
        let result = build(&link_file.to_string_lossy(), &["typst".to_string()]);
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
}
