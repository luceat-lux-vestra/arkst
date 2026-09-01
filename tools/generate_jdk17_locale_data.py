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
import hashlib
import io
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


REFERENCE_RUNTIME_VERSION = "17.0.20.1+1"
REFERENCE_VENDOR = "Eclipse Adoptium"
REFERENCE_VENDOR_VERSION = "Temurin-17.0.20.1+1"
REFERENCE_LOCALE_PROVIDERS = "CLDR,COMPAT"
REFERENCE_JDK_URL = (
    "https://github.com/adoptium/temurin17-binaries/releases/download/"
    "jdk-17.0.20.1%2B1/"
    "OpenJDK17U-jdk_aarch64_mac_hotspot_17.0.20.1_1.tar.gz"
)
REFERENCE_JDK_SHA256 = "196d13ba5f10414bef7f6a05a9b3f00edacb18ebacef2b99485db9e2ee18f0e8"
REFERENCE_JDK_SIZE = 185851019
EXPECTED_AVAILABLE_RECORD_COUNT = 1016
EXPECTED_TAG_RECORD_COUNT = 1015
EXPECTED_SOURCE_SHA256 = "a21268dd1fb3cc6fd5cea32b52fa63099eb390a7e82c27636195db1086d645fd"
EXPECTED_DUMP_SOURCE_SHA256 = "8bee476739d8b83d981811b2ccf3a91432cca668037dfdf119f9a3c84b15ba62"
EXPECTED_DISPLAY_RECORD_COUNT = 308533
EXPECTED_DISPLAY_SOURCE_SHA256 = "03d633326dc30ac8423cfb14b4bc0d3fa4f35e7a86575e8eefbdf540c620d489"
EXPECTED_DISPLAY_DUMP_SOURCE_SHA256 = "56c2df3f178e94884d3e79d941e739aa6e0b309ef83e0e6ce0090a57c637dd4d"
EXPECTED_COMPACT_RECORD_COUNT = 152731
EXPECTED_COMPACT_PROFILE_COUNT = 287
EXPECTED_COMPACT_KEY_COUNT = 1569
EXPECTED_COMPACT_VALUE_COUNT = 88024
EXPECTED_COMPACT_SHA256 = "c6666932c941652192cc351e75fb613d040e78bca8dc3b3623276c239e2fa8cb"

REFERENCE_JDK_TZ_SOURCE_MEMBER = "java.base/sun/util/cldr/CLDRBaseLocaleDataMetaInfo.java"
REFERENCE_JDK_TZ_SOURCE_SHA256 = "cae696f21cb57af82b17aa72c4b08a31cc703551ff3d4e5a284d54c9dd38a59c"
EXPECTED_TZ_SOURCE_ENTRY_COUNT = 593
EXPECTED_TZ_ID_COUNT = 461
DISPLAY_DUMP_HELPER = "dump_jdk17_locale_display_data.java"
JDK_EXPORTS = (
    "--add-exports=java.base/sun.util.resources=ALL-UNNAMED",
    "--add-exports=java.base/sun.util.locale.provider=ALL-UNNAMED",
)

# Executable representation budgets. These are checked during every
# regeneration, and the Rust integrity tests repeat the checked-in limits.
COMPACT_FORMAT_VERSION = 1
MAX_GENERATED_RUST_SOURCE_BYTES = 1 * 1024 * 1024
MAX_GENERATED_RUST_SOURCE_LINES = 100_000
MAX_COMPACT_SNAPSHOT_BYTES = 8 * 1024 * 1024

