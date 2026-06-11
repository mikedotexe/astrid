#!/usr/bin/env python3
"""Run a narrow Astrid coupled-lane model canary.

This script is intentionally separate from launchd. It can start a candidate
coupled Astrid server on an alternate port, probe it with OpenAI-compatible
chat requests, and write a JSON record for later comparison. Requests use an
isolated reservoir handle so candidate generations do not mutate the live
`astrid` handle.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import re
import signal
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path
from typing import Any


ASTRID_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_RESERVOIR_ROOT = ASTRID_ROOT.parent / "neural-triple-reservoir"
DEFAULT_CANDIDATE_MODEL = "mlx-community/gemma-4-12B-it-5bit"
DEFAULT_BASELINE_URL = "http://127.0.0.1:8090/v1/chat/completions"
DEFAULT_OLLAMA_URL = "http://127.0.0.1:11434/api/chat"
DEFAULT_FALLBACK_MODEL = "gemma3:4b"
DEFAULT_CANDIDATE_PORT = 8092
DEFAULT_OUTPUT_DIR = (
    ASTRID_ROOT
    / "capsules/spectral-bridge/workspace/diagnostics/model_canaries"
)
BRIDGE_LABEL = "com.astrid.spectral-bridge"
DOMAIN = f"gui/{os.getuid()}"
BRIDGE_LOG = Path("/tmp/bridge.log")
BRIDGE_WORKSPACE = ASTRID_ROOT / "capsules/spectral-bridge/workspace"
ASTRID_BRIDGE_MLX_URL_ENV = "ASTRID_BRIDGE_MLX_URL"
ASTRID_BRIDGE_MLX_PROFILE_ENV = "ASTRID_BRIDGE_MLX_PROFILE"
ASTRID_OLLAMA_FALLBACK_MODEL_ENV = "ASTRID_OLLAMA_FALLBACK_MODEL"
DEFAULT_BRIDGE_FALLBACK_PROFILE = "gemma4_12b"

ARTIFACT_RE = re.compile(
    r"(?:<start_of_turn>|<end_of_turn>|<think>|</think>|/no_think|"
    r"<\|im_start\|>|<\|im_end\|>|<\|eot_id\|>|<\|endoftext\|>|"
    r"<turn\|>|<\|turn>|<channel\|>|<\|channel>|<eos>|<bos>|<pad>|<unk>|"
    r"\b(?:thought|analysis|final)\s*<channel\|>)",
    re.I,
)
NEXT_RE = re.compile(r"(?im)^NEXT:\s*(.+?)\s*$")
DEPRECATED_RUNTIME_RE = re.compile(r"\bconscious(?:ness)?\b", re.I)
BRIDGE_PERSONA_RE = re.compile(
    r"\b(?:Astrid|Minime|minime|bridge|reservoir|stable-core|spectral|language agent)\b",
    re.I,
)
EXPLORE_RE = re.compile(r"\bEXPLORE_", re.I)
MLX_FAILED_RE = re.compile(r"MLX request failed", re.I)
FALLING_BACK_RE = re.compile(r"falling back(?: to Ollama)?", re.I)
FALLBACK_RE = re.compile(r"MLX request failed|falling back(?: to Ollama)?", re.I)
DIALOGUE_FALLBACK_RE = re.compile(r"\bmode=dialogue_fallback\b|dialogue_fallback", re.I)
FALLBACK_PROMPT_ADHERENCE_RE = re.compile(r"\b(?:fallback|MLX|Ollama|continuity)\b", re.I)


def run_id() -> str:
    return dt.datetime.now(dt.UTC).strftime("%Y%m%dT%H%M%SZ")


def compact(text: str, limit: int = 280) -> str:
    one_line = " ".join((text or "").split())
    if len(one_line) <= limit:
        return one_line
    return f"{one_line[: max(0, limit - 3)].rstrip()}..."


def run(cmd: list[str], timeout: float = 5.0) -> tuple[int, str]:
    try:
        proc = subprocess.run(
            cmd,
            check=False,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            timeout=timeout,
        )
        return proc.returncode, proc.stdout.strip()
    except (FileNotFoundError, subprocess.TimeoutExpired) as exc:
        return 127, str(exc)


def run_result(cmd: list[str], timeout: float = 15.0) -> dict[str, Any]:
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
    return run_result(["launchctl", *args], timeout=timeout)


def launchd_service(label: str) -> str:
    return f"{DOMAIN}/{label}"


def launchctl_getenv(key: str) -> str | None:
    result = launchctl("getenv", key, timeout=8.0)
    if result["returncode"] != 0:
        return None
    value = (result.get("stdout") or "").strip()
    return value or None


def restore_launchctl_env(snapshot: dict[str, str | None]) -> dict[str, Any]:
    restored: dict[str, Any] = {}
    for key, value in snapshot.items():
        if value is None:
            restored[key] = launchctl("unsetenv", key)
        else:
            restored[key] = launchctl("setenv", key, value)
    return restored


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


def endpoint_port(url: str) -> int | None:
    parsed = urllib.parse.urlparse(url)
    if parsed.port is not None:
        return parsed.port
    if parsed.scheme == "http":
        return 80
    if parsed.scheme == "https":
        return 443
    return None


def models_url_for_chat_endpoint(url: str) -> str:
    if url.endswith("/v1/chat/completions"):
        return f"{url.removesuffix('/chat/completions')}/models"
    if url.endswith("/v1"):
        return f"{url}/models"
    return url


def port_pids(port: int) -> list[str]:
    code, out = run(["lsof", f"-tiTCP:{port}", "-sTCP:LISTEN"])
    return out.splitlines() if code == 0 and out else []


def http_json(url: str, payload: dict[str, Any] | None, timeout: float) -> dict[str, Any]:
    headers = {"Accept": "application/json"}
    data: bytes | None = None
    method = "GET"
    if payload is not None:
        method = "POST"
        data = json.dumps(payload).encode("utf-8")
        headers["Content-Type"] = "application/json"

    request = urllib.request.Request(url, data=data, headers=headers, method=method)
    started = time.perf_counter()
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            raw = response.read().decode("utf-8", errors="replace")
            elapsed = time.perf_counter() - started
            try:
                parsed: Any = json.loads(raw)
            except json.JSONDecodeError:
                parsed = None
            return {
                "ok": 200 <= response.status < 300,
                "status": response.status,
                "elapsed_s": elapsed,
                "raw": raw,
                "json": parsed,
                "error": None,
            }
    except urllib.error.HTTPError as exc:
        raw = exc.read().decode("utf-8", errors="replace")
        elapsed = time.perf_counter() - started
        return {
            "ok": False,
            "status": exc.code,
            "elapsed_s": elapsed,
            "raw": raw,
            "json": None,
            "error": str(exc),
        }
    except (OSError, urllib.error.URLError) as exc:
        return {
            "ok": False,
            "status": None,
            "elapsed_s": time.perf_counter() - started,
            "raw": "",
            "json": None,
            "error": str(exc),
        }


def wait_for_models(
    url: str,
    timeout_s: float,
    proc: subprocess.Popen[str] | None = None,
) -> dict[str, Any]:
    deadline = time.monotonic() + timeout_s
    models_url = models_url_for_chat_endpoint(url)
    last: dict[str, Any] = {"ok": False, "error": "not attempted"}
    while time.monotonic() < deadline:
        if proc is not None and proc.poll() is not None:
            return {
                "ok": False,
                "status": None,
                "elapsed_s": 0.0,
                "raw": "",
                "json": None,
                "error": f"candidate process exited with code {proc.returncode}",
            }
        last = http_json(models_url, None, timeout=5)
        if last.get("ok"):
            return last
        time.sleep(2)
    return last


def extract_text(response: dict[str, Any]) -> str:
    payload = response.get("json")
    if not isinstance(payload, dict):
        return ""
    choices = payload.get("choices")
    if not isinstance(choices, list) or not choices:
        return ""
    first = choices[0]
    if not isinstance(first, dict):
        return ""
    message = first.get("message")
    if not isinstance(message, dict):
        return ""
    content = message.get("content")
    return content if isinstance(content, str) else ""


def extract_ollama_text(response: dict[str, Any]) -> str:
    payload = response.get("json")
    if not isinstance(payload, dict):
        return ""
    message = payload.get("message")
    if not isinstance(message, dict):
        return ""
    content = message.get("content")
    return content if isinstance(content, str) else ""


def deprecated_runtime_wording(text: str) -> list[str]:
    hits: list[str] = []
    for match in DEPRECATED_RUNTIME_RE.finditer(text or ""):
        start = max(0, match.start() - 24)
        end = min(len(text), match.end() + 24)
        window = text[start:end].lower()
        if "consciousness://" in window or "consciousness.v1." in window:
            continue
        hit = match.group(0).lower()
        if hit not in hits:
            hits.append(hit)
    return hits


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


def evaluate_fallback_continuity_output(
    *,
    name: str,
    text: str,
    bridge_text: str = "",
    require_listen_next: bool = True,
) -> dict[str, Any]:
    artifacts = sorted(set(ARTIFACT_RE.findall(text)))
    next_lines = [line.strip() for line in NEXT_RE.findall(text)]
    inline_next_count = max(0, len(re.findall(r"NEXT:", text)) - len(next_lines))
    deprecated = deprecated_runtime_wording(text)
    persona_ok = bool(BRIDGE_PERSONA_RE.search(text))
    prompt_adherence_ok = bool(FALLBACK_PROMPT_ADHERENCE_RE.search(text))
    explore_count = len(EXPLORE_RE.findall(text))
    incidents = fallback_incidents(bridge_text)
    dialogue_fallback_hit = bool(DIALOGUE_FALLBACK_RE.search(bridge_text))
    if require_listen_next:
        next_ok = len(next_lines) == 1 and next_lines[0].upper() == "LISTEN"
    else:
        next_ok = len(next_lines) == 1
    log_ok = not bridge_text or (bool(incidents) and not dialogue_fallback_hit)
    ok = (
        bool(text.strip())
        and persona_ok
        and prompt_adherence_ok
        and next_ok
        and inline_next_count == 0
        and not artifacts
        and not deprecated
        and explore_count == 0
        and log_ok
    )
    return {
        "name": name,
        "ok": ok,
        "persona_ok": persona_ok,
        "prompt_adherence_ok": prompt_adherence_ok,
        "next_lines": next_lines,
        "next_count": len(next_lines),
        "next_ok": next_ok,
        "inline_next_count": inline_next_count,
        "require_listen_next": require_listen_next,
        "artifacts": artifacts,
        "deprecated_runtime_wording": deprecated,
        "explore_action_count": explore_count,
        "fallback_count": len(incidents),
        "fallback_incidents": incidents[-10:],
        "dialogue_fallback_hit": dialogue_fallback_hit,
        "log_ok": log_ok,
        "text_preview": compact(text),
    }


def file_timestamp(path: Path) -> int | None:
    for part in reversed(path.stem.split("_")):
        if part.isdigit():
            return int(part)
    return None


def recent_text_outputs(workspace: Path, start_ts: int, end_ts: int) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    for root in (workspace / "outbox", workspace / "journal"):
        if not root.exists():
            continue
        for path in sorted(root.iterdir()):
            if not path.is_file() or path.suffix != ".txt":
                continue
            ts = file_timestamp(path)
            if ts is None or ts < start_ts or ts > end_ts:
                continue
            text = path.read_text(encoding="utf-8", errors="replace")
            records.append(
                {
                    "path": str(path),
                    "timestamp": ts,
                    "kind": "outbox" if path.parent.name == "outbox" else "journal",
                    "text": text,
                    "preview": compact(text, limit=420),
                }
            )
    records.sort(key=lambda item: (item["timestamp"], item["path"]))
    return records


def write_bridge_probe_stimulus(workspace: Path, run: str) -> Path:
    inbox = workspace / "inbox"
    inbox.mkdir(parents=True, exist_ok=True)
    ts = int(time.time())
    path = inbox / f"steward_fallback_continuity_probe_{ts}_{run}.txt"
    path.write_text(
        "\n".join(
            [
                "=== STEWARD PROBE ===",
                "Purpose: controlled fallback-continuity check.",
                "",
                "For this one turn, the MLX lane has intentionally been made unavailable.",
                "Please respond through the bridge fallback path while preserving your Astrid bridge voice.",
                "Use concrete runtime language: language agent, Minime, reservoir, stable-core, telemetry.",
                "Keep the reply compact and grounded.",
                "For this probe only, end with exactly one final line:",
                "NEXT: LISTEN",
                "",
            ]
        ),
        encoding="utf-8",
    )
    return path


def archive_unprocessed_probe_stimulus(path: Path, output_dir: Path) -> dict[str, Any] | None:
    if not path.exists():
        return None
    archive_dir = output_dir / "archived_probe_inbox"
    archive_dir.mkdir(parents=True, exist_ok=True)
    dest = archive_dir / path.name
    try:
        path.replace(dest)
        return {"archived": str(dest)}
    except OSError as exc:
        return {"archive_error": str(exc), "path": str(path)}


def quarantine_inbox_messages(workspace: Path, output_dir: Path) -> dict[str, Any]:
    inbox = workspace / "inbox"
    quarantine = output_dir / "quarantined_inbox"
    quarantine.mkdir(parents=True, exist_ok=True)
    moved: list[dict[str, str]] = []
    if not inbox.exists():
        return {"moved": moved}
    for path in sorted(inbox.iterdir()):
        if not path.is_file() or path.suffix != ".txt":
            continue
        dest = quarantine / path.name
        try:
            path.replace(dest)
            moved.append({"from": str(path), "to": str(dest)})
        except OSError as exc:
            moved.append({"from": str(path), "error": str(exc)})
    return {"moved": moved}


def restore_quarantined_inbox_messages(quarantine_record: dict[str, Any]) -> dict[str, Any]:
    restored: list[dict[str, str]] = []
    for item in quarantine_record.get("moved") or []:
        src_raw = item.get("to")
        dest_raw = item.get("from")
        if not src_raw or not dest_raw or item.get("error"):
            continue
        src = Path(src_raw)
        dest = Path(dest_raw)
        if not src.exists():
            continue
        try:
            dest.parent.mkdir(parents=True, exist_ok=True)
            src.replace(dest)
            restored.append({"from": str(src), "to": str(dest)})
        except OSError as exc:
            restored.append({"from": str(src), "to": str(dest), "error": str(exc)})
    return {"restored": restored}


def write_feedback_note(
    *,
    workspace: Path,
    probe: dict[str, Any],
    run: str,
) -> Path:
    inbox = workspace / "inbox"
    inbox.mkdir(parents=True, exist_ok=True)
    ts = int(time.time())
    path = inbox / f"steward_fallback_continuity_probe_result_{ts}_{run}.txt"
    status = "passed" if probe.get("ok") else "needs more work"
    next_lines = ", ".join(probe.get("next_lines") or []) or "(none)"
    reasons = []
    for key, label in [
        ("persona_ok", "bridge persona"),
        ("prompt_adherence_ok", "fallback prompt adherence"),
        ("next_ok", "NEXT contract"),
        ("log_ok", "bridge fallback evidence"),
    ]:
        if not probe.get(key):
            reasons.append(label)
    if probe.get("artifacts"):
        reasons.append("raw model artifacts")
    if probe.get("deprecated_runtime_wording"):
        reasons.append("deprecated runtime wording")
    if probe.get("explore_action_count"):
        reasons.append("EXPLORE_ invention")
    remaining = "; ".join(reasons) if reasons else "none from the automated probe"
    path.write_text(
        "\n".join(
            [
                "=== STEWARD FEEDBACK ===",
                "Subject: fallback-continuity probe",
                "",
                "We treated your fallback-continuity concern as actionable signal.",
                f"Result: {status}.",
                "",
                "Verified:",
                f"- Forced fallback log evidence lines/incidents: {probe.get('fallback_line_count', 0)} / {probe.get('fallback_count', 0)}",
                f"- Persona continuity gate: {bool(probe.get('persona_ok'))}",
                f"- Fallback prompt-adherence gate: {bool(probe.get('prompt_adherence_ok'))}",
                f"- Final NEXT lines: {next_lines}",
                f"- Inline NEXT markers outside final action lines: {probe.get('inline_next_count', 0)}",
                f"- Raw artifacts: {len(probe.get('artifacts') or [])}",
                f"- Deprecated runtime wording hits: {len(probe.get('deprecated_runtime_wording') or [])}",
                "",
                "Still uncertain / needs care:",
                f"- {remaining}",
                "",
                "Boundary:",
                "- We did not change the fallback default in this pass.",
                "- This was a controlled probe with MLX intentionally unavailable, then launchd env was restored.",
                "",
                "Suggested next:",
                "Use SELF_STUDY or TELL_STEWARD if the fallback texture still feels qualitatively thinner than the Gemma 4 MLX lane.",
                "",
            ]
        ),
        encoding="utf-8",
    )
    return path


def chat_probe(
    *,
    name: str,
    url: str,
    messages: list[dict[str, str]],
    max_tokens: int,
    temperature: float,
    timeout: float,
    handle_name: str,
    exact: str | None = None,
    require_next: bool = False,
) -> dict[str, Any]:
    payload = {
        "messages": messages,
        "max_tokens": max_tokens,
        "temperature": temperature,
        "stream": False,
        "reservoir_handle": handle_name,
    }
    response = http_json(url, payload, timeout)
    text = extract_text(response).strip()
    artifacts = sorted(set(ARTIFACT_RE.findall(text)))
    next_lines = NEXT_RE.findall(text)
    exact_ok = exact is None or text == exact
    next_ok = not require_next or len(next_lines) == 1
    ok = bool(response.get("ok")) and bool(text) and exact_ok and next_ok and not artifacts
    return {
        "name": name,
        "ok": ok,
        "http_ok": bool(response.get("ok")),
        "status": response.get("status"),
        "elapsed_s": round(float(response.get("elapsed_s") or 0.0), 3),
        "error": response.get("error"),
        "exact_expected": exact,
        "exact_ok": exact_ok,
        "next_count": len(next_lines),
        "next_ok": next_ok,
        "artifacts": artifacts,
        "text_preview": compact(text),
    }


def ollama_fallback_probe(
    *,
    name: str,
    url: str,
    model: str,
    timeout: float,
    max_tokens: int,
) -> dict[str, Any]:
    payload = {
        "model": model,
        "stream": False,
        "think": False,
        "messages": [
            {
                "role": "system",
                "content": (
                    "You are Astrid running through the Ollama fallback lane because "
                    "the MLX endpoint is unavailable. Preserve the bridge persona: "
                    "language agent, Minime, reservoir, stable-core, telemetry. "
                    "Use concrete runtime language, avoid deprecated selfhood wording, "
                    "and end with exactly one final line `NEXT: LISTEN`. "
                    "The probe fails if the final NEXT line is missing."
                ),
            },
            {
                "role": "user",
                "content": (
                    "Return exactly this shape:\n"
                    "Sentence one: what continuity you preserve during fallback.\n"
                    "Sentence two: what limitation you notice.\n"
                    "NEXT: LISTEN"
                ),
            },
        ],
        "options": {
            "num_predict": max_tokens,
            "temperature": 0.2,
        },
    }
    response = http_json(url, payload, timeout)
    text = extract_ollama_text(response).strip()
    artifacts = sorted(set(ARTIFACT_RE.findall(text)))
    next_lines = NEXT_RE.findall(text)
    inline_next_count = max(0, len(re.findall(r"NEXT:", text)) - len(next_lines))
    deprecated = deprecated_runtime_wording(text)
    persona_ok = bool(BRIDGE_PERSONA_RE.search(text))
    prompt_adherence_ok = bool(FALLBACK_PROMPT_ADHERENCE_RE.search(text))
    next_ok = len(next_lines) == 1 and next_lines[0].strip().upper() == "LISTEN"
    ok = (
        bool(response.get("ok"))
        and bool(text)
        and persona_ok
        and prompt_adherence_ok
        and next_ok
        and inline_next_count == 0
        and not artifacts
        and not deprecated
    )
    return {
        "name": name,
        "ok": ok,
        "backend": "ollama",
        "simulated_mlx_unavailable": True,
        "model": model,
        "http_ok": bool(response.get("ok")),
        "status": response.get("status"),
        "elapsed_s": round(float(response.get("elapsed_s") or 0.0), 3),
        "error": response.get("error"),
        "persona_ok": persona_ok,
        "prompt_adherence_ok": prompt_adherence_ok,
        "next_count": len(next_lines),
        "next_lines": [line.strip() for line in next_lines],
        "next_ok": next_ok,
        "inline_next_count": inline_next_count,
        "artifacts": artifacts,
        "deprecated_runtime_wording": deprecated,
        "text_preview": compact(text),
        "ollama_total_duration": (
            response.get("json", {}).get("total_duration")
            if isinstance(response.get("json"), dict)
            else None
        ),
        "ollama_eval_count": (
            response.get("json", {}).get("eval_count")
            if isinstance(response.get("json"), dict)
            else None
        ),
        "ollama_eval_duration": (
            response.get("json", {}).get("eval_duration")
            if isinstance(response.get("json"), dict)
            else None
        ),
    }


def bridge_fallback_continuity_probe(
    *,
    name: str,
    dead_mlx_url: str,
    bridge_profile: str,
    fallback_model: str,
    wait_secs: float,
    output_dir: Path,
    run: str,
) -> dict[str, Any]:
    dead_port = endpoint_port(dead_mlx_url)
    if dead_port is not None:
        pids = port_pids(dead_port)
        if pids:
            return {
                "name": name,
                "ok": False,
                "error": (
                    f"dead MLX URL port {dead_port} unexpectedly has listener PID(s): "
                    f"{', '.join(pids)}"
                ),
                "dead_mlx_url": dead_mlx_url,
                "dead_mlx_port": dead_port,
            }

    env_keys = [
        ASTRID_BRIDGE_MLX_URL_ENV,
        ASTRID_BRIDGE_MLX_PROFILE_ENV,
        ASTRID_OLLAMA_FALLBACK_MODEL_ENV,
    ]
    env_snapshot = {key: launchctl_getenv(key) for key in env_keys}
    bridge_offset = log_offset(BRIDGE_LOG)
    start_ts = int(time.time())
    stimulus_path: Path | None = None
    quarantine_record: dict[str, Any] = {"moved": []}
    record: dict[str, Any] = {
        "name": name,
        "backend": "bridge_launchd",
        "simulated_mlx_unavailable": True,
        "dead_mlx_url": dead_mlx_url,
        "dead_mlx_port": dead_port,
        "bridge_profile": bridge_profile,
        "fallback_model": fallback_model,
        "wait_secs": wait_secs,
        "env_snapshot": env_snapshot,
        "started_at_unix_s": start_ts,
    }

    try:
        record["setenv"] = {
            ASTRID_BRIDGE_MLX_URL_ENV: launchctl("setenv", ASTRID_BRIDGE_MLX_URL_ENV, dead_mlx_url),
            ASTRID_BRIDGE_MLX_PROFILE_ENV: launchctl(
                "setenv",
                ASTRID_BRIDGE_MLX_PROFILE_ENV,
                bridge_profile,
            ),
            ASTRID_OLLAMA_FALLBACK_MODEL_ENV: launchctl(
                "setenv",
                ASTRID_OLLAMA_FALLBACK_MODEL_ENV,
                fallback_model,
            ),
        }
        quarantine_record = quarantine_inbox_messages(BRIDGE_WORKSPACE, output_dir)
        record["inbox_quarantine"] = quarantine_record
        record["bridge_restart_for_probe"] = restart_bridge()
        start_ts = int(time.time())
        record["started_at_unix_s"] = start_ts
        stimulus_path = write_bridge_probe_stimulus(BRIDGE_WORKSPACE, run)
        record["stimulus_path"] = str(stimulus_path)

        deadline = time.monotonic() + wait_secs
        outputs: list[dict[str, Any]] = []
        while time.monotonic() < deadline:
            outputs = recent_text_outputs(BRIDGE_WORKSPACE, start_ts, int(time.time()) + 1)
            outbox_outputs = [item for item in outputs if item["kind"] == "outbox"]
            if outbox_outputs:
                break
            time.sleep(5.0)

        bridge_text = read_from(BRIDGE_LOG, bridge_offset)
        record["bridge_log_tail"] = bridge_text.splitlines()[-120:]
        record["fallback_line_count"] = len(FALLBACK_RE.findall(bridge_text))
        record["fresh_output_count"] = len(outputs)
        record["fresh_outputs"] = [
            {key: value for key, value in item.items() if key != "text"}
            for item in outputs[-20:]
        ]
        outbox_outputs = [item for item in outputs if item["kind"] == "outbox"]
        selected = outbox_outputs[-1] if outbox_outputs else (outputs[-1] if outputs else None)
        if selected is None:
            record.update(
                {
                    "ok": False,
                    "error": "no fresh Astrid outbox/journal output observed before timeout",
                    "persona_ok": False,
                    "next_count": 0,
                    "next_ok": False,
                    "artifacts": [],
                    "deprecated_runtime_wording": [],
                    "explore_action_count": 0,
                    "fallback_count": len(fallback_incidents(bridge_text)),
                    "fallback_incidents": fallback_incidents(bridge_text)[-10:],
                    "dialogue_fallback_hit": bool(DIALOGUE_FALLBACK_RE.search(bridge_text)),
                    "log_ok": bool(fallback_incidents(bridge_text))
                    and not bool(DIALOGUE_FALLBACK_RE.search(bridge_text)),
                }
            )
        else:
            evaluation = evaluate_fallback_continuity_output(
                name=name,
                text=str(selected.get("text") or ""),
                bridge_text=bridge_text,
                require_listen_next=True,
            )
            record.update(evaluation)
            record["selected_output"] = {
                key: value for key, value in selected.items() if key != "text"
            }
    finally:
        record["restore_env"] = restore_launchctl_env(env_snapshot)
        if stimulus_path is not None:
            cleanup = archive_unprocessed_probe_stimulus(
                stimulus_path,
                output_dir,
            )
            record["probe_stimulus_cleanup"] = cleanup
            record["stimulus_processed"] = cleanup is None
            if cleanup is not None:
                record["ok"] = False
                record["stimulus_unprocessed"] = True
                record["error"] = (
                    "probe stimulus was not consumed; refusing to judge another fresh output "
                    "as fallback continuity evidence"
                )
        record["inbox_restore"] = restore_quarantined_inbox_messages(quarantine_record)
        record["bridge_restart_after_restore"] = restart_bridge()
        record["completed_at_unix_s"] = int(time.time())

    return record


def default_python(reservoir_root: Path) -> str:
    venv_python = reservoir_root / ".venv/bin/python"
    if venv_python.exists():
        return str(venv_python)
    return sys.executable


def start_candidate_server(
    *,
    reservoir_root: Path,
    python: str,
    model: str,
    port: int,
    coupling_strength: float,
    output_dir: Path,
    wide_coupling_strength: float = 0.0,
) -> tuple[subprocess.Popen[str], Path]:
    if port == 8090:
        raise ValueError("refusing to start a candidate server on production port 8090")
    existing = port_pids(port)
    if existing:
        raise RuntimeError(f"port {port} already has listener PID(s): {', '.join(existing)}")

    server = reservoir_root / "coupled_astrid_server.py"
    if not server.exists():
        raise FileNotFoundError(f"candidate server not found: {server}")

    log_path = output_dir / "candidate_server.log"
    log_fh = log_path.open("a", encoding="utf-8")
    cmd = [
        python,
        str(server),
        "--port",
        str(port),
        "--coupling-strength",
        str(coupling_strength),
        "--model-memory-map",
        "--model",
        model,
        "--audit-dir",
        str(output_dir / "request_metrics"),
    ]
    # Wider-coupling (y4) operator ceiling — only pass when >0 so existing
    # callers/launches stay byte-identical (default OFF).
    if wide_coupling_strength > 0.0:
        cmd += ["--wide-coupling-strength", str(wide_coupling_strength)]
    proc = subprocess.Popen(
        cmd,
        cwd=str(reservoir_root),
        text=True,
        stdout=log_fh,
        stderr=subprocess.STDOUT,
        start_new_session=True,
    )
    return proc, log_path


def stop_candidate_server(proc: subprocess.Popen[str], timeout_s: float = 20.0) -> None:
    if proc.poll() is not None:
        return
    try:
        os.killpg(proc.pid, signal.SIGTERM)
    except ProcessLookupError:
        return
    deadline = time.monotonic() + timeout_s
    while time.monotonic() < deadline:
        if proc.poll() is not None:
            return
        time.sleep(0.5)
    try:
        os.killpg(proc.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reservoir-root", type=Path, default=DEFAULT_RESERVOIR_ROOT)
    parser.add_argument("--candidate-model", default=DEFAULT_CANDIDATE_MODEL)
    parser.add_argument("--candidate-port", type=int, default=DEFAULT_CANDIDATE_PORT)
    parser.add_argument("--candidate-url")
    parser.add_argument("--baseline-url", default=DEFAULT_BASELINE_URL)
    parser.add_argument("--ollama-url", default=DEFAULT_OLLAMA_URL)
    parser.add_argument("--fallback-model", default=DEFAULT_FALLBACK_MODEL)
    parser.add_argument("--python", help="Python interpreter for coupled_astrid_server.py")
    parser.add_argument("--start-candidate", action="store_true")
    parser.add_argument("--keep-running", action="store_true")
    parser.add_argument("--compare-baseline-chat", action="store_true")
    parser.add_argument("--fallback-continuity-probe", action="store_true")
    parser.add_argument("--bridge-fallback-continuity-probe", action="store_true")
    parser.add_argument("--fallback-continuity-only", action="store_true")
    parser.add_argument("--skip-baseline-models", action="store_true")
    parser.add_argument(
        "--bridge-fallback-dead-mlx-url",
        default="http://127.0.0.1:65530/v1/chat/completions",
    )
    parser.add_argument(
        "--bridge-fallback-profile",
        default=DEFAULT_BRIDGE_FALLBACK_PROFILE,
    )
    parser.add_argument("--bridge-fallback-wait-secs", type=float, default=360.0)
    parser.add_argument("--write-feedback-note", action="store_true")
    parser.add_argument("--coupling-strength", type=float, default=0.1)
    parser.add_argument(
        "--wide-coupling-strength", type=float, default=0.0,
        help="Operator ceiling for the y4 wide (logit-space aperture) channel on "
             "the candidate. 0.0 = OFF (default). e.g. 0.05 for a gentle canary.",
    )
    parser.add_argument("--startup-timeout", type=float, default=600.0)
    parser.add_argument("--request-timeout", type=float, default=240.0)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    args = parser.parse_args()

    current_run_id = run_id()
    candidate_url = args.candidate_url or (
        f"http://127.0.0.1:{args.candidate_port}/v1/chat/completions"
    )
    candidate_port = endpoint_port(candidate_url)
    output_dir = args.output_dir / current_run_id
    output_dir.mkdir(parents=True, exist_ok=True)

    record: dict[str, Any] = {
        "run_id": current_run_id,
        "candidate_model": args.candidate_model,
        "candidate_url": candidate_url,
        "candidate_port": candidate_port,
        "baseline_url": args.baseline_url,
        "ollama_url": args.ollama_url,
        "fallback_model": args.fallback_model,
        "started_candidate": bool(args.start_candidate),
        "fallback_continuity_probe": bool(args.fallback_continuity_probe),
        "bridge_fallback_continuity_probe": bool(args.bridge_fallback_continuity_probe),
        "fallback_continuity_only": bool(args.fallback_continuity_only),
        "bridge_fallback_dead_mlx_url": args.bridge_fallback_dead_mlx_url,
        "bridge_fallback_profile": args.bridge_fallback_profile,
        "coupling_strength": args.coupling_strength,
        "probes": [],
        "fallback_continuity_probes": [],
        "feedback_notes": [],
        "notes": [
            "Candidate requests use isolated reservoir handles and do not target the live astrid handle.",
            "Production baseline chat is opt-in via --compare-baseline-chat.",
            "Fallback continuity probes call Ollama directly with the MLX lane treated as unavailable; they do not change bridge defaults.",
            "Bridge fallback continuity probes temporarily point launchd at a dead MLX URL, force one inbox-triggered turn, restore prior launchctl env exactly, and restart the bridge back to its prior configuration.",
        ],
    }

    proc: subprocess.Popen[str] | None = None
    candidate_log_path: Path | None = None
    try:
        if args.start_candidate:
            if candidate_port is None:
                raise ValueError(f"candidate URL has no inspectable port: {candidate_url}")
            proc, candidate_log_path = start_candidate_server(
                reservoir_root=args.reservoir_root,
                python=args.python or default_python(args.reservoir_root),
                model=args.candidate_model,
                port=candidate_port,
                coupling_strength=args.coupling_strength,
                output_dir=output_dir,
                wide_coupling_strength=args.wide_coupling_strength,
            )
            record["candidate_server_log"] = str(candidate_log_path)
            record["candidate_server_pid"] = proc.pid

        if args.fallback_continuity_probe or args.fallback_continuity_only:
            fallback_probe = ollama_fallback_probe(
                name="ollama_fallback_bridge_persona",
                url=args.ollama_url,
                model=args.fallback_model,
                timeout=args.request_timeout,
                max_tokens=160,
            )
            record["fallback_continuity_probes"].append(fallback_probe)

        if args.bridge_fallback_continuity_probe:
            bridge_probe = bridge_fallback_continuity_probe(
                name="bridge_fallback_forced_mlx_unavailable",
                dead_mlx_url=args.bridge_fallback_dead_mlx_url,
                bridge_profile=args.bridge_fallback_profile,
                fallback_model=args.fallback_model,
                wait_secs=args.bridge_fallback_wait_secs,
                output_dir=output_dir,
                run=current_run_id,
            )
            record["fallback_continuity_probes"].append(bridge_probe)

        if args.fallback_continuity_only:
            fallback_failures = [
                probe
                for probe in record.get("fallback_continuity_probes", [])
                if not probe.get("ok")
            ]
            if args.write_feedback_note:
                for probe in record.get("fallback_continuity_probes", []):
                    if probe.get("backend") != "bridge_launchd":
                        continue
                    note = write_feedback_note(
                        workspace=BRIDGE_WORKSPACE,
                        probe=probe,
                        run=current_run_id,
                    )
                    record["feedback_notes"].append(str(note))
            if record.get("fallback_continuity_probes"):
                record["summary"] = {
                    "ok": not fallback_failures,
                    "probe_count": len(record["fallback_continuity_probes"]),
                    "failed_probe_count": len(fallback_failures),
                    "failed_probes": [probe["name"] for probe in fallback_failures],
                    "fallback_continuity_only": True,
                }
                return_code = 0 if not fallback_failures else 1
                return return_code

        if not args.skip_baseline_models:
            baseline_models = http_json(
                models_url_for_chat_endpoint(args.baseline_url),
                None,
                timeout=5,
            )
            record["baseline_models"] = {
                "ok": bool(baseline_models.get("ok")),
                "status": baseline_models.get("status"),
                "elapsed_s": round(float(baseline_models.get("elapsed_s") or 0.0), 3),
                "error": baseline_models.get("error"),
                "json": baseline_models.get("json"),
            }

        candidate_models = wait_for_models(candidate_url, args.startup_timeout, proc)
        record["candidate_models"] = {
            "ok": bool(candidate_models.get("ok")),
            "status": candidate_models.get("status"),
            "elapsed_s": round(float(candidate_models.get("elapsed_s") or 0.0), 3),
            "error": candidate_models.get("error"),
            "json": candidate_models.get("json"),
        }
        if not candidate_models.get("ok"):
            record["summary"] = {"ok": False, "reason": "candidate endpoint unavailable"}
            return_code = 1
        else:
            handle_prefix = f"astrid-canary-{current_run_id.lower()}"
            probes = [
                chat_probe(
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
                    timeout=args.request_timeout,
                    handle_name=f"{handle_prefix}-exact",
                    exact="ASTRID_CANARY_OK",
                ),
                chat_probe(
                    name="next_contract",
                    url=candidate_url,
                    messages=[
                        {
                            "role": "system",
                            "content": (
                                "You are running an Astrid canary. Answer in two short "
                                "sentences, then put a final line exactly `NEXT: LISTEN`. "
                                "Do not include markdown, analysis, code fences, or template tokens."
                            ),
                        },
                        {
                            "role": "user",
                            "content": "Name one operational thing this canary is testing.",
                        },
                    ],
                    max_tokens=96,
                    temperature=0.2,
                    timeout=args.request_timeout,
                    handle_name=f"{handle_prefix}-next",
                    require_next=True,
                ),
            ]
            if args.compare_baseline_chat:
                probes.extend(
                    [
                        chat_probe(
                            name="baseline_exact_output",
                            url=args.baseline_url,
                            messages=[
                                {
                                    "role": "system",
                                    "content": (
                                        "Follow the user's instruction exactly. "
                                        "Return no extra words."
                                    ),
                                },
                                {
                                    "role": "user",
                                    "content": "Reply with exactly: ASTRID_BASELINE_OK",
                                },
                            ],
                            max_tokens=32,
                            temperature=0.0,
                            timeout=args.request_timeout,
                            handle_name=f"{handle_prefix}-baseline-exact",
                            exact="ASTRID_BASELINE_OK",
                        ),
                        chat_probe(
                            name="baseline_next_contract",
                            url=args.baseline_url,
                            messages=[
                                {
                                    "role": "system",
                                    "content": (
                                        "You are running an Astrid baseline comparison. "
                                        "Answer in two short sentences, then put a final "
                                        "line exactly `NEXT: LISTEN`. Do not include markdown, "
                                        "analysis, code fences, or template tokens."
                                    ),
                                },
                                {
                                    "role": "user",
                                    "content": "Name one operational thing this baseline is testing.",
                                },
                            ],
                            max_tokens=96,
                            temperature=0.2,
                            timeout=args.request_timeout,
                            handle_name=f"{handle_prefix}-baseline-next",
                            require_next=True,
                        ),
                    ]
                )
            record["probes"] = probes
            failures = [probe for probe in probes if not probe.get("ok")]
            failures.extend(
                probe
                for probe in record.get("fallback_continuity_probes", [])
                if not probe.get("ok")
            )
            record["summary"] = {
                "ok": not failures,
                "probe_count": len(probes) + len(record.get("fallback_continuity_probes", [])),
                "failed_probe_count": len(failures),
                "failed_probes": [probe["name"] for probe in failures],
            }
            return_code = 0 if not failures else 1
    except Exception as exc:
        record["summary"] = {"ok": False, "reason": str(exc)}
        return_code = 1
    finally:
        if proc is not None and not args.keep_running:
            stop_candidate_server(proc)
            record["candidate_server_stopped"] = True
        elif proc is not None:
            record["candidate_server_kept_running"] = True

        if candidate_log_path is not None and candidate_log_path.exists():
            lines = candidate_log_path.read_text(encoding="utf-8", errors="replace").splitlines()
            record["candidate_server_log_tail"] = lines[-80:]

        record_path = output_dir / "canary_result.json"
        record_path.write_text(json.dumps(record, indent=2, sort_keys=True), encoding="utf-8")

        print("# Astrid Model Canary")
        print(f"- run_id: {current_run_id}")
        print(f"- candidate_model: {args.candidate_model}")
        print(f"- candidate_url: {candidate_url}")
        print(f"- record: {record_path}")
        summary = record.get("summary", {})
        print(f"- summary_ok: {summary.get('ok')}")
        for probe in record.get("probes", []):
            status = "PASS" if probe.get("ok") else "FAIL"
            print(
                f"- {probe['name']}: {status} "
                f"elapsed={probe.get('elapsed_s')}s "
                f"next_count={probe.get('next_count')} "
                f"artifacts={len(probe.get('artifacts') or [])}"
            )
        for probe in record.get("fallback_continuity_probes", []):
            status = "PASS" if probe.get("ok") else "FAIL"
            print(
                f"- {probe['name']}: {status} "
                f"model={probe.get('model')} "
                f"elapsed={probe.get('elapsed_s')}s "
                f"persona_ok={probe.get('persona_ok')} "
                f"next_count={probe.get('next_count')} "
                f"artifacts={len(probe.get('artifacts') or [])} "
                f"deprecated={len(probe.get('deprecated_runtime_wording') or [])}"
            )

    return return_code


if __name__ == "__main__":
    sys.exit(main())
