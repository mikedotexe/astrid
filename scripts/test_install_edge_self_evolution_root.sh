#!/usr/bin/env bash
# Adversarial, mutation-free tests for the root bootstrap contract.
set -euo pipefail
IFS=$'\n\t'

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)
REPO_ROOT=$(cd "$SCRIPT_DIR/.." && pwd -P)
INSTALLER=$SCRIPT_DIR/install_edge_self_evolution_root.sh
TEMP=$(mktemp -d)
TEMP=$(realpath "$TEMP")
trap 'rm -rf -- "$TEMP"' EXIT

fail() { printf 'FAIL: %s\n' "$*" >&2; exit 1; }
expect_failure() {
    local pattern=$1
    shift
    if "$@" >"$TEMP/failure.out" 2>"$TEMP/failure.err"; then fail "command unexpectedly succeeded: $pattern"; fi
    grep -q "$pattern" "$TEMP/failure.err" || { cat "$TEMP/failure.err" >&2; fail "missing failure: $pattern"; }
}
hash_file() { sha256sum -- "$1" | awk '{print $1}'; }

WORKSPACE=$TEMP/.astrid/home/default/edge
mkdir -p "$TEMP/input" \
    "$WORKSPACE/introspections" "$WORKSPACE/runtime" "$WORKSPACE/self-change" \
    "$WORKSPACE/autonomous" "$WORKSPACE/actions" "$WORKSPACE/self" "$WORKSPACE/perception" \
    "$TEMP/host/appliance" "$TEMP/host/data" "$TEMP/mockbin" "$TEMP/runtime" \
    "$TEMP/.config/systemd/user" "$TEMP/.config/astrid" \
    "$TEMP/.astrid/home/default/.local/capsules" "$TEMP/.astrid/home/default/.config/env" "$TEMP/.local/share/ollama/models" \
    "$TEMP/.local/bin" "$TEMP/.local/ollama-v0.32.5/bin"
printf '\177ELFmock-ollama\n' >"$TEMP/.local/ollama-v0.32.5/bin/ollama"
chmod 0755 "$TEMP/.local/ollama-v0.32.5/bin/ollama"
ln -s "$TEMP/.local/ollama-v0.32.5/bin/ollama" "$TEMP/.local/bin/ollama"
for unit in ollama-cpu.service astrid-model-warmup.service astrid.service astrid-edge-runtime.service astrid-edge-hindsight.service astrid-edge-hindsight.timer; do
    cp "$REPO_ROOT/packaging/systemd/$unit" "$TEMP/.config/systemd/user/$unit"
