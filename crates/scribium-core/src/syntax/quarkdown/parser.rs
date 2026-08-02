// A simplified Quarkdown-compatible parser.
//
// This will be expanded during M1 to support the full core subset.
// Currently a placeholder that parses `@`-prefixed function calls.

use crate::source::{ByteSpan, SourceId};

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

/// A literal value in a directive.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    String(String),
    Number(f64),
    Boolean(bool),
    Identifier(String),
}

/// Parse a directive from a `@`-prefixed source string.
///
/// Returns `None` if the source does not start with a valid directive.
pub fn parse_directive(source: &str, source_id: SourceId) -> Option<Directive> {
    let _ = source;
    let _ = source_id;
    // TODO: implement directive parser in M1
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::SourceId;

    #[test]
    fn parse_empty_returns_none() {
        assert!(parse_directive("", SourceId(1)).is_none());
    }

    #[test]
    fn parse_plain_text_returns_none() {
        assert!(parse_directive("hello world", SourceId(1)).is_none());
    }
}
