#!/usr/bin/env python3
"""Run external Temurin 25 differential checks for the #172 string slice.

The mapping and public-builtin checks are deliberately driven by a fresh Java
oracle invocation. Their transient outputs never become runtime data or
checked-in fixtures.
"""

from __future__ import annotations

import argparse
import hashlib
import os
import subprocess
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
HELPER = ROOT / "tools/dump_jdk25_unicode_data.java"
GENERATOR = ROOT / "tools/generate_jdk25_unicode_case.py"
CORPUS = ROOT / "tools/jdk25_unicode_corpus.tsv"


def environment() -> dict[str, str]:
    result = os.environ.copy()
    result.update({"LC_ALL": "C", "LANG": "C", "TZ": "UTC"})
    return result


def java_command(java: Path, classes: Path, mode: str) -> list[str]:
    return [
        str(java),
        "-Djava.locale.providers=CLDR",
        "-Duser.language=en",
        "-Duser.country=US",
        "-Duser.timezone=UTC",
        "-cp",
        str(classes),
        "DumpJdk25UnicodeData",
        mode,
    ]


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--java", type=Path, required=True)
    parser.add_argument("--archive", type=Path, required=True)
    args = parser.parse_args()

    subprocess.run(
        [
            sys.executable,
            str(GENERATOR),
            "--java",
            str(args.java),
            "--archive",
            str(args.archive),
            "--check",
        ],
        cwd=ROOT,
        check=True,
        env=environment(),
    )

    javac = args.java.parent / "javac"
    if not javac.is_file():
        raise SystemExit(f"matching javac executable does not exist: {javac}")

    with tempfile.TemporaryDirectory(prefix="arkst-jdk25-differential-") as temporary:
        directory = Path(temporary)
        classes = directory / "classes"
        classes.mkdir()
        maps = directory / "maps.tsv"
        corpus_output = directory / "corpus.out"
        subprocess.run(
            [str(javac), "-d", str(classes), str(HELPER)],
            cwd=ROOT,
            check=True,
            env=environment(),
        )
        map_result = subprocess.run(
            java_command(args.java, classes, "--maps"),
            cwd=ROOT,
            check=True,
            capture_output=True,
            env=environment(),
        )
        maps.write_bytes(map_result.stdout)
        corpus_result = subprocess.run(
            java_command(args.java, classes, "--corpus"),
            cwd=ROOT,
            check=True,
            input=CORPUS.read_bytes(),
            capture_output=True,
            env=environment(),
        )
        corpus_output.write_bytes(corpus_result.stdout)

        test_environment = environment()
        test_environment.update(
            {
                "ARKST_JDK25_UNICODE_MAPS": str(maps),
                "ARKST_JDK25_UNICODE_CORPUS": str(CORPUS),
                "ARKST_JDK25_UNICODE_CORPUS_OUTPUT": str(corpus_output),
            }
        )
        for test_name in (
            "jdk25_oracle_matches_all_generated_case_mappings",
            "jdk25_oracle_matches_public_string_builtins",
        ):
            result = subprocess.run(
                [
                    "cargo",
                    "test",
                    "-p",
                    "arkst-engine",
                    "--locked",
                    f"builtins::tests::{test_name}",
                    "--",
                    "--exact",
                ],
                cwd=ROOT,
                check=True,
                capture_output=True,
                text=True,
                env=test_environment,
            )
            if "running 1 test" not in result.stdout or "1 passed" not in result.stdout:
                raise RuntimeError(f"focused test did not execute exactly one test: {test_name}")
            print(result.stdout, end="")
            print(result.stderr, end="")

        print(f"mapping_oracle_bytes={len(map_result.stdout)}")
        print(f"mapping_oracle_sha256={hashlib.sha256(map_result.stdout).hexdigest()}")
        print(f"public_corpus_results={len(corpus_result.stdout.splitlines())}")


if __name__ == "__main__":
    main()
