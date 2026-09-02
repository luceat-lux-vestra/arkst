pub mod diagnostics;
pub mod source;
pub mod syntax;

/// Compatibility profile selection, divergence tracking, and diagnostics.
///
/// This module handles only:
/// - Selecting which compatibility profile to use
/// - Recording known divergences from reference behavior
/// - Generating `E8xxx` diagnostics for unsupported features
///
/// The actual Quarkdown-compatible syntax parsing lives in the
/// `arkst-quarkdown` crate, not here. This module is about *tracking*
/// compatibility, not *implementing* it.
pub mod profile;
