#!/usr/bin/env python3
"""Owner-held provider environment activation; check-only is strictly read-only.

The first launchd configuration migration requires a successful host fixture.
No forced termination, in-place package install, or runtime-state rollback exists.
"""
from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import plistlib
import re
import shlex
import subprocess
import sys
import time
import urllib.error
import urllib.request
import uuid

import model_environment as env


LABEL = "com.reservoir.coupled-astrid"
ASTRID = Path(__file__).resolve().parents[1]
MODEL_ROOT = Path("/Users/v/other/neural-triple-reservoir")


def healthy_readiness(health: dict, *, idle: bool = False) -> bool:
    worker = health.get("worker", {})
    phases = {"ready"} if idle else {"ready", "generating"}
    return (health.get("ready") is True and not health.get("error")
            and worker.get("phase") in phases and worker.get("last_generation_error") is None
            and (not idle or worker.get("queue_depth") == 0)
            and health.get("reservoir", {}).get("status") == "connected")


def qualified_process(qualification: dict, pid: int, started_at: str) -> dict:
    """A relevant provider restart invalidates its accepted observation window."""
    predecessor = qualification.get("predecessor_acceptance") or {}
    if predecessor:
        expected = predecessor.get("provider", {})
        matches = expected.get("pid") == pid and expected.get("started_at") == started_at
    else:
        from model_qualification import load_reference
        observed = load_reference(qualification.get("accepted_evidence", {}).get("passive_baseline"))
        provider = observed["samples"][0]["provider"]
        identity = " ".join(provider.get("identity", {}).get("stdout", "").split())
        matches = (provider.get("pids") == [pid]
                   and identity.startswith(f"{pid} {' '.join(started_at.split())} "))
    if not matches:
        raise ValueError("provider changed since accepted observation; repeat the baseline before promotion")
    return {"pid": pid, "started_at": started_at}


def run(*args: str, check=True):
    return subprocess.run(args, capture_output=True, text=True, check=check, timeout=30)


def verify_fixture(path: Path) -> dict:
    proof = env.read_json(path)
    fixture = Path(__file__).with_name("qualify_model_launchd_handoff.py")
    executable = Path(sys.executable).resolve()
    expected = {"schema": "model_launchd_handoff_fixture_v1", "passed": True,
                "debug_supported": True, "same_pid_candidate_exec": True,
                "rollback_same_pid_exec": True, "no_execution_during_hold": True,
                "old_work_finished_before_replacement": True,
                "launcher_sha256": env.digest(Path(env.__file__).resolve()),
                "fixture_sha256": env.digest(fixture), "uid": os.getuid(),
                "activation_sha256": env.digest(Path(__file__).resolve()),
                "hostname": os.uname().nodename, "python": str(executable),
                "python_sha256": env.digest(executable),
                "python_runtime_sha256": {str(path): env.digest(path) for path in env.interpreter_paths(executable)},
                "host_version": run("sw_vers", "-productVersion").stdout.strip(),
                "boot_session_uuid": run("sysctl", "-n", "kern.bootsessionuuid").stdout.strip()}
    failures = [name for name, value in expected.items() if proof.get(name) != value]
    if not str(proof.get("label", "")).startswith("com.codex.model-handoff-fixture."):
        failures.append("fixture_label")
    if proof.get("cleanup", {}).get("service_absent") is not True:
        failures.append("fixture_cleanup")
    if failures:
        raise RuntimeError("host handoff fixture not qualified: " + ", ".join(failures))
    return proof


