#!/usr/bin/env bash

set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
project_root="$(CDPATH= cd -- "$script_dir/.." && pwd)"
guard="$project_root/packaging/systemd/wait-for-icp-ssd"
test_root="$(mktemp -d)"
test_root="$(CDPATH= cd -P -- "$test_root" && pwd -P)"
trap 'rm -rf -- "$test_root"' EXIT

mount_path="$test_root/media-data"
root_path="$mount_path/astrid"
home_path="$test_root/home"
link_path="$home_path/.astrid-icp"
state_path="$root_path/state"
state_bin_path="$state_path/bin"
state_run_path="$state_path/run"
state_var_path="$state_path/var"
state_home_path="$state_path/home"
state_default_path="$state_home_path/default"
edge_workspace_path="$state_default_path/edge"
workspace_path="$root_path/workspace"
tmp_path="$root_path/tmp"
ollama_path="$root_path/ollama"
ollama_runtime_path="$ollama_path/runtime"
ollama_runtime_bin_path="$ollama_runtime_path/bin"
ollama_models_path="$ollama_path/models"
mock_bin="$test_root/bin"
mock_findmnt="$mock_bin/findmnt"
counter_path="$test_root/findmnt-count"
expected_uuid="6b7d53e2-b6fc-4363-9add-d3111eb2ef7d"

install -d -m 0700 "$root_path" "$home_path" "$mock_bin"
ln -s "$root_path" "$link_path"

cat >"$mock_findmnt" <<'MOCK'
#!/usr/bin/env bash
set -euo pipefail

case "${MOCK_FINDMNT_MODE:?}" in
    success)
        printf '%s %s ext4 rw,nosuid,nodev,relatime\n' \
            "$MOCK_TARGET" "$MOCK_UUID"
        ;;
    delayed)
        count=0
        if [[ -f "$MOCK_COUNTER" ]]; then
            count="$(<"$MOCK_COUNTER")"
        fi
        count=$((count + 1))
        printf '%s\n' "$count" >"$MOCK_COUNTER"
        if (( count < 2 )); then
            exit 1
        fi
        printf '%s %s ext4 rw,nosuid,nodev,relatime\n' \
            "$MOCK_TARGET" "$MOCK_UUID"
        ;;
    disappears | replaced)
        count=0
        if [[ -f "$MOCK_COUNTER" ]]; then
            count="$(<"$MOCK_COUNTER")"
        fi
        count=$((count + 1))
        printf '%s\n' "$count" >"$MOCK_COUNTER"
        if (( count == 1 )); then
            printf '%s %s ext4 rw,nosuid,nodev,relatime\n' \
                "$MOCK_TARGET" "$MOCK_UUID"
        elif [[ "$MOCK_FINDMNT_MODE" == "replaced" ]]; then
            printf '%s wrong-uuid ext4 rw,nosuid,nodev,relatime\n' "$MOCK_TARGET"
        else
            exit 1
        fi
        ;;
    unavailable)
        exit 1
        ;;
    wrong-uuid)
        printf '%s wrong-uuid ext4 rw,nosuid,nodev,relatime\n' "$MOCK_TARGET"
        ;;
    wrong-options)
        printf '%s %s ext4 rw,nosuid,relatime\n' "$MOCK_TARGET" "$MOCK_UUID"
        ;;
    read-only)
        printf '%s %s ext4 ro,nosuid,nodev,relatime\n' "$MOCK_TARGET" "$MOCK_UUID"
        ;;
    noexec)
        printf '%s %s ext4 rw,nosuid,nodev,noexec,relatime\n' \
            "$MOCK_TARGET" "$MOCK_UUID"
        ;;
    *)
        exit 64
        ;;
esac
MOCK
chmod 0755 "$mock_findmnt"

