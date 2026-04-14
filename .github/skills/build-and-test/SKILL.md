---
name: build-and-test
description: 'Build the Toaster project and run tests using Tauri + Rust + React toolchain. Use for: build project, run tests, compile, cargo, tauri, npm, vite, build failure, link error, LLVM, Vulkan.'
---

# Build and Test

Build the Toaster project and run test suites using the Tauri 2.x toolchain.

## When to Use
- Build the entire project or a specific target
- Run Rust tests
- Diagnose build or link failures

## Prerequisites

- Rust (stable, MSVC target on Windows)
- Node.js (v18+) or Bun
- VS Build Tools 2022 (Windows)
- LLVM (`winget install LLVM.LLVM`)
- Vulkan SDK (`winget install KhronosGroup.VulkanSDK`)
- Ninja (`winget install Ninja-build.Ninja`)
- CMake

## Procedure

### 1. Set Environment (Windows)

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;C:\Program Files\CMake\bin;$env:PATH"
$env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
$env:VULKAN_SDK = "C:\VulkanSDK\1.4.341.1"
$env:CMAKE_GENERATOR = "Ninja"

# Source MSVC environment
$vcvarsall = "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
$envOut = cmd /c "`"$vcvarsall`" x64 >nul 2>&1 && set" 2>&1
foreach ($line in $envOut) {
    if ($line -match "^([^=]+)=(.*)$") {
        [Environment]::SetEnvironmentVariable($matches[1], $matches[2], "Process")
    }
}

# Set bindgen include paths
$msvcInclude = "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC\14.44.35207\include"
$ucrtInclude = "C:\Program Files (x86)\Windows Kits\10\Include\10.0.26100.0\ucrt"
$env:BINDGEN_EXTRA_CLANG_ARGS = "-I`"$msvcInclude`" -I`"$ucrtInclude`""
```

### 2. Install Frontend Dependencies

```bash
npm install --ignore-scripts
```

### 3. Build

```bash
# Type check only (fast)
cd src-tauri && cargo check

# Full dev build (compiles Rust + starts Vite)
cargo tauri dev

# Production build
cargo tauri build
```

### 4. Run Tests

```bash
cd src-tauri && cargo test
```

### 5. Lint

```bash
cd src-tauri && cargo clippy
npm run lint
```

## Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| `libclang not found` | LLVM not installed | `winget install LLVM.LLVM`, set `LIBCLANG_PATH` |
| `stdbool.h not found` | Missing MSVC includes | Set `BINDGEN_EXTRA_CLANG_ARGS` with MSVC+UCRT include dirs |
| `VULKAN_SDK not set` | Vulkan SDK missing | `winget install KhronosGroup.VulkanSDK`, set env var |
| `Visual Studio not found` | Wrong CMake generator | Set `CMAKE_GENERATOR=Ninja` |
| `link.exe not found` | MSVC env not sourced | Run vcvars64.bat first |
| `ort does not provide prebuilt binaries for gnu` | Wrong Rust target | Use `stable-x86_64-pc-windows-msvc` not GNU |
