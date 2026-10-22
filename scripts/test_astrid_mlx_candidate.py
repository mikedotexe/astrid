"""Isolated intake tests: fake model only, no network listeners or GPU work."""
from __future__ import annotations

import base64
import json
import os
from pathlib import Path
import sys
from types import SimpleNamespace
from unittest.mock import patch

import numpy as np
import pytest

sys.path.insert(0, str(Path(__file__).parent))
import astrid_mlx_candidate as candidate
import astrid_mlx_fixture as fixture
import astrid_model_canary as canary


def inputs(tmp_path):
    root = tmp_path / "production"
    state = root / "state"
    state.mkdir(parents=True)
    np.savez(state / "astrid.npz", entity="astrid", tick_count=12,
             **{key: np.full((1, 192), .1, dtype=np.float32) for key in ("h1", "h2", "h3")})
    journal = b'{"version":1,"entries":[{"coupling_strength":0.12,"tick_count":12}]}\n'
    (state / "coupling_journal.json").write_bytes(journal)
    for name in candidate.SOURCE_NAMES:
        (root / name).write_text("# frozen source\n")
    return root


def frozen(tmp_path, root):
    destination = tmp_path / "frozen"
    candidate.capture_fixture(destination, root / "state/astrid.npz", root / "state/coupling_journal.json")
    return destination


def test_capture_and_trial_reset_keep_production_and_frozen_inputs_unchanged(tmp_path):
    root = inputs(tmp_path)
    source_journal = (root / "state/coupling_journal.json").read_bytes()
    frozen_dir = frozen(tmp_path, root)
    one = candidate.prepare_trial(tmp_path / "trial-one", root, frozen_dir)
    (one / "state/coupling_journal.json").write_text('{"entries":[]}')
    two = candidate.prepare_trial(tmp_path / "trial-two", root, frozen_dir)
    assert (two / "state/coupling_journal.json").read_bytes() == source_journal
    assert (root / "state/coupling_journal.json").read_bytes() == source_journal
    assert (frozen_dir / "coupling_journal.json").read_bytes() == source_journal
    assert (frozen_dir / "astrid.npz").stat().st_mode & 0o777 == 0o400
    assert two.stat().st_mode & 0o777 == 0o700
    with pytest.raises(FileExistsError):
        candidate.prepare_trial(tmp_path / "trial-two", root, frozen_dir)
    with pytest.raises(ValueError, match="outside"):
        candidate.prepare_trial(root / "candidate", root, frozen_dir)


def test_capture_hash_validation_and_no_overwrite(tmp_path):
    root = inputs(tmp_path)
    directory = frozen(tmp_path, root)
    with pytest.raises(FileExistsError):
        candidate.capture_fixture(directory, root / "state/astrid.npz", root / "state/coupling_journal.json")
    os.chmod(directory / "astrid.npz", 0o600)
    (directory / "astrid.npz").write_bytes(b"changed")
    with pytest.raises(ValueError, match="snapshot hash"):
        candidate.prepare_trial(tmp_path / "trial", root, directory)


@pytest.mark.parametrize("url", ["http://127.0.0.1:8090/v1/chat/completions",
    "http://localhost:8092/v1/chat/completions", "http://example.com:8092/v1/chat/completions",
    "http://127.0.0.1:8092/v1/chat/completions?redirect=7881", "https://127.0.0.1:8092/v1/chat/completions",
    "http://user@127.0.0.1:8092/v1/chat/completions"])
def test_candidate_endpoint_rejects_production_and_remote_urls(url):
    with pytest.raises(ValueError):
        candidate.validate_candidate_url(url)


