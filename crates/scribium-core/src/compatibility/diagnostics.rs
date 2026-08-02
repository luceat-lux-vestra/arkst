/// Compatibility diagnostics for Quarkdown features.
/// Generates `E8xxx` diagnostics for unsupported or diverging features.
pub fn unsupported_feature(feature: &str) -> String {
    format!("unsupported Quarkdown feature: {}", feature)
}
