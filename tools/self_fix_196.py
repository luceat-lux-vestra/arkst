#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def write(path: str, text: str) -> None:
    target = ROOT / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(text, encoding="utf-8")


def sha(path: str) -> str:
    return hashlib.sha256((ROOT / path).read_bytes()).hexdigest()


def replace_once(path: str, old: str, new: str) -> None:
    text = read(path)
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{path}: expected one replacement, found {count}: {old[:100]!r}")
    write(path, text.replace(old, new, 1))


def replace_regex(path: str, pattern: str, replacement: str) -> None:
    text = read(path)
    updated, count = re.subn(pattern, replacement, text, count=1, flags=re.MULTILINE | re.DOTALL)
    if count != 1:
        raise RuntimeError(f"{path}: expected one regex replacement, found {count}: {pattern!r}")
    write(path, updated)


def run(*args: str, capture: bool = False) -> subprocess.CompletedProcess[str]:
    print("+", " ".join(args), flush=True)
    return subprocess.run(args, cwd=ROOT, check=True, text=True, capture_output=capture)


WORD_HELPER = r'''/*
 * Generation-only word-boundary oracle for the exact pinned Eclipse Temurin 25 runtime.
 *
 * This independently authored helper observes the runtime BreakIterator through
 * reflection only during generation. Nothing here is linked into Arkst runtime code.
 */

import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.text.BreakIterator;
import java.util.Locale;

final class DumpWordBreakJdk25 {
    private static final int MIN_CODE_POINT = Character.MIN_CODE_POINT;
    private static final int MAX_CODE_POINT = Character.MAX_CODE_POINT;

    private DumpWordBreakJdk25() {}

    public static void main(String[] args) throws Exception {
        BreakIterator iterator = BreakIterator.getWordInstance(Locale.ROOT);
        Class<?> ruleClass = Class.forName("sun.text.RuleBasedBreakIterator");
        if (!ruleClass.isInstance(iterator)) {
            throw new IllegalStateException("unexpected root word BreakIterator: " + iterator.getClass());
        }

        short[] stateTable = (short[]) field(ruleClass, "stateTable").get(iterator);
        boolean[] endStates = (boolean[]) field(ruleClass, "endStates").get(iterator);
        boolean[] lookaheadStates = (boolean[]) field(ruleClass, "lookaheadStates").get(iterator);
        int numCategories = field(ruleClass, "numCategories").getInt(iterator);
        Method lookupCategory = ruleClass.getDeclaredMethod("lookupCategory", int.class);
        lookupCategory.setAccessible(true);

        if (stateTable.length != endStates.length * numCategories
                || lookaheadStates.length != endStates.length) {
            throw new IllegalStateException("inconsistent root word-break tables");
        }

        System.out.println(String.join("\t",
                "META",
                Integer.toString(numCategories),
                Integer.toString(endStates.length),
                Integer.toString(stateTable.length),
                iterator.getClass().getName()));
        for (int index = 0; index < stateTable.length; index++) {
            System.out.println("STATE\t" + index + "\t" + stateTable[index]);
        }
        for (int index = 0; index < endStates.length; index++) {
            System.out.println("END\t" + index + "\t" + (endStates[index] ? "1" : "0"));
            System.out.println("LOOK\t" + index + "\t" + (lookaheadStates[index] ? "1" : "0"));
        }

        int rangeStart = -1;
        int rangeEnd = -1;
        int rangeCategory = Integer.MIN_VALUE;
        for (int codePoint = MIN_CODE_POINT; codePoint <= MAX_CODE_POINT; codePoint++) {
            if (codePoint >= Character.MIN_SURROGATE && codePoint <= Character.MAX_SURROGATE) {
                if (rangeStart >= 0) {
                    emitRange(rangeStart, rangeEnd, rangeCategory);
                    rangeStart = -1;
                }
                continue;
            }
            int category = (Integer) lookupCategory.invoke(iterator, codePoint);
            if (rangeStart >= 0 && codePoint == rangeEnd + 1 && category == rangeCategory) {
                rangeEnd = codePoint;
            } else {
                if (rangeStart >= 0) {
                    emitRange(rangeStart, rangeEnd, rangeCategory);
                }
                rangeStart = rangeEnd = codePoint;
                rangeCategory = category;
            }
        }
        if (rangeStart >= 0) {
            emitRange(rangeStart, rangeEnd, rangeCategory);
        }
    }

    private static Field field(Class<?> type, String name) throws Exception {
        Field field = type.getDeclaredField(name);
        field.setAccessible(true);
        return field;
    }

    private static void emitRange(int start, int end, int category) {
        System.out.printf(Locale.ROOT, "CAT\t%04X\t%04X\t%d%n", start, end, category);
    }
}
'''

