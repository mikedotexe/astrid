#!/usr/bin/env python3
"""Verify the exact loaded-capsule contract for a CPU-edge appliance."""

from __future__ import annotations

import argparse
import json
import sys
from typing import Any


def loaded_capsules(value: Any) -> list[str]:
    if not isinstance(value, dict):
        raise ValueError("status response must be a JSON object")
    status = value.get("status")
    if not isinstance(status, dict):
        raise ValueError("status response is missing the status object")
    loaded = status.get("loaded_capsules")
    if not isinstance(loaded, list) or not all(isinstance(item, str) for item in loaded):
        raise ValueError("status.loaded_capsules must be a string array")
    if len(set(loaded)) != len(loaded):
        raise ValueError("status.loaded_capsules contains duplicate names")
    return loaded


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--expected-total", type=int, default=20)
    parser.add_argument("--required", action="append", default=[])
    args = parser.parse_args()
    if args.expected_total < 1:
        parser.error("--expected-total must be positive")

    try:
        value = json.load(sys.stdin)
        loaded = loaded_capsules(value)
    except (OSError, json.JSONDecodeError, ValueError) as error:
        print(f"error: invalid Astrid JSON status: {error}", file=sys.stderr)
        return 1

    missing = sorted(set(args.required).difference(loaded))
    print(f"loaded_capsule_count={len(loaded)}")
    print("loaded_capsules=" + ",".join(sorted(loaded)))
    if missing:
        print("error: required capsules are absent: " + ",".join(missing), file=sys.stderr)
        return 1
    if len(loaded) != args.expected_total:
        print(
            f"error: expected exactly {args.expected_total} loaded capsules, found {len(loaded)}",
            file=sys.stderr,
        )
        return 1
    print("loaded_capsule_contract=verified")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
