"""Source-first projection of one current problem row per Felt Contract."""

from __future__ import annotations

from collections import Counter, defaultdict
import hashlib
import json
from pathlib import Path
from typing import Any, Iterable

try:
    from experiential_systems.common import (
        authority_state,
        owner_atomic_write,
        owner_atomic_write_json,
        owner_atomic_write_jsonl,
    )
except ModuleNotFoundError:
    from scripts.experiential_systems.common import (
        authority_state,
        owner_atomic_write,
        owner_atomic_write_json,
        owner_atomic_write_jsonl,
    )

from .model import CurrentWaitV1, WORK_STATUS_WAIT, earliest_wait


def state_dir(workspace: Path) -> Path:
    return workspace / "diagnostics/living_problem_registry_v1"


def _json(path: Path) -> dict[str, Any]:
    if not path.is_file():
        return {}
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError(f"{path} must contain a JSON object")
    return value


def _jsonl(path: Path) -> list[dict[str, Any]]:
    if not path.is_file():
        return []
    rows: list[dict[str, Any]] = []
    for line_number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if not line.strip():
            continue
        value = json.loads(line)
        if not isinstance(value, dict):
            raise ValueError(f"{path}:{line_number} must contain a JSON object")
        rows.append(value)
    return rows


def _sha256_file(path: Path) -> str | None:
    if not path.is_file():
        return None
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def _canonical_sha256(value: dict[str, Any]) -> str:
    encoded = json.dumps(value, sort_keys=True, separators=(",", ":")).encode()
    return hashlib.sha256(encoded).hexdigest()


def _claim_id(work_item: dict[str, Any]) -> str:
    source = str(work_item.get("source_introspection_id") or "")
    claim = str(work_item.get("claim_id") or "")
    return f"{source}:{claim}" if source and claim else ""


def _source_artifact(
    artifacts: dict[str, Any], canonical_claim_id: str
) -> dict[str, Any]:
    introspection_id = canonical_claim_id.rsplit(":", 1)[0]
    direct = artifacts.get(introspection_id)
    if isinstance(direct, dict):
        return direct
    for artifact in artifacts.values():
        if (
            isinstance(artifact, dict)
            and artifact.get("introspection_id") == introspection_id
        ):
            return artifact
    return {}


def _work_wait(work_item: dict[str, Any]) -> CurrentWaitV1:
    status = str(
        work_item.get("status")
        or work_item.get("claim_classification")
        or ""
    )
    return WORK_STATUS_WAIT.get(status, CurrentWaitV1.CLAIM_DISPOSITION)


def _technical_wait(contract: dict[str, Any]) -> CurrentWaitV1 | None:
    counts = contract.get("technical_state_counts")
    if not isinstance(counts, dict):
        return None
    implemented = int(counts.get("implemented") or 0)
    deployed = int(counts.get("deployed") or 0)
    verified = int(counts.get("verified") or 0)
    if implemented and not deployed and not verified:
        return CurrentWaitV1.DEPLOYMENT
    if deployed or verified:
        return CurrentWaitV1.BEING_REVIEW
    return None


