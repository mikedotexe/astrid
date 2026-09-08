#!/usr/bin/env python3
"""Replay both actual transports against an isolated HTTP fixture.

Requires the lib test executable built by cargo test --lib --no-run. Never
contacts a model; workers receive explicit loopback routes and private roots.
All inputs, outputs, requests, hashes and timings remain in the output folder.
"""
from __future__ import annotations

import argparse
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import statistics
import subprocess
import threading
import time


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--sandbox", type=Path, required=True)
    parser.add_argument("--baseline-binary", type=Path)
    args = parser.parse_args()
    args.binary = args.binary.resolve()
    root = args.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    prose = "I can stay with this passage and its careful account of returning to a chosen task. The words remain clear enough for me to follow their next step.\n\nNEXT: LISTEN"
    cases = [
        {"name": "plain", "raw": prose},
        {"name": "bare", "raw": "<end_of_turn>" + prose},
        {"name": "reference", "raw": "The token `<end_of_turn>` is mentioned here. " + prose},
        {"name": "annotation", "raw": "<end_of_turn> (channel note) token. " + prose},
        {"name": "mixed_utf8", "raw": "🙂 α <end_of_turn> " + prose + " The token `<start_header_id>` matters."},
        {"name": "only_marker", "raw": "<end_of_turn>"},
        {"name": "empty", "raw": ""},
        {"name": "degenerate", "raw": "123456!!!!////<end_of_turn>"},
        {"name": "dialogue_rejection", "raw": "Visible words without an action."},
        {"name": "parse_error", "wire": "{not valid json"},
        {"name": "missing_message", "missing": True},
        {"name": "http_error", "http_status": 503},
        {"name": "body_error", "truncated": True},
        {"name": "timeout", "delay": 1.3},
        {"name": "oversize", "raw": "<end_of_turn> " + prose + " words" * 45000},
        {"name": "receipt_cap", "raw": "<end_of_turn> " * 40 + prose},
        {"name": "unknown_model", "raw": prose, "model": None},
        {"name": "verified_release", "raw": "<end_of_turn>" + prose, "verified": True},
        {"name": "fallback_chain", "raw": prose, "first_parse_error": True, "only": "ollama"},
        {"name": "protected_incomplete", "raw": "<end_of_turn>" + prose, "protected": True, "first_incomplete": True, "only": "ollama"},
    ]
    (root / "fixtures.json").write_text(json.dumps(cases, indent=2, ensure_ascii=False) + "\n")
    manifest = root / "test-binary-manifest.json"
    manifest.write_text(json.dumps({
        "schema": "stack_build_manifest_v1",
        "authority": "build_manifest_witness_not_deploy_authority",
        "repository": {"available": True, "head": subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()},
        "artifacts": {"spectral-bridge": {"exists": True, "path": str(args.binary), "sha256": sha(args.binary.read_bytes())}},
    }))
    active: dict = {}

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *_args):
            pass

        def do_POST(self):
            job = active[self.path]
            content = self.rfile.read(int(self.headers["Content-Length"]))
            index = len(job["requests"])
            job["requests"].append(content)
            case = job["case"]
            if case.get("delay"):
                time.sleep(case["delay"])
            status = case.get("http_status", 200)
            model = case.get("model", "actual-served-fixture")
            payload = {"model": model}
            if not case.get("missing"):
                message = {"role": "assistant", "content": case.get("raw", prose)}
                if job["provider"] == "mlx":
                    payload["choices"] = [{"message": message}]
                else:
                    payload["message"] = message
                    payload["done"] = not (case.get("first_incomplete") and index == 0)
            elif job["provider"] == "mlx":
                payload["choices"] = []
            wire = case.get("wire", json.dumps(payload, ensure_ascii=False)).encode()
            if case.get("first_parse_error") and index == 0:
                wire = b"{invalid first fallback"
            self.send_response(status)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(wire) + (100 if case.get("truncated") else 0)))
            self.end_headers()
            try:
                self.wfile.write(wire)
            except (BrokenPipeError, ConnectionResetError):
                pass

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    results = []
    latencies: dict[str, list[float]] = {"disabled": [], "enabled": [], "fault": []}
    for case in cases:
        for provider in ("mlx", "ollama"):
            if case.get("only", provider) != provider:
                continue
            arms = {}
            for mode in (["baseline"] if args.baseline_binary else []) + ["disabled", "enabled", "fault"]:
                directory = root / f"{provider}-{case['name']}-{mode}"
                directory.mkdir()
                url_path = "/" + directory.name
                active[url_path] = {"case": case, "provider": provider, "requests": []}
                job = {"provider": provider, "observer": mode, "protected": case.get("protected", False), "output": str(directory / "result.json")}
                if case.get("verified") and mode != "baseline":
                    job["manifest"] = str(manifest)
                case_path = directory / "case.json"
                case_path.write_text(json.dumps(job))
                route = f"http://127.0.0.1:{server.server_port}{url_path}"
                env = dict(os.environ, ASTRID_OBSERVATION_TEST_CASE=str(case_path),
                           ASTRID_BRIDGE_MLX_URL=route, ASTRID_BRIDGE_OLLAMA_URL=route,
                           ASTRID_BRIDGE_WORKSPACE=str(directory / "workspace"),
                           MINIME_WORKSPACE=str(directory / "minime-workspace"),
                           ASTRID_BRIDGE_MLX_PROFILE="gemma4_12b",
                           ASTRID_OLLAMA_FALLBACK_MODEL="configured-fixture")
                binary = args.baseline_binary if mode == "baseline" else args.binary
                command = ["sandbox-exec", "-f", str(args.sandbox), str(binary),
                           "llm::provider::provider_observation_tests::transport_worker", "--exact", "--nocapture"]
                with (directory / "worker.log").open("w") as log:
                    subprocess.run(command, env=env, stdout=log, stderr=subprocess.STDOUT, timeout=90, check=True)
                result = json.loads((directory / "result.json").read_text())
                requests = active[url_path]["requests"]
                (directory / "requests.json").write_text(json.dumps([json.loads(r) for r in requests], indent=2))
                request_hashes = [sha(r) for r in requests]
                if mode == "enabled":
                    envelopes = [json.loads(p.read_text()) for p in (directory / "evidence/events").glob("*-outcome.json")]
                    assert len(envelopes) == len(requests), (directory, len(envelopes), len(requests))
                    links = result["observation"]["attempts"]
                    assert len(links) == len(requests)
                    assert {r["attempt_id"] for r in envelopes} == {r["attempt_id"] for r in links}
                    assert len({r["attempt_id"] for r in links}) == len(links)
                    for record in envelopes:
                        assert record["request_sha256"] in request_hashes
                        assert record["generation_id"] == "fixture-generation"
                        assert record["logical_attempt_index"] == 1
                        assert record["release_before"] == record["release_after"]
                        expected_binding = "startup_verified_process_binding" if case.get("verified") else "activation_unknown"
                        assert record["release_before"]["status"] == expected_binding
                        if case.get("verified"):
                            assert record["release_before"]["manifest_sha256"] == sha(manifest.read_bytes())
                        if "raw_response_sha256" in record:
                            assert record["raw_response_sha256"] == sha(case.get("raw", prose).encode())
                        if "raw_artifact" in record:
                            raw_path = directory / "evidence" / record["raw_artifact"]
                            assert sha(raw_path.read_bytes()) == record["raw_response_sha256"]
                        assert record["reported_model"] == case.get("model", "actual-served-fixture") or record["outcome"] in {"parse_error", "http_error", "body_read_error", "timeout"}
                    result["outcomes"] = [r["outcome"] for r in envelopes]
                    assert all(link["recording_status"] == "recorded" for link in links)
                elif mode == "fault":
                    assert all(link["recording_status"] == "recording_failed" for link in result["observation"]["attempts"])
                    assert result["observation"]["decision_recording_status"] == "recording_failed"
                elif mode == "disabled":
                    assert not (directory / "evidence").exists()
                    assert result["observation"]["status"] == "disabled"
                result["request_count"] = len(requests)
                arms[mode] = result
                if mode in latencies and not case.get("delay") and not case.get("verified"):
                    latencies[mode].append(result["elapsed_ms"])
            keys = ("provider_text", "accepted", "model", "request_count")
            assert all({k: arm[k] for k in keys} == {k: arms["disabled"][k] for k in keys} for arm in arms.values()), (provider, case["name"], arms)
            results.append({"provider": provider, "case": case["name"], "parity": True,
                            "requests": arms["enabled"]["request_count"],
                            "outcomes": arms["enabled"]["outcomes"]})
            print(json.dumps(results[-1]), flush=True)
    server.shutdown()
    report = {"schema": "provider_observation_qualification_v1", "cases": results,
              "functional_parity": True, "compared_with_exact_baseline": bool(args.baseline_binary),
              "binary_sha256": sha(args.binary.read_bytes()),
              "timing_scope": "fixture-worker end-to-end request, parsing, gates and synchronous recording; excludes process startup; one sample per case/arm; not a production benchmark",
              "timing_ms": {k: {"n": len(v), "median": statistics.median(v), "max": max(v)} for k, v in latencies.items()},
              "retained_evidence_bytes": sum(p.stat().st_size for p in root.glob("*/evidence/**/*") if p.is_file()),
              "live_activation": False}
    (root / "summary.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
