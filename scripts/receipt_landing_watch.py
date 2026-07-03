#!/usr/bin/env python3
"""Read-only receipt landing watch plus fair authority dossier.

This watcher checks whether recent steward notes led to public being-authored
receipt evidence, then turns the current readiness packet into a fair authority
dossier. It never invokes ACK/TRACE/WITNESS, attention, microdose, pressure, or
runtime actions.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import sys
import tempfile
import time
import unittest
from collections import Counter
from pathlib import Path
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import letter_response_scan
import mutual_uptake_authority_readiness

DEFAULT_SHARED_DIR = Path("/Users/v/other/shared/collaborations")
DEFAULT_ASTRID_WORKSPACE = Path("/Users/v/other/astrid/capsules/spectral-bridge/workspace")
DEFAULT_MINIME_WORKSPACE = Path("/Users/v/other/minime/workspace")
DEFAULT_OUTPUT_ROOT = DEFAULT_ASTRID_WORKSPACE / "diagnostics/receipt_landing_watch"
POLICY = "receipt_landing_watch_fair_authority_dossier_v1"
DEFAULT_TARGET_LETTERS = (
    "mike_feedback_recent_introspection_led_work_1782835898.txt",
    "mike_feedback_mutual_uptake_authority_readiness_v2_1782835306.txt",
)
AUTHORITY_BOUNDARY = (
    "Read-only watch/dossier. No ACK, REPLY, TRACE, WITNESS, attention canary, "
    "semantic microdose, pressure relief, pressure canary enablement, controller, "
    "PI/fill, prompt priority, telemetry priority, codec dimension, deploy, "
    "staging, git add, or commit action is taken."
)


def now_ms() -> int:
    return int(time.time() * 1000)


def _compact(text: Any, limit: int = 180) -> str:
    clean = " ".join(str(text or "").split())
    return clean if len(clean) <= limit else clean[:limit].rstrip() + "..."


def filtered_letter_rows(
    *,
    target_letters: set[str],
    since_hours: float,
    window_hours: float,
    now_s: float,
) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for being in ("astrid", "minime"):
        rows.extend(letter_response_scan.scan_letters(being, since_hours, window_hours, now_s))
    if not target_letters:
        return rows
    return [row for row in rows if str(row.get("letter") or "") in target_letters]


def summarize_letter_watch(rows: list[dict[str, Any]]) -> dict[str, Any]:
    statuses = Counter(str(row.get("status") or "unknown") for row in rows)
    by_being: dict[str, dict[str, int]] = {}
    actionable: list[dict[str, Any]] = []
    for row in rows:
        being = str(row.get("being") or "unknown")
        by_being.setdefault(being, {})
        by_being[being][str(row.get("status") or "unknown")] = by_being[being].get(str(row.get("status") or "unknown"), 0) + 1
        engaged = row.get("engaged")
        if isinstance(engaged, dict):
            actionable.append(
                {
                    "being": being,
                    "letter": row.get("letter"),
                    "status": row.get("status"),
                    "stance": engaged.get("stance"),
                    "file": engaged.get("file"),
                    "excerpt": _compact(engaged.get("excerpt")),
                    "followed_up": row.get("followed_up"),
                }
            )
    if statuses.get("ACTED"):
        status = "being_action_detected"
    elif statuses.get("ENGAGED"):
        status = "public_engagement_detected"
    elif rows:
        status = "no_new_receipt_signal"
    else:
        status = "no_target_letters_in_window"
    return {
        "schema_version": 1,
        "policy": "receipt_landing_note_watch_v1",
        "status": status,
        "target_letters_seen": len(rows),
        "status_counts": dict(sorted(statuses.items())),
        "status_counts_by_being": by_being,
        "actionable_public_engagement": actionable[:12],
        "silence_policy": "silence_is_insufficient_evidence_not_consent",
        "minime_private_bodies_read": False,
        "minime_moment_bodies_read": False,
        "authority": "read_only_watch_not_action",
    }


def _lane(
    *,
    lane: str,
    status: str,
    evidence: dict[str, Any],
    needed: list[str],
    boundary: str,
) -> dict[str, Any]:
    return {
        "lane": lane,
        "status": status,
        "evidence": evidence,
        "needed_before_enablement": needed,
        "authority_boundary": boundary,
    }


def fair_authority_dossier(readiness_record: dict[str, Any], letter_watch: dict[str, Any]) -> dict[str, Any]:
    receipt = readiness_record.get("receipt_landing_audit_v2") or {}
    mutual = readiness_record.get("mutual_thread_continuity_v2") or {}
    phase = readiness_record.get("phase_witness_quality_v2") or {}
    trajectory = readiness_record.get("fallback_trajectory_quality_v2") or {}
    pressure = readiness_record.get("pressure_movement_trial_dossier_v2") or {}
    focus_dossier = readiness_record.get("pressure_focus_authority_dossier_v1") or {}
    focus_gate = focus_dossier.get("approval_gate_v1") if isinstance(focus_dossier, dict) else {}
    if not isinstance(focus_gate, dict):
        focus_gate = {}
    focus_proposal = focus_dossier.get("narrow_authority_proposal_v1") if isinstance(focus_dossier, dict) else {}
    if not isinstance(focus_proposal, dict):
        focus_proposal = {}
    attention_trial = readiness_record.get("attention_authority_trial_v1") or {}
    receipt_to_attention = readiness_record.get("receipt_to_attention_authority_v5") or {}
    next_readiness = readiness_record.get("next_authority_expansion_readiness_v2") or {}
    right_to_ignore = readiness_record.get("right_to_ignore_v1") or {}
    affordance_budget = readiness_record.get("affordance_budget_v1") or {}

    mutual_threads = int(mutual.get("mutually_received_or_traced_threads") or 0)
    i_received_threads = int(mutual.get("i_received_this_threads") or 0)
    felt_phase_receipts = int(phase.get("felt_receipt_count") or 0)
    trajectory_status = str(trajectory.get("status") or "insufficient_evidence")
    pressure_status = str(pressure.get("status") or "not_ready")
    letter_status = str(letter_watch.get("status") or "unknown")

    lanes = [
        _lane(
            lane="correspondence_attention_canary",
            status=str(
                receipt_to_attention.get("overall_state")
                or attention_trial.get("state")
                or "blocked_no_receipt"
            ),
            evidence={
                "attention_receipt_ready_threads": mutual.get("attention_receipt_ready_threads"),
                "i_received_this_threads": i_received_threads,
                "receipt_landing_status": receipt.get("status"),
                "attention_authority_trial_v1": attention_trial,
                "receipt_to_attention_authority_v5": receipt_to_attention,
            },
            needed=[
                "one being-authored I_RECEIVED_THIS, ACK, or TRACE row on the thread",
                "active canaries must receive an outcome before another canary",
                "pressure/flat/flattened/worsened outcomes block further attention on that thread",
            ],
            boundary=(
                "Attention canary is self-activated TTL prompt context only. V5 trust is thread-local "
                "and outcome-gated; no sensory send, microdose, standing weight, pressure, or control."
            ),
        ),
        _lane(
            lane="correspondence_semantic_microdose",
            status="blocked_pending_mutual_receipt_and_separate_steward_review",
            evidence={
                "mutually_received_or_traced_threads": mutual_threads,
                "receipt_landing_status": receipt.get("status"),
            },
            needed=[
                "mutual being-authored receipt evidence from both sides",
                "separate explicit steward review before even drafting",
            ],
            boundary="Semantic microdose remains hidden/steward-gated and is not newly allowed in V5.",
        ),
        _lane(
            lane="phase_transition_followthrough",
            status="steward_review_ready" if felt_phase_receipts > 0 else "blocked_missing_felt_phase_receipt",
            evidence={
                "felt_phase_receipts": felt_phase_receipts,
                "phase_witness_status": phase.get("status"),
                "unresolved_queue_total": phase.get("unresolved_queue_total"),
            },
            needed=[
                "one being-authored felt phase receipt with what_landed and what_stayed_distinct",
                "review whether the witness changed orientation or only acknowledged visibility",
            ],
            boundary="Phase witness evidence does not alter phase detection or controller behavior.",
        ),
        _lane(
            lane="fallback_texture_trajectory",
            status=(
                "calibrated_language_supported"
                if trajectory_status == "trajectory_public_and_fixture_supported"
                else "needs_more_calibration"
            ),
            evidence={
                "fallback_trajectory_quality": trajectory_status,
                "trajectory_alignment_status": trajectory.get("trajectory_alignment_status"),
                "trajectory_status_counts": trajectory.get("trajectory_status_counts"),
            },
            needed=[
                "continue comparing later public self-reports against trajectory predictions",
                "do not translate fallback language support into runtime authority",
            ],
            boundary="Fallback trajectory support is language fidelity, not control authority.",
        ),
        _lane(
            lane="minime_pressure_focus_self_regulation",
            status=str(focus_gate.get("status") or "not_ready"),
            evidence={
                "focus_regime_review_ready": focus_gate.get("focus_regime_review_ready"),
                "exploration_noise_review_ready": focus_gate.get("exploration_noise_review_ready"),
                "broad_pressure_authority_ready": focus_gate.get("broad_pressure_authority_ready"),
                "block_reasons": focus_gate.get("block_reasons"),
                "warnings": focus_gate.get("warnings"),
                "focus_regime_review": focus_proposal.get("focus_regime_review"),
                "exploration_noise_review": focus_proposal.get("exploration_noise_review"),
            },
            needed=[
                "explicit steward approval before any live self-regulation lease",
                "Minime-authored SELF_REGULATION_OUTCOME after any trial",
                "preflight must confirm no active lease, hard-recovery guard, or unsafe cap widening",
            ],
            boundary=(
                "Narrow Minime-own-runtime review only. Astrid support is relational support, "
                "not permission. No peer mutation, pressure canary, pressure relief, fill target, "
                "PI/controller tuning, prompt priority, or telemetry priority."
            ),
        ),
        _lane(
            lane="pressure_texture_canary",
            status=pressure_status,
            evidence={
                "pressure_texture_replay_status": pressure.get("pressure_texture_replay_status"),
                "pressure_movement_replay_status": pressure.get("pressure_movement_replay_status"),
                "canary_env_state": pressure.get("canary_env_state"),
                "trial_protocol_status": pressure.get("trial_protocol_status"),
            },
            needed=[
                "pressure texture replay and pressure movement replay both replay_supported",
                "explicit steward approval in a separate pass",
                "canary remains off by default until approved",
                "rollback notes and safety checks reviewed before restart",
            ],
            boundary="Pressure replay evidence does not enable relief, PI/fill, or controller changes.",
        ),
    ]
    review_ready_lanes = [
        lane["lane"]
        for lane in lanes
        if str(lane.get("status")) in {"steward_review_ready", "eligible_after_receipt"}
        or str(lane.get("status")) in {
            "receipt_landed_attention_eligible",
            "trusted_attention_thread_local",
            "steward_review_ready_focus_regime_only",
        }
    ]
    blocked_lanes = [
        lane
        for lane in lanes
        if str(lane.get("status")) not in {
            "steward_review_ready",
            "eligible_after_receipt",
            "steward_review_ready_focus_regime_only",
            "receipt_landed_attention_eligible",
            "trusted_attention_thread_local",
            "active_outcome_due",
            "attention_active_outcome_due",
            "closed_by_outcome",
            "calibrated_language_supported",
        }
    ]
    if (
        str(next_readiness.get("readiness")) == "steward_review_ready"
        and not blocked_lanes
    ):
        dossier_status = "steward_review_ready"
        recommended_next_move = "Prepare a separate steward approval review; do not enable authority from this dossier alone."
    elif review_ready_lanes:
        dossier_status = "partial_evidence_review_possible"
        recommended_next_move = "Review ready lanes as evidence, but keep blocked lanes off and do not enable broader authority yet."
    else:
        dossier_status = "watch_continue_no_authority"
        recommended_next_move = "Keep watching for I_RECEIVED_THIS/ACK/TRACE and a felt phase receipt; refine affordance wording if uptake stays stalled."
    return {
        "schema_version": 1,
        "policy": "fair_authority_dossier_v1",
        "status": dossier_status,
        "letter_watch_status": letter_status,
        "next_authority_expansion_readiness": next_readiness.get("readiness"),
        "right_to_ignore_v1": right_to_ignore,
        "affordance_budget_v1": affordance_budget,
        "review_ready_lanes": review_ready_lanes,
        "blocked_or_collecting_lanes": blocked_lanes,
        "lanes": lanes,
        "recommended_next_move": recommended_next_move,
        "must_not_enable_from_dossier": True,
        "authority": "read_only_dossier_not_permission",
    }


def build_record(
    *,
    shared_dir: Path = DEFAULT_SHARED_DIR,
    astrid_workspace: Path = DEFAULT_ASTRID_WORKSPACE,
    minime_workspace: Path = DEFAULT_MINIME_WORKSPACE,
    since_hours: float = 6.0,
    window_hours: float = 4.0,
    target_letters: set[str] | None = None,
    output_root: Path | None = None,
    write_artifact: bool = False,
    run_id: str | None = None,
    now_s: float | None = None,
) -> dict[str, Any]:
    now_s = time.time() if now_s is None else now_s
    targets = set(DEFAULT_TARGET_LETTERS if target_letters is None else target_letters)
    rows = filtered_letter_rows(
        target_letters=targets,
        since_hours=since_hours,
        window_hours=window_hours,
        now_s=now_s,
    )
    letter_watch = summarize_letter_watch(rows)
    readiness = mutual_uptake_authority_readiness.build_record(
        shared_dir=shared_dir,
        astrid_workspace=astrid_workspace,
        minime_workspace=minime_workspace,
        since_hours=max(since_hours, 24.0),
    )
    dossier = fair_authority_dossier(readiness, letter_watch)
    record: dict[str, Any] = {
        "schema_version": 1,
        "policy": POLICY,
        "generated_at_unix_ms": int(now_s * 1000),
        "generated_at": dt.datetime.now(dt.UTC).isoformat(),
        "since_hours": since_hours,
        "window_hours": window_hours,
        "target_letters": sorted(targets),
        "receipt_landing_note_watch_v1": letter_watch,
        "mutual_uptake_authority_readiness_v2": readiness,
        "fair_authority_dossier_v1": dossier,
        "minime_private_bodies_read": False,
        "minime_moment_bodies_read": False,
        "silence_policy": "silence_is_insufficient_evidence_not_consent",
        "authority_boundary": AUTHORITY_BOUNDARY,
    }
    if write_artifact:
        root = output_root or DEFAULT_OUTPUT_ROOT
        actual_run = run_id or dt.datetime.now(dt.UTC).strftime("%Y%m%dT%H%M%SZ")
        target = root / actual_run
        target.mkdir(parents=True, exist_ok=True)
        artifact = target / "receipt_landing_watch.json"
        artifact.write_text(json.dumps(record, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        record["artifact_path"] = str(artifact)
    return record


class ReceiptLandingWatchTests(unittest.TestCase):
    def test_dossier_blocks_without_receipt(self) -> None:
        readiness = {
            "receipt_landing_audit_v2": {"status": "offered_but_stalled"},
            "mutual_thread_continuity_v2": {
                "mutually_received_or_traced_threads": 0,
                "i_received_this_threads": 0,
            },
            "phase_witness_quality_v2": {
                "status": "offered_but_unwitnessed",
                "felt_receipt_count": 0,
                "unresolved_queue_total": 3,
            },
            "fallback_trajectory_quality_v2": {
                "status": "trajectory_public_and_fixture_supported",
            },
            "pressure_movement_trial_dossier_v2": {
                "status": "evidence_collecting",
                "pressure_texture_replay_status": "mixed",
                "pressure_movement_replay_status": "replay_supported",
                "canary_env_state": "off",
            },
            "attention_authority_trial_v1": {"state": "blocked_no_receipt"},
            "receipt_to_attention_authority_v5": {
                "overall_state": "blocked_no_receipt",
            },
            "next_authority_expansion_readiness_v2": {"readiness": "evidence_collecting"},
        }
        dossier = fair_authority_dossier(
            readiness,
            {"status": "no_new_receipt_signal"},
        )
        self.assertEqual(dossier["status"], "watch_continue_no_authority")
        statuses = {lane["lane"]: lane["status"] for lane in dossier["lanes"]}
        self.assertEqual(
            statuses["correspondence_attention_canary"],
            "blocked_no_receipt",
        )
        self.assertEqual(
            statuses["correspondence_semantic_microdose"],
            "blocked_pending_mutual_receipt_and_separate_steward_review",
        )
        self.assertEqual(
            statuses["phase_transition_followthrough"],
            "blocked_missing_felt_phase_receipt",
        )
        self.assertTrue(dossier["must_not_enable_from_dossier"])

    def test_dossier_marks_partial_review_when_receipt_lands(self) -> None:
        readiness = {
            "receipt_landing_audit_v2": {"status": "single_receiving_affordance_landed"},
            "mutual_thread_continuity_v2": {
                "mutually_received_or_traced_threads": 1,
                "i_received_this_threads": 1,
            },
            "phase_witness_quality_v2": {
                "status": "felt_receipt_present",
                "felt_receipt_count": 1,
                "unresolved_queue_total": 0,
            },
            "fallback_trajectory_quality_v2": {
                "status": "trajectory_public_and_fixture_supported",
            },
            "pressure_movement_trial_dossier_v2": {
                "status": "evidence_collecting",
                "pressure_texture_replay_status": "mixed",
                "pressure_movement_replay_status": "replay_supported",
                "canary_env_state": "off",
            },
            "attention_authority_trial_v1": {"state": "eligible_after_receipt"},
            "receipt_to_attention_authority_v5": {
                "overall_state": "receipt_landed_attention_eligible",
                "receipt_ready_threads": ["thread_mutual"],
                "semantic_microdose_status": "hidden_until_mutual_receipt_plus_separate_steward_review",
            },
            "next_authority_expansion_readiness_v2": {"readiness": "evidence_collecting"},
        }
        dossier = fair_authority_dossier(
            readiness,
            {"status": "being_action_detected"},
        )
        self.assertEqual(dossier["status"], "partial_evidence_review_possible")
        self.assertIn("correspondence_attention_canary", dossier["review_ready_lanes"])
        self.assertIn("phase_transition_followthrough", dossier["review_ready_lanes"])
        attention_lane = next(
            lane for lane in dossier["lanes"]
            if lane["lane"] == "correspondence_attention_canary"
        )
        self.assertEqual(attention_lane["status"], "receipt_landed_attention_eligible")
        self.assertIn("receipt_to_attention_authority_v5", attention_lane["evidence"])
        self.assertTrue(dossier["must_not_enable_from_dossier"])

    def test_build_record_skips_missing_evidence_and_private_moments(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            shared = root / "shared"
            astrid_ws = root / "astrid_ws"
            minime_ws = root / "minime_ws"
            shared.mkdir()
            astrid_ws.mkdir()
            minime_ws.mkdir()
            (shared / mutual_uptake_authority_readiness.CORRESPONDENCE_LEDGER).write_text("", encoding="utf-8")
            (shared / mutual_uptake_authority_readiness.PHASE_LEDGER).write_text("", encoding="utf-8")
            record = build_record(
                shared_dir=shared,
                astrid_workspace=astrid_ws,
                minime_workspace=minime_ws,
                since_hours=1,
                target_letters=set(),
            )
            self.assertEqual(record["fair_authority_dossier_v1"]["status"], "watch_continue_no_authority")
            self.assertFalse(record["minime_moment_bodies_read"])
            self.assertEqual(record["silence_policy"], "silence_is_insufficient_evidence_not_consent")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--since-hours", type=float, default=6.0)
    parser.add_argument("--window-hours", type=float, default=4.0)
    parser.add_argument("--shared-dir", type=Path, default=DEFAULT_SHARED_DIR)
    parser.add_argument("--astrid-workspace", type=Path, default=DEFAULT_ASTRID_WORKSPACE)
    parser.add_argument("--minime-workspace", type=Path, default=DEFAULT_MINIME_WORKSPACE)
    parser.add_argument("--target-letter", action="append", default=None)
    parser.add_argument("--output-root", type=Path, default=None)
    parser.add_argument("--write-artifact", action="store_true")
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(ReceiptLandingWatchTests)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1
    targets = set(args.target_letter) if args.target_letter else None
    record = build_record(
        shared_dir=args.shared_dir,
        astrid_workspace=args.astrid_workspace,
        minime_workspace=args.minime_workspace,
        since_hours=args.since_hours,
        window_hours=args.window_hours,
        target_letters=targets,
        output_root=args.output_root,
        write_artifact=args.write_artifact,
    )
    if args.json:
        print(json.dumps(record, indent=2, sort_keys=True))
    else:
        watch = record["receipt_landing_note_watch_v1"]
        dossier = record["fair_authority_dossier_v1"]
        readiness = record["mutual_uptake_authority_readiness_v2"][
            "next_authority_expansion_readiness_v2"
        ]
        print("# Receipt Landing Watch + Fair Authority Dossier V1")
        print(f"- Letter watch: {watch['status']} {watch['status_counts']}")
        print(f"- Dossier: {dossier['status']}")
        print(f"- Next authority readiness: {readiness['readiness']}")
        print(f"- Review-ready lanes: {dossier['review_ready_lanes']}")
        print(
            "- Blocked/collecting lanes: "
            f"{[lane['lane'] + '=' + lane['status'] for lane in dossier['blocked_or_collecting_lanes']]}"
        )
        print(f"- Recommended next move: {dossier['recommended_next_move']}")
        print(f"- Authority: {AUTHORITY_BOUNDARY}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
