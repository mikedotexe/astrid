#!/usr/bin/env bash
# Exercise the CPU-edge archive layout and both checksum layers without builds.

set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
test_root="$(mktemp -d)"
trap 'rm -rf -- "$test_root"' EXIT

binary_dir="$test_root/bin"
capsule_dir="$test_root/capsules"
external_capsule_dir="$test_root/external-capsules"
output_dir="$test_root/output"
install -d -m 0755 "$binary_dir" "$capsule_dir" "$external_capsule_dir" "$output_dir"

python3 - "$binary_dir" <<'PY'
import pathlib
import struct
import sys

root = pathlib.Path(sys.argv[1])
header = bytearray(64)
header[:4] = b"\x7fELF"
header[4:7] = bytes((2, 1, 1))  # ELF64, little-endian, current version.
struct.pack_into("<HHI", header, 16, 2, 62, 1)  # Executable, x86-64.
for name in (
    "astrid",
    "astrid-daemon",
    "astrid-build",
    "astrid-edge-runtime",
    "astrid-edge-steward-helper",
    "astrid-edge-rescue-helper",
    "astrid-edge-web-broker",
    "astrid-edge-provider-broker",
    "astrid-edge-presentation-broker",
    "astrid-edge-checkpoint",
):
    path = root / name
    path.write_bytes(header)
    path.chmod(0o755)
PY

python3 - "$capsule_dir" "$external_capsule_dir" <<'PY'
import io
import pathlib
import tarfile
import sys

local = [
    "astrid-capsule-cli", "astrid-capsule-fs", "astrid-capsule-http",
    "astrid-capsule-shell", "astrid-capsule-skills", "astrid-capsule-agents",
    "astrid-capsule-memory", "astrid-capsule-edge-context",
    "astrid-capsule-edge-introspector", "astrid-capsule-edge-spectral",
]
external = [
    "astrid-capsule-context-engine", "astrid-capsule-hook-bridge",
    "astrid-capsule-identity", "astrid-capsule-openai-compat",
    "astrid-capsule-prompt-builder", "astrid-capsule-react",
    "astrid-capsule-registry", "astrid-capsule-router",
    "astrid-capsule-session", "astrid-capsule-system",
]

def add_file(archive: tarfile.TarFile, name: str, content: bytes) -> None:
    info = tarfile.TarInfo(name)
    info.mode = 0o644
    info.size = len(content)
    archive.addfile(info, io.BytesIO(content))

for root, names in ((pathlib.Path(sys.argv[1]), local), (pathlib.Path(sys.argv[2]), external)):
    for capsule in names:
        manifest = (
            f'[package]\nname = "{capsule}"\nversion = "0.1.0"\n\n'
            '[[component]]\nid = "main"\nfile = "component.wasm"\ntype = "executable"\n'
        ).encode()
        with tarfile.open(root / f"{capsule}.capsule", "w:gz") as archive:
            add_file(archive, "Capsule.toml", manifest)
            add_file(archive, "component.wasm", b"\x00asm\x0d\x00\x01\x00")
PY

if "$script_dir/package_edge_appliance.sh" \
    --version test \
    --target unsupported-linux-target \
    --core-binary-dir "$binary_dir" \
    --edge-binary "$binary_dir/astrid-edge-runtime" \
    --steward-helper "$binary_dir/astrid-edge-steward-helper" \
    --rescue-helper "$binary_dir/astrid-edge-rescue-helper" \
    --web-broker "$binary_dir/astrid-edge-web-broker" \
    --provider-broker "$binary_dir/astrid-edge-provider-broker" \
    --presentation-broker "$binary_dir/astrid-edge-presentation-broker" \
    --checkpoint-helper "$binary_dir/astrid-edge-checkpoint" \
    --capsule-dir "$capsule_dir" \
    --external-capsule-dir "$external_capsule_dir" \
    --output-dir "$output_dir" >/dev/null 2>&1; then
    printf 'error: unsupported CPU-edge target was accepted\n' >&2
    exit 1
fi
if "$script_dir/package_edge_appliance.sh" \
    --version test \
    --target aarch64-unknown-linux-gnu \
    --core-binary-dir "$binary_dir" \
    --edge-binary "$binary_dir/astrid-edge-runtime" \
    --steward-helper "$binary_dir/astrid-edge-steward-helper" \
    --rescue-helper "$binary_dir/astrid-edge-rescue-helper" \
    --web-broker "$binary_dir/astrid-edge-web-broker" \
    --provider-broker "$binary_dir/astrid-edge-provider-broker" \
    --presentation-broker "$binary_dir/astrid-edge-presentation-broker" \
    --checkpoint-helper "$binary_dir/astrid-edge-checkpoint" \
    --capsule-dir "$capsule_dir" \
    --external-capsule-dir "$external_capsule_dir" \
    --output-dir "$output_dir" >/dev/null 2>&1; then
    printf 'error: x86-64 binaries were accepted for an ARM64 archive\n' >&2
    exit 1
fi
if "$script_dir/package_edge_appliance.sh" \
    --version ../escape \
    --target x86_64-unknown-linux-gnu \
    --core-binary-dir "$binary_dir" \
    --edge-binary "$binary_dir/astrid-edge-runtime" \
    --steward-helper "$binary_dir/astrid-edge-steward-helper" \
    --rescue-helper "$binary_dir/astrid-edge-rescue-helper" \
    --web-broker "$binary_dir/astrid-edge-web-broker" \
    --provider-broker "$binary_dir/astrid-edge-provider-broker" \
    --presentation-broker "$binary_dir/astrid-edge-presentation-broker" \
    --checkpoint-helper "$binary_dir/astrid-edge-checkpoint" \
    --capsule-dir "$capsule_dir" \
    --external-capsule-dir "$external_capsule_dir" \
    --output-dir "$output_dir" >/dev/null 2>&1; then
    printf 'error: unsafe CPU-edge version was accepted\n' >&2
    exit 1
fi

missing_external="$external_capsule_dir/astrid-capsule-session.capsule"
mv "$missing_external" "$test_root/astrid-capsule-session.capsule"
if "$script_dir/package_edge_appliance.sh" \
    --version test \
    --target x86_64-unknown-linux-gnu \
    --core-binary-dir "$binary_dir" \
    --edge-binary "$binary_dir/astrid-edge-runtime" \
    --steward-helper "$binary_dir/astrid-edge-steward-helper" \
    --rescue-helper "$binary_dir/astrid-edge-rescue-helper" \
    --web-broker "$binary_dir/astrid-edge-web-broker" \
    --provider-broker "$binary_dir/astrid-edge-provider-broker" \
    --presentation-broker "$binary_dir/astrid-edge-presentation-broker" \
    --checkpoint-helper "$binary_dir/astrid-edge-checkpoint" \
    --capsule-dir "$capsule_dir" \
    --external-capsule-dir "$external_capsule_dir" \
    --output-dir "$output_dir" >/dev/null 2>&1; then
    printf 'error: missing external pinned-source capsule was accepted\n' >&2
    exit 1
