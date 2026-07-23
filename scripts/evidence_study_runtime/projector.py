"""Deterministic projection for preregistered evidence studies."""

from __future__ import annotations

from collections import Counter
import hashlib
import json
import math
from pathlib import Path
from typing import Any, Callable

try:
    from evidence_store import EvidenceEventStore
    from experiential_systems.common import (
        RecordValidationError,
        authority_state,
        event_payload,
        owner_atomic_write,
        owner_atomic_write_json,
        owner_atomic_write_jsonl,
        project_events,
        validate_evidence_record,
    )
except ModuleNotFoundError:
    from scripts.evidence_store import EvidenceEventStore
    from scripts.experiential_systems.common import (
        RecordValidationError,
        authority_state,
        event_payload,
        owner_atomic_write,
        owner_atomic_write_json,
        owner_atomic_write_jsonl,
        project_events,
        validate_evidence_record,
    )

from .config import DEFAULT_MANIFEST, state_dir
from .model import (
    EvidenceStudyCampaignV1,
    EvidenceStudyPlanV1,
    MechanicalComparisonReceiptV1,
    StudyWindowReceiptV1,
    StudyWindowSpecV1,
)
from .review import (
    StudyCaptureGapReceiptV1,
    StudyReviewReceiptV1,
    group_review_opportunities,
    validate_review_admission,
)
from .storage import load_events, operator_path

STREAM = "felt_mechanism_concordance"
SCHEMA = "evidence_study_domain_event_v1"
PROJECTOR_VERSION = 1

Builder = Callable[[Any], Any]
BUILDERS: dict[str, Builder] = {
    "campaign_seeded": EvidenceStudyCampaignV1.from_untrusted,
    "plan_preregistered": EvidenceStudyPlanV1.from_untrusted,
    "window_started": StudyWindowSpecV1.from_untrusted,
    "window_stopped": StudyWindowSpecV1.from_untrusted,
    "window_assembled": StudyWindowReceiptV1.from_untrusted,
    "comparison_recorded": MechanicalComparisonReceiptV1.from_untrusted,
    "capture_gap_recorded": StudyCaptureGapReceiptV1.from_untrusted,
    "review_recorded": StudyReviewReceiptV1.from_untrusted,
}


def _sha256_file(path: Path) -> str:
    if not path.is_file():
        return hashlib.sha256(b"").hexdigest()
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _sample_source_hash(workspace: Path) -> str:
    digest = hashlib.sha256()
    sample_root = state_dir(workspace) / "samples"
    for path in sorted(sample_root.glob("*.jsonl")) if sample_root.is_dir() else ():
        digest.update(path.name.encode("utf-8"))
        digest.update(bytes.fromhex(_sha256_file(path)))
    return digest.hexdigest()


