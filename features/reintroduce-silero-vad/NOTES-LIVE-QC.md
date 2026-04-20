# Silero VAD — live QC evidence (R-002 prefilter)

Recorded 2026-04-19 during Phase 5 QC on `feat/vad-runtime-delta-r002`.

## Why "And uh" → "And" after enabling the prefilter

The user observed a splice that previously bled "And uh" now plays back
as a clean "And". Two independent mechanisms explain this — both
visible in the launch log `launch-20260419-203520.stdout.log`:

1. **Prefilter skipped the disfluency.** Log line:
   `VAD prefilter: 5 window(s) covering 14620000/21717312 µs (67.3% of
   buffer)`.
   Parakeet was only invoked on the 67.3 % of the buffer that Silero
   classified as speech. The remaining 32.7 % (inter-utterance silence
   + low-energy filler onsets that failed to trigger the default
   `onset_frames = 2` threshold) was never transcribed. A short "uh"
   whose onset is below the threshold simply never reaches the ASR.
2. **Boundary refinement snapped the splice.** Log line:
   `VAD boundary refinement: computed 723 frame probabilities`.
   The playback cut uses the per-frame P(speech) curve to snap into
   the deepest silence valley rather than the pre-feature
   zero-crossing + 20 ms energy-valley heuristic. Even if a fragment
   like "uh" were transcribed, the cut would now land beyond its
   acoustic tail.

## Regression framing

This is content-aware noise suppression, not magic. A very short real
word (e.g. "um" pronounced as a standalone discourse marker vs. "I'm")
could, in principle, be skipped by the same mechanism.

- Regression gate: `transcript-precision-eval`. Re-run before opening
  the PR and cite the word-count delta against the baseline fixture
  in the PR body.
- Graceful absence: AC-005-c — when the Silero ONNX is not installed,
  `TranscriptionManager::transcribe()` falls back to the full-file
  engine path unmodified. Prefilter and boundary-refine are both
  no-ops when the model is missing.

## Live evidence pointers

- Prefilter fires: `.launch-monitor/launch-20260419-203520.stdout.log`
  — grep `VAD prefilter:`.
- Boundary-refine fires: same log — grep
  `VAD boundary refinement:`.
- Binary wiring gate: `pwsh scripts/eval/eval-vad.ps1` → G9
  `prefilter_live_wired` must remain PASS.


## 2026-04-19 round 2 — prefilter default flipped to OFF

Live QC on commit `f37c70b` with prefilter enabled surfaced broader
word loss than the single "And uh" case above. Filler words and
short disfluencies are being dropped wholesale; audio edits derived
from the resulting word list are "wildly off". Refine-boundaries
(R-003) behaves correctly.

**Action (this commit):** flip `default_vad_prefilter_enabled` →
`false` in both backend (`src-tauri/src/settings/defaults.rs`) and
frontend fallbacks (`VadPrefilterToggle`, `VadStatusPill`). The
refine-boundaries default stays off as well. Users who opt in now
get a description warning about the word-loss tradeoff.

### Word-loss hypotheses (to investigate before re-flipping default)

- **H-A — sub-onset-threshold clipping (most likely).**
  `DEFAULT_ONSET_FRAMES = 2` + no pre/post pad means a low-energy
  onset (voiced "uh" at breath, word-initial stops) is discarded
  before the window opens. Candidate fix: add `pre_pad_ms ~= 100` and
  `post_pad_ms ~= 100` to every extracted speech window in
  `prefilter.rs` before feeding it to ASR.
- **H-B — offset-shift drift.**
  `offset_timestamps` shifts each window's returned word timestamps
  by that window's start-us. If the inter-window skipped silence is
  not exactly what the shift accounts for (rounding, sample-vs-frame
  boundary), a resulting `word.start_us` can land inside a deleted
  range → splice boundary snap then chooses the wrong side.

### Re-flip criteria

Default returns to `true` only after:

1. `scripts/eval/eval-edit-quality.ps1` A/B (prefilter OFF vs ON) on
   the same fixture shows no regression in word count and no
   regression in boundary-error distribution.
2. Runtime delta gate (`G6_runtime_delta`) shows measurable speedup.
3. Live-app QC on a filler-heavy fixture confirms no audible word
   loss at the splice.
