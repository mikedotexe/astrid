#!/usr/bin/env bash
# Adversarial tests for the complete offline bundle entrypoint.
set -euo pipefail
IFS=$'\n\t'

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
INSTALLER_SOURCE=$SCRIPT_DIR/install_edge_self_evolution_bundle.sh
TEMP=$(mktemp -d)
TEMP=$(CDPATH= cd -- "$TEMP" && pwd -P)
trap 'rm -rf -- "$TEMP"' EXIT

fail() { printf 'FAIL: %s\n' "$*" >&2; exit 1; }
expect_failure() {
    local pattern=$1
    shift
    if "$@" >"$TEMP/failure.out" 2>"$TEMP/failure.err"; then
        fail "command unexpectedly succeeded: $pattern"
    fi
    grep -q -- "$pattern" "$TEMP/failure.err" || {
        cat "$TEMP/failure.err" >&2
        fail "missing failure: $pattern"
    }
}

THERMAL_ROOT=$TEMP/sys/class/thermal
ICP_MOUNT=$TEMP/media/data
BACKUP=$ICP_MOUNT/astrid/backups/emmc-20260729T130835Z
MOCK_FINDMNT=$TEMP/bin/findmnt
ARG_LOG=$TEMP/root-installer.args
HANDOFF_BASE=$TEMP/operator-handoff-root
mkdir -p "$THERMAL_ROOT/thermal_zone0" "$BACKUP" "$TEMP/bin"
printf 'x86_pkg_temp\n' >"$THERMAL_ROOT/thermal_zone0/type"
printf '43000\n' >"$THERMAL_ROOT/thermal_zone0/temp"
cat >"$MOCK_FINDMNT" <<EOF
#!/bin/sh
printf '%s %s\n' '$ICP_MOUNT' '0123-ABCD'
EOF
chmod 0700 "$MOCK_FINDMNT"

