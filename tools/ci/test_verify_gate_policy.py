#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

MODULE_PATH = Path(__file__).with_name("verify_gate_policy.py")
spec = importlib.util.spec_from_file_location("verify_gate_policy", MODULE_PATH)
assert spec and spec.loader
mod = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = mod
spec.loader.exec_module(mod)

BASE_CI = """
name: CI
on:
  pull_request:
    branches: [main]
jobs:
  fmt:
    name: fmt
    runs-on: ubuntu-latest
    steps:
      - run: true
"""

POLICY = """
schema = 1
[ruleset]
name = "Protect main"
required_check_integration_id = 15368
[compatibility_scope]
run = ["crates/**", "tools/**", "tests/**", "docs/compatibility/**"]
skip = ["docs/**", "*.md", ".github/**", ".cargo/config.toml", ".mailmap", "clippy.toml"]
[[producer]]
workflow = ".github/workflows/ci.yml"
job = "fmt"
classification = "required"
contexts = ["fmt"]
always_present = true
rationale = "fixture"
"""


def ruleset(contexts: list[str]) -> dict:
    return {
        "name": "Protect main",
        "target": "branch",
        "enforcement": "active",
        "bypass_actors": [],
        "conditions": {"ref_name": {"include": ["refs/heads/main"], "exclude": []}},
        "rules": [
            {"type": "deletion"},
            {"type": "non_fast_forward"},
            {"type": "required_linear_history"},
            {
                "type": "pull_request",
                "parameters": {
                    "allowed_merge_methods": ["squash"],
                    "required_review_thread_resolution": True,
                    "require_extra_approval_for_unattributed_changes": True,
                },
            },
            {
                "type": "required_status_checks",
                "parameters": {
                    "strict_required_status_checks_policy": True,
                    "required_status_checks": [
                        {"context": value, "integration_id": 15368}
                        for value in contexts
                    ],
                },
            },
        ],
    }


