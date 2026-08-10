#!/usr/bin/env bash
# Install the immutable root boundary for CPU-edge self-evolution.
set -euo pipefail
IFS=$'\n\t'
umask 077
PATH=/usr/sbin:/usr/bin:/sbin:/bin
export PATH

readonly STEWARD_USER=astrid-edge-steward
readonly BUILDER_USER=astrid-edge-builder
readonly UPDATER_USER=astrid-edge-updater
readonly WEB_USER=astrid-edge-web
readonly PROVIDER_USER=astrid-edge-provider
readonly WARMUP_USER=astrid-edge-warmup
readonly PRESENTATION_USER=astrid-edge-presentation
readonly AUDIO_USER=astrid-edge-audio
readonly MODEL_LOCK_GROUP=astrid-edge-model-lock
readonly WEB_CORE_CLIENT_GROUP=astrid-edge-web-core-client
readonly WEB_RUNTIME_CLIENT_GROUP=astrid-edge-web-runtime-client
readonly PROVIDER_RUNTIME_GROUP=astrid-edge-provider-runtime-client
readonly PROVIDER_STEWARD_GROUP=astrid-edge-provider-steward-client
readonly PROVIDER_WARMUP_GROUP=astrid-edge-provider-warmup-client
readonly AUDIO_CLIENT_GROUP=astrid-edge-audio-client
readonly SUPERVISOR_CONFIG=/etc/astrid/edge-self-change.json
readonly RESCUE_CONFIG=/etc/astrid/edge-rescue-helper.json
readonly WEB_CORE_CONFIG=/etc/astrid-edge-self-change/web-broker-core.json
readonly WEB_RUNTIME_CONFIG=/etc/astrid-edge-self-change/web-broker-runtime.json
readonly WEB_STEWARD_CONFIG=/etc/astrid-edge-self-change/web-broker-steward.json
readonly BUILDER_STORE_CONFIG=/etc/astrid/edge-builder-store.json
readonly STATE_STORE_CONFIG=/etc/astrid/edge-state-store.json
readonly CORE_WEB_REQUEST_KEY=/etc/astrid-edge-self-change/core-web-request.key
readonly RUNTIME_WEB_REQUEST_KEY=/etc/astrid-edge-self-change/runtime-web-request.key
readonly STEWARD_WEB_REQUEST_KEY=/etc/astrid-edge-self-change/steward-web-request.key
readonly WEB_RESPONSE_SIGNING_KEY=/etc/astrid-edge-self-change/web-response-signing.key
readonly WEB_RESPONSE_VERIFY_KEY=/etc/astrid-edge-self-change/web-response.pub
readonly PROVIDER_CONFIG=/etc/astrid-edge-self-change/provider-broker.json
readonly PRESENTATION_CONFIG=/etc/astrid-edge-self-change/presentation-broker.json
readonly AUDIO_CONFIG=/etc/astrid-edge-self-change/audio-feeder.json
readonly HINDSIGHT_CONFIG=/etc/astrid-edge-self-change/hindsight-writer.json
readonly RUNTIME_PROVIDER_REQUEST_KEY=/etc/astrid-edge-self-change/edge-runtime-provider-request.key
readonly STEWARD_PROVIDER_REQUEST_KEY=/etc/astrid-edge-self-change/edge-steward-provider-request.key
readonly WARMUP_PROVIDER_REQUEST_KEY=/etc/astrid-edge-self-change/model-warmup-provider-request.key
readonly PROVIDER_LEDGER_KEY=/etc/astrid-edge-self-change/provider-ledger.key
readonly LEDGER_KEY_ROOT=/etc/astrid-edge-self-change/keys
readonly LEDGER_ATTESTATION_KEY=/etc/astrid-edge-self-change/keys/ledger-attestation.key
readonly MANAGER_MARKER=/etc/astrid/edge-service-manager.json
readonly AUTHORITY_ENV=/etc/astrid/edge-self-change-authority.env
readonly SUPERVISOR_KEY=/etc/astrid/edge-self-change.key
readonly PROFILES_CONFIG=/etc/astrid/edge-self-change-command-profiles.json
readonly STEWARD_CONFIG=/etc/astrid/edge-steward-helper.json
readonly SOURCE_KEY=/etc/astrid/edge-self-evolution-source.key
readonly INTENT_KEY=/etc/astrid/edge-self-evolution-intent.key
readonly SCHEDULED_AUTHORSHIP_VERIFY_KEY=/etc/astrid/edge-scheduled-authorship.pub
readonly SUPERVISOR_INSTALL=/usr/libexec/astrid/edge-self-change-supervisor
readonly STEWARD_INSTALL=/usr/libexec/astrid/astrid-edge-steward-helper
readonly RESCUE_INSTALL=/usr/libexec/astrid/astrid-edge-rescue-helper
readonly CHECKPOINT_INSTALL=/usr/libexec/astrid/astrid-edge-checkpoint
readonly CAPSULE_BUILDER_INSTALL=/usr/libexec/astrid/astrid-build
readonly WEB_BROKER_INSTALL=/usr/libexec/astrid-edge/immutable/astrid-edge-web-broker
readonly PROVIDER_BROKER_INSTALL=/usr/libexec/astrid-edge/immutable/astrid-edge-provider-broker
readonly PRESENTATION_BROKER_INSTALL=/usr/libexec/astrid-edge/immutable/astrid-edge-presentation-broker
readonly HINDSIGHT_WRITER_INSTALL=/usr/libexec/astrid-edge/immutable/edge_hindsight.py
readonly AUDIO_FEEDER_INSTALL=/usr/libexec/astrid-edge/immutable/edge_audio_feeder.py
readonly OPERATOR_REPORT_ROOT=/usr/libexec/astrid-edge/operator
readonly OPERATOR_REPORT_MANIFEST=/usr/libexec/astrid-edge/operator/MANIFEST.sha256
readonly BUILDER_STORE_INSTALL=/usr/libexec/astrid/astrid-edge-builder-store
readonly STATE_STORE_INSTALL=/usr/libexec/astrid/astrid-edge-state-store
readonly CANDIDATE_SANDBOX_ROOT=/usr/libexec/astrid-edge/immutable/candidate-rootfs
readonly BUILDER_STORE_VERIFY_UNIT=astrid-edge-builder-store-verify.service
readonly STATE_STORE_MIGRATION_RECOVER_UNIT=astrid-edge-state-store-migration-recover.service
readonly STATE_STORE_RECOVER_UNIT=astrid-edge-state-store-recover.service
readonly STATE_STORE_VERIFY_UNIT=astrid-edge-state-store-verify.service
readonly STATE_STORE_HEALTH_UNIT=astrid-edge-state-store-health.service
readonly STATE_STORE_HEALTH_TIMER=astrid-edge-state-store-health.timer
readonly WEB_CORE_SOCKET_UNIT=astrid-edge-web-broker-core.socket
readonly WEB_CORE_SERVICE_UNIT=astrid-edge-web-broker-core.service
readonly WEB_RUNTIME_SOCKET_UNIT=astrid-edge-web-broker-runtime.socket
readonly WEB_RUNTIME_SERVICE_UNIT=astrid-edge-web-broker-runtime.service
readonly WEB_STEWARD_SOCKET_UNIT=astrid-edge-web-broker-steward.socket
readonly WEB_STEWARD_SERVICE_UNIT=astrid-edge-web-broker-steward.service
readonly PROVIDER_SERVICE_TEMPLATE=astrid-edge-provider-broker@.service
readonly PROVIDER_RUNTIME_SOCKET_UNIT=astrid-edge-provider-runtime.socket
readonly PROVIDER_STEWARD_SOCKET_UNIT=astrid-edge-provider-steward.socket
readonly PROVIDER_WARMUP_SOCKET_UNIT=astrid-edge-provider-warmup.socket
readonly PRESENTATION_SOCKET_UNIT=astrid-edge-presentation-broker.socket
readonly PRESENTATION_SERVICE_TEMPLATE=astrid-edge-presentation-broker@.service
readonly CORE_LIVENESS_PATH_UNIT=astrid-edge-core-liveness.path
readonly SELF_CHANGE_INBOX_PATH_UNIT=astrid-edge-self-change-inbox.path
readonly AUDIO_SOCKET_UNIT=astrid-edge-audio-feeder.socket
readonly AUDIO_SERVICE_UNIT=astrid-edge-audio-feeder.service
readonly WEB_CORE_SOCKET=/run/astrid-edge-self-change/web-core.sock
readonly WEB_RUNTIME_SOCKET=/run/astrid-edge-self-change/web-runtime.sock
readonly WEB_STEWARD_SOCKET=/run/astrid-edge-self-change/web-steward.sock
readonly OPERATOR_STATUS_ROOT=/var/lib/astrid-edge-operator
readonly OPERATOR_STATUS=/var/lib/astrid-edge-operator/operator-status.json

dry_run=false
start_system_services=false
appliance_id= target= runtime_user= runtime_home= runtime_workspace= model_ipc=
model= ollama_origin= context_tokens= output_tokens= reflection_output_tokens= source_authoring_output_tokens=
connect_timeout_ms= header_timeout_ms= total_timeout_ms=
model_lock= autonomy_state= action_receipts=
thermal_celsius= maximum_thermal_celsius=
helper= helper_sha256= helper_install_path=
supervisor= supervisor_sha256= supervisor_install_path=
rescue_helper= rescue_helper_sha256= rescue_helper_install_path=
checkpoint= checkpoint_sha256= checkpoint_install_path=
capsule_builder= capsule_builder_sha256= capsule_builder_install_path=
web_broker= web_broker_sha256= web_broker_install_path=
provider_broker= provider_broker_sha256= provider_broker_install_path=
presentation_broker= presentation_broker_sha256= presentation_broker_install_path=
source_signing_key= source_signing_key_sha256=
source_bundle= source_bundle_sha256= toolchain_bundle= toolchain_bundle_sha256=
initial_generation_bundle= initial_generation_sha256= initial_generation_id=
state_root= release_root= source_root= candidate_root= inbox_root= builder_root= updater_root=
vendor_root= toolchain_root= unit_source_root= system_unit_root= control_root=
user_unit_root=
required_mount= required_mount_uuid=
declare -a steward_owned_specs=() astrid_system_units=() astrid_system_unit_hash_specs=() install_units=() enable_units=()

usage() {
    cat <<'EOF'
usage: install_edge_self_evolution_root.sh [--dry-run] REQUIRED_OPTIONS

Identity/runtime:
  --appliance-id ID --target TARGET --runtime-user USER --runtime-home ABS
  --runtime-workspace ABS --model-ipc ABS
  --steward-owned KIND=ABS (repeat exactly five canonical bindings)
  --model ID --ollama-origin http://127.0.0.1:PORT
  --context-tokens N --output-tokens N --reflection-output-tokens N
  --source-authoring-output-tokens N
  --connect-timeout-ms N
  --header-timeout-ms N --total-timeout-ms N
  --model-lock ABS --autonomy-state ABS --action-receipts ABS
  --thermal-celsius ABS
  --maximum-thermal-celsius N

Immutable executables and trust material:
  --helper ABS --helper-sha256 HEX
  --helper-install-path /usr/libexec/astrid/astrid-edge-steward-helper
  --supervisor ABS --supervisor-sha256 HEX
  --supervisor-install-path /usr/libexec/astrid/edge-self-change-supervisor
  --rescue-helper ABS --rescue-helper-sha256 HEX
  --rescue-helper-install-path /usr/libexec/astrid/astrid-edge-rescue-helper
  --checkpoint ABS --checkpoint-sha256 HEX
  --checkpoint-install-path /usr/libexec/astrid/astrid-edge-checkpoint
  --capsule-builder ABS --capsule-builder-sha256 HEX
  --capsule-builder-install-path /usr/libexec/astrid/astrid-build
  --web-broker ABS --web-broker-sha256 HEX
  --web-broker-install-path /usr/libexec/astrid-edge/immutable/astrid-edge-web-broker
  --provider-broker ABS --provider-broker-sha256 HEX
  --provider-broker-install-path /usr/libexec/astrid-edge/immutable/astrid-edge-provider-broker
  --presentation-broker ABS --presentation-broker-sha256 HEX
  --presentation-broker-install-path /usr/libexec/astrid-edge/immutable/astrid-edge-presentation-broker
  --source-signing-key ABS --source-signing-key-sha256 HEX

Immutable bundles:
  --source-bundle ABS --source-bundle-sha256 HEX
  --toolchain-bundle ABS --toolchain-bundle-sha256 HEX
  --initial-generation-bundle ABS --initial-generation-sha256 HEX
  --initial-generation-id ID

Destinations:
  --state-root ABS --release-root ABS --source-root ABS --candidate-root ABS
  --builder-root ABS --updater-root ABS
  --inbox-root ABS --vendor-root ABS --toolchain-root ABS
  --unit-source-root ABS --system-unit-root /etc/systemd/system
  --user-unit-root ABS (the appliance user's existing systemd/user directory)
  --control-root /usr/sbin --astrid-system-unit ABS (repeat 1..32)
  --astrid-system-unit-sha256 NAME=HEX (repeat for every system unit)
  --install-unit NAME (repeat) --enable-unit NAME (repeat; must be installed)
  --start-system-services (restore active service state after root-unit migration)

ICP requires: --required-mount /media/data --required-mount-uuid UUID

--dry-run performs only read-only validation and is the sole non-root mode.
EOF
}

die() { printf 'error: %s\n' "$*" >&2; exit 2; }
need_value() { [[ $# -ge 2 && -n $2 ]] || die "missing value for $1"; }

while (($#)); do
    case "$1" in
        --dry-run) dry_run=true; shift ;;
        --start-system-services) start_system_services=true; shift ;;
        --appliance-id) need_value "$@"; appliance_id=$2; shift 2 ;;
        --target) need_value "$@"; target=$2; shift 2 ;;
        --runtime-user) need_value "$@"; runtime_user=$2; shift 2 ;;
        --runtime-home) need_value "$@"; runtime_home=$2; shift 2 ;;
        --runtime-workspace) need_value "$@"; runtime_workspace=$2; shift 2 ;;
        --model-ipc) need_value "$@"; model_ipc=$2; shift 2 ;;
        --steward-owned) need_value "$@"; steward_owned_specs+=("$2"); shift 2 ;;
        --model) need_value "$@"; model=$2; shift 2 ;;
        --ollama-origin) need_value "$@"; ollama_origin=$2; shift 2 ;;
        --context-tokens) need_value "$@"; context_tokens=$2; shift 2 ;;
        --output-tokens) need_value "$@"; output_tokens=$2; shift 2 ;;
        --reflection-output-tokens) need_value "$@"; reflection_output_tokens=$2; shift 2 ;;
        --source-authoring-output-tokens) need_value "$@"; source_authoring_output_tokens=$2; shift 2 ;;
        --connect-timeout-ms) need_value "$@"; connect_timeout_ms=$2; shift 2 ;;
        --header-timeout-ms) need_value "$@"; header_timeout_ms=$2; shift 2 ;;
        --total-timeout-ms) need_value "$@"; total_timeout_ms=$2; shift 2 ;;
        --model-lock) need_value "$@"; model_lock=$2; shift 2 ;;
        --autonomy-state) need_value "$@"; autonomy_state=$2; shift 2 ;;
        --action-receipts) need_value "$@"; action_receipts=$2; shift 2 ;;
        --thermal-celsius) need_value "$@"; thermal_celsius=$2; shift 2 ;;
        --maximum-thermal-celsius) need_value "$@"; maximum_thermal_celsius=$2; shift 2 ;;
        --helper) need_value "$@"; helper=$2; shift 2 ;;
        --helper-sha256) need_value "$@"; helper_sha256=$2; shift 2 ;;
        --helper-install-path) need_value "$@"; helper_install_path=$2; shift 2 ;;
        --supervisor) need_value "$@"; supervisor=$2; shift 2 ;;
        --supervisor-sha256) need_value "$@"; supervisor_sha256=$2; shift 2 ;;
        --supervisor-install-path) need_value "$@"; supervisor_install_path=$2; shift 2 ;;
        --rescue-helper) need_value "$@"; rescue_helper=$2; shift 2 ;;
        --rescue-helper-sha256) need_value "$@"; rescue_helper_sha256=$2; shift 2 ;;
        --rescue-helper-install-path) need_value "$@"; rescue_helper_install_path=$2; shift 2 ;;
        --checkpoint) need_value "$@"; checkpoint=$2; shift 2 ;;
        --checkpoint-sha256) need_value "$@"; checkpoint_sha256=$2; shift 2 ;;
        --checkpoint-install-path) need_value "$@"; checkpoint_install_path=$2; shift 2 ;;
        --capsule-builder) need_value "$@"; capsule_builder=$2; shift 2 ;;
        --capsule-builder-sha256) need_value "$@"; capsule_builder_sha256=$2; shift 2 ;;
        --capsule-builder-install-path) need_value "$@"; capsule_builder_install_path=$2; shift 2 ;;
        --web-broker) need_value "$@"; web_broker=$2; shift 2 ;;
        --web-broker-sha256) need_value "$@"; web_broker_sha256=$2; shift 2 ;;
        --web-broker-install-path) need_value "$@"; web_broker_install_path=$2; shift 2 ;;
        --provider-broker) need_value "$@"; provider_broker=$2; shift 2 ;;
        --provider-broker-sha256) need_value "$@"; provider_broker_sha256=$2; shift 2 ;;
        --provider-broker-install-path) need_value "$@"; provider_broker_install_path=$2; shift 2 ;;
        --presentation-broker) need_value "$@"; presentation_broker=$2; shift 2 ;;
        --presentation-broker-sha256) need_value "$@"; presentation_broker_sha256=$2; shift 2 ;;
        --presentation-broker-install-path) need_value "$@"; presentation_broker_install_path=$2; shift 2 ;;
        --source-signing-key) need_value "$@"; source_signing_key=$2; shift 2 ;;
        --source-signing-key-sha256) need_value "$@"; source_signing_key_sha256=$2; shift 2 ;;
        --source-bundle) need_value "$@"; source_bundle=$2; shift 2 ;;
        --source-bundle-sha256) need_value "$@"; source_bundle_sha256=$2; shift 2 ;;
        --toolchain-bundle) need_value "$@"; toolchain_bundle=$2; shift 2 ;;
        --toolchain-bundle-sha256) need_value "$@"; toolchain_bundle_sha256=$2; shift 2 ;;
        --initial-generation-bundle) need_value "$@"; initial_generation_bundle=$2; shift 2 ;;
        --initial-generation-sha256) need_value "$@"; initial_generation_sha256=$2; shift 2 ;;
        --initial-generation-id) need_value "$@"; initial_generation_id=$2; shift 2 ;;
        --state-root) need_value "$@"; state_root=$2; shift 2 ;;
        --release-root) need_value "$@"; release_root=$2; shift 2 ;;
        --source-root) need_value "$@"; source_root=$2; shift 2 ;;
        --candidate-root) need_value "$@"; candidate_root=$2; shift 2 ;;
        --inbox-root) need_value "$@"; inbox_root=$2; shift 2 ;;
        --builder-root) need_value "$@"; builder_root=$2; shift 2 ;;
        --updater-root) need_value "$@"; updater_root=$2; shift 2 ;;
        --vendor-root) need_value "$@"; vendor_root=$2; shift 2 ;;
        --toolchain-root) need_value "$@"; toolchain_root=$2; shift 2 ;;
        --unit-source-root) need_value "$@"; unit_source_root=$2; shift 2 ;;
        --system-unit-root) need_value "$@"; system_unit_root=$2; shift 2 ;;
        --user-unit-root) need_value "$@"; user_unit_root=$2; shift 2 ;;
        --control-root) need_value "$@"; control_root=$2; shift 2 ;;
        --astrid-system-unit) need_value "$@"; astrid_system_units+=("$2"); shift 2 ;;
        --astrid-system-unit-sha256) need_value "$@"; astrid_system_unit_hash_specs+=("$2"); shift 2 ;;
        --install-unit) need_value "$@"; install_units+=("$2"); shift 2 ;;
        --enable-unit) need_value "$@"; enable_units+=("$2"); shift 2 ;;
        --required-mount) need_value "$@"; required_mount=$2; shift 2 ;;
        --required-mount-uuid) need_value "$@"; required_mount_uuid=$2; shift 2 ;;
        -h|--help) usage; exit 0 ;;
        *) die "unsupported argument: $1" ;;
    esac
done

if ! $dry_run && [[ $(id -u) != 0 ]]; then
    die "installation requires root; use --dry-run for validation"
fi

safe_id() { [[ $1 =~ ^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$ && $1 != . && $1 != .. ]]; }
safe_user() { [[ $1 =~ ^[a-z_][a-z0-9_-]{0,30}$ ]]; }
hex64() { [[ $1 =~ ^[0-9a-f]{64}$ ]]; }
unsigned_integer() { [[ $1 =~ ^[0-9]+$ ]]; }

