"""Portable paths and fork-specific study manifest loading."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from .model import _record_hash

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_WORKSPACE = ROOT / "capsules/spectral-bridge/workspace"
DEFAULT_MANIFEST = (
    Path(__file__).resolve().parent / "manifests/astrid_baseline_campaigns_v1.json"
)


def state_dir(workspace: Path) -> Path:
    return workspace / "diagnostics/evidence_study_runtime_v1"


def load_manifest(path: Path = DEFAULT_MANIFEST) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict) or value.get("schema") != "study_seed_manifest_v1":
        raise ValueError("study seed manifest must use study_seed_manifest_v1")
    campaigns = value.get("campaigns")
    if not isinstance(campaigns, list) or not campaigns:
        raise ValueError("study seed manifest requires campaigns")
    value["manifest_sha256"] = _record_hash(
        {key: item for key, item in value.items() if key != "manifest_sha256"}
    )
    return value
