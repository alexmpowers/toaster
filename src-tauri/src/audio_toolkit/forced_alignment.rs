//! Forced alignment for engines that do not emit authoritative per-word
//! timestamps (currently Whisper, and any future engine whose adapter sets
//! `word_timestamps_authoritative = false`).
//!
//! Why this exists
//! ---------------
//! Before `p1-authoritative-flag-actionable`, the primary source of per-word
//! timing for Whisper was a **character-proportional split** of each segment's
//! duration across its words. Char-split has no relationship to where phonemes
//! actually end — it is the single biggest source of boundary leakage in
//! connected speech. Downstream passes (`refine_word_boundaries`,
//! `correct_short_word_boundaries`, `align_onset_boundaries`,
//! `realign_suspicious_spans`) were heuristic patches on that broken primary
//! and could silently fail to converge.
//!
//! This module replaces the primary. The inputs are the same (a Whisper-style
//! segment span and the segment's words), but the boundary placement inside
//! the span is now chosen by a **1-D dynamic-programming alignment** against
//! frame-level acoustic energy:
//!
//!   * Each candidate boundary position pays a cost proportional to the local
//!     RMS energy (boundaries prefer quiet frames → between words).
//!   * Each candidate also pays a quadratic deviation penalty from its
//!     char-proportional expected position (keeps the solution from snapping
//!     all boundaries onto the same big silence).
//!   * Monotonicity is enforced by the DP recurrence itself (each boundary
//!     must land strictly after the previous one).
//!
//! The segment endpoints themselves (`seg_start_us`, `seg_end_us`) come from
//! the ASR engine and are treated as authoritative — we only redistribute the
//! interior boundaries.
//!
//! Fallback
//! --------
//! The aligner returns `None` when the inputs are too degenerate to produce a
//! meaningful result (zero duration, too few frames, single word). Callers
//! must fall back to the legacy char-proportional split in that case — see
//! `build_words_from_segments` in `commands/transcribe_file/mod.rs`.

use crate::audio_toolkit::timing;

/// 10 ms hop at 16 kHz → 160 samples. We keep the hop in seconds so the
/// module is sample-rate-agnostic even though every current caller passes
/// 16 kHz audio.
const FRAME_HOP_SEC: f64 = 0.010;
/// 25 ms analysis window at 16 kHz → 400 samples.
const FRAME_WIN_SEC: f64 = 0.025;

/// Frame-level energy envelope for a slice of PCM audio.
#[derive(Debug)]
pub struct EnergyFrames {
    /// Per-frame RMS energy. `frames[f]` is the RMS of samples starting at
    /// `f * hop_samples` in the slice.
    pub frames: Vec<f32>,
    pub hop_samples: usize,
    pub win_samples: usize,
    /// Sample rate the envelope was computed at.
    pub sample_rate_hz: f64,
}

impl EnergyFrames {
    pub fn compute(slice: &[f32], sample_rate_hz: f64) -> Self {
        let hop = ((sample_rate_hz * FRAME_HOP_SEC).round() as usize).max(1);
        let win = ((sample_rate_hz * FRAME_WIN_SEC).round() as usize).max(hop);

        let mut frames = Vec::new();
        if slice.len() >= win {
            let last_start = slice.len() - win;
            let mut pos = 0usize;
            while pos <= last_start {
                let end = pos + win;
                let mut sum_sq = 0.0f32;
                for s in &slice[pos..end] {
                    sum_sq += s * s;
                }
                frames.push((sum_sq / win as f32).sqrt());
                pos += hop;
            }
        }

        Self {
            frames,
            hop_samples: hop,
            win_samples: win,
            sample_rate_hz,
        }
    }

    /// Normalize frame energies to [0.0, 1.0] for use as a DP unit-less cost.
    /// Returns a vector even when all frames are silent (all zeros).
    fn normalized(&self) -> Vec<f32> {
        if self.frames.is_empty() {
            return Vec::new();
        }
        let max = self.frames.iter().copied().fold(0.0f32, f32::max);
        if max <= 1e-9 {
            return vec![0.0; self.frames.len()];
        }
        self.frames.iter().map(|e| (*e / max).min(1.0)).collect()
    }
}

/// Convert a frame index within the slice back to an absolute microsecond
/// timestamp. `seg_start_us` is the start of the slice in the source audio.
fn frame_to_us(
    frame_idx: usize,
    hop_samples: usize,
    sample_rate_hz: f64,
    seg_start_us: i64,
) -> i64 {
    let sample_idx = frame_idx * hop_samples;
    seg_start_us + timing::sample_to_us(sample_idx, sample_rate_hz)
}

