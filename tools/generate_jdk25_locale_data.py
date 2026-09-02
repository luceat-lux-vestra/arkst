#!/usr/bin/env python3
"""Generate the deterministic v2.5.1 .doclang locale snapshot.

The pinned Quarkdown implementation delegates locale lookup to
java.util.Locale. This generator captures the complete provider result set
from one exact, reproducible reference runtime. It then removes only records
that resolve to the same value through the shared display fallback order and
writes a compact, little-endian binary index. Runtime compilation never
invokes the JVM; it includes that binary snapshot instead.
"""

from __future__ import annotations

import argparse
import bisect
import csv
import hashlib
import io
import os
import re
import struct
import subprocess
import tarfile
import tempfile
import unicodedata
import zipfile
from collections import Counter
from dataclasses import dataclass
from pathlib import Path

try:
    import tomllib
except ModuleNotFoundError as error:  # pragma: no cover - generator requires Python 3.11+
    raise SystemExit("Python 3.11 or newer is required for the reference manifest") from error


MANIFEST_PATH = Path(__file__).resolve().parents[1] / "docs/compatibility/quarkdown/reference-jvm.toml"


def reference_manifest() -> dict[str, object]:
    with MANIFEST_PATH.open("rb") as stream:
        data = tomllib.load(stream)
    reference = data.get("reference")
    if not isinstance(reference, dict):
        raise ValueError(f"{MANIFEST_PATH}: missing [reference] table")
    return reference


REFERENCE = reference_manifest()


REFERENCE_RELEASE_VERSION = str(REFERENCE["release_version"])
REFERENCE_RUNTIME_VERSION = str(REFERENCE["runtime_version"])
REFERENCE_VENDOR = str(REFERENCE["vendor"])
REFERENCE_VENDOR_VERSION = str(REFERENCE["implementor_version"])
REFERENCE_RUNTIME_DISPLAY = str(REFERENCE["runtime_display"])
REFERENCE_JAVA_VERSION = str(REFERENCE["java_version"])
REFERENCE_UNICODE_VERSION = str(REFERENCE["unicode_version"])
REFERENCE_SOURCE_TAG = str(REFERENCE["source_tag"])
REFERENCE_SOURCE_REVISION = str(REFERENCE["source_revision"])
REFERENCE_BUILD_SOURCE_REVISION = str(REFERENCE["build_source_revision"])
REFERENCE_LOCALE_PROVIDERS = str(REFERENCE["locale_provider"])
REFERENCE_JDK_URL = str(REFERENCE["archive_url"])
REFERENCE_JDK_SHA256 = str(REFERENCE["archive_sha256"])
REFERENCE_JDK_SIZE = int(REFERENCE["archive_bytes"])
EXPECTED_AVAILABLE_RECORD_COUNT = int(REFERENCE["locale_available_record_count"])
EXPECTED_TAG_RECORD_COUNT = int(REFERENCE["locale_canonical_tag_record_count"])
EXPECTED_NAME_COLLISION_COUNT = int(REFERENCE["locale_name_collision_count"])
AVAILABLE_ORDER_MANIFEST = Path(str(REFERENCE["locale_available_order_path"])).name
EXPECTED_AVAILABLE_ORDER_MANIFEST_SHA256 = str(REFERENCE["locale_available_order_manifest_sha256"])
EXPECTED_SOURCE_SHA256 = str(REFERENCE["locale_logical_source_sha256"])
EXPECTED_DUMP_SOURCE_SHA256 = str(REFERENCE["locale_dump_helper_source_sha256"])
EXPECTED_DISPLAY_RECORD_COUNT = int(REFERENCE["locale_logical_display_record_count"])
EXPECTED_DISPLAY_SOURCE_SHA256 = str(REFERENCE["locale_logical_display_source_sha256"])
EXPECTED_DISPLAY_DUMP_SOURCE_SHA256 = str(REFERENCE["locale_display_dump_helper_source_sha256"])
EXPECTED_COMPACT_RECORD_COUNT = int(REFERENCE["locale_compact_record_count"])
EXPECTED_COMPACT_PROFILE_COUNT = int(REFERENCE["locale_compact_profile_count"])
EXPECTED_COMPACT_KEY_COUNT = int(REFERENCE["locale_compact_key_count"])
EXPECTED_COMPACT_VALUE_COUNT = int(REFERENCE["locale_compact_value_count"])
EXPECTED_COMPACT_SHA256 = str(REFERENCE["locale_compact_snapshot_sha256"])

REFERENCE_JDK_TZ_SOURCE_MEMBER = str(REFERENCE["locale_timezone_source_member"])
REFERENCE_JDK_TZ_SOURCE_SHA256 = str(REFERENCE["locale_timezone_source_sha256"])
EXPECTED_TZ_SOURCE_ENTRY_COUNT = int(REFERENCE["locale_timezone_source_rows"])
EXPECTED_TZ_UNIQUE_ENTRY_COUNT = int(REFERENCE["locale_timezone_unique_source_rows"])
EXPECTED_TZ_ID_COUNT = int(REFERENCE["locale_accepted_timezone_ids"])
REFERENCE_CLDR_SOURCE_MEMBER = REFERENCE_JDK_TZ_SOURCE_MEMBER
REFERENCE_CLDR_SOURCE_SHA256 = REFERENCE_JDK_TZ_SOURCE_SHA256
DISPLAY_DUMP_HELPER = Path(str(REFERENCE["locale_display_dump_helper_path"])).name
PUBLIC_ORACLE_HELPER = Path(str(REFERENCE["locale_public_oracle_helper_path"])).name
EXPECTED_PUBLIC_ORACLE_SOURCE_SHA256 = str(REFERENCE["locale_public_oracle_helper_source_sha256"])
EXPECTED_PUBLIC_ORACLE_RECORD_COUNT = int(REFERENCE["locale_public_oracle_record_count"])
EXPECTED_PUBLIC_ORACLE_OUTPUT_SHA256 = str(REFERENCE["locale_public_oracle_output_sha256"])
JDK_EXPORTS = (
    "--add-exports=java.base/sun.util.resources=ALL-UNNAMED",
    "--add-exports=java.base/sun.util.locale.provider=ALL-UNNAMED",
)

LocaleRow = tuple[str, str, str, str, str, str, str, str]

# Executable representation budgets. These are checked during every
# regeneration, and the Rust integrity tests repeat the checked-in limits.
COMPACT_FORMAT_VERSION = int(REFERENCE["locale_compact_format_version"])
MAX_GENERATED_RUST_SOURCE_BYTES = 1 * 1024 * 1024
MAX_GENERATED_RUST_SOURCE_LINES = 100_000
MAX_COMPACT_SNAPSHOT_BYTES = 8 * 1024 * 1024

DISPLAY_MAGIC = b"SCLD"
DISPLAY_HEADER_FORMAT = "<4s17I"
DISPLAY_HEADER_SIZE = struct.calcsize(DISPLAY_HEADER_FORMAT)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def verify_reference_archive(archive: Path, java: Path) -> None:
    if archive.stat().st_size != REFERENCE_JDK_SIZE:
        raise ValueError(
            "reference JDK archive size mismatch: "
            f"expected {REFERENCE_JDK_SIZE}, got {archive.stat().st_size}"
        )
    archive_sha256 = sha256_file(archive)
    if archive_sha256 != REFERENCE_JDK_SHA256:
        raise ValueError(
            "reference JDK archive fingerprint mismatch: "
            f"expected {REFERENCE_JDK_SHA256}, got {archive_sha256}"
        )

    with tarfile.open(archive, mode="r:gz") as tar:
        archived_executables: dict[str, bytes] = {}
        for executable in ("java", "javac"):
            suffix = f"/bin/{executable}"
            members = [member for member in tar.getmembers() if member.name.endswith(suffix)]
            if len(members) != 1:
                raise ValueError(
                    "reference JDK archive must contain exactly one "
                    f"bin/{executable}, found {len(members)}"
                )
            payload = tar.extractfile(members[0])
            if payload is None:
                raise ValueError(
                    f"reference JDK {executable} archive member is not a regular file"
                )
            archived_executables[executable] = payload.read()

    supplied_java = java.read_bytes()
    if supplied_java != archived_executables["java"]:
        raise ValueError(
            "supplied --java is not the Contents/Home/bin/java from the "
            "pinned reference archive"
        )
    javac = java.with_name("javac")
    if not javac.is_file() or javac.read_bytes() != archived_executables["javac"]:
        raise ValueError(
            "the sibling javac is not the Contents/Home/bin/javac from the "
            "pinned reference archive"
        )


