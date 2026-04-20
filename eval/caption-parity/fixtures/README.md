# Caption-parity eval fixtures

Fixtures consumed by `scripts/eval/eval-caption-parity.ps1` and by the
Rust dump binary `src-tauri/tests/caption_parity_dump.rs`. See
`features/caption-parity-eval/BLUEPRINT.md` for the ratified design.

## Layout

```
eval/caption-parity/fixtures/
  single_line_01/
    input.json       — fixture schema (words + profile + video_dims)
    expected.json    — snapshot of the dump output at commit time
  multi_line_wrap_01/
    input.json
    expected.json
  rapid_fire_01/
    input.json
    expected.json
```

## Fixture schema (`input.json`)

| Field                  | Type                                              | Required | Notes                                                                                     |
| ---------------------- | ------------------------------------------------- | -------- | ----------------------------------------------------------------------------------------- |
| `id`                   | string                                            | yes      | Echoed into dump output for diagnostics.                                                  |
| `timeline_domain`      | `"Source"` \| `"Edited"`                          | yes      | Passed to `build_blocks`.                                                                 |
| `video_dims`           | `{ width, height }`                               | yes      | Authoritative geometry space.                                                             |
| `caption_profile`      | `CaptionProfile`                                  | yes      | Full settings profile.                                                                    |
| `words`                | `Word[]`                                          | yes      | Post-edit word list.                                                                      |
| `keep_segments`        | `[i64, i64][]`                                    | no       | Source-clock keep ranges; only used when `timeline_domain == "Edited"`. Defaults to `[]`. |
| `preview_scale_factor` | number                                            | no       | `rendered_height / frame_height` the harness should assume. Defaults to `1.0`.            |
| `config_overrides`     | `{ max_segment_duration_us?, include_silenced? }` | no       | Pins layout knobs not covered by `CaptionProfile`.                                        |

`Word` shape matches `src-tauri/src/managers/editor/types.rs::Word`.

`CaptionProfile` shape matches `src-tauri/src/settings/types.rs::CaptionProfile`.

## Snapshot (`expected.json`)

Produced by the dump binary and captured via `-Regenerate` on the
harness. Shape:

```json
{
  "id": "single_line_01",
  "preview_scale_factor": 0.5,
  "layout": { "margin_v_px": ..., ... },
  "blocks": [ { "index": 1, "start_us": ..., "lines": [...], ... } ],
  "ass": "[Script Info]\n..."
}
```

The `ass` string is the exact payload `blocks_to_ass` emits so the
harness can parse `Dialogue:` + `Style:` lines for export-side geometry
comparison.

## Regenerating

```powershell
pwsh -File scripts/eval/eval-caption-parity.ps1 -Regenerate
```

Review the `git diff` against `expected.json` carefully before staging —
a legitimate layout fix will produce a small, intentional delta, and
unintended drift will produce a surprising one.

## Designed fixtures

| Fixture              | What it pins                                                                                                                             |
| -------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| `single_line_01`     | 4 short words, 1920x1080, default 90% width. Single block, single line — proves basic geometry + time parity with no wrapping decisions. |
| `multi_line_wrap_01` | 14 words at 40% max-width, font_size=48. Forces 2–3 visual lines inside one block — proves pixel-wrap-point parity.                      |
| `rapid_fire_01`      | 8 one-word captions forced into separate blocks by a 600 ms `max_segment_duration_us` override. Proves block-boundary time parity.       |

Add new fixtures only when there is a concrete parity regression to
regression-test. Re-snapshot with `-Regenerate` and stage both
`input.json` and `expected.json` in the same commit.
