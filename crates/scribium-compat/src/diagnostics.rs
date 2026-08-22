/// Compatibility diagnostics for Quarkdown features.
/// Generates `E8xxx` diagnostics for unsupported or diverging features.
pub fn unsupported_feature(feature: &str) -> String {
    format!("unsupported Quarkdown feature: {}", feature)
}

#[cfg(test)]
mod tests {
    use super::unsupported_feature;

    #[test]
    fn unsupported_feature_preserves_existing_message() {
        assert_eq!(
            unsupported_feature("foo"),
            "unsupported Quarkdown feature: foo"
        );
    }
}
