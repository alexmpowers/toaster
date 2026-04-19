//! Audio-aware disfluency cleanup.
//!
//! Given a word list and the source audio samples, this module:
//!
//! 1. Finds groups of adjacent repeated words (case-insensitive, ignoring
//!    already-deleted words) — e.g. `the the`, `best best`, or `the the the`.
//! 2. Scores every member of each group against the audio: peak level,
//!    RMS, silence ratio, and **spectral clarity** (tonal / HF-ratio /
//!    centroid-motion via `splice::clarity`) across the word's
//!    [start_us, end_us] window.
//! 3. Picks the single highest-scoring member as the survivor and marks
//!    every other member deleted.
//!
//! There are no positional heuristics (no "keep first" or "keep last"):
//! if the user mumbled the first of a pair, the second is kept; if the
//! user mumbled the last of a triple, the first or middle is kept —
//! whichever scored highest on the shared audio feature set.
//!
//! The scorer is the Rust counterpart to the Python
//! `articulation_score()` in `scripts/eval-verifier/audio_features.py`
//! and is designed to stay behaviorally identical so the eval-verifier
//! harness can keep scoring the live backend. **If you change the
//! weighting or add a term here, you MUST mirror it in the Python
//! function in the same commit.**

use crate::managers::editor::Word;
use crate::managers::splice::clarity::{self, SpectralClarity};

/// Result of scoring a single word against the source audio.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)] // peak/rms/silence_ratio/clarity are kept for future diagnostics & logging
pub struct WordClarity {
    /// Articulation score in [0.0, 1.0]. Higher = clearer.
    pub articulation: f32,
    /// Peak sample magnitude in the word's window.
    pub peak: f32,
    /// RMS sample magnitude in the word's window.
    pub rms: f32,
    /// Fraction of the window below the silence threshold.
    pub silence_ratio: f32,
    /// Spectral clarity sub-features (tonal/hf/centroid_motion/score).
    pub spectral: SpectralClarity,
}

/// A group of adjacent repeated words found in a transcript.
#[derive(Debug, Clone)]
pub struct RepeatGroup {
    /// Indices into the original `Word` slice, in order.
    pub members: Vec<usize>,
    /// Normalised form of the repeated token (lowercase, stripped).
    pub token: String,
}

/// A single audio-driven survivor decision.
#[derive(Debug, Clone)]
pub struct GroupDecision {
    pub group: RepeatGroup,
    /// Index (into the original `Word` slice) of the chosen survivor.
    pub survivor: usize,
    /// Indices to mark deleted (every group member except the survivor).
    pub losers: Vec<usize>,
    /// Articulation score of the survivor.
    pub survivor_score: f32,
    /// Per-member scores in the same order as `group.members`.
    pub member_scores: Vec<WordClarity>,
}

/// Normalise a word for repeat-group detection: lowercase, strip ASCII
/// punctuation from both ends. Empty strings are returned as-is.
pub fn normalize_token(word: &str) -> String {
    word.trim_matches(|c: char| c.is_ascii_punctuation())
        .to_lowercase()
}

/// Find all runs of 2+ adjacent identical (by `normalize_token`) words,
/// skipping any that are already deleted.
pub fn find_repeat_groups(words: &[Word]) -> Vec<RepeatGroup> {
    let mut groups = Vec::new();
    let mut i = 0;
    while i < words.len() {
        if words[i].deleted {
            i += 1;
            continue;
        }
        let token = normalize_token(&words[i].text);
        if token.is_empty() {
            i += 1;
            continue;
        }
        let mut members = vec![i];
        let mut j = i + 1;
        while j < words.len() {
            if words[j].deleted {
                j += 1;
                continue;
            }
            if normalize_token(&words[j].text) != token {
                break;
            }
            members.push(j);
            j += 1;
        }
        if members.len() >= 2 {
            groups.push(RepeatGroup { members, token });
        }
        i = j.max(i + 1);
    }
    groups
}

