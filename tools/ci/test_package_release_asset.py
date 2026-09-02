#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import importlib.util
import io
import sys
import tarfile
import tempfile
import unittest
import zipfile
from pathlib import Path

MODULE = Path(__file__).with_name("package_release_asset.py")
spec = importlib.util.spec_from_file_location("package_release_asset", MODULE)
assert spec and spec.loader
mod = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = mod
spec.loader.exec_module(mod)


class ReleasePackageTests(unittest.TestCase):
    def package_twice(self, target: str) -> tuple[bytes, str]:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            binary = root / ("arkst.exe" if "windows" in target else "arkst")
            binary.write_bytes(b"arkst-test-binary\n")
            first = root / "one"
            second = root / "two"
            a1, s1 = mod.package_asset(binary, target, "0.1.0", first)
            a2, s2 = mod.package_asset(binary, target, "0.1.0", second)
            self.assertEqual(a1.read_bytes(), a2.read_bytes())
            self.assertEqual(s1.read_text(), s2.read_text())
            digest = hashlib.sha256(a1.read_bytes()).hexdigest()
            self.assertEqual(s1.read_text(), f"{digest}  {a1.name}\n")
            return a1.read_bytes(), a1.name

    def test_tar_gz_is_deterministic_and_contains_only_binary(self) -> None:
        data, name = self.package_twice("x86_64-unknown-linux-gnu")
        self.assertTrue(name.endswith(".tar.gz"))
        with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as archive:
            members = archive.getmembers()
            self.assertEqual([member.name for member in members], ["arkst"])
            self.assertEqual(members[0].mtime, 0)
            self.assertEqual(members[0].mode, 0o755)
            self.assertEqual(archive.extractfile(members[0]).read(), b"arkst-test-binary\n")

    def test_zip_is_deterministic_and_contains_only_binary(self) -> None:
        data, name = self.package_twice("x86_64-pc-windows-msvc")
        self.assertTrue(name.endswith(".zip"))
        with zipfile.ZipFile(io.BytesIO(data)) as archive:
            self.assertEqual(archive.namelist(), ["arkst.exe"])
            info = archive.getinfo("arkst.exe")
            self.assertEqual(info.date_time, (1980, 1, 1, 0, 0, 0))
            self.assertEqual(archive.read("arkst.exe"), b"arkst-test-binary\n")

    def test_empty_binary_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            binary = root / "arkst"
            binary.write_bytes(b"")
            with self.assertRaisesRegex(mod.PackageError, "empty"):
                mod.package_asset(binary, "x86_64-unknown-linux-gnu", "0.1.0", root / "out")


if __name__ == "__main__":
    unittest.main()
