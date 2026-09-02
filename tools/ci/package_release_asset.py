#!/usr/bin/env python3
from __future__ import annotations

import argparse
import gzip
import hashlib
import io
import tarfile
import zipfile
from pathlib import Path

from verify_release_contract import TARGETS, asset_name


class PackageError(RuntimeError):
    pass


def package_asset(binary: Path, target: str, version: str, output_dir: Path) -> tuple[Path, Path]:
    if target not in TARGETS:
        raise PackageError(f"unsupported release target: {target}")
    if not binary.is_file():
        raise PackageError(f"release binary does not exist: {binary}")
    data = binary.read_bytes()
    if not data:
        raise PackageError(f"release binary is empty: {binary}")

    output_dir.mkdir(parents=True, exist_ok=True)
    cfg = TARGETS[target]
    name = asset_name(version, target)
    archive = output_dir / name
    member_name = cfg["binary"]

    if cfg["format"] == "tar.gz":
        with archive.open("wb") as raw:
            with gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) as compressed:
                with tarfile.open(fileobj=compressed, mode="w", format=tarfile.PAX_FORMAT) as tar:
                    info = tarfile.TarInfo(member_name)
                    info.size = len(data)
                    info.mode = 0o755
                    info.uid = 0
                    info.gid = 0
                    info.uname = ""
                    info.gname = ""
                    info.mtime = 0
                    tar.addfile(info, io.BytesIO(data))
    elif cfg["format"] == "zip":
        info = zipfile.ZipInfo(member_name, date_time=(1980, 1, 1, 0, 0, 0))
        info.create_system = 3
        info.external_attr = 0o100755 << 16
        info.compress_type = zipfile.ZIP_DEFLATED
        with zipfile.ZipFile(archive, "w") as zf:
            zf.writestr(info, data)
    else:
        raise PackageError(f"unsupported archive format: {cfg['format']}")

    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    sidecar = output_dir / f"{name}.sha256"
    sidecar.write_text(f"{digest}  {name}\n", encoding="utf-8")
    return archive, sidecar


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--target", required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    try:
        archive, sidecar = package_asset(args.binary, args.target, args.version, args.output_dir)
        print(archive)
        print(sidecar)
        return 0
    except (OSError, PackageError) as exc:
        print(f"release-package error: {exc}")
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
