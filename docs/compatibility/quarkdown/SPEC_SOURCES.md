# Quarkdown Compatibility — Specification Sources

This file records all public specification sources used for Scribium's
Quarkdown-compatible feature implementation.

## Reference Baseline

- **Reference version:** Quarkdown **v2.5.1** (released 2026-08-12;
  `iamgio/quarkdown` tag `v2.5.1`)
- **Resolved upstream tag commit:** `107ec3a9482f10d6f90d7580f8409b46a719d18e`
- **Compatibility target:** complete public-language and document-observable semantic compatibility (ADR 0016)
- **Current verified baseline:** v2.5.1; current implementation is partial
- **Historical evidence retained:** v2.5.0 sources below remain part of the
  provenance record and are not deleted by this adaptation review

## Primary Sources

| Source                                        | Title / Citation                                   | Used For                                        | Date Accessed |
|-----------------------------------------------|----------------------------------------------------|-------------------------------------------------|---------------|
| GitHub release tag `v2.5.0`                   | https://github.com/iamgio/quarkdown/releases/tag/v2.5.0 | Reference baseline identification and v2.5.0 release additions such as `.markdown`, `.llmstxt`, `.code` and `.json` | 2026-08-08 |
| Quarkdown wiki (badged **2.5.0**)              | https://quarkdown.com/wiki/syntax-of-a-function-call/ | Dot-prefixed calls; positional, named, and mixed arguments; nested calls; block vs inline calls; indented bodies | 2026-08-08 |
| Quarkdown wiki (badged **2.5.0**)              | https://quarkdown.com/wiki/conditional-statements/ | Conditional constructs: `.if`, `.ifnot`; boolean conditions; indented body semantics; nesting | 2026-08-08 |
| Quarkdown wiki (badged **2.5.0**)              | https://quarkdown.com/wiki/boolean/ | Boolean literals: `true`/`yes`, `false`/`no` (case-insensitive) | 2026-08-08 |
| Quarkdown wiki — "Variables"                   | https://quarkdown.com/wiki/variables/ | Variable declaration (`.var`), reference (`.name`), reassignment (`.name {value}`), block variables, boolean use in conditionals | 2026-08-08 |
| Quarkdown wiki — "Conditional statements" (v2.5.1) | https://quarkdown.com/wiki/conditional-statements/ | Lazy `.if`/`.ifnot` behavior and expression-valued conditions | 2026-08-18 |
| Quarkdown v2.5.1 `Logical.kt`                 | https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Logical.kt | Public `.islower`, `.isgreater`, `.equals`, and `.not` signatures, named parameters, numeric comparison, and boolean negation | 2026-08-18 |
| Quarkdown v2.5.1 `Comparison.kt`              | https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/internal/Comparison.kt | Equality plain-text fallback for String, Number, and Markdown content | 2026-08-18 |
| Quarkdown v2.5.1 `ConditionalTest.kt`         | https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-test/src/test/kotlin/com/quarkdown/test/ConditionalTest.kt | Public integration examples for `.islower` in `.if`, false branches, and `.ifnot` | 2026-08-18 |
| Quarkdown v2.5.1 `FlowTest.kt`                | https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/test/kotlin/com/quarkdown/stdlib/FlowTest.kt | Direct logical helper behavior and conditional control-flow results | 2026-08-18 |
| Quarkdown v2.5.1 `Strings.kt`                 | https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Strings.kt | Public scalar string signatures plus `.plaintext(content: InlineMarkdownContent)` returning a `StringValue` | 2026-08-18 |
| Quarkdown v2.5.1 `NodeUtils.kt`               | https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/util/node/NodeUtils.kt | `InlineContent.toPlainText()` projection: text/code and nested formatting/link labels recurse, soft breaks emit a newline, hard breaks and images emit nothing | 2026-08-18 |
| Quarkdown v2.5.1 `Math.kt`                    | https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Math.kt | Public `.sum`, `.subtract`, `.multiply`, `.divide`, `.rem`, `.pow`, `.abs`, `.negate`, `.sqrt`, `.logn`, `.pi`, `.sin`, `.cos`, `.tan`, `.truncate`, `.round`, `.iseven`, and `.range` signatures plus Float/Double/Int operation boundaries | 2026-08-18 |
| Quarkdown v2.5.1 `MathFunctionsTest.kt`       | https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-test/src/test/kotlin/com/quarkdown/test/MathFunctionsTest.kt | Public integration examples for arithmetic chains, nested calls, `.pi::truncate {2}`, zero trigonometry, `.cos {.pi}`, `.pi::multiply {2}::cos`, negative-decimal runtime failure, and fractional-decimal type failure | 2026-08-18 |
| Quarkdown wiki — "Math"                       | https://quarkdown.com/wiki/math/ | Public math-family scope and nested/chained arithmetic examples | 2026-08-18 |
| Quarkdown v2.5.1 `NumberValue.kt`             | https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/NumberValue.kt | Integral Float normalization to Int, including the observable finite/non-finite conversion boundary | 2026-08-18 |
| Quarkdown v2.5.1 `DynamicValueConverter.kt`   | https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/reflect/DynamicValueConverter.kt | Invocation-time typed conversion boundary reviewed for the gap inventory | 2026-08-18 |
| Quarkdown v2.5.1 `ValueFactory.kt`            | https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/factory/ValueFactory.kt | String-to-number and string-to-boolean conversion behavior, plus the upstream Dynamic String → inline lexer/parser path retained as an explicit Scribium compatibility gap | 2026-08-18 |
| Rust `libm` 0.2.16                         | https://docs.rs/libm/0.2.16/ | Pure-Rust `log`, `sin`, `cos`, and `tan` software implementation selected with default features disabled for native/WASM reproducibility; compared against 0.2.14 and 0.2.15 on the representative corpus | 2026-08-18 |
| Quarkdown wiki — "Syntax of a function call"   | https://quarkdown.com/wiki/syntax-of-a-function-call/ | Documented-but-deferred v2.5.0 constructs: line continuation, `::` chaining, tight/brace-wrapped calls, multi-line arguments | 2026-08-08 |
| Quarkdown wiki — "Syntax of a function call" (v2.5.1 syntax review) | https://quarkdown.com/wiki/syntax-of-a-function-call/ | Behavior specification for the #60 multiline-argument, continuation, chaining, tight-call, and block/inline boundary fixtures | 2026-08-14 |
| Quarkdown wiki — "Lambda" (v2.5.1)              | https://quarkdown.com/wiki/lambda/ | Headerless lambda implicit positional references (`.1`, `.2`, ...), nested scope behavior | 2026-08-16 |
| Quarkdown wiki — "Destructuring" (v2.5.1)      | https://quarkdown.com/wiki/destructuring/ | Pair/Dictionary destructuring eligibility, multi-parameter block headers, and component binding | 2026-08-18 |
| Quarkdown quickstart                          | https://quarkdown.com/                      | Call examples (`.pow {5} to:{2}`, `.align {center}` with an indented body) | 2026-08-08 |
| Quarkdown Core API — `Lambda` class           | https://quarkdown.com/docs/latest/quarkdown-core/com.quarkdown.core.function.value.data/-Lambda/index.html | Implicit positional references (`.1`, `.2`, ...): "If not present, parameter names are automatically set to `.1`, `.2`" | 2026-08-08 |
| Quarkdown stdlib API — `foreach` / `Flow`     | https://quarkdown.com/docs/latest/quarkdown-stdlib/com.quarkdown.stdlib.module.Flow/foreach.html | Iterative calls using implicit references (`**.1**`); iteration index starts at 1 | 2026-08-08 |
| Quarkdown wiki — "Loops" (v2.5.1)          | https://quarkdown.com/wiki/loops/ | `.foreach` maps an iterable to an ordered collection; `.repeat` is `.foreach {1..times}`; explicit and implicit block forms | 2026-08-17 |
| Quarkdown wiki — "Iterable" (v2.5.1)       | https://quarkdown.com/wiki/iterable/ | Markdown ordered/unordered lists become ordered collections; nested collections; integer Range as iterable | 2026-08-17 |
| Quarkdown wiki — "Dictionary" (v2.5.1)     | https://quarkdown.com/wiki/dictionary/ | String keys, recursive values, YAML-like Markdown-list syntax, ordered entry iteration, and nested dictionaries | 2026-08-18 |
| Quarkdown wiki — "Range"                    | https://quarkdown.com/wiki/range/ | Non-negative literal Range syntax, four open/closed endpoint shapes, and dynamic `.range` as the evaluated alternative | 2026-08-18 |
| Quarkdown wiki — "Iterable" (v2.5.1 collection operations review) | https://quarkdown.com/wiki/iterable/ | Collection, Pair, Dictionary, and Range iterable categories; Pair and dictionary-entry behavior; operation chaining | 2026-08-18 |
| Quarkdown stdlib API — `Collection` package | https://quarkdown.com/docs/quarkdown-stdlib/com.quarkdown.stdlib.module.Collection/ | Public signatures and chaining metadata for the v2.5.1 Collection API: `.getat`, `.first`, `.second`, `.third`, `.last`, `.size`, `.sumall`, `.average`, `.distinct`, `.sorted`, `.reversed`, `.groupvalues`, and `.pair` | 2026-08-18 |
| Quarkdown v2.5.1 `Collection.kt` | https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Collection.kt | Public source evidence for one-based access, `asDouble()`-based aggregation, Kotlin equality-based distinct/grouping, reversal, group shape/order, and the absence of public `.map`/`.filter` declarations | 2026-08-18 |
| Quarkdown v2.5.1 `Types.kt` | https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/internal/Types.kt | `Value.asDouble()` conversion: Number values are converted directly, parseable strings are converted, and other values fall back to `0.0` | 2026-08-18 |
| Quarkdown v2.5.1 `IterableTest.kt` | https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-test/src/test/kotlin/com/quarkdown/test/IterableTest.kt | Independent upstream test evidence for `.sumall`, `.average`, `.distinct`, `.reversed`, `.groupvalues`, ordered Pair iteration, and nested iterable results | 2026-08-18 |
| Quarkdown v2.5.1 `IterableValue.kt` | https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/IterableValue.kt | Public behavioral evidence that iterable values expose ordered components and Pair participates in iterable adaptation | 2026-08-18 |
| Quarkdown v2.5.1 `DictionaryValue.kt` | https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/DictionaryValue.kt | Public behavioral evidence that Dictionary adapts to an iterable of key-value Pair entries | 2026-08-18 |
| Quarkdown v2.5.1 `Lambda.kt` | https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/data/Lambda.kt | First-class lambda value, explicit/implicit parameters, optional arguments, forked invocation scope, and lexical parent context | 2026-08-18 |
| Quarkdown v2.5.1 `LambdaValue.kt` | https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/LambdaValue.kt | Lambda as a typed Value wrapper rather than a backend expression | 2026-08-18 |
| Quarkdown v2.5.1 `LambdaTest.kt` | https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-test/src/test/kotlin/com/quarkdown/test/LambdaTest.kt | Public examples for nested implicit scope, explicit inline parameters, callback passing, and legacy `@lambda` syntax | 2026-08-18 |
| Quarkdown v2.5.1 `Collection.kt` transform section | https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Collection.kt | `.sorted(from, by?)` signature, natural-order vs selector behavior, and no public `.map`/`.filter` declarations in the tracked tag | 2026-08-18 |
| Quarkdown v2.5.1 `Sorting.kt` | https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/internal/Sorting.kt | Stable selector-based sorting machinery and null-safe comparator helper used by stdlib sorting | 2026-08-18 |
| Kotlin stdlib `sortedWith` API | https://kotlinlang.org/api/core/kotlin-stdlib/kotlin.collections/sorted-with.html | Public contract that the comparator sort is stable and equal elements preserve relative order | 2026-08-18 |
| Quarkdown v2.5.1 `Range.kt` | https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/data/Range.kt | Public behavioral evidence for inclusive finite Range iteration, left-open default start, and right-open rejection | 2026-08-18 |
| Quarkdown v2.5.1 `ValueFactory.kt` | https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/factory/ValueFactory.kt | Public behavioral evidence for Range/Collection/Dictionary iterable adaptation, plain Markdown-list scalar values, and non-iterable scalar handling | 2026-08-18 |
| Quarkdown v2.5.1 `Flow.kt`                  | https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Flow.kt | Public source evidence that `.repeat` delegates to `forEach(Range(1, times), body)` | 2026-08-17 |
| Quarkdown v2.5.1 `Range.kt`                 | https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/data/Range.kt | Public source evidence for inclusive closed iteration, left-open default start, right-open rejection, and descending bounds | 2026-08-17 |
| Quarkdown v2.5.1 `Math.kt`                  | https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Math.kt | Public source evidence for `.range` optional `from`/`to` bounds, dynamic evaluation, Number-to-Int truncation, and the decimal post-processing operation order | 2026-08-18 |
| Quarkdown v2.5.1 `FlowTest.kt`              | https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/test/kotlin/com/quarkdown/stdlib/FlowTest.kt | Public test evidence for `..4` iteration and `1..` rejection | 2026-08-17 |
| GitHub release tag `v2.5.1`                   | https://github.com/iamgio/quarkdown/releases/tag/v2.5.1 | Release identification and D1-D5 public delta inventory | 2026-08-13 |
| CommonMark specification, current link rules  | https://spec.commonmark.org/current/#links | D2 balanced/escaped link destinations, literal trailing delimiters, and URI backslash-escape semantics | 2026-08-13 |
| CommonMark specification, current autolink rules | https://spec.commonmark.org/current/#autolinks | D2 autolink URI/email grammar and the rule that backslash escapes do not apply inside autolinks | 2026-08-13 |
| CommonMark specification, current list rules  | https://spec.commonmark.org/current/#lists | D3 nested list container and indentation semantics | 2026-08-13 |
| Quarkdown wiki — Markdown content            | https://quarkdown.com/wiki/markdown-content/ | Public body-content interaction for links, lists, and nested block content | 2026-08-13 |
| Quarkdown wiki — Iterable                   | https://quarkdown.com/wiki/iterable/ | Corroborating public scope for nested Markdown list document semantics | 2026-08-13 |
| Quarkdown wiki — Subdocuments                | https://quarkdown.com/wiki/subdocuments/ | Corroborating public scope for D4 local subdocument links and HTML output | 2026-08-13 |

