#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path

MODULE_PATH = Path(__file__).with_name("verify_cargo_graph_ownership.py")
spec = importlib.util.spec_from_file_location("verify_cargo_graph_ownership", MODULE_PATH)
assert spec and spec.loader
mod = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = mod
spec.loader.exec_module(mod)

PATCH = '''[patch.crates-io.citationberg]
git = "https://github.com/typst/citationberg.git"
rev = "06a591e2f237d25e1dfdedac3f3d1494c496c52d"
'''

ACTION = "EmbarkStudios/cargo-deny-action@3c6349835b2b7b196a839186cb8b78e02f7b5f25"
CI_WORKFLOW = f'''jobs:
  deny:
    name: license
    steps:
      - uses: {ACTION}
        with:
          command: check
          arguments: --all-features
      - uses: {ACTION}
        with:
          manifest-path: tools/spikes/typst-in-process/Cargo.toml
          command: check
          arguments: --all-features
          command-arguments: advisories sources
'''
SECURITY_WORKFLOW = f'''jobs:
  audit:
    steps:
      - uses: {ACTION}
        with:
          command: check
          arguments: --all-features
      - uses: {ACTION}
        with:
          manifest-path: tools/spikes/typst-in-process/Cargo.toml
          command: check
          arguments: --all-features
          command-arguments: advisories sources
'''


