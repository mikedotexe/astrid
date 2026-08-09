#!/usr/bin/env bash
# Verify source or packaged CPU-edge systemd units without installing them.
set -euo pipefail
IFS=$'\n\t'
umask 077

fail() {
    printf 'systemd-unit-verifier: %s\n' "$*" >&2
    exit 1
}

need_value() {
    (($# >= 2)) && [[ -n $2 ]] || fail "missing value for $1"
}

repo_root=$(cd "$(dirname "$0")/.." && pwd -P)
unit_root=$repo_root/packaging/systemd

while (($#)); do
    case "$1" in
        --unit-root)
            need_value "$@"
            unit_root=$2
            shift 2
            ;;
        -h|--help)
            printf 'usage: %s [--unit-root DIR]\n' "$0"
            exit 0
            ;;
        *) fail "unsupported argument: $1" ;;
    esac
done

[[ -d $unit_root ]] || fail "unit root is absent: $unit_root"
unit_root=$(cd "$unit_root" && pwd -P)
for command in find install mktemp rg sed sort systemd-analyze systemd-escape; do
    command -v "$command" >/dev/null || fail "required command is absent: $command"
done
for required in \
    astrid.service \
    astrid-edge-runtime.service \
    astrid-edge-hindsight.service \
    astrid-edge-hindsight.timer \
    astrid-model-warmup.service \
    ollama-cpu.service \
    icp/astrid.service \
    icp/astrid-edge-runtime.service \
    icp/astrid-edge-hindsight.service \
    icp/astrid-edge-hindsight.timer \
    icp/astrid-model-warmup.service \
    icp/ollama-cpu.service \
    wait-for-icp-ssd; do
    [[ -f $unit_root/$required ]] || fail "required unit asset is absent: $required"
done

tmp_parent=${RUNNER_TEMP:-${TMPDIR:-/tmp}}
fixture_root=$(mktemp -d "$tmp_parent/astrid-edge-systemd-verify.XXXXXX")
fixture_root=$(cd "$fixture_root" && pwd -P)
trap 'rm -rf -- "$fixture_root"' EXIT HUP INT TERM
fixture_home=$fixture_root/home
fixture_privileged_root=$fixture_root/privileged-root
immutable_units=$fixture_root/immutable-units
immutable_verify_units=$fixture_root/immutable-verify-units
merged_units=$fixture_root/merged-icp-units

# systemd 249 requires every direct Exec*= program to exist even during a
# static verify. These fixtures exercise the real unit text while ensuring
# verification never writes into the runner's HOME or privileged filesystem.
readonly -a user_fixture_paths=(
    .astrid/bin/edge-hindsight
    .astrid/bin/astrid-edge-runtime
    .astrid/bin/warm-ollama-model
    .astrid/bin/astrid-daemon
    .local/bin/ollama
    .astrid-icp/state/bin/edge-hindsight
    .astrid-icp/state/bin/astrid-edge-runtime
    .astrid-icp/state/bin/warm-ollama-model
    .astrid-icp/state/bin/astrid-daemon
    .astrid-icp/ollama/runtime/bin/ollama
)
for relative_path in "${user_fixture_paths[@]}"; do
    install -D -m 0755 /usr/bin/true "$fixture_home/$relative_path"
done
install -D -m 0755 "$unit_root/wait-for-icp-ssd" \
    "$fixture_home/.local/libexec/astrid/wait-for-icp-ssd"

mapfile -t base_units < <(find "$unit_root" -maxdepth 1 -type f \
    \( -name '*.service' -o -name '*.timer' \) \
    ! -name 'astrid-edge-self-change-supervisor.service' \
    ! -name 'astrid-edge-self-change-probation-health.service' \
    ! -name 'astrid-edge-self-change-probation-health.timer' \
    ! -name 'astrid-edge-generation-guard.service' \
    ! -name 'astrid-edge-core-liveness.service' \
    ! -name 'astrid-edge-steward.service' \
    ! -name 'astrid-edge-steward.timer' \
    ! -name 'astrid-edge-web-broker-*.service' \
    ! -name 'astrid-edge-provider-broker@.service' \
    ! -name 'astrid-edge-presentation-broker@.service.in' \
    ! -name 'astrid-edge-audio-feeder.service' \
    ! -name 'astrid-edge-state-store-health.service' \
    ! -name 'astrid-edge-state-store-health.timer' \
    -print | sort)
((${#base_units[@]} > 0)) || fail 'base unit set is empty'
HOME=$fixture_home systemd-analyze verify "${base_units[@]}"
HOME=$fixture_home systemd-analyze verify \
    "$unit_root"/icp/*.service \
    "$unit_root"/icp/*.timer

install -d -m 0700 "$immutable_units"
cp \
    "$unit_root/astrid-edge-self-change-supervisor.service" \
    "$unit_root/astrid-edge-self-change-probation-health.service" \
    "$unit_root/astrid-edge-self-change-probation-health.timer" \
    "$unit_root/astrid-edge-generation-guard.service" \
    "$unit_root/astrid-edge-core-liveness.service" \
    "$unit_root/astrid-edge-steward.timer" \
    "$unit_root/astrid-edge-web-broker-core.service" \
    "$unit_root/astrid-edge-web-broker-runtime.service" \
    "$unit_root/astrid-edge-web-broker-steward.service" \
    "$unit_root/astrid-edge-provider-broker@.service" \
    "$unit_root/astrid-edge-audio-feeder.service" \
    "$unit_root/astrid-edge-state-store-health.service" \
    "$unit_root/astrid-edge-state-store-health.timer" \
    "$unit_root/ollama-cpu.service" \
    "$immutable_units/"
sed 's|@@RUNTIME_GROUP@@|root|g' \
    "$unit_root/astrid-edge-steward.service" \
    >"$immutable_units/astrid-edge-steward.service"
sed \
    -e 's|@@RELEASE_PARENT@@|/opt/astrid-edge|g' \
    -e 's|@@GENERATION_FILE@@|/var/lib/astrid-edge/current-generation|g' \
    "$unit_root/astrid-edge-presentation-broker@.service.in" \
    >"$immutable_units/astrid-edge-presentation-broker@.service"
sed 's|@@RUNTIME_GROUP@@|root|g' \
    "$unit_root/astrid-edge-presentation-broker.socket.in" \
    >"$immutable_units/astrid-edge-presentation-broker.socket"
sed 's|@@RUNTIME_GROUP@@|root|g' \
    "$unit_root/astrid-edge-web-broker-runtime.socket.in" \
    >"$immutable_units/astrid-edge-web-broker-runtime.socket"
sed 's|@@CORE_GROUP@@|root|g' \
    "$unit_root/astrid-edge-web-broker-core.socket.in" \
    >"$immutable_units/astrid-edge-web-broker-core.socket"
sed 's|@@STEWARD_GROUP@@|root|g' \
    "$unit_root/astrid-edge-web-broker-steward.socket.in" \
    >"$immutable_units/astrid-edge-web-broker-steward.socket"
sed 's|@@PROVIDER_RUNTIME_GROUP@@|root|g' \
    "$unit_root/astrid-edge-provider-runtime.socket.in" \
    >"$immutable_units/astrid-edge-provider-runtime.socket"
sed 's|@@PROVIDER_STEWARD_GROUP@@|root|g' \
    "$unit_root/astrid-edge-provider-steward.socket.in" \
    >"$immutable_units/astrid-edge-provider-steward.socket"
sed 's|@@PROVIDER_WARMUP_GROUP@@|root|g' \
    "$unit_root/astrid-edge-provider-warmup.socket.in" \
    >"$immutable_units/astrid-edge-provider-warmup.socket"
sed 's|@AUDIO_CLIENT_GROUP@|root|g' \
    "$unit_root/astrid-edge-audio-feeder.socket.in" \
    >"$immutable_units/astrid-edge-audio-feeder.socket"
sed 's|@@CORE_LIVENESS_REQUEST@@|/var/lib/astrid-edge/workspace/runtime/core-liveness-recovery.request.json|g' \
    "$unit_root/astrid-edge-core-liveness.path.in" \
    >"$immutable_units/astrid-edge-core-liveness.path"

builder_root=/var/lib/astrid-edge-builder
builder_image=/var/lib/astrid-edge-builder.ext4
builder_mount_unit=$(systemd-escape --path --suffix=mount "$builder_root")
builder_mount_unit_sed=${builder_mount_unit//\\/\\\\}
sed \
    -e 's|@@ACTIVE_GENERATION_ROOT@@|/opt/astrid-edge/current|g' \
    -e 's|@@BUILDER_IMAGE_PARENT@@|/var/lib|g' \
    -e "s|@@BUILDER_IMAGE@@|$builder_image|g" \
    -e "s|@@BUILDER_ROOT@@|$builder_root|g" \
    "$unit_root/root/astrid-edge-builder-store.mount.in" \
    >"$immutable_units/$builder_mount_unit"
sed \
    -e 's|@@ACTIVE_GENERATION_ROOT@@|/opt/astrid-edge/current|g' \
    -e "s|@@BUILDER_MOUNT_UNIT@@|$builder_mount_unit_sed|g" \
    -e "s|@@BUILDER_IMAGE@@|$builder_image|g" \
    -e "s|@@BUILDER_ROOT@@|$builder_root|g" \
    "$unit_root/astrid-edge-builder-store-verify.service.in" \
    >"$immutable_units/astrid-edge-builder-store-verify.service"

state_backing_root=/var/lib/astrid-edge-state
state_migration_root=$state_backing_root/migration
runtime_image=$state_backing_root/runtime.ext4
rollback_image=$state_backing_root/rollback.ext4
runtime_state_root=/var/lib/astrid-edge-runtime-state
rollback_state_root=/var/lib/astrid-edge-rollback-state
runtime_mount_unit=$(systemd-escape --path --suffix=mount "$runtime_state_root")
rollback_mount_unit=$(systemd-escape --path --suffix=mount "$rollback_state_root")
sed \
    -e 's|@@ACTIVE_GENERATION_ROOT@@|/opt/astrid-edge/current|g' \
    -e "s|@@BACKING_ROOT@@|$state_backing_root|g" \
    -e "s|@@MIGRATION_ROOT@@|$state_migration_root|g" \
    -e "s|@@RUNTIME_IMAGE@@|$runtime_image|g" \
    -e "s|@@ROLLBACK_IMAGE@@|$rollback_image|g" \
    -e "s|@@RUNTIME_MOUNT_PARENT@@|$runtime_state_root|g" \
    -e "s|@@ROLLBACK_MOUNT_PARENT@@|$rollback_state_root|g" \
    -e "s|@@RUNTIME_MOUNT_UNIT@@|$runtime_mount_unit|g" \
    -e "s|@@ROLLBACK_MOUNT_UNIT@@|$rollback_mount_unit|g" \
    "$unit_root/astrid-edge-state-store-migration-recover.service.in" \
    >"$immutable_units/astrid-edge-state-store-migration-recover.service"
sed \
    -e 's|@@ACTIVE_GENERATION_ROOT@@|/opt/astrid-edge/current|g' \
    -e "s|@@RUNTIME_IMAGE_PARENT@@|$state_backing_root|g" \
    -e "s|@@RUNTIME_IMAGE@@|$runtime_image|g" \
    -e "s|@@RUNTIME_STATE_ROOT@@|$runtime_state_root|g" \
    "$unit_root/root/astrid-edge-state-store-runtime.mount.in" \
    >"$immutable_units/$runtime_mount_unit"
sed \
    -e 's|@@ACTIVE_GENERATION_ROOT@@|/opt/astrid-edge/current|g' \
    -e "s|@@ROLLBACK_IMAGE_PARENT@@|$state_backing_root|g" \
    -e "s|@@ROLLBACK_IMAGE@@|$rollback_image|g" \
    -e "s|@@ROLLBACK_STATE_ROOT@@|$rollback_state_root|g" \
    "$unit_root/root/astrid-edge-state-store-rollback.mount.in" \
    >"$immutable_units/$rollback_mount_unit"
sed \
    -e 's|@@ACTIVE_GENERATION_ROOT@@|/opt/astrid-edge/current|g' \
    -e "s|@@BACKING_ROOT@@|$state_backing_root|g" \
    -e "s|@@RUNTIME_IMAGE@@|$runtime_image|g" \
    -e "s|@@ROLLBACK_IMAGE@@|$rollback_image|g" \
    -e "s|@@RUNTIME_STATE_ROOT@@|$runtime_state_root|g" \
    -e "s|@@ROLLBACK_STATE_ROOT@@|$rollback_state_root|g" \
    -e "s|@@RUNTIME_MOUNT_UNIT@@|$runtime_mount_unit|g" \
    -e "s|@@ROLLBACK_MOUNT_UNIT@@|$rollback_mount_unit|g" \
    "$unit_root/astrid-edge-state-store-recover.service.in" \
    >"$immutable_units/astrid-edge-state-store-recover.service"
sed \
    -e 's|@@ACTIVE_GENERATION_ROOT@@|/opt/astrid-edge/current|g' \
    -e "s|@@MIGRATION_ROOT@@|$state_migration_root|g" \
    -e "s|@@RUNTIME_IMAGE@@|$runtime_image|g" \
    -e "s|@@ROLLBACK_IMAGE@@|$rollback_image|g" \
    -e "s|@@RUNTIME_STATE_ROOT@@|$runtime_state_root|g" \
    -e "s|@@ROLLBACK_STATE_ROOT@@|$rollback_state_root|g" \
    -e "s|@@RUNTIME_MOUNT_UNIT@@|$runtime_mount_unit|g" \
    -e "s|@@ROLLBACK_MOUNT_UNIT@@|$rollback_mount_unit|g" \
    "$unit_root/astrid-edge-state-store-verify.service.in" \
    >"$immutable_units/astrid-edge-state-store-verify.service"

if rg -n '@@[A-Z0-9_]+@@|@AUDIO_CLIENT_GROUP@' "$immutable_units"; then
    fail 'rendered immutable units retain a template placeholder'
fi
[[ ! -e $unit_root/icp/astrid-edge-audio-feeder.service ]] \
    || fail 'ICP must not advertise an audio feeder service'
[[ ! -e $unit_root/icp/astrid-edge-audio-feeder.socket ]] \
    || fail 'ICP must not advertise an audio feeder socket'

# Keep the privileged fixture allowlist exact. A new or misspelled immutable
# path must fail CI until it is deliberately reviewed here.
readonly expected_privileged_paths=$'/usr/libexec/astrid-edge/immutable/astrid-edge-presentation-broker\n/usr/libexec/astrid-edge/immutable/astrid-edge-provider-broker\n/usr/libexec/astrid-edge/immutable/astrid-edge-web-broker\n/usr/libexec/astrid-edge/immutable/edge_audio_feeder.py\n/usr/libexec/astrid/astrid-edge-builder-store\n/usr/libexec/astrid/astrid-edge-rescue-helper\n/usr/libexec/astrid/astrid-edge-state-store\n/usr/libexec/astrid/astrid-edge-steward-helper\n/usr/libexec/astrid/edge-self-change-supervisor'
actual_privileged_paths=$(rg --no-filename -o \
    '/usr/libexec/(astrid|astrid-edge/immutable)/[^[:space:]]+' \
    "$immutable_units" | sed 's/[;,]$//' | sort -u)
if [[ $actual_privileged_paths != "$expected_privileged_paths" ]]; then
    printf 'expected privileged unit paths:\n%s\nactual privileged unit paths:\n%s\n' \
        "$expected_privileged_paths" "$actual_privileged_paths" >&2
    fail 'immutable unit privileged-path allowlist changed'
fi

install -d -m 0700 "$immutable_verify_units"
cp -R "$immutable_units/." "$immutable_verify_units/"
while IFS= read -r privileged_path; do
    [[ -n $privileged_path ]] || continue
    install -D -m 0755 /usr/bin/true "$fixture_privileged_root$privileged_path"
done <<<"$expected_privileged_paths"
while IFS= read -r -d '' unit; do
    sed -i \
        -e "s|/usr/libexec/astrid/|$fixture_privileged_root/usr/libexec/astrid/|g" \
        -e "s|/usr/libexec/astrid-edge/immutable/|$fixture_privileged_root/usr/libexec/astrid-edge/immutable/|g" \
        "$unit"
done < <(find "$immutable_verify_units" -type f -print0)
if rg -n '(^|[=+[:space:]])/usr/libexec/(astrid|astrid-edge/immutable)/' \
    "$immutable_verify_units"; then
    fail 'a privileged path escaped fixture remapping'
fi

HOME=$fixture_home SYSTEMD_UNIT_PATH="$immutable_verify_units:" systemd-analyze verify \
    astrid-edge-self-change-supervisor.service \
    astrid-edge-self-change-probation-health.service \
    astrid-edge-self-change-probation-health.timer \
    astrid-edge-generation-guard.service \
    astrid-edge-core-liveness.service \
    astrid-edge-core-liveness.path \
    astrid-edge-steward.service \
    astrid-edge-steward.timer \
    astrid-edge-web-broker-core.socket \
    astrid-edge-web-broker-core.service \
    astrid-edge-web-broker-runtime.socket \
    astrid-edge-web-broker-runtime.service \
    astrid-edge-web-broker-steward.socket \
    astrid-edge-web-broker-steward.service \
    astrid-edge-provider-runtime.socket \
    astrid-edge-provider-broker@edge-runtime.service \
    astrid-edge-provider-steward.socket \
    astrid-edge-provider-broker@edge-steward.service \
    astrid-edge-provider-warmup.socket \
    astrid-edge-provider-broker@model-warmup.service \
    astrid-edge-audio-feeder.socket \
    astrid-edge-audio-feeder.service \
    astrid-edge-presentation-broker.socket \
    astrid-edge-presentation-broker@0.service \
    "$builder_mount_unit" \
    astrid-edge-builder-store-verify.service \
    astrid-edge-state-store-migration-recover.service \
    astrid-edge-state-store-recover.service \
    "$runtime_mount_unit" \
    "$rollback_mount_unit" \
    astrid-edge-state-store-verify.service \
    astrid-edge-state-store-health.service \
    astrid-edge-state-store-health.timer

"$repo_root/scripts/test_migrate_edge_system_renderer.sh" \
    --support-unit-root "$immutable_units"

install -d -m 0700 "$merged_units"
cp "$unit_root"/icp/*.service "$unit_root"/icp/*.timer "$merged_units/"
for service in \
    astrid.service \
    ollama-cpu.service \
    astrid-model-warmup.service \
    astrid-edge-runtime.service \
    astrid-edge-hindsight.service; do
    install -d -m 0700 "$merged_units/$service.d"
    cp "$unit_root/icp-ssd-required.conf" \
        "$merged_units/$service.d/ssd-required.conf"
done
HOME=$fixture_home SYSTEMD_UNIT_PATH="$merged_units:" systemd-analyze verify \
    astrid.service \
    ollama-cpu.service \
    astrid-model-warmup.service \
    astrid-edge-runtime.service \
    astrid-edge-hindsight.service \
    astrid-edge-hindsight.timer

printf 'systemd-unit-verifier: verified %s\n' "$unit_root"
