# Feature request: caption-parity-eval

## 1. Problem & Goals

Toaster renders captions via two paths:

- Live preview in the editor UI — an HTML/CSS/JS overlay
  (`src/components/player/CaptionOverlay.tsx`,
  `src/hooks/useCaptionLayout.ts`) that scales a `CaptionLayout`
  returned by the backend command `get_caption_layout` into DOM
  geometry over the `<video>` element.
- Burned-in export — the ASS emitter
  (`src-tauri/src/managers/captions/ass.rs`) consumed by FFmpeg's
  `subtitles` filter.

Both paths are fed by the single authoritative layout engine at
`src-tauri/src/managers/captions/layout.rs` (see module docstring:
"One authoritative layout engine that both the preview and the
export render from"). There is, however, no standing eval that
proves the two rendered outputs stay geometry-/time-equivalent as
the code evolves. Caption drift between preview and export is a
top user-visible regression class (see `ass.rs:6-22` for the
~35 % pill-vs-text mismatch history).

Goal: ship a standing harness under `scripts/eval/` that exercises
a fixed caption fixture set through both paths and asserts
geometry + timing parity within a fixed tolerance, emitting the
same pass/fail JSON shape as the other evals so
`eval-harness-runner` can wrap it.

## 2. Desired Outcome & Acceptance Criteria

"Done" looks like:

- `scripts/eval/eval-caption-parity.ps1` exists, runs headless,
  emits the standard pass/fail JSON shape.
- Three caption fixtures (single-line, multi-line wrap,
  rapid-fire sequence) live under `eval/caption-parity/` with
  expected-geometry snapshots.
- Assertions: time boundaries within 1 sample (@ 48 kHz);
  geometry within 1 px (line starts/ends, wrap points, pill
  position). Font-metric drift allowed only if identical on both
  paths.
- A deliberate regression to shared layout code causes the eval
  to fail with a readable diff.
- `eval-harness-runner` can invoke the script with a single
  command.
- CI integration plan documented (wiring to GitHub Actions may be
  a follow-up).

See `PRD.md` for the machine-checkable AC list.

## 3. Scope Boundaries

### In scope

- Headless harness script.
- Three fixtures + expected-geometry snapshots in
  `eval/caption-parity/`.
- Pass/fail JSON shape
  `{ pass, fixtures[{id,pass,diffs[]}], summary{total,passed,failed} }`.
- Negative-test mechanism (forced regression fixture) that proves
  the gates fire.
- A brief CI integration plan in `BLUEPRINT.md`.

### Out of scope (explicit)

- Actually wiring the new script into `.github/workflows/*.yml`
  (deferred — plan only).
- Expanding beyond the three starter fixtures.
- UI changes. No new i18n strings.
- Any hosted-inference dependency (forbidden per `AGENTS.md:37`).
- Touching unrelated Handy-era surface (see `handy-legacy-pruning`).

## 4. References to Existing Code

- `src-tauri/src/managers/captions/layout.rs:1-40` — authoritative
  layout engine; produces `CaptionBlock` with video-pixel
  geometry + timeline-domain timestamps. Single source of truth.
- `src-tauri/src/managers/captions/ass.rs:1-30` — ASS emitter;
  relies on libass BorderStyle=3 so the box auto-sizes to the
  same glyphs libass renders.
- `src-tauri/src/managers/captions/mod.rs` — re-exports
  `blocks_to_ass`, `build_blocks`, `compute_caption_layout`,
  `CaptionBlock`, `CaptionLayout`, `TimelineDomain`.
- `src/hooks/useCaptionLayout.ts:1-40` — preview side consumes
  `commands.getCaptionLayout`.
- `src/components/player/CaptionOverlay.tsx:1-40` — renders
  `CaptionBlock[]` as DOM pills.
- `scripts/eval/eval-audio-boundary.ps1:380-405` — existing
  eval's report emission convention
  (`eval/output/<eval>/<fixture>/<ts>/report.{json,md}`).
- `scripts/eval/eval-edit-quality.ps1` — simpler eval that shows
  the ffprobe/ffmpeg invocation convention.
- `.github/skills/audio-boundary-eval/SKILL.md` — gates-as-table
  format, fixture generator pattern, runner invocation style.
- `.github/agents/eval-harness-runner.agent.md` — wrapper agent;
  must be able to `pwsh -NoProfile -File` the new script.
- `features/product-map-v1/PRD.md:453` — Milestone 1 row
  `caption-parity-eval`; confirms the "1 px / 1 frame"
  commitment.

## 5. Edge Cases & Constraints

- Headless: no mounted `<video>`, no Tauri window. The preview
  geometry must be derivable from the backend's `CaptionLayout`
  + `CaptionBlock` outputs without a live DOM. The harness
  replays `commands::get_caption_layout` + `build_blocks` via a
  Rust test binary (cargo test) or a dedicated CLI shim, then
  compares against the ASS emission.
- ASS rendering geometry is best recovered by running libass via
  FFmpeg on a transparent PNG canvas and extracting pill bounds
  by alpha threshold — or by parsing `ass.rs` output directly for
  deterministic fields (x, y, width, height, start, end).
- Timing comparison must happen in a single `TimelineDomain`.
  Preview uses `Source`; export uses `Edited`. The harness picks
  one domain (declared per-fixture) and compares.
- Fixture transcripts must be deterministic (no whisper
  round-trip). Commit pre-computed `Word[]` + keep-segments.
- Font metric drift: `fontdue` vs libass glyph advance is the
  historical divergence (`ass.rs:10-22`). The eval MUST fail
  loudly if the pill width vs text width delta regresses.

## 6. Data Model (optional)

Fixture directory layout (planned, not yet on disk):

```
eval/caption-parity/
  fixtures/
    <fixture-id>/
      input.json      # Word[] + keep-segments + VideoDims +
                      # CaptionProfile + timeline domain
      expected.json   # snapshot of CaptionBlock[] geometry
                      # (video-pixel coords, per-line wrap points,
                      # start/end timestamps)
    single_line_01/
    multi_line_wrap_01/
    rapid_fire_01/
  README.md
```

Pass/fail JSON shape (emitted to
`eval/output/caption-parity/<ts>/report.json`):

```
{
  "pass": bool,
  "fixtures": [
    { "id": "single_line_01", "pass": bool,
      "diffs": [{"field":"line_0.end_x_px","expected":123,"actual":124,"tolerance":1}] }
  ],
  "summary": { "total": 3, "passed": 3, "failed": 0 }
}
```

## Q&A

Answers supplied inline by the controller and treated as
already-settled:

- Q: Parity tolerance?
  A: Time boundaries strict to 1 sample (@ 48 kHz). Geometry 1 px
  on line starts/ends, wrap points, pill position. Font-metric
  drift allowed only if identical on both paths.
- Q: Fixture set?
  A: Three fixtures — short single-line, long multi-line wrap,
  rapid back-to-back sequence. More is follow-up work.
- Q: Harness runner?
  A: Extend `scripts/eval/` convention. Add
  `scripts/eval/eval-caption-parity.ps1`. Emit the standard
  pass/fail JSON so `eval-harness-runner` can wrap it.
- Q: Headless?
  A: Yes. No live Tauri window. FFmpeg shell-out is allowed for
  the ASS-burn path; no `<video>` element required.
