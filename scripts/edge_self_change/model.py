"""Typed manifests, exact paths, and authenticated ledgers for edge self-change."""

from __future__ import annotations

import dataclasses
import fcntl
import hashlib
import hmac
import json
import os
import re
import stat
import tempfile
from pathlib import Path
from typing import Any, Mapping, Sequence


CONFIG_SCHEMA = "astrid.edge_self_change.config.v1"
PROFILE_SCHEMA = "astrid.edge_self_change.command_profiles.v1"
STATE_SCHEMA = "astrid.edge_self_change.state.v1"
LEDGER_SCHEMA = "astrid.edge_self_change.ledger_record.v1"
CANDIDATE_SCHEMA = "astrid.edge_self_change.candidate.v1"
BUILD_SCHEMA = "astrid.edge_self_change.build.v1"
INTENT_SCHEMA = "astrid.edge_self_change.scheduled_model_intent.v1"
INTENT_ENVELOPE_SCHEMA = "astrid.edge_self_change.intent_attestor_envelope.v1"
COMPLETED_INTENT_ENVELOPE_SCHEMA = (
    "astrid.edge_self_change.completed_intent_envelope.v1"
)
AUTHORED_COMPLETION_ENVELOPE_SCHEMA = (
    "astrid.edge.steward_helper.authored_completion_envelope.v2"
)
AUTHORED_COMPLETION_SCHEMA = "astrid.edge.steward_helper.authored_completion.v2"
GENERATION_SCHEMA = "astrid.edge_self_change.generation.v1"

DUE_COALESCE_SECONDS = 2 * 60 * 60
PROBATION_SECONDS = 60 * 60
PIPELINE_MAX_SECONDS = 24 * 60 * 60
# A signed scheduled intent may wait behind the one-hour bootstrap acceptance,
# a model/build maintenance lease, or a reboot. Its one accepted lifetime is
# therefore exactly the immutable pipeline lifetime; generation equality,
# candidate/envelope hashes, and replay ledgers remain independently required.
INTENT_INGEST_MAX_AGE_SECONDS = PIPELINE_MAX_SECONDS
RETENTION_SECONDS = 7 * 24 * 60 * 60
# Rollback safety is expressed in *prior* generations.  Counting the active
# generation toward this floor would leave only two rollback generations, so
# the total minimum is active + three prior generations.
MIN_RETAINED_PRIOR_GENERATIONS = 3
MIN_RETAINED_GENERATIONS = MIN_RETAINED_PRIOR_GENERATIONS + 1
MAX_JSON_BYTES = 256 * 1024
MAX_LEDGER_LINE_BYTES = 128 * 1024
MAX_LEDGER_BYTES = 256 * 1024 * 1024
MAX_COMMAND_OUTPUT_BYTES = 64 * 1024
MAX_GENERATION_ENTRIES = 50_000

IDENTIFIER_RE = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]{0,127}\Z")
HEX64_RE = re.compile(r"[0-9a-f]{64}\Z")
SOURCE_REVISION_RE = re.compile(r"[0-9a-f]{7,64}\Z")

# Candidates may never modify the mechanism which evaluates or activates them,
# capability/security roots, dependency/build authority, or host configuration.
# Configuration may only add prefixes; it cannot remove these.
IMMUTABLE_ROOT_DENYLIST = (
    ".git",
    ".github",
    "minime",
    "capsules/spectral-bridge",
    "capsules/introspector",
    "scripts/edge_self_change_supervisor.py",
    "scripts/edge_self_change",
    "scripts/build_edge_self_change_source_bundle.py",
    "scripts/test_build_edge_self_change_source_bundle.py",
    "scripts/install_steward_hooks.py",
    "scripts/install_edge_runtime.sh",
    "scripts/install_headless_linux.sh",
    "scripts/package_edge_appliance.sh",
    "services/astrid-edge-steward-helper",
    "services/astrid-edge-rescue-helper",
    "services/astrid-edge-web-broker",
    "services/astrid-edge-checkpoint",
    "services/astrid-edge-self-change-helper",
    "packaging/systemd/root",
    "packaging/systemd/astrid-edge-self-change-authority.conf",
    "packaging/systemd/astrid-edge-self-change-disabled.env",
    "packaging/systemd/astrid-edge-self-change-enabled.env",
    "packaging/systemd/astrid-edge-self-change-supervisor.service",
    "packaging/systemd/astrid-edge-self-change-probation-health.service",
    "packaging/systemd/astrid-edge-self-change-probation-health.timer",
    "packaging/systemd/astrid-edge-steward.service",
    "packaging/systemd/astrid-edge-steward.timer",
)

MUTABLE_CORE_CRATES = frozenset(
    {
        "astrid-approval",
        "astrid-audit",
        "astrid-build",
        "astrid-capabilities",
        "astrid-capsule",
        "astrid-cli",
        "astrid-config",
        "astrid-core",
        "astrid-crypto",
        "astrid-daemon",
        "astrid-events",
        "astrid-guest",
        "astrid-hooks",
        "astrid-kernel",
        "astrid-mcp",
        "astrid-minime-protocol",
        "astrid-openclaw",
        "astrid-prelude",
        "astrid-spectral-core",
        "astrid-storage",
        "astrid-telemetry",
        "astrid-types",
        "astrid-integration-tests",
        "astrid-test",
        "astrid-vfs",
        "astrid-workspace",
    }
)
EDGE_CAPSULES = frozenset(
    {
        "astrid-capsule-agents",
        "astrid-capsule-cli",
        "astrid-capsule-edge-context",
        "astrid-capsule-edge-introspector",
        "astrid-capsule-edge-spectral",
        "astrid-capsule-fs",
        "astrid-capsule-http",
        "astrid-capsule-memory",
        "astrid-capsule-shell",
        "astrid-capsule-skills",
        "astrid-capsule-context-engine",
        "astrid-capsule-hook-bridge",
        "astrid-capsule-identity",
        "astrid-capsule-openai-compat",
        "astrid-capsule-prompt-builder",
        "astrid-capsule-react",
        "astrid-capsule-registry",
        "astrid-capsule-router",
        "astrid-capsule-session",
        "astrid-capsule-system",
    }
)
MUTABLE_UNIT_FRAGMENTS = frozenset(
    {
        "ollama-cpu.service",
        "astrid-model-warmup.service",
        "astrid.service",
        "astrid-edge-runtime.service",
        "astrid-edge-hindsight.service",
        "astrid-edge-hindsight.timer",
    }
)

