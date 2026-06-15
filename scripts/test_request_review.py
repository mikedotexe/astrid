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

    def test_validate_target_override_bypasses_guard(self):
        # allow_unresolved=True: bypasses both the label block and the resolve probe.
        err = request_review.validate_target("your stuff (a / b)", allow_unresolved=True)
        self.assertIsNone(err)


class PostChangeQATests(unittest.TestCase):
    """The #9 post-change QA framing + ledger tag."""

    def test_post_change_letter_frames_the_confirmation(self):
        letter = request_review.post_change_letter(
            "src/codec.rs",
            "anything feel softer?",
            "shipped smoothstep gate at codec.rs:71",
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
        # It must NOT read as reopening consent.
        self.assertIn("does not reopen", letter.lower())

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


if __name__ == "__main__":
    unittest.main()
