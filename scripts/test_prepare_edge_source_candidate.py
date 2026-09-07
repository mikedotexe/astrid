#!/usr/bin/env python3
"""Temporary Git fixtures only; no devices, network, or runtime services."""

import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import tarfile
import tempfile
import unittest
from unittest import mock

SPEC = importlib.util.spec_from_file_location("candidate", Path(__file__).with_name("prepare_edge_source_candidate.py"))
CANDIDATE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CANDIDATE)


class CandidateTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.repo = self.root / "repo"
        self.repo.mkdir()
        self.git("init", "-q")
        self.git("config", "user.name", "Fixture")
        self.git("config", "user.email", "fixture@example.invalid")
        self.git("config", "commit.gpgsign", "false")
        self.git("config", "tag.gpgsign", "false")
        (self.repo / "Cargo.toml").write_text('[workspace.package]\nversion = "0.5.6"\n')
        (self.repo / "Cargo.lock").write_text("version = 4\n")
        (self.repo / ".gitignore").write_text("/private-local/\n")
        specs = self.repo / "packaging/headless"
        specs.mkdir(parents=True)
        for name in ("astralis-cpu-edge-capsules.toml", "astralis-cpu-edge-baseline-capsules.toml"):
            (specs / name).write_text('[[capsule]]\npackage="fixture"\nrevision="exact-pin"\n')
        self.git("add", "Cargo.toml", "Cargo.lock", ".gitignore", "packaging/headless")
        self.git("commit", "-qm", "fixture")
        self.commit = self.git("rev-parse", "HEAD").decode().strip()
        self.git("tag", "-a", "introspection", "-m", "fixture checkpoint")
        self.output = self.root / "candidate"

    def git(self, *args):
        return subprocess.check_output(["git", "-C", str(self.repo), *args], stderr=subprocess.PIPE)

    def prepare(self, **overrides):
        args = dict(repo=self.repo, source_ref="refs/tags/introspection", expected_commit=self.commit, output=self.output)
        return CANDIDATE.prepare(**(args | overrides))

    def test_exact_annotated_tag_archive_and_inputs(self):
        ignored = self.repo / "private-local"
        ignored.mkdir()
        (ignored / "secret").write_text("not published")
        receipt = self.prepare()
        self.assertEqual(receipt["source_commit"], self.commit)
        self.assertFalse(receipt["deploy_authorized"])
        self.assertEqual(receipt["status"], "source_pinned_not_linux_release_qualified")
        self.assertEqual(receipt["workspace_version"], "0.5.6")
        self.assertEqual(receipt["source_archive"]["sha256"], hashlib.sha256((self.output / "source.tar.gz").read_bytes()).hexdigest())
        with tarfile.open(self.output / "source.tar.gz") as archive:
            self.assertEqual(archive.extractfile("source/Cargo.lock").read(), b"version = 4\n")
            self.assertFalse(any("private-local" in name for name in archive.getnames()))
        self.assertEqual(json.loads((self.output / "candidate.json").read_text()), receipt)
        self.assertEqual(self.output.stat().st_mode & 0o777, 0o700)
        self.assertNotIn(str(self.repo), json.dumps(receipt))

    def test_existing_output_is_never_overwritten(self):
        self.output.mkdir()
        sentinel = self.output / "source.tar.gz"
        sentinel.write_bytes(b"preserve")
        with self.assertRaises(FileExistsError):
            self.prepare()
        self.assertEqual(sentinel.read_bytes(), b"preserve")

    def test_dirty_and_untracked_input_are_rejected(self):
        for path in ("Cargo.lock", "new-file"):
            with self.subTest(path=path):
                target = self.repo / path
                previous = target.read_bytes() if target.exists() else None
                target.write_text("changed")
                with self.assertRaisesRegex(ValueError, "tracked or untracked"):
                    self.prepare()
                if previous is None:
                    target.unlink()
                else:
                    target.write_bytes(previous)
                self.assertFalse(self.output.exists())

    def test_wrong_ref_commit_and_head_are_rejected(self):
        with self.assertRaisesRegex(ValueError, "expected commit"):
            self.prepare(expected_commit="0" * 40)
        self.git("commit", "--allow-empty", "-qm", "different head")
        with self.assertRaisesRegex(ValueError, "HEAD"):
            self.prepare()
        self.assertFalse(self.output.exists())

    def test_unqualified_ref_short_sha_and_source_output_are_rejected(self):
        for override in ({"source_ref": "introspection"}, {"expected_commit": self.commit[:12]}, {"output": self.repo / "output"}):
            with self.subTest(override=override), self.assertRaises(ValueError):
                self.prepare(**override)
        self.assertFalse(self.output.exists())

    def test_archive_failure_never_creates_success_receipt(self):
        original = subprocess.run

        def fail_archive(args, **kwargs):
            if "archive" in args:
                raise OSError("fixture archive failure")
            return original(args, **kwargs)

        with mock.patch.object(CANDIDATE.subprocess, "run", side_effect=fail_archive):
            with self.assertRaises(OSError):
                self.prepare()
        self.assertTrue(self.output.is_dir())
        self.assertFalse((self.output / "candidate.json").exists())

    def test_ref_change_during_archive_never_creates_success_receipt(self):
        original = CANDIDATE.require_clean
        count = 0

        def change_ref(repo, commit):
            nonlocal count
            original(repo, commit)
            count += 1
            if count == 2:
                self.git("tag", "-d", "introspection")

        with mock.patch.object(CANDIDATE, "require_clean", side_effect=change_ref):
            with self.assertRaises(subprocess.CalledProcessError):
                self.prepare()
        self.assertFalse((self.output / "candidate.json").exists())


if __name__ == "__main__":
    unittest.main()