PROFILE_ENVELOPES = {
    "model": "proposal-sandbox:no-host-write:v1",
    "build": "offline-build-sandbox:no-host-state:v1",
    "install": "root-stager:release-root-only:v1",
    "activate": "root-activator:ab-link-and-service-only:v1",
    "rollback": "root-rollback:ab-link-and-service-only:v1",
    "health": "read-only-health:v1",
    "retention": "root-retention:paired-generation-snapshot-only:v1",
    "synthetic": "operator-synthetic:offline-build-model-unloaded:v1",
}
FORBIDDEN_EXECUTABLE_NAMES = {
    "bash",
    "dash",
    "env",
    "fish",
    "perl",
    "python",
    "python3",
    "ruby",
    "sh",
    "sudo",
    "systemd-run",
    "zsh",
}
ALLOWED_PLACEHOLDERS = {
    "active_link",
    "build_id",
    "build_manifest",
    "candidate_id",
    "candidate_manifest",
    "generation_dir",
    "intent_envelope",
    "model_handoff",
    "previous_generation_dir",
    "releases_root",
    "state_root",
}


class SupervisorError(RuntimeError):
    """A fail-closed validation or state-machine error."""


class IntegrityError(SupervisorError):
    """Authenticated state or ledger continuity is invalid."""


class ProfileError(SupervisorError):
    """An immutable command profile is absent or unsafe."""


def canonical_bytes(value: Any) -> bytes:
    try:
        return json.dumps(
            value,
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=True,
            allow_nan=False,
        ).encode("ascii")
    except (TypeError, ValueError) as error:
        raise SupervisorError(f"value is not canonical JSON: {error}") from error


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as handle:
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                digest.update(chunk)
    except OSError as error:
        raise SupervisorError(f"cannot hash {path}: {error}") from error
    return digest.hexdigest()


def _require_exact_keys(
    value: Mapping[str, Any], *, required: set[str], optional: set[str], label: str
) -> None:
    keys = set(value)
    missing = required - keys
    unknown = keys - required - optional
    if missing:
        raise SupervisorError(f"{label} omitted fields: {sorted(missing)}")
    if unknown:
        raise SupervisorError(f"{label} contains unsupported fields: {sorted(unknown)}")


def _identifier(value: Any, label: str) -> str:
    text = str(value)
    if not IDENTIFIER_RE.fullmatch(text) or text in {".", ".."}:
        raise SupervisorError(f"invalid {label}")
    return text


def _hex64(value: Any, label: str) -> str:
    text = str(value)
    if not HEX64_RE.fullmatch(text):
        raise SupervisorError(f"invalid {label}")
    return text


