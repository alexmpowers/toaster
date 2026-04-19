-- Task graph for caption-settings-handlers.
-- Ingest into the session SQL store with the `sql` tool.

-- Schema: todos(id TEXT, title TEXT, description TEXT, status TEXT).
-- Allowed status values: 'pending', 'in_progress', 'done', 'blocked'.
-- todo_deps schema: (todo_id TEXT, depends_on TEXT).

INSERT INTO todos (id, title, description, status) VALUES
  ('csh-backend-handlers',
   'Add five typed caption command handlers',
   'Add change_caption_font_family_setting, change_caption_radius_px_setting, change_caption_padding_x_px_setting, change_caption_padding_y_px_setting, change_caption_max_width_percent_setting to src-tauri/src/commands/app_settings.rs following the pattern at lines 118-152. Register all five in invoke_handler! and specta::collect_commands! blocks in src-tauri/src/lib.rs (near lines 213-216). Each numeric handler clamps per ensure_caption_defaults (defaults.rs:588-603); enum handler accepts CaptionFontFamily directly. Embed `// mirrors ensure_caption_defaults:<line>` comments. Verifier: AC-001-a.',
   'pending'),
  ('csh-roundtrip-tests',
   'Add write-then-read + clamp round-trip tests',
   'In src-tauri/src/commands/app_settings.rs tests module, add unit tests per key: construct a test AppHandle, call the typed command with in-range and out-of-range values, then call settings::get_settings and assert the persisted field matches (with clamp for OOR inputs). Verifiers: AC-001-b, AC-001-c.',
   'pending'),
  ('csh-frontend-dispatch',
   'Wire five settingUpdaters entries',
   'Add five entries to the settingUpdaters map in src/stores/settingsStore.ts (near lines 119-127): each maps one caption_* key to commands.change<…>Setting. No other file changes. Verifier: AC-002-a.',
   'pending'),
  ('csh-specta-regen',
   'Regenerate bindings.ts via cargo tauri dev debug build',
   'Run `cargo tauri dev` once in debug mode so specta regenerates src/bindings.ts with the five new commands. Confirm each command name is present via rg. Do NOT hand-edit bindings.ts beyond the AGENTS.md one-line-per-field exception (not needed here — pure addition). Verifier: AC-002-c.',
   'pending'),
  ('csh-backcompat-test',
   'Backward-compat fixture-load test',
   'Add a test that deserializes two settings.json fixtures: (1) pre-bundle shape with all five keys present at legacy values, (2) pre-bundle shape with keys absent (serde defaults fill in). Assert both load without error and fields equal expected values. Verifiers: AC-003-a, AC-003-b.',
   'pending'),
  ('csh-liveapp-pass',
   'Live-app smoke: no warn, persistence across relaunch',
   'Run scripts/launch-toaster-monitored.ps1 -ObservationSeconds 180. Open devtools console. Navigate to Advanced -> Captions. Change each of the five controls. Confirm NO `No handler for setting: caption_…` warnings. Close and relaunch the app; confirm each of the five values persisted. Verifier: AC-002-b.',
   'pending'),
  ('csh-static-gates',
   'Static gates green',
   'Run `cargo test -p toaster --tests --no-run` (AC-004-a), `npm run lint` (AC-004-b via scripts/lint-and-build-gate.ps1 stub until implementation lands), `bun scripts/check-translations.ts` (AC-004-c), `bun run check:file-sizes` / `scripts/check-file-sizes.ts` (AC-004-d). If app_settings.rs trips the 800-line cap, execute the BLUEPRINT-prescribed submodule split into src-tauri/src/commands/app_settings/captions.rs and re-run the gate.',
   'pending'),
  ('csh-qc',
   'QC: run coverage + tasks gates',
   'Run `pwsh scripts/feature/check-feature-coverage.ps1 -Feature caption-settings-handlers` and `pwsh scripts/feature/check-feature-tasks.ps1 -Feature caption-settings-handlers`; both must exit 0. Then run eval-harness-runner to pick up any cross-cutting regression.',
   'pending');

INSERT INTO todo_deps (todo_id, depends_on) VALUES
  ('csh-roundtrip-tests', 'csh-backend-handlers'),
  ('csh-backcompat-test', 'csh-backend-handlers'),
  ('csh-specta-regen', 'csh-backend-handlers'),
  ('csh-frontend-dispatch', 'csh-specta-regen'),
  ('csh-liveapp-pass', 'csh-frontend-dispatch'),
  ('csh-liveapp-pass', 'csh-roundtrip-tests'),
  ('csh-liveapp-pass', 'csh-backcompat-test'),
  ('csh-static-gates', 'csh-frontend-dispatch'),
  ('csh-static-gates', 'csh-roundtrip-tests'),
  ('csh-qc', 'csh-liveapp-pass'),
  ('csh-qc', 'csh-static-gates');