done
printf '\177ELFmock-native-helper\n' >"$TEMP/input/steward-helper"
printf '\177ELFmock-rescue-helper\n' >"$TEMP/input/rescue-helper"
printf '\177ELFmock-checkpoint\n' >"$TEMP/input/checkpoint"
printf '\177ELFmock-capsule-builder\n' >"$TEMP/input/capsule-builder"
printf '\177ELFmock-web-broker\n' >"$TEMP/input/web-broker"
printf '\177ELFmock-provider-broker\n' >"$TEMP/input/provider-broker"
printf '\177ELFmock-presentation-broker\n' >"$TEMP/input/presentation-broker"
printf '#!/usr/bin/env python3\nprint("mock supervisor")\n' >"$TEMP/input/supervisor.pyz"
printf 'S%.0s' {1..32} >"$TEMP/input/source.key"
printf 'signed source bundle fixture\n' >"$TEMP/input/source.tar.gz"
printf 'offline toolchain bundle fixture\n' >"$TEMP/input/toolchain.tar.gz"
printf 'initial generation bundle fixture\n' >"$TEMP/input/generation.tar.gz"
chmod 0500 "$TEMP/input/steward-helper" "$TEMP/input/rescue-helper" "$TEMP/input/checkpoint" "$TEMP/input/capsule-builder" "$TEMP/input/web-broker" "$TEMP/input/provider-broker" "$TEMP/input/presentation-broker" "$TEMP/input/supervisor.pyz"
chmod 0600 "$TEMP/input/source.key" "$TEMP/input"/*.tar.gz
printf '%s\n' '{"schema":"astrid_edge_autonomy_state_v3","last_status":"authored_completed","consecutive_failures":0,"run_receipt_pending":false,"chain_receipt_pending":false,"action_dispatch_pending":false,"pending_action_response_sha256":null,"pending_action_trace":null,"pending_action_session_id":null,"pending_action_transcript_path":null,"pending_action_response_provenance":null,"thread_projection_pending":null}' >"$WORKSPACE/autonomous/state.json"
printf '\n' >"$WORKSPACE/actions/receipts.jsonl"
printf '{"schema":"fixture"}\n' >"$WORKSPACE/autonomous/thread_state.json"
printf '\n' >"$WORKSPACE/autonomous/thread_state.jsonl"
printf '{"schema":"fixture"}\n' >"$WORKSPACE/self/profile.json"
printf '{"schema":"fixture"}\n' >"$WORKSPACE/perception/latest.json"
mkdir -p "$WORKSPACE/operator/hindsight" "$WORKSPACE/web" \
    "$WORKSPACE/introspection" "$WORKSPACE/runtime"
printf '{"timestamp_unix_ms":0}\n' >"$WORKSPACE/runtime/spectral_state.json"
printf '{"schema":"fixture"}\n' >"$WORKSPACE/operator/hindsight/latest.json"
printf '{"fill_ratio":0.68}\n' >"$WORKSPACE/runtime/fill_history.jsonl"
printf '\n' >"$WORKSPACE/web/receipts.jsonl"
printf '\n' >"$WORKSPACE/introspection/receipts.jsonl"
printf '{"model":"qwen3.5:4b","max_output_tokens":"192"}\n' >"$TEMP/.astrid/home/default/.config/env/astrid-capsule-openai-compat.env.json"
chmod 0600 "$TEMP/.astrid/home/default/.config/env/astrid-capsule-openai-compat.env.json"
printf '42\n' >"$TEMP/runtime/thermal"
mkdir "$TEMP/runtime/model-ipc"

MOCK_LOG=$TEMP/mock.log
for command in systemctl launchctl useradd usermod groupadd; do
    cat >"$TEMP/mockbin/$command" <<EOF
#!/bin/sh
printf '%s %s\n' '$command' "\$*" >>'$MOCK_LOG'
exit 99
EOF
    chmod 0700 "$TEMP/mockbin/$command"
done

runtime_user=$(id -un)
declare -a ARGS=(
    --dry-run
    --start-system-services
    --appliance-id avado-edge
    --target x86_64-unknown-linux-gnu
    --runtime-user "$runtime_user"
    --runtime-home "$TEMP"
    --runtime-workspace "$WORKSPACE"
    --model-ipc "$TEMP/runtime/model-ipc"
    --steward-owned "continuity=$WORKSPACE/autonomous/thread_state.json"
    --steward-owned "self_profile=$WORKSPACE/self/profile.json"
    --steward-owned "verified_evidence=$WORKSPACE/autonomous/thread_state.jsonl"
    --steward-owned "machine_observation=$WORKSPACE/perception/latest.json"
    --steward-owned "spectral_host_state=$WORKSPACE/runtime/spectral_state.json"
    --model qwen3.5:4b
    --ollama-origin http://127.0.0.1:11434
    --context-tokens 4096
    --output-tokens 192
    --reflection-output-tokens 384
    --source-authoring-output-tokens 384
    --connect-timeout-ms 30000
    --header-timeout-ms 300000
    --total-timeout-ms 600000
    --model-lock "$TEMP/host/data/supervisor-state/model.lock"
    --autonomy-state "$WORKSPACE/autonomous/state.json"
    --action-receipts "$WORKSPACE/actions/receipts.jsonl"
    --thermal-celsius "$TEMP/runtime/thermal"
    --maximum-thermal-celsius 90
    --helper "$TEMP/input/steward-helper"
    --helper-sha256 "$(hash_file "$TEMP/input/steward-helper")"
    --helper-install-path /usr/libexec/astrid/astrid-edge-steward-helper
    --supervisor "$TEMP/input/supervisor.pyz"
    --supervisor-sha256 "$(hash_file "$TEMP/input/supervisor.pyz")"
    --supervisor-install-path /usr/libexec/astrid/edge-self-change-supervisor
    --rescue-helper "$TEMP/input/rescue-helper"
    --rescue-helper-sha256 "$(hash_file "$TEMP/input/rescue-helper")"
    --rescue-helper-install-path /usr/libexec/astrid/astrid-edge-rescue-helper
    --checkpoint "$TEMP/input/checkpoint"
    --checkpoint-sha256 "$(hash_file "$TEMP/input/checkpoint")"
    --checkpoint-install-path /usr/libexec/astrid/astrid-edge-checkpoint
    --capsule-builder "$TEMP/input/capsule-builder"
    --capsule-builder-sha256 "$(hash_file "$TEMP/input/capsule-builder")"
    --capsule-builder-install-path /usr/libexec/astrid/astrid-build
    --web-broker "$TEMP/input/web-broker"
    --web-broker-sha256 "$(hash_file "$TEMP/input/web-broker")"
    --web-broker-install-path /usr/libexec/astrid-edge/immutable/astrid-edge-web-broker
    --provider-broker "$TEMP/input/provider-broker"
    --provider-broker-sha256 "$(hash_file "$TEMP/input/provider-broker")"
    --provider-broker-install-path /usr/libexec/astrid-edge/immutable/astrid-edge-provider-broker
    --presentation-broker "$TEMP/input/presentation-broker"
    --presentation-broker-sha256 "$(hash_file "$TEMP/input/presentation-broker")"
    --presentation-broker-install-path /usr/libexec/astrid-edge/immutable/astrid-edge-presentation-broker
    --source-signing-key "$TEMP/input/source.key"
    --source-signing-key-sha256 "$(hash_file "$TEMP/input/source.key")"
    --source-bundle "$TEMP/input/source.tar.gz"
    --source-bundle-sha256 "$(hash_file "$TEMP/input/source.tar.gz")"
    --toolchain-bundle "$TEMP/input/toolchain.tar.gz"
    --toolchain-bundle-sha256 "$(hash_file "$TEMP/input/toolchain.tar.gz")"
    --initial-generation-bundle "$TEMP/input/generation.tar.gz"
    --initial-generation-sha256 "$(hash_file "$TEMP/input/generation.tar.gz")"
    --initial-generation-id generation-1
    --state-root "$TEMP/host/data/supervisor-state"
    --release-root "$TEMP/host/appliance/releases"
    --source-root "$TEMP/host/data/signed-source"
    --candidate-root "$TEMP/host/data/candidates"
    --builder-root "$TEMP/host/data/builder"
    --updater-root "$TEMP/host/data/updater"
    --inbox-root "$TEMP/host/data/supervisor-state/inbox"
    --vendor-root "$TEMP/host/data/signed-source/vendor"
    --toolchain-root "$TEMP/host/data/toolchain"
    --unit-source-root "$REPO_ROOT/packaging/systemd"
    --system-unit-root /etc/systemd/system
    --user-unit-root "$TEMP/.config/systemd/user"
    --control-root /usr/sbin
    --astrid-system-unit /etc/systemd/system/ollama-cpu.service
    --astrid-system-unit /etc/systemd/system/astrid-model-warmup.service
    --astrid-system-unit /etc/systemd/system/astrid.service
    --astrid-system-unit /etc/systemd/system/astrid-edge-runtime.service
    --astrid-system-unit /etc/systemd/system/astrid-edge-hindsight.service
    --astrid-system-unit /etc/systemd/system/astrid-edge-hindsight.timer
    --install-unit astrid-edge-self-change-supervisor.service
    --install-unit astrid-edge-self-change-probation-health.service
    --install-unit astrid-edge-self-change-probation-health.timer
    --install-unit astrid-edge-steward.service
    --install-unit astrid-edge-steward.timer
    --install-unit astrid-edge-web-broker-core.socket
    --install-unit astrid-edge-web-broker-core.service
    --install-unit astrid-edge-web-broker-runtime.socket
    --install-unit astrid-edge-web-broker-runtime.service
    --install-unit astrid-edge-web-broker-steward.socket
    --install-unit astrid-edge-web-broker-steward.service
    --install-unit astrid-edge-provider-broker@.service
    --install-unit astrid-edge-provider-runtime.socket
    --install-unit astrid-edge-provider-steward.socket
    --install-unit astrid-edge-provider-warmup.socket
    --install-unit astrid-edge-presentation-broker.socket
    --install-unit astrid-edge-presentation-broker@.service
    --install-unit astrid-edge-generation-guard.service
    --install-unit astrid-edge-core-liveness.service
    --install-unit astrid-edge-core-liveness.path
    --install-unit astrid-edge-self-change-inbox.path
    --install-unit astrid-edge-runtime.service.d/60-self-evolution-root.conf
    --enable-unit astrid-edge-steward.timer
    --enable-unit astrid-edge-web-broker-core.socket
    --enable-unit astrid-edge-self-change-probation-health.timer
    --enable-unit astrid-edge-web-broker-runtime.socket
    --enable-unit astrid-edge-web-broker-steward.socket
    --enable-unit astrid-edge-provider-runtime.socket
    --enable-unit astrid-edge-provider-steward.socket
    --enable-unit astrid-edge-provider-warmup.socket
    --enable-unit astrid-edge-presentation-broker.socket
    --enable-unit astrid-edge-generation-guard.service
    --enable-unit astrid-edge-core-liveness.path
    --enable-unit astrid-edge-self-change-inbox.path
)
for unit in ollama-cpu.service astrid-model-warmup.service astrid.service astrid-edge-runtime.service astrid-edge-hindsight.service astrid-edge-hindsight.timer; do
    ARGS+=(--astrid-system-unit-sha256 "$unit=$(hash_file "$REPO_ROOT/packaging/systemd/$unit")")
done

PATH="$TEMP/mockbin:$PATH" "$INSTALLER" "${ARGS[@]}" >"$TEMP/valid.out"
rm "$WORKSPACE/autonomous/thread_state.json" "$WORKSPACE/autonomous/thread_state.jsonl"
PATH="$TEMP/mockbin:$PATH" "$INSTALLER" "${ARGS[@]}" >"$TEMP/absent-owned-leaves.out"
grep -q 'actual CLI: --config \[--credential-directory\]' "$TEMP/absent-owned-leaves.out" || fail "canonical absent owned leaves were not accepted as unavailable"
printf '{"schema":"fixture"}\n' >"$WORKSPACE/autonomous/thread_state.json"
printf '\n' >"$WORKSPACE/autonomous/thread_state.jsonl"
grep -q 'actual CLI: --config \[--credential-directory\]' "$TEMP/valid.out" || fail "native CLI not reported"
! grep -q -- '--due-nonce\|--question' "$TEMP/valid.out" || fail "obsolete operator-selected prompt surface is advertised"
grep -q 'Python supervisor (actual CLI: --config --execute COMMAND)' "$TEMP/valid.out" || fail "supervisor CLI not reported"
grep -q 'bounded build/install/activate/rollback/health profiles enabled' "$TEMP/valid.out" || fail "full rescue profiles omitted"
grep -q 'immutable web broker' "$TEMP/valid.out" || fail "immutable web broker omitted"
grep -q 'persisted quotas core=8/hour+24/UTC-day runtime=8/hour+24/UTC-day steward=2/hour+12/UTC-day max=2/trace' "$TEMP/valid.out" || fail "immutable persisted web quotas omitted"
grep -q 'immutable provider broker' "$TEMP/valid.out" || fail "immutable provider broker omitted"
grep -q 'immutable presentation broker' "$TEMP/valid.out" || fail "immutable presentation broker omitted"
grep -q 'immutable operator reports=/usr/libexec/astrid-edge/operator' "$TEMP/valid.out" || fail "immutable operator report tree omitted"
grep -q 'sealed train runs directly; other exact Python -I launchers run trusted reports before optional bounded presentation' "$TEMP/valid.out" || fail "operator report isolation guarantee omitted"
grep -q 'fixed 64 GiB fully allocated ext4 builder store' "$TEMP/valid.out" || fail "persistent bounded builder store omitted"
grep -q 'independent 32 GiB runtime=.*rollback=.*runtime reserves 20%.*65,536 emergency inodes.*aggregate backing reserve=64 GiB' "$TEMP/valid.out" \
    || fail "independent bounded runtime and rollback stores omitted"
grep -q 'loopback-only origin=http://127.0.0.1:11434' "$TEMP/valid.out" || fail "loopback boundary omitted"
grep -q 'no helper/supervisor execution, filesystem write, systemctl, user service, or launchctl' "$TEMP/valid.out" || fail "dry-run guarantee omitted"
grep -q 'migrate exact user stack to root-owned system units' "$TEMP/valid.out" || fail "root system-unit migration omitted"
grep -q 'root runtime bindings: appliance=avado-edge' "$TEMP/valid.out" || fail "absolute root runtime binding report omitted"
grep -Fq "profile Ollama executable=$TEMP/.local/ollama-v0.32.5/bin/ollama runtime-root=$TEMP/.local/ollama-v0.32.5 models=$TEMP/.local/share/ollama/models" "$TEMP/valid.out" || fail "AVADO root migration did not bind the resolved versioned Ollama layout"
grep -Fq "ollama executable=$TEMP/.local/ollama-v0.32.5/bin/ollama runtime-root=$TEMP/.local/ollama-v0.32.5 models=$TEMP/.local/share/ollama/models" "$TEMP/valid.out" || fail "AVADO migrator did not report the exact Ollama layout"
grep -q 'reset ExecStart/ExecStartPre and verify the pinned digest on every start' "$TEMP/valid.out" || fail "AVADO migrator omitted the per-start Ollama digest gate"
grep -q 'edge env appliance=avado-edge.*legacy-scheduler=false dedicated-steward=true/120m' "$TEMP/valid.out" || fail "effective migrated runtime environment omitted"
grep -q 'core-liveness-recovery.request.json' "$TEMP/valid.out" || fail "core-liveness request path omitted"
grep -q 'immutable evidence projection=' "$TEMP/valid.out" || fail "immutable introspection evidence projection omitted"
grep -q 'root helper activates units only through private alias=.*updater/system-units' "$TEMP/valid.out" || fail "private unit activation alias omitted"
[[ ! -s $MOCK_LOG ]] || fail "dry-run invoked a mutating mock"
for path in "$TEMP/host/data/supervisor-state" "$TEMP/host/data/signed-source" "$TEMP/host/data/candidates" "$TEMP/host/appliance/releases"; do [[ ! -e $path ]] || fail "dry-run created $path"; done

# Exercise the second live appliance layout directly through the immutable
# migrator. ICP has no ~/.local/bin/ollama; its executable and CPU libraries
# live below the SSD-backed runtime root while only models remain writable.
MIGRATOR=$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system
ICP_HOME=$TEMP/icp-home
ICP_USER_UNITS=$ICP_HOME/.config/systemd/user
ICP_DATA=$TEMP/icp-data/astrid
ICP_OLLAMA=$ICP_DATA/ollama/runtime/bin/ollama
mkdir -p "$ICP_USER_UNITS" "$ICP_HOME/.config/astrid" \
    "$ICP_DATA/state/home/default/.local/capsules" \
    "$ICP_DATA/state/home/default/.config/env" \
    "$ICP_DATA/state/home/default/edge" \
    "$ICP_DATA/state/tmp" \
    "$ICP_DATA/workspace" \
    "$ICP_DATA/ollama/runtime/bin" \
    "$ICP_DATA/ollama/models"
ln -s "$ICP_DATA" "$ICP_HOME/.astrid-icp"
printf '\177ELFmock-icp-ollama\n' >"$ICP_OLLAMA"
chmod 0755 "$ICP_OLLAMA"
printf '{"model":"qwen3:1.7b","max_output_tokens":"128"}\n' >"$ICP_HOME/.astrid-icp/state/home/default/.config/env/astrid-capsule-openai-compat.env.json"
chmod 0600 "$ICP_HOME/.astrid-icp/state/home/default/.config/env/astrid-capsule-openai-compat.env.json"
declare -a ICP_MIGRATION_ARGS=(
    --dry-run
    --profile icp
    --appliance-id icp-edge
    --runtime-user "$runtime_user"
    --runtime-home "$ICP_HOME"
    --unit-source-root "$REPO_ROOT/packaging/systemd"
    --user-unit-root "$ICP_USER_UNITS"
    --system-unit-root /etc/systemd/system
    --rescue-system-unit-root "$TEMP/host/data/updater/system-units"
    --active-generation-root "$TEMP/host/appliance/releases/current"
    --source-root "$TEMP/host/data/signed-source"
    --candidate-root "$TEMP/host/data/candidates"
    --builder-root "$TEMP/host/data/builder"
    --updater-root "$TEMP/host/data/updater"
    --toolchain-root "$TEMP/host/data/toolchain"
    --management-marker /etc/astrid/edge-service-manager.json
    --self-evolution-dropin-sha256 "$(hash_file "$REPO_ROOT/packaging/systemd/root/astrid-edge-runtime-self-evolution.conf.in")"
    --post-install-verifier "$TEMP/input/rescue-helper"
    --post-install-verifier-config "$TEMP/input/rescue-config.json"
    --model-lock "$TEMP/host/data/supervisor-state/model.lock"
    --maintenance-lease "$TEMP/host/data/supervisor-state/maintenance.json"
    --authority-env /etc/astrid/edge-self-change-authority.env
    --unit-policy "$TEMP/host/data/supervisor-state/unit-policy.json"
    --ollama-binary "$ICP_OLLAMA"
    --ollama-binary-sha256 "$(hash_file "$ICP_OLLAMA")"
    --operator-report-manifest-sha256 0000000000000000000000000000000000000000000000000000000000000000
    --provider-output-tokens 112
    --state-store-helper "$REPO_ROOT/packaging/systemd/root/astrid-edge-state-store"
    --state-store-helper-sha256 "$(hash_file "$REPO_ROOT/packaging/systemd/root/astrid-edge-state-store")"
    --state-store-config /etc/astrid/edge-state-store.json
    --state-store-runtime-mount-unit astrid-edge-bounded-runtime.mount
    --state-store-rollback-mount-unit astrid-edge-bounded-rollback.mount
    --state-store-verify-unit astrid-edge-state-store-verify.service
    --state-store-health-timer astrid-edge-state-store-health.timer
    --required-mount /media/data
    --required-mount-uuid 0123-ABCD
    --profile-dropin-sha256 "icp-ssd-required.conf=$(hash_file "$REPO_ROOT/packaging/systemd/icp-ssd-required.conf")"
    --profile-dropin-sha256 "astrid-edge-tuning-authority.conf=$(hash_file "$REPO_ROOT/packaging/systemd/astrid-edge-tuning-authority.conf")"
)
for unit in ollama-cpu.service astrid-model-warmup.service astrid.service astrid-edge-runtime.service astrid-edge-hindsight.service astrid-edge-hindsight.timer; do
    cp "$REPO_ROOT/packaging/systemd/icp/$unit" "$ICP_USER_UNITS/$unit"
    ICP_MIGRATION_ARGS+=(--unit "$unit" --unit-sha256 "$unit=$(hash_file "$REPO_ROOT/packaging/systemd/icp/$unit")")
done
"$MIGRATOR" "${ICP_MIGRATION_ARGS[@]}" >"$TEMP/icp-migration.out"
grep -Fq "ollama executable=$ICP_OLLAMA runtime-root=$ICP_DATA/ollama/runtime models=$ICP_DATA/ollama/models" "$TEMP/icp-migration.out" || fail "ICP migrator did not bind the exact canonical SSD Ollama layout"
! grep -Fq "$ICP_HOME/.local/bin/ollama" "$TEMP/icp-migration.out" || fail "ICP migration retained the absent AVADO Ollama launcher"
expect_failure 'profile Ollama binary digest mismatch' "$MIGRATOR" "${ICP_MIGRATION_ARGS[@]}" --ollama-binary-sha256 "$(printf wrong-ollama | sha256sum | awk '{print $1}')"
expect_failure 'Ollama binary is not the exact profile launcher target' "$MIGRATOR" "${ICP_MIGRATION_ARGS[@]}" --ollama-binary "$TEMP/.local/ollama-v0.32.5/bin/ollama"
expect_failure 'rescue system-unit root must be the exact private updater alias' "$MIGRATOR" "${ICP_MIGRATION_ARGS[@]}" --rescue-system-unit-root /etc/systemd/system

expect_failure 'SHA-256 mismatch' env PATH="$TEMP/mockbin:$PATH" "$INSTALLER" "${ARGS[@]}" --helper-sha256 "$(printf wrong | sha256sum | awk '{print $1}')"
expect_failure 'Ollama must be exact IPv4 loopback HTTP' env PATH="$TEMP/mockbin:$PATH" "$INSTALLER" "${ARGS[@]}" --ollama-origin http://example.com:11434
expect_failure 'invalid model' env PATH="$TEMP/mockbin:$PATH" "$INSTALLER" "${ARGS[@]}" --model 'qwen3.5:4b","escape":true'
expect_failure 'unsupported root-boundary unit' env PATH="$TEMP/mockbin:$PATH" "$INSTALLER" "${ARGS[@]}" --install-unit astrid-edge-builder@.service
expect_failure 'ICP requires exact /media/data UUID guard' env PATH="$TEMP/mockbin:$PATH" "$INSTALLER" "${ARGS[@]}" --appliance-id icp-edge
expect_failure 'require exact five canonical steward-owned inputs' env PATH="$TEMP/mockbin:$PATH" "$INSTALLER" "${ARGS[@]}" --steward-owned "escape=$TEMP/input"
wrong_owned=("${ARGS[@]}")
for index in "${!wrong_owned[@]}"; do
    if [[ ${wrong_owned[$index]} == "continuity=$WORKSPACE/autonomous/thread_state.json" ]]; then
        wrong_owned[$index]="continuity=$WORKSPACE/self/profile.json"
    fi
done
expect_failure 'steward-owned input does not match its canonical workspace path' env PATH="$TEMP/mockbin:$PATH" "$INSTALLER" "${wrong_owned[@]}"
expect_failure "inbox root must be the supervisor's exact state-root/inbox path" env PATH="$TEMP/mockbin:$PATH" "$INSTALLER" "${ARGS[@]}" --inbox-root "$TEMP/host/data/other-inbox"
expect_failure 'model lock must be the exact persistent root-state lock' env PATH="$TEMP/mockbin:$PATH" "$INSTALLER" "${ARGS[@]}" --model-lock "$TEMP/runtime/model.lock"

mkdir -p "$TEMP/.config/systemd/user/astrid.service.d"
printf '# unreviewed\n[Service]\nEnvironment=ESCAPE=yes\n' >"$TEMP/.config/systemd/user/astrid.service.d/evil.conf"
expect_failure 'unreviewed live user drop-in blocks migration' env PATH="$TEMP/mockbin:$PATH" "$INSTALLER" "${ARGS[@]}"
rm -rf "$TEMP/.config/systemd/user/astrid.service.d"
printf '\n# tampered\n' >>"$TEMP/.config/systemd/user/astrid.service"
expect_failure 'deployed user unit differs from reviewed source' env PATH="$TEMP/mockbin:$PATH" "$INSTALLER" "${ARGS[@]}"
cp "$REPO_ROOT/packaging/systemd/astrid.service" "$TEMP/.config/systemd/user/astrid.service"
[[ ! -s $MOCK_LOG ]] || fail "failed dry-run invoked a mutating mock"

if [[ $(id -u) != 0 ]]; then
    no_dry=()
    for argument in "${ARGS[@]}"; do [[ $argument == --dry-run ]] || no_dry+=("$argument"); done
    expect_failure 'installation requires root' env PATH="$TEMP/mockbin:$PATH" "$INSTALLER" "${no_dry[@]}"
fi

# Exercise the install-time source verifier directly.  The dry-run deliberately
# never extracts privileged inputs, so these signed fixtures prove the exact
# path/origin boundary rather than merely grepping its implementation.
SOURCE_VERIFIER=$TEMP/source-verifier.py
sed -n '/^# BEGIN_INSTALL_SOURCE_VERIFIER$/,/^# END_INSTALL_SOURCE_VERIFIER$/p' \
    "$INSTALLER" >"$SOURCE_VERIFIER"
grep -q '^import hashlib' "$SOURCE_VERIFIER" || fail "source verifier extraction lacks its imports"
grep -q '^# END_INSTALL_SOURCE_VERIFIER$' "$SOURCE_VERIFIER" || fail "source verifier extraction is incomplete"

make_source_fixture() {
    local name=$1 path=$2 origin=$3 kind=${4:-regular} declared_mode=${5:-0644}
    local root=$TEMP/source-fixtures/$name
    python3 - "$root" "$TEMP/input/source.key" "$path" "$origin" "$kind" "$declared_mode" <<'PY'
import hashlib
import hmac
import json
import os
import shutil
import sys
from pathlib import Path, PurePosixPath

root = Path(sys.argv[1])
key_path = Path(sys.argv[2])
relative = sys.argv[3]
origin = sys.argv[4]
kind = sys.argv[5]
declared_mode = sys.argv[6]
shutil.rmtree(root, ignore_errors=True)
root.mkdir(parents=True, mode=0o700)

capsules = (
    "astrid-capsule-agents", "astrid-capsule-cli", "astrid-capsule-edge-context",
    "astrid-capsule-edge-introspector", "astrid-capsule-edge-spectral",
    "astrid-capsule-fs", "astrid-capsule-http", "astrid-capsule-memory",
    "astrid-capsule-shell", "astrid-capsule-skills", "astrid-capsule-context-engine",
    "astrid-capsule-hook-bridge", "astrid-capsule-identity",
    "astrid-capsule-openai-compat", "astrid-capsule-prompt-builder",
    "astrid-capsule-react", "astrid-capsule-registry", "astrid-capsule-router",
    "astrid-capsule-session", "astrid-capsule-system",
)
services = (
    "astrid-edge-checkpoint", "astrid-edge-presentation-broker",
    "astrid-edge-provider-broker", "astrid-edge-rescue-helper",
    "astrid-edge-runtime", "astrid-edge-steward-helper", "astrid-edge-web-broker",
)
payloads = {
    f"source/capsules/astralis/{capsule}/Cargo.lock":
        (b"version = 4\n", "mutable_build_manifest", "0644", "regular")
    for capsule in capsules
}
for service in services:
    payloads[f"source/services/{service}/Cargo.lock"] = (
        b"version = 4\n",
        "mutable_build_manifest" if service == "astrid-edge-runtime" else "inspect_only_immutable_boundary",
        "0644",
        "regular",
    )
payloads["source/crates/astrid-openclaw/kernel/engine.wasm"] = (
    b"\0asm\x01\0\0\0fixture-kernel", "build_required_immutable", "0644", "regular"
)
payloads["source/crates/astrid-openclaw/kernel/engine.wasm.blake3"] = (
    ("a" * 64 + "  engine.wasm\n").encode("ascii"),
    "build_required_immutable",
    "0644",
    "regular",
)
payloads[relative] = (b"signed source fixture\n", origin, declared_mode, kind)

records = []
for payload_relative, (content, payload_origin, payload_mode, payload_kind) in sorted(payloads.items()):
    payload = root.joinpath(*PurePosixPath(payload_relative).parts)
    payload.parent.mkdir(parents=True, exist_ok=True)
    if payload_kind in {"regular", "hardlink", "extra-dir"}:
        payload.write_bytes(content)
        os.chmod(payload, 0o700 if payload_mode == "0755" else 0o600)
        if payload_kind == "hardlink":
            os.link(payload, root / "unlisted-hardlink")
        if payload_kind == "extra-dir":
            (root / "unlisted-directory").mkdir()
    elif payload_kind == "symlink":
        payload.symlink_to("/dev/null")
        content = b""
    elif payload_kind == "fifo":
        os.mkfifo(payload, 0o600)
        content = b""
    else:
        raise SystemExit(f"unknown fixture kind: {payload_kind}")
    records.append({
        "path": payload_relative,
        "origin": payload_origin,
        "mode": payload_mode,
        "size": len(content),
        "sha256": hashlib.sha256(content).hexdigest(),
    })

key = key_path.read_bytes()
key_id = hashlib.sha256(key).hexdigest()[:16]
identity = {
    "schema": "astrid.edge.self_change_source_identity.v1",
    "appliance_id": None,
    "source_authority": "portable_bootstrap_non_authorizing",
    "repository_commit": "0" * 40,
    "rustc": {},
    "files": records,
}
identity_hash = hashlib.sha256(json.dumps(
    identity,
    sort_keys=True,
    separators=(",", ":"),
    ensure_ascii=True,
    allow_nan=False,
).encode("ascii")).hexdigest()
manifest = {
    "schema": "astrid.edge.self_change_source_bundle.v1",
    "appliance_id": None,
    "source_authority": "portable_bootstrap_non_authorizing",
    "source_id": "cpu-edge-portable:" + identity_hash,
    "source_identity_sha256": identity_hash,
    "repository_commit": "0" * 40,
    "git_object_format": "sha1",
    "rustc": {},
    "cargo_lock_version": 4,
    "cargo_lock_sha256": "0" * 64,
    "vendor_packages": [],
    "signature_mode": "hmac-sha256",
    "key_id": key_id,
    "file_count": len(records),
    "uncompressed_bytes": sum(record["size"] for record in records),
    "files": records,
}
canonical = json.dumps(
    manifest,
    sort_keys=True,
    separators=(",", ":"),
    ensure_ascii=True,
    allow_nan=False,
).encode("ascii")
signature = {
    "schema": "astrid.edge.self_change_source_signature.v1",
    "mode": "hmac-sha256",
    "key_id": key_id,
    "manifest_sha256": hashlib.sha256(canonical).hexdigest(),
    "hmac_sha256": hmac.new(key, canonical, hashlib.sha256).hexdigest(),
}
(root / "MANIFEST.json").write_bytes(canonical + b"\n")
(root / "MANIFEST.signature.json").write_bytes(
    json.dumps(signature, sort_keys=True, separators=(",", ":")).encode("ascii") + b"\n"
)
os.chmod(root / "MANIFEST.json", 0o600)
os.chmod(root / "MANIFEST.signature.json", 0o600)
print(root)
PY
}

valid_source_fixtures=(
    'ordinary|source/Cargo.toml|mutable_build_manifest|regular|0644'
    'service-manifest|source/services/astrid-edge-rescue-helper/Cargo.toml|inspect_only_immutable_boundary|regular|0644'
    'service-source|source/services/astrid-edge-web-broker/src/main.rs|inspect_only_immutable_boundary|regular|0644'
    'policy-module|source/scripts/edge_self_change/model.py|inspect_only_immutable_boundary|regular|0644'
    'reviewed-script|source/scripts/install_edge_self_evolution_root.sh|inspect_only_immutable_boundary|regular|0755'
    'root-program|source/packaging/systemd/root/migrate-edge-user-services-to-system|inspect_only_immutable_boundary|regular|0755'
    'root-template|source/packaging/systemd/root/astrid-edge-builder-store.mount.in|inspect_only_immutable_boundary|regular|0644'
    'system-unit|source/packaging/systemd/astrid-edge-self-change-probation-health.service|inspect_only_immutable_boundary|regular|0644'
    'documentation|source/docs/cpu-edge-self-evolution.md|inspect_only_immutable_boundary|regular|0644'
    'vendor-checksum|vendor/example/.cargo-checksum.json|operator_vendored_cargo|regular|0644'
)
for fixture in "${valid_source_fixtures[@]}"; do
    IFS='|' read -r fixture_name fixture_path fixture_origin fixture_kind fixture_mode <<<"$fixture"
    fixture_root=$(make_source_fixture "$fixture_name" "$fixture_path" "$fixture_origin" "$fixture_kind" "$fixture_mode")
    python3 "$SOURCE_VERIFIER" "$fixture_root" "$TEMP/input/source.key"
done

fixture_root=$(make_source_fixture forged-mutable \
    source/services/astrid-edge-steward-helper/src/main.rs mutable_core_source)
expect_failure 'origin disagrees with exact path policy' python3 "$SOURCE_VERIFIER" "$fixture_root" "$TEMP/input/source.key"
fixture_root=$(make_source_fixture forged-inspect source/Cargo.toml inspect_only_immutable_boundary)
expect_failure 'origin disagrees with exact path policy' python3 "$SOURCE_VERIFIER" "$fixture_root" "$TEMP/input/source.key"
for fixture in \
    'unknown-helper|source/services/astrid-edge-rescue-helper/README.md' \
    'helper-config|source/services/astrid-edge-checkpoint/config.json' \
    'unlisted-policy|source/scripts/edge_self_change/policy.toml' \
    'unlisted-script|source/scripts/unlisted_boundary.py' \
    'root-secret|source/packaging/systemd/root/private.key' \
    'private-state|source/crates/astrid-core/src/state/private.rs' \
    'model-blob|source/models/qwen.gguf'; do
    IFS='|' read -r fixture_name fixture_path <<<"$fixture"
    fixture_root=$(make_source_fixture "$fixture_name" "$fixture_path" mutable_core_source)
    expect_failure 'excluded or unexpected source path' python3 "$SOURCE_VERIFIER" "$fixture_root" "$TEMP/input/source.key"
done
fixture_root=$(make_source_fixture source-symlink source/Cargo.toml mutable_build_manifest symlink)
expect_failure 'type/size mismatch' python3 "$SOURCE_VERIFIER" "$fixture_root" "$TEMP/input/source.key"
fixture_root=$(make_source_fixture source-fifo source/Cargo.toml mutable_build_manifest fifo)
expect_failure 'type/size mismatch' python3 "$SOURCE_VERIFIER" "$fixture_root" "$TEMP/input/source.key"
fixture_root=$(make_source_fixture source-hardlink source/Cargo.toml mutable_build_manifest hardlink)
expect_failure 'type/size mismatch' python3 "$SOURCE_VERIFIER" "$fixture_root" "$TEMP/input/source.key"
fixture_root=$(make_source_fixture source-extra-directory source/Cargo.toml mutable_build_manifest extra-dir)
expect_failure 'directory membership mismatch' python3 "$SOURCE_VERIFIER" "$fixture_root" "$TEMP/input/source.key"

python3 - "$REPO_ROOT/scripts/build_edge_self_change_source_bundle.py" "$SOURCE_VERIFIER" <<'PY'
import runpy
import sys
from pathlib import Path

bundler = runpy.run_path(sys.argv[1])
verifier_source = Path(sys.argv[2]).read_text().split(
    "root, key_path = map(Path, sys.argv[1:])", 1
)[0]
verifier = {}
exec(verifier_source, verifier)
for name in (
    "PRIVATE_COMPONENTS",
    "INSPECT_ONLY_SERVICE_PREFIXES",
    "INSPECT_ONLY_SCRIPT_NAMES",
    "MUTABLE_CORE_CRATES",
    "EDGE_CAPSULES",
    "BUILD_FILE_SUFFIXES",
    "MUTABLE_UNIT_FRAGMENTS",
):
    if set(bundler[name]) != set(verifier[name]):
        raise SystemExit(f"installer source policy constant drifted from bundler: {name}")

paths = {
    "Cargo.toml",
    ".cargo/config.toml",
    "crates/astrid-core/src/lib.rs",
    "services/astrid-edge-runtime/src/lib.rs",
    "capsules/astralis/astrid-capsule-edge-spectral/src/lib.rs",
    "scripts/report_edge_activity.py",
    "packaging/appliances/avado.env",
    "packaging/systemd/astrid-edge-runtime.service",
    "packaging/systemd/icp/astrid-edge-runtime.service",
    "docs/cpu-edge-self-evolution.md",
}
for prefix in bundler["INSPECT_ONLY_SERVICE_PREFIXES"]:
    paths.update({f"{prefix}Cargo.toml", f"{prefix}src/lib.rs", f"{prefix}README.md"})
paths.update(
    {
        "crates/astrid-capsule/src/cpu_edge_policy.rs",
        "crates/astrid-capsule/src/engine/wasm/host/process.rs",
        "crates/astrid-capsule/src/loader.rs",
    }
)
for name in bundler["INSPECT_ONLY_SCRIPT_NAMES"]:
    paths.add(f"scripts/{name}")
for suffix in (".service", ".in", ".conf", ".key"):
    paths.add(f"packaging/systemd/root/reviewed{suffix}")
for name in (
    "astrid-edge-builder-store",
    "astrid-edge-self-evolution-control",
    "migrate-edge-user-services-to-system",
):
    paths.add(f"packaging/systemd/root/{name}")
for marker in (
    "self-change",
    "edge-steward",
    "edge-web-broker",
    "edge-checkpoint",
    "builder-store",
    "generation-guard",
    "core-liveness",
):
    for suffix in (".service", ".timer", ".socket", ".conf", ".env", ".in", ".md"):
        paths.add(f"packaging/systemd/astrid-edge-{marker}{suffix}")
for path in paths:
    expected = bundler["source_role"](path)
    actual = verifier["expected_source_origin"](path)
    if actual != expected:
        raise SystemExit(
            f"installer source role drifted from bundler for {path}: {actual!r} != {expected!r}"
        )
PY

bash -n "$INSTALLER"
bash -n "$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system"
sh -n "$REPO_ROOT/packaging/systemd/root/astrid-edge-self-evolution-control"
! rg -n -- '--recovery-state|--conversation-state' "$INSTALLER" "$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system" || fail "obsolete steward gate path arguments remain"
expect_failure 'unsupported argument: --recovery-state' "$INSTALLER" --dry-run --recovery-state /tmp/obsolete
expect_failure 'unsupported argument: --conversation-state' "$INSTALLER" --dry-run --conversation-state /tmp/obsolete
grep -q '^ExecStart=/usr/bin/python3 -I -E -s /usr/libexec/astrid/edge-self-change-supervisor --config /etc/astrid/edge-self-change.json --execute supervise$' "$REPO_ROOT/packaging/systemd/astrid-edge-self-change-supervisor.service" || fail "supervisor unit does not use the exact isolated Python entrypoint"
[[ $(grep -Ec '^Exec(StartPre|StopPost)=/usr/libexec/astrid/astrid-edge-rescue-helper --config /etc/astrid/edge-rescue-helper.json recover-model-after-build$' "$REPO_ROOT/packaging/systemd/astrid-edge-self-change-supervisor.service") == 2 ]] || fail "supervisor lacks symmetric boot/crash model restoration"
grep -q '^ExecStart=/usr/libexec/astrid/astrid-edge-rescue-helper --config /etc/astrid/edge-rescue-helper.json recover-core-liveness$' "$REPO_ROOT/packaging/systemd/astrid-edge-core-liveness.service" || fail "dedicated oneshot lacks immutable core-liveness broker"
! grep -q 'recover-core-liveness' "$REPO_ROOT/packaging/systemd/astrid-edge-self-change-supervisor.service" || fail "core liveness is coupled to candidate/reflection supervision"
core_liveness_path="$REPO_ROOT/packaging/systemd/astrid-edge-core-liveness.path.in"
grep -q '^PathExists=@@CORE_LIVENESS_REQUEST@@$' "$core_liveness_path" || fail "core-liveness watcher path is not an exact rendered request"
grep -q '^Unit=astrid-edge-core-liveness.service$' "$core_liveness_path" || fail "core-liveness watcher bypasses its exact immutable oneshot"
! grep -q 'PathChanged=\|PathModified=\|DirectoryNotEmpty=' "$core_liveness_path" || fail "core-liveness watcher accepts a broad trigger"
grep -q '^CapabilityBoundingSet=$' "$REPO_ROOT/packaging/systemd/astrid-edge-core-liveness.service" || fail "core-liveness oneshot retains Linux capabilities"
core_liveness_dropin=$(sed -n '/astrid-edge-core-liveness.service)/,/;;/p' "$INSTALLER")
grep -q "printf 'Group=%s" <<<"$core_liveness_dropin" || fail "core-liveness oneshot lacks the exclusive runtime group"
grep -q 'runtime_group_members' "$INSTALLER" || fail "runtime group exclusivity is not verified"
grep -q 'chmod 0770 "$self_change_liveness_root"' "$INSTALLER" || fail "core-liveness request parent lacks exact group-mediated DAC"
grep -q -- '--bounding-set=-all' "$INSTALLER" || fail "capability-free liveness read/cleanup is not exercised"
grep -q 'mode(0o640)' "$REPO_ROOT/services/astrid-edge-runtime/src/autonomy.rs" || fail "runtime liveness request is not group-readable with exact mode 0640"
grep -q 'before.mode() & 0o777 != 0o640' "$REPO_ROOT/services/astrid-edge-rescue-helper/src/core_liveness.rs" || fail "immutable helper does not require exact liveness request mode 0640"
python3 - \
    "$INSTALLER" \
    "$REPO_ROOT/packaging/systemd/astrid-edge-self-change-supervisor.service" \
    "$REPO_ROOT/packaging/systemd/astrid-edge-steward.timer" \
    "$REPO_ROOT/scripts/edge_self_change/model.py" \
    "$REPO_ROOT/scripts/edge_self_change/profiles.py" <<'PY'
import math
import re
import sys
from pathlib import Path

installer = Path(sys.argv[1]).read_text()
unit = Path(sys.argv[2]).read_text()
timer = Path(sys.argv[3]).read_text()
model = Path(sys.argv[4]).read_text()
profiles = Path(sys.argv[5]).read_text()

def exact(pattern: str, label: str) -> int:
    matches = re.findall(pattern, installer, flags=re.DOTALL)
    if len(matches) != 1:
        raise SystemExit(f"{label} is absent or ambiguous")
    return int(matches[0])

pipeline = exact(r'"pipeline_timeout_seconds":([0-9]+)', "pipeline timeout")
build = exact(r'"build":\{.*?"timeout_seconds":([0-9]+)\}', "build profile timeout")
synthetic = exact(r'"synthetic":\{.*?"timeout_seconds":([0-9]+)\}', "synthetic profile timeout")
service_match = re.findall(r'^TimeoutStartSec=([0-9]+)h$', unit, flags=re.MULTILINE)
if len(service_match) != 1:
    raise SystemExit("supervisor service timeout is absent or ambiguous")
service = int(service_match[0]) * 3600
if (pipeline, build, synthetic, service) != (86_400, 90_000, 7_200, 93_600):
    raise SystemExit("pipeline/profile/service timeout contract changed")
if not pipeline < build < service:
    raise SystemExit("outer timeout layers do not preserve cleanup margin")
profile_limit = re.findall(
    r'maximum_timeout = ([0-9_]+) if name == "build" else ([0-9_]+)',
    profiles,
)
if len(profile_limit) != 1 or tuple(int(value.replace("_", "")) for value in profile_limit[0]) != (93_600, 7_200):
    raise SystemExit("profile validator timeout envelope changed")

def timer_seconds(name: str) -> int:
    match = re.findall(rf'^{name}=([0-9]+)(s|min|h)$', timer, flags=re.MULTILINE)
    if len(match) != 1:
        raise SystemExit(f"{name} is absent or ambiguous")
    value, unit_name = match[0]
    return int(value) * {"s": 1, "min": 60, "h": 3600}[unit_name]

pipeline_lifetime_match = re.findall(
    r'^PIPELINE_MAX_SECONDS = ([0-9]+) \* ([0-9]+) \* ([0-9]+)$',
    model,
    flags=re.MULTILINE,
)
if len(pipeline_lifetime_match) != 1:
    raise SystemExit("pipeline lifetime is absent or ambiguous")
pipeline_lifetime = math.prod(int(value) for value in pipeline_lifetime_match[0])
if not re.search(
    r'^INTENT_INGEST_MAX_AGE_SECONDS = PIPELINE_MAX_SECONDS$',
    model,
    flags=re.MULTILINE,
):
    raise SystemExit("intent freshness must be the exact immutable pipeline lifetime")
intent_freshness = pipeline_lifetime
maximum_poll_gap = sum(
    timer_seconds(name)
    for name in ("OnUnitActiveSec", "RandomizedDelaySec", "AccuracySec")
)
if intent_freshness != 86_400 or maximum_poll_gap != 1050:
    raise SystemExit("intent freshness or steward maximum poll gap changed")
if intent_freshness <= maximum_poll_gap:
    raise SystemExit("a fresh reflection can expire before the next steward ingest")
PY
probation_service="$REPO_ROOT/packaging/systemd/astrid-edge-self-change-probation-health.service"
probation_timer="$REPO_ROOT/packaging/systemd/astrid-edge-self-change-probation-health.timer"
grep -q '^ExecStart=/usr/bin/python3 -I -E -s /usr/libexec/astrid/edge-self-change-supervisor --config /etc/astrid/edge-self-change.json --execute check-probation$' "$probation_service" || fail "probation sampler does not use the exact isolated Python path"
! grep -q '^OnSuccess=\|astrid-edge-steward.service' "$probation_service" || fail "probation sampler can trigger reflection or another service"
grep -q '^OnActiveSec=1min$' "$probation_timer" || fail "probation timer lacks the one-minute first sample"
grep -q '^OnUnitActiveSec=285s$' "$probation_timer" || fail "probation cadence plus accuracy can exceed five minutes"
grep -q '^AccuracySec=15s$' "$probation_timer" || fail "probation timer accuracy is not bounded"
grep -q '^RandomizedDelaySec=0$' "$probation_timer" || fail "probation timer has unbounded jitter"
grep -q '^OnUnitActiveSec=15min$' "$REPO_ROOT/packaging/systemd/astrid-edge-steward.timer" || fail "steward reflection cadence changed from fifteen minutes"
grep -q '^RandomizedDelaySec=2min$' "$REPO_ROOT/packaging/systemd/astrid-edge-steward.timer" || fail "steward reflection coalescing changed"
grep -q 'contains astrid-edge-self-change-probation-health.timer "${enable_units\[@\]}"' "$INSTALLER" || fail "installer does not require the probation timer to be enabled"
grep -q 'systemctl start astrid-edge-self-change-probation-health.timer' "$INSTALLER" || fail "authority activation omits the probation timer"
[[ $(grep -c 'systemctl stop astrid-edge-self-change-probation-health.timer' "$INSTALLER") -ge 2 ]] || fail "probation timer is not stopped in both signal and health-failure rollback paths"
grep -q 'contains "$CORE_LIVENESS_PATH_UNIT" "${enable_units\[@\]}"' "$INSTALLER" || fail "installer does not require the core-liveness watcher to be enabled"
grep -q 'systemctl start "$CORE_LIVENESS_PATH_UNIT"' "$INSTALLER" || fail "authority activation omits the core-liveness watcher"
[[ $(grep -c 'systemctl stop "$CORE_LIVENESS_PATH_UNIT"' "$INSTALLER") -ge 2 ]] || fail "core-liveness watcher is not stopped in both signal and health-failure rollback paths"
grep -Eq '^for command in .* setpriv( |;)' "$INSTALLER" || fail "live bootstrap does not preflight the capability-dropping DAC probe dependency"
grep -q 'chmod 0770 "$self_change_liveness_root"' "$INSTALLER" || fail "core-liveness request directory lacks the exact exclusive-group DAC"
grep -q 'install -m 0640 -o "$runtime_user" -g "$runtime_group" /dev/null "$liveness_dac_probe"' "$INSTALLER" || fail "core-liveness DAC probe does not match runtime request identity"
grep -q 'setpriv --reuid=0 --regid="$runtime_gid" --clear-groups --bounding-set=-all' "$INSTALLER" || fail "core-liveness oneshot DAC is not tested without capabilities"
grep -q -- '--ollama-binary ABS --ollama-binary-sha256 HEX64' "$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system" || fail "migrator CLI does not require an exact Ollama binary and digest"
grep -q 'ExecStartPre=/usr/bin/sha256sum --check --strict --status %s' "$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system" || fail "root Ollama unit lacks its immutable per-start digest gate"
grep -q 'ExecStart=%s serve' "$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system" || fail "root Ollama unit does not override the profile-specific executable"
grep -q 'readonly OLLAMA_RUNTIME_DIGEST=/etc/astrid/edge-ollama-runtime.sha256' "$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system" || fail "root Ollama digest contract path is not fixed"
! grep -q 'BindReadOnlyPaths=%s/.local/bin/ollama' "$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system" || fail "root migration retains the AVADO-only Ollama bind"
grep -q '"schema": "astrid.edge.service_manager.v2"' "$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system" || fail "service-manager receipt omits the pinned Ollama layout"
grep -q 'readonly OPERATOR_REPORT_ROOT=/usr/libexec/astrid-edge/operator' "$INSTALLER" || fail "immutable operator report install root is not fixed"
grep -q '/usr/bin/python3 -I -E -s \$OPERATOR_REPORT_ROOT/\$body' "$INSTALLER" || fail "operator launchers do not isolate Python from mutable imports"
grep -q '/usr/bin/sha256sum --check --strict --status \$OPERATOR_REPORT_MANIFEST' "$INSTALLER" || fail "operator launchers do not verify their immutable report tree"
grep -q 'chattr +i -- "$wrapper"' "$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system" || fail "home report wrappers are replaceable by the runtime identity"
grep -q '! runuser -u "$runtime_user" -- rm -f -- "$wrapper"' "$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system" || fail "home report wrapper immutability is not adversarially proven"
grep -q 'IMMUTABLE_OPERATOR_ROOT = Path("/usr/libexec/astrid-edge/operator")' "$REPO_ROOT/scripts/astrid_at_a_glance.py" || fail "at-a-glance does not use the immutable report root"
! grep -q 'home / ".astrid-icp/state/bin"\|home / ".astrid/bin"' "$REPO_ROOT/scripts/astrid_at_a_glance.py" || fail "at-a-glance can still execute runtime-writable state/bin code"
grep -q 'TRUSTED_COMMANDS = {' "$REPO_ROOT/scripts/report_edge_appliance.py" || fail "operator report subprocesses retain mutable PATH resolution"
grep -q 'command_environment\["PATH"\] = "/usr/bin:/bin"' "$REPO_ROOT/scripts/report_edge_appliance.py" || fail "operator report command PATH is not fixed"
grep -q 'astrid-edge-self-change-probation-health.service|astrid-edge-steward.service|astrid-edge-generation-guard.service' "$INSTALLER" || fail "probation sampler lacks the immutable namespace drop-in"
grep -q 'ExecStart=.*/astrid-edge-steward-helper --config /etc/astrid/edge-steward-helper.json --credential-directory /run/credentials/astrid-edge-steward.service' "$REPO_ROOT/packaging/systemd/astrid-edge-steward.service" || fail "native unit CLI or systemd-249 credential path mismatch"
grep -q '^LoadCredential=source.key:/etc/astrid/edge-self-evolution-source.key$' "$REPO_ROOT/packaging/systemd/astrid-edge-steward.service" || fail "source key credential absent"
grep -q '^LoadCredential=intent.key:/etc/astrid/edge-self-evolution-intent.key$' "$REPO_ROOT/packaging/systemd/astrid-edge-steward.service" || fail "intent key credential absent"
grep -q '^LoadCredential=web-broker-request.key:/etc/astrid-edge-self-change/steward-web-request.key$' "$REPO_ROOT/packaging/systemd/astrid-edge-steward.service" || fail "steward request credential absent"
grep -q '^LoadCredential=web-broker-response.pub:/etc/astrid-edge-self-change/web-response.pub$' "$REPO_ROOT/packaging/systemd/astrid-edge-steward.service" || fail "steward response verification credential absent"
! grep -q 'runtime-web-request.key\|web-response-signing.key' "$REPO_ROOT/packaging/systemd/astrid-edge-steward.service" || fail "steward receives another principal's request or signing secret"
grep -q '^CapabilityBoundingSet=$' "$REPO_ROOT/packaging/systemd/astrid-edge-steward.service" || fail "steward retains capabilities"
grep -q '^ExecStartPre=/usr/bin/test -s /etc/astrid/edge-rescue-helper.json$' "$REPO_ROOT/packaging/systemd/astrid-edge-generation-guard.service" || fail "generation guard does not fail on missing/empty verifier config"
grep -q '^ExecStartPre=/usr/bin/test -x /usr/libexec/astrid/astrid-edge-rescue-helper$' "$REPO_ROOT/packaging/systemd/astrid-edge-generation-guard.service" || fail "generation guard does not fail on missing/non-executable verifier"
grep -q '^ExecStartPre=/usr/libexec/astrid/astrid-edge-rescue-helper --config /etc/astrid/edge-rescue-helper.json reflection-reconcile$' "$REPO_ROOT/packaging/systemd/astrid-edge-generation-guard.service" || fail "generation guard does not reconcile stale reflection admission"
! grep -q '^Condition\|^Assert' "$REPO_ROOT/packaging/systemd/astrid-edge-generation-guard.service" || fail "generation guard prerequisite can be skipped or uses an unsupported assertion"
handoff_path_unit="$REPO_ROOT/packaging/systemd/astrid-edge-self-change-inbox.path.in"
grep -q '^PathExistsGlob=@@INBOX_ROOT@@/candidate-ready-\*\.json$' "$handoff_path_unit" || fail "candidate publication lacks an immediate immutable handoff watcher"
! grep -q '\.pending' "$handoff_path_unit" || fail "supervisor watcher can fire before immutable reflection cleanup"
grep -q '^Unit=astrid-edge-self-change-supervisor.service$' "$handoff_path_unit" || fail "candidate handoff watcher does not target the immutable supervisor"
grep -q 'trigger_only_no_candidate_or_deployment_authority' "$REPO_ROOT/services/astrid-edge-steward-helper/src/runner.rs" || fail "steward handoff marker is not explicitly non-authorizing"
grep -q 'candidate-ready-{}\.pending' "$REPO_ROOT/services/astrid-edge-steward-helper/src/runner.rs" || fail "mutable steward does not publish an inert pending handoff"
grep -q 'READY_PENDING_NAME' "$REPO_ROOT/scripts/edge_self_change/profiles.py" || fail "supervisor does not explicitly recognize inert pending handoffs"
grep -q 'must not read, quarantine' "$REPO_ROOT/scripts/edge_self_change/profiles.py" || fail "pending handoff ownership boundary is not reviewable"
/usr/bin/python3 -I -E -s - "$REPO_ROOT/services/astrid-edge-rescue-helper/src/reflection.rs" <<'PY' || fail "root reflection cleanup does not precede supervisor handoff promotion"
import sys
from pathlib import Path

