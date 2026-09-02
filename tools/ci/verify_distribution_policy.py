#!/usr/bin/env python3
"""Verify Scribium's fail-closed Cargo distribution policy."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tomllib
from collections.abc import Mapping
from pathlib import Path, PurePosixPath
from typing import Any


class DistributionPolicyError(RuntimeError):
    """Raised when the distribution inventory or Cargo metadata drifts."""


POLICY_VERSION = 1
CURRENT_DECISION = "no-current-public-distribution"
IGNORED_TARGET_KINDS = {"test", "example", "bench", "custom-build"}
ALLOWED_TARGET_KINDS = {"lib", "bin", *IGNORED_TARGET_KINDS}
ALLOWED_CONSUMERS = {
    "internal-toolchain",
    "repository-tooling",
    "test-only",
    "user-facing-cli",
}
ALLOWED_DISTRIBUTIONS = {"compiler-library", "internal-tooling", "test-support", "cli"}
ALLOWED_STATUSES = {"internal-only", "not-intended"}


def _expect_keys(table: Mapping[str, Any], expected: set[str], context: str) -> None:
    actual = set(table)
    missing = sorted(expected - actual)
    unknown = sorted(actual - expected)
    if missing or unknown:
        details: list[str] = []
        if missing:
            details.append(f"missing {', '.join(missing)}")
        if unknown:
            details.append(f"unknown {', '.join(unknown)}")
        raise DistributionPolicyError(f"{context}: {'; '.join(details)}")


def _expect_table(value: Any, context: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise DistributionPolicyError(f"{context}: expected a table")
    return value


def _expect_string(value: Any, context: str) -> str:
    if not isinstance(value, str):
        raise DistributionPolicyError(f"{context}: expected a string")
    return value


def _expect_bool(value: Any, context: str) -> bool:
    if not isinstance(value, bool):
        raise DistributionPolicyError(f"{context}: expected a boolean")
    return value


def _expect_string_list(value: Any, context: str) -> list[str]:
    if not isinstance(value, list) or not all(isinstance(item, str) for item in value):
        raise DistributionPolicyError(f"{context}: expected a string array")
    if value != sorted(value):
        raise DistributionPolicyError(f"{context}: entries must be sorted")
    if len(value) != len(set(value)):
        raise DistributionPolicyError(f"{context}: entries must be unique")
    return value


def _expect_status(value: Any, allowed: set[str], context: str) -> str:
    status = _expect_string(value, context)
    if status not in allowed:
        allowed_text = ", ".join(sorted(allowed))
        raise DistributionPolicyError(
            f"{context}: unknown status {status!r}; expected {allowed_text}"
        )
    return status


def load_policy(path: Path) -> Mapping[str, Any]:
    try:
        with path.open("rb") as stream:
            policy = tomllib.load(stream)
    except tomllib.TOMLDecodeError as exc:
        raise DistributionPolicyError(f"{path}: invalid TOML: {exc}") from exc
    except OSError as exc:
        raise DistributionPolicyError(f"{path}: cannot read policy: {exc}") from exc
    return _expect_table(policy, "policy")


def load_metadata(root: Path) -> Mapping[str, Any]:
    command = [
        "cargo",
        "metadata",
        "--format-version",
        "1",
        "--no-deps",
        "--locked",
        "--offline",
        "--manifest-path",
        str(root / "Cargo.toml"),
    ]
    try:
        result = subprocess.run(
            command,
            cwd=root,
            check=False,
            capture_output=True,
            text=True,
        )
    except OSError as exc:
        raise DistributionPolicyError(f"cargo metadata could not run: {exc}") from exc
    if result.returncode != 0:
        detail = result.stderr.strip()
        suffix = f": {detail}" if detail else ""
        raise DistributionPolicyError(
            f"cargo metadata failed with exit code {result.returncode}{suffix}"
        )
    try:
        metadata = json.loads(result.stdout)
    except json.JSONDecodeError as exc:
        raise DistributionPolicyError(f"cargo metadata returned invalid JSON: {exc}") from exc
    return _expect_table(metadata, "cargo metadata")


def _metadata_packages(metadata: Mapping[str, Any]) -> dict[str, Mapping[str, Any]]:
    packages = metadata.get("packages")
    if not isinstance(packages, list):
        raise DistributionPolicyError("cargo metadata: packages must be an array")

    package_map: dict[str, Mapping[str, Any]] = {}
    package_ids: set[str] = set()
    for index, raw_package in enumerate(packages):
        package = _expect_table(raw_package, f"cargo metadata package {index}")
        name = _expect_string(package.get("name"), f"cargo metadata package {index}.name")
        package_id = _expect_string(package.get("id"), f"cargo metadata package {name}.id")
        if name in package_map:
            raise DistributionPolicyError(f"cargo metadata: duplicate package {name!r}")
        if package_id in package_ids:
            raise DistributionPolicyError(f"cargo metadata: duplicate package id {package_id!r}")
        package_map[name] = package
        package_ids.add(package_id)

    workspace_members = metadata.get("workspace_members")
    if not isinstance(workspace_members, list) or not all(
        isinstance(item, str) for item in workspace_members
    ):
        raise DistributionPolicyError("cargo metadata: workspace_members must be a string array")
    if set(workspace_members) != package_ids:
        raise DistributionPolicyError(
            "cargo metadata: --no-deps package ids and workspace_members differ"
        )
    return package_map


def _target_inventory(
    package: Mapping[str, Any], package_name: str
) -> tuple[str, list[str], list[str]]:
    targets = package.get("targets")
    if not isinstance(targets, list):
        raise DistributionPolicyError(
            f"cargo metadata package {package_name}: targets must be an array"
        )

    library_targets: list[str] = []
    binary_targets: list[str] = []
    for index, raw_target in enumerate(targets):
        target = _expect_table(
            raw_target,
            f"cargo metadata package {package_name}.targets[{index}]",
        )
        target_name = _expect_string(
            target.get("name"),
            f"cargo metadata package {package_name}.targets[{index}].name",
        )
        kinds = target.get("kind")
        if not isinstance(kinds, list) or not all(isinstance(kind, str) for kind in kinds):
            raise DistributionPolicyError(
                f"cargo metadata package {package_name}.targets[{index}].kind: "
                "expected string array"
            )
        unsupported = sorted(set(kinds) - ALLOWED_TARGET_KINDS)
        if unsupported:
            raise DistributionPolicyError(
                f"cargo metadata package {package_name}: unsupported target kinds "
                f"{', '.join(unsupported)}"
            )
        if "lib" in kinds:
            library_targets.append(target_name)
        if "bin" in kinds:
            binary_targets.append(target_name)

    library_targets.sort()
    binary_targets.sort()
    if library_targets and binary_targets:
        artifact_kind = "library-and-binary"
    elif library_targets:
        artifact_kind = "library"
    elif binary_targets:
        artifact_kind = "binary"
    else:
        raise DistributionPolicyError(
            f"cargo metadata package {package_name}: no library or binary target"
        )
    return artifact_kind, library_targets, binary_targets


def _workspace_dependencies(
    package: Mapping[str, Any], package_names: set[str], package_name: str
) -> list[str]:
    dependencies = package.get("dependencies")
    if not isinstance(dependencies, list):
        raise DistributionPolicyError(
            f"cargo metadata package {package_name}: dependencies must be an array"
        )
    result: set[str] = set()
    for index, raw_dependency in enumerate(dependencies):
        dependency = _expect_table(
            raw_dependency,
            f"cargo metadata package {package_name}.dependencies[{index}]",
        )
        dependency_name = _expect_string(
            dependency.get("name"),
            f"cargo metadata package {package_name}.dependencies[{index}].name",
        )
        if dependency_name in package_names and dependency.get("path") is not None:
            result.add(dependency_name)
    return sorted(result)


def _validate_channels(policy: Mapping[str, Any]) -> None:
    channels = _expect_table(policy.get("channels"), "policy.channels")
    channel_fields = {
        "crates_io": {
            "publishable",
            "cargo_install",
            "distribution_status",
            "publication_channel",
        },
        "github_release": {
            "publishable",
            "package",
            "binary",
            "distribution_status",
            "publication_channel",
        },
        "internal_tools": {
            "publishable",
            "distribution_status",
            "publication_channel",
        },
        "wasm": {
            "artifact",
            "cargo_package",
            "publishable",
            "distribution_status",
            "publication_channel",
        },
    }
    _expect_keys(channels, set(channel_fields), "policy.channels")
    for name, fields in channel_fields.items():
        _expect_keys(
            _expect_table(channels[name], f"policy.channels.{name}"),
            fields,
            f"policy.channels.{name}",
        )

    crates_io = _expect_table(channels["crates_io"], "policy.channels.crates_io")
    if _expect_bool(crates_io["publishable"], "policy.channels.crates_io.publishable"):
        raise DistributionPolicyError("policy.channels.crates_io.publishable must be false")
    if _expect_bool(crates_io["cargo_install"], "policy.channels.crates_io.cargo_install"):
        raise DistributionPolicyError("policy.channels.crates_io.cargo_install must be false")
    if _expect_status(
        crates_io["distribution_status"],
        {"not-intended"},
        "policy.channels.crates_io.distribution_status",
    ) != "not-intended":
        raise DistributionPolicyError("policy.channels.crates_io must be not-intended")
    if (
        _expect_string(
            crates_io["publication_channel"],
            "policy.channels.crates_io.publication_channel",
        )
        != "crates.io"
    ):
        raise DistributionPolicyError(
            "policy.channels.crates_io.publication_channel must be crates.io"
        )

    github = _expect_table(channels["github_release"], "policy.channels.github_release")
    if _expect_bool(github["publishable"], "policy.channels.github_release.publishable"):
        raise DistributionPolicyError("policy.channels.github_release.publishable must be false")
    if (
        _expect_string(github["package"], "policy.channels.github_release.package")
        != "scribium-cli"
    ):
        raise DistributionPolicyError("policy.channels.github_release.package must be scribium-cli")
    if _expect_string(github["binary"], "policy.channels.github_release.binary") != "scribium":
        raise DistributionPolicyError("policy.channels.github_release.binary must be scribium")
    if _expect_status(
        github["distribution_status"],
        {"not-intended"},
        "policy.channels.github_release.distribution_status",
    ) != "not-intended":
        raise DistributionPolicyError("policy.channels.github_release must be not-intended")
    if (
        _expect_string(
            github["publication_channel"],
            "policy.channels.github_release.publication_channel",
        )
        != "none"
    ):
        raise DistributionPolicyError(
            "policy.channels.github_release.publication_channel must be none"
        )

    internal = _expect_table(channels["internal_tools"], "policy.channels.internal_tools")
    if _expect_bool(internal["publishable"], "policy.channels.internal_tools.publishable"):
        raise DistributionPolicyError("policy.channels.internal_tools.publishable must be false")
    if _expect_status(
        internal["distribution_status"],
        {"internal-only"},
        "policy.channels.internal_tools.distribution_status",
    ) != "internal-only":
        raise DistributionPolicyError("policy.channels.internal_tools must be internal-only")
    if (
        _expect_string(
            internal["publication_channel"],
            "policy.channels.internal_tools.publication_channel",
        )
        != "none"
    ):
        raise DistributionPolicyError(
            "policy.channels.internal_tools.publication_channel must be none"
        )

    wasm = _expect_table(channels["wasm"], "policy.channels.wasm")
    _expect_string(wasm["artifact"], "policy.channels.wasm.artifact")
    if _expect_string(wasm["cargo_package"], "policy.channels.wasm.cargo_package") != "none":
        raise DistributionPolicyError("policy.channels.wasm.cargo_package must be none")
    if _expect_bool(wasm["publishable"], "policy.channels.wasm.publishable"):
        raise DistributionPolicyError("policy.channels.wasm.publishable must be false")
    if _expect_status(
        wasm["distribution_status"], {"compile-only"}, "policy.channels.wasm.distribution_status"
    ) != "compile-only":
        raise DistributionPolicyError("policy.channels.wasm must be compile-only")
    if (
        _expect_string(
            wasm["publication_channel"],
            "policy.channels.wasm.publication_channel",
        )
        != "none"
    ):
        raise DistributionPolicyError("policy.channels.wasm.publication_channel must be none")


def _validate_policy_shape(policy: Mapping[str, Any]) -> list[Mapping[str, Any]]:
    _expect_keys(
        policy,
        {"schema_version", "decision", "review_required", "channels", "cli", "packages"},
        "policy",
    )
    if policy["schema_version"] != POLICY_VERSION:
        raise DistributionPolicyError(
            f"policy.schema_version must be {POLICY_VERSION}, got {policy['schema_version']!r}"
        )
    if _expect_string(policy["decision"], "policy.decision") != CURRENT_DECISION:
        raise DistributionPolicyError(
            f"policy.decision must be {CURRENT_DECISION!r} while this verifier "
            "enforces no public distribution"
        )
    if not _expect_bool(policy["review_required"], "policy.review_required"):
        raise DistributionPolicyError("policy.review_required must be true")
    _validate_channels(policy)

    cli = _expect_table(policy["cli"], "policy.cli")
    _expect_keys(
        cli,
        {"package", "binary", "cargo_install", "github_release", "distribution_status"},
        "policy.cli",
    )
    if _expect_string(cli["package"], "policy.cli.package") != "scribium-cli":
        raise DistributionPolicyError("policy.cli.package must be scribium-cli")
    if _expect_string(cli["binary"], "policy.cli.binary") != "scribium":
        raise DistributionPolicyError("policy.cli.binary must be scribium")
    if _expect_bool(cli["cargo_install"], "policy.cli.cargo_install"):
        raise DistributionPolicyError("policy.cli.cargo_install must be false")
    if _expect_bool(cli["github_release"], "policy.cli.github_release"):
        raise DistributionPolicyError("policy.cli.github_release must be false")
    _expect_status(cli["distribution_status"], {"not-intended"}, "policy.cli.distribution_status")

    packages = policy["packages"]
    if not isinstance(packages, list):
        raise DistributionPolicyError("policy.packages must be an array")
    entries: list[Mapping[str, Any]] = []
    names: list[str] = []
    for index, raw_package in enumerate(packages):
        package = _expect_table(raw_package, f"policy.packages[{index}]")
        name = _expect_string(package.get("name"), f"policy.packages[{index}].name")
        if name in names:
            raise DistributionPolicyError(f"policy.packages: duplicate package {name!r}")
        names.append(name)
        entries.append(package)
    if names != sorted(names):
        raise DistributionPolicyError("policy.packages: entries must be sorted by package name")

    package_fields = {
        "name",
        "manifest",
        "version",
        "consumer",
        "distribution",
        "distribution_status",
        "publication_channel",
        "artifact_kind",
        "library_targets",
        "binary_targets",
        "workspace_dependencies",
        "publishable",
    }
    for index, package in enumerate(entries):
        _expect_keys(
            package,
            package_fields,
            f"policy.packages[{index}]",
        )
    return entries


def _validate_package_entry(
    root: Path,
    entry: Mapping[str, Any],
    metadata_package: Mapping[str, Any],
    package_names: set[str],
) -> None:
    name = _expect_string(entry["name"], f"policy package {entry.get('name')!r}.name")
    context = f"policy package {name}"
    manifest = _expect_string(entry["manifest"], f"{context}.manifest")
    manifest_path = PurePosixPath(manifest)
    if manifest_path.is_absolute() or ".." in manifest_path.parts or "\\" in manifest:
        raise DistributionPolicyError(f"{context}.manifest must be a safe repository-relative path")
    expected_manifest = (root / Path(*manifest_path.parts)).resolve()
    actual_manifest = Path(
        _expect_string(
            metadata_package.get("manifest_path"),
            f"cargo metadata package {name}.manifest_path",
        )
    ).resolve()
    if expected_manifest != actual_manifest:
        try:
            actual_display = str(actual_manifest.relative_to(root))
        except ValueError:
            actual_display = str(actual_manifest)
        raise DistributionPolicyError(
            f"{context}.manifest disagrees with Cargo metadata: {manifest!r} != "
            f"{actual_display}"
        )

    version = _expect_string(entry["version"], f"{context}.version")
    metadata_version = _expect_string(
        metadata_package.get("version"),
        f"cargo metadata package {name}.version",
    )
    if version != metadata_version:
        raise DistributionPolicyError(
            f"{context}.version disagrees with Cargo metadata: {version!r} != {metadata_version!r}"
        )

    consumer = _expect_string(entry["consumer"], f"{context}.consumer")
    if consumer not in ALLOWED_CONSUMERS:
        raise DistributionPolicyError(f"{context}.consumer: unknown value {consumer!r}")
    distribution = _expect_string(entry["distribution"], f"{context}.distribution")
    if distribution not in ALLOWED_DISTRIBUTIONS:
        raise DistributionPolicyError(f"{context}.distribution: unknown value {distribution!r}")
    is_tool_package = manifest_path.parts[:1] == ("tools",)
    if is_tool_package != (distribution == "internal-tooling"):
        raise DistributionPolicyError(
            f"{context}: repository tools must be classified as internal-tooling"
        )
    distribution_status = _expect_status(
        entry["distribution_status"], ALLOWED_STATUSES, f"{context}.distribution_status"
    )
    publication_channel = _expect_string(
        entry["publication_channel"], f"{context}.publication_channel"
    )
    if publication_channel != "none":
        raise DistributionPolicyError(f"{context}.publication_channel must be none")
    artifact_kind = _expect_string(entry["artifact_kind"], f"{context}.artifact_kind")
    library_targets = _expect_string_list(entry["library_targets"], f"{context}.library_targets")
    binary_targets = _expect_string_list(entry["binary_targets"], f"{context}.binary_targets")
    workspace_dependencies = _expect_string_list(
        entry["workspace_dependencies"], f"{context}.workspace_dependencies"
    )
    if not set(workspace_dependencies).issubset(package_names):
        raise DistributionPolicyError(
            f"{context}.workspace_dependencies contains a non-workspace package"
        )
    if _expect_bool(entry["publishable"], f"{context}.publishable"):
        raise DistributionPolicyError(f"{context}.publishable must be false")

    actual_kind, actual_libraries, actual_binaries = _target_inventory(metadata_package, name)
    if artifact_kind != actual_kind:
        raise DistributionPolicyError(
            f"{context}.artifact_kind disagrees with Cargo metadata: "
            f"{artifact_kind!r} != {actual_kind!r}"
        )
    if library_targets != actual_libraries:
        raise DistributionPolicyError(
            f"{context}.library_targets disagrees with Cargo metadata: "
            f"{library_targets!r} != {actual_libraries!r}"
        )
    if binary_targets != actual_binaries:
        raise DistributionPolicyError(
            f"{context}.binary_targets disagrees with Cargo metadata: "
            f"{binary_targets!r} != {actual_binaries!r}"
        )
    actual_dependencies = _workspace_dependencies(metadata_package, package_names, name)
    if workspace_dependencies != actual_dependencies:
        raise DistributionPolicyError(
            f"{context}.workspace_dependencies disagrees with Cargo metadata: "
            f"{workspace_dependencies!r} != {actual_dependencies!r}"
        )

    cargo_publish = metadata_package.get("publish")
    if cargo_publish != []:
        raise DistributionPolicyError(
            f"{context}: Cargo metadata publish={cargo_publish!r}; expected [] "
            "from explicit publish = false"
        )

    expected_status = (
        "internal-only"
        if distribution in {"internal-tooling", "test-support"}
        else "not-intended"
    )
    if distribution_status != expected_status:
        raise DistributionPolicyError(
            f"{context}.distribution_status must be {expected_status!r} for {distribution!r}"
        )
    if distribution == "internal-tooling" and consumer != "repository-tooling":
        raise DistributionPolicyError(f"{context}.consumer must be repository-tooling")
    if distribution == "test-support" and consumer != "test-only":
        raise DistributionPolicyError(f"{context}.consumer must be test-only")
    if distribution == "cli" and (consumer != "user-facing-cli" or binary_targets != ["scribium"]):
        raise DistributionPolicyError("policy cli package must identify the scribium binary")


def verify_policy(root: Path, policy_path: Path, metadata: Mapping[str, Any]) -> None:
    root = root.resolve()
    policy = load_policy(policy_path)
    entries = _validate_policy_shape(policy)
    package_map = _metadata_packages(metadata)
    metadata_names = set(package_map)
    policy_names = {entry["name"] for entry in entries}
    if metadata_names != policy_names:
        missing = sorted(metadata_names - policy_names)
        stale = sorted(policy_names - metadata_names)
        details: list[str] = []
        if missing:
            details.append(f"missing from policy: {', '.join(missing)}")
        if stale:
            details.append(f"stale policy entries: {', '.join(stale)}")
        raise DistributionPolicyError(
            "workspace package inventory does not match Cargo metadata; " + "; ".join(details)
        )
    for entry in entries:
        name = entry["name"]
        _validate_package_entry(root, entry, package_map[name], metadata_names)

    cli = next(entry for entry in entries if entry["name"] == "scribium-cli")
    if cli["binary_targets"] != ["scribium"]:
        raise DistributionPolicyError("policy cli package must expose exactly the scribium binary")
    if not any(entry["distribution"] == "internal-tooling" for entry in entries):
        raise DistributionPolicyError("policy must classify repository tooling explicitly")


def _default_root() -> Path:
    return Path(__file__).resolve().parents[2]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=_default_root())
    parser.add_argument("--policy", type=Path)
    args = parser.parse_args()
    root = args.root.resolve()
    policy_path = args.policy or root / ".github" / "distribution-policy.toml"
    if not policy_path.is_absolute():
        policy_path = root / policy_path
    try:
        metadata = load_metadata(root)
        verify_policy(root, policy_path, metadata)
    except (DistributionPolicyError, OSError) as exc:
        print(f"distribution-policy error: {exc}", file=sys.stderr)
        return 1
    print(
        "distribution policy verified: no-current-public-distribution "
        f"({len(metadata['packages'])} workspace packages)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
