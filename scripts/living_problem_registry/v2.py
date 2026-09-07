"""Living Problem Registry V2 causal projection and compatibility boundary."""

from __future__ import annotations

from collections import Counter
from enum import StrEnum
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


class CurrentWaitV2(StrEnum):
    INTEGRITY_BLOCK = "integrity_block"
    SOURCE_READ = "source_read"
    DISPOSITION = "disposition"
    REPLAY = "replay"
    IMPLEMENTATION = "implementation"
    DEPLOYMENT = "deployment"
    BEING_REVIEW = "being_review"
    AUTHORITY = "authority"
    CLOSURE = "closure"


WAIT_ORDER = (
    CurrentWaitV2.INTEGRITY_BLOCK,
    CurrentWaitV2.SOURCE_READ,
    CurrentWaitV2.DISPOSITION,
    CurrentWaitV2.REPLAY,
    CurrentWaitV2.IMPLEMENTATION,
    CurrentWaitV2.DEPLOYMENT,
    CurrentWaitV2.BEING_REVIEW,
    CurrentWaitV2.AUTHORITY,
    CurrentWaitV2.CLOSURE,
)
WAIT_RANK = {value.value: index for index, value in enumerate(WAIT_ORDER)}
V1_WAIT_TO_V2 = {
    "blocked_integrity": CurrentWaitV2.INTEGRITY_BLOCK,
    "source_read": CurrentWaitV2.SOURCE_READ,
    "claim_disposition": CurrentWaitV2.DISPOSITION,
    "evidence_or_replay": CurrentWaitV2.REPLAY,
    "implementation": CurrentWaitV2.IMPLEMENTATION,
    "deployment": CurrentWaitV2.DEPLOYMENT,
    "being_review": CurrentWaitV2.BEING_REVIEW,
    "operator_or_mutual_authority": CurrentWaitV2.AUTHORITY,
    "closed": CurrentWaitV2.CLOSURE,
}
MAX_WORK_REFS = 64
MAX_EVIDENCE_REFS = 96


def state_dir_v2(workspace: Path) -> Path:
    return workspace / "diagnostics/living_problem_registry_v2"


def _canonical_sha256(value: dict[str, Any]) -> str:
    encoded = json.dumps(value, sort_keys=True, separators=(",", ":")).encode()
    return hashlib.sha256(encoded).hexdigest()


def _sha256_file(path: Path) -> str | None:
    if not path.is_file():
        return None
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def _changed_rows(
    previous: Iterable[dict[str, Any]], current: Iterable[dict[str, Any]]
) -> list[dict[str, Any]]:
    before = {
        str(row.get("problem_id")): str(row.get("problem_sha256"))
        for row in previous
    }
    return sorted(
        (
            row
            for row in current
            if before.get(str(row.get("problem_id"))) != row.get("problem_sha256")
        ),
        key=lambda row: str(row.get("problem_id")),
    )


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


def _evidence_ref(value: Any) -> str | None:
    if isinstance(value, str):
        return value.strip() or None
    if not isinstance(value, dict):
        return None
    target = str(
        value.get("target")
        or value.get("path")
        or value.get("receipt_id")
        or value.get("event_id")
        or value.get("card_id")
        or ""
    ).strip()
    kind = str(value.get("kind") or value.get("schema") or "evidence").strip()
    return f"{kind}:{target}" if target else None


def _work_summary(
    claim_ids: list[str],
    work_by_claim: dict[str, list[tuple[str, dict[str, Any]]]],
) -> tuple[list[dict[str, Any]], list[str], dict[str, int]]:
    work_refs: list[dict[str, Any]] = []
    evidence_refs: set[str] = set()
    counters: Counter[str] = Counter()
    for claim_id in claim_ids:
        for work_id, item in work_by_claim.get(claim_id, []):
            status = str(
                item.get("status")
                or item.get("claim_classification")
                or "unknown"
            )
            counters[status] += 1
            closure_cards = item.get("closure_cards") or []
            post_change_responses = item.get("post_change_responses") or []
            boundary = item.get("authority_boundary_packet_v2") or {}
            scoped_approval = (
                boundary.get("scoped_approval")
                if isinstance(boundary, dict)
                else None
            )
            local_refs = {
                ref
                for raw in item.get("evidence_links") or []
                if (ref := _evidence_ref(raw))
            }
            for raw in closure_cards:
                if ref := _evidence_ref(raw):
                    local_refs.add(ref)
            for raw in post_change_responses:
                if ref := _evidence_ref(raw):
                    local_refs.add(ref)
            evidence_refs.update(local_refs)
            work_refs.append(
                {
                    "work_item_id": str(work_id),
                    "claim_id": claim_id,
                    "status": status,
                    "route": str(item.get("route") or ""),
                    "authority_wait": bool(item.get("authority_boundary_wait")),
                    "live_authority_granted": bool(
                        item.get("live_authority_granted")
                    ),
                    "scoped_approval_present": scoped_approval is not None,
                    "closure_card_count": len(closure_cards),
                    "post_change_response_status": str(
                        item.get("post_change_response_status") or "not_requested"
                    ),
                    "evidence_refs": sorted(local_refs)[:16],
                }
            )
    work_refs.sort(key=lambda row: (row["claim_id"], row["work_item_id"]))
    return (
        work_refs[:MAX_WORK_REFS],
        sorted(evidence_refs)[:MAX_EVIDENCE_REFS],
        dict(sorted(counters.items())),
    )


