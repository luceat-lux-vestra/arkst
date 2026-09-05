#!/usr/bin/env python3
from __future__ import annotations

import base64
import importlib.util
import json
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent


def load(name: str, filename: str):
    spec = importlib.util.spec_from_file_location(name, HERE / filename)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


AUDIT = load("arkst_hardening_drift_audit", "hardening_drift_audit.py")
REPORT = load("arkst_hardening_drift_report", "report_hardening_drift.py")


class FakeIssueClient:
    def __init__(self, issues=None):
        self.issues = list(issues or [])
        self.created = []
        self.updated = []
        self.comments = []

    def find_owned(self, marker):
        return list(self.issues)

    def create_issue(self, title, body, labels):
        self.created.append((title, body, labels))
        return 77

    def update_issue(self, number, *, body=None, state=None):
        self.updated.append((number, body, state))

    def comment(self, number, body):
        self.comments.append((number, body))


def good_main_ruleset():
    contexts = [
        "fmt",
        "clippy",
        "test (ubuntu-latest)",
        "test (macos-latest)",
        "test (windows-latest)",
        "docs",
        "license",
        "wasm",
        "compatibility",
        "msrv",
    ]
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
                        {"context": context, "integration_id": 15368}
                        for context in contexts
                    ],
                },
            },
        ],
    }


def good_tag_ruleset():
    return {
        "name": "Protect immutable release tags",
        "target": "tag",
        "enforcement": "active",
        "bypass_actors": [],
        "current_user_can_bypass": "never",
        "conditions": {"ref_name": {"include": ["refs/tags/v*"], "exclude": []}},
        "rules": [{"type": "deletion"}, {"type": "update"}],
    }


class AuditClassificationTests(unittest.TestCase):
    def test_clean(self):
        self.assertEqual(AUDIT.classification([], []), "clean")

    def test_policy_drift(self):
        self.assertEqual(
            AUDIT.classification([AUDIT.Finding("x", "drift")], []), "policy-drift"
        )

    def test_infrastructure_failure_wins(self):
        self.assertEqual(
            AUDIT.classification(
                [AUDIT.Finding("x", "drift")], [AUDIT.Finding("api", "failed")]
            ),
            "infrastructure-failure",
        )

    def test_synthetic_modes_keep_manual_inventory(self):
        for mode in ("clean", "policy-drift", "infrastructure-failure"):
            payload = AUDIT.synthetic(mode)
            self.assertEqual(payload["classification"], mode)
            self.assertTrue(payload["manual_readback"])


class TagRulesetTests(unittest.TestCase):
    def test_exact_tag_ruleset_passes(self):
        findings, manual = AUDIT.check_tag_ruleset(good_tag_ruleset())
        self.assertEqual(findings, [])
        self.assertEqual(manual, [])

    def test_missing_update_rule_is_policy_drift(self):
        ruleset = good_tag_ruleset()
        ruleset["rules"] = [{"type": "deletion"}]
        findings, _ = AUDIT.check_tag_ruleset(ruleset)
        self.assertTrue(any(item.control == "release-tag-ruleset" for item in findings))

    def test_omitted_bypass_readback_is_explicit_manual(self):
        ruleset = good_tag_ruleset()
        ruleset.pop("bypass_actors")
        findings, manual = AUDIT.check_tag_ruleset(ruleset)
        self.assertEqual(findings, [])
        self.assertEqual(manual[0]["control"], "release-tag-ruleset.bypass-actors")


