#!/usr/bin/env python3
"""Qualify exact review staging with native binaries and synthetic private state.

No live paths, inference, services, delivery claims or activation authority.
Runtime consumption/crash behavior remains separately covered by Rust tests.
"""
from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import os
from pathlib import Path
import subprocess

from bridge_stage import atomic_json, verify_stage
from qualify_geometry_release import Fixture, file_hashes, require, sha, write_json

ACK = "retire-exact-dispatch-unresolved-without-retry-or-next"
SCHEMA = "study_uncertainty_review_v1"
SENTINEL = "PRIVATE_SYNTHETIC_SENTINEL_NOT_FOR_LOGS"


def encoded(value):
    return json.dumps(value, separators=(",", ":"), ensure_ascii=False).encode()


def retain(directory, value):
    raw = encoded(value)
    digest = hashlib.sha256(raw).hexdigest()
    path = directory / f"{digest}.json"
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("xb") as handle:
        os.chmod(path, 0o600)
        handle.write(raw)
        handle.flush()
        os.fsync(handle.fileno())
    return digest


def native(binary, manifest, fixture, *args, expected=None):
    root = fixture.root
    env = {key: os.environ[key] for key in ("PATH", "TMPDIR") if key in os.environ}
    env.update(HOME=str(root / "home"), RUST_LOG="warn")
    command = [str(binary), "--deployment-manifest", str(manifest),
               "--astrid-root", str(root / "astrid"),
               "--bridge-root", str(root / "astrid/capsules/spectral-bridge"),
               "--bridge-workspace", str(fixture.workspace),
               "--minime-root", str(root / "minime"),
               "--workspace-path", str(root / "minime/workspace"),
               "--perception-path", str(root / "perception"),
               "--db-path", str(root / "must-not-create.db"), *map(str, args)]
    result = subprocess.run(command, cwd=root, env=env, stdin=subprocess.DEVNULL,
                            capture_output=True, text=True, timeout=20, check=False)
    require(SENTINEL not in result.stdout + result.stderr, "private sentinel leaked to CLI output")
    require(not (root / "must-not-create.db").exists(), "review command started runtime storage")
    if expected is None:
        require(result.returncode == 0, f"native review failed: {result.stderr[-1500:]}")
        return json.loads(result.stdout)
    require(result.returncode != 0 and expected in result.stderr,
            "native refusal did not match its expected boundary")
    return {"refused": True, "reason": expected}


