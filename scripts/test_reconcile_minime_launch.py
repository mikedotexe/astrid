"""Synthetic launch identity and explicit overlay checks; never a service transition."""
import json
from pathlib import Path
import plistlib
import tempfile
import unittest
from unittest.mock import patch

import reconcile_minime_launch as tool


class ReconciliationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name).resolve()
        self.addCleanup(self.cleanup)
        self.canonical, self.candidate = self.root / "canonical", self.root / "candidate"
        for root in (self.canonical, self.candidate):
            for name in (*tool.OVERLAY, *tool.PRESERVE_CANONICAL, *tool.ASSETS,
                         tool.LAUNCHER, "scripts/minime_rescue_investigation.py", tool.PLIST):
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("# synthetic\n")
            (root / tool.PLIST).write_bytes(plistlib.dumps({"KeepAlive": True,
                "ProgramArguments": ["/bin/bash", str(self.canonical / tool.LAUNCHER)]}))
        self.installed = self.root / "installed.plist"
        self.installed.write_bytes((self.canonical / tool.PLIST).read_bytes())
        self.status = self.root / "status.json"
        self.refresh_status()

    def cleanup(self):
        for path in self.root.rglob("*"):
            if path.is_dir() and not path.is_symlink():
                path.chmod(0o700)
        self.temp.cleanup()

    def refresh_status(self):
        self.status.write_text(json.dumps({"source_inputs_at_start": tool.inventory(self.canonical),
            "source_path": str(self.canonical / "minime_autonomy/runtime.py"),
            "reload_required": False, "pid": 42, "checked_at": "synthetic"}))

    def run_tool(self):
        return tool.reconcile(self.canonical, self.candidate, self.installed, self.status, self.root / "out")

    def test_reviewed_overlay_keeps_newer_visual_service_and_assets(self):
        for name in tool.OVERLAY:
            (self.candidate / name).write_text("# candidate\n")
        (self.canonical / "visual_frame_service.py").write_text("# newer live visual\n")
        self.refresh_status()
        before = tool.inventory(self.canonical)
        result = self.run_tool()
        self.assertFalse(result["live_eligible_now"])
        self.assertFalse(result["canonical_writes"])
        self.assertEqual(tool.inventory(self.canonical), before)
        tree = Path(result["snapshot_root"])
        self.assertEqual((tree / "visual_frame_service.py").read_text(), "# newer live visual\n")
        self.assertTrue(all((tree / name).read_text() == "# candidate\n" for name in tool.OVERLAY))
        self.assertFalse(tree.stat().st_mode & 0o222)
        self.assertTrue((tree / tool.ASSETS[0]).exists())

    def test_new_reviewed_modules_are_snapshotted_without_canonical_writes(self):
        added = ("minime_autonomy/moment_context.py", "minime_autonomy/study_feedback.py",
                 "minime_autonomy/journal_recall.py")
        for name in added:
            (self.canonical / name).unlink()
        self.refresh_status()
        before = tool.inventory(self.canonical)
        result = self.run_tool()
        self.assertEqual(tool.inventory(self.canonical), before)
        self.assertEqual(set(result["selected_inputs"]) - set(before), set(added))
        for name in added:
            self.assertEqual((Path(result["snapshot_root"]) / name).read_bytes(),
                             (self.candidate / name).read_bytes())

    def test_legacy_recall_consumers_are_selected_without_touching_live_sources(self):
        names = ("minime_autonomy/expressive_journal.py", "minime_autonomy/research.py")
        for name in names:
            (self.canonical / name).write_text("# old recall consumer\n")
            (self.candidate / name).write_text("# typed recall consumer\n")
        self.refresh_status()
        before = tool.inventory(self.canonical)
        result = self.run_tool()
        self.assertEqual(tool.inventory(self.canonical), before)
        for name in names:
            self.assertEqual(result["selected_inputs"][name], tool.sha(self.candidate / name))
            self.assertEqual((Path(result["snapshot_root"]) / name).read_bytes(),
                             (self.candidate / name).read_bytes())

    def test_unreviewed_difference_fails(self):
        (self.candidate / "unexpected.py").write_text("# foreign\n")
        with self.assertRaisesRegex(ValueError, "unreviewed"):
            self.run_tool()

    def test_private_writing_recorders_are_bound_to_reviewed_overlay(self):
        names = ("minime_autonomy/generation_record.py", "minime_autonomy/source_study_diagnostics.py")
        before = tool.inventory(self.canonical)
        for name in names:
            self.assertIn(name, tool.OVERLAY)
            (self.candidate / name).write_text("# private-writing diagnostic containment\n")
        result = self.run_tool()
        self.assertEqual(tool.inventory(self.canonical), before)
        for name in names:
            self.assertEqual(result["selected_inputs"][name], tool.sha(self.candidate / name))
            self.assertEqual((Path(result["snapshot_root"]) / name).read_bytes(),
                             (self.candidate / name).read_bytes())

    def test_reflection_authority_repair_is_in_exact_reviewed_launch_inventory(self):
        name = "minime_autonomy/authority.py"
        self.assertIn(name, tool.OVERLAY)
        before = tool.inventory(self.canonical)
        (self.candidate / name).write_text("# reviewed reflection admission\n")
        result = self.run_tool()
        self.assertEqual(tool.inventory(self.canonical), before)
        self.assertEqual(result["selected_inputs"][name], tool.sha(self.candidate / name))
        self.assertEqual((Path(result["snapshot_root"]) / name).read_bytes(),
                         (self.candidate / name).read_bytes())

    def test_afterimage_presentation_modules_are_bound_to_reviewed_inventory(self):
        names = ("minime_autonomy/afterimages.py", "minime_autonomy/afterimage_prompts.py")
        before = tool.inventory(self.canonical)
        for name in names:
            (self.candidate / name).write_text("# reviewed afterimage presentation\n")
        result = self.run_tool()
        self.assertEqual(tool.inventory(self.canonical), before)
        for name in names:
            self.assertEqual(result["selected_inputs"][name], tool.sha(self.candidate / name))
            self.assertEqual((Path(result["snapshot_root"]) / name).read_bytes(),
                             (self.candidate / name).read_bytes())

    def test_live_loaded_identity_drift_fails(self):
        (self.canonical / "minime_autonomy/runtime.py").write_text("# new unlaunched\n")
        with self.assertRaisesRegex(ValueError, "running source identity"):
            self.run_tool()

    def test_launch_arguments_drift_fails(self):
        self.installed.write_bytes(plistlib.dumps({"ProgramArguments": ["wrong"]}))
        with self.assertRaisesRegex(ValueError, "launch plist"):
            self.run_tool()

    def test_asset_drift_fails(self):
        (self.candidate / tool.ASSETS[0]).write_text("changed")
        with self.assertRaisesRegex(ValueError, "asset differences"):
            self.run_tool()

    def test_partial_snapshot_keeps_failure_and_never_changes_canonical(self):
        before = tool.inventory(self.canonical)
        original = tool.checked_copy

        def corrupt(source, target, expected):
            original(source, target, expected)
            (self.candidate / "minime_autonomy/runtime.py").write_text("# concurrent writer\n")

        with patch.object(tool, "checked_copy", side_effect=corrupt):
            with self.assertRaisesRegex(ValueError, "changed during snapshot|source drift"):
                self.run_tool()
        self.assertEqual(tool.inventory(self.canonical), before)
        self.assertTrue((self.root / "out/failure.json").exists())

    def test_existing_packet_is_never_overwritten(self):
        self.run_tool()
        receipt = (self.root / "out/reconciliation.json").read_bytes()
        with self.assertRaises(FileExistsError):
            self.run_tool()
        self.assertEqual((self.root / "out/reconciliation.json").read_bytes(), receipt)

    def test_symlink_input_is_refused(self):
        source = self.candidate / "minime_autonomy/writing.py"
        source.unlink()
        source.symlink_to(self.canonical / "minime_autonomy/writing.py")
        with self.assertRaisesRegex(ValueError, "symlink"):
            self.run_tool()


if __name__ == "__main__":
    unittest.main()
