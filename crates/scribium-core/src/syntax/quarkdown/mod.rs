//! Quarkdown-compatible function-call syntax.
//!
//! This module implements the dot-prefixed function-call grammar documented
//! in the public Quarkdown syntax documentation:
//!
//! ```text
//! .function
//! .function {arg1} {arg2}
//! .function name:{value}
//! .function {arg} name:{value}
//! ```
//!
//! The parser produces a `QuarkdownCall` with a flat argument list, which is
//! normalized into the Scribium Markdown AST (`Block::DirectiveCall` /
//! `Inline::DirectiveCall`) by the Markdown parser. Indented bodies are
//! handled at the block level, not here.
//!
//! This is a clean-room implementation: no upstream Quarkdown source code is
//! used. See `docs/compatibility/quarkdown/` for provenance.

mod parser;

pub(crate) use parser::has_valid_call_boundary;
pub(crate) use parser::is_valid_normal_call_name;
pub use parser::{parse_directive_at, Arg, ArgContent, ParseError, QuarkdownCall};
