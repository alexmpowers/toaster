//! Boundary-alignment helpers for transcribe_media_file (extracted from mod.rs).

use super::{WordAlignmentMeta, SAMPLE_RATE_HZ};
use crate::audio_toolkit::timing::us_to_sample as timing_us_to_sample;
use crate::managers::editor::Word;

pub(super) fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
    (sum_sq / samples.len() as f32).sqrt()
}

/// Convert a sample index to microseconds at `SAMPLE_RATE_HZ`.
///
/// Delegates to [`crate::audio_toolkit::timing`] which uses
/// nearest-integer rounding (todo `p0-rounding-policy`). The previous
/// implementation truncated, which biased every conversion toward earlier
/// samples and accumulated drift on µs↔sample round-trips.
pub(super) fn sample_to_us(sample_idx: usize) -> i64 {
    crate::audio_toolkit::timing::sample_to_us(sample_idx, SAMPLE_RATE_HZ)
}

/// Convert microseconds to a sample index at `SAMPLE_RATE_HZ`, clamped to
/// `[0, total_samples - 1]`.
///
/// The clamp to `total_samples - 1` is *intentional* truncation — it
/// guarantees the result is a valid frame index for indexing into the
/// PCM buffer. The µs→sample math itself uses nearest-integer rounding
/// per the policy in [`crate::audio_toolkit::timing`].
pub(super) fn us_to_sample(timestamp_us: i64, total_samples: usize) -> usize {
    crate::audio_toolkit::timing::us_to_sample_clamped(timestamp_us, SAMPLE_RATE_HZ, total_samples)
}

pub(super) fn snap_to_zero_crossing(samples: &[f32], target: usize, half_window: usize) -> usize {
    if samples.len() < 2 {
        return target.min(samples.len().saturating_sub(1));
    }

    let max_z = samples.len().saturating_sub(2);
    let zc_start = target.saturating_sub(half_window).min(max_z);
    let zc_end = (target + half_window).min(max_z);

    let mut best_zc = target.min(max_z);
    let mut best_dist = usize::MAX;
    for z in zc_start..=zc_end {
        if samples[z] * samples[z + 1] <= 0.0 {
            let dist = z.abs_diff(target);
            if dist < best_dist {
                best_dist = dist;
                best_zc = z;
            }
        }
    }
    best_zc
}

pub(super) fn find_local_low_energy_boundary(
    samples: &[f32],
    center_sample: usize,
    half_window_samples: usize,
    rms_window_samples: usize,
    step_samples: usize,
) -> Option<(usize, f32, f32)> {
    if samples.len() < rms_window_samples || rms_window_samples == 0 || step_samples == 0 {
        return None;
    }

    let search_start = center_sample.saturating_sub(half_window_samples);
    let search_end = (center_sample + half_window_samples).min(samples.len());
    if search_start >= search_end || search_end - search_start < rms_window_samples {
        return None;
    }

    let mut min_energy = f32::MAX;
    let mut min_pos = center_sample;
    let mut pos = search_start;
    while pos + rms_window_samples <= search_end {
        let energy = rms(&samples[pos..pos + rms_window_samples]);
        if energy < min_energy {
            min_energy = energy;
            min_pos = pos + rms_window_samples / 2;
        }
        pos += step_samples;
    }

    let center_start = center_sample
        .saturating_sub(rms_window_samples / 2)
        .min(samples.len().saturating_sub(rms_window_samples));
    let center_end = (center_start + rms_window_samples).min(samples.len());
    let center_energy = rms(&samples[center_start..center_end]);

    Some((min_pos, min_energy, center_energy))
}

pub(super) fn normalized_word_text(text: &str) -> String {
    text.trim_matches(|c: char| !c.is_alphanumeric())
        .to_lowercase()
}

