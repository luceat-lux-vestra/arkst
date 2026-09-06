//! Executable evidence guard for Issue #298's pinned native-ingestion research slice.

const RESEARCH: &str = include_str!("../../../docs/research/quarkdown-native-ingestion-298.md");
const CLI_MAIN: &str = include_str!("../src/main.rs");
const CLI_COMMANDS: &str = include_str!("../src/commands.rs");
const VIRTUAL_PROJECT: &str = include_str!("../../arkst-project/src/virtual_project.rs");
const TARGET_SHA: &str = "107ec3a9482f10d6f90d7580f8409b46a719d18e";

fn normalized_research() -> String {
    RESEARCH.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn pinned_upstream_native_ingestion_contract_is_explicit() {
    assert!(RESEARCH.contains(TARGET_SHA));

    for source in [
        "ExecuteCommand.kt",
        "Execute.kt",
        "QdLibraries.kt",
        "PipelineInitialization.kt",
        "QdLibraryExporter.kt",
        "importing-external-libraries.qd",
    ] {
        assert!(RESEARCH.contains(source), "missing pinned source: {source}");
    }

    for contract in [
        "`-l <dir>` / `--libs <dir>`",
        "`<install directory>/lib/qd`",
        "`File.listFiles()`",
        "exactly lowercase `qd`",
        "`nameWithoutExtension`",
        "warning plus silent empty",
        "preconstructed loadable-library set",
    ] {
        assert!(
            RESEARCH.contains(contract),
            "missing upstream contract: {contract}"
        );
    }
}

#[test]
fn current_cli_project_ingestion_is_explicit_and_deterministic() {
    for contract in [
        "fn load_single_file_project(input: &Path)",
        "collect_project_files(&canonical_project_root, &canonical_project_root, &mut files)",
        "entries.sort_by_key(|entry| entry.file_name());",
        "fs::symlink_metadata(&path)",
        "if !physical_entry.starts_with(&canonical_project_root)",
        "VirtualProjectBuilder::new().entry(virtual_entry.as_str())?",
        "builder = builder.add_source(path.as_str(), source)?;",
        "builder = builder.add_asset(path.as_str(), bytes)?;",
        "compile_project(&loaded.project)",
    ] {
        assert!(
            CLI_COMMANDS.contains(contract),
            "missing CLI contract: {contract}"
        );
    }

    for builder_contract in [
        "pub fn add_loadable_library(",
        "libraries.sort_by(|a, b| a.0.cmp(&b.0));",
        "BuildError::DuplicateLoadableLibrary",
        "BuildError::EmptyLoadableLibraryName",
    ] {
        assert!(
            VIRTUAL_PROJECT.contains(builder_contract),
            "missing VirtualProject contract: {builder_contract}"
        );
    }
}

#[test]
fn evidence_slice_keeps_native_library_ingestion_gap_explicit() {
    assert!(!CLI_MAIN.contains("--libs"));
    assert!(!CLI_COMMANDS.contains(".add_loadable_library("));

    let research = normalized_research();
    for policy in [
        "explicit trusted-CLI authority",
        "does **not** implicitly probe a global installation library directory",
        "fail-fast CLI error",
        "non-recursive",
        "ordered by filename before ingestion",
        "eagerly read as UTF-8",
        "symlink outside the explicitly supplied library directory is rejected",
        "`build`, `check`, and `inspect`",
        "Document content cannot select or widen the host library directory",
    ] {
        assert!(
            research.contains(policy),
            "missing accepted policy: {policy}"
        );
    }

    assert!(research.contains("The next bounded #298 slice should implement"));
    assert!(research.contains("Canonical #155 audit ownership remains #298"));
}
