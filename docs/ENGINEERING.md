# Engineering Standard — Arkst

This document defines the engineering quality contract for Arkst. It is a
project standard for compiler and toolchain implementation, review, testing,
and maintenance. It is not a formatting guide.

Arkst implementations are reviewed against the standard expected of a
mature production compiler/toolchain, even while the project is pre-alpha.
The current implementation may be incomplete or in migration; incompleteness
must be represented honestly rather than hidden by a narrow passing test.

## Authority and implementation state

The authority order for engineering work is:

1. the latest explicit maintainer or task constraint;
2. accepted and superseding ADRs;
3. `docs/ARCHITECTURE.md`;
4. product and domain compatibility specifications;
5. this document;
6. `docs/ROADMAP.md` for sequencing and context; and
7. current code and tests as evidence of implementation and migration state.

Current physical code does not override accepted target architecture. A
temporary physical location, migration artifact, or compatibility alias is
not evidence of target ownership.

## ADR history discipline

Accepted ADRs are historical decision records. Later decisions supersede or
amend them through explicit metadata, addenda, or new ADRs; they do not
silently rewrite the original decision, considered options, consequences, or
rationale.

## Rust and library implementation discipline

The following forward-looking rules apply to Rust implementation work:

- `unsafe` is prohibited by default. Introducing it requires an accepted
  architecture or security decision, or equivalent maintainer approval, and a
  narrowly localized boundary with documented safety invariants, justification
  for why safe Rust is insufficient, and appropriate tests and review.
- Production/library code must not use `unwrap` or `expect` for
  user-controlled input, malformed documents, recoverable operational failures,
  or normal compiler/backend failure paths. They are permitted in tests,
  bootstrap/test infrastructure, or where an invariant is statically or
  trivially guaranteed and the reason is obvious or documented.
- Hidden global mutable state is prohibited. Compiler behavior remains
  explicit and deterministic; state is passed through explicit ownership and
  context unless an accepted architecture decision says otherwise.
- Platform-independent compiler and project abstractions must not leak native
  `PathBuf` or filesystem assumptions across their accepted boundaries. Native
  paths belong in CLI, host, and native-adapter layers where appropriate.
- Libraries do not call `process::exit`. Use structured errors and
  user-facing diagnostics as separate concepts, with no silent fallback and no
  user-facing panic.

## Correctness over velocity

Correctness comes before shortcut implementation. Work must account for valid,
malformed, adversarial, boundary, and resource-amplifying input. In particular:

- preserve documented invariants explicitly;
- return a structured error or diagnostic instead of silently degrading
  semantics;
- produce deterministic results for identical normalized inputs and options;
- never expose a user-visible panic;
- do not game acceptance criteria by weakening coverage or special-casing a
  visible fixture; and
- treat an unsupported behavior as a deliberate state, not as successful
  compilation.

When fixing a bug, identify the violated invariant, repair the owning
abstraction or state transition, and add regression coverage. A local patch is
acceptable only when it is the correct owner of the behavior.

## Architecture over convenience

Accepted architecture is a constraint on implementation choices. In
particular, implementation work must preserve:

- one authoritative Markdown `BlockParser` owned by the Markdown frontend;
- Markdown frontend → Quarkdown grammar dependency direction;
- Quarkdown grammar-only responsibility for the Quarkdown grammar crate;
- the accepted `arkst-source`, `arkst-project`, `arkst-engine`,
  `arkst-diagnostics`, `arkst-compat`, and `arkst-ir` ownership;
- one backend-neutral `IrDocument`, without an unapproved HIR/MIR split;
- separate Typst lowering and concrete compiler execution;
- no core → Typst dependency;
- no `RawTypst`, `BackendRaw`, or generic backend-code escape hatch in the
  backend-neutral IR;
- HTML/xberg isolation behind `arkst-html`;
- Pandoc as an optional development/compatibility oracle only; and
- host-owned filesystem, network, and process boundaries.

