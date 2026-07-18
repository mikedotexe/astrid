"""Read-only repository identity and mutation detection."""

from __future__ import annotations

import hashlib
from pathlib import Path
import subprocess
from typing import Any


def _run(repo: Path, *args: str, allow_failure: bool = False) -> bytes:
    result = subprocess.run(
        ["git", "-C", str(repo), *args],
        capture_output=True,
        timeout=20,
        check=False,
    )
    if result.returncode and not allow_failure:
        message = result.stderr.decode("utf-8", errors="replace").strip()
        raise RuntimeError(f"git {' '.join(args)} failed: {message}")
    return result.stdout


def _digest(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def repository_identity(repo: Path) -> dict[str, Any]:
    repo = repo.resolve()
    branch = _run(repo, "symbolic-ref", "--quiet", "--short", "HEAD", allow_failure=True)
    head = _run(repo, "rev-parse", "HEAD").decode().strip()
    status = _run(repo, "status", "--porcelain=v1", "-z", "--untracked-files=all")
    staged = _run(repo, "diff", "--cached", "--binary", "--no-ext-diff")
    remotes = _run(repo, "remote", "-v", allow_failure=True)
    return {
        "schema": "steward_repository_identity_v1",
        "schema_version": 1,
        "name": repo.name,
        "branch": branch.decode().strip() or None,
        "head": head,
        "status_sha256": _digest(status),
        "staged_sha256": _digest(staged),
        "dirty": bool(status),
        "staged": bool(staged),
        "remote_identity_sha256": _digest(remotes),
    }


def repository_identities(repositories: dict[str, Path]) -> dict[str, dict[str, Any]]:
    identities: dict[str, dict[str, Any]] = {}
    for name, path in sorted(repositories.items()):
        try:
            identities[name] = repository_identity(path)
        except (OSError, RuntimeError, subprocess.SubprocessError) as error:
            identities[name] = {
                "schema": "steward_repository_identity_v1",
                "schema_version": 1,
                "name": path.name,
                "unavailable": True,
                "error_type": type(error).__name__,
            }
    return identities


def git_policy_violations(
    before: dict[str, dict[str, Any]],
    after: dict[str, dict[str, Any]],
) -> list[str]:
    violations: list[str] = []
    for name in sorted(set(before) | set(after)):
        old = before.get(name) or {}
        new = after.get(name) or {}
        for field in ("head", "branch", "staged_sha256", "remote_identity_sha256"):
            if old.get(field) != new.get(field):
                violations.append(f"{name}:{field}_changed")
        if not old.get("staged") and new.get("staged"):
            marker = f"{name}:staged_sha256_changed"
            if marker not in violations:
                violations.append(f"{name}:staging_detected")
    return violations
