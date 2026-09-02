#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path


class ReleaseRulesetError(RuntimeError):
    pass


def validate_ruleset(ruleset: dict) -> None:
    if ruleset.get("name") != "Protect release tags":
        raise ReleaseRulesetError("release ruleset name must be Protect release tags")
    if ruleset.get("target") != "tag":
        raise ReleaseRulesetError("release ruleset must target tags")
    if ruleset.get("enforcement") != "active":
        raise ReleaseRulesetError("release ruleset must be active")
    refs = ruleset.get("conditions", {}).get("ref_name", {})
    if refs.get("include") != ["refs/tags/v*"] or refs.get("exclude") != []:
        raise ReleaseRulesetError("release ruleset must target exactly refs/tags/v*")
    rule_types = [rule.get("type") for rule in ruleset.get("rules", [])]
    if rule_types != ["deletion", "update"]:
        raise ReleaseRulesetError("release ruleset must contain exactly deletion and update restrictions")
    if ruleset.get("bypass_actors") != []:
        raise ReleaseRulesetError("release ruleset must not have bypass actors")
    if ruleset.get("current_user_can_bypass") != "never":
        raise ReleaseRulesetError("release ruleset must not be bypassable by the current actor")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("ruleset", type=Path)
    args = parser.parse_args()
    try:
        validate_ruleset(json.loads(args.ruleset.read_text(encoding="utf-8")))
        print("release tag ruleset verified")
        return 0
    except (OSError, ValueError, ReleaseRulesetError) as exc:
        print(f"release-ruleset error: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
