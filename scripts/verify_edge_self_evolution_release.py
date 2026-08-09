#!/usr/bin/env python3
"""Verify and optionally transfer a CPU-edge bootstrap from a trusted operator host.

The checksums inside the bootstrap are integrity metadata, not a publisher
identity.  This helper keeps the publisher trust decision outside the archive:
GitHub CLI verifies the OIDC/Sigstore provenance against an exact repository,
workflow, tag ref, and source commit before any bundled program is executed.
"""

from __future__ import annotations

import argparse
import base64
import datetime as dt
import hashlib
import json
import os
import re
import shlex
import stat
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Sequence


SCHEMA = "astrid.edge.operator_release_verification.v1"
PREDICATE = "https://slsa.dev/provenance/v1"
DEFAULT_REPOSITORY = "mikedotexe/astrid"
MAX_ARTIFACT_BYTES = 8 * 1024 * 1024 * 1024
MAX_ATTESTATION_OUTPUT_BYTES = 8 * 1024 * 1024
COMMIT_DIGEST = re.compile(r"[0-9a-f]{40}\Z")
SAFE_TAG_REF = re.compile(r"refs/tags/v[A-Za-z0-9][A-Za-z0-9._-]{0,126}\Z")
SAFE_ARTIFACT = re.compile(
    r"astrid-edge-self-evolution-(?P<version>[A-Za-z0-9][A-Za-z0-9._-]{0,126})"
    r"-x86_64-unknown-linux-gnu\.tar\.gz\Z"
)
SIGNER_WORKFLOW = f"{DEFAULT_REPOSITORY}/.github/workflows/release.yml"
CERT_IDENTITY_PREFIX = f"https://github.com/{SIGNER_WORKFLOW}@"
OIDC_ISSUER = "https://token.actions.githubusercontent.com"
ROOT_HANDOFF_SCHEMA = "astrid.edge.self_evolution_operator_handoff.v1"


class VerificationError(RuntimeError):
    """A release failed a trusted-operator verification invariant."""


