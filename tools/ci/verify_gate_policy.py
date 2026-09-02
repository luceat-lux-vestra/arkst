#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import tomllib
from dataclasses import dataclass, field
from pathlib import Path
from typing import Iterable

ALLOWED_CLASSIFICATIONS = {
    "required",
    "advisory",
    "path_scoped_optional",
    "reference_generated_data_deep_check",
    "scheduled_deep",
    "release_only",
}
PR_EVENTS = {"pull_request", "pull_request_target"}


class PolicyError(RuntimeError):
    pass


@dataclass(frozen=True)
class Trigger:
    event: str
    path_filtered: bool = False
    types_restricted: bool = False


@dataclass
class Job:
    job_id: str
    name: str
    has_if: bool = False
    matrix: dict[str, list[str]] = field(default_factory=dict)


@dataclass
class Workflow:
    path: str
    triggers: dict[str, Trigger]
    jobs: dict[str, Job]

    @property
    def is_pr_time(self) -> bool:
        return any(event in self.triggers for event in PR_EVENTS)


def _indent(raw: str) -> int:
    return len(raw) - len(raw.lstrip(" "))


def _strip_comment(value: str) -> str:
    in_single = False
    in_double = False
    escaped = False
    out: list[str] = []
    for ch in value:
        if escaped:
            out.append(ch)
            escaped = False
            continue
        if ch == "\\" and in_double:
            out.append(ch)
            escaped = True
            continue
        if ch == "'" and not in_double:
            in_single = not in_single
        elif ch == '"' and not in_single:
            in_double = not in_double
        if ch == "#" and not in_single and not in_double:
            break
        out.append(ch)
    return "".join(out).rstrip()


def _scalar(value: str) -> str:
    value = _strip_comment(value).strip()
    if len(value) >= 2 and value[0] == value[-1] and value[0] in {"'", '"'}:
        return value[1:-1]
    return value


def _inline_list(value: str) -> list[str] | None:
    value = _strip_comment(value).strip()
    if not (value.startswith("[") and value.endswith("]")):
        return None
    body = value[1:-1].strip()
    if not body:
        return []
    return [_scalar(item.strip()) for item in body.split(",")]


def parse_workflow(path: Path, root: Path) -> Workflow:
    lines = path.read_text(encoding="utf-8").splitlines()
    rel = path.relative_to(root).as_posix()

    triggers: dict[str, Trigger] = {}
    on_index = None
    for idx, raw in enumerate(lines):
        if _indent(raw) == 0 and _strip_comment(raw).strip().startswith("on:"):
            on_index = idx
            remainder = _strip_comment(raw).strip()[3:].strip()
            if remainder:
                inline = _inline_list(remainder)
                if inline is None:
                    raise PolicyError(f"{rel}: unsupported inline on: syntax")
                for event in inline:
                    triggers[event] = Trigger(event)
            break
    if on_index is None:
        raise PolicyError(f"{rel}: missing top-level on: trigger")

    if not triggers:
        idx = on_index + 1
        while idx < len(lines):
            raw = lines[idx]
            stripped = _strip_comment(raw).strip()
            if not stripped:
                idx += 1
                continue
            ind = _indent(raw)
            if ind == 0:
                break
            event_match = re.match(
                r"^\s{2}([A-Za-z0-9_-]+):(?:\s*(.*))?$", _strip_comment(raw)
            )
            if not event_match:
                idx += 1
                continue
            event = event_match.group(1)
            remainder = (event_match.group(2) or "").strip()
            path_filtered = False
            types_restricted = False
            if not remainder:
                sub = idx + 1
                while sub < len(lines):
                    sub_raw = lines[sub]
                    sub_stripped = _strip_comment(sub_raw).strip()
                    if not sub_stripped:
                        sub += 1
                        continue
                    sub_ind = _indent(sub_raw)
                    if sub_ind <= 2:
                        break
                    if sub_ind == 4:
                        key_match = re.match(
                            r"^\s{4}([A-Za-z0-9_-]+):", _strip_comment(sub_raw)
                        )
                        if key_match:
                            key = key_match.group(1)
                            if key in {"paths", "paths-ignore"}:
                                path_filtered = True
                            elif key == "types":
                                types_restricted = True
                    sub += 1
            triggers[event] = Trigger(event, path_filtered, types_restricted)
            idx += 1

    jobs_index = None
    for idx, raw in enumerate(lines):
        if _indent(raw) == 0 and _strip_comment(raw).strip() == "jobs:":
            jobs_index = idx
            break
    if jobs_index is None:
        raise PolicyError(f"{rel}: missing top-level jobs:")

    jobs: dict[str, Job] = {}
    idx = jobs_index + 1
    while idx < len(lines):
        raw = lines[idx]
        stripped = _strip_comment(raw).strip()
        if not stripped:
            idx += 1
            continue
        if _indent(raw) == 0:
            break
        match = re.match(r"^\s{2}([A-Za-z0-9_-]+):\s*$", _strip_comment(raw))
        if not match:
            idx += 1
            continue
        job_id = match.group(1)
        end = idx + 1
        while end < len(lines):
            candidate = lines[end]
            candidate_stripped = _strip_comment(candidate).strip()
            if candidate_stripped and _indent(candidate) <= 2:
                break
            end += 1
        block = lines[idx + 1 : end]
        name = job_id
        has_if = False
        matrix: dict[str, list[str]] = {}
        matrix_indent = None
        for block_raw in block:
            block_clean = _strip_comment(block_raw)
            block_stripped = block_clean.strip()
            if not block_stripped:
                continue
            block_ind = _indent(block_raw)
            if block_ind == 4:
                name_match = re.match(r"^\s{4}name:\s*(.+)$", block_clean)
                if name_match:
                    name = _scalar(name_match.group(1))
                if re.match(r"^\s{4}if:\s*", block_clean):
                    has_if = True
            if re.match(r"^\s{6}matrix:\s*$", block_clean):
                matrix_indent = 6
                continue
            if matrix_indent is not None:
                if block_ind <= matrix_indent:
                    matrix_indent = None
                    continue
                if block_ind == 8:
                    axis_match = re.match(
                        r"^\s{8}([A-Za-z0-9_-]+):\s*(.+)$", block_clean
                    )
                    if axis_match:
                        values = _inline_list(axis_match.group(2))
                        if values is not None:
                            matrix[axis_match.group(1)] = values
        jobs[job_id] = Job(job_id=job_id, name=name, has_if=has_if, matrix=matrix)
        idx = end

    return Workflow(path=rel, triggers=triggers, jobs=jobs)


