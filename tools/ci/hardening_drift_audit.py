#!/usr/bin/env python3
"""Compose Arkst's canonical hardening controls into one read-mostly drift audit."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import tempfile
import tomllib
import urllib.error
import urllib.request
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable

ROOT = Path(__file__).resolve().parents[2]
GATE_POLICY = ROOT / ".github" / "gate-policy.toml"
WORKFLOWS = ROOT / ".github" / "workflows"


class AuditInfrastructureError(RuntimeError):
    pass


@dataclass(frozen=True)
class Finding:
    control: str
    details: str

    def json(self) -> dict[str, str]:
        return {"control": self.control, "details": self.details}


class GitHubReadClient:
    def __init__(self, repository: str, token: str) -> None:
        self.base = f"https://api.github.com/repos/{repository}"
        self.headers = {
            "Accept": "application/vnd.github+json",
            "Authorization": f"Bearer {token}",
            "X-GitHub-Api-Version": "2022-11-28",
            "User-Agent": "arkst-hardening-drift-audit",
        }

    def get(self, path: str) -> Any:
        request = urllib.request.Request(self.base + path, headers=self.headers, method="GET")
        try:
            with urllib.request.urlopen(request, timeout=30) as response:
                body = response.read()
                return json.loads(body) if body else None
        except (urllib.error.URLError, TimeoutError) as exc:
            raise AuditInfrastructureError(f"GET {path} failed: {exc}") from exc
        except urllib.error.HTTPError as exc:
            raise AuditInfrastructureError(f"GET {path} failed with HTTP {exc.code}") from exc

    def optional(self, path: str) -> tuple[int, Any | None]:
        request = urllib.request.Request(self.base + path, headers=self.headers, method="GET")
        try:
            with urllib.request.urlopen(request, timeout=30) as response:
                body = response.read()
                return response.status, json.loads(body) if body else None
        except urllib.error.HTTPError as exc:
            if exc.code in {401, 403, 404}:
                return exc.code, None
            raise AuditInfrastructureError(f"GET {path} failed with HTTP {exc.code}") from exc
        except (urllib.error.URLError, TimeoutError) as exc:
            raise AuditInfrastructureError(f"GET {path} failed: {exc}") from exc


def _run(argv: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        argv,
        cwd=ROOT,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )


def _command_check(
    control: str,
    argv: list[str],
    run: Callable[[list[str]], subprocess.CompletedProcess[str]],
) -> list[Finding]:
    completed = run(argv)
    if completed.returncode == 0:
        return []
    detail = completed.stdout.strip() or f"exit {completed.returncode}"
    return [Finding(control, detail[-8000:])]


def canonical_static_checks(
    run: Callable[[list[str]], subprocess.CompletedProcess[str]] = _run,
) -> list[Finding]:
    checks = [
        (
            "distribution-policy",
            [sys.executable, "tools/ci/verify_distribution_policy.py"],
        ),
        (
            "workflow-security",
            [sys.executable, "tools/ci/verify_workflow_security.py"],
        ),
        (
            "reference-provenance-tests",
            [sys.executable, "tools/ci/test_reference_provenance.py"],
        ),
        (
            "reference-provenance",
            [sys.executable, "tools/verify_reference_provenance.py"],
        ),
        (
            "upstream-ingestion-guards",
            [sys.executable, "tools/ci/test_upstream_quarkdown_ingestion.py"],
        ),
    ]
    findings: list[Finding] = []
    for control, argv in checks:
        findings.extend(_command_check(control, argv, run))
    findings.extend(check_codeql_authority())
    findings.extend(check_supply_chain_authority())
    findings.extend(check_governance_docs_and_ownership())
    return findings


def check_codeql_authority(root: Path = ROOT) -> list[Finding]:
    findings: list[Finding] = []
    try:
        with (root / ".github" / "gate-policy.toml").open("rb") as stream:
            policy = tomllib.load(stream)
    except (OSError, tomllib.TOMLDecodeError) as exc:
        return [Finding("codeql-authority", f"cannot load gate policy: {exc}")]

    codeql_producers = [
        item
        for item in policy.get("producer", [])
        if item.get("workflow") == ".github/workflows/codeql.yml"
        and item.get("job") == "analyze"
    ]
    expected = {
        "classification": "advisory",
        "contexts": ["codeql (actions)", "codeql (rust)"],
        "always_present": True,
        "matrix_axis": "language",
        "matrix_values": ["actions", "rust"],
    }
    if len(codeql_producers) != 1:
        findings.append(
            Finding(
                "codeql-authority",
                f"expected exactly one canonical CodeQL producer, found {len(codeql_producers)}",
            )
        )
    else:
        producer = codeql_producers[0]
        for key, value in expected.items():
            if producer.get(key) != value:
                findings.append(
                    Finding(
                        "codeql-authority",
                        f"gate-policy {key} drift: expected {value!r}, got {producer.get(key)!r}",
                    )
                )

    scanners: list[str] = []
    for path in sorted((root / ".github" / "workflows").glob("*.y*ml")):
        try:
            text = path.read_text(encoding="utf-8")
        except OSError as exc:
            findings.append(Finding("codeql-authority", f"cannot read {path}: {exc}"))
            continue
        if "github/codeql-action/" in text:
            scanners.append(path.relative_to(root).as_posix())
    if scanners != [".github/workflows/codeql.yml"]:
        findings.append(
            Finding(
                "codeql-authority",
                f"checked-in CodeQL authority must be exactly codeql.yml, found {scanners!r}",
            )
        )

    codeql_path = root / ".github" / "workflows" / "codeql.yml"
    try:
        text = codeql_path.read_text(encoding="utf-8")
    except OSError as exc:
        return findings + [Finding("codeql-authority", f"cannot read codeql workflow: {exc}")]
    required_fragments = [
        "language: [actions, rust]",
        "build-mode: none",
        "security-events: write",
        "persist-credentials: false",
    ]
    for fragment in required_fragments:
        if fragment not in text:
            findings.append(
                Finding("codeql-authority", f"codeql workflow missing contract fragment {fragment!r}")
            )
    return findings


def check_supply_chain_authority(root: Path = ROOT) -> list[Finding]:
    path = root / ".github" / "workflows" / "security.yml"
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as exc:
        return [Finding("supply-chain-authority", f"cannot read security workflow: {exc}")]
    required = [
        "schedule:",
        "workflow_dispatch:",
        "EmbarkStudios/cargo-deny-action@",
        "arguments: --all-features",
        "report-failure:",
        "issues: write",
        "<!-- arkst-owned:scheduled-supply-chain-failure -->",
        "force_failure",
    ]
    return [
        Finding("supply-chain-authority", f"security workflow missing {fragment!r}")
        for fragment in required
        if fragment not in text
    ]


def check_governance_docs_and_ownership(root: Path = ROOT) -> list[Finding]:
    findings: list[Finding] = []
    required_files = [
        "CODE_OF_CONDUCT.md",
        "SECURITY.md",
        ".github/CODEOWNERS",
        ".github/workflows/issue-labeler.yml",
        ".github/workflows/pr-labeler.yml",
    ]
    for rel in required_files:
        if not (root / rel).is_file():
            findings.append(Finding("governance-files", f"required file missing: {rel}"))

    try:
        security = (root / "SECURITY.md").read_text(encoding="utf-8")
        if "/security/advisories/new" not in security:
            findings.append(
                Finding("security-reporting", "SECURITY.md no longer points to private vulnerability reporting")
            )
        if "Do not report security vulnerabilities via public GitHub issues" not in security:
            findings.append(
                Finding("security-reporting", "SECURITY.md lost the public-issue prohibition")
            )
    except OSError as exc:
        findings.append(Finding("security-reporting", f"cannot read SECURITY.md: {exc}"))

    try:
        codeowners = (root / ".github" / "CODEOWNERS").read_text(encoding="utf-8")
        required_owner_paths = [
            "/.github/ @luceat-lux-vestra",
            "/Cargo.toml @luceat-lux-vestra",
            "/Cargo.lock @luceat-lux-vestra",
            "/deny.toml @luceat-lux-vestra",
            "/LICENSE @luceat-lux-vestra",
            "/NOTICE @luceat-lux-vestra",
        ]
        for line in required_owner_paths:
            if line not in codeowners:
                findings.append(Finding("codeowners", f"sensitive ownership rule missing: {line}"))
    except OSError as exc:
        findings.append(Finding("codeowners", f"cannot read CODEOWNERS: {exc}"))

    try:
        labeler = (root / ".github" / "workflows" / "issue-labeler.yml").read_text(
            encoding="utf-8"
        )
        for label in ("type:task", "area:ci", "priority:normal"):
            if label not in labeler:
                findings.append(Finding("label-automation", f"managed label definition missing: {label}"))
        if "workflow_dispatch:" not in labeler:
            findings.append(Finding("label-automation", "issue label reconciliation dispatch is missing"))
    except OSError as exc:
        findings.append(Finding("label-automation", f"cannot read issue labeler: {exc}"))
    return findings


def _ruleset_by_name(client: GitHubReadClient, name: str) -> dict[str, Any]:
    raw = client.get("/rulesets")
    if not isinstance(raw, list):
        raise AuditInfrastructureError("rulesets endpoint returned a non-list payload")
    matches = [item for item in raw if isinstance(item, dict) and item.get("name") == name]
    if len(matches) != 1:
        raise AuditInfrastructureError(
            f"expected exactly one live ruleset named {name!r}, found {len(matches)}"
        )
    ruleset_id = matches[0].get("id")
    if not isinstance(ruleset_id, int):
        raise AuditInfrastructureError(f"ruleset {name!r} has no numeric id")
    detail = client.get(f"/rulesets/{ruleset_id}")
    if not isinstance(detail, dict):
        raise AuditInfrastructureError(f"ruleset {name!r} detail is not an object")
    return detail


def check_tag_ruleset(ruleset: dict[str, Any]) -> tuple[list[Finding], list[dict[str, str]]]:
    findings: list[Finding] = []
    manual: list[dict[str, str]] = []
    if ruleset.get("target") != "tag":
        findings.append(Finding("release-tag-ruleset", f"target is {ruleset.get('target')!r}, expected 'tag'"))
    if ruleset.get("enforcement") != "active":
        findings.append(
            Finding("release-tag-ruleset", f"enforcement is {ruleset.get('enforcement')!r}, expected 'active'")
        )
    include = ruleset.get("conditions", {}).get("ref_name", {}).get("include", [])
    if include != ["refs/tags/v*"]:
        findings.append(Finding("release-tag-ruleset", f"include drift: {include!r}"))
    types = {item.get("type") for item in ruleset.get("rules", []) if isinstance(item, dict)}
    if types != {"deletion", "update"}:
        findings.append(Finding("release-tag-ruleset", f"rule types drift: {sorted(types)!r}"))
    if "bypass_actors" in ruleset:
        if ruleset.get("bypass_actors"):
            findings.append(Finding("release-tag-ruleset", "bypass actors are configured"))
    else:
        manual.append(
            {
                "control": "release-tag-ruleset.bypass-actors",
                "reason": "low-privilege ruleset response omitted bypass_actors",
            }
        )
    if "current_user_can_bypass" in ruleset and ruleset.get("current_user_can_bypass") != "never":
        findings.append(
            Finding(
                "release-tag-ruleset",
                f"current_user_can_bypass={ruleset.get('current_user_can_bypass')!r}",
            )
        )
    return findings, manual


def check_repository_settings(repository: dict[str, Any]) -> list[Finding]:
    expected = {
        "visibility": "public",
        "default_branch": "main",
        "allow_squash_merge": True,
        "allow_merge_commit": False,
        "allow_rebase_merge": False,
        "delete_branch_on_merge": True,
        "allow_update_branch": True,
        "squash_merge_commit_title": "PR_TITLE",
        "squash_merge_commit_message": "PR_BODY",
        "has_wiki": False,
        "has_discussions": False,
    }
    findings: list[Finding] = []
    for key, value in expected.items():
        if key not in repository:
            raise AuditInfrastructureError(f"repository readback omitted required field {key!r}")
        if repository[key] != value:
            findings.append(
                Finding("repository-settings", f"{key}: expected {value!r}, got {repository[key]!r}")
            )
    return findings


def _optional_security_checks(
    client: GitHubReadClient, repository: dict[str, Any]
) -> tuple[list[Finding], list[dict[str, str]]]:
    findings: list[Finding] = []
    manual: list[dict[str, str]] = []

    security = repository.get("security_and_analysis")
    if isinstance(security, dict):
        for key in ("secret_scanning", "secret_scanning_push_protection"):
            value = security.get(key)
            if isinstance(value, dict) and value.get("status") != "enabled":
                findings.append(
                    Finding("security-settings", f"{key} expected enabled, got {value.get('status')!r}")
                )
    else:
        manual.append(
            {
                "control": "secret-scanning-and-push-protection",
                "reason": "repository readback omitted security_and_analysis",
            }
        )

    status, payload = client.optional("/private-vulnerability-reporting")
    if status == 200:
        if not isinstance(payload, dict) or payload.get("enabled") is not True:
            findings.append(
                Finding("private-vulnerability-reporting", f"expected enabled=true, got {payload!r}")
            )
    else:
        manual.append(
            {
                "control": "private-vulnerability-reporting",
                "reason": f"scheduled token readback unavailable (HTTP {status})",
            }
        )

    status, payload = client.optional("/dependency-graph/sbom")
    if status == 200:
        if not isinstance(payload, dict) or not isinstance(payload.get("sbom"), dict):
            raise AuditInfrastructureError("dependency graph SBOM readback was malformed")
    else:
        manual.append(
            {
                "control": "dependency-graph",
                "reason": f"scheduled token readback unavailable (HTTP {status})",
            }
        )

    manual.extend(
        [
            {
                "control": "codeql-default-setup-disabled",
                "reason": "Administration readback is intentionally not delegated to the scheduled GITHUB_TOKEN",
            },
            {
                "control": "actions-token-fork-and-allowlist-policy",
                "reason": "Administration readback remains an explicit manual control; no privileged PAT is introduced",
            },
        ]
    )
    return findings, manual


def live_checks(
    client: GitHubReadClient,
    run: Callable[[list[str]], subprocess.CompletedProcess[str]] = _run,
) -> tuple[list[Finding], list[dict[str, str]]]:
    findings: list[Finding] = []
    manual: list[dict[str, str]] = []

    repository = client.get("")
    if not isinstance(repository, dict):
        raise AuditInfrastructureError("repository endpoint returned a non-object payload")
    findings.extend(check_repository_settings(repository))

    main_ruleset = _ruleset_by_name(client, "Protect main")
    with tempfile.TemporaryDirectory() as directory:
        path = Path(directory) / "ruleset.json"
        path.write_text(json.dumps(main_ruleset), encoding="utf-8")
        findings.extend(
            _command_check(
                "protect-main-ruleset",
                [
                    sys.executable,
                    "tools/ci/verify_gate_policy.py",
                    "verify",
                    "--ruleset-json",
                    str(path),
                ],
                run,
            )
        )

    tag_ruleset = _ruleset_by_name(client, "Protect immutable release tags")
    tag_findings, tag_manual = check_tag_ruleset(tag_ruleset)
    findings.extend(tag_findings)
    manual.extend(tag_manual)

    security_findings, security_manual = _optional_security_checks(client, repository)
    findings.extend(security_findings)
    manual.extend(security_manual)
    return findings, manual


def classification(policy_findings: list[Finding], infrastructure: list[Finding]) -> str:
    if infrastructure:
        return "infrastructure-failure"
    if policy_findings:
        return "policy-drift"
    return "clean"


def result_payload(
    policy_findings: list[Finding],
    infrastructure: list[Finding],
    manual: list[dict[str, str]],
) -> dict[str, Any]:
    return {
        "schema_version": 1,
        "classification": classification(policy_findings, infrastructure),
        "policy_findings": [item.json() for item in policy_findings],
        "infrastructure_failures": [item.json() for item in infrastructure],
        "manual_readback": manual,
    }


def synthetic(mode: str) -> dict[str, Any]:
    manual = [
        {
            "control": "synthetic-manual-readback",
            "reason": "fixture proving explicit manual coverage inventory",
        }
    ]
    if mode == "clean":
        return result_payload([], [], manual)
    if mode == "policy-drift":
        return result_payload([Finding("synthetic", "safe synthetic policy drift")], [], manual)
    if mode == "infrastructure-failure":
        return result_payload([], [Finding("synthetic", "safe synthetic readback failure")], manual)
    raise ValueError(mode)


def run_live() -> dict[str, Any]:
    findings = canonical_static_checks()
    infrastructure: list[Finding] = []
    manual: list[dict[str, str]] = []
    repository = os.environ.get("GITHUB_REPOSITORY")
    token = os.environ.get("GITHUB_TOKEN")
    if not repository or not token:
        infrastructure.append(
            Finding("live-readback", "GITHUB_REPOSITORY and GITHUB_TOKEN are required")
        )
        return result_payload(findings, infrastructure, manual)
    try:
        live_findings, manual = live_checks(GitHubReadClient(repository, token))
        findings.extend(live_findings)
    except (AuditInfrastructureError, OSError, json.JSONDecodeError) as exc:
        infrastructure.append(Finding("live-readback", str(exc)))
    return result_payload(findings, infrastructure, manual)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument(
        "--mode",
        choices=("live", "clean", "policy-drift", "infrastructure-failure"),
        default="live",
    )
    args = parser.parse_args()
    try:
        payload = run_live() if args.mode == "live" else synthetic(args.mode)
    except (OSError, ValueError, tomllib.TOMLDecodeError) as exc:
        payload = result_payload([], [Finding("audit-runner", str(exc))], [])
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps(payload, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
