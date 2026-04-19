# PRD: unified model catalog

## Problem & Goals

Post-processor (GGUF LLM) models were added via the
`local-llm-model-catalog` feature as a parallel subsystem:
`managers/llm/catalog.rs` + `managers/llm/download.rs` +
`src/components/settings/post-processing/local-models/LlmModelCatalog.tsx`.
The original Models page and the `managers/model/` pipeline know
nothing about them. This violates AGENTS.md "Single source of truth
for dual-path logic": two catalogs, two download pipelines, two UIs,
two event shapes, all for the same concept (a curated, sha-verified
local model asset).

Unify them. One registry, one `ModelInfo`, one download pipeline, one
UI. A `category` discriminator field + optional per-category metadata
blocks resolve "transcripts vs post-processor" without bifurcating the
type system.

## Scope

### In scope

- `ModelCategory` enum extension + `ModelInfo` metadata-block shape.
- Migration of GGUF catalog entries from `managers/llm/catalog.rs`
  into `managers/model/catalog.rs`.
- Deletion of `managers/llm/catalog.rs` and `managers/llm/download.rs`.
- Rewire of `managers/llm/inference.rs` + `mod.rs` against the
  unified catalog.
- Unified `ModelDownloadProgress` event shape with `category` field.
- Tauri command surface consolidation + one-release deprecation shim.
- Models settings page category filter + per-card badge.
- Post Processing settings page local-models picker rewire.
- Settings migration for `post_process_local_model_id`.
- i18n: 20 locale files updated (English authoritative).
- Regenerate `src/bindings.ts`.

### Out of scope (explicit)

- Inference runtime logic in `managers/llm/inference.rs`.
- New model entries (migration only).
- Hosted inference — remains forbidden per AGENTS.md.
- Transcription adapter contract changes.
- Audio / export / captions.

## Requirements

### R-001 — Extend `ModelCategory` with `PostProcessor` variant

- Description: Add a `PostProcessor` variant to the `ModelCategory`
  enum in `src-tauri/src/managers/model/mod.rs:29-34`. Naming decision
  documented in `BLUEPRINT.md` "Naming decisions".
- Rationale: Single discriminator replaces the need for a parallel
  `LlmCatalogEntry` type.
- Acceptance Criteria
  - AC-001-a — The `ModelCategory` enum in
    `src-tauri/src/managers/model/mod.rs` has exactly three variants:
    `Transcription`, `PostProcessor`, `System`.
  - AC-001-b — `cargo test --manifest-path src-tauri/Cargo.toml` passes
    with zero compilation errors.

### R-002 — Migrate GGUF entries into unified catalog

- Description: Every `LlmCatalogEntry` previously declared in
  `src-tauri/src/managers/llm/catalog.rs` is expressed as a
  `ModelInfo` with `category = PostProcessor` inside
  `src-tauri/src/managers/model/catalog.rs` (or a split submodule
  under `managers/model/catalog/`).
- Rationale: Single registry; eliminates the parallel pipeline.
- Acceptance Criteria
  - AC-002-a — The count of entries in the unified catalog with
    `category = PostProcessor` equals the count of entries previously
    in `managers/llm/catalog.rs` (validated by a cargo test).
  - AC-002-b — `managers/llm/catalog.rs` and
    `managers/llm/download.rs` do not exist in the repo after the
    migration.

### R-003 — `ModelInfo` carries optional per-category metadata blocks

- Description: `ModelInfo` gains
  `transcription_metadata: Option<TranscriptionMetadata>` and
  `llm_metadata: Option<LlmMetadata>` per the data model in
  `REQUEST.md` section 6.
- Rationale: Preserves LLM-specific fields (quantization, context
  length, RAM recommendation, prompt template) without polluting the
  transcription fast path.