make_bundle() {
    local root=$1 attack=${2:-none}
    rm -rf -- "$root"
    mkdir -p "$root"
    /usr/bin/python3 - "$INSTALLER_SOURCE" "$root" "$THERMAL_ROOT" "$ICP_MOUNT" "$BACKUP" "$MOCK_FINDMNT" "$ARG_LOG" "$attack" "$(uname -s)" "$(uname -m)" <<'PY'
import gzip
import hashlib
import io
import json
import os
import sys
import tarfile
from pathlib import Path

(
    installer_source_raw,
    root_raw,
    thermal_root,
    icp_mount,
    backup,
    findmnt,
    argument_log,
    attack,
    kernel,
    machine,
) = sys.argv[1:]
root = Path(root_raw)
version = "test-v1"
target = "x86_64-unknown-linux-gnu"
cpu_root = f"astrid-cpu-edge-{version}-{target}"

installer = Path(installer_source_raw).read_text(encoding="utf-8")
replacements = {
    "/media/data": icp_mount,
    "readonly THERMAL_CLASS_ROOT=/sys/class/thermal": f"readonly THERMAL_CLASS_ROOT={thermal_root}",
    "readonly EXPECTED_KERNEL=Linux": f"readonly EXPECTED_KERNEL={kernel}",
    "readonly EXPECTED_MACHINE=x86_64": f"readonly EXPECTED_MACHINE={machine}",
    "readonly FINDMNT=/usr/bin/findmnt": f"readonly FINDMNT={findmnt}",
    "readonly OPERATOR_HANDOFF_BASE=/var/lib/astrid-edge-bootstrap": f"readonly OPERATOR_HANDOFF_BASE={Path(root_raw).parent / 'operator-handoff-root'}",
    "readonly IMMUTABLE_ROOT_UID=0": f"readonly IMMUTABLE_ROOT_UID={os.getuid()}",
    "readonly IMMUTABLE_ROOT_GID=0": f"readonly IMMUTABLE_ROOT_GID={os.getgid()}",
    'cursor = Path("/")\nfor part in extracted_root.parts[1:]:': 'cursor = base.parent\nfor part in extracted_root.relative_to(base.parent).parts:',
}
for old, new in replacements.items():
    installer = installer.replace(old, new)
(root / "install").write_text(installer, encoding="utf-8")
os.chmod(root / "install", 0o500)

system_stack = [
    "ollama-cpu.service",
    "astrid-model-warmup.service",
    "astrid.service",
    "astrid-edge-runtime.service",
    "astrid-edge-hindsight.service",
    "astrid-edge-hindsight.timer",
]
cpu_files = {
    "astrid-build": b"fixture astrid-build\n",
    "astrid-edge-checkpoint": b"fixture checkpoint\n",
    "astrid-edge-presentation-broker": b"fixture presentation\n",
    "astrid-edge-provider-broker": b"fixture provider\n",
    "astrid-edge-rescue-helper": b"fixture rescue\n",
    "astrid-edge-steward-helper": b"fixture steward\n",
    "astrid-edge-web-broker": b"fixture web\n",
}
mock_root_installer = f'''#!/usr/bin/env bash
set -euo pipefail
printf 'call\\n' >>'{argument_log}.calls'
printf '%s\\0' "$@" >'{argument_log}'
printf 'mock root installer accepted %s arguments\\n' "$#"
'''.encode()
cpu_files["scripts/install_edge_self_evolution_root.sh"] = mock_root_installer
for profile in ("avado", "icp"):
    prefix = "packaging/systemd" if profile == "avado" else "packaging/systemd/icp"
    for unit in system_stack:
        cpu_files[f"{prefix}/{unit}"] = f"{profile}:{unit}\n".encode()
build_manifest = {
    "schema": "astrid_cpu_edge_build_manifest_v3",
    "bundle_format": "cpu-edge.3",
    "version": version,
    "target": target,
    "expected_loaded_capsule_count": 20,
    "authority": "release_build_manifest_not_appliance_state_or_astrid_memory",
    "source_commit": "a" * 40,
}
cpu_files["BUILD-MANIFEST.json"] = (json.dumps(build_manifest, sort_keys=True) + "\n").encode()
cpu_sums = "".join(
    f"{hashlib.sha256(data).hexdigest()}  ./{name}\n"
    for name, data in sorted(cpu_files.items())
).encode()
cpu_files["SHA256SUMS"] = cpu_sums

cpu_archive = io.BytesIO()
with gzip.GzipFile(filename="", mode="wb", fileobj=cpu_archive, mtime=0) as compressed:
    with tarfile.open(fileobj=compressed, mode="w", format=tarfile.PAX_FORMAT) as archive:
        directories = {cpu_root}
        for name in cpu_files:
            parts = name.split("/")[:-1]
            for index in range(1, len(parts) + 1):
                directories.add(f"{cpu_root}/" + "/".join(parts[:index]))
        for name in sorted(directories, key=lambda value: (value.count("/"), value)):
            entry = tarfile.TarInfo(name)
            entry.type = tarfile.DIRTYPE
            entry.mode = 0o755
            entry.mtime = 0
            archive.addfile(entry)
        for name, data in sorted(cpu_files.items()):
            entry = tarfile.TarInfo(f"{cpu_root}/{name}")
            entry.size = len(data)
            entry.mode = 0o755 if name in {
                "astrid-build",
                "astrid-edge-checkpoint",
                "astrid-edge-presentation-broker",
                "astrid-edge-provider-broker",
                "astrid-edge-rescue-helper",
                "astrid-edge-steward-helper",
                "astrid-edge-web-broker",
                "scripts/install_edge_self_evolution_root.sh",
            } else 0o644
            entry.mtime = 0
            archive.addfile(entry, io.BytesIO(data))
        if attack == "traversal":
            data = b"escaped\n"
            entry = tarfile.TarInfo(f"{cpu_root}/../escape")
            entry.size = len(data)
            entry.mode = 0o644
            archive.addfile(entry, io.BytesIO(data))
        elif attack == "symlink":
            entry = tarfile.TarInfo(f"{cpu_root}/linked")
            entry.type = tarfile.SYMTYPE
            entry.linkname = "/etc/passwd"
            entry.mode = 0o644
            archive.addfile(entry)

payloads = {
    "cpu_edge_archive": ("payload/cpu-edge.tar.gz", cpu_archive.getvalue(), "0600"),
    "initial_generation": ("payload/initial-generation.tar.gz", b"generation fixture\n", "0600"),
    "portable_source": ("payload/portable-source.tar.gz", b"source fixture\n", "0600"),
    "pinned_toolchain": ("payload/pinned-toolchain.tar.gz", b"toolchain fixture\n", "0600"),
    "portable_source_key": ("payload/portable-source.key", b"K" * 32, "0600"),
    "immutable_supervisor": ("payload/edge-self-change-supervisor.pyz", b"#!/usr/bin/env python3\n", "0500"),
}
for _role, (name, data, mode) in payloads.items():
    path = root / name
    path.parent.mkdir(mode=0o700, exist_ok=True)
    path.write_bytes(data)
    os.chmod(path, int(mode, 8))

profiles = {
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
        "runtime_workspace": f"{icp_mount}/astrid/state/home/default/edge",
        "model": "qwen3:1.7b",
        "context_tokens": 2048,
        "output_tokens": 112,
        "source_authoring_output_tokens": 160,
        "header_timeout_ms": 420000,
        "total_timeout_ms": 660000,
        "audio": "explicitly_unavailable",
        "required_mount": icp_mount,
        "retained_backup": backup,
    },
}
manifest = {
    "schema": "astrid.edge.self_evolution_bootstrap.v1",
    "version": version,
    "target": target,
    "authority": "operator_release_bootstrap_not_model_authorship_or_appliance_authority",
    "portable_trust": "integrity_only_rebound_to_fresh_per_appliance_key_before_authorization",
    "ordinary_autonomy": "preserved",
    "initial_mode": "paused_bootstrap_acceptance_pending",
    "cross_appliance_or_mac_transfer": "forbidden",
    "profiles": profiles,
    "payloads": [
        {
            "role": role,
            "path": name,
            "bytes": len(data),
            "sha256": hashlib.sha256(data).hexdigest(),
            "mode": mode,
        }
        for role, (name, data, mode) in payloads.items()
    ],
}
(root / "MANIFEST.json").write_text(json.dumps(manifest, sort_keys=True, indent=2) + "\n", encoding="utf-8")
(root / "README.txt").write_text("test complete offline bundle\n", encoding="utf-8")
os.chmod(root / "MANIFEST.json", 0o600)
os.chmod(root / "README.txt", 0o600)
files = sorted(
    path.relative_to(root).as_posix()
    for path in root.rglob("*")
    if path.is_file() and path.name != "SHA256SUMS"
)
(root / "SHA256SUMS").write_text(
    "".join(f"{hashlib.sha256((root / name).read_bytes()).hexdigest()}  {name}\n" for name in files),
    encoding="ascii",
)
os.chmod(root / "SHA256SUMS", 0o600)
PY
}