def _problem_row(
    contract: dict[str, Any],
    *,
    artifacts: dict[str, Any],
    work_by_claim: dict[str, list[tuple[str, dict[str, Any]]]],
    trials_by_work: dict[str, list[tuple[str, dict[str, Any]]]],
    selected_ids: set[str],
) -> dict[str, Any]:
    contract_id = str(contract.get("contract_id") or "")
    claim_ids = sorted(str(value) for value in contract.get("claim_ids") or [])
    waits: set[CurrentWaitV1] = set()
    source_status_counts: Counter[str] = Counter()
    work_status_counts: Counter[str] = Counter()
    trial_status_counts: Counter[str] = Counter()
    work_item_ids: set[str] = set()
    trial_ids: set[str] = set()

    for claim_id in claim_ids:
        source = _source_artifact(artifacts, claim_id)
        source_status = str(source.get("status") or "missing")
        source_status_counts[source_status] += 1
        if source_status == "blocked_needs_steward":
            waits.add(CurrentWaitV1.BLOCKED_INTEGRITY)
        elif not source or not bool(source.get("full_read")):
            waits.add(CurrentWaitV1.SOURCE_READ)

        work_items = work_by_claim.get(claim_id, [])
        if not work_items:
            waits.add(CurrentWaitV1.CLAIM_DISPOSITION)
        for work_id, work_item in work_items:
            work_item_ids.add(work_id)
            status = str(
                work_item.get("status")
                or work_item.get("claim_classification")
                or "unknown"
            )
            work_status_counts[status] += 1
            waits.add(_work_wait(work_item))
            for trial_id, trial in trials_by_work.get(work_id, []):
                trial_ids.add(trial_id)
                trial_status = str(trial.get("status") or "unknown")
                trial_status_counts[trial_status] += 1
                if not trial.get("results") and trial_status not in {
                    "result_recorded",
                    "closed",
                }:
                    waits.add(CurrentWaitV1.EVIDENCE_OR_REPLAY)

    technical_wait = _technical_wait(contract)
    if technical_wait is not None:
        waits.add(technical_wait)

    felt_closed = bool(contract.get("felt_closed"))
    reopened = (
        str(contract.get("activity") or "") == "reopened"
        or str(contract.get("felt_review") or "")
        in {"still_friction", "contradicted", "objection"}
    )
    if felt_closed and not reopened:
        current_wait = CurrentWaitV1.CLOSED
    else:
        waits.discard(CurrentWaitV1.CLOSED)
        current_wait = earliest_wait(waits)

    row = {
        "schema": "living_problem_registry_entry_v1",
        "schema_version": 1,
        "problem_id": contract_id,
        "contract_id": contract_id,
        "anchor_claim_id": str(contract.get("anchor_claim_id") or ""),
        "claim_ids": claim_ids,
        "claim_count": len(claim_ids),
        "activity": str(contract.get("activity") or "unknown"),
        "felt_review": str(contract.get("felt_review") or "not_requested"),
        "felt_closed": felt_closed and not reopened,
        "administrative_terminal": bool(contract.get("administrative_terminal")),
        "reopened": reopened,
        "current_wait": current_wait.value,
        "source_status_counts": dict(sorted(source_status_counts.items())),
        "work_status_counts": dict(sorted(work_status_counts.items())),
        "trial_status_counts": dict(sorted(trial_status_counts.items())),
        "work_item_ids": sorted(work_item_ids),
        "trial_ids": sorted(trial_ids),
        "selected_for_steward_work": contract_id in selected_ids,
        "technical_state_counts": dict(
            sorted((contract.get("technical_state_counts") or {}).items())
        ),
        "evidence_state_counts": dict(
            sorted((contract.get("evidence_state_counts") or {}).items())
        ),
        "last_change_at": str(contract.get("last_change_at") or ""),
        "closure_contract": {
            "silence_closes": False,
            "administrative_terminal_closes": False,
            "family_membership_propagates_closure": False,
            "felt_confirmed_required": True,
        },
        "registry_relation": (
            "derived_steward_problem_projection_not_being_state_or_runtime_input"
        ),
        "artifact_authority_state_v1": authority_state(),
    }
    row["problem_sha256"] = _canonical_sha256(row)
    return row


def _changed_rows(
    previous: Iterable[dict[str, Any]], current: Iterable[dict[str, Any]]
) -> list[dict[str, Any]]:
    before = {
        str(row.get("problem_id")): str(row.get("problem_sha256"))
        for row in previous
    }
    changed = [
        row
        for row in current
        if before.get(str(row.get("problem_id"))) != row.get("problem_sha256")
    ]
    return sorted(changed, key=lambda row: str(row.get("problem_id")))


def _queue(rows: list[dict[str, Any]]) -> str:
    active = [row for row in rows if row["current_wait"] != CurrentWaitV1.CLOSED]
    active.sort(
        key=lambda row: (
            not bool(row["selected_for_steward_work"]),
            str(row["current_wait"]),
            str(row["last_change_at"]),
            str(row["problem_id"]),
        )
    )
    lines = [
        "# Living Problem Registry Queue",
        "",
        "Derived steward work order only. It is not a being-state, attention, "
        "consent, or runtime-control surface.",
        "",
    ]
    lines.extend(
        f"- `{row['problem_id']}` — `{row['current_wait']}`"
        + (" — selected" if row["selected_for_steward_work"] else "")
        for row in active
    )
    return "\n".join(lines) + "\n"


