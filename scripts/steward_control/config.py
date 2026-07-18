"""Portable TOML and environment configuration."""

from __future__ import annotations

from dataclasses import dataclass, field
import os
from pathlib import Path
import tomllib
from typing import Any

from .errors import ConfigurationError
from .model import canonical_json, sha256_bytes

CONFIG_ENV = "ASTRID_STEWARD_CONFIG"
PATH_ENV_KEYS = {
    "repo_root": "ASTRID_STEWARD_REPO_ROOT",
    "workspace": "ASTRID_STEWARD_WORKSPACE",
    "state_root": "ASTRID_STEWARD_STATE_ROOT",
    "store_root": "ASTRID_STEWARD_STORE_ROOT",
}
NUMBER_ENV_KEYS = {
    "lease_ttl_secs": "ASTRID_STEWARD_LEASE_TTL_SECS",
    "heartbeat_interval_secs": "ASTRID_STEWARD_HEARTBEAT_INTERVAL_SECS",
    "max_run_secs": "ASTRID_STEWARD_MAX_RUN_SECS",
    "projector_timeout_secs": "ASTRID_STEWARD_PROJECTOR_TIMEOUT_SECS",
    "pause_grace_secs": "ASTRID_STEWARD_PAUSE_GRACE_SECS",
}


def default_repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


def _resolve_path(value: str | Path, base: Path) -> Path:
    path = Path(value).expanduser()
    if not path.is_absolute():
        path = base / path
    return path.resolve()


def _positive_int(value: Any, name: str) -> int:
    if isinstance(value, bool):
        raise ConfigurationError(f"{name} must be a positive integer")
    try:
        resolved = int(value)
    except (TypeError, ValueError) as error:
        raise ConfigurationError(f"{name} must be a positive integer") from error
    if resolved <= 0:
        raise ConfigurationError(f"{name} must be a positive integer")
    return resolved


@dataclass(frozen=True)
class ControlConfig:
    repo_root: Path
    workspace: Path
    state_root: Path
    store_root: Path
    lease_ttl_secs: int = 1800
    heartbeat_interval_secs: int = 60
    max_run_secs: int = 1500
    projector_timeout_secs: int = 600
    pause_grace_secs: int = 30
    repositories: dict[str, Path] = field(default_factory=dict)
    profile: str = "source-first"

    def canonical_record(self) -> dict[str, Any]:
        def relative_or_hash(path: Path) -> str:
            try:
                return str(path.relative_to(self.repo_root))
            except ValueError:
                return f"external:{sha256_bytes(str(path).encode())}"

        return {
            "schema": "steward_control_config_v1",
            "schema_version": 1,
            "repo_root": ".",
            "workspace": relative_or_hash(self.workspace),
            "state_root": relative_or_hash(self.state_root),
            "store_root": relative_or_hash(self.store_root),
            "lease_ttl_secs": self.lease_ttl_secs,
            "heartbeat_interval_secs": self.heartbeat_interval_secs,
            "max_run_secs": self.max_run_secs,
            "projector_timeout_secs": self.projector_timeout_secs,
            "pause_grace_secs": self.pause_grace_secs,
            "repositories": {
                name: relative_or_hash(path)
                for name, path in sorted(self.repositories.items())
            },
            "profile": self.profile,
        }

    @property
    def config_sha256(self) -> str:
        return sha256_bytes(canonical_json(self.canonical_record()).encode())


def _read_config(path: Path | None) -> tuple[dict[str, Any], Path]:
    if path is None:
        return {}, default_repo_root()
    try:
        with path.open("rb") as handle:
            value = tomllib.load(handle)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise ConfigurationError(f"cannot load config {path}: {error}") from error
    if not isinstance(value, dict):
        raise ConfigurationError("steward configuration must be a TOML table")
    return value, path.parent.resolve()


def load_config(
    *,
    config_path: str | Path | None = None,
    repo_root: str | Path | None = None,
    workspace: str | Path | None = None,
    state_root: str | Path | None = None,
    store_root: str | Path | None = None,
    environ: dict[str, str] | None = None,
) -> ControlConfig:
    env = dict(os.environ if environ is None else environ)
    raw_config_path = config_path or env.get(CONFIG_ENV)
    path = Path(raw_config_path).expanduser().resolve() if raw_config_path else None
    raw, base = _read_config(path)
    control = raw.get("control") if isinstance(raw.get("control"), dict) else raw

    derived_repo = default_repo_root()
    configured_repo = repo_root or env.get(PATH_ENV_KEYS["repo_root"]) or control.get(
        "repo_root"
    )
    resolved_repo = (
        _resolve_path(configured_repo, base) if configured_repo else derived_repo
    )
    configured_workspace = (
        workspace
        or env.get(PATH_ENV_KEYS["workspace"])
        or control.get("workspace")
        or "capsules/spectral-bridge/workspace"
    )
    resolved_workspace = _resolve_path(configured_workspace, resolved_repo)
    configured_state = (
        state_root
        or env.get(PATH_ENV_KEYS["state_root"])
        or control.get("state_root")
        or "diagnostics/steward_control_v1"
    )
    resolved_state = _resolve_path(configured_state, resolved_workspace)
    configured_store = (
        store_root
        or env.get(PATH_ENV_KEYS["store_root"])
        or control.get("store_root")
        or "diagnostics/evidence_event_store_v2"
    )
    resolved_store = _resolve_path(configured_store, resolved_workspace)

    numbers: dict[str, int] = {}
    defaults = {
        "lease_ttl_secs": 1800,
        "heartbeat_interval_secs": 60,
        "max_run_secs": 1500,
        "projector_timeout_secs": 600,
        "pause_grace_secs": 30,
    }
    for name, default in defaults.items():
        raw_value = env.get(NUMBER_ENV_KEYS[name], control.get(name, default))
        numbers[name] = _positive_int(raw_value, name)
    if numbers["heartbeat_interval_secs"] >= numbers["lease_ttl_secs"]:
        raise ConfigurationError("heartbeat interval must be shorter than lease TTL")

    repositories: dict[str, Path] = {"astrid": resolved_repo}
    raw_repositories = raw.get("repositories")
    if isinstance(raw_repositories, dict):
        for name, value in raw_repositories.items():
            if isinstance(value, dict):
                value = value.get("path")
            if not isinstance(value, str) or not value.strip():
                raise ConfigurationError(f"repository {name!r} requires a path")
            repositories[str(name)] = _resolve_path(value, base)

    return ControlConfig(
        repo_root=resolved_repo,
        workspace=resolved_workspace,
        state_root=resolved_state,
        store_root=resolved_store,
        lease_ttl_secs=numbers["lease_ttl_secs"],
        heartbeat_interval_secs=numbers["heartbeat_interval_secs"],
        max_run_secs=numbers["max_run_secs"],
        projector_timeout_secs=numbers["projector_timeout_secs"],
        pause_grace_secs=numbers["pause_grace_secs"],
        repositories=repositories,
        profile=str(control.get("profile") or "source-first"),
    )