make_handoff() {
    local extracted_root=$1 appliance=$2
    /usr/bin/python3 - "$extracted_root" "$HANDOFF_BASE" "$appliance" <<'PY'
import hashlib
import json
import os
import shutil
import sys
import tarfile
from pathlib import Path

source = Path(sys.argv[1])
base = Path(sys.argv[2])
appliance = sys.argv[3]
temporary_archive = source.parent / "trusted-release.tar.gz"
if base.exists():
    shutil.rmtree(base)
with tarfile.open(temporary_archive, "w:gz") as archive:
    archive.add(source, arcname=source.name, recursive=True)
digest = hashlib.sha256(temporary_archive.read_bytes()).hexdigest()
handoff_root = base / digest
handoff_root.mkdir(parents=True, mode=0o700)
os.chmod(base, 0o700)
os.chmod(handoff_root, 0o700)
extracted = handoff_root / source.name
source.rename(extracted)
os.chmod(extracted, 0o700)
retained_archive = handoff_root / "release.tar.gz"
temporary_archive.rename(retained_archive)
os.chmod(retained_archive, 0o400)
metadata = extracted.stat()
receipt = {
    "schema": "astrid.edge.self_evolution_operator_handoff.v1",
    "authority": "trusted_operator_verified_github_oidc_sigstore_release_handoff",
    "repository": "unicity-astrid/astrid",
    "signer_workflow": ".github/workflows/release.yml",
    "source_tag": "vtest-v1",
    "source_commit": "a" * 40,
    "outer_archive_sha256": digest,
    "outer_archive_bytes": retained_archive.stat().st_size,
    "outer_archive_path": str(retained_archive),
    "extracted_root": str(extracted),
    "extracted_root_device": metadata.st_dev,
    "extracted_root_inode": metadata.st_ino,
    "appliance": appliance,
}
receipt_path = handoff_root / "operator-handoff.json"
receipt_path.write_text(json.dumps(receipt, sort_keys=True, separators=(",", ":")) + "\n")
os.chmod(receipt_path, 0o400)
print(receipt_path)
PY
}

