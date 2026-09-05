#!/usr/bin/env python3
"""Run the hardening drift audit with an explicit low-privilege readback boundary."""

from __future__ import annotations

from typing import Any, Callable

import hardening_drift_audit as audit

# GitHub's scheduled GITHUB_TOKEN may omit repository merge-policy fields from
# GET /repos/{owner}/{repo}. When a field is absent, the audit must not assume
# that the configured value is correct, but it also must not misclassify a
# permission-limited readback as an infrastructure outage. These controls stay
# in the result as explicit manual-readback obligations. If GitHub returns a
# field, the canonical checker still verifies its value and reports real drift.
PRIVILEGE_SENSITIVE_REPOSITORY_SETTINGS: dict[str, Any] = {
    "allow_squash_merge": True,
    "allow_merge_commit": False,
    "allow_rebase_merge": False,
    "delete_branch_on_merge": True,
    "allow_update_branch": True,
    "squash_merge_commit_title": "PR_TITLE",
    "squash_merge_commit_message": "PR_BODY",
}


def normalize_repository_settings(
    repository: dict[str, Any],
) -> tuple[dict[str, Any], list[dict[str, str]]]:
    """Preserve observable values and inventory privilege-hidden settings."""
    normalized = dict(repository)
    manual: list[dict[str, str]] = []
    for key, expected in PRIVILEGE_SENSITIVE_REPOSITORY_SETTINGS.items():
        if key in normalized:
            continue
        # Supply the accepted value only to let the canonical checker continue;
        # the explicit manual entry below prevents this from becoming a claimed
        # automated PASS.
        normalized[key] = expected
        manual.append(
            {
                "control": f"repository-settings.{key}",
                "reason": "scheduled low-privilege repository readback omitted this merge-policy field",
            }
        )
    return normalized, manual


def live_checks(
    client: audit.GitHubReadClient,
    run: Callable[[list[str]], Any] = audit._run,
) -> tuple[list[audit.Finding], list[dict[str, str]]]:
    """Delegate all live checks while adapting only the known privilege boundary."""
    original_check = audit.check_repository_settings
    captured_manual: list[dict[str, str]] = []

    def bounded_check(repository: dict[str, Any]) -> list[audit.Finding]:
        normalized, manual = normalize_repository_settings(repository)
        captured_manual.extend(manual)
        return original_check(normalized)

    audit.check_repository_settings = bounded_check
    try:
        findings, manual = audit.live_checks(client, run)
    finally:
        audit.check_repository_settings = original_check
    return findings, [*manual, *captured_manual]


def main() -> int:
    original_live_checks = audit.live_checks
    audit.live_checks = live_checks
    try:
        return audit.main()
    finally:
        audit.live_checks = original_live_checks


if __name__ == "__main__":
    raise SystemExit(main())