text = Path(sys.argv[1]).read_text(encoding="utf-8")
start = text.index("pub fn cleanup(config: &Config)")
end = text.index("pub fn reconcile(config: &Config)", start)
body = text[start:end]
if body.index("cleanup_inner(") >= body.index("promote_pending_supervisor_handoffs("):
    raise SystemExit("watched supervisor trigger can precede reflection lease cleanup")
if "trigger_only_no_candidate_or_deployment_authority" not in text:
    raise SystemExit("root promotion does not preserve non-authorizing marker semantics")
PY
grep -q 'SELF_CHANGE_INBOX_PATH_UNIT' "$INSTALLER" || fail "candidate handoff watcher is not wired into root bootstrap"
! grep -q '/etc/astrid/edge-self-evolution-current-generation' "$INSTALLER" || fail "generation binding is incorrectly static under /etc"
grep -q 'GENERATION_FILE=$state_root/current-generation' "$INSTALLER" || fail "generation binding is not supervisor-state-owned"
grep -q 'model_lock == "$state_root/model.lock"' "$INSTALLER" || fail "model lock is not rooted in persistent immutable state"
grep -q 'maintenance_lease=$state_root/maintenance.json' "$INSTALLER" || fail "maintenance lease is not rooted in persistent immutable state"
grep -q '"maintenance_lease":"$maintenance_lease"' "$INSTALLER" || fail "steward config lacks the exact persistent maintenance lease"
grep -q '"$STEWARD_CONFIG" "$RESCUE_CONFIG" "$GENERATION_FILE" "$state_root"' "$INSTALLER" || fail "steward namespace lacks its root reflection config or persistent maintenance root"
grep -q -- '--maintenance-lease "$maintenance_lease" --authority-env "$AUTHORITY_ENV"' "$INSTALLER" || fail "root migration lacks exact maintenance/authority inputs"
grep -q 'readonly MODEL_LOCK_GROUP=astrid-edge-model-lock' "$INSTALLER" || fail "dedicated model-lock group absent"
grep -q 'install -m 0640 -o root -g "$MODEL_LOCK_GROUP" /dev/null "$model_lock"' "$INSTALLER" || fail "read-only shared model-lock identity absent"
grep -q '^Group=@@RUNTIME_GROUP@@$' "$REPO_ROOT/packaging/systemd/astrid-edge-steward.service" || fail "steward output group is not rendered from the appliance runtime group"
grep -q '^SupplementaryGroups=astrid-edge-steward astrid-edge-model-lock astrid-edge-provider-steward-client$' "$REPO_ROOT/packaging/systemd/astrid-edge-steward.service" || fail "steward lacks its own DAC, model-lock, and provider-client groups"
grep -q 'SupplementaryGroups=astrid-edge-provider-runtime-client %s' "$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system" || fail "core lacks its dedicated provider and web-client groups"
! grep -q 'SupplementaryGroups=astrid-edge-model-lock.*provider-runtime\|SupplementaryGroups=astrid-edge-provider-runtime.*model-lock' "$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system" || fail "mutable core retains direct model-lock authority"
grep -q 'getfacl -R -p' "$INSTALLER" || fail "recursive steward ACL changes lack a recursive rollback image"
grep -q 'archive path/type/count escaped fixed policy' "$INSTALLER" || fail "archive extraction lacks canonical metadata validation"
grep -q 'stable_stage_file "$initial_generation_bundle"' "$INSTALLER" || fail "privileged archive consumption lacks stable root capture"
grep -q 'changed during stable capture' "$INSTALLER" || fail "stable input capture lacks post-copy identity check"
grep -q 'rel != canonical' "$INSTALLER" || fail "capsule archive aliases are not rejected before install"
grep -q 'shared_component.read_bytes() != component' "$INSTALLER" || fail "capsule BLAKE3 identity is not bound to exact archive bytes"
grep -q 'require_root_directory.*immutable destination parent' "$INSTALLER" || fail "immutable roots are not anchored below root-controlled parents"
/usr/bin/python3 -I -E -s - "$INSTALLER" <<'PY' || fail "root bootstrap does not enumerate the exact twenty-capsule cognition graph"
import ast, sys
from pathlib import Path

