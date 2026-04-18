# Build Instructions

This guide sets up and builds **Toaster** (Tauri + Rust + React).

## Prerequisites

### All platforms

- Rust (stable)
- Node.js (v18+) with npm (Bun optional for utility scripts)
- Tauri prerequisites for your OS
- CMake

### Windows

- Visual Studio 2022 Build Tools (C++ workload)
- LLVM (`winget install LLVM.LLVM`)
- Vulkan SDK (`winget install KhronosGroup.VulkanSDK`)
- Ninja (`winget install Ninja-build.Ninja`)

## Setup

### 1. Clone

```bash
git clone https://github.com/itsnotaboutthecell/toaster.git
cd toaster
```

### 2. Install frontend dependencies

```bash
bun install --frozen-lockfile
```

### 3. Windows environment initialization

Run this in the same shell before Cargo/Tauri commands:

```powershell
.\scripts\setup-env.ps1
```

## Development commands

```bash
# full app (frontend + backend) — cross-platform minimum
cargo tauri dev

# production build
cargo tauri build
# or: npm run tauri build

# frontend only
npm run dev
npm run build
```

On Windows the monitored launcher (below) is required for live dev mode — see
AGENTS.md §"Launch protocol".

## Launch protocol

See AGENTS.md §"Launch protocol" for the authoritative rule. In short: on
Windows use the monitored launcher; cross-platform minimum is `cargo tauri dev`.
Do not stop at process start; monitor startup output for 404/runtime/initialization
failures and gather logs on failure before reporting status.

### Monitored launcher (required on Windows)

Runs environment setup, starts the app, and prints bounded startup observation
with captured logs:

```powershell
.\scripts\launch-toaster-monitored.ps1 -ObservationSeconds 120
```

It runs environment setup, starts `cargo tauri dev`, and prints:

- `monitor_summary=...` (detected success/error signal keys + hints)
- `launch_logs_stdout=...` and `launch_logs_stderr=...` (captured logs)
- `launch_status=launched_ok|launched_with_errors|failed_to_launch`

### Offline local LLM eval gate (cleanup + precision + ASR oracle)

Run the combined offline rollout gate:

```powershell
.\scripts\run-local-llm-eval-gate.ps1 -MediaPath "C:\path\to\file.mp4" -AsrModelPath "C:\path\to\ggml-small.bin"
```

Optional output directory override:

```powershell
.\scripts\run-local-llm-eval-gate.ps1 -MediaPath "C:\path\to\file.mp4" -AsrModelPath "C:\path\to\ggml-small.bin" -OutputDir "C:\temp\toaster-local-llm-gate"
```

This gate has no silent fallback for required inputs:

- `-MediaPath` is required and must point to an existing media file.
- `-AsrModelPath` is required and must point to an existing local Whisper model file.

The run writes `local-llm-eval-gate-report.json` with machine-readable pass/fail output, explicit criteria for each check (`cleanup_quality`, `precision_safety`, `asr_leakage_oracle`), and failure reasons when the gate fails.

### First Build Timing

The first build after cloning (or after clearing `target/`) takes **2-4 minutes** due to:
- whisper-rs-sys Vulkan/ONNX compilation (~60s)
- Full Rust dependency compilation (~90s)
- Vite bundling (~15s)

Subsequent incremental builds typically take 10-30 seconds.
The launch monitoring script defaults to 120 seconds to accommodate first builds.

## Test and lint

```bash
cd src-tauri && cargo test
cd src-tauri && cargo test test_filter_filler_words -- --nocapture
cd src-tauri && cargo clippy
npm run lint
```

## Windows guardrails

