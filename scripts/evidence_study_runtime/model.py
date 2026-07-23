"""Immutable validated records for capture-first experiential studies."""

from __future__ import annotations

from dataclasses import dataclass, field
from enum import StrEnum
from typing import Any, Mapping

try:
    from experiential_systems.common import (
        RecordValidationError,
        authority_state,
        canonical_json,
        deterministic_id,
        sha256_bytes,
        validate_bounded_identifier,
        validate_evidence_record,
        validate_sha256,
        validate_timestamp,
    )
except ModuleNotFoundError:
    from scripts.experiential_systems.common import (
        RecordValidationError,
        authority_state,
        canonical_json,
        deterministic_id,
        sha256_bytes,
        validate_bounded_identifier,
        validate_evidence_record,
        validate_sha256,
        validate_timestamp,
    )

_TRUSTED = object()
DEFAULT_DURATION_MINUTES = 30
DEFAULT_SAMPLE_LIMIT = 2_048
MAX_DURATION_MINUTES = 120
MAX_SAMPLE_LIMIT = 8_192


class ComparisonKindV1(StrEnum):
    OBSERVATIONAL_CONTEXT = "observational_context"
    OFFLINE_COUNTERFACTUAL = "offline_counterfactual"


class MechanicalOutcomeV1(StrEnum):
    DIFFERENCE_OBSERVED = "difference_observed"
    NO_DIFFERENCE_OBSERVED = "no_difference_observed"
    INSUFFICIENT = "insufficient"


class ReviewOutcomeV1(StrEnum):
    NO_RESPONSE = "no_response"
    CORROBORATED = "corroborated"
    SMOOTH_FRICTION_REMAINS = "mechanism_smooth_felt_friction_remains"
    CONTRADICTED = "contradicted"
    INSUFFICIENT = "insufficient"


def _positive_int(value: Any, field_name: str, *, maximum: int | None = None) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
        raise RecordValidationError(f"{field_name} must be a positive integer")
    if maximum is not None and value > maximum:
        raise RecordValidationError(f"{field_name} exceeds hard maximum {maximum}")
    return value


