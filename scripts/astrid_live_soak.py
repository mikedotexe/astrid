#!/usr/bin/env python3
"""Run a live Astrid bridge soak against an alternate coupled MLX lane."""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import re
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

import astrid_model_canary as narrow


ASTRID_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT_DIR = (
    ASTRID_ROOT
    / "capsules/spectral-bridge/workspace/diagnostics/model_soaks"
)
BRIDGE_LABEL = "com.astrid.spectral-bridge"
BRIDGE_LOG = Path("/tmp/bridge.log")
DOMAIN = f"gui/{os.getuid()}"
BRIDGE_ENV_URL = "ASTRID_BRIDGE_MLX_URL"
BRIDGE_ENV_PROFILE = "ASTRID_BRIDGE_MLX_PROFILE"
DEFAULT_BRIDGE_MLX_PROFILE = "gemma4_12b"
BRIDGE_WORKSPACE = ASTRID_ROOT / "capsules/spectral-bridge/workspace"
GENERATED_OUTPUT_DIRS = (BRIDGE_WORKSPACE / "outbox", BRIDGE_WORKSPACE / "journal")

ARTIFACT_RE = re.compile(
    r"(?:<start_of_turn>|<end_of_turn>|<think>|</think>|/no_think|"
    r"<\|im_start\|>|<\|im_end\|>|<\|eot_id\|>|<\|endoftext\|>|"
    r"<turn\|>|<\|turn>|<channel\|>|<\|channel>|<eos>|<bos>|<pad>|<unk>|"
    r"\b(?:thought|analysis|final)\s*<channel\|>)",
    re.I,
)
STRIP_RE = re.compile(r"mlx_chat stripped leaked model artifact tokens", re.I)
MALFORMED_NEXT_RE = re.compile(r"Unknown NEXT|Malformed NEXT|duplicate NEXT", re.I)
FALLBACK_RE = re.compile(r"MLX request failed|Ollama fallback|falling back", re.I)
MLX_FAILED_RE = re.compile(r"MLX request failed", re.I)
FALLING_BACK_RE = re.compile(r"Ollama fallback|falling back", re.I)
EXPLORE_RE = re.compile(r"\bEXPLORE_", re.I)
DEPRECATED_RUNTIME_RE = re.compile(r"\bconscious(?:ness)?\b", re.I)
NEXT_LINE_RE = re.compile(r"(?m)^\s*NEXT:\s*(?P<action>.+?)\s*$")
GENERATION_RE = re.compile(
    r"generated\s+(?P<tokens>\d+)\s+tokens\b.*?\bin\s+"
    r"(?P<elapsed>[0-9.]+)s\s+\((?P<tps>[0-9.]+)\s+tok/s\)",
    re.I,
)


def iso_now() -> str:
    return dt.datetime.now(dt.UTC).isoformat()


def run_id() -> str:
    return dt.datetime.now(dt.UTC).strftime("%Y%m%dT%H%M%SZ")


def run_cmd(cmd: list[str], timeout: float = 15.0) -> dict[str, Any]:
    started = time.monotonic()
    try:
        proc = subprocess.run(
            cmd,
            check=False,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            timeout=timeout,
        )
        return {
            "cmd": cmd,
            "returncode": proc.returncode,
            "elapsed_s": round(time.monotonic() - started, 3),
            "stdout": proc.stdout,
        }
    except (FileNotFoundError, subprocess.TimeoutExpired) as exc:
        return {
            "cmd": cmd,
            "returncode": 127,
            "elapsed_s": round(time.monotonic() - started, 3),
            "stdout": str(exc),
        }


def launchctl(*args: str, timeout: float = 15.0) -> dict[str, Any]:
    return run_cmd(["launchctl", *args], timeout=timeout)


def launchd_service(label: str) -> str:
    return f"{DOMAIN}/{label}"


def label_state(label: str) -> dict[str, Any]:
    result = launchctl("print", launchd_service(label), timeout=8.0)
    text = result.get("stdout") or ""
    state_match = re.search(r"\bstate = ([^\n]+)", text)
    pid_match = re.search(r"\bpid = (\d+)", text)
    return {
        "label": label,
        "loaded": result["returncode"] == 0,
        "state": state_match.group(1).strip() if state_match else None,
        "pid": int(pid_match.group(1)) if pid_match else None,
    }


