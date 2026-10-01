#!/usr/bin/env python3
"""Exact-stage, owner-held engine transition; only via the sanctioned wrapper."""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import time

from minime_engine_transition_host import Host


def transition(host, *, stop_timeout=120, observe_seconds=360):
    """Record intent before each boundary; failure never removes an unknown hold."""
    record = {"schema": "minime_engine_transition_v1", "status": "preflight",
              "activation_performed": False, "complete_input_drain_claimed": False}
    host.preflight()
    host.prepare()
    def step(name, operation):
        record["phase"] = name
        host.record(record)
        result = operation()
        record[name] = result
        host.record(record)
        return result
    try:
        step("launch_holds", host.hold)
        step("old_processes_stopped", lambda: host.stop_old(stop_timeout))
        step("stopped_checkpoint", host.checkpoint)
        step("candidate_installed", host.install)
        record["activation_performed"] = True
        step("signed_handoff", host.handoff)
        step("engine_ready", lambda: host.start_engine(stop_timeout))
        step("companions_ready", lambda: host.start_companions(stop_timeout))
        step("observation", lambda: host.observe(observe_seconds))
        step("published_manifest", host.publish)
        record["status"] = "activated_verified"
    except BaseException as error:
        record.update(status="failed_requires_review", error=str(error)[:1500])
        host.record(record)
        # Recovery revalidates exact ownership and preserves the newest state.
        # No restart is attempted if stopped-state integrity was never established.
        if record.get("stopped_checkpoint"):
            try:
                record["rollback"] = host.rollback(stop_timeout)
                record["status"] = "rolled_back_requires_review"
            except Exception as rollback_error:
                record["rollback_error"] = str(rollback_error)[:1500]
        host.record(record)
        raise
    host.record(record)
    return record


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stage", type=Path, required=True)
    parser.add_argument("--manifest-sha256", required=True)
    parser.add_argument("--binding", type=Path, required=True)
    parser.add_argument("--transaction", type=Path, required=True)
    parser.add_argument("--actor", required=True)
    parser.add_argument("--ack", required=True)
    parser.add_argument("--legacy-stop-ack", required=True)
    parser.add_argument("--launcher-source", type=Path, required=True)
    parser.add_argument("--check-only", action="store_true")
    parser.add_argument("--stop-timeout", type=int, default=120)
    parser.add_argument("--observe-seconds", type=int, default=360)
    args = parser.parse_args()
    if os.environ.get("ASTRID_SANCTIONED_ENGINE_ACTIVATION") != "1":
        parser.error("use deploy_minime.sh --activate-stage")
    if not args.ack.strip() or not args.legacy_stop_ack.strip():
        parser.error("explicit transition and legacy continuity acknowledgements required")
    if not 30 <= args.stop_timeout <= 600 or not 360 <= args.observe_seconds <= 900:
        parser.error("stop timeout 30..600 and observation 360..900 seconds required")
    host = Host(args)
    with host.exclusive():
        if args.check_only:
            host.preflight()
            print(json.dumps({"status": "preflight_passed", "activation_performed": False}))
            return
        transition(host, stop_timeout=args.stop_timeout, observe_seconds=args.observe_seconds)
    print(json.dumps({"transaction": str(args.transaction), "status": "activated_verified",
                      "finished_at_unix_s": time.time()}))


if __name__ == "__main__":
    main()
