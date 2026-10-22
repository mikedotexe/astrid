#!/usr/bin/env python3
"""Build/check immutable qualification evidence before any provider activation.

Verification only reads retained artifacts and release manifests. It never runs
an interpreter, imports MLX, contacts a service, or creates approval evidence.
Output review records an actual human or agent assessment of retained samples;
it is a qualification gate, not a separate deployment authorization.
"""
from __future__ import annotations

import argparse
import base64
import csv
from copy import deepcopy
from datetime import datetime, timezone
import hashlib
import json
import math
from pathlib import Path
import re
from typing import Any

from astrid_mlx_memory import (evaluate as evaluate_memory, STRICT_POLICY,
                              SWAP_DIAGNOSTIC_POLICY, STAGE2_DIAGNOSTIC_POLICY)
from astrid_soak_observer import health_failures, review_observation
from astrid_soak_performance import compare_performance
from model_environment import validate_manifest


SCHEMA = "model_qualification_v1"
REVIEW_SCHEMA = "model_qualification_output_review_v1"
OFFLINE_TESTS = {"contracts", "generator-contract", "reservoir-equivalence", "multi-headed", "wide-coupling"}
PRE_PROMOTION_BASELINE_MIN_SECONDS = 3600


def baseline_minimum_for_stage(stage: str) -> int:
    """Historical default; prospective patch policy uses the reviewed watch."""
    require(stage in {"core", "full"}, "unsupported candidate stage")
    return PRE_PROMOTION_BASELINE_MIN_SECONDS if stage == "core" else 7200


def require(condition: Any, message: str) -> None:
    if not condition:
        raise ValueError(message)


def read_artifact(path: Path) -> tuple[bytes, str]:
    require(path.is_absolute() and path.resolve() == path and not path.is_symlink()
            and path.is_file(), f"artifact must be a canonical regular file: {path}")
    require(path.stat().st_size <= 128 * 1024 * 1024, f"artifact too large: {path}")
    raw = path.read_bytes()
    return raw, hashlib.sha256(raw).hexdigest()


def reference(path: Path) -> dict[str, str]:
    path = path.absolute()
    _, fingerprint = read_artifact(path)
    return {"path": str(path), "sha256": fingerprint}


def load_reference(ref: dict[str, Any], *, object_required: bool = True) -> Any:
    require(isinstance(ref, dict) and isinstance(ref.get("path"), str), "artifact reference missing")
    raw, actual = read_artifact(Path(ref["path"]))
    require(re.fullmatch(r"[0-9a-f]{64}", str(ref.get("sha256", "")))
            and ref["sha256"] == actual, f"artifact hash changed: {ref['path']}")
    if not object_required:
        return raw
    result = json.loads(raw)
    require(isinstance(result, dict), f"artifact is not an object: {ref['path']}")
    return result


def timestamp(value: Any) -> float:
    require(isinstance(value, str), "missing evidence timestamp")
    parsed = datetime.fromisoformat(value)
    require(parsed.tzinfo is not None, "evidence timestamps require a timezone")
    return parsed.timestamp()


def finite(value: Any) -> bool:
    return isinstance(value, (int, float)) and not isinstance(value, bool) and math.isfinite(value)


def interval(record: dict[str, Any]) -> tuple[float, float]:
    start, end = timestamp(record.get("started_at")), timestamp(record.get("completed_at"))
    require(end > start, "evidence time interval is empty or reversed")
    return start, end


def verify_baseline(record: dict[str, Any], review: dict[str, Any], baseline_hash: str,
                    *, minimum_seconds: int = 7200) -> float:
    require(record.get("mode") == "observe_only" and record.get("state") == "completed"
            and record.get("live_mutations") == [] and record.get("probes") == []
            and record.get("started_candidate") is False and record.get("restore") is None,
            "a completed passive baseline with zero live mutations is required")
    start, end = interval(record)
    require(finite(record.get("duration_s")) and record["duration_s"] >= minimum_seconds
            and end - start >= minimum_seconds,
            "baseline must cover at least two hours" if minimum_seconds == 7200
            else f"observation must cover at least {minimum_seconds} seconds")
    require(record.get("summary", {}).get("automated_ok") is True
            and record["summary"].get("failure_reasons") == [], "baseline health gate failed")
    require(review.get("schema") == "astrid_observation_provenance_review_v1"
            and review.get("source_record_sha256") == baseline_hash
            and review.get("passed") is True and review.get("qualified_observation") is True
            and review.get("host_reinspection_performed") is False,
            "retrospective baseline identity review is missing or failed")
    require(review_observation(record)["passed"], "retained baseline identity evidence is incomplete")
    samples = record["samples"]
    times = [sample.get("t_s") for sample in samples]
    require(all(finite(t) for t in times) and len(times) >= minimum_seconds // 60 + 1
            and 0 <= times[0] - start <= 60 and 0 <= end - times[-1] <= 60
            and all(0 < b - a <= 60 for a, b in zip(times, times[1:])),
            "baseline sampling coverage is incomplete")
    require(all(not health_failures(sample) for sample in samples), "retained baseline health sample failed")
    logs = record.get("log_sources", {})
    require({"bridge", "model"} <= set(logs)
            and all(cursor.get("discontinuities") == 0 and cursor.get("read_errors") == 0
                    for cursor in logs.values()), "baseline log evidence is incomplete")
    monitor = record.get("monitor", {})
    require(monitor.get("automated_ok") is True and monitor.get("failure_reasons") == [],
            "baseline monitor failed")
    for key in ("bridge_artifact_strip_count", "fallback_count", "malformed_next_count", "artifact_count"):
        require(monitor.get("events", {}).get(key) == 0, f"baseline incident: {key}")
    for key in ("artifact_count", "deprecated_language_count", "explore_action_count", "malformed_next_count"):
        require(monitor.get("generated_output_scan", {}).get("counts", {}).get(key) == 0,
                f"baseline generated output gate missing/failed: {key}")
    require(monitor.get("generation_stats", {}).get("count", 0) > 0
            or monitor.get("request_metrics", {}).get("count", 0) > 0, "baseline observed no generations")
    return end


