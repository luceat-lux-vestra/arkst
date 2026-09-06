#!/usr/bin/env python3
from __future__ import annotations

import re
import sys
from pathlib import Path


class LockedCargoPolicyError(RuntimeError):
    pass


CARGO_INVOCATION = re.compile(
    r"(?<![A-Za-z0-9_-])cargo\s+(?:\+\S+\s+)?(?P<subcommand>[A-Za-z0-9_-]+)(?P<args>.*)"
)
LOCKED_TOKEN = re.compile(r"(?:^|\s)--locked(?:\s|$)")
DEPENDENCY_RESOLVING = {
    "build",
    "check",
    "clippy",
    "doc",
    "metadata",
    "run",
    "test",
    "tree",
}
EXEMPT = {"fmt"}


def verify_ci_workflow(path: Path) -> None:
    text = path.read_text(encoding="utf-8")
    seen_dependency_commands = 0

    for line_number, raw in enumerate(text.splitlines(), start=1):
        match = CARGO_INVOCATION.search(raw)
        if match is None:
            continue

        subcommand = match.group("subcommand")
        args = match.group("args")
        if subcommand in EXEMPT:
            continue
        if subcommand not in DEPENDENCY_RESOLVING:
            raise LockedCargoPolicyError(
                f"{path.as_posix()}:{line_number}: unclassified cargo subcommand "
                f"{subcommand!r}; update the lockfile policy deliberately"
            )

        seen_dependency_commands += 1
        if LOCKED_TOKEN.search(args) is None:
            raise LockedCargoPolicyError(
                f"{path.as_posix()}:{line_number}: cargo {subcommand} must use --locked"
            )

    if seen_dependency_commands == 0:
        raise LockedCargoPolicyError(
            f"{path.as_posix()}: no dependency-resolving cargo commands found"
        )


def verify_repository(root: Path) -> None:
    verify_ci_workflow(root / ".github" / "workflows" / "ci.yml")


def main() -> int:
    try:
        verify_repository(Path("."))
    except (OSError, LockedCargoPolicyError) as exc:
        print(f"locked-cargo-policy error: {exc}", file=sys.stderr)
        return 1
    print("required CI Cargo commands are lockfile-fail-closed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
