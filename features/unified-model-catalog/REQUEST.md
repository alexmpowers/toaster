# Feature request: unified model catalog

## 1. Problem & Goals

Toaster ships two parallel "downloadable local model" subsystems. The
original transcription catalog lives under `managers/model/` and drives
the Models settings page. The `local-llm-model-catalog` feature added a
second, independent catalog under `managers/llm/` for the GGUF
post-processor, plus a second UI under
`src/components/settings/post-processing/local-models/`. There are two
catalogs, two `*Info` record types, two download pipelines, two sets of
Tauri events, and two disjoint UIs for what is fundamentally the same
concept: "a curated local model asset, downloadable on demand, verified
by sha256, selectable as the active backend for some pipeline stage".

Verbatim user request:

> Did we register the post processing models to our Models menu and
> framework? Ideally there should be a metadata field to determine if
> it's for the transcripts or the post processor, the PM can come up
> with the proper naming and labels during the project execution.

This directly violates the AGENTS.md "Single source of truth for
dual-path logic" rule. The business risk is not theoretical: every new
piece of model metadata (size, sha256, progress events, hash
verification, custom-model support) must now be implemented, tested,
and i18n-keyed twice. The dead-code / drift surface grows monotonically.

Goals:

1. One registry, one `ModelInfo` type, one download pipeline, one UI
   surface.
2. A `category` discriminator on `ModelInfo` so the same record answers
   "is this for transcription or post-processing?".
3. Zero loss of user configuration across migration
   (`AppSettings.post_process_local_model_id` must keep resolving).
4. No new network paths — local-only inference remains hard.

## 2. Desired Outcome & Acceptance Criteria

"Done" looks like:

- `ModelCategory` has a `PostProcessor` variant (naming decision, see
  BLUEPRINT) and every GGUF entry previously declared in
  `managers/llm/catalog.rs` now lives in `managers/model/catalog.rs`
  under that variant.
- `managers/llm/catalog.rs` and `managers/llm/download.rs` are deleted.
  The runtime stays — `managers/llm/inference.rs` and `mod.rs` continue
  to own the in-process GGUF inference path.
- The Models settings page shows both categories with a category filter
  (segmented control) and a per-card badge, using the labels
  "Transcription" and "Post-processing".
- The Post Processing settings page's local-models picker embeds the
  unified Models component with the filter locked to Post-processing,
  or deep-links to Models with that filter pre-applied.
- Existing Tauri commands that named `llm` (`download_llm_model`,
  `get_llm_models`, `delete_llm_model`, ...) either keep working via a
  one-release deprecation shim or are replaced with category-aware
  calls that take `category` as a parameter.
- `AppSettings.post_process_local_model_id` continues to resolve to a
  valid model after upgrade; a migration step remaps any legacy ids.
- `ModelDownloadProgress` events carry a `category` field; the frontend
  routes progress by category without needing two event channels.
- `cargo test`, coverage gate, and tasks gate all green. File-size cap
  respected (no new .rs/.tsx over 800 lines).
- `npm run lint`, `bun run check:file-sizes`, and
  `scripts/check-translations.ts` pass.
- Manual live-app verification per AGENTS.md "Verified means the live
  app" (numbered click-through in `coverage.json`).

## 3. Scope Boundaries

### In scope

- `managers/model/mod.rs`, `managers/model/catalog.rs`,
  `managers/model/download.rs`, `managers/model/hash.rs`.
- `managers/llm/` — delete `catalog.rs` and `download.rs`; keep
  `inference.rs`, `mod.rs`, `tests.rs`; rewire against unified catalog.
- Tauri commands under `src-tauri/src/commands/` that touch either
  catalog.
- `src/bindings.ts` regeneration.
- `src/components/settings/models/ModelsSettings.tsx`.
- `src/components/settings/post-processing/PostProcessingSettings.tsx`
  and `local-models/LlmModelCatalog.tsx` (either deleted or reduced to
  a filter-pinned wrapper).
- `src/i18n/locales/*/translation.json` — 20 locales; English is
  authoritative per `i18n-pruning`.
- Settings schema migration under `src-tauri/src/settings.rs`.

### Out of scope (explicit)

- The in-process GGUF inference engine itself
  (`managers/llm/inference.rs`).
- Cloud / hosted model providers. Not adding them; not removing the
  `LOCAL_GGUF_PROVIDER_ID` default.
- Transcription adapter contract changes.
- New model downloads — only the catalog entries that already exist
  today are migrated.
- Export or audio path. Nothing about seams, boundaries, or FFmpeg.

## 4. References to Existing Code

- `src-tauri/src/managers/model/mod.rs:29-34` — current `ModelCategory`
  enum (two variants). Extending here.
- `src-tauri/src/managers/model/mod.rs:36-59` — `ModelInfo` struct.
  Adding optional nested `transcription_metadata` and `llm_metadata`
  blocks; existing top-level fields stay for backward compatibility for
  one release.
