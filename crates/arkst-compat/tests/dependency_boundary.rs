//! Keep the physical compatibility crate dependency boundary explicit.

#[test]
fn compatibility_manifest_has_no_production_dependencies() {
    let manifest = include_str!("../Cargo.toml");

    assert!(
        !manifest.lines().any(|line| line.trim() == "[dependencies]"),
        "arkst-compat currently has no production dependencies"
    );
    for forbidden in [
        "arkst-core",
        "arkst-project",
        "arkst-markdown",
        "arkst-quarkdown",
        "arkst-engine",
        "arkst-ir",
        "arkst-html",
        "arkst-typst",
        "arkst-cli",
    ] {
        assert!(
            !manifest
                .lines()
                .any(|line| line.trim_start().starts_with(forbidden)),
            "unexpected direct arkst-compat dependency: {forbidden}"
        );
    }
}
