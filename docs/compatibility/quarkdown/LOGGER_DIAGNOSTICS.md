# Quarkdown logger and diagnostic builtins

Issue: #197  
Baseline: Quarkdown v2.6.0

This document defines Arkst's bounded evaluator contract for `.log`, `.debug`,
and `.error`.

## Clean-room observations

Disposable probe PR #362 used only the official Quarkdown v2.6.0 Linux x64
release artifact pinned at SHA-256
`5b015e47c820d06ff6774eb700e60d77bc575819d4a0140cc7f1a115e7ce6dc4`.
No upstream implementation source, tests, or fixtures were inspected or copied.

Observed CLI behavior:

- `.log` converts scalar messages to text, writes them to stdout in evaluation
  order, contributes no visible document value, and does not fail strict mode;
- `.debug` produced no observable stdout/stderr/document output, and the public
  CLI exposes no debug/verbose switch;
- `.error` reports the converted message as an error. Default CLI mode
  continues compilation, while `--strict` exits 66 and does not publish an
  output artifact;
- repeated `.log` calls preserve source evaluation order.

The CLI process-stream selection and strict-mode policy are host/application
behavior. They are evidence for semantics, not authorization for platform-
neutral engine code to open process streams.

## Arkst bounded semantic model

The engine exposes `RuntimeMessageSink`. Supplying a sink is the explicit host
capability for `.log` and `.debug`; the evaluator never writes stdout/stderr.

A runtime event contains:

- `RuntimeMessageLevel::{Log, Debug, Error}`;
- the shared scalar-to-String converted message;
- the original call `SourceSpan`.

`.log` and `.debug` emit exactly one ordered event and return `NoValue`.
Without an explicit sink they fail closed with source-backed `E3006` instead
of silently selecting a host stream.

`.error` emits an `Error` event when a sink is present, then emits one
source-backed structured `E3006` evaluator diagnostic and returns `Failed`.
The outer document evaluator continues to later nodes, matching the observed
non-strict recovery shape at the semantic boundary. Arkst does not yet claim
Quarkdown's rendered error representation or CLI `--strict` exit-code parity.

Sink authority is invocation-time host state. It propagates through child
scopes, source-defined functions, and loaded-library evaluation, but it is
never serialized into callable captures or IR.

## Status and non-goals

This is a bounded evaluator implementation. Full process-stream / CLI
presentation parity, public WASM/embedder sink exposure, and strict-mode
application policy remain outside this issue's engine slice.

The implementation does not add implicit stdout/stderr, environment,
filesystem, network, or process access and does not introduce a second logging
subsystem.
