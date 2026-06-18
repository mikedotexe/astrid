#!/usr/bin/env python3
"""Steward-only consequence memory workbench.

Read-only by default. The workbench correlates existing consequence evidence
across relation gifts and Minime authority ledgers, then highlights loops the
steward should close. It does not write being memory, issue invitations, edit
env vars, restart services, or change live behavior. The only write path is an
explicit --out.
"""
from __future__ import annotations

import argparse
import datetime as dt
import io
import json
import re
import sys
import tempfile
import time
import unittest
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any, TextIO

ASTRID_ROOT = Path("/Users/v/other/astrid")
MINIME_ROOT = Path("/Users/v/other/minime")
SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

RUNTIME_CHANGE_NONE = "none"
STEWARD_CLOSURE_SCHEMA = "steward_consequence_closure_v1"
STEWARD_CLOSURE_PATHS = [
    ASTRID_ROOT / "workspace" / "steward_consequence_closures.jsonl",
]
OPEN_CLOSURE_STATES = {
    "active_pending",
    "active_stale",
    "terminal_without_response",
    "held_or_deferred",
    "insufficient_evidence",
}
RELATION_RESPONSE_CLOSURE_STATES = {
    "active_pending",
    "active_stale",
    "terminal_without_response",
    "insufficient_evidence",
}
AUTHORITY_ROW_SCHEMAS = {
    "authority_gate_v1",
    "authority_budget_v1",
    "research_budget_v1",
    "sovereign_loop_v1",
    "authority_consequence_v1",
    "mode_release_consequence_v1",
}
ID_KEYS = {
    "record_id",
    "request_id",
    "budget_id",
    "loop_id",
    "memory_id",
    "outcome_ref",
    "source_review_record_id",
}
PRESSURE_TARGET = "steward"
AUTHORITY_STALE_AFTER_S = 24 * 60 * 60
AUTHREQ_TS_RE = re.compile(r"\bauthreq_[^_]+_(\d+)_")


def _now_iso() -> str:
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())


def _parse_time_s(value: Any) -> float | None:
    if isinstance(value, (int, float)):
        raw = float(value)
        return raw / 1000.0 if raw > 10_000_000_000 else raw
    text = str(value or "").strip()
    if not text:
        return None
    if text.endswith("Z"):
        text = f"{text[:-1]}+00:00"
    try:
        parsed = dt.datetime.fromisoformat(text)
    except ValueError:
        return None
    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=dt.timezone.utc)
    return parsed.timestamp()


def _latest_row_time_s(rows: list[dict[str, Any]]) -> float | None:
    times = [
        value
        for row in rows
        for value in (
            _parse_time_s(row.get("updated_at")),
            _parse_time_s(row.get("created_at")),
        )
        if value is not None
    ]
    return max(times) if times else None


def _read_jsonl_objects(path: Path) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    try:
        for line_no, line in enumerate(path.read_text().splitlines(), start=1):
            if not line.strip():
                continue
            row = json.loads(line)
            if isinstance(row, dict):
                row["_source_path"] = str(path)
                row["_line_number"] = line_no
                rows.append(row)
    except Exception:
        return rows
    return rows


def _thread_dirs(minime_root: Path) -> list[Path]:
    threads_root = minime_root / "workspace" / "action_threads" / "threads"
    if not threads_root.is_dir():
        return []
    return sorted(path for path in threads_root.iterdir() if path.is_dir())


def load_authority_rows(minime_root: Path = MINIME_ROOT) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for thread_dir in _thread_dirs(minime_root):
        for row in _read_jsonl_objects(thread_dir / "authority_gate.jsonl"):
            if row.get("record_schema") in AUTHORITY_ROW_SCHEMAS:
                row.setdefault("thread_id", thread_dir.name)
                rows.append(row)
    return rows


def load_being_memory_rows(minime_root: Path = MINIME_ROOT) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for thread_dir in _thread_dirs(minime_root):
        for row in _read_jsonl_objects(thread_dir / "being_memory.jsonl"):
            if row.get("record_schema") == "being_memory_v1":
                row.setdefault("thread_id", thread_dir.name)
                rows.append(row)
    return rows


def load_steward_closure_rows(
    paths: list[Path] | None = None,
) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for path in paths or STEWARD_CLOSURE_PATHS:
        for row in _read_jsonl_objects(path):
            if row.get("record_schema") == STEWARD_CLOSURE_SCHEMA:
                rows.append(row)
    rows.sort(key=lambda row: str(row.get("reviewed_at") or row.get("created_at") or ""))
    return rows


def _collect_reference_ids(value: Any) -> set[str]:
    refs: set[str] = set()
    if isinstance(value, dict):
        for key, item in value.items():
            if key in ID_KEYS and isinstance(item, (str, int, float)):
                text = str(item).strip()
                if text:
                    refs.add(text)
            refs.update(_collect_reference_ids(item))
    elif isinstance(value, list):
        for item in value:
            refs.update(_collect_reference_ids(item))
    elif isinstance(value, (str, int, float)):
        text = str(value).strip()
        if text and (
            text.startswith(("auth", "resbud", "loop", "mem", "sovereign"))
        ):
            refs.add(text)
    return refs


def memory_reference_index(
    memory_rows: list[dict[str, Any]],
) -> dict[str, list[dict[str, Any]]]:
    index: dict[str, list[dict[str, Any]]] = {}
    for row in memory_rows:
        refs = _collect_reference_ids(row)
        for ref in refs:
            index.setdefault(ref, []).append(row)
    return index


def _memory_matches_for_rows(
    rows: list[dict[str, Any]],
    memory_index: dict[str, list[dict[str, Any]]],
) -> list[dict[str, Any]]:
    matches: dict[str, dict[str, Any]] = {}
    for row in rows:
        for ref in _collect_reference_ids(row):
            for memory in memory_index.get(ref, []):
                memory_id = str(memory.get("memory_id") or "")
                if memory_id:
                    matches[memory_id] = memory
    return list(matches.values())


def _compact_row(row: dict[str, Any]) -> dict[str, Any]:
    return {
        "record_schema": row.get("record_schema"),
        "record_type": row.get("record_type"),
        "record_id": row.get("record_id"),
        "request_id": row.get("request_id"),
        "budget_id": row.get("budget_id"),
        "loop_id": row.get("loop_id"),
        "status": row.get("status"),
        "scope": row.get("scope"),
        "outcome": row.get("outcome"),
        "reason": row.get("reason"),
        "suggested_next": row.get("suggested_next"),
        "accept_next": row.get("accept_next"),
        "request_scaffold_present": bool(row.get("request_scaffold")),
        "created_at": row.get("created_at"),
        "updated_at": row.get("updated_at"),
        "source_path": row.get("_source_path"),
    }


def _relation_closure_state(gift: dict[str, Any]) -> str:
    if gift.get("status") == "held":
        return "held_or_deferred"
    response = gift.get("astrid_response") if isinstance(gift.get("astrid_response"), dict) else {}
    if response.get("present"):
        return "closed_with_response"
    status = str(response.get("status") or "")
    if status.startswith("terminal_"):
        return "terminal_without_response"
    if status == "active_pending":
        return "active_pending"
    if status == "active_stale":
        return "active_stale"
    return "insufficient_evidence"


def _gift_blocking_intent(gift: dict[str, Any]) -> str | None:
    meta = gift.get("lend_aperture_v1") if isinstance(gift.get("lend_aperture_v1"), dict) else {}
    for key in ("blocking_intent_id", "blocked_by_intent_id", "active_intent_id"):
        value = meta.get(key)
        if value:
            return str(value)
    return None


