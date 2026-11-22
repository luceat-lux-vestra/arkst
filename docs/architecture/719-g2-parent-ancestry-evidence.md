# #719 G2-B parent ancestry evidence

Status: **G2-B2 design evidence**
Verified baseline: **Quarkdown v2.6.0**
Upstream tag commit: `22f3c1169d0b1356fb51d8f43e1833d3d815aab1`

This note freezes the ancestry semantics required before Arkst removes
`EvaluationContext::clone_scope_tree()`. It is intentionally narrower than the
later G2-C callable-capture migration.

## Upstream evidence

The verified Quarkdown v2.6.0 implementation establishes a live parent scope
tree rather than a fork-time copy of parent libraries.

Relevant pinned sources:

- `quarkdown-core/src/main/kotlin/com/quarkdown/core/context/ScopeContext.kt`
- `quarkdown-core/src/main/kotlin/com/quarkdown/core/context/SubdocumentContext.kt`
- `quarkdown-core/src/main/kotlin/com/quarkdown/core/context/ChildContext.kt`
- `quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/data/Lambda.kt`

Observed contract:

1. `ScopeContext` is a child context with a concrete parent reference.
2. `SubdocumentContext` starts with its own empty library set and resolves a
   missing function by consulting the parent context at lookup time.
3. The parent is therefore not frozen into the child when the child is created.
   A declaration subsequently made visible in the parent is visible to a child
   lookup unless the child has a nearer binding with the same name.
4. Lambda invocation forks from the definition context and layers caller
   libraries and parameters onto the invocation context. Lambda parameters are
   installed last and therefore retain the already-frozen precedence contract.

This is consistent with Arkst's existing G0 S10 evidence:
**definition capture fallback -> caller-visible overlay -> invocation parameter**.

## Consequence for Arkst G2-B

Arkst's current `clone_scope_tree()` is a transitional implementation detail,
not the target language contract. A child currently receives a recursive
snapshot of parent frames; that snapshot must not be promoted into a permanent
semantic guarantee.

The G2-B target is:

- frame-local bindings remain owned by one `ContextFrame`;
- lexical read ancestry is represented by stable `ContextFrameId` links;
- lookup walks the live identity chain;
- nearest-frame shadowing remains unchanged;
- child-local declarations do not leak into parents.

A future regression should explicitly freeze that a child created before a
parent declaration/redefinition observes the parent's current binding at the
later lookup point, subject to nearer child shadowing.

## Read ancestry is not write-through ancestry

The upstream live-read evidence does **not** authorize a mechanical replacement
of Arkst's snapshot tree with direct mutable parent-frame writes.

Arkst already has a stricter failure-atomicity contract than the upstream
reference in several evaluator paths. Existing callable/iteration behavior
stages writes and publishes successful reassignment through the established
owner/writeback boundary.

Therefore the parent-ID migration must preserve these independent properties:

- reads may resolve through live parent identities;
- a child may stage reassignment of an existing outer owner;
- staged writes must be visible to subsequent reads in that child;
- successful evaluation publishes through the existing owner boundary;
- failed evaluation does not leak staged writes;
- parameters and capture/caller lookup bindings are not promoted to real
  variable owners merely because they are readable.

Do not implement G2-B by calling `set_value` directly on the real parent frame
during child evaluation.

## Identity and lifetime contract

G2-B2 introduces private typed `ContextFrameId` values for the session
registry, transaction undo keys, frame lookup, and rollback routing. Raw
`usize` remains only the allocator's internal counter.

The next ancestry slice may use `ContextFrameId` as a lexical read link, but
must also define lifetime and reclamation explicitly:

- ordinary transient child frames must not force all frames to remain retained
  until compilation end;
- a live child must not lose access to a required lexical parent;
- definition/capture lifetime is G2-C work and must not be approximated by
  retaining every transient frame;
- `from_capture()` / `with_caller_overlay()` remain transitional until
  capture hydration and definition-context identity move to one explicit
  model.

## G2-B2 acceptance boundary

This slice is complete when:

- all frame identity surfaces in `evaluator.rs` use `ContextFrameId`;
- session registry keys are typed;
- transaction undo records and lookup routing are typed;
- no `scope: usize`, `scope_key`, `allocate_scope_key`, or
  `next_scope_key` frame-identity contract remains;
- runtime behavior is otherwise unchanged;
- `clone_scope_tree()` remains explicitly pending rather than being removed
  without the read/write/lifetime split above.

The following are **not** G2-B2 completion claims:

- parent snapshot removal;
- capture/caller-overlay identity migration;
- final frame lifetime/reclamation;
- G2 exit.