def verify_preparation(preparation: dict[str, Any], inventory: dict[str, Any],
                       inventory_ref: dict[str, str], baseline: dict[str, Any],
                       releases: dict[str, Any], stage: str) -> None:
    require(preparation.get("schema") == "mlx_environment_preparation_v1"
            and preparation.get("live_environment_unchanged") is True,
            "preparation is incomplete")
    interval(preparation)
    source_hashes = preparation.get("source_hashes")
    require(isinstance(source_hashes, dict) and source_hashes, "prepared source closure is missing")
    identities = {item["path"]: item["sha256"] for item in baseline["identity_start"]}
    require(identities.get(inventory_ref["path"]) == inventory_ref["sha256"],
            "live package inventory was not observed during the baseline")
    baseline_stage = "baseline" if stage == "core" else "core"
    observed_source = (preparation["source_root"] if stage == "core"
                       else preparation["stages"]["core"]["source_root"])
    observed_python = (preparation.get("baseline_python") if stage == "core"
                       else preparation["stages"]["core"]["python"])
    for relative, expected in source_hashes.items():
        require(not Path(relative).is_absolute() and len(Path(relative).parts) == 1
                and relative.endswith(".py"), "unexpected prepared source path")
        require(identities.get(str(Path(observed_source) / relative)) == expected,
                f"source was not observed during the baseline: {relative}")
    require(inventory.get("packages") == preparation.get("stages", {}).get(baseline_stage, {}).get("packages")
            and inventory.get("executable") == observed_python,
            "baseline live package inventory differs from preserved release")
    # Metadata file identities, not just an inventory JSON, must be retained.
    metadata = {Path(path).parent.name for path in identities if Path(path).name == "METADATA"}
    for name, version in inventory["packages"].items():
        require(f"{name.replace('-', '_')}-{version}.dist-info" in metadata,
                f"live distribution identity missing: {name}")
    for name, release in releases.items():
        manifest = release["manifest"]
        prepared = preparation.get("stages", {}).get(baseline_stage if name == "baseline" else stage, {})
        require(all(manifest.get(key) == prepared.get(key) for key in ("python", "source_root", "packages")),
                f"{name} manifest differs from prepared stage")
        require(release["model"] == preparation.get("model_snapshot"), "model snapshot changed")
        actual = {str(path.relative_to(manifest["source_root"])): manifest["artifacts"].get(str(path))
                  for path in Path(manifest["source_root"]).glob("*.py")}
        require(actual == source_hashes, f"{name} source differs from baseline source")


def verify_receipt_identity(record: dict[str, Any], *, role: str, stage: str,
                            release: dict[str, Any], preparation: dict[str, Any],
                            preparation_hash: str, baseline_hash: str, baseline_end: float) -> None:
    start, _ = interval(record)
    require(start >= baseline_end, f"{role} qualification ran during the passive baseline")
    baseline_stage = "baseline" if stage == "core" else "core"
    require(record.get("stage") == (baseline_stage if role == "baseline" else stage)
            and record.get("qualification_target") == stage
            and record.get("manifest_sha256") == release["manifest_sha256"]
            and record.get("preparation_sha256") == preparation_hash
            and record.get("baseline_soak_sha256") == baseline_hash
            and record.get("packages") == release["manifest"]["packages"]
            and record.get("source_hashes") == preparation["source_hashes"]
            and record.get("model_snapshot") == release["model"],
            f"{role} receipt provenance does not match the selected release")
    if record.get("schema") == "mlx_offline_qualification_v1":
        require(record.get("source_root") == release["manifest"]["source_root"]
                and record.get("python") == release["manifest"]["python"],
                f"{role} offline source root or interpreter differs from selected release")
        if record.get("memory_policy") == STAGE2_DIAGNOSTIC_POLICY:
            require(record.get("selected_argv") == release["manifest"]["argv"],
                    "Stage 2 offline selected launch arguments differ from manifest")


def verify_embedding_audit(audit: dict[str, Any], record: dict[str, Any],
                           *, disabled_timing_exception: bool = False) -> None:
    sources = {"test_wide_coupling_offline.py", "coupled_astrid_server.py", "wide_coupling.py"}
    require(audit.get("schema") == "astrid_mlx_embedding_gate_v1"
            and audit.get("passed") is (not disabled_timing_exception)
            and audit.get("loader_calls") == 1
            and type(audit.get("loader_calls")) is int and type(audit.get("load_attempts")) is int
            and audit.get("load_attempts") == 1 and audit.get("loader_restored") is True
            and audit.get("loading_policy") == "lazy_embeddings_only"
            and audit.get("model") == record.get("model_snapshot")
            and audit.get("wrapper_sha256") == record.get("harness_hashes", {}).get("astrid_mlx_embedding_gate.py"),
            "embedding qualification loading policy is missing or failed")
    root = Path(str(record.get("source_root", "")))
    require(root.is_absolute() and str(root) == audit.get("source_root")
            and audit.get("test_path") == str(root / "test_wide_coupling_offline.py")
            and audit.get("error") == ("SystemExit: 1" if disabled_timing_exception else None)
            and not audit.get("memory_error")
            and audit.get("policy_errors") == [] and audit.get("peak_reset_before_test") is True,
            "embedding qualification path or execution evidence is missing")
    calls = audit.get("calls", [])
    require(len(calls) == 1 and calls[0].get("lazy") is True
            and calls[0].get("model") == record.get("model_snapshot")
            and calls[0].get("caller") == str(root / "coupled_astrid_server.py")
            and calls[0].get("original_kwargs") in ({}, {"model_config": {"model_type": "gemma4"}}),
            "embedding qualification call evidence is missing or changed")
    memory = audit.get("memory", {})
    require(set(memory) == {"before_load", "after_load", "after_test"}
            and all(type(row.get(key)) is int and row[key] >= 0
                    for row in memory.values() for key in ("active_bytes", "cache_bytes", "peak_bytes")),
            "embedding qualification memory evidence is missing")
    start, end = interval(audit)
    require(start >= timestamp(record.get("started_at"))
            and (not record.get("completed_at") or end <= timestamp(record["completed_at"])),
            "embedding qualification timing differs from offline run")
    fingerprints = audit.get("source_hashes", {})
    require(set(fingerprints) == sources
            and all(re.fullmatch(r"[0-9a-f]{64}", str(fingerprints[name]))
                    and fingerprints[name] == record.get("source_hashes", {}).get(name)
                    for name in sources), "embedding qualification source identity changed")


