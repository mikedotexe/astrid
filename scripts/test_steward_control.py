#!/usr/bin/env python3
"""Tests for the agent-neutral steward lifecycle."""

from __future__ import annotations

from dataclasses import replace
import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
import threading
import time
import unittest

try:
    from evidence_store import EvidenceEventStore
    from steward_control.config import ControlConfig, load_config
    from steward_control.controller import StewardController
    from steward_control.errors import BusyError, LeaseError, PausedError
    from steward_control.executor import run_subprocess
    from steward_control.git_state import repository_identity
    from steward_control.lease import token_hash
except ModuleNotFoundError:
    from scripts.evidence_store import EvidenceEventStore
    from scripts.steward_control.config import ControlConfig, load_config
    from scripts.steward_control.controller import StewardController
    from scripts.steward_control.errors import BusyError, LeaseError, PausedError
    from scripts.steward_control.executor import run_subprocess
    from scripts.steward_control.git_state import repository_identity
    from scripts.steward_control.lease import token_hash


def run_git(repo: Path, *args: str) -> None:
    subprocess.run(
        ["git", "-C", str(repo), *args],
        check=True,
        capture_output=True,
        timeout=20,
    )


class StewardControlTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.repo = self.root / "repo"
        self.repo.mkdir()
        run_git(self.repo, "init", "-q")
        run_git(self.repo, "config", "user.email", "test@example.invalid")
        run_git(self.repo, "config", "user.name", "Steward Test")
        (self.repo / "README.md").write_text("test\n", encoding="utf-8")
        run_git(self.repo, "add", "README.md")
        run_git(self.repo, "commit", "-qm", "initial")

        self.workspace = self.repo / "workspace"
        diagnostics = self.workspace / "diagnostics"
        self.store_root = diagnostics / "evidence_event_store_v2"
        self.state_root = diagnostics / "steward_control_v1"
        legacy = diagnostics / "legacy/events.jsonl"
        legacy.parent.mkdir(parents=True)
        legacy.write_text('{"legacy":true}\n', encoding="utf-8")
        legacy_hash = hashlib.sha256(legacy.read_bytes()).hexdigest()

        store = EvidenceEventStore(self.store_root)
        store.initialize_from_envelopes([], legacy_imported=True)
        store.activation_path.write_text(
            json.dumps({"active_store": "v2"}) + "\n",
            encoding="utf-8",
        )
        (self.store_root / "migration_receipt.json").write_text(
            json.dumps(
                {
                    "sources": [
                        {
                            "stream": "legacy",
                            "path": str(legacy),
                            "sha256": legacy_hash,
                        }
                    ]
                }
            )
            + "\n",
            encoding="utf-8",
        )
        self.config = ControlConfig(
            repo_root=self.repo,
            workspace=self.workspace,
            state_root=self.state_root,
            store_root=self.store_root,
            lease_ttl_secs=3,
            heartbeat_interval_secs=1,
            max_run_secs=2,
            projector_timeout_secs=2,
            pause_grace_secs=1,
            repositories={"astrid": self.repo},
            profile="none",
        )
        self.controller = StewardController(self.config)

    def tearDown(self) -> None:
        self.temp.cleanup()

    def resume(self) -> dict:
        return self.controller.resume(actor="test", acknowledgement="test resume")

    def test_config_precedence_and_portable_relative_paths(self) -> None:
        config_path = self.root / "config/steward.toml"
        config_path.parent.mkdir()
        config_path.write_text(
            "\n".join(
                [
                    "[control]",
                    'repo_root = "../repo"',
                    'workspace = "from-file"',
                    "lease_ttl_secs = 20",
                    "heartbeat_interval_secs = 2",
                ]
            ),
            encoding="utf-8",
        )
        loaded = load_config(
            config_path=config_path,
            workspace="from-cli",
            environ={"ASTRID_STEWARD_LEASE_TTL_SECS": "30"},
        )
        self.assertEqual(loaded.repo_root, self.repo.resolve())
        self.assertEqual(loaded.workspace, (self.repo / "from-cli").resolve())
        self.assertEqual(loaded.lease_ttl_secs, 30)
        self.assertNotIn(str(self.root), loaded.canonical_record()["repo_root"])

    def test_control_state_starts_paused_and_begin_is_denied(self) -> None:
        status = self.controller.status()
        self.assertTrue(status["state"]["paused"])
        with self.assertRaises(PausedError):
            self.controller.begin(actor="test")

    def test_pause_is_owner_only_and_token_is_not_exposed_by_status(self) -> None:
        self.resume()
        begin = self.controller.begin(actor="test")
        lease_path = self.state_root / "lease.json"
        mode = stat.S_IMODE(lease_path.stat().st_mode)
        self.assertEqual(mode, 0o600)
        self.assertNotIn(begin["lease_token"], lease_path.read_text(encoding="utf-8"))
        status = self.controller.status()
        self.assertNotIn("token_sha256", status["lease"])
        self.controller.pause(actor="test", reason="test pause")
        heartbeat = self.controller.heartbeat(
            run_id=begin["run_id"],
            lease_token=begin["lease_token"],
        )
        self.assertTrue(heartbeat["stop_requested"])
        self.controller.finish(
            run_id=begin["run_id"],
            lease_token=begin["lease_token"],
            outcome="cancelled",
        )

    def test_live_lease_is_exclusive_and_never_preempted(self) -> None:
        self.resume()
        begin = self.controller.begin(actor="first")
        with self.assertRaises(BusyError):
            self.controller.begin(actor="second")
        self.assertEqual(
            self.controller.leases.lease()["run_id"],
            begin["run_id"],
        )
        self.controller.finish(
            run_id=begin["run_id"],
            lease_token=begin["lease_token"],
            outcome="success",
        )

    def test_wrong_token_and_duplicate_finish(self) -> None:
        self.resume()
        begin = self.controller.begin(actor="test")
        with self.assertRaises(LeaseError):
            self.controller.heartbeat(
                run_id=begin["run_id"],
                lease_token="wrong",
            )
        finished = self.controller.finish(
            run_id=begin["run_id"],
            lease_token=begin["lease_token"],
            outcome="success",
        )
        duplicate = self.controller.finish(
            run_id=begin["run_id"],
            lease_token=begin["lease_token"],
            outcome="success",
        )
        self.assertFalse(finished["idempotent"])
        self.assertTrue(duplicate["idempotent"])

    def test_stale_session_is_reaped_by_expiry(self) -> None:
        self.resume()
        begin = self.controller.begin(actor="old")
        lease_path = self.state_root / "lease.json"
        lease = json.loads(lease_path.read_text(encoding="utf-8"))
        lease["expires_at_unix"] = time.time() - 1
        lease_path.write_text(json.dumps(lease), encoding="utf-8")
        replacement = self.controller.begin(actor="new")
        self.assertNotEqual(replacement["run_id"], begin["run_id"])
        self.controller.finish(
            run_id=replacement["run_id"],
            lease_token=replacement["lease_token"],
            outcome="success",
        )

    def test_worktree_edit_is_allowed_but_staging_is_policy_violation(self) -> None:
        self.resume()
        begin = self.controller.begin(actor="test")
        (self.repo / "new.txt").write_text("new\n", encoding="utf-8")
        run_git(self.repo, "add", "new.txt")
        finished = self.controller.finish(
            run_id=begin["run_id"],
            lease_token=begin["lease_token"],
            outcome="success",
        )
        self.assertEqual(finished["receipt"]["outcome"], "policy_violation")
        self.assertTrue(finished["receipt"]["git_policy_violations"])
        self.assertEqual(
            repository_identity(self.repo)["head"],
            finished["receipt"]["repositories_before"]["astrid"]["head"],
        )

    def test_event_stream_has_no_raw_token(self) -> None:
        self.resume()
        begin = self.controller.begin(actor="test")
        self.controller.finish(
            run_id=begin["run_id"],
            lease_token=begin["lease_token"],
            outcome="success",
        )
        events, corrupt = EvidenceEventStore(self.store_root).payloads_for_stream(
            "steward_control"
        )
        self.assertEqual(corrupt, 0)
        serialized = json.dumps(events)
        self.assertNotIn(begin["lease_token"], serialized)
        self.assertIn(token_hash(begin["lease_token"]), json.dumps(
            json.loads(
                (self.state_root / "runs" / f"{begin['run_id']}.json").read_text(
                    encoding="utf-8"
                )
            )
        ))

    def test_pause_spools_when_store_is_unavailable(self) -> None:
        (self.store_root / "active_store.json").unlink()
        result = self.controller.pause(actor="test", reason="evidence outage")
        self.assertFalse(result["event"]["appended"])
        self.assertEqual(len(self.controller.events.pending()), 1)

    def test_external_state_root_uses_configured_store_without_path_inference(
        self,
    ) -> None:
        external = replace(
            self.config,
            state_root=self.root / "private-control-state",
        )
        controller = StewardController(external)
        result = controller.pause(actor="test", reason="portable state")
        self.assertTrue(result["event"]["appended"])
        events, corrupt = EvidenceEventStore(self.store_root).payloads_for_stream(
            "steward_control"
        )
        self.assertEqual(corrupt, 0)
        self.assertTrue(events)
        self.assertEqual(
            stat.S_IMODE((external.state_root / "control.json").stat().st_mode),
            0o600,
        )

    def test_subprocess_adapter_propagates_exit_and_releases_lease(self) -> None:
        self.resume()
        return_code, result = run_subprocess(
            self.controller,
            actor="test",
            argv=[sys.executable, "-c", "raise SystemExit(7)"],
            max_secs=2,
        )
        self.assertEqual(return_code, 7)
        self.assertEqual(result["receipt"]["outcome"], "failed")
        self.assertIsNone(self.controller.leases.lease())

    def test_subprocess_watchdog_requests_graceful_interrupt(self) -> None:
        self.resume()
        started = time.monotonic()
        return_code, result = run_subprocess(
            self.controller,
            actor="test",
            argv=[
                sys.executable,
                "-c",
                (
                    "import signal,sys,time; "
                    "signal.signal(signal.SIGINT, lambda *_: sys.exit(130)); "
                    "time.sleep(30)"
                ),
            ],
            max_secs=1,
        )
        self.assertNotEqual(return_code, 0)
        self.assertEqual(result["receipt"]["outcome"], "cancelled")
        self.assertLess(time.monotonic() - started, 4)

    def test_pause_cooperatively_interrupts_wrapped_subprocess(self) -> None:
        self.resume()

        def pause_soon() -> None:
            time.sleep(0.2)
            self.controller.pause(actor="test", reason="fixture stop")

        thread = threading.Thread(target=pause_soon)
        thread.start()
        return_code, result = run_subprocess(
            self.controller,
            actor="test",
            argv=[
                sys.executable,
                "-c",
                (
                    "import signal,sys,time; "
                    "signal.signal(signal.SIGINT, lambda *_: sys.exit(130)); "
                    "time.sleep(30)"
                ),
            ],
            max_secs=10,
        )
        thread.join(timeout=2)
        self.assertNotEqual(return_code, 0)
        self.assertEqual(result["receipt"]["outcome"], "cancelled")
        self.assertTrue(self.controller.status()["state"]["paused"])

    def test_source_lag_reports_durable_cutoff_and_newest_file(self) -> None:
        introspections = self.workspace / "introspections"
        introspections.mkdir()
        (introspections / "introspection_test_100.txt").write_text(
            "old\n",
            encoding="utf-8",
        )
        (introspections / "introspection_test_120.txt").write_text(
            "new\n",
            encoding="utf-8",
        )
        addressing = (
            self.workspace / "diagnostics/introspection_addressing_v1"
        )
        addressing.mkdir()
        (addressing / "status.json").write_text(
            json.dumps(
                {
                    "cutoff": {
                        "cutoff": "introspection_test_100.txt",
                        "cutoff_timestamp": 100,
                    }
                }
            ),
            encoding="utf-8",
        )
        lag = self.controller.status()["source_lag"]
        self.assertEqual(
            lag["durable_cutoff_filename"],
            "introspection_test_100.txt",
        )
        self.assertEqual(
            lag["newest_canonical_filename"],
            "introspection_test_120.txt",
        )
        self.assertEqual(lag["timestamp_lag"], 20)

    def test_full_lifecycle_does_not_mutate_git_identity(self) -> None:
        run_git(
            self.repo,
            "remote",
            "add",
            "origin",
            "https://example.invalid/portable.git",
        )

        def snapshot() -> tuple[bytes, bytes, bytes, bytes]:
            commands = (
                ("rev-parse", "HEAD"),
                ("status", "--porcelain=v1", "-z"),
                ("for-each-ref", "--format=%(refname):%(objectname)"),
                ("remote", "-v"),
            )
            return tuple(
                subprocess.run(
                    ["git", "-C", str(self.repo), *command],
                    check=True,
                    capture_output=True,
                ).stdout
                for command in commands
            )

        before = snapshot()
        self.resume()
        begun = self.controller.begin(actor="test")
        self.controller.finish(
            run_id=begun["run_id"],
            lease_token=begun["lease_token"],
            outcome="success",
        )
        self.assertEqual(snapshot(), before)

    def test_git_identity_is_read_only(self) -> None:
        before = subprocess.run(
            ["git", "-C", str(self.repo), "status", "--porcelain=v1", "-z"],
            capture_output=True,
            check=True,
        ).stdout
        repository_identity(self.repo)
        after = subprocess.run(
            ["git", "-C", str(self.repo), "status", "--porcelain=v1", "-z"],
            capture_output=True,
            check=True,
        ).stdout
        self.assertEqual(before, after)


if __name__ == "__main__":
    unittest.main()
