#!/usr/bin/env bash
# userPromptSubmitted audit log (bash). Output ignored by CLI.
set -euo pipefail

log_dir="${HOME}/.copilot"
mkdir -p "$log_dir" || true
log_path="$log_dir/toaster-prompts.log"
ts=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

input="$(cat || true)"
if [ -z "$input" ]; then exit 0; fi

prompt=""
if command -v jq >/dev/null 2>&1; then
  prompt=$(printf '%s' "$input" | jq -r '.prompt // ""')
fi
[ -n "$prompt" ] || exit 0

flat=$(printf '%s' "$prompt" | tr '\n\r' '  ')
printf '%s %s\n' "$ts" "$flat" >> "$log_path" || true

for t in "launch toaster" "stop toaster" "run evals"; do
  if printf '%s' "$prompt" | grep -qi "$t"; then
    printf '# trigger-phrase: %s\n' "$t" >> "$log_path" || true
  fi
done

exit 0
