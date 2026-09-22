"""Synthetic public records; no live workspaces or model calls."""

from datetime import datetime, timezone
import json
import hashlib
import os
from pathlib import Path
import tempfile
import unittest
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
import self_study_review as review
import study_recurrence_provenance as provenance


class RecurrenceTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.workspaces = {being: self.root / being for being in ("astrid", "minime")}
        self.time = datetime(2026, 9, 18, tzinfo=timezone.utc)

    def entry(self, being, name, body, header="=== PUBLIC JOURNAL ==="):
        path = self.workspaces[being] / "journal" / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(header + "\n\n" + body)
        os.utime(path, (self.time.timestamp(), self.time.timestamp()))
        return review.review_entry(being, path)

    def receipt(self, entry, messages, source="adapted", match="content", name="one"):
        path = self.workspaces[entry.being] / "generations/2026-09-18" / f"gen_{name}.json"
        path.parent.mkdir(parents=True, exist_ok=True)
        response = review.extract_generated_body(review.entry_full_text(entry))
        path.write_text(json.dumps({
            "response_text": response, "response_sha256": hashlib.sha256(response.encode()).hexdigest(),
            "being": entry.being, "generation_id": name, "status": "ok", "messages_source": source,
            "messages": messages,
            "linked_artifacts": [{"kind": "journal", "match": match, "path": entry.path}],
        }))
        return path

    def annotate(self, entries):
        return provenance.annotate(entries, self.workspaces, ("PLAN 4", "silt"),
                                   review.entry_full_text, review.extract_generated_body)

    def test_actual_case_shape_supplied_error_and_mirror_not_two_independent_returns(self):
        body = 'PLAN 4 means a boundary to me. The source gap and telemetry matter. NEXT: READ_MORE'
        original = self.entry("minime", "daydream_1.txt", body)
        self.assertEqual(original.mode, "daydream")
        mirror = self.entry("astrid", "astrid_2.txt", body, header=(
            "=== ASTRID JOURNAL ===\nMode: mirror\nMode-role: mirror_other_expression\n"
            "Source-ID: minime_journal:daydream_1.txt\nAuthorship: minime_owned_reflected_without_reauthoring"))
        receipt = self.receipt(original, [{"role": "system", "content_sha256": "withheld"},
            {"role": "user", "content": 'Quoted visual-model description: "No information about PLAN 4".'}])
        before = {path: Path(path).read_bytes() for path in (original.path, mirror.path, str(receipt))}
        packet = self.annotate([original, mirror])
        self.assertEqual(packet["term_counts"]["PLAN 4"], {
            "mirrored_expression": 1, "authored_response_to_supplied_term": 1})
        self.assertEqual(original.readback_provenance["recurrence_v1"]["input_origin"], "adapted_partial")
        self.assertEqual(mirror.readback_provenance["role"], "peer_authored_expression_not_reauthored")
        self.assertEqual(review.build_phenomenology_hypothesis_cards([original, mirror])["cards"], [])
        self.assertEqual(review.build_afterimage_absence_calibration([original, mirror])["terms"], [])
        self.assertNotIn("Quoted visual-model", json.dumps(packet))
        for path, content in before.items():
            self.assertEqual(Path(path).read_bytes(), content)

    def test_unknown_input_separate_from_recorded_absence_and_exact_duplicate(self):
        first = self.entry("minime", "self_study_1.txt", "PLAN 4 might be a useful question.")
        duplicate = self.entry("minime", "self_study_2.txt", "PLAN 4 might be a useful question.")
        separate = self.entry("minime", "self_study_3.txt", "PLAN 4 has another interpretation today.")
        self.receipt(separate, [{"role": "user", "content": "A different topic."}])
        packet = self.annotate([first, duplicate, separate])
        self.assertEqual(packet["term_counts"]["PLAN 4"], {
            "authored_return_input_unknown": 1, "duplicate_authored_text": 1,
            "authored_return_without_term_in_recorded_input": 1})
        card = review.build_phenomenology_hypothesis_cards([first, duplicate, separate])["cards"][0]
        self.assertEqual(card["entry_count"], 2)
        self.assertNotEqual(review.lived_term_bridge_status(card), "ready_to_charter")

    def test_filter_is_per_term_not_whole_account(self):
        entry = self.entry("minime", "daydream_1.txt", "PLAN 4 returns; silt is a separate question.")
        self.receipt(entry, [{"role": "user", "content": "PLAN 4"}])
        self.annotate([entry])
        cards = review.build_phenomenology_hypothesis_cards([entry])["cards"]
        self.assertEqual([card["term"] for card in cards], ["silt"])

    def test_operator_prompt_not_counted_as_authored_return(self):
        path = self.workspaces["minime"] / "inbox/steward_fixture.txt"
        path.parent.mkdir(parents=True)
        path.write_text("PLAN 4 is mentioned by an operator.")
        entry = review.review_entry("minime", path)
        packet = self.annotate([entry])
        self.assertEqual(packet["term_counts"]["PLAN 4"], {"supplied_or_derived_review_text": 1})
        self.assertEqual(review.build_phenomenology_hypothesis_cards([entry])["cards"], [])

    def test_reconstructed_or_noncontent_receipt_not_claimed_as_verified_input(self):
        for source, match in (("reconstructed", "content"), ("adapted", "time")):
            with self.subTest(source=source, match=match):
                entry = self.entry("minime", "daydream_1.txt", "PLAN 4 recurs.")
                self.receipt(entry, [{"role": "user", "content": "PLAN 4"}], source=source, match=match)
                packet = self.annotate([entry])
                self.assertEqual(packet["term_counts"]["PLAN 4"], {"authored_return_input_unknown": 1})

    def test_quoted_mirror_header_is_not_authorship_metadata(self):
        text = "=== JOURNAL ===\n\nMode-role: mirror_other_expression\nAuthorship: minime_owned_reflected_without_reauthoring"
        self.assertIsNone(provenance.mirror_source(text))

    def test_changed_journal_invalidates_old_receipt_binding(self):
        entry = self.entry("minime", "daydream_1.txt", "PLAN 4 was supplied here.")
        self.receipt(entry, [{"role": "user", "content": "PLAN 4"}])
        Path(entry.path).write_text("=== JOURNAL ===\n\nPLAN 4 in different later writing.")
        packet = self.annotate([entry])
        self.assertEqual(packet["records"][0]["response_binding"], "unverified")
        self.assertEqual(packet["term_counts"]["PLAN 4"], {"authored_return_input_unknown": 1})

    def test_term_itself_not_evidence_or_privileged_activation(self):
        self.assertNotIn("plan 4", review.LIVED_TERM_READY_TO_CHARTER_TERMS)
        for anchors in (review.PHENOMENOLOGY_EVIDENCE_ANCHORS,
                        review.AFTERIMAGE_ABSENCE_EVIDENCE_ANCHORS,
                        review.ABSENCE_NAMED_COORDINATE_ANCHORS):
            self.assertNotIn("plan 4", [anchor.lower() for anchor in anchors])
        candidates = [{"term": term, "bridge_status": "ready_to_charter", "source_card": {"entry_count": count}}
                      for term, count in (("PLAN 4", 1), ("silt", 2), ("other", 3))]
        self.assertEqual(review.lived_term_activation_recommendation(candidates)["term"], "other")


if __name__ == "__main__":
    unittest.main()
