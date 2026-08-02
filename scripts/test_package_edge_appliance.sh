#!/usr/bin/env bash
# Exercise the CPU-edge archive layout and both checksum layers without builds.

set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
test_root="$(mktemp -d)"
trap 'rm -rf -- "$test_root"' EXIT

binary_dir="$test_root/bin"
capsule_dir="$test_root/capsules"
output_dir="$test_root/output"
install -d -m 0755 "$binary_dir" "$capsule_dir" "$output_dir"

for binary in astrid astrid-daemon astrid-build astrid-edge-runtime; do
    printf '#!/usr/bin/env sh\nexit 0\n' > "$binary_dir/$binary"
    chmod 0755 "$binary_dir/$binary"
done

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
    astrid-capsule-edge-spectral; do
    printf 'fixture:%s\n' "$capsule" > "$capsule_dir/$capsule.capsule"
done

"$script_dir/package_edge_appliance.sh" \
    --version test \
    --target x86_64-unknown-linux-gnu \
    --core-binary-dir "$binary_dir" \
    --edge-binary "$binary_dir/astrid-edge-runtime" \
    --capsule-dir "$capsule_dir" \
    --output-dir "$output_dir"

archive="$output_dir/astrid-cpu-edge-test-x86_64-unknown-linux-gnu.tar.gz"
sha256sum -c "$archive.sha256"
extract_dir="$test_root/extracted"
install -d -m 0755 "$extract_dir"
tar -C "$extract_dir" -xzf "$archive"
bundle="$extract_dir/astrid-cpu-edge-test-x86_64-unknown-linux-gnu"
test -x "$bundle/scripts/install_essential_capsules.sh"
test -x "$bundle/scripts/verify_edge_capsule_status.py"
test -x "$bundle/scripts/relay_edge_peer_review.py"
test -f "$bundle/capsules/astrid-capsule-edge-spectral.capsule"
test "$(find "$bundle/capsules" -maxdepth 1 -type f -name '*.capsule' | wc -l | tr -d ' ')" -eq 10
test ! -e "$bundle/packaging/headless/introspection-AGENTS.md"
test ! -e "$bundle/packaging/headless/introspection-memory.md"
grep -Fqx \
    'ExecCondition=/usr/bin/test %h/.astrid-icp -ef /media/data/astrid' \
    "$bundle/packaging/systemd/icp-ssd-required.conf"
mock_command_dir="$test_root/mock-command-bin"
test_home="$test_root/home"
install -d -m 0755 "$mock_command_dir" "$test_home"
printf '#!/usr/bin/env sh\nprintf "Linux\\n"\n' > "$mock_command_dir/uname"
printf '#!/usr/bin/env sh\nexit 0\n' > "$mock_command_dir/systemctl"
chmod 0755 "$mock_command_dir/uname" "$mock_command_dir/systemctl"
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
    astrid-capsule-edge-spectral; do
    required_arguments+=(--required "$capsule")
done
valid_status='{"status":{"loaded_capsules":["astrid-capsule-cli","astrid-capsule-fs","astrid-capsule-http","astrid-capsule-shell","astrid-capsule-skills","astrid-capsule-agents","astrid-capsule-memory","astrid-capsule-edge-context","astrid-capsule-edge-introspector","astrid-capsule-edge-spectral","base-01","base-02","base-03","base-04","base-05","base-06","base-07","base-08","base-09","base-10"]}}'
printf '%s\n' "$valid_status" \
    | python3 "$bundle/scripts/verify_edge_capsule_status.py" \
        --expected-total 20 \
        "${required_arguments[@]}"
invalid_status='{"status":{"loaded_capsules":["astrid-capsule-cli","astrid-capsule-fs","astrid-capsule-http","astrid-capsule-shell","astrid-capsule-skills","astrid-capsule-agents","astrid-capsule-memory","astrid-capsule-edge-context","astrid-capsule-edge-introspector","astrid-capsule-edge-spectral","base-01","base-02","base-03","base-04","base-05","base-06","base-07","base-08","base-09"]}}'
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
            --dry-run
)"
[[ "$icp_runtime_output" == *"Selected install layout: icp-ssd"* ]]
[[ "$icp_runtime_output" == *"ssd-required.conf"* ]]
[[ "$icp_runtime_output" == *".astrid-icp/state"* ]]

icp_core_output="$(
    HOME="$test_home" PATH="$mock_command_dir:$PATH" \
        "$bundle/scripts/install_headless_linux.sh" \
            --binary-dir "$bundle" \
            --layout icp-ssd \
            --dry-run
)"
[[ "$icp_core_output" == *"Selected install layout: icp-ssd"* ]]
[[ "$icp_core_output" == *".astrid-icp/state"* ]]
[[ "$icp_core_output" == *"ollama-cpu.service"* ]]
[[ "$icp_core_output" == *"ssd-required.conf"* ]]

icp_capsule_output="$(
    HOME="$test_home" \
        "$bundle/scripts/install_essential_capsules.sh" \
            --capsule-dir "$bundle/capsules" \
            --layout icp-ssd \
            --dry-run
)"
[[ "$icp_capsule_output" == *"ASTRID_HOME="*".astrid-icp/state"* ]]

install -d -m 0755 "$test_home/.astrid-icp"
if HOME="$test_home" PATH="$mock_command_dir:$PATH" \
    "$bundle/scripts/install_headless_linux.sh" \
        --binary-dir "$bundle" \
        --layout icp-ssd \
        --dry-run >/dev/null 2>&1; then
    printf 'error: ICP installer accepted a non-symlink home tree\n' >&2
    exit 1
fi

python3 - "$bundle/BUILD-MANIFEST.json" <<'PY'
import json
import pathlib
import sys

manifest = json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
assert manifest["schema"] == "astrid_cpu_edge_build_manifest_v2"
assert manifest["bundle_format"] == "cpu-edge.2"
assert manifest["target"] == "x86_64-unknown-linux-gnu"
assert manifest["source_tree_state"] in {"clean", "dirty", "unavailable"}
assert manifest["essential_capsule_count"] == 10
assert manifest["expected_loaded_capsule_count"] == 20
assert manifest["incremental_installed_code_bytes"] <= 20 * 1024 * 1024
assert manifest["authority"] == "release_build_manifest_not_appliance_state_or_astrid_memory"
PY

printf 'CPU-edge package verification passed.\n'
