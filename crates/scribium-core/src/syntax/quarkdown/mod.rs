/// Quarkdown-compatible syntax parser.
///
/// Parses Scribium's primary syntax: `@`-prefixed function calls,
/// expressions, conditionals, iteration, and variable bindings.
///
/// This is clean-room implementation based on public documentation.
/// See `docs/legal/CLEAN_ROOM_POLICY.md` and `docs/compatibility/quarkdown/`
/// for provenance records.
pub mod parser;