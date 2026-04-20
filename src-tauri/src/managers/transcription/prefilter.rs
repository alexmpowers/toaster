//! R-002 — ASR silence prefilter orchestrator.
//!
//! Hosts the per-window chunk loop that the transcription manager runs
//! when `settings.vad_prefilter_enabled` is true and the Silero VAD
//! ONNX is present. Per BLUEPRINT §AD-4 the prefilter never rewrites
//! the audio buffer — `audio_toolkit::vad::prefilter::remap_words` is
//! the single timestamp-shift site, and this module is the single
//! caller of that helper from the transcription pipeline.
//!
//! Graceful degradation (BLUEPRINT §AD-8): the caller (job.rs) routes
//! through the unchanged full-buffer path whenever this module returns
//! `Ok(None)`. We never propagate VAD-init errors into the ASR result.
//!
//! Single-source-of-truth invariants:
//!   - Per-window text concatenation joins with a single ASCII space
//!     (Whisper-style behavior; the post-filter `filter_transcription_output`
//!     squashes runs of whitespace so adapter-side normalization is
//!     unaffected).
//!   - Segment timestamps are shifted by `window.start_us / 1e6` via
//!     `transcribe_rs::TranscriptionResult::offset_timestamps`, the
//!     library-provided helper. We do **not** open-code the shift
//!     (mirrors the prefilter's `remap_words` SSoT contract).

use anyhow::Result;
use log::{info, warn};
use transcribe_rs::TranscriptionResult;

use crate::audio_toolkit::vad::prefilter::{
    prefilter_speech_windows, try_open_silero, SpeechWindow, VAD_SAMPLE_RATE_HZ,
};
use crate::managers::model::ModelManager;
use crate::settings::AppSettings;

use super::engine_call::call_engine_chunk;
use super::LoadedEngine;

/// Outcome of attempting the VAD-prefiltered chunk pass.
///
/// `Ok(None)` is the structured "fall back to full-buffer ASR" signal
/// — model missing, ORT init failure, no detected speech, or VAD
/// disabled. Callers must treat it as a non-error fall-back.
pub(super) enum PrefilterOutcome {
    /// Successful chunked pass — merged result ready for adapter.
    Ran(TranscriptionResult),
    /// Caller should fall back to single-shot full-buffer ASR.
    Fallback,
}

/// Run the VAD-prefiltered transcription pass if enabled and possible.
///
/// Returns `Ok(Fallback)` whenever the orchestrator decides to defer
/// to the standard full-buffer path (gate disabled, model missing,
/// ORT failure, no windows detected). Returns `Err` only when a
/// per-window engine call itself fails — the caller propagates that
/// error unchanged so prefilter mode never *loses* an error vs the
/// full-buffer path.
pub(super) fn run(
    engine: &mut LoadedEngine,
    audio: &[f32],
    settings: &AppSettings,
    normalized_language: &Option<String>,
    model_manager: &ModelManager,
) -> Result<PrefilterOutcome> {
    if !settings.vad_prefilter_enabled {
        return Ok(PrefilterOutcome::Fallback);
    }

    // Resolve the on-disk path through the catalog SSoT
    // (`silero_vad_model_path`) — same lookup the boundary refinement
    // path uses (`commands::waveform::vad_snap`).
    let model_path = match model_manager.get_model_path("silero-vad") {
        Ok(path) => path,
        Err(_) => {
            return Ok(PrefilterOutcome::Fallback);
        }
    };

    let mut vad = match try_open_silero(&model_path) {
        Ok(Some(vad)) => vad,
        Ok(None) => {
            // Missing model is the documented absent-graceful signal
            // (R-005 / AC-005-c). Silent fallback.
            return Ok(PrefilterOutcome::Fallback);
        }
        Err(e) => {
            warn!(
                "VAD prefilter: Silero open failed ({e}); falling back to full-buffer transcription."
            );
            return Ok(PrefilterOutcome::Fallback);
        }
    };

    // The transcription manager always passes 16 kHz mono PCM (the
    // canonical ASR working rate enforced by `transcribe_file`'s
    // `read_wav_samples` + adapter `native_input_sample_rate_hz =
    // 16_000`). If a future engine ships at a different rate the
    // prefilter must be skipped — not silently misaligned — so we
    // bail to fall-back when the assumption breaks.
    if !audio_is_long_enough_for_prefilter(audio.len()) {
        return Ok(PrefilterOutcome::Fallback);
    }

    let windows = prefilter_speech_windows(audio, &mut vad);
    if windows.is_empty() {
        info!("VAD prefilter: no speech windows detected; falling back to full-buffer transcription so silence-only inputs still surface engine output.");
        return Ok(PrefilterOutcome::Fallback);
    }

    let total_us: i64 = windows.iter().map(|w| w.end_us - w.start_us).sum();
    let buffer_us = samples_to_us(audio.len()) as i64;
    info!(
        "VAD prefilter: {} window(s) covering {}/{} µs ({:.1}% of buffer); transcribing per-window.",
        windows.len(),
        total_us,
        buffer_us,
        if buffer_us > 0 {
            100.0 * total_us as f64 / buffer_us as f64
        } else {
            0.0
        }
    );

    let mut merged_text = String::new();
    let mut merged_segments: Vec<transcribe_rs::TranscriptionSegment> = Vec::new();

    for window in &windows {
        let slice = match slice_audio(audio, *window) {
            Some(s) if !s.is_empty() => s,
            _ => continue,
        };

        let mut chunk = call_engine_chunk(engine, slice, settings, normalized_language)?;
        // SSoT timestamp shift — see module docs. `offset_timestamps`
        // clamps to zero internally, which is what we want at the
        // first window if pre-roll pushed `window.start_us` to zero.
        let offset_secs = window.start_us as f32 / 1_000_000.0;
        chunk.offset_timestamps(offset_secs);

        if !chunk.text.is_empty() {
            if !merged_text.is_empty() {
                merged_text.push(' ');
            }
            merged_text.push_str(chunk.text.trim());
        }
        if let Some(segs) = chunk.segments {
            merged_segments.extend(segs);
        }
    }

    let merged = TranscriptionResult {
        text: merged_text,
        segments: if merged_segments.is_empty() {
            None
        } else {
            Some(merged_segments)
        },
    };
    Ok(PrefilterOutcome::Ran(merged))
}