decode_args() {
    /usr/bin/python3 - "$ARG_LOG" "$1" <<'PY'
import sys
from pathlib import Path
raw = Path(sys.argv[1]).read_bytes()
if not raw.endswith(b"\0"):
    raise SystemExit("argument log is not NUL terminated")
Path(sys.argv[2]).write_text("\n".join(item.decode("utf-8") for item in raw[:-1].split(b"\0")) + "\n")
PY
}

assert_cli() {
    local appliance=$1 args_file=$2
    /usr/bin/python3 - "$appliance" "$args_file" "$ICP_MOUNT" "$THERMAL_ROOT/thermal_zone0/temp" <<'PY'
import hashlib
import sys
from pathlib import Path

appliance, args_file, icp_mount, thermal = sys.argv[1:]
args = Path(args_file).read_text().splitlines()

def values(flag):
    return [args[index + 1] for index, value in enumerate(args[:-1]) if value == flag]

def one(flag, expected):
    actual = values(flag)
    if actual != [expected]:
        raise SystemExit(f"{flag} mismatch: {actual!r} != {[expected]!r}")

if args[0] != "--dry-run" or args.count("--dry-run") != 1:
    raise SystemExit("dry-run was not forwarded exactly once")
if args.count("--start-system-services") != 1:
    raise SystemExit("service-state preservation flag missing")
if any(value in args for value in ("resume", "--resume", "rollback", "rescue")):
    raise SystemExit("bundle selected a control action")
one("--target", "x86_64-unknown-linux-gnu")
one("--thermal-celsius", str(Path(thermal).resolve()))
one("--maximum-thermal-celsius", "85")
one("--ollama-origin", "http://127.0.0.1:11434")
one("--connect-timeout-ms", "30000")

required_install = {
    "astrid-edge-self-change-supervisor.service",
    "astrid-edge-self-change-probation-health.service",
    "astrid-edge-self-change-probation-health.timer",
    "astrid-edge-steward.service",
    "astrid-edge-steward.timer",
    "astrid-edge-web-broker-core.socket",
    "astrid-edge-web-broker-core.service",
    "astrid-edge-web-broker-runtime.socket",
    "astrid-edge-web-broker-runtime.service",
    "astrid-edge-web-broker-steward.socket",
    "astrid-edge-web-broker-steward.service",
    "astrid-edge-provider-broker@.service",
    "astrid-edge-provider-runtime.socket",
    "astrid-edge-provider-steward.socket",
    "astrid-edge-provider-warmup.socket",
    "astrid-edge-presentation-broker.socket",
    "astrid-edge-presentation-broker@.service",
    "astrid-edge-generation-guard.service",
    "astrid-edge-core-liveness.service",
    "astrid-edge-core-liveness.path",
    "astrid-edge-self-change-inbox.path",
    "astrid-edge-runtime.service.d/60-self-evolution-root.conf",
}
required_enable = {
    "astrid-edge-steward.timer",
    "astrid-edge-self-change-probation-health.timer",
    "astrid-edge-web-broker-core.socket",
    "astrid-edge-web-broker-runtime.socket",
    "astrid-edge-web-broker-steward.socket",
    "astrid-edge-provider-runtime.socket",
    "astrid-edge-provider-steward.socket",
    "astrid-edge-provider-warmup.socket",
    "astrid-edge-presentation-broker.socket",
    "astrid-edge-generation-guard.service",
    "astrid-edge-core-liveness.path",
    "astrid-edge-self-change-inbox.path",
}
if set(values("--install-unit")) != required_install:
    raise SystemExit("install-unit set is not exact")
if set(values("--enable-unit")) != required_enable:
    raise SystemExit("enable-unit set is not exact")
if any("audio-feeder" in value for value in values("--install-unit") + values("--enable-unit")):
    raise SystemExit("wrapper passed audio units instead of relying on AVADO-only root auto-add")
if len(values("--astrid-system-unit")) != 6 or len(values("--astrid-system-unit-sha256")) != 6:
    raise SystemExit("six-unit root migration contract is incomplete")
system_stack = {
    "ollama-cpu.service",
    "astrid-model-warmup.service",
    "astrid.service",
    "astrid-edge-runtime.service",
    "astrid-edge-hindsight.service",
    "astrid-edge-hindsight.timer",
}
if set(values("--astrid-system-unit")) != {f"/etc/systemd/system/{unit}" for unit in system_stack}:
    raise SystemExit("authorized system-unit paths are not exact")
expected_unit_hashes = {
    f"{unit}={hashlib.sha256((appliance + ':' + unit + chr(10)).encode()).hexdigest()}"
    for unit in system_stack
}
if set(values("--astrid-system-unit-sha256")) != expected_unit_hashes:
    raise SystemExit("authorized system-unit hashes do not bind the selected profile")
if len(values("--steward-owned")) != 5:
    raise SystemExit("five canonical introspection inputs are incomplete")

if appliance == "avado":
    expected = {
        "--appliance-id": "avado-edge",
        "--runtime-user": "avado",
        "--runtime-home": "/home/avado",
        "--runtime-workspace": "/home/avado/.astrid/home/default/edge",
        "--model-ipc": "/home/avado/.astrid/run",
        "--model": "qwen3.5:4b",
        "--context-tokens": "4096",
        "--output-tokens": "192",
        "--source-authoring-output-tokens": "384",
        "--header-timeout-ms": "300000",
        "--total-timeout-ms": "600000",
        "--state-root": "/var/lib/astrid-edge-supervisor",
        "--release-root": "/opt/astrid-edge/releases",
        "--source-root": "/var/lib/astrid-edge-source",
        "--candidate-root": "/var/lib/astrid-edge-candidates",
        "--builder-root": "/var/lib/astrid-edge-builder",
        "--updater-root": "/var/lib/astrid-edge-updater",
        "--toolchain-root": "/opt/astrid-edge-toolchain",
    }
    if values("--required-mount") or values("--required-mount-uuid"):
        raise SystemExit("AVADO unexpectedly received an SSD mount guard")
else:
    expected = {
        "--appliance-id": "icp-edge",
        "--runtime-user": "nativeplanet",
        "--runtime-home": "/home/nativeplanet",
        "--runtime-workspace": f"{icp_mount}/astrid/state/home/default/edge",
        "--model-ipc": f"{icp_mount}/astrid/state/run",
        "--model": "qwen3:1.7b",
        "--context-tokens": "2048",
        "--output-tokens": "112",
        "--source-authoring-output-tokens": "160",
        "--header-timeout-ms": "420000",
        "--total-timeout-ms": "660000",
        "--state-root": f"{icp_mount}/astrid-edge-supervisor",
        "--release-root": f"{icp_mount}/astrid-edge-release-store/releases",
        "--source-root": f"{icp_mount}/astrid-edge-source",
        "--candidate-root": f"{icp_mount}/astrid-edge-candidates",
        "--builder-root": f"{icp_mount}/astrid-edge-builder",
        "--updater-root": f"{icp_mount}/astrid-edge-updater",
        "--toolchain-root": f"{icp_mount}/astrid-edge-toolchain",
        "--required-mount": icp_mount,
        "--required-mount-uuid": "0123-ABCD",
    }
for flag, expected_value in expected.items():
    one(flag, expected_value)
PY
}

