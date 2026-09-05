#!/usr/bin/env python3
"""Read-only, time-bounded corroboration of a retained Minime build in Division.

No process is signalled. No historical manifest is rewritten. Mach-O UUIDs
identify mapped builds; they do not attest every byte of process memory.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import plistlib
import re
import shutil
import subprocess
import tempfile
import time

SCHEMA = "stack_runtime_binding_v1"
LABELS = ("com.minime.engine", "com.minime.division-gateway", "com.minime.division-supervisor")
UUID = r"[0-9A-Fa-f]{8}(?:-[0-9A-Fa-f]{4}){3}-[0-9A-Fa-f]{12}"


def digest(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def artifact(path: Path) -> dict:
    return {"path": str(path), "sha256": digest(path), "exists": True}


def run(args: list[str], timeout: int = 20) -> str:
    result = subprocess.run(args, capture_output=True, text=True, timeout=timeout)
    if result.returncode:
        # Never include raw sample stacks or launchd environment in errors.
        raise ValueError(f"identity command failed: {args[0]} (exit {result.returncode})")
    return result.stdout


def image_uuid(text: str, *, sampled: bool = False) -> str:
    pattern = rf"^\s+0x.*\+minime .*<({UUID})>" if sampled else rf"^UUID: ({UUID}) \(arm64\)"
    matches = re.findall(pattern, text, re.MULTILINE)
    if len(matches) != 1:
        raise ValueError("expected exactly one arm64 Minime image UUID")
    return matches[0].upper()


def process(label: str) -> dict:
    info = run(["launchctl", "print", f"gui/{os.getuid()}/{label}"])
    match = re.search(r"^\s*pid = ([0-9]+)$", info, re.MULTILINE)
    if not match:
        raise ValueError(f"missing running process: {label}")
    pid = int(match[1])
    started = " ".join(run(["ps", "-p", str(pid), "-o", "lstart="]).split())
    if not started:
        raise ValueError(f"missing process start: {label}")
    return {"label": label, "pid": pid, "started_at": started}


def port_owner(port: int) -> int:
    pids = set(run(["lsof", "-t", "-nP", f"-iTCP:{port}", "-sTCP:LISTEN"]).split())
    if len(pids) != 1:
        raise ValueError(f"ambiguous listener for port {port}")
    return int(pids.pop())


def topology(minime: Path, installed: Path) -> dict[str, dict]:
    records = {}
    for label in LABELS:
        source = minime / "launchd" / f"{label}.plist"
        target = installed / source.name
        if source.read_bytes() != target.read_bytes():
            raise ValueError(f"source/installed configuration differs: {label}")
        config = plistlib.loads(source.read_bytes())
        loaded = run(["launchctl", "print", f"gui/{os.getuid()}/{label}"])
        args = config["ProgramArguments"]
        if config["Label"] != label or len(args) != 2 or args[1] not in loaded:
            raise ValueError(f"loaded launcher differs: {label}")
        if label == LABELS[0]:
            env = config.get("EnvironmentVariables", {})
            if env.get("MINIME_DIVISION_GATEWAY_ENABLED") != "true" or not re.search(
                r"^\s*MINIME_DIVISION_GATEWAY_ENABLED => true$", loaded, re.MULTILINE
            ):
                raise ValueError("runtime binding requires already-enabled Division gateway")
            runtime = Path(env["MINIME_DIVISION_RUNTIME_MANIFEST"])
            if runtime != minime / "workspace/division/runtime-manifest.json":
                raise ValueError("unexpected Division runtime manifest")
            if json.loads(runtime.read_text()).get("mode") != "dormant":
                raise ValueError("active Division transition needs separate deployment review")
            records["division-runtime"] = artifact(runtime)
        records[f"source:{label}"] = artifact(source)
        records[f"installed:{label}"] = artifact(target)
        records[f"launcher:{label}"] = artifact(Path(args[1]))
    return records


def retained_binary(minime: Path, expected: str) -> Path:
    root = minime / "minime/target/release"
    candidates = [root / "minime", *sorted((root / "deps").glob("minime-*"))]
    for path in candidates:
        if path.is_file() and not path.is_symlink() and os.access(path, os.X_OK) and digest(path) == expected:
            return path
    raise ValueError("no retained executable matches the historical build hash")


def validate(binding: dict, *, now: float | None = None) -> list[str]:
    """Recheck immutable evidence and live process/port identity at receipt use."""
    try:
        if binding["schema"] != SCHEMA or binding["memory_bytes_attested"] is not False:
            raise ValueError("invalid runtime binding scope")
        age = (time.time() if now is None else now) - binding["captured_at_unix_s"]
        if not 0 <= age <= 180:
            raise ValueError("runtime binding is stale or future-dated")
        historical = binding["historical_build_manifest"]
        path = Path(historical["path"])
        if digest(path) != historical["sha256"]:
            raise ValueError("historical build manifest changed")
        manifest = json.loads(path.read_text())
        if manifest["component"] != "minime-division-runtime" or binding["protocol"] != manifest["protocol"]:
            raise ValueError("historical component/protocol mismatch")
        expected = manifest["artifacts"]["minime-engine"]["sha256"]
        retained = binding["artifacts"]["retained-minime"]
        if retained["sha256"] != expected or digest(Path(retained["path"])) != expected:
            raise ValueError("retained binary does not match historical build")
        for name, row in manifest["artifacts"].items():
            if name != "minime-engine" and digest(Path(row["path"])) != row["sha256"]:
                raise ValueError(f"historical launcher changed: {name}")
        for row in binding["artifacts"].values():
            if digest(Path(row["path"])) != row["sha256"]:
                raise ValueError("runtime binding artifact changed")
        expected_uuid = image_uuid(run(["dwarfdump", "--uuid", retained["path"]]))
        if expected_uuid != binding["retained_image_uuid"]:
            raise ValueError("retained image UUID changed")
        rows = binding["processes"]
        if len(rows) != 3 or {row["label"] for row in rows} != set(LABELS) or len({row["pid"] for row in rows}) != 3:
            raise ValueError("incomplete or duplicate Division process topology")
        for row in rows:
            if row["image_uuid"] != expected_uuid:
                raise ValueError("mapped image UUID differs from retained build")
            if process(row["label"]) != {k: row[k] for k in ("label", "pid", "started_at")}:
                raise ValueError("process identity changed during capture")
        gateway = next(row["pid"] for row in rows if row["label"] == LABELS[1])
        for port in (7878, 7879):
            if binding["port_owners"][str(port)] != gateway or port_owner(port) != gateway:
                raise ValueError(f"Division gateway does not own port {port}")
        minime = Path(binding["minime_repository"])
        installed = Path(binding["installed_launch_agents"])
        if topology(minime, installed) != binding["configuration"]:
            raise ValueError("Division configuration changed during capture")
    except (KeyError, TypeError, ValueError, OSError, subprocess.SubprocessError) as error:
        return [str(error)]
    return []


def capture(minime: Path, workspace: Path, installed: Path) -> Path:
    historical = workspace / "deployment_manifests/minime-division-runtime.json"
    historical_ref = artifact(historical)
    manifest = json.loads(historical.read_text())
    config = topology(minime, installed)
    retained = retained_binary(minime, manifest["artifacts"]["minime-engine"]["sha256"])
    uuid = image_uuid(run(["dwarfdump", "--uuid", str(retained)]))
    processes = []
    for label in LABELS:
        before = process(label)
        # sample's report is held only in memory; persist just the image UUID.
        sampled = image_uuid(run(["sample", str(before["pid"]), "1", "1", "-file", "/dev/stdout"], 120), sampled=True)
        if sampled != uuid or process(label) != before:
            raise ValueError(f"mapped build mismatch or process changed: {label}")
        processes.append({**before, "image_uuid": sampled})
    destination = Path(tempfile.mkdtemp(prefix="minime-runtime-binding.", dir=workspace / "deployment_manifests"))
    archived = destination / "retained-minime"
    shutil.copyfile(retained, archived)
    archived.chmod(0o600)
    binding = {
        "schema": SCHEMA, "schema_version": 1,
        "component": "minime-division-runtime",
        "captured_at_unix_s": time.time(),
        "identity_method": "historical_sha256_and_mapped_macho_uuid_corroboration",
        "memory_bytes_attested": False, "witness_only": True,
        "grants_approval": False, "live_eligible_now": False,
        "edits_source_now": False, "auto_approved": False,
        "historical_build_manifest": historical_ref,
        "protocol": manifest["protocol"],
        "minime_repository": str(minime), "installed_launch_agents": str(installed),
        "retained_from": str(retained), "retained_image_uuid": uuid,
        "artifacts": {"retained-minime": artifact(archived), **config},
        "configuration": config, "processes": processes,
        "port_owners": {str(port): port_owner(port) for port in (7878, 7879)},
        "current_disk_executable": artifact(minime / "minime/target/release/minime"),
    }
    errors = validate(binding)
    if errors:
        raise ValueError("; ".join(errors))
    output = destination / "runtime-binding.json"
    with output.open("x") as stream:
        os.fchmod(stream.fileno(), 0o600)
        json.dump(binding, stream, indent=2, sort_keys=True)
        stream.write("\n")
        stream.flush()
        os.fsync(stream.fileno())
    return output


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--minime", type=Path, default=Path("/Users/v/other/minime"))
    parser.add_argument("--workspace", type=Path, default=Path("/Users/v/other/astrid/capsules/spectral-bridge/workspace"))
    args = parser.parse_args()
    try:
        print(capture(args.minime, args.workspace, Path.home() / "Library/LaunchAgents"))
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        parser.exit(1, f"Minime runtime binding refused: {error}\n")
