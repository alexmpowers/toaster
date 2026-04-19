# Area categorization: caption-parity-eval

Check all areas this feature touches:

- [ ] Frontend (React/TS)        — no production changes; harness
      compares against the backend `CaptionLayout` the preview
      already consumes. No DOM render required.
- [x] Backend (Rust managers)    — read-only dependency on
      `managers/captions/{layout,ass,fonts}.rs`. A small test
      binary / `cargo test` hook may be added under
      `src-tauri/tests/caption_parity/` to dump `CaptionBlock[]`
      deterministically for the PowerShell harness.
- [ ] Audio path
- [ ] Transcription adapter
- [x] Export pipeline            — ASS emission + FFmpeg
      `subtitles` filter read-only; harness shells out to FFmpeg
      to render a transparent-background frame for alpha
      geometry extraction.
- [x] Captions / UI strings (i18n) — captions geometry only; NO
      new user-visible strings. i18n parity remains green.
- [ ] Settings UI
- [x] Evals                      — primary delivery area. New
      `scripts/eval/eval-caption-parity.ps1` + fixtures under
      `eval/caption-parity/`.
