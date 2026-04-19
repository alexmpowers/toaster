# Blueprint: unified model catalog

## Naming decisions

The user explicitly delegated category naming to the PM. Decisions:

### Enum variant: `PostProcessor`

Candidates considered:

| Candidate       | Verdict | Reason |
|-----------------|---------|--------|
| `PostProcessor` | **chosen** | Matches the existing `PostProcessingSettings.tsx` surface name; future-proof for non-LLM post-processors (rule-based filler cleanup, regex passes). |
| `Cleanup`       | rejected | Too narrow — cleanup is one of several post-processing tasks (summarization, translation, punctuation restoration may land later). |
| `Language`      | rejected | Ambiguous vs transcription language selection. |
| `Llm`           | rejected | Leaks implementation detail. The engine may evolve (different runtimes, quantized vs fp16, non-LLM text models) without the category needing to change. |

### User-facing label: "Post-processing"

`PostProcessingSettings.tsx` already anchors the term in the product
vocabulary. Keeping parity reduces relearning cost for existing users.
The badge on each card reads "Post-processing" (hyphenated to match
title-case style of "Transcription"). Raw `PostProcessor` is never
shown to users; a `fn category_label(ModelCategory) -> &'static str`
helper on the backend side, mirrored by an i18n key on the frontend,
is the single source of truth for the label string.

### Filter control: segmented, not tabs, not grouped list alone

- **Segmented control** (`All | Transcription | Post-processing`) placed
  above a single grouped list with per-category section headings.
- Rejected tabs: hide the other category entirely, increasing click
  cost when a user wants to see "what do I have downloaded?" as a
  whole.
- Rejected filter chips (multi-select): mutual exclusivity maps to
  single-select segmented control better; users rarely want "none".
- `All` default preserves the current Models-page behavior for users
  who never touch post-processing.

## Architecture decisions

Per R-ID, the chosen approach and the existing pattern it follows.

- **R-001 `PostProcessor` variant** — Mechanical enum extension in
  `src-tauri/src/managers/model/mod.rs:29-34`. Follows the existing
  pattern used when `System` was added.
- **R-002 Catalog migration** — Mirror the pattern already in
  `managers/model/catalog.rs` (const array of entries constructed at
  module load time). If the file threatens 800 lines, split to
  `managers/model/catalog/{mod,transcription,post_processor}.rs` with
  `mod.rs` owning the aggregator `fn all() -> Vec<ModelInfo>`. Follows
  the split pattern already used in `audio_toolkit/`.
- **R-003 Metadata blocks** — Optional nested structs with
  `#[serde(default)]` + `Option<T>`. Follows the same pattern
  `ModelInfo::category` uses today (`#[serde(default)]`), which
  guarantees legacy JSON deserializes cleanly. Backend authority;
  frontend consumes via generated `bindings.ts`.
- **R-004 Single download pipeline** — Keep
  `managers/model/download.rs` as-is; extend it to handle
  post-processor entries by reading `category` off `ModelInfo` and
  choosing the on-disk target dir (`<app-data>/models/` for
  Transcription, `<app-data>/llm/` for PostProcessor — path remains
  `llm/` to avoid a file migration on disk). Delete
  `managers/llm/download.rs`.
- **R-005 Unified event** — Single `ModelDownloadProgress` struct in
  `managers/model/mod.rs` with `id`, `category`, `downloaded_bytes`,
  `total_bytes`, `status`. The `category` field is the discriminator
  the frontend uses to route into the right progress bar.
- **R-006 Command consolidation** — New commands under
  `commands/model.rs` take `category` as a parameter. Old `*_llm_*`
  commands become thin wrappers that call the new command with
  `category = PostProcessor` and `warn!` once per call. Remove shims
  in the next release.
- **R-007 Unified Models page** — `ModelsSettings.tsx` gains a
  segmented control + section headers + per-card badge. All strings
  via `t()` keyed i18n lookups; no raw enum tokens in the DOM.
