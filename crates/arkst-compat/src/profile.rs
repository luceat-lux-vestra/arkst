/// A compatibility profile describes which version of a reference syntax
/// Arkst aims to be compatible with.
#[derive(Debug, Clone)]
pub struct CompatibilityProfile {
    pub name: String,
    pub strict: bool,
}

impl Default for CompatibilityProfile {
    fn default() -> Self {
        Self {
            name: "quarkdown-v2.5".into(),
            strict: false,
        }
    }
}

/// A known divergence from a reference implementation.
#[derive(Debug, Clone)]
pub struct CompatibilityDivergence {
    pub feature: String,
    pub reference_behavior: String,
    pub arkst_behavior: String,
    pub rationale: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Guards the default compatibility-profile label: it must track the
    /// current verified compatibility baseline (see ADR-0016) and must not regress to an
    /// earlier Quarkdown value.
    #[test]
    fn default_profile_matches_reference_baseline() {
        let profile = CompatibilityProfile::default();

        assert_eq!(profile.name, "quarkdown-v2.5");
        assert!(!profile.strict);
    }
}
