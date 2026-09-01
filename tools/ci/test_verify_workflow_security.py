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


class WorkflowSecurityTests(unittest.TestCase):
    def verify(self, content: str, name: str = "fixture.yml") -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            path = root / ".github" / "workflows" / name
            path.parent.mkdir(parents=True)
            path.write_text(textwrap.dedent(content).lstrip(), encoding="utf-8")
            mod.verify_workflow(path, root)

    def reject(self, content: str, message: str) -> None:
        with self.assertRaisesRegex(mod.WorkflowSecurityError, message):
            self.verify(content)

    def test_clean_fixture_passes(self) -> None:
        self.verify(BASE)

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


if __name__ == "__main__":
    unittest.main()
