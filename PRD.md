# Toaster - Product Requirements Document

## 1. Product Vision

**Toaster** is a local-first desktop application for transcript-driven editing of spoken video and audio. Users open a recording, generate a word-level transcript, and edit the media by editing text. The product should feel close to **OBS Studio** in architecture and workspace behavior, and closer to **Audiate** in day-to-day editing flow.

### Core Principle

> "Edit spoken media by editing text, with native desktop speed and OBS-style docked workflow."

### Product Stance

1. **Local-first.** No cloud service required for core editing.
2. **No LLM dependency for cleanup.** Filler detection, pause handling, silence, and restore must work deterministically.
3. **OBS-like and compliant.** Native menu bar, dockable panels, reusable shared core, and safe OBS host integration.
4. **Non-destructive first.** Users can delete, silence, shorten, split, and restore without losing original media.
5. **One shared engine.** Standalone app and OBS host integration consume same core edit/session/export library.

---

## 2. Competitive Direction

### Keep from CutScript

- Transcript-first editing
- Fast open -> transcribe -> edit -> export flow
- Word-level transcript with waveform and preview
- Project save/load
- Caption export and optional burn-in

### Drop from CutScript

- Electron shell
- Python backend process
- Localhost HTTP bridge between UI and backend
- AI provider settings, API keys, Ollama/OpenAI/Claude routing
- LLM-based filler detection
- LLM-based clip suggestion as core workflow

### Learn from Audiate

- Suggested Edits as first-class workflow
- Silence vs Delete as separate actions
- Pause shortening as batch operation
- Editing Mode vs Transcript/Caption Mode
- Waveform editor for precise timing
- Restore markers and reversible edits
- Fast keyboard-first cleanup

### Learn from OBS

- Native menu bar and dock system
- Layout persistence and dock reset/lock behavior
- Profiles and reusable presets
- Stable crash handling and safe-mode expectations
- Shared core used by both standalone shell and host/plugin integration

---

## 3. Target Users

- YouTube creators editing talking-head videos, tutorials, and explainers
- Podcast and interview producers cleaning speech-heavy audio/video
- OBS users who want transcript-aware cleanup and offline cut/export workflow
- Educators and trainers producing narrated lessons and demos

---

## 4. Platform and Language Strategy

### 4.1 Target Platforms

1. **Windows** first
2. **macOS** second
3. **Linux** third

### 4.2 Programming Languages and Runtime

| Layer | Choice | Why |
|---|---|---|
| Core edit/session/export engine | **C17** | Stable ABI, plugin-friendly, easy OBS/shared-library integration |
| Desktop frontend | **C++20 + Qt 6 Widgets** | Native desktop menus, docks, waveform/preview UI, OBS alignment |
| OBS host integration | **C++20 + Qt 6 + OBS frontend/plugin APIs** | Dock reuse, native plugin compliance |
| Media I/O | **FFmpeg** | Proven decode/encode path and broad format support |
| Local transcription plugin | **C/C++ local ASR plugin** | Avoid Python service dependency; allow whisper.cpp or equivalent |
| Build | **CMake** | Cross-platform and OBS-aligned |

### 4.3 Human Languages

#### Launch

- **UI language:** English
- **Transcript cleanup language:** English
- **Caption export language:** whatever transcription returns, but English-first QA

#### Expansion Packs

Recommended next spoken-language packs:

- Spanish
- Portuguese (Brazil)
- French
- German
- Japanese

Each language pack must include:

- ASR support
- filler dictionary
- ignore dictionary
- repeated-word rules
- punctuation/caption spacing rules

---

## 5. UX Layout

### 5.1 Primary Workspace

- **Center-left:** video preview or audio preview surface
- **Right:** transcript editor
- **Bottom:** waveform/timeline
- **Optional right dock rail:** suggested edits, inspector, export, project info

### 5.2 Editing Modes