def project(workspace: Path, *, write: bool) -> dict[str, Any]:
    diagnostics = workspace / "diagnostics"
    contracts_path = diagnostics / "felt_contract_graph_v1/contracts.jsonl"
    addressing_path = diagnostics / "introspection_addressing_v1/status.json"
    sandbox_path = diagnostics / "sandbox_trial_queue_v1/status.json"
    selection_path = diagnostics / "steward_work_selection_v1/selection.json"
    dossier_path = diagnostics / "experiment_dossiers_v1/status.json"
    authority_path = diagnostics / "authority_temporal_v1/status.json"
    required = (contracts_path, addressing_path, sandbox_path, selection_path)
    missing = [str(path.relative_to(workspace)) for path in required if not path.is_file()]
    if missing:
        return {
            "schema": "living_problem_registry_status_v1",
            "schema_version": 1,
            "valid": False,
            "missing_inputs": missing,
            "artifact_authority_state_v1": authority_state(),
        }

    contracts = _jsonl(contracts_path)
    addressing = _json(addressing_path)
    sandbox = _json(sandbox_path)
    selection = _json(selection_path)
    work_items = addressing.get("work_items") or {}
    artifacts = addressing.get("artifacts") or {}
    trials = sandbox.get("trials") or {}
    work_by_claim: dict[str, list[tuple[str, dict[str, Any]]]] = defaultdict(list)
    trials_by_work: dict[str, list[tuple[str, dict[str, Any]]]] = defaultdict(list)
    for work_id, work_item in work_items.items():
        if isinstance(work_item, dict) and (claim_id := _claim_id(work_item)):
            work_by_claim[claim_id].append((str(work_id), work_item))
    for trial_id, trial in trials.items():
        if not isinstance(trial, dict):
            continue
        work_id = str(trial.get("source_work_item_id") or "")
        if work_id:
            trials_by_work[work_id].append((str(trial_id), trial))
    selected_ids = {
        str(item.get("contract_id"))
        for item in selection.get("selected_entries") or []
        if isinstance(item, dict)
    }
    rows = [
        _problem_row(
            contract,
            artifacts=artifacts,
            work_by_claim=work_by_claim,
            trials_by_work=trials_by_work,
            selected_ids=selected_ids,
        )
        for contract in contracts
    ]
    rows.sort(key=lambda row: str(row["problem_id"]))
    output = state_dir(workspace)
    previous = _jsonl(output / "problems.jsonl")
    changed = _changed_rows(previous, rows)
    wait_counts = Counter(str(row["current_wait"]) for row in rows)
    problem_ids = [str(row["problem_id"]) for row in rows]
    closed_ids = {
        str(row["problem_id"])
        for row in rows
        if row["current_wait"] == CurrentWaitV1.CLOSED
    }
    invalid_closed = [
        row["problem_id"]
        for row in rows
        if row["current_wait"] == CurrentWaitV1.CLOSED
        and (not row["felt_closed"] or row["reopened"])
    ]
    status = {
        "schema": "living_problem_registry_status_v1",
        "schema_version": 1,
        "valid": not invalid_closed and len(problem_ids) == len(set(problem_ids)),
        "write": write,
        "contract_count": len(contracts),
        "problem_count": len(rows),
        "active_problem_count": len(rows) - len(closed_ids),
        "closed_problem_count": len(closed_ids),
        "changed_problem_count": len(changed),
        "wait_counts": dict(sorted(wait_counts.items())),
        "selected_problem_count": sum(
            bool(row["selected_for_steward_work"]) for row in rows
        ),
        "source_hashes": {
            str(path.relative_to(workspace)): digest
            for path in (
                contracts_path,
                addressing_path,
                sandbox_path,
                selection_path,
                dossier_path,
                authority_path,
            )
            if (digest := _sha256_file(path)) is not None
        },
        "counter_audit": {
            "status": "consistent" if not invalid_closed else "inconsistent",
            "checks": {
                "one_problem_per_contract": len(rows) == len(contracts),
                "problem_ids_unique": len(problem_ids) == len(set(problem_ids)),
                "only_explicit_felt_closure_closes": not invalid_closed,
                "selection_is_subset": selected_ids.issubset(set(problem_ids)),
                "registry_grants_no_authority": True,
            },
        },
        "closure_relation": (
            "only_contract_local_explicit_felt_confirmed_closes; "
            "silence_and_family_membership_are_neutral"
        ),
        "runtime_relation": (
            "not_consumed_by_bridge_minime_model_scheduler_or_control_runtime"
        ),
        "artifact_authority_state_v1": authority_state(),
    }
    if write and status["valid"]:
        owner_atomic_write_jsonl(output / "problems.jsonl", rows)
        owner_atomic_write_jsonl(output / "changed_problems.jsonl", changed)
        owner_atomic_write_json(output / "status.json", status)
        queue = _queue(rows)
        owner_atomic_write(output / "queue.md", queue)
        owner_atomic_write(
            output / "report.md",
            "# Living Problem Registry\n\n"
            "This is a derived source-first steward projection. It does not model "
            "a being's attention, infer consent from silence, grant runtime authority, "
            "or propagate closure through claim-family similarity.\n\n"
            f"- contracts/problems: {len(rows)}\n"
            f"- active: {status['active_problem_count']}\n"
            f"- felt-closed: {status['closed_problem_count']}\n"
            f"- changed this projection: {len(changed)}\n\n"
            "## Current Waits\n\n"
            + "\n".join(
                f"- {wait}: {count}"
                for wait, count in sorted(wait_counts.items())
            )
            + "\n",
        )
    return status
