use std::fs;

/// Execute the `build` command: compile input to output format(s).
pub fn build(input: &str, formats: &[String]) -> anyhow::Result<()> {
    let source =
        fs::read_to_string(input).map_err(|e| anyhow::anyhow!("cannot read {}: {}", input, e))?;

    let options = scribium_core::CompileOptions {
        compatibility_profile: None,
    };
    let result = scribium_core::compile(&source, &options);

    for diag in &result.diagnostics {
        eprintln!("{:?}", diag);
    }

    if formats.contains(&"typst".to_string()) {
        let typst_code = scribium_typst::lowering::lower_to_typst_code(&result.ir);
        let out_path = format!("{}.typ", input);
        fs::write(&out_path, &typst_code)
            .map_err(|e| anyhow::anyhow!("cannot write {}: {}", out_path, e))?;
        eprintln!("Wrote generated Typst to {}", out_path);
    }

    // TODO: invoke Typst backend for pdf/html/svg/png
    Ok(())
}

/// Execute the `check` command: validate input without producing output.
pub fn check(input: &str) -> anyhow::Result<()> {
    let source =
        fs::read_to_string(input).map_err(|e| anyhow::anyhow!("cannot read {}: {}", input, e))?;

    let options = scribium_core::CompileOptions {
        compatibility_profile: None,
    };
    let result = scribium_core::compile(&source, &options);

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
    let source =
        fs::read_to_string(input).map_err(|e| anyhow::anyhow!("cannot read {}: {}", input, e))?;

    let options = scribium_core::CompileOptions {
        compatibility_profile: None,
    };
    let result = scribium_core::compile(&source, &options);

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
