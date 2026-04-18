#!/usr/bin/env bash
# Session-start hook (bash).
set -euo pipefail

warn() { printf '%s\n' "$*" >&2; }

log_dir="${HOME}/.copilot"
mkdir -p "$log_dir" || true
log_path="$log_dir/toaster-prompts.log"

ts=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

input=""
if [ -t 0 ]; then :; else input="$(cat || true)"; fi

cwd=""; src=""; prompt=""
if [ -n "$input" ] && command -v jq >/dev/null 2>&1; then
  cwd=$(printf '%s' "$input"  | jq -r '.cwd // ""')
  src=$(printf '%s' "$input"  | jq -r '.source // ""')
  prompt=$(printf '%s' "$input" | jq -r '.initialPrompt // ""')
fi

printf '%s session-start source=%s cwd=%s\n' "$ts" "$src" "$cwd" >> "$log_path" || true
if [ -n "$prompt" ]; then
  printf '# initialPrompt: %s\n' "$prompt" >> "$log_path" || true
fi

if printf '%s' "$prompt" | grep -qi 'launch toaster'; then
  warn "Recognized 'launch toaster' - monitored launcher is .\\scripts\\launch-toaster-monitored.ps1 -ObservationSeconds 120."
fi

exit 0
