#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
HELPER = ROOT / "tools/dump_jdk25_unicode_data.java"
GENERATOR = ROOT / "tools/generate_jdk25_unicode_case.py"
MANIFEST = ROOT / "docs/compatibility/quarkdown/reference-jvm.toml"
CORPUS = ROOT / "tools/jdk25_unicode_corpus.tsv"
OUTPUT = ROOT / "crates/arkst-engine/src/unicode_case.rs"


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def replace_once(text: str, old: str, new: str, label: str) -> str:
    if text.count(old) != 1:
        raise RuntimeError(f"{label}: expected exactly one occurrence")
    return text.replace(old, new, 1)


def sub_once(text: str, pattern: str, replacement: str, label: str) -> str:
    updated, count = re.subn(pattern, lambda _: replacement, text, count=1, flags=re.S | re.M)
    if count != 1:
        raise RuntimeError(f"{label}: expected exactly one regex match, got {count}")
    return updated


def load_generator():
    spec = importlib.util.spec_from_file_location("arkst_unicode_generator", GENERATOR)
    if spec is None or spec.loader is None:
        raise RuntimeError("cannot load Unicode generator")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def set_toml_string(text: str, key: str, value: str) -> str:
    updated, count = re.subn(rf'^{re.escape(key)} = ".*"$', f'{key} = "{value}"', text, count=1, flags=re.M)
    if count != 1:
        raise RuntimeError(f"manifest: missing string field {key}")
    return updated


def set_toml_int(text: str, key: str, value: int) -> str:
    updated, count = re.subn(rf'^{re.escape(key)} = \d+$', f'{key} = {value}', text, count=1, flags=re.M)
    if count != 1:
        raise RuntimeError(f"manifest: missing integer field {key}")
    return updated