WORD_GENERATOR = r'''#!/usr/bin/env python3
"""Generate the pinned Temurin 25 Locale.ROOT word-break DFA used by Final_Sigma."""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import os
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
HELPER = ROOT / "tools/dump_word_break_jdk25.java"
DEFAULT_OUTPUT = ROOT / "crates/arkst-engine/src/word_break.rs"
UNICODE_GENERATOR = ROOT / "tools/generate_jdk25_unicode_case.py"
MAX_SOURCE_BYTES = 256 * 1024


def load_unicode_generator():
    spec = importlib.util.spec_from_file_location("arkst_unicode_generator", UNICODE_GENERATOR)
    if spec is None or spec.loader is None:
        raise RuntimeError("cannot load pinned Unicode generator")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def environment() -> dict[str, str]:
    result = os.environ.copy()
    result.update({"LC_ALL": "C", "LANG": "C", "TZ": "UTC"})
    return result


def build_oracle(java: Path, javac: Path) -> str:
    with tempfile.TemporaryDirectory(prefix="arkst-jdk25-word-break-") as temporary:
        classes = Path(temporary)
        subprocess.run(
            [str(javac), "-d", str(classes), str(HELPER)],
            cwd=ROOT,
            check=True,
            env=environment(),
        )
        result = subprocess.run(
            [
                str(java),
                "--add-opens=java.base/sun.text=ALL-UNNAMED",
                "-Djava.locale.providers=CLDR",
                "-Duser.language=en",
                "-Duser.country=US",
                "-Duser.timezone=UTC",
                "-cp",
                str(classes),
                "DumpWordBreakJdk25",
            ],
            cwd=ROOT,
            check=True,
            capture_output=True,
            text=True,
            env=environment(),
        )
    return result.stdout


def parse(output: str):
    meta = None
    states: list[int] = []
    ends: list[bool] = []
    looks: list[bool] = []
    categories: list[tuple[int, int, int]] = []
    expected_state = expected_end = expected_look = 0
    for line_number, line in enumerate(output.splitlines(), 1):
        fields = line.split("\t")
        if fields[0] == "META":
            if meta is not None or len(fields) != 5:
                raise ValueError(f"oracle:{line_number}: malformed META")
            meta = (int(fields[1]), int(fields[2]), int(fields[3]), fields[4])
        elif fields[0] == "STATE":
            if len(fields) != 3 or int(fields[1]) != expected_state:
                raise ValueError(f"oracle:{line_number}: malformed STATE")
            states.append(int(fields[2])); expected_state += 1
        elif fields[0] == "END":
            if len(fields) != 3 or int(fields[1]) != expected_end or fields[2] not in {"0", "1"}:
                raise ValueError(f"oracle:{line_number}: malformed END")
            ends.append(fields[2] == "1"); expected_end += 1
        elif fields[0] == "LOOK":
            if len(fields) != 3 or int(fields[1]) != expected_look or fields[2] not in {"0", "1"}:
                raise ValueError(f"oracle:{line_number}: malformed LOOK")
            looks.append(fields[2] == "1"); expected_look += 1
        elif fields[0] == "CAT":
            if len(fields) != 4:
                raise ValueError(f"oracle:{line_number}: malformed CAT")
            categories.append((int(fields[1], 16), int(fields[2], 16), int(fields[3])))
        else:
            raise ValueError(f"oracle:{line_number}: unknown row {fields[0]!r}")
    if meta is None:
        raise ValueError("oracle: missing META")
    num_categories, num_states, state_len, class_name = meta
    if class_name != "sun.text.RuleBasedBreakIterator":
        raise ValueError(f"oracle: unexpected iterator class {class_name!r}")
    if len(states) != state_len or state_len != num_states * num_categories:
        raise ValueError("oracle: state table shape mismatch")
    if len(ends) != num_states or len(looks) != num_states:
        raise ValueError("oracle: state flag shape mismatch")
    if any(state < 0 or state >= num_states for state in states):
        raise ValueError("oracle: state transition outside table")
    previous = -1
    covered = 0
    for start, end, category in categories:
        if start > end or start <= previous:
            raise ValueError("oracle: category ranges overlap or are unsorted")
        if category < -1 or category >= num_categories:
            raise ValueError("oracle: category outside table")
        if start <= 0xDFFF and end >= 0xD800:
            raise ValueError("oracle: surrogate range must not be emitted")
        covered += end - start + 1
        previous = end
    if covered != 0x110000 - 0x800:
        raise ValueError(f"oracle: incomplete scalar coverage: {covered}")
    return num_categories, states, ends, looks, categories


def bool_array(name: str, values: list[bool]) -> str:
    rendered = ", ".join("true" if value else "false" for value in values)
    return f"#[rustfmt::skip]\nstatic {name}: &[bool] = &[{rendered}];\n"


def generate(reference, oracle_sha: str, num_categories: int, states, ends, looks, categories) -> str:
    state_lines = []
    for offset in range(0, len(states), 16):
        state_lines.append("    " + ", ".join(str(value) for value in states[offset:offset+16]) + ",\n")
    category_lines = []
    for offset in range(0, len(categories), 4):
        group = categories[offset:offset+4]
        category_lines.append("    " + ", ".join(
            f"(0x{start:04X}, 0x{end:04X}, {category})" for start, end, category in group
        ) + ",\n")
    return f'''//! Locale.ROOT word-break DFA from the pinned Temurin 25 oracle.