The current evidence set covers the function-call syntax documented on the wiki
page above, plus the conditional constructs (`.if` / `.ifnot`) and implicit
positional references (`.1`, `.2`, ...), at the levels recorded in
`docs/compatibility/quarkdown/README.md`. This is the current verified evidence
baseline, not a permanent restriction on the complete public-language target.
Public features not yet covered remain compatibility debt.

The current verified baseline is **Quarkdown v2.5.1**. The v2.5.0-badged
*"Syntax of a function call"* wiki page is the primary public specification
source for the currently evidenced function-call behavior. Version provenance
is recorded per source:

- The **function-call syntax** page carries a `2.5.0` badge as of
  2026-08-08.
- The **Lambda** wiki page carries a `2.5.1` badge as of 2026-08-16 and
  documents headerless implicit positional references and nested lambda
  scopes.
- The **Conditional statements** wiki page carries a `2.5.0` badge as of
  2026-08-08 and documents `.if` / `.ifnot` conditional semantics.
- The **Boolean** wiki page carries a `2.5.0` badge as of 2026-08-08 and
  documents boolean literals (`true`/`yes`, `false`/`no`, case-insensitive).
- The **`docs/latest/…` API pages** are unversioned and are corroborating
  sources rather than evidence that a behavior was introduced in or
  uniquely belongs to v2.5.0.
