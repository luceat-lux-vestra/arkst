#!/usr/bin/env python3
from __future__ import annotations

import copy
import importlib.util
import re
import sys
import tempfile
import unittest
from pathlib import Path
from typing import Any


MODULE_PATH = Path(__file__).with_name("verify_distribution_policy.py")
spec = importlib.util.spec_from_file_location("verify_distribution_policy", MODULE_PATH)
assert spec and spec.loader
mod = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = mod
spec.loader.exec_module(mod)


ROOT = Path(__file__).resolve().parents[2]
POLICY = ROOT / ".github" / "distribution-policy.toml"


class DistributionPolicyTests(unittest.TestCase):
    def metadata(self) -> dict[str, Any]:
        return copy.deepcopy(dict(mod.load_metadata(ROOT)))

    def verify(
        self,
        metadata: dict[str, Any] | None = None,
        policy_text: str | None = None,
    ) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            policy_path = Path(tmp) / "distribution-policy.toml"
            policy_path.write_text(
                policy_text if policy_text is not None else POLICY.read_text(encoding="utf-8"),
                encoding="utf-8",
            )
            mod.verify_policy(
                ROOT,
                policy_path,
                metadata if metadata is not None else self.metadata(),
            )

    def reject(
        self,
        pattern: str,
        metadata: dict[str, Any] | None = None,
        policy_text: str | None = None,
    ) -> None:
        with self.assertRaisesRegex(mod.DistributionPolicyError, pattern):
            self.verify(metadata, policy_text)

    @staticmethod
    def package(metadata: dict[str, Any], name: str) -> dict[str, Any]:
        return next(package for package in metadata["packages"] if package["name"] == name)

    @staticmethod
    def package_block(policy_text: str, name: str) -> tuple[int, int, str]:
        marker = f'[[packages]]\nname = "{name}"'
        start = policy_text.index(marker)
        next_start = policy_text.find("[[packages]]", start + len(marker))
        end = len(policy_text) if next_start == -1 else next_start
        return start, end, policy_text[start:end]

    @classmethod
    def set_package_field(cls, policy_text: str, name: str, field: str, value: str) -> str:
        start, end, block = cls.package_block(policy_text, name)
        updated, count = re.subn(
            rf"^{re.escape(field)} = .*?$",
            f"{field} = {value}",
            block,
            flags=re.MULTILINE,
        )
        if count != 1:
            raise AssertionError(f"expected one {field} field in {name}")
        return policy_text[:start] + updated + policy_text[end:]

    @classmethod
    def remove_package(cls, policy_text: str, name: str) -> str:
        start, end, _ = cls.package_block(policy_text, name)
        return policy_text[:start] + policy_text[end:]

    def test_repository_policy_passes_through_production_path(self) -> None:
        metadata = self.metadata()
        mod.verify_policy(ROOT, POLICY, metadata)
        self.assertEqual(len(metadata["packages"]), 16)
        self.assertTrue(all(package["publish"] == [] for package in metadata["packages"]))

    def test_github_release_cli_contract_is_enabled(self) -> None:
        policy = mod.load_policy(POLICY)
        self.assertEqual(policy["decision"], "github-release-cli")
        self.assertTrue(policy["channels"]["github_release"]["publishable"])
        self.assertEqual(
            policy["channels"]["github_release"]["publication_channel"],
            "github-releases",
        )
        self.assertTrue(policy["cli"]["github_release"])
        self.assertFalse(policy["cli"]["cargo_install"])

    def test_disabling_github_release_contract_is_rejected(self) -> None:
        policy_text = POLICY.read_text(encoding="utf-8").replace(
            "[channels.github_release]\npublishable = true",
            "[channels.github_release]\npublishable = false",
            1,
        )
        self.reject(
            r"policy.channels.github_release.publishable must be true",
            policy_text=policy_text,
        )

    def test_cli_wrong_publication_channel_is_rejected(self) -> None:
        policy_text = self.set_package_field(
            POLICY.read_text(encoding="utf-8"),
            "arkst-cli",
            "publication_channel",
            '"crates.io"',
        )
        self.reject(
            r"arkst-cli.publication_channel must be github-releases",
            policy_text=policy_text,
        )

    def test_accidental_publish_enable_is_rejected_by_cargo_metadata(self) -> None:
        metadata = self.metadata()
        self.package(metadata, "arkst-core")["publish"] = None
        self.reject(r"arkst-core: Cargo metadata publish=None", metadata=metadata)

    def test_new_workspace_package_drift_is_rejected(self) -> None:
        metadata = self.metadata()
        new_package = copy.deepcopy(metadata["packages"][0])
        new_package["name"] = "arkst-unlisted"
        new_package["id"] = "path+file:///workspace/arkst-unlisted#0.0.1"
        metadata["packages"].append(new_package)
        metadata["workspace_members"].append(new_package["id"])
        self.reject(r"missing from policy: arkst-unlisted", metadata=metadata)

    def test_stale_inventory_entry_is_rejected(self) -> None:
        policy_text = POLICY.read_text(encoding="utf-8")
        _, _, source_block = self.package_block(policy_text, "arkst-upstream-watch")
        stale_block = source_block.replace(
            'name = "arkst-upstream-watch"',
            'name = "arkst-z-stale"',
            1,
        ).replace(
            'manifest = "tools/upstream-watch/Cargo.toml"',
            'manifest = "tools/z-stale/Cargo.toml"',
            1,
        )
        self.reject(
            r"stale policy entries: arkst-z-stale",
            policy_text=policy_text + "\n" + stale_block,
        )

    def test_policy_publishable_true_is_rejected_under_current_decision(self) -> None:
        policy_text = self.set_package_field(
            POLICY.read_text(encoding="utf-8"),
            "arkst-cli",
            "publishable",
            "true",
        )
        self.reject(r"arkst-cli.publishable must be false", policy_text=policy_text)

    def test_internal_tool_publish_enable_is_rejected(self) -> None:
        metadata = self.metadata()
        self.package(metadata, "arkst-markdown-compat")["publish"] = None
        self.reject(
            r"arkst-markdown-compat: Cargo metadata publish=None",
            metadata=metadata,
        )

    def test_internal_tool_omission_is_rejected(self) -> None:
        policy_text = self.remove_package(
            POLICY.read_text(encoding="utf-8"), "arkst-upstream-watch"
        )
        self.reject(
            r"missing from policy: arkst-upstream-watch",
            policy_text=policy_text,
        )

    def test_internal_tool_reclassification_is_rejected(self) -> None:
        policy_text = self.set_package_field(
            POLICY.read_text(encoding="utf-8"),
            "arkst-upstream-watch",
            "distribution",
            '"compiler-library"',
        )
        self.reject(
            r"repository tools must be classified as internal-tooling",
            policy_text=policy_text,
        )

    def test_manifest_path_disagreement_is_rejected(self) -> None:
        policy_text = self.set_package_field(
            POLICY.read_text(encoding="utf-8"),
            "arkst-cli",
            "manifest",
            '"crates/arkst-core/Cargo.toml"',
        )
        self.reject(r"arkst-cli.manifest disagrees", policy_text=policy_text)

    def test_binary_classification_disagreement_is_rejected(self) -> None:
        policy_text = self.set_package_field(
            POLICY.read_text(encoding="utf-8"),
            "arkst-cli",
            "binary_targets",
            '["wrong-binary"]',
        )
        self.reject(r"arkst-cli.binary_targets disagrees", policy_text=policy_text)

    def test_unknown_wasm_classification_is_rejected(self) -> None:
        policy_text = POLICY.read_text(encoding="utf-8").replace(
            'distribution_status = "compile-only"',
            'distribution_status = "distributed"',
            1,
        )
        self.reject(r"unknown status 'distributed'", policy_text=policy_text)


if __name__ == "__main__":
    unittest.main()
