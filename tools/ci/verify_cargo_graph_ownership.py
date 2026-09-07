#!/usr/bin/env python3
from __future__ import annotations

import re
import sys
import tomllib
from pathlib import Path


class CargoGraphOwnershipError(RuntimeError):
    pass


RESEARCH_ROOT = Path("tools/spikes")
AUDITED_RESEARCH_MANIFEST = Path("tools/spikes/typst-in-process/Cargo.toml")
RESEARCH_MANIFESTS = {
    Path("tools/spikes/markdown-final-feasibility/Cargo.toml"),
    Path("tools/spikes/markdown-maintained-substrate-check/Cargo.toml"),
    Path("tools/spikes/markdown-substrate/Cargo.toml"),
    Path("tools/spikes/rushdown-safety-gate/Cargo.toml"),
    AUDITED_RESEARCH_MANIFEST,
    Path("tools/spikes/typst-subprocess/Cargo.toml"),
}
SUPPLY_CHAIN_WORKFLOWS = {
    Path(".github/workflows/ci.yml"): ("deny", "license"),
    Path(".github/workflows/security.yml"): ("audit", None),
}
CARGO_DENY_ACTION_REF = (
    "EmbarkStudios/cargo-deny-action@3c6349835b2b7b196a839186cb8b78e02f7b5f25"
)
CARGO_DENY_ACTION_FAMILY = "EmbarkStudios/cargo-deny-action@"
CARGO_DENY_ACTION = re.compile(
    rf"^\s*-\s+uses:\s+{re.escape(CARGO_DENY_ACTION_REF)}\s*(?:#.*)?$"
)
HEX40 = re.compile(r"^[0-9a-f]{40}$")
EXACT_VERSION = re.compile(
    r"^=\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$"
)


