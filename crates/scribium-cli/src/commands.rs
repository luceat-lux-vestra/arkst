use anyhow::Context;
use std::fs;
use std::path::{Component, Path, PathBuf};

use scribium_core::diagnostics::Severity;
use scribium_core::virtual_path::VirtualPathBuf;
use scribium_core::VirtualProjectBuilder;

/// Represents a loaded project with both physical and virtual paths.
struct LoadedProject {
    project: scribium_core::VirtualProject,
    /// The path as requested by the user (logical path for output naming)
    requested_entry: PathBuf,
    /// The canonicalized, resolved path (for file reading)
    #[allow(dead_code)]
    physical_entry: PathBuf,
}
/// Converts an OS-relative path to a VirtualPathBuf.
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

    // Project root is based on requested path (to handle symlinks correctly)
    let project_root = requested_entry
        .parent()
        .ok_or_else(|| {
            anyhow::anyhow!(
                "input has no parent directory: {}",
                requested_entry.display()
            )
        })?
        .to_path_buf();

    // Compute logical virtual entry from requested path (not canonicalized)
    let requested_relative = requested_entry.strip_prefix(&project_root).map_err(|_| {
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
        physical_entry,
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

/// Execute the `build` command: compile input to output format(s).
pub fn build(input: &str, formats: &[String]) -> anyhow::Result<()> {
    let input = Path::new(input);
    let loaded = load_single_file_project(input)?;
    let result = compile_project(&loaded.project)?;

    for diag in &result.diagnostics {
        eprintln!("{:?}", diag);
    }

    if formats.iter().any(|f| f == "typst") {
        let typst_code = scribium_typst::lowering::lower_to_typst_code(&result.ir);
        let out_path = loaded.requested_entry.with_extension("qd.typ");
        fs::write(&out_path, &typst_code)
            .map_err(|e| anyhow::anyhow!("cannot write {}: {}", out_path.display(), e))?;
        eprintln!("Wrote generated Typst to {}", out_path.display());
    }

    // TODO: invoke Typst backend for pdf/html/svg/png
    Ok(())
}

/// Execute the `check` command: validate input without producing output.
pub fn check(input: &str) -> anyhow::Result<()> {
    let input = Path::new(input);
    let loaded = load_single_file_project(input)?;
    let result = compile_project(&loaded.project)?;

    let error_count = result
        .diagnostics
        .iter()
        .filter(|d| matches!(&d.severity, Severity::Error))
        .count();

    for diag in &result.diagnostics {
        eprintln!("{:?}", diag);
    }

    if error_count > 0 {
        anyhow::bail!("found {} error(s)", error_count);
    }

    Ok(())
}

/// Execute the `inspect` command: show intermediate representation(s).
pub fn inspect(input: &str, emit: &str) -> anyhow::Result<()> {
    let input = Path::new(input);
    let loaded = load_single_file_project(input)?;
    let result = compile_project(&loaded.project)?;

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
        let external_dir = dir.path().join("external");
        fs::create_dir(&link_dir).unwrap();
        fs::create_dir(&external_dir).unwrap();

        // Create real file
        let real_file = external_dir.join("real.qd");
        fs::write(&real_file, "---\ntitle: Symlink Test\n---\n\n# Hello\n").unwrap();

        // Create symlink
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

        // Output should be at link_dir/link.qd.typ (logical path)
        let expected_output = link_file.with_extension("qd.typ");
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
        // Ensure no output at external/real.qd.typ
        let unexpected_output = real_file.with_extension("qd.typ");
        assert!(
            !unexpected_output.exists(),
            "output should not be at resolved path: {:?}",
            unexpected_output
        );
    }
}
