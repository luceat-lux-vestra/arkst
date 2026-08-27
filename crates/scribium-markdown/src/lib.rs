//! Rushdown-backed Markdown frontend.
//!
//! Rushdown is intentionally referenced only here. Its AST, `Segment` values,
//! parser extensions, and source accessors do not escape this crate.

mod parser;

pub mod ast;

pub use ast::{
    Block, CallArgument, CallSegment, Document, FrontMatter, Inline, ListItem, TableAlignment,
    TableCell, TableRow, TaskStatus, Value,
};
pub use parser::{
    parse, parse_md, parse_qd, parse_with_diagnostics, parse_with_markdown_profile,
    parse_with_mode, MarkdownProfile, Mode, ParseOutput, ParserDiagnostic,
};
