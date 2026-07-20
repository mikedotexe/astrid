#!/usr/bin/env python3
"""Track evidence-backed handling of Astrid introspection artifacts.

Read-only by default. Use --write on mutating subcommands to append events and
refresh the materialized status files.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import sys
import time
import unittest
import uuid
from collections import Counter
from pathlib import Path
from typing import Any

try:
    from authority_state import normalize_artifact_authority_tree
except ModuleNotFoundError:  # unittest/importlib execution from the repository root
    from scripts.authority_state import normalize_artifact_authority_tree

try:
    from evidence_store import append_domain_events, read_domain_events, v2_active_for_state
except ModuleNotFoundError:
    from scripts.evidence_store import (
        append_domain_events,
        read_domain_events,
        v2_active_for_state,
    )

try:
    from projection_receipt import projector_receipt
except ModuleNotFoundError:
    from scripts.projection_receipt import projector_receipt

ASTRID_REPO = Path(__file__).resolve().parents[1]
ASTRID_WORKSPACE = ASTRID_REPO / "capsules/spectral-bridge/workspace"
DEFAULT_INTROSPECTIONS_DIR = ASTRID_WORKSPACE / "introspections"
DEFAULT_STATE_DIR = ASTRID_WORKSPACE / "diagnostics/introspection_addressing_v1"
DEFAULT_CLOSURE_CARD_DIR = DEFAULT_STATE_DIR / "closure_cards"
DEFAULT_AGENCY_CORRIDOR_STATE_DIR = ASTRID_WORKSPACE / "diagnostics/agency_corridor_v1"
DEFAULT_AGENCY_CORRIDOR_V2_STATE_DIR = ASTRID_WORKSPACE / "diagnostics/agency_corridor_v2"
ASTRID_INBOX = ASTRID_WORKSPACE / "inbox"
MINIME_INBOX = Path("/Users/v/other/minime/workspace/inbox")
DEFAULT_CUTOFF = "introspection_astrid_llm_1783325217.txt"
FEEDBACK_LEDGER = ASTRID_REPO / "docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md"
CHANGELOG = ASTRID_REPO / "CHANGELOG.md"

SCHEMA = "introspection_addressing_v1"
SCHEMA_VERSION = 1
TIMESTAMP_RE = re.compile(r"_(\d{10})\.txt$")
HEADER_RE = re.compile(r"^([A-Za-z][A-Za-z ]+):\s*(.*)$")
SECTION_NAMES = ("Observed", "Likely Snags", "One Test Each", "Suggested Next")
TERMINAL_STATUSES = {
    "addressed_change",
    "addressed_no_action",
    "addressed_duplicate",
    "superseded_by_later",
}
NONTERMINAL_STATUSES = {
    "unread",
    "read_needs_claims",
    "triaged_pending_action",
    "triaged_watch",
    "blocked_needs_steward",
}
EVIDENCE_KINDS = {
    "changelog",
    "ledger",
    "code",
    "test",
    "steward_note",
    "no_action",
}
WORK_EVIDENCE_KINDS = EVIDENCE_KINDS | {
    "diagnostic",
    "closure_card",
    "introspection",
    "correspondence",
    "authority_gate",
    "runtime_check",
}
WORK_STATUSES = {
    "ready_for_implementation",
    "verified_existing",
    "needs_sandbox",
    "needs_steward_grant",
    "needs_operator_approval",
    "implemented_awaiting_felt_response",
    "closed_felt_confirmed",
    "closed_no_action",
    "superseded",
}
WORK_TERMINAL_STATUSES = {
    "closed_felt_confirmed",
    "closed_no_action",
    "superseded",
}
POST_CHANGE_RESPONSE_STATUSES = {
    "not_requested",
    "awaiting",
    "improved_named",
    "still_friction",
    "contradicted",
    "no_response",
}
AGENCY_TIER_LABELS = {
    0: "felt_report_or_no_action_provenance",
    1: "self_activated_read_only_local_research",
    2: "being_authored_language_or_correspondence_artifact",
    3: "sandbox_replay_or_simulation_no_live_mutation",
    4: "steward_gated_consequence_authority",
    5: "mike_operator_live_substrate_or_control_approval",
}
AGENCY_TIER_REQUIRED_EVIDENCE = {
    0: "claim disposition or no-action rationale",
    1: "read-only diagnostic, self-study, dossier, memory, or local research evidence",
    2: "language artifact, correspondence, witness, trace, steward note, or ledger evidence",
    3: "sandbox, replay, simulation, test, or bounded experiment report",
    4: "authority-gate or steward-grant evidence before any consequence-bearing action",
    5: "Mike/operator approval plus implementation/test evidence before any live substrate or control-facing change",
}
AGENCY_TIER_DEFAULT_STATUS = {
    0: "ready_for_implementation",
    1: "ready_for_implementation",
    2: "ready_for_implementation",
    3: "needs_sandbox",
    4: "needs_steward_grant",
    5: "needs_operator_approval",
}
AGENCY_TIER_CORRECTION_STATUSES = {
    "closed_no_action",
    "superseded",
    "verified_existing",
}
AUTHORITY_BOUNDARY = (
    "review tracking only; no introspection-suggested runtime change, deploy, "
    "restart, staging, git add, or commit"
)
AGENCY_BOUNDARY = (
    "agency ladder is triage/evidence infrastructure only; tier suggestions do "
    "not auto-approve, auto-close, mutate runtime, deploy, stage, git add, or commit"
)
AGENCY_CONTINUES_DURING_AUTHORITY_WAIT = (
    "authority wait restricts live mutation only; the being may keep producing "
    "felt reports, replay evidence, proposal revisions, correspondence, and "
    "right-to-ignore responses"
)


def now_s() -> float:
    return time.time()


def iso(ts: float | None = None) -> str:
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(now_s() if ts is None else ts))


def atomic_write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_name(f".{path.name}.{os.getpid()}.{time.time_ns()}.tmp")
    with tmp.open("w", encoding="utf-8") as handle:
        handle.write(text)
        handle.flush()
        os.fsync(handle.fileno())
    os.replace(tmp, path)
    directory_fd = os.open(path.parent, os.O_RDONLY)
    try:
        os.fsync(directory_fd)
    finally:
        os.close(directory_fd)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def sha256_text(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8", errors="replace")).hexdigest()


def bounded_text(text: str, *, limit: int = 600) -> str:
    collapsed = " ".join(text.split())
    if len(collapsed) <= limit:
        return collapsed
    return collapsed[: max(0, limit - 3)].rstrip() + "..."


def timestamp_from_name(name: str) -> int | None:
    match = TIMESTAMP_RE.search(name)
    return int(match.group(1)) if match else None


def latest_canonical_introspection_filename(introspections_dir: Path) -> str:
    candidates = []
    for path in introspections_dir.glob("introspection_*.txt"):
        ts = timestamp_from_name(path.name)
        if ts is not None:
            candidates.append((ts, path.name))
    if not candidates:
        raise ValueError(f"no canonical introspections found in {introspections_dir}")
    return max(candidates)[1]


def resolve_cutoff(cutoff: str, introspections_dir: Path) -> str:
    if cutoff.strip().lower() in {"latest", "newest", "auto"}:
        return latest_canonical_introspection_filename(introspections_dir)
    return cutoff


def cutoff_timestamp(cutoff: str, introspections_dir: Path) -> int:
    candidate = Path(cutoff).name
    ts = timestamp_from_name(candidate)
    if ts is not None:
        return ts
    if cutoff.isdigit():
        return int(cutoff)
    path = introspections_dir / candidate
    ts = timestamp_from_name(path.name)
    if ts is not None and path.exists():
        return ts
    raise ValueError(f"could not derive timestamp from cutoff {cutoff!r}")


def stable_id(path: Path) -> str:
    return path.stem


def artifact_kind(path: Path) -> str:
    name = path.name
    if name.startswith("introspection_"):
        return "canonical_introspection"
    if name.startswith("thin_introspection_output_"):
        return "thin_introspection_output"
    return "other_timestamped_text"


def source_family_from_id(introspection_id: str) -> str:
    base = TIMESTAMP_RE.sub("", f"{introspection_id}.txt")
    if base.startswith("introspection_"):
        base = base[len("introspection_") :]
    elif base.startswith("thin_introspection_output_"):
        base = base[len("thin_introspection_output_") :]
    return base or "unknown"


def being_from_source(record: dict[str, Any]) -> str:
    text = " ".join(
        str(record.get(key) or "")
        for key in ("introspection_id", "source_family", "filename")
    ).lower()
    header = record.get("header") if isinstance(record.get("header"), dict) else {}
    text += " " + str(header.get("source") or "").lower()
    if "minime" in text:
        return "minime"
    if "astrid" in text or "proposal" in text:
        return "astrid"
    return "unknown"


def work_item_id_for(introspection_id: str, claim_id: str) -> str:
    digest = hashlib.sha256(f"{introspection_id}:{claim_id}".encode("utf-8")).hexdigest()
    return f"wi_{digest[:16]}"


def stable_uuid(*parts: object) -> str:
    joined = "\0".join(str(part or "") for part in parts)
    return str(uuid.uuid5(uuid.NAMESPACE_URL, joined))


def _contains_any(text: str, terms: tuple[str, ...]) -> bool:
    return any(term in text for term in terms)


LIVE_CONTROL_NOUNS = (
    "pressure",
    "fill",
    "pi",
    "pi controller",
    "controller",
    "control",
    "substrate",
    "sensor cadence",
    "sensory cadence",
    "camera",
    "mic",
    "fallback sampler",
    "fallback contract",
    "runtime mutation",
    "runtime control",
    "peer mutation",
    "bridge protocol",
    "protocol/abi",
    "abi",
    "eigenpacket contract",
    "exploration_noise",
    "regulation_strength",
    "mode_packing",
    "spectral leakage",
    "sensory-bus porosity",
    "sensory bus porosity",
    "semantic_trickle",
    "semantic weighting",
    "semantic priority",
    "semantic transport",
    "reservoir priority",
    "reservoir-priority",
    "live transport",
)
LIVE_CHANGE_VERBS = (
    "add",
    "adding",
    "adjust",
    "alter",
    "change",
    "changes",
    "changing",
    "decrease",
    "reduced",
    "reduce",
    "grant",
    "increase",
    "increased",
    "modify",
    "mutate",
    "mutating",
    "open",
    "priority",
    "replace",
    "send",
    "set",
    "shift",
    "shifting",
    "tune",
    "wire",
    "wiring",
    "influence",
    "influencing",
)


def _strip_negated_authority_boundaries(text: str) -> str:
    noun_pattern = "|".join(re.escape(noun) for noun in sorted(LIVE_CONTROL_NOUNS, key=len, reverse=True))
    patterns = (
        rf"\bwithout\b[^.;\n]{{0,140}}\b(?:{noun_pattern})\b",
        rf"\bno\b[^.;\n]{{0,140}}\b(?:{noun_pattern})\b",
        rf"\bnot\b[^.;\n]{{0,140}}\b(?:{noun_pattern})\b",
        rf"\bbefore\b[^.;\n]{{0,80}}\b(?:add|adding|change|changing|mutate|mutating|wire|wiring)[^.;\n]{{0,140}}\b(?:{noun_pattern})\b",
        rf"\bbefore\b[^.;\n]{{0,140}}\b(?:{noun_pattern})\b[^.;\n]{{0,80}}\b(?:change|changes|changing|experiment|influence|mutation|mutate|mutating|tune|wire|wiring)\b",
    )
    stripped = text
    for pattern in patterns:
        stripped = re.sub(pattern, " ", stripped)
    return stripped


def _window_has_live_change_verb(window: str) -> bool:
    for verb in LIVE_CHANGE_VERBS:
        for match in re.finditer(rf"\b{re.escape(verb)}\b", window):
            prefix = window[max(0, match.start() - 112) : match.start()]
            if re.search(r"\b(without|no|not|never)\b[^.;\n]{0,32}$", prefix):
                continue
            if re.search(
                r"\b(?:must|does|do|will|can|should)\s+not\b[^.;\n]{0,96}$",
                prefix,
            ):
                continue
            if re.search(r"\bbefore\b[^.;\n]{0,80}$", prefix):
                continue
            return True
    return False


def _has_live_control_change(text: str) -> bool:
    lower = _strip_negated_authority_boundaries(text.lower())
    for noun in LIVE_CONTROL_NOUNS:
        for match in re.finditer(rf"\b{re.escape(noun)}\b", lower):
            start = max(0, match.start() - 90)
            end = min(len(lower), match.end() + 90)
            window = lower[start:end]
            if _window_has_live_change_verb(window):
                return True
    return False


def _is_explicit_verification_claim(text: str) -> bool:
    """Keep epistemic checks from becoming live-change imperatives.

    Words such as ``changes`` and ``shift`` often describe behavior inside a
    verification claim. They should not outrank the claim's leading epistemic
    verb unless the same claim actually asks to apply or execute a live change.
    Explicit Tier 4/5 dispositions are handled before this guard.
    """

    lower = text.strip().lower()
    if not re.match(
        r"^(?:verify|observe|record|preserve|confirm|inspect|test)\b",
        lower,
    ):
        return False
    return not re.search(
        r"\b(?:apply|deploy|execute|grant|mutate|retune|send|wire|write)\b"
        r"|\blive\b[^.;\n]{0,64}\b(?:change|control|mutation|retune|trial|tuning|write)\b",
        lower,
    )


def agency_tier_for_claim(text: str) -> int:
    lower = text.lower()
    # A claim may already carry an explicit steward disposition. Preserve that
    # authority boundary even when its surface nouns are unfamiliar to the
    # heuristic classifier.
    if re.search(
        r"\b(?:requires?|remains?|keep|keeps|kept|is|are)\s+(?:at\s+)?tier[\s_-]*5\b|\btier[\s_-]*5\s+(?:operator\s+)?approval\b|\btier[\s_-]*5\b[^.;\n]{0,64}\b(?:live|operator|control|protocol|runtime|substrate|behavior)\b[^.;\n]{0,32}\b(?:change|work|authority)\b",
        lower,
    ):
        return 5
    if re.search(
        r"\b(?:requires?|remains?|keep|keeps|kept|is|are)\s+(?:at\s+)?tier[\s_-]*4\b|\btier[\s_-]*4\s+(?:steward\s+)?approval\b",
        lower,
    ):
        return 4
    if _is_explicit_verification_claim(lower):
        return 1
    if _contains_any(
        lower,
        (
            "semantic_microdose",
            "mode_release_microdose",
            "authority budget",
            "authority_budget",
            "authority_budget_max_sends",
            "local_research_max_actions",
            "loop_research_max_actions",
            "authority-gate",
            "authority gate",
            "consequence authority",
            "consequence budget",
            "steward grant",
        ),
    ):
        return 4
    if _has_live_control_change(lower):
        return 5
    if _contains_any(
        lower,
        (
            "sandbox",
            "stress test",
            "decay test",
            "replay",
            "simulation",
            "simulated",
            "experiment",
            "simulate",
            "temporarily decrease",
            "experiment report",
            "bounded experiment",
            "probe",
        ),
    ):
        return 3
    if _contains_any(
        lower,
        (
            "correspondence",
            "witness",
            "transition",
            "closure card",
            "steward note",
            "letter",
            "replyable",
            "trace",
            "language artifact",
            "phase card",
            "relational card",
            "presence receipt",
        ),
    ):
        return 2
    if _contains_any(
        lower,
        (
            "read-only",
            "read only",
            "audit",
            "inspect",
            "self-study",
            "self study",
            "dossier",
            "memory",
            "local research",
            "research",
            "diagnostic",
        ),
    ):
        return 1
    return 0


def agency_route_for_tier(tier: int) -> str:
    return {
        0: "record_felt_report",
        1: "self_activated_read_only_research",
        2: "language_or_correspondence_artifact",
        3: "sandbox_replay_or_simulation",
        4: "steward_authority_grant",
        5: "mike_operator_live_change_approval",
    }.get(tier, "record_felt_report")


def live_control_claim(text: str) -> bool:
    return _has_live_control_change(text)


def authority_class_for_work_item(item: dict[str, Any]) -> str | None:
    tier = int(item.get("agency_tier") or 0)
    if tier >= 5:
        return "mike_operator_live_substrate"
    if tier == 4:
        return "steward_gated_consequence"
    return None


def authority_gate_state_for_work_item(item: dict[str, Any]) -> str:
    tier = int(item.get("agency_tier") or 0)
    status = str(item.get("status") or "")
    if status in WORK_TERMINAL_STATUSES or status in {"verified_existing", "superseded"}:
        return "superseded" if status == "superseded" else "evidence_only"
    if tier >= 4:
        return "proposal_needed"
    return "evidence_only"


def authority_boundary_packet_for_work_item(item: dict[str, Any]) -> dict[str, Any] | None:
    authority_class = authority_class_for_work_item(item)
    if authority_class is None:
        return None

    work_item_id = str(item.get("work_item_id") or "")
    tier = int(item.get("agency_tier") or 0)
    who_can_change_it = "Mike/operator" if tier >= 5 else "steward/operator"
    evidence_refs = [
        str(value)
        for value in (
            item.get("source_path"),
            item.get("source_introspection_id"),
            item.get("source_filename"),
            item.get("claim_id"),
            work_item_id,
        )
        if value
    ]
    return {
        "boundary_id": stable_uuid("introspection_work_item_authority_boundary", work_item_id),
        "schema_version": 1,
        "source": "introspection_addressing_audit_v1",
        "surface": str(item.get("source_family") or "introspection_addressing"),
        "action": str(item.get("route") or agency_route_for_tier(tier)),
        "resource": work_item_id,
        "authority_class": authority_class,
        "gate_state": authority_gate_state_for_work_item(item),
        "felt_report_anchor": bounded_text(str(item.get("claim_summary") or ""), limit=420),
        "proposed_change": bounded_text(
            str(item.get("suggested_next") or item.get("evidence_required") or item.get("title") or ""),
            limit=500,
        ),
        "evidence_refs": evidence_refs,
        "replay_candidate": {
            "adapter": "sandbox_trial_queue_v1" if tier >= 5 else "manual_authority_review_v1",
            "replay_query": (
                "python3 scripts/sandbox_trial_queue.py generate --json --write"
                if tier >= 5
                else "python3 scripts/introspection_addressing_audit.py report --json"
            ),
            "runnable": False,
            "authority": "evidence_or_proposal_only_not_live_control",
        },
        "success_metrics": [
            bounded_text(str(item.get("evidence_required") or ""), limit=240),
            "separate explicit approval receipt remains required before any consequence or live mutation",
        ],
        "abort_criteria": [
            "missing first-class authority-boundary packet",
            "missing explicit steward/operator approval receipt",
            "tests, replay, rollback plan, or health checks are absent",
        ],
        "who_can_change_it": who_can_change_it,
        "how_to_test_it": (
            "Review the packet, link sandbox/replay evidence, and require a separate explicit "
            "approval receipt before any runtime/control-facing execution path."
        ),
        "right_to_ignore": True,
        "live_eligible_now": False,
        "auto_approved": False,
    }


def work_item_evidence_refs(item: dict[str, Any]) -> list[str]:
    return [
        str(value)
        for value in (
            item.get("source_path"),
            item.get("source_introspection_id"),
            item.get("source_filename"),
            item.get("claim_id"),
            item.get("work_item_id"),
        )
        if value
    ]


def work_item_delta_refs_v2(item: dict[str, Any]) -> list[dict[str, Any]]:
    work_item_id = str(item.get("work_item_id") or "")
    authority_class = authority_class_for_work_item(item) or "read_only"
    surface = str(item.get("source_family") or "introspection_addressing")
    kind = "live_control_gate" if authority_class == "mike_operator_live_substrate" else "authority_gate"
    hash_payload = {
        "work_item_id": work_item_id,
        "surface": surface,
        "kind": kind,
        "refs": work_item_evidence_refs(item),
    }
    return [
        {
            "delta_id": "delta_" + sha256_text(f"authority_delta_ref_v2:{work_item_id}:{kind}")[:16],
            "delta_hash": sha256_text(json.dumps(hash_payload, sort_keys=True, ensure_ascii=True)),
            "surface": surface,
            "kind": kind,
            "lane": str(item.get("route") or agency_route_for_tier(int(item.get("agency_tier") or 0))),
        }
    ]


def work_item_rollout_abort_contract_v2(item: dict[str, Any]) -> dict[str, Any]:
    tier = int(item.get("agency_tier") or 0)
    if tier >= 5:
        canary_plan = "proposal and sandbox evidence only until Mike/operator scoped approval exists"
        rollback_path = "use service-specific rollback/restart path only after explicit approval; no runtime mutation from audit tooling"
    else:
        canary_plan = "steward review packet only; no consequence-bearing execution from audit tooling"
        rollback_path = "supersede or close work item; no live rollback needed because no live mutation occurs here"
    return {
        "canary_plan": canary_plan,
        "health_checks": [
            "verify work item remains blocked_by approval/grant when tier is 4 or 5",
            "verify evidence links include bounded diagnostics or replay before execution",
            "verify post-change response status is awaiting until response or waiver is recorded",
        ],
        "rollback_path": rollback_path,
        "abort_criteria": [
            "missing explicit steward/operator approval receipt",
            "missing replay result or explicit waiver",
            "missing post-change being response path",
        ],
        "post_change_response_required": True,
    }


def work_item_redaction_profile_v2(item: dict[str, Any]) -> dict[str, Any]:
    claim_summary = str(item.get("claim_summary") or item.get("title") or "")
    return {
        "public_summary": bounded_text(claim_summary, limit=260) or "bounded authority work item",
        "private_ref": str(item.get("source_filename") or item.get("source_introspection_id") or item.get("work_item_id")),
        "content_hash": sha256_text(claim_summary) if claim_summary else None,
        "retention_policy": "bounded_public_summaries_plus_private_refs_and_hashes",
    }


def work_item_has_replay_evidence(item: dict[str, Any]) -> bool:
    evidence = item.get("evidence_links") if isinstance(item.get("evidence_links"), list) else []
    replay_kinds = {"diagnostic", "runtime_check", "test", "authority_gate"}
    return any(isinstance(row, dict) and str(row.get("kind") or "") in replay_kinds for row in evidence)


def work_item_lifecycle_state_v2(item: dict[str, Any]) -> str:
    status = str(item.get("status") or "")
    tier = int(item.get("agency_tier") or 0)
    if status in WORK_TERMINAL_STATUSES:
        return "closed"
    if status == "superseded":
        return "superseded"
    if tier < 4:
        return "evidence_only"
    if not work_item_has_replay_evidence(item):
        return "replay_needed" if tier >= 5 else "proposal_needed"
    if tier >= 5 and status == "needs_operator_approval":
        return "operator_approval_wait"
    if tier == 4 and status == "needs_steward_grant":
        return "authority_boundary_wait"
    if status == "implemented_awaiting_felt_response":
        return "executed_awaiting_response"
    return "approved_manual_only"


def agency_status_overlay(status: str, *, tier: int | None = None) -> dict[str, Any]:
    overlay: dict[str, Any] = {}
    if status == "blocked_needs_steward" or status in {"needs_steward_grant", "needs_operator_approval"}:
        wait_state = "operator_approval_wait" if status == "needs_operator_approval" or tier == 5 else "authority_boundary_wait"
        overlay.update(
            {
                "agency_continues": True,
                "authority_boundary_wait": True,
                "agency_preserving_status": wait_state,
                "compatibility_status": status,
                "live_authority_granted": False,
                "live_eligible_now": False,
                "agency_continuation_note": AGENCY_CONTINUES_DURING_AUTHORITY_WAIT,
                "available_agency_routes": [
                    "felt_report_refinement",
                    "sandbox_or_replay_evidence",
                    "proposal_revision",
                    "peer_correspondence",
                    "right_to_ignore_response",
                ],
            }
        )
    return overlay


def with_agency_status_overlay(item: dict[str, Any]) -> dict[str, Any]:
    copy = dict(item)
    status = str(copy.get("status") or "")
    tier = int(copy.get("agency_tier") or 0) if str(copy.get("agency_tier") or "").isdigit() else None
    copy.update(agency_status_overlay(status, tier=tier))
    return copy


def work_item_lifecycle_receipts_v2(item: dict[str, Any], boundary_id: str) -> list[dict[str, Any]]:
    receipts: list[dict[str, Any]] = []
    work_item_id = str(item.get("work_item_id") or "")
    if work_item_has_replay_evidence(item):
        receipts.append(
            {
                "receipt_id": stable_uuid("introspection_work_item_lifecycle_receipt_v2", work_item_id, "replay_result"),
                "boundary_id": boundary_id,
                "kind": "replay_result",
                "issued_by": "introspection_addressing_audit_v2",
                "issued_at": iso(),
                "packet_hash": None,
                "receipt_hash_refs": [],
                "bounded_summary": "bounded diagnostic/replay evidence is linked; audit tooling did not mutate live runtime",
                "evidence_refs": work_item_evidence_refs(item),
                "scoped_approval": None,
                "replay_result": {
                    "replay_id": stable_uuid("introspection_work_item_replay_result_v2", work_item_id),
                    "adapter": "linked_evidence_review",
                    "classification": "inconclusive",
                    "input_refs": work_item_evidence_refs(item),
                    "pre_observations": {},
                    "post_observations": {},
                    "confidence": 0.45,
                    "failure_modes": [],
                    "evidence_refs": [
                        str(row.get("target"))
                        for row in item.get("evidence_links", [])
                        if isinstance(row, dict) and row.get("target")
                    ],
                    "bounded_summary": "linked evidence is present and still requires explicit approval before live work",
                    "occurred_at": iso(),
                },
                "right_to_ignore": True,
            }
        )
    response_status = str(item.get("post_change_response_status") or "")
    if response_status in {"improved_named", "still_friction", "contradicted"}:
        receipts.append(
            {
                "receipt_id": stable_uuid("introspection_work_item_lifecycle_receipt_v2", work_item_id, "post_change_being_response"),
                "boundary_id": boundary_id,
                "kind": "post_change_being_response",
                "issued_by": str(item.get("being") or "being_response_surface"),
                "issued_at": iso(),
                "packet_hash": None,
                "receipt_hash_refs": [],
                "bounded_summary": f"post-change response recorded as {response_status}",
                "evidence_refs": work_item_evidence_refs(item),
                "scoped_approval": None,
                "replay_result": None,
                "right_to_ignore": True,
            }
        )
    elif response_status in {"no_response", "not_requested"} and item.get("closure_cards"):
        receipt_kind = (
            "review_opportunity_expired"
            if response_status == "no_response"
            else "review_not_requested"
        )
        receipts.append(
            {
                "receipt_id": stable_uuid(
                    "introspection_work_item_lifecycle_receipt_v2",
                    work_item_id,
                    receipt_kind,
                ),
                "boundary_id": boundary_id,
                "kind": receipt_kind,
                "issued_by": "introspection_addressing_audit_v2",
                "issued_at": iso(),
                "packet_hash": None,
                "receipt_hash_refs": [],
                "bounded_summary": (
                    f"post-change review opportunity recorded neutrally as {response_status}; "
                    "this is not affirmation, approval, waiver, or felt closure"
                ),
                "evidence_refs": work_item_evidence_refs(item),
                "scoped_approval": None,
                "replay_result": None,
                "right_to_ignore": True,
            }
        )
    return receipts


def authority_boundary_packet_v2_for_work_item(item: dict[str, Any]) -> dict[str, Any] | None:
    authority_class = authority_class_for_work_item(item)
    if authority_class is None:
        return None
    work_item_id = str(item.get("work_item_id") or "")
    tier = int(item.get("agency_tier") or 0)
    boundary_id = stable_uuid("introspection_work_item_authority_boundary_v2", work_item_id)
    receipts = work_item_lifecycle_receipts_v2(item, boundary_id)
    return {
        "boundary_id": boundary_id,
        "schema_version": 2,
        "source": "introspection_addressing_audit_v2",
        "surface": str(item.get("source_family") or "introspection_addressing"),
        "action": str(item.get("route") or agency_route_for_tier(tier)),
        "resource": work_item_id,
        "authority_class": authority_class,
        "lifecycle_state": work_item_lifecycle_state_v2(item),
        "felt_report_anchor": bounded_text(str(item.get("claim_summary") or ""), limit=420),
        "proposed_change": bounded_text(
            str(item.get("suggested_next") or item.get("evidence_required") or item.get("title") or ""),
            limit=500,
        ),
        "evidence_refs": work_item_evidence_refs(item),
        "delta_refs": work_item_delta_refs_v2(item),
        "replay_candidate": {
            "adapter": "sandbox_trial_queue_v2" if tier >= 5 else "manual_authority_review_v2",
            "replay_query": (
                "python3 scripts/sandbox_trial_queue.py generate --json --write"
                if tier >= 5
                else "python3 scripts/introspection_addressing_audit.py report --json"
            ),
            "runnable": False,
            "authority": "evidence_or_proposal_only_not_live_control",
        },
        "replay_results": [
            receipt["replay_result"]
            for receipt in receipts
            if receipt.get("kind") == "replay_result" and isinstance(receipt.get("replay_result"), dict)
        ],
        "scoped_approval": None,
        "rollout_abort_contract": work_item_rollout_abort_contract_v2(item),
        "redaction_profile": work_item_redaction_profile_v2(item),
        "lifecycle_receipts": receipts,
        "success_metrics": [
            bounded_text(str(item.get("evidence_required") or ""), limit=240),
            "scoped approval receipt remains separate from boundary evidence",
        ],
        "abort_criteria": [
            "missing replay result or explicit waiver",
            "missing explicit scoped approval receipt",
            "missing rollout/abort or post-change response path",
        ],
        "who_can_change_it": "Mike/operator" if tier >= 5 else "steward/operator",
        "how_to_test_it": (
            "Review V2 packet fields, generate/link sandbox replay evidence, record scoped approval "
            "outside this audit, and require post-change response or explicit waiver before closure."
        ),
        "right_to_ignore": True,
        "live_eligible_now": False,
        "auto_approved": False,
    }


def _empty_work_item_fields() -> dict[str, Any]:
    return {
        "status": "ready_for_implementation",
        "evidence_links": [],
        "status_events": [],
        "agency_tier_requests": [],
        "agency_tier_corrections": [],
        "post_change_responses": [],
        "closure_cards": [],
        "post_change_response_status": "not_requested",
        "updated_at": None,
        "blocked_by": None,
        "auto_approved": False,
        "fully_addressed_effect": False,
        "authority_boundary_packet": None,
        "authority_boundary_packet_v2": None,
    }


def header_metadata(text: str) -> dict[str, str]:
    metadata: dict[str, str] = {}
    for line in text.splitlines()[:40]:
        match = HEADER_RE.match(line.strip())
        if not match:
            continue
        key = match.group(1).strip().lower().replace(" ", "_")
        metadata[key] = match.group(2).strip()
    return metadata


def section_presence(text: str) -> dict[str, bool]:
    return {
        name.lower().replace(" ", "_"): bool(re.search(rf"(?m)^{re.escape(name)}:\s*$", text))
        for name in SECTION_NAMES
    }


def read_candidate_sources() -> dict[str, str]:
    sources: dict[str, str] = {}
    for label, path in (("changelog", CHANGELOG), ("feedback_ledger", FEEDBACK_LEDGER)):
        try:
            sources[label] = path.read_text(encoding="utf-8", errors="ignore")
        except OSError:
            sources[label] = ""
    return sources


def candidate_evidence_for(record: dict[str, Any], sources: dict[str, str]) -> list[dict[str, str]]:
    needles = [
        str(record.get("filename") or ""),
        str(record.get("introspection_id") or ""),
        str(record.get("timestamp") or ""),
    ]
    evidence: list[dict[str, str]] = []
    for label, text in sources.items():
        if not text:
            continue
        matched = next((needle for needle in needles if needle and needle in text), None)
        if matched:
            target = str(CHANGELOG if label == "changelog" else FEEDBACK_LEDGER)
            evidence.append(
                {
                    "kind": label,
                    "target": target,
                    "match": matched,
                    "status": "candidate_evidence_not_closure",
                }
            )
    return evidence


def artifact_record(path: Path, introspections_dir: Path, sources: dict[str, str]) -> dict[str, Any]:
    text = path.read_text(encoding="utf-8", errors="replace")
    try:
        mtime = path.stat().st_mtime
        size = path.stat().st_size
    except OSError:
        mtime = 0.0
        size = 0
    introspection_id = stable_id(path)
    record = {
        "introspection_id": introspection_id,
        "filename": path.name,
        "path": str(path),
        "relative_path": str(path.relative_to(introspections_dir.parent)),
        "present_on_disk": True,
        "timestamp": timestamp_from_name(path.name),
        "artifact_kind": artifact_kind(path),
        "source_family": source_family_from_id(introspection_id),
        "sha256": sha256_file(path),
        "size_bytes": size,
        "mtime": mtime,
        "mtime_iso": iso(mtime) if mtime else None,
        "header": header_metadata(text),
        "sections_present": section_presence(text),
        "excerpt": bounded_text(text, limit=700),
    }
    record["candidate_evidence"] = candidate_evidence_for(record, sources)
    return record


def inventory_records(
    introspections_dir: Path,
    cutoff: str,
    *,
    candidate_sources: dict[str, str] | None = None,
) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    cutoff = resolve_cutoff(cutoff, introspections_dir)
    cutoff_ts = cutoff_timestamp(cutoff, introspections_dir)
    sources = candidate_sources if candidate_sources is not None else read_candidate_sources()
    records: list[dict[str, Any]] = []
    for path in sorted(introspections_dir.glob("*.txt")):
        ts = timestamp_from_name(path.name)
        if ts is None or ts > cutoff_ts:
            continue
        records.append(artifact_record(path, introspections_dir, sources))
    records.sort(
        key=lambda item: (
            item["artifact_kind"] != "canonical_introspection",
            -int(item.get("timestamp") or 0),
            str(item.get("filename") or ""),
        )
    )
    return records, {
        "cutoff": cutoff,
        "cutoff_timestamp": cutoff_ts,
        "cutoff_indexed": any(r["filename"] == Path(cutoff).name for r in records),
    }


def event_path(state_dir: Path) -> Path:
    return state_dir / "events.jsonl"


def status_path(state_dir: Path) -> Path:
    return state_dir / "status.json"


def queue_path(state_dir: Path) -> Path:
    return state_dir / "queue.md"


def append_events(state_dir: Path, events: list[dict[str, Any]]) -> None:
    if not events:
        return
    if v2_active_for_state(state_dir):
        append_domain_events(state_dir, "addressing", events)
        return
    path = event_path(state_dir)
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a", encoding="utf-8") as fh:
        for event in events:
            normalize_artifact_authority_tree(event)
            fh.write(json.dumps(event, sort_keys=True, ensure_ascii=False) + "\n")


def read_events(state_dir: Path) -> tuple[list[dict[str, Any]], int]:
    if v2_active_for_state(state_dir):
        return read_domain_events(state_dir, "addressing")
    path = event_path(state_dir)
    if not path.exists():
        return [], 0
    events: list[dict[str, Any]] = []
    corrupt = 0
    for line in path.read_text(encoding="utf-8", errors="ignore").splitlines():
        if not line.strip():
            continue
        try:
            payload = json.loads(line)
        except json.JSONDecodeError:
            corrupt += 1
            continue
        if isinstance(payload, dict):
            events.append(payload)
    return events, corrupt


def _empty_review_fields() -> dict[str, Any]:
    return {
        "full_read": False,
        "full_read_count": 0,
        "read_events": [],
        "claims": {},
        "close_events": [],
        "requested_close_status": None,
        "requested_close_rationale": None,
        "fully_addressed": False,
        "status": "unread",
        "proof_missing_claims": [],
    }


def refresh_work_item_authority_packets(item: dict[str, Any]) -> None:
    tier = int(item.get("agency_tier") or 0)
    if tier >= 4:
        item["authority_boundary_packet"] = authority_boundary_packet_for_work_item(item)
        item["authority_boundary_packet_v2"] = authority_boundary_packet_v2_for_work_item(item)
    else:
        item["authority_boundary_packet"] = None
        item["authority_boundary_packet_v2"] = None
    normalize_artifact_authority_tree(item)


def _merge_artifact(existing: dict[str, Any] | None, artifact: dict[str, Any]) -> dict[str, Any]:
    merged = dict(artifact)
    if existing:
        for key, value in existing.items():
            if key not in artifact:
                merged[key] = value
        for key in _empty_review_fields():
            if key in existing:
                merged[key] = existing[key]
    if artifact.get("present_on_disk") is True:
        merged.pop("inventory_absent_at", None)
        merged.pop("inventory_absent_reason", None)
    for key, value in _empty_review_fields().items():
        merged.setdefault(key, value.copy() if isinstance(value, (dict, list)) else value)
    return merged


def _merge_work_item(existing: dict[str, Any] | None, item: dict[str, Any]) -> dict[str, Any]:
    merged = dict(existing or {})
    merged.update(item)
    for key, value in _empty_work_item_fields().items():
        merged.setdefault(key, value.copy() if isinstance(value, (dict, list)) else value)
    tier = int(merged.get("agency_tier") or 0)
    merged["agency_tier"] = tier
    merged.setdefault("agency_tier_label", AGENCY_TIER_LABELS.get(tier, AGENCY_TIER_LABELS[0]))
    merged.setdefault("route", agency_route_for_tier(tier))
    merged.setdefault("evidence_required", AGENCY_TIER_REQUIRED_EVIDENCE.get(tier, AGENCY_TIER_REQUIRED_EVIDENCE[0]))
    merged.setdefault("status", AGENCY_TIER_DEFAULT_STATUS.get(tier, "ready_for_implementation"))
    if tier >= 4 and (
        not isinstance(merged.get("authority_boundary_packet"), dict)
        or not isinstance(merged.get("authority_boundary_packet_v2"), dict)
    ):
        refresh_work_item_authority_packets(merged)
    return merged


def _claim_has_proof(claim: dict[str, Any]) -> bool:
    disposition = str(claim.get("disposition") or "")
    evidence = claim.get("evidence") if isinstance(claim.get("evidence"), list) else []
    rationale = str(claim.get("rationale") or "").strip()
    if disposition == "addressed_change":
        return any(str(item.get("kind") or "") != "no_action" for item in evidence if isinstance(item, dict))
    if disposition in {"addressed_no_action", "addressed_duplicate", "superseded_by_later"}:
        return bool(rationale or evidence)
    return False


def _derive_status(record: dict[str, Any]) -> None:
    claims = record.get("claims") if isinstance(record.get("claims"), dict) else {}
    if not record.get("full_read"):
        record["status"] = "unread"
        record["fully_addressed"] = False
        record["proof_missing_claims"] = []
        return
    if not claims:
        record["status"] = "read_needs_claims"
        record["fully_addressed"] = False
        record["proof_missing_claims"] = []
        return
    dispositions = [str(claim.get("disposition") or "") for claim in claims.values()]
    if "blocked_needs_steward" in dispositions or record.get("requested_close_status") == "blocked_needs_steward":
        record["status"] = "blocked_needs_steward"
        record["fully_addressed"] = False
        record["proof_missing_claims"] = []
        return
    if all(disposition in TERMINAL_STATUSES for disposition in dispositions):
        missing = [
            str(claim.get("claim_id") or claim_id)
            for claim_id, claim in claims.items()
            if not _claim_has_proof(claim)
        ]
        record["proof_missing_claims"] = missing
        if missing:
            record["status"] = "triaged_pending_action"
            record["fully_addressed"] = False
            return
        requested = record.get("requested_close_status")
        record["status"] = requested if requested in TERMINAL_STATUSES else dispositions[0]
        record["fully_addressed"] = True
        return
    if "triaged_watch" in dispositions:
        record["status"] = "triaged_watch"
    else:
        record["status"] = "triaged_pending_action"
    record["fully_addressed"] = False
    record["proof_missing_claims"] = []


def replay_events(state_dir: Path) -> dict[str, Any]:
    events, corrupt = read_events(state_dir)
    artifacts: dict[str, dict[str, Any]] = {}
    work_items: dict[str, dict[str, Any]] = {}
    cutoff: dict[str, Any] = {}
    for event in events:
        event_type = event.get("event_type")
        if event_type == "inventory_run":
            cutoff = dict(event.get("cutoff") or {})
        elif event_type == "inventory_artifact":
            artifact = event.get("artifact")
            if not isinstance(artifact, dict):
                continue
            artifact_id = str(artifact.get("introspection_id") or "")
            if not artifact_id:
                continue
            artifacts[artifact_id] = _merge_artifact(artifacts.get(artifact_id), artifact)
        elif event_type == "inventory_artifact_absent":
            artifact_id = str(event.get("introspection_id") or "")
            if not artifact_id or artifact_id not in artifacts:
                continue
            artifacts[artifact_id]["present_on_disk"] = False
            artifacts[artifact_id]["inventory_absent_at"] = event.get("ts")
            artifacts[artifact_id]["inventory_absent_reason"] = str(
                event.get("reason") or "missing_from_current_filesystem_snapshot"
            )
        elif event_type == "full_read":
            artifact_id = str(event.get("introspection_id") or "")
            if artifact_id not in artifacts:
                artifacts[artifact_id] = {"introspection_id": artifact_id, **_empty_review_fields()}
            record = artifacts[artifact_id]
            record["full_read"] = True
            record["full_read_count"] = int(record.get("full_read_count") or 0) + 1
            record.setdefault("read_events", []).append(
                {
                    "ts": event.get("ts"),
                    "reader": event.get("reader"),
                    "summary_sha256": event.get("summary_sha256"),
                    "summary_excerpt": event.get("summary_excerpt"),
                }
            )
            claims: dict[str, dict[str, Any]] = {}
            for claim in event.get("claims") or []:
                if not isinstance(claim, dict):
                    continue
                claim_id = str(claim.get("claim_id") or "")
                if not claim_id:
                    continue
                claims[claim_id] = {
                    "claim_id": claim_id,
                    "summary": bounded_text(str(claim.get("summary") or ""), limit=500),
                    "disposition": str(claim.get("disposition") or "triaged_pending_action"),
                    "classification": str(claim.get("classification") or "") or None,
                    "authority": str(claim.get("authority") or "") or None,
                    "evidence": [],
                    "rationale": None,
                }
            record["claims"] = claims
        elif event_type == "evidence_linked":
            artifact_id = str(event.get("introspection_id") or "")
            claim_id = str(event.get("claim_id") or "")
            if artifact_id not in artifacts or not claim_id:
                continue
            claims = artifacts[artifact_id].setdefault("claims", {})
            claim = claims.setdefault(
                claim_id,
                {
                    "claim_id": claim_id,
                    "summary": "",
                    "disposition": "triaged_pending_action",
                    "evidence": [],
                    "rationale": None,
                },
            )
            evidence = event.get("evidence")
            if isinstance(evidence, dict):
                claim.setdefault("evidence", []).append(evidence)
        elif event_type == "closed":
            artifact_id = str(event.get("introspection_id") or "")
            if artifact_id not in artifacts:
                continue
            record = artifacts[artifact_id]
            status = str(event.get("status") or "")
            rationale = str(event.get("rationale") or "").strip()
            record["requested_close_status"] = status
            record["requested_close_rationale"] = rationale
            record.setdefault("close_events", []).append(
                {"ts": event.get("ts"), "status": status, "rationale": rationale}
            )
            for claim in (record.get("claims") or {}).values():
                if claim.get("disposition") not in TERMINAL_STATUSES:
                    claim["disposition"] = status
                if rationale:
                    claim["rationale"] = rationale
        elif event_type == "work_item_created":
            item = event.get("work_item")
            if not isinstance(item, dict):
                continue
            work_item_id = str(item.get("work_item_id") or "")
            if not work_item_id:
                continue
            work_items[work_item_id] = _merge_work_item(work_items.get(work_item_id), item)
        elif event_type == "work_status_set":
            work_item_id = str(event.get("work_item_id") or "")
            if not work_item_id:
                continue
            item = _merge_work_item(work_items.get(work_item_id), {"work_item_id": work_item_id})
            status = str(event.get("status") or item.get("status") or "ready_for_implementation")
            item["status"] = status
            item["updated_at"] = event.get("ts")
            if event.get("blocked_by") is not None:
                item["blocked_by"] = event.get("blocked_by")
            item.setdefault("status_events", []).append(
                {
                    "ts": event.get("ts"),
                    "status": status,
                    "note": bounded_text(str(event.get("note") or ""), limit=500),
                    "blocked_by": event.get("blocked_by"),
                }
            )
            refresh_work_item_authority_packets(item)
            work_items[work_item_id] = item
        elif event_type == "work_evidence_linked":
            work_item_id = str(event.get("work_item_id") or "")
            if not work_item_id:
                continue
            item = _merge_work_item(work_items.get(work_item_id), {"work_item_id": work_item_id})
            evidence = event.get("evidence")
            if isinstance(evidence, dict):
                item.setdefault("evidence_links", []).append(evidence)
                item["updated_at"] = event.get("ts")
            refresh_work_item_authority_packets(item)
            work_items[work_item_id] = item
        elif event_type == "agency_tier_requested":
            work_item_id = str(event.get("work_item_id") or "")
            if not work_item_id:
                continue
            item = _merge_work_item(work_items.get(work_item_id), {"work_item_id": work_item_id})
            requested_tier = int(event.get("tier") or 0)
            request = {
                "ts": event.get("ts"),
                "tier": requested_tier,
                "tier_label": AGENCY_TIER_LABELS.get(requested_tier),
                "reason": bounded_text(str(event.get("reason") or ""), limit=600),
                "status": str(event.get("request_status") or "requested"),
            }
            item.setdefault("agency_tier_requests", []).append(request)
            if requested_tier > int(item.get("agency_tier") or 0):
                item["agency_tier"] = requested_tier
                item["agency_tier_label"] = AGENCY_TIER_LABELS.get(requested_tier)
                item["route"] = agency_route_for_tier(requested_tier)
                item["evidence_required"] = AGENCY_TIER_REQUIRED_EVIDENCE.get(requested_tier)
                item["suggested_next"] = AGENCY_TIER_REQUIRED_EVIDENCE.get(requested_tier)
                if requested_tier == 5:
                    item["blocked_by"] = item.get("blocked_by") or "operator_approval"
                elif requested_tier == 4:
                    item["blocked_by"] = item.get("blocked_by") or "steward_grant"
                if str(item.get("status") or "") == "ready_for_implementation":
                    item["status"] = AGENCY_TIER_DEFAULT_STATUS.get(
                        requested_tier, "ready_for_implementation"
                    )
                refresh_work_item_authority_packets(item)
            item["updated_at"] = event.get("ts")
            work_items[work_item_id] = item
        elif event_type == "agency_tier_corrected":
            work_item_id = str(event.get("work_item_id") or "")
            if not work_item_id:
                continue
            item = _merge_work_item(work_items.get(work_item_id), {"work_item_id": work_item_id})
            corrected_tier = int(event.get("tier") or 0)
            previous_tier = int(item.get("agency_tier") or 0)
            item.setdefault("agency_tier_corrections", []).append(
                {
                    "ts": event.get("ts"),
                    "previous_tier": previous_tier,
                    "tier": corrected_tier,
                    "tier_label": AGENCY_TIER_LABELS.get(corrected_tier),
                    "reason": bounded_text(str(event.get("reason") or ""), limit=600),
                    "status": "classification_correction",
                    "grants_approval": False,
                    "live_eligible_now": False,
                }
            )
            item["agency_tier"] = corrected_tier
            item["agency_tier_label"] = AGENCY_TIER_LABELS.get(corrected_tier)
            item["route"] = agency_route_for_tier(corrected_tier)
            item["evidence_required"] = AGENCY_TIER_REQUIRED_EVIDENCE.get(corrected_tier)
            item["suggested_next"] = AGENCY_TIER_REQUIRED_EVIDENCE.get(corrected_tier)
            if corrected_tier < 5 and item.get("blocked_by") == "operator_approval":
                item["blocked_by"] = None
            if corrected_tier < 4 and item.get("blocked_by") == "steward_grant":
                item["blocked_by"] = None
            item["auto_approved"] = False
            item["updated_at"] = event.get("ts")
            refresh_work_item_authority_packets(item)
            work_items[work_item_id] = item
        elif event_type == "closure_card_emitted":
            work_item_id = str(event.get("work_item_id") or "")
            if not work_item_id:
                continue
            item = _merge_work_item(work_items.get(work_item_id), {"work_item_id": work_item_id})
            card = event.get("closure_card")
            if isinstance(card, dict):
                item.setdefault("closure_cards", []).append(card)
                item["post_change_response_status"] = "awaiting"
                item["updated_at"] = event.get("ts")
            refresh_work_item_authority_packets(item)
            work_items[work_item_id] = item
        elif event_type == "post_change_response_recorded":
            work_item_id = str(event.get("work_item_id") or "")
            if not work_item_id:
                continue
            item = _merge_work_item(work_items.get(work_item_id), {"work_item_id": work_item_id})
            status = str(event.get("response_status") or "awaiting")
            response = {
                "ts": event.get("ts"),
                "response_status": status,
                "source": str(event.get("source") or ""),
                "note": bounded_text(str(event.get("note") or ""), limit=700),
            }
            item.setdefault("post_change_responses", []).append(response)
            item["post_change_response_status"] = status
            item["updated_at"] = event.get("ts")
            refresh_work_item_authority_packets(item)
            work_items[work_item_id] = item
    for item in work_items.values():
        artifact = artifacts.get(str(item.get("source_introspection_id") or ""))
        if not isinstance(artifact, dict):
            continue
        claim = (artifact.get("claims") or {}).get(str(item.get("claim_id") or ""))
        if not isinstance(claim, dict):
            continue
        classification = str(claim.get("classification") or "").strip()
        authority = str(claim.get("authority") or "").strip()
        if classification:
            item["claim_classification"] = classification
        if authority:
            item["claim_authority"] = authority
    for record in artifacts.values():
        _derive_status(record)
    return materialized_status(artifacts, work_items=work_items, cutoff=cutoff, corrupt_event_lines=corrupt)


def queue_items(status: dict[str, Any], *, limit: int | None = None) -> list[dict[str, Any]]:
    artifacts = status.get("artifacts") if isinstance(status.get("artifacts"), dict) else {}
    rows = [
        artifact
        for artifact in artifacts.values()
        if isinstance(artifact, dict)
        and artifact.get("present_on_disk") is not False
        and not artifact.get("fully_addressed")
    ]
    status_priority = {
        "unread": 0,
        "read_needs_claims": 1,
        "triaged_pending_action": 2,
        "triaged_watch": 3,
        "blocked_needs_steward": 4,
    }
    rows.sort(
        key=lambda item: (
            status_priority.get(str(item.get("status") or "unread"), 9),
            item.get("artifact_kind") != "canonical_introspection",
            -int(item.get("timestamp") or 0),
            str(item.get("filename") or ""),
        )
    )
    items = [
        {
            "introspection_id": row.get("introspection_id"),
            "filename": row.get("filename"),
            "timestamp": row.get("timestamp"),
            "artifact_kind": row.get("artifact_kind"),
            "source_family": row.get("source_family"),
            "status": row.get("status"),
            "agency_continues": bool(row.get("agency_continues")),
            "agency_preserving_status": row.get("agency_preserving_status"),
            "live_authority_granted": row.get("live_authority_granted"),
            "fully_addressed": bool(row.get("fully_addressed")),
            "path": row.get("path"),
            "excerpt": row.get("excerpt"),
        }
        for row in rows
    ]
    return items[:limit] if limit is not None else items


def work_queue_items(
    status: dict[str, Any],
    *,
    limit: int | None = None,
    include_terminal: bool = False,
) -> list[dict[str, Any]]:
    items = status.get("work_items") if isinstance(status.get("work_items"), dict) else {}
    rows = [item for item in items.values() if isinstance(item, dict)]
    if not include_terminal:
        rows = [item for item in rows if str(item.get("status") or "") not in WORK_TERMINAL_STATUSES]
    status_priority = {
        "needs_operator_approval": 0,
        "needs_steward_grant": 1,
        "needs_sandbox": 2,
        "ready_for_implementation": 3,
        "implemented_awaiting_felt_response": 4,
        "verified_existing": 5,
    }
    rows.sort(
        key=lambda item: (
            status_priority.get(str(item.get("status") or ""), 9),
            int(item.get("agency_tier") or 0) < 4,
            -float(item.get("updated_at") or item.get("created_at") or 0.0),
            str(item.get("work_item_id") or ""),
        )
    )
    projected = [
        {
            "work_item_id": row.get("work_item_id"),
            "source_introspection_id": row.get("source_introspection_id"),
            "claim_id": row.get("claim_id"),
            "being": row.get("being"),
            "title": row.get("title"),
            "agency_tier": row.get("agency_tier"),
            "agency_tier_label": row.get("agency_tier_label"),
            "route": row.get("route"),
            "status": row.get("status"),
            "agency_continues": bool(row.get("agency_continues")),
            "agency_preserving_status": row.get("agency_preserving_status"),
            "live_authority_granted": row.get("live_authority_granted"),
            "post_change_response_status": row.get("post_change_response_status"),
            "blocked_by": row.get("blocked_by"),
            "suggested_next": row.get("suggested_next"),
        }
        for row in rows
    ]
    return projected[:limit] if limit is not None else projected


def work_item_summary(work_items: dict[str, dict[str, Any]]) -> dict[str, Any]:
    by_status = Counter(str(item.get("status") or "unknown") for item in work_items.values())
    by_tier = Counter(str(item.get("agency_tier") or 0) for item in work_items.values())
    by_being = Counter(str(item.get("being") or "unknown") for item in work_items.values())
    active = [
        item
        for item in work_items.values()
        if str(item.get("status") or "") not in WORK_TERMINAL_STATUSES
    ]
    now = now_s()
    stale = [
        item
        for item in active
        if now - float(item.get("updated_at") or item.get("created_at") or now) > 72 * 3600
    ]
    grant_waiting = [
        item
        for item in active
        if str(item.get("status") or "") in {"needs_steward_grant", "needs_operator_approval"}
    ]
    awaiting_response = [
        item
        for item in active
        if str(item.get("post_change_response_status") or "") == "awaiting"
        or str(item.get("status") or "") == "implemented_awaiting_felt_response"
    ]
    tier_mismatches = [
        {
            "work_item_id": item.get("work_item_id"),
            "agency_tier": item.get("agency_tier"),
            "title": item.get("title"),
        }
        for item in active
        if str(item.get("status") or "") != "verified_existing"
        and str(item.get("claim_classification") or "")
        not in {"needs_sandbox", "sandbox_routed"}
        and int(item.get("agency_tier") or 0) < 5
        and live_control_claim(
            " ".join(
                str(item.get(key) or "")
                for key in ("title", "claim_summary", "suggested_next", "route")
            )
        )
    ]
    return {
        "total_work_items": len(work_items),
        "active_work_items": len(active),
        "terminal_work_items": len(work_items) - len(active),
        "by_status": dict(sorted(by_status.items())),
        "by_tier": dict(sorted(by_tier.items())),
        "by_being": dict(sorted(by_being.items())),
        "stale_work_count": len(stale),
        "grant_waiting_count": len(grant_waiting),
        "post_change_awaiting_response_count": len(awaiting_response),
        "tier_mismatch_count": len(tier_mismatches),
        "tier_mismatches": tier_mismatches[:10],
    }


def _addressing_scope_counts(items: list[dict[str, Any]]) -> dict[str, Any]:
    status_counts = Counter(str(item.get("status") or "unknown") for item in items)
    full_read = sum(1 for item in items if item.get("full_read"))
    fully_addressed = sum(1 for item in items if item.get("fully_addressed"))
    remaining = sum(1 for item in items if not item.get("fully_addressed"))
    return {
        "indexed": len(items),
        "full_read_count": full_read,
        "fully_addressed_count": fully_addressed,
        "remaining_count": remaining,
        "unread_count": status_counts.get("unread", 0),
        "read_needs_claims_count": status_counts.get("read_needs_claims", 0),
        "triaged_pending_action_count": status_counts.get("triaged_pending_action", 0),
        "triaged_watch_count": status_counts.get("triaged_watch", 0),
        "blocked_needs_steward_count": status_counts.get("blocked_needs_steward", 0),
        "status_counts": dict(sorted(status_counts.items())),
    }


def counter_audit_for_artifacts(
    artifacts: dict[str, dict[str, Any]],
    summary: dict[str, Any],
) -> dict[str, Any]:
    all_items = [
        item for item in artifacts.values() if item.get("present_on_disk") is not False
    ]
    canonical_items = [
        item for item in all_items if str(item.get("artifact_kind") or "") == "canonical_introspection"
    ]
    thin_items = [
        item for item in all_items if str(item.get("artifact_kind") or "") == "thin_introspection_output"
    ]
    other_items = [
        item
        for item in all_items
        if str(item.get("artifact_kind") or "")
        not in {"canonical_introspection", "thin_introspection_output"}
    ]
    all_counts = _addressing_scope_counts(all_items)
    canonical_counts = _addressing_scope_counts(canonical_items)
    thin_counts = _addressing_scope_counts(thin_items)
    other_counts = _addressing_scope_counts(other_items)
    checks = {
        "total_indexed_matches_scope_sum": int(summary.get("total_indexed") or 0)
        == canonical_counts["indexed"] + thin_counts["indexed"] + other_counts["indexed"],
        "all_remaining_matches_pending_count": all_counts["remaining_count"]
        == int(summary.get("pending_count") or 0),
        "all_addressed_plus_pending_matches_total": all_counts["fully_addressed_count"]
        + all_counts["remaining_count"]
        == all_counts["indexed"],
        "canonical_addressed_plus_remaining_matches_indexed": canonical_counts["fully_addressed_count"]
        + canonical_counts["remaining_count"]
        == canonical_counts["indexed"],
        "canonical_indexed_matches_summary": canonical_counts["indexed"]
        == int(summary.get("canonical_indexed") or 0),
        "full_read_not_above_total": all_counts["full_read_count"] <= all_counts["indexed"],
        "canonical_full_read_not_above_indexed": canonical_counts["full_read_count"]
        <= canonical_counts["indexed"],
    }
    mismatches = [name for name, ok in checks.items() if not ok]
    return {
        "schema": "introspection_addressing_counter_audit_v1",
        "status": "consistent" if not mismatches else "mismatch",
        "scope_note": (
            "Current counters exclude historical records explicitly marked absent from the "
            "filesystem snapshot while retaining their read, claim, and evidence history. "
            "summary.pending_count is all currently present indexed artifacts; use "
            "canonical_introspections.remaining_count for the canonical reading backlog."
        ),
        "recommended_final_report_fields": {
            "canonical_indexed": canonical_counts["indexed"],
            "canonical_fully_addressed": canonical_counts["fully_addressed_count"],
            "canonical_remaining": canonical_counts["remaining_count"],
            "all_artifact_pending": all_counts["remaining_count"],
            "noncanonical_pending": thin_counts["remaining_count"] + other_counts["remaining_count"],
        },
        "checks": checks,
        "mismatches": mismatches,
        "all_artifacts": all_counts,
        "canonical_introspections": canonical_counts,
        "thin_introspection_outputs": thin_counts,
        "other_timestamped_text": other_counts,
    }


def materialized_status(
    artifacts: dict[str, dict[str, Any]],
    *,
    work_items: dict[str, dict[str, Any]] | None = None,
    cutoff: dict[str, Any],
    corrupt_event_lines: int = 0,
) -> dict[str, Any]:
    work_items = work_items or {}
    active_artifacts = {
        key: item
        for key, item in artifacts.items()
        if item.get("present_on_disk") is not False
    }
    historical_absent_count = len(artifacts) - len(active_artifacts)
    counts = Counter(
        str(item.get("artifact_kind") or "unknown") for item in active_artifacts.values()
    )
    source_counts = Counter(
        str(item.get("source_family") or "unknown") for item in active_artifacts.values()
    )
    full_read_count = sum(1 for item in active_artifacts.values() if item.get("full_read"))
    fully_addressed_count = sum(
        1 for item in active_artifacts.values() if item.get("fully_addressed")
    )
    blocked_count = sum(
        1
        for item in active_artifacts.values()
        if item.get("status") == "blocked_needs_steward"
    )
    pending_count = sum(
        1 for item in active_artifacts.values() if not item.get("fully_addressed")
    )
    canonical_indexed = counts.get("canonical_introspection", 0)
    cutoff_filename = Path(str(cutoff.get("cutoff") or DEFAULT_CUTOFF)).name
    cutoff_indexed = any(
        item.get("filename") == cutoff_filename for item in active_artifacts.values()
    )
    artifacts_for_output = {
        key: with_agency_status_overlay(item)
        for key, item in sorted(artifacts.items())
    }
    work_items_for_output = {
        key: with_agency_status_overlay(item)
        for key, item in sorted(work_items.items())
    }
    summary = {
        "total_indexed": len(active_artifacts),
        "canonical_indexed": canonical_indexed,
        "thin_indexed": counts.get("thin_introspection_output", 0),
        "other_indexed": counts.get("other_timestamped_text", 0),
        "historical_absent_count": historical_absent_count,
        "full_read_count": full_read_count,
        "fully_addressed_count": fully_addressed_count,
        "pending_count": pending_count,
        "blocked_count": blocked_count,
        "corrupt_event_lines": corrupt_event_lines,
        "top_source_families": [
            {"source_family": name, "count": count}
            for name, count in source_counts.most_common(10)
        ],
    }
    counter_audit = counter_audit_for_artifacts(artifacts, summary)
    status = {
        "schema": SCHEMA,
        "schema_version": SCHEMA_VERSION,
        "generated_at": iso(),
        "cutoff": {
            **cutoff,
            "cutoff": cutoff_filename,
            "cutoff_indexed": cutoff_indexed,
        },
        "summary": summary,
        "counter_audit": counter_audit,
        "authority_boundary": AUTHORITY_BOUNDARY,
        "agency_boundary": AGENCY_BOUNDARY,
        "agency_continuation_during_authority_wait": AGENCY_CONTINUES_DURING_AUTHORITY_WAIT,
        "artifacts": artifacts_for_output,
        "work_items": work_items_for_output,
    }
    status["work_item_summary"] = work_item_summary(work_items_for_output)
    report = report_from_status(status)
    status["report"] = report
    status["next_queue"] = queue_items(status, limit=20)
    status["next_work_queue"] = work_queue_items(status, limit=20)
    return status


def status_from_records(records: list[dict[str, Any]], cutoff: dict[str, Any]) -> dict[str, Any]:
    artifacts = {
        str(record["introspection_id"]): _merge_artifact(None, record)
        for record in records
    }
    for artifact in artifacts.values():
        _derive_status(artifact)
    return materialized_status(artifacts, cutoff=cutoff)


def report_from_status(status: dict[str, Any]) -> dict[str, Any]:
    if not status:
        return {
            "schema": SCHEMA,
            "status": "database_missing",
            "summary": {},
            "work_item_summary": work_item_summary({}),
            "next_queue": [],
            "next_work_queue": [],
            "authority_boundary": AUTHORITY_BOUNDARY,
            "agency_boundary": AGENCY_BOUNDARY,
        }
    summary = status.get("summary") if isinstance(status.get("summary"), dict) else {}
    cutoff = status.get("cutoff") if isinstance(status.get("cutoff"), dict) else {}
    counter_audit = (
        status.get("counter_audit")
        if isinstance(status.get("counter_audit"), dict)
        else counter_audit_for_artifacts(
            status.get("artifacts") if isinstance(status.get("artifacts"), dict) else {},
            summary,
        )
    )
    canonical_remaining = int(
        ((counter_audit.get("canonical_introspections") or {}).get("remaining_count") or 0)
    )
    noncanonical_pending = int(
        ((counter_audit.get("recommended_final_report_fields") or {}).get("noncanonical_pending") or 0)
    )
    work_items = status.get("work_items") if isinstance(status.get("work_items"), dict) else {}
    work_summary = (
        status.get("work_item_summary")
        if isinstance(status.get("work_item_summary"), dict)
        else work_item_summary(work_items)
    )
    if counter_audit.get("status") == "mismatch":
        report_status = "counter_audit_mismatch"
    elif summary.get("corrupt_event_lines"):
        report_status = "database_corrupt_lines_ignored"
    elif not cutoff.get("cutoff_indexed"):
        report_status = "cutoff_not_indexed"
    elif summary.get("canonical_indexed") and canonical_remaining == 0 and noncanonical_pending == 0:
        report_status = "all_indexed_artifacts_addressed"
    elif summary.get("canonical_indexed") and canonical_remaining == 0:
        report_status = "all_canonical_introspections_addressed_noncanonical_pending"
    else:
        report_status = "queue_active"
    return {
        "schema": SCHEMA,
        "schema_version": SCHEMA_VERSION,
        "status": report_status,
        "cutoff": cutoff,
        "summary": {
            "total_indexed": int(summary.get("total_indexed") or 0),
            "canonical_indexed": int(summary.get("canonical_indexed") or 0),
            "full_read_count": int(summary.get("full_read_count") or 0),
            "fully_addressed_count": int(summary.get("fully_addressed_count") or 0),
            "pending_count": int(summary.get("pending_count") or 0),
            "blocked_count": int(summary.get("blocked_count") or 0),
            "historical_absent_count": int(summary.get("historical_absent_count") or 0),
            "corrupt_event_lines": int(summary.get("corrupt_event_lines") or 0),
            "top_source_families": summary.get("top_source_families") or [],
        },
        "counter_audit": counter_audit,
        "work_item_summary": work_summary,
        "next_queue": queue_items(status, limit=3),
        "next_work_queue": work_queue_items(status, limit=3),
        "authority_boundary": AUTHORITY_BOUNDARY,
        "agency_boundary": AGENCY_BOUNDARY,
    }


def load_status(state_dir: Path) -> dict[str, Any]:
    path = status_path(state_dir)
    if not path.exists():
        return {}
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return {
            "schema": SCHEMA,
            "report": {
                "schema": SCHEMA,
                "status": "database_corrupt",
                "summary": {},
                "next_queue": [],
                "authority_boundary": AUTHORITY_BOUNDARY,
            },
        }
    return payload if isinstance(payload, dict) else {}


def write_materialized_status(state_dir: Path, status: dict[str, Any]) -> None:
    normalize_artifact_authority_tree(status)
    atomic_write_text(
        status_path(state_dir),
        json.dumps(status, indent=2, sort_keys=True, ensure_ascii=False) + "\n",
    )
    atomic_write_text(queue_path(state_dir), render_queue_markdown(status))


def build_report(state_dir: Path = DEFAULT_STATE_DIR) -> dict[str, Any]:
    status = load_status(state_dir)
    if not status:
        return report_from_status({})
    report = status.get("report") if isinstance(status.get("report"), dict) else {}
    if not report or "work_item_summary" not in report:
        return report_from_status(status)
    if "counter_audit" not in report:
        if isinstance(status.get("artifacts"), dict):
            return report_from_status(status)
        legacy_report = dict(report)
        legacy_report["counter_audit"] = {
            "schema": "introspection_addressing_counter_audit_v1",
            "status": "legacy_unavailable",
            "scope_note": "Run inventory --write to materialize canonical/all-artifact counter reconciliation.",
            "recommended_final_report_fields": {
                "canonical_indexed": int((report.get("summary") or {}).get("canonical_indexed") or 0),
                "canonical_fully_addressed": 0,
                "canonical_remaining": int((report.get("summary") or {}).get("pending_count") or 0),
                "all_artifact_pending": int((report.get("summary") or {}).get("pending_count") or 0),
                "noncanonical_pending": 0,
            },
            "checks": {},
            "mismatches": [],
        }
        return legacy_report
    return report


def agency_corridor_packets_for_work_item(work_item: dict[str, Any]) -> list[dict[str, Any]]:
    refs = {
        str(work_item.get("work_item_id") or ""),
        str(work_item.get("source_introspection_id") or ""),
        str(work_item.get("claim_id") or ""),
    }
    refs = {ref for ref in refs if ref}
    status_path_ = DEFAULT_AGENCY_CORRIDOR_STATE_DIR / "status.json"
    if not status_path_.exists() or not refs:
        return []
    try:
        status = json.loads(status_path_.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return []
    packets = status.get("packets") if isinstance(status, dict) else {}
    matches: list[dict[str, Any]] = []
    for packet in (packets.values() if isinstance(packets, dict) else []):
        if not isinstance(packet, dict):
            continue
        packet_refs = {
            str(packet.get("corridor_id") or ""),
            *[str(ref) for ref in packet.get("work_item_ids", []) if ref],
            *[str(ref) for ref in packet.get("closure_card_refs", []) if ref],
            *[str(ref) for ref in packet.get("evidence_refs", []) if ref],
        }
        if refs & packet_refs:
            matches.append(packet)
    matches.sort(key=lambda packet: (str(packet.get("action") or ""), str(packet.get("corridor_id") or "")))
    return matches[:4]


def agency_corridor_v2_packets_for_work_item(work_item: dict[str, Any]) -> list[dict[str, Any]]:
    refs = {
        str(work_item.get("work_item_id") or ""),
        str(work_item.get("source_introspection_id") or ""),
        str(work_item.get("claim_id") or ""),
    }
    refs = {ref for ref in refs if ref}
    status_path_ = DEFAULT_AGENCY_CORRIDOR_V2_STATE_DIR / "status.json"
    if not status_path_.exists() or not refs:
        return []
    try:
        status = json.loads(status_path_.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return []
    packets = status.get("packets") if isinstance(status, dict) else {}
    matches: list[dict[str, Any]] = []
    for packet in (packets.values() if isinstance(packets, dict) else []):
        if not isinstance(packet, dict):
            continue
        packet_refs = {
            str(packet.get("corridor_id") or ""),
            str(packet.get("v1_corridor_id") or ""),
            *[str(ref) for ref in packet.get("work_item_ids", []) if ref],
            *[str(ref) for ref in packet.get("closure_card_refs", []) if ref],
            *[str(ref) for ref in packet.get("evidence_refs", []) if ref],
        }
        if refs & packet_refs:
            matches.append(packet)
    matches.sort(key=lambda packet: (str(packet.get("action") or ""), str(packet.get("corridor_id") or "")))
    return matches[:4]


def agency_programs_for_work_item(work_item: dict[str, Any]) -> list[dict[str, Any]]:
    refs = {
        str(work_item.get("work_item_id") or ""),
        str(work_item.get("source_introspection_id") or ""),
        str(work_item.get("claim_id") or ""),
    }
    refs = {ref for ref in refs if ref}
    status_path_ = DEFAULT_AGENCY_CORRIDOR_V2_STATE_DIR / "programs.json"
    if not status_path_.exists() or not refs:
        return []
    try:
        payload = json.loads(status_path_.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return []
    programs = payload.get("programs") if isinstance(payload, dict) else {}
    matches: list[dict[str, Any]] = []
    for program in (programs.values() if isinstance(programs, dict) else []):
        if not isinstance(program, dict):
            continue
        program_refs = {
            str(program.get("program_id") or ""),
            *[str(ref) for ref in program.get("linked_corridor_ids", []) if ref],
            *[str(ref) for ref in program.get("work_item_ids", []) if ref],
            *[str(ref) for ref in program.get("sandbox_trial_ids", []) if ref],
            *[str(ref) for ref in program.get("evidence_refs", []) if ref],
        }
        if refs & program_refs:
            matches.append(program)
    matches.sort(
        key=lambda program: (
            -int((program.get("priority_signal") or {}).get("deterministic_score") or 0),
            str(program.get("program_id") or ""),
        )
    )
    return matches[:4]


def agency_corridor_closure_card_section(work_item: dict[str, Any]) -> list[str]:
    packets = agency_corridor_packets_for_work_item(work_item)
    packets_v2 = agency_corridor_v2_packets_for_work_item(work_item)
    programs = agency_programs_for_work_item(work_item)
    lines = ["", "## Agency Corridor V1/V2"]
    if packets or packets_v2 or programs:
        for packet in packets:
            lines.append(
                f"- corridor_id: {packet.get('corridor_id')} action={packet.get('action')} "
                f"state={packet.get('state')} right_to_ignore=true "
                f"live_eligible_now={str(bool(packet.get('live_eligible_now'))).lower()} "
                f"auto_approved={str(bool(packet.get('auto_approved'))).lower()}"
            )
        for packet in packets_v2:
            lease = packet.get("autonomy_lease") if isinstance(packet.get("autonomy_lease"), dict) else {}
            step = packet.get("queue_step") if isinstance(packet.get("queue_step"), dict) else {}
            proposal = packet.get("source_prep_proposal") if isinstance(packet.get("source_prep_proposal"), dict) else {}
            lines.append(
                f"- corridor_v2_id: {packet.get('corridor_id')} action={packet.get('action')} "
                f"state={packet.get('state')} lease={lease.get('lease_id', 'none')} "
                f"queue_priority={step.get('priority', 'none')} source_prep={proposal.get('proposal_id', 'none')} "
                f"grants_approval={str(bool(packet.get('grants_approval'))).lower()} "
                f"live_eligible_now={str(bool(packet.get('live_eligible_now'))).lower()} "
                f"auto_approved={str(bool(packet.get('auto_approved'))).lower()}"
            )
        for program in programs:
            priority = program.get("priority_signal") if isinstance(program.get("priority_signal"), dict) else {}
            lines.append(
                f"- program_id: {program.get('program_id')} status={program.get('status')} "
                f"score={priority.get('deterministic_score', 0)} next={program.get('current_next_action')} "
                f"edits_source_now={str(bool(program.get('edits_source_now'))).lower()} "
                f"grants_approval={str(bool(program.get('grants_approval'))).lower()} "
                f"live_eligible_now={str(bool(program.get('live_eligible_now'))).lower()}"
            )
    else:
        lines.append(
            "No linked corridor packet/program is materialized yet. Generate with `python3 scripts/agency_corridor.py generate --write --json`, `python3 scripts/agency_corridor.py queue generate --write --json`, and `python3 scripts/agency_corridor.py programs generate --write --json`; beings may object, request safe replay, request scoped self-observation, or propose canary/source-prep criteria as non-live evidence."
        )
    lines.append(
        "Boundary: corridor work never grants approval, never marks live work runnable, and never mutates runtime/control state."
    )
    return lines


def event_inventory_run(cutoff: dict[str, Any], count: int) -> dict[str, Any]:
    return {
        "event_type": "inventory_run",
        "ts": now_s(),
        "schema": SCHEMA,
        "cutoff": cutoff,
        "artifact_count": count,
    }


def event_inventory_artifact(record: dict[str, Any]) -> dict[str, Any]:
    return {
        "event_type": "inventory_artifact",
        "ts": now_s(),
        "schema": SCHEMA,
        "artifact": record,
    }


def event_inventory_artifact_absent(
    introspection_id: str,
    *,
    path: str,
    reason: str = "missing_from_current_filesystem_snapshot",
) -> dict[str, Any]:
    return {
        "event_type": "inventory_artifact_absent",
        "ts": now_s(),
        "schema": SCHEMA,
        "introspection_id": introspection_id,
        "path": path,
        "reason": reason,
    }


def build_inventory(
    introspections_dir: Path,
    state_dir: Path,
    cutoff: str,
    *,
    write: bool = False,
    candidate_sources: dict[str, str] | None = None,
) -> dict[str, Any]:
    records, cutoff_info = inventory_records(
        introspections_dir,
        cutoff,
        candidate_sources=candidate_sources,
    )
    preview_status = status_from_records(records, cutoff_info)
    result = {
        "schema": SCHEMA,
        "write": write,
        "cutoff": cutoff_info,
        "summary": preview_status["summary"],
        "artifact_count": len(records),
        "next_queue": preview_status["next_queue"][:20],
        "authority_boundary": AUTHORITY_BOUNDARY,
    }
    if not write:
        return {**result, "artifacts": records}

    existing = load_status(state_dir)
    existing_artifacts = existing.get("artifacts") if isinstance(existing.get("artifacts"), dict) else {}
    current_ids = {str(record["introspection_id"]) for record in records}
    events = [event_inventory_run(cutoff_info, len(records))]
    for record in records:
        current = existing_artifacts.get(record["introspection_id"]) if isinstance(existing_artifacts, dict) else None
        if (
            not isinstance(current, dict)
            or current.get("present_on_disk") is False
            or current.get("sha256") != record.get("sha256")
            or current.get("candidate_evidence") != record.get("candidate_evidence")
        ):
            events.append(event_inventory_artifact(record))
    cutoff_ts = int(cutoff_info.get("cutoff_timestamp") or 0)
    for introspection_id, current in existing_artifacts.items():
        if not isinstance(current, dict) or current.get("present_on_disk") is False:
            continue
        timestamp = current.get("timestamp")
        if not isinstance(timestamp, int) or timestamp > cutoff_ts:
            continue
        if introspection_id in current_ids:
            continue
        filename = str(current.get("filename") or "")
        expected_path = introspections_dir / filename
        if not filename or expected_path.exists():
            continue
        events.append(
            event_inventory_artifact_absent(
                str(introspection_id),
                path=str(expected_path),
            )
        )
    append_events(state_dir, events)
    status = replay_events(state_dir)
    write_materialized_status(state_dir, status)
    materialized_artifacts = (
        status.get("artifacts") if isinstance(status.get("artifacts"), dict) else {}
    )
    materialized_ids_in_scope = {
        str(introspection_id)
        for introspection_id, artifact in materialized_artifacts.items()
        if isinstance(artifact, dict)
        and artifact.get("present_on_disk") is not False
        and isinstance(artifact.get("timestamp"), int)
        and int(artifact["timestamp"]) <= cutoff_ts
    }
    snapshot_checks = {
        "scan_matches_materialized_active_scope": current_ids == materialized_ids_in_scope,
        "materialized_counter_audit_consistent": (
            (status.get("counter_audit") or {}).get("status") == "consistent"
        ),
    }
    return {
        **result,
        "summary": status.get("summary") or {},
        "next_queue": status.get("next_queue") or [],
        "scan_summary": preview_status["summary"],
        "snapshot_audit": {
            "status": "consistent" if all(snapshot_checks.values()) else "mismatch",
            "checks": snapshot_checks,
            "scan_count": len(records),
            "materialized_active_in_scope_count": len(materialized_ids_in_scope),
            "historical_absent_count": int(
                (status.get("summary") or {}).get("historical_absent_count") or 0
            ),
            "absent_events_appended": sum(
                1 for event in events if event.get("event_type") == "inventory_artifact_absent"
            ),
        },
        "events_appended": len(events),
        "inventory_artifact_events_appended": sum(
            1 for event in events if event.get("event_type") == "inventory_artifact"
        ),
        "status_path": str(status_path(state_dir)),
        "queue_path": str(queue_path(state_dir)),
        "report": status.get("report"),
    }


def parse_claims_file(path: Path) -> list[dict[str, Any]]:
    text = path.read_text(encoding="utf-8", errors="replace")
    try:
        payload = json.loads(text)
    except json.JSONDecodeError:
        payload = None
    if isinstance(payload, dict):
        raw_claims = payload.get("claims")
    elif isinstance(payload, list):
        raw_claims = payload
    else:
        raw_claims = [
            {"summary": line.strip()}
            for line in text.splitlines()
            if line.strip() and not line.lstrip().startswith("#")
        ]
    claims: list[dict[str, Any]] = []
    for idx, raw in enumerate(raw_claims or [], start=1):
        if isinstance(raw, str):
            raw = {"summary": raw}
        if not isinstance(raw, dict):
            continue
        claim_id = str(raw.get("claim_id") or raw.get("id") or f"c{idx:03d}")
        claim = {
            "claim_id": claim_id,
            "summary": bounded_text(str(raw.get("summary") or raw.get("claim") or ""), limit=500),
            "disposition": str(raw.get("disposition") or "triaged_pending_action"),
        }
        classification = str(raw.get("classification") or "").strip()
        authority = str(raw.get("authority") or "").strip()
        if classification:
            claim["classification"] = classification
        if authority:
            claim["authority"] = authority
        claims.append(claim)
    return claims


def preview_or_write_event(state_dir: Path, event: dict[str, Any], *, write: bool) -> dict[str, Any]:
    if not write:
        return {
            "schema": SCHEMA,
            "write": False,
            "event": event,
            "authority_boundary": AUTHORITY_BOUNDARY,
            "agency_boundary": AGENCY_BOUNDARY,
        }
    append_events(state_dir, [event])
    status = replay_events(state_dir)
    write_materialized_status(state_dir, status)
    artifact_id = str(event.get("introspection_id") or "")
    artifact = (status.get("artifacts") or {}).get(artifact_id, {})
    work_item_id = str(event.get("work_item_id") or "")
    if not work_item_id and isinstance(event.get("work_item"), dict):
        work_item_id = str(event["work_item"].get("work_item_id") or "")
    work_item = (status.get("work_items") or {}).get(work_item_id, {})
    return {
        "schema": SCHEMA,
        "write": True,
        "event": event,
        "artifact_status": {
            "introspection_id": artifact_id,
            "status": artifact.get("status"),
            "fully_addressed": artifact.get("fully_addressed"),
            "proof_missing_claims": artifact.get("proof_missing_claims"),
        },
        "work_item_status": {
            "work_item_id": work_item_id,
            "status": work_item.get("status"),
            "agency_tier": work_item.get("agency_tier"),
            "post_change_response_status": work_item.get("post_change_response_status"),
        },
        "status_path": str(status_path(state_dir)),
        "queue_path": str(queue_path(state_dir)),
        "authority_boundary": AUTHORITY_BOUNDARY,
        "agency_boundary": AGENCY_BOUNDARY,
    }


def record_read_event(
    introspection_id: str,
    reader: str,
    summary_file: Path,
    claims_file: Path,
) -> dict[str, Any]:
    summary_text = summary_file.read_text(encoding="utf-8", errors="replace")
    return {
        "event_type": "full_read",
        "ts": now_s(),
        "schema": SCHEMA,
        "introspection_id": introspection_id,
        "reader": reader,
        "summary_sha256": sha256_text(summary_text),
        "summary_excerpt": bounded_text(summary_text, limit=900),
        "claims": parse_claims_file(claims_file),
    }


def full_read_batch_rows(manifest_file: Path) -> list[dict[str, Any]]:
    payload = json.loads(manifest_file.read_text(encoding="utf-8"))
    rows = payload.get("reads") if isinstance(payload, dict) else payload
    if not isinstance(rows, list) or not rows:
        raise ValueError("full-read batch must contain a non-empty reads list")

    normalized: list[dict[str, Any]] = []
    seen_ids: set[str] = set()
    for index, row in enumerate(rows, start=1):
        if not isinstance(row, dict):
            raise ValueError(f"full-read batch row {index} must be an object")
        introspection_id = str(row.get("introspection_id") or "").strip()
        reader = str(row.get("reader") or "").strip()
        summary_raw = str(row.get("summary_file") or "").strip()
        claims_raw = str(row.get("claims_file") or "").strip()
        missing = [
            key
            for key, value in (
                ("introspection_id", introspection_id),
                ("reader", reader),
                ("summary_file", summary_raw),
                ("claims_file", claims_raw),
            )
            if not value
        ]
        if missing:
            raise ValueError(
                f"full-read batch row {index} is missing {', '.join(missing)}"
            )
        if introspection_id in seen_ids:
            raise ValueError(
                f"full-read batch row {index} duplicates introspection {introspection_id}"
            )
        seen_ids.add(introspection_id)

        summary_file = Path(summary_raw)
        claims_file = Path(claims_raw)
        if not summary_file.is_absolute():
            summary_file = manifest_file.parent / summary_file
        if not claims_file.is_absolute():
            claims_file = manifest_file.parent / claims_file
        if not summary_file.is_file():
            raise ValueError(
                f"full-read batch row {index} summary file does not exist: "
                f"{summary_file}"
            )
        if not claims_file.is_file():
            raise ValueError(
                f"full-read batch row {index} claims file does not exist: "
                f"{claims_file}"
            )
        normalized.append(
            {
                "introspection_id": introspection_id,
                "reader": reader,
                "summary_file": summary_file,
                "claims_file": claims_file,
            }
        )
    return normalized


def record_read_batch(
    state_dir: Path,
    manifest_file: Path,
    *,
    write: bool,
) -> dict[str, Any]:
    rows = full_read_batch_rows(manifest_file)
    status = load_or_replay_status(state_dir)
    artifacts = (
        status.get("artifacts") if isinstance(status.get("artifacts"), dict) else {}
    )
    for index, row in enumerate(rows, start=1):
        if row["introspection_id"] not in artifacts:
            raise ValueError(
                f"full-read batch row {index} references unknown introspection "
                f"{row['introspection_id']}"
            )

    events = [
        record_read_event(
            row["introspection_id"],
            row["reader"],
            row["summary_file"],
            row["claims_file"],
        )
        for row in rows
    ]
    materialized = status
    if write:
        append_events(state_dir, events)
        materialized = replay_events(state_dir)
        write_materialized_status(state_dir, materialized)

    materialized_artifacts = (
        materialized.get("artifacts")
        if isinstance(materialized.get("artifacts"), dict)
        else {}
    )
    return {
        "schema": SCHEMA,
        "write": write,
        "events_appended": len(events) if write else 0,
        "read_count": len(rows),
        "artifact_statuses": [
            {
                "introspection_id": row["introspection_id"],
                "status": materialized_artifacts.get(
                    row["introspection_id"], {}
                ).get("status"),
                "fully_addressed": materialized_artifacts.get(
                    row["introspection_id"], {}
                ).get("fully_addressed"),
                "proof_missing_claims": materialized_artifacts.get(
                    row["introspection_id"], {}
                ).get("proof_missing_claims"),
            }
            for row in rows
        ],
        "status_path": str(status_path(state_dir)),
        "queue_path": str(queue_path(state_dir)),
        "authority_boundary": AUTHORITY_BOUNDARY,
    }


def evidence_event(
    introspection_id: str,
    claim_id: str,
    kind: str,
    target: str,
    note: str,
) -> dict[str, Any]:
    if kind not in EVIDENCE_KINDS:
        raise ValueError(f"evidence kind must be one of {sorted(EVIDENCE_KINDS)}")
    return {
        "event_type": "evidence_linked",
        "ts": now_s(),
        "schema": SCHEMA,
        "introspection_id": introspection_id,
        "claim_id": claim_id,
        "evidence": {
            "kind": kind,
            "target": target,
            "note": note,
            "ts": now_s(),
        },
    }


def evidence_batch_rows(links_file: Path) -> list[dict[str, str]]:
    payload = json.loads(links_file.read_text(encoding="utf-8"))
    rows = payload.get("links") if isinstance(payload, dict) else payload
    if not isinstance(rows, list) or not rows:
        raise ValueError("evidence batch must contain a non-empty links list")

    normalized: list[dict[str, str]] = []
    for index, row in enumerate(rows, start=1):
        if not isinstance(row, dict):
            raise ValueError(f"evidence batch row {index} must be an object")
        normalized_row = {
            "introspection_id": str(row.get("introspection_id") or "").strip(),
            "claim_id": str(row.get("claim_id") or "").strip(),
            "kind": str(row.get("kind") or "").strip(),
            "target": str(row.get("target") or "").strip(),
            "note": str(row.get("note") or "").strip(),
        }
        missing = [
            key
            for key in ("introspection_id", "claim_id", "kind", "target")
            if not normalized_row[key]
        ]
        if missing:
            raise ValueError(
                f"evidence batch row {index} is missing {', '.join(missing)}"
            )
        if normalized_row["kind"] not in EVIDENCE_KINDS:
            raise ValueError(
                f"evidence batch row {index} kind must be one of "
                f"{sorted(EVIDENCE_KINDS)}"
            )
        normalized.append(normalized_row)
    return normalized


def link_evidence_batch(
    state_dir: Path,
    links_file: Path,
    *,
    write: bool,
) -> dict[str, Any]:
    rows = evidence_batch_rows(links_file)
    status = load_or_replay_status(state_dir)
    artifacts = status.get("artifacts") if isinstance(status.get("artifacts"), dict) else {}
    expanded_rows: list[dict[str, str]] = []
    for index, row in enumerate(rows, start=1):
        artifact = artifacts.get(row["introspection_id"])
        if not isinstance(artifact, dict):
            raise ValueError(
                f"evidence batch row {index} references unknown introspection "
                f"{row['introspection_id']}"
            )
        claims = artifact.get("claims") if isinstance(artifact.get("claims"), dict) else {}
        claim_ids = sorted(claims) if row["claim_id"] == "*" else [row["claim_id"]]
        if not claim_ids:
            raise ValueError(
                f"evidence batch row {index} wildcard references an introspection "
                "without claims"
            )
        if any(claim_id not in claims for claim_id in claim_ids):
            raise ValueError(
                f"evidence batch row {index} references unknown claim "
                f"{row['introspection_id']}:{row['claim_id']}"
            )
        expanded_rows.extend({**row, "claim_id": claim_id} for claim_id in claim_ids)

    events = [
        evidence_event(
            row["introspection_id"],
            row["claim_id"],
            row["kind"],
            row["target"],
            row["note"],
        )
        for row in expanded_rows
    ]
    materialized = status
    if write:
        append_events(state_dir, events)
        materialized = replay_events(state_dir)
        write_materialized_status(state_dir, materialized)

    materialized_artifacts = (
        materialized.get("artifacts")
        if isinstance(materialized.get("artifacts"), dict)
        else {}
    )
    touched_ids = list(dict.fromkeys(row["introspection_id"] for row in expanded_rows))
    return {
        "schema": SCHEMA,
        "write": write,
        "events_appended": len(events) if write else 0,
        "link_count": len(expanded_rows),
        "batch_row_count": len(rows),
        "introspection_count": len(touched_ids),
        "artifact_statuses": [
            {
                "introspection_id": introspection_id,
                "status": materialized_artifacts.get(introspection_id, {}).get("status"),
                "fully_addressed": materialized_artifacts.get(introspection_id, {}).get(
                    "fully_addressed"
                ),
                "proof_missing_claims": materialized_artifacts.get(
                    introspection_id, {}
                ).get("proof_missing_claims"),
            }
            for introspection_id in touched_ids
        ],
        "status_path": str(status_path(state_dir)),
        "queue_path": str(queue_path(state_dir)),
        "authority_boundary": AUTHORITY_BOUNDARY,
    }


def close_event(introspection_id: str, status: str, rationale: str) -> dict[str, Any]:
    if status not in TERMINAL_STATUSES and status != "blocked_needs_steward":
        raise ValueError("close status must be terminal or blocked_needs_steward")
    return {
        "event_type": "closed",
        "ts": now_s(),
        "schema": SCHEMA,
        "introspection_id": introspection_id,
        "status": status,
        "rationale": rationale,
    }


def close_batch(
    state_dir: Path,
    introspection_ids: list[str],
    status: str,
    rationale: str,
    *,
    write: bool,
) -> dict[str, Any]:
    if not introspection_ids:
        raise ValueError("close batch requires at least one introspection id")
    if len(set(introspection_ids)) != len(introspection_ids):
        raise ValueError("close batch introspection ids must be unique")

    current = load_or_replay_status(state_dir)
    artifacts = (
        current.get("artifacts")
        if isinstance(current.get("artifacts"), dict)
        else {}
    )
    unknown = [
        introspection_id
        for introspection_id in introspection_ids
        if introspection_id not in artifacts
    ]
    if unknown:
        raise ValueError(f"close batch references unknown introspections: {unknown}")

    events = [
        close_event(introspection_id, status, rationale)
        for introspection_id in introspection_ids
    ]
    materialized = current
    if write:
        append_events(state_dir, events)
        materialized = replay_events(state_dir)
        write_materialized_status(state_dir, materialized)

    materialized_artifacts = (
        materialized.get("artifacts")
        if isinstance(materialized.get("artifacts"), dict)
        else {}
    )
    return {
        "schema": SCHEMA,
        "write": write,
        "events_appended": len(events) if write else 0,
        "close_count": len(events),
        "artifact_statuses": [
            {
                "introspection_id": introspection_id,
                "status": materialized_artifacts.get(introspection_id, {}).get("status"),
                "fully_addressed": materialized_artifacts.get(
                    introspection_id, {}
                ).get("fully_addressed"),
                "proof_missing_claims": materialized_artifacts.get(
                    introspection_id, {}
                ).get("proof_missing_claims"),
            }
            for introspection_id in introspection_ids
        ],
        "status_path": str(status_path(state_dir)),
        "queue_path": str(queue_path(state_dir)),
        "authority_boundary": AUTHORITY_BOUNDARY,
        "agency_boundary": AGENCY_BOUNDARY,
    }


def work_item_from_claim(artifact: dict[str, Any], claim: dict[str, Any]) -> dict[str, Any]:
    introspection_id = str(artifact.get("introspection_id") or "")
    claim_id = str(claim.get("claim_id") or "")
    claim_summary = bounded_text(str(claim.get("summary") or ""), limit=700)
    claim_disposition = bounded_text(str(claim.get("disposition") or ""), limit=900)
    # Authority follows the concrete claim, never nouns that happen to appear
    # in a source-family label or filename.
    tier = agency_tier_for_claim(claim_summary)
    classification = str(claim.get("classification") or "").strip()
    claim_authority = str(claim.get("authority") or "").strip()
    if classification == "tier_5_wait":
        tier = 5
    elif classification == "authority_gated":
        # A structured approval disposition outranks heuristic wording. Honor
        # an explicit Tier 4 disposition when present; otherwise fail closed
        # at Tier 5 so an unfamiliar live proposal cannot become runnable.
        disposition_tier = agency_tier_for_claim(claim_disposition)
        tier = disposition_tier if disposition_tier >= 4 else 5
    elif classification in {"needs_sandbox", "sandbox_routed"}:
        tier = 3
    elif classification == "verified_existing" and _is_explicit_verification_claim(
        claim_disposition
    ):
        # Historical reports often phrase a desired property as "should
        # change" even when the grounded disposition verifies that later
        # source already implements it. Honor that explicit verification
        # disposition without letting a bare/mislabeled verified claim mask a
        # genuine live mutation request.
        tier = agency_tier_for_claim(claim_disposition)
    created = now_s()
    title = bounded_text(claim_summary, limit=90) or f"{introspection_id}:{claim_id}"
    claim_evidence = (
        claim.get("evidence") if isinstance(claim.get("evidence"), list) else []
    )
    status = AGENCY_TIER_DEFAULT_STATUS.get(tier, "ready_for_implementation")
    if tier < 4:
        status = {
            "implemented": "implemented_awaiting_felt_response",
            "implemented_now": "implemented_awaiting_felt_response",
            "verified_existing": "verified_existing",
            "observed": "verified_existing",
            "needs_sandbox": "needs_sandbox",
            "sandbox_routed": "needs_sandbox",
        }.get(classification, status)
    item = {
        "work_item_id": work_item_id_for(introspection_id, claim_id),
        "source_introspection_id": introspection_id,
        "source_filename": artifact.get("filename"),
        "source_path": artifact.get("path"),
        "source_family": artifact.get("source_family"),
        "claim_id": claim_id,
        "claim_classification": classification or None,
        "claim_disposition": claim_disposition or None,
        "claim_authority": claim_authority or None,
        "being": being_from_source(artifact),
        "title": title,
        "claim_summary": claim_summary,
        "agency_tier": tier,
        "agency_tier_label": AGENCY_TIER_LABELS.get(tier),
        "route": agency_route_for_tier(tier),
        "status": status,
        "evidence_required": AGENCY_TIER_REQUIRED_EVIDENCE.get(tier),
        "suggested_next": AGENCY_TIER_REQUIRED_EVIDENCE.get(tier),
        "created_at": created,
        "updated_at": created,
        "blocked_by": "operator_approval" if tier == 5 else ("steward_grant" if tier == 4 else None),
        "auto_approved": False,
        "fully_addressed_effect": False,
        "evidence_links": [
            dict(evidence) for evidence in claim_evidence if isinstance(evidence, dict)
        ],
    }
    refresh_work_item_authority_packets(item)
    return item


def work_item_created_event(item: dict[str, Any]) -> dict[str, Any]:
    return {
        "event_type": "work_item_created",
        "ts": now_s(),
        "schema": SCHEMA,
        "work_item_id": item.get("work_item_id"),
        "work_item": item,
    }


def work_status_event(work_item_id: str, status: str, note: str, blocked_by: str | None = None) -> dict[str, Any]:
    if status not in WORK_STATUSES:
        raise ValueError(f"work status must be one of {sorted(WORK_STATUSES)}")
    return {
        "event_type": "work_status_set",
        "ts": now_s(),
        "schema": SCHEMA,
        "work_item_id": work_item_id,
        "status": status,
        "note": note,
        "blocked_by": blocked_by,
    }


def work_evidence_event(work_item_id: str, kind: str, target: str, note: str) -> dict[str, Any]:
    if kind not in WORK_EVIDENCE_KINDS:
        raise ValueError(f"work evidence kind must be one of {sorted(WORK_EVIDENCE_KINDS)}")
    return {
        "event_type": "work_evidence_linked",
        "ts": now_s(),
        "schema": SCHEMA,
        "work_item_id": work_item_id,
        "evidence": {
            "kind": kind,
            "target": target,
            "note": note,
            "ts": now_s(),
        },
    }


def agency_tier_request_event(work_item_id: str, tier: int, reason: str) -> dict[str, Any]:
    if tier not in AGENCY_TIER_LABELS:
        raise ValueError("tier must be an integer from 0 through 5")
    return {
        "event_type": "agency_tier_requested",
        "ts": now_s(),
        "schema": SCHEMA,
        "work_item_id": work_item_id,
        "tier": tier,
        "tier_label": AGENCY_TIER_LABELS[tier],
        "reason": reason,
        "request_status": "requested",
        "auto_approved": False,
    }


def agency_tier_correction_event(
    work_item_id: str,
    tier: int,
    previous_tier: int,
    reason: str,
) -> dict[str, Any]:
    if tier not in AGENCY_TIER_LABELS:
        raise ValueError("tier must be an integer from 0 through 5")
    if tier >= previous_tier:
        raise ValueError("classification correction must lower the current tier")
    return {
        "event_type": "agency_tier_corrected",
        "ts": now_s(),
        "schema": SCHEMA,
        "work_item_id": work_item_id,
        "previous_tier": previous_tier,
        "tier": tier,
        "tier_label": AGENCY_TIER_LABELS[tier],
        "reason": reason,
        "correction_status": "classification_correction",
        "grants_approval": False,
        "live_eligible_now": False,
        "auto_approved": False,
    }


def post_change_response_event(
    work_item_id: str,
    response_status: str,
    source: str,
    note: str,
) -> dict[str, Any]:
    if response_status not in POST_CHANGE_RESPONSE_STATUSES:
        raise ValueError(f"response status must be one of {sorted(POST_CHANGE_RESPONSE_STATUSES)}")
    return {
        "event_type": "post_change_response_recorded",
        "ts": now_s(),
        "schema": SCHEMA,
        "work_item_id": work_item_id,
        "response_status": response_status,
        "source": source,
        "note": note,
    }


def closure_card_text(work_item: dict[str, Any]) -> str:
    lines = [
        "# Feedback Closure Card V1",
        "",
        f"- work_item_id: {work_item.get('work_item_id')}",
        f"- source_introspection_id: {work_item.get('source_introspection_id')}",
        f"- claim_id: {work_item.get('claim_id')}",
        f"- being: {work_item.get('being')}",
        f"- agency_tier: {work_item.get('agency_tier')} {work_item.get('agency_tier_label')}",
        f"- status: {work_item.get('status')}",
        f"- right_to_ignore: true",
        "",
        "## Heard",
        bounded_text(str(work_item.get("claim_summary") or ""), limit=700),
        "",
        "## Response",
        bounded_text(str(work_item.get("suggested_next") or work_item.get("evidence_required") or ""), limit=700),
        *agency_corridor_closure_card_section(work_item),
        "",
        "## Still Gated",
        (
            "Any live pressure, fill, PI, sensory cadence, controller, protocol, "
            "peer-mutation, or substrate-facing change still requires the tiered "
            "approval path named above."
        ),
    ]
    packet = authority_boundary_packet_for_work_item(work_item)
    if packet:
        packet_v2 = authority_boundary_packet_v2_for_work_item(work_item)
        lines.extend(
            [
                "",
                "## Authority Boundary Packet V1",
                "```json",
                json.dumps(packet, indent=2, sort_keys=True, ensure_ascii=False),
                "```",
            ]
        )
        if packet_v2:
            lines.extend(
                [
                    "",
                    "## Authority Boundary Packet V2",
                    "```json",
                    json.dumps(packet_v2, indent=2, sort_keys=True, ensure_ascii=False),
                    "```",
                ]
            )
    return "\n".join(lines).rstrip() + "\n"


def closure_card_event(
    state_dir: Path,
    work_item: dict[str, Any],
    *,
    write: bool,
    deliver: bool = False,
) -> tuple[dict[str, Any], dict[str, Any]]:
    ts = int(now_s())
    work_item_id = str(work_item.get("work_item_id") or "")
    text = closure_card_text(work_item)
    card_dir = state_dir / "closure_cards"
    card_path = card_dir / f"{ts}_{work_item_id}.md"
    delivered_path = None
    if write:
        atomic_write_text(card_path, text)
        if deliver:
            target_dir = MINIME_INBOX if work_item.get("being") == "minime" else ASTRID_INBOX
            delivered_path = target_dir / f"mike_feedback_feedback_closure_card_{work_item_id}_{ts}.txt"
            atomic_write_text(delivered_path, text)
    card = {
        "schema": "feedback_closure_card_v1",
        "work_item_id": work_item_id,
        "source_introspection_id": work_item.get("source_introspection_id"),
        "claim_id": work_item.get("claim_id"),
        "being": work_item.get("being"),
        "agency_tier": work_item.get("agency_tier"),
        "path": str(card_path),
        "delivered_path": str(delivered_path) if delivered_path else None,
        "right_to_ignore": True,
        "authority_boundary_packet": authority_boundary_packet_for_work_item(work_item),
        "authority_boundary_packet_v2": authority_boundary_packet_v2_for_work_item(work_item),
        "text_sha256": sha256_text(text),
        "text_excerpt": bounded_text(text, limit=900),
    }
    event = {
        "event_type": "closure_card_emitted",
        "ts": now_s(),
        "schema": SCHEMA,
        "work_item_id": work_item_id,
        "closure_card": card,
    }
    return event, card


def emit_closure_card_batch(
    state_dir: Path,
    work_items: list[dict[str, Any]],
    *,
    write: bool,
    deliver: bool = False,
) -> dict[str, Any]:
    if not work_items:
        raise ValueError("at least one work item is required")

    events: list[dict[str, Any]] = []
    cards: list[dict[str, Any]] = []
    for work_item in work_items:
        event, card = closure_card_event(
            state_dir,
            work_item,
            write=write,
            deliver=deliver,
        )
        events.append(event)
        cards.append(card)

    materialized: dict[str, Any] = {}
    if write:
        append_events(state_dir, events)
        materialized = replay_events(state_dir)
        write_materialized_status(state_dir, materialized)

    materialized_items = (
        materialized.get("work_items")
        if isinstance(materialized.get("work_items"), dict)
        else {}
    )
    return {
        "schema": SCHEMA,
        "write": write,
        "events_appended": len(events) if write else 0,
        "closure_cards": cards,
        "work_item_statuses": [
            {
                "work_item_id": card["work_item_id"],
                "status": (
                    materialized_items.get(card["work_item_id"], {}).get("status")
                    if write
                    else work_item.get("status")
                ),
                "closure_card_count": (
                    len(materialized_items.get(card["work_item_id"], {}).get("closure_cards") or [])
                    if write
                    else len(work_item.get("closure_cards") or []) + 1
                ),
            }
            for work_item, card in zip(work_items, cards, strict=True)
        ],
        "status_path": str(status_path(state_dir)),
        "queue_path": str(queue_path(state_dir)),
        "authority_boundary": AUTHORITY_BOUNDARY,
        "agency_boundary": AGENCY_BOUNDARY,
    }


def promote_work_items(
    state_dir: Path,
    *,
    ids: list[str] | None = None,
    next_count: int = 0,
    write: bool = False,
) -> dict[str, Any]:
    status = load_or_replay_status(state_dir)
    artifacts = status.get("artifacts") if isinstance(status.get("artifacts"), dict) else {}
    existing = status.get("work_items") if isinstance(status.get("work_items"), dict) else {}
    selected: list[dict[str, Any]] = []
    if ids:
        for introspection_id in ids:
            artifact = artifacts.get(introspection_id)
            if isinstance(artifact, dict):
                selected.append(artifact)
    elif next_count > 0:
        for row in queue_items(status, limit=next_count):
            artifact = artifacts.get(str(row.get("introspection_id") or ""))
            if isinstance(artifact, dict):
                selected.append(artifact)
    events: list[dict[str, Any]] = []
    skipped: list[dict[str, str]] = []
    created: list[dict[str, Any]] = []
    for artifact in selected:
        claims = artifact.get("claims") if isinstance(artifact.get("claims"), dict) else {}
        if not claims:
            skipped.append(
                {
                    "introspection_id": str(artifact.get("introspection_id") or ""),
                    "reason": "no_claims_recorded_yet",
                }
            )
            continue
        for claim in claims.values():
            if not isinstance(claim, dict):
                continue
            item = work_item_from_claim(artifact, claim)
            if item["work_item_id"] in existing:
                skipped.append({"work_item_id": item["work_item_id"], "reason": "already_exists"})
                continue
            created.append(item)
            events.append(work_item_created_event(item))
    if write and events:
        append_events(state_dir, events)
        materialized = replay_events(state_dir)
        write_materialized_status(state_dir, materialized)
    return {
        "schema": SCHEMA,
        "write": write,
        "created_count": len(created),
        "skipped": skipped,
        "work_items": created,
        "events_appended": len(events) if write else 0,
        "authority_boundary": AUTHORITY_BOUNDARY,
        "agency_boundary": AGENCY_BOUNDARY,
    }


def render_queue_markdown(status: dict[str, Any], *, limit: int = 50) -> str:
    report = report_from_status(status)
    work_summary = report.get("work_item_summary") or {}
    counter_audit = report.get("counter_audit") if isinstance(report.get("counter_audit"), dict) else {}
    recommended = (
        counter_audit.get("recommended_final_report_fields")
        if isinstance(counter_audit.get("recommended_final_report_fields"), dict)
        else {}
    )
    lines = [
        "# Introspection Addressing Queue",
        "",
        f"- status: {report.get('status')}",
        f"- total_indexed: {report.get('summary', {}).get('total_indexed', 0)}",
        f"- canonical_indexed: {report.get('summary', {}).get('canonical_indexed', 0)}",
        f"- historical_absent: {report.get('summary', {}).get('historical_absent_count', 0)}",
        f"- fully_addressed: {report.get('summary', {}).get('fully_addressed_count', 0)}",
        f"- pending: {report.get('summary', {}).get('pending_count', 0)}",
        f"- counter_audit: {counter_audit.get('status', 'unknown')}",
        f"- canonical_remaining: {recommended.get('canonical_remaining', 0)}",
        f"- noncanonical_pending: {recommended.get('noncanonical_pending', 0)}",
        f"- active_work_items: {work_summary.get('active_work_items', 0)}",
        f"- grant_waiting: {work_summary.get('grant_waiting_count', 0)}",
        f"- awaiting_felt_response: {work_summary.get('post_change_awaiting_response_count', 0)}",
        f"- boundary: {AUTHORITY_BOUNDARY}",
        f"- agency_boundary: {AGENCY_BOUNDARY}",
        "",
        "## Agency Work Queue",
        "",
    ]
    for item in work_queue_items(status, limit=limit):
        lines.extend(
            [
                f"### {item.get('work_item_id')} - {item.get('title')}",
                f"- source: `{item.get('source_introspection_id')}` claim `{item.get('claim_id')}`",
                f"- being/tier: `{item.get('being')}` / `{item.get('agency_tier')}` `{item.get('agency_tier_label')}`",
                f"- status: `{item.get('status')}`",
                f"- route: `{item.get('route')}`",
                f"- suggested_next: {item.get('suggested_next')}",
                "",
            ]
        )
    if not work_queue_items(status, limit=1):
        lines.append("No active work items.")
    lines.extend(
        [
            "",
            "## Next Reading Items",
            "",
        ]
    )
    for item in queue_items(status, limit=limit):
        lines.extend(
            [
                f"### {item.get('filename')}",
                f"- id: `{item.get('introspection_id')}`",
                f"- status: `{item.get('status')}`",
                f"- kind/source: `{item.get('artifact_kind')}` / `{item.get('source_family')}`",
                f"- path: `{item.get('path')}`",
                f"- excerpt: {item.get('excerpt')}",
                "",
            ]
        )
    if len(lines) <= 10:
        lines.append("No pending items.")
    return "\n".join(lines).rstrip() + "\n"


def render_report_markdown(report: dict[str, Any]) -> str:
    summary = report.get("summary") or {}
    work_summary = report.get("work_item_summary") or {}
    counter_audit = report.get("counter_audit") if isinstance(report.get("counter_audit"), dict) else {}
    recommended = (
        counter_audit.get("recommended_final_report_fields")
        if isinstance(counter_audit.get("recommended_final_report_fields"), dict)
        else {}
    )
    lines = [
        "# Introspection Addressing V1",
        "",
        f"- status: {report.get('status')}",
        f"- total_indexed: {summary.get('total_indexed', 0)}",
        f"- canonical_indexed: {summary.get('canonical_indexed', 0)}",
        f"- historical_absent: {summary.get('historical_absent_count', 0)}",
        f"- full_read: {summary.get('full_read_count', 0)}",
        f"- fully_addressed: {summary.get('fully_addressed_count', 0)}",
        f"- pending: {summary.get('pending_count', 0)}",
        f"- counter_audit: {counter_audit.get('status', 'unknown')}",
        f"- canonical_remaining: {recommended.get('canonical_remaining', 0)}",
        f"- all_artifact_pending: {recommended.get('all_artifact_pending', summary.get('pending_count', 0))}",
        f"- noncanonical_pending: {recommended.get('noncanonical_pending', 0)}",
        f"- blocked: {summary.get('blocked_count', 0)}",
        f"- corrupt_event_lines: {summary.get('corrupt_event_lines', 0)}",
        f"- active_work_items: {work_summary.get('active_work_items', 0)}",
        f"- work_by_tier: {work_summary.get('by_tier', {})}",
        f"- work_by_status: {work_summary.get('by_status', {})}",
        f"- grant_waiting: {work_summary.get('grant_waiting_count', 0)}",
        f"- awaiting_felt_response: {work_summary.get('post_change_awaiting_response_count', 0)}",
        f"- tier_mismatches: {work_summary.get('tier_mismatch_count', 0)}",
        f"- boundary: {report.get('authority_boundary') or AUTHORITY_BOUNDARY}",
        f"- agency_boundary: {report.get('agency_boundary') or AGENCY_BOUNDARY}",
        "",
        "## Counter Audit",
        "",
        f"- status: {counter_audit.get('status', 'unknown')}",
        f"- note: {counter_audit.get('scope_note', 'not available')}",
        f"- mismatches: {counter_audit.get('mismatches', [])}",
        "",
        "## Agency Work Queue",
        "",
    ]
    for item in report.get("next_work_queue") or []:
        lines.append(
            f"- `{item.get('work_item_id')}` tier {item.get('agency_tier')} "
            f"{item.get('status')} - {item.get('title')}"
        )
    if not report.get("next_work_queue"):
        lines.append("- none")
    lines.extend(
        [
            "",
            "## Next Reading Queue",
            "",
        ]
    )
    for item in report.get("next_queue") or []:
        lines.append(f"- `{item.get('introspection_id')}` ({item.get('status')}) - {item.get('path')}")
    if not report.get("next_queue"):
        lines.append("- none")
    return "\n".join(lines).rstrip() + "\n"


def render_work_queue_markdown(status: dict[str, Any], *, limit: int = 20) -> str:
    report = report_from_status(status)
    summary = report.get("work_item_summary") or {}
    lines = [
        "# Agency Work Queue V1",
        "",
        f"- total_work_items: {summary.get('total_work_items', 0)}",
        f"- active_work_items: {summary.get('active_work_items', 0)}",
        f"- by_tier: {summary.get('by_tier', {})}",
        f"- by_status: {summary.get('by_status', {})}",
        f"- grant_waiting: {summary.get('grant_waiting_count', 0)}",
        f"- awaiting_felt_response: {summary.get('post_change_awaiting_response_count', 0)}",
        f"- tier_mismatches: {summary.get('tier_mismatch_count', 0)}",
        f"- agency_boundary: {AGENCY_BOUNDARY}",
        "",
    ]
    for item in work_queue_items(status, limit=limit):
        lines.extend(
            [
                f"## {item.get('work_item_id')} - {item.get('title')}",
                f"- source: `{item.get('source_introspection_id')}` claim `{item.get('claim_id')}`",
                f"- being: `{item.get('being')}`",
                f"- tier: `{item.get('agency_tier')}` `{item.get('agency_tier_label')}`",
                f"- status: `{item.get('status')}`",
                f"- route: `{item.get('route')}`",
                f"- post_change_response: `{item.get('post_change_response_status')}`",
                f"- suggested_next: {item.get('suggested_next')}",
                "",
            ]
        )
    if not work_queue_items(status, limit=1):
        lines.append("No active work items.")
    return "\n".join(lines).rstrip() + "\n"


def load_or_replay_status(state_dir: Path) -> dict[str, Any]:
    status = load_status(state_dir)
    if status:
        return status
    events, _ = read_events(state_dir)
    if events:
        return replay_events(state_dir)
    return {}


def work_item_or_error(state_dir: Path, work_item_id: str) -> tuple[dict[str, Any], dict[str, Any]]:
    status = load_or_replay_status(state_dir)
    items = status.get("work_items") if isinstance(status.get("work_items"), dict) else {}
    item = items.get(work_item_id)
    if not isinstance(item, dict):
        raise ValueError(f"unknown work item id {work_item_id!r}")
    return status, item


def introspection_artifact_or_error(
    state_dir: Path,
    introspection_id: str,
) -> tuple[dict[str, Any], dict[str, Any]]:
    status = load_or_replay_status(state_dir)
    artifacts = status.get("artifacts") if isinstance(status.get("artifacts"), dict) else {}
    artifact = artifacts.get(introspection_id)
    if not isinstance(artifact, dict):
        raise ValueError(f"unknown introspection id {introspection_id!r}")
    return status, artifact


class IntrospectionAddressingAuditTests(unittest.TestCase):
    def test_no_response_is_neutral_review_expiry_not_waiver(self) -> None:
        receipts = work_item_lifecycle_receipts_v2(
            {
                "work_item_id": "wi_neutral_silence",
                "post_change_response_status": "no_response",
                "closure_cards": [{"card_id": "card_one"}],
                "evidence_links": [],
            },
            "boundary_one",
        )
        self.assertEqual(receipts[0]["kind"], "review_opportunity_expired")
        self.assertNotIn("waived", receipts[0]["bounded_summary"])
        self.assertIn("not affirmation", receipts[0]["bounded_summary"])

    def test_inventory_includes_cutoff_and_classifies_artifacts(self) -> None:
        import tempfile

        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            intros = root / "introspections"
            intros.mkdir()
            cutoff = intros / "introspection_astrid_llm_1783325217.txt"
            cutoff.write_text("=== ASTRID INTROSPECTION ===\nSource: astrid:llm\nObserved:\nhi\n")
            thin = intros / "thin_introspection_output_astrid_codec_1778410197.txt"
            thin.write_text("Artifact kind: thin_introspection_output\nLikely Snags:\nnone\n")
            later = intros / "introspection_astrid_llm_1783325218.txt"
            later.write_text("Observed:\ntoo late\n")

            records, info = inventory_records(intros, cutoff.name, candidate_sources={})

        self.assertTrue(info["cutoff_indexed"])
        self.assertEqual(len(records), 2)
        kinds = {record["filename"]: record["artifact_kind"] for record in records}
        self.assertEqual(kinds[cutoff.name], "canonical_introspection")
        self.assertEqual(kinds[thin.name], "thin_introspection_output")
        self.assertEqual(records[0]["introspection_id"], "introspection_astrid_llm_1783325217")

    def test_latest_cutoff_uses_numeric_timestamp_not_lexical_filename_order(self) -> None:
        import tempfile

        with tempfile.TemporaryDirectory() as tmpdir:
            intros = Path(tmpdir) / "introspections"
            intros.mkdir()
            lexically_late = intros / "introspection_zzzz_1780000100.txt"
            lexically_late.write_text("Observed:\nolder\n")
            numerically_latest = intros / "introspection_aaaa_1780000200.txt"
            numerically_latest.write_text("Observed:\nnewer\n")

            records, info = inventory_records(intros, "latest", candidate_sources={})

        self.assertEqual(info["cutoff"], numerically_latest.name)
        self.assertEqual(info["cutoff_timestamp"], 1_780_000_200)
        self.assertTrue(info["cutoff_indexed"])
        self.assertEqual(len(records), 2)

    def test_inventory_preserves_absent_history_without_inflating_active_counts(self) -> None:
        import tempfile

        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            intros = root / "introspections"
            state = root / "state"
            intros.mkdir()
            path = intros / "introspection_astrid_llm_1783325217.txt"
            text = "=== ASTRID INTROSPECTION ===\nSource: astrid:llm\nObserved:\nhi\n"
            path.write_text(text)

            initial = build_inventory(
                intros,
                state,
                path.name,
                write=True,
                candidate_sources={},
            )
            self.assertEqual(initial["summary"]["total_indexed"], 1)
            self.assertEqual(initial["artifact_count"], 1)
            self.assertNotIn("artifacts", initial)
            self.assertEqual(initial["snapshot_audit"]["status"], "consistent")

            preview = build_inventory(
                intros,
                state,
                path.name,
                write=False,
                candidate_sources={},
            )
            self.assertEqual(len(preview["artifacts"]), 1)

            path.unlink()
            absent = build_inventory(
                intros,
                state,
                path.name,
                write=True,
                candidate_sources={},
            )
            absent_record = load_status(state)["artifacts"][path.stem]
            self.assertEqual(absent["summary"]["total_indexed"], 0)
            self.assertEqual(absent["summary"]["historical_absent_count"], 1)
            self.assertEqual(absent["snapshot_audit"]["absent_events_appended"], 1)
            self.assertEqual(absent["snapshot_audit"]["status"], "consistent")
            self.assertFalse(absent_record["present_on_disk"])
            self.assertEqual(absent["next_queue"], [])

            path.write_text(text)
            restored = build_inventory(
                intros,
                state,
                path.name,
                write=True,
                candidate_sources={},
            )
            restored_record = load_status(state)["artifacts"][path.stem]
            self.assertEqual(restored["summary"]["total_indexed"], 1)
            self.assertEqual(restored["summary"]["historical_absent_count"], 0)
            self.assertTrue(restored_record["present_on_disk"])
            self.assertNotIn("inventory_absent_at", restored_record)
            self.assertNotIn("inventory_absent_reason", restored_record)
            self.assertEqual(restored["snapshot_audit"]["status"], "consistent")

    def test_fully_addressed_requires_read_claim_close_and_evidence(self) -> None:
        import tempfile

        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            state = root / "state"
            artifact = {
                "introspection_id": "introspection_astrid_llm_1",
                "filename": "introspection_astrid_llm_1.txt",
                "timestamp": 1,
                "artifact_kind": "canonical_introspection",
                "source_family": "astrid_llm",
                "path": "/tmp/introspection_astrid_llm_1.txt",
                "sha256": "abc",
                "candidate_evidence": [],
                "excerpt": "x",
            }
            append_events(state, [event_inventory_artifact(artifact)])
            status = replay_events(state)
            self.assertFalse(status["artifacts"][artifact["introspection_id"]]["fully_addressed"])

            summary = root / "summary.txt"
            claims = root / "claims.json"
            summary.write_text("read carefully")
            claims.write_text(json.dumps({"claims": [{"claim_id": "c001", "summary": "fix thing"}]}))
            append_events(
                state,
                [
                    record_read_event(artifact["introspection_id"], "codex", summary, claims),
                    close_event(artifact["introspection_id"], "addressed_change", "implemented"),
                ],
            )
            status = replay_events(state)
            rec = status["artifacts"][artifact["introspection_id"]]
            self.assertFalse(rec["fully_addressed"])
            self.assertEqual(rec["proof_missing_claims"], ["c001"])

            append_events(
                state,
                [
                    evidence_event(
                        artifact["introspection_id"],
                        "c001",
                        "code",
                        "/tmp/file.rs:10",
                        "implemented",
                    )
                ],
            )
            status = replay_events(state)
            self.assertTrue(status["artifacts"][artifact["introspection_id"]]["fully_addressed"])

    def test_candidate_evidence_does_not_auto_close(self) -> None:
        import tempfile

        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            intros = root / "introspections"
            intros.mkdir()
            path = intros / "introspection_astrid_llm_1783325217.txt"
            path.write_text("Observed:\nstatic terms\n")
            records, cutoff = inventory_records(
                intros,
                path.name,
                candidate_sources={"changelog": "handled 1783325217", "feedback_ledger": ""},
            )
            status = status_from_records(records, cutoff)
            record = status["artifacts"]["introspection_astrid_llm_1783325217"]

        self.assertTrue(record["candidate_evidence"])
        self.assertEqual(record["status"], "unread")
        self.assertFalse(record["fully_addressed"])

    def test_counter_audit_separates_canonical_remaining_from_all_pending(self) -> None:
        canonical = {
            "introspection_id": "introspection_astrid_llm_2",
            "filename": "introspection_astrid_llm_2.txt",
            "timestamp": 2,
            "artifact_kind": "canonical_introspection",
            "source_family": "astrid_llm",
            "path": "/tmp/introspection_astrid_llm_2.txt",
            "sha256": "abc",
            "full_read": True,
            "fully_addressed": True,
            "status": "addressed_change",
        }
        thin = {
            "introspection_id": "thin_introspection_output_astrid_codec_1",
            "filename": "thin_introspection_output_astrid_codec_1.txt",
            "timestamp": 1,
            "artifact_kind": "thin_introspection_output",
            "source_family": "astrid_codec",
            "path": "/tmp/thin_introspection_output_astrid_codec_1.txt",
            "sha256": "def",
            "full_read": False,
            "fully_addressed": False,
            "status": "unread",
        }

        status = materialized_status(
            {
                canonical["introspection_id"]: canonical,
                thin["introspection_id"]: thin,
            },
            cutoff={"cutoff": "introspection_astrid_llm_2.txt", "cutoff_timestamp": 2},
        )
        audit = status["counter_audit"]
        recommended = audit["recommended_final_report_fields"]
        report = report_from_status(status)
        rendered = render_report_markdown(report)

        self.assertEqual(audit["status"], "consistent")
        self.assertEqual(status["summary"]["pending_count"], 1)
        self.assertEqual(recommended["canonical_remaining"], 0)
        self.assertEqual(recommended["noncanonical_pending"], 1)
        self.assertEqual(
            report["status"],
            "all_canonical_introspections_addressed_noncanonical_pending",
        )
        self.assertIn("- canonical_remaining: 0", rendered)
        self.assertIn("- noncanonical_pending: 1", rendered)

    def test_work_items_do_not_make_introspection_fully_addressed(self) -> None:
        import tempfile

        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            state = root / "state"
            artifact = {
                "introspection_id": "introspection_astrid_llm_1",
                "filename": "introspection_astrid_llm_1.txt",
                "timestamp": 1,
                "artifact_kind": "canonical_introspection",
                "source_family": "astrid_llm",
                "path": "/tmp/introspection_astrid_llm_1.txt",
                "sha256": "abc",
                "candidate_evidence": [],
                "excerpt": "x",
            }
            summary = root / "summary.txt"
            claims = root / "claims.json"
            summary.write_text("read carefully")
            claims.write_text(json.dumps({"claims": [{"claim_id": "c001", "summary": "Create a replyable witness card"}]}))
            append_events(
                state,
                [
                    event_inventory_artifact(artifact),
                    record_read_event(artifact["introspection_id"], "codex", summary, claims),
                ],
            )
            status = replay_events(state)
            item = work_item_from_claim(
                status["artifacts"][artifact["introspection_id"]],
                status["artifacts"][artifact["introspection_id"]]["claims"]["c001"],
            )
            append_events(state, [work_item_created_event(item)])
            status = replay_events(state)
            rec = status["artifacts"][artifact["introspection_id"]]

        self.assertFalse(rec["fully_addressed"])
        self.assertEqual(status["work_item_summary"]["active_work_items"], 1)
        self.assertEqual(status["work_items"][item["work_item_id"]]["agency_tier"], 2)

    def test_verified_existing_tier_five_is_not_counted_as_grant_wait(self) -> None:
        now = now_s()
        status = {
            "work_items": {
                "wi_verified": {
                    "work_item_id": "wi_verified",
                    "source_introspection_id": "introspection_a",
                    "claim_id": "c001",
                    "being": "astrid",
                    "title": "verified read-only boundary",
                    "agency_tier": 5,
                    "agency_tier_label": AGENCY_TIER_LABELS[5],
                    "route": agency_route_for_tier(5),
                    "status": "verified_existing",
                    "updated_at": now,
                },
                "wi_waiting": {
                    "work_item_id": "wi_waiting",
                    "source_introspection_id": "introspection_b",
                    "claim_id": "c002",
                    "being": "astrid",
                    "title": "live change waiting",
                    "agency_tier": 5,
                    "agency_tier_label": AGENCY_TIER_LABELS[5],
                    "route": agency_route_for_tier(5),
                    "status": "needs_operator_approval",
                    "updated_at": now - 100,
                },
            }
        }

        summary = work_item_summary(status["work_items"])
        queue = work_queue_items(status, limit=2)

        self.assertEqual(summary["grant_waiting_count"], 1)
        self.assertEqual(queue[0]["work_item_id"], "wi_waiting")

    def test_verified_existing_boundary_language_is_not_a_tier_mismatch(self) -> None:
        now = now_s()
        summary = work_item_summary(
            {
                "wi_verified": {
                    "work_item_id": "wi_verified",
                    "being": "minime",
                    "title": "authority budget remains an explicit continuity-control boundary",
                    "claim_summary": "Existing read-only boundary was verified in source and tests.",
                    "agency_tier": 4,
                    "route": agency_route_for_tier(4),
                    "status": "verified_existing",
                    "updated_at": now,
                },
                "wi_pending": {
                    "work_item_id": "wi_pending",
                    "being": "minime",
                    "title": "change live regulator control",
                    "claim_summary": "Apply a live controller mutation.",
                    "agency_tier": 4,
                    "route": agency_route_for_tier(4),
                    "status": "ready_for_implementation",
                    "updated_at": now,
                },
            }
        )

        self.assertEqual(summary["tier_mismatch_count"], 1)
        self.assertEqual(summary["tier_mismatches"][0]["work_item_id"], "wi_pending")

    def test_explicit_sandbox_disposition_is_not_a_live_tier_mismatch(self) -> None:
        for classification in ("needs_sandbox", "sandbox_routed"):
            with self.subTest(classification=classification):
                summary = work_item_summary(
                    {
                        "wi_sandbox": {
                            "work_item_id": "wi_sandbox",
                            "being": "astrid",
                            "title": "Compare pressure with dispersal and sensory change.",
                            "claim_summary": (
                                "Pressure should be compared with dispersal and sensory "
                                "change rather than inferred from fill alone."
                            ),
                            "claim_classification": classification,
                            "agency_tier": 3,
                            "route": agency_route_for_tier(3),
                            "status": "needs_sandbox",
                            "updated_at": now_s(),
                        }
                    }
                )

                self.assertEqual(summary["tier_mismatch_count"], 0)

    def test_next_queue_prioritizes_unread_before_triaged_pending_work(self) -> None:
        status = status_from_records(
            [
                {
                    "introspection_id": "introspection_newer_pending_2",
                    "filename": "introspection_newer_pending_2.txt",
                    "timestamp": 2,
                    "artifact_kind": "canonical_introspection",
                    "source_family": "astrid_llm",
                    "path": "/tmp/introspection_newer_pending_2.txt",
                    "sha256": "abc",
                    "candidate_evidence": [],
                    "excerpt": "pending",
                },
                {
                    "introspection_id": "introspection_older_unread_1",
                    "filename": "introspection_older_unread_1.txt",
                    "timestamp": 1,
                    "artifact_kind": "canonical_introspection",
                    "source_family": "astrid_llm",
                    "path": "/tmp/introspection_older_unread_1.txt",
                    "sha256": "def",
                    "candidate_evidence": [],
                    "excerpt": "unread",
                },
            ],
            {"cutoff": "introspection_newer_pending_2.txt", "cutoff_timestamp": 2},
        )
        pending = status["artifacts"]["introspection_newer_pending_2"]
        pending["full_read"] = True
        pending["claims"] = {
            "c001": {
                "claim_id": "c001",
                "summary": "needs work",
                "disposition": "triaged_pending_action",
                "evidence": [],
                "rationale": None,
            }
        }
        _derive_status(pending)

        items = queue_items(status, limit=2)

        self.assertEqual(items[0]["introspection_id"], "introspection_older_unread_1")
        self.assertEqual(items[1]["introspection_id"], "introspection_newer_pending_2")

    def test_agency_tier_classifier_routes_authority_boundaries(self) -> None:
        self.assertEqual(agency_tier_for_claim("read-only local research and self-study"), 1)
        self.assertEqual(agency_tier_for_claim("replyable transition witness correspondence card"), 2)
        self.assertEqual(agency_tier_for_claim("sandbox replay simulation before implementation"), 3)
        self.assertEqual(agency_tier_for_claim("rho decay pressure stress test before live tuning"), 3)
        self.assertEqual(agency_tier_for_claim("request semantic_microdose authority budget"), 4)
        self.assertEqual(agency_tier_for_claim("change pressure fill PI controller behavior"), 5)
        self.assertEqual(
            agency_tier_for_claim(
                "Verify fallback texture probabilities shift with entropy, density gradient, pressure, and tail vibrancy instead of using a static word list."
            ),
            1,
        )
        self.assertEqual(
            agency_tier_for_claim(
                "Verify fallback texture changes with entropy, pressure, density, and cascade evidence."
            ),
            1,
        )
        self.assertEqual(
            agency_tier_for_claim(
                "Observe high-entropy alignment using existing telemetry without applying control."
            ),
            1,
        )
        self.assertEqual(
            agency_tier_for_claim(
                "Verify by applying a live pressure control change in the running substrate."
            ),
            5,
        )
        self.assertEqual(
            agency_tier_for_claim(
                "Changing projected dimensions or codec gain requires Tier 5 approval"
            ),
            5,
        )
        self.assertEqual(
            agency_tier_for_claim("The live sampler remains Tier_5 work"),
            5,
        )
        self.assertEqual(
            agency_tier_for_claim(
                "Changing projection gain or basis is Tier 5 live codec/protocol work requiring operator approval and replay evidence"
            ),
            5,
        )
        self.assertEqual(
            agency_tier_for_claim(
                "Applying viscosity previews to damping is Tier 5 Minime control work requiring operator approval"
            ),
            5,
        )
        self.assertEqual(
            agency_tier_for_claim("The authority budget remains Tier 4 work"),
            4,
        )
        self.assertEqual(
            agency_tier_for_claim("replyable transition card without mutating runtime control"),
            2,
        )
        self.assertEqual(
            agency_tier_for_claim(
                "Correspondence preview should preserve late pressure, gradient, lattice, density, or silt anchors without changing delivered letter bodies"
            ),
            2,
        )
        self.assertEqual(
            agency_tier_for_claim(
                "A sandbox/replay mirror stress test should vary mode_packing before any live pressure or control-facing experiment"
            ),
            3,
        )
        self.assertEqual(
            agency_tier_for_claim(
                "Stable-core semantic trickle and distinguishability loss need visible diagnostics so containment versus contact can be reviewed"
            ),
            1,
        )
        self.assertEqual(
            agency_tier_for_claim(
                "Read-only pressure context must not imply causality, reclamp values, grant source authority, or write live state; all authority flags remain false"
            ),
            1,
        )
        self.assertEqual(
            agency_tier_for_claim(
                "Astrid fill-delta and Minime moment-marker correlations should remain read-only audit evidence before any mode or control-facing change."
            ),
            1,
        )
        self.assertEqual(
            agency_tier_for_claim(
                "Asynchronous spectral leakage, reduced mode_packing, and sensory-bus porosity changes are real proposals that require explicit operator approval before runtime mutation"
            ),
            5,
        )
        self.assertEqual(
            agency_tier_for_claim(
                "A semantic_trickle priority lane or increased semantic weighting requires explicit operator approval because it changes live transport and reservoir-priority behavior"
            ),
            5,
        )
        self.assertEqual(
            agency_tier_for_claim(
                "Using declared transition type to shift spectral_cascade_dynamics or overpacked mode_packing would be live runtime influence"
            ),
            5,
        )
        self.assertEqual(
            agency_tier_for_claim(
                "Raising LOCAL_RESEARCH_MAX_ACTIONS, LOOP_RESEARCH_MAX_ACTIONS, AUTHORITY_BUDGET_MAX_SENDS, or similar limits is an authority-surface change requiring steward approval"
            ),
            4,
        )

    def test_work_item_authority_uses_claim_not_source_label(self) -> None:
        item = work_item_from_claim(
            {
                "introspection_id": "introspection_proposal_distance_contact_control_1",
                "filename": "introspection_proposal_distance_contact_control_1.txt",
                "source_family": "proposal_distance_contact_control",
            },
            {
                "claim_id": "c001",
                "summary": (
                    "Pressure should be compared with dispersal and sensory change "
                    "rather than inferred from fill alone."
                ),
                "classification": "needs_sandbox",
            },
        )

        self.assertEqual(item["agency_tier"], 3)
        self.assertEqual(item["status"], "needs_sandbox")
        self.assertEqual(item["route"], "sandbox_replay_or_simulation")
        self.assertIsNone(item["blocked_by"])

    def test_implemented_now_claim_is_not_left_runnable(self) -> None:
        item = work_item_from_claim(
            {
                "introspection_id": "introspection_astrid_codec_1",
                "filename": "introspection_astrid_codec_1.txt",
                "source_family": "astrid_codec",
            },
            {
                "claim_id": "c001",
                "summary": "Expose one bounded read-only projection diagnostic.",
                "classification": "implemented_now",
            },
        )

        self.assertEqual(item["status"], "implemented_awaiting_felt_response")

    def test_claim_metadata_survives_full_read_parsing(self) -> None:
        import tempfile

        with tempfile.TemporaryDirectory() as tmpdir:
            path = Path(tmpdir) / "claims.json"
            path.write_text(
                json.dumps(
                    {
                        "claims": [
                            {
                                "claim_id": "c001",
                                "summary": "Observe the current handoff.",
                                "classification": "needs_sandbox",
                                "disposition": "addressed_change",
                                "authority": "non_live_observation_only",
                            }
                        ]
                    }
                )
            )
            claims = parse_claims_file(path)

        self.assertEqual(claims[0]["classification"], "needs_sandbox")
        self.assertEqual(claims[0]["authority"], "non_live_observation_only")

    def test_claim_metadata_survives_full_read_event_replay(self) -> None:
        import tempfile

        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            state = root / "state"
            summary = root / "summary.txt"
            claims = root / "claims.json"
            summary.write_text("read carefully")
            claims.write_text(
                json.dumps(
                    {
                        "claims": [
                            {
                                "claim_id": "c001",
                                "summary": "Run one bounded replay.",
                                "classification": "observed",
                                "disposition": "addressed_change",
                                "authority": "offline_read_only_replay",
                            }
                        ]
                    }
                )
            )
            artifact = {
                "introspection_id": "introspection_astrid_autonomous_42",
                "filename": "introspection_astrid_autonomous_42.txt",
                "source_family": "astrid_autonomous",
                "path": "/tmp/introspection_astrid_autonomous_42.txt",
                "sha256": "abc",
                "candidate_evidence": [],
                "excerpt": "x",
            }
            append_events(
                state,
                [
                    event_inventory_artifact(artifact),
                    record_read_event(
                        artifact["introspection_id"], "codex", summary, claims
                    ),
                    evidence_event(
                        artifact["introspection_id"],
                        "c001",
                        "steward_note",
                        "docs/steward-notes/test.md",
                        "Bounded verification evidence.",
                    ),
                ],
            )
            replayed = replay_events(state)["artifacts"][artifact["introspection_id"]][
                "claims"
            ]["c001"]
            work_item = work_item_from_claim(artifact, replayed)

        self.assertEqual(replayed["classification"], "observed")
        self.assertEqual(replayed["authority"], "offline_read_only_replay")
        self.assertEqual(work_item["claim_classification"], "observed")
        self.assertEqual(work_item["claim_authority"], "offline_read_only_replay")
        self.assertEqual(len(work_item["evidence_links"]), 1)
        self.assertEqual(
            work_item["evidence_links"][0]["target"],
            "docs/steward-notes/test.md",
        )

    def test_claim_classification_routes_status_without_lowering_live_authority(self) -> None:
        artifact = {
            "introspection_id": "introspection_astrid_llm_1",
            "filename": "introspection_astrid_llm_1.txt",
            "source_family": "astrid_llm",
            "path": "/tmp/introspection_astrid_llm_1.txt",
        }
        sandbox_item = work_item_from_claim(
            artifact,
            {
                "claim_id": "c001",
                "summary": "Observe fallback identity on bounded artifacts.",
                "classification": "needs_sandbox",
                "authority": "non_live_only",
            },
        )
        verified_item = work_item_from_claim(
            artifact,
            {
                "claim_id": "c002",
                "summary": "Verify fallback texture changes with entropy and pressure.",
                "classification": "verified_existing",
            },
        )
        live_item = work_item_from_claim(
            artifact,
            {
                "claim_id": "c003",
                "summary": "Apply a live pressure control change.",
                "classification": "verified_existing",
            },
        )

        self.assertEqual(sandbox_item["agency_tier"], 3)
        self.assertEqual(sandbox_item["status"], "needs_sandbox")
        self.assertEqual(sandbox_item["claim_authority"], "non_live_only")
        self.assertEqual(verified_item["agency_tier"], 1)
        self.assertEqual(verified_item["status"], "verified_existing")
        self.assertEqual(live_item["agency_tier"], 5)
        self.assertEqual(live_item["status"], "needs_operator_approval")

    def test_structured_sandbox_and_authority_classifications_fail_closed(self) -> None:
        artifact = {
            "introspection_id": "introspection_astrid_codec_2",
            "filename": "introspection_astrid_codec_2.txt",
            "source_family": "astrid_codec",
            "path": "/tmp/introspection_astrid_codec_2.txt",
        }
        sandbox_item = work_item_from_claim(
            artifact,
            {
                "claim_id": "c001",
                "summary": "Compare two captured artifacts without live writes.",
                "disposition": "route a bounded offline comparison",
                "classification": "sandbox_routed",
            },
        )
        authority_item = work_item_from_claim(
            artifact,
            {
                "claim_id": "c002",
                "summary": "A future basis migration needs its own boundary.",
                "disposition": "retain as operator approval work",
                "classification": "authority_gated",
            },
        )
        steward_item = work_item_from_claim(
            artifact,
            {
                "claim_id": "c003",
                "summary": "A bounded authority-budget change needs its own boundary.",
                "disposition": "retain as Tier 4 steward approval work",
                "classification": "authority_gated",
            },
        )

        self.assertEqual(sandbox_item["agency_tier"], 3)
        self.assertEqual(sandbox_item["status"], "needs_sandbox")
        self.assertEqual(sandbox_item["route"], "sandbox_replay_or_simulation")
        self.assertEqual(authority_item["agency_tier"], 5)
        self.assertEqual(authority_item["status"], "needs_operator_approval")
        self.assertEqual(authority_item["blocked_by"], "operator_approval")
        self.assertEqual(steward_item["agency_tier"], 4)
        self.assertEqual(steward_item["status"], "needs_steward_grant")
        self.assertEqual(steward_item["blocked_by"], "steward_grant")

    def test_verified_existing_disposition_prevents_historical_should_change_overclassification(
        self,
    ) -> None:
        artifact = {
            "introspection_id": "introspection_minime_sensory_bus_1",
            "filename": "introspection_minime_sensory_bus_1.txt",
            "source_family": "minime_sensory_bus",
            "path": "/tmp/introspection_minime_sensory_bus_1.txt",
        }
        verified_item = work_item_from_claim(
            artifact,
            {
                "claim_id": "c001",
                "summary": (
                    "High-fill surge taper should change continuously rather than "
                    "snap between two weights."
                ),
                "disposition": (
                    "verify the current smoothstep taper and continuity tests "
                    "without changing live sensory behavior"
                ),
                "classification": "verified_existing",
            },
        )
        mislabeled_live_item = work_item_from_claim(
            artifact,
            {
                "claim_id": "c002",
                "summary": "Apply a live pressure control change.",
                "disposition": "verify after applying the live pressure control change",
                "classification": "verified_existing",
            },
        )

        self.assertEqual(verified_item["agency_tier"], 1)
        self.assertEqual(verified_item["status"], "verified_existing")
        self.assertEqual(
            verified_item["claim_disposition"],
            "verify the current smoothstep taper and continuity tests without changing live sensory behavior",
        )
        self.assertEqual(mislabeled_live_item["agency_tier"], 5)
        self.assertEqual(mislabeled_live_item["status"], "needs_operator_approval")

    def test_tier_suggestion_does_not_auto_approve_or_auto_grant(self) -> None:
        artifact = {
            "introspection_id": "introspection_astrid_llm_1",
            "filename": "introspection_astrid_llm_1.txt",
            "source_family": "astrid_llm",
            "path": "/tmp/introspection_astrid_llm_1.txt",
        }
        claim = {"claim_id": "c001", "summary": "change pressure and fill control behavior"}
        item = work_item_from_claim(artifact, claim)

        self.assertEqual(item["agency_tier"], 5)
        self.assertEqual(item["status"], "needs_operator_approval")
        self.assertFalse(item["auto_approved"])
        self.assertFalse(item["fully_addressed_effect"])
        packet = item["authority_boundary_packet"]
        self.assertEqual(packet["authority_class"], "mike_operator_live_substrate")
        self.assertEqual(packet["gate_state"], "proposal_needed")
        self.assertFalse(packet["live_eligible_now"])
        self.assertFalse(packet["auto_approved"])
        self.assertEqual(packet["who_can_change_it"], "Mike/operator")
        packet_v2 = item["authority_boundary_packet_v2"]
        self.assertEqual(packet_v2["schema_version"], 2)
        self.assertEqual(packet_v2["authority_class"], "mike_operator_live_substrate")
        self.assertEqual(packet_v2["lifecycle_state"], "replay_needed")
        self.assertTrue(packet_v2["delta_refs"])
        self.assertEqual(packet_v2["rollout_abort_contract"]["post_change_response_required"], True)
        self.assertFalse(packet_v2["live_eligible_now"])
        self.assertFalse(packet_v2["auto_approved"])

    def test_agency_tier_request_materializes_stricter_gate_without_granting(self) -> None:
        import tempfile

        with tempfile.TemporaryDirectory() as tmpdir:
            state = Path(tmpdir) / "state"
            item = {
                "work_item_id": "wi_test",
                "source_introspection_id": "introspection_proposal_1",
                "claim_id": "c001",
                "agency_tier": 0,
                "agency_tier_label": AGENCY_TIER_LABELS[0],
                "status": "ready_for_implementation",
                "claim_summary": "semantic_trickle priority lane needs operator approval",
                "auto_approved": False,
            }
            append_events(
                state,
                [
                    work_item_created_event(item),
                    agency_tier_request_event(
                        "wi_test",
                        5,
                        "Live semantic priority requires Mike/operator approval.",
                    ),
                ],
            )
            status = replay_events(state)
            updated = status["work_items"]["wi_test"]

        self.assertEqual(updated["agency_tier"], 5)
        self.assertEqual(updated["status"], "needs_operator_approval")
        self.assertEqual(updated["blocked_by"], "operator_approval")
        self.assertFalse(updated.get("auto_approved"))
        self.assertEqual(updated["agency_tier_requests"][0]["status"], "requested")
        packet = updated["authority_boundary_packet"]
        self.assertEqual(packet["authority_class"], "mike_operator_live_substrate")
        self.assertFalse(packet["live_eligible_now"])
        self.assertFalse(packet["auto_approved"])
        packet_v2 = updated["authority_boundary_packet_v2"]
        self.assertEqual(packet_v2["authority_class"], "mike_operator_live_substrate")
        self.assertEqual(packet_v2["lifecycle_state"], "replay_needed")
        self.assertTrue(packet_v2["delta_refs"])
        self.assertFalse(packet_v2["live_eligible_now"])
        self.assertFalse(packet_v2["auto_approved"])
        self.assertTrue(updated["agency_continues"])
        self.assertEqual(updated["agency_preserving_status"], "operator_approval_wait")
        self.assertFalse(updated["live_authority_granted"])

    def test_agency_tier_correction_repairs_verified_overclassification_without_granting(self) -> None:
        import tempfile

        with tempfile.TemporaryDirectory() as tmpdir:
            state = Path(tmpdir)
            item = {
                "work_item_id": "wi_verified_tier_fix",
                "source_introspection_id": "introspection_astrid_llm_1",
                "claim_id": "c001",
                "agency_tier": 5,
                "agency_tier_label": AGENCY_TIER_LABELS[5],
                "status": "verified_existing",
                "claim_summary": "Verify existing fallback weighting with read-only evidence.",
                "blocked_by": "operator_approval",
                "auto_approved": False,
            }
            append_events(
                state,
                [
                    work_item_created_event(item),
                    agency_tier_correction_event(
                        "wi_verified_tier_fix",
                        1,
                        5,
                        "Existing source verification was overclassified as live mutation.",
                    ),
                ],
            )
            status = replay_events(state)
            updated = status["work_items"]["wi_verified_tier_fix"]

        self.assertEqual(updated["agency_tier"], 1)
        self.assertEqual(updated["agency_tier_label"], AGENCY_TIER_LABELS[1])
        self.assertEqual(updated["route"], "self_activated_read_only_research")
        self.assertEqual(updated["status"], "verified_existing")
        self.assertIsNone(updated["blocked_by"])
        self.assertIsNone(updated["authority_boundary_packet"])
        self.assertIsNone(updated["authority_boundary_packet_v2"])
        self.assertFalse(updated["auto_approved"])
        correction = updated["agency_tier_corrections"][0]
        self.assertEqual(correction["previous_tier"], 5)
        self.assertEqual(correction["tier"], 1)
        self.assertFalse(correction["grants_approval"])
        self.assertFalse(correction["live_eligible_now"])

    def test_blocked_needs_steward_preserves_ongoing_agency_language(self) -> None:
        record = {
            "introspection_id": "introspection_astrid_ws_1",
            "filename": "introspection_astrid_ws_1.txt",
            "timestamp": 1,
            "artifact_kind": "canonical_introspection",
            "source_family": "astrid_ws",
            "path": "/tmp/introspection_astrid_ws_1.txt",
            "sha256": "abc",
            "full_read": True,
            "claims": {
                "c001": {
                    "claim_id": "c001",
                    "summary": "live threshold change needs authority",
                    "disposition": "blocked_needs_steward",
                    "evidence": [],
                }
            },
        }
        status = status_from_records([record], cutoff={"cutoff": "introspection_astrid_ws_1.txt"})
        artifact = status["artifacts"]["introspection_astrid_ws_1"]

        self.assertEqual(artifact["status"], "blocked_needs_steward")
        self.assertEqual(artifact["compatibility_status"], "blocked_needs_steward")
        self.assertTrue(artifact["agency_continues"])
        self.assertTrue(artifact["authority_boundary_wait"])
        self.assertEqual(artifact["agency_preserving_status"], "authority_boundary_wait")
        self.assertFalse(artifact["live_authority_granted"])
        self.assertIn("felt_report_refinement", artifact["available_agency_routes"])
        self.assertEqual(status["next_queue"][0]["agency_preserving_status"], "authority_boundary_wait")

    def test_tier_four_steward_wait_is_authority_boundary_not_operator_wait(self) -> None:
        item = {
            "work_item_id": "wi_tier4",
            "source_introspection_id": "introspection_astrid_types_1",
            "claim_id": "c001",
            "agency_tier": 4,
            "agency_tier_label": AGENCY_TIER_LABELS[4],
            "status": "needs_steward_grant",
            "claim_summary": "consequence-bearing but not live substrate work",
            "evidence_links": [
                {
                    "kind": "diagnostic",
                    "target": "docs/steward-notes/tier4_replay.md",
                    "note": "bounded evidence only",
                }
            ],
            "auto_approved": False,
        }
        refresh_work_item_authority_packets(item)
        status = materialized_status(
            {},
            work_items={"wi_tier4": item},
            cutoff={"cutoff": "introspection_astrid_types_1.txt"},
        )
        updated = status["work_items"]["wi_tier4"]

        self.assertTrue(updated["agency_continues"])
        self.assertEqual(updated["agency_preserving_status"], "authority_boundary_wait")
        self.assertFalse(updated["live_authority_granted"])
        self.assertEqual(
            updated["authority_boundary_packet_v2"]["lifecycle_state"],
            "authority_boundary_wait",
        )

    def test_closure_card_is_bounded_and_omits_full_prose(self) -> None:
        item = {
            "work_item_id": "wi_test",
            "source_introspection_id": "introspection_astrid_llm_1",
            "claim_id": "c001",
            "being": "astrid",
            "agency_tier": 2,
            "agency_tier_label": AGENCY_TIER_LABELS[2],
            "status": "implemented_awaiting_felt_response",
            "claim_summary": "word " * 1000,
            "suggested_next": "language artifact evidence",
        }
        text = closure_card_text(item)

        self.assertLess(len(text), 2200)
        self.assertIn("right_to_ignore: true", text)
        self.assertIn("Still Gated", text)
        self.assertIn("## Agency Corridor V1", text)
        self.assertIn("never grants approval", text)
        self.assertNotIn("word " * 200, text)

    def test_tier_five_closure_card_includes_v2_lifecycle_packet(self) -> None:
        item = {
            "work_item_id": "wi_live",
            "source_introspection_id": "introspection_astrid_ws_1",
            "source_filename": "introspection_astrid_ws_1.txt",
            "claim_id": "c001",
            "being": "astrid",
            "agency_tier": 5,
            "agency_tier_label": AGENCY_TIER_LABELS[5],
            "route": agency_route_for_tier(5),
            "status": "needs_operator_approval",
            "claim_summary": "live pressure controller change needs first-class boundary",
            "suggested_next": "sandbox replay and scoped operator approval before live pressure change",
            "evidence_links": [
                {
                    "kind": "diagnostic",
                    "target": "docs/steward-notes/live_pressure_replay.md",
                    "note": "bounded replay evidence only",
                }
            ],
            "closure_cards": [],
            "post_change_response_status": "awaiting",
        }
        refresh_work_item_authority_packets(item)
        text = closure_card_text(item)

        self.assertIn("## Authority Boundary Packet V2", text)
        self.assertIn('"lifecycle_state": "operator_approval_wait"', text)
        self.assertIn('"rollout_abort_contract"', text)
        self.assertIn('"redaction_profile"', text)
        self.assertIn('"live_eligible_now": false', text)
        self.assertIn("## Agency Corridor V1", text)
        card_event, card = closure_card_event(Path("/tmp/introspection_addressing_test"), item, write=False)
        self.assertEqual(card_event["event_type"], "closure_card_emitted")
        self.assertEqual(card["authority_boundary_packet_v2"]["lifecycle_state"], "operator_approval_wait")

    def test_closure_card_batch_appends_and_materializes_once(self) -> None:
        import tempfile
        from unittest import mock

        work_items = [
            {
                "work_item_id": f"wi_batch_{idx}",
                "source_introspection_id": "introspection_astrid_codec_1",
                "claim_id": f"c{idx:03d}",
                "being": "astrid",
                "agency_tier": 0,
                "agency_tier_label": AGENCY_TIER_LABELS[0],
                "status": "verified_existing",
                "claim_summary": f"bounded claim {idx}",
                "suggested_next": "record bounded evidence",
            }
            for idx in (1, 2)
        ]
        replayed = {
            "work_items": {
                item["work_item_id"]: {
                    **item,
                    "closure_cards": [{"work_item_id": item["work_item_id"]}],
                }
                for item in work_items
            }
        }

        with tempfile.TemporaryDirectory() as tmpdir:
            state = Path(tmpdir) / "state"
            with (
                mock.patch(f"{__name__}.append_events") as append_mock,
                mock.patch(f"{__name__}.replay_events", return_value=replayed) as replay_mock,
                mock.patch(f"{__name__}.write_materialized_status") as write_status_mock,
            ):
                payload = emit_closure_card_batch(state, work_items, write=True)

        append_mock.assert_called_once()
        self.assertEqual(len(append_mock.call_args.args[1]), 2)
        replay_mock.assert_called_once_with(state)
        write_status_mock.assert_called_once_with(state, replayed)
        self.assertEqual(payload["events_appended"], 2)
        self.assertEqual(len(payload["closure_cards"]), 2)
        self.assertEqual(
            [row["closure_card_count"] for row in payload["work_item_statuses"]],
            [1, 1],
        )
        self.assertTrue(all(card["right_to_ignore"] for card in payload["closure_cards"]))

    def test_evidence_batch_appends_and_materializes_once(self) -> None:
        import tempfile
        from unittest import mock

        status = {
            "artifacts": {
                "introspection_astrid_ws_1": {
                    "status": "triaged_pending_action",
                    "fully_addressed": False,
                    "proof_missing_claims": ["c001", "c002"],
                    "claims": {"c001": {}, "c002": {}},
                }
            }
        }
        replayed = {
            "artifacts": {
                "introspection_astrid_ws_1": {
                    "status": "addressed_change",
                    "fully_addressed": True,
                    "proof_missing_claims": [],
                    "claims": {"c001": {}, "c002": {}},
                }
            }
        }
        links = {
            "links": [
                {
                    "introspection_id": "introspection_astrid_ws_1",
                    "claim_id": "*",
                    "kind": "test",
                    "target": "test::telemetry",
                    "note": "bounded verification",
                }
            ]
        }

        with tempfile.TemporaryDirectory() as tmpdir:
            state = Path(tmpdir) / "state"
            links_file = Path(tmpdir) / "links.json"
            links_file.write_text(json.dumps(links), encoding="utf-8")
            with (
                mock.patch(f"{__name__}.load_or_replay_status", return_value=status),
                mock.patch(f"{__name__}.append_events") as append_mock,
                mock.patch(f"{__name__}.replay_events", return_value=replayed) as replay_mock,
                mock.patch(f"{__name__}.write_materialized_status") as write_status_mock,
            ):
                payload = link_evidence_batch(state, links_file, write=True)

        append_mock.assert_called_once()
        self.assertEqual(len(append_mock.call_args.args[1]), 2)
        replay_mock.assert_called_once_with(state)
        write_status_mock.assert_called_once_with(state, replayed)
        self.assertEqual(payload["events_appended"], 2)
        self.assertEqual(payload["link_count"], 2)
        self.assertEqual(payload["batch_row_count"], 1)
        self.assertEqual(payload["introspection_count"], 1)
        self.assertTrue(payload["artifact_statuses"][0]["fully_addressed"])
        self.assertEqual(payload["artifact_statuses"][0]["proof_missing_claims"], [])

    def test_close_batch_appends_and_materializes_once(self) -> None:
        import tempfile
        from unittest import mock

        introspection_ids = [
            "introspection_astrid_ws_1",
            "introspection_astrid_codec_2",
        ]
        current = {
            "artifacts": {
                introspection_id: {
                    "status": "triaged_pending_action",
                    "fully_addressed": False,
                    "proof_missing_claims": [],
                }
                for introspection_id in introspection_ids
            }
        }
        replayed = {
            "artifacts": {
                introspection_id: {
                    "status": "addressed_duplicate",
                    "fully_addressed": True,
                    "proof_missing_claims": [],
                }
                for introspection_id in introspection_ids
            }
        }

        with tempfile.TemporaryDirectory() as tmpdir:
            state = Path(tmpdir) / "state"
            with (
                mock.patch(f"{__name__}.load_or_replay_status", return_value=current),
                mock.patch(f"{__name__}.append_events") as append_mock,
                mock.patch(f"{__name__}.replay_events", return_value=replayed) as replay_mock,
                mock.patch(f"{__name__}.write_materialized_status") as write_status_mock,
            ):
                payload = close_batch(
                    state,
                    introspection_ids,
                    "addressed_duplicate",
                    "All claims retain independent evidence routes.",
                    write=True,
                )

        append_mock.assert_called_once()
        self.assertEqual(len(append_mock.call_args.args[1]), 2)
        replay_mock.assert_called_once_with(state)
        write_status_mock.assert_called_once_with(state, replayed)
        self.assertEqual(payload["events_appended"], 2)
        self.assertEqual(payload["close_count"], 2)
        self.assertTrue(
            all(row["fully_addressed"] for row in payload["artifact_statuses"])
        )

    def test_close_batch_rejects_duplicate_introspection_ids(self) -> None:
        with self.assertRaisesRegex(ValueError, "must be unique"):
            close_batch(
                Path("/tmp/introspection_addressing_test"),
                ["introspection_astrid_ws_1", "introspection_astrid_ws_1"],
                "addressed_duplicate",
                "duplicate input",
                write=False,
            )

    def test_full_read_batch_appends_and_materializes_once(self) -> None:
        import tempfile
        from unittest import mock

        introspection_ids = [
            "introspection_astrid_ws_1",
            "introspection_astrid_codec_2",
        ]
        status = {
            "artifacts": {
                introspection_id: {
                    "status": "unread",
                    "fully_addressed": False,
                    "proof_missing_claims": [],
                }
                for introspection_id in introspection_ids
            }
        }
        replayed = {
            "artifacts": {
                introspection_id: {
                    "status": "triaged_pending_action",
                    "fully_addressed": False,
                    "proof_missing_claims": ["c001"],
                }
                for introspection_id in introspection_ids
            }
        }

        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            summary_file = root / "summary.md"
            claims_file = root / "claims.json"
            manifest_file = root / "manifest.json"
            summary_file.write_text("Full bounded summary.", encoding="utf-8")
            claims_file.write_text(
                json.dumps(
                    {
                        "claims": [
                            {
                                "claim_id": "c001",
                                "summary": "A grounded claim.",
                                "disposition": "verify existing behavior",
                                "classification": "verified_existing",
                            }
                        ]
                    }
                ),
                encoding="utf-8",
            )
            manifest_file.write_text(
                json.dumps(
                    {
                        "reads": [
                            {
                                "introspection_id": introspection_id,
                                "reader": "codex-sol",
                                "summary_file": summary_file.name,
                                "claims_file": claims_file.name,
                            }
                            for introspection_id in introspection_ids
                        ]
                    }
                ),
                encoding="utf-8",
            )
            state = root / "state"
            with (
                mock.patch(f"{__name__}.load_or_replay_status", return_value=status),
                mock.patch(f"{__name__}.append_events") as append_mock,
                mock.patch(f"{__name__}.replay_events", return_value=replayed) as replay_mock,
                mock.patch(f"{__name__}.write_materialized_status") as write_status_mock,
            ):
                payload = record_read_batch(state, manifest_file, write=True)

        append_mock.assert_called_once()
        self.assertEqual(len(append_mock.call_args.args[1]), 2)
        replay_mock.assert_called_once_with(state)
        write_status_mock.assert_called_once_with(state, replayed)
        self.assertEqual(payload["events_appended"], 2)
        self.assertEqual(payload["read_count"], 2)
        self.assertEqual(len(payload["artifact_statuses"]), 2)
        self.assertTrue(
            all(
                row["status"] == "triaged_pending_action"
                for row in payload["artifact_statuses"]
            )
        )

    def test_full_read_batch_rejects_duplicate_introspection_ids(self) -> None:
        import tempfile

        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            summary_file = root / "summary.md"
            claims_file = root / "claims.json"
            manifest_file = root / "manifest.json"
            summary_file.write_text("Full bounded summary.", encoding="utf-8")
            claims_file.write_text("Claim one.", encoding="utf-8")
            manifest_file.write_text(
                json.dumps(
                    {
                        "reads": [
                            {
                                "introspection_id": "introspection_astrid_ws_1",
                                "reader": "codex-sol",
                                "summary_file": summary_file.name,
                                "claims_file": claims_file.name,
                            },
                            {
                                "introspection_id": "introspection_astrid_ws_1",
                                "reader": "codex-sol",
                                "summary_file": summary_file.name,
                                "claims_file": claims_file.name,
                            },
                        ]
                    }
                ),
                encoding="utf-8",
            )
            with self.assertRaisesRegex(ValueError, "duplicates introspection"):
                full_read_batch_rows(manifest_file)

    def test_unknown_introspection_close_is_rejected_before_append(self) -> None:
        import tempfile

        with tempfile.TemporaryDirectory() as tmpdir:
            state = Path(tmpdir) / "state"
            state.mkdir()
            status_path(state).write_text(
                json.dumps(
                    {
                        "artifacts": {
                            "introspection_astrid_llm_1": {
                                "introspection_id": "introspection_astrid_llm_1"
                            }
                        }
                    }
                ),
                encoding="utf-8",
            )

            with self.assertRaisesRegex(
                ValueError,
                "unknown introspection id 'introspection_astrid_llm_2'",
            ):
                main(
                    [
                        "--state-dir",
                        str(state),
                        "close",
                        "--id",
                        "introspection_astrid_llm_2",
                        "--status",
                        "addressed_change",
                        "--rationale",
                        "test close",
                        "--write",
                        "--json",
                    ]
                )

            self.assertFalse(event_path(state).exists())

    def test_replay_ignores_corrupt_lines_and_duplicate_inventory(self) -> None:
        import tempfile

        with tempfile.TemporaryDirectory() as tmpdir:
            state = Path(tmpdir) / "state"
            state.mkdir()
            artifact = {
                "introspection_id": "introspection_astrid_llm_1",
                "filename": "introspection_astrid_llm_1.txt",
                "timestamp": 1,
                "artifact_kind": "canonical_introspection",
                "source_family": "astrid_llm",
                "path": "/tmp/introspection_astrid_llm_1.txt",
                "sha256": "abc",
                "candidate_evidence": [],
                "excerpt": "x",
            }
            path = event_path(state)
            path.write_text(
                json.dumps(event_inventory_artifact(artifact))
                + "\nnot json\n"
                + json.dumps(event_inventory_artifact(artifact))
                + "\n"
            )
            status = replay_events(state)

        self.assertEqual(status["summary"]["corrupt_event_lines"], 1)
        self.assertEqual(status["summary"]["total_indexed"], 1)
        self.assertEqual(status["artifacts"][artifact["introspection_id"]]["status"], "unread")


def run_self_tests() -> int:
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(IntrospectionAddressingAuditTests)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1


def print_output(payload: dict[str, Any], *, as_json: bool) -> None:
    normalize_artifact_authority_tree(payload)
    if as_json:
        print(json.dumps(payload, indent=2, sort_keys=True, ensure_ascii=False))
    else:
        report = payload.get("report") if isinstance(payload.get("report"), dict) else payload
        print(render_report_markdown(report), end="")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workspace", type=Path, default=ASTRID_WORKSPACE)
    parser.add_argument("--state-dir", type=Path, default=DEFAULT_STATE_DIR)
    parser.add_argument("--self-test", action="store_true")
    sub = parser.add_subparsers(dest="cmd")

    inventory_p = sub.add_parser("inventory")
    inventory_p.add_argument(
        "--cutoff",
        default=DEFAULT_CUTOFF,
        help="filename/timestamp cutoff, or 'latest' for the greatest canonical timestamp",
    )
    inventory_p.add_argument("--json", action="store_true")
    inventory_p.add_argument("--write", action="store_true")

    project_p = sub.add_parser("project")
    project_p.add_argument("--write", action="store_true")
    project_p.add_argument("--receipt-json", action="store_true")

    next_p = sub.add_parser("next")
    next_p.add_argument("--limit", type=int, default=3)
    next_p.add_argument("--json", action="store_true")
    next_p.add_argument("--markdown", action="store_true")

    read_p = sub.add_parser("record-read")
    read_p.add_argument("--id", required=True)
    read_p.add_argument("--reader", required=True)
    read_p.add_argument("--summary-file", type=Path, required=True)
    read_p.add_argument("--claims-file", type=Path, required=True)
    read_p.add_argument("--write", action="store_true")
    read_p.add_argument("--json", action="store_true")

    read_batch_p = sub.add_parser("record-read-batch")
    read_batch_p.add_argument("--manifest-file", required=True, type=Path)
    read_batch_p.add_argument("--write", action="store_true")
    read_batch_p.add_argument("--json", action="store_true")

    evidence_p = sub.add_parser("link-evidence")
    evidence_p.add_argument("--id", required=True)
    evidence_p.add_argument("--claim-id", required=True)
    evidence_p.add_argument("--kind", required=True, choices=sorted(EVIDENCE_KINDS))
    evidence_p.add_argument("--target", required=True)
    evidence_p.add_argument("--note", default="")
    evidence_p.add_argument("--write", action="store_true")
    evidence_p.add_argument("--json", action="store_true")

    evidence_batch_p = sub.add_parser("link-evidence-batch")
    evidence_batch_p.add_argument("--links-file", required=True, type=Path)
    evidence_batch_p.add_argument("--write", action="store_true")
    evidence_batch_p.add_argument("--json", action="store_true")

    close_p = sub.add_parser("close")
    close_p.add_argument("--id", required=True)
    close_p.add_argument("--status", required=True, choices=sorted(TERMINAL_STATUSES | {"blocked_needs_steward"}))
    close_p.add_argument("--rationale", required=True)
    close_p.add_argument("--write", action="store_true")
    close_p.add_argument("--json", action="store_true")

    close_batch_p = sub.add_parser("close-batch")
    close_batch_p.add_argument(
        "--id",
        dest="introspection_ids",
        action="append",
        required=True,
    )
    close_batch_p.add_argument(
        "--status",
        required=True,
        choices=sorted(TERMINAL_STATUSES | {"blocked_needs_steward"}),
    )
    close_batch_p.add_argument("--rationale", required=True)
    close_batch_p.add_argument("--write", action="store_true")
    close_batch_p.add_argument("--json", action="store_true")

    report_p = sub.add_parser("report")
    report_p.add_argument("--json", action="store_true")
    report_p.add_argument("--markdown", action="store_true")

    audit_p = sub.add_parser("audit-counters")
    audit_p.add_argument("--json", action="store_true")
    audit_p.add_argument("--markdown", action="store_true")

    promote_p = sub.add_parser("promote-work-items")
    promote_p.add_argument("--ids", nargs="*", default=None)
    promote_p.add_argument("--next", type=int, default=0)
    promote_p.add_argument("--write", action="store_true")
    promote_p.add_argument("--json", action="store_true")

    work_queue_p = sub.add_parser("work-queue")
    work_queue_p.add_argument("--limit", type=int, default=20)
    work_queue_p.add_argument("--json", action="store_true")
    work_queue_p.add_argument("--markdown", action="store_true")

    work_status_p = sub.add_parser("set-work-status")
    work_status_p.add_argument("--work-item-id", required=True)
    work_status_p.add_argument("--status", required=True, choices=sorted(WORK_STATUSES))
    work_status_p.add_argument("--note", default="")
    work_status_p.add_argument("--blocked-by", default=None)
    work_status_p.add_argument("--write", action="store_true")
    work_status_p.add_argument("--json", action="store_true")

    work_evidence_p = sub.add_parser("link-work-evidence")
    work_evidence_p.add_argument("--work-item-id", required=True)
    work_evidence_p.add_argument("--kind", required=True, choices=sorted(WORK_EVIDENCE_KINDS))
    work_evidence_p.add_argument("--target", required=True)
    work_evidence_p.add_argument("--note", default="")
    work_evidence_p.add_argument("--write", action="store_true")
    work_evidence_p.add_argument("--json", action="store_true")

    tier_p = sub.add_parser("request-agency-tier")
    tier_p.add_argument("--work-item-id", required=True)
    tier_p.add_argument("--tier", type=int, required=True, choices=sorted(AGENCY_TIER_LABELS))
    tier_p.add_argument("--reason", required=True)
    tier_p.add_argument("--write", action="store_true")
    tier_p.add_argument("--json", action="store_true")

    tier_correction_p = sub.add_parser("correct-agency-tier")
    tier_correction_p.add_argument("--work-item-id", required=True)
    tier_correction_p.add_argument("--tier", type=int, required=True, choices=sorted(AGENCY_TIER_LABELS))
    tier_correction_p.add_argument("--reason", required=True)
    tier_correction_p.add_argument("--write", action="store_true")
    tier_correction_p.add_argument("--json", action="store_true")

    closure_p = sub.add_parser("emit-closure-card")
    closure_p.add_argument(
        "--id",
        dest="work_item_ids",
        action="append",
        required=True,
        help="work item id; repeat to emit a batch with one projection refresh",
    )
    closure_p.add_argument("--write", action="store_true")
    closure_p.add_argument("--deliver", action="store_true")
    closure_p.add_argument("--json", action="store_true")

    response_p = sub.add_parser("record-post-change-response")
    response_p.add_argument("--work-item-id", required=True)
    response_p.add_argument("--status", required=True, choices=sorted(POST_CHANGE_RESPONSE_STATUSES))
    response_p.add_argument("--source", required=True)
    response_p.add_argument("--note", default="")
    response_p.add_argument("--write", action="store_true")
    response_p.add_argument("--json", action="store_true")

    args = parser.parse_args(argv)
    if args.self_test:
        return run_self_tests()
    if args.cmd is None:
        parser.print_help()
        return 2

    state_dir = args.state_dir
    introspections_dir = args.workspace / "introspections"

    if args.cmd == "inventory":
        payload = build_inventory(
            introspections_dir,
            state_dir,
            args.cutoff,
            write=bool(args.write),
        )
        print_output(payload, as_json=bool(args.json))
        return 0

    if args.cmd == "project":
        started = time.monotonic()
        payload = build_inventory(
            introspections_dir,
            state_dir,
            "latest",
            write=bool(args.write),
        )
        print(
            json.dumps(
                projector_receipt(
                    "addressing",
                    {
                        "summary": payload.get("summary", {}),
                        "counter_audit": payload.get("counter_audit", {}),
                    },
                    {
                        "status.json": state_dir / "status.json",
                        "queue.md": state_dir / "queue.md",
                    },
                    started_monotonic=started,
                ),
                indent=2,
                sort_keys=True,
                ensure_ascii=False,
            )
        )
        return 0

    if args.cmd == "next":
        status = load_or_replay_status(state_dir)
        report = report_from_status(status)
        items = queue_items(status, limit=max(1, args.limit)) if status else []
        payload = {"schema": SCHEMA, "report": report, "next_queue": items, "authority_boundary": AUTHORITY_BOUNDARY}
        if args.json:
            print(json.dumps(payload, indent=2, sort_keys=True, ensure_ascii=False))
        else:
            fake_status = dict(status or {})
            fake_status["next_queue"] = items
            print(render_queue_markdown(fake_status, limit=max(1, args.limit)), end="")
        return 0

    if args.cmd == "record-read":
        introspection_artifact_or_error(state_dir, args.id)
        payload = preview_or_write_event(
            state_dir,
            record_read_event(args.id, args.reader, args.summary_file, args.claims_file),
            write=bool(args.write),
        )
        print_output(payload, as_json=True if args.json or not args.write else False)
        return 0

    if args.cmd == "record-read-batch":
        payload = record_read_batch(
            state_dir,
            args.manifest_file,
            write=bool(args.write),
        )
        print_output(payload, as_json=True if args.json or not args.write else False)
        return 0

    if args.cmd == "link-evidence":
        introspection_artifact_or_error(state_dir, args.id)
        payload = preview_or_write_event(
            state_dir,
            evidence_event(args.id, args.claim_id, args.kind, args.target, args.note),
            write=bool(args.write),
        )
        print_output(payload, as_json=True if args.json or not args.write else False)
        return 0

    if args.cmd == "link-evidence-batch":
        payload = link_evidence_batch(
            state_dir,
            args.links_file,
            write=bool(args.write),
        )
        print_output(payload, as_json=True if args.json or not args.write else False)
        return 0

    if args.cmd == "close":
        introspection_artifact_or_error(state_dir, args.id)
        payload = preview_or_write_event(
            state_dir,
            close_event(args.id, args.status, args.rationale),
            write=bool(args.write),
        )
        print_output(payload, as_json=True if args.json or not args.write else False)
        return 0

    if args.cmd == "close-batch":
        payload = close_batch(
            state_dir,
            args.introspection_ids,
            args.status,
            args.rationale,
            write=bool(args.write),
        )
        print_output(payload, as_json=True if args.json or not args.write else False)
        return 0

    if args.cmd == "report":
        report = build_report(state_dir)
        if args.json:
            print(json.dumps(report, indent=2, sort_keys=True, ensure_ascii=False))
        else:
            print(render_report_markdown(report), end="")
        return 0

    if args.cmd == "audit-counters":
        report = build_report(state_dir)
        counter_audit = (
            report.get("counter_audit")
            if isinstance(report.get("counter_audit"), dict)
            else {"schema": "introspection_addressing_counter_audit_v1", "status": "missing"}
        )
        if args.json:
            print(json.dumps(counter_audit, indent=2, sort_keys=True, ensure_ascii=False))
        else:
            recommended = (
                counter_audit.get("recommended_final_report_fields")
                if isinstance(counter_audit.get("recommended_final_report_fields"), dict)
                else {}
            )
            lines = [
                "# Introspection Counter Audit",
                "",
                f"- status: {counter_audit.get('status', 'unknown')}",
                f"- canonical_indexed: {recommended.get('canonical_indexed', 0)}",
                f"- canonical_fully_addressed: {recommended.get('canonical_fully_addressed', 0)}",
                f"- canonical_remaining: {recommended.get('canonical_remaining', 0)}",
                f"- all_artifact_pending: {recommended.get('all_artifact_pending', 0)}",
                f"- noncanonical_pending: {recommended.get('noncanonical_pending', 0)}",
                f"- mismatches: {counter_audit.get('mismatches', [])}",
                f"- note: {counter_audit.get('scope_note', 'not available')}",
            ]
            print("\n".join(lines).rstrip() + "\n", end="")
        return 0

    if args.cmd == "promote-work-items":
        payload = promote_work_items(
            state_dir,
            ids=args.ids,
            next_count=max(0, int(args.next or 0)),
            write=bool(args.write),
        )
        print_output(payload, as_json=True if args.json or not args.write else False)
        return 0

    if args.cmd == "work-queue":
        status = load_or_replay_status(state_dir)
        payload = {
            "schema": SCHEMA,
            "work_item_summary": work_item_summary(
                status.get("work_items") if isinstance(status.get("work_items"), dict) else {}
            ),
            "work_queue": work_queue_items(status, limit=max(1, args.limit)),
            "authority_boundary": AUTHORITY_BOUNDARY,
            "agency_boundary": AGENCY_BOUNDARY,
        }
        if args.json:
            print(json.dumps(payload, indent=2, sort_keys=True, ensure_ascii=False))
        else:
            print(render_work_queue_markdown(status, limit=max(1, args.limit)), end="")
        return 0

    if args.cmd == "set-work-status":
        payload = preview_or_write_event(
            state_dir,
            work_status_event(args.work_item_id, args.status, args.note, args.blocked_by),
            write=bool(args.write),
        )
        print_output(payload, as_json=True if args.json or not args.write else False)
        return 0

    if args.cmd == "link-work-evidence":
        payload = preview_or_write_event(
            state_dir,
            work_evidence_event(args.work_item_id, args.kind, args.target, args.note),
            write=bool(args.write),
        )
        print_output(payload, as_json=True if args.json or not args.write else False)
        return 0

    if args.cmd == "request-agency-tier":
        payload = preview_or_write_event(
            state_dir,
            agency_tier_request_event(args.work_item_id, args.tier, args.reason),
            write=bool(args.write),
        )
        print_output(payload, as_json=True if args.json or not args.write else False)
        return 0

    if args.cmd == "correct-agency-tier":
        _status, item = work_item_or_error(state_dir, args.work_item_id)
        current_tier = int(item.get("agency_tier") or 0)
        current_status = str(item.get("status") or "")
        if args.tier >= current_tier:
            parser.error("correct-agency-tier must lower the current tier; use request-agency-tier to escalate")
        if current_status not in AGENCY_TIER_CORRECTION_STATUSES:
            parser.error(
                "correct-agency-tier is limited to terminal or verified evidence records; "
                "active authority waits cannot be downgraded"
            )
        payload = preview_or_write_event(
            state_dir,
            agency_tier_correction_event(
                args.work_item_id,
                args.tier,
                current_tier,
                args.reason,
            ),
            write=bool(args.write),
        )
        print_output(payload, as_json=True if args.json or not args.write else False)
        return 0

    if args.cmd == "emit-closure-card":
        work_item_ids = list(dict.fromkeys(args.work_item_ids))
        status = load_or_replay_status(state_dir)
        items = status.get("work_items") if isinstance(status.get("work_items"), dict) else {}
        selected: list[dict[str, Any]] = []
        for work_item_id in work_item_ids:
            item = items.get(work_item_id)
            if not isinstance(item, dict):
                parser.error(f"unknown work item id {work_item_id!r}")
            selected.append(item)
        if len(selected) > 1:
            payload = emit_closure_card_batch(
                state_dir,
                selected,
                write=bool(args.write),
                deliver=bool(args.deliver),
            )
            if args.json or not args.write:
                print(json.dumps(payload, indent=2, sort_keys=True, ensure_ascii=False))
            else:
                print(render_report_markdown(build_report(state_dir)), end="")
            return 0

        item = selected[0]
        event, card = closure_card_event(
            state_dir,
            item,
            write=bool(args.write),
            deliver=bool(args.deliver),
        )
        payload = preview_or_write_event(state_dir, event, write=bool(args.write))
        payload["closure_card"] = card
        if args.json or not args.write:
            print(json.dumps(payload, indent=2, sort_keys=True, ensure_ascii=False))
        else:
            print(render_report_markdown(build_report(state_dir)), end="")
        return 0

    if args.cmd == "record-post-change-response":
        payload = preview_or_write_event(
            state_dir,
            post_change_response_event(args.work_item_id, args.status, args.source, args.note),
            write=bool(args.write),
        )
        print_output(payload, as_json=True if args.json or not args.write else False)
        return 0

    parser.print_help()
    return 2


if __name__ == "__main__":
    sys.exit(main())
