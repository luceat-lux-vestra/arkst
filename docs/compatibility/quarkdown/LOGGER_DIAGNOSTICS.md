# Quarkdown logger and diagnostic builtin contract

Issue: #197  
Baseline: Quarkdown v2.6.0  
Canonical #151 status: `PARTIAL`

This document defines Arkst's bounded evaluator contract for `.log`, `.debug`, and `.error`.

## Clean-room evidence

Disposable probe PRs #362 and #363 exercised only the official Quarkdown v2.6.0 Linux x64 distribution, pinned to SHA-256:

`5b015e47c820d06ff6774eb700e60d77bc575819d4a0140cc7f1a115e7ce6dc4`

The later Unit/value correction used disposable PRs #367, #369, #370, and #372 to
compare the exact official v2.5.1 and v2.6.0 Linux x64 artifacts:

- v2.5.1: `5751ab608fcb4daa2ec857a3368c029beed5429554ae0bdd95c660b2706269e9`
- v2.6.0: `5b015e47c820d06ff6774eb700e60d77bc575819d4a0140cc7f1a115e7ce6dc4`

Those probes proved the same Unit output/value-context boundary in both
versions, including the callable-body distinction between one unconsumed Unit,
repeated Unit statements (captureable empty content), and mixed Unit/content bodies. No Quarkdown implementation source, tests, or fixtures were inspected
or copied for these probes.

Disposable clean-room PR #378 then probed the logger `message: String`
conversion boundary against the same exact v2.5.1/v2.6.0 artifacts. Final
probe HEAD `82885148091846b7f7bb4e33611d6738a5953727` (workflow run
35426565850; jobs 105853447856 / 105853447726) observed identical behavior:
plain-text Pairs stringify as
`[DynamicValue(unwrappedValue=left, evaluationContext=null), DynamicValue(unwrappedValue=right, evaluationContext=null)]`,
closed `1..3` Ranges stringify as `1..3`, `None` as `None`, and Unit as
`kotlin.Unit`. Direct and variable-held Pair/Range values agree; `.debug`
accepts Pair/None/Unit while remaining CLI-silent; and `.error` accepts the
same Pair conversion. #378 is closed unmerged and no upstream implementation
source, tests, or fixtures were inspected or copied for this correction.

Disposable clean-room PR #382 then re-pinned the remaining `.error` output
contract against the exact same v2.5.1/v2.6.0 official artifacts. Workflow run
35432948304 passed for v2.5.1 job 105870633950 and v2.6.0 job 105870633937.
Both versions agree that default/non-strict `.error` exits 0, writes the stable
one-line failed-call presentation to stderr, renders an error component at the
call site, and continues following content; the same holds through a
source-defined function. Both strict probes exit 66 and publish no artifact.
Strict stack implementation frames differ between versions and are explicitly
not a compatibility contract. #382 is closed unmerged.

Disposable clean-room PR #386 then pins the strict evaluation/finalization
boundary on the same exact v2.5.1/v2.6.0 artifacts. Both versions agree that
strict mode is not a global evaluator abort: later top-level/caller `.log`
effects remain observable, a later explicit-error argument is still evaluated,
and selected function/conditional body-local content after `.error` remains
suppressed while caller/outer content continues. An unselected conditional
remains lazy and succeeds normally. Final strict failure reports the first
selected explicit error, exits 66, and publishes no artifact. #386 is closed
unmerged.

Disposable clean-room PR #385 then tightened non-strict body-continuation
semantics against the same exact artifacts. Final jobs 105880439809 (v2.5.1)
and 105880439763 (v2.6.0) agree that an explicit error inside a
source-defined function or selected conditional body replaces that body's
ordinary content: content before and after the error is suppressed, the error
component becomes the body/callable result, and caller/top-level content after
the invocation continues. Repeated top-level explicit errors each emit their
own stderr line and rendered component while intervening/following content
continues in source order. #385 is closed unmerged.

Observed behavior from probe runs 35284604253 / 35284697810 / 35302505118, with final return-value confirmation in run 35324039621 (job 105532850554):

- `.log {text}` writes the converted message to stdout, contributes no direct document output, and compilation succeeds in both default and `--strict` modes.
- numeric `.log {42}` writes `42`; repeated log calls preserve evaluation order.
- when `.log` is evaluated in value context and assigned to a variable, the observed value renders as `kotlin.Unit`; the same is true for `.debug`.
- `.debug` produces no observable stdout/stderr or direct document output in the distributed CLI, including `--strict`; the CLI exposes no debug/verbose switch that enables it.
- `.error {message}` is not ordinary logging. In default mode it reports a failed function call to stderr, materializes Quarkdown's rendered error component at the call site, continues evaluating following document content, and exits successfully.
- inside a selected conditional or source-defined function, `.error` replaces that body/callable's ordinary content: body-local content before/after the error is suppressed, the error component becomes the result, and caller/top-level content after the invocation continues.
- with `--strict`, evaluation retains the same body-local suppression and caller/top-level continuation semantics, then native build finalization fails with exit code 66 before any output artifact is published.
- strict stderr begins with `An error occurred while in strict mode (error code 66)`, identifies `Originated from function: error`, and reports the first selected explicit-error message; exact JVM stack frames are not reproduced.
- logger side effects after an earlier top-level explicit error can remain observable before strict finalization, matching clean-room #386.

