/// Quarkdown-compatible syntax parser.
///
/// Parses Scribium's primary syntax: `@`-prefixed function calls,
/// expressions, conditionals, iteration, and variable bindings.
///
/// This is clean-room implementation based on public documentation.
/// See `docs/legal/CLEAN_ROOM_POLICY.md` and `docs/compatibility/quarkdown/`
/// for provenance records.
pub mod parser;

pub use parser::parse_directive;

use crate::source::ByteSpan;
use crate::syntax::markdown::ast::Value;

/// A parsed Quarkdown directive.
#[derive(Debug, Clone, PartialEq)]
pub enum Directive {
    /// A function call: @name, @name(arg), @name[body], @name(arg)[body]
    Call {
        name: String,
        positional_args: Vec<Value>,
        named_args: Vec<(String, Value)>,
        body: Option<Box<Directive>>,
        span: ByteSpan,
    },
    /// A variable reference: @name
    Variable { name: String, span: ByteSpan },
    /// A conditional: @if(cond)[then] or @if(cond)[then][else]
    Conditional {
        condition: Box<Directive>,
        then_branch: Box<Directive>,
        else_branch: Option<Box<Directive>>,
        span: ByteSpan,
    },
    /// A raw value (string, number, boolean)
    Value(Value),
}
