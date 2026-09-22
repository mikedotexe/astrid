"""Private, synthetic evidence only; no launchd, live state or provider calls."""
import fcntl
import hashlib
import json
import os
import re
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import provider_observation_spool as spool


class SpoolTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.parent = Path(self.temp.name).resolve()
        self.old = self.parent / "old"
        self.old.mkdir(mode=0o700)
        for lane in ("events", "raw"):
            (self.old / lane).mkdir(mode=0o700)
        self.put(self.old / "writer.lock", b"")
        self.put(self.old / "raw/raw.txt", b"synthetic private fixture")
        self.put(self.old / "events/event.json", b'{"raw_artifact":"raw/raw.txt"}')
        self.manifest = self.parent / "seal.json"

    def put(self, path, data):
        path.write_bytes(data)
        path.chmod(0o600)

    def test_seal_and_verify_keep_exact_files_and_relative_references(self):
        before = {p: p.read_bytes() for p in self.old.glob("*/*")}
        result = spool.seal(self.old, self.manifest)
        self.assertEqual(result["sha256"], hashlib.sha256(self.manifest.read_bytes()).hexdigest())
        self.assertTrue(spool.verify(self.old, self.manifest)["verified"])
        self.assertEqual(before, {p: p.read_bytes() for p in self.old.glob("*/*")})
        self.assertEqual(self.manifest.stat().st_mode & 0o777, 0o600)

    def test_prepare_does_not_select_config_or_modify_predecessor(self):
        new = self.parent / "new"
        result = spool.prepare(new, self.old)
        self.assertFalse(result["configuration_changed"])
        self.assertFalse(result["predecessor_sealed"])
        self.assertEqual(spool.entries(new), [])
        self.assertEqual((new / "raw").stat().st_mode & 0o777, 0o700)
        self.assertEqual(json.loads((new / "epoch.json").read_text())["predecessor"], str(self.old))
        with self.assertRaises(FileExistsError):
            spool.prepare(new, self.old)

    def test_live_cross_process_writer_blocks_seal(self):
        with (self.old / "writer.lock").open("r+") as handle:
            fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
            result = subprocess.run([sys.executable, spool.__file__, "seal", "--root", str(self.old),
                                     "--manifest", str(self.manifest)], capture_output=True, text=True)
        self.assertEqual(result.returncode, 1)
        self.assertIn("writer active", result.stderr)
        self.assertFalse(self.manifest.exists())

    def test_duplicate_seal_cannot_overwrite(self):
        spool.seal(self.old, self.manifest)
        before = self.manifest.read_bytes()
        with self.assertRaises(FileExistsError):
            spool.seal(self.old, self.manifest)
        self.assertEqual(self.manifest.read_bytes(), before)

    def test_tamper_and_added_or_removed_files_fail(self):
        spool.seal(self.old, self.manifest)
        target = self.old / "raw/raw.txt"
        before = target.read_bytes()
        for value in (b"changed private fixture", b""):
            self.put(target, value)
            with self.assertRaises(ValueError):
                spool.verify(self.old, self.manifest)
        self.put(target, before)
        self.assertTrue(spool.verify(self.old, self.manifest)["verified"])
        self.put(self.old / "events/new.json", b"{}")
        with self.assertRaises(ValueError):
            spool.verify(self.old, self.manifest)
        (self.old / "events/new.json").unlink()
        target.unlink()
        with self.assertRaises(ValueError):
            spool.verify(self.old, self.manifest)

    def test_symlink_hardlink_and_world_readable_evidence_refused(self):
        target = self.old / "raw/raw.txt"
        target.chmod(0o644)
        with self.assertRaises(ValueError):
            spool.entries(self.old)
        target.chmod(0o600)
        os.link(target, self.parent / "alias")
        with self.assertRaises(ValueError):
            spool.entries(self.old)
        (self.parent / "alias").unlink()
        (self.old / "events/link").symlink_to(target)
        with self.assertRaises(ValueError):
            spool.entries(self.old)

    def test_noncanonical_and_nonprivate_paths_refused(self):
        alias = self.parent / "alias"
        alias.symlink_to(self.old, target_is_directory=True)
        with self.assertRaises(ValueError):
            spool.entries(alias)
        with self.assertRaises(ValueError):
            spool.entries(Path("relative"))
        with self.assertRaises(ValueError):
            spool.seal(self.old, self.old / "seal.json")
        self.old.chmod(0o755)
        with self.assertRaises(ValueError):
            spool.entries(self.old)

    def test_partial_files_are_counted_not_called_receipts(self):
        self.put(self.old / "events/unfinished.tmp", b"partial")
        actual = spool.capacity(spool.entries(self.old))
        self.assertEqual(actual["usage"]["files"], 3)
        self.assertEqual(actual["partial_files"], 1)
        spool.seal(self.old, self.manifest)
        self.assertTrue(spool.verify(self.old, self.manifest)["verified"])

    def test_near_and_full_limits_include_exact_file_capacity(self):
        with patch.object(spool, "LIMITS", {"files": 2, "bytes": 1000, "raw_bytes": 1000}):
            result = spool.capacity(spool.entries(self.old))
            self.assertEqual(result["status"], "full")
            self.assertEqual(result["remaining"]["files"], 0)
        with patch.object(spool, "LIMITS", {"files": 10, "bytes": 60, "raw_bytes": 1000}):
            self.assertEqual(spool.capacity(spool.entries(self.old))["status"], "near_capacity")
        with patch.object(spool, "MAX_FILES", 2):
            self.assertEqual(len(spool.entries(self.old)), 2)
            self.put(self.old / "events/extra.json", b"{}")
            with self.assertRaises(ValueError):
                spool.entries(self.old)

    def test_mutation_during_hashing_refuses_seal(self):
        original = spool.entries
        calls = 0
        def changed(root):
            nonlocal calls
            calls += 1
            if calls == 2:
                self.put(self.old / "events/late.json", b"{}")
            return original(root)
        with patch.object(spool, "entries", changed), self.assertRaises(ValueError):
            spool.seal(self.old, self.manifest)
        self.assertFalse(self.manifest.exists())

    def test_incomplete_publication_preserves_evidence(self):
        with patch.object(spool.os, "link", side_effect=OSError("injected interruption")):
            with self.assertRaises(OSError):
                spool.seal(self.old, self.manifest)
        self.assertFalse(self.manifest.exists())
        self.assertEqual(len(spool.entries(self.old)), 2)
        self.assertEqual(len(list(self.parent.glob(".*.pending"))), 1)
        spool.seal(self.old, self.manifest)
        self.assertTrue(spool.verify(self.old, self.manifest)["verified"])

    def test_wrong_epoch_or_corrupt_manifest_fails_without_repair(self):
        spool.seal(self.old, self.manifest)
        record = json.loads(self.manifest.read_text())
        record["root"] = "/wrong"
        self.put(self.manifest, json.dumps(record).encode())
        with self.assertRaises(ValueError):
            spool.verify(self.old, self.manifest)
        self.put(self.manifest, b"{partial")
        with self.assertRaises(ValueError):
            spool.verify(self.old, self.manifest)
        self.assertEqual(self.manifest.read_bytes(), b"{partial")

    def test_limits_match_actual_production_constants(self):
        source = (Path(__file__).resolve().parents[1] /
                  "capsules/spectral-bridge/src/llm/provider/provider_observation.rs").read_text()
        for name, expected in (("PROVIDER_SPOOL_MAX_FILES", spool.MAX_FILES),
                               ("PROVIDER_SPOOL_MAX_BYTES", spool.MAX_BYTES),
                               ("PROVIDER_RAW_SPOOL_MAX_BYTES", spool.MAX_RAW_BYTES)):
            match = re.search(rf"const {name}: \w+ = ([\d_]+);", source)
            self.assertIsNotNone(match)
            self.assertEqual(int(match[1].replace("_", "")), expected)

    def test_symlink_lock_and_manifest_refused(self):
        lock = self.old / "writer.lock"
        lock.unlink()
        lock.symlink_to(self.old / "raw/raw.txt")
        with self.assertRaises(ValueError):
            spool.seal(self.old, self.manifest)
        self.manifest.symlink_to(self.old / "events/event.json")
        with self.assertRaises(ValueError):
            spool.verify(self.old, self.manifest)


if __name__ == "__main__":
    unittest.main()
