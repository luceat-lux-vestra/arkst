//! `scribium-test-support` — Test utilities for Scribium.
//!
//! Provides:
//! - Fixture loading from `fixtures/` directories
//! - Golden test assertion helpers
//! - Temporary project builder for integration tests
//! - Normalized path and output comparison

/// Load a fixture file by name from `fixtures/{category}/`.
pub fn load_fixture(category: &str, name: &str) -> String {
    let path = std::path::Path::new("fixtures")
        .join(category)
        .join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot load fixture {:?}: {}", path, e))
}

/// Assertion helper for golden tests.
/// Compares actual output against an expected file.
pub fn assert_golden(actual: &str, expected_path: &str) {
    let expected = std::fs::read_to_string(expected_path)
        .unwrap_or_else(|e| panic!("cannot read golden {:?}: {}", expected_path, e));
    assert_eq!(actual, expected, "golden mismatch for {}", expected_path);
}