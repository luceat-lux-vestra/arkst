# Quarkdown v2.5.1 project data/file identity evidence (#189)

## Current bounded slice: `.filename`

This record pins the first #189 implementation slice against Quarkdown v2.5.1 at
`107ec3a9482f10d6f90d7580f8409b46a719d18e`. The remaining #189 families
`.listfiles`, `.csv`, and `.bibliography` are intentionally not claimed here.

Pinned `quarkdown-stdlib/.../Data.kt` uses the shared `file(context, path,
requireExistence = true)` helper. Observable ordering is resolve -> read
permission -> existence check. `.filename` therefore fails for a missing file;
it does not return a basename for an absent path. After that check, upstream
returns `File.name` when `extension=true` and `File.nameWithoutExtension` when
`extension=false`. Kotlin/JVM `nameWithoutExtension` uses the last `.` in the
final name, yielding `archive.tar` from `archive.tar.gz`, `plain` from `plain`,
an empty string from `.hidden`, and `trailing` from `trailing.`. The dotfile edge is
pinned at the pure logical-name helper boundary because a dot-prefixed token inside a
Quarkdown brace body is call-syntax ambiguous; source-level tests do not rewrite that
frontend grammar merely to manufacture a filename fixture.

Arkst represents only the project-bounded subset. `ResourceProvider::resource_metadata`
returns canonical logical identity for an existing resource without reading
bytes. `VirtualProjectResourceProvider` accepts both source and asset stores, so
a binary asset remains a valid file identity and no UTF-8 conversion is
performed merely to obtain its name. Resolution remains relative to the active
`SourceId`, including nested includes, and all host absolute/Windows/URI/project
escape references remain fail-closed.

The upstream `GlobalRead` behavior remains the accepted
`POLICY_DIVERGENCE:global-read` recorded by completed #296/#300/#304. Public
WASM resource binding/parity remains #191-owned. No `std::fs`, cwd, process,
environment, network, timestamp, permission, or native-path capability is
added to platform-neutral compiler code by this slice.