- **R-008 Post Processing embed** — Delete
  `local-models/LlmModelCatalog.tsx` and replace with a thin component
  `<LocalModelPicker category="PostProcessor" />` that internally uses
  the unified list filtered and lockable. Alternatively keep the file
  as a 20-40 line delegation shim. Either satisfies AC-008-a.
- **R-009 Settings migration** — In `settings.rs` load path, after
  deserializing `AppSettings`, run an `fn migrate_post_process_id()`
  that checks whether the stored id exists in the unified catalog and
  — if not — maps legacy ids via a hard-coded lookup. 1:1 today since
  ids are preserved, but the hook exists for future renames. Idempotent
  by construction (already-valid ids are passed through unchanged).
- **R-010 i18n** — `scripts/check-translations.ts` gates CI. English
  authoritative in `en/translation.json`; 19 other locales carry the
  English string as placeholder per `i18n-pruning`. New keys:
  `settings.models.filter.all`,
  `settings.models.filter.transcription`,
  `settings.models.filter.postProcessing`,
  `settings.models.badge.transcription`,
  `settings.models.badge.postProcessing`,
  `settings.postProcessing.manageModelsLink`.
- **R-011 Dep hygiene** — Run `cargo machete` + `knip` + `depcheck` on
  the diff. Deleting `managers/llm/catalog.rs` +
  `managers/llm/download.rs` likely removes one or more crates that
  are then unused; `dep-hygiene` skill reports and removes them.
- **R-012 File-size cap** — Pre-compute expected line counts before
  migration. Split `catalog.rs` proactively if the projected size
  exceeds 650 lines (100-line safety margin).
- **R-013 Live-app verification** — `coverage.json` contains a `manual`
  verifier with numbered click-through steps.

## Component & module touch-list

Backend (`src-tauri/src/`):

- `managers/model/mod.rs` — enum variant, new metadata structs, updated
  `ModelInfo`, unified event struct.
- `managers/model/catalog.rs` — migrated entries; possibly split into
  submodules under `managers/model/catalog/`.
- `managers/model/download.rs` — per-category target-dir selection.
- `managers/model/hash.rs` — no change (already category-agnostic).
- `managers/llm/mod.rs` — rewired to read from unified catalog.
- `managers/llm/inference.rs` — rewired to accept `ModelInfo` instead
  of `LlmModelInfo`.
- `managers/llm/catalog.rs` — **deleted**.
- `managers/llm/download.rs` — **deleted**.
- `managers/llm/tests.rs` — updated to the unified types.
- `commands/model.rs` (or equivalent) — new category-aware commands.
- `commands/llm.rs` or equivalent — reduced to shim forwarders.
- `settings.rs` — migration helper.

Frontend (`src/`):

- `components/settings/models/ModelsSettings.tsx` — segmented control,
  section headers, per-card badge, filter prop support.
- `components/settings/post-processing/PostProcessingSettings.tsx` —
  embed the unified picker.
- `components/settings/post-processing/local-models/LlmModelCatalog.tsx`
  — deleted or reduced to delegation shim.
- `bindings.ts` — regenerated from specta.
- `i18n/locales/*/translation.json` — 20 files updated.

## Single-source-of-truth placement

- **Backend authority**: `managers/model/catalog.rs` is the sole
  declaration of every downloadable local model asset (transcription
  or post-processor). The aggregator function returns a
  `Vec<ModelInfo>`; everything else reads it. `bindings.ts` is
  regenerated from the Rust types.
- **Frontend consumer**: `ModelsSettings.tsx` is the sole component
  that renders the catalog. `PostProcessingSettings.tsx` embeds it
  with a locked filter instead of maintaining its own list.
- **Label authority**: `category_label()` helper (backend) + matching
  i18n keys (frontend). No component invents its own copy.

## Data flow