BUNDLE=$TEMP/bundle
make_bundle "$BUNDLE"
"$BUNDLE/install" --dry-run --appliance avado >"$TEMP/avado.out"
decode_args "$TEMP/avado.args"
assert_cli avado "$TEMP/avado.args"
grep -q 'candidate promotion begins paused' "$TEMP/avado.out" || fail "paused AVADO bootstrap was not reported"

"$BUNDLE/install" --appliance icp --dry-run >"$TEMP/icp.out"
decode_args "$TEMP/icp.args"
assert_cli icp "$TEMP/icp.args"
grep -q 'Retained backup guard:' "$TEMP/icp.out" || fail "ICP backup guard was not reported"

# Exercise the hidden non-dry trust handoff without host mutation: this fixture
# substitutes a private UID/GID/base, while the production constants below are
# separately asserted to remain root:root under /var/lib.
HANDOFF_SOURCE=$TEMP/astrid-edge-self-evolution-test-v1-x86_64-unknown-linux-gnu
make_bundle "$HANDOFF_SOURCE"
HANDOFF_RECEIPT=$(make_handoff "$HANDOFF_SOURCE" icp)
HANDOFF_ROOT=${HANDOFF_RECEIPT%/operator-handoff.json}
HANDOFF_INSTALL=$HANDOFF_ROOT/astrid-edge-self-evolution-test-v1-x86_64-unknown-linux-gnu/install
: >"$ARG_LOG.calls"
"$HANDOFF_INSTALL" --operator-handoff "$HANDOFF_RECEIPT" --appliance icp >"$TEMP/handoff.out"
[[ $(wc -l <"$ARG_LOG.calls" | tr -d ' ') == 2 ]] \
    || fail "non-dry handoff did not run one read-only root preflight plus one installation"
