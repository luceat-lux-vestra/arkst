#!/usr/bin/env python3
from __future__ import annotations

import copy
import importlib.util
import sys
import unittest
from pathlib import Path

MODULE = Path(__file__).with_name("verify_release_contract.py")
spec = importlib.util.spec_from_file_location("verify_release_contract", MODULE)
assert spec and spec.loader
mod = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = mod
spec.loader.exec_module(mod)

ROOT = Path(__file__).resolve().parents[2]
POLICY = ROOT / ".github" / "distribution-policy.toml"


class ReleaseContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.policy = mod.load_policy(POLICY)
        cls.metadata = mod.load_metadata(ROOT)
        cls.version = next(
            package["version"]
            for package in cls.metadata["packages"]
            if package["name"] == "arkst-cli"
        )

    def test_current_contract_passes(self) -> None:
        plan = mod.validate_contract(self.policy, self.metadata, f"v{self.version}")
        self.assertEqual(plan["version"], self.version)
        self.assertEqual(
            [item["target"] for item in plan["matrix"]["include"]],
            list(mod.TARGETS),
        )

    def test_wrong_tag_is_rejected(self) -> None:
        with self.assertRaisesRegex(mod.ReleaseContractError, "does not match arkst-cli version"):
            mod.validate_contract(self.policy, self.metadata, "v9.9.9")

    def test_non_semver_tag_is_rejected(self) -> None:
        with self.assertRaisesRegex(mod.ReleaseContractError, "SemVer-shaped"):
            mod.validate_contract(self.policy, self.metadata, "release-1")

    def test_missing_target_is_rejected(self) -> None:
        policy = copy.deepcopy(self.policy)
        policy["channels"]["github_release"]["targets"].pop()
        with self.assertRaisesRegex(mod.ReleaseContractError, "targets must be exactly"):
            mod.validate_contract(policy, self.metadata)

    def test_duplicate_target_is_rejected(self) -> None:
        policy = copy.deepcopy(self.policy)
        targets = policy["channels"]["github_release"]["targets"]
        targets[-1] = targets[0]
        with self.assertRaisesRegex(mod.ReleaseContractError, "must not contain duplicates"):
            mod.validate_contract(policy, self.metadata)

    def test_publishable_cargo_package_is_rejected(self) -> None:
        metadata = copy.deepcopy(self.metadata)
        package = next(item for item in metadata["packages"] if item["name"] == "arkst-cli")
        package["publish"] = None
        with self.assertRaisesRegex(mod.ReleaseContractError, "must remain non-publishable"):
            mod.validate_contract(self.policy, metadata)

    def test_wrong_binary_identity_is_rejected(self) -> None:
        policy = copy.deepcopy(self.policy)
        policy["channels"]["github_release"]["binary"] = "other"
        with self.assertRaisesRegex(mod.ReleaseContractError, "binary must be arkst"):
            mod.validate_contract(policy, self.metadata)


if __name__ == "__main__":
    unittest.main()