fi
mv "$test_root/astrid-capsule-session.capsule" "$missing_external"
cp "$missing_external" "$external_capsule_dir/unapproved.capsule"
if "$script_dir/package_edge_appliance.sh" \
    --version test \
    --target x86_64-unknown-linux-gnu \
    --core-binary-dir "$binary_dir" \
    --edge-binary "$binary_dir/astrid-edge-runtime" \
    --steward-helper "$binary_dir/astrid-edge-steward-helper" \
    --rescue-helper "$binary_dir/astrid-edge-rescue-helper" \
    --web-broker "$binary_dir/astrid-edge-web-broker" \
    --provider-broker "$binary_dir/astrid-edge-provider-broker" \
    --presentation-broker "$binary_dir/astrid-edge-presentation-broker" \
    --checkpoint-helper "$binary_dir/astrid-edge-checkpoint" \
    --capsule-dir "$capsule_dir" \
    --external-capsule-dir "$external_capsule_dir" \
    --output-dir "$output_dir" >/dev/null 2>&1; then
    printf 'error: extra external pinned-source capsule was accepted\n' >&2
    exit 1
fi
rm "$external_capsule_dir/unapproved.capsule"
cp "$missing_external" "$test_root/original-session.capsule"
rm "$missing_external"
cp "$capsule_dir/astrid-capsule-http.capsule" "$missing_external"
if "$script_dir/package_edge_appliance.sh" \
    --version test \
    --target x86_64-unknown-linux-gnu \
    --core-binary-dir "$binary_dir" \
    --edge-binary "$binary_dir/astrid-edge-runtime" \
    --steward-helper "$binary_dir/astrid-edge-steward-helper" \
    --rescue-helper "$binary_dir/astrid-edge-rescue-helper" \
    --web-broker "$binary_dir/astrid-edge-web-broker" \
    --provider-broker "$binary_dir/astrid-edge-provider-broker" \
    --presentation-broker "$binary_dir/astrid-edge-presentation-broker" \
    --checkpoint-helper "$binary_dir/astrid-edge-checkpoint" \
    --capsule-dir "$capsule_dir" \
    --external-capsule-dir "$external_capsule_dir" \
    --output-dir "$output_dir" >/dev/null 2>&1; then
    printf 'error: substituted external pinned-source capsule identity was accepted\n' >&2
    exit 1
fi
rm "$missing_external"
mv "$test_root/original-session.capsule" "$missing_external"

"$script_dir/package_edge_appliance.sh" \
    --version test \
    --target x86_64-unknown-linux-gnu \
    --core-binary-dir "$binary_dir" \
    --edge-binary "$binary_dir/astrid-edge-runtime" \
    --steward-helper "$binary_dir/astrid-edge-steward-helper" \
    --rescue-helper "$binary_dir/astrid-edge-rescue-helper" \
    --web-broker "$binary_dir/astrid-edge-web-broker" \
    --provider-broker "$binary_dir/astrid-edge-provider-broker" \
    --presentation-broker "$binary_dir/astrid-edge-presentation-broker" \
    --checkpoint-helper "$binary_dir/astrid-edge-checkpoint" \
    --capsule-dir "$capsule_dir" \
    --external-capsule-dir "$external_capsule_dir" \
    --output-dir "$output_dir"

archive="$output_dir/astrid-cpu-edge-test-x86_64-unknown-linux-gnu.tar.gz"
sha256sum -c "$archive.sha256"
extract_dir="$test_root/extracted"
install -d -m 0755 "$extract_dir"
tar -C "$extract_dir" -xzf "$archive"
bundle="$extract_dir/astrid-cpu-edge-test-x86_64-unknown-linux-gnu"
test -f "$bundle/LICENSE-js-pdk"
repository_root="$(CDPATH= cd -- "$script_dir/.." && pwd)"
if find "$bundle" \( -type d -name __pycache__ -o -type f \( -name '*.pyc' -o -name '*.pyo' \) \) -print -quit | grep -q .; then
    printf 'error: interpreter cache entered the CPU-edge archive\n' >&2
    exit 1
fi
if find "$bundle" -type l -print -quit | grep -q .; then
    printf 'error: symlink entered the CPU-edge archive\n' >&2
    exit 1
fi
if find "$bundle" -type f \( -name '*.key' -o -name '*.db' -o -name '*.sqlite' -o -name '*.log' -o -name '*.gguf' -o -name '*.safetensors' \) -print -quit | grep -q .; then
    printf 'error: credential, state, or model artifact entered the CPU-edge archive\n' >&2
    exit 1
fi
test -x "$bundle/astrid-edge-steward-helper"
test -x "$bundle/astrid-edge-rescue-helper"
test -x "$bundle/astrid-edge-web-broker"
test -x "$bundle/astrid-edge-provider-broker"
test -x "$bundle/astrid-edge-presentation-broker"
test -x "$bundle/astrid-edge-checkpoint"
for immutable_script in \
    build_edge_self_change_source_bundle.py \
    build_edge_self_change_supervisor_zipapp.py \
    build_edge_self_change_toolchain_bundle.py \
    edge_audio_feeder.py \
    edge_hindsight.py \
    edge_self_change_supervisor.py \
    install_edge_self_evolution_root.sh; do
    test -x "$bundle/scripts/$immutable_script"
done
test -x "$bundle/scripts/install_edge_self_evolution_root.sh"
test -x "$bundle/scripts/build_edge_self_change_source_bundle.py"
test -x "$bundle/scripts/build_edge_self_change_toolchain_bundle.py"
test -x "$bundle/scripts/build_edge_self_change_supervisor_zipapp.py"
test -x "$bundle/scripts/edge_audio_feeder.py"
test -f "$bundle/docs/cpu-edge-self-evolution.md"
test -f "$bundle/packaging/systemd/astrid-edge-core-liveness.service"
test -f "$bundle/packaging/systemd/astrid-edge-core-liveness.path.in"
test -f "$bundle/packaging/systemd/astrid-edge-presentation-broker.socket.in"
test -f "$bundle/packaging/systemd/astrid-edge-presentation-broker@.service.in"
test -f "$bundle/packaging/headless/edge-presentation-broker.json.in"
test -f "$bundle/packaging/headless/edge-audio-feeder.json.in"
test -f "$bundle/packaging/headless/edge-hindsight-writer.json.in"
test -f "$bundle/packaging/systemd/astrid-edge-audio-feeder.service"
test -f "$bundle/packaging/systemd/astrid-edge-audio-feeder.socket.in"
test ! -e "$bundle/packaging/systemd/icp/astrid-edge-audio-feeder.service"
test ! -e "$bundle/packaging/systemd/icp/astrid-edge-audio-feeder.socket"
grep -Fq \
    'ExecStart=/usr/bin/python3 -I -E -s /usr/libexec/astrid-edge/immutable/edge_audio_feeder.py --config /etc/astrid-edge-self-change/audio-feeder.json' \
    "$bundle/packaging/systemd/astrid-edge-audio-feeder.service"
grep -Fqx 'SocketUser=root' \
    "$bundle/packaging/systemd/astrid-edge-audio-feeder.socket.in"
grep -Fqx 'SocketGroup=@AUDIO_CLIENT_GROUP@' \
    "$bundle/packaging/systemd/astrid-edge-audio-feeder.socket.in"
grep -Fq 'unit_source_root/../../scripts/edge_audio_feeder.py' \
    "$bundle/scripts/install_edge_self_evolution_root.sh"
grep -Fq 'ICP contains an AVADO-only audio feeder artifact' \
    "$bundle/scripts/install_edge_self_evolution_root.sh"
