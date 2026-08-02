#!/usr/bin/env bash
# Build and install the CPU-native reservoir/action sidecar for a Linux appliance.

set -euo pipefail

usage() {
    cat <<'EOF'
Usage: scripts/install_edge_runtime.sh [OPTIONS]

Options:
  --build-jobs N  Cargo jobs (default: all detected logical CPUs)
  --binary FILE   Prebuilt astrid-edge-runtime (skips the source build)
  --profile NAME  Appliance profile name or .env path (default: generic-cpu)
  --layout NAME   Install layout: auto, standard, or icp-ssd (default: auto)
  --observation-only  Disable tuning authority (default; use for first spectral rollout)
  --enable-tuning    Explicitly enable tuning when the profile permits it
  --start         Enable and restart astrid-edge-runtime.service
  --dry-run       Print install/service operations without changing the host
  -h, --help      Show this help
EOF
}

script_dir="$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
project_root="$(CDPATH= cd -- "$script_dir/.." && pwd)"
manifest="$project_root/services/astrid-edge-runtime/Cargo.toml"
warmup_script_source="$project_root/scripts/warm_ollama_model.sh"
report_source="$project_root/scripts/report_edge_appliance.py"
activity_report_source="$project_root/scripts/report_edge_activity.py"
hindsight_source="$project_root/scripts/edge_hindsight.py"
dashboard_source="$project_root/scripts/astrid_at_a_glance.py"
transport_migration_source="$project_root/scripts/migrate_edge_transport_sentinels.py"
operator_harness_migration_source="$project_root/scripts/migrate_edge_operator_harness_isolation.py"
interrupted_action_reconciliation_source="$project_root/scripts/reconcile_edge_interrupted_actions.py"
tuning_authority_dropin_source="$project_root/packaging/systemd/astrid-edge-tuning-authority.conf"
observation_only_env_source="$project_root/packaging/systemd/astrid-edge-observation-only.env"
tuning_enabled_env_source="$project_root/packaging/systemd/astrid-edge-tuning-enabled.env"
build_jobs="$(getconf _NPROCESSORS_ONLN 2>/dev/null || printf '1')"
prebuilt_binary=""
if [[ -x "$project_root/astrid-edge-runtime" ]]; then
    prebuilt_binary="$project_root/astrid-edge-runtime"
