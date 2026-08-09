#!/usr/bin/env bash
# Read-only health and reservoir-fill report for a deployed CPU appliance.

set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
if command -v python3 >/dev/null 2>&1 \
    && [[ -r "$script_dir/report_edge_appliance.py" ]]; then
    exec python3 "$script_dir/report_edge_appliance.py" "$@"
fi

usage() {
    cat <<'EOF'
Usage: scripts/report_edge_appliance.sh [--window-minutes N] [--workspace PATH]

Reports service state, current lane provenance, and fill statistics for the
recent window. The default window is 20 minutes.
EOF
}

window_minutes=20
workspace_override=""
while (( $# > 0 )); do
    case "$1" in
        --window-minutes)
            if (( $# < 2 )) || [[ ! "$2" =~ ^[1-9][0-9]*$ ]]; then
                printf 'error: --window-minutes requires a positive integer\n' >&2
                exit 2
            fi
            window_minutes="$2"
            shift 2
            ;;
        --workspace)
            if (( $# < 2 )) || [[ "$2" != /* ]]; then
                printf 'error: --workspace requires an absolute path\n' >&2
                exit 2
            fi
            workspace_override="$2"
            shift 2
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
    printf 'error: HOME must be an absolute path\n' >&2
    exit 1
fi
for command_name in jq systemctl; do
    if ! command -v "$command_name" >/dev/null 2>&1; then
        printf 'error: required command is unavailable: %s\n' "$command_name" >&2
        exit 1
    fi
done

profile_path="${XDG_CONFIG_HOME:-$HOME/.config}/astrid/edge-appliance.env"
workspace="${workspace_override:-$HOME/.astrid/home/default/edge}"
state_path="$workspace/runtime/spectral_state.json"
history_path="$workspace/runtime/fill_history.jsonl"
autonomy_path="$workspace/autonomous/state.json"
runs_path="$workspace/autonomous/runs.jsonl"
actions_path="$workspace/actions/receipts.jsonl"

if [[ ! -r "$state_path" || ! -r "$history_path" ]]; then
    printf 'error: edge telemetry is not readable under %s\n' "$workspace/runtime" >&2
    exit 1
fi

instance_name="edge Astrid"
profile_value() {
    local name="$1"
    [[ -r "$profile_path" ]] || return 0
    awk -F= -v name="$name" '
        $1 == name {
            value = substr($0, index($0, "=") + 1)
            gsub(/^"|"$/, "", value)
            print value
            exit
        }
    ' "$profile_path"
}
if [[ -r "$profile_path" ]]; then
    configured_name="$(profile_value ASTRID_EDGE_INSTANCE_NAME)"
    [[ -n "$configured_name" ]] && instance_name="$configured_name"
fi

service_value() {
    local service="$1"
    local property="$2"
    systemctl --user show "$service" --property "$property" --value 2>/dev/null ||
        printf 'unknown\n'
}

printf 'report_version=7\n'
printf 'instance_name=%s\n' "$instance_name"
printf 'hostname=%s\n' "$(hostname)"
printf 'astrid_service_state=%s\n' "$(service_value astrid.service ActiveState)"
printf 'astrid_service_restarts=%s\n' "$(service_value astrid.service NRestarts)"
printf 'edge_service_state=%s\n' "$(service_value astrid-edge-runtime.service ActiveState)"
printf 'edge_service_restarts=%s\n' \
    "$(service_value astrid-edge-runtime.service NRestarts)"
printf 'model_warmup_service_state=%s\n' \
    "$(service_value astrid-model-warmup.service ActiveState)"
if command -v loginctl >/dev/null 2>&1; then
    printf 'user_linger=%s\n' \
        "$(loginctl show-user "${USER:-$(id -un)}" -p Linger --value 2>/dev/null || printf unknown)"
fi
printf 'selected_model=%s\n' "$(profile_value ASTRID_OLLAMA_MODEL)"
printf 'selected_model_status=%s\n' "$(profile_value ASTRID_OLLAMA_SELECTION_STATUS)"
printf 'selected_model_context=%s\n' "$(profile_value ASTRID_OLLAMA_CONTEXT)"
printf 'selected_model_max_output=%s\n' "$(profile_value ASTRID_OLLAMA_MAX_OUTPUT)"
printf 'autonomy_enabled=%s\n' "$(profile_value ASTRID_EDGE_AUTONOMY_ENABLED)"
printf 'autonomy_interval_minutes=%s\n' \
    "$(profile_value ASTRID_EDGE_AUTONOMY_INTERVAL_MINUTES)"
printf 'autonomy_follow_up_minutes=%s\n' \
    "$(profile_value ASTRID_EDGE_AUTONOMY_FOLLOW_UP_MINUTES)"
printf 'autonomy_max_chain_steps=%s\n' \
    "$(profile_value ASTRID_EDGE_AUTONOMY_MAX_CHAIN_STEPS)"
printf 'autonomy_quiet_minutes=%s\n' \
    "$(profile_value ASTRID_EDGE_AUTONOMY_QUIET_MINUTES)"
printf 'autonomy_max_turns_per_day=%s\n' \
    "$(profile_value ASTRID_EDGE_AUTONOMY_MAX_TURNS_PER_DAY)"
printf 'autonomy_initiative_profile=%s\n' \
    "$(profile_value ASTRID_EDGE_AUTONOMY_INITIATIVE_PROFILE)"
printf 'autonomy_prompt_profile=%s\n' "$(profile_value ASTRID_EDGE_AUTONOMY_PROMPT_PROFILE)"

jq -r '
    "current_recorded_at_unix_ms=\(.recorded_at_unix_ms)",
    "current_fill_pct=\(.fill_pct)",
    "target_fill_pct=\(.target_fill_pct)",
    "effective_dimensionality=\(.effective_dimensionality)",
    "audio_fresh=\(.audio_fresh)",
    "audio_source=\(.audio_source)",
    "aux_fresh=\(.aux_fresh)",
    "aux_source=\(.aux_source)",
    "video_fresh=\(.video_fresh)",
    "video_source=\(.video_source)",
    "semantic_fresh=\(.semantic_fresh)",
    "aux_feature_source=\(.aux_source // "unknown")",
    (
        (.aux_features // {}) | to_entries[] |
        "aux_feature_\(.key)_available=\(.value != null)",
        "aux_feature_\(.key)_value=\(.value // "unavailable")"
    )
' "$state_path"

now_ms="$(date +%s%3N)"
recorded_at_ms="$(jq -r '.recorded_at_unix_ms // 0' "$state_path")"
telemetry_age_ms="$(( now_ms - recorded_at_ms ))"
(( telemetry_age_ms < 0 )) && telemetry_age_ms=0
printf 'telemetry_age_ms=%s\n' "$telemetry_age_ms"
warmup_path="$workspace/runtime/model_warmup.json"
if [[ -r "$warmup_path" ]]; then
    jq -r '
        "model_warmup_status=\(.status // "unknown")",
        "model_warmup_model=\(.model // "unknown")",
        "model_warmup_elapsed_ms=\(.elapsed_ms // 0)",
        "model_warmup_completed_at_unix_ms=\(.completed_at_unix_ms // 0)"
    ' "$warmup_path"
fi
window_ms="$(( window_minutes * 60 * 1000 ))"
cutoff_ms="$(( now_ms - window_ms ))"
printf 'fill_window_minutes=%s\n' "$window_minutes"
summarize_fill() {
    local prefix="$1"
    awk '
        NR == 1 {
            minimum = $1
            maximum = $1
        }
        {
            count++
            sum += $1
            if ($1 < minimum) minimum = $1
            if ($1 > maximum) maximum = $1
            if ($1 >= 65 && $1 <= 72) preferred++
            if ($1 >= 65 && $1 <= 73.5) broad++
        }
        END {
            printf "%s_samples=%d\n", prefix, count
            if (count > 0) {
                printf "%s_min_pct=%.2f\n", prefix, minimum
                printf "%s_mean_pct=%.2f\n", prefix, sum / count
                printf "%s_max_pct=%.2f\n", prefix, maximum
                printf "%s_inside_65_72_pct=%.1f\n", prefix, 100 * preferred / count
                printf "%s_inside_65_73_5_pct=%.1f\n", prefix, 100 * broad / count
            }
        }
    ' prefix="$prefix"
}

jq -r --argjson cutoff "$cutoff_ms" \
    'select(.recorded_at_unix_ms >= $cutoff) | .fill_pct' "$history_path" |
    summarize_fill fill_all

printf 'fill_settled_after_seconds=30\n'
jq -r --argjson cutoff "$cutoff_ms" \
    'select(.recorded_at_unix_ms >= $cutoff and .t_ms >= 30000) | .fill_pct' \
    "$history_path" |
    summarize_fill fill_settled

if [[ -r "$autonomy_path" ]]; then
    jq -r --argjson now "$now_ms" '
        "autonomy_last_status=\(.last_status // "unknown")",
        "autonomy_attempts_today=\(.attempts_today // .turns_today // 0)",
        "autonomy_authored_turns_today=\(.authored_turns_today // 0)",
        "autonomy_transport_recoveries_today=\(.transport_recoveries_today // 0)",
        "autonomy_consecutive_failures=\(.consecutive_failures // 0)",
        "autonomy_consecutive_action_validation_failures=\(
            .consecutive_action_validation_failures // 0
        )",
        "autonomy_total_attempts=\(.total_attempts // .total_turns // 0)",
        "autonomy_total_authored_turns=\(.total_authored_turns // 0)",
        "autonomy_total_transport_recoveries=\(.total_transport_recoveries // 0)",
        "autonomy_ordinary_session_generation=\(.ordinary_session_generation // 1)",
        "autonomy_ordinary_session_authored_turns=\(.ordinary_session_authored_turns // 0)",
        "autonomy_chain_session_generation=\(.chain_session_generation // 1)",
        "autonomy_last_session_name=\(.last_session_name // "unknown")",
        "autonomy_last_prompt_chars=\(.last_prompt_chars // 0)",
        "autonomy_last_prompt_estimated_tokens=\(.last_prompt_estimated_tokens // 0)",
        "autonomy_last_turn_elapsed_ms=\(.last_turn_elapsed_ms // 0)",
        "autonomy_current_turn_age_ms=\(
            if .last_status == "running" and (.last_started_at_unix_ms // 0) > 0
            then [$now - .last_started_at_unix_ms, 0] | max
            else 0
            end
        )",
        "autonomy_active_chain_id=\(.active_chain_id // "none")",
        "autonomy_active_chain_step=\(.active_chain_step // 0)",
        "autonomy_next_due_at_unix_ms=\(.next_due_at_unix_ms // 0)"
    ' "$autonomy_path"
fi

if [[ -r "$runs_path" ]]; then
    jq -r --argjson cutoff "$cutoff_ms" '
        select((.completed_at_unix_ms // 0) >= $cutoff) | .status
    ' "$runs_path" |
        awk '
            $0 == "authored_completed" { authored++ }
            $0 == "transport_recovery" { recovery++ }
            $0 == "failed" { failed++ }
            $0 == "interrupted" { interrupted++ }
            END {
                printf "autonomy_window_authored_turns=%d\n", authored
                printf "autonomy_window_transport_recoveries=%d\n", recovery
                printf "autonomy_window_failed_turns=%d\n", failed
                printf "autonomy_window_interrupted_turns=%d\n", interrupted
            }
        '
fi

if [[ -r "$actions_path" ]]; then
    tail -n 5 "$actions_path" |
        jq -r '
            [
                (.recorded_at_unix_ms // 0 | tostring),
                (.decision_source // "unknown"),
                (.status // "unknown"),
                ((.declared_next // "none") | gsub("[\t\r\n]"; " ") | .[0:160]),
                ((.unexecuted_intention // "none") | gsub("[\t\r\n]"; " ") | .[0:160]),
                (.validation_reason // "none"),
                (.artifact_path // "none")
            ] | @tsv
        ' |
        awk -F '\t' '{
            printf "recent_action_%d_recorded_at_unix_ms=%s\n", NR, $1
            printf "recent_action_%d_decision_source=%s\n", NR, $2
            printf "recent_action_%d_status=%s\n", NR, $3
            printf "recent_action_%d_declaration=%s\n", NR, $4
            printf "recent_action_%d_unexecuted_intention=%s\n", NR, $5
            printf "recent_action_%d_validation_reason=%s\n", NR, $6
            printf "recent_action_%d_artifact=%s\n", NR, $7
        }'
fi

artifact_count() {
    local relative="$1"
    local path="$workspace/$relative"
    if [[ -d "$path" ]]; then
        find "$path" -maxdepth 1 -type f ! -lname '*' -print 2>/dev/null | wc -l | tr -d ' '
    else
        printf '0\n'
    fi
}

for artifact_directory in \
    journal memories introspections proposals notices daydreams aspirations research plans \
    workshop/drafts workshop/revisions workshop/checks inbox autonomous/turns \
    autonomous/recoveries; do
    artifact_key="${artifact_directory//\//_}"
    printf 'artifact_count_%s=%s\n' \
        "$artifact_key" "$(artifact_count "$artifact_directory")"
done

if [[ -d "$workspace/research" ]]; then
    printf 'artifact_count_research_sources=%s\n' \
        "$(find "$workspace/research" -maxdepth 1 -type f -name 'source_*.md' ! -lname '*' -print 2>/dev/null | wc -l | tr -d ' ')"
else
    printf 'artifact_count_research_sources=0\n'
fi

if [[ -r /proc/meminfo ]]; then
    awk '
        $1 == "MemTotal:" { total_kib = $2 }
        $1 == "MemAvailable:" { available_kib = $2 }
        $1 == "SwapTotal:" { swap_total_kib = $2 }
        $1 == "SwapFree:" { swap_free_kib = $2 }
        END {
            printf "memory_total_mib=%.0f\n", total_kib / 1024
            printf "memory_available_mib=%.0f\n", available_kib / 1024
            printf "swap_used_mib=%.0f\n", (swap_total_kib - swap_free_kib) / 1024
        }
    ' /proc/meminfo
fi
if [[ -r /proc/loadavg ]]; then
    awk '{ printf "load_1m=%s\nload_5m=%s\nload_15m=%s\n", $1, $2, $3 }' /proc/loadavg
fi
llama_cpu_pct="$(
    ps -eo pcpu=,args= 2>/dev/null |
        awk '/[/]llama-server / { sum += $1 } END { printf "%.1f", sum + 0 }'
)"
printf 'ollama_llama_server_cpu_pct=%s\n' "$llama_cpu_pct"

if command -v curl >/dev/null 2>&1; then
    ollama_ps="$(curl --fail --silent --show-error --max-time 2 \
        http://127.0.0.1:11434/api/ps 2>/dev/null || true)"
    if [[ -n "$ollama_ps" ]] && jq -e '.models | type == "array"' >/dev/null <<<"$ollama_ps"; then
        jq -r '
            "ollama_loaded_model_count=\(.models | length)",
            "ollama_loaded_models=\(
                .models | map(.name // .model // empty) | join(",")
            )",
            "ollama_loaded_size_mib=\(
                ([.models[]?.size // 0] | add // 0) / 1048576 | floor
            )"
        ' <<<"$ollama_ps"
    else
        printf 'ollama_loaded_model_count=unknown\n'
    fi
fi

log_directory="$HOME/.astrid/log"
shopt -s nullglob
astrid_logs=("$log_directory"/astrid.*.log)
shopt -u nullglob
if (( ${#astrid_logs[@]} > 0 )); then
    cutoff_iso="$(date --utc -d "@$(( cutoff_ms / 1000 ))" +%Y-%m-%dT%H:%M:%S)"
    awk -v cutoff="$cutoff_iso" '
            $1 < cutoff { next }
            /search_web/ { search_web++ }
            /fetch_url/ { fetch_url++ }
            /HTTP stream response headers received/ {
                header_events++
                if (match($0, /elapsed_ms=[0-9]+/)) {
                    value = substr($0, RSTART + 11, RLENGTH - 11)
                    if (value > header_max) header_max = value
                }
            }
            END {
                printf "recent_search_web_log_mentions=%d\n", search_web
                printf "recent_fetch_url_log_mentions=%d\n", fetch_url
                printf "recent_stream_header_events=%d\n", header_events
                printf "recent_stream_header_max_elapsed_ms=%d\n", header_max
            }
        ' "${astrid_logs[@]}"
fi
