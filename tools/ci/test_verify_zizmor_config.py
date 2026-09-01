#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path

MODULE = Path(__file__).with_name("verify_zizmor_config.py")
spec = importlib.util.spec_from_file_location("verify_zizmor_config", MODULE)
assert spec and spec.loader
mod = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = mod
spec.loader.exec_module(mod)

VALID = """
rules:
  dangerous-triggers:
    ignore:
      # narrowly justified PRT exceptions
      - ai-review.yml
      - pr-labeler.yml
"""


class ZizmorConfigTests(unittest.TestCase):
    def verify(self, content: str) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "zizmor.yml"
            path.write_text(content, encoding="utf-8")
            mod.verify_config(path)

    def test_exact_suppression_inventory_passes(self) -> None:
        self.verify(VALID)

    def test_additional_suppression_is_rejected(self) -> None:
        broadened = VALID.replace(
            "      - pr-labeler.yml\n",
            "      - pr-labeler.yml\n      - upstream-quarkdown.yml\n",
        )
        with self.assertRaisesRegex(mod.ZizmorConfigError, "suppression policy drifted"):
            self.verify(broadened)

    def test_additional_rule_is_rejected(self) -> None:
        broadened = VALID + "  template-injection:\n    ignore:\n      - markdown-compat.yml\n"
        with self.assertRaisesRegex(mod.ZizmorConfigError, "suppression policy drifted"):
            self.verify(broadened)


if __name__ == "__main__":
    unittest.main()