# This program is transported as an argument over the already-authenticated SSH
# connection and executed by the host's fixed Python under interactive sudo.
# It never executes from the user-writable archive.  Root first copies and
# hashes the exact attested bytes, safely extracts them into a new O_EXCL
# handoff, writes the immutable receipt expected by the bundled installer, and
# only then execs that root-owned installer.
ROOT_HANDOFF_PROGRAM = r'''from __future__ import annotations
import datetime as dt
import hashlib
import json
import os
import re
import shutil
import stat
import sys
import tarfile
from pathlib import Path, PurePosixPath

SCHEMA = "astrid.edge.self_evolution_operator_handoff.v1"
AUTHORITY = "trusted_operator_verified_github_oidc_sigstore_release_handoff"
REPOSITORY = "mikedotexe/astrid"
SIGNER_WORKFLOW = ".github/workflows/release.yml"
BASE = Path("/var/lib/astrid-edge-bootstrap")
MAX_EXPANDED = 8 * 1024 * 1024 * 1024
MAX_MEMBER = 4 * 1024 * 1024 * 1024
MAX_MEMBERS = 64
REQUIRED_UID = 0
REQUIRED_GID = 0

def fail(message):
    raise RuntimeError(message)

def regular(path, expected_uid=None, expected_mode=None):
    metadata = path.lstat()
    if path.is_symlink() or not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1:
        fail(f"not a single-linked regular file: {path}")
    if expected_uid is not None and metadata.st_uid != expected_uid:
        fail(f"file has wrong owner: {path}")
    if expected_mode is not None and stat.S_IMODE(metadata.st_mode) != expected_mode:
        fail(f"file has wrong mode: {path}")
    return metadata

def fsync_directory(path):
    descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)

if len(sys.argv) != 8:
    fail("root handoff argument shape is not exact")
source = Path(sys.argv[1])
expected_sha = sys.argv[2]
expected_bytes = int(sys.argv[3])
root_name = sys.argv[4]
appliance = sys.argv[5]
source_tag = sys.argv[6]
source_commit = sys.argv[7]
if os.geteuid() != REQUIRED_UID:
    fail("root handoff requires euid 0")
if re.fullmatch(r"[0-9a-f]{64}", expected_sha) is None:
    fail("outer digest is malformed")
if not 0 < expected_bytes <= 8 * 1024 * 1024 * 1024:
    fail("outer archive size is invalid")
if re.fullmatch(r"astrid-edge-self-evolution-[A-Za-z0-9][A-Za-z0-9._-]{0,126}-x86_64-unknown-linux-gnu", root_name) is None:
    fail("extracted root name is invalid")
if appliance not in {"avado", "icp"}:
    fail("appliance is invalid")
if source_tag != "v" + root_name.removeprefix("astrid-edge-self-evolution-").removesuffix("-x86_64-unknown-linux-gnu"):
    fail("source tag does not match the archive version")
if re.fullmatch(r"[0-9a-f]{40}", source_commit) is None:
    fail("source commit is malformed")
if not source.is_absolute():
    fail("source archive path is not absolute")
if source.name != root_name + ".tar.gz" or source.parent.name != expected_sha:
    fail("source archive path is not bound to its digest and root name")

base_parent = BASE.parent.lstat()
if (
    BASE.parent.is_symlink()
    or not stat.S_ISDIR(base_parent.st_mode)
    or base_parent.st_uid != REQUIRED_UID
    or base_parent.st_mode & 0o022
):
    fail("root handoff parent identity is unsafe")
if BASE.exists() or BASE.is_symlink():
    metadata = BASE.lstat()
    if BASE.is_symlink() or not stat.S_ISDIR(metadata.st_mode) or metadata.st_uid != REQUIRED_UID or metadata.st_gid != REQUIRED_GID or stat.S_IMODE(metadata.st_mode) != 0o700:
        fail("root handoff base identity is unsafe")
else:
    os.mkdir(BASE, 0o700)
    os.chown(BASE, REQUIRED_UID, REQUIRED_GID)
    fsync_directory(BASE.parent)

handoff = BASE / expected_sha
archive_path = handoff / "release.tar.gz"
extracted_root = handoff / root_name
receipt_path = handoff / "operator-handoff.json"
created = False
try:
    os.mkdir(handoff, 0o700)
    created = True
    source_before = regular(source)
    source_descriptor = os.open(source, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
    destination_descriptor = os.open(
        archive_path,
        os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0),
        0o400,
    )
    digest = hashlib.sha256()
    copied = 0
    try:
        source_opened = os.fstat(source_descriptor)
        if (source_opened.st_dev, source_opened.st_ino, source_opened.st_size) != (source_before.st_dev, source_before.st_ino, source_before.st_size):
            fail("source archive changed before root copy")
        while True:
            chunk = os.read(source_descriptor, 1024 * 1024)
            if not chunk:
                break
            copied += len(chunk)
            if copied > expected_bytes:
                fail("source archive exceeded expected size")
            digest.update(chunk)
            view = memoryview(chunk)
            while view:
                view = view[os.write(destination_descriptor, view):]
        os.fsync(destination_descriptor)
        source_after = os.fstat(source_descriptor)
    finally:
        os.close(destination_descriptor)
        os.close(source_descriptor)
    if (source_after.st_dev, source_after.st_ino, source_after.st_size, source_after.st_mtime_ns) != (source_before.st_dev, source_before.st_ino, source_before.st_size, source_before.st_mtime_ns):
        fail("source archive changed during root copy")
    if copied != expected_bytes or digest.hexdigest() != expected_sha:
        fail("root-copied archive does not match the attested bytes")
    os.chown(archive_path, REQUIRED_UID, REQUIRED_GID)
    os.chmod(archive_path, 0o400)

    seen = set()
    total = 0
    members = []
    with tarfile.open(archive_path, mode="r:gz") as archive:
        for member in archive:
            if len(members) >= MAX_MEMBERS:
                fail("outer archive has too many members")
            pure = PurePosixPath(member.name)
            canonical = pure.as_posix()
            if (
                pure.is_absolute()
                or not pure.parts
                or pure.parts[0] != root_name
                or any(part in {"", ".", ".."} or part.startswith(".") for part in pure.parts)
                or (member.name != canonical and not (member.isdir() and member.name == canonical + "/"))
                or canonical in seen
                or not (member.isdir() or member.isfile())
                or getattr(member, "sparse", None)
                or member.mode & ~0o700
                or member.mode & 0o077
            ):
                fail("outer archive contains an unsafe path, type, alias, or mode")
            seen.add(canonical)
            if member.isfile():
                if member.size <= 0 or member.size > MAX_MEMBER:
                    fail("outer archive member size is unsafe")
                total += member.size
                if total > MAX_EXPANDED:
                    fail("outer archive expanded size is unsafe")
            members.append((member, pure))
        for member, pure in members:
            output = handoff.joinpath(*pure.parts)
            if member.isdir():
                output.mkdir(mode=member.mode, parents=True, exist_ok=True)
                if output.is_symlink() or not output.is_dir():
                    fail("outer directory extraction collided")
                os.chown(output, REQUIRED_UID, REQUIRED_GID)
                os.chmod(output, member.mode)
                continue
            output.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
            stream = archive.extractfile(member)
            if stream is None:
                fail("outer member body is absent")
            descriptor = os.open(
                output,
                os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0),
                member.mode,
            )
            written = 0
            try:
                while True:
                    chunk = stream.read(1024 * 1024)
                    if not chunk:
                        break
                    written += len(chunk)
                    if written > member.size:
                        fail("outer member exceeded its declaration")
                    view = memoryview(chunk)
                    while view:
                        view = view[os.write(descriptor, view):]
                os.fsync(descriptor)
                os.fchown(descriptor, REQUIRED_UID, REQUIRED_GID)
                os.fchmod(descriptor, member.mode)
            finally:
                os.close(descriptor)
            if written != member.size:
                fail("outer member size changed during extraction")

    root_metadata = extracted_root.lstat()
    if extracted_root.is_symlink() or not stat.S_ISDIR(root_metadata.st_mode):
        fail("exact extracted root is absent")
    for path in extracted_root.rglob("*"):
        metadata = path.lstat()
        if path.is_symlink() or metadata.st_uid != REQUIRED_UID or metadata.st_gid != REQUIRED_GID or metadata.st_mode & 0o077:
            fail("extracted content escaped root-only policy")
        if stat.S_ISREG(metadata.st_mode) and metadata.st_nlink != 1:
            fail("extracted content contains a hard link")
        if not (stat.S_ISREG(metadata.st_mode) or stat.S_ISDIR(metadata.st_mode)):
            fail("extracted content contains a special file")
    receipt = {
        "schema": SCHEMA,
        "authority": AUTHORITY,
        "repository": REPOSITORY,
        "signer_workflow": SIGNER_WORKFLOW,
        "source_tag": source_tag,
        "source_commit": source_commit,
        "outer_archive_sha256": expected_sha,
        "outer_archive_bytes": expected_bytes,
        "outer_archive_path": str(archive_path),
        "extracted_root": str(extracted_root),
        "extracted_root_device": root_metadata.st_dev,
        "extracted_root_inode": root_metadata.st_ino,
        "appliance": appliance,
    }
    body = json.dumps(receipt, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("ascii") + b"\n"
    descriptor = os.open(
        receipt_path,
        os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0),
        0o400,
    )
    try:
        written = 0
        while written < len(body):
            written += os.write(descriptor, body[written:])
        os.fsync(descriptor)
        os.fchown(descriptor, REQUIRED_UID, REQUIRED_GID)
        os.fchmod(descriptor, 0o400)
    finally:
        os.close(descriptor)
    fsync_directory(handoff)
    fsync_directory(BASE)
except Exception:
    if created and handoff.is_dir() and not handoff.is_symlink():
        shutil.rmtree(handoff)
        fsync_directory(BASE)
    raise

installer = extracted_root / "install"
regular(installer, expected_uid=REQUIRED_UID, expected_mode=0o500)
os.execve(
    "/usr/bin/bash",
    [
        "/usr/bin/bash",
        str(installer),
        "--operator-handoff",
        str(receipt_path),
        "--appliance",
        appliance,
    ],
    {"PATH": "/usr/sbin:/usr/bin:/sbin:/bin"},
)
'''


