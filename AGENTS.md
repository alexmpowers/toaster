# AGENTS.md

Guidance for AI coding assistants working in this repository.

Toaster is a transcript-first video/audio editor ("edit video by editing text"), forked from Handy.  
Stack: Tauri 2.x (Rust backend) + React/TypeScript/Tailwind frontend.

## Core architecture

```text
src/                  React + TypeScript + Tailwind UI
  components/         editor/player/settings and shared UI
  stores/             Zustand state
  i18n/               localization files
src-tauri/src/        Rust backend
  managers/           business logic domains (audio/model/transcription/editor/media/export/project/history)
  commands/           Tauri command handlers
  audio_toolkit/      lower-level audio/VAD/text helpers
```

## Repository layout

Authoritative top-down map. Re-read this before grep-storming for "where does X live?".

```text
toaster/
├── AGENTS.md                  # canonical agent guidance (this file)
├── CLAUDE.md / CRUSH.md       # pointer files → AGENTS.md (per canonical-instructions skill)
├── README.md / LICENSE        # standard GitHub root files
├── SECURITY.md / CONTRIBUTING.md / CONTRIBUTING_TRANSLATIONS.md
├── PRD.md                     # product requirements (transcript-first editor scope)
├── docs/
│   ├── build.md               # platform build setup (was BUILD.md)
│   └── build-macos.md         # macOS NSPanel / private API notes
├── extras/                    # eval fixture media (mp4/png) — see extras/README.md
├── evals/                     # gitignored eval outputs (audio-boundary/, multi-backend-parity/)
├── scripts/                   # PowerShell tooling
│   ├── setup-env.ps1          # MSVC + LLVM + Vulkan env (run first on Windows)
│   ├── launch-toaster-monitored.ps1
│   ├── eval-edit-quality.ps1
│   ├── eval-audio-boundary.ps1
│   ├── eval-multi-backend-parity.ps1
│   └── lib/                   # shared PS modules (AudioBoundary.psm1, ...)
├── src/                       # React + TypeScript + Tailwind frontend
│   ├── App.tsx
│   ├── bindings.ts            # generated Tauri command bindings (do not hand-edit)
│   ├── components/            # editor/, player/, settings/, shared/
│   ├── stores/                # Zustand state
│   ├── lib/                   # frontend utilities + types
│   └── i18n/locales/          # 22 locale files, gated by check-translations.ts
├── src-tauri/                 # Rust backend (Tauri 2.x)
│   ├── Cargo.toml / tauri.conf.json
│   ├── src/
│   │   ├── lib.rs             # app entry, plugin registration
│   │   ├── audio_toolkit/     # timing, forced_alignment, vad/, text helpers, constants
│   │   ├── commands/          # Tauri command handlers (transcribe_file/, waveform/, history.rs, ...)
│   │   └── managers/          # business logic
│   │       ├── transcription/ # adapter trait + backend implementations
│   │       ├── editor/        # keep-segments, time mapping (backend authority)
│   │       ├── cleanup/       # filler-word removal, post-processing
│   │       ├── model/         # ASR model lifecycle
│   │       ├── project/       # project save/load
│   │       ├── history/       # undo/redo
│   │       └── export/        # FFmpeg-driven render pipeline
│   └── tests/                 # Rust integration tests
│       └── fixtures/          # alignment/, boundary/, parity/, mock_transcription_sample.json
├── tests/                     # Playwright E2E (app.spec.ts, skipSchedule.spec.ts)
├── nix/                       # Nix module variants (hm-module.nix, module.nix)
├── .nix/                      # bun2nix output (bun.nix, bun-lock-hash) — tracked
├── flake.nix / flake.lock     # root Nix flake (convention: stays at root)
└── .github/
    ├── skills/                # project skills — invoke per AGENTS.md "Skills and agents"
    ├── agents/                # project agents (code-reviewer, repo-auditor, ...)
    └── workflows/             # CI
```

## Non-negotiable boundaries

- Backend managers own domain/business logic.
- Frontend calls Tauri commands and renders state/events.
- Keep-segment/time-mapping behavior must come from backend authority.
- Never swap the video element source to an audio preview file; keep original video rendering source and sync preview audio separately.
- **Single source of truth for dual-path logic.** Any rendering or logic that lives on both the preview path (React) and the export path (FFmpeg/Rust) — caption layout and sizing, word grouping, filler/allow word lists, keep-segments, time mapping — must have one authoritative implementation in the backend, consumed verbatim by both paths. Duplicating it in the frontend (or hardcoding a list in Rust that also exists in the UI) is a defect, not a shortcut. The caption preview↔export mismatch and the hardcoded filler list both came from violating this rule.
- **Local-only inference.** Toaster performs all transcription and cleanup locally. No runtime network calls to hosted LLM/transcription/caption APIs. Adding a dependency that phones home — or a feature flag that enables one — is a breaking product change and requires explicit approval before landing.