def transition(backend, *, expected_pid: int, actor: str, ack: str, migrate: bool,
               rollback: bool = False, timeout: float = 1500,
               now=time.monotonic, sleep=time.sleep) -> dict:
    if (expected_pid <= 1 and not (rollback and migrate and expected_pid == 0)
            or not actor.strip() or not ack.strip() or not 1 <= timeout <= 1800):
        raise ValueError("transition requires exact PID, actor, acknowledgement and bounded timeout")
    initial = backend.inspect(expected_pid, migrate=migrate)
    receipt = {"schema": "model_environment_transition_v1", "actor": actor,
               "acknowledgement_sha256": hashlib.sha256(ack.encode()).hexdigest(),
               "old_pid": expected_pid, "old_identity": initial,
               "migration": migrate, "rollback": rollback, "force_used": False,
               "runtime_state_rollback": False, "atomic_traffic_quiescence_claimed": False,
               "status": "preparing", "signal_sent": False}
    backend.begin(receipt)
    try:
        already_held = initial.get("already_held") is True
        missing_job = initial.get("interrupted_migration_job_absent") is True
        if not already_held and not missing_job:
            backend.pre_stack_receipt()
            backend.hold()
        if migrate and not already_held and not missing_job:
            # The durable receipt precedes arming one-shot launchd state.
            receipt["debug_override_attempted"] = True
            backend.record(receipt)
            backend.arm_next_spawn()
            receipt["debug_override_armed"] = True
            backend.record(receipt)
        # Full wheel/model rehashing can take time: finish it before observing
        # the final idle window, never between an idle observation and signal.
        backend.revalidate()
        if missing_job:
            backend.restore_held_job()
        elif not already_held:
            deadline = now() + timeout
            while True:
                backend.assert_old(expected_pid, initial)
                if backend.idle():
                    backend.assert_old(expected_pid, initial)
                    if backend.idle():
                        break
                if now() >= deadline:
                    raise RuntimeError("no idle empty-queue window; no signal sent; hold retained")
                sleep(.25)
            backend.assert_old(expected_pid, initial)
            backend.signal()
            receipt["signal_sent"] = True
            backend.record(receipt)
            deadline = now() + timeout
            while backend.old_exists(expected_pid, initial):
                if now() >= deadline:
                    raise RuntimeError("old provider did not exit; no forced termination; hold retained")
                sleep(.25)
        held = backend.wait_held(timeout)
        receipt.update(old_process_exited=True, held_process=held)
        backend.record(receipt)
        if migrate and not missing_job:
            backend.migrate_held(held)
            held = backend.wait_held(timeout)
            receipt["permanent_held_process"] = held
            backend.record(receipt)
        backend.select()
        receipt["candidate_selected"] = True
        backend.record(receipt)
        backend.release()
        receipt["hold_released"] = True
        backend.record(receipt)
        receipt["new_process"] = backend.verify_ready(held, timeout)
        backend.publish()
        backend.post_stack_receipt()
        receipt["status"] = "activated_verified"
        backend.record(receipt)
        return receipt
    except BaseException as error:
        receipt.update(status="failed_requires_review", error=str(error)[:2000])
        try:
            backend.retain_hold()
        except Exception as hold_error:
            receipt["hold_recovery_error"] = str(hold_error)[:1000]
        backend.record(receipt)
        raise