def load_toml(path: Path) -> dict:
    try:
        return tomllib.loads(path.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as exc:
        raise CargoGraphOwnershipError(f"cannot read {path.as_posix()}: {exc}") from exc


def dependency_tables(data: dict) -> list[tuple[str, dict]]:
    result: list[tuple[str, dict]] = []
    for name in ("dependencies", "dev-dependencies", "build-dependencies"):
        table = data.get(name)
        if isinstance(table, dict):
            result.append((name, table))
    targets = data.get("target")
    if isinstance(targets, dict):
        for target_name, target in targets.items():
            if not isinstance(target, dict):
                continue
            for name in ("dependencies", "dev-dependencies", "build-dependencies"):
                table = target.get(name)
                if isinstance(table, dict):
                    result.append((f"target.{target_name}.{name}", table))
    return result


def override_tables(data: dict) -> list[tuple[str, dict]]:
    result: list[tuple[str, dict]] = []
    patch = data.get("patch")
    if isinstance(patch, dict):
        for source, table in patch.items():
            if isinstance(table, dict):
                result.append((f"patch.{source}", table))
    replace = data.get("replace")
    if isinstance(replace, dict):
        result.append(("replace", replace))
    return result


def exact_dependency(value: object) -> bool:
    if isinstance(value, str):
        return EXACT_VERSION.fullmatch(value) is not None
    if not isinstance(value, dict):
        return False
    if "git" in value:
        rev = value.get("rev")
        return isinstance(rev, str) and HEX40.fullmatch(rev) is not None
    version = value.get("version")
    return isinstance(version, str) and EXACT_VERSION.fullmatch(version) is not None


def workspace_members(root: Path, root_data: dict) -> list[Path]:
    workspace = root_data.get("workspace")
    members = workspace.get("members") if isinstance(workspace, dict) else None
    if not isinstance(members, list) or not members:
        raise CargoGraphOwnershipError("Cargo.toml: workspace.members must be a non-empty list")
    result = []
    for member in members:
        if not isinstance(member, str) or any(ch in member for ch in "*?["):
            raise CargoGraphOwnershipError(
                f"Cargo.toml: workspace member {member!r} must be an explicit path"
            )
        result.append((root / member).resolve())
    return result


def verify_research_registry(root: Path) -> None:
    discovered = {
        path.relative_to(root)
        for path in (root / RESEARCH_ROOT).rglob("Cargo.toml")
        if "target" not in path.relative_to(root).parts
    }
    extra = discovered - RESEARCH_MANIFESTS
    missing = RESEARCH_MANIFESTS - discovered
    if extra:
        raise CargoGraphOwnershipError(
            "unregistered research Cargo manifest(s): "
            + ", ".join(sorted(path.as_posix() for path in extra))
        )
    if missing:
        raise CargoGraphOwnershipError(
            "registered research Cargo manifest(s) missing: "
            + ", ".join(sorted(path.as_posix() for path in missing))
        )

    locks = [
        path.relative_to(root)
        for path in (root / RESEARCH_ROOT).rglob("Cargo.lock")
        if "target" not in path.relative_to(root).parts
    ]
    if locks:
        raise CargoGraphOwnershipError(
            "research Cargo recipes must not commit lockfiles: "
            + ", ".join(sorted(path.as_posix() for path in locks))
        )


def verify_research_manifests(root: Path, root_data: dict) -> None:
    workspace = root_data.get("workspace")
    excludes = workspace.get("exclude") if isinstance(workspace, dict) else []
    excluded = {
        (root / value).resolve()
        for value in excludes
        if isinstance(value, str)
    }

    for rel in sorted(RESEARCH_MANIFESTS):
        data = load_toml(root / rel)
        package = data.get("package")
        if not isinstance(package, dict) or package.get("publish") is not False:
            raise CargoGraphOwnershipError(
                f"{rel.as_posix()}: research package must set publish = false"
            )

        research_workspace = data.get("workspace")
        if isinstance(research_workspace, dict) and any(
            key in research_workspace for key in ("members", "exclude", "default-members")
        ):
            raise CargoGraphOwnershipError(
                f"{rel.as_posix()}: containment [workspace] must not declare members/excludes/default-members"
            )
        if (
            not isinstance(research_workspace, dict)
            and (root / rel).parent.resolve() not in excluded
        ):
            raise CargoGraphOwnershipError(
                f"{rel.as_posix()}: must declare containment [workspace] or be listed in root workspace.exclude"
            )

        for table_name, table in dependency_tables(data):
            for name, value in table.items():
                if not exact_dependency(value):
                    raise CargoGraphOwnershipError(
                        f"{rel.as_posix()}: {table_name}.{name} must use an exact full version or immutable git rev"
                    )


def points_into_research(root: Path, manifest: Path, value: object) -> bool:
    if not isinstance(value, dict) or not isinstance(value.get("path"), str):
        return False
    dependency = (manifest.parent / value["path"]).resolve()
    research_root = (root / RESEARCH_ROOT).resolve()
    try:
        dependency.relative_to(research_root)
    except ValueError:
        return False
    return True


def verify_production_boundary(root: Path, root_data: dict) -> None:
    research_root = (root / RESEARCH_ROOT).resolve()
    members = workspace_members(root, root_data)
    for member in members:
        try:
            member.relative_to(research_root)
        except ValueError:
            pass
        else:
            raise CargoGraphOwnershipError(
                f"production workspace must not include research path {member.relative_to(root).as_posix()}"
            )

    manifests = [root / "Cargo.toml", *(member / "Cargo.toml" for member in members)]
    for manifest in manifests:
        data = load_toml(manifest)
        tables = dependency_tables(data) + override_tables(data)
        if manifest == root / "Cargo.toml":
            workspace = data.get("workspace")
            workspace_deps = (
                workspace.get("dependencies") if isinstance(workspace, dict) else None
            )
            if isinstance(workspace_deps, dict):
                tables.append(("workspace.dependencies", workspace_deps))
        for table_name, table in tables:
            for name, value in table.items():
                if points_into_research(root, manifest, value):
                    raise CargoGraphOwnershipError(
                        f"{manifest.relative_to(root).as_posix()}: {table_name}.{name} must not depend on or override from research path"
                    )


def citationberg_patch(data: dict, rel: Path) -> tuple[str, str]:
    patch = data.get("patch")
    crates_io = patch.get("crates-io") if isinstance(patch, dict) else None
    value = crates_io.get("citationberg") if isinstance(crates_io, dict) else None
    git = value.get("git") if isinstance(value, dict) else None
    rev = value.get("rev") if isinstance(value, dict) else None
    if not isinstance(git, str) or not isinstance(rev, str) or HEX40.fullmatch(rev) is None:
        raise CargoGraphOwnershipError(
            f"{rel.as_posix()}: citationberg patch must use git plus immutable 40-hex rev"
        )
    return git, rev


def leading_spaces(line: str) -> int:
    return len(line) - len(line.lstrip(" "))


def job_block(path: Path, job_id: str) -> str:
    lines = path.read_text(encoding="utf-8").splitlines()
    start: int | None = None
    for index, line in enumerate(lines):
        if leading_spaces(line) == 2 and line.strip() == f"{job_id}:":
            start = index
            break
    if start is None:
        raise CargoGraphOwnershipError(
            f"{path.as_posix()}: required supply-chain job {job_id!r} not found"
        )

    end = len(lines)
    for index in range(start + 1, len(lines)):
        line = lines[index]
        if leading_spaces(line) == 2 and line.strip().endswith(":") and not line.lstrip().startswith("#"):
            end = index
            break
    return "\n".join(lines[start:end])


def direct_field(block: str, name: str, indent: int) -> str | None:
    prefix = f"{name}:"
    for line in block.splitlines():
        if leading_spaces(line) != indent:
            continue
        stripped = line.strip()
        if stripped == prefix or stripped.startswith(prefix + " "):
            value = stripped[len(prefix) :].strip()
            return value.strip("\"'")
    return None


def action_blocks(text: str) -> list[str]:
    lines = text.splitlines()
    result: list[str] = []
    index = 0
    while index < len(lines):
        line = lines[index]
        if CARGO_DENY_ACTION_FAMILY in line and CARGO_DENY_ACTION.match(line) is None:
            raise CargoGraphOwnershipError(
                f"cargo-deny authority must use exact action ref {CARGO_DENY_ACTION_REF}"
            )
        if CARGO_DENY_ACTION.match(line) is None:
            index += 1
            continue
        indent = leading_spaces(line)
        block = [line]
        index += 1
        while index < len(lines):
            stripped = lines[index].lstrip()
            if stripped.startswith("- ") and leading_spaces(lines[index]) == indent:
                break
            block.append(lines[index])
            index += 1
        result.append("\n".join(block))
    return result


def field(block: str, name: str) -> str | None:
    match = re.search(rf"^\s+{re.escape(name)}:\s*(.*?)\s*$", block, re.MULTILINE)
    return None if match is None else match.group(1).strip("\"'")


def step_field(block: str, name: str) -> str | None:
    lines = block.splitlines()
    if not lines:
        return None
    return direct_field(block, name, leading_spaces(lines[0]) + 2)


def reject_conditional_or_nonblocking(path: Path, authority: str, block: str) -> None:
    if step_field(block, "if") is not None:
        raise CargoGraphOwnershipError(
            f"{path.as_posix()}: {authority} cargo-deny authority must not be conditional"
        )
    if step_field(block, "continue-on-error") is not None:
        raise CargoGraphOwnershipError(
            f"{path.as_posix()}: {authority} cargo-deny authority must not allow failure"
        )


def verify_supply_chain_workflow(
    path: Path, job_id: str, required_name: str | None
) -> None:
    job = job_block(path, job_id)
    if required_name is not None and direct_field(job, "name", 4) != required_name:
        raise CargoGraphOwnershipError(
            f"{path.as_posix()}: supply-chain job {job_id!r} must produce {required_name!r}"
        )
    if direct_field(job, "if", 4) is not None:
        raise CargoGraphOwnershipError(
            f"{path.as_posix()}: supply-chain authority job must not be conditional"
        )
    if direct_field(job, "continue-on-error", 4) is not None:
        raise CargoGraphOwnershipError(
            f"{path.as_posix()}: supply-chain authority job must not allow failure"
        )

    blocks = action_blocks(job)
    production = [
        block
        for block in blocks
        if field(block, "manifest-path") in {None, "Cargo.toml", "./Cargo.toml"}
    ]
    research = [
        block
        for block in blocks
        if field(block, "manifest-path") == AUDITED_RESEARCH_MANIFEST.as_posix()
    ]
    if len(production) != 1:
        raise CargoGraphOwnershipError(
            f"{path.as_posix()}: expected exactly one production cargo-deny authority"
        )
    if len(research) != 1:
        raise CargoGraphOwnershipError(
            f"{path.as_posix()}: expected exactly one audited research cargo-deny authority"
        )
    if len(blocks) != 2:
        raise CargoGraphOwnershipError(
            f"{path.as_posix()}: unexpected additional cargo-deny authority in supply-chain job"
        )

    reject_conditional_or_nonblocking(path, "production", production[0])
    reject_conditional_or_nonblocking(path, "research", research[0])

    if (
        field(production[0], "command") != "check"
        or field(production[0], "arguments") != "--all-features"
        or field(production[0], "command-arguments") not in {None, ""}
    ):
        raise CargoGraphOwnershipError(
            f"{path.as_posix()}: production cargo-deny must run full check --all-features"
        )
    checks = set((field(research[0], "command-arguments") or "").split())
    if (
        field(research[0], "command") != "check"
        or field(research[0], "arguments") != "--all-features"
        or checks != {"advisories", "sources"}
    ):
        raise CargoGraphOwnershipError(
            f"{path.as_posix()}: audited research cargo-deny must run advisories and sources"
        )


def verify_workflow_containment(root: Path) -> None:
    allowed = AUDITED_RESEARCH_MANIFEST.as_posix()
    allowed_workflows = {path.as_posix() for path in SUPPLY_CHAIN_WORKFLOWS}
    allowed_lines = {
        f"manifest-path: {allowed}",
        f'manifest-path: "{allowed}"',
        f"manifest-path: '{allowed}'",
    }
    for workflow in sorted((root / ".github/workflows").glob("*.y*ml")):
        rel = workflow.relative_to(root).as_posix()
        lines = workflow.read_text(encoding="utf-8").splitlines()
        literal_references = sum(line.strip() in allowed_lines for line in lines)
        if rel in allowed_workflows and literal_references != 1:
            raise CargoGraphOwnershipError(
                f"{rel}: expected exactly one literal audited research manifest reference"
            )
        for number, line in enumerate(lines, 1):
            if RESEARCH_ROOT.as_posix() not in line:
                continue
            if rel in allowed_workflows and line.strip() in allowed_lines:
                continue
            raise CargoGraphOwnershipError(
                f"{rel}:{number}: GitHub Actions must not execute/package unaudited research paths"
            )


def verify_repository(root: Path) -> None:
    root = root.resolve()
    root_data = load_toml(root / "Cargo.toml")
    verify_research_registry(root)
    verify_research_manifests(root, root_data)
    verify_production_boundary(root, root_data)

    production_patch = citationberg_patch(root_data, Path("Cargo.toml"))
    research_patch = citationberg_patch(
        load_toml(root / AUDITED_RESEARCH_MANIFEST), AUDITED_RESEARCH_MANIFEST
    )
    if production_patch != research_patch:
        raise CargoGraphOwnershipError(
            f"{AUDITED_RESEARCH_MANIFEST.as_posix()}: citationberg patch must exactly match production"
        )

    for workflow, (job_id, required_name) in SUPPLY_CHAIN_WORKFLOWS.items():
        verify_supply_chain_workflow(root / workflow, job_id, required_name)
    verify_workflow_containment(root)


def main() -> int:
    try:
        verify_repository(Path("."))
    except (OSError, CargoGraphOwnershipError) as exc:
        print(f"cargo-graph-ownership error: {exc}", file=sys.stderr)
        return 1
    print("research Cargo recipes are registered, isolated, pinned, and contained")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())