## Development commands

```bash
bun install --frozen-lockfile
cargo tauri dev
cargo tauri build
npm run dev
npm run build
cd src-tauri && cargo check
cd src-tauri && cargo test
cd src-tauri && cargo test test_filter_filler_words -- --nocapture
cd src-tauri && cargo clippy
npm run lint
```

## Launch protocol

When the user says **"launch toaster"** (or equivalent), enter live dev mode:

1. Run `.\scripts\setup-env.ps1` in the shell first.
2. Start the app with `.\scripts\launch-toaster-monitored.ps1 -ObservationSeconds 120` (async mode, keep running).
3. Monitor startup output for compilation errors, 404s, runtime panics, or failed initialization.
4. On failure signals, immediately gather logs and do first-line debugging before reporting status.
5. On success, report the app is running and stay ready to inspect logs on demand.

Do **not** use bare `npm run tauri dev` — always use the monitored launch script.

## Windows requirements

- Run `.\scripts\setup-env.ps1` in the same PowerShell session before Cargo/Tauri commands.
- Use MSVC Rust toolchain target (`stable-x86_64-pc-windows-msvc`), not GNU.
- Prefer running direct Cargo commands from `src-tauri\`.
- Stop running `toaster-app.exe`/`toaster.exe` before rebuilds to avoid DLL/link lock failures.

## Cargo runtime expectations

Cold full-workspace `cargo clippy` / `cargo check` on this repo's dependency tree (whisper-rs-sys + ffmpeg-sys + the Tauri stack) on Windows MSVC routinely takes **2–10+ minutes per invocation**. A running cargo process under 10 minutes is almost certainly compiling, not hung.

- During iteration, scope cargo runs narrowly: `cargo check -p <crate>`, `cargo clippy -p <crate> --lib`, or a single `cargo test <name>`.
- Run the full-workspace `cargo check` + `cargo clippy` sweep at most once per milestone, not after every small edit.
- Do not cancel, retry, or re-invoke a cargo run under 10 minutes unless there is concrete evidence of a deadlock (e.g., zero CPU for minutes, stuck linker lock on a known-running `toaster-app.exe`).
- If a long cargo run exhausts your time budget, report its state honestly rather than silently starting another one — this is what caused the 20-minute stall in session history.

## Conventions

- Rust: run `cargo fmt` + `cargo clippy`; avoid `.unwrap()` in production paths.
- TypeScript: strict typing, no `any`, functional components.
- UI strings must use i18next keys.
- Backend timestamps use microseconds.

## Precision and UX guardrails

- Audio-edit acceptance gate: do not call timestamp/playback/export fixes complete until midstream deletions stay clean during replay (including long edits and delete/undo cycles) with no audible remnants.
- Preserve precise transcription timing (per-word/per-segment); never synthesize equal-duration timestamps.
- Detect actions must highlight only matched words; destructive actions (Delete) must apply only to that highlighted subset.
- For dark theme highlights, use high-contrast but low-noise styling (avoid hard-to-read red-on-dark combinations).

### "Verified" means the live app, not `cargo check`

For any fix touching **audio edits, captions, preview↔export parity, or timeline rendering**, "done" / "fixed" / "verified" requires driving the exact failing input through the monitored live app (`scripts\launch-toaster-monitored.ps1`) or the fixture-based precision eval — not merely a successful compile/clippy/unit-test run. In the completion message, cite the command that was run and the observed behavior. Completion claims that skip this step have repeatedly turned out to be wrong ("precision edits are lying to you", caption export regressions, cleanup not deleting detected words) and erode user trust.

### Settings UI contract

- Every user-exposed setting must render a **human-readable label and one-line description**. Never surface raw flag or enum names (e.g. no `caption_bg_opacity_b3` — write "Background transparency" with a plain-language description).
- Numeric controls: sliders must support **smooth drag AND double-click-to-type keyboard entry**. Do not ship spinner up/down arrows as the primary editing affordance.
- Use existing color tokens (rest state `#EEEEEE`, accent orange on hover, etc.); do not invent new greys/reds. Never place red text on dark backgrounds or light-grey text on white — both have recurred as readability bugs.