def verify_generator_audit(audit: dict[str, Any], record: dict[str, Any]) -> None:
    """Recheck the retained real-generator CPU trace without importing MLX."""
    from astrid_mlx_generator_contract import validate_trace

    mode = "full_prefilled_prompt" if record.get("stage") == "full" else "final_prompt_token_only"
    require(audit.get("schema") == "astrid_mlx_generator_contract_v1" and audit.get("passed") is True
            and not audit.get("error") and audit.get("python") == record.get("python")
            and audit.get("wrapper_sha256") == record.get("harness_hashes", {}).get("astrid_mlx_generator_contract.py")
            and audit.get("expected_history_mode") == mode,
            "generator contract identity or result is missing/failed")
    versions = audit.get("versions", {})
    require(set(versions) == {"mlx", "mlx-metal", "mlx-lm"}
            and all(value == record.get("packages", {}).get(name) for name, value in versions.items())
            and audit.get("device") == {"default": "cpu", "generation_stream": "cpu"}
            and audit.get("cpu_selected_before_generator_import") is True,
            "generator contract package or CPU device identity differs")
    checks = validate_trace(audit.get("trace"), mode)
    require(audit.get("checks") == checks and checks and all(value is True for value in checks.values()),
            "generator contract assertions differ from the retained trace")
    sources = audit.get("source_hashes", {})
    require(isinstance(sources, dict) and len(sources) == 3, "generator contract source closure is missing")
    library = Path(record["python"]).parent.parent / "lib"
    sites = list(library.glob("python*/site-packages"))
    require(len(sites) == 1, "generator contract requires one installed site-packages directory")
    site = sites[0]
    binaries = list((site / "mlx").glob("core.*.so"))
    require(len(binaries) == 1, "generator contract requires one installed MLX core binary")
    expected = {str(site / "mlx_lm" / name) for name in ("generate.py", "__init__.py")}
    expected.add(str(binaries[0]))
    require(set(sources) == expected, "generator contract source escaped its installed package paths")
    for path, fingerprint in sources.items():
        load_reference({"path": path, "sha256": fingerprint}, object_required=False)
        package = "mlx-lm" if Path(path).parent.name == "mlx_lm" else "mlx"
        wheel_record = site / f"{package.replace('-', '_')}-{versions[package]}.dist-info/RECORD"
        with wheel_record.open(newline="", encoding="utf-8") as stream:
            rows = list(csv.reader(stream))
        encoded = "sha256=" + base64.urlsafe_b64encode(bytes.fromhex(fingerprint)).rstrip(b"=").decode()
        require(any(len(row) == 3 and row[0] == str(Path(path).relative_to(site)) and row[1] == encoded
                    for row in rows), "generator contract source is not covered by the installed wheel RECORD")
    start, end = interval(audit)
    require(start >= timestamp(record.get("started_at"))
            and (not record.get("completed_at") or end <= timestamp(record["completed_at"])),
            "generator contract timing differs from offline run")


def verify_offline(record: dict[str, Any]) -> None:
    require(record.get("schema") == "mlx_offline_qualification_v1" and record.get("passed") is True
            and not record.get("error"), "offline qualification failed or is incomplete")
    tests = record.get("tests", [])
    require(len(tests) == len(OFFLINE_TESTS) and {t.get("name") for t in tests} == OFFLINE_TESTS,
            "required offline gates are missing/failed")
    for test in tests:
        raw_log = load_reference({"path": test.get("log"), "sha256": test.get("log_sha256")},
                                 object_required=False)
        timing = test.get("wide_timing_exception")
        if timing is not None:
            require(test["name"] == "wide-coupling"
                    and record.get("memory_policy") == STAGE2_DIAGNOSTIC_POLICY
                    and record.get("qualification_target") == "full"
                    and test.get("returncode") == 1,
                    "disabled-wide timing exception escaped its scope")
            argv = record.get("selected_argv", [])
            require(isinstance(argv, list) and argv.count("--wide-coupling-strength") == 1,
                    "selected wide-path setting is missing")
            index = argv.index("--wide-coupling-strength")
            require(index + 1 < len(argv) and argv[index + 1] == "0.0",
                    "selected wide path is not disabled")
            audit = load_reference(test.get("embedding_audit"))
            verify_embedding_audit(audit, record, disabled_timing_exception=True)
            from stage2_wide_exception import evaluate as evaluate_wide_exception
            expected = evaluate_wide_exception(
                raw_log.decode(), audit, stage=record["stage"],
                qualification_target=record["qualification_target"],
                actual_returncode=test["returncode"], wide_coupling_strength=0.0,
                expected_source_root=record["source_root"],
                expected_source_hashes={key: record["source_hashes"][key]
                                        for key in ("test_wide_coupling_offline.py",
                                                    "coupled_astrid_server.py", "wide_coupling.py")},
                expected_wrapper_sha256=record["harness_hashes"]["astrid_mlx_embedding_gate.py"],
                expected_model=record["model_snapshot"])
            require(expected["accepted_exception"] and timing == expected,
                    "disabled-wide timing exception differs from retained failed test")
        else:
            require(test.get("returncode") == 0, "required offline gate failed")
        if test["name"] == "wide-coupling":
            if timing is None:
                verify_embedding_audit(load_reference(test.get("embedding_audit")), record)
        if test["name"] == "generator-contract":
            verify_generator_audit(load_reference(test.get("generator_audit")), record)
    verify_memory(record)


def verify_memory(record: dict[str, Any]) -> None:
    memory = record.get("host_memory", {})
    rows = memory.get("samples", [])
    policy = record.get("memory_policy", STRICT_POLICY)
    require(policy in {STRICT_POLICY, SWAP_DIAGNOSTIC_POLICY, STAGE2_DIAGNOSTIC_POLICY}
            and memory.get("policy", STRICT_POLICY) == policy,
            "host memory policy is missing or inconsistent")
    if policy == STAGE2_DIAGNOSTIC_POLICY:
        from qualify_mlx_packages import validate_memory_policy
        require(record.get("schema") in {"mlx_matched_package_qualification_v1",
                                          "mlx_offline_qualification_v1"},
                "Stage 2 diagnostic policy is not available for this receipt")
        validate_memory_policy(policy, preparation_hash=record.get("preparation_sha256"),
                               baseline_hash=record.get("baseline_soak_sha256"),
                               manifest_hash=record.get("manifest_sha256"), stage=record.get("stage"),
                               qualification_target=record.get("qualification_target"))
        require(isinstance(rows, list) and memory.get("automated_ok") is True,
                "Stage 2 host diagnostic receipt is missing")
        expected = evaluate_memory(rows, policy=policy)
        require(all(memory.get(key) == value for key, value in expected.items()),
                "Stage 2 host diagnostics differ from retained samples")
        return
    require(len(rows) >= 2 and all(finite(row.get(key)) for row in rows
                                 for key in ("t_s", "pressure_level", "swap_bytes")),
            "qualification host memory samples are missing")
    start, end = interval(record)
    require(0 <= rows[0]["t_s"] - start <= 15 and 0 <= end - rows[-1]["t_s"] <= 15
            and all(a["t_s"] < b["t_s"] for a, b in zip(rows, rows[1:])),
            "host memory samples do not cover qualification load/run/drain")
    if policy == SWAP_DIAGNOSTIC_POLICY:
        from qualify_mlx_packages import validate_memory_policy
        require(record.get("schema") == "mlx_matched_package_qualification_v1",
                "diagnostic policy is not available for this receipt")
        validate_memory_policy(policy, preparation_hash=record.get("preparation_sha256"),
                               baseline_hash=record.get("baseline_soak_sha256"),
                               manifest_hash=record.get("manifest_sha256"), stage=record.get("stage"),
                               qualification_target=record.get("qualification_target"))
    require(memory.get("automated_ok") is True
            and evaluate_memory(rows, policy=policy)["automated_ok"] is True,
            "qualification host memory pressure/swap gate failed")