- The **v2.5.1 release notes** are the primary source for release
  identification and D1-D5. The CommonMark links/autolinks/lists sections are
  corroborating public behavior specifications for D2/D3. The Markdown
  content and Subdocuments wiki pages corroborate scope and are not claims
  that those behaviors were introduced in v2.5.1.

The sources listed above are the sources consulted for this feature set and
the v2.5.1 impact review.

## Collection and Iterable operations evidence record

The v2.5.1 collection-operation review established the following observable
contract:

- `.size` accepts one iterable operand (named `of` in ordinary calls) and
  returns a non-negative numeric value, including `0` for an empty iterable.
- `.first` and `.last` accept one iterable operand (named `from` in ordinary
  calls), preserve the selected value's type, and return `None` for an empty
  iterable.
- `.getat` accepts an iterable, an integral numeric index, and optional
  `orelse`. Indexing is one-based. Zero, negative, too-large, and out-of-range
  indices use the absence/fallback result; fractional, non-finite, and
  non-numeric indices are invalid for the `Int` parameter.
- `.second` and `.third` are the one-based second and third element accessors;
  insufficient length produces the same `None` absence result as `.first`,
  `.last`, and an out-of-range `.getat` without `orelse`.
- `.sumall` applies `Value.asDouble()` to every element and sums the converted
  values. Non-numeric values convert to `0.0`; an empty collection sums to
  `0.0`. `.average` uses the same conversion for every element, divides by the
  full element count, and therefore produces `NaN` for an empty collection.
