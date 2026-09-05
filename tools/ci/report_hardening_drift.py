#!/usr/bin/env python3
"""Reconcile the one marker-owned issue for Arkst hardening drift."""

from __future__ import annotations

import argparse
import base64
import json
import os
import sys
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass
from typing import Any, Protocol

MARKER = "<!-- arkst-owned:hardening-drift-audit -->"
TITLE = "ci: repository hardening drift audit requires attention"
LABELS = ["type:task", "area:ci", "priority:normal"]


class ReporterError(RuntimeError):
    pass


@dataclass
class OwnedIssue:
    number: int
    state: str
    body: str


class IssueClient(Protocol):
    def find_owned(self, marker: str) -> list[OwnedIssue]: ...
    def create_issue(self, title: str, body: str, labels: list[str]) -> int: ...
    def update_issue(self, number: int, *, body: str | None = None, state: str | None = None) -> None: ...
    def comment(self, number: int, body: str) -> None: ...


class GitHubIssueClient:
    def __init__(self, repository: str, token: str) -> None:
        self.base = f"https://api.github.com/repos/{repository}"
        self.headers = {
            "Accept": "application/vnd.github+json",
            "Authorization": f"Bearer {token}",
            "X-GitHub-Api-Version": "2022-11-28",
            "User-Agent": "arkst-hardening-drift-reporter",
        }

    def _request(self, method: str, path: str, payload: dict[str, Any] | None = None) -> Any:
        data = None if payload is None else json.dumps(payload).encode("utf-8")
        headers = dict(self.headers)
        if data is not None:
            headers["Content-Type"] = "application/json"
        request = urllib.request.Request(self.base + path, headers=headers, data=data, method=method)
        try:
            with urllib.request.urlopen(request, timeout=30) as response:
                body = response.read()
                return json.loads(body) if body else None
        except (urllib.error.HTTPError, urllib.error.URLError, TimeoutError) as exc:
            raise ReporterError(f"{method} {path} failed: {exc}") from exc

    def find_owned(self, marker: str) -> list[OwnedIssue]:
        page = 1
        matches: list[OwnedIssue] = []
        while True:
            query = urllib.parse.urlencode(
                {"state": "all", "per_page": 100, "page": page, "labels": "type:task,area:ci"}
            )
            items = self._request("GET", f"/issues?{query}")
            if not isinstance(items, list):
                raise ReporterError("issues list returned non-list payload")
            for item in items:
                if not isinstance(item, dict) or item.get("pull_request"):
                    continue
                body = item.get("body") or ""
                if marker in body:
                    matches.append(
                        OwnedIssue(
                            number=int(item["number"]),
                            state=str(item.get("state", "")),
                            body=str(body),
                        )
                    )
            if len(items) < 100:
                return matches
            page += 1

    def create_issue(self, title: str, body: str, labels: list[str]) -> int:
        payload = self._request(
            "POST", "/issues", {"title": title, "body": body, "labels": labels}
        )
        if not isinstance(payload, dict) or not isinstance(payload.get("number"), int):
            raise ReporterError("create issue returned malformed payload")
        return payload["number"]

    def update_issue(
        self, number: int, *, body: str | None = None, state: str | None = None
    ) -> None:
        payload: dict[str, Any] = {}
        if body is not None:
            payload["body"] = body
        if state is not None:
            payload["state"] = state
        self._request("PATCH", f"/issues/{number}", payload)

    def comment(self, number: int, body: str) -> None:
        self._request("POST", f"/issues/{number}/comments", {"body": body})


def render_body(audit: dict[str, Any], run_url: str) -> str:
    return "\n".join(
        [
            MARKER,
            "## Repository hardening drift audit",
            "",
            f"**Classification:** `{audit.get('classification')}`",
            "",
            f"**Workflow run:** {run_url}",
            "",
            "### Confirmed policy drift",
            "```json",
            json.dumps(audit.get("policy_findings", []), indent=2)[:30000],
            "```",
            "",
            "### Infrastructure/readback failures",
            "```json",
            json.dumps(audit.get("infrastructure_failures", []), indent=2)[:20000],
            "```",
            "",
            "### Explicit manual-readback controls",
            "The scheduled low-privilege audit does not claim continuous coverage for these controls.",
            "```json",
            json.dumps(audit.get("manual_readback", []), indent=2)[:20000],
            "```",
            "",
            "This issue is owned by the hardening drift audit. Non-clean runs update/reopen it; a clean run records recovery and closes it.",
            "",
        ]
    )


def reconcile(client: IssueClient, audit: dict[str, Any], run_url: str) -> str:
    classification = audit.get("classification")
    if classification not in {"clean", "policy-drift", "infrastructure-failure"}:
        raise ReporterError(f"unsupported classification: {classification!r}")
    owned = client.find_owned(MARKER)
    if len(owned) > 1:
        raise ReporterError(
            f"duplicate owned hardening-drift issues found: {[item.number for item in owned]}"
        )
    current = owned[0] if owned else None

    if classification == "clean":
        if current is None:
            return "clean-no-owned-issue"
        if current.state == "open":
            client.comment(current.number, f"Audit recovered to clean.\n\nRun: {run_url}")
            client.update_issue(current.number, state="closed")
            return f"closed-recovered-issue-{current.number}"
        return f"clean-owned-issue-{current.number}-already-closed"

    body = render_body(audit, run_url)
    if current is None:
        number = client.create_issue(TITLE, body, LABELS)
        return f"created-issue-{number}"

    if current.state == "closed":
        client.update_issue(current.number, state="open")
    client.update_issue(current.number, body=body)
    client.comment(
        current.number,
        f"Hardening audit remains `{classification}`. Latest run: {run_url}",
    )
    return f"updated-issue-{current.number}"


def decode_result(encoded: str) -> dict[str, Any]:
    if not encoded:
        return {
            "schema_version": 1,
            "classification": "infrastructure-failure",
            "policy_findings": [],
            "infrastructure_failures": [
                {
                    "control": "detector-output",
                    "details": "detector did not publish a result payload",
                }
            ],
            "manual_readback": [],
        }
    try:
        payload = json.loads(base64.b64decode(encoded).decode("utf-8"))
    except (ValueError, UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ReporterError(f"invalid detector payload: {exc}") from exc
    if not isinstance(payload, dict):
        raise ReporterError("detector payload must be an object")
    return payload


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--result-b64", default=os.environ.get("AUDIT_RESULT_B64", ""))
    parser.add_argument("--run-url", required=True)
    args = parser.parse_args()
    repository = os.environ.get("GITHUB_REPOSITORY")
    token = os.environ.get("GITHUB_TOKEN")
    if not repository or not token:
        print("GITHUB_REPOSITORY and GITHUB_TOKEN are required", file=sys.stderr)
        return 2
    try:
        audit = decode_result(args.result_b64)
        action = reconcile(GitHubIssueClient(repository, token), audit, args.run_url)
        print(action)
        return 0 if audit.get("classification") == "clean" else 1
    except (ReporterError, OSError) as exc:
        print(f"hardening drift reporter error: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
