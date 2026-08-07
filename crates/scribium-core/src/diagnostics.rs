//! Structured diagnostics with stable codes, severity, and source spans.

use crate::source::SourceSpan;

/// A structured diagnostic with stable code, severity, and source spans.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Diagnostic {
    pub code: String,
    pub severity: Severity,
    pub message: String,
    pub primary: Option<SourceSpan>,
    pub secondary: Vec<SourceSpan>,
    pub hints: Vec<String>,
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}: {}", self.severity, self.code, self.message)
    }
}

impl std::error::Error for Diagnostic {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

/// Diagnostic severity level.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Severity {
    Error,
    Warning,
    Hint,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Error => write!(f, "error"),
            Severity::Warning => write!(f, "warning"),
            Severity::Hint => write!(f, "hint"),
        }
    }
}
