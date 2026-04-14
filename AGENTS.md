# AGENTS.md

This file provides guidance to AI coding assistants working with code in this repository.

Toaster is a transcript-first video/audio editor — "edit video by editing text." Forked from [Handy](https://github.com/cjpais/Handy), it inherits Handy's beautiful UI and offline model management, then adds video import, word-level transcript editing, waveform visualization, and media export.

## Development Commands

**Prerequisites:**

- [Rust](https://rustup.rs/) (latest stable, MSVC target on Windows)
- [Node.js](https://nodejs.org/) or [Bun](https://bun.sh/)
- Platform-specific: see [BUILD.md](BUILD.md)

**Windows-specific prerequisites:**

- Visual Studio 2022 Build Tools (C++ workload)
- [LLVM](https://releases.llvm.org/) (for bindgen/whisper-rs-sys)
- [Vulkan SDK](https://vulkan.lunarg.com/) (for whisper Vulkan acceleration)
- [Ninja](https://ninja-build.org/) build system
- [CMake](https://cmake.org/)

**Environment setup (Windows):**

```powershell
# Run the setup script to configure all environment variables
.\scripts\setup-env.ps1
```

**Core Development:**

```bash
# Install dependencies
npm install --ignore-scripts

# Run in development mode (starts Vite + Rust backend)
cargo tauri dev

# Build for production
cargo tauri build

# Frontend only development
npm run dev        # Start Vite dev server
npm run build      # Build frontend (TypeScript + Vite)

# Rust only
cd src-tauri
cargo check        # Type check
cargo test         # Run tests
cargo clippy       # Lint
```

**Linting and Formatting (run before committing):**

```bash
npm run lint              # ESLint for frontend
npm run lint:fix          # ESLint with auto-fix
cd src-tauri && cargo fmt # Rust formatting
cd src-tauri && cargo clippy # Rust linting
```

For detailed platform-specific build setup, see [BUILD.md](BUILD.md).

## Architecture Overview

Toaster is built with Tauri 2.x (Rust backend + React/TypeScript frontend). The architecture inherits Handy's manager pattern and extends it with video editing capabilities.

### Backend Structure (src-tauri/src/)

**From Handy (inherited):**
- `lib.rs` - Main entry point, Tauri setup, manager initialization
- `managers/` - Core business logic:
  - `audio.rs` - Audio recording and device management
  - `model.rs` - Model downloading and management (16+ whisper models)
  - `transcription.rs` - Speech-to-text processing pipeline
  - `history.rs` - Transcription history storage
- `audio_toolkit/` - Low-level audio processing:
  - `audio/` - Device enumeration, recording, resampling
  - `vad/` - Voice Activity Detection (Silero VAD)
- `commands/` - Tauri command handlers for frontend communication
- `settings.rs` - Application settings management
- `overlay.rs` - Recording overlay window (platform-specific)

**Toaster additions (to be built):**
- `managers/media.rs` - FFmpeg decode, video playback, waveform extraction
- `managers/editor.rs` - Transcript editing engine (word-level delete/restore/split, undo/redo)
- `managers/export.rs` - Caption export (SRT/VTT/script) + media re-encode
- `managers/project.rs` - Project save/load
- `commands/media.rs` - Open file, play/pause/seek commands
- `commands/editor.rs` - Word editing operations
- `commands/export.rs` - Export commands
- `commands/project.rs` - Project file commands

### Frontend Structure (src/)

**From Handy (inherited):**
- `App.tsx` - Main component with onboarding flow
- `components/` - React UI components:
  - `Sidebar.tsx` - Icon sidebar navigation
  - `settings/` - Settings UI panels
  - `model-selector/` - Model cards, download progress, status pills
  - `onboarding/` - First-run model download experience
  - `footer/` - Model status button, version info
  - `shared/`, `ui/`, `icons/` - Shared components
- `stores/` - Zustand state management
  - `modelStore.ts` - Model state
  - `settingsStore.ts` - Settings state
- `bindings.ts` - Auto-generated Tauri type bindings (via tauri-specta)
- `i18n/` - Internationalization (20+ languages)

**Toaster additions (to be built):**
- `components/editor/` - TranscriptView, VideoPlayer, WaveformView, EditorPanel
- `components/export/` - ExportPanel (format selection, progress)
- `stores/editorStore.ts` - Transcript words, selection, playback position
- `stores/projectStore.ts` - Project open/save state

### Key Architecture Patterns

**Manager Pattern:** Core functionality organized into managers initialized at startup and managed via Tauri state. Each manager owns its domain data and exposes operations.

**Command-Event Architecture:** Frontend → Backend via Tauri commands (`#[tauri::command]`); Backend → Frontend via events (`app.emit()`).

**State Flow:** Zustand stores → Tauri Commands → Rust Managers → Persistence

### Technology Stack

| Layer | Technology |
|-------|-----------|
| Frontend | React 18 + TypeScript + Tailwind CSS 4 |
| Desktop shell | Tauri 2.x (Rust) |
| State management | Zustand |
| Transcription | transcribe-rs (whisper.cpp + ONNX) |
| Audio I/O | cpal + rubato + rodio |
| VAD | vad-rs (Silero) |
| Video decode | ffmpeg-next (planned) |
| Project storage | rusqlite |
| Build | Vite (frontend) + Cargo (backend) |

### Toaster-Specific Concepts

**Word-level editing:** Each transcript word has: text, start_us, end_us, deleted, silenced, confidence, speaker_id. Operations (delete, restore, split) create undo snapshots.

**Timestamps:** All timestamps in **microseconds** (matching FFmpeg's AV_TIME_BASE).

**Keep-segments:** After deletions, calculate non-deleted time ranges for playback and export. Map edit-time → source-time by summing deleted durations.

**Filler detection:** Pattern-match words like "um", "uh", "like", "you know" and suggest bulk deletion.

## Internationalization (i18n)

All user-facing strings must use i18next translations. ESLint enforces this.

**Adding new text:**

1. Add key to `src/i18n/locales/en/translation.json`
2. Use in component: `const { t } = useTranslation(); t('key.path')`

## Code Style

**Rust:**

- Run `cargo fmt` and `cargo clippy` before committing
- Handle errors explicitly with `anyhow::Result` — avoid `.unwrap()` in production
- Use descriptive names, add doc comments for public APIs
- Follow the manager pattern for new features

**TypeScript/React:**

- Strict TypeScript, avoid `any` types
- Functional components with hooks
- Tailwind CSS for styling (no inline styles or CSS modules)
- Path aliases: `@/` → `./src/`
- All UI text via i18next

## Commit Guidelines

Use conventional commits: `feat:`, `fix:`, `docs:`, `refactor:`, `chore:`

## Design System

Toaster uses a custom color palette derived from its mascot:

```
Primary:            #D9D8D8  (Toaster Grey — mascot body)
Accent:             #E8A838  (Toast Gold — mascot toast)
Background Dark:    #1E1E1E
Background Panel:   #252525
Surface:            #2A2A2A
Border:             rgba(128, 128, 128, 0.2)
Text Primary:       #F0F0F0
Text Secondary:     rgba(240, 240, 240, 0.6)
Status Green:       #4ADE80
Status Yellow:      #FACC15
Status Red:         #EF4444
```

## Platform Notes

- **Windows**: MSVC target, Vulkan acceleration for whisper, requires LLVM + Vulkan SDK
- **macOS**: Metal acceleration, accessibility permissions required
- **Linux**: Vulkan acceleration, limited Wayland support