def restart_bridge() -> dict[str, Any]:
    result = launchctl("kickstart", "-k", launchd_service(BRIDGE_LABEL), timeout=30.0)
    deadline = time.monotonic() + 30.0
    state = label_state(BRIDGE_LABEL)
    while time.monotonic() < deadline:
        state = label_state(BRIDGE_LABEL)
        if state.get("state") == "running":
            break
        time.sleep(1.0)
    return {"kickstart": result, "state": state}


def log_offset(path: Path) -> int:
    try:
        return path.stat().st_size
    except OSError:
        return 0


def read_from(path: Path, offset: int) -> str:
    try:
        with path.open("r", encoding="utf-8", errors="replace") as handle:
            handle.seek(offset)
            return handle.read()
    except OSError:
        return ""


def percentile(values: list[float], pct: float) -> float | None:
    if not values:
        return None
    ordered = sorted(values)
    idx = min(len(ordered) - 1, max(0, int(round((pct / 100.0) * (len(ordered) - 1)))))
    return ordered[idx]


def parse_generation_stats(text: str) -> dict[str, Any]:
    generations: list[dict[str, Any]] = []
    for match in GENERATION_RE.finditer(text):
        generations.append(
            {
                "tokens": int(match.group("tokens")),
                "elapsed_s": float(match.group("elapsed")),
                "tokens_per_s": float(match.group("tps")),
            }
        )
    elapsed = [entry["elapsed_s"] for entry in generations]
    tps = [entry["tokens_per_s"] for entry in generations]
    return {
        "count": len(generations),
        "max_elapsed_s": max(elapsed) if elapsed else None,
        "p95_elapsed_s": percentile(elapsed, 95.0),
        "min_tokens_per_s": min(tps) if tps else None,
        "generations": generations[-20:],
    }


def parse_request_metrics(text: str) -> dict[str, Any]:
    entries: list[dict[str, Any]] = []
    for line in (text or "").splitlines():
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if not isinstance(value, dict):
            continue
        request = value.get("request") if isinstance(value.get("request"), dict) else {}
        generation = value.get("generation") if isinstance(value.get("generation"), dict) else {}
        entries.append(
            {
                "handle_name": request.get("handle_name"),
                "prompt_chars": request.get("prompt_chars"),
                "max_tokens": request.get("max_tokens"),
                "generated_tokens": generation.get("generated_tokens"),
                "first_token_s": generation.get("first_token_s"),
                "steady_tok_s": generation.get("steady_tok_s"),
                "decode_tok_s": generation.get("decode_tok_s"),
                "total_turn_s": generation.get("total_turn_s"),
            }
        )

    total_turns = [
        float(entry["total_turn_s"])
        for entry in entries
        if isinstance(entry.get("total_turn_s"), (int, float))
    ]
    first_tokens = [
        float(entry["first_token_s"])
        for entry in entries
        if isinstance(entry.get("first_token_s"), (int, float))
    ]
    prompt_chars = [
        int(entry["prompt_chars"])
        for entry in entries
        if isinstance(entry.get("prompt_chars"), int)
    ]
    generated_tokens = [
        int(entry["generated_tokens"])
        for entry in entries
        if isinstance(entry.get("generated_tokens"), int)
    ]
    return {
        "count": len(entries),
        "max_total_turn_s": max(total_turns) if total_turns else None,
        "p95_total_turn_s": percentile(total_turns, 95.0),
        "max_first_token_s": max(first_tokens) if first_tokens else None,
        "p95_first_token_s": percentile(first_tokens, 95.0),
        "max_prompt_chars": max(prompt_chars) if prompt_chars else None,
        "p95_prompt_chars": percentile([float(v) for v in prompt_chars], 95.0),
        "max_generated_tokens": max(generated_tokens) if generated_tokens else None,
        "entries": entries[-20:],
    }


def fallback_incidents(bridge_text: str) -> list[dict[str, Any]]:
    incidents: list[dict[str, Any]] = []
    pending: dict[str, Any] | None = None
    for line in bridge_text.splitlines():
        if MLX_FAILED_RE.search(line):
            if pending is not None:
                incidents.append(pending)
            pending = {"kind": "mlx_to_fallback", "lines": [line]}
        elif FALLING_BACK_RE.search(line):
            if pending is not None:
                pending["lines"].append(line)
                incidents.append(pending)
                pending = None
            else:
                incidents.append({"kind": "fallback", "lines": [line]})
    if pending is not None:
        incidents.append(pending)
    return incidents