//!
//! GENERATED FILE. This is generation-time data only: Arkst runtime has no JVM,
//! host locale, filesystem, or mutable global dependency. The DFA reproduces the
//! exact word-boundary predicate used by JDK `ConditionalSpecialCasing` for
//! invariant-locale Greek Final_Sigma lowercasing.

#![allow(dead_code)]

pub const REFERENCE_JVM_VERSION: &str = "{reference['java_version']}";
pub const REFERENCE_JVM_RUNTIME_VERSION: &str = "{reference['runtime_version']}";
pub const REFERENCE_JVM_ARCHIVE_SHA256: &str = "{reference['archive_sha256']}";
pub const WORD_BREAK_ORACLE_SHA256: &str = "{oracle_sha}";
pub const WORD_BREAK_NUM_CATEGORIES: usize = {num_categories};
pub const WORD_BREAK_NUM_STATES: usize = {len(ends)};
pub const WORD_BREAK_CATEGORY_RANGE_COUNT: usize = {len(categories)};

pub(crate) fn boundaries(characters: &[char]) -> Vec<bool> {{
    let mut result = vec![false; characters.len() + 1];
    result[0] = true;
    let mut start = 0;
    while start < characters.len() {{
        let next = next_boundary(characters, start);
        debug_assert!(next > start && next <= characters.len());
        if next <= start || next > characters.len() {{
            // Generated table invariants make this unreachable. Fail closed at
            // a single-scalar boundary rather than looping forever if corrupted.
            result[start + 1] = true;
            start += 1;
        }} else {{
            result[next] = true;
            start = next;
        }}
    }}
    result
}}

