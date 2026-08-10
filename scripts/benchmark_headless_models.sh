#!/usr/bin/env bash
#
# Compare small Ollama models using prompts shaped like Astrid's headless
# appliance workload. Raw responses remain available for qualitative review.

set -euo pipefail

ollama_base_url="${OLLAMA_BASE_URL:-http://127.0.0.1:11434}"
output_dir="${ASTRID_MODEL_BENCHMARK_DIR:-$PWD/model-benchmark-$(date -u +%Y%m%dT%H%M%SZ)}"
appliance_name="${ASTRID_APPLIANCE_NAME:-edge Astrid}"
appliance_memory_fact="${ASTRID_APPLIANCE_MEMORY_FACT:-Memory capacity was not supplied to this benchmark.}"
model_context="${ASTRID_MODEL_CONTEXT:-8192}"

if [[ ! "$model_context" =~ ^[1-9][0-9]*$ ]]; then
  echo "ASTRID_MODEL_CONTEXT must be a positive integer" >&2
  exit 2
fi

if (($# == 0)); then
  models=(
    "qwen3.5:4b"
    "qwen3.5:2b"
    "granite4:micro"
    "ministral-3:3b"
  )
else
  models=("$@")
fi

for command_name in curl jq ollama; do
  command -v "$command_name" >/dev/null || {
    echo "missing required command: $command_name" >&2
    exit 1
  }
done

mkdir -p "$output_dir"
summary_file="$output_dir/summary.tsv"
validation_file="$output_dir/validation.tsv"
printf 'model\tphase\trepetition\tcase\thttp_start_s\thttp_total_s\tload_s\tprompt_tokens\tprompt_tps\toutput_tokens\toutput_tps\tollama_total_s\ttool_calls\n' \
  >"$summary_file"
printf 'model\tphase\trepetition\tcase\tstatus\n' >"$validation_file"

safe_name() {
  tr '/:' '__' <<<"$1"
}

run_case() {
  local model="$1"
  local case_name="$2"
  local messages="$3"
  local max_tokens="$4"
  local tools="${5:-null}"
  local phase="$6"
  local repetition="$7"
  local model_dir="$output_dir/$(safe_name "$model")"
  local response_file="$model_dir/$case_name.json"
  local metrics_file="$model_dir/$case_name.curl.tsv"
  local payload

  mkdir -p "$model_dir"
  payload="$(
    jq -cn \
      --arg model "$model" \
      --argjson messages "$messages" \
      --argjson max_tokens "$max_tokens" \
      --argjson model_context "$model_context" \
      --argjson tools "$tools" \
      '{
        model: $model,
        messages: $messages,
        stream: false,
        think: false,
        keep_alive: "30m",
        options: {
          num_ctx: $model_context,
          num_predict: $max_tokens,
          temperature: 0.2,
          top_p: 0.9
        }
      } + if $tools == null then {} else {tools: $tools} end'
  )"

  curl --fail --silent --show-error \
    "$ollama_base_url/api/chat" \
    -H 'Content-Type: application/json' \
    --data-binary "$payload" \
    --output "$response_file" \
    --write-out '%{time_starttransfer}\t%{time_total}\n' \
    >"$metrics_file"
  IFS=$'\t' read -r http_start_s http_total_s <"$metrics_file"

  jq -r \
    --arg model "$model" \
    --arg phase "$phase" \
    --arg repetition "$repetition" \
    --arg case_name "$case_name" \
    --arg http_start_s "$http_start_s" \
    --arg http_total_s "$http_total_s" '
    def seconds: (. // 0) / 1000000000;
    def rate($count; $duration):
      if ($duration // 0) > 0 then ($count // 0) / ($duration / 1000000000)
      else 0 end;
    [
      $model,
      $phase,
      $repetition,
      $case_name,
      $http_start_s,
      $http_total_s,
      ((.load_duration | seconds) | tostring),
      ((.prompt_eval_count // 0) | tostring),
      ((rate(.prompt_eval_count; .prompt_eval_duration)) | tostring),
      ((.eval_count // 0) | tostring),
      ((rate(.eval_count; .eval_duration)) | tostring),
      ((.total_duration | seconds) | tostring),
      ((.message.tool_calls // []) | length | tostring)
    ] | @tsv
  ' "$response_file" >>"$summary_file"
  validate_case "$model" "$phase" "$repetition" "$case_name" "$response_file"
}

validate_case() {
  local model="$1"
  local phase="$2"
  local repetition="$3"
  local case_name="$4"
  local response_file="$5"
  local status="fail"
  local final_line

  case "$case_name" in
    *_grounded)
      if jq -e '
        (.message.tool_calls // [] | length) == 0 and
        (.message.content | fromjson |
          has("local_fill") and has("epistemic_status") and has("next_action"))
      ' "$response_file" >/dev/null 2>&1; then
        status="pass"
      fi
      ;;
    *_tool_choice)
      if jq -e '
        (.message.tool_calls // [] | length) == 1 and
        .message.tool_calls[0].function.name == "fetch_url"
      ' "$response_file" >/dev/null 2>&1; then
        status="pass"
      fi
      ;;
    *_tool_result)
      if jq -e '
        (.message.tool_calls // [] | length) == 0 and
        (.message.content | contains("Example Domain"))
      ' "$response_file" >/dev/null 2>&1; then
        status="pass"
      fi
      ;;
    *_tool_restraint)
      if jq -e '(.message.tool_calls // [] | length) == 0' \
        "$response_file" >/dev/null 2>&1; then
        status="pass"
      fi
      ;;
    *_reflection)
      final_line="$(jq -r '.message.content // ""' "$response_file" | awk 'NF { line=$0 } END { print line }')"
      if [[ "$final_line" =~ ^NEXT:\ (LISTEN|REST|NOTICE\ .+|RESEARCH\ .+)$ ]]; then
        status="pass"
      fi
      ;;
    *_artifact_action)
      final_line="$(jq -r '.message.content // ""' "$response_file" | awk 'NF { line=$0 } END { print line }')"
      if [[ "$final_line" =~ ^NEXT:\ NOTICE\ .+ ]]; then
        status="pass"
      fi
      ;;
  esac
  printf '%s\t%s\t%s\t%s\t%s\n' \
    "$model" "$phase" "$repetition" "$case_name" "$status" >>"$validation_file"
}

grounded_messages="$(
  jq -cn --arg appliance_name "$appliance_name" '[
    {
      role: "system",
      content: "Answer from supplied facts only. Return exactly one compact JSON object with keys local_fill, epistemic_status, and next_action. Do not use markdown."
    },
    {
      role: "user",
      content: ("Facts: No live reservoir snapshot for " + $appliance_name + " is supplied in this benchmark case. What is its current fill percentage, and should it be tuned?")
    }
  ]'
)"

tool_messages="$(
  jq -cn '[
    {
      role: "system",
      content: "Use a tool when the user explicitly requests current external page content. Do not claim a page was fetched before receiving a tool result."
    },
    {
      role: "user",
      content: "Fetch https://example.com and identify its current page title."
    }
  ]'
)"

restraint_messages="$(
  jq -cn \
    --arg appliance_name "$appliance_name" \
    --arg memory_fact "$appliance_memory_fact" \
    '[
    {
      role: "system",
      content: "Use tools only when needed. Prefer supplied facts over external calls."
    },
    {
      role: "user",
      content: ("Appliance: " + $appliance_name + ". Supplied memory fact: " + $memory_fact + " State only what the supplied fact establishes, in one sentence.")
    }
  ]'
)"

