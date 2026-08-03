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
    ollama_unit_source="$project_root/packaging/systemd/ollama-cpu.service"
    ssd_guard_source=""
    astrid_home="$HOME/.astrid"
fi
required_files=(
    "$unit_source"
    "$warmup_unit_source"
    "$warmup_script_source"
    "$ollama_unit_source"
)
if [[ "$layout" == "icp-ssd" ]]; then
    required_files+=("$ssd_guard_source")
else
    required_files+=("$local_ollama_source")
fi
for required_file in "${required_files[@]}"; do
    if [[ ! -f "$required_file" ]]; then
        printf 'error: required install asset not found: %s\n' "$required_file" >&2
        exit 1
    fi
done

if [[ "$layout" == "standard" ]]; then
    ollama_executable="$HOME/.local/bin/ollama"
    if (( start_service == 1 && dry_run == 0 )) && [[ ! -x "$ollama_executable" ]]; then
        printf 'error: bundled ollama-cpu.service requires executable %s\n' \
            "$ollama_executable" >&2
        exit 1
    elif (( dry_run == 1 )) || [[ ! -x "$ollama_executable" ]]; then
        printf '+ require executable %q for ollama-cpu.service\n' "$ollama_executable"
    fi
fi

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

validate_core_managed_directories() {
    local allowed_symlink=""
    local config_home="${XDG_CONFIG_HOME:-$HOME/.config}"
    local directory
    if [[ "$layout" == "icp-ssd" ]]; then
        allowed_symlink="$HOME/.astrid-icp"
    fi
    for directory in \
        "$astrid_home" \
        "$install_bin" \
        "$astrid_home/etc" \
        "$astrid_home/etc/install-manifests" \
        "$astrid_home/.install-transactions"; do
        validate_managed_directory_chain "$HOME" "$allowed_symlink" "$directory"
    done
    if [[ "$layout" == "icp-ssd" ]]; then
        for directory in \
            "$HOME/.astrid-icp/workspace" \
            "$HOME/.astrid-icp/tmp" \
            "$HOME/.astrid-icp/ollama" \
            "$HOME/.astrid-icp/ollama/models"; do
            validate_managed_directory_chain "$HOME" "$allowed_symlink" "$directory"
        done
    else
        validate_managed_directory_chain \
            "$HOME" "" "$HOME/.local/share/ollama/models"
    fi
    for directory in \
        "$config_home" \
        "$unit_dir" \
        "$unit_dropin_dir" \
        "$unit_dir/ollama-cpu.service.d" \
        "$unit_dir/astrid-model-warmup.service.d" \
        "$unit_dir/astrid-edge-runtime.service.d"; do
        validate_managed_directory_chain \
            "$(dirname -- "$config_home")" "" "$directory"
    done
}

# Install every managed file as one rollback-capable generation.  Files are
# first copied beside their destinations and byte/mode verified.  Only after
# the whole payload is ready do we switch live paths.  The shared lock also
# prevents the core and edge-runtime installers from interleaving generations.
transaction_component="headless-linux"
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
    ollama-cpu.service
    astrid-model-warmup.service
    astrid.service
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

validate_core_managed_directories
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
    run install -d -m 0700 "$HOME/.local/share/ollama/models"
fi
transaction_begin
for binary in astrid astrid-daemon astrid-build; do
    transaction_stage_file "$binary_dir/$binary" 0755 "$install_bin/$binary"
done
transaction_stage_file "$unit_source" 0644 "$unit_dest"
transaction_stage_file "$warmup_unit_source" 0644 \
    "$unit_dir/astrid-model-warmup.service"
transaction_stage_file "$ollama_unit_source" 0644 "$unit_dir/ollama-cpu.service"
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
else
    transaction_stage_file "$local_ollama_source" 0644 \
        "$unit_dropin_dir/local-ollama.conf"
fi
transaction_stage_file "$warmup_script_source" 0755 "$install_bin/warm-ollama-model"
transaction_stage_manifest
transaction_switch_and_verify
if (( dry_run == 0 )); then
    transaction_systemd_touched=1
fi
run systemctl --user daemon-reload

if (( start_service == 1 )); then
    if [[ "$layout" == "icp-ssd" ]]; then
        run systemctl --user enable \
            ollama-cpu.service \
            astrid-model-warmup.service \
            astrid.service
    else
        run systemctl --user enable \
            ollama-cpu.service \
            astrid-model-warmup.service \
            astrid.service
    fi
    run systemctl --user restart ollama-cpu.service
    run systemctl --user restart astrid-model-warmup.service
    run systemctl --user restart astrid.service
fi
transaction_commit

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