decode_args "$TEMP/handoff.args"
! grep -Fxq -- '--dry-run' "$TEMP/handoff.args" || fail "final trusted-root invocation remained a dry-run"
! grep -Fxq -- '--operator-handoff' "$TEMP/handoff.args" || fail "wrapper leaked its trust receipt into the root installer CLI"
grep -Fxq -- 'icp-edge' "$TEMP/handoff.args" || fail "trusted-root invocation selected the wrong appliance profile"
grep -q 'candidate promotion begins paused' "$TEMP/handoff.out" || fail "trusted-root installation did not report paused promotion"

expect_failure 'requires the trusted operator handoff' "$HANDOFF_INSTALL" --appliance icp
expect_failure 'reserved for a non-dry trusted-root installation' \
    "$HANDOFF_INSTALL" --dry-run --operator-handoff "$HANDOFF_RECEIPT" --appliance icp
expect_failure 'identity or attestation binding is not exact' \
    "$HANDOFF_INSTALL" --operator-handoff "$HANDOFF_RECEIPT" --appliance avado

chmod 0600 "$HANDOFF_RECEIPT"
expect_failure 'not exact immutable-root material' \
    "$HANDOFF_INSTALL" --operator-handoff "$HANDOFF_RECEIPT" --appliance icp
chmod 0400 "$HANDOFF_RECEIPT"

ln "$HANDOFF_ROOT/release.tar.gz" "$TEMP/retained-archive-hardlink"
expect_failure 'not exact immutable-root material' \
    "$HANDOFF_INSTALL" --operator-handoff "$HANDOFF_RECEIPT" --appliance icp
rm "$TEMP/retained-archive-hardlink"