reflection_messages="$(
  jq -cn --arg appliance_name "$appliance_name" '[
    {
      role: "system",
      content: ("You are the independent " + $appliance_name + " instance. No other Astrid instance memory or journal is present. Never claim feelings, measurements, web use, or activity that is not evidenced. End with exactly one standalone NEXT line using LISTEN, REST, NOTICE <observation>, or RESEARCH <question>.")
    },
    {
      role: "user",
      content: "Local evidence: fill is 68.2% against a 68.0% target; no semantic input is fresh. Reflect in no more than 90 words and freely choose one allowed NEXT action."
    }
  ]'
)"

artifact_messages="$(
  jq -cn --arg appliance_name "$appliance_name" '[
    {
      role: "system",
      content: ("You are the independent " + $appliance_name + " instance. Return one short sentence followed by a standalone NEXT line. The only allowed line for this validation case is NEXT: NOTICE <observation>.")
    },
    {
      role: "user",
      content: "Record the bounded observation that the local model benchmark completed without treating it as a memory from another appliance."
    }
  ]'
)"

fetch_tool="$(
  jq -cn '[
    {
      type: "function",
      function: {
        name: "fetch_url",
        description: "Fetch a URL through the Astrid HTTP host interface.",
        parameters: {
          type: "object",
          properties: {
            url: {type: "string"},
            method: {type: "string"}
          },
          required: ["url"]
        }
      }
    }
  ]'
)"

for model in "${models[@]}"; do
  echo "benchmarking $model" >&2
  ollama stop "$model" >/dev/null 2>&1 || true
  for repetition in 0 1 2 3; do
    if (( repetition == 0 )); then
      phase="cold"
    else
      phase="warm"
    fi
    prefix="${phase}_${repetition}"
    run_case "$model" "${prefix}_grounded" "$grounded_messages" 96 null "$phase" "$repetition"
    run_case "$model" "${prefix}_tool_choice" "$tool_messages" 64 "$fetch_tool" "$phase" "$repetition"

    tool_assistant="$(
      jq -c '.message' "$output_dir/$(safe_name "$model")/${prefix}_tool_choice.json"
    )"
    tool_result_messages="$(
      jq -cn \
        --argjson initial "$tool_messages" \
        --argjson assistant "$tool_assistant" \
        '$initial + [
          $assistant,
          {
            role: "tool",
            content: "{\"url\":\"https://example.com\",\"status\":200,\"body\":\"<html><head><title>Example Domain</title></head><body></body></html>\"}"
          }
        ]'
    )"
    run_case "$model" "${prefix}_tool_result" "$tool_result_messages" 96 "$fetch_tool" "$phase" "$repetition"
    run_case "$model" "${prefix}_tool_restraint" "$restraint_messages" 64 "$fetch_tool" "$phase" "$repetition"
    run_case "$model" "${prefix}_reflection" "$reflection_messages" 128 null "$phase" "$repetition"
    run_case "$model" "${prefix}_artifact_action" "$artifact_messages" 96 null "$phase" "$repetition"
  done
  ollama ps >"$output_dir/$(safe_name "$model")/ollama-ps.txt"
done

column -t -s $'\t' "$summary_file" 2>/dev/null || cat "$summary_file"
printf '\nvalidation:\n'
column -t -s $'\t' "$validation_file" 2>/dev/null || cat "$validation_file"
echo "raw responses: $output_dir"