def verify_benchmark(record: dict[str, Any], offline_ref: dict[str, str], offline: dict[str, Any],
                     fixture_ref: dict[str, str], fixture: dict[str, Any]) -> None:
    # These helpers only transform retained JSON; importing this module does not
    # execute its candidate runner or import a model library.
    from qualify_mlx_packages import sha, summarize, validate_initial_coupling, validate_output, workloads

    require(record.get("schema") == "mlx_matched_package_qualification_v1"
            and record.get("state") == "completed" and record.get("summary", {}).get("automated_ok") is True
            and record.get("production_requests") == 0, "benchmark is incomplete or failed")
    require(record.get("offline_receipt_sha256") == offline_ref["sha256"]
            and interval(record)[0] >= interval(offline)[1], "benchmark is not bound to prior offline gates")
    require(record.get("fixture_manifest_sha256") == fixture_ref["sha256"]
            and record.get("fixture") == fixture
            and fixture.get("schema") == "astrid_mlx_frozen_fixture_v1",
            "benchmark frozen fixture mismatch")
    require(record.get("configured_coupling_strength") == .1
            and finite(record.get("initial_restored_coupling_strength"))
            and record["initial_restored_coupling_strength"] >= 0,
            "benchmark configured/restored coupling evidence is missing or changed")
    expected = workloads()
    require(record.get("ordered_workloads") == [row["id"] for row in expected]
            and record.get("workload_sha256") == sha(expected), "benchmark workload definition changed")
    repetitions = record.get("repetitions")
    require(type(repetitions) is int and 3 <= repetitions <= 10, "benchmark requires 3..10 repetitions")
    samples = record.get("samples", [])
    require([(row.get("repetition"), row.get("workload_id")) for row in samples]
            == [(n, row["id"]) for n in range(repetitions) for row in expected],
            "benchmark samples are missing, duplicated or out of order")
    definitions = {row["id"]: row for row in expected}
    for sample in samples:
        workload = definitions[sample["workload_id"]]
        require(sample.get("failures") == [] and not validate_output(workload, sample["response"]),
                "benchmark sampled output did not pass native completion/quality gates")
        workload_index = record["ordered_workloads"].index(sample["workload_id"])
        require(not validate_initial_coupling(workload_index, sample["response"],
                                             record["initial_restored_coupling_strength"]),
                "benchmark actual initial coupling differs from restored journal")
        require(sample.get("generation", {}).get("raw_model_artifact_detected") is False,
                "raw model artifact evidence is missing or failed")
        require(sample["generation"].get("peak_memory_scope")
                == "mlx_active_allocations_peak_since_request_reset_excludes_allocator_cache_and_process_rss",
                "benchmark peak memory accounting scope is missing or different")
        reservoir = sample.get("readyz_after", {}).get("reservoir", {})
        require((sample.get("reservoir_audit") or {}).get("checkout_succeeded") is True,
                "benchmark reservoir checkout evidence is missing or failed")
        require(reservoir.get("status") == "connected" and reservoir.get("operation") == "push_state",
                "benchmark reservoir check-in evidence is missing or failed")
        require(sample.get("usage") == sample["response"].get("usage")
                and sample.get("request", {}).get("messages_sha256") == sha(workload["messages"])
                and sample["request"].get("max_tokens") == workload["max_tokens"]
                and sample["request"].get("temperature") == 0., "benchmark sample request/usage mismatch")
        elapsed = sample.get("generation", {}).get("total_turn_s")
        require(finite(elapsed) and 0 < elapsed <= workload["deadline_s"], "benchmark exceeded its lane deadline")
    require(record.get("matched_workloads") == summarize(samples, repetitions),
            "benchmark aggregate does not match its raw samples")
    trials = record.get("trial_evidence", [])
    require([trial.get("repetition") for trial in trials] == list(range(repetitions))
            and all(type(trial.get("exit_code")) is int and trial["exit_code"] == 0
                    and type(trial.get("pid")) is int and trial["pid"] > 1 for trial in trials),
            "candidate process exit evidence is missing or failed")
    for trial in trials:
        load_reference({"path": trial.get("log"), "sha256": trial.get("log_sha256")}, object_required=False)
        raw = load_reference({"path": trial.get("metrics"), "sha256": trial.get("metrics_sha256")},
                             object_required=False)
        audit = [json.loads(line) for line in raw.splitlines() if line.strip()]
        for sample in (row for row in samples if row["repetition"] == trial["repetition"]):
            require(any(row.get("request") == sample["request"] and row.get("generation") == sample["generation"]
                        and row.get("reservoir") == sample.get("reservoir_audit") for row in audit),
                    "benchmark raw sample does not match its retained request audit")
    verify_memory(record)