/// Score a word against a 16 kHz mono f32 sample buffer.
///
/// Articulation v2 is a weighted sum of four terms in `[0, 1]`:
///   * 0.40 * peak_term     — how close the peak is to -6 dBFS
///   * 0.25 * rms_term      — how close the RMS is to -14 dBFS
///   * 0.15 * (1 - silence) — how much of the window is above the silence floor
///   * 0.20 * spectral.score — tonal + HF + centroid-motion clarity (from rustfft)
///
/// Matches `audio_features.articulation_score()` in the verifier so the
/// live cleanup and the fixture-based eval harness share one formula.
/// Spectral term degrades to neutral (0.5) on windows shorter than one
/// FFT frame (32 ms at 16 kHz), so very short words still score sensibly.
pub fn score_word(word: &Word, samples: &[f32], sample_rate: u32) -> WordClarity {
    if word.start_us < 0 || word.end_us <= word.start_us || samples.is_empty() {
        return WordClarity {
            articulation: 0.0,
            peak: 0.0,
            rms: 0.0,
            silence_ratio: 1.0,
            spectral: SpectralClarity::neutral(),
        };
    }
    let sr = sample_rate as i64;
    // microseconds -> samples: us * sr / 1_000_000
    let to_sample = |us: i64| -> usize {
        let s = (us.saturating_mul(sr) / 1_000_000).max(0) as usize;
        s.min(samples.len())
    };
    let start = to_sample(word.start_us);
    let end = to_sample(word.end_us).max(start);
    if end <= start {
        return WordClarity {
            articulation: 0.0,
            peak: 0.0,
            rms: 0.0,
            silence_ratio: 1.0,
            spectral: SpectralClarity::neutral(),
        };
    }
    let window = &samples[start..end];
    let len_f = window.len() as f32;
    let mut peak: f32 = 0.0;
    let mut sumsq: f32 = 0.0;
    // -40 dBFS in linear amplitude for the silence floor.
    const SILENCE_FLOOR: f32 = 0.01;
    let mut silent_samples: usize = 0;
    for &s in window {
        let a = s.abs();
        if a > peak {
            peak = a;
        }
        sumsq += s * s;
        if a < SILENCE_FLOOR {
            silent_samples += 1;
        }
    }
    let rms = (sumsq / len_f).sqrt();
    let silence_ratio = (silent_samples as f32) / len_f;

    // Convert to dBFS with an epsilon floor so we never log(0).
    let peak_dbfs = 20.0 * (peak.max(1e-6)).log10();
    let rms_dbfs = 20.0 * (rms.max(1e-6)).log10();
    // peak_term: 1.0 at -6 dBFS, 0.0 at -40 dBFS.
    let peak_term = ((peak_dbfs + 40.0) / 34.0).clamp(0.0, 1.0);
    // rms_term: 1.0 at -15 dBFS, 0.0 at -50 dBFS.
    let rms_term = ((rms_dbfs + 50.0) / 35.0).clamp(0.0, 1.0);
    let silence_term = (1.0 - silence_ratio).clamp(0.0, 1.0);

    let spectral = clarity::analyze(window, sample_rate);

    let articulation = 0.40 * peak_term
        + 0.25 * rms_term
        + 0.15 * silence_term
        + 0.20 * spectral.score;

    WordClarity {
        articulation,
        peak,
        rms,
        silence_ratio,
        spectral,
    }
}

