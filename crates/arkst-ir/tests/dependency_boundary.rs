//! Keep the physical IR crate dependency boundary explicit.

#[test]
fn ir_manifest_has_only_allowed_production_dependencies() {
    let manifest = include_str!("../Cargo.toml");
    let (_, dependencies) = manifest
        .split_once("[dependencies]")
        .expect("IR manifest must declare a dependencies section");
    let dependencies = dependencies
        .split_once("\n[")
        .map_or(dependencies, |(deps, _)| deps);

    let dependency_names = dependencies
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| line.split_once('=').map_or(line, |(name, _)| name.trim()))
        .map(|name| name.strip_suffix(".workspace").unwrap_or(name))
        .collect::<std::collections::BTreeSet<_>>();

    assert!(
        dependency_names.contains("arkst-source"),
        "arkst-ir must depend on arkst-source for SourceSpan"
    );
    assert!(dependency_names.contains("serde"));
    assert_eq!(
        dependency_names.len(),
        2,
        "arkst-ir must not acquire upward Arkst production dependencies"
    );
}