@dataclass(frozen=True)
class FileIdentity:
    device: int
    inode: int
    size: int
    mtime_ns: int
    mode: int
    sha256: str


def stable_sha256(path: Path) -> FileIdentity:
    """Hash one regular single-linked file and reject an identity race."""

    before = path.lstat()
    if path.is_symlink() or not stat.S_ISREG(before.st_mode) or before.st_nlink != 1:
        raise VerificationError("artifact must be a single-linked regular file")
    if before.st_size <= 0 or before.st_size > MAX_ARTIFACT_BYTES:
        raise VerificationError("artifact size is outside the trusted operator bound")
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(path, flags)
    try:
        opened = os.fstat(descriptor)
        if (
            opened.st_dev != before.st_dev
            or opened.st_ino != before.st_ino
            or opened.st_size != before.st_size
            or not stat.S_ISREG(opened.st_mode)
            or opened.st_nlink != 1
        ):
            raise VerificationError("artifact identity changed before hashing")
        digest = hashlib.sha256()
        with os.fdopen(descriptor, "rb", closefd=False) as stream:
            while chunk := stream.read(1024 * 1024):
                digest.update(chunk)
        after_open = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    after = path.lstat()
    expected = (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns)
    if (
        (after_open.st_dev, after_open.st_ino, after_open.st_size, after_open.st_mtime_ns)
        != expected
        or (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns) != expected
        or after.st_nlink != 1
    ):
        raise VerificationError("artifact identity changed while hashing")
    return FileIdentity(
        device=before.st_dev,
        inode=before.st_ino,
        size=before.st_size,
        mtime_ns=before.st_mtime_ns,
        mode=stat.S_IMODE(before.st_mode),
        sha256=digest.hexdigest(),
    )


