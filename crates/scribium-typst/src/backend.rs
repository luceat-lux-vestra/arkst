/// Typst backend trait and adapters.
use std::time::Duration;

/// Abstract interface for the Typst compiler backend.
pub trait TypstBackend {
    /// Compile a Typst source document.
    fn compile(&self, input: &TypstInput) -> Result<TypstOutput, TypstError>;
    /// Return the Typst compiler version.
    fn version(&self) -> Result<String, TypstError>;
}

/// Input to the Typst compiler.
pub struct TypstInput {
    pub source: String,
    pub entry_path: String,
}

/// Output from the Typst compiler.
pub struct TypstOutput {
    pub pdf: Option<Vec<u8>>,
    pub html: Option<String>,
    pub svg: Option<Vec<u8>>,
    pub png: Option<Vec<u8>>,
    pub diagnostics: Vec<String>,
    pub duration: Duration,
}

/// Errors from the Typst backend.
#[derive(Debug)]
pub enum TypstError {
    Subprocess(String),
    Io(std::io::Error),
}

/// Subprocess backend — calls `typst compile` via CLI.
pub struct SubprocessBackend {
    pub typst_path: String,
}

impl SubprocessBackend {
    pub fn new(typst_path: impl Into<String>) -> Self {
        Self {
            typst_path: typst_path.into(),
        }
    }
}

impl TypstBackend for SubprocessBackend {
    fn compile(&self, _input: &TypstInput) -> Result<TypstOutput, TypstError> {
        // TODO: implement subprocess compilation
        Err(TypstError::Subprocess("not implemented".into()))
    }

    fn version(&self) -> Result<String, TypstError> {
        // TODO: parse `typst --version` output
        Ok("0.15.1 (subprocess)".into())
    }
}
