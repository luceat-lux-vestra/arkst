#!/usr/bin/env python3
from __future__ import annotations

import re
import sys
from pathlib import Path


class WorkflowSecurityError(RuntimeError):
    pass


SHA_REF = re.compile(r"^[0-9a-f]{40}$")
UNTRUSTED_EXPRESSIONS = (
    "github.event.pull_request.",
    "github.event.issue.",
    "github.event.comment.",
    "github.event.release.",
    "github.event.workflow_run.",
    "github.head_ref",
    "github.event.inputs.",
    "inputs.",
)
ALLOWED_WRITE_PERMISSIONS = {
    ".github/workflows/ai-review.yml": {"pull-requests"},
    ".github/workflows/issue-labeler.yml": {"issues"},
    ".github/workflows/pr-labeler.yml": {"issues", "pull-requests"},
    ".github/workflows/upstream-quarkdown.yml": {"issues"},
}


def _indent(line: str) -> int:
    return len(line) - len(line.lstrip(" "))


def _code(line: str) -> str:
    return line.split("#", 1)[0].rstrip()


def _top_level_block(lines: list[str], key: str) -> tuple[int, int] | None:
    needle = f"{key}:"
    for index, line in enumerate(lines):
        if _indent(line) == 0 and _code(line).strip().startswith(needle):
            end = len(lines)
            for cursor in range(index + 1, len(lines)):
                if _code(lines[cursor]).strip() and _indent(lines[cursor]) == 0:
                    end = cursor
                    break
            return index, end
    return None


def _job_blocks(lines: list[str], rel: str) -> list[tuple[str, list[str]]]:
    jobs = _top_level_block(lines, "jobs")
    if jobs is None:
        raise WorkflowSecurityError(f"{rel}: missing jobs block")
    _, end = jobs
    result: list[tuple[str, list[str]]] = []
    cursor = jobs[0] + 1
    while cursor < end:
        code = _code(lines[cursor])
        match = re.fullmatch(r"  ([A-Za-z0-9_-]+):\s*", code)
        if not match:
            cursor += 1
            continue
        job_id = match.group(1)
        block_end = end
        for candidate in range(cursor + 1, end):
            candidate_code = _code(lines[candidate])
            if candidate_code.strip() and _indent(lines[candidate]) == 2:
                block_end = candidate
                break
        result.append((job_id, lines[cursor:block_end]))
        cursor = block_end
    return result


def _executable_blocks(lines: list[str]) -> list[tuple[str, str]]:
    blocks: list[tuple[str, str]] = []
    index = 0
    while index < len(lines):
        code = _code(lines[index])
        scalar = re.match(r"^(\s*)(?:-\s+)?(run|script):\s*(.+)$", code)
        if scalar and scalar.group(3).strip() not in {"|", ">", "|-", ">-"}:
            blocks.append((scalar.group(2), scalar.group(3)))
            index += 1
            continue
        block = re.match(r"^(\s*)(?:-\s+)?(run|script):\s*[|>]-?\s*$", code)
        if not block:
            index += 1
            continue
        base = len(block.group(1))
        body: list[str] = []
        index += 1
        while index < len(lines):
            if _code(lines[index]).strip() and _indent(lines[index]) <= base:
                break
            body.append(lines[index])
            index += 1
        blocks.append((block.group(2), "\n".join(body)))
    return blocks


def _check_write_permissions(
    lines: list[str], rel: str, allowed_writes: set[str], base_indent: int, scope: str
) -> None:
    prefix = " " * base_indent
    for index, raw in enumerate(lines):
        code = _code(raw)
        if not re.fullmatch(rf"{re.escape(prefix)}permissions:(?:\s*.*)?", code):
            continue
        stripped = code.strip()
        if stripped == "permissions: write-all":
            raise WorkflowSecurityError(f"{rel}: {scope} permissions: write-all is forbidden")
        if stripped in {"permissions: {}", "permissions: read-all"}:
            continue
        if stripped != "permissions:":
            raise WorkflowSecurityError(f"{rel}: unsupported {scope} permissions syntax: {stripped}")
        for candidate in lines[index + 1 :]:
            candidate_code = _code(candidate)
            if candidate_code.strip() and _indent(candidate) <= base_indent:
                break
            match = re.fullmatch(
                rf"{' ' * (base_indent + 2)}([A-Za-z-]+):\s*write\s*", candidate_code
            )
            if match and match.group(1) not in allowed_writes:
                raise WorkflowSecurityError(
                    f"{rel}: unapproved {scope} write permission: {match.group(1)}"
                )


