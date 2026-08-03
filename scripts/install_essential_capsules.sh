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

validate_capsule_managed_directories() {
    local allowed_symlink=""
    local directory
    if [[ "$layout" == "icp-ssd" ]]; then
        allowed_symlink="$HOME/.astrid-icp"
    fi
    for directory in \
        "$astrid_state_home" \
        "$astrid_state_home/.install-transactions" \
        "$astrid_state_home/home" \
        "$astrid_state_home/home/default" \
        "$astrid_state_home/home/default/.local" \
        "$capsule_root" \
        "$astrid_state_home/home/default/.config" \
        "$capsule_env_dir" \
        "$astrid_state_home/etc" \
        "$astrid_state_home/etc/install-manifests"; do
        validate_managed_directory_chain "$HOME" "$allowed_symlink" "$directory"
    done
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

capsule_transaction_id=""
capsule_transaction_root=""
capsule_snapshot_root=""
capsule_preflight_home=""
capsule_verified_archive_dir=""
capsule_root="$astrid_state_home/home/default/.local/capsules"
capsule_env_dir="$astrid_state_home/home/default/.config/env"
capsule_service_restarted=0
capsule_service_state_snapshotted=0
capsule_service_was_active=0
capsule_live_mutation_started=0
capsule_manifest_switched=0
capsule_manifest_had_existing=0
capsule_manifest_destination="$astrid_state_home/etc/install-manifests/essential-capsules.current"
declare -a capsule_had_existing=()
declare -a capsule_env_had_existing=()

capsule_snapshot_service_state() {
    if (( restart_service == 0 )); then
        # `astrid capsule install` only replaces files on disk. Without
        # --restart this installer sends no reload request and makes no user
        # service lifecycle transition, so rollback leaves the running
        # generation untouched.
        return
    fi
    if ! command -v systemctl >/dev/null 2>&1; then
        printf 'error: --restart requires systemd\n' >&2
        return 1
    fi
    if (( dry_run == 1 )); then
        printf '+ snapshot prior active state for astrid.service\n'
        return
    fi
    if systemctl --user is-active --quiet astrid.service >/dev/null 2>&1; then
        capsule_service_was_active=1
    else
        capsule_service_was_active=0
    fi
    capsule_service_state_snapshotted=1
}

capsule_restore_service_state() {
    if (( capsule_service_state_snapshotted != 1 )); then
        printf 'error: cannot restore astrid.service without a prior state snapshot\n' >&2
        return 1
    fi
    if (( capsule_service_was_active == 1 )); then
        # Reload the restored capsule generation into a service that was
        # already running before the transaction.
        systemctl --user restart astrid.service >/dev/null 2>&1
    else
        # A failed --restart must never turn a previously stopped service on.
        systemctl --user stop astrid.service >/dev/null 2>&1
    fi
}

remove_managed_capsule_path() {
    local capsule="$1"
    local target="$2"
    local expected="$capsule_root/$capsule"

    if [[ "$target" != "$expected" && "$target" != "$expected.bak" ]]; then
        printf 'error: refusing rollback outside managed capsule path: %s\n' "$target" >&2
        return 1
    fi
    if [[ -L "$target" || -f "$target" ]]; then
        rm -f -- "$target"
    elif [[ -d "$target" ]]; then
        rm -R -- "$target"
    fi
}

capsule_transaction_abort() {
    local exit_code="${1:-1}"
    local reason="${2:-installer failure}"
    local rollback_failed=0
    local index capsule target snapshot env_target env_snapshot

    trap - EXIT HUP INT TERM
    if (( dry_run == 0 )) && [[ -n "$capsule_transaction_root" ]]; then
        printf 'Capsule transaction %s interrupted (%s); restoring prior capsule set...\n' \
            "$capsule_transaction_id" "$reason" >&2
        if (( capsule_live_mutation_started == 1 )); then
            for (( index = ${#capsules[@]} - 1; index >= 0; index-- )); do
                capsule="${capsules[$index]}"
                target="$capsule_root/$capsule"
                snapshot="$capsule_snapshot_root/capsules/$capsule"
                env_target="$capsule_env_dir/$capsule.env.json"
                env_snapshot="$capsule_snapshot_root/env/$capsule.env.json"
                if ! remove_managed_capsule_path "$capsule" "$target"; then
                    rollback_failed=1
                    continue
                fi
                if ! remove_managed_capsule_path "$capsule" "$target.bak"; then
                    rollback_failed=1
                    continue
                fi
                if [[ "${capsule_had_existing[$index]:-0}" == "1" ]]; then
                    if ! mv -f -- "$snapshot" "$target"; then
                        rollback_failed=1
                    fi
                fi
                if [[ -e "$env_target" || -L "$env_target" ]]; then
                    rm -f -- "$env_target" || rollback_failed=1
                fi
                if [[ "${capsule_env_had_existing[$index]:-0}" == "1" ]]; then
                    if ! mv -f -- "$env_snapshot" "$env_target"; then
                        rollback_failed=1
                    fi
                fi
            done
        fi
        if (( capsule_service_restarted == 1 )); then
            capsule_restore_service_state || rollback_failed=1
        fi
        rm -f -- "$capsule_manifest_destination.new-$capsule_transaction_id" \
            2>/dev/null || true
        if (( capsule_manifest_switched == 1 )); then
            rm -f -- "$capsule_manifest_destination" || rollback_failed=1
            if (( capsule_manifest_had_existing == 1 )); then
                mv -f -- "$capsule_snapshot_root/essential-capsules.current" \
                    "$capsule_manifest_destination" || rollback_failed=1
            fi
        fi
        if (( rollback_failed == 0 )); then
            rm -R -- "$capsule_transaction_root"
        else
            printf 'error: capsule rollback was incomplete; recovery material remains at %s\n' \
                "$capsule_transaction_root" >&2
        fi
    fi
    exit "$exit_code"
}

capsule_transaction_begin() {
    local transaction_parent="$astrid_state_home/.install-transactions"
    local index capsule target env_target pending

    if (( dry_run == 1 )); then
        capsule_transaction_id="essential-capsules-dry-run"
        printf '+ acquire shared CPU-edge install lock under %q\n' "$transaction_parent"
        printf '+ preflight all capsule archives in an isolated state home\n'
        printf '+ snapshot all ten live capsule targets before lifecycle installation\n'
        return
    fi
    if ! command -v flock >/dev/null 2>&1; then
        printf 'error: flock is required for transactional CPU-edge installation\n' >&2
        exit 1
    fi
    if ! command -v sha256sum >/dev/null 2>&1; then
        printf 'error: sha256sum is required for capsule-generation verification\n' >&2
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
    capsule_transaction_id="essential-capsules-$(date -u +%Y%m%dT%H%M%SZ)-$$"
    capsule_transaction_root="$(mktemp -d \
        "$transaction_parent/$capsule_transaction_id.XXXXXXXX")"
    chmod 0700 "$capsule_transaction_root"
    capsule_snapshot_root="$capsule_transaction_root/prior"
    capsule_preflight_home="$capsule_transaction_root/preflight-home"
    capsule_verified_archive_dir="$capsule_transaction_root/verified-archives"
    install -d -m 0700 \
        "$capsule_snapshot_root/capsules" \
        "$capsule_snapshot_root/env" \
        "$capsule_preflight_home" \
        "$capsule_verified_archive_dir"
    install -d -m 0700 "$capsule_root" "$capsule_env_dir"
    trap 'capsule_transaction_abort "$?" "unexpected exit"' EXIT
    trap 'capsule_transaction_abort 129 "hangup"' HUP
    trap 'capsule_transaction_abort 130 "interrupt"' INT
    trap 'capsule_transaction_abort 143 "termination"' TERM
    if [[ -L "$capsule_manifest_destination" \
        || ( -e "$capsule_manifest_destination" && ! -f "$capsule_manifest_destination" ) ]]; then
        printf 'error: capsule-generation manifest must be a regular file: %s\n' \
            "$capsule_manifest_destination" >&2
        return 1
    fi
    if [[ -f "$capsule_manifest_destination" ]]; then
        cp -p -- "$capsule_manifest_destination" \
            "$capsule_snapshot_root/essential-capsules.current"
        capsule_manifest_had_existing=1
    fi
    for (( index = 0; index < ${#capsules[@]}; index++ )); do
        capsule="${capsules[$index]}"
        target="$capsule_root/$capsule"
        env_target="$capsule_env_dir/$capsule.env.json"
        if [[ -e "$target.bak" || -L "$target.bak" ]]; then
            printf 'error: prior capsule recovery directory requires operator review: %s\n' \
                "$target.bak" >&2
            return 1
        fi
        if [[ -L "$target" || ( -e "$target" && ! -d "$target" ) ]]; then
            printf 'error: managed capsule target must be a real directory: %s\n' "$target" >&2
            return 1
        fi
        if [[ -d "$target" ]]; then
            cp -a -- "$target" "$capsule_snapshot_root/capsules/$capsule"
            capsule_had_existing+=("1")
        else
            capsule_had_existing+=("0")
        fi
        if [[ -L "$env_target" || ( -e "$env_target" && ! -f "$env_target" ) ]]; then
            printf 'error: capsule environment target must be a regular file: %s\n' \
                "$env_target" >&2
            return 1
        fi
        if [[ -f "$env_target" ]]; then
            cp -p -- "$env_target" "$capsule_snapshot_root/env/$capsule.env.json"
            capsule_env_had_existing+=("1")
        else
            capsule_env_had_existing+=("0")
        fi
    done
}

capsule_preflight_archives() {
    local capsule archive verified_archive installed

    if (( dry_run == 1 )); then
        for capsule in "${capsules[@]}"; do
            printf '+ isolated preflight install and verify %q\n' \
                "$output_dir/$capsule.capsule"
        done
        return
    fi
    for capsule in "${capsules[@]}"; do
        archive="$output_dir/$capsule.capsule"
        verified_archive="$capsule_verified_archive_dir/$capsule.capsule"
        install -m 0600 "$archive" "$verified_archive"
        if ! cmp -s -- "$archive" "$verified_archive"; then
            printf 'error: staged capsule archive differs from %s\n' "$archive" >&2
            return 1
        fi
        env ASTRID_HOME="$capsule_preflight_home" \
            "$astrid_bin" capsule install "$verified_archive" </dev/null
        installed="$capsule_preflight_home/home/default/.local/capsules/$capsule"
        if [[ ! -f "$installed/Capsule.toml" || ! -f "$installed/meta.json" ]]; then
            printf 'error: isolated preflight did not install expected capsule %s\n' \
                "$capsule" >&2
            return 1
        fi
        if [[ -n "$(find "$installed" -type l -print -quit)" ]]; then
            printf 'error: isolated preflight produced a symlink in %s\n' "$capsule" >&2
            return 1
        fi
    done
}

capsule_transaction_commit() {
    local manifest_dir="$astrid_state_home/etc/install-manifests"
    local manifest_stage="$capsule_transaction_root/essential-capsules.manifest"
    local manifest_destination="$capsule_manifest_destination"
    local capsule archive checksum

    if (( dry_run == 1 )); then
        printf '+ write owner-only capsule-generation manifest %q\n' "$manifest_destination"
        printf '+ committed verified capsule generation %q\n' "$capsule_transaction_id"
        return
    fi
    install -d -m 0700 "$manifest_dir"
    {
        printf 'schema=astrid_cpu_edge_capsule_generation_v1\n'
        printf 'generation=%s\n' "$capsule_transaction_id"
        printf 'created_at=%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
        for capsule in "${capsules[@]}"; do
            archive="$capsule_verified_archive_dir/$capsule.capsule"
            checksum="$(sha256sum "$archive")"
            printf 'capsule=%s archive_sha256=%s\n' "$capsule" "${checksum%% *}"
        done
    } > "$manifest_stage"
    chmod 0600 "$manifest_stage"
    install -m 0600 "$manifest_stage" "$manifest_destination.new-$capsule_transaction_id"
    if ! cmp -s -- "$manifest_stage" "$manifest_destination.new-$capsule_transaction_id"; then
        printf 'error: capsule-generation manifest verification failed\n' >&2
        return 1
    fi
    mv -f -- "$manifest_destination.new-$capsule_transaction_id" "$manifest_destination"
    capsule_manifest_switched=1
    local committed_root="$capsule_transaction_root"
    capsule_transaction_root=""
    trap - EXIT HUP INT TERM
    rm -R -- "$committed_root"
    printf '+ committed verified capsule generation %q\n' "$capsule_transaction_id"
}

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
    if [[ ! -f "$archive" || -L "$archive" || ! -r "$archive" ]]; then
        if (( dry_run == 0 )) || [[ -n "$prebuilt_capsule_dir" ]]; then
            printf 'error: expected capsule archive is not a readable regular file: %s\n' \
                "$archive" >&2
            exit 1
        fi
    fi

done

validate_capsule_managed_directories
capsule_transaction_begin
capsule_snapshot_service_state
capsule_preflight_archives
if (( dry_run == 0 )); then
    capsule_live_mutation_started=1
fi

for capsule in "${capsules[@]}"; do
    if (( dry_run == 0 )); then
        archive="$capsule_verified_archive_dir/$capsule.capsule"
    else
        archive="$output_dir/$capsule.capsule"
    fi
    printf 'Installing %s from verified preflight...\n' "$capsule"
    # EOF accepts manifest defaults for the two non-secret cwd_dir prompts.
    if (( dry_run == 0 )); then
        env ASTRID_HOME="$astrid_state_home" \
            "$astrid_bin" capsule install "$archive" </dev/null
    else
        printf '+ ASTRID_HOME=%q %q capsule install %q </dev/null\n' \
            "$astrid_state_home" "$astrid_bin" "$archive"
    fi
done

if (( restart_service == 1 )); then
    if (( dry_run == 1 )); then
        printf '+ systemctl --user restart astrid.service\n'
        printf '+ verify all ten essential capsules and exactly 20 total capsules loaded\n'
    else
        capsule_service_restarted=1
        systemctl --user restart astrid.service
        capsules_verified=0
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
                    capsules_verified=1
                    break
                fi
            fi
            sleep 1
        done

        if (( capsules_verified == 0 )); then
            printf 'error: astrid.service did not report all ten essential capsules and exactly 20 total loaded capsules\n' >&2
            systemctl --user status astrid.service --no-pager -l >&2 || true
            exit 1
        fi
    fi
fi

capsule_transaction_commit
printf 'Installed %s version-matched bootstrap capsules.\n' "${#capsules[@]}"
