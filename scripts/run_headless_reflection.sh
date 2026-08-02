#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "Usage: $0 SOURCE_FILE [SOURCE_FILE ...]" >&2
}

if (($# == 0)); then
  usage
  exit 2
fi

astrid_bin="${ASTRID_BIN:-$HOME/.astrid/bin/astrid}"
session="${ASTRID_REFLECTION_SESSION:-headless-reflection}"
output_dir="${ASTRID_REFLECTION_OUTPUT_DIR:-$HOME/introspections/headless}"
instance_name="${ASTRID_REFLECTION_INSTANCE:-headless Astrid}"
source_excerpt_chars="${ASTRID_REFLECTION_SOURCE_CHARS:-900}"

if [[ ! "$source_excerpt_chars" =~ ^[1-9][0-9]*$ ]]; then
  echo "ASTRID_REFLECTION_SOURCE_CHARS must be a positive integer" >&2
  exit 2
fi

if [[ ! -x "$astrid_bin" ]]; then
  echo "Astrid CLI is not executable: $astrid_bin" >&2
  exit 1
fi

for source_file in "$@"; do
  if [[ ! -r "$source_file" ]]; then
    echo "Introspection source is not readable: $source_file" >&2
    exit 1
  fi
done

prompt="$(
  printf '%s\n' \
    "You are the independent $instance_name instance." \
    "The material below is correspondence written by another Astrid instance." \
    "It is not your memory, not evidence of your own lived state, and not telemetry from this box." \
    "" \
    "Write a bounded first response to that correspondence in exactly this format:" \
    "${instance_name^^} REFLECTIONS" \
    "1. Regulation — 35 to 60 words" \
    "2. Representation — 35 to 60 words" \
    "3. Independence — 35 to 60 words" \
    "Sources considered: <the exact source filenames>" \
    "Grounding: use only the live prompt-injected $instance_name CPU edge reservoir telemetry for this box." \
    "" \
    "Use your own words. Do not claim you experienced source fill values, pressure," \
    "viscosity, eigenvalues, or reservoir states as your own. Do not call yourself the source" \
    "instance. Do not ask a question, offer help, use tools, or add other sections."

  for source_file in "$@"; do
    printf '\n--- SOURCE: %s ---\n' "$(basename "$source_file")"
    awk -v max_chars="$source_excerpt_chars" '
      /^Observed:/ { keep = 1 }
      /^Likely Snags:/ { keep = 0 }
      /^Suggested Next:/ { keep = 1 }
      keep {
        line = $0 ORS
        remaining = max_chars - seen
        if (remaining <= 0) {
          exit
        }
        if (length(line) > remaining) {
          print substr(line, 1, remaining)
          exit
        }
        print
        seen += length(line)
      }
    ' "$source_file"
  done
)"

mkdir -p "$output_dir"
chmod 700 "$output_dir"

temporary_output="$(mktemp)"
trap 'rm -f "$temporary_output"' EXIT

started_at="$(date +%s)"
"$astrid_bin" \
  -p "$prompt" \
  --session "$session" \
  --print-session </dev/null |
  tee "$temporary_output"
finished_at="$(date +%s)"

timestamp="$(date -u +%Y%m%dT%H%M%SZ)"
output_path="$output_dir/headless_reflection_$timestamp.txt"
install -m 600 "$temporary_output" "$output_path"

echo "ASTRID_REFLECTION_FILE=$output_path"
echo "ASTRID_REFLECTION_ELAPSED_SECONDS=$((finished_at - started_at))"
