#!/usr/bin/env python3
"""Verify Arkst's machine-readable license/provenance engineering invariants.

This verifier does not adjudicate copyright infringement. It enforces the
repository's release-clearance inputs: no direct Quarkdown implementation
source/dependency ingestion in protected paths, complete fixture tracking, and
a bounded historical source-exposure audit ledger.
"""
from __future__ import annotations

import argparse
import sys
import tomllib
from pathlib import Path

DEFAULT_ROOT = Path(__file__).resolve().parents[2]
POLICY_PATH = Path(".github/license-provenance-policy.toml")
AUDIT_PATH = Path(".github/license-provenance-audit.toml")
PROVENANCE_PATH = Path("docs/legal/PROVENANCE.md")

IMPLEMENTATION_CLASSIFICATIONS = {
    "FUNCTIONAL_CONTRACT",
    "SOURCE_INFLUENCED_INDEPENDENT",
    "POSSIBLE_TRANSLATION",
    "LITERAL_COPY",
}
FIXTURE_STATUSES = {"INDEPENDENT", "REVIEW_REQUIRED", "REMEDIATED"}
RELEASE_BLOCKING_IMPLEMENTATION = {"POSSIBLE_TRANSLATION", "LITERAL_COPY"}


class VerificationError(RuntimeError):
    pass


def load_toml(root: Path, relative: Path) -> dict:
    path = root / relative
    try:
        with path.open("rb") as stream:
            return tomllib.load(stream)
    except (OSError, tomllib.TOMLDecodeError) as exc:
        raise VerificationError(f"cannot load {relative}: {exc}") from exc