def load_policy(path: Path) -> dict:
    with path.open("rb") as stream:
        policy = tomllib.load(stream)
    if policy.get("schema") != 1:
        raise PolicyError(f"{path}: unsupported or missing schema = 1")
    producers = policy.get("producer", [])
    if not producers:
        raise PolicyError(f"{path}: no [[producer]] entries")
    seen_keys: set[tuple[str, str]] = set()
    seen_contexts: set[str] = set()
    for producer in producers:
        key = (producer.get("workflow", ""), producer.get("job", ""))
        if not all(key):
            raise PolicyError(f"{path}: producer missing workflow/job")
        if key in seen_keys:
            raise PolicyError(f"{path}: duplicate producer {key[0]}#{key[1]}")
        seen_keys.add(key)
        classification = producer.get("classification")
        if classification not in ALLOWED_CLASSIFICATIONS:
            raise PolicyError(
                f"{path}: {key[0]}#{key[1]} has invalid classification {classification!r}"
            )
        contexts = producer.get("contexts", [])
        if not contexts:
            raise PolicyError(f"{path}: {key[0]}#{key[1]} has no contexts")
        for context in contexts:
            if context in seen_contexts:
                raise PolicyError(f"{path}: duplicate PR context {context!r}")
            seen_contexts.add(context)
        if classification == "required" and producer.get("always_present") is not True:
            raise PolicyError(
                f"{path}: required producer {key[0]}#{key[1]} must set always_present = true"
            )
    return policy


def discover_workflows(root: Path) -> dict[str, Workflow]:
    workflows_dir = root / ".github" / "workflows"
    workflows: dict[str, Workflow] = {}
    for path in sorted([*workflows_dir.glob("*.yml"), *workflows_dir.glob("*.yaml")]):
        workflow = parse_workflow(path, root)
        workflows[workflow.path] = workflow
    return workflows


def _producer_contexts(producer: dict, job: Job) -> list[str]:
    axis = producer.get("matrix_axis")
    if axis is None:
        return [job.name]
    expected_values = producer.get("matrix_values", [])
    actual_values = job.matrix.get(axis)
    if actual_values != expected_values:
        raise PolicyError(
            f"{producer['workflow']}#{producer['job']}: matrix {axis!r} changed: "
            f"expected {expected_values}, got {actual_values}"
        )
    marker = "${{ matrix." + axis + " }}"
    if marker not in job.name:
        raise PolicyError(
            f"{producer['workflow']}#{producer['job']}: job name {job.name!r} "
            f"does not contain {marker!r}"
        )
    return [job.name.replace(marker, value) for value in actual_values]