text = Path(sys.argv[1]).read_text(encoding="utf-8")
start = text.index("capsules = {", text.index("required = {")) + len("capsules = ")
end = text.index("\narchive_names =", start)
capsules = ast.literal_eval(text[start:end])
expected = {
    "astrid-capsule-agents", "astrid-capsule-cli", "astrid-capsule-context-engine",
    "astrid-capsule-edge-context", "astrid-capsule-edge-introspector",
    "astrid-capsule-edge-spectral", "astrid-capsule-fs", "astrid-capsule-hook-bridge",
    "astrid-capsule-http", "astrid-capsule-identity", "astrid-capsule-memory",
    "astrid-capsule-openai-compat", "astrid-capsule-prompt-builder",
    "astrid-capsule-react", "astrid-capsule-registry", "astrid-capsule-router",
    "astrid-capsule-session", "astrid-capsule-shell", "astrid-capsule-skills",
    "astrid-capsule-system",
}
if capsules != expected or len(capsules) != 20:
    raise SystemExit("exact cognition inventory mismatch")
for required in (
    "astrid-capsule-openai-compat", "astrid-capsule-react",
    "astrid-capsule-prompt-builder", "astrid-capsule-session",
):
    if required not in capsules:
        raise SystemExit(f"required cognition hop absent: {required}")
