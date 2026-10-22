"""Model-free tests of the prospective exact-campaign memory-policy boundary."""
import hashlib
import json
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import Mock
import pytest
import astrid_mlx_memory as memory
import model_qualification as qualification
import qualify_mlx_packages as packages
from test_model_qualification import evidence, check, change_reference


def rows(pressure=1):
    return [{"t_s": float(i), "pressure_level": pressure, "swap_bytes": 1000. + i * 1000000}
            for i in range(0, 76, 5)]


def test_swap_growth_is_diagnostic_only_when_explicit():
    samples = rows()
    assert not memory.evaluate(samples)["automated_ok"]
    result = memory.evaluate(samples, policy=memory.SWAP_DIAGNOSTIC_POLICY)
    assert result["automated_ok"] and result["swap_is_diagnostic"]
    assert result["retained_swap_growth_bytes"] == 75000000
    assert result["additional_swap_longest_s"] == 70
    assert result["initial_swap_bytes"] == 1000
    assert result["final_swap_bytes"] == 75001000


def test_initial_warning_allowed_without_wait_but_critical_refused(monkeypatch):
    for level in (1, 2, 3, 4):
        monitor = memory.Monitor(policy=memory.SWAP_DIAGNOSTIC_POLICY)
        monitor.thread = Mock()
        monkeypatch.setattr(memory, "sample", lambda: {"t_s": 0., "pressure_level": level, "swap_bytes": 99.})
        if level <= 2:
            monitor.start()
            monitor.thread.start.assert_called_once()
        else:
            with pytest.raises(ValueError, match="noncritical"):
                monitor.start()
            monitor.thread.start.assert_not_called()


def test_warning_duration_boundary_and_reset():
    samples = rows()
    for sample in samples[:12]:
        sample["pressure_level"] = 2
    assert memory.evaluate(samples, policy=memory.SWAP_DIAGNOSTIC_POLICY)["automated_ok"]
    samples[12]["pressure_level"] = 2
    assert not memory.evaluate(samples, policy=memory.SWAP_DIAGNOSTIC_POLICY)["automated_ok"]
    samples[6]["pressure_level"] = 1
    assert memory.evaluate(samples, policy=memory.SWAP_DIAGNOSTIC_POLICY)["automated_ok"]


@pytest.mark.parametrize("mutate", [
    lambda r: r[4].update(pressure_level=3), lambda r: r[4].update(pressure_level=4),
    lambda r: r[4].update(pressure_level=0), lambda r: r[4].update(pressure_level=1.5),
    lambda r: r[4].update(swap_bytes=float("nan")), lambda r: r[4].update(swap_bytes=-1),
    lambda r: r[4].update(t_s=r[3]["t_s"]), lambda r: r[4].update(error="unavailable"),
    lambda r: r.__delitem__(slice(4, 8)),
])
def test_diagnostic_policy_does_not_hide_bad_telemetry(mutate):
    samples = rows()
    mutate(samples)
    assert not memory.evaluate(samples, policy=memory.SWAP_DIAGNOSTIC_POLICY)["automated_ok"]


def campaign_args(stage="core"):
    return dict(preparation_hash=packages.DIAGNOSTIC_PREPARATION,
                baseline_hash=packages.DIAGNOSTIC_BASELINE,
                manifest_hash=packages.DIAGNOSTIC_MANIFESTS[stage], stage=stage,
                qualification_target="core")


@pytest.mark.parametrize("field,value", [("preparation_hash", "f"*64), ("baseline_hash", "f"*64),
    ("manifest_hash", "f"*64), ("stage", "full"), ("qualification_target", "full")])
def test_policy_does_not_widen_to_other_inputs_or_stage2(field, value):
    args = campaign_args()
    packages.validate_memory_policy(memory.SWAP_DIAGNOSTIC_POLICY, **args)
    args[field] = value
    with pytest.raises(ValueError, match="limited"):
        packages.validate_memory_policy(memory.SWAP_DIAGNOSTIC_POLICY, **args)
    packages.validate_memory_policy(memory.STRICT_POLICY, **args)


def test_policy_refusal_precedes_output_and_model_work(tmp_path, monkeypatch):
    prep = tmp_path / "prep.json"
    prep.write_text(json.dumps({"live_environment_unchanged": True, "completed_at": "yes"}))
    args = SimpleNamespace(baseline_soak=tmp_path / "hour.json", stage="full", qualification_target="full",
                           preparation=prep, manifest=tmp_path / "manifest.json",
                           memory_policy=memory.SWAP_DIAGNOSTIC_POLICY, output_dir=tmp_path / "new")
    monkeypatch.setattr(packages, "validate_baseline", lambda *a, **k: packages.DIAGNOSTIC_BASELINE)
    monkeypatch.setattr(packages, "validate_predecessor_for_work", lambda *a, **k: {})
    monkeypatch.setattr(packages, "validate_stage", lambda *a, **k: {})
    monkeypatch.setattr(packages, "validate_release", lambda *a, **k: "f"*64)
    monkeypatch.setattr(packages, "validate_offline", lambda *a, **k: pytest.fail("effect boundary crossed"))
    with pytest.raises(ValueError, match="limited"):
        packages.run(args)
    assert not args.output_dir.exists()