def verify_repository(root: Path, policy: dict, ruleset: dict | None = None) -> None:
    workflows = discover_workflows(root)
    producers = {(item["workflow"], item["job"]): item for item in policy["producer"]}

    actual_pr_jobs: set[tuple[str, str]] = set()
    for workflow in workflows.values():
        if not workflow.is_pr_time:
            continue
        for job_id in workflow.jobs:
            actual_pr_jobs.add((workflow.path, job_id))

    policy_jobs = set(producers)
    missing_policy = sorted(actual_pr_jobs - policy_jobs)
    stale_policy = sorted(policy_jobs - actual_pr_jobs)
    if missing_policy:
        rendered = ", ".join(f"{path}#{job}" for path, job in missing_policy)
        raise PolicyError(f"unclassified PR-time producer(s): {rendered}")
    if stale_policy:
        rendered = ", ".join(f"{path}#{job}" for path, job in stale_policy)
        raise PolicyError(f"policy producer(s) missing from workflows: {rendered}")

    required_contexts: set[str] = set()
    for key, producer in producers.items():
        workflow = workflows[key[0]]
        job = workflow.jobs[key[1]]
        contexts = _producer_contexts(producer, job)
        expected_contexts = producer["contexts"]
        if contexts != expected_contexts:
            raise PolicyError(
                f"{key[0]}#{key[1]} context drift: expected {expected_contexts}, got {contexts}"
            )
        if producer["classification"] == "required":
            required_contexts.update(contexts)
            trigger = workflow.triggers.get("pull_request")
            if trigger is None:
                raise PolicyError(
                    f"required producer {key[0]}#{key[1]} must use pull_request"
                )
            if trigger.path_filtered:
                raise PolicyError(
                    f"required producer {key[0]}#{key[1]} has top-level paths/paths-ignore filtering"
                )
            if trigger.types_restricted:
                raise PolicyError(
                    f"required producer {key[0]}#{key[1]} restricts pull_request types"
                )
            if job.has_if:
                raise PolicyError(
                    f"required producer {key[0]}#{key[1]} has a job-level if condition"
                )

    if ruleset is not None:
        verify_ruleset(policy, ruleset, required_contexts)


def verify_ruleset(policy: dict, ruleset: dict, required_contexts: set[str]) -> None:
    expected = policy["ruleset"]
    if ruleset.get("name") != expected["name"]:
        raise PolicyError(
            f"ruleset name drift: expected {expected['name']!r}, got {ruleset.get('name')!r}"
        )
    if ruleset.get("target") != "branch" or ruleset.get("enforcement") != "active":
        raise PolicyError("Protect main ruleset must target branches with active enforcement")
    if ruleset.get("bypass_actors"):
        raise PolicyError("Protect main ruleset must not have bypass actors")
    include = set(ruleset.get("conditions", {}).get("ref_name", {}).get("include", []))
    if include != {"refs/heads/main"}:
        raise PolicyError(
            f"ruleset ref drift: expected only refs/heads/main, got {sorted(include)}"
        )

    rule_by_type = {rule.get("type"): rule for rule in ruleset.get("rules", [])}
    for required_type in (
        "deletion",
        "non_fast_forward",
        "required_linear_history",
        "pull_request",
        "required_status_checks",
    ):
        if required_type not in rule_by_type:
            raise PolicyError(f"ruleset missing {required_type!r} protection")

    pr = rule_by_type["pull_request"].get("parameters", {})
    if pr.get("allowed_merge_methods") != ["squash"]:
        raise PolicyError("ruleset must remain squash-only")
    if pr.get("required_review_thread_resolution") is not True:
        raise PolicyError("ruleset must require review-thread resolution")
    if pr.get("require_extra_approval_for_unattributed_changes") is not True:
        raise PolicyError("ruleset must require extra approval for unattributed changes")

    checks = rule_by_type["required_status_checks"].get("parameters", {})
    if checks.get("strict_required_status_checks_policy") is not True:
        raise PolicyError("ruleset required status checks must remain strict")
    live_checks = checks.get("required_status_checks", [])
    integration_id = expected.get("required_check_integration_id")
    if integration_id is not None:
        wrong_integrations = sorted(
            item.get("context", "<missing>")
            for item in live_checks
            if item.get("integration_id") != integration_id
        )
        if wrong_integrations:
            raise PolicyError(
                f"required-check integration drift for contexts: {wrong_integrations}; "
                f"expected integration_id={integration_id}"
            )
    live_contexts = {item["context"] for item in live_checks}
    if live_contexts != required_contexts:
        missing = sorted(required_contexts - live_contexts)
        stale = sorted(live_contexts - required_contexts)
        raise PolicyError(
            f"live required-context drift: missing={missing}, stale={stale}"
        )