test -f "$bundle/scripts/edge_self_change/supervisor.py"
# The generic archive is the only installation input. Prove that every
# root-owned unit/template, immutable configuration asset, and supervisor
# module from the release source is present exactly once in that archive.
find "$repository_root/packaging/systemd" -type f \
    ! -name '*.pyc' ! -name '*.pyo' -print \
    | sed "s|^$repository_root/||" | sort >"$test_root/expected-systemd-assets"
find "$bundle/packaging/systemd" -type f -print \
    | sed "s|^$bundle/||" | sort >"$test_root/actual-systemd-assets"
cmp "$test_root/expected-systemd-assets" "$test_root/actual-systemd-assets"
find "$repository_root/packaging/headless" -type f \
    ! -name introspection-AGENTS.md \
    ! -name introspection-memory.md \
    -print | sed "s|^$repository_root/||" | sort \
    >"$test_root/expected-headless-assets"
find "$bundle/packaging/headless" -type f -print \
    | sed "s|^$bundle/||" | sort >"$test_root/actual-headless-assets"
cmp "$test_root/expected-headless-assets" "$test_root/actual-headless-assets"
find "$repository_root/scripts/edge_self_change" -type f \
    ! -name '*.pyc' ! -name '*.pyo' -print \
    | sed "s|^$repository_root/||" | sort >"$test_root/expected-supervisor-modules"
find "$bundle/scripts/edge_self_change" -type f -print \
    | sed "s|^$bundle/||" | sort >"$test_root/actual-supervisor-modules"
cmp "$test_root/expected-supervisor-modules" "$test_root/actual-supervisor-modules"
test -x "$bundle/scripts/build_astralis_cpu_edge_capsules.py"
test -x "$bundle/scripts/install_essential_capsules.sh"
test -x "$bundle/scripts/install_headless_application_capsules.py"
test -x "$bundle/scripts/verify_edge_capsule_status.py"
for transactional_installer in \
    install_headless_linux.sh \
    install_edge_runtime.sh \
    install_essential_capsules.sh; do
    grep -Fq 'headless-application-capsules-*' \
        "$bundle/scripts/$transactional_installer"
done
test -x "$bundle/scripts/relay_edge_peer_review.py"
test -f "$bundle/capsules/astrid-capsule-edge-spectral.capsule"
test "$(find "$bundle/capsules" -maxdepth 1 -type f -name '*.capsule' | wc -l | tr -d ' ')" -eq 20
test ! -e "$bundle/packaging/headless/introspection-AGENTS.md"
test ! -e "$bundle/packaging/headless/introspection-memory.md"
test -x "$bundle/packaging/systemd/wait-for-icp-ssd"
for installer in install_headless_linux.sh install_edge_runtime.sh; do
    grep -Fq 'command+=(-u "$variable")' "$bundle/scripts/$installer"
done
grep -Fqx \
    'ExecCondition=%h/.local/libexec/astrid/wait-for-icp-ssd --systemd-condition --wait-seconds 75 --poll-seconds 2' \
    "$bundle/packaging/systemd/icp-ssd-required.conf"
grep -Fq 'UnsetEnvironment=ASTRID_ICP_SSD_GUARD_TEST_MODE ' \
    "$bundle/packaging/systemd/icp-ssd-required.conf"
if grep -Fq 'Environment=ASTRID_ICP_SSD_GUARD_TEST_MODE=' \
    "$bundle/packaging/systemd/icp-ssd-required.conf"; then
    printf 'error: production ICP drop-in exports the guard test gate\n' >&2
    exit 1
fi
if grep -Eq '^ConditionPath(IsMountPoint|IsSymbolicLink|IsDirectory)=/' \
    "$bundle/packaging/systemd/icp-ssd-required.conf"; then
    printf 'error: ICP SSD guard still contains clean-skip conditions\n' >&2
    exit 1
fi
grep -Fqx \
    'EnvironmentFile=%h/.config/astrid/edge-tuning-authority.env' \
    "$bundle/packaging/systemd/astrid-edge-tuning-authority.conf"
grep -Fqx 'ASTRID_EDGE_RESERVOIR_TUNING_ENABLED=false' \
    "$bundle/packaging/appliances/avado-i3-16g.env"
grep -Fqx 'ASTRID_EDGE_RESERVOIR_TUNING_PROFILE_PERMITS=true' \
    "$bundle/packaging/appliances/avado-i3-16g.env"
grep -Fqx 'ASTRID_EDGE_RESERVOIR_TUNING_ENABLED=false' \
    "$bundle/packaging/appliances/icp-j3455-8g.env"
grep -Fqx 'ASTRID_EDGE_RESERVOIR_TUNING_PROFILE_PERMITS=true' \
    "$bundle/packaging/appliances/icp-j3455-8g.env"
grep -Fqx 'ASTRID_EDGE_SOCKET=.astrid-icp/state/run/system.sock' \
    "$bundle/packaging/appliances/icp-j3455-8g.env"
grep -Fqx 'WorkingDirectory=%h' \
    "$bundle/packaging/systemd/icp/astrid-edge-runtime.service"
grep -Fqx 'EnvironmentFile=%h/.config/astrid/edge-appliance.env' \
    "$bundle/packaging/systemd/icp/astrid-edge-runtime.service"
grep -Fqx 'EnvironmentFile=%h/.config/astrid/edge-appliance.env' \
    "$bundle/packaging/systemd/icp/astrid.service"
grep -Fqx 'BindsTo=ollama-cpu.service' \
    "$bundle/packaging/systemd/icp/astrid-model-warmup.service"
grep -Fqx 'TimeoutStartSec=12min' \
    "$bundle/packaging/systemd/astrid-model-warmup.service"
grep -Fqx 'TimeoutStartSec=12min' \
    "$bundle/packaging/systemd/icp/astrid-model-warmup.service"
grep -Fqx 'Environment=ASTRID_HOME=%h/.astrid-icp/state' \
    "$bundle/packaging/systemd/icp/astrid-model-warmup.service"
grep -Fq 'Wants=network-online.target astrid-model-warmup.service' \
    "$bundle/packaging/systemd/icp/ollama-cpu.service"
grep -Fqx 'BindsTo=astrid-model-warmup.service' \
    "$bundle/packaging/systemd/icp/astrid.service"
grep -Fqx 'BindsTo=astrid.service' \
    "$bundle/packaging/systemd/icp/astrid-edge-runtime.service"

warmup_home="$test_root/warmup-home"
warmup_mock_bin="$test_root/warmup-mock-bin"
install -d -m 0700 \
    "$warmup_home/.astrid-icp/state/home/default/edge" \
    "$warmup_mock_bin"
printf '#!/usr/bin/env sh\nexit 0\n' > "$warmup_mock_bin/curl"
chmod 0755 "$warmup_mock_bin/curl"
(
    cd "$warmup_home"
    HOME="$warmup_home" \
        PATH="$warmup_mock_bin:$PATH" \
        ASTRID_HOME="$warmup_home/.astrid-icp/state" \
        ASTRID_EDGE_WORKSPACE=state/home/default/edge \
        ASTRID_OLLAMA_MODEL=qwen3:test \
        ASTRID_OLLAMA_KEEP_ALIVE=2h \
        "$bundle/scripts/warm_ollama_model.sh"
)
test -f \
    "$warmup_home/.astrid-icp/state/home/default/edge/runtime/model_warmup.json"
test ! -e "$warmup_home/state"
grep -Fqx 'ASTRID_EDGE_RESERVOIR_TUNING_PROFILE_PERMITS=false' \
    "$bundle/packaging/appliances/generic-cpu.env"
