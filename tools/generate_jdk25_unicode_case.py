#!/usr/bin/env python3
"""Generate the engine's Unicode case tables from the pinned Temurin 25 API.

The Java helper is the compatibility oracle. It emits the public JDK
Character/String results needed by the Kotlin/JVM string operations; this
generator validates the complete scalar and UTF-16 domains before producing a
small safe Rust lookup table. The oracle output is transient and is never
checked into the repository.
"""

from __future__ import annotations

import argparse
import hashlib
import os
import re
import subprocess
import tempfile
from pathlib import Path
from typing import Iterable

try:
    import tomllib
except ModuleNotFoundError as error:  # pragma: no cover - generator requires Python 3.11+
    raise SystemExit("Python 3.11 or newer is required for the reference manifest") from error


ROOT = Path(__file__).resolve().parents[1]
MANIFEST_PATH = ROOT / "docs/compatibility/quarkdown/reference-jvm.toml"
HELPER_PATH = ROOT / "tools/dump_jdk25_unicode_data.java"
DEFAULT_OUTPUT = ROOT / "crates/arkst-engine/src/unicode_case.rs"
MAX_CODE_POINT = 0x10FFFF
MAX_CODE_UNIT = 0xFFFF
MAX_GENERATED_SOURCE_BYTES = 1024 * 1024


def validate_reference(reference: dict[str, object]) -> None:
    required_values = {
        "distribution": "Eclipse Temurin",
        "java_version": "25.0.4.1",
        "runtime_version": "25.0.4.1+1-LTS",
        "release_tag": "jdk-25.0.4.1+1",
        "locale_provider": "CLDR",
        "unicode_version": "16.0.0",
    }
    for key, expected in required_values.items():
        actual = str(reference.get(key, ""))
        if actual != expected:
            raise ValueError(f"reference field {key} must be {expected!r}, got {actual!r}")


def manifest() -> dict[str, object]:
    with MANIFEST_PATH.open("rb") as stream:
        data = tomllib.load(stream)
    reference = data.get("reference")
    if not isinstance(reference, dict):
        raise ValueError(f"{MANIFEST_PATH}: missing [reference] table")
    return reference


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def validate_archive(archive: Path, reference: dict[str, object]) -> None:
    expected_name = str(reference["archive_filename"])
    if archive.name != expected_name:
        raise ValueError(f"expected archive named {expected_name}, got {archive.name}")
    expected_size = int(reference["archive_bytes"])
    actual_size = archive.stat().st_size
    if actual_size != expected_size:
        raise ValueError(f"{archive}: expected {expected_size} bytes, got {actual_size}")
    expected_sha = str(reference["archive_sha256"])
    actual_sha = sha256(archive)
    if actual_sha != expected_sha:
        raise ValueError(f"{archive}: expected SHA-256 {expected_sha}, got {actual_sha}")


def java_properties(java: Path) -> tuple[str, str, str, str, str]:
    result = subprocess.run(
        [str(java), "-XshowSettings:properties", "-version"],
        check=True,
        capture_output=True,
        text=True,
        env=deterministic_environment(),
    )
    output = result.stdout + result.stderr

    def property_value(name: str) -> str:
        match = re.search(rf"^\s*{re.escape(name)}\s*=\s*(.+)$", output, re.MULTILINE)
        if match is None:
            raise ValueError(f"{java}: missing {name} in java property output")
        return match.group(1).strip()

    return (
        property_value("java.version"),
        property_value("java.runtime.version"),
        property_value("java.vendor.version"),
        property_value("java.vendor"),
        property_value("java.version.date"),
    )


def validate_java(java: Path, reference: dict[str, object]) -> Path:
    if not java.is_file():
        raise ValueError(f"JDK executable does not exist: {java}")
    version, runtime_version, vendor_version, vendor, version_date = java_properties(java)
    expected_version = str(reference["java_version"])
    expected_runtime = str(reference["runtime_version"])
    expected_vendor_version = str(reference["implementor_version"])
    expected_vendor = str(reference["implementor"])
    expected_version_date = str(reference["java_version_date"])
    if version != expected_version:
        raise ValueError(f"expected java.version {expected_version}, got {version}")
    if runtime_version != expected_runtime:
        raise ValueError(f"expected java.runtime.version {expected_runtime}, got {runtime_version}")
    if vendor_version != expected_vendor_version:
        raise ValueError(
            f"expected java.vendor.version {expected_vendor_version}, got {vendor_version}"
        )
    if vendor != expected_vendor:
        raise ValueError(f"expected java.vendor {expected_vendor}, got {vendor}")
    if version_date != expected_version_date:
        raise ValueError(f"expected java.version.date {expected_version_date}, got {version_date}")
    version_result = subprocess.run(
        [str(java), "-version"],
        check=True,
        capture_output=True,
        text=True,
        env=deterministic_environment(),
    )
    expected_version_output = reference.get("java_version_output")
    if isinstance(expected_version_output, list):
        actual_version_output = version_result.stderr.splitlines()
        if actual_version_output != expected_version_output:
            raise ValueError(
                "java -version output changed: "
                f"expected {expected_version_output!r}, got {actual_version_output!r}"
            )
    javac = java.parent / "javac"
    if not javac.is_file():
        raise ValueError(f"matching javac executable does not exist: {javac}")
    return javac


