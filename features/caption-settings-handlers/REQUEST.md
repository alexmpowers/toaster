# Feature request: Caption settings handlers

## 1. Problem & Goals

Nine flat `caption_*` fields live on `AppSettings`
(`src-tauri/src/settings/types.rs:295-312`). Four of them have
dedicated typed Tauri command handlers
(`change_caption_font_size_setting`, `change_caption_bg_color_setting`,
`change_caption_text_color_setting`, `change_caption_position_setting`
at `src-tauri/src/commands/app_settings.rs:118-152`) and corresponding
entries in the frontend `settingUpdaters` dispatch map
(`src/stores/settingsStore.ts:119-127`).

The other **five** flat caption styling keys —
`caption_font_family`, `caption_radius_px`, `caption_padding_x_px`,
`caption_padding_y_px`, `caption_max_width_percent` — have no typed
backend command and no `settingUpdaters` entry. When the settings
store's `updateSetting` path handles them (see the fallback at
`src/stores/settingsStore.ts:250-255`) it falls through to a
`console.warn("No handler for setting: …")` and only mutates the
optimistic in-memory Zustand slice — the value is never persisted to
`settings.json`.

Goal: close the asymmetry called out in `features/product-map-v1/PRD.md`
§6 Milestone 1 item 1.2 ("Add backend command handlers for the 5
caption styling keys") by adding typed `change_caption_<field>_setting`
commands for each of the five keys, wiring them into the
`settingUpdaters` map, and adding specta-exported bindings so the
frontend and backend agree at the type level.

## 2. Desired Outcome & Acceptance Criteria

- Each of the five unhandled caption styling keys has a dedicated
  `change_caption_<field>_setting` Tauri command that mirrors the
  existing pattern at `src-tauri/src/commands/app_settings.rs:118-152`.
- Each command validates its input against the same clamp/enum range
  used by `ensure_caption_defaults`
  (`src-tauri/src/settings/defaults.rs:588-603`) so a bogus value from
  the frontend cannot corrupt `settings.json`.
- The frontend `settingUpdaters` map
  (`src/stores/settingsStore.ts:72-127`) gets five new entries, one
  per key, invoking the typed command.
- No call-site in `src/` invokes a generic setter for any of the five
  keys; `grep -rn` for the literal key strings shows only reads and
  the new typed write paths.
- `settings.json` files written by an older build (with any or all of
  these five keys present under their existing flat names) continue to
  deserialize cleanly; storage shape does not change.
- `src/bindings.ts` exposes the five new commands after a
  `cargo tauri dev` debug build (specta regeneration) without any
  manual edits beyond the AGENTS.md one-line-per-field exception.

## 3. Scope Boundaries

### In scope

- Add five typed backend commands under
  `src-tauri/src/commands/app_settings.rs` following the existing
  caption-handler pattern.
- Register all five in the Tauri `invoke_handler` allowlist and the
  specta builder in `src-tauri/src/lib.rs` (alongside the four
  existing `change_caption_*_setting` entries at
  `src-tauri/src/lib.rs:213-216`).
- Wire the five new commands into the frontend `settingUpdaters` map
  in `src/stores/settingsStore.ts`.
- Update the per-key clamp/enum ranges once, in the typed command, so
  the range lives next to the handler — cite
  `ensure_caption_defaults` as the prior art but do not remove the
  sanitization pass (it still guards on-disk `settings.json` loads).
- Round-trip test in `src-tauri/src/commands/app_settings.rs` test
  module: call each typed command, read back via `get_settings`, assert
  equality and that out-of-range values are clamped (not rejected) to
  match existing caption handlers.

### Out of scope (explicit)

- Do **not** change `CaptionProfile` / `CaptionProfileSet` or the
  `set_caption_profile` command
  (`src-tauri/src/commands/captions.rs:63-92`). This bundle is purely
  flat-key command plumbing; the profile-struct setter migration is a
  separate concern.
- Do **not** touch caption layout math
  (`src-tauri/src/managers/captions/*`) or the preview/export
  byte-identical layout gate.
- Do **not** remove or rename any flat `caption_*` field on
  `AppSettings`; storage shape is frozen.
- Do **not** remove the sanitization clamps in
  `ensure_caption_defaults` — they remain the defense for older
  on-disk settings.
- Do **not** hand-edit `src/bindings.ts` beyond the AGENTS.md
  one-line-per-field exception.

## 4. References to Existing Code

- `src-tauri/src/commands/app_settings.rs:118-152` — the four existing
  typed caption handlers. New handlers mirror this shape exactly.
- `src-tauri/src/lib.rs:213-216` — the `invoke_handler!` /
  `specta::collect_commands!` entries for the existing four. The five
  new entries go here, in the same block.
- `src-tauri/src/settings/types.rs:295-312` — the flat
  `AppSettings.caption_*` fields and their `#[serde(default = …)]`
  defaults (storage shape is authoritative here).
- `src-tauri/src/settings/types.rs:398-408` — the `CaptionFontFamily`
  enum, already `specta::Type`; the `font_family` command takes this
  enum directly.
- `src-tauri/src/settings/defaults.rs:44-59` — per-key default
  functions for `radius_px`, `padding_x_px`, `padding_y_px`,
  `max_width_percent`.
- `src-tauri/src/settings/defaults.rs:585-603` — `ensure_caption_defaults`
  clamp ranges (radius ≤ 64, padding ≤ 128, max_width 20-100). New
  typed setters reuse these exact bounds.
- `src/stores/settingsStore.ts:72-127` — the `settingUpdaters` dispatch
  map. Five new entries go here, alongside the existing four.
- `src/stores/settingsStore.ts:250-255` — the `console.warn` fallback
  that makes silent-no-persist failure visible.

## 5. Edge Cases & Constraints

- Backwards compatibility: an existing `settings.json` that already
  contains any of the five flat keys (written by a prior build via
  the generic no-op path, unlikely in practice but possible via
  manual edit) must deserialize without error. Since the fields are
  already on `AppSettings` with `#[serde(default = …)]`, this is
  a structural invariant and the new commands do not change it.
- Clamp behavior must match `ensure_caption_defaults`: out-of-range
  numeric values are clamped (not rejected) so the frontend never
  receives a `Result::Err` for slider drag edge cases. The enum
  (`caption_font_family`) rejects unknown strings at serde
  deserialization time — frontend passes the typed
  `CaptionFontFamily` via specta so runtime rejection is impossible.
- i18n impact: none — the keys are non-user-visible internal setting
  names and already have labels rendered from other strings.
- Line-cap: `src-tauri/src/commands/app_settings.rs` is currently
  under the 800-line cap (need to verify — estimate +50 lines after
  this bundle); if the five new handlers push it over, split into
  `commands/app_settings/captions.rs`.
- No events: the existing four caption handlers do NOT emit
  `settings-changed` events
  (`src-tauri/src/commands/app_settings.rs:120-152`), only `debug_mode`
  and `update_checks_enabled` do (lines 67, 86). The five new handlers
  follow the caption precedent — no event emit unless a new side-effect
  is discovered during investigation.

## 6. Data Model

No data-model changes. The five flat fields already exist on
`AppSettings` with their `#[serde(default = …)]` annotations. The only
code surface growing is the command layer and the frontend dispatch
map.

| Key                           | Type                 | Clamp / domain              |
|-------------------------------|----------------------|-----------------------------|
| `caption_font_family`         | `CaptionFontFamily`  | enum: Inter, Roboto, SystemUi |
| `caption_radius_px`           | `u32`                | `min(value, 64)`            |
| `caption_padding_x_px`        | `u32`                | `min(value, 128)`           |
| `caption_padding_y_px`        | `u32`                | `min(value, 128)`           |
| `caption_max_width_percent`   | `u32`                | `clamp(value, 20, 100)`     |

## Q&A

- **Q8 (from `features/product-map-v1/PRD.md` §8): typed per setting
  or single generic setter?** → **Typed per setting.** Decided
  upstream; locked by the seed. Matches the existing pattern for the
  four already-handled caption keys and for every other settings page
  (`selected_language`, `debug_mode`, `update_checks_enabled`, etc.).
  One `change_caption_<field>_setting` per key, each doing its own
  clamp/validation, no grouped bulk setter.
