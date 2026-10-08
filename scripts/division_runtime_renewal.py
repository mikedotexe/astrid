#!/usr/bin/env python3
"""Approved dormant renewal through existing consumer and launch-hold wrappers.

Default is read-only. Failed application retains transaction evidence and owned
holds for explicit recovery; never force, roll back authority, or retry work.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import subprocess
import time

from bridge_drain import drain
from division_runtime_renewal_host import Runtime, GATEWAY, SUPERVISOR, ASTRID, MINIME
from minime_engine_transition_io import exclusive, read_json, require
from paired_minime_handoff import PairedHandoff, BRIDGE
from restart_minime_agent import LaunchdAgent, PROTECTED, reload_agent


class RenewalAgent(LaunchdAgent):
    """Only the renewal owns these two services; all other guards are unchanged."""
    def __init__(self, pause_generation, ack, runtime):
        super().__init__(pause_generation, ack)
        self.runtime = runtime

    def protected(self):
        self.runtime.guards()
        result = {}
        for label in PROTECTED:
            if label in (GATEWAY, SUPERVISOR):
                continue
            pid = self.pid(label)
            start = self.identity(pid)
            require(pid and start, f"protected service not running: {label}")
            result[label] = {"pid": pid, "started_at": start}
        return result

    def maintenance(self):
        self.runtime.maintenance()


class RenewalHandoff(PairedHandoff):
    def __init__(self, runtime, stage, receipt, ack):
        super().__init__(stage, receipt, ack, root=ASTRID)
        self.runtime = runtime
        self.guard["mode"] = "dormant_division_renewal"

    def begin(self, backend, inputs, config, protected, emit):
        self.runtime.guards()
        self.next_identity = backend.next_handoff_identity(read_json(
            MINIME / "workspace/runtime/autonomous_agent_source_status.json"))
        require(self.next_identity is not None, "native NEXT handoff acknowledgement required")
        super().begin(backend, inputs, config, protected, emit)

    def transition(self, backend, protected, emit):
        self.assert_hold()
        backend.maintenance()
        require(not backend.jobs()["active"], "old agent left active work")
        require(backend.inputs() == self.inputs and backend.config() == self.config
                and backend.protected() == protected, "consumer boundary changed")
        seal = backend.verify_next_seal(self.next_identity)
        self.runtime.record("minime_next_seal_verified", proof=seal)
        self.runtime.record("bridge_drain_intent")
        result = drain(protected[BRIDGE]["pid"], self.stage / "spectral-bridge-server",
                       ASTRID / ".runtime/bridge-lifecycle", 900, True)
        self.runtime.record("bridge_drained", drain=result)
        self.assert_hold()
        require(backend.protected() == protected, "protected identity changed during drain")
        self.runtime.renew()
        self.runtime.guards()
        # The sanctioned bridge wrapper accepts an already acknowledged drain,
        # snapshots its exact checkpoint and verifies the unchanged release.
        self.runtime.record("consumer_restore_intent")
        result = super().transition(backend, protected, emit)
        self.runtime.guards()
        self.runtime.verify_ready()
        return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bridge-stage", type=Path, required=True)
    parser.add_argument("--transaction", type=Path, required=True)
    parser.add_argument("--pause-generation", type=int, required=True)
    parser.add_argument("--expected-agent-pid", type=int, required=True)
    parser.add_argument("--expected-bridge-pid", type=int, required=True)
    parser.add_argument("--expected-gateway-pid", type=int, required=True)
    parser.add_argument("--expected-manifest-sha256", required=True)
    parser.add_argument("--ack", required=True)
    parser.add_argument("--apply", action="store_true")
    args = parser.parse_args()
    runtime = Runtime(args)
    try:
        runtime.preflight()
        if not args.apply:
            print(json.dumps({"ok": True, "status": "read_only_preflight_passed",
                              **runtime.summary()}))
            return 0
        with exclusive(MINIME / "workspace/runtime/engine-release.lock"):
            runtime.guards()
            runtime.prepare()
            receipt = runtime.transaction / "consumer-handoff.jsonl"
            handoff = RenewalHandoff(runtime, args.bridge_stage, receipt, args.ack)
            backend = RenewalAgent(args.pause_generation, args.ack, runtime)
            with receipt.open("x") as handle:
                os.chmod(receipt, 0o600)
                def emit(event):
                    event = {"recorded_at_unix_s": time.time(), **event}
                    handle.write(json.dumps(event, sort_keys=True) + "\n")
                    handle.flush()
                    os.fsync(handle.fileno())
                    print(json.dumps({k: v for k, v in event.items() if k in
                        {"phase", "outcome", "old_pid", "new_pid", "diagnostic"}}), flush=True)
                result = reload_agent(backend, args.expected_agent_pid,
                    expected_inputs=runtime.agent_inputs, handoff=handoff, emit=emit)
                emit(result)
            runtime.guards()
            runtime.verify_ready()
            runtime.record("completed", consumers=result)
            print(json.dumps({"ok": True, "status": "completed", **runtime.summary()}))
        return 0
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        if runtime.prepared:
            runtime.record("failed_requires_review", error=str(error)[:1000],
                           automatic_retry=False, automatic_rollback=False)
        print(json.dumps({"ok": False, "error": str(error)[:1000],
                          "transaction": str(runtime.transaction), "force_used": False}))
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
