# #719 G0 — Accepted maintainer direction: full clean-break architecture

**Status:** POLICY APPROVED 2026-11-11; G0 and G1–G8 IMPLEMENTATION INCOMPLETE.
**Owner:** [#719](https://github.com/luceat-lux-vestra/arkst/issues/719).

## Maintainer's explicit decision

Arkst is still under active development. **There is no requirement to keep
backward compatibility with earlier Arkst Rust APIs, internal IR schemas,
CLI inspection JSON or executable legacy wire.** Work/effort is not a reason
to reduce migration scope. If an older ADR/design is wrong or a better
architecture is justified, replace it fully rather than introducing a
permanent adaptation layer. A temporary private migration bridge is not
a contractual public interface and must disappear by G8.

**Important distinction:** Arkst's previous API compatibility is waived.
Quarkdown public-language compatibility and independently reviewed explicit
divergences remain governed by ADR-0016. Security, diagnostics, source maps,
atomic rollback, host capability restrictions and G0–G8 gate proof remain.

## Decisions D1–D4 (supersede this brief's prior proposals)

| Decision | Accepted direction | Verification implication |
| --- | --- | --- |
| **D1 — runtime/IR** | Accept [ADR-0022](../adr/0022-runtime-final-document-ir.md): frontend AST executes inside `arkst-engine` runtime; immutable `arkst-ir` contains completed backend-neutral document semantics only | Remove all executable IR carriers and renderer backdoors across G1–G8; reconcile historical ADR-0020 clauses |
| **D2 — wire** | New explicitly versioned, strictly validated final-only wire **v1**; **reject all unversioned historical Arkst IR JSON**. No legacy importer, migration decoder or dual format | Negative tests for unknown/missing version, executable structures, malformed source tables/ranges, recursive/depth/size bounds and host inertness |
| **D3 — Rust API / CLI** | Remove old `arkst-core::ast_to_ir`, public `Evaluator::evaluate(IrDocument)`, old mixed-IR serde and obsolete facade tests when replaced. `inspect --emit ir` emits only v1; no deprecation interval/compatibility wrapper | Reconcile all production/host/CLI/WASM callers and fixtures to new signatures. Published release version/change notes still follow ADR-0010 |
| **D4 — unsupported syntax** | Fail closed with explicit source-backed diagnostics; do not keep executable calls in published final IR or silently discard unknown syntax | Independent source/diagnostic/partial-output tests and Quarkdown semantic comparisons, including intentional divergences |

## Wire v1 and public API acceptance contract

- New final-only serialization schema is a distinct versioned DTO, not
  unversioned `IrDocument` or hidden implicit v1.
- Decode only valid v1 with deterministic source-table ordering and
  validated source IDs/ranges; reject invalid/unknown schema versions.
- Reject callable/executable fields at every nested level, unknown
  critical fields and duplicate/ambiguous source records; strict depth/size
  bounds and no implicit filesystem/network/process/host calls.
- Legacy *positive import* test cases and migrators are **out of scope**
  because backward compatibility is explicitly not required; old JSON
  rejection is a **required negative test**, not a reduced safety obligation.
- Public Rust and Typst APIs expose final-only types; remove old core
  AST-to-IR and evaluator executable entry points, including transitional
  re-exports and downstream assumptions.
- During G0–G6, old evaluator/IR can remain *temporary internal scaffolding*
  and a differential oracle; this is not a forever-supported public API.
  Complete and test the clean removal in G6/G7 and verify zero bridges at G8.

## Full migration / no scope reduction

G0: exhaustive AST/IR producer/consumer and reference oracle inventory;
G1: full typed RuntimeValue and target conversions;
G2: EvalSession/context/binding/transaction identities;
G3: all direct AST calls and control flow;
G4: nested AST, callable, source/resource/host body evaluation;
G5: mutable evaluator state -> immutable final DocumentConfig;
G6: final-only IR and pure Typst lowerer;
G7: strict v1 wire, public Rust/CLI/host/WASM replacements;
G8: cross-platform diagnostics, source-map, security, performance,
Quarkdown compatibility and exact-main post-merge acceptance.

Existing DYN01–DYN06 and S04/S05/S06/S07/S12/S13/S14/S15/S16 are bounded
proofs, not full G0 exit. Finish the missing scenarios and production paths.
The original #719 sequencing still applies: **G0 must pass before G1**.
No architecture cost tradeoff may waive requirements.

## Separate security and language compatibility decisions

- CI now reports **RUSTSEC-2026-0486**: `rustix 1.1.4` is unsound on
  zero-length xattr buffers; patched version is `>= 1.1.6`. Remediate
  the dependency independently; never suppress the security gate.
- #682's structured String conversion and nonportable JVM context-shaped
  representation remain an independent *Quarkdown semantic* review, not
  a reason to keep the legacy Arkst IR wire.
- #718 implementation may use merged provenance foundations, but its
  own language/reference tests and #719 integration proof remain required.

**Acceptance vs delivery:** the target policy is decided, while the new
runtime, final IR, wire, public API cutover and full G0–G8 proof are NOT
implemented by this documentation branch. Keep #746 OPEN and #719 OPEN.
Do not claim G0 completion or start G1 until the remaining G0 gate passes.
