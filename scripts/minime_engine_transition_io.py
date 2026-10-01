"""Bounded host primitives for the explicitly approved engine transition."""
from __future__ import annotations

from contextlib import contextmanager
import fcntl
import hashlib
import json
import math
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time

from minime_runtime_binding import digest

LABELS = ("com.minime.engine", "com.minime.division-gateway", "com.minime.division-supervisor")
LAUNCHERS = ("launchd_minime_engine.sh", "launchd_division_gateway.sh", "launchd_division_supervisor.sh")
PORTS = {LABELS[0]: (7900, 7901, 7902), LABELS[1]: (7878, 7879, 7880, 7882, 7883), LABELS[2]: ()}
PROTECTED = ("com.minime.autonomous-agent", "com.astrid.spectral-bridge",
             "com.reservoir.coupled-astrid", "com.minime.visual-frame-service",
             "com.minime.host-sensory", "com.minime.camera-client", "com.minime.mic-to-sensory")
HOLD_LOOP = 'while [ -e "$HOLD" ] || [ -L "$HOLD" ]; do sleep 1; done\n'


def require(condition, message):
    if not condition:
        raise ValueError(message)


def regular(path: Path, limit=8 * 1024 * 1024):
    require(not any(p.is_symlink() for p in (path, *path.parents)), f"linked path: {path}")
    stat = path.stat()
    require(path.is_file() and stat.st_size <= limit, f"unbounded/nonregular file: {path}")
    return path


def read_json(path):
    return json.loads(regular(path).read_bytes())


def atomic(path, data, mode=0o600):
    require(not path.is_symlink(), f"linked destination: {path}")
    require(not any(p.is_symlink() for p in path.parents), "linked destination parent")
    with tempfile.NamedTemporaryFile(dir=path.parent, prefix="." + path.name, delete=False) as f:
        temp = Path(f.name)
        try:
            os.fchmod(f.fileno(), mode)
            f.write(data)
            f.flush()
            os.fsync(f.fileno())
            os.replace(temp, path)
            sync_dir(path.parent)
        finally:
            temp.unlink(missing_ok=True)


def sync_dir(path):
    fd = os.open(path, os.O_RDONLY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def save(path, value):
    atomic(path, (json.dumps(value, indent=2, sort_keys=True, allow_nan=False) + "\n").encode())


def command(args, *, cwd=None, timeout=30, env=None):
    result = subprocess.run([str(a) for a in args], cwd=cwd, timeout=timeout,
                            env=env, capture_output=True, text=True)
    require(result.returncode == 0, f"command failed: {Path(args[0]).name}, exit {result.returncode}")
    require(len(result.stdout) <= 2 * 1024 * 1024, "command output limit")
    return result.stdout


def started(pid):
    result = subprocess.run(["ps", "-p", str(pid), "-o", "lstart="],
                            capture_output=True, text=True, timeout=10)
    return " ".join(result.stdout.split()) if result.returncode == 0 else ""


def stop_exact(row, timeout):
    require(started(row["pid"]) == row["started_at"], "process changed before SIGTERM")
    os.kill(row["pid"], signal.SIGTERM)
    deadline = time.monotonic() + timeout
    while started(row["pid"]) == row["started_at"]:
        require(time.monotonic() < deadline, "SIGTERM timeout; no forced termination permitted")
        time.sleep(.25)


def listeners(port):
    result = subprocess.run(["lsof", "-t", "-nP", f"-iTCP:{port}", "-sTCP:LISTEN"],
                            capture_output=True, text=True, timeout=10)
    require(result.returncode in (0, 1), "listener inspection failed")
    return {int(p) for p in result.stdout.split()}


def hold_block(label):
    root = "$PROJECT_DIR" if label == LABELS[0] else "$ROOT"
    comment = "# A deployment may replace only the stopped engine, never a KeepAlive respawn.\n" if label == LABELS[0] else ""
    return f'{comment}HOLD="{root}/workspace/runtime/engine-release-holds/{label}.json"\n' + HOLD_LOOP + "\n"


def validate_launcher(old, new, label):
    block = hold_block(label).encode()
    require(new.count(block) == 1, "launcher needs exactly one reviewed hold block")
    require(old == new or new.replace(block, b"", 1) == old,
            "launcher changes exceed the reviewed launch hold")
    anchor = b'cd "$PROJECT_DIR"\n\n' if label == LABELS[0] else b'MANIFEST="$ROOT/workspace/division/runtime-manifest.json"\n\n'
    require(anchor + block in new, "hold must precede profile, state and executable access")


class Holds:
    """Only the transaction that created a hold may release it."""
    def __init__(self, root, owner):
        self.root, self.owner = root, owner

    def payload(self, label):
        require(label in LABELS, "unknown hold label")
        return {"schema": "minime_engine_launch_hold_v1", "owner": self.owner, "label": label}

    def create(self, label):
        self.root.mkdir(parents=True, exist_ok=True, mode=0o700)
        require(not any(p.is_symlink() for p in (self.root, *self.root.parents)), "linked hold root")
        path = self.root / (label + ".json")
        data = (json.dumps(self.payload(label), sort_keys=True) + "\n").encode()
        fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
        with os.fdopen(fd, "wb") as f:
            f.write(data)
            f.flush()
            os.fsync(f.fileno())
        sync_dir(self.root)

    def verify(self, label):
        require(read_json(self.root / (label + ".json")) == self.payload(label), "foreign or corrupt launch hold")

    def release(self, label):
        self.verify(label)
        (self.root / (label + ".json")).unlink()
        sync_dir(self.root)


@contextmanager
def exclusive(path):
    require(not path.is_symlink(), "linked transaction lock")
    fd = os.open(path, os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o600)
    try:
        fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        yield
    finally:
        os.close(fd)


def health_point(value, *, now, earliest_ms, candidate):
    provenance = value["provenance"]
    wall = provenance["wall_clock_unix_ms"]
    require(earliest_ms <= wall <= now * 1000 and now * 1000 - wall < 15000, "stale/future health")
    fill = value["fill_pct"]
    require(isinstance(fill, (int, float)) and math.isfinite(fill) and 0 <= fill <= 100,
            "invalid fill measurement")
    require(fill < 80, "deployment abort: fill reached the existing 80% warning boundary")
    restore = value["startup_restore"]
    require(restore["state"] == "restored" and restore["resume_mode"] == "pi_only_resume",
            "unexpected regulator restore")
    stable = value["stable_core"]
    require(stable["enabled"] and not stable["checkpoint_lineage_enabled"] and not stable["neural_bundle_enabled"],
            "unexpected runtime continuity policy")
    if candidate:
        require("measurement_basis_v1" in value and "fill_rate_v1" in value, "missing repaired measurement evidence")
    return {"wall_clock_unix_ms": wall, "fill_pct": fill, "t_s": value["t_s"],
            "session_id": provenance["session_id"], "snapshot_sequence": provenance["snapshot_sequence"],
            "stage": stable["stage"], "restart_gate_phase": stable["restart_gate"]["phase"],
            "target_fill_pct": stable["structural_pi"]["target_fill_pct"],
            "fill_rate_v1": value.get("fill_rate_v1"),
            "measurement_basis_v1": value.get("measurement_basis_v1")}