class GatePolicyNegativeTests(unittest.TestCase):
    def make_repo(
        self, workflow: str = BASE_CI, policy: str = POLICY
    ) -> tuple[tempfile.TemporaryDirectory, Path, dict]:
        tmp = tempfile.TemporaryDirectory()
        root = Path(tmp.name)
        (root / ".github" / "workflows").mkdir(parents=True)
        (root / ".github" / "workflows" / "ci.yml").write_text(
            textwrap.dedent(workflow).lstrip(), encoding="utf-8"
        )
        (root / ".github" / "gate-policy.toml").write_text(
            textwrap.dedent(policy).lstrip(), encoding="utf-8"
        )
        return tmp, root, mod.load_policy(root / ".github" / "gate-policy.toml")

    def test_clean_fixture_passes(self):
        tmp, root, policy = self.make_repo()
        self.addCleanup(tmp.cleanup)
        mod.verify_repository(root, policy, ruleset(["fmt"]))

    def test_required_job_rename_fails(self):
        tmp, root, policy = self.make_repo(
            BASE_CI.replace("  fmt:\n", "  format:\n", 1)
        )
        self.addCleanup(tmp.cleanup)
        with self.assertRaisesRegex(mod.PolicyError, "unclassified PR-time producer"):
            mod.verify_repository(root, policy)

    def test_required_job_removal_fails(self):
        workflow = (
            BASE_CI.split("jobs:\n", 1)[0]
            + "jobs:\n  other:\n    name: other\n    runs-on: ubuntu-latest\n"
        )
        tmp, root, policy = self.make_repo(workflow)
        self.addCleanup(tmp.cleanup)
        with self.assertRaises(mod.PolicyError):
            mod.verify_repository(root, policy)

    def test_policy_required_context_omission_fails(self):
        bad = POLICY.replace('contexts = ["fmt"]', 'contexts = ["format"]')
        tmp, root, policy = self.make_repo(policy=bad)
        self.addCleanup(tmp.cleanup)
        with self.assertRaisesRegex(mod.PolicyError, "context drift"):
            mod.verify_repository(root, policy)

    def test_stale_live_required_context_fails(self):
        tmp, root, policy = self.make_repo()
        self.addCleanup(tmp.cleanup)
        with self.assertRaisesRegex(mod.PolicyError, r"stale=\['old-context'\]"):
            mod.verify_repository(root, policy, ruleset(["fmt", "old-context"]))

    def test_required_producer_path_suppression_fails(self):
        workflow = BASE_CI.replace(
            "  pull_request:\n    branches: [main]",
            '  pull_request:\n    branches: [main]\n    paths: ["crates/**"]',
        )
        tmp, root, policy = self.make_repo(workflow)
        self.addCleanup(tmp.cleanup)
        with self.assertRaisesRegex(mod.PolicyError, "paths/paths-ignore"):
            mod.verify_repository(root, policy)

    def test_required_producer_job_condition_fails(self):
        workflow = BASE_CI.replace(
            "    name: fmt\n", "    name: fmt\n    if: github.actor != 'nobody'\n"
        )
        tmp, root, policy = self.make_repo(workflow)
        self.addCleanup(tmp.cleanup)
        with self.assertRaisesRegex(mod.PolicyError, "job-level if"):
            mod.verify_repository(root, policy)

    def test_unclassified_pr_job_fails(self):
        workflow = BASE_CI + "  surprise:\n    name: surprise\n    runs-on: ubuntu-latest\n"
        tmp, root, policy = self.make_repo(workflow)
        self.addCleanup(tmp.cleanup)
        with self.assertRaisesRegex(mod.PolicyError, "unclassified PR-time producer"):
            mod.verify_repository(root, policy)

    def test_unknown_compatibility_path_fails_closed(self):
        tmp, root, policy = self.make_repo()
        self.addCleanup(tmp.cleanup)
        with self.assertRaisesRegex(mod.PolicyError, "unclassified compatibility-scope"):
            mod.compatibility_scope(policy, ["new-runtime/kernel.rs"])

    def test_new_crate_and_tool_paths_run_compatibility(self):
        tmp, root, policy = self.make_repo()
        self.addCleanup(tmp.cleanup)
        relevant, paths, _ = mod.compatibility_scope(
            policy, ["crates/new-crate/src/lib.rs", "tools/new-oracle.py"]
        )
        self.assertTrue(relevant)
        self.assertEqual(paths, ["crates/new-crate/src/lib.rs", "tools/new-oracle.py"])

    def test_repository_metadata_paths_skip_compatibility(self):
        tmp, root, policy = self.make_repo()
        self.addCleanup(tmp.cleanup)
        relevant, paths, unknown = mod.compatibility_scope(
            policy, [".cargo/config.toml", ".mailmap", "clippy.toml"]
        )
        self.assertFalse(relevant)
        self.assertEqual(paths, [])
        self.assertEqual(unknown, [])

    def test_matrix_contexts_are_exact(self):
        workflow = """
name: CI
on:
  pull_request:
    branches: [main]
jobs:
  test:
    name: test (${{ matrix.os }})
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
    runs-on: ${{ matrix.os }}
"""
        policy_text = """
schema = 1
[ruleset]
name = "Protect main"
required_check_integration_id = 15368
[compatibility_scope]
run = ["crates/**"]
skip = ["docs/**"]
[[producer]]
workflow = ".github/workflows/ci.yml"
job = "test"
classification = "required"
contexts = ["test (ubuntu-latest)", "test (macos-latest)", "test (windows-latest)"]
always_present = true
matrix_axis = "os"
matrix_values = ["ubuntu-latest", "macos-latest", "windows-latest"]
rationale = "fixture"
"""
        tmp, root, policy = self.make_repo(workflow, policy_text)
        self.addCleanup(tmp.cleanup)
        mod.verify_repository(
            root,
            policy,
            ruleset(
                [
                    "test (ubuntu-latest)",
                    "test (macos-latest)",
                    "test (windows-latest)",
                ]
            ),
        )

    def test_jdk25_reference_paths_run_compatibility(self):
        tmp, root, policy = self.make_repo()
        self.addCleanup(tmp.cleanup)
        for path in [
            "crates/arkst-engine/src/locale.rs",
            "tools/generate_jdk25_locale_data.py",
            "tests/compat/corpus/jdk25.json",
            "docs/compatibility/quarkdown/reference-jvm.toml",
        ]:
            relevant, _, _ = mod.compatibility_scope(policy, [path])
            self.assertTrue(relevant, path)

    def test_docs_only_path_can_skip(self):
        tmp, root, policy = self.make_repo()
        self.addCleanup(tmp.cleanup)
        relevant, paths, _ = mod.compatibility_scope(policy, ["docs/guide.md"])
        self.assertFalse(relevant)
        self.assertEqual(paths, [])


if __name__ == "__main__":
    unittest.main()
