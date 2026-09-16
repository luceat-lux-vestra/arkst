# Quarkdown v2.6 slides-tailored project creation compatibility

Issue: #356. Parent migration checklist: #311.

## Decision

The Quarkdown v2.6 `quarkdown create` slides-tailored starter-content change is **not applicable to Arkst's current surface**.

This is a current-surface classification, not a permanent exclusion and not an implementation claim.

Arkst currently exposes compiler/toolchain commands for building, validating, and inspecting existing inputs. It does not expose a project-creation or project-scaffolding command, and no separate Arkst project-template owner was found on the audited baseline. Adding a dummy `create` command only to mirror this upstream release-note item would expand Arkst's product surface rather than improve compatibility of a surface Arkst already owns.

If Arkst later adds a user-facing `create`, `init`, `new`, scaffold, template-materialization, or equivalent project-generation surface, this v2.6 behavior becomes a compatibility obligation for that surface.

## Official v2.6 claim

The official Quarkdown v2.6.0 release notes describe this change under **Slides-tailored project creation**:

- creating a new `slides` project via `quarkdown create` now generates starter content designed for presentations.

The public claim is therefore scoped to CLI/project-generation output. It does not state a compiler, evaluator, IR, or renderer semantic change.

Release evidence:

- release: Quarkdown v2.6.0;
- published: `2026-09-08T03:14:48Z`;
- official Linux x64 distribution SHA-256: `5b015e47c820d06ff6774eb700e60d77bc575819d4a0140cc7f1a115e7ce6dc4`.

No Quarkdown implementation source was inspected for this classification.

## Current Arkst evidence

Audited Arkst main:

- commit: `aa0bf15763e86ea88d8b04726f6b4984d8bc92f6`;
- tree: `d2305e8931198a3a2b1be56d384b542aa4562bac`.

At that baseline, `crates/arkst-cli/src/main.rs` defines exactly these top-level subcommands:

- `build`;
- `check`;
- `inspect`.

The command dispatch likewise contains only those three command variants. There is no `create`, `init`, `new`, scaffold, or project-template command in the CLI enum.

A repository-wide search for scaffold/template/init/project-starter equivalents found no separate project-generation owner or materialization API on the current main baseline.

This absence is used only to classify the **current** compatibility surface. It must not be turned into a regression invariant that forbids Arkst from gaining a scaffolding command later.

## Why no black-box starter-content oracle is required now

The exact v2.6 starter file set and generated starter text are **UNPINNED / UNKNOWN** in this classification.

That is intentional.

Arkst has no current surface that emits project starter content, so exact upstream starter text cannot affect current Arkst correctness. Probing and encoding that text now would create an unnecessary dependency on an upstream UX surface Arkst does not own.

A future Arkst project-generation feature must obtain fresh clean-room black-box evidence before claiming v2.6 conformance. At that point, the oracle should observe the actual generated artifacts rather than infer or copy implementation structure.

## Ownership boundary

This release-note item belongs, if Arkst ever implements an equivalent, to the host/CLI project-scaffolding layer.

It does **not** justify:

- evaluator or function-semantic state;
- backend-neutral IR fields;
- Typst/PDF lowering behavior;
- hidden defaults injected while compiling an already existing `slides` document;
- a dead compatibility alias that accepts `create` but does not own project generation.

The existing slides compiler semantics remain independently owned by their dedicated compatibility slices. A starter template may exercise those semantics, but it is not itself semantic authority for them.

## Future conformance trigger

Reopen or supersede this classification when Arkst gains any command/API that creates a project from a selected document type.

Before marking that future surface Quarkdown-v2.6-compatible, clean-room evidence should pin at least:

- how a `slides` project is selected;
- generated file/directory set;
- generated starter contents relevant to presentations;
- behavior for an existing/non-empty destination;
- deterministic output or explicitly variable fields;
- error/failure behavior at the CLI boundary.

Only observable public behavior should be reproduced. Upstream implementation source remains out of scope.

## Migration-checklist consequence

The v2.6 checklist row can be marked complete **as current-surface N/A** because:

1. the upstream change is explicitly a project-creation/starter-output feature;
2. Arkst's audited CLI owns no project-creation equivalent;
3. repository search found no separate project-template surface;
4. no compiler semantic delta is implied by the public release note;
5. the future trigger and clean-room proof obligation are recorded here.

This row being complete does not mean Arkst implements `quarkdown create` compatibility. It means the release-note item has been classified against Arkst's actual current ownership without inventing a new product surface.