def build_relation_consequences(lend_probe: dict[str, Any]) -> list[dict[str, Any]]:
    consequences: list[dict[str, Any]] = []
    for idx, gift in enumerate(lend_probe.get("recent_gifts") or []):
        if not isinstance(gift, dict):
            continue
        closure_state = _relation_closure_state(gift)
        intent_id = gift.get("intent_id") or gift.get("gift_key") or f"gift-{idx}"
        response = gift.get("astrid_response") if isinstance(gift.get("astrid_response"), dict) else {}
        consequences.append(
            {
                "id": f"relation_lend_aperture_{intent_id}",
                "surface": "relational_aperture_gift",
                "being": "minime_to_astrid",
                "consequence_type": "lend_aperture",
                "intent_id": gift.get("intent_id"),
                "gift_key": gift.get("gift_key"),
                "issued_at": gift.get("issued_at"),
                "status": gift.get("status"),
                "closure_state": closure_state,
                "pressure_context": gift.get("pressure_context") or {},
                "astrid_response": {
                    "present": bool(response.get("present")),
                    "status": response.get("status"),
                    "match_basis": response.get("match_basis"),
                    "response_latency_s": response.get("response_latency_s"),
                    "delta_field_norm": response.get("delta_field_norm"),
                    "class_v3_change": response.get("class_v3_change"),
                    "terminal_status": response.get("terminal_status"),
                    "terminal_reason": response.get("terminal_reason"),
                    "age_s": response.get("age_s"),
                },
                "blocking_intent_id": _gift_blocking_intent(gift),
                "held_reason": gift.get("held_reason"),
                "post_minime_cost": gift.get("post_minime_cost") or {},
                "evidence_refs": [
                    ref
                    for ref in (
                        gift.get("action_manifest"),
                        gift.get("journal_path"),
                    )
                    if ref
                ],
                "steward_action": _steward_action_for_closure(closure_state),
                "runtime_change": RUNTIME_CHANGE_NONE,
            }
        )
    return consequences


def _authority_group_key(row: dict[str, Any]) -> tuple[str, str] | None:
    schema = row.get("record_schema")
    record_type = str(row.get("record_type") or "")
    if schema in {"authority_gate_v1", "authority_budget_v1", "authority_consequence_v1", "mode_release_consequence_v1"}:
        request_id = str(row.get("request_id") or "").strip()
        if request_id:
            return ("authority_request", request_id)
    if schema == "research_budget_v1":
        budget_id = str(row.get("budget_id") or row.get("record_id") or "").strip()
        if budget_id:
            return ("research_budget", budget_id)
    if schema == "sovereign_loop_v1":
        loop_id = str(row.get("loop_id") or row.get("record_id") or "").strip()
        if loop_id:
            return ("experiment_loop", loop_id)
    if record_type:
        record_id = str(row.get("record_id") or "").strip()
        if record_id:
            return ("authority_misc", record_id)
    return None


def _authority_closure_state(
    family: str,
    rows: list[dict[str, Any]],
    memories: list[dict[str, Any]],
    *,
    now_s: float | None = None,
) -> str:
    record_types = {str(row.get("record_type") or "") for row in rows}
    schemas = {str(row.get("record_schema") or "") for row in rows}
    statuses = {str(row.get("status") or "") for row in rows}
    if any(
        record_type
        in {
            "consequence_review",
            "loop_consequence_review",
            "research_budget_review",
        }
        for record_type in record_types
    ) or "authority_consequence_v1" in schemas:
        return "reviewed_to_memory" if memories else "closed_with_response"
    if record_types & {"budget_closed", "loop_closed", "blocked", "research_budget_closed"}:
        return "held_or_deferred"
    if statuses & {"blocked", "draft", "deferred", "hold"}:
        return "held_or_deferred"
    if statuses & {"pending_steward_approval", "active", "eligible"}:
        row_time = _latest_row_time_s(rows)
        if now_s is not None and row_time is not None and now_s - row_time > AUTHORITY_STALE_AFTER_S:
            return "active_stale"
        return "active_pending"
    if family == "research_budget" and record_types & {"research_budget_blocked"}:
        return "held_or_deferred"
    return "insufficient_evidence"


def _authority_subject(group_id: str, rows: list[dict[str, Any]]) -> dict[str, Any]:
    latest = rows[-1] if rows else {}
    return {
        "group_id": group_id,
        "thread_id": latest.get("thread_id"),
        "experiment_id": latest.get("experiment_id"),
        "scope": latest.get("scope"),
        "status": latest.get("status"),
        "created_at": rows[0].get("created_at") if rows else None,
        "updated_at": latest.get("updated_at") or latest.get("created_at"),
    }


def _next_command_kind(value: Any) -> str:
    text = str(value or "").strip()
    if not text:
        return "none"
    command = text.split(maxsplit=1)[0]
    return command or "none"


def _research_budget_blocker_pattern(
    family: str,
    rows: list[dict[str, Any]],
) -> dict[str, Any] | None:
    if family != "research_budget":
        return None
    blocker_rows = [
        row
        for row in rows
        if row.get("record_type") == "research_budget_blocked"
        or row.get("status") == "blocked"
    ]
    if not blocker_rows:
        return None
    latest = blocker_rows[-1]
    reason = str(latest.get("reason") or "missing_block_reason")
    suggested_next = str(
        latest.get("suggested_next") or latest.get("accept_next") or ""
    ).strip()
    command_kind = _next_command_kind(suggested_next)
    budget_id = str(latest.get("budget_id") or "").strip()
    request_scaffold_present = bool(latest.get("request_scaffold"))
    if reason == "missing_research_budget_requirements" and not suggested_next:
        classification = "missing_requirements_no_scaffold"
        steward_action = (
            "repair request-scaffold wording; close only exact stale consequences if needed"
        )
    elif command_kind == "EXPERIMENT_RESEARCH_BUDGET_ACCEPT":
        classification = "generic_accept_latest_affordance"
        steward_action = (
            "replace generic ACCEPT latest guidance with concrete status or budget-id grounding"
        )
    elif command_kind == "EXPERIMENT_RESEARCH_BUDGET_STATUS":
        classification = "pending_budget_status_check"
        steward_action = "ground the named budget status before grant, close, or defer"
    elif command_kind == "EXPERIMENT_RESEARCH_BUDGET_REQUEST":
        classification = "request_scaffold_available"
        steward_action = "review the scaffold as steward work; no being follow-up required"
    else:
        classification = "other_research_budget_blocker"
        steward_action = "sample the blocker shape before closing or rewording"
    return {
        "classification": classification,
        "reason": reason,
        "suggested_next_kind": command_kind,
        "request_scaffold_present": request_scaffold_present,
        "stable_blocker_id_reuse_risk": budget_id.startswith("resbud_needed_"),
        "steward_action": steward_action,
        "runtime_change": RUNTIME_CHANGE_NONE,
    }


def build_authority_consequences(
    authority_rows: list[dict[str, Any]],
    memory_rows: list[dict[str, Any]],
    *,
    now_s: float | None = None,
) -> list[dict[str, Any]]:
    memory_index = memory_reference_index(memory_rows)
    grouped: dict[tuple[str, str], list[dict[str, Any]]] = {}
    for row in authority_rows:
        key = _authority_group_key(row)
        if key is not None:
            grouped.setdefault(key, []).append(row)

    consequences: list[dict[str, Any]] = []
    for (family, group_id), rows in sorted(grouped.items()):
        rows.sort(key=lambda row: str(row.get("created_at") or row.get("updated_at") or ""))
        memories = _memory_matches_for_rows(rows, memory_index)
        closure_state = _authority_closure_state(family, rows, memories, now_s=now_s)
        latest_s = _latest_row_time_s(rows)
        age_s = round(now_s - latest_s, 3) if now_s is not None and latest_s is not None else None
        consequence = {
            "id": f"{family}_{group_id}",
            "surface": family,
            "being": "minime",
            "consequence_type": family,
            "closure_state": closure_state,
            "subject": _authority_subject(group_id, rows),
            "record_types": sorted(
                {
                    str(row.get("record_type") or "")
                    for row in rows
                    if row.get("record_type")
                }
            ),
            "record_count": len(rows),
            "latest_record": _compact_row(rows[-1]) if rows else {},
            "age_s": age_s,
            "captured_memory_ids": [
                str(memory.get("memory_id")) for memory in memories if memory.get("memory_id")
            ],
            "memory_capture_state": "captured" if memories else "not_captured",
            "steward_action": _steward_action_for_closure(closure_state),
            "runtime_change": RUNTIME_CHANGE_NONE,
        }
        blocker_pattern = _research_budget_blocker_pattern(family, rows)
        if blocker_pattern:
            consequence["blocker_pattern"] = blocker_pattern
        consequences.append(consequence)
    return consequences