def replay(workspace: Path) -> dict[str, Any]:
    rows, errors = load_events(workspace)
    campaigns: dict[str, EvidenceStudyCampaignV1] = {}
    plan_history: dict[str, list[EvidenceStudyPlanV1]] = {}
    windows: dict[str, StudyWindowSpecV1] = {}
    stopped: set[str] = set()
    receipts: dict[str, StudyWindowReceiptV1] = {}
    comparisons: dict[str, MechanicalComparisonReceiptV1] = {}
    gaps: dict[str, StudyCaptureGapReceiptV1] = {}
    reviews: dict[str, StudyReviewReceiptV1] = {}
    valid_events: list[dict[str, Any]] = []

    for index, event in enumerate(rows, 1):
        try:
            if not isinstance(event, dict):
                raise RecordValidationError("operator event must be an object")
            validate_evidence_record(event)
            event_type = str(event.get("event_type") or "")
            builder = BUILDERS.get(event_type)
            if builder is None:
                raise RecordValidationError("unsupported evidence study event")
            item = builder(event.get("record"))
            if event_type == "campaign_seeded":
                existing = campaigns.get(item.campaign_id)
                if existing is not None and existing != item:
                    raise RecordValidationError("campaign identity was redefined")
                campaigns[item.campaign_id] = item
            elif event_type == "plan_preregistered":
                history = plan_history.setdefault(item.concordance_study_id, [])
                if history:
                    previous = history[-1]
                    if item.plan_version != previous.plan_version + 1:
                        raise RecordValidationError("plan revisions must be sequential")
                    if item.frozen_prior_plan_sha256 != previous.plan_sha256:
                        raise RecordValidationError("plan revision did not freeze prior hash")
                elif item.plan_version != 1:
                    raise RecordValidationError("first observed plan must be version one")
                if not any(
                    item.concordance_study_id in campaign.study_ids
                    for campaign in campaigns.values()
                ):
                    raise RecordValidationError("plan precedes its campaign")
                history.append(item)
            elif event_type in {"window_started", "window_stopped"}:
                if item.plan_id not in {
                    plan.plan_id for history in plan_history.values() for plan in history
                }:
                    raise RecordValidationError("window precedes preregistration")
                existing = windows.get(item.window_id)
                if existing is not None and existing != item:
                    raise RecordValidationError("window identity was redefined")
                windows[item.window_id] = item
                if event_type == "window_stopped":
                    stopped.add(item.window_id)
            elif event_type == "window_assembled":
                if item.window_id not in windows:
                    raise RecordValidationError("receipt precedes its capture window")
                if item.plan_id != windows[item.window_id].plan_id:
                    raise RecordValidationError("receipt and capture plan disagree")
                receipts[item.receipt_id] = item
            elif event_type == "comparison_recorded":
                baseline = receipts.get(item.baseline_receipt_id)
                candidate = receipts.get(item.candidate_receipt_id)
                if baseline is None or baseline.role != "baseline":
                    raise RecordValidationError("comparison lacks a baseline receipt")
                if candidate is None or candidate.role != "candidate":
                    raise RecordValidationError("comparison lacks a candidate receipt")
                if baseline.plan_id != item.plan_id or candidate.plan_id != item.plan_id:
                    raise RecordValidationError("comparison mixes plan versions")
                comparisons[item.comparison_id] = item
            elif event_type == "capture_gap_recorded":
                if item.window_id not in windows:
                    raise RecordValidationError("gap precedes its capture window")
                gaps[item.gap_id] = item
            else:
                comparison = comparisons.get(item.comparison_id)
                if comparison is None:
                    raise RecordValidationError("review precedes mechanical comparison")
                campaign = campaigns.get(item.campaign_id)
                if campaign is None:
                    raise RecordValidationError("review names an unknown campaign")
                if (
                    comparison.campaign_id != item.campaign_id
                    or comparison.study_id != item.study_id
                    or item.study_id not in campaign.study_ids
                ):
                    raise RecordValidationError(
                        "review campaign, study, and comparison disagree"
                    )
                existing = reviews.get(item.review_id)
                if existing is not None:
                    if existing != item:
                        raise RecordValidationError(
                            "review identity was redefined"
                        )
                    valid_events.append(event)
                    continue
                prior = [
                    review
                    for review in reviews.values()
                    if review.campaign_id == item.campaign_id
                ]
                validate_review_admission(campaign, prior, item)
                reviews[item.review_id] = item
            valid_events.append(event)
        except (RecordValidationError, TypeError, ValueError) as error:
            errors.append(f"event_{index}:{error}")

    plans = {
        study_id: history[-1] for study_id, history in plan_history.items() if history
    }
    return {
        "campaigns": campaigns,
        "plan_history": plan_history,
        "plans": plans,
        "windows": windows,
        "stopped": stopped,
        "receipts": receipts,
        "comparisons": comparisons,
        "gaps": gaps,
        "reviews": reviews,
        "events": valid_events,
        "errors": errors,
    }


def _output_rows(state: dict[str, Any], key: str, identity: str) -> list[dict[str, Any]]:
    values = state[key].values()
    return [item.to_dict() for item in sorted(values, key=lambda item: getattr(item, identity))]


def _verified_fixture_samples(
    workspace: Path, receipt: StudyWindowReceiptV1
) -> list[dict[str, Any]] | None:
    path = state_dir(workspace) / receipt.scalar_fixture_ref
    if (
        not path.is_file()
        or _sha256_file(path) != receipt.scalar_fixture_sha256
    ):
        return None
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError):
        return None
    samples = value.get("samples") if isinstance(value, dict) else None
    if not isinstance(samples, list):
        return None
    return [item for item in samples if isinstance(item, dict)]


