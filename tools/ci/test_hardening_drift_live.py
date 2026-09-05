#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import sys
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


AUDIT = load("arkst_hardening_drift_audit_readback_test", "hardening_drift_audit.py")
LIVE = load("arkst_hardening_drift_live_test", "hardening_drift_live.py")


def minimum_visible_repository() -> dict[str, object]:
    return {
        "visibility": "public",
        "default_branch": "main",
        "has_wiki": False,
        "has_discussions": False,
    }


class RepositoryReadbackBoundaryTests(unittest.TestCase):
    def test_omitted_merge_policy_fields_become_explicit_manual_readback(self):
        normalized, manual = LIVE.normalize_repository_settings(minimum_visible_repository())

        self.assertEqual(AUDIT.check_repository_settings(normalized), [])
        self.assertEqual(
            {item["control"] for item in manual},
            {
                f"repository-settings.{key}"
                for key in LIVE.PRIVILEGE_SENSITIVE_REPOSITORY_SETTINGS
            },
        )

    def test_observable_merge_policy_drift_is_not_masked(self):
        repository = minimum_visible_repository()
        repository["allow_merge_commit"] = True
        normalized, manual = LIVE.normalize_repository_settings(repository)

        findings = AUDIT.check_repository_settings(normalized)
        self.assertTrue(any("allow_merge_commit" in item.details for item in findings))
        self.assertNotIn(
            "repository-settings.allow_merge_commit",
            {item["control"] for item in manual},
        )

    def test_missing_stable_repository_field_remains_fail_closed(self):
        repository = minimum_visible_repository()
        del repository["visibility"]
        normalized, _ = LIVE.normalize_repository_settings(repository)

        with self.assertRaises(AUDIT.AuditInfrastructureError):
            AUDIT.check_repository_settings(normalized)

    def test_live_adapter_integrates_manual_inventory_and_restores_checker(self):
        original_base = LIVE.BASE_LIVE_CHECKS
        original_check = LIVE.audit.check_repository_settings

        def fake_base_live_checks(_client, _run):
            findings = LIVE.audit.check_repository_settings(minimum_visible_repository())
            return findings, [{"control": "existing-manual", "reason": "fixture"}]

        LIVE.BASE_LIVE_CHECKS = fake_base_live_checks
        try:
            findings, manual = LIVE.live_checks(None)
        finally:
            LIVE.BASE_LIVE_CHECKS = original_base

        self.assertEqual(findings, [])
        controls = {item["control"] for item in manual}
        self.assertIn("existing-manual", controls)
        self.assertEqual(
            {
                control
                for control in controls
                if control.startswith("repository-settings.")
            },
            {
                f"repository-settings.{key}"
                for key in LIVE.PRIVILEGE_SENSITIVE_REPOSITORY_SETTINGS
            },
        )
        self.assertIs(LIVE.audit.check_repository_settings, original_check)


if __name__ == "__main__":
    unittest.main()
