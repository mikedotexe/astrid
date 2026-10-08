"""Host guards and ordered publication for an expired dormant manifest renewal."""
from __future__ import annotations

import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import plistlib
import re
import subprocess
import time
import urllib.request

from bridge_activate import LaunchdBridge
from minime_engine_transition_io import (Holds, atomic, command, digest, listeners,
    health_point, read_json, regular, require, save, stop_exact, validate_launcher)
from minime_runtime_binding import image_uuid, process
from reconcile_minime_launch import inventory
from restart_minime_agent import PROTECTED
from steward_control.config import load_config

ASTRID = Path("/Users/v/other/astrid")
MINIME = Path("/Users/v/other/minime")
ENGINE = "com.minime.engine"
GATEWAY = "com.minime.division-gateway"
SUPERVISOR = "com.minime.division-supervisor"
SERVICES = (SUPERVISOR, GATEWAY)
PORTS = (7878, 7879, 7880, 7882, 7883)
LIFETIME_MS = 30 * 24 * 60 * 60 * 1000


def renewed_manifest(old, now_ms):
    require(old["mode"] == "dormant" and old["candidate_hash"] == "unbound",
            "only dormant unbound renewal is supported")
    require(old["created_at_unix_ms"] <= old["expires_at_unix_ms"] < now_ms,
            "only an expired manifest may be renewed")
    return {**old, "created_at_unix_ms": now_ms, "expires_at_unix_ms": now_ms + LIFETIME_MS}


def parent_authority(value, manifest_hash, division_id):
    require(value.get("schema") == "division.gateway_authority.v1"
            and value.get("rail") == "parent"
            and value.get("manifest_sha256") == manifest_hash
            and value.get("division_id") == division_id
            and value.get("switch_receipt_sha256") is None
            and value.get("handoff_contract_receipt_sha256") is None
            and value.get("live_authority_granted_by_record") is False,
            "authority is not the exact dormant parent binding")


def renew_ordered(host):
    """Failures stop here; no automatic hold release or old authority restore."""
    host.freeze_services()
    host.publish_manifest()
    host.start_supervisor()
    host.start_gateway()
    host.verify_ready()


