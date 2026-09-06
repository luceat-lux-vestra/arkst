# Quarkdown v2.5.1 resource permission policy evidence (#296)

## Scope and identity

This note pins the upstream host-filesystem permission behavior that remains an
intentional compatibility/security divergence after the common logical
`VirtualProject` / `ResourceProvider` resolver was completed under #188.

- Arkst issue: #296
- Quarkdown compatibility target: v2.5.1
- Pinned upstream revision: `107ec3a9482f10d6f90d7580f8409b46a719d18e`
- Evidence merge: `e7da0718439ecf396fbfeceb133da872a940341a` (#300).
- The canonical #155 audit has consumed this policy decision. It does not authorize host
  filesystem access in `arkst-project`, `arkst-engine`, `arkst-core`, IR, or
  other platform-neutral compiler code.

## Pinned upstream sources

The conclusions below are derived from these exact files at the pinned
revision:

- `quarkdown-core/src/main/kotlin/com/quarkdown/core/permissions/Permission.kt`
- `quarkdown-core/src/main/kotlin/com/quarkdown/core/permissions/PermissionHolder.kt`
- `quarkdown-core/src/main/kotlin/com/quarkdown/core/permissions/MissingPermissionException.kt`
- `quarkdown-core/src/main/kotlin/com/quarkdown/core/ExitCodes.kt`
- `quarkdown-core/src/main/kotlin/com/quarkdown/core/context/file/FileSystem.kt`
- `quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Data.kt`
- `quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Ecosystem.kt`

No Quarkdown source or fixture is copied into Arkst; this note records only the
observable contract needed by the compatibility audit.

## Exact upstream permission contract

Quarkdown v2.5.1 defines distinct read capabilities:

- `ProjectRead`, exposed as `project-read`, authorizes files considered inside
  the root file system's working directory.
- `GlobalRead`, exposed as `global-read`, authorizes reads outside that project
  directory and, when no root file system working directory exists, all file
  reads.
- `Permission.DEFAULT_SET` contains `ProjectRead` and `NativeContent`; it does
  **not** contain `GlobalRead`.

`PermissionHolder.getReadPermission` chooses the required permission from the
resolved `File`: if `rootFileSystem?.workingDirectory` is absent it requires
`GlobalRead`; otherwise an `IOUtils.isSubPath(workingDirectory, file)` result
inside the root requires `ProjectRead` and anything outside requires
`GlobalRead`.

The FileSystem contract deliberately accepts both absolute and relative paths.
`FileSystem.resolve(path)` resolves a local file path and explicitly does not
check whether the file exists. A branched file system changes the current
working directory while retaining the original root used by permission
classification.

## Permission-before-existence ordering

The shared `Data.kt` `file(context, path, requireExistence)` helper establishes
an observable ordering:

1. resolve the supplied relative or absolute path through the current
   `FileSystem`;
2. call `context.requireReadPermission(file)`;
3. only after permission succeeds, check `file.exists()` when existence is
   required.

Therefore an outside-project path without `GlobalRead` fails the permission
check before Quarkdown determines whether the target exists. Missing-resource
and inaccessible/global-permission behavior are not interchangeable upstream.
`.read`, `.json`, `.listfiles`, `.filename`, and `.csv` use this shared file
path, and `.include` uses it for file fallback after loadable-library lookup.
`.includeall` inherits the same behavior through `.include`.

## Upstream failure surface

A failed permission check throws `MissingPermissionException`. Its rich message
identifies the missing permission and advises the CLI form `--allow
<permission-name>`. For an outside-project resource this means
`--allow global-read`.

The pinned `ExitCodes.kt` assigns missing-permission failure the process exit
code **72** (`MISSING_PERMISSION_EXIT_CODE = 72`). This is distinct from the
I/O error exit code and from the later existence or file-read failures.

## Arkst decision

Arkst intentionally does **not** emulate the upstream global host-filesystem
capability inside platform-neutral compiler code.

| Upstream behavior | Arkst classification |
|---|---|
| Source-relative/project-local resource access | Represented by the caller-supplied `VirtualProject` / `ResourceProvider` logical project model. |
| Absolute host paths or paths that escape the logical project | Intentional security divergence. They fail closed; document input cannot mint `global-read` authority. |
| Upstream `ProjectRead` / `GlobalRead` permission selection and exit-72 CLI failure | Recorded compatibility divergence, not a second evaluator permission system. Arkst keeps its deterministic typed project-resource diagnostics. |
| Native host resource/loadable-library discovery and ingestion | Completed separately under #298/#302. Supplying data into a `VirtualProject` is not equivalent to allowing arbitrary absolute document paths. |
| Public WASM/embedder resource binding and native/WASM parity | Deferred to #191; no browser URL, cwd, host path, or implicit filesystem fallback is introduced here. |
| Project data-file consumers such as `.listfiles`, `.filename`, `.csv`, bibliography | Remain #189-owned and must reuse the accepted logical/capability boundaries. |
| Process/environment access | Remains #190-owned and is not implied by filesystem permissions. |

This decision deliberately keeps the affected #155 resource rows conservative.
Documenting the divergence does not promote `.read`, `.json`, `.include`,
`.includeall`, or `.pathtoroot` to complete Quarkdown compatibility.

## Security invariants

1. Platform-neutral Arkst code does not acquire direct `std::fs`, cwd, process,
   environment, or network access from this reconciliation.
2. Absolute/global filesystem authority cannot be encoded by document input,
   callable capture, or serialized IR.
3. Existing host-path rejection and project-boundary checks remain fail-closed.
4. Completed native ingestion (#298/#302) and future WASM binding (#191) use explicit
   host/embedder boundaries; neither may silently weaken the logical resolver.
5. Compatibility status remains evidence-based: an intentional divergence is
   still a remaining compatibility gap even when its security policy is
   accepted.

## Canonical reconciliation

The canonical #155 resource audit/manifest and cross-audit reconciliation now consume
this record. Affected rows remain `PARTIAL`; `POLICY_DIVERGENCE:global-read` records the accepted
fail-closed global-read incompatibility without leaving completed #296 as an actionable
owner. #296/#300 remain historical policy/evidence provenance, while independently
owned gaps such as #189, #190, and #191 remain actionable where applicable.