safe_absolute() {
    local path=$1 resolved cursor component suffix=
    [[ $path == /* && $path != / && $path != *$'\n'* && $path != *$'\r'* && $path != *' '* ]] || return 1
    [[ $path != *'/../'* && $path != */.. && $path != *'/./'* && $path != */. && $path != *'//' ]] || return 1
    [[ $path =~ ^/[A-Za-z0-9._/@:+,=-]+(/[A-Za-z0-9._@:+,=-]+)*$ ]] || return 1
    if $dry_run && [[ $(uname -s) != Linux ]] && [[ $path == /etc/* || $path == /usr/* || $path == /var/lib/astrid-edge-volumes* || $path == /var/lib/astrid-edge-origin-mac-retirement ]]; then
        return 0
    fi
    if realpath -m -- / >/dev/null 2>&1; then
        resolved=$(realpath -m -- "$path") || return 1
    else
        cursor=$path
        while [[ ! -e $cursor ]]; do
            component=${cursor##*/}
            suffix=/$component$suffix
            cursor=${cursor%/*}
            [[ -n $cursor ]] || cursor=/
        done
        resolved=$(realpath "$cursor")$suffix || return 1
    fi
    [[ $resolved == "$path" ]]
}

path_within() { [[ $1 == "$2" || $1 == "$2"/* ]]; }

stat_values() {
    if stat -c '%u %a %h' -- "$1" >/dev/null 2>&1; then stat -c '%u %a %h' -- "$1"; else stat -f '%u %Lp %l' -- "$1"; fi
}
sha_file() { sha256sum -- "$1" | awk '{print $1}'; }

operator_report_launcher() {
    local body=$1 view client_format default_window default_limit
    if [[ $body == astrid_train.py ]]; then
        cat <<EOF
#!/bin/sh
PATH=/usr/bin:/bin
export PATH
umask 077
unset PYTHONHOME PYTHONPATH PYTHONSTARTUP
/usr/bin/sha256sum --check --strict --status $OPERATOR_REPORT_MANIFEST || exit 126
for argument in "\$@"; do
    case "\$argument" in
        --workspace|--workspace=*)
            echo 'error: the sealed appliance workspace cannot be overridden' >&2
            exit 64 ;;
    esac
done
exec /usr/bin/python3 -I -E -s $OPERATOR_REPORT_ROOT/astrid_train.py \
    --workspace '$runtime_workspace' "\$@"
EOF
        return
    fi
    case "$body" in
        astrid_at_a_glance.py)
            view=at-a-glance; client_format=text; default_window=180; default_limit=12 ;;
        report_edge_appliance.py)
            view=appliance; client_format=key-value; default_window=20; default_limit=1 ;;
        report_edge_activity.py)
            view=activity; client_format=text; default_window=60; default_limit=100 ;;
        *) die "unsupported immutable operator report body: $body" ;;
    esac
    cat <<EOF
#!/bin/sh
PATH=/usr/bin:/bin
export PATH
umask 077
unset PYTHONHOME PYTHONPATH PYTHONSTARTUP
/usr/bin/sha256sum --check --strict --status $OPERATOR_REPORT_MANIFEST || exit 126

# Nested dashboard calls execute only the sealed report. Candidate-controlled
# presentation is optional decoration and is never admitted back into a
# trusted report, JSON value, health result, or deployment decision.
if [ "\${ASTRID_EDGE_PRESENTATION_SUPPRESS:-}" = 1 ]; then
    exec /usr/bin/python3 -I -E -s $OPERATOR_REPORT_ROOT/$body "\$@"
fi

window_minutes=$default_window
limit=$default_limit
machine_format=false
follow_mode=false
pending=
for argument in "\$@"; do
    if [ "\$pending" = format ]; then
        case "\$argument" in json|jsonl) machine_format=true ;; esac
        pending=
        continue
    fi
    if [ "\$pending" = window ]; then
        case "\$argument" in
            ''|*[!0-9]*) ;;
            *)
                if [ "\${#argument}" -le 4 ] && [ "\$argument" -ge 1 ]; then
                    window_minutes=\$argument
                    [ "\$window_minutes" -le 1440 ] || window_minutes=1440
                fi ;;
        esac
        pending=
        continue
    fi
    if [ "\$pending" = limit ]; then
        case "\$argument" in
            ''|*[!0-9]*) ;;
            *)
                if [ "\${#argument}" -le 3 ] && [ "\$argument" -ge 1 ]; then
                    limit=\$argument
                    [ "\$limit" -le 100 ] || limit=100
                fi ;;
        esac
        pending=
        continue
    fi
    case "\$argument" in
        --format) pending=format ;;
        --format=json|--format=jsonl) machine_format=true ;;
        --window-minutes) pending=window ;;
        --window-minutes=*)
            candidate=\${argument#*=}
            case "\$candidate" in
                ''|*[!0-9]*) ;;
                *)
                    if [ "\${#candidate}" -le 4 ] && [ "\$candidate" -ge 1 ]; then
                        window_minutes=\$candidate
                        [ "\$window_minutes" -le 1440 ] || window_minutes=1440
                    fi ;;
            esac ;;
        --limit) pending=limit ;;
        --limit=*)
            candidate=\${argument#*=}
            case "\$candidate" in
                ''|*[!0-9]*) ;;
                *)
                    if [ "\${#candidate}" -le 3 ] && [ "\$candidate" -ge 1 ]; then
                        limit=\$candidate
                        [ "\$limit" -le 100 ] || limit=100
                    fi ;;
            esac ;;
        --follow) follow_mode=true ;;
    esac
done

export ASTRID_EDGE_PRESENTATION_SUPPRESS=1
if \$follow_mode; then
    exec /usr/bin/python3 -I -E -s $OPERATOR_REPORT_ROOT/$body "\$@"
fi

trusted_output=\$(/usr/bin/mktemp /tmp/astrid-edge-trusted-report.XXXXXX) || exit 126
presentation_output=\$(/usr/bin/mktemp /tmp/astrid-edge-untrusted-presentation.XXXXXX) || {
    /bin/rm -f -- "\$trusted_output"
    exit 126
}
trap '/bin/rm -f -- "\$trusted_output" "\$presentation_output"' EXIT HUP INT TERM
/usr/bin/python3 -I -E -s $OPERATOR_REPORT_ROOT/$body "\$@" >"\$trusted_output"
report_status=\$?
if [ "\$report_status" -ne 0 ]; then
    exit "\$report_status"
fi
/bin/cat -- "\$trusted_output" || exit 125

# JSON and JSONL are deliberately left as one trusted machine-readable value.
# Presentation cannot change the trusted command's exit status.
if \$machine_format; then
    exit 0
fi
if $PRESENTATION_BROKER_INSTALL client \
    --appliance-id '$appliance_id' \
    --view '$view' \
    --window-minutes "\$window_minutes" \
    --limit "\$limit" \
    --format '$client_format' \
    <"\$trusted_output" >"\$presentation_output" 2>/dev/null; then
    /bin/cat -- "\$presentation_output" || true
fi
exit 0
EOF
}

operator_report_manifest() {
    local body launcher body_hash launcher_hash
    for body in astrid_at_a_glance.py astrid_train.py report_edge_appliance.py report_edge_activity.py; do
        body_hash=$(sha_file "$operator_report_source_root/$body")
        case "$body" in
            astrid_at_a_glance.py) launcher=astrid-at-a-glance ;;
            astrid_train.py) launcher=astrid-train ;;
            report_edge_appliance.py) launcher=report-edge-appliance ;;
            report_edge_activity.py) launcher=report-edge-activity ;;
        esac
        launcher_hash=$(operator_report_launcher "$body" | sha256sum | awk '{print $1}')
        printf '%s  %s/%s\n' "$body_hash" "$OPERATOR_REPORT_ROOT" "$body"
        printf '%s  %s/%s\n' "$launcher_hash" "$OPERATOR_REPORT_ROOT" "$launcher"
    done
}

validate_input_file() {
    local path=$1 expected=$2 label=$3 owner mode links
    safe_absolute "$path" || die "$label path is not exact and absolute"
    [[ -f $path && ! -L $path ]] || die "$label must be a regular non-symlink"
    IFS=' ' read -r owner mode links <<<"$(stat_values "$path")"
    [[ $links == 1 ]] || die "$label must not be hard-linked"
    (( (8#$mode & 8#022) == 0 )) || die "$label is group/world writable"
    [[ $owner == 0 || $owner == "$(id -u)" || $owner == "${SUDO_UID:-0}" ]] || die "$label has an untrusted owner"
    hex64 "$expected" || die "$label expected SHA-256 is malformed"
    [[ $(sha_file "$path") == "$expected" ]] || die "$label SHA-256 mismatch"
}

# Copy a previously reviewed input into the private root-owned transaction
# directory without reopening a mutable operator path later in the bootstrap.
# The descriptor identity and all mutation-relevant metadata are checked before
# and after the bounded copy, and the staged digest is checked independently.
stable_stage_file() {
    local source=$1 destination=$2 expected=$3 maximum_bytes=$4 mode=$5 label=$6
    /usr/bin/python3 -I -E -s - "$source" "$destination" "$expected" "$maximum_bytes" "$mode" "${SUDO_UID:-0}" "$label" <<'PY'
import hashlib
import os
import stat
import sys

source, destination, expected, maximum_raw, mode_raw, sudo_uid_raw, label = sys.argv[1:]
maximum = int(maximum_raw)
mode = int(mode_raw, 8)
trusted_owners = {0, int(sudo_uid_raw)}

before = os.lstat(source)
flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
descriptor = os.open(source, flags)
try:
    opened = os.fstat(descriptor)
    identity = lambda value: (
        value.st_dev,
        value.st_ino,
        value.st_mode,
        value.st_nlink,
        value.st_uid,
        value.st_gid,
        value.st_size,
        value.st_mtime_ns,
        value.st_ctime_ns,
    )
    if identity(before) != identity(opened):
        raise SystemExit(f"{label} changed before stable capture")
    if (
        not stat.S_ISREG(opened.st_mode)
        or opened.st_nlink != 1
        or opened.st_uid not in trusted_owners
        or opened.st_mode & 0o022
        or opened.st_size < 0
        or opened.st_size > maximum
    ):
        raise SystemExit(f"{label} escaped stable-capture policy")
    os.makedirs(os.path.dirname(destination), mode=0o700, exist_ok=True)
    output = os.open(
        destination,
        os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_CLOEXEC", 0),
        0o600,
    )
    digest = hashlib.sha256()
    copied = 0
    try:
        while True:
            block = os.read(descriptor, 1024 * 1024)
            if not block:
                break
            copied += len(block)
            if copied > maximum:
                raise SystemExit(f"{label} exceeded stable-capture bound")
            digest.update(block)
            view = memoryview(block)
            while view:
                written = os.write(output, view)
                if written <= 0:
                    raise SystemExit(f"{label} stable capture made no progress")
                view = view[written:]
        if copied != opened.st_size or digest.hexdigest() != expected:
            raise SystemExit(f"{label} stable-capture digest or size mismatch")
        os.fchmod(output, mode)
        os.fchown(output, 0, 0)
        os.fsync(output)
    finally:
        os.close(output)
    after = os.lstat(source)
    if identity(opened) != identity(after):
        raise SystemExit(f"{label} changed during stable capture")
finally:
    os.close(descriptor)
PY
}

create_private_random_key() {
    local destination=$1 label=$2
    /usr/bin/python3 -I -E -s - "$destination" "$label" <<'PY'
import os, stat, sys

destination, label = sys.argv[1:]
parent = os.path.dirname(destination)
flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
descriptor = os.open(destination, flags, 0o400)
try:
    value = os.urandom(32)
    view = memoryview(value)
    while view:
        written = os.write(descriptor, view)
        if written <= 0:
            raise SystemExit(f"{label} creation made no progress")
        view = view[written:]
    os.fchmod(descriptor, 0o400)
    os.fchown(descriptor, 0, 0)
    os.fsync(descriptor)
    opened = os.fstat(descriptor)
    if not stat.S_ISREG(opened.st_mode) or opened.st_nlink != 1 or opened.st_size != 32 or stat.S_IMODE(opened.st_mode) != 0o400 or opened.st_uid != 0:
        raise SystemExit(f"{label} creation failed identity checks")
finally:
    os.close(descriptor)
directory = os.open(parent, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_CLOEXEC", 0))
try:
    os.fsync(directory)
finally:
    os.close(directory)
PY
}

validate_existing() {
    safe_absolute "$1" || die "$2 path is unsafe"
    [[ -e $1 && ! -L $1 ]] || die "$2 is absent or a symlink"
}

require_root_directory() {
    local path=$1 label=$2 owner mode links
    [[ -d $path && ! -L $path ]] || die "$label is not a real directory: $path"
    IFS=' ' read -r owner mode links <<<"$(stat_values "$path")"
    [[ $owner == 0 ]] || die "$label is not root-owned: $path"
    (( (8#$mode & 8#022) == 0 )) || die "$label is group/world writable: $path"
}

require_root_ancestry() {
    local path=$1 anchor=$2 label=$3 leaf_policy=${4:-root} cursor suffix component
    local -a ancestry_components=()
    safe_absolute "$path" || die "$label path is unsafe"
    path_within "$path" "$anchor" || die "$label escapes its immutable anchor"
    require_root_directory "$anchor" "$label anchor"
    suffix=${path#"$anchor"/}
    [[ $suffix != "$path" && -n $suffix ]] || return 0
    cursor=$anchor
    IFS='/' read -r -a ancestry_components <<<"$suffix"
    for component in "${ancestry_components[@]}"; do
        cursor=$cursor/$component
        [[ -e $cursor || -L $cursor ]] || break
        if [[ $cursor == "$path" && $leaf_policy == mutable ]]; then
            [[ -d $cursor && ! -L $cursor ]] || die "$label mutable leaf is not a real directory"
        else
            require_root_directory "$cursor" "$label ancestor"
        fi
    done
}

unit_allowed() {
    case "$1" in
        astrid-edge-self-change-supervisor.service|astrid-edge-self-change-inbox.path|astrid-edge-self-change-probation-health.service|astrid-edge-self-change-probation-health.timer|astrid-edge-steward.service|astrid-edge-steward.timer|astrid-edge-generation-guard.service|astrid-edge-core-liveness.service|astrid-edge-core-liveness.path|astrid-edge-runtime.service.d/60-self-evolution-root.conf) return 0 ;;
        astrid-edge-web-broker-core.socket|astrid-edge-web-broker-core.service|astrid-edge-web-broker-runtime.socket|astrid-edge-web-broker-runtime.service|astrid-edge-web-broker-steward.socket|astrid-edge-web-broker-steward.service) return 0 ;;
        astrid-edge-provider-broker@.service|astrid-edge-provider-runtime.socket|astrid-edge-provider-steward.socket|astrid-edge-provider-warmup.socket) return 0 ;;
        astrid-edge-presentation-broker.socket|astrid-edge-presentation-broker@.service) return 0 ;;
        astrid-edge-audio-feeder.socket|astrid-edge-audio-feeder.service) return 0 ;;
        *) return 1 ;;
    esac
}
unit_source() {
    case "$1" in
        astrid-edge-self-change-supervisor.service|astrid-edge-self-change-probation-health.service|astrid-edge-self-change-probation-health.timer|astrid-edge-steward.service|astrid-edge-steward.timer|astrid-edge-generation-guard.service|astrid-edge-core-liveness.service|astrid-edge-web-broker-core.service|astrid-edge-web-broker-runtime.service|astrid-edge-web-broker-steward.service|astrid-edge-provider-broker@.service) printf '%s/%s\n' "$unit_source_root" "$1" ;;
        astrid-edge-web-broker-core.socket|astrid-edge-web-broker-runtime.socket|astrid-edge-web-broker-steward.socket|astrid-edge-provider-runtime.socket|astrid-edge-provider-steward.socket|astrid-edge-provider-warmup.socket|astrid-edge-self-change-inbox.path|astrid-edge-core-liveness.path|astrid-edge-presentation-broker.socket|astrid-edge-presentation-broker@.service|astrid-edge-audio-feeder.socket) printf '%s/%s.in\n' "$unit_source_root" "$1" ;;
        astrid-edge-audio-feeder.service) printf '%s/%s\n' "$unit_source_root" "$1" ;;
        astrid-edge-runtime.service.d/60-self-evolution-root.conf) printf '%s/root/astrid-edge-runtime-self-evolution.conf.in\n' "$unit_source_root" ;;
        *) return 1 ;;
    esac
}
contains() { local needle=$1 item; shift; for item in "$@"; do [[ $item == "$needle" ]] && return 0; done; return 1; }

for variable in appliance_id target runtime_user runtime_home runtime_workspace model_ipc model ollama_origin \
    context_tokens output_tokens reflection_output_tokens source_authoring_output_tokens connect_timeout_ms header_timeout_ms total_timeout_ms model_lock \
    autonomy_state action_receipts thermal_celsius maximum_thermal_celsius \
    helper helper_sha256 helper_install_path supervisor supervisor_sha256 supervisor_install_path \
    rescue_helper rescue_helper_sha256 rescue_helper_install_path checkpoint checkpoint_sha256 checkpoint_install_path \
    capsule_builder capsule_builder_sha256 capsule_builder_install_path web_broker web_broker_sha256 web_broker_install_path \
    provider_broker provider_broker_sha256 provider_broker_install_path \
    presentation_broker presentation_broker_sha256 presentation_broker_install_path \
    source_signing_key source_signing_key_sha256 source_bundle source_bundle_sha256 toolchain_bundle \
    toolchain_bundle_sha256 initial_generation_bundle initial_generation_sha256 initial_generation_id \
    state_root release_root source_root candidate_root inbox_root builder_root updater_root vendor_root toolchain_root \
    unit_source_root system_unit_root user_unit_root control_root; do
    [[ -n ${!variable} ]] || die "required option is absent: ${variable//_/-}"
done
safe_id "$appliance_id" || die "invalid appliance id"
safe_id "$initial_generation_id" || die "invalid initial generation id"
safe_user "$runtime_user" || die "invalid runtime user"
$start_system_services || die "--start-system-services is required to avoid leaving the migrated appliance offline"
[[ $target == x86_64-unknown-linux-gnu || $target == aarch64-unknown-linux-gnu ]] || die "unsupported target"
[[ $model =~ ^[A-Za-z0-9][A-Za-z0-9._/@:+-]{0,127}$ ]] || die "invalid model"
[[ $ollama_origin =~ ^http://127\.0\.0\.1:[0-9]{1,5}$ ]] || die "Ollama must be exact IPv4 loopback HTTP"
for value in "$context_tokens" "$output_tokens" "$reflection_output_tokens" "$source_authoring_output_tokens" "$connect_timeout_ms" "$header_timeout_ms" "$total_timeout_ms" "$maximum_thermal_celsius"; do unsigned_integer "$value" || die "numeric bound is malformed"; done
((context_tokens >= 1024 && context_tokens <= 8192)) || die "context token bound invalid"
((output_tokens >= 64 && output_tokens <= 512)) || die "output token bound invalid"
((reflection_output_tokens >= 64 && reflection_output_tokens <= 512)) || die "rich-reflection output token bound invalid"
((source_authoring_output_tokens >= 64 && source_authoring_output_tokens <= 512)) || die "source-authoring output token bound invalid"
((reflection_output_tokens >= source_authoring_output_tokens)) || die "rich-reflection ceiling must cover the clean-source ceiling"
((connect_timeout_ms >= 100 && connect_timeout_ms <= 30000)) || die "connect timeout invalid"
((header_timeout_ms >= 1000 && header_timeout_ms <= 600000)) || die "header timeout invalid"
((total_timeout_ms > header_timeout_ms && total_timeout_ms <= 660000)) || die "total timeout invalid"
((maximum_thermal_celsius >= 50 && maximum_thermal_celsius <= 95)) || die "thermal bound invalid"
((${#steward_owned_specs[@]} == 5)) || die "require exact five canonical steward-owned inputs"
((${#astrid_system_units[@]} >= 1 && ${#astrid_system_units[@]} <= 32)) || die "require 1..32 Astrid unit paths"
((${#install_units[@]} >= 1)) || die "at least one requested unit is required"

for path in "$runtime_home" "$runtime_workspace" "$model_ipc" "$model_lock" "$autonomy_state" "$action_receipts" \
    "$thermal_celsius" "$helper_install_path" \
    "$supervisor_install_path" "$rescue_helper_install_path" "$checkpoint_install_path" "$capsule_builder_install_path" "$web_broker_install_path" "$provider_broker_install_path" "$presentation_broker_install_path" \
    "$state_root" "$release_root" "$source_root" "$candidate_root" "$builder_root" "$updater_root" \
    "$inbox_root" "$vendor_root" "$toolchain_root" "$unit_source_root" "$system_unit_root" "$user_unit_root" "$control_root"; do
    safe_absolute "$path" || die "unsafe or non-canonical path: $path"
done
[[ $helper_install_path == "$STEWARD_INSTALL" ]] || die "native helper install path does not match unit contract"
[[ $supervisor_install_path == "$SUPERVISOR_INSTALL" ]] || die "supervisor install path does not match unit contract"
[[ $rescue_helper_install_path == "$RESCUE_INSTALL" ]] || die "rescue-helper install path does not match immutable profile contract"
[[ $checkpoint_install_path == "$CHECKPOINT_INSTALL" ]] || die "checkpoint install path does not match rescue contract"
[[ $capsule_builder_install_path == "$CAPSULE_BUILDER_INSTALL" ]] || die "capsule-builder install path does not match rescue contract"
[[ $web_broker_install_path == "$WEB_BROKER_INSTALL" ]] || die "web-broker install path does not match immutable service contract"
[[ $provider_broker_install_path == "$PROVIDER_BROKER_INSTALL" ]] || die "provider-broker install path does not match immutable service contract"
[[ $presentation_broker_install_path == "$PRESENTATION_BROKER_INSTALL" ]] || die "presentation-broker install path does not match immutable service contract"
[[ $system_unit_root == /etc/systemd/system ]] || die "system unit root must be /etc/systemd/system"
[[ $control_root == /usr/sbin ]] || die "control root must be /usr/sbin"
[[ $user_unit_root == "$runtime_home/.config/systemd/user" ]] || die "user unit root must be the explicit runtime home's systemd/user directory"
[[ $vendor_root == "$source_root/vendor" ]] || die "vendor root must be the signed source bundle's vendor subtree"
[[ $inbox_root == "$state_root/inbox" ]] || die "inbox root must be the supervisor's exact state-root/inbox path"
if [[ $appliance_id == icp* && ( $required_mount != /media/data || -z $required_mount_uuid ) ]]; then die "ICP requires exact /media/data UUID guard"; fi
GENERATION_FILE=$state_root/current-generation
SUPERVISOR_STATUS=$state_root/steward-status.json
[[ $model_lock == "$state_root/model.lock" ]] || die "model lock must be the exact persistent root-state lock"
candidate_store=$candidate_root/candidate-outbox
model_handoff_root=$candidate_root/model-handoff
model_handoff_ledger=$candidate_root/model-unload-receipts.jsonl
scheduled_authorship_root=$candidate_root/scheduled-authorship
candidate_work=$builder_root/work
build_store=$builder_root/builds
state_snapshots=$updater_root/snapshots
system_unit_alias=$updater_root/system-units
profile_transactions=$state_snapshots/profile-transactions
generation_staging=$updater_root/generation-staging
maintenance_lease=$state_root/maintenance.json
maintenance_mutex=$state_root/maintenance.lock
unit_policy=$state_root/unit-policy.json
unit_transactions=$state_snapshots/unit-transactions
introspection_evidence_root=$state_root/introspection-evidence
inquiry_history_root=${state_root%/*}/astrid-edge-inquiry-history
build_evidence_root=$introspection_evidence_root/build-evidence
generation_diffs_root=$introspection_evidence_root/generation-diffs
safe_absolute "$system_unit_alias" || die "private system-unit alias path is unsafe"
safe_absolute "$inquiry_history_root" || die "private inquiry-history path is unsafe"
[[ $inquiry_history_root != "$state_root" && $inquiry_history_root != "$candidate_root" ]] \
    || die "inquiry history must be a dedicated sibling root"
[[ $system_unit_alias == "$updater_root/system-units" ]] || die "private system-unit alias escaped updater root"
[[ $runtime_workspace == */home/default/edge ]] || die "runtime workspace must end in home/default/edge"
astrid_state_root=${runtime_workspace%/home/default/edge}
origin_mac_workspace_root=$(readlink -f -- "${runtime_workspace%/edge}") \
    || die "cannot resolve exact origin-mac migration workspace"
if [[ $appliance_id == icp* ]]; then
    # Directly below the root-owned SSD mount: never below the appliance
    # user's writable /media/data/astrid tree.
    origin_mac_retirement_root=/media/data/.astrid-edge-origin-mac-retirement
else
    # AVADO's workspace and /var/lib share the root filesystem.  Keep the
    # correction outside every user-owned home ancestor.
    origin_mac_retirement_root=/var/lib/astrid-edge-origin-mac-retirement
fi
safe_absolute "$origin_mac_retirement_root" || die "origin-mac retirement root is unsafe"
[[ $origin_mac_retirement_root != "$origin_mac_workspace_root" && $origin_mac_retirement_root != "$origin_mac_workspace_root"/* ]] \
    || die "origin-mac retirement root must remain outside the accessible workspace"
maintenance_edge_acknowledgement=$runtime_workspace/runtime/maintenance-edge-ack.json
maintenance_core_acknowledgement=$astrid_state_root/run/maintenance-core-ack.json
sensor_state=$runtime_workspace/runtime/spectral_state.json
hindsight_state=$astrid_state_root/operator/hindsight/latest.json
fill_history=$runtime_workspace/runtime/fill_history.jsonl
web_receipts=$runtime_workspace/web/receipts.jsonl
introspection_receipts=$runtime_workspace/introspection/receipts.jsonl
steward_reflection_root=$runtime_workspace/introspections/scheduled
scheduled_introspection_root=$runtime_workspace/runtime/scheduled-introspection
steward_projection_root=$scheduled_introspection_root/projection
runtime_admission_root=$scheduled_introspection_root/admission
self_change_liveness_root=$runtime_workspace/runtime
self_change_root=$runtime_workspace/self-change
runtime_self_change_outbox=$self_change_root/outbox
steward_patch_outbox=$runtime_workspace/self-change/patch-outbox
declare -a steward_output_roots=("$steward_reflection_root" "$steward_projection_root" "$steward_patch_outbox")
declare -a steward_traverse_roots=("${steward_reflection_root%/*}" "$scheduled_introspection_root" "$self_change_root")
declare -a protected_steward_parents=("${steward_reflection_root%/*}" "$scheduled_introspection_root" "$self_change_root")
for path in "$runtime_workspace" "$model_ipc" "$autonomy_state" "$action_receipts" "$thermal_celsius" "$unit_source_root"; do validate_existing "$path" "runtime input"; done
[[ ! -e $model_lock && ! -L $model_lock ]] || die "persistent root-state model lock already exists"
for gate_path in "$autonomy_state" "$action_receipts" "$thermal_celsius"; do
    [[ -f $gate_path && ! -L $gate_path ]] || die "gate input must be a regular non-symlink: $gate_path"
done
for health_path in "$sensor_state" "$hindsight_state" "$fill_history" "$web_receipts" "$introspection_receipts"; do
    [[ -f $health_path && ! -L $health_path ]] || die "rescue health/activity input must be a regular non-symlink: $health_path"
done
runtime_uid=$(id -u "$runtime_user" 2>/dev/null) || die "runtime user does not exist"
runtime_gid=$(id -g "$runtime_user" 2>/dev/null) || die "runtime user group does not exist"
runtime_group=$(id -gn "$runtime_user" 2>/dev/null) || die "runtime group name does not exist"
((runtime_uid > 0 && runtime_gid > 0)) || die "runtime identity must be unprivileged"
safe_user "$runtime_group" || die "runtime primary group name is unsafe for unit rendering"
[[ $(stat_values "$runtime_home" | awk '{print $1}') == "$runtime_uid" ]] || die "runtime home is not owned by runtime user"
[[ $(stat_values "$runtime_workspace" | awk '{print $1}') == "$runtime_uid" ]] || die "workspace is not owned by runtime user"
ollama_launcher=$runtime_home/.local/bin/ollama
ollama_model_root=$runtime_home/.local/share/ollama/models
ollama_runtime_root=
if [[ $appliance_id == icp* ]]; then
    icp_link=$runtime_home/.astrid-icp
    [[ -L $icp_link ]] || die "ICP appliance root must remain the reviewed home symlink"
    icp_root=$(readlink -f -- "$icp_link") || die "cannot resolve the ICP appliance root"
    safe_absolute "$icp_root" || die "resolved ICP appliance root is unsafe"
    $dry_run || [[ $icp_root == /media/data/astrid ]] || die "ICP appliance symlink must resolve to exact /media/data/astrid"
    ollama_launcher=$icp_root/ollama/runtime/bin/ollama
    ollama_model_root=$icp_root/ollama/models
    ollama_runtime_root=$icp_root/ollama/runtime
    [[ $runtime_workspace == "$icp_root/state/home/default/edge" ]] \
        || die "ICP runtime workspace must use the canonical SSD root"
fi
[[ -e $ollama_launcher || -L $ollama_launcher ]] || die "profile Ollama launcher is absent: $ollama_launcher"
ollama_binary=$(readlink -f -- "$ollama_launcher") || die "profile Ollama launcher cannot be resolved"
safe_absolute "$ollama_binary" || die "resolved Ollama binary is not an exact absolute path"
if [[ $appliance_id == icp* ]]; then
    [[ $ollama_binary == "$ollama_runtime_root/bin/ollama" && ! -L $ollama_launcher ]] \
        || die "ICP Ollama binary is outside its exact SSD runtime root"
else
    ollama_runtime_root=${ollama_binary%/bin/ollama}
    [[ $ollama_binary == "$ollama_runtime_root/bin/ollama" \
        && $ollama_runtime_root == "$runtime_home/.local/"ollama-v* \
        && ${ollama_runtime_root##*/} =~ ^ollama-v[0-9]+\.[0-9]+\.[0-9]+$ ]] \
        || die "AVADO Ollama launcher target is outside its exact versioned runtime root"