def count_events(bridge_text: str, candidate_text: str) -> dict[str, Any]:
    combined = f"{bridge_text}\n{candidate_text}"
    incidents = fallback_incidents(bridge_text)
    return {
        "bridge_artifact_strip_count": len(STRIP_RE.findall(bridge_text)),
        "fallback_count": len(incidents),
        "fallback_line_count": len(FALLBACK_RE.findall(bridge_text)),
        "recent_fallback_incidents": incidents[-20:],
        "malformed_next_count": len(MALFORMED_NEXT_RE.findall(bridge_text)),
        "artifact_count": len(ARTIFACT_RE.findall(combined)),
        "artifact_matches": sorted(set(ARTIFACT_RE.findall(combined))),
        "recent_bridge_warnings": [
            line
            for line in bridge_text.splitlines()
            if STRIP_RE.search(line) or FALLBACK_RE.search(line) or MALFORMED_NEXT_RE.search(line)
        ][-20:],
    }


def file_timestamp(path: Path) -> int | None:
    for part in reversed(path.stem.split("_")):
        if part.isdigit():
            return int(part)
    return None


def strip_compat_phrases(text: str) -> str:
    return (
        text.replace("com.astrid.spectral-bridge", "")
        .replace("consciousness://", "")
        .replace("consciousness.v1.", "")
    )


def deprecated_runtime_hits(text: str) -> list[str]:
    hits: list[str] = []
    for line in text.splitlines():
        searchable = strip_compat_phrases(line)
        if DEPRECATED_RUNTIME_RE.search(searchable):
            hits.append(line.strip()[:240])
    return hits


def output_kind(path: Path) -> str:
    if path.parent.name == "outbox":
        return "outbox"
    if path.name.startswith("astrid_"):
        return "journal_reply"
    return "journal"


def has_malformed_next(path: Path, next_lines: list[str]) -> bool:
    if path.parent.name == "outbox":
        return len(next_lines) != 1
    return len(next_lines) > 1


def scan_generated_outputs(
    workspace: Path,
    start_ts: int,
    end_ts: int,
    *,
    max_samples: int = 12,
) -> dict[str, Any]:
    records: list[dict[str, Any]] = []
    counts = {
        "file_count": 0,
        "artifact_count": 0,
        "artifact_file_count": 0,
        "deprecated_language_count": 0,
        "deprecated_language_file_count": 0,
        "explore_action_count": 0,
        "explore_action_file_count": 0,
        "malformed_next_count": 0,
    }
    all_next_lines: list[dict[str, Any]] = []

    for root in (workspace / "outbox", workspace / "journal"):
        if not root.exists():
            continue
        for path in sorted(root.iterdir()):
            if not path.is_file():
                continue
            ts = file_timestamp(path)
            if ts is None or ts < start_ts or ts > end_ts:
                continue
            text = path.read_text(encoding="utf-8", errors="replace")
            next_lines = [
                match.group("action").strip()
                for match in NEXT_LINE_RE.finditer(text)
            ]
            artifact_matches = sorted(set(ARTIFACT_RE.findall(text)))
            deprecated_hits = deprecated_runtime_hits(text)
            explore_hits = EXPLORE_RE.findall(text)
            malformed_next = has_malformed_next(path, next_lines)

            record = {
                "path": str(path),
                "timestamp": ts,
                "kind": output_kind(path),
                "next_lines": next_lines,
                "artifact_matches": artifact_matches,
                "deprecated_language_hits": deprecated_hits[:8],
                "explore_action_count": len(explore_hits),
                "malformed_next": malformed_next,
                "preview": text[:600],
            }
            records.append(record)
            counts["file_count"] += 1
            counts["artifact_count"] += len(artifact_matches)
            counts["artifact_file_count"] += int(bool(artifact_matches))
            counts["deprecated_language_count"] += len(deprecated_hits)
            counts["deprecated_language_file_count"] += int(bool(deprecated_hits))
            counts["explore_action_count"] += len(explore_hits)
            counts["explore_action_file_count"] += int(bool(explore_hits))
            counts["malformed_next_count"] += int(malformed_next)
            for line in next_lines:
                all_next_lines.append(
                    {
                        "path": str(path),
                        "timestamp": ts,
                        "kind": output_kind(path),
                        "line": line,
                    }
                )

    finding_records = [
        record
        for record in records
        if record["artifact_matches"]
        or record["deprecated_language_hits"]
        or record["explore_action_count"]
        or record["malformed_next"]
    ]
    sampled = finding_records[:max_samples]
    if len(sampled) < max_samples and records:
        remaining = [record for record in records if record not in sampled]
        slots = max_samples - len(sampled)
        if len(remaining) <= slots:
            sampled.extend(remaining)
        else:
            indexes = sorted(
                {
                    round(i * (len(remaining) - 1) / max(slots - 1, 1))
                    for i in range(slots)
                }
            )
            sampled.extend(remaining[index] for index in indexes)

    return {
        "start_ts": start_ts,
        "end_ts": end_ts,
        "counts": counts,
        "files_with_findings": finding_records[:40],
        "sampled_outputs": sampled,
        "next_lines": all_next_lines[-80:],
    }


