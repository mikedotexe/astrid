#!/usr/bin/env bash
# Read-only suitability probe for appliance-class Astrid hosts.

set -euo pipefail

if [[ "$(uname -s)" != "Linux" ]]; then
    printf 'runtime_fit=unsupported\n'
    printf 'reason=this probe must run on Linux\n'
    exit 1
fi

command_exists() {
    command -v "$1" >/dev/null 2>&1
}

read_tool_version() {
    local tool="$1"
    if command_exists "$tool"; then
        "$tool" --version
    elif [[ -n "${HOME:-}" && -x "$HOME/.cargo/bin/$tool" ]]; then
        "$HOME/.cargo/bin/$tool" --version
    else
        printf 'absent\n'
    fi
}

read_mem_mib() {
    awk '/^MemTotal:/ { print int($2 / 1024); exit }' /proc/meminfo
}

read_mem_available_mib() {
    awk '/^MemAvailable:/ { print int($2 / 1024); exit }' /proc/meminfo
}

read_swap_mib() {
    awk '/^SwapTotal:/ { print int($2 / 1024); exit }' /proc/meminfo
}

read_cpu_model() {
    if command_exists lscpu; then
        lscpu | awk -F: '/^Model name:/ { sub(/^[[:space:]]+/, "", $2); print $2; exit }'
    else
        awk -F: '/^model name/ { sub(/^[[:space:]]+/, "", $2); print $2; exit }' /proc/cpuinfo
    fi
}

read_os_release_value() {
    local key="$1"
    awk -F= -v key="$key" '
        $1 == key {
            value = substr($0, index($0, "=") + 1)
            gsub(/^"|"$/, "", value)
            print value
            exit
        }
    ' /etc/os-release 2>/dev/null
}

read_cpu_features() {
    local flags
    local feature
    local found=()
    flags="$(
        awk -F: '
            /^(flags|Features)[[:space:]]*:/ {
                print " " tolower($2) " "
                exit
            }
        ' /proc/cpuinfo
    )"
    for feature in sse4_2 avx avx2 fma aes neon asimd dotprod; do
        if [[ "$flags" == *" $feature "* ]]; then
            found+=("$feature")
        fi
    done
    if (( ${#found[@]} == 0 )); then
        printf 'none_reported\n'
    else
        local joined
        joined="$(IFS=,; printf '%s' "${found[*]}")"
        printf '%s\n' "$joined"
    fi
}

read_network_interfaces() {
    local interface_path
    local interface
    local state
    local speed
    local entries=()
    for interface_path in /sys/class/net/*; do
        [[ -e "$interface_path" ]] || continue
        interface="${interface_path##*/}"
        [[ "$interface" == "lo" ]] && continue
        state="$(cat "$interface_path/operstate" 2>/dev/null || printf 'unknown')"
        speed="$(cat "$interface_path/speed" 2>/dev/null || printf 'unknown')"
        if [[ "$speed" =~ ^[0-9]+$ ]] && (( speed >= 0 )); then
            speed="${speed}Mbps"
        else
            speed="unknown"
        fi
        entries+=("${interface}:${state}:${speed}")
    done
    if (( ${#entries[@]} == 0 )); then
        printf 'none\n'
    else
        local joined
        joined="$(IFS=,; printf '%s' "${entries[*]}")"
        printf '%s\n' "$joined"
    fi
}

read_storage_devices() {
    if ! command_exists lsblk; then
        printf 'lsblk_unavailable\n'
        return
    fi
    lsblk -dn -o NAME,TYPE,SIZE,ROTA,TRAN,MODEL 2>/dev/null |
        awk '
            {
                line = $0
                gsub(/^[[:space:]]+|[[:space:]]+$/, "", line)
                gsub(/[[:space:]]+/, "_", line)
                if (line != "") {
                    if (seen++) {
                        printf ";"
                    }
                    printf "%s", line
                }
            }
            END {
                if (!seen) {
                    printf "none"
                }
                printf "\n"
            }
        '
}

read_physical_cores() {
    if command_exists lscpu; then
        lscpu -p=CORE,SOCKET 2>/dev/null |
            awk -F, '!/^#/ { seen[$1 ":" $2] = 1 } END { print length(seen) }'
    else
        printf 'unknown\n'
    fi
}

version_at_least() {
    local actual="$1"
    local minimum="$2"
    [[ "$(printf '%s\n%s\n' "$minimum" "$actual" | sort -V | head -n 1)" == "$minimum" ]]
}

architecture="$(uname -m)"
case "$architecture" in
    x86_64)
        release_target="x86_64-unknown-linux-gnu"
        ;;
    aarch64 | arm64)
        release_target="aarch64-unknown-linux-gnu"
        ;;
    *)
        release_target="unsupported"
        ;;
esac

logical_cpus="$(getconf _NPROCESSORS_ONLN 2>/dev/null || printf '1\n')"
physical_cores="$(read_physical_cores)"
memory_mib="$(read_mem_mib)"
memory_available_mib="$(read_mem_available_mib)"
swap_mib="$(read_swap_mib)"
root_filesystem="$(df -PmT "${HOME:-/}" | awk 'END { print $2 }')"
root_total_mib="$(df -PmT "${HOME:-/}" | awk 'END { print $3 }')"
root_available_mib="$(df -PmT "${HOME:-/}" | awk 'END { print $5 }')"
root_device="$(
    if command_exists findmnt; then
        findmnt -n -o SOURCE --target "${HOME:-/}" 2>/dev/null || printf 'unknown\n'
    else
        printf 'unknown\n'
    fi
)"
cpu_model="$(read_cpu_model)"
cpu_features="$(read_cpu_features)"
network_interfaces="$(read_network_interfaces)"
storage_devices="$(read_storage_devices)"
glibc_version="$(getconf GNU_LIBC_VERSION 2>/dev/null | awk '{ print $2 }' || true)"
init_name="$(ps -p 1 -o comm= | tr -d '[:space:]')"
os_id="$(read_os_release_value ID)"
os_version_id="$(read_os_release_value VERSION_ID)"
kernel_release="$(uname -r)"