grep -Fqx 'ASTRID_EDGE_RESERVOIR_TUNING_PROFILE_PERMITS=false' \
    "$bundle/packaging/appliances/icp-discovery.env"
mock_command_dir="$test_root/mock-command-bin"
test_home="$test_root/home"
install -d -m 0755 "$mock_command_dir" "$test_home"
printf '#!/usr/bin/env sh\nprintf "Linux\\n"\n' > "$mock_command_dir/uname"
printf '#!/usr/bin/env sh\nexit 0\n' > "$mock_command_dir/systemctl"
printf '#!/usr/bin/env sh\nexit 0\n' > "$mock_command_dir/flock"
chmod 0755 \
    "$mock_command_dir/uname" \
    "$mock_command_dir/systemctl" \
    "$mock_command_dir/flock"
(
    cd "$bundle"
    sha256sum -c SHA256SUMS
    install_output="$(
        HOME="$test_home" PATH="$mock_command_dir:$PATH" \
            ./scripts/install_essential_capsules.sh \
            --capsule-dir ./capsules \
            --restart \
            --dry-run
    )"
    printf '%s\n' "$install_output"
    [[ "$install_output" == *"exactly 20 total capsules loaded"* ]]
)

required_arguments=()
for capsule in \
    astrid-capsule-cli \
    astrid-capsule-fs \
    astrid-capsule-http \
    astrid-capsule-shell \
    astrid-capsule-skills \
    astrid-capsule-agents \
    astrid-capsule-memory \
    astrid-capsule-edge-context \
    astrid-capsule-edge-introspector \
    astrid-capsule-edge-spectral \
    astrid-capsule-context-engine \
    astrid-capsule-hook-bridge \
    astrid-capsule-identity \
    astrid-capsule-openai-compat \
    astrid-capsule-prompt-builder \
    astrid-capsule-react \
    astrid-capsule-registry \
    astrid-capsule-router \
    astrid-capsule-session \
    astrid-capsule-system; do
    required_arguments+=(--required "$capsule")
done
valid_status='{"status":{"loaded_capsules":["astrid-capsule-cli","astrid-capsule-fs","astrid-capsule-http","astrid-capsule-shell","astrid-capsule-skills","astrid-capsule-agents","astrid-capsule-memory","astrid-capsule-edge-context","astrid-capsule-edge-introspector","astrid-capsule-edge-spectral","astrid-capsule-context-engine","astrid-capsule-hook-bridge","astrid-capsule-identity","astrid-capsule-openai-compat","astrid-capsule-prompt-builder","astrid-capsule-react","astrid-capsule-registry","astrid-capsule-router","astrid-capsule-session","astrid-capsule-system"]}}'
printf '%s\n' "$valid_status" \
    | python3 "$bundle/scripts/verify_edge_capsule_status.py" \
        --expected-total 20 \
        "${required_arguments[@]}"
invalid_status='{"status":{"loaded_capsules":["astrid-capsule-cli","astrid-capsule-fs","astrid-capsule-http","astrid-capsule-shell","astrid-capsule-skills","astrid-capsule-agents","astrid-capsule-memory","astrid-capsule-edge-context","astrid-capsule-edge-introspector","astrid-capsule-edge-spectral"]}}'
if printf '%s\n' "$invalid_status" \
    | python3 "$bundle/scripts/verify_edge_capsule_status.py" \
        --expected-total 20 \
        "${required_arguments[@]}" >/dev/null 2>&1; then
    printf 'error: 19 loaded capsules incorrectly satisfied the 20-capsule contract\n' >&2
    exit 1
fi

observation_output="$(
    HOME="$test_home" PATH="$mock_command_dir:$PATH" \
        "$bundle/scripts/install_edge_runtime.sh" \
            --binary "$bundle/astrid-edge-runtime" \
            --profile avado-i3-16g \
            --observation-only \
            --dry-run
)"
[[ "$observation_output" == *"Reservoir tuning authority mode: observation-only"* ]]
[[ "$observation_output" == *"astrid-edge-observation-only.env"* ]]
[[ "$observation_output" == *"10-tuning-authority.conf"* ]]
[[ "$observation_output" == *"stage and verify generation"* ]]
[[ "$observation_output" == *"committed verified install generation"* ]]

tuning_output="$(
    HOME="$test_home" PATH="$mock_command_dir:$PATH" \
        "$bundle/scripts/install_edge_runtime.sh" \
            --binary "$bundle/astrid-edge-runtime" \
            --profile avado-i3-16g \
            --enable-tuning \
            --dry-run
)"
[[ "$tuning_output" == *"Reservoir tuning authority mode: enabled"* ]]
[[ "$tuning_output" == *"astrid-edge-tuning-enabled.env"* ]]
if HOME="$test_home" PATH="$mock_command_dir:$PATH" \
    "$bundle/scripts/install_edge_runtime.sh" \
        --binary "$bundle/astrid-edge-runtime" \
        --profile generic-cpu \
        --enable-tuning \
        --dry-run >/dev/null 2>&1; then
    printf 'error: generic profile incorrectly permitted reservoir tuning\n' >&2
    exit 1
fi

icp_runtime_output="$(
    HOME="$test_home" PATH="$mock_command_dir:$PATH" \
        "$bundle/scripts/install_edge_runtime.sh" \
            --binary "$bundle/astrid-edge-runtime" \
            --profile icp-j3455-8g \
            --layout icp-ssd \
            --observation-only \
            --start \
            --dry-run
)"
[[ "$icp_runtime_output" == *"Selected install layout: icp-ssd"* ]]
[[ "$icp_runtime_output" == *"ssd-required.conf"* ]]
[[ "$icp_runtime_output" == *"wait-for-icp-ssd"* ]]
[[ "$icp_runtime_output" == *"astrid-edge-hindsight.service.d"* ]]
[[ "$icp_runtime_output" == *".astrid-icp/state"* ]]
[[ "$icp_runtime_output" == *"mask --runtime astrid-edge-runtime.service"* ]]
[[ "$icp_runtime_output" == *"restart ollama-cpu.service"* ]]
[[ "$icp_runtime_output" != *"restart astrid.service"* ]]

icp_core_output="$(
    HOME="$test_home" PATH="$mock_command_dir:$PATH" \
        "$bundle/scripts/install_headless_linux.sh" \
            --binary-dir "$bundle" \
            --layout icp-ssd \
            --start \
            --dry-run
)"
[[ "$icp_core_output" == *"Selected install layout: icp-ssd"* ]]
[[ "$icp_core_output" == *".astrid-icp/state"* ]]
[[ "$icp_core_output" == *"ollama-cpu.service"* ]]
[[ "$icp_core_output" == *"ssd-required.conf"* ]]
[[ "$icp_core_output" == *"wait-for-icp-ssd"* ]]
[[ "$icp_core_output" == *"--mount-only"* ]]
[[ "$icp_core_output" == *"mask --runtime astrid-edge-runtime.service"* ]]
[[ "$icp_core_output" == *"restart ollama-cpu.service"* ]]
[[ "$icp_core_output" != *"restart astrid.service"* ]]
[[ "$icp_core_output" == *"verified files using atomic renames with generation rollback"* ]]

