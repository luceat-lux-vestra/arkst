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