def verify_predecessor_legacy(proof: dict[str, Any], core_manifest: Path,
                       core_release: dict[str, Any]) -> dict[str, Any]:
    refs = proof.get("predecessor", {})
    require(set(refs) == {"core_proof", "activation_receipt", "supervised_watch", "supervised_review",
                          "passive_soak", "passive_review"}, "full update requires accepted core predecessor evidence")
    records = {key: load_reference(ref) for key, ref in refs.items()}
    core_proof = records["core_proof"]
    require(core_proof.get("candidate_stage") == "core", "predecessor proof must qualify the core-only release")
    original = core_proof.get("artifacts", {}).get("baseline_manifest", {})
    require(isinstance(original.get("path"), str), "core proof baseline manifest is missing")
    checked_core = verify_proof(Path(refs["core_proof"]["path"]), core_manifest, Path(original["path"]))
    require(checked_core["proof_sha256"] == refs["core_proof"]["sha256"], "core proof changed")
    activation = records["activation_receipt"]
    require(activation.get("schema") == "model_environment_transition_v1"
            and activation.get("status") == "activated_verified" and activation.get("rollback") is False
            and activation.get("force_used") is False
            and activation.get("manifest") == str(core_manifest)
            and activation.get("old_identity", {}).get("manifest_sha256") == core_release["manifest_sha256"]
            and activation.get("old_identity", {}).get("qualification", {}).get("proof_sha256")
                == refs["core_proof"]["sha256"], "core was not activated with this accepted proof")
    process = activation.get("new_process", {})
    require(type(process.get("pid")) is int and process["pid"] > 1 and process.get("started_at"),
            "core activation process identity is missing")
    watch, soak = records["supervised_watch"], records["passive_soak"]
    watch_end = verify_baseline(watch, records["supervised_review"], refs["supervised_watch"]["sha256"],
                                minimum_seconds=1800)
    verify_baseline(soak, records["passive_review"], refs["passive_soak"]["sha256"])
    require(interval(watch)[0] >= timestamp(activation.get("recorded_at"))
            and interval(soak)[0] >= watch_end, "core watch/soak is not a sequential post-activation window")
    manifest = core_release["manifest"]
    source = Path(manifest["source_root"])
    required = {str(path) for path in source.glob("*.py")}
    required |= {str(core_manifest), manifest["package_inventory"]}
    required |= {path for path in manifest["artifacts"] if Path(path).name == "METADATA"}
    expected = {**manifest["artifacts"], str(core_manifest): core_release["manifest_sha256"]}
    identities = []
    for record in (watch, soak):
        observed = {item["path"]: item["sha256"] for item in record["identity_start"]}
        require(all(observed.get(path) == expected.get(path) for path in required),
                "core observation is not bound to the activated source/packages/manifest")
        provider = record["samples"][0]["provider"]
        identity = " ".join(provider["identity"]["stdout"].split())
        require(provider["pids"] == [process["pid"]]
                and identity.startswith(f"{process['pid']} {' '.join(process['started_at'].split())} "),
                "core observation belongs to a different provider process")
        identities.append(identity)
    require(identities[0] == identities[1], "core process changed between watch and soak")
    return {"manifest": {"path": str(core_manifest), "sha256": core_release["manifest_sha256"]},
            "core_proof": refs["core_proof"], "activation_receipt": refs["activation_receipt"],
            "supervised_watch": refs["supervised_watch"], "supervised_review": refs["supervised_review"],
            "passive_soak": refs["passive_soak"], "passive_review": refs["passive_review"],
            "provider": process}



def verify_predecessor_patch(proof: dict[str, Any], core_manifest: Path,
                       core_release: dict[str, Any]) -> dict[str, Any]:
    refs = proof.get("predecessor", {})
    require(set(refs) == {"core_proof", "activation_receipt", "supervised_watch", "supervised_review"},
            "full update requires exact accepted core proof, activation and watch")
    records = {key: load_reference(ref) for key, ref in refs.items()}
    core_proof = records["core_proof"]
    require(core_proof.get("candidate_stage") == "core", "predecessor proof must qualify the core-only release")
    # Historical core proof was accepted before this prospective Stage 2 reader
    # changed. Recheck its exact pre-change attestation instead of retrofitting
    # old receipt/harness identities to the new source.
    attestation_ref = {"path": "/Users/v/other/astrid/.runtime/mlx-upgrade/stage2-0323-01/"
                               "core-proof-reader-attestation-01.json",
                       "sha256": "32f9223754a23c7a0aa0ef34381d3db3f256342ecabf0979456a233fc74e1afc"}
    attestation = load_reference(attestation_ref)
    require(attestation.get("accepted_under_current_reader") is True
            and attestation.get("core_proof_sha256") == refs["core_proof"]["sha256"]
            and attestation.get("verified_proof_sha256") == refs["core_proof"]["sha256"]
            and attestation.get("core_manifest_sha256") == core_release["manifest_sha256"]
            and attestation.get("core_manifest_path") == str(core_manifest),
            "pre-change exact core proof attestation is missing or changed")
    activation = records["activation_receipt"]
    require(activation.get("schema") == "model_environment_transition_v1"
            and activation.get("status") == "activated_verified" and activation.get("rollback") is False
            and activation.get("force_used") is False
            and activation.get("manifest") == str(core_manifest)
            and activation.get("new_process", {}).get("manifest_sha256") == core_release["manifest_sha256"]
            and activation.get("old_identity", {}).get("qualification", {}).get("proof_sha256")
                == refs["core_proof"]["sha256"], "core was not activated with this accepted proof")
    process = activation.get("new_process", {})
    require(type(process.get("pid")) is int and process["pid"] > 1 and process.get("started_at"),
            "core activation process identity is missing")
    watch = records["supervised_watch"]
    verify_baseline(watch, records["supervised_review"], refs["supervised_watch"]["sha256"],
                    minimum_seconds=1800)
    require(interval(watch)[0] >= timestamp(activation.get("recorded_at")),
            "core watch predates activation")
    manifest = core_release["manifest"]
    source = Path(manifest["source_root"])
    required = {str(path) for path in source.glob("*.py")}
    required |= {str(core_manifest), manifest["package_inventory"]}
    required |= {path for path in manifest["artifacts"] if Path(path).name == "METADATA"}
    expected = {**manifest["artifacts"], str(core_manifest): core_release["manifest_sha256"]}
    for record in (watch,):
        observed = {item["path"]: item["sha256"] for item in record["identity_start"]}
        require(all(observed.get(path) == expected.get(path) for path in required),
                "core observation is not bound to the activated source/packages/manifest")
        provider = record["samples"][0]["provider"]
        identity = " ".join(provider["identity"]["stdout"].split())
        require(provider["pids"] == [process["pid"]]
                and identity.startswith(f"{process['pid']} {' '.join(process['started_at'].split())} "),
                "core observation belongs to a different provider process")
    return {"manifest": {"path": str(core_manifest), "sha256": core_release["manifest_sha256"]},
            "core_proof": refs["core_proof"], "activation_receipt": refs["activation_receipt"],
            "supervised_watch": refs["supervised_watch"], "supervised_review": refs["supervised_review"],
            "pre_change_core_proof_attestation": attestation_ref,
            "provider": process}


def verify_predecessor(proof: dict[str, Any], core_manifest: Path,
                       core_release: dict[str, Any]) -> dict[str, Any]:
    refs = proof.get("predecessor", {})
    if set(refs) == {"core_proof", "activation_receipt", "supervised_watch", "supervised_review"}:
        return verify_predecessor_patch(proof, core_manifest, core_release)
    return verify_predecessor_legacy(proof, core_manifest, core_release)


