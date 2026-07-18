"""Read-only concurrent-editor activity evidence."""

from __future__ import annotations

from pathlib import Path
import subprocess
from typing import Iterable

FOREIGN_ACTIVITY_WINDOW_SECS = 180.0
AGENT_STATE_DIR_DEFAULT = Path("~/.astrid/run/agent-state").expanduser()
AGENT_STATE_GLOBS = ("*.heartbeat", "*.json", "*.sqlite")


def newest_age(mtimes: Iterable[float], now: float) -> float | None:
    values = list(mtimes)
    return None if not values else now - max(values)


def _safe_mtime(path: Path) -> float | None:
    try:
        return path.stat().st_mtime
    except OSError:
        return None


def tree_activity_age(repo: Path, now: float) -> float | None:
    """Age of the newest uncommitted path, without changing repository state."""

    try:
        output = subprocess.run(
            ["git", "-C", str(repo), "status", "--porcelain", "-z"],
            capture_output=True,
            text=True,
            timeout=10,
            check=False,
        ).stdout
    except (OSError, subprocess.SubprocessError):
        return None
    mtimes: list[float] = []
    for entry in output.split("\0"):
        if len(entry) < 4:
            continue
        relative = entry[3:]
        if " -> " in relative:
            relative = relative.split(" -> ", 1)[1]
        modified = _safe_mtime(repo / relative)
        if modified is not None:
            mtimes.append(modified)
    return newest_age(mtimes, now)


def session_liveness_age(
    state_dir: Path,
    now: float,
    *,
    patterns: Iterable[str] = AGENT_STATE_GLOBS,
) -> float | None:
    mtimes: list[float] = []
    for pattern in patterns:
        for path in state_dir.glob(pattern):
            modified = _safe_mtime(path)
            if modified is not None:
                mtimes.append(modified)
    return newest_age(mtimes, now)


def foreign_activity(
    repo: Path,
    state_dir: Path,
    window_secs: float,
    now: float,
) -> dict[str, object]:
    """Return evidence that another editor may be actively changing the tree."""

    tree_age = tree_activity_age(repo, now)
    session_age = session_liveness_age(state_dir, now)
    active = tree_age is not None and tree_age <= window_secs
    session_live = session_age is not None and session_age <= window_secs
    return {
        "active": active,
        "tree_activity_age_s": None if tree_age is None else round(tree_age, 1),
        "session_state_live": session_live,
        "session_state_age_s": (
            None if session_age is None else round(session_age, 1)
        ),
        "window_s": window_secs,
        "agent_guess": "concurrent-editor" if active else None,
    }
