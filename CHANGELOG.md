# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Repository bootstrap with Apache-2.0 licensing and provenance policy.
- Product vision defining Scribium as a Quarkdown-compatible compiler.
- Clean-room Quarkdown compatibility policy and process documentation.
- Architecture, roadmap, and non-goals documentation.
- Baseline Rust workspace with scribium-core, scribium-typst, scribium-cli, scribium-test-support.
- ADR process and initial architecture decisions.
- GitHub templates for issues and pull requests.
- CI workflow with fmt, clippy, test, and dependency checks.
- M0 Foundation milestone — clean-room policy, naming research, parser/backend spikes.
- Minimal CommonMark-compatible Markdown parser (`syntax::markdown`) with
  byte-level source spans: ATX headings, paragraphs, emphasis/strong,
  unordered lists with nesting, fenced code blocks, thematic breaks, and
  hard/line breaks. No panics on malformed input.
- Source span infrastructure: `SourceId`, `ByteSpan`, `LineColumn`, `SourceSpan`.
- Structured diagnostics with stable error codes (`Diagnostic`, `Severity`).
- Compatibility profile selection and divergence tracking.
- CLI commands: `build`, `check`, `inspect`.
- Typst backend trait (`TypstBackend`) with `SubprocessBackend` adapter skeleton.
- Typst lowering skeleton (`lower_to_typst`).
- `build` accepts a bare file name (`scribium build document.qd`), resolving
  its project root to the current directory.
- `build --output <path>` to override the generated output path.

### Fixed

- `build` never overwrites the input source file: an output that resolves to
  the input is rejected with a clear error. Existing outputs are compared by
  file identity (device/inode on Unix, file index on Windows via `same-file`),
  so relative/absolute spellings, `.`/`..` components, symlinks, and hard
  links that alias the input are all detected; the check is repeated
  immediately before writing. Rejected builds leave the input byte-for-byte
  unchanged.
- Console test targets build on Windows (unused-import warnings only surfaced
  on non-unix platforms).

### Changed

- Supported CLI inputs are now `.qd`, `.scrib`, and `.md`; a `.typ` input is
  rejected as an unsupported format until Typst passthrough is implemented.
- Front matter is documented as a flat line-based `key: value` format, not
  full YAML: nested objects, arrays, and block strings are not supported.
  Keys split on the first colon; delimiters and metadata lines must start at
  column 0 (indented keys reject the block, which is preserved as regular
  Markdown); duplicate keys use last-wins semantics; user-defined metadata is
  stored in the IR in deterministic order.
- Added the `same-file` dependency for cross-platform file-identity checks.
- Windows CI previously failed to compile the `scribium` test binary due to
  unused imports on Windows-only configurations; this is resolved.

### Changed

- Issue templates: fixed label formatting (`type: bug` → `type:bug`),
  added milestone dropdown to feature requests.
- Removed duplicate/obsolete GitHub labels: `bug`, `enhancement`,
  `duplicate`, `invalid`, `question`, `good first issue`, `help wanted`,
  `dependencies`.
- Updated external dependencies via `cargo update`.
- Repo management: closed #2 (bootstrap completed), extracted remaining
  M0 tasks into #11 (name due diligence) and #12 (in-process Typst
  feasibility). Assigned #4, #5, #6 to M1 Vertical Slice milestone.