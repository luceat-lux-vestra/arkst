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
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn len(&self) -> usize {
        self.end - self.start
    }

    pub fn is_empty(&self) -> bool {
        self.start >= self.end
    }
}

/// A line/column location within a source file (1-indexed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LineColumn {
    pub line: usize,
    pub column: usize,
}

impl LineColumn {
    pub fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

/// A span referencing a source file by ID, with byte-range start/end positions.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct SourceSpan {
    pub source_id: u32,
    pub start: usize,
    pub end: usize,
}

impl SourceSpan {
    pub fn new(source_id: u32, start: usize, end: usize) -> Self {
        Self { source_id, start, end }
    }
}