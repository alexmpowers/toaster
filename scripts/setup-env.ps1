# Toaster Windows Build Environment Setup
# Run this script before building: . .\scripts\setup-env.ps1

Write-Host "Setting up Toaster build environment..." -ForegroundColor Cyan

# Rust
$env:PATH = "$env:USERPROFILE\.cargo\bin;C:\Program Files\CMake\bin;$env:PATH"

# LLVM (for bindgen / whisper-rs-sys)
$env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
if (-not (Test-Path "$env:LIBCLANG_PATH\libclang.dll")) {
    Write-Host "WARNING: LLVM not found. Install with: winget install LLVM.LLVM" -ForegroundColor Yellow
}

# Vulkan SDK (for whisper Vulkan acceleration)
$vulkanDir = Get-ChildItem "C:\VulkanSDK" -Directory -ErrorAction SilentlyContinue | Sort-Object Name -Descending | Select-Object -First 1 -ExpandProperty FullName
if ($vulkanDir) {
    $env:VULKAN_SDK = $vulkanDir
    Write-Host "Vulkan SDK: $vulkanDir" -ForegroundColor Green
} else {
    Write-Host "WARNING: Vulkan SDK not found. Install with: winget install KhronosGroup.VulkanSDK" -ForegroundColor Yellow
}

# CMake generator
$env:CMAKE_GENERATOR = "Ninja"

# Source MSVC environment (Visual Studio Build Tools)
$vcvarsall = "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
if (Test-Path $vcvarsall) {
    $envOut = cmd /c "`"$vcvarsall`" x64 >nul 2>&1 && set" 2>&1
    foreach ($line in $envOut) {
        if ($line -match "^([^=]+)=(.*)$") {
            [Environment]::SetEnvironmentVariable($matches[1], $matches[2], "Process")
        }
    }
    Write-Host "MSVC environment sourced" -ForegroundColor Green
} else {
    Write-Host "WARNING: VS Build Tools not found. Install C++ workload." -ForegroundColor Yellow
}

# Strip the MSBuild `Platform=x64` env var that vcvars64.bat exports.
#
# Root cause: CMake on Windows reads `Platform` (capital P) as the implicit
# default for `CMAKE_GENERATOR_PLATFORM`. We force `CMAKE_GENERATOR=Ninja`
# above; Ninja rejects platform specs, so every `project()` call fails with:
#   "Generator Ninja does not support platform specification, but
#    platform x64 was specified"
# We build with cl.exe + Ninja, never with MSBuild, so `Platform` has no
# legitimate consumer here. Do NOT delete this without re-reading the
# Build environment gotchas section in docs/build.md.
Remove-Item Env:Platform -ErrorAction SilentlyContinue

# Bindgen clang include paths
$msvcBase = "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC"
$msvcVersion = Get-ChildItem $msvcBase -Directory -ErrorAction SilentlyContinue | Sort-Object Name -Descending | Select-Object -First 1 -ExpandProperty Name
$ucrtBase = "C:\Program Files (x86)\Windows Kits\10\Include"
$ucrtVersion = Get-ChildItem $ucrtBase -Directory -ErrorAction SilentlyContinue | Sort-Object Name -Descending | Select-Object -First 1 -ExpandProperty Name

if ($msvcVersion -and $ucrtVersion) {
    $msvcInclude = "$msvcBase\$msvcVersion\include"
    $ucrtInclude = "$ucrtBase\$ucrtVersion\ucrt"

    # Locate clang's own builtin header directory (contains stdbool.h, stdint.h, etc.).
    # Must come FIRST on the include path: MSVC's <stdbool.h> / <stdint.h> rely on
    # clang builtin headers when libclang parses with --target=*-windows-msvc.
    # Without this, bindgen fails with "'stdbool.h' file not found" and whisper-rs-sys
    # falls back to stale bundled bindings → struct-layout assertion overflow errors.
    $clangBuiltinInclude = $null
    $clangLibDir = "C:\Program Files\LLVM\lib\clang"
    if (Test-Path $clangLibDir) {
        $clangVer = Get-ChildItem $clangLibDir -Directory -ErrorAction SilentlyContinue |
            Sort-Object { [int]($_.Name -replace '\D','') } -Descending |
            Select-Object -First 1 -ExpandProperty Name
        if ($clangVer) {
            $candidate = "$clangLibDir\$clangVer\include"
            if (Test-Path "$candidate\stdbool.h") { $clangBuiltinInclude = $candidate }
        }
    }

    $bindgenArgs = @("--target=x86_64-pc-windows-msvc")
    if ($clangBuiltinInclude) { $bindgenArgs += "-I`"$clangBuiltinInclude`"" }
    $bindgenArgs += "-I`"$msvcInclude`""
    $bindgenArgs += "-I`"$ucrtInclude`""
    $env:BINDGEN_EXTRA_CLANG_ARGS = $bindgenArgs -join " "
    Write-Host "Bindgen includes configured" -ForegroundColor Green
    if (-not $clangBuiltinInclude) {
        Write-Host "  WARNING: clang builtin include dir not found; bindgen may fail" -ForegroundColor Yellow
    }
}

# Verify toolchain
$checks = @(
    @{ Name = "rustc"; Cmd = { rustc --version 2>$null } },
    @{ Name = "cargo"; Cmd = { cargo --version 2>$null } },
    @{ Name = "cl.exe"; Cmd = { Get-Command cl.exe -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source } },
    @{ Name = "ninja"; Cmd = { Get-Command ninja -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source } },
    @{ Name = "cmake"; Cmd = { cmake --version 2>$null | Select-Object -First 1 } }
)

Write-Host "`nToolchain:" -ForegroundColor Cyan
foreach ($check in $checks) {
    $result = & $check.Cmd
    if ($result) {
        Write-Host "  [OK] $($check.Name): $result" -ForegroundColor Green
    } else {
        Write-Host "  [!!] $($check.Name): NOT FOUND" -ForegroundColor Red
    }
}

Write-Host "`nEnvironment ready. Run 'cargo tauri dev' to start." -ForegroundColor Cyan

# Preflight: catch future regressions of the vcvars `Platform` leak (or any
# other process setting CMAKE_GENERATOR_PLATFORM) on day zero. If either is
# set while CMAKE_GENERATOR=Ninja, every CMake-driven build (whisper-rs-sys,
# ggml, ffmpeg) will fail with "Generator Ninja does not support platform
# specification". Better to scream here than to debug a 5-minute cold cmake
# error.
if ($env:CMAKE_GENERATOR -eq "Ninja" -and ($env:Platform -or $env:CMAKE_GENERATOR_PLATFORM)) {
    Write-Host "`n[FAIL] Build env corrupted: CMAKE_GENERATOR=Ninja but Platform=[$env:Platform] CMAKE_GENERATOR_PLATFORM=[$env:CMAKE_GENERATOR_PLATFORM]" -ForegroundColor Red
    Write-Host "       CMake will reject the platform spec. See docs/build.md > Build environment gotchas." -ForegroundColor Red
}
