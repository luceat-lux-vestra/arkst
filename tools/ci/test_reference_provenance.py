#!/usr/bin/env python3
"""Negative and mutation tests for the generated/reference-data contract."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))
import verify_reference_provenance as verifier  # noqa: E402


class ProvenanceFixture:
    FILES = (
        "NOTICE",
        "tools/verify_reference_provenance.py",
        "tools/verify_jdk25_unicode.py",
        "tools/generate_jdk25_unicode_case.py",
        "tools/generate_jdk25_locale_data.py",
        "tools/dump_jdk25_unicode_data.java",
        "tools/dump_jdk25_locale_data.java",
        "tools/dump_jdk25_locale_display_data.java",
        "tools/dump_jdk25_locale_oracle.java",
        "tools/markdown-compat/extract_corpus.py",
        "tools/markdown-compat/prepare_references.py",
        "tools/markdown-compat/src/main.rs",
        "tools/jdk25_unicode_corpus.tsv",
        "tools/jdk25_available_locale_order.tsv",
        "crates/scribium-engine/src/unicode_case.rs",
        "crates/scribium-engine/src/locale.rs",
        "crates/scribium-engine/src/locale_data.rs",
        "crates/scribium-engine/data/jdk25_locale_display.bin",
        "tests/compat/corpus/commonmark.json",
        "tests/compat/corpus/gfm.json",
        "tests/compat/baselines/commonmark.json",
        "tests/compat/baselines/gfm.json",
    )

    def __init__(self) -> None:
        self.directory = tempfile.TemporaryDirectory(prefix="scribium-provenance-fixture-")
        self.root = Path(self.directory.name)
        for relative in self.FILES:
            destination = self.root / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.symlink_to(ROOT / relative)
        for relative in (
            "docs/compatibility/quarkdown/reference-jvm.toml",
            "tests/compat/references.toml",
        ):
            destination = self.root / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT / relative, destination)

    def close(self) -> None:
        self.directory.cleanup()

    def mutate(self, relative: str, old: str, new: str) -> None:
        path = self.root / relative
        content = path.read_text(encoding="utf-8")
        self.assert_replacement(content, old, new)
        path.write_text(content.replace(old, new, 1), encoding="utf-8")

    @staticmethod
    def assert_replacement(content: str, old: str, new: str) -> None:
        if content.count(old) != 1:
            raise AssertionError(f"expected one fixture replacement for {old!r}")
        if old == new:
            raise AssertionError("fixture replacement must change content")

    def replace_file(self, relative: str, data: bytes) -> None:
        path = self.root / relative
        path.unlink()
        path.write_bytes(data)


class ReferenceProvenanceTests(unittest.TestCase):
    def setUp(self) -> None:
        self.fixture = ProvenanceFixture()

    def tearDown(self) -> None:
        self.fixture.close()

    def assert_rejected(self, action) -> None:  # type: ignore[no-untyped-def]
        with self.assertRaises(verifier.VerificationError):
            action()

    def verify_fixture(self) -> None:
        verifier.verify_workspace(root=self.fixture.root)

    def test_baseline_fixture_passes(self) -> None:
        self.verify_fixture()

    def test_wrong_archive_hash(self) -> None:
        archive = self.fixture.root / "fake-jdk.tar.gz"
        archive.write_bytes(b"fixture archive")
        reference = {
            "archive_filename": archive.name,
            "archive_bytes": archive.stat().st_size,
            "archive_sha256": hashlib.sha256(archive.read_bytes()).hexdigest(),
        }
        verifier.check_jdk_archive(reference, archive)
        reference["archive_sha256"] = "0" * 64
        self.assert_rejected(lambda: verifier.check_jdk_archive(reference, archive))

    def test_wrong_archive_size(self) -> None:
        archive = self.fixture.root / "fake-jdk.tar.gz"
        archive.write_bytes(b"fixture archive")
        reference = {
            "archive_filename": archive.name,
            "archive_bytes": archive.stat().st_size + 1,
            "archive_sha256": hashlib.sha256(archive.read_bytes()).hexdigest(),
        }
        self.assert_rejected(lambda: verifier.check_jdk_archive(reference, archive))

    def test_wrong_immutable_git_revision(self) -> None:
        self.fixture.mutate(
            "tests/compat/references.toml",
            'revision = "9103e341a973013013bb1a80e13567007c5cef6f"',
            'revision = "0000000000000000000000000000000000000000"',
        )
        self.assert_rejected(self.verify_fixture)

    def test_markdown_repository_requires_https(self) -> None:
        self.fixture.mutate(
            "tests/compat/references.toml",
            'repository = "https://github.com/commonmark/commonmark-spec.git"',
            'repository = "http://github.com/commonmark/commonmark-spec.git"',
        )
        self.assert_rejected(self.verify_fixture)

    def test_markdown_corpus_path_cannot_escape(self) -> None:
        self.fixture.mutate(
            "tests/compat/references.toml",
            'corpus_path = "spec.txt"',
            'corpus_path = "../spec.txt"',
        )
        self.assert_rejected(self.verify_fixture)

    def test_preparation_rejects_escaping_corpus_path(self) -> None:
        script_path = ROOT / "tools/markdown-compat/prepare_references.py"
        spec = importlib.util.spec_from_file_location("prepare_references_fixture", script_path)
        if spec is None or spec.loader is None:
            raise AssertionError("could not load Markdown preparation script")
        preparation = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(preparation)
        with self.assertRaises(SystemExit):
            preparation.repository_relative_path("../spec.txt", "commonmark.corpus_path")
        with self.assertRaises(SystemExit):
            preparation.repository_relative_path("/tmp/spec.txt", "commonmark.corpus_path")

    def test_dirty_tracked_markdown_checkout_rejected(self) -> None:
        checkout = self.fixture.root / "dirty-markdown-checkout"
        checkout.mkdir()
        for command in (
            ("init", "--quiet"),
            ("config", "user.email", "fixture@example.invalid"),
            ("config", "user.name", "Fixture"),
        ):
            subprocess.run(["git", *command], cwd=checkout, check=True, capture_output=True, text=True)
        tracked = checkout / "tracked.txt"
        tracked.write_text("original\n", encoding="utf-8")
        subprocess.run(["git", "add", "tracked.txt"], cwd=checkout, check=True, capture_output=True, text=True)
        subprocess.run(
            ["git", "commit", "--quiet", "-m", "fixture"],
            cwd=checkout,
            check=True,
            capture_output=True,
            text=True,
        )
        tracked.write_text("mutated\n", encoding="utf-8")
        (checkout / "untracked-build-output").write_text("ignored\n", encoding="utf-8")
        self.assert_rejected(lambda: verifier.check_tracked_checkout_clean(checkout, "Markdown.fixture"))

    def test_missing_peeled_jdk_tag_proof_rejected(self) -> None:
        manifest = verifier.read_toml(self.fixture.root / "docs/compatibility/quarkdown/reference-jvm.toml")
        reference = manifest["reference"]
        output = f"{reference['source_revision']}\trefs/tags/{reference['source_tag']}\n"
        self.assert_rejected(lambda: verifier.check_peeled_tag_proof(output, reference))

    def test_wrong_peeled_jdk_tag_proof_rejected(self) -> None:
        manifest = verifier.read_toml(self.fixture.root / "docs/compatibility/quarkdown/reference-jvm.toml")
        reference = manifest["reference"]
        output = (
            f"{reference['source_revision']}\trefs/tags/{reference['source_tag']}\n"
            f"{'0' * 40}\trefs/tags/{reference['source_tag']}^{{}}\n"
        )
        self.assert_rejected(lambda: verifier.check_peeled_tag_proof(output, reference))

    def test_wrong_jvm_vendor(self) -> None:
        self.fixture.mutate(
            "docs/compatibility/quarkdown/reference-jvm.toml",
            'vendor = "Eclipse Adoptium"',
            'vendor = "Unexpected Vendor"',
        )
        self.assert_rejected(self.verify_fixture)

    def test_wrong_jvm_runtime_version(self) -> None:
        self.fixture.mutate(
            "docs/compatibility/quarkdown/reference-jvm.toml",
            'runtime_version = "25.0.4.1+1-LTS"',
            'runtime_version = "25.0.4.0+0-LTS"',
        )
        self.assert_rejected(self.verify_fixture)

    def test_wrong_jvm_version(self) -> None:
        self.fixture.mutate(
            "docs/compatibility/quarkdown/reference-jvm.toml",
            'java_version = "25.0.4.1"',
            'java_version = "25.0.4.0"',
        )
        self.assert_rejected(self.verify_fixture)

    def test_wrong_jdk_source_revision(self) -> None:
        self.fixture.mutate(
            "docs/compatibility/quarkdown/reference-jvm.toml",
            'source_revision = "520406d871955300957ef01e406ac2acd0f9b75c"',
            'source_revision = "0000000000000000000000000000000000000000"',
        )
        self.assert_rejected(self.verify_fixture)

    def test_wrong_unicode_version(self) -> None:
        self.fixture.mutate(
            "docs/compatibility/quarkdown/reference-jvm.toml",
            'unicode_version = "16.0.0"',
            'unicode_version = "15.0.0"',
        )
        self.assert_rejected(self.verify_fixture)

    def test_wrong_unicode_generated_source_record_count(self) -> None:
        self.fixture.mutate(
            "docs/compatibility/quarkdown/reference-jvm.toml",
            "unicode_generated_source_record_count = 68469",
            "unicode_generated_source_record_count = 68468",
        )
        self.assert_rejected(self.verify_fixture)

    def test_wrong_locale_provider(self) -> None:
        self.fixture.mutate(
            "docs/compatibility/quarkdown/reference-jvm.toml",
            'locale_provider = "CLDR"',
            'locale_provider = "COMPAT"',
        )
        self.assert_rejected(self.verify_fixture)

    def test_wrong_generator_identity(self) -> None:
        self.fixture.mutate(
            "docs/compatibility/quarkdown/reference-jvm.toml",
            'unicode_generator_source_sha256 = "2277ccc648379577000c6e4395514be0f24fb5dbd1e3062607a916ab8b178e8a"',
            'unicode_generator_source_sha256 = "' + "0" * 64 + '"',
        )
        self.assert_rejected(self.verify_fixture)

    def test_wrong_helper_identity(self) -> None:
        self.fixture.mutate(
            "docs/compatibility/quarkdown/reference-jvm.toml",
            'helper_source_sha256 = "e10e15f92ef6f996ed117e2d5e3d590a01df511abad7ffc583e349f59b76fa47"',
            'helper_source_sha256 = "' + "0" * 64 + '"',
        )
        self.assert_rejected(self.verify_fixture)

    def test_missing_generated_artifact(self) -> None:
        (self.fixture.root / "crates/scribium-engine/data/jdk25_locale_display.bin").unlink()
        self.assert_rejected(self.verify_fixture)

    def test_unexpected_extra_generated_artifact(self) -> None:
        extra = self.fixture.root / "crates/scribium-engine/data/unexpected.bin"
        extra.write_bytes(b"unexpected")
        self.assert_rejected(self.verify_fixture)

    def test_truncated_generated_artifact(self) -> None:
        source = (ROOT / "crates/scribium-engine/data/jdk25_locale_display.bin").read_bytes()
        self.fixture.replace_file("crates/scribium-engine/data/jdk25_locale_display.bin", source[:128])
        self.assert_rejected(self.verify_fixture)

    def test_expanded_generated_artifact(self) -> None:
        source = (ROOT / "crates/scribium-engine/data/jdk25_locale_display.bin").read_bytes()
        self.fixture.replace_file("crates/scribium-engine/data/jdk25_locale_display.bin", source + b"extra")
        self.assert_rejected(self.verify_fixture)

    def test_output_digest_mismatch(self) -> None:
        source = bytearray((ROOT / "crates/scribium-engine/src/locale_data.rs").read_bytes())
        source[-2] = ord(" ") if source[-2] != ord(" ") else ord("\n")
        self.fixture.replace_file("crates/scribium-engine/src/locale_data.rs", bytes(source))
        self.assert_rejected(self.verify_fixture)

    def test_corpus_digest_mismatch(self) -> None:
        source = bytearray((ROOT / "tests/compat/corpus/commonmark.json").read_bytes())
        source[-2] = ord(" ") if source[-2] != ord(" ") else ord("\n")
        self.fixture.replace_file("tests/compat/corpus/commonmark.json", bytes(source))
        self.assert_rejected(self.verify_fixture)

    def test_corpus_case_count_mismatch(self) -> None:
        self.fixture.mutate(
            "tests/compat/references.toml",
            "extracted_corpus_sha256 = \"ec8d30143a365274907de72235647f47ac2f4e3152e99e1fcfdbc6fdaf5929f0\"\nexpected_case_count = 652",
            "extracted_corpus_sha256 = \"ec8d30143a365274907de72235647f47ac2f4e3152e99e1fcfdbc6fdaf5929f0\"\nexpected_case_count = 651",
        )
        self.assert_rejected(self.verify_fixture)

    def test_missing_or_wrong_required_license_provenance(self) -> None:
        self.fixture.mutate(
            "tests/compat/references.toml",
            'notice_markers = ["commonmark/commonmark-spec", "CC-BY-SA-4.0"]',
            'notice_markers = ["commonmark/commonmark-spec", "missing-license-marker"]',
        )
        self.assert_rejected(self.verify_fixture)
        license_path = self.fixture.root / "fixture-COPYING"
        license_path.write_bytes(b"required license")
        license_metadata = {
            "bytes": license_path.stat().st_size,
            "sha256": hashlib.sha256(license_path.read_bytes()).hexdigest(),
        }
        verifier.check_artifact(
            license_path,
            label="fixture required license",
            expected_bytes=license_metadata["bytes"],
            minimum_bytes=license_metadata["bytes"],
            maximum_bytes=license_metadata["bytes"],
            expected_sha256=license_metadata["sha256"],
            policy="exact",
        )
        license_metadata["sha256"] = "0" * 64
        self.assert_rejected(
            lambda: verifier.check_artifact(
                license_path,
                label="fixture required license",
                expected_bytes=license_metadata["bytes"],
                minimum_bytes=license_metadata["bytes"],
                maximum_bytes=license_metadata["bytes"],
                expected_sha256=license_metadata["sha256"],
                policy="exact",
            )
        )

    def test_independent_result_rejects_producer_success_claim(self) -> None:
        _manifest, refs = verifier.check_markdown_static(
            self.fixture.root,
            self.fixture.root / "tests/compat/references.toml",
        )
        suites = []
        for report_name, reference_name in (("CommonMark", "commonmark"), ("GFM", "cmark_gfm")):
            reference = refs[reference_name]
            expected = reference["expected_result"]
            cases = [
                {"result": "PASS", "new_mismatch": False}
                for _ in range(expected["pass"])
            ]
            cases.extend(
                {"result": "KNOWN_MISMATCH", "new_mismatch": False}
                for _ in range(expected["known_mismatch"])
            )
            suites.append(
                {
                    "name": report_name,
                    "reference_version": reference["version"],
                    "reference_revision": reference["revision"],
                    "baseline_errors": [],
                    "cases": cases,
                }
            )
        report = {"schema_version": 1, "success": True, "errors": [], "suites": suites}
        report_path = self.fixture.root / "producer-report.json"
        suites[0]["cases"][0]["result"] = "KNOWN_MISMATCH"
        report_path.write_text(json.dumps(report), encoding="utf-8")
        self.assert_rejected(lambda: verifier.check_markdown_report(self.fixture.root, refs, report_path))


if __name__ == "__main__":
    unittest.main()
