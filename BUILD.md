# Build Instructions

This guide covers how to set up the development environment and build Toaster from source.

Toaster is forked from [Handy](https://github.com/cjpais/Handy). It inherits Handy's Tauri + React architecture and extends it with video editing capabilities.

## Prerequisites

### All Platforms

- [Rust](https://rustup.rs/) (latest stable)
- [Node.js](https://nodejs.org/) (v18+) or [Bun](https://bun.sh/)
- [Tauri Prerequisites](https://tauri.app/start/prerequisites/)
- [CMake](https://cmake.org/)

### Windows

- **Visual Studio 2022 Build Tools** with C++ workload
- **LLVM** — `winget install LLVM.LLVM` (provides libclang for bindgen)
- **Vulkan SDK** — `winget install KhronosGroup.VulkanSDK` (for whisper Vulkan acceleration)
- **Ninja** — `winget install Ninja-build.Ninja` (CMake generator)

**Quick install (PowerShell as admin):**

```powershell
winget install LLVM.LLVM --silent
winget install KhronosGroup.VulkanSDK --silent
winget install Ninja-build.Ninja --silent
```

### macOS

- Xcode Command Line Tools: `xcode-select --install`

### Linux

```bash
# Ubuntu/Debian
sudo apt update
sudo apt install build-essential libasound2-dev pkg-config libssl-dev \
  libvulkan-dev vulkan-tools glslc libgtk-3-dev libwebkit2gtk-4.1-dev \
  libayatana-appindicator3-dev librsvg2-dev cmake
```

## Setup

### 1. Clone

```bash
git clone https://github.com/itsnotaboutthecell/toaster.git
cd toaster
git checkout handy-fork
```

### 2. Install Dependencies

```bash
npm install --ignore-scripts
```

### 3. Environment Setup (Windows)

```powershell
.\scripts\setup-env.ps1
```

Or manually:

```powershell
$env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
$env:VULKAN_SDK = "C:\VulkanSDK\<version>"
$env:CMAKE_GENERATOR = "Ninja"

# Source MSVC environment
$vcvarsall = "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
$envOut = cmd /c "`"$vcvarsall`" x64 >nul 2>&1 && set"
foreach ($line in $envOut) {
    if ($line -match "^([^=]+)=(.*)$") {
        [Environment]::SetEnvironmentVariable($matches[1], $matches[2], "Process")
    }
}
```

### 4. Dev Server

```bash
cargo tauri dev
```

### 5. Production Build

```bash
cargo tauri build
```

## Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| `libclang not found` | LLVM not installed | `winget install LLVM.LLVM`, set `LIBCLANG_PATH` |
| `stdbool.h not found` | Missing MSVC include paths | Set `BINDGEN_EXTRA_CLANG_ARGS` with MSVC include dirs |
| `VULKAN_SDK not set` | Vulkan SDK not installed | `winget install KhronosGroup.VulkanSDK`, set `VULKAN_SDK` |
| `Visual Studio not found` | CMake using wrong generator | Set `CMAKE_GENERATOR=Ninja` |
| `link.exe not found` | MSVC env not sourced | Run vcvars64.bat or use setup-env.ps1 |
