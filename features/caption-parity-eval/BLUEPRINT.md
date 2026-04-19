# Blueprint: caption-parity-eval

## Architecture decisions

- **R-001 parity harness** follows the runner style of
  `scripts/eval/eval-audio-boundary.ps1:380-405` — one PowerShell
  entry point, a helper module under `scripts/lib/` if the line
  count grows, and a report written to
  `eval/output/<eval>/<timestamp>/report.{json,md}`. Pattern
  cited: `eval-audio-boundary.ps1` + `scripts/lib/AudioBoundary.psm1`.
- **R-001 preview geometry extraction** does NOT mount the DOM.
  The preview consumes `commands.getCaptionLayout` /
  `CaptionBlock[]` verbatim (see
  `src/hooks/useCaptionLayout.ts:1-35` and
  `src/components/player/CaptionOverlay.tsx:1-40`), so comparing
  the backend's own `CaptionLayout` + `CaptionBlock[]` is
  equivalent to comparing the rendered overlay modulo a single
  linear scale factor. That scale factor is declared explicitly
  in each fixture's `input.json` and applied inside the harness.
- **R-001 ASS geometry extraction** uses one of two methods
  (blueprint leaves the selection to the implementer; both are
  acceptable):
  1. Direct parse of the ASS document produced by
     `managers::captions::blocks_to_ass` for positional fields,
     plus an FFmpeg render onto a transparent canvas with alpha
     thresholding for the rendered pill bounds; OR
  2. FFmpeg render only, with pill bounds recovered via alpha
     thresholding. Option (1) gives tighter error messages;
     option (2) survives future ASS-emitter refactors. Either
     way, the comparison happens in video pixels within the
     declared `TimelineDomain`.
- **R-002 negative test** ships a `-ForceDrift <field>=<delta>`
  harness flag that synthetically perturbs one side of the diff
  before comparison; this avoids needing a committed "regressed"
  source tree. Pattern mirrors
  `eval-audio-boundary.ps1 -ForceLeaky`.
- **R-003 CI integration plan** is authored as a section in this
  blueprint and referenced from `coverage.json` as a
  `doc-section` verifier so the coverage gate enforces its
  presence.

## Component & module touch-list

Read-only dependencies (MUST NOT be modified by this feature):

- `src-tauri/src/managers/captions/layout.rs` — authoritative
  `build_blocks`, `compute_caption_layout`, `CaptionBlock`,
  `CaptionLayout`, `TimelineDomain`.
- `src-tauri/src/managers/captions/ass.rs` — `blocks_to_ass`.
- `src-tauri/src/managers/captions/fonts.rs` — `FontRegistry`.
- `src-tauri/src/managers/captions/mod.rs` — re-exports.
- `src/hooks/useCaptionLayout.ts`,
  `src/components/player/CaptionOverlay.tsx` — preview contract.

New (planning-only; implementation happens under
`superpowers:executing-plans`):

- `scripts/eval/eval-caption-parity.ps1` — harness.
- `scripts/lib/CaptionParity.psm1` — helpers (optional; created
  only if the harness crosses ~400 lines).
- `eval/caption-parity/fixtures/single_line_01/` — `input.json`
  + `expected.json`.
- `eval/caption-parity/fixtures/multi_line_wrap_01/` — same.
- `eval/caption-parity/fixtures/rapid_fire_01/` — same.
- `eval/caption-parity/README.md` — fixture authoring notes.
- `src-tauri/tests/caption_parity_dump.rs` (or an analogous
  test binary) — emits `CaptionBlock[]` as JSON for the harness
  to diff. Uses the same crate APIs as the preview's
  `get_caption_layout` command path.

Instruction / agent touches:

- `.github/agents/eval-harness-runner.agent.md` — add the new
  harness to its wrapped-eval list. No logic change elsewhere.

## Single-source-of-truth placement

- **Caption layout**: authoritative in
  `src-tauri/src/managers/captions/layout.rs`. Both paths
  (`ass.rs` for export, `CaptionOverlay.tsx` for preview)
  consume its output verbatim. The parity eval's role is to
  *prove* this contract. If the eval ever fails, the remediation
  policy is: fix the shared backend layout code so both
  consumers re-align — NEVER patch `ass.rs` or
  `CaptionOverlay.tsx` with path-specific quirks. This policy
  is load-bearing and is tracked by R-002 / AC-002-b.
