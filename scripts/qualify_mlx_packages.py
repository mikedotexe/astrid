#!/usr/bin/env python3
"""Run matched, isolated MLX package trials after a completed passive baseline.

Only owned candidates on 8092 and their private reservoir fixture are started.
No production request, launchd configuration, or live-state write is performed.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
import math
from pathlib import Path
import statistics
import subprocess
import time
import urllib.error
import urllib.request

import astrid_model_canary as canary
from astrid_mlx_candidate import fixture_inputs
from astrid_mlx_memory import (Monitor, STRICT_POLICY, SWAP_DIAGNOSTIC_POLICY,
                              STAGE2_DIAGNOSTIC_POLICY, POLICIES)
from astrid_soak_observer import review_observation
from model_environment import validate_manifest
from prepare_mlx_environments import INVENTORY, digest


ENDPOINT = "http://127.0.0.1:8092"
PRE_PROMOTION_BASELINE_MIN_SECONDS = 3600
QUALIFICATION_STAGES = {"core": {"baseline", "core"}, "full": {"core", "full"}}


def harness_hashes():
    names = ("qualify_mlx_packages.py", "qualify_mlx_offline.py", "astrid_model_canary.py",
             "astrid_mlx_candidate.py", "astrid_mlx_fixture.py", "astrid_mlx_memory.py",
             "astrid_soak_observer.py", "prepare_mlx_environments.py", "model_environment.py",
             "astrid_mlx_embedding_gate.py", "astrid_mlx_generator_contract.py",
             "stage2_wide_exception.py", "model_qualification.py",
             "mlx_0323_offline_harness_compatibility.json")
    return {name: digest(Path(__file__).with_name(name)) for name in names}


# One-time compatibility is deliberately limited to these immutable passing records.
LEGACY_OFFLINE = {'baseline': {'path': '/Users/v/other/astrid/.runtime/mlx-upgrade/alternative-0323-01/source-rebind-01/qualification-02/controlled-window-02/offline-baseline-01/offline.json', 'sha256': 'e87b27614f03276c81a113b47c496989dbfe06d51188c172586bc1537dd197eb'}, 'core': {'path': '/Users/v/other/astrid/.runtime/mlx-upgrade/alternative-0323-01/source-rebind-01/qualification-02/controlled-window-02/offline-core-01/offline.json', 'sha256': 'dbc475313e1f18a35b3d5a481d8a665d28811647a8e40a8f9512a93df9a507be'}}
DIAGNOSTIC_PREPARATION = "5a1658026900383579d970b405a92f8bc9f318f0b2d4a6f8c0d7e2ec9c2ebd24"
DIAGNOSTIC_BASELINE = "aad3884591a6200ba9b417acd4fc06cde051e3ff1409df16ddbf1509492aefd5"
DIAGNOSTIC_MANIFESTS = {
    "baseline": "b786f6e072fd486581877a0e76f0d72014a6abed155ad3f01dd64ca6da214503",
    "core": "7e2b8894de5295a54609befe831a12593f94cd53b3274cbbabb338d40f22b1c7",
}
STAGE2_PREPARATION = "a045d5d1243488f63213a14c636727d2af5d0b5aef26a16fb628d744687f88e3"
STAGE2_MANIFESTS = {
    "core": "7e2b8894de5295a54609befe831a12593f94cd53b3274cbbabb338d40f22b1c7",
    "full": "42936d2ba3d0f90bbf2903ee14380fab2004723a4052799e2fff1c5a5389c98a",
}
COMPATIBILITY_FILE = "mlx_0323_offline_harness_compatibility.json"


def validate_memory_policy(policy, *, preparation_hash, baseline_hash,
                           manifest_hash, stage, qualification_target):
    if policy == STRICT_POLICY:
        return
    if policy == STAGE2_DIAGNOSTIC_POLICY:
        if (qualification_target != "full" or stage not in STAGE2_MANIFESTS
                or preparation_hash != STAGE2_PREPARATION
                or manifest_hash != STAGE2_MANIFESTS[stage]):
            raise ValueError("Stage 2 diagnostic memory policy requires exact 0.32.3 full campaign")
        return
    if (policy != SWAP_DIAGNOSTIC_POLICY or qualification_target != "core"
            or stage not in DIAGNOSTIC_MANIFESTS
            or preparation_hash != DIAGNOSTIC_PREPARATION
            or baseline_hash != DIAGNOSTIC_BASELINE
            or manifest_hash != DIAGNOSTIC_MANIFESTS[stage]):
        raise ValueError("diagnostic memory policy is limited to the reviewed 0.32.3 core campaign")


def offline_harness_matches(receipt, receipt_ref, current):
    if receipt.get("harness_hashes") == current:
        return True
    # Only these two immutable strict-policy offline receipts cross this boundary.
    stage = receipt.get("stage")
    if (stage not in LEGACY_OFFLINE or receipt_ref != LEGACY_OFFLINE[stage]
            or receipt.get("schema") != "mlx_offline_qualification_v1"
            or receipt.get("passed") is not True or receipt.get("qualification_target") != "core"
            or receipt.get("predecessor_acceptance") is not None
            or receipt.get("memory_policy", STRICT_POLICY) != STRICT_POLICY
            or receipt.get("host_memory", {}).get("policy", STRICT_POLICY) != STRICT_POLICY
            or receipt.get("preparation_sha256") != DIAGNOSTIC_PREPARATION
            or receipt.get("baseline_soak_sha256") != DIAGNOSTIC_BASELINE
            or receipt.get("manifest_sha256") != DIAGNOSTIC_MANIFESTS[stage]):
        return False
    path = Path(__file__).with_name(COMPATIBILITY_FILE)
    raw = path.read_bytes()
    bridge = json.loads(raw)
    old = receipt["harness_hashes"]
    allowed_changes = {"qualify_mlx_packages.py", "astrid_mlx_memory.py"}
    return (bridge.get("schema") == "mlx_0323_exact_offline_harness_compatibility_v1"
            and bridge.get("legacy_receipts") == LEGACY_OFFLINE
            and bridge.get("legacy_harness") == old
            and set(current) == set(old) | {COMPATIBILITY_FILE}
            and current[COMPATIBILITY_FILE] == hashlib.sha256(raw).hexdigest()
            and bridge.get("current_harness") == {k: v for k, v in current.items()
                                                   if k != COMPATIBILITY_FILE}
            and all(current[k] == old[k] for k in old if k not in allowed_changes))


def messages(text):
    return [{"role": "system", "content": "Follow the user's instructions carefully. Do not include hidden reasoning or model control tokens."},
            {"role": "user", "content": text}]


def workloads():
    rows = "\n".join(f"Observation {i:04d}: The lake gauge is stable, the footpath remains open, and the weather station reports a light west wind."
                     for i in range(480))
    return [
        {"id": "exact", "max_tokens": 32, "deadline_s": 240, "messages": messages("Reply with exactly MLX_CANARY_OK and nothing else."), "expected": "MLX_CANARY_OK"},
        {"id": "next_contract", "max_tokens": 128, "deadline_s": 240,
         "messages": messages("In two sentences describe a quiet morning beside a lake. Finish with exactly one final line: NEXT: LISTEN"), "next": True},
        {"id": "dialogue", "max_tokens": 384, "deadline_s": 240,
         "messages": messages("I have half an hour before meeting a friend and feel scattered. Suggest a calm, practical way to use that time in about 150 words.")},
        {"id": "long_context", "max_tokens": 256, "deadline_s": 600,
         "messages": messages(rows + "\nSummarize the observations in three concise paragraphs. State the range of observation numbers and avoid inventing changes.")},
        {"id": "extended_writing", "max_tokens": 1200, "deadline_s": 1200, "min_words": 350,
         "messages": messages("Write a complete, coherent short story of about 500 words about a lighthouse keeper repairing an old radio. Use concrete sensory details and a clear ending. Return only the story.")},
    ]


def sha(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def percentile(values, fraction=.95):
    if not values:
        raise ValueError("cannot score missing performance samples")
    ordered = sorted(values)
    return ordered[max(0, math.ceil(len(ordered) * fraction) - 1)]


def summarize(samples, repetitions):
    result = {}
    for workload in workloads():
        rows = [row for row in samples if row["workload_id"] == workload["id"]]
        if len(rows) != repetitions:
            raise ValueError(f"incomplete workload: {workload['id']}")
        prompt_identities = {(row['request'].get('prompt_sha256_12'),
                              row['usage']['prompt_tokens'], row['request']['temperature']) for row in rows}
        if len(prompt_identities) != 1 or None in next(iter(prompt_identities)):
            raise ValueError(f"rendered prompt changed across trials: {workload['id']}")
        rendered_hash, prompt_tokens, temperature = next(iter(prompt_identities))
        def values(key):
            numbers = [row["generation"].get(key) for row in rows]
            if any(isinstance(x, bool) or not isinstance(x, (float, int)) or not math.isfinite(x) or x <= 0 for x in numbers):
                raise ValueError(f"missing/nonpositive metric {key}: {workload['id']}")
            return numbers
        if max(values("total_turn_s")) > workload["deadline_s"]:
            raise ValueError(f"request-specific deadline exceeded: {workload['id']}")
        result[workload["id"]] = {
            "messages_sha256": sha(workload["messages"]), "count": len(rows),
            "max_tokens": workload["max_tokens"], "deadline_s": workload["deadline_s"],
            "rendered_prompt_sha256_12": rendered_hash, "prompt_tokens": prompt_tokens,
            "temperature": temperature, "lane": "qualification",
            "p95_total_turn_s": percentile(values("total_turn_s")),
            "p95_first_token_s": percentile(values("first_token_s")),
            "steady_tok_s": statistics.median(values("steady_tok_s")),
            "peak_memory_bytes": max(values("peak_memory_bytes")),
            "completion_tokens": [row["usage"]["completion_tokens"] for row in rows],
        }
    return result


def http_json(path, payload=None, timeout=5):
    data = None if payload is None else json.dumps(payload).encode()
    request = urllib.request.Request(ENDPOINT + path, data=data,
                                     headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(request, timeout=timeout) as response:
        return json.load(response)


def wait_ready(proc, deadline_s=600):
    deadline = time.monotonic() + deadline_s
    while time.monotonic() < deadline:
        if proc.poll() is not None:
            raise RuntimeError(f"candidate exited during load: {proc.returncode}")
        try:
            health = http_json("/readyz")
            if health.get("ready") and health.get("worker", {}).get("phase") == "ready":
                if health.get("reservoir", {}).get("status") != "connected":
                    raise RuntimeError("private reservoir is not connected")
                validate_ownership(proc)
                return
        except (OSError, ValueError, urllib.error.URLError):
            pass
        time.sleep(1)
    raise RuntimeError("candidate readiness deadline exceeded")


def validate_ownership(proc):
    if proc.poll() is not None or any(canary.port_pids(port) != [str(proc.pid)] for port in (8092, 17881)):
        raise RuntimeError("candidate no longer owns both private listener ports")


def validate_output(workload, response):
    text = response["choices"][0]["message"]["content"]
    failures = []
    if not text.strip():
        failures.append("empty_output")
    if canary.ARTIFACT_RE.search(text):
        failures.append("raw_model_artifact")
    if workload.get("expected") and text.strip() != workload["expected"]:
        failures.append("exact_output")
    if workload.get("next"):
        lines = [line.strip() for line in text.splitlines() if line.strip()]
        if text.count("NEXT:") != 1 or not lines or lines[-1] != "NEXT: LISTEN":
            failures.append("next_contract")
    evidence = response.get("coupled_generation_v1") or {}
    usage = response.get("usage") or {}
    termination = evidence.get("termination") or {}
    controls = evidence.get("controls") or {}
    if (controls.get("thinking") is not False or controls.get("wide_coupling_strength") != 0.
            or controls.get("temperature") != 0. or controls.get("sampling_mode") != "greedy"
            or controls.get("processor_order") != ["reservoir", "additional_repetition"]
            or ("max_tokens" in workload and controls.get("max_tokens") != workload["max_tokens"])):
        failures.append("generation_controls_changed")
    if (evidence.get("schema") != "coupled_generation_v1"
            or evidence.get("finish_reason") != response["choices"][0].get("finish_reason")
            or evidence.get("completion_tokens") != usage.get("completion_tokens")
            or evidence.get("prompt_tokens") != usage.get("prompt_tokens")):
        failures.append("inconsistent_native_completion_evidence")
    if (response["choices"][0].get("finish_reason") != "stop"
            or termination.get("kind") != "model_eos"
            or termination.get("model_eos_reached") is not True):
        failures.append("workload_did_not_complete_at_native_eos")
    if len(text.split()) < workload.get("min_words", 1):
        failures.append("incomplete_requested_output")
    return failures


def validate_reservoir(audit, health):
    failures = []
    if (audit.get("reservoir") or {}).get("checkout_succeeded") is not True:
        failures.append("reservoir_checkout_failed_or_missing")
    if (health.get("reservoir", {}).get("status") != "connected"
            or health.get("reservoir", {}).get("operation") != "push_state"):
        failures.append("reservoir_sync_failed")
    return failures


def validate_initial_coupling(workload_index, response, restored_strength):
    if workload_index != 0:
        return []  # Preserve normal adaptation after the first request.
    actual = (response.get("coupled_generation_v1") or {}).get("controls", {}).get("reservoir_coupling_strength")
    if (isinstance(actual, bool) or not isinstance(actual, (int, float))
            or not math.isfinite(actual) or actual != restored_strength):
        return ["initial_restored_coupling_mismatch"]
    return []


def validate_baseline(path, *, stage, qualification_target, memory_policy=STRICT_POLICY):
    if stage not in QUALIFICATION_STAGES.get(qualification_target, set()):
        raise ValueError("environment stage differs from qualification target")
    minimum = (1800 if qualification_target == "full" and memory_policy == STAGE2_DIAGNOSTIC_POLICY
               else PRE_PROMOTION_BASELINE_MIN_SECONDS if qualification_target == "core" else 7200)
    record = json.loads(path.read_text())
    if (record.get("mode") != "observe_only" or record.get("state") != "completed"
            or record.get("duration_s", 0) < minimum
            or record.get("summary", {}).get("automated_ok") is not True
            or review_observation(record)["passed"] is not True):
        duration = "30-minute" if minimum == 1800 else "two-hour" if minimum == 7200 else "one-hour"
        raise ValueError(f"a completed clean {duration} passive baseline is required")
    return hashlib.sha256(path.read_bytes()).hexdigest()


def validate_predecessor_for_work_legacy(args, baseline_hash, preparation):
    """Admit Stage 2 work only against the accepted core activation and soak."""
    names = ("core_manifest", "core_proof", "activation_receipt", "supervised_watch",
             "supervised_review", "passive_review")
    supplied = {name: getattr(args, name, None) for name in names}
    if args.qualification_target == "core":
        if any(supplied.values()):
            raise ValueError("core qualification cannot borrow Stage 2 predecessor evidence")
        return None
    if args.qualification_target != "full" or not all(supplied.values()):
        raise ValueError("full qualification requires all accepted core predecessor artifacts")
    from model_qualification import reference, verify_predecessor, load_reference, interval
    core_manifest = supplied["core_manifest"]
    reference(core_manifest)  # Canonical, regular, bounded artifact before other reads.
    core_release = validate_manifest(core_manifest)
    prepared_core = preparation["stages"]["core"]
    manifest = core_release["manifest"]
    if (any(manifest.get(key) != prepared_core.get(key)
            for key in ("packages", "python", "source_root"))
            or core_release["model"] != preparation["model_snapshot"]):
        raise ValueError("accepted core predecessor differs from prepared Stage 2 baseline")
    refs = {name: reference(supplied[name]) for name in names if name != "core_manifest"}
    refs["passive_soak"] = reference(args.baseline_soak)
    if refs["passive_soak"]["sha256"] != baseline_hash:
        raise ValueError("Stage 2 baseline changed during predecessor admission")
    accepted = verify_predecessor({"predecessor": refs}, core_manifest, core_release)
    if args.stage == "core" and reference(args.manifest) != accepted["manifest"]:
        raise ValueError("selected Stage 2 core manifest differs from accepted predecessor")
    if interval(load_reference(refs["passive_soak"]))[1] > datetime.now(timezone.utc).timestamp():
        raise ValueError("core predecessor soak has not completed at admission time")
    return accepted



def validate_predecessor_for_work_patch(args, baseline_hash, preparation):
    """Admit Stage 2 against exact core proof, activation and accepted watch."""
    names = ("core_manifest", "core_proof", "activation_receipt", "supervised_watch",
             "supervised_review")
    supplied = {name: getattr(args, name, None) for name in names}
    if args.qualification_target == "core":
        if any(supplied.values()):
            raise ValueError("core qualification cannot borrow Stage 2 predecessor evidence")
        return None
    if args.qualification_target != "full" or not all(supplied.values()):
        raise ValueError("full qualification requires all accepted core predecessor artifacts")
    from model_qualification import reference, verify_predecessor, load_reference, interval
    core_manifest = supplied["core_manifest"]
    reference(core_manifest)  # Canonical, regular, bounded artifact before other reads.
    core_release = validate_manifest(core_manifest)
    prepared_core = preparation["stages"]["core"]
    manifest = core_release["manifest"]
    if (any(manifest.get(key) != prepared_core.get(key)
            for key in ("packages", "python", "source_root"))
            or core_release["model"] != preparation["model_snapshot"]):
        raise ValueError("accepted core predecessor differs from prepared Stage 2 baseline")
    refs = {name: reference(supplied[name]) for name in names if name != "core_manifest"}
    if refs["supervised_watch"]["sha256"] != baseline_hash:
        raise ValueError("Stage 2 baseline must be the accepted core watch")
    accepted = verify_predecessor({"predecessor": refs}, core_manifest, core_release)
    if args.stage == "core" and reference(args.manifest) != accepted["manifest"]:
        raise ValueError("selected Stage 2 core manifest differs from accepted predecessor")
    if interval(load_reference(refs["supervised_watch"]))[1] > datetime.now(timezone.utc).timestamp():
        raise ValueError("core predecessor watch has not completed at admission time")
    return accepted


def validate_predecessor_for_work(args, baseline_hash, preparation):
    if getattr(args, "memory_policy", STRICT_POLICY) == STAGE2_DIAGNOSTIC_POLICY:
        return validate_predecessor_for_work_patch(args, baseline_hash, preparation)
    return validate_predecessor_for_work_legacy(args, baseline_hash, preparation)


def validate_stage(preparation, name):
    stage = preparation["stages"][name]
    source = Path(stage["source_root"])
    if {str(p.relative_to(source)): digest(p) for p in source.glob("*.py")} != preparation["source_hashes"]:
        raise ValueError("prepared source closure changed")
    actual = json.loads(subprocess.check_output([stage["python"], "-B", "-c", INVENTORY], text=True))
    if actual["packages"] != stage["packages"] or actual["python"] != "3.12.13":
        raise ValueError("prepared Python package inventory changed")
    return stage


def validate_release(path, preparation, name):
    checked = validate_manifest(path)
    manifest = checked["manifest"]
    stage = preparation["stages"][name]
    if (manifest["packages"] != stage["packages"] or manifest["python"] != stage["python"]
            or manifest["source_root"] != stage["source_root"]
            or checked["model"] != preparation["model_snapshot"]):
        raise ValueError("release manifest differs from the prepared stage")
    validate_stage(preparation, name)
    return checked["manifest_sha256"]


def validate_offline(path, preparation, name, manifest_hash, baseline_hash,
                     qualification_target, predecessor_acceptance):
    receipt_hash = digest(path)
    receipt = json.loads(path.read_text())
    if digest(path) != receipt_hash:
        raise ValueError("offline receipt changed during admission")
    if (receipt.get("schema") != "mlx_offline_qualification_v1" or receipt.get("stage") != name
            or receipt.get("qualification_target") != qualification_target
            or receipt.get("predecessor_acceptance") != predecessor_acceptance
            or receipt.get("passed") is not True or not receipt.get("completed_at")
            or receipt.get("manifest_sha256") != manifest_hash
            or receipt.get("baseline_soak_sha256") != baseline_hash
            or not offline_harness_matches(receipt, {"path": str(path.absolute()),
                                                     "sha256": receipt_hash}, harness_hashes())
            or receipt.get("packages") != preparation["stages"][name]["packages"]
            or receipt.get("source_hashes") != preparation["source_hashes"]):
        raise ValueError("passing offline qualification for this exact stage is required")
    return receipt_hash


def run(args):
    memory_policy = getattr(args, "memory_policy", STRICT_POLICY)
    baseline_hash = validate_baseline(args.baseline_soak, stage=args.stage,
                                      qualification_target=args.qualification_target,
                                      memory_policy=memory_policy)
    preparation = json.loads(args.preparation.read_text())
    predecessor = validate_predecessor_for_work(args, baseline_hash, preparation)
    if not preparation.get("live_environment_unchanged") or not preparation.get("completed_at"):
        raise ValueError("environment preparation is incomplete")
    stage = validate_stage(preparation, args.stage)
    manifest_hash = validate_release(args.manifest, preparation, args.stage)
    validate_memory_policy(memory_policy, preparation_hash=digest(args.preparation),
                           baseline_hash=baseline_hash, manifest_hash=manifest_hash,
                           stage=args.stage, qualification_target=args.qualification_target)
    offline_hash = validate_offline(args.offline_receipt, preparation, args.stage, manifest_hash,
                                   baseline_hash, args.qualification_target, predecessor)
    fixture, fixture_bytes = fixture_inputs(args.frozen_fixture_dir)
    journal = json.loads(fixture_bytes["coupling_journal.json"])
    restored_strength = journal["entries"][-1].get("coupling_strength", .1) if journal["entries"] else .1
    if args.output_dir.exists():
        raise ValueError("output already exists; preserve previous evidence")
    args.output_dir.mkdir(parents=True, mode=0o700)
    record = {"schema": "mlx_matched_package_qualification_v1", "stage": args.stage,
              "qualification_target": args.qualification_target, "memory_policy": memory_policy,
              "predecessor_acceptance": predecessor,
              "baseline_soak_sha256": baseline_hash, "repetitions": args.repetitions,
              "started_at": datetime.now(timezone.utc).isoformat(),
              "manifest_sha256": manifest_hash, "preparation_sha256": digest(args.preparation),
              "harness_hashes": harness_hashes(),
              "offline_receipt_sha256": offline_hash,
              "fixture": fixture, "fixture_manifest_sha256": digest(args.frozen_fixture_dir / "fixture.json"),
              "configured_coupling_strength": .1, "initial_restored_coupling_strength": restored_strength,
              "trial_reset": "snapshot and journal reset per repetition; unique handles per workload; identical ordered workload batches preserve AGC history within batch",
              "ordered_workloads": [item["id"] for item in workloads()],
              "source_hashes": preparation["source_hashes"], "packages": stage["packages"],
              "model_snapshot": preparation["model_snapshot"], "workload_sha256": sha(workloads()),
              "samples": [], "state": "running", "production_requests": 0}
    record["trial_evidence"] = []
    output = args.output_dir / "qualification.json"
    def checkpoint():
        output.write_text(json.dumps(record, indent=2, sort_keys=True) + "\n")
    checkpoint()
    memory = Monitor(policy=memory_policy)
    try:
        memory.start()
        for repetition in range(args.repetitions):
            validate_stage(preparation, args.stage)
            trial = args.output_dir / f"trial-{repetition + 1:02d}"
            proc, log_path = canary.start_candidate_server(
                reservoir_root=Path(stage["source_root"]), python=stage["python"],
                model=preparation["model_snapshot"], port=8092, coupling_strength=.1,
                wide_coupling_strength=0., output_dir=trial,
                frozen_fixture_dir=args.frozen_fixture_dir)
            try:
                wait_ready(proc)
                package_path = trial / "candidate-runtime/packages.json"
                packages = json.loads(package_path.read_text())
                if {n: p['version'] for n, p in packages['packages'].items()} != stage['packages']:
                    raise ValueError("loaded candidate package inventory differs from the stage")
                package_hash = digest(package_path)
                for workload_index, workload in enumerate(workloads()):
                    validate_ownership(proc)
                    handle = f"qualification_{workload['id']}"
                    response = http_json("/v1/chat/completions", {
                        "model": "coupled-astrid", "messages": workload["messages"],
                        "max_tokens": workload["max_tokens"], "temperature": 0.,
                        "reservoir_handle": handle,
                    }, timeout=workload["deadline_s"])
                    metrics = trial / "request_metrics/coupled_request_metrics.jsonl"
                    rows = [json.loads(line) for line in metrics.read_text().splitlines()]
                    row = next(row for row in reversed(rows) if row["request"]["handle_name"] == handle)
                    if row["request"]["messages_sha256"] != sha(workload["messages"]):
                        raise ValueError("audit request identity mismatch")
                    if row["runtime"].get("candidate_packages_sha256") != package_hash:
                        raise ValueError("audit package identity mismatch")
                    failures = validate_output(workload, response)
                    failures.extend(validate_initial_coupling(workload_index, response, restored_strength))
                    if row["generation"].get("raw_model_artifact_detected") is not False:
                        failures.append("raw_model_artifact_or_missing_raw_evidence")
                    health = http_json("/readyz")
                    failures.extend(validate_reservoir(row, health))
                    record["samples"].append({"workload_id": workload["id"], "repetition": repetition,
                                              "generation": row["generation"], "request": row["request"],
                                              "reservoir_audit": row.get("reservoir"), "readyz_after": health,
                                              "usage": response["usage"],
                                              "response": response, "failures": failures})
                    checkpoint()
                    print(f"{args.stage} trial {repetition+1}: {workload['id']} failures={failures}", flush=True)
                    if failures:
                        raise RuntimeError(f"candidate output gate failed: {failures}")
            finally:
                canary.stop_candidate_server(proc, timeout_s=1250.)
                if proc.poll() is None:
                    raise RuntimeError("candidate did not exit; do not start another")
                record["trial_evidence"].append({"repetition": repetition, "pid": proc.pid,
                    "exit_code": proc.returncode, "log": str(log_path), "log_sha256": digest(log_path),
                    "metrics": str(trial / "request_metrics/coupled_request_metrics.jsonl"),
                    "metrics_sha256": digest(trial / "request_metrics/coupled_request_metrics.jsonl")
                        if (trial / "request_metrics/coupled_request_metrics.jsonl").is_file() else None})
                checkpoint()
                if proc.returncode != 0:
                    raise RuntimeError(f"candidate exited unsuccessfully: {proc.returncode}")
        record["matched_workloads"] = summarize(record["samples"], args.repetitions)
        validate_release(args.manifest, preparation, args.stage)
        if record["harness_hashes"] != harness_hashes():
            raise ValueError("qualification harness changed during trials")
        record["state"] = "completed"
        record["summary"] = {"automated_ok": True, "operator_review_required": True}
    except BaseException as error:
        record["state"] = "failed"
        record["summary"] = {"automated_ok": False, "failure_reasons": [str(error)]}
        raise
    finally:
        record["host_memory"] = memory.stop()
        if not record["host_memory"]["automated_ok"]:
            record["state"] = "failed"
            reasons = record.get("summary", {}).get("failure_reasons", [])
            record["summary"] = {"automated_ok": False, "failure_reasons": reasons + record["host_memory"]["failure_reasons"]}
        record["completed_at"] = datetime.now(timezone.utc).isoformat()
        checkpoint()
    return 0 if record["state"] == "completed" else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--preparation", type=Path, required=True)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--baseline-soak", type=Path, required=True)
    parser.add_argument("--offline-receipt", type=Path, required=True)
    parser.add_argument("--stage", choices=("baseline", "core", "full"), required=True)
    parser.add_argument("--qualification-target", choices=("core", "full"), required=True)
    parser.add_argument("--memory-policy", choices=POLICIES, default=STRICT_POLICY)
    parser.add_argument("--core-manifest", type=Path)
    parser.add_argument("--core-proof", type=Path)
    parser.add_argument("--activation-receipt", type=Path)
    parser.add_argument("--supervised-watch", type=Path)
    parser.add_argument("--supervised-review", type=Path)
    parser.add_argument("--passive-review", type=Path)
    parser.add_argument("--frozen-fixture-dir", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--repetitions", type=int, default=3)
    args = parser.parse_args()
    if args.repetitions < 3 or args.repetitions > 10:
        parser.error("qualification requires 3..10 matched repetitions")
    return run(args)


if __name__ == "__main__":
    raise SystemExit(main())
