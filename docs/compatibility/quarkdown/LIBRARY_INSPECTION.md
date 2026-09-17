# Quarkdown library inspection contract

Issue: #195  
Baseline: Quarkdown v2.6.0  
Canonical #151 status: `PARTIAL`

This document defines Arkst's bounded evaluator contract for `.libraries`, `.libexists`, `.functionexists`, and `.libfunctions`. The runtime-inspection contract itself is implemented, but the compatibility rows remain `PARTIAL` while Arkst's supported callable stdlib is smaller than Quarkdown's registered stdlib.

## Clean-room evidence

The behavior was revalidated with disposable PR #360 against the official Quarkdown v2.6.0 Linux x64 distribution. The downloaded archive was pinned to SHA-256:

`5b015e47c820d06ff6774eb700e60d77bc575819d4a0140cc7f1a115e7ce6dc4`

No upstream implementation source, tests, or fixtures were inspected or copied.

Observed behavior:

- a fresh context exposes exactly one library: `stdlib`;
- `.libexists` is exact and case-sensitive;
- a library merely available through `--libs` is not visible until it is included;
- `.functionexists` sees standard functions, source-defined functions, and functions published by a successful library include;
- unknown `.libfunctions` returns an empty collection;
- a loaded container library has no functions of its own in `.libfunctions`;
- each source-defined function is represented by a `__func__<name>` pseudo-library whose `.libfunctions` result contains that function name;
- `.libfunctions {stdlib}` produced 164 names in a repeatable registration order. Five fresh processes produced identical SHA-256 `76382a842380e87d11c6be7ef382e019687942b6872eec0e55a03b282ab4726a` for the newline-separated sequence;
- run `35275404709`, job `105384697142` established multi-library registration ordering. With local functions `local_first`, `local_second`, then `beta`, then `alpha`, `.libraries` returned `stdlib`, the two local pseudo-libraries, `beta`, its pseudo-library, `alpha`, then its two pseudo-libraries. Reversing only the two includes reversed those two container/function groups.

The pinned 164-name list is **ordering evidence only**. It is not an Arkst support manifest.

## Arkst semantic model

The evaluator owns one ordered in-memory registry view. It is not a filesystem/package/plugin registry and does not ask the host to enumerate anything.

`stdlib` is implicit and always first. Source-defined functions append one `__func__<name>` entry at declaration time. A loadable container appends at the point where `.include` resolves it; declarations evaluated inside that library append their pseudo-libraries immediately after it. Duplicate visible registry names do not move or duplicate an existing entry.

Registry writes participate in the same invocation transaction journal as evaluator variables, functions, extension targets, and document state. A failed include or failed enclosing invocation therefore cannot leak a container or function pseudo-library into later inspection.

Callable captures preserve visible source-function registration order when functions are serialized into the existing capture representation. Restoring a capture recreates the corresponding pseudo-library view without adding provider objects or host metadata.

## Function visibility

`.functionexists {name}` is true only when `name` is visible through one of Arkst's actual evaluator-owned callable paths:

- a visible source-defined function; or
- a Quarkdown v2.6 standard function that Arkst currently implements through its builtin/native/special-language dispatch.

The v2.6 oracle list is used to order supported standard functions, never to make an unsupported function appear implemented. For example, unsupported `paragraphstyle` and deferred `llmstxt` remain false even though the upstream v2.6 registry contains those names.

Special language-owned functions such as `function`, `pageformat`, `code`, and `extend` are included when Arkst implements their evaluator/parser ownership even though they are not all represented by the ordinary scalar builtin table.

A source-defined function keeps the existing dispatch precedence if it uses the same name as a shadowable inspection native.

## Library visibility

`.libraries` returns a collection of visible library names in registration order.

`.libexists {name}` checks only that in-memory view. It does not call `LoadableLibraryProvider`, probe the filesystem, read environment variables, inspect packages, or perform network/process discovery. Consequently, an available-but-unloaded provider library remains false.

`.libfunctions {libraryName}` returns:

- for `stdlib`: the subset of the pinned v2.6 ordering that Arkst actually supports;
- for a visible `__func__<name>` pseudo-library: that function name when the binding remains visible;
- for a loaded container library: an empty collection;
- for an unknown library: an empty collection.

Names are exact and case-sensitive.

## Diagnostics and host boundary

Invocation binding and scalar-string conversion use the evaluator's existing source-backed diagnostic machinery. Missing, extra, duplicate, unknown named, or non-convertible arguments fail without mutating registry state.

Pure inspection requires no host capability. The same initial registry semantics apply through resource-free evaluation and resource-backed evaluation, which keeps the semantic model usable by native and WASM-capable callers without exposing hidden host state.

This does not complete the separate #191 WASM binding work.

## Non-goals

This contract does not add:

- dynamic plugin/package discovery;
- filesystem, process, network, or environment inspection;
- provider enumeration APIs;
- complete Quarkdown stdlib support;
- a second evaluator or registry model;
- a claim that M3 (#263) is complete.