def _step_list_indent(lines: list[str], index: int) -> int:
    current = _code(lines[index])
    current_indent = _indent(lines[index])
    if current.lstrip().startswith("- "):
        return current_indent

    for previous in range(index - 1, -1, -1):
        code = _code(lines[previous])
        if not code.strip():
            continue
        indent = _indent(lines[previous])
        if indent < current_indent and code.lstrip().startswith("- "):
            return indent
        if indent < current_indent and indent <= 4:
            break

    raise WorkflowSecurityError("unable to resolve workflow step boundary")


def verify_workflow(path: Path, root: Path) -> None:
    rel = path.relative_to(root).as_posix()
    lines = path.read_text(encoding="utf-8").splitlines()
    text = "\n".join(lines)
    allowed_writes = ALLOWED_WRITE_PERMISSIONS.get(rel, set())

    permissions = _top_level_block(lines, "permissions")
    if permissions is None:
        raise WorkflowSecurityError(f"{rel}: explicit top-level permissions are required")
    _check_write_permissions(
        lines[permissions[0] : permissions[1]], rel, allowed_writes, 0, "workflow-level"
    )

    if _top_level_block(lines, "concurrency") is None:
        raise WorkflowSecurityError(f"{rel}: explicit concurrency policy is required")

    for job_id, block in _job_blocks(lines, rel):
        if not any(
            re.fullmatch(r"    timeout-minutes:\s*[0-9]+\s*", _code(line))
            for line in block
        ):
            raise WorkflowSecurityError(f"{rel}:{job_id}: timeout-minutes is required")
        _check_write_permissions(block, rel, allowed_writes, 4, f"job {job_id}")

    for index, raw in enumerate(lines):
        match = re.search(r"\buses:\s*([^\s#]+)", _code(raw))
        if not match:
            continue
        target = match.group(1).strip("'\"")
        if target.startswith("./") or target.startswith("$/"):
            continue
        if target.startswith("docker://"):
            if "@sha256:" not in target:
                raise WorkflowSecurityError(f"{rel}: mutable docker uses reference: {target}")
            continue
        if "@" not in target:
            raise WorkflowSecurityError(f"{rel}: action reference lacks immutable ref: {target}")
        _, ref = target.rsplit("@", 1)
        if SHA_REF.fullmatch(ref) is None:
            raise WorkflowSecurityError(f"{rel}: action reference is not a full SHA: {target}")

        if target.startswith("actions/checkout@"):
            step_indent = _step_list_indent(lines, index)
            found = False
            for candidate in lines[index + 1 :]:
                candidate_code = _code(candidate)
                if candidate_code.strip() and _indent(candidate) <= step_indent:
                    break
                if re.fullmatch(r"\s*persist-credentials:\s*false\s*", candidate_code):
                    found = True
                    break
            if not found:
                raise WorkflowSecurityError(
                    f"{rel}: actions/checkout must set persist-credentials: false"
                )

    if "pull_request_target:" in text and "actions/checkout@" in text:
        raise WorkflowSecurityError(
            f"{rel}: pull_request_target workflow must not checkout repository content"
        )

    for kind, body in _executable_blocks(lines):
        for expression in UNTRUSTED_EXPRESSIONS:
            if "${{" in body and expression in body:
                raise WorkflowSecurityError(
                    f"{rel}: untrusted GitHub expression interpolated directly into {kind}: {expression}"
                )
        if kind == "run" and (
            re.search(r"--jq\s+[\"']?\$\(", body)
            or re.search(r"--jq\s+[\"']?\$[A-Za-z_]", body)
        ):
            raise WorkflowSecurityError(
                f"{rel}: dynamic shell value used as jq program text; bind data with --arg"
            )

    for raw in lines:
        code = _code(raw)
        match = re.match(r"\s*image:\s*([^\s#]+)", code)
        if match:
            image = match.group(1).strip("'\"")
            if "@sha256:" not in image:
                raise WorkflowSecurityError(
                    f"{rel}: container/service image is not digest pinned: {image}"
                )


def verify_repository(root: Path) -> None:
    workflows = root / ".github" / "workflows"
    paths = sorted([*workflows.glob("*.yml"), *workflows.glob("*.yaml")])
    if not paths:
        raise WorkflowSecurityError("no workflows found")
    for path in paths:
        verify_workflow(path, root)


def main() -> int:
    try:
        verify_repository(Path("."))
    except (OSError, WorkflowSecurityError) as exc:
        print(f"workflow-security error: {exc}", file=sys.stderr)
        return 1
    print("workflow security policy verified")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
