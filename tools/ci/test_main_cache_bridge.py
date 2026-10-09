#!/usr/bin/env python3
"""Fail-closed contract for trusted main -> PR macOS/Windows Rust caches."""

from __future__ import annotations

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PRODUCER = ROOT / ".github/workflows/sccache-main-producer.yml"
CONSUMER = ROOT / ".github/workflows/ci.yml"
RUST_CACHE = "Swatinem/rust-cache@6323deb102c322ba6fcbdcafc7e3dddab59af2b6"


def validate(producer: str, consumer: str) -> None:
    """Refuse a cache producer that cannot populate the PR test cache family."""
    def require(haystack: str, needle: str, reason: str) -> None:
        if needle not in haystack:
            raise ValueError(f"{reason}: missing {needle!r}")

    require(producer, "on:\n  push:\n    branches: [main]", "trusted main-only event")
    if "\n  pull_request:" in producer or "\n  pull_request_target:" in producer:
        raise ValueError("untrusted events must not populate protected-main cache")

    # rust-cache hashes variables beginning with CARGO, including CARGO_INCREMENTAL.
    # Unlike the ARM sccache producer, the PR test cache is keyed before
    # rust-cache injects CARGO_INCREMENTAL=0; prevent producer-only key drift.
    producer_global = producer.split("\njobs:\n", 1)[0]
    if "CARGO_INCREMENTAL:" in producer_global:
        raise ValueError("producer-only CARGO_INCREMENTAL must not alter test cache keys")
    require(producer, '  produce:\n    env:\n', "bounded ARM producer environment")
    require(producer.split("\n  test:\n", 1)[0], 'CARGO_INCREMENTAL: "0"', "ARM producer incremental policy")

    require(producer, "\n  test:\n", "matching PR job ID")
    seed = producer.split("\n  test:\n", 1)[1]
    require(seed, "os: [macos-latest, windows-latest]", "both platforms")
    require(seed, "runs-on: " + "$" + "{{ matrix.os }}", "OS-matched runner")
    require(seed, "test \"$GITHUB_REF\" = \"refs/heads/$DEFAULT_BRANCH\"", "main authority")
    require(seed, 'test "$(git rev-parse HEAD)" = "$EXPECTED_SHA"', "SHA authority")
    require(seed, RUST_CACHE, "same pinned Rust cache action")
    require(seed, "save-if: " + "$" + "{{ github.ref == 'refs/heads/main' }}", "main-only cache writer")
    require(seed, "CARGO_PROFILE_TEST_DEBUG=1", "Windows debug parity")
    require(seed, "toolchain: 1.98.0", "toolchain parity")
    require(seed, "typst-version: 0.15.1", "Typst parity")
    for command in (
        "cargo test --locked -p arkst-cli --no-default-features",
        "cargo test --locked --workspace --all-targets --all-features",
        'ARKST_REQUIRE_TYPST: "1"',
    ):
        require(seed, command, "test workload parity")
        require(consumer, command, "PR test workload parity")

    name = "      - name: Restore Rust cache (macOS/Windows)"
    require(consumer, name, "PR restore-only cache")
    section = consumer.split(name, 1)[1].split("      - name:", 1)[0]
    require(section, RUST_CACHE, "same pinned PR Rust cache action")
    require(section, "matrix.os != 'ubuntu-24.04-arm'", "non-Linux matrix guard")
    # Allow one exact PR-scoped diagnostic seed to establish comparable warm
    # telemetry; production final HEAD must restore save-if: false.
    diagnostic = "save-if: ${{ github.event_name == 'pull_request' && github.event.pull_request.number == 580 && github.event.pull_request.head.repo.full_name == github.repository && github.ref == 'refs/pull/580/merge' }}"
    save_lines = [line.strip() for line in section.splitlines() if line.strip().startswith("save-if:")]
    if save_lines not in (["save-if: false"], [diagnostic]):
        raise ValueError("cache writes must be disabled or strictly limited to PR #580")
    # This CI-only LGPL-3.0 action was already pinned in CI before the
    # producer was introduced. Any license-policy exception must be scoped
    # to that exact reviewed SHA, never LGPL globally or a wildcard action.
    exception = "pkg:githubactions/Swatinem/rust-cache@6323deb102c322ba6fcbdcafc7e3dddab59af2b6"
    lines = [line for line in consumer.splitlines() if "allow-dependencies-licenses:" in line]
    if len(lines) != 1:
        raise ValueError("expected one authoritative dependency license exception list")
    allowed = lines[0].split("allow-dependencies-licenses:", 1)[1]
    action_exemptions = [item.strip() for item in allowed.split(",") if item.strip().startswith("pkg:githubactions/")]
    if action_exemptions != [exception]:
        raise ValueError("GitHub Actions license exception must match reviewed SHA exactly")
    license_allow = [line for line in consumer.splitlines() if "allow-licenses:" in line]
    if any("LGPL" in line for line in license_allow):
        raise ValueError("must not globally allow LGPL")



class MainCacheBridgeContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.producer = PRODUCER.read_text(encoding="utf-8")
        cls.consumer = CONSUMER.read_text(encoding="utf-8")

    def test_current_repository_contract(self) -> None:
        validate(self.producer, self.consumer)

    def test_producer_without_main_identity_rejected(self) -> None:
        bad = self.producer.replace("    branches: [main]", "    branches: [feature]", 1)
        with self.assertRaises(ValueError):
            validate(bad, self.consumer)

    def test_single_platform_rejected(self) -> None:
        bad = self.producer.replace(
            "os: [macos-latest, windows-latest]", "os: [macos-latest]", 1
        )
        with self.assertRaises(ValueError):
            validate(bad, self.consumer)

    def test_pr_cache_writes_rejected(self) -> None:
        bad = self.consumer.replace("save-if: ${{ github.event_name == 'pull_request' && github.event.pull_request.number == 580 && github.event.pull_request.head.repo.full_name == github.repository && github.ref == 'refs/pull/580/merge' }}", "save-if: true", 1)
        with self.assertRaises(ValueError):
            validate(self.producer, bad)

    def test_other_pr_diagnostic_cache_writes_rejected(self) -> None:
        bad = self.consumer.replace("pull_request.number == 580", "pull_request.number == 581", 1)
        with self.assertRaises(ValueError):
            validate(self.producer, bad)

    def test_broad_action_license_exception_rejected(self) -> None:
        bad = self.consumer.replace(
            "pkg:githubactions/Swatinem/rust-cache@6323deb102c322ba6fcbdcafc7e3dddab59af2b6",
            "pkg:githubactions/Swatinem/rust-cache",
            1,
        )
        with self.assertRaises(ValueError):
            validate(self.producer, bad)

    def test_wrong_producer_cache_action_rejected(self) -> None:
        bad = self.producer.replace(
            RUST_CACHE, "Swatinem/rust-cache@unreviewed", 1
        )
        with self.assertRaises(ValueError):
            validate(bad, self.consumer)


if __name__ == "__main__":
    unittest.main()