def reference_timezone_ids(archive: Path) -> list[str]:
    source = reference_cldr_source(archive)
    source_sha256 = sha256(source)
    if REFERENCE_JDK_TZ_SOURCE_SHA256 and source_sha256 != REFERENCE_JDK_TZ_SOURCE_SHA256:
        raise ValueError(
            "reference JDK timezone source fingerprint mismatch: "
            f"expected {REFERENCE_JDK_TZ_SOURCE_SHA256}, got {source_sha256}"
        )

    matches = re.findall(rb'tzCanonicalIDMap\.put\("([^"]+)",', source)
    if EXPECTED_TZ_SOURCE_ENTRY_COUNT and len(matches) != EXPECTED_TZ_SOURCE_ENTRY_COUNT:
        raise ValueError(
            "reference JDK timezone source entry count mismatch: "
            f"expected {EXPECTED_TZ_SOURCE_ENTRY_COUNT}, got {len(matches)}"
        )
    if EXPECTED_TZ_UNIQUE_ENTRY_COUNT and len(set(matches)) != EXPECTED_TZ_UNIQUE_ENTRY_COUNT:
        raise ValueError(
            "reference JDK timezone source unique-entry count mismatch: "
            f"expected {EXPECTED_TZ_UNIQUE_ENTRY_COUNT}, got {len(set(matches))}"
        )
    timezone_ids = sorted(
        {
            value.decode("ascii")
            for value in matches
            if value.isascii()
            and value.isalnum()
            and value == value.lower()
            and 3 <= len(value) <= 8
        }
    )
    if EXPECTED_TZ_ID_COUNT and len(timezone_ids) != EXPECTED_TZ_ID_COUNT:
        raise ValueError(
            "reference JDK Unicode timezone-id candidate count mismatch: "
            f"expected {EXPECTED_TZ_ID_COUNT}, got {len(timezone_ids)}"
        )
    return timezone_ids


def reference_cldr_source(archive: Path) -> bytes:
    with tarfile.open(archive, mode="r:gz") as tar:
        src_zip_members = [
            member
            for member in tar.getmembers()
            if member.name.endswith("/lib/src.zip")
        ]
        if len(src_zip_members) != 1:
            raise ValueError(
                "reference JDK archive must contain exactly one lib/src.zip, "
                f"found {len(src_zip_members)}"
            )
        src_zip = tar.extractfile(src_zip_members[0])
        if src_zip is None:
            raise ValueError("reference JDK src.zip archive member is not a regular file")
        src_zip_bytes = src_zip.read()

    with zipfile.ZipFile(io.BytesIO(src_zip_bytes)) as source_archive:
        source = source_archive.read(REFERENCE_CLDR_SOURCE_MEMBER)
    source_sha256 = sha256(source)
    if REFERENCE_CLDR_SOURCE_SHA256 and source_sha256 != REFERENCE_CLDR_SOURCE_SHA256:
        raise ValueError(
            "reference CLDR routing source fingerprint mismatch: "
            f"expected {REFERENCE_CLDR_SOURCE_SHA256}, got {source_sha256}"
        )
    return source


def reference_cldr_routing(archive: Path) -> tuple[list[tuple[str, str]], list[tuple[str, str]], list[tuple[str, str]]]:
    source = reference_cldr_source(archive).decode("utf-8")
    parent_locales: list[tuple[str, str]] = []
    for parent, body in re.findall(
        r'parentLocalesMap\.put\(Locale\.forLanguageTag\("([^"]+)"\),'
        r"\s*new String\[\] \{(.*?)\}\);",
        source,
        re.DOTALL,
    ):
        parent_locales.extend((child, parent) for child in re.findall(r'"([^"]+)"', body))

    likely_scripts: dict[str, str] = {}
    for script, body in re.findall(
        r'likelyScriptMap\.put\("([^"]+)", "([^"]*)"\);', source
    ):
        for language in body.split():
            previous = likely_scripts.setdefault(language, script)
            if previous != script:
                raise ValueError(
                    f"reference CLDR likely-script map has conflicting entries for {language}"
                )

    language_aliases = re.findall(
        r'languageAliasMap\.put\("([^"]+)", "([^"]+)"\);', source
    )
    return (
        sorted(set(parent_locales)),
        sorted(likely_scripts.items()),
        sorted(set(language_aliases)),
    )


def run_reference_java(java: Path, helper: Path) -> bytes:
    helper_sha256 = sha256(helper.read_bytes())
    if EXPECTED_DUMP_SOURCE_SHA256 and helper_sha256 != EXPECTED_DUMP_SOURCE_SHA256:
        raise ValueError(
            "reference locale dump helper fingerprint mismatch: "
            f"expected {EXPECTED_DUMP_SOURCE_SHA256}, got {helper_sha256}"
        )
    completed = subprocess.run(
        [
            str(java),
            "-Dfile.encoding=UTF-8",
            f"-Djava.locale.providers={REFERENCE_LOCALE_PROVIDERS}",
            "--source",
            "25",
            str(helper),
        ],
        check=False,
        capture_output=True,
        env=reference_environment(),
    )
    if completed.returncode != 0:
        raise RuntimeError(
            "reference JDK locale dump failed:\n"
            + completed.stderr.decode("utf-8", errors="replace")
        )
    return completed.stdout


def run_reference_public_oracle(java: Path, helper: Path) -> bytes:
    helper_sha256 = sha256(helper.read_bytes())
    if helper_sha256 != EXPECTED_PUBLIC_ORACLE_SOURCE_SHA256:
        raise ValueError(
            "reference public locale oracle helper fingerprint mismatch: "
            f"expected {EXPECTED_PUBLIC_ORACLE_SOURCE_SHA256}, got {helper_sha256}"
        )
    completed = subprocess.run(
        [
            str(java),
            "-Dfile.encoding=UTF-8",
            f"-Djava.locale.providers={REFERENCE_LOCALE_PROVIDERS}",
            "--source",
            "25",
            str(helper),
        ],
        check=False,
        capture_output=True,
        env=reference_environment(),
    )
    if completed.returncode != 0:
        raise RuntimeError(
            "reference JDK public locale oracle failed:\n"
            + completed.stderr.decode("utf-8", errors="replace")
        )
    return completed.stdout


def validate_public_oracle(raw: bytes) -> str:
    output_sha256 = sha256(raw)
    if EXPECTED_PUBLIC_ORACLE_OUTPUT_SHA256 and output_sha256 != EXPECTED_PUBLIC_ORACLE_OUTPUT_SHA256:
        raise ValueError(
            "reference public locale oracle output fingerprint mismatch: "
            f"expected {EXPECTED_PUBLIC_ORACLE_OUTPUT_SHA256}, got {output_sha256}"
        )
    rows = raw.decode("utf-8").splitlines()
    if len(rows) != EXPECTED_PUBLIC_ORACLE_RECORD_COUNT:
        raise ValueError(
            "reference public locale oracle record count mismatch: "
            f"expected {EXPECTED_PUBLIC_ORACLE_RECORD_COUNT}, got {len(rows)}"
        )
    for line_number, line in enumerate(rows, 1):
        fields = line.split("\t")
        if len(fields) != 11 or fields[0] != "locale":
            raise ValueError(f"public locale oracle row {line_number}: malformed row")
    return output_sha256