fn next_boundary(characters: &[char], start: usize) -> usize {{
    const START_STATE: usize = 1;
    const STOP_STATE: usize = 0;
    const IGNORE: i16 = -1;

    let mut result = start + 1;
    let mut lookahead_result = 0;
    let mut state = START_STATE;
    let mut cursor = start;
    while cursor < characters.len() && state != STOP_STATE {{
        let category = category(characters[cursor]);
        if category != IGNORE {{
            state = STATE_TABLE[state * WORD_BREAK_NUM_CATEGORIES + category as usize] as usize;
        }}
        if LOOKAHEAD_STATES[state] {{
            if END_STATES[state] {{
                result = lookahead_result;
            }} else {{
                lookahead_result = cursor + 1;
            }}
        }} else if END_STATES[state] {{
            result = cursor + 1;
        }}
        cursor += 1;
    }}
    if cursor == characters.len() && lookahead_result == characters.len() {{
        result = lookahead_result;
    }}
    result
}}

fn category(character: char) -> i16 {{
    let codepoint = character as u32;
    let index = CATEGORY_RANGES.partition_point(|range| range.0 <= codepoint);
    index
        .checked_sub(1)
        .and_then(|index| CATEGORY_RANGES.get(index))
        .filter(|range| codepoint <= range.1)
        .map_or(-1, |range| range.2)
}}

#[rustfmt::skip]
static STATE_TABLE: &[i16] = &[
{''.join(state_lines)}];
{bool_array('END_STATES', ends)}{bool_array('LOOKAHEAD_STATES', looks)}
#[rustfmt::skip]
static CATEGORY_RANGES: &[(u32, u32, i16)] = &[
{''.join(category_lines)}];

#[cfg(test)]
mod tests {{
    use super::boundaries;

    fn points(text: &str) -> Vec<usize> {{
        boundaries(&text.chars().collect::<Vec<_>>())
            .into_iter()
            .enumerate()
            .filter_map(|(index, boundary)| boundary.then_some(index))
            .collect()
    }}

