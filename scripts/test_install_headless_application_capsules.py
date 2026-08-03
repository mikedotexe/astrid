#!/usr/bin/env python3
"""Integration tests for the external headless capsule transaction installer."""

from __future__ import annotations

import json
import os
import stat
import subprocess
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent
INSTALLER = REPO_ROOT / "scripts/install_headless_application_capsules.py"


FAKE_ASTRID = r'''#!/usr/bin/env python3
import json
import os
import pathlib
import shutil
import sys
from datetime import datetime, timezone

home = pathlib.Path(os.environ["ASTRID_HOME"])
live_home = os.environ.get("FAKE_LIVE_HOME")
is_live = live_home is not None and home == pathlib.Path(live_home)
args = sys.argv[1:]
if args[:2] == ["capsule", "install"] and len(args) == 3:
    archive = pathlib.Path(args[2])
    spec = json.loads(archive.read_text(encoding="utf-8"))
    capsule_id = spec["id"]
    if "required_env_value" in spec:
        env_path = home / "home/default/.config/env" / (capsule_id + ".env.json")
        try:
            configured = json.loads(env_path.read_text(encoding="utf-8"))["model"]
        except Exception:
            configured = None
        if configured != spec["required_env_value"]:
            print("required lifecycle environment missing", file=sys.stderr)
            raise SystemExit(72)
    target = home / "home/default/.local/capsules" / capsule_id
    backup = target.with_suffix(".bak")
    if backup.exists():
        shutil.rmtree(backup)
    installed_at = "2026-01-01T00:00:00Z"
    if target.exists():
        try:
            installed_at = json.loads((target / "meta.json").read_text())["installed_at"]
        except Exception:
            pass
        target.rename(backup)
    target.mkdir(parents=True)
    (target / "Capsule.toml").write_text(
        f'[package]\nname = "{capsule_id}"\nversion = "1.0.0"\n',
        encoding="utf-8",
    )
    (target / "payload.txt").write_text(spec["payload"] + "\n", encoding="utf-8")
    now = datetime.now(timezone.utc).isoformat()
    meta = {
        "version": "1.0.0",
        "installed_at": installed_at,
        "updated_at": now,
        "source": str(archive),
        "imports": {},
        "exports": {},
        "topics": [],
        "wit_files": {},
    }
    if "wasm_hash" in spec:
        meta["wasm_hash"] = spec["wasm_hash"]
        object_path = home / "bin" / (spec["wasm_hash"] + ".wasm")
        object_path.parent.mkdir(parents=True, exist_ok=True)
        object_path.write_text(spec["wasm_payload"], encoding="utf-8")
    (target / "meta.json").write_text(json.dumps(meta, indent=2), encoding="utf-8")
    if is_live and spec.get("mutate_env"):
        env_path = home / "home/default/.config/env" / (capsule_id + ".env.json")
        env_path.write_text('{"model":"mutated"}\n', encoding="utf-8")
    if is_live and os.environ.get("FAKE_FAIL_LIVE_INSTALL") == "1":
        print("injected live install failure", file=sys.stderr)
        raise SystemExit(71)
    if backup.exists():
        shutil.rmtree(backup)
    raise SystemExit(0)
if args == ["--format", "json", "status"]:
    capsule_root = home / "home/default/.local/capsules"
    loaded = sorted(
        path.name
        for path in capsule_root.iterdir()
        if path.is_dir() and not path.name.endswith(".bak")
    )
    print(json.dumps({"status": {"loaded_capsules": loaded}}))
    raise SystemExit(0)
print("unexpected fake astrid arguments: " + repr(args), file=sys.stderr)
raise SystemExit(2)
'''