def run_reference_display_java(
    java: Path, helper: Path, timezone_ids: list[str]
) -> bytes:
    helper_sha256 = sha256(helper.read_bytes())
    if EXPECTED_DISPLAY_DUMP_SOURCE_SHA256 and helper_sha256 != EXPECTED_DISPLAY_DUMP_SOURCE_SHA256:
        raise ValueError(
            "reference locale display dump helper fingerprint mismatch: "
            f"expected {EXPECTED_DISPLAY_DUMP_SOURCE_SHA256}, got {helper_sha256}"
        )
    javac = java.with_name("javac")
    with tempfile.TemporaryDirectory(prefix="arkst-jdk25-locale-display-") as output:
        compiled = subprocess.run(
            [str(javac), *JDK_EXPORTS, "-d", output, str(helper)],
            check=False,
            capture_output=True,
            env=reference_environment(),
        )
        if compiled.returncode != 0:
            raise RuntimeError(
                "reference JDK locale display helper compilation failed:\n"
                + compiled.stderr.decode("utf-8", errors="replace")
            )
        completed = subprocess.run(
            [
                str(java),
                *JDK_EXPORTS,
                "-Dfile.encoding=UTF-8",
                f"-Djava.locale.providers={REFERENCE_LOCALE_PROVIDERS}",
                "-cp",
                output,
                "DumpJdk25LocaleDisplayData",
                "--timezone",
                *timezone_ids,
            ],
            check=False,
            capture_output=True,
            env=reference_environment(),
        )
    if completed.returncode != 0:
        raise RuntimeError(
            "reference JDK locale display dump failed:\n"
            + completed.stderr.decode("utf-8", errors="replace")
        )
    return completed.stdout


def reference_environment() -> dict[str, str]:
    environment = os.environ.copy()
    environment["LANG"] = "C.UTF-8"
    environment["LC_ALL"] = "C.UTF-8"
    return environment


def parse_dump(
    raw: bytes,
) -> tuple[
    list[LocaleRow],
    list[LocaleRow],
    list[tuple[str, list[str]]],
    str,
]:
    lines = raw.decode("utf-8").splitlines()
    metadata: dict[str, str] = {}
    available: list[LocaleRow] = []
    tags: list[LocaleRow] = []
    collisions: list[tuple[str, list[str]]] = []
    for line_number, line in enumerate(lines, 1):
        fields = line.split("\t")
        if fields[0] in {"available", "tag"}:
            if len(fields) != 9 or not fields[1]:
                raise ValueError(f"dump line {line_number}: malformed locale {fields[0]}")
            (available if fields[0] == "available" else tags).append(
                tuple(fields[1:9])
            )
        elif fields[0] == "collision":
            if len(fields) < 4 or any(not field for field in fields[2:]):
                raise ValueError(f"dump line {line_number}: malformed name collision")
            collisions.append((fields[1], fields[2:]))
        else:
            if len(fields) != 2 or fields[0] in metadata:
                raise ValueError(f"dump line {line_number}: malformed metadata")
            metadata[fields[0]] = fields[1]

    expected_metadata = {
        "runtime.version": REFERENCE_RUNTIME_VERSION,
        "java.vendor": REFERENCE_VENDOR,
        "java.vendor.version": REFERENCE_VENDOR_VERSION,
        "java.locale.providers": REFERENCE_LOCALE_PROVIDERS,
        "name-collision-count": str(EXPECTED_NAME_COLLISION_COUNT),
    }
    if metadata != expected_metadata:
        raise ValueError(
            "reference runtime metadata mismatch: "
            f"expected {expected_metadata!r}, got {metadata!r}"
        )
    if EXPECTED_AVAILABLE_RECORD_COUNT and len(available) != EXPECTED_AVAILABLE_RECORD_COUNT:
        raise ValueError(
            "reference available-locale record count mismatch: "
            f"expected {EXPECTED_AVAILABLE_RECORD_COUNT}, got {len(available)}"
        )
    if EXPECTED_TAG_RECORD_COUNT and len(tags) != EXPECTED_TAG_RECORD_COUNT:
        raise ValueError(
            "reference canonical-tag record count mismatch: "
            f"expected {EXPECTED_TAG_RECORD_COUNT}, got {len(tags)}"
        )
    if len(collisions) != EXPECTED_NAME_COLLISION_COUNT:
        raise ValueError(
            "reference name collision count mismatch: "
            f"expected {EXPECTED_NAME_COLLISION_COUNT}, got {len(collisions)}"
        )
    available = apply_pinned_available_order(available)
    duplicate_available_tags = {
        tag: count
        for tag, count in Counter(row[0] for row in available).items()
        if count > 1
    }
    print(f"duplicate_available_tags={duplicate_available_tags!r}")

    source_bytes = "".join(
        f"{kind}\t{tag}\t{display_name}\t{localized_name}\t{code}\t{script}\t{country}\t{variant}\t{localized_country}\n"
        for kind, rows in (("available", available), ("tag", tags))
        for tag, display_name, localized_name, code, script, country, variant, localized_country in rows
    ).encode("utf-8")
    source_sha256 = sha256(source_bytes)
    if EXPECTED_SOURCE_SHA256 and source_sha256 != EXPECTED_SOURCE_SHA256:
        raise ValueError(
            "reference locale source fingerprint mismatch: "
            f"expected {EXPECTED_SOURCE_SHA256}, got {source_sha256}"
        )
    return available, tags, collisions, source_sha256


def apply_pinned_available_order(
    available: list[LocaleRow],
) -> list[LocaleRow]:
    # The Java helper emits Locale.getAvailableLocales() without sorting. The
    # exact JDK provider union uses hash-based assembly whose unspecified raw
    # order is not stable across JVM starts, so this manifest is a captured
    # raw-array result from the pinned archive. We validate the current JVM's
    # complete row set before reusing that captured order; no name records are
    # sorted or otherwise approximated here.
    manifest_path = Path(__file__).with_name(AVAILABLE_ORDER_MANIFEST)
    manifest_bytes = manifest_path.read_bytes()
    manifest_sha256 = sha256(manifest_bytes)
    if manifest_sha256 != EXPECTED_AVAILABLE_ORDER_MANIFEST_SHA256:
        raise ValueError(
            "pinned available-locale order manifest fingerprint mismatch: "
            f"expected {EXPECTED_AVAILABLE_ORDER_MANIFEST_SHA256}, got {manifest_sha256}"
        )
    manifest = [
        tuple(row)
        for row in csv.reader(
            manifest_bytes.decode("utf-8").splitlines(), delimiter="\t"
        )
    ]
    if any(len(row) != 3 for row in manifest):
        raise ValueError("pinned available-locale order manifest contains a malformed row")
    if Counter(manifest) != Counter(row[:3] for row in available):
        raise ValueError(
            "pinned available-locale order manifest does not contain the exact "
            "reference locale rows"
        )
    remaining = list(available)
    ordered: list[LocaleRow] = []
    for key in manifest:
        for index, row in enumerate(remaining):
            if row[:3] == key:
                ordered.append(remaining.pop(index))
                break
        else:
            raise ValueError("pinned available-locale order manifest row was not found")
    return ordered


def available_order_manifest_bytes(available: list[LocaleRow]) -> bytes:
    output = io.StringIO(newline="")
    writer = csv.writer(
        output, delimiter="\t", lineterminator="\n", quoting=csv.QUOTE_ALL
    )
    writer.writerows(row[:3] for row in available)
    return output.getvalue().encode("utf-8")


