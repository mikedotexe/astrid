#!/usr/bin/env python3
"""Canonical non-live authority state for generated evidence artifacts."""

from __future__ import annotations

import argparse
import copy
import unittest
from dataclasses import dataclass
from typing import Any

ARTIFACT_STATE_KEY = "artifact_authority_state_v1"
PROJECTION_KEY = "authority_projection_v2"
LEGACY_AUTHORITY_MARKERS = (
    "live_eligible_now",
    "auto_approved",
    "grants_approval",
    "edits_source_now",
)
ALLOWED_ARTIFACT_STATES = frozenset({"evidence_only", "approval_pending"})


class ArtifactAuthorityStateError(ValueError):
    """Raised when evidence tooling attempts to emit live authority."""


@dataclass(frozen=True)
class ArtifactAuthorityStateV1:
    """A state evidence tooling may persist without granting authority."""

    state: str

    def __post_init__(self) -> None:
        if self.state not in ALLOWED_ARTIFACT_STATES:
            raise ArtifactAuthorityStateError(
                f"artifact authority state must be one of {sorted(ALLOWED_ARTIFACT_STATES)}; "
                f"got {self.state!r}"
            )

    @classmethod
    def evidence_only(cls) -> ArtifactAuthorityStateV1:
        return cls("evidence_only")

    @classmethod
    def approval_pending(cls) -> ArtifactAuthorityStateV1:
        return cls("approval_pending")

    def canonical_record(self) -> dict[str, Any]:
        return {
            "schema": "artifact_authority_state_v1",
            "schema_version": 1,
            "state": self.state,
            "witness_only": True,
        }

    def legacy_projection_v2(self) -> dict[str, Any]:
        return {
            "schema": "artifact_authority_projection_v2",
            "schema_version": 2,
            "source_state": self.state,
            "live_eligible_now": False,
            "auto_approved": False,
            "grants_approval": False,
            "edits_source_now": False,
        }


def _path_text(path: tuple[str, ...]) -> str:
    return ".".join(path) if path else "<root>"


def assert_artifact_authority_tree(value: Any, *, path: tuple[str, ...] = ()) -> None:
    """Reject forbidden canonical states and true legacy authority markers."""

    if isinstance(value, list):
        for index, item in enumerate(value):
            assert_artifact_authority_tree(item, path=(*path, str(index)))
        return
    if not isinstance(value, dict):
        return

    canonical = value.get(ARTIFACT_STATE_KEY)
    if canonical is not None:
        if not isinstance(canonical, dict):
            raise ArtifactAuthorityStateError(
                f"{_path_text((*path, ARTIFACT_STATE_KEY))} must be an object"
            )
        ArtifactAuthorityStateV1(str(canonical.get("state") or ""))

    for marker in LEGACY_AUTHORITY_MARKERS:
        if value.get(marker) is True:
            raise ArtifactAuthorityStateError(
                f"{_path_text((*path, marker))}=true is forbidden in evidence tooling"
            )

    for key, item in value.items():
        if key != ARTIFACT_STATE_KEY:
            assert_artifact_authority_tree(item, path=(*path, str(key)))


def _inferred_state(value: dict[str, Any], fallback: str) -> ArtifactAuthorityStateV1:
    canonical = value.get(ARTIFACT_STATE_KEY)
    if isinstance(canonical, dict) and canonical.get("state"):
        return ArtifactAuthorityStateV1(str(canonical["state"]))

    try:
        tier = int(value.get("agency_tier") or 0)
    except (TypeError, ValueError):
        tier = 0
    approval_wait = (
        tier >= 4
        or value.get("blocked_by") in {"steward_grant", "operator_approval"}
        or bool(value.get("authority_boundary_id"))
        or bool(value.get("authority_boundary_ids"))
        or str(value.get("authority_class") or "")
        in {"steward_gated_consequence", "mike_operator_live_substrate"}
        or str(value.get("gate_state") or "")
        in {"proposal_needed", "approval_pending", "operator_approval_wait"}
        or str(value.get("lifecycle_state") or "")
        in {
            "proposal_needed",
            "replay_needed",
            "operator_approval_wait",
            "authority_boundary_wait",
            "approved_manual_only",
        }
    )
    return (
        ArtifactAuthorityStateV1.approval_pending()
        if approval_wait
        else ArtifactAuthorityStateV1(fallback)
    )


def apply_artifact_authority_state(
    payload: dict[str, Any],
    state: ArtifactAuthorityStateV1 | str,
) -> dict[str, Any]:
    """Stamp one artifact and derive every legacy compatibility marker."""

    assert_artifact_authority_tree(payload)
    resolved = state if isinstance(state, ArtifactAuthorityStateV1) else ArtifactAuthorityStateV1(state)
    projection = resolved.legacy_projection_v2()
    payload[ARTIFACT_STATE_KEY] = resolved.canonical_record()
    payload[PROJECTION_KEY] = projection
    for marker in LEGACY_AUTHORITY_MARKERS:
        payload[marker] = projection[marker]
    return payload


def normalize_artifact_authority_tree(
    value: Any,
    *,
    default_state: str = "evidence_only",
) -> Any:
    """Validate then centrally project authority state throughout an artifact tree."""

    assert_artifact_authority_tree(value)

    def normalize(item: Any) -> None:
        if isinstance(item, list):
            for child in item:
                normalize(child)
            return
        if not isinstance(item, dict):
            return
        for key, child in list(item.items()):
            if key not in {ARTIFACT_STATE_KEY, PROJECTION_KEY}:
                normalize(child)
        if ARTIFACT_STATE_KEY in item or any(marker in item for marker in LEGACY_AUTHORITY_MARKERS):
            apply_artifact_authority_state(item, _inferred_state(item, default_state))

    normalize(value)
    if isinstance(value, dict) and ARTIFACT_STATE_KEY not in value:
        apply_artifact_authority_state(value, _inferred_state(value, default_state))
    assert_artifact_authority_tree(value)
    return value


class _SelfTests(unittest.TestCase):
    def test_states_project_all_legacy_markers_false(self) -> None:
        payload = {"live_eligible_now": False}
        apply_artifact_authority_state(payload, "approval_pending")
        self.assertEqual(payload[ARTIFACT_STATE_KEY]["state"], "approval_pending")
        self.assertTrue(all(payload[marker] is False for marker in LEGACY_AUTHORITY_MARKERS))

    def test_true_marker_is_rejected_before_normalization(self) -> None:
        with self.assertRaises(ArtifactAuthorityStateError):
            normalize_artifact_authority_tree({"nested": {"grants_approval": True}})

    def test_forbidden_canonical_state_is_rejected(self) -> None:
        with self.assertRaises(ArtifactAuthorityStateError):
            ArtifactAuthorityStateV1("authority_granted")

    def test_normalization_does_not_mutate_caller_on_copy(self) -> None:
        source = {"auto_approved": False}
        normalized = normalize_artifact_authority_tree(copy.deepcopy(source))
        self.assertNotIn(ARTIFACT_STATE_KEY, source)
        self.assertEqual(normalized[ARTIFACT_STATE_KEY]["state"], "evidence_only")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(_SelfTests)
        return 0 if unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful() else 1
    parser.print_help()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