standard_core_output="$(
    HOME="$test_home" PATH="$mock_command_dir:$PATH" \
        "$bundle/scripts/install_headless_linux.sh" \
            --binary-dir "$bundle" \
            --layout standard \
            --dry-run
)"
[[ "$standard_core_output" == *"ollama-cpu.service"* ]]
[[ "$standard_core_output" == *"require executable"*".local/bin/ollama"* ]]
[[ "$standard_core_output" == *"stage and verify generation"* ]]
if HOME="$test_home" PATH="$mock_command_dir:$PATH" \
    "$bundle/scripts/install_headless_linux.sh" \
        --binary-dir "$bundle" \
        --layout standard \
        --start >/dev/null 2>&1; then
    printf 'error: standard service start accepted a missing Ollama executable\n' >&2
    exit 1
fi
install -d -m 0755 "$test_home/.local/bin"
printf '#!/usr/bin/env sh\nexit 0\n' > "$test_home/.local/bin/ollama"
chmod 0755 "$test_home/.local/bin/ollama"
HOME="$test_home" PATH="$mock_command_dir:$PATH" \
    "$bundle/scripts/install_headless_linux.sh" \
        --binary-dir "$bundle" \
        --layout standard >/dev/null
cmp \
    "$bundle/packaging/systemd/ollama-cpu.service" \
    "$test_home/.config/systemd/user/ollama-cpu.service"

printf 'prior-core-generation\n' > "$test_home/.astrid/bin/astrid"
real_mv="$(command -v mv)"
fault_command_dir="$test_root/fault-command-bin"
install -d -m 0755 "$fault_command_dir"
printf '%s\n' \
    '#!/usr/bin/env bash' \
    'set -euo pipefail' \
    'for argument in "$@"; do' \
    '    if [[ "$argument" == *.astrid-stage.* ]]; then' \
    '        count=0' \
    '        [[ -f "$MOCK_MV_COUNT" ]] && count="$(<"$MOCK_MV_COUNT")"' \
    '        count=$((count + 1))' \
    '        printf "%s\\n" "$count" > "$MOCK_MV_COUNT"' \
    '        if [[ "$count" == "$MOCK_MV_FAIL_AT" ]]; then exit 79; fi' \
    '        break' \
    '    fi' \
    'done' \
    'exec "$REAL_MV" "$@"' \
    > "$fault_command_dir/mv"
chmod 0755 "$fault_command_dir/mv"
if HOME="$test_home" \
    PATH="$fault_command_dir:$mock_command_dir:$PATH" \
    REAL_MV="$real_mv" \
    MOCK_MV_COUNT="$test_root/mv-count" \
    MOCK_MV_FAIL_AT=2 \
    "$bundle/scripts/install_headless_linux.sh" \
        --binary-dir "$bundle" \
        --layout standard >/dev/null 2>&1; then
    printf 'error: injected core-generation switch failure unexpectedly succeeded\n' >&2
    exit 1
fi
grep -Fqx 'prior-core-generation' "$test_home/.astrid/bin/astrid"
if find "$test_home" \
    \( -name '*.astrid-stage.*' -o -name '*.astrid-backup.*' \) \
    -print -quit | grep -q .; then
    printf 'error: core-generation rollback left transaction files behind\n' >&2
    exit 1
fi

HOME="$test_home" PATH="$mock_command_dir:$PATH" \
    "$bundle/scripts/install_edge_runtime.sh" \
        --binary "$bundle/astrid-edge-runtime" \
        --profile avado-i3-16g \
        --observation-only >/dev/null
printf 'prior-edge-generation\n' > "$test_home/.astrid/bin/astrid-edge-runtime"
rm -f "$test_root/mv-count"
if HOME="$test_home" \
    PATH="$fault_command_dir:$mock_command_dir:$PATH" \
    REAL_MV="$real_mv" \
    MOCK_MV_COUNT="$test_root/mv-count" \
    MOCK_MV_FAIL_AT=2 \
    "$bundle/scripts/install_edge_runtime.sh" \
        --binary "$bundle/astrid-edge-runtime" \
        --profile avado-i3-16g \
        --observation-only >/dev/null 2>&1; then
    printf 'error: injected edge-generation switch failure unexpectedly succeeded\n' >&2
    exit 1
fi
grep -Fqx 'prior-edge-generation' "$test_home/.astrid/bin/astrid-edge-runtime"
if find "$test_home" \
    \( -name '*.astrid-stage.*' -o -name '*.astrid-backup.*' \) \
    -print -quit | grep -q .; then
    printf 'error: edge-generation rollback left transaction files behind\n' >&2
    exit 1
fi

stateful_command_dir="$test_root/stateful-command-bin"
install -d -m 0755 "$stateful_command_dir"
printf '%s\n' \
    '#!/usr/bin/env bash' \
    'set -euo pipefail' \
    '[[ "${1:-}" == "--user" ]] && shift' \
    'command_name="${1:-}"' \
    '[[ -n "$command_name" ]] && shift' \
    'state_dir="${MOCK_SYSTEMCTL_STATE_DIR:?}"' \
    'mkdir -p "$state_dir"' \
    'if [[ -n "${MOCK_SYSTEMCTL_LOG:-}" ]]; then printf "%s %s\\n" "$command_name" "$*" >> "$MOCK_SYSTEMCTL_LOG"; fi' \
    'case "$command_name" in' \
    '    daemon-reload) exit 0 ;;' \
    '    cat)' \
    '        [[ -f "$HOME/.config/systemd/user/${1:?}" ]]' \
    '        ;;' \
    '    is-enabled)' \
    '        [[ "${1:-}" == "--quiet" ]] && shift' \
    '        [[ -f "$state_dir/${1:?}.enabled" ]] && grep -Fqx 1 "$state_dir/$1.enabled"' \
    '        ;;' \
    '    is-active)' \
    '        [[ "${1:-}" == "--quiet" ]] && shift' \
    '        [[ -f "$state_dir/${1:?}.active" ]] && grep -Fqx 1 "$state_dir/$1.active"' \
    '        ;;' \
    '    enable | disable)' \
    '        value=0' \
    '        [[ "$command_name" == "enable" ]] && value=1' \
    '        for unit in "$@"; do printf "%s\\n" "$value" > "$state_dir/$unit.enabled"; done' \
    '        ;;' \
    '    restart | start | stop)' \
    '        value=1' \
    '        [[ "$command_name" == "stop" ]] && value=0' \
    '        for unit in "$@"; do' \
    '            operation="$command_name:$unit"' \
    '            if [[ "${MOCK_SYSTEMCTL_FAIL_ONCE:-}" == "$operation" && ! -e "$state_dir/failure-injected" ]]; then' \
    '                : > "$state_dir/failure-injected"' \
    '                exit 78' \
    '            fi' \
    '            printf "%s\\n" "$value" > "$state_dir/$unit.active"' \
    '        done' \
    '        ;;' \
    '    *) printf "unexpected mocked systemctl command: %s\\n" "$command_name" >&2; exit 2 ;;' \
    'esac' \
    > "$stateful_command_dir/systemctl"
chmod 0755 "$stateful_command_dir/systemctl"

core_service_home="$test_root/core-service-home"
core_state="$test_root/core-systemd-state"
install -d -m 0755 \
    "$core_service_home/.local/bin" \
    "$core_service_home/.astrid/bin" \
    "$core_service_home/.config/systemd/user" \
    "$core_state"