def rust_string(value: str) -> str:
    escaped = ['"']
    for character in value:
        if character == "\\":
            escaped.append("\\\\")
        elif character == '"':
            escaped.append('\\"')
        elif unicodedata.category(character) in {"Cc", "Cf"}:
            escaped.append(f"\\u{{{ord(character):x}}}")
        else:
            escaped.append(character)
    escaped.append('"')
    return "".join(escaped)


def parse_display_dump(raw: bytes) -> tuple[list[tuple[str, str, str]], str]:
    lines = raw.decode("utf-8").splitlines()
    display: list[tuple[str, str, str]] = []
    seen: set[tuple[str, str]] = set()
    for line_number, line in enumerate(lines, 1):
        fields = line.split("\t")
        if len(fields) != 4 or fields[0] != "display" or not fields[2] or not fields[3]:
            raise ValueError(f"display dump line {line_number}: malformed locale display data")
        profile, key, value = fields[1:]
        identity = (profile, key)
        if identity in seen:
            raise ValueError(f"duplicate locale display data: {identity!r}")
        seen.add(identity)
        display.append((profile, key, value))

    if display != sorted(display, key=lambda row: (row[0], row[1])):
        raise ValueError("locale display data is not deterministically sorted")

    source_bytes = "".join(
        f"display\t{profile}\t{key}\t{value}\n" for profile, key, value in display
    ).encode("utf-8")
    source_sha256 = sha256(source_bytes)
    if EXPECTED_DISPLAY_SOURCE_SHA256 and source_sha256 != EXPECTED_DISPLAY_SOURCE_SHA256:
        raise ValueError(
            "reference locale display source fingerprint mismatch: "
            f"expected {EXPECTED_DISPLAY_SOURCE_SHA256}, got {source_sha256}"
        )
    return display, source_sha256


def is_ascii_alpha(value: str) -> bool:
    return bool(value) and value.isascii() and value.isalpha()


def is_variant_subtag(value: str) -> bool:
    return (
        5 <= len(value) <= 8 and value.isascii() and value.isalnum()
    ) or (
        len(value) == 4
        and value[0].isdigit()
        and value.isascii()
        and value.isalnum()
    )


def parse_profile(profile: str) -> tuple[str, str, str, list[str]]:
    if not profile:
        return "", "", "", []
    parts = profile.split("-")
    language = parts[0].lower()
    index = 1
    script = ""
    if index < len(parts) and len(parts[index]) == 4 and is_ascii_alpha(parts[index]):
        script = parts[index][0].upper() + parts[index][1:].lower()
        index += 1
    region = ""
    if index < len(parts) and (
        (len(parts[index]) == 2 and is_ascii_alpha(parts[index]))
        or (len(parts[index]) == 3 and parts[index].isascii() and parts[index].isdigit())
    ):
        region = parts[index].upper()
        index += 1
    variants: list[str] = []
    while index < len(parts) and is_variant_subtag(parts[index]):
        variants.append(parts[index])
        index += 1
    return language, script, region, variants


def candidate_base_locale_tag(
    language: str, script: str, region: str, variants: list[str]
) -> str:
    if not language and not script and not region and not variants:
        return ""
    parts: list[str] = []
    if language:
        parts.append(language.lower())
    if script:
        parts.append(script[0].upper() + script[1:].lower())
    if region:
        parts.append(region.upper())
    valid_count = 0
    for variant in variants:
        if not is_variant_subtag(variant):
            break
        parts.append(variant)
        valid_count += 1
    if valid_count < len(variants):
        parts.extend(["x", "lvariant", *variants[valid_count:]])
    return "-".join(parts)


def default_candidates(
    language: str, script: str, region: str, variants: list[str]
) -> list[tuple[str, str, str, list[str]]]:
    prefixes = [variants[:count] for count in range(len(variants), 0, -1)]
    result: list[tuple[str, str, str, list[str]]] = []
    result.extend((language, script, region, prefix) for prefix in prefixes)
    if region:
        result.append((language, script, region, []))
    restart_region = region
    if script:
        result.append((language, script, "", []))
        if language == "zh" and not restart_region:
            if script == "Hans":
                restart_region = "CN"
            elif script == "Hant":
                restart_region = "TW"
        result.extend((language, "", restart_region, prefix) for prefix in prefixes)
        if restart_region:
            result.append((language, "", restart_region, []))
    if language:
        result.append((language, "", "", []))
    result.append(("", "", "", []))
    return result


def candidate_profiles(profile: str) -> list[str]:
    language, script, region, variants = parse_profile(profile)
    is_bokmal = False
    is_nynorsk = False
    if language == "no":
        if region == "NO" and variants == ["NY"]:
            variants = []
            is_nynorsk = True
        else:
            is_bokmal = True

    candidates: list[tuple[str, str, str, list[str]]]
    if language == "nb" or is_bokmal:
        base = default_candidates("nb", script, region, variants)
        candidates = []
        for candidate in base:
            cand_language, cand_script, cand_region, cand_variants = candidate
            if not cand_language:
                candidates.append(candidate)
                break
            other = ("no", cand_script, cand_region, cand_variants)
            if is_bokmal:
                candidates.extend([other, candidate])
            else:
                candidates.extend([candidate, other])
    elif language == "nn" or is_nynorsk:
        candidates = default_candidates("nn", script, region, variants)
        root_index = len(candidates) - 1
        candidates[root_index:root_index] = [
            ("no", "", "NO", ["NY"]),
            ("no", "", "NO", []),
            ("no", "", "", []),
        ]
    else:
        if language == "zh" and not script and region:
            if region in {"TW", "HK", "MO"}:
                script = "Hant"
            elif region in {"CN", "SG"}:
                script = "Hans"
        candidates = default_candidates(language, script, region, variants)

    result: list[str] = []
    for cand_language, cand_script, cand_region, cand_variants in candidates:
        tag = candidate_base_locale_tag(cand_language, cand_script, cand_region, cand_variants)
        if tag not in result:
            result.append(tag)
    return result


def fallback_profiles(profile: str) -> list[str]:
    candidates = candidate_profiles(profile)
    return candidates[1:] if candidates and candidates[0] == profile else candidates


def pool_section(strings: list[str]) -> tuple[bytes, int]:
    offsets = [0]
    encoded = bytearray()
    for value in strings:
        encoded.extend(value.encode("utf-8"))
        offsets.append(len(encoded))
    section = b"".join(struct.pack("<I", offset) for offset in offsets) + bytes(encoded)
    return section, len(encoded)


@dataclass(frozen=True)
class CompactModel:
    profiles: list[str]
    keys: list[str]
    values: list[str]
    ranges: list[int]
    fallback_ranges: list[int]
    fallback_ids: list[int]
    records: list[tuple[int, int]]

    def lookup(self, profile: str, key: str) -> str | None:
        profile_id = bisect.bisect_left(self.profiles, profile)
        if profile_id == len(self.profiles) or self.profiles[profile_id] != profile:
            return None
        key_id = bisect.bisect_left(self.keys, key)
        if key_id == len(self.keys) or self.keys[key_id] != key:
            return None
        start, end = self.ranges[profile_id : profile_id + 2]
        record_keys = [record[0] for record in self.records[start:end]]
        record_id = bisect.bisect_left(record_keys, key_id)
        if record_id == len(record_keys) or record_keys[record_id] != key_id:
            return None
        return self.values[self.records[start + record_id][1]]

    def resolve_profile(self, profile_id: int, key: str) -> str | None:
        if not 0 <= profile_id < len(self.profiles):
            return None
        return self._resolve_profile(profile_id, key, set())

    def _resolve_profile(
        self, profile_id: int, key: str, visiting: set[int]
    ) -> str | None:
        if profile_id in visiting:
            return None
        visiting.add(profile_id)
        profile = self.profiles[profile_id]
        value = self.lookup(profile, key)
        if value is not None:
            return value
        start, end = self.fallback_ranges[profile_id : profile_id + 2]
        for fallback_id in self.fallback_ids[start:end]:
            value = self._resolve_profile(fallback_id, key, visiting)
            if value is not None:
                visiting.remove(profile_id)
                return value
        visiting.remove(profile_id)
        return None

    def resolve(self, profile: str, key: str) -> str | None:
        profile_id = bisect.bisect_left(self.profiles, profile)
        if profile_id == len(self.profiles) or self.profiles[profile_id] != profile:
            return None
        return self.resolve_profile(profile_id, key)