1. **Edit Mode**
   - deleting words deletes media
   - silencing keeps duration but removes sound
   - pause shortening changes effective duration
2. **Transcript Mode**
   - edits transcript text for captions/script only
   - media timing unchanged
3. **Boundary Mode**
   - trim, roll, ripple, split, keyboard nudge, timing repair

---

## 6. Menu Bar and Dock Model

Toaster should use a native desktop menu bar with OBS-like structure.

### 6.1 Top-Level Menus

| Menu | Purpose | Required Entries |
|---|---|---|
| **File** | media/project lifecycle | New Project, Open Media, Open Project, Save Project, Save Project As, Import Transcript, Export Media, Export Captions, Export Script, Exit |
| **Edit** | direct editing actions | Undo, Redo, Delete, Silence, Unsilence, Restore, Split at Playhead, Edit Timing, Find, Find and Replace, Select All Fillers, Select All Pauses |
| **View** | interface visibility | Show/Hide Transcript, Waveform, Inspector, Suggested Edits, Status Bar, Always on Top, Reset UI, Zoom controls |
| **Docks** | OBS-style panel control | Transcript Dock, Preview Dock, Waveform Dock, Inspector Dock, Suggested Edits Dock, Export Dock, Logs Dock, Lock Docks, Reset Docks |
| **Project** | edit-session settings | Project Settings, Relink Media, Language, Caption Options, Rebuild Transcript Cache |
| **Profiles** | reusable presets | Cleanup Profiles, Export Profiles, Shortcut Profiles |
| **Tools** | analysis and utilities | Transcribe, Re-transcribe, Suggested Edits, Filler Dictionary, Pause Detection, Audio Cleanup, Batch Export, OBS Integration Tools |
| **Help** | support and diagnostics | Keyboard Shortcuts, Documentation, Release Notes, Logs, Crash Reports, Check for Updates, About |

### 6.2 Docking Rules

- Every major panel must be dockable.
- Dock visibility must be toggled from **Docks** menu.
- Layout must persist per user.
- Users must be able to **Reset Docks** and **Lock Docks**.

### 6.3 OBS Host Rule

When hosted inside OBS:

- Toaster surfaces must appear as **dock(s)** and **tool actions**
- Toaster must not replace OBS main window behavior
- destructive cut/ripple/export work must stay outside render/audio callbacks

---

## 7. Core User Stories

### P0 - Must Have

1. Open audio or video file and preview it locally.
2. Transcribe media into word-level text with timestamps.
3. Click transcript words to seek playback.
4. Select transcript words and **Delete**, **Silence**, or **Restore** them.
5. Save/load a project file that preserves transcript, edits, and settings.
6. Export cleaned media to a new file.
7. Export SRT/VTT captions and plain-text script.
8. Search transcript and run find/replace.
9. Show waveform and keep transcript/playhead/waveform selection in sync.
10. Support undo/redo for all edit actions.

### P1 - Suggested Edits Without LLM

1. Detect filler words from built-in dictionaries and repeated-word heuristics.
2. Let users **Silence Filler Words** or **Delete Filler Words** in batch.
3. Detect pauses using audio energy and duration thresholds.
4. Let users **Silence Pauses**, **Delete Pauses**, or **Shorten Pauses** in batch.
5. Support custom filler lists and ignore lists.
6. Support restore markers for deleted spans.
7. Support transcript-only correction mode for captions/scripts.

### P2 - Precision Editing

1. Split word at playhead.
2. Edit transcription timing with handles.
3. Drag boundary markers.
4. Roll adjacent word boundaries.
5. Ripple edit selected spans.
6. Snap to zero crossings, nearby words, and optional time grid.
7. Apply seam smoothing on edited joins during export.
8. Support keyboard nudge for fine timing control.

### P3 - OBS Workflow

