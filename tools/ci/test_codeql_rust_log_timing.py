#!/usr/bin/env python3
"""Adversarial tests for the offline Rust CodeQL log-timing evidence parser."""
from __future__ import annotations

from datetime import datetime, timedelta, timezone
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

MODULE_PATH = Path(__file__).with_name("codeql_rust_log_timing.py")
spec = importlib.util.spec_from_file_location("codeql_rust_log_timing", MODULE_PATH)
assert spec and spec.loader
mod = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mod)


def fixture() -> str:
    origin = datetime(2026, 10, 10, 0, 0, tzinfo=timezone.utc)
    entries = []
    def add(offset: int, text: str):
        ts = (origin + timedelta(seconds=offset)).strftime("%Y-%m-%dT%H:%M:%S.%fZ")
        entries.append((offset, f"{ts} {text}"))

    add(0, "Current runner version: '2.337.0'")
    add(2, "Downloaded CodeQL into /opt/hostedtoolcache/CodeQL/2.27.2/arm64/codeql/")
    add(3, "Using query pack /qlpacks/codeql/rust-queries/0.1.44/")
    add(10, "##[group]Extracting rust")
    add(30, "##[group]Finalizing rust")
    add(40, "##[group]Running queries for rust")
    for n in range(1, 38):
        # First 13 queries complete quickly, followed by an intentional
        # large completion gap. The gap is NOT a per-query time.
        delta = n if n <= 13 else n + 100
        add(40 + delta, f"[{n}/37 eval {delta}s] Evaluation done; writing results to codeql/rust-queries/queries/diagnostics/PublicQuery.bqrs.")
    add(183, "Shutting down query evaluator.")
    add(184, "| Total number of Rust files that were extracted with errors   |     0 |")
    add(185, "| Total number of Rust files that were extracted without error |   167 |")
    add(190, "Analysis upload status is complete.")
    add(191, "CodeQL job status was success.")
    add(192, "Complete job")
    return "\n".join(t for _, t in sorted(entries, key=lambda x: x[0])) + "\n"


class TimingTests(unittest.TestCase):
    def setUp(self):
        self.source = fixture()

    def fails(self, content):
        with self.assertRaises(mod.IncompleteEvidence):
            mod.parse_log(content)

    def test_success_metrics_are_bounded_and_fixed(self):
        r = mod.parse_log(self.source)
        self.assertEqual(r["queries_completed"], 37)
        self.assertEqual(r["seconds"]["extraction_group"], 20.0)
        self.assertEqual(r["seconds"]["finalize_group"], 10.0)
        self.assertEqual(r["max_completion_gap_after_ordinal"], 13)
        self.assertEqual(r["seconds"]["max_gap_between_query_completions"], 101.0)
        self.assertEqual(r["seconds"]["log_span"], 192.0)
        self.assertEqual(r["seconds"]["query_group_until_evaluator_shutdown"], 143.0)
        self.assertEqual(r["seconds"]["shutdown_to_upload_confirmed"], 7.0)

    def test_privacy_no_source_or_payload_echo(self):
        poisoned = self.source.replace(
            "Current runner version:",
            "secret=TOKEN_PRIVACY_SENTINEL /private/home/foo.rs Current runner version:",
        )
        r = mod.parse_log(poisoned)
        self.assertNotIn("TOKEN_PRIVACY_SENTINEL", json.dumps(r))
        self.assertNotIn("/private/home/", json.dumps(r))
        self.assertNotIn("PublicQuery", json.dumps(r))

    def test_missing_query_fails(self):
        self.fails("\n".join(l for l in self.source.splitlines() if "[7/37 eval " not in l))

    def test_changed_denominator_fails(self):
        self.fails(self.source.replace("[36/37 eval", "[36/36 eval"))

    def test_duplicate_query_ordinal_fails(self):
        self.fails(self.source.replace("[32/37 eval", "[31/37 eval"))

    def test_missing_phase_fails(self):
        self.fails(self.source.replace("##[group]Finalizing rust", "another step"))

    def test_duplicate_phase_fails(self):
        self.fails(self.source + self.source.splitlines()[3] + "\n")

    def test_wrong_cli_fails(self):
        self.fails(self.source.replace("/CodeQL/2.27.2/", "/CodeQL/2.27.3/"))

    def test_wrong_pack_fails(self):
        self.fails(self.source.replace("rust-queries/0.1.44", "rust-queries/0.1.45"))

    def test_ambiguous_multiple_versions_fails(self):
        self.fails(self.source + "2026-10-10T00:03:13.000000Z /CodeQL/2.26.4/\n")

    def test_wrong_extracted_count_fails(self):
        self.fails(self.source.replace("   167 |", "   166 |"))

    def test_errors_nonzero_fails(self):
        self.fails(self.source.replace("     0 |", "     1 |"))

    def test_missing_success_fails(self):
        self.fails(self.source.replace("CodeQL job status was success.", "CodeQL job status was failure."))

    def test_out_of_order_events_fail(self):
        lines = self.source.splitlines()
        ix_start = next(i for i, l in enumerate(lines) if "##[group]Extracting rust" in l)
        ix_query = next(i for i, l in enumerate(lines) if "##[group]Running queries for rust" in l)
        lines[ix_start] = lines[ix_start].replace("Extracting rust", "Running queries for rust")
        lines[ix_query] = lines[ix_query].replace("Running queries for rust", "Extracting rust")
        self.fails("\n".join(lines))

    def test_nonmonotonic_timestamps_fail(self):
        lines = self.source.splitlines()
        lines[5] = lines[5].replace("00:00:40.", "00:00:01.")
        self.fails("\n".join(lines))

    def test_extra_query_fails(self):
        extra = "2026-10-10T00:03:11.000000Z [38/37 eval 157s] Evaluation done; writing results to codeql/rust-queries/queries/summary/Fake.bqrs.\n"
        self.fails(self.source + extra)

    def test_missing_file_coverage_fails(self):
        self.fails("\n".join(l for l in self.source.splitlines() if "without error" not in l))

    def test_oversized_payload_fails(self):
        self.fails(self.source + "x" * (mod.MAX_BYTES + 1))

    def test_cli_failure_outputs_no_json_or_raw_log(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "job.log"
            path.write_text(self.source.replace("2.27.2", "SECRET-SENTINEL"))
            result = subprocess.run([sys.executable, "-S", str(MODULE_PATH), str(path)],
                                    capture_output=True, text=True, check=False)
            self.assertEqual(result.returncode, 2)
            self.assertEqual(result.stdout, "")
            self.assertIn("INCOMPLETE_DO_NOT_USE", result.stderr)
            self.assertNotIn("SECRET-SENTINEL", result.stderr)

    def test_cli_success_outputs_json_only(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "job.log"
            path.write_text(self.source)
            result = subprocess.run([sys.executable, "-S", str(MODULE_PATH), str(path)],
                                    capture_output=True, text=True, check=False)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(json.loads(result.stdout)["queries_completed"], 37)
            self.assertEqual(result.stderr, "")


if __name__ == "__main__":
    unittest.main()
