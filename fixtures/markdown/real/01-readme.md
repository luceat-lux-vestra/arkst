# Project Readme

This document describes a small **Scribium** project. It uses `scribium-core`
to turn Markdown into a backend-neutral document and links to the
[repository](https://github.com/luceat-lux-vestra/scribium).

Badges-like links are ordinary Markdown links: [build: passing][build].

[build]: https://example.test/build

## Getting started

1. Install the toolchain.
2. Read the guide.
   - Check the examples.
   - Run `cargo test`.

```sh
cargo run -p scribium-cli -- build examples/hello/main.qd
```

> Keep source provenance intact while documents move through the pipeline.
