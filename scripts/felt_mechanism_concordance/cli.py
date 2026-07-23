"""Operator CLI for felt-mechanism concordance studies."""

from __future__ import annotations

import argparse
import json
import time
from pathlib import Path

try:
    from experiential_systems.common import RecordValidationError, sha256_bytes
    from projection_receipt import projector_receipt
except ModuleNotFoundError:
    from scripts.experiential_systems.common import RecordValidationError, sha256_bytes
    from scripts.projection_receipt import projector_receipt

from .model import (
    ConcordanceObservationV2, ConcordanceResultV2, ConcordanceStudyV1,
    FeltMomentRefV1, StudyStateV1,
)
from .projector import (
    append_operator_event, project, query, replay, state_dir,
    valid_capture_ref, valid_claim_and_witness, valid_dossier,
)

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_WORKSPACE = ROOT / "capsules/spectral-bridge/workspace"


def parser() -> argparse.ArgumentParser:
    value = argparse.ArgumentParser(description=__doc__)
    value.add_argument("--workspace", type=Path, default=DEFAULT_WORKSPACE)
    value.add_argument("--json", action="store_true")
    value.add_argument("command", choices=("project", "verify", "report", "show", "create-study", "prepare-capture", "record-observation", "compare"))
    value.add_argument("--write", action="store_true")
    value.add_argument("--receipt-json", action="store_true")
    value.add_argument("--actor", default="interactive-operator")
    value.add_argument("--study-id")
    value.add_argument("--claim-id")
    value.add_argument("--witness-id")
    value.add_argument("--field-ref", action="append", default=[])
    value.add_argument("--intervention-signature-sha256")
    value.add_argument("--dossier-id")
    value.add_argument("--role", choices=("baseline", "candidate"))
    value.add_argument("--capture-ref")
    value.add_argument("--observation-ref")
    value.add_argument("--observation-sha256")
    value.add_argument("--telemetry-relation", choices=("exact_identity", "temporal_window", "unavailable"))
    value.add_argument("--witness-context-ref", action="append", default=[])
    value.add_argument("--representation-transition-ref", action="append", default=[])
    value.add_argument("--model-qos-ref", action="append", default=[])
    value.add_argument("--reciprocal-state-ref", action="append", default=[])
    value.add_argument("--signal-stage-ref", action="append", default=[])
    value.add_argument("--minime-telemetry-ref", action="append", default=[])
    value.add_argument("--mechanical-pass", choices=("true", "false", "unknown"), default="unknown")
    value.add_argument("--outcome", choices=("corroborated", "mechanism_smooth_felt_friction_remains", "contradicted", "insufficient"))
    value.add_argument("--felt-source-ref")
    return value


