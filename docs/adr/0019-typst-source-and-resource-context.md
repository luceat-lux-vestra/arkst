# ADR-0019: Typst Source and Resource Context

- **Status:** Accepted
- **Date:** 2026-08-19
- **Owners:** Scribium maintainers
- **Related issues:** #24
- **Related ADRs:** 0005, 0008, 0015

## Context

The native Typst subprocess backend originally wrote generated source to an
unrelated temporary `input.typ`. That preserved output isolation, but made a
future relative resource reference depend on the implementation's temporary
directory rather than on the Scribium source document.

Scribium already has a logical, filesystem-free `VirtualProject`. Native
execution additionally needs a bounded physical read capability while keeping
generated source and output isolated from the source tree.

## Decision

`TypstInput.entry_path` means the normalized, project-root-relative logical path
of the Scribium source entry. For example, when the explicit project root is
`/work/docs` and the entry is `/work/docs/manual/chapter1/main.qd`, the backend
entry path is `manual/chapter1/main.qd`.

The native subprocess backend may be configured with:

```rust
TypstSourceContext {
    project_root: PathBuf,
}
```

The project root is an explicit physical read boundary. It is never inferred
from `std::env::current_dir()`. The backend validates and normalizes the
logical entry path through the existing `VirtualPathBuf` rules. Absolute,
root-escaping, Windows-style, and empty entry paths are rejected.

For a context-backed compile, the backend:

1. creates a unique temporary build directory;
2. canonicalizes the explicit project root;
3. mirrors the project tree into a temporary staging directory;
4. rejects any source or symlink whose final canonical target is outside the
   project root, including both file and directory symlinks;
5. writes generated Typst only at the mirrored logical entry directory, using
   a `.typ` filename that does not shadow an existing source resource; and
6. invokes the pinned Typst CLI as:

   ```text
   typst compile --root <temporary-mirror> \
     <temporary-mirror>/<logical-entry>.typ <temporary-build>/output.pdf
   ```

Relative resource paths are interpreted in Scribium source/project context,
not relative to an implementation-specific temporary build directory. The
mirror is a read-context snapshot; the original project tree remains
untouched. The PDF is read from the separate temporary output location and the
temporary directory is cleaned on success and normal failure paths.

When no source context is configured, self-contained generated Typst remains
supported. The backend still validates `entry_path`, but does not treat its
temporary directory as a source root. Filesystem resources are therefore
unavailable unless an explicit context is supplied.

## Security boundary

Logical resource resolution is project-relative: the entry directory is joined
with the resource path and `..` may move only within the project root. Absolute
and root-escaping resource paths are rejected by the logical path boundary.
The native mirror additionally canonicalizes existing filesystem targets, so a
lexically in-root symlink cannot expose an out-of-root file. Typst receives
`--root` set to the mirror, which is the final native boundary for generated
Typst resource access. Compiler diagnostics redact the temporary build prefix.

The source tree is a read boundary, not a build directory. No generated `.typ`,
`.pdf`, or temporary metadata is written into it.

## Alternatives considered

### Temporary input plus `--root`

Rejected. Typst requires the input file to be contained by the root, and the
input file's physical directory controls relative imports and resource paths.

### Absolute host paths in generated Typst

Rejected. This would leak host paths, reduce reproducibility, complicate
cross-platform behavior, and make later sandboxing harder.

### Symlinked temporary mirror

Rejected as the default. Typst follows the symlink target, so a temporary root
containing links to the source tree would not itself enforce the source
boundary. The implementation copies the tree and converts permitted internal
symlinks into regular staged entries.

### Writing generated source beside the original entry

Rejected. It violates source-tree immutability and can leave generated files or
partial output after failures.

## Consequences

The subprocess adapter performs a bounded source-tree snapshot per compile.
This favors source semantics, boundary enforcement, and portability over
minimal copying cost. Pure lowering, `VirtualProject`, and WASM compilation
remain filesystem-free. The CLI supplies the current host-selected physical
root and the logical `VirtualProject.entry()` path; future project-root
discovery can evolve independently of this backend contract.

This ADR establishes resource context only. Markdown image lowering,
Quarkdown resource built-ins, remote loading, package resolution, caching, and
an in-process Typst backend remain separate work.

## Follow-up work

- Add resource-aware Markdown and Quarkdown features on this contract.
- Revisit CLI project-root discovery when `scribium.toml` configuration is
  implemented.
- Reassess staging cost and a more capable native resource adapter only with a
  separate architecture and security review.

## Implementation status addendum (2026-08-23)

The original decision and follow-up scope above are preserved as historical
context. The current M2 implementation applies this Typst source-context
contract to bounded project-relative Markdown image lowering. Source-relative
image resolution, project-boundary rejection, symlink handling, and temporary
Typst mirror separation are covered by the backend integration tests.

Quarkdown `.read`, `.json`, and `.include` use a separate semantic resource
path: `scribium-engine` owns the engine-neutral `ResourceProvider` interface,
while `scribium-core` owns the adapter from `VirtualProject` to that interface.
Those built-ins therefore remain filesystem-free at the compiler boundary and
do not depend on the Typst subprocess mirror. Their source-relative resolution,
project-boundary rejection, and nested source identity are covered by core and
engine resource tests.

Remote loading, package resolution, caching, directory/data families beyond
the bounded built-ins, and an in-process Typst backend remain separate or
deferred work; this addendum records the implemented boundaries without
widening or merging their ownership.
