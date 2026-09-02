#!/usr/bin/env python3
"""Check out the exact external Markdown references declared in TOML."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import tomllib
from pathlib import Path


REFERENCE_NAMES = ("commonmark", "cmark", "cmark_gfm")
REVISION_RE = re.compile(r"^[0-9a-f]{40}$")


def run(*args: str, cwd: Path | None = None) -> str:
    return subprocess.check_output(args, cwd=cwd, text=True).strip()


def repository_relative_path(value: object, field: str) -> Path:
    if not isinstance(value, str) or not value:
        raise SystemExit(f"{field} must be a non-empty repository-relative path")
    path = Path(value)
    if path.is_absolute() or ".." in path.parts:
        raise SystemExit(f"{field} must stay inside the pinned checkout: {value!r}")
    return path


def preflight_reference(name: str, reference: object) -> tuple[dict[str, object], Path]:
    if not isinstance(reference, dict):
        raise SystemExit(f"{name} reference must be a table")
    if reference.get("source_kind") != "immutable-git":
        raise SystemExit(f"{name}.source_kind must be immutable-git")
    repository = reference.get("repository")
    if not isinstance(repository, str) or not repository.startswith("https://"):
        raise SystemExit(f"{name}.repository must be an HTTPS URL")
    revision = reference.get("revision")
    if not isinstance(revision, str) or REVISION_RE.fullmatch(revision) is None:
        raise SystemExit(f"{name}.revision must be a full lowercase 40-hex commit")
    corpus_path = repository_relative_path(reference.get("corpus_path"), f"{name}.corpus_path")
    return reference, corpus_path


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--config", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--github-output", type=Path)
    args = parser.parse_args()

    config = tomllib.loads(args.config.read_text(encoding="utf-8"))
    references: dict[str, tuple[dict[str, object], Path]] = {}
    for name in REFERENCE_NAMES:
        references[name] = preflight_reference(name, config.get(name))

    args.output_dir.mkdir(parents=True, exist_ok=True)
    paths: dict[str, str] = {}
    for name in REFERENCE_NAMES:
        reference, corpus_path = references[name]
        checkout = args.output_dir / name
        run("git", "clone", "--quiet", str(reference["repository"]), str(checkout))
        run("git", "checkout", "--quiet", str(reference["revision"]), cwd=checkout)
        actual = run("git", "rev-parse", "HEAD", cwd=checkout)
        if actual != reference["revision"]:
            raise SystemExit(
                f"{name} revision mismatch: expected {reference['revision']}, got {actual}"
            )
        paths[f"{name}_root"] = str(checkout)
        paths[f"{name}_spec"] = str(checkout / corpus_path)

    paths_path = args.output_dir / "paths.json"
    paths_path.write_text(json.dumps(paths, indent=2) + "\n", encoding="utf-8")
    if args.github_output:
        with args.github_output.open("a", encoding="utf-8") as output:
            for key, value in paths.items():
                output.write(f"{key}={value}\n")


if __name__ == "__main__":
    main()