def read_text(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as exc:
        raise VerificationError(f"cannot read {path}: {exc}") from exc


def iter_glob_files(root: Path, patterns: list[str]):
    seen: set[Path] = set()
    for pattern in patterns:
        for path in root.glob(pattern):
            if path.is_file() and path not in seen:
                seen.add(path)
                yield path


def verify_protected_paths(root: Path, policy: dict) -> list[str]:
    findings: list[str] = []
    scan = policy.get("scan", {})
    patterns = scan.get("protected_globs", [])
    markers = scan.get("forbidden_implementation_source_markers", [])
    if not isinstance(patterns, list) or not patterns:
        return ["policy scan.protected_globs must be a non-empty list"]
    if not isinstance(markers, list) or not markers:
        return ["policy scan.forbidden_implementation_source_markers must be a non-empty list"]

    for path in iter_glob_files(root, patterns):
        text = read_text(path)
        for marker in markers:
            if marker in text:
                rel = path.relative_to(root).as_posix()
                findings.append(f"protected path {rel} contains prohibited implementation-source marker {marker!r}")
    return findings


def verify_dependency_ingestion(root: Path, policy: dict) -> list[str]:
    findings: list[str] = []
    dependency = policy.get("dependency", {})
    patterns = dependency.get("globs", [])
    markers = dependency.get("forbidden_markers", [])
    if not isinstance(patterns, list) or not isinstance(markers, list):
        return ["policy dependency globs/markers must be lists"]
    for path in iter_glob_files(root, patterns):
        text = read_text(path)
        relative = path.relative_to(root).as_posix()
        for marker in markers:
            if marker.lower() in text.lower():
                findings.append(f"dependency surface {relative} contains prohibited Quarkdown marker {marker!r}")
    return findings


def fixture_ids(root: Path, policy: dict) -> set[str]:
    root_rel = policy.get("fixtures", {}).get("root", "fixtures/quarkdown-conformance/cases")
    cases = root / root_rel
    if not cases.is_dir():
        raise VerificationError(f"fixture root missing: {root_rel}")
    return {p.name for p in cases.iterdir() if p.is_dir() and (p / "case.toml").is_file()}


def verify_audit_ledger(root: Path, policy: dict, audit: dict, release: bool) -> list[str]:
    findings: list[str] = []
    if audit.get("schema") != 1:
        findings.append("license-provenance audit schema must equal 1")

    source = audit.get("source_exposure", {})
    production_prs = source.get("production_prs", [])
    reviewed_prs = source.get("reviewed_prs", [])
    pending_prs = source.get("pending_prs", [])
    for label, values in (("production_prs", production_prs), ("reviewed_prs", reviewed_prs), ("pending_prs", pending_prs)):
        if not isinstance(values, list) or any(not isinstance(v, int) for v in values):
            findings.append(f"source_exposure.{label} must be an integer list")
    if isinstance(production_prs, list) and isinstance(reviewed_prs, list) and isinstance(pending_prs, list):
        if set(reviewed_prs) & set(pending_prs):
            findings.append("reviewed_prs and pending_prs overlap")
        if set(reviewed_prs) | set(pending_prs) != set(production_prs):
            findings.append("reviewed_prs + pending_prs must exactly cover production_prs")

    entries = audit.get("implementation", [])
    seen_prs: set[int] = set()
    blockers: list[str] = []
    for entry in entries:
        pr = entry.get("pr")
        classification = entry.get("classification")
        if not isinstance(pr, int):
            findings.append("each implementation entry requires integer pr")
            continue
        if pr in seen_prs:
            findings.append(f"duplicate implementation entry for PR #{pr}")
        seen_prs.add(pr)
        if classification not in IMPLEMENTATION_CLASSIFICATIONS:
            findings.append(f"PR #{pr} has invalid classification {classification!r}")
        if classification in RELEASE_BLOCKING_IMPLEMENTATION:
            blockers.append(f"PR #{pr}: {classification}")
        if not str(entry.get("basis", "")).strip():
            findings.append(f"PR #{pr} requires non-empty basis")

    if isinstance(reviewed_prs, list) and set(reviewed_prs) != seen_prs:
        findings.append("implementation entries must exactly cover source_exposure.reviewed_prs")

    actual_fixtures = fixture_ids(root, policy)
    fixture_entries = audit.get("fixture", [])
    tracked: dict[str, str] = {}
    review_required: list[str] = []
    for entry in fixture_entries:
        fixture = entry.get("id")
        status = entry.get("status")
        if not isinstance(fixture, str) or not fixture:
            findings.append("each fixture entry requires non-empty id")
            continue
        if fixture in tracked:
            findings.append(f"duplicate fixture entry {fixture}")
        if status not in FIXTURE_STATUSES:
            findings.append(f"fixture {fixture} has invalid status {status!r}")
        if not str(entry.get("basis", "")).strip():
            findings.append(f"fixture {fixture} requires non-empty basis")
        tracked[fixture] = str(status)
        if status == "REVIEW_REQUIRED":
            review_required.append(fixture)
            if not isinstance(entry.get("issue"), int):
                findings.append(f"fixture {fixture} REVIEW_REQUIRED requires integer issue")
    if set(tracked) != actual_fixtures:
        missing = sorted(actual_fixtures - set(tracked))
        stale = sorted(set(tracked) - actual_fixtures)
        findings.append(f"fixture ledger mismatch: missing={missing} stale={stale}")

    clearance = audit.get("release_clearance", {})
    coverage = clearance.get("coverage")
    if coverage not in {"INCOMPLETE", "COMPLETE"}:
        findings.append("release_clearance.coverage must be INCOMPLETE or COMPLETE")

    enforce_clearance = release or coverage == "COMPLETE"
    if enforce_clearance:
        if coverage != "COMPLETE":
            findings.append("release clearance requires coverage=COMPLETE")
        if pending_prs:
            findings.append(f"release clearance has pending historical source-exposed PRs: {pending_prs}")
        if blockers:
            findings.append(f"release-blocking implementation classifications remain: {blockers}")
        if review_required:
            findings.append(f"release clearance has REVIEW_REQUIRED fixtures: {sorted(review_required)}")

    return findings


def verify_canonical_provenance(root: Path, policy: dict) -> list[str]:
    findings: list[str] = []
    text = read_text(root / PROVENANCE_PATH)
    for phrase in policy.get("provenance", {}).get("forbidden_absolute_claims", []):
        if phrase in text:
            findings.append(f"canonical provenance retains disproven absolute claim {phrase!r}")
    required = policy.get("provenance", {}).get("required_historical_markers", [])
    for phrase in required:
        if phrase not in text:
            findings.append(f"canonical provenance missing required historical marker {phrase!r}")
    return findings


def verify(root: Path, release: bool = False) -> list[str]:
    policy = load_toml(root, POLICY_PATH)
    audit = load_toml(root, AUDIT_PATH)
    findings: list[str] = []
    if policy.get("schema") != 1:
        findings.append("license-provenance policy schema must equal 1")
    findings.extend(verify_protected_paths(root, policy))
    findings.extend(verify_dependency_ingestion(root, policy))
    findings.extend(verify_audit_ledger(root, policy, audit, release))
    findings.extend(verify_canonical_provenance(root, policy))
    return findings


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=DEFAULT_ROOT)
    parser.add_argument("--release", action="store_true", help="enforce finite release-clearance completion criteria")
    args = parser.parse_args()
    try:
        findings = verify(args.root.resolve(), release=args.release)
    except VerificationError as exc:
        print(f"license/provenance verifier infrastructure error: {exc}", file=sys.stderr)
        return 2
    if findings:
        for finding in findings:
            print(f"ERROR: {finding}", file=sys.stderr)
        return 1
    mode = "release" if args.release else "PR"
    print(f"license/provenance {mode} invariants: PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
