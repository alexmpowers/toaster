# `eval/fixtures/` — test fixture media

This directory holds **eval / fixture media** used by the precision and
audio-fidelity harnesses. It is intentionally committed to the repo so the
evals are reproducible across machines and CI.

> **Do not delete or rename files here without updating every dependent
> skill, agent, and script listed below.** They reference these paths
> verbatim.

## Contents

| File | Purpose |
|---|---|
| `toaster_example.mp4` | Original real-speech source clip (~24 MB). |
| `toaster_example-edited.mp4` | Hand-edited baseline used as the parity target (~4.4 MB). |
| `*.png` | Screenshots used by README / docs. |

## Dependents

Skills (`.github/skills/`):
- `audio-boundary-eval/SKILL.md`
- `transcript-precision-eval/SKILL.md`
- `transcription-adapter-contract/SKILL.md`

Agents (`.github/agents/`):
- `cut-drift-fuzzer.md`
- `eval-harness-runner.md`
- `waveform-diff.md`

Scripts (`scripts/`):
- `eval-edit-quality.ps1` (default `-Original` / `-Edited` arguments)

## Notes

- These MP4s are **not** bundled with releases.
- If clone size becomes a problem, migrate this directory to Git LFS rather
  than deleting — the eval harnesses depend on the fixtures being on disk.
