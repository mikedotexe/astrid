"""Deployment-receipt lookup for felt-review family budgets."""

from __future__ import annotations

import json
from pathlib import Path


def latest_successful_deployment(workspace: Path) -> str | None:
    path = workspace / "environment_receipts/latest_environment_receipt.json"
    try:
        receipt = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None
    deployment = receipt.get("deployment")
    if not isinstance(deployment, dict) or deployment.get("status") != "passed":
        return None
    return str(receipt.get("id") or "") or None