def deterministic_environment() -> dict[str, str]:
    environment = os.environ.copy()
    environment.update({"LC_ALL": "C", "LANG": "C", "TZ": "UTC"})
    return environment


def parse_codepoint(value: str, *, allow_surrogate: bool = False) -> int:
    codepoint = int(value, 16)
    if not 0 <= codepoint <= MAX_CODE_POINT:
        raise ValueError(f"invalid Unicode code point {value}")
    if not allow_surrogate and 0xD800 <= codepoint <= 0xDFFF:
        raise ValueError(f"unexpected surrogate mapping {value}")
    return codepoint


def parse_sequence(value: str) -> tuple[int, ...]:
    if value == "-":
        return ()
    return tuple(parse_codepoint(item, allow_surrogate=True) for item in value.split(","))


def parse_oracle(output: str) -> tuple[list[tuple[int, int, int]], list[dict[str, object]]]:
    scalar_rows: list[tuple[int, int, int]] = []
    char_rows: list[dict[str, object]] = []
    scalar_previous = -1
    char_previous = -1
    saw_chars = False
    for line_number, line in enumerate(output.splitlines(), 1):
        fields = line.split("\t")
        if fields[0] == "SCALAR":
            if saw_chars or len(fields) != 4:
                raise ValueError(f"oracle:{line_number}: malformed SCALAR row")
            codepoint = parse_codepoint(fields[1])
            uppercase = parse_codepoint(fields[2])
            lowercase = parse_codepoint(fields[3])
            if codepoint <= scalar_previous:
                raise ValueError(f"oracle:{line_number}: SCALAR rows are not sorted")
            if uppercase == codepoint and lowercase == codepoint:
                raise ValueError(f"oracle:{line_number}: identity SCALAR row was not omitted")
            scalar_rows.append((codepoint, uppercase, lowercase))
            scalar_previous = codepoint
        elif fields[0] == "CHAR":
            saw_chars = True
            if len(fields) != 8:
                raise ValueError(f"oracle:{line_number}: malformed CHAR row")
            code_unit = int(fields[1], 16)
            if not 0 <= code_unit <= MAX_CODE_UNIT or code_unit != char_previous + 1:
                raise ValueError(f"oracle:{line_number}: CHAR rows are not complete and sorted")
            char_rows.append(
                {
                    "code_unit": code_unit,
                    "simple_upper": parse_codepoint(fields[2], allow_surrogate=True),
                    "simple_lower": parse_codepoint(fields[3], allow_surrogate=True),
                    "simple_title": parse_codepoint(fields[4], allow_surrogate=True),
                    "full_upper": parse_sequence(fields[5]),
                    "full_lower": parse_sequence(fields[6]),
                    "titlecase": parse_sequence(fields[7]),
                }
            )
            char_previous = code_unit
        else:
            raise ValueError(f"oracle:{line_number}: unknown row type {fields[0]!r}")
    if len(char_rows) != MAX_CODE_UNIT + 1:
        raise ValueError(f"oracle: expected {MAX_CODE_UNIT + 1} CHAR rows, got {len(char_rows)}")
    return scalar_rows, char_rows


def ranges(mapping: Iterable[tuple[int, int]]) -> list[tuple[int, int, int]]:
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


def rust_ranges(name: str, mapping: Iterable[tuple[int, int]]) -> str:
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


def rust_full_table(name: str, rows: Iterable[tuple[int, tuple[int, ...]]]) -> str:
    lines = [f"#[rustfmt::skip]\nstatic {name}: &[(u32, [u32; 3])] = &[\n"]
    for codepoint, mapping in rows:
        values = mapping_array(mapping, codepoint)
        lines.append(
            f"    (0x{codepoint:04X}, [{', '.join(f'0x{value:04X}' for value in values)}]),\n"
        )
    lines.append("];\n")
    return "".join(lines)


