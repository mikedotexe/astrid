#!/usr/bin/env bash
# Build and install the version-matched, in-tree Astralis bootstrap capsules.

set -euo pipefail

usage() {
    cat <<'EOF'
Usage: scripts/install_essential_capsules.sh [OPTIONS]

Build and install the ten restored Astralis capsules shipped in this source
checkout. These provide the CLI compatibility uplink, filesystem, HTTP, shell,
skills, AGENTS.md, memory, read-only edge-reservoir context, and private edge
self-inspection and spectral-observation surfaces. They
do not install an LLM provider, session router, or copy credentials.

Options:
  --build-jobs N  Cargo jobs for each Component Model build (default: online CPUs)
  --capsule-dir DIR  Install prebuilt .capsule archives instead of building
  --layout NAME   Astrid state layout: standard or icp-ssd (default: standard)
  --restart       Restart astrid.service and verify all ten capsules load
  --dry-run       Verify inputs and print install operations without changes
  -h, --help      Show this help
EOF
}

script_dir="$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
project_root="$(CDPATH= cd -- "$script_dir/.." && pwd)"
status_verifier="$script_dir/verify_edge_capsule_status.py"
build_jobs="$(getconf _NPROCESSORS_ONLN 2>/dev/null || printf '1\n')"
prebuilt_capsule_dir=""
if [[ -f "$project_root/capsules/astrid-capsule-edge-context.capsule" ]]; then
    prebuilt_capsule_dir="$project_root/capsules"
fi
restart_service=0
dry_run=0
layout="standard"

