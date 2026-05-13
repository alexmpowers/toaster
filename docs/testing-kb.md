# Toaster Testing Knowledge Base

> Project-wide knowledge that accumulates across features. Inspired by
> afkode's "Testing Knowledge Base" (afkode.ai/docs § Runtime Journal).
> Every feature's QC tasks should append discoveries here so subsequent
> features do not re-hit the same walls.
>
> AGENTS.md remains canonical for _rules_. This file is for _empirical
> testing facts_ discovered during real runs.

## Format

Append entries chronologically under the relevant section. Each entry:

```
### YYYY-MM-DD — short title (feature: <slug>)
- Discovery: ...
- Implication: ...
- Citation: <file:line> or <command + observed output>
```

## Discoveries (append below this line)

<!-- Each feature's QC tasks append here. -->

## Build environment / vcvars

### 2026-04-18 — Strip-before-preflight ordering in setup-env.ps1 (feature: build-env-ninja-hardening)

- Discovery: In `scripts/setup-env.ps1`, the Ninja-hostile-vars strip
  block (lines ~41-63) runs _before_ the preflight that signals
  `$global:ToasterEnvPreflightOk` (lines ~133-145). Any tracked var set
  before sourcing setup-env (e.g. `$env:CMAKE_GENERATOR_INSTANCE='C:\stub'`)
  is wiped by strip and never observed by the preflight.
- Implication: To validate the preflight in QC, set the tracked var
  _after_ sourcing setup-env and re-run only the preflight block. Do not
  expect a pre-source injection to flip `ToasterEnvPreflightOk` to
  `$false`. The preflight is a defensive backstop for _future_ code paths
  that might re-export a tracked var after strip; the strip block is the
  primary defense against vcvars leakage.
- Citation: `scripts/setup-env.ps1:41-145`;
  `features/build-env-ninja-hardening/journal.md` "QC complete" entry.

### 2026-04-18 — Monitored-launch warm vs cold timings (feature: build-env-ninja-hardening)

- Discovery: A cold `cargo tauri dev` triggered by
  `scripts/launch-toaster-monitored.ps1 -ObservationSeconds 300` on this
  dependency tree (whisper-rs-sys + ffmpeg-sys + Tauri stack) takes
  roughly 4-5 minutes from "Setting up Toaster build environment..." to
  the first `tauri-ready` success signal. A warm relaunch (build cache
  intact, no source changes) reaches `launch_status=launched_ok` inside a
  60-second observation window with cycles to spare.
- Implication: Use `-ObservationSeconds 60` for warm relaunches to keep
  monitor cycles cheap; reserve 300+ for the first build of a session or
  after a `cargo clean`. Do not retry under 10 minutes if the cold path
  appears stuck — it is almost certainly compiling, not hung
  (per AGENTS.md "Cargo runtime expectations").
- Citation: `.launch-monitor/launch-20260418-115148.*.log` (cold, ~5
  minutes wall-clock) vs `.launch-monitor/launch-20260418-120109.*.log`
  (warm, exit 0 inside 60s).
