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

# Bindgen clang include paths
$msvcBase = "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC"
$msvcVersion = Get-ChildItem $msvcBase -Directory -ErrorAction SilentlyContinue | Sort-Object Name -Descending | Select-Object -First 1 -ExpandProperty Name
$ucrtBase = "C:\Program Files (x86)\Windows Kits\10\Include"
$ucrtVersion = Get-ChildItem $ucrtBase -Directory -ErrorAction SilentlyContinue | Sort-Object Name -Descending | Select-Object -First 1 -ExpandProperty Name

if ($msvcVersion -and $ucrtVersion) {
    $msvcInclude = "$msvcBase\$msvcVersion\include"
    $ucrtInclude = "$ucrtBase\$ucrtVersion\ucrt"
    $env:BINDGEN_EXTRA_CLANG_ARGS = "-I`"$msvcInclude`" -I`"$ucrtInclude`""
    Write-Host "Bindgen includes configured" -ForegroundColor Green
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
