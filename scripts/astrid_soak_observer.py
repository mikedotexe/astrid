"""Passive soak recorder. This module has no service or inference control path.

Only evidence files beneath the requested output directory are written. Process
inspection, log reads and GET /readyz do not start, route, or stop services.
"""

from __future__ import annotations

import hashlib
import argparse
import json
import os
import re
import signal
import subprocess
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path
from typing import Any, Callable


def command(args: list[str]) -> dict[str, Any]:
    try:
        result = subprocess.run(args, capture_output=True, text=True, timeout=8, check=False)
        return {"returncode": result.returncode, "stdout": result.stdout, "stderr": result.stderr}
    except (OSError, subprocess.TimeoutExpired) as exc:
        return {"returncode": None, "error": str(exc)}


def file_identity(path: Path) -> dict[str, Any]:
    try:
        stat = path.stat()
        digest = hashlib.sha256()
        with path.open("rb") as handle:
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                digest.update(chunk)
        return {"path": str(path), "sha256": digest.hexdigest(), "size": stat.st_size,
                "mtime_ns": stat.st_mtime_ns}
    except OSError as exc:
        return {"path": str(path), "error": str(exc)}


def get_health(url: str) -> dict[str, Any]:
    started = time.monotonic()
    try:
        with urllib.request.urlopen(url, timeout=5) as response:
            return {"url": url, "status": response.status, "json": json.load(response),
                    "elapsed_s": time.monotonic() - started}
    except (OSError, ValueError, urllib.error.URLError) as exc:
        return {"url": url, "error": str(exc), "elapsed_s": time.monotonic() - started}


def readyz(port: int) -> dict[str, Any]:
    return get_health(f"http://127.0.0.1:{port}/readyz")


def process_identity(pids: list[int]) -> dict[str, Any]:
    return command(["ps", "-p", ",".join(map(str, pids)), "-o",
                    "pid=,lstart=,command="]) if pids else {}


def listener(port: int) -> dict[str, Any]:
    listeners = command(["lsof", "-nP", f"-iTCP:{port}", "-sTCP:LISTEN", "-t"])
    pids = sorted({int(value) for value in listeners.get("stdout", "").split() if value.isdigit()})
    processes = command(["ps", "-p", ",".join(map(str, pids)), "-o",
                         "pid=,ppid=,lstart=,etime=,rss=,command="]) if pids else {}
    return {"pids": pids, "inspection": listeners, "processes": processes,
            "identity": process_identity(pids)}


def status(port: int, labels: list[str]) -> dict[str, Any]:
    services = {}
    for label in labels:
        result = command(["launchctl", "print", f"gui/{os.getuid()}/{label}"])
        pid = re.search(r"\bpid = (\d+)", result.get("stdout", ""))
        result["identity"] = process_identity([int(pid.group(1))]) if pid else {}
        services[label] = result
    provider = listener(port)
    ollama = listener(11434)
    ollama["version"] = get_health("http://127.0.0.1:11434/api/version")
    return {"t_s": time.time(), "services": services, "candidate_pids": provider["pids"],
            "provider": provider, "ollama": ollama,
            "readyz": readyz(port), "memory_pressure": command(["memory_pressure", "-Q"]),
            "vm_stat": command(["vm_stat"]),
            "swap": command(["sysctl", "vm.swapusage"])}


def health_failures(sample: dict[str, Any]) -> list[str]:
    health = sample.get("readyz", {})
    payload = health.get("json", {})
    worker = payload.get("worker", {})
    reasons = []
    if health.get("status") != 200 or payload.get("ready") is not True:
        reasons.append("provider_not_ready")
    if worker.get("phase") not in {"ready", "generating"}:
        reasons.append("worker_unhealthy")
    if worker.get("last_generation_error") is not None or payload.get("error"):
        reasons.append("generation_error")
    if payload.get("reservoir", {}).get("status") != "connected":
        reasons.append("reservoir_disconnected")
    ollama = sample.get("ollama", {})
    if not ollama.get("pids") or not ollama.get("version", {}).get("json", {}).get("version"):
        reasons.append("ollama_unavailable")
    return reasons


