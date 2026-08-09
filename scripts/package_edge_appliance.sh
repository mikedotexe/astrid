#!/usr/bin/env bash
# Assemble a versioned, self-verifying CPU-edge appliance archive.

set -euo pipefail

usage() {
    cat <<'EOF'
Usage: scripts/package_edge_appliance.sh --version VERSION --target TARGET \
  --core-binary-dir DIR --edge-binary FILE --steward-helper FILE \
  --rescue-helper FILE --web-broker FILE --provider-broker FILE --presentation-broker FILE \
  --checkpoint-helper FILE \
  --capsule-dir DIR --external-capsule-dir DIR [--output-dir DIR]

The capsule directory must contain exactly the ten version-matched,
repository-local CPU-edge `.capsule` archives. The external capsule
directory must contain exactly the ten signed baseline archives that form the
provider/ReAct/session cognition graph. The output is an installable CPU-edge
tar.gz plus a SHA-256 sidecar. TARGET must be x86_64-unknown-linux-gnu or
aarch64-unknown-linux-gnu.
EOF
}

version=""
target=""
core_binary_dir=""
edge_binary=""
steward_helper=""
rescue_helper=""
web_broker=""
provider_broker=""
presentation_broker=""
checkpoint_helper=""
capsule_dir=""
external_capsule_dir=""
output_dir="dist"

while (( $# > 0 )); do
    case "$1" in
        --version) version="${2:-}"; shift 2 ;;
        --target) target="${2:-}"; shift 2 ;;
        --core-binary-dir) core_binary_dir="${2:-}"; shift 2 ;;
        --edge-binary) edge_binary="${2:-}"; shift 2 ;;
        --steward-helper) steward_helper="${2:-}"; shift 2 ;;
        --rescue-helper) rescue_helper="${2:-}"; shift 2 ;;
        --web-broker) web_broker="${2:-}"; shift 2 ;;
        --provider-broker) provider_broker="${2:-}"; shift 2 ;;
        --presentation-broker) presentation_broker="${2:-}"; shift 2 ;;
        --checkpoint-helper) checkpoint_helper="${2:-}"; shift 2 ;;
        --capsule-dir) capsule_dir="${2:-}"; shift 2 ;;
        --external-capsule-dir) external_capsule_dir="${2:-}"; shift 2 ;;
        --output-dir) output_dir="${2:-}"; shift 2 ;;
        -h | --help) usage; exit 0 ;;
        *) printf 'error: unknown argument: %s\n' "$1" >&2; usage >&2; exit 2 ;;
    esac
done

if [[ -z "$version" || -z "$target" || -z "$core_binary_dir" || -z "$edge_binary" || -z "$steward_helper" || -z "$rescue_helper" || -z "$web_broker" || -z "$provider_broker" || -z "$presentation_broker" || -z "$checkpoint_helper" || -z "$capsule_dir" || -z "$external_capsule_dir" ]]; then
    usage >&2
    exit 2
fi
if [[ ! "$version" =~ ^[A-Za-z0-9][A-Za-z0-9._-]*$ ]]; then
    printf 'error: version must contain only letters, digits, dot, underscore, or hyphen\n' >&2
    exit 2
fi
case "$target" in
    x86_64-unknown-linux-gnu | aarch64-unknown-linux-gnu) ;;
    *)
        printf 'error: unsupported CPU-edge target: %s\n' "$target" >&2
        exit 2
        ;;
esac

script_dir="$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
project_root="$(CDPATH= cd -- "$script_dir/.." && pwd)"
bundle_name="astrid-cpu-edge-${version}-${target}"
staging_parent="$(mktemp -d)"
trap 'rm -rf -- "$staging_parent"' EXIT
bundle="$staging_parent/$bundle_name"

local_source_capsules=(
    astrid-capsule-cli
    astrid-capsule-fs
    astrid-capsule-http
    astrid-capsule-shell
    astrid-capsule-skills
    astrid-capsule-agents
    astrid-capsule-memory
    astrid-capsule-edge-context
    astrid-capsule-edge-introspector
    astrid-capsule-edge-spectral
)
external_source_capsules=(
    astrid-capsule-context-engine
    astrid-capsule-hook-bridge
    astrid-capsule-identity
    astrid-capsule-openai-compat
    astrid-capsule-prompt-builder
    astrid-capsule-react
    astrid-capsule-registry
    astrid-capsule-router
    astrid-capsule-session
    astrid-capsule-system
)
all_capsules=("${local_source_capsules[@]}" "${external_source_capsules[@]}")
for binary in astrid astrid-daemon astrid-build; do
    if [[ ! -x "$core_binary_dir/$binary" ]]; then
        printf 'error: missing core binary: %s\n' "$core_binary_dir/$binary" >&2
        exit 1
    fi
