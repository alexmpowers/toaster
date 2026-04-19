# src-tauri — Rust backend conventions

Path-scoped conventions for `src-tauri/`. Authoritative per the
[AGENTS.md nearest-file rule](https://agents.md/). The root
[`../AGENTS.md`](../AGENTS.md) covers cross-cutting rules; this file covers
Rust-specific ones.

## Quick rules

- ALWAYS run `. .\scripts\setup-env.ps1 *>&1 | Out-Null; cd src-tauri; cargo <cmd>` as **one** PowerShell call. Env does not persist across tool calls.
- PREFER scoped runs during iteration: `cargo check -p toaster --lib`, `cargo clippy -p toaster --lib`, `cargo test -p toaster --lib <name>`.
- AVOID full-workspace `cargo clippy` / `cargo check` — run them at most once per milestone (2–10 min cold on MSVC).
- NEVER use `.unwrap()` / `.expect()` / `panic!` in production paths. Return `anyhow::Result` or a typed error.
- PREFER `anyhow::Result<T>` for application errors, `thiserror` for library-level typed errors.
- ALWAYS use microseconds for timestamps (matching FFmpeg `AV_TIME_BASE`). Never `f64` seconds.
- ALWAYS put business logic in `managers/`; commands in `commands/` are thin IPC wrappers.
- NEVER duplicate preview-path logic in the frontend — the backend owns keep-segments, caption layout, word grouping, filler lists.
- FORBIDDEN: adding runtime network calls to hosted ASR / LLM / caption APIs. Toaster is local-only inference.
- FORBIDDEN: files over **800 lines** in `src-tauri/src/` (excluding generated). Enforced by `bun run check:file-sizes`. Grandfathered offenders in `scripts/file-size-allowlist.txt`.
- NEVER retry a cargo command a third time with the same incantation after two failures with an identical error hash — see "Retry budget" in the root [AGENTS.md](../AGENTS.md#session--workspace-hygiene).

## Windows build environment

- Use the MSVC Rust toolchain target (`stable-x86_64-pc-windows-msvc`), not GNU.
- Run `.\scripts\setup-env.ps1` first in every fresh PowerShell session to configure MSVC + LLVM + Vulkan.
- Stop running `toaster-app.exe` / `toaster.exe` before rebuilds to avoid DLL/link lock failures.
- Canonical one-liner when the agent's tool call starts a fresh shell:

  ```powershell
  . .\scripts\setup-env.ps1 *>&1 | Out-Null; cd src-tauri; cargo <cmd>
  ```

  Splitting this across two tool calls silently runs cargo without MSVC/Vulkan/bindgen env and wastes a 2–10 min compile on a broken link step.

## Cargo runtime expectations

Cold full-workspace `cargo clippy` / `cargo check` on this repo's dependency
tree (whisper-rs-sys + ffmpeg-sys + the Tauri stack) on Windows MSVC routinely
takes **2–10+ minutes per invocation**. A running cargo process under 10
minutes is almost certainly compiling, not hung.

- Do not cancel, retry, or re-invoke a cargo run under 10 minutes unless there is concrete evidence of a deadlock (zero CPU for minutes, stuck linker lock on a known-running `toaster-app.exe`).
- Scope narrowly during iteration; save the full sweep for milestone verification.

## Known Windows DLL pitfalls

- **`STATUS_ENTRYPOINT_NOT_FOUND (0xc0000139)` when the test binary launches.**
  Symptom: `cargo check -p toaster --lib` passes clean, linking succeeds, but
  `target\debug\deps\toaster_app_lib-*.exe` fails to start. Root cause seen
  repeatedly: `target\debug\deps\DirectML.dll` ends up as a 0-byte stub and
  clobbers the real copy in `target\debug\DirectML.dll` on the DLL search path.

  Diagnostic:

  ```powershell
  Get-ChildItem src-tauri\target\debug\deps\*.dll | Where-Object Length -lt 1024
  ```

  Recovery path (cheapest first):

  1. Re-run the monitored live app via `scripts\launch-toaster-monitored.ps1`
     — it uses a different target path and often sidesteps the issue.
  2. `cargo clean -p toaster` then rebuild.
  3. Only as last resort: identify missing exports via `dumpbin /dependents <exe>`.

  Do **not** burn another 5-minute rebuild retrying the exact same `cargo test`
  incantation — the hash will be identical and so will the failure.

## Style specifics

- `cargo fmt` before committing (ASCII-ordered imports, uppercase before lowercase).
- Doc comments on `pub` items.
- Prefer `let`-chains (`if let Some(x) = foo && x.is_valid()`) over nested `if let`.
- Prefer `#[expect(...)]` over `#[allow(...)]` when a clippy lint must be suppressed.
- `#[cfg(test)]` test-only imports should be marked `#[allow(unused_imports)]` when other tests gate their use behind additional cfgs.

## Example

```rust
// ✅ Good — explicit error handling, microsecond timestamps, no .unwrap()
pub fn get_word_at_time(words: &[Word], time_us: i64) -> Option<&Word> {
    words.iter().find(|w| w.start_us <= time_us && time_us < w.end_us)
}

// ❌ Bad — panics in production, seconds instead of microseconds
pub fn get_word_at_time(words: &[Word], time_s: f64) -> &Word {
    words.iter().find(|w| w.start <= time_s && time_s < w.end).unwrap()
}
```

## Related

- [`../src/AGENTS.md`](../src/AGENTS.md) — frontend / TypeScript conventions.
- Root [AGENTS.md](../AGENTS.md) "Skills and agents" — `transcript-precision-eval`, `audio-boundary-eval`, `transcription-adapter-contract`, `dep-hygiene`.
- [`../.github/instructions/code-review.instructions.md`](../.github/instructions/code-review.instructions.md) — Toaster-specific review gates (Critical on architecture violations, dual-path duplication, missing verification).
