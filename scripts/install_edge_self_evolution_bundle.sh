#!/usr/bin/env bash
# Verify and install one complete CPU-edge self-evolution release bundle.
set -euo pipefail
IFS=$'\n\t'
umask 077
PATH=/usr/sbin:/usr/bin:/sbin:/bin
export PATH
unset BASH_ENV CDPATH ENV GLOBIGNORE PYTHONHOME PYTHONPATH PYTHONSTARTUP

readonly PYTHON=/usr/bin/python3
readonly FINDMNT=/usr/bin/findmnt
readonly THERMAL_CLASS_ROOT=/sys/class/thermal
readonly ICP_MOUNT=/media/data
readonly EXPECTED_KERNEL=Linux
readonly EXPECTED_MACHINE=x86_64
readonly RETAINED_ICP_BACKUP=/media/data/astrid/backups/emmc-20260729T130835Z
readonly TARGET=x86_64-unknown-linux-gnu
readonly MAX_CPU_ARCHIVE_BYTES=$((60 * 1024 * 1024))
readonly OPERATOR_HANDOFF_BASE=/var/lib/astrid-edge-bootstrap
readonly IMMUTABLE_ROOT_UID=0
readonly IMMUTABLE_ROOT_GID=0

appliance=
dry_run=false
operator_handoff=

usage() {
    cat <<'EOF'
usage: install [--dry-run] --appliance avado|icp

Verifies the complete offline release, derives the exact live appliance
profile, and invokes the immutable root installer. Candidate promotion starts
paused. This command never resumes promotion or removes the retained ICP
backup. Internal hashes detect corruption but do not authenticate the release;
the GitHub OIDC/Sigstore attestation must already have been verified with
scripts/verify_edge_self_evolution_release.py on a trusted operator host.
EOF
}

die() { printf 'error: %s\n' "$*" >&2; exit 2; }

while (($#)); do
    case "$1" in
        --appliance)
            (($# >= 2)) && [[ -n $2 ]] || die "missing value for --appliance"
            [[ -z $appliance ]] || die "--appliance may be supplied only once"
            appliance=$2
            shift 2
            ;;
        --dry-run)
            $dry_run && die "--dry-run may be supplied only once"
            dry_run=true
            shift
            ;;
        # Internal trust handoff written only by the operator-side attestation
        # verifier after an interactive sudo copy/extraction transaction.
        --operator-handoff)
            (($# >= 2)) && [[ -n $2 ]] || die "missing value for --operator-handoff"
            [[ -z $operator_handoff ]] || die "--operator-handoff may be supplied only once"
            operator_handoff=$2
            shift 2
            ;;
        -h|--help) usage; exit 0 ;;
        *) die "unsupported argument: $1" ;;
    esac
done

[[ $appliance == avado || $appliance == icp ]] || die "--appliance must be avado or icp"
if $dry_run; then
    [[ -z $operator_handoff ]] || die "--operator-handoff is reserved for a non-dry trusted-root installation"
else
    [[ -n $operator_handoff ]] || die "non-dry installation requires the trusted operator handoff"
fi
[[ $(uname -s) == "$EXPECTED_KERNEL" ]] || die "CPU-edge installation is Linux-only"
[[ $(uname -m) == "$EXPECTED_MACHINE" ]] || die "AVADO and ICP require an x86-64 release"
[[ -x $PYTHON ]] || die "required interpreter is absent: $PYTHON"
if ! $dry_run && [[ $(id -u) != "$IMMUTABLE_ROOT_UID" ]]; then
    die "installation requires root; use --dry-run for read-only validation"
fi

script_source=${BASH_SOURCE[0]}
[[ $script_source == /* ]] || script_source=$PWD/$script_source
[[ ! -L $script_source && -f $script_source ]] || die "installer must be a regular non-symlink"
script_dir=$(CDPATH= cd -- "${script_source%/*}" && pwd -P) || die "cannot resolve bundle root"
script_path=$script_dir/${script_source##*/}
[[ $script_path == "$script_dir/install" ]] || die "installer must run as the packaged top-level ./install"

stage_parent=/tmp
[[ $(id -u) == 0 ]] && stage_parent=/var/tmp
stage=$(mktemp -d "$stage_parent/astrid-edge-self-evolution-bundle.XXXXXX") \
    || die "cannot create private verification directory"
case "$stage" in
    "$stage_parent"/astrid-edge-self-evolution-bundle.*) ;;
    *) die "temporary directory escaped its fixed parent" ;;
esac
[[ -d $stage && ! -L $stage ]] || die "temporary directory is unsafe"
chmod 0700 "$stage"

cleanup() {
    local status=$?
    trap - EXIT HUP INT TERM
    case "${stage:-}" in
        "$stage_parent"/astrid-edge-self-evolution-bundle.*)
            [[ -d $stage && ! -L $stage ]] && rm -rf -- "$stage"
            ;;
    esac
    exit "$status"
}
trap cleanup EXIT HUP INT TERM

