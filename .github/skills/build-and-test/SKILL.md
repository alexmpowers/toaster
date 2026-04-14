---
name: build-and-test
description: 'Build the Toaster project and run all tests using MSYS2 MinGW-w64 toolchain. Use for: build project, run tests, compile, make, cmake, mingw32-make, test-edit, test-signals, test-integration, build failure, link error, DLL lock.'
---

# Build and Test

Build the Toaster project and run test suites in the MSYS2 MinGW-w64 environment.

## When to Use
- Build the entire project or a specific target
- Run one or all test executables
- Diagnose build or link failures

## Prerequisites

MSYS2 packages: `mingw-w64-x86_64-gcc`, `mingw-w64-x86_64-make`, `mingw-w64-x86_64-ffmpeg`, `mingw-w64-x86_64-qt6-base`, `mingw-w64-x86_64-qt6-multimedia`, `mingw-w64-x86_64-SDL2`, `mingw-w64-x86_64-pkg-config`

## Procedure

### 1. Set Environment
```bash
export PATH="/c/Program Files/CMake/bin:/c/msys64/mingw64/bin:$PATH"
```

### 2. Configure (first time or after CMakeLists changes)
```bash
cd /c/git/toaster/build
cmake .. -G "MinGW Makefiles" -DCMAKE_PREFIX_PATH=/c/msys64/mingw64
```

### 3. Build
```bash
mingw32-make -j4
```

### 4. Run Tests
```bash
./bin/test-edit.exe        # Edit model (transcript, undo/redo)
./bin/test-signals.exe     # Signal/callback system
./bin/test-timeline.exe    # Timeline operations
./bin/test-integration.exe # Full pipeline (needs media files)
./bin/test-live.exe        # Whisper transcription (needs model)
```

## Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| `cannot open output file .exe` | Old process holds DLL lock | Kill `toaster-app.exe` then rebuild |
| `undefined reference to toaster_*` | Missing link to `toaster` library | Add `target_link_libraries(... PRIVATE toaster)` in CMakeLists.txt |
| `No package 'libavformat' found` | pkg-config can't find FFmpeg | Ensure `/c/msys64/mingw64/bin` is in PATH |
| Test prints `FAIL` | Test assertion failed | Read the FAIL message for the specific check that failed |