def mapping_array(mapping: tuple[int, ...], fallback: int) -> tuple[int, int, int]:
    if len(mapping) > 3:
        raise ValueError(
            "full Unicode mapping cannot be represented by the runtime's "
            f"[u32; 3] layout: U+{fallback:04X} has {len(mapping)} scalars"
        )
    if not mapping:
        mapping = (fallback,)
    values = list(mapping) + [0, 0, 0]
    return values[0], values[1], values[2]


def generate_source(
    scalar_rows: list[tuple[int, int, int]],
    char_rows: list[dict[str, object]],
    reference: dict[str, object],
    oracle_sha256: str,
) -> str:
    simple_upper = [
        (codepoint, uppercase)
        for codepoint, uppercase, _ in scalar_rows
        if uppercase != codepoint
    ]
    simple_lower = [
        (codepoint, lowercase)
        for codepoint, _, lowercase in scalar_rows
        if lowercase != codepoint
    ]
    simple_title = [
        (int(row["code_unit"]), int(row["simple_title"]))
        for row in char_rows
        if int(row["simple_title"]) != int(row["code_unit"])
    ]
    full_upper = [
        (int(row["code_unit"]), tuple(row["full_upper"]))
        for row in char_rows
        if tuple(row["full_upper"]) != (int(row["code_unit"]),)
    ]
    full_lower = [
        (int(row["code_unit"]), tuple(row["full_lower"]))
        for row in char_rows
        if tuple(row["full_lower"]) != (int(row["code_unit"]),)
    ]

    unicode_version = tuple(int(part) for part in str(reference["unicode_version"]).split("."))
    return f'''//! Unicode {reference["unicode_version"]} case mappings from the pinned JVM oracle.
//!
//! GENERATED FILE. The source is the exact Eclipse Temurin
//! {reference["runtime_version"]} public `Character`/`String` API output,
//! captured by `tools/dump_jdk25_unicode_data.java` with
//! `-Djava.locale.providers=CLDR` and deterministic host properties.
//!
//! Simple scalar mappings cover the complete Unicode scalar domain and are
//! used by Kotlin/JVM-compatible case-insensitive prefix comparison. Full
//! UTF-16-Char mappings cover the input domain used by Kotlin
//! `replaceFirstChar(Char::titlecase)`. Identity mappings are implicit.
//!
//! The runtime owns no JVM, filesystem, network, locale database, or mutable
//! global state. This file is regenerated only by the generation-time oracle.

#![allow(dead_code)]

pub const UNICODE_VERSION: (u64, u64, u64) = {unicode_version!r};
pub const REFERENCE_JVM_VERSION: &str = "{reference["java_version"]}";
pub const REFERENCE_JVM_RUNTIME_VERSION: &str = "{reference["runtime_version"]}";
pub const REFERENCE_JVM_VENDOR_VERSION: &str = "{reference["implementor_version"]}";
pub const REFERENCE_JVM_ARCHIVE_SHA256: &str =
    "{reference["archive_sha256"]}";
pub const ORACLE_OUTPUT_SHA256: &str =
    "{oracle_sha256}";
pub const SCALAR_MAPPING_RECORD_COUNT: usize = {len(scalar_rows)};
pub const UTF16_CHAR_RECORD_COUNT: usize = {len(char_rows)};
pub const SIMPLE_TITLECASE_RECORD_COUNT: usize = {len(simple_title)};
pub const FULL_UPPERCASE_RECORD_COUNT: usize = {len(full_upper)};
pub const FULL_LOWERCASE_RECORD_COUNT: usize = {len(full_lower)};

pub(crate) fn simple_uppercase(character: char) -> char {{
    lookup(character, SIMPLE_UPPERCASE_RANGES).unwrap_or(character)
}}

pub(crate) fn simple_lowercase(character: char) -> char {{
    lookup(character, SIMPLE_LOWERCASE_RANGES).unwrap_or(character)
}}

pub(crate) fn simple_titlecase_mapping(character: char) -> Option<char> {{
    lookup(character, SIMPLE_TITLECASE_RANGES)
}}

pub(crate) fn full_uppercase(character: char) -> [u32; 3] {{
    lookup_full(character, FULL_UPPERCASE).unwrap_or([character as u32, 0, 0])
}}

pub(crate) fn full_lowercase(character: char) -> [u32; 3] {{
    lookup_full(character, FULL_LOWERCASE).unwrap_or([character as u32, 0, 0])
}}

fn lookup(character: char, ranges: &[(u32, u32, i32)]) -> Option<char> {{
    let codepoint = character as u32;
    let index = ranges.partition_point(|range| range.0 <= codepoint);
    let range = index.checked_sub(1).and_then(|index| ranges.get(index));
    let &(_start, _end, delta) = range.filter(|range| codepoint <= range.1)?;
    char::from_u32((codepoint as i64 + i64::from(delta)) as u32)
}}

fn lookup_full(character: char, table: &[(u32, [u32; 3])]) -> Option<[u32; 3]> {{
    table
        .binary_search_by_key(&(character as u32), |entry| entry.0)
        .ok()
        .map(|index| table[index].1)
}}

{rust_ranges("SIMPLE_UPPERCASE_RANGES", simple_upper)}
{rust_ranges("SIMPLE_LOWERCASE_RANGES", simple_lower)}
{rust_ranges("SIMPLE_TITLECASE_RANGES", simple_title)}
{rust_full_table("FULL_UPPERCASE", full_upper)}
{rust_full_table("FULL_LOWERCASE", full_lower)}'''