def run_checked(argv: Sequence[str], *, timeout: int) -> bytes:
    # File-backed capture prevents a hostile or malfunctioning child from
    # forcing unbounded output into this trusted process's memory.
    with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
        try:
            result = subprocess.run(
                list(argv),
                stdin=subprocess.DEVNULL,
                stdout=stdout,
                stderr=stderr,
                check=False,
                timeout=timeout,
            )
        except (OSError, subprocess.TimeoutExpired) as error:
            raise VerificationError(f"operator command failed to execute: {argv[0]}") from error
        stdout_size = stdout.tell()
        stderr_size = stderr.tell()
        if stdout_size > MAX_ATTESTATION_OUTPUT_BYTES or stderr_size > MAX_ATTESTATION_OUTPUT_BYTES:
            raise VerificationError(f"operator command output exceeded its bound: {argv[0]}")
        stdout.seek(0)
        stderr.seek(0)
        output = stdout.read()
        error_output = stderr.read()
    if result.returncode != 0:
        detail = error_output[:1024].decode("utf-8", errors="replace").strip()
        raise VerificationError(f"operator command rejected the artifact: {argv[0]}: {detail}")
    return output


def run_interactive(argv: Sequence[str]) -> None:
    try:
        result = subprocess.run(list(argv), check=False)
    except OSError as error:
        raise VerificationError(f"interactive operator command failed: {argv[0]}") from error
    if result.returncode != 0:
        raise VerificationError(f"interactive root handoff failed: {argv[0]}")


def attested_subjects(document: Any) -> set[tuple[str, str]]:
    subjects_found: set[tuple[str, str]] = set()
    if not isinstance(document, list):
        return subjects_found
    for entry in document:
        if not isinstance(entry, dict):
            continue
        verification = entry.get("verificationResult")
        if not isinstance(verification, dict):
            continue
        statement = verification.get("statement")
        if not isinstance(statement, dict) or statement.get("predicateType") != PREDICATE:
            continue
        subjects = statement.get("subject")
        if not isinstance(subjects, list):
            continue
        for subject in subjects:
            if not isinstance(subject, dict) or not isinstance(subject.get("digest"), dict):
                continue
            name = subject.get("name")
            value = subject["digest"].get("sha256")
            if (
                isinstance(name, str)
                and isinstance(value, str)
                and re.fullmatch(r"[0-9a-f]{64}", value)
            ):
                subjects_found.add((name, value))
    return subjects_found


