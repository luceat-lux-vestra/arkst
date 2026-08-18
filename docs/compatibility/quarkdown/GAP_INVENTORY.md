# Quarkdown v2.5.1 Public-Language Gap Inventory

## Review snapshot

- **Tracked target:** Quarkdown `v2.5.1`
- **Resolved tag commit:** `107ec3a9482f10d6f90d7580f8409b46a719d18e`
- **Repository:** [`iamgio/quarkdown`](https://github.com/iamgio/quarkdown)
- **Review date:** 2026-08-18
- **Scribium comparison head:** `6a98df1a548e27fa61cb80b5f78dcc269187b0c5`
  (the exact current-main snapshot for this review)
- **Rushdown:** unchanged at
  `e5eb4e4446541ea0ed53111c1b37e779283ff57c`

This is an evidence-backed inventory of the public language surface, not a
claim that parsing implies compatibility. A row is compatible only when the
observable behavior in the row is implemented through evaluation and the
relevant output boundary and is covered by Scribium tests. The inventory uses
the v2.5.1 tag sources and public documentation/API pages; upstream source and
tests were inspected as behavioral evidence under the clean-room policy, not
copied or translated.

Status meanings:

- **Compatible:** the reviewed surface has tested semantic support at the
  stated boundary.
- **Partially compatible:** a bounded subset works, while documented behavior
  remains missing or intentionally different.
- **Unsupported:** the surface is known and may parse/preserve, but Scribium
  does not provide its semantics.
- **Scribium extension:** Scribium behavior without a corresponding public
  v2.5.1 declaration; it is not counted as upstream conformance.
- **Intentionally deferred:** a known gap assigned to a later milestone or
  blocked on a host/architecture boundary.

Classification index:

| Classification | Inventory rows |
|---|---|
| Compatible at the stated bounded boundary | Logical and comparison operations |
| Partially compatible | Function declarations/calls; variables; conditionals; lambdas/callables; iteration; collections; Dictionary/Pair/Range; type/value conversion; strings/text; metadata/document setup; error/absence |
| Compatible at the evidenced bounded boundary | Mathematics/numeric operations |
| Unsupported | Layout/document functions; include/read/data loading before its host boundary is introduced |
| Scribium extension | `.map` and `.filter` collection transforms; they are tested Scribium behavior but are not v2.5.1 upstream features |
| Intentionally deferred | Include/read/data loading, function-driven metadata, layout/components, comparator-language sorting, and generalized DynamicValue conversion |

## Inventory

| Family | v2.5.1 public surface and evidence | Scribium status and semantic gap | Test/conformance evidence | Milestone and order |
|---|---|---|---|---|
| Function declarations and calls | Dot-prefixed calls with positional, named, mixed, nested, block, inline, and chained forms; `.function` declares document functions. Public evidence: [function-call syntax](https://quarkdown.com/wiki/syntax-of-a-function-call/), [declaring functions](https://quarkdown.com/wiki/declaring-functions/), [`Flow.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Flow.kt). | **Partially compatible.** The grammar, source-backed spans, user-function binding, value-context calls, and the reviewed scalar/collection slices work. The complete v2.5.1 stdlib/component call surface is not implemented; unresolved ordinary calls are preserved and unresolved chain segments fail closed with `E3001`. | `scribium-quarkdown` parser tests; `scribium-core` function/chain tests; CLI chain failure test. | M2 foundation; maintain as the dependency of each later bounded family. |
| Variables and assignment | `.var {name} {value}`, parameterless access, `.name {newvalue}`, block variables, and scoped variables through `.let`. Evidence: [variables](https://quarkdown.com/wiki/variables/), [`Flow.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Flow.kt). | **Partially compatible.** Document-scope scalar/block values, source-order reassignment, typed value flow, and scoped `.let` are implemented. Complete DynamicValue conversion and all host/context sharing behavior are not. | `compile_variable_*`, `compile_let_*`, and function-scope tests in `crates/scribium-core/src/lib.rs`. | M2; keep typed value flow ahead of conversions and data functions. |
| Conditionals | `.if` and `.ifnot` lazily evaluate a boolean condition and body; condition results can be composed from logical functions. Evidence: [conditional statements](https://quarkdown.com/wiki/conditional-statements/), v2.5.1 [`ConditionalTest.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-test/src/test/kotlin/com/quarkdown/test/ConditionalTest.kt). | **Partially compatible.** Literal/variable conditions, lazy nested calls, and the selected logical/comparison expressions are implemented. Truthiness, arbitrary conversions, and all upstream function-produced condition values are not universally supported. | `compile_logical_comparisons_*`; `qd251_logical_comparison_expression_remains_structural_and_source_backed`; existing conditional laziness tests. | M2; the bounded logical/comparison predicate slice is complete, while other condition-producing families remain separately evidenced. |
| Lambdas and callables | First-class typed lambdas, explicit or implicit `.1`, `.2`, … parameters, optional parameters, lexical parent context, `.let`, callback arguments, and lambda invocation. Evidence: [lambda](https://quarkdown.com/wiki/lambda/), [`Lambda.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/data/Lambda.kt), [`LambdaTest.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-test/src/test/kotlin/com/quarkdown/test/LambdaTest.kt). | **Partially compatible.** Typed `IrValue::Callable`, explicit/implicit binding, capture, child scopes, and the reviewed callback forms work. General lambda consumers, component semantics, and all callback-typed stdlib functions remain unsupported. | `compile_implicit_lambda_*`, `first_class_callable_*`, and transform tests; frontend `@lambda` span tests. | M2 bounded callable foundation; component and generalized callback work is M3+. |
| Iteration | `.foreach` maps an iterable into an ordered collection; `.repeat` is one-based `.foreach {1..N}`. Evidence: [loops](https://quarkdown.com/wiki/loops/), [`Flow.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Flow.kt), [`FlowTest.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/test/kotlin/com/quarkdown/stdlib/FlowTest.kt). | **Partially compatible.** Typed block forms, closed/left-open ranges, ordered results, destructuring for the reviewed Pair form, child isolation, and failure atomicity work. Endless-range consumers, generalized patterns, and the full stdlib consumer set remain gaps. | `compile_foreach_*`, `compile_repeat_*`, and iterable conformance tests. | M2 reviewed slice; generalized patterns and resource limits are later work. |
| Collection and Iterable operations | `.getat`, `.first`, `.second`, `.third`, `.last`, `.size`, `.sumall`, `.average`, `.distinct`, `.sorted`, `.reversed`, `.groupvalues`; Pair, Dictionary, and Range adapt as iterables. Evidence: [`Collection.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Collection.kt), [`IterableTest.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-test/src/test/kotlin/com/quarkdown/test/IterableTest.kt). | **Partially compatible.** The reviewed typed operations and ordering/absence behavior are implemented. Comparator-language syntax and table-specific operations are missing. Scribium also exposes `.map` and `.filter` as explicit extensions; they are not counted as v2.5.1 compatibility. | `collection_*`, `compile_collection_*`, and transform tests in `crates/scribium-core`; extension behavior is documented separately in `docs/SYNTAX.md`. | M2 collection slice completed; comparator/table operations follow only after a bounded semantic proposal. |
| Dictionary, Pair, and Range values | `.pair`, `.dictionary`, one-based Pair/Dictionary entry access, ordered dictionary entries, last-write-wins keys, literal `A..B`/`..B`/`A..`/`..`, and dynamic `.range`. Evidence: [dictionary](https://quarkdown.com/wiki/dictionary/), [range](https://quarkdown.com/wiki/range/), [`DictionaryValue.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/DictionaryValue.kt), [`Range.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/data/Range.kt). | **Partially compatible.** Recursive typed values, ordered iteration, access, finite range behavior, and atomic construction work. Nested/general destructuring, mutation, and direct materialization of every value shape are intentionally limited. | Pair/dictionary/range tests in `crates/scribium-core/src/lib.rs` and `evaluator.rs`; frontend range-span tests. | M2 reviewed slice; do not expand into generalized patterns without architecture review. |
| Type and value conversion | Dynamic typing adapts a value at invocation time to `String`, `Number`, `Boolean`, `Range`, Markdown content, collections, and other public value types. Evidence: [typing](https://quarkdown.com/wiki/typing/), [`DynamicValueConverter.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/reflect/DynamicValueConverter.kt), [`ValueFactory.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/factory/ValueFactory.kt). | **Partially compatible.** Scribium keeps typed `IrValue`s and has narrow, explicit conversions for the implemented operations (including collection `asDouble` behavior and selected scalar comparisons). It does not expose a general DynamicValue conversion layer and rejects unsupported structured-to-text coercions. | Typed evaluator, conversion, absence, and failure-atomicity tests; no claim of complete DynamicValue compatibility. | M2 debt, but implementation must be split by semantic family; general conversion is not part of this PR. |
| String and text operations | `.string`, `.concatenate`, `.uppercase`, `.lowercase`, `.capitalize`, `.isempty`, `.isnotempty`, `.startswith`, `.plaintext`. Evidence: [`Strings.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Strings.kt), [typing](https://quarkdown.com/wiki/typing/). | **Partially compatible.** The scalar string family is implemented for the bounded invocation contract: `.string` preserves already-parsed quoted scalar whitespace, `.concatenate` supports `with` and default-true `if`, case transforms use Unicode-aware Rust operations, and the emptiness/prefix predicates return typed booleans. Plain strings, identifiers, numbers, booleans, and bounded plain-text content adapt at the invocation boundary; `None`, collections, and other structured/rich values fail closed. `.plaintext` rich Markdown-content projection and complete upstream DynamicValue conversion remain unsupported. | `scribium-quarkdown/src/lib.rs::parses_nested_content_and_scalar_classification`; `scribium-core/src/builtins.rs::tests::string_*`; `scribium-core/src/lib.rs::compile_v251_string_scalar_fixture_preserves_typed_value_flow`, `compile_string_predicates_feed_lazy_conditionals_without_text_materialization`, `compile_string_predicate_failure_is_atomic_and_source_backed`; `fixtures/quarkdown-conformance/cases/string-scalar-family/input.qd`. | M2; scalar string family implemented as a bounded slice. `.plaintext` and general DynamicValue conversion remain separate gaps. |
| Mathematical and numeric operations | `.sum`, `.subtract`, `.multiply`, `.divide`, `.rem`, `.pow`, `.abs`, `.negate`, `.sqrt`, `.logn`, `.pi`, `.sin`, `.cos`, `.tan`, `.truncate`, `.round`, `.iseven`, and `.range`. Evidence: [`Math.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Math.kt), [`MathFunctionsTest.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-test/src/test/kotlin/com/quarkdown/test/MathFunctionsTest.kt), [math](https://quarkdown.com/wiki/math/). | **Compatible at the evidenced numeric boundary.** Typed evaluator paths implement the arithmetic, unary, decimal, and transcendental functions; dynamic/literal `.range` remains covered separately. `.logn`, `.sin`, `.cos`, and `.tan` adapt through the shared `numeric_argument()` Float boundary, then use pinned pure-Rust `libm` binary64 software functions on the adapted Float and narrow to Float. `.pi` preserves the upstream binary64 `PI` constant and does not pass through Float normalization. `.truncate` reproduces the upstream Float/Double/toInt/Float boundary, requires strict integral `decimals: Int`, and `.round` reproduces Kotlin ties-to-even followed by Int conversion. `NumberValue`-style integral Float normalization remains the evaluator output boundary. | Arithmetic regression: `crates/scribium-core/src/builtins.rs::tests::numeric_*`, `crates/scribium-core/src/lib.rs::tests::compile_v251_numeric_arithmetic_fixture_preserves_typed_value_flow`. Decimal slice: `decimal_numeric_surface_matches_upstream_boundaries`, `compile_v251_numeric_decimal_fixture_preserves_typed_value_flow`, `compile_numeric_decimal_forms_share_one_semantic_path`, `compile_numeric_decimal_failure_is_atomic_and_source_backed`, `crates/scribium-test-support/src/lib.rs::tests::test_verify_numeric_decimal_family_is_semantically_supported`; `fixtures/quarkdown-conformance/cases/numeric-decimal-family/input.qd`. Transcendental slice: `transcendental_numeric_surface_matches_upstream_boundaries`, `deterministic_transcendental_math_has_stable_representative_bits`, `compile_v251_numeric_transcendental_fixture_preserves_typed_value_flow`, `compile_numeric_transcendental_failure_is_atomic_and_source_backed`, `crates/scribium-test-support/src/lib.rs::tests::test_verify_numeric_transcendental_family_is_semantically_supported`; `fixtures/quarkdown-conformance/cases/numeric-transcendental-family/input.qd`. | M2 numeric family slice; `.range` remains a separately evidenced existing semantic path. |
| Logical and comparison operations | `.islower {a} than:{b} orequals:{bool}`, `.isgreater`, `.equals {a} to:{b}`, and `.not {value}`. Evidence: [`Logical.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Logical.kt), [`Comparison.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/internal/Comparison.kt), conditional examples/tests. | **Compatible for the bounded slice implemented here.** Numeric ordering uses upstream `toFloat` comparison and accepts the reviewed numeric scalar text forms; equality preserves typed values with the documented plain-text fallback; negation requires a boolean. Unsupported conversion inputs fail with one source-backed `E3001` and no partial branch output. | `builtins::tests::logical_*`; `compile_logical_comparisons_*`; frontend structural/span test; CLI verification below. | M2 completed bounded logical/comparison slice; future logical expansion remains separately evidenced. |
| Include, read, and data loading | `.include`, `.includeall`, `.read`, `.json`, `.csv`, `.listfiles`, `.filename`, and context sandbox modes. Evidence: [including other files](https://quarkdown.com/wiki/including-other-quarkdown-files/), [`Ecosystem.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Ecosystem.kt), [`Data.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Data.kt). | **Unsupported / intentionally deferred.** Scribium core is filesystem-free; `VirtualProject` is the accepted host boundary and no include/read evaluator is present. | Architecture and threat-model review; no implementation evidence. | M3 host/data-loading work; excluded from this M2 PR. |
| Metadata and document setup | `.doctype`, `.docname`, `.docdescription`, `.docauthor(s)`, `.dockeywords`, `.doclang`, `.theme`, page/paragraph metadata, numbering, and related document state. Evidence: [document metadata](https://quarkdown.com/wiki/document-metadata/), [`Document.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Document.kt). | **Partially compatible at a different boundary.** Scribium supports project/front-matter metadata (`title`, `author`, `date`, raw fields), but not the Quarkdown function-driven document context or all observable layout metadata. | `VirtualProject` and front-matter tests; no Quarkdown document-function conformance claim. | M2 metadata baseline is partial; function-driven document setup is M3+/backend work and excluded. |
| Layout and document functions | `.row`, `.column`, `.grid`, `.container`, `.align`, `.box`, `.figure`, page breaks, tables, and related layout primitives. Evidence: [`Layout.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Layout.kt), [`Primitives.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Primitives.kt). | **Unsupported / intentionally deferred.** These require new semantic/layout ownership and backend contracts; they are not inferred from current Markdown structures. | Architecture/roadmap boundary; current Markdown tables and lists are not equivalent to Quarkdown layout functions. | M3+; explicitly excluded from this M2 slice. |
| Error and absence behavior | `none`, `.isnone`, `.otherwise`, `.ifpresent`, `.takeif`, invalid argument/type errors, and lazy failure behavior. Evidence: [`Optionality.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Optionality.kt), [conditional statements](https://quarkdown.com/wiki/conditional-statements/). | **Partially compatible.** `None`, `.isnone`, `.otherwise`, source-backed evaluator errors, nested diagnostic de-duplication, and atomic results are covered. `.ifpresent`/`.takeif` and the complete upstream error taxonomy are not. | Optionality, chain, nested-failure, and failure-atomicity tests. | M2 cross-cutting invariant; extend only alongside the owning semantic family. |

## Selection record

The current selected slice is the transcendental numeric family: `.logn`,
`.pi`, `.sin`, `.cos`, and `.tan`, integrated with the completed
arithmetic/unary and decimal numeric families and the existing `.sum`,
`.multiply`, and `.range` paths. It follows the completed
logical/comparison, scalar-string, arithmetic, and decimal slices and uses the
same typed invocation boundary.

This is a bounded public semantic family with direct v2.5.1 source/test
evidence and an official v2.5.1 macOS arm64 runtime probe for rendered edge
behavior. It fits the existing evaluator/value-flow boundary and needs no new
parser, filesystem capability, IR tier, or backend escape hatch. Unary
transcendental arguments reuse the shared binder and existing scalar numeric
path. The deterministic implementation pins `libm` `0.2.16` with
`default-features = false`; after Float adaptation it calls pure-Rust binary64
software functions and narrows to Float. This mirrors the Kotlin/JVM Float
overload's `float -> double Math.* -> float` boundary without Rust `std` math,
OS libc/libm FFI, or target-specific transcendental calls. Representative bits
are fixed in unit tests; the helper preserves signed zero and IEEE non-finite
classes before the existing NumberValue-style evaluator normalization.

The earlier decimal family remains implemented under the same boundary:
`decimals` has a narrow strict integer-compatible adapter, integral
NumberValue representations are accepted, fractional values and quoted text
are rejected, and negative accepted Int values fail at runtime. The evaluator
preserves `IrValue::Number` and materializes only through the existing normal
IR-to-Typst path.

The PR intentionally excludes `.plaintext`, general DynamicValue conversion,
`.ifpresent`/`.takeif`, comparator-language syntax for sorting, include/read/data
loading, metadata functions, components, and layout/document primitives. These
other families require separate evidence and are not represented as complete
compatibility here.

## Conformance evidence for the current numeric slice

- Frontend preserves `.if {.islower {2} than:{3}}` as a nested directive with
  original UTF-8/CRLF-safe spans:
  `crates/scribium-markdown/tests/quarkdown_v2_5_1.rs::qd251_logical_comparison_expression_remains_structural_and_source_backed`.
- Unit evaluation covers strict/inclusive ordering, boolean negation, typed
  equality, plain-text fallback, invalid values, duplicate bindings, and body
  rejection: `crates/scribium-core/src/builtins.rs::tests::*`.
- Unit evaluation covers the decimal surface, strict integral `decimals`,
  negative/fractional failure, Float/Double/Float truncation order, zero and
  negative values, Kotlin ties-to-even, NaN/Infinity/clamping boundaries,
  named/mixed binding, nested values, invalid conversion, and block-body
  rejection: `crates/scribium-core/src/builtins.rs::tests::decimal_numeric_surface_matches_upstream_boundaries`.
- The independently authored decimal conformance fixture covers direct,
  positional/named, chained, half-even, negative, zero-decimal, and nested
  arithmetic composition:
  `fixtures/quarkdown-conformance/cases/numeric-decimal-family/input.qd`.
- Integration coverage verifies ordinary/named/chain equivalence, typed
  `IrValue::Number` flow, source-backed nested failure, one diagnostic, and no
  partial enclosing output: `crates/scribium-core/src/lib.rs::tests::compile_v251_numeric_decimal_fixture_preserves_typed_value_flow`,
  `compile_numeric_decimal_forms_share_one_semantic_path`, and
  `compile_numeric_decimal_failure_is_atomic_and_source_backed`.
- The independently authored transcendental fixture covers binary64 `.pi`,
  Float-normalized `.logn`/trigonometry, zero and pi composition, nested
  typed values, and a chain into `.logn`:
  `fixtures/quarkdown-conformance/cases/numeric-transcendental-family/input.qd`.
- Transcendental unit coverage fixes representative `to_bits()` results for
  logarithm, sine, cosine, and tangent; checks `ln(0)`, negative-domain NaN,
  infinities, and `sin(-0)`, `cos(-0)`, and `tan(-0)`; and exercises named,
  textual, nested, chained, arity, body, and invalid-structured-input paths:
  `crates/scribium-core/src/builtins.rs::tests::transcendental_numeric_surface_matches_upstream_boundaries`
  and `deterministic_transcendental_math_has_stable_representative_bits`.
- Integration coverage verifies the rendered oracle cases, typed flow, one
  source-backed `E3001`, and no partial output on nested transcendental
  failure: `crates/scribium-core/src/lib.rs::tests::compile_v251_numeric_transcendental_fixture_preserves_typed_value_flow`
  and `compile_numeric_transcendental_failure_is_atomic_and_source_backed`.
- The prior arithmetic/unary regression remains covered by
  `crates/scribium-core/src/builtins.rs::tests::numeric_*`,
  `compile_v251_numeric_arithmetic_fixture_preserves_typed_value_flow`, and
  `fixtures/quarkdown-conformance/cases/numeric-arithmetic-family/input.qd`.
- Nested numeric failure is source-backed, emits one diagnostic, and publishes
  no partial output: `crates/scribium-core/src/lib.rs::tests::compile_numeric_nested_failure_is_atomic_and_source_backed`.
- Full compile coverage covers nested calls, variables, user functions,
  chains, lazy branches, failure atomicity, source spans, UTF-8, CRLF, and
  deterministic repeated execution: `crates/scribium-core/src/lib.rs::tests::compile_logical_*`.
- The focused backend integration test verifies the output boundary and real
  Typst/PDF compilation; no Typst-specific implementation was added.
- The repository's required `examples/hello` CLI smoke commands remain a
  pre-existing failure because that example references unresolved `show_code`;
  this slice does not alter the example or invent configuration semantics.

The scalar string conformance fixture covers quoted whitespace preservation,
positional/named/mixed binding, conditional concatenation, Unicode and boundary
case behavior, typed chain results, lazy predicate conditionals, and the
source-backed invalid-boolean failure path. `.plaintext` remains deferred
because it projects rich inline Markdown content rather than adapting a scalar
string; the `.equals` plain-text fallback remains private to equality.