# This is the single semantic fallback order used by compaction. The runtime
# consumes the same ordered profile candidates; it must not invent a second
# provider fallback policy.
DISPLAY_FALLBACK_ORDER = (
    "language-script-region",
    "language-script",
    "language-region",
    "language",
    "en",
    "root",
)

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
            suffix = f"/Contents/Home/bin/{executable}"
            members = [member for member in tar.getmembers() if member.name.endswith(suffix)]
            if len(members) != 1:
                raise ValueError(
                    "reference JDK archive must contain exactly one "
                    f"Contents/Home/bin/{executable}, found {len(members)}"
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
    with tarfile.open(archive, mode="r:gz") as tar:
        src_zip_members = [
            member
            for member in tar.getmembers()
            if member.name.endswith("/Contents/Home/lib/src.zip")
        ]
        if len(src_zip_members) != 1:
            raise ValueError(
                "reference JDK archive must contain exactly one Contents/Home/lib/src.zip, "
                f"found {len(src_zip_members)}"
            )
        src_zip = tar.extractfile(src_zip_members[0])
        if src_zip is None:
            raise ValueError("reference JDK src.zip archive member is not a regular file")
        src_zip_bytes = src_zip.read()

    with zipfile.ZipFile(io.BytesIO(src_zip_bytes)) as source_archive:
        source = source_archive.read(REFERENCE_JDK_TZ_SOURCE_MEMBER)
    source_sha256 = sha256(source)
    if source_sha256 != REFERENCE_JDK_TZ_SOURCE_SHA256:
        raise ValueError(
            "reference JDK timezone source fingerprint mismatch: "
            f"expected {REFERENCE_JDK_TZ_SOURCE_SHA256}, got {source_sha256}"
        )

    matches = re.findall(rb'tzCanonicalIDMap\.put\("([^"]+)",', source)
    if len(matches) != EXPECTED_TZ_SOURCE_ENTRY_COUNT or len(set(matches)) != len(matches):
        raise ValueError(
            "reference JDK timezone source entry count/uniqueness mismatch: "
            f"expected {EXPECTED_TZ_SOURCE_ENTRY_COUNT} unique entries, got "
            f"{len(matches)} entries and {len(set(matches))} unique entries"
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
    if len(timezone_ids) != EXPECTED_TZ_ID_COUNT:
        raise ValueError(
            "reference JDK Unicode timezone-id candidate count mismatch: "
            f"expected {EXPECTED_TZ_ID_COUNT}, got {len(timezone_ids)}"
        )
    return timezone_ids


def run_reference_java(java: Path, helper: Path) -> bytes:
    helper_sha256 = sha256(helper.read_bytes())
    if helper_sha256 != EXPECTED_DUMP_SOURCE_SHA256:
        raise ValueError(
            "reference locale dump helper fingerprint mismatch: "
            f"expected {EXPECTED_DUMP_SOURCE_SHA256}, got {helper_sha256}"
        )
    completed = subprocess.run(
        [
            str(java),
            f"-Djava.locale.providers={REFERENCE_LOCALE_PROVIDERS}",
            "--source",
            "17",
            str(helper),
        ],
        check=False,
        capture_output=True,
    )
    if completed.returncode != 0:
        raise RuntimeError(
            "reference JDK locale dump failed:\n"
            + completed.stderr.decode("utf-8", errors="replace")
        )
    return completed.stdout


def run_reference_display_java(
    java: Path, helper: Path, timezone_ids: list[str]
) -> bytes:
    helper_sha256 = sha256(helper.read_bytes())
    if helper_sha256 != EXPECTED_DISPLAY_DUMP_SOURCE_SHA256:
        raise ValueError(
            "reference locale display dump helper fingerprint mismatch: "
            f"expected {EXPECTED_DISPLAY_DUMP_SOURCE_SHA256}, got {helper_sha256}"
        )
    javac = java.with_name("javac")
    with tempfile.TemporaryDirectory(prefix="scribium-jdk17-locale-display-") as output:
        compiled = subprocess.run(
            [str(javac), *JDK_EXPORTS, "-d", output, str(helper)],
            check=False,
            capture_output=True,
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
                f"-Djava.locale.providers={REFERENCE_LOCALE_PROVIDERS}",
                "-cp",
                output,
                "DumpJdk17LocaleDisplayData",
                "--timezone",
                *timezone_ids,
            ],
            check=False,
            capture_output=True,
        )
    if completed.returncode != 0:
        raise RuntimeError(
            "reference JDK locale display dump failed:\n"
            + completed.stderr.decode("utf-8", errors="replace")
        )
    return completed.stdout


def parse_dump(
    raw: bytes,
) -> tuple[list[tuple[str, str, str]], list[tuple[str, str, str]], str]:
    lines = raw.decode("utf-8").splitlines()
    metadata: dict[str, str] = {}
    available: list[tuple[str, str, str]] = []
    tags: list[tuple[str, str, str]] = []
    for line_number, line in enumerate(lines, 1):
        fields = line.split("\t")
        if fields[0] in {"available", "tag"}:
            if len(fields) != 4 or any(not field for field in fields[1:]):
                raise ValueError(f"dump line {line_number}: malformed locale {fields[0]}")
            (available if fields[0] == "available" else tags).append(
                (fields[1], fields[2], fields[3])
            )
        else:
            if len(fields) != 2 or fields[0] in metadata:
                raise ValueError(f"dump line {line_number}: malformed metadata")
            metadata[fields[0]] = fields[1]

    expected_metadata = {
        "runtime.version": REFERENCE_RUNTIME_VERSION,
        "java.vendor": REFERENCE_VENDOR,
        "java.vendor.version": REFERENCE_VENDOR_VERSION,
        "java.locale.providers": REFERENCE_LOCALE_PROVIDERS,
    }
    if metadata != expected_metadata:
        raise ValueError(
            "reference runtime metadata mismatch: "
            f"expected {expected_metadata!r}, got {metadata!r}"
        )
    if len(available) != EXPECTED_AVAILABLE_RECORD_COUNT:
        raise ValueError(
            "reference available-locale record count mismatch: "
            f"expected {EXPECTED_AVAILABLE_RECORD_COUNT}, got {len(available)}"
        )
    if len(tags) != EXPECTED_TAG_RECORD_COUNT:
        raise ValueError(
            "reference canonical-tag record count mismatch: "
            f"expected {EXPECTED_TAG_RECORD_COUNT}, got {len(tags)}"
        )
    duplicate_available_tags = {
        tag: count
        for tag, count in Counter(tag for tag, _display, _localized in available).items()
        if count > 1
    }
    if duplicate_available_tags != {"nn-NO": 2}:
        raise ValueError(
            "unexpected duplicate available-locale tags: "
            f"{duplicate_available_tags!r}"
        )

    source_bytes = "".join(
        f"{kind}\t{tag}\t{display_name}\t{localized_name}\n"
        for kind, rows in (("available", available), ("tag", tags))
        for tag, display_name, localized_name in rows
    ).encode("utf-8")
    source_sha256 = sha256(source_bytes)
    if source_sha256 != EXPECTED_SOURCE_SHA256:
        raise ValueError(
            "reference locale source fingerprint mismatch: "
            f"expected {EXPECTED_SOURCE_SHA256}, got {source_sha256}"
        )
    return available, tags, source_sha256


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


def fallback_profiles(profile: str) -> list[str]:
    """Return the one canonical provider fallback sequence for one profile."""
    if not profile:
        return []
    parts = profile.split("-")
    language = parts[0]
    script = next(
        (part for part in parts[1:] if len(part) == 4 and is_ascii_alpha(part)),
        None,
    )
    region = next(
        (
            part
            for part in parts[1:]
            if (len(part) == 2 and is_ascii_alpha(part))
            or (len(part) == 3 and part.isascii() and part.isdigit())
        ),
        None,
    )
    candidates: list[str] = []

    def add(candidate: str) -> None:
        if candidate != profile and candidate not in candidates:
            candidates.append(candidate)

    if script is not None and region is not None:
        add(f"{language}-{script}-{region}")
    if script is not None:
        add(f"{language}-{script}")
    if region is not None:
        add(f"{language}-{region}")
    add(language)
    add("en")
    add("")
    return candidates


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
        profile = self.profiles[profile_id]
        value = self.lookup(profile, key)
        if value is not None:
            return value
        start, end = self.fallback_ranges[profile_id : profile_id + 2]
        for fallback_id in self.fallback_ids[start:end]:
            value = self.resolve_profile(fallback_id, key)
            if value is not None:
                return value
        return None

    def resolve(self, profile: str, key: str) -> str | None:
        for candidate in [profile, *fallback_profiles(profile)]:
            profile_id = bisect.bisect_left(self.profiles, candidate)
            if profile_id < len(self.profiles) and self.profiles[profile_id] == candidate:
                value = self.resolve_profile(profile_id, key)
                if value is not None:
                    return value
        return None


def compact_display(
    display: list[tuple[str, str, str]],
) -> tuple[bytes, CompactModel, dict[str, int]]:
    full = {(profile, key): value for profile, key, value in display}
    profiles = sorted({profile for profile, _key, _value in display} | {""})
    keys = sorted({key for _profile, key, _value in display})
    retained: list[tuple[str, str, str]] = []
    for profile, key, value in display:
        fallback_value = next(
            (
                full[(candidate, key)]
                for candidate in fallback_profiles(profile)
                if (candidate, key) in full
            ),
            None,
        )
        if fallback_value != value:
            retained.append((profile, key, value))

    values = sorted({value for _profile, _key, value in retained})
    profile_ids = {profile: index for index, profile in enumerate(profiles)}
    key_ids = {key: index for index, key in enumerate(keys)}
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

    fallback_ids: list[int] = []
    fallback_ranges = [0]
    for profile in profiles:
        fallback_ids.extend(
            profile_ids[candidate]
            for candidate in fallback_profiles(profile)
            if candidate in profile_ids
        )
        fallback_ranges.append(len(fallback_ids))

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
    available: list[tuple[str, str, str]],
    tags: list[tuple[str, str, str]],
    source_sha256: str,
    display_source_sha256: str,
    display_stats: dict[str, int],
    compact_sha256: str,
) -> str:
    lines = [
        "// Generated by `tools/generate_jdk17_locale_data.py`.",
        "//",
        "// Locale names are from the complete pinned JDK oracle; effective",
        "// CLDR→COMPAT display data",
        "// is stored in `data/jdk17_locale_display.bin` after semantic fallback",
        "// delta compaction and string interning.",
        f"// Reference runtime: {REFERENCE_VENDOR} {REFERENCE_VENDOR_VERSION},",
        f"// `java.locale.providers={REFERENCE_LOCALE_PROVIDERS}`.",
        "// Reference archive:",
        f"// {REFERENCE_JDK_URL}",
        f"// Reference archive SHA-256: {REFERENCE_JDK_SHA256}",
        f"// Dump helper SHA-256: {EXPECTED_DUMP_SOURCE_SHA256}",
        f"// Display-data dump helper SHA-256: {EXPECTED_DISPLAY_DUMP_SOURCE_SHA256}",
        f"// Unicode time-zone source: {REFERENCE_JDK_TZ_SOURCE_MEMBER}",
        f"// Unicode time-zone source SHA-256: {REFERENCE_JDK_TZ_SOURCE_SHA256}",
        f"// Unicode time-zone candidate count: {EXPECTED_TZ_ID_COUNT}",
        "",
        "#[allow(dead_code)]",
        f"pub const LOCALE_DATASET_VERSION: &str = {rust_string(REFERENCE_RUNTIME_VERSION)};",
        "#[allow(dead_code)]",
        f"pub const LOCALE_DATASET_SOURCE_SHA256: &str = {rust_string(source_sha256)};",
        "#[allow(dead_code)]",
        f"pub const LOCALE_DISPLAY_SOURCE_SHA256: &str = {rust_string(display_source_sha256)};",
        "#[allow(dead_code)]",
        f"pub const LOCALE_AVAILABLE_RECORD_COUNT: usize = {len(available)};",
        "#[allow(dead_code)]",
        f"pub const LOCALE_TAG_RECORD_COUNT: usize = {len(tags)};",
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
        "pub const LOCALE_DISPLAY_FALLBACK_ORDER: &[&str] = &[",
        *[f"    {rust_string(item)}," for item in DISPLAY_FALLBACK_ORDER],
        "];",
        "",
        "static LOCALE_NAME_RECORDS: &[LocaleRecord] = &[",
    ]
    for tag, display_name, localized_name in available:
        lines.extend(
            [
                "    LocaleRecord {",
                f"        tag: {rust_string(tag)},",
                f"        display_name: {rust_string(display_name)},",
                f"        localized_name: {rust_string(localized_name)},",
                "    },",
            ]
        )
    lines.append("];\n")
    lines.append("static LOCALE_TAG_RECORDS: &[LocaleRecord] = &[")
    for tag, display_name, localized_name in tags:
        lines.extend(
            [
                "    LocaleRecord {",
                f"        tag: {rust_string(tag)},",
                f"        display_name: {rust_string(display_name)},",
                f"        localized_name: {rust_string(localized_name)},",
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


def print_stats(stats: dict[str, int], display_source_sha256: str, compact_sha256: str) -> None:
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
    available: list[tuple[str, str, str]],
    tags: list[tuple[str, str, str]],
    source_sha256: str,
    display: list[tuple[str, str, str]],
    display_source_sha256: str,
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
        source_sha256=source_sha256,
        display_source_sha256=display_source_sha256,
        display_stats=stats,
        compact_sha256=compact_sha256,
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
        "--output", type=Path, default=Path("crates/scribium-engine/src/locale_data.rs")
    )
    parser.add_argument(
        "--binary-output",
        type=Path,
        default=Path("crates/scribium-engine/data/jdk17_locale_display.bin"),
    )
    parser.add_argument(
        "--check", action="store_true", help="fail if regeneration differs from checked-in outputs"
    )
    args = parser.parse_args()
    verify_reference_archive(args.archive, args.java)
    timezone_ids = reference_timezone_ids(args.archive)
    helper = Path(__file__).with_name("dump_jdk17_locale_data.java")
    display_helper = Path(__file__).with_name(DISPLAY_DUMP_HELPER)
    available, tags, source_sha256 = parse_dump(run_reference_java(args.java, helper))
    display, display_source_sha256 = parse_display_dump(
        run_reference_display_java(args.java, display_helper, timezone_ids)
    )
    generated, binary, stats = generate(
        available, tags, source_sha256, display, display_source_sha256
    )
    print_stats(stats, display_source_sha256, stats["compact_artifact_sha256"])
    if args.check:
        if args.output.read_bytes() != generated.encode("utf-8"):
            raise SystemExit(f"generated output differs: {args.output}")
        if args.binary_output.read_bytes() != binary:
            raise SystemExit(f"generated output differs: {args.binary_output}")
    else:
        args.output.write_bytes(generated.encode("utf-8"))
        args.binary_output.write_bytes(binary)


if __name__ == "__main__":
    main()