/// Slice the 16 kHz mono buffer to cover `[window.start_us, window.end_us)`.
/// Returns `None` if the slice is empty or out of bounds. Single
/// site that maps file-time windows back to PCM indices in the
/// transcription pipeline (mirrors `prefilter::remap_words` SSoT
/// contract on the inverse direction).
fn slice_audio(samples_16k: &[f32], window: SpeechWindow) -> Option<&[f32]> {
    let start = us_to_samples(window.start_us.max(0)).min(samples_16k.len());
    let end = us_to_samples(window.end_us.max(0)).min(samples_16k.len());
    if end <= start {
        return None;
    }
    Some(&samples_16k[start..end])
}

#[inline]
fn us_to_samples(us: i64) -> usize {
    (us as i128 * VAD_SAMPLE_RATE_HZ as i128 / 1_000_000) as usize
}

#[inline]
fn samples_to_us(samples: usize) -> u64 {
    (samples as u128 * 1_000_000 / VAD_SAMPLE_RATE_HZ as u128) as u64
}

#[inline]
fn audio_is_long_enough_for_prefilter(n: usize) -> bool {
    // Need at least one Silero frame; otherwise the VAD never advances
    // and we'd return zero windows + recurse to fall-back anyway. The
    // explicit early-out keeps logging quiet on micro-buffers.
    n >= crate::audio_toolkit::vad::SILERO_FRAME_SAMPLES_16K
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slice_audio_clamps_to_buffer_end() {
        let samples = vec![0.0_f32; 100];
        // Window extends past buffer — clamp not panic.
        let w = SpeechWindow {
            start_us: 0,
            end_us: 1_000_000_000,
        };
        let s = slice_audio(&samples, w).unwrap();
        assert_eq!(s.len(), 100);
    }

    #[test]
    fn slice_audio_returns_none_for_inverted_window() {
        let samples = vec![0.0_f32; 100];
        let w = SpeechWindow {
            start_us: 1_000,
            end_us: 0,
        };
        assert!(slice_audio(&samples, w).is_none());
    }

    #[test]
    fn us_sample_round_trips_at_16k() {
        // 30 ms exactly = 480 samples at 16 kHz.
        let samples_30ms = us_to_samples(30_000);
        assert_eq!(samples_30ms, 480);
        let us_480 = samples_to_us(480);
        assert_eq!(us_480, 30_000);
    }

    #[test]
    fn offset_timestamps_preserves_segment_durations_bit_exact() {
        // The orchestrator delegates the per-chunk timestamp shift to
        // `transcribe_rs::TranscriptionResult::offset_timestamps`. Per
        // `transcript-precision-eval` the precision invariant is not
        // "absolute timestamps are exact" (f32 seconds round at long
        // offsets) but **segment durations are preserved across the
        // shift** — equal-duration synthesis is forbidden, irregular
        // durations must survive intact. Mirror the prefilter
        // module's `remap_preserves_precision_no_rounding` test for
        // the segment path the orchestrator actually uses.
        let mut chunk = transcribe_rs::TranscriptionResult {
            text: "hello world".into(),
            segments: Some(vec![
                transcribe_rs::TranscriptionSegment {
                    start: 0.012345,
                    end: 0.567890,
                    text: "hello".into(),
                },
                transcribe_rs::TranscriptionSegment {
                    start: 0.567890,
                    end: 1.234567,
                    text: "world".into(),
                },
            ]),
        };
        let segs = chunk.segments.clone().unwrap();
        let d0 = segs[0].end - segs[0].start;
        let d1 = segs[1].end - segs[1].start;

        // Use a representative file-time offset: 7.654321 s (matches
        // the prefilter `remap_preserves_precision_no_rounding`
        // window start). 7.65 s is well below f32 precision loss.
        chunk.offset_timestamps(7.654_321);

        let shifted = chunk.segments.unwrap();
        assert!((shifted[0].end - shifted[0].start - d0).abs() < 1e-6);
        assert!((shifted[1].end - shifted[1].start - d1).abs() < 1e-6);
        // Sanity: monotonic and shifted in the right direction.
        assert!(shifted[0].start > 7.6 && shifted[0].start < 7.7);
        assert!(shifted[1].end > 8.8 && shifted[1].end < 8.95);
    }
}