- `.distinct` preserves the first occurrence of each upstream-equal value.
  `.reversed` returns a new collection in reverse materialized order.
  `.groupvalues` returns a collection of collections: groups appear in the
  first-seen value order and each group preserves input order, as exercised by
  the v2.5.1 iterable tests.
- The upstream source defines no generic public `.map` or `.filter` in the
  tracked v2.5.1 `Collection.kt`. Scribium retains both as explicit extensions;
  they are excluded from Quarkdown v2.5.1 conformance counts. Upstream `.foreach`
  returns an ordered collection with one result per input element and therefore
  has map-equivalent semantics for the evidenced block form.
- `Pair` is observed as two ordered elements. `Dictionary` is observed as an
  ordered iterable of `Pair(key, value)` entries. Strings are not iterable
  operands for these functions. Closed finite and left-open Ranges participate
  through the shared Scribium materialization policy; right-open and fully-open
  Range values remain representable but standard Iterable consumption rejects
  them as endless.

Scribium routes all operations above, `.foreach`, `.sorted`, and the two
extensions through one typed evaluator materialization path. Distinctness and
grouping use stable linear typed comparison rather than randomized hashing or
debug-string comparison. Number equality is deterministic, including explicit
NaN and signed-zero handling; Pair, Collection, Dictionary, and Range values
are compared recursively without source-span identity. Rich Content and
Callable values use their structural IR equality because the upstream value
classes do not define a public value-equality contract for those cases. This is
recorded as a known behavioral difference rather than an invented upstream
claim.

