#!/usr/bin/env python3
"""Fail-closed Rust CodeQL change-impact classifier.

Consumes GitHub pull-request metadata plus the fully paginated pull-request
files response and emits a machine-readable decision suitable for CI.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path, PurePosixPath
from typing import Any, Iterable

ALLOWED_STATUSES = {
    "added",
    "modified",
    "removed",
    "renamed",
    "copied",
    "changed",
    "unchanged",
}

RUST_CODEQL_POLICY_PATHS = {
    ".github/workflows/codeql.yml",
    ".github/gate-policy.toml",
    "tools/ci/codeql_scope.py",
    "tools/ci/hardening_drift_audit.py",
}


def _well_formed_path(value: Any) -> bool:
    if not isinstance(value, str) or not value:
        return False
    path = PurePosixPath(value)
    return not path.is_absolute() and ".." not in path.parts


def rust_input_path(path: Any) -> bool:
    """Return True for known Rust CodeQL analysis/policy inputs.

    Malformed paths deliberately fail closed and are therefore Rust-impacting.
    """
    if not _well_formed_path(path):
        return True

    value = str(path)
    name = PurePosixPath(value).name

    if value.endswith(".rs"):
        return True
    if name in {"Cargo.toml", "Cargo.lock", "build.rs", "rust-toolchain", "rust-toolchain.toml"}:
        return True
    if value == ".cargo" or value.startswith(".cargo/"):
        return True
    if value == ".github/codeql" or value.startswith(".github/codeql/"):
        return True
    if value in RUST_CODEQL_POLICY_PATHS:
        return True

    return False


def flatten_pages(value: Any) -> list[dict[str, Any]]:
    """Flatten gh api --paginate --slurp output.

    Any unexpected shape is rejected so callers can fail closed.
    """
    if not isinstance(value, list):
        raise ValueError("paginated response must be a list")

    if all(isinstance(item, dict) for item in value):
        return list(value)

    if not all(isinstance(page, list) for page in value):
        raise ValueError("paginated response must be a list of pages")

    flattened: list[dict[str, Any]] = []
    for page in value:
        if not all(isinstance(item, dict) for item in page):
            raise ValueError("each page must contain file records")
        flattened.extend(page)
    return flattened


def classify(records: Any, expected_count: Any) -> tuple[bool, str]:
    """Classify a pull request.

    True means Rust CodeQL must run. Unknown or malformed evidence also returns
    True by design.
    """
    if not isinstance(expected_count, int) or expected_count < 0:
        return True, "invalid-expected-count"
    if not isinstance(records, list):
        return True, "invalid-records"
    if len(records) != expected_count:
        return True, "count-mismatch"

    for record in records:
        if not isinstance(record, dict):
            return True, "malformed-record"

        status = record.get("status")
        filename = record.get("filename")
        previous = record.get("previous_filename")

        if status not in ALLOWED_STATUSES:
            return True, "unknown-status"
        if not _well_formed_path(filename):
            return True, "malformed-filename"

        if rust_input_path(filename):
            return True, f"rust-input:{filename}"

        if status == "renamed":
            if not _well_formed_path(previous):
                return True, "malformed-previous-filename"
            if rust_input_path(previous):
                return True, f"rust-input-previous:{previous}"
        elif previous not in (None, ""):
            return True, "unexpected-previous-filename"

    return False, "no-rust-input-change"


def classify_payload(meta: Any, pages: Any) -> dict[str, Any]:
    """Classify GitHub PR metadata and PR-files payloads fail-closed."""
    try:
        if not isinstance(meta, dict):
            raise ValueError("pull-request metadata must be an object")
        expected_count = meta.get("changed_files")
        records = flatten_pages(pages)
        rust_impact, reason = classify(records, expected_count)
    except (TypeError, ValueError) as exc:
        rust_impact = True
        reason = f"metadata-error:{exc}"
        records = []

    languages = ["actions", "rust"] if rust_impact else ["actions"]
    return {
        "rust_impact": rust_impact,
        "reason": reason,
        "languages": languages,
        "observed_file_count": len(records),
        "expected_file_count": meta.get("changed_files") if isinstance(meta, dict) else None,
    }


def _read_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def _write_github_output(path: Path, result: dict[str, Any]) -> None:
    with path.open("a", encoding="utf-8") as stream:
        stream.write(f"rust_impact={str(result['rust_impact']).lower()}\n")
        stream.write("languages=" + json.dumps(result["languages"], separators=(",", ":")) + "\n")
        stream.write(f"reason={result['reason']}\n")


def main(argv: Iterable[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--meta", type=Path, required=True)
    parser.add_argument("--pages", type=Path, required=True)
    parser.add_argument("--output-json", type=Path)
    parser.add_argument("--github-output", type=Path)
    args = parser.parse_args(list(argv) if argv is not None else None)

    try:
        meta = _read_json(args.meta)
        pages = _read_json(args.pages)
        result = classify_payload(meta, pages)
    except (OSError, json.JSONDecodeError) as exc:
        result = {
            "rust_impact": True,
            "reason": f"input-error:{exc.__class__.__name__}",
            "languages": ["actions", "rust"],
            "observed_file_count": 0,
            "expected_file_count": None,
        }

    encoded = json.dumps(result, indent=2, sort_keys=True)
    print(encoded)

    if args.output_json:
        args.output_json.write_text(encoded + "\n", encoding="utf-8")
    if args.github_output:
        _write_github_output(args.github_output, result)

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
