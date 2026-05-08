# Feature Freeze

**Status:** Active  
**Effective:** 2026-05-09  
**Lift condition:** All v0.1.1 readiness gates pass (see below)

## Why

Toaster's UX layer is solid (playback sync, keyboard nav, find & replace, inline editing).
The core problem is **transcript accuracy** — word-level timestamps from the ASR pipeline
are not precise enough for confident editing. Until this is fixed, no new features ship.

## What's frozen

All 24 features in `features/` that are not marked `DEFERRED.md` are frozen.
No new feature engineering, no new feature folders, no new PRDs.

**Allowed during freeze:**
- Bug fixes to the transcription/timestamp pipeline
- Eval infrastructure improvements
- Model picker simplification
- Build/CI/DX improvements
- Documentation updates

## Triage results

| Action | Features |
|--------|----------|
| **Killed** | `shorts-extractor`, `example-pm-dryrun`, `readme-launch-pass` |
| **Deferred** | `brand-title-sizing`, `chapter-markers`, `export-hardware-encoder`, `hardware-aware-model-picker`, `speaker-diarization`, `time-stretch-segments` |
| **Frozen** | All remaining 24 features |

## v0.1.1 readiness gates

All must pass to lift the freeze:

1. Transcript accuracy: oracle eval median ≤ 100ms, p95 ≤ 500ms on all fixture videos
2. Playback sync: active word highlight matches spoken word
3. Edit precision: delete word → no audible remnant
4. Export parity: exported video matches preview (captions + audio)
5. Punctuation: periods, commas, question marks visible in transcript
6. Paragraph structure: pause-aware breaks, not a wall of text
7. Model picker: Parakeet V3 default, no broken models shown
8. Build health: `npm run build` + `cargo check` + `cargo clippy` + `cargo test` green
9. Eval harness: all 5 evals pass (precision, boundary, caption-parity, edit-quality, transcript-accuracy)