/// Plan a survivor-picking decision for every repeat group. Pure function
/// — does not mutate the word list.
pub fn plan(words: &[Word], samples: &[f32], sample_rate: u32) -> Vec<GroupDecision> {
    let groups = find_repeat_groups(words);
    let mut decisions = Vec::with_capacity(groups.len());
    for group in groups {
        let scores: Vec<WordClarity> = group
            .members
            .iter()
            .map(|&idx| score_word(&words[idx], samples, sample_rate))
            .collect();
        // Pick the highest-articulation member. In a tie, prefer the
        // earliest position — this is purely a deterministic tie-break
        // for reproducibility, NOT a positional preference; if the audio
        // differs at all the winner is chosen by score alone.
        let mut best_i = 0usize;
        let mut best_score = scores[0].articulation;
        for (k, s) in scores.iter().enumerate().skip(1) {
            if s.articulation > best_score {
                best_score = s.articulation;
                best_i = k;
            }
        }
        let survivor = group.members[best_i];
        let losers: Vec<usize> = group
            .members
            .iter()
            .copied()
            .filter(|&idx| idx != survivor)
            .collect();
        decisions.push(GroupDecision {
            group,
            survivor,
            losers,
            survivor_score: best_score,
            member_scores: scores,
        });
    }
    decisions
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(text: &str, start_us: i64, end_us: i64) -> Word {
        Word {
            text: text.to_string(),
            start_us,
            end_us,
            deleted: false,
            silenced: false,
            confidence: -1.0,
            speaker_id: -1,
        }
    }

    fn tone_word(samples: &mut Vec<f32>, sr: u32, amp: f32, dur_ms: u32) -> (i64, i64) {
        let start_sample = samples.len();
        let n = (sr as u64 * dur_ms as u64 / 1000) as usize;
        for i in 0..n {
            // 440 Hz carrier
            let t = i as f32 / sr as f32;
            let env = (i as f32 / n as f32 * std::f32::consts::PI).sin();
            samples.push(amp * env * (2.0 * std::f32::consts::PI * 440.0 * t).sin());
        }
        let start_us = (start_sample as i64 * 1_000_000) / sr as i64;
        let end_us = (samples.len() as i64 * 1_000_000) / sr as i64;
        (start_us, end_us)
    }

    fn silence(samples: &mut Vec<f32>, sr: u32, dur_ms: u32) {
        let n = (sr as u64 * dur_ms as u64 / 1000) as usize;
        samples.extend(std::iter::repeat_n(0.0, n));
    }

    #[test]
    fn finds_adjacent_repeats_including_triples() {
        let words = vec![
            w("the", 0, 100_000),
            w("the", 100_000, 200_000),
            w("the", 200_000, 300_000),
            w("best", 300_000, 400_000),
            w("best", 400_000, 500_000),
            w("part", 500_000, 600_000),
        ];
        let groups = find_repeat_groups(&words);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].members, vec![0, 1, 2]);
        assert_eq!(groups[0].token, "the");
        assert_eq!(groups[1].members, vec![3, 4]);
        assert_eq!(groups[1].token, "best");
    }

    #[test]
    fn skips_already_deleted_words() {
        let mut words = vec![
            w("the", 0, 100_000),
            w("um", 100_000, 200_000),
            w("the", 200_000, 300_000),
            w("part", 300_000, 400_000),
        ];
        words[1].deleted = true;
        let groups = find_repeat_groups(&words);
        // "the um the" with um deleted collapses to "the the".
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].members, vec![0, 2]);
    }

    #[test]
    fn punctuation_and_case_are_ignored() {
        let words = vec![
            w("The,", 0, 100_000),
            w("the.", 100_000, 200_000),
            w("part", 200_000, 300_000),
        ];
        let groups = find_repeat_groups(&words);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].token, "the");
    }

    #[test]
    fn survivor_is_the_clearest_not_the_first() {
        // Two "the" tokens: first is mumbled (amp 0.08), second is clear
        // (amp 0.55). Plan MUST pick the second as the survivor.
        let sr = 16_000u32;
        let mut samples: Vec<f32> = Vec::new();
        silence(&mut samples, sr, 20);
        let (s0, e0) = tone_word(&mut samples, sr, 0.08, 160);
        silence(&mut samples, sr, 80);
        let (s1, e1) = tone_word(&mut samples, sr, 0.55, 160);
        silence(&mut samples, sr, 20);
        let words = vec![w("the", s0, e0), w("the", s1, e1)];

        let decisions = plan(&words, &samples, sr);
        assert_eq!(decisions.len(), 1);
        assert_eq!(
            decisions[0].survivor, 1,
            "expected clearer second token to win, got {:?}",
            decisions[0]
        );
        assert_eq!(decisions[0].losers, vec![0]);
    }

    #[test]
    fn survivor_is_the_clearest_in_a_triple() {
        // Three tokens: mumbled, clear, mumbled. The middle one must win.
        let sr = 16_000u32;
        let mut samples: Vec<f32> = Vec::new();
        silence(&mut samples, sr, 20);
        let (s0, e0) = tone_word(&mut samples, sr, 0.08, 140);
        silence(&mut samples, sr, 60);
        let (s1, e1) = tone_word(&mut samples, sr, 0.55, 140);
        silence(&mut samples, sr, 60);
        let (s2, e2) = tone_word(&mut samples, sr, 0.05, 140);
        silence(&mut samples, sr, 20);
        let words = vec![w("the", s0, e0), w("the", s1, e1), w("the", s2, e2)];

        let decisions = plan(&words, &samples, sr);
        assert_eq!(decisions.len(), 1);
        assert_eq!(
            decisions[0].survivor, 1,
            "expected middle token (the only clear one) to win"
        );
        assert_eq!(decisions[0].losers, vec![0, 2]);
    }

    #[test]
    fn articulation_prefers_loud_over_silent() {
        let sr = 16_000u32;
        let mut silent_buf: Vec<f32> = Vec::new();
        silence(&mut silent_buf, sr, 200);
        let silent_word = w("the", 0, 200_000);
        let silent_score = score_word(&silent_word, &silent_buf, sr);
        assert!(
            silent_score.articulation < 0.25,
            "silent window should score low, got {:?}",
            silent_score
        );

        let mut loud_buf: Vec<f32> = Vec::new();
        let (ls, le) = tone_word(&mut loud_buf, sr, 0.55, 200);
        let loud_word = w("the", ls, le);
        let loud_score = score_word(&loud_word, &loud_buf, sr);
        assert!(
            loud_score.articulation > 0.7,
            "loud window should score high, got {:?}",
            loud_score
        );
    }
}