def _descriptive_metrics(
    samples: list[dict[str, Any]],
) -> dict[str, dict[str, float | int]]:
    by_name: dict[str, list[float]] = {}
    for sample in samples:
        metrics = sample.get("metrics")
        if not isinstance(metrics, dict):
            continue
        for name, raw in metrics.items():
            if (
                isinstance(raw, (int, float))
                and not isinstance(raw, bool)
                and math.isfinite(float(raw))
            ):
                by_name.setdefault(str(name), []).append(float(raw))
    result: dict[str, dict[str, float | int]] = {}
    for name in sorted(by_name)[:16]:
        values = by_name[name]
        mean = sum(values) / len(values)
        variance = sum((value - mean) ** 2 for value in values) / len(
            values
        )
        result[name] = {
            "sample_count": len(values),
            "mean": mean,
            "minimum": min(values),
            "maximum": max(values),
            "variance": variance,
            "first_last_delta": values[-1] - values[0],
        }
    return result


def _comparison_capture_context(
    workspace: Path,
    comparison: MechanicalComparisonReceiptV1,
    state: dict[str, Any],
) -> list[dict[str, Any]]:
    rows = []
    for receipt_id in (
        comparison.baseline_receipt_id,
        comparison.candidate_receipt_id,
    ):
        receipt = state["receipts"].get(receipt_id)
        if receipt is None:
            continue
        samples = _verified_fixture_samples(workspace, receipt)
        rows.append(
            {
                "receipt_id": receipt.receipt_id,
                "role": receipt.role,
                "cohort": receipt.cohort,
                "declared_sample_count": receipt.sample_count,
                "fixture_verified": samples is not None,
                "sufficient": receipt.sufficient,
                "identity_relation": receipt.identity_relation,
                "process_identity_sha256": (
                    receipt.process_identity_sha256
                ),
                "deployment_identity_sha256": (
                    receipt.deployment_identity_sha256
                ),
                "gap_refs": list(receipt.gap_refs),
                "descriptive_metrics": (
                    _descriptive_metrics(samples)
                    if samples is not None
                    else {}
                ),
                "mechanical_context_only": True,
                "felt_texture_inferred": False,
                "causation_established": False,
                "raw_prose_included": False,
            }
        )
    return rows


def _status(state: dict[str, Any], *, write: bool, appended: int) -> dict[str, Any]:
    campaigns = state["campaigns"]
    plans = state["plans"]
    receipts = state["receipts"]
    comparisons = state["comparisons"]
    reviews = state["reviews"]
    review_counts = Counter(review.outcome for review in reviews.values())
    review_opportunities = group_review_opportunities(reviews.values())
    sufficient_receipts = sum(receipt.sufficient for receipt in receipts.values())
    felt_reviewed_comparisons = {
        review.comparison_id
        for review in reviews.values()
        if review.outcome != "no_response"
    }
    return {
        "schema": "evidence_study_runtime_status_v1",
        "schema_version": 1,
        "valid": not state["errors"],
        "write": write,
        "campaign_count": len(campaigns),
        "study_count": len(plans),
        "plan_version_count": sum(len(items) for items in state["plan_history"].values()),
        "window_count": len(state["windows"]),
        "active_window_count": len(state["windows"]) - len(state["stopped"]),
        "window_receipt_count": len(receipts),
        "sufficient_window_receipt_count": sufficient_receipts,
        "capture_gap_count": len(state["gaps"]),
        "mechanical_comparison_count": len(comparisons),
        "review_receipt_count": len(reviews),
        "review_opportunity_count": len(review_opportunities),
        "review_pending_count": (
            len(comparisons) - len(felt_reviewed_comparisons)
        ),
        "review_outcome_counts": dict(sorted(review_counts.items())),
        "appended_event_count": appended,
        "comparison_outcomes_are_mechanical_only": True,
        "silence_creates_felt_result": False,
        "causation_established": False,
        "closure_propagated": False,
        "errors": state["errors"],
        "counter_audit": {
            "status": "consistent" if not state["errors"] else "inconsistent",
            "checks": {
                "one_current_plan_per_study": len(plans)
                == len(state["plan_history"]),
                "comparisons_have_baselines": all(
                    comparison.baseline_receipt_id in receipts
                    for comparison in comparisons.values()
                ),
                "all_authority_evidence_only": True,
                "no_felt_result_from_silence": all(
                    review.outcome != "no_response"
                    or review.to_dict()["felt_result_established"] is False
                    for review in reviews.values()
                ),
                "review_budgets_count_opportunities": all(
                    len(
                        group_review_opportunities(
                            review
                            for review in reviews.values()
                            if review.campaign_id == campaign.campaign_id
                        )
                    )
                    <= campaign.review_opportunity_limit
                    for campaign in campaigns.values()
                ),
            },
        },
        "artifact_authority_state_v1": authority_state(),
    }


