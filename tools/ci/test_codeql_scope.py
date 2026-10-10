#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

MODULE_PATH = Path(__file__).with_name("codeql_scope.py")
SPEC = importlib.util.spec_from_file_location("codeql_scope", MODULE_PATH)
assert SPEC and SPEC.loader
SCOPE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SCOPE)


class CodeqlScopeTests(unittest.TestCase):
    def assert_case(self, records, count, expected):
        actual, _ = SCOPE.classify(records, count)
        self.assertEqual(actual, expected)

    def test_non_rust_paths_do_not_trigger_rust(self):
        self.assert_case(
            [
                {"status": "modified", "filename": "docs/README.md"},
                {"status": "modified", "filename": ".github/workflows/ci.yml"},
            ],
            2,
            False,
        )

    def test_rust_and_cargo_inputs_trigger(self):
        paths = [
            "crates/arkst-core/src/lib.rs",
            "Cargo.lock",
            "crates/arkst-core/Cargo.toml",
            "crates/arkst-core/build.rs",
            ".cargo/config.toml",
            "rust-toolchain.toml",
        ]
        for path in paths:
            with self.subTest(path=path):
                self.assert_case([{"status": "modified", "filename": path}], 1, True)

    def test_codeql_policy_inputs_trigger(self):
        for path in sorted(SCOPE.RUST_CODEQL_POLICY_PATHS | {".github/codeql/config.yml"}):
            with self.subTest(path=path):
                self.assert_case([{"status": "modified", "filename": path}], 1, True)

    def test_rename_checks_both_paths(self):
        self.assert_case(
            [{
                "status": "renamed",
                "filename": "docs/old-lib.md",
                "previous_filename": "crates/arkst-core/src/lib.rs",
            }],
            1,
            True,
        )
        self.assert_case(
            [{
                "status": "renamed",
                "filename": "crates/arkst-core/src/new.rs",
                "previous_filename": "docs/old.md",
            }],
            1,
            True,
        )
        self.assert_case(
            [{
                "status": "renamed",
                "filename": "docs/new.md",
                "previous_filename": "docs/old.md",
            }],
            1,
            False,
        )

    def test_removed_rust_input_triggers(self):
        self.assert_case([{"status": "removed", "filename": "Cargo.lock"}], 1, True)

    def test_unknown_or_malformed_records_fail_closed(self):
        cases = [
            ([{"status": "mystery", "filename": "docs/x.md"}], 1),
            ([{}], 1),
            ([{"status": "modified", "filename": "../src/lib.rs"}], 1),
            ([{"status": "modified", "filename": "docs/x.md", "previous_filename": "docs/y.md"}], 1),
        ]
        for records, count in cases:
            with self.subTest(records=records):
                self.assert_case(records, count, True)

    def test_count_mismatch_fails_closed(self):
        self.assert_case([{"status": "modified", "filename": "docs/x.md"}], 2, True)

    def test_flatten_pages_accepts_slurp_and_single_page_shapes(self):
        row = {"status": "modified", "filename": "docs/x.md"}
        self.assertEqual(SCOPE.flatten_pages([[row]]), [row])
        self.assertEqual(SCOPE.flatten_pages([row]), [row])

    def test_bad_page_shape_fails_closed(self):
        result = SCOPE.classify_payload({"changed_files": 1}, [["bad-record"]])
        self.assertTrue(result["rust_impact"])
        self.assertEqual(result["languages"], ["actions", "rust"])
        self.assertTrue(result["reason"].startswith("metadata-error:"))

    def test_cli_input_failure_emits_full_scan(self):
        with tempfile.TemporaryDirectory() as tempdir:
            root = Path(tempdir)
            meta = root / "meta.json"
            pages = root / "pages.json"
            out = root / "result.json"
            ghout = root / "github-output.txt"

            meta.write_text("{not-json", encoding="utf-8")
            pages.write_text("[]", encoding="utf-8")

            rc = SCOPE.main([
                "--meta", str(meta),
                "--pages", str(pages),
                "--output-json", str(out),
                "--github-output", str(ghout),
            ])
            self.assertEqual(rc, 0)

            result = json.loads(out.read_text(encoding="utf-8"))
            self.assertTrue(result["rust_impact"])
            self.assertEqual(result["languages"], ["actions", "rust"])
            self.assertIn("rust_impact=true", ghout.read_text(encoding="utf-8"))
            self.assertIn('languages=["actions","rust"]', ghout.read_text(encoding="utf-8"))


# Fail-closed textual contract for the pinned workflow; no YAML code executed.
CODEQL_WORKFLOW = Path(__file__).resolve().parents[2] / ".github" / "workflows" / "codeql.yml"
PINNED_ACTION = "github/codeql-action/analyze@2892aa5e19bbd11bc0cff5427e3b750a04d9e3c2"


def validate_codeql_upload_contract(workflow: str) -> None:
    """Retain full Rust+Actions SARIF security analysis without DB upload."""
    required = (
        "language: [actions, rust]",
        "runs-on: ubuntu-24.04-arm",
        "security-events: write",
        "gh api --paginate --slurp",
        'emit_full_scan "github-api-failure"',
        "github.event_name != 'pull_request' || github.event.pull_request.draft == false",
        "types: [opened, synchronize, reopened, ready_for_review]",
        "  schedule:",
        "  workflow_dispatch:",
        "uses: github/codeql-action/init@2892aa5e19bbd11bc0cff5427e3b750a04d9e3c2",
        "build-mode: none",
    )
    for token in required:
        if token not in workflow:
            raise ValueError(f"CodeQL security scan contract missing: {token}")
    step = "      - name: Analyze\n"
    if workflow.count(step) != 1:
        raise ValueError("one authoritative CodeQL analyze step is required")
    analyze = workflow.split(step, 1)[1]
    for token in (
        "uses: " + PINNED_ACTION,
        'category: "/language:${{ matrix.language }}"',
        "upload-database: false",
        "matrix.language != 'rust' || github.event_name != 'pull_request' || steps.rust_scope.outputs.rust_impact == 'true'",
    ):
        if token not in analyze:
            raise ValueError(f"CodeQL analyze contract missing: {token}")
    if analyze.count("upload-database:") != 1:
        raise ValueError("CodeQL database artifact setting is ambiguous")


class CodeqlUploadContractTests(unittest.TestCase):
    def test_codeql_scan_retains_sarif_without_database_archive(self):
        validate_codeql_upload_contract(CODEQL_WORKFLOW.read_text(encoding="utf-8"))

    def test_adversarial_security_regressions_fail_closed(self):
        original = CODEQL_WORKFLOW.read_text(encoding="utf-8")
        mutations = [
            ("upload-database: false", "upload-database: true"),
            ("security-events: write", "security-events: read"),
            ("language: [actions, rust]", "language: [actions]"),
            (PINNED_ACTION, "github/codeql-action/analyze@main"),
            ("gh api --paginate --slurp", "gh api"),
            ('emit_full_scan "github-api-failure"', 'echo "rust_impact=false"'),
            ("build-mode: none", "build-mode: manual"),
            ("  schedule:", "  # no schedule:"),
        ]
        for before, after in mutations:
            with self.subTest(before=before):
                self.assertIn(before, original)
                with self.assertRaises(ValueError):
                    validate_codeql_upload_contract(original.replace(before, after, 1))


if __name__ == "__main__":
    unittest.main()