guard_env=(
    "HOME=$home_path"
    "ASTRID_ICP_SSD_GUARD_TEST_MODE=1"
    "ASTRID_ICP_SSD_GUARD_MOUNT_PATH=$mount_path"
    "ASTRID_ICP_SSD_GUARD_EXPECTED_UUID=$expected_uuid"
    "ASTRID_ICP_SSD_GUARD_ROOT_PATH=$root_path"
    "ASTRID_ICP_SSD_GUARD_LINK_PATH=$link_path"
    "ASTRID_ICP_SSD_GUARD_STATE_PATH=$state_path"
    "ASTRID_ICP_SSD_GUARD_STATE_BIN_PATH=$state_bin_path"
    "ASTRID_ICP_SSD_GUARD_STATE_RUN_PATH=$state_run_path"
    "ASTRID_ICP_SSD_GUARD_STATE_VAR_PATH=$state_var_path"
    "ASTRID_ICP_SSD_GUARD_STATE_HOME_PATH=$state_home_path"
    "ASTRID_ICP_SSD_GUARD_STATE_DEFAULT_PATH=$state_default_path"
    "ASTRID_ICP_SSD_GUARD_EDGE_WORKSPACE_PATH=$edge_workspace_path"
    "ASTRID_ICP_SSD_GUARD_WORKSPACE_PATH=$workspace_path"
    "ASTRID_ICP_SSD_GUARD_TMP_PATH=$tmp_path"
    "ASTRID_ICP_SSD_GUARD_OLLAMA_PATH=$ollama_path"
    "ASTRID_ICP_SSD_GUARD_OLLAMA_RUNTIME_PATH=$ollama_runtime_path"
    "ASTRID_ICP_SSD_GUARD_OLLAMA_RUNTIME_BIN_PATH=$ollama_runtime_bin_path"
    "ASTRID_ICP_SSD_GUARD_OLLAMA_MODELS_PATH=$ollama_models_path"
    "ASTRID_ICP_SSD_GUARD_FINDMNT=$mock_findmnt"
    "ASTRID_ICP_SSD_GUARD_READLINK=/usr/bin/readlink"
    "ASTRID_ICP_SSD_GUARD_SLEEP=/bin/sleep"
    "MOCK_TARGET=$mount_path"
    "MOCK_UUID=$expected_uuid"
    "MOCK_COUNTER=$counter_path"
)

run_guard() {
    local mode="$1"
    shift
    env "${guard_env[@]}" "MOCK_FINDMNT_MODE=$mode" "$guard" "$@"
}

expect_status() {
    local expected="$1"
    local output="$2"
    shift 2
    local status=0

    set +e
    "$@" >"$output" 2>&1
    status=$?
    set -e
    if (( status != expected )); then
        printf 'error: expected status %s, received %s\n' "$expected" "$status" >&2
        sed -n '1,80p' "$output" >&2
        exit 1
    fi
}

rm -f -- "$link_path"
mount_only_output="$test_root/mount-only.out"
expect_status 0 "$mount_only_output" \
    run_guard success --mount-only --wait-seconds 0
grep -Fqx 'icp-ssd-guard: ready' "$mount_only_output"
ln -s "$root_path" "$link_path"

bootstrap_output="$test_root/bootstrap.out"
expect_status 0 "$bootstrap_output" \
    run_guard success --bootstrap --wait-seconds 0
grep -Fqx 'icp-ssd-guard: ready' "$bootstrap_output"

install -d -m 0700 \
    "$state_bin_path" \
    "$state_run_path" \
    "$state_var_path" \
    "$edge_workspace_path" \
    "$workspace_path" \
    "$tmp_path" \
    "$ollama_runtime_bin_path" \
    "$ollama_models_path"

immediate_output="$test_root/immediate.out"
expect_status 0 "$immediate_output" run_guard success --wait-seconds 0
grep -Fqx 'icp-ssd-guard: ready' "$immediate_output"

rm -f -- "$counter_path"
delayed_output="$test_root/delayed.out"
expect_status 0 "$delayed_output" \
    run_guard delayed --wait-seconds 2 --poll-seconds 1
grep -Fqx 'icp-ssd-guard: ready' "$delayed_output"
test "$(<"$counter_path")" -eq 3

rm -f -- "$counter_path"
disappears_output="$test_root/disappears.out"
expect_status 75 "$disappears_output" run_guard disappears --wait-seconds 0
grep -Fqx \
    'icp-ssd-guard: timeout: mount metadata unavailable' \
    "$disappears_output"
test "$(<"$counter_path")" -eq 2

rm -f -- "$counter_path"
replaced_output="$test_root/replaced.out"
expect_status 78 "$replaced_output" run_guard replaced --wait-seconds 0
grep -Fq 'icp-ssd-guard: permanent: wrong UUID:' "$replaced_output"
test "$(<"$counter_path")" -eq 2

timeout_output="$test_root/timeout.out"
expect_status 75 "$timeout_output" run_guard unavailable --wait-seconds 0
grep -Fqx 'icp-ssd-guard: timeout: mount metadata unavailable' "$timeout_output"

condition_ready_output="$test_root/condition-ready.out"
expect_status 0 "$condition_ready_output" \
    run_guard success --systemd-condition --wait-seconds 0
grep -Fqx 'icp-ssd-guard: ready' "$condition_ready_output"

condition_timeout_output="$test_root/condition-timeout.out"
expect_status 255 "$condition_timeout_output" \
    run_guard unavailable --systemd-condition --wait-seconds 0
grep -Fqx \
    'icp-ssd-guard: timeout: mount metadata unavailable' \
    "$condition_timeout_output"

uuid_output="$test_root/uuid.out"
expect_status 78 "$uuid_output" run_guard wrong-uuid --wait-seconds 2
grep -Fq 'icp-ssd-guard: permanent: wrong UUID:' "$uuid_output"