pub(super) fn boundary_has_interpolated_pattern(
    words: &[Word],
    meta: Option<&[WordAlignmentMeta]>,
    boundary_idx: usize,
) -> bool {
    let Some(meta) = meta else {
        return false;
    };
    if boundary_idx + 1 >= words.len() || meta.len() != words.len() {
        return false;
    }

    if meta[boundary_idx].interpolated || meta[boundary_idx + 1].interpolated {
        return true;
    }

    let left = normalized_word_text(&words[boundary_idx].text);
    let right = normalized_word_text(&words[boundary_idx + 1].text);
    if left.is_empty() || right.is_empty() || left != right {
        return false;
    }

    (boundary_idx > 0 && meta[boundary_idx - 1].interpolated)
        || (boundary_idx + 2 < words.len() && meta[boundary_idx + 2].interpolated)
}

/// **Non-authoritative only; skipped entirely for authoritative timestamps.**
///
/// After the DP aligner places interior boundaries inside each ASR segment,
/// this pass runs as a narrow, confidence-gated re-align for boundaries
/// that still look suspicious. DP-aligned non-interpolated boundaries are
/// trusted and skipped. When confidence is unknown (all current engines),
/// requires at least 2 independent heuristic signals or an interpolated
/// pattern — a single heuristic alone is insufficient.
///
/// Called from `transcribe_media_file` only when `!authoritative`.
/// Authoritative engines (Parakeet word-level) preserve native timestamps.
pub(super) fn realign_suspicious_spans(
    words: &mut [Word],
    samples: &[f32],
    meta: Option<&[WordAlignmentMeta]>,
) {
    if words.len() < 2 || samples.is_empty() {
        return;
    }

    const LOCAL_WINDOW_US: i64 = 120_000;
    const RMS_WINDOW_SAMPLES: usize = 80; // 5 ms at 16 kHz
    const RMS_STEP_SAMPLES: usize = 40; // 2.5 ms
    const ZC_SNAP_HALF: usize = 32; // ±2 ms
    const MIN_WORD_US: i64 = 10_000;
    const LOW_CONF_THRESHOLD: f32 = 0.45;
    const MAX_REALIGN_BOUNDARIES: usize = 256;

    let half_window_samples = timing_us_to_sample(LOCAL_WINDOW_US, SAMPLE_RATE_HZ);
    let mut adjusted = 0usize;

    for i in 0..words.len() - 1 {
        if adjusted >= MAX_REALIGN_BOUNDARIES {
            break;
        }

        // Skip boundaries where both adjacent words were DP-aligned and
        // neither is interpolated. The DP forced aligner's global
        // energy-deviation optimization already placed these boundaries
        // at the best available position — overriding them with a local
        // heuristic scan degrades accuracy.
        if let Some(m) = meta {
            let left_trusted = m[i].dp_aligned && !m[i].interpolated;
            let right_trusted = m.get(i + 1).is_some_and(|r| r.dp_aligned && !r.interpolated);
            if left_trusted && right_trusted {
                continue;
            }
        }

        let left_duration = (words[i].end_us - words[i].start_us).max(0);
        let right_duration = (words[i + 1].end_us - words[i + 1].start_us).max(0);
        let min_duration = left_duration.min(right_duration);
        let max_duration = left_duration.max(right_duration);

        let low_confidence = [words[i].confidence, words[i + 1].confidence]
            .iter()
            .any(|&c| (0.0..LOW_CONF_THRESHOLD).contains(&c));
        let confidence_unknown = words[i].confidence < 0.0 || words[i + 1].confidence < 0.0;
        let very_short_word = left_duration < 35_000 || right_duration < 35_000;
        let abrupt_duration_jump = min_duration > 0
            && max_duration > min_duration * 3
            && (max_duration - min_duration) > 80_000;
        let interpolated_pattern = boundary_has_interpolated_pattern(words, meta, i);

        let boundary_sample = us_to_sample(words[i].end_us, samples.len());
        let Some((low_energy_sample, min_energy, boundary_energy)) = find_local_low_energy_boundary(
            samples,
            boundary_sample,
            half_window_samples,
            RMS_WINDOW_SAMPLES,
            RMS_STEP_SAMPLES,
        ) else {
            continue;
        };

        // Boundary in high-energy region often indicates a cut in the middle of phonemes.
        let boundary_high_energy = boundary_energy > (min_energy * 1.35 + 1e-5);

        // When confidence is known and low, any heuristic is enough to trigger.
        // When confidence is unknown (sentinel -1.0 — all current engines),
        // require stronger evidence: interpolated pattern always qualifies,
        // otherwise need at least 2 independent heuristics. This prevents
        // near-universal realignment that was overriding valid timestamps.
        let heuristic_count = [very_short_word, abrupt_duration_jump, boundary_high_energy]
            .iter()
            .filter(|&&x| x)
            .count();
        let heuristic_suspicious = interpolated_pattern || heuristic_count >= 2;
        if !(low_confidence || (confidence_unknown && heuristic_suspicious)) {
            continue;
        }

        let snapped_sample = snap_to_zero_crossing(samples, low_energy_sample, ZC_SNAP_HALF);
        let refined_us = sample_to_us(snapped_sample);
        if (refined_us - words[i].end_us).abs() < 1_000 {
            continue;
        }

        if refined_us > words[i].start_us + MIN_WORD_US
            && refined_us < words[i + 1].end_us - MIN_WORD_US
        {
            words[i].end_us = refined_us;
            words[i + 1].start_us = refined_us;
            adjusted += 1;
        }
    }
}