    #[test]
    fn repeated_punctuation_retains_sequence_sensitive_boundaries() {{
        assert_eq!(points("ΟΣ'Α"), vec![0, 4]);
        assert_eq!(points("ΟΣ''Α"), vec![0, 2, 3, 5]);
        assert_eq!(points("ΟΣ.Α"), vec![0, 4]);
        assert_eq!(points("ΟΣ..Α"), vec![0, 2, 3, 5]);
    }}
}}
'''


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--java", type=Path, required=True)
    parser.add_argument("--archive", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    unicode_generator = load_unicode_generator()
    reference = unicode_generator.manifest()
    unicode_generator.validate_reference(reference)
    unicode_generator.validate_archive(args.archive, reference)
    javac = unicode_generator.validate_java(args.java, reference)

    oracle = build_oracle(args.java, javac)
    oracle_sha = hashlib.sha256(oracle.encode("utf-8")).hexdigest()
    expected_oracle = reference.get("word_break_oracle_output_sha256")
    if expected_oracle not in (None, "PENDING") and expected_oracle != oracle_sha:
        raise ValueError(f"word-break oracle SHA changed: expected {expected_oracle}, got {oracle_sha}")
    parsed = parse(oracle)
    generated = generate(reference, oracle_sha, *parsed)
    generated_bytes = len(generated.encode("utf-8"))
    if generated_bytes >= MAX_SOURCE_BYTES:
        raise ValueError(f"generated word-break source exceeds {MAX_SOURCE_BYTES} bytes")
    generated_sha = hashlib.sha256(generated.encode("utf-8")).hexdigest()
    if args.check:
        if args.output.read_text(encoding="utf-8") != generated:
            raise SystemExit(f"{args.output} is not deterministic generated output")
    else:
        args.output.write_text(generated, encoding="utf-8")

    print(f"word_break_oracle_output_sha256={oracle_sha}")
    print(f"word_break_generated_source_bytes={generated_bytes}")
    print(f"word_break_generated_source_sha256={generated_sha}")
    print(f"word_break_num_categories={parsed[0]}")
    print(f"word_break_num_states={len(parsed[2])}")
    print(f"word_break_category_range_count={len(parsed[4])}")


if __name__ == "__main__":
    main()
'''


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--java", type=Path, required=True)
    parser.add_argument("--archive", type=Path, required=True)
    args = parser.parse_args()

    write("tools/dump_word_break_jdk25.java", WORD_HELPER)
    write("tools/generate_word_break_jdk25.py", WORD_GENERATOR)

    replace_once(
        "crates/arkst-engine/src/lib.rs",
        "pub(crate) mod unicode_case;\n",
        "pub(crate) mod unicode_case;\npub(crate) mod word_break;\n",
    )
    replace_once(
        "crates/arkst-engine/src/builtins.rs",
        "    is_cased as unicode_is_cased, is_final_sigma_context as unicode_is_final_sigma_context,\n",
        "    is_cased as unicode_is_cased,\n",
    )
    replace_regex(
        "crates/arkst-engine/src/builtins.rs",
        r"/// Kotlin/JVM's locale-invariant `String\.lowercase\(\)`.*?\nfn evaluate_plaintext\(",
        '''/// Kotlin/JVM's locale-invariant `String.lowercase()` (`toLowerCase(Locale.ROOT)`).
/// Unconditional mappings come from the pinned full-lowercase table. Greek
/// Final_Sigma uses the exact generated Locale.ROOT word boundaries from the
/// same pinned Temurin 25 runtime, so punctuation sequences are contextual.
pub(crate) fn canonical_lowercase(text: &str) -> String {
    let characters: Vec<_> = text.chars().collect();
    let boundaries = characters
        .iter()
        .any(|&character| character == '\\u{03A3}')
        .then(|| crate::word_break::boundaries(&characters));
    let mut result = String::with_capacity(text.len());
    for (index, &character) in characters.iter().enumerate() {
        if character == '\\u{03A3}'
            && is_final_sigma(
                &characters,
                index,
                boundaries.as_deref().expect("sigma requires word boundaries"),
            )
        {
            result.push('\\u{03C2}');
            continue;
        }
        match unicode_mapping_to_string(&unicode_full_lowercase(character)) {
            Some(mapping) if !mapping.is_empty() => result.push_str(&mapping),
            _ => result.push(character),
        }
    }
    result
}

fn is_final_sigma(characters: &[char], index: usize, boundaries: &[bool]) -> bool {
    debug_assert_eq!(boundaries.len(), characters.len() + 1);
    let mut cursor = index;
    let mut has_cased_before = false;
    while cursor > 0 && !boundaries[cursor] {
        cursor -= 1;
        if unicode_is_cased(characters[cursor]) {
            has_cased_before = true;
            break;
        }
    }
    if !has_cased_before {
        return false;
    }

    cursor = index + 1;
    while cursor < characters.len() && !boundaries[cursor] {
        if unicode_is_cased(characters[cursor]) {
            return false;
        }
        cursor += 1;
    }
    true
}

fn evaluate_plaintext(''',
    )

    repeated_test = r'''

#[test]
fn localize_key_lookup_preserves_sequence_sensitive_jvm_word_boundaries() {
    let single_apostrophe = compile_source(
        r#".doclang {en}
.localization {ui}
    - en
      - οσ'α: ordinary
.localize {ui:ΟΣ'Α}
"#,
    );
    assert!(single_apostrophe.diagnostics.is_empty(), "{single_apostrophe:?}");
    assert_eq!(output_text(&single_apostrophe), "ordinary");

    let repeated_apostrophe = compile_source(
        r#".doclang {en}
.localization {ui}
    - en
      - ος''α: final
.localize {ui:ΟΣ''Α}
"#,
    );
    assert!(repeated_apostrophe.diagnostics.is_empty(), "{repeated_apostrophe:?}");
    assert_eq!(output_text(&repeated_apostrophe), "final");

    let repeated_period = compile_source(
        r#".doclang {en}
.localization {ui}
    - en
      - ος..α: final
.localize {ui:ΟΣ..Α}
"#,
    );
    assert!(repeated_period.diagnostics.is_empty(), "{repeated_period:?}");
    assert_eq!(output_text(&repeated_period), "final");
}
'''
    marker = "\n#[test]\nfn localize_key_lookup_is_not_general_case_insensitive()"
    replace_once("crates/arkst-core/tests/quarkdown_localization.rs", marker, repeated_test + marker)

    corpus_path = "tools/jdk25_unicode_corpus.tsv"
    corpus = read(corpus_path)
    if "LOWER\tΟΣ''Α" not in corpus:
        corpus = corpus.rstrip() + "\nLOWER\tΟΣ'Α\nLOWER\tΟΣ''Α\nLOWER\tΟΣ.Α\nLOWER\tΟΣ..Α\n"
        write(corpus_path, corpus)

    replace_once(
        "tools/verify_jdk25_unicode.py",
        '''    subprocess.run(\n        [\n            sys.executable,\n            str(GENERATOR),\n            "--java",\n            str(args.java),\n            "--archive",\n            str(args.archive),\n            "--check",\n        ],\n        cwd=ROOT,\n        check=True,\n        env=environment(),\n    )\n''',
        '''    subprocess.run(\n        [\n            sys.executable,\n            str(GENERATOR),\n            "--java",\n            str(args.java),\n            "--archive",\n            str(args.archive),\n            "--check",\n        ],\n        cwd=ROOT,\n        check=True,\n        env=environment(),\n    )\n    subprocess.run(\n        [\n            sys.executable,\n            str(ROOT / "tools/generate_word_break_jdk25.py"),\n            "--java",\n            str(args.java),\n            "--archive",\n            str(args.archive),\n            "--check",\n        ],\n        cwd=ROOT,\n        check=True,\n        env=environment(),\n    )\n''',
    )

    # Add fail-closed provenance checks for the new generator/helper/artifact.
    replace_once(
        "tools/verify_reference_provenance.py",
        '''        ("unicode_verifier_path", "unicode_verifier_source_sha256", "JDK Unicode verifier"),\n''',
        '''        ("unicode_verifier_path", "unicode_verifier_source_sha256", "JDK Unicode verifier"),\n        ("word_break_helper_path", "word_break_helper_source_sha256", "JDK word-break helper"),\n        ("word_break_generator_path", "word_break_generator_source_sha256", "JDK word-break generator"),\n''',
    )
    provenance_block = '''\n    word_break_path = relative_path(root, reference.get("word_break_generated_source_path"), "JDK word-break artifact path")\n    word_break_bytes = require_int(reference, "word_break_generated_source_bytes", "JDK word-break artifact bytes")\n    word_break_limit = require_int(reference, "word_break_generated_source_limit_bytes", "JDK word-break artifact limit")\n    if word_break_bytes > word_break_limit:\n        raise VerificationError("JDK word-break artifact exceeds its size limit")\n    check_artifact(\n        word_break_path,\n        label="JDK word-break generated Rust",\n        expected_bytes=word_break_bytes,\n        minimum_bytes=word_break_bytes,\n        maximum_bytes=word_break_bytes,\n        expected_sha256=require_sha(reference, "word_break_generated_source_sha256", "JDK word-break artifact SHA-256"),\n        policy="exact",\n    )\n    word_break_source = word_break_path.read_text(encoding="utf-8")\n    for name, expected in (\n        ("REFERENCE_JVM_VERSION", reference["java_version"]),\n        ("REFERENCE_JVM_RUNTIME_VERSION", reference["runtime_version"]),\n        ("REFERENCE_JVM_ARCHIVE_SHA256", reference["archive_sha256"]),\n        ("WORD_BREAK_ORACLE_SHA256", reference["word_break_oracle_output_sha256"]),\n        ("WORD_BREAK_NUM_CATEGORIES", reference["word_break_num_categories"]),\n        ("WORD_BREAK_NUM_STATES", reference["word_break_num_states"]),\n        ("WORD_BREAK_CATEGORY_RANGE_COUNT", reference["word_break_category_range_count"]),\n    ):\n        check_rust_const(word_break_source, name, expected, "JDK word-break artifact")\n'''
    replace_once(
        "tools/verify_reference_provenance.py",
        "\n    locale_path = relative_path(root, reference.get(\"locale_generated_rust_path\"), \"JDK locale artifact path\")\n",
        provenance_block + "\n    locale_path = relative_path(root, reference.get(\"locale_generated_rust_path\"), \"JDK locale artifact path\")\n",
    )

    replace_once(
        "docs/compatibility/quarkdown/reference-jvm.md",
        '''- `Character.isLowerCase`, `Character.isUpperCase`, and\n  `Character.isTitleCase`, which provide the pinned `Cased` property used by\n  invariant-locale contextual final-sigma lowering.\n''',
        '''- `Character.isLowerCase`, `Character.isUpperCase`, and\n  `Character.isTitleCase`, which provide the pinned `Cased` property used by\n  invariant-locale contextual final-sigma lowering; and\n- the exact `Locale.ROOT` `RuleBasedBreakIterator` forward DFA/category mapping\n  used by JDK `ConditionalSpecialCasing.isFinalCased`, captured into a separate\n  generated `word_break.rs` so sequence-sensitive word boundaries do not depend\n  on a host JVM or locale at runtime.\n''',
    )
    replace_once(
        "docs/compatibility/quarkdown/reference-jvm.md",
        '''The generated data contains `2933` non-identity scalar rows and all `65536`\nUTF-16 code-unit rows.\n''',
        '''The generated case data contains `2933` non-identity scalar rows and all\n`65536` UTF-16 code-unit rows. Final-sigma word segmentation is separately\ngenerated from the same pinned runtime's root word-break DFA; the Reference JVM\ncheck regenerates both artifacts byte-for-byte.\n''',
    )
    replace_once(
        "docs/compatibility/quarkdown/LOCALIZATION_PROBES.md",
        '''An entry containing only the context-insensitive `οσ` key (U+03BF U+03C3)\nfailed with `LocalizationKeyNotFoundException`; it did not match the pinned\nlowercase result `ος` or the original `ΟΣ`. This negative case prevents an\nincorrect simple per-scalar sigma mapping from passing.\n''',
        '''An entry containing only the context-insensitive `οσ` key (U+03BF U+03C3)\nfailed with `LocalizationKeyNotFoundException`; it did not match the pinned\nlowercase result `ος` or the original `ΟΣ`. This negative case prevents an\nincorrect simple per-scalar sigma mapping from passing.\n\nThe repository's pinned Temurin differential additionally gates sequence-sensitive\nword boundaries that a one-character sigma probe cannot prove. In particular,\n`ΟΣ'Α` lowercases with ordinary sigma while `ΟΣ''Α` lowercases with final sigma;\nthe Reference JVM corpus checks both forms (and the analogous single/repeated\nperiod forms) against the exact Temurin 25 runtime before `.localize` can rely on\nthe generated word-break DFA.\n''',
    )

    # Insert manifest fields with placeholders, then generate exact values.
    manifest_path = "docs/compatibility/quarkdown/reference-jvm.toml"
    manifest = read(manifest_path)
    insertion = '''\n# Exact Locale.ROOT word-break DFA used by ConditionalSpecialCasing Final_Sigma.\nword_break_helper_path = "tools/dump_word_break_jdk25.java"\nword_break_helper_source_sha256 = "PENDING"\nword_break_generator_path = "tools/generate_word_break_jdk25.py"\nword_break_generator_source_sha256 = "PENDING"\nword_break_oracle_output_sha256 = "PENDING"\nword_break_generated_source_path = "crates/arkst-engine/src/word_break.rs"\nword_break_generated_source_bytes = 0\nword_break_generated_source_sha256 = "PENDING"\nword_break_generated_source_limit_bytes = 262144\nword_break_num_categories = 0\nword_break_num_states = 0\nword_break_category_range_count = 0\n'''
    anchor = 'unicode_generated_source_limit_bytes = 1048576\n'
    if insertion.strip() not in manifest:
        if manifest.count(anchor) != 1:
            raise RuntimeError("manifest word-break anchor not unique")
        manifest = manifest.replace(anchor, anchor + insertion, 1)
    write(manifest_path, manifest)

    generation = run(
        sys.executable,
        "tools/generate_word_break_jdk25.py",
        "--java",
        str(args.java),
        "--archive",
        str(args.archive),
        capture=True,
    )
    print(generation.stdout, end="")
    metadata = {}
    for line in generation.stdout.splitlines():
        if "=" in line:
            key, value = line.split("=", 1)
            metadata[key] = value

    manifest = read(manifest_path)
    dynamic = {
        "word_break_helper_source_sha256": sha("tools/dump_word_break_jdk25.java"),
        "word_break_generator_source_sha256": sha("tools/generate_word_break_jdk25.py"),
        "word_break_oracle_output_sha256": metadata["word_break_oracle_output_sha256"],
        "word_break_generated_source_bytes": metadata["word_break_generated_source_bytes"],
        "word_break_generated_source_sha256": metadata["word_break_generated_source_sha256"],
        "word_break_num_categories": metadata["word_break_num_categories"],
        "word_break_num_states": metadata["word_break_num_states"],
        "word_break_category_range_count": metadata["word_break_category_range_count"],
    }
    for key, value in dynamic.items():
        if key.endswith("sha256"):
            replacement = f'{key} = "{value}"'
            manifest = re.sub(rf'^{re.escape(key)} = ".*"$', replacement, manifest, count=1, flags=re.MULTILINE)
        else:
            manifest = re.sub(rf'^{re.escape(key)} = \d+$', f'{key} = {value}', manifest, count=1, flags=re.MULTILINE)

    corpus = (ROOT / corpus_path).read_bytes()
    corpus_count = sum(1 for line in corpus.decode("utf-8").splitlines() if line and not line.startswith("#"))
    corpus_sha = hashlib.sha256(corpus).hexdigest()
    for key in ("unicode_corpus_bytes", "unicode_corpus_min_bytes", "unicode_corpus_max_bytes"):
        manifest = re.sub(rf'^{key} = \d+$', f'{key} = {len(corpus)}', manifest, count=1, flags=re.MULTILINE)
    manifest = re.sub(r'^unicode_corpus_record_count = \d+$', f'unicode_corpus_record_count = {corpus_count}', manifest, count=1, flags=re.MULTILINE)
    manifest = re.sub(r'^unicode_corpus_sha256 = ".*"$', f'unicode_corpus_sha256 = "{corpus_sha}"', manifest, count=1, flags=re.MULTILINE)

    # Source identities changed because the verifier now knows the word-break artifact.
    manifest = re.sub(
        r'^unicode_verifier_source_sha256 = ".*"$',
        f'unicode_verifier_source_sha256 = "{sha("tools/verify_jdk25_unicode.py")}"',
        manifest,
        count=1,
        flags=re.MULTILINE,
    )
    manifest = re.sub(
        r'^verifier_source_sha256 = ".*"$',
        f'verifier_source_sha256 = "{sha("tools/verify_reference_provenance.py")}"',
        manifest,
        count=1,
        flags=re.MULTILINE,
    )
    write(manifest_path, manifest)

    # Ensure generated data and the external pinned-JVM differential are exact.
    run(sys.executable, "tools/generate_word_break_jdk25.py", "--java", str(args.java), "--archive", str(args.archive), "--check")
    run(sys.executable, "tools/verify_jdk25_unicode.py", "--java", str(args.java), "--archive", str(args.archive))
    run("cargo", "fmt", "--all", "--", "--check")
    run("cargo", "test", "-p", "arkst-engine", "--all-targets", "--all-features")
    run("cargo", "test", "-p", "arkst-core", "--test", "quarkdown_localization")
    run("git", "diff", "--check")


if __name__ == "__main__":
    main()