chmod 0600 "$HANDOFF_RECEIPT"
/usr/bin/python3 - "$HANDOFF_RECEIPT" <<'PY'
import json, sys
from pathlib import Path
path = Path(sys.argv[1])
value = json.loads(path.read_text())
value["source_commit"] = "b" * 40
path.write_text(json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n")
PY
chmod 0400 "$HANDOFF_RECEIPT"
expect_failure 'does not match the CPU-edge build manifest' \
    "$HANDOFF_INSTALL" --operator-handoff "$HANDOFF_RECEIPT" --appliance icp

# Restore the exact receipt, then prove that retained archive substitution is
# independently detected even though the extracted tree is unchanged.
chmod 0600 "$HANDOFF_RECEIPT"
/usr/bin/python3 - "$HANDOFF_RECEIPT" <<'PY'
import json, sys
from pathlib import Path
path = Path(sys.argv[1])
value = json.loads(path.read_text())
value["source_commit"] = "a" * 40
path.write_text(json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n")
PY
chmod 0400 "$HANDOFF_RECEIPT"
chmod 0600 "$HANDOFF_ROOT/release.tar.gz"
printf 'substitution\n' >>"$HANDOFF_ROOT/release.tar.gz"
chmod 0400 "$HANDOFF_ROOT/release.tar.gz"
expect_failure 'size does not match the operator handoff' \
    "$HANDOFF_INSTALL" --operator-handoff "$HANDOFF_RECEIPT" --appliance icp

# Corruption in either inventory layer fails before the root installer runs.
printf 'tamper\n' >>"$BUNDLE/payload/portable-source.tar.gz"
expect_failure 'manifest payload mismatch' "$BUNDLE/install" --dry-run --appliance avado

make_bundle "$BUNDLE" traversal
expect_failure 'unsafe path, type, mode, or alias' "$BUNDLE/install" --dry-run --appliance avado
[[ ! -e $TEMP/escape ]] || fail "nested traversal escaped extraction root"

make_bundle "$BUNDLE" symlink
expect_failure 'unsafe path, type, mode, or alias' "$BUNDLE/install" --dry-run --appliance avado

make_bundle "$BUNDLE"
rm "$BUNDLE/payload/portable-source.key"
ln -s /etc/passwd "$BUNDLE/payload/portable-source.key"
expect_failure 'link or special member' "$BUNDLE/install" --dry-run --appliance avado

make_bundle "$BUNDLE"
printf 'unexpected\n' >"$BUNDLE/extra"
chmod 0600 "$BUNDLE/extra"
expect_failure 'bundle inventory is not exact' "$BUNDLE/install" --dry-run --appliance avado

expect_failure 'must be avado or icp' "$BUNDLE/install" --dry-run --appliance other
expect_failure 'unsupported argument' "$BUNDLE/install" --dry-run --appliance avado --resume

# Production constants remain exact even though runtime fixtures substitute
# private paths to exercise both profiles without touching either live host.
grep -Fq 'readonly ICP_MOUNT=/media/data' "$INSTALLER_SOURCE" || fail "production ICP mount changed"
grep -Fq 'readonly RETAINED_ICP_BACKUP=/media/data/astrid/backups/emmc-20260729T130835Z' "$INSTALLER_SOURCE" || fail "production backup guard changed"
grep -Fq 'state_root=/var/lib/astrid-edge-supervisor' "$INSTALLER_SOURCE" || fail "production AVADO state root changed"
grep -Fq 'readonly OPERATOR_HANDOFF_BASE=/var/lib/astrid-edge-bootstrap' "$INSTALLER_SOURCE" || fail "production operator handoff root changed"
grep -Fq 'readonly IMMUTABLE_ROOT_UID=0' "$INSTALLER_SOURCE" || fail "production handoff receipt owner is not root"
grep -Fq 'readonly IMMUTABLE_ROOT_GID=0' "$INSTALLER_SOURCE" || fail "production handoff receipt group is not root"
grep -Fq 'astrid.edge.self_evolution_operator_handoff.v1' "$INSTALLER_SOURCE" || fail "operator handoff schema is not bound"
grep -Fq 'trusted_operator_verified_github_oidc_sigstore_release_handoff' "$INSTALLER_SOURCE" || fail "operator handoff authority is not exact"
grep -Fq 'GitHub OIDC/Sigstore' "$INSTALLER_SOURCE" || fail "external release-authentication prerequisite is not explicit"
! grep -Fq 'authenticity remains an operator responsibility through the published archive sidecar' "$INSTALLER_SOURCE" || fail "integrity sidecar is mislabeled as authenticity"
! grep -Eq '(^|[[:space:]])(rm|unlink)[[:space:]].*emmc-20260729T130835Z' "$INSTALLER_SOURCE" || fail "installer contains backup deletion"
! grep -Eq '/Users/|/Applications/|spectral-bridge|minime' "$INSTALLER_SOURCE" || fail "installer contains Mac/Minime/bridge paths"

printf 'edge self-evolution bundle installer tests passed\n'