def stable_identity(sample: dict[str, Any]) -> str:
    """Exclude changing RSS/elapsed/queue/launchd run counters from identity."""
    identities = {label: value.get("identity", {}).get("stdout")
                  for label, value in sample.get("services", {}).items()}
    identities["provider"] = sample.get("provider", {}).get("identity", {}).get("stdout")
    identities["ollama"] = sample.get("ollama", {}).get("identity", {}).get("stdout")
    identities["ollama_version"] = sample.get("ollama", {}).get("version", {}).get("json", {}).get("version")
    return json.dumps(identities, sort_keys=True)


def valid_process_identity(value: dict[str, Any], expected_pids: list[int]) -> bool:
    if value.get("returncode") != 0 or not expected_pids:
        return False
    lines = value.get("stdout", "").splitlines()
    matches = [re.fullmatch(
        r"\s*(\d+)\s+\w{3}\s+\w{3}\s+\d{1,2}\s+\d{2}:\d{2}:\d{2}\s+\d{4}\s+\S.*", line
    ) for line in lines]
    return (bool(matches) and all(matches)
            and {int(match.group(1)) for match in matches if match} == set(expected_pids))


def review_observation(record: dict[str, Any]) -> dict[str, Any]:
    """Apply current provenance guards to retained samples without re-recording.

    This is also usable for a window recorded by an earlier observer version;
    missing original evidence cannot be repaired by inspecting the host later.
    """
    failures: list[dict[str, Any]] = []
    before, after = record.get("identity_start"), record.get("identity_end")
    for phase, values in (("start", before), ("end", after)):
        if not isinstance(values, list) or not values:
            failures.append({"kind": "source_identity_missing", "phase": phase})
            continue
        for item in values:
            if (not isinstance(item, dict) or item.get("error") or not item.get("path")
                    or not re.fullmatch(r"[0-9a-f]{64}", str(item.get("sha256", "")))):
                failures.append({"kind": "source_identity_incomplete", "phase": phase})
    if before != after:
        failures.append({"kind": "source_identity_changed"})
    samples = record.get("samples")
    if not isinstance(samples, list) or not samples:
        failures.append({"kind": "samples_missing"})
        samples = []
    for index, sample in enumerate(samples):
        for name in ("provider", "ollama"):
            service = sample.get(name, {})
            if not valid_process_identity(service.get("identity", {}), service.get("pids", [])):
                failures.append({"kind": "process_identity_incomplete", "sample": index, "service": name})
        services = sample.get("services", {})
        if "com.astrid.spectral-bridge" not in services:
            failures.append({"kind": "bridge_identity_missing", "sample": index})
        for label, service in services.items():
            pid = re.search(r"\bpid = (\d+)", service.get("stdout", ""))
            if service.get("returncode") != 0 or "state = running" not in service.get("stdout", ""):
                failures.append({"kind": "observed_service_not_running", "sample": index, "service": label})
            if not valid_process_identity(service.get("identity", {}), [int(pid.group(1))] if pid else []):
                failures.append({"kind": "process_identity_incomplete", "sample": index, "service": label})
    identities = {stable_identity(sample) for sample in samples}
    if len(identities) > 1:
        failures.append({"kind": "process_identity_changed"})
    if record.get("state") != "completed":
        failures.append({"kind": "observation_incomplete"})
    return {"schema": "astrid_observation_provenance_review_v1", "passed": not failures,
            "sample_count": len(samples), "identity_file_count": len(before) if isinstance(before, list) else 0,
            "failures": failures, "reviewer_sha256": file_identity(Path(__file__)).get("sha256"),
            "host_reinspection_performed": False}


def log_cursor(path: Path) -> dict[str, Any]:
    try:
        stat = path.stat()
        return {"path": str(path), "inode": stat.st_ino, "offset": stat.st_size,
                "initial_offset": stat.st_size, "initial_inode": stat.st_ino,
                "bytes_collected": 0, "discontinuities": 0, "read_errors": 0}
    except OSError:
        return {"path": str(path), "inode": None, "offset": 0, "initial_offset": 0,
                "initial_inode": None, "bytes_collected": 0,
                "discontinuities": 0, "read_errors": 0}


