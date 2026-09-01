#!/usr/bin/env python3
from __future__ import annotations

import re
import sys

_ALLOWED = re.compile(r"[A-Za-z0-9][A-Za-z0-9._/+'-]*\Z")


class TagValidationError(ValueError):
    pass


def validate_tag(tag: str) -> None:
    encoded = tag.encode("utf-8")
    if not 1 <= len(encoded) <= 128:
        raise TagValidationError("observed tag must be 1..128 UTF-8 bytes")
    if _ALLOWED.fullmatch(tag) is None:
        raise TagValidationError("observed tag contains unsupported characters")
    if ".." in tag or "//" in tag or tag.endswith(("/", ".", ".lock")):
        raise TagValidationError("observed tag is not an accepted ref-like release tag")


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: validate_upstream_tag.py TAG", file=sys.stderr)
        return 2
    try:
        validate_tag(sys.argv[1])
    except TagValidationError as exc:
        print(f"invalid upstream tag: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