def _prerequisite_states(
    *,
    current_wait: CurrentWaitV2,
    closed: bool,
    evidence_refs: list[str],
) -> list[dict[str, Any]]:
    current_rank = WAIT_RANK[current_wait.value]
    states: list[dict[str, Any]] = []
    for wait in WAIT_ORDER:
        rank = WAIT_RANK[wait.value]
        if closed:
            state = "satisfied"
        elif wait == CurrentWaitV2.INTEGRITY_BLOCK:
            state = (
                "waiting"
                if current_wait == CurrentWaitV2.INTEGRITY_BLOCK
                else "satisfied"
            )
        elif current_wait == CurrentWaitV2.INTEGRITY_BLOCK:
            state = "blocked_by_integrity"
        elif rank < current_rank:
            state = "satisfied"
        elif rank == current_rank:
            state = "waiting"
        else:
            state = "blocked_by_prerequisite"
        states.append(
            {
                "prerequisite": wait.value,
                "state": state,
                "evidence_refs": evidence_refs if rank == current_rank else [],
            }
        )
    return states


def build_rows_v2(
    *,
    contracts: list[dict[str, Any]],
    rows_v1: list[dict[str, Any]],
    work_by_claim: dict[str, list[tuple[str, dict[str, Any]]]],
) -> list[dict[str, Any]]:
    contracts_by_id = {
        str(contract.get("contract_id")): contract for contract in contracts
    }
    rows: list[dict[str, Any]] = []
    for compat in rows_v1:
        problem_id = str(compat["problem_id"])
        contract = contracts_by_id[problem_id]
        claim_ids = [str(value) for value in compat.get("claim_ids") or []]
        work_refs, evidence_refs, work_status_counts = _work_summary(
            claim_ids, work_by_claim
        )
        wait = V1_WAIT_TO_V2[str(compat["current_wait"])]
        closed = bool(compat["felt_closed"]) and not bool(compat["reopened"])
        row = {
            "schema": "living_problem_registry_entry_v2",
            "schema_version": 2,
            "problem_id": problem_id,
            "contract_id": problem_id,
            "stable_problem_identity": "felt_contract_id",
            "anchor_claim_id": str(compat.get("anchor_claim_id") or ""),
            "claim_ids": claim_ids,
            "claim_count": len(claim_ids),
            "lifecycle_state": "closed" if closed else "open",
            "current_wait": wait.value,
            "current_wait_satisfied": closed,
            "first_unsatisfied_causal_prerequisite": (
                None if closed else wait.value
            ),
            "causal_prerequisites": _prerequisite_states(
                current_wait=wait,
                closed=closed,
                evidence_refs=evidence_refs,
            ),
            "source_status_counts": compat["source_status_counts"],
            "work_status_counts": work_status_counts,
            "trial_status_counts": compat["trial_status_counts"],
            "work_item_refs": work_refs,
            "work_item_ref_count": len(compat["work_item_ids"]),
            "work_item_refs_truncated": len(compat["work_item_ids"]) > MAX_WORK_REFS,
            "trial_ids": compat["trial_ids"],
            "evidence_refs": evidence_refs,
            "evidence_refs_truncated": len(evidence_refs) >= MAX_EVIDENCE_REFS,
            "technical_state_counts": compat["technical_state_counts"],
            "evidence_state_counts": compat["evidence_state_counts"],
            "being_review": {
                "state": str(contract.get("felt_review") or "not_requested"),
                "right_to_ignore": True,
                "silence_is_neutral": True,
                "felt_closed": closed,
                "reopen_count": int(contract.get("reopen_count") or 0),
            },
            "closure_contract": compat["closure_contract"],
            "authority": {
                "registry_grants_authority": False,
                "registry_schedules_work": False,
                "selection_is_advisory_only": True,
                "selected_in_steward_projection": bool(
                    compat["selected_for_steward_work"]
                ),
            },
            "compatibility_view": {
                "schema": "living_problem_registry_entry_v1",
                "current_wait": compat["current_wait"],
                "problem_sha256": compat["problem_sha256"],
            },
            "last_change_at": compat["last_change_at"],
            "artifact_authority_state_v1": authority_state(),
        }
        row["problem_sha256"] = _canonical_sha256(row)
        rows.append(row)
    return sorted(rows, key=lambda row: str(row["problem_id"]))


