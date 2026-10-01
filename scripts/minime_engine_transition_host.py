"""Host implementation for a narrow, operator-approved legacy engine transition.

No broad stop/start script, forced signal, profile mutation, state backup restore,
key provisioning or automatic controller resume is permitted here.
"""
from __future__ import annotations

from contextlib import contextmanager
import hashlib
import json
import os
from pathlib import Path
import plistlib
import time

import minime_engine_stage as stage_tool
import minime_runtime_binding as binding_tool
from minime_engine_transition_io import (
    LABELS, LAUNCHERS, PORTS, PROTECTED, Holds, atomic, command, digest, exclusive,
    health_point, listeners, read_json, regular, require, save, started, stop_exact,
    sync_dir, validate_launcher,
)

ASTRID = Path("/Users/v/other/astrid")
MINIME = Path("/Users/v/other/minime")


class Host:
    def __init__(self, args):
        self.args = args
        self.root = MINIME
        self.workspace = self.root / "workspace"
        self.engine = self.root / "minime/target/release/minime"
        self.stage = args.stage.resolve(strict=True)
        self.transaction = args.transaction.absolute()
        self.holds = Holds(self.workspace / "runtime/engine-release-holds", str(self.transaction))
        self.manifest_path = ASTRID / "capsules/spectral-bridge/workspace/deployment_manifests/minime-division-runtime.json"
        self.division = self.workspace / "division/runtime-manifest.json"
        self.runtime = self.workspace / "division/runtime"
        self.control_root = Path.home() / ".minime/self-control-v2"
        self.profile = self.workspace / "rescue_profile.json"
        self.log = self.root / "logs/minime-engine.log"
        self.current = {}
        self.released = set()
        self.candidate = True
        self.rollback_pending_retained = False
        self.env = {k: v for k, v in os.environ.items() if k in ("HOME", "PATH", "USER", "TMPDIR", "LANG")}

    @contextmanager
    def exclusive(self):
        with exclusive(self.workspace / "runtime/engine-release.lock"):
            yield

    def control(self, *, target=True, pending=False):
        value = json.loads(command([self.engine, "self-control", "status", "--root", self.control_root], env=self.env))
        require(value.get("integrity_verified") is True and value.get("pending_transition") is False,
                "self-control integrity or command transition not settled")
        if target:
            require(value.get("state_targets_this_binary") is True, "self-control does not target installed binary")
        if not pending:
            require(value.get("pending_deployment_handoff") is None, "pending self-control handoff")
        return {k: value[k] for k in ("state_sha256", "revision_by_family", "deployment_identity",
                                     "cli_deployment_identity", "binary_sha256", "state_targets_this_binary")}

    def dormant(self, *, fresh=True):
        manifest = read_json(self.division)
        authority = read_json(self.runtime / "authority.json")
        supervisor = read_json(self.runtime / "supervisor-status.json")
        require(manifest["mode"] == "dormant" and manifest["candidate_hash"] == "unbound", "Division is not dormant")
        require(manifest["expires_at_unix_ms"] > time.time() * 1000 + 3600000, "dormant manifest expiry too near")
        require(authority["rail"] == "parent" and not authority["live_authority_granted_by_record"]
                and authority.get("switch_receipt_sha256") is None
                and authority.get("handoff_contract_receipt_sha256") is None, "active Division authority")
        require(not supervisor["children"] and not supervisor["matching_intents"]
                and supervisor["mode"] == "idle_parent_authoritative", "Division is not quiescent")
        if fresh:
            require(0 <= time.time() * 1000 - supervisor["updated_at_unix_ms"] < 15000, "stale supervisor")
        return manifest

    def protected(self):
        return {label: binding_tool.process(label) for label in PROTECTED}

    def guards(self):
        require(self.protected() == self.before_protected, "protected service identity changed")
        for path, sha in self.fixed.items():
            require(digest(Path(path)) == sha, f"configuration/source drift: {path}")
        for label, name in zip(LABELS, LAUNCHERS):
            expected = self.new_launchers[name] if self.launchers_installed else self.old_launchers[name]
            require((self.root / "scripts" / name).read_bytes() == expected, "launcher drift")

    def preflight(self):
        require(not self.transaction.exists() and not self.transaction.is_symlink(), "transaction already exists")
        require(not any(p.is_symlink() for p in self.transaction.parents), "linked transaction parent")
        stage_tool.verify(self.stage, self.args.manifest_sha256)
        self.stage_manifest = read_json(self.stage / "manifest.json")
        source = Path(self.stage_manifest["source_root"])
        require(stage_tool.inputs(source) == self.stage_manifest["inputs"], "staged engine source drift")
        require(self.args.launcher_source.resolve() == source.resolve(), "launcher source differs from qualified checkout")
        self.binding = read_json(self.args.binding)
        require(not binding_tool.validate(self.binding), "fresh live build binding refused")
        require(self.binding["minime_repository"] == str(self.root), "binding repository mismatch")
        self.before = {r["label"]: {k: r[k] for k in ("label", "pid", "started_at")} for r in self.binding["processes"]}
        self.current = dict(self.before)
        self.before_protected = self.protected()
        self.old_manifest = read_json(self.manifest_path)
        self.old_sha = digest(regular(self.engine, 512 * 1024 * 1024))
        require(self.old_sha == self.binding["current_disk_executable"]["sha256"]
                == self.binding["artifacts"]["retained-minime"]["sha256"], "old disk/mapped identity mismatch")
        self.old_division = self.dormant()
        control = json.loads(command(["python3", ASTRID / "scripts/steward_control.py", "--json", "status"], timeout=60))
        require(control["state"]["paused"] and control["lease"] is None
                and control["active_projection"] is None and control["evidence"]["valid"], "controller not quiescent")
        command(["python3", ASTRID / "scripts/deploy_preflight.py", "--component", "minime",
                 "--repo", self.root, "--ack", self.args.ack], timeout=60)
        command(["python3", ASTRID / "scripts/deploy_preflight.py", "--component", "bridge",
                 "--repo", ASTRID, "--ack", self.args.ack], timeout=60)
        profile = read_json(self.profile)
        require(profile["runtime_profile"] == "stable_core_v1" and not profile["stable_core_checkpoint_lineage_enabled"]
                and not profile["stable_core_neural_bundle_enabled"], "unqualified restore policy")
        normalized = json.loads(command(["python3", "-c",
            "import json,sys;sys.path.insert(0,'scripts');import minime_rescue_investigation as m;print(json.dumps(m.load_active_profile(m.default_context())))"], cwd=self.root))
        require(normalized == profile, "launcher would rewrite the active profile")
        self.old_launchers, self.new_launchers = {}, {}
        fixed_paths = [self.profile, self.root / "scripts/minime_rescue_investigation.py"]
        for label, name in zip(LABELS, LAUNCHERS):
            old = regular(self.root / "scripts" / name).read_bytes()
            new = regular(source / "scripts" / name).read_bytes()
            validate_launcher(old, new, label)
            command(["bash", "-n", source / "scripts" / name])
            self.old_launchers[name], self.new_launchers[name] = old, new
            require(not os.path.lexists(self.holds.root / (label + ".json")), "pre-existing launch hold")
            for path in (self.root / "launchd" / (label + ".plist"), Path.home() / "Library/LaunchAgents" / (label + ".plist")):
                fixed_paths.append(path)
                env = plistlib.loads(path.read_bytes()).get("EnvironmentVariables", {})
                require(not any(k in env for k in ("MINIME_SELF_CONTROL_ROOT", "MINIME_DEPLOYMENT_IDENTITY")), "unqualified control identity override")
        self.fixed = {str(p): digest(regular(p)) for p in fixed_paths}
        for path in source.glob("scripts/launchd_*.sh"):
            if path.name in LAUNCHERS:
                self.fixed[str(path)] = digest(regular(path))
        self.launchers_installed = False
        self.before_control = self.control()
        self.before_health = health_point(read_json(self.workspace / "health.json"), now=time.time(), earliest_ms=0, candidate=False)
        self.guards()

    def prepare(self):
        self.transaction.mkdir(mode=0o700)
        atomic(self.transaction / "old-minime", regular(self.engine, 512 * 1024 * 1024).read_bytes(), 0o500)
        require(digest(self.transaction / "old-minime") == self.old_sha, "rollback copy mismatch")
        for name, data in self.old_launchers.items():
            atomic(self.transaction / name, data, 0o400)
        save(self.transaction / "old-build-manifest.json", self.old_manifest)
        save(self.transaction / "old-division-manifest.json", self.old_division)
        save(self.transaction / "before.json", {"processes": self.before, "protected": self.before_protected,
             "fixed_hashes": self.fixed, "control": self.before_control, "health": self.before_health,
             "stage_manifest_sha256": self.args.manifest_sha256, "actor": self.args.actor,
             "ack": self.args.ack, "legacy_stop_ack": self.args.legacy_stop_ack,
             "complete_input_drain_claimed": False, "no_state_backup_restore": True})

    def record(self, record):
        save(self.transaction / "transition.json", {**record, "updated_at_unix_s": time.time()})

    def hold(self):
        require(not binding_tool.validate(self.binding), "binding drift before launch hold")
        self.guards()
        for label in LABELS:
            self.holds.create(label)
        for name, data in self.new_launchers.items():
            atomic(self.root / "scripts" / name, data, 0o755)
        self.launchers_installed = True
        self.guards()
        return {"owner": str(self.transaction), "labels": list(LABELS)}

    def wait_held(self, label, timeout):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            try:
                row = binding_tool.process(label)
                require(row != self.current.get(label), "old process still present")
                args = command(["ps", "-p", str(row["pid"]), "-o", "comm=", "-o", "args="])
                require("/bin/bash" in args and str(self.root / "scripts" / LAUNCHERS[LABELS.index(label)]) in args,
                        "replacement is not the held launch shell")
                self.holds.verify(label)
                require(all(not listeners(p) for p in PORTS[label]), "held service still owns listeners")
                self.current[label] = row
                return row
            except ValueError:
                time.sleep(.5)
        raise ValueError("replacement launch shell did not reach its hold")

    def stop_old(self, timeout):
        self.guards()
        require(digest(self.engine) == self.old_sha, "old executable changed before stop")
        self.dormant()
        self.control()
        for label in (LABELS[2], LABELS[1], LABELS[0]):
            self.holds.verify(label)
            require(binding_tool.process(label) == self.before[label], "old service moved before stop")
            if label == LABELS[0]:
                self.signal_at = time.time()
                self.log_offset = self.log.stat().st_size
            stop_exact(self.before[label], timeout)
            self.wait_held(label, timeout)
        self.guards()
        return {"old": self.before, "held": dict(self.current), "signal_at_unix_s": self.signal_at,
                "input_and_gateway_drain_acknowledged": False}

    def stopped(self):
        self.guards()
        for label in LABELS:
            self.holds.verify(label)
            require(binding_tool.process(label) == self.current[label], "held process identity changed")
            args = command(["ps", "-p", str(self.current[label]["pid"]), "-o", "comm="])
            require("bash" in args, "held process already executed")
            require(all(not listeners(p) for p in PORTS[label]), "unexpected listener during stopped interval")

    def checkpoint(self):
        self.stopped()
        context = self.workspace / "regulator_context.json"
        require(context.stat().st_mtime >= self.signal_at, "no post-signal regulator checkpoint")
        with self.log.open("rb") as f:
            f.seek(self.log_offset)
            tail = f.read(8 * 1024 * 1024)
        require(b"State saved. Exiting." in tail, "old engine did not report its shutdown save")
        directory = self.transaction / ("rollback-checkpoint" if not self.candidate else "stopped-checkpoint")
        directory.mkdir(mode=0o700)
        hashes = {}
        for name in ("regulator_context.json", "rescue_scaffold.bin", "rescue_scaffold.json"):
            src = regular(self.workspace / name)
            before = digest(src)
            atomic(directory / name, src.read_bytes(), 0o400)
            require(before == digest(src) == digest(directory / name), "stopped checkpoint moved")
            hashes[name] = before
        inspector = self.stage / "bin/engine_restore_inspect"
        report = json.loads(command(["/usr/bin/sandbox-exec", "-p",
            "(version 1)(allow default)(deny network*)(deny file-write*)", inspector,
            "--context", directory / "regulator_context.json", "--scaffold", directory / "rescue_scaffold.bin",
            "--scaffold-metadata", directory / "rescue_scaffold.json"], timeout=30))
        require(report.get("passed") is True and report["context"]["resume_mode"] == "pi_only_resume", "stopped restore validation failed")
        self.stopped_control = self.control()
        self.checkpoint_hashes = hashes
        save(directory / "inspection.json", report)
        with context.open("rb") as f:
            os.fsync(f.fileno())
        sync_dir(context.parent)
        return {"hashes": hashes, "restore": report, "control": self.stopped_control,
                "scope": "PI_context_and_scaffold_not_complete_inflight_or_reservoir_state"}

    def install(self):
        self.stopped()
        stage_tool.verify(self.stage, self.args.manifest_sha256)
        require(digest(self.engine) == self.old_sha, "old executable changed")
        atomic(self.engine, (self.stage / "bin/minime").read_bytes(), 0o755)
        self.expected_sha = self.stage_manifest["artifacts"]["minime"]["sha256"]
        require(digest(self.engine) == self.expected_sha, "installed candidate mismatch")
        return {"sha256": self.expected_sha, "path": str(self.engine)}

    def handoff(self):
        self.stopped()
        before = self.control(target=False)
        require(before["state_sha256"] == self.stopped_control["state_sha256"], "control state moved while stopped")
        result = json.loads(command([self.engine, "self-control", "prepare-deployment-handoff",
            "--root", self.control_root, "--operator-actor", self.args.actor,
            "--operator-ack", self.args.ack], env=self.env))
        require(result["status"] == "prepared", "a signed deployment handoff was not prepared")
        handoff = result["handoff"]
        require(handoff["target_binary_sha256"] == self.expected_sha
                and handoff["target_executable_path"] == str(self.engine)
                and handoff["from_state_sha256"] == before["state_sha256"], "handoff binding mismatch")
        self.handoff_id = handoff["handoff_id"]
        self.pending_sha = digest(self.control_root / "deployment_handoff.pending.json")
        save(self.transaction / ("handoff.json" if self.candidate else "rollback-handoff.json"), result)
        return {k: handoff[k] for k in ("handoff_id", "from_state_sha256", "from_deployment_identity",
                                      "to_deployment_identity", "target_binary_sha256")}

    def rebind_dormant(self):
        self.stopped()
        for name, sha in self.checkpoint_hashes.items():
            require(digest(self.workspace / name) == sha, "stopped startup input changed before launch")
        state = self.control(target=False, pending=True)
        require(state["state_sha256"] == self.stopped_control["state_sha256"], "control state changed before launch")
        if self.handoff_id is not None:
            require(digest(self.control_root / "deployment_handoff.pending.json") == self.pending_sha,
                    "signed handoff changed before launch")
        current = read_json(self.division)
        require(current == self.old_division or current == getattr(self, "bound_division", None), "dormant manifest changed")
        require(read_json(self.runtime / "authority.json")["rail"] == "parent", "authority changed")
        parent = self.current[LABELS[0]]
        manifest = dict(self.old_division)
        manifest["parent_process_identity"] = hashlib.sha256(f'{parent["pid"]}:{parent["started_at"]}'.encode()).hexdigest()
        manifest["parent_deployment_identity"] = self.expected_sha
        manifest["plan_digest"] = hashlib.sha256(("dormant:" + self.expected_sha).encode()).hexdigest()
        manifest["created_at_unix_ms"] = int(time.time() * 1000)
        # Retain the existing bounded lifetime and all ceremony/owner paths.
        save(self.division, manifest)
        self.bound_division = manifest

    def mapped(self, label):
        row = binding_tool.process(label)
        require(row == self.current[label], "replacement PID/start changed")
        uuid = binding_tool.image_uuid(command(["dwarfdump", "--uuid", self.engine]))
        sampled = binding_tool.image_uuid(command(["sample", str(row["pid"]), "1", "1", "-file", "/dev/stdout"], timeout=120), sampled=True)
        require(uuid == sampled and binding_tool.process(label) == row, "replacement mapped image mismatch")
        require(all(listeners(p) == {row["pid"]} for p in PORTS[label]), "replacement port identity mismatch")
        return {**row, "image_uuid": uuid, "binary_sha256": self.expected_sha}

    def wait_ready(self, label, timeout):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            require(binding_tool.process(label) == self.current[label], "replacement exited or respawned")
            if all(listeners(p) == {self.current[label]["pid"]} for p in PORTS[label]):
                return
            time.sleep(.5)
        raise ValueError("replacement readiness timeout")

    def start_engine(self, timeout):
        self.rebind_dormant()
        self.start_at = time.time()
        self.holds.release(LABELS[0])
        self.released.add(LABELS[0])
        self.wait_ready(LABELS[0], timeout)
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            value = read_json(self.workspace / "health.json")
            if value["provenance"]["wall_clock_unix_ms"] >= self.start_at * 1000:
                point = health_point(value, now=time.time(), earliest_ms=self.start_at * 1000, candidate=self.candidate)
                break
            time.sleep(.5)
        else:
            raise ValueError("no fresh startup health")
        lineage = self.control(pending=self.rollback_pending_retained)
        if self.handoff_id is not None:
            applied = self.control_root / "deployment_handoffs/applied"
            matches = []
            paths = list(applied.glob("*.json"))
            require(len(paths) <= 4096, "applied handoff inventory exceeds review bound")
            for path in paths:
                value = read_json(path)
                if value.get("handoff", {}).get("handoff_id") == self.handoff_id:
                    matches.append(value)
            require(len(matches) == 1 and matches[0]["state_fields_preserved_except_deployment_identity"] is True,
                    "missing applied state-preservation witness")
        self.guards()
        return {"process": self.mapped(LABELS[0]), "health": point, "control": lineage,
                "applied_handoff_id": self.handoff_id}

    def start_companions(self, timeout):
        # Supervisor rebinds only existing parent authority before gateway reads it.
        self.holds.release(LABELS[2])
        self.released.add(LABELS[2])
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            require(binding_tool.process(LABELS[2]) == self.current[LABELS[2]], "supervisor exited")
            row = read_json(self.runtime / "supervisor-status.json")
            if row["pid"] == self.current[LABELS[2]]["pid"] and row["manifest_sha256"] == digest(self.division):
                break
            time.sleep(.5)
        else:
            raise ValueError("supervisor readiness timeout")
        self.holds.release(LABELS[1])
        self.released.add(LABELS[1])
        self.wait_ready(LABELS[1], timeout)
        gateway = read_json(self.runtime / "gateway-status.json")
        require(gateway["pid"] == self.current[LABELS[1]]["pid"]
                and gateway["manifest_sha256"] == digest(self.division)
                and gateway["mode"] == "transparent_parent", "gateway binding mismatch")
        self.dormant()
        self.guards()
        return [self.mapped(label) for label in LABELS[1:]]

    def observe(self, seconds):
        points = []
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            self.guards()
            require(digest(self.engine) == self.expected_sha, "live binary drift")
            for label in LABELS:
                require(binding_tool.process(label) == self.current[label], "live process changed")
                require(all(listeners(p) == {self.current[label]["pid"]} for p in PORTS[label]), "live listener changed")
            point = health_point(read_json(self.workspace / "health.json"), now=time.time(),
                                 earliest_ms=self.start_at * 1000, candidate=self.candidate)
            if points:
                require(point["session_id"] == points[-1]["session_id"] and point["t_s"] >= points[-1]["t_s"], "engine clock/session reset")
            points.append(point)
            save(self.transaction / ("observation.json" if self.candidate else "rollback-observation.json"), points)
            time.sleep(5)
        self.control(pending=self.rollback_pending_retained)
        self.dormant()
        return {"seconds": seconds, "samples": len(points), "min_fill": min(p["fill_pct"] for p in points),
                "max_fill": max(p["fill_pct"] for p in points), "last": points[-1]}

    def publish(self):
        self.guards()
        value = dict(self.old_manifest)
        value.update(actor=self.args.actor, command="verified exact-stage activation; no rebuild",
                     activated_at_unix_s=time.time(),
                     stage_manifest_sha256=self.args.manifest_sha256 if self.candidate else None,
                     transaction=str(self.transaction))
        if self.candidate:
            value["built_at_unix_s"] = self.stage_manifest["created_at_unix_s"]
            value.pop("built_at", None)
            value["repository"] = {"path": self.stage_manifest["source_root"], "head": self.stage_manifest["source_head"],
                                   "dirty": self.stage_manifest["source_dirty"], "source_identity": "exact_stage_inputs"}
        value["artifacts"] = {"minime-engine": binding_tool.artifact(self.engine),
                              **{name: binding_tool.artifact(self.root / "scripts" / script)
                                 for name, script in zip(("engine-launcher", "gateway-launcher", "supervisor-launcher"), LAUNCHERS)}}
        save(self.transaction / ("new-build-manifest.json" if self.candidate else "rollback-build-manifest.json"), value)
        save(self.manifest_path, value)
        return {"path": str(self.manifest_path), "sha256": digest(self.manifest_path)}

    def rollback(self, timeout):
        # Never replace unknown processes or restore old control/preference files.
        self.guards()
        require(digest(self.engine) in (self.old_sha, self.stage_manifest["artifacts"]["minime"]["sha256"]),
                "unknown binary at rollback boundary")
        for label in LABELS:
            if label in self.released:
                self.holds.create(label)
            else:
                self.holds.verify(label)
        for label in (LABELS[2], LABELS[1], LABELS[0]):
            if label in self.released:
                require(binding_tool.process(label) == self.current[label], "unknown rollback process")
                if label == LABELS[0]:
                    self.signal_at, self.log_offset = time.time(), self.log.stat().st_size
                stop_exact(self.current[label], timeout)
                self.wait_held(label, timeout)
        engine_ran = LABELS[0] in self.released
        self.released.clear()
        self.candidate = False
        if engine_ran:
            self.checkpoint()
        self.stopped()
        require(digest(self.transaction / "old-minime") == self.old_sha, "rollback asset changed")
        atomic(self.engine, (self.transaction / "old-minime").read_bytes(), 0o755)
        self.expected_sha = self.old_sha
        # A failed pre-start handoff can remain pending. Let the existing signed
        # implementation preserve/quarantine it; never delete its evidence here.
        current = self.control(target=False, pending=True)
        if current["state_targets_this_binary"]:
            require(not engine_ran, "rollback expected a new deployment identity")
            require(current["state_sha256"] == self.stopped_control["state_sha256"], "old control state unexpectedly moved")
            self.handoff_id = None
            self.rollback_pending_retained = True
        else:
            self.stopped_control = current
            self.handoff()
        engine = self.start_engine(timeout)
        companions = self.start_companions(timeout)
        observation = self.observe(360)
        manifest = self.publish()
        return {"engine": engine, "companions": companions, "observation": observation, "manifest": manifest,
                "prestart_pending_evidence_retained_for_review": self.rollback_pending_retained}