1. Provide OBS dock with transcript, suggested edits, and project status.
2. Allow safe non-destructive tagging inside OBS host.
3. Queue destructive cleanup/export to offline workflow.
4. Preserve OBS-friendly dock behavior and crash-safe startup.

---

## 8. Deterministic Cleanup Engine

### 8.1 Filler Detection

Must not require prompt calls or external LLMs.

Inputs:

- transcript words
- timestamps
- per-word confidence
- optional speaker ID

Rules:

1. Exact filler lexicon
2. phrase lexicon (`you know`, `kind of`, `sort of`)
3. repeated-word detection (`I I I`, `the the`)
4. sentence-initial soft fillers (`so`, `well`, `actually`) behind confidence rules
5. user ignore list
6. per-language dictionaries

Outputs:

- suggested delete list
- suggested silence list
- reviewable reason tags

### 8.2 Pause Detection

Inputs:

- audio RMS / energy envelope
- optional breath/noise classifier
- transcript gap timing

Rules:

1. configurable minimum pause duration
2. configurable silence threshold
3. preserve breaths when user wants natural cadence
4. separate **silence** from **shorten**

Outputs:

- pause spans
- suggested shorten targets
- reviewable batch actions

---

## 9. Keyboard Model

Default shortcuts should be simple and editor-friendly:

- Space - Play/Pause
- J / K / L - Shuttle backward, pause, shuttle forward
- Left / Right - Seek small step
- Shift+Left / Shift+Right - Seek larger step
- Delete / Backspace - Delete selection
- Ctrl+Delete - Silence selection
- Ctrl+Z / Ctrl+Shift+Z - Undo / Redo
- Ctrl+F - Find
- Ctrl+Alt+F - Find and Replace
- Ctrl+T - Split word at playhead
- Ctrl+Shift+T - Edit transcription timing
- Ctrl+1 - Toggle waveform
- Ctrl+E - Export
- Alt+[ / Alt+] - Boundary nudge earlier/later

---

## 10. Architecture

```text
Standalone Qt App                OBS Host
-----------------                -----------------
Menu bar + docks                 OBS dock + tool entry
Preview + transcript             Transcript dock
Waveform + inspector             Non-destructive actions
        \                              /
         \                            /
          ---------- libtoaster ----------
          session model | edit engine
          filler rules  | pause detector
          project I/O   | exporter API
          local ASR     | plugin registry
                   |
             FFmpeg / local ASR / caption writer
```

### Architectural Rules

1. `libtoaster` must know nothing about Qt.
2. Frontend and OBS host both consume same C API.
3. Plugin ABI remains stable and native.
4. No Python child process.
5. No localhost HTTP bridge.
6. No cloud API key management in core UX.

---

## 11. Data Model

Project file must preserve:

- source media path
- transcript words and segments
- speaker IDs when available
- deleted spans
- silenced spans
- pause edits
- split points
- boundary edits
- export settings
- selected language and cleanup profile
- dock/workspace layout metadata

---

## 12. Dependencies

| Dependency | Purpose |
|---|---|
| FFmpeg | decode, seek, export |
| Qt 6 Widgets | native desktop UI and docks |
| CMake | build |
| Local ASR engine plugin | transcription without Python service |
| OBS Studio SDK | OBS host integration |

---

## 13. Non-Goals

- No LLM-required filler detection
- No prompt-based clip suggestion in MVP
- No Electron frontend
- No Python backend service
- No mandatory cloud transcription
- No unsafe live destructive edits on active OBS render/audio path

---

## 14. Milestones

The milestones below describe product direction. Sections **16-19** translate them into
implementation-facing guidance grounded in the current repository baseline.

### Phase 1 - Native Core

- local session/edit engine
- project save/load
- transcript editing
- export media and captions

### Phase 2 - Suggested Edits

- deterministic filler detection
- pause detection/shorten/silence
- transcript-only edit mode
- timing editor and split word

### Phase 3 - Precision + Polish

