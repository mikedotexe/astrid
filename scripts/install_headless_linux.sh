#!/usr/bin/env bash
# Install release binaries and a persistent systemd user service.

set -euo pipefail

usage() {
    cat <<'EOF'
Usage: scripts/install_headless_linux.sh [OPTIONS]

Install Astrid core binaries and the matching systemd user-service layout.
Initialize the selected state tree before starting the service for the first time.

Options:
  --binary-dir DIR  Directory containing astrid, astrid-daemon, and astrid-build
  --layout NAME     Install layout: standard or icp-ssd (default: standard)
  --start           Enable and (re)start astrid.service after installation
  --dry-run         Print the operations without changing the host
  -h, --help        Show this help

The default binary directory is target/release in a source checkout, or the
top level of an extracted Astrid release archive.
EOF
}

script_dir="$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
project_root="$(CDPATH= cd -- "$script_dir/.." && pwd)"

if [[ -x "$project_root/target/release/astrid" ]]; then
    binary_dir="$project_root/target/release"
else
    binary_dir="$project_root"
fi

start_service=0
dry_run=0
layout="standard"

while (( $# > 0 )); do
    case "$1" in
        --binary-dir)
            if (( $# < 2 )); then
                printf 'error: --binary-dir requires a directory\n' >&2
                exit 2
            fi
            binary_dir="$2"
            shift 2
            ;;
        --start)
            start_service=1
            shift
            ;;
        --layout)
            if (( $# < 2 )) || [[ "$2" != "standard" && "$2" != "icp-ssd" ]]; then
                printf 'error: --layout requires standard or icp-ssd\n' >&2
                exit 2
            fi
            layout="$2"
            shift 2
            ;;
        --dry-run)
            dry_run=1
            shift
            ;;
        -h | --help)
            usage
            exit 0
            ;;
        *)
            printf 'error: unknown argument: %s\n' "$1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

if [[ "$(uname -s)" != "Linux" ]]; then
    printf 'error: the headless service installer supports Linux only\n' >&2
    exit 1
