#!/usr/bin/env python3
"""Capture/reset private MLX trials without changing the production server.

Capture once, then pass that directory as --frozen-fixture-dir to each canary.
Each candidate launch resets both reservoir state and adaptive coupling journal.
"""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import importlib.metadata
import inspect
import json
import os
from pathlib import Path
import re
import sys
import urllib.parse

from astrid_mlx_fixture import FIXTURE_URL


CANDIDATE_PORT = 8092
SOURCE_NAMES = ("coupled_astrid_server.py", "coupled_http_gateway.py",
                "mlx_reservoir.py", "generation_controls.py")
ARTIFACT_RE = re.compile(
    r"(?:<start_of_turn>|<end_of_turn>|<think>|</think>|/no_think|"
    r"<\|im_start\|>|<\|im_end\|>|<\|eot_id\|>|<\|endoftext\|>|"
    r"<turn\|>|<\|turn>|<channel\|>|<\|channel>|<eos>|<bos>|<pad>|<unk>|</s>|"
    r"<\|think\|>|<\|tool_call>|<\|tool>|<\|tool_response>|<\|image>|<\|audio>|<\|video\|>|"
    r"\b(?:thought|analysis|final)\s*<channel\|>)",
    re.I,
)


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def private_write(path: Path, data: bytes, mode=0o600):
    with path.open("xb") as handle:
        os.chmod(path, mode)
        handle.write(data)


def validate_candidate_url(url: str):
    parsed = urllib.parse.urlsplit(url)
    if (parsed.scheme != "http" or parsed.hostname != "127.0.0.1"
            or parsed.port != CANDIDATE_PORT or parsed.username is not None
            or parsed.password is not None or parsed.query or parsed.fragment
            or parsed.path != "/v1/chat/completions"):
        raise ValueError("candidate must be http://127.0.0.1:8092/v1/chat/completions")


def capture_fixture(destination: Path, snapshot: Path, journal: Path):
    """Copy existing on-disk evidence; never request a live snapshot or modify it."""
    snapshot_bytes = snapshot.read_bytes()
    journal_bytes = journal.read_bytes()
    parsed = json.loads(journal_bytes)
    if not isinstance(parsed, dict) or not isinstance(parsed.get("entries"), list):
        raise ValueError("coupling journal requires an entries list")
    # Reject a moving file instead of capturing partial bytes during an update.
    if snapshot.read_bytes() != snapshot_bytes or journal.read_bytes() != journal_bytes:
        raise ValueError("fixture inputs changed during capture; retry after a calm interval")
    destination.mkdir(parents=True, mode=0o700, exist_ok=False)
    private_write(destination / "astrid.npz", snapshot_bytes, 0o400)
    private_write(destination / "coupling_journal.json", journal_bytes, 0o400)
    manifest = {"schema": "astrid_mlx_frozen_fixture_v1", "live_state_modified": False,
                "snapshot": {"source": str(snapshot.resolve()), "sha256": sha256(snapshot_bytes)},
                "journal": {"source": str(journal.resolve()), "sha256": sha256(journal_bytes)}}
    private_write(destination / "fixture.json", (json.dumps(manifest, indent=2) + "\n").encode(), 0o400)
    return manifest


def fixture_inputs(directory: Path):
    manifest = json.loads((directory / "fixture.json").read_text())
    if manifest.get("schema") != "astrid_mlx_frozen_fixture_v1":
        raise ValueError("unsupported fixture manifest")
    result = {}
    for key, filename in (("snapshot", "astrid.npz"), ("journal", "coupling_journal.json")):
        data = (directory / filename).read_bytes()
        if sha256(data) != manifest[key]["sha256"]:
            raise ValueError(f"frozen {key} hash mismatch")
        result[filename] = data
    return manifest, result