```
managers/model/catalog.rs (const Vec<ModelInfo>)
        |
        v
managers/model/mod.rs (Manager state: catalog + per-id status)
        |                         |
        v                         v
commands/model.rs             download.rs -> hash.rs
        |                         |
        v                         v
bindings.ts <--- specta        ModelDownloadProgress events
        |                         |
        v                         v
ModelsSettings.tsx <-- listens ---+
        |
        v
PostProcessingSettings.tsx (embeds ModelsSettings with filter locked)
```

## Migration / compatibility

- **Order of operations** (execution plan):
  1. Extend `ModelCategory` with `PostProcessor` (R-001).
  2. Introduce `TranscriptionMetadata` + `LlmMetadata`; leave legacy
     flat fields in place (R-003).
  3. Migrate GGUF entries into unified catalog (R-002).
  4. Switch `managers/llm/inference.rs` to read from unified catalog.
  5. Unify download pipeline + event shape (R-004, R-005).
  6. Introduce new Tauri commands with category arg; reduce old ones
     to shims (R-006).
  7. Update `ModelsSettings.tsx` with filter + badges (R-007).
  8. Embed unified picker in `PostProcessingSettings.tsx` (R-008).
  9. Add settings migration (R-009).
  10. i18n sweep (R-010).
  11. Delete `managers/llm/catalog.rs` + `managers/llm/download.rs`.
  12. Dep hygiene sweep (R-011).
  13. File-size check + split `catalog.rs` if needed (R-012).
  14. Live-app verification (R-013).

- **Backwards compatibility**:
  - Legacy `ModelInfo` JSON deserializes cleanly (`#[serde(default)]`).
  - Legacy `post_process_local_model_id` values migrated through the
    lookup table.
  - Old Tauri command names remain callable for one release.
  - On-disk model files under `<app-data>/llm/` stay at that path;
    only the Rust code that reads them changes.

## Risk register

| Risk | Mitigation | AC catching regression |
|------|------------|------------------------|
| `managers/model/catalog.rs` crosses 800-line file-size cap after migration. | Pre-compute line count; proactively split into `catalog/{mod,transcription,post_processor}.rs` if projected > 650 lines. | AC-012-a |
| Persisted `post_process_local_model_id` fails to resolve after upgrade, silently resetting the user's pick. | Settings migration with 1:1 id mapping table; fixture-based cargo test. | AC-009-a |
| Tauri command renames break external scripts / in-flight dev builds. | One-release deprecation shim forwarding to unified commands; deprecation warn log. | AC-006-a |
| Unified `ModelDownloadProgress` event shape breaks frontend progress bars. | Keep `id` at top level; add `category` as new field; hashmap keyed by `id` survives. Live-app AC covers download UX. | AC-005-b |
| Deleting `managers/llm/catalog.rs` leaves a dangling crate dependency (e.g. a GGUF-parsing helper only used there). | Run `dep-hygiene` skill after deletion; remove newly-orphaned crates. | AC-011-a |
| i18n drift: English key added but locale files forgotten, CI fails downstream. | Pre-commit run of `scripts/check-translations.ts`; 19 locales get English placeholder per `i18n-pruning`. | AC-010-a |
| Serde default on new metadata blocks missed; legacy JSON fails to deserialize. | `#[serde(default)]` on both `transcription_metadata` and `llm_metadata`; cargo test with legacy fixture. | AC-003-b |
| Manual/visual regression on Models page (wrong colors, raw enum leakage). | `manual` coverage verifier with numbered steps; Settings UI contract from AGENTS.md in review checklist. | AC-007-c, AC-013-a |
| Accidentally introducing a runtime network dependency (e.g. pulling in a crate that auto-fetches weights). | `dep-hygiene` + reviewer check; local-only inference is a hard rule. | AC-011-a |
| Catalog migration loses a GGUF entry (off-by-one). | Cargo test comparing entry counts pre/post using a snapshot of the old count. | AC-002-a |