class LaunchdBackend:
    def __init__(self, *, control_root: Path, manifest: Path, baseline: Path,
                 fixture_proof: Path, actor: str, ack: str, resume_transaction: Path | None = None,
                 qualification_proof: Path | None = None, rollback: bool = False,
                 sudo_debug: bool = False, qualification_baseline: Path | None = None):
        self.root = control_root.absolute()
        if self.root.parent.resolve() / self.root.name != self.root:
            raise ValueError("control root must be a canonical path")
        self.manifest, self.baseline, self.fixture_proof = manifest.absolute(), baseline.absolute(), fixture_proof.absolute()
        self.actor, self.ack = actor, ack
        self.target, self.domain = f"gui/{os.getuid()}/{LABEL}", f"gui/{os.getuid()}"
        self.launcher = Path(env.__file__).resolve()
        self.launcher_python = Path(sys.executable).resolve()
        self.source_plist = MODEL_ROOT / "launchd" / f"{LABEL}.plist"
        self.installed_plist = Path.home() / "Library/LaunchAgents" / f"{LABEL}.plist"
        self.deployment_manifest = ASTRID / "capsules/spectral-bridge/workspace/deployment_manifests/coupled-model.json"
        self.transaction_id = uuid.uuid4().hex
        self.transaction = self.root / "transactions" / self.transaction_id
        self.guard = {"schema": env.HOLD_SCHEMA, "transaction_id": self.transaction_id, "actor": actor}
        self.checked = None
        self.initial = None
        self.migrate = False
        self.resume_transaction = resume_transaction
        self.qualification_proof = qualification_proof.absolute() if qualification_proof is not None else None
        self.qualification_baseline = (qualification_baseline or baseline).absolute()
        self.rollback, self.sudo_debug = rollback, sudo_debug
        self.current_receipt = None

    def qualification(self):
        if self.rollback:
            if self.manifest != self.baseline:
                raise ValueError("rollback qualification exemption requires the preserved baseline")
            return {"exempt": "explicit_preserved_baseline_rollback",
                    "baseline": self.rollback_provenance()}
        if self.qualification_proof is None:
            raise ValueError("candidate activation requires accepted qualification proof")
        from model_qualification import verify_proof
        proof = verify_proof(self.qualification_proof, self.manifest, self.qualification_baseline)
        return {**proof, "qualification_baseline_manifest": str(self.qualification_baseline),
                "rollback_manifest_sha256": env.digest(self.baseline)}

    def accepted_selection(self):
        selected = env.read_json(self.root / "active.json")
        if selected.get("schema") != env.SELECTION_SCHEMA:
            raise ValueError("active environment selection is not valid")
        records = list((self.root / "transactions").glob("*/receipt.json"))
        records += list((self.root / "transactions").glob("*/recoveries/*/receipt.json"))
        for path in sorted(records, reverse=True):
            receipt = env.read_json(path)
            if (receipt.get("status") == "activated_verified"
                    and receipt.get("manifest") == selected.get("manifest")
                    and receipt.get("new_process", {}).get("manifest_sha256") == selected.get("manifest_sha256")
                    and receipt.get("force_used") is False and receipt.get("runtime_state_rollback") is False):
                return {"selection": selected, "receipt": receipt,
                        "receipt_path": str(path), "receipt_sha256": env.digest(path)}
        raise ValueError("active selection has no matching verified activation receipt")

    def rollback_provenance(self):
        anchor = env.read_json(self.root / "rollback-baseline.json")
        target = self.baseline_anchor()
        if self.resume_transaction is not None:
            previous = self.resume_transaction.resolve(strict=True)
            if previous.parent != self.root / "transactions":
                raise ValueError("rollback recovery transaction is outside this control root")
            receipt = env.read_json(previous / "receipt.json")
            if (receipt.get("status") != "failed_requires_review"
                    or receipt.get("baseline_manifest") != str(self.baseline)
                    or receipt.get("old_identity", {}).get("baseline_manifest_sha256") != target["manifest_sha256"]):
                raise ValueError("rollback target differs from the failed transaction's preserved predecessor")
            return {"target": target, "failed_transaction": str(previous),
                    "receipt_sha256": env.digest(previous / "receipt.json")}
        if anchor == target:
            return {"target": target, "original": True}
        accepted = self.accepted_selection()
        prior = accepted["receipt"]
        if (prior.get("baseline_manifest") != str(self.baseline)
                or prior.get("old_identity", {}).get("baseline_manifest_sha256") != target["manifest_sha256"]):
            raise ValueError("rollback must match the original transaction baseline or the accepted predecessor")
        return {"target": target, "accepted_transaction": accepted["receipt_path"],
                "receipt_sha256": accepted["receipt_sha256"]}

    def verify_baseline_equivalence(self):
        original = env.read_json(self.baseline)
        tested = env.read_json(self.qualification_baseline)
        for key in ("argv", "working_directory", "runtime_paths"):
            if tested.get(key) != original.get(key):
                raise ValueError("tested baseline settings differ from preserved rollback environment")
        inventories = [env.read_json(Path(item["package_inventory"])) for item in (original, tested)]
        if inventories[0].get("packages") != inventories[1].get("packages"):
            raise ValueError("tested baseline packages differ from preserved rollback environment")
        closures = []
        for manifest in (original, tested):
            source = Path(manifest["source_root"])
            closures.append({str(Path(path).relative_to(source)): fingerprint
                             for path, fingerprint in manifest["artifacts"].items()
                             if Path(path).is_relative_to(source) and Path(path).suffix == ".py"})
        if not closures[0] or closures[0] != closures[1]:
            raise ValueError("tested baseline source differs from preserved rollback environment")

    def baseline_anchor(self):
        return {"schema": "model_environment_rollback_baseline_v1", "manifest": str(self.baseline),
                "manifest_sha256": env.digest(self.baseline)}

    def require_predecessor_acceptance(self, qualification, original_anchor, baseline_hash):
        if original_anchor == self.baseline_anchor():
            return  # A qualified core retry may follow an explicit original rollback.
        predecessor = (qualification.get("predecessor_acceptance") or {}).get("manifest")
        if predecessor != {"path": str(self.baseline), "sha256": baseline_hash}:
            raise ValueError("later activation requires accepted predecessor proof and postobservations")

    def validate_privilege_boundary(self, *, migrate):
        if os.geteuid() == 0:
            raise ValueError("run as the ordinary desktop user, never sudo the whole controller")
        if self.sudo_debug and not migrate:
            raise ValueError("--sudo-debug applies only to the first launcher migration")
        if self.sudo_debug and not sys.stdin.isatty():
            raise ValueError("--sudo-debug requires an interactive Terminal; credentials are never piped")

    def launcher_arguments(self):
        return [str(self.launcher_python), str(self.launcher), "launch", "--control-root", str(self.root)]

    def job(self):
        return run("launchctl", "print", self.target).stdout

    def job_loaded(self):
        result = run("launchctl", "print", self.target, check=False)
        if result.returncode == 0:
            return True
        absent = f'Could not find service "{LABEL}" in domain for user gui: {os.getuid()}'
        if result.returncode != 113 or absent not in result.stderr:
            raise RuntimeError("cannot prove the production launchd job is absent")
        return False

    def pid(self):
        text = run("launchctl", "print", self.target, check=False).stdout
        match = re.search(r"^\s*pid = (\d+)\s*$", text, re.MULTILINE)
        return int(match[1]) if match else None

    def loaded_arguments(self):
        match = re.search(r"^\targuments = \{\n(.*?)^\t\}", self.job(), re.M | re.S)
        return [] if match is None else [line.strip() for line in match[1].splitlines()]

    def preflight(self):
        result = run(sys.executable, "-B", str(ASTRID / "scripts/deploy_preflight.py"),
                     "--component", "model", "--repo", str(MODEL_ROOT), "--ack", self.ack, "--json")
        if json.loads(result.stdout).get("ok") is not True:
            raise RuntimeError("cooperative model preflight failed")
        state = json.loads(run(sys.executable, "-B", str(ASTRID / "scripts/steward_control.py"),
                               "--json", "status").stdout)
        if state.get("state", {}).get("paused") is not True or state.get("lease") is not None:
            raise RuntimeError("model transition requires paused stewardship and no active lease")

    def inspect(self, expected_pid, *, migrate):
        # The host gate runs before any hold, receipt, plist, or service mutation.
        verify_fixture(self.fixture_proof)
        self.validate_privilege_boundary(migrate=migrate)
        qualification = self.qualification()
        self.preflight()
        self.checked = env.validate_manifest(self.manifest)
        baseline = env.validate_manifest(self.baseline)
        anchor_path = self.root / "rollback-baseline.json"
        original_anchor = None
        if anchor_path.exists():
            original_anchor = env.read_json(anchor_path)
        elif not migrate:
            raise RuntimeError("first migration must establish the preserved rollback anchor")
        if not self.rollback:
            self.verify_baseline_equivalence()
            if not migrate:
                accepted = self.accepted_selection()
                if (accepted["selection"].get("manifest") != str(self.baseline)
                        or accepted["selection"].get("manifest_sha256") != baseline["manifest_sha256"]):
                    raise ValueError("later activation must preserve the current accepted environment as rollback")
                self.require_predecessor_acceptance(qualification, original_anchor, baseline["manifest_sha256"])
        candidate = self.checked["manifest"]
        if candidate["working_directory"] != str(MODEL_ROOT):
            raise RuntimeError("production provider must retain canonical runtime state directory")
        if any(candidate[key] != baseline["manifest"][key]
               for key in ("argv", "working_directory", "runtime_paths")):
            raise RuntimeError("environment transition cannot change model, controls, or runtime-state paths")
        recovery = None
        existing_hold = env.hold_record(self.root)
        if self.resume_transaction is not None:
            previous = self.resume_transaction.resolve(strict=True)
            if previous.parent != self.root / "transactions":
                raise RuntimeError("recovery transaction is outside this control root")
            recovery = env.read_json(previous / "receipt.json")
            if (recovery.get("status") != "failed_requires_review"
                    or recovery.get("transaction_id") != previous.name
                    or recovery.get("baseline_manifest") != str(self.baseline)
                    or self.manifest != self.baseline):
                raise RuntimeError("recovery must explicitly select the failed transaction's baseline")
            self.guard = {"schema": env.HOLD_SCHEMA, "transaction_id": previous.name,
                          "actor": recovery["actor"]}
            env.assert_hold(self.root, self.guard)
            self.transaction = previous / "recoveries" / self.transaction_id
        elif existing_hold is not None:
            raise RuntimeError("existing model hold requires explicit recovery; not overwritten")
        if expected_pid == 0:
            if not self.rollback or not migrate or recovery is None:
                raise ValueError("PID 0 requires explicit interrupted-migration rollback recovery")
            settings = self.inspect_absent_migration(recovery)
            self.migrate = True
            self.initial = {"pid": 0, "started_at": None, "interrupted_migration_job_absent": True,
                            "source_plist_sha256": env.digest(self.source_plist),
                            "installed_plist_sha256": env.digest(self.installed_plist),
                            "launcher_sha256": env.digest(self.launcher), "settings": settings,
                            "manifest_sha256": self.checked["manifest_sha256"],
                            "baseline_manifest_sha256": baseline["manifest_sha256"],
                            "fixture_sha256": env.digest(self.fixture_proof), "qualification": qualification,
                            "original_baseline_anchor": original_anchor,
                            "recovery_of": str(self.resume_transaction.resolve()),
                            "prior_failure_sha256": env.digest(self.resume_transaction / "receipt.json")}
            return self.initial
        settings = plistlib.loads(self.source_plist.read_bytes())
        if settings != plistlib.loads(self.installed_plist.read_bytes()) or settings.get("KeepAlive") is not True:
            raise RuntimeError("source and installed KeepAlive configuration differ")
        if self.loaded_arguments() != settings.get("ProgramArguments"):
            raise RuntimeError("loaded launchd arguments differ from installed configuration")
        if not migrate and settings["ProgramArguments"] != self.launcher_arguments():
            raise RuntimeError("first migration is required before environment selection")
        if migrate and settings["ProgramArguments"] == self.launcher_arguments():
            raise RuntimeError("service already uses environment launcher; omit migration")
        if migrate and settings["ProgramArguments"][0] != baseline["manifest"]["python"]:
            raise RuntimeError("first migration must preserve the original provider interpreter for rollback")
        start = env.process_start(expected_pid)
        if self.pid() != expected_pid or not start:
            raise RuntimeError("provider PID/start changed")
        self.migrate = migrate
        self.initial = {"pid": expected_pid, "started_at": start,
                        "source_plist_sha256": env.digest(self.source_plist),
                        "installed_plist_sha256": env.digest(self.installed_plist),
                        "launcher_sha256": env.digest(self.launcher),
                        "manifest_sha256": self.checked["manifest_sha256"],
                        "baseline_manifest_sha256": baseline["manifest_sha256"],
                        "fixture_sha256": env.digest(self.fixture_proof),
                        "qualification": qualification, "sudo_debug_requested": self.sudo_debug,
                        "original_baseline_anchor": original_anchor,
                        "settings": settings}
        if not self.rollback:
            self.initial["qualified_process"] = qualified_process(qualification, expected_pid, start)
        if recovery is not None:
            self.initial["recovery_of"] = str(self.resume_transaction.resolve())
            self.initial["prior_failure_sha256"] = env.digest(self.resume_transaction / "receipt.json")
            self.initial["already_held"] = not self.listeners() and self.held(allow_running=True) is not None
        if not migrate and not self.initial.get("already_held"):
            active = env.selected_release(self.root)
            command = shlex.split(run("ps", "-o", "command=", "-p", str(expected_pid)).stdout.strip())
            if not env.command_matches(command, active["command"]):
                raise RuntimeError("running provider does not match the current environment selection")
        return self.initial

    def inspect_absent_migration(self, recovery):
        env.assert_hold(self.root, self.guard)
        phases = {"bootout_pending", "held_job_removed", "source_plist_written",
                  "installed_plist_written", "bootstrap_pending"}
        if (not self.rollback or self.resume_transaction is None or recovery.get("migration") is not True
                or recovery.get("old_process_exited") is not True or recovery.get("migration_phase") not in phases):
            raise ValueError("receipt does not prove an interrupted owned launcher migration")
        held = recovery.get("held_process", {})
        old = recovery.get("old_identity", {})
        if held.get("hold_only") is not True or held.get("hold") != self.guard:
            raise ValueError("receipt is missing the exact one-shot held process ownership")
        for identity in (old, held):
            pid = identity.get("pid")
            if type(pid) is not int or pid <= 1 or not identity.get("started_at"):
                raise ValueError("interrupted migration process identity is missing")
            if env.process_start(pid) is not None:
                raise RuntimeError("a previous migration PID still exists; no absent-job recovery")
        if self.job_loaded() or self.listeners():
            raise RuntimeError("absent-job recovery requires no launchd job and no model listener")
        settings = old.get("settings")
        if (not isinstance(settings, dict) or settings.get("Label") != LABEL or settings.get("KeepAlive") is not True
                or settings.get("ProgramArguments", [])[:1] != [self.checked["manifest"]["python"]]):
            raise ValueError("interrupted migration lacks the original direct baseline settings")
        permanent = dict(settings, ProgramArguments=self.launcher_arguments())
        original = []
        for name, key in (("previous-source.plist", "source_plist_sha256"),
                          ("previous-installed.plist", "installed_plist_sha256")):
            saved = self.resume_transaction / name
            if env.digest(saved) != old.get(key) or plistlib.loads(saved.read_bytes()) != settings:
                raise ValueError("preserved original launch configuration changed")
            original.append(saved.read_bytes())
        for path, before in zip((self.source_plist, self.installed_plist), original):
            if path.read_bytes() not in (before, plistlib.dumps(permanent)):
                raise ValueError("interrupted migration plist is neither exact original nor owned permanent configuration")
        return settings

    def begin(self, receipt):
        self.transaction.mkdir(parents=True, mode=0o700)
        anchor = self.initial["original_baseline_anchor"] or self.baseline_anchor()
        anchor_path = self.root / "rollback-baseline.json"
        try:
            with anchor_path.open("x") as handle:
                json.dump(anchor, handle, sort_keys=True)
                handle.write("\n")
                handle.flush()
                os.fsync(handle.fileno())
            env.sync_directory(self.root)
        except FileExistsError:
            if env.read_json(anchor_path) != anchor:
                raise RuntimeError("preserved rollback anchor changed before transition")
        for name, source in (("previous-source.plist", self.source_plist),
                             ("previous-installed.plist", self.installed_plist)):
            (self.transaction / name).write_bytes(source.read_bytes())
        if (self.root / "active.json").exists():
            env.atomic_json(self.transaction / "previous-selection.json", env.read_json(self.root / "active.json"))
        if self.deployment_manifest.exists():
            env.atomic_json(self.transaction / "previous-deployment-manifest.json",
                            env.read_json(self.deployment_manifest))
        self.record(receipt)

    def record(self, receipt):
        self.current_receipt = receipt
        env.atomic_json(self.transaction / "receipt.json", {**receipt,
                        "transaction_id": self.transaction_id, "manifest": str(self.manifest),
                        "baseline_manifest": str(self.baseline), "recorded_at": dt.datetime.now(dt.UTC).isoformat()})

    def stack_receipt(self, phase):
        run("bash", str(ASTRID / "scripts/capture_stack_receipt.sh"),
            "--actor", self.actor, "--ack", f"{self.ack}; model environment {phase}")

    def pre_stack_receipt(self):
        if self.resume_transaction is None and not self.rollback:
            self.stack_receipt("pre-transition")
        else:
            # A failed replacement may not satisfy readiness. Retain the actual
            # failed inventory while explicit rollback restores the baseline.
            result = run("bash", str(ASTRID / "scripts/capture_stack_receipt.sh"),
                         "--actor", self.actor, "--ack", f"{self.ack}; explicit failed-transition recovery inventory",
                         check=False)
            env.atomic_json(self.transaction / "recovery-pre-stack.json",
                            {"returncode": result.returncode, "stdout": result.stdout,
                             "stderr": result.stderr, "readiness_waived_for_explicit_rollback_only": True})

    def post_stack_receipt(self):
        self.stack_receipt("post-transition")

    def hold(self):
        if self.resume_transaction is not None:
            env.assert_hold(self.root, self.guard)
            return
        actual = env.create_hold(self.root, self.transaction_id, self.actor)
        if actual != self.guard:
            raise RuntimeError("unexpected hold identity")

    def retain_hold(self):
        if env.hold_record(self.root) is None:
            env.create_hold(self.root, self.guard["transaction_id"], self.guard["actor"])
        env.assert_hold(self.root, self.guard)

    def arm_next_spawn(self):
        env.assert_hold(self.root, self.guard)
        self.validate_privilege_boundary(migrate=self.migrate)
        if not self.migrate or self.target != f"gui/{os.getuid()}/{LABEL}":
            raise ValueError("next-spawn override requires the validated first production migration")
        # Admin-01 parsed the repeated interpreter token as a source file.
        # Local man-page argv0 wording differs; this script-first tail must
        # pass the owned host fixture before production can reach this call.
        command = ["/bin/launchctl", "debug", self.target, "--program", str(self.launcher_python), "--",
                   *self.launcher_arguments()[1:], "--hold-only", self.guard["transaction_id"]]
        if self.sudo_debug:
            # Only debug is privileged. The terminal owns password entry; Python
            # neither reads credentials nor captures sudo's terminal output.
            subprocess.run(["/usr/bin/sudo", "--", *command], check=True, timeout=180)
        else:
            run(*command)

    def health(self, route="readyz"):
        try:
            with urllib.request.urlopen(f"http://127.0.0.1:8090/{route}", timeout=3) as response:
                return json.load(response)
        except urllib.error.HTTPError as response:
            try:
                return json.load(response)
            except (OSError, ValueError):
                return {}
            finally:
                response.close()
        except (OSError, ValueError, urllib.error.URLError):
            return {}

    def idle(self):
        health = self.health()
        if self.rollback:
            worker = health.get("worker", {})
            return (health.get("model") == "coupled-astrid" and worker.get("phase") == "ready"
                    and type(worker.get("queue_depth")) is int and worker["queue_depth"] == 0
                    and self.initial is not None and self.listeners() == [str(self.initial["pid"])])
        return healthy_readiness(health, idle=True)

    def assert_old(self, expected_pid, initial):
        env.assert_hold(self.root, self.guard)
        if self.pid() != expected_pid or env.process_start(expected_pid) != initial["started_at"]:
            raise RuntimeError("provider identity changed; no signal sent")
        if not self.rollback and initial.get("qualified_process") != {
                "pid": expected_pid, "started_at": initial["started_at"]}:
            raise RuntimeError("provider is not the qualified observation process; no signal sent")

    def revalidate(self):
        self.preflight()
        verify_fixture(self.fixture_proof)
        if self.qualification() != self.initial["qualification"]:
            raise RuntimeError("qualification evidence changed; no signal sent")
        if env.read_json(self.root / "rollback-baseline.json") != (
                self.initial["original_baseline_anchor"] or self.baseline_anchor()):
            raise RuntimeError("original rollback anchor changed; no signal sent")
        checked = env.validate_manifest(self.manifest)
        baseline = env.validate_manifest(self.baseline)
        values = {"source_plist_sha256": env.digest(self.source_plist),
                  "installed_plist_sha256": env.digest(self.installed_plist),
                  "launcher_sha256": env.digest(self.launcher),
                  "manifest_sha256": checked["manifest_sha256"],
                  "baseline_manifest_sha256": baseline["manifest_sha256"]}
        if any(value != self.initial[name] for name, value in values.items()):
            raise RuntimeError("transition inputs changed; no signal sent")

    def signal(self):
        run("launchctl", "kill", "SIGTERM", self.target)

    def old_exists(self, expected_pid, initial):
        return env.process_start(expected_pid) == initial["started_at"]

    def listeners(self):
        return run("lsof", "-t", "-nP", "-iTCP:8090", "-sTCP:LISTEN", check=False).stdout.split()

    def held(self, *, allow_running=False):
        env.assert_hold(self.root, self.guard)
        current = self.pid()
        path = self.root / "held" / f"{current}.json"
        if current is None or not path.is_file():
            return None
        receipt = env.read_json(path)
        command = shlex.split(run("ps", "-o", "command=", "-p", str(current)).stdout.strip())
        expected_command = self.launcher_arguments()
        if receipt.get("hold_only"):
            expected_command += ["--hold-only", self.guard["transaction_id"]]
        if allow_running and not env.command_matches(command, expected_command):
            return None
        if (receipt.get("hold") != self.guard or receipt.get("pid") != current
                or receipt.get("started_at") != env.process_start(current)
                or receipt.get("launcher_sha256") != env.digest(self.launcher)
                or receipt.get("model_imported") is not False
                or not env.command_matches(command, expected_command) or self.listeners()):
            raise RuntimeError("replacement is not the verified inference-free held process")
        return receipt

    def wait_held(self, timeout):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            held = self.held()
            if held:
                return held
            time.sleep(.25)
        raise RuntimeError("held replacement did not appear; no forced termination")

    def migrate_held(self, receipt):
        if self.held() != receipt or not receipt.get("hold_only"):
            raise RuntimeError("bootout requires the exact one-shot held replacement")
        self.migration_checkpoint("bootout_pending")
        run("launchctl", "bootout", self.target)
        deadline = time.monotonic() + 10
        while env.process_start(receipt["pid"]) == receipt["started_at"] and time.monotonic() < deadline:
            time.sleep(.1)
        if env.process_start(receipt["pid"]) == receipt["started_at"] or self.listeners():
            raise RuntimeError("held process did not exit cleanly")
        self.migration_checkpoint("held_job_removed")
        self.install_held_launcher()

    def migration_checkpoint(self, phase):
        if self.current_receipt is None:
            raise RuntimeError("migration requires a durable owned transition receipt")
        self.current_receipt["migration_phase"] = phase
        self.record(self.current_receipt)

    def restore_held_job(self):
        if not self.initial.get("interrupted_migration_job_absent"):
            raise RuntimeError("no verified absent migration to recover")
        self.inspect_absent_migration(env.read_json(self.resume_transaction / "receipt.json"))
        self.install_held_launcher()

    def install_held_launcher(self):
        env.assert_hold(self.root, self.guard)
        if self.job_loaded() or self.listeners():
            raise RuntimeError("launcher installation requires no loaded job or model listener")
        for path, key in ((self.source_plist, "source_plist_sha256"),
                          (self.installed_plist, "installed_plist_sha256")):
            if env.digest(path) != self.initial[key]:
                raise RuntimeError("launch configuration changed before held installation")
        settings = dict(self.initial["settings"])
        settings["ProgramArguments"] = self.launcher_arguments()
        payload = plistlib.dumps(settings)
        for path in (self.source_plist, self.installed_plist):
            temporary = path.with_name(f".{path.name}.{self.transaction_id}.pending")
            with temporary.open("xb") as handle:
                handle.write(payload)
                handle.flush()
                os.fsync(handle.fileno())
            os.chmod(temporary, 0o644)
            os.replace(temporary, path)
            env.sync_directory(path.parent)
            self.migration_checkpoint("source_plist_written" if path == self.source_plist else "installed_plist_written")
        if self.job_loaded() or self.listeners():
            raise RuntimeError("service or listener appeared before held bootstrap")
        if any(path.read_bytes() != payload for path in (self.source_plist, self.installed_plist)):
            raise RuntimeError("launch configuration changed before held bootstrap")
        self.migration_checkpoint("bootstrap_pending")
        run("launchctl", "bootstrap", self.domain, str(self.installed_plist))
        if self.loaded_arguments() != settings["ProgramArguments"]:
            raise RuntimeError("permanent launcher configuration did not load")

    def select(self):
        env.assert_hold(self.root, self.guard)
        if self.qualification() != self.initial["qualification"]:
            raise RuntimeError("qualification evidence changed before selection")
        selected = env.selection(self.manifest)
        if selected["manifest_sha256"] != self.initial["manifest_sha256"]:
            raise RuntimeError("candidate manifest changed before selection")
        env.atomic_json(self.root / "active.json", selected)

    def release(self):
        env.release_hold(self.root, self.guard)

    def verify_ready(self, held, timeout):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            if self.pid() != held["pid"] or env.process_start(held["pid"]) != held["started_at"]:
                raise RuntimeError("selected provider process changed during readiness")
            health = self.health()
            live = self.health("livez")
            if (self.listeners() == [str(held["pid"])] and live.get("live") is True
                    and not live.get("error") and live.get("phase") in {"ready", "generating"}
                    and healthy_readiness(health)):
                return {"pid": held["pid"], "started_at": held["started_at"],
                        "readiness": health, "manifest_sha256": self.checked["manifest_sha256"]}
            time.sleep(.5)
        raise RuntimeError("provider did not become ready; review required, no automatic rollback")

    def publish(self):
        from environment_receipts import write_build_manifest
        source = Path(self.checked["manifest"]["source_root"])
        artifacts = {"python": Path(self.checked["manifest"]["python"]),
                     "launchd-plist": self.source_plist, "environment-selection": self.root / "active.json",
                     "environment-manifest": self.manifest, "environment-launcher": self.launcher,
                     **{key: source / name for key, name in {
                         "server": "coupled_astrid_server.py", "gateway": "coupled_http_gateway.py",
                         "logit-processor": "mlx_reservoir.py", "generation-controls": "generation_controls.py"}.items()}}
        write_build_manifest(self.deployment_manifest, component="coupled-model", repository=MODEL_ROOT,
                             artifacts=artifacts, actor=self.actor,
                             command="owned held environment transition; verified SIGTERM and same-PID readiness")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("operation", choices=("check", "activate", "rollback"))
    parser.add_argument("--control-root", type=Path, required=True)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--baseline-manifest", type=Path, required=True)
    parser.add_argument("--fixture-proof", type=Path, required=True)
    parser.add_argument("--qualification-proof", type=Path,
                        help="Accepted, hash-bound qualification required for check and activation")
    parser.add_argument("--qualification-baseline-manifest", type=Path,
                        help="Manifest of the isolated tested baseline clone; defaults to the rollback baseline")
    parser.add_argument("--expected-pid", type=int, required=True,
                        help="Exact provider PID; 0 only for owned interrupted-migration rollback with an absent job")
    parser.add_argument("--actor", required=True)
    parser.add_argument("--ack", required=True)
    parser.add_argument("--migrate-launcher", action="store_true")
    parser.add_argument("--sudo-debug", action="store_true",
                        help="First migration only: sudo the debug invocation in Terminal, retaining normal user controller identity")
    parser.add_argument("--timeout-secs", type=float, default=1500)
    parser.add_argument("--resume-transaction", type=Path,
                        help="Explicit rollback recovery of an owned retained hold; original failure remains intact")
    args = parser.parse_args()
    if os.geteuid() == 0:
        parser.error("do not sudo the controller; use --sudo-debug for the first migration")
    if args.operation != "rollback" and args.qualification_proof is None:
        parser.error("check and activate require --qualification-proof")
    if args.operation == "rollback" and args.manifest.resolve() != args.baseline_manifest.resolve():
        parser.error("rollback must select the explicit preserved baseline manifest")
    if args.resume_transaction is not None and args.operation != "rollback":
        parser.error("retained-hold recovery is available only for explicit rollback")
    backend = LaunchdBackend(control_root=args.control_root, manifest=args.manifest,
                             baseline=args.baseline_manifest, fixture_proof=args.fixture_proof,
                             actor=args.actor, ack=args.ack, resume_transaction=args.resume_transaction,
                             qualification_proof=args.qualification_proof, rollback=args.operation == "rollback",
                             sudo_debug=args.sudo_debug, qualification_baseline=args.qualification_baseline_manifest)
    try:
        if args.operation == "check":
            result = backend.inspect(args.expected_pid, migrate=args.migrate_launcher)
        else:
            result = transition(backend, expected_pid=args.expected_pid, actor=args.actor,
                                ack=args.ack, migrate=args.migrate_launcher,
                                rollback=args.operation == "rollback", timeout=args.timeout_secs)
        print(json.dumps(result, indent=2))
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        parser.exit(1, f"model environment transition refused: {error}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
