#!/usr/bin/env python3
"""Extract embedded specification examples into a stable source corpus.

The upstream specifications use a deliberately small example format. This
script stores only the source, section, and example number; expected HTML is
not used as Scribium's oracle. The checked-in JSON is regenerated in CI from
the pinned upstream revision and compared byte-for-byte.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path


MARKER = "`" * 32 + " example"
CLOSER = "`" * 32


def extract(spec_path: Path, prefix: str) -> list[dict[str, object]]:
    section = ""
    state = "text"
    markdown: list[str] = []
    output: list[dict[str, object]] = []
    example = 0
    extensions: list[str] = []

    with spec_path.open(encoding="utf-8", newline="") as spec_file:
        lines = spec_file.readlines()

    for line in lines:
        stripped = line.strip()
        if stripped.startswith(MARKER):
            state = "markdown"
            markdown = []
            extensions = stripped[len(MARKER) :].split()
            continue
        if state == "output" and stripped == CLOSER:
            example += 1
            if "disabled" not in extensions:
                output.append(
                    {
                        "id": f"{prefix}-{example:04d}",
                        "number": example,
                        "section": section,
                        "markdown": "".join(markdown).replace("→", "\t"),
                        "extensions": extensions,
                    }
                )
            state = "text"
            continue
        if state == "markdown" and stripped == ".":
            state = "output"
            continue
        if state == "markdown":
            markdown.append(line)
            continue
        if state == "text" and line.startswith("#"):
            heading = line.lstrip("#").strip()
            if heading:
                section = heading

    if state != "text":
        raise ValueError(f"unterminated example in {spec_path}")
    if not output:
        raise ValueError(f"no examples found in {spec_path}")
    return output


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--spec", type=Path, required=True)
    parser.add_argument("--prefix", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    cases = extract(args.spec, args.prefix)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(
        json.dumps(cases, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )


if __name__ == "__main__":
    main()
