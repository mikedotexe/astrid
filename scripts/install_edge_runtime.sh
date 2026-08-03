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
    && ! grep -Eq '^[[:space:]]*ASTRID_EDGE_RESERVOIR_TUNING_PROFILE_PERMITS=("true"|true)[[:space:]]*$' \
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

validate_managed_directory_chain() {
    local base="${1%/}"
    local allowed_symlink="$2"
    local destination="$3"
    local current remaining component

    case "$destination/" in
        "$base/"*) ;;
        *)
            printf 'error: managed directory escapes its validation root: %s\n' \
                "$destination" >&2
            return 1
            ;;
    esac
    remaining="${destination#"$base"}"
    remaining="${remaining#/}"
    current="$base"
    while [[ -n "$remaining" ]]; do
        if [[ "$remaining" == */* ]]; then
            component="${remaining%%/*}"
            remaining="${remaining#*/}"
        else
            component="$remaining"
            remaining=""
        fi
        if [[ -z "$component" || "$component" == "." || "$component" == ".." ]]; then
            printf 'error: invalid managed directory component in %s\n' "$destination" >&2
            return 1
        fi
        current="$current/$component"
        if [[ -L "$current" && "$current" != "$allowed_symlink" ]]; then
            printf 'error: managed directory component must not be a symlink: %s\n' \
                "$current" >&2
            return 1
        fi
        if [[ -e "$current" && ! -d "$current" ]]; then
            printf 'error: managed directory component is not a directory: %s\n' \
                "$current" >&2
            return 1
        fi
    done
}

validate_edge_managed_directories() {
    local allowed_symlink=""
    local config_home="${XDG_CONFIG_HOME:-$HOME/.config}"
    local directory
    if [[ "$layout" == "icp-ssd" ]]; then
        allowed_symlink="$HOME/.astrid-icp"
    fi
    for directory in \
        "$astrid_home" \
        "$astrid_home/bin" \
        "$astrid_home/etc" \
        "$astrid_home/etc/install-manifests" \
        "$astrid_home/.install-transactions" \
        "$astrid_home/home" \
        "$astrid_home/home/default" \
        "$edge_workspace" \
        "$edge_workspace/spectral" \
        "$edge_workspace/tuning" \
        "$edge_workspace/tuning/evidence" \
        "$astrid_home/operator" \
        "$astrid_home/operator/hindsight" \
        "$astrid_home/home/default/.config" \
        "$capsule_env_dir"; do
        validate_managed_directory_chain "$HOME" "$allowed_symlink" "$directory"
    done
    for directory in \
        "$config_home" \
        "$unit_dir" \
        "$unit_dir/astrid-edge-runtime.service.d" \
        "$unit_dir/astrid.service.d" \
        "$unit_dir/ollama-cpu.service.d" \
        "$unit_dir/astrid-model-warmup.service.d" \
        "$profile_dir"; do
        validate_managed_directory_chain \
            "$(dirname -- "$config_home")" "" "$directory"
    done
}

# Stage the complete managed payload before switching any live path.  Core and
# edge installers share this lock, so their generations cannot interleave.
transaction_component="edge-runtime"
transaction_id=""
transaction_root=""
transaction_attempted=0
transaction_systemd_touched=0
declare -a transaction_sources=()
declare -a transaction_modes=()
declare -a transaction_destinations=()
declare -a transaction_stages=()
declare -a transaction_backups=()
declare -a transaction_had_existing=()
declare -a transaction_managed_units=(
    astrid-model-warmup.service
    astrid-edge-runtime.service
    astrid-edge-hindsight.service
    astrid-edge-hindsight.timer
)
declare -a transaction_unit_was_enabled=()
declare -a transaction_unit_was_active=()

transaction_snapshot_systemd_state() {
    local unit

    if (( dry_run == 1 )); then
        printf '+ snapshot prior enabled/active state for managed user units\n'
        return
    fi
    for unit in "${transaction_managed_units[@]}"; do
        if systemctl --user is-enabled --quiet "$unit" >/dev/null 2>&1; then
            transaction_unit_was_enabled+=(1)
        else
            transaction_unit_was_enabled+=(0)
        fi
        if systemctl --user is-active --quiet "$unit" >/dev/null 2>&1; then
            transaction_unit_was_active+=(1)
        else
            transaction_unit_was_active+=(0)
        fi
    done
}

