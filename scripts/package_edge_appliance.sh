#!/usr/bin/env bash
# Assemble a versioned, self-verifying CPU-edge appliance archive.

set -euo pipefail

usage() {
    cat <<'EOF'
Usage: scripts/package_edge_appliance.sh --version VERSION --target TARGET \
  --core-binary-dir DIR --edge-binary FILE --capsule-dir DIR [--output-dir DIR]

The capsule directory must contain all ten version-matched essential
`.capsule` archives. The output is an installable CPU-edge tar.gz plus a
SHA-256 sidecar. TARGET must be x86_64-unknown-linux-gnu or
aarch64-unknown-linux-gnu.
EOF
}

version=""
target=""
core_binary_dir=""
edge_binary=""
capsule_dir=""
output_dir="dist"

while (( $# > 0 )); do
    case "$1" in
        --version) version="${2:-}"; shift 2 ;;
        --target) target="${2:-}"; shift 2 ;;
        --core-binary-dir) core_binary_dir="${2:-}"; shift 2 ;;
        --edge-binary) edge_binary="${2:-}"; shift 2 ;;
        --capsule-dir) capsule_dir="${2:-}"; shift 2 ;;
        --output-dir) output_dir="${2:-}"; shift 2 ;;
        -h | --help) usage; exit 0 ;;
        *) printf 'error: unknown argument: %s\n' "$1" >&2; usage >&2; exit 2 ;;
    esac
done

if [[ -z "$version" || -z "$target" || -z "$core_binary_dir" || -z "$edge_binary" || -z "$capsule_dir" ]]; then
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

essential_capsules=(
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
python3 - "$target" \
    "$core_binary_dir/astrid" \
    "$core_binary_dir/astrid-daemon" \
    "$core_binary_dir/astrid-build" \
    "$edge_binary" <<'PY'
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
for capsule in "${essential_capsules[@]}"; do
    if [[ ! -f "$capsule_dir/$capsule.capsule" ]]; then
        printf 'error: missing capsule archive: %s\n' "$capsule_dir/$capsule.capsule" >&2
        exit 1
    fi
done

incremental_code_bytes="$(wc -c < "$edge_binary")"
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
for capsule in "${essential_capsules[@]}"; do
    install -m 0644 "$capsule_dir/$capsule.capsule" "$bundle/capsules/"
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
install -m 0644 "$project_root/docs/headless-linux.md" "$bundle/docs/"
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
install -m 0644 "$project_root/README.md" "$bundle/"
for license in "$project_root"/LICENSE*; do
    [[ -f "$license" ]] && install -m 0644 "$license" "$bundle/"
done

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
    "${#essential_capsules[@]}" \
    "20" \
    "$incremental_code_bytes" <<'PY'
import json
import pathlib
import sys
import time

root = pathlib.Path(sys.argv[1])
files = sorted(
    path.relative_to(root).as_posix()
    for path in root.rglob("*")
    if path.is_file()
)
manifest = {
    "schema": "astrid_cpu_edge_build_manifest_v2",
    "bundle_format": "cpu-edge.2",
    "version": sys.argv[2],
    "target": sys.argv[3],
    "binary_format": "elf64-little-endian",
    "binary_architecture_verified": True,
    "source_commit": sys.argv[4],
    "source_tree_state": sys.argv[5],
    "rustc": sys.argv[6],
    "essential_capsule_count": int(sys.argv[7]),
    "expected_loaded_capsule_count": int(sys.argv[8]),
    "incremental_installed_code_bytes": int(sys.argv[9]),
    "incremental_installed_code_ceiling_bytes": 20 * 1024 * 1024,
    "linux_glibc_build_baseline": "Ubuntu 22.04 / glibc 2.35",
    "generated_at_unix_ms": time.time_ns() // 1_000_000,
    "files_before_inventory": files,
    "authority": "release_build_manifest_not_appliance_state_or_astrid_memory",
}
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
printf '%s\n' "$archive"