These observations distinguish three contracts rather than one severity-only logger API.

## Arkst bounded semantic model

### `.log`

Arkst evaluates and converts the required `message` argument through the shared invocation/binding and scalar String conversion path.

The platform-neutral evaluator never discovers or writes stdout, stderr, a process logger, environment state, filesystem state, or network state. A caller that wants `.log` observability must explicitly supply a `LogSink`.

With a sink, Arkst emits one source-backed `LogEvent { level: Log, ... }` immediately at the call site. Event order is evaluation order. A successful call returns typed `IrValue::Unit` in value context. An unconsumed direct call suppresses Unit document output; capture, variable reference, a source-defined callable with one otherwise-unconsumed Unit, or `::string` can make the value observable as `kotlin.Unit`. Two or more direct Unit statements in one callable instead produce a captureable empty-content value, while mixed Unit/content bodies retain the real content.

Without a sink, Arkst deterministically rejects the otherwise valid call with `E3010`. Argument binding and conversion happen before this capability rejection, so malformed calls retain their ordinary binding/conversion diagnostics.

This remains intentionally `PARTIAL`: the engine models the independently evidenced Unit value boundary and a logger-specific bounded DynamicValue-to-String adapter for Unit, None, closed Range, and plain-text Pair values. The generic scalar String adapter remains unchanged, so unrelated String consumers do not inherit these logger-only conversions. Native `arkst build` supplies an explicit host-owned sink that writes `Log` events to stdout in evaluation order while leaving `Debug` silent; the core/evaluator still perform no ambient process I/O, and ordinary `compile(...)` remains no-sink/fail-closed for `.log`. Default native build treats only a structured explicit-`.error` diagnostic paired with a direct document-level explicit-error component as recoverable, emits the clean-room-evidenced one-line stderr presentation, and continues artifact production. Native `--strict` preserves evaluation/logger continuation, then converts the first paired explicit error into the bounded strict stderr prefix, exit 66, and no-artifact finalization before output lowering/writes. Ordinary errors remain fatal. Arkst-specific `check`/`inspect` output policy is not promoted by this bounded CLI build slice. Unreviewed structured DynamicValue categories remain fail-closed. Unit stays distinct from both `None` and evaluator `NoValue`; only the separately evidenced equality operation treats Unit and None as equivalent. #190 is complete; public WASM/embedder exposure remains #191-owned. #368 records the value-model correction to #149.

### `.debug`

Arkst validates and converts `message` identically to `.log`.

If a `LogSink` is supplied, one source-backed `Debug` event is emitted. If no sink is supplied, the call remains host-silent after successful binding/conversion, matching the observable CLI default. In either successful case the semantic result is typed `IrValue::Unit`; direct call output suppresses it, while value-context consumers can observe the same bounded Unit contract as `.log`.

The optional explicit sink is an Arkst embedder boundary; it does not claim that Quarkdown exposes an equivalent public debug switch.

### `.error`

Arkst does not route `.error` through `LogSink`. After normal binding and String conversion it creates one source-backed `E3011` compiler diagnostic and a backend-neutral `IrComponent::ExplicitError` carrying only the converted message and provenance. At top level, output contexts materialize that component and later document content continues. Inside a source-defined function, the explicit error becomes the callable result: earlier callable output is discarded, later callable statements are not evaluated, and the caller continues after the invocation. The invocation still uses failure-style transaction rollback, and an explicit-error component is not exposed as an ordinary scalar/chain value.

The evaluator performs no stderr write. Native `arkst build` treats an explicit `.error` as recoverable only when the dedicated E3011 diagnostic is paired 1:1 with a direct document-level materialized `IrComponent::ExplicitError` carrying the same source span and evidenced message presentation. Unrelated E3011 diagnostics, unmaterialized value-context errors, orphan error components, and explicit errors nested inside unevidenced wrapper/list/blockquote output contexts remain fatal. Default build writes the clean-room-evidenced line `Cannot call function error(String message) with arguments (<message>): <message>` to stderr and continues artifact production only after that pairing check. Typst lowering emits a visible semantic error block without claiming Quarkdown's HTML/CSS styling or source-snippet markup.

This remains `PARTIAL`: the independently evidenced default and native `--strict` build contracts are covered, including exit 66, no-artifact finalization, first-error stderr presentation, top-level/caller continuation, selected-conditional/function body replacement, unselected-conditional laziness, chain-stop, and failure-style rollback. Exact Quarkdown HTML/CSS error-card styling, exact JVM stack traces, unevidenced output contexts, Arkst-specific `check`/`inspect` policy, and unreviewed logger String categories are not generalized.

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
- reopen or duplicate completed #190 host/process capability work;
- complete #191 public WASM/embedder bindings;
- broaden the evidenced Unit contract into generalized JVM/Kotlin object emulation;
- claim exact Quarkdown HTML/CSS error-card styling or exact JVM strict stack-trace parity;
- claim logger String formatting for structured categories beyond the independently evidenced Unit/None/closed-Range/plain-text-Pair subset;
- close #197 while exact HTML styling/unevidenced output contexts, analysis-command policy, and unreviewed logger String categories remain;
- claim M3 (#263) completion.