condition_uuid_output="$test_root/condition-uuid.out"
expect_status 78 "$condition_uuid_output" \
    run_guard wrong-uuid --systemd-condition --wait-seconds 2
grep -Fq 'icp-ssd-guard: permanent: wrong UUID:' "$condition_uuid_output"

options_output="$test_root/options.out"
expect_status 78 "$options_output" run_guard wrong-options --wait-seconds 2
grep -Fqx \
    'icp-ssd-guard: permanent: required mount option is absent: nodev' \
    "$options_output"

readonly_output="$test_root/read-only.out"
expect_status 78 "$readonly_output" \
    run_guard read-only --bootstrap --wait-seconds 2
grep -Fqx \
    'icp-ssd-guard: permanent: forbidden mount option is present: ro' \
    "$readonly_output"

noexec_output="$test_root/noexec.out"
expect_status 78 "$noexec_output" run_guard noexec --wait-seconds 2
grep -Fqx \
    'icp-ssd-guard: permanent: forbidden mount option is present: noexec' \
    "$noexec_output"

rmdir -- "$workspace_path"
ln -s "$state_path" "$workspace_path"
symlinked_child_output="$test_root/symlinked-child.out"
expect_status 78 "$symlinked_child_output" run_guard success --wait-seconds 2
grep -Fq \
    'icp-ssd-guard: permanent: workspace directory must not be a symlink:' \
    "$symlinked_child_output"
rm -f -- "$workspace_path"
install -d -m 0700 "$workspace_path"

rmdir -- "$state_bin_path"
ln -s "$ollama_runtime_bin_path" "$state_bin_path"
symlinked_binary_output="$test_root/symlinked-binary.out"
expect_status 78 "$symlinked_binary_output" run_guard success --wait-seconds 2
grep -Fq \
    'icp-ssd-guard: permanent: state binary directory must not be a symlink:' \
    "$symlinked_binary_output"
rm -f -- "$state_bin_path"
install -d -m 0700 "$state_bin_path"

escaped_state_path="$test_root/escaped-state"
install -d -m 0700 "$escaped_state_path"
escaped_child_output="$test_root/escaped-child.out"
expect_status 78 "$escaped_child_output" \
    env "${guard_env[@]}" \
    "ASTRID_ICP_SSD_GUARD_STATE_PATH=$escaped_state_path" \
    MOCK_FINDMNT_MODE=success \
    "$guard" --wait-seconds 2
grep -Fq \
    'icp-ssd-guard: permanent: state directory escapes the exact SSD root:' \
    "$escaped_child_output"

rmdir -- "$workspace_path"
missing_child_output="$test_root/missing-child.out"
expect_status 78 "$missing_child_output" run_guard success --wait-seconds 2
grep -Fq \
    'icp-ssd-guard: permanent: workspace directory is not a directory:' \
    "$missing_child_output"
install -d -m 0700 "$workspace_path"

chmod 0500 "$tmp_path"
unwritable_child_output="$test_root/unwritable-child.out"
expect_status 78 "$unwritable_child_output" run_guard success --wait-seconds 2
grep -Fq \
    'icp-ssd-guard: permanent: temporary directory is not owner-writable:' \
    "$unwritable_child_output"
chmod 0700 "$tmp_path"

rm -f -- "$link_path"
ln -s "$mount_path/wrong-root" "$link_path"
install -d -m 0700 "$mount_path/wrong-root"
link_output="$test_root/link.out"
expect_status 78 "$link_output" run_guard success --wait-seconds 2
grep -Fq 'icp-ssd-guard: permanent: wrong symlink target:' "$link_output"
rm -f -- "$link_path"
ln -s "$root_path" "$link_path"

args_output="$test_root/args.out"
expect_status 2 "$args_output" run_guard success --poll-seconds 0
grep -Fq -- '--poll-seconds must be an integer from 1 through 60' "$args_output"

exclusive_output="$test_root/exclusive.out"
expect_status 2 "$exclusive_output" \
    run_guard success --mount-only --bootstrap --wait-seconds 0
grep -Fq -- '--mount-only and --bootstrap are mutually exclusive' \
    "$exclusive_output"

command_output="$test_root/command.out"
expect_status 78 "$command_output" \
    env "${guard_env[@]}" \
    ASTRID_ICP_SSD_GUARD_FINDMNT=/definitely/missing/findmnt \
    MOCK_FINDMNT_MODE=success \
    "$guard" --wait-seconds 0
grep -Fqx \
    'icp-ssd-guard: permanent: findmnt command is unavailable: /definitely/missing/findmnt' \
    "$command_output"

printf 'wait-for-icp-ssd tests passed\n'