/// Aligned boundary result: per-word (start_us, end_us) tuples covering
/// exactly `[seg_start_us, seg_end_us]` without gaps or overlaps.
pub type AlignedWords = Vec<(i64, i64)>;

/// Deviation penalty weight for the DP objective. Cost at boundary frame `b`
/// for expected frame `e_i` over `F` total frames is
/// `energy_norm[b] + LAMBDA_DEV * ((b - e_i) / F)^2`.
///
/// Tuned so that in a segment with clear silences between words the energy
/// term dominates (aligner snaps to silences), while in a segment of fully
/// connected speech the deviation term dominates (aligner stays near the
/// char-proportional estimate rather than collapsing onto noise floor
/// fluctuations).
const LAMBDA_DEV: f32 = 0.35;

/// Minimum characters per word for weight computation. Prevents 1-letter
/// words like "I"/"a" from collapsing to zero weight.
const MIN_WORD_CHAR_WEIGHT: usize = 1;

/// Minimum frames between adjacent boundaries (guarantees non-degenerate
/// word durations of ≥ ~30 ms at 10 ms hop).
const MIN_BOUNDARY_SEP_FRAMES: usize = 3;

/// Half-width factor of the per-boundary search window, as a fraction of the
/// expected per-word span in frames. Wider than the legacy ±80 ms refinement
/// so alignment can recover from char-proportional being badly off on short
/// words next to long words.
const SEARCH_HALF_FACTOR: f32 = 0.6;

/// Minimum half-width of the search window in frames (~100 ms at 10 ms hop).
const MIN_SEARCH_HALF_FRAMES: usize = 10;

