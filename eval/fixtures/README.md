# `eval/fixtures/` — test fixture media

This directory holds **eval / fixture media** used by the precision and
audio-fidelity harnesses. It is intentionally committed to the repo so the
evals are reproducible across machines and CI.

> **Do not delete or rename files here without updating every dependent
> skill, agent, and script listed below.** They reference these paths
> verbatim.

## Contents

| File                         | Purpose                                                                      |
| ---------------------------- | ---------------------------------------------------------------------------- |
| `toaster_example.mp4`        | Original real-speech source clip (~24 MB, 21 s, conversational).             |
| `toaster_example-edited.mp4` | Hand-edited baseline used as the parity target (~4.4 MB).                    |
| `pagineted-reports.mp4`      | Presentation-style video (~322 MB, longer, wider pauses between phrases).    |
| `*.png`                      | Screenshots used by README / docs.                                           |

## Multi-fixture eval support

The eval harness (`scripts/eval/run-eval-harness.ps1`) accepts optional
`-FixtureOriginal` / `-FixtureEdited` / `-FixtureBaseline` parameters.
When omitted, it defaults to `toaster_example.mp4`. To run against a
different fixture:

```powershell
pwsh scripts/eval/run-eval-harness.ps1 `
    -FixtureOriginal eval/fixtures/pagineted-reports.mp4 `
    -FixtureEdited eval/fixtures/pagineted-reports-edited.mp4
```

Per-video baselines are stored as `tests/fixtures/<video-stem>.baseline.json`.

## Adding a new fixture

1. Place the `.mp4` in this directory.
2. Add a row to the table above.
3. Create a `.toaster` project file (via the app or manually).
4. Generate a baseline: run `eval-edit-quality.ps1` and save the output
   to `tests/fixtures/<video-stem>.baseline.json`.
5. Update the dependents list below if the fixture is referenced by name.

## Dependents

Skills (`.github/skills/`):

- `toaster-eval/SKILL.md`
- `toaster-adapter-contract/SKILL.md`

Former agents (now consolidated into `toaster-eval`):

- Cut-drift fuzzer, eval harness, waveform diff — all in `toaster-eval`

Scripts (`scripts/`):

- `eval-edit-quality.ps1` (accepts `-Original` / `-Edited` arguments)
- `run-eval-harness.ps1` (accepts `-FixtureOriginal` / `-FixtureEdited`)

## Notes

- These MP4s are **not** bundled with releases.
- If clone size becomes a problem, migrate this directory to Git LFS rather
  than deleting — the eval harnesses depend on the fixtures being on disk.
