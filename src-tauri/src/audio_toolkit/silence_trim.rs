//! Leading and trailing silence trimming for word boundaries.
//!
//! ASR engines commonly include pre-speech padding and post-speech silence
//! in their segment boundaries. When the DP forced aligner (or the
//! char-proportional fallback) pins the first/last word to those segment
//! edges, the result is words that highlight before/after the speaker
//! actually speaks.
//!
//! These functions detect the onset/offset of speech within a word's audio
//! span using frame-level RMS energy, and return tighter boundaries.

use super::forced_alignment::{frame_to_us, EnergyFrames};
use super::timing;

/// Minimum silence duration (µs) before we trim. Shorter silences are
/// treated as natural inter-phoneme pauses and left attached to the word.
const MIN_SILENCE_US: i64 = 200_000; // 200 ms

/// RMS energy threshold below which a frame is considered silent, expressed
/// as a fraction of the segment's peak energy. Intentionally lenient — we
/// only trim obvious silence, not quiet speech.
const SILENCE_FRACTION: f32 = 0.05;

/// Compute the sample-range slice for a `[seg_start_us, seg_end_us)` span.
/// Returns `None` if the range is empty or degenerate.
fn sample_range(
    samples: &[f32],
    seg_start_us: i64,
    seg_end_us: i64,
    sample_rate_hz: f64,
) -> Option<(usize, usize)> {
    if seg_end_us <= seg_start_us {
        return None;
    }
    let start = timing::us_to_sample_clamped(seg_start_us, sample_rate_hz, samples.len());
    let end_raw = timing::us_to_sample_clamped(seg_end_us, sample_rate_hz, samples.len());
    let end = if end_raw + 1 >= samples.len() {
        samples.len()
    } else {
        end_raw
    };
    if end <= start {
        None
    } else {
        Some((start, end))
    }
}

/// Detect where speech ends relative to the segment's end, and trim the
/// last word so it doesn't absorb a trailing pause.
///
/// Returns the trimmed `end_us` for the last word (≤ `seg_end_us`).
/// If no significant trailing silence is found, returns `seg_end_us`
/// unchanged.
///
/// Scans backward from the segment's end, looking for the last frame that
/// exceeds the silence threshold. Places the trimmed boundary 1 frame
/// (10 ms) after that frame to avoid clipping the phoneme tail.
pub fn trim_trailing_silence(
    samples: &[f32],
    seg_start_us: i64,
    seg_end_us: i64,
    sample_rate_hz: f64,
) -> i64 {
    let (start, end) = match sample_range(samples, seg_start_us, seg_end_us, sample_rate_hz) {
        Some(r) => r,
        None => return seg_end_us,
    };

    let slice = &samples[start..end];
    let frames = EnergyFrames::compute(slice, sample_rate_hz);
    if frames.frames.is_empty() {
        return seg_end_us;
    }

    let peak = frames.frames.iter().copied().fold(0.0f32, f32::max);
    if peak <= 1e-9 {
        return seg_end_us;
    }
    let threshold = peak * SILENCE_FRACTION;

    let last_speech_frame = match frames.frames.iter().rposition(|&e| e > threshold) {
        Some(f) => f,
        None => return seg_end_us,
    };

    let trim_frame = last_speech_frame + 1;
    let trim_us = frame_to_us(trim_frame, frames.hop_samples, sample_rate_hz, seg_start_us);

    if seg_end_us - trim_us >= MIN_SILENCE_US {
        trim_us
    } else {
        seg_end_us
    }
}

/// Mirror of [`trim_trailing_silence`] for the **leading** edge.
///
/// ASR engines commonly include pre-speech padding in segment boundaries,
/// causing the first word to start 100-300 ms before the speaker actually
/// begins. Scans *forward* from `seg_start_us`, finds the first energy
/// frame above the silence threshold, and returns a tighter start time.
///
/// Returns the trimmed `start_us` for the first word (≥ `seg_start_us`).
/// If no significant leading silence is found, returns `seg_start_us`
/// unchanged.
pub fn trim_leading_silence(
    samples: &[f32],
    seg_start_us: i64,
    seg_end_us: i64,
    sample_rate_hz: f64,
) -> i64 {
    trim_leading_silence_inner(
        samples,
        seg_start_us,
        seg_end_us,
        sample_rate_hz,
        SILENCE_FRACTION,
        MIN_SILENCE_US,
    )
}

