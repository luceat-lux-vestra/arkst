# ADR-0008: Configuration and Project Layout

- **Status:** Accepted
- **Date:** 2026-08-02
- **Owners:** Scribium maintainers
- **Related issues:** #1

## Context

Scribium needs a project-level configuration system. The config must support
multi-entry projects, output target selection, resource limits, and compatibility
profiles.

## Decision Drivers

- Minimal magic — explicit config over auto-detection
- TOML format (Rust ecosystem standard)
- CLI flags override file config
- Config file discovery from working directory upward

## Considered Options

### Option 1: Only CLI flags (rejected)

Unworkable for multi-entry projects. Each run would need repeated flags.

### Option 2: Package.json-style field in Cargo.toml (rejected)

Scribium is not a Rust project dependency. Coupling to Cargo.toml is misleading.

### Option 3: `scribium.toml` project file (chosen)

Top-level `scribium.toml` with optional project-level config. CLI flags
override file values. Config discovery walks up from entry file's directory.

## Decision

Use `scribium.toml` for project-level configuration. Config discovery: start
at entry file directory, walk up to filesystem root or git root. CLI flags
take precedence over file values.

## Consequences

### Positive

- Explicit, discoverable configuration
- Standard TOML format
- CLI flags override for ad-hoc builds

### Negative

- Another config file to create for non-trivial projects
- Config discovery adds complexity

### Risks

- Config file in parent directory may surprise users
- Mitigation: print discovered config path in verbose mode

## References

- `docs/ARCHITECTURE.md` (config model section)
- `crates/scribium-cli/src/config.rs`