def test_fixture_copies_each_handle_and_fresh_trial_resets_state(tmp_path):
    root = inputs(tmp_path)
    snapshot = root / "state/astrid.npz"
    server = fixture.FrozenReservoir(snapshot)
    initial = server.dispatch({"type": "pull_state", "name": "astrid"})
    server.dispatch({"type": "create_handle", "name": "probe-a"})
    changed = base64.b64encode(np.ones(192, dtype=np.float32).tobytes()).decode()
    assert server.dispatch({"type": "push_state", "name": "probe-a", "tick_delta": 5,
                            "h1": changed, "h2": changed, "h3": changed})["ok"]
    assert server.dispatch({"type": "pull_state", "name": "probe-a"})["tick_count"] == 17
    assert server.dispatch({"type": "pull_state", "name": "astrid"}) == initial
    server.dispatch({"type": "create_handle", "name": "probe-b"})
    assert server.dispatch({"type": "pull_state", "name": "probe-b"})["h1"] == initial["h1"]
    assert fixture.FrozenReservoir(snapshot).dispatch({"type": "pull_state", "name": "astrid"}) == initial
    with pytest.raises(ValueError, match="unsupported"):
        server.dispatch({"type": "tick", "name": "astrid"})
    with pytest.raises(ValueError):
        server.dispatch({"type": "push_state", "name": "astrid", "h1": "!!!", "h2": changed, "h3": changed})
    assert server.dispatch({"type": "pull_state", "name": "astrid"}) == initial


def test_start_builds_private_launcher_and_preserves_legacy_tuple_api(tmp_path):
    root = inputs(tmp_path)
    directory = frozen(tmp_path, root)
    process = SimpleNamespace(pid=123)
    with patch.object(canary, "port_pids", return_value=[]), patch.object(canary.subprocess, "Popen", return_value=process) as popen:
        proc, log = canary.start_candidate_server(reservoir_root=root, python="/candidate/python",
            model="/pinned/model", port=8092, coupling_strength=.1, output_dir=tmp_path / "output",
            frozen_fixture_dir=directory)
    assert proc is process
    command = popen.call_args.args[0]
    assert command[:2] == ["/candidate/python", "-B"]
    assert command[2].endswith("astrid_mlx_candidate.py")
    assert popen.call_args.kwargs["cwd"] == str(tmp_path / "output/candidate-runtime")
    assert str(root) != popen.call_args.kwargs["cwd"]
    assert command[command.index("--wide-coupling-strength") + 1] == "0.0"
    assert log.parent == tmp_path / "output"
    args = candidate.server_arguments(root, log.parent / "candidate-runtime", "/pinned/model", .1, 0.)
    for flag, value in (("--host", "127.0.0.1"), ("--port", "8092"), ("--model-qos-mode", "active"),
                        ("--generation-queue-capacity", "4"), ("--coupling-strength", "0.1")):
        assert args[args.index(flag) + 1] == value
    assert args[args.index("--model-qos-receipts") + 1].startswith(str(log.parent))


def test_live_port_and_occupied_port_refuse_before_capture(tmp_path):
    root = inputs(tmp_path)
    params = dict(reservoir_root=root, python="python", model="model", coupling_strength=.1, output_dir=tmp_path / "output")
    with pytest.raises(ValueError, match="8092"):
        canary.start_candidate_server(**params, port=8090)
    with patch.object(canary, "port_pids", return_value=["123"]), pytest.raises(RuntimeError, match="listener"):
        canary.start_candidate_server(**params, port=8092)
    assert not (tmp_path / "output").exists()


def test_retired_bridge_fallback_cli_refuses_before_mutation(monkeypatch):
    monkeypatch.setattr(sys, "argv", ["canary", "--bridge-fallback-continuity-probe"])
    with patch.object(canary, "launchctl") as launchctl, pytest.raises(SystemExit) as error:
        canary.main()
    assert error.value.code == 2
    launchctl.assert_not_called()


def test_fixture_bind_is_private_and_occupied_port_fails_before_thread(tmp_path):
    root = inputs(tmp_path)
    with patch("websockets.sync.server.serve", side_effect=OSError("address in use")) as serve:
        with pytest.raises(OSError, match="address in use"):
            fixture.start_fixture(root / "state/astrid.npz")
    assert serve.call_args.args[1:] == ("127.0.0.1", 17881)


