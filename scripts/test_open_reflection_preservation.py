"""Schema-13 old/new helper qualification, using synthetic isolated state only."""
import json
import os
from pathlib import Path
import tempfile
import unittest

from qualify_geometry_release import Fixture, SOURCE, file_hashes
from qualify_reflection_continuity_release import once


@unittest.skipUnless(os.environ.get("QUALIFICATION_OLD_READER") and
                     os.environ.get("ASTRID_SOURCE_STUDY_BIN"), "actual helper paths required")
class CurrentReflectionPreservation(unittest.TestCase):
    def test_both_owners_keep_old_pending_input_and_choose_new_continuation(self):
        old = Path(os.environ["QUALIFICATION_OLD_READER"])
        new = Path(os.environ["ASTRID_SOURCE_STUDY_BIN"])
        with tempfile.TemporaryDirectory() as temporary:
            for owner in ("astrid", "minime"):
                with self.subTest(owner=owner):
                    f = Fixture(Path(temporary) / owner, owner, old, new)
                    f.prepare(old, "SELF_STUDY QUESTION NEW Synthetic unresolved question?")
                    page = f.prepare(old, f"SELF_STUDY OPEN {SOURCE} 1")
                    f.deliver(old, page, "STUDY_NOTE: Exact retained note.\nNEXT: INTROSPECT")
                    draft = f.prepare(old, "WRITE START unrelated private work")
                    f.deliver(old, draft, "PRIVATE_UNSELECTED_SENTINEL\nNEXT: WRITE PARK")
                    f.prepare(old, "WRITE PARK")
                    published = f.workspace / "journal/introspect_2026-10-07.txt"
                    published.parent.mkdir(parents=True)
                    published.write_text("=== SYNTHETIC PUBLIC ===\n\nPUBLIC_PRIOR_SENTINEL\nNEXT: REST\n")
                    original = published.read_bytes()
                    before = f.checkpoint()
                    self.assertEqual(before["version"], 13)
                    prepared, retry = once(f, old, "pending-old", "INTROSPECT")
                    self.assertIn("PUBLIC_PRIOR_SENTINEL", prepared["text"])
                    self.assertEqual(f.call(new, **retry), prepared)
                    authored = "Exact selected passage, including λ. No conclusion."
                    action = "WRITE FROM_REFLECTION " + prepared["navigation_id"]
                    receipt = f.deliver(new, prepared, authored + "\nNEXT: " + action)
                    artifact = Path(receipt["artifact_path"]).read_bytes()
                    fresh, _ = once(f, new, "fresh-new", "INTROSPECT")
                    for absent in ("PUBLIC_PRIOR_SENTINEL", "PRIVATE_UNSELECTED_SENTINEL", authored):
                        self.assertNotIn(absent, fresh["text"])
                    carried, request = once(f, new, "explicit-continuation", action)
                    self.assertIn(authored, carried["text"])
                    self.assertNotIn("PRIVATE_UNSELECTED_SENTINEL", carried["text"])
                    self.assertEqual(carried, f.call(new, **request))
                    stable = file_hashes(f.state)
                    f.call(new, reject=True, **(request | {"action": "WRITE START conflicting"}))
                    self.assertEqual(stable, file_hashes(f.state))
                    f.deliver(new, carried, "Optional development.\nNEXT: WRITE PARK")
                    f.prepare(new, "WRITE PARK")
                    returned = f.prepare(new, "WRITE RESUME d2")
                    self.assertIn(authored, returned["text"])
                    self.assertIn("Optional development.", returned["text"])
                    after = f.checkpoint()
                    self.assertEqual(after["version"], 13)
                    for field in ("questions", "bookmarks", "notebook", "progress"):
                        self.assertEqual(before[field], after[field], field)
                    self.assertEqual(published.read_bytes(), original)
                    self.assertEqual(Path(receipt["artifact_path"]).read_bytes(), artifact)
                    drafts = json.loads((f.state / "writing/drafts-v2.json").read_bytes())
                    self.assertEqual(drafts["drafts"]["d1"]["parts"], ["PRIVATE_UNSELECTED_SENTINEL"])
                    # Same-schema old helper inspection must not erase newer authored work.
                    f.prepare(old, "WRITE HELP")
                    inspected = json.loads((f.state / "writing/drafts-v2.json").read_bytes())
                    self.assertEqual(drafts["drafts"], inspected["drafts"])


if __name__ == "__main__":
    unittest.main()