done
if [[ ! -x "$edge_binary" ]]; then
    printf 'error: missing edge runtime binary: %s\n' "$edge_binary" >&2
    exit 1
fi
for helper in "$steward_helper" "$rescue_helper" "$web_broker" "$provider_broker" "$presentation_broker" "$checkpoint_helper"; do
    if [[ ! -x "$helper" ]]; then
        printf 'error: missing immutable CPU-edge helper: %s\n' "$helper" >&2
        exit 1
    fi
done
python3 - "$target" \
    "$core_binary_dir/astrid" \
    "$core_binary_dir/astrid-daemon" \
    "$core_binary_dir/astrid-build" \
    "$edge_binary" \
    "$steward_helper" \
    "$rescue_helper" \
    "$web_broker" \
    "$provider_broker" \
    "$presentation_broker" \
    "$checkpoint_helper" <<'PY'
import pathlib
import struct
import sys

target = sys.argv[1]
expected_machine = {
    "x86_64-unknown-linux-gnu": 62,
    "aarch64-unknown-linux-gnu": 183,
}[target]
for raw_path in sys.argv[2:]:
    path = pathlib.Path(raw_path)
    with path.open("rb") as handle:
        header = handle.read(20)
    if len(header) < 20 or header[:4] != b"\x7fELF":
        raise SystemExit(f"error: packaged binary is not ELF: {path}")
    if header[4] != 2 or header[5] != 1:
        raise SystemExit(f"error: packaged binary is not little-endian ELF64: {path}")
    elf_type, machine = struct.unpack_from("<HH", header, 16)
    if elf_type not in {2, 3}:
        raise SystemExit(f"error: packaged ELF is not executable/shared: {path}")
    if machine != expected_machine:
        raise SystemExit(
            f"error: packaged ELF machine {machine} does not match target {target}: {path}"
        )
PY
python3 - "$capsule_dir" "$external_capsule_dir" \
    "${#local_source_capsules[@]}" "${local_source_capsules[@]}" \
    "${#external_source_capsules[@]}" "${external_source_capsules[@]}" <<'PY'
import os
import pathlib
import re
import stat
import sys
import tarfile

local_root = pathlib.Path(sys.argv[1])
external_root = pathlib.Path(sys.argv[2])
local_count = int(sys.argv[3])
local_names = sys.argv[4:4 + local_count]
external_count_index = 4 + local_count
external_count = int(sys.argv[external_count_index])
external_names = sys.argv[external_count_index + 1:]
if len(external_names) != external_count:
    raise SystemExit("error: internal external-source capsule argument count is inconsistent")