Forbidden convenience changes include adding a `arkst-project` dependency
to `arkst-typst`, filesystem access inside platform-independent compiler
crates, Markdown implementing Quarkdown semantic evaluation, Typst-specific
types leaking into the IR, Quarkdown grammar depending on the Markdown AST,
and introducing a generic plugin framework without an accepted ADR.

Temporary architecture violations are not permitted unless explicitly
approved with a documented removal condition. If current physical placement
conflicts with the target, implement the smallest migration step that respects
the target; do not make the target conform to the accident.

## Invariant-driven design

Important invariants should be visible in types, ownership, control flow, and
tests. Prefer domain representations that make invalid states difficult to
construct when the added complexity is justified. Relevant invariants include:

- one owner for each parser, evaluator, lowering, and lifecycle state
  transition;
- explicit lifecycle and compatibility state rather than scattered booleans;
- backend-neutral semantic representations;
- deterministic normalization and stable ordering;
- exact source identity preservation;
- typed diagnostic categories and stable diagnostic ownership; and
- explicit distinction between the tracked upstream target and the verified
  compatibility baseline.

Type-level cleverness is not required for its own sake. The design should make
the important invariant easier to see, test, and maintain.

## No speculative abstraction

High quality does not mean maximal abstraction. Prefer the simplest design that
fully satisfies accepted architecture and documented foreseeable requirements.
Do not add frameworks, generic extension systems, abstraction layers,
configuration mechanisms, or traits solely for hypothetical future use.

Avoid unnecessary generic plugin systems, premature HIR/MIR proliferation,
backend abstractions with no concrete consumer, generic escape hatches, and
traits introduced only to make code look extensible. Every abstraction should
have a concrete ownership or variability reason that can be explained in the
review.

## No patch-shaped implementation

Do not accumulate chains of special-case conditionals for individual
regressions. Do not duplicate parser state across recognizers, represent
compiler states as strings when domain types are appropriate, or place
compatibility hacks in unrelated layers. Do not swallow an unsupported
construct as ordinary text merely to avoid an error. Do not claim completion
while knowingly deferring correctness in an untracked TODO. Do not copy and
paste lowering logic or update snapshots without semantic review.

A regression fixture is evidence of an invariant or contract problem. It is not
an instruction to hard-code that fixture.

## Compatibility is correctness

Quarkdown compatibility is part of Arkst's correctness contract, not a
best-effort optional feature. The product target is complete compatibility with
the publicly documented Quarkdown document language and document-observable
semantics for the tracked stable upstream release. This is independent
clean-room work; it is not implementation identity.

For the tracked target:

- a missing public-language feature is explicit compatibility debt;
- unsupported behavior must not silently masquerade as different Arkst
  semantics;
- deliberate divergences require visible documentation and rationale; and
- a current compatibility claim requires conformance evidence.

Full-target ambition must never be used to make false claims about the current
partial implementation. The compatibility matrix records verified claims; it
does not define a permanent selected subset.

## Source provenance is correctness

Source provenance is a correctness property. Future work must not casually:

- discard the original `SourceId`;
- reduce span precision without a documented reason;
- fabricate precise original spans from transformed or generated data;
- map generated offsets to source locations without reliable evidence; or
- lose source identity during parsing, normalization, evaluation, expansion,
  or lowering.

Where exact provenance is impossible, represent that uncertainty honestly with
fragment-level or optional provenance. Do not invent precision. This applies
especially to parser/container refactors, HTML normalization, evaluation and
expansion, generated Typst source maps, and backend diagnostics.

## Public API discipline

Public APIs should be the smallest useful stable surface. Keep implementation
details private, export stable domain concepts rather than migration artifacts,
and never make a type public solely because a test needs access. Public types
must follow accepted ownership boundaries and have documentation.

Do not prematurely freeze Rust types that ADR-0015 intentionally leaves
conceptual or deferred. A public API change needs an explicit compatibility,
ownership, and migration assessment.

## Error and diagnostic discipline

Typed Rust operational errors and user-facing diagnostics are separate
concepts. Preserve the diagnostic ownership and E1xxx–E9xxx ranges defined by
the accepted ADRs.