def test_runner_redirects_startup_before_main_and_rejects_source_drift(tmp_path, monkeypatch):
    root = inputs(tmp_path)
    source = '''
RESERVOIR_WS_URL = "ws://127.0.0.1:7881"
class CoupledAstridServer:
    def generate_coupled(self, messages): pass
    def _write_request_audit(self, audit): pass
    def _pull_state(self, handle_name="astrid", audit=None, _bootstrap_retry=True): return True
def main():
    assert RESERVOIR_WS_URL == "ws://127.0.0.1:17881"
    import sys, pathlib
    assert sys.argv[sys.argv.index("--port")+1] == "8092"
    assert pathlib.Path.cwd().name == "candidate-runtime"
    return 7
'''
    (root / "coupled_astrid_server.py").write_text(source)
    trial = candidate.prepare_trial(tmp_path / "output", root, frozen(tmp_path, root))
    stopped = []
    fake_fixture = SimpleNamespace(shutdown=lambda: stopped.append("shutdown"))
    fake_thread = SimpleNamespace(join=lambda timeout: stopped.append("joined"))
    with patch.object(fixture, "start_fixture", return_value=(fake_fixture, fake_thread)):
        old_argv, old_path = sys.argv, list(sys.path)
        monkeypatch.chdir(tmp_path)
        try:
            assert candidate.run_trial(root, trial, "model", .1, 0.) == 7
        finally:
            sys.argv, sys.path[:] = old_argv, old_path
    assert stopped == ["shutdown", "joined"]
    assert (trial / "packages.json").exists()
    (root / "generation_controls.py").write_text("# changed\n")
    with pytest.raises(ValueError, match="source changed"):
        candidate.run_trial(root, trial, "model", .1, 0.)


def test_instrumentation_preserves_result_and_adds_memory_and_package_identity():
    calls = []
    class Server:
        tokenizer = SimpleNamespace(detokenizer=SimpleNamespace(text="\nA valid answer.\n"))
        def _pull_state(self, handle_name="astrid", audit=None, _bootstrap_retry=True):
            return True
        def generate_coupled(self, messages, temperature=.8):
            self._write_request_audit({"request": {}, "generation": {"total_turn_s": 2}, "runtime": {}})
            return "unchanged result"
        def _write_request_audit(self, audit):
            calls.append(audit)
    module = SimpleNamespace(CoupledAstridServer=Server,
                             mx=SimpleNamespace(reset_peak_memory=lambda: calls.append("reset"), get_peak_memory=lambda: 1234))
    candidate.instrument_candidate(module, "packages-sha")
    assert Server().generate_coupled([{"role": "user", "content": "fixture"}]) == "unchanged result"
    assert calls[0] == "reset"
    assert calls[1]["generation"]["total_turn_s"] == 2
    assert calls[1]["generation"]["peak_memory_bytes"] == 1234
    assert calls[1]["generation"]["raw_model_artifact_detected"] is False
    assert "excludes_allocator_cache_and_process_rss" in calls[1]["generation"]["peak_memory_scope"]
    assert calls[1]["runtime"]["candidate_packages_sha256"] == "packages-sha"
    assert len(calls[1]["request"]["messages_sha256"]) == 64


@pytest.mark.parametrize("raw_text,expected", [
    ("\nA valid answer.\n", False),
    ("\tA valid answer.  ", False),
    ("<think>Hidden thought.</think>A valid answer.", True),
    ("analysis<channel|>Hidden thought.", True),
    ("<|tool_call>dangerous formatting", True),
    ("<|video|>unexpected media formatting", True),
    ("A valid answer.</s>", True),
])
def test_raw_artifact_scan_distinguishes_whitespace_from_control_text(raw_text, expected):
    receipts = []
    class Server:
        tokenizer = SimpleNamespace(detokenizer=SimpleNamespace(text=raw_text))
        def _pull_state(self, handle_name="astrid", audit=None, _bootstrap_retry=True):
            return True
        def generate_coupled(self, messages):
            self._write_request_audit({"request": {}, "generation": {}, "runtime": {}})
        def _write_request_audit(self, audit):
            receipts.append(audit)
    module = SimpleNamespace(CoupledAstridServer=Server,
                             mx=SimpleNamespace(reset_peak_memory=lambda: None, get_peak_memory=lambda: 1234))
    candidate.instrument_candidate(module, "packages-sha")
    Server().generate_coupled([])
    assert receipts[0]["generation"]["raw_model_artifact_detected"] is expected
    assert "before_cleanup" in receipts[0]["generation"]["raw_artifact_scan_scope"]
    assert canary.ARTIFACT_RE is candidate.ARTIFACT_RE