class Runtime:
    def __init__(self, args):
        self.args = args
        self.transaction = args.transaction.absolute()
        self.prepared = False
        self.phase = "unprepared"
        self.root = MINIME
        self.runtime = MINIME / "workspace/division/runtime"
        self.manifest = MINIME / "workspace/division/runtime-manifest.json"
        self.binary = MINIME / "minime/target/release/minime"
        self.authority = self.runtime / "authority.json"
        self.holds = Holds(MINIME / "workspace/runtime/engine-release-holds", str(self.transaction))
        self.current = {}
        self.released = set()
        self.published = False
        self.supervisor_started = False

    def summary(self):
        return {"transaction": str(self.transaction), "old_manifest_sha256": self.old_sha,
                "proposed_lifetime_days": 30, "mode": "dormant", "candidate_hash": "unbound",
                "engine_restart": False, "live_authority_granted_by_record": False,
                "gateway_connection_drain_acknowledged": False}

    def record(self, phase, **details):
        event = {"schema": "division_dormant_renewal_v1", "phase": phase,
                 "recorded_at_unix_ms": int(time.time() * 1000), **self.summary(), **details}
        path = self.transaction / "events.jsonl"
        with path.open("a") as handle:
            os.chmod(path, 0o600)
            handle.write(json.dumps(event, sort_keys=True) + "\n")
            handle.flush()
            os.fsync(handle.fileno())
        save(self.transaction / "status.json", event)
        self.phase = phase

    def no_children_or_ceremony(self):
        # This first renewal deliberately refuses nonempty ceremony history.
        # It does not reinterpret or erase any being-authored posture.
        ledger = Path(self.old["ceremony_ledger"])
        require(not os.path.lexists(ledger), "ceremony ledger present; separate review required")
        for being in ("astrid", "minime"):
            result = subprocess.run(["launchctl", "print", f"gui/{os.getuid()}/com.minime.division-child-{being}"],
                                    capture_output=True, text=True, timeout=10)
            require(result.returncode == 113, "daughter launch job is present or cannot be verified absent")
        require(all(not listeners(port) for port in (7903, 7904, 7905, 7906)), "daughter ports active")

    def load_configuration(self, label):
        source = MINIME / "launchd" / (label + ".plist")
        installed = Path.home() / "Library/LaunchAgents" / source.name
        require(source.read_bytes() == installed.read_bytes(), "source/installed launch config mismatch")
        config = plistlib.loads(source.read_bytes())
        launcher = MINIME / "scripts" / ("launchd_division_supervisor.sh" if label == SUPERVISOR else "launchd_division_gateway.sh")
        require(config["ProgramArguments"] == ["/bin/bash", str(launcher)]
                and config["KeepAlive"] is True, "unexpected Division launcher")
        text = command(["launchctl", "print", f"gui/{os.getuid()}/{label}"])
        match = re.search(r"^\targuments = \{\n(.*?)^\t\}", text, re.M | re.S)
        require(match is not None and [s.strip() for s in match[1].splitlines()] == config["ProgramArguments"],
                "loaded Division launcher differs")
        validate_launcher(launcher.read_bytes(), launcher.read_bytes(), label)
        return [source, installed, launcher], text

    def preflight(self):
        require(self.args.ack.strip() and len(self.args.ack) <= 2000, "bounded operator acknowledgement required")
        require(not os.path.lexists(self.transaction), "transaction exists; inspect it, do not replay")
        require(not any(p.is_symlink() for p in self.transaction.parents), "linked transaction parent")
        require(self.transaction.parent.is_dir(), "transaction parent must already exist")
        control = json.loads(command(["python3", ASTRID / "scripts/steward_control.py", "--json", "status"], timeout=60))
        require(control["state"]["paused"] and control["state"]["pause_generation"] == self.args.pause_generation
                and control["state"]["actor"] == "codex-astra-interactive"
                and control["lease"] is None and control["active_projection"] is None
                and control["evidence"]["valid"], "controller not quiescent at approved generation")
        for component, repo in (("bridge", ASTRID), ("minime", MINIME)):
            command(["python3", ASTRID / "scripts/deploy_preflight.py", "--component", component,
                     "--repo", repo, "--ack", self.args.ack], timeout=60)
        self.old_bytes = regular(self.manifest, 65536).read_bytes()
        self.old_sha = hashlib.sha256(self.old_bytes).hexdigest()
        require(self.old_sha == self.args.expected_manifest_sha256, "manifest identity changed")
        self.old = json.loads(self.old_bytes)
        spec = importlib.util.spec_from_file_location("runtime_manifest", MINIME / "scripts/division_runtime_manifest.py")
        verifier = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(verifier)
        verifier.verify(verifier.parser().parse_args(["verify", "--structural-only", "--manifest", str(self.manifest)]))
        renewed_manifest(self.old, int(time.time() * 1000))
        require(self.old["runtime_dir"] == str(self.runtime), "unqualified runtime directory")
        require(self.old["ceremony_ledger"] == str(MINIME / "workspace/division/ceremony_v1.jsonl"), "unqualified ceremony path")
        self.no_children_or_ceremony()
        self.old_authority_bytes = regular(self.authority).read_bytes()
        parent_authority(json.loads(self.old_authority_bytes), self.old_sha, self.old["division_id"])
        self.engine = process(ENGINE)
        expected = hashlib.sha256(f'{self.engine["pid"]}:{self.engine["started_at"]}'.encode()).hexdigest()
        require(expected == self.old["parent_process_identity"], "parent process binding drift")
        self.binary_sha = digest(regular(self.binary, 512 * 1024 * 1024))
        require(self.binary_sha == self.old["parent_deployment_identity"], "parent binary binding drift")
        self.uuid = image_uuid(command(["dwarfdump", "--uuid", self.binary]))
        for label in (ENGINE, GATEWAY):
            row = process(label)
            mapped = image_uuid(command(["sample", str(row["pid"]), "1", "1", "-file", "/dev/stdout"], timeout=30), sampled=True)
            require(mapped == self.uuid and process(label) == row, "mapped binary mismatch")
        require(all(listeners(p) == {self.engine["pid"]} for p in (7900, 7901, 7902)), "parent endpoint ownership changed")
        self.current[GATEWAY] = process(GATEWAY)
        require(self.current[GATEWAY]["pid"] == self.args.expected_gateway_pid, "gateway PID changed")
        fixed = [self.binary, MINIME / "scripts/division_runtime_manifest.py", MINIME / "workspace/rescue_profile.json"]
        for label in SERVICES:
            paths, loaded = self.load_configuration(label)
            fixed.extend(paths)
            require(not os.path.lexists(self.holds.root / (label + ".json")), "foreign launch hold present")
            if label == SUPERVISOR:
                require(not re.search(r"^\s*pid = \d+$", loaded, re.M)
                        and re.search(r"^\s*last exit code = 1$", loaded, re.M),
                        "expected expired supervisor is not quiescent")
        bridge = LaunchdBridge(self.args.bridge_stage)
        self.bridge = bridge.inspect(self.args.expected_bridge_pid)
        require(self.bridge["drain_supported"] and Path(self.bridge["binary"]) == self.args.bridge_stage / "spectral-bridge-server",
                "renewal must retain the exact live bridge release")
        self.agent_inputs = inventory(MINIME)
        status = read_json(MINIME / "workspace/runtime/autonomous_agent_source_status.json")
        require(status["pid"] == self.args.expected_agent_pid and status["source_inputs_at_start"] == self.agent_inputs
                and status["reload_required"] is False, "agent launch source differs from loaded source")
        self.fixed = {str(p): digest(regular(p, 512 * 1024 * 1024)) for p in fixed}
        for p in Path(__file__).parent.glob("division_runtime_renewal*.py"):
            self.fixed[str(p)] = digest(p)
        self.before_health = self.health()
        self.protected = {label: process(label) for label in PROTECTED
                          if label not in (*SERVICES, "com.astrid.spectral-bridge")}
        # Import before any live hold so a missing observer cannot strand services.
        from websockets.sync.client import connect  # noqa: F401
        self.guards()

    def health(self):
        return health_point(read_json(MINIME / "workspace/health.json"), now=time.time(),
                            earliest_ms=0, candidate=False)

    def maintenance(self):
        control = read_json(load_config(repo_root=ASTRID).state_root / "control.json")
        require(control.get("paused") is True
                and control.get("pause_generation") == self.args.pause_generation
                and control.get("actor") == "codex-astra-interactive",
                "interactive maintenance hold changed")

    def guards(self):
        self.maintenance()
        for label, identity in self.protected.items():
            require(process(label) == identity, f"protected service changed: {label}")
        require(process(ENGINE) == self.engine, "parent engine changed")
        health = self.health()
        require(health["session_id"] == self.before_health["session_id"]
                and health["t_s"] >= self.before_health["t_s"], "parent checkpoint continuity changed")
        for path, sha in self.fixed.items():
            require(digest(Path(path)) == sha, f"source/configuration drift: {path}")
        require(inventory(MINIME) == self.agent_inputs, "Minime launch inputs changed")
        require(digest(self.manifest) == (self.new_sha if self.published else self.old_sha), "runtime manifest changed")
        if not self.supervisor_started:
            require(self.authority.read_bytes() == self.old_authority_bytes, "old authority changed")
        else:
            parent_authority(read_json(self.authority), self.new_sha, self.old["division_id"])
        self.no_children_or_ceremony()
        for label, row in self.current.items():
            require(process(label) == row, "Division process identity changed")
        for label in SERVICES:
            if (self.holds.root / (label + ".json")).exists():
                self.holds.verify(label)

    def prepare(self):
        self.transaction.mkdir(mode=0o700)
        self.prepared = True
        self.new = renewed_manifest(self.old, int(time.time() * 1000))
        self.new_bytes = (json.dumps(self.new, sort_keys=True, indent=2) + "\n").encode()
        self.new_sha = hashlib.sha256(self.new_bytes).hexdigest()
        atomic(self.transaction / "old-manifest.json", self.old_bytes, 0o400)
        atomic(self.transaction / "old-authority.json", self.old_authority_bytes, 0o400)
        atomic(self.transaction / "new-manifest.json", self.new_bytes, 0o400)
        save(self.transaction / "before.json", {"engine": self.engine, "division": self.current,
            "fixed": self.fixed, "bridge": self.bridge, "agent_inputs": self.agent_inputs,
            "new_manifest_sha256": self.new_sha, "pause_generation": self.args.pause_generation,
            "health": self.before_health, "protected": self.protected})
        self.record("prepared")

    def wait_held(self, label):
        deadline = time.monotonic() + 60
        while time.monotonic() < deadline:
            self.holds.verify(label)
            try:
                row = process(label)
                args = command(["ps", "-p", str(row["pid"]), "-o", "comm=", "-o", "args="])
                launcher = "launchd_division_supervisor.sh" if label == SUPERVISOR else "launchd_division_gateway.sh"
                if "/bin/bash" in args and str(MINIME / "scripts" / launcher) in args:
                    self.current[label] = row
                    return
            except ValueError:
                pass
            time.sleep(.25)
        raise ValueError("service did not enter its owned launch hold; no force")

    def freeze_services(self):
        self.guards()
        self.record("service_holds_intent")
        for label in SERVICES:
            self.holds.create(label)
        # The expired supervisor exits by itself; wait for its KeepAlive shell.
        self.wait_held(SUPERVISOR)
        self.guards()
        self.record("gateway_stop_intent", gateway=self.current[GATEWAY])
        stop_exact(self.current[GATEWAY], 60)
        self.wait_held(GATEWAY)
        require(all(not listeners(p) for p in PORTS), "gateway listeners remain after stop")
        self.guards()
        self.record("services_held", services=self.current)

    def publish_manifest(self):
        self.guards()
        require(all(not listeners(p) for p in PORTS), "gateway must be stopped before publication")
        for label in SERVICES:
            self.holds.verify(label)
            require("bash" in command(["ps", "-p", str(self.current[label]["pid"]), "-o", "comm="]), "service not held")
        require((self.transaction / "new-manifest.json").read_bytes() == self.new_bytes, "candidate manifest changed")
        self.record("manifest_publish_intent", new_manifest_sha256=self.new_sha)
        atomic(self.manifest, self.new_bytes)
        self.published = True
        self.guards()
        self.record("manifest_published", new_manifest_sha256=self.new_sha)

    def start_supervisor(self):
        self.guards()
        self.record("supervisor_release_intent")
        self.holds.release(SUPERVISOR)
        self.released.add(SUPERVISOR)
        deadline = time.monotonic() + 60
        while time.monotonic() < deadline:
            require(process(SUPERVISOR) == self.current[SUPERVISOR], "supervisor respawned unexpectedly")
            value = read_json(self.runtime / "supervisor-status.json")
            if value.get("pid") == self.current[SUPERVISOR]["pid"] and value.get("manifest_sha256") == self.new_sha:
                parent_authority(read_json(self.authority), self.new_sha, self.old["division_id"])
                self.supervisor_started = True
                self.verify_supervisor(value)
                self.verify_mapped(SUPERVISOR)
                self.record("supervisor_verified", supervisor=self.current[SUPERVISOR])
                return
            time.sleep(.25)
        raise ValueError("renewed supervisor not ready; gateway hold retained")

    def verify_supervisor(self, value):
        require(value["pid"] == self.current[SUPERVISOR]["pid"] and value["manifest_sha256"] == self.new_sha
                and value["mode"] == "idle_parent_authoritative" and value["parent_authoritative"] is True
                and not value["children"] and not value["matching_intents"]
                and value["handoff_ready"] is False and value["commit_recommended"] is False
                and value["live_authority_granted_by_record"] is False
                and 0 <= time.time() * 1000 - value["updated_at_unix_ms"] < 15000,
                "supervisor status not fresh dormant parent readiness")

    def start_gateway(self):
        self.guards()
        self.verify_supervisor(read_json(self.runtime / "supervisor-status.json"))
        self.record("gateway_release_intent")
        self.holds.release(GATEWAY)
        self.released.add(GATEWAY)
        deadline = time.monotonic() + 60
        while time.monotonic() < deadline:
            require(process(GATEWAY) == self.current[GATEWAY], "gateway respawned unexpectedly")
            if all(listeners(p) == {self.current[GATEWAY]["pid"]} for p in PORTS):
                self.verify_ready()
                self.verify_mapped(GATEWAY)
                self.record("gateway_verified", gateway=self.current[GATEWAY])
                return
            time.sleep(.25)
        raise ValueError("renewed gateway not ready")

    def verify_mapped(self, label):
        row = self.current[label]
        mapped = image_uuid(command(["sample", str(row["pid"]), "1", "1", "-file", "/dev/stdout"],
                                    timeout=30), sampled=True)
        require(mapped == self.uuid and process(label) == row, "new mapped binary mismatch")

    def observe_telemetry(self):
        from websockets.sync.client import connect
        samples = []
        with connect("ws://127.0.0.1:7878", open_timeout=5, close_timeout=2,
                     max_size=512 * 1024) as stream:
            for _ in range(2):
                packet = json.loads(stream.recv(timeout=10))
                values = packet.get("eigenvalues")
                stamp = packet.get("t_ms")
                require(isinstance(stamp, int) and not isinstance(stamp, bool)
                        and isinstance(values, list) and 0 < len(values) <= 128
                        and all(isinstance(v, (float, int)) and not isinstance(v, bool)
                                and math.isfinite(v) for v in values), "invalid public telemetry")
                samples.append({"t_ms": stamp, "eigenvalue_count": len(values)})
        require(samples[1]["t_ms"] > samples[0]["t_ms"], "public telemetry did not advance")
        self.verify_ready()
        self.record("passive_telemetry_verified", samples=samples, health=self.health())

    def verify_ready(self):
        self.guards()
        self.verify_supervisor(read_json(self.runtime / "supervisor-status.json"))
        value = read_json(self.runtime / "gateway-status.json")
        require(value["pid"] == self.current[GATEWAY]["pid"] and value["manifest_sha256"] == self.new_sha
                and value["mode"] == "transparent_parent" and value["live_authority_granted_by_record"] is False
                and value["started_at_unix_ms"] >= self.new["created_at_unix_ms"], "gateway binding not renewed")
        require(all(listeners(p) == {self.current[GATEWAY]["pid"]} for p in PORTS), "public port ownership differs")
        require(all(listeners(p) == {self.engine["pid"]} for p in (7900, 7901, 7902)), "parent port ownership differs")

    def renew(self):
        renew_ordered(self)
        deadline = time.monotonic() + 60
        while time.monotonic() < deadline:
            self.verify_ready()
            try:
                with urllib.request.urlopen("http://127.0.0.1:8090/readyz", timeout=3) as response:
                    ready = json.load(response)
                if (ready.get("ready") is True and ready.get("worker", {}).get("phase") == "ready"
                        and ready.get("worker", {}).get("queue_depth") == 0
                        and ready.get("reservoir", {}).get("status") == "connected"):
                    self.observe_telemetry()
                    self.record("runtime_renewed_provider_reconnected")
                    return
            except (OSError, ValueError):
                pass
            time.sleep(.5)
        raise ValueError("provider did not reconnect; consumers remain held")
