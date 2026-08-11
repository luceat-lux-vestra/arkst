//! Authoritative Markdown block parsing state and line classification.

mod classify;
mod line;
mod parser;

pub(crate) use line::{split_lines, SourceLine};
pub(crate) use parser::BlockParser;
