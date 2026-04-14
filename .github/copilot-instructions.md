# Project Guidelines

Toaster is a transcript-first video/audio editor ("edit video by editing text"). Forked from [Handy](https://github.com/cjpais/Handy), it uses Tauri 2.x (Rust backend + React/TypeScript frontend).

## Architecture

```
src/                  React + TypeScript + Tailwind frontend
  components/         UI components (Sidebar, settings, model-selector, editor)
  stores/             Zustand state management
  i18n/               Internationalization (20+ languages)
src-tauri/            Rust backend
  src/
    managers/         Core business logic (audio, model, transcription, editor, media)
    commands/         Tauri IPC command handlers
    audio_toolkit/    Low-level audio processing + VAD
```

**Hard boundary**: Backend managers own all business logic. Frontend components call Tauri commands — never access files, FFmpeg, or models directly.

## Build and Test

See [BUILD.md](BUILD.md) for full setup. Quick start:

```bash
npm install --ignore-scripts
cargo tauri dev          # Dev mode (Vite + Rust)
cargo tauri build        # Production build
cd src-tauri && cargo test  # Rust tests
```

Windows requires: VS Build Tools, LLVM, Vulkan SDK, Ninja, CMake. Use `scripts/setup-env.ps1`.

## Code Style

### Rust (src-tauri/)
- `cargo fmt` + `cargo clippy` before committing
- Handle errors with `anyhow::Result` — no `.unwrap()` in production
- Follow the manager pattern: business logic in `managers/`, thin wrappers in `commands/`
- All timestamps in **microseconds** (matching FFmpeg's `AV_TIME_BASE`)
- Doc comments on all public functions

### TypeScript/React (src/)
- Strict TypeScript, no `any` types
- Functional components with hooks
- Tailwind CSS for all styling (no inline styles or CSS modules)
- All UI text via i18next: `const { t } = useTranslation()`
- Path aliases: `@/` → `./src/`

## Design System

```
Primary:          #D9D8D8  (Toaster Grey)
Accent:           #E8A838  (Toast Gold)
Background:       #1E1E1E  (dark theme)
Text Primary:     #F0F0F0
Status Green:     #4ADE80
Status Red:       #EF4444
```

## Commit Guidelines

Conventional commits: `feat:`, `fix:`, `docs:`, `refactor:`, `chore:`

## Key Concepts

- **Word-level editing**: Each word has text, start_us, end_us, deleted, silenced, confidence, speaker_id
- **Keep-segments**: Non-deleted time ranges for playback/export
- **Manager pattern**: Managers are initialized at startup, held in Tauri state, accessed via commands
- **Command-Event pattern**: Frontend → Backend via commands; Backend → Frontend via events
