"""Observe-only must never enter any live mutation path, even on failure."""

from __future__ import annotations

import json
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import astrid_live_soak as soak
import astrid_soak_observer as observer


def identity(pid):
    return {"returncode": 0, "stdout": f"{pid} Thu Oct 15 10:00:00 2026 /usr/bin/python service\n"}


def healthy_sample():
    return {"candidate_pids": [42], "provider": {"pids": [42], "identity": identity(42)},
        "services": {"com.astrid.spectral-bridge": {
            "returncode": 0, "stdout": "state = running\npid = 44", "identity": identity(44)}},
        "readyz": {"status": 200, "json": {"ready": True,
            "worker": {"phase": "generating", "last_generation_error": None},
            "reservoir": {"status": "connected"}}},
        "ollama": {"pids": [43], "identity": identity(43),
                   "version": {"json": {"version": "0.25.2"}}}}


@pytest.mark.parametrize("outcome,expected", [("success", 0), ("error", 1), ("interrupt", 130)])
def test_observe_only_never_mutates_services(monkeypatch, tmp_path, outcome, expected):
    bridge = tmp_path / "bridge.log"
    model = tmp_path / "model.log"
    bridge.touch()
    model.touch()
    source = tmp_path / "source.py"
    source.write_text("# unchanged source\n")

    def forbidden(*_args, **_kwargs):
        pytest.fail("observer reached active live-control path")

    for module, names in ((soak, ("launchctl", "restart_bridge", "run_narrow_probes", "monitor_soak")),
                          (soak.narrow, ("start_candidate_server", "stop_candidate_server", "wait_for_models", "chat_probe"))):
        for name in names:
            monkeypatch.setattr(module, name, forbidden)

    def sample(*_args):
        if outcome == "error":
            raise RuntimeError("inspection failed")
        if outcome == "interrupt":
            raise KeyboardInterrupt
        model.write_text("generated 10 tokens on astrid in 1.0s (10.0 tok/s)\n")
        return healthy_sample()

    monkeypatch.setattr(observer, "status", sample)
    monkeypatch.setattr(observer, "command", lambda cmd: {"returncode": 0, "stdout": ""})
    monkeypatch.setattr(sys, "argv", ["soak", "--observe-only", "--candidate-port", "8090",
        "--existing-server-log", str(model), "--bridge-log", str(bridge),
        "--identity-file", str(source),
        "--bridge-workspace", str(tmp_path / "workspace"), "--duration-secs", "0",
        "--output-dir", str(tmp_path / "evidence")])
    assert soak.main() == expected
    record = json.loads(next((tmp_path / "evidence").glob("*/soak_result.json")).read_text())
    assert record["live_mutations"] == []
    assert record["restore"] is None
    assert record["probes"] == []
    assert record["state"] == {"success": "completed", "error": "error", "interrupt": "interrupted"}[outcome]
    assert record["monitor"]["performance_evidence"] == "not_available"


def test_log_rotation_is_recorded_instead_of_silently_skipped(tmp_path):
    source = tmp_path / "live.log"
    evidence = tmp_path / "evidence.log"
    source.write_text("old lines\n")
    cursor = observer.log_cursor(source)
    source.rename(tmp_path / "old.log")
    source.write_text("new line\n")
    observer.collect_log(cursor, evidence)
    assert cursor["discontinuities"] == 1
    assert evidence.read_text() == "new line\n"


def test_status_commands_are_read_only(monkeypatch):
    calls = []
    def fake_command(cmd):
        calls.append(cmd)
        return {"returncode": 0, "stdout": "42\n" if cmd[0] == "lsof" else ""}
    monkeypatch.setattr(observer, "command", fake_command)
    monkeypatch.setattr(observer, "readyz", lambda port: {"status": 200})
    monkeypatch.setattr(observer, "get_health", lambda url: {"status": 200})
    observer.status(8090, ["com.astrid.spectral-bridge"])
    assert {cmd[0] for cmd in calls} == {"launchctl", "lsof", "ps", "memory_pressure", "vm_stat", "sysctl"}
    assert next(cmd for cmd in calls if cmd[0] == "launchctl")[1] == "print"
    assert ["memory_pressure", "-Q"] in calls


def test_generating_ready_status_still_checks_reservoir_and_generation_error():
    sample = {"readyz": {"status": 200, "json": {"ready": True,
        "worker": {"phase": "generating", "last_generation_error": "metal failure"},
        "reservoir": {"status": "degraded"}}},
        "ollama": {"pids": [1], "version": {"json": {"version": "0.25.2"}}}}
    assert observer.health_failures(sample) == ["generation_error", "reservoir_disconnected"]


def observation():
    files = [{"path": "/frozen/source.py", "sha256": "a" * 64, "size": 10, "mtime_ns": 1}]
    return {"state": "completed", "identity_start": files, "identity_end": files,
            "samples": [healthy_sample()]}


def test_old_record_can_be_reviewed_without_host_inspection(monkeypatch):
    monkeypatch.setattr(observer, "command", lambda *_: pytest.fail("review must not inspect current host"))
    result = observer.review_observation(observation())
    assert result["passed"]
    assert result["host_reinspection_performed"] is False


@pytest.mark.parametrize("missing", ["provider", "ollama", "files", "observed_service"])
def test_missing_identity_evidence_never_counts_as_stable(missing):
    record = observation()
    if missing == "files":
        record["identity_start"] = record["identity_end"] = []
    elif missing == "observed_service":
        record["samples"][0]["services"]["another.observed.service"] = {"returncode": 3}
    else:
        record["samples"][0][missing]["identity"] = {"returncode": 1, "stdout": ""}
    assert not observer.review_observation(record)["passed"]
