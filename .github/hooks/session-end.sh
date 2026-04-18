#!/usr/bin/env bash
# Session-end marker (bash).
set -euo pipefail
log_dir="${HOME}/.copilot"
mkdir -p "$log_dir" || true
log_path="$log_dir/toaster-prompts.log"
ts=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

input="$(cat || true)"
reason=""
if [ -n "$input" ] && command -v jq >/dev/null 2>&1; then
  reason=$(printf '%s' "$input" | jq -r '.reason // ""')
fi
printf '%s session-end reason=%s\n' "$ts" "$reason" >> "$log_path" || true
exit 0
