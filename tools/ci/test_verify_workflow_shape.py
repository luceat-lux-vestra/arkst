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
        with self.assertRaisesRegex(mod.WorkflowShapeError, "unsupported job-key syntax"):
            self.verify_text(
                """name: CI
on:
  pull_request:
jobs:
  "fmt":
    runs-on: ubuntu-latest
"""
            )

    def test_quoted_trigger_fails_closed(self):
        with self.assertRaisesRegex(mod.WorkflowShapeError, "unsupported top-level trigger syntax"):
            self.verify_text(
                """name: CI
on:
  "pull_request":
jobs:
  fmt:
    runs-on: ubuntu-latest
"""
            )

    def test_inline_on_fails_closed(self):
        with self.assertRaisesRegex(mod.WorkflowShapeError, "inline top-level on"):
            self.verify_text(
                """name: CI
on: [pull_request]
jobs:
  fmt:
    runs-on: ubuntu-latest
"""
            )


if __name__ == "__main__":
    unittest.main()