def _review_packet(
    campaign: EvidenceStudyCampaignV1,
    state: dict[str, Any],
    *,
    workspace: Path | None = None,
) -> dict[str, Any]:
    comparisons = sorted(
        (
            comparison
            for comparison in state["comparisons"].values()
            if comparison.campaign_id == campaign.campaign_id
        ),
        key=lambda item: item.comparison_id,
    )
    reviews = [
        review
        for review in state["reviews"].values()
        if review.campaign_id == campaign.campaign_id
    ]
    opportunities = group_review_opportunities(reviews)
    latest = opportunities[-1] if opportunities else ()
    pending_comparison_ids = {
        item.comparison_id for item in comparisons
    } - {
        item.comparison_id
        for item in reviews
        if item.outcome != "no_response"
    }
    latest_named_friction = any(
        item.outcome
        in {"mechanism_smooth_felt_friction_remains", "contradicted"}
        for item in latest
    )
    latest_is_silence = bool(latest) and all(
        item.outcome == "no_response" for item in latest
    )
    if latest_is_silence:
        review_state = "review_pending"
    elif latest_named_friction and (
        len(opportunities) < campaign.review_opportunity_limit
    ):
        review_state = "named_friction_follow_up_available"
    elif latest_named_friction:
        review_state = "named_friction_review_budget_exhausted"
    elif pending_comparison_ids:
        review_state = "review_pending" if reviews else "ready_for_felt_review"
    elif reviews:
        review_state = "review_recorded"
    elif comparisons:
        review_state = "ready_for_felt_review"
    else:
        review_state = "awaiting_mechanical_comparison"
    return {
        "schema": "evidence_study_review_packet_v1",
        "schema_version": 1,
        "campaign_id": campaign.campaign_id,
        "comparison_domain": campaign.comparison_domain,
        "study_ids": list(campaign.study_ids),
        "mechanical_comparisons": [
            {
                "comparison_id": item.comparison_id,
                "study_id": item.study_id,
                "outcome": item.outcome,
            }
            for item in comparisons
        ],
        "descriptive_capture_context": [
            {
                "comparison_id": item.comparison_id,
                "study_id": item.study_id,
                "cohorts": _comparison_capture_context(
                    workspace, item, state
                ),
            }
            for item in comparisons
        ]
        if workspace is not None
        else [],
        "review_receipt_refs": [item.review_id for item in reviews],
        "review_opportunity_count": len(opportunities),
        "review_opportunity_limit": campaign.review_opportunity_limit,
        "pending_comparison_ids": sorted(pending_comparison_ids),
        "qualitative_context_receipts": [
            {
                "review_id": item.review_id,
                "comparison_id": item.comparison_id,
                "outcome": item.outcome,
                "source_ref": item.source_ref,
                "source_field_refs": list(item.source_field_refs),
                "mapping_link_v1": {
                    "mechanical_comparison_id": item.comparison_id,
                    "felt_source_ref": item.source_ref,
                    "felt_source_field_refs": list(
                        item.source_field_refs
                    ),
                    "pointer_only": True,
                    "calculation_performed": False,
                    "causation_established": False,
                },
                "context_relation": "explicit_post_window_felt_source",
                "unscored": True,
                "mechanical_comparison_modified": False,
                "raw_prose_included": False,
            }
            for item in reviews
            if item.outcome != "no_response"
        ],
        "review_state": review_state,
        "right_to_ignore": True,
        "follow_up_requires_named_friction": (
            campaign.follow_up_requires_named_friction
        ),
        "packet_establishes_felt_result": False,
        "silence_is_neutral": True,
        "closure_propagated": False,
        "causation_established": False,
        "artifact_authority_state_v1": authority_state(),
    }