// Refine word boundaries with hybrid RMS + zero-crossing snapping.
//
// After proportional timestamp distribution, word boundaries may fall in the
// middle of speech. This function performs a two-stage refinement per boundary:
//
// 1. **RMS energy scan** (±80 ms): slides a 5 ms window across the search range
//    and picks the centre of the lowest-energy window — the most likely gap between
//    words.
// 2. **Zero-crossing snap** (±2 ms around the energy minimum): moves the candidate
//    to the nearest sample index where the signal crosses zero, avoiding a cut mid-
//    waveform cycle which would produce an audible click on export.
//
// Monotonic ordering and per-word minimum-duration constraints are preserved.
//
// `samples` must be 16 kHz mono f32 audio.

/// **Fallback-only safety net.** Runs after the DP aligner as a
/// pre-correction for char-proportional boundaries that surface through
/// the fallback path: when one word in an adjacent pair is very short
/// (<200 ms), scan ±200 ms around the boundary for the lowest-energy
/// point and snap the boundary there. Boundaries where both adjacent
/// words were DP-aligned are skipped — the global optimizer already
/// placed them optimally.
///
/// `samples` must be 16 kHz mono f32 audio.
pub(super) fn correct_short_word_boundaries(
    words: &mut [Word],
    samples: &[f32],
    meta: Option<&[WordAlignmentMeta]>,
) {
    const SAMPLE_RATE: f64 = 16000.0;
    const SHORT_THRESHOLD_US: i64 = 200_000; // 200ms
    const SEARCH_US: i64 = 200_000; // search ±200ms
    const RMS_WINDOW: usize = 48; // 3ms

    for i in 0..words.len().saturating_sub(1) {
        // Skip boundaries where both adjacent words were DP-aligned.
        // The DP aligner already placed this boundary at the energy-optimal
        // position within the segment — re-scanning would contradict it.
        if let Some(m) = meta {
            if m[i].dp_aligned && m[i + 1].dp_aligned {
                continue;
            }
        }

        let left_dur = words[i].end_us - words[i].start_us;
        let right_dur = words[i + 1].end_us - words[i + 1].start_us;

        // Only correct when one word is much shorter than the other
        if left_dur >= SHORT_THRESHOLD_US && right_dur >= SHORT_THRESHOLD_US {
            continue;
        }
        if left_dur <= 0 || right_dur <= 0 {
            continue;
        }

        let boundary_us = words[i].end_us;
        let center_sample = timing_us_to_sample(boundary_us, SAMPLE_RATE);
        let half_window = timing_us_to_sample(SEARCH_US, SAMPLE_RATE);

        let search_start = center_sample.saturating_sub(half_window);
        let search_end = (center_sample + half_window).min(samples.len());

        if search_end - search_start < RMS_WINDOW {
            continue;
        }

        // Find the minimum energy point in the search window
        let mut min_energy = f32::MAX;
        let mut min_pos = center_sample;
        let mut pos = search_start;
        while pos + RMS_WINDOW <= search_end {
            let mut sum_sq = 0.0f32;
            for s in &samples[pos..pos + RMS_WINDOW] {
                sum_sq += s * s;
            }
            let energy = (sum_sq / RMS_WINDOW as f32).sqrt();
            if energy < min_energy {
                min_energy = energy;
                min_pos = pos + RMS_WINDOW / 2;
            }
            pos += RMS_WINDOW / 2;
        }

        let refined_us = crate::audio_toolkit::timing::sample_to_us(min_pos, SAMPLE_RATE);

        // Only apply if it preserves minimum word durations
        let min_word_us = 10_000; // 10ms
        if refined_us > words[i].start_us + min_word_us
            && refined_us < words[i + 1].end_us - min_word_us
        {
            words[i].end_us = refined_us;
            words[i + 1].start_us = refined_us;
        }
    }
}