while (( $# > 0 )); do
    case "$1" in
        --build-jobs)
            if (( $# < 2 )) || [[ ! "$2" =~ ^[1-9][0-9]*$ ]]; then
                printf 'error: --build-jobs requires a positive integer\n' >&2
                exit 2
            fi
            build_jobs="$2"
            shift 2
            ;;
        --capsule-dir)
            if (( $# < 2 )) || [[ ! -d "$2" ]]; then
                printf 'error: --capsule-dir requires a readable directory\n' >&2
                exit 2
            fi
            prebuilt_capsule_dir="$2"
            shift 2
            ;;
        --layout)
            if (( $# < 2 )) || [[ "$2" != "standard" && "$2" != "icp-ssd" ]]; then
                printf 'error: --layout requires standard or icp-ssd\n' >&2
                exit 2
            fi
            layout="$2"
            shift 2
            ;;
        --restart)
            restart_service=1
            shift
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

if [[ -z "${HOME:-}" || "$HOME" != /* ]]; then
    printf 'error: HOME must be set to an absolute path\n' >&2
    exit 1
fi
if [[ "$layout" == "icp-ssd" ]]; then
    astrid_state_home="$HOME/.astrid-icp/state"
else
    astrid_state_home="$HOME/.astrid"
fi

if [[ -x "$project_root/target/release/astrid" ]]; then
    astrid_bin="$project_root/target/release/astrid"
elif [[ -x "$project_root/astrid" ]]; then
    astrid_bin="$project_root/astrid"
elif [[ "$layout" == "icp-ssd" && -x "$astrid_state_home/bin/astrid" ]]; then
    astrid_bin="$astrid_state_home/bin/astrid"
elif [[ -x "${HOME:-}/.astrid/bin/astrid" ]]; then
    astrid_bin="$HOME/.astrid/bin/astrid"
elif command -v astrid >/dev/null 2>&1; then
    astrid_bin="$(command -v astrid)"
else
    printf 'error: astrid binary not found; build or install Astrid first\n' >&2
    exit 1
fi

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
        if [[ ! -L "$icp_link" ]]; then
            printf 'error: ICP layout requires %s to be an SSD symlink\n' "$icp_link" >&2
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
        printf 'error: refusing ICP capsule install into non-symlink path %s\n' \
            "$icp_link" >&2
        exit 1
    elif (( dry_run == 1 )); then
        printf '+ require %q to resolve exactly to %q\n' "$icp_link" "$icp_ssd_root"
    fi
fi

if [[ -z "$prebuilt_capsule_dir" ]] && ! command -v rustup >/dev/null 2>&1; then
    printf 'error: rustup is required to build the in-tree capsules\n' >&2
    exit 1
fi
if [[ ! -f "$status_verifier" || ! -r "$status_verifier" ]]; then
    printf 'error: loaded-capsule verifier is unavailable: %s\n' "$status_verifier" >&2
    exit 1
fi

capsules=(
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
output_dir="$project_root/dist/essential"
if [[ -n "$prebuilt_capsule_dir" ]]; then
    output_dir="$prebuilt_capsule_dir"
    printf 'Using prebuilt essential capsules from %s\n' "$output_dir"
else
    run rustup target add wasm32-wasip2
    run install -d -m 0755 "$output_dir"

    for capsule in "${capsules[@]}"; do
        source_dir="$project_root/capsules/astralis/$capsule"
        if [[ ! -f "$source_dir/Capsule.toml" || ! -f "$source_dir/Cargo.toml" ]]; then
            printf 'error: in-tree capsule source is incomplete: %s\n' "$source_dir" >&2
            exit 1
        fi

        printf 'Building %s with %s Cargo job(s)...\n' "$capsule" "$build_jobs"
        if (( dry_run == 0 )); then
            CARGO_BUILD_JOBS="$build_jobs" "$astrid_bin" build \
                --type rust-component \
                --output "$output_dir" \
                "$source_dir"
        else
            printf '+ CARGO_BUILD_JOBS=%q %q build --type rust-component --output %q %q\n' \
                "$build_jobs" "$astrid_bin" "$output_dir" "$source_dir"
        fi
    done
fi

for capsule in "${capsules[@]}"; do
    archive="$output_dir/$capsule.capsule"
    if [[ ! -f "$archive" ]]; then
        if (( dry_run == 0 )) || [[ -n "$prebuilt_capsule_dir" ]]; then
            printf 'error: expected capsule archive was not built: %s\n' "$archive" >&2
            exit 1
        fi
    fi

    printf 'Installing %s...\n' "$capsule"
    # EOF accepts manifest defaults for the two non-secret cwd_dir prompts.
    if (( dry_run == 0 )); then
        env ASTRID_HOME="$astrid_state_home" \
            "$astrid_bin" capsule install "$archive" </dev/null
    else
        printf '+ ASTRID_HOME=%q %q capsule install %q </dev/null\n' \
            "$astrid_state_home" "$astrid_bin" "$archive"
    fi
done

printf 'Installed %s version-matched bootstrap capsules.\n' "${#capsules[@]}"

if (( restart_service == 1 )); then
    if ! command -v systemctl >/dev/null 2>&1; then
        printf 'error: --restart requires systemd\n' >&2
        exit 1
    fi

    if (( dry_run == 1 )); then
        printf '+ systemctl --user restart astrid.service\n'
        printf '+ verify all ten essential capsules and exactly 20 total capsules loaded\n'
        exit 0
    fi
    systemctl --user restart astrid.service
    for _attempt in 1 2 3 4 5 6 7 8 9 10; do
        if systemctl --user is-active --quiet astrid.service; then
            status_output="$(
                env ASTRID_HOME="$astrid_state_home" \
                    "$astrid_bin" --format json status 2>/dev/null || true
            )"
            verifier_arguments=()
            for capsule in "${capsules[@]}"; do
                verifier_arguments+=(--required "$capsule")
            done
            if verification="$(
                printf '%s\n' "$status_output" \
                    | python3 "$status_verifier" \
                        --expected-total 20 \
                        "${verifier_arguments[@]}" \
                        2>/dev/null
            )"; then
                printf '%s\n' "$verification"
                exit 0
            fi
        fi
        sleep 1
    done

    printf 'error: astrid.service did not report all ten essential capsules and exactly 20 total loaded capsules\n' >&2
    systemctl --user status astrid.service --no-pager -l >&2 || true
    exit 1
fi