def _queue(rows: list[dict[str, Any]]) -> str:
    active = [row for row in rows if row["lifecycle_state"] != "closed"]
    active.sort(
        key=lambda row: (
            WAIT_RANK[str(row["current_wait"])],
            str(row["last_change_at"]),
            str(row["problem_id"]),
        )
    )
    lines = [
        "# Living Problem Registry V2 Steward Queue",
        "",
        "Advisory projection only. Entries are not scheduled work and cannot grant "
        "authority, set owner priority, infer consent, or close a Felt Contract.",
        "",
    ]
    lines.extend(
        f"- `{row['problem_id']}` - `{row['current_wait']}`"
        for row in active
    )
    return "\n".join(lines) + "\n"


def project_v2(
    workspace: Path,
    *,
    write: bool,
    contracts: list[dict[str, Any]],
    rows_v1: list[dict[str, Any]],
    work_by_claim: dict[str, list[tuple[str, dict[str, Any]]]],
    input_paths: Iterable[Path],
) -> dict[str, Any]:
    rows = build_rows_v2(
        contracts=contracts,
        rows_v1=rows_v1,
        work_by_claim=work_by_claim,
    )
    output = state_dir_v2(workspace)
    previous = _jsonl(output / "problems.jsonl")
    changed = _changed_rows(previous, rows)
    problem_ids = [str(row["problem_id"]) for row in rows]
    invalid_closed = [
        row["problem_id"]
        for row in rows
        if row["lifecycle_state"] == "closed"
        and not row["being_review"]["felt_closed"]
    ]
    wait_counts = Counter(str(row["current_wait"]) for row in rows)
    status = {
        "schema": "living_problem_registry_status_v2",
        "schema_version": 2,
        "valid": (
            not invalid_closed
            and len(rows) == len(contracts)
            and len(problem_ids) == len(set(problem_ids))
        ),
        "write": write,
        "contract_count": len(contracts),
        "problem_count": len(rows),
        "active_problem_count": sum(
            row["lifecycle_state"] != "closed" for row in rows
        ),
        "closed_problem_count": sum(
            row["lifecycle_state"] == "closed" for row in rows
        ),
        "changed_problem_count": len(changed),
        "wait_counts": dict(sorted(wait_counts.items())),
        "source_hashes": {
            str(path.relative_to(workspace)): digest
            for path in input_paths
            if (digest := _sha256_file(path)) is not None
        },
        "counter_audit": {
            "status": "consistent" if not invalid_closed else "inconsistent",
            "checks": {
                "one_problem_per_current_felt_contract": len(rows)
                == len(contracts),
                "problem_ids_unique": len(problem_ids) == len(set(problem_ids)),
                "only_explicit_contract_local_felt_confirmation_closes": (
                    not invalid_closed
                ),
                "silence_remains_neutral": True,
                "registry_suggestions_do_not_schedule_work": True,
                "registry_grants_no_authority": True,
                "related_contracts_do_not_propagate_closure": True,
            },
        },
        "compatibility_view": "diagnostics/living_problem_registry_v1",
        "runtime_relation": (
            "derived_evidence_only_not_consumed_as_owner_priority_scheduler_or_control"
        ),
        "artifact_authority_state_v1": authority_state(),
    }
    if write and status["valid"]:
        owner_atomic_write_jsonl(output / "problems.jsonl", rows)
        owner_atomic_write_jsonl(output / "changed_problems.jsonl", changed)
        owner_atomic_write_json(output / "status.json", status)
        owner_atomic_write(output / "queue.md", _queue(rows))
        owner_atomic_write(
            output / "report.md",
            "# Living Problem Registry V2\n\n"
            "One derived row per current Felt Contract. The current wait is the "
            "first unsatisfied causal prerequisite. Registry state is evidence, "
            "not owner priority, scheduling, authority, assent, or closure.\n\n"
            f"- contracts/problems: {len(rows)}\n"
            f"- active: {status['active_problem_count']}\n"
            f"- explicitly felt-closed: {status['closed_problem_count']}\n"
            f"- changed packets: {len(changed)}\n\n"
            "## Current Waits\n\n"
            + "\n".join(
                f"- {wait}: {count}"
                for wait, count in sorted(
                    wait_counts.items(), key=lambda item: WAIT_RANK[item[0]]
                )
            )
            + "\n",
        )
    return status