def prepare_trial(output_dir: Path, reservoir_root: Path, frozen_fixture_dir: Path | None):
    output_dir = output_dir.resolve()
    root = reservoir_root.resolve()
    if output_dir == root or output_dir.is_relative_to(root):
        raise ValueError("candidate output must be outside the production reservoir tree")
    output_dir.mkdir(parents=True, mode=0o700, exist_ok=True)
    fixture = frozen_fixture_dir
    if fixture is None:
        fixture = output_dir / "frozen-inputs"
        capture_fixture(fixture, root / "state/astrid.npz", root / "state/coupling_journal.json")
    manifest, inputs = fixture_inputs(fixture)
    trial = output_dir / "candidate-runtime"
    trial.mkdir(mode=0o700, exist_ok=False)
    (trial / "state").mkdir(mode=0o700)
    private_write(trial / "astrid.npz", inputs["astrid.npz"], 0o400)
    private_write(trial / "state/coupling_journal.json", inputs["coupling_journal.json"])
    source_hashes = {name: sha256((root / name).read_bytes()) for name in SOURCE_NAMES}
    record = {"schema": "astrid_mlx_candidate_isolation_v1", "fixture": manifest,
              "source_root": str(root), "source_sha256": source_hashes,
              "cwd": str(trial), "reservoir_url": FIXTURE_URL,
              "candidate_url": f"http://127.0.0.1:{CANDIDATE_PORT}/v1/chat/completions",
              "reset_scope": "fresh_process_reservoir_and_coupling_journal",
              "live_reservoir_connected": False}
    private_write(trial / "isolation.json", (json.dumps(record, indent=2) + "\n").encode(), 0o400)
    (output_dir / "request_metrics").mkdir(mode=0o700, exist_ok=False)
    return trial


def server_arguments(root, trial, model, coupling_strength, wide_coupling_strength):
    return [str(root / "coupled_astrid_server.py"), "--host", "127.0.0.1",
            "--port", str(CANDIDATE_PORT), "--model", model,
            "--coupling-strength", str(coupling_strength), "--wide-coupling-strength", str(wide_coupling_strength),
            "--model-qos-mode", "active", "--generation-queue-capacity", "4",
            "--readiness-stale-seconds", "5",
            "--model-qos-receipts", str(trial / "model_qos_receipts.jsonl"),
            "--audit-dir", str(trial.parent / "request_metrics"), "--model-memory-map"]


def package_inventory():
    packages = {}
    for distribution in importlib.metadata.distributions():
        name = distribution.metadata.get("Name")
        if name:
            packages[name.lower().replace("_", "-")] = {
                "version": distribution.version,
                "metadata_sha256": sha256((distribution.read_text("METADATA") or "").encode()),
                "record_sha256": sha256((distribution.read_text("RECORD") or "").encode()),
            }
    return {"schema": "astrid_mlx_candidate_packages_v1", "python": sys.executable,
            "fingerprint_scope": "installed_distribution_versions_metadata_and_records_not_wheel_bytes",
            "packages": dict(sorted(packages.items()))}


def instrument_candidate(server, package_fingerprint):
    """Measure only the private process, preserving the production source bytes."""
    original_generate = server.CoupledAstridServer.generate_coupled
    original_write = server.CoupledAstridServer._write_request_audit
    original_pull = server.CoupledAstridServer._pull_state
    signature = inspect.signature(original_generate)
    pull_signature = inspect.signature(original_pull)

    def checked_pull(self, *args, **kwargs):
        bound = pull_signature.bind(self, *args, **kwargs)
        bound.apply_defaults()
        result = original_pull(self, *args, **kwargs)
        audit = bound.arguments.get("audit")
        if audit is not None:
            # The outer bootstrap retry returns the final checkout result.
            # A later successful push must not hide a failed initial checkout.
            reservoir = dict(audit.get("reservoir", {}) or {})
            reservoir["checkout_succeeded"] = result is True
            audit["reservoir"] = reservoir
        return result

    def measured(self, *args, **kwargs):
        bound = signature.bind(self, *args, **kwargs)
        bound.apply_defaults()
        self._qualification_prompt_sha256 = sha256(json.dumps(
            bound.arguments["messages"], sort_keys=True, separators=(",", ":")).encode())
        server.mx.reset_peak_memory()
        return original_generate(self, *args, **kwargs)

    def write_audit(self, audit):
        generation = audit["generation"]
        raw_text = self.tokenizer.detokenizer.text
        if not isinstance(raw_text, str):
            raise ValueError("candidate raw detokenizer text is unavailable")
        generation["raw_model_artifact_detected"] = ARTIFACT_RE.search(raw_text) is not None
        generation["raw_artifact_scan_scope"] = "detokenizer_text_before_cleanup_excludes_filtered_tokens"
        generation["peak_memory_bytes"] = int(server.mx.get_peak_memory())
        generation["peak_memory_scope"] = "mlx_active_allocations_peak_since_request_reset_excludes_allocator_cache_and_process_rss"
        audit["request"]["messages_sha256"] = self._qualification_prompt_sha256
        audit["runtime"]["candidate_packages_sha256"] = package_fingerprint
        original_write(self, audit)

    server.CoupledAstridServer.generate_coupled = measured
    server.CoupledAstridServer._write_request_audit = write_audit
    server.CoupledAstridServer._pull_state = checked_pull