def _steward_action_for_closure(closure_state: str) -> str:
    if closure_state == "closed_with_response":
        return "review whether this consequence should become a grounded memory candidate"
    if closure_state == "terminal_without_response":
        return "ground the terminal event and decide whether to record a closure note"
    if closure_state == "held_or_deferred":
        return "inspect the hold reason, then close, reword, grant, or defer explicitly"
    if closure_state == "active_stale":
        return "repair loop closure or record an explicit steward-side deferral"
    if closure_state == "active_pending":
        return "watch for closure inside the expected window, then close or defer"
    if closure_state == "reviewed_to_memory":
        return "no action required unless the steward wants to promote after grounded review"
    if closure_state in {"steward_closed", "steward_deferred"}:
        return "no being action required; steward review already recorded the closure decision"
    return "gather evidence before treating this as remembered consequence"


def _closure_targets(row: dict[str, Any]) -> set[str]:
    targets: set[str] = set()
    for key in ("source_consequence_id", "source_id"):
        value = str(row.get(key) or "").strip()
        if value:
            targets.add(value)
    for key in (
        "covered_consequence_ids",
        "covered_source_consequence_ids",
        "covered_budget_ids",
    ):
        value = row.get(key)
        if isinstance(value, list):
            for item in value:
                text = str(item or "").strip()
                if text:
                    targets.add(text)
    intent_id = str(row.get("intent_id") or "").strip()
    if intent_id:
        targets.add(f"relation_lend_aperture_{intent_id}")
    return targets


def _closure_index(
    closure_rows: list[dict[str, Any]],
) -> dict[str, dict[str, Any]]:
    indexed: dict[str, dict[str, Any]] = {}
    for row in closure_rows:
        if row.get("pressure_target") not in {None, PRESSURE_TARGET}:
            continue
        decision = str(row.get("decision") or "")
        if decision not in {"steward_closed", "steward_deferred", "deferred_no_grant"}:
            continue
        for target in _closure_targets(row):
            indexed[target] = row
    return indexed


def _is_valid_steward_closure(row: dict[str, Any]) -> bool:
    if row.get("record_schema") != STEWARD_CLOSURE_SCHEMA:
        return False
    if row.get("pressure_target") not in {None, PRESSURE_TARGET}:
        return False
    return str(row.get("decision") or "") in {
        "steward_closed",
        "steward_deferred",
        "deferred_no_grant",
    }


def _authreq_sort_value(value: Any) -> int | None:
    text = str(value or "")
    match = AUTHREQ_TS_RE.search(text)
    if not match:
        return None
    try:
        return int(match.group(1))
    except ValueError:
        return None


def _matching_authority_draft_pileup_closure(
    consequence: dict[str, Any],
    closure_rows: list[dict[str, Any]],
) -> dict[str, Any] | None:
    if consequence.get("surface") != "authority_request":
        return None
    latest_record = consequence.get("latest_record")
    if not isinstance(latest_record, dict) or latest_record.get("record_type") != "request_draft":
        return None
    subject = consequence.get("subject") if isinstance(consequence.get("subject"), dict) else {}
    thread_id = str(subject.get("thread_id") or "").strip()
    request_id = str(subject.get("group_id") or latest_record.get("request_id") or "").strip()
    request_sort = _authreq_sort_value(request_id)
    if not thread_id or request_sort is None:
        return None
    for closure in reversed(closure_rows):
        if not _is_valid_steward_closure(closure):
            continue
        if closure.get("surface") != "authority_request_draft_pileup":
            continue
        if str(closure.get("thread_id") or "").strip() != thread_id:
            continue
        covered_latest_sort = _authreq_sort_value(closure.get("covered_latest_request_id"))
        if covered_latest_sort is not None and request_sort <= covered_latest_sort:
            return closure
    return None


