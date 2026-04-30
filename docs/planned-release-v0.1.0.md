# Planned Release: v0.1.0

> **Status:** Release blockers cleared for first-pass v0.1.0. Linux artifacts already verified green; Windows builds will produce **unsigned** MSIs (SmartScreen warning is acceptable for a fresh-project first release). macOS is **deferred** — no Apple Developer certificate.

---

## Scope of v0.1.0 (first pass)

| Platform        | Architecture | Format       | Status   |
| --------------- | ------------ | ------------ | -------- |
| Linux (Debian)  | x64          | `.deb`       | ✅ green |
| Linux (Debian)  | ARM64        | `.deb`       | ✅ green |
| Linux (RPM)     | x64          | `.rpm`       | ✅ green |
| Linux (RPM)     | ARM64        | `.rpm`       | ✅ green |
| Linux (AppImage)| x64          | `.AppImage`  | ✅ green |
| Linux (AppImage)| ARM64        | `.AppImage`  | ✅ green |
| Windows         | x64          | `.msi`       | 🟡 fix applied — re-run pending |
| Windows         | ARM64        | `.msi`       | 🟡 fix applied — re-run pending |
| macOS ARM (M1+) | aarch64      | `.dmg`       | ⏸ deferred — needs Apple cert |
| macOS Intel     | x86_64       | `.dmg`       | ⏸ deferred — needs Apple cert |

---

## What was fixed for the first-pass

### 🐧 Linux — RESOLVED ✅

`TAURI_SIGNING_PRIVATE_KEY` + `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` repo secrets supplied; `pubkey` in `src-tauri/tauri.conf.json` updated to match. Run #6 produced clean `.deb` / `.rpm` / `.AppImage` artifacts on all 3 Linux runners.

### 🪟 Windows — FIX APPLIED 🟡

**Original error:** `failed to bundle project 'program path has no file name'` after the build itself completed successfully.

**Root cause (corrected):** Earlier diagnosis attributed this to the NSIS template, but that fix did not unblock subsequent runs. The actual root cause is an empty `"signCommand": ""` literal in `src-tauri/tauri.conf.json` `bundle.windows`. Tauri sees a configured custom signing command, splits the empty string for execution, and fails because the program path has no file name.

**Fix applied:** Removed the empty `bundle.windows.signCommand` (and the now-empty `bundle.windows` object) from `tauri.conf.json`. Tauri will skip Authenticode signing entirely, producing unsigned MSIs that install with a SmartScreen warning. Auto-updater integrity is unaffected — it uses minisign with the `pubkey` in `tauri.conf.json`.

**Follow-up (not blocking v0.1.0):** Wire `signCommand` to `trusted-signing-cli` (Azure Trusted Signing) or `signtool` (EV cert) for v0.1.1. The `build.yml` reusable workflow already installs `trusted-signing-cli@0.9.0` when `sign-binaries: true`, so the rewire is small.

### 🍎 macOS — DEFERRED ⏸

Removed both macOS rows (`macos-26`, `macos-latest`) from the `release.yml` `publish-tauri` matrix. `build.yml` is unchanged, so re-adding macOS support after obtaining an Apple Developer ID certificate is a one-line revert.

---

## Release Checklist

### Infrastructure (required before any installer ships)

- [x] Rotate/add `TAURI_SIGNING_PRIVATE_KEY` + `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` secrets → Linux signing green
- [x] Update `pubkey` in `src-tauri/tauri.conf.json` to match the new signing key
- [ ] ~~Add Apple Developer ID certificate secrets~~ → **deferred to v0.1.1+**
- [x] Fix Windows bundling (`signCommand` empty-string trap) → unblocks both Windows targets
- [x] All required `Main Branch Build` CI jobs pass on the latest commit

### Quality (required before public release)

- [ ] Regenerate export-parity eval baseline — see issue #5 (non-blocking, separate issue)
- [x] Repair Playwright e2e suite (24/36 tests failing) — issue #4 **FIXED**, all 35/35 passing
- [x] CI `continue-on-error: true` removed from `playwright.yml`

### Release mechanics

- [x] Bump version in `src-tauri/tauri.conf.json` (currently `0.1.0`)
- [x] Update `plugins.updater.endpoints` URL in `tauri.conf.json` to point to `alexmpowers/toaster`
- [ ] Trigger `release.yml` (workflow_dispatch) — creates draft GitHub release, builds Linux + Windows, uploads installers
- [ ] Verify all 8 installer artifacts attached to the draft release (6 Linux + 2 Windows + per-artifact `.sig` + `latest.json`)
- [ ] Publish the draft release → tags `v0.1.0` → auto-updater endpoint goes live

---

## Notes

- **Auto-updater pubkey + endpoint** in `tauri.conf.json` already point at `alexmpowers/toaster`.
- First Windows build after a clean clone takes 15+ minutes due to whisper-rs-sys + Vulkan + ONNX compilation; CI Rust caches are keyed per-platform and warm runs are faster.
- Windows ARM64 uses a custom Vulkan SDK extraction path (`humbletim/install-vulkan-sdk` does not yet support ARM64 natively).
- Run #6 confirmed Windows builds **complete successfully** (~15 min compile) — only the post-build sign step failed. With the `signCommand` removed, the bundling step should now produce MSIs cleanly.

---

## Out of scope for v0.1.0

- macOS builds (Apple cert required)
- Windows code signing (Azure Trusted Signing or EV cert + signtool)
- Stale `handy` references in `build.yml` macOS-only steps and `nix-check.yml` flake outputs
- Export-parity eval baseline regeneration (issue #5)