- Use MSVC Rust toolchain target (not GNU)
- Run Cargo commands from `src-tauri\` when working directly with Cargo
- Stop running `toaster-app.exe`/`toaster.exe` before rebuilds to avoid DLL lock/link errors

## Windows code signing

The production build (`cargo tauri build`) produces an unsigned installer by default.
`src-tauri/tauri.conf.json` sets `"signCommand": ""` — an empty string means no signing.

**Unsigned builds will trigger Windows SmartScreen warnings** ("Windows protected your PC")
on first launch, which may deter users.

### What you need to sign

1. **Code signing certificate** — an EV (Extended Validation) certificate removes
   SmartScreen warnings immediately; a standard (OV) certificate builds trust over time.
2. **Set `signCommand`** in `tauri.conf.json` to invoke `signtool`, e.g.:
   ```json
   "signCommand": "signtool sign /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 /f \"%CERT_PATH%\" /p \"%CERT_PASSWORD%\" \"%1\""
   ```
3. **CI environment variables** — expose `CERT_PATH` (path to `.pfx` file) and
   `CERT_PASSWORD` (certificate password) as secrets in your CI pipeline.

For full details see the
[Tauri Windows signing guide](https://v2.tauri.app/distribute/sign/windows/).

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| `libclang not found` | LLVM missing | Install LLVM and set `LIBCLANG_PATH` |
| `VULKAN_SDK not set` | Vulkan SDK missing | Install Vulkan SDK and set `VULKAN_SDK` |
| `link.exe not found` | MSVC env not loaded | Run `scripts/setup-env.ps1` in current shell |
| `ort does not provide prebuilt binaries for gnu` | Wrong target | Use `stable-x86_64-pc-windows-msvc` |
| `Generator Ninja does not support platform specification, but platform x64 was specified` | `Platform=x64` (set by `vcvars64.bat`) leaked into the env alongside `CMAKE_GENERATOR=Ninja`. CMake on Windows reads `Platform` as the implicit default for `CMAKE_GENERATOR_PLATFORM`. | `setup-env.ps1` strips it after sourcing vcvars; if you bypass that script, `Remove-Item Env:Platform` before invoking cargo. Stale `target/debug/build/whisper-rs-sys-*/CMakeCache.txt` remembers the bad generator — delete those dirs once after the fix. |

## Build environment gotchas

These are non-obvious interactions that have broken the Windows build more
than once. `setup-env.ps1` is the single place that papers over them; do
not delete its workarounds without re-reading this section.

### `Platform=x64` from vcvars vs `CMAKE_GENERATOR=Ninja`

`vcvars64.bat` exports `Platform=x64` for MSBuild's benefit. CMake on
Windows reads the `Platform` env var (capital P) as the implicit default
for `CMAKE_GENERATOR_PLATFORM`. We force `CMAKE_GENERATOR=Ninja` because
Ninja gives faster incremental builds than the MSBuild-backed Visual
Studio generator. The two are mutually exclusive — Ninja rejects platform
specs, so every `project()` in `whisper-rs-sys`, `ggml`, and any other
CMake-driven dep will fail with:

> Generator Ninja does not support platform specification, but platform x64 was specified

We use cl.exe + Ninja end-to-end (never MSBuild from inside cargo), so
`Platform` has no legitimate consumer in this build. `setup-env.ps1`
clears it immediately after sourcing vcvars, and runs a preflight check
that screams loudly if either `Platform` or `CMAKE_GENERATOR_PLATFORM`
sneaks back in alongside `CMAKE_GENERATOR=Ninja`.

If you ever switch to MSBuild (don't), drop `CMAKE_GENERATOR=Ninja` first.

### Stale `whisper-rs-sys` CMakeCache after generator change

CMake records the generator in `CMakeCache.txt`. If a previous build
configured with one generator (say "Visual Studio 18 2026" because the
Platform leak forced a fallback) and a later build runs with a different
one ("Ninja"), CMake aborts with:

> Does not match the generator used previously: Ninja
> Either remove the CMakeCache.txt file and CMakeFiles directory or choose a different binary directory.

One-time fix after correcting the env (or any time you suspect a stale
whisper-rs-sys cache, see next section):

```powershell
.\scripts\clean-whisper-cache.ps1
```

That helper nukes both the build artifacts AND the cargo fingerprint dir,
under both `debug/` and `release/`. Either alone is insufficient — cargo
will short-circuit re-running build.rs if only one is missing.

You should not need this again once `Platform` is stripped properly.

### whisper-rs-sys does not advertise `CMAKE_GENERATOR` to cargo

`whisper-rs-sys/build.rs` declares `cargo:rerun-if-env-changed` only for
`BINDGEN_EXTRA_CLANG_ARGS*` and `VULKAN_SDK`. It does **not** declare
`CMAKE_GENERATOR`, `Platform`, or `CMAKE_GENERATOR_PLATFORM`. Consequence:
once a build fails under a bad env, the failure is cached forever from
cargo's point of view, and re-running cargo with corrected env vars will
still hit the same broken `CMakeCache.txt`. Different cargo subcommands
(`check` vs `test`, with different feature flags) hash to different
`whisper-rs-sys-<hash>` directories, so a `cargo check` that succeeds
does not guarantee `cargo test` will too. Use `scripts\clean-whisper-cache.ps1`
to wipe all of them at once when in doubt.
