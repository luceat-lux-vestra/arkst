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
| Quarkdown v2.5.1 stdlib registration/public sweep | [`Stdlib.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Stdlib.kt), [`QFunction.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-native-library-processor/src/main/kotlin/com/quarkdown/processor/annotation/QFunction.kt), and [`Collection.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Collection.kt) | Issue #151 complete public `@QFunction` registration, aliases, exact collection signatures, and pinned 162-name manifest | 2026-08-25 |
| Quarkdown v2.5.1 Unicode string semantics | [`StringCase.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/util/StringCase.kt) and [`Strings.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Strings.kt) | Issue #151 correction: `Char::titlecase` for `.capitalize` and Kotlin `String.startsWith(prefix, ignoreCase)` for `.startswith` | 2026-08-25 |
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
| Quarkdown v2.5.1/current `main` `Text.kt`      | https://raw.githubusercontent.com/iamgio/quarkdown/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Text.kt; https://raw.githubusercontent.com/iamgio/quarkdown/main/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Text.kt | `.br` public name and argumentless `LineBreak.wrappedAsValue()` producer; pinned and current-main files are unchanged for this function | 2026-08-21 |
| Quarkdown v2.5.1 `LineBreak.kt`                 | https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/ast/base/inline/LineBreak.kt | `LineBreak` is the upstream inline hard-break node reused by `.br` | 2026-08-21 |
| Quarkdown v2.5.1 `NodeUtils.kt`               | https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/util/node/NodeUtils.kt | `InlineContent.toPlainText()` projection: text/code and nested formatting/link labels recurse, soft breaks emit a newline, hard breaks and images emit nothing | 2026-08-18 |
| Quarkdown v2.5.1 `Math.kt`                    | https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Math.kt | Public `.sum`, `.subtract`, `.multiply`, `.divide`, `.rem`, `.pow`, `.abs`, `.negate`, `.sqrt`, `.logn`, `.pi`, `.sin`, `.cos`, `.tan`, `.truncate`, `.round`, `.iseven`, and `.range` signatures plus Float/Double/Int operation boundaries | 2026-08-18 |
| Quarkdown v2.5.1 `MathFunctionsTest.kt`       | https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-test/src/test/kotlin/com/quarkdown/test/MathFunctionsTest.kt | Public integration examples for arithmetic chains, nested calls, `.pi::truncate {2}`, zero trigonometry, `.cos {.pi}`, `.pi::multiply {2}::cos`, negative-decimal runtime failure, and fractional-decimal type failure | 2026-08-18 |
| Quarkdown wiki — "Math"                       | https://quarkdown.com/wiki/math/ | Public math-family scope and nested/chained arithmetic examples | 2026-08-18 |
| Quarkdown v2.5.1 `NumberValue.kt`             | https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/NumberValue.kt | Integral Float normalization to Int, including the observable finite/non-finite conversion boundary | 2026-08-18 |
| Quarkdown v2.5.1 `DynamicValueConverter.kt`   | https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/reflect/DynamicValueConverter.kt | Invocation-time typed conversion boundary reviewed for the gap inventory; null/None returns no converted value and conversion is consumed at argument binding | 2026-08-20 |
| Quarkdown v2.5.1 `ValueFactory.kt`            | https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/factory/ValueFactory.kt | Number integer-first/Float fallback, exact Boolean text forms, textual Range forms, scalar `toString`, and context-sensitive Markdown conversion boundaries | 2026-08-20 |
| Quarkdown v2.5.1 `RegularArgumentsBinder.kt`  | https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/call/binding/RegularArgumentsBinder.kt | Actual invocation consumer for DynamicValue target conversion; target type is selected from the bound parameter | 2026-08-20 |
| Quarkdown v2.5.1 value taxonomy | https://github.com/iamgio/quarkdown/tree/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value; https://github.com/iamgio/quarkdown/tree/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/data | Public value categories reviewed for #149: String, Number, Boolean, None, Enum, DynamicValue, Markdown/inline content, Node, Iterable/Collection, Pair, Dictionary, Lambda, Range, Size, Color, and Void/no-output | 2026-08-25 |
| Quarkdown v2.5.1 `FunctionParameter.kt` | https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/FunctionParameter.kt | Parameter name/index, optionality, nullable/body/injected metadata, and the absence of a generic variadic parameter contract | 2026-08-25 |
| Quarkdown v2.5.1 `EnumValue.kt` | https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/EnumValue.kt | Typed enum wrapper and static enum identity boundary; dynamic public-name lookup is consumed through `DynamicValueConverter`/`ValueFactory` | 2026-08-25 |
| Quarkdown v2.5.1 `DynamicValue.kt` | https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/DynamicValue.kt | Raw/evaluable invocation value category, context-sensitive evaluation, and distinction from statically originated typed values | 2026-08-25 |
| Quarkdown v2.5.1 `MarkdownContentValue.kt` / `NodeValue.kt` | https://github.com/iamgio/quarkdown/tree/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value; https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/NodeValue.kt; https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/factory/ValueFactory.kt | Content-bearing versus scalar conversion, String adaptation to inline content, node adaptation, and context-required body conversion | 2026-08-25 |
| Quarkdown v2.5.1 value/binding/conversion audit | [`VALUE_MODEL_AUDIT.md`](VALUE_MODEL_AUDIT.md) | Canonical #149 matrix and independent Scribium evidence index; no upstream source/test/fixture copied or translated | 2026-08-25 |
| Quarkdown v2.5.1 `Flow.kt` | https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Flow.kt | `.if`, `.ifnot`, `.foreach`, `.repeat`, `.function`, `.extend`, `.var`, `.let`, and `.node` evaluation contracts, including lazy bodies, temporary binding, and the value-to-content boundary | 2026-08-25 |
| Quarkdown v2.5.1 `Lambda.kt` | https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/data/Lambda.kt | Definition context, caller-library propagation, explicit/implicit parameters, optionality, destructuring, and child invocation scope | 2026-08-25 |
| Quarkdown v2.5.1 `FunctionExtension.kt` | https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/internal/FunctionExtension.kt | `.extend` target resolution, optional conditions, `.super` parent delegation, argument merging, and chained extensions; no implementation copied | 2026-08-25 |
| Quarkdown v2.5.1 `Optionality.kt` | https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Optionality.kt | None-aware callback and predicate laziness, kept separate from target conversion failures | 2026-08-25 |
| Quarkdown v2.5.1 `Collection.kt` | https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Collection.kt | Ordered selectors, aggregation, distinct/group/reverse/sort semantics, and public absence of `.map`/`.filter` in this source family | 2026-08-25 |
| Quarkdown v2.5.1 programmable call pipeline | [`FunctionCallRefiner.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/parser/FunctionCallRefiner.kt); [`FunctionCallNodeExpander.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/call/FunctionCallNodeExpander.kt); [`NodeOutputValueVisitor.kt`](https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/output/node/NodeOutputValueVisitor.kt) | Raw argument/body representation, invocation expansion, value-to-output boundary, and separation of parser, evaluator, value conversion, IR, and output evidence | 2026-08-25 |
| Quarkdown v2.5.1 programmable-semantics audit | [`PROGRAMMABLE_SEMANTICS_AUDIT.md`](PROGRAMMABLE_SEMANTICS_AUDIT.md) | Canonical #150 inventory and independent black-box evidence record; no upstream source/test/fixture copied or translated | 2026-08-25 |
| Quarkdown v2.5.1 `Lambda.kt`                  | https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/data/Lambda.kt | Actual callable invocation consumer for argument conversion in a captured/child evaluation context | 2026-08-20 |
| Quarkdown v2.5.1 `Range.kt`                   | https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/data/Range.kt | Range endpoint representation and canonical `start..end` string form | 2026-08-20 |
| Quarkdown v2.5.1 `Optionality.kt`             | https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Optionality.kt | Existing `.ifpresent(None)` skip and `.takeif(None)` predicate behavior kept separate from conversion failure and omission | 2026-08-20 |
| Rust `libm` 0.2.16                         | https://docs.rs/libm/0.2.16/ | Pure-Rust `log`, `sin`, `cos`, and `tan` software implementation selected with default features disabled for native/WASM reproducibility; compared against 0.2.14 and 0.2.15 on the representative corpus | 2026-08-18 |
| Quarkdown wiki — "Syntax of a function call"   | https://quarkdown.com/wiki/syntax-of-a-function-call/ | Original v2.5.0 evidence record for line continuation, `::` chaining, tight/brace-wrapped calls, and multi-line arguments; superseded as a status claim by the v2.5.1 review below | 2026-08-08 |
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
| Quarkdown v2.5.1 `Optionality.kt` | https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Optionality.kt | `.none`, `.isnone`, `.otherwise`, lazy `.ifpresent` callback results, and Boolean-only `.takeif` callbacks | 2026-08-19 |
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
| Quarkdown v2.5.1 `Html.kt`                   | https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Html.kt | `.html` one-String signature, `Html(content)` node construction, verbatim/unsanitized contract, and shared `NativeContent` use by `.css` | 2026-08-19 |
| Quarkdown v2.5.1 `Data.kt`                   | https://github.com/iamgio/quarkdown/blob/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Data.kt | `.read` UTF-8 text/newline/range behavior and `.json` recursive data-value mapping; `.csv`, `.listfiles`, and `.filename` remain outside this slice | 2026-08-19 |
| Quarkdown v2.5.1 `Ecosystem.kt`              | https://github.com/iamgio/quarkdown/blob/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Ecosystem.kt | `.include` source evaluation, source-relative working-directory changes, and `share`/`scope`/`subdocument` context modes | 2026-08-19 |
| Quarkdown v2.5.1 `Markdown.kt`               | https://github.com/iamgio/quarkdown/blob/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Markdown.kt | `.markdown` raw `NativeContent` Markdown contract; it is not a resource-file loader | 2026-08-19 |
| Quarkdown v2.5.1 `Html` AST node             | https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/ast/base/block/Html.kt | Generic `Node` shape and raw String payload | 2026-08-19 |
| Quarkdown v2.5.1 HTML documentation          | https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/docs/html.qd | Inline/block examples, target-specific behavior, unsanitized warning, and other-target ignore semantics | 2026-08-19 |
| Quarkdown v2.5.1 function-call lexer/refiner | https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/lexer/patterns/FunctionCallPatterns.kt; https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/parser/FunctionCallRefiner.kt | Inline vs block placement, trimmed inline arguments, plain-text body handling, and dynamic expression ownership | 2026-08-19 |
| Quarkdown v2.5.1 `GrammarUtils.kt` | https://raw.githubusercontent.com/iamgio/quarkdown/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/parser/walker/GrammarUtils.kt | `unescapedMatch()` escaped-delimiter recognition and `balancedDelimitersMatch()` behavior: escaped syntax delimiters do not change call/argument recognition or brace depth for the #148 escaped-delimiter audit | 2026-08-24 |
| Quarkdown v2.5.1 `FunctionCallGrammar.kt` | https://raw.githubusercontent.com/iamgio/quarkdown/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/parser/walker/funcall/FunctionCallGrammar.kt | Shared `IDENTIFIER_PATTERN` for function and optional named-argument identifiers (`[a-zA-Z][a-zA-Z0-9]*|[0-9]+`), numeric identifier/implicit-reference recognition, escaped call and argument delimiter recognition, braced argument balancing, adjacent named-argument delimiter grammar, optional `argumentSeparator` before every inline argument and chain `::`, direct LF line-continuation token, separate `trailingLineContinuation`, tight wrapper, and body-argument lexical boundaries for the #148 audit | 2026-08-24 |
| Quarkdown v2.5.1 `FunctionCallTest.kt` | https://raw.githubusercontent.com/iamgio/quarkdown/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-test/src/test/kotlin/com/quarkdown/test/FunctionCallTest.kt | Pinned integration evidence for continuation before the first argument (`.code \\` followed by `lang:{txt}`), continuation after arguments, continuation plus body, and trailing-content placement | 2026-08-24 |
| Quarkdown v2.5.1 `FunctionCallChainingTest.kt` | https://raw.githubusercontent.com/iamgio/quarkdown/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-test/src/test/kotlin/com/quarkdown/test/FunctionCallChainingTest.kt | Pinned integration evidence for direct `::` chain parsing, nested chain calls, and chain value-flow examples; separator-placement grammar remains sourced from `FunctionCallGrammar.kt` | 2026-08-24 |
| Quarkdown v2.5.1 output visitors/renderers | https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/call/FunctionCallNodeExpander.kt; https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-html/src/main/kotlin/com/quarkdown/rendering/html/node/BaseHtmlNodeRenderer.kt; https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-plaintext/src/main/kotlin/com/quarkdown/rendering/plaintext/node/PlainTextNodeRenderer.kt; https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-markdown/src/main/kotlin/com/quarkdown/rendering/markdown/node/GfmNodeRenderer.kt | Block/inline node placement, HTML verbatim output, and non-HTML empty visitors | 2026-08-19 |
| Quarkdown v2.5.1 function-call pipeline | https://github.com/iamgio/quarkdown/tree/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/call; https://github.com/iamgio/quarkdown/tree/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/pipeline/stages | Walker/refiner/node expansion stages, lazy argument/body handling, binding order, and output materialization | 2026-08-20 |
| Quarkdown v2.5.1 lambda/context model | https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/data/Lambda.kt; https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/context/ScopeContext.kt; https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/context/MutableContext.kt; https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/context/SubdocumentContext.kt | Definition/calling scope, implicit parameters, parameter shadowing, variable-owner mutation, and document-state sharing | 2026-08-20 |
| Quarkdown v2.5.1 layout/document semantics | https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Layout.kt; https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Document.kt; https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/document/DocumentInfo.kt | Row/column/grid input contract, positive grid validation, document read/write dual semantics, and document state fields | 2026-08-20 |
| Quarkdown v2.5.1 document metadata contract | https://quarkdown.com/wiki/document-metadata/; https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Document.kt; https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/document/DocumentInfo.kt | Read/write dual behavior; `.docauthor` first-name getter and append-only shorthand; `.docauthors` nested dictionary getter/setter, append semantics, ordered author info; `.dockeywords` ordered iterable getter/setter and list-body contract; shared document state | 2026-08-24 |
| Scribium bounded `.dockeywords` evidence | [independent conformance case](../../../fixtures/quarkdown-conformance/cases/dockeywords-family/); `crates/scribium-core/src/lib.rs::tests::dockeywords_*`; `crates/scribium-ir/src/lib.rs::tests::document_state_roundtrips_deterministically_and_defaults_for_old_ir` | Replace semantics, bounded String/Identifier/Number/Boolean element conversion, callable sharing, source-backed atomic failures, ordered IR snapshot, and old-IR keyword defaults | 2026-08-24 |
| Quarkdown v2.5.1 theme document-state source | https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Document.kt; https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/document/DocumentTheme.kt; https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/call/binding/RegularArgumentsBinder.kt; https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/parser/FunctionCallRefiner.kt; https://quarkdown.com/wiki/themes/ | `theme(color: String? = null, @LikelyNamed layout: String? = null)`; both regular parameters bind positionally or by name; `@LikelyNamed` is not a runtime restriction; nullable `NoneValue`; regular block body falls back to final `layout`; `FunctionCallRefiner` wraps the body as raw `DynamicValue` text and does not execute nested calls by default; lowercase normalization; `VoidValue`; complete `DocumentTheme` replacement; theme existence validation deferred to rendering | 2026-08-24 |
| Theme documentation/source ambiguity | https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Document.kt; https://quarkdown.com/wiki/themes/ | Public KDoc says omitted components are kept/defaulted, but the pinned v2.5.1 implementation constructs a new `DocumentTheme(color?.lowercase(), layout?.lowercase())` on every call. Scribium follows the pinned observable implementation for this bounded contract: omitted components become null and prior components are not preserved. | 2026-08-24 |
| Scribium bounded `.theme` evidence | [independent conformance case](../../../fixtures/quarkdown-conformance/cases/theme-document-state/); `crates/scribium-core/src/lib.rs::tests::theme_*`; `crates/scribium-ir/src/lib.rs::tests::document_theme_component_shapes_roundtrip_without_collapsing_empty_state` | Two positional or named nullable component bindings, `.none`, mixed forms, duplicate/arity failures, scalar conversion, lowercase normalization, whole-state replacement, explicit empty setter, callable sharing, source-defined shadowing, source-backed atomic failures, front-matter separation, deterministic serde, old-IR theme default, and rejection of block bodies before nested evaluation. Upstream raw block-body fallback is an explicit deferred gap because the current frontend/IR exposes parsed nodes rather than lossless raw body text | 2026-08-24 |
| Quarkdown v2.5.1 `.captionposition` source | https://raw.githubusercontent.com/iamgio/quarkdown/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Document.kt; https://raw.githubusercontent.com/iamgio/quarkdown/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/document/layout/caption/CaptionPosition.kt; https://raw.githubusercontent.com/iamgio/quarkdown/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/document/layout/caption/CaptionPositionInfo.kt; https://raw.githubusercontent.com/iamgio/quarkdown/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/document/DocumentInfo.kt | `captionPosition(default, figures, tables, @Name("code") codeBlocks)`; closed `TOP`/`BOTTOM` enum; initial `default = BOTTOM`; nullable per-kind overrides; `code` source alias; `VoidValue`; partial state construction followed by merge | 2026-08-24 |
| Quarkdown v2.5.1 caption-position merge implementation | https://raw.githubusercontent.com/quarkdown-labs/amber.kt/v2.2.0/processor/src/main/kotlin/com/quarkdown/amber/processors/mergeable/MergeableSourceGenerator.kt; https://raw.githubusercontent.com/iamgio/quarkdown/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/document/layout/caption/CaptionPositionInfo.kt | Amber `@Mergeable` generated `merge(other)` keeps non-nullable receiver properties and falls back to `other` only for receiver-nullable properties; `captionPosition` therefore preserves omitted/null overrides and preserves old overrides when `default` changes | 2026-08-24 |
| Quarkdown v2.5.1 regular argument binding evidence | https://raw.githubusercontent.com/iamgio/quarkdown/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/call/binding/RegularArgumentsBinder.kt; https://raw.githubusercontent.com/iamgio/quarkdown/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/test/kotlin/com/quarkdown/core/RegularArgumentsBinderTest.kt | Binder-owned positional order, named lookup, positional-then-named mixing, `UnnamedArgumentAfterNamedException` ownership and rejection stage, unknown parameter, excess positional arguments, duplicate positional/named binding, and body fallback to the final bindable parameter; this is the #163 ownership evidence and does not make named-argument lexical recognition a binder rule; `@LikelyNamed` is not a runtime named-only rule | 2026-08-24 |
| Quarkdown v2.5.1 caption-position tests and docs | https://raw.githubusercontent.com/iamgio/quarkdown/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-test/src/test/kotlin/com/quarkdown/test/CaptionTest.kt; https://raw.githubusercontent.com/iamgio/quarkdown/107ec3a9482f10d6f90d7580f8409b46a719d18e/docs/caption-position.qd | Public caption-position examples and integration coverage for default, per-kind overrides, and `code` caption targets; rendering behavior is upstream evidence but remains outside this Scribium slice | 2026-08-24 |
| Scribium bounded `.captionposition` evidence | [independent conformance case](../../../fixtures/quarkdown-conformance/cases/captionposition-document-state/); `crates/scribium-core/src/lib.rs::tests::captionposition_*`; `crates/scribium-engine/src/value_conversion.rs`; `crates/scribium-ir/src/lib.rs::tests::document_state_roundtrips_deterministically_and_defaults_for_old_ir` | Closed enum conversion, binding failures, repeated merge/preserve behavior, body rejection, nested rollback, callable sharing, source-defined shadowing, no output, immutable IR snapshot, deterministic serde, and old-IR default compatibility; no caption rendering claim | 2026-08-24 |
| Scribium `.captionposition` block-body compatibility gap | Pinned [`RegularArgumentsBinder.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/call/binding/RegularArgumentsBinder.kt), [`FunctionCallRefiner.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/parser/FunctionCallRefiner.kt), and current Scribium `CallBody`/IR boundary | Upstream maps an indented `.captionposition` body to the final `codeBlocks` parameter as raw `DynamicValue` text. Scribium cannot preserve that lossless raw body at the current frontend/IR boundary, so it rejects the body before evaluation exactly like `.theme`; this is an explicit compatibility gap, not an upstream body-invalid claim | 2026-08-24 |
| Quarkdown v2.5.1 document metadata/localization documentation | https://quarkdown.com/wiki/document-metadata/; https://quarkdown.com/wiki/localization/ | `.doclang` read/write dual behavior; its input is a case-insensitive English full name or an IETF BCP 47 tag, not an allowlist of built-in localization locales; public built-in locale review set is Chinese, English, French, German, Italian, Japanese, Polish, Portuguese, Russian, and Ukrainian; unset default; distinction between setting document language and later localization/rendering consumers | 2026-08-24 |
| Quarkdown v2.5.1 `Document.kt` / `DocumentInfo.kt` | https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Document.kt; https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/document/DocumentInfo.kt | `docLanguage(locale: String? = null)`; getter returns `locale?.localizedName ?: ""`; successful setter replaces `DocumentInfo.locale`; invalid lookup throws; successful setter returns `VoidValue`; locale is part of immutable document information | 2026-08-24 |
| Quarkdown v2.5.1 locale model and JVM loader | https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/localization/Locale.kt; https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/localization/LocaleLoader.kt; https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/localization/jvm/JVMLocale.kt; https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/localization/jvm/JVMLocaleLoader.kt | Observable locale fields: canonical `tag`, language `code`, optional country code, English `displayName`, locale-language `localizedName`; `find` tries case-insensitive English name before `Locale.forLanguageTag`; JVM/system locale data is recorded as upstream behavior only and is not reused by Scribium | 2026-08-24 |
| Quarkdown v2.5.1 `LocaleTest.kt` | https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/test/kotlin/com/quarkdown/core/LocaleTest.kt | Pinned observable examples: `en`/`English` → canonical tag `en`, English name `English`, localized name `English`; `en-US`/`English (United States)` → canonical tag `en-US` and localized name `English (United States)`; `it`/`Italian` → tag `it`, localized name `italiano`; `fr-CA`/`French (Canada)` → tag `fr-CA`, localized name `français (Canada)`; the CJK test directly resolves `ko` (and `zh`/`ja`), establishing `ko` as a reviewed lookup target; case-insensitive lookup and invalid lookup failure | 2026-08-24 |
| Bounded locale table oracle evidence | https://raw.githubusercontent.com/iamgio/quarkdown/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/localization/jvm/JVMLocale.kt; https://raw.githubusercontent.com/iamgio/quarkdown/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/localization/jvm/JVMLocaleLoader.kt | Evidence-time `java.util.Locale` oracle under JDK 25.0.3, following the pinned `JVMLocale` getters: public ten records `zh/Chinese/中文`, `en/English/English`, `fr/French/français`, `de/German/Deutsch`, `it/Italian/italiano`, `ja/Japanese/日本語`, `pl/Polish/polski`, `pt/Portuguese/português`, `ru/Russian/русский`, `uk/Ukrainian/українська`; pinned lookup records `ko/Korean/한국어`, `en-US/English (United States)/English (United States)`, and `fr-CA/French (Canada)/français (Canada)`. This oracle is evidence only; Scribium has no JVM/OS locale dependency. | 2026-08-24 |
| Quarkdown v2.5.1 regular binder/refiner/None source | https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/call/binding/RegularArgumentsBinder.kt; https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/parser/FunctionCallRefiner.kt; https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/NoneValue.kt | Positional/named binding rules, duplicate and excess argument errors, unnamed-after-named rejection, nullable `NoneValue` conversion to null, and raw block-body `DynamicValue` fallback to the final regular parameter without executing nested calls by default | 2026-08-24 |
| Scribium bounded `.doclang` evidence | `crates/scribium-engine/src/locale.rs`; `crates/scribium-core/src/lib.rs::tests::doclang_*`; `crates/scribium-ir/src/lib.rs::tests::document_state_roundtrips_deterministically_and_defaults_for_old_ir`; [independent conformance case](../../../fixtures/quarkdown-conformance/cases/doclang-family/) | Deterministic evidence-backed name-first/tag-second resolution, canonical tag and localized-name snapshot, nullable `.none` getter behavior, positional/named binding and source-backed failures, callable sharing, source-defined shadowing, body rejection before nested evaluation, atomic rollback, surrounding-content preservation, and old-IR locale default | 2026-08-24 |
| Quarkdown v2.5.1 dictionary conversion contract | https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/util/node/conversion/list/MarkdownListToDictionaryValue.kt; https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/value/DictionaryValue.kt; https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/call/binding/RegularArgumentsBinder.kt | YAML-like nested list conversion, empty nested dictionaries, insertion-ordered mutable maps with duplicate replacement, typed dictionary binding, and bounded scalar String evidence | 2026-08-24 |
| Quarkdown v2.5.1 permission infrastructure | https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/permissions/Permission.kt; https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-core/src/main/kotlin/com/quarkdown/core/permissions/MissingPermissionException.kt; https://github.com/iamgio/quarkdown/blob/107ec3a9482f10d6f90d7580f8409b46a719d18e/quarkdown-cli/src/main/kotlin/com/quarkdown/cli/exec/ExecuteCommand.kt | `NativeContent`, default grant, denial diagnostic, and host/CLI allow/deny composition | 2026-08-19 |
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
fractional NumberValue. Because `Int` is a `Number` subtype, the decimal
slice uses the same DynamicValue Number conversion: dynamic text is parsed as
Int first, then Float, and only an integral normalized NumberValue reaches the
`Int` boundary. Dynamic `2`/`2.0` therefore succeed, dynamic `1.5` and NaN
fail, and static StringValue text such as `2` remains rejected. The existing
Scribium scalar numeric path remains the adaptation boundary for `x` and does
not introduce a general DynamicValue converter.

The official v2.5.1 macOS arm64 release binary from the
[v2.5.1 release](https://github.com/iamgio/quarkdown/releases/tag/v2.5.1) was
also probed with independently authored inputs because the final conversion
behavior is not determined by `round()` alone. It confirmed `2.5 -> 2`,
`3.5 -> 4`, `-2.5 -> -2`, `-3.5 -> -4`, NaN -> `0`, positive/negative
infinity -> `Int.MAX_VALUE`/`Int.MIN_VALUE`, large finite rounding clamps, and
integral `decimals` values above ordinary decimal precision do not hit an
arbitrary upstream limit. Dynamic numeric strings for the `Int` parameter use
the Number conversion described above; static StringValue text fails type
conversion.

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
`1.5574077` are also retained in the conformance/unit corpus. The reviewed
`ValueFactory.number` conversion accepts textual numeric input with integer
parsing before Float fallback, and `decimals: Int` inherits that same
DynamicValue conversion before requiring NumberValue-compatible integral
normalization.

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
ordinary non-range String behavior, bounded textual Range conversion,
upstream Number-to-Int conversion, and endless open-range rejection.

## Bounded scalar conversion evidence record

The v2.5.1 [`DynamicValueConverter.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/reflect/DynamicValueConverter.kt)
has two actual consumers in the reviewed core: regular argument binding and
lambda invocation. [`RegularArgumentsBinder.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-core/src/main/kotlin/com/quarkdown/core/function/call/binding/RegularArgumentsBinder.kt)
invokes it only when the bound value is actually `DynamicValue`; a static
`StringValue` does not convert to `Number`, `Boolean`, or `Iterable` and only
adapts through its own String/`InlineMarkdownContent` boundary. Its `null`
result for an unwrapped `None` is distinct from an invalid conversion and from
omission of an optional parameter. The conversion candidates were therefore
classified by invocation origin, target, and current consumer rather than by
copying the upstream object hierarchy.

The reviewed `ValueFactory.kt` rules and Scribium boundaries are:

- `Number`: an existing numeric value is identity-equivalent; DynamicValue
  text is parsed as `Int` first and then `Float`, with no whitespace
  normalization. A static StringValue is not parsed. Invalid text, overflow
  outside both parse paths, and unsupported structured values fail through the
  existing source-backed diagnostic path.
- `Boolean`: an existing Boolean is identity-equivalent; DynamicValue text
  accepts only case-insensitive `true`, `yes`, `false`, or `no`. A static
  StringValue is not truthiness-coerced. Non-empty text and numeric truthiness
  are not added.
- `Range`: DynamicValue text matches only the unsigned decimal `x..y`, `..y`,
  `x..`, or `..` forms. A static StringValue is not parsed as an iterable. The
  conversion does not call the source parser; endpoint overflow follows the
  reviewed `toIntOrNull()` open-end behavior, while standard iterable
  consumption still rejects endless ranges.
- `String`: scalar String/Identifier, Number, Boolean, and typed Range values
  have a bounded textual boundary. `None`, collections, callables, and rich
  document/content values are not blindly stringified.
- `EvaluableString`, `MarkdownContent`, and `InlineMarkdownContent` require
  evaluation context and parser/lexer participation. String → Markdown and
  String → InlineMarkdownContent reparsing is **context-sensitive conversion
  deferred**.
- `Size`, `Sizes`, `Color`, enums, and layout/document values are
  **component/layout conversion deferred** because no current approved
  Scribium consumer requires them. Generic collection/callable conversion is
  **unsupported conversion** in this slice; unreviewed target families remain
  **currently unverified conversion** rather than compatibility claims.

The independently authored fixture at
`fixtures/quarkdown-conformance/cases/dynamic-value-scalar-family/` connects
the Number, Boolean, Range, and String boundaries to existing evaluator
consumers. Unit and compile-level tests cover typed identity, malformed input,
None, callback scope, source-backed failure, and atomic publication. No
Quarkdown source, test, or fixture was copied or translated.

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

## Optionality callback evidence record

The v2.5.1 [`Optionality.kt`](https://raw.githubusercontent.com/iamgio/quarkdown/v2.5.1/quarkdown-stdlib/src/main/kotlin/com/quarkdown/stdlib/Optionality.kt)
source defines `.none`, `.isnone`, `.otherwise`, `.ifpresent`, and `.takeif`.
`.ifpresent` returns `None` without invoking its mapping lambda when the value
is absent; otherwise it returns the lambda's typed result. `.takeif` always
evaluates its condition, including for `None`, requires a Boolean result, and
returns the original value or `None` accordingly. Only `.ifpresent` has an
absence short-circuit; `.takeif` remains callback-observable for `None`.

Scribium implements the reviewed boundary with `IrValue::None` and the shared
`IrValue::Callable` path. First-class `@lambda` values and headerless indented
callback bodies use the existing lexical capture, child-scope, and implicit
parameter machinery. Callback errors remain source-backed and atomic: no
partial optionality result is published. The independently authored evidence
fixture is
`fixtures/quarkdown-conformance/cases/optionality-callback-family/input.qd`;
compile coverage includes lazy absence, named/mixed callback arguments,
capture/shadowing, UTF-8/CRLF spans, and callback failure atomicity.

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
