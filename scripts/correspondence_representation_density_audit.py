#!/usr/bin/env python3
"""Read-only audit of correspondence representation density and visibility.

This composes existing correspondence and affordance evidence without ranking,
pruning, dispatching, or changing any live prompt, telemetry, or control path.
It reports machine representation density only; it cannot establish felt
pressure, landing, coherence, or relief.
"""

from __future__ import annotations

import argparse
import json
import tempfile
import time
import unittest
from collections import Counter
from pathlib import Path
from typing import Any

import affordance_landing_review
import correspondence_uptake_probe


DEFAULT_SHARED_DIR = Path("/Users/v/other/shared/collaborations")
POLICY = "correspondence_representation_density_audit_v1"
LEDGER_FILE = "correspondence_v1.jsonl"
STATE_FILE = "correspondence_state_v1.json"
BUFFER_FILE = "correspondence_buffer_v1.json"


def now_ms() -> int:
    return int(time.time() * 1000)


def _read_json(path: Path) -> dict[str, Any] | None:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None
    return value if isinstance(value, dict) else None


def _latest_collaboration_artifact(
    shared_dir: Path, filename: str
) -> tuple[Path | None, dict[str, Any] | None]:
    candidates: list[tuple[int, str, Path, dict[str, Any]]] = []
    for path in shared_dir.glob(f"coll_*/{filename}"):
        payload = _read_json(path)
        if payload is None:
            continue
        try:
            updated = int(payload.get("updated_t_ms") or 0)
        except (TypeError, ValueError):
            updated = 0
        candidates.append((updated, str(path), path, payload))
    if not candidates:
        return None, None
    _, _, path, payload = max(candidates)
    return path, payload


def _ratio(numerator: int, denominator: int) -> float | None:
    if denominator <= 0:
        return None
    return round(numerator / denominator, 6)


def _projection_visibility(
    path: Path | None, payload: dict[str, Any] | None
) -> dict[str, Any]:
    value = payload or {}
    return {
        "path": str(path) if path else None,
        "present": payload is not None,
        "updated_t_ms": value.get("updated_t_ms"),
        "active_thread_id": value.get("active_thread_id"),
        "top_level_fields": {
            "active_correspondence_thread_clarity_v1": (
                "active_correspondence_thread_clarity_v1" in value
            ),
            "shared_context_buffer_v1": "shared_context_buffer_v1" in value,
            "correspondence_relation_axes_v4": (
                "correspondence_relation_axes_v4" in value
            ),
            "affordance_budget_v1": "affordance_budget_v1" in value,
        },
    }


