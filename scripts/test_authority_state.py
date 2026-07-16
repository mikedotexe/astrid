#!/usr/bin/env python3
"""Focused tests for the artifact authority-state boundary."""

from __future__ import annotations

import unittest

try:
    from authority_state import (
        ARTIFACT_STATE_KEY,
        ArtifactAuthorityStateError,
        apply_artifact_authority_state,
        normalize_artifact_authority_tree,
    )
except ModuleNotFoundError:
    from scripts.authority_state import (
        ARTIFACT_STATE_KEY,
        ArtifactAuthorityStateError,
        apply_artifact_authority_state,
        normalize_artifact_authority_tree,
    )


class ArtifactAuthorityStateTests(unittest.TestCase):
    def test_tier_five_artifact_infers_approval_pending(self) -> None:
        payload = {"agency_tier": 5, "auto_approved": False}
        normalize_artifact_authority_tree(payload)
        self.assertEqual(payload[ARTIFACT_STATE_KEY]["state"], "approval_pending")

    def test_evidence_artifact_projects_schema_v2_compatibility(self) -> None:
        payload: dict[str, object] = {}
        apply_artifact_authority_state(payload, "evidence_only")
        self.assertEqual(payload["authority_projection_v2"]["schema_version"], 2)
        self.assertFalse(payload["live_eligible_now"])
        self.assertFalse(payload["auto_approved"])
        self.assertFalse(payload["grants_approval"])
        self.assertFalse(payload["edits_source_now"])

    def test_nested_true_marker_is_rejected(self) -> None:
        with self.assertRaises(ArtifactAuthorityStateError):
            normalize_artifact_authority_tree(
                {"packet": {"artifact_authority_state_v1": {"state": "evidence_only"}, "edits_source_now": True}}
            )

    def test_live_executable_canonical_state_is_rejected(self) -> None:
        with self.assertRaises(ArtifactAuthorityStateError):
            normalize_artifact_authority_tree(
                {"artifact_authority_state_v1": {"state": "live_executable"}}
            )

    def test_marker_free_top_level_is_stamped(self) -> None:
        payload = {"schema": "status_v1", "summary": {"count": 1}}
        normalize_artifact_authority_tree(payload)
        self.assertEqual(payload[ARTIFACT_STATE_KEY]["state"], "evidence_only")

    def test_corrupt_projection_true_marker_is_rejected(self) -> None:
        with self.assertRaises(ArtifactAuthorityStateError):
            normalize_artifact_authority_tree(
                {"authority_projection_v2": {"grants_approval": True}}
            )


if __name__ == "__main__":
    unittest.main()
