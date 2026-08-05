use anyhow::Context;
use std::fs;
use std::path::{Component, Path, PathBuf};

use scribium_core::virtual_path::VirtualPathBuf;
use scribium_core::VirtualProjectBuilder;

/// Represents a loaded project with both physical and virtual paths.
struct LoadedProject {
    project: scribium_core::VirtualProject,
    physical_entry: PathBuf,
    #[allow(dead_code)]
    project_root: PathBuf,
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
    let physical_entry = input
        .canonicalize()
        .with_context(|| format!("cannot resolve {}", input.display()))?;

    let project_root = physical_entry
        .parent()
        .ok_or_else(|| {
            anyhow::anyhow!(
                "input has no parent directory: {}",
                physical_entry.display()
            )
        })?
        .to_path_buf();

    let relative_entry = physical_entry.strip_prefix(&project_root).map_err(|_| {
        anyhow::anyhow!(
            "input is outside project root: {}",
            physical_entry.display()
        )
    })?;

    let virtual_entry = os_relative_path_to_virtual(relative_entry)?;

    let source = fs::read_to_string(&physical_entry)
        .with_context(|| format!("cannot read {}", physical_entry.display()))?;

    let project = VirtualProjectBuilder::new()
        .entry(virtual_entry.as_str())?
        .add_source(virtual_entry.as_str(), source)?
        .build()?;

    Ok(LoadedProject {
        project,
        physical_entry,
        project_root,
    })
}

/// Reads a single input file and compiles it as a one-source VirtualProject.
fn compile_file(input: &Path) -> anyhow::Result<scribium_core::CompileResult> {
    let loaded = load_single_file_project(input)?;

    let options = scribium_core::CompileOptions {
        compatibility_profile: None,
    };
    Ok(scribium_core::compile(&loaded.project, &options))
}

/// Execute the `build` command: compile input to output format(s).
pub fn build(input: &str, formats: &[String]) -> anyhow::Result<()> {
    let input = Path::new(input);
    let loaded = load_single_file_project(input)?;
    let result = compile_file(&loaded.physical_entry)?;

    for diag in &result.diagnostics {
        eprintln!("{:?}", diag);
    }

    if formats.contains(&"typst".to_string()) {
        let typst_code = scribium_typst::lowering::lower_to_typst_code(&result.ir);
        let out_path = loaded.physical_entry.with_extension("qd.typ");
        fs::write(&out_path, &typst_code)
            .map_err(|e| anyhow::anyhow!("cannot write {}: {}", out_path.display(), e))?;
        eprintln!("Wrote generated Typst to {}", out_path.display());
    }

    // TODO: invoke Typst backend for pdf/html/svg/png
    Ok(())
}

/// Execute the `check` command: validate input without producing output.
pub fn check(input: &str) -> anyhow::Result<()> {
    let result = compile_file(Path::new(input))?;

    let error_count = result.diagnostics.len();
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
    let result = compile_file(Path::new(input))?;

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