/// **Fallback-only safety net.** Runs after the DP aligner to nudge
/// boundaries that still land in non-minimum energy regions. Only fires
/// on boundaries where at least one adjacent word came from the
/// char-proportional fallback path. DP-aligned boundaries are trusted
/// and skipped — the global energy-deviation optimization already
/// placed them at the best available position.
pub(super) fn refine_word_boundaries(
    words: &mut [Word],
    samples: &[f32],
    meta: Option<&[WordAlignmentMeta]>,
) {
    if words.len() < 2 || samples.is_empty() {
        return;
    }

    const SEARCH_WINDOW_US: i64 = 80_000; // ±80ms default search window
    const SHORT_WORD_SEARCH_WINDOW_US: i64 = 160_000; // ±160ms for short-word boundaries
    const SHORT_WORD_DURATION_THRESHOLD_US: i64 = 200_000; // words < 200ms are "short"
    const RMS_WINDOW_SAMPLES: usize = 80; // 5ms RMS analysis window (16000 * 0.005)
                                          // ±2 ms at 16 kHz = 32 samples; tight enough to stay near the energy dip
    const ZC_SNAP_HALF: usize = 32;

    // For each boundary between adjacent words, find the minimum energy point
    // then snap it to the nearest zero-crossing.
    for i in 0..words.len() - 1 {
        // Skip boundaries where both adjacent words were DP-aligned.
        // The DP aligner's global energy-deviation optimization already
        // placed these at the best available position.
        if let Some(m) = meta {
            if m[i].dp_aligned && m[i + 1].dp_aligned {
                continue;
            }
        }

        let boundary_us = words[i].end_us;

        // Use a wider search window when either adjacent word is short.
        // Short leading words are the primary source of audible remnants
        // after deletion because proportional split underestimates their
        // duration.
        let left_dur = (words[i].end_us - words[i].start_us).max(0);
        let right_dur = (words[i + 1].end_us - words[i + 1].start_us).max(0);
        let window_us = if left_dur < SHORT_WORD_DURATION_THRESHOLD_US
            || right_dur < SHORT_WORD_DURATION_THRESHOLD_US
        {
            SHORT_WORD_SEARCH_WINDOW_US
        } else {
            SEARCH_WINDOW_US
        };

        // Search window in samples
        let center_sample = timing_us_to_sample(boundary_us, SAMPLE_RATE_HZ);
        let half_window_samples = timing_us_to_sample(window_us, SAMPLE_RATE_HZ);

        let search_start = center_sample.saturating_sub(half_window_samples);
        let search_end = (center_sample + half_window_samples).min(samples.len());

        if search_start >= search_end || search_end - search_start < RMS_WINDOW_SAMPLES {
            continue;
        }

        // Stage 1: slide the RMS window and find the minimum-energy centre.
        let mut min_energy = f32::MAX;
        let mut min_pos = center_sample;

        let mut pos = search_start;
        while pos + RMS_WINDOW_SAMPLES <= search_end {
            let energy = rms(&samples[pos..pos + RMS_WINDOW_SAMPLES]);
            if energy < min_energy {
                min_energy = energy;
                min_pos = pos + RMS_WINDOW_SAMPLES / 2; // centre of the window
            }
            pos += RMS_WINDOW_SAMPLES / 2; // 50 % overlap for smooth coverage
        }

        // If local energy is essentially flat around this boundary, moving it
        // is more likely to introduce drift than improve seam quality.
        // Require a meaningful energy dip before applying a correction.
        let center_start = center_sample
            .saturating_sub(RMS_WINDOW_SAMPLES / 2)
            .min(samples.len().saturating_sub(RMS_WINDOW_SAMPLES));
        let center_end = (center_start + RMS_WINDOW_SAMPLES).min(samples.len());
        let center_energy = rms(&samples[center_start..center_end]);
        const MIN_DIP_RATIO: f32 = 0.97; // at least 3% dip to consider reliable
        if center_energy > 1e-6 && min_energy >= center_energy * MIN_DIP_RATIO {
            // No clear energy dip found. For short coarticulated words (e.g.,
            // "new release" spoken without pause), the proportional char-weight
            // boundary can land mid-phoneme. Use the minimum-energy point anyway
            // when either adjacent word is very short — even a marginal dip is
            // better than a proportional guess through active speech.
            const COARTICULATED_SHORT_THRESHOLD_US: i64 = 250_000; // 250ms
            let is_short_boundary = left_dur < COARTICULATED_SHORT_THRESHOLD_US
                || right_dur < COARTICULATED_SHORT_THRESHOLD_US;
            if !is_short_boundary {
                continue;
            }
            // For short words: still use min_energy position but require it
            // differs from center by at least 1 RMS window step to avoid
            // no-op corrections.
            if min_pos.abs_diff(center_sample) < RMS_WINDOW_SAMPLES / 2 {
                continue;
            }
        }

        // Stage 2: snap min_pos to the nearest zero-crossing within ±ZC_SNAP_HALF.
        // A zero-crossing exists between index z and z+1 when the two samples have
        // opposite signs (or one is exactly zero).
        let zc_start = min_pos.saturating_sub(ZC_SNAP_HALF);
        let zc_end = (min_pos + ZC_SNAP_HALF).min(samples.len().saturating_sub(1));

        let mut best_zc = min_pos;
        let mut best_dist = usize::MAX;
        for z in zc_start..zc_end {
            if samples[z] * samples[z + 1] <= 0.0 {
                let dist = z.abs_diff(min_pos);
                if dist < best_dist {
                    best_dist = dist;
                    best_zc = z;
                }
            }
        }
        min_pos = best_zc;

        // Convert back to microseconds
        let refined_us = sample_to_us(min_pos);

        // Only snap if the refined point preserves minimum word durations on both
        // sides, keeping monotonic ordering intact.
        let min_word_us = 10_000; // minimum 10 ms per word
        if refined_us > words[i].start_us + min_word_us
            && refined_us < words[i + 1].end_us - min_word_us
        {
            words[i].end_us = refined_us;
            words[i + 1].start_us = refined_us;
        }
    }
}

