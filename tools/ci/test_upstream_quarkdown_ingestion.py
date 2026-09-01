#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import json
import subprocess
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
VALIDATOR = Path(__file__).with_name("validate_upstream_tag.py")
FILTER = Path(__file__).with_name("upstream_issue_filter.jq")

spec = importlib.util.spec_from_file_location("validate_upstream_tag", VALIDATOR)
assert spec and spec.loader
validator = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = validator
spec.loader.exec_module(validator)


class UpstreamIngestionTests(unittest.TestCase):
    def test_normal_release_tag_is_accepted(self) -> None:
        validator.validate_tag("v2.6.0")

    def test_quote_bearing_tag_is_accepted_for_bound_filter_regression(self) -> None:
        validator.validate_tag("v2.6.0'probe")

    def test_control_and_structurally_invalid_tags_are_rejected(self) -> None:
        for tag in ["v2.6.0\nforged=true", "v2..6", "v2//6", "v2.6.", "v2.6.lock"]:
            with self.subTest(tag=tag), self.assertRaises(validator.TagValidationError):
                validator.validate_tag(tag)

    def test_oversize_tag_is_rejected(self) -> None:
        with self.assertRaises(validator.TagValidationError):
            validator.validate_tag("v" + "a" * 128)

    def test_quote_bearing_marker_is_data_not_program_text(self) -> None:
        tag = "v2.6.0'probe"
        validator.validate_tag(tag)
        marker = f"<!-- scribium-upstream-drift:quarkdown:{tag} -->"
        payload = [
            {"number": 17, "body": "unrelated"},
            {"number": 42, "body": f"prefix {marker} suffix"},
        ]
        completed = subprocess.run(
            ["jq", "-sr", "--arg", "marker", marker, "-f", str(FILTER)],
            input=json.dumps(payload),
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=True,
            cwd=ROOT,
        )
        self.assertEqual(completed.stdout.strip(), "42")

    def test_bound_filter_does_not_match_nearby_marker(self) -> None:
        marker = "<!-- scribium-upstream-drift:quarkdown:v2.6.0'probe -->"
        payload = [{"number": 42, "body": marker + "-different"}]
        completed = subprocess.run(
            ["jq", "-sr", "--arg", "marker", marker + "!", "-f", str(FILTER)],
            input=json.dumps(payload),
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=True,
            cwd=ROOT,
        )
        self.assertEqual(completed.stdout.strip(), "")


if __name__ == "__main__":
    unittest.main()
