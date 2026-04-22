# Planned Release: v0.1.0

> **Status:** Draft — build infrastructure not yet ready. Publish once all checklist items below are green.

---

## Build Blockers

### 🍎 macOS (ARM + Intel)

**Error:** `security: SecKeychainItemImport: One or more parameters passed to a function were not valid`

**Cause:** `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, and `KEYCHAIN_PASSWORD` repo secrets are empty. The certificate import step fails before compilation starts.

**Fix:** Supply an Apple Developer ID certificate as a base64-encoded `.p12` in repo secrets (`APPLE_CERTIFICATE`), set the matching `APPLE_CERTIFICATE_PASSWORD`, `KEYCHAIN_PASSWORD`, `APPLE_ID`, `APPLE_TEAM_ID`, and `APPLE_ID_PASSWORD`. See [Tauri macOS signing guide](https://v2.tauri.app/distribute/sign/macos/).

---

### 🪟 Windows (x64 + ARM64)

**Error:** `failed to bundle project \`program path has no file name\``

**Cause:** The NSIS installer template (`src-tauri/nsis/installer.nsi`) contains a `File` or `ExecWait` directive whose computed path resolves to empty at bundle time.

**Fix:** Audit `src-tauri/nsis/installer.nsi` for empty computed paths. As a workaround, pass `--bundles msi` to skip NSIS and produce an MSI-only Windows installer while the template is debugged.

Windows code signing (`signCommand` in `tauri.conf.json`) is currently empty — unsigned builds will show SmartScreen warnings. See `docs/build.md#windows-code-signing` for signing setup with `signtool`.

---

### 🐧 Linux (x64 + ARM64)

**Error:** `failed to decode secret key: incorrect updater private key password: Missing comment in secret key`

**Cause:** `TAURI_SIGNING_PRIVATE_KEY` or `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` repo secret is missing, empty, or has a mismatched password.

**Fix:** Regenerate the keypair with `bunx tauri signer generate -w ~/.tauri/toaster.key`, store the private key + password as repo secrets, and update the `pubkey` in `src-tauri/tauri.conf.json` with the new public key.

---

## Planned Installer Formats

| Platform | Format | Status |
|---|---|---|
| Windows x64 | `.msi` + `.exe` (NSIS) | ❌ Blocked — NSIS bundler crash |
| Windows ARM64 | `.msi` + `.exe` (NSIS) | ❌ Blocked — NSIS bundler crash |
| macOS ARM (M1+) | `.dmg` | ❌ Blocked — no signing cert |
| macOS Intel | `.dmg` | ❌ Blocked — no signing cert |
| Linux x64 | `.deb` (Ubuntu 22.04) | ❌ Blocked — invalid signing key |
| Linux x64 | `.AppImage` + `.rpm` (Ubuntu 24.04) | ❌ Blocked — invalid signing key |
| Linux ARM64 | `.AppImage` + `.deb` + `.rpm` | ❌ Blocked — invalid signing key |

---

## Release Checklist

### Infrastructure (required before any installer ships)

- [ ] Rotate/add `TAURI_SIGNING_PRIVATE_KEY` + `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` secrets → unblocks all Linux targets
- [ ] Update `pubkey` in `src-tauri/tauri.conf.json` to match the new signing key
- [ ] Add Apple Developer ID certificate secrets → unblocks both macOS targets
- [ ] Fix NSIS template `program path has no file name` error → unblocks both Windows targets
- [ ] All 7 `Main Branch Build` CI jobs pass (workflow: `.github/workflows/main-build.yml`)

### Quality (required before public release)

- [ ] Regenerate export-parity eval baseline — see issue #5
- [ ] Repair Playwright e2e suite (24/36 tests failing) — see issue #4
- [ ] CI `continue-on-error: true` removed from `playwright.yml` once suite is green

### Release mechanics

- [ ] Bump version in `src-tauri/tauri.conf.json` (currently `0.1.0`) and `package.json` if needed
- [x] Update `plugins.updater.endpoints` URL in `tauri.conf.json` to point to `alexmpowers/toaster` (currently points to `itsnotaboutthecell/toaster`)
- [ ] Trigger `release.yml` (workflow_dispatch) — creates draft GitHub release, builds all platforms, uploads installers
- [ ] Verify all installer artifacts attached to the draft release
- [ ] Publish the draft release

---

## Notes

- The auto-updater pubkey in `tauri.conf.json` and the updater endpoint URL should both be updated to match this repo (`alexmpowers/toaster`) before publishing.
- First build after a clean clone takes 2–4 minutes due to whisper-rs-sys / Vulkan / ONNX compilation; CI Rust caches are keyed per-platform.
- Windows ARM64 uses a custom Vulkan SDK extraction path (humbletim/install-vulkan-sdk does not yet support ARM64 natively).
