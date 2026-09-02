#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
import tomllib
from pathlib import Path


class ReleaseContractError(RuntimeError):
    pass


TARGETS = {
    "x86_64-unknown-linux-gnu": {
        "runner": "ubuntu-24.04",
        "binary": "arkst",
        "format": "tar.gz",
    },
    "aarch64-apple-darwin": {
        "runner": "macos-26",
        "binary": "arkst",
        "format": "tar.gz",
    },
    "x86_64-pc-windows-msvc": {
        "runner": "windows-2025",
        "binary": "arkst.exe",
        "format": "zip",
    },
}
SEMVER_TAG = re.compile(r"^v(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)$")


def load_policy(path: Path) -> dict:
    with path.open("rb") as stream:
        return tomllib.load(stream)


def load_metadata(root: Path) -> dict:
    proc = subprocess.run(
        [
            "cargo",
            "metadata",
            "--format-version",
            "1",
            "--no-deps",
            "--locked",
            "--offline",
        ],
        cwd=root,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if proc.returncode != 0:
        raise ReleaseContractError(f"cargo metadata failed: {proc.stderr.strip()}")
    return json.loads(proc.stdout)


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise ReleaseContractError(message)


def asset_name(version: str, target: str) -> str:
    cfg = TARGETS[target]
    return f"arkst-{version}-{target}.{cfg['format']}"


def validate_contract(policy: dict, metadata: dict, tag: str | None = None) -> dict:
    _require(policy.get("schema_version") == 1, "distribution policy schema_version must be 1")
    _require(policy.get("decision") == "github-release-cli", "distribution decision must be github-release-cli")

    channels = policy.get("channels", {})
    crates = channels.get("crates_io", {})
    github = channels.get("github_release", {})
    wasm = channels.get("wasm", {})
    cli = policy.get("cli", {})

    _require(crates.get("publishable") is False, "crates.io must remain disabled")
    _require(crates.get("cargo_install") is False, "public cargo install must remain disabled")
    _require(github.get("publishable") is True, "GitHub Release channel must be enabled")
    _require(github.get("package") == "arkst-cli", "GitHub Release package must be arkst-cli")
    _require(github.get("binary") == "arkst", "GitHub Release binary must be arkst")
    _require(github.get("distribution_status") == "intended", "GitHub Release status must be intended")
    _require(github.get("publication_channel") == "github-releases", "GitHub Release publication channel must be github-releases")
    _require(wasm.get("publishable") is False, "WASM distribution must remain disabled")
    _require(wasm.get("distribution_status") == "compile-only", "WASM must remain compile-only")

    _require(cli.get("package") == "arkst-cli", "CLI package must be arkst-cli")
    _require(cli.get("binary") == "arkst", "CLI binary must be arkst")
    _require(cli.get("cargo_install") is False, "CLI cargo_install must remain false")
    _require(cli.get("github_release") is True, "CLI GitHub Release flag must be true")
    _require(cli.get("distribution_status") == "intended", "CLI distribution status must be intended")

    targets = github.get("targets")
    expected_targets = list(TARGETS)
    _require(isinstance(targets, list), "GitHub Release targets must be declared")
    _require(len(targets) == len(set(targets)), "GitHub Release targets must not contain duplicates")
    _require(targets == expected_targets, f"GitHub Release targets must be exactly {expected_targets}")

    entries = policy.get("packages", [])
    cli_entries = [entry for entry in entries if entry.get("name") == "arkst-cli"]
    _require(len(cli_entries) == 1, "distribution inventory must contain exactly one arkst-cli entry")
    entry = cli_entries[0]
    _require(entry.get("distribution") == "cli", "arkst-cli distribution must be cli")
    _require(entry.get("distribution_status") == "intended", "arkst-cli status must be intended")
    _require(entry.get("publication_channel") == "github-releases", "arkst-cli publication channel must be github-releases")
    _require(entry.get("publishable") is False, "arkst-cli Cargo publication must remain disabled")
    _require(entry.get("binary_targets") == ["arkst"], "arkst-cli binary target must be exactly arkst")

    packages = {package["name"]: package for package in metadata.get("packages", [])}
    _require("arkst-cli" in packages, "Cargo metadata is missing arkst-cli")
    cargo_cli = packages["arkst-cli"]
    version = cargo_cli.get("version")
    _require(version == entry.get("version"), "arkst-cli version must match the canonical inventory")
    _require(cargo_cli.get("publish") == [], "arkst-cli must remain non-publishable in Cargo metadata")
    for package in packages.values():
        _require(package.get("publish") == [], f"{package['name']} must remain non-publishable")

    binary_targets = sorted(
        target["name"]
        for target in cargo_cli.get("targets", [])
        if "bin" in target.get("kind", [])
    )
    _require(binary_targets == ["arkst"], "Cargo metadata must expose exactly the arkst CLI binary")

    if tag is not None:
        _require(SEMVER_TAG.fullmatch(tag) is not None, f"release tag must be SemVer-shaped vX.Y.Z, got {tag!r}")
        _require(tag == f"v{version}", f"release tag {tag!r} does not match arkst-cli version {version!r}")

    matrix = []
    for target in targets:
        cfg = TARGETS[target]
        matrix.append(
            {
                "target": target,
                "runner": cfg["runner"],
                "binary": cfg["binary"],
                "asset": asset_name(version, target),
            }
        )
    return {"version": version, "matrix": {"include": matrix}}


def verify_assets(dist: Path, plan: dict) -> None:
    expected: set[str] = set()
    for item in plan["matrix"]["include"]:
        expected.add(item["asset"])
        expected.add(item["asset"] + ".sha256")
    actual = {path.name for path in dist.iterdir() if path.is_file()}
    if actual != expected:
        raise ReleaseContractError(
            f"release asset set mismatch: missing={sorted(expected - actual)}, unexpected={sorted(actual - expected)}"
        )

    for item in plan["matrix"]["include"]:
        asset = dist / item["asset"]
        sidecar = dist / f"{item['asset']}.sha256"
        digest = hashlib.sha256(asset.read_bytes()).hexdigest()
        expected_line = f"{digest}  {asset.name}\n"
        if sidecar.read_text(encoding="utf-8") != expected_line:
            raise ReleaseContractError(f"checksum sidecar mismatch for {asset.name}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--policy", type=Path, default=Path(".github/distribution-policy.toml"))
    parser.add_argument("--tag")
    parser.add_argument("--github-output", type=Path)
    parser.add_argument("--dist", type=Path)
    args = parser.parse_args()

    try:
        root = Path(".")
        plan = validate_contract(load_policy(args.policy), load_metadata(root), args.tag)
        if args.dist is not None:
            verify_assets(args.dist, plan)
        if args.github_output is not None:
            with args.github_output.open("a", encoding="utf-8") as stream:
                stream.write(f"version={plan['version']}\n")
                stream.write("matrix=" + json.dumps(plan["matrix"], separators=(",", ":")) + "\n")
        print("release contract verified")
        return 0
    except (OSError, ValueError, ReleaseContractError) as exc:
        print(f"release-contract error: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