- boundary trim/roll/ripple
- seam smoothing
- dock layout persistence
- robust shortcut map

### Phase 4 - OBS Integration

- OBS dock
- tool entry points
- offline export handoff
- crash-safe / safe-mode-aware integration

---

## 15. Success Criteria

1. User can clean up a spoken clip without touching a traditional timeline for common edits.
2. Filler and pause cleanup works offline with no API keys and no prompt calls.
3. UI feels native and dockable, not browser-like.
4. Standalone app and OBS integration share one edit core.
5. Exported media, captions, and script remain synchronized after edits.

---

## 16. Current Implementation Baseline

This section reflects the repository as it exists today and should be used as the starting
point for implementation planning.

### 16.1 Core engine baseline

- `libtoaster/` already contains a shared transcript/session core with word-state flags,
  delete/silence/restore operations, cut spans, keep-segment generation, project save/load,
  suggestion-list APIs, filler analysis, pause analysis, and signal infrastructure.
- `test/` already covers edit behavior, project round-trip, analysis, timeline/keep-segment
  math, signals, and a Windows frontend automation smoke path.
- The next build phases should treat these core contracts as the foundation, not reopen them
  casually without a clear product reason.

### 16.2 Frontend baseline

- `frontend/` already provides a native Qt shell with dock creation, menu creation, media/project
  open/save/export entry points, playback wiring, transcript search/navigation, waveform
  rendering, cleanup-analysis actions, and automation entry points.
- The frontend therefore already has meaningful structure. The near-term goal is to make the
  existing workflow reliable and coherent end-to-end rather than replace the UI stack.

### 16.3 Baseline gap

- The repository now has real foundations, but the earlier PRD jumped from product intent to
  milestone labels without bridging through current implementation state.
- The implementation plan should now focus on integrating the existing core + frontend + tests
  into a clearly usable standalone workflow before expanding OBS-specific scope.

---

## 17. Implementation Notes from Comparative Research

The following notes are implementation guidance and do not change the product stance above.

### 17.1 Borrow from Handy

- For any future live capture / push-to-talk / speech-triggered workflow, normalize capture
  early, use smoothed VAD, preserve brief speech lead-in/tail, and preload/unload transcription
  models around user intent.
- Treat recording, cancel, processing, and paste as one coordinated lifecycle rather than a set
  of unrelated callbacks.
- Keep optional AI polishing above the deterministic core workflow rather than inside it.

### 17.2 Borrow from CutScript

- Keep transcript-first editing and the "edit transcript -> derive keep segments -> export"
  mental model.
- Show long-running pipeline state clearly during slow operations: loading, extracting audio,
  transcribing, aligning, exporting.
- Make export modes explicit: fast copy, full re-encode, sidecar captions, burn-in captions.
- Support batch-friendly cleanup review, but keep deterministic rules first.

### 17.3 Do not copy

- Do not reintroduce Electron, Python backend sidecars, localhost bridges, or LLM-required
  cleanup.
- Do not let competitive research blur the shared-core / OBS-safe boundary that defines Toaster.

---

## 18. Usable Standalone State

Before OBS integration becomes a primary build target, Toaster should reach a usable standalone
state.

A usable standalone build means the following workflow is reliable on Windows:

1. Open local audio/video and preview it.
2. Transcribe media or import a timestamped transcript.
3. Select transcript words and delete, silence, restore, and inspect results.
4. Keep playback, transcript selection, and waveform selection in sync.
5. Save and reload a project without losing edit state.
6. Export edited media plus sidecar captions/script.
7. Run deterministic filler/pause cleanup in reviewable batches.
8. Pass core regression tests and frontend smoke automation for the supported workflow.

Usable standalone state does **not** require full OBS integration, advanced boundary tools beyond
the defined core workflow, or every future language pack.

---

## 19. Phased Implementation Plan

The phase plan below is implementation-facing and should be used to sequence work from the
current repo baseline to a usable product.

