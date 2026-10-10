#!/usr/bin/env python3
"""Offline, fail-closed timing evidence from one *successful* Rust CodeQL job log.

Not a CodeQL profiler or a security result. Does not call GitHub, read CodeQL
databases, or upload artifacts. Prints only static metric names and numbers.
Query completion gaps are observations, never per-query costs.
"""
from __future__ import annotations

import argparse
from datetime import datetime
import json
from pathlib import Path
import re
import sys

EXPECTED_CLI = "2.27.2"
EXPECTED_PACK = "0.1.44"
EXPECTED_QUERIES = 37
EXPECTED_EXTRACTED = 167
MAX_BYTES = 8 * 1024 * 1024

STAMP = re.compile(r"^\ufeff?(20\d\d-\d\d-\d\dT\d\d:\d\d:\d\d\.\d+Z) ")
EVAL = re.compile(r"^\[(\d+)/(\d+) eval [^\]]+\] Evaluation done; writing results to codeql/rust-queries/queries/")
FILE_OK = re.compile(r"^\| Total number of Rust files that were extracted without error\s*\|\s*(\d+)\s*\|$")
FILE_BAD = re.compile(r"^\| Total number of Rust files that were extracted with errors\s*\|\s*(\d+)\s*\|$")
PACK = re.compile(r"codeql/rust-queries/(\d+\.\d+\.\d+)")
CLI = re.compile(r"/CodeQL/(\d+\.\d+\.\d+)/")
MARKERS = {
    "extraction": "##[group]Extracting rust",
    "finalize": "##[group]Finalizing rust",
    "queries": "##[group]Running queries for rust",
    "shutdown": "Shutting down query evaluator.",
    "upload_confirmed": "Analysis upload status is complete.",
}
SUCCESS = "CodeQL job status was success."


class IncompleteEvidence(ValueError):
    """A deliberately fixed failure class; never render raw log contents."""


def seconds(a: datetime, b: datetime) -> float:
    n = (b - a).total_seconds()
    if n < 0 or n > 1800:
        raise IncompleteEvidence("invalid_duration")
    return round(n, 3)


def parse_log(content: str) -> dict:
    if not content or len(content.encode("utf-8")) > MAX_BYTES:
        raise IncompleteEvidence("invalid_input_size")

    moments: dict[str, datetime] = {}
    queries: list[datetime] = []
    ok: list[int] = []
    bad: list[int] = []
    seen_cli: set[str] = set()
    seen_pack: set[str] = set()
    success = 0
    first: datetime | None = None
    last: datetime | None = None

    for line in content.splitlines():
        m = STAMP.match(line)
        if not m:
            continue
        try:
            ts = datetime.fromisoformat(m.group(1).replace("Z", "+00:00"))
        except ValueError as exc:
            raise IncompleteEvidence("malformed_timestamp") from exc
        if last is not None and ts < last:
            raise IncompleteEvidence("nonmonotonic_log_timestamp")
        first = first or ts
        last = ts
        message = line[m.end():]
        for key, literal in MARKERS.items():
            if literal in message:
                if key in moments:
                    raise IncompleteEvidence("duplicate_phase")
                moments[key] = ts
        e = EVAL.match(message)
        if e:
            ordinal, total = map(int, e.groups())
            if total != EXPECTED_QUERIES or ordinal != len(queries) + 1:
                raise IncompleteEvidence("query_count_or_order_mismatch")
            queries.append(ts)
        f_ok = FILE_OK.match(message)
        if f_ok:
            ok.append(int(f_ok.group(1)))
        f_bad = FILE_BAD.match(message)
        if f_bad:
            bad.append(int(f_bad.group(1)))
        # Only version identities are collected, never arbitrary strings/paths.
        seen_cli.update(CLI.findall(message))
        seen_pack.update(PACK.findall(message))
        if SUCCESS in message:
            success += 1

    if set(moments) != set(MARKERS) or first is None or last is None:
        raise IncompleteEvidence("incomplete_phase_markers")
    if seen_cli != {EXPECTED_CLI} or seen_pack != {EXPECTED_PACK}:
        raise IncompleteEvidence("cli_or_pack_mismatch")
    if ok != [EXPECTED_EXTRACTED] or bad != [0]:
        raise IncompleteEvidence("source_coverage_mismatch")
    if len(queries) != EXPECTED_QUERIES or success != 1:
        raise IncompleteEvidence("incomplete_query_or_success_proof")

    start, fin, run, stop, uploaded = [moments[k] for k in MARKERS]
    if not (first <= start < fin < run < queries[0] < queries[-1] <= stop < uploaded <= last):
        raise IncompleteEvidence("invalid_event_order")
    if any(not (run < q <= stop) for q in queries):
        raise IncompleteEvidence("query_outside_phase")

    gaps = [seconds(a, b) for a, b in zip(queries, queries[1:])]
    gap = max(gaps)
    # The ordinals describe completion order only, NOT query identities,
    # isolated query cost, or a causal slow predicate.
    return {
        "classification": "OFFLINE_LOG_TIMING_ONLY_NOT_A_SECURITY_SCAN",
        "schema_version": 1,
        "cli_version": EXPECTED_CLI,
        "query_pack_version": EXPECTED_PACK,
        "queries_completed": EXPECTED_QUERIES,
        "rust_files_without_error": EXPECTED_EXTRACTED,
        "rust_files_with_errors": 0,
        "seconds": {
            "log_span": seconds(first, last),
            "extraction_group": seconds(start, fin),
            "finalize_group": seconds(fin, run),
            "query_group_until_evaluator_shutdown": seconds(run, stop),
            "shutdown_to_upload_confirmed": seconds(stop, uploaded),
            "max_gap_between_query_completions": gap,
        },
        "max_completion_gap_after_ordinal": gaps.index(gap) + 1,
        "warning": "Completion gaps are not per-query cost or proof of a bottleneck cause.",
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("job_log", type=Path, help="Locally saved successful Rust CodeQL job log")
    args = parser.parse_args()
    try:
        if args.job_log.stat().st_size > MAX_BYTES:
            raise IncompleteEvidence("invalid_input_size")
        result = parse_log(args.job_log.read_text(encoding="utf-8"))
    except (OSError, UnicodeError):
        print("INCOMPLETE_DO_NOT_USE: unreadable_input", file=sys.stderr)
        return 2
    except IncompleteEvidence as exc:
        print("INCOMPLETE_DO_NOT_USE: " + str(exc), file=sys.stderr)
        return 2
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
