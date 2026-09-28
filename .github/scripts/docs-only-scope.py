#!/usr/bin/env python3
"""Fail-closed classifier for Arkst's documentation-only CI fast path."""

from __future__ import annotations

import argparse
import sys
import tomllib
from dataclasses import dataclass
from pathlib import Path, PurePosixPath


SIMPLE_STATUSES = frozenset({"added", "modified", "removed"})
MOVE_STATUSES = frozenset({"renamed", "copied"})


@dataclass(frozen=True)
class ScopePolicy:
    exact_paths: frozenset[str]
    markdown_prefixes: tuple[str, ...]
    excluded_prefixes: tuple[str, ...]


@dataclass(frozen=True)
class Change:
    status: str
    filename: str
    previous_filename: str = ""


def _safe_path(path: str) -> bool:
    if not path or path.startswith("/") or "\\" in path:
        return False
    parts = PurePosixPath(path).parts
    return bool(parts) and all(part not in {"", ".", ".."} for part in parts)


def load_policy(path: Path) -> ScopePolicy:
    with path.open("rb") as stream:
        raw = tomllib.load(stream).get("docs_only_fast_path")

    if not isinstance(raw, dict):
        raise ValueError("missing [docs_only_fast_path] policy")

    exact = raw.get("exact_paths")
    prefixes = raw.get("markdown_prefixes")
    excluded = raw.get("excluded_prefixes", [])

    if not isinstance(exact, list) or not exact:
        raise ValueError("docs_only_fast_path.exact_paths must be a non-empty list")
    if not isinstance(prefixes, list) or not prefixes:
        raise ValueError("docs_only_fast_path.markdown_prefixes must be a non-empty list")
    if not isinstance(excluded, list):
        raise ValueError("docs_only_fast_path.excluded_prefixes must be a list")

    for value in [*exact, *prefixes, *excluded]:
        if not isinstance(value, str) or not _safe_path(value.rstrip("/")):
            raise ValueError(f"invalid docs-only path policy entry: {value!r}")

    if len(set(exact)) != len(exact):
        raise ValueError("docs_only_fast_path.exact_paths contains duplicates")
    if len(set(prefixes)) != len(prefixes):
        raise ValueError("docs_only_fast_path.markdown_prefixes contains duplicates")
    if len(set(excluded)) != len(excluded):
        raise ValueError("docs_only_fast_path.excluded_prefixes contains duplicates")
    if any(not prefix.endswith("/") for prefix in [*prefixes, *excluded]):
        raise ValueError("docs-only prefixes must end with '/'")
    if any("/" in path for path in exact):
        raise ValueError("exact docs-only paths must remain repository-root files")

    return ScopePolicy(
        exact_paths=frozenset(exact),
        markdown_prefixes=tuple(prefixes),
        excluded_prefixes=tuple(excluded),
    )


def is_allowed_path(path: str, policy: ScopePolicy) -> bool:
    if not _safe_path(path):
        return False
    if path in policy.exact_paths:
        return True
    if any(path.startswith(prefix) for prefix in policy.excluded_prefixes):
        return False
    return path.endswith(".md") and any(
        path.startswith(prefix) for prefix in policy.markdown_prefixes
    )


def is_docs_only_change(change: Change, policy: ScopePolicy) -> bool:
    if change.status in SIMPLE_STATUSES:
        return not change.previous_filename and is_allowed_path(change.filename, policy)
    if change.status in MOVE_STATUSES:
        return (
            bool(change.previous_filename)
            and is_allowed_path(change.filename, policy)
            and is_allowed_path(change.previous_filename, policy)
        )
    return False


def classify(changes: list[Change], expected_count: int, policy: ScopePolicy) -> bool:
    if expected_count <= 0 or not changes or len(changes) != expected_count:
        return False
    if len(set(changes)) != len(changes):
        return False
    return all(is_docs_only_change(change, policy) for change in changes)


def parse_record(line: str) -> Change | None:
    fields = line.rstrip("\n").split("\t")
    if len(fields) != 3:
        return None
    status, filename, previous_filename = fields
    if not status or not filename:
        return None
    return Change(status=status, filename=filename, previous_filename=previous_filename)


def self_test(policy: ScopePolicy) -> None:
    for path in policy.exact_paths:
        assert is_allowed_path(path, policy), path
    for prefix in policy.markdown_prefixes:
        sample = f"{prefix}guide.md"
        if not any(sample.startswith(excluded) for excluded in policy.excluded_prefixes):
            assert is_allowed_path(sample, policy), sample
    for prefix in policy.excluded_prefixes:
        assert not is_allowed_path(f"{prefix}policy.md", policy)

    positive = [
        [Change("modified", "README.md")],
        [Change("added", "docs/architecture/overview.md")],
        [Change("removed", "docs/ROADMAP.md")],
        [Change("renamed", "docs/new-name.md", "docs/old-name.md")],
        [Change("copied", "docs/copy.md", "docs/source.md")],
    ]
    negative = [
        [],
        [Change("modified", ".github/pull_request_template.md")],
        [Change("modified", ".github/gate-policy.toml")],
        [Change("modified", "Cargo.toml")],
        [Change("modified", "LICENSE")],
        [Change("modified", "NOTICE")],
        [Change("modified", "examples/demo.md")],
        [Change("modified", "fixtures/sample.md")],
        [Change("modified", "docs/architecture/diagram.yml")],
        [Change("modified", "docs/legal/CLEAN_ROOM_POLICY.md")],
        [Change("modified", "README.md"), Change("modified", "crates/arkst-core/src/lib.rs")],
        [Change("modified", "../README.md")],
        [Change("modified", "docs/../README.md")],
        [Change("modified", "docs/readme.MD")],
        [Change("renamed", "docs/runtime-moved-here.md", "crates/arkst-core/src/lib.rs")],
        [Change("renamed", "docs/new-name.md", "")],
        [Change("copied", "docs/copy.md", "Cargo.toml")],
        [Change("changed", "README.md")],
        [Change("modified", "README.md", "docs/old.md")],
    ]

    for changes in positive:
        assert classify(changes, len(changes), policy), changes
    for changes in negative:
        assert not classify(changes, len(changes), policy), changes

    one = [Change("modified", "README.md")]
    assert not classify(one, 2, policy)
    assert not classify(one + one, 2, policy)

    assert parse_record("modified\tREADME.md\t\n") == Change("modified", "README.md")
    assert parse_record("renamed\tdocs/new.md\tdocs/old.md\n") == Change(
        "renamed", "docs/new.md", "docs/old.md"
    )
    assert parse_record("broken\n") is None


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--policy", type=Path, required=True)
    parser.add_argument("--expected-count", type=int)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    try:
        policy = load_policy(args.policy)
    except (OSError, ValueError, tomllib.TOMLDecodeError) as error:
        print(f"invalid docs-only policy: {error}", file=sys.stderr)
        return 2

    if args.self_test:
        self_test(policy)
        print("docs-only scope self-test passed")
        return 0

    if args.expected_count is None:
        parser.error("--expected-count is required unless --self-test is used")

    changes: list[Change] = []
    for line in sys.stdin:
        change = parse_record(line)
        if change is None:
            print("false")
            return 0
        changes.append(change)

    print("true" if classify(changes, args.expected_count, policy) else "false")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
