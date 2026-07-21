"""Deployment evidence matching for lived-state witness reconciliation."""

from __future__ import annotations

from bisect import bisect_right
from datetime import datetime
import json
from pathlib import Path
from typing import Any


def environment_receipts_path(workspace: Path) -> Path:
    return workspace / "environment_receipts/environment_receipts.jsonl"


def successful_deployments(workspace: Path) -> list[dict[str, Any]]:
    path = environment_receipts_path(workspace)
    receipts: list[dict[str, Any]] = []
    if not path.is_file():
        return receipts
    for raw in path.read_text(encoding="utf-8", errors="replace").splitlines():
        if not raw.strip():
            continue
        try:
            value = json.loads(raw)
        except json.JSONDecodeError:
            continue
        if not isinstance(value, dict):
            continue
        deployment = value.get("deployment")
        compatibility = value.get("compatibility_status")
        if (
            not isinstance(deployment, dict)
            or deployment.get("status") != "passed"
            or not isinstance(compatibility, dict)
            or compatibility.get("compatible") is not True
            or not isinstance(value.get("t_ms"), int)
        ):
            continue
        receipts.append(value)
    receipts.sort(key=lambda receipt: (int(receipt["t_ms"]), str(receipt.get("id") or "")))
    return receipts


def deployment_ref(receipt: dict[str, Any]) -> dict[str, Any]:
    repositories = receipt.get("repositories")
    repositories = repositories if isinstance(repositories, dict) else {}
    astrid = repositories.get("astrid")
    astrid = astrid if isinstance(astrid, dict) else {}
    artifacts = receipt.get("artifacts")
    artifacts = artifacts if isinstance(artifacts, dict) else {}
    binaries = artifacts.get("binaries")
    binaries = binaries if isinstance(binaries, dict) else {}
    bridge_binary = binaries.get("spectral-bridge")
    bridge_binary = bridge_binary if isinstance(bridge_binary, dict) else {}
    processes = receipt.get("processes")
    processes = processes if isinstance(processes, dict) else {}
    process = processes.get("new")
    process = process if isinstance(process, dict) else {}
    authority = receipt.get("artifact_authority_state_v1")
    authority = authority if isinstance(authority, dict) else {}
    exact_receipt_valid = bool(
        receipt.get("schema") == "stack_environment_receipt_v2"
        and receipt.get("schema_version") == 2
        and receipt.get("component") == "spectral-bridge"
        and authority.get("state") == "evidence_only"
        and authority.get("witness_only") is True
        and all(
            receipt.get(marker) is False
            for marker in (
                "live_eligible_now",
                "auto_approved",
                "grants_approval",
                "edits_source_now",
            )
        )
        and bridge_binary.get("exists") is True
        and process.get("running") is True
    )
    process_started_at_unix_ms: int | None = None
    process_started_at = process.get("started_at")
    if isinstance(process_started_at, str):
        try:
            process_started_at_unix_ms = int(
                datetime.strptime(process_started_at, "%a %b %d %H:%M:%S %Y")
                .astimezone()
                .timestamp()
                * 1_000
            )
        except ValueError:
            process_started_at_unix_ms = None
    return {
        "deployment_receipt_id": str(receipt.get("id") or ""),
        "t_ms": int(receipt.get("t_ms") or 0),
        "source_identity_sha256": astrid.get("source_identity_sha256"),
        "artifact_sha256": bridge_binary.get("sha256"),
        "pid": process.get("pid"),
        "process_started_at_unix_ms": process_started_at_unix_ms,
        "exact_identity_available": exact_receipt_valid
        and all(
            value is not None
            for value in (
                astrid.get("source_identity_sha256"),
                bridge_binary.get("sha256"),
                process.get("pid"),
                process_started_at_unix_ms,
            )
        ),
    }


def deployment_before(
    deployments: list[dict[str, Any]], authored_at_ms: int
) -> dict[str, Any] | None:
    positions = [int(receipt["t_ms"]) for receipt in deployments]
    index = bisect_right(positions, authored_at_ms) - 1
    return deployments[index] if index >= 0 else None


def exact_deployment_match(
    witness: dict[str, Any], receipt: dict[str, Any] | None
) -> bool:
    if receipt is None:
        return False
    deployment = deployment_ref(receipt)
    candidate = witness.get("startup_build_candidate_v1")
    process = witness.get("observed_process_v1")
    if not isinstance(candidate, dict) or not isinstance(process, dict):
        return False
    return bool(
        deployment["exact_identity_available"]
        and candidate.get("source_identity_sha256")
        == deployment["source_identity_sha256"]
        and candidate.get("artifact_sha256") == deployment["artifact_sha256"]
        and process.get("pid") == deployment["pid"]
        and abs(
            int(process.get("process_started_at_unix_ms") or 0)
            - int(deployment["process_started_at_unix_ms"] or 0)
        )
        <= 2_000
    )


def exact_deployment_receipt(
    witness: dict[str, Any], deployments: list[dict[str, Any]]
) -> dict[str, Any] | None:
    matches = [
        receipt for receipt in deployments if exact_deployment_match(witness, receipt)
    ]
    if not matches:
        return None
    return min(
        matches,
        key=lambda receipt: (int(receipt["t_ms"]), str(receipt.get("id") or "")),
    )


def alignment(
    witness: dict[str, Any] | None,
    authored_at_ms: int,
    deployments: list[dict[str, Any]],
    *,
    historical: bool,
) -> dict[str, Any]:
    exact_receipt = (
        exact_deployment_receipt(witness, deployments) if witness is not None else None
    )
    receipt = exact_receipt or deployment_before(deployments, authored_at_ms)
    if exact_receipt is not None:
        outcome = "same_deployment"
        exact = True
    elif historical and receipt is not None:
        outcome = "temporal_association_only"
        exact = False
    elif historical:
        outcome = "historical_unrecoverable"
        exact = False
    else:
        outcome = "deployment_unknown"
        exact = False
    return {
        "outcome": outcome,
        "deployment_receipt_id": str(receipt.get("id") or "") if receipt else None,
        "exact_identity_match": exact,
        "temporal_association_only": outcome == "temporal_association_only",
        "direct_causation_claimed": False,
        "failed_deployment_can_establish_alignment": False,
        "mutable_build_manifest_can_establish_alignment": False,
    }