printf '#!/usr/bin/env sh\nexit 0\n' > "$core_service_home/.local/bin/ollama"
chmod 0755 "$core_service_home/.local/bin/ollama"
printf 'prior-core-binary\n' > "$core_service_home/.astrid/bin/astrid"
chmod 0755 "$core_service_home/.astrid/bin/astrid"
for unit in \
    ollama-cpu.service \
    astrid-model-warmup.service \
    astrid.service; do
    printf 'prior-core-unit:%s\n' "$unit" \
        > "$core_service_home/.config/systemd/user/$unit"
done
printf '0\n' > "$core_state/ollama-cpu.service.enabled"
printf '1\n' > "$core_state/ollama-cpu.service.active"
printf '1\n' > "$core_state/astrid-model-warmup.service.enabled"
printf '0\n' > "$core_state/astrid-model-warmup.service.active"
printf '0\n' > "$core_state/astrid.service.enabled"
printf '1\n' > "$core_state/astrid.service.active"
if HOME="$core_service_home" \
    PATH="$stateful_command_dir:$mock_command_dir:$PATH" \
    MOCK_SYSTEMCTL_STATE_DIR="$core_state" \
    MOCK_SYSTEMCTL_FAIL_ONCE='restart:astrid-model-warmup.service' \
    "$bundle/scripts/install_headless_linux.sh" \
        --binary-dir "$bundle" \
        --layout standard \
        --start >/dev/null 2>&1; then
    printf 'error: injected core service-transition failure unexpectedly succeeded\n' >&2
    exit 1
fi
test -f "$core_state/failure-injected"
grep -Fqx 'prior-core-binary' "$core_service_home/.astrid/bin/astrid"
grep -Fqx 'prior-core-unit:astrid.service' \
    "$core_service_home/.config/systemd/user/astrid.service"
grep -Fqx 0 "$core_state/ollama-cpu.service.enabled"
grep -Fqx 1 "$core_state/ollama-cpu.service.active"
grep -Fqx 1 "$core_state/astrid-model-warmup.service.enabled"
grep -Fqx 0 "$core_state/astrid-model-warmup.service.active"
grep -Fqx 0 "$core_state/astrid.service.enabled"
grep -Fqx 1 "$core_state/astrid.service.active"

edge_service_home="$test_root/edge-service-home"
edge_state="$test_root/edge-systemd-state"
install -d -m 0755 \
    "$edge_service_home/.astrid/bin" \
    "$edge_service_home/.config/systemd/user" \
    "$edge_state"
printf 'prior-edge-binary\n' \
    > "$edge_service_home/.astrid/bin/astrid-edge-runtime"
chmod 0755 "$edge_service_home/.astrid/bin/astrid-edge-runtime"
for unit in \
    astrid-model-warmup.service \
    astrid-edge-runtime.service \
    astrid-edge-hindsight.service \
    astrid-edge-hindsight.timer; do
    printf 'prior-edge-unit:%s\n' "$unit" \
        > "$edge_service_home/.config/systemd/user/$unit"
done
printf '0\n' > "$edge_state/astrid-model-warmup.service.enabled"
printf '1\n' > "$edge_state/astrid-model-warmup.service.active"
printf '1\n' > "$edge_state/astrid-edge-runtime.service.enabled"
printf '0\n' > "$edge_state/astrid-edge-runtime.service.active"
printf '0\n' > "$edge_state/astrid-edge-hindsight.timer.enabled"
printf '1\n' > "$edge_state/astrid-edge-hindsight.timer.active"
if HOME="$edge_service_home" \
    PATH="$stateful_command_dir:$mock_command_dir:$PATH" \
    MOCK_SYSTEMCTL_STATE_DIR="$edge_state" \
    MOCK_SYSTEMCTL_FAIL_ONCE='restart:astrid-edge-runtime.service' \
    "$bundle/scripts/install_edge_runtime.sh" \
        --binary "$bundle/astrid-edge-runtime" \
        --profile avado-i3-16g \
        --observation-only \
        --start >/dev/null 2>&1; then
    printf 'error: injected edge service-transition failure unexpectedly succeeded\n' >&2
    exit 1
fi
test -f "$edge_state/failure-injected"
grep -Fqx 'prior-edge-binary' \
    "$edge_service_home/.astrid/bin/astrid-edge-runtime"
grep -Fqx 'prior-edge-unit:astrid-edge-runtime.service' \
    "$edge_service_home/.config/systemd/user/astrid-edge-runtime.service"
grep -Fqx 0 "$edge_state/astrid-model-warmup.service.enabled"
grep -Fqx 1 "$edge_state/astrid-model-warmup.service.active"
grep -Fqx 1 "$edge_state/astrid-edge-runtime.service.enabled"
grep -Fqx 0 "$edge_state/astrid-edge-runtime.service.active"
grep -Fqx 0 "$edge_state/astrid-edge-hindsight.timer.enabled"
grep -Fqx 1 "$edge_state/astrid-edge-hindsight.timer.active"

ledger_home="$test_root/ledger-home"
external_ledger="$test_root/external-dispatch-ledger.jsonl"
install -d -m 0700 "$ledger_home/.astrid/home/default/edge/actions"
printf '{}\n' > "$external_ledger"
chmod 0644 "$external_ledger"
ln -s "$external_ledger" \
    "$ledger_home/.astrid/home/default/edge/actions/dispatches.jsonl"
ledger_symlink_error="$test_root/ledger-symlink-error.log"
if HOME="$ledger_home" PATH="$mock_command_dir:$PATH" \
    "$bundle/scripts/install_edge_runtime.sh" \
        --binary "$bundle/astrid-edge-runtime" \
        --profile avado-i3-16g \
        --observation-only >/dev/null 2>"$ledger_symlink_error"; then
    printf 'error: edge installer followed an activity-ledger symlink\n' >&2
    exit 1
fi
grep -Fq 'refusing owner-mode normalization through ledger symlink' \
    "$ledger_symlink_error"
external_mode="$(stat -c '%a' "$external_ledger" 2>/dev/null || stat -f '%Lp' "$external_ledger")"
[[ "$external_mode" == "644" ]]

nonregular_home="$test_root/nonregular-ledger-home"
install -d -m 0700 \
    "$nonregular_home/.astrid/home/default/edge/actions/receipts.jsonl"
nonregular_ledger_error="$test_root/nonregular-ledger-error.log"
if HOME="$nonregular_home" PATH="$mock_command_dir:$PATH" \
    "$bundle/scripts/install_edge_runtime.sh" \
        --binary "$bundle/astrid-edge-runtime" \
        --profile avado-i3-16g \
        --observation-only >/dev/null 2>"$nonregular_ledger_error"; then
    printf 'error: edge installer accepted a nonregular activity ledger\n' >&2
    exit 1
fi
grep -Fq 'activity ledger is not a regular file' "$nonregular_ledger_error"

icp_capsule_output="$(
    HOME="$test_home" \
        "$bundle/scripts/install_essential_capsules.sh" \
            --capsule-dir "$bundle/capsules" \
            --layout icp-ssd \
            --dry-run
)"
[[ "$icp_capsule_output" == *"ASTRID_HOME="*".astrid-icp/state"* ]]
[[ "$icp_capsule_output" == *"preflight all capsule archives"* ]]
[[ "$icp_capsule_output" == *"committed verified capsule generation"* ]]