def validate_root(root: pathlib.Path, names: list[str], label: str) -> None:
    metadata = root.lstat()
    if not stat.S_ISDIR(metadata.st_mode) or stat.S_ISLNK(metadata.st_mode):
        raise SystemExit(f"error: {label} capsule root is not a real directory: {root}")
    expected = {f"{name}.capsule" for name in names}
    actual = {entry.name for entry in os.scandir(root)}
    if actual != expected:
        missing = sorted(expected - actual)
        extra = sorted(actual - expected)
        raise SystemExit(
            f"error: {label} capsule root is not exact; missing={missing} extra={extra}"
        )
    for filename in sorted(expected):
        path = root / filename
        metadata = path.lstat()
        if not stat.S_ISREG(metadata.st_mode) or stat.S_ISLNK(metadata.st_mode) or metadata.st_nlink != 1:
            raise SystemExit(f"error: {label} capsule archive is linked or non-regular: {path}")
        if metadata.st_size <= 0 or metadata.st_size > 64 * 1024 * 1024:
            raise SystemExit(f"error: {label} capsule archive size is invalid: {path}")
        names_seen: set[str] = set()
        component_count = 0
        expanded = 0
        capsule_manifest = b""
        try:
            with tarfile.open(path, "r:gz") as archive:
                for member in archive:
                    pure = pathlib.PurePosixPath(member.name)
                    if pure.is_absolute() or not pure.parts or any(part in {"", ".", ".."} for part in pure.parts):
                        raise ValueError("archive path escapes its root")
                    normalized = pure.as_posix()
                    if normalized in names_seen:
                        raise ValueError("archive contains duplicate entries")
                    names_seen.add(normalized)
                    if member.isdir():
                        continue
                    if not member.isfile() or member.size > 16 * 1024 * 1024:
                        raise ValueError("archive contains a link, special, or oversized entry")
                    expanded += member.size
                    if expanded > 64 * 1024 * 1024 or len(names_seen) > 256:
                        raise ValueError("archive inventory exceeds bounds")
                    if pure.suffix.lower() == ".wasm":
                        stream = archive.extractfile(member)
                        header = stream.read(8) if stream is not None else b""
                        if header != b"\x00asm\x0d\x00\x01\x00":
                            raise ValueError("WASM payload is not a Component Model binary")
                        component_count += 1
                    elif normalized == "Capsule.toml":
                        stream = archive.extractfile(member)
                        capsule_manifest = stream.read(1024 * 1024 + 1) if stream is not None else b""
                        if len(capsule_manifest) > 1024 * 1024:
                            raise ValueError("Capsule.toml is oversized")
        except (OSError, tarfile.TarError, ValueError) as error:
            raise SystemExit(f"error: invalid {label} capsule archive {path}: {error}") from error
        if "Capsule.toml" not in names_seen or component_count != 1:
            raise SystemExit(
                f"error: {label} capsule archive lacks Capsule.toml or an exact single component: {path}"
            )
        try:
            manifest_text = capsule_manifest.decode("utf-8")
        except UnicodeDecodeError as error:
            raise SystemExit(f"error: {label} capsule manifest is not UTF-8: {path}") from error
        package = re.search(r"(?ms)^\[package\]\s*(.*?)(?=^\[|\Z)", manifest_text)
        identity = re.search(r'(?m)^\s*name\s*=\s*"([A-Za-z0-9._-]+)"\s*$', package.group(1) if package else "")
        expected_identity = filename.removesuffix(".capsule")
        if identity is None or identity.group(1) != expected_identity:
            raise SystemExit(
                f"error: {label} capsule manifest identity does not match filename: {path}"
            )

validate_root(local_root, local_names, "repository-local")
validate_root(external_root, external_names, "external pinned-source")
if set(local_names) & set(external_names):
    raise SystemExit("error: local and external capsule identities overlap")
PY

incremental_code_bytes="$(wc -c < "$edge_binary")"
incremental_code_bytes="$((incremental_code_bytes + $(wc -c < "$steward_helper") + $(wc -c < "$rescue_helper") + $(wc -c < "$web_broker") + $(wc -c < "$provider_broker") + $(wc -c < "$presentation_broker") + $(wc -c < "$checkpoint_helper")))"
for capsule in \
    astrid-capsule-edge-context \
    astrid-capsule-edge-introspector \
    astrid-capsule-edge-spectral; do
    capsule_bytes="$(wc -c < "$capsule_dir/$capsule.capsule")"
    incremental_code_bytes="$((incremental_code_bytes + capsule_bytes))"
done
if (( incremental_code_bytes > 20 * 1024 * 1024 )); then
    printf 'error: incremental CPU-edge code exceeds the 20 MiB installed ceiling: %s bytes\n' \
        "$incremental_code_bytes" >&2
    exit 1
fi

install -d -m 0755 \
    "$bundle/capsules" \
    "$bundle/docs" \
    "$bundle/packaging" \
    "$bundle/scripts"
install -m 0755 \
    "$core_binary_dir/astrid" \
    "$core_binary_dir/astrid-daemon" \
    "$core_binary_dir/astrid-build" \
    "$edge_binary" \
    "$bundle/"
