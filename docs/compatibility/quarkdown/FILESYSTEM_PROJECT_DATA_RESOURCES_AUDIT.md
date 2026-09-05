# Issue #155 — Filesystem, project, data, and resource-backed audit

## Audit identity and scope

- **Arkst audit base:** `1a1fc7b1a978baa23d5eb0bfbef83ec49af5253f`
- **Quarkdown target:** v2.5.1 at `107ec3a9482f10d6f90d7580f8409b46a719d18e`
- **Authority:** the machine-checkable [filesystem/project/data/resource manifest](FILESYSTEM_PROJECT_DATA_RESOURCES_AUDIT_MANIFEST.tsv)
- **Parent tracker:** [#147](https://github.com/luceat-lux-vestra/arkst/issues/147)
- **Audit issue:** [#155](https://github.com/luceat-lux-vestra/arkst/issues/155)

This is an evidence and reconciliation record. It does not implement
Quarkdown resource features, change the compatibility target, redesign the
Typst backend, or add filesystem/network/process access. Post-audit production
ordering is defined by [#156](https://github.com/luceat-lux-vestra/arkst/issues/156).

The manifest is authoritative for the canonical status and ownership of every
enumerated row. The offline guard derives its row and ownership checks from the
manifest declarations; this document does not maintain a second count table.

## Discovery methodology

The sweep was performed against a detached checkout of the exact target SHA.
It searched the v2.5.1 public stdlib and core tree for declarations and hooks
that read, resolve, identify, import, include, register, or fetch resources,
including `fileSystem`, path resolution, file reads, media URLs, permissions,
environment access, nested subdocuments, and output-side resource conversion.

The pinned source families were:

- `Data.kt`: `.read`, `.pathtoroot`, `.listfiles`, `.filename`, `.json`, and
  `.csv`;
- `Ecosystem.kt`: `.include`, `.includeall`, `.subdocument`, and loadable
  library behavior;
- `Bibliography.kt`: bibliography file loading;
- `Primitives.kt`, `Document.kt`, `Markdown.kt`, and `Html.kt`: image, font,
  raw-body, link, and generated external-URL surfaces;
- `Process.kt`: `.env`;
- core `FileSystem`, permission, media, link-resolution, and subdocument
  registration contracts; and
- the pinned media converter and project/context paths that make remote fetch,
  nested identity, and host-boundary behavior observable.

The sweep also checked candidate names from the earlier stdlib and content
audits. `.filetree` is explicitly recorded as a negative discovery: its body
describes a visual tree and it does not read the named files. `.markdown` and
`.llmstxt` are likewise recorded because they accept body/configuration data or
emit external URLs but do not load resources. Date/time/system-clock searches
found no public v2.5.1 date/time QFunction; `.env`, host filesystem state, and
remote media are the applicable external-state surfaces.

No Quarkdown source, test, or fixture was copied or translated. Each manifest
row has a source URL containing the exact target SHA.

## Status and ownership result

The owned evaluator/data result is deliberately narrow:

- `.read`, `.json`, `.include`, `.includeall`, and `.pathtoroot` are `PARTIAL`. Their local,
  source-relative, in-memory subsets are evidenced, including strict text
  decoding, JSON conversion, nested source identity, active-stack cycle
  checks, repeated/shared bulk includes, fail-fast partial effects, include sandbox behavior,
  and logical project/subdocument root projection. Absolute/global FileSystem semantics,
  loadable libraries, complete upstream permission and diagnostic behavior, public WASM
  resource ingestion, and the full nested graph contract remain open.
- `.listfiles`, `.filename`, `.csv`, `.bibliography`, `.subdocument`, and `.env`
  remain `UNSUPPORTED`. The manifest states each absent contract and assigns its bounded
  follow-up; absence is not inferred merely from a missing high-level test.
- The VirtualProject/ResourceProvider model, logical normalization, project
  boundary, and host-determinism isolation are `SUPPORTED_SEMANTICS`: the
  semantic boundary is represented, while specific consumers and output layers
  remain separate gaps.
- Resource diagnostics/provenance, nested resource identity, and the current
  Typst source-context path are `PARTIAL` because the current evidence covers
  the in-memory or staging subset but not all language-visible consumers.
- The public WASM resource boundary is `DEFERRED`, with explicit M6/product
  sequencing and a bounded follow-up. WASM-capable core code is not evidence of
  a binding or end-to-end native/WASM compatibility claim.

The manifest uses explicit handoff rows for image/media, Markdown image
resource consumption, font configuration, ordinary links, raw Markdown,
`.llmstxt`, `.subdocumentgraph`, and `.filetree`. Those rows remain visible in
the audit without re-owning presentation, layout, structural-reference, or
body-only behavior already assigned to #154, #175, #181, or #182. CSV table
output is coordinated with #183; the #155-owned row is the missing data-file
loader boundary.

## Coherent resource model

Quarkdown v2.5.1 uses a context `FileSystem` with a working directory, root,
branches for nested contexts, and ProjectRead/GlobalRead permissions. Relative
paths are therefore source/context-relative in upstream; project/global
permission determines whether a resolved host path is readable. Remote media
is a separate URL/media path and is gated by network permission.

Arkst's current model is one in-memory logical resource model:

1. The host CLI establishes an explicit project root and builds a
   `VirtualProject` containing sorted source and asset stores.
2. `VirtualProject::resolve_resource_path` resolves a local reference from the
   calling source's logical parent. It rejects URI references and host absolute
   or Windows path forms, canonicalizes logical separators/components, and
   rejects project-root escape.
3. `VirtualProjectResourceProvider` maps the project to the engine's
   `ResourceProvider`. `.read` and `.json` read text; `.include` reads a source
   while retaining its target `SourceId`, path, source stack, and nested base.
4. The evaluator remains filesystem-, process-, and network-free. IR carries
   source identity/provenance; it does not introduce a backend-specific raw
   resource escape.
5. The pure Typst lowering boundary does not read resources. The native
   subprocess adapter receives an explicit canonical project root, mirrors the
   project into an isolated temporary tree, places the generated entry at its
   project-relative `TypstInput.entry_path`, and invokes Typst with `--root`.

This is coherent at the evaluator/provider boundary. It is not yet an
end-to-end claim for every consumer: current Markdown/Typst image lowering
classifies local references but does not prove VirtualProject asset existence,
and native Typst staging currently couples backend resources to a host mirror.
Those facts are recorded as resource/backend gaps rather than promoted to
evaluator support.

## Relative bases, identity, and nested loading

The audit distinguishes the following bases and identities:

| Concept | Current Arkst meaning |
|---|---|
| Resource root / project root | The logical root of `VirtualProject`; no host path is exposed to core. |
| Source root | The parent of the calling source's canonical logical path. |
| Entry document | The explicit entry `SourceId` and project-relative `VirtualPath`. |
| Nested document | A target `SourceId` plus canonical logical path returned by `read_source`; its source parent is the next relative base. |
| Resource identity | Source content is identified by `SourceId` and logical path; assets use canonical logical paths. Host canonical paths are adapter-only. |
| Backend entry | `TypstInput.entry_path` is logical/project-relative; the native adapter's mirror path is not evaluator resource identity. |

Current `.include` evaluation uses the post-resolution target source identity,
an active source stack for cycle detection, and a per-target nested base. A
repeated include is allowed when it is not active. Nested `.read` evidence
proves that the included source's identity, rather than the entry document or
process cwd, controls relative lookup. Ordinary subdocument links and graph
registration do not yet share this complete model; that gap is assigned to
#188 before #154 graph/output reconciliation.

## Boundary and security findings

The logical path implementation and native staging evidence cover the relevant
adversarial classes: `..` traversal, absolute paths, Windows drive/backslash
forms, repeated separators, `.` normalization, project-root escape, and native
symlink escape. The project provider returns typed boundary/not-found/invalid
reference errors, while the subprocess adapter rejects canonical/symlink paths
outside its explicit project root.

The audit does not claim that upstream and Arkst have identical path
contracts: upstream can resolve absolute/global paths under permission rules,
and upstream host path behavior has platform-specific details. Arkst's
fail-closed logical policy is an intentional architecture boundary, so rows
requiring full v2.5.1 absolute/global semantics remain partial or unsupported.
Directory traversal, metadata sorting, filename identity, data-format errors,
and inaccessible-host-file distinctions are not silently borrowed from the
host; they remain bounded follow-up work.

## Deterministic external inputs

- Core evaluation does not inherit the process cwd, OS temp directory, host
  filesystem, process environment, or network. Resource state is passed through
  `VirtualProject`/`ResourceProvider`.
- The native Typst adapter intentionally uses an isolated OS temporary mirror,
  but only after the evaluator boundary and with an explicit project root. This
  is backend execution state, not language resource identity.
- `.env` is an upstream process-environment surface and is currently
  unsupported in Arkst. It is assigned to #190 so future behavior must use
  explicit capability/injection or deterministic denial.
- Remote image/media and font URLs are real upstream network surfaces. The
  current Arkst policy rejects URI references and does not fetch them. The
  resource boundary is recorded here and media/layout implementation remains
  with #182/#175.
- No public current-date/current-time QFunction was found in the pinned
  v2.5.1 sweep. No date/time support claim is therefore hidden in a resource
  row.

## Native, WASM, Markdown, and Typst implications

The core/project/engine resource APIs are in-memory and structurally
WASM-capable, but the repository has no WASM resource binding or end-to-end
native/WASM resource fixture. Compile capability is not conformance evidence.
The bounded future boundary is #191; it must preserve logical paths,
`SourceId`, bytes/text, deterministic ordering, typed errors, and project
boundaries without filesystem/process/network fallbacks.

Markdown image parsing and current local-reference classification are evidence
for the consumer boundary only. They do not prove asset existence, media type,
Quarkdown image sizing, remote behavior, or rendered output; #182 owns those
surfaces. Ordinary links preserve URLs and are not fetch operations. The raw
`.markdown` body and generated `.llmstxt` URL are explicitly not resource
loaders.

The #24 contract is reconciled to current code rather than repeated from its
closed issue: `TypstInput.entry_path` is project-relative, CLI passes an
explicit canonical `TypstSourceContext.project_root`, staging mirrors the
project under an isolated temp root, and Typst receives `--root`. The current
gap is that backend asset/import resolution is tied to the native mirror rather
than the evaluator's VirtualProject provider, and no WASM backend path exists.
The ordered redesign/re-evaluation remains #187; no #187 work is started here.

## Historical #62 reconciliation

#62's closed implementation is not treated as proof of complete v2.5.1
conformance. Current code and tests confirm a bounded `.read`, `.json`, and
`.include` source-relative VirtualProject slice with nested source identity,
typed provider errors, cycle detection, repeated includes, and strict UTF-8
behavior. They do not confirm upstream absolute/global permission semantics, loadable library
names, `.listfiles`, `.filename`, `.csv`, `.bibliography`, or `.subdocument` graph behavior.
Current `.includeall` and `.pathtoroot` evidence is bounded to deterministic logical-project
semantics and does not claim those remaining host/library/WASM contracts.

The current provider is WASM-safe in its core design because it owns no host
filesystem access. That is distinct from a WASM binding, which is absent. The
manifest records this distinction and routes implementation to #188/#189/#191
without adding direct `std::fs`, cwd lookup, temp-dependent evaluator behavior,
or network access.

## Bounded follow-up ownership

- [#188](https://github.com/luceat-lux-vestra/arkst/issues/188): common
  logical project resolution, nested loading, source identity, subdocument
  resolution, and `.read`/`.json`/`.include` family closure;
- [#189](https://github.com/luceat-lux-vestra/arkst/issues/189): bounded
  project data-file and file-identity loaders (`.listfiles`, `.filename`,
  `.csv`, `.bibliography`), coordinated with #181/#183 consumers;
- [#190](https://github.com/luceat-lux-vestra/arkst/issues/190): explicit,
  deterministic `.env` capability/injection or rejection;
- [#191](https://github.com/luceat-lux-vestra/arkst/issues/191): deferred
  WASM project/resource binding;
- [#175](https://github.com/luceat-lux-vestra/arkst/issues/175),
  [#181](https://github.com/luceat-lux-vestra/arkst/issues/181),
  [#182](https://github.com/luceat-lux-vestra/arkst/issues/182),
  [#183](https://github.com/luceat-lux-vestra/arkst/issues/183), and
  [#187](https://github.com/luceat-lux-vestra/arkst/issues/187) retain their
  established layout, structural/reference, media, table, and Typst/backend
  ownership boundaries; and
- [#156](https://github.com/luceat-lux-vestra/arkst/issues/156) is the
  completed cross-audit reconciliation record; implementation sequencing now
  follows its canonical dependency graph.

The [offline guard](../../../crates/arkst-core/tests/filesystem_project_data_resources_audit.rs)
rejects unpinned provenance, missing schema/evidence, contradictory support
claims, unowned actionable gaps, omitted network/WASM/boundary fields, and
placeholder evidence without cloning or fetching Quarkdown.