install -d -m 0755 "$test_home/.astrid-icp"
if HOME="$test_home" PATH="$mock_command_dir:$PATH" \
    "$bundle/scripts/install_headless_linux.sh" \
        --binary-dir "$bundle" \
        --layout icp-ssd \
        --dry-run >/dev/null 2>&1; then
    printf 'error: ICP installer accepted a non-symlink home tree\n' >&2
    exit 1
fi

core_symlink_home="$test_root/core-symlink-home"
core_external_target="$test_root/core-external-target"
install -d -m 0700 "$core_symlink_home/.astrid" "$core_external_target"
printf 'core-external-sentinel\n' > "$core_external_target/sentinel"
ln -s "$core_external_target" "$core_symlink_home/.astrid/bin"
core_symlink_error="$test_root/core-symlink-ancestor-error.log"
if HOME="$core_symlink_home" PATH="$mock_command_dir:$PATH" \
    "$bundle/scripts/install_headless_linux.sh" \
        --binary-dir "$bundle" \
        --layout standard >/dev/null 2>"$core_symlink_error"; then
    printf 'error: core installer followed a managed ancestor symlink\n' >&2
    exit 1
fi
grep -Fq 'managed directory component must not be a symlink' \
    "$core_symlink_error"
grep -Fqx 'core-external-sentinel' "$core_external_target/sentinel"
test "$(find "$core_external_target" -mindepth 1 -maxdepth 1 -print | wc -l | tr -d ' ')" -eq 1

edge_symlink_home="$test_root/edge-symlink-home"
edge_external_target="$test_root/edge-external-target"
install -d -m 0700 "$edge_symlink_home/.astrid" "$edge_external_target"
printf 'edge-external-sentinel\n' > "$edge_external_target/sentinel"
ln -s "$edge_external_target" "$edge_symlink_home/.astrid/bin"
edge_symlink_error="$test_root/edge-symlink-ancestor-error.log"
if HOME="$edge_symlink_home" PATH="$mock_command_dir:$PATH" \
    "$bundle/scripts/install_edge_runtime.sh" \
        --binary "$bundle/astrid-edge-runtime" \
        --profile avado-i3-16g \
        --observation-only >/dev/null 2>"$edge_symlink_error"; then
    printf 'error: edge installer followed a managed ancestor symlink\n' >&2
    exit 1
fi
grep -Fq 'managed directory component must not be a symlink' \
    "$edge_symlink_error"
grep -Fqx 'edge-external-sentinel' "$edge_external_target/sentinel"
test "$(find "$edge_external_target" -mindepth 1 -maxdepth 1 -print | wc -l | tr -d ' ')" -eq 1