def probe(old_binary, old_manifest, binary, manifest, reader, root):
    f = Fixture(root, "astrid", reader, reader)
    prepared = f.prepare(reader, f"WRITE START {SENTINEL}", request_id="synthetic-draft")
    require(prepared["input_kind"] == "private_writing", "fixture did not prepare private writing")
    store = f.workspace / "action_threads"
    handoff = f.state / "study-handoff"
    store.mkdir(exist_ok=True)
    job_id = "study-job-" + hashlib.sha256(b"synthetic-exact-dispatch").hexdigest()
    intent = retain(handoff, {"owner_root": str(store), "target": {
        "label": "WRITE CONTINUE", "offset": "auto", "operation_id": job_id}})
    output = retain(handoff, prepared)
    state = {"selection_revision": 7, "foreground_reader": None, "return_reader": None,
             "mailbox_window": None, "native_focus_window": None,
             "study_handoff": {"active": job_id, "jobs": {
                 job_id: {"intent": intent, "output": output, "phase": "claimed"}}}}
    state_path = store / "activity_runtime_v3.json"
    write_json(state_path, state)
    for version, next_version in ((1, 2), (2, 3)):
        (store / f"activity_runtime_v{version}.json").write_text(
            f'{{"foreground_reader":"requires activity_runtime_v{next_version}"}}\n')
    review = {"schema": SCHEMA, "operation_id": "synthetic-operator-review",
              "actor": "qualification-only", "acknowledgement": ACK, "job_id": job_id,
              "expected_selection_revision": 7, "expected_intent_sha256": intent,
              "expected_output_sha256": output,
              "evidence_sha256": hashlib.sha256(b"synthetic evidence, not live approval").hexdigest(),
              "provider_quiescent_observed": True}
    request = root / "review.json"
    write_json(request, review)
    before = file_hashes(f.workspace)
    accepted = lambda: native(binary, manifest, f, "--stage-uncertain-study-review", request)
    with ThreadPoolExecutor(max_workers=3) as pool:
        results = list(pool.map(lambda _: accepted(), range(3)))
    for result in results:
        require(result["status"] == "staged_not_applied" and result["job_id"] == job_id,
                "staging falsely applied or changed the selected job")
        for flag in ("delivery_verified", "generation_retried", "next_dispatched"):
            require(result[flag] is False, "staging claimed delivery or execution")
    after = file_hashes(f.workspace)
    require(all(after.get(key) == value for key, value in before.items()),
            "staging changed preexisting synthetic authored/input state")
    new = set(after) - set(before)
    expected_path = handoff / f"review-{job_id}-r7.json"
    require(new == {str(expected_path.relative_to(f.workspace))},
            "staging wrote something other than the exact review")
    require(expected_path.stat().st_mode & 0o777 == 0o600, "review file is not private")
    require(json.loads(expected_path.read_bytes()) == review, "staged review changed")

    conflict = root / "conflict.json"
    write_json(conflict, {**review, "operation_id": "conflicting-retry"})
    native(binary, manifest, f, "--stage-uncertain-study-review", conflict,
           expected="conflicting review retry")
    require(file_hashes(f.workspace) == after, "conflicting retry changed state")
    stale = root / "stale.json"
    write_json(stale, {**review, "expected_selection_revision": 8})
    native(binary, manifest, f, "--stage-uncertain-study-review", stale,
           expected="activity revision changed")
    busy = root / "unverified-quiescence.json"
    write_json(busy, {**review, "provider_quiescent_observed": False})
    native(binary, manifest, f, "--stage-uncertain-study-review", busy,
           expected="operator must verify provider quiescence")
    invalid = root / "unknown-private-field.json"
    write_json(invalid, {**review, "unexpected_private_prose": SENTINEL})
    native(binary, manifest, f, "--stage-uncertain-study-review", invalid,
           expected="invalid review JSON")
    require(file_hashes(f.workspace) == after, "rejected reviews changed retained state")

    # Synthetic terminal-state fixture, not a live mutation or proof of runtime
    # application. Rust transaction tests qualify the actual phase transition.
    review_hash = retain(handoff, review)
    state["study_handoff"]["active"] = None
    state["selection_revision"] = 8
    state["study_handoff"]["jobs"][job_id].update(
        phase="retired_unresolved", uncertainty_review=review_hash)
    atomic_json(state_path, state)
    retired = file_hashes(f.workspace)
    require(accepted()["status"] == "already_applied", "native retry did not recognize review")
    require(file_hashes(f.workspace) == retired, "retired retry mutated history")

    checkpoint = {"exchange_count": 3, "creative_temperature": 0.7, "response_length": 128,
                  "self_reflect_paused": False, "ears_closed": False, "senses_snoozed": False,
                  "recent_next_choices": [], "history": [], "activity": state}
    write_json(f.workspace / "state.json", checkpoint)
    # Both invoke the real runtime checkpoint decoder without starting services.
    # New-state decode reaches the independent, deliberately missing control gate.
    native(binary, manifest, f, "--verify-deployment-inputs",
           expected="staged activation requires existing valid self-control state")
    native(old_binary, old_manifest, f, "--verify-deployment-inputs",
           expected="invalid deployment checkpoint")
    require(file_hashes(f.workspace) == {**retired, "state.json": sha(f.workspace / "state.json")},
            "checkpoint inspection rewrote newer state")
    return {"synthetic_only": True, "native_cli_staging": True,
            "cross_process_identical_retry": True, "conflicting_retry_refused": True,
            "stale_revision_refused": True, "private_input_and_prose_preserved": True,
            "private_error_output": True, "terminal_retry_noop": True,
            "new_checkpoint_schema_decoded": True, "old_checkpoint_downgrade_refused": True,
            "runtime_retirement_executed": False, "services_started": False,
            "generation_retried": False, "delivery_verified": False}


def qualify(old, new, out):
    stages = {"old": verify_stage(old), "new": verify_stage(new)}
    sources = {name: sha(Path(__file__).with_name(name)) for name in (
        Path(__file__).name, "qualify_geometry_release.py", "bridge_stage.py")}
    require(sha(old / "helpers/astrid-source-study") == sha(new / "helpers/astrid-source-study"),
            "shared helper changed; this bridge-only qualifier cannot authorize migration")
    out.mkdir(mode=0o700)
    try:
        result = probe(old / "spectral-bridge-server", old / "manifest.json",
                       new / "spectral-bridge-server", new / "manifest.json",
                       new / "helpers/astrid-source-study", out / "synthetic")
        require(stages == {"old": verify_stage(old), "new": verify_stage(new)}, "stage drift")
        require(sources == {name: sha(Path(__file__).with_name(name)) for name in sources},
                "qualification source drift")
        receipt = dict(schema="private_delivery_recovery_qualification_v1", status="passed",
                       stages=stages, checks=result, qualifier_inputs=sources,
                       activation_performed=False, live_authority_granted=False,
                       live_eligible_now=False)
        write_json(out / "qualification.json", receipt)
        return receipt
    except Exception as error:
        write_json(out / "failure.json", dict(error=str(error), activation_performed=False))
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("old-stage", "new-stage", "out"):
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    result = qualify(args.old_stage.resolve(strict=True), args.new_stage.resolve(strict=True),
                     args.out.absolute())
    print(json.dumps(dict(status=result["status"], receipt=str(args.out / "qualification.json"),
                          activation_performed=False)))


if __name__ == "__main__":
    main()