PY
grep -q '"maximum_output_tokens":512' "$INSTALLER" || fail "provider global output ceiling is not 512 tokens"
grep -q '"runtime":.*"maximum_output_tokens":$output_tokens' "$INSTALLER" || fail "runtime provider output ceiling does not bind the appliance profile"
grep -q '"steward":.*"maximum_output_tokens":$reflection_output_tokens' "$INSTALLER" || fail "scheduled reflection provider output ceiling does not bind the rich profile"
grep -q '"warmup":.*"maximum_output_tokens":2' "$INSTALLER" || fail "warmup provider canary output ceiling is not two tokens"
grep -q '"$active_generation_root/astrid" --format json status' "$MIGRATOR" || fail "root migrator does not query structured cognition status"
grep -q 'len(loaded) != 20' "$MIGRATOR" || fail "root migrator does not require exactly twenty loaded capsules"
grep -q 'astrid-capsule-openai-compat.*astrid-capsule-react' "$MIGRATOR" || fail "root migrator omits provider/ReAct cognition checks"
grep -q 'astrid-capsule-prompt-builder.*astrid-capsule-session' "$MIGRATOR" || fail "root migrator omits prompt/session cognition checks"
grep -q 'core PID changed during cognition graph verification' "$MIGRATOR" || fail "root migrator does not recheck the core PID after cognition validation"
grep -q 'core restarted during cognition graph verification' "$MIGRATOR" || fail "root migrator does not recheck NRestarts after cognition validation"
for script in warm_ollama_model.sh report_edge_appliance.py report_edge_appliance.sh report_edge_activity.py report_edge_fleet_activity.py edge_hindsight.py astrid_at_a_glance.py astrid_train.py retire_edge_origin_mac_affordance.py; do
    grep -q "scripts/$script" "$INSTALLER" || fail "initial generation omits executable $script"
done
grep -q 'origin_mac_retirement_root=/var/lib/astrid-edge-origin-mac-retirement' "$INSTALLER" \
    || fail "AVADO origin-mac correction is not below immutable /var/lib ancestry"
grep -q 'origin_mac_retirement_root=/media/data/.astrid-edge-origin-mac-retirement' "$INSTALLER" \
    || fail "ICP origin-mac correction is not directly below the root-controlled SSD mount"
grep -q -- '--retirement-root "$origin_mac_retirement_root"' "$INSTALLER" \
    || fail "origin-mac migration lacks its exact root-controlled retirement binding"
grep -q 'origin_mac_correction_committed=true' "$INSTALLER" \
    || fail "outer bootstrap does not record the independently durable correction boundary"
grep -q 'preserving the independently committed origin-mac correction and canonical receipt' "$INSTALLER" \
    || fail "outer rollback does not state its durable correction behavior"
grep -q 'origin-mac durable transaction member identity is invalid' "$INSTALLER" \
    || fail "root bootstrap does not verify the canonical transaction and receipt identities"
! grep -q 'created_paths+=("$origin_mac_retirement_root")' "$INSTALLER" \
    || fail "outer rollback can erase an independently committed origin-mac correction"
grep -q '^PrivateNetwork=yes$' "$REPO_ROOT/packaging/systemd/astrid-edge-steward.service" || fail "steward lacks a private empty network namespace"
grep -q '^RestrictAddressFamilies=AF_UNIX$' "$REPO_ROOT/packaging/systemd/astrid-edge-steward.service" || fail "steward can open non-Unix sockets"
! grep -q '^IPAddressAllow=localhost$' "$REPO_ROOT/packaging/systemd/astrid-edge-steward.service" || fail "steward still advertises direct loopback authority"
if rg -n '^BindsTo=astrid-edge-web-broker-(core|runtime|steward)\.(service|socket)$' \
    "$REPO_ROOT/packaging/systemd/astrid-edge-steward.service" \
    "$REPO_ROOT/packaging/systemd/root/astrid-edge-runtime-self-evolution.conf.in" \
    "$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system"; then
    fail "broker failure can propagate a non-restarting dependency stop"
fi
for principal in core runtime steward; do
    service="$REPO_ROOT/packaging/systemd/astrid-edge-web-broker-$principal.service"
    socket="$REPO_ROOT/packaging/systemd/astrid-edge-web-broker-$principal.socket.in"
    ! grep -q '^ExecStartPre=' "$service" || fail "$principal broker retains a helper process outside its deny-all execution envelope"
    grep -q "^ExecStart=/usr/libexec/astrid-edge/immutable/astrid-edge-web-broker --config /etc/astrid-edge-self-change/web-broker-$principal.json$" "$service" || fail "$principal broker does not invoke its exact self-validating config path"
    grep -q '^NoExecPaths=/$' "$service" || fail "$principal broker can execute ambient host programs"
    grep -q '^ExecPaths=/usr/libexec/astrid-edge/immutable/astrid-edge-web-broker /usr/lib /lib /lib64$' "$service" || fail "$principal broker execution allowlist is not exact"
    grep -q "^ReadOnlyPaths=/etc/astrid-edge-self-change/web-broker-$principal.json$" "$service" || fail "$principal broker config is not read-only"
    grep -q '^LoadCredential=request.key:/etc/astrid-edge-self-change/' "$service" || fail "$principal broker request credential is absent"
    grep -q '^LoadCredential=response-signing.key:/etc/astrid-edge-self-change/web-response-signing.key$' "$service" || fail "$principal broker signing credential is absent"
    ! grep -q '^Condition\|^Assert' "$service" || fail "$principal broker prerequisite can be skipped or uses an unsupported assertion"
    grep -q '^StandardInput=socket$' "$service" || fail "$principal broker does not consume exactly its activated listener"
    grep -q '^SocketUser=root$' "$socket" || fail "$principal broker socket is not root-owned"
    grep -q '^SocketMode=0660$' "$socket" || fail "$principal broker socket lacks exact client-group mode"
    grep -q '^DirectoryMode=0755$' "$socket" || fail "$principal broker socket parent is not immutable and mutually traversable"
done
for principal in core runtime steward; do
    service="$REPO_ROOT/packaging/systemd/astrid-edge-web-broker-$principal.service"
    for foreign in core runtime steward; do
        [[ $foreign == "$principal" ]] && continue
        ! grep -q "$foreign-web-request.key" "$service" || fail "$principal broker receives $foreign request authority"
    done
    for foreign_state in core runtime steward; do
        [[ $foreign_state == "$principal" ]] && continue
        grep -q "InaccessiblePaths=.*astrid-edge-web-$foreign_state" "$service" || fail "$principal broker can inspect $foreign_state quota state"
    done
done
if rg -n '^Assert(FileNotEmpty|FileIsExecutable|PathIsRegular|PathIsExecutable)=' "$REPO_ROOT/packaging/systemd"; then
    fail "unsupported systemd file assertion directive remains"
fi
if rg -n '%d' "$REPO_ROOT/packaging/systemd"; then
    fail "systemd-250 credential-directory specifier remains in Ubuntu 22.04 appliance units"
fi
grep -q 'broker_resolver_output=.*\/usr/bin/python3 -I -E -s - /etc/resolv.conf' "$INSTALLER" || fail "broker DNS allowlist is not derived with the exact isolated Python interpreter"
if rg -n '(^|[[:space:]])python3[[:space:]]+-' "$INSTALLER" "$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system" | grep -v '/usr/bin/python3 -I -E -s -'; then
    fail "bare Python inline invocation remains in a privileged installer path"
fi
grep -q 'astrid-edge-web-broker-core.service|astrid-edge-web-broker-runtime.service|astrid-edge-web-broker-steward.service)' "$INSTALLER" || fail "broker processes have no generated private-root boundary"
grep -q '^Requires=astrid-edge-web-broker-runtime.socket$' "$REPO_ROOT/packaging/systemd/root/astrid-edge-runtime-self-evolution.conf.in" || fail "runtime does not require its authenticated broker socket at startup"
grep -q '^LoadCredential=web-broker-request.key:@@RUNTIME_WEB_REQUEST_KEY@@$' "$REPO_ROOT/packaging/systemd/root/astrid-edge-runtime-self-evolution.conf.in" || fail "runtime request credential template is absent"
grep -q '^LoadCredential=web-broker-response.pub:@@WEB_RESPONSE_VERIFY_KEY@@$' "$REPO_ROOT/packaging/systemd/root/astrid-edge-runtime-self-evolution.conf.in" || fail "runtime response verification credential template is absent"
grep -q '^Environment=ASTRID_EDGE_WEB_BROKER_REQUEST_KEY_PATH=/run/credentials/astrid-edge-runtime.service/web-broker-request.key$' "$REPO_ROOT/packaging/systemd/root/astrid-edge-runtime-self-evolution.conf.in" || fail "runtime request credential path is not systemd-249 compatible"
grep -q '^Environment=ASTRID_EDGE_WEB_BROKER_RESPONSE_VERIFY_KEY_PATH=/run/credentials/astrid-edge-runtime.service/web-broker-response.pub$' "$REPO_ROOT/packaging/systemd/root/astrid-edge-runtime-self-evolution.conf.in" || fail "runtime response credential path is not systemd-249 compatible"
grep -q '^BindReadOnlyPaths=@@SCHEDULED_AUTHORSHIP_ROOT@@:/run/astrid-edge-self-change/scheduled-authorship$' "$REPO_ROOT/packaging/systemd/root/astrid-edge-runtime-self-evolution.conf.in" || fail "runtime lacks the exact persistent scheduled-authorship projection"
grep -q '^LoadCredential=scheduled-authorship.pub:@@SCHEDULED_AUTHORSHIP_VERIFY_KEY@@$' "$REPO_ROOT/packaging/systemd/root/astrid-edge-runtime-self-evolution.conf.in" || fail "runtime scheduled-authorship verifier credential is absent"
grep -q '^Environment=ASTRID_EDGE_SCHEDULED_AUTHORSHIP_ATTESTATION_PATH=/run/astrid-edge-self-change/scheduled-authorship/current.json$' "$REPO_ROOT/packaging/systemd/root/astrid-edge-runtime-self-evolution.conf.in" || fail "runtime scheduled-authorship path is not exact"
grep -q '^Environment=ASTRID_EDGE_SCHEDULED_AUTHORSHIP_VERIFY_KEY_PATH=/run/credentials/astrid-edge-runtime.service/scheduled-authorship.pub$' "$REPO_ROOT/packaging/systemd/root/astrid-edge-runtime-self-evolution.conf.in" || fail "runtime scheduled-authorship credential path is not systemd-249 compatible"
grep -q '^Environment=ASTRID_EDGE_SCHEDULED_AUTHORSHIP_VERIFY_KEY_SHA256=@@SCHEDULED_AUTHORSHIP_VERIFY_KEY_SHA256@@$' "$REPO_ROOT/packaging/systemd/root/astrid-edge-runtime-self-evolution.conf.in" || fail "runtime does not bind the scheduled-authorship verifier digest"
grep -q '^Environment=ASTRID_EDGE_SCHEDULED_AUTHORSHIP_STEWARD_UID=@@SCHEDULED_AUTHORSHIP_STEWARD_UID@@$' "$REPO_ROOT/packaging/systemd/root/astrid-edge-runtime-self-evolution.conf.in" || fail "runtime does not bind the scheduled-authorship producer UID"
! grep -q '^ConditionPathExists=.*scheduled-authorship/current.json$' "$REPO_ROOT/packaging/systemd/root/astrid-edge-runtime-self-evolution.conf.in" || fail "fresh boot incorrectly requires a pre-existing scheduled-authorship record"
grep -Fq 'install -d -m 0750 -o "$STEWARD_USER" -g "$runtime_group" "$scheduled_authorship_root"' "$INSTALLER" || fail "persistent scheduled-authorship root has the wrong identity"
grep -q -- '--print-scheduled-authorship-verifying-key' "$INSTALLER" || fail "installer does not derive the domain-separated public verifier"
grep -q 'O_EXCL.*O_NOFOLLOW' "$INSTALLER" || fail "scheduled-authorship verifier is not create-once and no-follow"
grep -q 'scheduled-authorship public key write made no progress' "$INSTALLER" || fail "scheduled-authorship verifier write loop can stall"
for scheduled_substitution in SCHEDULED_AUTHORSHIP_ROOT SCHEDULED_AUTHORSHIP_VERIFY_KEY SCHEDULED_AUTHORSHIP_VERIFY_KEY_SHA256 SCHEDULED_AUTHORSHIP_STEWARD_UID; do
    grep -q "@@$scheduled_substitution@@" "$INSTALLER" || fail "runtime rendering omits $scheduled_substitution"
done
grep -q 'scheduled-authorship root is not steward:runtime mode 0750' "$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system" || fail "root migration does not revalidate scheduled-authorship ownership"
grep -q 'scheduled-authorship public key identity is invalid' "$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system" || fail "root migration does not revalidate the scheduled-authorship verifier"
grep -q 'Requires=astrid-edge-provider-runtime.socket %s' "$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system" || fail "root-migrated core does not require its immutable web socket"
grep -q 'LoadCredential=web-request.key:%s' "$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system" || fail "root-migrated core lacks its isolated web request credential"
grep -q 'LoadCredential=web-response.pub:%s' "$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system" || fail "root-migrated core lacks the broker response verifier"
for core_web_env in ASTRID_EDGE_CORE_WEB_BROKER_SOCKET ASTRID_EDGE_CORE_WEB_BROKER_REQUEST_CREDENTIAL ASTRID_EDGE_CORE_WEB_BROKER_RESPONSE_CREDENTIAL ASTRID_EDGE_CORE_WEB_BROKER_RESPONSE_KEY_SHA256; do
    grep -q "Environment=$core_web_env=" "$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system" || fail "root-migrated core web binding absent: $core_web_env"
