#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import tempfile
import unittest
from pathlib import Path

MODULE_PATH = Path(__file__).with_name("verify_license_provenance.py")
spec = importlib.util.spec_from_file_location("verify_license_provenance", MODULE_PATH)
assert spec and spec.loader
verifier = importlib.util.module_from_spec(spec)
spec.loader.exec_module(verifier)

POLICY = '''schema = 1
issue = 444
[scan]
protected_globs = ["crates/**/src/**/*", "crates/**/tests/**/*", "fixtures/**/*", "examples/**/*"]
forbidden_implementation_source_markers = ["raw.githubusercontent.com/iamgio/quarkdown/", "/src/main/kotlin/", "/src/test/kotlin/"]
[dependency]
globs = ["Cargo.toml", "Cargo.lock", ".gitmodules", "crates/**/Cargo.toml"]
forbidden_markers = ["iamgio/quarkdown", "quarkdown-core", "quarkdown-stdlib", "quarkdown-wasm"]
[fixtures]
root = "fixtures/quarkdown-conformance/cases"
[provenance]
forbidden_absolute_claims = ["All Quarkdown-compatible features are implemented based solely on:"]
required_historical_markers = ["historical implementation-source inspection"]
'''

AUDIT = '''schema = 1
[source_exposure]
production_prs = [10, 11]
reviewed_prs = [10]
pending_prs = [11]
[release_clearance]
coverage = "INCOMPLETE"
[[implementation]]
pr = 10
classification = "SOURCE_INFLUENCED_INDEPENDENT"
basis = "independent structure"
[[fixture]]
id = "one"
status = "INDEPENDENT"
[[fixture]]
id = "two"
status = "REVIEW_REQUIRED"
issue = 444
'''


class LicenseProvenanceVerifierTests(unittest.TestCase):
    def make_root(self) -> Path:
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        root = Path(tmp.name)
        (root / ".github").mkdir()
        (root / "docs/legal").mkdir(parents=True)
        (root / "crates/a/src").mkdir(parents=True)
        (root / "fixtures/quarkdown-conformance/cases/one").mkdir(parents=True)
        (root / "fixtures/quarkdown-conformance/cases/two").mkdir(parents=True)
        (root / ".github/license-provenance-policy.toml").write_text(POLICY, encoding="utf-8")
        (root / ".github/license-provenance-audit.toml").write_text(AUDIT, encoding="utf-8")
        (root / "docs/legal/PROVENANCE.md").write_text(
            "historical implementation-source inspection is recorded here\n", encoding="utf-8"
        )
        (root / "Cargo.toml").write_text("[workspace]\n", encoding="utf-8")
        (root / "Cargo.lock").write_text("# lock\n", encoding="utf-8")
        (root / "crates/a/src/lib.rs").write_text("pub fn ok() {}\n", encoding="utf-8")
        for case in ("one", "two"):
            (root / f"fixtures/quarkdown-conformance/cases/{case}/case.toml").write_text(
                f'id = "{case}"\n', encoding="utf-8"
            )
        return root

    def test_pr_mode_allows_tracked_historical_debt_but_prevents_regression(self) -> None:
        root = self.make_root()
        self.assertEqual(verifier.verify(root), [])

    def test_release_mode_fails_until_finite_clearance_conditions_are_met(self) -> None:
        root = self.make_root()
        findings = verifier.verify(root, release=True)
        self.assertTrue(any("coverage=COMPLETE" in item for item in findings))
        self.assertTrue(any("pending historical" in item for item in findings))
        self.assertTrue(any("REVIEW_REQUIRED" in item for item in findings))

    def test_protected_path_rejects_upstream_implementation_source_reference(self) -> None:
        root = self.make_root()
        (root / "crates/a/src/lib.rs").write_text(
            "// https://raw.githubusercontent.com/iamgio/quarkdown/v2.6.0/x\n", encoding="utf-8"
        )
        findings = verifier.verify(root)
        self.assertTrue(any("implementation-source marker" in item for item in findings))

    def test_dependency_surface_rejects_quarkdown_ingestion(self) -> None:
        root = self.make_root()
        (root / "Cargo.toml").write_text(
            '[dependencies]\nq = { git = "https://github.com/iamgio/quarkdown" }\n', encoding="utf-8"
        )
        findings = verifier.verify(root)
        self.assertTrue(any("dependency surface" in item for item in findings))

    def test_new_fixture_must_be_added_to_audit_ledger(self) -> None:
        root = self.make_root()
        path = root / "fixtures/quarkdown-conformance/cases/three"
        path.mkdir()
        (path / "case.toml").write_text('id = "three"\n', encoding="utf-8")
        findings = verifier.verify(root)
        self.assertTrue(any("fixture ledger mismatch" in item for item in findings))

    def test_possible_translation_blocks_release(self) -> None:
        root = self.make_root()
        audit_path = root / ".github/license-provenance-audit.toml"
        audit_path.write_text(
            AUDIT.replace("SOURCE_INFLUENCED_INDEPENDENT", "POSSIBLE_TRANSLATION"),
            encoding="utf-8",
        )
        findings = verifier.verify(root, release=True)
        self.assertTrue(any("POSSIBLE_TRANSLATION" in item for item in findings))


if __name__ == "__main__":
    unittest.main()