class CargoGraphOwnershipTests(unittest.TestCase):
    def make_repo(self) -> Path:
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        root = Path(tmp.name)
        (root / ".github/workflows").mkdir(parents=True)
        (root / "crates/app").mkdir(parents=True)
        (root / "Cargo.toml").write_text(
            '[workspace]\nmembers = ["crates/app"]\nexclude = [\n'
            '  "tools/spikes/typst-in-process",\n'
            '  "tools/spikes/typst-subprocess",\n]\n\n'
            + PATCH,
            encoding="utf-8",
        )
        (root / "crates/app/Cargo.toml").write_text(
            '[package]\nname = "app"\nversion = "0.1.0"\n', encoding="utf-8"
        )
        manifests = {
            "markdown-final-feasibility": '[package]\nname="a"\nversion="0.1.0"\npublish=false\n\n[workspace]\n\n[dependencies]\nmarkdown="=1.0.0"\n',
            "markdown-maintained-substrate-check": '[package]\nname="b"\nversion="0.1.0"\npublish=false\n\n[workspace]\n\n[dependencies]\nrushdown="=0.18.0"\n',
            "markdown-substrate": '[package]\nname="c"\nversion="0.1.0"\npublish=false\n\n[workspace]\n\n[dependencies]\nmarkdown-it="=0.6.1"\n',
            "rushdown-safety-gate": '[package]\nname="d"\nversion="0.1.0"\npublish=false\n\n[workspace]\n\n[dependencies]\nrushdown="=0.18.0"\n',
            "typst-in-process": '[package]\nname="e"\nversion="0.1.0"\npublish=false\n\n[dependencies]\ntypst="=0.15.1"\n\n'
            + PATCH,
            "typst-subprocess": '[package]\nname="f"\nversion="0.1.0"\npublish=false\n',
        }
        for name, content in manifests.items():
            directory = root / "tools/spikes" / name
            directory.mkdir(parents=True)
            (directory / "Cargo.toml").write_text(content, encoding="utf-8")
        (root / ".github/workflows/ci.yml").write_text(CI_WORKFLOW, encoding="utf-8")
        (root / ".github/workflows/security.yml").write_text(
            SECURITY_WORKFLOW, encoding="utf-8"
        )
        return root

    def test_contained_repository_passes(self):
        mod.verify_repository(self.make_repo())

    def test_unregistered_manifest_fails_closed(self):
        root = self.make_repo()
        path = root / "tools/spikes/rogue/Cargo.toml"
        path.parent.mkdir(parents=True)
        path.write_text(
            '[package]\nname="rogue"\nversion="0.1.0"\npublish=false\n\n[workspace]\n',
            encoding="utf-8",
        )
        with self.assertRaisesRegex(
            mod.CargoGraphOwnershipError, "unregistered research Cargo manifest"
        ):
            mod.verify_repository(root)

    def test_unisolated_manifest_fails_closed(self):
        root = self.make_repo()
        path = root / "Cargo.toml"
        path.write_text(
            path.read_text(encoding="utf-8").replace(
                '  "tools/spikes/typst-subprocess",\n', ""
            ),
            encoding="utf-8",
        )
        with self.assertRaisesRegex(
            mod.CargoGraphOwnershipError, "must declare containment"
        ):
            mod.verify_repository(root)

    def test_publishable_research_package_fails_closed(self):
        root = self.make_repo()
        path = root / "tools/spikes/typst-subprocess/Cargo.toml"
        path.write_text(
            path.read_text(encoding="utf-8").replace("publish=false\n", ""),
            encoding="utf-8",
        )
        with self.assertRaisesRegex(
            mod.CargoGraphOwnershipError, "publish = false"
        ):
            mod.verify_repository(root)

    def test_non_exact_dependency_fails_closed(self):
        root = self.make_repo()
        path = root / "tools/spikes/typst-in-process/Cargo.toml"
        path.write_text(
            path.read_text(encoding="utf-8").replace(
                'typst="=0.15.1"', 'typst="0.15.1"'
            ),
            encoding="utf-8",
        )
        with self.assertRaisesRegex(mod.CargoGraphOwnershipError, "exact full version"):
            mod.verify_repository(root)

    def test_partial_exact_dependency_fails_closed(self):
        root = self.make_repo()
        path = root / "tools/spikes/typst-in-process/Cargo.toml"
        path.write_text(
            path.read_text(encoding="utf-8").replace(
                'typst="=0.15.1"', 'typst="=0.15"'
            ),
            encoding="utf-8",
        )
        with self.assertRaisesRegex(mod.CargoGraphOwnershipError, "exact full version"):
            mod.verify_repository(root)

    def test_research_lockfile_fails_closed(self):
        root = self.make_repo()
        (root / "tools/spikes/typst-in-process/Cargo.lock").write_text(
            "version=4\n", encoding="utf-8"
        )
        with self.assertRaisesRegex(
            mod.CargoGraphOwnershipError, "must not commit lockfiles"
        ):
            mod.verify_repository(root)

    def test_production_path_dependency_fails_closed(self):
        root = self.make_repo()
        path = root / "crates/app/Cargo.toml"
        path.write_text(
            path.read_text(encoding="utf-8")
            + '\n[dependencies]\nspike={path="../../tools/spikes/typst-subprocess"}\n',
            encoding="utf-8",
        )
        with self.assertRaisesRegex(
            mod.CargoGraphOwnershipError, "must not depend on or override from research path"
        ):
            mod.verify_repository(root)

    def test_production_patch_path_fails_closed(self):
        root = self.make_repo()
        path = root / "Cargo.toml"
        path.write_text(
            path.read_text(encoding="utf-8")
            + '\n[patch."https://example.invalid/index"]\nspike={path="tools/spikes/typst-subprocess"}\n',
            encoding="utf-8",
        )
        with self.assertRaisesRegex(
            mod.CargoGraphOwnershipError, "must not depend on or override from research path"
        ):
            mod.verify_repository(root)

    def test_patch_drift_fails_closed(self):
        root = self.make_repo()
        path = root / "tools/spikes/typst-in-process/Cargo.toml"
        path.write_text(
            path.read_text(encoding="utf-8").replace(
                "06a591e2f237d25e1dfdedac3f3d1494c496c52d",
                "16a591e2f237d25e1dfdedac3f3d1494c496c52d",
            ),
            encoding="utf-8",
        )
        with self.assertRaisesRegex(
            mod.CargoGraphOwnershipError, "must exactly match production"
        ):
            mod.verify_repository(root)

    def test_required_license_name_drift_fails_closed(self):
        root = self.make_repo()
        path = root / ".github/workflows/ci.yml"
        path.write_text(
            path.read_text(encoding="utf-8").replace("    name: license\n", "    name: audit\n"),
            encoding="utf-8",
        )
        with self.assertRaisesRegex(mod.CargoGraphOwnershipError, "must produce 'license'"):
            mod.verify_repository(root)

    def test_cargo_deny_action_identity_drift_fails_closed(self):
        root = self.make_repo()
        path = root / ".github/workflows/ci.yml"
        path.write_text(
            path.read_text(encoding="utf-8").replace(
                "3c6349835b2b7b196a839186cb8b78e02f7b5f25",
                "4c6349835b2b7b196a839186cb8b78e02f7b5f25",
                1,
            ),
            encoding="utf-8",
        )
        with self.assertRaisesRegex(
            mod.CargoGraphOwnershipError, "must use exact action ref"
        ):
            mod.verify_repository(root)

    def test_missing_typst_audit_fails_closed(self):
        root = self.make_repo()
        path = root / ".github/workflows/ci.yml"
        path.write_text(
            f'''jobs:
  deny:
    name: license
    steps:
      - uses: {ACTION}
        with:
          command: check
          arguments: --all-features
''',
            encoding="utf-8",
        )
        with self.assertRaisesRegex(
            mod.CargoGraphOwnershipError, "audited research cargo-deny"
        ):
            mod.verify_repository(root)

    def test_conditional_research_audit_fails_closed(self):
        root = self.make_repo()
        path = root / ".github/workflows/ci.yml"
        path.write_text(
            path.read_text(encoding="utf-8").replace(
                f"      - uses: {ACTION}\n        with:\n          manifest-path: tools/spikes/typst-in-process/Cargo.toml",
                f"      - uses: {ACTION}\n        if: false\n        with:\n          manifest-path: tools/spikes/typst-in-process/Cargo.toml",
            ),
            encoding="utf-8",
        )
        with self.assertRaisesRegex(
            mod.CargoGraphOwnershipError, "research cargo-deny authority must not be conditional"
        ):
            mod.verify_repository(root)

    def test_research_continue_on_error_fails_closed(self):
        root = self.make_repo()
        path = root / ".github/workflows/ci.yml"
        path.write_text(
            path.read_text(encoding="utf-8").replace(
                f"      - uses: {ACTION}\n        with:\n          manifest-path: tools/spikes/typst-in-process/Cargo.toml",
                f"      - uses: {ACTION}\n        continue-on-error: true\n        with:\n          manifest-path: tools/spikes/typst-in-process/Cargo.toml",
            ),
            encoding="utf-8",
        )
        with self.assertRaisesRegex(
            mod.CargoGraphOwnershipError, "research cargo-deny authority must not allow failure"
        ):
            mod.verify_repository(root)

    def test_conditional_production_audit_fails_closed(self):
        root = self.make_repo()
        path = root / ".github/workflows/ci.yml"
        path.write_text(
            path.read_text(encoding="utf-8").replace(
                f"      - uses: {ACTION}\n        with:\n          command: check",
                f"      - uses: {ACTION}\n        if: false\n        with:\n          command: check",
                1,
            ),
            encoding="utf-8",
        )
        with self.assertRaisesRegex(
            mod.CargoGraphOwnershipError, "production cargo-deny authority must not be conditional"
        ):
            mod.verify_repository(root)

    def test_production_continue_on_error_fails_closed(self):
        root = self.make_repo()
        path = root / ".github/workflows/ci.yml"
        path.write_text(
            path.read_text(encoding="utf-8").replace(
                f"      - uses: {ACTION}\n        with:\n          command: check",
                f"      - uses: {ACTION}\n        continue-on-error: true\n        with:\n          command: check",
                1,
            ),
            encoding="utf-8",
        )
        with self.assertRaisesRegex(
            mod.CargoGraphOwnershipError, "production cargo-deny authority must not allow failure"
        ):
            mod.verify_repository(root)

    def test_conditional_authority_job_fails_closed(self):
        root = self.make_repo()
        path = root / ".github/workflows/ci.yml"
        path.write_text(
            path.read_text(encoding="utf-8").replace(
                "  deny:\n    name: license\n",
                "  deny:\n    name: license\n    if: false\n",
            ),
            encoding="utf-8",
        )
        with self.assertRaisesRegex(
            mod.CargoGraphOwnershipError, "supply-chain authority job must not be conditional"
        ):
            mod.verify_repository(root)

    def test_authority_job_continue_on_error_fails_closed(self):
        root = self.make_repo()
        path = root / ".github/workflows/ci.yml"
        path.write_text(
            path.read_text(encoding="utf-8").replace(
                "  deny:\n    name: license\n",
                "  deny:\n    name: license\n    continue-on-error: true\n",
            ),
            encoding="utf-8",
        )
        with self.assertRaisesRegex(
            mod.CargoGraphOwnershipError, "supply-chain authority job must not allow failure"
        ):
            mod.verify_repository(root)

    def test_typst_audit_outside_required_job_fails_closed(self):
        root = self.make_repo()
        path = root / ".github/workflows/ci.yml"
        path.write_text(
            f'''jobs:
  deny:
    name: license
    steps:
      - uses: {ACTION}
        with:
          command: check
          arguments: --all-features
  research:
    steps:
      - uses: {ACTION}
        with:
          manifest-path: tools/spikes/typst-in-process/Cargo.toml
          command: check
          arguments: --all-features
          command-arguments: advisories sources
''',
            encoding="utf-8",
        )
        with self.assertRaisesRegex(
            mod.CargoGraphOwnershipError, "audited research cargo-deny"
        ):
            mod.verify_repository(root)

    def test_additional_cargo_deny_authority_fails_closed(self):
        root = self.make_repo()
        path = root / ".github/workflows/ci.yml"
        path.write_text(
            path.read_text(encoding="utf-8")
            + f'''      - uses: {ACTION}
        with:
          manifest-path: tools/other/Cargo.toml
          command: check
          arguments: --all-features
''',
            encoding="utf-8",
        )
        with self.assertRaisesRegex(
            mod.CargoGraphOwnershipError, "unexpected additional cargo-deny authority"
        ):
            mod.verify_repository(root)

    def test_other_workflow_cannot_run_research_manifest(self):
        root = self.make_repo()
        (root / ".github/workflows/rogue.yml").write_text(
            'jobs:\n  x:\n    steps:\n      - run: cargo run --manifest-path tools/spikes/markdown-substrate/Cargo.toml\n',
            encoding="utf-8",
        )
        with self.assertRaisesRegex(
            mod.CargoGraphOwnershipError,
            "must not execute/package unaudited research",
        ):
            mod.verify_repository(root)

    def test_other_workflow_cannot_cd_to_research_root(self):
        root = self.make_repo()
        (root / ".github/workflows/rogue.yml").write_text(
            'jobs:\n  x:\n    steps:\n      - run: cd tools/spikes && cargo metadata\n',
            encoding="utf-8",
        )
        with self.assertRaisesRegex(
            mod.CargoGraphOwnershipError,
            "must not execute/package unaudited research",
        ):
            mod.verify_repository(root)

    def test_duplicate_allowed_manifest_reference_fails_closed(self):
        root = self.make_repo()
        path = root / ".github/workflows/ci.yml"
        path.write_text(
            path.read_text(encoding="utf-8")
            + '''      - uses: example/other-action@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
        with:
          manifest-path: tools/spikes/typst-in-process/Cargo.toml
''',
            encoding="utf-8",
        )
        with self.assertRaisesRegex(
            mod.CargoGraphOwnershipError,
            "expected exactly one literal audited research manifest reference",
        ):
            mod.verify_repository(root)


if __name__ == "__main__":
    unittest.main()