/// Align the start of segment-leading words to the true speech onset.
///
/// **Safety net; primary timing comes from forced alignment.** The DP
/// aligner pins `word[0].start` to the engine-reported `seg_start_us`
/// verbatim. Whisper's segment `start` often lands at or slightly after
/// the first phoneme rather than at the acoustic onset (the pre-voice
/// burst of a plosive, the initial fricative noise, etc.).  When the
/// leading word is subsequently deleted, audio before `start_us` that
/// still contains speech energy leaks through.
///
/// This pass scans backwards from the first word's nominal start to find
/// the earliest sample whose short-term energy exceeds a threshold derived
/// from the word's body.  The word's `start_us` is then shifted to that
/// onset point (snapped to a zero-crossing for click-free cuts).
///
/// It also pushes the first inter-word boundary later if the first word is
/// very short, using a wider energy search to ensure the gap lands in actual
/// silence rather than mid-phoneme.
///
/// Monotonic ordering is preserved: onset can only move earlier (never past
/// the previous word's end), and inter-word shifts respect minimum durations.
pub(super) fn align_onset_boundaries(words: &mut [Word], samples: &[f32]) {
    if words.is_empty() || samples.is_empty() {
        return;
    }

    const ONSET_SEARCH_US: i64 = 50_000; // search up to 50 ms before nominal start
    const RMS_WINDOW: usize = 48; // 3 ms at 16 kHz
    const RMS_STEP: usize = 16; // 1 ms step
    const ZC_SNAP_HALF: usize = 32;
    const MIN_WORD_US: i64 = 10_000;

    // Phase 1: pull each segment-leading word's start to the true onset.
    // A word is "segment-leading" if it is the first word overall, or there
    // is a gap (≥ 20 ms) between it and the previous word's end, which
    // typically marks a segment boundary.
    for i in 0..words.len() {
        let is_segment_start = i == 0 || (words[i].start_us - words[i - 1].end_us) >= 20_000;
        if !is_segment_start {
            continue;
        }

        let nominal_start_sample = us_to_sample(words[i].start_us, samples.len());

        // Measure the word's body energy (first 20 ms of the word body).
        let body_start = nominal_start_sample;
        let body_end = (body_start + 320).min(samples.len()); // 20 ms
        if body_end <= body_start + RMS_WINDOW {
            continue;
        }
        let body_energy = rms(&samples[body_start..body_end]);
        if body_energy < 1e-5 {
            continue; // silence — nothing to align
        }

        // Onset threshold: 15% of body energy
        let onset_threshold = body_energy * 0.15;

        // Search backwards from nominal start
        let onset_search_samples = timing_us_to_sample(ONSET_SEARCH_US, SAMPLE_RATE_HZ);
        let search_start = nominal_start_sample.saturating_sub(onset_search_samples);
        // Don't cross into the previous word
        let earliest_allowed = if i > 0 {
            us_to_sample(words[i - 1].end_us, samples.len())
        } else {
            0
        };
        let search_start = search_start.max(earliest_allowed);

        if nominal_start_sample <= search_start + RMS_WINDOW {
            continue;
        }

        // Scan backwards: find the earliest position whose energy is above
        // threshold (i.e. the onset of speech).
        let mut onset_sample = nominal_start_sample;
        let mut pos = nominal_start_sample.saturating_sub(RMS_WINDOW);
        while pos >= search_start {
            let end = (pos + RMS_WINDOW).min(samples.len());
            if end - pos < RMS_WINDOW {
                break;
            }
            let e = rms(&samples[pos..end]);
            if e >= onset_threshold {
                onset_sample = pos;
            } else {
                // Energy dropped below threshold — onset is at the last
                // above-threshold position (already recorded).
                break;
            }
            if pos < RMS_STEP {
                break;
            }
            pos -= RMS_STEP;
        }

        if onset_sample < nominal_start_sample {
            let snapped = snap_to_zero_crossing(samples, onset_sample, ZC_SNAP_HALF);
            let onset_us = sample_to_us(snapped);
            let lower_bound = if i > 0 {
                words[i - 1].end_us + MIN_WORD_US
            } else {
                0
            };
            if onset_us >= lower_bound && onset_us < words[i].start_us {
                words[i].start_us = onset_us;
            }
        }
    }
}
