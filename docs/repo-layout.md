# Repository layout

Authoritative top-down map. Re-read this before grep-storming for "where does X live?".

```text
toaster/
├── AGENTS.md                  # canonical agent guidance (entry document)
├── README.md / LICENSE        # standard GitHub root files
├── SECURITY.md / CONTRIBUTING.md / CONTRIBUTING_TRANSLATIONS.md
├── docs/
│   ├── PRD.md                 # product requirements (transcript-first editor scope)
│   ├── build.md               # platform build setup
│   ├── repo-layout.md         # this file
│   ├── commands.md            # rendered commands reference (auto from registry)
│   ├── spec-driven.md         # spec-driven development lifecycle (PM agent)
│   └── testing-kb.md          # empirical testing facts across features
├── eval/                      # evaluation ecosystem
│   ├── fixtures/              # committed fixture media (mp4/png) — see eval/fixtures/README.md
│   └── output/                # gitignored eval run outputs (audio-boundary/, multi-backend-parity/)
├── features/                  # spec-driven planning bundles (see docs/spec-driven.md)
│   └── .templates/            # starter templates for REQUEST/PRD/BLUEPRINT/coverage/tasks
├── scripts/                   # PowerShell / TS tooling
│   ├── setup-env.ps1          # MSVC + LLVM + Vulkan env (run first on Windows)
│   ├── launch-toaster-monitored.ps1  # live-app verification runner
│   ├── agents-registry.ts     # reader CLI over .github/registry/*.json
│   ├── build-registry.ts      # auto-gen skills.json + agents.json from frontmatter
│   ├── check-translations.ts  # i18n locale parity
│   ├── check-file-sizes.ts    # file-size cap enforcement
│   ├── sql/                   # reusable SQL snippets (reap-stale-todos.sql, ...)
│   ├── eval/                  # precision, boundary, parity evals + fixture generators
│   ├── gate/                  # CI gate scripts (cargo-check, lint, tsc, registry)
│   ├── feature/               # PM tooling (scaffold, promote, coverage, tasks, board)
│   ├── dev/                   # debugging & cleanup (dump-debug, dump-caption, clean-*)
│   ├── migrate/               # one-off migration/audit scripts
│   └── lib/                   # shared PS modules (AudioBoundary.psm1, ...)
├── src/                       # React + TypeScript + Tailwind frontend
│   ├── AGENTS.md              # frontend conventions (nearest-file wins)
│   ├── App.tsx
│   ├── bindings.ts            # generated Tauri command bindings (do not hand-edit)
│   ├── assets/                # brand images (toaster.png, toaster.svg, toaster_text.svg)
│   ├── components/            # editor/, player/, settings/, shared/
│   ├── stores/                # Zustand state
│   ├── lib/                   # frontend utilities + types
│   └── i18n/locales/          # 20 locale files, gated by check-translations.ts
├── src-tauri/                 # Rust backend (Tauri 2.x)
│   ├── AGENTS.md              # Rust conventions + Windows build env (nearest-file wins)
│   ├── Cargo.toml / tauri.conf.json
│   ├── src/
│   │   ├── lib.rs             # app entry, plugin registration
│   │   ├── audio_toolkit/     # timing, forced_alignment, audio/, text helpers, constants
│   │   ├── commands/          # Tauri command handlers (transcribe_file/, waveform/, ...)
│   │   └── managers/          # business logic
│   │       ├── captions/       # caption generation and styling
│   │       ├── editor/        # keep-segments, time mapping (backend authority)
│   │       ├── export.rs      # FFmpeg-driven render pipeline
│   │       ├── filler.rs      # filler-word detection and removal
│   │       ├── model/         # ASR model lifecycle
│   │       ├── splice/        # audio splice logic
│   │       ├── transcription/ # adapter trait + backend implementations
│   │       └── project.rs     # project save/load
│   └── tests/                 # Rust integration tests
│       └── fixtures/          # alignment/, boundary/, parity/, mock_transcription_sample.json
├── tests/                     # Playwright E2E (app.spec.ts, skipSchedule.spec.ts)
├── nix/                       # Nix module variants (hm-module.nix, module.nix)
├── .nix/                      # bun2nix output (bun.nix, bun-lock-hash) — tracked
├── flake.nix / flake.lock     # root Nix flake (convention: stays at root)
└── .github/
    ├── registry/              # structured-data source of truth (rules, commands, testing, boundaries, hygiene, verification, skills, agents)
    ├── templates/             # plan-active-work.md, retry-log-entry.md, ...
    ├── skills/                # project skills — see `bun scripts/registry/reader.ts skills`
    ├── agents/                # custom agents — see `bun scripts/registry/reader.ts agents`
    ├── instructions/          # thin Copilot path-scoped pointers → nested AGENTS.md
    ├── prompts/               # invocable prompt files (/launch-toaster, /dump-debug, /run-eval)
    └── workflows/             # CI
```