Require:

- structured diagnostics with stable categories and codes;
- producer-owned diagnostic semantics;
- typed domain errors rather than a generic string-error funnel;
- no panic-based user errors and no silent fallback;
- original-source spans when reliably available; and
- optional spans when no reliable original location exists.

Never fabricate a source location just to populate a field. Libraries do not
call `process::exit`; the native CLI/host owns process-level reporting and exit
behavior.

## Testing quality standard

Tests verify behavior and invariants, not private implementation structure. Use
the combinations appropriate to the change:

| Area | Required evidence |
|------|-------------------|
| Parser/frontend | focused units, malformed and boundary input, regressions, source-span assertions, and property/fuzz tests when parser risk warrants them |
| Quarkdown compatibility | independently authored fixtures, public-specification provenance, compatibility-level evidence, permitted black-box evidence where needed, and tracked-target/verified-baseline metadata |
| Engine/IR | semantic invariants, evaluation order, scope rules, determinism, normalization properties, and idempotency where promised |
| Typst lowering | deterministic golden output, source-map behavior, generated-source compilation through a verified compiler where appropriate, and the Typst compatibility corpus |
| Architecture migration | before/after regression evidence; observable behavior remains unchanged unless the task explicitly changes semantics |

Do not delete or weaken a test because a migration makes it inconvenient.
Malformed input, adversarial input, source provenance, and deterministic output
are first-class test concerns. Snapshot changes require semantic review and an
explanation in the change.

## Performance and resource discipline

Avoid obvious asymptotic regressions and gratuitous copying or allocation on
compiler hot paths. Enforce bounded processing and resource limits where an
untrusted document can amplify work. Measure non-obvious performance-driven
complexity, and include benchmark evidence when claiming a meaningful
optimization.

Do not sacrifice maintainability to remove an allocation without measurement
or an architectural constraint. Do not introduce unnecessary lifetime or
generic complexity merely because the project is written in Rust.

## Dependency discipline

Evaluate every new dependency for architectural ownership, maintenance
quality, license compatibility, security, platform and WASM implications,
transitive dependency cost, deterministic behavior, and whether a simpler
project-local solution is preferable. Dependencies are not categorically
forbidden, but they must solve a concrete problem and belong to the layer that
owns it. Do not add one merely to avoid understanding a small domain problem.

## Security and robustness

Preserve the accepted host/security boundaries. Compiler documents must not
gain arbitrary shell execution, process execution, filesystem access, network
access, or executable selection. Security capabilities belong to accepted
host/project architecture.

Untrusted or malformed documents must not reasonably cause uncontrolled
recursion, uncontrolled expansion, pathological unbounded evaluation,
panic-based failure, or path escape through compiler-core abstractions. Do not
weaken explicit resource-limit or virtual-path architecture for convenience.

## Architecture-review boundary for agents

Implementation agents may analyze current code, choose local details inside
accepted architecture, refactor locally when required by the assigned task,
add tests, and improve naming/internal structure within scope.

Agents must not independently decide:

- new crate ownership or dependency direction;
- a new semantic layer or IR tier;
- compatibility-policy exceptions or permanent upstream divergences;
- security-model changes;
- generic plugin/extension mechanisms or backend escape hatches;
- breaking public API strategy; or
- architecture-wide migration sequencing.

When one of these decisions becomes necessary, stop architectural invention,
document the blocking question and its evidence, and request architecture
review. Human review and merge remain the authority boundary for automated
adaptation work.

## Review evidence

Every implementation change should make it possible to answer:

- Which accepted contract or invariant does this change implement or preserve?
- What malformed, boundary, regression, provenance, and security cases were
  considered?
- Which compatibility evidence supports any changed claim?
- Which checks were run, and what was intentionally not run?
- Does the change alter public API, dependencies, security boundaries, or
  architecture ownership?

The answer belongs in the PR description or linked design record. A passing
narrow test is necessary evidence when relevant, but never the complete quality
argument.