fi
if [[ -z "${HOME:-}" || "$HOME" != /* ]]; then
    printf 'error: HOME must be set to an absolute path\n' >&2
    exit 1
fi
if ! command -v systemctl >/dev/null 2>&1; then
    printf 'error: systemd is required; see docs/headless-linux.md for manual startup\n' >&2
    exit 1
fi

for binary in astrid astrid-daemon astrid-build; do
    if [[ ! -x "$binary_dir/$binary" ]]; then
        printf 'error: executable not found: %s/%s\n' "$binary_dir" "$binary" >&2
        exit 1
    fi
done

warmup_script_source="$project_root/scripts/warm_ollama_model.sh"
if [[ "$layout" == "icp-ssd" ]]; then
    unit_source="$project_root/packaging/systemd/icp/astrid.service"
    warmup_unit_source="$project_root/packaging/systemd/icp/astrid-model-warmup.service"
    ollama_unit_source="$project_root/packaging/systemd/icp/ollama-cpu.service"
    ssd_guard_source="$project_root/packaging/systemd/icp-ssd-required.conf"
    astrid_home="$HOME/.astrid-icp/state"
else
    unit_source="$project_root/packaging/systemd/astrid.service"
    warmup_unit_source="$project_root/packaging/systemd/astrid-model-warmup.service"
    local_ollama_source="$project_root/packaging/systemd/astrid-local-ollama.conf"
    ollama_unit_source=""
    ssd_guard_source=""
    astrid_home="$HOME/.astrid"
fi
required_files=(
    "$unit_source"
    "$warmup_unit_source"
    "$warmup_script_source"
)
if [[ "$layout" == "icp-ssd" ]]; then
    required_files+=("$ollama_unit_source" "$ssd_guard_source")
else
    required_files+=("$local_ollama_source")
fi
for required_file in "${required_files[@]}"; do
    if [[ ! -f "$required_file" ]]; then
        printf 'error: required install asset not found: %s\n' "$required_file" >&2
        exit 1
    fi
done

install_bin="$astrid_home/bin"
unit_dir="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
unit_dest="$unit_dir/astrid.service"
unit_dropin_dir="$unit_dir/astrid.service.d"

run() {
    printf '+'
    printf ' %q' "$@"
    printf '\n'
    if (( dry_run == 0 )); then
        "$@"
    fi
}

if [[ "$layout" == "icp-ssd" ]]; then
    icp_link="$HOME/.astrid-icp"
    icp_ssd_root="/media/data/astrid"
    if (( dry_run == 0 )); then
        if ! command -v mountpoint >/dev/null 2>&1 || ! mountpoint -q /media/data; then
            printf 'error: ICP layout requires the SSD mounted at /media/data\n' >&2
            exit 1
        fi
    fi
    if [[ -L "$icp_link" ]]; then
        resolved_icp_link="$(readlink -f -- "$icp_link" 2>/dev/null || true)"
        if [[ "$resolved_icp_link" != "$icp_ssd_root" ]]; then
            printf 'error: %s must resolve exactly to %s (found %s)\n' \
                "$icp_link" "$icp_ssd_root" "${resolved_icp_link:-unresolved}" >&2
            exit 1
        fi
    elif [[ -e "$icp_link" ]]; then
        printf 'error: refusing ICP install into non-symlink path %s; archive/migrate it to %s first\n' \
            "$icp_link" "$icp_ssd_root" >&2
        exit 1
    elif (( dry_run == 1 )); then
        printf '+ require mounted writable SSD directory %q\n' "$icp_ssd_root"
        run ln -s -- "$icp_ssd_root" "$icp_link"
    else
        if [[ ! -d "$icp_ssd_root" || ! -w "$icp_ssd_root" ]]; then
            printf 'error: ICP SSD root must already exist and be owner-writable: %s\n' \
                "$icp_ssd_root" >&2
            exit 1
        fi
        run ln -s -- "$icp_ssd_root" "$icp_link"
    fi
fi

run install -d -m 0700 "$astrid_home" "$install_bin" "$astrid_home/etc"
if [[ "$layout" == "icp-ssd" ]]; then
    run install -d -m 0700 \
        "$HOME/.astrid-icp" \
        "$HOME/.astrid-icp/workspace" \
        "$HOME/.astrid-icp/tmp" \
        "$HOME/.astrid-icp/ollama" \
        "$HOME/.astrid-icp/ollama/models"
    run install -d -m 0755 \
        "$unit_dir" \
        "$unit_dir/astrid.service.d" \
        "$unit_dir/ollama-cpu.service.d" \
        "$unit_dir/astrid-model-warmup.service.d" \
        "$unit_dir/astrid-edge-runtime.service.d"
else
    run install -d -m 0755 "$unit_dir" "$unit_dropin_dir"
fi
for binary in astrid astrid-daemon astrid-build; do
    run install -m 0755 "$binary_dir/$binary" "$install_bin/$binary"
done
run install -m 0644 "$unit_source" "$unit_dest"
run install -m 0644 "$warmup_unit_source" "$unit_dir/astrid-model-warmup.service"
if [[ "$layout" == "icp-ssd" ]]; then
    run install -m 0644 "$ollama_unit_source" "$unit_dir/ollama-cpu.service"
    for service in \
        astrid.service \
        ollama-cpu.service \
        astrid-model-warmup.service \
        astrid-edge-runtime.service; do
        run install -m 0644 \
            "$ssd_guard_source" \
            "$unit_dir/$service.d/ssd-required.conf"
    done
else
    run install -m 0644 "$local_ollama_source" "$unit_dropin_dir/local-ollama.conf"
fi
run install -m 0755 "$warmup_script_source" "$install_bin/warm-ollama-model"
run systemctl --user daemon-reload

if (( start_service == 1 )); then
    if [[ "$layout" == "icp-ssd" ]]; then
        run systemctl --user enable \
            ollama-cpu.service \
            astrid-model-warmup.service \
            astrid.service
        run systemctl --user restart ollama-cpu.service
    else
        run systemctl --user enable astrid-model-warmup.service astrid.service
    fi
    run systemctl --user restart astrid-model-warmup.service
    run systemctl --user restart astrid.service
fi

printf '\nInstalled Astrid binaries in %s\n' "$install_bin"
printf 'Installed user service at %s\n' "$unit_dest"
printf 'Selected install layout: %s\n' "$layout"

if (( start_service == 0 )); then
    printf '\nInitialize and start Astrid with:\n'
    if [[ "$layout" == "icp-ssd" ]]; then
        printf '  cd %q && ASTRID_HOME=%q %q init\n' \
            "$HOME/.astrid-icp/workspace" "$astrid_home" "$install_bin/astrid"
    else
        printf '  %q init\n' "$install_bin/astrid"
    fi
    printf '  systemctl --user enable --now astrid.service\n'
fi

if command -v loginctl >/dev/null 2>&1; then
    linger="$(loginctl show-user "${USER:-$(id -un)}" -p Linger --value 2>/dev/null || true)"
    if [[ "$linger" != "yes" ]]; then
        printf '\nFor startup before login, an administrator must run:\n'
        printf '  sudo loginctl enable-linger %q\n' "${USER:-$(id -un)}"
    fi
fi