fi
profile="generic-cpu"
layout="auto"
start_service=0
dry_run=0
tuning_mode="observation-only"
tuning_mode_explicit=0

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
        --binary)
            if (( $# < 2 )) || [[ ! -x "$2" ]]; then
                printf 'error: --binary requires an executable file\n' >&2
                exit 2
            fi
            prebuilt_binary="$2"
            shift 2
            ;;
        --profile)
            if (( $# < 2 )) || [[ -z "$2" ]]; then
                printf 'error: --profile requires a name or .env path\n' >&2
                exit 2
            fi
            profile="$2"
            shift 2
            ;;
        --layout)
            if (( $# < 2 )) || [[ "$2" != "auto" && "$2" != "standard" && "$2" != "icp-ssd" ]]; then
                printf 'error: --layout requires auto, standard, or icp-ssd\n' >&2
                exit 2
            fi
            layout="$2"
            shift 2
            ;;
        --start)
            start_service=1
            shift
            ;;
        --observation-only)
            if (( tuning_mode_explicit == 1 )) && [[ "$tuning_mode" != "observation-only" ]]; then
                printf 'error: --observation-only and --enable-tuning are mutually exclusive\n' >&2
                exit 2
            fi
            tuning_mode="observation-only"
            tuning_mode_explicit=1
            shift
            ;;
        --enable-tuning)
            if (( tuning_mode_explicit == 1 )) && [[ "$tuning_mode" != "enabled" ]]; then
                printf 'error: --observation-only and --enable-tuning are mutually exclusive\n' >&2
                exit 2
            fi
            tuning_mode="enabled"
            tuning_mode_explicit=1
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

if [[ "$(uname -s)" != "Linux" ]]; then
    printf 'error: astrid-edge-runtime currently targets Linux /proc host telemetry\n' >&2
    exit 1
fi
if [[ -z "${HOME:-}" || "$HOME" != /* ]]; then
    printf 'error: HOME must be an absolute path\n' >&2
    exit 1
fi
if [[ -z "$prebuilt_binary" ]] && ! command -v cargo >/dev/null 2>&1; then
    printf 'error: cargo is required to build astrid-edge-runtime\n' >&2
    exit 1
fi
if ! command -v systemctl >/dev/null 2>&1; then
    printf 'error: systemd is required for this installer\n' >&2
    exit 1
fi

if [[ "$profile" == */* || "$profile" == *.env ]]; then
    profile_source="$profile"
else
    profile_source="$project_root/packaging/appliances/$profile.env"
fi
if [[ ! -f "$profile_source" || ! -r "$profile_source" ]]; then
    printf 'error: appliance profile not readable: %s\n' "$profile_source" >&2
    exit 1
fi
edge_context_source="${profile_source%.env}.edge-context.json"
if [[ ! -f "$edge_context_source" || ! -r "$edge_context_source" ]]; then
    printf 'error: matching edge-context profile not readable: %s\n' \
        "$edge_context_source" >&2
    exit 1
fi

profile_basename="$(basename -- "$profile_source" .env)"
if [[ "$layout" == "auto" ]]; then
    if [[ "$profile_basename" == icp-* ]]; then
        layout="icp-ssd"
    else
        layout="standard"
    fi
fi
if [[ "$layout" == "icp-ssd" ]]; then
    astrid_home="$HOME/.astrid-icp/state"
    unit_source="$project_root/packaging/systemd/icp/astrid-edge-runtime.service"
    warmup_unit_source="$project_root/packaging/systemd/icp/astrid-model-warmup.service"
    hindsight_unit_source="$project_root/packaging/systemd/icp/astrid-edge-hindsight.service"
    hindsight_timer_source="$project_root/packaging/systemd/icp/astrid-edge-hindsight.timer"
    ssd_guard_source="$project_root/packaging/systemd/icp-ssd-required.conf"
else
    astrid_home="$HOME/.astrid"
    unit_source="$project_root/packaging/systemd/astrid-edge-runtime.service"
    warmup_unit_source="$project_root/packaging/systemd/astrid-model-warmup.service"
    hindsight_unit_source="$project_root/packaging/systemd/astrid-edge-hindsight.service"
    hindsight_timer_source="$project_root/packaging/systemd/astrid-edge-hindsight.timer"
    ssd_guard_source=""
fi
edge_workspace="$astrid_home/home/default/edge"
for required_file in \
    "$unit_source" \
    "$warmup_unit_source" \
    "$warmup_script_source" \
    "$report_source" \
    "$activity_report_source" \
    "$hindsight_source" \
    "$dashboard_source" \
    "$transport_migration_source" \
    "$operator_harness_migration_source" \
    "$interrupted_action_reconciliation_source" \
    "$tuning_authority_dropin_source" \
    "$observation_only_env_source" \
    "$tuning_enabled_env_source" \
    "$hindsight_unit_source" \
    "$hindsight_timer_source"; do
    if [[ ! -f "$required_file" || ! -r "$required_file" ]]; then
        printf 'error: required install asset not readable: %s\n' "$required_file" >&2
        exit 1
    fi
done
if [[ "$layout" == "icp-ssd" && ( ! -f "$ssd_guard_source" || ! -r "$ssd_guard_source" ) ]]; then
    printf 'error: required ICP SSD service guard not readable: %s\n' "$ssd_guard_source" >&2
    exit 1
fi
if grep -Ev '^[[:space:]]*(#.*)?$|^[A-Z][A-Z0-9_]*=(\"[^\"]*\"|[^[:space:]#]+)[[:space:]]*$' \
    "$profile_source" >/dev/null; then
    printf 'error: invalid EnvironmentFile syntax in %s\n' "$profile_source" >&2
    exit 1
fi
if [[ "$tuning_mode" == "enabled" ]] \
    && ! grep -Eq '^[[:space:]]*ASTRID_EDGE_RESERVOIR_TUNING_ENABLED=("true"|true)[[:space:]]*$' \
        "$profile_source"; then
    printf 'error: --enable-tuning requires a profile that explicitly permits reservoir tuning\n' >&2
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
            printf 'error: ICP layout requires %s to be a symlink created by install_headless_linux.sh\n' \
                "$icp_link" >&2
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
        printf 'error: refusing ICP install into non-symlink path %s\n' "$icp_link" >&2
        exit 1
    elif (( dry_run == 1 )); then
        printf '+ require %q to resolve exactly to %q\n' "$icp_link" "$icp_ssd_root"
    fi
fi

if [[ -n "$prebuilt_binary" ]]; then
    binary="$prebuilt_binary"
    printf 'Using prebuilt astrid-edge-runtime at %s\n' "$binary"
else
    source_commit="$(git -C "$project_root" rev-parse HEAD 2>/dev/null || printf 'unknown')"
    if [[ "$source_commit" != "unknown" ]] \
        && ! git -C "$project_root" diff --quiet --ignore-submodules -- 2>/dev/null; then
        source_commit="${source_commit}-dirty"
    fi
    printf 'Building astrid-edge-runtime with %s jobs (source %s)\n' "$build_jobs" "$source_commit"
    if (( dry_run == 0 )); then
        ASTRID_EDGE_SOURCE_COMMIT="$source_commit" \
            cargo build \
            --release \
            --manifest-path "$manifest" \
            --jobs "$build_jobs"
    else
        printf '+ ASTRID_EDGE_SOURCE_COMMIT=%q cargo build --release --manifest-path %q --jobs %q\n' \
            "$source_commit" "$manifest" "$build_jobs"
    fi
    binary="$project_root/services/astrid-edge-runtime/target/release/astrid-edge-runtime"
fi
unit_dir="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
profile_dir="${XDG_CONFIG_HOME:-$HOME/.config}/astrid"
capsule_env_dir="$astrid_home/home/default/.config/env"
run install -d -m 0700 \
    "$edge_workspace" \
    "$edge_workspace/spectral" \
    "$edge_workspace/tuning" \
    "$edge_workspace/tuning/evidence" \
    "$astrid_home/operator/hindsight" \
    "$astrid_home/bin"
run install -d -m 0700 "$capsule_env_dir"
run install -d -m 0755 "$unit_dir" "$unit_dir/astrid-edge-runtime.service.d"
if [[ "$layout" == "icp-ssd" ]]; then
    run install -d -m 0755 \
        "$unit_dir/astrid.service.d" \
        "$unit_dir/ollama-cpu.service.d" \
        "$unit_dir/astrid-model-warmup.service.d" \
        "$unit_dir/astrid-edge-runtime.service.d"
fi
run install -d -m 0700 "$profile_dir"
run install -m 0755 "$binary" "$astrid_home/bin/astrid-edge-runtime"
run install -m 0755 "$warmup_script_source" "$astrid_home/bin/warm-ollama-model"
run install -m 0755 "$report_source" "$astrid_home/bin/report-edge-appliance"
run install -m 0755 "$activity_report_source" "$astrid_home/bin/report-edge-activity"
run install -m 0755 "$activity_report_source" "$astrid_home/bin/report_edge_activity.py"
run install -m 0755 "$hindsight_source" "$astrid_home/bin/edge-hindsight"
run install -m 0755 "$hindsight_source" "$HOME/astrid-hindsight"
run install -m 0755 "$transport_migration_source" \
    "$astrid_home/bin/migrate-edge-transport-sentinels"
run install -m 0755 "$operator_harness_migration_source" \
    "$astrid_home/bin/migrate-edge-operator-harness-isolation"
run install -m 0755 "$interrupted_action_reconciliation_source" \
    "$astrid_home/bin/reconcile-edge-interrupted-actions"
run install -m 0755 "$dashboard_source" "$HOME/astrid-at-a-glance"
run install -m 0600 "$profile_source" "$profile_dir/edge-appliance.env"
if [[ "$tuning_mode" == "enabled" ]]; then
    tuning_authority_env_source="$tuning_enabled_env_source"
else
    tuning_authority_env_source="$observation_only_env_source"
fi
run install -m 0600 \
    "$tuning_authority_env_source" \
    "$profile_dir/edge-tuning-authority.env"
run install -m 0600 "$edge_context_source" \
    "$capsule_env_dir/astrid-capsule-edge-context.env.json"
run install -m 0644 "$unit_source" "$unit_dir/astrid-edge-runtime.service"
run install -m 0644 "$warmup_unit_source" "$unit_dir/astrid-model-warmup.service"
run install -m 0644 "$hindsight_unit_source" "$unit_dir/astrid-edge-hindsight.service"
run install -m 0644 "$hindsight_timer_source" "$unit_dir/astrid-edge-hindsight.timer"
run install -m 0644 \
    "$tuning_authority_dropin_source" \
    "$unit_dir/astrid-edge-runtime.service.d/10-tuning-authority.conf"
if [[ "$layout" == "icp-ssd" ]]; then
    for service in \
        astrid.service \
        ollama-cpu.service \
        astrid-model-warmup.service \
        astrid-edge-runtime.service; do
        run install -m 0644 \
            "$ssd_guard_source" \
            "$unit_dir/$service.d/ssd-required.conf"
    done
fi
run systemctl --user daemon-reload

for activity_ledger in \
    "$edge_workspace/actions/receipts.jsonl" \
    "$edge_workspace/autonomous/runs.jsonl" \
    "$edge_workspace/autonomous/chains.jsonl" \
    "$edge_workspace/autonomous/recoveries.jsonl" \
    "$edge_workspace/autonomous/authorship_corrections.jsonl" \
    "$edge_workspace/autonomous/thread_state.json" \
    "$edge_workspace/autonomous/thread_state.jsonl" \
    "$edge_workspace/web/receipts.jsonl" \
    "$edge_workspace/introspection/receipts.jsonl" \
    "$edge_workspace/perception/latest.json" \
    "$edge_workspace/perception/observations.jsonl" \
    "$edge_workspace/studies/registry.json" \
    "$edge_workspace/studies/receipts.jsonl" \
    "$edge_workspace/research/duplication_notices.jsonl" \
    "$edge_workspace/peer/receipts.jsonl" \
    "$edge_workspace/self/profile.json" \
    "$edge_workspace/runtime/spectral_state.json" \
    "$edge_workspace/spectral/rollups.jsonl" \
    "$edge_workspace/spectral/recent_rollups.jsonl" \
    "$edge_workspace/spectral/receipts.jsonl" \
    "$edge_workspace/tuning/state.json" \
    "$edge_workspace/tuning/receipts.jsonl" \
    "$edge_workspace/tuning/signing.key" \
    "$edge_workspace/tuning/signing.pub"; do
    if [[ -e "$activity_ledger" ]]; then
        run chmod 0600 "$activity_ledger"
    fi
done
if [[ -d "$edge_workspace/tuning/evidence" ]]; then
    while IFS= read -r -d '' evidence_file; do
        run chmod 0600 "$evidence_file"
    done < <(find "$edge_workspace/tuning/evidence" -maxdepth 1 -type f -print0)
fi

if (( start_service == 1 )); then
    run systemctl --user enable \
        astrid-model-warmup.service \
        astrid-edge-runtime.service \
        astrid-edge-hindsight.timer
    run systemctl --user restart astrid-model-warmup.service
    run systemctl --user restart astrid-edge-runtime.service
    run systemctl --user start astrid-edge-hindsight.timer
fi

printf '\nInstalled CPU edge runtime at %s\n' "$astrid_home/bin/astrid-edge-runtime"
printf 'Selected install layout: %s\n' "$layout"
printf 'Reservoir tuning authority mode: %s\n' "$tuning_mode"
printf 'Installed appliance profile %s at %s\n' \
    "$profile_source" "$profile_dir/edge-appliance.env"
printf 'Installed matching model-context identity at %s\n' \
    "$capsule_env_dir/astrid-capsule-edge-context.env.json"
printf 'Installed owner-only hindsight viewer at %s\n' "$HOME/astrid-hindsight"
if (( start_service == 0 )); then
    printf 'Start it with: systemctl --user enable --now astrid-edge-runtime.service\n'
fi