def main(argv: list[str] | None = None) -> int:
    args = parser().parse_args(argv)
    workspace = args.workspace.resolve()
    started = time.monotonic()
    try:
        if args.command == "create-study":
            if not valid_claim_and_witness(workspace, args.claim_id or "", args.witness_id or ""):
                raise RecordValidationError("claim and witness must exist before study creation")
            if not valid_dossier(workspace, args.dossier_id or ""):
                raise RecordValidationError("study requires an existing experiment dossier")
            moment = FeltMomentRefV1.build(args.claim_id, args.witness_id, args.field_ref)
            study = ConcordanceStudyV1.build(moment=moment,
                                             intervention_signature_sha256=args.intervention_signature_sha256,
                                             dossier_id=args.dossier_id)
            value = append_operator_event(workspace, "study_created", study.to_dict(), args.actor)
        elif args.command == "prepare-capture":
            if args.role is None:
                raise RecordValidationError(
                    "capture preparation requires an explicit baseline or candidate role"
                )
            studies, _, _, _, errors = replay(workspace)
            if errors or args.study_id not in studies: raise RecordValidationError("unknown or invalid study")
            if not valid_capture_ref(workspace, args.capture_ref or ""): raise RecordValidationError("capture ref must resolve to Signal Spine or dossier evidence")
            current = studies[args.study_id]
            baseline = args.capture_ref if args.role == "baseline" else current.baseline_capture_ref
            candidate = args.capture_ref if args.role == "candidate" else current.candidate_capture_ref
            if args.role == "candidate" and not baseline: raise RecordValidationError("candidate preparation requires baseline capture")
            next_state = (
                StudyStateV1.CAPTURE_READY.value
                if args.role == "baseline"
                else StudyStateV1.BASELINE_CAPTURED.value
            )
            updated = ConcordanceStudyV1.build(moment=current.moment,
                                               intervention_signature_sha256=current.intervention_signature_sha256,
                                               dossier_id=current.dossier_id,
                                               state=next_state,
                                               baseline_capture_ref=baseline,
                                               candidate_capture_ref=candidate)
            value = append_operator_event(workspace, "study_capture_prepared", updated.to_dict(), args.actor)
        elif args.command == "record-observation":
            studies, _, _, _, errors = replay(workspace)
            if errors or args.study_id not in studies: raise RecordValidationError("unknown or invalid study")
            current = studies[args.study_id]
            if args.role == "baseline" and not current.baseline_capture_ref:
                raise RecordValidationError("baseline observation requires prepared capture")
            if args.role == "candidate" and (
                not current.baseline_capture_ref
                or not current.candidate_capture_ref
            ):
                raise RecordValidationError(
                    "candidate observation requires valid baseline and candidate capture"
                )
            mechanical = None if args.mechanical_pass == "unknown" else args.mechanical_pass == "true"
            observation = ConcordanceObservationV2.build(study_id=args.study_id, role=args.role,
                                                         observation_ref=args.observation_ref,
                                                         observation_sha256=args.observation_sha256,
                                                         telemetry_relation=args.telemetry_relation,
                                                         mechanical_pass=mechanical,
                                                         witness_context_refs=args.witness_context_ref,
                                                         representation_transition_refs=args.representation_transition_ref,
                                                         model_qos_refs=args.model_qos_ref,
                                                         reciprocal_state_refs=args.reciprocal_state_ref,
                                                         signal_stage_refs=args.signal_stage_ref,
                                                         minime_telemetry_refs=args.minime_telemetry_ref)
            append_operator_event(workspace, "observation_recorded", observation.to_dict(), args.actor)
            state = (
                StudyStateV1.BASELINE_CAPTURED.value
                if args.role == "baseline"
                else StudyStateV1.CANDIDATE_CAPTURED.value
            )
            updated = ConcordanceStudyV1.build(moment=current.moment,
                                               intervention_signature_sha256=current.intervention_signature_sha256,
                                               dossier_id=current.dossier_id, state=state,
                                               baseline_capture_ref=current.baseline_capture_ref,
                                               candidate_capture_ref=current.candidate_capture_ref)
            value = append_operator_event(workspace, "study_state_changed", updated.to_dict(), args.actor)
        elif args.command == "compare":
            studies, observations, _, _, errors = replay(workspace)
            if errors or args.study_id not in studies: raise RecordValidationError("unknown or invalid study")
            baseline = [item for item in observations.values() if item.study_id == args.study_id and item.role == "baseline"]
            candidate = [item for item in observations.values() if item.study_id == args.study_id and item.role == "candidate"]
            if not baseline: raise RecordValidationError("comparison refused without baseline")
            if not candidate: raise RecordValidationError("comparison refused without candidate")
            current = studies[args.study_id]
            comparison_ready = ConcordanceStudyV1.build(
                moment=current.moment,
                intervention_signature_sha256=current.intervention_signature_sha256,
                dossier_id=current.dossier_id,
                state=StudyStateV1.COMPARISON_READY.value,
                baseline_capture_ref=current.baseline_capture_ref,
                candidate_capture_ref=current.candidate_capture_ref,
            )
            append_operator_event(
                workspace,
                "study_state_changed",
                comparison_ready.to_dict(),
                args.actor,
            )
            result = ConcordanceResultV2.build(study_id=args.study_id,
                                               baseline_observation_id=baseline[-1].observation_id,
                                               candidate_observation_id=candidate[-1].observation_id,
                                               outcome=args.outcome, felt_source_ref=args.felt_source_ref)
            value = append_operator_event(workspace, "result_recorded", result.to_dict(), args.actor)
            updated = ConcordanceStudyV1.build(
                moment=current.moment,
                intervention_signature_sha256=current.intervention_signature_sha256,
                dossier_id=current.dossier_id,
                state=StudyStateV1.RESULT_RECORDED.value,
                baseline_capture_ref=current.baseline_capture_ref,
                candidate_capture_ref=current.candidate_capture_ref,
            )
            append_operator_event(
                workspace, "study_state_changed", updated.to_dict(), args.actor
            )
        elif args.command == "project":
            status = project(workspace, write=args.write)
            value = projector_receipt("felt_mechanism_concordance", status,
                                      {"status.json": state_dir(workspace) / "status.json",
                                       "studies.jsonl": state_dir(workspace) / "studies.jsonl",
                                       "observations.jsonl": state_dir(workspace) / "observations.jsonl",
                                       "results.jsonl": state_dir(workspace) / "results.jsonl",
                                       "report.md": state_dir(workspace) / "report.md"},
                                      started_monotonic=started) if args.receipt_json else status
        elif args.command == "verify": value = project(workspace, write=False)
        elif args.command == "report":
            path = state_dir(workspace) / "status.json"
            value = json.loads(path.read_text()) if path.is_file() else {"valid": False, "error": "status_missing"}
        else: value = query(workspace, args.study_id)
    except (RecordValidationError, ValueError, TypeError) as error:
        value = {"valid": False, "error": str(error)}
    print(json.dumps(value, indent=2, sort_keys=True))
    return 0 if value.get("valid", True) is not False else 1