done
grep -q 'core web-broker authority leaked into persistent account membership' "$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system" || fail "core web authority is not constrained to per-service supplementary groups"
grep -q 'core web-broker socket ownership or mode escaped its dedicated client group' "$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system" || fail "migration does not attest the live core web socket DAC"
grep -q '^Environment=ASTRID_EDGE_HOST_NETWORK_POLICY=unavailable_private_network$' "$REPO_ROOT/packaging/systemd/root/astrid-edge-runtime-self-evolution.conf.in" || fail "runtime template can mislabel private-namespace loopback counters as host network activity"
grep -q "printf 'Environment=ASTRID_EDGE_HOST_NETWORK_POLICY=unavailable_private_network" "$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system" || fail "effective root runtime policy does not declare host network unavailable"
grep -q '^ProtectProc=invisible$' "$REPO_ROOT/packaging/systemd/root/astrid-edge-runtime-self-evolution.conf.in" || fail "runtime template exposes unrelated process metadata"
grep -q '^ProcSubset=all$' "$REPO_ROOT/packaging/systemd/root/astrid-edge-runtime-self-evolution.conf.in" || fail "runtime template cannot read bounded host sensing data"
mutable_runtime_template="$REPO_ROOT/packaging/systemd/root/astrid-edge-runtime-self-evolution.conf.in"
grep -q '^TemporaryFileSystem=/var:ro,nosuid,nodev,noexec,size=4M,mode=0755$' "$mutable_runtime_template" || fail "mutable edge runtime can inspect host /var"
for hidden_host_path in /etc/ssh /etc/sudoers /etc/sudoers.d /etc/polkit-1 /etc/apt /etc/dpkg /etc/ufw /etc/nftables.conf /etc/NetworkManager /etc/netplan /etc/fstab /etc/crypttab /etc/default/grub /etc/ssl/private /etc/systemd /boot /usr/local /usr/src; do
    grep -Eq "^InaccessiblePaths=.*-?${hidden_host_path//\//\\/}([[:space:]]|$)" "$mutable_runtime_template" || fail "mutable edge namespace exposes $hidden_host_path"
    grep -Fq -- "-$hidden_host_path" "$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system" || fail "root-migrated core/edge namespace exposes $hidden_host_path"
done
grep -Fq "printf 'TemporaryFileSystem=/var:ro,nosuid,nodev,noexec,size=4M,mode=0755\\n'" "$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system" || fail "root-migrated core can inspect host /var"
! grep -Eq 'TemporaryFileSystem=/proc|TemporaryFileSystem=/sys|InaccessiblePaths=.*(^|[[:space:]])-?/(proc|sys)([[:space:]]|$)' "$mutable_runtime_template" "$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system" || fail "mutable namespace hides required /proc or /sys sensing"
grep -q '^NoExecPaths=/$' "$REPO_ROOT/packaging/systemd/root/astrid-edge-runtime-self-evolution.conf.in" || fail "mutable edge runtime can execute ambient files"
grep -q '^ExecPaths=@@ACTIVE_GENERATION_ROOT@@/astrid-edge-runtime @@ACTIVE_GENERATION_ROOT@@/astrid /usr/lib /usr/lib64 /lib /lib64$' "$REPO_ROOT/packaging/systemd/root/astrid-edge-runtime-self-evolution.conf.in" || fail "runtime template execution allowlist is broader than its exact binaries and loader roots"
! grep -Eq '^ExecPaths=.*(^|[[:space:]])(/bin|/usr/bin|@@ACTIVE_GENERATION_ROOT@@)([[:space:]]|$)' "$REPO_ROOT/packaging/systemd/root/astrid-edge-runtime-self-evolution.conf.in" || fail "runtime template can execute a shell, host tool directory, or whole generation"
! grep -q '^BindReadOnlyPaths=@@ACTIVE_GENERATION_ROOT@@$\|^ReadOnlyPaths=@@ACTIVE_GENERATION_ROOT@@$' "$REPO_ROOT/packaging/systemd/root/astrid-edge-runtime-self-evolution.conf.in" || fail "runtime template can inspect the whole active generation"
! grep -q 'STEWARD_WEB_REQUEST\|WEB_RESPONSE_SIGNING' "$REPO_ROOT/packaging/systemd/root/astrid-edge-runtime-self-evolution.conf.in" || fail "runtime template receives another principal's request or signing secret"
grep -q '"client_id":"edge-runtime".*"expected_peer_uid":\$runtime_uid.*"request_key_path":"/run/credentials/\$WEB_RUNTIME_SERVICE_UNIT/request.key".*"socket_gid":\$web_runtime_client_gid.*"socket_path":"\$WEB_RUNTIME_SOCKET"' "$INSTALLER" || fail "runtime broker config does not bind its peer, dedicated key group, and socket"
grep -q '"client_id":"edge-steward".*"expected_peer_uid":\$steward_uid.*"request_key_path":"/run/credentials/\$WEB_STEWARD_SERVICE_UNIT/request.key".*"socket_gid":\$steward_gid.*"socket_path":"\$WEB_STEWARD_SOCKET"' "$INSTALLER" || fail "steward broker config does not bind its peer, key, group, and socket"
grep -q '"client_id":"edge-core".*"expected_peer_uid":\$runtime_uid.*"request_key_path":"/run/credentials/\$WEB_CORE_SERVICE_UNIT/request.key".*"socket_gid":\$web_core_client_gid.*"socket_path":"\$WEB_CORE_SOCKET"' "$INSTALLER" || fail "core broker config does not bind its peer, dedicated key group, and socket"
grep -q 'response_signing_key_path.*response-signing.key.*response_signing_key_sha256.*\$web_response_signing_sha256' "$INSTALLER" || fail "broker configs do not bind the signing seed"
grep -q 'request_key_path.*\$STEWARD_WEB_REQUEST_KEY.*request_key_sha256.*\$steward_web_request_sha256.*response_verify_key_path.*\$WEB_RESPONSE_VERIFY_KEY' "$INSTALLER" || fail "steward config does not bind its isolated web credentials"
grep -q 'create_private_random_key "$CORE_WEB_REQUEST_KEY"' "$INSTALLER" || fail "core web key is not generated with no-replace semantics"
grep -q 'create_private_random_key "$RUNTIME_WEB_REQUEST_KEY"' "$INSTALLER" || fail "runtime web key is not generated with no-replace semantics"
grep -q 'create_private_random_key "$STEWARD_WEB_REQUEST_KEY"' "$INSTALLER" || fail "steward web key is not generated with no-replace semantics"
grep -q -- '--key-init' "$INSTALLER" || fail "broker response keypair is not initialized by the immutable broker"
secret_key_block=$(sed -n '/^secret_key_hashes=(/,/^)/p' "$INSTALLER")
for key_hash in 'sha_file "$SUPERVISOR_KEY"' 'sha_file "$INTENT_KEY"' 'sha_file "$SOURCE_KEY"' ledger_attestation_sha256 core_web_request_sha256 runtime_web_request_sha256 steward_web_request_sha256 web_response_signing_sha256 runtime_provider_sha256 steward_provider_sha256 warmup_provider_sha256 provider_ledger_sha256; do
    grep -Fq "$key_hash" <<<"$secret_key_block" || fail "secret uniqueness inventory omits $key_hash"
done
! grep -q 'web_response_verify_sha256' <<<"$secret_key_block" || fail "derived public verify key entered the secret uniqueness inventory"
grep -q '\${#secret_key_hashes\[@\]} == 12' "$INSTALLER" || fail "secret uniqueness inventory has no exact cardinality assertion"
grep -q 'sort -u.*secret_key_hashes' "$INSTALLER" || fail "all secret trust domains are not proven pairwise distinct"
grep -q 'readonly LEDGER_ATTESTATION_KEY=/etc/astrid-edge-self-change/keys/ledger-attestation.key' "$INSTALLER" || fail "dedicated lifecycle-ledger key path is absent"
grep -q 'create_private_random_key "$LEDGER_ATTESTATION_KEY" "root lifecycle ledger attestation key"' "$INSTALLER" || fail "lifecycle-ledger key is not created with no-replace semantics"
grep -q 'ledger_attestation_key":"$LEDGER_ATTESTATION_KEY"' "$INSTALLER" || fail "rescue configuration omits the dedicated lifecycle-ledger key"
grep -q '"$ledger_attestation_sha256"' "$INSTALLER" || fail "lifecycle-ledger key is excluded from cross-domain collision checks"
grep -q '"telemetry_addr":"127.0.0.1:7878"' "$INSTALLER" || fail "rescue health does not bind exact host-loopback telemetry"
grep -q 'audio_policy=required_fresh_numeric' "$INSTALLER" || fail "AVADO rescue health does not require fresh numeric audio"
grep -q 'expected_audio_source=physical_alsa_numeric_feeder:default:16000hz:1ch' "$INSTALLER" || fail "AVADO rescue health does not bind the immutable numeric ALSA feeder source"
grep -q 'audio_policy=required_unavailable' "$INSTALLER" || fail "ICP rescue health does not require explicit audio unavailability"
grep -q 'expected_audio_source=unavailable_no_audio_input' "$INSTALLER" || fail "ICP rescue health does not bind the unavailable audio source"
grep -q '"audio_policy":"$audio_policy","expected_audio_source":"$expected_audio_source"' "$INSTALLER" || fail "profile health policy is absent from the immutable rescue config"
grep -q '^EnvironmentFile=/etc/astrid/edge-self-change-authority.env$' "$REPO_ROOT/packaging/systemd/root/astrid-edge-runtime-self-evolution.conf.in" || fail "runtime authority is not root-owned"
grep -q 'ASTRID_EDGE_MAINTENANCE_CORE_ACK_PATH' "$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system" || fail "core maintenance acknowledgement is not wired"
grep -q 'ASTRID_EDGE_MAINTENANCE_EDGE_ACK_PATH' "$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system" || fail "edge maintenance acknowledgement is not wired"
for exact_runtime_binding in ASTRID_EDGE_APPLIANCE_ID ASTRID_EDGE_SOCKET ASTRID_EDGE_TOKEN ASTRID_EDGE_WORKSPACE ASTRID_EDGE_ASTRID_CLI ASTRID_EDGE_SELF_CHANGE_ROOT ASTRID_EDGE_CORE_LIVENESS_REQUEST_PATH; do
    grep -q "Environment=$exact_runtime_binding=" "$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system" || fail "root runtime binding absent: $exact_runtime_binding"
done
grep -q 'Environment=ASTRID_EDGE_SCHEDULED_INTROSPECTION_ENABLED=false' "$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system" || fail "legacy runtime reflection scheduler is not immutably disabled"
grep -q 'Environment=ASTRID_EDGE_DEDICATED_STEWARD_ENABLED=true' "$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system" || fail "dedicated steward is absent from effective runtime self-profile"
grep -q -- '--source-root ABS --candidate-root ABS --builder-root ABS' "$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system" || fail "root migration cannot receive the exact private source/candidate/builder roots"
grep -q -- '--updater-root ABS --toolchain-root ABS' "$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system" || fail "root migration cannot receive the exact private updater/toolchain roots"
grep -q -- '--source-root "$source_root" --candidate-root "$candidate_root" --builder-root "$builder_root" --updater-root "$updater_root" --toolchain-root "$toolchain_root"' "$INSTALLER" || fail "bootstrap omits mutable-process private-root isolation inputs"
grep -q 'declare -a private_managed_roots=(' "$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system" || fail "mutable process private roots are not centralized"
grep -q 'astrid.service|astrid-edge-runtime.service)' "$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system" || fail "core and edge do not share immutable private-root hiding"
core_exec_block=$(sed -n '/^[[:space:]]*astrid.service)/,/^[[:space:]]*astrid-edge-runtime.service)/p' "$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system")
grep -Fq "printf 'NoExecPaths=/\nExecPaths=%s/astrid-daemon /usr/lib /usr/lib64 /lib /lib64\n'" <<<"$core_exec_block" || fail "core does not allow only its exact daemon and loader roots"
! grep -Eq "ExecPaths=.*(/bin|/usr/bin|source_root|candidate_root|builder_root|runtime_workspace)" <<<"$core_exec_block" || fail "core execution allowlist includes a shell, host tools, or mutable path"
edge_exec_block=$(sed -n '/^[[:space:]]*astrid-edge-runtime.service)/,/^[[:space:]]*astrid-edge-hindsight.service)/p' "$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system")
grep -Fq "printf 'NoExecPaths=/\nExecPaths=%s/astrid-edge-runtime %s/astrid /usr/lib /usr/lib64 /lib /lib64\n'" <<<"$edge_exec_block" || fail "edge does not allow only its exact runtime/CLI and loader roots"
! grep -Eq "ExecPaths=.*(/bin|/usr/bin|source_root|candidate_root|builder_root|runtime_workspace)" <<<"$edge_exec_block" || fail "edge execution allowlist includes a shell, host tools, or mutable path"
grep -q 'BindReadOnlyPaths=%s/astrid-daemon' <<<"$core_exec_block" || fail "core exact executable is not re-exposed below the hidden release root"
grep -q 'BindReadOnlyPaths=%s/astrid-edge-runtime.*BindReadOnlyPaths=%s/astrid' <<<"$edge_exec_block" || fail "edge exact executables are not re-exposed below the hidden release root"
grep -Fq "printf 'BindsTo=\\nWants=astrid.service\\n'" "$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system" || fail "edge runtime remains bound to core restart lifetime"
[[ $(grep -c 'Environment=ASTRID_EDGE_REFLECTION_LEASE_PATH=%s' "$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system") -eq 2 ]] || fail "core and edge do not both receive the distinct reflection lease"
grep -q "ASTRID_EDGE_REFLECTION_LEASE_PATH=/run/astrid-edge-self-change/reflection.json" "$INSTALLER" || fail "post-cutover acceptance does not verify the reflection lease"
grep -q 'atomic_authority_install "$authority_enabled_source"' "$INSTALLER" || fail "authority rail is never activated after acceptance"
grep -q 'atomic_authority_install "$authority_disabled_source"' "$INSTALLER" || fail "authority rail lacks fail-closed restoration"
grep -q '"build":{"argv":\["--config","\$RESCUE_CONFIG","build","--candidate-manifest","{candidate_manifest}","--intent-envelope","{intent_envelope}"' "$INSTALLER" || fail "immutable build profile omits the supervisor-attested intent envelope"
grep -q '"synthetic":{"argv":\["--config","\$RESCUE_CONFIG","synthetic-lifecycle"\].*"network":"deny".*"privilege_envelope":"operator-synthetic:offline-build-model-unloaded:v1".*"run_as_gid":0,"run_as_uid":0.*"timeout_seconds":7200' "$INSTALLER" || fail "synthetic lifecycle lacks its fixed root/offline/cgroup profile"
grep -q '"retention":{"argv":\["--config","\$RESCUE_CONFIG","retention"\].*"network":"deny".*"privilege_envelope":"root-retention:paired-generation-snapshot-only:v1".*"run_as_gid":0,"run_as_uid":0.*"timeout_seconds":600' "$INSTALLER" || fail "paired retention lacks its fixed root-only immutable profile"
grep -q "rescue_help == \*'--intent-envelope ABSOLUTE'\*" "$INSTALLER" || fail "bootstrap does not verify the installed rescue helper accepts the intent envelope"
grep -q "rescue_help == \*'recover-model-after-build'\*" "$INSTALLER" || fail "bootstrap does not verify the installed rescue helper accepts model recovery"
grep -q "rescue_help == \*'recover-core-liveness'\*" "$INSTALLER" || fail "bootstrap does not verify the installed rescue helper accepts core recovery"
grep -q "rescue_help == \*'health | retention | synthetic-lifecycle'\*" "$INSTALLER" || fail "bootstrap does not verify the installed rescue helper accepts paired retention"
grep -q "rescue_help == \*'reconcile-storage-reserve'\*" "$INSTALLER" || fail "bootstrap does not verify the signed storage-reserve recovery command"
state_recover_unit="$REPO_ROOT/packaging/systemd/astrid-edge-state-store-recover.service.in"
state_migration_recover_unit="$REPO_ROOT/packaging/systemd/astrid-edge-state-store-migration-recover.service.in"
grep -q '^Requires=@@RUNTIME_MOUNT_UNIT@@ @@ROLLBACK_MOUNT_UNIT@@$' "$state_recover_unit" || fail "storage-reserve recovery is not ordered after both exact mounts"
grep -q '^After=@@RUNTIME_MOUNT_UNIT@@ @@ROLLBACK_MOUNT_UNIT@@$' "$state_recover_unit" || fail "storage-reserve recovery can run before its journal and runtime volume are mounted"
grep -q '^ExecStart=/usr/libexec/astrid/astrid-edge-rescue-helper --config /etc/astrid/edge-rescue-helper.json reconcile-storage-reserve$' "$state_recover_unit" || fail "storage-reserve recovery bypasses the signed immutable reconciler"
grep -q '^NoExecPaths=/$' "$state_recover_unit" || fail "storage-reserve recovery can execute ambient host programs"
grep -q '^ExecPaths=/usr/bin/python3 /usr/bin/findmnt /usr/bin/systemd-escape /usr/bin/test /usr/sbin/blkid /usr/sbin/dumpe2fs /usr/sbin/losetup /usr/libexec/astrid/astrid-edge-state-store /usr/libexec/astrid/astrid-edge-rescue-helper /usr/lib /usr/lib64 /lib /lib64$' "$state_recover_unit" || fail "storage-reserve recovery execution allowlist is not exact"
grep -q '^DevicePolicy=closed$' "$state_recover_unit" || fail "storage-reserve recovery lacks a closed device policy"
[[ $(grep -c '^DeviceAllow=' "$state_recover_unit") -eq 1 ]] || fail "storage-reserve recovery has more than one device allowance"
grep -q '^DeviceAllow=block-loop r$' "$state_recover_unit" || fail "storage-reserve recovery device allowance is not read-only loop inspection"
! grep -Eq '^CapabilityBoundingSet=.*(SYS_ADMIN|MKNOD|SYS_RAWIO)' "$state_recover_unit" || fail "storage-reserve recovery can mount, create, or raw-write devices"
grep -q '^SystemCallFilter=~.*@mount.*@raw-io' "$state_recover_unit" || fail "storage-reserve recovery does not deny mount and raw-I/O syscall classes"
grep -q '^Before=@@RUNTIME_MOUNT_UNIT@@ @@ROLLBACK_MOUNT_UNIT@@$' "$state_migration_recover_unit" || fail "pre-mount migration recovery is not ordered before both state volumes"
grep -q '^PrivateDevices=yes$' "$state_migration_recover_unit" || fail "pre-mount migration recovery can inspect host block devices"
grep -q '^ExecStart=/usr/bin/python3 -I -E -s /usr/libexec/astrid/astrid-edge-state-store recover --config /etc/astrid/edge-state-store.json$' "$state_migration_recover_unit" || fail "pre-mount migration recovery does not use the fixed immutable profile"
grep -q '"$STATE_STORE_RECOVER_UNIT" "$STATE_STORE_VERIFY_UNIT"' "$INSTALLER" || fail "installer does not enable pre-verification storage recovery"
grep -q '"$state_store_helper" prepare --config "$state_store_config"' "$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system" || fail "operator cutover does not create the initial reserve before signed recovery becomes mandatory"
grep -q 'services/astrid-edge-steward-helper/' "$INSTALLER" || fail "steward helper is absent from the inspect-only boundary"
grep -q 'services/astrid-edge-checkpoint/' "$INSTALLER" || fail "checkpoint helper is absent from the inspect-only boundary"
grep -q '"edge-checkpoint",' "$INSTALLER" || fail "checkpoint units are absent from the inspect-only boundary"
grep -q 'services/astrid-edge-rescue-helper/' "$INSTALLER" || fail "rescue helper is absent from the inspect-only boundary"
grep -q 'services/astrid-edge-web-broker/' "$INSTALLER" || fail "web broker is absent from the inspect-only boundary"
grep -q 'INSPECT_ONLY_ORIGIN = "inspect_only_immutable_boundary"' "$INSTALLER" || fail "immutable source has no distinct inspect-only provenance"
grep -q '"scripts/astrid_train.py"' "$REPO_ROOT/services/astrid-edge-rescue-helper/src/manifest.rs" \
    || fail "immutable rescue source verifier omits the sealed inquiry viewer"