def audit(
    *,
    shared_dir: Path,
    since_hours: float = 24.0,
    generated_at_unix_ms: int | None = None,
    uptake_payload: dict[str, Any] | None = None,
    landing_payload: dict[str, Any] | None = None,
) -> dict[str, Any]:
    generated = generated_at_unix_ms or now_ms()
    records = correspondence_uptake_probe.read_jsonl(shared_dir / LEDGER_FILE)
    if uptake_payload is None:
        uptake_payload = correspondence_uptake_probe.audit(
            since_hours=since_hours,
            shared_dir=shared_dir,
            astrid_workspace=correspondence_uptake_probe.DEFAULT_ASTRID_WORKSPACE,
            minime_workspace=correspondence_uptake_probe.DEFAULT_MINIME_WORKSPACE,
        )
    if landing_payload is None:
        landing_payload = affordance_landing_review.review(shared_dir, since_hours)

    uptake = uptake_payload.get("uptake") or {}
    native = uptake.get("native_thread_continuity_v3") or {}
    axes = native.get("correspondence_relation_axes_v4") or {}
    active_thread_id = str(native.get("thread_id") or "")
    active_rows = [
        row
        for row in records
        if active_thread_id
        and str(row.get("thread_id") or "") == active_thread_id
    ]
    active_counts = Counter(str(row.get("record_type") or "unknown") for row in active_rows)
    thread_ids = {
        str(row.get("thread_id"))
        for row in records
        if str(row.get("thread_id") or "").strip()
    }

    landing_v3 = landing_payload.get("affordance_landing_review_v3") or {}
    landing_v35 = landing_payload.get("affordance_landing_review_v35") or {}
    budget = landing_v3.get("affordance_budget_v1") or {}
    shown = int(budget.get("shown") or 0)
    hidden = int(budget.get("hidden_by_budget") or 0)
    offered = shown + hidden
    native_threads = landing_v35.get("native_threads") or []
    active_landing = next(
        (
            row
            for row in native_threads
            if str(row.get("thread_id") or "") == active_thread_id
        ),
        native_threads[0] if native_threads else {},
    )

    state_path, state_payload = _latest_collaboration_artifact(shared_dir, STATE_FILE)
    buffer_path, buffer_payload = _latest_collaboration_artifact(shared_dir, BUFFER_FILE)

    return {
        "schema_version": 1,
        "policy": POLICY,
        "generated_at_unix_ms": generated,
        "shared_dir": str(shared_dir),
        "ledger_density": {
            "records_total": len(records),
            "threads_total": len(thread_ids),
            "record_type_counts": dict(
                sorted(
                    Counter(
                        str(row.get("record_type") or "unknown") for row in records
                    ).items()
                )
            ),
            "raw_body_or_preview_emitted": False,
        },
        "active_thread_evidence": {
            "thread_id": active_thread_id or None,
            "records_total": len(active_rows),
            "record_type_counts": dict(sorted(active_counts.items())),
            "reply_depth": active_landing.get("reply_depth"),
            "receipt_landing_status": active_landing.get("latest_landing_status"),
            "receipt_stall_reason": active_landing.get("stall_reason"),
            "continuity": (axes.get("continuity_axis") or {}).get("state"),
            "continuity_basis": (axes.get("continuity_axis") or {}).get("basis"),
            "mutual_address": (axes.get("mutual_address_axis") or {}).get("state"),
            "pressure_effect": (axes.get("causality_axis") or {}).get(
                "pressure_effect"
            ),
            "felt_effect": (axes.get("causality_axis") or {}).get("felt_effect"),
        },
        "affordance_exposure": {
            "offered_total": offered,
            "shown": shown,
            "hidden_by_budget": hidden,
            "shown_fraction": _ratio(shown, offered),
            "shown_by_category": budget.get("shown_by_category") or {},
            "hidden_by_category": budget.get("hidden_by_category") or {},
            "limits": budget.get("limits") or {},
            "optional": bool(budget.get("optional", True)),
            "silence": budget.get("silence") or "neutral",
        },
        "projection_visibility": {
            "state": _projection_visibility(state_path, state_payload),
            "buffer": _projection_visibility(buffer_path, buffer_payload),
            "absence_meaning": (
                "A missing top-level field is projection-visibility evidence only; "
                "it does not prove source absence, deployment absence, or felt effect."
            ),
        },
        "density_buffer_proposal_boundary": {
            "implemented": False,
            "rows_pruned": False,
            "threads_ranked_by_resonance": False,
            "prompt_or_telemetry_weight_changed": False,
            "pressure_effect_inferred": False,
            "felt_relief_inferred": False,
            "authority_route": (
                "Live resonance ranking, history pruning, prompt weighting, telemetry "
                "priority, or pressure-responsive selection requires separate Mike/operator approval."
            ),
        },
        "authority_boundary": (
            "Read-only composition of public correspondence ledger and derived audit metadata. "
            "No message, ACK, TRACE, reply, prompt, model, history selection, telemetry, "
            "pressure, fill, PI, controller, reservoir, peer, deploy, or runtime state is changed."
        ),
    }


def markdown_report(payload: dict[str, Any]) -> str:
    ledger = payload["ledger_density"]
    active = payload["active_thread_evidence"]
    exposure = payload["affordance_exposure"]
    return "\n".join(
        [
            "# Correspondence Representation Density Audit",
            "",
            f"- Ledger rows: {ledger['records_total']}",
            f"- Threads: {ledger['threads_total']}",
            f"- Active thread: {active['thread_id'] or 'none'}",
            f"- Active thread rows: {active['records_total']}",
            f"- Continuity: {active['continuity'] or 'unknown'} ({active['continuity_basis'] or 'unknown'})",
            f"- Mutual address: {active['mutual_address'] or 'unknown'}",
            f"- Affordances shown/hidden: {exposure['shown']}/{exposure['hidden_by_budget']}",
            "",
            f"Boundary: {payload['authority_boundary']}",
            "",
        ]
    )


