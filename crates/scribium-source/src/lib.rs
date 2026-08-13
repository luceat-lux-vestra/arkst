//! Platform-neutral source identity and byte-span primitives.
//!
//! This crate deliberately contains no filesystem, process, network, or
//! backend dependency so frontend crates can remain WASM-compatible.

/// A unique identifier for a source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SourceId(pub u32);

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