def collect_log(cursor: dict[str, Any], destination: Path) -> None:
    """Retain appended bytes, recording rotation/truncation as incomplete evidence."""
    try:
        path = Path(cursor["path"])
        with path.open("rb") as source:
            stat = os.fstat(source.fileno())
            if cursor["inode"] is not None and (
                stat.st_ino != cursor["inode"] or stat.st_size < cursor["offset"]
            ):
                cursor["discontinuities"] += 1
                cursor["offset"] = 0
            cursor["inode"] = stat.st_ino
            source.seek(cursor["offset"])
            with destination.open("ab") as target:
                while chunk := source.read(1024 * 1024):
                    target.write(chunk)
                    cursor["bytes_collected"] += len(chunk)
                cursor["offset"] = source.tell()
    except OSError as exc:
        cursor["read_errors"] += 1
        cursor["last_error"] = str(exc)


def observe(args: Any, helpers: dict[str, Callable[..., Any]]) -> int:
    """Record success, errors and interruption without any restoration actions."""
    if args.duration_secs < 0 or args.sample_interval_secs <= 0:
        raise ValueError("duration must be nonnegative and sample interval positive")
    current_run = helpers["run_id"]()
    output = args.output_dir / current_run
    output.mkdir(parents=True, exist_ok=False)
    record_path = output / "soak_result.json"
    model_log = args.existing_server_log or helpers["default_existing_server_log"](args.reservoir_root)
    logs = {"bridge": Path(args.bridge_log), "model": model_log}
    logs.update({f"metrics_{i}": path for i, path in enumerate(args.request_metrics_log)})
    cursors = {name: log_cursor(path) for name, path in logs.items()}
    started = time.monotonic()
    start_ts = int(time.time())
    labels = list(dict.fromkeys(["com.astrid.spectral-bridge", *args.observe_label]))
    record: dict[str, Any] = {
        "schema_version": 2, "mode": "observe_only", "run_id": current_run,
        "started_at": helpers["iso_now"](), "requested_duration_s": args.duration_secs,
        "duration_s": 0, "sample_interval_s": args.sample_interval_secs,
        "candidate_port": args.candidate_port, "candidate_model": args.candidate_model,
        "candidate_url": f"http://127.0.0.1:{args.candidate_port}",
        "state": "running", "probes": [], "restore": None, "started_candidate": False,
        "live_mutations": [], "samples": [], "log_sources": cursors,
        "identity_start": [file_identity(path) for path in args.identity_file],
        "routing_environment": {key: command(["launchctl", "getenv", key]) for key in
                                ("ASTRID_BRIDGE_MLX_URL", "ASTRID_BRIDGE_MLX_PROFILE")},
        "observer_identity": file_identity(Path(__file__)),
    }

    def checkpoint() -> None:
        record["duration_s"] = round(time.monotonic() - started, 3)
        temporary = record_path.with_suffix(".json.tmp")
        temporary.write_text(json.dumps(record, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        temporary.replace(record_path)

    def interrupted(_signum: int, _frame: Any) -> None:
        raise KeyboardInterrupt

    previous_sigterm = signal.signal(signal.SIGTERM, interrupted)
    code = 0
    checkpoint()
    print(record_path, flush=True)
    try:
        while True:
            sample = status(args.candidate_port, labels)
            sample["health_failures"] = health_failures(sample)
            record["samples"].append(sample)
            with (output / "samples.jsonl").open("a", encoding="utf-8") as handle:
                handle.write(json.dumps(sample, sort_keys=True) + "\n")
            for name, cursor in cursors.items():
                collect_log(cursor, output / f"{name}.log")
            checkpoint()
            remaining = args.duration_secs - (time.monotonic() - started)
            if remaining <= 0:
                break
            time.sleep(min(args.sample_interval_secs, remaining))
        record["state"] = "completed"
    except KeyboardInterrupt:
        record["state"] = "interrupted"
        code = 130
    except Exception as exc:
        record["state"] = "error"
        record["error"] = f"{type(exc).__name__}: {exc}"
        code = 1
    finally:
        signal.signal(signal.SIGTERM, previous_sigterm)
        for name, cursor in cursors.items():
            collect_log(cursor, output / f"{name}.log")
        record["identity_end"] = [file_identity(path) for path in args.identity_file]
        record["completed_at"] = helpers["iso_now"]()
        def evidence(name: str) -> str:
            path = output / f"{name}.log"
            return path.read_text(encoding="utf-8", errors="replace") if path.exists() else ""
        events = helpers["count_events"](evidence("bridge"), evidence("model"))
        generations = helpers["parse_generation_stats"](evidence("model"))
        metrics = helpers["parse_request_metrics"]("\n".join(
            evidence(name) for name in logs if name.startswith("metrics_")))
        output_scan = helpers["scan_generated_outputs"](args.bridge_workspace, start_ts, int(time.time()))
        samples = record["samples"]
        bridge_bad = sum("state = running" not in sample["services"].get(
            "com.astrid.spectral-bridge", {}).get("stdout", "") for sample in samples)
        missing = sum(not sample["candidate_pids"] for sample in samples)
        health_errors = sum(bool(sample["health_failures"]) for sample in samples)
        identities = {stable_identity(sample) for sample in samples}
        failures = []
        for reason, failed in (
            ("observation_incomplete", record["state"] != "completed" or not samples),
            ("bridge_not_running", bridge_bad > 0), ("candidate_missing", missing > 0),
            ("health_endpoint_failed", health_errors > 0), ("process_identity_changed", len(identities) > 1),
            ("source_identity_changed", record["identity_start"] != record["identity_end"]),
            ("log_evidence_incomplete", any(c["discontinuities"] or c["read_errors"] for c in cursors.values())),
            ("fallback_incidents_exceeded", events["fallback_count"] > args.max_fallback_incidents),
            ("no_generation_observed", not generations["count"] and not metrics["count"]),
        ):
            if failed:
                failures.append(reason)
        provenance = review_observation(record)
        record["provenance_review"] = provenance
        if not provenance["passed"]:
            failures.append("provenance_incomplete_or_changed")
        failures.extend(reason for reason in helpers["build_failure_reasons"](
            bridge_bad_samples=[], candidate_missing_samples=[], events=events,
            latency_ok=True, output_scan=output_scan,
            max_fallback_incidents=args.max_fallback_incidents) if reason not in failures)
        record["monitor"] = {
            "events": events, "generation_stats": generations, "request_metrics": metrics,
            "generated_output_scan": output_scan, "bridge_bad_sample_count": bridge_bad,
            "candidate_missing_sample_count": missing, "health_error_sample_count": health_errors,
            "identity_change_count": max(0, len(identities) - 1),
            "failure_reasons": failures, "automated_ok": not failures,
            "performance_evidence": "available" if metrics["count"] else "not_available",
        }
        record["summary"] = {"automated_ok": not failures, "failure_reasons": failures,
                             "operator_review_required": True,
                             "performance_qualification": "separate_matched_benchmark_required"}
        checkpoint()
        if failures and not code:
            code = 1
    return code


def main() -> int:
    parser = argparse.ArgumentParser(description="Review retained passive observation provenance without host inspection")
    parser.add_argument("--review-record", type=Path, required=True)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    raw = args.review_record.read_bytes()
    record = json.loads(raw)
    report = review_observation(record)
    report["source_record_sha256"] = hashlib.sha256(raw).hexdigest()
    report["source_record"] = str(args.review_record.resolve())
    report["observed_health_ok"] = record.get("summary", {}).get("automated_ok") is True
    report["qualified_observation"] = report["passed"] and report["observed_health_ok"]
    rendered = json.dumps(report, indent=2, sort_keys=True) + "\n"
    if args.output:
        with args.output.open("x", encoding="utf-8") as handle:
            handle.write(rendered)
    print(rendered, end="")
    return 0 if report["qualified_observation"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
