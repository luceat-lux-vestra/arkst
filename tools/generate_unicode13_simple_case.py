#!/usr/bin/env python3
"""Generate the engine's Unicode 13 simple case mapping ranges.

The input is the official Unicode 13.0.0 UnicodeData.txt. The generator
verifies the exact pinned input before reading any case mappings. The generated
range tables preserve only mappings present in the file's simple uppercase,
simple lowercase, and simple titlecase columns; full mappings are intentionally
not inferred.
"""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path


UNICODE_DATA_SIZE = 1_851_767
# Published Unicode 13.0.0 UnicodeData.txt SHA-256 fingerprint prefix.
UNICODE_DATA_SHA256_PREFIX = "bdbffbbfc8ad"
# Exact Git blob identity of ICU 67.1's vendored Unicode 13.0.0 UnicodeData.txt.
# Git blob hashes cover both the byte length and complete file contents.
UNICODE_DATA_GIT_BLOB_SHA1 = "e22f967bbab8f2477a43533a334e21ebc0728eda"


def validate_source(source: Path) -> None:
    data = source.read_bytes()
    if len(data) != UNICODE_DATA_SIZE:
        raise ValueError(
            f"{source}: expected UnicodeData.txt byte length {UNICODE_DATA_SIZE}, "
            f"got {len(data)}"
        )

    sha256 = hashlib.sha256(data).hexdigest()
    if not sha256.startswith(UNICODE_DATA_SHA256_PREFIX):
        raise ValueError(
            f"{source}: expected UnicodeData.txt SHA-256 prefix "
            f"{UNICODE_DATA_SHA256_PREFIX}, got {sha256}"
        )

    git_blob = f"blob {len(data)}\0".encode("ascii") + data
    git_blob_sha1 = hashlib.sha1(git_blob).hexdigest()
    if git_blob_sha1 != UNICODE_DATA_GIT_BLOB_SHA1:
        raise ValueError(
            f"{source}: expected pinned UnicodeData.txt Git blob SHA-1 "
            f"{UNICODE_DATA_GIT_BLOB_SHA1}, got {git_blob_sha1}"
        )


def read_mapping(source: Path, field: int) -> list[tuple[int, int]]:
    mappings = []
    for line_number, line in enumerate(source.read_text(encoding="utf-8").splitlines(), 1):
        if not line or line.startswith("#"):
            continue
        fields = line.split(";")
        if len(fields) < 15:
            raise ValueError(f"{source}:{line_number}: expected UnicodeData fields")
        value = fields[field]
        if value:
            codepoint = int(fields[0], 16)
            mapped = int(value, 16)
            mappings.append((codepoint, mapped))
    return mappings


def ranges(mapping: list[tuple[int, int]]) -> list[tuple[int, int, int]]:
    result: list[tuple[int, int, int]] = []
    start = previous = delta = None
    for codepoint, mapped in mapping:
        current_delta = mapped - codepoint
        if (
            start is not None
            and codepoint == previous + 1
            and current_delta == delta
        ):
            previous = codepoint
            continue
        if start is not None:
            result.append((start, previous, delta))
        start = previous = codepoint
        delta = current_delta
    if start is not None:
        result.append((start, previous, delta))
    return result


def rust_ranges(name: str, mapping: list[tuple[int, int]]) -> str:
    rows = ranges(mapping)
    lines = [f"#[rustfmt::skip]\nstatic {name}: &[(u32, u32, i32)] = &[\n"]
    for offset in range(0, len(rows), 4):
        group = rows[offset : offset + 4]
        lines.append(
            "    "
            + ", ".join(
                f"(0x{start:04X}, 0x{end:04X}, {delta})"
                for start, end, delta in group
            )
            + ",\n"
        )
    lines.append("];\n")
    return "".join(lines)


def generate(source: Path) -> str:
    validate_source(source)
    uppercase = read_mapping(source, 12)
    lowercase = read_mapping(source, 13)
    titlecase = read_mapping(source, 14)
    return f'''//! Unicode 13.0 simple case mappings for JVM-compatible character comparison.
//!
//! GENERATED FILE. Source: Unicode Character Database 13.0.0,
//! `UnicodeData.txt`, fields 12, 13, and 14 (Simple_Uppercase_Mapping,
//! Simple_Lowercase_Mapping, and Simple_Titlecase_Mapping):
//! <https://www.unicode.org/Public/13.0.0/ucd/UnicodeData.txt>
//! UCD licensing: <https://www.unicode.org/license.txt>
//!
//! Do not infer a simple mapping from a full mapping. The distinction matters
//! for characters such as U+1F80, whose full uppercase mapping has two scalars
//! while its UnicodeData simple uppercase mapping is U+1F88.

pub const UNICODE_VERSION: (u64, u64, u64) = (13, 0, 0);

pub(crate) fn simple_uppercase(character: char) -> char {{
    lookup(character, SIMPLE_UPPERCASE_RANGES).unwrap_or(character)
}}

pub(crate) fn simple_lowercase(character: char) -> char {{
    lookup(character, SIMPLE_LOWERCASE_RANGES).unwrap_or(character)
}}

pub(crate) fn simple_titlecase_mapping(character: char) -> Option<char> {{
    lookup(character, SIMPLE_TITLECASE_RANGES)
}}

fn lookup(character: char, ranges: &[(u32, u32, i32)]) -> Option<char> {{
    let codepoint = character as u32;
    let index = ranges.partition_point(|range| range.0 <= codepoint);
    let range = index.checked_sub(1).and_then(|index| ranges.get(index));
    let &(start, _end, delta) = range.filter(|range| codepoint <= range.1)?;
    debug_assert!(codepoint >= start);
    char::from_u32((codepoint as i64 + i64::from(delta)) as u32)
}}

{rust_ranges("SIMPLE_UPPERCASE_RANGES", uppercase)}
{rust_ranges("SIMPLE_LOWERCASE_RANGES", lowercase)}
{rust_ranges("SIMPLE_TITLECASE_RANGES", titlecase)}'''


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("source", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    args.output.write_text(generate(args.source), encoding="utf-8")


if __name__ == "__main__":
    main()
