#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path

MODULE_PATH = Path(__file__).with_name("verify_locked_cargo_policy.py")
spec = importlib.util.spec_from_file_location("verify_locked_cargo_policy", MODULE_PATH)
assert spec and spec.loader
mod = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = mod
spec.loader.exec_module(mod)


class LockedCargoPolicyTests(unittest.TestCase):
    def verify_text(self, text: str) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "ci.yml"
            path.write_text(text, encoding="utf-8")
            mod.verify_ci_workflow(path)

    def assert_rejected(self, text: str, pattern: str) -> None:
        with self.assertRaisesRegex(mod.LockedCargoPolicyError, pattern):
            self.verify_text(text)

    def test_locked_dependency_commands_and_fmt_pass(self):
        self.verify_text(
            """jobs:
  fmt:
    steps:
      - run: cargo fmt --all --check
  test:
    steps:
      - run: cargo test --locked --workspace --all-features
      - run: cargo clippy --locked --workspace --all-targets -- -D warnings
      - run: cargo tree --locked -p arkst-cli --no-default-features
"""
        )

    def test_unlocked_test_fails_closed(self):
        self.assert_rejected(
            """jobs:
  test:
    steps:
      - run: cargo test --workspace --all-features
""",
            r"cargo test must use --locked",
        )

    def test_unlocked_tree_inside_shell_block_fails_closed(self):
        self.assert_rejected(
            """jobs:
  test:
    steps:
      - run: |
          output="$(mktemp)"
          cargo tree -p arkst-cli > "$output"
""",
            r"cargo tree must use --locked",
        )

    def test_unknown_cargo_subcommand_fails_closed(self):
        self.assert_rejected(
            """jobs:
  test:
    steps:
      - run: cargo something --locked
""",
            r"unclassified cargo subcommand",
        )

    def test_no_dependency_resolving_command_fails_closed(self):
        self.assert_rejected(
            """jobs:
  fmt:
    steps:
      - run: cargo fmt --all --check
""",
            r"no dependency-resolving cargo commands found",
        )

    def test_current_repository_workflow_passes(self):
        root = MODULE_PATH.parents[2]
        mod.verify_repository(root)


if __name__ == "__main__":
    unittest.main()