def verify_output_review(review: dict[str, Any], releases: dict[str, Any], pairs: list[dict[str, Any]],
                 benchmark_hashes: list[str], requires_retry: bool) -> str:
    require(review.get("schema") == REVIEW_SCHEMA and review.get("reviewer_kind") in {"human", "agent"}
            and isinstance(review.get("reviewer"), str) and bool(review["reviewer"].strip())
            and review.get("accepted") is True, "explicit sampled-output review is required")
    require(all(review.get(f"{role}_manifest_sha256") == release["manifest_sha256"]
                for role, release in releases.items()), "output review belongs to different release manifests")
    require(sorted(review.get("benchmark_sha256", [])) == sorted(benchmark_hashes),
            "output review does not cover every qualification attempt")
    require(timestamp(review.get("reviewed_at")) >= max(interval(receipt)[1]
                for pair in pairs for receipt in pair["records"].values()), "output review predates the samples")
    covered = set()
    final_pair = pairs[-1]
    for item in review.get("sample_reviews", []):
        require(item.get("accepted") is True and isinstance(item.get("comments"), str)
                and item["comments"].strip(), "sample review is missing an explicit assessment")
        match = next((receipt for pair in pairs for role, receipt in pair["records"].items()
                      if pair["refs"][role]["sha256"] == item.get("benchmark_sha256")), None)
        require(match is not None and any(row["workload_id"] == item.get("workload_id")
                    and row["repetition"] == item.get("repetition") for row in match["samples"]),
                "output review points to an unknown sample")
        covered.add((item["benchmark_sha256"], item["workload_id"]))
    needed = {(final_pair["refs"][role]["sha256"], workload) for role in ("baseline", "candidate")
              for workload in final_pair["records"][role]["ordered_workloads"]}
    require(needed <= covered, "output review must sample every final workload for both environments")
    if requires_retry:
        discrepancy = review.get("discrepancy_review", {})
        require(discrepancy.get("accepted") is True and isinstance(discrepancy.get("explanation"), str)
                and discrepancy["explanation"].strip(), "initial performance discrepancy needs an explicit explanation")
        contention = load_reference(discrepancy.get("contention_evidence"))
        require(contention.get("schema") == "mlx_contention_evidence_v1"
                and sorted(contention.get("initial_benchmark_sha256", []))
                == sorted(ref["sha256"] for ref in pairs[0]["refs"].values()),
                "contention evidence is not bound to the initial pair")
        observations = contention.get("observations", [])
        require(observations, "measured contention evidence is missing")
        for observation in observations:
            at = timestamp(observation.get("at"))
            require(any(interval(record)[0] <= at <= interval(record)[1]
                        for record in pairs[0]["records"].values())
                    and finite(observation.get("value")) and observation["value"] >= 0
                    and observation.get("metric") and observation.get("unit"),
                    "contention measurement is missing or outside the initial runs")
            require(load_reference(observation.get("source"), object_required=False),
                    "contention measurement source is empty")
    return review["reviewer"]


# An explicit post-measurement review can reinterpret only this immutable pair.
# This never changes the published comparator or any recorded receipt.
PRACTICAL_POLICY = "mlx_0323_campaign04_short_request_reassessment_v1"
PRACTICAL_CAPS = {"p95_total_turn_s": 5.0, "p95_first_token_s": 2.0}
PRACTICAL_PAIR = {
    "baseline": {"path": "/Users/v/other/astrid/.runtime/mlx-upgrade/alternative-0323-01/normal-load-campaign-04/matched-baseline-02/qualification.json",
                 "sha256": "4b16506060c08b435b828e621903cecc45eefebeedcfd165d12e3cf90d8e98c4"},
    "candidate": {"path": "/Users/v/other/astrid/.runtime/mlx-upgrade/alternative-0323-01/normal-load-campaign-04/matched-core-02/qualification.json",
                  "sha256": "13ee2dd7922394955f19179182b839d718ff28f0b1b9df6a8e702e1d27159a68"},
}
PRACTICAL_COMPARISON = {
    "path": "/Users/v/other/astrid/.runtime/mlx-upgrade/alternative-0323-01/normal-load-campaign-04/matched-comparison-01.json",
    "sha256": "efc8bc4dd37e2fdb790ee04670357c2e6335b09f558f9fd5529cff12fec5dcad",
}
PRACTICAL_INPUTS = {
    "preparation": "5a1658026900383579d970b405a92f8bc9f318f0b2d4a6f8c0d7e2ec9c2ebd24",
    "passive_baseline": "aad3884591a6200ba9b417acd4fc06cde051e3ff1409df16ddbf1509492aefd5",
    "baseline_manifest": "b786f6e072fd486581877a0e76f0d72014a6abed155ad3f01dd64ca6da214503",
    "candidate_manifest": "7e2b8894de5295a54609befe831a12593f94cd53b3274cbbabb338d40f22b1c7",
    "frozen_fixture": "e93ea0aa680636e87adb6cae0a02f192fce3ee1855d1f0ce7766ffcc284d5a40",
}


def practical_assessment(published: dict[str, Any]) -> dict[str, Any]:
    """Pure alternative assessment; never mutates or replaces published results."""
    result = deepcopy(published)
    result["policy"] = PRACTICAL_POLICY
    result["after_the_fact_reinterpretation"] = True
    result["published_ok"] = published["ok"]
    result["thresholds"]["short_request_absolute_caps_s"] = dict(PRACTICAL_CAPS)
    for metrics in result["workloads"].values():
        for metric, cap in PRACTICAL_CAPS.items():
            value = metrics.get(metric)
            if value and value["baseline"] < cap:
                value["published_relative_ok"] = value["ok"]
                value["published_threshold_ratio"] = value.pop("threshold_ratio")
                value["gate"] = "absolute_cap_for_baseline_below_cap"
                value["threshold_s"] = cap
                value["ok"] = value["candidate"] <= cap + 1e-12
    result["regressions"] = [
        {"kind": "performance_regression", "workload": workload, "metric": metric, **value}
        for workload, metrics in result["workloads"].items() for metric, value in metrics.items()
        if value.get("ok") is False]
    result["ok"] = result["complete"] and not result["regressions"]
    return result


def practical_reader_hashes() -> dict[str, str]:
    return {name: hashlib.sha256(Path(__file__).with_name(name).read_bytes()).hexdigest()
            for name in ("model_qualification.py", "astrid_soak_performance.py")}


