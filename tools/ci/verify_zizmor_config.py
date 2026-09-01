#!/usr/bin/env python3
from __future__ import annotations

import sys
from pathlib import Path


class ZizmorConfigError(RuntimeError):
    pass


EXPECTED = [
    "rules:",
    "dangerous-triggers:",
    "ignore:",
    "- ai-review.yml",
    "- pr-labeler.yml",
]


def normalized_config(path: Path) -> list[str]:
    lines: list[str] = []
    for raw in path.read_text(encoding="utf-8").splitlines():
        code = raw.split("#", 1)[0].strip()
        if code:
            lines.append(code)
    return lines


def verify_config(path: Path) -> None:
    actual = normalized_config(path)
    if actual != EXPECTED:
        raise ZizmorConfigError(
            "zizmor suppression policy drifted; only dangerous-triggers for "
            "ai-review.yml and pr-labeler.yml may be ignored"
        )


def main() -> int:
    try:
        verify_config(Path("zizmor.yml"))
    except (OSError, ZizmorConfigError) as exc:
        print(f"zizmor-config error: {exc}", file=sys.stderr)
        return 1
    print("zizmor suppression inventory verified")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
