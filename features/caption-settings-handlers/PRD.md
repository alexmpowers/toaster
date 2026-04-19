# PRD: Caption settings handlers

## Problem & Goals

Five of the nine flat `caption_*` fields on `AppSettings`
(`src-tauri/src/settings/types.rs:295-312`) have no typed backend
command. The frontend `settingUpdaters` map
(`src/stores/settingsStore.ts:72-127`) falls through to a
`console.warn("No handler for setting: …")` for those keys, leaving
the optimistic Zustand update in place but never persisting to
`settings.json`. This bundle adds typed `change_caption_<field>_setting`
handlers for the five keys so every caption styling setting persists
through a typed, specta-exported command, closing Milestone 1 item 1.2
of `features/product-map-v1/PRD.md`.

## Scope

### In scope

- Add five typed Tauri commands in
  `src-tauri/src/commands/app_settings.rs`:
  `change_caption_font_family_setting`,
  `change_caption_radius_px_setting`,
  `change_caption_padding_x_px_setting`,
  `change_caption_padding_y_px_setting`,
  `change_caption_max_width_percent_setting`.
- Register them in `src-tauri/src/lib.rs` `invoke_handler!` and
  `specta::collect_commands!` alongside the existing four.
- Add five `settingUpdaters` entries in
  `src/stores/settingsStore.ts`.
- Unit-test round-trip (write via typed command → read via
  `get_settings` → equal) and clamp behavior in the
  `src-tauri/src/commands/app_settings.rs` test module.
- Regenerate `src/bindings.ts` via `cargo tauri dev` debug build.

### Out of scope (explicit)

- `CaptionProfile` / `CaptionProfileSet` and
  `commands::captions::set_caption_profile` — untouched.
- Caption layout math / preview / export — untouched.
- Flat-field deprecation or rename — storage shape frozen.
- Removal of `ensure_caption_defaults` clamps — retained as the
  on-disk defense.

## Requirements

### R-001 — Typed command per unhandled caption styling key

- Description: Add one `#[tauri::command] #[specta::specta]` function
  per key, mirroring
  `change_caption_font_size_setting`
  (`src-tauri/src/commands/app_settings.rs:118-125`). Each reads the
  current settings, mutates the named field after clamp/enum
  validation, writes via `settings::write_settings`, returns `Ok(())`.
- Rationale: Restores symmetry with the other settings pages and the
  four already-typed caption handlers. Removes the silent-no-persist
  failure mode for the five remaining keys.
- Acceptance Criteria
  - AC-001-a — Five new commands exist in
    `src-tauri/src/commands/app_settings.rs`, each matching the
    `app: AppHandle, <value>: <T> -> Result<(), String>` signature of
    the existing four caption handlers, and each is wired into both
    the `invoke_handler!` and `specta::collect_commands!` blocks in
    `src-tauri/src/lib.rs`.
  - AC-001-b — Each numeric command clamps its input to the exact
    range used in `ensure_caption_defaults`
    (`src-tauri/src/settings/defaults.rs:588-603`):
    radius `min(v, 64)`, padding_x `min(v, 128)`,
    padding_y `min(v, 128)`, max_width_percent `clamp(v, 20, 100)`.
    The enum command accepts `CaptionFontFamily` directly — serde
    rejects unknown variants at deserialization time.
  - AC-001-c — A `cargo test -p toaster` round-trip test for each of
    the five keys calls the typed command via the test-mode
    `AppHandle` helper, then reads `settings::get_settings` and
    asserts the field equals the written value (or the clamped
    value, for out-of-range inputs).

### R-002 — Frontend routes the five keys through the typed path

- Description: Register five entries in the `settingUpdaters` map
  (`src/stores/settingsStore.ts:72-127`), one per key, each calling
  the corresponding specta-generated `commands.change<…>Setting`.
  Remove no existing entries. The generic `console.warn` fallback
  stays in place for other future mistakes.
- Rationale: With these entries present, the existing
  `updateSetting(key, value)` call-site in `CaptionSettings.tsx`
  (unchanged) routes through the typed command and persists. No
  behavior change to any consumer; only the plumbing differs.
- Acceptance Criteria
  - AC-002-a — The `settingUpdaters` map in
    `src/stores/settingsStore.ts` contains keys
    `caption_font_family`, `caption_radius_px`,
    `caption_padding_x_px`, `caption_padding_y_px`,
    `caption_max_width_percent`, each invoking the matching
    specta-generated command.
  - AC-002-b — `grep -rn "No handler for setting" src/` prints only
    the single warn line in `settingsStore.ts`; after this bundle a
    manual drag of each of the five caption controls in the live app
    logs no `No handler for setting: caption_…` warning.
  - AC-002-c — `src/bindings.ts` (post `cargo tauri dev` debug build)
    exposes a specta entry for each of the five new commands; `grep`
    the file for the five command names returns five matches.

### R-003 — Storage shape and backward compatibility

- Description: The flat `AppSettings.caption_*` fields keep their
  existing `#[serde(default = …)]` annotations
  (`src-tauri/src/settings/types.rs:295-312`). No field renames, no
  new fields, no removals. An existing `settings.json` (written by any
  prior build) deserializes cleanly and the new typed commands
  continue to read/write the same field positions.
- Rationale: Storage-shape stability is a mandatory constraint from
  the seed and from AGENTS.md.
- Acceptance Criteria
  - AC-003-a — A fixture `settings.json` containing the pre-bundle
    shape (with and without each of the five keys present) loads via
    `settings::get_settings` without panic or error; the test asserts
    field values after load equal the on-disk values (or defaults,
    when absent).
  - AC-003-b — `diff` of `src-tauri/src/settings/types.rs` for this
    bundle shows no changes to the `caption_*` field declarations or
    their `#[serde(default = …)]` annotations (verified by reviewer
    inspecting the blueprint's "Migration / compatibility" section).

### R-004 — Static gates stay green

- Description: The whole-tree gates listed in AGENTS.md keep passing.
- Rationale: Hard rule from the seed.
- Acceptance Criteria
  - AC-004-a — `cargo check -p toaster --tests` exits 0.
  - AC-004-b — `npm run lint` exits 0.
  - AC-004-c — `bun scripts/check-translations.ts` exits 0 (no i18n
    keys added or removed; parity trivially holds).
  - AC-004-d — `bun run check:file-sizes` exits 0; if
    `src-tauri/src/commands/app_settings.rs` exceeds 800 lines the
    BLUEPRINT-prescribed split into a `captions.rs` submodule is
    performed and the gate passes post-split.

## Edge cases & constraints

- Clamp-rather-than-reject matches the existing four caption handlers
  so slider drags cannot error out at range boundaries.
- The enum field (`caption_font_family`) relies on serde + specta to
  reject invalid variants before the handler is invoked — no runtime
  check needed.
- `ensure_caption_defaults` remains the load-time defense for
  hand-edited `settings.json` files; the new typed setters are the
  write-time defense for live-app interactions.
- No `settings-changed` event is emitted; caption styling changes are
  picked up via the frontend's own optimistic Zustand update and on
  next `refreshSettings`. This matches the existing four caption
  handlers.

## Data model (if applicable)

No changes. See `REQUEST.md` §6 for the field/type/clamp table.

## Non-functional requirements

- File-size cap of 800 lines on `.rs` / `.ts` / `.tsx`. If
  `app_settings.rs` tips over, split per BLUEPRINT.
- i18n parity across 20 locales — trivially maintained (no keys
  added or removed).
- No new crate or npm dependency.
- Local-only inference invariant unaffected.
