use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn production_dependency_names(manifest: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut in_dependencies = false;

    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_dependencies = line == "[dependencies]";
            continue;
        }
        if !in_dependencies || line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some((name, _)) = line.split_once('=') {
            let name = name
                .trim()
                .strip_suffix(".workspace")
                .unwrap_or(name.trim());
            names.insert(name.to_string());
        }
    }

    names
}

fn rust_sources(root: &Path, sources: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_sources(&path, sources);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            sources.push(path);
        }
    }
}

#[test]
fn production_manifest_has_no_upward_or_native_dependencies() {
    let manifest = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
        .expect("read arkst-typst manifest");
    let dependencies = production_dependency_names(&manifest);
    let forbidden = [
        "arkst-core",
        "arkst-project",
        "arkst-engine",
        "arkst-typst-subprocess",
        "arkst-cli",
        "arkst-test-support",
        "tempfile",
    ];

    for dependency in forbidden {
        assert!(
            !dependencies.contains(dependency),
            "arkst-typst production dependencies must not include {dependency}: {dependencies:?}"
        );
    }
}

#[test]
fn production_sources_contain_no_native_execution_apis() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut sources = Vec::new();
    rust_sources(&source_root, &mut sources);
    assert!(!sources.is_empty(), "expected production Rust sources");

    let forbidden = [
        "std::fs",
        "std::process",
        "std::path::PathBuf",
        "tempfile::",
    ];
    for source in sources {
        let contents = fs::read_to_string(&source).expect("read production source");
        for needle in forbidden {
            assert!(
                !contents.contains(needle),
                "{needle} must not appear in pure arkst-typst production source: {}",
                source.display()
            );
        }
    }
}
