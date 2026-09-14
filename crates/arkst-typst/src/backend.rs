//! Platform-neutral contract for Typst compiler adapters.

use std::time::Duration;

/// Output target requested from an official Typst compiler adapter.
///
/// `Typst` source emission itself is owned by Arkst lowering and is therefore
/// not represented here. These values name compiler-owned rendering targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TypstOutputTarget {
    Pdf,
    Png,
    Svg,
    Html,
    /// Experimental multi-file Typst target. Adapters may reject this until
    /// they implement deterministic bundle artifact collection.
    Bundle,
}

impl TypstOutputTarget {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Png => "png",
            Self::Svg => "svg",
            Self::Html => "html",
            Self::Bundle => "bundle",
        }
    }

    pub const fn extension(self) -> Option<&'static str> {
        match self {
            Self::Pdf => Some("pdf"),
            Self::Png => Some("png"),
            Self::Svg => Some("svg"),
            Self::Html => Some("html"),
            Self::Bundle => None,
        }
    }

    pub const fn is_experimental(self) -> bool {
        matches!(self, Self::Html | Self::Bundle)
    }
}

impl std::fmt::Display for TypstOutputTarget {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// One artifact produced by a Typst compiler target.
///
/// The path is adapter-relative logical output identity, never a host path.
/// A target may return more than one artifact (for example multi-page PNG/SVG
/// or a future bundle target), so callers must not assume a single byte blob.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypstArtifact {
    pub path: String,
    pub bytes: Vec<u8>,
}

impl TypstArtifact {
    pub fn new(path: impl Into<String>, bytes: Vec<u8>) -> Self {
        Self {
            path: path.into(),
            bytes,
        }
    }
}

/// Abstract interface for a Typst compiler backend.
///
/// Concrete execution errors remain owned by the adapter that produces them;
/// this contract therefore exposes no filesystem, process, or host-path type.
pub trait TypstBackend {
    type Error: std::error::Error;

    /// Compile a Typst source document for an explicit official Typst target.
    fn compile_target(
        &self,
        input: &TypstInput,
        target: TypstOutputTarget,
    ) -> Result<TypstOutput, Self::Error>;

    /// Backward-compatible convenience for the historical PDF-only backend
    /// contract. New code should select a target explicitly.
    fn compile(&self, input: &TypstInput) -> Result<TypstOutput, Self::Error> {
        self.compile_target(input, TypstOutputTarget::Pdf)
    }

    /// Whether this adapter implements the requested target contract.
    ///
    /// This describes adapter support, not whether an arbitrary external Typst
    /// executable is new enough to accept the target at runtime.
    fn supports_target(&self, target: TypstOutputTarget) -> bool;

    /// Return the Typst compiler version.
    fn version(&self) -> Result<String, Self::Error>;
}

/// Input to a Typst compiler adapter.
pub struct TypstInput {
    pub source: String,
    /// Project-root-relative logical path of the Arkst source entry.
    ///
    /// A native adapter validates this value before interpreting it in a host
    /// project context. Lowering itself treats it as opaque contract data.
    pub entry_path: String,
}

/// Output from a Typst compiler adapter.
#[derive(Debug)]
pub struct TypstOutput {
    pub target: TypstOutputTarget,
    pub artifacts: Vec<TypstArtifact>,
    pub diagnostics: Vec<String>,
    pub duration: Duration,
}

impl TypstOutput {
    pub fn new(
        target: TypstOutputTarget,
        artifacts: Vec<TypstArtifact>,
        diagnostics: Vec<String>,
        duration: Duration,
    ) -> Self {
        Self {
            target,
            artifacts,
            diagnostics,
            duration,
        }
    }

    pub fn single_artifact(&self) -> Option<&TypstArtifact> {
        if self.artifacts.len() == 1 {
            self.artifacts.first()
        } else {
            None
        }
    }

    pub fn single_artifact_bytes(&self) -> Option<&[u8]> {
        self.single_artifact().map(|artifact| artifact.bytes.as_slice())
    }

    pub fn into_single_artifact(mut self) -> Option<TypstArtifact> {
        if self.artifacts.len() == 1 {
            self.artifacts.pop()
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets_have_stable_names_and_extensions() {
        let cases = [
            (TypstOutputTarget::Pdf, "pdf", Some("pdf"), false),
            (TypstOutputTarget::Png, "png", Some("png"), false),
            (TypstOutputTarget::Svg, "svg", Some("svg"), false),
            (TypstOutputTarget::Html, "html", Some("html"), true),
            (TypstOutputTarget::Bundle, "bundle", None, true),
        ];

        for (target, name, extension, experimental) in cases {
            assert_eq!(target.as_str(), name);
            assert_eq!(target.extension(), extension);
            assert_eq!(target.is_experimental(), experimental);
            assert_eq!(target.to_string(), name);
        }
    }

    #[test]
    fn single_artifact_is_only_returned_for_exactly_one_artifact() {
        let empty = TypstOutput::new(
            TypstOutputTarget::Pdf,
            vec![],
            vec![],
            Duration::ZERO,
        );
        assert!(empty.single_artifact().is_none());
        assert!(empty.single_artifact_bytes().is_none());

        let one = TypstOutput::new(
            TypstOutputTarget::Pdf,
            vec![TypstArtifact::new("output.pdf", b"%PDF".to_vec())],
            vec![],
            Duration::ZERO,
        );
        assert_eq!(one.single_artifact().unwrap().path, "output.pdf");
        assert_eq!(one.single_artifact_bytes(), Some(b"%PDF".as_slice()));

        let many = TypstOutput::new(
            TypstOutputTarget::Png,
            vec![
                TypstArtifact::new("output-1.png", vec![1]),
                TypstArtifact::new("output-2.png", vec![2]),
            ],
            vec![],
            Duration::ZERO,
        );
        assert!(many.single_artifact().is_none());
        assert!(many.single_artifact_bytes().is_none());

        let owned = TypstOutput::new(
            TypstOutputTarget::Pdf,
            vec![TypstArtifact::new("output.pdf", b"owned".to_vec())],
            vec![],
            Duration::ZERO,
        )
        .into_single_artifact()
        .unwrap();
        assert_eq!(owned.bytes, b"owned");
    }
}
