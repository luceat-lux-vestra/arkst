# Clean-Room Compatibility Policy

## Purpose

Arkst targets complete compatibility with the publicly documented Quarkdown
document language and document-observable semantics of the tracked stable
release. This policy governs how that target is achieved without using or
referencing the original Quarkdown implementation code. Clean-room
independence is an implementation constraint; it does not turn public language
features into permanent exclusions. Current claims still require evidence in
the compatibility matrix.

## Permitted Sources

Implementation may derive requirements only from:

1. **Public user-facing syntax documentation**
   - Official Quarkdown syntax guides
   - Public specification documents
   - Published articles explaining syntax

2. **Public CLI behavior**
   - Observed command-line interface behavior
   - Documented CLI flags and exit codes
   - Published help text

3. **Public input/output examples**
   - Example files in public repositories (read-only observation)
   - Tutorial output samples
   - Independently authored training materials

4. **Independently authored specifications**
   - Compatibility specifications written by Arkst contributors
   - Conformance test inputs authored independently
   - Semantic analysis based on observed behavior

5. **Black-box conformance observations**
   - Input → output observations through the official implementation
   - Error message format observations (not exact text copy)
   - Performance characteristics

## Prohibited Actions

- Copying Quarkdown source code (any language)
- Translating Kotlin/JVM code to Rust
- Copying internal AST structure definitions
- Copying test fixtures from Quarkdown repositories
- Copying themes, CSS, HTML templates, icons
- Copying documentation sentences, comments, or error messages
- Copying commit messages or CHANGELOG entries
- Importing code from quarkdown-wasm or related repositories
- Using Quarkdown implementation code as a reference during implementation
- Copying implementation-specific behavior without an independently documented
  compatibility need

## Required Records

Each compatibility feature must record:

| Field               | Description                                                  |
|---------------------|--------------------------------------------------------------|
| Specification source| URL or citation of the public source                         |
| Independently       | Test input authored by Arkst contributors                 |
| authored input      |                                                              |
| Observed reference  | Output from the official Quarkdown implementation            |
| behavior            |                                                              |
| Arkst behavior   | Arkst's output for the same input                         |
| Compatibility level | Parsed / Semantically supported / Output-equivalent /        |
|                     | Known divergence / Unsupported                               |
| Known divergence    | Documented behavioral differences and rationale              |

## Compatibility Levels

| Level                | Definition                                               |
|----------------------|----------------------------------------------------------|
| Unsupported          | Not handled, produces explicit diagnostic                |
| Parsed               | Accepted syntactically, behavior undefined or rejected    |
| Semantically         | Arkst's semantics match specified behavior             |
| supported            |                                                          |
| Output-equivalent    | Typst output matches reference output for tested cases    |
| Known divergence     | Deliberate semantic difference with documented rationale  |

## Audit

Before any release, a provenance audit must confirm:

1. No prohibited source material exists in the repository
2. All compatibility features have provenance records
3. All compatibility test fixtures have independently authored inputs
4. Known divergences are documented and justified
5. The NOTICE and third-party license files are current