grep -q '"target":"\$target".*"active_generation_link":"\$release_parent/current"' "$INSTALLER" || fail "steward cumulative-source target/link binding is absent"
if rg -n 'launchctl|/Users/|\.ssh/' "$INSTALLER" "$REPO_ROOT/packaging/systemd/astrid-edge-self-change-supervisor.service" "$REPO_ROOT/packaging/systemd/astrid-edge-steward.service" "$REPO_ROOT/packaging/systemd/root" | grep -v -e 'no helper/supervisor execution.*launchctl' -e 'InaccessiblePaths=.*\.ssh'; then
    fail "Mac or SSH mutation surface present"
fi
grep -q 'for unit in "${STACK\[@\]}"; do userctl disable "$unit"; done' "$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system" || fail "user-unit migration is not fixed-stack bounded"
grep -q -- '--completion-marker' "$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system" || fail "nested migration has no durable commit hand-off"
grep -q 'migration_completion_marker=$stage/system-migration.complete' "$INSTALLER" || fail "outer transaction does not bind nested migration completion"
grep -q 'unit_policy=$state_root/unit-policy.json' "$INSTALLER" || fail "unit policy is not rooted in immutable supervisor state"
grep -q 'unit_transactions=$state_snapshots/unit-transactions' "$INSTALLER" || fail "unit transactions are not rooted in private updater snapshots"
grep -q 'profile_transactions=$state_snapshots/profile-transactions' "$INSTALLER" || fail "profile transitions lack their fixed private journal root"
grep -q '"$rescue_helper_install_path" --config "$RESCUE_CONFIG" profile-bootstrap' "$INSTALLER" || fail "initial generation profile is not immutably bootstrapped"
grep -q 'active_profile_env=$state_root/active-profile.env' "$INSTALLER" || fail "active profile output is not supervisor-state-bound"
grep -q "== '0 400 1'" "$INSTALLER" || fail "active profile lacks exact create-once root mode verification"
grep -Fq "printf 'EnvironmentFile=\\nEnvironmentFile=%s\\n' \"\$active_profile_env\"" "$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system" || fail "root services do not reset mutable profile files before loading the active immutable profile"
grep -q 'generation_staging=$updater_root/generation-staging' "$INSTALLER" || fail "updater generation staging root is not exact"
grep -q 'install -d -m 0700 -o "$UPDATER_USER" -g "$UPDATER_USER" "$generation_staging"' "$INSTALLER" || fail "generation staging is not owned only by updater"
grep -q 'runuser -u "$UPDATER_USER" -- test -w "$generation_staging"' "$INSTALLER" || fail "updater staging write authority is not verified"
grep -q 'system_unit_alias=$updater_root/system-units' "$INSTALLER" || fail "private system-unit alias is not rooted beneath the immutable updater"
grep -q '"system_unit_root":"$system_unit_alias","unit_policy":"$unit_policy","unit_transactions":"$unit_transactions"' "$INSTALLER" || fail "rescue config omits private transactional unit roots"
grep -q -- '--unit-policy "$unit_policy"' "$INSTALLER" || fail "root migration is not delegated exact unit-policy bootstrap authority"
grep -q -- '--rescue-system-unit-root "$system_unit_alias"' "$INSTALLER" || fail "root migration is not bound to the private system-unit alias"
grep -q 'operator_bootstrap_reviewed_immutable_dropins' "$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system" || fail "migration does not seal immutable unit drop-in policy"
grep -q 'output.open("xb")' "$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system" || fail "unit policy creation can overwrite an existing authority record"
grep -q '90-root-runtime-boundary.conf' "$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system" || fail "unit policy does not require every runtime boundary"
grep -q 'ReadWritePaths=.*\$state_snapshots.*\$release_parent' "$INSTALLER" || fail "generation guard cannot reconcile its crash journal/A-B binding"
grep -q '"\$state_snapshots" "\$release_parent" "\$system_unit_alias"' "$INSTALLER" || fail "generation guard cannot reconcile crash-partial live unit transactions"
grep -q '"\$updater_root" "\$runtime_workspace" "\$system_unit_alias"' "$INSTALLER" || fail "supervisor/probation namespaces cannot write their exact private live unit transaction root"
write_dropin_body=$(sed -n '/^write_dropin()/,/^}/p' "$INSTALLER")
[[ $(printf '%s\n' "$write_dropin_body" | grep -o '"\$system_unit_root"' | wc -l | tr -d ' ') == 0 ]] || fail "supervisor namespace still exposes the real system-unit root"
[[ $(printf '%s\n' "$write_dropin_body" | grep -o '"\$system_unit_alias"' | wc -l | tr -d ' ') == 5 ]] || fail "private unit alias authority escaped supervisor, probation, or generation guard"
grep -q 'InaccessiblePaths=.*-/etc/systemd/system' <<<"$write_dropin_body" || fail "self-change namespaces can inspect the real system-unit tree"
alias_mount_template="$REPO_ROOT/packaging/systemd/root/astrid-edge-system-units-alias.mount.in"
grep -q '^What=/etc/systemd/system$' "$alias_mount_template" || fail "private unit alias does not bind the exact live manager tree"
grep -q '^Where=@@SYSTEM_UNIT_ALIAS@@$' "$alias_mount_template" || fail "private unit alias target is not installer-bound"
grep -q '^RequiresMountsFor=@@UPDATER_ROOT@@$' "$alias_mount_template" || fail "private unit alias is not ordered after its backing root"
grep -q '^Options=bind,nosuid,nodev,noexec$' "$alias_mount_template" || fail "private unit alias lacks a non-executable bind policy"
grep -q 'runuser -u "$denied_user" -- test -r "$system_unit_alias"' "$INSTALLER" || fail "installer does not prove mutable identities cannot traverse the unit alias"
grep -q 'private rescue system-unit alias is not bound to the live unit root' "$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system" || fail "migrator does not re-attest the private alias mount"
grep -q 'readonly CANDIDATE_SANDBOX_ROOT=/usr/libexec/astrid-edge/immutable/candidate-rootfs' "$INSTALLER" || fail "candidate sandbox root is not the fixed immutable path"
grep -q 'install -d -m 0755 -o root -g root "$CANDIDATE_SANDBOX_ROOT"' "$INSTALLER" || fail "candidate sandbox root is not created inside the root transaction"
grep -q 'candidate workspace decoy ancestry is not exact mode-0555' "$INSTALLER" || fail "candidate sandbox workspace ancestry is not sealed read-only"
grep -q 'candidate workspace decoy is not exact empty mode-0555' "$INSTALLER" || fail "candidate workspace decoy identity is not attested"
grep -q 'chmod 0555 "$CANDIDATE_SANDBOX_ROOT"' "$INSTALLER" || fail "candidate sandbox root is not sealed read-only"
candidate_skeleton_block=$(sed -n '/^declare -a candidate_sandbox_directories=(/,/^)/p' "$INSTALLER")
candidate_skeleton_actual=$(printf '%s\n' "$candidate_skeleton_block" | tail -n +2 | sed '$d' | tr ' ' '\n' | sed '/^$/d' | LC_ALL=C sort)
candidate_skeleton_expected=$(printf '%s\n' bin dev etc home lib lib64 media mnt opt proc root run sbin sys tmp usr usr/bin usr/lib usr/lib64 usr/libexec usr/local usr/sbin usr/share var var/tmp | LC_ALL=C sort)
[[ $candidate_skeleton_actual == "$candidate_skeleton_expected" ]] || fail "candidate sandbox skeleton is incomplete or excessive"
grep -q 'candidate sandbox tree membership is incomplete or excessive' "$INSTALLER" || fail "installer does not attest exact candidate sandbox membership"
grep -q '"candidate_sandbox_root":"$CANDIDATE_SANDBOX_ROOT"' "$INSTALLER" || fail "rescue config omits the candidate sandbox root"
grep -q 'systemd_run_path=$(readlink -f -- /usr/bin/systemd-run)' "$INSTALLER" || fail "systemd-run path is not resolved from the exact immutable launcher"
grep -q '"systemd_run":{"path":"$systemd_run_path","sha256":"$(sha_file "$systemd_run_path")"}' "$INSTALLER" || fail "rescue config does not digest-pin systemd-run"
grep -q 'candidate_memory_max_bytes=10737418240' "$INSTALLER" || fail "AVADO candidate cgroup is not capped at 10 GiB"
grep -q 'candidate_memory_max_bytes=5368709120' "$INSTALLER" || fail "ICP candidate cgroup is not capped at 5 GiB"
grep -q 'candidate_memory_swap_max_bytes=134217728' "$INSTALLER" || fail "candidate swap ceiling exceeds 128 MiB"
grep -q 'candidate_tasks_max=256' "$INSTALLER" || fail "candidate task ceiling is not exact"
grep -q 'candidate_cpu_quota_percent=$((build_workers \* 100))' "$INSTALLER" || fail "candidate CPU quota is not bound to appliance build workers"
for candidate_policy_field in candidate_memory_max_bytes candidate_memory_swap_max_bytes candidate_tasks_max candidate_cpu_quota_percent; do
    grep -q "\"$candidate_policy_field\":\$$candidate_policy_field" "$INSTALLER" || fail "rescue policy omits $candidate_policy_field"