def verify_attestation(
    artifact: Path,
    *,
    source_ref: str,
    source_digest: str,
    gh_program: str = "gh",
) -> tuple[FileIdentity, bytes, str]:
    if not SAFE_TAG_REF.fullmatch(source_ref):
        raise VerificationError("source ref must be an exact version tag ref")
    if not COMMIT_DIGEST.fullmatch(source_digest):
        raise VerificationError("source digest must be a lowercase 40-hex commit")
    artifact_match = SAFE_ARTIFACT.fullmatch(artifact.name)
    if artifact_match is None:
        raise VerificationError("artifact name is not a CPU-edge self-evolution bootstrap")
    if source_ref != f"refs/tags/v{artifact_match.group('version')}":
        raise VerificationError("artifact version does not match the exact source tag")

    before = stable_sha256(artifact)
    output = run_checked(
        (
            gh_program,
            "attestation",
            "verify",
            str(artifact),
            "--repo",
            DEFAULT_REPOSITORY,
            "--signer-workflow",
            SIGNER_WORKFLOW,
            "--signer-digest",
            source_digest,
            "--source-ref",
            source_ref,
            "--source-digest",
            source_digest,
            "--hostname",
            "github.com",
            "--cert-oidc-issuer",
            OIDC_ISSUER,
            "--cert-identity",
            f"{CERT_IDENTITY_PREFIX}{source_ref}",
            "--deny-self-hosted-runners",
            "--predicate-type",
            PREDICATE,
            "--format",
            "json",
        ),
        timeout=300,
    )
    try:
        parsed = json.loads(output)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise VerificationError("GitHub returned malformed attestation JSON") from error
    if (artifact.name, before.sha256) not in attested_subjects(parsed):
        raise VerificationError("verified attestation did not name the exact artifact and digest")
    after = stable_sha256(artifact)
    if after != before:
        raise VerificationError("artifact changed during attestation verification")
    return before, output, SIGNER_WORKFLOW


def transfer_verified(
    artifact: Path,
    identity: FileIdentity,
    *,
    appliance: str,
    ssh_program: str = "ssh",
    scp_program: str = "scp",
) -> str:
    destinations = {
        "avado": ("avado", "/home/avado/astrid-bootstrap"),
        "icp": ("icp", "/home/nativeplanet/astrid-bootstrap"),
    }
    try:
        host, base = destinations[appliance]
    except KeyError as error:
        raise VerificationError("appliance must be avado or icp") from error
    remote_directory = f"{base}/{identity.sha256}"
    remote_path = f"{remote_directory}/{artifact.name}"
    run_checked((ssh_program, host, "--", "/usr/bin/install", "-d", "-m", "0700", base), timeout=60)
    run_checked((ssh_program, host, "--", "/usr/bin/mkdir", remote_directory), timeout=60)
    run_checked((ssh_program, host, "--", "/usr/bin/chmod", "0700", remote_directory), timeout=60)
    run_checked((scp_program, "-p", "--", str(artifact), f"{host}:{remote_path}"), timeout=14_400)
    output = run_checked(
        (ssh_program, host, "--", "/usr/bin/sha256sum", "--", remote_path), timeout=300
    )
    expected_output = f"{identity.sha256}  {remote_path}\n".encode("ascii")
    if output != expected_output:
        raise VerificationError("remote artifact digest or path does not match verified local bytes")
    if stable_sha256(artifact) != identity:
        raise VerificationError("local artifact changed during transfer")
    return remote_path


def install_verified_root_handoff(
    *,
    appliance: str,
    remote_path: str,
    identity: FileIdentity,
    source_ref: str,
    source_digest: str,
    ssh_program: str = "ssh",
) -> None:
    destinations = {
        "avado": ("avado", "/home/avado/astrid-bootstrap/"),
        "icp": ("icp", "/home/nativeplanet/astrid-bootstrap/"),
    }
    try:
        host, prefix = destinations[appliance]
    except KeyError as error:
        raise VerificationError("root handoff requires avado or icp") from error
    if not remote_path.startswith(prefix) or not remote_path.endswith(
        "-x86_64-unknown-linux-gnu.tar.gz"
    ):
        raise VerificationError("remote archive path is outside the fixed transfer root")
    artifact_name = remote_path.rsplit("/", 1)[-1]
    match = SAFE_ARTIFACT.fullmatch(artifact_name)
    if match is None or source_ref != f"refs/tags/v{match.group('version')}":
        raise VerificationError("remote archive identity does not match the source tag")
    encoded = base64.b64encode(ROOT_HANDOFF_PROGRAM.encode("utf-8")).decode("ascii")
    launcher = (
        "import base64,sys;"
        "exec(compile(base64.b64decode(sys.argv[1]),'<trusted-edge-root-handoff>','exec'))"
    )
    remote_arguments = [
        "/usr/bin/sudo",
        "/usr/bin/python3",
        "-I",
        "-E",
        "-s",
        "-c",
        launcher,
        encoded,
        remote_path,
        identity.sha256,
        str(identity.size),
        artifact_name.removesuffix(".tar.gz"),
        appliance,
        source_ref.removeprefix("refs/tags/"),
        source_digest,
    ]
    # One shell-quoted remote command is necessary because OpenSSH otherwise
    # concatenates argv without preserving argument boundaries. Every dynamic
    # field has already passed an exact grammar.
    run_interactive((ssh_program, "-t", host, shlex.join(remote_arguments)))