FAKE_SYSTEMCTL = r'''#!/usr/bin/env python3
import os
import pathlib
import sys

args = sys.argv[1:]
if args and args[0] == "--user":
    args = args[1:]
state = pathlib.Path(os.environ["FAKE_SYSTEMD_STATE"])
state.mkdir(parents=True, exist_ok=True)
active_path = state / "active"
pid_path = state / "pid"
restart_path = state / "restarts"
health_bad = state / "health-bad"
active = active_path.read_text().strip() if active_path.exists() else "0"
pid = int(pid_path.read_text()) if pid_path.exists() else 100
restarts = restart_path.read_text().strip() if restart_path.exists() else "0"

def save(new_active, new_pid=pid):
    active_path.write_text(str(new_active) + "\n")
    pid_path.write_text(str(new_pid) + "\n")

command = args[0] if args else ""
if command == "is-active":
    if health_bad.exists():
        raise SystemExit(3)
    raise SystemExit(0 if active == "1" else 3)
if command == "show":
    print("LoadState=loaded")
    print("ActiveState=" + ("active" if active == "1" else "inactive"))
    print("SubState=" + ("running" if active == "1" else "dead"))
    print("UnitFileState=enabled")
    print("MainPID=" + (str(pid) if active == "1" else "0"))
    print("NRestarts=" + restarts)
    print("ExecMainStartTimestampMonotonic=" + str(pid * 1000))
    print("FragmentPath=/fixture/astrid.service")
    print("DropInPaths=/fixture/astrid.service.d/profile.conf")
    raise SystemExit(0)
if command == "restart":
    if os.environ.get("FAKE_FAIL_RESTART") == "1":
        raise SystemExit(73)
    new_pid = pid if os.environ.get("FAKE_RESTART_SAME_PID") == "1" else pid + 1
    save(1, new_pid)
    if os.environ.get("FAKE_BUMP_RESTARTS") == "1":
        restart_path.write_text(str(int(restarts) + 1) + "\n")
    if os.environ.get("FAKE_HEALTH_FAIL") == "1":
        health_bad.touch()
    raise SystemExit(0)
if command == "stop":
    save(0)
    raise SystemExit(0)
if command == "start":
    health_bad.unlink(missing_ok=True)
    save(1, pid + 1)
    raise SystemExit(0)
print("unexpected fake systemctl arguments: " + repr(args), file=sys.stderr)
raise SystemExit(2)
'''


class InstallerTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.home = self.root / "home"
        self.home.mkdir()
        self.bin_dir = self.root / "bin"
        self.bin_dir.mkdir()
        self.astrid = self.bin_dir / "astrid"
        self.systemctl = self.bin_dir / "systemctl"
        self.astrid.write_text(FAKE_ASTRID, encoding="utf-8")
        self.systemctl.write_text(FAKE_SYSTEMCTL, encoding="utf-8")
        self.astrid.chmod(0o755)
        self.systemctl.chmod(0o755)
        self.state = self.root / "systemd"
        self.state.mkdir()
        (self.state / "active").write_text("1\n", encoding="utf-8")
        (self.state / "pid").write_text("100\n", encoding="utf-8")
        (self.state / "restarts").write_text("0\n", encoding="utf-8")
        self.astrid_home = self.home / ".astrid"
        self.capsule_root = (
            self.astrid_home / "home/default/.local/capsules"
        )
        self.env_root = self.astrid_home / "home/default/.config/env"
        self.capsule_root.mkdir(parents=True)
        self.env_root.mkdir(parents=True)
        self._write_installed("base-capsule", "base")
        self._write_installed("astrid-capsule-react", "prior")
        self.react_env = self.env_root / "astrid-capsule-react.env.json"
        self.react_env.write_text('{"model":"prior"}\n', encoding="utf-8")
        self.react_env.chmod(0o640)
        self.archive = self.root / "react.capsule"
        self.archive.write_text(
            json.dumps({"id": "astrid-capsule-react", "payload": "new"}),
            encoding="utf-8",
        )
        self.new_env = self.root / "react.env.json"
        self.new_env.write_text('{"model":"new"}\n', encoding="utf-8")
        self.current_manifest = (
            self.astrid_home
            / "etc/install-manifests/headless-application-capsules.current.json"
        )
        self.current_manifest.parent.mkdir(parents=True)
        self.current_manifest.write_text("prior-current\n", encoding="utf-8")

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def _write_installed(self, capsule_id: str, payload: str) -> None:
        target = self.capsule_root / capsule_id
        target.mkdir()
        (target / "Capsule.toml").write_text(
            f'[package]\nname = "{capsule_id}"\nversion = "0.1.0"\n',
            encoding="utf-8",
        )
        (target / "payload.txt").write_text(payload + "\n", encoding="utf-8")
        (target / "meta.json").write_text(
            json.dumps(
                {
                    "version": "0.1.0",
                    "installed_at": "2026-01-01T00:00:00Z",
                    "updated_at": "2026-01-01T00:00:00Z",
                    "imports": {},
                    "exports": {},
                    "topics": [],
                    "wit_files": {},
                }
            ),
            encoding="utf-8",
        )

    def _environment(self, **overrides: str) -> dict[str, str]:
        environment = os.environ.copy()
        environment.update(
            {
                "HOME": str(self.home),
                "PATH": str(self.bin_dir) + os.pathsep + environment["PATH"],
                "FAKE_SYSTEMD_STATE": str(self.state),
                "FAKE_LIVE_HOME": str(self.astrid_home),
            }
        )
        environment.update(overrides)
        return environment

    def _run(
        self,
        *extra: str,
        environment: dict[str, str] | None = None,
    ) -> subprocess.CompletedProcess[str]:
        command = [
            "python3",
            str(INSTALLER),
            "--astrid-bin",
            str(self.astrid),
            "--capsule",
            str(self.archive),
            *extra,
        ]
        return subprocess.run(
            command,
            env=environment or self._environment(),
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )

    def _assert_prior_restored(self) -> None:
        payload = (
            self.capsule_root / "astrid-capsule-react/payload.txt"
        ).read_text(encoding="utf-8")
        self.assertEqual(payload, "prior\n")
        self.assertEqual(self.react_env.read_text(encoding="utf-8"), '{"model":"prior"}\n')
        self.assertEqual(stat.S_IMODE(self.react_env.stat().st_mode), 0o640)
        self.assertEqual(self.current_manifest.read_text(encoding="utf-8"), "prior-current\n")
        pending = list(
            (self.astrid_home / ".install-transactions").glob(
                "headless-application-capsules-*"
            )
        )
        self.assertEqual(pending, [])

    def test_dry_run_performs_isolated_preflight_without_live_mutation(self) -> None:
        result = self._run(
            "--env",
            f"astrid-capsule-react={self.new_env}",
            "--restart",
            "--expected-total",
            "2",
            "--dry-run",
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Dry-run preflight passed", result.stdout)
        self.assertIn("stable PID, NRestarts", result.stdout)
        self.assertEqual(
            (self.capsule_root / "astrid-capsule-react/payload.txt").read_text(),
            "prior\n",
        )
        self.assertFalse((self.astrid_home / ".install-transactions").exists())

    def test_icp_dry_run_targets_ssd_layout_without_live_mutation(self) -> None:
        result = self._run("--layout", "icp-ssd", "--dry-run")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn(".astrid-icp/state/.install-transactions", result.stdout)
        self.assertIn("resolve exactly to /media/data/astrid", result.stdout)
        self.assertFalse((self.home / ".astrid-icp").exists())

    def test_success_commits_owner_only_hashed_generation(self) -> None:
        result = self._run(
            "--env",
            f"astrid-capsule-react={self.new_env}",
            "--restart",
            "--expected-total",
            "2",
            "--health-attempts",
            "1",
            "--health-stability-seconds",
            "0",
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Committed verified headless application-capsule generation", result.stdout)
        self.assertEqual(
            (self.capsule_root / "astrid-capsule-react/payload.txt").read_text(),
            "new\n",
        )
        self.assertEqual(self.react_env.read_text(), '{"model":"new"}\n')
        self.assertEqual(stat.S_IMODE(self.react_env.stat().st_mode), 0o600)
        current = json.loads(self.current_manifest.read_text(encoding="utf-8"))
        self.assertEqual(
            current["schema"], "astrid_headless_application_capsule_generation_v1"
        )
        self.assertEqual(current["capsules"][0]["capsule_id"], "astrid-capsule-react")
        self.assertTrue(current["service"]["restart_requested"])
        self.assertEqual(current["service"]["expected_loaded_capsules"], 2)
        history = list(
            (self.current_manifest.parent / "headless-application-capsules").glob("*.json")
        )
        self.assertEqual(len(history), 1)
        for path in (
            self.current_manifest,
            self.current_manifest.with_suffix(".sha256"),
            history[0],
            history[0].with_suffix(".sha256"),
        ):
            self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o600)
        self.assertEqual((self.state / "pid").read_text().strip(), "101")

    def test_operator_env_is_present_during_preflight_and_live_lifecycle(self) -> None:
        self.archive.write_text(
            json.dumps(
                {
                    "id": "astrid-capsule-react",
                    "payload": "new",
                    "required_env_value": "new",
                }
            ),
            encoding="utf-8",
        )
        result = self._run(
            "--env",
            f"astrid-capsule-react={self.new_env}",
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            (self.capsule_root / "astrid-capsule-react/payload.txt").read_text(),
            "new\n",
        )

    def test_lifecycle_cannot_silently_replace_operator_environment(self) -> None:
        self.archive.write_text(
            json.dumps(
                {
                    "id": "astrid-capsule-react",
                    "payload": "new",
                    "mutate_env": True,
                }
            ),
            encoding="utf-8",
        )
        result = self._run(
            "--env",
            f"astrid-capsule-react={self.new_env}",
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("changed the operator-selected environment", result.stderr)
        self._assert_prior_restored()

    def test_live_install_failure_restores_exact_paths_without_restart(self) -> None:
        result = self._run(
            "--env",
            f"astrid-capsule-react={self.new_env}",
            environment=self._environment(FAKE_FAIL_LIVE_INSTALL="1"),
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("injected live install failure", result.stderr)
        self._assert_prior_restored()
        self.assertEqual((self.state / "pid").read_text().strip(), "100")
        self.assertEqual((self.state / "active").read_text().strip(), "1")

    def test_failed_service_health_rolls_back_and_restores_active_state(self) -> None:
        result = self._run(
            "--env",
            f"astrid-capsule-react={self.new_env}",
            "--restart",
            "--expected-total",
            "2",
            "--health-attempts",
            "1",
            "--health-stability-seconds",
            "0",
            environment=self._environment(FAKE_HEALTH_FAIL="1"),
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("failed capsule health verification", result.stderr)
        self._assert_prior_restored()
        self.assertEqual((self.state / "active").read_text().strip(), "1")
        self.assertGreater(int((self.state / "pid").read_text()), 101)

    def test_rollback_restores_referenced_content_object_exactly(self) -> None:
        digest = "a" * 64
        object_path = self.astrid_home / "bin" / f"{digest}.wasm"
        object_path.parent.mkdir(parents=True)
        object_path.write_text("prior-wasm", encoding="utf-8")
        object_path.chmod(0o640)
        self.archive.write_text(
            json.dumps(
                {
                    "id": "astrid-capsule-react",
                    "payload": "new",
                    "wasm_hash": digest,
                    "wasm_payload": "new-wasm",
                }
            ),
            encoding="utf-8",
        )
        result = self._run(environment=self._environment(FAKE_FAIL_LIVE_INSTALL="1"))
        self.assertNotEqual(result.returncode, 0)
        self._assert_prior_restored()
        self.assertEqual(object_path.read_text(encoding="utf-8"), "prior-wasm")
        self.assertEqual(stat.S_IMODE(object_path.stat().st_mode), 0o640)

    def test_restart_must_change_pid_without_incrementing_nrestarts(self) -> None:
        same_pid = self._run(
            "--restart",
            "--expected-total",
            "2",
            "--health-attempts",
            "1",
            "--health-stability-seconds",
            "0",
            environment=self._environment(FAKE_RESTART_SAME_PID="1"),
        )
        self.assertNotEqual(same_pid.returncode, 0)
        self.assertIn("MainPID did not change", same_pid.stderr)
        self._assert_prior_restored()

        bumped = self._run(
            "--restart",
            "--expected-total",
            "2",
            "--health-attempts",
            "1",
            "--health-stability-seconds",
            "0",
            environment=self._environment(FAKE_BUMP_RESTARTS="1"),
        )
        self.assertNotEqual(bumped.returncode, 0)
        self.assertIn("unexpected restart", bumped.stderr)
        self._assert_prior_restored()

    def test_failed_service_health_keeps_previously_inactive_service_stopped(self) -> None:
        (self.state / "active").write_text("0\n", encoding="utf-8")
        result = self._run(
            "--restart",
            "--expected-total",
            "2",
            "--health-attempts",
            "1",
            "--health-stability-seconds",
            "0",
            environment=self._environment(FAKE_HEALTH_FAIL="1"),
        )
        self.assertNotEqual(result.returncode, 0)
        self._assert_prior_restored()
        self.assertEqual((self.state / "active").read_text().strip(), "0")

    def test_omitted_env_preserves_content_and_records_owner_only_mode(self) -> None:
        result = self._run()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.react_env.read_text(), '{"model":"prior"}\n')
        self.assertEqual(stat.S_IMODE(self.react_env.stat().st_mode), 0o600)
        current = json.loads(self.current_manifest.read_text(encoding="utf-8"))
        self.assertEqual(
            current["capsules"][0]["environment"]["source"],
            "preserved_or_capsule_default",
        )
        self.assertFalse(current["service"]["restart_requested"])

    def test_unknown_env_and_duplicate_capsule_ids_fail_closed(self) -> None:
        unverifiable_total = self._run("--expected-total", "2", "--dry-run")
        self.assertNotEqual(unverifiable_total.returncode, 0)
        self.assertIn("requires --restart", unverifiable_total.stderr)
        unknown = self._run(
            "--env",
            f"different-capsule={self.new_env}",
            "--dry-run",
        )
        self.assertNotEqual(unknown.returncode, 0)
        self.assertIn("absent from this transaction", unknown.stderr)
        duplicate = self._run(
            "--capsule",
            str(self.archive),
            "--dry-run",
        )
        self.assertNotEqual(duplicate.returncode, 0)
        self.assertIn("same capsule identifier", duplicate.stderr)

    def test_symlink_archive_and_stale_transaction_are_rejected(self) -> None:
        linked_archive = self.root / "linked.capsule"
        linked_archive.symlink_to(self.archive)
        result = subprocess.run(
            [
                "python3",
                str(INSTALLER),
                "--astrid-bin",
                str(self.astrid),
                "--capsule",
                str(linked_archive),
                "--dry-run",
            ],
            env=self._environment(),
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("non-symlink", result.stderr)

        transaction_parent = self.astrid_home / ".install-transactions"
        transaction_parent.mkdir(parents=True)
        stale = transaction_parent / "headless-application-capsules-stale"
        stale.mkdir()
        stale_result = self._run()
        self.assertNotEqual(stale_result.returncode, 0)
        self.assertIn("pending CPU-edge transaction", stale_result.stderr)
        self.assertEqual(
            (self.capsule_root / "astrid-capsule-react/payload.txt").read_text(),
            "prior\n",
        )


if __name__ == "__main__":
    unittest.main()
