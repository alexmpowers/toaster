# PRD: caption-parity-eval

## Problem & Goals

Toaster renders captions through two paths — the live preview DOM
overlay (`src/components/player/CaptionOverlay.tsx` +
`src/hooks/useCaptionLayout.ts`) and the burned-in ASS export
(`src-tauri/src/managers/captions/ass.rs` → FFmpeg `subtitles`
filter). Both paths today consume a single authoritative
`CaptionLayout` / `CaptionBlock` stream from
`src-tauri/src/managers/captions/layout.rs` (see its module
docstring and `ass.rs:6-22`). There is no standing eval that
proves this contract holds as the code evolves.

Goal: deliver a headless, CI-ready harness that exercises three
caption-heavy fixtures through both paths and asserts
geometry/timing parity within fixed tolerances, emitting the
standard pass/fail JSON shape used by the other evals so that
`eval-harness-runner` (`.github/agents/eval-harness-runner.agent.md`)
can wrap it.

This closes Milestone 1 row 1.5 of
`features/product-map-v1/PRD.md:453` ("1 px / 1 frame").

## Scope

### In scope

- New PowerShell harness `scripts/eval/eval-caption-parity.ps1`
  (headless; emits standard pass/fail JSON).
- Three fixtures under `eval/caption-parity/fixtures/`:
  `single_line_01`, `multi_line_wrap_01`, `rapid_fire_01`.
- Expected-geometry snapshots committed alongside inputs.
- Tolerance policy: time 1 sample (@ 48 kHz), geometry 1 px.
- Negative-test mechanism (deliberate regression fixture or
  `-ForceDrift` flag) that proves the gates fire.
- Documented CI integration plan in `BLUEPRINT.md`.
- Minimal test shim in `src-tauri/tests/` (or a `--dump-layout`
  flag on an existing dev binary) to deterministically export
  `CaptionBlock[]` to JSON for the harness to diff.

### Out of scope (explicit)

- Actual wiring of the new script into
  `.github/workflows/*.yml`. Planned only; enforced only by the
  CI-integration-plan AC.
- Expanding beyond the three starter fixtures. Follow-up slug.
- Any UI changes or new i18n strings.
- Any hosted-inference or network dependency
  (`AGENTS.md:37`).
- Touching Handy-era dead code
  (`handy-legacy-pruning` skill).

## Requirements

### R-001 — Parity harness script

- Description: ship a headless PowerShell harness that runs every
  fixture through both the preview `CaptionLayout` path and the
  ASS-burn path, diffs the resulting geometry and timing against
  the snapshot, and emits the standard pass/fail JSON.
- Rationale: a standing gate against the top user-visible
  caption regression class; mirrors the runner style of
  `scripts/eval/eval-audio-boundary.ps1:380-405`.
- Acceptance Criteria
  - AC-001-a — `scripts/eval/eval-caption-parity.ps1` exists,
    runs without a mounted `<video>` / Tauri window, and writes
    `eval/output/caption-parity/<timestamp>/report.json` matching
    the shape `{ pass: bool, fixtures: [{id, pass, diffs: []}],
    summary: {total, passed, failed} }`. Exit code is 0 on pass
    and non-zero on any fixture failure.
  - AC-001-b — Three fixtures exist under
    `eval/caption-parity/fixtures/` — `single_line_01`,
    `multi_line_wrap_01`, `rapid_fire_01` — each with
    `input.json` (Word[] + keep-segments + VideoDims +
    CaptionProfile + timeline domain) and `expected.json`
    (snapshot of `CaptionBlock[]` geometry + timing).
  - AC-001-c — Harness fails any fixture whose time boundary
    delta exceeds 1 sample at 48 kHz, or whose geometry delta on
    line starts/ends, wrap points, or pill position exceeds 1 px
    against the snapshot. Tolerances are read from a single
    declared config block in the script, not per-call.

### R-002 — Harness integration and negative-test proof

- Description: the new script must be a drop-in for
  `eval-harness-runner`, and a deliberate regression to shared
  layout code must produce a readable diff in the report.
- Rationale: an eval that cannot be regressed proves nothing; an
  eval that another agent cannot call is invisible to CI.
- Acceptance Criteria
  - AC-002-a — `.github/agents/eval-harness-runner.agent.md`
    invokes `scripts/eval/eval-caption-parity.ps1` with the same
    shape (single `pwsh -NoProfile -File …`) it already uses for
    the other evals, and the harness's JSON output satisfies the
    runner's expected-schema check without per-script special
    casing.
  - AC-002-b — With a deliberate regression applied to the
    shared layout code (e.g. a 2 px padding bump in
    `managers/captions/layout.rs` or a `-ForceDrift` harness
    flag that injects the drift synthetically), the harness
    exits non-zero and the report `diffs` array cites the
    regressed field by name (`line_0.end_x_px`, `block_0.end_us`,
    etc.) with `expected`, `actual`, and `tolerance`.

### R-003 — CI integration plan

- Description: document how this harness will be wired into CI
  alongside the existing eval gates.
- Rationale: shipping the harness without a plan for CI means it
  never becomes a gate. Actual workflow wiring is a follow-up;
  the plan MUST land now.
- Acceptance Criteria
  - AC-003-a — `features/caption-parity-eval/BLUEPRINT.md`
    contains a "CI integration plan" section naming the target
    workflow file, the job/step placement, the trigger events,
    any required runner setup (FFmpeg on PATH), and the
    success/failure expectation. The section is referenced from
    `coverage.json` as a `doc-section` verifier so the coverage
    gate enforces its presence.

## Edge cases & constraints

- Headless requirement: the preview-side geometry must be
  obtainable from `CaptionLayout` + `CaptionBlock` outputs
  alone. If the harness needs to reach into the Rust backend, it
  does so via `cargo test`-driven JSON dumps under
  `src-tauri/tests/`, not via a live Tauri window.
- Timeline domain: each fixture declares either `Source` or
  `Edited`; both paths must be compared within the same domain.
- Font metrics: allowed to drift only if both paths drift
  identically. The eval must still catch the historical
  fontdue-vs-libass divergence (`ass.rs:10-22`).
- Determinism: fixtures must not depend on whisper-rs output,
  the system clock, locale, or installed fonts beyond the bundled
  Toaster font set (`src-tauri/src/managers/captions/fonts.rs`).

## Data model (if applicable)

See `REQUEST.md` §6 for fixture directory layout and pass/fail
JSON shape. Both are authoritative here.

## Non-functional requirements

- Script runtime on CI hardware: target under 60 s wall clock for
  all three fixtures combined; hard fail at 180 s.
- No network access at any step (per `AGENTS.md:37`).
- Output files confined to `eval/output/caption-parity/`.
- i18n: no new keys; `bun scripts/check-translations.ts` must
  remain green as a side-effect.
- File-size cap: harness script stays under 800 lines; if it
  grows, split helpers into `scripts/lib/CaptionParity.psm1`
  (pattern from `scripts/lib/AudioBoundary.psm1`).