if command_exists systemd-detect-virt; then
    virtualization="$(systemd-detect-virt 2>/dev/null || true)"
    virtualization="${virtualization:-none}"
else
    virtualization="unknown"
fi

if [[ -r /proc/asound/cards ]]; then
    audio_cards="$(
        awk '/^[[:space:]]*[0-9]+[[:space:]]+\[/ { count++ } END { print count + 0 }' \
            /proc/asound/cards
    )"
else
    audio_cards=0
fi

if command_exists systemctl; then
    systemd_version="$(systemctl --version | awk 'NR == 1 { print $2 }')"
    if systemctl --user show-environment >/dev/null 2>&1; then
        systemd_user="available"
    else
        systemd_user="installed_no_user_session"
    fi
else
    systemd_version="absent"
    systemd_user="unavailable"
fi

recommended_runtime_workers="$logical_cpus"
if (( memory_mib >= 8192 )); then
    recommended_build_jobs="$logical_cpus"
elif (( logical_cpus >= 2 )); then
    recommended_build_jobs=2
else
    recommended_build_jobs=1
fi

runtime_fit="ready"
if [[ "$release_target" == "unsupported" ]]; then
    runtime_fit="unsupported_architecture"
elif (( memory_mib < 2048 )); then
    runtime_fit="insufficient_memory"
elif (( memory_mib < 4096 || root_available_mib < 2048 )); then
    runtime_fit="constrained"
fi

if (( memory_mib >= 7168 && root_available_mib >= 8192 )); then
    install_strategy="source_build_with_jobs_${recommended_build_jobs}_or_prebuilt_release"
else
    install_strategy="prebuilt_release_preferred"
fi

if (( root_available_mib < 8192 )); then
    local_model_candidate="off_box_until_storage_is_freed"
    recommended_model_context=0
elif (( memory_mib >= 14336 && logical_cpus >= 4 )); then
    local_model_candidate="benchmark_qwen3.5_4b_q4_class"
    recommended_model_context=8192
elif (( memory_mib >= 7168 )); then
    local_model_candidate="benchmark_2b_q4_class_or_off_box"
    recommended_model_context=4096
elif (( memory_mib >= 4096 )); then
    local_model_candidate="sub_2b_q4_or_off_box"
    recommended_model_context=2048
else
    local_model_candidate="off_box"
    recommended_model_context=0
fi

release_abi="unknown"
if [[ -n "$glibc_version" ]]; then
    if version_at_least "$glibc_version" "2.35"; then
        release_abi="compatible_with_published_linux_baseline"
    else
        release_abi="older_than_published_linux_baseline_build_on_host"
    fi
fi

printf 'astrid_probe_version=2\n'
printf 'hostname=%s\n' "$(hostname)"
printf 'os_id=%s\n' "${os_id:-unknown}"
printf 'os_version_id=%s\n' "${os_version_id:-unknown}"
printf 'kernel_release=%s\n' "$kernel_release"
printf 'architecture=%s\n' "$architecture"
printf 'release_target=%s\n' "$release_target"
printf 'cpu_model=%s\n' "${cpu_model:-unknown}"
printf 'cpu_features=%s\n' "$cpu_features"
printf 'physical_cores=%s\n' "$physical_cores"
printf 'logical_cpus=%s\n' "$logical_cpus"
printf 'memory_mib=%s\n' "$memory_mib"
printf 'memory_available_mib=%s\n' "${memory_available_mib:-unknown}"
printf 'swap_mib=%s\n' "${swap_mib:-0}"
printf 'root_device=%s\n' "$root_device"
printf 'root_filesystem=%s\n' "$root_filesystem"
printf 'root_total_mib=%s\n' "$root_total_mib"
printf 'root_available_mib=%s\n' "$root_available_mib"
printf 'storage_devices=%s\n' "$storage_devices"
printf 'network_interfaces=%s\n' "$network_interfaces"
printf 'audio_cards=%s\n' "$audio_cards"
printf 'virtualization=%s\n' "$virtualization"
printf 'glibc_version=%s\n' "${glibc_version:-unknown}"
printf 'release_abi=%s\n' "$release_abi"
printf 'init=%s\n' "${init_name:-unknown}"
printf 'systemd_version=%s\n' "$systemd_version"
printf 'systemd_user=%s\n' "$systemd_user"
printf 'rustc=%s\n' "$(read_tool_version rustc)"
printf 'cargo=%s\n' "$(read_tool_version cargo)"
printf 'recommended_cargo_build_jobs=%s\n' "$recommended_build_jobs"
printf 'recommended_tokio_worker_threads=%s\n' "$recommended_runtime_workers"
printf 'recommended_service_nofile=65536\n'
printf 'recommended_service_tasks_max=1024\n'
printf 'recommended_reservoir_fill_target=0.68\n'
printf 'recommended_ollama_num_parallel=1\n'
printf 'recommended_ollama_max_loaded_models=1\n'
printf 'local_model_candidate=%s\n' "$local_model_candidate"
printf 'recommended_model_context=%s\n' "$recommended_model_context"
printf 'model_selection_gate=on_device_benchmark_required\n'
printf 'install_strategy=%s\n' "$install_strategy"
printf 'runtime_fit=%s\n' "$runtime_fit"
printf 'gpu_required=no\n'