def _nonnegative_int(value: Any, field_name: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise RecordValidationError(f"{field_name} must be a nonnegative integer")
    return value


def _bounded_ids(
    values: Any,
    field_name: str,
    *,
    minimum: int = 0,
    maximum: int = 64,
) -> tuple[str, ...]:
    if not isinstance(values, (list, tuple)):
        raise RecordValidationError(f"{field_name} must be a list")
    if not minimum <= len(values) <= maximum:
        raise RecordValidationError(
            f"{field_name} must contain between {minimum} and {maximum} values"
        )
    result = tuple(
        validate_bounded_identifier(value, field_name, limit=200) or ""
        for value in values
    )
    if len(set(result)) != len(result):
        raise RecordValidationError(f"{field_name} contains duplicates")
    return result


def _finite_scalar(value: Any, field_name: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise RecordValidationError(f"{field_name} must be numeric")
    resolved = float(value)
    if not (-1.0e12 < resolved < 1.0e12):
        raise RecordValidationError(f"{field_name} must be finite and bounded")
    return resolved


def _bounded_scalars(values: Any, field_name: str, *, maximum: int = 64) -> dict[str, float]:
    if not isinstance(values, Mapping) or len(values) > maximum:
        raise RecordValidationError(f"{field_name} must be a bounded mapping")
    result: dict[str, float] = {}
    for key, value in values.items():
        bounded_key = validate_bounded_identifier(key, field_name, limit=120) or ""
        result[bounded_key] = _finite_scalar(value, f"{field_name}.{bounded_key}")
    return dict(sorted(result.items()))


def _record_hash(record: Mapping[str, Any], *, omit: tuple[str, ...] = ()) -> str:
    bounded = {key: value for key, value in record.items() if key not in omit}
    return sha256_bytes(canonical_json(bounded).encode("utf-8"))


@dataclass(frozen=True)
class EvidenceStudyCampaignV1:
    campaign_id: str
    campaign_key: str
    comparison_domain: str
    study_ids: tuple[str, ...]
    review_opportunity_limit: int
    follow_up_requires_named_friction: bool
    _token: object = field(repr=False, compare=False)

    def __post_init__(self) -> None:
        if self._token is not _TRUSTED:
            raise RecordValidationError("campaigns require the internal builder")

    @classmethod
    def build(
        cls,
        *,
        campaign_key: str,
        comparison_domain: str,
        study_ids: list[str],
        review_opportunity_limit: int = 1,
        follow_up_requires_named_friction: bool = True,
    ) -> EvidenceStudyCampaignV1:
        key = validate_bounded_identifier(campaign_key, "campaign_key", limit=120) or ""
        domain = (
            validate_bounded_identifier(comparison_domain, "comparison_domain", limit=120)
            or ""
        )
        studies = _bounded_ids(study_ids, "study_ids", minimum=1, maximum=16)
        limit = _positive_int(
            review_opportunity_limit, "review_opportunity_limit", maximum=2
        )
        if not isinstance(follow_up_requires_named_friction, bool):
            raise RecordValidationError(
                "follow_up_requires_named_friction must be boolean"
            )
        core = {
            "campaign_key": key,
            "comparison_domain": domain,
            "study_ids": list(studies),
        }
        return cls(
            deterministic_id("campaign", core),
            key,
            domain,
            studies,
            limit,
            follow_up_requires_named_friction,
            _TRUSTED,
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": "evidence_study_campaign_v1",
            "schema_version": 1,
            "campaign_id": self.campaign_id,
            "campaign_key": self.campaign_key,
            "comparison_domain": self.comparison_domain,
            "study_ids": list(self.study_ids),
            "review_opportunity_limit": self.review_opportunity_limit,
            "follow_up_requires_named_friction": self.follow_up_requires_named_friction,
            "membership_propagates_authority": False,
            "membership_propagates_evidence": False,
            "membership_propagates_outcome": False,
            "membership_propagates_closure": False,
            "raw_prose_included": False,
            "artifact_authority_state_v1": authority_state(),
        }

    @classmethod
    def from_untrusted(cls, value: Any) -> EvidenceStudyCampaignV1:
        if not isinstance(value, dict):
            raise RecordValidationError("campaign must be an object")
        validate_evidence_record(value)
        built = cls.build(
            campaign_key=value.get("campaign_key"),
            comparison_domain=value.get("comparison_domain"),
            study_ids=value.get("study_ids"),
            review_opportunity_limit=value.get("review_opportunity_limit"),
            follow_up_requires_named_friction=value.get(
                "follow_up_requires_named_friction"
            ),
        )
        if value.get("campaign_id") != built.campaign_id:
            raise RecordValidationError("campaign identity mismatch")
        if any(
            value.get(field_name) is not False
            for field_name in (
                "membership_propagates_authority",
                "membership_propagates_evidence",
                "membership_propagates_outcome",
                "membership_propagates_closure",
                "raw_prose_included",
            )
        ):
            raise RecordValidationError("campaign propagation boundary mismatch")
        return built


@dataclass(frozen=True)
class EvidenceStudyPlanV1:
    plan_id: str
    plan_sha256: str
    plan_version: int
    frozen_prior_plan_sha256: str | None
    campaign_id: str
    concordance_study_id: str
    canonical_claim_id: str
    dossier_id: str
    witness_id: str
    sample_kind: str
    comparison_kind: str
    baseline_cohort: str
    candidate_cohort: str
    metric_names: tuple[str, ...]
    thresholds: dict[str, float]
    minimum_total_samples: int
    minimum_baseline_samples: int
    minimum_candidate_samples: int
    duration_minutes: int
    sample_limit: int
    extension_limit: int
    intervention_signature_sha256: str
    _token: object = field(repr=False, compare=False)

    def __post_init__(self) -> None:
        if self._token is not _TRUSTED:
            raise RecordValidationError("plans require the internal builder")

    @classmethod
    def build(
        cls,
        *,
        plan_version: int,
        frozen_prior_plan_sha256: str | None,
        campaign_id: str,
        concordance_study_id: str,
        canonical_claim_id: str,
        dossier_id: str,
        witness_id: str,
        sample_kind: str,
        comparison_kind: str,
        baseline_cohort: str,
        candidate_cohort: str,
        metric_names: list[str],
        thresholds: Mapping[str, Any],
        minimum_total_samples: int,
        minimum_baseline_samples: int,
        minimum_candidate_samples: int,
        duration_minutes: int = DEFAULT_DURATION_MINUTES,
        sample_limit: int = DEFAULT_SAMPLE_LIMIT,
        extension_limit: int = 0,
        intervention_signature_sha256: str,
    ) -> EvidenceStudyPlanV1:
        version = _positive_int(plan_version, "plan_version")
        prior = validate_sha256(
            frozen_prior_plan_sha256, "frozen_prior_plan_sha256", optional=True
        )
        if version == 1 and prior is not None:
            raise RecordValidationError("first plan version cannot name a prior hash")
        if version > 1 and prior is None:
            raise RecordValidationError("revised plan must freeze the prior hash")
        campaign = validate_bounded_identifier(campaign_id, "campaign_id") or ""
        study = (
            validate_bounded_identifier(concordance_study_id, "concordance_study_id")
            or ""
        )
        claim = (
            validate_bounded_identifier(canonical_claim_id, "canonical_claim_id")
            or ""
        )
        dossier = validate_bounded_identifier(dossier_id, "dossier_id") or ""
        witness = validate_bounded_identifier(witness_id, "witness_id") or ""
        kind = validate_bounded_identifier(sample_kind, "sample_kind", limit=80) or ""
        comparison = ComparisonKindV1(comparison_kind).value
        baseline = (
            validate_bounded_identifier(baseline_cohort, "baseline_cohort", limit=120)
            or ""
        )
        candidate = (
            validate_bounded_identifier(candidate_cohort, "candidate_cohort", limit=120)
            or ""
        )
        if baseline == candidate:
            raise RecordValidationError("baseline and candidate cohorts must differ")
        metrics = _bounded_ids(metric_names, "metric_names", minimum=1, maximum=32)
        bounded_thresholds = _bounded_scalars(thresholds, "thresholds", maximum=16)
        minimum_total = _positive_int(
            minimum_total_samples, "minimum_total_samples", maximum=MAX_SAMPLE_LIMIT
        )
        minimum_baseline = _positive_int(
            minimum_baseline_samples,
            "minimum_baseline_samples",
            maximum=MAX_SAMPLE_LIMIT,
        )
        minimum_candidate = _positive_int(
            minimum_candidate_samples,
            "minimum_candidate_samples",
            maximum=MAX_SAMPLE_LIMIT,
        )
        if minimum_total < minimum_baseline + minimum_candidate:
            raise RecordValidationError(
                "minimum total must cover baseline and candidate minima"
            )
        duration = _positive_int(
            duration_minutes, "duration_minutes", maximum=MAX_DURATION_MINUTES
        )
        limit = _positive_int(sample_limit, "sample_limit", maximum=MAX_SAMPLE_LIMIT)
        extension = _nonnegative_int(extension_limit, "extension_limit")
        if extension > 1:
            raise RecordValidationError("at most one capture extension is permitted")
        signature = (
            validate_sha256(
                intervention_signature_sha256, "intervention_signature_sha256"
            )
            or ""
        )
        core = {
            "plan_version": version,
            "frozen_prior_plan_sha256": prior,
            "campaign_id": campaign,
            "concordance_study_id": study,
            "canonical_claim_id": claim,
            "dossier_id": dossier,
            "witness_id": witness,
            "sample_kind": kind,
            "comparison_kind": comparison,
            "baseline_cohort": baseline,
            "candidate_cohort": candidate,
            "metric_names": list(metrics),
            "thresholds": bounded_thresholds,
            "minimum_total_samples": minimum_total,
            "minimum_baseline_samples": minimum_baseline,
            "minimum_candidate_samples": minimum_candidate,
            "duration_minutes": duration,
            "sample_limit": limit,
            "extension_limit": extension,
            "intervention_signature_sha256": signature,
        }
        plan_sha256 = _record_hash(core)
        return cls(
            deterministic_id("studyplan", {"study": study, "version": version, "hash": plan_sha256}),
            plan_sha256,
            version,
            prior,
            campaign,
            study,
            claim,
            dossier,
            witness,
            kind,
            comparison,
            baseline,
            candidate,
            metrics,
            bounded_thresholds,
            minimum_total,
            minimum_baseline,
            minimum_candidate,
            duration,
            limit,
            extension,
            signature,
            _TRUSTED,
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": "evidence_study_plan_v1",
            "schema_version": 1,
            "plan_id": self.plan_id,
            "plan_sha256": self.plan_sha256,
            "plan_version": self.plan_version,
            "frozen_prior_plan_sha256": self.frozen_prior_plan_sha256,
            "campaign_id": self.campaign_id,
            "concordance_study_id": self.concordance_study_id,
            "canonical_claim_id": self.canonical_claim_id,
            "dossier_id": self.dossier_id,
            "witness_id": self.witness_id,
            "sample_kind": self.sample_kind,
            "comparison_kind": self.comparison_kind,
            "baseline_cohort": self.baseline_cohort,
            "candidate_cohort": self.candidate_cohort,
            "metric_names": list(self.metric_names),
            "thresholds": self.thresholds,
            "minimum_total_samples": self.minimum_total_samples,
            "minimum_baseline_samples": self.minimum_baseline_samples,
            "minimum_candidate_samples": self.minimum_candidate_samples,
            "duration_minutes": self.duration_minutes,
            "sample_limit": self.sample_limit,
            "extension_limit": self.extension_limit,
            "intervention_signature_sha256": self.intervention_signature_sha256,
            "preregistered_before_capture": True,
            "causation_established": False,
            "felt_outcome_inferred": False,
            "closure_propagated": False,
            "raw_prose_included": False,
            "artifact_authority_state_v1": authority_state(),
        }

    @classmethod
    def from_untrusted(cls, value: Any) -> EvidenceStudyPlanV1:
        if not isinstance(value, dict):
            raise RecordValidationError("study plan must be an object")
        validate_evidence_record(value)
        built = cls.build(
            plan_version=value.get("plan_version"),
            frozen_prior_plan_sha256=value.get("frozen_prior_plan_sha256"),
            campaign_id=value.get("campaign_id"),
            concordance_study_id=value.get("concordance_study_id"),
            canonical_claim_id=value.get("canonical_claim_id"),
            dossier_id=value.get("dossier_id"),
            witness_id=value.get("witness_id"),
            sample_kind=value.get("sample_kind"),
            comparison_kind=value.get("comparison_kind"),
            baseline_cohort=value.get("baseline_cohort"),
            candidate_cohort=value.get("candidate_cohort"),
            metric_names=value.get("metric_names"),
            thresholds=value.get("thresholds"),
            minimum_total_samples=value.get("minimum_total_samples"),
            minimum_baseline_samples=value.get("minimum_baseline_samples"),
            minimum_candidate_samples=value.get("minimum_candidate_samples"),
            duration_minutes=value.get("duration_minutes"),
            sample_limit=value.get("sample_limit"),
            extension_limit=value.get("extension_limit"),
            intervention_signature_sha256=value.get(
                "intervention_signature_sha256"
            ),
        )
        if value.get("plan_id") != built.plan_id or value.get("plan_sha256") != built.plan_sha256:
            raise RecordValidationError("study plan identity or frozen hash mismatch")
        if value.get("preregistered_before_capture") is not True:
            raise RecordValidationError("study plan must be preregistered")
        if any(
            value.get(field_name) is not False
            for field_name in (
                "causation_established",
                "felt_outcome_inferred",
                "closure_propagated",
                "raw_prose_included",
            )
        ):
            raise RecordValidationError("study plan epistemic boundary mismatch")
        return built


@dataclass(frozen=True)
class StudyWindowSpecV1:
    window_id: str
    campaign_id: str
    study_id: str
    plan_id: str
    plan_sha256: str
    sample_kinds: tuple[str, ...]
    started_at_unix_ms: int
    expires_at_unix_ms: int
    sample_limit: int
    actor: str
    signal_capture_window_ref: str | None
    extension_of_window_id: str | None
    _token: object = field(repr=False, compare=False)

    def __post_init__(self) -> None:
        if self._token is not _TRUSTED:
            raise RecordValidationError("window specs require the internal builder")

    @classmethod
    def build(
        cls,
        *,
        campaign_id: str,
        study_id: str,
        plan_id: str,
        plan_sha256: str,
        sample_kinds: list[str],
        started_at_unix_ms: int,
        expires_at_unix_ms: int,
        sample_limit: int,
        actor: str,
        signal_capture_window_ref: str | None = None,
        extension_of_window_id: str | None = None,
    ) -> StudyWindowSpecV1:
        campaign = validate_bounded_identifier(campaign_id, "campaign_id") or ""
        study = validate_bounded_identifier(study_id, "study_id") or ""
        plan = validate_bounded_identifier(plan_id, "plan_id") or ""
        plan_hash = validate_sha256(plan_sha256, "plan_sha256") or ""
        kinds = _bounded_ids(sample_kinds, "sample_kinds", minimum=1, maximum=4)
        allowed = {"telemetry", "heartbeat", "codec_lane", "codec_gate"}
        if any(kind not in allowed for kind in kinds):
            raise RecordValidationError("unsupported study sample kind")
        started = validate_timestamp(started_at_unix_ms, "started_at_unix_ms")
        expires = validate_timestamp(expires_at_unix_ms, "expires_at_unix_ms")
        if expires <= started:
            raise RecordValidationError("capture window expiry must follow start")
        if expires - started > MAX_DURATION_MINUTES * 60 * 1_000:
            raise RecordValidationError("capture window exceeds two-hour hard limit")
        limit = _positive_int(sample_limit, "sample_limit", maximum=MAX_SAMPLE_LIMIT)
        bounded_actor = validate_bounded_identifier(actor, "actor", limit=120) or ""
        signal_ref = validate_bounded_identifier(
            signal_capture_window_ref,
            "signal_capture_window_ref",
            optional=True,
            limit=200,
        )
        extension_of = validate_bounded_identifier(
            extension_of_window_id,
            "extension_of_window_id",
            optional=True,
            limit=200,
        )
        core = {
            "campaign_id": campaign,
            "study_id": study,
            "plan_id": plan,
            "plan_sha256": plan_hash,
            "sample_kinds": list(kinds),
            "started_at_unix_ms": started,
            "expires_at_unix_ms": expires,
            "sample_limit": limit,
            "actor": bounded_actor,
            "signal_capture_window_ref": signal_ref,
            "extension_of_window_id": extension_of,
        }
        return cls(
            deterministic_id("studywindow", core),
            campaign,
            study,
            plan,
            plan_hash,
            kinds,
            started,
            expires,
            limit,
            bounded_actor,
            signal_ref,
            extension_of,
            _TRUSTED,
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": "study_window_spec_v1",
            "schema_version": 1,
            "window_id": self.window_id,
            "campaign_id": self.campaign_id,
            "study_id": self.study_id,
            "plan_id": self.plan_id,
            "plan_sha256": self.plan_sha256,
            "sample_kinds": list(self.sample_kinds),
            "started_at_unix_ms": self.started_at_unix_ms,
            "expires_at_unix_ms": self.expires_at_unix_ms,
            "sample_limit": self.sample_limit,
            "actor": self.actor,
            "signal_capture_window_ref": self.signal_capture_window_ref,
            "extension_of_window_id": self.extension_of_window_id,
            "capture_can_delay_behavior": False,
            "preregistered_before_capture": True,
            "raw_prose_included": False,
            "artifact_authority_state_v1": authority_state(),
        }

    @classmethod
    def from_untrusted(cls, value: Any) -> StudyWindowSpecV1:
        if not isinstance(value, dict):
            raise RecordValidationError("study window must be an object")
        validate_evidence_record(value)
        built = cls.build(
            campaign_id=value.get("campaign_id"),
            study_id=value.get("study_id"),
            plan_id=value.get("plan_id"),
            plan_sha256=value.get("plan_sha256"),
            sample_kinds=value.get("sample_kinds"),
            started_at_unix_ms=value.get("started_at_unix_ms"),
            expires_at_unix_ms=value.get("expires_at_unix_ms"),
            sample_limit=value.get("sample_limit"),
            actor=value.get("actor"),
            signal_capture_window_ref=value.get("signal_capture_window_ref"),
            extension_of_window_id=value.get("extension_of_window_id"),
        )
        if value.get("window_id") != built.window_id:
            raise RecordValidationError("study window identity mismatch")
        if (
            value.get("capture_can_delay_behavior") is not False
            or value.get("preregistered_before_capture") is not True
            or value.get("raw_prose_included") is not False
        ):
            raise RecordValidationError("study window safety boundary mismatch")
        return built


@dataclass(frozen=True)
class StudyWindowReceiptV1:
    receipt_id: str
    window_id: str
    campaign_id: str
    study_id: str
    plan_id: str
    role: str
    comparison_kind: str
    cohort: str
    sample_count: int
    qualifying_sample_count: int
    sample_set_sha256: str
    scalar_fixture_ref: str
    scalar_fixture_sha256: str
    process_identity_sha256: str | None
    deployment_identity_sha256: str | None
    identity_relation: str
    gap_refs: tuple[str, ...]
    sufficient: bool
    _token: object = field(repr=False, compare=False)

    def __post_init__(self) -> None:
        if self._token is not _TRUSTED:
            raise RecordValidationError("window receipts require the internal builder")

    @classmethod
    def build(
        cls,
        *,
        window_id: str,
        campaign_id: str,
        study_id: str,
        plan_id: str,
        role: str,
        comparison_kind: str,
        cohort: str,
        sample_count: int,
        qualifying_sample_count: int,
        sample_set_sha256: str,
        scalar_fixture_ref: str,
        scalar_fixture_sha256: str,
        process_identity_sha256: str | None,
        deployment_identity_sha256: str | None,
        identity_relation: str,
        gap_refs: list[str],
        sufficient: bool,
    ) -> StudyWindowReceiptV1:
        window = validate_bounded_identifier(window_id, "window_id") or ""
        campaign = validate_bounded_identifier(campaign_id, "campaign_id") or ""
        study = validate_bounded_identifier(study_id, "study_id") or ""
        plan = validate_bounded_identifier(plan_id, "plan_id") or ""
        if role not in {"baseline", "candidate"}:
            raise RecordValidationError("window receipt role must be baseline or candidate")
        comparison = ComparisonKindV1(comparison_kind).value
        bounded_cohort = validate_bounded_identifier(cohort, "cohort", limit=120) or ""
        count = _nonnegative_int(sample_count, "sample_count")
        qualifying = _nonnegative_int(
            qualifying_sample_count, "qualifying_sample_count"
        )
        if qualifying > count:
            raise RecordValidationError("qualifying count exceeds sample count")
        sample_hash = validate_sha256(sample_set_sha256, "sample_set_sha256") or ""
        fixture_ref = (
            validate_bounded_identifier(
                scalar_fixture_ref, "scalar_fixture_ref", limit=240
            )
            or ""
        )
        fixture_hash = (
            validate_sha256(scalar_fixture_sha256, "scalar_fixture_sha256") or ""
        )
        process_hash = validate_sha256(
            process_identity_sha256, "process_identity_sha256", optional=True
        )
        deployment_hash = validate_sha256(
            deployment_identity_sha256,
            "deployment_identity_sha256",
            optional=True,
        )
        if identity_relation not in {
            "exact_identity",
            "temporal_association",
            "identity_unavailable",
        }:
            raise RecordValidationError("unsupported identity relation")
        if identity_relation == "exact_identity" and (
            process_hash is None or deployment_hash is None
        ):
            raise RecordValidationError(
                "exact identity requires process and deployment receipts"
            )
        gaps = _bounded_ids(gap_refs, "gap_refs", maximum=64)
        if not isinstance(sufficient, bool):
            raise RecordValidationError("sufficient must be boolean")
        if gaps and sufficient:
            raise RecordValidationError("capture gaps make a window insufficient")
        core = {
            "window_id": window,
            "campaign_id": campaign,
            "study_id": study,
            "plan_id": plan,
            "role": role,
            "comparison_kind": comparison,
            "cohort": bounded_cohort,
            "sample_count": count,
            "qualifying_sample_count": qualifying,
            "sample_set_sha256": sample_hash,
            "scalar_fixture_sha256": fixture_hash,
            "process_identity_sha256": process_hash,
            "deployment_identity_sha256": deployment_hash,
            "identity_relation": identity_relation,
            "gap_refs": list(gaps),
            "sufficient": sufficient,
        }
        return cls(
            deterministic_id("windowreceipt", core),
            window,
            campaign,
            study,
            plan,
            role,
            comparison,
            bounded_cohort,
            count,
            qualifying,
            sample_hash,
            fixture_ref,
            fixture_hash,
            process_hash,
            deployment_hash,
            identity_relation,
            gaps,
            sufficient,
            _TRUSTED,
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": "study_window_receipt_v1",
            "schema_version": 1,
            "receipt_id": self.receipt_id,
            "window_id": self.window_id,
            "campaign_id": self.campaign_id,
            "study_id": self.study_id,
            "plan_id": self.plan_id,
            "role": self.role,
            "comparison_kind": self.comparison_kind,
            "cohort": self.cohort,
            "sample_count": self.sample_count,
            "qualifying_sample_count": self.qualifying_sample_count,
            "sample_set_sha256": self.sample_set_sha256,
            "scalar_fixture_ref": self.scalar_fixture_ref,
            "scalar_fixture_sha256": self.scalar_fixture_sha256,
            "process_identity_sha256": self.process_identity_sha256,
            "deployment_identity_sha256": self.deployment_identity_sha256,
            "identity_relation": self.identity_relation,
            "gap_refs": list(self.gap_refs),
            "sufficient": self.sufficient,
            "felt_result_established": False,
            "causation_established": False,
            "closure_propagated": False,
            "raw_prose_included": False,
            "artifact_authority_state_v1": authority_state(),
        }

    @classmethod
    def from_untrusted(cls, value: Any) -> StudyWindowReceiptV1:
        if not isinstance(value, dict):
            raise RecordValidationError("study window receipt must be an object")
        validate_evidence_record(value)
        built = cls.build(
            window_id=value.get("window_id"),
            campaign_id=value.get("campaign_id"),
            study_id=value.get("study_id"),
            plan_id=value.get("plan_id"),
            role=value.get("role"),
            comparison_kind=value.get("comparison_kind"),
            cohort=value.get("cohort"),
            sample_count=value.get("sample_count"),
            qualifying_sample_count=value.get("qualifying_sample_count"),
            sample_set_sha256=value.get("sample_set_sha256"),
            scalar_fixture_ref=value.get("scalar_fixture_ref"),
            scalar_fixture_sha256=value.get("scalar_fixture_sha256"),
            process_identity_sha256=value.get("process_identity_sha256"),
            deployment_identity_sha256=value.get("deployment_identity_sha256"),
            identity_relation=value.get("identity_relation"),
            gap_refs=value.get("gap_refs"),
            sufficient=value.get("sufficient"),
        )
        if value.get("receipt_id") != built.receipt_id:
            raise RecordValidationError("window receipt identity mismatch")
        if any(
            value.get(field_name) is not False
            for field_name in (
                "felt_result_established",
                "causation_established",
                "closure_propagated",
                "raw_prose_included",
            )
        ):
            raise RecordValidationError("window receipt epistemic boundary mismatch")
        return built


@dataclass(frozen=True)
class MechanicalComparisonReceiptV1:
    comparison_id: str
    campaign_id: str
    study_id: str
    plan_id: str
    comparison_kind: str
    baseline_receipt_id: str
    candidate_receipt_id: str
    outcome: str
    metric_summary: dict[str, float]
    comparison_sha256: str
    _token: object = field(repr=False, compare=False)

    def __post_init__(self) -> None:
        if self._token is not _TRUSTED:
            raise RecordValidationError("comparisons require the internal builder")

    @classmethod
    def build(
        cls,
        *,
        campaign_id: str,
        study_id: str,
        plan_id: str,
        comparison_kind: str,
        baseline_receipt_id: str,
        candidate_receipt_id: str,
        outcome: str,
        metric_summary: Mapping[str, Any],
    ) -> MechanicalComparisonReceiptV1:
        campaign = validate_bounded_identifier(campaign_id, "campaign_id") or ""
        study = validate_bounded_identifier(study_id, "study_id") or ""
        plan = validate_bounded_identifier(plan_id, "plan_id") or ""
        comparison = ComparisonKindV1(comparison_kind).value
        baseline = (
            validate_bounded_identifier(baseline_receipt_id, "baseline_receipt_id")
            or ""
        )
        candidate = (
            validate_bounded_identifier(candidate_receipt_id, "candidate_receipt_id")
            or ""
        )
        if baseline == candidate:
            raise RecordValidationError("comparison requires distinct cohort receipts")
        resolved_outcome = MechanicalOutcomeV1(outcome).value
        summary = _bounded_scalars(metric_summary, "metric_summary")
        core = {
            "campaign_id": campaign,
            "study_id": study,
            "plan_id": plan,
            "comparison_kind": comparison,
            "baseline_receipt_id": baseline,
            "candidate_receipt_id": candidate,
            "outcome": resolved_outcome,
            "metric_summary": summary,
        }
        digest = _record_hash(core)
        return cls(
            deterministic_id("mechanicalcomparison", {"sha256": digest}),
            campaign,
            study,
            plan,
            comparison,
            baseline,
            candidate,
            resolved_outcome,
            summary,
            digest,
            _TRUSTED,
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": "mechanical_comparison_receipt_v1",
            "schema_version": 1,
            "comparison_id": self.comparison_id,
            "campaign_id": self.campaign_id,
            "study_id": self.study_id,
            "plan_id": self.plan_id,
            "comparison_kind": self.comparison_kind,
            "baseline_receipt_id": self.baseline_receipt_id,
            "candidate_receipt_id": self.candidate_receipt_id,
            "outcome": self.outcome,
            "metric_summary": self.metric_summary,
            "comparison_sha256": self.comparison_sha256,
            "felt_outcome": None,
            "felt_outcome_inferred": False,
            "causation_established": False,
            "closure_propagated": False,
            "raw_prose_included": False,
            "artifact_authority_state_v1": authority_state(),
        }

    @classmethod
    def from_untrusted(cls, value: Any) -> MechanicalComparisonReceiptV1:
        if not isinstance(value, dict):
            raise RecordValidationError("comparison receipt must be an object")
        validate_evidence_record(value)
        built = cls.build(
            campaign_id=value.get("campaign_id"),
            study_id=value.get("study_id"),
            plan_id=value.get("plan_id"),
            comparison_kind=value.get("comparison_kind"),
            baseline_receipt_id=value.get("baseline_receipt_id"),
            candidate_receipt_id=value.get("candidate_receipt_id"),
            outcome=value.get("outcome"),
            metric_summary=value.get("metric_summary"),
        )
        if (
            value.get("comparison_id") != built.comparison_id
            or value.get("comparison_sha256") != built.comparison_sha256
            or value.get("felt_outcome") is not None
        ):
            raise RecordValidationError("mechanical comparison identity mismatch")
        if any(
            value.get(field_name) is not False
            for field_name in (
                "felt_outcome_inferred",
                "causation_established",
                "closure_propagated",
                "raw_prose_included",
            )
        ):
            raise RecordValidationError("mechanical comparison overclaims its scope")
        return built
