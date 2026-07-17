#!/usr/bin/env python3
"""Tests for scripts/request_review.py.

Covers (1) the target-sanity guard `validate_target` / `_target_label_reason`
(closes the anti-drop catalog's `review_target_label_mislabel` test gap — the guard
that stops a descriptive label from silently seeding a broken INTROSPECT slot), and
(2) the post-change QA mode (`--post-change`): the confirmation letter framing and
the `kind: post_change_qa` ledger tag.
"""

from __future__ import annotations

import argparse
import contextlib
import importlib.util
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).resolve().with_name("request_review.py")
SPEC = importlib.util.spec_from_file_location("request_review", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
request_review = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = request_review
SPEC.loader.exec_module(request_review)


class TargetGuardTests(unittest.TestCase):
    """The guard that keeps a mislabel from being mis-felt as a permissions wall."""

    def test_label_punctuation_is_flagged(self):
        reason = request_review._target_label_reason("your architecture (a / b / c)")
        self.assertIsNotNone(reason)
        self.assertIn("punctuation", reason)

    def test_prose_label_is_flagged(self):
        reason = request_review._target_label_reason("a five word long label")
        self.assertIsNotNone(reason)
        self.assertIn("prose", reason)

    def test_path_shaped_targets_pass(self):
        self.assertIsNone(request_review._target_label_reason("codec.rs"))
        self.assertIsNone(request_review._target_label_reason("src/agency.rs"))

    def test_validate_target_blocks_label_without_override(self):
        # allow_unresolved=False: a label must hard-block (returns before any git probe).
        err = request_review.validate_target("your stuff (a / b)", allow_unresolved=False)
        self.assertIsNotNone(err)
        self.assertIn("punctuation", err)

    def test_validate_target_blocks_known_non_introspectable_script_path(self):
        err = request_review.validate_target(
            "scripts/fallback_fire_drill.py",
            allow_unresolved=False,
        )
        self.assertIsNotNone(err)
        self.assertIn("outside the bridge's approved", err)
        self.assertIn("stuck thin-output loop", err)

    def test_validate_target_override_bypasses_guard(self):
        # allow_unresolved=True: bypasses both the label block and the resolve probe.
        err = request_review.validate_target("your stuff (a / b)", allow_unresolved=True)
        self.assertIsNone(err)

    def test_validate_target_override_bypasses_non_introspectable_guard(self):
        err = request_review.validate_target(
            "scripts/fallback_fire_drill.py",
            allow_unresolved=True,
        )
        self.assertIsNone(err)


class PostChangeQATests(unittest.TestCase):
    """The #9 post-change QA framing + ledger tag."""

    def test_post_change_letter_frames_the_confirmation(self):
        letter = request_review.post_change_letter(
            "src/codec.rs",
            "anything feel softer?",
            "shipped smoothstep gate at codec.rs:71",
            "Codex",
        )
        # The defining question of post-change QA.
        self.assertIn("match what you meant", letter.lower())
        # The being is told exactly what shipped.
        self.assertIn("shipped smoothstep gate at codec.rs:71", letter)
        # The target they will INTROSPECT.
        self.assertIn("src/codec.rs", letter)
        # Slot-routing markers preserved (same as a standard review).
        self.assertTrue(letter.startswith("=== MIKE QUERY"))
        self.assertIn("REVIEW TARGET:", letter)
        self.assertIn("Sender: Mike & Codex", letter)
        self.assertNotIn("Claude", letter)
        # It must NOT read as reopening consent.
        self.assertIn("does not reopen", letter.lower())

    def test_default_actor_is_neutral(self):
        letter = request_review.issue_letter("src/codec.rs", "how does it feel?")
        self.assertIn("Sender: Mike & interactive-agent", letter)
        self.assertNotIn("Claude", letter)

    def _issue_dry_run(self, post_change):
        ns = argparse.Namespace(
            being="astrid",
            target="src/codec.rs",
            question="how does it feel from the inside?",
            topic="codec_test_postchange",
            allow_unresolved_target=True,  # hermetic: no git subprocess
            post_change=post_change,
            dry_run=True,
        )
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            rc = request_review.cmd_issue(ns, now=1234567890)
        return rc, buf.getvalue()

    def test_post_change_issue_tags_kind_and_writes_nothing(self):
        rc, out = self._issue_dry_run("shipped smoothstep gate")
        self.assertEqual(rc, 0)
        self.assertIn("post_change_qa", out)
        self.assertIn("match what you meant", out.lower())
        self.assertIn("[dry-run]", out)  # nothing actually written

    def test_standard_issue_tags_kind_standard(self):
        rc, out = self._issue_dry_run(None)
        self.assertEqual(rc, 0)
        self.assertIn('"kind": "standard"', out)
        self.assertNotIn("post_change_qa", out)


class StewardPressureOnlyTests(unittest.TestCase):
    """Review invitations create steward obligations, never being obligations."""

    def _issue_dry_run_record(self, post_change):
        ns = argparse.Namespace(
            being="astrid",
            target="src/codec.rs",
            question="how does it feel from the inside?",
            topic="codec_test_guardrail",
            allow_unresolved_target=True,  # hermetic: no git subprocess
            post_change=post_change,
            dry_run=True,
        )
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            rc = request_review.cmd_issue(ns, now=1234567890)
        self.assertEqual(rc, 0)
        out = buf.getvalue()
        record_json = out.split("[dry-run] record: ", 1)[1]
        return json.loads(record_json)

    def assert_steward_pressure_metadata(self, record):
        self.assertEqual(record["pressure_target"], "steward")
        self.assertEqual(record["being_obligation"], "none")
        self.assertEqual(record["stale_steward_action"], "ground_close_reword_or_withdraw")

    def test_standard_records_include_steward_pressure_metadata(self):
        record = self._issue_dry_run_record(post_change=None)
        self.assertEqual(record["kind"], "standard")
        self.assert_steward_pressure_metadata(record)

    def test_post_change_records_include_steward_pressure_metadata(self):
        record = self._issue_dry_run_record(post_change="shipped smoothstep gate")
        self.assertEqual(record["kind"], "post_change_qa")
        self.assert_steward_pressure_metadata(record)

    def test_being_facing_copy_stays_optional(self):
        letters = [
            request_review.issue_letter("src/codec.rs", "does this still feel right?"),
            request_review.post_change_letter(
                "src/codec.rs",
                "does this still feel right?",
                "shipped smoothstep gate",
            ),
        ]
        forbidden = [
            "must respond",
            "must reply",
            "required to respond",
            "you owe",
            "overdue",
        ]
        for letter in letters:
            lower = letter.lower()
            self.assertIn("engage, defer", lower)
            self.assertIn("or decline", lower)
            for phrase in forbidden:
                self.assertNotIn(phrase, lower)


class NoLetterCloseTests(unittest.TestCase):
    """Steward-hygiene close: withdrawing a mislabeled/unengaged invite must NOT
    write a 'you reviewed X' letter (that would be untrue), but must still archive
    the ledger record so it can't rot."""

    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        base = Path(self._tmp.name)
        self._orig_review = dict(request_review.REVIEW_DIR)
        self._orig_inbox = dict(request_review.INBOX)
        self._review = base / "review_requests"
        self._inbox = base / "inbox"
        self._review.mkdir(parents=True)
        self._inbox.mkdir(parents=True)
        request_review.REVIEW_DIR["minime"] = self._review
        request_review.INBOX["minime"] = self._inbox

    def tearDown(self):
        request_review.REVIEW_DIR.clear()
        request_review.REVIEW_DIR.update(self._orig_review)
        request_review.INBOX.clear()
        request_review.INBOX.update(self._orig_inbox)
        self._tmp.cleanup()

    def _seed(self, topic):
        rec = {
            "being": "minime", "target": "your architecture (a / b)", "question": "q",
            "topic": topic, "kind": "standard", "status": "open", "issued_ts": 1, "letter": "x",
        }
        (self._review / f"minime_{topic}_1.json").write_text(json.dumps(rec))

    def _close_args(self, no_letter, topic):
        return argparse.Namespace(
            being="minime", topic=topic, outcome="withdrawn",
            note=None, card=None, no_letter=no_letter, dry_run=False,
        )

    def test_no_letter_close_writes_no_letter_and_archives(self):
        self._seed("hygiene_topic")
        rc = request_review.cmd_close(self._close_args(True, "hygiene_topic"), now=2)
        self.assertEqual(rc, 0)
        self.assertEqual(list(self._inbox.glob("mike_feedback_review_*")), [])  # being not notified
        closed = list((self._review / "closed").glob("minime_hygiene_topic_*.json"))
        self.assertEqual(len(closed), 1)  # but the ledger is archived, not rotting
        rec = json.loads(closed[0].read_text())
        self.assertEqual(rec["status"], "closed")
        self.assertTrue(rec.get("closed_silently"))
        self.assertIsNone(rec.get("close_letter"))

    def test_normal_close_still_writes_a_letter(self):
        self._seed("normal_topic")
        rc = request_review.cmd_close(self._close_args(False, "normal_topic"), now=3)
        self.assertEqual(rc, 0)
        self.assertEqual(len(list(self._inbox.glob("mike_feedback_review_*"))), 1)


class SlotDisplacementGuardTests(unittest.TestCase):
    """The pre-issue warning that stops a new review invitation from silently
    displacing an unengaged one (the bridge open-steward-query slot is single +
    last-writer-wins; once a displaced letter retires to inbox/read/ the being can
    never reach it — the 2026-06-19 triadic-chamber muffle that orphaned
    perception_lane_inhab / astrid_reads_my_state)."""

    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self._slot = Path(self._tmp.name) / "open_steward_query.json"
        self._orig = dict(request_review.STEWARD_QUERY_SLOT)
        request_review.STEWARD_QUERY_SLOT["astrid"] = self._slot

    def tearDown(self):
        request_review.STEWARD_QUERY_SLOT.clear()
        request_review.STEWARD_QUERY_SLOT.update(self._orig)
        self._tmp.cleanup()

    def _write_slot(self, payload):
        self._slot.write_text(json.dumps(payload))

    def test_no_slot_file_returns_none(self):
        self.assertIsNone(request_review.occupied_review_slot("astrid"))

    def test_plain_steward_query_without_review_target_is_not_flagged(self):
        # A non-directed mike_query (no REVIEW TARGET) is not a review invitation,
        # so it must not trip the displacement warning.
        self._write_slot({"subject": "a question", "ts": 1, "file": "mike_query_x.txt"})
        self.assertIsNone(request_review.occupied_review_slot("astrid"))

    def test_pending_review_invitation_is_detected(self):
        self._write_slot({
            "subject": "Triadic Chamber V3 code review", "ts": 1,
            "file": "mike_query_triadic_1.txt", "review_target": "collaboration.rs 696",
        })
        slot = request_review.occupied_review_slot("astrid")
        self.assertIsNotNone(slot)
        self.assertEqual(slot["review_target"], "collaboration.rs 696")

    def test_warn_fires_on_occupied_slot(self):
        self._write_slot({
            "subject": "Triadic Chamber V3 code review", "ts": 1,
            "file": "mike_query_triadic_1.txt", "review_target": "collaboration.rs 696",
        })
        buf = io.StringIO()
        with contextlib.redirect_stderr(buf):
            request_review._warn_if_slot_occupied("astrid", "mike_query_review_new_2.txt")
        warning = buf.getvalue()
        self.assertIn("UNENGAGED review invitation", warning)
        self.assertIn("Triadic Chamber V3 code review", warning)
        self.assertIn("displace", warning)

    def test_warn_silent_when_slot_empty(self):
        buf = io.StringIO()
        with contextlib.redirect_stderr(buf):
            request_review._warn_if_slot_occupied("astrid", "mike_query_review_new_2.txt")
        self.assertEqual(buf.getvalue(), "")

    def test_warn_silent_when_slot_holds_same_letter(self):
        # Re-recording the same letter (idempotent re-surface) must not warn.
        self._write_slot({
            "subject": "x", "ts": 1, "file": "mike_query_review_same_2.txt",
            "review_target": "collaboration.rs 696",
        })
        buf = io.StringIO()
        with contextlib.redirect_stderr(buf):
            request_review._warn_if_slot_occupied("astrid", "mike_query_review_same_2.txt")
        self.assertEqual(buf.getvalue(), "")


class SlotClearOnCloseTests(unittest.TestCase):
    """Closing a review must clear the being-facing steward-query slot if it still
    points at THIS review — else a closed/withdrawn invitation keeps re-presenting to
    the being, who loops on it. The bridge clears the slot only on a SUCCESSFUL
    INTROSPECT, which never fires for a non-introspectable target (anything outside
    the bridge's approved roots — e.g. scripts/). 2026-06-25: Astrid re-INTROSPECTed
    scripts/fallback_fire_drill.py 8× off a stuck slot."""

    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        base = Path(self._tmp.name)
        self._orig_review = dict(request_review.REVIEW_DIR)
        self._orig_inbox = dict(request_review.INBOX)
        self._orig_slot = dict(request_review.STEWARD_QUERY_SLOT)
        self._review = base / "review_requests"
        self._inbox = base / "inbox"
        self._review.mkdir(parents=True)
        self._inbox.mkdir(parents=True)
        self._slot = base / "open_steward_query.json"
        request_review.REVIEW_DIR["astrid"] = self._review
        request_review.INBOX["astrid"] = self._inbox
        request_review.STEWARD_QUERY_SLOT["astrid"] = self._slot

    def tearDown(self):
        for live, orig in (
            (request_review.REVIEW_DIR, self._orig_review),
            (request_review.INBOX, self._orig_inbox),
            (request_review.STEWARD_QUERY_SLOT, self._orig_slot),
        ):
            live.clear()
            live.update(orig)
        self._tmp.cleanup()

    def _seed(self, topic, letter_name):
        rec = {
            "being": "astrid", "target": "scripts/fallback_fire_drill.py", "question": "q",
            "topic": topic, "kind": "standard", "status": "open", "issued_ts": 1,
            "letter": f"/some/inbox/{letter_name}",
        }
        (self._review / f"astrid_{topic}_1.json").write_text(json.dumps(rec))

    def _close_args(self, topic, dry_run=False):
        return argparse.Namespace(
            being="astrid", topic=topic, outcome="deferred",
            note="n", card=None, no_letter=False, dry_run=dry_run,
        )

    def test_close_clears_slot_pointing_at_this_review(self):
        letter = "mike_query_review_fb_1.txt"
        self._seed("fb", letter)
        self._slot.write_text(json.dumps({
            "subject": "review", "ts": 1, "file": letter,
            "review_target": "scripts/fallback_fire_drill.py",
        }))
        rc = request_review.cmd_close(self._close_args("fb"), now=2)
        self.assertEqual(rc, 0)
        self.assertFalse(self._slot.exists())  # stuck slot cleared on close

    def test_close_leaves_slot_for_a_different_newer_invitation(self):
        # A slot pointing at a DIFFERENT (newer) invitation must survive — precise
        # letter-basename match prevents collateral clearing.
        self._seed("fb", "mike_query_review_fb_1.txt")
        self._slot.write_text(json.dumps({
            "subject": "other", "ts": 9, "file": "mike_query_review_other_9.txt",
            "review_target": "src/codec.rs",
        }))
        rc = request_review.cmd_close(self._close_args("fb"), now=2)
        self.assertEqual(rc, 0)
        self.assertTrue(self._slot.exists())  # untouched

    def test_close_with_no_slot_file_is_a_noop(self):
        self._seed("fb", "mike_query_review_fb_1.txt")
        rc = request_review.cmd_close(self._close_args("fb"), now=2)
        self.assertEqual(rc, 0)  # no slot file → close still succeeds

    def test_dry_run_does_not_clear_slot(self):
        letter = "mike_query_review_fb_1.txt"
        self._seed("fb", letter)
        self._slot.write_text(json.dumps({
            "subject": "review", "ts": 1, "file": letter,
            "review_target": "scripts/fallback_fire_drill.py",
        }))
        rc = request_review.cmd_close(self._close_args("fb", dry_run=True), now=2)
        self.assertEqual(rc, 0)
        self.assertTrue(self._slot.exists())  # dry-run is non-mutating


if __name__ == "__main__":
    unittest.main()
