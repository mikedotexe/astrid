#!/usr/bin/env python3
"""Project capture-first experiment dossiers over existing sandbox trials."""

from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
import os
from pathlib import Path
import re
import sys
import tempfile
import unittest
from typing import Any

try:
    from evidence_store import EvidenceEventStore, EvidenceStoreError
    from evidence_store.adapter import append_domain_events, read_domain_events
    from evidence_store.model import canonical_json
except ModuleNotFoundError:
    from scripts.evidence_store import EvidenceEventStore, EvidenceStoreError
    from scripts.evidence_store.adapter import append_domain_events, read_domain_events
    from scripts.evidence_store.model import canonical_json

DEFAULT_WORKSPACE = Path("/Users/v/other/astrid/capsules/spectral-bridge/workspace")
PROJECTOR_VERSION = 1
DOSSIER_STATES = (
    "draft",
    "capture-ready",
    "baseline-captured",
    "candidate-captured",
    "comparison-ready",
    "result-recorded",
    "review-pending",
    "closed",
)
STATE_INDEX = {state: index for index, state in enumerate(DOSSIER_STATES)}
CAPTURE_REF_RE = re.compile(
    r"^(?:sha256:[0-9a-f]{64}|capture:[A-Za-z0-9_.-]+:sha256:[0-9a-f]{64})$"
)


def state_dir(workspace: Path) -> Path:
    return workspace / "diagnostics/experiment_dossiers_v1"


def family_state_dir(workspace: Path) -> Path:
    return workspace / "diagnostics/claim_families_v1"


def sandbox_status_path(workspace: Path) -> Path:
    return workspace / "diagnostics/sandbox_trial_queue_v1/status.json"


def family_status_path(workspace: Path) -> Path:
    return workspace / "diagnostics/claim_families_v1/status.json"


def authority_state(state: str) -> dict[str, Any]:
    if state not in {"evidence_only", "approval_pending"}:
        raise ValueError(f"invalid dossier authority state: {state}")
    return {
        "schema": "artifact_authority_state_v1",
        "schema_version": 1,
        "state": state,
        "live_eligible_now": False,
        "auto_approved": False,
        "grants_approval": False,
        "edits_source_now": False,
    }


def digest(value: Any) -> str:
    return hashlib.sha256(canonical_json(value).encode("utf-8")).hexdigest()


def agency_tier_value(value: Any) -> int:
    if isinstance(value, bool):
        return 0
    if isinstance(value, int):
        return value
    match = re.search(r"\d+", str(value or ""))
    return int(match.group()) if match else 0


