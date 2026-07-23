"""Capture-gap and felt-review receipts for evidence studies."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Iterable

try:
    from experiential_systems.common import (
        RecordValidationError,
        authority_state,
        deterministic_id,
        validate_bounded_identifier,
        validate_evidence_record,
        validate_timestamp,
    )
except ModuleNotFoundError:
    from scripts.experiential_systems.common import (
        RecordValidationError,
        authority_state,
        deterministic_id,
        validate_bounded_identifier,
        validate_evidence_record,
        validate_timestamp,
    )

from .model import (
    EvidenceStudyCampaignV1,
    ReviewOutcomeV1,
    _TRUSTED,
    _nonnegative_int,
)


@dataclass(frozen=True)
class StudyCaptureGapReceiptV1:
    gap_id: str
    window_id: str
    study_id: str
    reason: str
    dropped_sample_count: int
    observed_at_unix_ms: int
    source_ref: str
    _token: object = field(repr=False, compare=False)

    def __post_init__(self) -> None:
        if self._token is not _TRUSTED:
            raise RecordValidationError("capture gaps require the internal builder")

    @classmethod
    def build(
        cls,
        *,
        window_id: str,
        study_id: str,
        reason: str,
        dropped_sample_count: int,
        observed_at_unix_ms: int,
        source_ref: str,
    ) -> StudyCaptureGapReceiptV1:
        window = validate_bounded_identifier(window_id, "window_id") or ""
        study = validate_bounded_identifier(study_id, "study_id") or ""
        bounded_reason = validate_bounded_identifier(reason, "reason", limit=120) or ""
        if bounded_reason not in {
            "queue_exhausted",
            "writer_disconnected",
            "asynchronous_write_failed",
            "crash_recovery_gap",
            "sample_limit_reached",
            "identity_mismatch",
            "required_cohort_missing",
        }:
            raise RecordValidationError("unsupported study capture gap reason")
        dropped = _nonnegative_int(dropped_sample_count, "dropped_sample_count")
        observed = validate_timestamp(observed_at_unix_ms, "observed_at_unix_ms")
        ref = validate_bounded_identifier(source_ref, "source_ref", limit=240) or ""
        core = {
            "window_id": window,
            "study_id": study,
            "reason": bounded_reason,
            "dropped_sample_count": dropped,
            "observed_at_unix_ms": observed,
            "source_ref": ref,
        }
        return cls(
            deterministic_id("studygap", core),
            window,
            study,
            bounded_reason,
            dropped,
            observed,
            ref,
            _TRUSTED,
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": "study_capture_gap_receipt_v1",
            "schema_version": 1,
            "gap_id": self.gap_id,
            "window_id": self.window_id,
            "study_id": self.study_id,
            "reason": self.reason,
            "dropped_sample_count": self.dropped_sample_count,
            "observed_at_unix_ms": self.observed_at_unix_ms,
            "source_ref": self.source_ref,
            "study_sufficient": False,
            "behavior_blocked": False,
            "raw_prose_included": False,
            "artifact_authority_state_v1": authority_state(),
        }

    @classmethod
    def from_untrusted(cls, value: Any) -> StudyCaptureGapReceiptV1:
        if not isinstance(value, dict):
            raise RecordValidationError("capture gap must be an object")
        validate_evidence_record(value)
        built = cls.build(
            window_id=value.get("window_id"),
            study_id=value.get("study_id"),
            reason=value.get("reason"),
            dropped_sample_count=value.get("dropped_sample_count"),
            observed_at_unix_ms=value.get("observed_at_unix_ms"),
            source_ref=value.get("source_ref"),
        )
        if value.get("gap_id") != built.gap_id:
            raise RecordValidationError("capture gap identity mismatch")
        if (
            value.get("study_sufficient") is not False
            or value.get("behavior_blocked") is not False
            or value.get("raw_prose_included") is not False
        ):
            raise RecordValidationError("capture gap safety boundary mismatch")
        return built


@dataclass(frozen=True)
class StudyReviewReceiptV1:
    review_id: str
    campaign_id: str
    study_id: str
    comparison_id: str
    outcome: str
    source_ref: str
    source_field_refs: tuple[str, ...]
    opportunity_completed: bool
    _token: object = field(repr=False, compare=False)

    def __post_init__(self) -> None:
        if self._token is not _TRUSTED:
            raise RecordValidationError("review receipts require the internal builder")

    @classmethod
    def build(
        cls,
        *,
        campaign_id: str,
        study_id: str,
        comparison_id: str,
        outcome: str,
        source_ref: str,
        source_field_refs: Iterable[str] = (),
        opportunity_completed: bool,
    ) -> StudyReviewReceiptV1:
        campaign = validate_bounded_identifier(campaign_id, "campaign_id") or ""
        study = validate_bounded_identifier(study_id, "study_id") or ""
        comparison = validate_bounded_identifier(comparison_id, "comparison_id") or ""
        resolved = ReviewOutcomeV1(outcome).value
        ref = validate_bounded_identifier(source_ref, "source_ref", limit=240) or ""
        raw_field_refs = list(source_field_refs)
        if len(raw_field_refs) > 16:
            raise RecordValidationError(
                "review source_field_refs exceeds the bounded maximum"
            )
        field_refs = tuple(
            sorted(
                {
                    validate_bounded_identifier(
                        item, "source_field_ref", limit=200
                    )
                    or ""
                    for item in raw_field_refs
                }
            )
        )
        if not isinstance(opportunity_completed, bool) or not opportunity_completed:
            raise RecordValidationError(
                "review outcome requires an explicitly completed opportunity"
            )
        core = {
            "campaign_id": campaign,
            "study_id": study,
            "comparison_id": comparison,
            "outcome": resolved,
            "source_ref": ref,
            "opportunity_completed": opportunity_completed,
        }
        if field_refs:
            core["source_field_refs"] = list(field_refs)
        return cls(
            deterministic_id("studyreview", core),
            campaign,
            study,
            comparison,
            resolved,
            ref,
            field_refs,
            opportunity_completed,
            _TRUSTED,
        )

    def to_dict(self) -> dict[str, Any]:
        no_response = self.outcome == ReviewOutcomeV1.NO_RESPONSE.value
        return {
            "schema": "study_review_receipt_v1",
            "schema_version": 1,
            "review_id": self.review_id,
            "campaign_id": self.campaign_id,
            "study_id": self.study_id,
            "comparison_id": self.comparison_id,
            "outcome": self.outcome,
            "source_ref": self.source_ref,
            "source_field_refs": list(self.source_field_refs),
            "opportunity_completed": self.opportunity_completed,
            "felt_result_established": not no_response,
            "review_pending": no_response,
            "silence_affirms": False,
            "closure_propagated": False,
            "raw_prose_included": False,
            "artifact_authority_state_v1": authority_state(),
        }

    @classmethod
    def from_untrusted(cls, value: Any) -> StudyReviewReceiptV1:
        if not isinstance(value, dict):
            raise RecordValidationError("review receipt must be an object")
        validate_evidence_record(value)
        field_refs = value.get("source_field_refs", [])
        if not isinstance(field_refs, list):
            raise RecordValidationError(
                "review source_field_refs must be a list"
            )
        built = cls.build(
            campaign_id=value.get("campaign_id"),
            study_id=value.get("study_id"),
            comparison_id=value.get("comparison_id"),
            outcome=value.get("outcome"),
            source_ref=value.get("source_ref"),
            source_field_refs=field_refs,
            opportunity_completed=value.get("opportunity_completed"),
        )
        no_response = built.outcome == ReviewOutcomeV1.NO_RESPONSE.value
        if value.get("review_id") != built.review_id:
            raise RecordValidationError("review receipt identity mismatch")
        if (
            value.get("felt_result_established") is not (not no_response)
            or value.get("review_pending") is not no_response
            or value.get("silence_affirms") is not False
            or value.get("closure_propagated") is not False
            or value.get("raw_prose_included") is not False
        ):
            raise RecordValidationError("review silence or closure boundary mismatch")
        return built


def group_review_opportunities(
    reviews: Iterable[StudyReviewReceiptV1],
) -> tuple[tuple[StudyReviewReceiptV1, ...], ...]:
    """Group independent study receipts from one delivered campaign review."""

    grouped: dict[tuple[str, str], list[StudyReviewReceiptV1]] = {}
    for review in reviews:
        key = (review.campaign_id, review.source_ref)
        grouped.setdefault(key, []).append(review)
    return tuple(tuple(items) for items in grouped.values())


def validate_review_admission(
    campaign: EvidenceStudyCampaignV1,
    prior_reviews: Iterable[StudyReviewReceiptV1],
    review: StudyReviewReceiptV1,
) -> None:
    """Enforce a campaign opportunity budget without conflating its studies."""

    if review.campaign_id != campaign.campaign_id:
        raise RecordValidationError("review names a different campaign")
    prior = [
        item
        for item in prior_reviews
        if item.campaign_id == campaign.campaign_id
    ]
    same_opportunity = [
        item for item in prior if item.source_ref == review.source_ref
    ]
    if any(item.study_id == review.study_id for item in same_opportunity):
        raise RecordValidationError(
            "one review opportunity may record each study only once"
        )
    if same_opportunity:
        return
    opportunities = group_review_opportunities(prior)
    if len(opportunities) >= campaign.review_opportunity_limit:
        raise RecordValidationError("campaign review budget exceeded")
    if (
        opportunities
        and campaign.follow_up_requires_named_friction
        and all(
            item.outcome
            not in {
                ReviewOutcomeV1.SMOOTH_FRICTION_REMAINS.value,
                ReviewOutcomeV1.CONTRADICTED.value,
            }
            for item in prior
        )
    ):
        raise RecordValidationError(
            "follow-up review requires named friction or contradiction"
        )