- **Font metrics**: authoritative in
  `src-tauri/src/managers/captions/fonts.rs` /
  `FontRegistry`. The eval must catch the historical
  fontdue-vs-libass drift (`ass.rs:6-22`).
- **Time mapping**: `TimelineDomain` is declared per fixture;
  harness never mixes `Source` and `Edited` in a single diff.

## Data flow

```
  eval/caption-parity/fixtures/<id>/input.json
           |
           v
  src-tauri/tests/caption_parity_dump (cargo test) --> preview.json
           |
           v
  managers::captions::blocks_to_ass        --> export.ass
           |
           v (FFmpeg subtitles filter over transparent canvas)
                                           --> export_bounds.json
           |
           v
  scripts/eval/eval-caption-parity.ps1
     compares preview.json vs export_bounds.json vs expected.json
     emits eval/output/caption-parity/<ts>/report.{json,md}
```

## Migration / compatibility

- No migrations. Eval-only feature. Existing evals untouched.
- `eval-harness-runner` gets one additional entry; no breaking
  change to its JSON schema.
- No user-facing changes. `bun scripts/check-translations.ts`
  unaffected.

## CI integration plan

**This section is the verifier for AC-003-a. Referenced from
`coverage.json` as a `doc-section` entry.**

- **Target workflow file**: `.github/workflows/evals.yml` (the
  existing eval-gate workflow, same file that runs
  `eval-audio-boundary.ps1`). If a dedicated caption-parity
  workflow is preferred during review, the fallback is a new
  `.github/workflows/caption-parity.yml` mirroring the
  audio-boundary structure.
- **Job/step placement**: new step *after* the audio-boundary
  eval step, under the same `evals` job, so both gates share the
  same FFmpeg runner setup. Step name:
  `Run caption parity eval`.
- **Trigger events**: `pull_request` (all branches targeting
  `main`) and `push` to `main`. No scheduled runs.
- **Runner setup requirements**:
  - Windows runner (PowerShell 7+, matches the existing eval
    scripts).
  - FFmpeg 7 on PATH (same requirement as audio-boundary eval).
  - Rust toolchain already present for the test binary.
  - No network access required — fixtures and snapshots are
    committed to the repo.
- **Success/failure expectation**: step succeeds iff the harness
  exits 0 (all three fixtures pass within 1 px / 1 sample).
  Non-zero exit fails the step and blocks merge. The
  `report.json` file is uploaded as a workflow artifact for
  post-mortem.
- **Deferred to follow-up**: the actual YAML edit. This plan is
  committed now so that the follow-up PR is mechanical. Tracked
  by AC-003-a via `doc-section` verification of this section.

## Risk register

| Risk | Mitigation | AC catching regression |
|------|------------|------------------------|
| Harness asserts the wrong contract (e.g. diffs preview DOM against its own expected snapshot instead of export geometry) | Two independent comparisons required per fixture: preview vs snapshot AND export vs snapshot. Both must pass. | AC-001-c |
| fontdue/libass drift silently returns | Dedicated assertion on pill-width-vs-text-width ratio, cross-checked against the rendered ASS output. | AC-001-c + AC-002-b |
| ASS parsing brittleness | Prefer alpha-threshold extraction from FFmpeg render as the primary method; ASS-text parsing is a diagnostic aid only. | AC-001-a, AC-002-b |
| Script grows past 800-line cap | Factor into `scripts/lib/CaptionParity.psm1` once it crosses ~400 lines (pattern: `AudioBoundary.psm1`). | AGENTS.md file-size gate (`bun run check:file-sizes`) |
| Harness passes locally but fails on CI due to missing FFmpeg | CI integration plan explicitly requires FFmpeg 7 on PATH; harness exits 2 with a clear message if FFmpeg is missing. | AC-003-a (documented), AC-001-a (behavior) |
| Someone "fixes" a failing parity by patching `ass.rs` or `CaptionOverlay.tsx` directly instead of the shared layout | Single-source-of-truth placement section above makes this policy explicit; any such PR is a `superpowers:code-reviewer` Critical finding per `.github/instructions/code-review.instructions.md`. | AC-002-b (regression reproducible) |