class RepositorySettingsTests(unittest.TestCase):
    def good(self):
        return {
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

    def test_clean_repository_settings(self):
        self.assertEqual(AUDIT.check_repository_settings(self.good()), [])

    def test_merge_commit_reenable_is_detected(self):
        repository = self.good()
        repository["allow_merge_commit"] = True
        findings = AUDIT.check_repository_settings(repository)
        self.assertIn("allow_merge_commit", findings[0].details)

    def test_missing_authoritative_field_is_infrastructure_failure(self):
        repository = self.good()
        del repository["allow_update_branch"]
        with self.assertRaises(AUDIT.AuditInfrastructureError):
            AUDIT.check_repository_settings(repository)


class StaticAuthorityTests(unittest.TestCase):
    def make_root(self):
        temp = tempfile.TemporaryDirectory()
        root = Path(temp.name)
        (root / ".github" / "workflows").mkdir(parents=True)
        return temp, root

    def write_codeql_fixture(self, root, *, extra_scanner=False, classification="advisory"):
        (root / ".github" / "gate-policy.toml").write_text(
            textwrap.dedent(
                f"""
                schema = 1
                [[producer]]
                workflow = ".github/workflows/codeql.yml"
                job = "analyze"
                classification = "{classification}"
                contexts = ["codeql (actions)", "codeql (rust)"]
                always_present = true
                matrix_axis = "language"
                matrix_values = ["actions", "rust"]
                """
            ),
            encoding="utf-8",
        )
        (root / ".github" / "workflows" / "codeql.yml").write_text(
            "\n".join(
                [
                    "language: [actions, rust]",
                    "build-mode: none",
                    "security-events: write",
                    "persist-credentials: false",
                    "uses: github/codeql-action/init@0123456789012345678901234567890123456789",
                ]
            ),
            encoding="utf-8",
        )
        if extra_scanner:
            (root / ".github" / "workflows" / "shadow.yml").write_text(
                "uses: github/codeql-action/analyze@0123456789012345678901234567890123456789\n",
                encoding="utf-8",
            )

    def test_codeql_exact_authority_passes(self):
        temp, root = self.make_root()
        self.addCleanup(temp.cleanup)
        self.write_codeql_fixture(root)
        self.assertEqual(AUDIT.check_codeql_authority(root), [])

    def test_second_codeql_producer_fails(self):
        temp, root = self.make_root()
        self.addCleanup(temp.cleanup)
        self.write_codeql_fixture(root, extra_scanner=True)
        findings = AUDIT.check_codeql_authority(root)
        self.assertTrue(any("exactly codeql.yml" in item.details for item in findings))

    def test_codeql_required_promotion_without_policy_review_fails(self):
        temp, root = self.make_root()
        self.addCleanup(temp.cleanup)
        self.write_codeql_fixture(root, classification="required")
        findings = AUDIT.check_codeql_authority(root)
        self.assertTrue(any("classification" in item.details for item in findings))

    def test_supply_chain_reporter_removal_fails(self):
        temp, root = self.make_root()
        self.addCleanup(temp.cleanup)
        path = root / ".github" / "workflows" / "security.yml"
        path.write_text("schedule:\nworkflow_dispatch:\nEmbarkStudios/cargo-deny-action@x\narguments: --all-features\n", encoding="utf-8")
        findings = AUDIT.check_supply_chain_authority(root)
        self.assertTrue(any("report-failure" in item.details for item in findings))

    def test_codeowners_sensitive_path_removal_fails(self):
        temp, root = self.make_root()
        self.addCleanup(temp.cleanup)
        for rel in (
            "CODE_OF_CONDUCT.md",
            "SECURITY.md",
            ".github/CODEOWNERS",
            ".github/workflows/issue-labeler.yml",
            ".github/workflows/pr-labeler.yml",
        ):
            path = root / rel
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("", encoding="utf-8")
        findings = AUDIT.check_governance_docs_and_ownership(root)
        self.assertTrue(any(item.control == "codeowners" for item in findings))
        self.assertTrue(any(item.control == "security-reporting" for item in findings))


class WorkflowBoundaryTests(unittest.TestCase):
    def test_production_dispatch_is_live_only_and_commit_pinned(self):
        workflow = (
            HERE.parents[1] / ".github" / "workflows" / "hardening-drift-audit.yml"
        ).read_text(encoding="utf-8")
        self.assertIn("workflow_dispatch:", workflow)
        self.assertIn("--mode live", workflow)
        self.assertNotIn("inputs:", workflow)
        self.assertNotIn("policy-drift", workflow)
        self.assertNotIn("infrastructure-failure", workflow)
        self.assertIn("ref: ${{ github.sha }}", workflow)
        self.assertNotIn("ref: main", workflow)


class ReporterLifecycleTests(unittest.TestCase):
    def payload(self, classification):
        return {
            "classification": classification,
            "policy_findings": [],
            "infrastructure_failures": [],
            "manual_readback": [],
        }

    def test_create_on_first_drift(self):
        client = FakeIssueClient()
        action = REPORT.reconcile(client, self.payload("policy-drift"), "run-url")
        self.assertEqual(action, "created-issue-77")
        self.assertEqual(len(client.created), 1)
        self.assertIn(REPORT.MARKER, client.created[0][1])

    def test_update_existing_open_issue(self):
        client = FakeIssueClient([REPORT.OwnedIssue(12, "open", REPORT.MARKER)])
        action = REPORT.reconcile(client, self.payload("policy-drift"), "run-url")
        self.assertEqual(action, "updated-issue-12")
        self.assertTrue(any(item[0] == 12 and item[1] for item in client.updated))
        self.assertEqual(len(client.comments), 1)

    def test_reopen_closed_issue_on_regression(self):
        client = FakeIssueClient([REPORT.OwnedIssue(12, "closed", REPORT.MARKER)])
        REPORT.reconcile(client, self.payload("infrastructure-failure"), "run-url")
        self.assertIn((12, None, "open"), client.updated)

    def test_clean_recovery_comments_and_closes(self):
        client = FakeIssueClient([REPORT.OwnedIssue(12, "open", REPORT.MARKER)])
        action = REPORT.reconcile(client, self.payload("clean"), "run-url")
        self.assertEqual(action, "closed-recovered-issue-12")
        self.assertIn((12, None, "closed"), client.updated)
        self.assertIn("recovered to clean", client.comments[0][1])

    def test_duplicate_owned_markers_fail_closed(self):
        client = FakeIssueClient(
            [
                REPORT.OwnedIssue(12, "open", REPORT.MARKER),
                REPORT.OwnedIssue(13, "open", REPORT.MARKER),
            ]
        )
        with self.assertRaises(REPORT.ReporterError):
            REPORT.reconcile(client, self.payload("policy-drift"), "run-url")

    def test_missing_detector_output_becomes_infrastructure_failure(self):
        payload = REPORT.decode_result("")
        self.assertEqual(payload["classification"], "infrastructure-failure")

    def test_marker_search_does_not_depend_on_labels(self):
        client = REPORT.GitHubIssueClient("example/repo", "token")
        requested = []

        def fake_request(method, path, payload=None):
            requested.append((method, path, payload))
            return [{"number": 12, "state": "open", "body": REPORT.MARKER}]

        client._request = fake_request
        owned = client.find_owned(REPORT.MARKER)
        self.assertEqual([item.number for item in owned], [12])
        self.assertNotIn("labels=", requested[0][1])

    def test_malformed_clean_payload_is_rejected(self):
        encoded = base64.b64encode(json.dumps({"classification": "clean"}).encode()).decode()
        with self.assertRaises(REPORT.ReporterError):
            REPORT.decode_result(encoded)


if __name__ == "__main__":
    unittest.main()