/// Model-aware variant: when the engine is known to inject pre-speech
/// padding (e.g. Parakeet), uses a lower threshold and shorter minimum
/// silence window for more aggressive trim.
pub fn trim_leading_silence_padded(
    samples: &[f32],
    seg_start_us: i64,
    seg_end_us: i64,
    sample_rate_hz: f64,
) -> i64 {
    // More aggressive: 3% of peak (vs 5%) and 100ms min (vs 200ms).
    // Parakeet consistently adds 200-300ms of padding, so we want to catch
    // even shorter silence windows that the default would skip.
    const PADDED_SILENCE_FRACTION: f32 = 0.03;
    const PADDED_MIN_SILENCE_US: i64 = 100_000;
    trim_leading_silence_inner(
        samples,
        seg_start_us,
        seg_end_us,
        sample_rate_hz,
        PADDED_SILENCE_FRACTION,
        PADDED_MIN_SILENCE_US,
    )
}

fn trim_leading_silence_inner(
    samples: &[f32],
    seg_start_us: i64,
    seg_end_us: i64,
    sample_rate_hz: f64,
    silence_fraction: f32,
    min_silence_us: i64,
) -> i64 {
    let (start, end) = match sample_range(samples, seg_start_us, seg_end_us, sample_rate_hz) {
        Some(r) => r,
        None => return seg_start_us,
    };

    let slice = &samples[start..end];
    let frames = EnergyFrames::compute(slice, sample_rate_hz);
    if frames.frames.is_empty() {
        return seg_start_us;
    }

    let peak = frames.frames.iter().copied().fold(0.0f32, f32::max);
    if peak <= 1e-9 {
        return seg_start_us;
    }
    let threshold = peak * silence_fraction;

    let first_speech_frame = match frames.frames.iter().position(|&e| e > threshold) {
        Some(f) => f,
        None => return seg_start_us,
    };

    if first_speech_frame == 0 {
        return seg_start_us;
    }

    // 1 frame before speech onset to preserve the attack.
    let trim_frame = first_speech_frame.saturating_sub(1);
    let trim_us = frame_to_us(trim_frame, frames.hop_samples, sample_rate_hz, seg_start_us);

    if trim_us - seg_start_us >= min_silence_us {
        trim_us
    } else {
        seg_start_us
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── trailing ──────────────────────────────────────────────────────

    #[test]
    fn trailing_detects_long_pause() {
        let sr = 16_000.0;
        let speech_samples = (sr * 0.3) as usize;
        let silence_samples = (sr * 0.5) as usize;
        let total = speech_samples + silence_samples;
        let mut samples = vec![0.0f32; total];
        for (k, s) in samples.iter_mut().take(speech_samples).enumerate() {
            let t = k as f64 / sr;
            *s = 0.5 * (2.0 * std::f64::consts::PI * 300.0 * t).sin() as f32;
        }

        let seg_end_us = (total as f64 / sr * 1_000_000.0) as i64;
        let trimmed = trim_trailing_silence(&samples, 0, seg_end_us, sr);

        assert!(trimmed < seg_end_us, "Expected trim but got full segment");
        assert!(
            trimmed <= 350_000,
            "Trim point {trimmed} µs too far from 300 ms"
        );
        assert!(
            trimmed >= 290_000,
            "Trim point {trimmed} µs clipped into speech"
        );
    }

    #[test]
    fn trailing_preserves_short_pause() {
        let sr = 16_000.0;
        let speech_samples = (sr * 0.3) as usize;
        let silence_samples = (sr * 0.1) as usize;
        let total = speech_samples + silence_samples;
        let mut samples = vec![0.0f32; total];
        for (k, s) in samples.iter_mut().take(speech_samples).enumerate() {
            let t = k as f64 / sr;
            *s = 0.5 * (2.0 * std::f64::consts::PI * 300.0 * t).sin() as f32;
        }

        let seg_end_us = (total as f64 / sr * 1_000_000.0) as i64;
        let trimmed = trim_trailing_silence(&samples, 0, seg_end_us, sr);
        assert_eq!(
            trimmed, seg_end_us,
            "Short trailing silence should not be trimmed"
        );
    }

    #[test]
    fn trailing_handles_all_silent() {
        let samples = vec![0.0f32; 16_000];
        let trimmed = trim_trailing_silence(&samples, 0, 1_000_000, 16_000.0);
        assert_eq!(trimmed, 1_000_000, "All-silent should not be trimmed");
    }

    // ── leading ───────────────────────────────────────────────────────

    #[test]
    fn leading_detects_long_pause() {
        let sr = 16_000.0;
        let silence_samples = (sr * 0.5) as usize;
        let speech_samples = (sr * 0.3) as usize;
        let total = silence_samples + speech_samples;
        let mut samples = vec![0.0f32; total];
        for i in silence_samples..total {
            let t = i as f64 / sr;
            samples[i] = 0.5 * (2.0 * std::f64::consts::PI * 300.0 * t).sin() as f32;
        }

        let seg_end_us = (total as f64 / sr * 1_000_000.0) as i64;
        let trimmed = trim_leading_silence(&samples, 0, seg_end_us, sr);

        assert!(trimmed > 0, "Expected leading trim but got original start");
        assert!(
            trimmed >= 450_000,
            "Trim point {trimmed} µs too early — near 500 ms"
        );
        assert!(
            trimmed <= 520_000,
            "Trim point {trimmed} µs cut into speech"
        );
    }

    #[test]
    fn leading_preserves_short_pause() {
        let sr = 16_000.0;
        let silence_samples = (sr * 0.1) as usize;
        let speech_samples = (sr * 0.3) as usize;
        let total = silence_samples + speech_samples;
        let mut samples = vec![0.0f32; total];
        for i in silence_samples..total {
            let t = i as f64 / sr;
            samples[i] = 0.5 * (2.0 * std::f64::consts::PI * 300.0 * t).sin() as f32;
        }

        let seg_end_us = (total as f64 / sr * 1_000_000.0) as i64;
        let trimmed = trim_leading_silence(&samples, 0, seg_end_us, sr);
        assert_eq!(trimmed, 0, "Short leading silence should not be trimmed");
    }

    #[test]
    fn leading_handles_all_silent() {
        let samples = vec![0.0f32; 16_000];
        let trimmed = trim_leading_silence(&samples, 0, 1_000_000, 16_000.0);
        assert_eq!(trimmed, 0, "All-silent should not be trimmed");
    }

    // ── padded leading trim ───────────────────────────────────────────

    #[test]
    fn padded_trims_shorter_silence_than_default() {
        // 150ms silence + 300ms speech. Default (200ms min) won't trim;
        // padded (100ms min) should.
        let sr = 16_000.0;
        let silence_samples = (sr * 0.15) as usize;
        let speech_samples = (sr * 0.3) as usize;
        let total = silence_samples + speech_samples;
        let mut samples = vec![0.0f32; total];
        for i in silence_samples..total {
            let t = i as f64 / sr;
            samples[i] = 0.5 * (2.0 * std::f64::consts::PI * 300.0 * t).sin() as f32;
        }

        let seg_end_us = (total as f64 / sr * 1_000_000.0) as i64;

        let default_trimmed = trim_leading_silence(&samples, 0, seg_end_us, sr);
        assert_eq!(default_trimmed, 0, "Default should NOT trim 150ms silence");

        let padded_trimmed = trim_leading_silence_padded(&samples, 0, seg_end_us, sr);
        assert!(padded_trimmed > 0, "Padded should trim 150ms silence");
        assert!(
            padded_trimmed >= 100_000,
            "Padded trim {padded_trimmed} µs too early"
        );
    }
}
