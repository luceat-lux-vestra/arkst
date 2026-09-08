# Quarkdown v2.5.1 project data/file identity evidence (#189)

## Implemented bounded slices: `.filename` and `.listfiles`

This record pins the bounded #189 implementation slices against Quarkdown v2.5.1 at
`107ec3a9482f10d6f90d7580f8409b46a719d18e`. The remaining #189 families `.csv` and `.bibliography` are intentionally not claimed here.

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
bytes. `VirtualProjectResourceProvider` accepts source, asset, and logical-directory
identity, so binary assets and explicit or inferred directories remain valid
identity targets without UTF-8 conversion merely to obtain a name. Resolution remains relative to the active
`SourceId`, including nested includes, and all host absolute/Windows/URI/project
escape references remain fail-closed.

The upstream `GlobalRead` behavior remains the accepted
`POLICY_DIVERGENCE:global-read` recorded by completed #296/#300/#304. Public
WASM resource binding/parity remains #191-owned. No `std::fs`, cwd, process,
environment, network, timestamp, permission, or native-path capability is
added to platform-neutral compiler code by these slices.

## Bounded `.listfiles` contract

Pinned `Data.kt` defines `directories=true`, `recursive=false`, optional
regex `pattern`, `fullpath=true`, `sortby=NONE`, and `order=ASCENDING`.
`NONE` returns an unordered set after presentation mapping; `NAME` lowercases
filenames before applying the pinned human-friendly alphanumeric comparator,
while `LAST_MODIFIED` depends on host timestamps. `fullpath=true` returns
native absolute paths.

Arkst implements the deterministic project-bounded subset whose inputs already
exist in `VirtualProject`: directories inferred from source and asset path
prefixes plus explicit host-supplied empty-directory identity, direct/recursive
enumeration, directory inclusion or exclusion,
`fullpath:false`, `sortby:none`, and an ASCII-bounded `sortby:name` mode.
`sortby:none` preserves the existing set-like bare-name deduplication behavior;
its internal order is deterministic but is not claimed as upstream enumeration-
order parity. `order` has no effect for `sortby:none`, matching upstream NONE.
For `sortby:name`, Arkst applies ASCII lowercase-before-compare semantics,
numeric chunks, leading-zero length tie-breaking, and ascending/descending
ordering; the ordered NAME result preserves duplicate bare names rather than
collapsing them as NONE does. This bounded comparator slice merged in #307 at
`f1b2b969f98f95d5961513fc37f802a0e94d2ad7`.

Explicit empty-directory identity is represented in `VirtualProject` and the
native CLI project-ingestion boundary. `pattern`, native absolute `fullpath:true`, and
timestamp-backed `sortby:lastmodified` still fail closed instead of being
approximated. Non-ASCII `sortby:name` also fails closed: exact Kotlin/JVM
Unicode `lowercase()` plus comparator parity has not been proven and remains
explicit #189 compatibility debt. A file passed as the directory target fails
as not-a-directory; an absent or unregistered target fails as not found.
Impossible in-memory trees that imply the same logical path is both a file and
directory fail as an internal provider-contract error rather than choosing one
interpretation.

The name-sorting regression evidence covers ordinary numeric ordering,
leading-zero ties, descending order, duplicate-name preservation, mixed ASCII
case, and non-ASCII fail-closed behavior in
`crates/arkst-core/tests/quarkdown_resource_builtins.rs` and
`crates/arkst-core/tests/quarkdown_listfiles_name_sort_189.rs`.

`ResourceProvider::list_directory` is opt-in and defaults to
`UnsupportedOperation`, so existing/custom providers gain no directory
observation capability implicitly. The production provider enumerates only immutable project source/asset paths and
explicit logical-directory identities. No `std::fs`, cwd, native path,
timestamp, permission, environment, process, or network state enters the
evaluator/project boundary.