done
grep -Fq "printf 'ReadWritePaths=%s %s %s %s\\nReadOnlyPaths=%s\\n'" "$INSTALLER" || fail "generation guard may rewrite immutable release contents"
grep -q '"\$web_receipts" "\$introspection_receipts" "\${maintenance_core_acknowledgement%/\*}" "\${maintenance_edge_acknowledgement%/\*}"' "$INSTALLER" || fail "steward reflection guard cannot inspect exact activity or dynamic ACK parents"
grep -q '"\$source_root" "\$release_parent" "\$model_lock"' "$INSTALLER" || fail "steward reflection guard cannot verify the active executable or model lock"
grep -q 'introspection_evidence_root=$state_root/introspection-evidence' "$INSTALLER" || fail "immutable introspection evidence root is not supervisor-state-bound"
grep -q 'install -d -m 2750 -o root -g "$STEWARD_USER"' "$INSTALLER" || fail "immutable introspection evidence modes are not installed"
grep -q '"$introspection_evidence_root" "$introspection_evidence_root"' "$INSTALLER" || fail "steward lacks explicit read-only evidence projection"
grep -q 'for denied_user in "$runtime_user" "$BUILDER_USER" "$UPDATER_USER"' "$INSTALLER" || fail "mutable identities are not checked for evidence write denial"
grep -q 'install -d -m 0750 -o "$STEWARD_USER" -g "$runtime_group" "$output_root"' "$INSTALLER" || fail "scheduled steward output roots are not steward-owned and runtime-readable"
grep -q '^UMask=0027$' "$REPO_ROOT/packaging/systemd/astrid-edge-steward.service" || fail "scheduled outputs are not constrained to owner-write/group-read files"
grep -q 'setfacl -m "d:m::r-x" "$output_root"' "$INSTALLER" || fail "scheduled output defaults can grant group write authority"
grep -q 'BindPaths=/run/astrid-edge-self-change' "$INSTALLER" || fail "root reflection hooks cannot access the runtime proof boundary"
grep -q '"\$maintenance_mutex" "\$candidate_root" "\$inbox_root" "\$inquiry_history_root" "\$steward_reflection_root" "\$steward_projection_root" "\$steward_patch_outbox"' "$INSTALLER" || fail "steward namespace omits its exact schedule/mutex, inquiry history, or bounded output roots"
grep -q '"inquiry_history_root":"\$inquiry_history_root"' "$INSTALLER" || fail "steward config lacks the dedicated read-only inquiry history root"
grep -q '^ExecStartPre=+/usr/libexec/astrid/astrid-edge-rescue-helper --config /etc/astrid/edge-rescue-helper.json reflection-prepare$' "$REPO_ROOT/packaging/systemd/astrid-edge-steward.service" || fail "steward lacks root reflection admission preparation"
grep -q '^ExecStopPost=+/usr/libexec/astrid/astrid-edge-rescue-helper --config /etc/astrid/edge-rescue-helper.json reflection-cleanup$' "$REPO_ROOT/packaging/systemd/astrid-edge-steward.service" || fail "steward lacks invocation-bound reflection cleanup"
grep -q '^PrivateNetwork=yes$' "$REPO_ROOT/packaging/systemd/astrid-edge-self-change-supervisor.service" || fail "supervisor shadow build lost its isolated network namespace"
grep -q '^ProtectProc=invisible$' "$REPO_ROOT/packaging/systemd/astrid-edge-self-change-supervisor.service" || fail "builder children can enumerate unrelated host processes"
grep -q '^ProcSubset=all$' "$REPO_ROOT/packaging/systemd/astrid-edge-self-change-supervisor.service" || fail "fixed compilers cannot read aggregate CPU or memory topology"
grep -q '^ExecStartPre=/usr/libexec/astrid/astrid-edge-rescue-helper --config /etc/astrid/edge-rescue-helper.json verify-proc-isolation$' "$REPO_ROOT/packaging/systemd/astrid-edge-self-change-supervisor.service" || fail "live builder process-isolation replay is absent"
grep -q 'builder can read unrelated root process metadata' "$REPO_ROOT/services/astrid-edge-rescue-helper/src/main.rs" || fail "process-isolation replay does not fail closed"
grep -q '^RestrictAddressFamilies=AF_UNIX AF_INET$' "$REPO_ROOT/packaging/systemd/astrid-edge-self-change-supervisor.service" || fail "supervisor cannot bind the isolated IPv4 shadow reservoir"
grep -q '^MemoryDenyWriteExecute=no$' "$REPO_ROOT/packaging/systemd/astrid-edge-self-change-supervisor.service" || fail "offline Wasmtime invariant replay remains blocked by MDWE"
grep -q '^NoExecPaths=/$' "$REPO_ROOT/packaging/systemd/astrid-edge-self-change-supervisor.service" || fail "supervisor lacks default-deny file execution"
supervisor_dropin_block=$(sed -n '/astrid-edge-self-change-supervisor.service)/,/astrid-edge-self-change-probation-health.service)/p' "$INSTALLER")
grep -q "TemporaryFileSystem=/run:ro,nosuid,nodev,noexec,size=16M,mode=0755" <<<"$supervisor_dropin_block" || fail "candidate build children can inspect the host /run tree"
grep -q "BindReadOnlyPaths=/run/systemd/private" <<<"$supervisor_dropin_block" || fail "root supervisor cannot reach the exact private manager socket"
grep -q "BindPaths=/run/astrid-edge-self-change" <<<"$supervisor_dropin_block" || fail "supervisor private /run omits the bounded Astrid transaction tree"
! grep -Eq 'Bind(ReadOnly)?Paths=/run/(dbus|systemd/(journal|notify)|user)' <<<"$supervisor_dropin_block" || fail "candidate build namespace exposes D-Bus, journald, notify, or user sockets"
grep -q 'builder identity has supplementary group authority' "$INSTALLER" || fail "bootstrap does not reject builder socket-group authority"
grep -q 'builder can open the root systemd manager socket' "$INSTALLER" || fail "bootstrap does not prove builder denial on the sole host manager socket"
grep -q 'builder can open a mode-0644 canary beneath the mutable runtime workspace' "$INSTALLER" || fail "bootstrap does not prove builder read denial beneath the live workspace"
grep -q "ExecPaths=/usr/bin /usr/sbin /bin /sbin %s %s %s %s %s %s %s" <<<"$supervisor_dropin_block" || fail "supervisor cannot execute the fixed rescue children and disposable candidate outputs"
for required_exec_root in toolchain_root builder_root rescue_helper_install_path capsule_builder_install_path checkpoint_install_path systemctl_path systemd_analyze_path; do
    grep -q "\$$required_exec_root" <<<"$supervisor_dropin_block" || fail "supervisor execution boundary omits $required_exec_root"
done
! grep -q '\$source_root\|\$candidate_root\|\$runtime_workspace\|\$state_root\|\$updater_root' <<<"$(grep 'ExecPaths=' <<<"$supervisor_dropin_block")" || fail "supervisor permits candidate-supplied execution outside the disposable builder root"
grep -q '^PrivateNetwork=yes$' "$REPO_ROOT/packaging/systemd/astrid-edge-self-change-probation-health.service" || fail "probation service lacks its isolated network namespace"
grep -q '^JoinsNamespaceOf=astrid-edge-runtime.service$' "$REPO_ROOT/packaging/systemd/astrid-edge-self-change-probation-health.service" || fail "probation service does not join the exact edge-runtime namespace"
grep -q '^IPAddressDeny=any$' "$REPO_ROOT/packaging/systemd/astrid-edge-self-change-probation-health.service" || fail "probation service lacks default-deny IP policy"
grep -q '^IPAddressAllow=localhost$' "$REPO_ROOT/packaging/systemd/astrid-edge-self-change-probation-health.service" || fail "probation service lacks exact loopback allowance"
grep -q '^RestrictAddressFamilies=AF_UNIX AF_INET$' "$REPO_ROOT/packaging/systemd/astrid-edge-self-change-probation-health.service" || fail "probation service socket families exceed Unix and IPv4 loopback"
grep -q '^MemoryDenyWriteExecute=yes$' "$REPO_ROOT/packaging/systemd/astrid-edge-self-change-probation-health.service" || fail "probation service unnecessarily permits executable writable mappings"
grep -q '^NoExecPaths=/$' "$REPO_ROOT/packaging/systemd/astrid-edge-self-change-probation-health.service" || fail "probation sampler lacks default-deny file execution"
probation_dropin_block=$(sed -n '/astrid-edge-self-change-probation-health.service)/,/astrid-edge-generation-guard.service)/p' "$INSTALLER")
grep -q "ExecPaths=/usr/bin /usr/sbin /bin /sbin %s %s %s" <<<"$probation_dropin_block" || fail "probation cannot execute its exact health/rollback helpers"
! grep -q '\$toolchain_root\|\$builder_root\|\$capsule_builder_install_path\|\$source_root\|\$candidate_root' <<<"$(grep 'ExecPaths=' <<<"$probation_dropin_block")" || fail "probation execution boundary accidentally gained build authority"
grep -q 'readonly OPERATOR_STATUS_ROOT=/var/lib/astrid-edge-operator' "$INSTALLER" || fail "operator status root is not the fixed persistent path"
grep -q 'install -d -m 2750 -o root -g "$runtime_group" "$OPERATOR_STATUS_ROOT"' "$INSTALLER" || fail "operator status root is not root-owned setgid operator-readable"
grep -q 'declare -a protected_steward_parents=' "$INSTALLER" || fail "steward outputs have no immutable parent boundary"
grep -Fq 'chown root:"$runtime_group" "$parent"' "$INSTALLER" || fail "steward output parents remain mutable-runtime owned"
grep -q 'chmod 0710 "$parent"' "$INSTALLER" || fail "steward output parents are not traversal-only"
grep -q 'runtime can rename a steward-owned output root' "$INSTALLER" || fail "bootstrap does not prove rename denial for steward outputs"
grep -q 'runtime can unlink a steward-owned output member' "$INSTALLER" || fail "bootstrap does not prove unlink denial for steward outputs"
grep -Fq 'install -d -m 0700 -o "$runtime_user" -g "$runtime_gid" "$runtime_self_change_outbox"' "$INSTALLER" || fail "runtime lost its exact writable self-change outbox below the immutable parent"
! grep -q 'TemporaryFileSystem=.*candidate_work\|TemporaryFileSystem=%s:rw,size=%s,mode=0710' "$INSTALLER" || fail "candidate build storage is still RAM-backed"
grep -q 'TemporaryFileSystem=/tmp:rw,size=.*TemporaryFileSystem=/var/tmp:rw,size=' "$INSTALLER" || fail "untrusted temporary scratch can fill the host filesystem"
grep -q '^supervisor_memory_max=9663676416$' "$INSTALLER" || fail "AVADO build cgroup does not reserve host acceptance memory"
grep -q 'supervisor_memory_max=4294967296' "$INSTALLER" || fail "ICP build cgroup does not reserve host acceptance memory"
grep -q 'host_memory_kib >= required_memory_kib' "$INSTALLER" || fail "build cgroup is not bounded against actual host memory"
grep -q 'builder_initialize_args=(initialize --config "$BUILDER_STORE_CONFIG" --image "$builder_image" --mount "$builder_root"' "$INSTALLER" || fail "builder filesystem is not initialized under the immutable helper"
grep -q 'SystemCallFilter=~kill tkill tgkill pidfd_send_signal pidfd_getfd kcmp process_vm_readv process_vm_writev ptrace' "$MIGRATOR" || fail "mutable core/edge units can duplicate peer descriptors or introspect same-UID processes"
grep -Fq "printf 'SystemCallFilter=\\nSystemCallFilter=~pidfd_getfd kcmp process_vm_readv process_vm_writev ptrace\\nProtectProc=invisible\\nProcSubset=all\\n'" "$MIGRATOR" || fail "root-pinned Ollama cannot preempt/reap its runner or discover host CPU/memory"
grep -Fq "printf 'ProtectProc=invisible\\nProcSubset=all\\n'" "$MIGRATOR" || fail "edge runtime cannot read bounded host /proc sensing inputs"
grep -q '"$BUILDER_STORE_INSTALL" "${builder_initialize_args\[@\]}"' "$INSTALLER" || fail "immutable builder-store initializer is not executed"
grep -q '"$BUILDER_STORE_INSTALL" prepare --config "$BUILDER_STORE_CONFIG"' "$INSTALLER" || fail "mounted builder filesystem layout is not prepared and verified"
grep -Fq 'builder_mount_unit_sed=${builder_mount_unit' "$INSTALLER" || fail "systemd mount-unit replacement is not escaped for sed"
grep -Fq 's|@@BUILDER_MOUNT_UNIT@@|$builder_mount_unit_sed|g' "$INSTALLER" || fail "escaped mount-unit replacement is not used by verifier rendering"
grep -q 'Requires=%s.*BUILDER_STORE_VERIFY_UNIT' "$INSTALLER" || fail "build/activation services do not require the verified builder filesystem"
grep -q 'for ((index=${#started_now\[@\]}-1; index>=0; index--))' "$INSTALLER" || fail "rollback does not stop verifier before its mount"
grep -q '^ExecStartPre=/usr/bin/test -s /etc/astrid/edge-builder-store.json$' "$REPO_ROOT/packaging/systemd/astrid-edge-builder-store-verify.service.in" || fail "builder verifier does not fail on an absent/empty identity config"
grep -q '^ExecStartPre=/usr/bin/test -x /usr/libexec/astrid/astrid-edge-builder-store$' "$REPO_ROOT/packaging/systemd/astrid-edge-builder-store-verify.service.in" || fail "builder verifier does not fail on a missing/non-executable helper"
if rg -n -- '--candidate|--generation|record-candidate|stage[[:space:]]' "$REPO_ROOT/packaging/systemd/root/astrid-edge-self-evolution-control"; then
    fail "operator wrapper exposes change selection"
fi
grep -q '^    synthetic)$' "$REPO_ROOT/packaging/systemd/root/astrid-edge-self-evolution-control" || fail "operator synthetic action is absent"
grep -q '"\$PYTHON" -I -E -s "\$SUPERVISOR" --config "\$CONFIG" --execute request-synthetic' "$REPO_ROOT/packaging/systemd/root/astrid-edge-self-evolution-control" || fail "operator synthetic action bypasses the exact isolated Python supervisor queue"
grep -q 'exec /usr/bin/systemctl start astrid-edge-self-change-supervisor.service' "$REPO_ROOT/packaging/systemd/root/astrid-edge-self-evolution-control" || fail "operator synthetic action does not enter the bounded supervisor service"
grep -q '^    resume)$' "$REPO_ROOT/packaging/systemd/root/astrid-edge-self-evolution-control" || fail "operator resume is not independently fail-closed"
grep -q 'usage: \$0 --reason TEXT \[--ack-rescue\]' "$REPO_ROOT/packaging/systemd/root/astrid-edge-self-evolution-control" || fail "operator resume does not advertise explicit rescue acknowledgement"
grep -q 'execute resume --reason "\$2" --ack-rescue' "$REPO_ROOT/packaging/systemd/root/astrid-edge-self-evolution-control" || fail "supported operator wrapper cannot explicitly acknowledge rescue mode"
grep -q 'systemctl start "\$INBOX_PATH_UNIT"' "$REPO_ROOT/packaging/systemd/root/astrid-edge-self-evolution-control" || fail "operator resume does not restore the immutable candidate handoff watcher"
grep -q 'exec /usr/bin/systemctl start astrid-edge-self-change-supervisor.service' "$REPO_ROOT/packaging/systemd/root/astrid-edge-self-evolution-control" || fail "operator resume does not immediately process queued signed intents"
grep -q 'systemctl stop "\$INBOX_PATH_UNIT"' "$REPO_ROOT/packaging/systemd/root/astrid-edge-self-evolution-control" || fail "pause/rescue controls leave automatic candidate wakeups enabled"
if sed -n '/pause|rollback|rescue)/,/;;/p' "$REPO_ROOT/packaging/systemd/root/astrid-edge-self-evolution-control" | grep -q -- '--ack-rescue'; then fail "ordinary controls can accidentally acknowledge rescue mode"; fi
grep -q 'for action in status pause resume rollback rescue synthetic' "$INSTALLER" || fail "fixed synthetic operator entry point is not installed"
grep -q 'core.get("mode") != "paused"' "$INSTALLER" || fail "bootstrap does not verify that live self-change starts deployment-paused"
grep -q 'bootstrap_acceptance_pending' "$REPO_ROOT/scripts/edge_self_change/supervisor.py" || fail "supervisor lacks the immutable bootstrap acceptance pause"
grep -q 'exec /usr/bin/cat "$OPERATOR_STATUS"' "$REPO_ROOT/packaging/systemd/root/astrid-edge-self-evolution-control" || fail "status does not use the narrow operator projection"
grep -q '^OPERATOR_STATUS=/var/lib/astrid-edge-operator/operator-status.json$' "$REPO_ROOT/packaging/systemd/root/astrid-edge-self-evolution-control" || fail "status control does not use the persistent projection"
if sed -n '/status)/,/;;/p' "$REPO_ROOT/packaging/systemd/root/astrid-edge-self-evolution-control" | grep -q 'id -u'; then fail "read-only status is incorrectly sudo-gated"; fi

# On a root Linux test host, prove two distinct service identities can open the
# same root-owned ACL lock and cannot both hold its kernel flock. This is skipped
# on developer Macs and unprivileged CI, while the static installer assertions
# above still run everywhere.
if [[ $(uname -s) == Linux && $(id -u) == 0 ]] && command -v setfacl >/dev/null && command -v runuser >/dev/null && command -v flock >/dev/null; then
    mapfile -t acl_users < <(getent passwd | awk -F: '$3 > 0 && $3 < 65534 {print $1}' | head -n 2)
    if ((${#acl_users[@]} == 2)); then
        acl_lock=$TEMP/two-identity.lock
        install -m 0600 -o root -g root /dev/null "$acl_lock"
        setfacl -m "u:${acl_users[0]}:rw-,u:${acl_users[1]}:rw-,m::rw-" "$acl_lock"
        runuser -u "${acl_users[0]}" -- sh -c 'exec 9<>"$1"; flock -n 9; sleep 1' sh "$acl_lock" &
        holder=$!
        sleep 0.1
        if runuser -u "${acl_users[1]}" -- sh -c 'exec 9<>"$1"; flock -n 9' sh "$acl_lock"; then
            kill "$holder" 2>/dev/null || true
            fail "second ACL identity acquired an already-held model lock"
        fi
        wait "$holder"
    fi
fi

printf 'PASS: root bootstrap dry-run, immutable rescue/broker boundary, crash-safe migration, and isolation checks\n'
