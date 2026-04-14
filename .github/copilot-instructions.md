# Project Guidelines

Toaster is a text-based video/audio editor ("edit video by editing text"). Architecture mirrors OBS Studio: a pure-C core engine (`libtoaster/`) with a Qt6 frontend (`frontend/`). See [PRD.md](../PRD.md) for full product requirements and phased roadmap.

## Architecture

```
libtoaster/     Pure C library — edit model, plugin system, signals, project I/O
  callback/     Signal/slot infrastructure (OBS-style)
  util/         Platform helpers, threading, darray
frontend/       Qt6/C++ GUI — video widget, transcript panel, playback engine
plugins/        Loadable modules (FFmpeg decoder/exporter, filler filter, pause filter, whisper)
test/           CLI test harness (no framework)
```

**Hard boundary**: `libtoaster` has zero knowledge of Qt or any UI. The frontend and OBS plugin (Phase 4) are both consumers of the same C API.

## Build and Test

Requires MSYS2 MinGW-w64 environment with FFmpeg, Qt6, SDL2.

```bash
export PATH="/c/Program Files/CMake/bin:/c/msys64/mingw64/bin:$PATH"
cd /c/git/toaster/build
cmake .. -G "MinGW Makefiles" -DCMAKE_PREFIX_PATH=/c/msys64/mingw64
mingw32-make -j4
```

Run tests: `./build/bin/test-edit.exe`, `./build/bin/test-signals.exe`, etc.  
Run app: `./build/bin/toaster-app.exe`

**Gotcha**: Kill old `toaster-app.exe` before rebuilding — DLL locks cause link failures on Windows.

## Code Style

### C (libtoaster, plugins, tests)
- `toaster_` prefix for all public symbols; `_t` suffix for types; `snake_case` everywhere
- `TOASTER_API` macro on public functions (controls dllexport/visibility)
- `bool` returns for success/failure (true = success); no exceptions
- `calloc()` for zero-init; destructors always null-check first
- Array growth: `cap ? cap * 2 : initial_size` (exponential doubling)
- All timestamps in **microseconds** (matching FFmpeg's `AV_TIME_BASE`)
- Forward declarations over transitive includes

### C++ (frontend)
- Qt conventions: `m_` prefix for member variables, camelCase methods
- **Always** `blockSignals(true)` around programmatic QTextEdit content changes to prevent re-entrant signal loops

## Plugin API

All plugin types (filter, decoder, exporter, encoder) follow the same pattern:

1. Define a static `toaster_{type}_info_t` struct populated with callback function pointers (`get_name`, `create`, `destroy`, plus type-specific operations)
2. Export a `{plugin_name}_load(void)` function that calls `toaster_register_{type}(&info)`
3. Call the load function **after** `toaster_startup()` — never use `__attribute__((constructor))`

See `plugins/filler-filter/filler-filter.c` as the canonical example.

## Test Conventions

Tests are standalone C executables using a simple PASS/FAIL macro pattern:

```c
static int failures = 0;
#define PASS(name) printf("  PASS: %s\n", name)
#define FAIL(name, msg) do { printf("  FAIL: %s — %s\n", name, msg); failures++; } while (0)
```

Each test calls `toaster_startup()` at entry and `toaster_shutdown()` at exit. Return `failures ? 1 : 0`.

## Known Pitfalls

- **Plugin registration timing**: Explicit `*_load()` after `toaster_startup()`, not constructors
- **FFmpeg packet interleaving**: `av_read_frame()` returns packets for any stream — queue packets per stream
- **AVFrame separation**: Use separate `video_frame` / `audio_frame` to prevent decode corruption
- **FFmpeg cleanup order**: Free sws/swr contexts before avcodec, avcodec before avformat
- **Timestamp mapping**: Deleted words reduce effective duration; playback must sum deleted segments to map edit-time → source-time

## Optional Terse Skills

- `caveman` is opt-in. Use it only when the user explicitly asks for caveman mode, fewer tokens, or very terse answers.
- `caveman-commit` is opt-in for terse conventional commit messages.
- `caveman-review` is opt-in for terse paste-ready review comments.
- Clarity overrides terseness for security warnings, irreversible actions, onboarding explanations, and multi-step FFmpeg or audio-debugging procedures.
- These skills complement the project guidance in this file; they do not replace it.