def project(workspace: Path, *, write: bool) -> dict[str, Any]:
    state = replay(workspace)
    payloads = []
    for event in state["events"]:
        record = dict(event["record"])
        try:
            from experiential_epistemics import lint_value
        except ModuleNotFoundError:
            from scripts.experiential_epistemics import lint_value
        epistemic_issues = lint_value(record)
        if epistemic_issues:
            state["errors"].append(
                f"event_{event['event_id']}:epistemic_lint:{epistemic_issues[0]}"
            )
            continue
        aggregate_id = str(
            record.get("campaign_id")
            or record.get("study_id")
            or record.get("window_id")
            or event["event_id"]
        )
        payloads.append(
            event_payload(
                schema=SCHEMA,
                event_type=str(event["event_type"]),
                aggregate_type="evidence_study",
                aggregate_id=aggregate_id,
                idempotency_key=f"evidence-study:{event['event_id']}",
                record=record,
            )
        )
    appended = (
        project_events(
            workspace,
            STREAM,
            payloads,
            actor="evidence-study-projector",
            source_kind="preregistered_study_runtime",
            source_locator_value=(
                "diagnostics/evidence_study_runtime_v1/operator_events.jsonl"
            ),
        )
        if write and not state["errors"]
        else 0
    )
    status = _status(state, write=write, appended=appended)
    if write and status["valid"]:
        root = state_dir(workspace)
        owner_atomic_write_jsonl(
            root / "campaigns.jsonl",
            _output_rows(state, "campaigns", "campaign_id"),
        )
        plans = [
            plan.to_dict()
            for _, history in sorted(state["plan_history"].items())
            for plan in history
        ]
        owner_atomic_write_jsonl(root / "plans.jsonl", plans)
        owner_atomic_write_jsonl(
            root / "windows.jsonl", _output_rows(state, "windows", "window_id")
        )
        owner_atomic_write_jsonl(
            root / "window_receipts.jsonl",
            _output_rows(state, "receipts", "receipt_id"),
        )
        owner_atomic_write_jsonl(
            root / "comparisons.jsonl",
            _output_rows(state, "comparisons", "comparison_id"),
        )
        owner_atomic_write_jsonl(
            root / "capture_gaps.jsonl", _output_rows(state, "gaps", "gap_id")
        )
        owner_atomic_write_jsonl(
            root / "reviews.jsonl", _output_rows(state, "reviews", "review_id")
        )
        review_packet_paths = []
        for campaign in sorted(
            state["campaigns"].values(), key=lambda item: item.campaign_id
        ):
            path = root / "review_packets" / f"{campaign.campaign_id}.json"
            owner_atomic_write_json(
                path, _review_packet(campaign, state, workspace=workspace)
            )
            review_packet_paths.append(path)
        owner_atomic_write(
            root / "report.md",
            "# Evidence-to-Study Runtime\n\n"
            "Mechanical comparisons remain evidence-only. Silence leaves felt review pending.\n\n"
            f"- Campaigns: {status['campaign_count']}\n"
            f"- Studies: {status['study_count']}\n"
            f"- Capture gaps: {status['capture_gap_count']}\n"
            f"- Mechanical comparisons: {status['mechanical_comparison_count']}\n"
            f"- Felt reviews pending: {status['review_pending_count']}\n",
        )
        projected_outputs = {
            path.name: _sha256_file(path)
            for path in (
                root / "campaigns.jsonl",
                root / "plans.jsonl",
                root / "windows.jsonl",
                root / "window_receipts.jsonl",
                root / "comparisons.jsonl",
                root / "capture_gaps.jsonl",
                root / "reviews.jsonl",
                root / "report.md",
            )
        }
        projected_outputs.update(
            {
                f"review_packets/{path.name}": _sha256_file(path)
                for path in review_packet_paths
            }
        )
        status["projection_hashes"] = projected_outputs
        owner_atomic_write_json(root / "status.json", status)
        checkpoint_outputs = {
            **projected_outputs,
            "status.json": _sha256_file(root / "status.json"),
        }
        store = EvidenceEventStore(
            workspace / "diagnostics/evidence_event_store_v2"
        )
        store.write_checkpoint(
            "evidence_study_runtime_v1",
            PROJECTOR_VERSION,
            checkpoint_outputs,
            input_streams=(STREAM,),
            source_hashes={
                "operator_events": _sha256_file(operator_path(workspace)),
                "samples": _sample_source_hash(workspace),
                "seed_manifest": _sha256_file(DEFAULT_MANIFEST),
            },
            dependency_output_hashes={},
            command_sha256=hashlib.sha256(
                b"evidence_study_runtime_v1:project"
            ).hexdigest(),
            config_sha256=_sha256_file(DEFAULT_MANIFEST),
        )
    return status


def query(workspace: Path, identifier: str) -> dict[str, Any]:
    state = replay(workspace)
    for history in state["plan_history"].values():
        for plan in history:
            if plan.plan_id == identifier:
                return {"valid": True, "kind": "plans", "record": plan.to_dict()}
    for key in (
        "campaigns",
        "windows",
        "receipts",
        "comparisons",
        "gaps",
        "reviews",
    ):
        item = state[key].get(identifier)
        if item is not None:
            return {"valid": True, "kind": key, "record": item.to_dict()}
    return {"valid": False, "error": "study record not found"}
