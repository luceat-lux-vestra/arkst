#!/usr/bin/env python3
"""Fail-closed contract for trusted main -> PR macOS/Windows Rust caches."""

from __future__ import annotations

import unittest
from fnmatch import fnmatchcase
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[2]
PRODUCER = ROOT / ".github/workflows/sccache-main-producer.yml"
CONSUMER = ROOT / ".github/workflows/ci.yml"
RUST_CACHE = "Swatinem/rust-cache@d1527c8f67ef9af357290475d4af4b0fab8f12e0"

# The producer is an optional CI cache warmer, not a second main product-test run.
# Cargo dependency/build configuration changes seed; ordinary source/docs do not.
EXPECTED_SEED_PATHS = {
    "Cargo.toml", "Cargo.lock", "**/Cargo.toml", "**/Cargo.lock",
    ".cargo/**", "**/.cargo/**", "rust-toolchain", "rust-toolchain.toml",
    ".github/workflows/sccache-main-producer.yml",
}


def validate_seed_paths(producer: str) -> None:
    header = producer.split("\njobs:", 1)[0]
    match = re.search(
        r"(?m)^  push:\n    branches: \[main\]\n(?:    #[^\n]*\n)*    paths:\n"
        r"((?:      - '[^']+'\n)+)",
        header,
    )
    if not match:
        raise ValueError("protected-main cache seed requires static path gating")
    actual = set(re.findall(r"(?m)^      - '([^']+)'$", match.group(1)))
    if actual != EXPECTED_SEED_PATHS:
        raise ValueError(f"seed path policy drift: {actual ^ EXPECTED_SEED_PATHS}")
    positive = (
        "Cargo.toml", "Cargo.lock", "crates/arkst-cli/Cargo.toml",
        ".cargo/config.toml", "crates/x/.cargo/config.toml",
        "rust-toolchain.toml", ".github/workflows/sccache-main-producer.yml",
    )
    negative = (
        "README.md", "docs/compatibility/notes.md", "crates/arkst-cli/src/lib.rs",
        "tools/ci/test_main_cache_bridge.py", ".github/workflows/ci.yml",
    )
    for path in positive:
        if not any(fnmatchcase(path, glob) for glob in actual):
            raise ValueError(f"required seed input ignored: {path}")
    for path in negative:
        if any(fnmatchcase(path, glob) for glob in actual):
            raise ValueError(f"source-only CI cache re-seeding: {path}")



def validate(producer: str, consumer: str) -> None:
    """Refuse a cache producer that cannot populate the PR test cache family."""
    def require(haystack: str, needle: str, reason: str) -> None:
        if needle not in haystack:
            raise ValueError(f"{reason}: missing {needle!r}")

    require(producer, "on:\n  push:\n    branches: [main]", "trusted main-only event")
    require(producer, "\n  workflow_dispatch:\n", "manual cache recovery")
    validate_seed_paths(producer)
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
    require(seed, "id: platform-rust-cache", "cache hit identity")
    cache_miss_guard = "if: " + "$" + "{{ steps.platform-rust-cache.outputs.cache-hit != 'true' }}"
    for named_step in (
        "Install Typst 0.15.1 (pinned)",
        "Test default CLI feature boundary",
        "Test workspace (Typst required)",
    ):
        step_prefix = "      - name: " + named_step + "\n"
        require(seed, step_prefix, "main producer step identity")
        body = seed.split(step_prefix, 1)[1].split("      - name:", 1)[0]
        require(body, cache_miss_guard, "cache-hit skip protection")

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
    require(section, "save-if: false", "PR cannot create per-merge-ref cache")
    if "save-if: true" in section:
        raise ValueError("PR Rust cache must not be writable")
    # This CI-only LGPL-3.0 action was already pinned in CI before the
    # producer was introduced. Any license-policy exception must be scoped
    # to that exact reviewed SHA, never LGPL globally or a wildcard action.
    exception = "pkg:githubactions/Swatinem/rust-cache@d1527c8f67ef9af357290475d4af4b0fab8f12e0"
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

    def test_dependency_only_seed_scope(self) -> None:
        validate_seed_paths(self.producer)

    def test_manual_recovery_required(self) -> None:
        bad = self.producer.replace("  workflow_dispatch:\n", "", 1)
        with self.assertRaises(ValueError):
            validate(bad, self.consumer)

    def test_unbounded_seed_scope_rejected(self) -> None:
        bad = self.producer.replace("      - 'Cargo.toml'", "      - '**'", 1)
        with self.assertRaises(ValueError):
            validate(bad, self.consumer)

    def test_missing_lockfile_trigger_rejected(self) -> None:
        bad = self.producer.replace("      - 'Cargo.lock'\n", "", 1)
        with self.assertRaises(ValueError):
            validate(bad, self.consumer)

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

    def test_main_warm_cache_cannot_repeat_full_tests(self) -> None:
        bad = self.producer.replace(
            "if: " + "$" + "{{ steps.platform-rust-cache.outputs.cache-hit != 'true' }}",
            "if: " + "$" + "{{ steps.platform-rust-cache.outputs.cache-hit == 'true' }}",
            1,
        )
        with self.assertRaises(ValueError):
            validate(bad, self.consumer)

    def test_pr_cache_writes_rejected(self) -> None:
        bad = self.consumer.replace("save-if: false", "save-if: true", 1)
        with self.assertRaises(ValueError):
            validate(self.producer, bad)

    def test_broad_action_license_exception_rejected(self) -> None:
        bad = self.consumer.replace(
            "pkg:githubactions/Swatinem/rust-cache@d1527c8f67ef9af357290475d4af4b0fab8f12e0",
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