### 19.1 Phase 1 - Usable Standalone Foundation

**Goal:** complete the minimum end-to-end standalone editing loop.  
**Parallelization:** mostly sequential until shared contracts are stable, then parallel by
workstream.

**Sequential foundation work**

- Finalize transcript / project / export contract expectations in `libtoaster`.
- Confirm end-to-end media load, transcription import/transcribe flow, and export-path
  ownership.
- Define the exact "definition of done" for usable standalone state.

**Parallel workstreams after the foundation is stable**

- **Core/session track:** transcript state transitions, project persistence polish, keep-segment
  correctness.
- **Media/export track:** preview reliability, waveform loading, export correctness, caption/script
  outputs.
- **Frontend workflow track:** selection/edit actions, inspector clarity, transcript
  search/navigation, dock cohesion.
- **Quality track:** test coverage, automation smoke flow, and repeatable Windows build/run
  validation.

**Phase exit**

- The Windows standalone app can reliably perform the P0 workflow without timeline-first editing.

### 19.2 Phase 2 - Deterministic Cleanup and Guided Editing

**Goal:** deliver the offline cleanup workflow that makes the product meaningfully better than
manual editing.  
**Parallelization:** parallel inside an agreed data model; sequential only where core structures
must change.

**Workstreams**

- **Filler engine track:** dictionaries, repeated-word handling, ignore lists, review reasons.
- **Pause engine track:** move from transcript-gap-only logic toward the PRD's intended
  audio-energy-aware detection.
- **Review UX track:** suggestion-list quality, batch-apply flows, transcript-only correction
  mode.
- **Recovery track:** restore markers, reversible cleanup behavior, regression cases.

**Phase exit**

- Users can run deterministic filler and pause cleanup with reviewable batch actions and
  reversible results.

### 19.3 Phase 3 - Precision Editing and Release Hardening

**Goal:** improve timing control, polish, and release confidence.  
**Parallelization:** mixed; precision-editing features can progress in parallel, but release
hardening gates phase exit.

**Workstreams**

- Boundary editing, split/roll/ripple behavior, and timing repair.
- Seam smoothing and edited-join quality during export.
- Undo/redo depth, shortcut completeness, dock persistence, and workflow polish.
- Packaging, crash handling expectations, logging, and repeatable release validation.

**Phase exit**

- Standalone Toaster feels coherent, recoverable, and ready for real-world speech-editing
  sessions.

### 19.4 Phase 4 - OBS-safe Integration

**Goal:** reuse the shared core in OBS without weakening host safety.  
**Parallelization:** mostly sequential after standalone contracts are stable.

**Workstreams**

- OBS dock/tool entry design.
- Non-destructive tagging inside host.
- Offline destructive cleanup/export handoff.
- Crash-safe and safe-mode-aware host behavior.
- Shared-core parity checks between standalone and OBS flows.

**Phase exit**

- OBS integration adds tagging and handoff value without duplicating the edit core or moving
  destructive work into unsafe host paths.

### 19.5 Sequential vs Parallel Summary

| Workstream | Depends on | Can run in parallel with |
|---|---|---|
| Shared transcript/project/export contracts | none | little; this is the main sequential foundation |
| Standalone media/export pipeline | contract freeze | transcript UI and quality tracks |
| Transcript/editor UX | contract freeze | media/export and quality tracks |
| Deterministic cleanup engines | stable transcript/timing model | cleanup review UX and recovery work |
| Automation, regression, packaging | each active phase | almost all tracks, but it blocks phase exit |
| OBS integration | stable standalone product and export handoff model | limited parallelism; mostly late-phase sequential work |

### 19.6 Recommended next build phase

After this PRD update lands, the next build phase should focus on **Phase 1 - Usable Standalone
Foundation**. That is the shortest path to a product people can actually use, and it creates the
stable contracts that later cleanup, precision editing, and OBS work depend on.
