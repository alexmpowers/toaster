# macOS Private API Usage

## Why `macOSPrivateApi: true`?

Toaster sets `"macOSPrivateApi": true` in `src-tauri/tauri.conf.json` (and enables the
`macos-private-api` Cargo feature in `src-tauri/Cargo.toml`) because the recording
overlay panel requires **NSPanel** functionality provided by
[tauri-nspanel](https://github.com/ahkohd/tauri-nspanel).

### What it enables

| Feature | Private API surface | Where used |
|---|---|---|
| **NSPanel overlay** | `tauri-nspanel` converts a standard Tauri window into an `NSPanel` — an Apple-private `NSWindow` subclass that can float above all spaces, ignore activation, and behave as a status-level overlay. | `src-tauri/src/overlay.rs` — `RecordingOverlayPanel` |
| **Transparent, always-on-top panel** | `PanelLevel::Status`, `CollectionBehavior::can_join_all_spaces()`, `no_activate(true)`, `transparent(true)` — these rely on private `NSPanel` / `NSWindow` APIs that Tauri only exposes when the private-API flag is on. | `overlay.rs` lines ~291-310 |
| **Tauri `macos-private-api` feature** | Tauri's own `macos-private-api` Cargo feature unlocks full-window transparency and additional `NSWindow` configuration hooks that the NSPanel plugin depends on. | `src-tauri/Cargo.toml` line 35 |

### Dependency chain

```
tauri.conf.json  →  "macOSPrivateApi": true      (tells Tauri runtime to enable private API access)
Cargo.toml       →  tauri features: ["macos-private-api"]  (compiles Tauri with private window APIs)
Cargo.toml       →  tauri-nspanel dependency      (macOS-only; converts windows to NSPanels)
overlay.rs       →  PanelBuilder / RecordingOverlayPanel   (creates the floating recording indicator)
lib.rs           →  tauri_nspanel::init() plugin   (registers the NSPanel plugin with the Tauri app)
```

## App Store implications

**Apple's Mac App Store will likely reject apps that use private APIs.** The App Review
process scans binaries for references to undocumented selectors and frameworks.
`tauri-nspanel` calls into private `NSPanel` APIs that would trigger rejection.

This is acceptable for Toaster because the app is distributed directly (outside the
App Store) via the built-in Tauri updater.

## Alternatives to avoid private APIs

If App Store distribution is ever needed, these approaches could replace the current
overlay implementation:

1. **Standard `NSWindow` with `level = .floating`** — Use a regular Tauri window with
   `always_on_top(true)` and `decorations(false)`. This loses some behaviors
   (e.g., `can_join_all_spaces`, ignoring activation focus) but avoids all private APIs.

2. **`NSStatusItem` menu-bar indicator** — Replace the floating overlay with a macOS
   menu-bar status item. No private APIs needed, but the UX is different (not
   positioned near the recording area).

3. **`CALayer` / overlay view** — Render the recording indicator as a native overlay
   view inside the main window rather than a separate panel. Limited to the main
   window's bounds.

4. **`accessibilityDisplayShouldDifferentiateWithoutColor`-safe standard window** —
   A standard Tauri WebviewWindow with `set_ignore_cursor_events(true)` and
   `always_on_top(true)`, accepting the trade-off that the window may steal focus
   briefly on show.

## Direct distribution (current approach)

For direct distribution (`.dmg` / `.app` bundle downloaded from the website or via
the Tauri updater), private API usage carries no restrictions. Apple only enforces
the private-API ban for App Store submissions; notarization alone does not reject
private API usage.