The result shape remains backend-neutral `IrValue`: `.reversed`, `.distinct`,
and `.groupvalues` return recursive `Collection` values, while aggregation
returns `Number`. Materialization reserves lengths with checked arithmetic and
publishes results only after the operation succeeds, preserving source-backed
diagnostics for endless ranges, invalid operands, and allocation failures.

## Decimal numeric post-processing evidence record

The v2.5.1 [`Math.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Math.kt)
definitions are `truncate(x: Number, decimals: Int)` and
`round(x: Number)`. `truncate` rejects negative Int values at runtime, uses
`x.toInt()` for zero decimals, and otherwise evaluates
`(x.toFloat() * 10.0.pow(decimals)).toInt() / multiplier.toFloat()`. The
`Double` multiplier and `Float` division are observable and are retained as
separate evaluator steps. `round` preserves upstream Int values and otherwise
uses Kotlin `round(x.toFloat()).toInt()`, whose exact halfway behavior is
nearest-even rather than Rust's default ties-away-from-zero behavior.

[`NumberValue.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/NumberValue.kt)
normalizes integral Float values to Int before the builtin result is exposed.
[`DynamicValueConverter.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/reflect/DynamicValueConverter.kt)
and [`ValueFactory.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/factory/ValueFactory.kt)
were reviewed to distinguish an integral numeric representation from a
fractional NumberValue. The decimal slice therefore accepts `2`/`2.0` as an
integral numeric `Int` boundary, rejects `1.5`, quoted numeric text, and NaN
for `decimals`, and preserves the negative-Int runtime failure. The existing
Scribium scalar numeric path remains the adaptation boundary for `x` and does
not introduce a general DynamicValue converter.

