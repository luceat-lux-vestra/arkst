#!/usr/bin/env python3
"""Verify Scribium's generated/reference-data integrity contract.

The two specialized manifests remain the source of target-specific values:
the JDK manifest describes an archive-backed oracle and the Markdown manifest
describes immutable-git references. This verifier supplies their shared,
fail-closed integrity rules without making those transport models identical.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
CONTRACT = "scribium.reference-data-integrity"
CONTRACT_VERSION = 1
SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
REVISION_RE = re.compile(r"^[0-9a-f]{40}$")


class VerificationError(RuntimeError):
    """A deterministic contract violation."""


def read_toml(path: Path) -> dict[str, Any]:
    try:
        with path.open("rb") as stream:
            value = tomllib.load(stream)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise VerificationError(f"cannot read TOML manifest {path}: {error}") from error
    if not isinstance(value, dict):
        raise VerificationError(f"manifest {path} is not a TOML table")
    return value


def read_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise VerificationError(f"cannot read JSON artifact {path}: {error}") from error


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as stream:
            for chunk in iter(lambda: stream.read(1024 * 1024), b""):
                digest.update(chunk)
    except OSError as error:
        raise VerificationError(f"cannot read {path}: {error}") from error
    return digest.hexdigest()


def relative_path(root: Path, value: Any, field: str) -> Path:
    if not isinstance(value, str) or not value:
        raise VerificationError(f"{field} must be a non-empty repository-relative path")
    path = Path(value)
    if path.is_absolute() or ".." in path.parts:
        raise VerificationError(f"{field} must stay inside the repository: {value!r}")
    return root / path


def external_path(value: Any, field: str) -> Path:
    if not isinstance(value, str) or not value:
        raise VerificationError(f"{field} must be a non-empty path")
    return Path(value)


def require_string(table: dict[str, Any], key: str, field: str) -> str:
    value = table.get(key)
    if not isinstance(value, str) or not value:
        raise VerificationError(f"{field} must be a non-empty string")
    return value


def require_int(table: dict[str, Any], key: str, field: str) -> int:
    value = table.get(key)
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise VerificationError(f"{field} must be a non-negative integer")
    return value


def require_sha(table: dict[str, Any], key: str, field: str) -> str:
    value = require_string(table, key, field)
    if SHA256_RE.fullmatch(value) is None:
        raise VerificationError(f"{field} must be a lowercase SHA-256 digest")
    return value


def require_revision(table: dict[str, Any], key: str, field: str) -> str:
    value = require_string(table, key, field)
    if REVISION_RE.fullmatch(value) is None:
        raise VerificationError(f"{field} must be a full lowercase git revision")
    return value


def check_artifact(
    path: Path,
    *,
    label: str,
    expected_bytes: int,
    minimum_bytes: int,
    maximum_bytes: int,
    expected_sha256: str,
    policy: str,
) -> None:
    if minimum_bytes > maximum_bytes:
        raise VerificationError(f"{label}: size bounds are inverted")
    if policy == "exact" and not (
        expected_bytes == minimum_bytes == maximum_bytes
    ):
        raise VerificationError(f"{label}: exact policy requires equal size values")
    if not path.is_file():
        raise VerificationError(f"{label}: required artifact is missing: {path}")
    actual_bytes = path.stat().st_size
    if actual_bytes < minimum_bytes or actual_bytes > maximum_bytes:
        raise VerificationError(
            f"{label}: size {actual_bytes} is outside [{minimum_bytes}, {maximum_bytes}]"
        )
    if policy == "exact" and actual_bytes != expected_bytes:
        raise VerificationError(
            f"{label}: expected exactly {expected_bytes} bytes, got {actual_bytes}"
        )
    actual_sha256 = sha256_file(path)
    if actual_sha256 != expected_sha256:
        raise VerificationError(
            f"{label}: expected SHA-256 {expected_sha256}, got {actual_sha256}"
        )


def contract_table(root: Path, manifest: dict[str, Any], label: str) -> dict[str, Any]:
    if manifest.get("schema_version") != 1:
        raise VerificationError(f"{label}: unsupported or missing schema_version = 1")
    provenance = manifest.get("provenance")
    if not isinstance(provenance, dict):
        raise VerificationError(f"{label}: missing [provenance] table")
    if provenance.get("contract") != CONTRACT:
        raise VerificationError(f"{label}: wrong provenance contract identity")
    if provenance.get("contract_version") != CONTRACT_VERSION:
        raise VerificationError(f"{label}: unsupported provenance contract version")
    source_kind = require_string(provenance, "source_kind", f"{label}.source_kind")
    if source_kind not in {"archive-backed", "immutable-git"}:
        raise VerificationError(f"{label}: unsupported source kind {source_kind!r}")
    verifier = relative_path(root, provenance.get("verifier_path"), f"{label}.verifier_path")
    expected_verifier_sha = require_sha(
        provenance, "verifier_source_sha256", f"{label}.verifier_source_sha256"
    )
    check_artifact(
        verifier,
        label=f"{label} verifier",
        expected_bytes=verifier.stat().st_size if verifier.is_file() else 0,
        minimum_bytes=1,
        maximum_bytes=sys.maxsize,
        expected_sha256=expected_verifier_sha,
        policy="bounded",
    )
    notice = relative_path(root, provenance.get("notice_path"), f"{label}.notice_path")
    if not notice.is_file():
        raise VerificationError(f"{label}: required NOTICE file is missing: {notice}")
    markers = provenance.get("notice_required_markers")
    if not isinstance(markers, list) or not markers or not all(
        isinstance(marker, str) and marker for marker in markers
    ):
        raise VerificationError(f"{label}: notice_required_markers must be non-empty")
    notice_text = notice.read_text(encoding="utf-8")
    for marker in markers:
        if marker not in notice_text:
            raise VerificationError(f"{label}: NOTICE marker is missing: {marker!r}")
    if provenance.get("runtime_scope") != "development-ci-only":
        raise VerificationError(f"{label}: reference tooling must be development/CI-only")
    return provenance


def check_exact_globs(
    root: Path, provenance: dict[str, Any], expected_paths: set[str], label: str
) -> None:
    patterns = provenance.get("exact_artifact_globs")
    if not isinstance(patterns, list) or not patterns or not all(
        isinstance(pattern, str) and pattern for pattern in patterns
    ):
        raise VerificationError(f"{label}: exact_artifact_globs must be non-empty")
    actual_paths: set[str] = set()
    for pattern in patterns:
        for candidate in root.glob(pattern):
            if candidate.is_file():
                actual_paths.add(candidate.relative_to(root).as_posix())
    if actual_paths != expected_paths:
        missing = sorted(expected_paths - actual_paths)
        extra = sorted(actual_paths - expected_paths)
        raise VerificationError(
            f"{label}: exact generated-artifact set changed; missing={missing}, extra={extra}"
        )


def check_script_identity(
    root: Path, table: dict[str, Any], path_key: str, sha_key: str, label: str
) -> None:
    path = relative_path(root, table.get(path_key), f"{label}.{path_key}")
    expected_sha = require_sha(table, sha_key, f"{label}.{sha_key}")
    if not path.is_file():
        raise VerificationError(f"{label}: required tool is missing: {path}")
    actual_sha = sha256_file(path)
    if actual_sha != expected_sha:
        raise VerificationError(
            f"{label}: {path_key} identity mismatch; expected {expected_sha}, got {actual_sha}"
        )


def rust_const(source: str, name: str) -> str:
    match = re.search(
        rf"pub const {re.escape(name)}\s*:\s*[^=]+\s*=\s*([^;]+);", source
    )
    if match is None:
        raise VerificationError(f"generated Rust source is missing constant {name}")
    value = match.group(1).strip()
    if value.startswith('"') and value.endswith('"'):
        return value[1:-1]
    return value


def check_rust_const(source: str, name: str, expected: Any, label: str) -> None:
    actual = rust_const(source, name)
    if str(actual) != str(expected):
        raise VerificationError(
            f"{label}: {name} expected {expected!r}, got {actual!r}"
        )


def check_jdk_static(root: Path, manifest_path: Path) -> tuple[dict[str, Any], dict[str, Any]]:
    manifest = read_toml(manifest_path)
    provenance = contract_table(root, manifest, "JDK")
    reference = manifest.get("reference")
    if not isinstance(reference, dict):
        raise VerificationError("JDK: missing [reference] table")
    if reference.get("source_kind") != "archive-backed":
        raise VerificationError("JDK: reference source_kind must be archive-backed")
    for key in (
        "vendor",
        "distribution",
        "release_version",
        "runtime_display",
        "java_version",
        "runtime_version",
        "release_tag",
        "locale_provider",
        "unicode_version",
        "archive_filename",
        "archive_url",
        "source_repository",
        "source_tag",
        "source_revision_kind",
        "source_tag_proof",
        "build_source_repository",
        "build_source_revision_kind",
        "release_date",
        "implementor",
        "implementor_version",
        "java_version_date",
    ):
        require_string(reference, key, f"JDK.reference.{key}")
    for key in ("archive_sha256", "helper_source_sha256", "oracle_output_sha256", "generated_source_sha256"):
        require_sha(reference, key, f"JDK.reference.{key}")
    for key in ("source_revision", "build_source_revision"):
        require_revision(reference, key, f"JDK.reference.{key}")
    if reference["source_revision_kind"] != "git-commit" or reference["build_source_revision_kind"] != "git-commit":
        raise VerificationError("JDK: source revisions must be immutable git commits")
    if reference["source_tag_proof"] != "peeled-git-tag":
        raise VerificationError("JDK: source tag proof must be a peeled git tag")
    require_int(reference, "archive_bytes", "JDK.reference.archive_bytes")
    if reference["source_tag"] == reference["release_tag"]:
        raise VerificationError("JDK: source tag must identify the JDK source, not the binary release tag")
    if not reference["archive_url"].startswith("https://"):
        raise VerificationError("JDK: archive URL must be HTTPS")
    if not reference["source_repository"].startswith("https://") or not reference[
        "build_source_repository"
    ].startswith("https://"):
        raise VerificationError("JDK: source repositories must be HTTPS")
    source_license_ids = reference.get("source_license_ids")
    source_license_urls = reference.get("source_license_urls")
    if not isinstance(source_license_ids, list) or not source_license_ids or not all(
        isinstance(value, str) and value for value in source_license_ids
    ):
        raise VerificationError("JDK: source_license_ids must be explicit")
    if not isinstance(source_license_urls, list) or not source_license_urls or not all(
        isinstance(value, str) and value.startswith("https://") for value in source_license_urls
    ):
        raise VerificationError("JDK: source_license_urls must be explicit HTTPS links")
    notice_text = relative_path(root, provenance["notice_path"], "JDK notice path").read_text(encoding="utf-8")
    for url in source_license_urls:
        if url not in notice_text:
            raise VerificationError(f"JDK: source license URL is not linked from NOTICE: {url}")
    require_string(reference, "locale_semantic_verifier_test", "JDK.reference.locale_semantic_verifier_test")
    require_string(reference, "locale_semantic_result_id", "JDK.reference.locale_semantic_result_id")
    java_version_output = reference.get("java_version_output")
    expected_java_version_output = [
        f'openjdk version "{reference["java_version"]}" {reference["java_version_date"]} LTS',
        f"OpenJDK Runtime Environment {reference['implementor_version']} (build {reference['runtime_version']})",
        f"OpenJDK 64-Bit Server VM {reference['implementor_version']} (build {reference['runtime_version']}, mixed mode, sharing)",
    ]
    if java_version_output != expected_java_version_output:
        raise VerificationError("JDK.reference.java_version_output is inconsistent with the identity fields")

    for path_key, sha_key, label in (
        ("unicode_helper_path", "helper_source_sha256", "JDK Unicode helper"),
        ("unicode_generator_path", "unicode_generator_source_sha256", "JDK Unicode generator"),
        ("unicode_verifier_path", "unicode_verifier_source_sha256", "JDK Unicode verifier"),
        ("locale_dump_helper_path", "locale_dump_helper_source_sha256", "JDK locale helper"),
        ("locale_display_dump_helper_path", "locale_display_dump_helper_source_sha256", "JDK display helper"),
        ("locale_public_oracle_helper_path", "locale_public_oracle_helper_source_sha256", "JDK public oracle helper"),
        ("locale_generator_path", "locale_generator_source_sha256", "JDK locale generator"),
        ("locale_semantic_verifier_path", "locale_semantic_verifier_source_sha256", "JDK semantic verifier"),
    ):
        check_script_identity(root, reference, path_key, sha_key, label)

    unicode_path = relative_path(root, reference.get("unicode_generated_source_path"), "JDK unicode artifact path")
    unicode_bytes = require_int(reference, "unicode_generated_source_bytes", "JDK unicode artifact bytes")
    unicode_min = require_int(reference, "unicode_generated_source_min_bytes", "JDK unicode artifact min bytes")
    unicode_max = require_int(reference, "unicode_generated_source_max_bytes", "JDK unicode artifact max bytes")
    unicode_limit = require_int(reference, "unicode_generated_source_limit_bytes", "JDK unicode artifact limit bytes")
    if unicode_max > unicode_limit:
        raise VerificationError("JDK Unicode generated Rust: maximum exceeds its size limit")
    check_artifact(
        unicode_path,
        label="JDK Unicode generated Rust",
        expected_bytes=unicode_bytes,
        minimum_bytes=unicode_min,
        maximum_bytes=unicode_max,
        expected_sha256=reference["generated_source_sha256"],
        policy=require_string(reference, "unicode_artifact_size_policy", "JDK unicode artifact policy"),
    )
    unicode_source = unicode_path.read_text(encoding="utf-8")
    if f"//! Unicode {reference['unicode_version']} case mappings" not in unicode_source:
        raise VerificationError("JDK Unicode artifact: Unicode version identity is stale")
    for name, expected in (
        ("REFERENCE_JVM_VERSION", reference["java_version"]),
        ("REFERENCE_JVM_RUNTIME_VERSION", reference["runtime_version"]),
        ("REFERENCE_JVM_VENDOR_VERSION", reference["implementor_version"]),
        ("REFERENCE_JVM_ARCHIVE_SHA256", reference["archive_sha256"]),
        ("ORACLE_OUTPUT_SHA256", reference["oracle_output_sha256"]),
        ("SCALAR_MAPPING_RECORD_COUNT", reference["unicode_scalar_mapping_record_count"]),
        ("UTF16_CHAR_RECORD_COUNT", reference["unicode_utf16_char_record_count"]),
    ):
        check_rust_const(unicode_source, name, expected, "JDK Unicode artifact")
    unicode_corpus = relative_path(root, reference.get("unicode_corpus_path"), "JDK Unicode corpus path")
    check_artifact(
        unicode_corpus,
        label="JDK Unicode oracle corpus",
        expected_bytes=require_int(reference, "unicode_corpus_bytes", "JDK Unicode corpus bytes"),
        minimum_bytes=require_int(reference, "unicode_corpus_min_bytes", "JDK Unicode corpus min bytes"),
        maximum_bytes=require_int(reference, "unicode_corpus_max_bytes", "JDK Unicode corpus max bytes"),
        expected_sha256=require_sha(reference, "unicode_corpus_sha256", "JDK Unicode corpus SHA-256"),
        policy="exact",
    )
    corpus_records = sum(
        1 for line in unicode_corpus.read_text(encoding="utf-8").splitlines() if line and not line.startswith("#")
    )
    if corpus_records != require_int(reference, "unicode_corpus_record_count", "JDK Unicode corpus records"):
        raise VerificationError("JDK Unicode oracle corpus record count changed")

    locale_path = relative_path(root, reference.get("locale_generated_rust_path"), "JDK locale artifact path")
    locale_source = locale_path.read_text(encoding="utf-8") if locale_path.is_file() else ""
    locale_limit = require_int(reference, "locale_generated_rust_limit_bytes", "JDK locale artifact limit bytes")
    locale_max = require_int(reference, "locale_generated_rust_max_bytes", "JDK locale artifact max bytes")
    if locale_max > locale_limit:
        raise VerificationError("JDK locale generated Rust: maximum exceeds its size limit")
    check_artifact(
        locale_path,
        label="JDK locale generated Rust",
        expected_bytes=require_int(reference, "locale_generated_rust_bytes", "JDK locale artifact bytes"),
        minimum_bytes=require_int(reference, "locale_generated_rust_min_bytes", "JDK locale artifact min bytes"),
        maximum_bytes=locale_max,
        expected_sha256=require_sha(reference, "locale_generated_rust_sha256", "JDK locale artifact SHA-256"),
        policy=require_string(reference, "locale_artifact_size_policy", "JDK locale artifact policy"),
    )
    if locale_source.count("\n") != require_int(reference, "locale_generated_rust_lines", "JDK locale artifact lines"):
        raise VerificationError("JDK locale generated Rust line count changed")
    for marker in (
        f"Reference runtime: {reference['runtime_display']} ({reference['vendor']}),",
        f"Reference java.version: {reference['java_version']}",
        f"Reference Unicode version: {reference['unicode_version']}",
        f"`java.locale.providers={reference['locale_provider']}`.",
        f"// {reference['archive_url']}",
        f"Reference archive SHA-256: {reference['archive_sha256']}",
        f"Reference source tag: {reference['source_tag']}",
        f"Reference source revision: {reference['source_revision']}",
        f"Reference build source revision: {reference['build_source_revision']}",
    ):
        if marker not in locale_source:
            raise VerificationError(f"JDK locale artifact: missing provenance marker {marker!r}")
    for name, expected in (
        ("LOCALE_DATASET_VERSION", reference["release_version"]),
        ("LOCALE_DATASET_SOURCE_SHA256", reference["locale_logical_source_sha256"]),
        ("LOCALE_DISPLAY_SOURCE_SHA256", reference["locale_logical_display_source_sha256"]),
        ("LOCALE_AVAILABLE_RECORD_COUNT", reference["locale_available_record_count"]),
        ("LOCALE_TAG_RECORD_COUNT", reference["locale_canonical_tag_record_count"]),
        ("LOCALE_NAME_COLLISION_COUNT", reference["locale_name_collision_count"]),
        ("LOCALE_AVAILABLE_ORDER_MANIFEST_SHA256", reference["locale_available_order_manifest_sha256"]),
        ("LOCALE_PUBLIC_ORACLE_RECORD_COUNT", reference["locale_public_oracle_record_count"]),
        ("LOCALE_PUBLIC_ORACLE_OUTPUT_SHA256", reference["locale_public_oracle_output_sha256"]),
        ("LOCALE_DISPLAY_ORACLE_RECORD_COUNT", reference["locale_logical_display_record_count"]),
        ("LOCALE_DISPLAY_COMPACT_RECORD_COUNT", reference["locale_compact_record_count"]),
        ("LOCALE_DISPLAY_PROFILE_COUNT", reference["locale_compact_profile_count"]),
        ("LOCALE_DISPLAY_KEY_COUNT", reference["locale_compact_key_count"]),
        ("LOCALE_DISPLAY_VALUE_COUNT", reference["locale_compact_value_count"]),
        ("LOCALE_DISPLAY_COMPACT_SNAPSHOT_BYTES", reference["locale_compact_snapshot_bytes"]),
        ("LOCALE_DISPLAY_COMPACT_FORMAT_VERSION", reference["locale_compact_format_version"]),
        ("LOCALE_DISPLAY_COMPACT_SHA256", reference["locale_compact_snapshot_sha256"]),
        ("LOCALE_DISPLAY_GENERATED_SOURCE_BYTES", reference["locale_generated_rust_bytes"]),
    ):
        check_rust_const(locale_source, name, expected, "JDK locale artifact")

    order_path = relative_path(root, reference.get("locale_available_order_path"), "JDK locale order path")
    check_artifact(
        order_path,
        label="JDK available-locale order",
        expected_bytes=require_int(reference, "locale_available_order_bytes", "JDK order bytes"),
        minimum_bytes=require_int(reference, "locale_available_order_min_bytes", "JDK order min bytes"),
        maximum_bytes=require_int(reference, "locale_available_order_max_bytes", "JDK order max bytes"),
        expected_sha256=require_sha(reference, "locale_available_order_manifest_sha256", "JDK order SHA-256"),
        policy="exact",
    )
    compact_path = relative_path(root, reference.get("locale_compact_snapshot_path"), "JDK locale compact path")
    compact_limit = require_int(reference, "locale_compact_snapshot_limit_bytes", "JDK compact limit bytes")
    compact_max = require_int(reference, "locale_compact_snapshot_max_bytes", "JDK compact max bytes")
    if compact_max > compact_limit:
        raise VerificationError("JDK compact locale snapshot: maximum exceeds its size limit")
    check_artifact(
        compact_path,
        label="JDK compact locale snapshot",
        expected_bytes=require_int(reference, "locale_compact_snapshot_bytes", "JDK compact bytes"),
        minimum_bytes=require_int(reference, "locale_compact_snapshot_min_bytes", "JDK compact min bytes"),
        maximum_bytes=compact_max,
        expected_sha256=require_sha(reference, "locale_compact_snapshot_sha256", "JDK compact SHA-256"),
        policy=require_string(reference, "locale_compact_snapshot_size_policy", "JDK compact policy"),
    )
    compact = compact_path.read_bytes()
    if len(compact) < 8 or compact[:4] != b"SCLD":
        raise VerificationError("JDK compact locale snapshot: invalid or truncated header")
    format_version = int.from_bytes(compact[4:8], "little")
    if format_version != reference["locale_compact_format_version"]:
        raise VerificationError("JDK compact locale snapshot: format version changed")

    if require_string(reference, "locale_public_oracle_size_policy", "JDK public oracle size policy") != "exact-record-count-and-digest":
        raise VerificationError("JDK public semantic oracle: unsupported size policy")
    expected_artifacts = {
        reference["unicode_corpus_path"],
        reference["locale_available_order_path"],
        reference["locale_compact_snapshot_path"],
    }
    check_exact_globs(root, provenance, expected_artifacts, "JDK")
    return manifest, reference


def check_markdown_license_metadata(
    reference: dict[str, Any], label: str
) -> list[dict[str, Any]]:
    require_string(reference, "source_kind", f"{label}.source_kind")
    if reference["source_kind"] != "immutable-git":
        raise VerificationError(f"{label}: source_kind must be immutable-git")
    require_string(reference, "repository", f"{label}.repository")
    require_string(reference, "version", f"{label}.version")
    require_revision(reference, "revision", f"{label}.revision")
    require_string(reference, "corpus_path", f"{label}.corpus_path")
    require_string(reference, "license", f"{label}.license")
    source_sha = require_sha(reference, "source_corpus_sha256", f"{label}.source_corpus_sha256")
    del source_sha
    for key in ("source_corpus_bytes", "source_corpus_min_bytes", "source_corpus_max_bytes", "expected_case_count"):
        require_int(reference, key, f"{label}.{key}")
    license_files = reference.get("license_files")
    if not isinstance(license_files, list) or not license_files:
        raise VerificationError(f"{label}: required license_files are missing")
    normalized: list[dict[str, Any]] = []
    for index, license_file in enumerate(license_files):
        if not isinstance(license_file, dict):
            raise VerificationError(f"{label}.license_files[{index}] is not a table")
        path = license_file.get("path")
        if not isinstance(path, str) or not path or Path(path).is_absolute() or ".." in Path(path).parts:
            raise VerificationError(f"{label}.license_files[{index}].path is unsafe")
        normalized.append(
            {
                "path": path,
                "bytes": require_int(license_file, "bytes", f"{label}.license_files[{index}].bytes"),
                "sha256": require_sha(license_file, "sha256", f"{label}.license_files[{index}].sha256"),
            }
        )
    markers = reference.get("notice_markers")
    if not isinstance(markers, list) or not markers or not all(
        isinstance(marker, str) and marker for marker in markers
    ):
        raise VerificationError(f"{label}: notice_markers must be non-empty")
    if "extracted_corpus_path" in reference:
        for key in (
            "extraction_script_path",
            "preparation_script_path",
            "differential_harness_path",
            "semantic_result_id",
            "extracted_corpus_path",
            "baseline_path",
        ):
            require_string(reference, key, f"{label}.{key}")
        for key in (
            "extraction_script_source_sha256",
            "preparation_script_source_sha256",
            "differential_harness_source_sha256",
            "extracted_corpus_sha256",
            "baseline_sha256",
        ):
            require_sha(reference, key, f"{label}.{key}")
        for key in (
            "extracted_corpus_bytes",
            "extracted_corpus_min_bytes",
            "extracted_corpus_max_bytes",
            "baseline_bytes",
            "baseline_case_count",
        ):
            require_int(reference, key, f"{label}.{key}")
        expected_result = reference.get("expected_result")
        if not isinstance(expected_result, dict):
            raise VerificationError(f"{label}: expected_result must be a table")
        for key in ("total", "pass", "known_mismatch", "unsupported", "harness_error", "new_mismatch"):
            require_int(expected_result, key, f"{label}.expected_result.{key}")
    return normalized


def check_json_artifact(
    path: Path, *, label: str, expected_bytes: int, minimum_bytes: int, maximum_bytes: int, expected_sha256: str
) -> Any:
    check_artifact(
        path,
        label=label,
        expected_bytes=expected_bytes,
        minimum_bytes=minimum_bytes,
        maximum_bytes=maximum_bytes,
        expected_sha256=expected_sha256,
        policy="exact",
    )
    return read_json(path)


def check_markdown_static(root: Path, manifest_path: Path) -> tuple[dict[str, Any], dict[str, Any]]:
    manifest = read_toml(manifest_path)
    provenance = contract_table(root, manifest, "Markdown")
    refs: dict[str, dict[str, Any]] = {}
    for name in ("commonmark", "cmark", "cmark_gfm"):
        reference = manifest.get(name)
        if not isinstance(reference, dict):
            raise VerificationError(f"Markdown: missing [{name}] table")
        refs[name] = reference
    for name in ("commonmark", "cmark", "cmark_gfm"):
        reference = refs[name]
        check_markdown_license_metadata(reference, f"Markdown.{name}")
        if "extracted_corpus_path" in reference:
            for path_key, sha_key, label in (
                ("extraction_script_path", "extraction_script_source_sha256", "Markdown extraction script"),
                ("preparation_script_path", "preparation_script_source_sha256", "Markdown preparation script"),
                ("differential_harness_path", "differential_harness_source_sha256", "Markdown differential harness"),
            ):
                check_script_identity(root, reference, path_key, sha_key, label)
        for marker in reference["notice_markers"]:
            notice = relative_path(root, provenance["notice_path"], "Markdown notice path")
            if marker not in notice.read_text(encoding="utf-8"):
                raise VerificationError(f"Markdown.{name}: NOTICE marker is missing: {marker!r}")

        if "extracted_corpus_path" not in reference:
            continue
        baseline_path = relative_path(root, reference["baseline_path"], f"Markdown.{name} baseline path")
        baseline = check_json_artifact(
            baseline_path,
            label=f"Markdown.{name} baseline",
            expected_bytes=reference["baseline_bytes"],
            minimum_bytes=reference["baseline_bytes"],
            maximum_bytes=reference["baseline_bytes"],
            expected_sha256=reference["baseline_sha256"],
        )
        if not isinstance(baseline, dict) or not isinstance(baseline.get("reference"), dict):
            raise VerificationError(f"Markdown.{name}: malformed baseline")
        parser_name = "cmark" if name == "commonmark" else "cmark_gfm"
        if baseline["reference"].get("version") != reference["version"]:
            raise VerificationError(f"Markdown.{name}: baseline version drift")
        if baseline["reference"].get("revision") != reference["revision"]:
            raise VerificationError(f"Markdown.{name}: baseline revision drift")
        if baseline["reference"].get("parser_revision") != refs[parser_name]["revision"]:
            raise VerificationError(f"Markdown.{name}: baseline parser revision drift")
        cases = baseline.get("cases")
        if not isinstance(cases, dict) or len(cases) != reference["baseline_case_count"]:
            raise VerificationError(f"Markdown.{name}: baseline case count changed")

        corpus_path = relative_path(root, reference["extracted_corpus_path"], f"Markdown.{name} corpus path")
        corpus = check_json_artifact(
            corpus_path,
            label=f"Markdown.{name} extracted corpus",
            expected_bytes=reference["extracted_corpus_bytes"],
            minimum_bytes=reference["extracted_corpus_min_bytes"],
            maximum_bytes=reference["extracted_corpus_max_bytes"],
            expected_sha256=reference["extracted_corpus_sha256"],
        )
        if not isinstance(corpus, list) or len(corpus) != reference["expected_case_count"]:
            raise VerificationError(f"Markdown.{name}: corpus case count changed")

    expected_paths = {
        refs["commonmark"]["extracted_corpus_path"],
        refs["cmark_gfm"]["extracted_corpus_path"],
        refs["commonmark"]["baseline_path"],
        refs["cmark_gfm"]["baseline_path"],
    }
    check_exact_globs(root, provenance, expected_paths, "Markdown")
    return manifest, refs


def git_output(cwd: Path, *args: str) -> str:
    try:
        result = subprocess.run(
            ["git", *args], cwd=cwd, check=True, capture_output=True, text=True
        )
    except (OSError, subprocess.CalledProcessError) as error:
        raise VerificationError(f"git verification failed in {cwd}: {error}") from error
    return result.stdout.strip()


def normalized_remote(value: str) -> str:
    return value.removesuffix("/").removesuffix(".git")


def check_markdown_sources(
    root: Path, refs: dict[str, Any], paths_path: Path, extracted: dict[str, Path]
) -> None:
    paths = read_json(paths_path)
    if not isinstance(paths, dict):
        raise VerificationError("Markdown preparation paths are not a JSON object")
    for name, reference in refs.items():
        root_key = f"{name}_root"
        spec_key = f"{name}_spec"
        checkout = external_path(paths.get(root_key), f"Markdown paths.{root_key}")
        spec = external_path(paths.get(spec_key), f"Markdown paths.{spec_key}")
        if not checkout.is_dir() or not spec.is_file():
            raise VerificationError(f"Markdown.{name}: prepared checkout/spec is missing")
        if git_output(checkout, "rev-parse", "HEAD") != reference["revision"]:
            raise VerificationError(f"Markdown.{name}: immutable revision mismatch")
        remote = git_output(checkout, "config", "--get", "remote.origin.url")
        if normalized_remote(remote) != normalized_remote(reference["repository"]):
            raise VerificationError(f"Markdown.{name}: source repository mismatch")
        expected_spec = checkout / reference["corpus_path"]
        if spec.resolve() != expected_spec.resolve():
            raise VerificationError(f"Markdown.{name}: corpus path escaped the pinned checkout")
        check_artifact(
            spec,
            label=f"Markdown.{name} pinned source corpus",
            expected_bytes=reference["source_corpus_bytes"],
            minimum_bytes=reference["source_corpus_min_bytes"],
            maximum_bytes=reference["source_corpus_max_bytes"],
            expected_sha256=reference["source_corpus_sha256"],
            policy="exact",
        )
        for license_file in check_markdown_license_metadata(reference, f"Markdown.{name}"):
            check_artifact(
                checkout / license_file["path"],
                label=f"Markdown.{name} source {license_file['path']}",
                expected_bytes=license_file["bytes"],
                minimum_bytes=license_file["bytes"],
                maximum_bytes=license_file["bytes"],
                expected_sha256=license_file["sha256"],
                policy="exact",
            )
    for name, path in extracted.items():
        reference_name = "commonmark" if name == "commonmark" else "cmark_gfm"
        reference = refs[reference_name]
        extracted_json = check_json_artifact(
            path,
            label=f"Markdown.{reference_name} regenerated corpus",
            expected_bytes=reference["extracted_corpus_bytes"],
            minimum_bytes=reference["extracted_corpus_min_bytes"],
            maximum_bytes=reference["extracted_corpus_max_bytes"],
            expected_sha256=reference["extracted_corpus_sha256"],
        )
        if not isinstance(extracted_json, list) or len(extracted_json) != reference["expected_case_count"]:
            raise VerificationError(f"Markdown.{reference_name}: regenerated case count changed")
        checked_in = relative_path(root, reference["extracted_corpus_path"], "Markdown corpus path")
        if path.read_bytes() != checked_in.read_bytes():
            raise VerificationError(f"Markdown.{reference_name}: regenerated corpus differs byte-for-byte")


def check_markdown_report(root: Path, refs: dict[str, Any], report_path: Path) -> None:
    report = read_json(report_path)
    if not isinstance(report, dict) or report.get("schema_version") != 1:
        raise VerificationError("Markdown differential report has an unsupported schema")
    if report.get("errors") != []:
        raise VerificationError("Markdown differential report contains errors")
    suites = report.get("suites")
    if not isinstance(suites, list) or len(suites) != 2:
        raise VerificationError("Markdown differential report does not contain both suites")
    by_name = {suite.get("name"): suite for suite in suites if isinstance(suite, dict)}
    for report_name, reference_name in (("CommonMark", "commonmark"), ("GFM", "cmark_gfm")):
        suite = by_name.get(report_name)
        if not isinstance(suite, dict):
            raise VerificationError(f"Markdown report is missing {report_name}")
        reference = refs[reference_name]
        if suite.get("reference_version") != reference["version"] or suite.get("reference_revision") != reference["revision"]:
            raise VerificationError(f"Markdown {report_name}: report source identity drift")
        cases = suite.get("cases")
        if not isinstance(cases, list):
            raise VerificationError(f"Markdown {report_name}: report cases are malformed")
        counts = {"total": len(cases), "pass": 0, "known_mismatch": 0, "unsupported": 0, "harness_error": 0, "new_mismatch": 0}
        for case in cases:
            if not isinstance(case, dict):
                raise VerificationError(f"Markdown {report_name}: malformed case result")
            result = case.get("result")
            key = {"PASS": "pass", "KNOWN_MISMATCH": "known_mismatch", "UNSUPPORTED": "unsupported", "HARNESS_ERROR": "harness_error"}.get(result)
            if key is None:
                raise VerificationError(f"Markdown {report_name}: unknown case result {result!r}")
            counts[key] += 1
            if case.get("new_mismatch"):
                counts["new_mismatch"] += 1
        expected = reference["expected_result"]
        if counts != expected:
            raise VerificationError(f"Markdown {report_name}: recomputed result {counts} != {expected}")
        if suite.get("baseline_errors") != []:
            raise VerificationError(f"Markdown {report_name}: baseline errors are present")


def check_jdk_oracle(reference: dict[str, Any], path: Path) -> None:
    if not path.is_file():
        raise VerificationError(f"JDK public semantic oracle is missing: {path}")
    raw = path.read_bytes()
    actual_sha = hashlib.sha256(raw).hexdigest()
    if actual_sha != reference["locale_public_oracle_output_sha256"]:
        raise VerificationError(
            "JDK public semantic oracle digest mismatch: "
            f"expected {reference['locale_public_oracle_output_sha256']}, got {actual_sha}"
        )
    lines = raw.splitlines()
    if not reference["locale_public_oracle_min_records"] <= len(lines) <= reference["locale_public_oracle_max_records"]:
        raise VerificationError("JDK public semantic oracle record bounds changed")
    if len(lines) != reference["locale_public_oracle_record_count"]:
        raise VerificationError("JDK public semantic oracle record count changed")
    for line_number, line in enumerate(lines, 1):
        if len(line.split(b"\t")) != 11:
            raise VerificationError(f"JDK public semantic oracle row {line_number} is malformed")


def check_jdk_archive(reference: dict[str, Any], archive: Path) -> None:
    if archive.name != reference["archive_filename"]:
        raise VerificationError("JDK archive filename does not match the manifest")
    check_artifact(
        archive,
        label="JDK reference archive",
        expected_bytes=reference["archive_bytes"],
        minimum_bytes=reference["archive_bytes"],
        maximum_bytes=reference["archive_bytes"],
        expected_sha256=reference["archive_sha256"],
        policy="exact",
    )


def verify_jdk_git_sources(root: Path, reference: dict[str, Any]) -> None:
    tag_refs = git_output(
        root,
        "ls-remote",
        reference["source_repository"],
        f"refs/tags/{reference['source_tag']}",
        f"refs/tags/{reference['source_tag']}^{{}}",
    ).splitlines()
    tag_values = {line.split("\t", 1)[1]: line.split("\t", 1)[0] for line in tag_refs if "\t" in line}
    actual_tag = tag_values.get(
        f"refs/tags/{reference['source_tag']}^{{}}",
        tag_values.get(f"refs/tags/{reference['source_tag']}"),
    )
    if actual_tag != reference["source_revision"]:
        raise VerificationError("JDK source tag does not resolve to the pinned source revision")
    with tempfile.TemporaryDirectory(prefix="scribium-jdk-source-proof-") as temporary:
        probe = Path(temporary)
        git_output(probe, "init", "--quiet")
        git_output(probe, "remote", "add", "origin", reference["build_source_repository"])
        git_output(probe, "fetch", "--quiet", "--depth=1", "origin", reference["build_source_revision"])
        actual_build = git_output(probe, "rev-parse", "FETCH_HEAD")
        if actual_build != reference["build_source_revision"]:
            raise VerificationError("JDK build source revision could not be proven")


def run_checked(root: Path, command: list[str], *, environment: dict[str, str] | None = None) -> None:
    try:
        subprocess.run(command, cwd=root, check=True, env=environment)
    except (OSError, subprocess.CalledProcessError) as error:
        raise VerificationError(f"independent verifier failed: {' '.join(command)}") from error


def run_jdk_dynamic(
    root: Path,
    reference: dict[str, Any],
    archive: Path,
    java: Path,
    oracle: Path | None,
    run_semantic: bool,
) -> None:
    check_jdk_archive(reference, archive)
    if oracle is not None:
        check_jdk_oracle(reference, oracle)
    run_checked(
        root,
        [
            sys.executable,
            str(relative_path(root, reference["locale_generator_path"], "JDK locale generator path")),
            "--java",
            str(java),
            "--archive",
            str(archive),
            "--check",
        ],
    )
    run_checked(
        root,
        [
            sys.executable,
            str(relative_path(root, reference["unicode_verifier_path"], "JDK Unicode verifier path")),
            "--java",
            str(java),
            "--archive",
            str(archive),
        ],
    )
    if run_semantic:
        if oracle is None:
            raise VerificationError("JDK semantic verification requires --locale-oracle")
        environment = os.environ.copy()
        environment["SCRIBIUM_JDK25_LOCALE_ORACLE"] = str(oracle)
        test_name = reference["locale_semantic_verifier_test"]
        run_checked(
            root,
            ["cargo", "test", "-p", "scribium-engine", "--locked", test_name, "--", "--exact"],
            environment=environment,
        )


def verify_workspace(
    *,
    root: Path = ROOT,
    jdk_manifest_path: Path | None = None,
    markdown_manifest_path: Path | None = None,
    markdown_paths: Path | None = None,
    commonmark_extracted: Path | None = None,
    gfm_extracted: Path | None = None,
    markdown_report: Path | None = None,
    jdk_archive: Path | None = None,
    jdk_java: Path | None = None,
    locale_oracle: Path | None = None,
    run_semantic: bool = False,
    verify_git_sources: bool = False,
) -> None:
    jdk_manifest, jdk_reference = check_jdk_static(
        root,
        jdk_manifest_path or root / "docs/compatibility/quarkdown/reference-jvm.toml",
    )
    _markdown_manifest, markdown_refs = check_markdown_static(
        root,
        markdown_manifest_path or root / "tests/compat/references.toml",
    )
    if verify_git_sources:
        verify_jdk_git_sources(root, jdk_reference)
    if (jdk_archive is None) != (jdk_java is None):
        raise VerificationError("--jdk-archive and --jdk-java must be supplied together")
    if jdk_archive is not None and jdk_java is not None:
        run_jdk_dynamic(root, jdk_reference, jdk_archive, jdk_java, locale_oracle, run_semantic)
    elif locale_oracle is not None or run_semantic:
        raise VerificationError("locale oracle/semantic verification requires JDK runtime arguments")
    if markdown_paths is not None:
        extracted = {}
        if commonmark_extracted is None or gfm_extracted is None:
            raise VerificationError("Markdown source verification requires both extracted corpora")
        extracted["commonmark"] = commonmark_extracted
        extracted["gfm"] = gfm_extracted
        check_markdown_sources(root, markdown_refs, markdown_paths, extracted)
    elif any(path is not None for path in (commonmark_extracted, gfm_extracted)):
        raise VerificationError("extracted Markdown corpora require --markdown-paths")
    if markdown_report is not None:
        check_markdown_report(root, markdown_refs, markdown_report)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--jdk-manifest", type=Path)
    parser.add_argument("--markdown-manifest", type=Path)
    parser.add_argument("--markdown-paths", type=Path)
    parser.add_argument("--commonmark-extracted", type=Path)
    parser.add_argument("--gfm-extracted", type=Path)
    parser.add_argument("--markdown-report", type=Path)
    parser.add_argument("--jdk-archive", type=Path)
    parser.add_argument("--jdk-java", type=Path)
    parser.add_argument("--locale-oracle", type=Path)
    parser.add_argument("--run-semantic", action="store_true")
    parser.add_argument("--verify-git-sources", action="store_true")
    args = parser.parse_args()
    try:
        verify_workspace(
            root=args.root,
            jdk_manifest_path=args.jdk_manifest,
            markdown_manifest_path=args.markdown_manifest,
            markdown_paths=args.markdown_paths,
            commonmark_extracted=args.commonmark_extracted,
            gfm_extracted=args.gfm_extracted,
            markdown_report=args.markdown_report,
            jdk_archive=args.jdk_archive,
            jdk_java=args.jdk_java,
            locale_oracle=args.locale_oracle,
            run_semantic=args.run_semantic,
            verify_git_sources=args.verify_git_sources,
        )
    except VerificationError as error:
        print(f"reference provenance verification failed: {error}", file=sys.stderr)
        return 1
    print("reference provenance verification: PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
