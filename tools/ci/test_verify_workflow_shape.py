#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path

MODULE_PATH = Path(__file__).with_name("verify_workflow_shape.py")
spec = importlib.util.spec_from_file_location("verify_workflow_shape", MODULE_PATH)
assert spec and spec.loader
mod = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = mod
spec.loader.exec_module(mod)


class WorkflowShapeTests(unittest.TestCase):
    def verify_text(self, text: str) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "workflow.yml"
            path.write_text(text, encoding="utf-8")
            mod.verify_workflow(path)

    def assert_rejected(self, text: str, pattern: str) -> None:
        with self.assertRaisesRegex(mod.WorkflowShapeError, pattern):
            self.verify_text(text)

    def test_supported_shape_passes(self):
        self.verify_text(
            """name: CI
on:
  pull_request:
    branches: [main]
jobs:
  fmt:
    name: fmt
    runs-on: ubuntu-latest
"""
        )

    def test_quoted_job_key_fails_closed(self):
        self.assert_rejected(
            """name: CI
on:
  pull_request:
jobs:
  "fmt":
    runs-on: ubuntu-latest
""",
            "unsupported job-key syntax",
        )

    def test_quoted_trigger_fails_closed(self):
        self.assert_rejected(
            """name: CI
on:
  "pull_request":
jobs:
  fmt:
    runs-on: ubuntu-latest
""",
            "unsupported top-level trigger syntax",
        )

    def test_inline_on_fails_closed(self):
        self.assert_rejected(
            """name: CI
on: [pull_request]
jobs:
  fmt:
    runs-on: ubuntu-latest
""",
            "inline top-level on",
        )

    def test_inline_trigger_mapping_fails_closed(self):
        self.assert_rejected(
            """name: CI
on:
  pull_request: {paths: ["crates/**"]}
jobs:
  fmt:
    runs-on: ubuntu-latest
""",
            "unsupported top-level trigger syntax",
        )

    def test_quoted_trigger_mapping_key_fails_closed(self):
        self.assert_rejected(
            """name: CI
on:
  pull_request:
    "paths": ["crates/**"]
jobs:
  fmt:
    runs-on: ubuntu-latest
""",
            "unsupported trigger mapping syntax",
        )

    def test_quoted_job_if_fails_closed(self):
        self.assert_rejected(
            """name: CI
on:
  pull_request:
jobs:
  fmt:
    name: fmt
    "if": github.actor != 'nobody'
    runs-on: ubuntu-latest
""",
            "unsupported job mapping syntax",
        )

    def test_yaml_merge_key_fails_closed(self):
        self.assert_rejected(
            """name: CI
on:
  pull_request:
jobs:
  fmt:
    <<: *defaults
    name: fmt
    runs-on: ubuntu-latest
""",
            "unsupported job mapping syntax",
        )

    def test_tab_indentation_fails_closed(self):
        self.assert_rejected(
            "name: CI\non:\n  pull_request:\njobs:\n  fmt:\n\truns-on: ubuntu-latest\n",
            "tab-indented YAML",
        )


if __name__ == "__main__":
    unittest.main()