fi
[[ -f $ollama_binary && ! -L $ollama_binary && -x $ollama_binary ]] || die "profile Ollama binary is not an executable regular file"
IFS=' ' read -r ollama_owner ollama_mode ollama_links <<<"$(stat_values "$ollama_binary")"
[[ $ollama_owner == "$runtime_uid" && $ollama_links == 1 ]] || die "profile Ollama binary identity is not runtime-owned and single-linked"
(( (8#$ollama_mode & 8#022) == 0 )) || die "profile Ollama binary is group/world writable"
ollama_binary_sha256=$(sha_file "$ollama_binary")
hex64 "$ollama_binary_sha256" || die "profile Ollama binary digest is malformed"
for directory_spec in "$ollama_runtime_root:runtime" "$ollama_model_root:models"; do
    directory=${directory_spec%:*}; label=${directory_spec##*:}
    [[ -d $directory && ! -L $directory ]] || die "profile Ollama $label root is absent or a symlink"
    IFS=' ' read -r directory_owner directory_mode directory_links <<<"$(stat_values "$directory")"
    [[ $directory_owner == "$runtime_uid" && $directory_links -ge 1 ]] || die "profile Ollama $label root identity is invalid"
    (( (8#$directory_mode & 8#022) == 0 )) || die "profile Ollama $label root is group/world writable"
done
for gate_path in "$autonomy_state" "$action_receipts"; do path_within "$gate_path" "$runtime_workspace" || die "gate path escapes workspace"; done
for output_root in "${steward_output_roots[@]}" "$runtime_admission_root"; do
    safe_absolute "$output_root" || die "self-change output path is unsafe"
    path_within "$output_root" "$runtime_workspace" || die "self-change output escapes workspace"
    [[ ! -e $output_root && ! -L $output_root ]] || die "refusing to overwrite a reserved self-change output: $output_root"
done
for parent in "${steward_reflection_root%/*}" "${scheduled_introspection_root%/*}" "$runtime_workspace"; do
    validate_existing "$parent" "self-change output parent"
    [[ -d $parent && ! -L $parent && $(stat_values "$parent" | awk '{print $1}') == "$runtime_uid" ]] || die "self-change output parent is not a runtime-owned directory"
done
for parent in "$scheduled_introspection_root" "$self_change_root"; do
    if [[ -e $parent ]]; then
        [[ -d $parent && ! -L $parent && $(stat_values "$parent" | awk '{print $1}') == "$runtime_uid" ]] || die "reserved parent is not a runtime-owned directory"
    fi
done
if [[ -e $runtime_self_change_outbox || -L $runtime_self_change_outbox ]]; then
    [[ -d $runtime_self_change_outbox && ! -L $runtime_self_change_outbox \
        && $(stat_values "$runtime_self_change_outbox" | awk '{print $1}') == "$runtime_uid" ]] \
        || die "runtime self-change outbox is not a runtime-owned directory"
fi

declare -a owned_kinds=() owned_paths=()
for spec in "${steward_owned_specs[@]}"; do
    kind=${spec%%=*}; path=${spec#*=}
    [[ $kind != "$spec" ]] || die "steward-owned entry must be KIND=ABS"
    safe_id "$kind" || die "invalid steward-owned kind"
    if ((${#owned_kinds[@]} > 0)) && contains "$kind" "${owned_kinds[@]}"; then die "duplicate steward-owned kind"; fi
    case "$kind" in
        continuity) expected_owned_path=$runtime_workspace/autonomous/thread_state.json ;;
        self_profile) expected_owned_path=$runtime_workspace/self/profile.json ;;
        verified_evidence) expected_owned_path=$runtime_workspace/autonomous/thread_state.jsonl ;;
        machine_observation) expected_owned_path=$runtime_workspace/perception/latest.json ;;
        spectral_host_state) expected_owned_path=$runtime_workspace/runtime/spectral_state.json ;;
        *) die "steward-owned kind is outside the exact canonical introspection contract" ;;
    esac
    safe_absolute "$path" || die "steward-owned input path is unsafe"
    [[ $path == "$expected_owned_path" ]] || die "steward-owned input does not match its canonical workspace path"
    owned_parent=${path%/*}
    [[ -d $owned_parent && ! -L $owned_parent ]] || die "steward-owned input parent is absent, linked, or non-directory"
    [[ $(stat_values "$owned_parent" | awk '{print $1}') == "$runtime_uid" ]] || die "steward-owned input parent owner mismatch"
    if [[ -e $path || -L $path ]]; then
        [[ -f $path && ! -L $path ]] || die "existing steward-owned input must be a regular non-symlink"
        [[ $(stat_values "$path" | awk '{print $1}') == "$runtime_uid" ]] || die "steward-owned input owner mismatch"
    fi
    owned_kinds+=("$kind"); owned_paths+=("$path")
done

readonly -a required_system_stack=(ollama-cpu.service astrid-model-warmup.service astrid.service astrid-edge-runtime.service astrid-edge-hindsight.service astrid-edge-hindsight.timer)
((${#astrid_system_units[@]} == ${#required_system_stack[@]})) || die "the exact six-unit Astrid system stack must be authorized"
((${#astrid_system_unit_hash_specs[@]} == ${#required_system_stack[@]})) || die "every Astrid system unit requires an explicit SHA-256"
for path in "${astrid_system_units[@]}"; do
    safe_absolute "$path" || die "Astrid unit path is unsafe"
    path_within "$path" "$system_unit_root" || die "Astrid unit escapes system unit root"
    [[ ${path##*/} == astrid*.service || ${path##*/} == astrid*.timer || ${path##*/} == ollama-cpu.service ]] || die "non-Astrid appliance unit refused"
done
for unit in "${required_system_stack[@]}"; do contains "$system_unit_root/$unit" "${astrid_system_units[@]}" || die "authorized system stack omits $unit"; done
declare -a system_stack_hashes=()
for unit in "${required_system_stack[@]}"; do
    matched_hash=
    for spec in "${astrid_system_unit_hash_specs[@]}"; do
        [[ ${spec%%=*} == "$unit" ]] || continue
        [[ -z $matched_hash ]] || die "duplicate system-unit SHA-256 for $unit"
        matched_hash=${spec#*=}
    done
    hex64 "$matched_hash" || die "missing or invalid system-unit SHA-256 for $unit"
    profile_source_root=$unit_source_root; [[ $appliance_id == icp* ]] && profile_source_root=$unit_source_root/icp
    validate_input_file "$profile_source_root/$unit" "$matched_hash" "authorized system unit"
    system_stack_hashes+=("$matched_hash")
done
migrator=$unit_source_root/root/migrate-edge-user-services-to-system
migrator_sha256=$(sha_file "$migrator")
validate_input_file "$migrator" "$migrator_sha256" "system-service migrator"
builder_store_helper=$unit_source_root/root/astrid-edge-builder-store
builder_store_mount_template=$unit_source_root/root/astrid-edge-builder-store.mount.in
builder_store_verify_template=$unit_source_root/astrid-edge-builder-store-verify.service.in
system_unit_alias_mount_template=$unit_source_root/root/astrid-edge-system-units-alias.mount.in
state_store_helper=$unit_source_root/root/astrid-edge-state-store
state_store_runtime_mount_template=$unit_source_root/root/astrid-edge-state-store-runtime.mount.in
state_store_rollback_mount_template=$unit_source_root/root/astrid-edge-state-store-rollback.mount.in
state_store_migration_recover_template=$unit_source_root/astrid-edge-state-store-migration-recover.service.in
state_store_recover_template=$unit_source_root/astrid-edge-state-store-recover.service.in
state_store_verify_template=$unit_source_root/astrid-edge-state-store-verify.service.in
state_store_health_service=$unit_source_root/astrid-edge-state-store-health.service
state_store_health_timer=$unit_source_root/astrid-edge-state-store-health.timer
state_store_bounded_dropin_template=$unit_source_root/astrid-edge-bounded-state.conf.in
builder_store_helper_sha256=$(sha_file "$builder_store_helper")
builder_store_mount_template_sha256=$(sha_file "$builder_store_mount_template")
builder_store_verify_template_sha256=$(sha_file "$builder_store_verify_template")
system_unit_alias_mount_template_sha256=$(sha_file "$system_unit_alias_mount_template")
state_store_helper_sha256=$(sha_file "$state_store_helper")
state_store_runtime_mount_template_sha256=$(sha_file "$state_store_runtime_mount_template")
state_store_rollback_mount_template_sha256=$(sha_file "$state_store_rollback_mount_template")
state_store_migration_recover_template_sha256=$(sha_file "$state_store_migration_recover_template")
state_store_recover_template_sha256=$(sha_file "$state_store_recover_template")
state_store_verify_template_sha256=$(sha_file "$state_store_verify_template")
state_store_health_service_sha256=$(sha_file "$state_store_health_service")
state_store_health_timer_sha256=$(sha_file "$state_store_health_timer")
state_store_bounded_dropin_template_sha256=$(sha_file "$state_store_bounded_dropin_template")
validate_input_file "$builder_store_helper" "$builder_store_helper_sha256" "builder-store helper"
validate_input_file "$builder_store_mount_template" "$builder_store_mount_template_sha256" "builder-store mount template"
validate_input_file "$builder_store_verify_template" "$builder_store_verify_template_sha256" "builder-store verifier template"
validate_input_file "$system_unit_alias_mount_template" "$system_unit_alias_mount_template_sha256" "private system-unit alias mount template"
validate_input_file "$state_store_helper" "$state_store_helper_sha256" "state-store helper"
validate_input_file "$state_store_runtime_mount_template" "$state_store_runtime_mount_template_sha256" "runtime-state mount template"
validate_input_file "$state_store_rollback_mount_template" "$state_store_rollback_mount_template_sha256" "rollback-state mount template"
validate_input_file "$state_store_migration_recover_template" "$state_store_migration_recover_template_sha256" "state-store migration recovery template"
validate_input_file "$state_store_recover_template" "$state_store_recover_template_sha256" "state-store recovery template"
validate_input_file "$state_store_verify_template" "$state_store_verify_template_sha256" "state-store verifier template"
validate_input_file "$state_store_health_service" "$state_store_health_service_sha256" "state-store health service"
validate_input_file "$state_store_health_timer" "$state_store_health_timer_sha256" "state-store health timer"
validate_input_file "$state_store_bounded_dropin_template" "$state_store_bounded_dropin_template_sha256" "bounded-state drop-in template"
[[ -x $builder_store_helper && $(LC_ALL=C head -c2 "$builder_store_helper") == '#!' ]] || die "builder-store helper must be an executable script"
[[ -x $state_store_helper && $(LC_ALL=C head -c2 "$state_store_helper") == '#!' ]] || die "state-store helper must be an executable script"
operator_report_source_root=$(readlink -f -- "$unit_source_root/../../scripts") || die "cannot resolve immutable operator report sources"
safe_absolute "$operator_report_source_root" || die "immutable operator report source root is not exact"
for operator_body in astrid_at_a_glance.py astrid_train.py report_edge_appliance.py report_edge_activity.py; do
    operator_body_path=$operator_report_source_root/$operator_body
    validate_input_file "$operator_body_path" "$(sha_file "$operator_body_path")" "immutable operator report body"
done
operator_report_manifest_content=$(operator_report_manifest)
operator_report_manifest_sha256=$(printf '%s\n' "$operator_report_manifest_content" | sha256sum | awk '{print $1}')
hex64 "$operator_report_manifest_sha256" || die "immutable operator report manifest digest is malformed"
presentation_config_template=$(readlink -f -- "$unit_source_root/../headless/edge-presentation-broker.json.in") \
    || die "cannot resolve presentation-broker config template"
presentation_config_template_sha256=$(sha_file "$presentation_config_template")
validate_input_file "$presentation_config_template" "$presentation_config_template_sha256" "presentation-broker config template"
audio_config_template=$(readlink -f -- "$unit_source_root/../headless/edge-audio-feeder.json.in") \
    || die "cannot resolve audio-feeder config template"
audio_config_template_sha256=$(sha_file "$audio_config_template")
validate_input_file "$audio_config_template" "$audio_config_template_sha256" "audio-feeder config template"
hindsight_config_template=$(readlink -f -- "$unit_source_root/../headless/edge-hindsight-writer.json.in") \
    || die "cannot resolve hindsight-writer config template"
hindsight_config_template_sha256=$(sha_file "$hindsight_config_template")
validate_input_file "$hindsight_config_template" "$hindsight_config_template_sha256" "hindsight-writer config template"
audio_feeder_source=$(readlink -f -- "$unit_source_root/../../scripts/edge_audio_feeder.py") \
    || die "cannot resolve immutable audio feeder source"
audio_feeder_source_sha256=$(sha_file "$audio_feeder_source")
validate_input_file "$audio_feeder_source" "$audio_feeder_source_sha256" "immutable audio feeder source"
control_source=$unit_source_root/root/astrid-edge-self-evolution-control
control_source_sha256=$(sha_file "$control_source")
validate_input_file "$control_source" "$control_source_sha256" "control wrapper"
authority_disabled_source=$unit_source_root/astrid-edge-self-change-disabled.env
authority_enabled_source=$unit_source_root/astrid-edge-self-change-enabled.env
authority_disabled_sha256=$(sha_file "$authority_disabled_source")
authority_enabled_sha256=$(sha_file "$authority_enabled_source")
validate_input_file "$authority_disabled_source" "$authority_disabled_sha256" "disabled self-change authority"
validate_input_file "$authority_enabled_source" "$authority_enabled_sha256" "enabled self-change authority"
[[ $(<"$authority_disabled_source") == 'ASTRID_EDGE_SELF_CHANGE_ENABLED=false' ]] || die "disabled self-change authority is not exact"
[[ $(<"$authority_enabled_source") == 'ASTRID_EDGE_SELF_CHANGE_ENABLED=true' ]] || die "enabled self-change authority is not exact"
declare -a profile_source_names=() profile_source_hashes=()
if [[ $appliance_id == icp* ]]; then
    profile_source_names=(icp-ssd-required.conf astrid-edge-tuning-authority.conf wait-for-icp-ssd)
else
    profile_source_names=(astrid-local-ollama.conf)
fi
for profile_source_name in "${profile_source_names[@]}"; do
    profile_source_path=$unit_source_root/$profile_source_name
    profile_source_hash=$(sha_file "$profile_source_path")
    validate_input_file "$profile_source_path" "$profile_source_hash" "appliance profile source"
    profile_source_hashes+=("$profile_source_hash")
done

validate_input_file "$helper" "$helper_sha256" "native steward helper"
[[ $(LC_ALL=C od -An -tx1 -N4 -- "$helper" | tr -d ' \n') == 7f454c46 && -x $helper ]] || die "native steward helper must be executable ELF"
validate_input_file "$supervisor" "$supervisor_sha256" "self-contained Python supervisor"
[[ -x $supervisor ]] || die "supervisor executable is not executable"
supervisor_prefix=$(LC_ALL=C head -c2 "$supervisor")
[[ $supervisor_prefix == '#!' ]] || die "self-contained Python supervisor must have an executable shebang"
for executable_spec in \
    "$rescue_helper|$rescue_helper_sha256|rescue helper" \
    "$checkpoint|$checkpoint_sha256|checkpoint helper" \
    "$capsule_builder|$capsule_builder_sha256|capsule builder" \
    "$web_broker|$web_broker_sha256|immutable web broker" \
    "$provider_broker|$provider_broker_sha256|immutable provider broker" \
    "$presentation_broker|$presentation_broker_sha256|immutable presentation broker"; do
    IFS='|' read -r executable_path executable_hash executable_label <<<"$executable_spec"
    validate_input_file "$executable_path" "$executable_hash" "$executable_label"
    [[ $(LC_ALL=C od -An -tx1 -N4 -- "$executable_path" | tr -d ' \n') == 7f454c46 && -x $executable_path ]] || die "$executable_label must be an executable ELF"
done
validate_input_file "$source_signing_key" "$source_signing_key_sha256" "source signing key"
[[ $(stat_values "$source_signing_key" | awk '{print $2}') =~ ^[046]00$ && $(wc -c <"$source_signing_key" | tr -d ' ') == 32 ]] || die "source signing key must be exact 32-byte owner-only material"
validate_input_file "$source_bundle" "$source_bundle_sha256" "source bundle"
validate_input_file "$toolchain_bundle" "$toolchain_bundle_sha256" "toolchain bundle"
validate_input_file "$initial_generation_bundle" "$initial_generation_sha256" "initial generation bundle"

declare -a disjoint_roots=("$state_root" "$release_root" "$source_root" "$candidate_root" "$builder_root" "$updater_root" "$toolchain_root")
for ((left=0; left<${#disjoint_roots[@]}; left++)); do
    for ((right=left+1; right<${#disjoint_roots[@]}; right++)); do
        path_within "${disjoint_roots[$left]}" "${disjoint_roots[$right]}" && die "destination roots overlap"
        path_within "${disjoint_roots[$right]}" "${disjoint_roots[$left]}" && die "destination roots overlap"
    done
done
for path in "${disjoint_roots[@]}"; do
    { path_within "$path" "$origin_mac_retirement_root" || path_within "$origin_mac_retirement_root" "$path"; } \
        && die "origin-mac retirement root overlaps a mutable self-evolution root"
done

if [[ $appliance_id == icp* ]]; then
    [[ $required_mount == /media/data && -n $required_mount_uuid ]] || die "ICP requires exact /media/data UUID guard"
    if ! $dry_run; then
        [[ $state_root == /media/data/astrid-edge-supervisor ]] || die "ICP supervisor root must be the fixed root-owned SSD sibling"
        [[ $release_root == /media/data/astrid-edge-release-store/releases ]] || die "ICP release root must be the fixed root-owned SSD sibling"
        [[ $source_root == /media/data/astrid-edge-source ]] || die "ICP source root must be the fixed root-owned SSD sibling"
        [[ $candidate_root == /media/data/astrid-edge-candidates ]] || die "ICP candidate root must be the fixed root-owned SSD sibling"
        [[ $builder_root == /media/data/astrid-edge-builder ]] || die "ICP builder root must be the fixed root-owned SSD sibling"
        [[ $updater_root == /media/data/astrid-edge-updater ]] || die "ICP updater root must be the fixed root-owned SSD sibling"
        [[ $toolchain_root == /media/data/astrid-edge-toolchain ]] || die "ICP toolchain root must be the fixed root-owned SSD sibling"
    fi
else
    $dry_run || [[ $builder_root == /var/lib/astrid-edge-builder ]] || die "AVADO builder root must use the fixed persistent path"
fi
if [[ -n $required_mount || -n $required_mount_uuid ]]; then
    [[ -n $required_mount && -n $required_mount_uuid ]] || die "mount path and UUID must be paired"
    safe_absolute "$required_mount" || die "required mount path is unsafe"
    [[ $required_mount_uuid =~ ^[A-Fa-f0-9-]{4,64}$ ]] || die "mount UUID is malformed"
fi
backup_root=/media/data/astrid/backups
for path in "${disjoint_roots[@]}"; do
    { path_within "$path" "$backup_root" || path_within "$backup_root" "$path"; } && die "managed root overlaps retained backup tree"
done

if [[ $appliance_id == icp* ]]; then
    contains "$AUDIO_SOCKET_UNIT" "${install_units[@]}" && die "ICP must not install the AVADO audio feeder"
    contains "$AUDIO_SERVICE_UNIT" "${install_units[@]}" && die "ICP must not install the AVADO audio feeder"
    contains "$AUDIO_SOCKET_UNIT" "${enable_units[@]}" && die "ICP must not enable the AVADO audio feeder"
else
    contains "$AUDIO_SOCKET_UNIT" "${install_units[@]}" || install_units+=("$AUDIO_SOCKET_UNIT")
    contains "$AUDIO_SERVICE_UNIT" "${install_units[@]}" || install_units+=("$AUDIO_SERVICE_UNIT")
    contains "$AUDIO_SOCKET_UNIT" "${enable_units[@]}" || enable_units+=("$AUDIO_SOCKET_UNIT")
fi

declare -a unique_units=() unit_template_names=() unit_template_hashes=()
for unit in "${install_units[@]}"; do
    unit_allowed "$unit" || die "unsupported root-boundary unit: $unit"
    source_path=$(unit_source "$unit")
    source_hash=$(sha_file "$source_path")
    validate_input_file "$source_path" "$source_hash" "unit template"
    if ((${#unique_units[@]} == 0)) || ! contains "$unit" "${unique_units[@]}"; then
        unique_units+=("$unit")
        unit_template_names+=("$unit")
        unit_template_hashes+=("$source_hash")
    fi
done
install_units=("${unique_units[@]}")
contains astrid-edge-self-change-supervisor.service "${install_units[@]}" && ! contains astrid-edge-steward.service "${install_units[@]}" && die "supervisor requires native steward service"
contains astrid-edge-steward.timer "${install_units[@]}" && ! contains astrid-edge-self-change-supervisor.service "${install_units[@]}" && die "steward timer requires supervisor"
contains astrid-edge-steward.service "${install_units[@]}" && ! contains "$WEB_STEWARD_SOCKET_UNIT" "${install_units[@]}" && die "steward requires its isolated immutable web-broker socket"
contains astrid-edge-steward.service "${install_units[@]}" && ! contains "$PROVIDER_STEWARD_SOCKET_UNIT" "${install_units[@]}" && die "steward requires its isolated immutable provider socket"
contains "$WEB_CORE_SERVICE_UNIT" "${install_units[@]}" && ! contains "$WEB_CORE_SOCKET_UNIT" "${install_units[@]}" && die "core web broker requires its isolated immutable socket"
contains "$PRESENTATION_SERVICE_TEMPLATE" "${install_units[@]}" && ! contains "$PRESENTATION_SOCKET_UNIT" "${install_units[@]}" && die "presentation broker service requires its root-owned socket"
contains astrid-edge-self-change-probation-health.timer "${install_units[@]}" && ! contains astrid-edge-self-change-probation-health.service "${install_units[@]}" && die "probation timer requires its immutable sampler"
contains astrid-edge-self-change-probation-health.service "${install_units[@]}" && ! contains astrid-edge-self-change-supervisor.service "${install_units[@]}" && die "probation sampler requires the immutable supervisor"
contains "$CORE_LIVENESS_PATH_UNIT" "${install_units[@]}" && ! contains astrid-edge-core-liveness.service "${install_units[@]}" && die "core-liveness watcher requires its immutable recovery oneshot"
contains "$SELF_CHANGE_INBOX_PATH_UNIT" "${install_units[@]}" && ! contains astrid-edge-self-change-supervisor.service "${install_units[@]}" && die "candidate handoff watcher requires the immutable supervisor"
for unit in ${enable_units[@]+"${enable_units[@]}"}; do
    contains "$unit" "${install_units[@]}" || die "enabled unit was not requested"
    case "$unit" in *.service.d/*|astrid-edge-steward.service|astrid-edge-self-change-probation-health.service|astrid-edge-core-liveness.service|astrid-edge-web-broker-*.service|astrid-edge-provider-broker@.service|astrid-edge-presentation-broker@.service) die "unit cannot be enabled directly: $unit";; esac
done
for required_unit in astrid-edge-self-change-supervisor.service astrid-edge-self-change-probation-health.service astrid-edge-self-change-probation-health.timer astrid-edge-steward.service astrid-edge-steward.timer \
    "$WEB_CORE_SOCKET_UNIT" "$WEB_CORE_SERVICE_UNIT" "$WEB_RUNTIME_SOCKET_UNIT" "$WEB_RUNTIME_SERVICE_UNIT" "$WEB_STEWARD_SOCKET_UNIT" "$WEB_STEWARD_SERVICE_UNIT" \
    "$PROVIDER_SERVICE_TEMPLATE" "$PROVIDER_RUNTIME_SOCKET_UNIT" "$PROVIDER_STEWARD_SOCKET_UNIT" "$PROVIDER_WARMUP_SOCKET_UNIT" \
    "$PRESENTATION_SOCKET_UNIT" "$PRESENTATION_SERVICE_TEMPLATE" \
    astrid-edge-generation-guard.service astrid-edge-core-liveness.service "$CORE_LIVENESS_PATH_UNIT" "$SELF_CHANGE_INBOX_PATH_UNIT" astrid-edge-runtime.service.d/60-self-evolution-root.conf; do
    contains "$required_unit" "${install_units[@]}" || die "required self-evolution unit was not requested: $required_unit"
done
if [[ $appliance_id != icp* ]]; then
    contains "$AUDIO_SOCKET_UNIT" "${install_units[@]}" || die "AVADO audio feeder socket is absent"
    contains "$AUDIO_SERVICE_UNIT" "${install_units[@]}" || die "AVADO audio feeder service is absent"
fi
contains astrid-edge-steward.timer "${enable_units[@]}" || die "the coalesced steward timer must be explicitly enabled"
contains astrid-edge-self-change-probation-health.timer "${enable_units[@]}" || die "the five-minute probation sampler must be explicitly enabled"
contains "$WEB_CORE_SOCKET_UNIT" "${enable_units[@]}" || die "the core web-broker socket must be explicitly enabled"
contains "$WEB_RUNTIME_SOCKET_UNIT" "${enable_units[@]}" || die "the runtime web-broker socket must be explicitly enabled"
contains "$WEB_STEWARD_SOCKET_UNIT" "${enable_units[@]}" || die "the steward web-broker socket must be explicitly enabled"
contains "$PROVIDER_RUNTIME_SOCKET_UNIT" "${enable_units[@]}" || die "the runtime provider socket must be explicitly enabled"
contains "$PROVIDER_STEWARD_SOCKET_UNIT" "${enable_units[@]}" || die "the steward provider socket must be explicitly enabled"
contains "$PROVIDER_WARMUP_SOCKET_UNIT" "${enable_units[@]}" || die "the warmup provider socket must be explicitly enabled"
contains "$PRESENTATION_SOCKET_UNIT" "${enable_units[@]}" || die "the untrusted presentation socket must be explicitly enabled"
contains astrid-edge-generation-guard.service "${enable_units[@]}" || die "the immutable generation guard must be explicitly enabled"
contains "$CORE_LIVENESS_PATH_UNIT" "${enable_units[@]}" || die "the attested core-liveness watcher must be explicitly enabled"
contains "$SELF_CHANGE_INBOX_PATH_UNIT" "${enable_units[@]}" || die "the non-authorizing candidate handoff watcher must be explicitly enabled"
[[ $appliance_id == icp* ]] || contains "$AUDIO_SOCKET_UNIT" "${enable_units[@]}" || die "the AVADO audio feeder socket must be enabled"

release_parent=${release_root%/releases}
[[ $release_parent != "$release_root" ]] || die "release root must end in /releases"
for parent in "${state_root%/*}" "$release_parent" "${source_root%/*}" "${candidate_root%/*}" "${builder_root%/*}" "${updater_root%/*}" "${toolchain_root%/*}"; do
    if [[ $appliance_id == icp* ]] && ! $dry_run && path_within "$parent" /media/data; then
        require_root_ancestry "$parent" /media/data "ICP immutable destination"
    elif $dry_run && [[ $parent == "$release_parent" && ! -e $parent && ! -L $parent ]]; then
        # A mutation-free bundle preflight must work on a clean appliance,
        # before the root bootstrap creates the fixed /opt release parent.
        # Validate its already-existing ancestor; live installation still
        # performs the complete root-owned ancestry check before any write.
        release_parent_ancestor=${parent%/*}
        validate_existing "$release_parent_ancestor" "release destination ancestor"
        [[ -d $release_parent_ancestor ]] || die "release destination ancestor is not a directory"
    else
        validate_existing "$parent" "destination parent"
    fi
done
builder_image=${builder_root}.ext4
safe_absolute "$builder_image" || die "builder image path is unsafe"
runtime_state_mount=$(readlink -f -- "$astrid_state_root") || die "cannot resolve the canonical Astrid runtime-state root"
safe_absolute "$runtime_state_mount" || die "canonical Astrid runtime-state root is unsafe"
if [[ $appliance_id == icp* ]]; then
    [[ $runtime_state_mount == /media/data/astrid/state ]] || die "ICP runtime state is not the exact SSD-backed canonical root"
    state_volume_root=/media/data/astrid-edge-volumes
else
    [[ $runtime_state_mount == "$runtime_home/.astrid" ]] || die "AVADO runtime state is not the exact canonical home root"
    state_volume_root=/var/lib/astrid-edge-volumes
fi
runtime_state_image=$state_volume_root/runtime-state.ext4
rollback_state_image=$state_volume_root/rollback-state.ext4
state_migration_root=$state_volume_root/migration
state_migration_journal=$state_migration_root/state-migration.json
runtime_state_backup=$state_migration_root/runtime-source-backup
rollback_state_backup=$state_migration_root/rollback-source-backup
rollback_state_mount=$state_snapshots
for path in "$state_volume_root" "$runtime_state_image" "$rollback_state_image" "$state_migration_root" "$state_migration_journal" "$runtime_state_backup" "$rollback_state_backup" "$rollback_state_mount"; do
    safe_absolute "$path" || die "bounded state-store path is unsafe: $path"
done
for left in "$runtime_state_mount" "$rollback_state_mount"; do
    for right in "$state_volume_root" "$builder_root" "$builder_image"; do
        { path_within "$left" "$right" || path_within "$right" "$left"; } && die "bounded state-store path overlaps another managed root"
    done
done
for path in "$state_root" "$release_root" "$source_root" "$candidate_root" "$updater_root" "$toolchain_root"; do
    { path_within "$builder_image" "$path" || path_within "$path" "$builder_image"; } && die "builder image overlaps another managed root"
done
declare -a managed_files=("$helper_install_path" "$supervisor_install_path" "$rescue_helper_install_path" "$checkpoint_install_path" "$capsule_builder_install_path" "$web_broker_install_path" "$provider_broker_install_path" "$presentation_broker_install_path" "$HINDSIGHT_WRITER_INSTALL" "$HINDSIGHT_CONFIG" "$OPERATOR_REPORT_ROOT" "$OPERATOR_STATUS_ROOT" "$BUILDER_STORE_INSTALL" "$STATE_STORE_INSTALL" "$CANDIDATE_SANDBOX_ROOT" "$SUPERVISOR_CONFIG" "$RESCUE_CONFIG" "$WEB_CORE_CONFIG" "$WEB_RUNTIME_CONFIG" "$WEB_STEWARD_CONFIG" "$PROVIDER_CONFIG" "$PRESENTATION_CONFIG" "$BUILDER_STORE_CONFIG" "$STATE_STORE_CONFIG" "$builder_image" "$state_volume_root" "$CORE_WEB_REQUEST_KEY" "$RUNTIME_WEB_REQUEST_KEY" "$STEWARD_WEB_REQUEST_KEY" "$WEB_RESPONSE_SIGNING_KEY" "$WEB_RESPONSE_VERIFY_KEY" "$RUNTIME_PROVIDER_REQUEST_KEY" "$STEWARD_PROVIDER_REQUEST_KEY" "$WARMUP_PROVIDER_REQUEST_KEY" "$PROVIDER_LEDGER_KEY" "$LEDGER_ATTESTATION_KEY" "$SUPERVISOR_KEY" "$PROFILES_CONFIG" "$STEWARD_CONFIG" "$SOURCE_KEY" "$INTENT_KEY" "$SCHEDULED_AUTHORSHIP_VERIFY_KEY" "$MANAGER_MARKER" "$AUTHORITY_ENV" "$release_parent/current" "$release_parent/slot-a" "$release_parent/slot-b")
[[ $appliance_id == icp* ]] || managed_files+=("$AUDIO_FEEDER_INSTALL" "$AUDIO_CONFIG")
for path in "${managed_files[@]}" "${disjoint_roots[@]}"; do [[ ! -e $path && ! -L $path ]] || die "refusing to overwrite managed path: $path"; done
for action in status pause resume rollback rescue; do [[ ! -e $control_root/astrid-edge-self-change-$action && ! -L $control_root/astrid-edge-self-change-$action ]] || die "control wrapper exists"; done
[[ ! -e $control_root/astrid-edge-self-evolution-control && ! -L $control_root/astrid-edge-self-evolution-control ]] || die "control dispatcher exists"
for unit in "${install_units[@]}"; do [[ ! -e $system_unit_root/$unit && ! -L $system_unit_root/$unit ]] || die "requested unit already exists"; done
for unit in "${install_units[@]}"; do
    if [[ $unit == *.service ]]; then
        [[ ! -e $system_unit_root/$unit.d/60-self-evolution-root.conf && ! -L $system_unit_root/$unit.d/60-self-evolution-root.conf ]] || die "requested unit drop-in already exists"
    fi
done

if $dry_run; then
    printf 'DRY-RUN: native steward helper (actual CLI: --config [--credential-directory]) sha256=%s -> %s\n' "$helper_sha256" "$helper_install_path"
    printf 'DRY-RUN: Python supervisor (actual CLI: --config --execute COMMAND) sha256=%s -> %s\n' "$supervisor_sha256" "$supervisor_install_path"
    printf 'DRY-RUN: native rescue/checkpoint/capsule-builder helpers are digest-pinned under /usr/libexec/astrid\n'
    printf 'DRY-RUN: immutable web broker sha256=%s -> %s; isolated core/runtime/steward AF_UNIX sockets enforce peer identity; persisted quotas core=8/hour+24/UTC-day runtime=8/hour+24/UTC-day steward=2/hour+12/UTC-day max=2/trace\n' "$web_broker_sha256" "$web_broker_install_path"
    printf 'DRY-RUN: immutable provider broker sha256=%s -> %s; isolated runtime/steward/warmup AF_UNIX sockets enforce peer identity\n' "$provider_broker_sha256" "$provider_broker_install_path"
    printf 'DRY-RUN: immutable presentation broker sha256=%s -> %s; candidate output is untrusted decoration only\n' "$presentation_broker_sha256" "$presentation_broker_install_path"
    printf 'DRY-RUN: immutable operator reports=%s manifest-sha256=%s; sealed train runs directly; other exact Python -I launchers run trusted reports before optional bounded presentation\n' "$OPERATOR_REPORT_ROOT" "$operator_report_manifest_sha256"
    printf 'DRY-RUN: fixed 64 GiB fully allocated ext4 builder store=%s image=%s; 8 GiB internal and 64 GiB backing reserves\n' "$builder_root" "$builder_image"
    printf 'DRY-RUN: independent 32 GiB runtime=%s and rollback=%s ext4 images; runtime reserves 20%% for root recovery plus 65,536 emergency inodes; aggregate backing reserve=64 GiB\n' "$runtime_state_image" "$rollback_state_image"
    printf 'DRY-RUN: source/toolchain/generation bundle hashes verified; secure regular-only extraction planned\n'
    printf 'DRY-RUN: root-owned separate supervisor/source/intent/ledger HMAC keys and distinct actual-schema configs\n'
    printf 'DRY-RUN: locked identities: %s %s %s %s %s; bounded build/install/activate/rollback/health profiles enabled\n' "$STEWARD_USER" "$BUILDER_USER" "$UPDATER_USER" "$WEB_USER" "$PRESENTATION_USER"
    printf 'DRY-RUN: runtime sees only active generation=%s/current workspace=%s and model-ipc=%s\n' "$release_parent" "$runtime_workspace" "$model_ipc"
    printf 'DRY-RUN: root runtime bindings: appliance=%s socket=%s/run/system.sock token=%s/run/system.token cli=%s/current/astrid liveness-request=%s/runtime/core-liveness-recovery.request.json\n' \
        "$appliance_id" "$astrid_state_root" "$astrid_state_root" "$release_parent" "$runtime_workspace"
    printf 'DRY-RUN: profile Ollama executable=%s runtime-root=%s models=%s sha256=%s; immutable per-start digest gate\n' \
        "$ollama_binary" "$ollama_runtime_root" "$ollama_model_root" "$ollama_binary_sha256"
    printf 'DRY-RUN: steward uses loopback-only origin=%s; source(ro)=%s candidate(rw)=%s inbox(rw)=%s\n' "$ollama_origin" "$source_root" "$candidate_root" "$inbox_root"
    printf 'DRY-RUN: signed scheduled-authorship source=%s -> runtime read-only alias=/run/astrid-edge-self-change/scheduled-authorship with public verifier credential=%s\n' "$scheduled_authorship_root" "$SCHEDULED_AUTHORSHIP_VERIFY_KEY"
    printf 'DRY-RUN: immutable evidence projection=%s (root:%s 2750, steward read-only)\n' "$introspection_evidence_root" "$STEWARD_USER"
    printf 'DRY-RUN: root helper activates units only through private alias=%s; mutable builders cannot traverse its root-only parent\n' "$system_unit_alias"
    printf 'DRY-RUN: retained ICP backup excluded from every managed root: %s\n' "$backup_root"
    printf 'DRY-RUN: install units:'; printf ' %s' "${install_units[@]}"; printf '\nDRY-RUN: enable units:'
    if ((${#enable_units[@]} > 0)); then printf ' %s' "${enable_units[@]}"; fi
    printf '\nDRY-RUN: no helper/supervisor execution, filesystem write, systemctl, user service, or launchctl action performed\n'
    migration_profile=avado; [[ $appliance_id == icp* ]] && migration_profile=icp
    migration_args=(--dry-run --profile "$migration_profile" --appliance-id "$appliance_id" --runtime-user "$runtime_user" --runtime-home "$runtime_home" --unit-source-root "$unit_source_root" --user-unit-root "$user_unit_root" --system-unit-root "$system_unit_root" --rescue-system-unit-root "$system_unit_alias" \
        --active-generation-root "$release_parent/current" --management-marker "$MANAGER_MARKER" \
        --source-root "$source_root" --candidate-root "$candidate_root" --builder-root "$builder_root" --updater-root "$updater_root" --toolchain-root "$toolchain_root" \
        --model-lock "$model_lock" --maintenance-lease "$maintenance_lease" --authority-env "$AUTHORITY_ENV" \
        --self-evolution-dropin-sha256 "$(sha_file "$unit_source_root/root/astrid-edge-runtime-self-evolution.conf.in")" --unit-policy "$unit_policy" --provider-output-tokens "$output_tokens" \
        --post-install-verifier "$rescue_helper_install_path" --post-install-verifier-config "$RESCUE_CONFIG" \
        --state-store-helper "$state_store_helper" --state-store-helper-sha256 "$state_store_helper_sha256" \
        --state-store-config "$STATE_STORE_CONFIG" \
        --state-store-runtime-mount-unit astrid-edge-bounded-runtime.mount \
        --state-store-rollback-mount-unit astrid-edge-bounded-rollback.mount \
        --state-store-verify-unit "$STATE_STORE_VERIFY_UNIT" --state-store-health-timer "$STATE_STORE_HEALTH_TIMER")
    migration_args+=(--ollama-binary "$ollama_binary" --ollama-binary-sha256 "$ollama_binary_sha256")
    migration_args+=(--operator-report-manifest-sha256 "$operator_report_manifest_sha256")
    for ((index=0; index<${#required_system_stack[@]}; index++)); do migration_args+=(--unit "${required_system_stack[$index]}" --unit-sha256 "${required_system_stack[$index]}=${system_stack_hashes[$index]}"); done
    [[ -n $required_mount ]] && migration_args+=(--required-mount "$required_mount" --required-mount-uuid "$required_mount_uuid")
    if [[ $migration_profile == avado ]]; then
        migration_args+=(--profile-dropin-sha256 "astrid-local-ollama.conf=$(sha_file "$unit_source_root/astrid-local-ollama.conf")")
    else
        migration_args+=(--profile-dropin-sha256 "icp-ssd-required.conf=$(sha_file "$unit_source_root/icp-ssd-required.conf")")
        migration_args+=(--profile-dropin-sha256 "astrid-edge-tuning-authority.conf=$(sha_file "$unit_source_root/astrid-edge-tuning-authority.conf")")
    fi
    $start_system_services && migration_args+=(--start-services)
    "$migrator" "${migration_args[@]}"
    exit 0
fi

[[ $(id -u) == 0 ]] || die "installation requires root; use --dry-run for validation"
[[ $(uname -s) == Linux ]] || die "installation is Linux-only"
for command in findmnt systemctl systemd-run systemd-analyze systemd-escape useradd usermod groupadd passwd install sha256sum tar python3 getfacl setfacl runuser readlink fallocate mkfs.ext4 blkid losetup setpriv getent chattr lsattr ldconfig; do command -v "$command" >/dev/null || die "required command absent: $command"; done
builder_mount_unit=$(systemd-escape --path --suffix=mount "$builder_root")
[[ $builder_mount_unit == *.mount && $builder_mount_unit != */* ]] || die "systemd returned an invalid builder mount unit name"
system_unit_alias_mount_unit=$(systemd-escape --path --suffix=mount "$system_unit_alias")
[[ $system_unit_alias_mount_unit == *.mount && $system_unit_alias_mount_unit != */* ]] \
    || die "systemd returned an invalid private system-unit alias mount name"
# `sed` consumes a single backslash in replacement text. Preserve systemd's
# `\xNN` unit-name escapes so Requires=/After= resolve the rendered mount unit.
builder_mount_unit_sed=${builder_mount_unit//\\/\\\\}
for unit in "$builder_mount_unit" "$BUILDER_STORE_VERIFY_UNIT"; do
    [[ ! -e $system_unit_root/$unit && ! -L $system_unit_root/$unit ]] || die "builder-store unit already exists: $unit"
done
[[ ! -e $system_unit_root/$system_unit_alias_mount_unit && ! -L $system_unit_root/$system_unit_alias_mount_unit ]] \
    || die "private system-unit alias mount already exists: $system_unit_alias_mount_unit"
if [[ -n $required_mount ]]; then
    mount_target=$(findmnt -rn -M "$required_mount" -o TARGET) || die "required SSD is not mounted"
    mount_uuid=$(findmnt -rn -M "$required_mount" -o UUID) || die "required SSD UUID unavailable"
    [[ $mount_target == "$required_mount" && $mount_uuid == "$required_mount_uuid" ]] || die "required SSD mount/UUID mismatch"
fi

broker_resolver_output=$(/usr/bin/python3 -I -E -s - /etc/resolv.conf <<'PY'
import ipaddress, sys

values = []
with open(sys.argv[1], encoding="ascii", errors="strict") as handle:
    for raw in handle:
        fields = raw.split()
        if len(fields) < 2 or fields[0] != "nameserver":
            continue
        if len(values) >= 4 or "%" in fields[1]:
            raise SystemExit("resolver list escaped numeric bounded policy")
        address = ipaddress.ip_address(fields[1])
        if address.is_unspecified or address.is_multicast:
            raise SystemExit("resolver address is unsafe")
        value = f"{address}/{address.max_prefixlen}"
        if value not in values:
            values.append(value)
if not values:
    raise SystemExit("no numeric DNS resolver is configured")
print("\n".join(values))
PY
) || die "cannot derive a bounded numeric DNS allowlist for the immutable web broker"
mapfile -t broker_resolvers <<<"$broker_resolver_output"

# All immutable destinations must be anchored beneath root-controlled parents.
# Merely checking the final path for a symlink is insufficient when an
# unprivileged owner can rename an ancestor after installation.
for parent in "${state_root%/*}" "$release_parent" "${source_root%/*}" \
    "${candidate_root%/*}" "${builder_root%/*}" "${updater_root%/*}" \
    "${toolchain_root%/*}" "${state_volume_root%/*}" /usr /etc "$control_root" "$system_unit_root"; do
    if [[ $appliance_id == icp* ]] && path_within "$parent" /media/data; then
        require_root_ancestry "$parent" /media/data "ICP immutable destination"
    else
        require_root_directory "$parent" "immutable destination parent"
    fi
done

lock=/run/lock/astrid-edge-self-evolution-bootstrap.lock
mkdir "$lock" 2>/dev/null || die "another bootstrap is active"
stage=$(mktemp -d /var/tmp/astrid-edge-self-evolution.XXXXXX)
declare -a created_users=() created_groups=() created_paths=() enabled_now=() started_now=()
acl_snapshot=$stage/original.acl
acl_changed=false
committed=false
authority_activation_in_progress=false
origin_mac_correction_committed=false
authority_bootstrap_dropin=/run/systemd/system/astrid-edge-runtime.service.d/99-self-change-bootstrap-authority.conf

rollback_transaction() {
    local status=$? index path
    trap - EXIT HUP INT TERM
    if ${authority_activation_in_progress:-false}; then
        # A signal or command failure anywhere after the authority bit moves to
        # true must converge back to disabled before preserving the already
        # committed root-manager migration.
        set +e
        systemctl stop "$CORE_LIVENESS_PATH_UNIT" >/dev/null 2>&1
        systemctl stop "$SELF_CHANGE_INBOX_PATH_UNIT" >/dev/null 2>&1
        systemctl stop astrid-edge-steward.timer >/dev/null 2>&1
        systemctl stop astrid-edge-self-change-probation-health.timer >/dev/null 2>&1
        systemctl stop astrid-edge-runtime.service >/dev/null 2>&1
        rm -f -- "$authority_bootstrap_dropin"
        systemctl daemon-reload >/dev/null 2>&1
        if atomic_authority_install "$authority_disabled_source"; then
            systemctl start astrid-edge-runtime.service >/dev/null 2>&1
        fi
        authority_activation_in_progress=false
    fi
    if ! $committed && [[ -f ${migration_completion_marker:-} && ! -L ${migration_completion_marker:-} ]]; then
        # The nested migration owns its own rollback.  Its root-only marker is
        # written only after every root service is installed, verified,
        # enabled/started as requested, and its transaction is committed.
        # Preserve the matching outer boundary if a signal arrives in the
        # otherwise unavoidable child-exit/parent-assignment hand-off window.
        committed=true
    fi
    if ! $committed; then
        set +e
        if ${origin_mac_correction_committed:-false}; then
            printf 'warning: preserving the independently committed origin-mac correction and canonical receipt: %s\n' "$origin_mac_retirement_root" >&2
        fi
        bounded_state_live=false
        system_unit_alias_live=false
        [[ -n ${runtime_state_mount:-} ]] && findmnt -rn -M "$runtime_state_mount" >/dev/null 2>&1 && bounded_state_live=true
        for ((index=${#started_now[@]}-1; index>=0; index--)); do systemctl stop "${started_now[$index]}" >/dev/null 2>&1; done
        [[ -n ${system_unit_alias:-} ]] && findmnt -rn -M "$system_unit_alias" >/dev/null 2>&1 && system_unit_alias_live=true
        for ((index=${#enabled_now[@]}-1; index>=0; index--)); do
            unit=${enabled_now[$index]}
            if $bounded_state_live && [[ $unit == "${runtime_state_mount_unit:-}" || $unit == "${rollback_state_mount_unit:-}" || $unit == "$STATE_STORE_MIGRATION_RECOVER_UNIT" || $unit == "$STATE_STORE_RECOVER_UNIT" || $unit == "$STATE_STORE_VERIFY_UNIT" || $unit == "$STATE_STORE_HEALTH_TIMER" ]]; then
                continue
            fi
            systemctl disable "$unit" >/dev/null 2>&1
        done
        $acl_changed && setfacl --restore="$acl_snapshot" >/dev/null 2>&1
        for ((index=${#created_paths[@]}-1; index>=0; index--)); do
            path=${created_paths[$index]}
            if $system_unit_alias_live && [[ $path == "$updater_root" || $path == "$system_unit_root/${system_unit_alias_mount_unit:-}" ]]; then
                printf 'warning: preserving private system-unit alias transaction member because its bind mount remains active: %s\n' "$path" >&2
                continue
            fi
            if [[ $path == "$builder_root" || $path == "$builder_image" || $path == "$BUILDER_STORE_CONFIG" ]] \
                && findmnt -rn -M "$builder_root" >/dev/null 2>&1; then
                printf 'warning: preserving builder-store transaction members because its mount remains active: %s\n' "$path" >&2
                continue
            fi
            if $bounded_state_live; then
                case "$path" in
                    "$STATE_STORE_INSTALL"|"$STATE_STORE_CONFIG"|"$state_volume_root"|"$runtime_state_image"|"$rollback_state_image"|\
                    "$system_unit_root/$runtime_state_mount_unit"|"$system_unit_root/$rollback_state_mount_unit"|\
                    "$system_unit_root/$STATE_STORE_MIGRATION_RECOVER_UNIT"|"$system_unit_root/$STATE_STORE_RECOVER_UNIT"|"$system_unit_root/$STATE_STORE_VERIFY_UNIT"|\
                    "$system_unit_root/$STATE_STORE_HEALTH_UNIT"|"$system_unit_root/$STATE_STORE_HEALTH_TIMER")
                        printf 'warning: preserving committed bounded-state transaction member: %s\n' "$path" >&2
                        continue ;;
                esac
            fi
            [[ -n $path && $path != / && $path != /home && $path != /root && $path != "$backup_root" ]] && rm -rf -- "$path"
        done
        for ((index=${#created_users[@]}-1; index>=0; index--)); do userdel "${created_users[$index]}" >/dev/null 2>&1; done
        for ((index=${#created_groups[@]}-1; index>=0; index--)); do groupdel "${created_groups[$index]}" >/dev/null 2>&1; done
        systemctl daemon-reload >/dev/null 2>&1
        printf 'error: bootstrap transaction rolled back\n' >&2
    fi
    rm -rf -- "$stage"; rmdir "$lock" 2>/dev/null || true
    exit "$status"
}
trap rollback_transaction EXIT HUP INT TERM

# Every supplied executable, key, archive, unit, and migration helper is
# captured once beneath the private root transaction directory. From this point
# onward no mutable operator/repository path is consumed by the privileged
# installer or its nested migrator.
trusted_input_root=$stage/trusted-inputs
stable_stage_file "$helper" "$trusted_input_root/steward-helper" "$helper_sha256" 268435456 0500 "native steward helper"
stable_stage_file "$supervisor" "$trusted_input_root/supervisor" "$supervisor_sha256" 67108864 0500 "self-contained Python supervisor"
stable_stage_file "$rescue_helper" "$trusted_input_root/rescue-helper" "$rescue_helper_sha256" 268435456 0500 "rescue helper"
stable_stage_file "$checkpoint" "$trusted_input_root/checkpoint" "$checkpoint_sha256" 268435456 0500 "checkpoint helper"
stable_stage_file "$capsule_builder" "$trusted_input_root/capsule-builder" "$capsule_builder_sha256" 268435456 0500 "capsule builder"
stable_stage_file "$web_broker" "$trusted_input_root/web-broker" "$web_broker_sha256" 268435456 0500 "immutable web broker"
stable_stage_file "$provider_broker" "$trusted_input_root/provider-broker" "$provider_broker_sha256" 268435456 0500 "immutable provider broker"
stable_stage_file "$presentation_broker" "$trusted_input_root/presentation-broker" "$presentation_broker_sha256" 268435456 0500 "immutable presentation broker"
stable_stage_file "$source_signing_key" "$trusted_input_root/source.key" "$source_signing_key_sha256" 64 0400 "source signing key"
stable_stage_file "$source_bundle" "$trusted_input_root/source.tar.gz" "$source_bundle_sha256" 2147483648 0400 "source bundle"
stable_stage_file "$toolchain_bundle" "$trusted_input_root/toolchain.tar.gz" "$toolchain_bundle_sha256" 4294967296 0400 "toolchain bundle"
stable_stage_file "$initial_generation_bundle" "$trusted_input_root/generation.tar.gz" "$initial_generation_sha256" 536870912 0400 "initial generation bundle"

trusted_unit_source_root=$trusted_input_root/units
for ((index=0; index<${#unit_template_names[@]}; index++)); do
    unit=${unit_template_names[$index]}
    staged_unit_relative=$unit
    [[ $unit == astrid-edge-runtime.service.d/60-self-evolution-root.conf ]] && staged_unit_relative=root/astrid-edge-runtime-self-evolution.conf.in
    case "$unit" in
        astrid-edge-web-broker-*.socket|astrid-edge-provider-*.socket|astrid-edge-self-change-inbox.path|astrid-edge-core-liveness.path|astrid-edge-presentation-broker.socket|astrid-edge-presentation-broker@.service|astrid-edge-audio-feeder.socket) staged_unit_relative=$unit.in ;;
    esac
    stable_stage_file "$(unit_source "$unit")" "$trusted_unit_source_root/$staged_unit_relative" \
        "${unit_template_hashes[$index]}" 1048576 0400 "root-boundary unit template"
done
for ((index=0; index<${#required_system_stack[@]}; index++)); do
    unit=${required_system_stack[$index]}
    profile_source_root=$unit_source_root
    destination_profile_root=$trusted_unit_source_root
    if [[ $appliance_id == icp* ]]; then
        profile_source_root=$unit_source_root/icp
        destination_profile_root=$trusted_unit_source_root/icp
    fi
    stable_stage_file "$profile_source_root/$unit" "$destination_profile_root/$unit" \
        "${system_stack_hashes[$index]}" 1048576 0400 "authorized system unit"
done
for ((index=0; index<${#profile_source_names[@]}; index++)); do
    stable_stage_file "$unit_source_root/${profile_source_names[$index]}" "$trusted_unit_source_root/${profile_source_names[$index]}" \
        "${profile_source_hashes[$index]}" 1048576 0500 "appliance profile source"
done
stable_stage_file "$migrator" "$trusted_unit_source_root/root/migrate-edge-user-services-to-system" "$migrator_sha256" 1048576 0500 "system-service migrator"
stable_stage_file "$builder_store_helper" "$trusted_unit_source_root/root/astrid-edge-builder-store" "$builder_store_helper_sha256" 1048576 0500 "builder-store helper"
stable_stage_file "$builder_store_mount_template" "$trusted_unit_source_root/root/astrid-edge-builder-store.mount.in" "$builder_store_mount_template_sha256" 1048576 0400 "builder-store mount template"
stable_stage_file "$builder_store_verify_template" "$trusted_unit_source_root/astrid-edge-builder-store-verify.service.in" "$builder_store_verify_template_sha256" 1048576 0400 "builder-store verifier template"
stable_stage_file "$system_unit_alias_mount_template" "$trusted_unit_source_root/root/astrid-edge-system-units-alias.mount.in" "$system_unit_alias_mount_template_sha256" 1048576 0400 "private system-unit alias mount template"
stable_stage_file "$state_store_helper" "$trusted_unit_source_root/root/astrid-edge-state-store" "$state_store_helper_sha256" 2097152 0500 "state-store helper"
stable_stage_file "$state_store_runtime_mount_template" "$trusted_unit_source_root/root/astrid-edge-state-store-runtime.mount.in" "$state_store_runtime_mount_template_sha256" 1048576 0400 "runtime-state mount template"
stable_stage_file "$state_store_rollback_mount_template" "$trusted_unit_source_root/root/astrid-edge-state-store-rollback.mount.in" "$state_store_rollback_mount_template_sha256" 1048576 0400 "rollback-state mount template"
stable_stage_file "$state_store_migration_recover_template" "$trusted_unit_source_root/astrid-edge-state-store-migration-recover.service.in" "$state_store_migration_recover_template_sha256" 1048576 0400 "state-store migration recovery template"
stable_stage_file "$state_store_recover_template" "$trusted_unit_source_root/astrid-edge-state-store-recover.service.in" "$state_store_recover_template_sha256" 1048576 0400 "state-store recovery template"
stable_stage_file "$state_store_verify_template" "$trusted_unit_source_root/astrid-edge-state-store-verify.service.in" "$state_store_verify_template_sha256" 1048576 0400 "state-store verifier template"
stable_stage_file "$state_store_health_service" "$trusted_unit_source_root/astrid-edge-state-store-health.service" "$state_store_health_service_sha256" 1048576 0400 "state-store health service"
stable_stage_file "$state_store_health_timer" "$trusted_unit_source_root/astrid-edge-state-store-health.timer" "$state_store_health_timer_sha256" 1048576 0400 "state-store health timer"
stable_stage_file "$state_store_bounded_dropin_template" "$trusted_unit_source_root/astrid-edge-bounded-state.conf.in" "$state_store_bounded_dropin_template_sha256" 1048576 0400 "bounded-state drop-in template"
stable_stage_file "$control_source" "$trusted_unit_source_root/root/astrid-edge-self-evolution-control" "$control_source_sha256" 1048576 0500 "control wrapper"
stable_stage_file "$authority_disabled_source" "$trusted_unit_source_root/astrid-edge-self-change-disabled.env" "$authority_disabled_sha256" 1024 0400 "disabled self-change authority"
stable_stage_file "$authority_enabled_source" "$trusted_unit_source_root/astrid-edge-self-change-enabled.env" "$authority_enabled_sha256" 1024 0400 "enabled self-change authority"
stable_stage_file "$presentation_config_template" "$trusted_input_root/edge-presentation-broker.json.in" \
    "$presentation_config_template_sha256" 65536 0400 "presentation-broker config template"
stable_stage_file "$audio_config_template" "$trusted_input_root/edge-audio-feeder.json.in" \
    "$audio_config_template_sha256" 65536 0400 "audio-feeder config template"
stable_stage_file "$hindsight_config_template" "$trusted_input_root/edge-hindsight-writer.json.in" \
    "$hindsight_config_template_sha256" 65536 0400 "hindsight-writer config template"
stable_stage_file "$audio_feeder_source" "$trusted_input_root/edge_audio_feeder.py" \
    "$audio_feeder_source_sha256" 16777216 0400 "immutable audio feeder source"

helper=$trusted_input_root/steward-helper
supervisor=$trusted_input_root/supervisor
rescue_helper=$trusted_input_root/rescue-helper
checkpoint=$trusted_input_root/checkpoint
capsule_builder=$trusted_input_root/capsule-builder
web_broker=$trusted_input_root/web-broker
provider_broker=$trusted_input_root/provider-broker
presentation_broker=$trusted_input_root/presentation-broker
source_signing_key=$trusted_input_root/source.key
source_bundle=$trusted_input_root/source.tar.gz
toolchain_bundle=$trusted_input_root/toolchain.tar.gz
initial_generation_bundle=$trusted_input_root/generation.tar.gz
unit_source_root=$trusted_unit_source_root
migrator=$unit_source_root/root/migrate-edge-user-services-to-system
builder_store_helper=$unit_source_root/root/astrid-edge-builder-store
builder_store_mount_template=$unit_source_root/root/astrid-edge-builder-store.mount.in
builder_store_verify_template=$unit_source_root/astrid-edge-builder-store-verify.service.in
system_unit_alias_mount_template=$unit_source_root/root/astrid-edge-system-units-alias.mount.in
state_store_helper=$unit_source_root/root/astrid-edge-state-store
state_store_runtime_mount_template=$unit_source_root/root/astrid-edge-state-store-runtime.mount.in
state_store_rollback_mount_template=$unit_source_root/root/astrid-edge-state-store-rollback.mount.in
state_store_migration_recover_template=$unit_source_root/astrid-edge-state-store-migration-recover.service.in
state_store_recover_template=$unit_source_root/astrid-edge-state-store-recover.service.in
state_store_verify_template=$unit_source_root/astrid-edge-state-store-verify.service.in
state_store_health_service=$unit_source_root/astrid-edge-state-store-health.service
state_store_health_timer=$unit_source_root/astrid-edge-state-store-health.timer
state_store_bounded_dropin_template=$unit_source_root/astrid-edge-bounded-state.conf.in
control_source=$unit_source_root/root/astrid-edge-self-evolution-control
authority_disabled_source=$unit_source_root/astrid-edge-self-change-disabled.env
authority_enabled_source=$unit_source_root/astrid-edge-self-change-enabled.env
presentation_config_template=$trusted_input_root/edge-presentation-broker.json.in
audio_config_template=$trusted_input_root/edge-audio-feeder.json.in
hindsight_config_template=$trusted_input_root/edge-hindsight-writer.json.in
audio_feeder_source=$trusted_input_root/edge_audio_feeder.py

ensure_role() {
    local user=$1 home shell primary_gid expected_gid lock_state
    if ! getent group "$user" >/dev/null; then groupadd --system "$user"; created_groups+=("$user"); fi
    if getent passwd "$user" >/dev/null; then
        IFS=: read -r _ _ _ primary_gid _ home shell <<<"$(getent passwd "$user")"
        expected_gid=$(getent group "$user" | cut -d: -f3)
        lock_state=$(passwd -S "$user" | awk '{print $2}')
        [[ $home == /nonexistent && $shell == /usr/sbin/nologin && $primary_gid == "$expected_gid" && $lock_state == L ]] || die "existing role is not the exact locked identity: $user"
    else
        useradd --system --gid "$user" --home-dir /nonexistent --shell /usr/sbin/nologin --no-create-home "$user"
        created_users+=("$user")
        usermod --lock "$user"
    fi
}
ensure_role "$STEWARD_USER"; ensure_role "$BUILDER_USER"; ensure_role "$UPDATER_USER"; ensure_role "$WEB_USER"; ensure_role "$PROVIDER_USER"; ensure_role "$WARMUP_USER"; ensure_role "$PRESENTATION_USER"
if [[ $appliance_id != icp* ]]; then ensure_role "$AUDIO_USER"; fi
getent group "$MODEL_LOCK_GROUP" >/dev/null && die "reserved model-lock group already exists"
groupadd --system "$MODEL_LOCK_GROUP"; created_groups+=("$MODEL_LOCK_GROUP")
client_groups=("$WEB_CORE_CLIENT_GROUP" "$WEB_RUNTIME_CLIENT_GROUP" "$PROVIDER_RUNTIME_GROUP" "$PROVIDER_STEWARD_GROUP" "$PROVIDER_WARMUP_GROUP")
[[ $appliance_id == icp* ]] || client_groups+=("$AUDIO_CLIENT_GROUP")
for client_group in "${client_groups[@]}"; do
    getent group "$client_group" >/dev/null && die "reserved client group already exists: $client_group"
    groupadd --system "$client_group"; created_groups+=("$client_group")
done
steward_uid=$(id -u "$STEWARD_USER"); steward_gid=$(id -g "$STEWARD_USER")
builder_uid=$(id -u "$BUILDER_USER"); builder_gid=$(id -g "$BUILDER_USER")
updater_uid=$(id -u "$UPDATER_USER"); updater_gid=$(id -g "$UPDATER_USER")
web_gid=$(id -g "$WEB_USER")
web_uid=$(id -u "$WEB_USER")
provider_uid=$(id -u "$PROVIDER_USER"); provider_gid=$(id -g "$PROVIDER_USER")
warmup_uid=$(id -u "$WARMUP_USER"); warmup_gid=$(id -g "$WARMUP_USER")
presentation_uid=$(id -u "$PRESENTATION_USER"); presentation_gid=$(id -g "$PRESENTATION_USER")
audio_uid= audio_gid=
if [[ $appliance_id != icp* ]]; then audio_uid=$(id -u "$AUDIO_USER"); audio_gid=$(id -g "$AUDIO_USER"); fi
model_lock_gid=$(getent group "$MODEL_LOCK_GROUP" | cut -d: -f3)
web_core_client_gid=$(getent group "$WEB_CORE_CLIENT_GROUP" | cut -d: -f3)
web_runtime_client_gid=$(getent group "$WEB_RUNTIME_CLIENT_GROUP" | cut -d: -f3)
provider_runtime_gid=$(getent group "$PROVIDER_RUNTIME_GROUP" | cut -d: -f3)
provider_steward_gid=$(getent group "$PROVIDER_STEWARD_GROUP" | cut -d: -f3)
provider_warmup_gid=$(getent group "$PROVIDER_WARMUP_GROUP" | cut -d: -f3)
audio_client_gid=
if [[ $appliance_id != icp* ]]; then audio_client_gid=$(getent group "$AUDIO_CLIENT_GROUP" | cut -d: -f3); fi
[[ $model_lock_gid =~ ^[1-9][0-9]*$ ]] || die "dedicated model-lock group has an invalid GID"
role_uids=("$runtime_uid" "$steward_uid" "$builder_uid" "$updater_uid" "$web_uid" "$provider_uid" "$warmup_uid" "$presentation_uid")
role_gids=("$runtime_gid" "$steward_gid" "$builder_gid" "$updater_gid" "$web_gid" "$provider_gid" "$warmup_gid" "$presentation_gid" "$model_lock_gid" "$web_core_client_gid" "$web_runtime_client_gid" "$provider_runtime_gid" "$provider_steward_gid" "$provider_warmup_gid")
if [[ $appliance_id != icp* ]]; then role_uids+=("$audio_uid"); role_gids+=("$audio_gid" "$audio_client_gid"); fi
[[ $(printf '%s\n' "${role_uids[@]}" | LC_ALL=C sort -u | wc -l | tr -d ' ') == ${#role_uids[@]} ]] || die "runtime and immutable service UIDs must be distinct"
[[ $(printf '%s\n' "${role_gids[@]}" | LC_ALL=C sort -u | wc -l | tr -d ' ') == ${#role_gids[@]} ]] || die "runtime and immutable service GIDs must be distinct"
account_members=("$runtime_user" "$STEWARD_USER" "$WARMUP_USER")
[[ $appliance_id == icp* ]] || account_members+=("$AUDIO_USER")
for member in "${account_members[@]}"; do
    current_groups=$(id -G "$member" | tr ' ' '\n')
    forbidden_gids=("$web_core_client_gid" "$web_runtime_client_gid" "$provider_runtime_gid" "$provider_steward_gid" "$provider_warmup_gid")
    [[ $appliance_id == icp* ]] || forbidden_gids+=("$audio_client_gid")
    for forbidden_gid in "${forbidden_gids[@]}"; do
        ! grep -Fxq "$forbidden_gid" <<<"$current_groups" || die "client socket authority leaked into persistent account membership: $member"
    done
done
# Candidate-native commands run as this exact account inside the supervisor's
# private `/run`.  Refuse supplementary memberships that could turn a visible
# Astrid socket into ambient authority, and prove that the only re-exposed host
# manager socket remains inaccessible after the UID/GID drop.
mapfile -t builder_groups < <(id -G "$BUILDER_USER" | tr ' ' '\n' | sed '/^$/d')
[[ ${#builder_groups[@]} == 1 && ${builder_groups[0]} == "$builder_gid" ]] \
    || die "builder identity has supplementary group authority"
[[ -S /run/systemd/private && ! -L /run/systemd/private ]] \
    || die "root systemd private manager socket is absent or linked"
setpriv --reuid="$builder_uid" --regid="$builder_gid" --clear-groups \
    /usr/bin/python3 -I -E -s - <<'PY' || die "builder can open the root systemd manager socket"
import socket

client = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
try:
    client.connect("/run/systemd/private")
except PermissionError:
    raise SystemExit(0)
except OSError as error:
    raise SystemExit(f"unexpected systemd manager socket result: {error}")
else:
    raise SystemExit(1)
finally:
    client.close()
PY

# The capability-free liveness oneshot relies on one deliberately narrow DAC
# group. Refuse a shared primary/supplementary group before granting traversal.
mapfile -t runtime_group_members < <(
    {
        getent passwd | awk -F: -v gid="$runtime_gid" '$4 == gid { print $1 }'
        getent group "$runtime_group" | awk -F: '{ count = split($4, names, ","); for (index = 1; index <= count; index++) if (names[index] != "") print names[index] }'
    } | LC_ALL=C sort -u
)
[[ ${#runtime_group_members[@]} == 1 && ${runtime_group_members[0]} == "$runtime_user" ]] \
    || die "runtime liveness group has unintended members"

declare -a acl_targets=("${owned_paths[@]}" "$autonomy_state" "$action_receipts" "${steward_reflection_root%/*}" "$self_change_liveness_root")
for parent in "$scheduled_introspection_root" "$self_change_root"; do [[ -e $parent ]] && acl_targets+=("$parent"); done
[[ -e $runtime_self_change_outbox ]] && acl_targets+=("$runtime_self_change_outbox")
# Every recursive grant must have a recursive rollback image.  Without `-R`,
# a failed bootstrap would restore only the directory ACL while leaving the
# steward's inherited read authority on pre-existing artifact files.
getfacl -R -p "${acl_targets[@]}" >"$acl_snapshot"
acl_changed=true
[[ -d $self_change_liveness_root && ! -L $self_change_liveness_root ]] \
    || die "core-liveness request parent is not a real directory"
chown "$runtime_user:$runtime_group" "$self_change_liveness_root"
chmod 0770 "$self_change_liveness_root"
liveness_dac_probe=$self_change_liveness_root/.core-liveness-dac-probe.$$
[[ ! -e $liveness_dac_probe && ! -L $liveness_dac_probe ]] \
    || die "core-liveness DAC probe path already exists"
install -m 0640 -o "$runtime_user" -g "$runtime_group" /dev/null "$liveness_dac_probe"
created_paths+=("$liveness_dac_probe")
setpriv --reuid=0 --regid="$runtime_gid" --clear-groups --bounding-set=-all \
    --inh-caps=-all --ambient-caps=-all /bin/sh -c \
    'grep -Eq "^CapEff:[[:space:]]+0+$" /proc/self/status && test -r "$1" && rm -- "$1"' sh "$liveness_dac_probe" \
    || die "capability-free root/runtime-group identity cannot consume the liveness request"
[[ ! -e $liveness_dac_probe && ! -L $liveness_dac_probe ]] \
    || die "capability-free liveness request cleanup failed"
for parent in "$scheduled_introspection_root" "$self_change_root"; do
    if [[ ! -e $parent ]]; then
        install -d -m 0700 -o "$runtime_user" -g "$runtime_gid" "$parent"
        created_paths+=("$parent")
    fi
done
install -d -m 0700 -o "$runtime_user" -g "$runtime_gid" "$runtime_admission_root"; created_paths+=("$runtime_admission_root")
if [[ ! -e $runtime_self_change_outbox && ! -L $runtime_self_change_outbox ]]; then
    created_paths+=("$runtime_self_change_outbox")
fi
install -d -m 0700 -o "$runtime_user" -g "$runtime_gid" "$runtime_self_change_outbox"
# A read-only child is not durable if the mutable runtime owns its parent: it
# could rename the whole steward directory and replace it between restarts.
# Root owns the three narrow container directories, while exact runtime- and
# steward-owned children retain their independent write authority.  The ACL
# snapshot above includes these pre-existing parents, so a failed bootstrap
# restores their original ownership and mode transactionally.
for parent in "${protected_steward_parents[@]}"; do
    chown root:"$runtime_group" "$parent"
    chmod 0710 "$parent"
done
for output_root in "${steward_output_roots[@]}"; do
    install -d -m 0750 -o "$STEWARD_USER" -g "$runtime_group" "$output_root"
    created_paths+=("$output_root")
    setfacl -m "d:m::r-x" "$output_root"
done
for parent in "${steward_traverse_roots[@]}"; do setfacl -m "u:$STEWARD_USER:--x" "$parent"; done

# Prove the mutable appliance identity cannot replace a steward output by
# renaming its directory or unlinking a steward-owned member.  These are
# availability and provenance checks, not merely read-permission checks.
for output_root in "${steward_output_roots[@]}"; do
    probe=$output_root/.immutable-parent-probe.$$
    install -m 0640 -o "$STEWARD_USER" -g "$runtime_group" /dev/null "$probe"
    ! runuser -u "$runtime_user" -- mv -- "$output_root" "$output_root.runtime-rename-probe" \
        || die "runtime can rename a steward-owned output root: $output_root"
    ! runuser -u "$runtime_user" -- rm -- "$probe" \
        || die "runtime can unlink a steward-owned output member: $output_root"
    rm -- "$probe"
done
setfacl -R -m "u:$STEWARD_USER:r-X" "${owned_paths[@]}"
for path in "$autonomy_state" "$action_receipts"; do setfacl -m "u:$STEWARD_USER:r--" "$path"; done
# sysfs generally does not support POSIX ACL mutation.  Thermal telemetry must
# already be readable by the locked steward identity; never try to mutate the
# kernel filesystem to manufacture that authority.
runuser -u "$STEWARD_USER" -- test -r "$thermal_celsius" || die "thermal telemetry is not steward-readable"
for path in "${owned_paths[@]}"; do [[ -d $path ]] && setfacl -m "d:u:$STEWARD_USER:r-x" "$path"; done

# The root supervisor must bind the live workspace for state snapshots, but
# candidate-native tests execute after an exact builder UID/GID drop. Prove the
# resulting DAC boundary with a deliberately world-readable file: denial must
# therefore come from workspace traversal, not from the canary's own mode.
builder_workspace_probe=$runtime_workspace/.astrid-builder-workspace-denial-probe.$$
[[ ! -e $builder_workspace_probe && ! -L $builder_workspace_probe ]] \
    || die "builder workspace denial probe path already exists"
install -m 0644 -o "$runtime_user" -g "$runtime_group" /dev/null "$builder_workspace_probe"
created_paths+=("$builder_workspace_probe")
setpriv --reuid="$builder_uid" --regid="$builder_gid" --clear-groups --bounding-set=-all \
    --inh-caps=-all --ambient-caps=-all /usr/bin/python3 -I -E -s - "$builder_workspace_probe" <<'PY' \
    || die "builder can open a mode-0644 canary beneath the mutable runtime workspace"
import sys

try:
    with open(sys.argv[1], "rb") as handle:
        handle.read(1)
except PermissionError:
    raise SystemExit(0)
except OSError as error:
    raise SystemExit(f"unexpected builder workspace denial result: {error}")
raise SystemExit(1)
PY
rm -- "$builder_workspace_probe"

secure_extract() {
    local archive=$1 expected_root=$2 destination=$3 maximum_bytes=$4 maximum_members=$5
    # Parse the tar metadata rather than human-formatted `tar -t` output.  This
    # rejects normalized aliases (a//b, a/./b), links, devices, sparse files,
    # duplicate extraction targets, and decompression bombs before root-owned
    # extraction begins.
    /usr/bin/python3 -I -E -s - "$archive" "$expected_root" "$maximum_bytes" "$maximum_members" <<'PY'
import sys, tarfile
from pathlib import PurePosixPath

archive_path, expected_root, maximum_bytes_raw, maximum_members_raw = sys.argv[1:]
maximum_bytes = int(maximum_bytes_raw)
maximum_members = int(maximum_members_raw)
seen = set()
total = 0
with tarfile.open(archive_path, mode="r:gz") as archive:
    for member in archive:
        rel = member.name
        normalized = PurePosixPath(rel)
        parts = normalized.parts
        canonical = str(normalized)
        if (
            not rel
            or rel.startswith("/")
            or not parts
            or parts[0] != expected_root
            or any(part in {"", ".", ".."} for part in parts)
            or (rel != canonical and not (member.isdir() and rel == f"{canonical}/"))
            or rel in seen
            or len(seen) >= maximum_members
            or not (member.isfile() or member.isdir())
            or getattr(member, "sparse", None)
        ):
            raise SystemExit("archive path/type/count escaped fixed policy")
        seen.add(rel)
        if member.isfile():
            if member.size < 0 or member.size > maximum_bytes:
                raise SystemExit("archive member exceeds extraction bound")
            total += member.size
            if total > maximum_bytes:
                raise SystemExit("archive exceeds total extraction bound")
if not seen:
    raise SystemExit("archive is empty")
PY
    mkdir -p "$destination"
    tar -xzf "$archive" --no-same-owner --no-same-permissions --delay-directory-restore -C "$destination"
    [[ -d $destination/$expected_root && ! -L $destination/$expected_root ]] || die "archive fixed root absent"
    if find "$destination/$expected_root" -type l -o \( ! -type f ! -type d \) | grep -q .; then die "archive extracted unsafe entries"; fi
    if find "$destination/$expected_root" -type f -links +1 | grep -q .; then die "archive extracted hard links"; fi
    if find "$destination/$expected_root" -perm /022 | grep -q .; then die "archive extracted writable trusted content"; fi
}

secure_extract "$source_bundle" astrid-edge-self-change-source "$stage/source" 2147483648 200000
secure_extract "$toolchain_bundle" astrid-edge-toolchain "$stage/toolchain" 4294967296 200000
secure_extract "$initial_generation_bundle" astrid-edge-generation "$stage/generation" 536870912 10000

generation_staged=$stage/generation/astrid-edge-generation
/usr/bin/python3 -I -E -s - "$generation_staged" "$target" <<'PY'
import hashlib, json, os, stat, sys
from pathlib import Path, PurePosixPath

root = Path(sys.argv[1])
target = sys.argv[2]
manifest_path = root / ".astrid-edge-generation.json"
raw = manifest_path.read_bytes()
value = json.loads(raw)
if set(value) != {"schema", "appliance_id", "version", "target", "inventory", "authority"}:
    raise SystemExit("initial-generation manifest shape failed")
if value["schema"] != "astrid.edge_self_change.initial_generation.v1" or value["target"] != target:
    raise SystemExit("initial-generation identity failed")
if value["appliance_id"] != "portable-bootstrap-non-authorizing":
    raise SystemExit("initial-generation bootstrap appliance identity failed")
if value["authority"] != "operator_packaged_initial_generation_not_model_candidate":
    raise SystemExit("initial-generation authority failed")
if raw != (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode("ascii"):
    raise SystemExit("initial-generation manifest is not canonical")
expected = set()
for item in value["inventory"]:
    if set(item) != {"path", "size", "sha256"}:
        raise SystemExit("initial-generation inventory shape failed")
    rel = item["path"]
    parts = PurePosixPath(rel).parts
    if not rel or rel.startswith("/") or any(part in {"", ".", ".."} for part in parts) or rel in expected:
        raise SystemExit("initial-generation inventory path failed")
    path = root / rel
    info = path.lstat()
    if not stat.S_ISREG(info.st_mode) or info.st_nlink != 1:
        raise SystemExit("initial-generation inventory member is not regular")
    data = path.read_bytes()
    if len(data) != item["size"] or hashlib.sha256(data).hexdigest() != item["sha256"]:
        raise SystemExit("initial-generation inventory digest failed")
    expected.add(rel)
actual = {
    path.relative_to(root).as_posix()
    for path in root.rglob("*")
    if path.is_file() and path != manifest_path
}
if actual != expected:
    raise SystemExit("initial-generation inventory membership failed")
expected_top = {
    ".astrid-edge-generation.json", "astrid", "astrid-daemon", "astrid-build",
    "astrid-edge-runtime", "capsules", "packaging", "scripts",
}
actual_top = {path.name for path in root.iterdir()}
if actual_top != expected_top:
    raise SystemExit(
        f"initial generation is not the exact mutable runtime layout: "
        f"missing={sorted(expected_top - actual_top)} extra={sorted(actual_top - expected_top)}"
    )
required = {
    "astrid", "astrid-daemon", "astrid-build", "astrid-edge-runtime",
    "scripts/warm_ollama_model.sh", "scripts/report_edge_appliance.py",
    "scripts/report_edge_appliance.sh", "scripts/report_edge_activity.py",
    "scripts/report_edge_fleet_activity.py", "scripts/edge_hindsight.py",
    "scripts/astrid_at_a_glance.py", "scripts/astrid_train.py",
    "scripts/retire_edge_origin_mac_affordance.py",
}
if not required <= actual:
    raise SystemExit("initial-generation required runtime payload is incomplete")
expected_scripts = {
    "warm_ollama_model.sh", "report_edge_appliance.py", "report_edge_appliance.sh",
    "report_edge_activity.py", "report_edge_fleet_activity.py", "edge_hindsight.py",
    "astrid_at_a_glance.py", "astrid_train.py", "retire_edge_origin_mac_affordance.py",
}
if {path.name for path in (root / "scripts").iterdir()} != expected_scripts:
    raise SystemExit("initial-generation runtime script membership is not exact")
capsules = {
    "astrid-capsule-cli", "astrid-capsule-fs", "astrid-capsule-http",
    "astrid-capsule-shell", "astrid-capsule-skills", "astrid-capsule-agents",
    "astrid-capsule-memory", "astrid-capsule-edge-context",
    "astrid-capsule-edge-introspector", "astrid-capsule-edge-spectral",
    "astrid-capsule-context-engine", "astrid-capsule-hook-bridge",
    "astrid-capsule-identity", "astrid-capsule-openai-compat",
    "astrid-capsule-prompt-builder", "astrid-capsule-react",
    "astrid-capsule-registry", "astrid-capsule-router",
    "astrid-capsule-session", "astrid-capsule-system",
}
archive_names = {
    path.stem for path in (root / "capsules").glob("*.capsule") if path.is_file()
}
if archive_names != capsules or any(
    not path.is_file() or path.suffix != ".capsule" for path in (root / "capsules").iterdir()
):
    raise SystemExit("initial-generation capsule archive membership failed")
PY

# Expand the exact twenty operator-packaged capsules with the digest-pinned
# generation CLI as the unprivileged runtime identity. The CLI supplies its
# normal manifest/meta normalization; a strict archive replay below restores
# each local Component Model payload so the generation is self-contained.
/usr/bin/python3 -I -E -s - "$generation_staged" <<'PY'
import sys, tarfile
from pathlib import Path, PurePosixPath

root = Path(sys.argv[1])
capsules = (
    "astrid-capsule-cli", "astrid-capsule-fs", "astrid-capsule-http",
    "astrid-capsule-shell", "astrid-capsule-skills", "astrid-capsule-agents",
    "astrid-capsule-memory", "astrid-capsule-edge-context",
    "astrid-capsule-edge-introspector", "astrid-capsule-edge-spectral",
    "astrid-capsule-context-engine", "astrid-capsule-hook-bridge",
    "astrid-capsule-identity", "astrid-capsule-openai-compat",
    "astrid-capsule-prompt-builder", "astrid-capsule-react",
    "astrid-capsule-registry", "astrid-capsule-router",
    "astrid-capsule-session", "astrid-capsule-system",
)
for capsule in capsules:
    archive_path = root / "capsules" / f"{capsule}.capsule"
    total = 0
    seen = set()
    wasm = []
    with tarfile.open(archive_path, mode="r:gz") as archive:
        for member in archive.getmembers():
            rel = member.name
            normalized = PurePosixPath(rel)
            parts = normalized.parts
            canonical = str(normalized)
            if (
                not rel
                or rel.startswith("/")
                or (rel != canonical and not (member.isdir() and rel == f"{canonical}/"))
                or any(part in {"", ".", ".."} for part in parts)
                or rel in seen
                or len(seen) >= 256
                or not (member.isfile() or member.isdir())
            ):
                raise SystemExit("capsule archive path/type/count failed")
            seen.add(rel)
            if member.isfile():
                if member.size > 64 * 1024 * 1024:
                    raise SystemExit("capsule archive member exceeds bound")
                total += member.size
                if total > 64 * 1024 * 1024:
                    raise SystemExit("capsule archive exceeds expanded bound")
                if rel.endswith(".wasm"):
                    wasm.append(member)
        if "Capsule.toml" not in seen or len(wasm) != 1:
            raise SystemExit("capsule archive lacks exact manifest/Component payload")
        stream = archive.extractfile(wasm[0])
        if stream is None:
            raise SystemExit("cannot read capsule Component payload")
        prefix = stream.read(8)
        if prefix != b"\x00asm\x0d\x00\x01\x00":
            raise SystemExit("capsule payload is not a Component Model binary")
PY
setfacl -m "u:$runtime_user:--x" "$stage" "$stage/generation"
setfacl -R -m "u:$runtime_user:r-X" "$generation_staged"
capsule_home=$stage/capsule-home
install -d -m 0700 -o "$runtime_user" -g "$runtime_gid" "$capsule_home"
for capsule in \
    astrid-capsule-cli astrid-capsule-fs astrid-capsule-http astrid-capsule-shell \
    astrid-capsule-skills astrid-capsule-agents astrid-capsule-memory \
    astrid-capsule-edge-context astrid-capsule-edge-introspector astrid-capsule-edge-spectral \
    astrid-capsule-context-engine astrid-capsule-hook-bridge astrid-capsule-identity \
    astrid-capsule-openai-compat astrid-capsule-prompt-builder astrid-capsule-react \
    astrid-capsule-registry astrid-capsule-router astrid-capsule-session astrid-capsule-system; do
    runuser -u "$runtime_user" -- env -i \
        HOME="$capsule_home" ASTRID_HOME="$capsule_home" PATH=/usr/bin:/bin \
        "$generation_staged/astrid" capsule install "$generation_staged/capsules/$capsule.capsule" </dev/null
done

/usr/bin/python3 -I -E -s - "$generation_staged" "$capsule_home" "$appliance_id" <<'PY'
import hashlib, json, os, re, stat, sys, tarfile
from pathlib import Path, PurePosixPath

root = Path(sys.argv[1])
home = Path(sys.argv[2])
appliance_id = sys.argv[3]
installed = home / "home/default/.local/capsules"
capsules = (
    "astrid-capsule-cli", "astrid-capsule-fs", "astrid-capsule-http",
    "astrid-capsule-shell", "astrid-capsule-skills", "astrid-capsule-agents",
    "astrid-capsule-memory", "astrid-capsule-edge-context",
    "astrid-capsule-edge-introspector", "astrid-capsule-edge-spectral",
    "astrid-capsule-context-engine", "astrid-capsule-hook-bridge",
    "astrid-capsule-identity", "astrid-capsule-openai-compat",
    "astrid-capsule-prompt-builder", "astrid-capsule-react",
    "astrid-capsule-registry", "astrid-capsule-router",
    "astrid-capsule-session", "astrid-capsule-system",
)
children = list(installed.iterdir())
if {path.name for path in children} != set(capsules) or any(
    not path.is_dir() or path.is_symlink() for path in children
):
    raise SystemExit("expanded capsule membership failed")
records = []
for capsule in capsules:
    archive_path = root / "capsules" / f"{capsule}.capsule"
    archive_bytes = archive_path.read_bytes()
    target = installed / capsule
    archive_files = {}
    wasm = []
    total = 0
    with tarfile.open(archive_path, mode="r:gz") as archive:
        seen = set()
        for member in archive.getmembers():
            rel = member.name
            normalized = PurePosixPath(rel)
            parts = normalized.parts
            canonical = str(normalized)
            if not rel or rel.startswith("/") or (rel != canonical and not (member.isdir() and rel == f"{canonical}/")) or any(part in {"", ".", ".."} for part in parts):
                raise SystemExit("capsule archive path failed")
            if rel in seen or len(seen) >= 256 or not (member.isfile() or member.isdir()):
                raise SystemExit("capsule archive type/count failed")
            seen.add(rel)
            if member.isfile():
                if member.size > 64 * 1024 * 1024:
                    raise SystemExit("capsule archive member exceeds bound")
                total += member.size
                if total > 64 * 1024 * 1024:
                    raise SystemExit("capsule archive exceeds expanded bound")
                stream = archive.extractfile(member)
                if stream is None:
                    raise SystemExit("cannot read capsule archive member")
                data = stream.read(member.size + 1)
                if len(data) != member.size:
                    raise SystemExit("capsule archive member length failed")
                archive_files[rel] = data
                if rel.endswith(".wasm"):
                    wasm.append(rel)
        if "Capsule.toml" not in seen or len(wasm) != 1:
            raise SystemExit("capsule archive lacks exact manifest/Component payload")
        if "meta.json" in archive_files:
            raise SystemExit("capsule archive collides with normalized installer metadata")
        component = archive_files[wasm[0]]
        if len(component) > 64 * 1024 * 1024 or not component.startswith(b"\x00asm\x0d\x00\x01\x00"):
            raise SystemExit("capsule payload is not a bounded Component Model binary")
    for path in target.rglob("*"):
        info = path.lstat()
        if not (stat.S_ISREG(info.st_mode) or stat.S_ISDIR(info.st_mode)) or info.st_nlink < 1:
            raise SystemExit("capsule installer produced linked or special content")
    for rel, data in archive_files.items():
        path = target.joinpath(*PurePosixPath(rel).parts)
        path.parent.mkdir(parents=True, exist_ok=True)
        if path.exists() or path.is_symlink():
            info = path.lstat()
            if not stat.S_ISREG(info.st_mode) or info.st_nlink != 1 or path.read_bytes() != data:
                raise SystemExit("capsule installer output differs from archive identity")
        else:
            path.write_bytes(data)
    meta_path = target / "meta.json"
    meta = json.loads(meta_path.read_bytes())
    wasm_hash = meta.get("wasm_hash")
    if (
        not isinstance(wasm_hash, str)
        or not re.fullmatch(r"[0-9a-f]{64}", wasm_hash)
    ):
        raise SystemExit("expanded capsule lacks installer-authenticated WASM identity")
    # Astrid's installer addresses Component binaries with BLAKE3, not SHA-256.
    # Avoid reimplementing that primitive here: bind the operator-digest-pinned
    # CLI result to the exact archive payload by verifying its single-link
    # content-addressed output byte-for-byte. SHA-256 remains the outer
    # generation inventory hash below.
    shared_component = home / "bin" / f"{wasm_hash}.wasm"
    shared_info = shared_component.lstat()
    if (
        not stat.S_ISREG(shared_info.st_mode)
        or shared_info.st_nlink != 1
        or shared_component.read_bytes() != component
    ):
        raise SystemExit("capsule installer content-addressed WASM differs from archive")
    meta["installed_at"] = "1970-01-01T00:00:00+00:00"
    meta["updated_at"] = "1970-01-01T00:00:00+00:00"
    meta.pop("source", None)
    meta_path.write_bytes((json.dumps(meta, sort_keys=True, separators=(",", ":")) + "\n").encode("ascii"))
    expected_files = set(archive_files) | {"meta.json"}
    actual_files = {
        path.relative_to(target).as_posix()
        for path in target.rglob("*")
        if path.is_file()
    }
    if actual_files != expected_files:
        raise SystemExit("expanded capsule files differ from exact archive-plus-meta contract")
    files = []
    for path in sorted(item for item in target.rglob("*") if item.is_file()):
        info = path.lstat()
        if not stat.S_ISREG(info.st_mode) or info.st_nlink != 1:
            raise SystemExit("expanded capsule contains linked or special content")
        data = path.read_bytes()
        files.append({
            "path": path.relative_to(target).as_posix(),
            "size": len(data),
            "sha256": hashlib.sha256(data).hexdigest(),
        })
    records.append({
        "capsule_id": capsule,
        "archive": f"capsules/{capsule}.capsule",
        "archive_sha256": hashlib.sha256(archive_bytes).hexdigest(),
        "expanded_files": files,
    })
identity = {
    "schema": "astrid.edge.installed_capsules.v1",
    "authority": "deterministic_expansion_of_operator_packaged_archives",
    "capsules": records,
}
(installed / "CAPSULES.json").write_bytes(
    (json.dumps(identity, sort_keys=True, separators=(",", ":")) + "\n").encode("ascii")
)
destination = root / "installed-capsules"
if destination.exists() or destination.is_symlink():
    raise SystemExit("initial generation already contains an installed capsule tree")
os.rename(installed, destination)

# The outer archive digest authenticated the original operator manifest. Bind
# the deterministic expansion into that manifest's complete inventory so the
# immutable rescue helper can later prove exact post-install membership.
manifest_path = root / ".astrid-edge-generation.json"
manifest = json.loads(manifest_path.read_bytes())
inventory = []
for path in sorted(item for item in root.rglob("*") if item.is_file() and item != manifest_path):
    data = path.read_bytes()
    inventory.append({
        "path": path.relative_to(root).as_posix(),
        "size": len(data),
        "sha256": hashlib.sha256(data).hexdigest(),
    })
manifest["inventory"] = inventory
if manifest.get("appliance_id") != "portable-bootstrap-non-authorizing":
    raise SystemExit("initial generation lost its portable bootstrap identity")
manifest["appliance_id"] = appliance_id
manifest_path.write_bytes(
    (json.dumps(manifest, sort_keys=True, separators=(",", ":")) + "\n").encode("ascii")
)
persisted = json.loads(manifest_path.read_text(encoding="ascii"))
if persisted.get("appliance_id") != appliance_id:
    raise SystemExit("initial generation appliance rebinding did not persist")
if manifest_path.read_bytes() != (json.dumps(persisted, sort_keys=True, separators=(",", ":")) + "\n").encode("ascii"):
    raise SystemExit("locally rebound initial generation is not canonical")
PY
rm -rf -- "$capsule_home"

# Verify the signed source inventory.  Reviewed immutable-boundary source is
# intentionally inspect-only: it may be compiled for the fixed helpers, but it
# can never be reclassified as candidate-mutable source by changing a manifest
# origin string.
/usr/bin/python3 -I -E -s - "$stage/source/astrid-edge-self-change-source" "$source_signing_key" <<'PY'
# BEGIN_INSTALL_SOURCE_VERIFIER
import hashlib, hmac, json, re, stat, sys
from pathlib import Path, PurePosixPath

MAX_FILES = 100_000
MAX_UNCOMPRESSED_BYTES = 4 * 1024 * 1024 * 1024
HEX_64 = re.compile(r"[0-9a-f]{64}\Z")
INSPECT_ONLY_ORIGIN = "inspect_only_immutable_boundary"
PRIVATE_COMPONENTS = frozenset(
    {
        ".ssh",
        "backups",
        "credentials",
        "home",
        "journals",
        "operator-quarantine",
        "private-keys",
        "secrets",
        "state",
        "trusted",
        "workspace",
    }
)
INSPECT_ONLY_SERVICE_PREFIXES = (
    "services/astrid-edge-steward-helper/",
    "services/astrid-edge-rescue-helper/",
    "services/astrid-edge-web-broker/",
    "services/astrid-edge-provider-broker/",
    "services/astrid-edge-presentation-broker/",
    "services/astrid-edge-checkpoint/",
)
INSPECT_ONLY_SCRIPT_NAMES = frozenset(
    {
        "build_edge_self_change_source_bundle.py",
        "build_edge_self_change_supervisor_zipapp.py",
        "build_edge_self_change_toolchain_bundle.py",
        "astrid_train.py",
        "edge_audio_feeder.py",
        "edge_hindsight.py",
        "edge_self_change_supervisor.py",
        "install_edge_self_evolution_root.sh",
        "report_edge_appliance.sh",
        "report_edge_fleet_activity.py",
        "test_build_edge_self_change_source_bundle.py",
        "test_build_edge_self_change_supervisor_zipapp.py",
        "test_build_edge_self_change_toolchain_bundle.py",
        "test_edge_builder_store.py",
        "test_edge_audio_feeder.py",
        "test_edge_state_store.py",
        "test_edge_probation_health_systemd.py",
        "test_edge_self_change_supervisor.py",
        "test_install_edge_self_evolution_root.sh",
    }
)
MUTABLE_LIVE_REPORTS = frozenset(
    {"astrid_at_a_glance.py", "report_edge_activity.py", "report_edge_appliance.py"}
)
BUILD_REQUIRED_REPORT_TESTS = frozenset(
    {
        "test_astrid_train.py",
        "test_edge_hindsight.py",
        "test_report_edge_activity.py",
        "test_report_edge_appliance.py",
    }
)
MUTABLE_CORE_CRATES = frozenset(
    {
        "astrid-approval",
        "astrid-audit",
        "astrid-build",
        "astrid-capabilities",
        "astrid-capsule",
        "astrid-cli",
        "astrid-config",
        "astrid-core",
        "astrid-crypto",
        "astrid-daemon",
        "astrid-events",
        "astrid-guest",
        "astrid-hooks",
        "astrid-integration-tests",
        "astrid-kernel",
        "astrid-mcp",
        "astrid-minime-protocol",
        "astrid-openclaw",
        "astrid-prelude",
        "astrid-spectral-core",
        "astrid-storage",
        "astrid-telemetry",
        "astrid-test",
        "astrid-types",
        "astrid-vfs",
        "astrid-workspace",
    }
)
EDGE_CAPSULES = frozenset(
    {
        "astrid-capsule-agents",
        "astrid-capsule-cli",
        "astrid-capsule-edge-context",
        "astrid-capsule-edge-introspector",
        "astrid-capsule-edge-spectral",
        "astrid-capsule-fs",
        "astrid-capsule-http",
        "astrid-capsule-memory",
        "astrid-capsule-shell",
        "astrid-capsule-skills",
        "astrid-capsule-context-engine",
        "astrid-capsule-hook-bridge",
        "astrid-capsule-identity",
        "astrid-capsule-openai-compat",
        "astrid-capsule-prompt-builder",
        "astrid-capsule-react",
        "astrid-capsule-registry",
        "astrid-capsule-router",
        "astrid-capsule-session",
        "astrid-capsule-system",
    }
)
EDGE_STANDALONE_SERVICES = frozenset(
    {
        "astrid-edge-checkpoint",
        "astrid-edge-presentation-broker",
        "astrid-edge-provider-broker",
        "astrid-edge-rescue-helper",
        "astrid-edge-runtime",
        "astrid-edge-steward-helper",
        "astrid-edge-web-broker",
    }
)
BUILD_FILE_SUFFIXES = frozenset(
    {".rs", ".toml", ".json", ".js", ".mjs", ".wit", ".wat", ".wasm", ".blake3"}
)
MUTABLE_UNIT_FRAGMENTS = frozenset(
    {
        "ollama-cpu.service",
        "astrid-model-warmup.service",
        "astrid.service",
        "astrid-edge-runtime.service",
        "astrid-edge-hindsight.service",
        "astrid-edge-hindsight.timer",
    }
)


def safe_relative_path(value):
    if (
        not isinstance(value, str)
        or not value
        or value.startswith("/")
        or "\\" in value
        or "\x00" in value
        or any(ord(character) < 32 or ord(character) == 127 for character in value)
    ):
        raise SystemExit("unsafe source inventory path")
    raw_parts = value.split("/")
    if any(part in {"", ".", ".."} for part in raw_parts):
        raise SystemExit("unsafe source inventory path")
    relative = PurePosixPath(value)
    if relative.as_posix() != value or any(part in {"", ".", ".."} for part in relative.parts):
        raise SystemExit("unsafe source inventory path")
    return relative


def inspect_only_boundary_path(path):
    relative = PurePosixPath(path)
    if any(part in PRIVATE_COMPONENTS or part.startswith(".") for part in relative.parts):
        return False
    for prefix in INSPECT_ONLY_SERVICE_PREFIXES:
        if path.startswith(prefix):
            leaf = path.removeprefix(prefix)
            return leaf in {"Cargo.toml", "Cargo.lock"} or (
                leaf.startswith("src/") and relative.suffix == ".rs"
            )
    if path.startswith("scripts/edge_self_change/"):
        return relative.suffix == ".py"
    if path.startswith("scripts/"):
        return path.removeprefix("scripts/") in INSPECT_ONLY_SCRIPT_NAMES
    if path.startswith("packaging/systemd/root/"):
        return relative.suffix in {".service", ".in", ".conf"} or relative.name in {
            "astrid-edge-builder-store",
            "astrid-edge-state-store",
            "astrid-edge-self-evolution-control",
            "migrate-edge-user-services-to-system",
        }
    if path.startswith("packaging/systemd/"):
        name = relative.name
        return any(
            marker in name
            for marker in (
                "self-change",
                "edge-steward",
                "edge-web-broker",
                "edge-provider-broker",
                "edge-presentation-broker",
                "edge-checkpoint",
                "builder-store",
                "state-store",
                "audio-feeder",
                "generation-guard",
                "core-liveness",
            )
        ) and relative.suffix in {".service", ".timer", ".socket", ".conf", ".env", ".in"}
    if path == "packaging/headless/edge-audio-feeder.json.in":
        return True
    return path == "docs/cpu-edge-self-evolution.md"


def denied_source_path(path):
    parts = PurePosixPath(path).parts
    if any(part in PRIVATE_COMPONENTS for part in parts):
        return True
    if parts and parts[0] in {".git", ".github", "minime"}:
        return True
    if path.startswith(("capsules/spectral-bridge/", "capsules/introspector/")):
        return True
    if path.startswith(INSPECT_ONLY_SERVICE_PREFIXES):
        return True
    if path.startswith(("scripts/edge_self_change/", "services/astrid-edge-self-change-")):
        return True
    if path in {f"scripts/{name}" for name in INSPECT_ONLY_SCRIPT_NAMES}:
        return True
    return path.startswith("packaging/systemd/") and any(
        marker in path
        for marker in (
            "self-change",
            "edge-steward",
            "edge-web-broker",
            "edge-checkpoint",
            "core-liveness",
        )
    )


def expected_source_origin(path):
    """Mirror the signed bundler's closed source-role policy."""

    if inspect_only_boundary_path(path):
        return INSPECT_ONLY_ORIGIN
    if denied_source_path(path):
        return None
    if path == "LICENSE-js-pdk":
        return "build_required_immutable"
    if path == "Cargo.lock" or path == "Cargo.toml":
        return "mutable_build_manifest"
    if path == "wit/astrid-capsule.wit":
        return "build_required_immutable"
    if path in {".cargo/config.toml", "clippy.toml", "rustfmt.toml"}:
        return "build_required_manifest"
    if path.startswith("crates/"):
        parts = PurePosixPath(path).parts
        if len(parts) < 3:
            return None
        crate_name = parts[1]
        relative = PurePosixPath(*parts[2:])
        if relative.as_posix() == "Cargo.toml":
            return "mutable_build_manifest" if crate_name in MUTABLE_CORE_CRATES else "build_required_immutable"
        if relative.as_posix() == "build.rs":
            return "mutable_core_source" if crate_name in MUTABLE_CORE_CRATES else "build_required_immutable"
        if relative.suffix in BUILD_FILE_SUFFIXES:
            if crate_name in MUTABLE_CORE_CRATES and relative.suffix == ".rs":
                return "mutable_core_source"
            return "build_required_immutable"
        return None
    if path.startswith("services/astrid-edge-runtime/"):
        relative = path.removeprefix("services/astrid-edge-runtime/")
        if relative in {"Cargo.toml", "Cargo.lock"}:
            return "mutable_build_manifest"
        if relative.startswith("src/") and path.endswith(".rs"):
            return "mutable_edge_runtime"
        return None
    for capsule in EDGE_CAPSULES:
        prefix = f"capsules/astralis/{capsule}/"
        if not path.startswith(prefix):
            continue
        relative = path.removeprefix(prefix)
        if relative in {"Cargo.toml", "Cargo.lock"}:
            return "mutable_build_manifest"
        if relative == "Capsule.toml":
            return "mutable_capsule_manifest"
        if relative.startswith("src/") and PurePosixPath(relative).suffix in {
            ".rs",
            ".md",
            ".json",
            ".toml",
            ".txt",
        }:
            return "mutable_edge_capsule"
        return None
    if path.startswith("scripts/"):
        name = path.removeprefix("scripts/")
        if name == "warm_ollama_model.sh":
            return "build_required_runtime_script"
        if name in MUTABLE_LIVE_REPORTS:
            return "mutable_edge_report"
        if name in BUILD_REQUIRED_REPORT_TESTS:
            return "build_required_immutable"
        return None
    if path.startswith("packaging/appliances/") and PurePosixPath(path).suffix in {".env", ".json"}:
        return "mutable_appliance_profile"
    if path.startswith("packaging/systemd/"):
        parts = PurePosixPath(path).parts
        name = parts[-1]
        if (name.startswith("astrid") or name == "ollama-cpu.service") and PurePosixPath(name).suffix in {
            ".service",
            ".timer",
            ".conf",
            ".env",
        }:
            if name in MUTABLE_UNIT_FRAGMENTS and (
                len(parts) == 3 or (len(parts) == 4 and parts[2] == "icp")
            ):
                return "mutable_astrid_service_template"
            return "build_required_service_template"
    return None


root, key_path = map(Path, sys.argv[1:])
manifest_path = root / "MANIFEST.json"
signature_path = root / "MANIFEST.signature.json"
for metadata_path in (manifest_path, signature_path):
    info = metadata_path.lstat()
    if not stat.S_ISREG(info.st_mode) or info.st_nlink != 1 or stat.S_IMODE(info.st_mode) & 0o077:
        raise SystemExit("source metadata must be owner-only regular files")
manifest_bytes = manifest_path.read_bytes()
signature_bytes = signature_path.read_bytes()
signature = json.loads(signature_bytes)
manifest = json.loads(manifest_bytes)
canonical = json.dumps(manifest, sort_keys=True, separators=(",", ":"), ensure_ascii=True, allow_nan=False).encode("ascii")
canonical_signature = json.dumps(signature, sort_keys=True, separators=(",", ":"), ensure_ascii=True, allow_nan=False).encode("ascii")
key = key_path.read_bytes()
if len(key) != 32 or manifest_bytes != canonical + b"\n" or signature_bytes != canonical_signature + b"\n": raise SystemExit("noncanonical source manifest")
key_id = hashlib.sha256(key).hexdigest()[:16]
manifest_keys = {
    "schema", "appliance_id", "source_authority", "source_id", "source_identity_sha256", "repository_commit",
    "git_object_format", "rustc", "cargo_lock_version", "cargo_lock_sha256",
    "vendor_packages", "signature_mode", "key_id", "file_count",
    "uncompressed_bytes", "files",
}
signature_keys = {"schema", "mode", "key_id", "manifest_sha256", "hmac_sha256"}
if not isinstance(manifest, dict) or set(manifest) != manifest_keys or not isinstance(signature, dict) or set(signature) != signature_keys: raise SystemExit("source schema mismatch")
if manifest.get("schema") != "astrid.edge.self_change_source_bundle.v1" or signature.get("schema") != "astrid.edge.self_change_source_signature.v1": raise SystemExit("source schema mismatch")
if manifest.get("appliance_id") is not None or manifest.get("source_authority") != "portable_bootstrap_non_authorizing": raise SystemExit("bootstrap source is not explicitly portable and non-authorizing")
if signature.get("mode") != "hmac-sha256" or manifest.get("signature_mode") != "hmac-sha256" or signature.get("key_id") != key_id or manifest.get("key_id") != key_id or signature.get("manifest_sha256") != hashlib.sha256(canonical).hexdigest(): raise SystemExit("source signature envelope mismatch")
if not hmac.compare_digest(str(signature.get("hmac_sha256", "")), hmac.new(key, canonical, hashlib.sha256).hexdigest()): raise SystemExit("source HMAC mismatch")
expected = {"MANIFEST.json", "MANIFEST.signature.json"}; seen = set()
files = manifest.get("files")
if not isinstance(files, list) or not files or len(files) > MAX_FILES or isinstance(manifest.get("file_count"), bool) or manifest.get("file_count") != len(files): raise SystemExit("source inventory count mismatch")
previous = ""
total = 0
for item in files:
    if not isinstance(item, dict) or set(item) != {"path", "origin", "mode", "size", "sha256"}:
        raise SystemExit("source inventory record schema mismatch")
    rel = item["path"]
    relative = safe_relative_path(rel)
    if rel in seen or rel <= previous:
        raise SystemExit("source inventory is not strictly sorted and unique")
    previous = rel
    if rel.startswith("source/"):
        source_relative = rel.removeprefix("source/")
        if not source_relative:
            raise SystemExit("source inventory contains an unexpected source path")
        origin = expected_source_origin(source_relative)
        if origin is None:
            raise SystemExit("source inventory contains an excluded or unexpected source path")
    elif rel.startswith("vendor/"):
        origin = "operator_vendored_cargo"
    elif rel == "rustc-version.txt":
        origin = "operator_supplied_toolchain_metadata"
    else:
        raise SystemExit("source inventory contains an unexpected payload path")
    if item["origin"] != origin:
        raise SystemExit("source inventory origin disagrees with exact path policy")
    if item["mode"] not in {"0600", "0644", "0755"}:
        raise SystemExit("source inventory mode mismatch")
    size = item["size"]
    if isinstance(size, bool) or not isinstance(size, int) or size < 0:
        raise SystemExit("source inventory size mismatch")
    if not isinstance(item["sha256"], str) or HEX_64.fullmatch(item["sha256"]) is None:
        raise SystemExit("source inventory digest schema mismatch")
    total += size
    if total > MAX_UNCOMPRESSED_BYTES:
        raise SystemExit("source inventory exceeds total byte bound")
    seen.add(rel)
    path = root.joinpath(*relative.parts); info = path.lstat()
    actual_mode = stat.S_IMODE(info.st_mode)
    declared_mode = int(item["mode"], 8)
    if not stat.S_ISREG(info.st_mode) or info.st_nlink != 1 or info.st_size != size: raise SystemExit("source inventory type/size mismatch")
    if actual_mode & ~declared_mode or not actual_mode & 0o400 or bool(actual_mode & 0o100) != bool(declared_mode & 0o100): raise SystemExit("source inventory extracted mode mismatch")
    if hashlib.sha256(path.read_bytes()).hexdigest() != item.get("sha256"): raise SystemExit("source inventory digest mismatch")
    expected.add(rel)
if isinstance(manifest.get("uncompressed_bytes"), bool) or manifest.get("uncompressed_bytes") != total: raise SystemExit("source inventory aggregate size mismatch")
required_capsule_locks = {
    f"source/capsules/astralis/{capsule}/Cargo.lock" for capsule in EDGE_CAPSULES
}
missing_capsule_locks = sorted(required_capsule_locks - seen)
if missing_capsule_locks:
    raise SystemExit(f"source inventory omits required edge capsule locks: {missing_capsule_locks}")
required_service_locks = {
    f"source/services/{service}/Cargo.lock" for service in EDGE_STANDALONE_SERVICES
}
missing_service_locks = sorted(required_service_locks - seen)
if missing_service_locks:
    raise SystemExit(f"source inventory omits required edge service locks: {missing_service_locks}")
required_quickjs = {
    "source/LICENSE-js-pdk",
    "source/crates/astrid-openclaw/kernel/engine.wasm",
    "source/crates/astrid-openclaw/kernel/engine.wasm.blake3",
}
missing_quickjs = sorted(required_quickjs - seen)
if missing_quickjs:
    raise SystemExit(f"source inventory omits required QuickJS kernel inputs: {missing_quickjs}")
commit = manifest.get("repository_commit")
rustc = manifest.get("rustc")
if not isinstance(commit, str) or re.fullmatch(r"[0-9a-f]{40}", commit) is None or not isinstance(rustc, dict):
    raise SystemExit("source identity metadata is invalid")
identity = {
    "schema": "astrid.edge.self_change_source_identity.v1",
    "appliance_id": None,
    "source_authority": "portable_bootstrap_non_authorizing",
    "repository_commit": commit,
    "rustc": rustc,
    "files": files,
}
identity_hash = hashlib.sha256(json.dumps(identity, sort_keys=True, separators=(",", ":"), ensure_ascii=True, allow_nan=False).encode("ascii")).hexdigest()
if manifest.get("source_identity_sha256") != identity_hash or manifest.get("source_id") != f"cpu-edge-portable:{identity_hash}":
    raise SystemExit("portable source identity mismatch")
actual = set()
actual_directories = set()
for path in root.rglob("*"):
    info = path.lstat()
    rel = path.relative_to(root).as_posix()
    if stat.S_ISREG(info.st_mode):
        if info.st_nlink != 1 or stat.S_IMODE(info.st_mode) & 0o022:
            raise SystemExit("source inventory contains linked or writable content")
        actual.add(rel)
    elif stat.S_ISDIR(info.st_mode):
        if stat.S_IMODE(info.st_mode) & 0o022:
            raise SystemExit("source inventory contains writable directories")
        actual_directories.add(rel)
    else:
        raise SystemExit("source inventory contains symlink or special content")
if actual != expected: raise SystemExit("source inventory membership mismatch")
expected_directories = set()
for rel in expected:
    parent = PurePosixPath(rel).parent
    while parent != PurePosixPath("."):
        expected_directories.add(parent.as_posix())
        parent = parent.parent
if actual_directories != expected_directories: raise SystemExit("source inventory directory membership mismatch")
# END_INSTALL_SOURCE_VERIFIER
PY

source_staged=$stage/source/astrid-edge-self-change-source
toolchain_staged=$stage/toolchain/astrid-edge-toolchain
generation_staged=$stage/generation/astrid-edge-generation
[[ -d $source_staged/vendor ]] || die "signed source bundle has no vendor subtree"

ensure_root_install_directory() {
    local path=$1 parent=${1%/*}
    if [[ -e $path || -L $path ]]; then
        require_root_directory "$path" "immutable install directory"
        return
    fi
    require_root_directory "$parent" "immutable install parent"
    install -d -m 0755 -o root -g root "$path"
    created_paths+=("$path")
}
require_root_directory /usr/libexec "immutable executable root"
ensure_root_install_directory /usr/libexec/astrid
ensure_root_install_directory /usr/libexec/astrid-edge
ensure_root_install_directory /usr/libexec/astrid-edge/immutable

# Candidate-native commands are launched by the immutable system manager in a
# deliberately empty root. Host libraries, the disposable transaction, and
# the signed toolchain are re-exposed only as per-command binds. Materialize
# the production workspace path as an empty read-only decoy. The capability-
# empty generation guard can therefore attest that it is empty, while the
# builder still cannot write it and the transient unit's InaccessiblePaths
# gate independently prevents access to that path.
install -d -m 0755 -o root -g root "$CANDIDATE_SANDBOX_ROOT"
created_paths+=("$CANDIDATE_SANDBOX_ROOT")
declare -a candidate_sandbox_directories=(
    bin dev etc home lib lib64 media mnt opt proc root run sbin sys tmp
    usr usr/bin usr/lib usr/lib64 usr/libexec usr/local usr/sbin usr/share
    var var/tmp
)
for candidate_sandbox_relative in "${candidate_sandbox_directories[@]}"; do
    install -d -m 0555 -o root -g root "$CANDIDATE_SANDBOX_ROOT/$candidate_sandbox_relative"
done
candidate_workspace_suffix=${runtime_workspace#/}
IFS='/' read -r -a candidate_workspace_components <<<"$candidate_workspace_suffix"
((${#candidate_workspace_components[@]} >= 2)) || die "candidate workspace decoy path is too shallow"
candidate_workspace_cursor=$CANDIDATE_SANDBOX_ROOT
for ((candidate_component_index=0; candidate_component_index<${#candidate_workspace_components[@]}; candidate_component_index++)); do
    candidate_component=${candidate_workspace_components[$candidate_component_index]}
    [[ -n $candidate_component && $candidate_component != . && $candidate_component != .. ]] \
        || die "candidate workspace decoy contains an unsafe component"
    candidate_workspace_cursor=$candidate_workspace_cursor/$candidate_component
    install -d -m 0555 -o root -g root "$candidate_workspace_cursor"
    IFS=' ' read -r candidate_owner candidate_mode candidate_links <<<"$(stat_values "$candidate_workspace_cursor")"
    [[ $candidate_owner == 0 && $(stat -c '%g' -- "$candidate_workspace_cursor") == 0 && $candidate_links -ge 1 ]] \
        || die "candidate workspace decoy ancestry is not root-owned"
    [[ $candidate_mode == 555 ]] || die "candidate workspace decoy ancestry is not exact mode-0555"
done
[[ -z $(find "$candidate_workspace_cursor" -mindepth 1 -maxdepth 1 -print -quit) ]] \
    || die "candidate workspace decoy is not exact empty mode-0555"
chmod 0555 "$CANDIDATE_SANDBOX_ROOT"
[[ $(stat_values "$CANDIDATE_SANDBOX_ROOT" | awk '{print $1" "$2}') == '0 555' \
    && $(stat -c '%g' -- "$CANDIDATE_SANDBOX_ROOT") == 0 ]] \
    || die "candidate sandbox root is not exact root:root mode-0555"
/usr/bin/python3 -I -E -s - "$CANDIDATE_SANDBOX_ROOT" "$runtime_workspace" <<'PY'
import os
import stat
import sys
from pathlib import Path, PurePosixPath

root = Path(sys.argv[1])
workspace = PurePosixPath(sys.argv[2])
skeleton = {
    "bin", "dev", "etc", "home", "lib", "lib64", "media", "mnt", "opt",
    "proc", "root", "run", "sbin", "sys", "tmp", "usr", "usr/bin", "usr/lib",
    "usr/lib64", "usr/libexec", "usr/local", "usr/sbin", "usr/share", "var", "var/tmp",
}
workspace_parts = workspace.parts[1:]
if len(workspace_parts) < 2:
    raise SystemExit("candidate workspace decoy path is too shallow")
cursor = PurePosixPath()
for component in workspace_parts:
    cursor /= component
    skeleton.add(cursor.as_posix())

root_info = root.lstat()
if (
    not stat.S_ISDIR(root_info.st_mode)
    or stat.S_ISLNK(root_info.st_mode)
    or root_info.st_uid != 0
    or root_info.st_gid != 0
    or stat.S_IMODE(root_info.st_mode) != 0o555
):
    raise SystemExit("candidate sandbox root identity drifted")

actual = set()
for path in root.rglob("*"):
    relative = path.relative_to(root).as_posix()
    info = path.lstat()
    if (
        relative not in skeleton
        or not stat.S_ISDIR(info.st_mode)
        or stat.S_ISLNK(info.st_mode)
        or info.st_uid != 0
        or info.st_gid != 0
        or stat.S_IMODE(info.st_mode) != 0o555
    ):
        raise SystemExit(f"candidate sandbox tree escaped exact directory policy: {relative}")
    actual.add(relative)
if actual != skeleton:
    raise SystemExit("candidate sandbox tree membership is incomplete or excessive")
decoy = root.joinpath(*workspace_parts)
if next(decoy.iterdir(), None) is not None:
    raise SystemExit("candidate workspace decoy is not empty")
PY
ensure_root_install_directory /etc/astrid
ensure_root_install_directory /etc/astrid-edge-self-change
if [[ -e $LEDGER_KEY_ROOT || -L $LEDGER_KEY_ROOT ]]; then
    require_root_directory "$LEDGER_KEY_ROOT" "ledger credential root"
    [[ $(stat_values "$LEDGER_KEY_ROOT" | awk '{print $2}') == 700 ]] \
        || die "ledger credential root must have exact mode 0700"
else
    install -d -m 0700 -o root -g root "$LEDGER_KEY_ROOT"
    created_paths+=("$LEDGER_KEY_ROOT")
fi
require_root_directory "$control_root" "control root"
require_root_directory "$system_unit_root" "system unit root"
[[ ! -e $OPERATOR_STATUS_ROOT && ! -L $OPERATOR_STATUS_ROOT ]] || die "operator status persistent directory already exists"
# This is a persistent, root-owned operator projection—not a volatile runtime
# socket tree. Setgid preserves the appliance operator group across the
# supervisor's atomic rename while withholding write authority from that group.
install -d -m 2750 -o root -g "$runtime_group" "$OPERATOR_STATUS_ROOT"; created_paths+=("$OPERATOR_STATUS_ROOT")
install -m 0555 -o root -g root "$helper" "$helper_install_path"; created_paths+=("$helper_install_path")
install -m 0555 -o root -g root "$supervisor" "$supervisor_install_path"; created_paths+=("$supervisor_install_path")
install -m 0555 -o root -g root "$rescue_helper" "$rescue_helper_install_path"; created_paths+=("$rescue_helper_install_path")
install -m 0555 -o root -g root "$checkpoint" "$checkpoint_install_path"; created_paths+=("$checkpoint_install_path")
install -m 0555 -o root -g root "$capsule_builder" "$capsule_builder_install_path"; created_paths+=("$capsule_builder_install_path")
install -m 0555 -o root -g root "$web_broker" "$web_broker_install_path"; created_paths+=("$web_broker_install_path")
install -m 0555 -o root -g root "$provider_broker" "$provider_broker_install_path"; created_paths+=("$provider_broker_install_path")
install -m 0555 -o root -g root "$presentation_broker" "$presentation_broker_install_path"; created_paths+=("$presentation_broker_install_path")
install -m 0555 -o root -g root "$builder_store_helper" "$BUILDER_STORE_INSTALL"; created_paths+=("$BUILDER_STORE_INSTALL")
install -m 0555 -o root -g root "$state_store_helper" "$STATE_STORE_INSTALL"; created_paths+=("$STATE_STORE_INSTALL")
for key_path in "$CORE_WEB_REQUEST_KEY" "$RUNTIME_WEB_REQUEST_KEY" "$STEWARD_WEB_REQUEST_KEY" "$WEB_RESPONSE_SIGNING_KEY" "$WEB_RESPONSE_VERIFY_KEY"; do
    [[ ! -e $key_path && ! -L $key_path ]] || die "reserved web-broker credential already exists: $key_path"
    created_paths+=("$key_path")
done
create_private_random_key "$CORE_WEB_REQUEST_KEY" "core web request key"
create_private_random_key "$RUNTIME_WEB_REQUEST_KEY" "runtime web request key"
create_private_random_key "$STEWARD_WEB_REQUEST_KEY" "steward web request key"
for key_spec in \
    "$RUNTIME_PROVIDER_REQUEST_KEY|runtime provider request key" \
    "$STEWARD_PROVIDER_REQUEST_KEY|steward provider request key" \
    "$WARMUP_PROVIDER_REQUEST_KEY|warmup provider request key" \
    "$PROVIDER_LEDGER_KEY|provider receipt ledger key"; do
    IFS='|' read -r key_path key_label <<<"$key_spec"
    [[ ! -e $key_path && ! -L $key_path ]] || die "reserved provider credential already exists: $key_path"
    created_paths+=("$key_path")
    create_private_random_key "$key_path" "$key_label"
done
created_paths+=("$LEDGER_ATTESTATION_KEY")
create_private_random_key "$LEDGER_ATTESTATION_KEY" "root lifecycle ledger attestation key"
[[ $(stat_values "$LEDGER_ATTESTATION_KEY" | awk '{print $1" "$2" "$3}') == '0 400 1' && $(wc -c <"$LEDGER_ATTESTATION_KEY" | tr -d ' ') == 32 ]] \
    || die "ledger attestation key identity is invalid"
ledger_attestation_sha256=$(sha_file "$LEDGER_ATTESTATION_KEY")
"$web_broker_install_path" --key-init \
    --signing-seed "$WEB_RESPONSE_SIGNING_KEY" \
    --verify-key "$WEB_RESPONSE_VERIFY_KEY" >"$stage/web-key-initialization.json"
for key_path in "$CORE_WEB_REQUEST_KEY" "$RUNTIME_WEB_REQUEST_KEY" "$STEWARD_WEB_REQUEST_KEY"; do
    [[ $(stat_values "$key_path" | awk '{print $1" "$2" "$3}') == '0 400 1' && $(wc -c <"$key_path" | tr -d ' ') == 32 ]] \
        || die "web request credential identity is invalid: $key_path"
done
[[ $(stat_values "$WEB_RESPONSE_SIGNING_KEY" | awk '{print $1" "$2" "$3}') == '0 600 1' && $(wc -c <"$WEB_RESPONSE_SIGNING_KEY" | tr -d ' ') == 32 ]] \
    || die "web response signing seed identity is invalid"
[[ $(stat_values "$WEB_RESPONSE_VERIFY_KEY" | awk '{print $1" "$2" "$3}') == '0 644 1' && $(wc -c <"$WEB_RESPONSE_VERIFY_KEY" | tr -d ' ') == 32 ]] \
    || die "web response verify key identity is invalid"
core_web_request_sha256=$(sha_file "$CORE_WEB_REQUEST_KEY")
runtime_web_request_sha256=$(sha_file "$RUNTIME_WEB_REQUEST_KEY")
steward_web_request_sha256=$(sha_file "$STEWARD_WEB_REQUEST_KEY")
web_response_signing_sha256=$(sha_file "$WEB_RESPONSE_SIGNING_KEY")
web_response_verify_sha256=$(sha_file "$WEB_RESPONSE_VERIFY_KEY")
runtime_provider_sha256=$(sha_file "$RUNTIME_PROVIDER_REQUEST_KEY")
steward_provider_sha256=$(sha_file "$STEWARD_PROVIDER_REQUEST_KEY")
warmup_provider_sha256=$(sha_file "$WARMUP_PROVIDER_REQUEST_KEY")
provider_ledger_sha256=$(sha_file "$PROVIDER_LEDGER_KEY")
/usr/bin/python3 -I -E -s - "$stage/web-key-initialization.json" "$web_response_signing_sha256" "$web_response_verify_sha256" <<'PY'
import json, sys

path, signing_hash, verify_hash = sys.argv[1:]
value = json.load(open(path, encoding="utf-8"))
if set(value) != {"schema", "signing_seed_sha256", "verify_key_sha256", "created_signing_seed", "created_verify_key"}:
    raise SystemExit("web key initialization result has unexpected fields")
if value != {
    "schema": "astrid.edge.web_broker.key_initialization.v1",
    "signing_seed_sha256": signing_hash,
    "verify_key_sha256": verify_hash,
    "created_signing_seed": True,
    "created_verify_key": True,
}:
    raise SystemExit("web key initialization result does not bind exact fresh keys")
PY
[[ ! -e $SOURCE_KEY && ! -L $SOURCE_KEY ]] || die "reserved local source signing key already exists"
created_paths+=("$SOURCE_KEY")
create_private_random_key "$SOURCE_KEY" "per-appliance derived source signing key"
[[ $(stat_values "$SOURCE_KEY" | awk '{print $1" "$2" "$3}') == '0 400 1' && $(wc -c <"$SOURCE_KEY" | tr -d ' ') == 32 ]] \
    || die "per-appliance derived source signing key identity is invalid"
install -m 0444 -o root -g root "$authority_disabled_source" "$AUTHORITY_ENV"; created_paths+=("$AUTHORITY_ENV")

# The operator-signed archive is intentionally portable and cannot authorize
# a local candidate. Rebind its already-verified inventory to this exact
# appliance with a fresh key that never leaves the box, then discard the
# portable envelope before the steward can read the source root.
/usr/bin/python3 -I -E -s - "$source_staged" "$SOURCE_KEY" "$appliance_id" <<'PY'
import hashlib, hmac, json, os, pathlib, re, stat, sys, tempfile

root = pathlib.Path(sys.argv[1])
key_path = pathlib.Path(sys.argv[2])
appliance_id = sys.argv[3]
if re.fullmatch(r"[a-z0-9][a-z0-9._-]{0,63}", appliance_id) is None:
    raise SystemExit("invalid appliance identifier for source rebinding")
manifest_path = root / "MANIFEST.json"
signature_path = root / "MANIFEST.signature.json"
manifest = json.loads(manifest_path.read_text(encoding="ascii"))
if manifest.get("appliance_id") is not None or manifest.get("source_authority") != "portable_bootstrap_non_authorizing":
    raise SystemExit("source snapshot is not a verified portable bootstrap")
key = key_path.read_bytes()
if len(key) != 32:
    raise SystemExit("local source signing key has an invalid length")
identity = {
    "schema": "astrid.edge.self_change_source_identity.v1",
    "appliance_id": appliance_id,
    "source_authority": "appliance_local_authorizing",
    "repository_commit": manifest["repository_commit"],
    "rustc": manifest["rustc"],
    "files": manifest["files"],
}
canonical_identity = json.dumps(identity, sort_keys=True, separators=(",", ":"), ensure_ascii=True, allow_nan=False).encode("ascii")
identity_hash = hashlib.sha256(canonical_identity).hexdigest()
key_id = hashlib.sha256(key).hexdigest()[:16]
manifest.update({
    "appliance_id": appliance_id,
    "source_authority": "appliance_local_authorizing",
    "source_id": f"cpu-edge:{identity_hash}",
    "source_identity_sha256": identity_hash,
    "signature_mode": "hmac-sha256",
    "key_id": key_id,
})
canonical_manifest = json.dumps(manifest, sort_keys=True, separators=(",", ":"), ensure_ascii=True, allow_nan=False).encode("ascii")
signature = {
    "schema": "astrid.edge.self_change_source_signature.v1",
    "mode": "hmac-sha256",
    "key_id": key_id,
    "manifest_sha256": hashlib.sha256(canonical_manifest).hexdigest(),
    "hmac_sha256": hmac.new(key, canonical_manifest, hashlib.sha256).hexdigest(),
}

def replace(path, body):
    parent_fd = os.open(path.parent, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_CLOEXEC", 0))
    temporary = None
    try:
        descriptor, raw = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
        temporary = pathlib.Path(raw)
        try:
            os.write(descriptor, body + b"\n")
            os.fchmod(descriptor, 0o600)
            os.fsync(descriptor)
        finally:
            os.close(descriptor)
        os.replace(temporary, path)
        temporary = None
        os.fsync(parent_fd)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)
        os.close(parent_fd)

replace(manifest_path, canonical_manifest)
replace(signature_path, json.dumps(signature, sort_keys=True, separators=(",", ":"), ensure_ascii=True, allow_nan=False).encode("ascii"))

# Re-read the durable local envelope and reject even a structurally valid
# manifest for another appliance before publishing the source directory.
persisted_manifest = json.loads(manifest_path.read_text(encoding="ascii"))
persisted_signature = json.loads(signature_path.read_text(encoding="ascii"))
if persisted_manifest.get("appliance_id") != appliance_id or persisted_manifest.get("source_authority") != "appliance_local_authorizing":
    raise SystemExit("locally rebound source has the wrong appliance identity")
if persisted_signature != signature or not hmac.compare_digest(
    persisted_signature["hmac_sha256"], hmac.new(key, canonical_manifest, hashlib.sha256).hexdigest()
):
    raise SystemExit("locally rebound source signature is invalid")
PY

mv "$source_staged" "$source_root"; created_paths+=("$source_root"); chown -R root:"$STEWARD_USER" "$source_root"
find "$source_root" -type d -exec chmod g+rx {} +; find "$source_root" -type f -exec chmod g+r {} +
mv "$toolchain_staged" "$toolchain_root"; created_paths+=("$toolchain_root"); chown -R root:"$BUILDER_USER" "$toolchain_root"
find "$toolchain_root" -type d -exec chmod g+rx {} +
find "$toolchain_root" -type f -exec chmod g+r {} +
find "$toolchain_root" -type f -perm /100 -exec chmod g+x {} +
install -d -m 0755 -o root -g root "$release_root"; created_paths+=("$release_root")
mv "$generation_staged" "$release_root/$initial_generation_id"
chown -R root:root "$release_root/$initial_generation_id"
find "$release_root/$initial_generation_id" -type d -exec chmod 0555 {} +
find "$release_root/$initial_generation_id" -type f -exec chmod 0444 {} +
for executable_path in \
    astrid astrid-daemon astrid-build astrid-edge-runtime \
    scripts/warm_ollama_model.sh scripts/report_edge_appliance.py \
    scripts/report_edge_appliance.sh scripts/report_edge_activity.py \
    scripts/report_edge_fleet_activity.py scripts/edge_hindsight.py \
    scripts/astrid_at_a_glance.py scripts/astrid_train.py \
    scripts/retire_edge_origin_mac_affordance.py; do
    chmod 0555 "$release_root/$initial_generation_id/$executable_path"
done
setfacl -R -m "u:$runtime_user:r-X" "$release_root/$initial_generation_id"
/usr/bin/python3 -I -E -s \
    "$release_root/$initial_generation_id/scripts/retire_edge_origin_mac_affordance.py" \
    --workspace-root "$origin_mac_workspace_root" \
    --operator-root "$OPERATOR_STATUS_ROOT" \
    --retirement-root "$origin_mac_retirement_root" \
    --runtime-gid "$runtime_gid" >/dev/null
origin_mac_correction_committed=true
[[ $(stat_values "$OPERATOR_STATUS_ROOT/origin-mac-affordance-retirement.json" | awk '{print $1" "$2" "$3}') == "0 640 1" ]] \
    || die "origin-mac affordance retirement receipt identity is invalid"
[[ $(stat_values "$origin_mac_retirement_root" | awk '{print $1" "$2}') == "0 700" ]] \
    || die "origin-mac retirement root identity is invalid"
for origin_mac_transaction_member in transaction.json receipt.json; do
    [[ $(stat_values "$origin_mac_retirement_root/$origin_mac_transaction_member" | awk '{print $1" "$2" "$3}') == "0 600 1" ]] \
        || die "origin-mac durable transaction member identity is invalid: $origin_mac_transaction_member"
done
install -m 0444 -o root -g root \
    "$release_root/$initial_generation_id/scripts/edge_hindsight.py" \
    "$HINDSIGHT_WRITER_INSTALL"
created_paths+=("$HINDSIGHT_WRITER_INSTALL")
[[ $(stat_values "$HINDSIGHT_WRITER_INSTALL" | awk '{print $1" "$2" "$3}') == '0 444 1' ]] \
    || die "immutable hindsight writer identity is invalid"
[[ $(sha_file "$HINDSIGHT_WRITER_INSTALL") == $(sha_file "$release_root/$initial_generation_id/scripts/edge_hindsight.py") ]] \
    || die "immutable hindsight writer digest mismatch"
sed -e "s|@APPLIANCE_ID@|$appliance_id|g" \
    -e "s|@WORKSPACE@|$runtime_workspace|g" \
    -e "s|@STATE_ROOT@|$runtime_state_mount|g" \
    -e "s|@OPERATOR_ROOT@|$runtime_state_mount/operator/hindsight|g" \
    -e "s|@WRITER_PATH@|$HINDSIGHT_WRITER_INSTALL|g" \
    -e "s|@WRITER_SHA256@|$(sha_file "$HINDSIGHT_WRITER_INSTALL")|g" \
    -e "s|@ACTIVITY_REPORT_PATH@|$OPERATOR_REPORT_ROOT/report_edge_activity.py|g" \
    -e "s|@ACTIVITY_REPORT_SHA256@|$(sha_file "$operator_report_source_root/report_edge_activity.py")|g" \
    -e "s|@OPERATOR_REPORT_MANIFEST_PATH@|$OPERATOR_REPORT_MANIFEST|g" \
    -e "s|@OPERATOR_REPORT_MANIFEST_SHA256@|$operator_report_manifest_sha256|g" \
    "$hindsight_config_template" >"$stage/hindsight-writer.json"
install -m 0440 -o root -g "$runtime_group" "$stage/hindsight-writer.json" "$HINDSIGHT_CONFIG"
created_paths+=("$HINDSIGHT_CONFIG")

if [[ $appliance_id != icp* ]]; then
    install -m 0444 -o root -g root "$audio_feeder_source" "$AUDIO_FEEDER_INSTALL"
    created_paths+=("$AUDIO_FEEDER_INSTALL")
    libasound_link=$(ldconfig -p | awk '$1 == "libasound.so.2" { print $NF; exit }')
    [[ $libasound_link == /* ]] || die "cannot resolve the exact libasound.so.2 runtime"
    libasound_path=$(readlink -f -- "$libasound_link") || die "cannot canonicalize libasound.so.2"
    IFS=' ' read -r libasound_owner libasound_mode libasound_links < <(stat_values "$libasound_path")
    [[ $libasound_owner == 0 && $libasound_mode == 644 && $libasound_links == 1 ]] \
        || die "libasound.so.2 resolved target is not an immutable root-owned 0644 regular file"
    sed -e "s|@APPLIANCE_ID@|$appliance_id|g" \
        -e 's|@CHANNELS@|1|g' -e 's|@DEVICE@|default|g' \
        -e "s|@RUNTIME_UID@|$runtime_uid|g" \
        -e "s|@LIBASOUND_REALPATH@|$libasound_path|g" \
        -e "s|@LIBASOUND_SHA256@|$(sha_file "$libasound_path")|g" \
        -e 's|@SAMPLE_RATE@|16000|g' \
        "$audio_config_template" >"$stage/audio-feeder.json"
    install -m 0440 -o root -g "$AUDIO_USER" "$stage/audio-feeder.json" "$AUDIO_CONFIG"
    created_paths+=("$AUDIO_CONFIG")
else
    [[ ! -e $AUDIO_FEEDER_INSTALL && ! -L $AUDIO_FEEDER_INSTALL && ! -e $AUDIO_CONFIG && ! -L $AUDIO_CONFIG ]] \
        || die "ICP contains an AVADO-only audio feeder artifact"
fi

# Operator-facing reports execute with the SSH user's ambient authority, so
# they must never resolve code through a mutable state/bin directory or the
# candidate-controlled A/B generation. Seal the audited bootstrap copies in a
# separate root-owned tree and expose only fixed launchers using the host
# Python in isolated mode and a fixed PATH.
install -d -m 0755 -o root -g root "$OPERATOR_REPORT_ROOT"
created_paths+=("$OPERATOR_REPORT_ROOT")
for operator_body in astrid_at_a_glance.py astrid_train.py report_edge_appliance.py report_edge_activity.py; do
    case "$operator_body" in
        astrid_at_a_glance.py) operator_launcher=astrid-at-a-glance ;;
        astrid_train.py) operator_launcher=astrid-train ;;
        report_edge_appliance.py) operator_launcher=report-edge-appliance ;;
        report_edge_activity.py) operator_launcher=report-edge-activity ;;
    esac
    install -m 0444 -o root -g root \
        "$release_root/$initial_generation_id/scripts/$operator_body" \
        "$OPERATOR_REPORT_ROOT/$operator_body"
    operator_report_launcher "$operator_body" >"$stage/$operator_launcher"
    install -m 0555 -o root -g root "$stage/$operator_launcher" \
        "$OPERATOR_REPORT_ROOT/$operator_launcher"
done
printf '%s\n' "$operator_report_manifest_content" >"$stage/operator-report-manifest.sha256"
install -m 0444 -o root -g root "$stage/operator-report-manifest.sha256" "$OPERATOR_REPORT_MANIFEST"
chmod 0555 "$OPERATOR_REPORT_ROOT"
[[ $(sha_file "$OPERATOR_REPORT_MANIFEST") == "$operator_report_manifest_sha256" ]] \
    || die "installed immutable operator report manifest digest mismatch"
sha256sum --check --strict --status "$OPERATOR_REPORT_MANIFEST" \
    || die "installed immutable operator report tree failed verification"
for operator_file in MANIFEST.sha256 astrid_at_a_glance.py astrid_train.py report_edge_appliance.py report_edge_activity.py; do
    [[ $(stat_values "$OPERATOR_REPORT_ROOT/$operator_file" | awk '{print $1" "$2" "$3}') == '0 444 1' ]] \
        || die "immutable operator report body identity is invalid: $operator_file"
done
for operator_file in astrid-at-a-glance astrid-train report-edge-appliance report-edge-activity; do
    [[ $(stat_values "$OPERATOR_REPORT_ROOT/$operator_file" | awk '{print $1" "$2" "$3}') == '0 555 1' ]] \
        || die "immutable operator report launcher identity is invalid: $operator_file"
done
install -d -m 0710 -o root -g "$MODEL_LOCK_GROUP" "$state_root"; created_paths+=("$state_root")
install -m 0640 -o root -g "$MODEL_LOCK_GROUP" /dev/null "$model_lock"
[[ $(stat_values "$model_lock" | awk '{print $1" "$2" "$3}') == '0 640 1' ]] || die "persistent model lock ownership/mode/link count is invalid"
[[ $(stat -c '%g' -- "$model_lock") == "$model_lock_gid" ]] || die "persistent model lock has the wrong dedicated group"
for member in "$STEWARD_USER" "$PROVIDER_USER"; do
    member_uid=$(id -u "$member"); member_gid=$(id -g "$member")
    setpriv --reuid="$member_uid" --regid="$member_gid" --groups="$model_lock_gid" \
        test -r "$model_lock" || die "$member cannot open the persistent model lock with its exact service group"
    ! setpriv --reuid="$member_uid" --regid="$member_gid" --groups="$model_lock_gid" \
        test -w "$model_lock" || die "$member unexpectedly has model-lock write authority"
done
! setpriv --reuid="$runtime_uid" --regid="$runtime_gid" --clear-groups test -r "$model_lock" \
    || die "runtime unexpectedly has direct model-lock visibility"
# Immutable rescue code is the only producer for bounded build and generation
# evidence. The steward can inspect the sealed projections, while the mutable
# runtime and the isolated builder/updater identities cannot write them.
install -d -m 2750 -o root -g "$STEWARD_USER" \
    "$introspection_evidence_root" "$build_evidence_root" "$generation_diffs_root"
for evidence_root in "$introspection_evidence_root" "$build_evidence_root" "$generation_diffs_root"; do
    [[ $(stat_values "$evidence_root" | awk '{print $1" "$2}') == "0 2750" ]] \
        || die "immutable introspection evidence root has the wrong owner or mode: $evidence_root"
    runuser -u "$STEWARD_USER" -- test -r "$evidence_root" \
        || die "steward cannot read immutable introspection evidence: $evidence_root"
    for denied_user in "$runtime_user" "$BUILDER_USER" "$UPDATER_USER"; do
        ! runuser -u "$denied_user" -- test -w "$evidence_root" \
            || die "$denied_user can write immutable introspection evidence: $evidence_root"
    done
done
install -d -m 0700 -o "$STEWARD_USER" -g "$STEWARD_USER" "$candidate_root"; created_paths+=("$candidate_root")
install -d -m 0750 -o "$STEWARD_USER" -g "$runtime_group" "$scheduled_authorship_root"
install -d -m 0750 -o "$STEWARD_USER" -g "$runtime_group" "$inquiry_history_root"; created_paths+=("$inquiry_history_root")
runuser -u "$STEWARD_USER" -- test -w "$inquiry_history_root" \
    || die "steward cannot write its dedicated inquiry history"
runuser -u "$runtime_user" -- test -r "$inquiry_history_root" \
    || die "runtime owner cannot read the dedicated inquiry history"
! runuser -u "$runtime_user" -- test -w "$inquiry_history_root" \
    || die "runtime owner can mutate the immutable inquiry history"
install -d -m 0700 -o "$STEWARD_USER" -g "$STEWARD_USER" "$inbox_root"; created_paths+=("$inbox_root")
install -d -m 0700 -o "$STEWARD_USER" -g "$STEWARD_USER" "$candidate_store" "$model_handoff_root"
install -d -m 0710 -o root -g "$BUILDER_USER" "$builder_root"; created_paths+=("$builder_root")
install -d -m 0700 -o root -g root "$updater_root" "$state_snapshots" "$unit_transactions" "$profile_transactions" "$system_unit_alias"; created_paths+=("$updater_root")
install -d -m 0700 -o "$UPDATER_USER" -g "$UPDATER_USER" "$generation_staging"
runuser -u "$UPDATER_USER" -- test -w "$generation_staging" || die "updater cannot write its exact generation staging root"
for denied_user in "$runtime_user" "$STEWARD_USER" "$BUILDER_USER"; do
    ! runuser -u "$denied_user" -- test -r "$generation_staging" || die "$denied_user can read updater generation staging"
    ! runuser -u "$denied_user" -- test -w "$generation_staging" || die "$denied_user can write updater generation staging"
done

# The immutable helper may update only the reviewed Astrid unit fragments. It
# receives the live manager tree through this root-only bind alias instead of
# exposing /etc/systemd/system inside the supervisor namespace. Candidate and
# runtime identities cannot traverse the alias's 0700 updater parent.
system_unit_alias_stage=$stage/system-unit-alias
mkdir -p "$system_unit_alias_stage"
sed -e "s|@@UPDATER_ROOT@@|$updater_root|g" \
    -e "s|@@SYSTEM_UNIT_ALIAS@@|$system_unit_alias|g" \
    "$system_unit_alias_mount_template" >"$system_unit_alias_stage/$system_unit_alias_mount_unit"
if grep -E '@@[A-Z0-9_]+@@' "$system_unit_alias_stage/$system_unit_alias_mount_unit" >/dev/null; then
    die "private system-unit alias mount rendering left an unresolved placeholder"
fi
chmod 0644 "$system_unit_alias_stage/$system_unit_alias_mount_unit"
systemd-analyze verify "$system_unit_alias_stage/$system_unit_alias_mount_unit"
install -m 0644 -o root -g root "$system_unit_alias_stage/$system_unit_alias_mount_unit" \
    "$system_unit_root/$system_unit_alias_mount_unit"
created_paths+=("$system_unit_root/$system_unit_alias_mount_unit")
systemctl daemon-reload
systemctl enable "$system_unit_alias_mount_unit"; enabled_now+=("$system_unit_alias_mount_unit")
started_now+=("$system_unit_alias_mount_unit"); systemctl start "$system_unit_alias_mount_unit"
[[ $(findmnt -rn -M "$system_unit_alias" -o TARGET) == "$system_unit_alias" ]] \
    || die "private system-unit alias did not become an exact mountpoint"
[[ $(stat -c '%D:%i' -- "$system_unit_alias") == $(stat -c '%D:%i' -- "$system_unit_root") ]] \
    || die "private system-unit alias does not bind the live manager tree"
for denied_user in "$runtime_user" "$STEWARD_USER" "$BUILDER_USER" "$UPDATER_USER"; do
    ! runuser -u "$denied_user" -- test -r "$system_unit_alias" \
        || die "$denied_user can traverse the private system-unit alias"
done
ln -s "releases/$initial_generation_id" "$release_parent/slot-a"; created_paths+=("$release_parent/slot-a")
ln -s "releases/$initial_generation_id" "$release_parent/slot-b"; created_paths+=("$release_parent/slot-b")
ln -s "releases/$initial_generation_id" "$release_parent/current"; created_paths+=("$release_parent/current")
printf '%s\n' "$initial_generation_id" >"$GENERATION_FILE"; chmod 0444 "$GENERATION_FILE"; chown root:root "$GENERATION_FILE"; created_paths+=("$GENERATION_FILE")

# Allocate two independent, fixed-capacity state filesystems before any mutable
# generation can run.  The currently live user services are not interrupted
# here: the nested manager performs the stopped-copy cutover later, after all
# immutable units and rollback material have been staged.
install -d -m 0700 -o root -g root "$state_volume_root" "$state_migration_root"
created_paths+=("$state_volume_root" "$runtime_state_image" "$rollback_state_image" "$STATE_STORE_CONFIG")
if [[ $appliance_id == icp* ]]; then
    for immutable_root in "$state_root" "$release_parent" "$source_root" \
        "$builder_root" "$updater_root" "$toolchain_root" "$state_volume_root"; do
        require_root_ancestry "$immutable_root" /media/data "installed ICP immutable root"
    done
    require_root_ancestry "$candidate_root" /media/data "installed ICP candidate root" mutable
fi
state_initialize_args=(initialize --config "$STATE_STORE_CONFIG" --appliance-id "$appliance_id" \
    --runtime-image "$runtime_state_image" --runtime-mount "$runtime_state_mount" \
    --runtime-uid "$runtime_uid" --runtime-gid "$runtime_gid" \
    --rollback-image "$rollback_state_image" --rollback-mount "$rollback_state_mount" \
    --migration-journal "$state_migration_journal" \
    --runtime-source-backup "$runtime_state_backup" --rollback-source-backup "$rollback_state_backup")
if [[ -n $required_mount ]]; then
    state_initialize_args+=(--required-backing-mount "$required_mount" --required-backing-uuid "$required_mount_uuid")
fi
/usr/bin/python3 -I -E -s "$STATE_STORE_INSTALL" "${state_initialize_args[@]}"
mapfile -t state_mount_units < <(/usr/bin/python3 -I -E -s "$STATE_STORE_INSTALL" unit-names --config "$STATE_STORE_CONFIG")
[[ ${#state_mount_units[@]} == 2 ]] || die "state-store helper returned an invalid mount-unit set"
runtime_state_mount_unit=${state_mount_units[0]}
rollback_state_mount_unit=${state_mount_units[1]}
[[ $runtime_state_mount_unit == *.mount && $rollback_state_mount_unit == *.mount && $runtime_state_mount_unit != "$rollback_state_mount_unit" ]] \
    || die "state-store mount-unit identities are invalid"
runtime_state_mount_unit_sed=${runtime_state_mount_unit//\\/\\\\}
rollback_state_mount_unit_sed=${rollback_state_mount_unit//\\/\\\\}
state_unit_stage=$stage/state-store-units
mkdir -p "$state_unit_stage"
sed -e "s|@@ACTIVE_GENERATION_ROOT@@|$release_parent/current|g" \
    -e "s|@@RUNTIME_IMAGE_PARENT@@|${runtime_state_image%/*}|g" \
    -e "s|@@RUNTIME_IMAGE@@|$runtime_state_image|g" \
    -e "s|@@RUNTIME_STATE_ROOT@@|$runtime_state_mount|g" \
    "$state_store_runtime_mount_template" >"$state_unit_stage/$runtime_state_mount_unit"
sed -e "s|@@ACTIVE_GENERATION_ROOT@@|$release_parent/current|g" \
    -e "s|@@ROLLBACK_IMAGE_PARENT@@|${rollback_state_image%/*}|g" \
    -e "s|@@ROLLBACK_IMAGE@@|$rollback_state_image|g" \
    -e "s|@@ROLLBACK_STATE_ROOT@@|$rollback_state_mount|g" \
    "$state_store_rollback_mount_template" >"$state_unit_stage/$rollback_state_mount_unit"
sed -e "s|@@ACTIVE_GENERATION_ROOT@@|$release_parent/current|g" \
    -e "s|@@RUNTIME_MOUNT_UNIT@@|$runtime_state_mount_unit_sed|g" \
    -e "s|@@ROLLBACK_MOUNT_UNIT@@|$rollback_state_mount_unit_sed|g" \
    -e "s|@@BACKING_ROOT@@|$state_volume_root|g" \
    -e "s|@@RUNTIME_IMAGE@@|$runtime_state_image|g" \
    -e "s|@@ROLLBACK_IMAGE@@|$rollback_state_image|g" \
    -e "s|@@RUNTIME_MOUNT_PARENT@@|${runtime_state_mount%/*}|g" \
    -e "s|@@ROLLBACK_MOUNT_PARENT@@|${rollback_state_mount%/*}|g" \
    -e "s|@@MIGRATION_ROOT@@|$state_migration_root|g" \
    "$state_store_migration_recover_template" >"$state_unit_stage/$STATE_STORE_MIGRATION_RECOVER_UNIT"
sed -e "s|@@ACTIVE_GENERATION_ROOT@@|$release_parent/current|g" \
    -e "s|@@RUNTIME_MOUNT_UNIT@@|$runtime_state_mount_unit_sed|g" \
    -e "s|@@ROLLBACK_MOUNT_UNIT@@|$rollback_state_mount_unit_sed|g" \
    -e "s|@@BACKING_ROOT@@|$state_volume_root|g" \
    -e "s|@@RUNTIME_IMAGE@@|$runtime_state_image|g" \
    -e "s|@@ROLLBACK_IMAGE@@|$rollback_state_image|g" \
    -e "s|@@RUNTIME_STATE_ROOT@@|$runtime_state_mount|g" \
    -e "s|@@ROLLBACK_STATE_ROOT@@|$rollback_state_mount|g" \
    -e "s|@@MIGRATION_ROOT@@|$state_migration_root|g" \
    "$state_store_recover_template" >"$state_unit_stage/$STATE_STORE_RECOVER_UNIT"
sed -e "s|@@ACTIVE_GENERATION_ROOT@@|$release_parent/current|g" \
    -e "s|@@RUNTIME_MOUNT_UNIT@@|$runtime_state_mount_unit_sed|g" \
    -e "s|@@ROLLBACK_MOUNT_UNIT@@|$rollback_state_mount_unit_sed|g" \
    -e "s|@@RUNTIME_IMAGE@@|$runtime_state_image|g" \
    -e "s|@@ROLLBACK_IMAGE@@|$rollback_state_image|g" \
    -e "s|@@MIGRATION_ROOT@@|$state_migration_root|g" \
    -e "s|@@RUNTIME_STATE_ROOT@@|$runtime_state_mount|g" \
    -e "s|@@ROLLBACK_STATE_ROOT@@|$rollback_state_mount|g" \
    "$state_store_verify_template" >"$state_unit_stage/$STATE_STORE_VERIFY_UNIT"
cp "$state_store_health_service" "$state_unit_stage/$STATE_STORE_HEALTH_UNIT"
cp "$state_store_health_timer" "$state_unit_stage/$STATE_STORE_HEALTH_TIMER"
if grep -R -E '@@[A-Z0-9_]+@@' "$state_unit_stage" >/dev/null; then die "state-store unit rendering left an unresolved placeholder"; fi
chmod 0644 "$state_unit_stage"/*
SYSTEMD_UNIT_PATH="$state_unit_stage:" systemd-analyze verify \
    "$runtime_state_mount_unit" "$rollback_state_mount_unit" "$STATE_STORE_MIGRATION_RECOVER_UNIT" "$STATE_STORE_RECOVER_UNIT" \
    "$STATE_STORE_VERIFY_UNIT" "$STATE_STORE_HEALTH_UNIT" "$STATE_STORE_HEALTH_TIMER"
for unit in "$runtime_state_mount_unit" "$rollback_state_mount_unit" "$STATE_STORE_MIGRATION_RECOVER_UNIT" "$STATE_STORE_RECOVER_UNIT" "$STATE_STORE_VERIFY_UNIT" "$STATE_STORE_HEALTH_UNIT" "$STATE_STORE_HEALTH_TIMER"; do
    install -m 0644 -o root -g root "$state_unit_stage/$unit" "$system_unit_root/$unit"
    created_paths+=("$system_unit_root/$unit")
done
systemctl daemon-reload
for unit in "$runtime_state_mount_unit" "$rollback_state_mount_unit" "$STATE_STORE_MIGRATION_RECOVER_UNIT" "$STATE_STORE_RECOVER_UNIT" "$STATE_STORE_VERIFY_UNIT" "$STATE_STORE_HEALTH_TIMER"; do
    systemctl enable "$unit"
    enabled_now+=("$unit")
done

# Allocate the bounded build filesystem before the rescue configuration can
# authorize any build.  The image and config are exact no-overwrite transaction
# members; rollback stops verifier then mount before removing either one.
created_paths+=("$builder_image" "$BUILDER_STORE_CONFIG")
builder_initialize_args=(initialize --config "$BUILDER_STORE_CONFIG" --image "$builder_image" --mount "$builder_root" --builder-uid "$builder_uid" --builder-gid "$builder_gid")
if [[ -n $required_mount ]]; then
    builder_initialize_args+=(--required-backing-mount "$required_mount" --required-backing-uuid "$required_mount_uuid")
fi
/usr/bin/python3 -I -E -s "$BUILDER_STORE_INSTALL" "${builder_initialize_args[@]}"
[[ $(/usr/bin/python3 -I -E -s "$BUILDER_STORE_INSTALL" unit-name --config "$BUILDER_STORE_CONFIG") == "$builder_mount_unit" ]] \
    || die "builder-store helper and installer disagree on the escaped mount unit"
builder_unit_stage=$stage/builder-units
mkdir -p "$builder_unit_stage"
sed -e "s|@@ACTIVE_GENERATION_ROOT@@|$release_parent/current|g" \
    -e "s|@@BUILDER_IMAGE_PARENT@@|${builder_image%/*}|g" \
    -e "s|@@BUILDER_IMAGE@@|$builder_image|g" \
    -e "s|@@BUILDER_ROOT@@|$builder_root|g" \
    "$builder_store_mount_template" >"$builder_unit_stage/$builder_mount_unit"
sed -e "s|@@ACTIVE_GENERATION_ROOT@@|$release_parent/current|g" \
    -e "s|@@BUILDER_MOUNT_UNIT@@|$builder_mount_unit_sed|g" \
    -e "s|@@BUILDER_IMAGE@@|$builder_image|g" \
    -e "s|@@BUILDER_ROOT@@|$builder_root|g" \
    "$builder_store_verify_template" >"$builder_unit_stage/$BUILDER_STORE_VERIFY_UNIT"
if grep -R -E '@@[A-Z0-9_]+@@' "$builder_unit_stage" >/dev/null; then die "builder-store unit rendering left an unresolved placeholder"; fi
chmod 0644 "$builder_unit_stage/$builder_mount_unit" "$builder_unit_stage/$BUILDER_STORE_VERIFY_UNIT"
systemd-analyze verify "$builder_unit_stage/$builder_mount_unit" "$builder_unit_stage/$BUILDER_STORE_VERIFY_UNIT"
for unit in "$builder_mount_unit" "$BUILDER_STORE_VERIFY_UNIT"; do
    install -m 0644 -o root -g root "$builder_unit_stage/$unit" "$system_unit_root/$unit"
    created_paths+=("$system_unit_root/$unit")
done
systemctl daemon-reload
for unit in "$builder_mount_unit" "$BUILDER_STORE_VERIFY_UNIT"; do systemctl enable "$unit"; enabled_now+=("$unit"); done
started_now+=("$builder_mount_unit"); systemctl start "$builder_mount_unit"
/usr/bin/python3 -I -E -s "$BUILDER_STORE_INSTALL" prepare --config "$BUILDER_STORE_CONFIG"
started_now+=("$BUILDER_STORE_VERIFY_UNIT"); systemctl start "$BUILDER_STORE_VERIFY_UNIT"
for unit in "$builder_mount_unit" "$BUILDER_STORE_VERIFY_UNIT"; do systemctl is-active --quiet "$unit" || die "builder-store unit did not become active: $unit"; done

cat >"$PROFILES_CONFIG" <<EOF
{"profiles":{"activate":{"argv":["--config","$RESCUE_CONFIG","activate","--generation-dir","{generation_dir}","--previous-generation-dir","{previous_generation_dir}"],"candidate_argv":false,"executable":"$rescue_helper_install_path","executable_sha256":"$rescue_helper_sha256","network":"deny","privilege_envelope":"root-activator:ab-link-and-service-only:v1","run_as_gid":0,"run_as_uid":0,"shell":false,"timeout_seconds":3600},"build":{"argv":["--config","$RESCUE_CONFIG","build","--candidate-manifest","{candidate_manifest}","--intent-envelope","{intent_envelope}","--model-handoff","{model_handoff}","--build-manifest","{build_manifest}"],"candidate_argv":false,"executable":"$rescue_helper_install_path","executable_sha256":"$rescue_helper_sha256","network":"deny","privilege_envelope":"offline-build-sandbox:no-host-state:v1","run_as_gid":0,"run_as_uid":0,"shell":false,"timeout_seconds":90000},"health":{"argv":["--config","$RESCUE_CONFIG","health"],"candidate_argv":false,"executable":"$rescue_helper_install_path","executable_sha256":"$rescue_helper_sha256","network":"deny","privilege_envelope":"read-only-health:v1","run_as_gid":0,"run_as_uid":0,"shell":false,"timeout_seconds":600},"install":{"argv":["--config","$RESCUE_CONFIG","install","--build-manifest","{build_manifest}"],"candidate_argv":false,"executable":"$rescue_helper_install_path","executable_sha256":"$rescue_helper_sha256","network":"deny","privilege_envelope":"root-stager:release-root-only:v1","run_as_gid":0,"run_as_uid":0,"shell":false,"timeout_seconds":1800},"retention":{"argv":["--config","$RESCUE_CONFIG","retention"],"candidate_argv":false,"executable":"$rescue_helper_install_path","executable_sha256":"$rescue_helper_sha256","network":"deny","privilege_envelope":"root-retention:paired-generation-snapshot-only:v1","run_as_gid":0,"run_as_uid":0,"shell":false,"timeout_seconds":600},"rollback":{"argv":["--config","$RESCUE_CONFIG","rollback","--generation-dir","{generation_dir}"],"candidate_argv":false,"executable":"$rescue_helper_install_path","executable_sha256":"$rescue_helper_sha256","network":"deny","privilege_envelope":"root-rollback:ab-link-and-service-only:v1","run_as_gid":0,"run_as_uid":0,"shell":false,"timeout_seconds":3600},"synthetic":{"argv":["--config","$RESCUE_CONFIG","synthetic-lifecycle"],"candidate_argv":false,"executable":"$rescue_helper_install_path","executable_sha256":"$rescue_helper_sha256","network":"deny","privilege_envelope":"operator-synthetic:offline-build-model-unloaded:v1","run_as_gid":0,"run_as_uid":0,"shell":false,"timeout_seconds":7200}},"schema":"astrid.edge_self_change.command_profiles.v1","trusted_executable_roots":["/usr/libexec/astrid"]}
EOF
chmod 0400 "$PROFILES_CONFIG"; chown root:root "$PROFILES_CONFIG"; created_paths+=("$PROFILES_CONFIG")
cat >"$SUPERVISOR_CONFIG" <<EOF
{"schema":"astrid.edge_self_change.config.v1","state_root":"$state_root","releases_root":"$release_root","active_link":"$release_parent/current","signing_key":"$SUPERVISOR_KEY","intent_attestation_key":"$INTENT_KEY","command_profiles":"$PROFILES_CONFIG","operator_status":"$OPERATOR_STATUS","model_handoff_root":"$model_handoff_root","appliance_id":"$appliance_id","target":"$target"}
EOF
chmod 0400 "$SUPERVISOR_CONFIG"; chown root:root "$SUPERVISOR_CONFIG"; created_paths+=("$SUPERVISOR_CONFIG")

# `init` exclusively creates both supervisor keys. Precreating either one would
# violate its no-overwrite contract and make a real bootstrap impossible.
/usr/bin/python3 -I -E -s "$supervisor_install_path" --help >/dev/null
created_paths+=("$SUPERVISOR_KEY" "$INTENT_KEY")
/usr/bin/python3 -I -E -s "$supervisor_install_path" --config "$SUPERVISOR_CONFIG" --execute init
for key in "$SUPERVISOR_KEY" "$INTENT_KEY"; do
    [[ -f $key && ! -L $key && $(wc -c <"$key" | tr -d ' ') == 32 ]] || die "supervisor created an invalid key"
    [[ $(stat_values "$key" | awk '{print $1" "$2" "$3}') == '0 600 1' ]] || die "supervisor key ownership/mode/link count is invalid"
done
secret_key_hashes=(
    "$(sha_file "$SUPERVISOR_KEY")" "$(sha_file "$INTENT_KEY")" "$(sha_file "$SOURCE_KEY")"
    "$ledger_attestation_sha256"
    "$core_web_request_sha256" "$runtime_web_request_sha256" "$steward_web_request_sha256"
    "$web_response_signing_sha256" "$runtime_provider_sha256" "$steward_provider_sha256"
    "$warmup_provider_sha256" "$provider_ledger_sha256"
)
[[ ${#secret_key_hashes[@]} == 12 ]] || die "secret trust-domain inventory changed"
[[ $(printf '%s\n' "${secret_key_hashes[@]}" | LC_ALL=C sort -u | wc -l | tr -d ' ') == ${#secret_key_hashes[@]} ]] \
    || die "independent secret trust-domain credentials collide"
[[ -f $OPERATOR_STATUS && ! -L $OPERATOR_STATUS && $(stat_values "$OPERATOR_STATUS" | awk '{print $1" "$2" "$3}') == '0 640 1' ]] || die "operator status projection ownership/mode/link count is invalid"
[[ $(stat -c '%g' -- "$OPERATOR_STATUS") == "$runtime_gid" ]] || die "operator status projection is not runtime-group-readable"
/usr/bin/python3 -I -E -s - "$OPERATOR_STATUS" <<'PY'
import hashlib, json, sys
value = json.load(open(sys.argv[1], encoding="utf-8"))
if set(value) != {"schema", "core", "core_sha256"} or value["schema"] != "astrid.edge_self_change.operator_status_envelope.v1":
    raise SystemExit("operator projection envelope is invalid")
core = value["core"]
if core.get("provenance") != "immutable_supervisor_sanitized_projection" or core.get("authority") != "observation_only_not_deployment_authority" or core.get("mode") != "paused":
    raise SystemExit("operator projection provenance is invalid")
encoded = json.dumps(core, sort_keys=True, separators=(",", ":"), ensure_ascii=True, allow_nan=False).encode("ascii")
if hashlib.sha256(encoded).hexdigest() != value["core_sha256"]:
    raise SystemExit("operator projection hash is invalid")
expected_core = {
    "schema", "appliance_id", "generated_at", "state_revision", "mode",
    "active_generation", "previous_generation", "pipeline_phase",
    "latest_transition", "restart_expectation", "lifecycle", "provenance",
    "authority",
}
if set(core) != expected_core or core.get("schema") != "astrid.edge_self_change.operator_status.v3":
    raise SystemExit("operator projection core shape is not exact v3")
lifecycle = core.get("lifecycle")
if not isinstance(lifecycle, dict) or set(lifecycle) != {
    "schema", "events", "included", "total", "truncated", "maximum_events",
    "ledger_heads",
} or lifecycle.get("schema") != "astrid.edge_self_change.operator_lifecycle.v1":
    raise SystemExit("operator projection lifecycle shape is invalid")
for forbidden in ("key_id", "signing_key", "intent_attestor", "inbox", "prompt", "response", "diff", "build_log"):
    if forbidden in core:
        raise SystemExit("operator projection exposes a forbidden private field")
PY

owned_json=
for ((index=0; index<${#owned_paths[@]}; index++)); do
    [[ -z $owned_json ]] || owned_json+=,
    owned_json+="{\"kind\":\"${owned_kinds[$index]}\",\"path\":\"${owned_paths[$index]}\",\"maximum_files\":50,\"maximum_bytes_per_file\":65536}"
done
source_manifest_sha256=$(sha_file "$source_root/MANIFEST.json")
expected_source_id=$(/usr/bin/python3 -I -E -s - "$source_root/MANIFEST.json" "$appliance_id" <<'PY'
import hashlib, json, re, sys
value = json.load(open(sys.argv[1], encoding="utf-8"))
appliance_id = sys.argv[2]
source_id = value.get("source_id")
identity = {
    "schema": "astrid.edge.self_change_source_identity.v1",
    "appliance_id": appliance_id,
    "source_authority": "appliance_local_authorizing",
    "repository_commit": value.get("repository_commit"),
    "rustc": value.get("rustc"),
    "files": value.get("files"),
}
identity_hash = hashlib.sha256(json.dumps(identity, sort_keys=True, separators=(",", ":"), ensure_ascii=True, allow_nan=False).encode("ascii")).hexdigest()
if (
    value.get("appliance_id") != appliance_id
    or value.get("source_authority") != "appliance_local_authorizing"
    or value.get("source_identity_sha256") != identity_hash
    or source_id != f"cpu-edge:{identity_hash}"
):
    raise SystemExit("signed source manifest is not bound to this appliance")
print(source_id)
PY
)
[[ $expected_source_id =~ ^cpu-edge:[0-9a-f]{64}$ ]] || die "signed source manifest has an invalid source_id"
source_key_sha256=$(sha_file "$SOURCE_KEY")
intent_key_sha256=$(sha_file "$INTENT_KEY")
cat >"$PROVIDER_CONFIG" <<EOF
{"schema":"astrid.edge.provider_broker.config.v1","appliance_id":"$appliance_id","ollama_origin":"$ollama_origin","model":"$model","keep_alive":"2h","context_tokens":$context_tokens,"maximum_output_tokens":512,"maximum_request_body_bytes":131072,"maximum_response_body_bytes":8388608,"connect_timeout_ms":$connect_timeout_ms,"header_timeout_ms":$header_timeout_ms,"inter_chunk_timeout_ms":120000,"total_timeout_ms":$total_timeout_ms,"client_read_timeout_ms":5000,"client_write_timeout_ms":120000,"maximum_concurrent_requests":1,"model_lock":"$model_lock","maintenance_lease":"$maintenance_lease","reflection_lease":"/run/astrid-edge-self-change/reflection.json","ledger_path":"/var/lib/astrid-edge-provider/receipts.jsonl","runtime":{"client_id":"edge-runtime","expected_peer_uid":$runtime_uid,"socket_path":"/run/astrid-edge-self-change/provider-runtime.sock","socket_gid":$provider_runtime_gid,"request_key_sha256":"$runtime_provider_sha256","maximum_requests_per_hour":48,"maximum_output_tokens":$output_tokens},"steward":{"client_id":"edge-steward","expected_peer_uid":$steward_uid,"socket_path":"/run/astrid-edge-self-change/provider-steward.sock","socket_gid":$provider_steward_gid,"request_key_sha256":"$steward_provider_sha256","maximum_requests_per_hour":32,"maximum_output_tokens":$reflection_output_tokens},"warmup":{"client_id":"model-warmup","expected_peer_uid":$warmup_uid,"socket_path":"/run/astrid-edge-self-change/provider-warmup.sock","socket_gid":$provider_warmup_gid,"request_key_sha256":"$warmup_provider_sha256","maximum_requests_per_hour":12,"maximum_output_tokens":2},"ledger_key_sha256":"$provider_ledger_sha256"}
EOF
chmod 0440 "$PROVIDER_CONFIG"; chown root:"$PROVIDER_USER" "$PROVIDER_CONFIG"; created_paths+=("$PROVIDER_CONFIG")
cat >"$STEWARD_CONFIG" <<EOF
{"schema":"astrid.edge.steward_helper.config.v1","appliance_id":"$appliance_id","target":"$target","model":"$model","ollama_origin":"$ollama_origin","provider_broker":{"socket_path":"/run/astrid-edge-self-change/provider-steward.sock","request_key_path":"$STEWARD_PROVIDER_REQUEST_KEY","request_key_sha256":"$steward_provider_sha256"},"connect_timeout_ms":$connect_timeout_ms,"header_timeout_ms":$header_timeout_ms,"total_timeout_ms":$total_timeout_ms,"web_broker":{"socket_path":"$WEB_STEWARD_SOCKET","request_key_path":"$STEWARD_WEB_REQUEST_KEY","request_key_sha256":"$steward_web_request_sha256","response_verify_key_path":"$WEB_RESPONSE_VERIFY_KEY","response_verify_key_sha256":"$web_response_verify_sha256","connect_timeout_ms":1000,"header_timeout_ms":10000,"total_timeout_ms":20000,"result_limit":5},"context_tokens":$context_tokens,"output_tokens":$reflection_output_tokens,"source_authoring_output_tokens":$source_authoring_output_tokens,"model_lock":"$model_lock","workspace_root":"$runtime_workspace","workspace_uid":$runtime_uid,"workspace_gid":$runtime_gid,"source_root":"$source_root","source_manifest":"$source_root/MANIFEST.json","source_manifest_sha256":"$source_manifest_sha256","source_signature":"$source_root/MANIFEST.signature.json","expected_source_id":"$expected_source_id","active_generation_link":"$release_parent/current","maintenance_lease":"$maintenance_lease","source_signing_key":"$SOURCE_KEY","source_signing_key_sha256":"$source_key_sha256","attestor_key":"$INTENT_KEY","attestor_key_sha256":"$intent_key_sha256","state_root":"$candidate_root","inquiry_history_root":"$inquiry_history_root","supervisor_inbox":"$inbox_root","supervisor_status":"$SUPERVISOR_STATUS","current_generation":"$GENERATION_FILE","patch_export_root":"$steward_patch_outbox","owned_inputs":[$owned_json],"gates":{"autonomy_state":"$autonomy_state","action_receipts":"$action_receipts","thermal_celsius":"$thermal_celsius","maximum_thermal_celsius":$maximum_thermal_celsius}}
EOF
chmod 0440 "$STEWARD_CONFIG"; chown root:"$STEWARD_USER" "$STEWARD_CONFIG"; created_paths+=("$STEWARD_CONFIG")

# Derive only the public half of the steward's domain-separated Ed25519
# authorship key.  The mutable runtime receives this fixed 32-byte verifier as
# a systemd credential; the root-only intent key never crosses the boundary.
scheduled_authorship_verify_key_hex=$("$helper_install_path" --config "$STEWARD_CONFIG" --print-scheduled-authorship-verifying-key)
[[ $scheduled_authorship_verify_key_hex =~ ^[0-9a-f]{64}$ ]] \
    || die "steward returned a malformed scheduled-authorship public key"
/usr/bin/python3 -I -E -s - "$SCHEDULED_AUTHORSHIP_VERIFY_KEY" "$scheduled_authorship_verify_key_hex" "$runtime_gid" <<'PY'
import os, pathlib, sys

destination = pathlib.Path(sys.argv[1])
key = bytes.fromhex(sys.argv[2])
gid = int(sys.argv[3])
if len(key) != 32 or destination.parent != pathlib.Path("/etc/astrid"):
    raise SystemExit("scheduled-authorship public key parameters are invalid")
flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
descriptor = os.open(destination, flags, 0o440)
try:
    os.fchown(descriptor, 0, gid)
    os.fchmod(descriptor, 0o440)
    written = 0
    while written < len(key):
        count = os.write(descriptor, key[written:])
        if count <= 0:
            raise SystemExit("scheduled-authorship public key write made no progress")
        written += count
    os.fsync(descriptor)
finally:
    os.close(descriptor)
parent = os.open(destination.parent, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_CLOEXEC", 0))
try:
    os.fsync(parent)
finally:
    os.close(parent)
PY
created_paths+=("$SCHEDULED_AUTHORSHIP_VERIFY_KEY")
[[ $(stat_values "$SCHEDULED_AUTHORSHIP_VERIFY_KEY" | awk '{print $1" "$2" "$3}') == '0 440 1' \
    && $(stat -c '%g' -- "$SCHEDULED_AUTHORSHIP_VERIFY_KEY") == "$runtime_gid" \
    && $(wc -c <"$SCHEDULED_AUTHORSHIP_VERIFY_KEY" | tr -d ' ') == 32 ]] \
    || die "scheduled-authorship public key identity is invalid"
scheduled_authorship_verify_key_sha256=$(sha_file "$SCHEDULED_AUTHORSHIP_VERIFY_KEY")
[[ $scheduled_authorship_verify_key_sha256 =~ ^[0-9a-f]{64}$ ]] \
    || die "scheduled-authorship public key digest is invalid"

cat >"$WEB_CORE_CONFIG" <<EOF
{"client_id":"edge-core","client_read_timeout_ms":5000,"client_write_timeout_ms":5000,"connect_timeout_ms":5000,"expected_peer_uid":$runtime_uid,"header_timeout_ms":15000,"maximum_concurrent_requests":2,"maximum_request_body_bytes":4096,"maximum_results":5,"maximum_searches_per_hour":8,"maximum_searches_per_utc_day":24,"maximum_upstream_body_bytes":1048576,"quota_state_path":"/var/lib/astrid-edge-web-core/search-quota.jsonl","request_key_path":"/run/credentials/$WEB_CORE_SERVICE_UNIT/request.key","request_key_sha256":"$core_web_request_sha256","response_signing_key_path":"/run/credentials/$WEB_CORE_SERVICE_UNIT/response-signing.key","response_signing_key_sha256":"$web_response_signing_sha256","response_verify_key_sha256":"$web_response_verify_sha256","schema":"astrid.edge.web_broker.config.v3","socket_gid":$web_core_client_gid,"socket_path":"$WEB_CORE_SOCKET","total_timeout_ms":30000,"upstream_origin":"https://search.brave.com/search"}
EOF
cat >"$WEB_RUNTIME_CONFIG" <<EOF
{"client_id":"edge-runtime","client_read_timeout_ms":5000,"client_write_timeout_ms":5000,"connect_timeout_ms":5000,"expected_peer_uid":$runtime_uid,"header_timeout_ms":15000,"maximum_concurrent_requests":2,"maximum_request_body_bytes":4096,"maximum_results":5,"maximum_searches_per_hour":8,"maximum_searches_per_utc_day":24,"maximum_upstream_body_bytes":1048576,"quota_state_path":"/var/lib/astrid-edge-web-runtime/search-quota.jsonl","request_key_path":"/run/credentials/$WEB_RUNTIME_SERVICE_UNIT/request.key","request_key_sha256":"$runtime_web_request_sha256","response_signing_key_path":"/run/credentials/$WEB_RUNTIME_SERVICE_UNIT/response-signing.key","response_signing_key_sha256":"$web_response_signing_sha256","response_verify_key_sha256":"$web_response_verify_sha256","schema":"astrid.edge.web_broker.config.v3","socket_gid":$web_runtime_client_gid,"socket_path":"$WEB_RUNTIME_SOCKET","total_timeout_ms":30000,"upstream_origin":"https://search.brave.com/search"}
EOF
cat >"$WEB_STEWARD_CONFIG" <<EOF
{"client_id":"edge-steward","client_read_timeout_ms":5000,"client_write_timeout_ms":5000,"connect_timeout_ms":5000,"expected_peer_uid":$steward_uid,"header_timeout_ms":15000,"maximum_concurrent_requests":2,"maximum_request_body_bytes":4096,"maximum_results":5,"maximum_searches_per_hour":2,"maximum_searches_per_utc_day":12,"maximum_upstream_body_bytes":1048576,"quota_state_path":"/var/lib/astrid-edge-web-steward/search-quota.jsonl","request_key_path":"/run/credentials/$WEB_STEWARD_SERVICE_UNIT/request.key","request_key_sha256":"$steward_web_request_sha256","response_signing_key_path":"/run/credentials/$WEB_STEWARD_SERVICE_UNIT/response-signing.key","response_signing_key_sha256":"$web_response_signing_sha256","response_verify_key_sha256":"$web_response_verify_sha256","schema":"astrid.edge.web_broker.config.v3","socket_gid":$steward_gid,"socket_path":"$WEB_STEWARD_SOCKET","total_timeout_ms":30000,"upstream_origin":"https://search.brave.com/search"}
EOF
for config in "$WEB_CORE_CONFIG" "$WEB_RUNTIME_CONFIG" "$WEB_STEWARD_CONFIG"; do chmod 0440 "$config"; chown root:"$WEB_USER" "$config"; created_paths+=("$config"); done

cargo_path=$toolchain_root/toolchain/bin/cargo
rustc_path=$toolchain_root/toolchain/bin/rustc
rustfmt_path=$toolchain_root/toolchain/bin/rustfmt
python_path=$(readlink -f -- /usr/bin/python3)
systemctl_path=$(readlink -f -- /usr/bin/systemctl)
systemd_run_path=$(readlink -f -- /usr/bin/systemd-run)
systemd_analyze_path=$(readlink -f -- /usr/bin/systemd-analyze)
[[ $systemd_run_path == /usr/bin/systemd-run ]] \
    || die "candidate transient-unit launcher is not the exact immutable path"
for trusted_path in "$cargo_path" "$rustc_path" "$rustfmt_path" "$python_path" "$systemctl_path" "$systemd_run_path" "$systemd_analyze_path"; do
    [[ -f $trusted_path && ! -L $trusted_path && -x $trusted_path ]] || die "trusted rescue executable is absent or linked: $trusted_path"
    [[ $(stat_values "$trusted_path" | awk '{print $1" "$3}') == '0 1' ]] || die "trusted rescue executable is not root-owned single-link content: $trusted_path"
done
sed -e "s|@@APPLIANCE_ID@@|$appliance_id|g" \
    -e "s|@@TARGET@@|$target|g" \
    -e "s|@@PYTHON_PATH@@|$python_path|g" \
    -e "s|@@PYTHON_SHA256@@|$(sha_file "$python_path")|g" \
    "$presentation_config_template" >"$PRESENTATION_CONFIG"
if grep -E '@@[A-Z0-9_]+@@' "$PRESENTATION_CONFIG" >/dev/null; then
    die "presentation-broker config rendering left an unresolved placeholder"
fi
chmod 0440 "$PRESENTATION_CONFIG"
chown root:"$PRESENTATION_USER" "$PRESENTATION_CONFIG"
created_paths+=("$PRESENTATION_CONFIG")
/usr/bin/python3 -I -E -s - "$PRESENTATION_CONFIG" "$appliance_id" "$target" "$python_path" "$(sha_file "$python_path")" <<'PY'
import json, pathlib, sys

path, appliance_id, target, python_path, python_sha256 = sys.argv[1:]
value = json.load(open(path, encoding="ascii"))
if value.get("schema") != "astrid.edge_candidate_presentation.broker_config.v1":
    raise SystemExit("presentation broker config schema drift")
if value.get("appliance_id") != appliance_id or value.get("target") != target:
    raise SystemExit("presentation broker config identity drift")
if value.get("python") != {"path": python_path, "sha256": python_sha256}:
    raise SystemExit("presentation broker Python binding drift")
if value.get("releases_root") != "/opt/astrid-edge-presentation/releases" or value.get("active_link") != "/opt/astrid-edge-presentation/current":
    raise SystemExit("presentation broker release projection drift")
if value.get("generation_binding") != "/run/astrid-edge-presentation-input/current-generation":
    raise SystemExit("presentation broker generation binding drift")
if value.get("policy", {}).get("sandbox_contract") != "unprivileged_no_network_no_home_read_only_generation_projection_only_v1":
    raise SystemExit("presentation broker sandbox policy drift")
PY
build_workers=4; [[ $appliance_id == icp* ]] && build_workers=2
candidate_memory_max_bytes=10737418240
[[ $appliance_id == icp* ]] && candidate_memory_max_bytes=5368709120
candidate_memory_swap_max_bytes=134217728
candidate_tasks_max=256
candidate_cpu_quota_percent=$((build_workers * 100))
# Keep a hard host-memory reserve for the kernel, system manager, base Astrid
# services, and the >=2 GiB live-acceptance floor. Build storage has its own
# fixed ext4 ceiling and is never backed by anonymous RAM.
supervisor_memory_max=9663676416
[[ $appliance_id == icp* ]] && supervisor_memory_max=4294967296
audio_policy=required_fresh_numeric
expected_audio_source=physical_alsa_numeric_feeder:default:16000hz:1ch
if [[ $appliance_id == icp* ]]; then
    audio_policy=required_unavailable
    expected_audio_source=unavailable_no_audio_input
fi
host_memory_kib=$(awk '/^MemTotal:/ { print $2; exit }' /proc/meminfo)
[[ $host_memory_kib =~ ^[0-9]+$ ]] || die "host memory total is unavailable"
required_memory_kib=$(( (supervisor_memory_max + 2147483648 + 1023) / 1024 ))
(( host_memory_kib >= required_memory_kib )) || die "build cgroup would violate the 2 GiB host-memory reserve"
IFS=' ' read -r state_backing_uuid runtime_state_uuid rollback_state_uuid < <(/usr/bin/python3 -I -E -s - "$STATE_STORE_CONFIG" <<'PY'
import json, sys
value = json.load(open(sys.argv[1], encoding="ascii"))
print(value["backing"]["uuid"], value["runtime"]["filesystem_uuid"], value["rollback"]["filesystem_uuid"])
PY
)
[[ $state_backing_uuid =~ ^[0-9a-f-]{36}$ && $runtime_state_uuid =~ ^[0-9a-f-]{36}$ && $rollback_state_uuid =~ ^[0-9a-f-]{36}$ ]] \
    || die "bounded state-store UUID projection is malformed"
state_store_config_sha256=$(sha_file "$STATE_STORE_CONFIG")
cat >"$RESCUE_CONFIG" <<EOF
{
  "schema":"astrid.edge_rescue_helper.config.v1",
  "appliance_id":"$appliance_id","target":"$target","model":"$model","ollama_origin":"$ollama_origin",
  "source":{"root":"$source_root","manifest":"$source_root/MANIFEST.json","signature":"$source_root/MANIFEST.signature.json","signing_key":"$SOURCE_KEY","intent_attestation_key":"$INTENT_KEY","ledger_attestation_key":"$LEDGER_ATTESTATION_KEY","vendor":"$vendor_root"},
  "roots":{"supervisor_state":"$state_root","candidate_store":"$candidate_store","model_handoff_root":"$model_handoff_root","model_handoff_ledger":"$model_handoff_ledger","candidate_work":"$candidate_work","build_store":"$build_store","releases":"$release_root","active_link":"$release_parent/current","generation_binding":"$GENERATION_FILE","maintenance_lease":"$maintenance_lease","maintenance_mutex":"$maintenance_mutex","state_snapshots":"$state_snapshots","workspace":"$runtime_workspace","system_unit_root":"$system_unit_alias","unit_policy":"$unit_policy","unit_transactions":"$unit_transactions","candidate_sandbox_root":"$CANDIDATE_SANDBOX_ROOT"},
  "identities":{"steward_uid":$steward_uid,"steward_gid":$steward_gid,"builder_uid":$builder_uid,"builder_gid":$builder_gid,"updater_uid":$updater_uid,"updater_gid":$updater_gid,"runtime_uid":$runtime_uid,"runtime_gid":$runtime_gid},
  "storage":{"config":"$STATE_STORE_CONFIG","config_sha256":"$state_store_config_sha256","install_attestation":"/run/astrid-edge-state-store/install-attestation.json","health_attestation":"/run/astrid-edge-state-store/health-attestation.json","runtime_state_mount":"$runtime_state_mount","rollback_mount":"$rollback_state_mount","backing_uuid":"$state_backing_uuid","runtime_filesystem_uuid":"$runtime_state_uuid","rollback_filesystem_uuid":"$rollback_state_uuid","image_bytes":34359738368,"host_reserve_bytes":68719476736,"store_minimum_free_bytes":4294967296,"emergency_inode_reserve_files":65536},
  "executables":{"cargo":{"path":"$cargo_path","sha256":"$(sha_file "$cargo_path")"},"rustc":{"path":"$rustc_path","sha256":"$(sha_file "$rustc_path")"},"rustfmt":{"path":"$rustfmt_path","sha256":"$(sha_file "$rustfmt_path")"},"python":{"path":"$python_path","sha256":"$(sha_file "$python_path")"},"systemctl":{"path":"$systemctl_path","sha256":"$(sha_file "$systemctl_path")"},"systemd_run":{"path":"$systemd_run_path","sha256":"$(sha_file "$systemd_run_path")"},"systemd_analyze":{"path":"$systemd_analyze_path","sha256":"$(sha_file "$systemd_analyze_path")"},"checkpoint":{"path":"$checkpoint_install_path","sha256":"$checkpoint_sha256"},"capsule_builder":{"path":"$capsule_builder_install_path","sha256":"$capsule_builder_sha256"},"invariant_runner":{"path":"$rescue_helper_install_path","sha256":"$rescue_helper_sha256"},"package_verifier":{"path":"$rescue_helper_install_path","sha256":"$rescue_helper_sha256"},"state_store":{"path":"$STATE_STORE_INSTALL","sha256":"$state_store_helper_sha256"}},
  "services":{"core":"astrid.service","warmup":"astrid-model-warmup.service","edge":"astrid-edge-runtime.service"},
  "drain":{"autonomy_state":"$autonomy_state","model_lock":"$model_lock","model_lock_gid":$model_lock_gid,"maintenance_edge_acknowledgement":"$maintenance_edge_acknowledgement","maintenance_core_acknowledgement":"$maintenance_core_acknowledgement","activity_ledgers":["$action_receipts","$web_receipts","$introspection_receipts"],"maximum_wait_seconds":900,"poll_milliseconds":500},
  "policy":{"maximum_files":25,"maximum_changed_lines":4000,"build_workers":$build_workers,"command_timeout_seconds":3600,"pipeline_timeout_seconds":86400,"maximum_candidate_bytes":16777216,"minimum_free_disk_bytes":4294967296,"candidate_memory_max_bytes":$candidate_memory_max_bytes,"candidate_memory_swap_max_bytes":$candidate_memory_swap_max_bytes,"candidate_tasks_max":$candidate_tasks_max,"candidate_cpu_quota_percent":$candidate_cpu_quota_percent,"network_policy":"private-network-none:v1","dependency_policy":"signed-vendor-offline-locked:v1"},
  "health":{"sensor_state":"$sensor_state","hindsight_state":"$hindsight_state","fill_history":"$fill_history","model_warmup_receipt":"/var/lib/astrid-edge-model-warmup/receipt.json","model_warmup_uid":$warmup_uid,"meminfo":"/proc/meminfo","swaps":"/proc/swaps","thermal_celsius":"$thermal_celsius","telemetry_addr":"127.0.0.1:7878","audio_policy":"$audio_policy","expected_audio_source":"$expected_audio_source","maximum_age_seconds":1200,"maximum_thermal_celsius":$maximum_thermal_celsius,"minimum_available_ram_bytes":2147483648,"maximum_swap_bytes":134217728,"minimum_fill_samples":10}
}
EOF
chmod 0400 "$RESCUE_CONFIG"; chown root:root "$RESCUE_CONFIG"; created_paths+=("$RESCUE_CONFIG")

# Materialize the immutable runtime profile from the attested initial
# generation before any mutable unit can start.  The helper is create-once:
# an existing divergent profile, pending transaction, link, or non-root-owned
# output fails closed rather than being replaced.
"$rescue_helper_install_path" --config "$RESCUE_CONFIG" profile-bootstrap \
    --generation-dir "$release_root/$initial_generation_id"
active_profile_env=$state_root/active-profile.env
[[ -f $active_profile_env && ! -L $active_profile_env ]] || die "profile bootstrap did not create a regular active profile"
[[ $(stat_values "$active_profile_env" | awk '{print $1" "$2" "$3}') == '0 400 1' ]] \
    || die "active profile ownership/mode/link count is invalid"

control_source=$unit_source_root/root/astrid-edge-self-evolution-control
validate_input_file "$control_source" "$(sha_file "$control_source")" "control wrapper"
install -m 0555 -o root -g root "$control_source" "$control_root/astrid-edge-self-evolution-control"; created_paths+=("$control_root/astrid-edge-self-evolution-control")
for action in status pause resume rollback rescue synthetic; do ln -s astrid-edge-self-evolution-control "$control_root/astrid-edge-self-change-$action"; created_paths+=("$control_root/astrid-edge-self-change-$action"); done

write_dropin() {
    local unit=$1 file=$stage/units/$unit.d/60-self-evolution-root.conf path
    mkdir -p "${file%/*}"
    {
        printf '[Unit]\n'; [[ -n $required_mount ]] && printf 'RequiresMountsFor=%s\nConditionPathIsMountPoint=%s\n' "$required_mount" "$required_mount"
        case "$unit" in
            astrid-edge-self-change-supervisor.service|astrid-edge-self-change-probation-health.service|astrid-edge-generation-guard.service|astrid-edge-core-liveness.service|astrid-edge-steward.service)
                printf 'Requires=%s\nAfter=%s\n' "$STATE_STORE_VERIFY_UNIT" "$STATE_STORE_VERIFY_UNIT" ;;
        esac
        case "$unit" in
            astrid-edge-self-change-supervisor.service|astrid-edge-self-change-probation-health.service|astrid-edge-generation-guard.service)
                printf 'Requires=%s\nAfter=%s\nRequiresMountsFor=%s\n' "$BUILDER_STORE_VERIFY_UNIT" "$BUILDER_STORE_VERIFY_UNIT" "$system_unit_alias" ;;
        esac
        printf '\n[Service]\nProtectHome=tmpfs\n'
        # Reset service-fragment mount policy before composing the immutable
        # root namespace.  On systemd 249 an inaccessible ancestor cannot be
        # re-exposed by a nested BindPath; empty read-only tmpfs ancestors can.
        printf 'InaccessiblePaths=\nTemporaryFileSystem=\nBindPaths=\nBindReadOnlyPaths=\nReadOnlyPaths=\nReadWritePaths=\n'
        printf 'InaccessiblePaths=/root -/boot -/usr/local -/etc/systemd/system -/etc/ssh -/etc/sudoers -/etc/sudoers.d -/etc/polkit-1 -/etc/apt -/etc/dpkg -/etc/ufw -/etc/firewalld -/etc/nftables.conf -/etc/iptables -/etc/NetworkManager -/etc/netplan -/etc/fstab -/etc/crypttab -/etc/default/grub -/etc/initramfs-tools -/etc/kernel -/etc/ssl/private\n'
        printf 'TemporaryFileSystem=/media:ro,nosuid,nodev,noexec,size=4M,mode=0755\nTemporaryFileSystem=/mnt:ro,nosuid,nodev,noexec,size=4M,mode=0755\nTemporaryFileSystem=/opt:ro,nosuid,nodev,noexec,size=4M,mode=0755\nTemporaryFileSystem=/srv:ro,nosuid,nodev,noexec,size=4M,mode=0755\nTemporaryFileSystem=/var:ro,nosuid,nodev,noexec,size=4M,mode=0755\n'
        case "$unit" in
            astrid-edge-self-change-supervisor.service|astrid-edge-self-change-probation-health.service|astrid-edge-generation-guard.service|astrid-edge-core-liveness.service)
                printf 'BindReadOnlyPaths=%s\nBindReadOnlyPaths=/run/astrid-edge-state-store\nBindReadOnlyPaths=%s\nBindReadOnlyPaths=%s\n' \
                    "$STATE_STORE_CONFIG" "$runtime_state_mount" "$rollback_state_mount" ;;
        esac
        case "$unit" in
            astrid-edge-self-change-supervisor.service|astrid-edge-self-change-probation-health.service)
                printf 'Group=%s\n' "$runtime_group"
                printf 'MemoryMax=%s\nMemorySwapMax=134217728\nTasksMax=256\nCPUQuota=%s00%%\nIOWeight=100\n' "$supervisor_memory_max" "$build_workers"
                printf 'LimitFSIZE=536870912\nTemporaryFileSystem=/tmp:rw,size=536870912,mode=1777\nTemporaryFileSystem=/var/tmp:rw,size=268435456,mode=1777\n'
                printf 'BindReadOnlyPaths=%s\nBindReadOnlyPaths=%s\nBindReadOnlyPaths=%s\n' "$source_root" "$toolchain_root" "$candidate_root"
                printf 'BindPaths=%s\nBindPaths=%s\nBindPaths=%s\nBindPaths=%s\nBindPaths=%s\nBindPaths=%s\nBindPaths=%s\n' "$state_root" "$release_parent" "$builder_root" "$updater_root" "$runtime_workspace" "$system_unit_alias" "$OPERATOR_STATUS_ROOT"
                printf 'ReadWritePaths=%s %s %s %s %s %s %s /run/astrid-edge-self-change\n' "$state_root" "$release_parent" "$builder_root" "$updater_root" "$runtime_workspace" "$system_unit_alias" "$OPERATOR_STATUS_ROOT" ;;
            astrid-edge-generation-guard.service)
                printf 'BindReadOnlyPaths=%s\nBindReadOnlyPaths=%s\nBindReadOnlyPaths=%s\nBindReadOnlyPaths=%s\nBindReadOnlyPaths=%s\n' \
                    "$source_root" "$toolchain_root" "$candidate_root" "$builder_root" "$runtime_workspace"
                printf 'BindPaths=%s\nBindPaths=%s\nBindPaths=%s\nBindPaths=%s\n' "$state_root" "$state_snapshots" "$release_parent" "$system_unit_alias"
                printf 'ReadWritePaths=%s %s %s %s\nReadOnlyPaths=%s\n' "$state_root" "$state_snapshots" "$release_parent" "$system_unit_alias" "$release_root" ;;
            astrid-edge-core-liveness.service)
                printf 'Group=%s\n' "$runtime_group"
                printf 'MemoryMax=134217728\nMemorySwapMax=0\nTasksMax=32\n'
                printf 'BindReadOnlyPaths=%s\n' "$RESCUE_CONFIG"
                printf 'BindPaths=%s\nBindPaths=%s/runtime\n' "$state_root" "$runtime_workspace"
                printf 'ReadWritePaths=%s %s/runtime\n' "$state_root" "$runtime_workspace" ;;
            astrid-edge-steward.service)
                printf 'LoadCredential=supervisor-status:%s\n' "$SUPERVISOR_STATUS"
                printf 'BindReadOnlyPaths=%s\nBindReadOnlyPaths=%s\nBindReadOnlyPaths=%s\nBindReadOnlyPaths=%s\n' "$STEWARD_CONFIG" "$RESCUE_CONFIG" "$GENERATION_FILE" "$state_root"
                printf 'BindReadOnlyPaths=%s\nReadOnlyPaths=%s\n' "$introspection_evidence_root" "$introspection_evidence_root"
                for path in "${owned_paths[@]}" "$autonomy_state" "$action_receipts" "$thermal_celsius"; do printf 'BindReadOnlyPaths=%s\n' "$path"; done
                for path in "$web_receipts" "$introspection_receipts" "${maintenance_core_acknowledgement%/*}" "${maintenance_edge_acknowledgement%/*}"; do printf 'BindReadOnlyPaths=%s\n' "$path"; done
                printf 'BindReadOnlyPaths=%s\nBindReadOnlyPaths=%s\nBindReadOnlyPaths=%s\n' "$source_root" "$release_parent" "$model_lock"
                printf 'BindPaths=%s\nBindPaths=%s\nBindPaths=%s\nBindPaths=%s\nBindPaths=%s\nBindPaths=%s\nBindPaths=%s\nBindPaths=/run/astrid-edge-self-change\n' \
                    "$maintenance_mutex" "$candidate_root" "$inbox_root" "$inquiry_history_root" "$steward_reflection_root" "$steward_projection_root" "$steward_patch_outbox"
                printf 'ReadWritePaths=%s %s %s %s %s %s %s /run/astrid-edge-self-change\n' \
                    "$maintenance_mutex" "$candidate_root" "$inbox_root" "$inquiry_history_root" "$steward_reflection_root" "$steward_projection_root" "$steward_patch_outbox" ;;
            astrid-edge-web-broker-core.service|astrid-edge-web-broker-runtime.service|astrid-edge-web-broker-steward.service)
                printf 'InaccessiblePaths=%s %s %s %s %s %s %s %s %s\n' \
                    "$state_root" "$release_parent" "$source_root" "$candidate_root" "$builder_root" "$updater_root" "$toolchain_root" "$runtime_workspace" "$model_ipc"
                for resolver in "${broker_resolvers[@]}"; do printf 'IPAddressAllow=%s\n' "$resolver"; done ;;
            astrid-edge-provider-broker@.service)
                # The gateway and steward coordinate through this exact
                # persistent root-owned inode. No mutable runtime process is a
                # member of its service-only supplementary group.
                printf 'BindReadOnlyPaths=%s\nReadOnlyPaths=%s\n' "$model_lock" "$model_lock"
                printf 'BindReadOnlyPaths=-%s\nReadOnlyPaths=-%s\n' "$maintenance_lease" "$maintenance_lease" ;;
        esac
        case "$unit" in
            astrid-edge-self-change-supervisor.service)
                # Candidate-native tests execute only beneath the disposable
                # builder filesystem. Host compiler/linker helpers are
                # root-owned and immutable to every candidate identity; all
                # other candidate/source/workspace paths remain noexec.  Hide
                # the host runtime tree from every build child and re-expose
                # only the root-only systemd manager socket plus Astrid's
                # bounded private runtime tree.  Builder UID children cannot
                # open the manager socket and cannot see D-Bus, journald, or
                # unrelated service sockets.
                printf 'TemporaryFileSystem=/run:ro,nosuid,nodev,noexec,size=16M,mode=0755\n'
                printf 'BindReadOnlyPaths=/run/systemd/private\nBindPaths=/run/astrid-edge-self-change\n'
                printf 'ExecPaths=/usr/bin /usr/sbin /bin /sbin %s %s %s %s %s %s %s\n' \
                    "$toolchain_root" "$builder_root" "$rescue_helper_install_path" \
                    "$capsule_builder_install_path" "$checkpoint_install_path" \
                    "$systemctl_path" "$systemd_analyze_path" ;;
            astrid-edge-self-change-probation-health.service)
                # Probation may invoke health and an automatic rollback, but
                # never receives the toolchain or candidate-output exception.
                printf 'ExecPaths=/usr/bin /usr/sbin /bin /sbin %s %s %s\n' \
                    "$rescue_helper_install_path" "$checkpoint_install_path" "$systemctl_path" ;;
        esac
    } >"$file"; chmod 0644 "$file"
}

mkdir -p "$stage/units"
for unit in "${install_units[@]}"; do
    source_path=$(unit_source "$unit"); destination=$stage/units/$unit; mkdir -p "${destination%/*}"
    if [[ $unit == astrid-edge-runtime.service.d/60-self-evolution-root.conf ]]; then
        private_managed_roots=("$state_root" "$release_parent" "$source_root" "$candidate_root" "$builder_root" "$builder_image" "$updater_root" "$toolchain_root" /etc/astrid-edge-self-change /usr/libexec/astrid-edge/immutable)
        sed -e "s|@@RUNTIME_WORKSPACE@@|$runtime_workspace|g" \
            -e "s|@@ACTIVE_GENERATION_ROOT@@|$release_parent/current|g" \
            -e "s|@@MODEL_IPC@@|$model_ipc|g" \
            -e "s|@@MAINTENANCE_ROOT@@|$state_root|g" \
            -e "s|@@MAINTENANCE_LEASE@@|$maintenance_lease|g" \
            -e "s|@@MAINTENANCE_EDGE_ACK@@|$maintenance_edge_acknowledgement|g" \
            -e "s|@@GENERATION_BINDING@@|$GENERATION_FILE|g" \
            -e "s|@@RUNTIME_WEB_REQUEST_KEY@@|$RUNTIME_WEB_REQUEST_KEY|g" \
            -e "s|@@RUNTIME_WEB_REQUEST_KEY_SHA256@@|$runtime_web_request_sha256|g" \
            -e "s|@@WEB_RESPONSE_VERIFY_KEY@@|$WEB_RESPONSE_VERIFY_KEY|g" \
            -e "s|@@WEB_RESPONSE_VERIFY_KEY_SHA256@@|$web_response_verify_sha256|g" \
            -e "s|@@SCHEDULED_AUTHORSHIP_ROOT@@|$scheduled_authorship_root|g" \
            -e "s|@@SCHEDULED_AUTHORSHIP_VERIFY_KEY@@|$SCHEDULED_AUTHORSHIP_VERIFY_KEY|g" \
            -e "s|@@SCHEDULED_AUTHORSHIP_VERIFY_KEY_SHA256@@|$scheduled_authorship_verify_key_sha256|g" \
            -e "s|@@SCHEDULED_AUTHORSHIP_STEWARD_UID@@|$steward_uid|g" \
            "$source_path" >"$destination.rendering"
        /usr/bin/python3 -I -E -s - "$destination.rendering" "$destination" "${private_managed_roots[@]}" <<'PY'
import pathlib, sys

source = pathlib.Path(sys.argv[1])
destination = pathlib.Path(sys.argv[2])
roots = sys.argv[3:]
body = source.read_text(encoding="utf-8")
marker = "@@PRIVATE_MANAGED_ROOT_MOUNTS@@"
if body.count(marker) != 1 or not roots:
    raise SystemExit("mutable runtime private-root marker is not exact")
mounts = "\n".join(
    f"TemporaryFileSystem={root}:ro,nosuid,nodev,noexec,size=4M,mode=0755"
    for root in roots
)
destination.write_text(body.replace(marker, mounts), encoding="utf-8")
PY
        rm -f -- "$destination.rendering"
    elif [[ $unit == astrid-edge-web-broker-core.socket || $unit == astrid-edge-web-broker-runtime.socket || $unit == astrid-edge-web-broker-steward.socket ]]; then
        socket_group=$WEB_RUNTIME_CLIENT_GROUP
        [[ $unit == astrid-edge-web-broker-core.socket ]] && socket_group=$WEB_CORE_CLIENT_GROUP
        [[ $unit == astrid-edge-web-broker-steward.socket ]] && socket_group=$STEWARD_USER
        sed -e "s|@@CORE_GROUP@@|$socket_group|g" \
            -e "s|@@SOCKET_GROUP@@|$socket_group|g" \
            -e "s|@@RUNTIME_GROUP@@|$socket_group|g" \
            -e "s|@@STEWARD_GROUP@@|$socket_group|g" \
            "$source_path" >"$destination"
    elif [[ $unit == "$PROVIDER_RUNTIME_SOCKET_UNIT" || $unit == "$PROVIDER_STEWARD_SOCKET_UNIT" || $unit == "$PROVIDER_WARMUP_SOCKET_UNIT" ]]; then
        sed -e "s|@@PROVIDER_RUNTIME_GROUP@@|$PROVIDER_RUNTIME_GROUP|g" \
            -e "s|@@PROVIDER_STEWARD_GROUP@@|$PROVIDER_STEWARD_GROUP|g" \
            -e "s|@@PROVIDER_WARMUP_GROUP@@|$PROVIDER_WARMUP_GROUP|g" \
            "$source_path" >"$destination"
    elif [[ $unit == "$PRESENTATION_SOCKET_UNIT" ]]; then
        sed -e "s|@@RUNTIME_GROUP@@|$runtime_group|g" \
            "$source_path" >"$destination"
    elif [[ $unit == "$AUDIO_SOCKET_UNIT" ]]; then
        sed -e "s|@AUDIO_CLIENT_GROUP@|$AUDIO_CLIENT_GROUP|g" \
            "$source_path" >"$destination"
    elif [[ $unit == astrid-edge-steward.service ]]; then
        sed -e "s|@@RUNTIME_GROUP@@|$runtime_group|g" \
            "$source_path" >"$destination"
    elif [[ $unit == "$PRESENTATION_SERVICE_TEMPLATE" ]]; then
        sed -e "s|@@RELEASE_PARENT@@|$release_parent|g" \
            -e "s|@@GENERATION_FILE@@|$GENERATION_FILE|g" \
            "$source_path" >"$destination"
    elif [[ $unit == "$SELF_CHANGE_INBOX_PATH_UNIT" ]]; then
        sed -e "s|@@INBOX_ROOT@@|$inbox_root|g" \
            "$source_path" >"$destination"
    elif [[ $unit == "$CORE_LIVENESS_PATH_UNIT" ]]; then
        sed -e "s|@@CORE_LIVENESS_REQUEST@@|$runtime_workspace/runtime/core-liveness-recovery.request.json|g" \
            "$source_path" >"$destination"
    else
        cp "$source_path" "$destination"
    fi
    # Rendering a templated primary fragment and composing its immutable
    # namespace are orthogonal. In particular the steward's runtime-group
    # substitution must not suppress the drop-in that grants only its bounded
    # reflection/candidate roots.
    case "$unit" in
        astrid-edge-self-change-supervisor.service|astrid-edge-self-change-probation-health.service|astrid-edge-steward.service|astrid-edge-generation-guard.service|astrid-edge-core-liveness.service|astrid-edge-web-broker-core.service|astrid-edge-web-broker-runtime.service|astrid-edge-web-broker-steward.service|astrid-edge-provider-broker@.service) write_dropin "$unit" ;;
    esac
    chmod 0644 "$destination"
done
if grep -R -E '@@[A-Z0-9_]+@@' "$stage/units" >/dev/null; then
    die "self-evolution unit rendering left an unresolved placeholder"
fi
declare -a verify_units=()
for file in "$stage"/units/*.service "$stage"/units/*.socket "$stage"/units/*.timer "$stage"/units/*.path; do [[ -e $file ]] && verify_units+=("$file"); done
if ((${#verify_units[@]} > 0)); then systemd-analyze verify "${verify_units[@]}"; fi
for unit in "${install_units[@]}"; do
    destination=$system_unit_root/$unit; install -d -m 0755 "${destination%/*}"; install -m 0644 -o root -g root "$stage/units/$unit" "$destination"; created_paths+=("$destination")
    if [[ -d $stage/units/$unit.d ]]; then
        if [[ ! -d $system_unit_root/$unit.d ]]; then install -d -m 0755 "$system_unit_root/$unit.d"; created_paths+=("$system_unit_root/$unit.d"); fi
        install -m 0644 -o root -g root "$stage/units/$unit.d/60-self-evolution-root.conf" "$system_unit_root/$unit.d/60-self-evolution-root.conf"
        created_paths+=("$system_unit_root/$unit.d/60-self-evolution-root.conf")
    fi
done

declare -a installed_verify_units=()
for unit in "${install_units[@]}"; do
    case "$unit" in
        *.service|*.socket|*.timer|*.path) installed_verify_units+=("$system_unit_root/$unit") ;;
        astrid-edge-runtime.service.d/*) ;;
    esac
done
if ((${#installed_verify_units[@]} > 0)); then systemd-analyze verify "${installed_verify_units[@]}"; fi

helper_help=$("$helper_install_path" --help 2>&1 || true)
[[ $helper_help == *'--config ABSOLUTE_JSON [--credential-directory ABS]'* ]] || die "native helper CLI contract mismatch"
[[ $helper_help != *'--due-nonce'* && $helper_help != *'--question'* ]] || die "native helper retained an obsolete operator-selected prompt surface"
rescue_help=$("$rescue_helper_install_path" --help 2>&1 || true)
[[ $rescue_help == *'reconcile-storage-reserve'* && $rescue_help == *'verify-install'* && $rescue_help == *'--intent-envelope ABSOLUTE'* && $rescue_help == *'--model-handoff ABSOLUTE'* && $rescue_help == *'rollback --generation-dir ABSOLUTE'* && $rescue_help == *'profile-bootstrap --generation-dir ABSOLUTE'* && $rescue_help == *'health | retention | synthetic-lifecycle'* && $rescue_help == *'recover-model-after-build'* && $rescue_help == *'recover-core-liveness'* ]] || die "native rescue-helper CLI contract mismatch"
provider_help=$("$provider_broker_install_path" --help 2>&1 || true)
[[ $provider_help == *'serve --config ABS --client CLIENT --credential-directory ABS'* && $provider_help == *'warmup --config ABS --key ABS --receipt ABS'* ]] || die "immutable provider-broker CLI contract mismatch"
presentation_help=$("$presentation_broker_install_path" --help 2>&1 || true)
[[ $presentation_help == *'usage: astrid-edge-presentation-broker serve|client'* ]] || die "immutable presentation-broker CLI contract mismatch"
systemctl daemon-reload
for unit in ${enable_units[@]+"${enable_units[@]}"}; do systemctl enable "$unit"; enabled_now+=("$unit"); done
startup_sockets=("$WEB_CORE_SOCKET_UNIT" "$WEB_RUNTIME_SOCKET_UNIT" "$WEB_STEWARD_SOCKET_UNIT" "$PROVIDER_RUNTIME_SOCKET_UNIT" "$PROVIDER_STEWARD_SOCKET_UNIT" "$PROVIDER_WARMUP_SOCKET_UNIT" "$PRESENTATION_SOCKET_UNIT")
[[ $appliance_id == icp* ]] || startup_sockets+=("$AUDIO_SOCKET_UNIT")
for unit in "${startup_sockets[@]}"; do
    started_now+=("$unit"); systemctl start "$unit"
    systemctl is-active --quiet "$unit" || die "immutable broker socket did not become active: $unit"
done
for unit in "$WEB_CORE_SERVICE_UNIT" "$WEB_RUNTIME_SERVICE_UNIT" "$WEB_STEWARD_SERVICE_UNIT"; do
    started_now+=("$unit"); systemctl start "$unit"
    systemctl is-active --quiet "$unit" || die "immutable web-broker process did not become active: $unit"
    [[ $(systemctl show "$unit" --property=NRestarts --value) == 0 ]] || die "immutable web-broker process restarted during bootstrap: $unit"
done

migration_profile=avado; [[ $appliance_id == icp* ]] && migration_profile=icp
migration_args=(--profile "$migration_profile" --appliance-id "$appliance_id" --runtime-user "$runtime_user" --runtime-home "$runtime_home" --unit-source-root "$unit_source_root" --user-unit-root "$user_unit_root" --system-unit-root "$system_unit_root" --rescue-system-unit-root "$system_unit_alias" \
    --active-generation-root "$release_parent/current" --management-marker "$MANAGER_MARKER" \
    --source-root "$source_root" --candidate-root "$candidate_root" --builder-root "$builder_root" --updater-root "$updater_root" --toolchain-root "$toolchain_root" \
    --model-lock "$model_lock" --maintenance-lease "$maintenance_lease" --authority-env "$AUTHORITY_ENV" \
    --self-evolution-dropin-sha256 "$(sha_file "$system_unit_root/astrid-edge-runtime.service.d/60-self-evolution-root.conf")" --unit-policy "$unit_policy" --provider-output-tokens "$output_tokens" \
    --post-install-verifier "$rescue_helper_install_path" --post-install-verifier-config "$RESCUE_CONFIG" \
    --state-store-helper "$STATE_STORE_INSTALL" --state-store-helper-sha256 "$state_store_helper_sha256" \
    --state-store-config "$STATE_STORE_CONFIG" \
    --state-store-runtime-mount-unit "$runtime_state_mount_unit" \
    --state-store-rollback-mount-unit "$rollback_state_mount_unit" \
    --state-store-verify-unit "$STATE_STORE_VERIFY_UNIT" --state-store-health-timer "$STATE_STORE_HEALTH_TIMER")
migration_args+=(--ollama-binary "$ollama_binary" --ollama-binary-sha256 "$ollama_binary_sha256")
migration_args+=(--operator-report-manifest-sha256 "$operator_report_manifest_sha256")
for ((index=0; index<${#required_system_stack[@]}; index++)); do migration_args+=(--unit "${required_system_stack[$index]}" --unit-sha256 "${required_system_stack[$index]}=${system_stack_hashes[$index]}"); done
if [[ $migration_profile == avado ]]; then
    migration_args+=(--profile-dropin-sha256 "astrid-local-ollama.conf=$(sha_file "$unit_source_root/astrid-local-ollama.conf")")
else
    migration_args+=(--profile-dropin-sha256 "icp-ssd-required.conf=$(sha_file "$unit_source_root/icp-ssd-required.conf")")
    migration_args+=(--profile-dropin-sha256 "astrid-edge-tuning-authority.conf=$(sha_file "$unit_source_root/astrid-edge-tuning-authority.conf")")
fi
[[ -n $required_mount ]] && migration_args+=(--required-mount "$required_mount" --required-mount-uuid "$required_mount_uuid")
$start_system_services && migration_args+=(--start-services)
migration_completion_marker=$stage/system-migration.complete
migration_args+=(--completion-marker "$migration_completion_marker")
"$migrator" "${migration_args[@]}"
[[ -f $migration_completion_marker && ! -L $migration_completion_marker ]] || die "system-service migration did not commit its completion marker"

atomic_authority_install() {
    local source=$1 pending=$AUTHORITY_ENV.pending.$$
    rm -f -- "$pending" || return 1
    install -m 0444 -o root -g root "$source" "$pending" || { rm -f -- "$pending"; return 1; }
    /usr/bin/python3 -I -E -s - "$pending" <<'PY'
import os, sys
descriptor = os.open(sys.argv[1], os.O_RDONLY | getattr(os, "O_CLOEXEC", 0))
try:
    os.fsync(descriptor)
finally:
    os.close(descriptor)
PY
    [[ $? == 0 ]] || { rm -f -- "$pending"; return 1; }
    mv -T -- "$pending" "$AUTHORITY_ENV" || { rm -f -- "$pending"; return 1; }
    /usr/bin/python3 -I -E -s - "${AUTHORITY_ENV%/*}" <<'PY'
import os, sys
descriptor = os.open(sys.argv[1], os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
try:
    os.fsync(descriptor)
finally:
    os.close(descriptor)
PY
    [[ $? == 0 ]] || return 1
}

# Authority remains persistently disabled through source capture, A/B setup,
# and root-manager cutover. A /run-only drop-in proves a fresh enabled runtime
# before the enabled bit is committed to disk. Thus a power loss before proof
# reboots disabled; after proof it reboots into the already-accepted state.
# Any later failure restores disabled state and leaves the migrated base stack
# available for operator repair.
authority_healthy=true
authority_activation_in_progress=true
[[ ! -e $authority_bootstrap_dropin && ! -L $authority_bootstrap_dropin ]] || authority_healthy=false
if $authority_healthy; then
    install -d -m 0755 -o root -g root "${authority_bootstrap_dropin%/*}" || authority_healthy=false
fi
if $authority_healthy; then
    bootstrap_authority_source=$stage/99-self-change-bootstrap-authority.conf
    printf '[Service]\nEnvironment=ASTRID_EDGE_SELF_CHANGE_ENABLED=true\n' >"$bootstrap_authority_source"
    install -m 0644 -o root -g root "$bootstrap_authority_source" "$authority_bootstrap_dropin" || authority_healthy=false
fi
if $authority_healthy; then systemctl daemon-reload || authority_healthy=false; fi
if $authority_healthy; then systemctl restart astrid-edge-runtime.service || authority_healthy=false; fi
systemctl is-active --quiet astrid-edge-runtime.service || authority_healthy=false
[[ $(systemctl show astrid-edge-runtime.service --property=NRestarts --value) == 0 ]] || authority_healthy=false
edge_pid=$(systemctl show astrid-edge-runtime.service --property=MainPID --value) || { edge_pid=; authority_healthy=false; }
[[ $edge_pid =~ ^[1-9][0-9]*$ ]] || authority_healthy=false
if $authority_healthy; then
    edge_environment=$stage/edge-runtime.environment
    tr '\0' '\n' <"/proc/$edge_pid/environ" >"$edge_environment" || authority_healthy=false
    for expected_environment in \
        'ASTRID_EDGE_SELF_CHANGE_ENABLED=true' \
        "ASTRID_EDGE_MAINTENANCE_LEASE_PATH=$maintenance_lease" \
        'ASTRID_EDGE_REFLECTION_LEASE_PATH=/run/astrid-edge-self-change/reflection.json' \
        "ASTRID_EDGE_MAINTENANCE_EDGE_ACK_PATH=$maintenance_edge_acknowledgement" \
        "ASTRID_EDGE_GENERATION_BINDING_PATH=$GENERATION_FILE" \
        "ASTRID_EDGE_APPLIANCE_ID=$appliance_id" \
        "ASTRID_EDGE_SOCKET=$astrid_state_root/run/system.sock" \
        "ASTRID_EDGE_TOKEN=$astrid_state_root/run/system.token" \
        "ASTRID_EDGE_WORKSPACE=$runtime_workspace" \
        "ASTRID_EDGE_ASTRID_CLI=$release_parent/current/astrid" \
        "ASTRID_EDGE_SELF_CHANGE_ROOT=$runtime_workspace/self-change" \
        "ASTRID_EDGE_CORE_LIVENESS_REQUEST_PATH=$runtime_workspace/runtime/core-liveness-recovery.request.json" \
        'ASTRID_EDGE_SCHEDULED_INTROSPECTION_ENABLED=false' \
        'ASTRID_EDGE_DEDICATED_STEWARD_ENABLED=true' \
        'ASTRID_EDGE_DEDICATED_STEWARD_INTERVAL_MINUTES=120' \
        "ASTRID_EDGE_WEB_BROKER_SOCKET_PATH=$WEB_RUNTIME_SOCKET"; do
        grep -Fxq "$expected_environment" "$edge_environment" || authority_healthy=false
    done
fi
if $authority_healthy; then
    "$rescue_helper_install_path" --config "$RESCUE_CONFIG" health >/dev/null || authority_healthy=false
fi
if $authority_healthy; then
    atomic_authority_install "$authority_enabled_source" || authority_healthy=false
fi
if $authority_healthy; then
    rm -f -- "$authority_bootstrap_dropin" || authority_healthy=false
    systemctl daemon-reload || authority_healthy=false
fi
if $authority_healthy; then
    systemctl start astrid-edge-steward.timer || authority_healthy=false
    systemctl is-active --quiet astrid-edge-steward.timer || authority_healthy=false
    systemctl start astrid-edge-self-change-probation-health.timer || authority_healthy=false
    systemctl is-active --quiet astrid-edge-self-change-probation-health.timer || authority_healthy=false
    started_now+=("$CORE_LIVENESS_PATH_UNIT")
    systemctl start "$CORE_LIVENESS_PATH_UNIT" || authority_healthy=false
    systemctl is-active --quiet "$CORE_LIVENESS_PATH_UNIT" || authority_healthy=false
    started_now+=("$SELF_CHANGE_INBOX_PATH_UNIT")
    systemctl start "$SELF_CHANGE_INBOX_PATH_UNIT" || authority_healthy=false
    systemctl is-active --quiet "$SELF_CHANGE_INBOX_PATH_UNIT" || authority_healthy=false
fi
if ! $authority_healthy; then
    systemctl stop "$CORE_LIVENESS_PATH_UNIT" >/dev/null 2>&1 || true
    systemctl stop "$SELF_CHANGE_INBOX_PATH_UNIT" >/dev/null 2>&1 || true
    systemctl stop astrid-edge-steward.timer >/dev/null 2>&1 || true
    systemctl stop astrid-edge-self-change-probation-health.timer >/dev/null 2>&1 || true
    systemctl stop astrid-edge-runtime.service >/dev/null 2>&1 || true
    rm -f -- "$authority_bootstrap_dropin" || true
    systemctl daemon-reload >/dev/null 2>&1 || true
    if atomic_authority_install "$authority_disabled_source"; then
        systemctl start astrid-edge-runtime.service >/dev/null 2>&1 || true
    fi
    authority_activation_in_progress=false
    die "self-change authority activation failed closed; root-managed base stack remains installed with authority disabled"
fi
authority_activation_in_progress=false
committed=true
printf 'Installed the full CPU-edge root boundary for %s with an immutable web broker, generation guard, five bounded rescue profiles, and crash-safe system-manager ownership.\n' "$appliance_id"
printf 'No candidate or Action was selected by this bootstrap; the native steward remains voluntary and the authenticated supervisor owns A/B activation and rollback.\n'
printf 'Self-change deployment remains paused for bootstrap acceptance; scheduled reflection stays enabled and only an explicit sudo resume can release a queued genuine candidate.\n'