def _glob_regex(pattern: str) -> re.Pattern[str]:
    out = ["^"]
    i = 0
    while i < len(pattern):
        ch = pattern[i]
        if ch == "*":
            if i + 1 < len(pattern) and pattern[i + 1] == "*":
                out.append(".*")
                i += 2
            else:
                out.append("[^/]*")
                i += 1
        elif ch == "?":
            out.append("[^/]")
            i += 1
        else:
            out.append(re.escape(ch))
            i += 1
    out.append("$")
    return re.compile("".join(out))


def classify_compatibility_path(policy: dict, path: str) -> str:
    scope = policy["compatibility_scope"]
    for pattern in scope["run"]:
        if _glob_regex(pattern).match(path):
            return "run"
    for pattern in scope["skip"]:
        if _glob_regex(pattern).match(path):
            return "skip"
    return "unknown"


def compatibility_scope(policy: dict, paths: Iterable[str]) -> tuple[bool, list[str], list[str]]:
    run_paths: list[str] = []
    unknown: list[str] = []
    for path in sorted(set(paths)):
        classification = classify_compatibility_path(policy, path)
        if classification == "run":
            run_paths.append(path)
        elif classification == "unknown":
            unknown.append(path)
    if unknown:
        raise PolicyError("unclassified compatibility-scope path(s): " + ", ".join(unknown))
    return bool(run_paths), run_paths, unknown


def changed_paths(base_sha: str, head_sha: str) -> list[str]:
    completed = subprocess.run(
        ["git", "diff", "--name-only", "--no-renames", f"{base_sha}...{head_sha}"],
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    )
    return [line for line in completed.stdout.splitlines() if line]


def cmd_verify(args: argparse.Namespace) -> None:
    root = Path(args.repository).resolve()
    policy = load_policy(root / args.policy)
    ruleset = None
    if args.ruleset_json:
        ruleset = json.loads(Path(args.ruleset_json).read_text(encoding="utf-8"))
    verify_repository(root, policy, ruleset)
    print("merge-gate policy verified")


def cmd_scope(args: argparse.Namespace) -> None:
    root = Path(args.repository).resolve()
    policy = load_policy(root / args.policy)
    if args.paths:
        paths = args.paths
    else:
        paths = changed_paths(args.base_sha, args.head_sha)
    relevant, run_paths, _ = compatibility_scope(policy, paths)
    print(f"compatibility relevant: {str(relevant).lower()}")
    if run_paths:
        print("compatibility-triggering paths:")
        for path in run_paths:
            print(f"  {path}")
    if args.github_output:
        with Path(args.github_output).open("a", encoding="utf-8") as stream:
            stream.write(f"relevant={str(relevant).lower()}\n")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Verify Arkst merge-gate policy and compatibility scope"
    )
    parser.add_argument("--repository", default=".")
    parser.add_argument("--policy", default=".github/gate-policy.toml")
    sub = parser.add_subparsers(dest="command", required=True)

    verify = sub.add_parser("verify")
    verify.add_argument("--ruleset-json")
    verify.set_defaults(func=cmd_verify)

    scope = sub.add_parser("compatibility-scope")
    source = scope.add_mutually_exclusive_group(required=True)
    source.add_argument("--paths", nargs="+")
    source.add_argument("--base-sha")
    scope.add_argument("--head-sha")
    scope.add_argument("--github-output")
    scope.set_defaults(func=cmd_scope)
    return parser


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()
    if args.command == "compatibility-scope" and args.base_sha and not args.head_sha:
        parser.error("--head-sha is required with --base-sha")
    try:
        args.func(args)
    except (
        PolicyError,
        OSError,
        subprocess.CalledProcessError,
        json.JSONDecodeError,
        tomllib.TOMLDecodeError,
    ) as exc:
        print(f"gate-policy error: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
