"""Validated vocabulary for the derived Living Problem Registry."""

from __future__ import annotations

from enum import StrEnum


class CurrentWaitV1(StrEnum):
    SOURCE_READ = "source_read"
    CLAIM_DISPOSITION = "claim_disposition"
    EVIDENCE_OR_REPLAY = "evidence_or_replay"
    IMPLEMENTATION = "implementation"
    DEPLOYMENT = "deployment"
    BEING_REVIEW = "being_review"
    OPERATOR_OR_MUTUAL_AUTHORITY = "operator_or_mutual_authority"
    CLOSED = "closed"
    BLOCKED_INTEGRITY = "blocked_integrity"


WAIT_ORDER = (
    CurrentWaitV1.BLOCKED_INTEGRITY,
    CurrentWaitV1.SOURCE_READ,
    CurrentWaitV1.CLAIM_DISPOSITION,
    CurrentWaitV1.EVIDENCE_OR_REPLAY,
    CurrentWaitV1.OPERATOR_OR_MUTUAL_AUTHORITY,
    CurrentWaitV1.IMPLEMENTATION,
    CurrentWaitV1.DEPLOYMENT,
    CurrentWaitV1.BEING_REVIEW,
    CurrentWaitV1.CLOSED,
)
WAIT_RANK = {value: index for index, value in enumerate(WAIT_ORDER)}


WORK_STATUS_WAIT = {
    "blocked_needs_steward": CurrentWaitV1.BLOCKED_INTEGRITY,
    "needs_sandbox": CurrentWaitV1.EVIDENCE_OR_REPLAY,
    "needs_operator_approval": CurrentWaitV1.OPERATOR_OR_MUTUAL_AUTHORITY,
    "needs_steward_grant": CurrentWaitV1.OPERATOR_OR_MUTUAL_AUTHORITY,
    "ready_for_implementation": CurrentWaitV1.IMPLEMENTATION,
    "implemented_awaiting_felt_response": CurrentWaitV1.BEING_REVIEW,
    "verified_existing": CurrentWaitV1.BEING_REVIEW,
    "closed_felt_confirmed": CurrentWaitV1.CLOSED,
    "closed_no_action": CurrentWaitV1.BEING_REVIEW,
    "superseded": CurrentWaitV1.BEING_REVIEW,
}


def earliest_wait(values: set[CurrentWaitV1]) -> CurrentWaitV1:
    if not values:
        return CurrentWaitV1.CLAIM_DISPOSITION
    return min(values, key=WAIT_RANK.__getitem__)