transaction_prepare_systemd_rollback() {
    local index unit
    local restore_failed=0

    # Remove enablement links while the newly installed unit files still
    # exist. This also handles a failed first install whose restored state has
    # no unit file at all.
    for (( index = 0; index < ${#transaction_managed_units[@]}; index++ )); do
        if [[ "${transaction_unit_was_enabled[$index]}" == "0" ]]; then
            unit="${transaction_managed_units[$index]}"
            systemctl --user disable "$unit" >/dev/null 2>&1 || restore_failed=1
        fi
    done
    return "$restore_failed"
}

transaction_restore_systemd_state() {
    local index unit
    local restore_failed=0

    systemctl --user daemon-reload >/dev/null 2>&1 || restore_failed=1
    for (( index = 0; index < ${#transaction_managed_units[@]}; index++ )); do
        unit="${transaction_managed_units[$index]}"
        if [[ "${transaction_unit_was_enabled[$index]}" == "1" ]]; then
            systemctl --user enable "$unit" >/dev/null 2>&1 || restore_failed=1
        fi
    done
    for (( index = 0; index < ${#transaction_managed_units[@]}; index++ )); do
        unit="${transaction_managed_units[$index]}"
        if [[ "${transaction_unit_was_active[$index]}" == "1" ]]; then
            # A failed install may already have restarted the new generation.
            # Restart after restoring files so the prior generation is live.
            systemctl --user restart "$unit" >/dev/null 2>&1 || restore_failed=1
        else
            systemctl --user stop "$unit" >/dev/null 2>&1 || restore_failed=1
        fi
    done
    return "$restore_failed"
}

transaction_abort() {
    local exit_code="${1:-1}"
    local reason="${2:-installer failure}"
    local rollback_failed=0
    local index destination stage backup had_existing

    trap - EXIT HUP INT TERM
    if (( dry_run == 0 )) && [[ -n "$transaction_root" ]]; then
        printf 'Install transaction %s interrupted (%s); restoring prior generation...\n' \
            "$transaction_id" "$reason" >&2
        if (( transaction_systemd_touched == 1 )); then
            transaction_prepare_systemd_rollback || rollback_failed=1
        fi
        for (( index = transaction_attempted - 1; index >= 0; index-- )); do
            destination="${transaction_destinations[$index]}"
            stage="${transaction_stages[$index]}"
            backup="${transaction_backups[$index]}"
            had_existing="${transaction_had_existing[$index]}"
            if [[ -e "$backup" || -L "$backup" ]]; then
                if [[ -e "$destination" || -L "$destination" ]]; then
                    rm -f -- "$destination" || rollback_failed=1
                fi
                mv -f -- "$backup" "$destination" || rollback_failed=1
            elif [[ "$had_existing" == "0" && ! -e "$stage" && ! -L "$stage" ]] \
                && [[ -e "$destination" || -L "$destination" ]]; then
                rm -f -- "$destination" || rollback_failed=1
            fi
        done
        for stage in "${transaction_stages[@]}"; do
            if [[ -e "$stage" || -L "$stage" ]]; then
                rm -f -- "$stage" || rollback_failed=1
            fi
        done
        if (( transaction_systemd_touched == 1 )); then
            transaction_restore_systemd_state || rollback_failed=1
        fi
        if (( rollback_failed == 0 )); then
            rm -f -- "$transaction_root/$transaction_component.manifest" 2>/dev/null || true
            rmdir -- "$transaction_root" 2>/dev/null || true
        else
            printf 'error: rollback was incomplete; recovery material remains at %s\n' \
                "$transaction_root" >&2
        fi
    fi
    exit "$exit_code"
}

transaction_begin() {
    local transaction_parent="$astrid_home/.install-transactions"
    local pending

    if (( dry_run == 1 )); then
        transaction_id="${transaction_component}-dry-run"
        printf '+ acquire shared CPU-edge install lock under %q\n' "$transaction_parent"
        printf '+ stage and verify generation %q before switching any live file\n' \
            "$transaction_id"
        return
    fi
    if ! command -v flock >/dev/null 2>&1; then
        printf 'error: flock is required for transactional CPU-edge installation\n' >&2
        exit 1
    fi
    if ! command -v sha256sum >/dev/null 2>&1; then
        printf 'error: sha256sum is required for install-generation verification\n' >&2
        exit 1
    fi
    install -d -m 0700 "$transaction_parent"
    exec 9>"$transaction_parent/install.lock"
    chmod 0600 "$transaction_parent/install.lock"
    if ! flock -n 9; then
        printf 'error: another CPU-edge installer is already active\n' >&2
        exit 1
    fi
    for pending in \
        "$transaction_parent"/headless-linux-* \
        "$transaction_parent"/edge-runtime-* \
        "$transaction_parent"/essential-capsules-*; do
        if [[ -d "$pending" ]]; then
            printf 'error: pending CPU-edge transaction requires operator recovery: %s\n' \
                "$pending" >&2
            exit 1
        fi
    done
    transaction_id="${transaction_component}-$(date -u +%Y%m%dT%H%M%SZ)-$$"
    transaction_root="$(mktemp -d "$transaction_parent/$transaction_id.XXXXXXXX")"
    chmod 0700 "$transaction_root"
    trap 'transaction_abort "$?" "unexpected exit"' EXIT
    trap 'transaction_abort 129 "hangup"' HUP
    trap 'transaction_abort 130 "interrupt"' INT
    trap 'transaction_abort 143 "termination"' TERM
    transaction_snapshot_systemd_state
}

transaction_stage_file() {
    local source="$1"
    local mode="$2"
    local destination="$3"
    local stage="$destination.astrid-stage.$transaction_id"
    local backup="$destination.astrid-backup.$transaction_id"
    local actual_mode expected_mode had_existing=0

    printf '+ stage %q -> %q mode %s; verify bytes and mode\n' \
        "$source" "$destination" "$mode"
    transaction_sources+=("$source")
    transaction_modes+=("$mode")
    transaction_destinations+=("$destination")
    transaction_stages+=("$stage")
    transaction_backups+=("$backup")
    if [[ -e "$destination" || -L "$destination" ]]; then
        had_existing=1
    fi
    transaction_had_existing+=("$had_existing")
    if (( dry_run == 1 )); then
        return
    fi
    if [[ -d "$destination" && ! -L "$destination" ]]; then
        printf 'error: managed install destination is a directory: %s\n' "$destination" >&2
        return 1
    fi
    if [[ -e "$stage" || -L "$stage" || -e "$backup" || -L "$backup" ]]; then
        printf 'error: transaction path collision beside %s\n' "$destination" >&2
        return 1
    fi
    install -m "$mode" "$source" "$stage"
    if ! cmp -s -- "$source" "$stage"; then
        printf 'error: staged bytes differ for %s\n' "$destination" >&2
        return 1
    fi
    actual_mode="$(stat -c '%a' "$stage" 2>/dev/null || stat -f '%Lp' "$stage")"
    expected_mode="${mode#0}"
    if [[ "$actual_mode" != "$expected_mode" ]]; then
        printf 'error: staged mode %s does not match %s for %s\n' \
            "$actual_mode" "$expected_mode" "$destination" >&2
        return 1
    fi
}

transaction_stage_manifest() {
    local manifest_dir="$astrid_home/etc/install-manifests"
    local manifest_source="$transaction_root/$transaction_component.manifest"
    local index checksum

    if (( dry_run == 1 )); then
        printf '+ write owner-only generation manifest %q\n' \
            "$manifest_dir/$transaction_component.current"
        transaction_stage_file /dev/null 0600 \
            "$manifest_dir/$transaction_component.current"
        return
    fi
    install -d -m 0700 "$manifest_dir"
    {
        printf 'schema=astrid_cpu_edge_install_generation_v1\n'
        printf 'component=%s\n' "$transaction_component"
        printf 'generation=%s\n' "$transaction_id"
        printf 'created_at=%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
        for (( index = 0; index < ${#transaction_sources[@]}; index++ )); do
            checksum="$(sha256sum "${transaction_sources[$index]}")"
            printf 'file_sha256=%s destination=%q\n' \
                "${checksum%% *}" "${transaction_destinations[$index]}"
        done
    } > "$manifest_source"
    chmod 0600 "$manifest_source"
    transaction_stage_file "$manifest_source" 0600 \
        "$manifest_dir/$transaction_component.current"
}

transaction_switch_and_verify() {
    local index source destination stage backup had_existing actual_mode expected_mode

    printf '+ switch %s verified files using atomic renames with generation rollback %q\n' \
        "${#transaction_destinations[@]}" "$transaction_id"
    if (( dry_run == 1 )); then
        return
    fi
    for (( index = 0; index < ${#transaction_destinations[@]}; index++ )); do
        destination="${transaction_destinations[$index]}"
        stage="${transaction_stages[$index]}"
        backup="${transaction_backups[$index]}"
        had_existing="${transaction_had_existing[$index]}"
        transaction_attempted=$((index + 1))
        if [[ "$had_existing" == "1" ]]; then
            mv -f -- "$destination" "$backup"
        fi
        mv -f -- "$stage" "$destination"
    done
    for (( index = 0; index < ${#transaction_destinations[@]}; index++ )); do
        source="${transaction_sources[$index]}"
        destination="${transaction_destinations[$index]}"
        if ! cmp -s -- "$source" "$destination"; then
            printf 'error: live verification failed for %s\n' "$destination" >&2
            return 1
        fi
        actual_mode="$(stat -c '%a' "$destination" 2>/dev/null || stat -f '%Lp' "$destination")"
        expected_mode="${transaction_modes[$index]}"
        expected_mode="${expected_mode#0}"
        if [[ "$actual_mode" != "$expected_mode" ]]; then
            printf 'error: live mode verification failed for %s\n' "$destination" >&2
            return 1
        fi
    done
}

transaction_commit() {
    local backup committed_root

    if (( dry_run == 0 )); then
        committed_root="$transaction_root"
        transaction_root=""
        trap - EXIT HUP INT TERM
        for backup in "${transaction_backups[@]}"; do
            if [[ -e "$backup" || -L "$backup" ]]; then
                rm -f -- "$backup" || \
                    printf 'warning: retained prior-generation backup %s\n' "$backup" >&2
            fi
        done
        rm -f -- "$committed_root/$transaction_component.manifest" 2>/dev/null || true
        rmdir -- "$committed_root" 2>/dev/null || true
    fi
    printf '+ committed verified install generation %q\n' "$transaction_id"
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
validate_edge_managed_directories
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
transaction_begin
transaction_stage_file "$binary" 0755 "$astrid_home/bin/astrid-edge-runtime"
transaction_stage_file "$warmup_script_source" 0755 "$astrid_home/bin/warm-ollama-model"
transaction_stage_file "$report_source" 0755 "$astrid_home/bin/report-edge-appliance"
transaction_stage_file "$activity_report_source" 0755 \
    "$astrid_home/bin/report-edge-activity"
transaction_stage_file "$activity_report_source" 0755 \
    "$astrid_home/bin/report_edge_activity.py"
transaction_stage_file "$hindsight_source" 0755 "$astrid_home/bin/edge-hindsight"
transaction_stage_file "$hindsight_source" 0755 "$HOME/astrid-hindsight"
transaction_stage_file "$transport_migration_source" 0755 \
    "$astrid_home/bin/migrate-edge-transport-sentinels"
transaction_stage_file "$operator_harness_migration_source" 0755 \
    "$astrid_home/bin/migrate-edge-operator-harness-isolation"
transaction_stage_file "$interrupted_action_reconciliation_source" 0755 \
    "$astrid_home/bin/reconcile-edge-interrupted-actions"
transaction_stage_file "$dashboard_source" 0755 "$HOME/astrid-at-a-glance"
transaction_stage_file "$profile_source" 0600 "$profile_dir/edge-appliance.env"
if [[ "$tuning_mode" == "enabled" ]]; then
    tuning_authority_env_source="$tuning_enabled_env_source"
else
    tuning_authority_env_source="$observation_only_env_source"
fi
transaction_stage_file \
    "$tuning_authority_env_source" \
    0600 \
    "$profile_dir/edge-tuning-authority.env"
transaction_stage_file "$edge_context_source" 0600 \
    "$capsule_env_dir/astrid-capsule-edge-context.env.json"
transaction_stage_file "$unit_source" 0644 "$unit_dir/astrid-edge-runtime.service"
transaction_stage_file "$warmup_unit_source" 0644 \
    "$unit_dir/astrid-model-warmup.service"
transaction_stage_file "$hindsight_unit_source" 0644 \
    "$unit_dir/astrid-edge-hindsight.service"
transaction_stage_file "$hindsight_timer_source" 0644 \
    "$unit_dir/astrid-edge-hindsight.timer"
transaction_stage_file \
    "$tuning_authority_dropin_source" \
    0644 \
    "$unit_dir/astrid-edge-runtime.service.d/10-tuning-authority.conf"
if [[ "$layout" == "icp-ssd" ]]; then
    for service in \
        astrid.service \
        ollama-cpu.service \
        astrid-model-warmup.service \
        astrid-edge-runtime.service; do
        transaction_stage_file \
            "$ssd_guard_source" \
            0644 \
            "$unit_dir/$service.d/ssd-required.conf"
    done
fi
transaction_stage_manifest

if [[ -d "$edge_workspace" ]]; then
    edge_workspace_real="$(CDPATH= cd -P -- "$edge_workspace" && pwd)"
else
    # A dry run prints directory creation without materializing it.
    edge_workspace_real="$edge_workspace"
fi
harden_owner_only_ledger() {
    local ledger="$1"
    local ledger_parent_real

    if [[ -L "$ledger" ]]; then
        printf 'error: refusing owner-mode normalization through ledger symlink: %s\n' \
            "$ledger" >&2
        return 1
    fi
    if [[ -e "$ledger" ]]; then
        ledger_parent_real="$(CDPATH= cd -P -- "$(dirname -- "$ledger")" && pwd)"
        case "$ledger_parent_real/" in
            "$edge_workspace_real/"*) ;;
            *)
                printf 'error: activity ledger resolves outside the private edge tree: %s\n' \
                    "$ledger" >&2
                return 1
                ;;
        esac
    fi
    if [[ -e "$ledger" && ! -f "$ledger" ]]; then
        printf 'error: activity ledger is not a regular file: %s\n' "$ledger" >&2
        return 1
    fi
    if [[ -f "$ledger" ]]; then
        run chmod 0600 "$ledger"
    fi
}

for activity_ledger in \
    "$edge_workspace/actions/dispatches.jsonl" \
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
    "$edge_workspace/spectral/recent_rollups.current.jsonl" \
    "$edge_workspace/spectral/recent_rollups.previous.jsonl" \
    "$edge_workspace/spectral/activity_receipts.current.jsonl" \
    "$edge_workspace/spectral/activity_receipts.previous.jsonl" \
    "$edge_workspace/spectral/receipts.jsonl" \
    "$edge_workspace/tuning/state.json" \
    "$edge_workspace/tuning/receipts.jsonl" \
    "$edge_workspace/tuning/signing.key" \
    "$edge_workspace/tuning/signing.pub"; do
    harden_owner_only_ledger "$activity_ledger"
done
if [[ -d "$edge_workspace/tuning/evidence" ]]; then
    while IFS= read -r -d '' evidence_file; do
        run chmod 0600 "$evidence_file"
    done < <(find "$edge_workspace/tuning/evidence" -maxdepth 1 -type f -print0)
fi

transaction_switch_and_verify
if (( dry_run == 0 )); then
    transaction_systemd_touched=1
fi
run systemctl --user daemon-reload

if (( start_service == 1 )); then
    run systemctl --user enable \
        astrid-model-warmup.service \
        astrid-edge-runtime.service \
        astrid-edge-hindsight.timer
    run systemctl --user restart astrid-model-warmup.service
    run systemctl --user restart astrid-edge-runtime.service
    run systemctl --user start astrid-edge-hindsight.timer
fi
transaction_commit

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
