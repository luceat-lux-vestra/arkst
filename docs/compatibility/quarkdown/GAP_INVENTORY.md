# Quarkdown v2.5.1 Public-Language Gap Inventory

## Review snapshot

- **Tracked target:** Quarkdown `v2.5.1`
- **Resolved tag commit:** `107ec3a9482f10d6f90d7580f8409b46a719d18e`
- **Repository:** [`iamgio/quarkdown`](https://github.com/iamgio/quarkdown)
- **Review date:** 2026-08-19
- **Scribium comparison head:** `9fd877f2a3b4440f2b944bf919d2d7ac693359e4`
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
| Compatible at the stated bounded boundary | Logical and comparison operations; optionality callbacks; Collection selector sorting; project-backed `.read`, `.json`, and `.include` |
| Partially compatible | Function declarations/calls; variables; conditionals; lambdas/callables; iteration; collections; Dictionary/Pair/Range; type/value conversion; strings/text; metadata/document setup; complete error taxonomy |
| Compatible at the evidenced bounded boundary | Mathematics/numeric operations |
| Unsupported | Layout/document functions; unimplemented data-loading families such as `.csv`, `.listfiles`, and `.filename` |
| Scribium extension | `.map` and `.filter` collection transforms; they are tested Scribium behavior but are not v2.5.1 upstream features |
| Intentionally deferred | Unimplemented data-loading families, `.llmstxt`, function-driven metadata, layout/components, arbitrary comparator syntax not present in v2.5.1, and generalized DynamicValue conversion |

## v2.5.1 stdlib surface classification

The pinned v2.5.1 stdlib tree at commit
`107ec3a9482f10d6f90d7580f8409b46a719d18e` was re-enumerated on the current
Scribium head. Names below are the public `@QFunction` names, including
`@Name` aliases. “Implemented” always means the bounded semantic boundary
stated in the corresponding family row; it does not promote the whole family
to complete compatibility.

| Classification | v2.5.1 functions | Boundary / reason |
|---|---|---|
| Implemented (bounded) | `.abs`, `.average`, `.capitalize`, `.concatenate`, `.cos`, `.dictionary`, `.distinct`, `.divide`, `.equals`, `.first`, `.foreach`, `.function`, `.getat`, `.groupvalues`, `.html`, `.if`, `.ifnot`, `.ifpresent`, `.include`, `.isempty`, `.isgreater`, `.iseven`, `.isnone`, `.islower`, `.isnotempty`, `.json`, `.last`, `.lowercase`, `.let`, `.markdown`, `.multiply`, `.negate`, `.none`, `.not`, `.otherwise`, `.pair`, `.pi`, `.plaintext`, `.range`, `.read`, `.repeat`, `.rem`, `.reversed`, `.round`, `.second`, `.sin`, `.size`, `.sorted`, `.sqrt`, `.startswith`, `.string`, `.subtract`, `.sum`, `.sumall`, `.takeif`, `.tan`, `.third`, `.truncate`, `.uppercase`, `.var` | Typed evaluator results, shared callable/iterable paths, bounded native-content/resource boundaries, or existing structural document semantics. `.sorted` is natural-order or `by` selector sorting; it is not a two-value comparator API. |
| Partially implemented | `.autopagebreak`, `.captionposition`, `.currentpage`, `.docauthor`, `.docauthors`, `.docdescription`, `.dockeywords`, `.doclang`, `.docname`, `.doctype`, `.font`, `.footer`, `.formatpagenumber`, `.lastheading`, `.marker`, `.navigation`, `.noautopagebreak`, `.nonumbering`, `.numbering`, `.pageformat`, `.pagemargin`, `.paragraphstyle`, `.resetpagenumber`, `.tableofcontents`, `.texmacro`, `.theme`, `.totalpages` | Project/front-matter or IR metadata provides only a different, partial boundary; function-driven document context and observable layout state are not implemented. |
| Unsupported | `.allemojis`, `.align`, `.bibliography`, `.box`, `.br`, `.center`, `.cite`, `.clip`, `.code`, `.codespan`, `.collapse`, `.column`, `.container`, `.debug`, `.emoji`, `.error`, `.extend`, `.figure`, `.filetree`, `.float`, `.fragment`, `.fullspan`, `.functionexists`, `.get`, `.grid`, `.heading`, `.icon`, `.image`, `.keybinding`, `.landscape`, `.libraries`, `.libexists`, `.libfunctions`, `.libraryexists`, `.link`, `.localization`, `.localize`, `.log`, `.loremipsum`, `.math`, `.match`, `.mermaid`, `.numbered`, `.pagebreak`, `.paragraph`, `.ref`, `.slides`, `.speakernote`, `.subdocumentgraph`, `.table`, `.tablebyrows`, `.tablecolumn`, `.tablecolumns`, `.tablecompute`, `.tablefilter`, `.tablesort`, `.text`, `.textcollapse`, `.todo`, `.whitespace`, `.xychart` | The call may be preserved or parsed, but no approved evaluator/backend-neutral semantic implementation exists. |
| Intentionally deferred | `.csv`, `.css`, `.cssproperties`, `.env`, `.filename`, `.htmloptions`, `.includeall`, `.listfiles`, `.llmstxt`, `.pathtoroot`, `.subdocument` | Resource families, target-specific options, and process/environment access are explicitly outside this slice or require a separate host/security decision. `.llmstxt` remains deferred per task scope. |
| Scribium extension | `.map`, `.filter` | Existing typed collection transforms have no corresponding public v2.5.1 `Collection.kt` declarations and are excluded from upstream compatibility counts. |

The inventory deliberately keeps native bounded support separate from
partially implemented family claims. In particular, `.ifpresent` and
`.takeif` use first-class `@lambda` values or headerless indented callback
bodies through the existing callable path; ordinary content is not reparsed or
silently reclassified as a lambda.

## Inventory

| Family | v2.5.1 public surface and evidence | Scribium status and semantic gap | Test/conformance evidence | Milestone and order |
|---|---|---|---|---|
| Function declarations and calls | Dot-prefixed calls with positional, named, mixed, nested, block, inline, and chained forms; `.function` declares document functions. Public evidence: [function-call syntax](https://quarkdown.com/wiki/syntax-of-a-function-call/), [declaring functions](https://quarkdown.com/wiki/declaring-functions/), [`Flow.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Flow.kt). | **Partially compatible.** The grammar, source-backed spans, user-function binding, value-context calls, and the reviewed scalar/collection slices work. The complete v2.5.1 stdlib/component call surface is not implemented; unresolved ordinary calls are preserved and unresolved chain segments fail closed with `E3001`. | `scribium-quarkdown` parser tests; `scribium-core` function/chain tests; CLI chain failure test. | M2 foundation; maintain as the dependency of each later bounded family. |
| Variables and assignment | `.var {name} {value}`, parameterless access, `.name {newvalue}`, block variables, and scoped variables through `.let`. Evidence: [variables](https://quarkdown.com/wiki/variables/), [`Flow.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Flow.kt). | **Partially compatible.** Document-scope scalar/block values, source-order reassignment, typed value flow, and scoped `.let` are implemented. Complete DynamicValue conversion and all host/context sharing behavior are not. | `compile_variable_*`, `compile_let_*`, and function-scope tests in `crates/scribium-core/src/lib.rs`. | M2; keep typed value flow ahead of conversions and data functions. |
| Conditionals | `.if` and `.ifnot` lazily evaluate a boolean condition and body; condition results can be composed from logical functions. Evidence: [conditional statements](https://quarkdown.com/wiki/conditional-statements/), v2.5.1 [`ConditionalTest.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-test/src/test/kotlin/com/quarkdown/test/ConditionalTest.kt). | **Partially compatible.** Literal/variable conditions, lazy nested calls, and the selected logical/comparison expressions are implemented. Truthiness, arbitrary conversions, and all upstream function-produced condition values are not universally supported. | `compile_logical_comparisons_*`; `qd251_logical_comparison_expression_remains_structural_and_source_backed`; existing conditional laziness tests. | M2; the bounded logical/comparison predicate slice is complete, while other condition-producing families remain separately evidenced. |
| Lambdas and callables | First-class typed lambdas, explicit or implicit `.1`, `.2`, … parameters, optional parameters, lexical parent context, `.let`, callback arguments, and lambda invocation. Evidence: [lambda](https://quarkdown.com/wiki/lambda/), [`Lambda.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/data/Lambda.kt), [`LambdaTest.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-test/src/test/kotlin/com/quarkdown/test/LambdaTest.kt). | **Partially compatible.** Typed `IrValue::Callable`, explicit/implicit binding, capture, child scopes, and the reviewed callback forms work. General lambda consumers, component semantics, and all callback-typed stdlib functions remain unsupported. | `compile_implicit_lambda_*`, `first_class_callable_*`, and transform tests; frontend `@lambda` span tests. | M2 bounded callable foundation; component and generalized callback work is M3+. |
| Iteration | `.foreach` maps an iterable into an ordered collection; `.repeat` is one-based `.foreach {1..N}`. Evidence: [loops](https://quarkdown.com/wiki/loops/), [`Flow.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Flow.kt), [`FlowTest.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/test/kotlin/com/quarkdown/stdlib/FlowTest.kt). | **Partially compatible.** Typed block forms, closed/left-open ranges, ordered results, destructuring for the reviewed Pair form, child isolation, and failure atomicity work. Endless-range consumers, generalized patterns, and the full stdlib consumer set remain gaps. | `compile_foreach_*`, `compile_repeat_*`, and iterable conformance tests. | M2 reviewed slice; generalized patterns and resource limits are later work. |
| Collection and Iterable operations | `.getat`, `.first`, `.second`, `.third`, `.last`, `.size`, `.sumall`, `.average`, `.distinct`, `.sorted`, `.reversed`, `.groupvalues`; Pair, Dictionary, and Range adapt as iterables. Evidence: [`Collection.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Collection.kt), [`IterableTest.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-test/src/test/kotlin/com/quarkdown/test/IterableTest.kt). | **Partially compatible.** The reviewed typed operations and ordering/absence behavior are implemented. Upstream `.sorted` accepts an optional one-element `by` selector and uses stable natural-key ordering; Scribium covers that selector boundary. Table-specific operations and unrelated collection APIs remain missing. Scribium also exposes `.map` and `.filter` as explicit extensions; they are not counted as v2.5.1 compatibility. | `collection_*`, `compile_collection_*`, and transform tests in `crates/scribium-core`; extension behavior is documented separately in `docs/SYNTAX.md`. | M2 bounded selector/collection slice; table operations and generalized consumers remain deferred. |
| Dictionary, Pair, and Range values | `.pair`, `.dictionary`, one-based Pair/Dictionary entry access, ordered dictionary entries, last-write-wins keys, literal `A..B`/`..B`/`A..`/`..`, and dynamic `.range`. Evidence: [dictionary](https://quarkdown.com/wiki/dictionary/), [range](https://quarkdown.com/wiki/range/), [`DictionaryValue.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/DictionaryValue.kt), [`Range.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/data/Range.kt). | **Partially compatible.** Recursive typed values, ordered iteration, access, finite range behavior, and atomic construction work. Nested/general destructuring, mutation, and direct materialization of every value shape are intentionally limited. | Pair/dictionary/range tests in `crates/scribium-core/src/lib.rs` and `evaluator.rs`; frontend range-span tests. | M2 reviewed slice; do not expand into generalized patterns without architecture review. |
| Type and value conversion | Dynamic typing adapts a **DynamicValue-origin argument at invocation time** to `String`, `Number`, `Boolean`, `Range`, Markdown content, collections, and other public value types. A statically materialized `StringValue` does not enter that converter for `Number`, `Boolean`, or iterable targets; it only participates in its own String/InlineMarkdownContent boundaries. Evidence: [typing](https://quarkdown.com/wiki/typing/), [`RegularArgumentsBinder.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/call/binding/RegularArgumentsBinder.kt), [`DynamicValueConverter.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/reflect/DynamicValueConverter.kt), [`ValueFactory.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/factory/ValueFactory.kt). | **Partially compatible.** The bounded scalar conversion slice is implemented only where an existing evaluator consumer requires invocation-time DynamicValue conversion. Number uses integer-first then floating parsing; Boolean accepts only case-insensitive `true`/`yes`/`false`/`no`; textual Range uses the reviewed `x..y`, `..y`, `x..`, and `..` forms. Static nested String results are not generically coerced to Number, Boolean, or Iterable. Context-sensitive Markdown conversion is deferred, generic collection/callable/document conversion is unsupported, and no universal DynamicValue conversion framework is exposed. | `value_conversion` unit tests; dynamic-vs-static compile tests; `compile_v251_dynamic_value_scalar_fixture_uses_existing_consumers`; callback/atomicity/provenance regressions; independently authored `dynamic-value-scalar-family` fixture. | M2 bounded invocation-time scalar conversion implemented; context-sensitive, component/layout, collection-dependent, and currently unverified conversion surfaces remain compatibility debt. |
| String and text operations | `.string`, `.concatenate`, `.uppercase`, `.lowercase`, `.capitalize`, `.isempty`, `.isnotempty`, `.startswith`, `.plaintext`. Evidence: [`Strings.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Strings.kt), [`NodeUtils.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/util/node/NodeUtils.kt), [`ValueFactory.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/factory/ValueFactory.kt), [typing](https://quarkdown.com/wiki/typing/). | **Partially compatible.** The scalar string family remains bounded. `.plaintext` now performs the evidenced structural projection for already-parsed `IrValue::Content` inline trees and the oracle-equivalent Identifier/Number/Boolean boundary. Emphasis, strong, strikethrough, code, and link labels recurse; soft breaks emit a newline; hard breaks and images emit nothing. Unresolved inline calls, `String`, `None`, collections, ranges, pairs, dictionaries, and callables fail closed. Dynamic String → InlineMarkdownContent conversion that would require upstream reparsing remains unsupported by design. | `scribium-markdown/tests/quarkdown_v2_5_1.rs::qd251_plaintext_keeps_existing_scalar_and_content_argument_classification`; `scribium-core/src/builtins.rs::tests::plaintext_*`; `scribium-core/src/evaluator.rs` shared value-context evaluation; `scribium-core/src/lib.rs::compile_v251_plaintext_fixture_projects_evaluated_inline_content`, `compile_plaintext_rejects_unsupported_values_atomically`; `scribium-test-support/src/lib.rs::tests::test_verify_plaintext_family_is_semantically_supported`; `fixtures/quarkdown-conformance/cases/plaintext-family/input.qd`. | M2; bounded already-parsed inline projection implemented. General DynamicValue conversion and String → InlineMarkdownContent reparsing remain compatibility debt. |
| Target-specific HTML content | `.html` accepts one `content: String`; inline and isolated block forms are documented. v2.5.1 evidence: [`Html.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Html.kt#L59-L90), [`Html` node](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/ast/base/block/Html.kt#L1-L14), [HTML documentation](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/docs/html.qd#L43-L58). | **Implemented for the closed Html semantic slice.** The evaluator checks explicit `NativeContent` (default granted; host/API denial emits one source-backed `E3004`), evaluates the single String boundary, preserves source-backed block/inline `TargetSpecificContent`, and Typst/PDF omit it silently. A future HTML output backend consumes the payload; `scribium-html` remains only the Markdown/foreign-HTML normalization boundary. `.css` and `.htmloptions` remain unsupported, and ordinary mixed `.qd`/`.scrib` raw HTML remains `E8001`. | `crates/scribium-markdown/tests/quarkdown_html_contract.rs`; `crates/scribium-core/tests/quarkdown_html_contract.rs`; `crates/scribium-typst/tests/backend_integration.rs`; [ADR-0018](../../adr/0018-quarkdown-target-specific-native-content.md). | M2 bounded semantic slice implemented; HTML backend remains future work. |
| Mathematical and numeric operations | `.sum`, `.subtract`, `.multiply`, `.divide`, `.rem`, `.pow`, `.abs`, `.negate`, `.sqrt`, `.logn`, `.pi`, `.sin`, `.cos`, `.tan`, `.truncate`, `.round`, `.iseven`, and `.range`. Evidence: [`Math.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Math.kt), [`MathFunctionsTest.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-test/src/test/kotlin/com/quarkdown/test/MathFunctionsTest.kt), [math](https://quarkdown.com/wiki/math/). | **Compatible at the evidenced numeric boundary.** Typed evaluator paths implement the arithmetic, unary, decimal, and transcendental functions; dynamic/literal `.range` remains covered separately. `.logn`, `.sin`, `.cos`, and `.tan` adapt through the shared `numeric_argument()` Float boundary, then use pinned pure-Rust `libm` binary64 software functions on the adapted Float and narrow to Float. `.pi` preserves the upstream binary64 `PI` constant and does not pass through Float normalization. `.truncate` reproduces the upstream Float/Double/toInt/Float boundary, requires strict integral `decimals: Int`, and `.round` reproduces Kotlin ties-to-even followed by Int conversion. `NumberValue`-style integral Float normalization remains the evaluator output boundary. | Arithmetic regression: `crates/scribium-core/src/builtins.rs::tests::numeric_*`, `crates/scribium-core/src/lib.rs::tests::compile_v251_numeric_arithmetic_fixture_preserves_typed_value_flow`. Decimal slice: `decimal_numeric_surface_matches_upstream_boundaries`, `compile_v251_numeric_decimal_fixture_preserves_typed_value_flow`, `compile_numeric_decimal_forms_share_one_semantic_path`, `compile_numeric_decimal_failure_is_atomic_and_source_backed`, `crates/scribium-test-support/src/lib.rs::tests::test_verify_numeric_decimal_family_is_semantically_supported`; `fixtures/quarkdown-conformance/cases/numeric-decimal-family/input.qd`. Transcendental slice: `transcendental_numeric_surface_matches_upstream_boundaries`, `deterministic_transcendental_math_has_stable_representative_bits`, `compile_v251_numeric_transcendental_fixture_preserves_typed_value_flow`, `compile_numeric_transcendental_failure_is_atomic_and_source_backed`, `crates/scribium-test-support/src/lib.rs::tests::test_verify_numeric_transcendental_family_is_semantically_supported`; `fixtures/quarkdown-conformance/cases/numeric-transcendental-family/input.qd`. | M2 numeric family slice; `.range` remains a separately evidenced existing semantic path. |
| Logical and comparison operations | `.islower {a} than:{b} orequals:{bool}`, `.isgreater`, `.equals {a} to:{b}`, and `.not {value}`. Evidence: [`Logical.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Logical.kt), [`Comparison.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/internal/Comparison.kt), conditional examples/tests. | **Compatible for the bounded slice implemented here.** Numeric ordering uses upstream `toFloat` comparison and accepts the reviewed numeric scalar text forms; equality preserves typed values with the documented plain-text fallback; negation requires a boolean. Unsupported conversion inputs fail with one source-backed `E3001` and no partial branch output. | `builtins::tests::logical_*`; `compile_logical_comparisons_*`; frontend structural/span test; CLI verification below. | M2 completed bounded logical/comparison slice; future logical expansion remains separately evidenced. |
| Include, read, and data loading | `.include`, `.includeall`, `.read`, `.json`, `.csv`, `.listfiles`, `.filename`, and context sandbox modes. Evidence: [including other files](https://quarkdown.com/wiki/including-other-quarkdown-files/), [`Ecosystem.kt`](https://github.com/iamgio/quarkdown/blob/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Ecosystem.kt), [`Data.kt`](https://github.com/iamgio/quarkdown/blob/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Data.kt). | **Partially compatible.** `.read`, `.json`, and `.include` are implemented over `VirtualProject` only. Paths resolve relative to the current logical source, nested includes change that base to the included source, active include stacks detect cycles, and repeated includes remain valid. Local paths that leave the project, absolute paths, URI schemes, missing resources, and invalid UTF-8 produce structured diagnostics; the evaluator has no host filesystem or network capability. `.includeall`, `.csv`, `.listfiles`, and `.filename` remain deferred. | `crates/scribium-core/tests/quarkdown_resource_builtins.rs` covers local/parent-relative reads, UTF-8 and JSON failures, nested source context, cycles, repeated includes, and in-memory projects. CLI loading is the native filesystem boundary. | M2 bounded resource-backed slice; additional data families require separate upstream evidence and semantic tests. |
| Metadata and document setup | `.doctype`, `.docname`, `.docdescription`, `.docauthor(s)`, `.dockeywords`, `.doclang`, `.theme`, page/paragraph metadata, numbering, and related document state. Evidence: [document metadata](https://quarkdown.com/wiki/document-metadata/), [`Document.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Document.kt). | **Partially compatible at a different boundary.** Scribium supports project/front-matter metadata (`title`, `author`, `date`, raw fields), but not the Quarkdown function-driven document context or all observable layout metadata. | `VirtualProject` and front-matter tests; no Quarkdown document-function conformance claim. | M2 metadata baseline is partial; function-driven document setup is M3+/backend work and excluded. |
| Layout and document functions | `.row`, `.column`, `.grid`, `.container`, `.align`, `.box`, `.figure`, page breaks, tables, and related layout primitives. Evidence: [`Layout.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Layout.kt), [`Primitives.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Primitives.kt). | **Unsupported / intentionally deferred.** These require new semantic/layout ownership and backend contracts; they are not inferred from current Markdown structures. | Architecture/roadmap boundary; current Markdown tables and lists are not equivalent to Quarkdown layout functions. | M3+; explicitly excluded from this M2 slice. |
| Error and absence behavior | `none`, `.isnone`, `.otherwise`, `.ifpresent`, `.takeif`, invalid argument/type errors, and lazy failure behavior. Evidence: [`Optionality.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Optionality.kt), [conditional statements](https://quarkdown.com/wiki/conditional-statements/). | **Partially compatible; optionality callbacks are implemented at a bounded boundary.** `None`, `.isnone`, `.otherwise`, the `.ifpresent` absence short-circuit, Boolean-only `.takeif` callback execution, source-backed evaluator errors, nested diagnostic de-duplication, and atomic results are covered. The complete upstream error taxonomy and unmarked explicit inline-lambda classification remain outside this slice. | `compile_optionality_*`, `compile_isnone_returns_a_semantic_boolean_for_optional_values`, and `fixtures/quarkdown-conformance/cases/optionality-callback-family/input.qd`. | M2 optionality callback slice; retain the family as partial until the remaining error and conversion surfaces are evidenced. |

## Selection record

The preceding selected slice was the transcendental numeric family: `.logn`,
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

The current `.plaintext` slice is a separate bounded builtin adaptation. It
consumes already-parsed inline content directly from `IrValue::Content` after
the shared evaluator has resolved nested calls. It does not create a
plain-text backend, invoke Typst, rewrite source, or reparse `IrValue::String`.
The upstream v2.5.1 runtime probe observed the following projection contract:

- text, code, emphasis, strong, strikethrough, and link labels preserve their
  displayed text;
- soft breaks emit a newline;
- hard breaks and images emit no text; and
- empty inline content emits the empty string.

The unsupported Dynamic String → InlineMarkdownContent conversion remains a
documented gap because upstream may reparse Markdown-bearing strings. The
remaining resource/data loading, metadata functions, components, and
layout/document primitives are also outside this slice. `.sorted` is covered
at the upstream `by` selector boundary; v2.5.1 does not expose an arbitrary
negative/zero/positive comparator parameter.

## Optionality callback evidence

The v2.5.1 `Optionality.kt` contract defines `.none`, `.isnone`, `.otherwise`,
`.ifpresent(value, mapping)`, and `.takeif(value, condition)`. `.ifpresent`
returns `None` without invoking its mapping callback for a `None` value and
otherwise returns the callback's typed result. `.takeif` invokes its condition
for every value, including `None`, requires a Boolean callback result, and
returns the original value or `None`.

Scribium implements this bounded family through `IrValue::None` and the
existing `IrValue::Callable` invocation path. First-class `@lambda` callbacks
and headerless indented callback bodies use fresh child scopes, immutable
captures, nearest-scope implicit parameters, and source-backed diagnostics.
Callback failure is atomic: no optionality result is published, and nested
failure produces one E3001/E3003 diagnostic. The independently authored
fixture is `fixtures/quarkdown-conformance/cases/optionality-callback-family/input.qd`;
compile, lexical capture/shadowing, lazy-none, UTF-8/CRLF provenance, and
failure-atomicity coverage is in `crates/scribium-core/src/lib.rs::tests::compile_optionality_*`.

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
source-backed invalid-boolean failure path. The separate `plaintext-family`
fixture covers rich formatting, nested formatting, code spans, link labels,
soft/hard breaks, and evaluated nested calls through the already-parsed IR
boundary. Dynamic String reparsing remains a compatibility gap; the `.equals`
plain-text fallback remains private to equality.
