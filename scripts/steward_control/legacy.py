"""Warning records for retired steward entry points."""

from __future__ import annotations

from typing import Any


def retired_entrypoint(name: str) -> dict[str, Any]:
    return {
        "schema": "steward_control_legacy_entrypoint_v1",
        "schema_version": 1,
        "entrypoint": name,
        "retired": True,
        "mutated": False,
        "replacement": "scripts/steward_control.py",
    }
