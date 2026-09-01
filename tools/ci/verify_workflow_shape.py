#!/usr/bin/env python3
from __future__ import annotations

import re
import sys
from pathlib import Path


class WorkflowShapeError(RuntimeError):
    pass


def _indent(line: str) -> int:
    return len(line) - len(line.lstrip(" "))


def _code(line: str) -> str:
    return line.split("#", 1)[0].rstrip()


def _require_simple_mapping_key(rel: str, code: str, label: str) -> None:
    if not re.fullmatch(r"    [A-Za-z0-9_-]+:(?:\s*.*)?", code):
        raise WorkflowShapeError(f"{rel}: unsupported {label} syntax: {code.strip()!r}")


def _block_end(lines: list[str], start: int) -> int:
    for i in range(start + 1, len(lines)):
        raw = lines[i]
        if _code(raw).strip() and _indent(raw) == 0:
            return i
    return len(lines)


def verify_workflow(path: Path) -> None:
    lines = path.read_text(encoding="utf-8").splitlines()
    rel = path.as_posix()

    for line in lines:
        leading = line[: len(line) - len(line.lstrip())]
        if "\t" in leading:
            raise WorkflowShapeError(
                f"{rel}: tab-indented YAML is not supported by the gate verifier"
            )

    try:
        on_index = next(
            i
            for i, line in enumerate(lines)
            if _indent(line) == 0 and _code(line).strip().startswith("on:")
        )
        jobs_index = next(
            i
            for i, line in enumerate(lines)
            if _indent(line) == 0 and _code(line).strip() == "jobs:"
        )
    except StopIteration as exc:
        raise WorkflowShapeError(f"{rel}: missing top-level on:/jobs: block") from exc

    on_line = _code(lines[on_index]).strip()
    if on_line != "on:":
        raise WorkflowShapeError(
            f"{rel}: inline top-level on: syntax is not supported by the gate verifier"
        )

    on_end = _block_end(lines, on_index)
    for raw in lines[on_index + 1 : on_end]:
        code = _code(raw)
        stripped = code.strip()
        if not stripped:
            continue
        indent = _indent(raw)
        if indent == 2:
            if not re.fullmatch(r"  [A-Za-z0-9_-]+:\s*", code):
                raise WorkflowShapeError(
                    f"{rel}: unsupported top-level trigger syntax: {stripped!r}"
                )
        elif indent == 4:
            if stripped.startswith("- "):
                if not re.fullmatch(r"    - [A-Za-z0-9_-]+:(?:\s*.*)?", code):
                    raise WorkflowShapeError(
                        f"{rel}: unsupported trigger-list syntax: {stripped!r}"
                    )
            else:
                _require_simple_mapping_key(rel, code, "trigger mapping")

    jobs_end = _block_end(lines, jobs_index)
    for raw in lines[jobs_index + 1 : jobs_end]:
        code = _code(raw)
        stripped = code.strip()
        if not stripped:
            continue
        indent = _indent(raw)
        if indent == 2:
            if not re.fullmatch(r"  [A-Za-z0-9_-]+:\s*", code):
                raise WorkflowShapeError(
                    f"{rel}: unsupported job-key syntax: {stripped!r}"
                )
        elif indent == 4:
            _require_simple_mapping_key(rel, code, "job mapping")


def verify_repository(root: Path) -> None:
    workflows = root / ".github" / "workflows"
    paths = sorted([*workflows.glob("*.yml"), *workflows.glob("*.yaml")])
    if not paths:
        raise WorkflowShapeError("no GitHub Actions workflows found")
    for path in paths:
        verify_workflow(path)


def main() -> int:
    try:
        verify_repository(Path("."))
    except (OSError, WorkflowShapeError) as exc:
        print(f"workflow-shape error: {exc}", file=sys.stderr)
        return 1
    print("workflow YAML shape verified")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