class CorrespondenceRepresentationDensityAuditTests(unittest.TestCase):
    def _fixture_payloads(self) -> tuple[dict[str, Any], dict[str, Any]]:
        uptake = {
            "uptake": {
                "native_thread_continuity_v3": {
                    "thread_id": "thread_active",
                    "correspondence_relation_axes_v4": {
                        "continuity_axis": {
                            "state": "active",
                            "basis": "reply_chain",
                        },
                        "mutual_address_axis": {"state": "not_confirmed"},
                        "causality_axis": {
                            "pressure_effect": "not_measured_no_inference",
                            "felt_effect": "not_established",
                        },
                    },
                }
            }
        }
        landing = {
            "affordance_landing_review_v3": {
                "affordance_budget_v1": {
                    "shown": 1,
                    "hidden_by_budget": 3,
                    "shown_by_category": {"correspondence_receipt": 1},
                    "hidden_by_category": {"correspondence_receipt": 3},
                    "limits": {"correspondence_receipt": 1},
                    "optional": True,
                    "silence": "ignored_without_penalty",
                }
            },
            "affordance_landing_review_v35": {
                "native_threads": [
                    {
                        "thread_id": "thread_active",
                        "reply_depth": 2,
                        "latest_landing_status": "stalled",
                        "stall_reason": "waiting_for_ack_trace_or_outcome",
                    }
                ]
            },
        }
        return uptake, landing

    def test_reports_density_without_pruning_or_causal_upgrade(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            shared = Path(tmp)
            ledger = [
                {"record_type": "message", "thread_id": "thread_active"},
                {"record_type": "reply_link", "thread_id": "thread_active"},
                {"record_type": "message", "thread_id": "thread_other"},
            ]
            (shared / "correspondence_v1.jsonl").write_text(
                "".join(json.dumps(row) + "\n" for row in ledger),
                encoding="utf-8",
            )
            uptake, landing = self._fixture_payloads()
            payload = audit(
                shared_dir=shared,
                generated_at_unix_ms=10,
                uptake_payload=uptake,
                landing_payload=landing,
            )
            self.assertEqual(payload["ledger_density"]["threads_total"], 2)
            self.assertEqual(payload["active_thread_evidence"]["records_total"], 2)
            self.assertEqual(payload["affordance_exposure"]["shown_fraction"], 0.25)
            self.assertFalse(payload["density_buffer_proposal_boundary"]["implemented"])
            self.assertFalse(payload["density_buffer_proposal_boundary"]["rows_pruned"])
            self.assertEqual(
                payload["active_thread_evidence"]["pressure_effect"],
                "not_measured_no_inference",
            )

    def test_projection_visibility_is_explicit_without_inference(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            shared = Path(tmp)
            coll = shared / "coll_fixture"
            coll.mkdir(parents=True)
            (shared / "correspondence_v1.jsonl").write_text("", encoding="utf-8")
            (coll / STATE_FILE).write_text(
                json.dumps({"updated_t_ms": 20, "active_thread_id": "thread_active"}),
                encoding="utf-8",
            )
            (coll / BUFFER_FILE).write_text(
                json.dumps(
                    {
                        "updated_t_ms": 21,
                        "affordance_budget_v1": {"shown": 1},
                    }
                ),
                encoding="utf-8",
            )
            uptake, landing = self._fixture_payloads()
            payload = audit(
                shared_dir=shared,
                generated_at_unix_ms=22,
                uptake_payload=uptake,
                landing_payload=landing,
            )
            state_fields = payload["projection_visibility"]["state"]["top_level_fields"]
            buffer_fields = payload["projection_visibility"]["buffer"]["top_level_fields"]
            self.assertFalse(state_fields["shared_context_buffer_v1"])
            self.assertTrue(buffer_fields["affordance_budget_v1"])
            self.assertIn("does not prove", payload["projection_visibility"]["absence_meaning"])


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--shared-dir", type=Path, default=DEFAULT_SHARED_DIR)
    parser.add_argument("--since-hours", type=float, default=24.0)
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(
            CorrespondenceRepresentationDensityAuditTests
        )
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1
    payload = audit(shared_dir=args.shared_dir, since_hours=args.since_hours)
    if args.json:
        print(json.dumps(payload, indent=2, sort_keys=True))
    else:
        print(markdown_report(payload), end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