def test_historical_compatibility_is_not_current_harness_permission():
    raw = Path(packages.__file__).with_name(packages.COMPATIBILITY_FILE).read_bytes()
    bridge = json.loads(raw)
    historical = {**bridge["current_harness"],
                  packages.COMPATIBILITY_FILE: hashlib.sha256(raw).hexdigest()}
    for stage, ref in packages.LEGACY_OFFLINE.items():
        record = {"schema": "mlx_offline_qualification_v1", "stage": stage,
                  "passed": True, "qualification_target": "core",
                  "preparation_sha256": packages.DIAGNOSTIC_PREPARATION,
                  "baseline_soak_sha256": packages.DIAGNOSTIC_BASELINE,
                  "manifest_sha256": packages.DIAGNOSTIC_MANIFESTS[stage],
                  "harness_hashes": bridge["legacy_harness"]}
        assert packages.offline_harness_matches(record, ref, historical)
        assert not packages.offline_harness_matches(record, ref, packages.harness_hashes())
        for name in ("astrid_mlx_candidate.py", "astrid_mlx_memory.py", packages.COMPATIBILITY_FILE):
            assert not packages.offline_harness_matches(record, ref, {**historical, name: "f"*64})
        assert not packages.offline_harness_matches(record, {**ref, "sha256": "f"*64}, historical)
        assert not packages.offline_harness_matches(record, {**ref, "path": str(ref["path"])+".copy"}, historical)
        assert not packages.offline_harness_matches({**record, "qualification_target": "full"}, ref, historical)


def test_retained_historical_offline_pair_stays_strict_and_requires_its_harness():
    if not all(Path(ref["path"]).is_file() for ref in packages.LEGACY_OFFLINE.values()):
        pytest.skip("requires retained local historical qualification evidence")
    current = packages.harness_hashes()
    for stage, ref in packages.LEGACY_OFFLINE.items():
        path = Path(ref["path"])
        raw = path.read_bytes()
        assert hashlib.sha256(raw).hexdigest() == ref["sha256"]
        record = json.loads(raw)
        assert not packages.offline_harness_matches(record, ref, current)
        qualification.verify_offline(record)  # Reads retained evidence; no model/process call.
        preparation = {"stages": {stage: {"packages": record["packages"]}}, "source_hashes": record["source_hashes"]}
        with pytest.raises(ValueError, match="exact stage"):
            packages.validate_offline(path, preparation, stage, record["manifest_sha256"],
                record["baseline_soak_sha256"], "core", None)
        assert path.read_bytes() == raw


def stamp_diagnostic(evidence, monkeypatch, *, candidate_only=False):
    proof = evidence["proof"]
    refs = proof["artifacts"]
    monkeypatch.setattr(packages, "DIAGNOSTIC_PREPARATION", refs["preparation"]["sha256"])
    monkeypatch.setattr(packages, "DIAGNOSTIC_BASELINE", refs["passive_baseline"]["sha256"])
    monkeypatch.setattr(packages, "DIAGNOSTIC_MANIFESTS", {
        "baseline": refs["baseline_manifest"]["sha256"], "core": refs["candidate_manifest"]["sha256"]})
    def stamp(record):
        samples = record["host_memory"]["samples"]
        for i, sample in enumerate(samples):
            sample["swap_bytes"] = i * 1000000
        record["memory_policy"] = memory.SWAP_DIAGNOSTIC_POLICY
        record["host_memory"] = {**memory.evaluate(samples, policy=memory.SWAP_DIAGNOSTIC_POLICY), "samples": samples}
    for role, ref in proof["performance_attempts"][0].items():
        if not candidate_only or role == "candidate":
            change_reference(ref, stamp)
    evidence["save_review"]()


def test_final_proof_and_activation_reader_recompute_bound_policy(evidence, monkeypatch):
    stamp_diagnostic(evidence, monkeypatch)
    result = check(evidence)
    assert result["accepted"] and result["memory_policy"] == memory.SWAP_DIAGNOSTIC_POLICY
    path = evidence["root"] / "proof.json"
    path.write_text(json.dumps(evidence["proof"]))
    assert qualification.verify_proof(path, evidence["paths"]["candidate"], evidence["paths"]["baseline"])["accepted"]
    ref = evidence["proof"]["performance_attempts"][0]["candidate"]
    change_reference(ref, lambda r: r["host_memory"]["samples"][1].update(pressure_level=3))
    evidence["save_review"]()
    with pytest.raises(ValueError, match="memory pressure"):
        check(evidence)


def test_mixed_matched_policy_is_not_comparable(evidence, monkeypatch):
    stamp_diagnostic(evidence, monkeypatch, candidate_only=True)
    with pytest.raises(ValueError, match="matched memory policy differs"):
        check(evidence)


def test_policy_cannot_relabel_old_failed_benchmark(evidence, monkeypatch):
    stamp_diagnostic(evidence, monkeypatch)
    ref = evidence["proof"]["performance_attempts"][0]["candidate"]
    change_reference(ref, lambda r: r.update(state="failed"))
    evidence["save_review"]()
    with pytest.raises(ValueError, match="benchmark"):
        check(evidence)


def test_policy_stamp_must_match_recomputed_memory_record(evidence, monkeypatch):
    stamp_diagnostic(evidence, monkeypatch)
    ref = evidence["proof"]["performance_attempts"][0]["candidate"]
    change_reference(ref, lambda r: r["host_memory"].update(policy=memory.STRICT_POLICY))
    evidence["save_review"]()
    with pytest.raises(ValueError, match="policy"):
        check(evidence)