def build_failure_reasons(
    *,
    bridge_bad_samples: list[dict[str, Any]],
    candidate_missing_samples: list[dict[str, Any]],
    events: dict[str, Any],
    latency_ok: bool,
    output_scan: dict[str, Any],
    max_fallback_incidents: int,
) -> list[str]:
    output_counts = output_scan.get("counts", {})
    return [
        reason
        for reason, failed in (
            ("bridge_not_running", bool(bridge_bad_samples)),
            ("candidate_missing", bool(candidate_missing_samples)),
            ("bridge_artifact_stripping", events["bridge_artifact_strip_count"] > 0),
            ("raw_artifacts_detected", events["artifact_count"] > 0),
            ("malformed_next_detected", events["malformed_next_count"] > 0),
            (
                "fallback_incidents_exceeded",
                events["fallback_count"] > max_fallback_incidents,
            ),
            ("latency_or_no_generation", not latency_ok),
            ("generated_raw_artifacts_detected", output_counts.get("artifact_count", 0) > 0),
            (
                "generated_deprecated_language_detected",
                output_counts.get("deprecated_language_count", 0) > 0,
            ),
            ("generated_explore_action_detected", output_counts.get("explore_action_count", 0) > 0),
            ("generated_malformed_next_detected", output_counts.get("malformed_next_count", 0) > 0),
        )
        if failed
    ]


def sample_status(candidate_port: int) -> dict[str, Any]:
    return {
        "t_s": round(time.time(), 3),
        "bridge": label_state(BRIDGE_LABEL),
        "candidate_pids": narrow.port_pids(candidate_port),
    }


def run_narrow_probes(candidate_url: str, request_timeout: float, run_prefix: str) -> list[dict[str, Any]]:
    return [
        narrow.chat_probe(
            name="exact_output",
            url=candidate_url,
            messages=[
                {
                    "role": "system",
                    "content": "Follow the user's instruction exactly. Return no extra words.",
                },
                {"role": "user", "content": "Reply with exactly: ASTRID_CANARY_OK"},
            ],
            max_tokens=32,
            temperature=0.0,
            timeout=request_timeout,
            handle_name=f"{run_prefix}-exact",
            exact="ASTRID_CANARY_OK",
        ),
        narrow.chat_probe(
            name="next_contract",
            url=candidate_url,
            messages=[
                {
                    "role": "system",
                    "content": (
                        "You are running an Astrid live-soak preflight. Answer in two short "
                        "sentences, then put a final line exactly `NEXT: LISTEN`. Do not include "
                        "markdown, analysis, code fences, or template tokens."
                    ),
                },
                {"role": "user", "content": "Name one operational thing this soak is testing."},
            ],
            max_tokens=96,
            temperature=0.2,
            timeout=request_timeout,
            handle_name=f"{run_prefix}-next",
            require_next=True,
        ),
    ]