## Debugging tools

- `.\scripts\dump-debug-state.ps1` — Print current settings, FFmpeg status, and project state for diagnostics.
- `.\scripts\dump-caption-style.ps1` — ASS subtitle style reference and troubleshooting guide.

## Skills and agents

The following skills and agents are available under `.github/skills/` and `.github/agents/`.
Invoke them at the appropriate time — they are not optional suggestions.

### Required workflow skills

- **verification-before-completion** — Invoke before claiming ANY work is complete, fixed, or passing. No completion claims without fresh verification evidence (command output, not assumptions). This is non-negotiable.
- **systematic-debugging** — Invoke when encountering any bug, test failure, or unexpected behavior. Root cause investigation must complete before proposing fixes. No random guess-and-check.
- **test-driven-development** — Invoke when implementing any feature or bugfix. Write the failing test first, watch it fail, then write minimal code to pass. No production code without a failing test.
- **receiving-code-review** — Invoke when receiving code review feedback. Evaluate technically before implementing. No performative agreement or blind implementation.
- **canonical-instructions** — Invoke whenever editing an AI-instruction file (AGENTS.md, CLAUDE.md, .github/copilot-instructions.md, CRUSH.md). AGENTS.md is the single source of truth; other files are pointers.

### Build and environment

- **build-and-test** — Invoke for compile/test/lint runs, toolchain issues, and Windows build environment troubleshooting.
- **dep-hygiene** — Invoke before adding a dependency, after removing a module, and on any PR claiming "dead code cleanup". Enforces `cargo machete` / `knip` / `depcheck` gates.

### Legacy pruning and product scope

- **handy-legacy-pruning** — Invoke before editing any Handy-era dictation module (actions.rs, shortcut/, overlay.rs, tray*.rs, clipboard.rs, input.rs, audio_feedback.rs, apple_intelligence.rs, recorder.rs, vad/, PushToTalk.tsx, AudioFeedback.tsx, HandyKeysShortcutInput.tsx). Forces the "is this still on the transcript-editor path?" question before extending dead code.
- **i18n-pruning** — Invoke when deleting or renaming any i18next key. Ensures all 22 locale files stay in sync.
- **transcript-precision-eval** — Invoke when touching word operations, keep-segment logic, time mapping, transcription post-processing, or export.
- **audio-boundary-eval** — Invoke on any PR that modifies `managers/editor`, `commands/waveform`, export splice logic, preview audio rendering, or boundary snapping. Extends `transcript-precision-eval` with seam-level gates: cross-seam leakage `xcorr < 0.15` over 0–80 ms, click-free seams (`z < 4.0`), and preview↔export within 1 sample / `-40 dBFS` RMS.
- **transcription-adapter-contract** — Invoke before merging any PR that adds or swaps an ASR / forced-alignment backend. Enforces the `NormalizedTranscriptionResult` schema (monotonic non-overlap, no zero-duration words, stripped non-speech tokens, no silent equal-duration synthesis) and requires a round-trip fixture test that keeps precision + boundary gates green with the new backend.

### Review and audit agents

- **code-reviewer** — Invoke after completing a major feature, fix, or project step. Reviews implementation against the original plan, architecture boundaries, and coding standards. Catches plan deviations, boundary violations, and missing verification.
- **repo-auditor** — Invoke for whole-repository health audits (dead modules, monoliths, instruction drift, dep bloat, workflow duplication). Complements code-reviewer, which is diff-scoped.
- **eval-harness-runner** — Invoke to run the precision / midstream / export evals with one command and produce a pass/fail JSON for CI.
- **waveform-diff** — Invoke after audio-path milestones, or when a bug report sounds like "tiny remnants / clicks / drift". Renders preview and export to PCM, measures seam neighborhoods at sample level (cross-correlation, HF-burst energy, sample discontinuity, preview↔export parity), and emits JSON + human-readable findings. Does not fix code; reports only.
- **cut-drift-fuzzer** — Invoke before merging any edit-engine / time-mapping / undo-redo / export change. Runs seeded deterministic sequences (1000 ops) over synthetic beacon and real fixtures, asserting monotonic time maps, no cumulative duration drift (≤ 21 µs), no panics, and beacon preservation within 1 sample on PCM export. Emits pass/fail JSON.

## Hooks

Tool-call enforcement for the rules above lives in .github/hooks/. See .github/hooks/README.md for the list and bypass env vars.

