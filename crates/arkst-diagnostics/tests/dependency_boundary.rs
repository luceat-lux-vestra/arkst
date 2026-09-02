//! Keep the physical diagnostics crate dependency boundary explicit.

#[test]
fn diagnostics_manifest_has_only_allowed_production_dependencies() {
    let manifest = include_str!("../Cargo.toml");
    let (_, dependencies) = manifest
        .split_once("[dependencies]")
        .expect("diagnostics manifest must declare a dependencies section");
    let dependencies = dependencies
        .split_once("\n[")
        .map_or(dependencies, |(deps, _)| deps);

    for line in dependencies
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let dependency_name = line.split_once('=').map_or(line, |(name, _)| name.trim());
        let dependency_name = dependency_name
            .strip_suffix(".workspace")
            .unwrap_or(dependency_name);
        assert!(
            matches!(dependency_name, "arkst-source" | "serde"),
            "unexpected direct arkst-diagnostics dependency: {line}"
        );
    }
}
