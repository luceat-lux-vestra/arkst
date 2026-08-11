/// Markdown parser module.
///
/// Parses CommonMark/GFM-compatible Markdown into the Scribium AST.
/// Spans are preserved for all parsed nodes.
pub mod ast;
mod block;
mod inline;
pub mod parser;

pub use ast::{Block, Document, Inline, ListItem, Value};
pub use parser::{parse, parse_with_diagnostics, ParseOutput, ParserDiagnostic};
