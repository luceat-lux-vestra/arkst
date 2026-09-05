#!/usr/bin/env python3
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
    return f"""//! Locale.ROOT word-break DFA from the pinned Temurin 25 oracle.
//!
//! GENERATED FILE. This is generation-time data only: Arkst runtime has no JVM,
//! host locale, filesystem, or mutable global dependency. The DFA reproduces the
//! exact word-boundary predicate used by JDK `ConditionalSpecialCasing` for
//! invariant-locale Greek Final_Sigma lowercasing.

#![allow(dead_code)]

pub const REFERENCE_JVM_VERSION: &str = "{reference['java_version']}";
pub const REFERENCE_JVM_RUNTIME_VERSION: &str = "{reference['runtime_version']}";
pub const REFERENCE_JVM_ARCHIVE_SHA256: &str =
    "{reference['archive_sha256']}";
pub const WORD_BREAK_ORACLE_SHA256: &str =
    "{oracle_sha}";
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
        assert_eq!(points("ΟΣ''Α"), vec![0, 2, 3, 4, 5]);
        assert_eq!(points("ΟΣ.Α"), vec![0, 4]);
        assert_eq!(points("ΟΣ..Α"), vec![0, 2, 3, 4, 5]);
    }}
}}
"""


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
