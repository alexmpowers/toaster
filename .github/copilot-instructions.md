# Copilot instructions

Canonical guidance for this repository lives in [`../AGENTS.md`](../AGENTS.md).
Path-scoped conventions live in nested `AGENTS.md` files — nearest-file wins
per the [agents.md spec](https://agents.md/):

- [`../src-tauri/AGENTS.md`](../src-tauri/AGENTS.md) — Rust backend conventions, Windows env, cargo rules, DLL pitfalls.
- [`../src/AGENTS.md`](../src/AGENTS.md) — frontend conventions (TypeScript, React, Tailwind, i18n, bindings.ts, settings UI).

The `.github/instructions/*.instructions.md` files exist so GitHub Copilot
picks these rules up path-scoped via `applyTo:` frontmatter; they are
pointers to the nested AGENTS.md above. If a rule changes, edit the nested
AGENTS.md, not the pointer.

## Project overview

Toaster is a transcript-first video/audio editor ("edit video by editing text").
Stack: Tauri 2.x (Rust 1.82+), React 18, TypeScript 5, Vite 6, Tailwind CSS 3.

- `src/` — React + TypeScript + Tailwind UI
- `src-tauri/src/` — Rust backend (managers, commands, audio_toolkit)
- `.github/instructions/` — path-scoped convention files (Rust, TS, bindings, settings UI, code review)
- `.github/agents/` — custom agents (product-manager, eval-harness-runner, repo-auditor, etc.)
- `.github/skills/` — domain-specific skills (transcript-precision-eval, audio-boundary-eval, etc.)

## Key commands

```bash
bun install --frozen-lockfile
cargo tauri dev          # run dev app
npm run build            # frontend build
npm run lint             # frontend lint
cd src-tauri && cargo check && cargo clippy && cargo test
```

## Non-negotiable rules

- Backend managers own domain/business logic; frontend renders state.
- Keep-segment/time-mapping authority lives in the backend.
- No runtime network calls to hosted LLM/transcription/caption APIs (local-only inference).
- Single source of truth for dual-path logic (preview + export share one backend implementation).
- Backend timestamps use microseconds. UI strings use i18next keys.
- `src/bindings.ts` is specta-generated — do not hand-edit.

## Copilot-specific notes

Nothing at the moment. Add items here only if they apply **exclusively** to
GitHub Copilot tooling; everything shared with other agents belongs in
AGENTS.md.
