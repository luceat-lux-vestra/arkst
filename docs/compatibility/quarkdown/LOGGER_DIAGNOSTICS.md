# Quarkdown logger and diagnostic builtin contract

Issue: #197  
Baseline: Quarkdown v2.6.0  
Canonical #151 status: `PARTIAL`

This document defines Arkst's bounded evaluator contract for `.log`, `.debug`, and `.error`.

## Clean-room evidence

Disposable probe PRs #362 and #363 exercised only the official Quarkdown v2.6.0 Linux x64 distribution, pinned to SHA-256:

`5b015e47c820d06ff6774eb700e60d77bc575819d4a0140cc7f1a115e7ce6dc4`

No Quarkdown implementation source, tests, or fixtures were inspected or copied.

Observed behavior from probe runs 35284604253 / 35284697810 / 35302505118, with final return-value confirmation in run 35324039621 (job 105532850554):

- `.log {text}` writes the converted message to stdout, contributes no direct document output, and compilation succeeds in both default and `--strict` modes.
- numeric `.log {42}` writes `42`; repeated log calls preserve evaluation order.
- when `.log` is evaluated in value context and assigned to a variable, the observed value renders as `kotlin.Unit`; the same is true for `.debug`.
- `.debug` produces no observable stdout/stderr or direct document output in the distributed CLI, including `--strict`; the CLI exposes no debug/verbose switch that enables it.
- `.error {message}` is not ordinary logging. In default mode it reports a failed function call to stderr, materializes Quarkdown's rendered error component at the call site, continues evaluating following document content, and exits successfully.
- `.error` behaves the same inside a conditional and inside a source-defined function: the call is represented as an error while later content continues.
- with `--strict`, `.error` aborts compilation with exit code 66 and produces no output artifact.
- a preceding `.log` remains observable even when a later strict `.error` aborts the compile.

These observations distinguish three contracts rather than one severity-only logger API.

## Arkst bounded semantic model

### `.log`

Arkst evaluates and converts the required `message` argument through the shared invocation/binding and scalar String conversion path.

The platform-neutral evaluator never discovers or writes stdout, stderr, a process logger, environment state, filesystem state, or network state. A caller that wants `.log` observability must explicitly supply a `LogSink`.

With a sink, Arkst emits one source-backed `LogEvent { level: Log, ... }` immediately at the call site. Event order is evaluation order. The current bounded implementation then returns evaluator `NoValue`; it does **not** yet model the upstream value-context `kotlin.Unit` result.

Without a sink, Arkst deterministically rejects the otherwise valid call with `E3010`. Argument binding and conversion happen before this capability rejection, so malformed calls retain their ordinary binding/conversion diagnostics.

This is intentionally `PARTIAL`: the engine semantic boundary exists, but the normal CLI does not yet reproduce Quarkdown's stdout behavior and Arkst does not yet model the observed first-class `kotlin.Unit` return value. Returning a fabricated String or conflating Unit with `None`/evaluator `NoValue` is explicitly rejected. Host/CLI exposure remains coordinated with #190, value-model closure with #149, and public WASM/embedder exposure with #191.

### `.debug`

Arkst validates and converts `message` identically to `.log`.

If a `LogSink` is supplied, one source-backed `Debug` event is emitted. If no sink is supplied, the call is a silent no-op after successful binding/conversion, matching the observable v2.6 CLI default. As with `.log`, Arkst currently returns evaluator `NoValue` rather than modeling the observed value-context `kotlin.Unit`. This keeps the bounded implementation honest and leaves the value-model gap explicit.

The optional explicit sink is an Arkst embedder boundary; it does not claim that Quarkdown exposes an equivalent public debug switch.

### `.error`

Arkst does not route `.error` through `LogSink`. After normal binding and String conversion it creates one source-backed `E3011` compiler diagnostic and the call fails semantically. Evaluation of later top-level content continues, matching the observed non-strict continuation behavior. This is explicitly covered both with and without an injected logger sink, proving that `.error` does not accidentally acquire logger-capability semantics.

The diagnostic remains structured compiler state rather than an implicit stderr write.

This is `PARTIAL` because Arkst does not yet reproduce Quarkdown's rendered HTML error component, exact stderr text, or CLI `--strict` exit-code behavior. Those output/CLI effects must not be fabricated inside the platform-neutral evaluator.

## Ordering, failure, and provenance

- `LogSink::emit` occurs synchronously at the logger call site.
- a successfully emitted log event is not rolled back merely because a later call fails; this matches the observed `.log`-before-`.error` behavior.
- malformed, duplicate, missing, or non-convertible arguments emit no logger event.
- every event and explicit `.error` diagnostic carries the originating `SourceSpan`.
- source-defined functions retain existing dispatch precedence over the native logger names.
- repeated evaluation owns no hidden process/global logger state.

## Host and platform boundary

`LogSink`, `LogEvent`, and `LogLevel` are platform-neutral engine types. They contain only semantic data and source provenance. The evaluator exposes both resource-free injection and a combined resource/loadable-library/logger entry point so logger authority composes with ordinary project evaluation without serializing host objects into IR.

This contract does not:

- authorize ambient stdout/stderr or a process logging framework;
- add environment, filesystem, network, or plugin discovery;
- complete #190 host/process capability work;
- complete #191 public WASM/embedder bindings;
- invent or stringify a fake Unit value in place of the observed `kotlin.Unit` result;
- claim Quarkdown error-card rendering or strict-mode CLI parity;
- claim exact upstream `kotlin.Unit` value semantics or rich DynamicValue-to-String formatting;
- close #197 while those return/value and output-policy gaps remain;
- claim M3 (#263) completion.