- Acceptance Criteria
  - AC-003-a — A cargo test asserts every catalog entry with
    `category = PostProcessor` has `llm_metadata.is_some()` and every
    entry with `category = Transcription` has
    `transcription_metadata.is_some()`.
  - AC-003-b — Legacy serialized `ModelInfo` JSON (without the new
    fields) deserializes successfully with `transcription_metadata`
    and `llm_metadata` both defaulting to `None` (covered by a
    `#[serde(default)]` + cargo test).

### R-004 — Single download pipeline under `managers/model/download.rs`

- Description: All model downloads, regardless of category, flow
  through `managers/model/download.rs`, which in turn invokes
  `managers/model/hash.rs` for sha256 verification.
- Rationale: One code path means one set of tests, one set of events,
  one bug surface.
- Acceptance Criteria
  - AC-004-a — Ripgrep for `fn download` under
    `src-tauri/src/managers/llm/` returns zero results.
  - AC-004-b — A cargo test downloads a fixture post-processor entry
    through the unified pipeline and asserts the resulting file's
    sha256 matches the catalog entry's recorded hash.

### R-005 — Unified `ModelDownloadProgress` event shape

- Description: The Tauri event emitted during downloads has a single
  shape that includes `{ id, category, downloaded_bytes, total_bytes,
  status }`. Both categories emit on the same channel.
- Rationale: Frontend routes by category; no second event channel to
  maintain.
- Acceptance Criteria
  - AC-005-a — `grep -rn 'ModelDownloadProgress' src-tauri/src`
    reports exactly one struct definition, and its serde shape
    includes a `category` field.
  - AC-005-b — During a live-app post-processor download, the
    frontend receives progress events on the unified channel and the
    progress bar advances to 100% on completion.

### R-006 — Tauri command consolidation + deprecation shim

- Description: New commands `download_model(id, category)`,
  `get_models(category?)`, `delete_model(id)` replace the `*_llm_*`
  variants. The old names are retained for one release as thin shims
  that forward with `category = PostProcessor` and log a deprecation
  warning.
- Rationale: Smaller command surface; external scripts and in-flight
  dev builds keep working.
- Acceptance Criteria
  - AC-006-a — Invoking the deprecation-shim command
    `download_llm_model` from the frontend returns the same result as
    `download_model(id, PostProcessor)` and emits a single
    `warn!`-level log line mentioning "deprecated".
  - AC-006-b — `src/bindings.ts` regenerates cleanly and TypeScript
    consumers can call both the new and shimmed commands without
    type errors (`npm run build` passes).

### R-007 — Unified Models settings page with category filter + badges

- Description: `ModelsSettings.tsx` renders both categories in a
  single grouped list, with a segmented-control filter
  (`All | Transcription | Post-processing`) above the list and a
  category badge on every card. Color tokens follow the existing
  design system (rest `#EEEEEE`, accent orange on hover for the
  Post-processing pill).
- Rationale: One UI surface satisfies the "single source of truth"
  rule and surfaces the full inventory per the user request.
- Acceptance Criteria
  - AC-007-a — With the filter set to `All`, the Models page shows at
    least one card with a "Transcription" badge and at least one
    with a "Post-processing" badge.
  - AC-007-b — Clicking the Post-processing filter hides every card
    whose badge is not "Post-processing".
  - AC-007-c — No badge or filter label uses the raw enum token
    `PostProcessor`; all user-visible copy uses "Post-processing".

### R-008 — Post Processing page picker embeds the unified component

- Description: `PostProcessingSettings.tsx`'s local-models picker is
  replaced by (or reduced to a thin wrapper around) the unified
  `ModelsSettings.tsx` with the filter locked to Post-processing. A
  "Manage models" affordance deep-links to the full Models page with
  the same filter pre-applied.
- Rationale: Eliminates the duplicated picker UI in
  `local-models/LlmModelCatalog.tsx`.
