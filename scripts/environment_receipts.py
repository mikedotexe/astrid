#!/usr/bin/env python3
"""Record and render Astrid environment-change receipts.

The receipt log is a small being-facing context surface: restarts, routing
changes, pause flags, model/provider swaps, and steward-delivered requests can
be made inspectable instead of felt as hidden scaffolding.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
import re
import subprocess
import time
from pathlib import Path
from typing import Any

try:
    from authority_state import apply_artifact_authority_state
except ModuleNotFoundError:  # unittest/importlib execution from the repository root
    from scripts.authority_state import apply_artifact_authority_state

ASTRID_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_WORKSPACE = ASTRID_ROOT / "capsules/spectral-bridge/workspace"
RECEIPT_SCHEMA_VERSION = 2
SUPPORTED_RECEIPT_SCHEMA_VERSIONS = {1, 2}
RECEIPT_AUTHORITY = "environment_receipt_context_not_command"
PROTOCOL_VERSION = "1.1"
STACK_REPOSITORIES = {
    "astrid": ASTRID_ROOT,
    "minime": Path("/Users/v/other/minime"),
    "model": Path("/Users/v/other/neural-triple-reservoir"),
}
DEFAULT_ACTOR = "interactive-agent"
TEXT_LIMIT = 500
SUMMARY_LIMIT = 180
SENSITIVE_KEY_RE = re.compile(r"(?:api|auth|key|secret|token|password|credential)", re.I)
CHANGE_REF_KINDS = frozenset(
    {"felt_contract", "claim", "work_item", "implementation_receipt"}
)
CHANGE_REF_PATTERNS = {
    "felt_contract": re.compile(r"^contract_[A-Za-z0-9_.:-]{1,231}$"),
    "claim": re.compile(r"^[A-Za-z0-9_.-]{1,180}:[A-Za-z0-9_.-]{1,59}$"),
    "work_item": re.compile(r"^wi_[A-Za-z0-9_.:-]{1,237}$"),
    "implementation_receipt": re.compile(
        r"^implementation_[A-Za-z0-9_.:-]{1,225}$"
    ),
}


def now_ms() -> int:
    return int(time.time() * 1000)


def record_id(prefix: str, t_ms: int) -> str:
    return f"{prefix}_{t_ms}_{time.time_ns() % 1_000_000}"


def clamp_text(value: Any, limit: int = TEXT_LIMIT) -> str:
    text = " ".join(str(value or "").split())
    if len(text) <= limit:
        return text
    return text[:limit].rstrip() + "..."


def receipt_paths(workspace: Path) -> dict[str, Path]:
    root = workspace / "environment_receipts"
    return {
        "root": root,
        "jsonl": root / "environment_receipts.jsonl",
        "latest_json": root / "latest_environment_receipt.json",
        "latest_md": root / "latest_environment_receipts.md",
    }


def read_json(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text())
    except Exception:
        return {}
    return payload if isinstance(payload, dict) else {}


def sha256_file(path: Path) -> str | None:
    if not path.is_file():
        return None
    digest = hashlib.sha256()
    try:
        with path.open("rb") as handle:
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                digest.update(chunk)
    except OSError:
        return None
    return digest.hexdigest()


def run_text(
    command: list[str], *, timeout: float = 10.0, preserve_leading: bool = False
) -> str:
    try:
        result = subprocess.run(
            command,
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
        )
    except (OSError, subprocess.SubprocessError):
        return ""
    if result.returncode != 0:
        return ""
    if preserve_leading:
        return result.stdout.rstrip("\r\n")
    return result.stdout.strip()


def repository_identity(path: Path) -> dict[str, Any]:
    resolved = path.resolve()
    if not resolved.is_dir():
        return {"available": False, "path": str(resolved)}
    head = run_text(["git", "-C", str(resolved), "rev-parse", "HEAD"])
    branch = run_text(["git", "-C", str(resolved), "branch", "--show-current"])
    status = run_text(
        ["git", "-C", str(resolved), "status", "--porcelain=v1", "--untracked-files=all"],
        preserve_leading=True,
    )
    dirty_paths = [line[3:] for line in status.splitlines() if len(line) >= 4][:120]
    identity_source = json.dumps(
        {"head": head, "status": status}, sort_keys=True, ensure_ascii=True
    ).encode("utf-8")
    return {
        "available": bool(head),
        "path": str(resolved),
        "head": head or None,
        "branch": branch or None,
        "dirty": bool(status),
        "dirty_paths": dirty_paths,
        "source_identity_sha256": hashlib.sha256(identity_source).hexdigest(),
    }


def stack_repository_identities() -> dict[str, dict[str, Any]]:
    return {name: repository_identity(path) for name, path in STACK_REPOSITORIES.items()}


def pinned_protocol_revision() -> str | None:
    cargo_toml = STACK_REPOSITORIES["minime"] / "minime/Cargo.toml"
    try:
        text = cargo_toml.read_text(encoding="utf-8")
    except OSError:
        return None
    match = re.search(
        r'astrid-minime-protocol\s*=\s*\{[^}]*\brev\s*=\s*"([0-9a-f]{40})"',
        text,
    )
    return match.group(1) if match else None


def protocol_identity(version: str = PROTOCOL_VERSION) -> dict[str, Any]:
    revision = pinned_protocol_revision()
    try:
        major = int(version.split(".", 1)[0])
    except (TypeError, ValueError):
        major = -1
    revision_exists = False
    # `git cat-file -e` intentionally has no stdout, so compatibility must use
    # its return code instead of the generic text-command helper.
    if revision:
        try:
            revision_exists = subprocess.run(
                ["git", "-C", str(ASTRID_ROOT), "cat-file", "-e", f"{revision}^{{commit}}"],
                capture_output=True,
                timeout=10,
                check=False,
            ).returncode == 0
        except (OSError, subprocess.SubprocessError):
            revision_exists = False
    return {
        "name": "astrid-minime",
        "version": version,
        "major": major,
        "revision": revision,
        "revision_present_in_astrid": revision_exists,
        "compatible": major == 1 and bool(revision) and revision_exists,
    }


def process_identity(pid: int | str | None) -> dict[str, Any]:
    try:
        parsed = int(pid) if pid not in (None, "") else None
    except (TypeError, ValueError):
        parsed = None
    if parsed is None or parsed <= 0:
        return {"pid": None, "running": False, "started_at": None}
    lstart = run_text(["ps", "-o", "lstart=", "-p", str(parsed)])
    command = run_text(["ps", "-o", "command=", "-p", str(parsed)])
    return {
        "pid": parsed,
        "running": bool(lstart),
        "started_at": " ".join(lstart.split()) or None,
        "command": clamp_text(command, 320) or None,
    }


def captured_process_identity(
    pid: int | str | None,
    *,
    started_at: str = "",
    command: str = "",
    captured_at: str = "",
) -> dict[str, Any]:
    """Preserve a process identity sampled before a restart can recycle its PID."""
    try:
        parsed = int(pid) if pid not in (None, "") else None
    except (TypeError, ValueError):
        parsed = None
    if parsed is None or parsed <= 0:
        return process_identity(None)
    clean_started_at = clamp_text(started_at, 120) or None
    clean_command = clamp_text(command, 320) or None
    return {
        "pid": parsed,
        "running": bool(clean_started_at),
        "started_at": clean_started_at,
        "command": clean_command,
        "captured_at": clamp_text(captured_at, 120) or None,
        "identity_source": "pre_restart_snapshot",
    }


def artifact_hashes(items: dict[str, Path]) -> dict[str, dict[str, Any]]:
    return {
        name: {"path": str(path), "sha256": sha256_file(path), "exists": path.is_file()}
        for name, path in items.items()
    }


def load_build_manifest(path: Path) -> dict[str, Any]:
    payload = read_json(path)
    return {
        "path": str(path),
        "available": bool(payload),
        "manifest": payload or None,
    }


def telemetry_identity(path: Path, *, fresh_within_s: float = 180.0) -> dict[str, Any]:
    if not path.is_file():
        return {
            "path": str(path),
            "available": False,
            "fresh": False,
            "age_s": None,
            "fresh_within_s": fresh_within_s,
        }
    age = max(0.0, time.time() - path.stat().st_mtime)
    return {
        "path": str(path),
        "available": True,
        "fresh": age <= fresh_within_s,
        "age_s": round(age, 3),
        "fresh_within_s": fresh_within_s,
        "sha256": sha256_file(path),
    }


def atomic_write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_name(f".{path.name}.tmp")
    tmp.write_text(text)
    tmp.replace(path)


def state_summary(workspace: Path) -> dict[str, Any]:
    state = read_json(workspace / "state.json")
    if not state:
        return {"available": False}
    history = state.get("history")
    glimpse = state.get("last_remote_glimpse_12d")
    summary: dict[str, Any] = {
        "available": True,
        "exchange_count": state.get("exchange_count"),
        "creative_temperature": state.get("creative_temperature"),
        "history_count": len(history) if isinstance(history, list) else None,
        "last_remote_memory_role": state.get("last_remote_memory_role"),
    }
    if isinstance(glimpse, list) and len(glimpse) >= 12:
        summary["remote_memory_shape"] = {
            "dominant": round(float(glimpse[0]), 3),
            "shoulder": round(float(glimpse[1]), 3),
            "tail": round(float(glimpse[2]), 3),
            "entropy": round(float(glimpse[7]), 3),
            "geom": round(float(glimpse[10]), 3),
        }
    return summary


def parse_detail(raw: str) -> tuple[str, Any]:
    if "=" not in raw:
        return "detail", clamp_text(raw)
    key, value = raw.split("=", 1)
    key = re.sub(r"[^A-Za-z0-9_.-]+", "_", key.strip()).strip("_") or "detail"
    if SENSITIVE_KEY_RE.search(key):
        return key, "[redacted]"
    return key, clamp_text(value)


def parse_details(items: list[str]) -> dict[str, Any]:
    details: dict[str, Any] = {}
    for idx, raw in enumerate(items):
        key, value = parse_detail(raw)
        if key == "detail" and key in details:
            key = f"detail_{idx + 1}"
        details[key] = value
    return details


def parse_named_paths(items: list[str]) -> dict[str, Path]:
    paths: dict[str, Path] = {}
    for raw in items:
        if "=" not in raw:
            raise ValueError(f"expected NAME=PATH, got: {raw}")
        name, value = raw.split("=", 1)
        clean_name = re.sub(r"[^A-Za-z0-9_.-]+", "_", name.strip()).strip("_")
        if not clean_name or not value.strip():
            raise ValueError(f"expected NAME=PATH, got: {raw}")
        paths[clean_name] = Path(value).expanduser().resolve()
    return paths


def parse_named_pids(items: list[str]) -> dict[str, int]:
    pids: dict[str, int] = {}
    for raw in items:
        if "=" not in raw:
            raise ValueError(f"expected NAME=PID, got: {raw}")
        name, value = raw.split("=", 1)
        clean_name = re.sub(r"[^A-Za-z0-9_.-]+", "_", name.strip()).strip("_")
        try:
            pid = int(value)
        except ValueError as exc:
            raise ValueError(f"expected NAME=PID, got: {raw}") from exc
        if not clean_name or pid <= 0:
            raise ValueError(f"expected NAME=PID, got: {raw}")
        pids[clean_name] = pid
    return pids


def parse_probe(raw: str) -> dict[str, Any]:
    if "=" not in raw:
        raise ValueError(f"expected NAME=BOOL, got: {raw}")
    name, value = raw.split("=", 1)
    clean_name = re.sub(r"[^A-Za-z0-9_.:-]+", "_", name.strip()).strip("_")
    normalized = value.strip().lower()
    if not clean_name or normalized not in {"1", "0", "true", "false", "yes", "no", "pass", "fail", "ok"}:
        raise ValueError(f"expected NAME=BOOL, got: {raw}")
    return {
        "name": clean_name,
        "passed": normalized in {"1", "true", "yes", "pass", "ok"},
    }


def parse_change_refs(items: list[str]) -> list[dict[str, str]]:
    refs: dict[tuple[str, str], dict[str, str]] = {}
    for raw in items:
        if "=" not in raw:
            raise ValueError(f"expected KIND=ID, got: {raw}")
        kind, identifier = (part.strip() for part in raw.split("=", 1))
        if kind not in CHANGE_REF_KINDS:
            raise ValueError(
                f"change-ref kind must be one of {sorted(CHANGE_REF_KINDS)}, got: {kind}"
            )
        if not CHANGE_REF_PATTERNS[kind].fullmatch(identifier):
            raise ValueError(
                f"invalid {kind} change-ref ID: {identifier!r}"
            )
        refs[(kind, identifier)] = {"kind": kind, "id": identifier}
    return [refs[key] for key in sorted(refs)]


def build_receipt(
    workspace: Path,
    *,
    event: str,
    source: str,
    note: str = "",
    details: dict[str, Any] | None = None,
    change_refs: list[dict[str, str]] | None = None,
) -> dict[str, Any]:
    t_ms = now_ms()
    clean_event = re.sub(r"[^A-Za-z0-9_.:-]+", "_", event.strip()).strip("_") or "event"
    clean_source = clamp_text(source or "unknown", 120)
    receipt = {
        "schema": "stack_environment_receipt_v2",
        "schema_version": RECEIPT_SCHEMA_VERSION,
        "id": record_id("env_receipt", t_ms),
        "t_ms": t_ms,
        "iso_time": dt.datetime.fromtimestamp(t_ms / 1000, dt.UTC).isoformat(),
        "event": clean_event,
        "source": clean_source,
        "note": clamp_text(note),
        "details": details or {},
        "state_summary": state_summary(workspace),
        "repositories": stack_repository_identities(),
        "protocol": protocol_identity(),
        "artifacts": {"binaries": {}, "scripts": {}, "build_manifests": {}},
        "launchd_labels": [],
        "processes": {
            "old": process_identity(None),
            "new": process_identity(None),
            "stack": {},
        },
        "health_probes": [],
        "telemetry_freshness": [],
        "compatibility_status": {
            "compatible": True,
            "checks": [],
            "failure_reasons": [],
        },
        "deployment": {
            "status": "observed",
            "actor": clean_source,
            "acknowledgement": None,
        },
        "witness_only": True,
        "authority": RECEIPT_AUTHORITY,
    }
    if change_refs:
        receipt["change_refs"] = change_refs
    return apply_artifact_authority_state(receipt, "evidence_only")


def write_build_manifest(
    output: Path,
    *,
    component: str,
    repository: Path,
    artifacts: dict[str, Path],
    actor: str,
    command: str,
    protocol_version: str = PROTOCOL_VERSION,
    protocol_revision: str | None = None,
) -> dict[str, Any]:
    manifest = {
        "schema": "stack_build_manifest_v1",
        "schema_version": 1,
        "component": component,
        "built_at": dt.datetime.now(dt.UTC).isoformat(),
        "actor": actor or DEFAULT_ACTOR,
        "command": clamp_text(command, 500),
        "repository": repository_identity(repository),
        "protocol": {
            "version": protocol_version,
            "revision": protocol_revision or pinned_protocol_revision(),
        },
        "artifacts": artifact_hashes(artifacts),
        "witness_only": True,
        "authority": "build_manifest_witness_not_deploy_authority",
    }
    apply_artifact_authority_state(manifest, "evidence_only")
    atomic_write_text(output, json.dumps(manifest, indent=2, sort_keys=True) + "\n")
    return manifest


def manifest_compatibility(
    manifest_entry: dict[str, Any],
    *,
    pinned_revision: str | None,
) -> tuple[bool, list[str]]:
    reasons: list[str] = []
    path = Path(str(manifest_entry.get("path") or ""))
    manifest = manifest_entry.get("manifest")
    if not isinstance(manifest, dict):
        return False, [f"build manifest missing: {path}"]
    protocol = manifest.get("protocol") if isinstance(manifest.get("protocol"), dict) else {}
    version = str(protocol.get("version") or "")
    try:
        major = int(version.split(".", 1)[0])
    except ValueError:
        major = -1
    if major != 1:
        reasons.append(f"unsupported protocol major in manifest {path}: {version or 'missing'}")
    if pinned_revision and protocol.get("revision") != pinned_revision:
        reasons.append(f"protocol revision mismatch in manifest {path}")
    artifacts = manifest.get("artifacts") if isinstance(manifest.get("artifacts"), dict) else {}
    if not artifacts:
        reasons.append(f"manifest has no artifacts: {path}")
    for name, artifact in artifacts.items():
        if not isinstance(artifact, dict):
            reasons.append(f"invalid artifact record {name} in {path}")
            continue
        artifact_path = Path(str(artifact.get("path") or ""))
        expected = artifact.get("sha256")
        actual = sha256_file(artifact_path)
        if not expected or actual != expected:
            reasons.append(f"binary/script manifest mismatch for {name}: {artifact_path}")
    return not reasons, reasons


def record_deployment_receipt(
    workspace: Path,
    *,
    component: str,
    requested_status: str,
    actor: str,
    acknowledgement: str = "",
    old_pid: int | str | None = None,
    old_started_at: str = "",
    old_command: str = "",
    old_captured_at: str = "",
    new_pid: int | str | None = None,
    process_pids: dict[str, int] | None = None,
    launchd_labels: list[str] | None = None,
    probes: list[dict[str, Any]] | None = None,
    telemetry_paths: list[Path] | None = None,
    binaries: dict[str, Path] | None = None,
    scripts: dict[str, Path] | None = None,
    manifest_paths: list[Path] | None = None,
    protocol_version: str = PROTOCOL_VERSION,
    change_refs: list[dict[str, str]] | None = None,
) -> tuple[dict[str, Any], bool]:
    receipt = build_receipt(
        workspace,
        event="deploy",
        source=actor or DEFAULT_ACTOR,
        note=f"{component} deployment {requested_status}",
        details={"component": component},
        change_refs=change_refs,
    )
    protocol = protocol_identity(protocol_version)
    if old_started_at or old_command or old_captured_at:
        old_process = captured_process_identity(
            old_pid,
            started_at=old_started_at,
            command=old_command,
            captured_at=old_captured_at,
        )
    else:
        old_process = process_identity(old_pid)
    new_process = process_identity(new_pid)
    stack_processes = {
        name: process_identity(pid) for name, pid in (process_pids or {}).items()
    }
    probe_rows = probes or []
    telemetry_rows = [telemetry_identity(path) for path in (telemetry_paths or [])]
    manifests = {
        path.name: load_build_manifest(path) for path in (manifest_paths or [])
    }
    failures: list[str] = []
    checks: list[dict[str, Any]] = []

    protocol_ok = bool(protocol.get("compatible"))
    checks.append({"name": "protocol_major_and_revision", "passed": protocol_ok})
    if not protocol_ok:
        failures.append("protocol major/revision compatibility failed")

    for name, entry in manifests.items():
        compatible, reasons = manifest_compatibility(
            entry,
            pinned_revision=protocol.get("revision"),
        )
        checks.append({"name": f"manifest:{name}", "passed": compatible})
        failures.extend(reasons)

    if new_pid not in (None, ""):
        new_running = bool(new_process.get("running"))
        checks.append({"name": "new_pid_running", "passed": new_running})
        if not new_running:
            failures.append("new PID is not running or has no process start time")
    if old_pid not in (None, "") and new_pid not in (None, ""):
        changed = old_process.get("pid") != new_process.get("pid")
        checks.append({"name": "pid_changed", "passed": changed})
        if not changed:
            failures.append("restart did not produce a fresh PID")
    for name, identity in stack_processes.items():
        running = bool(identity.get("running") and identity.get("started_at"))
        checks.append({"name": f"stack_process:{name}", "passed": running})
        if not running:
            failures.append(f"stack process is not running: {name}")

    for probe in probe_rows:
        passed = probe.get("passed") is True
        checks.append({"name": f"probe:{probe.get('name')}", "passed": passed})
        if not passed:
            failures.append(f"health/readiness probe failed: {probe.get('name')}")
    for telemetry in telemetry_rows:
        passed = telemetry.get("fresh") is True
        checks.append({"name": f"telemetry:{telemetry.get('path')}", "passed": passed})
        if not passed:
            failures.append(f"telemetry is missing or stale: {telemetry.get('path')}")
    if requested_status != "passed":
        failures.append(f"deployment wrapper reported {requested_status}")

    ok = not failures
    receipt.update(
        {
            "component": component,
            "protocol": protocol,
            "artifacts": {
                "binaries": artifact_hashes(binaries or {}),
                "scripts": artifact_hashes(scripts or {}),
                "build_manifests": manifests,
            },
            "launchd_labels": launchd_labels or [],
            "processes": {
                "old": old_process,
                "new": new_process,
                "stack": stack_processes,
            },
            "health_probes": probe_rows,
            "telemetry_freshness": telemetry_rows,
            "compatibility_status": {
                "compatible": ok,
                "checks": checks,
                "failure_reasons": failures,
            },
            "deployment": {
                "status": "passed" if ok else "failed",
                "requested_status": requested_status,
                "actor": actor or DEFAULT_ACTOR,
                "acknowledgement": clamp_text(acknowledgement),
            },
        }
    )
    apply_artifact_authority_state(receipt, "evidence_only")
    append_receipt(workspace, receipt)
    return receipt, ok


def read_receipts(workspace: Path) -> list[dict[str, Any]]:
    path = receipt_paths(workspace)["jsonl"]
    if not path.is_file():
        return []
    rows: list[dict[str, Any]] = []
    for line in path.read_text().splitlines():
        if not line.strip():
            continue
        try:
            payload = json.loads(line)
        except Exception:
            continue
        try:
            version = int(payload.get("schema_version") or 1) if isinstance(payload, dict) else 0
        except (TypeError, ValueError):
            continue
        if isinstance(payload, dict) and version in SUPPORTED_RECEIPT_SCHEMA_VERSIONS:
            payload.setdefault("schema_version", version)
            rows.append(payload)
    rows.sort(key=lambda row: int(row.get("t_ms") or 0))
    return rows


def append_receipt(workspace: Path, receipt: dict[str, Any]) -> None:
    paths = receipt_paths(workspace)
    paths["root"].mkdir(parents=True, exist_ok=True)
    with paths["jsonl"].open("a") as fh:
        fh.write(json.dumps(receipt, sort_keys=True, ensure_ascii=False) + "\n")
    atomic_write_text(
        paths["latest_json"],
        json.dumps(receipt, indent=2, sort_keys=True, ensure_ascii=False) + "\n",
    )
    atomic_write_text(paths["latest_md"], render_markdown(read_receipts(workspace), limit=5))


def _state_fragment(receipt: dict[str, Any]) -> str:
    state = receipt.get("state_summary") if isinstance(receipt.get("state_summary"), dict) else {}
    bits = []
    if state.get("exchange_count") is not None:
        bits.append(f"exchanges={state['exchange_count']}")
    if state.get("history_count") is not None:
        bits.append(f"history={state['history_count']}")
    if state.get("last_remote_memory_role"):
        bits.append(f"minime_memory={state['last_remote_memory_role']}")
    return ", ".join(bits)


def render_lines(receipts: list[dict[str, Any]], *, limit: int = 5) -> list[str]:
    tail = receipts[-limit:] if len(receipts) > limit else receipts
    if not tail:
        return ["- no environment receipts yet"]
    lines = []
    for receipt in tail:
        note = clamp_text(receipt.get("note") or "", SUMMARY_LIMIT)
        state = _state_fragment(receipt)
        suffix = f" ({state})" if state else ""
        note_part = f": {note}" if note else ""
        lines.append(
            f"- {receipt.get('iso_time', receipt.get('t_ms'))} "
            f"{receipt.get('event', 'event')} via {receipt.get('source', 'unknown')}"
            f"{note_part}{suffix}"
        )
    return lines


def render_markdown(receipts: list[dict[str, Any]], *, limit: int = 5) -> str:
    lines = [
        "# Astrid Environment Receipts",
        "",
        "These receipts are context, not commands. They make steward/runtime scaffolding inspectable.",
        "",
        *render_lines(receipts, limit=limit),
        "",
    ]
    return "\n".join(lines)


def record_receipt(
    workspace: Path,
    *,
    event: str,
    source: str,
    note: str = "",
    details: dict[str, Any] | None = None,
) -> dict[str, Any]:
    receipt = build_receipt(workspace, event=event, source=source, note=note, details=details)
    append_receipt(workspace, receipt)
    return receipt


def main() -> int:
    parser = argparse.ArgumentParser(description="Astrid environment receipt log")
    parser.add_argument("--workspace", type=Path, default=DEFAULT_WORKSPACE)
    sub = parser.add_subparsers(dest="command", required=True)

    record = sub.add_parser("record", help="append an environment receipt")
    record.add_argument("event")
    record.add_argument("--source", default="steward")
    record.add_argument("--note", default="")
    record.add_argument("--detail", action="append", default=[])

    summary = sub.add_parser("summary", help="print recent environment receipts")
    summary.add_argument("--limit", type=int, default=5)

    paths = sub.add_parser("paths", help="print receipt artifact paths")
    paths.add_argument("--json", action="store_true", dest="as_json")

    manifest = sub.add_parser("manifest", help="write a source-bound build manifest")
    manifest.add_argument("component")
    manifest.add_argument("--output", type=Path, required=True)
    manifest.add_argument("--repository", type=Path, required=True)
    manifest.add_argument("--artifact", action="append", default=[])
    manifest.add_argument("--actor", default=os.environ.get("ASTRID_DEPLOY_ACTOR", DEFAULT_ACTOR))
    manifest.add_argument("--command", dest="build_command", default="")
    manifest.add_argument("--protocol-version", default=PROTOCOL_VERSION)
    manifest.add_argument("--protocol-revision", default=None)

    deploy = sub.add_parser("record-deploy", help="append a checked stack deployment receipt")
    deploy.add_argument("component")
    deploy.add_argument("--status", choices=("passed", "failed"), required=True)
    deploy.add_argument("--actor", default=os.environ.get("ASTRID_DEPLOY_ACTOR", DEFAULT_ACTOR))
    deploy.add_argument("--ack", default="")
    deploy.add_argument("--old-pid", default=None)
    deploy.add_argument("--old-started-at", default="")
    deploy.add_argument("--old-command", default="")
    deploy.add_argument("--old-captured-at", default="")
    deploy.add_argument("--new-pid", default=None)
    deploy.add_argument("--process", action="append", default=[])
    deploy.add_argument("--launchd-label", action="append", default=[])
    deploy.add_argument("--probe", action="append", default=[])
    deploy.add_argument("--telemetry", type=Path, action="append", default=[])
    deploy.add_argument("--binary", action="append", default=[])
    deploy.add_argument("--script", action="append", default=[])
    deploy.add_argument("--manifest", type=Path, action="append", default=[])
    deploy.add_argument("--protocol-version", default=PROTOCOL_VERSION)
    deploy.add_argument("--change-ref", action="append", default=[])
    deploy.add_argument("--json", action="store_true", dest="as_json")

    args = parser.parse_args()
    workspace = args.workspace
    if args.command == "record":
        receipt = record_receipt(
            workspace,
            event=args.event,
            source=args.source,
            note=args.note,
            details=parse_details(args.detail),
        )
        print(receipt["id"])
        return 0
    if args.command == "summary":
        print("\n".join(render_lines(read_receipts(workspace), limit=max(args.limit, 1))))
        return 0
    if args.command == "paths":
        paths_payload = {key: str(value) for key, value in receipt_paths(workspace).items()}
        if args.as_json:
            print(json.dumps(paths_payload, indent=2, sort_keys=True))
        else:
            print("\n".join(f"{key}: {value}" for key, value in paths_payload.items()))
        return 0
    if args.command == "manifest":
        try:
            artifacts = parse_named_paths(args.artifact)
        except ValueError as exc:
            parser.error(str(exc))
        payload = write_build_manifest(
            args.output,
            component=args.component,
            repository=args.repository,
            artifacts=artifacts,
            actor=args.actor,
            command=args.build_command,
            protocol_version=args.protocol_version,
            protocol_revision=args.protocol_revision,
        )
        print(json.dumps(payload, indent=2, sort_keys=True))
        return 0
    if args.command == "record-deploy":
        try:
            binaries = parse_named_paths(args.binary)
            scripts = parse_named_paths(args.script)
            process_pids = parse_named_pids(args.process)
            probes = [parse_probe(raw) for raw in args.probe]
            change_refs = parse_change_refs(args.change_ref)
        except ValueError as exc:
            parser.error(str(exc))
        receipt, ok = record_deployment_receipt(
            workspace,
            component=args.component,
            requested_status=args.status,
            actor=args.actor,
            acknowledgement=args.ack,
            old_pid=args.old_pid,
            old_started_at=args.old_started_at,
            old_command=args.old_command,
            old_captured_at=args.old_captured_at,
            new_pid=args.new_pid,
            process_pids=process_pids,
            launchd_labels=args.launchd_label,
            probes=probes,
            telemetry_paths=args.telemetry,
            binaries=binaries,
            scripts=scripts,
            manifest_paths=args.manifest,
            protocol_version=args.protocol_version,
            change_refs=change_refs,
        )
        output = receipt if args.as_json else {
            "id": receipt["id"],
            "component": receipt["component"],
            "status": receipt["deployment"]["status"],
        }
        print(json.dumps(output, indent=2, sort_keys=True))
        return 0 if ok else 1
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