def load_object(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError(f"{path} must contain an object")
    return value


def claim_family_index(family_status: dict[str, Any]) -> dict[str, str]:
    result: dict[str, str] = {}
    families = family_status.get("families")
    if not isinstance(families, dict):
        return result
    for family_id, family in families.items():
        claims = family.get("claims") if isinstance(family, dict) else None
        if isinstance(claims, dict):
            for claim_id in claims:
                result[str(claim_id)] = str(family_id)
    return result


def intervention_signature(trial: dict[str, Any]) -> str:
    bounded = {
        "adapter": trial.get("adapter"),
        "trial_mode": trial.get("trial_mode"),
        "proposed_intervention": trial.get("proposed_intervention"),
        "agency_tier": trial.get("agency_tier"),
        "surface": (
            trial.get("authority_boundary_packet_v2", {}).get("surface")
            if isinstance(trial.get("authority_boundary_packet_v2"), dict)
            else None
        ),
    }
    return digest(bounded)


def initial_dossier_events(workspace: Path) -> list[dict[str, Any]]:
    sandbox_raw = sandbox_status_path(workspace).read_bytes()
    family_raw = family_status_path(workspace).read_bytes()
    sandbox = json.loads(sandbox_raw)
    family_status = json.loads(family_raw)
    families = claim_family_index(family_status)
    trials = sandbox.get("trials")
    if not isinstance(trials, dict):
        raise ValueError("sandbox status trials must be an object")
    grouped: dict[tuple[str, str], list[dict[str, Any]]] = {}
    unrouted: list[dict[str, Any]] = []
    for trial_id, trial in sorted(trials.items()):
        if not isinstance(trial, dict):
            continue
        introspection_id = str(trial.get("source_introspection_id") or "")
        claim_id = str(trial.get("claim_id") or "")
        canonical_claim_id = f"{introspection_id}:{claim_id}"
        family_id = families.get(canonical_claim_id)
        if family_id is None:
            unrouted.append(
                {
                    "trial_id": str(trial.get("trial_id") or trial_id),
                    "canonical_claim_id": (
                        canonical_claim_id
                        if introspection_id and claim_id
                        else None
                    ),
                    "routing_reason": (
                        "claim_family_missing"
                        if introspection_id and claim_id
                        else "claim_identity_missing"
                    ),
                    "status": trial.get("status"),
                    "adapter": trial.get("adapter"),
                    "agency_tier": trial.get("agency_tier"),
                }
            )
            continue
        signature = intervention_signature(trial)
        grouped.setdefault((family_id, signature), []).append(
            {
                "trial_id": str(trial.get("trial_id") or trial_id),
                "canonical_claim_id": canonical_claim_id,
                "status": trial.get("status"),
                "runnable": bool(trial.get("runnable")),
                "agency_tier": trial.get("agency_tier"),
                "adapter": trial.get("adapter"),
            }
        )
    source_sha256 = digest(
        {
            "sandbox": hashlib.sha256(sandbox_raw).hexdigest(),
            "families": hashlib.sha256(family_raw).hexdigest(),
        }
    )
    events = []
    for trial in unrouted:
        events.append(
            {
                "schema": "experiment_dossier_domain_event_v1",
                "schema_version": 1,
                "event_type": "experiment_dossier_trial_unrouted",
                "aggregate_type": "sandbox_trial",
                "aggregate_id": trial["trial_id"],
                "trial_id": trial["trial_id"],
                "unrouted_trial": trial,
                "source_receipt": {
                    "sandbox_status_sha256": hashlib.sha256(sandbox_raw).hexdigest(),
                    "claim_family_status_sha256": hashlib.sha256(family_raw).hexdigest(),
                },
                "idempotency_key": (
                    f"experiment_dossier_unrouted:{trial['trial_id']}:{source_sha256}"
                ),
                "artifact_authority_state_v1": authority_state("evidence_only"),
            }
        )
    for (family_id, signature), trial_refs in sorted(grouped.items()):
        approval_pending = any(
            agency_tier_value(trial.get("agency_tier")) >= 4
            or "approval_required" in str(trial.get("status") or "")
            for trial in trial_refs
        )
        dossier_id = f"dossier_{digest([family_id, signature])[:20]}"
        dossier = {
            "schema": "experiment_dossier_v1",
            "schema_version": 1,
            "dossier_id": dossier_id,
            "claim_family_id": family_id,
            "intervention_signature": signature,
            "state": "draft" if approval_pending else "capture-ready",
            "trial_refs": trial_refs,
            "baseline_capture_ref": None,
            "candidate_capture_ref": None,
            "comparison_ref": None,
            "result_ref": None,
            "review_ref": None,
            "closure_ref": None,
            "dossier_sufficient": False,
            "candidate_comparison_allowed": False,
            "live_authority_granted": False,
            "artifact_authority_state_v1": authority_state(
                "approval_pending" if approval_pending else "evidence_only"
            ),
        }
        events.append(
            {
                "schema": "experiment_dossier_domain_event_v1",
                "schema_version": 1,
                "event_type": "experiment_dossier_projected",
                "aggregate_type": "experiment_dossier",
                "aggregate_id": dossier_id,
                "dossier_id": dossier_id,
                "dossier": dossier,
                "source_receipt": {
                    "sandbox_status_sha256": hashlib.sha256(sandbox_raw).hexdigest(),
                    "claim_family_status_sha256": hashlib.sha256(family_raw).hexdigest(),
                },
                "idempotency_key": f"experiment_dossier:{dossier_id}:{source_sha256}",
                "artifact_authority_state_v1": dossier[
                    "artifact_authority_state_v1"
                ],
            }
        )
    return events


def validate_transition(
    dossier: dict[str, Any],
    target_state: str,
    evidence_ref: str,
    approval_receipt: str | None,
) -> None:
    if target_state not in DOSSIER_STATES:
        raise ValueError(f"unknown dossier state: {target_state}")
    current = str(dossier.get("state") or "draft")
    if STATE_INDEX[target_state] != STATE_INDEX[current] + 1:
        raise ValueError(f"invalid dossier transition: {current} -> {target_state}")
    if not evidence_ref.strip():
        raise ValueError("transition evidence reference must not be empty")
    if target_state in {"baseline-captured", "candidate-captured"} and not (
        CAPTURE_REF_RE.fullmatch(evidence_ref.strip())
    ):
        raise ValueError(
            "capture transition requires a content-addressed sha256 reference"
        )
    baseline = dossier.get("baseline_capture_ref")
    if target_state in {"candidate-captured", "comparison-ready"} and not (
        isinstance(baseline, str) and CAPTURE_REF_RE.fullmatch(baseline)
    ):
        raise ValueError("candidate capture/comparison requires a valid baseline")
    candidate = dossier.get("candidate_capture_ref")
    if target_state == "comparison-ready" and not (
        isinstance(candidate, str) and CAPTURE_REF_RE.fullmatch(candidate)
    ):
        raise ValueError("comparison requires a captured candidate")
    authority = dossier.get("artifact_authority_state_v1")
    if (
        isinstance(authority, dict)
        and authority.get("state") == "approval_pending"
        and target_state in {"candidate-captured", "comparison-ready"}
        and not approval_receipt
    ):
        raise ValueError("approval-pending intervention requires an external approval receipt")


def apply_transition(
    dossier: dict[str, Any], event: dict[str, Any]
) -> dict[str, Any]:
    updated = dict(dossier)
    target = str(event.get("target_state") or "")
    evidence_ref = str(event.get("evidence_ref") or "")
    field_by_state = {
        "baseline-captured": "baseline_capture_ref",
        "candidate-captured": "candidate_capture_ref",
        "comparison-ready": "comparison_ref",
        "result-recorded": "result_ref",
        "review-pending": "review_ref",
        "closed": "closure_ref",
    }
    if target in field_by_state:
        updated[field_by_state[target]] = evidence_ref
    updated["state"] = target
    updated["candidate_comparison_allowed"] = bool(
        updated.get("baseline_capture_ref") and updated.get("candidate_capture_ref")
    )
    updated["dossier_sufficient"] = bool(
        updated.get("baseline_capture_ref")
        and updated.get("candidate_capture_ref")
        and updated.get("comparison_ref")
    )
    return updated


def project(events: list[dict[str, Any]]) -> dict[str, Any]:
    dossiers: dict[str, dict[str, Any]] = {}
    history: list[dict[str, Any]] = []
    unrouted: dict[str, dict[str, Any]] = {}
    transition_violations: list[dict[str, Any]] = []
    stateful_fields = {
        "state",
        "baseline_capture_ref",
        "candidate_capture_ref",
        "comparison_ref",
        "result_ref",
        "review_ref",
        "closure_ref",
        "dossier_sufficient",
        "candidate_comparison_allowed",
        "live_authority_granted",
    }
    for event in events:
        event_type = event.get("event_type")
        dossier_id = str(event.get("dossier_id") or "")
        if event_type == "experiment_dossier_projected":
            dossier = event.get("dossier")
            if isinstance(dossier, dict):
                if str(dossier.get("claim_family_id") or "").startswith(
                    "family_unresolved_"
                ):
                    for trial in dossier.get("trial_refs") or []:
                        if isinstance(trial, dict) and trial.get("trial_id"):
                            unrouted[str(trial["trial_id"])] = {
                                **trial,
                                "routing_reason": "legacy_unresolved_family_projection",
                            }
                    continue
                previous = dossiers.get(dossier_id)
                if previous is None:
                    dossiers[dossier_id] = dict(dossier)
                else:
                    refreshed = dict(dossier)
                    for field in stateful_fields:
                        if field in previous:
                            refreshed[field] = previous[field]
                    old_authority = previous.get("artifact_authority_state_v1")
                    new_authority = refreshed.get("artifact_authority_state_v1")
                    old_pending = (
                        isinstance(old_authority, dict)
                        and old_authority.get("state") == "approval_pending"
                    )
                    new_pending = (
                        isinstance(new_authority, dict)
                        and new_authority.get("state") == "approval_pending"
                    )
                    if old_pending or new_pending:
                        refreshed["artifact_authority_state_v1"] = authority_state(
                            "approval_pending"
                        )
                    dossiers[dossier_id] = refreshed
        elif event_type == "experiment_dossier_transitioned":
            if dossier_id in dossiers:
                try:
                    validate_transition(
                        dossiers[dossier_id],
                        str(event.get("target_state") or ""),
                        str(event.get("evidence_ref") or ""),
                        (
                            str(event["approval_receipt"])
                            if event.get("approval_receipt")
                            else None
                        ),
                    )
                except ValueError as error:
                    transition_violations.append(
                        {
                            "dossier_id": dossier_id,
                            "event_type": event_type,
                            "error": str(error),
                        }
                    )
                else:
                    dossiers[dossier_id] = apply_transition(
                        dossiers[dossier_id], event
                    )
                    history.append(event)
        elif event_type == "experiment_dossier_trial_unrouted":
            trial = event.get("unrouted_trial")
            if isinstance(trial, dict) and trial.get("trial_id"):
                unrouted[str(trial["trial_id"])] = trial
    counts = Counter(str(dossier.get("state") or "draft") for dossier in dossiers.values())
    baseline_missing = sum(
        1
        for dossier in dossiers.values()
        if dossier.get("state")
        in {
            "candidate-captured",
            "comparison-ready",
            "result-recorded",
            "review-pending",
            "closed",
        }
        and not dossier.get("baseline_capture_ref")
    )
    return {
        "schema": "experiment_dossier_status_v1",
        "schema_version": 1,
        "dossier_count": len(dossiers),
        "state_counts": dict(sorted(counts.items())),
        "baseline_missing_violation_count": baseline_missing,
        "transition_violation_count": len(transition_violations),
        "unrouted_trial_count": len(unrouted),
        "counter_audit": {
            "all_dossiers_have_one_state": sum(counts.values()) == len(dossiers),
            "candidate_or_later_has_baseline": baseline_missing == 0,
            "transition_replay_valid": not transition_violations,
        },
        "dossiers": dict(sorted(dossiers.items())),
        "transition_history": history,
        "transition_violations": transition_violations,
        "unrouted_trials": dict(sorted(unrouted.items())),
        "artifact_authority_state_v1": authority_state("evidence_only"),
    }


def render_report(status: dict[str, Any]) -> str:
    lines = [
        "# Experiment Dossiers",
        "",
        f"- Dossiers: {status['dossier_count']}",
        f"- Baseline gate violations: {status['baseline_missing_violation_count']}",
        f"- Transition replay violations: {status['transition_violation_count']}",
        f"- Unrouted legacy trials: {status['unrouted_trial_count']}",
    ]
    lines.extend(
        f"- {state}: {status['state_counts'].get(state, 0)}"
        for state in DOSSIER_STATES
    )
    lines.extend(
        [
            "- Candidate comparison requires baseline: enforced",
            "- Live interventions remain approval pending",
            "",
        ]
    )
    return "\n".join(lines)


def atomic_write_text(path: Path, payload: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    handle = tempfile.NamedTemporaryFile(
        "w",
        encoding="utf-8",
        dir=path.parent,
        prefix=f".{path.name}.",
        delete=False,
    )
    temporary = Path(handle.name)
    try:
        with handle:
            handle.write(payload)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
        directory_fd = os.open(path.parent, os.O_RDONLY)
        try:
            os.fsync(directory_fd)
        finally:
            os.close(directory_fd)
    finally:
        temporary.unlink(missing_ok=True)


def write_projection(workspace: Path, status: dict[str, Any]) -> dict[str, str]:
    root = state_dir(workspace)
    root.mkdir(parents=True, exist_ok=True)
    outputs = {
        "status.json": json.dumps(status, indent=2, sort_keys=True) + "\n",
        "report.md": render_report(status),
    }
    hashes = {}
    for name, payload in outputs.items():
        atomic_write_text(root / name, payload)
        hashes[name] = hashlib.sha256(payload.encode()).hexdigest()
    return hashes


def generate(workspace: Path, *, write: bool) -> dict[str, Any]:
    generated = initial_dossier_events(workspace)
    if write:
        append_domain_events(
            family_state_dir(workspace), "claim_families", generated
        )
        events, corrupt = read_domain_events(
            family_state_dir(workspace), "claim_families"
        )
        if corrupt:
            raise EvidenceStoreError("claim family stream is corrupt")
    else:
        events = generated
    status = project(events)
    status["generated_event_count"] = len(generated)
    if write:
        hashes = write_projection(workspace, status)
        EvidenceEventStore(
            workspace / "diagnostics/evidence_event_store_v2"
        ).write_checkpoint("experiment_dossiers_v1", PROJECTOR_VERSION, hashes)
        status["projection_hashes"] = hashes
    return status


def transition(
    workspace: Path,
    dossier_id: str,
    target_state: str,
    evidence_ref: str,
    approval_receipt: str | None,
) -> dict[str, Any]:
    events, corrupt = read_domain_events(
        family_state_dir(workspace), "claim_families"
    )
    if corrupt:
        raise EvidenceStoreError("claim family stream is corrupt")
    status = project(events)
    dossier = status["dossiers"].get(dossier_id)
    if not isinstance(dossier, dict):
        raise ValueError(f"unknown dossier: {dossier_id}")
    validate_transition(dossier, target_state, evidence_ref, approval_receipt)
    authority = dossier.get("artifact_authority_state_v1")
    authority = authority if isinstance(authority, dict) else authority_state("evidence_only")
    event = {
        "schema": "experiment_dossier_domain_event_v1",
        "schema_version": 1,
        "event_type": "experiment_dossier_transitioned",
        "aggregate_type": "experiment_dossier",
        "aggregate_id": dossier_id,
        "dossier_id": dossier_id,
        "from_state": dossier.get("state"),
        "target_state": target_state,
        "evidence_ref": evidence_ref,
        "approval_receipt": approval_receipt,
        "idempotency_key": (
            f"dossier_transition:{dossier_id}:{target_state}:"
            f"{digest([evidence_ref, approval_receipt])}"
        ),
        "artifact_authority_state_v1": authority,
    }
    append_domain_events(family_state_dir(workspace), "claim_families", [event])
    return event


class ExperimentDossierTests(unittest.TestCase):
    capture_ref = f"sha256:{'a' * 64}"

    def dossier(self, authority: str = "evidence_only") -> dict[str, Any]:
        return {
            "dossier_id": "dossier_one",
            "state": "capture-ready",
            "baseline_capture_ref": None,
            "candidate_capture_ref": None,
            "candidate_comparison_allowed": False,
            "dossier_sufficient": False,
            "artifact_authority_state_v1": authority_state(authority),
        }

    def test_candidate_is_refused_without_baseline(self) -> None:
        dossier = self.dossier()
        dossier["state"] = "baseline-captured"
        dossier["baseline_capture_ref"] = None
        with self.assertRaisesRegex(ValueError, "requires a valid baseline"):
            validate_transition(
                dossier, "candidate-captured", self.capture_ref, None
            )

    def test_approval_pending_candidate_requires_external_receipt(self) -> None:
        dossier = self.dossier("approval_pending")
        dossier["state"] = "baseline-captured"
        dossier["baseline_capture_ref"] = self.capture_ref
        with self.assertRaisesRegex(ValueError, "external approval receipt"):
            validate_transition(
                dossier, "candidate-captured", self.capture_ref, None
            )
        validate_transition(
            dossier,
            "candidate-captured",
            self.capture_ref,
            "operator_receipt",
        )

    def test_reprojection_preserves_advanced_state(self) -> None:
        projected = self.dossier()
        events = [
            {
                "event_type": "experiment_dossier_projected",
                "dossier_id": "dossier_one",
                "dossier": projected,
            },
            {
                "event_type": "experiment_dossier_transitioned",
                "dossier_id": "dossier_one",
                "target_state": "baseline-captured",
                "evidence_ref": self.capture_ref,
            },
            {
                "event_type": "experiment_dossier_projected",
                "dossier_id": "dossier_one",
                "dossier": projected,
            },
        ]
        dossier = project(events)["dossiers"]["dossier_one"]
        self.assertEqual(dossier["state"], "baseline-captured")
        self.assertEqual(dossier["baseline_capture_ref"], self.capture_ref)

    def test_invalid_replay_transition_is_held_as_a_violation(self) -> None:
        status = project(
            [
                {
                    "event_type": "experiment_dossier_projected",
                    "dossier_id": "dossier_one",
                    "dossier": self.dossier(),
                },
                {
                    "event_type": "experiment_dossier_transitioned",
                    "dossier_id": "dossier_one",
                    "target_state": "candidate-captured",
                    "evidence_ref": self.capture_ref,
                },
            ]
        )
        self.assertEqual(
            status["dossiers"]["dossier_one"]["state"], "capture-ready"
        )
        self.assertEqual(status["transition_violation_count"], 1)
        self.assertFalse(status["counter_audit"]["transition_replay_valid"])

    def test_legacy_unresolved_family_is_reported_as_unrouted(self) -> None:
        dossier = {
            **self.dossier(),
            "claim_family_id": "family_unresolved_old",
            "trial_refs": [{"trial_id": "trial_one"}],
        }
        status = project(
            [
                {
                    "event_type": "experiment_dossier_projected",
                    "dossier_id": "dossier_old",
                    "dossier": dossier,
                }
            ]
        )
        self.assertEqual(status["dossier_count"], 0)
        self.assertEqual(status["unrouted_trial_count"], 1)

    def test_projection_never_produces_comparison_without_baseline(self) -> None:
        events = [
            {
                "event_type": "experiment_dossier_projected",
                "dossier_id": "dossier_one",
                "dossier": self.dossier(),
            }
        ]
        status = project(events)
        self.assertEqual(status["baseline_missing_violation_count"], 0)
        self.assertFalse(
            status["dossiers"]["dossier_one"]["candidate_comparison_allowed"]
        )

    def test_intervention_signature_is_stable_and_bounded(self) -> None:
        trial = {
            "adapter": "offline_replay",
            "trial_mode": "read_only",
            "proposed_intervention": "compare bounded fixtures",
            "agency_tier": 2,
        }
        self.assertEqual(
            intervention_signature(trial), intervention_signature(dict(trial))
        )


def cli_summary(status: dict[str, Any]) -> dict[str, Any]:
    return {
        key: value
        for key, value in status.items()
        if key
        not in {
            "dossiers",
            "transition_history",
            "transition_violations",
            "unrouted_trials",
        }
    }


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workspace", type=Path, default=DEFAULT_WORKSPACE)
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    commands = parser.add_subparsers(dest="command")
    generate_parser = commands.add_parser("generate")
    generate_parser.add_argument("--write", action="store_true")
    commands.add_parser("report")
    advance = commands.add_parser("advance")
    advance.add_argument("--dossier-id", required=True)
    advance.add_argument("--state", required=True, choices=DOSSIER_STATES)
    advance.add_argument("--evidence-ref", required=True)
    advance.add_argument("--approval-receipt")
    advance.add_argument("--write", action="store_true")
    return parser


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(
            ExperimentDossierTests
        )
        return 0 if unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful() else 1
    workspace = args.workspace.resolve()
    try:
        if args.command in {"generate", "report"}:
            status = generate(
                workspace,
                write=args.command == "generate" and bool(args.write),
            )
            print(json.dumps(cli_summary(status), indent=2, sort_keys=True))
            return 0
        if args.command == "advance":
            if not args.write:
                raise ValueError("advance requires --write")
            event = transition(
                workspace,
                args.dossier_id,
                args.state,
                args.evidence_ref,
                args.approval_receipt,
            )
            print(json.dumps(event, indent=2, sort_keys=True))
            return 0
    except (EvidenceStoreError, OSError, ValueError) as error:
        print(json.dumps({"status": "failed", "error": str(error)}), file=sys.stderr)
        return 1
    parser.print_help()
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