def run(*args: str) -> None:
    print("+", " ".join(args), flush=True)
    subprocess.run(args, cwd=ROOT, check=True)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--java", type=Path, required=True)
    parser.add_argument("--archive", type=Path, required=True)
    args = parser.parse_args()

    helper = HELPER.read_text(encoding="utf-8")
    helper = sub_once(
        helper,
        r'''\n        // The pinned JDK's Final_Cased implementation also consults its\n        // locale-root word-boundary iterator\..*?\n        }\n    }\n\n    private static boolean isCased''',
        "\n    }\n\n    private static boolean isCased",
        "remove scalar final-sigma oracle loop",
    )
    helper = sub_once(
        helper,
        r'''\n    private static boolean isFinalSigmaContext\(int codePoint\) \{.*?\n    }\n''',
        "\n",
        "remove scalar final-sigma helper",
    )
    if "FINAL_SIGMA" in helper or "isFinalSigmaContext" in helper:
        raise RuntimeError("legacy scalar final-sigma helper remains")
    HELPER.write_text(helper, encoding="utf-8")

    generator = GENERATOR.read_text(encoding="utf-8")
    generator = replace_once(
        generator,
        ") -> tuple[list[tuple[int, int, int]], list[dict[str, object]], list[int], list[int]]:",
        ") -> tuple[list[tuple[int, int, int]], list[dict[str, object]], list[int]]:",
        "parse_oracle return type",
    )
    for old in [
        "    final_sigma_context_rows: list[int] = []\n",
        "    final_sigma_context_previous = -1\n",
        "    saw_final_sigma_context = False\n",
    ]:
        generator = replace_once(generator, old, "", "parse_oracle legacy state")
    generator = generator.replace(" or saw_final_sigma_context", "")
    generator = generator.replace(" or saw_final_sigma_context", "")
    generator = replace_once(
        generator,
        "            if not saw_chars or saw_final_sigma_context or len(fields) != 2:\n",
        "            if not saw_chars or len(fields) != 2:\n",
        "CASED ordering",
    ) if "if not saw_chars or saw_final_sigma_context or len(fields) != 2:" in generator else generator
    generator = sub_once(
        generator,
        r'''\n        elif fields\[0\] == "FINAL_SIGMA":.*?            saw_final_sigma_context = True''',
        "",
        "remove FINAL_SIGMA parser branch",
    )
    generator = replace_once(
        generator,
        '''    if not final_sigma_context_rows:\n        raise ValueError("oracle: FINAL_SIGMA property is empty")\n    return scalar_rows, char_rows, cased_rows, final_sigma_context_rows\n''',
        "    return scalar_rows, char_rows, cased_rows\n",
        "parse_oracle return",
    )
    generator = replace_once(
        generator,
        "    final_sigma_context_rows: list[int],\n",
        "",
        "generate_source parameter",
    )
    generator = replace_once(
        generator,
        '''//! The generated Cased property supports the invariant-locale\n//! context-sensitive Final_Sigma rule used by complete-string lowercase, and\n//! the generated Final_Sigma context property captures the pinned JDK word\n//! boundary behavior without a runtime locale or word-break dependency.\n''',
        '''//! The generated Cased property supports invariant-locale contextual\n//! Final_Sigma lowering. Word boundaries are generated separately from the same\n//! pinned runtime into `word_break.rs`; no scalar approximation is retained here.\n''',
        "generated docs",
    )
    generator = replace_once(
        generator,
        "pub const FINAL_SIGMA_CONTEXT_RECORD_COUNT: usize = {len(final_sigma_context_rows)};\n",
        "",
        "generated legacy count",
    )
    generator = replace_once(
        generator,
        '''\npub(crate) fn is_final_sigma_context(character: char) -> bool {{\n    contains(character as u32, FINAL_SIGMA_CONTEXT_RANGES)\n}}\n''',
        "",
        "generated legacy API",
    )
    generator = replace_once(
        generator,
        '{rust_property_ranges("CASED_RANGES", cased_rows)}\n{rust_property_ranges("FINAL_SIGMA_CONTEXT_RANGES", final_sigma_context_rows)}\'\'\'',
        '{rust_property_ranges("CASED_RANGES", cased_rows)}\'\'\'',
        "generated legacy table",
    )
    generator = replace_once(
        generator,
        "    scalar_rows, char_rows, cased_rows, final_sigma_context_rows = parse_oracle(oracle_output)\n",
        "    scalar_rows, char_rows, cased_rows = parse_oracle(oracle_output)\n",
        "main parse unpack",
    )
    generator = replace_once(
        generator,
        "        cased_rows,\n        final_sigma_context_rows,\n        reference,\n",
        "        cased_rows,\n        reference,\n",
        "main generate args",
    )
    generator = replace_once(
        generator,
        '    print(f"final_sigma_context_records={len(final_sigma_context_rows)}")\n',
        "",
        "main legacy metric",
    )
    if "final_sigma_context" in generator.lower() or "FINAL_SIGMA" in generator:
        raise RuntimeError("legacy scalar final-sigma generator state remains")
    GENERATOR.write_text(generator, encoding="utf-8")

    corpus = CORPUS.read_text(encoding="utf-8")
    additions = ["LOWER\t𐐀Σ", "LOWER\tΣ𐐀", "LOWER\t𐐀'Σ"]
    for line in additions:
        if line not in corpus.splitlines():
            corpus = corpus.rstrip() + "\n" + line + "\n"
    CORPUS.write_text(corpus, encoding="utf-8")

    # Rebuild the case artifact directly from the transformed generator before
    # updating exact manifest fingerprints, so no verification is weakened.
    module = load_generator()
    reference = module.manifest()
    module.validate_reference(reference)
    module.validate_archive(args.archive, reference)
    javac = module.validate_java(args.java, reference)
    oracle_output, oracle_sha = module.build_oracle(args.java, javac)
    scalar_rows, char_rows, cased_rows = module.parse_oracle(oracle_output)
    generated = module.generate_source(scalar_rows, char_rows, cased_rows, reference, oracle_sha)
    OUTPUT.write_text(generated, encoding="utf-8")

    generated_bytes = len(generated.encode("utf-8"))
    generated_sha = hashlib.sha256(generated.encode("utf-8")).hexdigest()
    corpus_bytes = CORPUS.read_bytes()
    corpus_count = sum(1 for line in corpus.splitlines() if line and not line.startswith("#"))

    manifest = MANIFEST.read_text(encoding="utf-8")
    manifest = set_toml_string(manifest, "helper_source_sha256", sha(HELPER))
    manifest = set_toml_string(manifest, "oracle_output_sha256", oracle_sha)
    manifest = set_toml_string(manifest, "generated_source_sha256", generated_sha)
    manifest = set_toml_string(manifest, "unicode_generator_source_sha256", sha(GENERATOR))
    manifest = set_toml_string(manifest, "unicode_corpus_sha256", hashlib.sha256(corpus_bytes).hexdigest())
    manifest = set_toml_int(manifest, "unicode_corpus_bytes", len(corpus_bytes))
    manifest = set_toml_int(manifest, "unicode_corpus_min_bytes", len(corpus_bytes))
    manifest = set_toml_int(manifest, "unicode_corpus_max_bytes", len(corpus_bytes))
    manifest = set_toml_int(manifest, "unicode_corpus_record_count", corpus_count)
    manifest = set_toml_int(manifest, "unicode_generated_source_bytes", generated_bytes)
    manifest = set_toml_int(manifest, "unicode_generated_source_min_bytes", generated_bytes)
    manifest = set_toml_int(manifest, "unicode_generated_source_max_bytes", generated_bytes)
    MANIFEST.write_text(manifest, encoding="utf-8")

    # The provenance verifier itself is unchanged; all exact identities should
    # now agree with the new bounded artifacts.
    run(sys.executable, "tools/generate_jdk25_unicode_case.py", "--java", str(args.java), "--archive", str(args.archive), "--check")
    run(sys.executable, "tools/generate_word_break_jdk25.py", "--java", str(args.java), "--archive", str(args.archive), "--check")
    run(sys.executable, "tools/verify_jdk25_unicode.py", "--java", str(args.java), "--archive", str(args.archive))
    run(sys.executable, "tools/verify_reference_provenance.py")
    run("cargo", "fmt", "--all", "--", "--check")
    run("cargo", "test", "-p", "arkst-engine", "--all-targets", "--all-features")
    run("cargo", "test", "-p", "arkst-core", "--test", "quarkdown_localization")
    run("git", "diff", "--check")

    # Cargo invocations may reorder equivalent workspace lock entries. Restore
    # exact HEAD lockfile bytes so this bounded compatibility fix never owns it.
    subprocess.run(["git", "checkout", "HEAD", "--", "Cargo.lock"], cwd=ROOT, check=True)


if __name__ == "__main__":
    main()
