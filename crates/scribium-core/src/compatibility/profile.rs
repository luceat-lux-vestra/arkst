/// A compatibility profile describes which version of a reference syntax
/// Scribium aims to be compatible with.
#[derive(Debug, Clone)]
pub struct CompatibilityProfile {
    pub name: String,
    pub strict: bool,
}

impl Default for CompatibilityProfile {
    fn default() -> Self {
        Self {
            name: "quarkdown-v0.9".into(),
            strict: false,
        }
    }
}

/// A known divergence from a reference implementation.
#[derive(Debug, Clone)]
pub struct CompatibilityDivergence {
    pub feature: String,
    pub reference_behavior: String,
    pub scribium_behavior: String,
    pub rationale: String,
}