capsule_names=(
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
live_capsule_root="$test_home/.astrid/home/default/.local/capsules"
install -d -m 0700 "$live_capsule_root"
for capsule in "${capsule_names[@]}"; do
    install -d -m 0700 "$live_capsule_root/$capsule"
    printf 'prior:%s\n' "$capsule" > "$live_capsule_root/$capsule/prior.txt"
done
printf '%s\n' \
    '#!/usr/bin/env bash' \
    'set -euo pipefail' \
    'if [[ "${1:-}" != "capsule" || "${2:-}" != "install" ]]; then exit 2; fi' \
    'archive="$3"' \
    'capsule="${archive##*/}"' \
    'capsule="${capsule%.capsule}"' \
    'target="$ASTRID_HOME/home/default/.local/capsules/$capsule"' \
    'mkdir -p "$target"' \
    'printf "[package]\\nname = \\"%s\\"\\nversion = \\"1.0.0\\"\\n" "$capsule" > "$target/Capsule.toml"' \
    'printf "{}\\n" > "$target/meta.json"' \
    'printf "new:%s\\n" "$capsule" > "$target/new.txt"' \
    'if [[ "$ASTRID_HOME" == "$MOCK_LIVE_ASTRID_HOME" ]]; then' \
    '    count=0' \
    '    [[ -f "$MOCK_CAPSULE_COUNT" ]] && count="$(<"$MOCK_CAPSULE_COUNT")"' \
    '    count=$((count + 1))' \
    '    printf "%s\\n" "$count" > "$MOCK_CAPSULE_COUNT"' \
    '    if [[ "$MOCK_CAPSULE_FAIL_AT" != 0 && "$count" == "$MOCK_CAPSULE_FAIL_AT" ]]; then exit 71; fi' \
    'fi' \
    > "$bundle/astrid"
chmod 0755 "$bundle/astrid"

capsule_symlink_home="$test_root/capsule-symlink-home"
capsule_external_target="$test_root/capsule-external-target"
install -d -m 0700 \
    "$capsule_symlink_home/.astrid/home/default/.local" \
    "$capsule_external_target"
printf 'capsule-external-sentinel\n' > "$capsule_external_target/sentinel"
ln -s "$capsule_external_target" \
    "$capsule_symlink_home/.astrid/home/default/.local/capsules"
capsule_symlink_error="$test_root/capsule-symlink-ancestor-error.log"
if HOME="$capsule_symlink_home" PATH="$mock_command_dir:$PATH" \
    "$bundle/scripts/install_essential_capsules.sh" \
        --capsule-dir "$bundle/capsules" >/dev/null 2>"$capsule_symlink_error"; then
    printf 'error: capsule installer followed a managed ancestor symlink\n' >&2
    exit 1
fi
grep -Fq 'managed directory component must not be a symlink' \
    "$capsule_symlink_error"
grep -Fqx 'capsule-external-sentinel' "$capsule_external_target/sentinel"
test "$(find "$capsule_external_target" -mindepth 1 -maxdepth 1 -print | wc -l | tr -d ' ')" -eq 1

if HOME="$test_home" \
    PATH="$mock_command_dir:$PATH" \
    MOCK_LIVE_ASTRID_HOME="$test_home/.astrid" \
    MOCK_CAPSULE_COUNT="$test_root/capsule-count" \
    MOCK_CAPSULE_FAIL_AT=2 \
    "$bundle/scripts/install_essential_capsules.sh" \
        --capsule-dir "$bundle/capsules" >/dev/null 2>&1; then
    printf 'error: injected capsule-set failure unexpectedly succeeded\n' >&2
    exit 1
fi
for capsule in "${capsule_names[@]}"; do
    grep -Fqx "prior:$capsule" "$live_capsule_root/$capsule/prior.txt"
    test ! -e "$live_capsule_root/$capsule/new.txt"
done
if find "$test_home/.astrid/.install-transactions" \
    -maxdepth 1 -type d -name 'essential-capsules-*' -print -quit | grep -q .; then
    printf 'error: capsule-set rollback left a transaction directory behind\n' >&2
    exit 1
fi

capsule_service_state="$test_root/capsule-systemd-state"
install -d -m 0755 "$capsule_service_state"
printf '0\n' > "$capsule_service_state/astrid.service.active"
rm -f "$test_root/capsule-count" "$capsule_service_state/failure-injected"
if HOME="$test_home" \
    PATH="$stateful_command_dir:$mock_command_dir:$PATH" \
    MOCK_SYSTEMCTL_STATE_DIR="$capsule_service_state" \
    MOCK_SYSTEMCTL_FAIL_ONCE='restart:astrid.service' \
    MOCK_LIVE_ASTRID_HOME="$test_home/.astrid" \
    MOCK_CAPSULE_COUNT="$test_root/capsule-count" \
    MOCK_CAPSULE_FAIL_AT=0 \
    "$bundle/scripts/install_essential_capsules.sh" \
        --capsule-dir "$bundle/capsules" \
        --restart >/dev/null 2>&1; then
    printf 'error: injected capsule restart failure unexpectedly succeeded\n' >&2
    exit 1
fi
test -f "$capsule_service_state/failure-injected"
grep -Fqx 0 "$capsule_service_state/astrid.service.active"
for capsule in "${capsule_names[@]}"; do
    grep -Fqx "prior:$capsule" "$live_capsule_root/$capsule/prior.txt"
    test ! -e "$live_capsule_root/$capsule/new.txt"
done

printf '1\n' > "$capsule_service_state/astrid.service.active"
rm -f "$test_root/capsule-count" "$capsule_service_state/failure-injected"
if HOME="$test_home" \
    PATH="$stateful_command_dir:$mock_command_dir:$PATH" \
    MOCK_SYSTEMCTL_STATE_DIR="$capsule_service_state" \
    MOCK_SYSTEMCTL_FAIL_ONCE='restart:astrid.service' \
    MOCK_LIVE_ASTRID_HOME="$test_home/.astrid" \
    MOCK_CAPSULE_COUNT="$test_root/capsule-count" \
    MOCK_CAPSULE_FAIL_AT=0 \
    "$bundle/scripts/install_essential_capsules.sh" \
        --capsule-dir "$bundle/capsules" \
        --restart >/dev/null 2>&1; then
    printf 'error: injected capsule restart failure unexpectedly succeeded\n' >&2
    exit 1
fi
test -f "$capsule_service_state/failure-injected"
grep -Fqx 1 "$capsule_service_state/astrid.service.active"
for capsule in "${capsule_names[@]}"; do
    grep -Fqx "prior:$capsule" "$live_capsule_root/$capsule/prior.txt"
    test ! -e "$live_capsule_root/$capsule/new.txt"
done

no_restart_log="$test_root/capsule-no-restart-systemctl.log"
rm -f "$test_root/capsule-count" "$no_restart_log"
if HOME="$test_home" \
    PATH="$stateful_command_dir:$mock_command_dir:$PATH" \
    MOCK_SYSTEMCTL_STATE_DIR="$capsule_service_state" \
    MOCK_SYSTEMCTL_LOG="$no_restart_log" \
    MOCK_LIVE_ASTRID_HOME="$test_home/.astrid" \
    MOCK_CAPSULE_COUNT="$test_root/capsule-count" \
    MOCK_CAPSULE_FAIL_AT=2 \
    "$bundle/scripts/install_essential_capsules.sh" \
        --capsule-dir "$bundle/capsules" >/dev/null 2>&1; then
    printf 'error: injected no-restart capsule failure unexpectedly succeeded\n' >&2
    exit 1
fi
test ! -e "$no_restart_log"
grep -Fqx 1 "$capsule_service_state/astrid.service.active"

rm -f "$test_root/capsule-count"
HOME="$test_home" \
    PATH="$mock_command_dir:$PATH" \
    MOCK_LIVE_ASTRID_HOME="$test_home/.astrid" \
    MOCK_CAPSULE_COUNT="$test_root/capsule-count" \
    MOCK_CAPSULE_FAIL_AT=0 \
    "$bundle/scripts/install_essential_capsules.sh" \
        --capsule-dir "$bundle/capsules" >/dev/null
for capsule in "${capsule_names[@]}"; do
    grep -Fqx "new:$capsule" "$live_capsule_root/$capsule/new.txt"
done
test -f "$test_home/.astrid/etc/install-manifests/essential-capsules.current"

python3 - "$bundle/BUILD-MANIFEST.json" <<'PY'
import json
import pathlib
import sys

manifest = json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
assert manifest["schema"] == "astrid_cpu_edge_build_manifest_v3"
assert manifest["bundle_format"] == "cpu-edge.3"
assert manifest["target"] == "x86_64-unknown-linux-gnu"
assert manifest["binary_format"] == "elf64-little-endian"
assert manifest["binary_architecture_verified"] is True
assert manifest["source_tree_state"] in {"clean", "dirty", "unavailable"}
assert manifest["essential_capsule_count"] == 20
assert manifest["local_source_capsule_count"] == 10
assert manifest["external_pinned_source_capsule_count"] == 10
assert manifest["rebuildable_capsule_count"] == 20
assert manifest["packaged_capsule_count"] == 20
assert manifest["expected_loaded_capsule_count"] == 20
assert len(manifest["capsule_archives"]) == 20
assert {record["class"] for record in manifest["capsule_archives"]} == {
    "local_repository_source", "external_pinned_source"
}
assert manifest["incremental_installed_code_bytes"] <= 20 * 1024 * 1024
assert manifest["authority"] == "release_build_manifest_not_appliance_state_or_astrid_memory"
assert manifest["self_evolution"]["mac_minime_bridge_scope"] == "excluded"
required_assets = {
    "scripts/edge_audio_feeder.py",
    "packaging/headless/edge-audio-feeder.json.in",
    "packaging/headless/edge-hindsight-writer.json.in",
    "packaging/systemd/astrid-edge-audio-feeder.service",
    "packaging/systemd/astrid-edge-audio-feeder.socket.in",
}
assert required_assets <= set(manifest["files_before_inventory"])
PY

generation_archive="$output_dir/astrid-edge-generation-test-x86_64-unknown-linux-gnu.tar.gz"
sha256sum -c "$generation_archive.sha256"
generation_extract="$test_root/generation"
install -d -m 0755 "$generation_extract"
tar -C "$generation_extract" -xzf "$generation_archive"
test -f "$generation_extract/astrid-edge-generation/.astrid-edge-generation.json"
python3 - "$generation_extract/astrid-edge-generation" <<'PY'
import json, pathlib, sys

root = pathlib.Path(sys.argv[1])
assert {path.name for path in root.iterdir()} == {
    ".astrid-edge-generation.json",
    "astrid",
    "astrid-build",
    "astrid-daemon",
    "astrid-edge-runtime",
    "capsules",
    "packaging",
    "scripts",
}
assert {path.name for path in (root / "scripts").iterdir()} == {
    "astrid_at_a_glance.py",
    "astrid_train.py",
    "edge_hindsight.py",
    "report_edge_activity.py",
    "report_edge_appliance.py",
    "report_edge_appliance.sh",
    "report_edge_fleet_activity.py",
    "retire_edge_origin_mac_affordance.py",
    "warm_ollama_model.sh",
}
assert len(tuple((root / "capsules").glob("*.capsule"))) == 20
for forbidden in (
    "astrid-edge-steward-helper",
    "astrid-edge-rescue-helper",
    "astrid-edge-web-broker",
    "astrid-edge-provider-broker",
    "astrid-edge-presentation-broker",
    "astrid-edge-checkpoint",
    "docs",
    "source-snapshot",
):
    assert not (root / forbidden).exists(), forbidden
manifest = json.loads((root / ".astrid-edge-generation.json").read_text(encoding="ascii"))
assert manifest["schema"] == "astrid.edge_self_change.initial_generation.v1"
assert manifest["appliance_id"] == "portable-bootstrap-non-authorizing"
assert manifest["authority"] == "operator_packaged_initial_generation_not_model_candidate"
assert {item["path"] for item in manifest["inventory"]} == {
    path.relative_to(root).as_posix()
    for path in root.rglob("*")
    if path.is_file() and path.name != ".astrid-edge-generation.json"
}
PY

printf 'CPU-edge package verification passed.\n'
