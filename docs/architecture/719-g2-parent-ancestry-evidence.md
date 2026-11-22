# #719 G2-B parent ancestry evidence

Status: **G2-B2 evidence accepted; G2-B3 implementation candidate**
Verified baseline: **Quarkdown v2.6.0**
Upstream tag commit: `22f3c1169d0b1356fb51d8f43e1833d3d815aab1`

This note freezes the ancestry semantics used to remove Arkst's recursive
`EvaluationContext` scope snapshots. It remains intentionally narrower than
the later G2-C callable-capture migration.

## Upstream evidence

The verified Quarkdown v2.6.0 behavior establishes a live parent scope tree
rather than a fork-time copy of parent libraries.

Relevant pinned sources reviewed for this architecture decision:

- `quarkdown-core/src/main/kotlin/com/quarkdown/core/context/ScopeContext.kt`
- `quarkdown-core/src/main/kotlin/com/quarkdown/core/context/SubdocumentContext.kt`
- `quarkdown-core/src/main/kotlin/com/quarkdown/core/context/ChildContext.kt`
- `quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/data/Lambda.kt`

Observed contract:

1. `ScopeContext` is a child context with a concrete parent reference.
2. `SubdocumentContext` starts with its own library layer and resolves a
   missing function against the parent context at lookup time.
3. Parent visibility is therefore live rather than frozen at child creation.
   A later parent declaration/redefinition is visible unless a nearer child
   binding shadows it.
4. Lambda invocation layers caller-visible bindings and parameters over the
   definition environment. Parameters remain the highest-precedence lookup
   layer.

This remains consistent with Arkst's G0 S10 contract:
**definition capture fallback -> caller-visible overlay -> invocation parameter**.

## G2-B3 implementation model

The G2-B3 candidate removes the recursive
`Option<Box<EvaluationContext>>` parent tree and `clone_scope_tree()`.

One lexical frame now owns:

- its typed `ContextFrameId`;
- frame-local variables/functions/extensions/library inspection state;
- variable-owner and forwarded-owner metadata;
- invocation-local assigned-value staging;
- an optional `ContextFrameLink` to its live read parent.

`ContextFrameLink` carries both the typed identity and a strong
`Rc<RefCell<ContextFrame>>` handle. The handle is deliberate: a child may
continue to require lexical read access after the parent `EvaluationContext`
object is dropped.

The `EvalSession` registry is **not** converted into a retain-all arena.
`EvaluationContext::drop` removes its frame's registry entry as before. A
required lexical parent survives only while an actual descendant frame holds
the strong parent link. Because ancestry is parent-only, this does not create a
reference cycle.

## Live reads and shadowing

The following read surfaces walk `ContextFrameLink` ancestry rather than a
copied context tree:

- variable lookup;
- function lookup;
- implicit lambda parameters;
- visible lambda scope;
- extension target lookup;
- binding collection used by capture/caller overlay;
- extension-target collection;
- library-inspection collection;
- variable-owner/forwarded-owner discovery.

Nearest-frame lookup shadowing is retained.

A focused regression creates a child, mutates/adds bindings in the real parent
after child creation, and proves that the child observes the current parent
state while its own later local bindings continue to shadow parent changes.

## Read ancestry is not write-through ancestry

Live parent visibility does **not** authorize immediate mutation of the real
parent during child evaluation.

Arkst keeps its stricter failure-atomic write policy:

1. `assign_value` resolves real/forwarded owners through the live frame chain.
2. If the owner is outside the current frame, the new value is staged in the
   current frame's `assigned_variables` rather than written through.
3. The staged value is visible to later reads in that child.
4. Parameters remain authoritative lookup bindings and continue to shadow a
   staged write targeting an outer owner.
5. A successful callable boundary relays the staged assignment outward one
   caller boundary.
6. Publication becomes final only when that boundary reaches the actual owner
   frame.
7. Existing transaction journaling remains responsible for rollback of writes
   that have reached a live frame.

This preserves the old publication timing without keeping a copied parent
scope tree.

New local declarations still become owners only in their exact frame.
Definition captures, parameters, and copied caller-overlay bindings do not gain
owner status merely because they are readable.

## Lifetime and reclamation

G2-B3 explicitly proves both sides of the lifetime contract:

- dropping a parent `EvaluationContext` removes that frame from the
  `EvalSession` registry;
- a still-live child continues reading that parent through its direct
  `ContextFrameLink`;
- once the descendant chain is dropped, those strong links are released
  naturally.

This avoids retaining every transient invocation frame until compilation end.

The separate G2-C definition-capture problem remains. `from_capture()`
currently hydrates a temporary definition context and
`with_caller_overlay()` bridges its frame into the caller session through a
strong read link. G2-C must replace durable copied capture environments with an
explicit definition-context identity/lifetime model without weakening resource
capability restrictions.

## Transaction rollback

Rollback continues to use typed frame identities in the compilation-scoped
registry.

The former regression that mutated a copied snapshot parent now mutates the
actual live parent frame while a child-owned invocation transaction is active.
Rollback through the child context resolves the parent's `ContextFrameId` and
restores that real frame. This directly tests the architecture that remains
after snapshot removal.

## G2-B3 acceptance boundary

G2-B3 is complete only when the exact candidate proves all of the following:

- `EvaluationContext.parent`: **0**;
- `clone_scope_tree()`: **0**;
- `clone_context_frame()`: **0**;
- variable/function/extension/library/lambda reads use frame ancestry;
- owner discovery uses the same live ancestry;
- outer-owner writes remain staged until successful publication boundaries;
- child-after-parent-update live visibility is covered;
- child shadowing is covered;
- parameter-over-staged-write precedence is covered;
- parent-frame lifetime without registry retention is covered;
- parent-frame transaction rollback is covered;
- existing callable failure/writeback, capture precedence, resource capability,
  and platform regression suites remain green.

The following remain **outside G2-B3**:

- replacing `RuntimeCallableCapture` copied variables/functions with a durable
  definition-context identity;
- removing `from_capture()` / caller-overlay hydration;
- final G2-C frame lifetime for durable closures;
- G2 exit acceptance.
