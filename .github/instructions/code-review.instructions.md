---
name: Code review rules
description: Toaster-specific architecture boundaries and verification requirements for code review
applyTo: "**"
excludeAgent: "cloud-agent"
---

# Toaster Review Addendum

This file is a **delta** on top of `superpowers:code-reviewer`. When reviewing
a Toaster PR, also apply the rules below. Flag any violation as **Critical**.

## Architecture boundaries (Critical if violated)

| Boundary | Rule |
|---|---|
| Business logic location | Must live in `src-tauri/src/managers/`, not in frontend or `commands/` |
| Command layer | `src-tauri/src/commands/` are thin IPC wrappers — no business logic |
| Frontend role | Calls Tauri commands and renders state — no independent timeline or keep-segment logic |
| Timeline authority | Backend keep-segment / time-mapping is the single source of truth |
| Video element source | Never swapped to an audio preview file |
| Timestamps | Microseconds, matching FFmpeg `AV_TIME_BASE` |
| UI text | i18next keys, not hardcoded strings |
| Dual-path logic | Caption layout, word grouping, filler lists, keep-segments, time mapping — one backend implementation consumed by preview and export verbatim |
| Network | Local-only inference. No runtime calls to hosted LLM / ASR / caption APIs |

## Verification (Critical if missing)

The PR must show evidence — not claims — that the following gates ran:

- [ ] `cargo test` (if backend touched) with output captured
- [ ] `cargo clippy` (if backend touched) clean
- [ ] `npm run lint` (if frontend touched) clean
- [ ] `bun scripts/check-translations.ts` (if i18next keys touched)
- [ ] For audio / timeline / export changes: the precision eval
  ([eval-edit-quality.ps1](../../scripts/eval/eval-edit-quality.ps1)) AND boundary eval
  ([eval-audio-boundary.ps1](../../scripts/eval/eval-audio-boundary.ps1)) ran green, OR the relevant
  `cargo test` fixture ran green.
- [ ] For UI or playback-path changes: live-app check via
  [launch-toaster-monitored.ps1](../../scripts/launch-toaster-monitored.ps1) — per the "Verified means the
  live app, not `cargo check`" rule in AGENTS.md.

"Should work" and "looks correct" are not verification. Flag missing
evidence as **Critical: No verification evidence**.

## Dependency and locale hygiene (Critical if orphaned)

- If the PR deletes a module, the corresponding crate / npm package must
  come out of `Cargo.toml` / `package.json` in the same PR — see
  `dep-hygiene` skill.
- If the PR deletes a UI component, its i18next keys must come out of all
  20 locale files in the same commit — see `i18n-pruning` skill.

## Output structure (on top of superpowers:code-reviewer)

Preserve the upstream Critical / Important / Suggestion categorization.
Add a final "Toaster verification status" checklist using the items above
so it is explicit which gates ran.
