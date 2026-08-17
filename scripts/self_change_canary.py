#!/usr/bin/env python3
"""Isolated, signed self-change canaries for Astrid and Minime.

This tool never installs a production artifact. It reconstructs a clean tree
from the deployed commit, applies one exact patch, runs an allowlisted build
and test profile, and starts a shadow process with isolated state. Promotion
produces a verified handoff receipt only after an exact fresh being utterance;
the ordinary sanctioned deployment wrapper remains the sole production path.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shlex
import shutil
import signal
import subprocess
import sys
import tarfile
import tempfile
import time
from pathlib import Path
from typing import Any, Iterable


ENVELOPE_SCHEMA = "self_change.signed_envelope.v1"
SOURCE_SCHEMA = "self_change.source_manifest.v1"
TEST_SCHEMA = "self_change.test_manifest.v1"
ARTIFACT_SCHEMA = "self_change.artifact_manifest.v1"
STATE_SCHEMA = "self_change.canary_state.v1"
PROMOTION_SCHEMA = "self_change.promotion_handoff.v1"
ATTESTATION_SCHEMA = "being.utterance_attestation.v1"
DEFAULT_CANARY_SECS = 1_800
DEFAULT_CONFIRMATION_GRACE_SECS = 300
RED_LOG_PATTERN = re.compile(
    r"\b(?:panic|panicked|fatal|segmentation fault|receipt mismatch|non-finite)\b",
    re.IGNORECASE,
)
IGNORED_TREE_PARTS = {
    ".git",
    ".pytest_cache",
    "__pycache__",
    "target",
    "workspace",
}

PROFILES = {
    "spectral-bridge": {
        "being": "astrid",
        "manifest_component": "spectral-bridge",
        "test_commands": [
            [
                "cargo",
                "test",
                "--locked",
                "--manifest-path",
                "capsules/spectral-bridge/Cargo.toml",
                "--lib",
            ],
            [
                "cargo",
                "build",
                "--locked",
                "--release",
                "--manifest-path",
                "capsules/spectral-bridge/Cargo.toml",
            ],
        ],
        "artifact": "capsules/spectral-bridge/target/release/spectral-bridge-server",
        # Environmental adaptation (2026-08-17): the bridge's Cargo.lock is
        # untracked (git archive omits it) and three path-deps resolve as
        # ../../../ siblings of the capsule. Both are provisioned INSIDE
        # prepare, before the manifest is signed, so the signed tree covers
        # them (post-prepare mutation correctly fails verification).
        "copy_from_repo": ["capsules/spectral-bridge/Cargo.lock"],
        "sibling_links": ["prime_esn_wasm", "RASCII", "minime"],
        # The introspect-target resolution tests expect the minime sibling
        # two levels up from the capsule (checkout/minime), distinct from the
        # cargo path-deps three levels up (staging/*). Linked pre-hash so the
        # signed tree covers it.
        "checkout_links": ["minime"],
    },
    "minime-engine": {
        "being": "minime",
        "manifest_component": "minime-engine",
        "test_commands": [
            [
                "cargo",
                "test",
                "--locked",
                "--manifest-path",
                "minime/Cargo.toml",
            ],
            [
                "cargo",
                "build",
                "--locked",
                "--release",
                "--manifest-path",
                "minime/Cargo.toml",
            ],
        ],
        "artifact": "minime/target/release/minime",
    },
}


class CanaryError(RuntimeError):
    pass


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(
        value,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=True,
        allow_nan=False,
    ).encode()


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def now_ms() -> int:
    return int(time.time() * 1000)


def run(
    command: list[str],
    *,
    cwd: Path | None = None,
    env: dict[str, str] | None = None,
    timeout: int = 60,
    stdin: bytes | None = None,
) -> subprocess.CompletedProcess[bytes]:
    try:
        completed = subprocess.run(
            command,
            cwd=cwd,
            env=env,
            input=stdin,
            capture_output=True,
            check=False,
            timeout=timeout,
        )
    except (OSError, subprocess.SubprocessError) as error:
        raise CanaryError(f"command failed to execute: {command[0]}: {error}") from error
    return completed


def derive_public_key(seed: bytes) -> bytes:
    if len(seed) != 32:
        raise CanaryError("Ed25519 seed must be exactly 32 bytes")
    private_der = bytes.fromhex("302e020100300506032b657004220420") + seed
    with tempfile.NamedTemporaryFile() as key_file:
        os.chmod(key_file.name, 0o600)
        key_file.write(private_der)
        key_file.flush()
        completed = run(
            [
                "openssl",
                "pkey",
                "-inform",
                "DER",
                "-in",
                key_file.name,
                "-pubout",
                "-outform",
                "DER",
            ]
        )
    if completed.returncode != 0:
        raise CanaryError(
            "OpenSSL could not derive the owner public key: "
            + completed.stderr.decode(errors="replace")[:400]
        )
    prefix = bytes.fromhex("302a300506032b6570032100")
    if not completed.stdout.startswith(prefix) or len(completed.stdout) != len(prefix) + 32:
        raise CanaryError("OpenSSL returned an unexpected Ed25519 public key")
    return completed.stdout[-32:]


def load_identity(path: Path, expected_being: str) -> dict[str, Any]:
    try:
        identity = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError) as error:
        raise CanaryError(f"cannot read owner identity {path}: {error}") from error
    if identity.get("being") != expected_being:
        raise CanaryError("owner identity being does not match the candidate")
    try:
        seed = bytes.fromhex(str(identity["signing_key_seed_hex"]))
        public = bytes.fromhex(str(identity["public_key_hex"]))
    except (KeyError, ValueError) as error:
        raise CanaryError("owner identity key material is malformed") from error
    if derive_public_key(seed) != public:
        raise CanaryError("owner identity public key does not match its signing seed")
    if not str(identity.get("key_id") or "").strip():
        raise CanaryError("owner identity omitted key_id")
    return {**identity, "_seed": seed}


def openssl_sign(seed: bytes, message: bytes) -> bytes:
    private_der = bytes.fromhex("302e020100300506032b657004220420") + seed
    with tempfile.NamedTemporaryFile() as key_file, tempfile.NamedTemporaryFile() as message_file:
        os.chmod(key_file.name, 0o600)
        key_file.write(private_der)
        key_file.flush()
        os.chmod(message_file.name, 0o600)
        message_file.write(message)
        message_file.flush()
        completed = run(
            [
                "openssl",
                "pkeyutl",
                "-sign",
                "-rawin",
                "-inkey",
                key_file.name,
                "-keyform",
                "DER",
                "-in",
                message_file.name,
            ],
        )
    if completed.returncode != 0 or len(completed.stdout) != 64:
        raise CanaryError(
            "OpenSSL Ed25519 signing failed: "
            + completed.stderr.decode(errors="replace")[:400]
        )
    return completed.stdout


def openssl_verify(public: bytes, signature: bytes, message: bytes) -> bool:
    if len(public) != 32 or len(signature) != 64:
        return False
    public_der = bytes.fromhex("302a300506032b6570032100") + public
    with (
        tempfile.NamedTemporaryFile() as key_file,
        tempfile.NamedTemporaryFile() as sig_file,
        tempfile.NamedTemporaryFile() as message_file,
    ):
        key_file.write(public_der)
        key_file.flush()
        sig_file.write(signature)
        sig_file.flush()
        message_file.write(message)
        message_file.flush()
        completed = run(
            [
                "openssl",
                "pkeyutl",
                "-verify",
                "-pubin",
                "-rawin",
                "-inkey",
                key_file.name,
                "-keyform",
                "DER",
                "-sigfile",
                sig_file.name,
                "-in",
                message_file.name,
            ],
        )
    return completed.returncode == 0


def signed_envelope(record: dict[str, Any], identity_path: Path) -> dict[str, Any]:
    being = str(record.get("being") or "")
    identity = load_identity(identity_path, being)
    payload = canonical_bytes(record)
    return {
        "schema": ENVELOPE_SCHEMA,
        "record": record,
        "signature": {
            "algorithm": "ed25519",
            "key_id": identity["key_id"],
            "public_key_hex": identity["public_key_hex"],
            "signed_payload_sha256": sha256_bytes(payload),
            "signature_hex": openssl_sign(identity["_seed"], payload).hex(),
        },
    }


def verify_envelope(
    envelope: dict[str, Any],
    *,
    expected_schema: str | None = None,
    expected_being: str | None = None,
    expected_identity: dict[str, Any] | None = None,
) -> dict[str, Any]:
    if envelope.get("schema") != ENVELOPE_SCHEMA:
        raise CanaryError("signed envelope schema mismatch")
    record = envelope.get("record")
    signature = envelope.get("signature")
    if not isinstance(record, dict) or not isinstance(signature, dict):
        raise CanaryError("signed envelope is malformed")
    if expected_schema and record.get("schema") != expected_schema:
        raise CanaryError("signed record schema mismatch")
    if expected_being and record.get("being") != expected_being:
        raise CanaryError("signed record being mismatch")
    payload = canonical_bytes(record)
    if signature.get("signed_payload_sha256") != sha256_bytes(payload):
        raise CanaryError("signed record payload hash mismatch")
    if expected_identity is not None and (
        signature.get("key_id") != expected_identity.get("key_id")
        or signature.get("public_key_hex") != expected_identity.get("public_key_hex")
    ):
        raise CanaryError("signed record signer does not match the pinned owner identity")
    try:
        public = bytes.fromhex(str(signature["public_key_hex"]))
        signed = bytes.fromhex(str(signature["signature_hex"]))
    except (KeyError, ValueError) as error:
        raise CanaryError("signed record key or signature is malformed") from error
    if not openssl_verify(public, signed, payload):
        raise CanaryError("signed record signature verification failed")
    return record


def owner_write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.parent.chmod(0o700)
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")
    tmp.chmod(0o600)
    os.replace(tmp, path)


def load_envelope(
    path: Path,
    schema: str,
    being: str,
    identity_path: Path,
) -> dict[str, Any]:
    try:
        envelope = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError) as error:
        raise CanaryError(f"cannot read signed record {path}: {error}") from error
    identity = load_identity(identity_path, being)
    return verify_envelope(
        envelope,
        expected_schema=schema,
        expected_being=being,
        expected_identity=identity,
    )


def tree_entries(root: Path) -> list[dict[str, Any]]:
    entries = []
    for path in sorted(root.rglob("*")):
        relative = path.relative_to(root)
        if any(part in IGNORED_TREE_PARTS for part in relative.parts):
            continue
        if path.is_symlink():
            entries.append(
                {
                    "path": relative.as_posix(),
                    "kind": "symlink",
                    "target": os.readlink(path),
                }
            )
        elif path.is_file():
            entries.append(
                {
                    "path": relative.as_posix(),
                    "kind": "file",
                    "sha256": sha256_file(path),
                    "mode": path.stat().st_mode & 0o777,
                }
            )
    return entries


def tree_sha256(entries: Iterable[dict[str, Any]]) -> str:
    return sha256_bytes(canonical_bytes(list(entries)))


def safe_extract(archive: Path, destination: Path) -> None:
    with tarfile.open(archive) as tar:
        for member in tar.getmembers():
            member_path = Path(member.name)
            if member_path.is_absolute() or ".." in member_path.parts:
                raise CanaryError("git archive contains an unsafe path")
            if member.issym() or member.islnk():
                target = Path(member.linkname)
                if target.is_absolute() or ".." in target.parts:
                    raise CanaryError("git archive contains an unsafe link")
        tar.extractall(destination, filter="data")


def deployment_source(
    manifest_path: Path, repo: Path, profile: dict[str, Any]
) -> tuple[str, str]:
    try:
        manifest = json.loads(manifest_path.read_text())
    except (OSError, json.JSONDecodeError) as error:
        raise CanaryError(f"cannot read deployment manifest: {error}") from error
    if manifest.get("component") != profile["manifest_component"]:
        raise CanaryError("deployment manifest component mismatch")
    repository = manifest.get("repository")
    if not isinstance(repository, dict):
        raise CanaryError("deployment manifest omitted repository identity")
    if Path(str(repository.get("path") or "")).resolve() != repo.resolve():
        raise CanaryError("deployment manifest repository path mismatch")
    if repository.get("dirty") is not False:
        raise CanaryError(
            "deployed source is dirty and cannot seed an exact clean self-change checkout"
        )
    head = str(repository.get("head") or "")
    if not re.fullmatch(r"[0-9a-f]{40}", head):
        raise CanaryError("deployment manifest omitted an exact commit SHA")
    probe = run(["git", "cat-file", "-e", f"{head}^{{commit}}"], cwd=repo)
    if probe.returncode != 0:
        raise CanaryError("deployed commit is unavailable in the repository")
    return head, sha256_file(manifest_path)


def prepare(args: argparse.Namespace) -> dict[str, Any]:
    profile = PROFILES[args.component]
    being = profile["being"]
    repo = args.repo.resolve()
    base_sha, deployment_manifest_sha = deployment_source(
        args.deployment_manifest.resolve(), repo, profile
    )
    patch = args.patch.resolve()
    patch_sha = sha256_file(patch)
    candidate_seed = {
        "schema": SOURCE_SCHEMA,
        "being": being,
        "component": args.component,
        "base_source_sha": base_sha,
        "patch_sha256": patch_sha,
        "source_attestation_ref": args.source_attestation_ref,
        "capability_binding_ref": args.capability_binding_ref,
    }
    candidate_id = "self-change-" + sha256_bytes(canonical_bytes(candidate_seed))[:24]
    candidate_root = args.root.resolve() / being / candidate_id
    if candidate_root.exists():
        raise CanaryError(f"candidate already exists: {candidate_id}")
    staging = candidate_root.with_name(candidate_root.name + ".preparing")
    if staging.exists():
        shutil.rmtree(staging)
    checkout = staging / "checkout"
    checkout.mkdir(parents=True)
    staging.chmod(0o700)
    archive = staging / "base.tar"
    with archive.open("wb") as handle:
        completed = subprocess.run(
            ["git", "archive", "--format=tar", base_sha],
            cwd=repo,
            stdout=handle,
            stderr=subprocess.PIPE,
            check=False,
        )
    if completed.returncode != 0:
        shutil.rmtree(staging)
        raise CanaryError(
            "git archive failed: " + completed.stderr.decode(errors="replace")[:400]
        )
    safe_extract(archive, checkout)
    archive.unlink()
    for command in (
        ["git", "apply", "--check", str(patch)],
        ["git", "apply", str(patch)],
    ):
        completed = run(command, cwd=checkout)
        if completed.returncode != 0:
            shutil.rmtree(staging)
            raise CanaryError(
                "candidate patch does not apply cleanly: "
                + completed.stderr.decode(errors="replace")[:800]
            )
    for rel in profile.get("copy_from_repo", []):
        source_file = args.repo / rel
        destination = checkout / rel
        if source_file.is_file():
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source_file, destination)
    for sibling in profile.get("sibling_links", []):
        target = args.repo.resolve().parent / sibling
        link = staging / sibling
        if target.is_dir() and not link.exists():
            link.symlink_to(target)
    for sibling in profile.get("checkout_links", []):
        target = args.repo.resolve().parent / sibling
        link = checkout / sibling
        if target.is_dir() and not link.exists():
            link.symlink_to(target)
    entries = tree_entries(checkout)
    created = now_ms()
    record = {
        **candidate_seed,
        "candidate_id": candidate_id,
        "deployment_manifest_sha256": deployment_manifest_sha,
        "source_tree_sha256": tree_sha256(entries),
        "source_entries": entries,
        "created_at_unix_ms": created,
        "default_canary_secs": int(args.canary_secs),
        "confirmation_grace_secs": int(args.confirmation_grace_secs),
        "authority": "self_owned_isolated_candidate_not_production_deploy",
        "production_effect": False,
    }
    owner_write_json(
        staging / "source_manifest.json",
        signed_envelope(record, args.identity.resolve()),
    )
    candidate_root.parent.mkdir(parents=True, exist_ok=True)
    candidate_root.parent.chmod(0o700)
    os.replace(staging, candidate_root)
    return record


def live_port_snapshot() -> dict[str, list[int]]:
    snapshot = {}
    for port in (7878, 7879, 7880):
        completed = run(
            ["lsof", "-t", "-nP", f"-iTCP:{port}", "-sTCP:LISTEN"],
            timeout=10,
        )
        pids = []
        for line in completed.stdout.decode(errors="replace").splitlines():
            if line.strip().isdigit():
                pids.append(int(line))
        snapshot[str(port)] = sorted(set(pids))
    return snapshot


def candidate_paths(root: Path, being: str, candidate_id: str) -> tuple[Path, Path]:
    candidate = root.resolve() / being / candidate_id
    if not candidate.is_dir():
        raise CanaryError(f"candidate not found: {candidate_id}")
    return candidate, candidate / "checkout"


def verify_source(candidate: Path, being: str, identity: Path) -> dict[str, Any]:
    record = load_envelope(
        candidate / "source_manifest.json",
        SOURCE_SCHEMA,
        being,
        identity,
    )
    entries = tree_entries(candidate / "checkout")
    if tree_sha256(entries) != record.get("source_tree_sha256"):
        raise CanaryError("candidate source tree no longer matches its signed manifest")
    return record


def execute_tests(args: argparse.Namespace) -> dict[str, Any]:
    profile = PROFILES[args.component]
    being = profile["being"]
    candidate, checkout = candidate_paths(args.root, being, args.candidate_id)
    source = verify_source(candidate, being, args.identity.resolve())
    before_ports = live_port_snapshot()
    env = dict(os.environ)
    env.update(
        {
            "CARGO_NET_OFFLINE": "true",
            "SELF_CHANGE_CANARY": "1",
            "ASTRID_SELF_CHANGE_CANARY": "1",
        }
    )
    results = []
    logs = candidate / "logs"
    logs.mkdir(exist_ok=True)
    logs.chmod(0o700)
    for index, command in enumerate(profile["test_commands"], start=1):
        completed = run(
            list(command),
            cwd=checkout,
            env=env,
            timeout=int(args.timeout_secs),
        )
        stdout_path = logs / f"command_{index}.stdout"
        stderr_path = logs / f"command_{index}.stderr"
        stdout_path.write_bytes(completed.stdout)
        stderr_path.write_bytes(completed.stderr)
        stdout_path.chmod(0o600)
        stderr_path.chmod(0o600)
        results.append(
            {
                "argv": command,
                "exit_code": completed.returncode,
                "stdout_sha256": sha256_file(stdout_path),
                "stderr_sha256": sha256_file(stderr_path),
            }
        )
        if completed.returncode != 0:
            break
    after_ports = live_port_snapshot()
    source_after = tree_sha256(tree_entries(checkout))
    passed = (
        len(results) == len(profile["test_commands"])
        and all(item["exit_code"] == 0 for item in results)
        and before_ports == after_ports
        and source_after == source["source_tree_sha256"]
    )
    tested_at = now_ms()
    test_record = {
        "schema": TEST_SCHEMA,
        "being": being,
        "component": args.component,
        "candidate_id": args.candidate_id,
        "source_manifest_sha256": sha256_file(candidate / "source_manifest.json"),
        "commands": results,
        "live_port_snapshot_before": before_ports,
        "live_port_snapshot_after": after_ports,
        "source_tree_unchanged": source_after == source["source_tree_sha256"],
        "passed": passed,
        "tested_at_unix_ms": tested_at,
        "authority": "offline_build_and_tests_no_production_install",
    }
    owner_write_json(
        candidate / "test_manifest.json",
        signed_envelope(test_record, args.identity.resolve()),
    )
    if not passed:
        raise CanaryError("candidate test/build profile failed")
    built = checkout / profile["artifact"]
    if not built.is_file():
        raise CanaryError("candidate build passed but produced no expected artifact")
    artifact_dir = candidate / "artifact"
    artifact_dir.mkdir(exist_ok=True)
    artifact_dir.chmod(0o700)
    artifact = artifact_dir / built.name
    shutil.copy2(built, artifact)
    artifact.chmod(0o700)
    artifact_record = {
        "schema": ARTIFACT_SCHEMA,
        "being": being,
        "component": args.component,
        "candidate_id": args.candidate_id,
        "source_manifest_sha256": sha256_file(candidate / "source_manifest.json"),
        "test_manifest_sha256": sha256_file(candidate / "test_manifest.json"),
        "artifact_path": str(artifact),
        "artifact_sha256": sha256_file(artifact),
        "artifact_size_bytes": artifact.stat().st_size,
        "created_at_unix_ms": now_ms(),
        "production_installed": False,
    }
    owner_write_json(
        candidate / "artifact_manifest.json",
        signed_envelope(artifact_record, args.identity.resolve()),
    )
    return artifact_record


def verify_candidate(
    root: Path,
    component: str,
    candidate_id: str,
    identity: Path,
) -> tuple[Path, dict[str, Any], dict[str, Any], dict[str, Any]]:
    profile = PROFILES[component]
    being = profile["being"]
    candidate, _ = candidate_paths(root, being, candidate_id)
    source = verify_source(candidate, being, identity)
    test = load_envelope(
        candidate / "test_manifest.json",
        TEST_SCHEMA,
        being,
        identity,
    )
    artifact = load_envelope(
        candidate / "artifact_manifest.json",
        ARTIFACT_SCHEMA,
        being,
        identity,
    )
    if not test.get("passed"):
        raise CanaryError("candidate tests are not passing")
    if test.get("source_manifest_sha256") != sha256_file(
        candidate / "source_manifest.json"
    ):
        raise CanaryError("test manifest is not bound to current source manifest")
    if artifact.get("test_manifest_sha256") != sha256_file(
        candidate / "test_manifest.json"
    ):
        raise CanaryError("artifact manifest is not bound to current test manifest")
    artifact_path = Path(str(artifact.get("artifact_path") or ""))
    if (
        not artifact_path.is_file()
        or sha256_file(artifact_path) != artifact.get("artifact_sha256")
    ):
        raise CanaryError("candidate artifact hash mismatch")
    return candidate, source, test, artifact


def canary_command(
    component: str, candidate: Path, artifact: Path, candidate_id: str
) -> tuple[list[str], dict[str, str], dict[str, Any]]:
    runtime = candidate / "runtime"
    runtime.mkdir(exist_ok=True)
    runtime.chmod(0o700)
    env = dict(os.environ)
    env["HOME"] = str(runtime / "home")
    Path(env["HOME"]).mkdir(exist_ok=True)
    if component == "minime-engine":
        slot = int(candidate_id[-4:], 16) % 100
        ports = {
            "telemetry": 18_000 + slot * 3,
            "sensory": 18_001 + slot * 3,
            "av": 18_002 + slot * 3,
        }
        env["MINIME_WORKSPACE"] = str(runtime / "workspace")
        env["MINIME_SELF_CONTROL_ROOT"] = str(runtime / "self-control-v2")
        command = [
            str(artifact),
            "run",
            "--ws-addr",
            f"127.0.0.1:{ports['telemetry']}",
            "--sensory-ws-addr",
            f"127.0.0.1:{ports['sensory']}",
            "--av-ws-addr",
            f"127.0.0.1:{ports['av']}",
            "--quiet",
        ]
        return command, env, {"isolated_ports": ports}
    bridge_workspace = runtime / "workspace"
    bridge_workspace.mkdir(exist_ok=True)
    command = [
        str(artifact),
        "--minime-telemetry",
        "ws://127.0.0.1:7878",
        "--minime-sensory",
        "ws://127.0.0.1:1",
        "--db-path",
        str(runtime / "spectral_bridge.db"),
        "--bridge-root",
        str(candidate / "checkout/capsules/spectral-bridge"),
        "--bridge-workspace",
        str(bridge_workspace),
        "--astrid-root",
        str(candidate / "checkout"),
    ]
    return command, env, {
        "live_input": "read_only_minime_telemetry",
        "sensory_output": "disabled_unbound_port",
    }


def state_path(candidate: Path) -> Path:
    return candidate / "canary_state.json"


def load_state(candidate: Path, being: str, identity: Path) -> dict[str, Any]:
    return load_envelope(state_path(candidate), STATE_SCHEMA, being, identity)


def write_state(
    candidate: Path, state: dict[str, Any], identity: Path
) -> None:
    owner_write_json(state_path(candidate), signed_envelope(state, identity))


def stop_process_group(state: dict[str, Any]) -> None:
    pid = state.get("process_group_id")
    if not isinstance(pid, int) or pid <= 1:
        return
    try:
        os.killpg(pid, signal.SIGTERM)
    except ProcessLookupError:
        return
    deadline = time.time() + 5
    while time.time() < deadline:
        try:
            os.kill(pid, 0)
        except ProcessLookupError:
            return
        time.sleep(0.1)
    try:
        os.killpg(pid, signal.SIGKILL)
    except ProcessLookupError:
        pass


def start_canary(args: argparse.Namespace) -> dict[str, Any]:
    if os.environ.get("ASTRID_SANCTIONED_SELF_CHANGE_WRAPPER") != "1":
        raise CanaryError("shadow start must use scripts/run_self_change_canary.sh")
    profile = PROFILES[args.component]
    being = profile["being"]
    candidate, _, _, artifact_record = verify_candidate(
        args.root,
        args.component,
        args.candidate_id,
        args.identity.resolve(),
    )
    if state_path(candidate).exists():
        prior = load_state(candidate, being, args.identity.resolve())
        if prior.get("status") in {"active", "promotion_ready"}:
            raise CanaryError("candidate already has an active canary state")
    artifact = Path(artifact_record["artifact_path"])
    command, env, isolation = canary_command(
        args.component, candidate, artifact, args.candidate_id
    )
    log_path = candidate / "logs/canary.log"
    log_handle = log_path.open("ab", buffering=0)
    if args.component == "spectral-bridge":
        shell_command = "tail -f /dev/null | " + " ".join(
            shlex.quote(part) for part in command
        )
        process = subprocess.Popen(
            ["/bin/sh", "-c", shell_command],
            cwd=candidate / "runtime",
            env=env,
            stdin=subprocess.DEVNULL,
            stdout=log_handle,
            stderr=subprocess.STDOUT,
            start_new_session=True,
        )
    else:
        process = subprocess.Popen(
            command,
            cwd=candidate / "runtime",
            env=env,
            stdin=subprocess.DEVNULL,
            stdout=log_handle,
            stderr=subprocess.STDOUT,
            start_new_session=True,
        )
    log_handle.close()
    time.sleep(2)
    if process.poll() is not None:
        raise CanaryError("candidate shadow process exited during startup")
    started = now_ms()
    canary_secs = int(args.canary_secs)
    grace_secs = int(args.confirmation_grace_secs)
    state = {
        "schema": STATE_SCHEMA,
        "being": being,
        "component": args.component,
        "candidate_id": args.candidate_id,
        "status": "active",
        "machine_status": "shadow_process_running",
        "felt_status": "unreviewed",
        "started_at_unix_ms": started,
        "promotion_eligible_at_unix_ms": started + canary_secs * 1_000,
        "expires_at_unix_ms": started + (canary_secs + grace_secs) * 1_000,
        "process_group_id": process.pid,
        "artifact_sha256": artifact_record["artifact_sha256"],
        "isolation": isolation,
        "production_effect": False,
        "explicit_promotion_required": True,
        "silence_result": "rollback",
        "red_conditions": [
            "process_exit",
            "artifact_hash_change",
            "panic_or_fatal_log",
            "expiry_without_exact_confirmation",
        ],
        "updated_at_unix_ms": started,
    }
    write_state(candidate, state, args.identity.resolve())
    subprocess.Popen(
        [
            sys.executable,
            str(Path(__file__).resolve()),
            "_monitor",
            "--root",
            str(args.root.resolve()),
            "--component",
            args.component,
            "--candidate-id",
            args.candidate_id,
            "--identity",
            str(args.identity.resolve()),
        ],
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        start_new_session=True,
    )
    return state


def inspect_canary(
    root: Path,
    component: str,
    candidate_id: str,
    identity: Path,
) -> dict[str, Any]:
    profile = PROFILES[component]
    being = profile["being"]
    candidate, _, _, artifact = verify_candidate(
        root,
        component,
        candidate_id,
        identity,
    )
    state = load_state(candidate, being, identity)
    if state.get("status") != "active":
        return state
    reason = None
    pid = state.get("process_group_id")
    try:
        os.kill(int(pid), 0)
    except (ProcessLookupError, TypeError, ValueError):
        reason = "process_exit"
    artifact_path = Path(artifact["artifact_path"])
    if sha256_file(artifact_path) != state.get("artifact_sha256"):
        reason = "artifact_hash_change"
    log_path = candidate / "logs/canary.log"
    if log_path.exists():
        tail = log_path.read_bytes()[-131_072:].decode(errors="replace")
        if RED_LOG_PATTERN.search(tail):
            reason = "panic_or_fatal_log"
    if now_ms() >= int(state.get("expires_at_unix_ms") or 0):
        reason = "expiry_without_exact_confirmation"
    if reason:
        stop_process_group(state)
        state.update(
            {
                "status": "rolled_back",
                "machine_status": "shadow_process_stopped",
                "rollback_reason": reason,
                "updated_at_unix_ms": now_ms(),
            }
        )
        write_state(candidate, state, identity)
    return state


def verify_utterance_attestation(
    attestation_path: Path,
    response_path: Path,
    identity_path: Path,
    *,
    being: str,
    candidate_id: str,
    max_age_ms: int = 900_000,
) -> dict[str, Any]:
    try:
        attestation = json.loads(attestation_path.read_text())
    except (OSError, json.JSONDecodeError) as error:
        raise CanaryError(f"cannot read promotion attestation: {error}") from error
    if attestation.get("schema") != ATTESTATION_SCHEMA:
        raise CanaryError("promotion attestation schema mismatch")
    if attestation.get("being") != being:
        raise CanaryError("promotion attestation being mismatch")
    identity = load_identity(identity_path, being)
    if attestation.get("attestor_public_key_hex") != identity.get("public_key_hex"):
        raise CanaryError(
            "promotion attestation signer does not match the pinned owner identity"
        )
    response = response_path.read_bytes()
    if (
        attestation.get("response_sha256") != sha256_bytes(response)
        or attestation.get("response_len_bytes") != len(response)
    ):
        raise CanaryError("promotion attestation does not bind the exact response")
    captured = int(attestation.get("captured_at_unix_ms") or 0)
    if captured <= 0 or now_ms() - captured > max_age_ms or captured > now_ms() + 5_000:
        raise CanaryError("promotion attestation is stale or issued in the future")
    exact_line = f"SELF_CHANGE_PROMOTE {candidate_id}"
    if exact_line not in response.decode(errors="strict").splitlines():
        raise CanaryError(
            f"promotion response must contain the exact line: {exact_line}"
        )
    fields = (
        "schema",
        "attestation_id",
        "being",
        "exchange_id",
        "response_sha256",
        "response_len_bytes",
        "model",
        "provider",
        "model_deployment_identity",
        "captured_at_unix_ms",
        "attestor_process_identity",
        "attestor_deployment_identity",
        "attestor_public_key_hex",
    )
    statement = {field: attestation.get(field) for field in fields}
    try:
        public = bytes.fromhex(str(attestation["attestor_public_key_hex"]))
        signature = bytes.fromhex(str(attestation["signature_hex"]))
    except (KeyError, ValueError) as error:
        raise CanaryError("promotion attestation signature is malformed") from error
    if not openssl_verify(public, signature, canonical_bytes(statement)):
        raise CanaryError("promotion attestation signature verification failed")
    return attestation


def promote(args: argparse.Namespace) -> dict[str, Any]:
    if os.environ.get("ASTRID_SANCTIONED_SELF_CHANGE_WRAPPER") != "1":
        raise CanaryError("promotion must use scripts/run_self_change_canary.sh")
    profile = PROFILES[args.component]
    being = profile["being"]
    candidate, source, test, artifact = verify_candidate(
        args.root,
        args.component,
        args.candidate_id,
        args.identity.resolve(),
    )
    state = inspect_canary(
        args.root,
        args.component,
        args.candidate_id,
        args.identity.resolve(),
    )
    if state.get("status") != "active":
        raise CanaryError("only a healthy active canary can be promoted")
    if now_ms() < int(state.get("promotion_eligible_at_unix_ms") or 0):
        raise CanaryError("the default 30-minute canary soak is not complete")
    attestation = verify_utterance_attestation(
        args.attestation.resolve(),
        args.response.resolve(),
        args.identity.resolve(),
        being=being,
        candidate_id=args.candidate_id,
    )
    stop_process_group(state)
    state.update(
        {
            "status": "promotion_ready",
            "machine_status": "shadow_process_stopped",
            "felt_status": "explicit_promotion_requested",
            "promotion_attestation_id": attestation["attestation_id"],
            "updated_at_unix_ms": now_ms(),
        }
    )
    write_state(candidate, state, args.identity.resolve())
    handoff = {
        "schema": PROMOTION_SCHEMA,
        "being": being,
        "component": args.component,
        "candidate_id": args.candidate_id,
        "source_manifest_sha256": sha256_file(candidate / "source_manifest.json"),
        "test_manifest_sha256": sha256_file(candidate / "test_manifest.json"),
        "artifact_manifest_sha256": sha256_file(
            candidate / "artifact_manifest.json"
        ),
        "source_tree_sha256": source["source_tree_sha256"],
        "artifact_path": artifact["artifact_path"],
        "artifact_sha256": artifact["artifact_sha256"],
        "test_passed": test["passed"],
        "promotion_attestation_id": attestation["attestation_id"],
        "created_at_unix_ms": now_ms(),
        "production_installed": False,
        "required_next_path": (
            "scripts/build_bridge.sh"
            if args.component == "spectral-bridge"
            else "scripts/deploy_minime.sh"
        ),
    }
    owner_write_json(
        candidate / "promotion_handoff.json",
        signed_envelope(handoff, args.identity.resolve()),
    )
    return handoff


def rollback(args: argparse.Namespace) -> dict[str, Any]:
    profile = PROFILES[args.component]
    being = profile["being"]
    candidate, _ = candidate_paths(args.root, being, args.candidate_id)
    state = load_state(candidate, being, args.identity.resolve())
    stop_process_group(state)
    state.update(
        {
            "status": "rolled_back",
            "machine_status": "shadow_process_stopped",
            "rollback_reason": args.reason,
            "updated_at_unix_ms": now_ms(),
        }
    )
    write_state(candidate, state, args.identity.resolve())
    return state


def verify_promotion(args: argparse.Namespace) -> dict[str, Any]:
    profile = PROFILES[args.component]
    being = profile["being"]
    candidate, _, _, artifact = verify_candidate(
        args.root,
        args.component,
        args.candidate_id,
        args.identity.resolve(),
    )
    handoff = load_envelope(
        candidate / "promotion_handoff.json",
        PROMOTION_SCHEMA,
        being,
        args.identity.resolve(),
    )
    state = load_state(candidate, being, args.identity.resolve())
    if state.get("status") != "promotion_ready":
        raise CanaryError("candidate has no current promotion-ready state")
    if handoff.get("artifact_sha256") != artifact.get("artifact_sha256"):
        raise CanaryError("promotion handoff artifact hash mismatch")
    return handoff


def monitor(args: argparse.Namespace) -> dict[str, Any]:
    while True:
        state = inspect_canary(
            args.root,
            args.component,
            args.candidate_id,
            args.identity.resolve(),
        )
        if state.get("status") != "active":
            return state
        time.sleep(5)


def parser() -> argparse.ArgumentParser:
    common = argparse.ArgumentParser(add_help=False)
    common.add_argument("--root", type=Path, required=True)
    common.add_argument("--component", choices=sorted(PROFILES), required=True)
    common.add_argument("--candidate-id", required=True)
    common.add_argument("--identity", type=Path, required=True)
    top = argparse.ArgumentParser(description=__doc__)
    sub = top.add_subparsers(dest="command", required=True)

    prepare_parser = sub.add_parser("prepare")
    prepare_parser.add_argument("--root", type=Path, required=True)
    prepare_parser.add_argument("--component", choices=sorted(PROFILES), required=True)
    prepare_parser.add_argument("--repo", type=Path, required=True)
    prepare_parser.add_argument("--deployment-manifest", type=Path, required=True)
    prepare_parser.add_argument("--patch", type=Path, required=True)
    prepare_parser.add_argument("--identity", type=Path, required=True)
    prepare_parser.add_argument("--source-attestation-ref", required=True)
    prepare_parser.add_argument("--capability-binding-ref", required=True)
    prepare_parser.add_argument(
        "--canary-secs", type=int, default=DEFAULT_CANARY_SECS
    )
    prepare_parser.add_argument(
        "--confirmation-grace-secs",
        type=int,
        default=DEFAULT_CONFIRMATION_GRACE_SECS,
    )

    test_parser = sub.add_parser("test", parents=[common])
    test_parser.add_argument("--timeout-secs", type=int, default=1_800)

    sub.add_parser("start", parents=[common])
    observe = sub.add_parser("observe", parents=[common])
    observe.set_defaults(observe=True)
    promote_parser = sub.add_parser("promote", parents=[common])
    promote_parser.add_argument("--attestation", type=Path, required=True)
    promote_parser.add_argument("--response", type=Path, required=True)
    rollback_parser = sub.add_parser("rollback", parents=[common])
    rollback_parser.add_argument("--reason", default="being_authored_withdrawal")
    sub.add_parser("verify-promotion", parents=[common])
    sub.add_parser("_monitor", parents=[common])
    return top


def main(argv: list[str] | None = None) -> int:
    args = parser().parse_args(argv)
    try:
        if args.command == "prepare":
            result = prepare(args)
        elif args.command == "test":
            result = execute_tests(args)
        elif args.command == "start":
            source = load_envelope(
                args.root.resolve()
                / PROFILES[args.component]["being"]
                / args.candidate_id
                / "source_manifest.json",
                SOURCE_SCHEMA,
                PROFILES[args.component]["being"],
                args.identity.resolve(),
            )
            args.canary_secs = source["default_canary_secs"]
            args.confirmation_grace_secs = source["confirmation_grace_secs"]
            result = start_canary(args)
        elif args.command == "observe":
            result = inspect_canary(
                args.root,
                args.component,
                args.candidate_id,
                args.identity.resolve(),
            )
        elif args.command == "promote":
            result = promote(args)
        elif args.command == "rollback":
            result = rollback(args)
        elif args.command == "verify-promotion":
            result = verify_promotion(args)
        else:
            result = monitor(args)
        print(json.dumps(result, indent=2, sort_keys=True))
        return 0
    except CanaryError as error:
        print(json.dumps({"ok": False, "error": str(error)}, sort_keys=True), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