def _timestamp(value: Any, label: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise SupervisorError(f"invalid {label}")
    return value


def validate_relative_path(value: Any, label: str = "path") -> str:
    text = str(value)
    if not text or len(text) > 512 or "\\" in text or "\x00" in text:
        raise SupervisorError(f"invalid {label}")
    if any(ord(character) < 0x20 or ord(character) == 0x7F for character in text):
        raise SupervisorError(f"invalid {label}")
    candidate = Path(text)
    if candidate.is_absolute() or any(part in {"", ".", ".."} for part in candidate.parts):
        raise SupervisorError(f"invalid {label}")
    normalized = candidate.as_posix()
    if normalized != text:
        raise SupervisorError(f"non-canonical {label}")
    return normalized


def path_has_prefix(path: str, prefix: str) -> bool:
    return path == prefix or path.startswith(prefix + "/")


def is_mutable_cpu_edge_path(path: str) -> bool:
    """Match the explicit mutable surface carried by a signed CPU-edge source ID."""

    if path in {"Cargo.toml", "Cargo.lock"}:
        return True
    parts = Path(path).parts
    if len(parts) >= 3 and parts[0] == "crates" and parts[1] in MUTABLE_CORE_CRATES:
        relative = Path(*parts[2:]).as_posix()
        return relative in {"Cargo.toml", "build.rs"} or (
            relative.endswith(".rs")
            and (relative.startswith("src/") or relative.startswith("tests/"))
        )
    runtime_prefix = "services/astrid-edge-runtime/"
    if path.startswith(runtime_prefix):
        relative = path.removeprefix(runtime_prefix)
        return relative in {"Cargo.toml", "Cargo.lock", "build.rs"} or (
            relative.startswith("src/") and relative.endswith(".rs")
        )
    if len(parts) >= 3 and parts[:2] == ("capsules", "astralis"):
        if parts[2] in EDGE_CAPSULES:
            relative = Path(*parts[3:]).as_posix()
            return relative in {"Cargo.toml", "Cargo.lock", "Capsule.toml", "build.rs"} or (
                relative.startswith("src/")
                and Path(relative).suffix.casefold() in {".rs", ".md", ".json", ".toml", ".txt"}
            )
    if len(parts) == 2 and parts[0] == "scripts":
        name = parts[1]
        return (
            (name.startswith("report_edge_") or name.startswith("test_report_edge_"))
            and Path(name).suffix in {".py", ".sh"}
        ) or name in {
            "astrid_at_a_glance.py",
            "edge_hindsight.py",
            "test_edge_hindsight.py",
        }
    if len(parts) == 3 and parts[:2] == ("packaging", "appliances"):
        return Path(parts[2]).suffix in {".env", ".json"}
    if len(parts) in {3, 4} and parts[:2] == ("packaging", "systemd"):
        name = parts[-1]
        if len(parts) == 4 and parts[2] != "icp":
            return False
        return name in MUTABLE_UNIT_FRAGMENTS
    return False


def validate_changed_paths(values: Any, extra_denylist: Sequence[str] = ()) -> tuple[str, ...]:
    if not isinstance(values, list) or not 1 <= len(values) <= 25:
        raise SupervisorError("changed_paths must contain 1..25 paths")
    denylist = tuple(IMMUTABLE_ROOT_DENYLIST) + tuple(extra_denylist)
    paths: list[str] = []
    for raw in values:
        path = validate_relative_path(raw, "changed path")
        if any(path_has_prefix(path, prefix) for prefix in denylist):
            raise SupervisorError(f"changed path enters immutable root: {path}")
        if not is_mutable_cpu_edge_path(path):
            raise SupervisorError(f"changed path is outside the signed CPU-edge surface: {path}")
        paths.append(path)
    if len(paths) != len(set(paths)):
        raise SupervisorError("changed_paths contains duplicates")
    return tuple(sorted(paths))


def _resolved_absolute(path: Path, label: str) -> Path:
    if not path.is_absolute():
        raise SupervisorError(f"{label} must be absolute")
    resolved = path.resolve(strict=False)
    if resolved != path:
        raise SupervisorError(f"{label} contains a symlink or non-canonical component")
    return path


def _lstat_no_link(path: Path, label: str) -> os.stat_result:
    try:
        info = path.lstat()
    except OSError as error:
        raise SupervisorError(f"cannot inspect {label} {path}: {error}") from error
    if stat.S_ISLNK(info.st_mode):
        raise SupervisorError(f"{label} must not be a symlink: {path}")
    return info


def validate_bounded_path(
    root: Path,
    candidate: Path,
    *,
    require_exists: bool = True,
    allow_final_symlink: bool = False,
) -> Path:
    root = _resolved_absolute(root, "path root")
    candidate = _resolved_absolute(candidate, "bounded path") if not allow_final_symlink else candidate
    try:
        candidate.relative_to(root)
    except ValueError as error:
        raise SupervisorError(f"path escapes root {root}: {candidate}") from error
    _lstat_no_link(root, "path root")
    relative = candidate.relative_to(root)
    current = root
    for index, part in enumerate(relative.parts):
        current = current / part
        final = index == len(relative.parts) - 1
        try:
            info = current.lstat()
        except FileNotFoundError:
            if require_exists:
                raise SupervisorError(f"bounded path does not exist: {current}") from None
            break
        except OSError as error:
            raise SupervisorError(f"cannot inspect bounded path {current}: {error}") from error
        if stat.S_ISLNK(info.st_mode) and not (final and allow_final_symlink):
            raise SupervisorError(f"bounded path traverses symlink: {current}")
    return candidate


def ensure_private_dir(path: Path) -> None:
    path = _resolved_absolute(path, "private directory")
    missing: list[Path] = []
    cursor = path
    while not cursor.exists():
        missing.append(cursor)
        if cursor.parent == cursor:
            break
        cursor = cursor.parent
    info = _lstat_no_link(cursor, "directory ancestor")
    if not stat.S_ISDIR(info.st_mode):
        raise SupervisorError(f"directory ancestor is not a directory: {cursor}")
    for item in reversed(missing):
        try:
            item.mkdir(mode=0o700)
        except FileExistsError:
            concurrent = _lstat_no_link(item, "concurrently created private directory")
            if not stat.S_ISDIR(concurrent.st_mode):
                raise SupervisorError(
                    f"concurrently created private path is not a directory: {item}"
                ) from None
        except OSError as error:
            raise SupervisorError(f"cannot create private directory {item}: {error}") from error
    os.chmod(path, 0o700)


def atomic_write(path: Path, data: bytes, mode: int = 0o600) -> None:
    ensure_private_dir(path.parent)
    descriptor, temporary = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    try:
        os.fchmod(descriptor, mode)
        with os.fdopen(descriptor, "wb", closefd=True) as handle:
            handle.write(data)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
        directory_fd = os.open(path.parent, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
        try:
            os.fsync(directory_fd)
        finally:
            os.close(directory_fd)
    except BaseException:
        try:
            os.unlink(temporary)
        except FileNotFoundError:
            pass
        raise


def read_json(path: Path, *, root: Path | None = None, immutable: bool = False) -> dict[str, Any]:
    if immutable:
        path = _resolved_absolute(path, "immutable JSON path")
    if root is not None:
        validate_bounded_path(root, path)
    info = _lstat_no_link(path, "JSON file")
    if not stat.S_ISREG(info.st_mode) or info.st_size > MAX_JSON_BYTES:
        raise SupervisorError(f"JSON input is not a bounded regular file: {path}")
    if immutable:
        if info.st_uid not in {0, os.geteuid()} or info.st_mode & 0o022:
            raise SupervisorError(f"immutable JSON must be owner-controlled and not group/world writable: {path}")
    try:
        value = json.loads(path.read_bytes())
    except (OSError, json.JSONDecodeError) as error:
        raise SupervisorError(f"cannot decode JSON {path}: {error}") from error
    if not isinstance(value, dict):
        raise SupervisorError(f"JSON document must be an object: {path}")
    return value


@dataclasses.dataclass(frozen=True)
class Config:
    state_root: Path
    releases_root: Path
    active_link: Path
    signing_key: Path
    intent_attestation_key: Path
    command_profiles: Path
    operator_status: Path
    model_handoff_root: Path
    appliance_id: str
    target: str
    extra_denylist: tuple[str, ...] = ()

    @classmethod
    def from_file(cls, path: Path) -> "Config":
        value = read_json(_resolved_absolute(path, "config path"), immutable=True)
        _require_exact_keys(
            value,
            required={
                "schema",
                "state_root",
                "releases_root",
                "active_link",
                "signing_key",
                "intent_attestation_key",
                "command_profiles",
                "operator_status",
                "model_handoff_root",
                "appliance_id",
                "target",
            },
            optional={"immutable_root_denylist"},
            label="supervisor config",
        )
        if value["schema"] != CONFIG_SCHEMA:
            raise SupervisorError("unsupported supervisor config schema")
        extra_raw = value.get("immutable_root_denylist", [])
        if not isinstance(extra_raw, list) or len(extra_raw) > 64:
            raise SupervisorError("invalid immutable_root_denylist")
        extra = tuple(validate_relative_path(item, "immutable deny prefix") for item in extra_raw)
        target = str(value["target"])
        if target not in {"x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"}:
            raise SupervisorError("unsupported appliance target")
        config = cls(
            state_root=_resolved_absolute(Path(str(value["state_root"])), "state_root"),
            releases_root=_resolved_absolute(Path(str(value["releases_root"])), "releases_root"),
            active_link=Path(str(value["active_link"])),
            signing_key=_resolved_absolute(Path(str(value["signing_key"])), "signing_key"),
            intent_attestation_key=_resolved_absolute(
                Path(str(value["intent_attestation_key"])), "intent_attestation_key"
            ),
            command_profiles=_resolved_absolute(
                Path(str(value["command_profiles"])), "command_profiles"
            ),
            operator_status=_resolved_absolute(
                Path(str(value["operator_status"])), "operator_status"
            ),
            model_handoff_root=_resolved_absolute(
                Path(str(value["model_handoff_root"])), "model_handoff_root"
            ),
            appliance_id=_identifier(value["appliance_id"], "appliance_id"),
            target=target,
            extra_denylist=extra,
        )
        if not config.active_link.is_absolute() or config.active_link.name != "current":
            raise SupervisorError("active_link must be an absolute path named current")
        _resolved_absolute(config.active_link.parent, "active_link parent")
        if config.active_link.parent != config.releases_root.parent:
            raise SupervisorError("active_link and releases_root must share one parent")
        if (
            config.state_root == config.releases_root
            or config.state_root in config.releases_root.parents
            or config.releases_root in config.state_root.parents
        ):
            raise SupervisorError("state_root and releases_root must be separate")
        mutable_roots = (config.state_root, config.releases_root, config.active_link.parent)
        for root_of_trust in (
            config.signing_key,
            config.intent_attestation_key,
            config.command_profiles,
        ):
            if any(root_of_trust == root or root in root_of_trust.parents for root in mutable_roots):
                raise SupervisorError("root-of-trust files must be outside mutable state and releases")
        if config.signing_key == config.intent_attestation_key:
            raise SupervisorError("ledger and intent attestation keys must be separate")
        if config.operator_status.name != "operator-status.json":
            raise SupervisorError("operator_status must be an absolute path named operator-status.json")
        if any(
            config.operator_status == root
            or root in config.operator_status.parents
            or config.operator_status in root.parents
            for root in mutable_roots
        ):
            raise SupervisorError("operator_status must remain outside mutable state and releases")
        if config.model_handoff_root.name != "model-handoff":
            raise SupervisorError("model_handoff_root must be an absolute model-handoff directory")
        if any(
            config.model_handoff_root == root
            or root in config.model_handoff_root.parents
            or config.model_handoff_root in root.parents
            for root in mutable_roots
        ):
            raise SupervisorError(
                "model_handoff_root must remain separate from supervisor state and releases"
            )
        return config


@dataclasses.dataclass(frozen=True)
class Candidate:
    candidate_id: str
    base_generation: str
    proposal_sha256: str
    patch_sha256: str
    changed_paths: tuple[str, ...]
    created_at: int
    privilege_envelope: str

    @classmethod
    def parse(cls, value: Mapping[str, Any], extra_denylist: Sequence[str] = ()) -> "Candidate":
        _require_exact_keys(
            value,
            required={
                "schema",
                "candidate_id",
                "base_generation",
                "proposal_sha256",
                "patch_sha256",
                "changed_paths",
                "created_at",
                "privilege_envelope",
            },
            optional=set(),
            label="candidate",
        )
        if value["schema"] != CANDIDATE_SCHEMA:
            raise SupervisorError("unsupported candidate schema")
        envelope = str(value["privilege_envelope"])
        if envelope != "proposal-only:no-execution:v1":
            raise SupervisorError("candidate requested authority outside proposal-only envelope")
        return cls(
            candidate_id=_identifier(value["candidate_id"], "candidate_id"),
            base_generation=_identifier(value["base_generation"], "base_generation"),
            proposal_sha256=_hex64(value["proposal_sha256"], "proposal_sha256"),
            patch_sha256=_hex64(value["patch_sha256"], "patch_sha256"),
            changed_paths=validate_changed_paths(value["changed_paths"], extra_denylist),
            created_at=_timestamp(value["created_at"], "created_at"),
            privilege_envelope=envelope,
        )

    def payload(self) -> dict[str, Any]:
        value = dataclasses.asdict(self)
        value["schema"] = CANDIDATE_SCHEMA
        value["changed_paths"] = list(self.changed_paths)
        return value


@dataclasses.dataclass(frozen=True)
class Build:
    appliance_id: str
    build_id: str
    candidate_id: str
    candidate_sha256: str
    base_generation: str
    generation_id: str
    source_revision: str
    bundle_sha256: str
    tests_sha256: str
    target: str
    created_at: int
    privilege_envelope: str

    @classmethod
    def parse(
        cls,
        value: Mapping[str, Any],
        expected_target: str,
        expected_appliance_id: str,
    ) -> "Build":
        _require_exact_keys(
            value,
            required={
                "schema",
                "appliance_id",
                "build_id",
                "candidate_id",
                "candidate_sha256",
                "base_generation",
                "generation_id",
                "source_revision",
                "bundle_sha256",
                "tests_sha256",
                "target",
                "created_at",
                "privilege_envelope",
            },
            optional=set(),
            label="build",
        )
        if value["schema"] != BUILD_SCHEMA:
            raise SupervisorError("unsupported build schema")
        appliance_id = _identifier(value["appliance_id"], "appliance_id")
        if appliance_id != expected_appliance_id:
            raise SupervisorError("build appliance identity does not match this appliance")
        if value["target"] != expected_target:
            raise SupervisorError("build target does not match appliance")
        revision = str(value["source_revision"])
        if not SOURCE_REVISION_RE.fullmatch(revision):
            raise SupervisorError("invalid source_revision")
        envelope = str(value["privilege_envelope"])
        if envelope != PROFILE_ENVELOPES["build"]:
            raise SupervisorError("build exceeded offline unprivileged envelope")
        return cls(
            appliance_id=appliance_id,
            build_id=_identifier(value["build_id"], "build_id"),
            candidate_id=_identifier(value["candidate_id"], "candidate_id"),
            candidate_sha256=_hex64(value["candidate_sha256"], "candidate_sha256"),
            base_generation=_identifier(value["base_generation"], "base_generation"),
            generation_id=_identifier(value["generation_id"], "generation_id"),
            source_revision=revision,
            bundle_sha256=_hex64(value["bundle_sha256"], "bundle_sha256"),
            tests_sha256=_hex64(value["tests_sha256"], "tests_sha256"),
            target=str(value["target"]),
            created_at=_timestamp(value["created_at"], "created_at"),
            privilege_envelope=envelope,
        )

    def payload(self) -> dict[str, Any]:
        return {"schema": BUILD_SCHEMA, **dataclasses.asdict(self)}


@dataclasses.dataclass(frozen=True)
class ScheduledIntent:
    intent_id: str
    appliance_id: str
    trace_id: str
    session_id: str
    turn_id: str
    response_sha256: str
    terminal_declaration_sha256: str
    candidate_id: str
    candidate_sha256: str
    base_generation: str
    current_generation: str
    observed_at: int
    origin: str
    authorship_status: str
    transport_status: str
    declaration_provenance: str
    fallback: bool
    executor_repair: bool
    operator_harness: bool

    @classmethod
    def parse(
        cls,
        value: Mapping[str, Any],
        now: int,
        expected_appliance_id: str,
        *,
        require_fresh: bool = True,
    ) -> "ScheduledIntent":
        _require_exact_keys(
            value,
            required={
                "schema",
                "intent_id",
                "appliance_id",
                "trace_id",
                "session_id",
                "turn_id",
                "response_sha256",
                "terminal_declaration_sha256",
                "candidate_id",
                "candidate_sha256",
                "base_generation",
                "current_generation",
                "observed_at",
                "origin",
                "authorship_status",
                "transport_status",
                "declaration_provenance",
                "fallback",
                "executor_repair",
                "operator_harness",
            },
            optional=set(),
            label="scheduled model intent",
        )
        if value["schema"] != INTENT_SCHEMA:
            raise SupervisorError("unsupported scheduled model intent schema")
        observed = _timestamp(value["observed_at"], "observed_at")
        if observed > now or (
            require_fresh and now - observed > INTENT_INGEST_MAX_AGE_SECONDS
        ):
            raise SupervisorError("scheduled model intent is not fresh")
        appliance_id = _identifier(value["appliance_id"], "appliance_id")
        if appliance_id != expected_appliance_id:
            raise SupervisorError("scheduled model intent belongs to another appliance")
        if (
            value["origin"] != "scheduled_autonomy"
            or value["authorship_status"] != "genuinely_authored"
            or value["transport_status"] != "authored_completed"
            or value["declaration_provenance"] != "exact_terminal_model_declaration"
            or value["fallback"] is not False
            or value["executor_repair"] is not False
            or value["operator_harness"] is not False
        ):
            raise SupervisorError(
                "promotion requires an exact genuinely-authored scheduled model declaration"
            )
        return cls(
            intent_id=_identifier(value["intent_id"], "intent_id"),
            appliance_id=appliance_id,
            trace_id=_identifier(value["trace_id"], "trace_id"),
            session_id=_identifier(value["session_id"], "session_id"),
            turn_id=_identifier(value["turn_id"], "turn_id"),
            response_sha256=_hex64(value["response_sha256"], "response_sha256"),
            terminal_declaration_sha256=_hex64(
                value["terminal_declaration_sha256"], "terminal_declaration_sha256"
            ),
            candidate_id=_identifier(value["candidate_id"], "candidate_id"),
            candidate_sha256=_hex64(value["candidate_sha256"], "candidate_sha256"),
            base_generation=_identifier(value["base_generation"], "base_generation"),
            current_generation=_identifier(value["current_generation"], "current_generation"),
            observed_at=observed,
            origin=str(value["origin"]),
            authorship_status=str(value["authorship_status"]),
            transport_status=str(value["transport_status"]),
            declaration_provenance=str(value["declaration_provenance"]),
            fallback=False,
            executor_repair=False,
            operator_harness=False,
        )

    def payload(self) -> dict[str, Any]:
        return {"schema": INTENT_SCHEMA, **dataclasses.asdict(self)}


class Signer:
    def __init__(self, path: Path):
        path = _resolved_absolute(path, "signing key")
        info = _lstat_no_link(path, "signing key")
        if not stat.S_ISREG(info.st_mode) or info.st_size != 32:
            raise IntegrityError("signing key must be an exact 32-byte regular file")
        if info.st_uid not in {0, os.geteuid()} or info.st_mode & 0o077:
            raise IntegrityError("signing key must be owner-only and operator controlled")
        self._key = path.read_bytes()
        self.key_id = "hmac-sha256:" + sha256_bytes(self._key)[:16]

    def sign(self, value: bytes) -> str:
        return hmac.new(self._key, value, hashlib.sha256).hexdigest()

    def verify(self, value: bytes, signature: str) -> bool:
        return hmac.compare_digest(self.sign(value), signature)


class IntentAttestor:
    """Verify root-of-trust envelopes; mutable runtime claims are never accepted bare."""

    def __init__(self, path: Path):
        self.signer = Signer(path)

    @property
    def key_id(self) -> str:
        return self.signer.key_id

    def verify_envelope(self, value: Mapping[str, Any], now: int) -> dict[str, Any]:
        """Verify the completion proof and all three exact HMAC-bound layers."""

        _require_exact_keys(
            value,
            required={"schema", "intent_envelope", "authored_completion", "auth"},
            optional=set(),
            label="completed intent envelope",
        )
        if value["schema"] != COMPLETED_INTENT_ENVELOPE_SCHEMA:
            raise IntegrityError("bare or unsupported scheduled intent claim")
        intent_envelope = value["intent_envelope"]
        authored_completion = value["authored_completion"]
        auth = value["auth"]
        if not isinstance(intent_envelope, dict) or not isinstance(authored_completion, dict):
            raise IntegrityError("completed intent proof layers must be objects")
        self._verify_auth(
            auth,
            canonical_bytes(
                {
                    "schema": COMPLETED_INTENT_ENVELOPE_SCHEMA,
                    "intent_envelope": intent_envelope,
                    "authored_completion": authored_completion,
                }
            ),
            "completed intent wrapper",
        )
        intent = self._verify_intent_envelope(intent_envelope, now)
        self._verify_completion(authored_completion, intent_envelope, intent, now)
        return {
            **intent,
            "intent_envelope_sha256": sha256_bytes(canonical_bytes(intent_envelope)),
            "envelope_sha256": sha256_bytes(canonical_bytes(value)),
        }

    def _verify_intent_envelope(
        self, value: Mapping[str, Any], now: int
    ) -> dict[str, Any]:
        _require_exact_keys(
            value,
            required={"schema", "core", "auth"},
            optional=set(),
            label="intent attestor envelope",
        )
        core = value["core"]
        auth = value["auth"]
        if value["schema"] != INTENT_ENVELOPE_SCHEMA or not isinstance(core, dict):
            raise IntegrityError("bare or unsupported scheduled intent claim")
        _require_exact_keys(
            core,
            required={
                "envelope_id",
                "created_at",
                "candidate_sha256",
                "candidate",
                "intent",
            },
            optional=set(),
            label="intent attestor core",
        )
        created_at = _timestamp(core["created_at"], "attestor created_at")
        if created_at > now or now - created_at > INTENT_INGEST_MAX_AGE_SECONDS:
            raise IntegrityError("intent attestor envelope is not fresh")
        _identifier(core["envelope_id"], "envelope_id")
        candidate_sha256 = _hex64(core["candidate_sha256"], "candidate_sha256")
        if not isinstance(core["candidate"], dict) or not isinstance(core["intent"], dict):
            raise IntegrityError("attestor candidate and intent must be objects")
        if sha256_bytes(canonical_bytes(core["candidate"])) != candidate_sha256:
            raise IntegrityError("attestor candidate hash mismatch")
        signed = canonical_bytes({"schema": INTENT_ENVELOPE_SCHEMA, "core": core})
        self._verify_auth(auth, signed, "intent attestor")
        return {
            **core,
            "nested_envelope_sha256": sha256_bytes(canonical_bytes(value)),
        }

    def _verify_completion(
        self,
        value: Mapping[str, Any],
        intent_envelope: Mapping[str, Any],
        verified_intent: Mapping[str, Any],
        now: int,
    ) -> None:
        _require_exact_keys(
            value,
            required={"schema", "core", "core_sha256", "auth"},
            optional=set(),
            label="authored completion envelope",
        )
        core = value["core"]
        auth = value["auth"]
        if value["schema"] != AUTHORED_COMPLETION_ENVELOPE_SCHEMA or not isinstance(core, dict):
            raise IntegrityError("unsupported authored completion proof")
        _require_exact_keys(
            core,
            required={
                "schema",
                "appliance_id",
                "due_nonce",
                "trace_id",
                "session_id",
                "turn_id",
                "response_sha256",
                "transaction_sha256",
                "completed_at_unix_ms",
                "candidate_publication",
                "status",
                "provenance",
            },
            optional=set(),
            label="authored completion core",
        )
        core_bytes = canonical_bytes(core)
        if (
            core["schema"] != AUTHORED_COMPLETION_SCHEMA
            or _hex64(value["core_sha256"], "completion core_sha256")
            != sha256_bytes(core_bytes)
            or core["status"] != "authored_completed"
            or core["provenance"] != "model_authored_runtime_scheduled"
        ):
            raise IntegrityError("authored completion proof fields are not exact")
        self._verify_auth(auth, core_bytes, "authored completion")
        for field in ("due_nonce", "trace_id", "session_id", "turn_id"):
            _identifier(core[field], f"completion {field}")
        for field in ("response_sha256", "transaction_sha256"):
            _hex64(core[field], f"completion {field}")
        completed_at_ms = _timestamp(
            core["completed_at_unix_ms"], "completion completed_at_unix_ms"
        )
        created_at = int(verified_intent["created_at"])
        if (
            completed_at_ms < created_at * 1_000
            or completed_at_ms > (now + 60) * 1_000
            or now * 1_000 - completed_at_ms
            > INTENT_INGEST_MAX_AGE_SECONDS * 1_000
        ):
            raise IntegrityError("authored completion timestamp is not fresh or ordered")
        publication = core["candidate_publication"]
        if not isinstance(publication, dict):
            raise IntegrityError("candidate completion binding is absent")
        _require_exact_keys(
            publication,
            required={
                "intent_envelope_id",
                "intent_envelope_sha256",
                "intent_id",
                "terminal_declaration_sha256",
                "candidate_id",
                "candidate_sha256",
                "base_generation",
            },
            optional=set(),
            label="candidate completion binding",
        )
        for field in ("intent_envelope_id", "intent_id", "candidate_id", "base_generation"):
            _identifier(publication[field], f"completion publication {field}")
        for field in (
            "intent_envelope_sha256",
            "terminal_declaration_sha256",
            "candidate_sha256",
        ):
            _hex64(publication[field], f"completion publication {field}")
        candidate = verified_intent["candidate"]
        intent = verified_intent["intent"]
        exact_joins = (
            (core["appliance_id"], intent["appliance_id"]),
            (core["trace_id"], intent["trace_id"]),
            (core["session_id"], intent["session_id"]),
            (core["turn_id"], intent["turn_id"]),
            (core["response_sha256"], intent["response_sha256"]),
            (publication["intent_envelope_id"], verified_intent["envelope_id"]),
            (
                publication["intent_envelope_sha256"],
                sha256_bytes(canonical_bytes(intent_envelope)),
            ),
            (publication["intent_id"], intent["intent_id"]),
            (
                publication["terminal_declaration_sha256"],
                intent["terminal_declaration_sha256"],
            ),
            (publication["candidate_id"], candidate["candidate_id"]),
            (publication["candidate_sha256"], verified_intent["candidate_sha256"]),
            (publication["candidate_sha256"], intent["candidate_sha256"]),
            (publication["base_generation"], candidate["base_generation"]),
            (publication["base_generation"], intent["base_generation"]),
        )
        if any(left != right for left, right in exact_joins):
            raise IntegrityError(
                "authored completion proof does not match the exact nested intent"
            )

    def _verify_auth(self, value: Any, signed: bytes, label: str) -> None:
        if (
            not isinstance(value, dict)
            or set(value) != {"algorithm", "key_id", "signature"}
            or value.get("algorithm") != "hmac-sha256"
            or value.get("key_id") != self.key_id
            or not HEX64_RE.fullmatch(str(value.get("signature") or ""))
            or not self.signer.verify(signed, str(value.get("signature") or ""))
        ):
            raise IntegrityError(f"{label} authentication failed")


def read_stable_regular(
    path: Path, *, maximum_bytes: int = MAX_JSON_BYTES, owners: set[int] | None = None
) -> bytes:
    """Read one owner-controlled regular file once without following links."""

    expected_owners = {0, os.geteuid()} if owners is None else owners
    before = _lstat_no_link(path, "bounded input")
    if (
        not stat.S_ISREG(before.st_mode)
        or before.st_nlink != 1
        or before.st_uid not in expected_owners
        or before.st_mode & 0o077
        or before.st_size > maximum_bytes
    ):
        raise IntegrityError("bounded input is not an owner-only regular file")
    descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
    try:
        opened = os.fstat(descriptor)
        chunks: list[bytes] = []
        remaining = maximum_bytes + 1
        while remaining > 0:
            chunk = os.read(descriptor, min(64 * 1024, remaining))
            if not chunk:
                break
            chunks.append(chunk)
            remaining -= len(chunk)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    data = b"".join(chunks)
    identity = lambda info: (info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns)
    if (
        len(data) > maximum_bytes
        or len(data) != before.st_size
        or identity(before) != identity(opened)
        or identity(opened) != identity(after)
    ):
        raise IntegrityError("bounded input changed while being read")
    return data


class Ledger:
    def __init__(self, path: Path, signer: Signer, ledger_name: str):
        self.path = path
        self.signer = signer
        self.ledger_name = ledger_name

    def read(self) -> list[dict[str, Any]]:
        try:
            before = os.lstat(self.path)
        except FileNotFoundError:
            return []
        self._validate_identity(before)
        flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
        descriptor = os.open(self.path, flags)
        try:
            fcntl.flock(descriptor, fcntl.LOCK_SH)
            opened = os.fstat(descriptor)
            if self._static_identity(before) != self._static_identity(opened):
                raise IntegrityError("ledger changed before its descriptor was secured")
            data = self._read_descriptor(descriptor, opened.st_size)
            after = os.fstat(descriptor)
            path_after = os.lstat(self.path)
            final = os.fstat(descriptor)
            if (
                self._read_identity(opened) != self._read_identity(after)
                or self._read_identity(after) != self._read_identity(final)
                or self._static_identity(after) != self._static_identity(path_after)
            ):
                raise IntegrityError("ledger changed or was replaced while being read")
        finally:
            try:
                fcntl.flock(descriptor, fcntl.LOCK_UN)
            finally:
                os.close(descriptor)
        return self._decode(data)

    def _decode(self, data: bytes) -> list[dict[str, Any]]:
        records: list[dict[str, Any]] = []
        previous = "0" * 64
        identifiers: set[str] = set()
        if data and not data.endswith(b"\n"):
            raise IntegrityError("ledger ends with an incomplete record")
        for sequence, line in enumerate(data.splitlines(keepends=True), start=1):
            if len(line) > MAX_LEDGER_LINE_BYTES or not line.endswith(b"\n"):
                raise IntegrityError(f"ledger line {sequence} is oversized or incomplete")
            try:
                record = json.loads(line)
            except json.JSONDecodeError as error:
                raise IntegrityError(f"ledger line {sequence} is invalid JSON") from error
            if not isinstance(record, dict):
                raise IntegrityError(f"ledger line {sequence} is not an object")
            core = record.get("core")
            if (
                record.get("schema") != LEDGER_SCHEMA
                or record.get("ledger") != self.ledger_name
                or not isinstance(core, dict)
                or core.get("sequence") != sequence
                or core.get("previous_hash") != previous
            ):
                raise IntegrityError(f"ledger continuity failed at line {sequence}")
            core_hash = sha256_bytes(canonical_bytes(core))
            if record.get("record_hash") != core_hash:
                raise IntegrityError(f"ledger hash failed at line {sequence}")
            auth = record.get("auth")
            signed = canonical_bytes(
                {"ledger": self.ledger_name, "record_hash": core_hash, "schema": LEDGER_SCHEMA}
            )
            if (
                not isinstance(auth, dict)
                or auth.get("algorithm") != "hmac-sha256"
                or auth.get("key_id") != self.signer.key_id
                or not self.signer.verify(signed, str(auth.get("signature") or ""))
            ):
                raise IntegrityError(f"ledger authentication failed at line {sequence}")
            event_id = str(core.get("event_id") or "")
            if not IDENTIFIER_RE.fullmatch(event_id) or event_id in identifiers:
                raise IntegrityError(f"ledger event replay at line {sequence}")
            identifiers.add(event_id)
            records.append(record)
            previous = core_hash
        return records

    def append(self, kind: str, payload: Mapping[str, Any], event_id: str, now: int) -> dict[str, Any]:
        event_id = _identifier(event_id, "event_id")
        ensure_private_dir(self.path.parent)
        try:
            before: os.stat_result | None = os.lstat(self.path)
            self._validate_identity(before)
        except FileNotFoundError:
            before = None
        flags = os.O_RDWR | os.O_CREAT | os.O_APPEND | getattr(os, "O_NOFOLLOW", 0)
        descriptor = os.open(self.path, flags, 0o600)
        try:
            fcntl.flock(descriptor, fcntl.LOCK_EX)
            opened = os.fstat(descriptor)
            self._validate_identity(opened)
            if before is not None and self._static_identity(before) != self._static_identity(opened):
                raise IntegrityError("ledger changed before its append lock was secured")
            records = self._decode(self._read_descriptor(descriptor, opened.st_size))
            if any(record["core"]["event_id"] == event_id for record in records):
                raise IntegrityError(f"replayed ledger event: {event_id}")
            previous = records[-1]["record_hash"] if records else "0" * 64
            core = {
                "sequence": len(records) + 1,
                "previous_hash": previous,
                "event_id": event_id,
                "kind": _identifier(kind, "ledger event kind"),
                "recorded_at": now,
                "payload": dict(payload),
            }
            record_hash = sha256_bytes(canonical_bytes(core))
            signed = canonical_bytes(
                {"ledger": self.ledger_name, "record_hash": record_hash, "schema": LEDGER_SCHEMA}
            )
            record = {
                "schema": LEDGER_SCHEMA,
                "ledger": self.ledger_name,
                "core": core,
                "record_hash": record_hash,
                "auth": {
                    "algorithm": "hmac-sha256",
                    "key_id": self.signer.key_id,
                    "signature": self.signer.sign(signed),
                },
            }
            line = canonical_bytes(record) + b"\n"
            if len(line) > MAX_LEDGER_LINE_BYTES:
                raise IntegrityError("ledger record exceeds its bounded line ceiling")
            if opened.st_size + len(line) > MAX_LEDGER_BYTES:
                raise IntegrityError("ledger append exceeds its bounded byte ceiling")
            offset = 0
            while offset < len(line):
                written = os.write(descriptor, line[offset:])
                if written <= 0:
                    raise IntegrityError("ledger append made no progress")
                offset += written
            os.fsync(descriptor)
            after = os.fstat(descriptor)
            path_after = os.lstat(self.path)
            final = os.fstat(descriptor)
            if (
                self._static_identity(opened) != self._static_identity(after)
                or self._read_identity(after) != self._read_identity(final)
                or self._static_identity(after) != self._static_identity(path_after)
                or final.st_size != opened.st_size + len(line)
            ):
                raise IntegrityError("ledger changed or was replaced during append")
        finally:
            try:
                fcntl.flock(descriptor, fcntl.LOCK_UN)
            finally:
                os.close(descriptor)
        directory = os.open(self.path.parent, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
        try:
            os.fsync(directory)
        finally:
            os.close(directory)
        return record

    def _validate_identity(self, info: os.stat_result) -> None:
        if (
            not stat.S_ISREG(info.st_mode)
            or info.st_nlink != 1
            or info.st_uid not in {0, os.geteuid()}
            or info.st_mode & 0o077
            or info.st_size > MAX_LEDGER_BYTES
        ):
            raise IntegrityError(f"ledger is not an owner-only regular file: {self.path}")

    @staticmethod
    def _static_identity(info: os.stat_result) -> tuple[int, int, int, int, int, int]:
        return (
            info.st_dev,
            info.st_ino,
            info.st_nlink,
            info.st_uid,
            info.st_gid,
            stat.S_IMODE(info.st_mode),
        )

    @classmethod
    def _read_identity(cls, info: os.stat_result) -> tuple[int, ...]:
        return (*cls._static_identity(info), info.st_size, info.st_mtime_ns)

    @staticmethod
    def _read_descriptor(descriptor: int, size: int) -> bytes:
        if size > MAX_LEDGER_BYTES:
            raise IntegrityError("ledger exceeds its bounded byte ceiling")
        os.lseek(descriptor, 0, os.SEEK_SET)
        chunks: list[bytes] = []
        remaining = size
        while remaining > 0:
            chunk = os.read(descriptor, min(64 * 1024, remaining))
            if not chunk:
                raise IntegrityError("ledger ended before its descriptor size")
            chunks.append(chunk)
            remaining -= len(chunk)
        return b"".join(chunks)
