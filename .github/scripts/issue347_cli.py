from pathlib import Path

p = Path("crates/arkst-cli/src/commands.rs")
s = p.read_text()
start = s.index("pub fn build_with_backend_and_libraries(")
end = s.index("/// Returns the default output path for Typst output.", start)
new = r'''pub fn build_with_backend_and_libraries(
    input: &str,
    formats: &[String],
    output: Option<&Path>,
    typst_path: &Path,
    backend: BackendSelection,
    libraries_dir: Option<&Path>,
) -> anyhow::Result<()> {
    const SUPPORTED_FORMATS: &[&str] = &["typst", "pdf", "png", "svg"];

    let unsupported: Vec<&String> = formats
        .iter()
        .filter(|f| !SUPPORTED_FORMATS.contains(&f.as_str()))
        .collect();
    if let Some(format) = unsupported.first() {
        anyhow::bail!(
            "output format '{}' is not yet implemented (supported: typst, pdf, png, svg)",
            format
        );
    }
    if formats.is_empty() {
        anyhow::bail!("no output format requested");
    }
    if output.is_some() && formats.len() != 1 {
        anyhow::bail!(
            "cannot use --output with multiple formats; specify one format or omit --output"
        );
    }

    ensure_backend_available(backend)?;
    if backend == BackendSelection::InProcess {
        if !formats.iter().any(|format| format == "pdf") {
            anyhow::bail!(
                "--backend in-process only applies to PDF output; omit it for typst-only output"
            );
        }
        if formats
            .iter()
            .any(|format| matches!(format.as_str(), "png" | "svg"))
        {
            anyhow::bail!("in-process backend supports only PDF rendered output");
        }
    }

    let input_path = Path::new(input);
    let loaded = load_single_file_project_with_libraries(input_path, libraries_dir)?;
    let result = compile_project(&loaded.project)?;

    for diag in &result.diagnostics {
        eprintln!("{:?}", diag);
    }
    ensure_no_errors(&result.diagnostics)?;

    let (typst_code, source_map) = arkst_typst::lowering::lower_to_typst(&result.ir);

    // Establish every requested base path before spawning a backend. This
    // catches source/output aliases and explicit-output misuse without leaving
    // generated files behind.
    let mut requested_outputs = Vec::with_capacity(formats.len());
    for format in formats {
        let out_path = match output {
            Some(path) => path.to_path_buf(),
            None if format == "typst" => default_typst_output_path(&loaded.requested_entry),
            None => loaded.requested_entry.with_extension(format),
        };
        ensure_distinct_output(&loaded.requested_entry, &out_path)?;
        reject_lexically_colliding_output(&loaded.requested_entry, &out_path)?;
        requested_outputs.push((format.clone(), out_path));
    }

    struct PendingOutput {
        path: PathBuf,
        bytes: Vec<u8>,
        label: String,
    }

    // Render every requested backend target before publishing anything. This
    // is the transaction boundary required for mixed `typst` + rendered
    // builds: backend/preflight failure cannot leave an earlier output file.
    let typst_input = TypstInput {
        source: typst_code.clone(),
        entry_path: loaded.project.entry().as_str().to_string(),
    };
    let mut pending = Vec::<PendingOutput>::new();
    for (format, base_path) in requested_outputs {
        if format == "typst" {
            pending.push(PendingOutput {
                path: base_path,
                bytes: typst_code.as_bytes().to_vec(),
                label: "generated Typst".to_string(),
            });
            continue;
        }

        let target = match format.as_str() {
            "pdf" => TypstTarget::Pdf,
            "png" => TypstTarget::Png,
            "svg" => TypstTarget::Svg,
            _ => unreachable!("validated above"),
        };
        let typst_output = match backend {
            BackendSelection::Subprocess => SubprocessBackend::new(typst_path)
                .with_source_context(loaded.source_context.clone())
                .compile(&typst_input, target)
                .map_err(|error| anyhow::anyhow!("{target} compilation failed: {error}"))?,
            #[cfg(feature = "typst-inprocess")]
            BackendSelection::InProcess => InProcessBackend::new(&loaded.project)
                .compile_with_source_map(&typst_input, &source_map, target)
                .map_err(|error| anyhow::anyhow!("{target} compilation failed: {error}"))?,
            #[cfg(not(feature = "typst-inprocess"))]
            BackendSelection::InProcess => {
                return Err(anyhow::anyhow!(
                    "in-process backend support is not enabled in this build; rebuild with feature `typst-inprocess`"
                ));
            }
        };
        if typst_output.target != target {
            anyhow::bail!(
                "{target} backend returned mismatched target {}",
                typst_output.target
            );
        }
        if typst_output.artifacts.is_empty() {
            anyhow::bail!("{target} backend did not produce output");
        }

        let artifact_count = typst_output.artifacts.len();
        for (index, artifact) in typst_output.artifacts.into_iter().enumerate() {
            let path = if artifact_count == 1 {
                base_path.clone()
            } else {
                numbered_output_path(&base_path, index + 1)?
            };
            pending.push(PendingOutput {
                path,
                bytes: artifact.bytes,
                label: format.to_ascii_uppercase(),
            });
        }
    }

    // Multi-artifact names are known only after rendering. Validate all of
    // them before creating output files, then resolve every effective path and
    // reject aliases/collisions across the whole publication plan.
    for item in &pending {
        ensure_distinct_output(&loaded.requested_entry, &item.path)?;
        reject_lexically_colliding_output(&loaded.requested_entry, &item.path)?;
    }

    let mut resolved = Vec::<PendingOutput>::with_capacity(pending.len());
    for item in pending {
        let resolved_path = resolve_output_path(&item.path)?;
        ensure_distinct_output(&loaded.requested_entry, &resolved_path)?;
        if resolved
            .iter()
            .any(|existing| same_file_paths(&existing.path, &resolved_path))
        {
            anyhow::bail!(
                "output path collision after resolution: '{}'",
                resolved_path.display()
            );
        }
        resolved.push(PendingOutput {
            path: resolved_path,
            bytes: item.bytes,
            label: item.label,
        });
    }

    // Publication is per-file atomic. The transaction guarantee above covers
    // backend and preflight failures; a host filesystem failure during a later
    // rename is not claimed to be crash-atomic across the whole output set.
    for item in resolved {
        write_output_atomically(&item.path, &item.bytes)?;
        eprintln!("Wrote {} to {}", item.label, item.path.display());
    }

    Ok(())
}

fn numbered_output_path(base: &Path, index: usize) -> anyhow::Result<PathBuf> {
    let stem = base.file_stem().ok_or_else(|| {
        anyhow::anyhow!("output path has no file stem: '{}'", base.display())
    })?;
    let mut name = std::ffi::OsString::from(stem);
    name.push(format!("-{index}"));
    if let Some(extension) = base.extension() {
        name.push(".");
        name.push(extension);
    }
    Ok(base.with_file_name(name))
}

'''
s = s[:start] + new + s[end:]

# Existing fake Typst helpers must not depend on argument positions now that
# the subprocess adapter passes --format and, for HTML, feature flags.
s = s.replace(
    '''if [ \"$1\" = \"compile\" ]; then\n  if [ \"$2\" = \"--root\" ]; then output=\"$5\"; else output=\"$3\"; fi\n  printf '%s' '{}' > \"$output\"''',
    '''if [ \"$1\" = \"compile\" ]; then\n  output=\"\"\n  for arg in \"$@\"; do output=\"$arg\"; done\n  printf '%s' '{}' > \"$output\"''',
)
s = s.replace(
    '''if \"%1\"==\"compile\" (\n  if \"%2\"==\"--root\" (\n    copy /B \"{}\" \"%~5\" >nul\n  ) else (\n    copy /B \"{}\" \"%~3\" >nul\n  )\n  exit /b 0\n)''',
    '''if \"%1\"==\"compile\" (\n  for %%A in (%*) do set \"output=%%~A\"\n  copy /B \"{}\" \"%output%\" >nul\n  exit /b 0\n)''',
)
p.write_text(s)