def run_trial(root: Path, trial: Path, model: str, coupling_strength: float, wide_coupling_strength: float):
    from astrid_mlx_fixture import start_fixture

    root, trial = root.resolve(), trial.resolve()
    record = json.loads((trial / "isolation.json").read_text())
    if record["source_root"] != str(root) or record["cwd"] != str(trial):
        raise ValueError("candidate identity differs from prepared trial")
    for name, expected in record["source_sha256"].items():
        if sha256((root / name).read_bytes()) != expected:
            raise ValueError(f"candidate source changed: {name}")
    if sha256((trial / "astrid.npz").read_bytes()) != record["fixture"]["snapshot"]["sha256"]:
        raise ValueError("candidate snapshot changed")
    if sha256((trial / "state/coupling_journal.json").read_bytes()) != record["fixture"]["journal"]["sha256"]:
        raise ValueError("candidate journal changed; prepare a fresh trial to reset it")
    os.chdir(trial)
    os.umask(0o077)
    sys.dont_write_bytecode = True
    fixture, thread = start_fixture(trial / "astrid.npz")
    try:
        sys.path.insert(0, str(root))
        spec = importlib.util.spec_from_file_location("coupled_astrid_server", root / SOURCE_NAMES[0])
        if spec is None or spec.loader is None:
            raise ValueError("cannot import coupled server")
        server = importlib.util.module_from_spec(spec)
        sys.modules[spec.name] = server
        spec.loader.exec_module(server)
        server.RESERVOIR_WS_URL = FIXTURE_URL
        packages = (json.dumps(package_inventory(), indent=2, sort_keys=True) + "\n").encode()
        private_write(trial / "packages.json", packages, 0o400)
        instrument_candidate(server, sha256(packages))
        sys.argv = server_arguments(root, trial, model, coupling_strength, wide_coupling_strength)
        return server.main()
    finally:
        fixture.shutdown()
        thread.join(timeout=5)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    capture = commands.add_parser("capture")
    capture.add_argument("--output-dir", type=Path, required=True)
    capture.add_argument("--snapshot", type=Path, required=True)
    capture.add_argument("--journal", type=Path, required=True)
    run = commands.add_parser("run")
    run.add_argument("--reservoir-root", type=Path, required=True)
    run.add_argument("--trial-dir", type=Path, required=True)
    run.add_argument("--model", required=True)
    run.add_argument("--coupling-strength", type=float, default=0.1)
    run.add_argument("--wide-coupling-strength", type=float, default=0.0)
    args = parser.parse_args()
    if args.command == "capture":
        print(json.dumps(capture_fixture(args.output_dir, args.snapshot, args.journal), indent=2))
        return 0
    return run_trial(args.reservoir_root, args.trial_dir, args.model, args.coupling_strength, args.wide_coupling_strength)


if __name__ == "__main__":
    raise SystemExit(main())
