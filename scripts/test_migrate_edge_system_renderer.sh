#!/usr/bin/env bash
# Exercise the production user->system renderer without root or host mutation.
set -euo pipefail
IFS=$'\n\t'
umask 077

REPO_ROOT=$(cd "$(dirname "$0")/.." && pwd -P)
MIGRATOR=$REPO_ROOT/packaging/systemd/root/migrate-edge-user-services-to-system
SUPPORT_UNIT_ROOT=

fail() { printf 'renderer-test: %s\n' "$*" >&2; exit 1; }
need_value() { [[ $# -ge 2 && -n $2 ]] || fail "missing value for $1"; }
hash_file() { sha256sum -- "$1" | awk '{print $1}'; }

while (($#)); do
    case "$1" in
        --support-unit-root) need_value "$@"; SUPPORT_UNIT_ROOT=$2; shift 2 ;;
        -h|--help)
            printf 'usage: %s [--support-unit-root ABS]\n' "$0"
            exit 0 ;;
        *) fail "unsupported argument: $1" ;;
    esac
done

[[ -x $MIGRATOR ]] || fail "migrator is absent or not executable"
RUNTIME_USER=$(id -un)
RUNTIME_UID=$(id -u)
[[ $RUNTIME_USER =~ ^[a-z_][a-z0-9_-]{0,30}$ ]] || fail "test account name is incompatible with the appliance contract"

TEMP=$(mktemp -d "${TMPDIR:-/tmp}/astrid-edge-renderer.XXXXXX")
TEMP=$(cd "$TEMP" && pwd -P)
trap 'rm -rf -- "$TEMP"' EXIT HUP INT TERM

readonly -a STACK=(
    ollama-cpu.service
    astrid-model-warmup.service
    astrid.service
    astrid-edge-runtime.service
    astrid-edge-hindsight.service
    astrid-edge-hindsight.timer
)
readonly ACTIVE_GENERATION_ROOT=/opt/astrid-edge/releases/current
readonly MANAGEMENT_MARKER=/etc/astrid/edge-service-manager.json
readonly MODEL_LOCK=/var/lib/astrid-edge-self-change/model.lock
readonly MAINTENANCE_LEASE=/var/lib/astrid-edge-self-change/maintenance.json
readonly AUTHORITY_ENV=/etc/astrid/edge-self-change-authority.env
readonly UNIT_POLICY=/var/lib/astrid-edge-self-change/unit-policy.json
readonly STATE_DEVICE=/dev/disk/by-uuid/11111111-2222-3333-4444-555555555555
readonly WEB_RESPONSE_SHA=ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff
readonly AUDIO_UID=4242

declare -a RENDERED_ROOTS=()

render_profile() {
    local profile=$1
    local root=$TEMP/$profile home=$TEMP/$profile/home
    local user_units=$home/.config/systemd/user
    local source_profile=$REPO_ROOT/packaging/systemd
    local state_root core_workspace model_root capsule_root ollama_runtime ollama_binary
    local output_tokens appliance_id
    local -a args=() profile_hash_args=()

    mkdir -p "$user_units" "$home/.config/astrid"
    if [[ $profile == avado ]]; then
        appliance_id=avado-render-fixture
        output_tokens=192
        state_root=$home/.astrid
        core_workspace=$home
        model_root=$home/.local/share/ollama/models
        capsule_root=$state_root/home/default/.local/capsules
        ollama_runtime=$home/.local/ollama-v0.0.0
        ollama_binary=$ollama_runtime/bin/ollama
        mkdir -p "$capsule_root" "$model_root" "$ollama_runtime/bin" "$home/.local/bin"
        ln -s "$ollama_binary" "$home/.local/bin/ollama"
        args+=(--render-audio-feeder-uid "$AUDIO_UID")
        profile_hash_args=(
            --profile-dropin-sha256
            "astrid-local-ollama.conf=$(hash_file "$REPO_ROOT/packaging/systemd/astrid-local-ollama.conf")"
        )
    else
        appliance_id=icp-render-fixture
        output_tokens=112
        local data_root=$TEMP/$profile/data/astrid
        mkdir -p "$data_root/state/home/default/.local/capsules" \
            "$data_root/state/home/default/edge" \
            "$data_root/state/tmp" \
            "$data_root/workspace" \
            "$data_root/ollama/runtime/bin" \
            "$data_root/ollama/models"
        ln -s "$data_root" "$home/.astrid-icp"
        state_root=$data_root/state
        core_workspace=$data_root/workspace
        model_root=$data_root/ollama/models
        capsule_root=$state_root/home/default/.local/capsules
        ollama_runtime=$data_root/ollama/runtime
        ollama_binary=$ollama_runtime/bin/ollama
        source_profile=$REPO_ROOT/packaging/systemd/icp
        args+=(--required-mount /media/data --required-mount-uuid 0123-ABCD)
        profile_hash_args=(
            --profile-dropin-sha256
            "icp-ssd-required.conf=$(hash_file "$REPO_ROOT/packaging/systemd/icp-ssd-required.conf")"
            --profile-dropin-sha256
            "astrid-edge-tuning-authority.conf=$(hash_file "$REPO_ROOT/packaging/systemd/astrid-edge-tuning-authority.conf")"
        )
    fi

    printf '\177ELFsynthetic-render-fixture\n' >"$ollama_binary"
    chmod 0755 "$ollama_binary"
    for unit in "${STACK[@]}"; do
        cp "$source_profile/$unit" "$user_units/$unit"
        args+=(--unit "$unit" --unit-sha256 "$unit=$(hash_file "$source_profile/$unit")")
    done

    local output_root=$TEMP/$profile-output
    mkdir -m 0700 "$output_root"
    local before_ollama before_units after_units
    before_ollama=$(hash_file "$ollama_binary")
    before_units=$(find "$user_units" -maxdepth 1 -type f -print0 | LC_ALL=C sort -z | xargs -0 sha256sum | sha256sum | awk '{print $1}')

    "$MIGRATOR" \
        --render-only \
        --output-root "$output_root" \
        --render-state-device "$STATE_DEVICE" \
        --render-web-response-verify-sha256 "$WEB_RESPONSE_SHA" \
        --profile "$profile" \
        --appliance-id "$appliance_id" \
        --runtime-user "$RUNTIME_USER" \
        --runtime-home "$home" \
        --unit-source-root "$REPO_ROOT/packaging/systemd" \
        --user-unit-root "$user_units" \
        --system-unit-root /etc/systemd/system \
        --rescue-system-unit-root /var/lib/astrid-edge-updater/system-units \
        --active-generation-root "$ACTIVE_GENERATION_ROOT" \
        --management-marker "$MANAGEMENT_MARKER" \
        --self-evolution-dropin-sha256 "$(hash_file "$REPO_ROOT/packaging/systemd/root/astrid-edge-runtime-self-evolution.conf.in")" \
        --post-install-verifier /usr/libexec/astrid-edge/immutable/astrid-edge-rescue-helper \
        --post-install-verifier-config /etc/astrid/edge-rescue-helper.json \
        --model-lock "$MODEL_LOCK" \
        --maintenance-lease "$MAINTENANCE_LEASE" \
        --authority-env "$AUTHORITY_ENV" \
        --unit-policy "$UNIT_POLICY" \
        --ollama-binary "$ollama_binary" \
        --ollama-binary-sha256 "$before_ollama" \
        --operator-report-manifest-sha256 aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa \
        --provider-output-tokens "$output_tokens" \
        --state-store-helper /usr/libexec/astrid/astrid-edge-state-store \
        --state-store-helper-sha256 bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb \
        --state-store-config /etc/astrid/edge-state-store.json \
        --state-store-runtime-mount-unit var-lib-astrid-edge-runtime-state.mount \
        --state-store-rollback-mount-unit var-lib-astrid-edge-rollback-state.mount \
        --state-store-verify-unit astrid-edge-state-store-verify.service \
        --state-store-health-timer astrid-edge-state-store-health.timer \
        --source-root /var/lib/astrid-edge-source \
        --candidate-root /var/lib/astrid-edge-candidates \
        --builder-root /var/lib/astrid-edge-builder \
        --updater-root /var/lib/astrid-edge-updater \
        --toolchain-root /opt/astrid-edge-toolchain \
        "${profile_hash_args[@]}" \
        "${args[@]}" \
        >"$TEMP/$profile-render.out"

    grep -Fq "RENDER-ONLY: production system-manager stack rendered at $output_root/units" "$TEMP/$profile-render.out" \
        || fail "$profile renderer did not report its bounded output"
    [[ $(hash_file "$ollama_binary") == "$before_ollama" ]] || fail "$profile renderer modified the model binary fixture"
    after_units=$(find "$user_units" -maxdepth 1 -type f -print0 | LC_ALL=C sort -z | xargs -0 sha256sum | sha256sum | awk '{print $1}')
    [[ $after_units == "$before_units" ]] || fail "$profile renderer modified deployed user-unit fixtures"
    for unit in "${STACK[@]}"; do
        [[ -f $output_root/units/$unit ]] || fail "$profile render omitted $unit"
    done
    if grep -r -n -E -- '%[ht]|@@[A-Z0-9_]+@@|@AUDIO_CLIENT_GROUP@' "$output_root/units"; then
        fail "$profile render retained a user-manager specifier or template placeholder"
    fi

    python3 - "$profile" "$output_root/units" "$home" "$state_root" "$core_workspace" \
        "$ACTIVE_GENERATION_ROOT" "$ollama_binary" "$ollama_runtime" "$model_root" \
        "$RUNTIME_UID" "$AUDIO_UID" "$STATE_DEVICE" "$WEB_RESPONSE_SHA" <<'PY'
import pathlib
import shlex
import sys

(
    profile,
    raw_root,
    home,
    state_root,
    core_workspace,
    active_generation,
    ollama_binary,
    ollama_runtime,
    model_root,
    runtime_uid,
    audio_uid,
    state_device,
    web_response_sha,
) = sys.argv[1:]
root = pathlib.Path(raw_root)


def records(unit: str):
    paths = [root / unit]
    dropins = root / f"{unit}.d"
    if dropins.is_dir():
        paths.extend(sorted(dropins.glob("*.conf")))
    result = {}
    section = ""
    for path in paths:
        for raw in path.read_text(encoding="utf-8").splitlines():
            line = raw.strip()
            if not line or line.startswith(("#", ";")):
                continue
            if line.startswith("[") and line.endswith("]"):
                section = line[1:-1]
                continue
            if "=" not in line:
                continue
            key, value = line.split("=", 1)
            result.setdefault((section, key), []).append(value)
    return result


def effective_list(data, section, key):
    current = []
    for value in data.get((section, key), []):
        if value == "":
            current = []
        else:
            current.append(value)
    return current


def words(values):
    return [word for value in values for word in shlex.split(value)]


def scalar(data, section, key):
    values = data.get((section, key), [])
    return values[-1] if values else None


def environment(data):
    result = {}
    for value in effective_list(data, "Service", "Environment"):
        for assignment in shlex.split(value):
            name, content = assignment.split("=", 1)
            result[name] = content
    return result


units = {
    name: records(name)
    for name in (
        "ollama-cpu.service",
        "astrid-model-warmup.service",
        "astrid.service",
        "astrid-edge-runtime.service",
        "astrid-edge-hindsight.service",
        "astrid-edge-hindsight.timer",
    )
}
active_profile = "/var/lib/astrid-edge-self-change/active-profile.env"

ollama = units["ollama-cpu.service"]
assert effective_list(ollama, "Service", "ExecStart") == [f"{ollama_binary} serve"]
assert effective_list(ollama, "Service", "ExecStartPre") == [
    "/usr/bin/sha256sum --check --strict --status /etc/astrid/edge-ollama-runtime.sha256"
]
assert scalar(ollama, "Service", "ProtectProc") == "invisible"
assert scalar(ollama, "Service", "ProcSubset") == "all"
assert scalar(ollama, "Service", "PrivateNetwork") == "yes"
assert effective_list(ollama, "Service", "SystemCallFilter") == [
    "~pidfd_getfd kcmp process_vm_readv process_vm_writev ptrace"
]
assert ollama_runtime in words(effective_list(ollama, "Service", "BindReadOnlyPaths"))
assert model_root in words(effective_list(ollama, "Service", "BindPaths"))
assert model_root in words(effective_list(ollama, "Service", "ReadWritePaths"))

warmup = units["astrid-model-warmup.service"]
assert effective_list(warmup, "Unit", "ConditionPathExists") == []
assert "ollama-cpu.service" in words(effective_list(warmup, "Unit", "After"))
assert "astrid-edge-provider-warmup.socket" in words(effective_list(warmup, "Unit", "Requires"))
assert effective_list(warmup, "Service", "ExecStart") == [
    "/usr/libexec/astrid-edge/immutable/astrid-edge-provider-broker warmup --config /etc/astrid-edge-self-change/provider-broker.json --key /run/credentials/astrid-model-warmup.service/provider-request.key --receipt /var/lib/astrid-edge-model-warmup/receipt.json"
]
assert scalar(warmup, "Service", "StateDirectory") == "astrid-edge-model-warmup"
assert "provider-request.key:/etc/astrid-edge-self-change/model-warmup-provider-request.key" in effective_list(
    warmup, "Service", "LoadCredential"
)

core = units["astrid.service"]
edge = units["astrid-edge-runtime.service"]
workspace = f"{state_root}/home/default/edge"
assert effective_list(core, "Service", "ExecStart") == [f"{active_generation}/astrid-daemon --workspace {core_workspace}"]
assert effective_list(edge, "Service", "ExecStart") == [f"{active_generation}/astrid-edge-runtime"]
assert scalar(core, "Service", "WorkingDirectory") == workspace
assert scalar(edge, "Service", "WorkingDirectory") == workspace
assert effective_list(core, "Service", "EnvironmentFile") == [active_profile]
edge_environment_files = [active_profile, "/etc/astrid/edge-self-change-authority.env"]
if profile == "icp":
    edge_environment_files.extend(
        [
            f"{home}/.config/astrid/edge-tuning-authority.env",
            f"{home}/.config/astrid/edge-spectral-deferred.env",
        ]
    )
assert effective_list(edge, "Service", "EnvironmentFile") == edge_environment_files
assert effective_list(core, "Service", "NoExecPaths") == ["/"]
assert effective_list(edge, "Service", "NoExecPaths") == ["/"]
assert effective_list(core, "Service", "ExecPaths") == [
    f"{active_generation}/astrid-daemon /usr/lib /usr/lib64 /lib /lib64"
]
assert effective_list(edge, "Service", "ExecPaths") == [
    f"{active_generation}/astrid-edge-runtime {active_generation}/astrid /usr/lib /usr/lib64 /lib /lib64"
]
assert effective_list(edge, "Unit", "BindsTo") == []
assert "astrid.service" in words(effective_list(edge, "Unit", "Wants"))
assert "astrid-edge-provider-runtime.socket" in words(effective_list(core, "Unit", "Requires"))
assert "astrid-edge-web-broker-core.socket" in words(effective_list(core, "Unit", "Requires"))
assert "astrid-edge-web-broker-runtime.socket" in words(effective_list(edge, "Unit", "Requires"))
assert "astrid-edge-generation-guard.service" in words(effective_list(core, "Unit", "After"))
assert "astrid-edge-generation-guard.service" in words(effective_list(edge, "Unit", "After"))

core_env = environment(core)
edge_env = environment(edge)
assert core_env["ASTRID_LOCAL_PROVIDER_UNIX_SOCKET"] == "/run/astrid-edge-self-change/provider-runtime.sock"
assert core_env["ASTRID_EDGE_CORE_WEB_BROKER_SOCKET"] == "/run/astrid-edge-self-change/web-core.sock"
assert core_env["ASTRID_EDGE_CORE_WEB_BROKER_RESPONSE_KEY_SHA256"] == web_response_sha
assert edge_env["ASTRID_EDGE_SOCKET"] == f"{state_root}/run/system.sock"
assert edge_env["ASTRID_EDGE_TOKEN"] == f"{state_root}/run/system.token"
assert edge_env["ASTRID_EDGE_WORKSPACE"] == workspace
assert edge_env["ASTRID_EDGE_ASTRID_CLI"] == f"{active_generation}/astrid"
assert edge_env["ASTRID_EDGE_SELF_CHANGE_ROOT"] == f"{workspace}/self-change"
assert edge_env["ASTRID_EDGE_SCHEDULED_INTROSPECTION_ENABLED"] == "false"
assert edge_env["ASTRID_EDGE_DEDICATED_STEWARD_ENABLED"] == "true"
assert edge_env["ASTRID_EDGE_DEDICATED_STEWARD_INTERVAL_MINUTES"] == "120"

for name, data in (("core", core), ("edge", edge)):
    exposed = words(effective_list(data, "Service", "BindPaths"))
    exposed += words(effective_list(data, "Service", "BindReadOnlyPaths"))
    for forbidden in (
        "/var/lib/astrid-edge-source",
        "/var/lib/astrid-edge-candidates",
        "/var/lib/astrid-edge-builder",
        "/var/lib/astrid-edge-updater",
        "/opt/astrid-edge-toolchain",
    ):
        assert forbidden not in exposed, (name, forbidden, exposed)
    groups = words(effective_list(data, "Service", "SupplementaryGroups"))
    assert "astrid-edge-model-lock" not in groups
    assert "astrid-edge-steward" not in groups
    temporary = words(effective_list(data, "Service", "TemporaryFileSystem"))
    assert any(item.startswith("/var/lib/astrid-edge-source:") for item in temporary)
    assert any(item.startswith("/var/lib/astrid-edge-candidates:") for item in temporary)

services = (
    "ollama-cpu.service",
    "astrid-model-warmup.service",
    "astrid.service",
    "astrid-edge-runtime.service",
    "astrid-edge-hindsight.service",
)
if profile == "avado":
    assert f"ASTRID_EDGE_AUDIO_FEEDER_UID={audio_uid}" in effective_list(edge, "Service", "Environment")
    assert "astrid-edge-audio-client" in words(effective_list(edge, "Service", "SupplementaryGroups"))
    for unit in services:
        assert effective_list(units[unit], "Service", "ExecCondition") == []
else:
    assert not any("AUDIO_FEEDER_UID=" in value for value in effective_list(edge, "Service", "Environment"))
    assert "astrid-edge-audio-client" not in words(effective_list(edge, "Service", "SupplementaryGroups"))
    for unit in services:
        data = units[unit]
        assert "/media/data" in words(effective_list(data, "Unit", "RequiresMountsFor"))
        assert "/media/data" in words(effective_list(data, "Unit", "ConditionPathIsMountPoint"))
        assert effective_list(data, "Service", "ExecCondition") == [
            "/usr/libexec/astrid-edge/immutable/wait-for-icp-ssd --systemd-condition --wait-seconds 75 --poll-seconds 2"
        ]

for data in units.values():
    for values in data.values():
        assert all("%h" not in value and "%t" not in value for value in values)

hindsight_timer = units["astrid-edge-hindsight.timer"]
assert "astrid-edge-state-store-verify.service" in words(
    effective_list(hindsight_timer, "Unit", "Requires")
)
assert state_device in "\n".join(
    value for data in units.values() for values in data.values() for value in values
)
PY

    RENDERED_ROOTS+=("$output_root/units")
}

render_profile avado
render_profile icp

if command -v systemd-analyze >/dev/null 2>&1; then
    [[ $SUPPORT_UNIT_ROOT == /* && -d $SUPPORT_UNIT_ROOT && ! -L $SUPPORT_UNIT_ROOT ]] \
        || fail "systemd-analyze is available; --support-unit-root must name the rendered immutable support-unit directory"
    verify_root=$TEMP/systemd-verify
    verify_exec_root=$verify_root/executables
    verify_support_root=$verify_root/support
    mkdir -m 0700 -p "$verify_exec_root" "$verify_support_root"
    cp -R "$SUPPORT_UNIT_ROOT/." "$verify_support_root/"

    declare -a VERIFY_RENDERED_ROOTS=()
    for rendered in "${RENDERED_ROOTS[@]}"; do
        verify_rendered=$verify_root/$(basename "$(dirname "$rendered")")
        mkdir -m 0700 "$verify_rendered"
        cp -R "$rendered/." "$verify_rendered/"
        VERIFY_RENDERED_ROOTS+=("$verify_rendered")
    done

    # systemd 249 insists that direct Exec*= paths exist during static
    # verification. Discover only commands under the three reviewed appliance
    # prefixes, reject any other absent absolute executable, and remap those
    # commands into this test's private root. Unit text outside the copies is
    # never changed and the runner's /opt and /usr/libexec stay untouched.
    mapfile -t fixture_commands < <(python3 - \
        "$verify_support_root" "${VERIFY_RENDERED_ROOTS[@]}" <<'PY'
import os
import pathlib
import shlex
import sys

approved = (
    "/opt/astrid-edge/releases/current/",
    "/usr/libexec/astrid/",
    "/usr/libexec/astrid-edge/immutable/",
)
expected = {
    "/opt/astrid-edge/releases/current/astrid-daemon",
    "/opt/astrid-edge/releases/current/astrid-edge-runtime",
    "/usr/libexec/astrid-edge/immutable/astrid-edge-presentation-broker",
    "/usr/libexec/astrid-edge/immutable/astrid-edge-provider-broker",
    "/usr/libexec/astrid-edge/immutable/astrid-edge-web-broker",
    "/usr/libexec/astrid-edge/immutable/wait-for-icp-ssd",
    "/usr/libexec/astrid/astrid-edge-rescue-helper",
    "/usr/libexec/astrid/astrid-edge-steward-helper",
}
commands = set()
for raw_root in sys.argv[1:]:
    root = pathlib.Path(raw_root)
    for path in sorted(item for item in root.rglob("*") if item.is_file()):
        for raw_line in path.read_text(encoding="utf-8").splitlines():
            line = raw_line.strip()
            if not line.startswith((
                "ExecCondition=",
                "ExecStart=",
                "ExecStartPre=",
                "ExecStartPost=",
                "ExecReload=",
                "ExecStop=",
                "ExecStopPost=",
            )):
                continue
            value = line.split("=", 1)[1]
            while value and value[0] in "-+!:@":
                value = value[1:]
            words = shlex.split(value)
            if not words:
                continue
            executable = words[0]
            if executable.startswith(approved):
                commands.add(executable)
            elif executable.startswith("/") and not os.access(executable, os.X_OK):
                raise SystemExit(
                    f"unapproved missing absolute executable in rendered unit: {executable}"
                )
if commands != expected:
    raise SystemExit(
        "rendered unit command allowlist changed:\n"
        f"expected={sorted(expected)!r}\nactual={sorted(commands)!r}"
    )
for command in sorted(commands):
    print(command)
PY
    )
    ((${#fixture_commands[@]} > 0)) || fail "rendered unit executable fixture set is empty"
    for command in "${fixture_commands[@]}"; do
        install -D -m 0755 /usr/bin/true "$verify_exec_root$command"
    done
    for unit_root in "$verify_support_root" "${VERIFY_RENDERED_ROOTS[@]}"; do
        while IFS= read -r -d '' unit; do
            sed -i \
                -e "s|$ACTIVE_GENERATION_ROOT/|$verify_exec_root$ACTIVE_GENERATION_ROOT/|g" \
                -e "s|/usr/libexec/astrid/|$verify_exec_root/usr/libexec/astrid/|g" \
                -e "s|/usr/libexec/astrid-edge/immutable/|$verify_exec_root/usr/libexec/astrid-edge/immutable/|g" \
                "$unit"
        done < <(find "$unit_root" -type f -print0)
    done

    systemd --version | sed -n '1p'
    for rendered in "${VERIFY_RENDERED_ROOTS[@]}"; do
        SYSTEMD_UNIT_PATH="$rendered:$verify_support_root:" systemd-analyze verify \
            ollama-cpu.service \
            astrid-model-warmup.service \
            astrid.service \
            astrid-edge-runtime.service \
            astrid-edge-hindsight.service \
            astrid-edge-hindsight.timer
    done
else
    printf 'renderer-test: systemd-analyze unavailable; exact effective-property checks passed, systemd-249 verification deferred to CI\n'
fi

printf 'renderer-test: AVADO and ICP production renderer checks passed\n'
