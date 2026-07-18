#!/usr/bin/env python3
"""Warning-only facade for the retired session-hook installer."""

from __future__ import annotations

import argparse
import json

try:
    from steward_control.legacy import retired_entrypoint
except ModuleNotFoundError:
    from scripts.steward_control.legacy import retired_entrypoint


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        record = retired_entrypoint("install_steward_hooks.py")
        return 0 if record["retired"] and not record["mutated"] else 1
    print(
        json.dumps(
            retired_entrypoint("install_steward_hooks.py"),
            indent=2,
            sort_keys=True,
        )
    )
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
