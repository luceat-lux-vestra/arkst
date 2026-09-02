#!/usr/bin/env python3
from __future__ import annotations

import copy
import importlib.util
import sys
import unittest
from pathlib import Path

MODULE = Path(__file__).with_name("verify_release_ruleset.py")
spec = importlib.util.spec_from_file_location("verify_release_ruleset", MODULE)
assert spec and spec.loader
mod = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = mod
spec.loader.exec_module(mod)

VALID = {
    "name": "Protect release tags",
    "target": "tag",
    "enforcement": "active",
    "conditions": {"ref_name": {"exclude": [], "include": ["refs/tags/v*"]}},
    "rules": [{"type": "deletion"}, {"type": "update"}],
    "bypass_actors": [],
    "current_user_can_bypass": "never",
}


class ReleaseRulesetTests(unittest.TestCase):
    def test_valid_ruleset_passes(self) -> None:
        mod.validate_ruleset(VALID)

    def test_bypass_is_rejected(self) -> None:
        value = copy.deepcopy(VALID)
        value["bypass_actors"] = [{"actor_id": 1}]
        with self.assertRaisesRegex(mod.ReleaseRulesetError, "must not have bypass"):
            mod.validate_ruleset(value)

    def test_mutable_tag_rule_is_rejected(self) -> None:
        value = copy.deepcopy(VALID)
        value["rules"] = [{"type": "deletion"}]
        with self.assertRaisesRegex(mod.ReleaseRulesetError, "deletion and update"):
            mod.validate_ruleset(value)

    def test_broader_tag_scope_is_rejected(self) -> None:
        value = copy.deepcopy(VALID)
        value["conditions"]["ref_name"]["include"] = ["refs/tags/*"]
        with self.assertRaisesRegex(mod.ReleaseRulesetError, "exactly refs/tags/v"):
            mod.validate_ruleset(value)


if __name__ == "__main__":
    unittest.main()
