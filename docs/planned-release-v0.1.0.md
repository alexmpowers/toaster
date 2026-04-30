# Planned Release: v0.1.0

> **Status:** ✅ **RELEASED** — `v0.1.0` published 2026-04-30. All 17 artifacts (8 binaries + 8 minisign sigs + `latest.json`) live at <https://github.com/alexmpowers/toaster/releases/tag/v0.1.0>.

---

## Shipped artifacts

| Platform        | Architecture | Format       | Status     |
| --------------- | ------------ | ------------ | ---------- |
| Linux (Debian)  | x64          | `.deb`       | ✅ shipped |
| Linux (Debian)  | ARM64        | `.deb`       | ✅ shipped |
| Linux (RPM)     | x64          | `.rpm`       | ✅ shipped |
| Linux (RPM)     | ARM64        | `.rpm`       | ✅ shipped |
| Linux (AppImage)| x64          | `.AppImage`  | ✅ shipped |
| Linux (AppImage)| ARM64        | `.AppImage`  | ✅ shipped |
| Windows         | x64          | `.msi`       | ✅ shipped (unsigned — SmartScreen warning) |
| Windows         | ARM64        | `.msi`       | ✅ shipped (unsigned — SmartScreen warning) |
| macOS ARM (M1+) | aarch64      | `.dmg`       | ⏸ deferred — needs Apple cert |
| macOS Intel     | x86_64       | `.dmg`       | ⏸ deferred — needs Apple cert |

Auto-updater manifest: <https://github.com/alexmpowers/toaster/releases/latest/download/latest.json> (12 platform entries, minisign-verified).

---

## What was fixed for v0.1.0

### 🐧 Linux — RESOLVED ✅

`TAURI_SIGNING_PRIVATE_KEY` + `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` repo secrets supplied; `pubkey` in `src-tauri/tauri.conf.json` updated to match.

### 🪟 Windows — RESOLVED ✅

**Original error in run #6:** `failed to bundle project 'program path has no file name'` after the build itself completed (~15 min).

**Root cause:** Earlier diagnosis blamed the NSIS installer template, but removing NSIS did not unblock subsequent runs. The actual cause was an empty `"signCommand": ""` literal in `src-tauri/tauri.conf.json` `bundle.windows`. Tauri saw a configured custom signing command, split the empty string for execution, and crashed because the program path had no file name.

**Fix in commit `2d8ad97`:** Removed `bundle.windows.signCommand` (and the now-empty `bundle.windows` object) from `tauri.conf.json`. Tauri skips Authenticode signing entirely; MSIs ship unsigned with a SmartScreen "Unknown publisher" prompt the first time. Auto-updater integrity is unaffected — it uses minisign with the `pubkey`, which is independent of Authenticode.

**Follow-up (v0.1.1+):** Wire `signCommand` to `trusted-signing-cli` (Azure Trusted Signing) or `signtool` (EV cert). The `build.yml` reusable workflow already installs `trusted-signing-cli@0.9.0` when `sign-binaries: true`, so the rewire is small.

### 🍎 macOS — DEFERRED ⏸

In commit `2d8ad97`, removed both macOS rows (`macos-26`, `macos-latest`) from the `release.yml` `publish-tauri` matrix. `build.yml` is unchanged, so re-adding macOS support after obtaining an Apple Developer ID certificate is a one-line revert.

---

## Release Checklist (final)

### Infrastructure

- [x] `TAURI_SIGNING_PRIVATE_KEY` + password secrets supplied → Linux signing green
- [x] `pubkey` in `src-tauri/tauri.conf.json` updated to match the new signing key
- [ ] ~~Add Apple Developer ID certificate secrets~~ → **deferred to v0.1.1+**
- [x] Fix Windows bundling (`signCommand` empty-string trap removed) → Windows builds green
- [x] All required CI jobs pass on the latest commit

### Quality

- [ ] Regenerate export-parity eval baseline — see issue #5 (non-blocking)
- [x] Repair Playwright e2e suite — issue #4 **FIXED**, all 35/35 passing
- [x] CI `continue-on-error: true` removed from `playwright.yml`

### Release mechanics

- [x] Version `0.1.0` in `src-tauri/tauri.conf.json`
- [x] `plugins.updater.endpoints` URL points to `alexmpowers/toaster`
- [x] Triggered `release.yml` (run `25176868106`, commit `2d8ad97`) — all 7 jobs green
- [x] All 17 installer artifacts attached to the draft (6 Linux + 2 Windows + 8 sigs + `latest.json`)
- [x] `latest.json` validated locally — 12 platform entries
- [x] Windows MSI structurally valid (OLE compound magic bytes)
- [x] Stale draft releases cleaned up (3 deleted)
- [x] Draft promoted to **published** at 2026-04-30 16:54 UTC, marked `Latest`
- [x] Updater endpoint resolves via HTTPS, returns valid manifest

---

## Out of scope for v0.1.0 (logged for v0.1.1+)

- macOS builds (Apple Developer cert + signing/notarization secrets required)
- Windows code signing (Azure Trusted Signing or EV cert + signtool)
- Stale `handy` references in `build.yml` macOS-only steps and `nix-check.yml` flake outputs
- Export-parity eval baseline regeneration (issue #5)