# A non-dry installation may execute only from the root-owned result of the
# trusted operator's GitHub OIDC/Sigstore verification handoff. Internal bundle
# hashes cannot create this authority: the wrapper never writes the receipt,
# never copies the retained archive, and never makes its own extracted root.
handoff_identity=
if ! $dry_run; then
    handoff_identity=$(
        "$PYTHON" -I -E -s - \
            "$OPERATOR_HANDOFF_BASE" "$operator_handoff" "$script_dir" "$script_path" \
            "$appliance" "$IMMUTABLE_ROOT_UID" "$IMMUTABLE_ROOT_GID" <<'PY'
import hashlib
import json
import os
import re
import stat
import sys
from pathlib import Path

base = Path(sys.argv[1])
receipt_path = Path(sys.argv[2])
extracted_root = Path(sys.argv[3])
script_path = Path(sys.argv[4])
appliance = sys.argv[5]
required_uid = int(sys.argv[6])
required_gid = int(sys.argv[7])
hex64 = re.compile(r"[0-9a-f]{64}\Z")
hex40 = re.compile(r"[0-9a-f]{40}\Z")
safe_tag = re.compile(r"v[A-Za-z0-9][A-Za-z0-9._-]{0,127}\Z")
schema_keys = {
    "schema",
    "authority",
    "repository",
    "signer_workflow",
    "source_tag",
    "source_commit",
    "outer_archive_sha256",
    "outer_archive_bytes",
    "outer_archive_path",
    "extracted_root",
    "extracted_root_device",
    "extracted_root_inode",
    "appliance",
}

def fail(message: str) -> None:
    raise SystemExit(f"error: {message}")

def identity(metadata: os.stat_result) -> tuple[int, int, int, int]:
    return (metadata.st_dev, metadata.st_ino, metadata.st_size, metadata.st_mtime_ns)

def require_root_owned(path: Path, *, directory: bool, mode=None) -> os.stat_result:
    metadata = path.lstat()
    expected_type = stat.S_ISDIR(metadata.st_mode) if directory else stat.S_ISREG(metadata.st_mode)
    if (
        not expected_type
        or stat.S_ISLNK(metadata.st_mode)
        or metadata.st_uid != required_uid
        or metadata.st_gid != required_gid
        or metadata.st_mode & 0o022
        or (mode is not None and stat.S_IMODE(metadata.st_mode) != mode)
        or (not directory and metadata.st_nlink != 1)
    ):
        fail(f"operator handoff member is not exact immutable-root material: {path}")
    return metadata

def stable_read(path: Path, maximum: int) -> tuple[bytes, os.stat_result]:
    before = require_root_owned(path, directory=False, mode=0o400)
    if before.st_size <= 0 or before.st_size > maximum:
        fail(f"operator handoff member is empty or oversized: {path}")
    descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
    try:
        opened = os.fstat(descriptor)
        chunks = []
        remaining = maximum + 1
        while remaining:
            block = os.read(descriptor, min(1024 * 1024, remaining))
            if not block:
                break
            chunks.append(block)
            remaining -= len(block)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    data = b"".join(chunks)
    if identity(before) != identity(opened) or identity(opened) != identity(after):
        fail(f"operator handoff member changed while read: {path}")
    if len(data) != before.st_size or len(data) > maximum:
        fail(f"operator handoff member changed size while read: {path}")
    return data, before

if not receipt_path.is_absolute() or receipt_path.name != "operator-handoff.json":
    fail("operator handoff receipt path is not exact")
try:
    relative_receipt = receipt_path.relative_to(base)
except ValueError:
    fail("operator handoff receipt escapes its fixed root")
if len(relative_receipt.parts) != 2:
    fail("operator handoff receipt is not under one digest root")
digest_directory = relative_receipt.parts[0]
if hex64.fullmatch(digest_directory) is None:
    fail("operator handoff digest directory is malformed")
handoff_root = base / digest_directory
if extracted_root.parent != handoff_root or script_path != extracted_root / "install":
    fail("running installer is outside the exact operator handoff root")

# Every ancestor and every extracted member is immutable to non-root users.
# Checking only the top directory would leave user-owned mode-0600 payloads
# replaceable by their owner even below a protected directory.
cursor = Path("/")
for part in extracted_root.parts[1:]:
    cursor /= part
    require_root_owned(cursor, directory=True)
for path in extracted_root.rglob("*"):
    metadata = path.lstat()
    if stat.S_ISDIR(metadata.st_mode):
        require_root_owned(path, directory=True)
    elif stat.S_ISREG(metadata.st_mode):
        require_root_owned(path, directory=False)
    else:
        fail(f"operator handoff extraction contains a link or special member: {path}")

actual_entries = {path.name for path in handoff_root.iterdir()}
expected_entries = {"release.tar.gz", "operator-handoff.json", extracted_root.name}
if actual_entries != expected_entries:
    fail("operator handoff digest root inventory is not exact")

receipt_bytes, _ = stable_read(receipt_path, 64 * 1024)
try:
    receipt = json.loads(receipt_bytes)
except (UnicodeError, json.JSONDecodeError) as error:
    fail(f"operator handoff receipt is malformed: {error}")
if not isinstance(receipt, dict) or set(receipt) != schema_keys:
    fail("operator handoff receipt shape is not exact")
archive_digest = receipt.get("outer_archive_sha256")
archive_path = handoff_root / "release.tar.gz"
root_metadata = extracted_root.lstat()
if (
    receipt.get("schema") != "astrid.edge.self_evolution_operator_handoff.v1"
    or receipt.get("authority") != "trusted_operator_verified_github_oidc_sigstore_release_handoff"
    or receipt.get("repository") != "unicity-astrid/astrid"
    or receipt.get("signer_workflow") != ".github/workflows/release.yml"
    or not isinstance(receipt.get("source_tag"), str)
    or safe_tag.fullmatch(receipt["source_tag"]) is None
    or not isinstance(receipt.get("source_commit"), str)
    or hex40.fullmatch(receipt["source_commit"]) is None
    or not isinstance(archive_digest, str)
    or hex64.fullmatch(archive_digest) is None
    or archive_digest != digest_directory
    or not isinstance(receipt.get("outer_archive_bytes"), int)
    or isinstance(receipt.get("outer_archive_bytes"), bool)
    or receipt["outer_archive_bytes"] <= 0
    or receipt["outer_archive_bytes"] > 8 * 1024 * 1024 * 1024
    or receipt.get("outer_archive_path") != str(archive_path)
    or receipt.get("extracted_root") != str(extracted_root)
    or receipt.get("extracted_root_device") != root_metadata.st_dev
    or receipt.get("extracted_root_inode") != root_metadata.st_ino
    or receipt.get("appliance") != appliance
):
    fail("operator handoff receipt identity or attestation binding is not exact")

archive_metadata = require_root_owned(archive_path, directory=False, mode=0o400)
if archive_metadata.st_size != receipt["outer_archive_bytes"]:
    fail("retained outer archive size does not match the operator handoff")
descriptor = os.open(archive_path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
digest = hashlib.sha256()
try:
    opened = os.fstat(descriptor)
    while True:
        block = os.read(descriptor, 1024 * 1024)
        if not block:
            break
        digest.update(block)
    after = os.fstat(descriptor)
finally:
    os.close(descriptor)
if identity(archive_metadata) != identity(opened) or identity(opened) != identity(after):
    fail("retained outer archive changed while hashed")
if digest.hexdigest() != archive_digest:
    fail("retained outer archive digest does not match the trusted handoff")

print(archive_digest)
print(receipt["source_tag"])
print(receipt["source_commit"])
PY
    )
    [[ $handoff_identity == *$'\n'* ]] || die "operator handoff verifier returned malformed identity"
fi

# Verify every outer member, the exact top-level SHA inventory, the manifest's
# fixed authority/profile contract, and stable-copy payloads into a private
# directory. These hashes detect corruption only. They do not replace the
# external GitHub OIDC/Sigstore release-attestation verification performed on
# a trusted operator host before this root entrypoint is invoked.
bundle_identity=$(
    "$PYTHON" -I -E -s - "$script_dir" "$script_path" "$stage" <<'PY'
import hashlib
import json
import os
import re
import stat
import sys
from pathlib import Path, PurePosixPath

root = Path(sys.argv[1])
script = Path(sys.argv[2])
stage = Path(sys.argv[3])
hex64 = re.compile(r"[0-9a-f]{64}\Z")
safe_id = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]{0,127}\Z")
payload_policy = {
    "cpu_edge_archive": ("payload/cpu-edge.tar.gz", 60 * 1024 * 1024, "0600"),
    "initial_generation": ("payload/initial-generation.tar.gz", 512 * 1024 * 1024, "0600"),
    "portable_source": ("payload/portable-source.tar.gz", 2 * 1024 * 1024 * 1024, "0600"),
    "pinned_toolchain": ("payload/pinned-toolchain.tar.gz", 4 * 1024 * 1024 * 1024, "0600"),
    "portable_source_key": ("payload/portable-source.key", 32, "0600"),
    "immutable_supervisor": ("payload/edge-self-change-supervisor.pyz", 64 * 1024 * 1024, "0500"),
}
expected_profiles = {
    "avado": {
        "appliance_id": "avado-edge",
        "runtime_user": "avado",
        "runtime_home": "/home/avado",
        "runtime_workspace": "/home/avado/.astrid/home/default/edge",
        "model": "qwen3.5:4b",
        "context_tokens": 4096,
        "output_tokens": 192,
        "source_authoring_output_tokens": 384,
        "header_timeout_ms": 300000,
        "total_timeout_ms": 600000,
        "audio": "physical_numeric_only",
    },
    "icp": {
        "appliance_id": "icp-edge",
        "runtime_user": "nativeplanet",
        "runtime_home": "/home/nativeplanet",
        "runtime_workspace": "/media/data/astrid/state/home/default/edge",
        "model": "qwen3:1.7b",
        "context_tokens": 2048,
        "output_tokens": 112,
        "source_authoring_output_tokens": 160,
        "header_timeout_ms": 420000,
        "total_timeout_ms": 660000,
        "audio": "explicitly_unavailable",
        "required_mount": "/media/data",
        "retained_backup": "/media/data/astrid/backups/emmc-20260729T130835Z",
    },
}

def fail(message: str) -> None:
    raise SystemExit(f"error: {message}")

def identity(metadata: os.stat_result) -> tuple[int, int, int, int]:
    return (metadata.st_dev, metadata.st_ino, metadata.st_size, metadata.st_mtime_ns)

def read_regular(path: Path, maximum: int) -> tuple[bytes, os.stat_result]:
    before = path.lstat()
    if not stat.S_ISREG(before.st_mode) or stat.S_ISLNK(before.st_mode) or before.st_nlink != 1:
        fail(f"bundle member is linked or non-regular: {path.name}")
    if before.st_size <= 0 or before.st_size > maximum:
        fail(f"bundle member is empty or oversized: {path.name}")
    descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
    try:
        opened = os.fstat(descriptor)
        chunks = []
        remaining = maximum + 1
        while remaining:
            chunk = os.read(descriptor, min(1024 * 1024, remaining))
            if not chunk:
                break
            chunks.append(chunk)
            remaining -= len(chunk)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    data = b"".join(chunks)
    if identity(before) != identity(opened) or identity(opened) != identity(after):
        fail(f"bundle member changed while read: {path.name}")
    if len(data) != before.st_size or len(data) > maximum:
        fail(f"bundle member changed size or exceeded its bound: {path.name}")
    return data, before

def copy_and_hash_regular(source: Path, destination: Path, maximum: int, mode: int) -> tuple[int, str, os.stat_result]:
    before = source.lstat()
    if not stat.S_ISREG(before.st_mode) or stat.S_ISLNK(before.st_mode) or before.st_nlink != 1:
        fail(f"bundle payload is linked or non-regular: {source.name}")
    if before.st_size <= 0 or before.st_size > maximum:
        fail(f"bundle payload is empty or oversized: {source.name}")
    source_descriptor = os.open(source, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
    destination_descriptor = -1
    digest = hashlib.sha256()
    written = 0
    try:
        opened = os.fstat(source_descriptor)
        destination_descriptor = os.open(
            destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL, mode
        )
        while True:
            chunk = os.read(source_descriptor, 1024 * 1024)
            if not chunk:
                break
            written += len(chunk)
            if written > maximum:
                fail(f"bundle payload exceeded its bound: {source.name}")
            digest.update(chunk)
            view = memoryview(chunk)
            while view:
                count = os.write(destination_descriptor, view)
                view = view[count:]
        os.fsync(destination_descriptor)
        after = os.fstat(source_descriptor)
    finally:
        if destination_descriptor >= 0:
            os.close(destination_descriptor)
        os.close(source_descriptor)
    if identity(before) != identity(opened) or identity(opened) != identity(after):
        fail(f"bundle payload changed while copied: {source.name}")
    if written != before.st_size:
        fail(f"bundle payload changed size while copied: {source.name}")
    return written, digest.hexdigest(), before

root_meta = root.lstat()
if not stat.S_ISDIR(root_meta.st_mode) or stat.S_ISLNK(root_meta.st_mode):
    fail("bundle root is not a real directory")
if not os.path.samefile(script, root / "install"):
    fail("running installer is not the inventoried bundle member")
expected_files = {
    "MANIFEST.json", "README.txt", "install", "SHA256SUMS",
    *(policy[0] for policy in payload_policy.values()),
}
actual_files = set()
top_modes = {
    "MANIFEST.json": 0o600,
    "README.txt": 0o600,
    "install": 0o500,
    "SHA256SUMS": 0o600,
}
for path in root.rglob("*"):
    relative = path.relative_to(root).as_posix()
    metadata = path.lstat()
    if stat.S_ISDIR(metadata.st_mode) and not stat.S_ISLNK(metadata.st_mode):
        if relative != "payload":
            fail(f"unexpected bundle directory: {relative}")
        continue
    if not stat.S_ISREG(metadata.st_mode) or stat.S_ISLNK(metadata.st_mode) or metadata.st_nlink != 1:
        fail(f"bundle contains a link or special member: {relative}")
    if metadata.st_mode & 0o022:
        fail(f"bundle member is group/world writable: {relative}")
    if relative in top_modes and stat.S_IMODE(metadata.st_mode) != top_modes[relative]:
        fail(f"top-level bundle member mode is not exact: {relative}")
    actual_files.add(relative)
if actual_files != expected_files:
    fail(f"bundle inventory is not exact: missing={sorted(expected_files-actual_files)} extra={sorted(actual_files-expected_files)}")

manifest_bytes, manifest_meta = read_regular(root / "MANIFEST.json", 1024 * 1024)
try:
    manifest = json.loads(manifest_bytes)
except (UnicodeError, json.JSONDecodeError) as error:
    fail(f"bundle manifest is malformed: {error}")
if not isinstance(manifest, dict):
    fail("bundle manifest is not an object")
if (
    manifest.get("schema") != "astrid.edge.self_evolution_bootstrap.v1"
    or manifest.get("authority") != "operator_release_bootstrap_not_model_authorship_or_appliance_authority"
    or manifest.get("portable_trust") != "integrity_only_rebound_to_fresh_per_appliance_key_before_authorization"
    or manifest.get("ordinary_autonomy") != "preserved"
    or manifest.get("initial_mode") != "paused_bootstrap_acceptance_pending"
    or manifest.get("cross_appliance_or_mac_transfer") != "forbidden"
    or manifest.get("target") != "x86_64-unknown-linux-gnu"
    or not isinstance(manifest.get("version"), str)
    or safe_id.fullmatch(manifest["version"]) is None
    or manifest.get("profiles") != expected_profiles
):
    fail("bundle manifest identity, authority, target, or profiles are not exact")

records = manifest.get("payloads")
if not isinstance(records, list) or len(records) != len(payload_policy):
    fail("bundle payload manifest is not exact")
record_by_role = {}
for record in records:
    if not isinstance(record, dict) or set(record) != {"role", "path", "bytes", "sha256", "mode"}:
        fail("bundle payload record shape is not exact")
    role = record.get("role")
    if role not in payload_policy or role in record_by_role:
        fail("bundle payload role is unknown or duplicated")
    path, maximum, mode = payload_policy[role]
    if (
        record.get("path") != path
        or record.get("mode") != mode
        or not isinstance(record.get("bytes"), int)
        or isinstance(record.get("bytes"), bool)
        or record["bytes"] <= 0
        or record["bytes"] > maximum
        or not isinstance(record.get("sha256"), str)
        or hex64.fullmatch(record["sha256"]) is None
    ):
        fail(f"bundle payload record violates policy: {role}")
    record_by_role[role] = record
if set(record_by_role) != set(payload_policy):
    fail("bundle payload roles are incomplete")

sums_bytes, _ = read_regular(root / "SHA256SUMS", 1024 * 1024)
try:
    lines = sums_bytes.decode("ascii").splitlines()
except UnicodeError as error:
    fail(f"SHA256SUMS is not ASCII: {error}")
sums = {}
for line in lines:
    match = re.fullmatch(r"([0-9a-f]{64})  ([A-Za-z0-9][A-Za-z0-9._/-]*)", line)
    if match is None:
        fail("SHA256SUMS contains malformed input")
    digest, name = match.groups()
    pure = PurePosixPath(name)
    if pure.is_absolute() or pure.as_posix() != name or any(part in {"", ".", ".."} or part.startswith(".") for part in pure.parts):
        fail("SHA256SUMS contains an unsafe path")
    if name in sums:
        fail("SHA256SUMS contains a duplicate path")
    sums[name] = digest
expected_sums = expected_files - {"SHA256SUMS"}
if set(sums) != expected_sums:
    fail("SHA256SUMS inventory is not exact")

payload_destination = stage / "payload"
payload_destination.mkdir(mode=0o700)
for name in sorted(expected_sums):
    if name.startswith("payload/"):
        role = next(role for role, policy in payload_policy.items() if policy[0] == name)
        record = record_by_role[role]
        expected_mode = int(record["mode"], 8)
        length, digest, metadata = copy_and_hash_regular(
            root / name, stage / name, payload_policy[role][1], expected_mode
        )
        if stat.S_IMODE(metadata.st_mode) != expected_mode:
            fail(f"payload mode mismatch: {role}")
        if length != record["bytes"] or digest != record["sha256"]:
            fail(f"manifest payload mismatch: {role}")
    else:
        data, _ = read_regular(root / name, 2 * 1024 * 1024)
        digest = hashlib.sha256(data).hexdigest()
    if digest != sums[name]:
        fail(f"SHA256SUMS mismatch: {name}")

if (stage / "payload/portable-source.key").stat().st_size != 32:
    fail("portable source key is not exactly 32 bytes")
if not (stage / "payload/edge-self-change-supervisor.pyz").read_bytes().startswith(b"#!"):
    fail("immutable supervisor is not an executable zipapp")
print(manifest["version"])
print(manifest["target"])
PY
)
[[ $bundle_identity == *$'\n'* ]] || die "bundle verifier returned malformed identity"
version=${bundle_identity%%$'\n'*}
target=${bundle_identity#*$'\n'}
[[ -n $version && -n $target && $target != *$'\n'* ]] || die "bundle verifier returned malformed identity"
[[ $target == "$TARGET" ]] || die "bundle target does not match the two x86 appliances"
if ! $dry_run; then
    handoff_archive_sha256=${handoff_identity%%$'\n'*}
    handoff_remainder=${handoff_identity#*$'\n'}
    [[ $handoff_remainder == *$'\n'* ]] || die "operator handoff verifier returned malformed identity"
    handoff_source_tag=${handoff_remainder%%$'\n'*}
    handoff_source_commit=${handoff_remainder#*$'\n'}
    [[ -n $handoff_archive_sha256 && -n $handoff_source_tag && -n $handoff_source_commit \
        && $handoff_source_commit != *$'\n'* ]] \
        || die "operator handoff verifier returned malformed identity"
    [[ $handoff_source_tag == "v$version" ]] \
        || die "operator handoff source tag does not match the release version"
    [[ $script_dir == "$OPERATOR_HANDOFF_BASE/$handoff_archive_sha256/astrid-edge-self-evolution-$version-$target" ]] \
        || die "extracted root name does not match the attested release identity"
fi

# The nested CPU-edge archive is small enough to unpack before any privileged
# mutation. Parse tar metadata, reject every link/special/alias, extract into
# the private root with O_NOFOLLOW, then verify its own exact SHA inventory.
cpu_identity=$(
    "$PYTHON" -I -E -s - "$stage/payload/cpu-edge.tar.gz" "$stage/cpu" "$version" "$target" "$MAX_CPU_ARCHIVE_BYTES" <<'PY'
import hashlib
import json
import os
import re
import stat
import sys
import tarfile
from pathlib import Path, PurePosixPath

archive_path = Path(sys.argv[1])
destination = Path(sys.argv[2])
version = sys.argv[3]
target = sys.argv[4]
maximum_archive = int(sys.argv[5])
expected_root_name = f"astrid-cpu-edge-{version}-{target}"
maximum_expanded = 512 * 1024 * 1024
maximum_member = 64 * 1024 * 1024
maximum_members = 20_000
hex64 = re.compile(r"[0-9a-f]{64}\Z")

def fail(message: str) -> None:
    raise SystemExit(f"error: {message}")

metadata = archive_path.lstat()
if not stat.S_ISREG(metadata.st_mode) or stat.S_ISLNK(metadata.st_mode) or metadata.st_nlink != 1:
    fail("CPU-edge archive is linked or non-regular")
if metadata.st_size <= 0 or metadata.st_size > maximum_archive:
    fail("CPU-edge archive exceeds the 60 MiB ceiling")
destination.mkdir(mode=0o700)
seen = set()
total = 0
members = []
try:
    with tarfile.open(archive_path, mode="r:gz") as archive:
        for member in archive:
            if len(seen) >= maximum_members:
                fail("CPU-edge archive has too many members")
            raw = member.name
            pure = PurePosixPath(raw)
            canonical = pure.as_posix()
            parts = pure.parts
            if (
                not raw
                or pure.is_absolute()
                or not parts
                or parts[0] != expected_root_name
                or any(part in {"", ".", ".."} or part.startswith(".") for part in parts)
                or (raw != canonical and not (member.isdir() and raw == canonical + "/"))
                or canonical in seen
                or not (member.isdir() or member.isfile())
                or getattr(member, "sparse", None)
                or member.mode & ~0o755
                or member.mode & 0o022
            ):
                fail("CPU-edge archive contains an unsafe path, type, mode, or alias")
            seen.add(canonical)
            if member.isfile():
                if member.size <= 0 or member.size > maximum_member:
                    fail("CPU-edge archive member is empty or oversized")
                total += member.size
                if total > maximum_expanded:
                    fail("CPU-edge archive exceeds its expanded-size ceiling")
            members.append((member, pure))

        for member, pure in members:
            output = destination.joinpath(*pure.parts)
            if member.isdir():
                output.mkdir(mode=member.mode & 0o755, parents=True, exist_ok=True)
                if output.is_symlink() or not output.is_dir():
                    fail("CPU-edge directory extraction collided")
                os.chmod(output, member.mode & 0o755)
                continue
            output.parent.mkdir(mode=0o755, parents=True, exist_ok=True)
            stream = archive.extractfile(member)
            if stream is None:
                fail("CPU-edge member body is absent")
            descriptor = os.open(
                output,
                os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0),
                member.mode & 0o755,
            )
            written = 0
            try:
                while True:
                    chunk = stream.read(1024 * 1024)
                    if not chunk:
                        break
                    written += len(chunk)
                    if written > member.size:
                        fail("CPU-edge member expanded beyond its declaration")
                    view = memoryview(chunk)
                    while view:
                        count = os.write(descriptor, view)
                        view = view[count:]
                os.fsync(descriptor)
            finally:
                os.close(descriptor)
            if written != member.size:
                fail("CPU-edge member size changed during extraction")
            os.chmod(output, member.mode & 0o755)
except (OSError, tarfile.TarError) as error:
    fail(f"CPU-edge archive cannot be extracted: {error}")

root = destination / expected_root_name
if not root.is_dir() or root.is_symlink():
    fail("CPU-edge archive lacks its exact root")
for path in root.rglob("*"):
    metadata = path.lstat()
    if stat.S_ISLNK(metadata.st_mode) or not (stat.S_ISREG(metadata.st_mode) or stat.S_ISDIR(metadata.st_mode)):
        fail("CPU-edge extraction contains a link or special member")
    if stat.S_ISREG(metadata.st_mode) and metadata.st_nlink != 1:
        fail("CPU-edge extraction contains a hard link")

manifest_path = root / "BUILD-MANIFEST.json"
sums_path = root / "SHA256SUMS"
try:
    manifest = json.loads(manifest_path.read_bytes())
except (OSError, UnicodeError, json.JSONDecodeError) as error:
    fail(f"CPU-edge build manifest is malformed: {error}")
if (
    not isinstance(manifest, dict)
    or manifest.get("schema") != "astrid_cpu_edge_build_manifest_v3"
    or manifest.get("bundle_format") != "cpu-edge.3"
    or manifest.get("version") != version
    or manifest.get("target") != target
    or manifest.get("expected_loaded_capsule_count") != 20
    or manifest.get("authority") != "release_build_manifest_not_appliance_state_or_astrid_memory"
):
    fail("CPU-edge build manifest identity is not exact")
try:
    sum_lines = sums_path.read_text(encoding="ascii").splitlines()
except (OSError, UnicodeError) as error:
    fail(f"CPU-edge SHA inventory is malformed: {error}")
sums = {}
for line in sum_lines:
    match = re.fullmatch(r"([0-9a-f]{64})  \./([^\r\n]+)", line)
    if match is None:
        fail("CPU-edge SHA inventory has malformed syntax")
    digest, name = match.groups()
    pure = PurePosixPath(name)
    if pure.is_absolute() or pure.as_posix() != name or any(part in {"", ".", ".."} for part in pure.parts):
        fail("CPU-edge SHA inventory contains an unsafe path")
    if name in sums:
        fail("CPU-edge SHA inventory contains a duplicate")
    sums[name] = digest
actual = {
    path.relative_to(root).as_posix()
    for path in root.rglob("*")
    if path.is_file() and path != sums_path
}
if set(sums) != actual:
    fail("CPU-edge SHA inventory does not cover the exact extraction")
for name, digest in sums.items():
    path = root / name
    if hashlib.sha256(path.read_bytes()).hexdigest() != digest:
        fail(f"CPU-edge SHA mismatch: {name}")

required = {
    "astrid-build", "astrid-edge-checkpoint", "astrid-edge-presentation-broker",
    "astrid-edge-provider-broker", "astrid-edge-rescue-helper",
    "astrid-edge-steward-helper", "astrid-edge-web-broker",
    "scripts/install_edge_self_evolution_root.sh", "packaging/systemd",
}
for name in required:
    path = root / name
    if not path.exists() or path.is_symlink():
        fail(f"CPU-edge release omits required bootstrap input: {name}")
source_commit = manifest.get("source_commit", "unknown")
if not isinstance(source_commit, str) or "\n" in source_commit or "\r" in source_commit:
    fail("CPU-edge source commit is malformed")
print(root)
print(source_commit)
PY
)
[[ $cpu_identity == *$'\n'* ]] || die "CPU-edge extractor returned malformed identity"
cpu_root=${cpu_identity%%$'\n'*}
cpu_source_commit=${cpu_identity#*$'\n'}
[[ -n $cpu_root && -n $cpu_source_commit && $cpu_source_commit != *$'\n'* ]] \
    || die "CPU-edge extractor returned malformed identity"
[[ $cpu_root == "$stage/cpu/astrid-cpu-edge-$version-$target" ]] \
    || die "CPU-edge extractor returned an unexpected root"
if ! $dry_run; then
    [[ $cpu_source_commit == "$handoff_source_commit" ]] \
        || die "operator handoff source commit does not match the CPU-edge build manifest"
fi

hash_file() {
    "$PYTHON" -I -E -s - "$1" <<'PY'
import hashlib, os, stat, sys
path = sys.argv[1]
before = os.lstat(path)
if not stat.S_ISREG(before.st_mode) or stat.S_ISLNK(before.st_mode) or before.st_nlink != 1:
    raise SystemExit("hash input is linked or non-regular")
descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
try:
    opened = os.fstat(descriptor)
    digest = hashlib.sha256()
    while True:
        block = os.read(descriptor, 1024 * 1024)
        if not block:
            break
        digest.update(block)
    after = os.fstat(descriptor)
finally:
    os.close(descriptor)
identity = lambda item: (item.st_dev, item.st_ino, item.st_size, item.st_mtime_ns)
if identity(before) != identity(opened) or identity(opened) != identity(after):
    raise SystemExit("hash input changed while read")
print(digest.hexdigest())
PY
}

discover_thermal() {
    "$PYTHON" -I -E -s - "$THERMAL_CLASS_ROOT" <<'PY'
import os, stat, sys
from pathlib import Path

root = Path(sys.argv[1])
matches = []
for type_path in sorted(root.glob("thermal_zone*/type")):
    try:
        if type_path.read_text(encoding="ascii", errors="strict").strip() != "x86_pkg_temp":
            continue
        temperature = (type_path.parent / "temp").resolve(strict=True)
        metadata = temperature.lstat()
        raw = temperature.read_text(encoding="ascii", errors="strict").strip()
        value = int(raw)
    except (OSError, UnicodeError, ValueError):
        continue
    if not stat.S_ISREG(metadata.st_mode) or temperature.is_symlink() or not (0 <= value <= 150_000):
        continue
    matches.append(str(temperature))
if not matches:
    raise SystemExit("error: no readable x86_pkg_temp sysfs sensor was found")
print(sorted(set(matches))[0])
PY
}

thermal_celsius=$(discover_thermal) || exit $?
[[ $thermal_celsius == /* && $thermal_celsius != *$'\n'* ]] \
    || die "thermal sensor resolver returned an unsafe path"

readonly -a system_stack=(
    ollama-cpu.service
    astrid-model-warmup.service
    astrid.service
    astrid-edge-runtime.service
    astrid-edge-hindsight.service
    astrid-edge-hindsight.timer
)
readonly -a install_units=(
    astrid-edge-self-change-supervisor.service
    astrid-edge-self-change-probation-health.service
    astrid-edge-self-change-probation-health.timer
    astrid-edge-steward.service
    astrid-edge-steward.timer
    astrid-edge-web-broker-core.socket
    astrid-edge-web-broker-core.service
    astrid-edge-web-broker-runtime.socket
    astrid-edge-web-broker-runtime.service
    astrid-edge-web-broker-steward.socket
    astrid-edge-web-broker-steward.service
    astrid-edge-provider-broker@.service
    astrid-edge-provider-runtime.socket
    astrid-edge-provider-steward.socket
    astrid-edge-provider-warmup.socket
    astrid-edge-presentation-broker.socket
    astrid-edge-presentation-broker@.service
    astrid-edge-generation-guard.service
    astrid-edge-core-liveness.service
    astrid-edge-core-liveness.path
    astrid-edge-self-change-inbox.path
    astrid-edge-runtime.service.d/60-self-evolution-root.conf
)
readonly -a enable_units=(
    astrid-edge-steward.timer
    astrid-edge-self-change-probation-health.timer
    astrid-edge-web-broker-core.socket
    astrid-edge-web-broker-runtime.socket
    astrid-edge-web-broker-steward.socket
    astrid-edge-provider-runtime.socket
    astrid-edge-provider-steward.socket
    astrid-edge-provider-warmup.socket
    astrid-edge-presentation-broker.socket
    astrid-edge-generation-guard.service
    astrid-edge-core-liveness.path
    astrid-edge-self-change-inbox.path
)

if [[ $appliance == avado ]]; then
    appliance_id=avado-edge
    runtime_user=avado
    runtime_home=/home/avado
    runtime_workspace=/home/avado/.astrid/home/default/edge
    model_ipc=/home/avado/.astrid/run
    model=qwen3.5:4b
    context_tokens=4096
    output_tokens=192
    source_authoring_output_tokens=384
    header_timeout_ms=300000
    total_timeout_ms=600000
    state_root=/var/lib/astrid-edge-supervisor
    release_root=/opt/astrid-edge/releases
    source_root=/var/lib/astrid-edge-source
    candidate_root=/var/lib/astrid-edge-candidates
    builder_root=/var/lib/astrid-edge-builder
    updater_root=/var/lib/astrid-edge-updater
    toolchain_root=/opt/astrid-edge-toolchain
    profile_unit_root=$cpu_root/packaging/systemd
else
    appliance_id=icp-edge
    runtime_user=nativeplanet
    runtime_home=/home/nativeplanet
    runtime_workspace=$ICP_MOUNT/astrid/state/home/default/edge
    model_ipc=$ICP_MOUNT/astrid/state/run
    model=qwen3:1.7b
    context_tokens=2048
    output_tokens=112
    source_authoring_output_tokens=160
    header_timeout_ms=420000
    total_timeout_ms=660000
    state_root=$ICP_MOUNT/astrid-edge-supervisor
    release_root=$ICP_MOUNT/astrid-edge-release-store/releases
    source_root=$ICP_MOUNT/astrid-edge-source
    candidate_root=$ICP_MOUNT/astrid-edge-candidates
    builder_root=$ICP_MOUNT/astrid-edge-builder
    updater_root=$ICP_MOUNT/astrid-edge-updater
    toolchain_root=$ICP_MOUNT/astrid-edge-toolchain
    profile_unit_root=$cpu_root/packaging/systemd/icp

    [[ -x $FINDMNT ]] || die "required mount inspector is absent: $FINDMNT"
    mount_identity=$("$FINDMNT" -rn -M "$ICP_MOUNT" -o TARGET,UUID) \
        || die "ICP SSD mount identity is unavailable"
    [[ -n $mount_identity && $mount_identity != *$'\n'* ]] \
        || die "ICP SSD mount identity is unavailable or ambiguous"
    IFS=' ' read -r mounted_target required_mount_uuid extra <<<"$mount_identity"
    [[ $mounted_target == "$ICP_MOUNT" && -z ${extra:-} ]] \
        || die "ICP SSD mount target is not exact"
    [[ $required_mount_uuid =~ ^[A-Fa-f0-9-]{4,64}$ ]] || die "ICP SSD UUID is malformed"
    [[ -d $RETAINED_ICP_BACKUP && ! -L $RETAINED_ICP_BACKUP ]] \
        || die "retained ICP eMMC backup is absent, linked, or not a directory"
    "$PYTHON" -I -E -s - "$ICP_MOUNT" "$RETAINED_ICP_BACKUP" <<'PY' \
        || die "retained ICP backup ancestry is unsafe"
import os, stat, sys
from pathlib import Path

mount = Path(sys.argv[1])
backup = Path(sys.argv[2])
if backup.resolve(strict=True) != backup:
    raise SystemExit(1)
cursor = mount
for part in backup.relative_to(mount).parts:
    cursor = cursor / part
    metadata = cursor.lstat()
    if not stat.S_ISDIR(metadata.st_mode) or stat.S_ISLNK(metadata.st_mode):
        raise SystemExit(1)
PY
    backup_target=$("$FINDMNT" -rn -T "$RETAINED_ICP_BACKUP" -o TARGET,UUID) \
        || die "cannot bind retained ICP backup to the SSD"
    [[ -n $backup_target && $backup_target != *$'\n'* ]] \
        || die "retained ICP backup mount identity is unavailable or ambiguous"
    IFS=' ' read -r backup_mount backup_uuid backup_extra <<<"$backup_target"
    [[ $backup_mount == "$ICP_MOUNT" && $backup_uuid == "$required_mount_uuid" && -z ${backup_extra:-} ]] \
        || die "retained ICP backup is not on the guarded SSD"
fi

unit_source_root=$cpu_root/packaging/systemd
root_installer=$cpu_root/scripts/install_edge_self_evolution_root.sh
[[ -x $root_installer && ! -L $root_installer ]] || die "root installer is absent or unsafe"

inbox_root=$state_root/inbox
vendor_root=$source_root/vendor
initial_generation_sha256=$(hash_file "$stage/payload/initial-generation.tar.gz")
initial_generation_id=bootstrap-${initial_generation_sha256:0:16}

declare -a args=(
    --start-system-services
    --appliance-id "$appliance_id"
    --target "$target"
    --runtime-user "$runtime_user"
    --runtime-home "$runtime_home"
    --runtime-workspace "$runtime_workspace"
    --model-ipc "$model_ipc"
    --steward-owned "continuity=$runtime_workspace/autonomous/thread_state.json"
    --steward-owned "self_profile=$runtime_workspace/self/profile.json"
    --steward-owned "verified_evidence=$runtime_workspace/autonomous/thread_state.jsonl"
    --steward-owned "machine_observation=$runtime_workspace/perception/latest.json"
    --steward-owned "spectral_host_state=$runtime_workspace/runtime/spectral_state.json"
    --model "$model"
    --ollama-origin http://127.0.0.1:11434
    --context-tokens "$context_tokens"
    --output-tokens "$output_tokens"
    --source-authoring-output-tokens "$source_authoring_output_tokens"
    --connect-timeout-ms 30000
    --header-timeout-ms "$header_timeout_ms"
    --total-timeout-ms "$total_timeout_ms"
    --model-lock "$state_root/model.lock"
    --autonomy-state "$runtime_workspace/autonomous/state.json"
    --action-receipts "$runtime_workspace/actions/receipts.jsonl"
    --thermal-celsius "$thermal_celsius"
    --maximum-thermal-celsius 85
    --helper "$cpu_root/astrid-edge-steward-helper"
    --helper-sha256 "$(hash_file "$cpu_root/astrid-edge-steward-helper")"
    --helper-install-path /usr/libexec/astrid/astrid-edge-steward-helper
    --supervisor "$stage/payload/edge-self-change-supervisor.pyz"
    --supervisor-sha256 "$(hash_file "$stage/payload/edge-self-change-supervisor.pyz")"
    --supervisor-install-path /usr/libexec/astrid/edge-self-change-supervisor
    --rescue-helper "$cpu_root/astrid-edge-rescue-helper"
    --rescue-helper-sha256 "$(hash_file "$cpu_root/astrid-edge-rescue-helper")"
    --rescue-helper-install-path /usr/libexec/astrid/astrid-edge-rescue-helper
    --checkpoint "$cpu_root/astrid-edge-checkpoint"
    --checkpoint-sha256 "$(hash_file "$cpu_root/astrid-edge-checkpoint")"
    --checkpoint-install-path /usr/libexec/astrid/astrid-edge-checkpoint
    --capsule-builder "$cpu_root/astrid-build"
    --capsule-builder-sha256 "$(hash_file "$cpu_root/astrid-build")"
    --capsule-builder-install-path /usr/libexec/astrid/astrid-build
    --web-broker "$cpu_root/astrid-edge-web-broker"
    --web-broker-sha256 "$(hash_file "$cpu_root/astrid-edge-web-broker")"
    --web-broker-install-path /usr/libexec/astrid-edge/immutable/astrid-edge-web-broker
    --provider-broker "$cpu_root/astrid-edge-provider-broker"
    --provider-broker-sha256 "$(hash_file "$cpu_root/astrid-edge-provider-broker")"
    --provider-broker-install-path /usr/libexec/astrid-edge/immutable/astrid-edge-provider-broker
    --presentation-broker "$cpu_root/astrid-edge-presentation-broker"
    --presentation-broker-sha256 "$(hash_file "$cpu_root/astrid-edge-presentation-broker")"
    --presentation-broker-install-path /usr/libexec/astrid-edge/immutable/astrid-edge-presentation-broker
    --source-signing-key "$stage/payload/portable-source.key"
    --source-signing-key-sha256 "$(hash_file "$stage/payload/portable-source.key")"
    --source-bundle "$stage/payload/portable-source.tar.gz"
    --source-bundle-sha256 "$(hash_file "$stage/payload/portable-source.tar.gz")"
    --toolchain-bundle "$stage/payload/pinned-toolchain.tar.gz"
    --toolchain-bundle-sha256 "$(hash_file "$stage/payload/pinned-toolchain.tar.gz")"
    --initial-generation-bundle "$stage/payload/initial-generation.tar.gz"
    --initial-generation-sha256 "$initial_generation_sha256"
    --initial-generation-id "$initial_generation_id"
    --state-root "$state_root"
    --release-root "$release_root"
    --source-root "$source_root"
    --candidate-root "$candidate_root"
    --builder-root "$builder_root"
    --updater-root "$updater_root"
    --inbox-root "$inbox_root"
    --vendor-root "$vendor_root"
    --toolchain-root "$toolchain_root"
    --unit-source-root "$unit_source_root"
    --system-unit-root /etc/systemd/system
    --user-unit-root "$runtime_home/.config/systemd/user"
    --control-root /usr/sbin
)
$dry_run && args=(--dry-run "${args[@]}")
[[ $appliance == icp ]] && args+=(--required-mount "$ICP_MOUNT" --required-mount-uuid "$required_mount_uuid")

for unit in "${system_stack[@]}"; do
    unit_path=$profile_unit_root/$unit
    [[ -f $unit_path && ! -L $unit_path ]] || die "profile unit is absent or unsafe: $unit"
    args+=(--astrid-system-unit "/etc/systemd/system/$unit")
    args+=(--astrid-system-unit-sha256 "$unit=$(hash_file "$unit_path")")
done
for unit in "${install_units[@]}"; do args+=(--install-unit "$unit"); done
for unit in "${enable_units[@]}"; do args+=(--enable-unit "$unit"); done

# Before the wrapper creates even the one required empty AVADO release anchor,
# run the immutable installer's complete mutation-free validation over these
# exact captured inputs and the live profile. The second invocation consumes
# the same private files and arguments; it does not rebuild an unchecked CLI.
if ! $dry_run; then
    "$root_installer" --dry-run "${args[@]}"
fi

# The immutable installer requires an existing root-owned release parent so it
# can validate ancestry before opening its own transaction. Create only this
# empty fixed AVADO anchor, only after every archive/profile/hash check, and
# never during a read-only dry-run.
if [[ $appliance == avado ]] && ! $dry_run; then
    "$PYTHON" -I -E -s - /opt <<'PY' || die "AVADO /opt ancestry is unsafe"
import os, stat, sys
metadata = os.lstat(sys.argv[1])
if not stat.S_ISDIR(metadata.st_mode) or stat.S_ISLNK(metadata.st_mode) or metadata.st_uid != 0 or metadata.st_mode & 0o022:
    raise SystemExit(1)
PY
    if [[ -e /opt/astrid-edge || -L /opt/astrid-edge ]]; then
        "$PYTHON" -I -E -s - /opt/astrid-edge <<'PY' \
            || die "AVADO release parent is not an exact root-owned directory"
import os, stat, sys
metadata = os.lstat(sys.argv[1])
if not stat.S_ISDIR(metadata.st_mode) or stat.S_ISLNK(metadata.st_mode) or metadata.st_uid != 0 or metadata.st_mode & 0o022:
    raise SystemExit(1)
PY
    else
        install -d -m 0755 -o root -g root /opt/astrid-edge
    fi
fi

printf 'Verified internal CPU-edge bundle integrity %s for %s (%s).\n' "$version" "$appliance" "$target"
printf 'Thermal guard: %s; candidate promotion begins paused.\n' "$thermal_celsius"
[[ $appliance == icp ]] && printf 'Retained backup guard: %s on UUID %s.\n' "$RETAINED_ICP_BACKUP" "$required_mount_uuid"
"$root_installer" "${args[@]}"
