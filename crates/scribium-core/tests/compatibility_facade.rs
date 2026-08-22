fn accepts_core_profile(_: scribium_core::compatibility::profile::CompatibilityProfile) {}
fn accepts_compat_profile(_: scribium_compat::profile::CompatibilityProfile) {}
fn accepts_core_divergence(_: scribium_core::compatibility::profile::CompatibilityDivergence) {}
fn accepts_compat_divergence(_: scribium_compat::profile::CompatibilityDivergence) {}
fn accepts_core_source(_: scribium_core::compatibility::source::CompatibilitySource) {}
fn accepts_compat_source(_: scribium_compat::source::CompatibilitySource) {}
fn accepts_core_syntax(_: scribium_core::compatibility::syntax::SyntaxExtension) {}
fn accepts_compat_syntax(_: scribium_compat::syntax::SyntaxExtension) {}

#[test]
fn compatibility_facade_preserves_module_paths_and_type_identity() {
    let _: fn(scribium_core::compatibility::profile::CompatibilityProfile) = accepts_compat_profile;
    let _: fn(scribium_compat::profile::CompatibilityProfile) = accepts_core_profile;
    let _: fn(scribium_core::compatibility::profile::CompatibilityDivergence) =
        accepts_compat_divergence;
    let _: fn(scribium_compat::profile::CompatibilityDivergence) = accepts_core_divergence;
    let _: fn(scribium_core::compatibility::source::CompatibilitySource) = accepts_compat_source;
    let _: fn(scribium_compat::source::CompatibilitySource) = accepts_core_source;
    let _: fn(scribium_core::compatibility::syntax::SyntaxExtension) = accepts_compat_syntax;
    let _: fn(scribium_compat::syntax::SyntaxExtension) = accepts_core_syntax;
}

#[test]
fn compatibility_facade_preserves_existing_behavior() {
    let profile = scribium_core::compatibility::profile::CompatibilityProfile::default();
    assert_eq!(profile.name, "quarkdown-v2.5");
    assert!(!profile.strict);
    assert_eq!(
        scribium_core::compatibility::diagnostics::unsupported_feature("foo"),
        "unsupported Quarkdown feature: foo"
    );
}
