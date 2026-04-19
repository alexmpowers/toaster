# Area categorization: unified model catalog

Check all areas this feature touches:

- [x] Frontend (React/TS) — Models page + Post Processing page picker.
- [x] Backend (Rust managers) — `managers/model/` + `managers/llm/`
      wiring + Tauri command surface.
- [ ] Audio path
- [ ] Transcription adapter — contract unchanged; only the metadata
      home changes.
- [ ] Export pipeline
- [x] Captions / UI strings (i18n) — 20 locale files need new/renamed
      keys (filter labels, category badges, "Manage models" link copy).
- [x] Settings UI — Models page layout + Post Processing settings
      rewire.
- [x] Evals — `handy-legacy-pruning` + `dep-hygiene` applicable; no
      new eval harnesses required.
- [x] Persistent settings schema — `post_process_local_model_id`
      migration.
