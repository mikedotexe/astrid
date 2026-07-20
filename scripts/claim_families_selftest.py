"""Focused unit tests for claim-family matching and delta semantics."""

from __future__ import annotations

import json
from pathlib import Path
import tempfile
import unittest

from claim_family_matcher import polarity, requested_outcome


def run() -> int:
    from claim_families import (
        ClaimRecord,
        addressing_status_path,
        family_claims,
        incremental_events,
        load_claims,
        migration_events,
        project,
    )

    class ClaimFamilyTests(unittest.TestCase):
        def claim(
            self,
            claim_id: str,
            summary: str,
            target: str = "astrid_codec",
        ) -> ClaimRecord:
            return ClaimRecord(
                canonical_claim_id=claim_id,
                introspection_id=claim_id.split(":")[0],
                claim_id=claim_id.split(":")[-1],
                summary=summary,
                authority_class="evidence_only_non_live",
                target_surface=target,
                requested_outcome=requested_outcome(summary),
                polarity=polarity(summary),
                record={"canonical_claim_id": claim_id, "text": summary},
            )

        def test_strict_match_joins_duplicates_and_keeps_singleton(self) -> None:
            claims = [
                self.claim("a:c001", "Preserve exact sensory JSON compatibility."),
                self.claim("b:c001", "Preserve exact sensory JSON compatibility."),
                self.claim(
                    "c:c001",
                    "Preserve sensory compatibility where practical.",
                ),
            ]
            families, suggestions = family_claims(claims)
            self.assertEqual(
                sorted(family["member_count"] for family in families),
                [1, 2],
            )
            assigned = [
                member
                for family in families
                for member in family["member_claim_ids"]
            ]
            self.assertEqual(
                sorted(assigned),
                sorted(claim.canonical_claim_id for claim in claims),
            )
            self.assertTrue(
                all(
                    family["membership_propagates_closure"] is False
                    for family in families
                )
            )
            self.assertIsInstance(suggestions, dict)

        def test_authority_or_target_mismatch_never_auto_joins(self) -> None:
            left = self.claim(
                "a:c001",
                "Preserve exact sensory JSON compatibility.",
            )
            right = self.claim(
                "b:c001",
                "Preserve exact sensory JSON compatibility.",
                target="minime_regulator",
            )
            families, _ = family_claims([left, right])
            self.assertEqual(len(families), 2)

        def test_incremental_baseline_is_stable(self) -> None:
            claims = [
                self.claim(
                    "a:c001",
                    "Preserve exact sensory JSON compatibility.",
                )
            ]
            families, suggestions = family_claims(claims)
            events = migration_events(claims, families, suggestions, "source")
            migration = incremental_events(claims, events)
            self.assertEqual(
                [event["event_type"] for event in migration],
                ["claim_family_v3_migration_completed"],
            )
            self.assertEqual(
                incremental_events(claims, [*events, *migration]),
                [],
            )

        def test_one_new_claim_produces_bounded_delta(self) -> None:
            old = self.claim(
                "a:c001",
                "Preserve exact sensory JSON compatibility.",
            )
            new = self.claim(
                "b:c001",
                "Preserve exact sensory JSON compatibility.",
            )
            families, suggestions = family_claims([old])
            baseline = migration_events([old], families, suggestions, "source")
            migration = incremental_events([old], baseline)
            events = incremental_events([old, new], [*baseline, *migration])
            self.assertEqual(
                [event["event_type"] for event in events],
                [
                    "claim_family_membership_assigned",
                    "claim_family_delta_projected",
                ],
            )
            assignment = events[0]
            self.assertEqual(assignment["family_id"], families[0]["family_id"])
            self.assertFalse(assignment["closure_propagated"])
            self.assertFalse(assignment["evidence_sufficiency_propagated"])
            self.assertFalse(assignment["authority_propagated"])

        def test_claim_content_change_never_moves_membership(self) -> None:
            old = self.claim("a:c001", "Preserve exact output.")
            old = ClaimRecord(
                **{
                    **old.__dict__,
                    "record": {
                        **old.record,
                        "canonical_claim_record_sha256": "old",
                    },
                }
            )
            families, suggestions = family_claims([old])
            baseline = migration_events([old], families, suggestions, "source")
            migration = incremental_events([old], baseline)
            changed = ClaimRecord(
                **{
                    **old.__dict__,
                    "summary": "Preserve exact output and metadata.",
                    "record": {
                        **old.record,
                        "text": "Preserve exact output and metadata.",
                        "canonical_claim_record_sha256": "new",
                    },
                }
            )
            events = incremental_events([changed], [*baseline, *migration])
            self.assertEqual(events[0]["event_type"], "claim_content_changed")
            self.assertFalse(events[0]["membership_changed"])
            self.assertNotIn(
                "claim_family_membership_assigned",
                [event["event_type"] for event in events],
            )

        def test_review_budget_and_immediate_objection(self) -> None:
            events = [
                {
                    "event_type": "claim_family_created",
                    "family": {"family_id": "family_one"},
                },
                {
                    "event_type": "claim_family_membership_assigned",
                    "family_id": "family_one",
                    "canonical_claim_id": "a:c001",
                    "claim": {"text": "claim"},
                },
                {
                    "event_type": "claim_family_migration_completed",
                    "migration_receipt": {"family_ids": ["family_one"]},
                },
                {
                    "event_type": "felt_review_response_recorded",
                    "family_id": "family_one",
                    "classification": "objection",
                    "immediate_surface": True,
                },
            ]
            status = project(events, "receipt_one")
            self.assertTrue(
                status["felt_review_budget_v1"]["family_one"][
                    "packet_available"
                ]
            )
            self.assertTrue(
                status["felt_review_responses"][0]["immediate_surface"]
            )

        def test_membership_correction_preserves_boundaries(self) -> None:
            events = [
                {
                    "event_type": "claim_family_created",
                    "family": {"family_id": "family_one"},
                },
                {
                    "event_type": "claim_family_created",
                    "family": {"family_id": "family_two"},
                },
                {
                    "event_type": "claim_family_membership_assigned",
                    "family_id": "family_one",
                    "canonical_claim_id": "a:c001",
                    "claim": {"text": "claim"},
                },
                {
                    "event_type": "claim_family_membership_corrected",
                    "family_id": "family_two",
                    "from_family_id": "family_one",
                    "to_family_id": "family_two",
                    "canonical_claim_id": "a:c001",
                    "claim": {"text": "claim"},
                    "closure_propagated": False,
                },
            ]
            status = project(events, None)
            self.assertNotIn(
                "a:c001",
                status["families"].get("family_one", {}).get("claims", {}),
            )
            self.assertIn("a:c001", status["families"]["family_two"]["claims"])
            self.assertFalse(
                status["membership_history"][0]["closure_propagated"]
            )

        def test_realistic_status_preserves_claim_identity(self) -> None:
            with tempfile.TemporaryDirectory() as directory:
                workspace = Path(directory)
                path = addressing_status_path(workspace)
                path.parent.mkdir(parents=True)
                path.write_text(
                    json.dumps(
                        {
                            "next_queue": [
                                {"introspection_id": "intro_one"}
                            ],
                            "artifacts": {
                                "intro_one": {
                                    "introspection_id": "intro_one",
                                    "source_family": "astrid_codec",
                                    "status": "read",
                                    "fully_addressed": False,
                                    "sha256": "source",
                                    "claims": {
                                        "c001": {
                                            "claim_id": "c001",
                                            "summary": "Preserve exact output.",
                                            "disposition": "verified",
                                            "evidence": [{"target": "test"}],
                                        }
                                    },
                                }
                            },
                        }
                    ),
                    encoding="utf-8",
                )
                claims, _ = load_claims(workspace)
                self.assertEqual(
                    claims[0].canonical_claim_id,
                    "intro_one:c001",
                )
                self.assertEqual(claims[0].record["claim_id"], "c001")
                self.assertEqual(claims[0].record["queue_position"], 0)
                self.assertEqual(
                    claims[0].record["evidence_links"],
                    [{"kind": None, "target": "test", "ts": None}],
                )

    suite = unittest.defaultTestLoader.loadTestsFromTestCase(ClaimFamilyTests)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1