def fallback_graph(
    profiles: list[str], profile_ids: dict[str, int]
) -> tuple[list[int], list[int]]:
    """Build the one fallback graph shared by compaction and the blob runtime.

    The graph is serialized into the snapshot. The compacting proof and the
    Rust resolver both consume these exact edges, so they cannot drift into
    separate fallback approximations.
    """
    fallback_ids: list[int] = []
    fallback_ranges = [0]
    for profile in profiles:
        seen: set[int] = set()
        for candidate in fallback_profiles(profile):
            profile_id = profile_ids.get(candidate)
            if profile_id is not None and profile_id not in seen:
                fallback_ids.append(profile_id)
                seen.add(profile_id)
        fallback_ranges.append(len(fallback_ids))
    return fallback_ranges, fallback_ids


def compact_display(
    display: list[tuple[str, str, str]],
) -> tuple[bytes, CompactModel, dict[str, int]]:
    profiles = sorted({profile for profile, _key, _value in display} | {""})
    keys = sorted({key for _profile, key, _value in display})
    profile_ids = {profile: index for index, profile in enumerate(profiles)}
    key_ids = {key: index for index, key in enumerate(keys)}
    fallback_ranges, fallback_ids = fallback_graph(profiles, profile_ids)
    # Resolve each oracle row through the same serialized fallback graph that
    # the final runtime will use. This is the semantic-delta decision point.
    all_values = sorted({value for _profile, _key, value in display})
    all_value_ids = {value: index for index, value in enumerate(all_values)}
    rows_by_profile: dict[str, list[tuple[int, int]]] = {profile: [] for profile in profiles}
    for profile, key, value in display:
        rows_by_profile[profile].append((key_ids[key], all_value_ids[value]))
    full_ranges = [0]
    full_records: list[tuple[int, int]] = []
    for profile in profiles:
        full_records.extend(sorted(rows_by_profile[profile], key=lambda row: row[0]))
        full_ranges.append(len(full_records))
    full_model = CompactModel(
        profiles,
        keys,
        all_values,
        full_ranges,
        fallback_ranges,
        fallback_ids,
        full_records,
    )
    retained: list[tuple[str, str, str]] = []
    for profile, key, value in display:
        profile_id = profile_ids[profile]
        fallback_value = None
        for fallback_id in fallback_ids[fallback_ranges[profile_id] : fallback_ranges[profile_id + 1]]:
            fallback_value = full_model._resolve_profile(fallback_id, key, {profile_id})
            if fallback_value is not None:
                break
        if fallback_value != value:
            retained.append((profile, key, value))

    values = sorted({value for _profile, _key, value in retained})
    value_ids = {value: index for index, value in enumerate(values)}
    numeric_records = [
        (key_ids[key], value_ids[value]) for profile, key, value in retained
    ]
    ranges = [0]
    retained_index = 0
    for profile in profiles:
        while retained_index < len(retained) and retained[retained_index][0] == profile:
            retained_index += 1
        ranges.append(retained_index)

    profile_section, raw_profile_bytes = pool_section(profiles)
    key_section, raw_key_bytes = pool_section(keys)
    value_section, raw_value_bytes = pool_section(values)
    ranges_section = b"".join(struct.pack("<I", value) for value in ranges)
    fallback_section = b"".join(
        struct.pack("<I", value) for value in [*fallback_ranges, *fallback_ids]
    )
    records_section = b"".join(
        struct.pack("<II", key_id, value_id) for key_id, value_id in numeric_records
    )
    profile_offset = DISPLAY_HEADER_SIZE
    key_offset = profile_offset + len(profile_section)
    value_offset = key_offset + len(key_section)
    ranges_offset = value_offset + len(value_section)
    fallback_offset = ranges_offset + len(ranges_section)
    records_offset = fallback_offset + len(fallback_section)
    header = struct.pack(
        DISPLAY_HEADER_FORMAT,
        DISPLAY_MAGIC,
        COMPACT_FORMAT_VERSION,
        len(profiles),
        len(keys),
        len(values),
        len(numeric_records),
        profile_offset,
        len(profile_section),
        key_offset,
        len(key_section),
        value_offset,
        len(value_section),
        ranges_offset,
        len(ranges_section),
        fallback_offset,
        len(fallback_section),
        records_offset,
        len(records_section),
    )
    binary = (
        header
        + profile_section
        + key_section
        + value_section
        + ranges_section
        + fallback_section
        + records_section
    )
    model = CompactModel(
        profiles, keys, values, ranges, fallback_ranges, fallback_ids, numeric_records
    )
    stats = {
        "oracle_records": len(display),
        "compact_records": len(numeric_records),
        "profile_count": len(profiles),
        "key_count": len(keys),
        "unique_value_count": len(values),
        "raw_string_pool_bytes": raw_profile_bytes + raw_key_bytes + raw_value_bytes,
        "numeric_index_bytes": len(ranges_section)
        + len(fallback_section)
        + len(records_section),
        "compact_snapshot_bytes": len(binary),
    }
    return binary, model, stats


def read_u32(data: bytes, offset: int) -> int:
    end = offset + 4
    if offset < 0 or end > len(data):
        raise ValueError("binary snapshot integer is out of bounds")
    return struct.unpack_from("<I", data, offset)[0]


def decode_pool(data: bytes, offset: int, length: int, count: int) -> list[str]:
    offset_end = offset + (count + 1) * 4
    section_end = offset + length
    if offset < DISPLAY_HEADER_SIZE or offset_end > section_end or section_end > len(data):
        raise ValueError("binary snapshot string pool is out of bounds")
    offsets = [read_u32(data, offset + index * 4) for index in range(count + 1)]
    bytes_start = offset_end
    bytes_length = section_end - bytes_start
    if offsets[-1] > bytes_length:
        raise ValueError("binary snapshot string offset is out of bounds")
    strings = []
    for start, end in zip(offsets, offsets[1:]):
        if start > end or end > bytes_length:
            raise ValueError("binary snapshot string offsets are not monotonic")
        strings.append(data[bytes_start + start : bytes_start + end].decode("utf-8"))
    return strings