def verify_practical_review(review: dict[str, Any], stage: str, pairs: list[dict[str, Any]],
                            refs: dict[str, Any], comparisons: list[dict[str, Any]]) -> dict[str, Any]:
    require(stage == "core" and len(pairs) == 1 and pairs[0]["refs"] == PRACTICAL_PAIR,
            "practical reassessment is limited to the exact campaign04 core pair")
    require(all(refs.get(name, {}).get("sha256") == expected for name, expected in PRACTICAL_INPUTS.items()),
            "practical reassessment input binding differs")
    require(review.get("schema") == "mlx_practical_performance_review_v1"
            and review.get("accepted") is True and review.get("policy") == PRACTICAL_POLICY
            and review.get("after_the_fact_reinterpretation") is True
            and review.get("published_failure_preserved") is True
            and review.get("reviewer_kind") in {"agent", "human"}
            and isinstance(review.get("reviewer"), str) and bool(review["reviewer"].strip())
            and isinstance(review.get("rationale"), str) and bool(review["rationale"].strip())
            and isinstance(review.get("limitations"), list) and bool(review["limitations"])
            and all(isinstance(item, str) and bool(item.strip()) for item in review["limitations"]),
            "explicit after-the-fact performance review is required")
    require(review.get("benchmark_refs") == PRACTICAL_PAIR
            and review.get("input_hashes") == PRACTICAL_INPUTS
            and review.get("absolute_caps_s") == PRACTICAL_CAPS
            and review.get("published_comparison") == PRACTICAL_COMPARISON
            and review.get("reader_hashes") == practical_reader_hashes(),
            "practical policy review does not bind exact evidence, caps and readers")
    original = load_reference(PRACTICAL_COMPARISON)
    require(original.get("baseline") == PRACTICAL_PAIR["baseline"]
            and original.get("core") == PRACTICAL_PAIR["candidate"]
            and original.get("decision") == "FAILED_PUBLISHED_COMPARATIVE_PERFORMANCE"
            and original.get("comparison") == comparisons[0] and comparisons[0]["ok"] is False,
            "published performance failure was not preserved")
    require(timestamp(review.get("reviewed_at")) >= max(timestamp(original.get("at_utc")),
                max(interval(record)[1] for record in pairs[0]["records"].values())),
            "practical performance review predates the measurements")
    assessment = practical_assessment(comparisons[0])
    require(assessment["ok"] and review.get("assessment") == assessment,
            "alternative practical performance gates failed or assessment differs")
    return assessment


def verify_record(proof: dict[str, Any], candidate_manifest: Path, baseline_manifest: Path) -> dict[str, Any]:
    require(proof.get("schema") == SCHEMA and proof.get("candidate_stage") in {"core", "full"},
            "unsupported qualification proof schema/stage")
    refs = proof.get("artifacts", {})
    required = {"candidate_manifest", "baseline_manifest", "preparation", "live_inventory", "passive_baseline",
                "passive_review", "offline_baseline", "offline_candidate", "frozen_fixture", "output_review"}
    require(required <= set(refs), "qualification proof is missing evidence references")
    artifacts = {key: load_reference(ref) for key, ref in refs.items()}
    releases = {}
    for role, path in (("candidate", candidate_manifest), ("baseline", baseline_manifest)):
        require(refs[f"{role}_manifest"]["path"] == str(path.absolute()), f"{role} manifest path mismatch")
        releases[role] = validate_manifest(path)
        require(releases[role]["manifest_sha256"] == refs[f"{role}_manifest"]["sha256"],
                f"{role} manifest changed during validation")
    stage = proof["candidate_stage"]
    baseline_hash = refs["passive_baseline"]["sha256"]
    patch_stage2 = stage == "full" and proof.get("qualification_policy") == STAGE2_DIAGNOSTIC_POLICY
    if patch_stage2:
        require(set(proof.get("predecessor", {})) == {"core_proof", "activation_receipt",
                                                       "supervised_watch", "supervised_review"},
                "Stage 2 patch proof requires the exact reviewed core watch predecessor")
    baseline_end = verify_baseline(
        artifacts["passive_baseline"], artifacts["passive_review"], baseline_hash,
        minimum_seconds=1800 if patch_stage2 else baseline_minimum_for_stage(stage))
    predecessor = verify_predecessor(proof, baseline_manifest, releases["baseline"]) if stage == "full" else None
    if predecessor and patch_stage2:
        require(refs["passive_baseline"] == predecessor["supervised_watch"]
                and refs["passive_review"] == predecessor["supervised_review"],
                "full qualification must use the accepted core watch as its passive baseline")
    elif predecessor:
        require(refs["passive_baseline"] == predecessor["passive_soak"]
                and refs["passive_review"] == predecessor["passive_review"],
                "full qualification must use the accepted core soak as its passive baseline")
    preparation = artifacts["preparation"]
    verify_preparation(preparation, artifacts["live_inventory"], refs["live_inventory"],
                       artifacts["passive_baseline"], releases, stage)
    kwargs = {"stage": stage, "preparation": preparation, "preparation_hash": refs["preparation"]["sha256"],
              "baseline_hash": baseline_hash, "baseline_end": baseline_end}
    from qualify_mlx_packages import harness_hashes, offline_harness_matches
    harness = harness_hashes()
    require(harness and all(re.fullmatch(r"[0-9a-f]{64}", value) for value in harness.values()),
            "qualification harness identity missing")
    for role in releases:
        receipt = artifacts[f"offline_{role}"]
        require(offline_harness_matches(receipt, refs[f"offline_{role}"], harness),
                "offline qualification harness changed")
        if stage == "full":
            require(receipt.get("predecessor_acceptance") == predecessor,
                    "offline work did not admit this accepted core predecessor")
        verify_receipt_identity(receipt, role=role, release=releases[role], **kwargs)
        verify_offline(receipt)
    attempts = proof.get("performance_attempts", [])
    require(isinstance(attempts, list) and 1 <= len(attempts) <= 2, "one or two matched performance pairs are required")
    pairs, benchmark_hashes, comparisons, orders = [], [], [], []
    for attempt in attempts:
        require(set(attempt) == {"baseline", "candidate"}, "performance attempt requires baseline/candidate refs")
        records = {role: load_reference(ref) for role, ref in attempt.items()}
        for role, record in records.items():
            require(record.get("harness_hashes") == harness, "benchmark qualification harness changed")
            if stage == "full":
                require(record.get("predecessor_acceptance") == predecessor,
                        "matched work did not admit this accepted core predecessor")
            verify_receipt_identity(record, role=role, release=releases[role], **kwargs)
            verify_benchmark(record, refs[f"offline_{role}"], artifacts[f"offline_{role}"],
                             refs["frozen_fixture"], artifacts["frozen_fixture"])
            benchmark_hashes.append(attempt[role]["sha256"])
        policy = records["baseline"].get("memory_policy", STRICT_POLICY)
        if patch_stage2:
            require(policy == STAGE2_DIAGNOSTIC_POLICY,
                    "Stage 2 patch proof requires its diagnostic policy in both halves")
        require(records["candidate"].get("memory_policy", STRICT_POLICY) == policy
                and (not pairs or pairs[0]["records"]["baseline"].get("memory_policy", STRICT_POLICY) == policy),
                "matched memory policy differs across environments or attempts")
        require(records["baseline"]["fixture_manifest_sha256"] == records["candidate"]["fixture_manifest_sha256"],
                "matched workloads used different reservoir fixtures")
        for field in ("configured_coupling_strength", "initial_restored_coupling_strength"):
            require(records["baseline"][field] == records["candidate"][field]
                    and (not pairs or records["baseline"][field] == pairs[0]["records"]["baseline"][field]),
                    f"matched benchmark coupling differs: {field}")
        ordered = sorted(records, key=lambda role: interval(records[role])[0])
        require(interval(records[ordered[0]])[1] <= interval(records[ordered[1]])[0],
                "benchmark environments overlapped")
        if pairs:
            require(min(interval(record)[0] for record in records.values())
                    >= max(interval(record)[1] for record in pairs[-1]["records"].values()),
                    "retest overlaps or predates the initial attempt")
        orders.append(ordered)
        comparisons.append(compare_performance(records["baseline"], records["candidate"]))
        pairs.append({"records": records, "refs": attempt})
    require(len(set(benchmark_hashes)) == len(benchmark_hashes), "benchmark receipt was reused across attempts")
    require(all(comparison["complete"] for comparison in comparisons), "matched performance evidence is incomplete")
    patch_diagnostics = patch_stage2 and policy == STAGE2_DIAGNOSTIC_POLICY
    alternative = None
    if "performance_policy_review" in refs:
        alternative = verify_practical_review(artifacts["performance_policy_review"], stage, pairs, refs, comparisons)
    retry = not comparisons[0]["ok"] and alternative is None and not patch_diagnostics
    require(comparisons[-1]["ok"] or alternative is not None or patch_diagnostics,
            "matched performance thresholds failed")
    if len(pairs) == 2:
        require(orders[1] == list(reversed(orders[0])), "retest must reverse the environment run order")
    require(not retry or len(pairs) == 2, "initial regression requires a complete reversed retest")
    reviewer = verify_output_review(artifacts["output_review"], releases, pairs, benchmark_hashes, retry)
    return {"schema": SCHEMA, "accepted": True, "candidate_stage": stage,
            "qualification_policy": proof.get("qualification_policy"),
            "candidate_manifest_sha256": releases["candidate"]["manifest_sha256"],
            "baseline_manifest_sha256": releases["baseline"]["manifest_sha256"],
            "accepted_evidence": refs, "performance_attempts": attempts, "performance": comparisons,
            "reverse_retest_accepted": retry, "memory_policy": policy, "output_reviewer": reviewer,
            "published_performance_passed": all(item["ok"] for item in comparisons),
            "comparative_ratios_diagnostic": patch_diagnostics,
            "alternative_performance": alternative,
            "reviewer_kind": artifacts["output_review"]["reviewer_kind"], "predecessor_acceptance": predecessor}


