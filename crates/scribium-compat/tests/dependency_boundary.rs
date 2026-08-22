//! Keep the physical compatibility crate dependency boundary explicit.

#[test]
fn compatibility_manifest_has_no_production_dependencies() {
    let manifest = include_str!("../Cargo.toml");

    assert!(
        !manifest.lines().any(|line| line.trim() == "[dependencies]"),
        "scribium-compat currently has no production dependencies"
    );
    for forbidden in [
        "scribium-core",
        "scribium-project",
        "scribium-markdown",
        "scribium-quarkdown",
        "scribium-engine",
        "scribium-ir",
        "scribium-html",
        "scribium-typst",
        "scribium-cli",
    ] {
        assert!(
            !manifest
                .lines()
                .any(|line| line.trim_start().starts_with(forbidden)),
            "unexpected direct scribium-compat dependency: {forbidden}"
        );
    }
}
