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
        "scribium-core",
        "scribium-project",
        "scribium-typst",
        "scribium-cli",
        "scribium-test-support",
    ] {
        assert!(
            !dependencies.contains(forbidden),
            "scribium-engine must not depend on {forbidden}"
        );
    }
}

#[test]
fn engine_manifest_declares_its_lower_layer_inputs() {
    let dependencies = production_dependencies();
    for required in [
        "scribium-compat",
        "scribium-diagnostics",
        "scribium-ir",
        "scribium-markdown",
        "scribium-quarkdown",
        "scribium-source",
    ] {
        assert!(
            dependencies.contains(required),
            "scribium-engine must declare {required} as a production dependency"
        );
    }
}