install -m 0755 "$steward_helper" "$bundle/astrid-edge-steward-helper"
install -m 0755 "$rescue_helper" "$bundle/astrid-edge-rescue-helper"
install -m 0755 "$web_broker" "$bundle/astrid-edge-web-broker"
install -m 0755 "$provider_broker" "$bundle/astrid-edge-provider-broker"
install -m 0755 "$presentation_broker" "$bundle/astrid-edge-presentation-broker"
install -m 0755 "$checkpoint_helper" "$bundle/astrid-edge-checkpoint"
for capsule in "${local_source_capsules[@]}"; do
    install -m 0644 "$capsule_dir/$capsule.capsule" "$bundle/capsules/"
done
for capsule in "${external_source_capsules[@]}"; do
    install -m 0644 "$external_capsule_dir/$capsule.capsule" "$bundle/capsules/"
done

cp -R "$project_root/packaging/appliances" "$bundle/packaging/"
install -d -m 0755 "$bundle/packaging/headless"
for asset in "$project_root"/packaging/headless/*; do
    case "$(basename -- "$asset")" in
        introspection-AGENTS.md | introspection-memory.md)
            # These optional operator templates describe an inherited Mac
            # corpus. Independent CPU-edge bundles must carry no Mac paths or
            # inheritance instructions.
            continue
            ;;
    esac
    cp -R "$asset" "$bundle/packaging/headless/"
done
cp -R "$project_root/packaging/systemd" "$bundle/packaging/"
install -m 0644 \
    "$project_root/docs/headless-linux.md" \
    "$project_root/docs/cpu-edge-self-evolution.md" \
    "$bundle/docs/"
for asset in \
    build_astralis_cpu_edge_capsules.py \
    install_headless_linux.sh \
    install_headless_application_capsules.py \
    install_edge_runtime.sh \
    install_essential_capsules.sh \
    verify_edge_capsule_status.py \
    probe_headless_linux.sh \
    warm_ollama_model.sh \
    report_edge_appliance.py \
    report_edge_appliance.sh \
    report_edge_activity.py \
    report_edge_fleet_activity.py \
    relay_edge_peer_review.py \
    edge_audio_feeder.py \
    edge_hindsight.py \
    astrid_at_a_glance.py \
    migrate_edge_safe_fallback_authorship.py \
    migrate_edge_transport_sentinels.py \
    migrate_edge_operator_harness_isolation.py \
    reconcile_edge_interrupted_actions.py \
    retire_edge_autonomy_session.py \
    benchmark_headless_models.py \
    benchmark_headless_models.sh \
    finish_avado_host_hardening.sh \
    finish_icp_host_hardening.sh \
    finish_icp_host_hardening_stage2.sh; do
    install -m 0755 "$project_root/scripts/$asset" "$bundle/scripts/"
done
for asset in \
    build_edge_self_change_source_bundle.py \
    build_edge_self_change_toolchain_bundle.py \
    build_edge_self_change_supervisor_zipapp.py \
    edge_self_change_supervisor.py \
    install_edge_self_evolution_root.sh; do
    install -m 0755 "$project_root/scripts/$asset" "$bundle/scripts/"
done
cp -R "$project_root/scripts/edge_self_change" "$bundle/scripts/"
# Recursive source copies can contain local interpreter caches even though
# those files are untracked.  Appliance archives contain source and immutable
# assets only, never developer-machine build products.
find "$bundle" -type d -name __pycache__ -prune -exec rm -rf -- {} +
find "$bundle" -type f \( -name '*.pyc' -o -name '*.pyo' \) -delete
install -m 0644 "$project_root/README.md" "$bundle/"
for license in "$project_root"/LICENSE*; do
    [[ -f "$license" ]] && install -m 0644 "$license" "$bundle/"
done

# The archive is assembled from an operator workspace that may be dirty.  Its
# explicitly copied roots still must not smuggle host state, credentials,
# model blobs, databases, logs, authored artifacts, links, or special files
# into a first-class appliance release.
python3 - "$bundle" <<'PY'
import pathlib
import stat
import sys

root = pathlib.Path(sys.argv[1])
forbidden_components = {
    "backups", "credentials", "home", "journals", "models",
    "operator-quarantine", "private-keys", "secrets", "state",
}
forbidden_suffixes = {
    ".db", ".gguf", ".key", ".log", ".onnx", ".pem",
    ".pyo", ".pyc", ".safetensors", ".sqlite", ".sqlite3",
}
for path in root.rglob("*"):
    relative = path.relative_to(root)
    metadata = path.lstat()
    if stat.S_ISLNK(metadata.st_mode) or not (
        stat.S_ISREG(metadata.st_mode) or stat.S_ISDIR(metadata.st_mode)
    ):
        raise SystemExit(f"error: linked or special archive member: {relative}")
    if any(part.startswith(".") or part in forbidden_components for part in relative.parts):
        raise SystemExit(f"error: private or hidden archive member: {relative}")
    if path.is_file() and path.suffix.lower() in forbidden_suffixes:
        raise SystemExit(f"error: credential/state/model artifact entered archive: {relative}")
PY

source_commit="$(git -C "$project_root" rev-parse HEAD 2>/dev/null || printf 'unknown')"
source_tree_state="unavailable"
if git -C "$project_root" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    source_tree_state="clean"
    if [[ -n "$(git -C "$project_root" status --porcelain --untracked-files=normal)" ]]; then
        source_tree_state="dirty"
    fi
fi
rustc_version="$(rustc --version 2>/dev/null || printf 'unknown')"
python3 - \
    "$bundle" \
    "$version" \
    "$target" \
    "$source_commit" \
    "$source_tree_state" \
    "$rustc_version" \
    "${#local_source_capsules[@]}" \
    "${#external_source_capsules[@]}" \
    "${#all_capsules[@]}" \
    "$incremental_code_bytes" <<'PY'
import json
import pathlib
import sys
import time

root = pathlib.Path(sys.argv[1])
local_source = {
    "astrid-capsule-agents", "astrid-capsule-cli", "astrid-capsule-edge-context",
    "astrid-capsule-edge-introspector", "astrid-capsule-edge-spectral",
    "astrid-capsule-fs", "astrid-capsule-http", "astrid-capsule-memory",
    "astrid-capsule-shell", "astrid-capsule-skills",
}
external_source = {
    "astrid-capsule-context-engine", "astrid-capsule-hook-bridge",
    "astrid-capsule-identity", "astrid-capsule-openai-compat",
    "astrid-capsule-prompt-builder", "astrid-capsule-react",
    "astrid-capsule-registry", "astrid-capsule-router",
    "astrid-capsule-session", "astrid-capsule-system",
}
files = sorted(
    path.relative_to(root).as_posix()
    for path in root.rglob("*")
    if path.is_file()
)
if any("__pycache__" in pathlib.PurePosixPath(name).parts or name.endswith((".pyc", ".pyo")) for name in files):
    raise SystemExit("error: interpreter cache entered the CPU-edge archive")
manifest = {
    "schema": "astrid_cpu_edge_build_manifest_v3",
    "bundle_format": "cpu-edge.3",
    "version": sys.argv[2],
    "target": sys.argv[3],
    "binary_format": "elf64-little-endian",
    "binary_architecture_verified": True,
    "source_commit": sys.argv[4],
    "source_tree_state": sys.argv[5],
    "rustc": sys.argv[6],
    "essential_capsule_count": int(sys.argv[9]),
    "local_source_capsule_count": int(sys.argv[7]),
    "external_pinned_source_capsule_count": int(sys.argv[8]),
    "rebuildable_capsule_count": int(sys.argv[9]),
    "packaged_capsule_count": int(sys.argv[9]),
    "expected_loaded_capsule_count": int(sys.argv[9]),
    "capsule_archives": [
        {
            "capsule_id": path.stem,
            "class": "local_repository_source" if path.stem in local_source else "external_pinned_source",
            "path": path.relative_to(root).as_posix(),
            "sha256": __import__("hashlib").sha256(path.read_bytes()).hexdigest(),
        }
        for path in sorted((root / "capsules").glob("*.capsule"))
        if path.stem in local_source | external_source
    ],
    "incremental_installed_code_bytes": int(sys.argv[10]),
    "incremental_installed_code_ceiling_bytes": 20 * 1024 * 1024,
    "linux_glibc_build_baseline": "Ubuntu 22.04 / glibc 2.35",
    "generated_at_unix_ms": time.time_ns() // 1_000_000,
    "files_before_inventory": files,
    "authority": "release_build_manifest_not_appliance_state_or_astrid_memory",
    "self_evolution": {
        "scheduled_reflection": "immutable_root_owned_two_hour_coalesced",
        "candidate_authority": "exact_model_output_separately_attested",
        "build_network": "offline_locked_vendor_only",
        "activation": "ab_probation_with_automatic_rollback",
        "mac_minime_bridge_scope": "excluded",
    },
}
if len(manifest["capsule_archives"]) != 20:
    raise SystemExit("error: build manifest capsule inventory is not the exact twenty")
(root / "BUILD-MANIFEST.json").write_text(
    json.dumps(manifest, sort_keys=True, indent=2) + "\n",
    encoding="utf-8",
)
PY

(
    cd "$bundle"
    find . -type f ! -name SHA256SUMS -print0 \
        | sort -z \
        | xargs -0 sha256sum \
        > SHA256SUMS
)
mkdir -p "$output_dir"
archive="$output_dir/$bundle_name.tar.gz"
tar -C "$staging_parent" -czf "$archive" "$bundle_name"
archive_bytes="$(wc -c < "$archive")"
if (( archive_bytes > 60 * 1024 * 1024 )); then
    printf 'error: CPU-edge archive exceeds the 60 MiB release ceiling: %s bytes\n' \
        "$archive_bytes" >&2
    exit 1
fi
sha256sum "$archive" > "$archive.sha256"
generation_root="$staging_parent/astrid-edge-generation"
# The outer appliance archive carries installers, immutable rescue helpers,
# documentation, and operator bootstrap material. The active A/B generation is
# a separate exact mutable runtime payload matching rescue `assemble_bundle`;
# dormant privilege-boundary programs must never enter it.
install -d -m 0755 "$generation_root/capsules" "$generation_root/scripts" "$generation_root/packaging"
install -m 0755 \
    "$core_binary_dir/astrid" \
    "$core_binary_dir/astrid-daemon" \
    "$core_binary_dir/astrid-build" \
    "$edge_binary" \
    "$generation_root/"
for capsule in "${local_source_capsules[@]}"; do
    install -m 0644 "$capsule_dir/$capsule.capsule" "$generation_root/capsules/"
done
for capsule in "${external_source_capsules[@]}"; do
    install -m 0644 "$external_capsule_dir/$capsule.capsule" "$generation_root/capsules/"
done
for script in \
    warm_ollama_model.sh \
    report_edge_appliance.py \
    report_edge_appliance.sh \
    report_edge_activity.py \
    report_edge_fleet_activity.py \
    edge_hindsight.py \
    astrid_at_a_glance.py; do
    install -m 0755 "$project_root/scripts/$script" "$generation_root/scripts/"
done
cp -R "$project_root/packaging/appliances" "$generation_root/packaging/"
cp -R "$project_root/packaging/systemd" "$generation_root/packaging/"
python3 - "$generation_root" "$version" "$target" <<'PY'
import hashlib
import json
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
inventory = []
for path in sorted(item for item in root.rglob("*") if item.is_file()):
    data = path.read_bytes()
    inventory.append({
        "path": path.relative_to(root).as_posix(),
        "size": len(data),
        "sha256": hashlib.sha256(data).hexdigest(),
    })
manifest = {
    "schema": "astrid.edge_self_change.initial_generation.v1",
    "appliance_id": "portable-bootstrap-non-authorizing",
    "version": sys.argv[2],
    "target": sys.argv[3],
    "inventory": inventory,
    "authority": "operator_packaged_initial_generation_not_model_candidate",
}
(root / ".astrid-edge-generation.json").write_text(
    json.dumps(manifest, sort_keys=True, separators=(",", ":")) + "\n",
    encoding="utf-8",
)
PY
find "$generation_root" -type d -exec chmod 0755 {} +
find "$generation_root" -type f -exec chmod 0644 {} +
find "$generation_root" -type f \( -name astrid -o -name astrid-daemon -o -name astrid-build -o -name astrid-edge-runtime -o -path '*/scripts/*' -o -path '*/packaging/systemd/*' \) -exec chmod 0755 {} +
generation_archive="$output_dir/astrid-edge-generation-${version}-${target}.tar.gz"
tar -C "$staging_parent" -czf "$generation_archive" astrid-edge-generation
sha256sum "$generation_archive" > "$generation_archive.sha256"
printf '%s\n' "$archive"
printf '%s\n' "$generation_archive"
