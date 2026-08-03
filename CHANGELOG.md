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
- Baseline Rust workspace with scribium-core, scribium-typst, scribium-cli, scribium-compat-quarkdown.
- ADR process and initial architecture decisions.
- GitHub templates for issues and pull requests.
- CI workflow with fmt, clippy, test, and dependency checks.
- M0 Foundation milestone — clean-room policy, naming research, parser/backend spikes.
- Minimal CommonMark-compatible Markdown parser (`syntax::markdown`) with
  byte-level source spans: ATX headings, paragraphs, emphasis/strong,
  unordered lists with nesting, fenced code blocks, thematic breaks, and
  hard/soft line breaks. No panics on malformed input.