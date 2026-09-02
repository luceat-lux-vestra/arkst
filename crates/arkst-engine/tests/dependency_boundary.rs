//! Keep the semantic engine physically below orchestration and project crates.

use std::collections::BTreeSet;

fn production_dependencies() -> BTreeSet<&'static str> {
    let manifest = include_str!("../Cargo.toml");
    let (_, dependencies) = manifest
        .split_once("[dependencies]")
        .expect("engine manifest must declare a dependencies section");
    let dependencies = dependencies
        .split_once("\n[")
        .map_or(dependencies, |(deps, _)| deps);
    dependencies
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| line.split_once('=').map_or(line, |(name, _)| name.trim()))
        .map(|name| name.strip_suffix(".workspace").unwrap_or(name))
        .collect()
}

#[test]
fn engine_has_no_forbidden_upward_or_backend_dependencies() {
    let dependencies = production_dependencies();
    for forbidden in [
        "arkst-core",
        "arkst-project",
        "arkst-typst",
        "arkst-cli",
        "arkst-test-support",
    ] {
        assert!(
            !dependencies.contains(forbidden),
            "arkst-engine must not depend on {forbidden}"
        );
    }
}

#[test]
fn engine_manifest_declares_its_lower_layer_inputs() {
    let dependencies = production_dependencies();
    for required in [
        "arkst-compat",
        "arkst-diagnostics",
        "arkst-ir",
        "arkst-markdown",
        "arkst-quarkdown",
        "arkst-source",
    ] {
        assert!(
            dependencies.contains(required),
            "arkst-engine must declare {required} as a production dependency"
        );
    }
}