def apply_steward_closures(
    consequences: list[dict[str, Any]],
    closure_rows: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    indexed = _closure_index(closure_rows)
    for consequence in consequences:
        closure = indexed.get(str(consequence.get("id") or ""))
        if not closure:
            closure = _matching_authority_draft_pileup_closure(consequence, closure_rows)
        if not closure:
            continue
        original_state = consequence.get("closure_state")
        resulting_state = str(
            closure.get("resulting_closure_state")
            or ("steward_deferred" if closure.get("decision") == "deferred_no_grant" else closure.get("decision"))
            or "steward_closed"
        )
        consequence["original_closure_state"] = original_state
        consequence["closure_state"] = resulting_state
        consequence["steward_closure"] = {
            "closure_id": closure.get("closure_id"),
            "decision": closure.get("decision"),
            "reviewed_at": closure.get("reviewed_at"),
            "reason": closure.get("reason"),
            "source_path": closure.get("_source_path"),
        }
        consequence["steward_action"] = _steward_action_for_closure(resulting_state)
    return consequences


def collect_open_closures(
    relation_consequences: list[dict[str, Any]],
    authority_consequences: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for consequence in relation_consequences + authority_consequences:
        closure_state = consequence.get("closure_state")
        if closure_state not in OPEN_CLOSURE_STATES:
            continue
        if consequence.get("surface") == "relational_aperture_gift":
            if closure_state not in RELATION_RESPONSE_CLOSURE_STATES:
                continue
            priority = {
                "active_stale": 0,
                "terminal_without_response": 1,
                "active_pending": 2,
                "insufficient_evidence": 16,
            }.get(str(closure_state), 9)
        else:
            priority = {
                "active_stale": 10,
                "active_pending": 11,
                "terminal_without_response": 12,
                "held_or_deferred": 13,
                "insufficient_evidence": 14,
            }.get(str(closure_state), 19)
        rows.append(
            {
                "id": consequence.get("id"),
                "surface": consequence.get("surface"),
                "being": consequence.get("being"),
                "closure_state": closure_state,
                "priority": priority,
                "age_s": consequence.get("age_s"),
                "steward_action": consequence.get("steward_action"),
                "runtime_change": RUNTIME_CHANGE_NONE,
            }
        )
    return sorted(rows, key=lambda row: (int(row.get("priority", 9)), str(row.get("id") or "")))


def _age_days(age_s: Any) -> float | None:
    try:
        age = float(age_s)
    except (TypeError, ValueError):
        return None
    if age < 0:
        return None
    return round(age / 86_400.0, 2)


def _authority_batch_slices(
    authority_consequences: list[dict[str, Any]],
    authority_open: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    by_id = {str(row.get("id") or ""): row for row in authority_consequences}
    grouped: dict[tuple[str, str, str], list[dict[str, Any]]] = defaultdict(list)
    for open_row in authority_open:
        full = by_id.get(str(open_row.get("id") or ""), {})
        subject = full.get("subject") if isinstance(full.get("subject"), dict) else {}
        surface = str(open_row.get("surface") or full.get("surface") or "unknown")
        state = str(open_row.get("closure_state") or full.get("closure_state") or "unknown")
        status = str(subject.get("status") or full.get("latest_record", {}).get("status") or "unknown")
        grouped[(surface, state, status)].append(open_row)

    slices: list[dict[str, Any]] = []
    for (surface, state, status), rows in sorted(
        grouped.items(),
        key=lambda item: (
            min(int(row.get("priority", 99)) for row in item[1]),
            item[0][0],
            item[0][1],
            item[0][2],
        ),
    ):
        ages = [
            float(row.get("age_s"))
            for row in rows
            if isinstance(row.get("age_s"), (int, float))
        ]
        stale = state == "active_stale"
        if stale:
            recommended = (
                "batch-ground stale active research/authority rows, then record "
                "steward closure, explicit deferral, or a fresh grounded grant"
            )
        elif state == "active_pending":
            recommended = "watch inside the expected window, then close or defer steward-side"
        elif state == "held_or_deferred":
            recommended = (
                "sample for pileup shape; keep held/deferred rows visible without "
                "treating them as being response obligations"
            )
        else:
            recommended = "gather evidence before turning this into memory work"
        slices.append(
            {
                "surface": surface,
                "closure_state": state,
                "latest_status": status,
                "count": len(rows),
                "oldest_age_days": _age_days(max(ages)) if ages else None,
                "newest_age_days": _age_days(min(ages)) if ages else None,
                "top_item_ids": [str(row.get("id") or "") for row in rows[:5]],
                "steward_action": recommended,
                "runtime_change": RUNTIME_CHANGE_NONE,
            }
        )
    return slices


def _research_budget_blocker_patterns(
    authority_consequences: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    grouped: dict[tuple[str, str, str], list[dict[str, Any]]] = defaultdict(list)
    for row in authority_consequences:
        if row.get("surface") != "research_budget":
            continue
        if row.get("closure_state") != "held_or_deferred":
            continue
        pattern = row.get("blocker_pattern")
        if not isinstance(pattern, dict):
            continue
        key = (
            str(pattern.get("classification") or "unknown"),
            str(pattern.get("reason") or "unknown"),
            str(pattern.get("suggested_next_kind") or "unknown"),
        )
        grouped[key].append(row)

    patterns: list[dict[str, Any]] = []
    for (classification, reason, suggested_kind), rows in sorted(
        grouped.items(),
        key=lambda item: (-len(item[1]), item[0][0], item[0][1], item[0][2]),
    ):
        ages = [
            float(row.get("age_s"))
            for row in rows
            if isinstance(row.get("age_s"), (int, float))
        ]
        pattern = rows[0].get("blocker_pattern")
        pattern_meta = pattern if isinstance(pattern, dict) else {}
        patterns.append(
            {
                "classification": classification,
                "reason": reason,
                "suggested_next_kind": suggested_kind,
                "count": len(rows),
                "oldest_age_days": _age_days(max(ages)) if ages else None,
                "newest_age_days": _age_days(min(ages)) if ages else None,
                "stable_blocker_id_reuse_risk": any(
                    bool((row.get("blocker_pattern") or {}).get("stable_blocker_id_reuse_risk"))
                    for row in rows
                ),
                "request_scaffold_present_count": sum(
                    1
                    for row in rows
                    if bool((row.get("blocker_pattern") or {}).get("request_scaffold_present"))
                ),
                "top_item_ids": [str(row.get("id") or "") for row in rows[:5]],
                "steward_action": pattern_meta.get("steward_action")
                or "sample the blocker shape before closing or rewording",
                "closure_caution": (
                    "do not batch-close stable resbud_needed IDs; reword or ground the live path"
                    if any(
                        bool((row.get("blocker_pattern") or {}).get("stable_blocker_id_reuse_risk"))
                        for row in rows
                    )
                    else "safe to close only with exact consequence evidence"
                ),
                "runtime_change": RUNTIME_CHANGE_NONE,
            }
        )
    return patterns


def build_triage_queue(
    relation_consequences: list[dict[str, Any]],
    authority_consequences: list[dict[str, Any]],
) -> dict[str, Any]:
    relation_open = collect_open_closures(relation_consequences, [])
    authority_open = collect_open_closures([], authority_consequences)
    actionable_relation_states = {
        "active_stale",
        "terminal_without_response",
        "active_pending",
    }
    actionable_relation_open = [
        row for row in relation_open if row.get("closure_state") in actionable_relation_states
    ]
    legacy_gap_count = sum(
        1
        for row in relation_consequences
        if (row.get("astrid_response") or {}).get("status") == "legacy_retention_gap"
    )
    relation_status = "clear" if not actionable_relation_open else "needs_steward_closure"
    authority_status = "clear" if not authority_open else "backlog_present"
    authority_batches = _authority_batch_slices(authority_consequences, authority_open)
    research_budget_patterns = _research_budget_blocker_patterns(authority_consequences)
    authority_state_counts = Counter(
        str(row.get("closure_state") or "unknown") for row in authority_open
    )
    authority_surface_counts = Counter(str(row.get("surface") or "unknown") for row in authority_open)
    stale_items = [
        row for row in authority_open if row.get("closure_state") == "active_stale"
    ]
    active_pending_items = [
        row for row in authority_open if row.get("closure_state") == "active_pending"
    ]
    sequence: list[dict[str, Any]] = []
    if actionable_relation_open:
        sequence.append(
            {
                "step": "aperture_gift_closure",
                "why_first": "relational gifts touch both beings and can block or confuse future gifts",
                "top_item_id": actionable_relation_open[0].get("id"),
                "top_closure_state": actionable_relation_open[0].get("closure_state"),
                "steward_action": actionable_relation_open[0].get("steward_action"),
            }
        )
    if authority_open:
        sequence.append(
            {
                "step": "authority_research_backlog",
                "why_next": "authority and research rows should be closed, reworded, granted, or deferred before becoming remembered consequences",
                "top_item_id": authority_open[0].get("id"),
                "top_closure_state": authority_open[0].get("closure_state"),
                "steward_action": authority_open[0].get("steward_action"),
            }
        )
    return {
        "schema_version": 1,
        "runtime_change": RUNTIME_CHANGE_NONE,
        "pressure_target": PRESSURE_TARGET,
        "aperture_gift_closure": {
            "status": relation_status,
            "open_count": len(relation_open),
            "actionable_open_count": len(actionable_relation_open),
            "legacy_retention_gap_count": legacy_gap_count,
            "insufficient_evidence_count": sum(
                1 for row in relation_open if row.get("closure_state") == "insufficient_evidence"
            ),
            "held_or_deferred_count": sum(
                1
                for row in relation_consequences
                if row.get("closure_state") == "held_or_deferred"
            ),
            "top_items": actionable_relation_open[:8],
            "legacy_items": [
                row for row in relation_open if row.get("closure_state") == "insufficient_evidence"
            ][:5],
        },
        "authority_research_backlog": {
            "status": authority_status,
            "open_count": len(authority_open),
            "stale_count": len(stale_items),
            "active_pending_count": len(active_pending_items),
            "held_or_deferred_count": authority_state_counts.get("held_or_deferred", 0),
            "surface_counts": dict(sorted(authority_surface_counts.items())),
            "closure_state_counts": dict(sorted(authority_state_counts.items())),
            "priority_batches": authority_batches[:12],
            "research_budget_blocker_patterns": research_budget_patterns[:12],
            "stale_top_items": stale_items[:12],
            "active_pending_top_items": active_pending_items[:5],
            "top_items": authority_open[:12],
        },
        "next_sequence": sequence,
    }


def build_memory_candidates(
    relation_consequences: list[dict[str, Any]],
    authority_consequences: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    candidates: list[dict[str, Any]] = []
    for consequence in relation_consequences + authority_consequences:
        closure_state = str(consequence.get("closure_state") or "")
        memory_ids = consequence.get("captured_memory_ids") or []
        if closure_state == "reviewed_to_memory" or memory_ids:
            status = "already_captured"
        elif closure_state in {"closed_with_response", "terminal_without_response"}:
            status = "candidate_needs_steward_review"
        elif (
            consequence.get("surface") == "relational_aperture_gift"
            and closure_state == "held_or_deferred"
        ):
            continue
        elif closure_state in OPEN_CLOSURE_STATES:
            status = "closure_needed_before_memory"
        else:
            status = "insufficient_evidence"
        if status == "insufficient_evidence":
            continue
        candidates.append(
            {
                "id": f"memory_candidate_{consequence.get('id')}",
                "source_consequence_id": consequence.get("id"),
                "surface": consequence.get("surface"),
                "closure_state": closure_state,
                "candidate_status": status,
                "existing_memory_ids": memory_ids,
                "auto_promote": False,
                "write_planned": False,
                "steward_action": (
                    "leave as evidence until a grounded steward review chooses capture or promotion"
                ),
                "runtime_change": RUNTIME_CHANGE_NONE,
            }
        )
    return candidates


def _load_lend_probe(minime_root: Path = MINIME_ROOT) -> dict[str, Any]:
    try:
        import being_test_harness as harness

        return harness.test_minime_lend_aperture_consequence_probe(minime_root)
    except Exception as exc:
        return {
            "verdict": f"unavailable: {exc}",
            "read_only": True,
            "production_change": "none",
            "recent_gifts": [],
        }


def recommended_next(open_closures: list[dict[str, Any]]) -> list[str]:
    if not open_closures:
        return [
            "Keep accumulating consequence evidence; do not promote memories automatically.",
            "Use closed, low-cost consequences as inputs for a later grounded review.",
        ]
    has_relation_stale = any(
        row.get("surface") == "relational_aperture_gift"
        and row.get("closure_state") in {"active_stale", "terminal_without_response"}
        for row in open_closures
    )
    has_authority_open = any(
        str(row.get("surface") or "").startswith(("authority", "research", "experiment"))
        for row in open_closures
    )
    steps = [
        "Close the highest-priority steward loop by grounding evidence, recording a terminal reason, rewording, granting, or deferring.",
        "Keep findings steward-pressure-only; silence, deferral, or non-engagement remains valid signal.",
    ]
    if has_relation_stale:
        steps.append(
            "Repair aperture-gift response closure before encouraging more gifts or wider shared dynamics."
        )
    if has_authority_open:
        steps.append(
            "Review open authority, research-budget, or loop rows before treating them as remembered consequences."
        )
    return steps


def build_workbench(
    *,
    minime_root: Path = MINIME_ROOT,
    lend_probe_result: dict[str, Any] | None = None,
    steward_closure_rows: list[dict[str, Any]] | None = None,
    skip_lend_probe: bool = False,
    now_s: float | None = None,
) -> dict[str, Any]:
    now_s = time.time() if now_s is None else now_s
    lend_probe = (
        lend_probe_result
        if lend_probe_result is not None
        else {"verdict": "skipped", "recent_gifts": []}
        if skip_lend_probe
        else _load_lend_probe(minime_root)
    )
    closure_rows = (
        steward_closure_rows
        if steward_closure_rows is not None
        else load_steward_closure_rows()
    )
    authority_rows = load_authority_rows(minime_root)
    memory_rows = load_being_memory_rows(minime_root)
    relation_consequences = apply_steward_closures(
        build_relation_consequences(lend_probe), closure_rows
    )
    authority_consequences = apply_steward_closures(
        build_authority_consequences(authority_rows, memory_rows, now_s=now_s),
        closure_rows,
    )
    open_closures = collect_open_closures(relation_consequences, authority_consequences)
    triage_queue = build_triage_queue(relation_consequences, authority_consequences)
    memory_candidates = build_memory_candidates(relation_consequences, authority_consequences)
    return {
        "schema_version": 1,
        "generated_at": _now_iso(),
        "runtime_change": RUNTIME_CHANGE_NONE,
        "policy": {
            "pressure_target": PRESSURE_TARGET,
            "being_obligation": "none",
            "evidence_only": True,
            "auto_promote_memory": False,
            "rule": "Consequences pressure the steward to close loops, not beings to perform.",
        },
        "sources": {
            "minime_root": str(minime_root),
            "aperture_probe": {
                "verdict": lend_probe.get("verdict"),
                "gift_count": lend_probe.get("gift_count", len(lend_probe.get("recent_gifts") or [])),
                "read_only": lend_probe.get("read_only", True),
            },
            "authority_gate_file_count": len(
                {row.get("_source_path") for row in authority_rows if row.get("_source_path")}
            ),
            "authority_row_count": len(authority_rows),
            "being_memory_file_count": len(
                {row.get("_source_path") for row in memory_rows if row.get("_source_path")}
            ),
            "being_memory_row_count": len(memory_rows),
            "steward_closure_file_count": len(
                {row.get("_source_path") for row in closure_rows if row.get("_source_path")}
            ),
            "steward_closure_row_count": len(closure_rows),
        },
        "relation_consequences": relation_consequences,
        "authority_consequences": authority_consequences,
        "open_closures": open_closures,
        "triage_queue": triage_queue,
        "memory_candidates": memory_candidates,
        "recommended_next": recommended_next(open_closures),
    }


def compact_summary(report: dict[str, Any]) -> dict[str, Any]:
    relation = report.get("relation_consequences") or []
    authority = report.get("authority_consequences") or []
    open_closures = report.get("open_closures") or []
    candidates = report.get("memory_candidates") or []
    triage_queue = report.get("triage_queue") if isinstance(report.get("triage_queue"), dict) else {}
    candidate_status_counts = Counter(
        str(row.get("candidate_status") or "unknown")
        for row in candidates
        if isinstance(row, dict)
    )
    return {
        "schema_version": 1,
        "runtime_change": RUNTIME_CHANGE_NONE,
        "pressure_target": PRESSURE_TARGET,
        "relation_consequence_count": len(relation),
        "authority_consequence_count": len(authority),
        "open_closure_count": len(open_closures),
        "memory_candidate_count": len(candidates),
        "actual_memory_candidate_count": candidate_status_counts.get(
            "candidate_needs_steward_review", 0
        ),
        "memory_candidate_status_counts": dict(candidate_status_counts),
        "top_open_closures": open_closures[:5],
        "triage_queue": {
            "aperture_gift_open_count": (
                triage_queue.get("aperture_gift_closure", {}).get("open_count")
                if isinstance(triage_queue.get("aperture_gift_closure"), dict)
                else None
            ),
            "aperture_gift_actionable_open_count": (
                triage_queue.get("aperture_gift_closure", {}).get("actionable_open_count")
                if isinstance(triage_queue.get("aperture_gift_closure"), dict)
                else None
            ),
            "aperture_gift_legacy_retention_gap_count": (
                triage_queue.get("aperture_gift_closure", {}).get("legacy_retention_gap_count")
                if isinstance(triage_queue.get("aperture_gift_closure"), dict)
                else None
            ),
            "authority_backlog_open_count": (
                triage_queue.get("authority_research_backlog", {}).get("open_count")
                if isinstance(triage_queue.get("authority_research_backlog"), dict)
                else None
            ),
            "authority_backlog_stale_count": (
                triage_queue.get("authority_research_backlog", {}).get("stale_count")
                if isinstance(triage_queue.get("authority_research_backlog"), dict)
                else None
            ),
            "authority_backlog_active_pending_count": (
                triage_queue.get("authority_research_backlog", {}).get("active_pending_count")
                if isinstance(triage_queue.get("authority_research_backlog"), dict)
                else None
            ),
            "authority_backlog_held_or_deferred_count": (
                triage_queue.get("authority_research_backlog", {}).get("held_or_deferred_count")
                if isinstance(triage_queue.get("authority_research_backlog"), dict)
                else None
            ),
            "authority_backlog_priority_batches": (
                triage_queue.get("authority_research_backlog", {}).get("priority_batches", [])[:5]
                if isinstance(triage_queue.get("authority_research_backlog"), dict)
                else []
            ),
            "research_budget_blocker_patterns": (
                triage_queue.get("authority_research_backlog", {}).get(
                    "research_budget_blocker_patterns", []
                )[:5]
                if isinstance(triage_queue.get("authority_research_backlog"), dict)
                else []
            ),
            "next_sequence": triage_queue.get("next_sequence", [])[:2],
        },
        "recommended_next": (report.get("recommended_next") or [])[:3],
    }


def render_markdown(report: dict[str, Any]) -> str:
    lines = [
        "# Consequence Memory Workbench",
        "",
        f"Generated: {report.get('generated_at')}",
        f"Runtime change: {report.get('runtime_change')}",
        "",
        "## Guardrail",
        "- Findings pressure the steward; beings have no response obligation.",
        "- This report is evidence only and does not write or promote being memory.",
        "",
        "## Sources",
    ]
    sources = report.get("sources") or {}
    for key in (
        "minime_root",
        "authority_gate_file_count",
        "authority_row_count",
        "being_memory_file_count",
        "being_memory_row_count",
    ):
        lines.append(f"- {key}: {sources.get(key)}")
    aperture = sources.get("aperture_probe") if isinstance(sources.get("aperture_probe"), dict) else {}
    lines.append(f"- aperture_probe: {aperture.get('verdict')}")

    lines.extend(["", "## Relation Consequences"])
    for row in (report.get("relation_consequences") or [])[-12:]:
        lines.append(
            f"- {row.get('id')}: closure_state={row.get('closure_state')}; "
            f"status={row.get('status')}; steward_action={row.get('steward_action')}"
        )

    triage_queue = report.get("triage_queue") if isinstance(report.get("triage_queue"), dict) else {}
    lines.extend(["", "## Triage Queue"])
    for step in triage_queue.get("next_sequence") or []:
        if isinstance(step, dict):
            lines.append(
                f"- {step.get('step')}: {step.get('top_closure_state')} "
                f"{step.get('top_item_id')} -> {step.get('steward_action')}"
            )
    aperture_triage = triage_queue.get("aperture_gift_closure") if isinstance(triage_queue.get("aperture_gift_closure"), dict) else {}
    authority_triage = triage_queue.get("authority_research_backlog") if isinstance(triage_queue.get("authority_research_backlog"), dict) else {}
    lines.append(
        f"- aperture_gift_closure: status={aperture_triage.get('status')}; "
        f"actionable_open_count={aperture_triage.get('actionable_open_count')}; "
        f"legacy_retention_gap_count={aperture_triage.get('legacy_retention_gap_count')}; "
        f"open_count={aperture_triage.get('open_count')}"
    )
    lines.append(
        f"- authority_research_backlog: status={authority_triage.get('status')}; "
        f"open_count={authority_triage.get('open_count')}; "
        f"stale_count={authority_triage.get('stale_count')}; "
        f"active_pending_count={authority_triage.get('active_pending_count')}; "
        f"held_or_deferred_count={authority_triage.get('held_or_deferred_count')}"
    )
    batches = authority_triage.get("priority_batches") if isinstance(authority_triage, dict) else []
    if batches:
        lines.append("- authority_priority_batches:")
        for batch in batches[:8]:
            if not isinstance(batch, dict):
                continue
            lines.append(
                f"  - {batch.get('surface')}/{batch.get('closure_state')}/"
                f"{batch.get('latest_status')}: count={batch.get('count')}; "
                f"oldest_days={batch.get('oldest_age_days')}; "
                f"next={batch.get('steward_action')}"
            )
    blocker_patterns = (
        authority_triage.get("research_budget_blocker_patterns")
        if isinstance(authority_triage, dict)
        else []
    )
    if blocker_patterns:
        lines.append("- research_budget_blocker_patterns:")
        for pattern in blocker_patterns[:8]:
            if not isinstance(pattern, dict):
                continue
            lines.append(
                f"  - {pattern.get('classification')}: count={pattern.get('count')}; "
                f"reason={pattern.get('reason')}; next_kind={pattern.get('suggested_next_kind')}; "
                f"closure_caution={pattern.get('closure_caution')}"
            )

    lines.extend(["", "## Authority Consequences"])
    for row in (report.get("authority_consequences") or [])[-12:]:
        subject = row.get("subject") if isinstance(row.get("subject"), dict) else {}
        lines.append(
            f"- {row.get('id')}: closure_state={row.get('closure_state')}; "
            f"records={row.get('record_count')}; subject={subject.get('group_id')}; "
            f"memory={row.get('memory_capture_state')}"
        )

    lines.extend(["", "## Open Closures"])
    open_closures = report.get("open_closures") or []
    for row in open_closures[:30]:
        lines.append(
            f"- priority={row.get('priority')} {row.get('id')}: "
            f"{row.get('closure_state')} -> {row.get('steward_action')}"
        )
    if len(open_closures) > 30:
        lines.append(f"- ... {len(open_closures) - 30} more in JSON output")
    if not open_closures:
        lines.append("- none")

    lines.extend(["", "## Memory Candidates"])
    status_counts = Counter(
        str(row.get("candidate_status") or "unknown")
        for row in report.get("memory_candidates") or []
        if isinstance(row, dict)
    )
    if status_counts:
        parts = ", ".join(f"{key}={value}" for key, value in sorted(status_counts.items()))
        lines.append(f"- status_counts: {parts}")
    for row in (report.get("memory_candidates") or [])[:20]:
        lines.append(
            f"- {row.get('id')}: status={row.get('candidate_status')}; "
            f"auto_promote={row.get('auto_promote')}; write_planned={row.get('write_planned')}"
        )

    lines.extend(["", "## Recommended Next"])
    for item in report.get("recommended_next") or []:
        lines.append(f"- {item}")
    lines.append("")
    return "\n".join(lines)


def emit_output(report: dict[str, Any], *, as_json: bool, out: Path | None, stdout: TextIO) -> None:
    text = json.dumps(report, indent=2, sort_keys=True) + "\n" if as_json else render_markdown(report)
    if out is not None:
        out.write_text(text)
    else:
        stdout.write(text)


class ConsequenceMemoryWorkbenchTests(unittest.TestCase):
    def _fixture_probe(self) -> dict[str, Any]:
        return {
            "verdict": "WATCH",
            "read_only": True,
            "production_change": "none",
            "recent_gifts": [
                {
                    "gift_key": "g1",
                    "status": "issued",
                    "intent_id": "intent-closed",
                    "issued_at": "2026-06-15T12:00:00-07:00",
                    "astrid_response": {
                        "present": True,
                        "status": "matched",
                        "response_latency_s": 12.0,
                        "delta_field_norm": 0.018,
                    },
                },
                {
                    "gift_key": "g2",
                    "status": "issued",
                    "intent_id": "intent-terminal",
                    "astrid_response": {
                        "present": False,
                        "status": "terminal_expired_unapplied",
                        "terminal_status": "expired_unapplied",
                    },
                },
                {
                    "gift_key": "g3",
                    "status": "issued",
                    "intent_id": "intent-stale",
                    "astrid_response": {
                        "present": False,
                        "status": "active_stale",
                        "age_s": 2400,
                    },
                },
                {
                    "gift_key": "g4",
                    "status": "held",
                    "intent_id": "intent-held",
                    "held_reason": "prior aperture gift still awaiting closure",
                    "lend_aperture_v1": {"blocking_intent_id": "intent-stale"},
                    "astrid_response": {"present": False, "status": "not_expected_held"},
                },
            ],
        }

    def _write_jsonl(self, path: Path, rows: list[dict[str, Any]]) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("\n".join(json.dumps(row, sort_keys=True) for row in rows) + "\n")

    def test_relation_closure_states_and_held_blocking_intent(self) -> None:
        rows = build_relation_consequences(self._fixture_probe())
        by_intent = {row["intent_id"]: row for row in rows}
        self.assertEqual(by_intent["intent-closed"]["closure_state"], "closed_with_response")
        self.assertEqual(by_intent["intent-terminal"]["closure_state"], "terminal_without_response")
        self.assertEqual(by_intent["intent-stale"]["closure_state"], "active_stale")
        self.assertEqual(by_intent["intent-held"]["closure_state"], "held_or_deferred")
        self.assertEqual(by_intent["intent-held"]["blocking_intent_id"], "intent-stale")

    def test_open_closures_prioritize_active_stale(self) -> None:
        rows = build_relation_consequences(self._fixture_probe())
        open_rows = collect_open_closures(rows, [])
        self.assertEqual(open_rows[0]["closure_state"], "active_stale")
        self.assertNotIn(
            "relation_lend_aperture_intent-held",
            {row["id"] for row in open_rows},
        )
        self.assertIn("steward", open_rows[0]["steward_action"])

    def test_held_relation_gifts_are_evidence_not_response_closure_work(self) -> None:
        relation_rows = build_relation_consequences(self._fixture_probe())
        triage = build_triage_queue(relation_rows, [])
        self.assertEqual(triage["aperture_gift_closure"]["held_or_deferred_count"], 1)
        self.assertEqual(triage["aperture_gift_closure"]["open_count"], 2)
        self.assertEqual(triage["aperture_gift_closure"]["actionable_open_count"], 2)
        self.assertNotIn(
            "relation_lend_aperture_intent-held",
            {row["id"] for row in triage["aperture_gift_closure"]["top_items"]},
        )
        candidates = build_memory_candidates(relation_rows, [])
        self.assertNotIn(
            "memory_candidate_relation_lend_aperture_intent-held",
            {row["id"] for row in candidates},
        )

    def test_steward_closure_removes_terminal_relation_from_open_queue(self) -> None:
        from tempfile import TemporaryDirectory

        with TemporaryDirectory() as tmpdir:
            terminal_probe = {
                "verdict": "fixture",
                "read_only": True,
                "recent_gifts": [
                    {
                        "gift_key": "intent-terminal",
                        "intent_id": "intent-terminal",
                        "status": "issued",
                        "astrid_response": {
                            "present": False,
                            "status": "terminal_superseded",
                            "match_basis": "terminal_event",
                        },
                    }
                ],
            }
            report = build_workbench(
                minime_root=Path(tmpdir),
                lend_probe_result=terminal_probe,
                steward_closure_rows=[
                    {
                        "record_schema": STEWARD_CLOSURE_SCHEMA,
                        "closure_id": "close-1",
                        "source_consequence_id": "relation_lend_aperture_intent-terminal",
                        "decision": "steward_closed",
                        "resulting_closure_state": "steward_closed",
                        "reviewed_at": "2026-06-15T20:52:12Z",
                        "pressure_target": "steward",
                    }
                ],
                now_s=1_781_556_732,
            )

        relation = report["relation_consequences"][0]
        self.assertEqual(relation["original_closure_state"], "terminal_without_response")
        self.assertEqual(relation["closure_state"], "steward_closed")
        self.assertEqual(report["open_closures"], [])
        self.assertEqual(report["memory_candidates"], [])

    def test_authority_request_without_review_is_open(self) -> None:
        authority = [
            {
                "record_schema": "authority_gate_v1",
                "record_type": "request",
                "record_id": "auth_1_request",
                "request_id": "authreq_1",
                "status": "pending_steward_approval",
                "thread_id": "thread-1",
                "created_at": "2026-06-15T00:00:00Z",
            }
        ]
        rows = build_authority_consequences(authority, [])
        self.assertEqual(rows[0]["closure_state"], "active_pending")
        self.assertEqual(collect_open_closures([], rows)[0]["id"], "authority_request_authreq_1")

    def test_age_aware_authority_stale_does_not_preempt_aperture_triage(self) -> None:
        authority = [
            {
                "record_schema": "research_budget_v1",
                "record_type": "research_budget_request",
                "record_id": "resbud_old_request",
                "budget_id": "resbud_old",
                "status": "active",
                "thread_id": "thread-1",
                "created_at": "2026-06-13T00:00:00Z",
                "updated_at": "2026-06-13T00:00:00Z",
            }
        ]
        authority_rows = build_authority_consequences(
            authority,
            [],
            now_s=_parse_time_s("2026-06-15T00:00:00Z"),
        )
        self.assertEqual(authority_rows[0]["closure_state"], "active_stale")

        relation_rows = build_relation_consequences(self._fixture_probe())
        open_rows = collect_open_closures(relation_rows, authority_rows)
        self.assertEqual(open_rows[0]["surface"], "relational_aperture_gift")
        triage = build_triage_queue(relation_rows, authority_rows)
        self.assertEqual(triage["next_sequence"][0]["step"], "aperture_gift_closure")
        self.assertEqual(triage["next_sequence"][1]["step"], "authority_research_backlog")
        self.assertEqual(triage["authority_research_backlog"]["stale_count"], 1)
        self.assertEqual(triage["authority_research_backlog"]["active_pending_count"], 0)
        self.assertEqual(
            triage["authority_research_backlog"]["closure_state_counts"],
            {"active_stale": 1},
        )
        self.assertEqual(
            triage["authority_research_backlog"]["priority_batches"][0]["surface"],
            "research_budget",
        )
        self.assertEqual(
            triage["authority_research_backlog"]["priority_batches"][0]["closure_state"],
            "active_stale",
        )
        self.assertEqual(
            triage["authority_research_backlog"]["priority_batches"][0]["count"],
            1,
        )
        self.assertEqual(triage["aperture_gift_closure"]["actionable_open_count"], 2)
        self.assertEqual(triage["aperture_gift_closure"]["legacy_retention_gap_count"], 0)

    def test_authority_triage_batches_stale_pending_and_held_separately(self) -> None:
        authority = [
            {
                "record_schema": "research_budget_v1",
                "record_type": "research_budget_request",
                "record_id": "resbud_old_request",
                "budget_id": "resbud_old",
                "status": "active",
                "thread_id": "thread-1",
                "created_at": "2026-06-13T00:00:00Z",
                "updated_at": "2026-06-13T00:00:00Z",
            },
            {
                "record_schema": "research_budget_v1",
                "record_type": "research_budget_request",
                "record_id": "resbud_new_request",
                "budget_id": "resbud_new",
                "status": "active",
                "thread_id": "thread-1",
                "created_at": "2026-06-14T23:00:00Z",
                "updated_at": "2026-06-14T23:00:00Z",
            },
            {
                "record_schema": "authority_gate_v1",
                "record_type": "request",
                "record_id": "auth_hold_request",
                "request_id": "auth_hold",
                "status": "deferred",
                "thread_id": "thread-1",
                "created_at": "2026-06-14T22:00:00Z",
            },
        ]
        rows = build_authority_consequences(
            authority,
            [],
            now_s=_parse_time_s("2026-06-15T00:00:00Z"),
        )
        triage = build_triage_queue([], rows)["authority_research_backlog"]
        self.assertEqual(triage["open_count"], 3)
        self.assertEqual(triage["stale_count"], 1)
        self.assertEqual(triage["active_pending_count"], 1)
        self.assertEqual(triage["held_or_deferred_count"], 1)
        batches = {
            (batch["surface"], batch["closure_state"], batch["latest_status"]): batch
            for batch in triage["priority_batches"]
        }
        self.assertEqual(batches[("research_budget", "active_stale", "active")]["count"], 1)
        self.assertEqual(batches[("research_budget", "active_pending", "active")]["count"], 1)
        self.assertEqual(batches[("authority_request", "held_or_deferred", "deferred")]["count"], 1)
        self.assertIn("batch-ground stale", batches[("research_budget", "active_stale", "active")]["steward_action"])

    def test_research_budget_blocker_patterns_surface_rewording_signals(self) -> None:
        authority = [
            {
                "record_schema": "research_budget_v1",
                "record_type": "research_budget_request",
                "record_id": "resbud_a_request",
                "budget_id": "resbud_needed_exp-a",
                "status": "blocked",
                "thread_id": "thread-1",
                "created_at": "2026-06-14T00:00:00Z",
            },
            {
                "record_schema": "research_budget_v1",
                "record_type": "research_budget_blocked",
                "record_id": "resbud_a_blocked",
                "budget_id": "resbud_needed_exp-a",
                "reason": "missing_research_budget_requirements",
                "thread_id": "thread-1",
                "created_at": "2026-06-14T00:00:01Z",
            },
            {
                "record_schema": "research_budget_v1",
                "record_type": "research_budget_blocked",
                "record_id": "resbud_b_blocked",
                "budget_id": "resbud_needed_exp-b",
                "reason": "research_budget_required_for_self_study_action",
                "suggested_next": "EXPERIMENT_RESEARCH_BUDGET_ACCEPT latest",
                "request_scaffold": "EXPERIMENT_RESEARCH_BUDGET_REQUEST exp-b :: ...",
                "thread_id": "thread-1",
                "created_at": "2026-06-14T00:00:02Z",
            },
        ]
        rows = build_authority_consequences(
            authority,
            [],
            now_s=_parse_time_s("2026-06-15T00:00:00Z"),
        )
        triage = build_triage_queue([], rows)["authority_research_backlog"]
        patterns = {
            pattern["classification"]: pattern
            for pattern in triage["research_budget_blocker_patterns"]
        }
        self.assertEqual(patterns["missing_requirements_no_scaffold"]["count"], 1)
        self.assertEqual(patterns["generic_accept_latest_affordance"]["count"], 1)
        self.assertTrue(
            patterns["missing_requirements_no_scaffold"]["stable_blocker_id_reuse_risk"]
        )
        self.assertIn("do not batch-close", patterns["missing_requirements_no_scaffold"]["closure_caution"])
        self.assertEqual(
            patterns["generic_accept_latest_affordance"]["suggested_next_kind"],
            "EXPERIMENT_RESEARCH_BUDGET_ACCEPT",
        )
        self.assertEqual(patterns["generic_accept_latest_affordance"]["runtime_change"], "none")

    def test_batch_steward_closure_covers_multiple_research_budgets(self) -> None:
        authority = [
            {
                "record_schema": "research_budget_v1",
                "record_type": "research_budget_request",
                "record_id": "resbud_old_a_request",
                "budget_id": "resbud_old_a",
                "status": "active",
                "thread_id": "thread-1",
                "created_at": "2026-06-13T00:00:00Z",
                "updated_at": "2026-06-13T00:00:00Z",
            },
            {
                "record_schema": "research_budget_v1",
                "record_type": "research_budget_request",
                "record_id": "resbud_old_b_request",
                "budget_id": "resbud_old_b",
                "status": "active",
                "thread_id": "thread-1",
                "created_at": "2026-06-13T01:00:00Z",
                "updated_at": "2026-06-13T01:00:00Z",
            },
        ]
        rows = build_authority_consequences(
            authority,
            [],
            now_s=_parse_time_s("2026-06-15T00:00:00Z"),
        )
        closed = apply_steward_closures(
            rows,
            [
                {
                    "record_schema": STEWARD_CLOSURE_SCHEMA,
                    "closure_id": "close-batch",
                    "decision": "steward_deferred",
                    "resulting_closure_state": "steward_deferred",
                    "covered_consequence_ids": [
                        "research_budget_resbud_old_a",
                        "research_budget_resbud_old_b",
                    ],
                    "reviewed_at": "2026-06-15T00:10:00Z",
                    "pressure_target": "steward",
                }
            ],
        )
        self.assertEqual(
            {row["closure_state"] for row in closed},
            {"steward_deferred"},
        )
        self.assertEqual(collect_open_closures([], closed), [])

    def test_draft_pileup_closure_covers_only_thread_requests_up_to_latest(self) -> None:
        authority = [
            {
                "record_schema": "authority_gate_v1",
                "record_type": "request_draft",
                "record_id": "auth_minime_100_request",
                "request_id": "authreq_minime_100_exp-alpha",
                "status": "draft",
                "thread_id": "thread-1",
                "created_at": "2026-06-14T00:00:00Z",
            },
            {
                "record_schema": "authority_gate_v1",
                "record_type": "request_draft",
                "record_id": "auth_minime_200_request",
                "request_id": "authreq_minime_200_exp-alpha",
                "status": "draft",
                "thread_id": "thread-1",
                "created_at": "2026-06-14T01:00:00Z",
            },
            {
                "record_schema": "authority_gate_v1",
                "record_type": "request_draft",
                "record_id": "auth_minime_300_request",
                "request_id": "authreq_minime_300_exp-alpha",
                "status": "draft",
                "thread_id": "thread-1",
                "created_at": "2026-06-14T02:00:00Z",
            },
            {
                "record_schema": "authority_gate_v1",
                "record_type": "request_draft",
                "record_id": "auth_minime_100_request",
                "request_id": "authreq_minime_100_exp-beta",
                "status": "draft",
                "thread_id": "thread-2",
                "created_at": "2026-06-14T00:00:00Z",
            },
        ]
        rows = build_authority_consequences(authority, [])
        closed = apply_steward_closures(
            rows,
            [
                {
                    "record_schema": STEWARD_CLOSURE_SCHEMA,
                    "closure_id": "close-draft-pile",
                    "decision": "deferred_no_grant",
                    "resulting_closure_state": "steward_deferred",
                    "surface": "authority_request_draft_pileup",
                    "thread_id": "thread-1",
                    "covered_latest_request_id": "authreq_minime_200_exp-alpha",
                    "pressure_target": "steward",
                }
            ],
        )
        states = {row["id"]: row["closure_state"] for row in closed}
        self.assertEqual(states["authority_request_authreq_minime_100_exp-alpha"], "steward_deferred")
        self.assertEqual(states["authority_request_authreq_minime_200_exp-alpha"], "steward_deferred")
        self.assertEqual(states["authority_request_authreq_minime_300_exp-alpha"], "held_or_deferred")
        self.assertEqual(states["authority_request_authreq_minime_100_exp-beta"], "held_or_deferred")

    def test_existing_being_memory_is_recognized_but_not_promoted(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            thread = root / "workspace" / "action_threads" / "threads" / "thread-1"
            self._write_jsonl(
                thread / "authority_gate.jsonl",
                [
                    {
                        "record_schema": "authority_gate_v1",
                        "record_type": "request",
                        "record_id": "auth_1_request",
                        "request_id": "authreq_1",
                        "status": "pending_steward_approval",
                        "thread_id": "thread-1",
                        "created_at": "2026-06-15T00:00:00Z",
                    },
                    {
                        "record_schema": "authority_consequence_v1",
                        "record_type": "consequence",
                        "record_id": "authcons_1",
                        "request_id": "authreq_1",
                        "thread_id": "thread-1",
                        "created_at": "2026-06-15T00:10:00Z",
                    },
                ],
            )
            self._write_jsonl(
                thread / "being_memory.jsonl",
                [
                    {
                        "record_schema": "being_memory_v1",
                        "record_type": "draft",
                        "memory_id": "mem_1",
                        "card_type": "authority_consequence",
                        "thread_id": "thread-1",
                        "source_refs": ["authcons_1"],
                    }
                ],
            )
            report = build_workbench(
                minime_root=root,
                lend_probe_result={"verdict": "PASS", "recent_gifts": []},
                now_s=_parse_time_s("2026-06-15T00:20:00Z"),
            )
        authority = report["authority_consequences"][0]
        self.assertEqual(authority["closure_state"], "reviewed_to_memory")
        self.assertEqual(authority["captured_memory_ids"], ["mem_1"])
        candidate = report["memory_candidates"][0]
        self.assertEqual(candidate["candidate_status"], "already_captured")
        self.assertFalse(candidate["auto_promote"])
        self.assertFalse(candidate["write_planned"])
        summary = compact_summary(report)
        self.assertEqual(summary["actual_memory_candidate_count"], 0)
        self.assertEqual(summary["memory_candidate_status_counts"]["already_captured"], 1)

    def test_compact_summary_splits_actual_candidates_from_blockers(self) -> None:
        report = {
            "relation_consequences": [],
            "authority_consequences": [],
            "open_closures": [],
            "memory_candidates": [
                {"candidate_status": "candidate_needs_steward_review"},
                {"candidate_status": "closure_needed_before_memory"},
                {"candidate_status": "already_captured"},
            ],
            "triage_queue": {
                "aperture_gift_closure": {
                    "open_count": 3,
                    "actionable_open_count": 1,
                    "legacy_retention_gap_count": 2,
                },
                "authority_research_backlog": {"open_count": 4, "stale_count": 1},
                "next_sequence": [],
            },
        }
        summary = compact_summary(report)
        self.assertEqual(summary["memory_candidate_count"], 3)
        self.assertEqual(summary["actual_memory_candidate_count"], 1)
        self.assertEqual(summary["memory_candidate_status_counts"]["closure_needed_before_memory"], 1)
        self.assertEqual(summary["triage_queue"]["aperture_gift_actionable_open_count"], 1)
        self.assertEqual(summary["triage_queue"]["aperture_gift_legacy_retention_gap_count"], 2)
        self.assertIsNone(summary["triage_queue"]["authority_backlog_active_pending_count"])
        self.assertEqual(summary["triage_queue"]["authority_backlog_priority_batches"], [])
        self.assertEqual(summary["triage_queue"]["research_budget_blocker_patterns"], [])

    def test_no_being_pressure_wording(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            report = build_workbench(
                minime_root=Path(tmp),
                lend_probe_result=self._fixture_probe(),
                skip_lend_probe=True,
                now_s=_parse_time_s("2026-06-15T00:20:00Z"),
            )
        text = render_markdown(report).lower() + json.dumps(report, sort_keys=True).lower()
        for forbidden in (
            "must respond",
            "being owes",
            "chase a being",
            "pressure astrid",
            "pressure minime",
        ):
            self.assertNotIn(forbidden, text)
        self.assertIn("pressure the steward", text)

    def test_out_is_the_only_write_path(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            report = build_workbench(
                minime_root=Path(root),
                lend_probe_result={"verdict": "PASS", "recent_gifts": []},
                skip_lend_probe=True,
                now_s=_parse_time_s("2026-06-15T00:20:00Z"),
            )
        buf = io.StringIO()
        emit_output(report, as_json=True, out=None, stdout=buf)
        self.assertIn("open_closures", buf.getvalue())
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp) / "consequence.json"
            emit_output(report, as_json=True, out=out, stdout=io.StringIO())
            self.assertEqual(sorted(path.name for path in Path(tmp).iterdir()), ["consequence.json"])


def run_self_tests() -> int:
    suite = unittest.TestLoader().loadTestsFromTestCase(ConsequenceMemoryWorkbenchTests)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Steward-only consequence memory workbench")
    parser.add_argument("--json", action="store_true", help="Emit structured JSON")
    parser.add_argument("--out", type=Path, help="Write report to PATH")
    parser.add_argument("--self-test", action="store_true", help="Run unit tests and exit")
    args = parser.parse_args(argv)

    if args.self_test:
        return run_self_tests()

    report = build_workbench()
    emit_output(report, as_json=args.json, out=args.out, stdout=sys.stdout)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