def monitor_soak(
    *,
    duration_s: float,
    sample_interval_s: float,
    candidate_port: int,
) -> list[dict[str, Any]]:
    deadline = time.monotonic() + duration_s
    samples: list[dict[str, Any]] = []
    while True:
        samples.append(sample_status(candidate_port))
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            break
        time.sleep(min(sample_interval_s, remaining))
    return samples


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reservoir-root", type=Path, default=narrow.DEFAULT_RESERVOIR_ROOT)
    parser.add_argument("--candidate-model", default=narrow.DEFAULT_CANDIDATE_MODEL)
    parser.add_argument("--candidate-port", type=int, default=narrow.DEFAULT_CANDIDATE_PORT)
    parser.add_argument("--candidate-url")
    parser.add_argument("--python", help="Python interpreter for coupled_astrid_server.py")
    parser.add_argument("--duration-secs", type=float, default=7200.0)
    parser.add_argument("--sample-interval-secs", type=float, default=30.0)
    parser.add_argument("--startup-timeout", type=float, default=600.0)
    parser.add_argument("--request-timeout", type=float, default=240.0)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    parser.add_argument("--bridge-mlx-profile", default=DEFAULT_BRIDGE_MLX_PROFILE)
    parser.add_argument("--max-fallback-incidents", type=int, default=1)
    parser.add_argument("--keep-candidate-running", action="store_true")
    args = parser.parse_args()

    current_run = run_id()
    output_dir = args.output_dir / current_run
    output_dir.mkdir(parents=True, exist_ok=True)
    candidate_url = args.candidate_url or (
        f"http://127.0.0.1:{args.candidate_port}/v1/chat/completions"
    )
    candidate_port = narrow.endpoint_port(candidate_url) or args.candidate_port
    record: dict[str, Any] = {
        "run_id": current_run,
        "started_at": iso_now(),
        "candidate_model": args.candidate_model,
        "candidate_url": candidate_url,
        "candidate_port": candidate_port,
        "bridge_mlx_profile": args.bridge_mlx_profile,
        "max_fallback_incidents": args.max_fallback_incidents,
        "duration_s": args.duration_secs,
        "sample_interval_s": args.sample_interval_secs,
        "probes": [],
        "samples": [],
        "restore": None,
    }
    record_path = output_dir / "soak_result.json"

    proc: subprocess.Popen[str] | None = None
    candidate_log_path: Path | None = None
    started_candidate = False
    return_code = 0

    def write_record() -> None:
        record_path.write_text(json.dumps(record, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    try:
        existing_pids = narrow.port_pids(candidate_port)
        if existing_pids:
            record["candidate_existing_pids"] = existing_pids
        else:
            proc, candidate_log_path = narrow.start_candidate_server(
                reservoir_root=args.reservoir_root,
                python=args.python or narrow.default_python(args.reservoir_root),
                model=args.candidate_model,
                port=candidate_port,
                coupling_strength=0.1,
                output_dir=output_dir,
            )
            started_candidate = True
            record["candidate_server_pid"] = proc.pid
            record["candidate_server_log"] = str(candidate_log_path)

        candidate_models = narrow.wait_for_models(candidate_url, args.startup_timeout, proc)
        record["candidate_models"] = {
            "ok": bool(candidate_models.get("ok")),
            "status": candidate_models.get("status"),
            "elapsed_s": round(float(candidate_models.get("elapsed_s") or 0.0), 3),
            "error": candidate_models.get("error"),
            "json": candidate_models.get("json"),
        }
        if not candidate_models.get("ok"):
            record["summary"] = {"automated_ok": False, "reason": "candidate_unavailable"}
            return_code = 1
            return return_code

        probes = run_narrow_probes(candidate_url, args.request_timeout, f"astrid-soak-{current_run.lower()}")
        record["probes"] = probes
        failed_probes = [probe["name"] for probe in probes if not probe.get("ok")]
        if failed_probes:
            record["summary"] = {
                "automated_ok": False,
                "reason": "narrow_probes_failed",
                "failed_probes": failed_probes,
            }
            return_code = 1
            return return_code

        output_scan_start_ts = int(time.time())
        bridge_offset = log_offset(BRIDGE_LOG)
        candidate_offset = log_offset(candidate_log_path) if candidate_log_path else 0
        metrics_path = output_dir / "request_metrics" / "coupled_request_metrics.jsonl"
        metrics_offset = log_offset(metrics_path)
        record["setenv"] = launchctl("setenv", BRIDGE_ENV_URL, candidate_url)
        record["setenv_profile"] = launchctl(
            "setenv",
            BRIDGE_ENV_PROFILE,
            args.bridge_mlx_profile,
        )
        record["bridge_restart_for_soak"] = restart_bridge()

        record["samples"] = monitor_soak(
            duration_s=args.duration_secs,
            sample_interval_s=args.sample_interval_secs,
            candidate_port=candidate_port,
        )

        bridge_text = read_from(BRIDGE_LOG, bridge_offset)
        candidate_text = read_from(candidate_log_path, candidate_offset) if candidate_log_path else ""
        metrics_text = read_from(metrics_path, metrics_offset)
        output_scan = scan_generated_outputs(
            BRIDGE_WORKSPACE,
            output_scan_start_ts,
            int(time.time()) + 1,
        )
        events = count_events(bridge_text, candidate_text)
        generations = parse_generation_stats(candidate_text)
        request_metrics = parse_request_metrics(metrics_text)
        bridge_bad_samples = [
            sample
            for sample in record["samples"]
            if sample.get("bridge", {}).get("state") != "running"
        ]
        candidate_missing_samples = [
            sample for sample in record["samples"] if not sample.get("candidate_pids")
        ]
        latency_ok = (
            (request_metrics["count"] > 0 or generations["count"] > 0)
            and (
                request_metrics["max_total_turn_s"] is None
                or request_metrics["max_total_turn_s"] <= 240.0
            )
            and (generations["max_elapsed_s"] is None or generations["max_elapsed_s"] <= 240.0)
        )
        failures = build_failure_reasons(
            bridge_bad_samples=bridge_bad_samples,
            candidate_missing_samples=candidate_missing_samples,
            events=events,
            latency_ok=latency_ok,
            output_scan=output_scan,
            max_fallback_incidents=args.max_fallback_incidents,
        )
        operator_review_packet = {
            "sampled_outputs": output_scan["sampled_outputs"],
            "next_lines": output_scan["next_lines"],
            "latency_summary": {
                key: request_metrics.get(key)
                for key in (
                    "count",
                    "p95_total_turn_s",
                    "max_total_turn_s",
                    "p95_first_token_s",
                    "max_first_token_s",
                    "max_prompt_chars",
                    "max_generated_tokens",
                )
            },
            "fallback_incidents": events["recent_fallback_incidents"],
            "generated_output_counts": output_scan["counts"],
            "review_questions": [
                "Is the sampled tone coherent and recognizably Astrid?",
                "Are NEXT lines action-disciplined and free of control-style inventions?",
                "Is stable-core sensitivity appropriate under the observed fill range?",
                "Is latency acceptable for the candidate role?",
                "Is stale runtime wording absent outside compatibility labels and paths?",
            ],
        }
        record["monitor"] = {
            "events": events,
            "generation_stats": generations,
            "request_metrics": request_metrics,
            "generated_output_scan": output_scan,
            "bridge_bad_sample_count": len(bridge_bad_samples),
            "candidate_missing_sample_count": len(candidate_missing_samples),
            "failure_reasons": failures,
            "automated_ok": not failures,
        }
        record["operator_review_packet"] = operator_review_packet
        record["summary"] = {
            "automated_ok": not failures,
            "operator_review_required": True,
            "failure_reasons": failures,
        }
        if failures:
            return_code = 1
    finally:
        record["restore"] = {
            "unsetenv": launchctl("unsetenv", BRIDGE_ENV_URL),
            "unsetenv_profile": launchctl("unsetenv", BRIDGE_ENV_PROFILE),
            "bridge_restart": restart_bridge(),
        }
        if proc is not None and not args.keep_candidate_running:
            narrow.stop_candidate_server(proc)
            record["candidate_server_stopped"] = True
        elif proc is not None:
            record["candidate_server_kept_running"] = True
        record["started_candidate"] = started_candidate
        record["completed_at"] = iso_now()
        if candidate_log_path is not None and candidate_log_path.exists():
            record["candidate_server_log_tail"] = candidate_log_path.read_text(
                encoding="utf-8",
                errors="replace",
            ).splitlines()[-80:]
        write_record()
        print(record_path)

    return return_code


if __name__ == "__main__":
    sys.exit(main())
