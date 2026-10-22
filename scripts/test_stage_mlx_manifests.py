"""Model-free release staging tests; no installed interpreter is executed."""
from __future__ import annotations

import base64
import hashlib
import json
from pathlib import Path
import plistlib

import pytest

import model_environment as models
import stage_mlx_manifests as staging


def save(path, value):
    path.write_text(json.dumps(value))


def make_env(root, packages):
    python = root / "bin/python"
    python.parent.mkdir(parents=True)
    python.write_text("#!/bin/sh\nexit 99\n")
    python.chmod(0o700)
    (root / "pyvenv.cfg").write_text("include-system-site-packages = false\n")
    site = root / "lib/python3.12/site-packages"
    site.mkdir(parents=True)
    for name, version in packages.items():
        dist = site / f"{name.replace('-', '_')}-{version}.dist-info"
        dist.mkdir()
        (dist / "METADATA").write_text(f"Name: {name}\nVersion: {version}\n")
        payload = site / f"{name}.py"
        payload.write_text("# installed fixture bytes\n")
        hashed = base64.urlsafe_b64encode(hashlib.sha256(payload.read_bytes()).digest()).rstrip(b"=").decode()
        (dist / "RECORD").write_text(f"{payload.name},sha256={hashed},{payload.stat().st_size}\n")
    return str(python)


@pytest.fixture
def prepared(tmp_path):
    root = tmp_path.resolve()
    live, evidence, model = (root / name for name in ("live", "evidence", "model"))
    for path in (live, evidence, model):
        path.mkdir()
    (live / "coupled_astrid_server.py").write_text("# qualified frozen source\n")
    (live / "generation_controls.py").write_text("# qualified controls\n")
    (model / "config.json").write_text("{}")
    blob = root / "weight-blob"
    blob.write_bytes(b"model weight fixture")
    (model / "model.safetensors").symlink_to(blob)
    base_packages = {"mlx": "0.32.0", "mlx-metal": "0.32.0", "mlx-lm": "0.31.3"}
    original_python = make_env(live / ".venv", base_packages)
    record = {"schema": "mlx_environment_preparation_v1", "completed_at": "2026-10-15T00:00:00Z",
              "live_environment_unchanged": True, "source_root": str(live),
              "source_revision": "e719cd7f06af1c63fa3288c9689e479a47503586",
              "source_hashes": {p.name: models.digest(p) for p in live.glob("*.py")},
              "baseline_python": original_python, "baseline_python_sha256": models.digest(Path(original_python)),
              "model_snapshot": str(model), "stages": {}}
    for name in ("baseline", "core", "full"):
        packages = dict(base_packages)
        if name != "baseline":
            packages.update({"mlx": "0.32.4", "mlx-metal": "0.32.4"})
        if name == "full":
            packages["mlx-lm"] = "0.32.0"
        stage = root / name
        source = stage / "source"
        source.mkdir(parents=True)
        for filename in record["source_hashes"]:
            (source / filename).write_bytes((live / filename).read_bytes())
        python = make_env(stage / ".venv", packages)
        lock = evidence / f"{name}.requirements.lock"
        lock.write_text("# fake lock\n")
        record["stages"][name] = {"python": python, "source_root": str(source),
                                  "packages": packages, "requirements_lock": str(lock)}
        save(evidence / f"{name}-inventory.json", {"executable": python, "python": "3.12.13", "packages": packages})
    save(evidence / "live-inventory.json", {"executable": original_python, "python": "3.12.13", "packages": base_packages})
    save(evidence / "wheel-inventory.json", {"fixture": True})
    preparation = evidence / "preparation.json"
    save(preparation, record)
    settings = {"Label": "com.reservoir.coupled-astrid", "WorkingDirectory": str(live),
                "ProgramArguments": [original_python, str(live / "coupled_astrid_server.py"),
                                     "--port", "8090", "--coupling-strength", "0.1", "--model-memory-map",
                                     "--model", staging.MODEL_REPOSITORY, "--model-qos-mode", "active"],
                "KeepAlive": True, "RunAtLoad": True,
                "StandardOutPath": str(live / "logs/coupled-astrid.log"),
                "StandardErrorPath": str(live / "logs/coupled-astrid.log")}
    plist = root / "original.plist"
    plist.write_bytes(plistlib.dumps(settings))
    return preparation, plist, evidence / "manifests", record


def run(prepared):
    preparation, plist, output, _record = prepared
    return staging.stage_manifests(preparation, plist, output, "fixture", launch_environment={})


