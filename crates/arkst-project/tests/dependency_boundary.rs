//! Keep the physical project crate dependency boundary explicit.

#[test]
fn project_manifest_has_only_platform_neutral_direct_dependencies() {
    let manifest = include_str!("../Cargo.toml");
    let (_, dependencies) = manifest
        .split_once("[dependencies]")
        .expect("project manifest must declare a dependencies section");
    let dependencies = dependencies
        .split_once("\n[")
        .map_or(dependencies, |(deps, _)| deps);

    for line in dependencies
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        assert!(
            line.starts_with("arkst-source") || line.starts_with("thiserror"),
            "unexpected direct arkst-project dependency: {line}"
        );
    }
}
