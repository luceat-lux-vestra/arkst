//! Platform-neutral source identity and byte-span primitives.
//!
//! This crate deliberately contains no filesystem, process, network, or
//! backend dependency so frontend crates can remain WASM-compatible.

use std::sync::Arc;

/// A unique identifier for a source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SourceId(pub u32);

/// Immutable source text shared by source-backed frontend and IR values.
///
/// Body values retain a clone of this handle and their own byte span instead
/// of owning a copied body string. Cloning a handle therefore preserves one
/// source buffer across nested bodies while keeping provenance lossless.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceText(Arc<str>);

impl SourceText {
    pub fn new(source: impl Into<Arc<str>>) -> Self {
        Self(source.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn slice(&self, span: ByteSpan) -> Option<&str> {
        span.checked_str(self.as_str())
    }
}

impl Default for SourceText {
    fn default() -> Self {
        Self(Arc::from(""))
    }
}

impl serde::Serialize for SourceText {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for SourceText {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let source = <String as serde::Deserialize>::deserialize(deserializer)?;
        Ok(Self(Arc::from(source)))
    }
}

/// A byte-level location within a source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ByteSpan {
    pub start: usize,
    pub end: usize,
}

impl ByteSpan {
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub const fn len(self) -> usize {
        self.end.saturating_sub(self.start)
    }

    pub const fn is_empty(self) -> bool {
        self.start >= self.end
    }

    pub fn checked_str(self, source: &str) -> Option<&str> {
        source.get(self.start..self.end)
    }

    pub fn is_valid_for(self, source: &str) -> bool {
        self.start <= self.end
            && self.end <= source.len()
            && source.is_char_boundary(self.start)
            && source.is_char_boundary(self.end)
    }
}

/// A span referencing a source file by ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SourceSpan {
    pub source_id: SourceId,
    pub start: usize,
    pub end: usize,
}

impl SourceSpan {
    pub const fn new(source_id: SourceId, start: usize, end: usize) -> Self {
        Self {
            source_id,
            start,
            end,
        }
    }

    pub const fn byte_span(self) -> ByteSpan {
        ByteSpan::new(self.start, self.end)
    }
}

/// A one-based line/column location.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LineColumn {
    pub line: usize,
    pub column: usize,
}

/// A span for generated output, deliberately distinct from source provenance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GeneratedSpan {
    pub start: usize,
    pub end: usize,
}

/// An entry in a generated-source map linking output bytes to source input.
///
/// The representation is backend-neutral; a lowering backend owns the
/// entries it produces while this crate owns their shared source-span type.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SourceMapEntry {
    /// Range in generated output (byte offsets).
    pub generated_start: usize,
    pub generated_end: usize,
    /// The original source span this generated range belongs to.
    pub original: SourceSpan,
}

#[cfg(test)]
mod tests {
    use super::SourceText;
    use std::sync::Arc;

    #[test]
    fn cloned_source_text_handles_share_one_immutable_buffer() {
        let original = SourceText::new("prefix\nbody\n".to_string());
        let clone = original.clone();
        assert!(Arc::ptr_eq(&original.0, &clone.0));
        assert_eq!(clone.as_str(), "prefix\nbody\n");
    }
}
