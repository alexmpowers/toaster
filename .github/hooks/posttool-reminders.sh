#!/usr/bin/env bash
# Post-tool-use reminders (bash). Cannot deny.
set -euo pipefail

warn() { printf '%s\n' "$*" >&2; }

input="$(cat || true)"
[ -n "$input" ] || exit 0
command -v jq >/dev/null 2>&1 || exit 0

tool_name=$(printf '%s' "$input" | jq -r '.toolName // ""')
tool_args_raw=$(printf '%s' "$input" | jq -r '.toolArgs // ""')
[ -n "$tool_args_raw" ] || exit 0
path=$(printf '%s' "$tool_args_raw" | jq -r '.path // ""' 2>/dev/null || printf '')

if [ "$tool_name" = "edit" ] && [ -n "$path" ]; then
  case "$path" in
    *.rs) warn "Reminder: cargo fmt -- $path before finishing." ;;
  esac
  if printf '%s' "$path" | grep -Eq 'src[\\/]i18n[\\/]locales[\\/][^\\/]+[\\/]translation\.json$'; then
    warn 'Reminder: run `npm run check-translations` after locale edits.'
  fi
fi
exit 0