def verify_proof(path: Path, candidate_manifest: Path, baseline_manifest: Path) -> dict[str, Any]:
    ref = reference(path)
    checked = verify_record(load_reference(ref), candidate_manifest, baseline_manifest)
    # Catch ordinary concurrent replacements while the potentially large frozen
    # model/package closure was being read. Activation rechecks at its boundaries.
    require(reference(path) == ref, "qualification proof changed during verification")
    return {**checked, "proof_sha256": ref["sha256"], "proof_path": ref["path"]}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    for command in ("create", "check"):
        item = sub.add_parser(command)
        item.add_argument("--candidate-manifest", type=Path, required=True)
        item.add_argument("--baseline-manifest", type=Path, required=True)
        item.add_argument("--proof", type=Path, required=True)
        if command == "create":
            item.add_argument("--candidate-stage", choices=("core", "full"), required=True)
            item.add_argument("--qualification-policy", choices=(STAGE2_DIAGNOSTIC_POLICY,))
            for name in ("preparation", "live-inventory", "passive-baseline", "passive-review", "offline-baseline",
                         "offline-candidate", "frozen-fixture", "output-review"):
                item.add_argument(f"--{name}", type=Path, required=True)
            item.add_argument("--performance-policy-review", type=Path)
            item.add_argument("--benchmark-pair", type=Path, nargs=2, action="append", required=True,
                              metavar=("BASELINE", "CANDIDATE"))
            for name in ("core-proof", "activation-receipt", "supervised-watch", "supervised-review",
                         "core-passive-soak", "core-passive-review"):
                item.add_argument(f"--{name}", type=Path)
    args = parser.parse_args()
    if args.command == "create":
        names = ("candidate_manifest", "baseline_manifest", "preparation", "live_inventory", "passive_baseline",
                 "passive_review", "offline_baseline", "offline_candidate", "frozen_fixture", "output_review")
        proof = {"schema": SCHEMA, "created_at": datetime.now(timezone.utc).isoformat(),
                 "candidate_stage": args.candidate_stage,
                 "artifacts": {name: reference(getattr(args, name)) for name in names},
                 "performance_attempts": [{"baseline": reference(a), "candidate": reference(b)}
                                          for a, b in args.benchmark_pair]}
        if args.performance_policy_review:
            proof["artifacts"]["performance_policy_review"] = reference(args.performance_policy_review)
        if args.qualification_policy:
            require(args.candidate_stage == "full" and args.qualification_policy == STAGE2_DIAGNOSTIC_POLICY,
                    "patch qualification policy requires Stage 2 full target")
            proof["qualification_policy"] = args.qualification_policy
        if args.candidate_stage == "full":
            predecessor = {"core_proof": args.core_proof, "activation_receipt": args.activation_receipt,
                           "supervised_watch": args.supervised_watch, "supervised_review": args.supervised_review}
            if not args.qualification_policy:
                predecessor.update(passive_soak=args.core_passive_soak,
                                   passive_review=args.core_passive_review)
            require(all(predecessor.values()), "full proof requires accepted core predecessor evidence")
            proof["predecessor"] = {name: reference(path) for name, path in predecessor.items()}
        verify_record(proof, args.candidate_manifest, args.baseline_manifest)
        with args.proof.open("x", encoding="utf-8") as handle:
            json.dump(proof, handle, indent=2, sort_keys=True)
            handle.write("\n")
    print(json.dumps(verify_proof(args.proof, args.candidate_manifest, args.baseline_manifest), indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
