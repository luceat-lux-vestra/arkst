# Quarkdown v2.5.1 native resource/library ingestion evidence (#298)

## Scope and identity

This note pins the native host ingestion behavior that remains outside Arkst's
platform-neutral `VirtualProject` / `ResourceProvider` /
`LoadableLibraryProvider` semantics.

- Arkst issue: #298
- Quarkdown compatibility target: v2.5.1
- Pinned upstream revision: `107ec3a9482f10d6f90d7580f8409b46a719d18e`
- Current Arkst baseline for this evidence slice:
  `e7da0718439ecf396fbfeceb133da872a940341a`
- This slice records the host boundary only. It does not add filesystem,
  process, environment, cwd, or network access to `arkst-project`,
  `arkst-engine`, `arkst-core`, IR, or other platform-neutral crates.

## Pinned upstream sources

The conclusions below are derived from these exact Quarkdown files at the
pinned revision:

- `quarkdown-cli/src/main/kotlin/com/quarkdown/cli/exec/ExecuteCommand.kt`
- `quarkdown-cli/src/main/kotlin/com/quarkdown/cli/exec/Execute.kt`
- `quarkdown-cli/src/main/kotlin/com/quarkdown/cli/lib/QdLibraries.kt`
- `quarkdown-cli/src/main/kotlin/com/quarkdown/cli/PipelineInitialization.kt`
- `quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/external/QdLibraryExporter.kt`
- `docs/importing-external-libraries.qd`

No Quarkdown implementation source or fixture is copied into Arkst.

## Exact upstream library-directory contract

Quarkdown v2.5.1 exposes `-l <dir>` / `--libs <dir>` as the external-library
directory option. When the option is omitted, `ExecuteCommand` selects the
installation layout's Quarkdown library directory, documented as
`<install directory>/lib/qd`.

`QdLibraries.fromDirectory` performs the host discovery:

1. the directory must exist;
2. the path must be a directory;
3. only its direct children are inspected (`File.listFiles()`); there is no
   recursive library-directory walk;
4. only files whose extension is exactly lowercase `qd` are selected;
5. the library name is the file's `nameWithoutExtension`;
6. each selected file becomes a `QdLibraryExporter` whose reader opens that
   file when the library is loaded.

The discovery code does not sort `listFiles()` before converting exporters to
a set. Arkst must not inherit host directory enumeration order as semantic or
provenance order.

`Execute.kt` catches discovery exceptions, logs the message as a warning, and
continues with an empty external-library set. This means an invalid configured
library directory is not an upstream pipeline-fatal error at that boundary.
Arkst may deliberately choose stricter fail-fast host diagnostics, but such a
choice is a recorded CLI/host divergence rather than an evaluator semantic
change.

`PipelineInitialization` exports the discovered external libraries before
constructing the pipeline context. `.include {name}` therefore observes a
preconstructed loadable-library set; document evaluation does not perform
library-directory discovery itself.

`QdLibraryExporter` creates a library whose name is the discovered filename
stem and whose `onLoad` action reads/includes the `.qd` source. The host file
reader is outside the language-level library registry.

## Current Arkst native resource ingestion

Arkst already has a substantial explicit native project-ingestion boundary in
`crates/arkst-cli/src/commands.rs`.

`load_single_file_project`:

- validates the requested entry extension;
- canonicalizes the requested entry and logical project root;
- rejects an entry whose canonical target escapes that root;
- recursively enumerates the project root through `collect_project_files`;
- sorts every directory's entries by filename before traversal;
- avoids recursive directory-symlink aliases and excludes symlink escapes;
- eagerly reads project files at the CLI host boundary;
- inserts recognized source files and all bytes as assets into a
  `VirtualProjectBuilder`;
- builds the complete in-memory `VirtualProject` before calling
  `arkst_core::compile`.

The project builder itself remains I/O-free. It sorts logical sources/assets
before construction, allocates deterministic source IDs, and already supports
`add_loadable_library(name, source)`. Library names are exact case-sensitive
semantic keys; empty and duplicate names fail atomically during `build`.

## Current gap

The native CLI does not yet connect a host library directory to
`VirtualProjectBuilder::add_loadable_library`:

- `crates/arkst-cli/src/main.rs` exposes no `-l` / `--libs` option;
- `crates/arkst-cli/src/commands.rs` has no native library-directory discovery
  or `add_loadable_library` call;
- consequently a CLI compilation cannot currently populate the in-memory
  loadable-library registry from external `.qd` files, even though the
  platform-neutral registry and evaluator dispatch already exist.

This is the bounded #298 implementation gap. It is not #188 resolver work and
it is not evidence that `VirtualProject` itself should gain filesystem access.

## Accepted Arkst native-ingestion contract for the implementation slice

The follow-up implementation should use this contract:

1. `-l <dir>` / `--libs <dir>` is an explicit trusted-CLI authority. Arkst does
   **not** implicitly probe a global installation library directory until an
   Arkst-owned install-layout contract exists.
2. The supplied directory is resolved by the CLI/native adapter before
   evaluation. A missing, non-directory, unreadable, or otherwise invalid
   directory is a fail-fast CLI error rather than a warning plus silent empty
   registry. This is an intentional host-diagnostic divergence from the pinned
   upstream behavior.
3. Discovery is non-recursive and accepts only direct children with the exact
   lowercase `.qd` extension, matching the pinned upstream selection rule.
4. Candidate entries are ordered by filename before ingestion. Host directory
   enumeration order must not affect library provenance IDs or diagnostics.
5. The semantic library name is the UTF-8 filename stem. Empty or invalid
   names fail before evaluation; duplicate semantic names remain rejected by
   the existing `VirtualProjectBuilder` contract.
6. Library source is eagerly read as UTF-8 before `VirtualProject::build`.
   Unreadable or invalid-UTF-8 source is therefore a deterministic host
   ingestion error, not a lazy evaluator-time filesystem failure.
7. A library entry that resolves through a symlink outside the explicitly
   supplied library directory is rejected. Supplying a directory must not
   silently confer authority over arbitrary paths reachable by symlink.
8. `build`, `check`, and `inspect` must consume the same native ingestion
   contract so command choice cannot change library availability.
9. Once ingested, existing exact case-sensitive library dispatch, detached
   provenance IDs, caller-relative resource bases, and library-before-file
   `.include` behavior remain unchanged.
10. Document content cannot select or widen the host library directory.

## Ownership boundaries

- #298 owns this native host discovery/ingestion surface.
- #296 owns the accepted upstream `ProjectRead` / `GlobalRead` compatibility
  divergence and must not be used to authorize arbitrary host paths here.
- #191 owns public WASM/embedder resource and library binding/parity.
- #189 owns additional project data-file consumers.
- #199/#181 own subdocument graph registration/output.

## Security invariants

1. Platform-neutral crates stay free of direct host filesystem/cwd/process/
   environment/network discovery.
2. Filesystem enumeration and file reads occur only in the trusted native CLI
   adapter and finish before platform-neutral evaluation.
3. Explicit `--libs` authority is bounded to its canonical directory and does
   not follow escaping symlinks.
4. No implicit current-directory, environment-variable, installation-layout,
   or network fallback is introduced by #298.
5. This evidence slice does not promote any canonical compatibility status.

## Follow-up

The next bounded #298 slice should implement the accepted CLI ingestion
contract with adversarial native-host fixtures for deterministic ordering,
non-recursion, exact extension/name handling, missing/non-directory inputs,
invalid UTF-8, and symlink escape. Canonical #155 audit ownership remains #298
until that executable host behavior is merged and reconciled.