def decode_binary(data: bytes) -> CompactModel:
    if len(data) < DISPLAY_HEADER_SIZE:
        raise ValueError("binary snapshot header is truncated")
    fields = struct.unpack_from(DISPLAY_HEADER_FORMAT, data)
    magic, version, profile_count, key_count, value_count, record_count, *sections = fields
    if magic != DISPLAY_MAGIC or version != COMPACT_FORMAT_VERSION:
        raise ValueError("binary snapshot magic/version mismatch")
    (
        profile_offset,
        profile_length,
        key_offset,
        key_length,
        value_offset,
        value_length,
        ranges_offset,
        ranges_length,
        fallback_offset,
        fallback_length,
        records_offset,
        records_length,
    ) = sections
    bounds = [
        (profile_offset, profile_length),
        (key_offset, key_length),
        (value_offset, value_length),
        (ranges_offset, ranges_length),
        (fallback_offset, fallback_length),
        (records_offset, records_length),
    ]
    for offset, length in bounds:
        if offset < DISPLAY_HEADER_SIZE or offset + length > len(data):
            raise ValueError("binary snapshot section is out of bounds")
    if ranges_length != (profile_count + 1) * 4 or records_length != record_count * 8:
        raise ValueError("binary snapshot numeric section length mismatch")
    for index, (left_offset, left_length) in enumerate(bounds):
        left_end = left_offset + left_length
        for right_offset, right_length in bounds[index + 1 :]:
            if left_offset < right_offset + right_length and right_offset < left_end:
                raise ValueError("binary snapshot sections overlap")
    profiles = decode_pool(data, profile_offset, profile_length, profile_count)
    keys = decode_pool(data, key_offset, key_length, key_count)
    values = decode_pool(data, value_offset, value_length, value_count)
    ranges = [
        read_u32(data, ranges_offset + index * 4) for index in range(profile_count + 1)
    ]
    fallback_count_offset = fallback_offset + (profile_count + 1) * 4
    if fallback_count_offset > fallback_offset + fallback_length:
        raise ValueError("binary snapshot fallback section is truncated")
    fallback_id_bytes = fallback_offset + fallback_length - fallback_count_offset
    if fallback_id_bytes % 4:
        raise ValueError("binary snapshot fallback section length mismatch")
    fallback_ranges = [
        read_u32(data, fallback_offset + index * 4) for index in range(profile_count + 1)
    ]
    fallback_ids = [
        read_u32(data, fallback_count_offset + index * 4)
        for index in range(fallback_id_bytes // 4)
    ]
    records = [
        (
            read_u32(data, records_offset + index * 8),
            read_u32(data, records_offset + index * 8 + 4),
        )
        for index in range(record_count)
    ]
    if profiles != sorted(set(profiles)) or keys != sorted(set(keys)):
        raise ValueError("binary snapshot dictionaries are not sorted and unique")
    if values != sorted(set(values)):
        raise ValueError("binary snapshot values are not sorted and unique")
    if ranges != sorted(ranges) or ranges[-1] != record_count:
        raise ValueError("binary snapshot profile ranges are invalid")
    if fallback_ranges != sorted(fallback_ranges) or fallback_ranges[-1] != len(fallback_ids):
        raise ValueError("binary snapshot fallback ranges are invalid")
    if any(fallback_id >= profile_count for fallback_id in fallback_ids):
        raise ValueError("binary snapshot fallback ID is out of bounds")
    for start, end in zip(ranges, ranges[1:]):
        profile_records = records[start:end]
        if any(
            key_id >= key_count or value_id >= value_count
            for key_id, value_id in profile_records
        ):
            raise ValueError("binary snapshot record ID is out of bounds")
        key_ids = [key_id for key_id, _value_id in profile_records]
        if key_ids != sorted(set(key_ids)):
            raise ValueError("binary snapshot records are not strictly sorted")
    return CompactModel(
        profiles, keys, values, ranges, fallback_ranges, fallback_ids, records
    )


def prove_reconstruction(
    display: list[tuple[str, str, str]], model: CompactModel, binary: bytes
) -> None:
    decoded = decode_binary(binary)
    for profile, key, expected in display:
        actual = decoded.resolve(profile, key)
        if actual != expected:
            raise ValueError(
                "compact locale reconstruction mismatch: "
                f"({profile!r}, {key!r}) expected {expected!r}, got {actual!r}"
            )
    if decoded != model:
        raise ValueError("binary snapshot differs from the compact model")


def enforce_budgets(source: str, binary: bytes, stats: dict[str, int]) -> None:
    source_bytes = len(source.encode("utf-8"))
    source_lines = source.count("\n")
    stats["generated_rust_source_bytes"] = source_bytes
    stats["generated_rust_source_lines"] = source_lines
    if source_bytes >= MAX_GENERATED_RUST_SOURCE_BYTES:
        raise ValueError(
            "generated Rust locale source exceeds budget: "
            f"{source_bytes} >= {MAX_GENERATED_RUST_SOURCE_BYTES}"
        )
    if source_lines >= MAX_GENERATED_RUST_SOURCE_LINES:
        raise ValueError(
            "generated Rust locale source line count exceeds budget: "
            f"{source_lines} >= {MAX_GENERATED_RUST_SOURCE_LINES}"
        )
    if len(binary) > MAX_COMPACT_SNAPSHOT_BYTES:
        raise ValueError(
            "compact locale snapshot exceeds budget: "
            f"{len(binary)} > {MAX_COMPACT_SNAPSHOT_BYTES}"
        )


def render_rust_source(
    available: list[LocaleRow],
    tags: list[LocaleRow],
    collisions: list[tuple[str, list[str]]],
    source_sha256: str,
    display_source_sha256: str,
    display_stats: dict[str, int],
    compact_sha256: str,
    cldr_parent_locales: list[tuple[str, str]],
    cldr_likely_scripts: list[tuple[str, str]],
    cldr_language_aliases: list[tuple[str, str]],
) -> str:
    lines = [
        "// Generated by `tools/generate_jdk25_locale_data.py`.",
        "//",
        "// Locale names are from the complete pinned JDK oracle; effective",
        "// CLDR display data",
        "// is stored in `data/jdk25_locale_display.bin` after semantic fallback",
        "// delta compaction and string interning.",
        f"// Reference runtime: {REFERENCE_RUNTIME_DISPLAY} ({REFERENCE_VENDOR}),",
        f"// Reference java.version: {REFERENCE_JAVA_VERSION}",
        f"// Reference Unicode version: {REFERENCE_UNICODE_VERSION}",
        f"// `java.locale.providers={REFERENCE_LOCALE_PROVIDERS}`.",
        "// Reference archive:",
        f"// {REFERENCE_JDK_URL}",
        f"// Reference archive SHA-256: {REFERENCE_JDK_SHA256}",
        f"// Reference source tag: {REFERENCE_SOURCE_TAG}",
        f"// Reference source revision: {REFERENCE_SOURCE_REVISION}",
        f"// Reference build source revision: {REFERENCE_BUILD_SOURCE_REVISION}",
        f"// Available-locale raw-order manifest: {AVAILABLE_ORDER_MANIFEST}",
        f"// Available-locale raw-order manifest SHA-256: {EXPECTED_AVAILABLE_ORDER_MANIFEST_SHA256}",
        f"// Dump helper SHA-256: {EXPECTED_DUMP_SOURCE_SHA256}",
        f"// Display-data dump helper SHA-256: {EXPECTED_DISPLAY_DUMP_SOURCE_SHA256}",
        f"// Public differential oracle helper SHA-256: {EXPECTED_PUBLIC_ORACLE_SOURCE_SHA256}",
        f"// Public differential oracle output SHA-256: {EXPECTED_PUBLIC_ORACLE_OUTPUT_SHA256}",
        f"// Unicode time-zone source: {REFERENCE_JDK_TZ_SOURCE_MEMBER}",
        f"// Unicode time-zone source SHA-256: {REFERENCE_JDK_TZ_SOURCE_SHA256}",
        f"// Unicode time-zone source rows: {EXPECTED_TZ_SOURCE_ENTRY_COUNT}",
        f"// Unicode time-zone unique source rows: {EXPECTED_TZ_UNIQUE_ENTRY_COUNT}",
        f"// Unicode time-zone candidate count: {EXPECTED_TZ_ID_COUNT}",
        "",
        "#[allow(dead_code)]",
        f"pub const LOCALE_DATASET_VERSION: &str = {rust_string(REFERENCE_RELEASE_VERSION)};",
        "#[allow(dead_code)]",
        f"pub const LOCALE_DATASET_SOURCE_SHA256: &str = {rust_string(source_sha256)};",
        "#[allow(dead_code)]",
        f"pub const LOCALE_DISPLAY_SOURCE_SHA256: &str = {rust_string(display_source_sha256)};",
        "#[allow(dead_code)]",
        f"pub const LOCALE_AVAILABLE_RECORD_COUNT: usize = {len(available)};",
        "#[allow(dead_code)]",
        f"pub const LOCALE_TAG_RECORD_COUNT: usize = {len(tags)};",
        "#[allow(dead_code)]",
        f"pub const LOCALE_NAME_COLLISION_COUNT: usize = {len(collisions)};",
        "#[allow(dead_code)]",
        "pub const LOCALE_AVAILABLE_ORDER_MANIFEST_SHA256: &str = "
        f"{rust_string(EXPECTED_AVAILABLE_ORDER_MANIFEST_SHA256)};",
        "#[allow(dead_code)]",
        f"pub const LOCALE_PUBLIC_ORACLE_RECORD_COUNT: usize = {EXPECTED_PUBLIC_ORACLE_RECORD_COUNT};",
        "#[allow(dead_code)]",
        "pub const LOCALE_PUBLIC_ORACLE_OUTPUT_SHA256: &str = "
        f"{rust_string(EXPECTED_PUBLIC_ORACLE_OUTPUT_SHA256)};",
        "#[allow(dead_code)]",
        f"pub const LOCALE_DISPLAY_ORACLE_RECORD_COUNT: usize = {display_stats['oracle_records']};",
        "#[allow(dead_code)]",
        "pub const LOCALE_DISPLAY_RECORD_COUNT: usize = LOCALE_DISPLAY_ORACLE_RECORD_COUNT;",
        "#[allow(dead_code)]",
        f"pub const LOCALE_DISPLAY_COMPACT_RECORD_COUNT: usize = {display_stats['compact_records']};",
        "#[allow(dead_code)]",
        f"pub const LOCALE_DISPLAY_PROFILE_COUNT: usize = {display_stats['profile_count']};",
        "#[allow(dead_code)]",
        f"pub const LOCALE_DISPLAY_KEY_COUNT: usize = {display_stats['key_count']};",
        "#[allow(dead_code)]",
        f"pub const LOCALE_DISPLAY_VALUE_COUNT: usize = {display_stats['unique_value_count']};",
        "#[allow(dead_code)]",
        f"pub const LOCALE_DISPLAY_RAW_STRING_POOL_BYTES: usize = {display_stats['raw_string_pool_bytes']};",
        "#[allow(dead_code)]",
        f"pub const LOCALE_DISPLAY_NUMERIC_INDEX_BYTES: usize = {display_stats['numeric_index_bytes']};",
        "#[allow(dead_code)]",
        f"pub const LOCALE_DISPLAY_COMPACT_SNAPSHOT_BYTES: usize = {display_stats['compact_snapshot_bytes']};",
        "#[allow(dead_code)]",
        f"pub const LOCALE_DISPLAY_COMPACT_FORMAT_VERSION: u32 = {COMPACT_FORMAT_VERSION};",
        "#[allow(dead_code)]",
        f"pub const LOCALE_DISPLAY_COMPACT_SHA256: &str = {rust_string(compact_sha256)};",
        "#[allow(dead_code)]",
        "pub const LOCALE_DISPLAY_MAX_GENERATED_SOURCE_BYTES: usize = "
        f"{MAX_GENERATED_RUST_SOURCE_BYTES};",
        "#[allow(dead_code)]",
        "pub const LOCALE_DISPLAY_MAX_COMPACT_SNAPSHOT_BYTES: usize = "
        f"{MAX_COMPACT_SNAPSHOT_BYTES};",
        "",
        "// Generated from the pinned JDK25 CLDR routing metadata. These maps",
        "// reproduce provider bundle selection; ResourceBundle candidate",
        "// identities remain implemented by the shared runtime algorithm.",
        "#[allow(dead_code)]",
        f"pub const CLDR_ROUTING_SOURCE_SHA256: &str = {rust_string(REFERENCE_CLDR_SOURCE_SHA256)};",
        "#[allow(dead_code)]",
        "pub static CLDR_PARENT_LOCALES: &[(&str, &str)] = &[",
    ]
    lines.extend(
        f"    ({rust_string(child)}, {rust_string(parent)}),"
        for child, parent in cldr_parent_locales
    )
    lines.extend(
        [
            "];",
            "pub static CLDR_LIKELY_SCRIPTS: &[(&str, &str)] = &[",
        ]
    )
    lines.extend(
        f"    ({rust_string(language)}, {rust_string(script)}),"
        for language, script in cldr_likely_scripts
    )
    lines.extend(
        [
            "];",
            "pub static CLDR_LANGUAGE_ALIASES: &[(&str, &str)] = &[",
        ]
    )
    lines.extend(
        f"    ({rust_string(alias)}, {rust_string(target)}),"
        for alias, target in cldr_language_aliases
    )
    lines.extend(
        [
            "];",
            "#[allow(dead_code)]",
            "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
            "struct LocaleNameCollision {",
            "    display_name: &'static str,",
            "    winner_tag: &'static str,",
            "    member_start: usize,",
            "    member_count: usize,",
            "}",
            "#[allow(dead_code)]",
            "static LOCALE_NAME_COLLISION_MEMBER_TAGS: &[&str] = &[",
        ]
    )
    lines.extend(
        f"    {rust_string(tag)},"
        for _display_name, member_tags in collisions
        for tag in member_tags
    )
    lines.extend(
        [
            "];",
            "#[allow(dead_code)]",
            "static LOCALE_NAME_COLLISIONS: &[LocaleNameCollision] = &[",
        ]
    )
    member_start = 0
    for display_name, member_tags in collisions:
        lines.append("    LocaleNameCollision {")
        lines.append(f"        display_name: {rust_string(display_name)},")
        lines.append(f"        winner_tag: {rust_string(member_tags[0])},")
        lines.append(f"        member_start: {member_start},")
        lines.append(f"        member_count: {len(member_tags)},")
        lines.append("    },")
        member_start += len(member_tags)
    lines.extend(
        [
            "];",
            "static LOCALE_NAME_RECORDS: &[LocaleRecord] = &[",
        ]
    )
    for tag, display_name, localized_name, code, script, country, variant, localized_country in available:
        lines.extend(
            [
                "    LocaleRecord {",
                f"        tag: {rust_string(tag)},",
                f"        display_name: {rust_string(display_name)},",
                f"        localized_name: {rust_string(localized_name)},",
                f"        code: {rust_string(code)},",
                f"        script: {rust_string(script)},",
                f"        country_code: {rust_string(country)},",
                f"        variant: {rust_string(variant)},",
                f"        localized_country_name: {rust_string(localized_country)},",
                "    },",
            ]
        )
    lines.append("];\n")
    lines.append("static LOCALE_TAG_RECORDS: &[LocaleRecord] = &[")
    for tag, display_name, localized_name, code, script, country, variant, localized_country in tags:
        lines.extend(
            [
                "    LocaleRecord {",
                f"        tag: {rust_string(tag)},",
                f"        display_name: {rust_string(display_name)},",
                f"        localized_name: {rust_string(localized_name)},",
                f"        code: {rust_string(code)},",
                f"        script: {rust_string(script)},",
                f"        country_code: {rust_string(country)},",
                f"        variant: {rust_string(variant)},",
                f"        localized_country_name: {rust_string(localized_country)},",
                "    },",
            ]
        )
    lines.append("];\n")
    # The source-size statistic is resolved below because the decimal value is
    # part of the generated source itself.
    lines.extend(
        [
            "#[allow(dead_code)]",
            "pub const LOCALE_DISPLAY_GENERATED_SOURCE_BYTES: usize = 0;",
            "",
        ]
    )
    return "\n".join(lines)


def render_with_source_size(**kwargs: object) -> str:
    source = render_rust_source(**kwargs)
    for _ in range(8):
        source_size = len(source.encode("utf-8"))
        updated = re.sub(
            r"(pub const LOCALE_DISPLAY_GENERATED_SOURCE_BYTES: usize = )\d+(;)",
            rf"\g<1>{source_size}\g<2>",
            source,
        )
        if len(updated.encode("utf-8")) == source_size:
            return updated
        source = updated
    raise ValueError("could not stabilize generated Rust source-size metadata")


def print_stats(
    available: list[LocaleRow],
    tags: list[LocaleRow],
    collisions: list[tuple[str, list[str]]],
    source_sha256: str,
    stats: dict[str, int],
    display_source_sha256: str,
    compact_sha256: str,
) -> None:
    print(f"available_records={len(available)}")
    print(f"canonical_tag_records={len(tags)}")
    print(f"name_collision_classes={len(collisions)}")
    print(f"available_order_manifest_sha256={EXPECTED_AVAILABLE_ORDER_MANIFEST_SHA256}")
    print(f"logical_source_sha256={source_sha256}")
    print(f"locale_dump_helper_sha256={EXPECTED_DUMP_SOURCE_SHA256}")
    print(f"display_dump_helper_sha256={EXPECTED_DISPLAY_DUMP_SOURCE_SHA256}")
    print(f"public_oracle_helper_sha256={EXPECTED_PUBLIC_ORACLE_SOURCE_SHA256}")
    print(f"timezone_source_sha256={REFERENCE_JDK_TZ_SOURCE_SHA256}")
    print(f"timezone_source_rows={EXPECTED_TZ_SOURCE_ENTRY_COUNT}")
    print(f"timezone_unique_source_rows={EXPECTED_TZ_UNIQUE_ENTRY_COUNT}")
    print(f"accepted_timezone_ids={EXPECTED_TZ_ID_COUNT}")
    for key in (
        "oracle_records",
        "compact_records",
        "profile_count",
        "key_count",
        "unique_value_count",
        "raw_string_pool_bytes",
        "numeric_index_bytes",
        "compact_snapshot_bytes",
        "generated_rust_source_bytes",
        "generated_rust_source_lines",
    ):
        print(f"{key}={stats[key]}")
    print(f"logical_display_source_sha256={display_source_sha256}")
    print(f"compact_artifact_sha256={compact_sha256}")


def generate(
    available: list[LocaleRow],
    tags: list[LocaleRow],
    collisions: list[tuple[str, list[str]]],
    source_sha256: str,
    display: list[tuple[str, str, str]],
    display_source_sha256: str,
    cldr_parent_locales: list[tuple[str, str]],
    cldr_likely_scripts: list[tuple[str, str]],
    cldr_language_aliases: list[tuple[str, str]],
) -> tuple[str, bytes, dict[str, int]]:
    binary, model, stats = compact_display(display)
    compact_sha256 = sha256(binary)
    if EXPECTED_DISPLAY_RECORD_COUNT and len(display) != EXPECTED_DISPLAY_RECORD_COUNT:
        raise ValueError(
            "reference locale display record count mismatch: "
            f"expected {EXPECTED_DISPLAY_RECORD_COUNT}, got {len(display)}"
        )
    if EXPECTED_COMPACT_RECORD_COUNT and stats["compact_records"] != EXPECTED_COMPACT_RECORD_COUNT:
        raise ValueError(
            "compact locale display record count mismatch: "
            f"expected {EXPECTED_COMPACT_RECORD_COUNT}, got {stats['compact_records']}"
        )
    if EXPECTED_COMPACT_PROFILE_COUNT and stats["profile_count"] != EXPECTED_COMPACT_PROFILE_COUNT:
        raise ValueError("compact locale profile count mismatch")
    if EXPECTED_COMPACT_KEY_COUNT and stats["key_count"] != EXPECTED_COMPACT_KEY_COUNT:
        raise ValueError("compact locale key count mismatch")
    if EXPECTED_COMPACT_VALUE_COUNT and stats["unique_value_count"] != EXPECTED_COMPACT_VALUE_COUNT:
        raise ValueError("compact locale value count mismatch")
    if EXPECTED_COMPACT_SHA256 and compact_sha256 != EXPECTED_COMPACT_SHA256:
        raise ValueError(
            "compact locale artifact fingerprint mismatch: "
            f"expected {EXPECTED_COMPACT_SHA256}, got {compact_sha256}"
        )
    source = render_with_source_size(
        available=available,
        tags=tags,
        collisions=collisions,
        source_sha256=source_sha256,
        display_source_sha256=display_source_sha256,
        display_stats=stats,
        compact_sha256=compact_sha256,
        cldr_parent_locales=cldr_parent_locales,
        cldr_likely_scripts=cldr_likely_scripts,
        cldr_language_aliases=cldr_language_aliases,
    )
    enforce_budgets(source, binary, stats)
    stats["compact_artifact_sha256"] = compact_sha256
    prove_reconstruction(display, model, binary)
    return source, binary, stats


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--java", type=Path, required=True)
    parser.add_argument(
        "--archive", type=Path, required=True, help="the exact pinned JDK archive containing --java"
    )
    parser.add_argument(
        "--output", type=Path, default=Path("crates/arkst-engine/src/locale_data.rs")
    )
    parser.add_argument(
        "--binary-output",
        type=Path,
        default=Path("crates/arkst-engine/data/jdk25_locale_display.bin"),
    )
    parser.add_argument(
        "--available-order-output",
        type=Path,
        default=Path("tools/jdk25_available_locale_order.tsv"),
    )
    parser.add_argument(
        "--check", action="store_true", help="fail if regeneration differs from checked-in outputs"
    )
    args = parser.parse_args()
    verify_reference_archive(args.archive, args.java)
    timezone_ids = reference_timezone_ids(args.archive)
    cldr_parent_locales, cldr_likely_scripts, cldr_language_aliases = reference_cldr_routing(
        args.archive
    )
    helper = Path(__file__).with_name("dump_jdk25_locale_data.java")
    display_helper = Path(__file__).with_name(DISPLAY_DUMP_HELPER)
    public_oracle_helper = Path(__file__).with_name(PUBLIC_ORACLE_HELPER)
    public_oracle_sha256 = sha256_file(public_oracle_helper)
    if public_oracle_sha256 != EXPECTED_PUBLIC_ORACLE_SOURCE_SHA256:
        raise ValueError(
            "reference public locale oracle helper fingerprint mismatch: "
            f"expected {EXPECTED_PUBLIC_ORACLE_SOURCE_SHA256}, got {public_oracle_sha256}"
        )
    public_oracle_output_sha256 = validate_public_oracle(
        run_reference_public_oracle(args.java, public_oracle_helper)
    )
    available, tags, collisions, source_sha256 = parse_dump(run_reference_java(args.java, helper))
    display, display_source_sha256 = parse_display_dump(
        run_reference_display_java(args.java, display_helper, timezone_ids)
    )
    generated, binary, stats = generate(
        available,
        tags,
        collisions,
        source_sha256,
        display,
        display_source_sha256,
        cldr_parent_locales,
        cldr_likely_scripts,
        cldr_language_aliases,
    )
    print_stats(
        available,
        tags,
        collisions,
        source_sha256,
        stats,
        display_source_sha256,
        stats["compact_artifact_sha256"],
    )
    print(f"public_oracle_record_count={EXPECTED_PUBLIC_ORACLE_RECORD_COUNT}")
    print(f"public_oracle_output_sha256={public_oracle_output_sha256}")
    if args.check:
        if args.output.read_bytes() != generated.encode("utf-8"):
            raise SystemExit(f"generated output differs: {args.output}")
        if args.binary_output.read_bytes() != binary:
            raise SystemExit(f"generated output differs: {args.binary_output}")
        if args.available_order_output.read_bytes() != available_order_manifest_bytes(available):
            raise SystemExit(f"generated output differs: {args.available_order_output}")
    else:
        args.output.write_bytes(generated.encode("utf-8"))
        args.binary_output.write_bytes(binary)
        args.available_order_output.write_bytes(available_order_manifest_bytes(available))


if __name__ == "__main__":
    main()