- `src-tauri/src/managers/model/catalog.rs` — 16 transcription entries;
  target home for the migrated post-processor entries.
- `src-tauri/src/managers/model/download.rs` — authoritative download
  pipeline (keeps the name).
- `src-tauri/src/managers/model/hash.rs` — sha256 verification; used
  by both categories after migration.
- `src-tauri/src/managers/llm/catalog.rs:12-60` — `LlmCatalogEntry` +
  `LlmModelInfo`. Source records to migrate; file deleted at end.
- `src-tauri/src/managers/llm/download.rs` — second download pipeline;
  deleted.
- `src-tauri/src/managers/llm/inference.rs` — retained; rewired to
  read from the unified catalog.
- `src/components/settings/models/ModelsSettings.tsx` — target for
  category filter + per-card badge.
- `src/components/settings/post-processing/PostProcessingSettings.tsx`
  — post-process selector; rewires to unified bindings.
- `src/components/settings/post-processing/local-models/LlmModelCatalog.tsx`
  — deleted or reduced to a filter-locked embed.
- `src-tauri/src/settings.rs` — `post_process_local_model_id`
  resolution + migration.
- `AGENTS.md` — "Single source of truth for dual-path logic",
  "Local-only inference", file-size cap, "Verified means the live
  app".

## 5. Edge Cases & Constraints

- **File-size cap (800 lines).** `managers/model/catalog.rs` already
  hosts 16 entries; adding post-processor entries plus an `LlmMetadata`
  nested struct may push it near the cap. Blueprint risk register
  acknowledges this and plans a split
  (`catalog/transcription.rs` + `catalog/post_processor.rs` aggregated
  in `catalog/mod.rs`) if the cap is threatened.
- **Persisted settings.** A user upgrading across this change must not
  lose their post-process model selection. Migration must map any
  legacy `post_process_local_model_id` value to the new unified id
  (expected 1:1 — ids we keep are stable).
- **Tauri command surface.** Renaming `download_llm_model` ->
  `download_model` with a `category` arg breaks external scripts and
  in-flight dev builds. Decision: deprecation shim for one release that
  forwards to the unified command.
- **Event shape.** `ModelDownloadProgress` currently exists in both
  worlds with slightly different payloads. Unified event MUST include
  `category` and MUST keep `id` at the top level so frontend hashmaps
  keyed by id keep working.
- **i18n.** Every user-visible string change requires 20 translation
  files updated. English is authoritative; `i18n-pruning` allows other
  locales to mirror the English value as a placeholder until a
  translation pass lands.
- **No hosted inference.** Confirm no dependency introduced by the
  refactor opens a network fetch path at runtime. The only network
  path remains the user-initiated curated-download fetch.

## 6. Data Model

```rust
pub enum ModelCategory {
    Transcription,
    PostProcessor, // new
    System,
}

pub struct ModelInfo {
    // ... existing flat fields ...
    pub category: ModelCategory,
    pub transcription_metadata: Option<TranscriptionMetadata>,
    pub llm_metadata: Option<LlmMetadata>,
}

pub struct TranscriptionMetadata {
    pub engine_type: EngineType,
    pub accuracy_score: f32,
    pub speed_score: f32,
    pub supports_translation: bool,
    pub supports_language_selection: bool,
    pub supported_languages: Vec<String>,
}

pub struct LlmMetadata {
    pub quantization: String,
    pub context_length: u32,
    pub recommended_ram_gb: u32,
    pub prompt_template_id: Option<String>,
}
```

Transcription-only fields currently at the top level of `ModelInfo`
remain there for one release (serde default values keep old saved
state readable) and are also mirrored into `transcription_metadata` so
the new code path is the single-read authority. A later cleanup task
removes the duplicated top-level fields once no caller reads them.

## Q&A

This run was dispatched with pre-confirmed scope from the controller;
the six canonical clarifications are answered inline.

- **Q1. Which `ModelCategory` variant name?** — `PostProcessor`. See
  BLUEPRINT "Naming decisions" for the rejected alternatives.
- **Q2. UI label for the category?** — "Post-processing" (matches the
  existing settings page title `PostProcessingSettings`).
- **Q3. UI pattern: tabs vs filter chips vs grouped list?** —
  Segmented control (`All | Transcription | Post-processing`) above a
  single grouped list with section headings.
- **Q4. Per-card badge copy?** — "Transcription" or "Post-processing"
  pill, reusing existing color tokens (accent orange reserved for the
  Post-processing pill hover state per the design system).
- **Q5. Keep Post Processing page's model picker or fold into
  Models?** — Keep the Post Processing page for provider selection and
  prompts; replace its local-models sub-component with a filter-locked
  embed of the unified Models component. A "Manage models" link
  deep-links to Models with the Post-processing filter pre-applied.
- **Q6. Deprecation shim for `download_llm_model` et al.?** — Yes,
  one release. Shim forwards to the unified command with
  `category = PostProcessor`. Logged as deprecated; remove next
  release.