The official v2.5.1 macOS arm64 release binary from the
[v2.5.1 release](https://github.com/iamgio/quarkdown/releases/tag/v2.5.1) was
also probed with independently authored inputs because the final conversion
behavior is not determined by `round()` alone. It confirmed `2.5 -> 2`,
`3.5 -> 4`, `-2.5 -> -2`, `-3.5 -> -4`, NaN -> `0`, positive/negative
infinity -> `Int.MAX_VALUE`/`Int.MIN_VALUE`, large finite rounding clamps, and
integral `decimals` values above ordinary decimal precision do not hit an
arbitrary upstream limit. Quoted numeric strings for the `Int` parameter fail
type conversion.

Scribium covers this evidence through the shared `bind_arguments` path,
`integer_argument`, `kotlin_float_to_int`, typed `IrValue::Number` results,
ordinary/named/mixed/chain calls, nested composition, source-backed atomic
failure, and the independently authored
`fixtures/quarkdown-conformance/cases/numeric-decimal-family/input.qd`.

## Transcendental numeric evidence record

The v2.5.1 [`Math.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Math.kt)
definitions are `logn(x: Number)`, `pi()`, `sin(x: Number)`,
`cos(x: Number)`, and `tan(x: Number)`. The four unary functions explicitly
call `x.toFloat()` before Kotlin's Float overload; `.pi` passes
`kotlin.math.PI`, a binary64 `Double`, directly to `NumberValue`. The public
[`MathFunctionsTest.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-test/src/test/kotlin/com/quarkdown/test/MathFunctionsTest.kt)
observes `.pi::truncate {2}` as `3.14`, `.cos {0}` as `1`, `.sin {0}` as
`0`, `.tan {0}` as `0`, `.cos {.pi}` as `-1`, and
`.pi::multiply {2}::cos` as `1`.

The installed official v2.5.1 macOS arm64 release was used as a black-box
oracle for an independently authored edge corpus. Before `NumberValue`
normalization, the direct Float math classes are `ln(1) = +0`, `ln(0) =
-Infinity`, `ln(negative finite) = NaN`, `ln(+Infinity) = +Infinity`, and
`ln(NaN) = NaN`; sine and tangent preserve the sign of `-0`, while cosine
returns `+1`. At the rendered value boundary, `NumberValue` converts integral
Float results to Int: the runtime therefore renders `.logn {0}` as
`-2147483648`, `.logn {-1}` as `NaN`, `.logn {1.40129846e-45}` as
`-103.27893`, `.logn {3.4028235e38}` as `21.487562` after the upstream
integral-Number normalization, and zero trigonometric results as `0`/`1`.
The standard representative outputs `.logn {2}` → `0.6931472`,
`.sin {1}` → `0.84147096`, `.cos {1}` → `0.5403023`, and `.tan {1}` →
`1.5574077` are also retained in the conformance/unit corpus. Quoted
`"2"` is rejected by the upstream Number conversion; Scribium's existing
numeric scalar adaptation remains the explicitly bounded compatibility path
for accepted textual/adapted values.

Kotlin/JVM bytecode for the Float overload expands to `f2d`,
`java.lang.Math.*(double)`, then `d2f`. Scribium reproduces that observable
operation order with `libm` `0.2.16` built as
`default-features = false`: the already-adapted Float is widened to binary64,
the pure-Rust software operation runs, and the result is narrowed to Float.
This avoids Rust `std` transcendental calls, OS libc/libm FFI, and
target-specific intrinsics. Versions `0.2.14`, `0.2.15`, and `0.2.16` were
compared on the representative input corpus and produced identical selected
bits; `0.2.16` is pinned for the reviewed current release. Exact layer tests
use `to_bits()` for finite values and signed zero, while evaluator tests cover
the existing `NumberValue(Float)` integral normalization for NaN and
infinities. The dependency's no-default-feature build is the source/dependency
evidence used for Linux x86_64, macOS arm64, Windows x86_64, and wasm32
portability; repository CI runs the first three natively and checks wasm32
compilation only, with no wasm execution claim.

## Range construction and conversion evidence record

The v2.5.1 [Range wiki](https://quarkdown.com/wiki/range/) documents literal
syntax `A..B`, `..B`, `A..`, and `..`, with non-negative integer literal
endpoints. It also states that the operator is syntactic sugar for `.range`
except that literal endpoints are not dynamically evaluated. The v2.5.1
[`Math.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Math.kt)
source documents optional `from` and `to` Number parameters and calls
`Number.toInt()` for both bounds, so ordinary fractions truncate toward zero.

The core [`Range.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/data/Range.kt)
source establishes `Int?` endpoints, inclusive `IntRange` iteration, a default
left-open start of `1`, and rejection of every range whose end is absent as
endless. Descending finite ranges therefore produce an empty iterator through
the upstream integer-range behavior. The [`Iterable` wiki](https://quarkdown.com/wiki/iterable/)
confirms that integer Range is a valid ordered iterable.

The installed official v2.5.1 macOS arm64 release was independently probed
before implementation. The probes observed `3.9 -> 3`, `-3.9 -> -3`,
`-0.9 -> 0`, NaN -> `0`, positive infinity and values above `Int.MAX_VALUE`
-> `Int.MAX_VALUE`, and negative infinity and values below `Int.MIN_VALUE`
-> `Int.MIN_VALUE`. Literal `2147483647..2147483647` remains finite, while
`2147483648..2147483648` is observed as an endless/open-bound failure rather
than a wrapped integer. Scribium reproduces this boundary behavior with
checked conversion from the frontend's non-negative literal representation to
the signed core `IrRange` domain.

The implemented semantic policy is therefore:

- literal and dynamic construction converge to one typed `IrRange`;
- dynamic `.range` accepts optional positional/named `from` and `to` bounds;
- closed ranges iterate inclusively and descending ranges are empty;
- left-open iterable ranges default to `1`;
- right-open and fully-open ranges are valid representations but fail through
  standard Iterable consumption as endless; and
- cardinality is checked in a wider intermediate domain before checked target
  conversion and fallible reservation, with no arbitrary upper bound.

Public source was consulted only as permitted behavioral/API evidence. No
Quarkdown implementation source, test, or fixture was copied or translated.
The official locally installed release reported `quarkdown version 2.5.1` and
was probed with independently authored documents for Pair access,
Dictionary entry access, closed and descending Ranges, empty access, zero and
negative indices, large indices, fractional indices, `orelse`, and scalar
operands, plus dynamic Range numeric boundaries and literal endpoint limits.
The probes confirmed one-based access, `None` for ordinary misses, typed
fallback values, empty descending Ranges, fractional-index rejection,
non-iterable String behavior, upstream Number-to-Int conversion, and endless
open-range rejection.

## Generic callable and collection-transform evidence record

The v2.5.1 [`Lambda.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/data/Lambda.kt)
and [`LambdaValue.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/LambdaValue.kt)
sources establish that a lambda is a typed first-class value. It retains a
parent lexical context, accepts either explicit named parameters or implicit
`.1`, `.2`, and later positional names, forks a child context for each
invocation, fills omitted optional parameters with `None`, and rejects an
invalid explicit argument count. Invocation also propagates the calling scope
needed by dynamic body arguments. The public v2.5.1 lambda tests provide
examples for nested implicit masking, explicit callback parameters, callback
passing, and the legacy `@lambda` marker.

Scribium represents that semantic value as `IrValue::Callable`. Its body,
parameter spans, definition span, and immutable lexical snapshot stay in the
backend-neutral IR. `.foreach`, `.map`, `.filter`, `.sorted` selector calls,
and first-class callback values all use one evaluator invocation path and one
`coerce_iterable` adaptation path. A callback is evaluated in a fresh child
scope, explicit parameters shadow visible names, and the nearest implicit
lambda scope masks outer `.1`/`.2` references. Transform results are recursive
typed `IrValue`s and are published only after the complete operation succeeds.

The v2.5.1 [`Collection.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Collection.kt)
source exposes `.sorted(from, by?)`. Without `by`, it requires naturally
comparable elements; with `by`, it compares the selector result. It exposes no
descending argument, and no public `.map` or `.filter` declaration is present
in this tracked `v2.5.1` file. The implementation rejects unsortable values
rather than defining an arbitrary order. Its `sortedWith` path uses the Kotlin
stdlib stable-sort contract, where equal elements preserve relative order.
Scribium therefore implements `.sorted` only for homogeneous Number, String,
or Boolean keys, uses a deterministic stable ascending sort, rejects `None`,
and reports heterogeneous or unsupported keys as source-backed diagnostics.
NaN ordering is explicit and deterministic in Scribium; the reviewed Quarkdown
source does not specify a separate NaN rule, so this is not promoted as an
upstream compatibility claim.

`.map` and `.filter` are included because this task explicitly defines them as
the Scribium collection-transform slice, not because public `.map`/`.filter`
definitions were found in the v2.5.1 `Collection.kt` source. Their callback
shape is the shared `by` first-class callable form. `.filter` is deliberately
Boolean-only and rejects `None`, Number, String, Collection, and other
non-Boolean results; this fail-closed policy is an implementation boundary,
not an upstream v2.5.1 compatibility claim. Upstream behavior not evidenced by
the reviewed v2.5.1 sources remains deferred rather than generalized.

Pair and Dictionary callbacks receive the same ordered Pair sequence already
used by `.foreach`; two explicit callback parameters use the existing Pair
destructuring rule. Range callbacks consume the existing finite inclusive or
left-open materialization and reject right-open/fully-open endless ranges.
Markdown lists use the same supported list adaptation. No transform materializes
through text serialization, generated Markdown, or a second parser.

## `.plaintext` behavioral evidence record

The v2.5.1 declaration in `Strings.kt` accepts `InlineMarkdownContent` and
returns a `StringValue` by calling the core `NodeUtils.kt` plain-text helper.
That helper walks the processed inline AST: text and code contribute literal
content; emphasis, strong, strikethrough, and links recurse into their child
content; and soft breaks contribute a newline. The v2.5.1 source path omits
hard-break text and does not traverse image children.

An independently authored runtime probe against the resolved tag commit
`107ec3a9482f10d6f90d7580f8409b46a719d18e` confirmed these observable results:
`A<soft-break>B` becomes `A\nB`, `A<hard-break>B` becomes `AB`,
`[hello](...)` becomes `hello`, `![hello](...)` becomes the empty string,
and empty inline content becomes the empty string. Nested `.uppercase` calls
are resolved before projection. Identifier, number, and boolean arguments
produce their scalar text; a quoted Markdown-bearing string is reparsed by
upstream, which is why Scribium explicitly leaves String →
InlineMarkdownContent conversion unsupported instead of returning syntax
literally. The same probe checked the `content` named form and indented body;
missing, extra, unknown-named, and duplicate bindings fail through the normal
upstream argument binder.

Scribium implements only the already-parsed `IrValue::Content` boundary. The
single evaluator resolves nested calls first, then the builtin recursively
projects `IrInline` values without invoking the Markdown parser, serializing
source, or using Typst. Unresolved calls and unsupported structured values
fail closed with the existing source-backed `E3001` path.

## Observational Method

- Implemented from public documentation and a permitted black-box probe of the
  official v2.5.1 macOS arm64 release; the probe checked successful `.1`
  binding, Dictionary ordering, duplicate-key replacement, Pair/Dictionary
  destructuring, and observed unresolved-reference failures for missing and
  zero-argument `.N` references
- The probe confirmed the `key value:` block-header form, whole-Pair binding
  for one implicit parameter, component binding for two explicit parameters,
  and failure for two-parameter destructuring of a non-Pair item. Explicit
  lambda scope masks implicit `.1` lookup.
- No Quarkdown source code or tests are copied or translated. The v2.5.1
  `Math.kt`, `MathFunctionsTest.kt`, `NumberValue.kt`, and converter links above
  were consulted only as public behavioral/API evidence for the numeric slices:
  `toFloat()` arithmetic, the Float/Double/Float `.truncate` operation order,
  Kotlin ties-to-even `.round`, strict `Int` binding, `Number.toInt()`
  truncation for `.pow` and `.iseven`, and `NaN`/infinity behavior for invalid
  floating domains. The `Flow.kt`,
  `Range.kt`, and `FlowTest.kt` links remain evidence for dynamic endpoint
  conversion and the iteration policy (including left-open, endless,
  descending, and repeat-zero behavior).
- The test inputs in `fixtures/` are independently authored from the
  specification documents above; they are not copied from reference inputs
- Each feature's provenance is recorded in
  `docs/compatibility/quarkdown/README.md`

## Clean-room use boundary

The public source links above are not implementation authorities to copy or
translate. The following remain explicitly prohibited as implementation input:

- copying or translating Quarkdown implementation source code (any language)
- copying or translating Quarkdown internal tests or test fixtures
- Quarkdown themes, CSS, HTML templates
- Quarkdown commit history or internal documentation
- quarkdown-wasm source code