@pytest.mark.parametrize("outcome", ["success", "failure", "bootstrap"])
def test_checkout_instrumentation_preserves_final_result_and_bootstrap(outcome):
    calls = []
    class Server:
        def generate_coupled(self, messages): pass
        def _write_request_audit(self, audit): pass
        def _pull_state(self, handle_name="astrid", audit=None, _bootstrap_retry=True):
            calls.append((handle_name, audit, _bootstrap_retry))
            if outcome == "bootstrap" and _bootstrap_retry:
                # The original server creates a missing handle then recursively
                # retries through self._pull_state using the same request audit.
                return self._pull_state(handle_name, audit, _bootstrap_retry=False)
            if audit is not None:
                audit["reservoir"] = {"pull_bytes": 123}
            return outcome != "failure"
    candidate.instrument_candidate(SimpleNamespace(CoupledAstridServer=Server), "packages-sha")
    server = Server()
    audit = {"reservoir": {}}
    assert server._pull_state("qualification_exact", audit=audit) is (outcome != "failure")
    assert audit["reservoir"] == {"pull_bytes": 123, "checkout_succeeded": outcome != "failure"}
    assert len(calls) == (2 if outcome == "bootstrap" else 1)
    # Startup has no request audit; no extra audit or altered return semantics.
    assert server._pull_state() is (outcome != "failure")


def test_checkout_instrumentation_preserves_exceptions():
    sentinel = RuntimeError("fixture checkout exception")
    class Server:
        def generate_coupled(self, messages): pass
        def _write_request_audit(self, audit): pass
        def _pull_state(self, handle_name="astrid", audit=None, _bootstrap_retry=True):
            raise sentinel
    candidate.instrument_candidate(SimpleNamespace(CoupledAstridServer=Server), "packages-sha")
    with pytest.raises(RuntimeError) as raised:
        Server()._pull_state(audit={"reservoir": {}})
    assert raised.value is sentinel


def test_performance_rows_join_only_the_matching_probe(tmp_path):
    metrics = tmp_path / "request_metrics"
    metrics.mkdir()
    row = {"request": {"handle_name": "private-a", "messages_sha256": "abc", "max_tokens": 96, "prompt_chars": 40},
           "generation": {"prompt_tokens": 20, "completion_tokens": 18, "total_turn_s": 2, "first_token_s": .25, "decode_tok_s": 10, "peak_memory_bytes": 100}}
    (metrics / "coupled_request_metrics.jsonl").write_text(json.dumps(row)+"\n")
    samples = canary.performance_samples(tmp_path, [{"name": "next_contract", "handle_name": "private-a"}])
    assert samples[0]["prompt_sha256"] == "abc"
    assert samples[0]["scenario"] == "next_contract"
    assert samples[0]["peak_memory_bytes"] == 100
    assert samples[0]["first_token_s"] == .25
    assert samples[0]["completion_tokens"] == 18
    assert not canary.performance_samples(tmp_path, [{"name": "other", "handle_name": "other"}])


@pytest.mark.parametrize("listeners", [(["foreign"], ["42"]), (["42"], ["foreign"]), (["42", "foreign"], ["42"])])
def test_readiness_refuses_foreign_model_or_fixture_owner_before_http(listeners):
    proc = SimpleNamespace(pid=42, poll=lambda: None)
    with patch.object(canary, "port_pids", side_effect=listeners), patch.object(canary, "http_json") as http:
        result = canary.wait_for_models("http://127.0.0.1:8092/v1/chat/completions", 5, proc)
    assert not result["ok"]
    assert "unexpected PID" in result["error"]
    http.assert_not_called()


def test_readiness_waits_for_both_owned_listeners_and_rechecks_after_response():
    proc = SimpleNamespace(pid=42, poll=lambda: None)
    with patch.object(canary, "port_pids", side_effect=[[], ["42"], ["42"], ["42"], ["42"], ["42"]]), \
         patch.object(canary.time, "sleep"), patch.object(canary, "http_json", return_value={"ok": True}) as http:
        assert canary.wait_for_models("http://127.0.0.1:8092/v1/chat/completions", 5, proc)["ok"]
    http.assert_called_once()
    with patch.object(canary, "port_pids", side_effect=[["42"], ["42"], ["43"]]), \
         patch.object(canary, "http_json", return_value={"ok": True}):
        result = canary.wait_for_models("http://127.0.0.1:8092/v1/chat/completions", 5, proc)
    assert not result["ok"]
    assert "unexpected PID" in result["error"]
