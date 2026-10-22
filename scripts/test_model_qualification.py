"""Model-free qualification evidence fixtures; no service or GPU operations."""
from __future__ import annotations

from copy import deepcopy
import base64
import csv
from datetime import datetime, timezone
import json
from pathlib import Path
import sys
from types import SimpleNamespace

import pytest

import model_environment as environments
import model_qualification as qualification
import qualify_mlx_packages as packages
from astrid_mlx_memory import evaluate as evaluate_memory
from astrid_soak_observer import review_observation
from qualify_mlx_packages import harness_hashes, sha, summarize, workloads
from qualify_model_launchd_handoff import fixture_manifest
from test_astrid_soak_observer import healthy_sample
from test_astrid_mlx_generator_contract import valid_trace


def iso(seconds):
    return datetime.fromtimestamp(1792022400 + seconds, timezone.utc).isoformat()


def write(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
    return qualification.reference(path)


def generator_sources(manifest):
    library = next((Path(manifest["python"]).parent.parent / "lib").glob("python*/site-packages"))
    sources = {}
    for filename in ("mlx_lm/generate.py", "mlx_lm/__init__.py", "mlx/core.fixture.so"):
        path = library / filename
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("synthetic package bytes, never imported\n")
        sources[str(path)] = environments.digest(path)
        package = "mlx-lm" if path.parent.name == "mlx_lm" else "mlx"
        record = library / f"{package.replace('-', '_')}-{manifest['packages'][package]}.dist-info/RECORD"
        with record.open(newline="") as stream:
            rows = [row for row in csv.reader(stream) if row[0] != filename]
        encoded = "sha256=" + base64.urlsafe_b64encode(bytes.fromhex(sources[str(path)])).rstrip(b"=").decode()
        rows.append([filename, encoded, str(path.stat().st_size)])
        with record.open("w", newline="") as stream:
            csv.writer(stream, lineterminator="\n").writerows(rows)
        manifest["artifacts"][str(record)] = environments.digest(record)
    return sources


def generator_audit(root, name, manifest, harness, *, mode="final_prompt_token_only", start=7301, end=7399):
    from astrid_mlx_generator_contract import validate_trace
    sources = generator_sources(manifest)
    trace = valid_trace(mode)
    return write(root / f"{name}-generator.json", {
        "schema": "astrid_mlx_generator_contract_v1", "passed": True,
        "versions": {key: manifest["packages"][key] for key in ("mlx", "mlx-metal", "mlx-lm")},
        "python": manifest["python"], "expected_history_mode": mode,
        "device": {"default": "cpu", "generation_stream": "cpu"},
        "cpu_selected_before_generator_import": True,
        "source_hashes": sources, "wrapper_sha256": harness["astrid_mlx_generator_contract.py"],
        "trace": trace, "checks": validate_trace(trace, mode),
        "started_at": iso(start), "completed_at": iso(end),
    })


@pytest.fixture
def evidence(tmp_path):
    root = tmp_path.resolve()
    paths = {role: fixture_manifest(root, role, Path(sys.executable).resolve())
             for role in ("baseline", "candidate")}
    manifests = {role: environments.read_json(path) for role, path in paths.items()}
    # The fixture launcher embeds its release name. Qualification requires
    # identical provider source across environments, so normalize fixture text.
    source = root / "baseline/coupled_astrid_server.py"
    other = root / "candidate/coupled_astrid_server.py"
    other.write_bytes(source.read_bytes())
    manifests["candidate"]["artifacts"][str(other)] = environments.digest(other)
    for role in paths:
        manifests[role]["artifacts"].update(generator_sources(manifests[role]))
        for name in ("test_wide_coupling_offline.py", "wide_coupling.py"):
            extra = Path(manifests[role]["source_root"]) / name
            extra.write_text("# synthetic source identity; never executed\n")
            manifests[role]["artifacts"][str(extra)] = environments.digest(extra)
        write(paths[role], manifests[role])
    inventory_path = root / "live-inventory.json"
    inventory = environments.read_json(Path(manifests["baseline"]["package_inventory"]))
    inventory_ref = write(inventory_path, inventory)
    source_hashes = {path.name: environments.digest(path) for path in source.parent.glob("*.py")}
    preparation = {"schema": "mlx_environment_preparation_v1", "live_environment_unchanged": True,
                   "started_at": iso(-100), "completed_at": iso(-10),
                   "source_root": str(source.parent), "source_hashes": source_hashes,
                   "baseline_python": inventory["executable"],
                   "model_snapshot": str(root / "model-placeholder"),
                   "stages": {stage: {key: manifests[role][key] for key in ("python", "source_root", "packages")}
                              for role, stage in (("baseline", "baseline"), ("candidate", "core"))}}
    preparation_ref = write(root / "preparation.json", preparation)
    identities = [{"path": str(path), "sha256": environments.digest(path), "size": path.stat().st_size,
                   "mtime_ns": path.stat().st_mtime_ns}
                  for path in (*source.parent.glob("*.py"), inventory_path)]
    identities += [{"path": path, "sha256": fingerprint, "size": Path(path).stat().st_size,
                    "mtime_ns": Path(path).stat().st_mtime_ns}
                   for path, fingerprint in manifests["baseline"]["artifacts"].items()
                   if Path(path).name == "METADATA"]
    samples = []
    for second in range(0, 7201, 30):
        sample = healthy_sample()
        sample["t_s"] = 1792022400 + second
        samples.append(sample)
    baseline = {"mode": "observe_only", "state": "completed", "duration_s": 7201,
                "started_at": iso(0), "completed_at": iso(7201), "live_mutations": [], "probes": [],
                "started_candidate": False, "restore": None, "samples": samples,
                "identity_start": identities, "identity_end": deepcopy(identities),
                "summary": {"automated_ok": True, "failure_reasons": []},
                "log_sources": {name: {"discontinuities": 0, "read_errors": 0} for name in ("bridge", "model")},
                "monitor": {"automated_ok": True, "failure_reasons": [],
                    "generation_stats": {"count": 4}, "events": {name: 0 for name in (
                        "bridge_artifact_strip_count", "fallback_count", "malformed_next_count", "artifact_count")},
                    "generated_output_scan": {"counts": {name: 0 for name in ("artifact_count",
                        "deprecated_language_count", "explore_action_count", "malformed_next_count")}}}}
    baseline_ref = write(root / "passive.json", baseline)
    review = {**review_observation(baseline), "source_record_sha256": baseline_ref["sha256"],
              "qualified_observation": True}
    fixture = {"schema": "astrid_mlx_frozen_fixture_v1", "snapshot": {"sha256": "a" * 64},
               "journal": {"sha256": "b" * 64}}
    refs = {"baseline_manifest": qualification.reference(paths["baseline"]),
            "candidate_manifest": qualification.reference(paths["candidate"]),
            "preparation": preparation_ref, "live_inventory": inventory_ref, "passive_baseline": baseline_ref,
            "passive_review": write(root / "passive-review.json", review),
            "frozen_fixture": write(root / "fixture.json", fixture)}
    common = {"preparation_sha256": preparation_ref["sha256"], "baseline_soak_sha256": baseline_ref["sha256"],
              "qualification_target": "core",
              "source_hashes": source_hashes, "model_snapshot": preparation["model_snapshot"],
              "harness_hashes": harness_hashes()}
    for role in paths:
        memory_rows = [{"t_s": 1792022400 + second, "pressure_level": 1, "swap_bytes": 0}
                       for second in range(7300, 7401, 5)]
        offline = {**common, "schema": "mlx_offline_qualification_v1", "stage": "baseline" if role == "baseline" else "core",
                   "manifest_sha256": refs[f"{role}_manifest"]["sha256"], "packages": manifests[role]["packages"],
                   "source_root": manifests[role]["source_root"], "python": manifests[role]["python"],
                   "started_at": iso(7300), "completed_at": iso(7400), "passed": True,
                   "host_memory": {**evaluate_memory(memory_rows), "samples": memory_rows},
                   "tests": [{"name": name, "returncode": 0} for name in sorted(qualification.OFFLINE_TESTS)]}
        for test in offline["tests"]:
            log = root / f"{role}-{test['name']}.log"
            log.write_text("fixture offline test passed\n")
            test.update(log=str(log), log_sha256=environments.digest(log))
            if test["name"] == "generator-contract":
                test["generator_audit"] = generator_audit(root, role, manifests[role], common["harness_hashes"])
            if test["name"] == "wide-coupling":
                test["embedding_audit"] = write(root / f"{role}-embedding.json", {
                    "schema": "astrid_mlx_embedding_gate_v1", "passed": True,
                    "loader_calls": 1, "load_attempts": 1, "loader_restored": True,
                    "loading_policy": "lazy_embeddings_only",
                    "wrapper_sha256": common["harness_hashes"]["astrid_mlx_embedding_gate.py"],
                    "model": preparation["model_snapshot"], "source_hashes": source_hashes,
                    "source_root": manifests[role]["source_root"],
                    "test_path": str(Path(manifests[role]["source_root"]) / "test_wide_coupling_offline.py"),
                    "started_at": iso(7301), "completed_at": iso(7399),
                    "policy_errors": [], "peak_reset_before_test": True,
                    "calls": [{"lazy": True, "model": preparation["model_snapshot"], "original_kwargs": {},
                               "caller": str(Path(manifests[role]["source_root"]) / "coupled_astrid_server.py")}],
                    "memory": {key: {"active_bytes": 1, "cache_bytes": 2, "peak_bytes": 3}
                               for key in ("before_load", "after_load", "after_test")},
                })
        refs[f"offline_{role}"] = write(root / f"offline-{role}.json", offline)

    def make_benchmark(role, start, prefix="initial", total=1.):
        raw_samples = []
        for repetition in range(3):
            for workload in workloads():
                content = workload.get("expected", "A calm lake.\nNEXT: LISTEN" if workload.get("next")
                                       else "A complete thought. " * 180)
                completion = min(workload["max_tokens"] - 1, 100)
                usage = {"prompt_tokens": 10, "completion_tokens": completion}
                response = {"usage": usage, "choices": [{"message": {"content": content}, "finish_reason": "stop"}],
                            "coupled_generation_v1": {"schema": "coupled_generation_v1", **usage,
                                "controls": {"thinking": False, "wide_coupling_strength": 0., "temperature": 0.,
                                             "sampling_mode": "greedy", "processor_order": ["reservoir", "additional_repetition"],
                                             "max_tokens": workload["max_tokens"], "reservoir_coupling_strength": .02},
                                "finish_reason": "stop", "termination": {"kind": "model_eos", "model_eos_reached": True}}}
                raw_samples.append({"workload_id": workload["id"], "repetition": repetition,
                    "generation": {"total_turn_s": total, "first_token_s": .1, "steady_tok_s": 20., "peak_memory_bytes": 1000,
                        "raw_model_artifact_detected": False,
                        "peak_memory_scope": "mlx_active_allocations_peak_since_request_reset_excludes_allocator_cache_and_process_rss"},
                    "request": {"messages_sha256": sha(workload["messages"]), "max_tokens": workload["max_tokens"],
                                "prompt_sha256_12": "a" * 12, "temperature": 0.},
                    "readyz_after": {"reservoir": {"status": "connected", "operation": "push_state"}},
                    "reservoir_audit": {"checkout_succeeded": True},
                    "usage": usage, "response": response, "failures": []})
        memory_samples = [{"t_s": 1792022400 + second, "pressure_level": 1, "swap_bytes": 0}
                          for second in range(start, start + 101, 5)]
        benchmark = {**common, "schema": "mlx_matched_package_qualification_v1",
                     "stage": "baseline" if role == "baseline" else "core", "state": "completed",
                     "started_at": iso(start), "completed_at": iso(start + 100), "repetitions": 3,
                     "manifest_sha256": refs[f"{role}_manifest"]["sha256"], "packages": manifests[role]["packages"],
                     "offline_receipt_sha256": refs[f"offline_{role}"]["sha256"],
                     "fixture_manifest_sha256": refs["frozen_fixture"]["sha256"], "fixture": fixture,
                     "configured_coupling_strength": .1, "initial_restored_coupling_strength": .02,
                     "workload_sha256": sha(workloads()), "ordered_workloads": [row["id"] for row in workloads()],
                     "samples": raw_samples, "matched_workloads": summarize(raw_samples, 3), "production_requests": 0,
                     "host_memory": {**evaluate_memory(memory_samples), "samples": memory_samples},
                     "summary": {"automated_ok": True, "operator_review_required": True}}
        benchmark["trial_evidence"] = []
        for repetition in range(3):
            log = root / f"{prefix}-{role}-{repetition}.log"
            log.write_text("fixture candidate drained and exited\n")
            metrics = root / f"{prefix}-{role}-{repetition}.jsonl"
            metrics.write_text("".join(json.dumps({"request": row["request"], "generation": row["generation"],
                                                 "reservoir": row["reservoir_audit"]}) + "\n"
                                       for row in raw_samples if row["repetition"] == repetition))
            benchmark["trial_evidence"].append({"repetition": repetition, "pid": 100 + repetition, "exit_code": 0,
                "log": str(log), "log_sha256": environments.digest(log),
                "metrics": str(metrics), "metrics_sha256": environments.digest(metrics)})
        return write(root / f"{prefix}-{role}.json", benchmark)

    pair = {"baseline": make_benchmark("baseline", 8000), "candidate": make_benchmark("candidate", 8200)}
    proof = {"schema": qualification.SCHEMA, "candidate_stage": "core", "artifacts": refs,
             "performance_attempts": [pair]}

    def save_review(discrepancy=None):
        final = proof["performance_attempts"][-1]
        review = {"schema": qualification.REVIEW_SCHEMA, "reviewer_kind": "agent", "reviewer": "fixture only",
                  "reviewed_at": iso(10000), "accepted": True,
                  "baseline_manifest_sha256": refs["baseline_manifest"]["sha256"],
                  "candidate_manifest_sha256": refs["candidate_manifest"]["sha256"],
                  "benchmark_sha256": [ref["sha256"] for pair in proof["performance_attempts"] for ref in pair.values()],
                  "sample_reviews": [{"benchmark_sha256": final[role]["sha256"], "workload_id": row["id"],
                      "repetition": 0, "accepted": True, "comments": "Synthetic fixture demonstrates review binding."}
                      for role in paths for row in workloads()]}
        if discrepancy:
            review["discrepancy_review"] = discrepancy
        refs["output_review"] = write(root / "output-review.json", review)

    save_review()
    return {"root": root, "paths": paths, "proof": proof, "make_benchmark": make_benchmark, "save_review": save_review}


def check(evidence):
    return qualification.verify_record(evidence["proof"], evidence["paths"]["candidate"], evidence["paths"]["baseline"])


def change_reference(ref, edit):
    value = qualification.load_reference(ref)
    edit(value)
    ref.update(write(Path(ref["path"]), value))


def test_full_proof_checks_without_mutating_or_running_commands(evidence, monkeypatch):
    import subprocess
    monkeypatch.setattr(subprocess, "run", lambda *_a, **_k: pytest.fail("verification launched a command"))
    monkeypatch.setattr(subprocess, "Popen", lambda *_a, **_k: pytest.fail("verification started a process"))
    path = evidence["root"] / "proof.json"
    write(path, evidence["proof"])
    before = {p: p.read_bytes() for p in evidence["root"].rglob("*") if p.is_file()}
    result = qualification.verify_proof(path, evidence["paths"]["candidate"], evidence["paths"]["baseline"])
    assert result["accepted"] and result["proof_sha256"] == environments.digest(path)
    assert not result["reverse_retest_accepted"]
    assert before == {p: p.read_bytes() for p in before}


def test_mutated_evidence_is_rejected(evidence):
    Path(evidence["proof"]["artifacts"]["offline_candidate"]["path"]).write_text("{}")
    with pytest.raises(ValueError, match="hash changed"):
        check(evidence)


def test_one_hour_pre_promotion_record_does_not_shorten_post_activation_soak(evidence):
    assert qualification.baseline_minimum_for_stage("core") == 3600
    assert qualification.baseline_minimum_for_stage("full") == 7200
    refs = evidence["proof"]["artifacts"]
    record = qualification.load_reference(refs["passive_baseline"])
    review = qualification.load_reference(refs["passive_review"])
    record["samples"] = record["samples"][:121]
    record.update(duration_s=3601, completed_at=iso(3601))
    baseline_hash = refs["passive_baseline"]["sha256"]
    assert qualification.verify_baseline(
        record, review, baseline_hash, minimum_seconds=3600) == qualification.timestamp(iso(3601))
    with pytest.raises(ValueError, match="two hours"):
        qualification.verify_baseline(record, review, baseline_hash)


@pytest.mark.parametrize("edit,expected", [
    (lambda r: r.update(duration_s=7199), "two hours"),
    (lambda r: r.update(state="interrupted"), "completed passive"),
    (lambda r: r["samples"][10]["provider"].update(identity={}), "identity"),
    (lambda r: r["samples"][10]["readyz"]["json"]["worker"].update(last_generation_error="metal failure"), "health sample"),
    (lambda r: r["monitor"]["events"].update(fallback_count=1), "incident"),
    (lambda r: r["samples"].__delitem__(slice(10, 12)), "sampling coverage"),
])
def test_baseline_cannot_be_qualified_by_summary_alone(evidence, edit, expected):
    refs = evidence["proof"]["artifacts"]
    record = qualification.load_reference(refs["passive_baseline"])
    review = qualification.load_reference(refs["passive_review"])
    edit(record)
    with pytest.raises(ValueError, match=expected):
        qualification.verify_baseline(record, review, refs["passive_baseline"]["sha256"])


@pytest.mark.parametrize("field,value", [("source_hashes", {}), ("packages", {}), ("manifest_sha256", "f" * 64)])
def test_offline_receipt_must_match_source_packages_and_manifest(evidence, field, value):
    change_reference(evidence["proof"]["artifacts"]["offline_candidate"], lambda r: r.update({field: value}))
    with pytest.raises(ValueError, match="provenance"):
        check(evidence)


def test_offline_receipt_requires_every_gate(evidence):
    change_reference(evidence["proof"]["artifacts"]["offline_candidate"], lambda r: r["tests"].pop())
    with pytest.raises(ValueError, match="offline gates"):
        check(evidence)


@pytest.mark.parametrize("edit", [
    lambda r: r.update(loader_calls=2),
    lambda r: r.update(loading_policy="eager"),
    lambda r: r.update(loader_restored=False),
    lambda r: r.update(source_hashes={}),
    lambda r: r.update(wrapper_sha256="f" * 64),
    lambda r: r.update(memory={}),
    lambda r: r.update(calls=[]),
    lambda r: r.update(source_root="/wrong/stage"),
])
def test_embedding_audit_policy_and_sources_are_rechecked(evidence, edit):
    def change_offline(record):
        wide = next(t for t in record["tests"] if t["name"] == "wide-coupling")
        change_reference(wide["embedding_audit"], edit)
    change_reference(evidence["proof"]["artifacts"]["offline_candidate"], change_offline)
    with pytest.raises(ValueError, match="embedding qualification"):
        check(evidence)


@pytest.mark.parametrize("edit,expected", [
    (lambda r: r["samples"][0]["response"]["coupled_generation_v1"]["termination"].update(kind="stop_sequence", model_eos_reached=False), "native completion"),
    (lambda r: r["matched_workloads"]["exact"].update(p95_total_turn_s=.01), "aggregate"),
    (lambda r: r["host_memory"]["samples"][-1].update(swap_bytes=1), "pressure/swap"),
    (lambda r: r.update(production_requests=1), "incomplete or failed"),
    (lambda r: r["samples"].pop(), "samples are missing"),
])
def test_raw_benchmark_is_rechecked(evidence, edit, expected):
    ref = evidence["proof"]["performance_attempts"][0]["candidate"]
    change_reference(ref, edit)
    evidence["save_review"]()
    with pytest.raises(ValueError, match=expected):
        check(evidence)


@pytest.mark.parametrize("edit,expected", [
    (lambda r: r.update(accepted=False), "explicit sampled"),
    (lambda r: r.update(reviewer_kind="automated_summary"), "explicit sampled"),
    (lambda r: r["sample_reviews"].pop(), "every final workload"),
    (lambda r: r.update(reviewed_at=iso(1)), "predates"),
    (lambda r: r.update(benchmark_sha256=[]), "every qualification"),
])
def test_output_review_is_explicit_and_bound(evidence, edit, expected):
    change_reference(evidence["proof"]["artifacts"]["output_review"], edit)
    with pytest.raises(ValueError, match=expected):
        check(evidence)


def retry(evidence, *, reversed_order=True, initial_total=2., final_total=1.):
    proof = evidence["proof"]
    proof["performance_attempts"][0]["candidate"] = evidence["make_benchmark"]("candidate", 8200, total=initial_total)
    proof["performance_attempts"].append({
        "candidate": evidence["make_benchmark"]("candidate", 8500 if reversed_order else 8700, "retest", total=final_total),
        "baseline": evidence["make_benchmark"]("baseline", 8700 if reversed_order else 8500, "retest")})
    evidence["save_review"]()


def test_regression_needs_reversed_retest_and_reasoned_contention_review(evidence):
    retry(evidence)
    with pytest.raises(ValueError, match="discrepancy"):
        check(evidence)
    measurement = evidence["root"] / "contention-sample.txt"
    measurement.write_text("fixture CPU contention sample: 95 percent at initial candidate timestamp")
    contention = {"schema": "mlx_contention_evidence_v1", "initial_benchmark_sha256": [ref["sha256"]
                  for ref in evidence["proof"]["performance_attempts"][0].values()],
                  "observations": [{"at": iso(8250), "metric": "cpu_utilization", "value": 95, "unit": "percent",
                                    "source": qualification.reference(measurement)}]}
    evidence["save_review"]({"accepted": True, "explanation": "Fixture: measured competing CPU work ended before rerun.",
                             "contention_evidence": write(evidence["root"] / "contention.json", contention)})
    assert check(evidence)["reverse_retest_accepted"]
    measurement.write_text("changed")
    with pytest.raises(ValueError, match="hash changed"):
        check(evidence)


@pytest.mark.parametrize("reversed_order,final_total,expected", [(False, 1., "reverse"), (True, 2., "thresholds failed")])
def test_retry_cannot_waive_wrong_order_or_failed_performance(evidence, reversed_order, final_total, expected):
    retry(evidence, reversed_order=reversed_order, final_total=final_total)
    with pytest.raises(ValueError, match=expected):
        check(evidence)


def test_failed_initial_quality_cannot_use_performance_retry_exception(evidence):
    retry(evidence)
    change_reference(evidence["proof"]["performance_attempts"][0]["candidate"],
                     lambda r: r.update(state="failed"))
    evidence["save_review"]()
    with pytest.raises(ValueError, match="incomplete or failed"):
        check(evidence)


@pytest.mark.parametrize("gate", ["offline_candidate", "benchmark"])
def test_harness_changes_require_rerunning_qualification(evidence, gate):
    ref = (evidence["proof"]["artifacts"][gate] if gate.startswith("offline")
           else evidence["proof"]["performance_attempts"][0]["candidate"])
    change_reference(ref, lambda r: r["harness_hashes"].update({"astrid_mlx_candidate.py": "e" * 64}))
    with pytest.raises(ValueError, match="harness changed"):
        check(evidence)


def test_missing_raw_artifact_evidence_does_not_pass(evidence):
    ref = evidence["proof"]["performance_attempts"][0]["candidate"]
    change_reference(ref, lambda r: r["samples"][0]["generation"].pop("raw_model_artifact_detected"))
    with pytest.raises(ValueError, match="raw model artifact"):
        check(evidence)


@pytest.fixture
def predecessor(evidence):
    root = evidence["root"]
    core_path = evidence["paths"]["candidate"]
    core_release = environments.validate_manifest(core_path)
    core_proof_ref = write(root / "core-proof.json", evidence["proof"])
    activation = {"schema": "model_environment_transition_v1", "status": "activated_verified",
                  "rollback": False, "force_used": False, "manifest": str(core_path), "recorded_at": iso(10100),
                  "old_identity": {"manifest_sha256": core_release["manifest_sha256"],
                                   "qualification": {"proof_sha256": core_proof_ref["sha256"]}},
                  "new_process": {"pid": 42, "started_at": "Thu Oct 15 10:00:00 2026"}}
    refs = {"core_proof": core_proof_ref, "activation_receipt": write(root / "activation.json", activation)}
    manifest = core_release["manifest"]
    bound = [core_path, Path(manifest["package_inventory"]), *Path(manifest["source_root"]).glob("*.py")]
    bound += [Path(path) for path in manifest["artifacts"] if Path(path).name == "METADATA"]
    identities = [{"path": str(path), "sha256": environments.digest(path)} for path in bound]
    for name, review_name, start, duration in (("supervised_watch", "supervised_review", 10101, 1800),
                                             ("passive_soak", "passive_review", 11903, 7200)):
        record = deepcopy(qualification.load_reference(evidence["proof"]["artifacts"]["passive_baseline"]))
        record.update(started_at=iso(start), completed_at=iso(start + duration + 1), duration_s=duration + 1,
                      identity_start=identities, identity_end=deepcopy(identities), samples=[])
        for offset in range(0, duration + 1, 30):
            sample = healthy_sample()
            sample["t_s"] = 1792022400 + start + offset
            record["samples"].append(sample)
        refs[name] = write(root / f"{name}.json", record)
        review = {**review_observation(record), "source_record_sha256": refs[name]["sha256"],
                  "qualified_observation": True}
        refs[review_name] = write(root / f"{review_name}.json", review)
    return {"proof": {"predecessor": refs}, "path": core_path, "release": core_release}


def test_full_update_requires_accepted_core_and_sequential_watch_soak(predecessor):
    result = qualification.verify_predecessor(predecessor["proof"], predecessor["path"], predecessor["release"])
    assert result["manifest"] == {"path": str(predecessor["path"]),
                                  "sha256": predecessor["release"]["manifest_sha256"]}
    assert result["provider"]["pid"] == 42


@pytest.mark.parametrize("edit,expected", [
    (lambda r: r.update(status="failed_requires_review"), "was not activated"),
    (lambda r: r["new_process"].update(pid=99), "different provider"),
    (lambda r: r.update(recorded_at=iso(20000)), "sequential post-activation"),
])
def test_core_acceptance_cannot_claim_failed_or_different_activation(predecessor, edit, expected):
    change_reference(predecessor["proof"]["predecessor"]["activation_receipt"], edit)
    with pytest.raises(ValueError, match=expected):
        qualification.verify_predecessor(predecessor["proof"], predecessor["path"], predecessor["release"])


def test_full_stage_cannot_skip_predecessor(evidence):
    evidence["proof"]["candidate_stage"] = "full"
    with pytest.raises(ValueError, match="accepted core predecessor"):
        check(evidence)


@pytest.fixture
def full_evidence(evidence, predecessor):
    root = evidence["root"]
    full_path = fixture_manifest(root, "full", Path(sys.executable).resolve())
    full = environments.read_json(full_path)
    full["artifacts"].update(generator_sources(full))
    server = full_path.parent / "coupled_astrid_server.py"
    server.write_bytes((predecessor["path"].parent / server.name).read_bytes())
    full["artifacts"][str(server)] = environments.digest(server)
    for name in ("test_wide_coupling_offline.py", "wide_coupling.py"):
        extra = full_path.parent / name
        extra.write_bytes((predecessor["path"].parent / name).read_bytes())
        full["artifacts"][str(extra)] = environments.digest(extra)
    write(full_path, full)
    core = predecessor["release"]["manifest"]
    proof = deepcopy(evidence["proof"])
    proof.update(candidate_stage="full", predecessor=predecessor["proof"]["predecessor"])
    refs = proof["artifacts"]
    refs["baseline_manifest"] = qualification.reference(predecessor["path"])
    refs["candidate_manifest"] = qualification.reference(full_path)
    preparation = qualification.load_reference(refs["preparation"])
    preparation["stages"]["full"] = {key: full[key] for key in ("python", "source_root", "packages")}
    refs["preparation"] = write(root / "full-preparation.json", preparation)
    refs["live_inventory"] = qualification.reference(Path(core["package_inventory"]))
    refs["passive_baseline"] = proof["predecessor"]["passive_soak"]
    refs["passive_review"] = proof["predecessor"]["passive_review"]
    accepted_predecessor = qualification.verify_predecessor(
        proof, predecessor["path"], predecessor["release"])
    pair = {}
    for role, stage, manifest in (("baseline", "core", core), ("candidate", "full", full)):
        identity = {"stage": stage, "qualification_target": "full",
                    "predecessor_acceptance": accepted_predecessor,
                    "packages": manifest["packages"],
                    "source_root": manifest["source_root"], "python": manifest["python"],
                    "manifest_sha256": refs[f"{role}_manifest"]["sha256"],
                    "preparation_sha256": refs["preparation"]["sha256"],
                    "baseline_soak_sha256": refs["passive_baseline"]["sha256"]}
        offline = qualification.load_reference(evidence["proof"]["artifacts"]["offline_candidate"])
        offline.update(identity, started_at=iso(20000), completed_at=iso(20100))
        generator = next(t for t in offline["tests"] if t["name"] == "generator-contract")
        generator["generator_audit"] = generator_audit(root, f"full-{role}", manifest, offline["harness_hashes"],
            mode="full_prefilled_prompt" if stage == "full" else "final_prompt_token_only", start=20001, end=20099)
        wide = next(t for t in offline["tests"] if t["name"] == "wide-coupling")
        embedding = qualification.load_reference(wide["embedding_audit"])
        embedding.update(source_root=manifest["source_root"], started_at=iso(20001), completed_at=iso(20099),
                         test_path=str(Path(manifest["source_root"]) / "test_wide_coupling_offline.py"))
        embedding["calls"][0]["caller"] = str(Path(manifest["source_root"]) / "coupled_astrid_server.py")
        wide["embedding_audit"] = write(root / f"full-{role}-embedding.json", embedding)
        memory = [{"t_s": 1792022400 + second, "pressure_level": 1, "swap_bytes": 0}
                  for second in range(20000, 20101, 5)]
        offline["host_memory"] = {**evaluate_memory(memory), "samples": memory}
        refs[f"offline_{role}"] = write(root / f"full-offline-{role}.json", offline)
        benchmark = qualification.load_reference(evidence["proof"]["performance_attempts"][0]["candidate"])
        start = 21000 if role == "baseline" else 21200
        benchmark.update(identity, offline_receipt_sha256=refs[f"offline_{role}"]["sha256"],
                         started_at=iso(start), completed_at=iso(start + 100))
        memory = [{"t_s": 1792022400 + second, "pressure_level": 1, "swap_bytes": 0}
                  for second in range(start, start + 101, 5)]
        benchmark["host_memory"] = {**evaluate_memory(memory), "samples": memory}
        pair[role] = write(root / f"full-benchmark-{role}.json", benchmark)
    proof["performance_attempts"] = [pair]
    review = qualification.load_reference(evidence["proof"]["artifacts"]["output_review"])
    review.update(reviewed_at=iso(23000), benchmark_sha256=[ref["sha256"] for ref in pair.values()],
                  baseline_manifest_sha256=refs["baseline_manifest"]["sha256"],
                  candidate_manifest_sha256=refs["candidate_manifest"]["sha256"],
                  sample_reviews=[{"benchmark_sha256": pair[role]["sha256"], "workload_id": row["id"],
                      "repetition": 0, "accepted": True, "comments": "Full stage synthetic review fixture."}
                      for role in pair for row in workloads()])
    refs["output_review"] = write(root / "full-output-review.json", review)
    return {"proof": proof, "paths": {"baseline": predecessor["path"], "candidate": full_path},
            "original_baseline": evidence["proof"]["artifacts"]["passive_baseline"],
            "original_review": evidence["proof"]["artifacts"]["passive_review"]}


def test_full_proof_uses_core_soak_inventory_and_matched_core_comparison(full_evidence):
    result = check(full_evidence)
    assert result["accepted"] and result["candidate_stage"] == "full"
    assert result["predecessor_acceptance"]["manifest"]["path"] == str(full_evidence["paths"]["baseline"])


def test_full_prework_admission_requires_actual_core_lineage(full_evidence):
    proof = full_evidence["proof"]
    refs = proof["predecessor"]
    artifacts = proof["artifacts"]
    args = SimpleNamespace(
        qualification_target="full", stage="core",
        manifest=full_evidence["paths"]["baseline"],
        core_manifest=full_evidence["paths"]["baseline"],
        baseline_soak=Path(refs["passive_soak"]["path"]),
        core_proof=Path(refs["core_proof"]["path"]),
        activation_receipt=Path(refs["activation_receipt"]["path"]),
        supervised_watch=Path(refs["supervised_watch"]["path"]),
        supervised_review=Path(refs["supervised_review"]["path"]),
        passive_review=Path(refs["passive_review"]["path"]))
    preparation = qualification.load_reference(artifacts["preparation"])
    accepted = packages.validate_predecessor_for_work(
        args, refs["passive_soak"]["sha256"], preparation)
    assert accepted["passive_soak"] == refs["passive_soak"]
    args.stage = "full"
    args.manifest = full_evidence["paths"]["candidate"]
    assert packages.validate_predecessor_for_work(
        args, refs["passive_soak"]["sha256"], preparation) == accepted
    args.baseline_soak = Path(full_evidence["original_baseline"]["path"])
    with pytest.raises(ValueError, match="baseline changed"):
        packages.validate_predecessor_for_work(args, refs["passive_soak"]["sha256"], preparation)


def test_full_prework_rejects_alternate_core_manifest_and_future_soak(full_evidence, monkeypatch):
    proof = full_evidence["proof"]
    refs = proof["predecessor"]
    core = full_evidence["paths"]["baseline"]
    args = SimpleNamespace(
        qualification_target="full", stage="core", manifest=core, core_manifest=core,
        baseline_soak=Path(refs["passive_soak"]["path"]),
        core_proof=Path(refs["core_proof"]["path"]),
        activation_receipt=Path(refs["activation_receipt"]["path"]),
        supervised_watch=Path(refs["supervised_watch"]["path"]),
        supervised_review=Path(refs["supervised_review"]["path"]),
        passive_review=Path(refs["passive_review"]["path"]))
    preparation = qualification.load_reference(proof["artifacts"]["preparation"])
    alternate = core.parent / "alternate-core.json"
    alternate.write_bytes(core.read_bytes())
    args.manifest = alternate
    with pytest.raises(ValueError, match="selected Stage 2 core manifest"):
        packages.validate_predecessor_for_work(args, refs["passive_soak"]["sha256"], preparation)
    args.manifest = core
    soak_end = qualification.interval(qualification.load_reference(refs["passive_soak"]))[1]
    monkeypatch.setattr(packages, "datetime", SimpleNamespace(
        now=lambda _tz: SimpleNamespace(timestamp=lambda: soak_end - 1)))
    with pytest.raises(ValueError, match="has not completed at admission time"):
        packages.validate_predecessor_for_work(args, refs["passive_soak"]["sha256"], preparation)


def test_full_proof_rejects_core_target_receipt(full_evidence):
    ref = full_evidence["proof"]["artifacts"]["offline_baseline"]
    change_reference(ref, lambda record: record.update(qualification_target="core"))
    with pytest.raises(ValueError, match="receipt provenance"):
        check(full_evidence)


def test_full_proof_rejects_receipt_without_prework_admission(full_evidence):
    ref = full_evidence["proof"]["artifacts"]["offline_baseline"]
    change_reference(ref, lambda record: record.pop("predecessor_acceptance"))
    with pytest.raises(ValueError, match="did not admit"):
        check(full_evidence)


def test_full_proof_rejects_matched_work_without_prework_admission(full_evidence):
    ref = full_evidence["proof"]["performance_attempts"][0]["candidate"]
    change_reference(ref, lambda record: record.pop("predecessor_acceptance"))
    with pytest.raises(ValueError, match="did not admit"):
        check(full_evidence)


def test_full_proof_cannot_reuse_pre_upgrade_passive_baseline(full_evidence):
    full_evidence["proof"]["artifacts"].update(passive_baseline=full_evidence["original_baseline"],
                                             passive_review=full_evidence["original_review"])
    with pytest.raises(ValueError, match="accepted core soak"):
        check(full_evidence)


def test_full_qualification_cannot_run_before_core_soak_is_accepted(full_evidence):
    change_reference(full_evidence["proof"]["artifacts"]["offline_candidate"],
                     lambda r: r.update(started_at=iso(10000)))
    with pytest.raises(ValueError, match="ran during the passive baseline"):
        check(full_evidence)


def test_reservoir_connected_without_checkin_is_not_a_pass(evidence):
    change_reference(evidence["proof"]["performance_attempts"][0]["candidate"],
                     lambda r: r["samples"][0]["readyz_after"]["reservoir"].update(operation="pull_state"))
    with pytest.raises(ValueError, match="check-in"):
        check(evidence)


def test_offline_log_mutation_invalidates_proof(evidence):
    offline = qualification.load_reference(evidence["proof"]["artifacts"]["offline_candidate"])
    Path(offline["tests"][0]["log"]).write_text("changed")
    with pytest.raises(ValueError, match="hash changed"):
        check(evidence)


@pytest.mark.parametrize("edit,expected", [
    (lambda r: r["trial_evidence"][0].update(exit_code=-11), "process exit"),
    (lambda r: r["trial_evidence"].pop(), "process exit"),
    (lambda r: r["trial_evidence"][0].update(metrics_sha256="e" * 64), "hash changed"),
])
def test_candidate_crash_or_missing_process_audit_evidence_blocks_proof(evidence, edit, expected):
    change_reference(evidence["proof"]["performance_attempts"][0]["candidate"], edit)
    with pytest.raises(ValueError, match=expected):
        check(evidence)


def test_adapted_coupling_is_preserved_as_a_matched_input(evidence):
    assert check(evidence)["accepted"]
    change_reference(evidence["proof"]["performance_attempts"][0]["candidate"],
                     lambda r: r.update(initial_restored_coupling_strength=.1))
    with pytest.raises(ValueError, match="coupling differs"):
        check(evidence)


def test_missing_restored_coupling_does_not_pass(evidence):
    change_reference(evidence["proof"]["performance_attempts"][0]["candidate"],
                     lambda r: r.pop("initial_restored_coupling_strength"))
    with pytest.raises(ValueError, match="coupling evidence"):
        check(evidence)


@pytest.mark.parametrize("checkout", [False, None])
def test_successful_checkin_cannot_hide_failed_or_missing_checkout(evidence, checkout):
    ref = evidence["proof"]["performance_attempts"][0]["candidate"]
    change_reference(ref, lambda r: r["samples"][0]["reservoir_audit"].update(checkout_succeeded=checkout))
    with pytest.raises(ValueError, match="checkout evidence"):
        check(evidence)


@pytest.mark.parametrize("repetition", [0, 1, 2])
def test_each_trial_must_actually_restore_journal_coupling(evidence, repetition):
    ref = evidence["proof"]["performance_attempts"][0]["candidate"]
    change_reference(ref, lambda r: r["samples"][repetition * len(workloads())]["response"]["coupled_generation_v1"]["controls"]
                     .update(reservoir_coupling_strength=.1))
    with pytest.raises(ValueError, match="actual initial coupling"):
        check(evidence)


def test_later_requests_may_adapt_coupling(evidence):
    ref = evidence["proof"]["performance_attempts"][0]["candidate"]
    change_reference(ref, lambda r: r["samples"][1]["response"]["coupled_generation_v1"]["controls"]
                     .update(reservoir_coupling_strength=.1))
    evidence["save_review"]()
    assert check(evidence)["accepted"]


@pytest.mark.parametrize("edit", [
    lambda audit: audit.update(passed=False),
    lambda audit: audit.update(python="/foreign/bin/python"),
    lambda audit: audit.update(expected_history_mode="full_prefilled_prompt"),
    lambda audit: audit["versions"].update(mlx="unqualified"),
    lambda audit: audit["device"].update(generation_stream="gpu"),
    lambda audit: audit.update(cpu_selected_before_generator_import=False),
    lambda audit: audit.update(wrapper_sha256="0" * 64),
    lambda audit: audit.update(trace=[]),
    lambda audit: audit.update(checks={}),
    lambda audit: audit.update(source_hashes={}),
    lambda audit: audit.update(started_at=iso(7299)),
])
def test_real_generator_contract_cannot_be_qualified_by_pass_flag_alone(evidence, edit):
    offline = qualification.load_reference(evidence["proof"]["artifacts"]["offline_candidate"])
    gate = next(row for row in offline["tests"] if row["name"] == "generator-contract")
    audit = qualification.load_reference(gate["generator_audit"])
    edit(audit)
    with pytest.raises(ValueError):
        qualification.verify_generator_audit(audit, offline)


def test_generator_package_source_mutation_blocks_proof(evidence):
    offline = qualification.load_reference(evidence["proof"]["artifacts"]["offline_candidate"])
    gate = next(row for row in offline["tests"] if row["name"] == "generator-contract")
    audit = qualification.load_reference(gate["generator_audit"])
    Path(next(iter(audit["source_hashes"]))).write_text("changed package code\n")
    with pytest.raises(ValueError, match="hash changed"):
        qualification.verify_generator_audit(audit, offline)


def test_generator_shadow_copy_under_same_environment_is_rejected(evidence):
    offline = qualification.load_reference(evidence["proof"]["artifacts"]["offline_candidate"])
    gate = next(row for row in offline["tests"] if row["name"] == "generator-contract")
    audit = qualification.load_reference(gate["generator_audit"])
    source = next(path for path in audit["source_hashes"] if Path(path).name == "generate.py")
    shadow = Path(offline["python"]).parent.parent / "lib/shadow/generate.py"
    shadow.parent.mkdir()
    shadow.write_bytes(Path(source).read_bytes())
    audit["source_hashes"][str(shadow)] = audit["source_hashes"].pop(source)
    with pytest.raises(ValueError, match="installed package paths"):
        qualification.verify_generator_audit(audit, offline)


def test_generator_unrecorded_package_file_is_rejected(evidence):
    offline = qualification.load_reference(evidence["proof"]["artifacts"]["offline_candidate"])
    gate = next(row for row in offline["tests"] if row["name"] == "generator-contract")
    audit = qualification.load_reference(gate["generator_audit"])
    source = Path(next(path for path in audit["source_hashes"] if Path(path).name == "generate.py"))
    record = source.parent.parent / "mlx_lm-0.0.0.dist-info/RECORD"
    record.write_text("mlx_lm-0.0.0.dist-info/RECORD,,\n")
    with pytest.raises(ValueError, match="installed wheel RECORD"):
        qualification.verify_generator_audit(audit, offline)