def test_generates_valid_manifests_with_original_rollback_and_preserved_paths(prepared):
    preparation, plist, output, record = prepared
    live = Path(record["source_root"])
    before = {str(p): (p.read_bytes(), p.stat().st_mtime_ns) for p in live.rglob("*") if p.is_file()}
    receipt = run(prepared)
    assert set(receipt["manifests"]) == {"baseline", "core", "full", "original"}
    assert receipt["production_modified"] is False
    manifests = {name: models.validate_manifest(Path(item["path"]))["manifest"]
                 for name, item in receipt["manifests"].items()}
    assert manifests["original"]["python"] == record["baseline_python"]
    assert manifests["original"]["source_root"] == record["stages"]["baseline"]["source_root"]
    assert manifests["baseline"]["python"] != manifests["original"]["python"]
    for manifest in manifests.values():
        assert manifest["argv"] == manifests["original"]["argv"]
        assert manifest["working_directory"] == str(live)
        assert manifest["runtime_paths"]["coupling_journal"] == str(live / "state/coupling_journal.json")
        assert models.option(manifest["argv"], "--model-qos-receipts") == str(live / "workspace/model_qos_receipts.jsonl")
        assert str(Path(record["model_snapshot"]) / "model.safetensors") in manifest["artifacts"]
        assert str(plist) not in manifest["artifacts"]
        assert str(output / "original-launch-settings.json") in manifest["artifacts"]
    after = {str(p): (p.read_bytes(), p.stat().st_mtime_ns) for p in live.rglob("*") if p.is_file()}
    assert before == after
    assert (output / "full.json").stat().st_mode & 0o777 == 0o400
    assert (output / "staging.json").is_file()


def test_existing_output_is_never_overwritten(prepared):
    output = prepared[2]
    output.mkdir()
    sentinel = output / "old.json"
    sentinel.write_text("preserve me")
    with pytest.raises(ValueError, match="already exists"):
        run(prepared)
    assert sentinel.read_text() == "preserve me"
    assert list(output.iterdir()) == [sentinel]


def test_original_launch_drift_prevents_rollback_manifest(prepared):
    plist = prepared[1]
    settings = plistlib.loads(plist.read_bytes())
    settings["ProgramArguments"][-1] = "shadow"
    plist.write_bytes(plistlib.dumps(settings))
    with pytest.raises(ValueError, match="launch settings"):
        run(prepared)
    assert not prepared[2].exists()


def test_live_environment_override_requires_review(prepared):
    with pytest.raises(ValueError, match="environment differs"):
        staging.stage_manifests(*prepared[:3], "fixture", launch_environment={"COUPLED_ASTRID_WIDE_STRENGTH": ".2"})
    assert not prepared[2].exists()


def test_source_drift_is_not_blessed(prepared):
    record = prepared[3]
    (Path(record["stages"]["full"]["source_root"]) / "new.py").write_text("# unexpected\n")
    with pytest.raises(ValueError, match="source closure"):
        run(prepared)
    assert not (prepared[2] / "staging.json").exists()


def test_installed_payload_drift_is_not_blessed(prepared):
    python = Path(prepared[3]["stages"]["baseline"]["python"])
    payload = python.parent.parent / "lib/python3.12/site-packages/mlx.py"
    payload.write_text("changed after RECORD creation")
    with pytest.raises(ValueError, match="package content changed"):
        run(prepared)
    assert not (prepared[2] / "staging.json").exists()


def test_mismatched_inventory_is_not_blessed(prepared):
    inventory = prepared[0].parent / "live-inventory.json"
    value = json.loads(inventory.read_text())
    value["packages"]["mlx"] = "99.0"
    save(inventory, value)
    with pytest.raises(ValueError, match="inventory does not match"):
        run(prepared)


def test_production_output_path_is_refused(prepared):
    preparation, plist, _output, record = prepared
    output = Path(record["source_root"]) / "manifest-evidence"
    with pytest.raises(ValueError, match="outside the runtime tree"):
        staging.stage_manifests(preparation, plist, output, "fixture", launch_environment={})
    assert not output.exists()


def test_mutating_live_plist_later_does_not_invalidate_rollback(prepared):
    receipt = run(prepared)
    prepared[1].write_text("replacement launch configuration")
    models.validate_manifest(Path(receipt["manifests"]["original"]["path"]))


@pytest.mark.parametrize("modified", ["wrapper", "runtime"])
def test_framework_python_wrapper_and_runtime_are_both_bound(prepared, modified):
    python = Path(prepared[3]["stages"]["full"]["python"])
    framework = prepared[0].parent.parent / "Python.framework/Versions/3.12"
    wrapper = framework / "bin/python3.12"
    runtime = framework / "Resources/Python.app/Contents/MacOS/Python"
    for path in (wrapper, runtime):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(f"#!/bin/sh\n# {path.name}\nexit 99\n")
        path.chmod(0o700)
    python.unlink()
    python.symlink_to(wrapper)
    receipt = run(prepared)
    manifest_path = Path(receipt["manifests"]["full"]["path"])
    manifest = models.validate_manifest(manifest_path)["manifest"]
    assert manifest["artifacts"][str(wrapper)] == models.digest(wrapper)
    assert manifest["artifacts"][str(runtime)] == models.digest(runtime)
    (wrapper if modified == "wrapper" else runtime).write_text("unqualified executable replacement")
    with pytest.raises(ValueError, match="artifact changed"):
        models.validate_manifest(manifest_path)


def test_framework_without_runtime_is_refused(prepared):
    python = Path(prepared[3]["stages"]["full"]["python"])
    wrapper = prepared[0].parent.parent / "Python.framework/Versions/3.12/bin/python3.12"
    wrapper.parent.mkdir(parents=True)
    wrapper.write_text("#!/bin/sh\nexit 99\n")
    wrapper.chmod(0o700)
    python.unlink()
    python.symlink_to(wrapper)
    with pytest.raises(ValueError, match="runtime executable is missing"):
        run(prepared)
    assert not (prepared[2] / "staging.json").exists()
