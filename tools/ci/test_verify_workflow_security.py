#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

MODULE = Path(__file__).with_name("verify_workflow_security.py")
spec = importlib.util.spec_from_file_location("verify_workflow_security", MODULE)
assert spec and spec.loader
mod = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = mod
spec.loader.exec_module(mod)

ZIZMOR_MODULE = Path(__file__).with_name("verify_zizmor_config.py")
zizmor_spec = importlib.util.spec_from_file_location("verify_zizmor_config", ZIZMOR_MODULE)
assert zizmor_spec and zizmor_spec.loader
zizmor = importlib.util.module_from_spec(zizmor_spec)
sys.modules[zizmor_spec.name] = zizmor
zizmor_spec.loader.exec_module(zizmor)

ROOT = Path(__file__).resolve().parents[2]

BASE = """
name: Fixture
on:
  pull_request:
permissions:
  contents: read
concurrency:
  group: fixture-${{ github.ref }}
  cancel-in-progress: true
jobs:
  check:
    runs-on: ubuntu-latest
    timeout-minutes: 5
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1
        with:
          persist-credentials: false
      - uses: owner/action@0123456789abcdef0123456789abcdef01234567
      - run: echo ok
"""

SECURITY_REPORTER = """
name: Security Audit
on:
  schedule:
    - cron: "0 0 * * 1"
permissions:
  contents: read
concurrency:
  group: security-${{ github.ref }}
  cancel-in-progress: false
jobs:
  audit:
    runs-on: ubuntu-latest
    timeout-minutes: 5
    steps:
      - run: echo ok
  report-failure:
    needs: [audit]
    runs-on: ubuntu-latest
    timeout-minutes: 5
    permissions:
      contents: read
      issues: write
    steps:
      - uses: actions/github-script@3a2844b7e9c422d3c10d287c895573f7108da1b3
        with:
          script: |
            core.info('report')
"""


class WorkflowSecurityTests(unittest.TestCase):
    def verify(self, content: str, name: str = "fixture.yml") -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            path = root / ".github" / "workflows" / name
            path.parent.mkdir(parents=True)
            path.write_text(textwrap.dedent(content).lstrip(), encoding="utf-8")
            mod.verify_workflow(path, root)

    def reject(self, content: str, message: str, name: str = "fixture.yml") -> None:
        with self.assertRaisesRegex(mod.WorkflowSecurityError, message):
            self.verify(content, name=name)

    def test_clean_fixture_passes(self) -> None:
        self.verify(BASE)

    def test_named_checkout_step_with_disabled_credentials_passes(self) -> None:
        named = BASE.replace(
            "      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1\n",
            "      - name: Checkout repository\n"
            "        uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1\n",
        )
        self.verify(named)

    def test_mutable_action_ref_is_rejected(self) -> None:
        self.reject(
            BASE.replace(
                "owner/action@0123456789abcdef0123456789abcdef01234567",
                "owner/action@v1",
            ),
            "full SHA",
        )

    def test_checkout_credentials_must_be_disabled(self) -> None:
        self.reject(
            BASE.replace("persist-credentials: false", "persist-credentials: true"),
            "persist-credentials",
        )

    def test_broad_workflow_write_permission_is_rejected(self) -> None:
        self.reject(
            BASE.replace("contents: read", "contents: write"),
            "unapproved workflow-level write",
        )

    def test_broad_job_write_permission_is_rejected(self) -> None:
        bad = BASE.replace(
            "    runs-on: ubuntu-latest\n",
            "    runs-on: ubuntu-latest\n    permissions:\n      contents: write\n",
        )
        self.reject(bad, "unapproved job check write")

    def test_security_reporter_job_may_write_issues(self) -> None:
        self.verify(SECURITY_REPORTER, name="security.yml")

    def test_security_audit_job_may_not_write_issues(self) -> None:
        bad = SECURITY_REPORTER.replace(
            "  audit:\n    runs-on: ubuntu-latest\n",
            "  audit:\n    runs-on: ubuntu-latest\n    permissions:\n      contents: read\n      issues: write\n",
        )
        self.reject(bad, "unapproved job audit write permission", name="security.yml")

    def test_security_workflow_may_not_write_issues_at_top_level(self) -> None:
        bad = SECURITY_REPORTER.replace(
            "permissions:\n  contents: read\n",
            "permissions:\n  contents: read\n  issues: write\n",
            1,
        )
        self.reject(bad, "unapproved workflow-level write permission", name="security.yml")

    def test_missing_timeout_is_rejected(self) -> None:
        self.reject(BASE.replace("    timeout-minutes: 5\n", ""), "timeout-minutes")

    def test_missing_concurrency_is_rejected(self) -> None:
        self.reject(
            BASE.replace(
                "concurrency:\n  group: fixture-${{ github.ref }}\n  cancel-in-progress: true\n",
                "",
            ),
            "concurrency",
        )

    def test_pull_request_target_checkout_is_rejected(self) -> None:
        self.reject(
            BASE.replace("pull_request:", "pull_request_target:"), "must not checkout"
        )

    def test_untrusted_pr_body_direct_shell_interpolation_is_rejected(self) -> None:
        bad = BASE.replace("echo ok", "echo '${{ github.event.pull_request.body }}'")
        self.reject(bad, "untrusted GitHub expression")

    def test_untrusted_pr_title_github_script_interpolation_is_rejected(self) -> None:
        bad = BASE.replace(
            "      - run: echo ok\n",
            "      - uses: actions/github-script@3a2844b7e9c422d3c10d287c895573f7108da1b3\n"
            "        with:\n"
            "          script: |\n"
            "            core.info('${{ github.event.pull_request.title }}')\n",
        )
        self.reject(bad, "untrusted GitHub expression")

    def test_dynamic_jq_program_is_rejected(self) -> None:
        bad = BASE.replace("echo ok", 'FILTER=".[]"; gh api x --jq "$FILTER"')
        self.reject(bad, "dynamic shell value used as jq program text")

    def test_mutable_container_image_is_rejected(self) -> None:
        bad = BASE.replace(
            "    runs-on: ubuntu-latest\n",
            "    runs-on: ubuntu-latest\n    container:\n      image: ubuntu:latest\n",
        )
        self.reject(bad, "digest pinned")

    def test_repository_zizmor_suppression_inventory_is_exact(self) -> None:
        zizmor.verify_config(ROOT / "zizmor.yml")

    def test_broadened_zizmor_suppression_is_rejected(self) -> None:
        valid = (ROOT / "zizmor.yml").read_text(encoding="utf-8")
        broadened = valid.replace(
            "      - pr-labeler.yml\n",
            "      - pr-labeler.yml\n      - upstream-quarkdown.yml\n",
        )
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "zizmor.yml"
            path.write_text(broadened, encoding="utf-8")
            with self.assertRaisesRegex(zizmor.ZizmorConfigError, "suppression policy drifted"):
                zizmor.verify_config(path)


if __name__ == "__main__":
    unittest.main()