def write_receipt(path: Path, receipt: dict[str, Any]) -> None:
    payload = json.dumps(receipt, sort_keys=True, indent=2, allow_nan=False).encode("utf-8") + b"\n"
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags, 0o600)
    except OSError as error:
        raise VerificationError("refusing to replace an existing receipt") from error
    try:
        written = 0
        while written < len(payload):
            written += os.write(descriptor, payload[written:])
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def write_attestation(path: Path, payload: bytes) -> None:
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags, 0o600)
    except OSError as error:
        raise VerificationError("refusing to replace existing attestation evidence") from error
    try:
        written = 0
        while written < len(payload):
            written += os.write(descriptor, payload[written:])
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def prepare_evidence_paths(receipt: Path) -> tuple[Path, Path]:
    if not receipt.is_absolute():
        raise VerificationError("receipt path must be absolute")
    parent = receipt.parent
    parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    try:
        if parent.resolve(strict=True) != parent or parent.is_symlink():
            raise VerificationError("receipt parent has linked or non-canonical ancestry")
        metadata = parent.lstat()
    except OSError as error:
        raise VerificationError("receipt parent is unavailable") from error
    if (
        not stat.S_ISDIR(metadata.st_mode)
        or metadata.st_uid != os.getuid()
        or stat.S_IMODE(metadata.st_mode) & 0o077
    ):
        raise VerificationError("receipt parent must be an owner-only operator directory")
    attestation = Path(f"{receipt}.attestation.json")
    for path in (receipt, attestation):
        if path.exists() or path.is_symlink():
            raise VerificationError("refusing to replace existing verification evidence")
    return receipt, attestation


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--artifact", type=Path, required=True)
    result.add_argument("--source-ref", required=True, help="exact release ref, e.g. refs/tags/v0.8.0")
    result.add_argument("--source-digest", required=True, help="exact release source commit")
    result.add_argument("--appliance", choices=("avado", "icp"))
    result.add_argument(
        "--root-install",
        action="store_true",
        help="after transfer, open interactive sudo and install from a root-owned digest handoff",
    )
    result.add_argument("--receipt", type=Path, required=True)
    return result


def main(argv: Sequence[str] | None = None) -> int:
    args = parser().parse_args(argv)
    try:
        if args.root_install and args.appliance is None:
            raise VerificationError("--root-install requires --appliance")
        receipt_path, attestation_path = prepare_evidence_paths(args.receipt)
        identity, attestation, signer_workflow = verify_attestation(
            args.artifact,
            source_ref=args.source_ref,
            source_digest=args.source_digest,
        )
        remote_path = None
        if args.appliance is not None:
            remote_path = transfer_verified(
                args.artifact, identity, appliance=args.appliance
            )
        if args.root_install:
            assert args.appliance is not None and remote_path is not None
            install_verified_root_handoff(
                appliance=args.appliance,
                remote_path=remote_path,
                identity=identity,
                source_ref=args.source_ref,
                source_digest=args.source_digest,
            )
        receipt = {
            "schema": SCHEMA,
            "verified_at": dt.datetime.now(dt.timezone.utc).isoformat().replace("+00:00", "Z"),
            "repository": DEFAULT_REPOSITORY,
            "signer_workflow": signer_workflow,
            "source_ref": args.source_ref,
            "source_digest": args.source_digest,
            "predicate_type": PREDICATE,
            "deny_self_hosted_runners": True,
            "artifact_name": args.artifact.name,
            "artifact_bytes": identity.size,
            "artifact_sha256": identity.sha256,
            "attestation_verification_json_sha256": hashlib.sha256(attestation).hexdigest(),
            "attestation_verification_json_path": str(attestation_path),
            "appliance": args.appliance,
            "remote_path": remote_path,
            "remote_sha256": identity.sha256 if remote_path is not None else None,
            "root_install_requested": args.root_install,
            "root_install_completed": args.root_install,
            "authority": "trusted_operator_verification_not_astrid_authorship",
        }
        write_attestation(attestation_path, attestation)
        try:
            write_receipt(receipt_path, receipt)
        except Exception:
            attestation_path.unlink(missing_ok=True)
            raise
        directory = os.open(receipt_path.parent, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
        try:
            os.fsync(directory)
        finally:
            os.close(directory)
    except VerificationError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2
    print(json.dumps(receipt, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