def build_oracle(java: Path, javac: Path) -> tuple[str, str]:
    with tempfile.TemporaryDirectory(prefix="arkst-jdk25-unicode-") as temporary:
        classes = Path(temporary)
        subprocess.run(
            [str(javac), "-d", str(classes), str(HELPER_PATH)],
            check=True,
            capture_output=True,
            text=True,
            env=deterministic_environment(),
        )
        result = subprocess.run(
            [
                str(java),
                "-Djava.locale.providers=CLDR",
                "-Duser.language=en",
                "-Duser.country=US",
                "-Duser.timezone=UTC",
                "-cp",
                str(classes),
                "DumpJdk25UnicodeData",
                "--maps",
            ],
            check=True,
            capture_output=True,
            text=True,
            env=deterministic_environment(),
        )
    return result.stdout, hashlib.sha256(result.stdout.encode("utf-8")).hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--java", type=Path, required=True)
    parser.add_argument("--archive", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    reference = manifest()
    validate_reference(reference)
    validate_archive(args.archive, reference)
    javac = validate_java(args.java, reference)
    helper_sha = sha256(HELPER_PATH)
    expected_helper_sha = str(reference["helper_source_sha256"])
    if helper_sha != expected_helper_sha:
        raise ValueError(f"helper SHA-256 changed: expected {expected_helper_sha}, got {helper_sha}")
    oracle_output, oracle_sha = build_oracle(args.java, javac)
    expected_oracle_sha = reference.get("oracle_output_sha256")
    if expected_oracle_sha is not None and oracle_sha != expected_oracle_sha:
        raise ValueError(f"oracle output SHA-256 changed: expected {expected_oracle_sha}, got {oracle_sha}")
    scalar_rows, char_rows = parse_oracle(oracle_output)
    generated = generate_source(scalar_rows, char_rows, reference, oracle_sha)
    generated_bytes = len(generated.encode("utf-8"))
    if generated_bytes >= MAX_GENERATED_SOURCE_BYTES:
        raise ValueError(
            "generated Rust source exceeds the hard 1 MiB budget: "
            f"{generated_bytes} bytes"
        )
    generated_sha = hashlib.sha256(generated.encode("utf-8")).hexdigest()
    expected_generated_sha = reference.get("generated_source_sha256")
    if expected_generated_sha is not None and generated_sha != expected_generated_sha:
        raise ValueError(
            "generated source SHA-256 changed: "
            f"expected {expected_generated_sha}, got {generated_sha}"
        )
    if args.check:
        actual = args.output.read_text(encoding="utf-8")
        if actual != generated:
            raise SystemExit(f"{args.output} is not deterministic generated output")
    else:
        args.output.write_text(generated, encoding="utf-8")

    print(f"reference_jvm={reference['runtime_version']}")
    print(f"archive_bytes={args.archive.stat().st_size}")
    print(f"archive_sha256={reference['archive_sha256']}")
    print(f"unicode_version={reference['unicode_version']}")
    print(f"oracle_output_sha256={oracle_sha}")
    print(f"scalar_mapping_records={len(scalar_rows)}")
    print(f"utf16_char_records={len(char_rows)}")
    print(f"oracle_records={len(scalar_rows) + len(char_rows)}")
    print(f"simple_titlecase_records={sum(1 for row in char_rows if int(row['simple_title']) != int(row['code_unit']))}")
    print(f"full_uppercase_records={sum(1 for row in char_rows if tuple(row['full_upper']) != (int(row['code_unit']),))}")
    print(f"full_lowercase_records={sum(1 for row in char_rows if tuple(row['full_lower']) != (int(row['code_unit']),))}")
    print(f"generated_source_bytes={generated_bytes}")
    print(f"generated_source_sha256={generated_sha}")


if __name__ == "__main__":
    main()
