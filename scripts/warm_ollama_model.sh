#!/usr/bin/env bash
# Load the appliance-selected Ollama model before Astrid autonomy starts.

set -euo pipefail

model="${ASTRID_OLLAMA_MODEL:-}"
keep_alive="${ASTRID_OLLAMA_KEEP_ALIVE:-2h}"
base_url="${ASTRID_OLLAMA_BASE_URL:-http://127.0.0.1:11434}"
workspace="${ASTRID_EDGE_WORKSPACE:-.astrid/home/default/edge}"

if [[ ! "$model" =~ ^[A-Za-z0-9._:/-]+$ ]]; then
    printf 'error: ASTRID_OLLAMA_MODEL is missing or invalid\n' >&2
    exit 2
fi
if [[ ! "$keep_alive" =~ ^[1-9][0-9]*[hm]$ ]]; then
    printf 'error: ASTRID_OLLAMA_KEEP_ALIVE must be a positive hour/minute duration\n' >&2
    exit 2
fi
if [[ "$workspace" != /* ]]; then
    workspace="$PWD/$workspace"
fi
mkdir -p "$workspace/runtime"

started_at_ms="$(( $(date +%s) * 1000 ))"
deadline="$(( $(date +%s) + 120 ))"
until curl --fail --silent --show-error --max-time 2 "$base_url/api/version" >/dev/null 2>&1; do
    if (( $(date +%s) >= deadline )); then
        printf 'error: Ollama did not become ready at %s\n' "$base_url" >&2
        exit 1
    fi
    sleep 1
done

payload="$(
    printf '{"model":"%s","prompt":"","stream":false,"keep_alive":"%s"}' \
        "$model" "$keep_alive"
)"
curl --fail --silent --show-error --max-time 600 \
    -H 'Content-Type: application/json' \
    --data-binary "$payload" \
    "$base_url/api/generate" >/dev/null

completed_at_ms="$(( $(date +%s) * 1000 ))"
temporary="$workspace/runtime/model_warmup.json.tmp"
destination="$workspace/runtime/model_warmup.json"
printf '{\n  "schema": "astrid_edge_model_warmup_v1",\n  "model": "%s",\n  "status": "loaded",\n  "started_at_unix_ms": %s,\n  "completed_at_unix_ms": %s,\n  "elapsed_ms": %s,\n  "keep_alive": "%s"\n}\n' \
    "$model" \
    "$started_at_ms" \
    "$completed_at_ms" \
    "$(( completed_at_ms - started_at_ms ))" \
    "$keep_alive" >"$temporary"
mv "$temporary" "$destination"