- Acceptance Criteria
  - AC-008-a — `src/components/settings/post-processing/local-models/LlmModelCatalog.tsx`
    either does not exist or its file body is <= 40 lines of
    delegation to the unified component.
  - AC-008-b — In the live app, selecting a post-processor model on
    the Post Processing page persists a valid
    `post_process_local_model_id` that, on app restart, resolves to
    the same model.

### R-009 — Settings migration preserves user selection

- Description: On load, if `AppSettings.post_process_local_model_id`
  references a legacy id, `settings.rs` remaps it to the corresponding
  unified id (expected 1:1 since ids are preserved).
- Rationale: Users who had selected a post-processor model must not
  find their selection reset after upgrade.
- Acceptance Criteria
  - AC-009-a — A cargo test with a fixture `AppSettings` JSON that
    contains a legacy `post_process_local_model_id` produces a loaded
    settings object whose `post_process_local_model_id` resolves to a
    valid entry in the unified catalog.

### R-010 — i18n parity across 20 locales

- Description: Every new or renamed user-visible key
  (filter labels, badge copy, "Manage models" link, deprecation-era
  notices if any) exists in all 20 `src/i18n/locales/*/translation.json`
  files. English is authoritative; other locales mirror the English
  value as a placeholder per `i18n-pruning`.
- Rationale: `scripts/check-translations.ts` is a CI gate.
- Acceptance Criteria
  - AC-010-a — `bun run scripts/check-translations.ts` exits 0 after
    the changes land.

### R-011 — No new network paths; local-only inference preserved

- Description: The refactor introduces no new runtime network fetch
  beyond the existing user-initiated curated-download path. No new
  npm or cargo dependency that phones home.
- Rationale: AGENTS.md "Local-only inference" is non-negotiable.
- Acceptance Criteria
  - AC-011-a — The `dep-hygiene` skill is invoked against the diff
    and reports no new network-dependent crate or package (cargo
    machete / knip / depcheck output cited in the verifier).

### R-012 — File-size cap preserved

- Description: No new `.rs` or `.tsx` file under `src/` or
  `src-tauri/src/` exceeds 800 lines. If `managers/model/catalog.rs`
  threatens the cap after migration, split into
  `managers/model/catalog/mod.rs` +
  `managers/model/catalog/transcription.rs` +
  `managers/model/catalog/post_processor.rs`.
- Rationale: AGENTS.md "Conventions" file-size rule; enforced by CI.
- Acceptance Criteria
  - AC-012-a — `bun run check:file-sizes` exits 0 after the changes
    land.

### R-013 — Live-app verification of the unified Models page

- Description: The refactor is not considered done until a human
  driver launches the app via
  `scripts\launch-toaster-monitored.ps1 -ObservationSeconds 300`,
  exercises the Models page and the Post Processing page's picker,
  and confirms the observable behavior in the AC-013 steps.
- Rationale: AGENTS.md "Verified means the live app, not
  `cargo check`".
- Acceptance Criteria
  - AC-013-a — Live-app walkthrough (filter switching, download
    start/cancel, badge copy, deep-link from Post Processing)
    completes without runtime errors or visible regressions per the
    numbered steps in `coverage.json`.

## Edge cases & constraints

- File-size cap may force a catalog split (R-012).
- Deprecation shim must log exactly once per invocation to avoid log
  spam (enforce via a warn-level log line).
- Migration must be idempotent — running twice on the same settings
  file must not churn the value.
- Backwards-compatible serde: legacy `ModelInfo` JSON missing the new
  `transcription_metadata` / `llm_metadata` fields must deserialize
  with those as `None`.

## Data model (if applicable)

See `REQUEST.md` section 6 for the full struct sketch.

## Non-functional requirements

- `cargo check` + `cargo test` must pass on Windows MSVC toolchain.
- No clippy regressions (`cargo clippy` clean on touched crates).
- No new `.unwrap()` on production paths (AGENTS.md "Conventions").
- UI: segmented control supports keyboard navigation and mouse;
  badges meet accessibility contrast thresholds per the Settings UI
  contract in AGENTS.md.