/// Per-segment forced alignment.
///
/// Given an engine-reported segment span `[seg_start_us, seg_end_us)` and the
/// words within it, places the N-1 interior boundaries at the DP-optimal
/// frames. The first word's start is pinned to `seg_start_us`; the last
/// word's end is pinned to `seg_end_us`.
///
/// Returns `None` when the inputs are too degenerate to align (no words,
/// non-positive duration, slice outside the sample buffer, or fewer frames
/// than `MIN_BOUNDARY_SEP_FRAMES * (N - 1) + 1`). Callers must fall back to
/// the legacy char-proportional split in that case.
pub fn align_words_in_segment(
    words: &[&str],
    seg_start_us: i64,
    seg_end_us: i64,
    samples: &[f32],
    sample_rate_hz: f64,
) -> Option<AlignedWords> {
    if words.is_empty() || seg_end_us <= seg_start_us {
        return None;
    }
    let n = words.len();

    // Single word: the whole segment is the word.
    if n == 1 {
        return Some(vec![(seg_start_us, seg_end_us)]);
    }

    // Extract the audio slice for this segment.
    let start_sample = timing::us_to_sample_clamped(seg_start_us, sample_rate_hz, samples.len());
    let end_sample_raw = timing::us_to_sample_clamped(seg_end_us, sample_rate_hz, samples.len());
    // `us_to_sample_clamped` clamps to `len-1`; we want an exclusive upper
    // bound, so reopen the range up to `samples.len()` when the segment
    // actually runs to the tail.
    let end_sample = if end_sample_raw + 1 >= samples.len() {
        samples.len()
    } else {
        end_sample_raw
    };
    if end_sample <= start_sample {
        return None;
    }
    let slice = &samples[start_sample..end_sample];

    let frames = EnergyFrames::compute(slice, sample_rate_hz);
    let f = frames.frames.len();
    // Need enough frames for N-1 boundaries with the minimum separation plus
    // the head/tail. If not, the DP result would be forced and meaningless.
    if f < MIN_BOUNDARY_SEP_FRAMES * n {
        return None;
    }

    let norm = frames.normalized();

    // Char-weight expected boundary positions in frames.
    let weights: Vec<usize> = words
        .iter()
        .map(|w| w.chars().count().max(MIN_WORD_CHAR_WEIGHT))
        .collect();
    let total_w: usize = weights.iter().sum();
    // cum[i] = sum of weights[0..i], so cum[n] == total_w.
    let mut cum = vec![0usize; n + 1];
    for i in 0..n {
        cum[i + 1] = cum[i] + weights[i];
    }
    // Expected frame for boundary i (i = 1..n-1). Use f - 1 so boundary n
    // corresponds to the last frame (end of the last word). Boundary 0 is
    // frame 0 (pinned), boundary n is frame f-1 (pinned).
    let expected_frame = |i: usize| -> usize {
        let frac = cum[i] as f32 / total_w as f32;
        ((frac * (f - 1) as f32).round() as usize).min(f - 1)
    };

    // Per-boundary search window half-width in frames.
    let avg_span_frames = (f as f32 / n as f32).max(1.0);
    let search_half = ((avg_span_frames * SEARCH_HALF_FACTOR) as usize).max(MIN_SEARCH_HALF_FRAMES);

    // DP. Internal boundaries are indices 1..=n-1; values are frame indices.
    // Let B = n-1 be the number of internal boundaries.
    let b = n - 1;

    // Candidate frames per boundary.
    let mut candidates: Vec<Vec<usize>> = Vec::with_capacity(b);
    for i in 1..=b {
        let e = expected_frame(i);
        let lo = e
            .saturating_sub(search_half)
            .max(i * MIN_BOUNDARY_SEP_FRAMES);
        let hi = (e + search_half).min(f.saturating_sub((b - i + 1) * MIN_BOUNDARY_SEP_FRAMES));
        if hi <= lo {
            // Not enough room for a meaningful search; abort and let caller
            // fall back.
            return None;
        }
        let mut row = Vec::with_capacity(hi - lo + 1);
        for frame in lo..=hi {
            row.push(frame);
        }
        candidates.push(row);
    }

    // local_cost[i][k] = unary cost of placing boundary i+1 at candidates[i][k].
    let local_cost = |i: usize, frame: usize| -> f32 {
        let e = expected_frame(i + 1) as f32;
        let dev = (frame as f32 - e) / f as f32;
        norm[frame] + LAMBDA_DEV * dev * dev
    };

    // dp[i][k] = minimum total cost placing boundaries 1..=i+1 such that
    // boundary i+1 is at candidates[i][k].
    // back[i][k] = index into candidates[i-1] chosen as predecessor.
    let mut dp: Vec<Vec<f32>> = candidates
        .iter()
        .map(|row| vec![f32::INFINITY; row.len()])
        .collect();
    let mut back: Vec<Vec<usize>> = candidates
        .iter()
        .map(|row| vec![0usize; row.len()])
        .collect();

    // Base row.
    for (k, &frame) in candidates[0].iter().enumerate() {
        dp[0][k] = local_cost(0, frame);
    }

    // Transition.
    for i in 1..b {
        for (k, &frame) in candidates[i].iter().enumerate() {
            let cost_k = local_cost(i, frame);
            let mut best = f32::INFINITY;
            let mut best_prev = 0usize;
            for (pk, &pframe) in candidates[i - 1].iter().enumerate() {
                if pframe + MIN_BOUNDARY_SEP_FRAMES > frame {
                    continue;
                }
                let cand = dp[i - 1][pk] + cost_k;
                if cand < best {
                    best = cand;
                    best_prev = pk;
                }
            }
            dp[i][k] = best;
            back[i][k] = best_prev;
        }
    }

    // Trace back from the last boundary's best endpoint.
    let mut end_k = 0usize;
    let mut end_cost = f32::INFINITY;
    for (k, &c) in dp[b - 1].iter().enumerate() {
        if c < end_cost {
            end_cost = c;
            end_k = k;
        }
    }
    if !end_cost.is_finite() {
        return None;
    }

    let mut chosen_frames = vec![0usize; b];
    chosen_frames[b - 1] = candidates[b - 1][end_k];
    for i in (1..b).rev() {
        let pk = back[i][end_k];
        chosen_frames[i - 1] = candidates[i - 1][pk];
        end_k = pk;
    }

    // Convert frames → µs. Boundary 0 = seg_start_us, boundary n = seg_end_us.
    let mut boundaries_us: Vec<i64> = Vec::with_capacity(n + 1);
    boundaries_us.push(seg_start_us);
    for &frame in &chosen_frames {
        boundaries_us.push(frame_to_us(
            frame,
            frames.hop_samples,
            sample_rate_hz,
            seg_start_us,
        ));
    }
    boundaries_us.push(seg_end_us);

    // Safety: clamp any boundary that slipped outside the segment span, and
    // enforce strict monotonicity with at least 1 µs spacing so the invariant
    // checks in `NormalizedTranscriptionResult::validate` never see a
    // zero-duration word from this path.
    for i in 1..=n {
        if boundaries_us[i] <= boundaries_us[i - 1] {
            boundaries_us[i] = boundaries_us[i - 1] + 1;
        }
        if boundaries_us[i] > seg_end_us {
            boundaries_us[i] = seg_end_us;
        }
    }
    boundaries_us[0] = seg_start_us;
    boundaries_us[n] = seg_end_us;

    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        out.push((boundaries_us[i], boundaries_us[i + 1]));
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a deterministic synthetic segment: N "words" each of
    /// `word_dur_sec` containing a tone, separated by `gap_sec` of silence.
    /// Returns (samples, boundaries_sec) where boundaries_sec has N entries
    /// of (word_start_sec, word_end_sec).
    fn synth_words(word_dur_sec: f64, gap_sec: f64, n: usize) -> (Vec<f32>, Vec<(f64, f64)>) {
        let sr = 16_000.0_f64;
        let word_samples = (sr * word_dur_sec) as usize;
        let gap_samples = (sr * gap_sec) as usize;
        let total = n * word_samples + (n - 1) * gap_samples;
        let mut samples = vec![0.0f32; total];
        let mut cursor = 0usize;
        let mut boundaries = Vec::with_capacity(n);
        for i in 0..n {
            let word_start = cursor;
            // 300 Hz tone at 0.5 amplitude.
            for (k, s) in samples
                .iter_mut()
                .skip(cursor)
                .take(word_samples)
                .enumerate()
            {
                let t = k as f64 / sr;
                *s = 0.5 * (2.0 * std::f64::consts::PI * 300.0 * t).sin() as f32;
            }
            cursor += word_samples;
            let word_end = cursor;
            boundaries.push((word_start as f64 / sr, word_end as f64 / sr));
            if i + 1 < n {
                cursor += gap_samples;
            }
        }
        (samples, boundaries)
    }

    #[test]
    fn aligner_snaps_boundaries_into_silence_gaps() {
        // 3 words of 300 ms separated by 200 ms of silence. Oracle gap
        // centers: 400 ms and 900 ms. Char-proportional (equal weights)
        // would put boundaries at t = 1.3/3 ≈ 433 ms and 867 ms — in silence
        // here, but off-center. The aligner should pull them toward the
        // acoustic minimum.
        let (samples, _) = synth_words(0.3, 0.2, 3);
        let seg_start_us = 0_i64;
        let seg_end_us = 1_300_000_i64;
        let words = ["alpha", "bravo", "charlie"];
        let aligned = align_words_in_segment(&words, seg_start_us, seg_end_us, &samples, 16_000.0)
            .expect("aligner must succeed on well-formed synthetic input");
        assert_eq!(aligned.len(), 3);
        assert_eq!(aligned[0].0, seg_start_us);
        assert_eq!(aligned[2].1, seg_end_us);

        // First boundary should land inside the first gap [300 ms, 500 ms].
        let b1 = aligned[0].1;
        assert!(
            (300_000..=500_000).contains(&b1),
            "boundary 1 at {b1} µs not in silence gap [300_000, 500_000]"
        );
        // Second boundary inside the second gap [800 ms, 1000 ms].
        let b2 = aligned[1].1;
        assert!(
            (800_000..=1_000_000).contains(&b2),
            "boundary 2 at {b2} µs not in silence gap [800_000, 1_000_000]"
        );
    }

    #[test]
    fn aligner_single_word_returns_whole_span() {
        let samples = vec![0.0_f32; 16_000];
        let out = align_words_in_segment(&["hello"], 100, 500_000, &samples, 16_000.0).unwrap();
        assert_eq!(out, vec![(100, 500_000)]);
    }

    #[test]
    fn aligner_returns_none_when_too_few_frames() {
        // 30 ms of audio but 4 words requested → insufficient frames.
        let samples = vec![0.0_f32; 480];
        let words = ["a", "b", "c", "d"];
        assert!(align_words_in_segment(&words, 0, 30_000, &samples, 16_000.0).is_none());
    }

    #[test]
    fn aligner_output_is_monotonic_and_covers_span() {
        let (samples, _) = synth_words(0.2, 0.1, 5);
        let total_us = (samples.len() as f64 / 16_000.0 * 1_000_000.0) as i64;
        let words = ["one", "two", "three", "four", "five"];
        let out =
            align_words_in_segment(&words, 0, total_us, &samples, 16_000.0).expect("must align");
        assert_eq!(out.len(), 5);
        assert_eq!(out[0].0, 0);
        assert_eq!(out[4].1, total_us);
        for i in 0..out.len() {
            assert!(out[i].0 < out[i].1, "word {i} has non-positive duration");
            if i + 1 < out.len() {
                assert_eq!(
                    out[i].1,
                    out[i + 1].0,
                    "word {i}/{} boundary is not contiguous",
                    i + 1
                );
            }
        }
    }
}
